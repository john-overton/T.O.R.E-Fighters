//! The V-22 tiltrotor (VTOL overhaul design 4.8, slice P5): two
//! [`super::rotor`] proprotors on nacelles that turn from the helicopter
//! stop to the airplane downstops, an interconnected drive
//! ([`super::drive`]), the angle-of-attack wing of [`super::aero`] and a
//! fly-by-wire mixer, on the [`super::body`] rigid body. One continuous
//! physics from the hover to wingborne flight, with no mode switch.
//!
//! - **Nacelles** turn 0 (airplane, on the downstops) to 97.5 degrees (past
//!   vertical) at 8 degrees per second (Pub). The pilot's demand moves by the
//!   conversion keys and lever and by `0` (nacelles forward); the nacelles
//!   follow it through the corridor protection below.
//! - **Rotors**: each proprotor is a [`RotorModel`] whose shaft is the nacelle
//!   axis, at the end of a mast from the centre of gravity out at the wing
//!   tip, so the same equations make a helicopter rotor in the hover and a
//!   propeller in airplane mode. The hub's air velocity includes the body's
//!   rotation, so a roll meets the rotors' heave damping.
//! - **Mixer** (the real aircraft's fly-by-wire mixer: always present, not
//!   an aid). Rotor terms are scaled by `sin(nacelle)`: lateral stick is
//!   differential collective (roll), longitudinal stick longitudinal cyclic
//!   on both rotors (pitch), the pedals differential longitudinal cyclic
//!   (yaw). The wing's flaperons, elevator and rudders act through the
//!   angle-of-attack wing, whose authority grows with dynamic pressure, so
//!   the rotors phase out as the surfaces take over.
//! - **Thrust control lever**: the collective in every mode (more lever,
//!   more thrust). The flight computers add the blade pitch the airspeed
//!   along the shaft needs (`1.5 x lambda`, less with advance ratio), so the
//!   lever keeps meaning thrust as the rotors become propellers. As the
//!   nacelles come down (from 75 degrees, fully below 30) the lever becomes
//!   a power lever, as the real TCL is in airplane mode: the flight
//!   computers set the blade pitch for the thrust that absorbs the lever's
//!   share of the engines' power at the airspeed along the shaft, inverting
//!   the rotor's own thrust law, so full lever is full power at any speed
//!   and the rotor speed governor never runs out of power. Midway, the
//!   computers take blade pitch off when the rotor speed droops, so a
//!   conversion at full lever cannot stall the rotors.
//! - **Rotor speed**: 100 percent, scheduled to 84 percent on the downstops
//!   at 5 percent a second (design 4.8). The drive's governor holds it.
//! - **Wing download**: up to 10 percent of the rotors' thrust pushes down
//!   on the wing in a hover, fading with `sin(nacelle)` and with airspeed as
//!   the wake sweeps aft.
//! - **Wing**: the angle-of-attack wing of the jets with its own
//!   [`V22_WING`] shape, a 45 degree per second roll law and the published
//!   110 kt 1 G stall speed (the PT envelope is the AH-64's). The fuselage
//!   adds its flat-plate drag.
//! - **Conversion corridor** ([`corridor_limits`]): indicated airspeed limits
//!   against nacelle angle. Protection is always on, at every stability
//!   level and whatever the Easy flight physics cheat says (decision 9): it
//!   drives the nacelles forward at the full rate when the aircraft is too
//!   fast for their angle, refuses aft motion above 200 KCAS and beyond the
//!   upper edge, and stops forward motion at the lower edge; both are slowed
//!   near the edge. It never touches the airframe, only the nacelle demand,
//!   and the pilot's demand is kept ([`LiftState::corridor_hold`]).
//! - **Ground**: wheels hold pitch and yaw as on the helicopters; dynamic
//!   rollover as there. Nacelles below 60 degrees on the wheels below 10 kt
//!   of ground speed is a rotor strike (design 4.9): the published rolling
//!   takeoff with 45 degrees of nacelle is not.
//!
//! The Easy flight physics cheat (slice P8) reaches the rotors through
//! [`State::rotor_hazards`] and the stability law, as on the helicopters;
//! corridor protection and the rotor strike ignore it.
//!
//! Every constant marked fitted is an agent decision of 2026-10-08 (slice
//! P5); the per-aircraft values are the V-22 rows of
//! [`crate::models::variety::RotorParameters`] and
//! [`crate::models::variety::TiltrotorParameters`].

use super::{
    DT, State,
    aero::{self, AirData, LiftCapacity, RollLaw, WingForces, WingInputs, WingShape},
    airframe,
    body::{GRAVITY, Inertia, Moments, body_axes},
    drive::{self, DriveModel, DriveStep},
    fuselage::{self, AirframeInput, AirframeLoads},
    rotor::{self, DiskFrame, Hazards, RotorInput, RotorModel, RotorOutput},
    sas,
    state::{PilotAids, Rotor},
    trace,
};
use crate::{
    attitude::{Basis, Vector, cross, dot},
    models::{
        FlightModel,
        config::Configuration,
        variety::{CorridorPoint, PoweredLift, RotorLayout, RotorParameters, TiltrotorParameters},
    },
};

/// ft/s per knot.
const KT: f64 = 1.687_81;
/// The nacelles count as on the downstops below this angle, degrees.
const DOWNSTOP_DEGREES: f64 = 0.5;
/// Collective lever travel per second, as on the helicopters.
const COLLECTIVE_RATE: f64 = 2.;
/// The thrust control lever becomes a power lever as the nacelles come
/// down: fully below the first angle, not at all above the second,
/// degrees (fitted).
const POWER_LEVER_DEGREES: [f64; 2] = [30., 75.];
/// The power lever's thrust divides by the airspeed along the shaft, no
/// less than this, ft/s (fitted: only reached with the nacelles low at
/// low speed).
const POWER_LEVER_SPEED_FLOOR: f64 = 60.;
/// The pitch schedule counts the air through the disk from the angle of
/// attack fully from this forward airspeed, ft/s (fitted).
const SCHEDULE_AIRSPEED_FPS: f64 = 60.;
/// Rotor speed droop protection in the conversion, where the lever is part
/// collective and part power lever: blade pitch taken off per unit of rotor
/// speed below its reference, past a small allowance, rad, at its peak
/// halfway (fitted: the conversion at full lever keeps the rotor above 90
/// percent, and the loop is stable at 120 Hz).
const DROOP_PROTECTION_GAIN: f64 = 5.;
const DROOP_ALLOWANCE: f64 = 0.01;
/// The power lever's power follows the rotor speed error like the drive's
/// governor does, rated power per unit of rotor speed, so the engines
/// follow the lever and the rotor keeps its speed (fitted: the drive's own
/// governor gain).
const POWER_LEVER_GOVERNOR: f64 = 5.;
/// Retreating blade stall only in edgewise flight: with the nacelles below
/// this angle the proprotors fly axially (fitted).
const EDGEWISE_DEGREES: f64 = 60.;
/// The wing download fades out by this many hover induced velocities of
/// airspeed (fitted).
const DOWNLOAD_FADE: f64 = 2.;
/// Corridor protection slows nacelle motion within this many degrees of an
/// edge, to no less than this share of the full rate, and when it drives
/// the nacelles forward off the upper edge it aims this far inside it, in
/// KCAS (fitted).
const CORRIDOR_SLOW_DEGREES: f64 = 5.;
const CORRIDOR_SLOW_SHARE: f64 = 0.25;
const CORRIDOR_MARGIN_KCAS: f64 = 5.;
/// Rotor strike: nacelles below this angle on the wheels below this ground
/// speed (design 4.9).
const ROTOR_STRIKE_DEGREES: f64 = 60.;
const ROTOR_STRIKE_KT: f64 = 10.;
/// Dynamic rollover, as on the helicopters (design 4.9).
const ROLLOVER_BANK_DEGREES: f64 = 15.;
const ROLLOVER_THRUST_SHARE: f64 = 0.1;
const WHEEL_YAW_HOLD: f64 = 0.05;
/// The stall warning only with the nacelles below this angle, as in the
/// real aircraft (design 4.8).
const STALL_WARNING_DEGREES: f64 = 35.;
/// Trim: Newton iterations, inner equilibrium passes and the finite
/// difference step.
const TRIM_ITERATIONS: usize = 16;
const TRIM_EQUILIBRIUM_PASSES: usize = 80;
const TRIM_STEP: f64 = 1e-6;

