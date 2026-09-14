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
    pub external_equipment_lbs: i32,
    pub radar: tore_formats::weapons::Seeker,
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
        Ok(Self {
            radar,
            external_equipment_lbs,
            aircraft: a.id,
            stations,
            hit_points,
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
        Ok(Self {
            external,
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
    pub fn designate_next(&mut self) {
        self.designated = self
            .targets
            .iter()
            .filter(|t| t.hp > 0)
            .find(|t| Some(t.id) > self.designated)
            .or_else(|| self.targets.iter().find(|t| t.hp > 0))
            .map(|t| t.id);
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
            .map(|(s, count)| f64::from(s.weapon.weight.max(0)) * f64::from(*count))
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
        self.targets.clear();
        self.targets.push(Target {
            id: 1,
            position: std::array::from_fn(|i| {
                launcher.position[i] + launcher.basis.forward[i] * distance
            }),
            velocity: launcher.basis.forward.map(|v| v * 300.),
            radius: 28.,
            hp: self.config.hit_points,
        });
        self.designated = None;
    }
    pub fn radar_detects(&self, launcher: Launcher, target: Vector) -> bool {
        launcher.radar
            && cone(
                &self.config.radar.zones[0],
                launcher.position,
                launcher.basis.forward,
                target,
            )
    }
    pub fn can_lock(&self, launcher: Launcher) -> bool {
        let w = &self.config.stations[self.selected].weapon;
        if w.seeker.signature == 0 {
            return false;
        }
        self.designated
            .and_then(|id| self.targets.iter().find(|t| t.id == id && t.hp > 0))
            .is_some_and(|t| {
                (w.seeker.signature != 3 || self.radar_detects(launcher, t.position))
                    && acquisition(
                        w,
                        launcher.position,
                        launcher.basis.forward,
                        t.position,
                        launcher.radar,
                        1,
                    )
            })
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
        let index = self.selected;
        let station = &self.config.stations[index];
        let w = &station.weapon;
        let guided = w.seeker.signature != 0;
        let allowed = !guided || self.can_lock(launcher);
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
                if acquisition(w, p.position, p.direction, t.position, launcher.radar, 0) {
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
                    p.target = None;
                }
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
                    // Source damage class 0 is used for the aircraft range target.
                    // HP subtraction/destruction replaces unported subsystem rolls.
                    t.hp = (t.hp - i32::from(w.damage.by_class[0]).max(0)).max(0);
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
    if w.seeker.signature == 3 && !radar {
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
    let d = sub(target, position);
    let distance = dot(d, d).sqrt();
    let angle = dot(unit(d), forward).clamp(-1., 1.).acos();
    distance >= f64::from(z.minimum_range)
        && distance <= f64::from(z.maximum_range)
        && d[1] >= f64::from(z.minimum_altitude)
        && d[1] <= f64::from(z.maximum_altitude)
        && angle <= f64::from(z.heading.min(z.pitch).max(0)) * std::f64::consts::TAU / 65520.
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
            flags: if guided { 0x40 } else { 0x844 },
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
                external_equipment_lbs: 0,
                radar: seeker,
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
        s.designate_next();
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
}
