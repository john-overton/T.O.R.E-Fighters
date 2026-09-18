//! Live range host, original geometry and sampled original effect art.
use crate::{
    AppResult,
    aircraft::Airframe,
    flight,
    terrain::{Camera, World},
};
use std::collections::BTreeMap;
use tore_formats::{Pic, shape::Shape};
use tore_sim::{
    attitude::{Basis, Vector, unit},
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
    if ai_poses {
        target.basis.angles()
    } else {
        [target.velocity[0].atan2(target.velocity[2]), 0., 0.]
    }
}
/// Presentation snapshots are taken before combat advances and before the AI
/// overwrites its live poses. They never feed back into sensors or physics.
struct TargetPresentation {
    previous: BTreeMap<u32, (Vector, Basis)>,
    alpha: f64,
}
impl Default for TargetPresentation {
    fn default() -> Self {
        Self {
            previous: BTreeMap::new(),
            alpha: 1.0,
        }
    }
}
impl TargetPresentation {
    fn capture(&mut self, targets: &[live::Target], ai_poses: bool) {
        self.previous.clear();
        self.previous.extend(targets.iter().map(|t| {
            let [yaw, pitch, bank] = target_pose(t, ai_poses);
            (t.id, (t.position, Basis::new(yaw, pitch, bank)))
        }));
    }

    fn pose(&self, target: &live::Target, ai_poses: bool) -> (Vector, [f64; 3]) {
        let angles = target_pose(target, ai_poses);
        let Some((position, basis)) = self.previous.get(&target.id) else {
            return (target.position, angles);
        };
        let alpha = self.alpha.clamp(0., 1.);
        (
            std::array::from_fn(|i| position[i] + (target.position[i] - position[i]) * alpha),
            basis
                .blended(Basis::new(angles[0], angles[1], angles[2]), alpha)
                .angles(),
        )
    }
}

