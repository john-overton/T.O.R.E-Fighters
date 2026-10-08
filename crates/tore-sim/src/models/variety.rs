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
            // The always-present central nose gear reaches Z=-21, below
            // the switched outriggers at Z=-17. Source scale is 1/3 ft.
            Av8 => 7.,
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
        configuration.aerodynamics.envelopes = fitted_envelopes(a);
        // Transports and airliners: a little more drag, so full power in
        // level flight settles at 96 percent of the top speed, just under
        // the overspeed shake (John, 2026-10-08; the value is an agent fit).
        if matches!(a.id, C130 | Ac130 | E3 | Il76 | E2 | B747 | A310) {
            configuration.aerodynamics.level_speed_fraction = HEAVY_LEVEL_SPEED_FRACTION;
        }
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
/// Share of the 1 G top speed where the variety transports and airliners
/// top out in level flight at full power (fitted, 2026-10-08).
pub const HEAVY_LEVEL_SPEED_FRACTION: f64 = 0.96;

/// A fitted correction to a decoded speed envelope (docs/spec/variety-flight.md,
/// "Top speeds"). Agent decisions, 2026-10-08, from published figures.
#[derive(Clone, Copy, Debug, PartialEq)]
enum SpeedFit {
    /// Multiply every fast-side speed by `fast` and every altitude by `altitude`.
    Scale { fast: f64, altitude: f64 },
    /// Cap the 1 G row's top speed at the true airspeed of the calibrated
    /// VMO schedule `vmo_kcas` ((altitude ft, knots), straight lines between
    /// points, held beyond the ends) and at `mmo`, in the standard
    /// atmosphere; the other rows' fast sides shrink in the same proportion
    /// at each altitude.
    Limits {
        vmo_kcas: &'static [(f64, f64)],
        mmo: f64,
    },
}

fn speed_fit(id: AircraftId) -> Option<SpeedFit> {
    use AircraftId::*;
    Some(match id {
        // af.mil AC-130U fact sheet: 300 mph (261 kt) at sea level; decoded
        // 338 kt there (the C-130's envelope).
        Ac130 => SpeedFit::Scale {
            fast: 261. / 338.,
            altitude: 1.,
        },
        // Decoded as a copy of the AH-64 envelope: 130 kt, 7,000 ft. Published
        // V-22 figures: 275 kt at sea level, 25,000 ft service ceiling.
        V22 => SpeedFit::Scale {
            fast: 275. / 130.,
            altitude: 25_000. / 7_000.,
        },
        // AH-64: maximum level speed 158 kt published; decoded 130 kt.
        Ah64 => SpeedFit::Scale {
            fast: 158. / 130.,
            altitude: 1.,
        },
        // EASA TCDS IM.A.196: 747-400 VMO/MMO 375 KCAS / 0.92.
        B747 => SpeedFit::Limits {
            vmo_kcas: &[(0., 375.)],
            mmo: 0.92,
        },
        // EASA TCDS EASA.A.172: A310-300 VMO 360 KIAS (basic), MMO 0.84.
        A310 => SpeedFit::Limits {
            vmo_kcas: &[(0., 360.)],
            mmo: 0.84,
        },
        // No E-3 or 707-320B limit could be found on its own. FAA TCDS 4A26
        // revision 11, part III (707-300B series, the E-3's 707-320B airframe
        // with JT3D engines, military TF33): VMO 375 kt IAS at sea level,
        // 381 at 10,000 ft, 385 at 15,000, 390 at 20,000, 394 at 23,000;
        // MMO 0.887 at 23,000 ft and above. Requested by John 2026-10-08.
        E3 => SpeedFit::Limits {
            vmo_kcas: &[
                (0., 375.),
                (10_000., 381.),
                (15_000., 385.),
                (20_000., 390.),
                (23_000., 394.),
            ],
            mmo: 0.887,
        },
        _ => return None,
    })
}

