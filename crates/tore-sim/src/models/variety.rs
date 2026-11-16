//! Exact batch identities with aircraft-owned source configuration and explicit fits.
use super::{
    Conditions, FlightModel, Response, Tuning,
    config::{Configuration, Equipment},
};
use std::sync::Arc;
use tore_formats::aircraft::{Aircraft, AircraftId};

mod b52;
mod lift;
pub use lift::{
    BodyParameters, CorridorPoint, HandlingTargets, JetParameters, LiftEngines, LiftKind,
    PoweredLift, RotorLayout, RotorParameters, RotorRotation, TiltrotorParameters,
};
pub use lift::{RotorcraftAirframe, StubWing, TailRotorParameters, V22_NACELLE_RANGE_DEGREES};
#[derive(Clone, Debug, PartialEq)]
pub struct VarietyFlightModel {
    pub id: AircraftId,
    pub(super) configuration: Arc<Configuration>,
    /// Shared, so cloning the model each tick stays cheap.
    pub lift: Option<Arc<PoweredLift>>,
    /// A helicopter's or the V-22's level cruise power, ft·lbf/s, the AI's
    /// fuel planning figure (slice HF3,
    /// [`crate::flight::powered::fuel::cruise_power`]): trimmed once here.
    pub cruise_power: Option<f64>,
}
impl VarietyFlightModel {
    /// Exact reviewed PT identity, including shared F-4B/J shape references.
    pub fn identity(id: AircraftId) -> Option<(&'static str, &'static str)> {
        use AircraftId::*;
        // A variant (the Tornado IDS (Clean)) is its source's exact PT.
        Some(match id.source() {
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
            Mig25 => ("MiG-25", "MIG25.SH"),
            F16A => ("F-16", "F16.SH"),
            Mig31 => ("MiG-31", "MIG31.SH"),
            M2000 => ("M2000", "M20.SH"),
            F15E => ("F-15E", "F15E.SH"),
            Gripen => ("Gripen", "GRI.SH"),
            F5E => ("F-5", "F5E.SH"),
            Tornado => ("Tornado", "TOR.SH"),
            Mf1 => ("MF1", "MF1.SH"),
            B52 => ("B-52", "B52.SH"),
            B2 => ("B-2", "B2.SH"),
            F117 => ("F-117", "F117.SH"),
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
        let alignment_rate = match a.id.source() {
            // The B-52 takes the transports' 0.4, not the fighters' 0.7 (B52-8).
            C130 | Ac130 | E3 | Il76 | E2 | B52 => 0.4,
            B747 | A310 => 0.3,
            _ => 0.7,
        };
        // Fitted contact plane from reviewed deployed original gear geometry.
        let clearance = match a.id.source() {
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
            // MIG25.SH deployed gear low point Z=-28 at 1/3 ft a unit, the
            // source scale (docs/formats/aircraft-variety.md, MiG-25 review).
            Mig25 => 28. / 3.,
            F16A => 7.,
            // MIG31.SH deployed gear low point Z=-30 at 1/3 ft a unit, the
            // source scale (docs/formats/aircraft-variety.md, MiG-31 review).
            Mig31 => 30. / 3.,
            // M20.SH deployed gear low point Z=-12 at 1/3 ft a unit (gear
            // contact -13 against low -12 in the M2K-0 review), scaled by the
            // shape's height factor (M2K-2: 12 units become 18.06, 6.02 ft).
            M2000 => 12. / 3. * tore_formats::aircraft::scale::shape_axis_scale(a.id)[2],
            // Batch 5 (B5-1): deployed gear low points from the source
            // reviews, in shape units at the shape's own scale, not yet
            // scaled by any real-size fit. F15E.SH -27 and GRI.SH -16 and
            // F5E.SH -18 and MF1.SH -17 at 1/3 ft a unit; TOR.SH -11 at its
            // header scale of 2/3 ft a unit (interim, agent decision
            // 2026-11-10, pending John).
            F15E => 27. / 3.,
            Gripen => 16. / 3.,
            F5E => 18. / 3.,
            Tornado => 11. * 2. / 3.,
            // Update (MF1-2, 2026-11-10): MF1.SH's gear low point Z=-17 is
            // scaled by the shape's height factor (17 units become 17.93,
            // 5.98 ft); the B5-1 note above gave the unscaled 17 units.
            Mf1 => 17. / 3. * tore_formats::aircraft::scale::shape_axis_scale(a.id)[2],
            // Bomber batch A (BMB-A1, 2026-11-15): deployed gear low points
            // from the source reviews, in shape units at the shape's own
            // scale, not yet scaled by any real-size fit. B52.SH -14 at its
            // header scale of 2/3 ft a unit; B2.SH -33 and F117.SH -21 at
            // 1/3 ft a unit.
            B52 => 14. * 2. / 3.,
            B2 => 33. / 3.,
            // Update (F117-2, 2026-11-15): F117.SH's gear low point Z=-21 is
            // scaled by the shape's height factor (21 units become 18.62,
            // 6.21 ft); the note above gave the unscaled 21 units (7 ft).
            F117 => 21. / 3. * tore_formats::aircraft::scale::shape_axis_scale(a.id)[2],
            _ => unreachable!("registered variety identity"),
        };
        let mut tuning = Tuning {
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
        if a.id.source() == B52 {
            b52::fit(&mut tuning, &mut configuration);
        }
        // Transports and airliners: a little more drag, so full power in
        // level flight settles at 96 percent of the top speed, just under
        // the overspeed shake (John, 2026-10-08; the value is an agent fit).
        if matches!(a.id, C130 | Ac130 | E3 | Il76 | E2 | B747 | A310) {
            configuration.aerodynamics.level_speed_fraction = HEAVY_LEVEL_SPEED_FRACTION;
        }
        configuration.hook_available = a.fields["flags"].number()? & 0x02 != 0;
        configuration.controls = Some(super::handling::Profile::from_aircraft(a)?);
        let lift = PoweredLift::for_aircraft(a, &configuration).map(Arc::new);
        let cruise_power = lift
            .as_deref()
            .and_then(|lift| crate::flight::powered::fuel::cruise_power(lift, &configuration));
        Ok(Self {
            id: a.id,
            configuration: Arc::new(configuration),
            lift,
            cruise_power,
        })
    }
}
/// The stall reference fraction of a variety aircraft (see
/// [`super::AircraftModel::stall_reference_fraction`]): 1, an explicit uncalibrated
/// fit, except the B-52H's (B52-8, fitted).
pub fn stall_reference_fraction(id: AircraftId) -> f64 {
    match id.source() {
        AircraftId::B52 => b52::STALL_REFERENCE_FRACTION,
        _ => 1.,
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
    fn the_mirage_2000_gear_low_point_scales_with_its_shape_height_and_no_other_does() {
        let clearance = |id| {
            let model = AircraftModel::for_aircraft(&synthetic(id)).unwrap();
            model.configuration().equipment.ground_clearance_ft
        };
        // 12 shape units at a third of a foot, times the z factor 1.5053.
        let expected = 12. / 3. * tore_formats::aircraft::scale::M2000_SHAPE_SCALE[2];
        assert!((clearance(AircraftId::M2000) - expected).abs() < 1e-12);
        assert!((clearance(AircraftId::M2000) - 6.0212).abs() < 1e-9);
        // The MiG-31 keeps its 30 unit low point at a third of a foot.
        assert_eq!(clearance(AircraftId::Mig31), 30. / 3.);
    }
    /// TOR-2: `TOR.SH` deployed gear low point Z=-11 is at its header scale,
    /// 2/3 ft a unit (7.33 ft), and follows the drawn scale.
    #[test]
    fn the_tornado_gear_low_point_is_at_its_header_scale() {
        let model = AircraftModel::for_aircraft(&synthetic(AircraftId::Tornado)).unwrap();
        let clearance = model.configuration().equipment.ground_clearance_ft;
        assert!(
            (clearance - 11. * crate::combat::debris::scale(AircraftId::Tornado)).abs() < 1e-12
        );
        assert!((clearance - 22. / 3.).abs() < 1e-12);
    }
    /// BMB-A1: the bombers' deployed gear low points (B52.SH -14 at 2/3 ft a
    /// unit, B2.SH -33 and F117.SH -21 at 1/3 ft a unit).
    #[test]
    fn the_bomber_batch_a_gear_low_points_follow_their_source_reviews() {
        for (id, feet) in [
            (AircraftId::B52, 28. / 3.),
            (AircraftId::B2, 11.),
            // F117-2: 7 ft at the plain scale, times the height factor.
            (
                AircraftId::F117,
                7. * tore_formats::aircraft::scale::F117_SHAPE_SCALE[2],
            ),
        ] {
            let model = AircraftModel::for_aircraft(&synthetic(id)).unwrap();
            let clearance = model.configuration().equipment.ground_clearance_ft;
            assert!((clearance - feet).abs() < 1e-12, "{id:?}");
        }
    }
    /// B2-8: the B-2 at its real thrust, weights and ceiling, with the record's
    /// own flight tuning, climbs at about the one published rate (3,000 ft/min,
    /// aerospaceweb) on a bomber's climb speed, and its speed brake slows it as a
    /// pair of drag rudders would, not as a fighter airbrake. Loose pins of the
    /// fitted figures (docs/formats/aircraft-variety.md, "B-2 flight review").
    #[test]
    fn the_b2_climbs_at_a_bombers_rate_and_its_speed_brake_is_a_modest_one() {
        use crate::flight::{PilotInput, State};
        let mut a = synthetic(AircraftId::B2);
        for (key, value) in [
            ("weight", 158_000),
            ("internalFuel", 167_000),
            ("thrust", 69_200),
            ("maxTakeoffWeight", 376_000),
            ("fuelConsumption", 6),
            ("_gpullDrag", 12),
            ("airBrakesDrag", 64),
        ] {
            a.fields.get_mut(key).unwrap().value = value.to_string();
        }
        // The corrected rows: only the 1 G polygon matters to the climb.
        a.envelopes = [
            (
                0,
                vec![
                    [170., 0.],
                    [200., 15_500.],
                    [290., 34_250.],
                    [420., 46_625.],
                    [550., 53_000.],
                    [720., 51_500.],
                    [855., 45_750.],
                    [895., 36_000.],
                    [790., 0.],
                ],
            ),
            (
                1,
                vec![
                    [220., 0.],
                    [250., 15_000.],
                    [350., 33_500.],
                    [460., 44_750.],
                    [560., 50_000.],
                    [700., 48_000.],
                    [800., 43_000.],
                    [840., 36_000.],
                    [750., 0.],
                ],
            ),
            (
                2,
                vec![
                    [260., 0.],
                    [300., 16_000.],
                    [445., 35_000.],
                    [590., 43_000.],
                    [700., 41_000.],
                    [780., 34_000.],
                    [720., 0.],
                ],
            ),
            (
                3,
                vec![
                    [300., 0.],
                    [350., 16_000.],
                    [490., 31_500.],
                    [610., 38_000.],
                    [705., 35_500.],
                    [750., 31_000.],
                    [720., 0.],
                ],
            ),
        ]
        .into_iter()
        .map(|(g, points)| tore_formats::aircraft::Envelope { g, points })
        .collect();
        let model = AircraftModel::for_aircraft(&a).unwrap();
        let ceiling = model
            .configuration()
            .aerodynamics
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .unwrap()
            .points
            .iter()
            .map(|p| p[1])
            .fold(0., f64::max);
        assert_eq!(ceiling, 50_000.);
        const KT: f64 = 1.68781;
        // One step of level flight at `speed_kt` and `altitude` with the brake
        // as given: the thrust, the drag and the weight the model applied.
        let step = |speed_kt: f64, altitude: f64, brake: f64| {
            let mut s = State::from_model(model.clone(), [0., altitude, 0.]);
            s.enable_research(1).unwrap();
            s.speed = speed_kt * KT;
            s.velocity = crate::attitude::Basis::new(0., 0., 0.)
                .forward
                .map(|v| v * s.speed);
            s.throttle = 1.;
            s.brake_out = brake > 0.;
            s.brake = brake;
            s.cheats.unlimited_fuel = true;
            s.step(&PilotInput::default(), |_, _| -60_000.);
            let t = s.trace.0.adapter.expect("hybrid step trace");
            (
                t.power.thrust_lbf,
                t.forces.drag.total_lbf,
                t.forces.weight_lbs,
            )
        };
        // Climb: the specific excess power at 275 kt true at sea level (about
        // 255 kt equivalent at 5,000 ft) at the full 325,000 lb, as a rate. The
        // probe's bomber climb takes about 3,100 ft/min to 5,000 ft; this one
        // step reads 2,456 ft/min (the state's own gear position). The one
        // published rate is 3,000 ft/min (aerospaceweb), so the pin is loose.
        let (thrust, drag, weight) = step(275., 0., 0.);
        assert_eq!(weight, 325_000.);
        assert!((thrust - 69_200.).abs() < 1.);
        let rate = (thrust - drag) * 275. * KT / weight * 60.;
        assert!((2_000. ..4_500.).contains(&rate), "{rate} ft/min");
        // The thrust to weight at the real gross weight is the published 0.205
        // (336,500 lb, Wikipedia).
        assert!((69_200. / 336_500. - 0.205_f64).abs() < 0.002);
        // Speed brake at 312 kt at 1,000 ft: the extra drag is a modest
        // fraction of the weight (fitted 0.14; the template's 256 gave 0.44).
        let (_, clean, w) = step(312., 1_000., 0.);
        let (_, braked, _) = step(312., 1_000., 1.);
        let extra = (braked - clean) / w;
        assert!((0.08..0.22).contains(&extra), "{extra} G");
    }
    /// F117-8: the F-117A at its real thrust, weights and (corrected) record has
    /// no speed brake, so the brake lever adds no drag, and the shared model
    /// gives it a thrust to weight and a climb of a subsonic strike jet. Loose
    /// pins (docs/formats/aircraft-variety.md, "F-117 flight review"): the
    /// published thrust to weight is 0.40 at 52,500 lb (Wikipedia), and the
    /// published climb figures (2,820 to 9,000 ft/min) span a factor of three,
    /// so only a broad band is pinned.
    #[test]
    fn the_f117_has_no_speed_brake_and_climbs_like_a_subsonic_strike_jet() {
        use crate::flight::{PilotInput, State};
        let mut a = synthetic(AircraftId::F117);
        for (key, value) in [
            ("weight", 29_500),
            ("internalFuel", 18_500),
            ("thrust", 21_600),
            ("maxTakeoffWeight", 52_500),
            ("fuelConsumption", 3),
            ("_gpullDrag", 38),
            ("airBrakesDrag", 0),
        ] {
            a.fields.get_mut(key).unwrap().value = value.to_string();
        }
        a.envelopes = [
            (
                0,
                vec![
                    [250., 0.],
                    [265., 16_000.],
                    [327., 29_500.],
                    [418., 38_750.],
                    [570., 47_000.],
                    [715., 47_000.],
                    [897., 42_000.],
                    [961., 36_000.],
                    [1_060., 0.],
                ],
            ),
            (
                1,
                vec![
                    [285., 0.],
                    [305., 17_000.],
                    [367., 30_500.],
                    [458., 37_750.],
                    [570., 45_000.],
                    [715., 45_000.],
                    [847., 40_000.],
                    [891., 36_000.],
                    [1_000., 0.],
                ],
            ),
            (
                2,
                vec![
                    [315., 0.],
                    [335., 17_000.],
                    [392., 25_500.],
                    [481., 30_750.],
                    [570., 34_000.],
                    [710., 35_000.],
                    [891., 34_000.],
                    [980., 0.],
                ],
            ),
            (
                3,
                vec![
                    [350., 0.],
                    [385., 13_000.],
                    [457., 21_500.],
                    [570., 26_000.],
                    [890., 26_000.],
                    [940., 0.],
                ],
            ),
        ]
        .into_iter()
        .map(|(g, points)| tore_formats::aircraft::Envelope { g, points })
        .collect();
        let model = AircraftModel::for_aircraft(&a).unwrap();
        const KT: f64 = 1.68781;
        let step = |speed_kt: f64, altitude: f64, brake: f64| {
            let mut s = State::from_model(model.clone(), [0., altitude, 0.]);
            s.enable_research(1).unwrap();
            s.speed = speed_kt * KT;
            s.velocity = crate::attitude::Basis::new(0., 0., 0.)
                .forward
                .map(|v| v * s.speed);
            s.throttle = 1.;
            s.brake_out = brake > 0.;
            s.brake = brake;
            s.cheats.unlimited_fuel = true;
            s.step(&PilotInput::default(), |_, _| -60_000.);
            let t = s.trace.0.adapter.expect("hybrid step trace");
            (
                t.power.thrust_lbf,
                t.forces.drag.total_lbf,
                t.forces.weight_lbs,
            )
        };
        // The brake lever moves the state but adds no drag at any speed.
        for (kt, ft) in [(295., 1_000.), (413., 1_000.), (475., 36_000.)] {
            let (_, clean, _) = step(kt, ft, 0.);
            let (_, braked, _) = step(kt, ft, 1.);
            assert!((braked - clean).abs() < 1e-6, "{kt} kt at {ft} ft");
        }
        // Thrust to weight at the maximum takeoff weight is the published
        // 0.40 (Wikipedia: 21,600 / 52,500 = 0.41).
        assert!((21_600. / 52_500. - 0.40_f64).abs() < 0.02);
        // Clean at 48,000 lb, 370 kt at sea level: the specific excess power
        // as a climb rate, a loose band across the published 2,820 to 9,000
        // ft/min (the model reads about 8,000 ft/min on this one step).
        let (thrust, drag, weight) = step(370., 0., 0.);
        assert_eq!(weight, 48_000.);
        let rate = (thrust - drag) * 370. * KT / weight * 60.;
        assert!((3_000. ..11_000.).contains(&rate), "{rate} ft/min");
    }
    /// The B-52H's flight terms (B52-8, 2026-11-15): a B-52 built from
    /// synthetic values near the corrected record (real thrust, weights and
    /// fuel, a placard-like 1 G top edge of 385 kt at sea level and 515 kt at
    /// 36,000 ft), against the published figures the terms were fitted to,
    /// loosely: a climb of 3,600 ft/min or more at sea level and a cruise burn
    /// that gives a range near the published 7,600 nautical miles.
    fn b52_like() -> Aircraft {
        let mut a = synthetic(AircraftId::B52);
        for (key, value) in [
            ("thrust", "136000"),
            ("weight", "185000"),
            ("internalFuel", "312197"),
            ("maxTakeoffWeight", "488000"),
            ("fuelConsumption", "20"),
            ("loadedDrag", "0"),
        ] {
            a.fields.get_mut(key).unwrap().value = value.into();
        }
        for e in &mut a.envelopes {
            e.points = vec![
                [210., 0.],
                [350., 41_250.],
                [590., 55_000.],
                [770., 55_000.],
                [870., 36_000.],
                [650., 0.],
            ];
        }
        a
    }
    fn fly_b52_level(
        altitude: f64,
        kt: f64,
        fuel: f64,
        seconds: usize,
        hold: bool,
    ) -> crate::flight::State {
        use crate::flight::{DT, PilotInput, State};
        let mut s = State::new(&b52_like(), [0., altitude, 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.fuel = fuel;
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s.speed = kt * 1.68781;
        s.velocity = [0., 0., s.speed];
        s.throttle = 0.4;
        let target = s.speed;
        for _ in 0..(seconds as f64 / DT) as usize {
            let throttle = if hold {
                (s.throttle + (target - s.speed) * 0.01 * DT).clamp(0.05, 1.)
            } else {
                1.
            };
            let input = PilotInput {
                pitch: (0.004 * (altitude - s.position[1])
                    - 0.04 * s.velocity[1]
                    - 0.5 * s.pitch_rate)
                    .clamp(-1., 1.),
                throttle: Some(throttle),
                ..Default::default()
            };
            s.step_surface(&input, |_, _| crate::research::Surface::runway(-60_000.));
        }
        s
    }
    #[test]
    fn the_b52_flies_its_fitted_terms_and_no_other_aircraft_does() {
        for id in AircraftId::SELECTABLE {
            if VarietyFlightModel::identity(id).is_none() {
                continue;
            }
            let model = AircraftModel::for_aircraft(&synthetic(id)).unwrap();
            let c = model.configuration();
            let b52 = id.source() == AircraftId::B52;
            assert_eq!(c.aerodynamics.drag_shape.is_some(), b52, "{id:?}");
            assert_eq!(model.stall_reference_fraction() != 1., b52, "{id:?}");
            assert_eq!(c.tuning.thrust_lapse_feet != 70_000., b52, "{id:?}");
            // The transports and the B-52 turn their noses at 0.4 a second.
            assert_eq!(
                c.tuning.alignment_rate == 0.4,
                matches!(
                    id.source(),
                    AircraftId::C130
                        | AircraftId::Ac130
                        | AircraftId::E3
                        | AircraftId::Il76
                        | AircraftId::E2
                        | AircraftId::B52
                ),
                "{id:?}"
            );
        }
        // The fitted drag is 1 at the reference speed like the square law,
        // and well under it below.
        let model = AircraftModel::for_aircraft(&synthetic(AircraftId::B52)).unwrap();
        let a = &model.configuration().aerodynamics;
        assert!((a.drag_ratio(1.) - 1.).abs() < 1e-12);
        assert!(
            (0.1..0.3).contains(&a.drag_ratio(0.78)),
            "{}",
            a.drag_ratio(0.78)
        );
        assert!(
            (0.15..0.4).contains(&a.drag_ratio(0.857)),
            "{}",
            a.drag_ratio(0.857)
        );
        assert!(a.drag_ratio(0.5) < 0.25 * 0.5 * 0.5 + 1e-9);
    }
    #[test]
    fn the_b52_climbs_cruises_and_tops_out_near_the_published_figures() {
        // Sea level, 300 kt, 400,000 lb: published rate of climb 6,270 ft/min
        // (105 ft/s); specific excess power from the speed gained in level
        // flight, loosely between 60 and 180 ft/s (the model gives about 150).
        let before = fly_b52_level(0., 300., 215_000., 1, false);
        let after = fly_b52_level(0., 300., 215_000., 4, false);
        let gained = (after.speed - before.speed) / 3.;
        let ps = 0.5 * (after.speed + before.speed) * gained / 32.174;
        assert!((60. ..180.).contains(&ps), "excess power {ps} ft/s");
        // Mach 0.77 at 36,000 ft (441 kt) holding speed: the burn gives a
        // range of 6,000 to 9,000 nautical miles from 312,197 lb (published
        // combat range about 7,600).
        let cruise = fly_b52_level(36_000., 441., 215_000., 400, true);
        let flow = 20. * cruise.throttle;
        let range_nm = 312_197. / flow * 441. / 3600.;
        assert!((6_000. ..9_000.).contains(&range_nm), "range {range_nm} nm");
        // Full power at 36,000 ft settles at the record's top-speed edge,
        // unchanged by the fit.
        let top = fly_b52_level(36_000., 441., 215_000., 300, false);
        assert!(
            (top.speed / 1.68781 - 515.).abs() < 20.,
            "{}",
            top.speed / 1.68781
        );
    }
    #[test]
    fn the_mirage_f1_gear_low_point_scales_with_its_shape_height() {
        let model = AircraftModel::for_aircraft(&synthetic(AircraftId::Mf1)).unwrap();
        let clearance = model.configuration().equipment.ground_clearance_ft;
        // 17 shape units at a third of a foot, times the z factor 1.0546.
        let expected = 17. / 3. * tore_formats::aircraft::scale::MF1_SHAPE_SCALE[2];
        assert!((clearance - expected).abs() < 1e-12);
        assert!((clearance - 5.9761).abs() < 1e-3);
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
