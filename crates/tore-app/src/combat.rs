//! Live range host, original geometry and sampled original effect art.
use crate::{
    AppResult,
    aircraft_type::AircraftType,
    flight,
    snapshot::{
        AircraftPose, Damage, DebrisPose, Draw, EffectPose, Engine, MarkPose, PilotPose,
        ProjectilePose, RenderSnapshot,
    },
    terrain::Terrain,
};
use std::{collections::BTreeMap, sync::Arc};
use tore_formats::aircraft::AircraftId;
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
/// The last two per-tick render snapshots. They are presentation output only
/// and never feed back into sensors or physics, except that an aircraft whose
/// AI stopped flying keeps the devices last drawn for it. The frame's blend of
/// the two is the app's (`CombatView::presented`).
#[derive(Default)]
struct RenderHistory {
    /// The snapshot one tick earlier; none right after a restart.
    previous: Option<RenderSnapshot>,
    current: RenderSnapshot,
    /// Each target's index in `previous` and in `current`, by id.
    places: [BTreeMap<u32, usize>; 2],
    /// How many times the history has started over.
    restarts: u64,
}
impl RenderHistory {
    /// Forgets both snapshots, as after a reset.
    fn restart(&mut self) {
        *self = Self {
            restarts: self.restarts + 1,
            ..Self::default()
        };
    }
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
    fn previous_target(&self, id: u32) -> Option<&AircraftPose> {
        self.previous
            .as_ref()
            .zip(self.places[0].get(&id))
            .map(|(snapshot, &index)| &snapshot.targets[index])
    }
}