pub struct Combat {
    pub state: live::State,
    pub smoke_art: crate::menu::Sprite,
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
    presentation: TargetPresentation,
    dummies: Vec<(usize, Vector)>,
    mission_spawns: Option<Vec<crate::ai_wings::MissionSpawn>>,
    dummy_models: Vec<Airframe>,
    dummy_configs: Vec<live::Configuration>,
    pub recorder: Option<crate::combat_tape::Recorder>,
    last_launcher: Option<Launcher>,
    shapes: BTreeMap<String, Shape>,
    explosions: Vec<Vec<([f32; 2], [f32; 3])>>,
    ground_impacts: Vec<Vec<([f32; 2], [f32; 3])>>,
}
fn weapon_shapes(
    config: &live::Configuration,
    data: &BTreeMap<String, Vec<u8>>,
) -> BTreeMap<String, Shape> {
    config
        .stations
        .iter()
        .filter_map(|station| {
            let name = station.weapon.shape.as_ref()?;
            let shape = data
                .get(name)
                .and_then(|bytes| Shape::parse(bytes).ok())
                .filter(|shape| !shape.faces.is_empty());
            if shape.is_none() {
                eprintln!("Combat: {name} uses a tracer marker; line/point drawing remains open");
            }
            shape.map(|shape| (name.clone(), shape))
        })
        .collect()
}

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
        let shapes = weapon_shapes(&config, data);
        let pic = Pic::parse(
            data.get("AIRLRG.PIC")
                .ok_or("missing AIRLRG.PIC combat art")?,
        )?;
        if pic.width != 256 || pic.height != 232 {
            return Err("unreviewed AIRLRG frame sheet".into());
        }
        // Fitted 3x4 frame layout over visually reviewed original effect art.
        let explosions = effect_frames(&pic, &h.palette, 58);
        let impact = Pic::parse(data.get("GRDLRGA.PIC").ok_or("missing GRDLRGA.PIC")?)?;
        if impact.width != 256 || impact.height != 252 {
            return Err("unreviewed ground impact sheet".into());
        }
        let ground_impacts = effect_frames(&impact, &h.palette, 63);
        let smoke = Pic::parse(data.get("SMOKE.PIC").ok_or("missing SMOKE.PIC")?)?;
        if smoke.width != 256 || smoke.height != 43 {
            return Err("unreviewed smoke sheet dimensions".into());
        }
        let mut smoke_rgba = smoke.rgba(&h.palette);
        for (index, color) in smoke.pixels.iter().zip(smoke_rgba.chunks_exact_mut(4)) {
            if *index == 255 {
                color.fill(0);
            }
        }
        let smoke_art = crate::menu::Sprite {
            width: smoke.width,
            height: smoke.height,
            rgba: smoke_rgba,
            glyphs: smoke.glyphs,
        };
        Ok(Self {
            smoke_art,
            state: live::State::new(config, true)?,
            dummies: Vec::new(),
            mission_spawns: None,
            dummy_models: Vec::new(),
            dummy_configs: Vec::new(),
            input: FireInput::default(),
            controller: FireInput::default(),
            range,
            ai_poses: false,
            clean_recording: false,
            initial_ammo,
            presentation: TargetPresentation::default(),
            recorder: None,
            last_launcher: None,
            shapes,
            explosions,
            ground_impacts,
        })
    }
    pub fn present_targets(&mut self, alpha: f64) {
        self.presentation.alpha = alpha;
    }

    pub fn dummy_geometry(&self, camera: &Camera, world: &World) -> Vec<(&Airframe, Vec<f32>)> {
        self.dummy_models
            .iter()
            .enumerate()
            .map(|(index, model)| {
                let mut vertices = Vec::new();
                for target in self.state.targets.iter().filter(|t| t.airborne) {
                    if self
                        .dummies
                        .get(target.id.saturating_sub(1) as usize)
                        .is_none_or(|(i, _)| *i != index)
                    {
                        continue;
                    }
                    let mut pose = model.start(world);
                    let (position, angles) = self.presentation.pose(target, self.ai_poses);
                    pose.position = position;
                    pose.damage_fraction = target.damage_fraction();
                    [pose.yaw, pose.pitch, pose.bank] = angles;
                    pose.gear = 0.;
                    pose.flaps = 0.;
                    pose.exhaust = 0.;
                    pose.bay = 0.;
                    vertices.extend(model.vertices(&pose, camera, world));
                }
                for piece in self.state.debris.iter().filter(|p| {
                    p.owner > 0
                        && self
                            .dummies
                            .get(p.owner as usize - 1)
                            .is_some_and(|(i, _)| *i == index)
                }) {
                    let mut pose = model.start(world);
                    pose.position = piece.position;
                    [pose.yaw, pose.pitch, pose.bank] = piece.basis.angles();
                    vertices.extend(model.fragment_vertices(&pose, camera, world));
                }
                (model, vertices)
            })
            .collect()
    }
    /// Populate all six creator wings, retaining their sides for placement.
    pub fn mission_aircraft(
        &mut self,
        wings: &[tore_sim::ai::launch::WingLaunch],
        separation: f64,
        data: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<()> {
        self.mission_dummies(&tore_sim::ai::launch::legacy_pairs(wings), separation, data)?;
        self.mission_spawns = Some(crate::ai_wings::mission_spawns(wings, separation));
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
                self.shapes
                    .extend(weapon_shapes(self.dummy_configs.last().unwrap(), data));
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
        self.state.command(command, l);
        self.last_launcher = Some(l);
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
        self.input.cancel();
        self.controller.cancel();
        self.state.release();
    }
    pub fn reset(&mut self, s: &mut flight::State) -> AppResult<()> {
        self.presentation = TargetPresentation::default();
        let l = launcher(s);
        if let Some(r) = &mut self.recorder {
            r.record("reset", l);
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
        s.damage_fraction = 0.;
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
        Ok(())
    }
    pub fn step(&mut self, s: &mut flight::State, world: &World) -> AppResult<Vec<Event>> {
        self.presentation
            .capture(&self.state.targets, self.ai_poses);
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
        let events = self
            .state
            .step(self.input.held || self.controller.held, l, |x, z| {
                f64::from(world.height(x as f32, z as f32))
            });
        s.set_payload(self.state.payload_lbs())?;
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
        if self.state.player_hp == 0 {
            s.crashed = true;
        }
        Ok(events)
    }
    pub fn readout(&self, s: &flight::State, rcs_scale: f64) -> crate::instruments::CombatReadout {
        let i = self.state.selected;
        crate::instruments::CombatReadout {
            weapon: self.state.configuration().stations[i].weapon.name.clone(),
            guided: self.state.configuration().stations[i]
                .weapon
                .seeker
                .signature
                != 0,
            ammo: self.state.rounds(i),
            systems: format!(
                "HP{} V{} R{} E{}",
                self.state.player_hp,
                if self.state.visual_failed { "!" } else { "+" },
                if self.state.radar_failed {
                    "!"
                } else if launcher(s).radar {
                    "+"
                } else {
                    "-"
                },
                if self.state.ecm_failed {
                    "!"
                } else if launcher(s).jammer {
                    "+"
                } else {
                    "-"
                }
            ),
            readiness: self.state.readiness(launcher(s)).label(),
            damage: self
                .state
                .last_subsystem
                .map(|i| format!("SOURCE FAULT {i}"))
                .or_else(|| {
                    self.state.history.last().map(|hit| {
                        format!("C{} HIT {} HP {}", hit.class, hit.applied, hit.hp_after)
                    })
                }),
            loaded: self.range,
            target: self
                .state
                .designated()
                .and_then(|id| self.state.targets.iter().find(|t| t.id == id))
                .map(|t| (t.id, t.hp, self.state.can_lock(launcher(s)))),
            scope: crate::scope::scope(&self.state, s),
            rcs: crate::scope::rcs(&self.state, s, rcs_scale),
        }
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
                            "T{id} HP {} {}",
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
    pub fn vertices(
        &self,
        h: &Airframe,
        s: &flight::State,
        camera: &Camera,
        world: &crate::terrain::World,
    ) -> Vec<f32> {
        let mut v = Vec::new();
        for t in self.state.targets.iter().filter(|t| t.airborne) {
            let mut pose = s.clone();
            let (position, angles) = self.presentation.pose(t, self.ai_poses);
            pose.position = position;
            pose.damage_fraction = t.damage_fraction();
            [pose.yaw, pose.pitch, pose.bank] = angles;
            pose.exhaust = 0.;
            pose.gear = 0.;
            pose.flaps = 0.;
            pose.elevator = 0.;
            pose.aileron = 0.;
            pose.rudder = 0.;
            pose.brake = 0.;
            pose.hook = 0.;
            if self.dummies.is_empty() {
                v.extend(h.vertices(&pose, camera, world));
            }
        }
        for piece in self
            .state
            .debris
            .iter()
            .filter(|p| p.owner == 0 || self.dummies.is_empty())
        {
            let mut pose = s.clone();
            pose.position = piece.position;
            [pose.yaw, pose.pitch, pose.bank] = piece.basis.angles();
            v.extend(h.fragment_vertices(&pose, camera, world));
        }
        // Attached external stores are hidden until the dedicated ordnance
        // rendering pass. Loadout/flight state and launched projectiles remain
        // independent of this presentation decision in every camera.
        for p in &self.state.projectiles {
            if let Some(shape) = p
                .weapon(self.state.configuration())
                .shape
                .as_ref()
                .and_then(|name| self.shapes.get(name))
            {
                let right = unit([p.direction[2], 0., -p.direction[0]]);
                mesh(
                    &mut v,
                    shape,
                    p.position,
                    right,
                    tore_sim::attitude::cross(p.direction, right),
                    p.direction,
                    &h.palette,
                );
            }
            // A visible thin strip marks the actual swept projectile segment.
            let right = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.).right;
            let a: Vector = std::array::from_fn(|i| p.previous[i] + right[i] * 0.4);
            let b: Vector = std::array::from_fn(|i| p.previous[i] - right[i] * 0.4);
            for pos in [a, b, p.position] {
                vertex(&mut v, pos, [1., 0.8, 0.3]);
            }
        }
        let basis = Basis::new(
            f64::from(camera.yaw),
            f64::from(camera.pitch),
            -f64::from(camera.roll),
        );
        for e in &self.state.effects {
            if e.kind == EffectKind::Launch {
                continue;
            }
            if matches!(e.kind, EffectKind::Flare | EffectKind::Chaff) {
                let color = if e.kind == EffectKind::Flare {
                    [1.0, 0.8, 0.3]
                } else {
                    [0.7, 0.8, 0.9]
                };
                // Fitted presentation: a small expanding camera-facing device glint.
                let size = 2.0 + f64::from(45 - e.ticks) * 0.15;
                for [x, y] in [
                    [-1., -1.],
                    [1., -1.],
                    [1., 1.],
                    [-1., -1.],
                    [1., 1.],
                    [-1., 1.],
                ] {
                    let pos = std::array::from_fn(|i| {
                        e.position[i] + basis.right[i] * x * size + basis.up[i] * y * size
                    });
                    vertex(&mut v, pos, color);
                }
                continue;
            }
            let scale = if e.kind == EffectKind::Destroyed {
                75.
            } else {
                15.
            };
            let duration = if e.kind == EffectKind::Destroyed {
                240
            } else {
                45
            };
            let frame = (usize::from(duration - e.ticks) * 12 / usize::from(duration)).min(11);
            let frames = if e.kind == EffectKind::DebrisImpact {
                &self.ground_impacts
            } else {
                &self.explosions
            };
            for (xy, color) in &frames[frame] {
                for d in [
                    [0., 0.],
                    [1. / 20., 0.],
                    [0., 1. / 20.],
                    [0., 1. / 20.],
                    [1. / 20., 0.],
                    [1. / 20., 1. / 20.],
                ] {
                    let pos: Vector = std::array::from_fn(|i| {
                        e.position[i]
                            + basis.right[i] * f64::from(xy[0] + d[0]) * scale
                            + basis.up[i] * f64::from(xy[1] + d[1]) * scale
                    });
                    vertex(&mut v, pos, *color);
                }
            }
        }
        v
    }
}
fn effect_frames(
    pic: &Pic,
    base: &[[u8; 3]; 256],
    cell_height: usize,
) -> Vec<Vec<([f32; 2], [f32; 3])>> {
    let mut palette = *base;
    palette[..pic.palette.len()].copy_from_slice(&pic.palette);
    (0..12)
        .map(|frame| {
            let mut cells = Vec::new();
            for y in 0..20 {
                for x in 0..20 {
                    let index = pic.pixels[(frame / 3 * cell_height + y * cell_height / 20)
                        * pic.width
                        + frame % 3 * 80
                        + x * 80 / 20];
                    if index != 255 {
                        cells.push((
                            [x as f32 / 20. - 0.5, 0.5 - y as f32 / 20.],
                            palette[index as usize].map(|v| f32::from(v) / 255.),
                        ));
                    }
                }
            }
            cells
        })
        .collect()
}
fn mesh(
    out: &mut Vec<f32>,
    shape: &Shape,
    position: Vector,
    right: Vector,
    up: Vector,
    forward: Vector,
    palette: &[[u8; 3]; 256],
) {
    for face in &shape.faces {
        // These are texture-only SH faces (including the missile exhaust
        // sheets). Palette index zero is not an opaque substitute for them.
        // Keep them omitted until the weapon texture/animation path is decoded.
        if matches!(face.subtype, 0x4c | 0x5c | 0x6c | 0x7c) {
            continue;
        }
        for i in 1..face.positions.len() - 1 {
            for j in [0, i, i + 1] {
                let q = face.positions[j];
                let pos = std::array::from_fn(|k| {
                    position[k]
                        + (right[k] * f64::from(q[0])
                            + up[k] * f64::from(q[2])
                            + forward[k] * f64::from(q[1]))
                            / 3.
                });
                vertex(
                    out,
                    pos,
                    palette[face.colors[j] as usize].map(|c| f32::from(c) / 255.),
                );
                let layer = out.len() - 5;
                out[layer] = -1.;
            }
        }
    }
}
fn vertex(out: &mut Vec<f32>, pos: Vector, color: [f32; 3]) {
    // Trailing -1 opts out of the weather palette: this color is already resolved.
    out.extend([
        pos[0] as f32,
        pos[1] as f32,
        pos[2] as f32,
        0.,
        0.,
        -6., // Emissive effect; mesh() opts solid weapon bodies into lighting.
        color[0],
        color[1],
        color[2],
        -1.,
    ]);
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
                no_target.command(live::Command::ClearDesignation, l);
                no_target.step(true, l, |_, _| 0.);
                if no_target.ammo[index] != initial {
                    return Err("undesignated launch consumed ammo".into());
                }
                let weapon = &combat.state.configuration().stations[index].weapon;
                let mut too_close = combat.state.clone();
                let distance = f64::from(weapon.seeker.zones[1].minimum_range) - 1.;
                too_close.targets[0].position =
                    std::array::from_fn(|k| l.position[k] + l.basis.forward[k] * distance);
                let close_reason = too_close.readiness(l);
                too_close.step(true, l, |_, _| 0.);
                if too_close.rounds(index) != initial
                    || close_reason != live::Readiness::MinimumRange
                {
                    return Err("source minimum-range launch was not inhibited".into());
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
    #[test]
    fn palette_mesh_does_not_turn_texture_only_exhaust_into_solid_faces() {
        let face = tore_formats::shape::Face {
            fog: tore_formats::shape::FogMode::Enabled,
            positions: vec![[0., 0., 0.], [3., 0., 0.], [0., 3., 0.]],
            colors: vec![1; 3],
            uv: vec![],
            texture: "SYNTHETIC.PIC".into(),
            subtype: 0x61,
            normal: None,
            address: 0,
        };
        let mut exhaust = face.clone();
        exhaust.subtype = 0x4c;
        let shape = Shape {
            faces: vec![face, exhaust],
            state_words: Default::default(),
        };
        let mut out = vec![];
        mesh(
            &mut out,
            &shape,
            [0.; 3],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            &[[255; 3]; 256],
        );
        // One triangle of ten-float vertices; the exhaust face is omitted.
        assert_eq!(out.len(), 30);
    }
}

#[cfg(test)]
mod ai_pose_tests {
    use super::*;
    use tore_sim::{
        combat::missiles::{TargetRole, seeker::Heat},
        sensors,
    };

    fn target(velocity: Vector, basis: Basis) -> live::Target {
        live::Target {
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
            radius: 28.,
            hp: 100,
            initial_hp: 100,
            fragment_offset: [0.; 3],
            fragment_released: false,
            category: 0,
        }
    }

    #[test]
    fn formation_rendering_shares_the_camera_tick_fraction() {
        // A fixed slot must stay fixed at every render fraction, including
        // frames without a simulation tick. 800 ft/s used to produce a
        // 6.67 ft (2.03 m) sawtooth when only the camera was interpolated.
        for turning in [false, true] {
            let mut history = TargetPresentation::default();
            let mut t = target([0., 0., 800.], Basis::new(0., 0., 0.));
            let mut camera_before = [0.; 3];
            for tick in 0..240 {
                let offset = [512., 0., -512.];
                t.position = std::array::from_fn(|i| camera_before[i] + offset[i]);
                history.capture(std::slice::from_ref(&t), true);
                let heading = if turning { tick as f64 * 0.001 } else { 0. };
                let camera_after: Vector = std::array::from_fn(|i| {
                    camera_before[i] + Basis::new(heading, 0., 0.).forward[i] * 800. / 120.
                });
                t.position = std::array::from_fn(|i| camera_after[i] + offset[i]);
                t.basis = Basis::new(heading, 0.1, 0.3);
                let authoritative = t.position;
                for alpha in [0., 0.13, 0.5, 0.91, 1.] {
                    history.alpha = alpha;
                    let (position, angles) = history.pose(&t, true);
                    for i in 0..3 {
                        let camera =
                            camera_before[i] + (camera_after[i] - camera_before[i]) * alpha;
                        assert!((position[i] - camera - offset[i]).abs() < 1e-9);
                    }
                    assert!(angles.iter().all(|a| a.is_finite()));
                    assert_eq!(t.position, authoritative);
                }
                camera_before = camera_after;
            }
            history = TargetPresentation::default();
            assert_eq!(
                history.pose(&t, true).0,
                t.position,
                "restart must discard history"
            );
        }
    }

    #[test]
    fn target_presentation_blends_attitude_and_keeps_new_targets_current() {
        let mut history = TargetPresentation::default();
        let mut t = target([0., 0., 800.], Basis::new(359_f64.to_radians(), 0., 0.));
        history.capture(std::slice::from_ref(&t), true);
        t.basis = Basis::new(1_f64.to_radians(), 0., 0.);
        history.alpha = 0.5;
        let (_, angles) = history.pose(&t, true);
        assert!(
            angles[0].sin().abs() < 1e-9,
            "heading must take the short path"
        );
        t.id = 2;
        t.position = [100.; 3];
        assert_eq!(history.pose(&t, true), (t.position, t.basis.angles()));
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
