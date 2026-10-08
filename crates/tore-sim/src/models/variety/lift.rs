//! The powered-lift parameters of the six VTOL aircraft: the AV-8 and Yak-141
//! vectoring jets, the V-22 tiltrotor and the AH-64, Mi-24 and CH-47
//! helicopters (VTOL overhaul design, section 8).
//!
//! Each value says where it comes from: **PT** the aircraft's own record,
//! **Pub** a public figure verified on 2026-10-08 (design section 8.3),
//! **Derived** computed from Pub figures, **Man** the retail manual, **Fit**
//! an agent's starting value, tuned later against the design's acceptance
//! numbers. All are agent decisions unless a line says otherwise.
//!
//! One table per kind, so the slices that build each kind edit only their
//! own: [`rotor`] (helicopters, P2 and P3; the V-22's rotors, P5), [`jet`]
//! (P4) and [`tiltrotor`] (P5). [`body`] and [`targets`] are shared.

use crate::models::config::Configuration;
use tore_formats::aircraft::{Aircraft, AircraftId};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiftKind {
    VectorJet,
    Tiltrotor,
    Helicopter,
}

/// Everything the powered-lift flight model knows about one aircraft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PoweredLift {
    pub kind: LiftKind,
    // The fitted attitude-hold law of the variety import (docs/spec/
    // variety-flight.md, "Powered lift and controls"). Slices P2 and P4
    // replace that law and retire these fields.
    pub efficiency: f64,
    pub additional_lift_lbf: f64,
    pub response_seconds: f64,
    pub pitch_degrees: f64,
    pub bank_degrees: f64,
    pub yaw_degrees_per_second: f64,
    pub horizontal_damping: f64,
    /// The rigid body every kind flies on.
    pub body: BodyParameters,
    /// The rotor system: helicopters and the V-22.
    pub rotor: Option<RotorParameters>,
    /// Nozzles and lift engines: the vectoring jets.
    pub jet: Option<JetParameters>,
    /// Nacelles, rotor speed schedule and conversion corridor: the V-22.
    pub tiltrotor: Option<TiltrotorParameters>,
    /// Handling targets the stability levels and fits aim at.
    pub targets: HandlingTargets,
}

/// Mass distribution. The inertia about each axis is the current mass times
/// the radius squared, so it follows fuel and stores.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodyParameters {
    /// Radii of gyration about the roll, pitch and yaw axes, ft. Derived from
    /// the UH-60A (helicopters) and F-16 (jets) published inertias, scaled
    /// by length and span, then Fit (design 8.2, "Handling").
    pub radii_of_gyration_ft: [f64; 3],
}

/// Which way a single main rotor turns, seen from above.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RotorRotation {
    /// American convention: torque yaws the nose right.
    CounterClockwise,
    /// Mil convention: torque yaws the nose left.
    Clockwise,
}

/// How the rotors are arranged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RotorLayout {
    /// One main rotor and a tail rotor at `tail_rotor_arm_ft` behind the
    /// centre of gravity.
    Single {
        rotation: RotorRotation,
        tail_rotor_arm_ft: f64,
    },
    /// Two counter-rotating rotors, fore and aft, `hub_spacing_ft` apart.
    Tandem { hub_spacing_ft: f64 },
    /// Two counter-rotating rotors side by side on nacelles,
    /// `hub_spacing_ft` apart.
    SideBySide { hub_spacing_ft: f64 },
}

impl RotorLayout {
    /// How many main rotors: one, or two.
    pub fn rotors(self) -> usize {
        match self {
            Self::Single { .. } => 1,
            Self::Tandem { .. } | Self::SideBySide { .. } => 2,
        }
    }
}

