//! Aircraft-owned flight laws and editable, validated tuning. No renderer types.
pub mod config;
pub mod f18;
pub mod rafale_c;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tuning {
    pub legacy_roll_limit_rad_per_second: f64,
    pub roll_response_seconds: f64,
    pub pitch_response_seconds: f64,
    pub alignment_rate: f64,
    pub rudder_rate: f64,
    /// Fitted drag/weight per squared lateral airspeed fraction.
    pub sideslip_drag: f64,
    pub trim_degrees: f64,
    pub pull_aoa_degrees_per_g: f64,
    pub thrust_lapse_feet: f64,
    pub tire_scrub_rate: f64,
    pub rolling_deceleration: f64,
    pub brake_deceleration: f64,
}
impl Tuning {
    pub fn validate(self) -> tore_formats::Result<Self> {
        let positive = [
            self.legacy_roll_limit_rad_per_second,
            self.roll_response_seconds,
            self.pitch_response_seconds,
            self.thrust_lapse_feet,
        ];
        let nonnegative = [
            self.alignment_rate,
            self.rudder_rate,
            self.sideslip_drag,
            self.trim_degrees,
            self.pull_aoa_degrees_per_g,
            self.tire_scrub_rate,
            self.rolling_deceleration,
            self.brake_deceleration,
        ];
        if positive.iter().any(|v| !v.is_finite() || *v < 1. / 120.)
            || nonnegative.iter().any(|v| !v.is_finite() || *v < 0.)
        {
            return Err(std::io::Error::other("invalid aircraft tuning"));
        }
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Conditions {
    pub altitude_msl_ft: f64,
    pub tas_fps: f64,
    pub load_factor: f64,
}
#[derive(Clone, Copy, Debug)]
pub struct Response {
    pub trim_aoa_rad: f64,
    pub thrust_lapse: f64,
}
pub trait FlightModel {
    fn configuration(&self) -> &config::Configuration;
    fn response(&self, input: Conditions) -> Response;
    fn tuning(&self) -> Tuning;
}
#[derive(Clone, Debug, PartialEq)]
pub enum AircraftModel {
    F18(f18::F18FlightModel),
    RafaleC(rafale_c::RafaleCFlightModel),
}
impl AircraftModel {
    pub fn for_aircraft(a: &tore_formats::aircraft::Aircraft) -> tore_formats::Result<Self> {
        match (a.name.as_str(), a.shape.as_str()) {
            ("F/A-18D", "F18.SH") => Ok(Self::F18(f18::F18FlightModel::from_aircraft(a)?)),
            ("RAFALE", "RAF.SH") => Ok(Self::RafaleC(rafale_c::RafaleCFlightModel::from_aircraft(
                a,
            )?)),
            #[cfg(test)]
            ("Synthetic", "TEST.SH") => Ok(Self::F18(f18::F18FlightModel::from_aircraft(a)?)),
            _ => Err(std::io::Error::other(
                "aircraft has no registered flight model",
            )),
        }
    }
    pub fn set_tuning(&mut self, tuning: Tuning) -> tore_formats::Result<()> {
        let mut c = self.configuration().clone();
        c.tuning = tuning;
        self.set_configuration(c)
    }
    /// Validate before replacing; clones and other aircraft retain their configuration.
    pub fn set_configuration(
        &mut self,
        configuration: config::Configuration,
    ) -> tore_formats::Result<()> {
        configuration.validate()?;
        let configuration = std::sync::Arc::new(configuration);
        match self {
            Self::F18(m) => m.configuration = configuration,
            Self::RafaleC(m) => m.configuration = configuration,
        }
        Ok(())
    }
}
impl FlightModel for AircraftModel {
    fn configuration(&self) -> &config::Configuration {
        match self {
            Self::F18(m) => m.configuration(),
            Self::RafaleC(m) => m.configuration(),
        }
    }
    fn response(&self, c: Conditions) -> Response {
        match self {
            Self::F18(m) => m.response(c),
            Self::RafaleC(m) => m.response(c),
        }
    }
    fn tuning(&self) -> Tuning {
        match self {
            Self::F18(m) => m.tuning(),
            Self::RafaleC(m) => m.tuning(),
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn model() -> AircraftModel {
        AircraftModel::for_aircraft(&crate::flight::integration_tests::profile()).unwrap()
    }
    #[test]
    fn configuration_rejects_missing_fields_and_invalid_mods_atomically() {
        let mut source = crate::flight::integration_tests::profile();
        source.fields.remove("thrust");
        assert!(AircraftModel::for_aircraft(&source).is_err());
        let mut source = crate::flight::integration_tests::profile();
        source.fields.remove("turbulencePercent");
        assert!(AircraftModel::for_aircraft(&source).is_err());
        let mut model = model();
        let original = model.clone();
        for change in 0..6 {
            let mut c = model.configuration().clone();
            match change {
                0 => c.mass.empty_lbs = f64::NAN,
                1 => c.mass.max_takeoff_lbs = c.mass.empty_lbs,
                2 => c.aerodynamics.envelopes.retain(|e| e.g != 1),
                3 => c.equipment.control_seconds = 0.,
                4 => c.native.landing.descent_fps = -1,
                _ => c.turbulence_percent = -1,
            }
            assert!(model.set_configuration(c).is_err());
            assert_eq!(model, original);
        }
    }
    #[test]
    fn model_configuration_drives_mass_propulsion_fuel_and_devices() {
        use crate::flight::State;
        let original = model();
        let mut modified = original.clone();
        let mut c = modified.configuration().clone();
        c.mass.internal_fuel_lbs = 500.;
        c.mass.max_takeoff_lbs = 12000.;
        c.propulsion.military_thrust_lbf *= 2.;
        c.propulsion.military_fuel_lbs_per_second = 4.;
        c.equipment.deployment_seconds = 1.;
        modified.set_configuration(c).unwrap();
        assert_eq!(original.configuration().mass.internal_fuel_lbs, 1000.);
        for researched in [false, true] {
            let mut base = State::from_model(original.clone(), [0., 15000., 0.]);
            let mut tuned = State::from_model(modified.clone(), base.position);
            assert_eq!(tuned.fuel, 500.);
            assert!(tuned.set_payload(1501.).is_err());
            assert!(tuned.set_payload(1500.).is_ok());
            tuned.set_payload(0.).unwrap();
            base.gear_down = true;
            tuned.gear_down = true;
            if researched {
                base.enable_research(1).unwrap();
                tuned.enable_research(1).unwrap();
            }
            for _ in 0..120 {
                base.step(&Default::default(), |_, _| 0.);
                tuned.step(&Default::default(), |_, _| 0.);
            }
            assert!(tuned.speed > base.speed);
            assert!((tuned.fuel - 497.2).abs() < 1e-8);
            assert!(tuned.gear > 0.99 && base.gear < 0.34);
        }
    }
    #[test]
    fn configured_contact_limits_reach_research_state() {
        use crate::{flight::State, research::Surface};
        let original = model();
        let mut modified = original.clone();
        let mut c = modified.configuration().clone();
        c.native.landing.descent_fps = 1;
        modified.set_configuration(c).unwrap();
        let mut safe = State::from_model(original, [0., 8.01, 0.]);
        let mut unsafe_landing = State::from_model(modified, safe.position);
        for s in [&mut safe, &mut unsafe_landing] {
            s.enable_research(1).unwrap();
            s.gear = 1.;
            s.gear_down = true;
            s.yaw = 0.;
            s.velocity = [0., -5., 280.];
            s.speed = 280.;
            s.step_surface(&Default::default(), |_, _| Surface::runway(0.));
        }
        assert!(!safe.crashed);
        assert!(unsafe_landing.crashed);
    }
    #[test]
    fn models_can_be_tuned_independently() {
        let mut hornet = AircraftModel::F18(
            f18::F18FlightModel::from_aircraft(&crate::flight::integration_tests::profile())
                .unwrap(),
        );
        let rafale = AircraftModel::RafaleC(
            rafale_c::RafaleCFlightModel::from_aircraft(
                &crate::flight::integration_tests::profile(),
            )
            .unwrap(),
        );
        let c = Conditions {
            altitude_msl_ft: 10000.,
            tas_fps: 700.,
            load_factor: 3.,
        };
        let before = rafale.response(c).trim_aoa_rad;
        let mut t = hornet.tuning();
        t.pull_aoa_degrees_per_g = 2.5;
        hornet.set_tuning(t).unwrap();
        assert_ne!(hornet.response(c).trim_aoa_rad, before);
        assert_eq!(rafale.response(c).trim_aoa_rad, before);
        t.pitch_response_seconds = 0.;
        assert!(hornet.set_tuning(t).is_err());
    }
}
