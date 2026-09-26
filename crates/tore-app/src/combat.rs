//! Live range host, original geometry and sampled original effect art.
use crate::{
    AppResult,
    aircraft::Airframe,
    flight,
    render_snapshot::{
        AircraftPose, CombatArt, Damage, DebrisPose, Draw, EffectPose, Engine, PilotPose,
        ProjectilePose, RenderSnapshot,
    },
    sim_renderer::Contact,
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
use tore_sim::{
    attitude::{Basis, Vector},
    combat::live::{self, EffectKind, Event, Launcher},
};

#[derive(Default)]
pub struct FireInput {
    pub held: bool,
    inhibited: bool,
}
impl FireInput {
    pub fn space(&mut self, down: bool, repeat: bool, blocked: bool) {
        if !down {
            self.held = false;
            self.inhibited = false;
        } else if blocked {
            self.cancel();
            self.inhibited = true;
        } else if !repeat && !self.inhibited {
            self.held = true;
        }
    }
    pub fn cancel(&mut self) {
        self.inhibited |= self.held;
        self.held = false;
    }
}
/// Yaw, pitch and bank for one drawn airborne target.
///
/// With `ai_poses` off this is the existing straight-flight fixture rule: the
/// heading comes from the velocity and the aircraft is drawn level. With it on
/// the target carries a real attitude written by the AI bridge, so the stored
/// basis is used instead.
pub fn target_pose(target: &live::Target, ai_poses: bool) -> [f64; 3] {
    if ai_poses || target.hp <= 0 {
        target.basis.angles()
    } else {
        [target.velocity[0].atan2(target.velocity[2]), 0., 0.]
    }
}
/// The last two per-tick render snapshots and the frame's tick fraction.
/// They are presentation only and never feed back into sensors or physics.
struct RenderHistory {
    /// The snapshot one tick earlier; none right after a restart.
    previous: Option<RenderSnapshot>,
    current: RenderSnapshot,
    /// Each target's index in `previous` and in `current`, by id.
    places: [BTreeMap<u32, usize>; 2],
    alpha: f64,
}
impl Default for RenderHistory {
    fn default() -> Self {
        Self {
            previous: None,
            current: RenderSnapshot::default(),
            places: Default::default(),
            alpha: 1.0,
        }
    }
}
impl RenderHistory {
    fn places(snapshot: &RenderSnapshot) -> BTreeMap<u32, usize> {
        snapshot
            .targets
            .iter()
            .enumerate()
            .map(|(index, pose)| (pose.id, index))
            .collect()
    }
    /// Replaces the current snapshot, keeping the previous one.
    fn set_current(&mut self, current: RenderSnapshot) {
        self.places[1] = Self::places(&current);
        self.current = current;
    }
    /// The current snapshot becomes the previous one.
    fn advance(&mut self, next: RenderSnapshot) {
        let places = Self::places(&next);
        self.places[0] = std::mem::replace(&mut self.places[1], places);
        self.previous = Some(std::mem::replace(&mut self.current, next));
    }
    fn current_target(&self, id: u32) -> Option<&AircraftPose> {
        self.places[1]
            .get(&id)
            .map(|&index| &self.current.targets[index])
    }
    /// One target at the frame's tick fraction.
    fn presented_target(&self, id: u32) -> Option<AircraftPose> {
        let current = self.current_target(id)?;
        let previous = self
            .previous
            .as_ref()
            .zip(self.places[0].get(&id))
            .map(|(snapshot, &index)| &snapshot.targets[index]);
        Some(crate::render_snapshot::blend(
            previous,
            current,
            self.alpha.clamp(0., 1.),
        ))
    }
}

pub struct Combat {
    pub state: live::State,
    /// Effect, smoke, weapon and ejection art drawn from render snapshots.
    pub art: CombatArt,
    contrail_offsets: Vec<Vector>,
    contrail_sortie: u64,
    pub contrails: tore_sim::combat::smoke::Smoke,
    pub input: FireInput,
    pub controller: FireInput,
    pub range: bool,
    /// Draw airborne targets with their own stored attitude instead of the
    /// straight-flight fixture pose. Enabled by an AI creator launch; disabled
    /// for explicit fixtures, which keep their level velocity-based pose.
    pub ai_poses: bool,
    /// Pilot-only tapes retain their existing clean-aircraft initial state.
    pub clean_recording: bool,
    initial_ammo: Option<Vec<u16>>,
    render: RenderHistory,
    dummies: Vec<(usize, Vector)>,
    mission_spawns: Option<Vec<crate::ai_wings::MissionSpawn>>,
    /// The accepted Quick Mission layout, kept so restart rebuilds it exactly.
    pub mission_layout: Option<crate::quick_mission::MissionLayout>,
    dummy_models: Vec<Airframe>,
    dummy_configs: Vec<live::Configuration>,
    dummy_contrail_offsets: Vec<Vec<Vector>>,
    airport_objects: Vec<tore_sim::airport::StaticObject>,
    pub recorder: Option<crate::combat_tape::Recorder>,
    last_launcher: Option<Launcher>,
    /// Player commands since the mission recorder last looked. Nothing in
    /// flight reads them.
    notes: std::collections::VecDeque<CommandNote>,
}

/// A player command combat received, kept for the mission recording.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandNote {
    Command(live::Command),
    /// The trigger was let go while it was held.
    Release,
}
/// Command notes kept between drains; the oldest are dropped first.
const MAX_COMMAND_NOTES: usize = 64;