/// One aircraft's rotor system (design 4.4 to 4.8). Every rotor of an
/// aircraft is the same.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotorParameters {
    pub layout: RotorLayout,
    /// Rotor radius, ft.
    pub radius_ft: f64,
    /// Rotor speed at 100 percent, rpm.
    pub rotor_speed_rpm: f64,
    /// Blade area over disk area.
    pub solidity: f64,
    /// Maximum static thrust of all the rotors together at sea level, lbf:
    /// the PT thrust where plausible, else Fit.
    pub max_thrust_lbf: f64,
    /// Engine cut in a hover at fixed collective: seconds for the rotor
    /// speed to fall from 100 to 80 percent (Fit).
    pub energy_seconds: f64,
    /// Never-exceed speed, kt (true for the helicopters; KCAS in airplane
    /// mode for the V-22). Retreating blade stall starts here at the
    /// reference blade loading.
    pub never_exceed_kt: f64,
    /// Blade collective pitch at the bottom and the top of the lever,
    /// degrees (Fit).
    pub collective_degrees: [f64; 2],
    /// Disk tilt from the shaft at full stick, [longitudinal, lateral],
    /// degrees (Fit, with the Lock number, to the hover rate targets).
    pub cyclic_degrees: [f64; 2],
    /// Disk tilt away from the in-plane airflow per unit advance ratio,
    /// rad: the blowback that gives speed stability (Fit).
    pub blowback: f64,
    /// Lock number: the rotor's rate damping is a disk lag of
    /// `16 / (lock_number x rotor speed)` seconds times the body rate (Fit).
    pub lock_number: f64,
    /// Hub height above the centre of gravity, ft (Fit).
    pub hub_height_ft: f64,
    /// Hub moment per radian of disk tilt, beyond the thrust's own lever
    /// arm, as a multiple of the reference weight times the hub height
    /// (Fit).
    pub hub_stiffness: f64,
    /// Blade profile drag coefficient (Fit).
    pub profile_drag: f64,
    /// Ground effect: induced velocity times `1 - (R / (k z))²` at hub
    /// height `z`; this is `k` (Fit; the textbook value is 4).
    pub ground_effect_constant: f64,
    /// Peak rise of the induced velocity in the vortex ring state above the
    /// hover value, as a share of it, at a descent of one hover induced
    /// velocity (Fit to the shape of NACA TN-2474, design 4.4).
    pub vortex_ring_rise: f64,
    /// The anti-torque tail rotor: single main rotor only.
    pub tail_rotor: Option<TailRotorParameters>,
    /// Fuselage, tail surfaces and stub wings.
    pub airframe: RotorcraftAirframe,
}

/// A single-rotor helicopter's tail rotor (design 4.5). It sits at the
/// layout's tail rotor arm behind the centre of gravity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TailRotorParameters {
    /// Radius, ft.
    pub radius_ft: f64,
    /// Tip speed at 100 percent rotor speed, ft/s (Fit).
    pub tip_speed_fps: f64,
    /// Blade area over disk area (Fit).
    pub solidity: f64,
    /// Thrust change at full pedal as a share of the thrust that balances
    /// the hover torque at the reference weight (Fit).
    pub pedal_authority: f64,
}

/// A rotorcraft's fuselage and fixed surfaces (design 4.4 and 4.5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotorcraftAirframe {
    /// Equivalent flat-plate drag areas along the body [forward, side,
    /// vertical] axes, ft² (Fit: the forward area to the level top speed).
    pub flat_plate_ft2: [f64; 3],
    /// Horizontal tail: pitch moment per radian of angle of attack per unit
    /// dynamic pressure, ft³ (tail area x lift slope x arm; Fit).
    pub tail_pitch_ft3: f64,
    /// The angle of attack at which the horizontal tail carries no load,
    /// degrees (Fit).
    pub tail_trim_degrees: f64,
    /// Vertical fin: yaw moment per radian of sideslip per unit dynamic
    /// pressure, ft³ (Fit).
    pub fin_yaw_ft3: f64,
    /// Share of the reference hover torque the cambered fin carries at
    /// 120 kt, growing with dynamic pressure (Fit).
    pub fin_torque_share: f64,
    /// Half the main wheel track, ft: the lever the weight has against a
    /// roll about the wheels on the ground (Fit).
    pub half_track_ft: f64,
    /// Stub wings that carry lift at speed: the Mi-24.
    pub stub_wing: Option<StubWing>,
}

/// Stub wings (design 4.5: the Mi-24's carry up to a quarter of the lift
/// at speed).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StubWing {
    /// Wing area times lift slope, ft² per radian (Fit).
    pub lift_area_ft2: f64,
    /// Incidence to the fuselage datum, degrees (Fit).
    pub incidence_degrees: f64,
    /// Stall angle of attack, degrees: the lift holds there (Fit).
    pub stall_degrees: f64,
}