/// The V-22's wing: a thick, heavily loaded wing with a modest stall angle
/// and gentler moments than the jets' (fitted to fly like the published
/// aircraft: 110 kt stall, 45 degrees per second of roll).
pub const V22_WING: WingShape = WingShape {
    zero_lift_alpha: -2. * std::f64::consts::PI / 180.,
    stall_alpha: 14. * std::f64::consts::PI / 180.,
    trim_alpha: 3. * std::f64::consts::PI / 180.,
    short_period_rad_per_second: 1.5,
    short_period_damping: 0.7,
    alpha_rate_limit: 15. * std::f64::consts::PI / 180.,
    roll_seconds: 0.6,
    roll_seconds_minimum: 0.25,
    dihedral: 0.2,
    weathercock_rad_per_second: 1.2,
    weathercock_damping: 0.6,
};

/// How much of a helicopter the V-22 is at a nacelle angle (degrees): 1
/// from 75 degrees up, 0 from 30 down, a smooth step between. The thrust
/// control lever is the collective in that share and a power lever in the
/// rest; it is also the state's hover fraction
/// ([`super::LiftState::hover_fraction`]), which weights the Easy flight
/// physics attitude retention (slice P8).
pub fn helicopter_share(nacelle_degrees: f64) -> f64 {
    let [low, high] = POWER_LEVER_DEGREES;
    rotor::smoothstep((nacelle_degrees - low) / (high - low))
}

/// Indicated airspeed, kt, of a true airspeed in ft/s at a density.
pub fn kcas(true_airspeed_fps: f64, density: f64) -> f64 {
    true_airspeed_fps * (density / rotor::sea_level_density()).sqrt() / KT
}

/// The corridor's [minimum, maximum] KCAS at a nacelle angle, straight
/// lines between its points. Above its highest point there is no minimum
/// (rearward flight allowed, reported as 0) and its maximum holds.
pub fn corridor_limits(corridor: &[CorridorPoint], nacelle_degrees: f64) -> [f64; 2] {
    let point = |p: &CorridorPoint| [p.minimum_kcas.unwrap_or(0.), p.maximum_kcas];
    let Some(first) = corridor.first() else {
        return [0., f64::MAX];
    };
    if nacelle_degrees >= first.nacelle_degrees {
        return point(first);
    }
    for pair in corridor.windows(2) {
        let (high, low) = (&pair[0], &pair[1]);
        if nacelle_degrees >= low.nacelle_degrees {
            let t = (nacelle_degrees - low.nacelle_degrees)
                / (high.nacelle_degrees - low.nacelle_degrees);
            let [a, b] = [point(low), point(high)];
            return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t];
        }
    }
    point(corridor.last().unwrap())
}

/// The corridor's points as (nacelle degrees, [minimum, maximum] KCAS),
/// from `range` (the nacelles' stop) down to the lowest point.
fn corridor_points(corridor: &[CorridorPoint], range: f64) -> Vec<(f64, [f64; 2])> {
    let mut points = vec![(range, corridor_limits(corridor, range))];
    points.extend(
        corridor
            .iter()
            .filter(|p| p.nacelle_degrees < range)
            .map(|p| {
                (
                    p.nacelle_degrees,
                    corridor_limits(corridor, p.nacelle_degrees),
                )
            }),
    );
    points
}

/// The highest nacelle angle whose maximum is at least `kcas`: how far aft
/// the nacelles may sit. The lowest point when none is.
pub fn aft_limit_degrees(corridor: &[CorridorPoint], range: f64, kcas: f64) -> f64 {
    let points = corridor_points(corridor, range);
    if points[0].1[1] >= kcas {
        return points[0].0;
    }
    for pair in points.windows(2) {
        let ((high, [_, at_high]), (low, [_, at_low])) = (pair[0], pair[1]);
        if at_low >= kcas {
            // The maximum rises from `at_high` to `at_low` going forward.
            return high - (kcas - at_high) / (at_low - at_high).max(1e-9) * (high - low);
        }
    }
    points.last().unwrap().0
}

/// The lowest nacelle angle whose minimum is at most `kcas`: how far
/// forward the nacelles may go. The stop when none is.
pub fn forward_limit_degrees(corridor: &[CorridorPoint], range: f64, kcas: f64) -> f64 {
    let points = corridor_points(corridor, range);
    let last = points.len() - 1;
    if points[last].1[0] <= kcas {
        return points[last].0;
    }
    for pair in points.windows(2).rev() {
        let ((high, [at_high, _]), (low, [at_low, _])) = (pair[0], pair[1]);
        if at_high <= kcas {
            // The minimum falls from `at_low` to `at_high` going aft.
            return low + (at_low - kcas) / (at_low - at_high).max(1e-9) * (high - low);
        }
    }
    points[0].0
}

/// One tick of the nacelles under corridor protection.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NacelleStep {
    /// The nacelle angle after the tick, degrees.
    pub degrees: f64,
    /// Where protection sent the nacelles instead of the pilot's demand,
    /// degrees; none while the pilot's demand stands.
    pub hold_degrees: Option<f64>,
}