pub fn launcher(s: &flight::State) -> Launcher {
    Launcher {
        position: s.position,
        basis: Basis::new(s.yaw, s.pitch, s.bank),
        speed_fps: s.speed,
        velocity: s.velocity,
        bay_ready: !s.bay_available() || s.bay >= 0.95,
        // Selecting the passive infrared channel stops radar transmission
        // without changing the radar power switch itself.
        radar_power: s.radar,
        radar: s.radar && s.engine && s.sensors.channel == tore_sim::sensors::Channel::Radar,
        jammer: s.jammer && s.engine,
        alive: !s.crashed,
        controls: s.sensors,
    }
}
impl Combat {
    /// Player airborne startup convention: canonical gun selected and armed.
    pub fn apply_startup_weapons(&mut self) {
        apply_startup_weapon_state(&mut self.state);
    }
    pub fn uses_normal_startup_defaults(&self) -> bool {
        !self.range && self.recorder.is_none() && !self.clean_recording
    }
    pub fn add_airport_targets(&mut self, scene: &tore_sim::airport::Scene) -> AppResult<()> {
        scene.validate().map_err(std::io::Error::other)?;
        // A new layout replaces static identities atomically in the staged state.
        let mut staged = self.state.clone();
        staged.remove_ground_targets();
        for object in &scene.objects {
            Self::register_airport_object(&mut staged, object)?;
        }
        self.state = staged;
        self.airport_objects = scene.objects.clone();
        Ok(())
    }
    fn register_airport_object(
        state: &mut live::State,
        object: &tore_sim::airport::StaticObject,
    ) -> AppResult<()> {
        state.add_ground_target(object.id, object.bounds, object.hit_points, object.category)?;
        if let Some(target) = state.targets.iter_mut().find(|t| t.id == object.id) {
            target.signature.radar = object.radar_signature;
            target.signature.infrared = object.infrared_signature;
        }
        Ok(())
    }
    pub fn ground_name(&self, id: u32) -> Option<&str> {
        self.airport_objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.name.as_str())
    }
    pub fn new(h: &Airframe, data: &BTreeMap<String, Vec<u8>>, range: bool) -> AppResult<Self> {
        let config = live::Configuration::from_source(&h.profile, |name| {
            data.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing live-fire resource {name}")))
        })?;
        Self::configured(h, data, range, config, None)
    }
    pub fn with_loadout(
        h: &Airframe,
        data: &BTreeMap<String, Vec<u8>>,
        load: &tore_sim::combat::loadout::Loadout,
    ) -> AppResult<Self> {
        load.validate()?;
        Self::configured(
            h,
            data,
            false,
            load.configuration.clone(),
            Some(load.quantities.clone()),
        )
    }
    fn configured(
        h: &Airframe,
        data: &BTreeMap<String, Vec<u8>>,
        range: bool,
        config: live::Configuration,
        initial_ammo: Option<Vec<u16>>,
    ) -> AppResult<Self> {
        let mut art = CombatArt::load(data, &h.palette)?;
        art.add_weapon_shapes(&config, data);
        Ok(Self {
            art,
            contrail_offsets: h.contrail_offsets(),
            contrail_sortie: 0,
            contrails: Default::default(),
            state: live::State::new(config, true)?,
            dummies: Vec::new(),
            mission_spawns: None,
            mission_layout: None,
            dummy_models: Vec::new(),
            dummy_configs: Vec::new(),
            dummy_contrail_offsets: Vec::new(),
            airport_objects: Vec::new(),
            input: FireInput::default(),
            controller: FireInput::default(),
            range,
            ai_poses: false,
            clean_recording: false,
            initial_ammo,
            render: RenderHistory::default(),
            recorder: None,
            last_launcher: None,
            notes: Default::default(),
        })
    }
    /// The frame's fraction of the way from the previous tick to the current one.
    pub fn present_targets(&mut self, alpha: f64) {
        self.render.alpha = alpha;
    }

    /// Everything combat draws for this tick, as plain data. `wings` supplies
    /// the AI aircraft's devices and ejected pilots. An aircraft whose AI is
    /// not alive, or that has no AI, keeps the devices last drawn for it.
    pub fn snapshot(
        &self,
        player: &flight::State,
        wings: Option<&crate::ai_wings::AiWings>,
    ) -> RenderSnapshot {
        let ownship = self.dummies.is_empty();
        let model = |id: u32| {
            id.checked_sub(1)
                .and_then(|index| self.dummies.get(index as usize))
                .map(|(model, _)| self.dummy_models[*model].profile.id)
        };
        let draw = |id: u32| {
            if ownship {
                Draw::Ownship
            } else {
                model(id).map_or(Draw::Hidden, Draw::Model)
            }
        };
        let flying: BTreeMap<u32, [f64; crate::render_snapshot::DEVICES]> = wings
            .into_iter()
            .flat_map(|wings| wings.mission().actors())
            .filter(|actor| actor.alive())
            .map(|actor| (actor.id(), crate::render_snapshot::devices(actor.flight())))
            .collect();
        // AI aircraft whose afterburner is lit, for their flame lights.
        let burning: std::collections::BTreeSet<u32> = wings
            .into_iter()
            .flat_map(|wings| wings.mission().actors())
            .filter(|actor| actor.alive() && actor.flight().afterburner_active())
            .map(|actor| actor.id())
            .collect();
        // The last devices drawn hold once an aircraft stops flying.
        let simulated = |id: u32| {
            flying
                .get(&id)
                .copied()
                .or_else(|| self.render.current_target(id).and_then(|pose| pose.devices))
        };
        let config = self.state.configuration();
        let capacity = config.damage_capacity;
        let player_engine = Engine {
            lit: player.engine && player.fuel > 0.,
            afterburner: player.afterburner_active(),
            rates: player.auxiliary_rates,
            flame: player.afterburner_active()
                && player.escape.is_none()
                && self.state.player_hp > 0,
        };
        // Fixtures copy the player's state with their own crash flag.
        let fixture_afterburner = ownship
            && if player.crashed {
                let mut alive = player.clone();
                alive.crashed = false;
                alive.afterburner_active()
            } else {
                player_engine.afterburner
            };
        let model_engine = Engine {
            lit: true,
            afterburner: false,
            rates: [0.; 3],
            flame: false,
        };
        let pilot = |owner: u32, escape: &tore_sim::ejection::Escape| PilotPose {
            owner,
            position: escape.position,
            heading: escape.heading,
            phase: escape.phase,
        };
        RenderSnapshot {
            tick: self.state.tick(),
            player: AircraftPose {
                id: 0,
                aircraft: Some(config.aircraft),
                draw: Draw::Ownship,
                position: player.position,
                attitude: [player.yaw, player.pitch, player.bank],
                velocity: player.velocity,
                devices: Some(crate::render_snapshot::devices(player)),
                engine: player_engine,
                damage: Damage {
                    hp: self.state.player_hp,
                    initial_hp: capacity,
                    // The exact amounts the drawn fractions divide.
                    sections: self.state.player_damage_amounts(),
                    structural: self.state.player_damage_section(),
                },
                airborne: true,
                wreck: player.wreck.as_ref().map(|wreck| wreck.phase),
                crashed: player.crashed,
            },
            targets: self
                .state
                .targets
                .iter()
                .map(|t| AircraftPose {
                    id: t.id,
                    aircraft: t.aircraft,
                    draw: draw(t.id),
                    position: t.position,
                    attitude: target_pose(t, self.ai_poses),
                    velocity: t.velocity,
                    devices: simulated(t.id),
                    engine: Engine {
                        flame: t.airborne && t.hp > 0 && burning.contains(&t.id),
                        ..if ownship {
                            Engine {
                                afterburner: fixture_afterburner && t.hp > 0,
                                ..player_engine
                            }
                        } else {
                            model_engine
                        }
                    },
                    damage: Damage {
                        hp: t.hp,
                        initial_hp: t.initial_hp,
                        sections: t.localized_damage.amounts,
                        structural: t.localized_damage.structural_section,
                    },
                    airborne: t.airborne,
                    wreck: t.wreck.as_ref().map(|wreck| wreck.phase),
                    crashed: t.hp <= 0,
                })
                .collect(),
            projectiles: self
                .state
                .projectiles
                .iter()
                .map(|p| {
                    let weapon = p.weapon(config);
                    ProjectilePose {
                        id: p.id,
                        owner: p.owner,
                        weapon: weapon.source.clone(),
                        shape: weapon.shape.clone(),
                        gun: live::is_gun(weapon),
                        tracer: p.tracer,
                        position: p.position,
                        previous: p.previous,
                        direction: p.direction,
                        target: p.target,
                        incoming: p.incoming,
                        speed_f8: p.speed_f8,
                    }
                })
                .collect(),
            effects: self
                .state
                .effects
                .iter()
                .map(|e| EffectPose {
                    kind: e.kind,
                    position: e.position,
                    ticks: e.ticks,
                })
                .collect(),
            debris: self
                .state
                .debris
                .iter()
                .map(|piece| DebrisPose {
                    owner: piece.owner,
                    draw: if piece.owner == 0 {
                        Draw::Ownship
                    } else {
                        draw(piece.owner)
                    },
                    position: piece.position,
                    attitude: piece.basis.angles(),
                    variant: if piece.owner == 0 {
                        player.damage_variant
                    } else {
                        self.state
                            .targets
                            .iter()
                            .find(|target| target.id == piece.owner)
                            .and_then(|target| target.localized_damage.structural_section)
                            .map(|section| section as usize)
                    },
                })
                .collect(),
            pilots: player
                .escape
                .iter()
                .map(|escape| pilot(0, escape))
                .chain(
                    wings
                        .into_iter()
                        .flat_map(|wings| wings.escapees())
                        .map(|(owner, escape)| pilot(owner, escape)),
                )
                .collect(),
            models: self.dummy_models.iter().map(|h| h.profile.id).collect(),
        }
    }
    /// The flame of every lit afterburner as a light source, each engine
    /// sharing its aircraft's strength, at this frame's presented poses
    /// (docs/spec/engine-material.md#afterburner-glow): the player's from
    /// its presented flight state, every other aircraft's from the presented
    /// snapshot, as a replay draws them.
    pub fn afterburner_glows(
        &self,
        player: &flight::State,
    ) -> Vec<crate::countermeasure_renderer::Afterburner> {
        let mut glows = Vec::new();
        if player.afterburner_active() && player.escape.is_none() && self.state.player_hp > 0 {
            glows.extend(crate::render_snapshot::afterburner_glow(
                player.position,
                [player.yaw, player.pitch, player.bank],
                &self.contrail_offsets,
            ));
        }
        let player_type = self.state.configuration().aircraft;
        // Only the lit aircraft, blended as the picture blends them, so the
        // rest of the picture is not built again for the lights.
        let lit = RenderSnapshot {
            targets: self
                .render
                .current
                .targets
                .iter()
                .filter(|pose| pose.engine.flame)
                .filter_map(|pose| self.presented_target(pose.id))
                .collect(),
            ..RenderSnapshot::default()
        };
        glows.extend(crate::render_snapshot::target_glows(&lit, |pose| {
            crate::render_snapshot::engine_outlets(
                pose.aircraft,
                player_type,
                &self.dummy_models,
                &self.dummy_contrail_offsets,
                &self.contrail_offsets,
            )
        }));
        glows
    }
    /// Starts a new render history from the current state: after a reset and
    /// once a mission's AI has placed its aircraft.
    pub fn restart_render(
        &mut self,
        player: &flight::State,
        wings: Option<&crate::ai_wings::AiWings>,
    ) {
        self.render = RenderHistory::default();
        let current = self.snapshot(player, wings);
        self.render.set_current(current);
    }
    /// Ends a simulation tick: the current snapshot becomes the previous one.
    pub fn advance_render(
        &mut self,
        player: &flight::State,
        wings: Option<&crate::ai_wings::AiWings>,
    ) {
        let next = self.snapshot(player, wings);
        self.render.advance(next);
    }
    /// Retakes the current snapshot after a command changed the scene between
    /// ticks, so the change shows at once as it always has.
    pub fn refresh_render(
        &mut self,
        player: &flight::State,
        wings: Option<&crate::ai_wings::AiWings>,
    ) {
        let current = self.snapshot(player, wings);
        self.render.set_current(current);
    }
    /// The latest tick's snapshot, uninterpolated.
    #[allow(dead_code)] // Read by the mission recorder.
    pub fn render_snapshot(&self) -> &RenderSnapshot {
        &self.render.current
    }
    /// The picture for this frame: the last two snapshots at the frame's tick fraction.
    pub fn presented(&self) -> RenderSnapshot {
        crate::render_snapshot::interpolate(
            self.render.previous.as_ref(),
            &self.render.current,
            self.render.alpha,
        )
    }
    fn presented_target(&self, id: u32) -> Option<AircraftPose> {
        self.render.presented_target(id)
    }
    /// Aircraft models loaded for other aircraft, in draw order.
    pub fn models(&self) -> &[Airframe] {
        &self.dummy_models
    }

    /// Per-model vertices for the dummy formation, with each airborne
    /// aircraft's vertex range for the spotting aid.
    pub fn dummy_geometry(
        &self,
        camera: &Camera,
        world: &World,
    ) -> Vec<(&Airframe, Vec<f32>, Vec<Contact>)> {
        crate::render_snapshot::aircraft_batches(
            &self.presented(),
            &self.dummy_models,
            camera,
            world,
        )
    }
    /// Combat geometry drawn with the ownship airframe over its presented state.
    pub fn vertices(
        &self,
        h: &Airframe,
        s: &flight::State,
        camera: &Camera,
        world: &World,
    ) -> crate::sim_renderer::CombatGeometry {
        crate::render_snapshot::combat_geometry(&self.presented(), &self.art, h, s, camera, world)
    }
    /// Populate all six creator wings, retaining their sides for placement.
    pub fn mission_aircraft(
        &mut self,
        wings: &[tore_sim::ai::launch::WingLaunch],
        layout: &crate::quick_mission::MissionLayout,
        data: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<()> {
        self.mission_dummies(
            &tore_sim::ai::launch::legacy_pairs(wings),
            layout.enemy.distance_ft,
            data,
        )?;
        self.mission_spawns = Some(crate::ai_wings::mission_spawns(wings, &layout.spawn_plan()));
        self.mission_layout = Some(layout.clone());
        Ok(())
    }

    pub fn mission_dummies(
        &mut self,
        wings: &[(tore_formats::aircraft::AircraftId, usize)],
        separation: f64,
        data: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<()> {
        for (id, count) in wings {
            if *count == 0 {
                continue;
            }
            let model = if let Some(i) = self.dummy_models.iter().position(|h| h.profile.id == *id)
            {
                i
            } else {
                let h = Airframe::load(data, *id)?;
                self.dummy_configs
                    .push(live::Configuration::from_source(&h.profile, |name| {
                        data.get(name)
                            .cloned()
                            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
                    })?);
                self.art
                    .add_weapon_shapes(self.dummy_configs.last().unwrap(), data);
                self.dummy_contrail_offsets.push(h.contrail_offsets());
                self.dummy_models.push(h);
                self.dummy_models.len() - 1
            };
            for _ in 0..*count {
                let n = self.dummies.len();
                // Fitted stagger: first contact straight ahead, successive pairs
                // 500 feet to either side, 500 feet deeper per pair.
                let side = if n == 0 {
                    0.
                } else if n % 2 == 1 {
                    1.
                } else {
                    -1.
                };
                let row = n.div_ceil(2) as f64;
                self.dummies
                    .push((model, [side * row * 500., 0., separation + row * 500.]));
            }
        }
        Ok(())
    }
    pub fn command(&mut self, command: live::Command, l: Launcher) {
        if let Some(r) = &mut self.recorder {
            r.record(&crate::combat_tape::command_name(command), l);
        }
        self.note(CommandNote::Command(command));
        self.state.command(command, l);
        self.last_launcher = Some(l);
    }
    fn note(&mut self, note: CommandNote) {
        if self.notes.len() == MAX_COMMAND_NOTES {
            self.notes.pop_front();
        }
        self.notes.push_back(note);
    }
    /// Player commands since the last call, oldest first. For mission
    /// recordings; flight never reads them.
    pub fn take_notes(&mut self) -> Vec<CommandNote> {
        self.notes.drain(..).collect()
    }
    pub fn finish_recording(&mut self) -> AppResult<()> {
        if let Some(mut r) = self.recorder.take() {
            r.flush()?;
        }
        Ok(())
    }
    pub fn cancel(&mut self) {
        if let (Some(r), Some(l)) = (&mut self.recorder, self.last_launcher) {
            r.record("release", l);
        }
        if self.input.held || self.controller.held {
            self.note(CommandNote::Release);
        }
        self.input.cancel();
        self.controller.cancel();
        self.state.release();
    }
    pub fn reset(&mut self, s: &mut flight::State) -> AppResult<()> {
        self.render = RenderHistory::default();
        self.contrails = Default::default();
        self.contrail_sortie = self.contrail_sortie.wrapping_add(1);
        let l = launcher(s);
        if let Some(r) = &mut self.recorder {
            r.record(if self.range { "reset" } else { "reset-scene" }, l);
        }
        self.last_launcher = Some(l);
        let weapon_rules = self.state.weapon_rules;
        self.state = live::State::new(
            self.state.configuration().clone(),
            s.native.is_none() && !self.clean_recording,
        )?;
        self.state.weapon_rules = weapon_rules;
        if weapon_rules == tore_sim::combat::missiles::Rules::Compatibility
            && let Some(r) = &mut self.recorder
        {
            r.record("compatibility-weapons", l);
        }
        if let Some(ammo) = &self.initial_ammo {
            self.state.ammo.clone_from(ammo);
        }
        self.input = FireInput::default();
        self.controller.cancel();
        s.set_payload(self.state.payload_lbs())?;
        s.systems = tore_sim::aircraft_systems::Systems::new(
            self.state.configuration().engines,
            self.state.external_fuel_lbs(),
        );
        s.damage_fraction = 0.;
        s.damage_variant = None;
        s.damage_regions = [0.; live::DAMAGE_SECTIONS];
        s.bay = 0.;
        s.bay_open = false;
        s.bay_auto_open = false;
        if self.range {
            self.state.range_target(launcher(s));
        }
        for (index, (model, offset)) in self.dummies.iter().enumerate() {
            let fixture_position = std::array::from_fn(|i| {
                l.position[i] + l.basis.right[i] * offset[0] + l.basis.forward[i] * offset[2]
            });
            let (position, basis) = self
                .mission_spawns
                .as_ref()
                .map(|spawns| spawns[index].pose(l.position, l.basis))
                .unwrap_or((fixture_position, l.basis));
            self.state
                .add_dummy(&self.dummy_configs[*model], position, basis);
            debug_assert_eq!(self.state.targets.last().unwrap().id as usize, index + 1);
        }
        // Aircraft are spawned first, preserving their roster ordering.
        for object in &self.airport_objects {
            Self::register_airport_object(&mut self.state, object)?;
        }
        self.restart_render(s, None);
        Ok(())
    }
    pub fn step(&mut self, s: &mut flight::State, world: &World) -> AppResult<Vec<Event>> {
        let l = launcher(s);
        self.last_launcher = Some(l);
        if let Some(r) = &mut self.recorder {
            r.record(
                if self.input.held || self.controller.held {
                    "fire"
                } else {
                    "tick"
                },
                l,
            );
        }
        let mut events = Vec::new();
        if s.airburst()
            && let Some(event) = self.state.player_airburst(s.position)
        {
            events.push(event);
        }
        if s.ground_impact()
            && let Some(event) = self.state.player_ground_impact(s.position)
        {
            events.push(event);
        }
        // Stop wreck emissions before advancing smoke on the impact/airburst tick.
        events.extend(
            self.state
                .step(self.input.held || self.controller.held, l, |x, z| {
                    f64::from(world.height(x as f32, z as f32))
                }),
        );
        for index in 0..45 {
            while s.systems.counts[index] < self.state.subsystem_counts[index] {
                s.systems.hit(index, s.throttle);
                if let Some(hardpoint) = index.checked_sub(36) {
                    let config = self.state.configuration();
                    if config.external_fuel_lbs[hardpoint] > 0. {
                        s.systems.fuel.external[hardpoint] = 0.;
                        s.systems.notify(format!(
                            "External fuel tank {} damaged: fuel lost",
                            hardpoint + 1
                        ));
                    } else if let Some(Some(slot)) = config.hardpoint_slots.get(hardpoint) {
                        s.systems.notify(format!(
                            "{} station damaged",
                            config.stations[*slot].weapon.hud_name
                        ));
                    } else {
                        s.systems.notify(
                            if self.state.radar_failed && hardpoint == config.radar_hardpoint {
                                "Radar failed"
                            } else if self.state.visual_failed
                                && hardpoint == config.visual_hardpoint
                            {
                                "Visual sensor failed"
                            } else if self.state.infrared_failed
                                && Some(hardpoint) == config.infrared_hardpoint
                            {
                                "Infrared sensor failed"
                            } else if Some(hardpoint) == config.rwr_hardpoint {
                                "RWR failed"
                            } else if hardpoint == config.ecm_hardpoint {
                                "Countermeasure equipment damaged"
                            } else {
                                "Hardpoint equipment damaged"
                            },
                        );
                    }
                }
            }
        }
        if s.systems.fatal() {
            s.crashed = true;
        }
        if s.crashed
            && let Some(event) = self.state.systems_destroyed()
        {
            events.push(event);
        }
        use tore_sim::combat::smoke::contrail_altitude_ft;
        let mut outlets = Vec::new();
        let mut add = |id: u32, position: Vector, basis: Basis, offsets: &[Vector]| {
            if position[1] < contrail_altitude_ft(self.contrail_sortie, id) {
                return;
            }
            for (engine, offset) in offsets.iter().enumerate() {
                let point = std::array::from_fn(|i| {
                    position[i]
                        + basis.right[i] * offset[0]
                        + basis.up[i] * offset[1]
                        + basis.forward[i] * offset[2]
                });
                outlets.push((u64::from(id) * 2 + engine as u64, point));
            }
        };
        let height = f64::from(world.height(s.position[0] as f32, s.position[2] as f32));
        if !s.crashed
            && s.engine
            && s.fuel > 0.
            && self.state.player_hp > 0
            && !s.supported_at(height)
        {
            add(0, s.position, l.basis, &self.contrail_offsets);
        }
        for target in self.state.targets.iter().filter(|t| t.airborne && t.hp > 0) {
            if let Some(id) = target.aircraft {
                if id == self.state.configuration().aircraft {
                    add(
                        target.id,
                        target.position,
                        target.basis,
                        &self.contrail_offsets,
                    );
                } else if let Some(index) =
                    self.dummy_models.iter().position(|h| h.profile.id == id)
                {
                    add(
                        target.id,
                        target.position,
                        target.basis,
                        &self.dummy_contrail_offsets[index],
                    );
                }
            }
        }
        self.contrails.step([]);
        self.contrails.contrails(outlets);
        s.set_payload((self.state.payload_lbs() - s.systems.used_external_lbs()).max(0.))?;
        s.bay_auto_open = s.bay_available()
            && self.state.armed
            && (self.state.designated().is_some()
                || self.state.launch_mode == tore_sim::combat::missiles::LaunchMode::Boresight)
            && self.state.rounds(self.state.selected) > 0
            && self.state.configuration().stations[self.state.selected]
                .weapon
                .seeker
                .signature
                != 0;
        if self.state.radar_failed {
            s.radar = false;
        }
        if self.state.ecm_failed {
            s.jammer = false;
        }
        s.damage_fraction = (1.
            - f64::from(self.state.player_hp)
                / f64::from(self.state.configuration().damage_capacity))
        .clamp(0., 1.);
        if events.iter().any(|e| matches!(e, Event::PlayerDamaged(_))) {
            s.systems.report_impact(s.ticks, s.damage_fraction);
        }
        s.damage_variant = self
            .state
            .player_damage_section()
            .map(|section| section as usize);
        s.damage_regions = self.state.player_damage_regions();
        if self.state.player_hp == 0 {
            s.crashed = true;
            if matches!(
                self.state.player_damage_section(),
                Some(live::DamageSection::Nose | live::DamageSection::Cockpit)
            ) {
                s.systems.kill_pilot("Pilot killed: nose or cockpit lost");
            }
        }
        if events.contains(&Event::PilotKilled) {
            s.systems.kill_pilot("Pilot killed by cockpit hit");
        }
        Ok(events)
    }
    pub fn view_pose(&self, target: &live::Target, presented: bool) -> ([f64; 3], [f64; 3]) {
        if presented {
            self.presented_pose(target)
        } else {
            (target.position, target.basis.angles())
        }
    }
    /// Where a target is drawn this frame; one missing from the snapshots is
    /// drawn where it is.
    fn presented_pose(&self, target: &live::Target) -> ([f64; 3], [f64; 3]) {
        self.presented_target(target.id).map_or(
            (target.position, target_pose(target, self.ai_poses)),
            |pose| (pose.position, pose.attitude),
        )
    }

    pub fn target_camera(&self, player: &flight::State) -> Option<Camera> {
        let target = self.state.display_target()?;
        let (position, _) = self.presented_pose(target);
        Some(crate::target_window::camera(player.position, position))
    }

    pub fn framed_target_camera(
        &self,
        player: &flight::State,
        aircraft: &Airframe,
        world: &World,
    ) -> Option<Camera> {
        let target = self.state.display_target()?;
        let mut camera = self.target_camera(player)?;
        if let Some(object) = self.airport_objects.iter().find(|o| o.id == target.id) {
            let bounds = object.bounds;
            let basis = Basis::new(bounds.heading, bounds.pitch, bounds.bank);
            let corners = (0..8).map(|corner| {
                let local: [f64; 3] = std::array::from_fn(|i| {
                    bounds.half[i] * if corner & (1 << i) == 0 { -1. } else { 1. }
                });
                std::array::from_fn(|i| {
                    bounds.center[i]
                        + basis.right[i] * local[0]
                        + basis.up[i] * local[1]
                        + basis.forward[i] * local[2]
                })
            });
            crate::target_window::fit(&mut camera, corners);
        } else {
            let model = self
                .dummy_models
                .iter()
                .find(|h| Some(h.profile.id) == target.aircraft)
                .unwrap_or(aircraft);
            let mut pose = player.clone();
            pose.wreck = target.wreck.clone();
            pose.crashed = target.hp <= 0;
            let presented = self.presented_target(target.id);
            let (position, angles) = presented
                .as_ref()
                .map_or((target.position, target_pose(target, self.ai_poses)), |p| {
                    (p.position, p.attitude)
                });
            pose.position = position;
            [pose.yaw, pose.pitch, pose.bank] = angles;
            pose.damage_fraction = target.damage_fraction();
            pose.damage_variant = target
                .localized_damage
                .structural_section
                .map(|s| s as usize);
            pose.damage_regions = target.localized_damage.fractions(target.initial_hp);
            pose.gear = 0.;
            pose.flaps = 0.;
            pose.exhaust = 0.;
            pose.bay = 0.;
            pose.elevator = 0.;
            pose.aileron = 0.;
            pose.rudder = 0.;
            pose.brake = 0.;
            pose.hook = 0.;
            if let Some(devices) = presented.and_then(|p| p.devices) {
                crate::render_snapshot::set_devices(&mut pose, devices);
            }
            let vertices = model.vertices(&pose, &camera, world);
            crate::target_window::fit(
                &mut camera,
                vertices
                    .chunks_exact(10)
                    .map(|v| [f64::from(v[0]), f64::from(v[1]), f64::from(v[2])]),
            );
        }
        Some(camera)
    }

    pub fn readout(&self, s: &flight::State, rcs_scale: f64) -> crate::instruments::CombatReadout {
        let mut groups: Vec<(String, String, u32, bool)> = Vec::new();
        for (index, station) in self.state.configuration().stations.iter().enumerate() {
            let selected = self.state.armed && index == self.state.selected;
            if let Some(group) = groups.iter_mut().find(|g| g.0 == station.weapon.source) {
                group.2 += u32::from(self.state.rounds(index));
                group.3 |= selected;
            } else {
                groups.push((
                    station.weapon.source.clone(),
                    station.weapon.hud_name.clone(),
                    u32::from(self.state.rounds(index)),
                    selected,
                ));
            }
        }
        crate::instruments::CombatReadout {
            weapons: groups
                .into_iter()
                .map(|(_, name, count, selected)| (name, count, selected))
                .collect(),
            chaff: self.state.chaff,
            flares: self.state.flares,
            target: self.state.display_target().map(|target| {
                let name = self
                    .ground_name(target.id)
                    .map(str::to_owned)
                    .or_else(|| target.aircraft.map(|id| id.label().to_owned()))
                    .unwrap_or_else(|| format!("CONTACT {}", target.id));
                crate::target_window::Readout::new(target, s, name)
            }),
            envelope_target: self.state.display_target().and_then(|target| {
                self.dummy_models
                    .iter()
                    .find(|h| Some(h.profile.id) == target.aircraft)
                    .map(|h| h.profile.envelopes.clone())
            }),
            scope: crate::scope::scope(&self.state, s),
            rcs: crate::scope::rcs(&self.state, s, rcs_scale),
            rwr_failed: self.state.rwr_failed,
            rwr: self.rwr_readout(s),
        }
    }
    fn rwr_readout(&self, own: &flight::State) -> crate::scope::Rwr {
        use crate::scope::{EmitterKind, EmitterState, Indicator, Rwr, RwrEmitter, RwrMissile};
        use tore_sim::combat::threats::GuidanceClass;
        let operating = !self.state.rwr_failed && own.systems.counts[32] <= 1;
        let emitters = if operating {
            self.state
                .emitters
                .iter()
                .map(|emitter| RwrEmitter {
                    id: emitter.id,
                    bearing_rad: emitter.bearing_rad,
                    distance_nmi: emitter.distance_nmi,
                    // Allegiance is supplied by the mission bridge only for known
                    // aircraft. Passive reception alone remains unidentified.
                    kind: match emitter.symbol {
                        tore_sim::sensors::passive::Symbol::Ground => EmitterKind::Ground,
                        tore_sim::sensors::passive::Symbol::Aircraft => EmitterKind::EnemyAircraft,
                        tore_sim::sensors::passive::Symbol::Unknown => EmitterKind::Unknown,
                    },
                    state: EmitterState::Detected,
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut radar_indicator = if self.state.emitters.is_empty() || !operating {
            Indicator::Off
        } else {
            Indicator::Detected
        };
        let mut infrared_indicator = Indicator::Off;
        let missiles = self
            .state
            .missile_threats
            .records()
            .map(|record| {
                if !record.stale && record.targeting_receiver {
                    match record.guidance_class {
                        Some(GuidanceClass::Radar) => radar_indicator = Indicator::Incoming,
                        Some(GuidanceClass::Infrared) => infrared_indicator = Indicator::Incoming,
                        _ => {}
                    }
                }
                RwrMissile {
                    id: record.missile_id,
                    bearing_rad: record.bearing_deg.to_radians(),
                    distance_nmi: record.position.map(|position| {
                        position
                            .iter()
                            .zip(own.position)
                            .map(|(a, b)| (a - b).powi(2))
                            .sum::<f64>()
                            .sqrt()
                            / tore_sim::sensors::FEET_PER_NAUTICAL_MILE
                    }),
                    known_targeting_receiver: record.targeting_receiver,
                    stale: record.stale,
                }
            })
            .collect();
        let mut readout = Rwr {
            tick: self.state.sensors.tick(),
            operating,
            emitters,
            missiles,
            radar_indicator,
            infrared_indicator,
        };
        readout.mark_supported_sources(
            self.state
                .missile_threats
                .records()
                .filter(|r| {
                    !r.stale
                        && r.targeting_receiver
                        && r.source
                            == tore_sim::combat::threats::EvidenceSource::ElectronicSupported
                })
                .filter_map(|r| r.radar_bearing_deg),
        );
        readout
    }

    pub fn equipment_damage_report(&self) -> Vec<String> {
        let config = self.state.configuration();
        (36..45)
            .filter(|i| self.state.subsystem_counts[*i] > 0)
            .map(|i| {
                let hardpoint = i - 36;
                if config.external_fuel_lbs[hardpoint] > 0. {
                    format!("External tank {} damaged", hardpoint + 1)
                } else if let Some(Some(slot)) = config.hardpoint_slots.get(hardpoint) {
                    format!("{} station failed", config.stations[*slot].weapon.hud_name)
                } else if hardpoint == config.radar_hardpoint {
                    "Radar failed".into()
                } else if hardpoint == config.visual_hardpoint {
                    "Visual sensor failed".into()
                } else if Some(hardpoint) == config.infrared_hardpoint {
                    "Infrared sensor failed".into()
                } else if Some(hardpoint) == config.rwr_hardpoint {
                    "RWR failed".into()
                } else if hardpoint == config.ecm_hardpoint {
                    format!(
                        "Countermeasures: jammer {}, chaff {}, flares {}",
                        if self.state.ecm_failed {
                            "failed"
                        } else {
                            "available"
                        },
                        self.state.chaff,
                        self.state.flares
                    )
                } else {
                    format!("Hardpoint {} equipment damaged", hardpoint + 1)
                }
            })
            .collect()
    }
    pub fn status(&self, s: &flight::State) -> String {
        let i = self.state.selected;
        let target = self.state.designated().map_or("NO TARGET".into(), |id| {
            self.state
                .targets
                .iter()
                .find(|t| t.id == id)
                .map_or("NO TARGET".into(), |t| {
                    if t.hp == 0 {
                        "DESTROYED".into()
                    } else {
                        format!(
                            "{} HP {} {}",
                            self.ground_name(id)
                                .map_or_else(|| format!("T{id}"), str::to_owned),
                            t.hp,
                            if self.state.configuration().stations[i]
                                .weapon
                                .seeker
                                .signature
                                == 0
                            {
                                "VISUAL"
                            } else if self.state.can_lock(launcher(s)) {
                                "LOCK"
                            } else {
                                "NO LOCK"
                            }
                        )
                    }
                })
        });
        let scope = crate::scope::scope(&self.state, s);
        format!(
            "{} {} {}  {} C{} HIT {} | HP {} SYS {} ECM {} T-JAM {} IN {} | {} {} {:.0}NM {} CONTACTS{}{}",
            self.state.configuration().stations[i].weapon.name,
            self.state.rounds(i),
            self.state.readiness(launcher(s)).label(),
            target,
            live::damage_class(self.state.range_category),
            self.state.history.last().map_or(0, |hit| hit.applied),
            self.state.player_hp,
            self.state
                .last_subsystem
                .map_or("--".into(), |i| i.to_string()),
            if self.state.ecm_failed {
                "FAIL"
            } else if launcher(s).jammer {
                "ON"
            } else {
                "OFF"
            },
            if self.state.target_jammer {
                "ON"
            } else {
                "OFF"
            },
            self.state.projectiles.iter().filter(|p| p.incoming).count(),
            scope.channel,
            scope.mode.unwrap_or("OFF"),
            scope.range_nmi,
            scope.contacts.iter().filter(|c| !c.stale).count(),
            if scope.history { " HIST" } else { "" },
            scope
                .status
                .map(|status| format!(" {status}"))
                .unwrap_or_default()
        )
    }
}

pub(crate) fn apply_startup_weapon_state(state: &mut live::State) {
    if let Some(index) = state
        .configuration()
        .stations
        .iter()
        .position(|station| live::is_gun(&station.weapon))
    {
        state.selected = index;
    }
    state.armed = true;
}

/// Uses the same imported configuration, flight state, trigger host, movement and
/// hit/effect path as desktop flight. Explicit scripted range, not a retail replay.
/// Haptics follow confirmed ownship events. A distant target explosion is not
/// player damage; incoming fixture launches must not feel like own launches.
pub fn feedback(event: &Event, config: &live::Configuration) -> Option<tore_input::FeedbackEvent> {
    use tore_input::FeedbackEvent as F;
    match event {
        Event::Fired(i) => Some(if config.stations[*i].internal {
            F::GunFired
        } else {
            F::MissileLaunched
        }),
        Event::PlayerDamaged(_) => Some(F::Damage),
        Event::PlayerDestroyed => Some(F::Crash),
        _ => None,
    }
}
pub fn smoke(h: &Airframe, data: &BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    let world = World::for_theater(data, "UKR")?;
    let mut combat = Combat::new(h, data, true)?;
    println!(
        "systems source {:?}: player capacity={} ECM={:?} weights={} repeat-limited=45",
        h.profile.id,
        combat.state.configuration().damage_capacity,
        combat.state.configuration().ecm,
        combat
            .state
            .configuration()
            .system_damage
            .iter()
            .map(|v| u32::from(v & 15))
            .sum::<u32>()
    );
    let mut damaged = live::State::new(combat.state.configuration().clone(), true)?;
    let l = launcher(&h.start(&world));
    // A source missile followed by gun hits exercises selection on a varied
    // damage history; a particular all-gun seed can legitimately select no fault.
    damaged.selected = damaged
        .configuration()
        .stations
        .iter()
        .position(|s| s.weapon.source == "AGM65G.JT")
        .or_else(|| {
            damaged
                .configuration()
                .stations
                .iter()
                .position(|s| s.weapon.seeker.signature != 0)
        })
        .unwrap_or(0);
    damaged.command(live::Command::Incoming, l);
    let mut replica = damaged.clone();
    let mut systems = 0;
    let mut destroyed = 0;
    for _ in 0..1200 {
        let events = damaged.step(false, l, |_, _| 0.);
        if events != replica.step(false, l, |_, _| 0.) {
            return Err("source incoming damage replay diverged".into());
        }
        systems += events
            .iter()
            .filter(|e| matches!(e, Event::SubsystemDamaged(_)))
            .count();
        destroyed += events
            .iter()
            .filter(|e| matches!(e, Event::PlayerDestroyed))
            .count();
        if damaged.projectiles.is_empty() {
            break;
        }
    }
    for _ in 0..40 {
        damaged.command(live::Command::DamagePlayer, l);
        replica.command(live::Command::DamagePlayer, l);
        let events = damaged.step(false, l, |_, _| 0.);
        if events != replica.step(false, l, |_, _| 0.)
            || format!("{damaged:?}") != format!("{replica:?}")
        {
            return Err("source damage replay diverged".into());
        }
        systems += events
            .iter()
            .filter(|e| matches!(e, Event::SubsystemDamaged(_)))
            .count();
        destroyed += events
            .iter()
            .filter(|e| matches!(e, Event::PlayerDestroyed))
            .count();
    }
    // Different source weapons produce different damage histories. A fatal
    // missile can legitimately select no subsystem. Cover each source station
    // plus gradual gun damage without changing aircraft damage values or RNG.
    for source in 0..combat.state.configuration().stations.len() {
        if systems > 0 {
            break;
        }
        let mut gradual = live::State::new(combat.state.configuration().clone(), true)?;
        gradual.selected = source;
        if source > 0 {
            gradual.command(live::Command::Incoming, l);
        }
        let mut replay = gradual.clone();
        for tick in 0..1240 {
            if tick >= 1200 {
                gradual.command(live::Command::DamagePlayer, l);
                replay.command(live::Command::DamagePlayer, l);
            }
            let events = gradual.step(false, l, |_, _| 0.);
            if events != replay.step(false, l, |_, _| 0.)
                || format!("{gradual:?}") != format!("{replay:?}")
            {
                return Err("gradual source damage replay diverged".into());
            }
            systems += events
                .iter()
                .filter(|e| matches!(e, Event::SubsystemDamaged(_)))
                .count();
        }
    }
    if (systems == 0
        && damaged
            .configuration()
            .stations
            .iter()
            .any(|s| s.weapon.seeker.signature != 0))
        || destroyed != 1
        || damaged.player_hp != 0
    {
        return Err(format!("source automatic damage/destruction failed faults={systems} kills={destroyed} HP={} damage={} counts={:?} source={:?}",damaged.player_hp,damaged.player_damage,damaged.subsystem_counts,damaged.configuration().system_damage).into());
    }
    println!(
        "systems automatic {:?}: faults={systems} destruction={destroyed} indices={:?} PASS",
        h.profile.id, damaged.subsystem_counts
    );
    for index in 0..combat.state.ammo.len() {
        let station = &combat.state.configuration().stations[index];
        if !station.internal && station.weapon.seeker.signature == 0 {
            continue;
        }
        for jammer in [false, true] {
            let mut state = live::State::new(combat.state.configuration().clone(), true)?;
            state.selected = index;
            let mut l = launcher(&h.start(&world));
            l.jammer = jammer;
            state.command(live::Command::Incoming, l);
            let mut replay = state.clone();
            let initial = state.player_hp;
            let ammo = state.ammo.clone();
            let mut outcome = false;
            let mut mixer = tore_input::FeedbackMixer::default();
            let mut pulses = 0;
            for _ in 0..1200 {
                let events = state.step(false, l, |_, _| 0.);
                if events != replay.step(false, l, |_, _| 0.)
                    || format!("{state:?}") != format!("{replay:?}")
                {
                    return Err("incoming replay diverged".into());
                }
                for event in &events {
                    if let Some(cue) = feedback(event, state.configuration()) {
                        mixer.event(cue);
                    }
                    outcome |= matches!(event, Event::PlayerDamaged(_) | Event::Defeated(0));
                }
                if matches!(mixer.tick(), Some(tore_input::FeedbackUpdate::Pulse { .. })) {
                    pulses += 1;
                }
                if outcome {
                    break;
                }
            }
            if !outcome || state.ammo != ammo || (state.player_hp < initial && pulses == 0) {
                return Err(format!(
                    "incoming lifecycle failed slot {} jammer={jammer}",
                    index + 1
                )
                .into());
            }
            for _ in 0..120 {
                mixer.tick();
            }
            if mixer.tick().is_some() {
                return Err("feedback failed to settle".into());
            }
            println!(
                "systems incoming {:?} slot={} jammer={jammer} HP={initial}->{} haptic-pulses={pulses} PASS",
                h.profile.id,
                index + 1,
                state.player_hp
            );
        }
    }
    for index in 0..combat.state.ammo.len() {
        let station = &combat.state.configuration().stations[index];
        if !station.internal && station.weapon.seeker.signature == 0 {
            ballistic_smoke(combat.state.configuration(), index)?;
            continue;
        }
        for (class, category) in [
            combat.state.configuration().target_category,
            0x2000,
            0x100,
            0x400,
            0x40,
        ]
        .into_iter()
        .enumerate()
        {
            let mut flight = h.start(&world);
            let tape = std::env::var_os("TORE_COMBAT_EVIDENCE")
                .filter(|_| class == 0)
                .map(|root| {
                    std::path::PathBuf::from(root).join(format!(
                        "{:?}-slot-{}.tape",
                        h.profile.id,
                        index + 1
                    ))
                });
            if let Some(path) = &tape {
                std::fs::create_dir_all(path.parent().ok_or("tape directory missing")?)?;
                combat.recorder = Some(crate::combat_tape::Recorder::new(
                    path,
                    data,
                    combat.state.configuration(),
                    "UKR",
                )?);
            }
            combat.reset(&mut flight)?;
            // This stationary range fixture starts with an open available bay.
            // Live actuator delay and closed-bay release have separate tests.
            if flight.bay_available() {
                flight.bay = 1.;
                flight.bay_open = true;
            }
            for _ in 0..index {
                combat.command(live::Command::NextWeapon, launcher(&flight));
            }
            combat.state.range_category = category;
            combat.command(live::Command::ReplaceTarget, launcher(&flight));
            // Selection needs a current observation, and a radar weapon track
            // needs half a second of it, so observe before designating.
            observe(&mut combat, &mut flight, &world, 1)?;
            combat.command(live::Command::Designate, launcher(&flight));
            observe(&mut combat, &mut flight, &world, ACQUISITION)?;
            let initial = combat.state.ammo[index];
            let mut negative = combat.state.clone();
            let l = launcher(&flight);
            negative.command(live::Command::ToggleArm, l);
            negative.step(true, l, |_, _| 0.);
            if negative.ammo[index] != initial || negative.readiness(l) != live::Readiness::Safe {
                return Err("safe inhibited shot consumed ammunition".into());
            }
            negative.command(live::Command::ToggleArm, l);
            negative.command(live::Command::FailStation, l);
            let mass = negative.payload_lbs();
            negative.step(true, l, |_, _| 0.);
            if negative.rounds(index) != initial || negative.payload_lbs() != mass {
                return Err("station failure changed ammunition/mass".into());
            }
            if index != 0 {
                let mut no_target = combat.state.clone();
                // Remove the fixture contact as well as its designation. A
                // bare ClearDesignation may reacquire the same aircraft in
                // boresight during this step, which is a valid launch rather
                // than an undesignated negative case.
                no_target.command(live::Command::ClearRange, l);
                no_target.step(true, l, |_, _| 0.);
                // Seeker service may validly switch a supported weapon into
                // boresight during the step. The captured release gate, not the
                // stale pre-service readiness, decides whether any debit was legal.
                if no_target.release_readiness != live::Readiness::Ready
                    && no_target.ammo[index] != initial
                {
                    return Err("inhibited targetless launch consumed ammo".into());
                }
                let weapon = &combat.state.configuration().stations[index].weapon;
                let mut too_close = combat.state.clone();
                too_close.launch_mode = tore_sim::combat::missiles::LaunchMode::Cued;
                if weapon.seeker.zones[1].minimum_range > 0
                    && combat.state.readiness(l) == live::Readiness::Ready
                {
                    // Leave enough margin for the 300 ft/s range target to
                    // advance during the observation refresh below.
                    let distance = f64::from(weapon.seeker.zones[1].minimum_range) - 100.;
                    too_close.targets[0].position =
                        std::array::from_fn(|k| l.position[k] + l.basis.forward[k] * distance);
                    too_close.step(false, l, |_, _| 0.);
                    let close_reason = too_close.readiness(l);
                    if close_reason != live::Readiness::MinimumRange {
                        return Err(format!(
                            "source minimum-range launch was not inhibited: slot={} minimum={} reason={close_reason:?} mode={:?} designated={:?}",
                            index + 1,
                            weapon.seeker.zones[1].minimum_range,
                            too_close.launch_mode,
                            too_close.designated()
                        ).into());
                    }
                    too_close.step(true, l, |_, _| 0.);
                    if too_close.rounds(index) != initial {
                        return Err("minimum-range inhibited shot consumed ammunition".into());
                    }
                }
                let mut tracking = combat.state.clone();
                tracking.step(true, l, |_, _| 0.);
                if tracking.projectiles.is_empty() {
                    return Err("source guidance probe did not launch".into());
                }
                tracking.step(false, Launcher { radar: false, ..l }, |_, _| 0.);
                let loses_track = weapon.seeker.signature == 3 && weapon.flags & 0x200 != 0;
                if tracking.projectiles.iter().any(|p| {
                    if let Some(guidance) = &p.guidance {
                        p.target.is_none()
                            || (loses_track
                                && !matches!(
                                    guidance.seeker.status,
                                    tore_sim::combat::missiles::seeker::Status::Memory
                                        | tore_sim::combat::missiles::seeker::Status::Lost
                                ))
                    } else {
                        p.target.is_none() != loses_track
                    }
                }) {
                    return Err("radar-off support and retained-identity contract failed".into());
                }
                let mut jettison = combat.state.clone();
                let internal = jettison.configuration().stations[index].internal;
                let expected_rounds = if internal { initial } else { 0 };
                let expected_mass = jettison.payload_lbs()
                    - if internal {
                        0.
                    } else {
                        f64::from(weapon.weight.max(0)) * f64::from(initial)
                    };
                jettison.command(live::Command::Jettison, l);
                if jettison.rounds(index) != expected_rounds
                    || jettison.payload_lbs() != expected_mass
                {
                    return Err("source jettison mass/ammunition contract failed".into());
                }
                if combat.state.configuration().stations[index]
                    .weapon
                    .seeker
                    .signature
                    == 3
                {
                    let mut radar_off = combat.state.clone();
                    let off = Launcher { radar: false, ..l };
                    radar_off.step(true, off, |_, _| 0.);
                    if radar_off.ammo[index] != initial
                        || radar_off.readiness(off) != live::Readiness::RadarOff
                    {
                        return Err("radar-off launch was not inhibited".into());
                    }
                }
            }
            // Replay the same authoritative host tick inputs in a second state.
            // Presentation/pause never calls this path and cannot advance either copy.
            let mut replay = combat.state.clone();
            if let Some(name) = combat.state.configuration().stations[index]
                .weapon
                .fire_sound
                .as_deref()
            {
                let pcm = tore_formats::pcm::Pcm::parse(
                    name,
                    data.get(name).ok_or("missing firing PCM")?,
                )?;
                if !pcm.samples.windows(2).any(|s| s[0] != s[1]) {
                    return Err("firing PCM contains no signal".into());
                }
            }
            for name in ["&EXPL3.5K", "&EXPL12.5K"] {
                tore_formats::pcm::Pcm::parse(name, data.get(name).ok_or("missing impact PCM")?)?;
            }

            let mut fired = 0;
            let mut impacts = 0;
            let mut destroyed = 0;
            // Pulse for missiles, hold for gun. Two shots are available in the
            // smallest source station; damage remains source class-0 per hit.
            for tick in 0..6000 {
                if tick % 240 == 0 {
                    combat.input.space(true, false, false);
                }
                if index != 0 && tick % 240 == 1 {
                    combat.input.space(false, false, false);
                }
                flight.step(&flight::PilotInput::default(), |x, z| {
                    f64::from(world.height(x as f32, z as f32))
                });
                let replay_events = replay.step(combat.input.held, launcher(&flight), |x, z| {
                    f64::from(world.height(x as f32, z as f32))
                });
                let events = combat.step(&mut flight, &world)?;
                if events != replay_events
                    || combat.state.ammo != replay.ammo
                    || combat.state.projectiles != replay.projectiles
                    || combat.state.targets != replay.targets
                    || combat.state.history != replay.history
                    || combat.state.effects != replay.effects
                {
                    return Err("combat host replay diverged".into());
                }
                for event in events {
                    match event {
                        Event::Fired(_) => fired += 1,
                        Event::Hit(_) => impacts += 1,
                        Event::Destroyed(_) => destroyed += 1,
                        _ => {}
                    }
                }
                if destroyed > 0 || (class != 0 && impacts > 0) {
                    break;
                }
            }
            if fired == 0
                || impacts == 0
                || (class == 0 && destroyed != 1)
                || combat.state.ammo[index] >= initial
                || combat.state.history.iter().any(|hit| {
                    hit.class != class
                        || hit.nominal
                            != i32::from(
                                combat.state.configuration().stations[index]
                                    .weapon
                                    .damage
                                    .by_class[class],
                            )
                            .max(0)
                })
                || combat
                    .state
                    .history
                    .iter()
                    .map(|hit| hit.applied)
                    .sum::<i32>()
                    != combat.state.configuration().hit_points - combat.state.targets[0].hp
                || (class == 0
                    && !combat
                        .state
                        .effects
                        .iter()
                        .any(|e| e.kind == EffectKind::Destroyed))
            {
                return Err(format!(
                    "combat smoke {} {} failed: fired={fired} hits={impacts} destroyed={destroyed}",
                    h.profile.name,
                    combat.state.configuration().stations[index].weapon.source
                )
                .into());
            }
            println!(
                "combat smoke {} slot={} {} class={class}: shots={fired} hits={impacts} destroyed={destroyed} ammo={}->{} effects={} PASS",
                h.profile.name,
                index + 1,
                combat.state.configuration().stations[index].weapon.source,
                initial,
                combat.state.ammo[index],
                combat.state.effects.len()
            );
            combat.cancel();
            let ammo = combat.state.ammo.clone();
            for _ in 0..120 {
                combat.step(&mut flight, &world)?;
            }
            if combat.state.ammo != ammo {
                return Err("firing continued after release".into());
            }
            if let Some(path) = &tape {
                combat
                    .recorder
                    .as_mut()
                    .ok_or("missing smoke recorder")?
                    .flush()?;
                let decoded = crate::combat_tape::replay(
                    path,
                    data,
                    combat.state.configuration().clone(),
                    "UKR",
                    &world,
                )?;
                if format!("{decoded:?}") != format!("{:?}", combat.state) {
                    return Err("serialized live-fire replay diverged before reset".into());
                }
                // Also replay manual state transitions, including a full reset.
                for command in [
                    live::Command::ToggleArm,
                    live::Command::ClearDesignation,
                    live::Command::FailStation,
                    live::Command::Jettison,
                    live::Command::NextWeapon,
                    live::Command::CycleClass,
                    live::Command::DamagePlayer,
                    live::Command::ToggleTargetJammer,
                    live::Command::Incoming,
                    live::Command::ReleaseChaff,
                    live::Command::ReleaseFlare,
                ] {
                    combat.cancel();
                    combat.command(command, launcher(&flight));
                    combat.step(&mut flight, &world)?;
                }
                combat
                    .recorder
                    .as_mut()
                    .ok_or("missing smoke recorder")?
                    .flush()?;
                let decoded = crate::combat_tape::replay(
                    path,
                    data,
                    combat.state.configuration().clone(),
                    "UKR",
                    &world,
                )?;
                if format!("{decoded:?}") != format!("{:?}", combat.state) {
                    return Err("serialized manual-command replay diverged before reset".into());
                }
                combat.reset(&mut flight)?;
                combat.step(&mut flight, &world)?;
                combat.finish_recording()?;
                let decoded = crate::combat_tape::replay(
                    path,
                    data,
                    combat.state.configuration().clone(),
                    "UKR",
                    &world,
                )?;
                if format!("{decoded:?}") != format!("{:?}", combat.state) {
                    return Err("serialized combat replay diverged after commands/reset".into());
                }
                println!("serialized combat replay {} PASS", path.display());
            }
        }
    }
    Ok(())
}

const ACQUISITION: usize = tore_sim::sensors::track::ACQUISITION_STEPS as usize;

/// Advance the shared sensors without firing, so a scripted probe designates
/// and tracks the same way a player does. It uses the recorded host step, so a
/// tape replays the same observations.
fn observe(
    combat: &mut Combat,
    flight: &mut flight::State,
    world: &World,
    steps: usize,
) -> AppResult<()> {
    for _ in 0..steps {
        combat.step(flight, world)?;
    }
    Ok(())
}

/// Unguided external stores have release/contact checks, not a missile lock or
/// same-altitude interception requirement. Does not claim blast-radius parity.
fn ballistic_smoke(config: &live::Configuration, index: usize) -> AppResult<()> {
    let mut state = live::State::new(config.clone(), true)?;
    state.selected = index;
    let l = Launcher {
        position: [0., 500., 0.],
        basis: Basis::new(0., -0.3, 0.),
        speed_fps: 500.,
        velocity: Basis::new(0., -0.3, 0.).forward.map(|v| v * 500.),
        bay_ready: true,
        radar_power: false,
        radar: false,
        jammer: false,
        alive: true,
        controls: Default::default(),
    };
    let initial = state.ammo[index];
    let mut safe = state.clone();
    safe.command(live::Command::ToggleArm, l);
    safe.step(true, l, |_, _| 0.);
    if safe.ammo[index] != initial {
        return Err("safe unguided store released".into());
    }
    let mut failed = state.clone();
    failed.command(live::Command::FailStation, l);
    failed.step(true, l, |_, _| 0.);
    if failed.rounds(index) != initial {
        return Err("failed unguided station released".into());
    }
    let mut replay = state.clone();
    let mut ground = false;
    let mut fired = false;
    for tick in 0..7200 {
        let events = state.step(tick == 0, l, |_, _| 0.);
        if events != replay.step(tick == 0, l, |_, _| 0.)
            || format!("{state:?}") != format!("{replay:?}")
        {
            return Err("unguided release replay diverged".into());
        }
        fired |= events.iter().any(|e| matches!(e, Event::Fired(_)));
        ground |= events.contains(&Event::Ground);
        if ground {
            break;
        }
    }
    if !fired
        || !ground
        || state.ammo[index] >= initial
        || state.projectiles.iter().any(|p| p.target.is_some())
    {
        return Err(format!(
            "unguided release/contact failed {}",
            config.stations[index].weapon.source
        )
        .into());
    }
    state.command(live::Command::Jettison, l);
    if state.rounds(index) != 0 {
        return Err("unguided jettison failed".into());
    }
    println!(
        "ballistic smoke {:?} {}: safe/failure inhibition, no-lock release, deterministic ground contact and jettison PASS",
        config.aircraft, config.stations[index].weapon.source
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fire_requires_unmodified_press_and_release_after_interruption() {
        let mut f = FireInput::default();
        f.space(true, false, true);
        assert!(!f.held);
        f.space(true, true, false);
        assert!(!f.held);
        f.space(false, false, false);
        f.space(true, false, false);
        assert!(f.held);
        f.cancel();
        assert!(!f.held);
        f.space(true, true, false);
        assert!(!f.held);
        f.space(false, false, true);
        f.space(true, false, false);
        assert!(f.held);
    }
}

#[cfg(test)]
mod ai_pose_tests {
    use super::*;
    use crate::render_snapshot::blend;
    use tore_formats::aircraft::AircraftId;
    use tore_sim::{
        combat::missiles::{TargetRole, seeker::Heat},
        sensors,
    };

    fn target(velocity: Vector, basis: Basis) -> live::Target {
        live::Target {
            aircraft: Some(AircraftId::F18),
            role: TargetRole::Aircraft,
            heat: Heat::Unknown,
            radar_emitting: false,
            id: 1,
            position: [0.; 3],
            velocity,
            basis,
            configuration: sensors::Configuration::CLEAN,
            signature: sensors::SignatureProfile::default(),
            jammer: None,
            jammer_active: false,
            airborne: true,
            on_ground: false,
            radius: 28.,
            hp: 100,
            initial_hp: 100,
            fragment_offsets: [[0.; 3]; 2],
            wreck: None,
            wreck_power: tore_sim::wreck::Power::default(),
            fragment_released: false,
            localized_damage: live::LocalizedDamage::default(),
            category: 0,
        }
    }

    /// A target's drawn pose as a snapshot records it.
    fn drawn(t: &live::Target) -> AircraftPose {
        AircraftPose {
            id: t.id,
            position: t.position,
            attitude: target_pose(t, true),
            ..Default::default()
        }
    }

    #[test]
    fn formation_rendering_shares_the_camera_tick_fraction() {
        // A fixed slot must stay fixed at every render fraction, including
        // frames without a simulation tick. 800 ft/s used to produce a
        // 6.67 ft (2.03 m) sawtooth when only the camera was interpolated.
        for turning in [false, true] {
            let mut t = target([0., 0., 800.], Basis::new(0., 0., 0.));
            let mut camera_before = [0.; 3];
            for tick in 0..240 {
                let offset = [512., 0., -512.];
                t.position = std::array::from_fn(|i| camera_before[i] + offset[i]);
                let previous = drawn(&t);
                let heading = if turning { tick as f64 * 0.001 } else { 0. };
                let camera_after: Vector = std::array::from_fn(|i| {
                    camera_before[i] + Basis::new(heading, 0., 0.).forward[i] * 800. / 120.
                });
                t.position = std::array::from_fn(|i| camera_after[i] + offset[i]);
                t.basis = Basis::new(heading, 0.1, 0.3);
                let authoritative = t.position;
                for alpha in [0., 0.13, 0.5, 0.91, 1.] {
                    let pose = blend(Some(&previous), &drawn(&t), alpha);
                    for i in 0..3 {
                        let camera =
                            camera_before[i] + (camera_after[i] - camera_before[i]) * alpha;
                        assert!((pose.position[i] - camera - offset[i]).abs() < 1e-9);
                    }
                    assert!(pose.attitude.iter().all(|a| a.is_finite()));
                    assert_eq!(t.position, authoritative);
                }
                camera_before = camera_after;
            }
            assert_eq!(
                blend(None, &drawn(&t), 0.5).position,
                t.position,
                "restart must discard history"
            );
        }
    }

    #[test]
    fn target_presentation_blends_attitude_and_keeps_new_targets_current() {
        let mut t = target([0., 0., 800.], Basis::new(359_f64.to_radians(), 0., 0.));
        let previous = drawn(&t);
        t.basis = Basis::new(1_f64.to_radians(), 0., 0.);
        let pose = blend(Some(&previous), &drawn(&t), 0.5);
        assert!(
            pose.attitude[0].sin().abs() < 1e-9,
            "heading must take the short path"
        );
        t.id = 2;
        t.position = [100.; 3];
        let pose = blend(None, &drawn(&t), 0.5);
        assert_eq!(
            (pose.position, pose.attitude),
            (t.position, t.basis.angles())
        );
    }

    /// The fixture rule is unchanged with AI poses disabled: heading from the
    /// velocity, level wings, whatever attitude the row happens to carry.
    #[test]
    fn the_fixture_pose_ignores_the_stored_attitude() {
        let banked = Basis::new(1.5, 0.4, 0.9);
        let t = target([300., 0., 0.], banked);
        let pose = target_pose(&t, false);
        assert!((pose[0] - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(pose[1], 0.);
        assert_eq!(pose[2], 0.);
    }

    /// With the option on the AI's own attitude is drawn, so a banking AI
    /// aircraft looks like one.
    #[test]
    fn the_ai_pose_uses_the_stored_attitude() {
        let banked = Basis::new(1.5, 0.4, 0.9);
        let t = target([300., 0., 0.], banked);
        let pose = target_pose(&t, true);
        for (got, want) in pose.iter().zip(banked.angles()) {
            assert!(
                (got - want).abs() < 1e-9,
                "{pose:?} vs {:?}",
                banked.angles()
            );
        }
        assert!(pose[2].abs() > 0.5, "bank was discarded: {pose:?}");
    }
}

/// Exact drawn output of one synthetic combat scene. The hashes pin every
/// vertex live flight uploads for other aircraft, debris, weapons, effects
/// and ejected pilots, so a presentation refactor can prove it changed nothing.
/// Mission recordings reuse the scene to prove a replay draws the same.
#[cfg(test)]
pub(crate) mod render_hash_tests {
    use super::*;
    use crate::{damage_art::DamageArt, render_snapshot::combat_geometry};
    use tore_formats::{
        aircraft::AircraftId,
        shape::{Face, FogMode, Line, Shape},
        weapons::Weapon,
    };
    use tore_sim::attitude::unit;
    use tore_sim::{
        combat::{
            FallState,
            debris::Piece,
            live::{DamageSection, LocalizedDamage},
            missiles::{TargetRole, seeker::Heat},
        },
        ejection::{Escape, Phase},
        sensors,
    };

    /// Batches per model, fixture targets with the ownship, combat geometry
    /// beside loaded models, camera poses and ejected pilots.
    const HASHES: [u64; 5] = [
        0xd383_5c36_01f2_ee53,
        0x512b_90ca_288a_4dfd,
        0x8f02_bfe4_efe7_7fb5,
        0x7ddb_79bb_1315_bf69,
        0x7e3f_dcdd_a149_201d,
    ];

    /// Where `HASHES` was recorded. The camera poses (`HASHES[3]`) are f64
    /// angles from trigonometry, whose last bit differs between maths
    /// libraries, so, as in tore-sim's golden tests, that hash is compared
    /// only here. The drawn vertices are f32 and match on every CI platform.
    const RECORDED_PLATFORM: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

    /// FNV-1a over exact bit patterns.
    struct Fnv(u64);
    impl Fnv {
        fn new() -> Self {
            Self(0xcbf2_9ce4_8422_2325)
        }
        fn bytes(&mut self, bytes: &[u8]) {
            for byte in bytes {
                self.0 = (self.0 ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3);
            }
        }
        fn count(&mut self, count: usize) {
            self.bytes(&(count as u64).to_le_bytes());
        }
        fn floats(&mut self, values: &[f32]) {
            self.count(values.len());
            for value in values {
                self.bytes(&value.to_bits().to_le_bytes());
            }
        }
        fn doubles(&mut self, values: &[f64]) {
            self.count(values.len());
            for value in values {
                self.bytes(&value.to_bits().to_le_bytes());
            }
        }
        fn contacts(&mut self, contacts: &[Contact]) {
            self.count(contacts.len());
            for contact in contacts {
                self.bytes(&contact.first.to_le_bytes());
                self.bytes(&contact.count.to_le_bytes());
                self.floats(&contact.center);
                self.floats(&[contact.extent]);
            }
        }
    }

    /// A small source-space polygon (X right, Y forward, Z up) around `at`.
    fn face(address: usize, at: [f32; 3], corners: usize, texture: &str, subtype: u8) -> Face {
        Face {
            positions: (0..corners)
                .map(|i| {
                    let angle = i as f32 * std::f32::consts::TAU / corners as f32 + 0.3;
                    [
                        at[0] + 4. * angle.cos(),
                        at[1] + 3. * angle.sin(),
                        at[2] + angle.sin(),
                    ]
                })
                .collect(),
            colors: (0..corners)
                .map(|i| (address % 97 + i * 31) as u8)
                .collect(),
            fog: if address.is_multiple_of(2) {
                FogMode::Enabled
            } else {
                FogMode::Disabled
            },
            uv: if texture.is_empty() {
                Vec::new()
            } else {
                (0..corners)
                    .map(|i| [1. + i as f32, 2. + (i % 2) as f32])
                    .collect()
            },
            texture: texture.into(),
            subtype,
            normal: Some([0.3, 0.8, -0.5]),
            address,
        }
    }
    fn shape(faces: Vec<Face>) -> Shape {
        Shape {
            lines: Vec::new(),
            faces,
            state_words: Default::default(),
        }
    }
    /// Reviewed F/A-18D part addresses: body, flame, both nozzles, brake,
    /// hook, gear and doors, flap, both tailplanes and a split rudder.
    fn hornet() -> Shape {
        let mut rudder = face(0x5467, [0.; 3], 4, "", 0x20);
        rudder.positions = vec![
            [8., -40., 4.],
            [8., -20., 4.],
            [18., -30., 24.],
            [18., -45., 24.],
        ];
        shape(vec![
            face(0x1000, [0., 10., 0.], 4, "_SYN.PIC", 0x60),
            face(0x1001, [-12., -5., 1.], 3, "", 0x21),
            face(0x5310, [0., -58., 0.], 4, "_SYN.PIC", 0x4c),
            face(0x3ff7, [-3., -50., 0.], 4, "", 0x20),
            face(0x4026, [3., -50., 0.], 4, "", 0x20),
            face(0x5059, [0., -28., 5.], 3, "", 0x20),
            face(0x4a03, [0., -37., -3.], 3, "", 0x20),
            face(0x4b69, [-3., 0., -6.], 3, "", 0x20),
            face(0x4bfa, [8., -7., -6.], 3, "", 0x20),
            face(0x4ee1, [0., 55., -6.], 3, "", 0x20),
            face(0x525b, [-13., -8., 4.], 4, "", 0x20),
            face(0x449f, [-11., -43., 0.], 3, "", 0x20),
            face(0x486b, [11., -43., 0.], 3, "", 0x20),
            rudder,
        ])
    }
    /// Rafale C parts; the gear-down pose adds a main gear leg.
    fn rafale(gear: bool) -> Shape {
        let mut faces = vec![
            face(0x1000, [0., 12., 0.], 4, "_SYN.PIC", 0x60),
            face(0x4265, [0., -49., 0.], 4, "_SYN.PIC", 0x4c),
            face(0x3032, [-2., -45., 0.], 4, "", 0x20),
            face(0x3e1e, [2., -3., 4.], 3, "", 0x20),
            face(0x3b2f, [-2., 7., -7.], 3, "", 0x20),
            face(0x4190, [-10., -23., -1.], 4, "", 0x20),
            face(0x3d81, [-5., 30., 0.], 3, "", 0x20),
            face(0x3f08, [0., -40., 10.], 3, "", 0x20),
        ];
        if gear {
            faces.push(face(0x3c50, [-4., 10., -7.], 3, "", 0x20));
        }
        shape(faces)
    }
    /// F-14 parts over a synthetic rig: swept wing, flap, tailplane, split
    /// rudder, cold nozzle, and rig flame, gear, hook and brake groups.
    fn tomcat() -> Shape {
        shape(vec![
            face(0x1000, [0., 8., 0.], 4, "_SYN.PIC", 0x60),
            face(0x3000, [0., -20., 0.], 4, "", 0x4c),
            face(0x3100, [0., 17., -3.], 3, "", 0x20),
            face(0x3101, [5., 1., -2.], 3, "", 0x20),
            face(0x3200, [0., -5., -2.], 3, "", 0x20),
            face(0x3300, [2., -11., 1.], 3, "", 0x20),
            face(0x4e00, [-12., -2., 1.], 4, "", 0x20),
            face(0x540d, [10., -6., 1.], 3, "", 0x20),
            face(0x4828, [-6., -12., 0.], 3, "", 0x20),
            face(0x4a7b, [4., -14., 6.], 4, "", 0x20),
            face(0x48a6, [-2., -16., 0.], 4, "", 0x20),
        ])
    }
    fn damage() -> DamageArt {
        let piece = |address: usize, texture: &str| {
            shape(vec![
                face(address, [2., 5., 1.], 4, texture, 0x60),
                face(address + 1, [-6., -9., 0.], 3, "", 0x20),
            ])
        };
        DamageArt::synthetic(
            [20., 60., 12.],
            [piece(0x2000, "_DMG.PIC"), piece(0x2100, "_SYN.PIC")],
            [piece(0x2200, "_DMG.PIC"), piece(0x2300, "")],
            BTreeMap::from([
                ("_SYN.PIC".to_string(), [16, 8, 0]),
                ("_DMG.PIC".to_string(), [16, 4, 8]),
                ("_F18_A.PIC".to_string(), [16, 4, 12]),
            ]),
        )
    }
    pub(crate) fn hornet_airframe(material: bool) -> Airframe {
        Airframe::synthetic(
            AircraftId::F18,
            (0..16).map(|_| hornet()).collect(),
            None,
            damage(),
            material.then(|| crate::engine_material::Image {
                width: 2,
                height: 2,
                pixels: vec![200; 16],
            }),
        )
    }
    pub(crate) fn models() -> Vec<Airframe> {
        vec![
            hornet_airframe(false),
            Airframe::synthetic(
                AircraftId::Rafale,
                vec![rafale(false), rafale(true)],
                None,
                damage(),
                None,
            ),
            Airframe::synthetic(
                AircraftId::F14,
                vec![tomcat()],
                Some(crate::additional_animation::Rig::synthetic(
                    AircraftId::F14,
                    &[0x3000],
                    &[0x3300],
                    &[0x3100, 0x3101],
                    &[0x3200],
                )),
                damage(),
                None,
            ),
        ]
    }
    fn missile_shape(seed: usize) -> Shape {
        shape(vec![
            face(0x10 + seed, [0., 10., 0.], 4, "", 0x61),
            face(0x20 + seed, [0., -10., 1.], 3, "", 0x4c),
            face(0x30 + seed, [2., 0., 0.], 3, "", 0x20),
        ])
    }
    /// Twelve synthetic effect frames, a different cell count per frame.
    fn frames(seed: usize) -> Vec<Vec<([f32; 2], [f32; 3])>> {
        (0..12)
            .map(|frame| {
                (0..=(frame + seed) % 4)
                    .map(|cell| {
                        (
                            [cell as f32 / 20. - 0.5, 0.5 - frame as f32 / 20.],
                            [frame as f32 / 12., cell as f32 / 4., seed as f32 / 8.],
                        )
                    })
                    .collect()
            })
            .collect()
    }
    /// The player's gun and one missile station with a loaded shape.
    fn state() -> live::State {
        let mut config = crate::ai_wings::tests::combat_fixture(false)
            .configuration()
            .clone();
        config.stations[0].weapon.source = "M61.JT".into();
        let mut missile = config.stations[0].clone();
        missile.weapon.source = "SYNMSL.JT".into();
        missile.weapon.shape = Some("SYNMSL.SH".into());
        config.stations.push(missile);
        live::State::new(config, true).unwrap()
    }
    pub(crate) fn combat(models: Vec<Airframe>, dummies: Vec<(usize, Vector)>) -> Combat {
        Combat {
            state: state(),
            art: CombatArt::synthetic(
                BTreeMap::from([
                    ("SYNMSL.SH".to_string(), missile_shape(0)),
                    ("AIMSL.SH".to_string(), missile_shape(1)),
                ]),
                frames(1),
                frames(2),
            ),
            contrail_offsets: Vec::new(),
            contrail_sortie: 0,
            contrails: Default::default(),
            input: FireInput::default(),
            controller: FireInput::default(),
            range: false,
            ai_poses: true,
            clean_recording: false,
            initial_ammo: None,
            render: RenderHistory::default(),
            dummies,
            mission_spawns: None,
            mission_layout: None,
            dummy_models: models,
            dummy_configs: Vec::new(),
            dummy_contrail_offsets: Vec::new(),
            airport_objects: Vec::new(),
            recorder: None,
            last_launcher: None,
            notes: Default::default(),
        }
    }

    fn aircraft(
        id: u32,
        kind: AircraftId,
        position: Vector,
        velocity: Vector,
        basis: Basis,
    ) -> live::Target {
        live::Target {
            aircraft: Some(kind),
            role: TargetRole::Aircraft,
            heat: Heat::Unknown,
            radar_emitting: false,
            id,
            position,
            velocity,
            basis,
            configuration: sensors::Configuration::CLEAN,
            signature: sensors::SignatureProfile::default(),
            jammer: None,
            jammer_active: false,
            airborne: true,
            on_ground: false,
            radius: 28.,
            hp: 100,
            initial_hp: 100,
            fragment_offsets: [[0.; 3]; 2],
            wreck: None,
            wreck_power: tore_sim::wreck::Power::default(),
            fragment_released: false,
            localized_damage: LocalizedDamage::default(),
            category: 0,
        }
    }
    fn damaged(amounts: [i32; 6], section: Option<DamageSection>) -> LocalizedDamage {
        LocalizedDamage {
            amounts,
            structural_variant: section.map(|s| usize::from(s as u8 > 2)),
            structural_section: section,
        }
    }
    fn wreck(id: u32, phase: tore_sim::wreck::Phase) -> Option<tore_sim::wreck::Wreck> {
        let mut wreck = tore_sim::wreck::Wreck::new(id, 100, [0.; 3]);
        wreck.phase = phase;
        Some(wreck)
    }
    fn shot(
        id: u32,
        owner: u32,
        station: usize,
        weapon: Option<Weapon>,
        [position, previous]: [Vector; 2],
        direction: Vector,
        tracer: bool,
    ) -> live::Projectile {
        live::Projectile {
            id,
            owner,
            weapon,
            guidance: None,
            motion: None,
            guidance_ticks: None,
            age: 3,
            incoming: owner != 0,
            station,
            position,
            previous,
            direction: unit(direction),
            speed_f8: 900 * 256,
            launched_t: 0,
            target: Some(1),
            fall: FallState::default(),
            gun_round: None,
            tracer,
        }
    }
    fn piece(owner: u32, position: Vector, basis: Basis) -> Piece {
        Piece {
            owner,
            variant: 0,
            position,
            velocity: [0.; 3],
            basis,
        }
    }

    pub(crate) struct Scene {
        previous: Vec<live::Target>,
        current: Vec<live::Target>,
        devices: Vec<(u32, [f64; 11], [f64; 11])>,
        projectiles: Vec<live::Projectile>,
        effects: Vec<live::Effect>,
        debris: Vec<Piece>,
    }
    pub(crate) fn scene(config: &live::Configuration) -> Scene {
        use AircraftId::{F14, F18, Rafale};
        use tore_sim::wreck::Phase::{Exploded, Falling};
        let b = Basis::new;
        let mut current: Vec<_> = [
            (
                1,
                F18,
                [12., 5003., 3006.5],
                [80., 20., 700.],
                [0.25, 0.06, 0.5],
            ),
            (
                2,
                Rafale,
                [405., 5099., 2507.],
                [30., -5., 650.],
                [0.05, -0.12, -0.35],
            ),
            (
                3,
                F14,
                [-590., 4905., 3510.],
                [50., 25., 600.],
                [0.12, 0.21, 0.12],
            ),
            (
                4,
                F18,
                [201., 4795., 4005.],
                [10., -60., 500.],
                [0.3, -0.5, 1.2],
            ),
            (
                5,
                Rafale,
                [-300., 5200., 2800.],
                [0., -40., 400.],
                [5.9, -0.3, 2.],
            ),
            (6, F14, [900., 5000., 3000.], [0.; 3], [1., 0., 0.]),
            (7, F18, [150., 5050., 2200.], [0., 0., 750.], [0., 0.02, 0.]),
            (
                20,
                F18,
                [-800., 5100., 2600.],
                [-20., 0., 640.],
                [6.2, 0.1, -0.6],
            ),
            (100, F18, [300., 4000., 3000.], [0.; 3], [0.7, 0., 0.]),
            (101, F18, [320., 4000., 3040.], [0.; 3], [1.7, 0., 0.]),
        ]
        .into_iter()
        .map(|(id, kind, position, velocity, [yaw, pitch, bank])| {
            aircraft(id, kind, position, velocity, b(yaw, pitch, bank))
        })
        .collect();
        // Flying on with a failed left wing.
        current[1].hp = 60;
        current[1].localized_damage = damaged([0, 0, 5, 80, 0, 0], Some(DamageSection::LeftWing));
        // Destroyed through the nose and falling.
        current[3].hp = 0;
        current[3].localized_damage = damaged([90, 0, 0, 0, 0, 0], Some(DamageSection::Nose));
        current[3].wreck = wreck(4, Falling);
        // Destroyed without a structural break, wreck already exploded.
        current[4].hp = 0;
        current[4].localized_damage = damaged([30, 30, 20, 10, 5, 5], None);
        current[4].wreck = wreck(5, Exploded);
        // Parked.
        current[5].airborne = false;
        current[5].on_ground = true;
        // No model slot, carrying a left-wing break for its debris.
        current[7].localized_damage = damaged([0, 0, 0, 80, 0, 0], Some(DamageSection::LeftWing));
        // Ground objects, one standing and one destroyed.
        for (target, hp) in current[8..].iter_mut().zip([50, 0]) {
            target.aircraft = None;
            target.role = TargetRole::Surface;
            target.airborne = false;
            target.hp = hp;
        }
        let mut previous: Vec<_> = current
            .iter()
            .filter(|t| t.id != 20)
            .cloned()
            .map(|mut t| {
                let shift = f64::from(t.id % 10);
                t.position = [
                    t.position[0] - 3. * shift,
                    t.position[1] + 0.5 * shift,
                    t.position[2] - 6. - shift,
                ];
                t.velocity[0] -= 10.;
                let [yaw, pitch, bank] = t.basis.angles();
                t.basis = Basis::new(yaw - 0.02 * shift, pitch + 0.01, bank - 0.05 * shift);
                t
            })
            .collect();
        // A heading that crosses north between ticks.
        previous[1].basis = Basis::new(6.26, -0.1, -0.3);
        previous.push(aircraft(
            21,
            F18,
            [0., 5000., 0.],
            [0., 0., 700.],
            b(0., 0., 0.),
        ));
        let devices = vec![
            (
                1,
                [1., 0.5, 0., 0., 0., 0.2, 0.1, -0.3, 0.05, 700., 0.6],
                [0.8, 0.4, 0.3, 0.1, 0., 0.6, -0.2, 0.2, -0.1, 720., 0.9],
            ),
            (
                2,
                [0.3, 0.2, 0.5, 0., 0., 1., 0.2, 0.1, 0.3, 600., 0.8],
                [0.2, 0.25, 0.6, 0., 0., 0.9, 0.15, 0.2, 0.25, 610., 0.85],
            ),
            (
                3,
                [0., 0.2, 0.4, 1., 0., 0.5, 0.3, -0.2, 0.2, 800., 0.5],
                [0.5, 0.1, 0.3, 1., 0., 0.7, 0.25, -0.1, 0.25, 860., 0.55],
            ),
            (
                5,
                [0., 0., 0., 0., 0., 0.4, 0.2, 0.2, 0.1, 500., 0.4],
                [0., 0., 0., 0., 0., 0.3, 0.1, 0.25, 0.15, 480., 0.3],
            ),
        ];
        let weapon = |source: &str, shape: Option<&str>| {
            let mut weapon = config.stations[1].weapon.clone();
            weapon.source = source.into();
            weapon.shape = shape.map(Into::into);
            Some(weapon)
        };
        let projectiles = vec![
            shot(
                10,
                0,
                0,
                None,
                [[3., 5001., 130.], [0., 5000., 100.]],
                [3., 1., 30.],
                true,
            ),
            shot(
                11,
                0,
                0,
                None,
                [[5., 5002., 160.], [2., 5001., 130.]],
                [3., 1., 30.],
                false,
            ),
            shot(
                12,
                0,
                0,
                None,
                [[9., 5003., 90.], [9., 5003., 90.]],
                [0., 0., 1.],
                true,
            ),
            shot(
                13,
                0,
                1,
                None,
                [[100., 5020., 900.], [98., 5019., 880.]],
                [0.1, 0.05, 0.99],
                false,
            ),
            shot(
                14,
                1,
                0,
                weapon("AIMSL.JT", Some("AIMSL.SH")),
                [[300., 5040., 1500.], [301., 5041., 1520.]],
                [-0.05, -0.05, -1.],
                false,
            ),
            shot(
                15,
                2,
                0,
                weapon("NOSHAPE.JT", None),
                [[-200., 4990., 1800.], [-199., 4990., 1790.]],
                [-0.1, 0., 1.],
                false,
            ),
            shot(
                16,
                3,
                0,
                weapon("GSH301.JT", None),
                [[-100., 5010., 1200.], [-95., 5011., 1180.]],
                [-0.25, -0.05, 1.],
                true,
            ),
            shot(
                17,
                0,
                1,
                None,
                [[50., 5015., 700.], [49., 5015., 690.]],
                [0.1, 0., 1.],
                false,
            ),
        ];
        let effect = |kind, position, ticks| live::Effect {
            position,
            kind,
            ticks,
        };
        let effects = vec![
            effect(EffectKind::Flare, [10., 5050., 500.], 45),
            effect(EffectKind::Flare, [14., 5040., 520.], 12),
            effect(EffectKind::Chaff, [-20., 5060., 480.], 30),
            effect(EffectKind::Launch, [0., 5000., 300.], 40),
            effect(EffectKind::Hit, [30., 5000., 900.], 45),
            effect(EffectKind::Hit, [35., 5005., 910.], 23),
            effect(EffectKind::Hit, [40., 5010., 920.], 1),
            effect(EffectKind::Destroyed, [200., 4800., 4000.], 240),
            effect(EffectKind::Destroyed, [210., 4790., 4010.], 121),
            effect(EffectKind::Destroyed, [220., 4780., 4020.], 7),
            effect(EffectKind::Ground, [0., 4000., 1500.], 45),
            effect(EffectKind::Ground, [10., 4000., 1510.], 3),
            effect(EffectKind::DebrisImpact, [-30., 4000., 1600.], 45),
            effect(EffectKind::DebrisImpact, [-40., 4000., 1610.], 20),
        ];
        let debris = vec![
            piece(0, [50., 5010., 2900.], b(0.4, 0.2, 0.9)),
            piece(1, [20., 5000., 3000.], b(1., 0., 0.3)),
            piece(2, [410., 5090., 2510.], b(2., 0.4, -1.)),
            piece(4, [210., 4780., 4010.], b(0.1, -0.2, 0.5)),
            piece(20, [-790., 5100., 2610.], b(0.3, 0.3, 0.3)),
            piece(99, [0., 5000., 2500.], b(0., 0., 0.)),
        ];
        Scene {
            previous,
            current,
            devices,
            projectiles,
            effects,
            debris,
        }
    }
    /// The presented player state that fixture targets copy.
    pub(crate) fn player() -> flight::State {
        let mut s =
            flight::State::new(&flight::animation_tests::profile(), [0., 5000., 0.]).unwrap();
        s.bay = 0.35;
        s.speed = 820.;
        s.throttle = 0.95;
        s.burner = true;
        s.exhaust = 0.8;
        s.gear = 1.;
        s.flaps = 0.5;
        s.brake = 0.2;
        s.hook = 0.4;
        s.elevator = 0.3;
        s.aileron = -0.2;
        s.rudder = 0.1;
        s.auxiliary_rates = [0.1, 0.4, -0.3];
        s.damage_variant = Some(DamageSection::LeftWing as usize);
        s
    }
    fn camera(position: [f32; 3], [yaw, pitch, roll]: [f32; 3]) -> Camera {
        let mut camera = Camera::new();
        camera.position = position;
        camera.yaw = yaw;
        camera.pitch = pitch;
        camera.roll = roll;
        camera
    }
    /// A general view, one hiding target 7 and missile 17, and one looking
    /// straight down the player's first tracer.
    pub(crate) fn cameras() -> Vec<Camera> {
        let mut hiding = camera([500., 5100., 1500.], [-0.3, 0.05, -0.2]);
        hiding.hidden_target = Some(7);
        hiding.hidden_projectile = Some(17);
        vec![
            camera([-400., 5400., 600.], [0.2, -0.15, 0.1]),
            hiding,
            camera([33., 5011., 430.], [3.24, -0.03, 0.]),
        ]
    }
    pub(crate) fn pilots() -> Vec<Escape> {
        [
            Phase::Seat,
            Phase::Freefall,
            Phase::Inflating,
            Phase::Parachute,
            Phase::Landed,
            Phase::Impact,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, phase)| {
            let i = i as f64;
            let mut pilot = Escape::new(
                [100. * i, 5000. - 50. * i, 2000. + 30. * i],
                [0.; 3],
                Basis::new(0.4 * i, 0., 0.),
            );
            pilot.phase = phase;
            pilot
        })
        .collect()
    }
    pub(crate) fn escape_art() -> crate::ejection_art::Art {
        let pose = |n: usize| Shape {
            lines: vec![
                Line {
                    positions: [[0., 0., 3. + n as f32], [2., 1., 9.]],
                    color: 40 + n as u8,
                    fog: FogMode::Enabled,
                },
                Line {
                    positions: [[1.; 3], [1.; 3]],
                    color: 1,
                    fog: FogMode::Disabled,
                },
            ],
            faces: vec![
                face(0x10 + n, [0., 0., 2.], 4, "_EJECTA.PIC", 0x60),
                face(0x20 + n, [1., 1., 1.], 3, "", 0x20),
            ],
            state_words: Default::default(),
        };
        crate::ejection_art::Art::synthetic(
            (0..5).map(pose).collect(),
            &[("_EJECTA.PIC", 8, 8), ("_EJECTB.PIC", 8, 4)],
        )
    }

    /// Presents the scene at one tick fraction, as live flight would: the
    /// previous and current tick's snapshots, each aircraft's devices as the
    /// AI simulated them on those ticks, and the frame's fraction.
    fn load(
        combat: &mut Combat,
        scene: &Scene,
        alpha: f64,
        ai_poses: bool,
        player: &flight::State,
    ) {
        let [previous, current] = snapshots(combat, scene, ai_poses, player);
        combat.render = RenderHistory::default();
        combat.render.set_current(previous);
        combat.render.advance(current);
        combat.present_targets(alpha);
    }
    /// The scene's previous and current tick as live flight snapshots them,
    /// with each aircraft's devices as the AI simulated them on those ticks.
    /// Leaves the current tick's scene in `combat.state`.
    pub(crate) fn snapshots(
        combat: &mut Combat,
        scene: &Scene,
        ai_poses: bool,
        player: &flight::State,
    ) -> [RenderSnapshot; 2] {
        combat.ai_poses = ai_poses;
        combat.render = RenderHistory::default();
        combat.state.targets.clone_from(&scene.previous);
        let mut previous = combat.snapshot(player, None);
        combat.state.targets.clone_from(&scene.current);
        combat.state.projectiles.clone_from(&scene.projectiles);
        combat.state.effects.clone_from(&scene.effects);
        combat.state.debris.clone_from(&scene.debris);
        let mut current = combat.snapshot(player, None);
        for &(id, before, after) in &scene.devices {
            for (snapshot, devices) in [(&mut previous, before), (&mut current, after)] {
                if let Some(pose) = snapshot.targets.iter_mut().find(|pose| pose.id == id) {
                    pose.devices = Some(devices);
                }
            }
        }
        [previous, current]
    }

    #[test]
    fn drawn_combat_scene_is_unchanged() {
        let ownship = hornet_airframe(true);
        let player = player();
        let mut stepped = crate::terrain::tests::world();
        stepped.smooth_weather = false;
        let worlds = [crate::terrain::tests::world(), stepped];
        let mut with_models = combat(models(), (0..7).map(|i| (i % 3, [0.; 3])).collect());
        let mut fixture = combat(Vec::new(), Vec::new());
        let scene = scene(with_models.state.configuration());
        let mut hashes = [(); 5].map(|()| Fnv::new());
        let mut drawn = [0; 3];
        for ai_poses in [true, false] {
            for alpha in [0., 0.37, 1.] {
                load(&mut with_models, &scene, alpha, ai_poses, &player);
                load(&mut fixture, &scene, alpha, ai_poses, &player);
                for target in &scene.current {
                    let (position, angles) = with_models.view_pose(target, true);
                    hashes[3].doubles(&position);
                    hashes[3].doubles(&angles);
                }
                let [modelled, fixtures] = [&with_models, &fixture].map(Combat::presented);
                for world in &worlds {
                    for camera in cameras() {
                        for (model, vertices, contacts) in
                            with_models.dummy_geometry(&camera, world)
                        {
                            hashes[0].bytes(format!("{:?}", model.profile.id).as_bytes());
                            hashes[0].floats(&vertices);
                            hashes[0].contacts(&contacts);
                            drawn[0] += vertices.len();
                        }
                        let geometry = combat_geometry(
                            &fixtures,
                            &fixture.art,
                            &ownship,
                            &player,
                            &camera,
                            world,
                        );
                        hashes[1].floats(&geometry.vertices);
                        hashes[1].contacts(&geometry.contacts);
                        drawn[1] += geometry.vertices.len();
                        let geometry = combat_geometry(
                            &modelled,
                            &with_models.art,
                            &ownship,
                            &player,
                            &camera,
                            world,
                        );
                        hashes[2].floats(&geometry.vertices);
                        hashes[2].contacts(&geometry.contacts);
                        drawn[2] += geometry.vertices.len();
                    }
                }
            }
        }
        let art = escape_art();
        let pilots = pilots();
        for camera in cameras() {
            hashes[4].floats(&art.vertices_for(
                pilots.iter().map(|p| (p.position, p.heading, p.phase)),
                &ownship.palette,
                camera.position.map(f64::from),
            ));
        }
        assert!(drawn.iter().all(|&floats| floats > 10_000), "{drawn:?}");
        let mut hashes = hashes.map(|hash| hash.0);
        if !RECORDED_PLATFORM {
            hashes[3] = HASHES[3];
        }
        assert_eq!(hashes, HASHES, "drawn output changed: {hashes:#018x?}");
    }

    #[test]
    fn snapshots_route_each_aircraft_and_piece_to_the_model_that_draws_it() {
        use crate::render_snapshot::Draw;
        let player = player();
        let mut with_models = combat(models(), (0..7).map(|i| (i % 3, [0.; 3])).collect());
        let scene = scene(with_models.state.configuration());
        load(&mut with_models, &scene, 0.5, true, &player);
        let snapshot = with_models.render_snapshot();
        let draw = |id| snapshot.target(id).unwrap().draw;
        assert_eq!(draw(1), Draw::Model(AircraftId::F18));
        assert_eq!(draw(2), Draw::Model(AircraftId::Rafale));
        assert_eq!(draw(3), Draw::Model(AircraftId::F14));
        assert_eq!(draw(20), Draw::Hidden, "no model slot");
        assert_eq!(
            snapshot.models,
            [AircraftId::F18, AircraftId::Rafale, AircraftId::F14]
        );
        let debris: Vec<_> = snapshot
            .debris
            .iter()
            .map(|piece| (piece.owner, piece.draw, piece.variant))
            .collect();
        assert_eq!(
            debris,
            [
                (0, Draw::Ownship, player.damage_variant),
                (1, Draw::Model(AircraftId::F18), None),
                (2, Draw::Model(AircraftId::Rafale), Some(3)),
                (4, Draw::Model(AircraftId::F18), Some(0)),
                (20, Draw::Hidden, Some(3)),
                (99, Draw::Hidden, None),
            ]
        );
        let mut fixture = combat(Vec::new(), Vec::new());
        load(&mut fixture, &scene, 0.5, true, &player);
        let snapshot = fixture.render_snapshot();
        assert!(snapshot.targets.iter().all(|t| t.draw == Draw::Ownship));
        assert!(snapshot.debris.iter().all(|p| p.draw == Draw::Ownship));
        assert_eq!(snapshot.player.id, 0);
    }

    /// Fixture targets drawn from a snapshot match the rule they were drawn
    /// with before snapshots existed: a copy of the presented player state
    /// with the target's pose, damage and crash flag, devices stowed. The
    /// player states cover afterburner, dry, engine off, no fuel and a
    /// crashed player whose afterburner switch is still on.
    #[test]
    fn fixture_targets_keep_the_player_copy_rule() {
        let ownship = hornet_airframe(true);
        let world = crate::terrain::tests::world();
        let mut fixture = combat(Vec::new(), Vec::new());
        let scene = scene(fixture.state.configuration());
        fixture.state.targets.clone_from(&scene.current);
        let mut states = Vec::new();
        for case in 0..6 {
            let mut s = player();
            s.throttle = 1.;
            match case {
                1 => s.burner = false,
                2 => s.engine = false,
                3 => s.fuel = 0.,
                4 => s.crashed = true,
                5 => s.throttle = 0.5,
                _ => {}
            }
            states.push(s);
        }
        assert!(states[0].afterburner_active() && !states[4].afterburner_active());
        for s in &states {
            fixture.restart_render(s, None);
            let snapshot = fixture.presented();
            for camera in cameras() {
                let mut expected = Vec::new();
                for t in scene
                    .current
                    .iter()
                    .filter(|t| t.airborne && Some(t.id) != camera.hidden_target)
                {
                    let mut pose = s.clone();
                    pose.wreck = t.wreck.clone();
                    pose.crashed = t.hp <= 0;
                    pose.position = t.position;
                    pose.damage_fraction = t.damage_fraction();
                    pose.damage_variant = t
                        .localized_damage
                        .structural_section
                        .map(|section| section as usize);
                    pose.damage_regions = t.localized_damage.fractions(t.initial_hp);
                    [pose.yaw, pose.pitch, pose.bank] = target_pose(t, fixture.ai_poses);
                    pose.exhaust = 0.;
                    pose.gear = 0.;
                    pose.flaps = 0.;
                    pose.elevator = 0.;
                    pose.aileron = 0.;
                    pose.rudder = 0.;
                    pose.brake = 0.;
                    pose.hook = 0.;
                    expected.extend(ownship.vertices(&pose, &camera, &world));
                }
                let drawn =
                    combat_geometry(&snapshot, &fixture.art, &ownship, s, &camera, &world).vertices;
                assert!(!expected.is_empty());
                assert_eq!(drawn, expected);
            }
        }
    }

    #[test]
    fn afterburner_lights_sit_in_the_flames_of_the_presented_poses() {
        use crate::countermeasure_renderer::{
            AFTERBURNER_BEHIND_FEET, AFTERBURNER_SHARE, Afterburner,
        };
        let player = player();
        let mut combat = combat(models(), (0..7).map(|i| (i % 3, [0.; 3])).collect());
        combat.contrail_offsets = vec![[-2., 0.5, -20.], [2., 0.5, -20.]];
        combat.dummy_contrail_offsets = vec![vec![[0., 1., -18.]], vec![], vec![[1., 0., -15.]]];
        let scene = scene(combat.state.configuration());
        let [previous, mut current] = snapshots(&mut combat, &scene, true, &player);
        // Aircraft 1 (the player's type), 2 (Rafale) and 3 (F-14) burn.
        for pose in &mut current.targets {
            pose.engine.flame = matches!(pose.id, 1..=3);
        }
        combat.render = RenderHistory::default();
        combat.render.set_current(previous);
        combat.render.advance(current);
        combat.present_targets(0.37);
        // The lights as they were computed before the snapshots carried
        // them: each lit aircraft's presented pose and its own outlets.
        let glow = |position: Vector, [yaw, pitch, bank]: [f64; 3], offsets: &[Vector]| {
            let basis = Basis::new(yaw, pitch, bank);
            let share = AFTERBURNER_SHARE / offsets.len().max(1) as f64;
            offsets
                .iter()
                .map(|o| Afterburner {
                    position: std::array::from_fn(|i| {
                        position[i]
                            + basis.right[i] * o[0]
                            + basis.up[i] * o[1]
                            + basis.forward[i] * (o[2] - AFTERBURNER_BEHIND_FEET)
                    }),
                    share,
                })
                .collect::<Vec<_>>()
        };
        let mut expected = Vec::new();
        if player.afterburner_active() && player.escape.is_none() {
            expected.extend(glow(
                player.position,
                [player.yaw, player.pitch, player.bank],
                &combat.contrail_offsets,
            ));
        }
        for target in combat
            .state
            .targets
            .iter()
            .filter(|t| matches!(t.id, 1..=3))
        {
            let offsets = match target.aircraft {
                Some(AircraftId::Rafale) => &combat.dummy_contrail_offsets[1],
                Some(AircraftId::F14) => &combat.dummy_contrail_offsets[2],
                _ => &combat.contrail_offsets,
            };
            let (position, angles) = combat.presented_pose(target);
            expected.extend(glow(position, angles, offsets));
        }
        let glows = combat.afterburner_glows(&player);
        assert_eq!(glows, expected);
        // Both of the player's type's engines, the Rafale's none and the
        // F-14's one light the scene.
        assert_eq!(
            glows.len(),
            3 + 2 * usize::from(player.afterburner_active())
        );
    }

    #[test]
    fn devices_hold_once_nothing_simulates_the_aircraft() {
        let player = player();
        let mut combat = combat(models(), (0..7).map(|i| (i % 3, [0.; 3])).collect());
        combat.state.targets = scene(combat.state.configuration()).current;
        combat.restart_render(&player, None);
        let devices = |combat: &Combat| combat.render_snapshot().target(1).unwrap().devices;
        assert_eq!(devices(&combat), None);
        let last = [0.5; crate::render_snapshot::DEVICES];
        combat
            .render
            .current
            .targets
            .iter_mut()
            .find(|pose| pose.id == 1)
            .unwrap()
            .devices = Some(last);
        combat.advance_render(&player, None);
        assert_eq!(devices(&combat), Some(last));
        combat.restart_render(&player, None);
        assert_eq!(devices(&combat), None, "a restart forgets them");
    }

    /// A replay rebuilds the player's aircraft from snapshots alone and draws
    /// exactly what live flight draws from the presented flight state.
    #[test]
    fn the_player_round_trips_through_snapshots() {
        use crate::render_snapshot::{interpolate, pose_state};
        let ownship = hornet_airframe(true);
        let world = crate::terrain::tests::world();
        let template =
            flight::State::new(&flight::animation_tests::profile(), [0., 5000., 0.]).unwrap();
        let mut combat = combat(Vec::new(), Vec::new());
        let scene = scene(combat.state.configuration());
        combat.state.targets.clone_from(&scene.current);
        combat.state.projectiles.clone_from(&scene.projectiles);
        combat.state.effects.clone_from(&scene.effects);
        combat.state.debris.clone_from(&scene.debris);
        let capacity = combat.state.configuration().damage_capacity;
        for case in 0..8 {
            let mut previous = player();
            previous.position = [0., 5000., 2000.];
            [previous.yaw, previous.pitch, previous.bank] = [0.1, 0., 0.2];
            previous.velocity = [10., 0., 700.];
            let mut current = previous.clone();
            current.position = [30., 5010., 2100.];
            [current.yaw, current.pitch, current.bank] = [0.2, 0.05, 0.4];
            current.velocity = [20., 5., 710.];
            [current.gear, current.flaps, current.exhaust] = [0.4, 0.2, 0.6];
            [current.elevator, current.aileron, current.rudder] = [-0.1, 0.3, -0.2];
            let mut hp = capacity / 5;
            match case {
                1 => {
                    current.burner = false;
                    [previous.throttle, current.throttle] = [0.5, 0.6];
                }
                2 => current.engine = false,
                3 => hp = 0,
                4 | 5 | 7 => {
                    current.crashed = true;
                    let mut wreck = tore_sim::wreck::Wreck::new(0, 9, [0.; 3]);
                    if case == 5 {
                        wreck.phase = tore_sim::wreck::Phase::Grounded;
                    }
                    current.wreck = Some(wreck);
                }
                _ => {}
            }
            // Afterburner lit, and still switched on after a crash.
            if matches!(case, 6 | 7) {
                [previous.throttle, current.throttle] = [0.99, 1.];
            }
            assert_eq!(current.afterburner_active(), case == 6);
            // Damage as the combat step leaves it on the flight state.
            combat
                .state
                .preview_localized_damage(DamageSection::LeftWing, 0.8);
            combat.state.player_hp = hp;
            for s in [&mut previous, &mut current] {
                s.damage_fraction = (1. - f64::from(hp) / f64::from(capacity)).clamp(0., 1.);
                s.damage_variant = combat
                    .state
                    .player_damage_section()
                    .map(|section| section as usize);
                s.damage_regions = combat.state.player_damage_regions();
            }
            let snapshots = [&previous, &current].map(|s| combat.snapshot(s, None));
            for alpha in [0., 0.37, 1.] {
                let presented = current.presented(&previous, alpha);
                let frame = interpolate(Some(&snapshots[0]), &snapshots[1], alpha);
                let rebuilt = pose_state(&template, &frame.player);
                for camera in cameras() {
                    assert_eq!(
                        ownship.vertices(&rebuilt, &camera, &world),
                        ownship.vertices(&presented, &camera, &world),
                        "case {case} at {alpha}"
                    );
                    // Fixtures and the player's debris copy the player state.
                    let [replayed, live] = [&rebuilt, &presented].map(|s| {
                        combat_geometry(&frame, &combat.art, &ownship, s, &camera, &world).vertices
                    });
                    assert_eq!(replayed, live, "fixtures, case {case} at {alpha}");
                }
            }
        }
    }
}
