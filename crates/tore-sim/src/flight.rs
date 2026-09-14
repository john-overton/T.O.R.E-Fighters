//! Deterministic 120 Hz free-flight adapter. PT facts are recovered; integration is authored.
use crate::attitude::{Basis, cross, dot, unit};
use crate::models::{Conditions, FlightModel};
use std::collections::BTreeSet;
use tore_formats::aircraft::Aircraft;
pub const DT: f64 = 1.0 / 120.0;
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    model: crate::models::AircraftModel,
    pub research: Option<crate::research::Research>,
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
    pub payload_lbs: f64,
    pub engine: bool,
    pub burner: bool,
    pub exhaust: f64,
    pub rudder: f64,
    pub elevator: f64,
    pub aileron: f64,
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
    pub fn new(a: &Aircraft, position: [f64; 3]) -> tore_formats::Result<Self> {
        Ok(Self::from_model(
            crate::models::AircraftModel::for_aircraft(a)?,
            position,
        ))
    }
    pub fn model(&self) -> &crate::models::AircraftModel {
        &self.model
    }
    /// Configure a model before constructing a flight; live fuel/payload remain state.
    pub fn from_model(model: crate::models::AircraftModel, position: [f64; 3]) -> Self {
        let fuel = model.configuration().mass.internal_fuel_lbs;
        Self {
            model,
            research: None,
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
            fuel,
            payload_lbs: 0.,
            engine: true,
            burner: false,
            exhaust: 0.,
            rudder: 0.,
            elevator: 0.,
            aileron: 0.,
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
        result.gear = lerp(previous.gear, self.gear);
        result.flaps = lerp(previous.flaps, self.flaps);
        result.brake = lerp(previous.brake, self.brake);
        result.hook = lerp(previous.hook, self.hook);
        result.exhaust = lerp(previous.exhaust, self.exhaust);
        result.rudder = lerp(previous.rudder, self.rudder);
        result.elevator = lerp(previous.elevator, self.elevator);
        result.aileron = lerp(previous.aileron, self.aileron);
        result
    }
    pub fn afterburner_active(&self) -> bool {
        self.engine
            && self.fuel > 0.
            && self.burner
            && self.throttle > self.model.configuration().equipment.afterburner_throttle
            && !self.crashed
    }
    /// The reviewed Rafale game model has no hook control.
    pub fn hook_available(&self) -> bool {
        matches!(self.model, crate::models::AircraftModel::F18(_))
    }
    pub fn toggle(&mut self, key: &str) {
        match key {
            "g" => self.gear_down = !self.gear_down,
            "f" => self.flaps_down = !self.flaps_down,
            "b" => self.brake_out = !self.brake_out,
            "h" if self.hook_available() => self.hook_down = !self.hook_down,
            "e" => self.engine = !self.engine,
            "t" => self.burner = !self.burner,
            "r" => self.radar = !self.radar,
            "j" => self.jammer = !self.jammer,
            _ => {}
        }
    }
    pub fn step(&mut self, keys: &BTreeSet<String>, ground: impl Fn(f64, f64) -> f64) {
        self.step_surface(keys, |x, z| crate::research::Surface::terrain(ground(x, z)));
    }
    /// Mass-only payload API until loadout/weapon release is integrated.
    pub fn set_payload(&mut self, pounds: f64) -> tore_formats::Result<()> {
        if !pounds.is_finite()
            || pounds < 0.
            || pounds + self.fuel + self.model.configuration().mass.empty_lbs
                > self.model.configuration().mass.max_takeoff_lbs
        {
            return Err(std::io::Error::other(
                "payload outside aircraft mass limits",
            ));
        }
        self.payload_lbs = pounds;
        Ok(())
    }
    pub fn enable_research(&mut self, seed: i32) -> tore_formats::Result<()> {
        self.research = Some(crate::research::Research::new(seed)?);
        Ok(())
    }
    pub fn step_surface(
        &mut self,
        keys: &BTreeSet<String>,
        ground: impl Fn(f64, f64) -> crate::research::Surface,
    ) {
        let model = self.model.clone();
        let c = model.configuration();
        if self.crashed {
            return;
        }
        let wind = if self.research.is_some() {
            ground(self.position[0], self.position[2]).wind
        } else {
            [0.; 3]
        };
        for (v, w) in self.velocity.iter_mut().zip(wind) {
            *v -= w;
        }
        self.ticks += 1;
        let k = |s: &str| f64::from(keys.contains(s));
        self.throttle = (self.throttle
            + (k("PageUp") - k("PageDown")) * DT * c.equipment.throttle_rate_per_second)
            .clamp(0., 1.);
        for (v, on) in [
            (&mut self.gear, self.gear_down),
            (&mut self.flaps, self.flaps_down),
            (&mut self.brake, self.brake_out),
            (&mut self.hook, self.hook_down),
        ] {
            *v = (*v + (if on { 1. } else { -1. }) * DT / c.equipment.deployment_seconds)
                .clamp(0., 1.);
        }
        if self.fuel <= 0. {
            self.engine = false;
            self.burner = false;
        }
        let ab = self.afterburner_active() && c.propulsion.afterburner_thrust_lbf > 0.;
        let target = f64::from(ab);
        self.exhaust = (self.exhaust
            + (target - self.exhaust).clamp(
                -DT / c.equipment.exhaust_seconds,
                DT / c.equipment.exhaust_seconds,
            ))
        .clamp(0., 1.);
        self.rudder += ((k("x") - k("z")) - self.rudder) * (DT / c.equipment.control_seconds);
        self.elevator +=
            ((k("ArrowDown") - k("ArrowUp")) - self.elevator) * (DT / c.equipment.control_seconds);
        self.aileron += ((k("ArrowRight") - k("ArrowLeft")) - self.aileron)
            * (DT / c.equipment.control_seconds);
        let rate = if ab {
            c.propulsion.afterburner_fuel_lbs_per_second
        } else {
            c.propulsion.military_fuel_lbs_per_second * self.throttle
        };
        if self.engine {
            self.fuel = (self.fuel - rate * DT).max(0.);
        }
        let env = c.aerodynamics.envelopes.iter().find(|e| e.g == 1).unwrap();
        let (stall, vmax) = env.speeds(self.position[1]).unwrap_or((900., 1000.));
        let authority = (self.speed / stall.max(1.)).powi(2).clamp(0., 1.);
        let (mut lo, mut hi) = (-1., 1.);
        for e in &c.aerodynamics.envelopes {
            if let Some((low, high)) = e.speeds(self.position[1])
                && self.speed >= low
                && self.speed <= high
            {
                lo = f64::min(lo, e.g as f64);
                hi = f64::max(hi, e.g as f64);
            }
        }
        let loading = (self.fuel + self.payload_lbs) / c.mass.empty_lbs;
        hi /= 1. + loading * c.aerodynamics.loaded_elevator_percent / 100.;
        lo /= 1. + loading * c.aerodynamics.loaded_elevator_percent / 100.;
        let mut command = (1.
            + (k("ArrowDown") - k("ArrowUp"))
                * if keys.contains("ArrowDown") {
                    hi - 1.
                } else {
                    1. - lo
                })
        .clamp(lo, hi)
            * authority;
        if let Some(r) = &mut self.research {
            r.advance(
                c,
                self.speed,
                stall,
                self.elevator,
                self.rudder,
                self.throttle,
                self.bank,
                self.roll_rate,
            );
            if r.spinning != 0 {
                command *= 0.15;
            }
        }
        self.g += (command - self.g) * (DT * 4.).min(1.);
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let roll_limit = if self.research.is_some() {
            c.aerodynamics.roll_limit_rad_per_second.clamp(0.1, 6.)
        } else {
            c.tuning.legacy_roll_limit_rad_per_second
        };
        let tuning = model.tuning();
        let roll_command = (k("ArrowRight") - k("ArrowLeft")) * roll_limit * authority;
        self.roll_rate += (roll_command - self.roll_rate) * (DT / tuning.roll_response_seconds);
        let pitch_command = (command - basis.up[1]) * 32.174 / self.speed.max(60.);
        self.pitch_rate += (pitch_command - self.pitch_rate) * (DT / tuning.pitch_response_seconds);
        // Authored trim target, not decoded gpullAOA units. Preserve a positive
        // nose/flight-path separation under load rather than aligning to zero AoA.
        let direction = unit(self.velocity);
        let along = dot(basis.up, direction);
        let lift_axis = unit(std::array::from_fn(|i| basis.up[i] - along * direction[i]));
        let response = model.response(Conditions {
            altitude_msl_ft: self.position[1],
            tas_fps: self.speed,
            load_factor: self.g,
        });
        let alpha = response.trim_aoa_rad;
        let desired_nose =
            std::array::from_fn(|i| direction[i] * alpha.cos() + lift_axis[i] * alpha.sin());
        let alignment = cross(basis.forward, desired_nose);
        // The gravity component across aircraft-right contributes to body yaw
        // as the flight path turns. Pitch alone misses this during a banked pull.
        let turn_yaw = -basis.right[1] * 32.174 / self.speed.max(60.);
        let mut rotation = std::array::from_fn(|i| {
            DT * (-basis.right[i] * self.pitch_rate - basis.forward[i] * self.roll_rate
                + basis.up[i] * (turn_yaw + (k("x") - k("z")) * tuning.rudder_rate * authority)
                + alignment[i] * tuning.alignment_rate * authority)
        });
        if let Some(r) = &self.research {
            if r.on_ground {
                for (i, v) in rotation.iter_mut().enumerate() {
                    *v += basis.up[i] * self.rudder * 0.3 * (self.speed / 40.).clamp(0., 1.) * DT;
                }
            }
            if r.spinning != 0 {
                // Fitted stable rotating descent coupled to independent momentum.
                for (i, v) in rotation.iter_mut().enumerate() {
                    *v = DT * (basis.up[i] * r.spin_yaw_rate(c) + basis.right[i] * 0.15);
                }
            }
        }
        let basis = basis.rotated(rotation);
        [self.yaw, self.pitch, self.bank] = basis.angles();
        let max_thrust = c
            .propulsion
            .afterburner_thrust_lbf
            .max(c.propulsion.military_thrust_lbf);
        let lapse = response.thrust_lapse;
        let thrust = if self.engine {
            if ab {
                max_thrust
            } else {
                c.propulsion.military_thrust_lbf * self.throttle
            }
        } else {
            0.
        } * lapse;
        let weight = c.mass.empty_lbs + self.fuel + self.payload_lbs;
        // Drag normalized against the source 1G upper envelope. This is not the native force law.
        let drag = max_thrust
            * lapse
            * (self.speed / vmax.max(100.)).powi(2)
            * (1. + loading * c.aerodynamics.loaded_drag_percent / 100.)
            + weight
                * (c.aerodynamics.g_pull_drag_f8 * (self.g.abs() - 1.).max(0.)
                    + c.native.drag.gear as f64 * self.gear
                    + c.native.drag.flaps as f64 * self.flaps
                    + c.native.drag.airbrake as f64 * self.brake)
                / 256.;
        let drag = if self.research.is_some() {
            drag.min(weight * self.speed / 32.174 / DT)
        } else {
            drag
        };
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
        for (v, w) in self.velocity.iter_mut().zip(wind) {
            *v += w;
        }
        self.vertical_speed = self.velocity[1];
        for i in 0..3 {
            self.position[i] += self.velocity[i] * DT;
        }
        let surface = ground(self.position[0], self.position[2]);
        if let Some(mut r) = self.research.take() {
            r.contact(self, surface, c);
            self.research = Some(r);
            return;
        }
        let floor = surface.height + c.equipment.ground_clearance_ft;
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
        let mut previous = State::new(&a, [0.; 3]).unwrap();
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
pub(crate) mod integration_tests {
    use super::*;
    use std::collections::BTreeMap;
    use tore_formats::aircraft::{Envelope, Token};
    fn base_profile() -> Aircraft {
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
            ("loadedDrag", 0),
            ("_gpullDrag", 0),
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
            id: tore_formats::aircraft::AircraftId::F18,
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
    pub(crate) fn profile() -> Aircraft {
        let mut a = base_profile();
        for key in [
            "rudderDrag",
            "bayDrag",
            "wheelBrakesDrag",
            "stallWarningDelay",
            "stallDelay",
            "stallSeverity",
            "stallPitchDown",
            "spinEntry",
            "spinExit",
            "spinYawLow",
            "spinYawHigh",
            "spinAOALow",
            "spinAOAHigh",
            "spinBankLow",
            "spinBankHigh",
            "crashSpeedForward",
            "crashSpeedSide",
            "crashSpeedVertical",
            "crashPitch",
            "crashRoll",
            "flags",
        ] {
            a.fields.insert(
                key.into(),
                Token {
                    kind: "word".into(),
                    value: "0".into(),
                    scaled: false,
                },
            );
        }
        for key in ["gearDrag", "flapsDrag", "airBrakesDrag"] {
            a.fields.get_mut(key).unwrap().kind = "word".into();
        }
        for axis in ["x", "y", "z"] {
            for suffix in ["min", "max", "acc", "dacc"] {
                a.fields.insert(
                    format!("_bv.{axis}.{suffix}"),
                    Token {
                        kind: "word".into(),
                        value: if suffix == "min" { "-100" } else { "100" }.into(),
                        scaled: false,
                    },
                );
            }
        }
        for (key, value) in [
            ("_brv.x.max", 100),
            ("crashSpeedForward", 330),
            ("crashSpeedSide", 50),
            ("crashSpeedVertical", 30),
            ("crashPitch", 25),
            ("crashRoll", 10),
            ("spinExit", -2),
        ] {
            a.fields.insert(
                key.into(),
                Token {
                    kind: "word".into(),
                    value: value.to_string(),
                    scaled: false,
                },
            );
        }
        a
    }
    #[test]
    fn telemetry_separates_air_ground_and_altitude_datums() {
        use crate::telemetry::{AirData, Atmosphere, EnvironmentReading};
        let mut s = State::new(&profile(), [0., 10000., 0.]).unwrap();
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s.velocity = [0., 10., 600.];
        let mut e = EnvironmentReading {
            terrain_msl_ft: 2500.,
            wind_world_fps: [0., 0., 100.],
            atmosphere: Atmosphere::standard(10000.).unwrap(),
        };
        let d = AirData::sample(&s, e).unwrap();
        assert_eq!(d.altitude_msl_ft, 10000.);
        assert_eq!(d.altitude_asl_ft(), 10000.);
        assert_eq!(d.altitude_agl_ft, 7500.);
        assert_eq!(d.vertical_speed_fpm, 600.);
        assert!(d.ground_speed_knots > d.true_airspeed_knots);
        assert!(d.equivalent_airspeed_knots < d.true_airspeed_knots);
        assert!(d.mach > 0.4 && d.mach < 0.6);
        assert!(d.angle_of_attack_deg.unwrap() < 0.);
        assert_eq!(d.sideslip_deg, Some(0.));
        assert!(d.indicated_airspeed_knots.is_none() && d.indicated_altitude_ft.is_none());
        e.atmosphere = Atmosphere::standard(0.).unwrap();
        let sea = AirData::sample(&s, e).unwrap();
        assert!((sea.equivalent_airspeed_knots - sea.true_airspeed_knots).abs() < 1e-9);
        s.velocity = e.wind_world_fps;
        assert!(
            AirData::sample(&s, e)
                .unwrap()
                .angle_of_attack_deg
                .is_none()
        );
        e.atmosphere.temperature_k = 0.;
        assert!(AirData::sample(&s, e).is_err());
        assert!(Atmosphere::standard(f64::NAN).is_err());
    }
    #[test]
    fn hybrid_wind_is_advection_and_payload_is_bounded() {
        let a = profile();
        let mut calm = State::new(&a, [0., 5000., 0.]).unwrap();
        calm.enable_research(1).unwrap();
        let mut windy = calm.clone();
        windy.velocity[0] += 40.;
        let mut surface = crate::research::Surface::runway(0.);
        surface.wind = [40., 0., 0.];
        for _ in 0..1200 {
            calm.step_surface(&Default::default(), |_, _| {
                crate::research::Surface::runway(0.)
            });
            windy.step_surface(&Default::default(), |_, _| surface);
        }
        assert!((windy.position[0] - calm.position[0] - 400.).abs() < 1e-7);
        assert!((windy.speed - calm.speed).abs() < 1e-7);
        assert!(calm.set_payload(f64::NAN).is_err());
        assert!(calm.set_payload(-1.).is_err());
        assert!(calm.set_payload(100000.).is_err());
        calm.set_payload(1000.).unwrap();
        assert_eq!(calm.payload_lbs, 1000.);
        let mut light = State::new(&a, [0., 5000., 0.]).unwrap();
        light.enable_research(1).unwrap();
        light.throttle = 1.;
        light.burner = true;
        let mut heavy = light.clone();
        heavy.set_payload(3000.).unwrap();
        for _ in 0..600 {
            light.step(&Default::default(), |_, _| 0.);
            heavy.step(&Default::default(), |_, _| 0.);
        }
        assert!(
            heavy.speed < light.speed,
            "payload must reduce acceleration"
        );
    }
    #[test]
    fn hybrid_pause_render_rate_and_ground_replay() {
        let a = profile();
        let mut initial = State::new(&a, [0., 5000., 0.]).unwrap();
        initial.enable_research(1).unwrap();
        let mut states = Vec::new();
        for hz in [30, 60, 144] {
            let mut s = initial.clone();
            let mut c = Clock { remainder: 0. };
            for _ in 0..hz * 3 {
                for _ in 0..c.steps(1. / hz as f64) {
                    s.step(&Default::default(), |_, _| 0.);
                }
            }
            states.push(s);
        }
        assert_eq!(states[0], states[1]);
        assert_eq!(states[1], states[2]);
        let before = initial.clone();
        let _ = initial.presented(&before, 0.5);
        assert_eq!(initial, before);
        initial.position[1] = 8.;
        initial.velocity = [0., -1., 100.];
        initial.yaw = 0.;
        initial.gear = 1.;
        initial.gear_down = true;
        initial.throttle = 0.;
        for _ in 0..1200 {
            initial.step_surface(&Default::default(), |_, _| {
                crate::research::Surface::runway(0.)
            });
        }
        assert!(!initial.crashed && initial.research.as_ref().unwrap().on_ground);
        assert!(initial.velocity[2] >= 0. && initial.speed < 100.);
    }
    #[test]
    fn momentum_and_roll_response_survive_control_release() {
        let a = profile();
        let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
        let keys = BTreeSet::from(["ArrowDown".into(), "ArrowRight".into()]);
        for _ in 0..60 {
            s.step(&keys, |_, _| 0.);
        }
        let nose = Basis::new(s.yaw, s.pitch, s.bank).forward;
        assert!(dot(nose, unit(s.velocity)) < 0.99999);
        let rate = s.roll_rate;
        s.step(&Default::default(), |_, _| 0.);
        assert!(s.roll_rate > 0. && s.roll_rate < rate);
        assert_eq!(s.vertical_speed, s.velocity[1]);
        assert!(s.velocity.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn sustained_pull_can_complete_a_loop() {
        let a = profile();
        let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
        s.throttle = 1.;
        s.burner = true;
        let start = Basis::new(s.yaw, s.pitch, s.bank).forward;
        let keys = BTreeSet::from(["ArrowDown".into()]);
        let (mut vertical, mut inverted, mut completed) = (false, false, false);
        for _ in 0..120 * 90 {
            s.step(&keys, |_, _| 0.);
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
        let mut s = State::new(&a, [0., 5000., 0.]).unwrap();
        s.bank = std::f64::consts::FRAC_PI_2;
        s.step(&Default::default(), |_, _| 0.);
        assert!(s.velocity[1] < 0. && s.position[1] < 5000.);
    }
    #[test]
    fn simulation_is_identical_at_different_render_rates() {
        let a = profile();
        let start = State::new(&a, [0., 5000., 0.]).unwrap();
        let mut result = Vec::new();
        for hz in [30, 60, 144] {
            let mut s = start.clone();
            let mut c = Clock { remainder: 0. };
            let keys = ["ArrowDown".to_string(), "ArrowRight".into()].into();
            for _ in 0..hz * 3 {
                for _ in 0..c.steps(1. / hz as f64) {
                    s.step(&keys, |_, _| 0.);
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
        let mut s = State::new(&a, [0., 5000., 0.]).unwrap();
        s.fuel = 0.;
        s.burner = true;
        s.step(&Default::default(), |_, _| 0.);
        assert!(!s.engine && !s.burner);
        s.position[1] = 0.;
        s.step(&Default::default(), |_, _| 0.);
        assert!(s.crashed);
        let final_state = s.clone();
        s.step(&Default::default(), |_, _| 0.);
        assert_eq!(s, final_state);
    }
    #[test]
    fn burner_consumes_more_fuel_and_actuators_reach_endpoints() {
        let a = profile();
        let mut dry = State::new(&a, [0., 5000., 0.]).unwrap();
        dry.throttle = 1.;
        let mut wet = dry.clone();
        wet.burner = true;
        wet.gear_down = true;
        for _ in 0..400 {
            dry.step(&Default::default(), |_, _| 0.);
            wet.step(&Default::default(), |_, _| 0.);
        }
        assert!(wet.fuel < dry.fuel);
        assert_eq!(wet.gear, 1.);
        wet.gear_down = false;
        for _ in 0..400 {
            wet.step(&Default::default(), |_, _| 0.);
        }
        assert_eq!(wet.gear, 0.);
    }
    #[test]
    fn animation_travel_reverses_and_presentation_interpolates() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]).unwrap();
        s.gear_down = true;
        s.flaps_down = true;
        s.brake_out = true;
        s.hook_down = true;
        for _ in 0..180 {
            s.step(&Default::default(), |_, _| 0.);
        }
        for value in [s.gear, s.flaps, s.brake, s.hook] {
            assert!((value - 0.5).abs() < 1e-8);
        }
        let previous = s.clone();
        s.gear_down = false;
        s.flaps_down = false;
        s.brake_out = false;
        s.hook_down = false;
        s.step(&Default::default(), |_, _| 0.);
        let render = s.presented(&previous, 0.5);
        assert!(s.gear < render.gear && render.gear < previous.gear);
        for _ in 0..180 {
            s.step(&Default::default(), |_, _| 0.);
        }
        assert_eq!([s.gear, s.flaps, s.brake, s.hook], [0.; 4]);
    }
    #[test]
    fn exhaust_and_controls_respond_then_settle() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]).unwrap();
        s.throttle = 1.;
        s.burner = true;
        let keys = ["ArrowDown", "ArrowRight", "x"].map(String::from).into();
        for _ in 0..30 {
            s.step(&keys, |_, _| 0.);
        }
        assert_eq!(s.exhaust, 1.);
        assert!(s.afterburner_active());
        assert!(s.elevator > 0.9 && s.aileron > 0.9 && s.rudder > 0.9);
        s.throttle = 0.95;
        assert!(!s.afterburner_active());
        for _ in 0..120 {
            s.step(&Default::default(), |_, _| 0.);
        }
        assert_eq!(s.exhaust, 0.);
        assert!(s.elevator.abs() < 0.001 && s.rudder.abs() < 0.001);
        s.throttle = 1.;
        s.engine = false;
        assert!(!s.afterburner_active());
        s.engine = true;
        s.fuel = 0.;
        assert!(!s.afterburner_active());
    }
    #[test]
    fn banked_pulls_retain_aoa_and_mirror_left_right() {
        let a = profile();
        let mut outputs = Vec::new();
        for bank in [-45f64, 45.] {
            let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
            s.bank = bank.to_radians();
            let keys = ["ArrowDown".to_string()].into();
            for _ in 0..360 {
                s.step(&keys, |_, _| 0.);
            }
            let body = Basis::new(s.yaw, s.pitch, s.bank);
            let forward = dot(s.velocity, body.forward);
            let side = dot(s.velocity, body.right);
            let up = dot(s.velocity, body.up);
            let alpha = (-up).atan2(forward);
            let beta = side.atan2(forward.hypot(up));
            assert!(
                alpha > 2f64.to_radians() && alpha < 20f64.to_radians(),
                "alpha={alpha}"
            );
            assert!(beta.abs() < 1f64.to_radians(), "beta={beta}");
            outputs.push((alpha, beta, s.position[1], s.speed));
        }
        assert!((outputs[0].0 - outputs[1].0).abs() < 1e-9);
        assert!((outputs[0].1 + outputs[1].1).abs() < 1e-9);
        assert!((outputs[0].2 - outputs[1].2).abs() < 1e-9);
        assert!((outputs[0].3 - outputs[1].3).abs() < 1e-9);
    }
    #[test]
    fn roll_in_and_pull_preserve_lateral_flight_path_lag() {
        let a = profile();
        let mut sides = Vec::new();
        for roll in ["ArrowLeft", "ArrowRight"] {
            let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
            let keys = [roll.to_string(), "ArrowDown".to_string()].into();
            for _ in 0..90 {
                s.step(&keys, |_, _| 0.);
            }
            let b = Basis::new(s.yaw, s.pitch, s.bank);
            sides.push(dot(s.velocity, b.right));
            assert!(dot(s.velocity, b.up) < -1.);
        }
        assert!(sides[0].abs() > 0.1, "lateral lag {sides:?}");
        assert!((sides[0] + sides[1]).abs() < 1e-9);
    }
}

/// Reproducible probes of reviewed helper translations using the imported PT.
/// Deliberately separate from the authored 120 Hz integrator.
pub fn native_report(a: &Aircraft) -> tore_formats::Result<()> {
    use tore_formats::flight_model as n;
    let word = |key: &str| -> tore_formats::Result<i16> {
        let token = a
            .fields
            .get(key)
            .ok_or_else(|| std::io::Error::other(format!("missing PT {key}")))?;
        i16::try_from(token.number()?)
            .map_err(|_| std::io::Error::other(format!("invalid word {key}")))
    };
    let coefficient = word("gpullAOA")?;
    println!(
        "native_helpers_v2 aircraft={} method=static_translation complete_model=false",
        a.name
    );
    println!(
        "gpullAOA={coefficient} lowAOASpeed={} lowAOAPitch={}",
        word("lowAOASpeed")?,
        word("lowAOAPitch")?
    );
    for g in [-3, 0, 1, 3, 6, 9] {
        let mut aoa = 0;
        for _ in 0..128 {
            aoa = n::pull_aoa(aoa, g * 256, coefficient, 2);
        }
        println!(
            "g={g} pull_offset_after_1s_deg={:.6} turn_at_750fps_deg_s={:.6}",
            aoa as f64 / 256.,
            n::g_to_turn(g * 256, 750)? as f64 / 256.
        );
    }
    let envelope = a
        .envelopes
        .iter()
        .find(|e| e.g == 1)
        .ok_or_else(|| std::io::Error::other("missing 1G envelope"))?;
    for altitude in [0, 5000, 20000, 36000, 50000] {
        let limits = n::envelope_limits(
            envelope,
            altitude * 256,
            false,
            [word("structure[0]")?, word("structure[1]")?],
        )?;
        let upper = i16::try_from(limits.maximum)
            .map_err(|_| std::io::Error::other("native upper speed overflow"))?;
        println!(
            "altitude_ft={altitude} min_fps={} max_fps={} structure_fps={} drag_percent_at_750fps={}",
            limits.minimum,
            limits.maximum,
            limits.structural,
            n::drag_percent(750 * 256, altitude * 256, upper)?
        );
    }
    for throttle in [0, 50, 100, 101] {
        println!(
            "throttle={throttle} fuel_rate_fixed8={}",
            n::fuel_rate(
                word("fuelConsumption")?,
                word("aftFuelConsumption")?,
                throttle
            )
        );
    }
    let profile = n::profile::FlightProfile::from_fields(&a.fields)?;
    println!("native_profile={profile:?}");
    let mut stall = n::departure::StallState::default();
    // A supplied below-envelope condition, not a complete simulated trajectory.
    for sample in 0..5 {
        stall.advance(
            &profile.departure,
            true,
            true,
            profile.extended_warning,
            false,
            256,
        )?;
        println!(
            "departure_condition_sample={sample} mode={:?} elapsed={} severity={}",
            stall.mode,
            stall.elapsed,
            n::departure::stall_severity(&profile.departure, stall.elapsed, 150, 200)?
        );
    }
    for direction in [-1, 1] {
        let mut state = n::departure::SpinState::entered(direction, false)?;
        let mut motion = n::departure::SpinMotion {
            speed_f8: 350 * 256,
            ..Default::default()
        };
        let input = n::departure::SpinInput {
            pitch_stick: 256,
            rudder: direction as i32 * 256,
            throttle_f8: 100 * 256,
            speed_f8: 350 * 256,
            clean_stall_fps: 200,
            thrust_vector_f8: 0,
            inhibited: false,
        };
        for _ in 0..128 {
            state.advance(&mut motion, &profile.departure, input, 2)?;
        }
        println!("spin_component_direction={direction} state={state:?} motion={motion:?}");
    }
    // FA 0x452482..0x4524d6 writes the 1G envelope maximum into cp+0x245.
    let upper = n::envelope_limits(
        envelope,
        5000 * 256,
        false,
        [word("structure[0]")?, word("structure[1]")?],
    )?
    .maximum;
    let velocity_limits =
        profile.loaded_velocity(i16::try_from(upper).map_err(std::io::Error::other)?)?;
    println!("velocity_probe_forward_limit={upper} source=reviewed_1G_envelope_update_at_5000ft");
    let velocity = n::integration::Velocity {
        forward: 100 * 256,
        side: 5 * 256,
        down: 0,
    };
    let forces = n::integration::Forces {
        drag: 100_000,
        forward: 500_000,
        side: 0,
        down: 0,
    };
    for ground in [false, true] {
        println!(
            "velocity_component_ground={ground} result={:?}",
            n::integration::velocity_step(
                velocity,
                forces,
                30_000,
                velocity_limits,
                ground,
                ground,
                2
            )?
        );
    }
    Ok(())
}

/// Imported-table probes; no executable loader or emulation involved.
pub fn native_rotation_report(
    a: &Aircraft,
    table: &tore_formats::flight_model::rotation::TrigTable,
) -> tore_formats::Result<()> {
    use tore_formats::flight_model::{forces as f, rotation as r};
    println!(
        "native_rotation_v1 table=caller_supplied_data provenance=see_extractor_manifest complete_model=false"
    );
    for pitch in [0, 45, 80, 90, -90] {
        let angle = r::degrees_to_pa(pitch * 256)?;
        let trig = table.sin_cos(angle);
        let rates = r::body_rates(table, [0, 5 * 256, 0], r::degrees_to_pa(45 * 256)?, angle)?;
        println!("pitch_deg={pitch} pa={angle} trig={trig:?} rates_f8={rates:?}");
    }
    let word = |key: &str| -> tore_formats::Result<i16> {
        a.fields
            .get(key)
            .ok_or_else(|| std::io::Error::other(format!("missing {key}")))?
            .number()
            .and_then(|v| i16::try_from(v).map_err(std::io::Error::other))
    };
    for bank in [-45, 0, 45, 180] {
        let gravity = f::gravity_force(
            30000,
            table.sin_cos(0),
            table.sin_cos(r::degrees_to_pa(-bank * 256)?),
        )?;
        let lift = f::lift_force(
            f::LiftInput {
                speed_f8: 750 * 256,
                first_envelope_speed: 200,
                stall_fps: 213,
                lift_scale_f8: 256,
                flaps_lift: word("flapsLift")?,
                drag_percent: 62,
                weight: 30000,
            },
            f::DragDevices::default(),
        )?;
        let force = f::assemble(0, [0, 0], lift, gravity);
        println!(
            "force_probe_bank={bank} supplied_weight=30000 gravity={gravity:?} lift={lift} forward={} side={} down={}",
            force.forward, force.side, force.down
        );
    }
    Ok(())
}
