//! Deterministic 120 Hz free-flight adapter. PT facts are recovered; integration is authored.
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
        self.bank = (self.bank + (k("ArrowRight") - k("ArrowLeft")) * DT * 1.8 * authority)
            .rem_euclid(std::f64::consts::TAU);
        if self.bank > std::f64::consts::PI {
            self.bank -= std::f64::consts::TAU;
        }
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
        self.pitch +=
            (self.g * self.bank.cos() - self.pitch.cos()) * 32.174 / self.speed.max(60.) * DT;
        self.pitch = self.pitch.clamp(-1.5, 1.5);
        self.yaw = (self.yaw
            + (self.g * self.bank.sin() * 32.174 / self.speed.max(60.)
                + (k("x") - k("z")) * 0.12 * authority)
                * DT)
            .rem_euclid(std::f64::consts::TAU);
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
        self.speed = (self.speed
            + ((thrust - drag) / weight * 32.174 - 32.174 * self.pitch.sin()) * DT)
            .clamp(0., 6000.);
        self.vertical_speed = self.speed * self.pitch.sin();
        self.position[0] += self.speed * self.pitch.cos() * self.yaw.sin() * DT;
        self.position[2] += self.speed * self.pitch.cos() * self.yaw.cos() * DT;
        self.position[1] += self.vertical_speed * DT;
        let floor = ground(self.position[0], self.position[2]) + 8.;
        if self.position[1] <= floor {
            self.position[1] = floor;
            self.crashed = true;
            self.speed = 0.;
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
    fn profile() -> Aircraft {
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
    fn banked_lift_does_not_create_a_climb_without_a_pull() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]);
        s.bank = std::f64::consts::FRAC_PI_2;
        s.step(&a, &Default::default(), |_, _| 0.);
        assert!(s.pitch < 0. && s.position[1] < 5000.);
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