/// Moves the nacelles from `actual` toward the pilot's `demand` (degrees)
/// for `dt` at `rate` (degrees per second) within the corridor at `kcas`
/// (module documentation).
pub fn protect_nacelles(
    t: &TiltrotorParameters,
    actual: f64,
    demand: f64,
    kcas: f64,
    dt: f64,
) -> NacelleStep {
    let range = t.nacelle_range_degrees;
    let rate = t.nacelle_rate_degrees_per_second;
    let aft_limit = aft_limit_degrees(t.corridor, range, kcas);
    let forward_limit = forward_limit_degrees(t.corridor, range, kcas);
    let mut target = demand.clamp(0., range);
    let mut speed = rate;
    if actual > aft_limit + 1e-9 {
        // Too fast for the nacelles' angle: forward at the full rate, a
        // little inside the edge, whatever the pilot asks.
        target = target.min(aft_limit_degrees(
            t.corridor,
            range,
            kcas + CORRIDOR_MARGIN_KCAS,
        ));
    } else if target > actual {
        // Aft: never above the aft lock, never past the upper edge, slower
        // near it.
        let limit = if kcas > t.aft_lock_kcas {
            actual
        } else {
            aft_limit
        };
        target = target.min(limit);
        speed *= ((limit - actual) / CORRIDOR_SLOW_DEGREES).clamp(CORRIDOR_SLOW_SHARE, 1.);
    } else if target < actual {
        // Forward: never past the lower edge, slower near it. Below the
        // edge already (the aircraft slowed), the nacelles stay put: they
        // are not raised for the pilot.
        let limit = forward_limit.min(actual);
        target = target.max(limit);
        speed *= ((actual - limit) / CORRIDOR_SLOW_DEGREES).clamp(CORRIDOR_SLOW_SHARE, 1.);
    }
    let step = speed * dt;
    let degrees = (actual + (target - actual).clamp(-step, step)).clamp(0., range);
    NacelleStep {
        degrees,
        hold_degrees: (target != demand.clamp(0., range) || speed < rate).then_some(target),
    }
}

/// The V-22's model, derived from its parameters and its configuration.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tiltrotor {
    pub lift: PoweredLift,
    pub tilt: TiltrotorParameters,
    pub rotor_parameters: RotorParameters,
    /// The left and right proprotors: counter-rotating, the right one
    /// counter-clockwise seen from above with the nacelles up (which way
    /// round is fitted; the torques cancel either way).
    pub rotors: [RotorModel; 2],
    pub drive: DriveModel,
    /// Each hub's distance out from the centre line, ft.
    pub half_span_ft: f64,
    /// Empty plus full internal fuel, lb.
    pub reference_weight: f64,
    /// Maximum static thrust of both rotors at sea level, lbf.
    pub max_thrust: f64,
    /// The sideslip a full rudder holds, rad, and the side force per unit
    /// sideslip velocity, 1/s: the conventional tuning's.
    pub rudder_slip: f64,
    pub side_force: f64,
}

/// One instant of the tiltrotor, enough to find its loads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Instant {
    pub basis: Basis,
    /// Body rates [roll, pitch, yaw], rad/s.
    pub rates: [f64; 3],
    /// Air velocity, world axes, ft/s.
    pub air_velocity: Vector,
    pub altitude_ft: f64,
    pub density: f64,
    pub rotor_speed: f64,
    pub rotor_speed_reference: f64,
    pub rotors: [Rotor; 2],
    /// Nacelle angle, rad: 0 on the downstops, π/2 vertical.
    pub nacelle: f64,
    /// Thrust control lever, 0..1.
    pub lever: f64,
    /// Engine power available now, ft·lbf/s: the power lever's full scale.
    pub available_power: f64,
    /// The rotor mixer's stick and pedal [pitch (aft positive), roll
    /// (right), pedal (nose right)], after the stability law.
    pub controls: [f64; 3],
    /// The wing surfaces' pilot command, same order, and the rudder.
    pub pilot: [f64; 3],
    pub rudder: f64,
    pub flaps: f64,
    pub weight: f64,
    /// Each hub's height above the surface, ft; none out of ground effect.
    pub hub_heights_agl_ft: [Option<f64>; 2],
    pub seconds: f64,
    pub hazards: Hazards,
    /// Fuselage drag growth (stores, damage) and wing lift loss.
    pub drag_factor: f64,
    pub lift_scale: f64,
    /// The wing's G limits [negative, positive].
    pub g_limits: [f64; 2],
    pub rudder_slip: f64,
    pub side_force: f64,
}

/// Everything acting on the tiltrotor at one instant, except gravity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loads {
    /// World axes, lbf.
    pub force: Vector,
    pub moments: Moments,
    /// Left and right.
    pub rotors: [RotorOutput; 2],
    pub wing: WingForces,
    pub airframe: AirframeLoads,
    /// The wing download, lbf.
    pub download_lbf: f64,
    /// Both rotors' power, ft·lbf/s.
    pub power: f64,
    /// Both rotors' thrust, lbf.
    pub thrust_lbf: f64,
}

/// Each rotor's controls this instant: [left, right] collective lever
/// (blade pitch through the rotor's lever scale, unclamped) and
/// longitudinal cyclic.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Mixed {
    levers: [f64; 2],
    cyclic: [f64; 2],
}

impl Tiltrotor {
    /// The model of a tiltrotor, or none for any other aircraft.
    pub fn new(lift: &PoweredLift, c: &Configuration) -> Option<Self> {
        let tilt = lift.tiltrotor?;
        let p = lift.rotor?;
        let RotorLayout::SideBySide { hub_spacing_ft } = p.layout else {
            return None;
        };
        let reference_weight = c.mass.empty_lbs + c.mass.internal_fuel_lbs;
        let rotors = [-1., 1.].map(|turn| RotorModel::new(&p, turn, reference_weight / 2.));
        let rho0 = rotor::sea_level_density();
        let hover =
            2. * rotors[0].hover_power(reference_weight / 2. / (1. - tilt.wing_download), rho0);
        let rated = p.rated_power_hp.map_or_else(
            || 2. * rotors[0].hover_power(p.max_thrust_lbf / 2., rho0),
            |hp| hp * 550.,
        );
        Some(Self {
            lift: *lift,
            tilt,
            rotor_parameters: p,
            rotors,
            drive: DriveModel::new(rated, hover, p.energy_seconds),
            half_span_ft: hub_spacing_ft / 2.,
            reference_weight,
            max_thrust: p.max_thrust_lbf,
            rudder_slip: c.tuning.rudder_rate / c.tuning.alignment_rate,
            side_force: c.tuning.sideslip_force,
        })
    }

    /// The share of the thrust control lever that is a power lever at a
    /// nacelle angle (rad), 0..1: one less [`helicopter_share`].
    pub fn power_lever_share(&self, nacelle: f64) -> f64 {
        1. - helicopter_share(nacelle.to_degrees())
    }

    /// The governor's reference rotor speed for a nacelle angle (degrees):
    /// the airplane-mode share on the downstops, 100 percent otherwise.
    pub fn rotor_speed_target(&self, nacelle_degrees: f64) -> f64 {
        if nacelle_degrees < DOWNSTOP_DEGREES {
            self.tilt.airplane_rotor_speed
        } else {
            1.
        }
    }

    /// The wing's lift at its stall angle now: 1 G at the published stall
    /// speed (indicated, at the reference weight, weight-scaled, a quarter
    /// lower with full flaps), with airspeed squared.
    pub fn wing_capacity(&self, weight: f64, flaps: f64, air: &AirData) -> LiftCapacity {
        let rho0 = rotor::sea_level_density();
        let clean = self.tilt.wing_stall_kt
            * KT
            * (rho0 / air.density.max(1e-9)).sqrt()
            * (weight / self.reference_weight).max(0.).sqrt();
        let flapped = (clean * (1. - 0.25 * flaps.clamp(0., 1.))).max(1.);
        LiftCapacity {
            lift_lbf: weight * (air.speed / flapped).powi(2),
            reference_qbar: 0.5 * air.density * clean * clean,
            clean_stall_fps: clean.max(1.),
        }
    }