/// True airspeed in ft/s at `altitude_ft` for a calibrated airspeed in knots,
/// limited to Mach `mmo`, in the standard atmosphere (subsonic, compressible).
fn limit_speed_fps(altitude_ft: f64, schedule: &[(f64, f64)], mmo: f64) -> f64 {
    let vmo_kcas = match schedule.iter().position(|(h, _)| *h > altitude_ft) {
        Some(0) => schedule[0].1,
        Some(i) => {
            let ((h0, v0), (h1, v1)) = (schedule[i - 1], schedule[i]);
            v0 + (v1 - v0) * (altitude_ft - h0) / (h1 - h0)
        }
        None => schedule.last().map_or(f64::INFINITY, |p| p.1),
    };
    const P0: f64 = 101_325.;
    const A0: f64 = 340.294;
    let Ok(air) = crate::telemetry::Atmosphere::standard(altitude_ft.clamp(-2_000., 100_000.))
    else {
        return f64::INFINITY;
    };
    let calibrated = vmo_kcas * 0.514_444;
    let impact = P0 * ((1. + 0.2 * (calibrated / A0).powi(2)).powf(3.5) - 1.);
    let mach = (5. * ((impact / air.static_pressure_pa + 1.).powf(2. / 7.) - 1.))
        .sqrt()
        .min(mmo);
    let sound = (1.4 * 287.053 * air.temperature_k).sqrt();
    mach * sound / 0.514_444 * 1.68781
}

/// The index of a polygon's first highest vertex. Points up to it are the
/// slow side, the ones after it the fast side (the decoded row layout).
fn top_index(points: &[[f64; 2]]) -> usize {
    let top = points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
    points.iter().position(|p| p[1] == top).unwrap_or(0)
}

