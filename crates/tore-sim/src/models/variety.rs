//! Exact batch identities with aircraft-owned source configuration and explicit fits.
use super::{
    Conditions, FlightModel, Response, Tuning,
    config::{Configuration, Equipment},
};
use std::sync::Arc;
use tore_formats::aircraft::{Aircraft, AircraftId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiftKind {
    VectorJet,
    Tiltrotor,
    Helicopter,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoweredLift {
    pub kind: LiftKind,
    pub efficiency: f64,
    pub additional_lift_lbf: f64,
    pub response_seconds: f64,
    pub pitch_degrees: f64,
    pub bank_degrees: f64,
    pub yaw_degrees_per_second: f64,
    pub horizontal_damping: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct VarietyFlightModel {
    pub id: AircraftId,
    pub(super) configuration: Arc<Configuration>,
    pub lift: Option<PoweredLift>,
}
impl VarietyFlightModel {
    /// Exact reviewed PT identity, including shared F-4B/J shape references.
    pub fn identity(id: AircraftId) -> Option<(&'static str, &'static str)> {
        use AircraftId::*;
        Some(match id {
            C130 => ("C-130", "C130.SH"),
            Ac130 => ("AC-130U", "AC130.SH"),
            E3 => ("E-3", "AWACS.SH"),
            Il76 => ("IL-76", "IL76.SH"),
            E2 => ("E-2C", "E2C.SH"),
            Av8 => ("Av-8", "AV8.SH"),
            Yak141 => ("Yak-141", "Y141.SH"),
            V22 => ("V-22", "V22.SH"),
            Ah64 => ("AH-64", "APA.SH"),
            Mi24 => ("Mi-24", "HIND.SH"),
            Ch47 => ("CH-47", "CH47.SH"),
            Mig17 => ("MiG-17", "M17.SH"),
            F4B => ("F-4B", "F4J.SH"),
            F4J => ("F-4J", "F4J.SH"),
            F4E => ("F-4", "F4E.SH"),
            F4G => ("F-4G", "F4.SH"),
            A7 => ("A-7", "A7.SH"),
            F15 => ("F-15", "F15.SH"),
            F16C => ("F-16C", "F16.SH"),
            F104 => ("F-104", "F104.SH"),
            A10 => ("A-10", "A10.SH"),
            B747 => ("B747", "B747.SH"),
            A310 => ("A310", "A310.SH"),
            _ => return None,
        })
    }
    pub fn from_aircraft(a: &Aircraft) -> tore_formats::Result<Self> {
        let Some((name, shape)) = Self::identity(a.id) else {
            return Err(std::io::Error::other("unregistered variety aircraft"));
        };
        if a.name != name || a.shape != shape {
            return Err(std::io::Error::other(
                "variety aircraft identity does not match its exact PT",
            ));
        }
        use AircraftId::*;
        let alignment_rate = match a.id {
            C130 | Ac130 | E3 | Il76 | E2 => 0.4,
            B747 | A310 => 0.3,
            _ => 0.7,
        };
        // Fitted contact plane from reviewed deployed original gear geometry.
        let clearance = match a.id {
            C130 => 14.,
            Ac130 => 40. / 3.,
            E3 => 38. / 3.,
            Il76 => 50. / 3.,
            E2 => 28. / 3.,
            Av8 => 17. / 3.,
            Yak141 => 7.,
            V22 => 13.,
            Ah64 => 23. / 3.,
            Mi24 => 10.,
            Ch47 => 29. / 3.,
            Mig17 => 14. / 3.,
            F4B | F4J => 20. / 3.,
            F4E | F4G => 7.,
            A7 => 25. / 3.,
            F15 => 9.,
            F16C => 7.,
            F104 => 13. / 3.,
            A10 => 22. / 3.,
            B747 => 20.,
            A310 => 14.,
            _ => unreachable!("registered variety identity"),
        };
        let tuning = Tuning {
            legacy_roll_limit_rad_per_second: 1.8,
            roll_response_seconds: 0.2,
            pitch_response_seconds: 0.1,
            alignment_rate,
            rudder_rate: 0.12,
            sideslip_drag: 0.5,
            sideslip_force: 0.8,
            sideslip_roll: 0.35,
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
            ground_clearance_ft: clearance,
        };
        let mut configuration = Configuration::from_aircraft(a, tuning, equipment)?;
        configuration.hook_available = a.fields["flags"].number()? & 0x02 != 0;
        configuration.controls = Some(super::handling::Profile::from_aircraft(a)?);
        let lift = match a.id {
            Av8 => Some((LiftKind::VectorJet, 1., 0., 0.35, 20., 25., 35., 0.06)),
            Yak141 => Some((LiftKind::VectorJet, 1., 18000., 0.45, 20., 25., 30., 0.07)),
            V22 => Some((LiftKind::Tiltrotor, 1.10, 0., 0.65, 20., 25., 30., 0.10)),
            Ah64 => Some((LiftKind::Helicopter, 0.98, 0., 0.45, 20., 30., 45., 0.14)),
            Mi24 => Some((LiftKind::Helicopter, 0.84, 0., 0.60, 18., 25., 35., 0.12)),
            Ch47 => Some((LiftKind::Helicopter, 0.245, 0., 0.85, 15., 20., 25., 0.10)),
            _ => None,
        }
        .map(
            |(
                kind,
                efficiency,
                additional_lift_lbf,
                response_seconds,
                pitch_degrees,
                bank_degrees,
                yaw_degrees_per_second,
                horizontal_damping,
            )| PoweredLift {
                kind,
                efficiency,
                additional_lift_lbf,
                response_seconds,
                pitch_degrees,
                bank_degrees,
                yaw_degrees_per_second,
                horizontal_damping,
            },
        );
        Ok(Self {
            id: a.id,
            configuration: Arc::new(configuration),
            lift,
        })
    }
}
impl FlightModel for VarietyFlightModel {
    fn configuration(&self) -> &Configuration {
        &self.configuration
    }
    fn tuning(&self) -> Tuning {
        self.configuration.tuning
    }
    fn response(&self, c: Conditions) -> Response {
        let t = self.tuning();
        Response {
            trim_aoa_rad: ((t.trim_degrees + t.pull_aoa_degrees_per_g * (c.load_factor - 1.))
                * (450. * 1.68781 / c.tas_fps.max(150.)).powi(2))
            .clamp(-12., 20.)
            .to_radians(),
            thrust_lapse: (-c.altitude_msl_ft.max(0.) / t.thrust_lapse_feet).exp(),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::models::AircraftModel;
    pub fn synthetic(id: AircraftId) -> Aircraft {
        let mut a = crate::flight::integration_tests::profile();
        let (name, shape) = VarietyFlightModel::identity(id).unwrap();
        a.id = id;
        a.name = name.into();
        a.shape = shape.into();
        a.fields.get_mut("thrust").unwrap().value = "50000".into();
        a.fields.get_mut("aftThrust").unwrap().value = "0".into();
        a
    }
    #[test]
    fn all_variety_identities_resolve_owned_source_values() {
        for id in AircraftId::SELECTABLE {
            if VarietyFlightModel::identity(id).is_none() {
                continue;
            }
            let aircraft = synthetic(id);
            let mut model = AircraftModel::for_aircraft(&aircraft).unwrap();
            let original = model.clone();
            assert_eq!(model.configuration().mass.empty_lbs, 10000.);
            assert_eq!(model.configuration().mass.internal_fuel_lbs, 1000.);
            assert_eq!(model.configuration().propulsion.military_thrust_lbf, 50000.);
            assert_eq!(
                model.configuration().aerodynamics.envelopes,
                aircraft.envelopes
            );
            let mut config = model.configuration().clone();
            config.mass.internal_fuel_lbs = 500.;
            config.tuning.trim_degrees = 3.;
            model.set_configuration(config).unwrap();
            assert_eq!(original.configuration().mass.internal_fuel_lbs, 1000.);
            assert_eq!(original.tuning().trim_degrees, 2.);
            let mut wrong = synthetic(id);
            wrong.shape = "F18.SH".into();
            assert!(AircraftModel::for_aircraft(&wrong).is_err());
        }
    }
    #[test]
    fn f4_variants_preserve_their_independent_source_fuel_and_thrust() {
        let mut early = synthetic(AircraftId::F4B);
        early.fields.get_mut("thrust").unwrap().value = "23260".into();
        let mut later = synthetic(AircraftId::F4G);
        later.fields.get_mut("thrust").unwrap().value = "23620".into();
        later.fields.get_mut("internalFuel").unwrap().value = "900".into();
        let early = AircraftModel::for_aircraft(&early).unwrap();
        let later = AircraftModel::for_aircraft(&later).unwrap();
        assert_eq!(early.configuration().propulsion.military_thrust_lbf, 23260.);
        assert_eq!(later.configuration().propulsion.military_thrust_lbf, 23620.);
        assert_eq!(early.configuration().mass.internal_fuel_lbs, 1000.);
        assert_eq!(later.configuration().mass.internal_fuel_lbs, 900.);
        assert_eq!(
            early.configuration().controls,
            later.configuration().controls
        );
    }
    #[test]
    fn conventional_variety_flies_without_powered_lift_and_remains_deterministic() {
        for id in AircraftId::SELECTABLE {
            if VarietyFlightModel::identity(id).is_none() {
                continue;
            }
            let model = AircraftModel::for_aircraft(&synthetic(id)).unwrap();
            if model.powered_lift().is_some() {
                continue;
            }
            let mut state = crate::flight::State::from_model(model, [0., 5000., 0.]);
            state.enable_research(1).unwrap();
            let mut copy = state.clone();
            let input = crate::flight::PilotInput {
                roll: 0.1,
                yaw: 0.05,
                ..Default::default()
            };
            for _ in 0..1200 {
                state.step(&input, |_, _| 0.);
                copy.step(&input, |_, _| 0.);
            }
            assert_eq!(state, copy, "{id:?}");
            assert!(!state.crashed, "{id:?}");
            assert!(state.position[2].abs() > 100., "{id:?}");
            assert!(state.fuel < 1000., "{id:?}");
        }
    }
}