    /// The disk frame of the nacelles at `nacelle` (rad): the shaft, the way
    /// a forward cyclic leans it (forward in the hover, down in airplane
    /// mode) and right.
    pub fn frame(basis: &Basis, nacelle: f64) -> DiskFrame {
        let (s, c) = nacelle.sin_cos();
        DiskFrame {
            shaft: std::array::from_fn(|k| c * basis.forward[k] + s * basis.up[k]),
            forward: std::array::from_fn(|k| s * basis.forward[k] - c * basis.up[k]),
            right: basis.right,
        }
    }

    /// Each hub's position from the centre of gravity, world axes, ft:
    /// [left, right].
    pub fn hubs(&self, basis: &Basis, nacelle: f64) -> [Vector; 2] {
        let shaft = Self::frame(basis, nacelle).shaft;
        let mast = self.rotor_parameters.hub_height_ft;
        [-1., 1.].map(|side| {
            std::array::from_fn(|k| side * self.half_span_ft * basis.right[k] + mast * shaft[k])
        })
    }

    /// The mixer: each rotor's collective lever and longitudinal cyclic.
    fn mix(
        &self,
        i: &Instant,
        hub_air: &[Vector; 2],
        rotors: &[Rotor; 2],
        frame: &DiskFrame,
    ) -> Mixed {
        let rotor = &self.rotors[0];
        let (s, c) = i.nacelle.sin_cos();
        let s = s.max(0.);
        let nr = i.rotor_speed.max(0.05);
        let tip = nr * rotor.tip_speed_fps;
        let [pitch, roll, pedal] = i.controls.map(|v| v.clamp(-1., 1.));
        let w = self.power_lever_share(i.nacelle);
        // The scheduled pitch: the airspeed along the shaft that forward
        // flight brings, cancelled in the thrust law. The air meeting the
        // wing from below counts only once the aircraft is moving forward,
        // so a climb or descent in the hover keeps its heave damping.
        let forward = dot(i.air_velocity, i.basis.forward).max(0.);
        let upward =
            dot(i.air_velocity, i.basis.up) * rotor::smoothstep(forward / SCHEDULE_AIRSPEED_FPS);
        let lambda = (forward * c.max(0.) + upward * s) / tip;
        let lever_pitch = rotor.blade_pitch(i.lever);
        let droop = DROOP_PROTECTION_GAIN
            * (i.rotor_speed_reference - DROOP_ALLOWANCE - i.rotor_speed).max(0.);
        let differential = self.tilt.differential_collective_degrees.to_radians() * roll * s;
        let thrust_scale =
            i.density * rotor.area_ft2 * tip * tip * rotor.solidity * rotor::LIFT_SLOPE / 2.;
        // Right stick: more pitch on the left rotor, less on the right.
        let levers = std::array::from_fn(|k| {
            let side = if k == 0 { 1. } else { -1. };
            let v = hub_air[k];
            // The disk as it leans now (its flapping: cyclic and blowback),
            // as the rotor sees it.
            let [long, lat] = rotors[k].tilt;
            let normal = crate::attitude::unit(std::array::from_fn(|n| {
                frame.shaft[n] + long.tan() * frame.forward[n] + lat.tan() * frame.right[n]
            }));
            let axial = dot(v, normal);
            let in_plane: Vector = std::array::from_fn(|n| v[n] - axial * normal[n]);
            let mu = dot(in_plane, in_plane).sqrt() / tip;
            let advance = 1. / 3. + 0.5 * mu * mu;
            // The schedule also takes out the inflow the disk's lean
            // changes in forward flight, not the climb's (heave damping).
            let leaned = (axial - dot(v, frame.shaft)) / tip;
            let helicopter = 0.5 * (lambda + leaned) / advance + lever_pitch;
            // The power lever: the thrust that absorbs the lever's share of
            // the power at this axial speed, and the pitch that makes it.
            let induced = rotors[k].induced_fps;
            let profile = rotor.profile_power(i.density, nr, mu);
            let power = i.lever.clamp(0., 1.) * i.available_power
                + POWER_LEVER_GOVERNOR
                    * (i.rotor_speed - i.rotor_speed_reference)
                    * self.drive.rated_power;
            let thrust = (0.5 * power - profile)
                / (axial.max(POWER_LEVER_SPEED_FLOOR)
                    + rotor::INDUCED_POWER_FACTOR * induced.max(0.));
            let airplane = (thrust / thrust_scale + 0.5 * (axial + induced) / tip) / advance;
            let pitch = (1. - w) * helicopter + w * airplane - 4. * w * (1. - w) * droop
                + side * differential;
            rotor.lever_for_pitch(pitch)
        });
        // Left rotor forward with right pedal: the left side pushes ahead.
        let yaw = self.tilt.differential_cyclic_degrees / rotor.cyclic_range()[0].to_degrees();
        let cyclic = [-1., 1.].map(|side| ((-pitch - side * yaw * pedal) * s).clamp(-1., 1.));
        Mixed { levers, cyclic }
    }

