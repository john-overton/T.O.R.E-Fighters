//! Explicit development live-fire adapter. Source configuration and recovered scalar
//! kernels are combined with authored scheduling, guidance and swept-sphere contacts.
//! This is NOT the diagnostic native-parity update or a retail AI implementation.
use super::{
    EnginePhase, FallState, PlayerTrigger, axial_speed, commanded_speed, engine_phase,
    launch_speed, removal_due, unload,
};
use crate::attitude::{Basis, Vector, dot, unit};
use tore_formats::{
    Result,
    aircraft::{Aircraft, AircraftId},
    weapons::Weapon,
};

pub const MAX_PROJECTILES: usize = 256;
pub const MAX_EFFECTS: usize = 64;
pub const MAX_HIT_RECORDS: usize = 128;

/// Exact category switch at FA 0x411470; category is not a bitmask here.
pub fn damage_class(category: u16) -> usize {
    match category {
        0x40 | 0x200 | 0x800 | 0x1000 => 4,
        0x100 => 2,
        0x400 => 3,
        0x2000 => 1,
        _ => 0,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Readiness {
    Ready,
    Safe,
    LauncherLost,
    StationFailed,
    Empty,
    Capacity,
    NoTarget,
    TargetDestroyed,
    RadarOff,
    RadarCoverage,
    TerrainMasked,
    MinimumRange,
    MaximumRange,
    Altitude,
    FieldOfView,
}
impl Readiness {
    pub fn label(self) -> &'static str {
        match self {
            Self::Ready => "READY",
            Self::Safe => "SAFE",
            Self::LauncherLost => "LAUNCHER LOST",
            Self::StationFailed => "STATION FAILED",
            Self::Empty => "EMPTY",
            Self::Capacity => "PROJECTILE LIMIT",
            Self::NoTarget => "NO TARGET",
            Self::TargetDestroyed => "TARGET DESTROYED",
            Self::RadarOff => "RADAR OFF",
            Self::RadarCoverage => "RADAR COVERAGE",
            Self::TerrainMasked => "TERRAIN MASKED",
            Self::MinimumRange => "MIN RANGE",
            Self::MaximumRange => "MAX RANGE",
            Self::Altitude => "ALTITUDE LIMIT",
            Self::FieldOfView => "SEEKER FOV",
        }
    }
}

/// Authored manual-range commands. Apply at tick boundaries for deterministic replay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    NextWeapon,
    Designate,
    ClearDesignation,
    ToggleArm,
    Jettison,
    ReplaceTarget,
    CycleClass,
    FailStation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HitRecord {
    pub tick: u64,
    pub target: u32,
    pub station: usize,
    pub class: usize,
    pub nominal: i32,
    pub applied: i32,
    pub hp_after: i32,
}
#[derive(Clone, Debug)]
pub struct Station {
    pub weapon: Weapon,
    pub mount: Vector,
    pub count: u16,
    pub internal: bool,
}
#[derive(Clone, Debug)]
pub struct Configuration {
    pub aircraft: AircraftId,
    pub stations: Vec<Station>,
    pub hit_points: i32,
    pub target_category: u16,
    pub external_equipment_lbs: i32,
    pub radar: tore_formats::weapons::Seeker,
    pub visual: tore_formats::weapons::Seeker,
}
impl Configuration {
    fn validate(&self) -> Result<()> {
        if self.stations.is_empty()
            || self.stations.len() > 32
            || self.hit_points <= 0
            || self.external_equipment_lbs < 0
        {
            return Err(super::invalid("invalid live configuration bounds"));
        }
        for s in &self.stations {
            let m = &s.weapon.movement;
            if s.count == 0
                || s.count >= 32767
                || s.mount.iter().any(|v| !v.is_finite())
                || m.minimum_speed < 0
                || m.maximum_speed < m.minimum_speed
                || !(0..=i32::MAX / 256).contains(&m.acceleration)
                || !(0..=i32::MAX / 256).contains(&m.deceleration)
                || s.weapon.burst.actual_rounds_per_game == 0
            {
                return Err(super::invalid("invalid live station bounds"));
            }
        }
        Ok(())
    }
    pub fn from_source(
        a: &Aircraft,
        mut read: impl FnMut(&str) -> Result<Vec<u8>>,
    ) -> Result<Self> {
        let mut stations = Vec::new();
        let mut external_equipment_lbs = 0i32;
        for h in a.hardpoints.iter().filter(|h| h.flags & 8 == 0) {
            if let Some(name) = h.store.as_deref() {
                let weight = if name.ends_with(".GAS") {
                    let tank = tore_formats::weapons::Tank::parse(&read(name)?)?;
                    i32::from(tank.empty_weight).checked_add(tank.fuel_weight)
                } else if name.ends_with(".SEE") {
                    let e = tore_formats::aircraft::Equipment::parse(name, &read(name)?)?;
                    Some(
                        e.fields
                            .get("weight")
                            .ok_or_else(|| super::invalid("missing equipment weight"))?
                            .number()?,
                    )
                } else {
                    Some(0)
                }
                .ok_or_else(|| super::invalid("external equipment mass overflow"))?;
                if weight < 0 {
                    return Err(super::invalid("negative external equipment weight"));
                }
                external_equipment_lbs = external_equipment_lbs
                    .checked_add(
                        weight
                            .checked_mul(h.count)
                            .ok_or_else(|| super::invalid("external mass overflow"))?,
                    )
                    .ok_or_else(|| super::invalid("external mass overflow"))?;
            }
        }
        for h in &a.hardpoints {
            let Some(name) = h.store.as_deref().filter(|n| n.ends_with(".JT")) else {
                continue;
            };
            let weapon = Weapon::parse(name, &read(name)?)?;
            // Restrict the live adapter to the actual default stations of the
            // two reviewed aircraft. Catalog import never makes another type flyable.
            let permitted = match a.id {
                AircraftId::F18 => ["M61.JT", "AIM120.JT", "AGM65G.JT", "AIM9M.JT"].contains(&name),
                AircraftId::Rafale => {
                    ["DEFA.JT", "AGM65G.JT", "MICA.JT", "R530.JT", "R550.JT"].contains(&name)
                }
            };
            if weapon.movement.acceleration > i32::MAX / 256
                || weapon.movement.deceleration > i32::MAX / 256
            {
                return Err(super::invalid("live acceleration exceeds fixed8 domain"));
            }
            if !permitted
                || h.count <= 0
                || h.count > 32766
                || weapon.burst.actual_rounds_per_game == 0
            {
                return Err(super::invalid("unreviewed live-fire station"));
            }
            stations.push(Station {
                weapon,
                mount: h.position.map(|v| f64::from(v) / 3.),
                count: h.count as u16,
                internal: h.flags & 8 != 0,
            });
        }
        let hit_points = a
            .object
            .get("hitPoints")
            .ok_or_else(|| super::invalid("missing aircraft hit points"))?
            .number()?;
        if hit_points <= 0 || stations.is_empty() || !stations[0].internal {
            return Err(super::invalid("invalid live-fire aircraft configuration"));
        }
        let radar_name = a
            .hardpoints
            .iter()
            .filter_map(|h| h.store.as_deref())
            .find(|n| *n == "F18R.SEE")
            .ok_or_else(|| super::invalid("missing reviewed radar station"))?;
        let radar = tore_formats::weapons::Seeker::parse(radar_name, &read(radar_name)?)?;
        let visual_name = a
            .hardpoints
            .iter()
            .filter_map(|h| h.store.as_deref())
            .find(|n| *n == "VIS340.SEE")
            .ok_or_else(|| super::invalid("missing reviewed visual sensor"))?;
        let visual = tore_formats::weapons::Seeker::parse(visual_name, &read(visual_name)?)?;
        Ok(Self {
            radar,
            visual,
            external_equipment_lbs,
            aircraft: a.id,
            stations,
            hit_points,
            target_category: a
                .object
                .get("obj_class")
                .ok_or_else(|| super::invalid("missing object category"))?
                .number()? as u16,
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub id: u32,
    pub position: Vector,
    pub velocity: Vector,
    pub radius: f64,
    pub hp: i32,
    pub category: u16,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Projectile {
    pub station: usize,
    pub position: Vector,
    pub previous: Vector,
    pub direction: Vector,
    pub speed_f8: i32,
    pub launched_t: u16,
    pub target: Option<u32>,
    pub fall: FallState,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EffectKind {
    Launch,
    Hit,
    Destroyed,
    Ground,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Effect {
    pub position: Vector,
    pub kind: EffectKind,
    pub ticks: u16,
}
#[derive(Clone, Debug, PartialEq)]
pub enum Event {
    Fired(usize),
    Hit(u32),
    Destroyed(u32),
    Ground,
    TrackLost(u32),
}
#[derive(Clone, Debug)]
pub struct State {
    config: Configuration,
    pub ammo: Vec<u16>,
    pub selected: usize,
    pub designated: Option<u32>,
    pub projectiles: Vec<Projectile>,
    pub targets: Vec<Target>,
    pub effects: Vec<Effect>,
    pub shots: u32,
    pub hits: u32,
    pub kills: u32,
    pub armed: bool,
    pub history: Vec<HitRecord>,
    pub range_category: u16,
    next_target_id: u32,
    masked_targets: Vec<u32>,
    external: bool,
    tick: u64,
    service_remainder: u16,
    triggers: Vec<PlayerTrigger>,
}
#[derive(Clone, Copy)]
pub struct Launcher {
    pub position: Vector,
    pub basis: Basis,
    pub speed_fps: f64,
    pub radar: bool,
    pub alive: bool,
}
impl State {
    pub fn configuration(&self) -> &Configuration {
        &self.config
    }
    pub fn new(config: Configuration, external: bool) -> Result<Self> {
        config.validate()?;
        let ammo = config
            .stations
            .iter()
            .map(|s| if s.internal || external { s.count } else { 0 })
            .collect();
        let triggers = vec![PlayerTrigger::default(); config.stations.len()];
        let range_category = config.target_category;
        Ok(Self {
            external,
            armed: true,
            history: vec![],
            range_category,
            next_target_id: 1,
            masked_targets: vec![],
            config,
            ammo,
            selected: 0,
            designated: None,
            projectiles: vec![],
            targets: vec![],
            effects: vec![],
            shots: 0,
            hits: 0,
            kills: 0,
            tick: 0,
            service_remainder: 0,
            triggers,
        })
    }
    pub fn release(&mut self) {
        for t in &mut self.triggers {
            t.release();
        }
    }
    pub fn select_next(&mut self) {
        self.release();
        self.selected = (self.selected + 1) % self.ammo.len();
    }
    pub fn designate_next(&mut self, launcher: Launcher) {
        self.designated = self
            .targets
            .iter()
            .filter(|t| self.detects(launcher, t))
            .find(|t| Some(t.id) > self.designated)
            .or_else(|| self.targets.iter().find(|t| self.detects(launcher, t)))
            .map(|t| t.id);
    }
    pub fn command(&mut self, command: Command, launcher: Launcher) {
        match command {
            Command::NextWeapon => self.select_next(),
            Command::Designate => self.designate_next(launcher),
            Command::ClearDesignation => self.designated = None,
            Command::ToggleArm => {
                self.armed = !self.armed;
                self.release();
            }
            Command::Jettison => {
                if !self.config.stations[self.selected].internal {
                    self.ammo[self.selected] = 0;
                    self.release();
                }
            }
            Command::ReplaceTarget => self.range_target(launcher),
            Command::CycleClass => {
                self.range_category =
                    [0x80, 0x2000, 0x100, 0x400, 0x40][(damage_class(self.range_category) + 1) % 5];
                self.range_target(launcher);
            }
            // Native equipment damage marks the station's high bit. Selecting
            // the failure manually is a test fixture, not a recovered damage roll.
            Command::FailStation => {
                self.ammo[self.selected] |= 0x8000;
                self.release();
            }
        }
    }
    pub fn rounds(&self, station: usize) -> u16 {
        self.ammo[station] & 0x7fff
    }
    pub fn readiness(&self, launcher: Launcher) -> Readiness {
        if !launcher.alive {
            return Readiness::LauncherLost;
        }
        if !self.armed {
            return Readiness::Safe;
        }
        if self.ammo[self.selected] & 0x8000 != 0 {
            return Readiness::StationFailed;
        }
        if self.rounds(self.selected) == 0 {
            return Readiness::Empty;
        }
        if self.projectiles.len() >= MAX_PROJECTILES {
            return Readiness::Capacity;
        }
        self.launch_solution(launcher)
    }
    fn launch_solution(&self, launcher: Launcher) -> Readiness {
        let w = &self.config.stations[self.selected].weapon;
        if w.seeker.signature == 0 {
            return Readiness::Ready;
        }
        let Some(t) = self
            .designated
            .and_then(|id| self.targets.iter().find(|t| t.id == id))
        else {
            return Readiness::NoTarget;
        };
        if t.hp <= 0 {
            return Readiness::TargetDestroyed;
        }
        if self.masked_targets.contains(&t.id) {
            return Readiness::TerrainMasked;
        }
        if w.seeker.signature == 3 {
            if !launcher.radar {
                return Readiness::RadarOff;
            }
            if !self.radar_detects(launcher, t.position) {
                return Readiness::RadarCoverage;
            }
        }
        zone_readiness(
            &w.seeker.zones[1],
            launcher.position,
            launcher.basis.forward,
            t.position,
        )
    }
    pub fn payload_lbs(&self) -> f64 {
        f64::from(if self.external {
            self.config.external_equipment_lbs
        } else {
            0
        }) + self
            .config
            .stations
            .iter()
            .zip(&self.ammo)
            .filter(|(s, _)| !s.internal)
            .map(|(s, count)| f64::from(s.weapon.weight.max(0)) * f64::from(*count & 0x7fff))
            .sum::<f64>()
    }
    /// An explicit, non-AI range target of the selected ported aircraft. No
    /// targets are inserted into ordinary free flight or fabricated on scopes.
    pub fn range_target(&mut self, launcher: Launcher) {
        let w = &self.config.stations[self.selected].weapon;
        let distance = if w.seeker.signature == 0 {
            900.
        } else {
            f64::from(w.seeker.zones[1].minimum_range) + 3000.
        };
        // Retire the previous engagement atomically. Never let an old missile
        // hit or track a replacement fixture with a reused identity.
        self.release();
        self.projectiles.clear();
        self.effects.clear();
        self.targets.clear();
        self.masked_targets.clear();
        let id = self.next_target_id;
        self.next_target_id = self
            .next_target_id
            .checked_add(1)
            .expect("range ID exhaustion");
        self.targets.push(Target {
            id,
            category: self.range_category,
            position: std::array::from_fn(|i| {
                launcher.position[i] + launcher.basis.forward[i] * distance
            }),
            velocity: launcher.basis.forward.map(|v| v * 300.),
            radius: 28.,
            hp: self.config.hit_points,
        });
        self.designated = None;
    }
    pub fn detects(&self, launcher: Launcher, target: &Target) -> bool {
        target.hp > 0
            && !self.masked_targets.contains(&target.id)
            && (self.radar_detects(launcher, target.position)
                || cone(
                    &self.config.visual.zones[0],
                    launcher.position,
                    launcher.basis.forward,
                    target.position,
                ))
    }
    pub fn radar_detects(&self, launcher: Launcher, target: Vector) -> bool {
        !self
            .targets
            .iter()
            .any(|t| t.position == target && self.masked_targets.contains(&t.id))
            && launcher.radar
            && cone(
                &self.config.radar.zones[0],
                launcher.position,
                launcher.basis.forward,
                target,
            )
    }
    pub fn can_lock(&self, launcher: Launcher) -> bool {
        self.config.stations[self.selected].weapon.seeker.signature != 0
            && self.launch_solution(launcher) == Readiness::Ready
    }

    fn effect(&mut self, position: Vector, kind: EffectKind) {
        if self.effects.len() == MAX_EFFECTS {
            self.effects.remove(0);
        }
        self.effects.push(Effect {
            position,
            kind,
            ticks: if kind == EffectKind::Destroyed {
                240
            } else {
                45
            },
        });
    }
    /// Exactly one host 120 Hz tick. Pausing means NOT calling this method.
    /// The host-to-native time conversion and stage ordering are authored here.
    pub fn step(
        &mut self,
        held: bool,
        launcher: Launcher,
        ground: impl Fn(f64, f64) -> f64,
    ) -> Vec<Event> {
        let mut events = Vec::new();
        let now = (self.tick / 30) as u16;
        self.tick += 1;
        self.service_remainder += 256;
        let service = (self.service_remainder / 120) as i16;
        self.service_remainder %= 120;
        for e in &mut self.effects {
            e.ticks = e.ticks.saturating_sub(1);
        }
        self.effects.retain(|e| e.ticks > 0);
        // Authored bounded terrain line-of-sight test shared by acquisition
        // and scope contacts. Native masking/cadence remains unverified.
        self.masked_targets = self
            .targets
            .iter()
            .filter(|t| t.hp > 0 && terrain_hit(launcher.position, t.position, &ground).is_some())
            .map(|t| t.id)
            .collect();
        let index = self.selected;
        let allowed = self.readiness(launcher) == Readiness::Ready;
        let station = &self.config.stations[index];
        let w = &station.weapon;
        let guided = w.seeker.signature != 0;
        let due =
            self.triggers[index].poll(held && launcher.alive, w.flags, w.burst.game_burst_t, now);
        if due && allowed {
            // Representative burst grouping is provisional. Debit source rounds
            // per representative projectile, including a partial last debit.
            let count = usize::from(w.burst.game_rounds_in_burst.max(1)).min(32);
            for _ in 0..count {
                if self.projectiles.len() == MAX_PROJECTILES
                    || !unload(
                        &mut self.ammo[index],
                        u16::from(w.burst.actual_rounds_per_game),
                    )
                {
                    break;
                }
                let position = std::array::from_fn(|i| {
                    launcher.position[i]
                        + launcher.basis.right[i] * station.mount[0]
                        + launcher.basis.up[i] * station.mount[1]
                        + launcher.basis.forward[i] * station.mount[2]
                });
                self.projectiles.push(Projectile {
                    station: index,
                    position,
                    previous: position,
                    direction: launcher.basis.forward,
                    speed_f8: launch_speed(&w.movement, (launcher.speed_fps * 256.) as i32)
                        .expect("validated speed limits")
                        * 256,
                    launched_t: now,
                    target: if guided { self.designated } else { None },
                    fall: FallState::default(),
                });
                self.shots += 1;
                events.push(Event::Fired(index));
            }
        }
        if events.iter().any(|e| matches!(e, Event::Fired(_))) {
            self.effect(launcher.position, EffectKind::Launch);
        }
        // Targets use a deliberately explicit scripted flight profile, not AI.
        let old_targets: Vec<_> = self.targets.iter().map(|t| t.position).collect();
        for t in &mut self.targets {
            if t.hp > 0 {
                for i in 0..3 {
                    t.position[i] += t.velocity[i] / 120.;
                }
            }
        }
        let mut impacts = Vec::new();
        self.projectiles.retain_mut(|p| {
            let w = &self.config.stations[p.station].weapon;
            let m = &w.movement;
            if removal_due(m, now, p.launched_t, (p.position[1] * 256.) as i32) {
                return false;
            }
            p.previous = p.position;
            let phase = engine_phase(m, now, p.launched_t);
            if let Some(t) = p
                .target
                .and_then(|id| self.targets.iter().find(|t| t.id == id && t.hp > 0))
            {
                if acquisition(w, p.position, p.direction, t.position, launcher.radar, 0)
                    && terrain_hit(p.position, t.position, &ground).is_none()
                {
                    let desired = unit(sub(t.position, p.position));
                    let rate = if phase == EnginePhase::Powered {
                        m.powered_turn_rate
                    } else {
                        m.unpowered_turn_rate
                    };
                    // Authored pursuit, capped by source angle-rate field; native
                    // PN, lead, sun, Doppler, ECM and RNG contracts remain open.
                    let angle = dot(p.direction, desired).clamp(-1., 1.).acos();
                    let fraction = (f64::from(rate.max(0)) * std::f64::consts::TAU
                        / 65520.
                        / 120.
                        / angle.max(1e-9))
                    .min(1.);
                    p.direction = unit(std::array::from_fn(|i| {
                        p.direction[i] * (1. - fraction) + desired[i] * fraction
                    }));
                } else {
                    events.push(Event::TrackLost(t.id));
                    p.target = None;
                }
            } else if let Some(id) = p.target.take() {
                events.push(Event::TrackLost(id));
            }
            if w.flags & 0x40 != 0 {
                let target =
                    commanded_speed(m, phase, p.speed_f8, (p.position[1] * 256.) as i32) as i16;
                p.speed_f8 =
                    axial_speed(m, p.speed_f8, target, false, service).expect("validated movement");
            }
            let distance = f64::from(p.speed_f8) * f64::from(service) / 65536.;
            for i in 0..3 {
                p.position[i] += p.direction[i] * distance;
            }
            p.position[1] = f64::from(
                p.fall
                    .advance(
                        w.flags & 4 != 0,
                        phase,
                        service,
                        (p.position[1] * 256.) as i32,
                    )
                    .expect("positive service"),
            ) / 256.;
            let armed = now.wrapping_sub(p.launched_t) >= w.damage.fuze_arm_t;
            let mut first: Option<(f64, Option<usize>)> = None;
            if armed {
                for (i, t) in self.targets.iter().enumerate().filter(|(_, t)| t.hp > 0) {
                    let radius = t.radius + f64::from(w.damage.fuze_radius.max(0));
                    if let Some(at) = segment_sphere(
                        sub(p.previous, old_targets[i]),
                        sub(p.position, t.position),
                        radius,
                    ) && first.is_none_or(|f| at < f.0)
                    {
                        first = Some((at, Some(i)));
                    }
                }
            }
            // Sample the whole segment, then bisect the first crossing; bounded
            // contact approximation prevents fast rounds tunneling through terrain.
            if let Some(at) = terrain_hit(p.previous, p.position, &ground)
                && first.is_none_or(|f| at < f.0)
            {
                first = Some((at, None));
            }
            if let Some((at, target)) = first {
                let position =
                    std::array::from_fn(|i| p.previous[i] + (p.position[i] - p.previous[i]) * at);
                if let Some(i) = target {
                    let t = &mut self.targets[i];
                    let class = damage_class(t.category);
                    let nominal = i32::from(w.damage.by_class[class]).max(0);
                    let applied = nominal.min(t.hp);
                    t.hp -= applied;
                    if self.history.len() == MAX_HIT_RECORDS {
                        self.history.remove(0);
                    }
                    self.history.push(HitRecord {
                        tick: self.tick,
                        target: t.id,
                        station: p.station,
                        class,
                        nominal,
                        applied,
                        hp_after: t.hp,
                    });
                    self.hits += 1;
                    events.push(Event::Hit(t.id));
                    if t.hp == 0 {
                        self.kills += 1;
                        events.push(Event::Destroyed(t.id));
                    }
                    impacts.push((
                        position,
                        if t.hp == 0 {
                            EffectKind::Destroyed
                        } else {
                            EffectKind::Hit
                        },
                    ));
                } else {
                    events.push(Event::Ground);
                    impacts.push((position, EffectKind::Ground));
                }
                return false;
            }
            true
        });
        for (p, kind) in impacts {
            self.effect(p, kind);
        }
        events
    }
}
fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}
fn acquisition(
    w: &Weapon,
    position: Vector,
    forward: Vector,
    target: Vector,
    radar: bool,
    zone: usize,
) -> bool {
    // PROJLock checks launcher illumination under flag 0x200. The reviewed
    // default radar stores all require radar at launch; only R530 has 0x200.
    if w.seeker.signature == 3 && !radar && (zone == 1 || w.flags & 0x200 != 0) {
        return false;
    }
    cone(&w.seeker.zones[zone], position, forward, target)
}
fn cone(
    z: &tore_formats::weapons::Zone,
    position: Vector,
    forward: Vector,
    target: Vector,
) -> bool {
    zone_readiness(z, position, forward, target) == Readiness::Ready
}
fn zone_readiness(
    z: &tore_formats::weapons::Zone,
    position: Vector,
    forward: Vector,
    target: Vector,
) -> Readiness {
    let d = sub(target, position);
    let distance = dot(d, d).sqrt();
    let angle = dot(unit(d), forward).clamp(-1., 1.).acos();
    if distance < f64::from(z.minimum_range) {
        Readiness::MinimumRange
    } else if distance > f64::from(z.maximum_range) {
        Readiness::MaximumRange
    } else if d[1] < f64::from(z.minimum_altitude) || d[1] > f64::from(z.maximum_altitude) {
        Readiness::Altitude
    } else if angle > f64::from(z.heading.min(z.pitch).max(0)) * std::f64::consts::TAU / 65520. {
        Readiness::FieldOfView
    } else {
        Readiness::Ready
    }
}

pub fn segment_sphere(a: Vector, b: Vector, radius: f64) -> Option<f64> {
    let d = sub(b, a);
    let c = dot(a, a) - radius * radius;
    if c <= 0. {
        return Some(0.);
    }
    let aa = dot(d, d);
    let bb = dot(a, d);
    let disc = bb * bb - aa * c;
    if aa <= 1e-15 || disc < 0. {
        return None;
    }
    let t = (-bb - disc.sqrt()) / aa;
    (0. ..=1.).contains(&t).then_some(t)
}
fn terrain_hit(a: Vector, b: Vector, ground: &impl Fn(f64, f64) -> f64) -> Option<f64> {
    let below = |t: f64| {
        let p: Vector = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
        p[1] <= ground(p[0], p[2])
    };
    if below(0.) {
        return Some(0.);
    }
    for i in 1..=8 {
        let mut high = f64::from(i) / 8.;
        if below(high) {
            let mut low = f64::from(i - 1) / 8.;
            for _ in 0..10 {
                let mid = (low + high) * 0.5;
                if below(mid) {
                    high = mid;
                } else {
                    low = mid;
                }
            }
            return Some(high);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::weapons::*;
    fn fixture(guided: bool) -> State {
        let zone = Zone {
            heading: 12000,
            pitch: 12000,
            minimum_range: 0,
            maximum_range: 10000,
            minimum_altitude: i32::MIN,
            maximum_altitude: i32::MAX,
        };
        let seeker = Seeker {
            flags: [0; 2],
            signature: if guided { 3 } else { 0 },
            look_down: 0,
            doppler_above: 0,
            doppler_below: 0,
            doppler_minimum_range: 0,
            all_aspect: 0,
            zones: [zone; 2],
            chaff_flare_chance: 0,
            deception_chance: 0,
        };
        let w = Weapon {
            source: "SYNTHETIC.JT".into(),
            name: "Synthetic".into(),
            shape: None,
            fire_sound: None,
            native_callback: "_PROJProc".into(),
            flags: if guided { 0x240 } else { 0x844 },
            object_flags: 0,
            weight: 10,
            movement: Movement {
                minimum_speed: 10,
                corner_speed: 1000,
                maximum_speed: 2000,
                acceleration: 100,
                deceleration: 2,
                initial_speed: 1000,
                final_speed: 500,
                launch_retard: 100,
                ignite_t: 0,
                fuel_t: 10,
                remove_t: 20,
                powered_turn_rate: 10000,
                unpowered_turn_rate: 10000,
                performance_at_0: 100,
                performance_at_20: 100,
                cruise: [0; 4],
                jink: [0; 3],
            },
            burst: Burst {
                projectiles_in_pod: 1,
                actual_rounds_per_game: 2,
                game_rounds_in_burst: 1,
                game_rounds_in_carpet_burst: 1,
                game_burst_t: 1,
                reload_t: 0,
                startup_shots: 0,
                random_fire_percent: 0,
                offset_fire_percent: 0,
                offset_fire_heading: 0,
                offset_fire_pitch: 0,
                sine_pattern: [0; 4],
            },
            seeker,
            guidance: Guidance {
                track_t: 1,
                track_max_g_raw: 1,
                target_sun_chance: 0,
                max_aon: 0,
                chances: [100; 4],
                hit_modifiers: [0; 9],
            },
            damage: Damage {
                by_class: [10; 5],
                fuze_arm_t: 0,
                fuze_radius: 0,
                side_hit_fuze_failure: 0,
                collateral_radius: 0,
                collateral_percent: 0,
            },
            effects: Effects {
                object_explosion: 0,
                land_explosion: 0,
                water_explosion: 0,
                crater_size: 0,
                smoke: [0; 5],
                max_sound_distance: 0,
                frequency_adjustment: 0,
            },
        };
        State::new(
            Configuration {
                aircraft: AircraftId::F18,
                stations: vec![Station {
                    weapon: w,
                    mount: [0.; 3],
                    count: 11,
                    internal: !guided,
                }],
                hit_points: 20,
                target_category: 0x80,
                external_equipment_lbs: 0,
                radar: seeker,
                visual: seeker,
            },
            true,
        )
        .unwrap()
    }
    fn launcher() -> Launcher {
        Launcher {
            position: [0., 1000., 0.],
            basis: Basis::new(0., 0., 0.),
            speed_fps: 300.,
            radar: true,
            alive: true,
        }
    }
    #[test]
    fn swept_contact_handles_tunneling_moving_targets_and_nearest_root() {
        assert_eq!(
            segment_sphere([0., 0., -100.], [0., 0., 100.], 10.),
            Some(0.45)
        );
        assert_eq!(segment_sphere([0.; 3], [0.; 3], 1.), Some(0.));
        assert_eq!(segment_sphere([20., 0., -100.], [20., 0., 100.], 10.), None);
        assert_eq!(
            terrain_hit([0., 10., 0.], [0., -10., 100.], &|_, _| 0.),
            Some(0.5)
        );
    }
    #[test]
    fn source_debit_partial_last_round_empty_release_and_expiry() {
        let mut s = fixture(false);
        for _ in 0..300 {
            s.step(true, launcher(), |_, _| 0.);
        }
        assert_eq!(s.ammo, [0]);
        assert_eq!(s.shots, 6);
        for _ in 0..600 {
            s.step(false, launcher(), |_, _| 0.);
        }
        assert!(s.projectiles.is_empty());
        assert!(s.effects.is_empty());
        let mut s = fixture(false);
        s.step(true, launcher(), |_, _| 0.);
        s.release();
        for _ in 0..60 {
            s.step(false, launcher(), |_, _| 0.);
        }
        assert_eq!(s.shots, 1);
    }
    #[test]
    fn detection_launch_and_inflight_lock_loss_are_distinct() {
        let mut s = fixture(true);
        let mut l = launcher();
        s.range_target(l);
        s.designate_next(launcher());
        l.radar = false;
        assert!(!s.radar_detects(l, s.targets[0].position));
        assert!(!s.can_lock(l));
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.ammo, [11]);
        s.step(false, l, |_, _| 0.);
        l.radar = true;
        assert!(s.can_lock(l));
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.ammo, [9]);
        assert_eq!(s.projectiles[0].target, Some(1));
        l.radar = false;
        s.step(false, l, |_, _| 0.);
        assert_eq!(s.projectiles[0].target, None);
        assert!(s.projectiles[0].position[2] > 0.);
    }
    #[test]
    fn actual_target_damage_destroys_once_and_generates_effects() {
        let mut s = fixture(false);
        s.targets.push(Target {
            id: 7,
            position: [0., 1000., 150.],
            velocity: [0.; 3],
            radius: 20.,
            hp: 20,
            category: 0x80,
        });
        let mut kills = 0;
        for _ in 0..180 {
            for e in s.step(true, launcher(), |_, _| 0.) {
                if matches!(e, Event::Destroyed(7)) {
                    kills += 1;
                }
            }
        }
        assert_eq!((s.hits, s.kills, kills, s.targets[0].hp), (2, 1, 1, 0));
        assert!(s.effects.iter().any(|e| e.kind == EffectKind::Destroyed));
    }
    #[test]
    fn fixed_ticks_replay_across_presentation_rates_and_pause() {
        let run = |fps: usize| {
            let mut s = fixture(false);
            let mut clock = crate::flight::Clock { remainder: 0. };
            let mut tick = 0;
            for frame in 0..fps * 4 {
                // No call into either clock or combat while paused. Resume has
                // no elapsed wall-time backlog. Inputs are indexed by sim tick.
                if frame >= fps && frame < fps * 2 {
                    continue;
                }
                for _ in 0..clock.steps(1. / fps as f64) {
                    s.step(tick < 90, launcher(), |_, _| 0.);
                    tick += 1;
                }
            }
            (tick, s.ammo, s.projectiles, s.targets, s.effects, s.shots)
        };
        assert_eq!(run(30), run(60));
        assert_eq!(run(60), run(144));
    }
    #[test]
    fn capacity_failure_does_not_debit_and_dead_launcher_cannot_fire() {
        let mut s = fixture(false);
        let mut l = launcher();
        l.alive = false;
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.shots, 0);
        l.alive = true;
        s.step(true, l, |_, _| 0.);
        let p = s.projectiles[0].clone();
        s.projectiles = vec![p; MAX_PROJECTILES];
        let ammo = s.ammo.clone();
        for _ in 0..30 {
            s.step(true, l, |_, _| 0.);
        }
        assert_eq!(s.ammo, ammo);
    }
    #[test]
    fn native_damage_category_switch_is_exact_not_a_mask() {
        for (category, index) in [
            (0x80, 0),
            (0x2000, 1),
            (0x100, 2),
            (0x400, 3),
            (0x40, 4),
            (0x200, 4),
            (0x800, 4),
            (0x1000, 4),
            (0x4000, 0),
            (0x8000, 0),
            (0x240, 0),
        ] {
            assert_eq!(damage_class(category), index);
        }
    }
    #[test]
    fn all_damage_classes_report_nominal_applied_and_cumulative_hp() {
        for (index, category) in [0x80, 0x2000, 0x100, 0x400, 0x40].into_iter().enumerate() {
            let mut s = fixture(false);
            s.config.stations[0].weapon.damage.by_class = [3, 7, 9, 11, 25];
            s.targets.push(Target {
                id: 7,
                position: [0., 1000., 150.],
                velocity: [0.; 3],
                radius: 20.,
                hp: 20,
                category,
            });
            for _ in 0..180 {
                s.step(true, launcher(), |_, _| 0.);
            }
            assert!(s.history.iter().all(|h| h.class == index));
            assert_eq!(
                s.history.iter().map(|h| h.applied).sum::<i32>(),
                20 - s.targets[0].hp
            );
            assert!(s.history.iter().all(|h| h.applied <= h.nominal));
        }
    }
    #[test]
    fn failed_station_keeps_mass_and_jettison_cannot_remove_internal_gun() {
        let mut s = fixture(true);
        let mass = s.payload_lbs();
        s.command(Command::FailStation, launcher());
        assert_eq!(s.readiness(launcher()), Readiness::StationFailed);
        assert_eq!(s.rounds(0), 11);
        assert_eq!(s.payload_lbs(), mass);
        s.step(true, launcher(), |_, _| 0.);
        assert_eq!(s.shots, 0);
        s.command(Command::Jettison, launcher());
        assert_eq!(s.payload_lbs(), 0.);
        let mut gun = fixture(false);
        gun.command(Command::Jettison, launcher());
        assert_eq!(gun.rounds(0), 11);
    }
    #[test]
    fn readiness_reports_inhibits_without_ammunition_consumption() {
        let mut s = fixture(true);
        let l = launcher();
        assert_eq!(s.readiness(l), Readiness::NoTarget);
        s.command(Command::ToggleArm, l);
        assert_eq!(s.readiness(l), Readiness::Safe);
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.rounds(0), 11);
        s.command(Command::ToggleArm, l);
        s.range_target(l);
        s.designate_next(launcher());
        assert_eq!(s.readiness(l), Readiness::Ready);
        let z = &mut s.config.stations[0].weapon.seeker.zones[1];
        z.minimum_range = 4000;
        assert_eq!(s.readiness(l), Readiness::MinimumRange);
        s.config.stations[0].weapon.seeker.zones[1].minimum_range = 0;
        s.config.stations[0].weapon.seeker.zones[1].maximum_range = 2000;
        assert_eq!(s.readiness(l), Readiness::MaximumRange);
    }
    #[test]
    fn replacement_clears_old_engagement_and_uses_fresh_identity() {
        let mut s = fixture(true);
        s.range_target(launcher());
        s.designate_next(launcher());
        s.step(true, launcher(), |_, _| 0.);
        let ammo = s.ammo.clone();
        assert!(!s.projectiles.is_empty());
        s.range_target(launcher());
        assert_eq!(s.targets[0].id, 2);
        assert!(s.projectiles.is_empty() && s.effects.is_empty() && s.designated.is_none());
        assert_eq!(s.ammo, ammo);
    }
    #[test]
    fn autonomous_tracking_survives_radar_off_but_dead_target_is_retired() {
        let mut s = fixture(true);
        s.config.stations[0].weapon.flags &= !0x200;
        s.range_target(launcher());
        s.designate_next(launcher());
        s.step(true, launcher(), |_, _| 0.);
        let mut l = launcher();
        l.radar = false;
        assert_eq!(s.readiness(l), Readiness::RadarOff);
        s.step(false, l, |_, _| 0.);
        assert_eq!(s.projectiles[0].target, Some(1));
        s.targets[0].hp = 0;
        let events = s.step(false, l, |_, _| 0.);
        assert_eq!(s.projectiles[0].target, None);
        assert!(events.contains(&Event::TrackLost(1)));
    }
    #[test]
    fn terrain_visibility_gates_designation_launch_scope_and_tracking() {
        let mut s = fixture(true);
        let l = launcher();
        s.range_target(l);
        s.designate_next(l);
        let wall = |_: f64, z: f64| {
            if (1000. ..2000.).contains(&z) {
                2000.
            } else {
                0.
            }
        };
        s.step(true, l, wall);
        assert_eq!(s.readiness(l), Readiness::TerrainMasked);
        assert_eq!(s.rounds(0), 11);
        assert!(!s.radar_detects(l, s.targets[0].position));
        s.command(Command::ClearDesignation, l);
        s.designate_next(l);
        assert_eq!(s.designated, None);
        s.step(false, l, |_, _| 0.);
        s.designate_next(l);
        s.step(true, l, |_, _| 0.);
        assert_eq!(s.projectiles[0].target, Some(1));
        assert!(s.step(false, l, wall).contains(&Event::TrackLost(1)));
    }
    #[test]
    fn manual_command_tape_is_identical_with_pause_and_render_cadence() {
        let run = |fps: usize| {
            let mut s = fixture(true);
            let l = launcher();
            let mut clock = crate::flight::Clock { remainder: 0. };
            let mut tick = 0;
            for frame in 0..fps * 5 {
                if (fps..fps * 2).contains(&frame) {
                    continue;
                }
                for _ in 0..clock.steps(1. / fps as f64) {
                    let command = match tick {
                        0 => Some(Command::ReplaceTarget),
                        1 => Some(Command::Designate),
                        10 | 20 => Some(Command::ToggleArm),
                        40 => Some(Command::ClearDesignation),
                        50 => Some(Command::Designate),
                        60 => Some(Command::FailStation),
                        90 => Some(Command::Jettison),
                        100 => Some(Command::CycleClass),
                        _ => None,
                    };
                    if let Some(command) = command {
                        s.command(command, l);
                    }
                    s.step(tick % 30 == 0, l, |_, _| 0.);
                    tick += 1;
                }
            }
            (tick, format!("{s:?}"))
        };
        assert_eq!(run(30), run(60));
        assert_eq!(run(60), run(144));
    }
}