impl RotorParameters {
    /// Tip speed at 100 percent rotor speed, ft/s.
    pub fn tip_speed_fps(&self) -> f64 {
        self.rotor_speed_rpm * std::f64::consts::TAU / 60. * self.radius_ft
    }
    /// Area of one rotor's disk, ft².
    pub fn disk_area_ft2(&self) -> f64 {
        std::f64::consts::PI * self.radius_ft * self.radius_ft
    }
}

/// The vectoring jets' nozzles, engine spool and lift engines (design 4.7).
/// The puffer jets' rates come from the PT's `puffRot` fields, through the
/// handling profile's auxiliary axes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JetParameters {
    /// Nozzle travel from aft (0) to the braking stop, degrees: PT
    /// `vtLimitDown` (-100) and the manual.
    pub nozzle_range_degrees: f64,
    /// Nozzle slew rate, degrees per second: PT `vtSpeed` (unit inferred).
    pub nozzle_rate_degrees_per_second: f64,
    /// Share of main thrust left with the nozzles vertical (Fit: the AV-8
    /// takes off vertically unloaded but cannot hover at combat weight; the
    /// Yak-141 just hovers clean).
    pub vertical_efficiency: f64,
    /// Engine spool lags up and down near hover power, seconds (Fit).
    pub spool_up_seconds: f64,
    pub spool_down_seconds: f64,
    /// The Yak-141's lift engines.
    pub lift_engines: Option<LiftEngines>,
}

/// Lift engines that run only for takeoff and landing (Yak-141).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiftEngines {
    /// Thrust of both together, lbf (the variety import's fit; published
    /// 2 x 9,040).
    pub thrust_lbf: f64,
    /// Spool-in time, seconds (Fit).
    pub spool_seconds: f64,
    /// They start when the main nozzle passes this angle with the main
    /// engine running, degrees (Fit).
    pub start_nozzle_degrees: f64,
    /// They stop when the nozzle returns below this angle, degrees (Fit).
    pub stop_nozzle_degrees: f64,
    /// They stop above this airspeed, kt (Fit).
    pub stop_speed_kt: f64,
}

/// One row of the V-22 conversion corridor: indicated airspeed limits at a
/// nacelle angle (design 4.8; mid-points and rules Pub, edges Fit).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CorridorPoint {
    pub nacelle_degrees: f64,
    /// Lowest KCAS, or none (rearward flight allowed).
    pub minimum_kcas: Option<f64>,
    pub maximum_kcas: f64,
}

/// The tiltrotor's nacelles, rotor speed schedule, wing and corridor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TiltrotorParameters {
    /// Nacelle travel from the downstops (0, airplane) to the stop, degrees
    /// (Pub 97.5).
    pub nacelle_range_degrees: f64,
    /// Nacelle slew rate, degrees per second (Pub).
    pub nacelle_rate_degrees_per_second: f64,
    /// The helicopter preset, degrees (Pub: hover at 86 to 88).
    pub helicopter_nacelle_degrees: f64,
    /// Rotor speed on the downstops as a share of 100 percent (Pub: 333 of
    /// 397 rpm).
    pub airplane_rotor_speed: f64,
    /// How fast the governor's reference moves between the two, per second
    /// (Fit).
    pub rotor_speed_rate: f64,
    /// Wing 1 G stall speed at sea level, kt (Pub; the PT envelope is the
    /// AH-64's).
    pub wing_stall_kt: f64,
    /// Wingborne maximum roll rate, degrees per second (Fit; the PT's is the
    /// AH-64's).
    pub wingborne_roll_degrees_per_second: f64,
    /// No aft nacelle motion above this KCAS (Pub).
    pub aft_lock_kcas: f64,
    /// Gear speed warning above this KCAS (Pub limit, warning Fit).
    pub gear_limit_kcas: f64,
    /// Rotor thrust lost to the wing in a hover (Fit).
    pub wing_download: f64,
    /// The corridor, nacelle angles descending from the stop.
    pub corridor: &'static [CorridorPoint],
}

/// Handling numbers a player can measure, which the stability levels and
/// the fits aim at (design 8.2, "Handling").
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HandlingTargets {
    /// Hover full-stick steady rates at Damper, roll, pitch and yaw, degrees
    /// per second.
    pub hover_rates_degrees_per_second: [f64; 3],
}

