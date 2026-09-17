//! MiG-29-owned fitted laws. Source configuration with an independently owned shared response fit.
//! Shared initial fit is an agent choice documented in docs/spec/roster-aircraft.md.
use super::{
    Conditions, FlightModel, Response, Tuning,
    config::{Configuration, Equipment},
};
use std::sync::Arc;
#[derive(Clone, Debug, PartialEq)]
pub struct Mig29FlightModel {
    pub(super) configuration: Arc<Configuration>,
}
impl Mig29FlightModel {
    pub fn from_aircraft(a: &tore_formats::aircraft::Aircraft) -> tore_formats::Result<Self> {
        let supported = a.id == tore_formats::aircraft::AircraftId::Mig29
            && a.name == "MiG-29"
            && a.shape == "MIG29.SH";
        if !supported {
            return Err(std::io::Error::other(
                "aircraft identity does not match Mig29FlightModel",
            ));
        }
        let tuning = Tuning {
            legacy_roll_limit_rad_per_second: 1.8,
            roll_response_seconds: 0.2,
            pitch_response_seconds: 0.1,
            alignment_rate: 0.7,
            rudder_rate: 0.12,
            sideslip_drag: 0.5,
            trim_degrees: 2.,
            pull_aoa_degrees_per_g: 1.25,
            thrust_lapse_feet: 70000.,
            tire_scrub_rate: 8.,
            rolling_deceleration: 0.8,
            brake_deceleration: 18.,
        };
        let equipment = Equipment {
            deployment_seconds: 3.,
            exhaust_seconds: 0.2,
            control_seconds: 0.1,
            throttle_rate_per_second: 0.35,
            afterburner_throttle: 0.95,
            ground_clearance_ft: 6.666666666667,
        };
        Ok(Self {
            configuration: Arc::new(Configuration::from_aircraft(a, tuning, equipment)?),
        })
    }
}
impl FlightModel for Mig29FlightModel {
    fn configuration(&self) -> &Configuration {
        &self.configuration
    }
    fn tuning(&self) -> Tuning {
        self.configuration.tuning
    }
    fn response(&self, c: Conditions) -> Response {
        let t = self.configuration.tuning;
        Response {
            trim_aoa_rad: ((t.trim_degrees + t.pull_aoa_degrees_per_g * (c.load_factor - 1.))
                * (450. * 1.68781 / c.tas_fps.max(150.)).powi(2))
            .clamp(-12., 20.)
            .to_radians(),
            thrust_lapse: (-c.altitude_msl_ft.max(0.) / t.thrust_lapse_feet).exp(),
        }
    }
}