    /// Everything acting on the tiltrotor at `i`, except gravity.
    pub fn loads(&self, i: &Instant) -> Loads {
        let basis = i.basis;
        let frame = Self::frame(&basis, i.nacelle);
        let hubs = self.hubs(&basis, i.nacelle);
        let axes = body_axes(&basis);
        let omega: Vector = std::array::from_fn(|k| (0..3).map(|a| axes[a][k] * i.rates[a]).sum());
        let hub_air = hubs.map(|hub| {
            let spin = cross(omega, hub);
            std::array::from_fn(|k| i.air_velocity[k] + spin[k])
        });
        let mixed = self.mix(i, &hub_air, &i.rotors, &frame);
        // Retreating blade stall belongs to edgewise flight: with the
        // nacelles low the proprotors fly axially, so neither its effects
        // nor its vibration cue.
        let edgewise = i.nacelle.to_degrees() >= EDGEWISE_DEGREES;
        let blade_stall = i.hazards.blade_stall && edgewise;
        let outputs: [RotorOutput; 2] = std::array::from_fn(|k| {
            let out = self.rotors[k].evaluate(
                &i.rotors[k],
                &RotorInput {
                    frame,
                    air_velocity: hub_air[k],
                    density: i.density,
                    rotor_speed: i.rotor_speed,
                    collective: mixed.levers[k],
                    cyclic: [mixed.cyclic[k], 0.],
                    hub_height_agl_ft: i.hub_heights_agl_ft[k],
                    seconds: i.seconds,
                    hazards: Hazards {
                        blade_stall,
                        ..i.hazards
                    },
                },
            );
            RotorOutput {
                blade_stall: if edgewise { out.blade_stall } else { 0. },
                ..out
            }
        });
        let mut force = [0.; 3];
        let mut moment = [0.; 3];
        let nr = i.rotor_speed.max(0.05);
        for k in 0..2 {
            let out = &outputs[k];
            let thrust: Vector = out.normal.map(|n| n * out.thrust_lbf);
            let lever = cross(hubs[k], thrust);
            // Each rotor's torque reacts on the airframe about its shaft;
            // counter-rotating, they cancel unless the rotors differ. Not a
            // hazard the Easy flight physics cheat removes: nothing to
            // cancel, as there is no tail rotor.
            let torque = self.rotors[k].turn * out.power / (nr * self.rotors[k].omega);
            for n in 0..3 {
                force[n] += thrust[n];
                moment[n] += lever[n] + out.hub_moment[n] + torque * frame.shaft[n];
            }
        }
        let thrust_lbf = outputs[0].thrust_lbf + outputs[1].thrust_lbf;
        // Wing download: the wake on the wing, along the shaft, fading as
        // the nacelles come down and as the wake sweeps aft.
        let air = AirData::new(&basis, i.air_velocity, i.altitude_ft);
        let hover_induced = 0.5 * (outputs[0].hover_induced_fps + outputs[1].hover_induced_fps);
        let fade = 1. - rotor::smoothstep(air.speed / (DOWNLOAD_FADE * hover_induced).max(1.));
        let download_lbf =
            self.tilt.wing_download * thrust_lbf.max(0.) * i.nacelle.sin().max(0.) * fade;
        for (f, shaft) in force.iter_mut().zip(frame.shaft) {
            *f -= download_lbf * shaft;
        }
        let inertia = Inertia::from_weight(i.weight, self.lift.body.radii_of_gyration_ft);
        let wing = aero::wing(
            &air,
            &WingInputs {
                shape: V22_WING,
                roll: self.roll_law(),
                basis,
                rates: i.rates,
                inertia,
                weight_lbs: i.weight,
                capacity: self.wing_capacity(i.weight, i.flaps, &air),
                lift_scale: i.lift_scale,
                limits: i.g_limits,
                controls: i.pilot,
                rudder: i.rudder,
                other_force: force,
                rudder_slip: i.rudder_slip,
                side_force: i.side_force,
            },
        );
        let airframe = fuselage::airframe_loads(
            &self.rotor_parameters.airframe,
            &AirframeInput {
                basis,
                air_velocity: i.air_velocity,
                density: i.density,
                tail_arm_ft: 0.,
                drag_factor: i.drag_factor,
                turn: 0.,
                reference_torque: 0.,
                lift_factor: i.lift_scale,
            },
        );
        let mut applied = fuselage::body_moments(&basis, moment);
        // The rotors' rate damping: each disk lags the body about the axes
        // in its plane (pitch, and roll or yaw as the nacelles turn).
        let (s, c) = i.nacelle.sin_cos();
        let mast = self.rotor_parameters.hub_height_ft.abs();
        let lagging: f64 = outputs
            .iter()
            .map(|o| (o.tilt_stiffness + o.thrust_lbf.max(0.) * mast) * o.rate_lag_seconds)
            .sum();
        let mut damping = [lagging * s * s, lagging, lagging * c * c];
        for n in 0..3 {
            applied[n] += wing.moments.applied[n] + airframe.moments[n];
            damping[n] += wing.moments.damping[n] + airframe.damping[n];
            force[n] += wing.force[n] + airframe.force[n];
        }
        Loads {
            force,
            moments: Moments { applied, damping },
            rotors: outputs,
            wing,
            airframe,
            download_lbf,
            power: outputs[0].power + outputs[1].power,
            thrust_lbf,
        }
    }

    /// The wingborne roll law: the fitted 45 degrees per second.
    pub fn roll_law(&self) -> RollLaw {
        let acceleration = self.tilt.wingborne_roll_acceleration_degrees.to_radians();
        RollLaw {
            maximum: self.tilt.wingborne_roll_degrees_per_second.to_radians(),
            acceleration,
            deceleration: acceleration,
        }
    }

    /// What the stability law senses at `i`. The rotors' torques cancel, so
    /// there is no torque pedal.
    pub fn sensed(i: &Instant) -> sas::Sensed {
        let airspeed_fps = dot(i.air_velocity, i.air_velocity).sqrt();
        let sideslip_rad = if airspeed_fps > 1e-9 {
            (dot(i.air_velocity, i.basis.right) / airspeed_fps)
                .clamp(-1., 1.)
                .asin()
        } else {
            0.
        };
        sas::Sensed {
            airspeed_fps,
            sideslip_rad,
            torque_pedal: 0.,
        }
    }

    /// The trimmed state of level flight at `airspeed_fps` along `heading`
    /// with the nacelles at `nacelle` (rad), at `density` and `altitude_ft`,
    /// out of ground effect, gear up, with the stability law at `level` in
    /// the loop. None when the solution needs more than full travel of any
    /// control or more power than the engines have.
    #[allow(clippy::too_many_arguments)] // The trim's conditions, each its own.
    pub fn trim(
        &self,
        hazards: Hazards,
        weight: f64,
        heading: f64,
        airspeed_fps: f64,
        nacelle: f64,
        altitude_ft: f64,
        drag_factor: f64,
        level: tore_input::StabilityLevel,
        available_power: f64,
    ) -> Option<Trim> {
        let density = rotor::air_density(altitude_ft);
        let air_velocity = Basis::new(heading, 0., 0.)
            .forward
            .map(|f| f * airspeed_fps);
        let reference = self.rotor_speed_target(nacelle.to_degrees());
        let rotor = &self.rotors[0];
        let induced = (weight / 2. / (2. * density * rotor.area_ft2)).sqrt();
        // Unknowns: the lever, the stick and pedal, pitch and bank.
        let mut x = [0.7, 0., 0., 0., 0.05, 0.];
        let evaluate = |x: &[f64; 6]| -> (Instant, Loads) {
            let basis = Basis::new(heading, x[4], x[5]);
            let pilot = [x[1], x[2], x[3]];
            let mut instant = Instant {
                basis,
                rates: [0.; 3],
                air_velocity,
                altitude_ft,
                density,
                rotor_speed: reference,
                rotor_speed_reference: reference,
                rotors: [Rotor {
                    induced_fps: induced,
                    tilt: [0.; 2],
                }; 2],
                nacelle,
                lever: x[0],
                available_power,
                controls: pilot,
                pilot,
                rudder: x[3],
                flaps: 0.,
                weight,
                hub_heights_agl_ft: [None; 2],
                seconds: 0.,
                hazards,
                drag_factor,
                lift_scale: 1.,
                g_limits: [-1., 3.],
                rudder_slip: self.rudder_slip,
                side_force: self.side_force,
            };
            let mut aids = PilotAids {
                stability: level,
                trim: pilot,
                attitude_reference: [x[4], x[5], heading],
                ..Default::default()
            };
            let body = sas::Body {
                attitude: [x[4], x[5], heading],
                rates: [0.; 3],
            };
            let mut loads = self.loads(&instant);
            for _ in 0..TRIM_EQUILIBRIUM_PASSES {
                for k in 0..2 {
                    instant.rotors[k].induced_fps +=
                        0.5 * (loads.rotors[k].induced_target_fps - instant.rotors[k].induced_fps);
                    instant.rotors[k].tilt = loads.rotors[k].tilt_target;
                }
                instant.controls = sas::augment(
                    &self.lift,
                    level,
                    &mut aids,
                    pilot,
                    body,
                    Self::sensed(&instant),
                )
                .controls;
                loads = self.loads(&instant);
            }
            (instant, loads)
        };
        let residual = |loads: &Loads| -> [f64; 6] {
            let f = loads.force;
            let m = loads.moments.applied;
            let scale = weight * 10.;
            [
                f[0] / weight,
                f[1] / weight - 1.,
                f[2] / weight,
                m[0] / scale,
                m[1] / scale,
                m[2] / scale,
            ]
        };
        for _ in 0..TRIM_ITERATIONS {
            let (_, loads) = evaluate(&x);
            let r = residual(&loads);
            let mut jacobian = [[0.; 6]; 6];
            for j in 0..6 {
                let mut stepped = x;
                stepped[j] += TRIM_STEP;
                let (_, loads) = evaluate(&stepped);
                let rs = residual(&loads);
                for n in 0..6 {
                    jacobian[n][j] = (rs[n] - r[n]) / TRIM_STEP;
                }
            }
            let step = super::helicopter::solve(jacobian, r.map(|v| -v))?;
            for (value, delta) in x.iter_mut().zip(step) {
                *value += delta.clamp(-0.2, 0.2);
            }
        }
        let (instant, loads) = evaluate(&x);
        let worst = residual(&loads).iter().fold(0_f64, |a, b| a.max(b.abs()));
        let within = (0. ..=1.).contains(&x[0])
            && x[1..4].iter().all(|v| v.abs() <= 1.)
            && loads.power <= available_power;
        (worst < 1e-6 && within && x.iter().all(|v| v.is_finite())).then_some(Trim {
            lever: x[0],
            controls: [x[1], x[2], x[3]],
            pitch: x[4],
            bank: x[5],
            rotors: instant.rotors,
            rotor_speed: reference,
            engine_power: loads.power,
        })
    }
}