impl PoweredLift {
    /// The powered-lift parameters of `a`, or none for a conventional
    /// aircraft. `a` is the variety PT the model is built from and `c` the
    /// configuration built from it.
    pub fn for_aircraft(a: &Aircraft, c: &Configuration) -> Option<Self> {
        use AircraftId::*;
        let (
            kind,
            efficiency,
            additional_lift_lbf,
            response_seconds,
            pitch_degrees,
            bank_degrees,
            yaw_degrees_per_second,
            horizontal_damping,
        ) = match a.id {
            Av8 => (LiftKind::VectorJet, 1., 0., 0.35, 20., 25., 35., 0.06),
            Yak141 => (LiftKind::VectorJet, 1., 18000., 0.45, 20., 25., 30., 0.07),
            V22 => (LiftKind::Tiltrotor, 1.10, 0., 0.65, 20., 25., 30., 0.10),
            Ah64 => (LiftKind::Helicopter, 0.98, 0., 0.45, 20., 30., 45., 0.14),
            Mi24 => (LiftKind::Helicopter, 0.84, 0., 0.60, 18., 25., 35., 0.12),
            Ch47 => (LiftKind::Helicopter, 0.245, 0., 0.85, 15., 20., 25., 0.10),
            _ => return None,
        };
        Some(Self {
            kind,
            efficiency,
            additional_lift_lbf,
            response_seconds,
            pitch_degrees,
            bank_degrees,
            yaw_degrees_per_second,
            horizontal_damping,
            body: body(a.id)?,
            rotor: rotor(a.id, c),
            jet: jet(a),
            tiltrotor: tiltrotor(a.id),
            targets: targets(a.id)?,
        })
    }
}

fn body(id: AircraftId) -> Option<BodyParameters> {
    use AircraftId::*;
    let radii_of_gyration_ft = match id {
        Av8 => [4.5, 8.5, 9.5],
        Yak141 => [4.5, 11., 12.],
        V22 => [9., 10., 12.5],
        Ah64 => [3.2, 8.5, 8.3],
        Mi24 => [3.5, 10., 10.],
        Ch47 => [5.5, 13.8, 13.5],
        _ => return None,
    };
    Some(BodyParameters {
        radii_of_gyration_ft,
    })
}

/// A PT number, when the record has the field.
fn field(a: &Aircraft, key: &str) -> Option<f64> {
    a.fields
        .get(key)
        .and_then(|token| token.number().ok())
        .map(f64::from)
}

