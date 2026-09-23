//! Pilot escape and fitted recovery assessment. See docs/spec/ejection.md.
use crate::{
    attitude::Basis,
    flight::{DT, State},
    models::FlightModel,
};

pub const CONFIRM_TICKS: u64 = 240;
pub const SEAT_TICKS: u64 = 90;
pub const INFLATE_TICKS: u64 = 120;
const GRAVITY: f64 = 32.174;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Seat,
    Freefall,
    Inflating,
    Parachute,
    Landed,
    Impact,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Escape {
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub heading: f64,
    pub phase: Phase,
    pub ticks: u64,
    inflation_started: u64,
}
impl Escape {
    pub fn new(position: [f64; 3], velocity: [f64; 3], basis: Basis) -> Self {
        Self {
            position: std::array::from_fn(|i| position[i] + basis.up[i] * 6.),
            velocity: std::array::from_fn(|i| velocity[i] + basis.up[i] * 80.),
            heading: basis.angles()[0],
            phase: Phase::Seat,
            ticks: 0,
            inflation_started: 0,
        }
    }
    pub fn step(&mut self, ground: impl Fn(f64, f64) -> f64) {
        if matches!(self.phase, Phase::Landed | Phase::Impact) {
            return;
        }
        self.ticks += 1;
        if self.phase == Phase::Seat && self.ticks >= SEAT_TICKS {
            self.phase = Phase::Freefall;
        }
        if self.phase == Phase::Freefall
            && self.velocity[1] < 0.
            && self.position[1] - ground(self.position[0], self.position[2]) <= 4000.
        {
            self.phase = Phase::Inflating;
            self.inflation_started = self.ticks;
        }
        if self.phase == Phase::Inflating && self.ticks - self.inflation_started >= INFLATE_TICKS {
            self.phase = Phase::Parachute;
        }
        if matches!(self.phase, Phase::Inflating | Phase::Parachute) {
            let opening = if self.phase == Phase::Parachute {
                1.
            } else {
                (self.ticks - self.inflation_started) as f64 / INFLATE_TICKS as f64
            };
            self.velocity[1] -= GRAVITY * DT * (1. - opening);
            for (i, v) in self.velocity.iter_mut().enumerate() {
                let target = if i == 1 { -18. } else { 0. };
                *v += (target - *v) * (DT / 1.5) * opening;
            }
        } else {
            self.velocity[1] -= GRAVITY * DT;
            for v in &mut self.velocity {
                *v -= *v * (v.abs() * 0.0015 * DT).min(0.5);
            }
        }
        for i in 0..3 {
            self.position[i] += self.velocity[i] * DT;
        }
        let height = ground(self.position[0], self.position[2]);
        if self.position[1] <= height {
            self.position[1] = height;
            self.phase = if self.phase == Phase::Parachute && self.velocity[1] >= -30. {
                Phase::Landed
            } else {
                Phase::Impact
            };
            self.velocity = [0.; 3];
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hazard {
    Destroyed,
    Dive,
    Lift,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Assessment {
    pub hazard: Hazard,
    pub impact_seconds: f64,
}

/// Conservative analytic recovery estimate, not a reconstruction of original AI.
pub fn assess(s: &State, ground: impl Fn(f64, f64) -> f64) -> Option<Assessment> {
    let height = ground(s.position[0], s.position[2]);
    if s.systems.pilot.dead
        || s.escape.is_some()
        || !s.seat_available()
        || s.wreck_gone()
        || s.supported_at(height)
        || s.position[1] <= height
    {
        return None;
    }
    let sink = -s.velocity[1];
    let agl = s.position[1] - height;
    let impact_seconds = if sink > 0. { agl / sink } else { f64::INFINITY };
    if s.crashed || s.systems.structure.failed {
        return Some(Assessment {
            hazard: Hazard::Destroyed,
            impact_seconds,
        });
    }
    // John's 2026-09-23 safeguard: healthy aircraft above 200 AGL never auto-eject.
    let undamaged = s.damage_fraction <= 0.
        && s.damage_regions.iter().all(|d| *d <= 0.)
        && s.systems.counts.iter().all(|n| *n == 0)
        && !s.systems.structure.wing_damage
        && !s.systems.structure.burning()
        && s.systems.fluids.hydraulic >= 1.;
    if undamaged && agl > 200. {
        return None;
    }
    if sink <= 20. {
        return None;
    }
    let config = s.model().configuration();
    let regions = crate::aircraft_systems::regional_effects(s.damage_regions);
    let response = s
        .systems
        .controls([1.; 3], [s.elevator, s.aileron, s.rudder], s.ticks);
    let health = (1. - s.damage_fraction).clamp(0., 1.);
    let pitch = response[0].clamp(0., 1.) * regions.authority[0] * health;
    let roll = response[1].abs().clamp(0., 1.) * regions.authority[1] * health;
    let envelope_g = config
        .aerodynamics
        .envelopes
        .iter()
        .filter(|e| {
            e.g > 0
                && e.speeds(s.position[1])
                    .is_some_and(|(lo, hi)| (lo..=hi).contains(&s.speed))
        })
        .map(|e| f64::from(e.g))
        .fold(0., f64::max);
    let available_g = envelope_g
        * pitch
        * regions.lift
        * if s.systems.structure.wing_damage {
            0.5
        } else {
            1.
        };
    if available_g <= 1.05 {
        return (s.pitch < -5_f64.to_radians() && (impact_seconds <= 8. || pitch <= 0.25))
            .then_some(Assessment {
                hazard: Hazard::Lift,
                impact_seconds,
            });
    }
    let speed = s.speed.max(1.);
    let gamma = (sink / speed).clamp(0., 1.).asin();
    let roll_time = s.bank.abs() / (config.aerodynamics.roll_limit_rad_per_second * roll).max(0.05);
    let radius = speed * speed / (GRAVITY * (available_g - 1.));
    let time = (roll_time + radius * gamma / speed).min(10.);
    let terrain = (0..=8)
        .map(|i| {
            let t = time * f64::from(i) / 8.;
            ground(
                s.position[0] + s.velocity[0] * t,
                s.position[2] + s.velocity[2] * t,
            )
        })
        .fold(height, f64::max);
    let required = sink * roll_time + radius * (1. - gamma.cos()) + 75.;
    (s.position[1] - terrain <= required).then_some(Assessment {
        hazard: Hazard::Dive,
        impact_seconds,
    })
}

/// Where an AI aircraft taking off or landing is, for [`catastrophic`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirfieldContext {
    /// Height above the surface below, feet.
    pub agl_ft: f64,
    /// The surface below is a runway.
    pub landable_below: bool,
    /// Horizontal distance to the landing point, feet.
    pub landing_distance_ft: f64,
}

/// `opinionated` (John, 2026-09-23): during landing an AI pilot ejects only
/// from a catastrophe; other hazards use a go-around. The extension to the
/// takeoff roll and climb-out and the thresholds are `fitted` agent decisions.
/// Damage at or above this fraction is critical.
pub const CRITICAL_DAMAGE_FRACTION: f64 = 0.5;
/// A dead engine can glide this many feet forward per foot of height.
pub const DEAD_STICK_GLIDE_RATIO: f64 = 6.0;
/// Bank beyond this near the ground is out of control.
pub const EXTREME_BANK_DEG: f64 = 90.0;
/// Height below which [`EXTREME_BANK_DEG`] counts.
pub const EXTREME_BANK_AGL_FT: f64 = 1_000.0;
/// Ground contact this close is unavoidable; it is catastrophic only when
/// the touchdown would be a crash.
pub const UNAVOIDABLE_IMPACT_S: f64 = 1.0;

/// Whether an AI aircraft on an airfield sequence should still eject for
/// `assessment`: destroyed or structurally failed, on fire, critically
/// damaged, a dead engine that cannot glide to the landing point, spinning,
/// inverted or beyond 90 degrees of bank low down, or ground contact within
/// a second that would be a crash (off the runway, gear not down, or outside
/// the aircraft's landing limits). Anything else is a go-around.
pub fn catastrophic(s: &State, assessment: Assessment, field: AirfieldContext) -> bool {
    use tore_formats::flight_model::ground::{LandingSeverity, landing_severity};
    if assessment.hazard == Hazard::Destroyed
        || s.systems.structure.failed
        || s.systems.structure.burning()
        || s.systems.structure.wing_damage
        || s.damage_fraction >= CRITICAL_DAMAGE_FRACTION
    {
        return true;
    }
    let engine_dead = !s.engine || s.fuel <= 0. || s.systems.power_available() <= 0.;
    if engine_dead && field.agl_ft * DEAD_STICK_GLIDE_RATIO < field.landing_distance_ft {
        return true;
    }
    if s.research.as_ref().is_some_and(|r| r.spinning != 0)
        || (s.bank.abs() > EXTREME_BANK_DEG.to_radians() && field.agl_ft < EXTREME_BANK_AGL_FT)
    {
        return true;
    }
    if assessment.impact_seconds > UNAVOIDABLE_IMPACT_S {
        return false;
    }
    let basis = Basis::new(s.yaw, s.pitch, s.bank);
    let forward = crate::attitude::dot(s.velocity, basis.forward);
    let side = crate::attitude::dot(s.velocity, basis.right);
    let severity = landing_severity(
        s.model().configuration().native.landing,
        (s.bank.to_degrees() * 256.) as i32,
        (s.pitch.to_degrees() * 256.) as i32,
        (forward * 256.) as i32,
        (side * 256.) as i32,
        s.velocity[1] as i16,
    );
    !field.landable_below || s.gear < 0.99 || severity != LandingSeverity::WithinLimits
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Monitor {
    ticks: u64,
    random: u64,
}
impl Monitor {
    pub fn seeded(seed: u64) -> Self {
        Self {
            random: seed,
            ..Self::default()
        }
    }
    pub fn step(&mut self, assessment: Option<Assessment>) -> Option<Hazard> {
        let Some(a) = assessment else {
            self.ticks = 0;
            return None;
        };
        // Changing the diagnosis does not postpone a still-active danger episode.
        self.ticks += 1;
        let delay = if a.hazard == Hazard::Lift { 240 } else { 30 };
        if !self.ticks.is_multiple_of(120)
            || !(a.hazard == Hazard::Destroyed || a.impact_seconds <= 2. || self.ticks >= delay)
        {
            return None;
        }
        // Dedicated per-pilot stream. Combat, renderer and other actors never consume it.
        self.random = self.random.wrapping_add(0x9e3779b97f4a7c15);
        let mut draw = self.random;
        draw = (draw ^ (draw >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        draw = (draw ^ (draw >> 27)).wrapping_mul(0x94d049bb133111eb);
        draw ^= draw >> 31;
        (draw % 100 < 70).then_some(a.hazard)
    }
}

impl State {
    pub fn view_position(&self) -> [f64; 3] {
        self.escape.as_ref().map_or(self.position, |p| p.position)
    }
    pub fn seat_available(&self) -> bool {
        self.model().configuration().ejection_seat
    }
    pub fn can_eject(&self) -> bool {
        self.seat_available()
            && !self.systems.pilot.dead
            && self.escape.is_none()
            && !self.wreck_gone()
    }
    pub fn request_ejection(&mut self) {
        if !self.can_eject() {
            self.eject_armed_at = None;
            self.systems.notify("Ejection unavailable");
            return;
        }
        if self
            .eject_armed_at
            .is_some_and(|t| self.ticks.saturating_sub(t) <= CONFIRM_TICKS)
        {
            self.eject();
        } else {
            self.eject_armed_at = Some(self.ticks);
            self.systems
                .notify("Press eject again within 2 seconds to confirm");
        }
    }
    pub fn eject(&mut self) -> bool {
        if !self.can_eject() {
            return false;
        }
        self.escape = Some(Escape::new(
            self.position,
            self.velocity,
            Basis::new(self.yaw, self.pitch, self.bank),
        ));
        self.systems.pilot.ejected = true;
        self.eject_armed_at = None;
        self.autopilot.disengage();
        self.radar = false;
        self.jammer = false;
        self.crashed = true;
        self.systems.notify("Pilot ejected");
        true
    }
    /// Also used by the AI host, where combat owns the abandoned aircraft motion.
    pub fn step_escape(&mut self, ground: impl Fn(f64, f64) -> f64) {
        let Some(escape) = &mut self.escape else {
            return;
        };
        let before = escape.phase;
        escape.step(ground);
        if escape.phase == Phase::Impact && self.systems.pilot.kill() {
            self.systems.notify("Pilot killed during ejection");
        }
        let landed = self.escape.as_ref().unwrap().phase == Phase::Landed;
        for message in self.systems.pilot.advance(landed) {
            self.systems.notify(message);
        }
        if landed && before != Phase::Landed && !self.systems.pilot.dead {
            self.systems.notify("Pilot landed safely");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::{PilotCommand, PilotInput};
    fn state() -> State {
        let mut a = crate::flight::integration_tests::profile();
        a.fields.get_mut("flags").unwrap().value = "16".into();
        State::new(&a, [0., 10000., 0.]).unwrap()
    }
    #[test]
    fn healthy_above_200_agl_never_auto_ejects_even_in_a_steep_inverted_dive() {
        for agl in [200.001, 201., 1000., 10000.] {
            for bank in [0., 1.5, std::f64::consts::PI] {
                for speed in [100., 600., 2000.] {
                    let mut s = state();
                    s.position[1] = 5000. + agl;
                    s.pitch = -89_f64.to_radians();
                    s.bank = bank;
                    s.speed = speed;
                    s.velocity = Basis::new(s.yaw, s.pitch, bank).forward.map(|v| v * speed);
                    assert_eq!(assess(&s, |_, _| 5000.), None);
                    assert!(s.eject(), "manual escape remains available above 200 AGL");
                }
            }
        }
    }
    #[test]
    fn ejection_requires_confirmation_and_preserves_a_living_pilot_in_a_wreck() {
        let mut s = state();
        s.crashed = true;
        s.request_ejection();
        assert!(s.escape.is_none());
        s.ticks += CONFIRM_TICKS + 1;
        s.request_ejection();
        assert!(s.escape.is_none(), "expired confirmation arms anew");
        s.ticks += CONFIRM_TICKS;
        s.step(
            &PilotInput {
                commands: vec![PilotCommand::Eject],
                ..Default::default()
            },
            |_, _| 0.,
        );
        assert!(s.escape.is_some());
        assert!(!s.systems.pilot.dead);
        assert!(!s.eject());
        s.systems.kill_pilot("Later aircraft explosion");
        assert!(!s.systems.pilot.dead);
    }
    #[test]
    fn absent_seat_dead_pilot_and_prior_impact_refuse_escape() {
        let mut s = state();
        s.systems.pilot.kill();
        assert!(!s.eject());
        let mut s = state();
        let mut config = s.model().configuration().clone();
        config.ejection_seat = false;
        let mut model = s.model().clone();
        model.set_configuration(config).unwrap();
        s = State::from_model(model, s.position);
        assert!(!s.eject());
        let mut s = state();
        s.crashed = true;
        s.position[1] = 0.;
        s.step(&PilotInput::default(), |_, _| 0.);
        assert!(!s.eject());
    }
    #[test]
    fn launch_inherits_attitude_velocity_and_has_exact_phase_boundaries() {
        let basis = Basis::new(0., 0., 0.);
        let mut p = Escape::new([0., 5000., 0.], [100., 0., 20.], basis);
        assert_eq!(p.position, [0., 5006., 0.]);
        assert_eq!(p.velocity, [100., 80., 20.]);
        for _ in 0..SEAT_TICKS - 1 {
            p.step(|_, _| 0.);
        }
        assert_eq!(p.phase, Phase::Seat);
        p.step(|_, _| 0.);
        assert_eq!(p.phase, Phase::Freefall);
        p.position[1] = 4000.;
        p.velocity[1] = -1.;
        p.step(|_, _| 0.);
        assert_eq!(p.phase, Phase::Inflating);
        for _ in 0..INFLATE_TICKS - 1 {
            p.step(|_, _| 0.);
        }
        assert_eq!(p.phase, Phase::Inflating);
        p.step(|_, _| 0.);
        assert_eq!(p.phase, Phase::Parachute);
    }
    #[test]
    fn chute_lands_alive_and_low_inverted_launch_is_fatal() {
        let mut s = state();
        s.position[1] = 1000.;
        s.velocity = [0.; 3];
        assert!(s.eject());
        for _ in 0..120 * 120 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert_eq!(s.escape.as_ref().unwrap().phase, Phase::Landed);
        assert!(
            !s.systems.pilot.dead,
            "later wreck explosion must not kill escapee"
        );
        let mut s = state();
        s.position[1] = 15.;
        s.bank = std::f64::consts::PI;
        s.velocity = [0.; 3];
        assert!(s.eject());
        for _ in 0..60 {
            s.step_escape(|_, _| 0.);
        }
        assert_eq!(s.escape.as_ref().unwrap().phase, Phase::Impact);
        assert!(s.systems.pilot.dead);
    }
    #[test]
    fn escape_wounds_progress_and_landing_does_not_revive_a_dead_pilot() {
        let mut s = state();
        s.systems.hit(34, 0.);
        // Repeated wounds shorten the existing 900-second timer below one second.
        for _ in 0..10 {
            s.systems.hit(34, 0.);
        }
        s.eject();
        for _ in 0..120 {
            s.step_escape(|_, _| 0.);
        }
        assert!(s.systems.pilot.dead);
        let pilot = s.escape.as_mut().unwrap();
        pilot.phase = Phase::Parachute;
        pilot.position[1] = 0.01;
        pilot.velocity = [0., -18., 0.];
        s.step_escape(|_, _| 0.);
        assert!(s.systems.pilot.dead);
    }
    #[test]
    fn assessment_distinguishes_recoverable_dives_terrain_and_lost_authority() {
        let mut s = state();
        assert_eq!(assess(&s, |_, _| 0.), None);
        s.pitch = -45_f64.to_radians();
        s.velocity = Basis::new(s.yaw, s.pitch, 0.).forward.map(|v| v * s.speed);
        assert_eq!(assess(&s, |_, _| 0.), None, "high dive can recover");
        s.position[1] = 200.01;
        assert_eq!(
            assess(&s, |_, _| 0.),
            None,
            "healthy aircraft above 200 AGL is protected"
        );
        s.position[1] = 200.;
        assert_eq!(assess(&s, |_, _| 0.).unwrap().hazard, Hazard::Dive);
        s.position[1] = 10000.;
        s.damage_fraction = 0.1;
        assert!(
            assess(&s, |_, z| if z > 100. { 9950. } else { 0. }).is_some(),
            "terrain along path matters"
        );
        s.systems.fluids.hydraulic = 0.;
        s.elevator = 0.;
        assert_eq!(assess(&s, |_, _| 0.).unwrap().hazard, Hazard::Lift);
        s.systems.pilot.kill();
        assert_eq!(assess(&s, |_, _| 0.), None);
    }
    #[test]
    fn monitor_polls_once_per_second_with_70_percent_chance_and_keeps_failed_pilots_aboard() {
        let mut monitor = Monitor::seeded(4); // Reviewed synthetic draws: 78, 4, 47.
        let a = Assessment {
            hazard: Hazard::Dive,
            impact_seconds: 1.5,
        };
        for _ in 0..119 {
            assert_eq!(monitor.step(Some(a)), None);
        }
        assert_eq!(
            monitor.step(Some(a)),
            None,
            "78 fails the 70% roll at one second"
        );
        for _ in 0..119 {
            assert_eq!(monitor.step(Some(a)), None);
        }
        assert_eq!(
            monitor.step(Some(a)),
            Some(Hazard::Dive),
            "4 succeeds at two seconds"
        );
        let random = monitor.random;
        assert_eq!(monitor.step(None), None);
        assert_eq!(
            monitor.random, random,
            "recovery does not reseed or consume a draw"
        );
        for _ in 0..119 {
            assert_eq!(monitor.step(Some(a)), None);
        }
        assert_eq!(monitor.step(Some(a)), Some(Hazard::Dive));
        let mut changing = Monitor::seeded(0);
        for _ in 0..119 {
            assert_eq!(changing.step(Some(a)), None);
        }
        assert_eq!(
            changing.step(Some(Assessment {
                hazard: Hazard::Destroyed,
                ..a
            })),
            Some(Hazard::Destroyed),
            "changing danger reasons must not postpone the next second's roll"
        );
        let mut lift = Monitor::seeded(0);
        let a = Assessment {
            hazard: Hazard::Lift,
            impact_seconds: 10.,
        };
        for _ in 0..239 {
            assert_eq!(lift.step(Some(a)), None);
        }
        assert_eq!(lift.step(Some(a)), Some(Hazard::Lift));
        // Separate pilots replay exactly even when stepped alongside another pilot.
        let mut first = Monitor::seeded(8);
        let mut replay = first.clone();
        let mut other = Monitor::seeded(99);
        for _ in 0..1200 {
            other.step(Some(a));
            assert_eq!(first.step(Some(a)), replay.step(Some(a)));
        }
    }
    #[test]
    fn recorded_commands_replay_deterministically_and_do_not_change_flight_adapters() {
        for research in [false, true] {
            let mut a = state();
            if research {
                a.enable_research(123).unwrap();
            }
            let mut b = a.clone();
            let input = PilotInput {
                commands: vec![PilotCommand::Eject],
                ..Default::default()
            };
            let mut bytes = format!("{}\n", tore_input::recording::HEADER).into_bytes();
            for tick in 1..=1200 {
                let frame = if tick == 1 || tick == 12 {
                    input.clone()
                } else {
                    PilotInput::default()
                };
                tore_input::recording::write_frame(&mut bytes, tick, &frame).unwrap();
                a.step(&frame, |_, _| 0.);
            }
            for frame in tore_input::recording::read(bytes.as_slice()).unwrap() {
                b.step(&frame, |_, _| 0.);
            }
            assert_eq!(a, b);
            assert_eq!(a.research.is_some(), research);
            assert!(a.escape.is_some());
        }
    }
}