/// A trimmed state of flight.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trim {
    pub lever: f64,
    /// Stick and pedal [pitch, roll, pedal].
    pub controls: [f64; 3],
    pub pitch: f64,
    pub bank: f64,
    pub rotors: [Rotor; 2],
    pub rotor_speed: f64,
    pub engine_power: f64,
}

/// The corridor readout for the HUD (slice P7).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corridor {
    /// Indicated airspeed now, kt.
    pub kcas: f64,
    /// The nacelles' angle and the pilot's demand, degrees.
    pub nacelle_degrees: f64,
    pub demand_degrees: f64,
    /// The corridor at the nacelles' angle, [minimum, maximum] KCAS.
    pub limits_kcas: [f64; 2],
    /// The nacelle angles the corridor allows at this airspeed, [forward
    /// limit, aft limit], degrees: the bracket on the tape.
    pub allowed_degrees: [f64; 2],
    /// Protection is moving or holding the nacelles: the CONV cue.
    pub protecting: bool,
}

impl State {
    /// The V-22's nacelle angle, degrees from the downstops (0) through
    /// vertical (90) to the stop (97.5): what the HUD shows and the nacelle
    /// animation draws. Zero on other aircraft.
    pub fn nacelle_degrees(&self) -> f64 {
        self.model()
            .powered_lift()
            .and_then(|lift| lift.tiltrotor)
            .map_or(0., |t| {
                self.lift_controls.conversion_actual.clamp(0., 1.) * t.nacelle_range_degrees
            })
    }

    /// The tiltrotor's structural speed now, true airspeed in ft/s: the
    /// rotor table's never-exceed speed (calibrated, 280 KCAS in airplane
    /// mode) or, with the nacelles up, the conversion corridor's maximum at
    /// their angle, whichever is lower, at this altitude's density. None on
    /// other aircraft. The overspeed rule (docs/spec/overspeed.md) uses it.
    pub(crate) fn tiltrotor_structural_fps(&self) -> Option<f64> {
        let lift = self.model().powered_lift()?;
        let t = lift.tiltrotor?;
        let vne = lift.rotor?.structural_kt?;
        let corridor = corridor_limits(t.corridor, self.nacelle_degrees())[1];
        let density = rotor::air_density(self.position[1]);
        Some(vne.min(corridor) * KT * (rotor::sea_level_density() / density).sqrt())
    }

    /// The conversion corridor now, for the HUD; none on aircraft without
    /// nacelles.
    pub fn conversion_corridor(&self) -> Option<Corridor> {
        let t = self.model().powered_lift()?.tiltrotor?;
        let range = t.nacelle_range_degrees;
        let air = dot(self.velocity, self.velocity).sqrt();
        let kcas = kcas(air, rotor::air_density(self.position[1]));
        let nacelle_degrees = self.nacelle_degrees();
        Some(Corridor {
            kcas,
            nacelle_degrees,
            demand_degrees: self.lift_controls.conversion.clamp(0., 1.) * range,
            limits_kcas: corridor_limits(t.corridor, nacelle_degrees),
            allowed_degrees: [
                forward_limit_degrees(t.corridor, range, kcas),
                aft_limit_degrees(t.corridor, range, kcas),
            ],
            protecting: self.lift_controls.corridor_hold.is_some(),
        })
    }

    /// Fuselage drag growth from carried stores and regional damage, as on
    /// the helicopters.
    fn tiltrotor_drag_factor(&self, c: &Configuration, damage_percent: f64) -> f64 {
        (1. + self.carried_lbs() / c.mass.empty_lbs * c.aerodynamics.loaded_drag_percent / 100.)
            * (1. + damage_percent / 100.)
    }

    /// The available engine power of a tiltrotor now, ft·lbf/s, before the
    /// power lever.
    fn tiltrotor_available_power(&self, model: &Tiltrotor) -> f64 {
        if self.engine {
            model.drive.available_power(
                rotor::air_density(self.position[1]),
                self.systems.power_available(),
                self.throttle,
            )
        } else {
            0.
        }
    }

    /// Trims the tiltrotor in level flight at `airspeed_fps` (0 for a
    /// hover) with the nacelles at `nacelle_degrees`, at its current weight
    /// and altitude, out of ground effect: attitude, lever, stick and pedal
    /// trim, the rotors' inflow and tilt, rotor speed and engine power, with
    /// the body at rest. Sets the velocity along the heading for a forward
    /// trim. Returns false, and changes nothing, when no trim within the
    /// controls' travel and the engines' power exists.
    pub fn trim_tiltrotor(&mut self, airspeed_fps: f64, nacelle_degrees: f64) -> bool {
        let Some(lift) = self.model().powered_lift() else {
            return false;
        };
        let c = self.model().configuration();
        let Some(model) = Tiltrotor::new(&lift, c) else {
            return false;
        };
        let range = model.tilt.nacelle_range_degrees;
        let nacelle_degrees = nacelle_degrees.clamp(0., range);
        let weight = c.mass.empty_lbs + self.fuel + self.carried_lbs();
        let drag_factor = self.tiltrotor_drag_factor(c, 0.);
        let level = self
            .stability_in_effect()
            .unwrap_or(tore_input::StabilityLevel::Off);
        let throttle = self.throttle;
        self.throttle = 1.;
        let available = self.tiltrotor_available_power(&model);
        self.throttle = throttle;
        let Some(trim) = model.trim(
            self.rotor_hazards(),
            weight,
            self.yaw,
            airspeed_fps,
            nacelle_degrees.to_radians(),
            self.position[1],
            drag_factor,
            level,
            available,
        ) else {
            return false;
        };
        self.pitch = trim.pitch;
        self.bank = trim.bank;
        self.roll_rate = 0.;
        self.pitch_rate = 0.;
        let lift_controls = &mut self.lift_controls;
        lift_controls.body_rates = [0.; 3];
        lift_controls.collective = trim.lever;
        lift_controls.collective_actual = trim.lever;
        lift_controls.conversion = nacelle_degrees / range;
        lift_controls.conversion_actual = nacelle_degrees / range;
        lift_controls.corridor_hold = None;
        lift_controls.aids.trim = trim.controls;
        lift_controls.aids.attitude_reference = [trim.pitch, trim.bank, self.yaw];
        lift_controls.rotors = trim.rotors;
        lift_controls.drive.rotor_speed = trim.rotor_speed;
        lift_controls.drive.rotor_speed_reference = model.rotor_speed_target(nacelle_degrees);
        lift_controls.drive.engine_output[0] = trim.engine_power;
        lift_controls.thrust_lbf = weight;
        self.rudder = trim.controls[2];
        self.throttle = 1.;
        if airspeed_fps > 0. {
            let forward = Basis::new(self.yaw, 0., 0.).forward;
            self.velocity = forward.map(|f| f * airspeed_fps);
            self.speed = airspeed_fps;
        }
        true
    }