fn rotor(id: AircraftId, c: &Configuration) -> Option<RotorParameters> {
    use AircraftId::*;
    let pt_thrust = c.propulsion.military_thrust_lbf;
    Some(match id {
        // Rotor 48 ft 0 in (Pub W-AH64); 727 ft/s hover tip speed and 0.0928
        // thrust-weighted solidity (Pub TM4201), so 289 rpm (Derived); the
        // PT thrust as maximum rotor thrust (it reproduces the real 2 x
        // 1,690 shp within 10 percent, design 8.2); Vne 197 kt (Pub).
        Ah64 => RotorParameters {
            layout: RotorLayout::Single {
                rotation: RotorRotation::CounterClockwise,
                tail_rotor_arm_ft: 30.,
            },
            radius_ft: 24.,
            rotor_speed_rpm: 289.,
            solidity: 0.0928,
            max_thrust_lbf: pt_thrust,
            energy_seconds: 1.8,
            never_exceed_kt: 197.,
            // Hovers at gross weight near 75 percent lever (design 5.5).
            collective_degrees: [1., 15.],
            cyclic_degrees: [12., 22.],
            blowback: 0.05,
            lock_number: 2.4,
            hub_height_ft: 6.5,
            hub_stiffness: 1.5,
            profile_drag: 0.008,
            ground_effect_constant: 2.7,
            vortex_ring_rise: 1.3,
            tail_rotor: Some(TailRotorParameters {
                radius_ft: 4.6,
                tip_speed_fps: 700.,
                solidity: 0.2,
                pedal_authority: 1.25,
            }),
            airframe: RotorcraftAirframe {
                flat_plate_ft2: [65., 300., 450.],
                tail_pitch_ft3: 5_000.,
                tail_trim_degrees: -2.,
                fin_yaw_ft3: 2_000.,
                fin_torque_share: 0.2,
                half_track_ft: 3.3,
                stub_wing: None,
            },
        },
        // 17.30 m rotor and 240 rpm (Pub AW-Mi24); solidity, tail arm and
        // Vne Fit (not verifiable).
        Mi24 => RotorParameters {
            layout: RotorLayout::Single {
                rotation: RotorRotation::Clockwise,
                tail_rotor_arm_ft: 34.,
            },
            radius_ft: 28.4,
            rotor_speed_rpm: 240.,
            solidity: 0.078,
            max_thrust_lbf: pt_thrust,
            energy_seconds: 2.,
            never_exceed_kt: 190.,
            collective_degrees: [1., 16.],
            cyclic_degrees: [9., 20.],
            blowback: 0.05,
            lock_number: 3.5,
            hub_height_ft: 7.5,
            hub_stiffness: 1.5,
            profile_drag: 0.008,
            ground_effect_constant: 2.7,
            vortex_ring_rise: 1.3,
            tail_rotor: Some(TailRotorParameters {
                radius_ft: 6.4,
                tip_speed_fps: 700.,
                solidity: 0.15,
                pedal_authority: 2.4,
            }),
            // Stub wings carry about a quarter of the weight at 170 kt
            // (Pub W-Mi24: "up to a quarter of total lift").
            airframe: RotorcraftAirframe {
                flat_plate_ft2: [52., 300., 500.],
                tail_pitch_ft3: 3_000.,
                tail_trim_degrees: -2.,
                fin_yaw_ft3: 2_500.,
                fin_torque_share: 0.2,
                half_track_ft: 4.9,
                stub_wing: Some(StubWing {
                    lift_area_ft2: 320.,
                    incidence_degrees: 14.,
                    stall_degrees: 15.,
                }),
            },
        },
        // Two 60 ft counter-rotating rotors (Pub W-CH47), 38.9 ft apart
        // (Derived from the 98 ft 10.7 in rotors-turning length); rotor
        // speed, solidity and Vne Fit. The PT thrust (4.7 times the maximum
        // weight) is implausible, so 1.25 x the PT maximum takeoff weight.
        Ch47 => RotorParameters {
            layout: RotorLayout::Tandem {
                hub_spacing_ft: 38.9,
            },
            radius_ft: 30.,
            rotor_speed_rpm: 225.,
            solidity: 0.062,
            max_thrust_lbf: 1.25 * c.mass.max_takeoff_lbs,
            energy_seconds: 2.5,
            never_exceed_kt: 180.,
            // Starting values for slice P3; the CH-47 still flies the old
            // powered law.
            collective_degrees: [1., 14.],
            cyclic_degrees: [8., 8.],
            blowback: 0.1,
            lock_number: 4.,
            hub_height_ft: 8.,
            hub_stiffness: 1.,
            profile_drag: 0.008,
            ground_effect_constant: 2.7,
            vortex_ring_rise: 1.3,
            tail_rotor: None,
            airframe: RotorcraftAirframe {
                flat_plate_ft2: [60., 250., 600.],
                tail_pitch_ft3: 0.,
                tail_trim_degrees: 0.,
                fin_yaw_ft3: 1_500.,
                fin_torque_share: 0.,
                half_track_ft: 5.,
                stub_wing: None,
            },
        },
        // Two 38 ft 1 in rotors 46.5 ft apart (Pub, Derived), 397 rpm at
        // 100 percent (Pub), 0.105 solidity (Pub, scale rotor); the PT thrust
        // as both rotors' static thrust; 280 KCAS never-exceed in airplane
        // mode (Pub).
        V22 => RotorParameters {
            layout: RotorLayout::SideBySide {
                hub_spacing_ft: 46.5,
            },
            radius_ft: 19.04,
            rotor_speed_rpm: 397.,
            solidity: 0.105,
            max_thrust_lbf: pt_thrust,
            energy_seconds: 1.5,
            never_exceed_kt: 280.,
            // Starting values for slice P5; the V-22 still flies the old
            // powered law. Proprotor blade pitch reaches about 50 degrees
            // to absorb full power at cruise (design 4.8).
            collective_degrees: [0., 50.],
            cyclic_degrees: [8., 8.],
            blowback: 0.1,
            lock_number: 4.,
            hub_height_ft: 0.,
            hub_stiffness: 1.,
            profile_drag: 0.008,
            ground_effect_constant: 2.7,
            vortex_ring_rise: 1.3,
            tail_rotor: None,
            airframe: RotorcraftAirframe {
                flat_plate_ft2: [30., 200., 500.],
                tail_pitch_ft3: 0.,
                tail_trim_degrees: 0.,
                fin_yaw_ft3: 0.,
                fin_torque_share: 0.,
                half_track_ft: 7.,
                stub_wing: None,
            },
        },
        _ => return None,
    })
}