/// The aircraft's speed envelopes with its fitted top-speed correction
/// applied, or the decoded ones unchanged. The flight model, the overspeed
/// rule and the envelope window all use these.
pub fn fitted_envelopes(a: &Aircraft) -> Vec<tore_formats::aircraft::Envelope> {
    let mut envelopes = a.envelopes.clone();
    match speed_fit(a.id) {
        None => {}
        Some(SpeedFit::Scale { fast, altitude }) => {
            for e in &mut envelopes {
                let top = top_index(&e.points);
                let slowest = e.points[top][0];
                for (i, p) in e.points.iter_mut().enumerate() {
                    if i > top {
                        p[0] = (p[0] * fast).max(slowest);
                    }
                    p[1] *= altitude;
                }
            }
        }
        Some(SpeedFit::Limits { vmo_kcas, mmo }) => {
            let Some(one) = a.envelopes.iter().find(|e| e.g == 1) else {
                return envelopes;
            };
            let factor = |altitude: f64| {
                one.speeds(altitude).map_or(1., |(_, top)| {
                    (limit_speed_fps(altitude, vmo_kcas, mmo) / top).min(1.)
                })
            };
            for e in &mut envelopes {
                let top = top_index(&e.points);
                // Add fast-side vertices every 5,000 ft up to 35,000 ft so
                // the capped edge follows the calibrated-speed curve.
                let mut points = e.points[..=top].to_vec();
                for pair in e.points[top..].windows(2) {
                    let ([s0, h0], [s1, h1]) = (pair[0], pair[1]);
                    let mut levels: Vec<f64> = (1..8)
                        .map(|k| f64::from(k) * 5_000.)
                        .filter(|h| *h < h0.max(h1) && *h > h0.min(h1))
                        .collect();
                    if h1 < h0 {
                        levels.reverse();
                    }
                    for h in levels {
                        points.push([s0 + (s1 - s0) * (h - h0) / (h1 - h0), h]);
                    }
                    points.push(pair[1]);
                }
                for p in &mut points[top + 1..] {
                    p[0] *= factor(p[1]);
                }
                e.points = points;
            }
            // No higher row may reach past the fitted top speed.
            if let Some(one) = envelopes.iter().find(|e| e.g == 1).cloned() {
                for e in envelopes.iter_mut().filter(|e| e.g != 1) {
                    let top = top_index(&e.points);
                    for p in &mut e.points[top + 1..] {
                        if let Some((_, limit)) = one.speeds(p[1]) {
                            p[0] = p[0].min(limit);
                        }
                    }
                }
            }
        }
    }
    envelopes
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
                fitted_envelopes(&aircraft)
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
    fn fitted_top_speeds_move_only_the_fast_side_and_stay_valid() {
        // Synthetic rows: slow side 200 ft/s at sea level, top at 50,000 ft,
        // fast edge 1,800 ft/s at sea level.
        let one = |a: &Aircraft, h: f64| {
            fitted_envelopes(a)
                .iter()
                .find(|e| e.g == 1)
                .unwrap()
                .speeds(h)
                .unwrap()
        };
        let plain = synthetic(AircraftId::Il76);
        assert_eq!(fitted_envelopes(&plain), plain.envelopes);
        let gunship = synthetic(AircraftId::Ac130);
        let (slow, fast) = one(&gunship, 0.);
        assert_eq!(slow, 200.);
        assert!((fast - 1_800. * 261. / 338.).abs() < 1e-6);
        let osprey = synthetic(AircraftId::V22);
        let top = fitted_envelopes(&osprey)[0]
            .points
            .iter()
            .map(|p| p[1])
            .fold(0., f64::max);
        assert!((top - 50_000. * 25_000. / 7_000.).abs() < 1e-6);
        for id in [AircraftId::B747, AircraftId::A310, AircraftId::E3] {
            let airliner = synthetic(id);
            let fitted = fitted_envelopes(&airliner);
            // Sea level: the published VMO, calibrated equals true.
            let vmo = if id == AircraftId::A310 { 360. } else { 375. };
            assert!((one(&airliner, 0.).1 / 1.68781 - vmo).abs() < 1., "{id:?}");
            // Higher, the calibrated limit is a faster true airspeed.
            assert!(one(&airliner, 10_000.).1 > one(&airliner, 0.).1 * 1.12);
            // Slow sides and every vertex count stay inside the PT contract,
            // and no row reaches past the fitted 1 G edge at its vertices.
            for (e, raw) in fitted.iter().zip(&airliner.envelopes) {
                assert!(e.points.len() <= 20);
                assert_eq!(e.points[..2], raw.points[..2]);
                for p in &e.points {
                    assert!(p[0] <= one(&airliner, p[1]).1 + 1e-6);
                }
            }
            assert!(AircraftModel::for_aircraft(&airliner).is_ok());
        }
    }
    #[test]
    fn heavies_reach_full_thrust_drag_just_under_their_top_speed() {
        use crate::flight::{PilotInput, State};
        for id in AircraftId::SELECTABLE {
            if VarietyFlightModel::identity(id).is_none() {
                continue;
            }
            let heavy = matches!(
                id,
                AircraftId::C130
                    | AircraftId::Ac130
                    | AircraftId::E3
                    | AircraftId::Il76
                    | AircraftId::E2
                    | AircraftId::B747
                    | AircraftId::A310
            );
            let model = AircraftModel::for_aircraft(&synthetic(id)).unwrap();
            let fraction = model.configuration().aerodynamics.level_speed_fraction;
            assert_eq!(
                fraction,
                if heavy {
                    HEAVY_LEVEL_SPEED_FRACTION
                } else {
                    1.
                }
            );
        }
        // Synthetic IL-76: 1 G edge 1,800 ft/s at sea level, loaded drag 0.
        let mut s = State::new(&synthetic(AircraftId::Il76), [0., 0., 0.]).unwrap();
        s.enable_research(1).unwrap();
        let speed = HEAVY_LEVEL_SPEED_FRACTION * 1_800.;
        s.speed = speed;
        s.velocity = [0., 0., speed];
        s.yaw = 0.;
        s.pitch = 0.;
        s.throttle = 1.;
        s.step(&PilotInput::default(), |_, _| -1_000.);
        let t = s.trace().adapter.unwrap();
        let full = 50_000. * t.power.lapse;
        assert!((t.forces.drag.airframe_lbf / full - 1.).abs() < 0.01);
    }
    #[test]
    fn harrier_ground_start_places_the_lowest_central_wheel_on_the_runway() {
        let model = AircraftModel::for_aircraft(&synthetic(AircraftId::Av8)).unwrap();
        let mut state = crate::flight::State::from_model(model, [0., 5000., 0.]);
        state.enable_research(1).unwrap();
        state.start_on_runway([0., 123., 0.], 0.).unwrap();
        assert_eq!(state.position[1], 130.);
        for _ in 0..240 {
            state.step(&crate::flight::PilotInput::default(), |_, _| 123.);
        }
        assert!(!state.crashed);
        assert!((state.position[1] - 7. - 123.).abs() < 0.001);
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