    /// One tick of the nacelles: corridor protection on the pilot's demand
    /// at this airspeed, and the rotor speed schedule. Returns the nacelle
    /// angle, degrees.
    fn step_nacelles(&mut self, model: &Tiltrotor, kcas: f64, hydraulics: bool) -> f64 {
        let t = &model.tilt;
        let range = t.nacelle_range_degrees;
        let lift = &mut self.lift_controls;
        let actual = lift.conversion_actual.clamp(0., 1.) * range;
        let demand = lift.conversion.clamp(0., 1.) * range;
        let step = protect_nacelles(t, actual, demand, kcas, if hydraulics { DT } else { 0. });
        lift.conversion_actual = step.degrees / range;
        lift.corridor_hold = step.hold_degrees.map(|d| d / range);
        let drive = &mut lift.drive;
        let target = model.rotor_speed_target(step.degrees);
        let rate = t.rotor_speed_rate * DT;
        drive.rotor_speed_reference += (target - drive.rotor_speed_reference).clamp(-rate, rate);
        step.degrees
    }

    /// One tick of the tiltrotor. The velocity is air-relative on entry
    /// (the adapter took the wind out); the shared tail of the powered step
    /// (wind back, position, contact) follows.
    #[allow(clippy::too_many_arguments)] // Shared tick data already calculated by the adapter.
    pub(super) fn step_tiltrotor(
        &mut self,
        lift: PoweredLift,
        model: Tiltrotor,
        c: &Configuration,
        stick: [f64; 3],
        initial_surface: crate::research::Surface,
        runway_wind_fraction: f64,
        mut t: trace::AdapterTrace,
        ground: impl Fn(f64, f64) -> crate::research::Surface,
        fuel_rate: f64,
    ) {
        let hydraulics = self.systems.fluids.hydraulic > 0.;
        if hydraulics {
            let lever = &mut self.lift_controls;
            let step = COLLECTIVE_RATE * DT;
            lever.collective_actual = (lever.collective_actual
                + (lever.collective - lever.collective_actual).clamp(-step, step))
            .clamp(0., 1.);
        }
        let hazards = self.rotor_hazards();
        let carried = self.carried_lbs();
        let weight = c.mass.empty_lbs + self.fuel + carried;
        let mass = weight / GRAVITY;
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let density = rotor::air_density(self.position[1]);
        let airspeed = dot(self.velocity, self.velocity).sqrt();
        let indicated = kcas(airspeed, density);
        let nacelle_degrees = self.step_nacelles(&model, indicated, hydraulics);
        let nacelle = nacelle_degrees.to_radians();
        let effects = t.regional.effects;
        let wheel_contact = self.research.as_ref().is_some_and(|r| r.on_ground);
        let limits = self.envelope_limits(c);
        let tuning = self.model().tuning();
        let hubs = model.hubs(&basis, nacelle);
        let mut instant = Instant {
            basis,
            rates: self.lift_controls.body_rates,
            air_velocity: self.velocity,
            altitude_ft: self.position[1],
            density,
            rotor_speed: self.lift_controls.drive.rotor_speed,
            rotor_speed_reference: self.lift_controls.drive.rotor_speed_reference,
            rotors: self.lift_controls.rotors,
            nacelle,
            lever: self.lift_controls.collective_actual,
            available_power: self.tiltrotor_available_power(&model),
            controls: stick,
            pilot: stick,
            rudder: self.rudder * effects.authority[2] + effects.yaw_bias,
            flaps: self.flaps,
            weight,
            hub_heights_agl_ft: hubs
                .map(|hub| Some(self.position[1] + hub[1] - initial_surface.height)),
            seconds: self.ticks as f64 * DT,
            hazards,
            drag_factor: self.tiltrotor_drag_factor(c, effects.drag_percent),
            lift_scale: effects.lift * if self.systems.has(25) { 0.5 } else { 1. },
            g_limits: limits.limits,
            rudder_slip: model.rudder_slip,
            side_force: model.side_force,
        };
        // The stability level's feedback on the rotor mixer; the wing's
        // surfaces fly the pilot's own command (their law is already a G
        // and roll-rate command), as on the jets.
        let augmented = self.augment(&lift, stick, Tiltrotor::sensed(&instant));
        instant.controls = augmented.controls;
        instant.pilot = augmented.pilot;
        let loads = model.loads(&instant);
        for (state, out) in self.lift_controls.rotors.iter_mut().zip(&loads.rotors) {
            rotor::relax(state, out, DT);
        }
        // The interconnected drive.
        let available = instant.available_power;
        let next = model.drive.advance(
            &mut self.lift_controls.drive,
            DriveStep {
                load_power: loads.power,
                available_power: available,
                // The Easy flight physics floor is applied below, never
                // above the airplane-mode reference.
                rotor_stall_hazard: true,
                wheel_contact,
                seconds: DT,
            },
        );
        let reference = self.lift_controls.drive.rotor_speed_reference;
        let next = if !hazards.rotor_stall && !wheel_contact {
            let floored = next.max(drive::EASY_ROTOR_FLOOR.min(reference));
            self.lift_controls.drive.rotor_speed = floored;
            floored
        } else {
            next
        };
        self.lift_controls.thrust_lbf = loads.thrust_lbf;
        let warnings = &mut self.lift_controls.warnings;
        drive::update_warnings(warnings, next);
        let count = |ticks: u32, on: bool| if on { ticks.saturating_add(1) } else { 0 };
        let stalled = loads.rotors.iter().any(|r| r.blade_stall > 0.05);
        warnings.blade_stall = count(warnings.blade_stall, stalled);
        warnings.gear_speed = count(
            warnings.gear_speed,
            self.gear > 0. && indicated > model.tilt.gear_limit_kcas,
        );
        // The rigid body, then the velocity, from the start-of-tick loads.
        let inertia = Inertia::from_weight(weight, lift.body.radii_of_gyration_ft);
        self.advance_body(inertia, loads.moments);
        // Devices, the G pull and sideslip, the conventional terms in the
        // PT's own fields, fading with the wing's dynamic pressure.
        let slip_fraction = if airspeed > 1e-9 {
            dot(self.velocity, basis.right) / airspeed
        } else {
            0.
        };
        let (devices, drag_trace) = self.airframe_drag(
            c,
            airframe::DragInputs {
                weight,
                top_speed: limits.top_speed,
                reference_thrust: 0.,
                lapse: 1.,
                loading: limits.loading,
                load_factor: loads.wing.lift_g,
                slip_drag: weight
                    * tuning.sideslip_drag
                    * slip_fraction.powi(2)
                    * loads.wing.authority,
                drag_percent: self.device_drag_percent(limits.top_speed),
                wheel_contact,
                damage_percent: 0.,
            },
        );
        let devices = devices * loads.wing.authority;
        let direction = crate::attitude::unit(self.velocity);
        let force: Vector = std::array::from_fn(|k| loads.force[k] - direction[k] * devices);
        for (velocity, f) in self.velocity.iter_mut().zip(force) {
            *velocity += f / mass * DT;
        }
        self.velocity[1] -= GRAVITY * DT;
        self.speed = dot(self.velocity, self.velocity).sqrt();
        if self.speed > 6000. {
            t.forces.speed_capped_from_fps = Some(self.speed);
            self.velocity = self.velocity.map(|v| v * 6000. / self.speed);
            self.speed = 6000.;
        }
        let support = force[1] / weight;
        let wheel_load = (1. - support).clamp(0., 1.);
        self.g = dot(force, basis.up) / weight;
        self.lift_g = loads.wing.lift_g;
        let rates = self.lift_controls.body_rates;
        self.maneuver = crate::telemetry::Maneuver {
            tick: self.ticks,
            commanded_g: loads.wing.commanded_g,
            lift_g: loads.wing.lift_g,
            achieved_g: self.g,
            body_rates_rad_per_second: rates,
            rudder_command: stick[2],
            rudder_deflection: self.rudder,
            effective_rudder: augmented.controls[2],
            ..Default::default()
        };
        let stall_tas = corridor_limits(model.tilt.corridor, nacelle_degrees)[0] * KT
            / (density / rotor::sea_level_density()).sqrt();
        t.power = trace::PowerTrace {
            engine: self.engine,
            fuel_starved: self.fuel + self.systems.external_lbs() <= 0.,
            afterburner: false,
            burner_blocked: self.burner_block(),
            throttle: self.throttle,
            afterburner_throttle: c.equipment.afterburner_throttle,
            fuel_flow_lbs_per_second: if self.engine { fuel_rate } else { 0. },
            unlimited_fuel: self.cheats.unlimited_fuel,
            rated_thrust_lbf: model.max_thrust,
            lapse: available / model.drive.rated_power.max(1.),
            power_available: self.systems.power_available(),
            thrust_lbf: loads.thrust_lbf,
        };
        t.forces = trace::ForceTrace {
            weight_lbs: weight,
            carried_lbs: carried,
            payload_lbs: self.payload_lbs,
            ignore_weapon_weights: self.cheats.ignore_weapon_weights,
            drag: trace::DragTrace {
                total_lbf: loads.airframe.drag_lbf + devices,
                airframe_lbf: loads.airframe.drag_lbf,
                ..drag_trace
            },
            achieved_g: self.g,
            support_g: support,
            wheel_load,
            speed_capped_from_fps: t.forces.speed_capped_from_fps,
        };
        t.envelope = limits.trace(stick[0], loads.wing.commanded_g);
        t.envelope.authority = loads.wing.authority;
        t.lift.commanded_g = loads.wing.commanded_g;
        t.lift.target_g = loads.wing.commanded_g;
        t.lift.lagged_g = loads.wing.lift_g;
        self.trace.0.adapter = Some(t);
        for (velocity, wind) in self.velocity.iter_mut().zip(initial_surface.wind) {
            *velocity += wind;
        }
        self.vertical_speed = self.velocity[1];
        // Dynamic rollover: on the wheels, banked past the limit under
        // thrust (design 4.9).
        let rolled = wheel_contact
            && hazards.dynamic_rollover
            && self.bank.abs() > ROLLOVER_BANK_DEGREES.to_radians()
            && loads.thrust_lbf > ROLLOVER_THRUST_SHARE * weight;
        let bank_before_contact = self.bank;
        let previous_position = self.advance_position();
        let surface = ground(self.position[0], self.position[2]);
        let mut research = self.research.take().expect("hybrid powered lift");
        // The stall warning, with the nacelles low: below the corridor's
        // lower edge, which on the downstops is the wing's stall speed.
        if nacelle_degrees < STALL_WARNING_DEGREES {
            let departure = research.advance(
                c,
                airspeed,
                stall_tas,
                self.pitch,
                stick[2],
                self.throttle,
                self.bank,
                self.roll_rate,
                dot(direction, basis.forward),
                false,
            );
            self.trace.0.adapter.as_mut().unwrap().departure = Some(departure);
        } else {
            research.departure = Default::default();
            research.spinning = 0;
            research.spin_rate = 0.;
            research.severity_f8 = 0;
            research.stall_active = false;
        }
        self.finish_contact(
            Some(research),
            c,
            surface,
            airframe::ContactInputs {
                wheel_load,
                runway_wind_fraction,
                previous_position,
                parked_in_wind: None,
            },
        );
        if self.crashed {
            return;
        }
        if rolled {
            self.crash_on_ground("Rolled over on the ground");
            return;
        }
        if !self.weight_on_wheels() {
            return;
        }
        // Rotor strike: the proprotors reach the ground with the nacelles
        // low unless the aircraft is rolling (design 4.9). Not a hazard the
        // Easy flight physics cheat removes.
        let ground_speed = self.velocity[0].hypot(self.velocity[2]);
        if nacelle_degrees < ROTOR_STRIKE_DEGREES && ground_speed < ROTOR_STRIKE_KT * KT {
            self.crash_on_ground("Rotor strike");
            return;
        }
        // The contact holds pitch and the loaded wheels hold yaw. A roll the
        // weight cannot hold about the wheels banks the aircraft.
        let rates = &mut self.lift_controls.body_rates;
        rates[1] = 0.;
        if wheel_load > WHEEL_YAW_HOLD {
            rates[2] = 0.;
        }
        let lifted = (weight - force[1]).max(0.) * model.rotor_parameters.airframe.half_track_ft;
        if hazards.dynamic_rollover && loads.moments.applied[0].abs() > lifted {
            self.bank = bank_before_contact;
        } else {
            rates[0] = 0.;
        }
        self.roll_rate = rates[0];
        self.pitch_rate = rates[1];
    }

    /// A crash on the wheels: engines off, stopped, with `message`.
    fn crash_on_ground(&mut self, message: &str) {
        self.crashed = true;
        self.engine = false;
        self.burner = false;
        self.velocity = [0.; 3];
        self.speed = 0.;
        self.vertical_speed = 0.;
        self.systems.notify(message);
    }
}

#[cfg(test)]
#[path = "tiltrotor_tests.rs"]
pub(crate) mod acceptance;