fn jet(a: &Aircraft) -> Option<JetParameters> {
    use AircraftId::*;
    // PT `vtLimitDown` is -100 and `vtSpeed` 100 on both jets; a record
    // without them (the synthetic test aircraft) takes the same.
    let nozzle_range_degrees = field(a, "vtLimitDown")
        .filter(|v| *v < 0.)
        .map_or(100., f64::abs);
    let nozzle_rate_degrees_per_second = field(a, "vtSpeed").filter(|v| *v > 0.).unwrap_or(100.);
    let (vertical_efficiency, lift_engines) = match a.id {
        Av8 => (0.75, None),
        Yak141 => (
            0.95,
            Some(LiftEngines {
                thrust_lbf: 18000.,
                spool_seconds: 2.,
                start_nozzle_degrees: 30.,
                stop_nozzle_degrees: 20.,
                stop_speed_kt: 200.,
            }),
        ),
        _ => return None,
    };
    Some(JetParameters {
        nozzle_range_degrees,
        nozzle_rate_degrees_per_second,
        vertical_efficiency,
        spool_up_seconds: 0.8,
        spool_down_seconds: 0.6,
        lift_engines,
    })
}

/// The V-22 conversion corridor (design 4.8).
const V22_CORRIDOR: [CorridorPoint; 7] = [
    CorridorPoint {
        nacelle_degrees: 85.,
        minimum_kcas: None,
        maximum_kcas: 100.,
    },
    CorridorPoint {
        nacelle_degrees: 80.,
        minimum_kcas: Some(30.),
        maximum_kcas: 130.,
    },
    CorridorPoint {
        nacelle_degrees: 60.,
        minimum_kcas: Some(60.),
        maximum_kcas: 160.,
    },
    CorridorPoint {
        nacelle_degrees: 45.,
        minimum_kcas: Some(75.),
        maximum_kcas: 180.,
    },
    CorridorPoint {
        nacelle_degrees: 30.,
        minimum_kcas: Some(90.),
        maximum_kcas: 200.,
    },
    CorridorPoint {
        nacelle_degrees: 15.,
        minimum_kcas: Some(100.),
        maximum_kcas: 200.,
    },
    CorridorPoint {
        nacelle_degrees: 0.,
        minimum_kcas: Some(110.),
        maximum_kcas: 280.,
    },
];

fn tiltrotor(id: AircraftId) -> Option<TiltrotorParameters> {
    (id == AircraftId::V22).then_some(TiltrotorParameters {
        nacelle_range_degrees: 97.5,
        nacelle_rate_degrees_per_second: 8.,
        helicopter_nacelle_degrees: 87.,
        airplane_rotor_speed: 0.84,
        rotor_speed_rate: 0.05,
        wing_stall_kt: 110.,
        wingborne_roll_degrees_per_second: 45.,
        aft_lock_kcas: 200.,
        gear_limit_kcas: 140.,
        wing_download: 0.1,
        corridor: &V22_CORRIDOR,
    })
}