pub struct Combat {
    pub state: live::State,
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
    pub mission_layout: Option<crate::mission_layout::MissionLayout>,
    /// The other aircraft types this mission loaded, in draw order. The app
    /// keeps the drawn model of each beside them.
    dummy_types: Vec<Arc<AircraftType>>,
    dummy_configs: Vec<live::Configuration>,
    airport_objects: Vec<tore_sim::airport::StaticObject>,
    /// The records of the combat tape being written, collected until the app
    /// drains them (`take_tape`); `None` when no tape is being recorded.
    /// Combat holds no file: the app owns the writer.
    tape: Option<Vec<crate::combat_tape::Entry>>,
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

/// Lowest airborne mission start above sea level, feet (John, 2026-09-29).
pub const SPAWN_MIN_MSL_FT: f64 = 5000.;
/// Lowest airborne mission start above the ground under it, feet.
pub const SPAWN_MIN_AGL_FT: f64 = 1000.;

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
        !self.range && self.tape.is_none() && !self.clean_recording
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
    pub fn new(h: &AircraftType, data: &BTreeMap<String, Vec<u8>>, range: bool) -> AppResult<Self> {
        let config = live::Configuration::from_source(&h.profile, |name| {
            data.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("missing live-fire resource {name}")))
        })?;
        Self::configured(h, range, config, None)
    }
    pub fn with_loadout(
        h: &AircraftType,
        load: &tore_sim::combat::loadout::Loadout,
    ) -> AppResult<Self> {
        load.validate()?;
        Self::configured(
            h,
            false,
            load.configuration.clone(),
            Some(load.quantities.clone()),
        )
    }
    fn configured(
        h: &AircraftType,
        range: bool,
        config: live::Configuration,
        initial_ammo: Option<Vec<u16>>,
    ) -> AppResult<Self> {
        Ok(Self {
            contrail_offsets: h.contrail_offsets.clone(),
            contrail_sortie: 0,
            contrails: Default::default(),
            state: live::State::new(config, true)?,
            dummies: Vec::new(),
            mission_spawns: None,
            mission_layout: None,
            dummy_types: Vec::new(),
            dummy_configs: Vec::new(),
            airport_objects: Vec::new(),
            input: FireInput::default(),
            controller: FireInput::default(),
            range,
            ai_poses: false,
            clean_recording: false,
            initial_ammo,
            render: RenderHistory::default(),
            tape: None,
            last_launcher: None,
            notes: Default::default(),
        })
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
                .map(|(model, _)| self.dummy_types[*model].profile.id)
        };
        let draw = |id: u32| {
            if ownship {
                Draw::Ownship
            } else {
                model(id).map_or(Draw::Hidden, Draw::Model)
            }
        };
        let flying: BTreeMap<u32, [f64; crate::snapshot::DEVICES]> = wings
            .into_iter()
            .flat_map(|wings| wings.mission().actors())
            .filter(|actor| actor.alive())
            .map(|actor| (actor.id(), crate::snapshot::devices(actor.flight())))
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
                devices: Some(crate::snapshot::devices(player)),
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
                    blast: e.blast,
                })
                .collect(),
            marks: self
                .state
                .marks
                .iter()
                .map(|m| MarkPose::of(m, self.state.tick()))
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
            models: self.dummy_types.iter().map(|h| h.profile.id).collect(),
        }
    }
    /// once a mission's AI has placed its aircraft.
    pub fn restart_render(
        &mut self,
        player: &flight::State,
        wings: Option<&crate::ai_wings::AiWings>,
    ) {
        self.render.restart();
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
    /// An AI aircraft that flew into the ground with hit points left gets a
    /// crash site where it hit, as a shot-down wreck does when it lands.
    /// Presentation only; call after each AI step.
    pub fn ai_crashes(&mut self, wings: &crate::ai_wings::AiWings, world: &Terrain) {
        let crashed: Vec<_> = wings
            .mission()
            .actors()
            .iter()
            .filter(|actor| actor.flight().crashed && actor.flight().escape.is_none())
            .filter(|actor| {
                self.state
                    .targets
                    .iter()
                    .any(|t| t.id == actor.id() && t.hp > 0)
            })
            .map(|actor| (actor.id(), actor.flight().position))
            .collect();
        for (id, position) in crashed {
            let water = world.over_water(position[0], position[2]);
            self.state.aircraft_crashed(id, position, water);
        }
    }
    /// Developer preview (`TORE_EFFECT_PREVIEW=1`): every explosion type
    /// 15 to 38 halfway through its animation in a row 2,500 feet ahead,
    /// surface types on the ground, and a burning crash site with its
    /// column and three weapon craters on the ground 4,000 feet ahead.
    pub fn preview_effects(&mut self, s: &flight::State, world: &Terrain) {
        use tore_sim::combat::{blast, smoke};
        let (sin, cos) = s.yaw.sin_cos();
        let (forward, right) = ([sin, 0., cos], [cos, 0., -sin]);
        let at = |ahead: f64, across: f64| -> Vector {
            let x = s.position[0] + forward[0] * ahead + right[0] * across;
            let z = s.position[2] + forward[2] * ahead + right[2] * across;
            [x, f64::from(world.height(x as f32, z as f32)), z]
        };
        for kind in blast::FIRST..=blast::LAST {
            let row = blast::explosion(kind).expect("table type");
            let mut position = at(2500., (f64::from(kind) - 26.5) * 120.);
            if !row.surface {
                position[1] = s.position[1];
            }
            self.state.effects.push(live::Effect {
                position,
                kind: live::EffectKind::Destroyed,
                ticks: u16::from(row.seconds) * 60,
                blast: Some(kind),
            });
        }
        let site = at(4000., 0.);
        self.state.aircraft_crashed(u32::MAX, site, false);
        for (across, size) in [(-900., 3), (-600., 9), (900., 18)] {
            self.state.marks.push(blast::Mark {
                position: at(4000., across),
                kind: blast::MarkKind::Crater(size),
                ticks: blast::FOREVER,
                born: 0,
                serial: u64::MAX - size as u64,
            });
        }
        for _ in 0..1200 {
            self.state
                .smoke
                .step([([site[0], site[1] + 20., site[2]], smoke::Kind::Burning)]);
        }
    }
    /// The latest tick's snapshot, uninterpolated.
    pub fn render_snapshot(&self) -> &RenderSnapshot {
        &self.render.current
    }
    /// The snapshot one tick before [`Self::render_snapshot`]; none right
    /// after a restart.
    pub fn previous_snapshot(&self) -> Option<&RenderSnapshot> {
        self.render.previous.as_ref()
    }
    /// How many times the render history has started over, at a reset or a
    /// restart. A frame's tick fraction belongs to one history only.
    pub fn render_restarts(&self) -> u64 {
        self.render.restarts
    }
    /// One target in the latest snapshot.
    pub fn current_target(&self, id: u32) -> Option<&AircraftPose> {
        self.render.current_target(id)
    }
    /// One target in the snapshot before the latest.
    pub fn previous_target(&self, id: u32) -> Option<&AircraftPose> {
        self.render.previous_target(id)
    }
    /// The other aircraft types this mission loaded, in draw order.
    pub fn dummy_types(&self) -> &[Arc<AircraftType>] {
        &self.dummy_types
    }
    /// The live-fire configuration of each of [`Self::dummy_types`].
    pub fn dummy_configurations(&self) -> &[live::Configuration] {
        &self.dummy_configs
    }
    /// Where the player's aircraft's engines exhaust.
    pub fn contrail_offsets(&self) -> &[Vector] {
        &self.contrail_offsets
    }
    /// A ground object of the airport scene.
    pub fn ground_object(&self, id: u32) -> Option<&tore_sim::airport::StaticObject> {
        self.airport_objects.iter().find(|o| o.id == id)
    }
    /// Populate all six creator wings, retaining their sides for placement.
    /// `load` supplies the type of each aircraft the wings fly, the first
    /// time it is needed.
    pub fn mission_aircraft(
        &mut self,
        wings: &[tore_sim::ai::launch::WingLaunch],
        layout: &crate::mission_layout::MissionLayout,
        data: &BTreeMap<String, Vec<u8>>,
        load: &mut dyn FnMut(AircraftId) -> AppResult<Arc<AircraftType>>,
    ) -> AppResult<()> {
        self.mission_dummies(
            &tore_sim::ai::launch::legacy_pairs(wings),
            layout.enemy.distance_ft,
            data,
            load,
        )?;
        self.mission_spawns = Some(crate::ai_wings::mission_spawns(wings, &layout.spawn_plan()));
        self.mission_layout = Some(layout.clone());
        Ok(())
    }

    /// Adds straight-flight fixtures of the given aircraft. `load` supplies
    /// the type of each aircraft the first time it is needed; the caller
    /// keeps whatever draws it.
    pub fn mission_dummies(
        &mut self,
        wings: &[(AircraftId, usize)],
        separation: f64,
        data: &BTreeMap<String, Vec<u8>>,
        load: &mut dyn FnMut(AircraftId) -> AppResult<Arc<AircraftType>>,
    ) -> AppResult<()> {
        for (id, count) in wings {
            if *count == 0 {
                continue;
            }
            let model = if let Some(i) = self.dummy_types.iter().position(|h| h.profile.id == *id) {
                i
            } else {
                let h = load(*id)?;
                self.dummy_configs
                    .push(live::Configuration::from_source(&h.profile, |name| {
                        data.get(name)
                            .cloned()
                            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
                    })?);
                self.dummy_types.push(h);
                self.dummy_types.len() - 1
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
        if self.tape.is_some() {
            self.record_tape(crate::combat_tape::command_name(command), l);
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
    /// Starts collecting the combat tape's records.
    pub fn start_tape(&mut self) {
        self.tape.get_or_insert_with(Vec::new);
    }
    /// Stops collecting; records not yet taken are dropped.
    pub fn stop_tape(&mut self) {
        self.tape = None;
    }
    /// A tape is being collected.
    pub fn recording_tape(&self) -> bool {
        self.tape.is_some()
    }
    /// Adds one record to the tape, when one is being collected.
    pub fn record_tape(&mut self, action: impl Into<String>, launcher: Launcher) {
        if let Some(tape) = &mut self.tape {
            tape.push(crate::combat_tape::Entry {
                action: action.into(),
                launcher,
            });
        }
    }
    /// The records collected since the last call, oldest first. Write-only:
    /// nothing in combat reads them.
    pub fn take_tape(&mut self) -> Vec<crate::combat_tape::Entry> {
        self.tape.as_mut().map(std::mem::take).unwrap_or_default()
    }
    pub fn cancel(&mut self) {
        if self.tape.is_some()
            && let Some(l) = self.last_launcher
        {
            self.record_tape("release", l);
        }
        if self.input.held || self.controller.held {
            self.note(CommandNote::Release);
        }
        self.input.cancel();
        self.controller.cancel();
        self.state.release();
    }
    pub fn reset(&mut self, s: &mut flight::State) -> AppResult<()> {
        self.render.restart();
        self.contrails = Default::default();
        self.contrail_sortie = self.contrail_sortie.wrapping_add(1);
        let l = launcher(s);
        self.record_tape(if self.range { "reset" } else { "reset-scene" }, l);
        self.last_launcher = Some(l);
        let weapon_rules = self.state.weapon_rules;
        self.state = live::State::new(
            self.state.configuration().clone(),
            s.native.is_none() && !self.clean_recording,
        )?;
        self.state.weapon_rules = weapon_rules;
        if weapon_rules == tore_sim::combat::missiles::Rules::Compatibility {
            self.record_tape("compatibility-weapons", l);
        }
        if let Some(ammo) = &self.initial_ammo {
            self.state.ammo.clone_from(ammo);
        }
        self.state.start_load();
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
    /// Opinionated (requested by John, 2026-09-29): an airborne mission
    /// aircraft never starts inside the terrain. After [`Self::reset`] placed
    /// the wings, any airborne aircraft lower than [`SPAWN_MIN_MSL_FT`] above
    /// sea level or [`SPAWN_MIN_AGL_FT`] above the ground under it is raised
    /// to the higher of the two. Runway starts are untouched. Returns how many
    /// aircraft were raised.
    pub fn raise_airborne_spawns(&mut self, world: &Terrain) -> usize {
        let Some(spawns) = self.mission_spawns.as_ref() else {
            return 0;
        };
        let mut raised = 0;
        for (index, spawn) in spawns.iter().enumerate() {
            if spawn.runway_order.is_some() {
                continue;
            }
            let Some(target) = self.state.targets.get_mut(index) else {
                continue;
            };
            let [x, y, z] = target.position;
            let floor = SPAWN_MIN_MSL_FT
                .max(f64::from(world.height(x as f32, z as f32)) + SPAWN_MIN_AGL_FT);
            if y < floor {
                target.position[1] = floor;
                raised += 1;
            }
        }
        raised
    }
    pub fn step(&mut self, s: &mut flight::State, world: &Terrain) -> AppResult<Vec<Event>> {
        let l = launcher(s);
        self.last_launcher = Some(l);
        self.record_tape(
            if self.input.held || self.controller.held {
                "fire"
            } else {
                "tick"
            },
            l,
        );
        let mut events = Vec::new();
        self.state.note_loaded();
        if s.airburst()
            && let Some(event) = self.state.player_airburst(s.position)
        {
            events.push(event);
        }
        if s.ground_impact()
            && let Some(event) = self
                .state
                .player_ground_impact(s.position, world.over_water(s.position[0], s.position[2]))
        {
            events.push(event);
        }
        self.state.smoke.wind = world.wind();
        self.state.devices.wind = world.wind();
        self.contrails.wind = world.wind();
        // Stop wreck emissions before advancing smoke on the impact/airburst tick.
        events.extend(self.state.step_surface(
            self.input.held || self.controller.held,
            l,
            |x, z| f64::from(world.height(x as f32, z as f32)),
            |x, z| world.over_water(x, z),
        ));
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
        let scrape = s.take_belly_scrape();
        if scrape > 0. {
            self.state.scrape_damage(scrape, &mut events);
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
                } else if let Some(other) = self.dummy_types.iter().find(|h| h.profile.id == id) {
                    add(
                        target.id,
                        target.position,
                        target.basis,
                        &other.contrail_offsets,
                    );
                }
            }
        }
        self.contrails.step([]);
        self.contrails.contrails(outlets);
        s.set_payload((self.state.payload_lbs() - s.systems.used_external_lbs()).max(0.))?;
        // The bays stay shut until a release asks for them.
        s.bay_auto_open = s.bay_available() && self.state.bay_demand();
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
        // A station that ran dry hands the selection on, but never while the
        // trigger is held: the next store must not fire from the same press.
        if !(self.input.held || self.controller.held)
            && self.state.armed
            && !self.state.carries(self.state.selected)
        {
            self.command(live::Command::AdvanceFromEmpty, l);
        }
        Ok(events)
    }
}

pub(crate) fn apply_startup_weapon_state(state: &mut live::State) {
    // The gun is the startup weapon. A gun station that carries nothing falls
    // back to the first station that does, and an aircraft with nothing
    // loaded starts on NAV, so an empty station never shows up armed.
    let stations = &state.configuration().stations;
    let gun = stations
        .iter()
        .position(|station| live::is_gun(&station.weapon))
        .filter(|index| state.carries(*index));
    let choice = gun.or_else(|| {
        (0..stations.len()).find(|index| state.carries(*index) && state.station_allowed(*index))
    });
    match choice {
        Some(index) => {
            state.selected = index;
            state.armed = true;
        }
        None => state.armed = false,
    }
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
/// Where two Debug dumps first differ, with a little context from each, so a
/// failed replay comparison says which field moved.
fn first_difference(a: &str, b: &str) -> String {
    let at = a
        .bytes()
        .zip(b.bytes())
        .position(|(x, y)| x != y)
        .unwrap_or(a.len().min(b.len()));
    let window = |s: &str| {
        let from = s.floor_char_boundary(at.saturating_sub(120));
        let to = s.floor_char_boundary((at + 120).min(s.len()));
        s[from..to].to_owned()
    };
    format!(
        "at byte {at}: replay ...{}... live ...{}...",
        window(a),
        window(b)
    )
}

pub fn smoke(h: &AircraftType, data: &BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    let world = Terrain::for_theater(data, "UKR")?;
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
    // System faults are part of the check, so exercise Realistic damage.
    damaged.cheats.damage = tore_sim::cheats::Damage::Realistic;
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
    // Forty gun-sized hits kill most aircraft; the A-4E's guns are weaker, so
    // keep hitting until the aircraft is destroyed.
    for _ in 0..2000 {
        if damaged.player_hp == 0 {
            break;
        }
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
        gradual.cheats.damage = tore_sim::cheats::Damage::Realistic;
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
    // The tape file of the last slot that recorded one. A slot that stops early
    // (a surface weapon refused against the aircraft target) leaves its tape
    // open, and the records that follow go to that file until the next slot's
    // tape replaces it, as they did when combat held the file.
    let mut recorder: Option<crate::combat_tape::Recorder> = None;
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
                if combat.recording_tape() && recorder.is_some() {
                    write_tape(&mut combat, recorder.as_mut())?;
                }
                recorder = Some(crate::combat_tape::Recorder::new(
                    path,
                    data,
                    combat.state.configuration(),
                    "UKR",
                )?);
                combat.start_tape();
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
            // A surface weapon cannot engage the practice aircraft
            // (docs/spec/missiles.md, target-role rules): the shot is refused,
            // not spent, and there is no surface target to fire it at yet.
            if tore_sim::combat::missiles::Profile::for_weapon(
                &combat.state.configuration().stations[index].weapon,
            )
            .is_some_and(|p| p.role == tore_sim::combat::missiles::TargetRole::Surface)
            {
                let mut refused = combat.state.clone();
                refused.step(true, l, |_, _| 0.);
                if refused.readiness(l) != live::Readiness::WrongTarget
                    || refused.ammo[index] != initial
                    || !refused.projectiles.is_empty()
                {
                    return Err(format!(
                        "surface weapon was not refused against the aircraft target: slot={} weapon={} readiness={:?}",
                        index + 1,
                        combat.state.configuration().stations[index].weapon.source,
                        refused.readiness(l)
                    )
                    .into());
                }
                println!(
                    "combat smoke {} slot={} {} class={class}: refused against aircraft target PASS",
                    h.profile.name,
                    index + 1,
                    combat.state.configuration().stations[index].weapon.source,
                );
                combat.cancel();
                continue;
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
                    return Err(format!(
                        "source guidance probe did not launch: slot={} weapon={} readiness={:?} ammo={} initial={initial} mode={:?}",
                        index + 1,
                        weapon.source,
                        combat.state.readiness(l),
                        tracking.ammo[index],
                        tracking.launch_mode
                    )
                    .into());
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
                    // Passive channel: the switch stays on but nothing is
                    // transmitting, so a radar weapon that needs the aircraft's
                    // lock is inhibited.
                    let mut radar_off = combat.state.clone();
                    let off = Launcher { radar: false, ..l };
                    radar_off.step(true, off, |_, _| 0.);
                    let profile = tore_sim::combat::missiles::Profile::for_weapon(
                        &combat.state.configuration().stations[index].weapon,
                    );
                    let guided_by_radar = profile.is_none_or(|p| {
                        p.guidance == tore_sim::combat::missiles::Guidance::Supported
                    });
                    if guided_by_radar
                        // Losing the radar track may also drop the designation,
                        // so any reason but Ready is a valid inhibit.
                        && (radar_off.ammo[index] != initial
                            || radar_off.readiness(off) == live::Readiness::Ready)
                    {
                        return Err(format!(
                            "radar-off launch was not inhibited: slot={} weapon={} ammo={} initial={initial} readiness={:?} mode={:?}",
                            index + 1,
                            combat.state.configuration().stations[index].weapon.source,
                            radar_off.ammo[index],
                            radar_off.readiness(off),
                            radar_off.launch_mode
                        )
                        .into());
                    }
                    // Power switch off: a reviewed radar missile may still be
                    // released, permanently unguided (docs/features.md, "Uncued
                    // launch with the onboard seeker enabled").
                    if profile.is_some() {
                        let mut dumb = combat.state.clone();
                        let off = Launcher {
                            radar: false,
                            radar_power: false,
                            ..l
                        };
                        dumb.step(true, off, |_, _| 0.);
                        if dumb.ammo[index] != initial - 1
                            || dumb.projectiles.iter().any(|p| p.target.is_some())
                        {
                            return Err(format!(
                                "radar-power-off release was not an unguided shot: slot={} ammo={} initial={initial}",
                                index + 1,
                                dumb.ammo[index]
                            )
                            .into());
                        }
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
            let mut collided = false;
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
                // The host hands a dry station's selection on (see `step`);
                // the second state applies the same rule from the tape.
                if !combat.input.held && replay.armed && !replay.carries(replay.selected) {
                    replay.command(live::Command::AdvanceFromEmpty, launcher(&flight));
                }
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
                        // The fixture flies at the player, so a slow gun can
                        // lose the race and the two aircraft collide; the
                        // collision then zeroes what the rounds had not.
                        Event::PlayerDestroyed => collided = true,
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
                || (!collided
                    && combat
                        .state
                        .history
                        .iter()
                        .map(|hit| hit.applied)
                        .sum::<i32>()
                        != combat.state.configuration().hit_points - combat.state.targets[0].hp)
                || (class == 0
                    && !combat
                        .state
                        .effects
                        .iter()
                        .any(|e| e.kind == EffectKind::Destroyed))
            {
                let applied: i32 = combat.state.history.iter().map(|hit| hit.applied).sum();
                let wrong_class = combat.state.history.iter().find(|hit| hit.class != class);
                return Err(format!(
                    "combat smoke {} {} failed: fired={fired} hits={impacts} destroyed={destroyed} class={class} ammo={}->{} applied={applied} lost_hp={} wrong_class={:?}",
                    h.profile.name,
                    combat.state.configuration().stations[index].weapon.source,
                    initial,
                    combat.state.ammo[index],
                    combat.state.configuration().hit_points - combat.state.targets[0].hp,
                    wrong_class.map(|hit| (hit.class, hit.nominal, hit.applied))
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
                write_tape(&mut combat, recorder.as_mut())?;
                let decoded = crate::combat_tape::replay_without_airports(
                    path,
                    data,
                    combat.state.configuration().clone(),
                    "UKR",
                    &world,
                )?;
                let (replayed, live_state) =
                    (format!("{decoded:?}"), format!("{:?}", combat.state));
                if replayed != live_state {
                    return Err(format!(
                        "serialized live-fire replay diverged before reset: {}",
                        first_difference(&replayed, &live_state)
                    )
                    .into());
                }
                // Also replay manual state transitions, including a full reset.
                // A slow gun can lose the race with the fixture and collide, and
                // a tape does not record the host turning a crashed flight into
                // a dead player, so the manual commands start from a fresh
                // flight and a fresh combat state.
                flight = h.start(&world);
                combat.reset(&mut flight)?;
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
                write_tape(&mut combat, recorder.as_mut())?;
                let decoded = crate::combat_tape::replay_without_airports(
                    path,
                    data,
                    combat.state.configuration().clone(),
                    "UKR",
                    &world,
                )?;
                let (replayed, live_state) =
                    (format!("{decoded:?}"), format!("{:?}", combat.state));
                if replayed != live_state {
                    return Err(format!(
                        "serialized manual-command replay diverged before reset: {}",
                        first_difference(&replayed, &live_state)
                    )
                    .into());
                }
                combat.reset(&mut flight)?;
                combat.step(&mut flight, &world)?;
                write_tape(&mut combat, recorder.as_mut())?;
                combat.stop_tape();
                let decoded = crate::combat_tape::replay_without_airports(
                    path,
                    data,
                    combat.state.configuration().clone(),
                    "UKR",
                    &world,
                )?;
                let (replayed, live_state) =
                    (format!("{decoded:?}"), format!("{:?}", combat.state));
                if replayed != live_state {
                    return Err(format!(
                        "serialized combat replay diverged after commands/reset: {}",
                        first_difference(&replayed, &live_state)
                    )
                    .into());
                }
                println!("serialized combat replay {} PASS", path.display());
            }
        }
    }
    if combat.recording_tape() && recorder.is_some() {
        write_tape(&mut combat, recorder.as_mut())?;
        combat.stop_tape();
    }
    Ok(())
}

/// Writes the records combat has collected to the smoke harness's tape file
/// and flushes it.
fn write_tape(
    combat: &mut Combat,
    recorder: Option<&mut crate::combat_tape::Recorder>,
) -> AppResult<()> {
    let recorder = recorder.ok_or("missing smoke recorder")?;
    recorder.write_all(combat.take_tape());
    recorder.flush()
}

const ACQUISITION: usize = tore_sim::sensors::track::ACQUISITION_STEPS as usize;

/// Advance the shared sensors without firing, so a scripted probe designates
/// and tracks the same way a player does. It uses the recorded host step, so a
/// tape replays the same observations.
fn observe(
    combat: &mut Combat,
    flight: &mut flight::State,
    world: &Terrain,
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
    use tore_formats::aircraft::AircraftId;
    /// A player combat state with a gun and a missile, loaded as given.
    fn loaded(ammo: [u16; 2]) -> (Combat, flight::State) {
        let mut c = fixtures::combat(vec![], vec![]);
        let mut config = c.state.configuration().clone();
        config.stations[0].weapon.source = AircraftId::F18.gun().into();
        c.state = live::State::new(config, true).unwrap();
        c.initial_ammo = Some(ammo.to_vec());
        let mut f =
            flight::State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        c.reset(&mut f).unwrap();
        c.apply_startup_weapons();
        (c, f)
    }
    fn listed(c: &Combat, f: &flight::State) -> Vec<(String, u32, bool)> {
        crate::combat_view::readout(c, f, 1.).weapons
    }
    #[test]
    fn an_emptied_station_stays_empty_and_is_not_listed() {
        let (c, f) = loaded([500, 0]);
        assert_eq!(
            c.state.ammo,
            [500, 0],
            "a zero quantity is not the default load"
        );
        let list = listed(&c, &f);
        assert_eq!(list.len(), 1, "the empty missile is not carried: {list:?}");
        assert_eq!(list[0].1, 500);
    }
    #[test]
    fn a_fully_empty_aircraft_starts_on_nav_with_nothing_listed() {
        let (mut c, f) = loaded([0, 0]);
        assert_eq!(c.state.ammo, [0, 0]);
        assert!(!c.state.armed, "nothing to arm");
        assert!(listed(&c, &f).is_empty());
        // Cycling the selection has nowhere to go.
        c.state.cycle_selection(true);
        assert!(!c.state.armed);
        c.state.cycle_selection(false);
        assert!(!c.state.armed);
    }
    #[test]
    fn the_selection_ring_skips_empty_stations_and_startup_falls_back() {
        let (mut c, _) = loaded([0, 3]);
        assert!(c.state.armed);
        assert_eq!(
            c.state.selected, 1,
            "the empty gun is not selected at startup"
        );
        c.state.cycle_selection(true);
        assert!(!c.state.armed, "NAV");
        c.state.cycle_selection(true);
        assert_eq!((c.state.armed, c.state.selected), (true, 1));
    }
    #[test]
    fn a_station_emptied_in_flight_keeps_its_row_but_is_never_selectable() {
        let (mut c, f) = loaded([500, 3]);
        assert_eq!((c.state.armed, c.state.selected), (true, 0), "the gun");
        assert_eq!(listed(&c, &f).len(), 2);
        // The gun's last round is fired: the selection moves to the missile
        // and the gun keeps a row at zero, greyed by the window.
        c.state.ammo[0] = 0;
        c.state.advance_from_empty();
        assert_eq!((c.state.armed, c.state.selected), (true, 1));
        let list = listed(&c, &f);
        assert_eq!(list.len(), 2, "the dry gun is still listed: {list:?}");
        assert_eq!((list[0].1, list[0].2), (0, false));
        assert_eq!((list[1].1, list[1].2), (3, true));
        // The ring skips the dry gun.
        c.state.cycle_selection(true);
        assert!(!c.state.armed, "NAV");
        c.state.cycle_selection(true);
        assert_eq!((c.state.armed, c.state.selected), (true, 1));
        // The last missile goes too: NAV, both rows still listed.
        c.state.ammo[1] = 0;
        c.state.advance_from_empty();
        assert!(!c.state.armed);
        assert_eq!(listed(&c, &f).len(), 2);
        c.state.cycle_selection(true);
        assert!(!c.state.armed, "nothing left to select");
    }
    #[test]
    fn a_dry_station_hands_on_only_to_an_allowed_one() {
        let (mut c, _) = loaded([500, 3]);
        c.state.cheats.guns_only = true;
        c.state.ammo[0] = 0;
        c.state.advance_from_empty();
        assert!(!c.state.armed, "the missile is not allowed under guns only");
    }
    #[test]
    fn a_restart_reloads_exactly_the_edited_quantities() {
        let (mut c, mut f) = loaded([500, 0]);
        c.state.ammo.fill(7);
        c.reset(&mut f).unwrap();
        assert_eq!(c.state.ammo, [500, 0]);
    }

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
    use crate::snapshot::blend;
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
            faults: Default::default(),
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

/// Synthetic combat scenes for tests, with no art: a state with a gun and a
/// missile station, other aircraft, fixtures, weapons, effects, debris and
/// pilots, and the snapshots live flight would take of them. Drawing tests in
/// `combat_view` build on it.
#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use tore_formats::{aircraft::AircraftId, weapons::Weapon};
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
    pub(crate) fn combat(
        dummy_types: Vec<Arc<AircraftType>>,
        dummies: Vec<(usize, Vector)>,
    ) -> Combat {
        Combat {
            state: state(),
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
            dummy_types,
            dummy_configs: Vec::new(),
            airport_objects: Vec::new(),
            tape: None,
            last_launcher: None,
            notes: Default::default(),
        }
    }
    /// Synthetic types for the three other aircraft the scene draws, in the
    /// order of the drawn models.
    pub(crate) fn types() -> Vec<Arc<AircraftType>> {
        [AircraftId::F18, AircraftId::Rafale, AircraftId::F14]
            .into_iter()
            .map(|id| Arc::new(AircraftType::synthetic(id, Vec::new())))
            .collect()
    }
    /// Sets where the player's aircraft's engines exhaust.
    pub(crate) fn set_contrail_offsets(combat: &mut Combat, offsets: Vec<Vector>) {
        combat.contrail_offsets = offsets;
    }
    /// Makes two snapshots the render history.
    pub(crate) fn set_history(
        combat: &mut Combat,
        previous: RenderSnapshot,
        current: RenderSnapshot,
    ) {
        combat.render = RenderHistory::default();
        combat.render.set_current(previous);
        combat.render.advance(current);
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
            faults: Default::default(),
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
        pub(crate) previous: Vec<live::Target>,
        pub(crate) current: Vec<live::Target>,
        pub(crate) devices: Vec<(u32, [f64; 11], [f64; 11])>,
        pub(crate) projectiles: Vec<live::Projectile>,
        pub(crate) effects: Vec<live::Effect>,
        pub(crate) debris: Vec<Piece>,
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
            blast: None,
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
    /// Loads the scene as live flight would hold it: the previous and
    /// current tick's snapshots, each aircraft's devices as the AI simulated
    /// them on those ticks.
    pub(crate) fn load(combat: &mut Combat, scene: &Scene, ai_poses: bool, player: &flight::State) {
        let [previous, current] = snapshots(combat, scene, ai_poses, player);
        set_history(combat, previous, current);
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
    fn snapshots_route_each_aircraft_and_piece_to_the_model_that_draws_it() {
        use crate::snapshot::Draw;
        let player = player();
        let mut with_models = combat(types(), (0..7).map(|i| (i % 3, [0.; 3])).collect());
        let scene = scene(with_models.state.configuration());
        load(&mut with_models, &scene, true, &player);
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
        load(&mut fixture, &scene, true, &player);
        let snapshot = fixture.render_snapshot();
        assert!(snapshot.targets.iter().all(|t| t.draw == Draw::Ownship));
        assert!(snapshot.debris.iter().all(|p| p.draw == Draw::Ownship));
        assert_eq!(snapshot.player.id, 0);
    }

    #[test]
    fn devices_hold_once_nothing_simulates_the_aircraft() {
        let player = player();
        let mut combat = combat(types(), (0..7).map(|i| (i % 3, [0.; 3])).collect());
        combat.state.targets = scene(combat.state.configuration()).current;
        combat.restart_render(&player, None);
        let devices = |combat: &Combat| combat.render_snapshot().target(1).unwrap().devices;
        assert_eq!(devices(&combat), None);
        let last = [0.5; crate::snapshot::DEVICES];
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
}
