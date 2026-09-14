//! Deterministic 120 Hz free-flight adapter. PT facts are recovered; integration is authored.
use crate::attitude::{Basis, cross, dot, unit};
use std::collections::BTreeSet;
use tore_formats::aircraft::Aircraft;
pub const DT: f64 = 1.0 / 120.0;
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub position: [f64; 3],
    pub yaw: f64,
    pub pitch: f64,
    pub bank: f64,
    pub speed: f64,
    pub velocity: [f64; 3],
    pub roll_rate: f64,
    pub pitch_rate: f64,
    pub vertical_speed: f64,
    pub g: f64,
    pub throttle: f64,
    pub fuel: f64,
    pub engine: bool,
    pub burner: bool,
    pub gear: f64,
    pub flaps: f64,
    pub brake: f64,
    pub hook: f64,
    pub gear_down: bool,
    pub flaps_down: bool,
    pub brake_out: bool,
    pub hook_down: bool,
    pub radar: bool,
    pub jammer: bool,
    pub crashed: bool,
    pub ticks: u64,
}
impl State {
    pub fn new(a: &Aircraft, position: [f64; 3]) -> Self {
        Self {
            position,
            yaw: 0.3,
            pitch: 0.,
            bank: 0.,
            speed: 450. * 1.68781,
            velocity: Basis::new(0.3, 0., 0.).forward.map(|v| v * 450. * 1.68781),
            roll_rate: 0.,
            pitch_rate: 0.,
            vertical_speed: 0.,
            g: 1.,
            throttle: 0.7,
            fuel: a.number("internalFuel"),
            engine: true,
            burner: false,
            gear: 0.,
            flaps: 0.,
            brake: 0.,
            hook: 0.,
            gear_down: false,
            flaps_down: false,
            brake_out: false,
            hook_down: false,
            radar: true,
            jammer: false,
            crashed: false,
            ticks: 0,
        }
    }
    /// Interpolate presentation only, leaving fixed-tick state and discrete controls untouched.
    pub fn presented(&self, previous: &Self, alpha: f64) -> Self {
        if self.crashed {
            return self.clone();
        }
        let alpha = alpha.clamp(0., 1.);
        let lerp = |a: f64, b: f64| a + (b - a) * alpha;
        let mut result = self.clone();
        for i in 0..3 {
            result.position[i] = lerp(previous.position[i], self.position[i]);
        }
        [result.yaw, result.pitch, result.bank] =
            Basis::new(previous.yaw, previous.pitch, previous.bank)
                .blended(Basis::new(self.yaw, self.pitch, self.bank), alpha)
                .angles();
        result.velocity = std::array::from_fn(|i| lerp(previous.velocity[i], self.velocity[i]));
        result.speed = lerp(previous.speed, self.speed);
        result.vertical_speed = lerp(previous.vertical_speed, self.vertical_speed);
        result.g = lerp(previous.g, self.g);
        result
    }
    pub fn toggle(&mut self, key: &str) {
        match key {
            "g" => self.gear_down = !self.gear_down,
            "f" => self.flaps_down = !self.flaps_down,
            "b" => self.brake_out = !self.brake_out,
            "h" => self.hook_down = !self.hook_down,
            "e" => self.engine = !self.engine,
            "t" => self.burner = !self.burner,
            "r" => self.radar = !self.radar,
            "j" => self.jammer = !self.jammer,
            _ => {}
        }
    }
    pub fn step(
        &mut self,
        a: &Aircraft,
        keys: &BTreeSet<String>,
        ground: impl Fn(f64, f64) -> f64,
    ) {
        if self.crashed {
            return;
        }
        self.ticks += 1;
        let k = |s: &str| f64::from(keys.contains(s));
        self.throttle = (self.throttle + (k("PageUp") - k("PageDown")) * DT * 0.35).clamp(0., 1.);
        for (v, on) in [
            (&mut self.gear, self.gear_down),
            (&mut self.flaps, self.flaps_down),
            (&mut self.brake, self.brake_out),
            (&mut self.hook, self.hook_down),
        ] {
            *v = (*v + (if on { 1. } else { -1. }) * DT / 3.).clamp(0., 1.);
        }
        if self.fuel <= 0. {
            self.engine = false;
            self.burner = false;
        }
        let ab = self.burner && self.engine && self.throttle > 0.95 && a.number("aftThrust") > 0.;
        let rate = if ab {
            a.number("aftFuelConsumption")
        } else {
            a.number("fuelConsumption") * self.throttle
        };
        if self.engine {
            self.fuel = (self.fuel - rate * DT).max(0.);
        }
        let env = a.envelopes.iter().find(|e| e.g == 1).unwrap();
        let (stall, vmax) = env.speeds(self.position[1]).unwrap_or((900., 1000.));
        let authority = (self.speed / stall.max(1.)).powi(2).clamp(0., 1.);
        let (mut lo, mut hi) = (-1., 1.);
        for e in &a.envelopes {
            if let Some((low, high)) = e.speeds(self.position[1])
                && self.speed >= low
                && self.speed <= high
            {
                lo = f64::min(lo, e.g as f64);
                hi = f64::max(hi, e.g as f64);
            }
        }
        let loading = self.fuel / a.number("weight");
        hi /= 1. + loading * a.number("loadedElevator") / 100.;
        lo /= 1. + loading * a.number("loadedElevator") / 100.;
        let command = (1.
            + (k("ArrowDown") - k("ArrowUp"))
                * if keys.contains("ArrowDown") {
                    hi - 1.
                } else {
                    1. - lo
                })
        .clamp(lo, hi)
            * authority;
        self.g += (command - self.g) * (DT * 4.).min(1.);
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let roll_command = (k("ArrowRight") - k("ArrowLeft")) * 1.8 * authority;
        self.roll_rate += (roll_command - self.roll_rate) * (DT / 0.2);
        let pitch_command = (command - basis.up[1]) * 32.174 / self.speed.max(60.);
        self.pitch_rate += (pitch_command - self.pitch_rate) * (DT / 0.1);
        // Authored aerodynamic alignment: the nose responds before the flight path settles.
        let alignment = cross(basis.forward, unit(self.velocity));
        let rotation = std::array::from_fn(|i| {
            DT * (-basis.right[i] * self.pitch_rate - basis.forward[i] * self.roll_rate
                + basis.up[i] * (k("x") - k("z")) * 0.12 * authority
                + alignment[i] * 0.7 * authority)
        });
        let basis = basis.rotated(rotation);
        [self.yaw, self.pitch, self.bank] = basis.angles();
        let max_thrust = a.number("aftThrust").max(a.number("thrust"));
        let lapse = (-self.position[1].max(0.) / 70000.).exp();
        let thrust = if self.engine {
            if ab {
                max_thrust
            } else {
                a.number("thrust") * self.throttle
            }
        } else {
            0.
        } * lapse;
        let weight = a.number("weight") + self.fuel;
        // Drag normalized against the source 1G upper envelope. This is not the native force law.
        let drag = max_thrust
            * lapse
            * (self.speed / vmax.max(100.)).powi(2)
            * (1. + loading * a.number("loadedDrag") / 100.)
            + weight
                * (a.number("_gpullDrag") * (self.g.abs() - 1.).max(0.)
                    + a.number("gearDrag") * self.gear
                    + a.number("flapsDrag") * self.flaps
                    + a.number("airBrakesDrag") * self.brake)
                / 256.;
        let direction = unit(self.velocity);
        let along = dot(basis.up, direction);
        let lift = unit(std::array::from_fn(|i| basis.up[i] - along * direction[i]));
        for i in 0..3 {
            self.velocity[i] += (basis.forward[i] * thrust / weight * 32.174
                - direction[i] * drag / weight * 32.174
                + lift[i] * self.g * 32.174
                - if i == 1 { 32.174 } else { 0. })
                * DT;
        }
        self.speed = dot(self.velocity, self.velocity).sqrt();
        if self.speed > 6000. {
            self.velocity = self.velocity.map(|v| v * 6000. / self.speed);
            self.speed = 6000.;
        }
        self.vertical_speed = self.velocity[1];
        for i in 0..3 {
            self.position[i] += self.velocity[i] * DT;
        }
        let floor = ground(self.position[0], self.position[2]) + 8.;
        if self.position[1] <= floor {
            self.position[1] = floor;
            self.crashed = true;
            self.speed = 0.;
            self.velocity = [0.; 3];
            self.vertical_speed = 0.;
            self.engine = false;
            self.burner = false;
        }
    }
}
pub struct Clock {
    pub remainder: f64,
}
impl Clock {
    pub fn steps(&mut self, seconds: f64) -> usize {
        self.steps_scaled(seconds, 1.)
    }
    pub fn steps_scaled(&mut self, seconds: f64, scale: f64) -> usize {
        self.remainder += seconds.clamp(0., 0.25) * scale.clamp(0.5, 8.);
        let n = ((self.remainder + 1e-10) / DT).floor() as usize;
        self.remainder -= n as f64 * DT;
        n
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_wraps_angles_without_changing_simulation() {
        let a = super::integration_tests::profile();
        let mut previous = State::new(&a, [0.; 3]);
        previous.yaw = 359f64.to_radians();
        previous.bank = 179f64.to_radians();
        let mut current = previous.clone();
        current.yaw = 1f64.to_radians();
        current.bank = -179f64.to_radians();
        current.position[0] = 10.;
        current.gear_down = true;
        let before = current.clone();
        let rendered = current.presented(&previous, 0.5);
        assert!((rendered.yaw.sin()).abs() < 1e-9);
        assert!((rendered.bank.abs() - std::f64::consts::PI).abs() < 1e-9);
        assert_eq!(rendered.position[0], 5.);
        assert!(rendered.gear_down);
        assert_eq!(current, before);
        current.crashed = true;
        assert_eq!(current.presented(&previous, 0.), current);
    }
    #[test]
    fn fixed_clock_independent_of_render_rate() {
        for hz in [24, 30, 60, 144] {
            let mut c = Clock { remainder: 0. };
            let n: usize = (0..hz * 10).map(|_| c.steps(1. / hz as f64)).sum();
            assert_eq!(n, 1200);
        }
    }
}
#[cfg(test)]
mod integration_tests {
    use super::*;
    use std::collections::BTreeMap;
    use tore_formats::aircraft::{Envelope, Token};
    pub(super) fn profile() -> Aircraft {
        let fields = [
            ("weight", 10000),
            ("internalFuel", 1000),
            ("thrust", 8000),
            ("aftThrust", 12000),
            ("fuelConsumption", 2),
            ("aftFuelConsumption", 10),
            ("maxTakeoffWeight", 15000),
            ("gearDrag", 23),
            ("flapsDrag", 70),
            ("airBrakesDrag", 256),
            ("loadedElevator", 40),
        ]
        .into_iter()
        .map(|(k, v)| {
            (
                k.into(),
                Token {
                    kind: "dword".into(),
                    value: v.to_string(),
                    scaled: false,
                },
            )
        })
        .collect();
        Aircraft {
            name: "Synthetic".into(),
            shape: "TEST.SH".into(),
            fields,
            object: BTreeMap::new(),
            hardpoints: vec![],
            sounds: BTreeMap::new(),
            envelopes: (-2..=6)
                .map(|g| Envelope {
                    g,
                    points: vec![[200., 0.], [250., 50000.], [1300., 50000.], [1800., 0.]],
                })
                .collect(),
        }
    }
    #[test]
    fn momentum_and_roll_response_survive_control_release() {
        let a = profile();
        let mut s = State::new(&a, [0., 15000., 0.]);
        let keys = BTreeSet::from(["ArrowDown".into(), "ArrowRight".into()]);
        for _ in 0..60 {
            s.step(&a, &keys, |_, _| 0.);
        }
        let nose = Basis::new(s.yaw, s.pitch, s.bank).forward;
        assert!(dot(nose, unit(s.velocity)) < 0.99999);
        let rate = s.roll_rate;
        s.step(&a, &Default::default(), |_, _| 0.);
        assert!(s.roll_rate > 0. && s.roll_rate < rate);
        assert_eq!(s.vertical_speed, s.velocity[1]);
        assert!(s.velocity.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn sustained_pull_can_complete_a_loop() {
        let a = profile();
        let mut s = State::new(&a, [0., 15000., 0.]);
        s.throttle = 1.;
        s.burner = true;
        let start = Basis::new(s.yaw, s.pitch, s.bank).forward;
        let keys = BTreeSet::from(["ArrowDown".into()]);
        let (mut vertical, mut inverted, mut completed) = (false, false, false);
        for _ in 0..120 * 90 {
            s.step(&a, &keys, |_, _| 0.);
            let b = Basis::new(s.yaw, s.pitch, s.bank);
            vertical |= b.forward[1] > 0.999;
            inverted |= b.up[1] < -0.9;
            if inverted && b.up[1] > 0.9 && dot(b.forward, start) > 0.98 {
                completed = true;
                break;
            }
        }
        assert!(
            vertical && inverted && completed && !s.crashed,
            "vertical={vertical} inverted={inverted} completed={completed} state={s:?}"
        );
    }
    #[test]
    fn banked_lift_does_not_create_a_climb_without_a_pull() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]);
        s.bank = std::f64::consts::FRAC_PI_2;
        s.step(&a, &Default::default(), |_, _| 0.);
        assert!(s.velocity[1] < 0. && s.position[1] < 5000.);
    }
    #[test]
    fn simulation_is_identical_at_different_render_rates() {
        let a = profile();
        let start = State::new(&a, [0., 5000., 0.]);
        let mut result = Vec::new();
        for hz in [30, 60, 144] {
            let mut s = start.clone();
            let mut c = Clock { remainder: 0. };
            let keys = ["ArrowDown".to_string(), "ArrowRight".into()].into();
            for _ in 0..hz * 3 {
                for _ in 0..c.steps(1. / hz as f64) {
                    s.step(&a, &keys, |_, _| 0.);
                }
            }
            result.push(s);
        }
        assert_eq!(result[0], result[1]);
        assert_eq!(result[0], result[2]);
        assert_ne!(result[0].position, start.position);
    }
    #[test]
    fn depleted_fuel_stops_power_and_ground_contact_stops_flight() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]);
        s.fuel = 0.;
        s.burner = true;
        s.step(&a, &Default::default(), |_, _| 0.);
        assert!(!s.engine && !s.burner);
        s.position[1] = 0.;
        s.step(&a, &Default::default(), |_, _| 0.);
        assert!(s.crashed);
        let final_state = s.clone();
        s.step(&a, &Default::default(), |_, _| 0.);
        assert_eq!(s, final_state);
    }
    #[test]
    fn burner_consumes_more_fuel_and_actuators_reach_endpoints() {
        let a = profile();
        let mut dry = State::new(&a, [0., 5000., 0.]);
        dry.throttle = 1.;
        let mut wet = dry.clone();
        wet.burner = true;
        wet.gear_down = true;
        for _ in 0..400 {
            dry.step(&a, &Default::default(), |_, _| 0.);
            wet.step(&a, &Default::default(), |_, _| 0.);
        }
        assert!(wet.fuel < dry.fuel);
        assert_eq!(wet.gear, 1.);
        wet.gear_down = false;
        for _ in 0..400 {
            wet.step(&a, &Default::default(), |_, _| 0.);
        }
        assert_eq!(wet.gear, 0.);
    }
}