fn targets(id: AircraftId) -> Option<HandlingTargets> {
    use AircraftId::*;
    let hover_rates_degrees_per_second = match id {
        // PT `puffRot` maxima.
        Av8 | Yak141 => [50., 20., 20.],
        V22 => [45., 30., 30.],
        // PT roll and yaw, pitch Fit.
        Ah64 => [90., 45., 90.],
        Mi24 => [90., 40., 80.],
        // Fit; the PT says 90 on every axis.
        Ch47 => [45., 25., 45.],
        _ => return None,
    };
    Some(HandlingTargets {
        hover_rates_degrees_per_second,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::AircraftModel;

    fn lift(id: AircraftId) -> Option<PoweredLift> {
        AircraftModel::for_aircraft(&super::super::tests::synthetic(id))
            .unwrap()
            .powered_lift()
    }

    #[test]
    fn the_six_carry_the_parameters_of_their_kind_and_no_other_aircraft_does() {
        use AircraftId::*;
        for id in AircraftId::SELECTABLE {
            if super::super::VarietyFlightModel::identity(id).is_none() {
                continue;
            }
            let Some(lift) = lift(id) else {
                assert!(!matches!(id, Av8 | Yak141 | V22 | Ah64 | Mi24 | Ch47));
                continue;
            };
            assert!(
                lift.body
                    .radii_of_gyration_ft
                    .iter()
                    .all(|k| k.is_finite() && *k > 0.)
            );
            assert!(
                lift.targets
                    .hover_rates_degrees_per_second
                    .iter()
                    .all(|r| *r > 0.)
            );
            assert_eq!(
                lift.jet.is_some(),
                lift.kind == LiftKind::VectorJet,
                "{id:?}"
            );
            assert_eq!(
                lift.rotor.is_some(),
                lift.kind != LiftKind::VectorJet,
                "{id:?}"
            );
            assert_eq!(lift.tiltrotor.is_some(), lift.kind == LiftKind::Tiltrotor);
            if let Some(rotor) = lift.rotor {
                for value in [
                    rotor.radius_ft,
                    rotor.rotor_speed_rpm,
                    rotor.solidity,
                    rotor.max_thrust_lbf,
                    rotor.energy_seconds,
                    rotor.never_exceed_kt,
                ] {
                    assert!(value.is_finite() && value > 0., "{id:?}");
                }
                let rotors = if matches!(id, Ah64 | Mi24) { 1 } else { 2 };
                assert_eq!(rotor.layout.rotors(), rotors, "{id:?}");
            }
        }
    }

    #[test]
    fn rotor_figures_reproduce_the_published_tip_speeds_and_the_pt_thrust() {
        let tip = |id| lift(id).unwrap().rotor.unwrap().tip_speed_fps();
        // AH-64 727 ft/s (TM4201), V-22 790 (Derived), Mi-24 713 (Derived).
        assert!((tip(AircraftId::Ah64) - 727.).abs() < 2.);
        assert!((tip(AircraftId::V22) - 790.).abs() < 2.);
        assert!((tip(AircraftId::Mi24) - 713.).abs() < 2.);
        // The synthetic PT thrust is 50,000 lbf and its maximum takeoff
        // weight 15,000 lb.
        let thrust = |id| lift(id).unwrap().rotor.unwrap().max_thrust_lbf;
        assert_eq!(thrust(AircraftId::Ah64), 50_000.);
        assert_eq!(thrust(AircraftId::Ch47), 1.25 * 15_000.);
        let ah64 = lift(AircraftId::Ah64).unwrap().rotor.unwrap();
        assert_eq!(
            ah64.layout,
            RotorLayout::Single {
                rotation: RotorRotation::CounterClockwise,
                tail_rotor_arm_ft: 30.
            }
        );
    }

    #[test]
    fn jets_take_the_pt_nozzle_travel_and_the_v22_its_corridor() {
        let mut aircraft = super::super::tests::synthetic(AircraftId::Av8);
        // Without the fields: the PT values both jets carry.
        let jet = lift(AircraftId::Av8).unwrap().jet.unwrap();
        assert_eq!(jet.nozzle_range_degrees, 100.);
        assert_eq!(jet.nozzle_rate_degrees_per_second, 100.);
        for (key, value) in [("vtLimitDown", "-95"), ("vtSpeed", "90")] {
            aircraft.fields.insert(
                key.into(),
                tore_formats::aircraft::Token {
                    kind: "word".into(),
                    value: value.into(),
                    scaled: false,
                },
            );
        }
        let jet = AircraftModel::for_aircraft(&aircraft)
            .unwrap()
            .powered_lift()
            .unwrap()
            .jet
            .unwrap();
        assert_eq!(jet.nozzle_range_degrees, 95.);
        assert_eq!(jet.nozzle_rate_degrees_per_second, 90.);
        assert!(
            lift(AircraftId::Av8)
                .unwrap()
                .jet
                .unwrap()
                .lift_engines
                .is_none()
        );
        assert!(
            lift(AircraftId::Yak141)
                .unwrap()
                .jet
                .unwrap()
                .lift_engines
                .is_some()
        );
        let tilt = lift(AircraftId::V22).unwrap().tiltrotor.unwrap();
        assert!(
            tilt.corridor
                .windows(2)
                .all(|w| w[0].nacelle_degrees > w[1].nacelle_degrees)
        );
        assert_eq!(tilt.corridor.last().unwrap().maximum_kcas, 280.);
        assert!(tilt.helicopter_nacelle_degrees < tilt.nacelle_range_degrees);
    }
}
