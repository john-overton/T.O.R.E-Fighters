//! The angle-of-attack wing of the powered-lift aircraft (VTOL overhaul
//! design 4.3): lift follows the actual angle of attack, so the nose and the
//! flight path are coupled the way a fixed-wing aircraft's are, and the wing
//! fades out with dynamic pressure instead of being switched off.
//!
//! The conventional hybrid law commands G and aligns the nose to the flight
//! path; this wing produces the same envelope from a lift curve and a set of
//! aerodynamic moments, so it can share the sky with puffer jets, nozzles and
//! rotors that point the nose anywhere at low speed. It is tuned against the
//! conventional law on the same PT (the J4 oracle run in the jet tests):
//!
//! - **Lift** `L = C(α) x capacity`, where the capacity is the lift at the
//!   stall angle, from the aircraft's own speed envelope at this altitude,
//!   weight and flap setting ([`lift_capacity`]): 1 G at the 1 G stall
//!   speed, each row's G at that row's slow edge. So the stall speed and the
//!   available G are the conventional model's. Above the envelope's ceiling
//!   the capacity falls with the air density.
//!   `C(α)` ([`lift_coefficient`]) is linear from the zero-lift angle to 1 at
//!   the stall angle, falls to 0.7 over the next 15 degrees, holds to 45
//!   degrees and fades to nothing at 90; the negative side mirrors it.
//! - **Pitch**: a short-period law, stiffness and damping growing with
//!   dynamic pressure, `M = Iyy [Ws² sin(α_cmd - α) + 2 ζ Ws (q_cmd - q)]`
//!   (the angle-of-attack term limited to closing at 25 degrees per second),
//!   `Ws = Ws_ref sqrt(q̄ / q̄_ref)`. The stick commands G exactly as the
//!   conventional law does, within the same envelope limits; `α_cmd` is the
//!   angle at which the wing supplies the commanded G less what the thrust
//!   already gives along the lift direction, and `q_cmd = (n_cmd - up_y) g /
//!   V` is the conventional pitch-rate target. Neutral stick is 1 G. Below
//!   the stall speed, where the wing cannot lift the weight, the command
//!   blends (by the share of the weight the wing can lift) into the stick
//!   setting the angle of attack directly, neutral holding a trim angle, so
//!   a jetborne aircraft's nose answers the stick instead of the wing
//!   chasing a G it cannot make.
//! - **Roll**: the stick commands the PT roll rate (`_brv`) above twice the
//!   stall speed, as the conventional law does, reached through a lag that
//!   shrinks with dynamic pressure and the PT roll acceleration limits.
//!   Dihedral rolls away from sideslip.
//! - **Yaw**: weathercock stability, yaw damping toward the turn's own yaw
//!   rate (the conventional turn coordination term) and the rudder, which
//!   holds a sideslip like the conventional rudder does.
//! - **Side force and slip drag** are the conventional tuning's, faded with
//!   dynamic pressure below the stall speed's.
//!
//! The wing is generic: the vectoring jets fly it (slice P4) and the V-22
//! reuses it in airplane mode (slice P5) with its own [`WingShape`] and roll
//! law. Every constant is `fitted` (agent decisions, 2026-10-08) unless its
//! line says otherwise.

use super::body::{GRAVITY, Inertia, Moments};
use crate::attitude::{Basis, Vector, cross, dot, unit};

/// Sea-level standard air density, slug/ft³.
pub const SEA_LEVEL_DENSITY: f64 = 0.002_376_9;
const KG_M3_TO_SLUG_FT3: f64 = 0.001_940_32;
/// The lowest speed the pitch-rate and turn-coordination targets divide by,
/// ft/s: the conventional law's floor.
const RATE_SPEED_FLOOR_FPS: f64 = 60.;

/// Standard-day air density, slug/ft³, at `altitude_ft` (held at the
/// atmosphere model's range limits).
pub fn density(altitude_ft: f64) -> f64 {
    crate::telemetry::Atmosphere::standard(altitude_ft.clamp(-2_000., 100_000.))
        .map(|a| a.density_kg_m3() * KG_M3_TO_SLUG_FT3)
        .unwrap_or(SEA_LEVEL_DENSITY)
}

/// The air the aircraft flies through, from its attitude and air-relative
/// velocity.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirData {
    /// Air-relative velocity, world axes, ft/s.
    pub velocity: Vector,
    /// True airspeed, ft/s.
    pub speed: f64,
    /// Velocity in body axes [forward, right, up], ft/s.
    pub body: [f64; 3],
    /// Angle of attack, rad: positive when the air meets the wing from
    /// below. Near ±π flying backward.
    pub alpha: f64,
    /// Sideslip, rad: positive when the aircraft slides to its right.
    pub beta: f64,
    /// Air density, slug/ft³.
    pub density: f64,
    /// Dynamic pressure, lbf/ft².
    pub qbar: f64,
}

impl AirData {
    pub fn new(basis: &Basis, velocity: Vector, altitude_ft: f64) -> Self {
        let speed = dot(velocity, velocity).sqrt();
        let body = [
            dot(velocity, basis.forward),
            dot(velocity, basis.right),
            dot(velocity, basis.up),
        ];
        let density = density(altitude_ft);
        Self {
            velocity,
            speed,
            body,
            alpha: (-body[2]).atan2(body[0]),
            beta: if speed > 1e-9 {
                (body[1] / speed).clamp(-1., 1.).asin()
            } else {
                0.
            },
            density,
            qbar: 0.5 * density * speed * speed,
        }
    }
}

/// The shape of a wing's lift curve and the strength of its aerodynamic
/// moments, relative to the dynamic pressure at its stall speed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WingShape {
    /// Angle of attack with no lift, rad.
    pub zero_lift_alpha: f64,
    /// Angle of attack of maximum lift, rad.
    pub stall_alpha: f64,
    /// Angle of attack neutral stick holds below the stall speed, rad.
    pub trim_alpha: f64,
    /// Short-period natural frequency at the stall speed's dynamic pressure,
    /// rad/s; it grows with the square root of dynamic pressure.
    pub short_period_rad_per_second: f64,
    /// Short-period damping ratio.
    pub short_period_damping: f64,
    /// Fastest the pitch law closes an angle-of-attack error, rad/s.
    pub alpha_rate_limit: f64,
    /// Roll lag at the stall speed's dynamic pressure, s; it shrinks with
    /// the square root of dynamic pressure down to `roll_seconds_minimum`.
    pub roll_seconds: f64,
    pub roll_seconds_minimum: f64,
    /// Dihedral: roll acceleration, rad/s², per unit sine of sideslip at the
    /// stall speed's dynamic pressure, growing with dynamic pressure.
    pub dihedral: f64,
    /// Weathercock natural frequency at the stall speed's dynamic pressure,
    /// rad/s, growing with the square root of dynamic pressure.
    pub weathercock_rad_per_second: f64,
    /// Yaw damping ratio.
    pub weathercock_damping: f64,
}

/// The vectoring jets' wing. The stall angle and the moment strengths are
/// fitted to fly like the conventional law on the same PT: about a quarter
/// second of G onset at combat speed, the PT roll rate, a few degrees of
/// sideslip under full rudder.
pub const JET_WING: WingShape = WingShape {
    zero_lift_alpha: 0.,
    stall_alpha: 18. * std::f64::consts::PI / 180.,
    trim_alpha: 4. * std::f64::consts::PI / 180.,
    short_period_rad_per_second: 2.,
    short_period_damping: 0.7,
    alpha_rate_limit: 25. * std::f64::consts::PI / 180.,
    roll_seconds: 0.5,
    roll_seconds_minimum: 0.12,
    dihedral: 0.35,
    weathercock_rad_per_second: 1.5,
    weathercock_damping: 0.6,
};

/// The roll law's PT figures: the maximum rate, rad/s, and the onset and
/// release accelerations, rad/s² (`_brv` on the jets).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RollLaw {
    pub maximum: f64,
    pub acceleration: f64,
    pub deceleration: f64,
}

impl RollLaw {
    /// The roll law of a handling profile's roll axis.
    pub fn from_axis(axis: tore_formats::flight_model::normal_control::LoadedAxis) -> Self {
        Self {
            maximum: f64::from(axis.maximum).to_radians(),
            acceleration: f64::from(axis.acceleration).to_radians(),
            deceleration: f64::from(axis.deceleration).to_radians(),
        }
    }
}

/// `C(α)`: lift as a share of the lift at the stall angle (see the module
/// documentation).
pub fn lift_coefficient(shape: &WingShape, alpha: f64) -> f64 {
    let from_zero = alpha - shape.zero_lift_alpha;
    let past = from_zero.abs();
    let stall = shape.stall_alpha - shape.zero_lift_alpha;
    let fade = 15_f64.to_radians();
    let hold = 45_f64.to_radians();
    let right = std::f64::consts::FRAC_PI_2;
    let magnitude = if past <= stall {
        past / stall
    } else if past <= stall + fade {
        1. - 0.3 * (past - stall) / fade
    } else if past <= hold {
        0.7
    } else if past <= right {
        0.7 * (right - past) / (right - hold)
    } else {
        0.
    };
    magnitude.copysign(from_zero)
}

/// The angle of attack at which the lift is `c` of the stall lift, on the
/// linear part of the curve; beyond it, the stall angle.
pub fn alpha_for(shape: &WingShape, c: f64) -> f64 {
    shape.zero_lift_alpha + c.clamp(-1., 1.) * (shape.stall_alpha - shape.zero_lift_alpha)
}

/// How much a wing can lift now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiftCapacity {
    /// Lift at the stall angle, lbf.
    pub lift_lbf: f64,
    /// Dynamic pressure at the 1 G stall speed with the clean wing, lbf/ft²:
    /// the reference the moments scale with.
    pub reference_qbar: f64,
    /// The 1 G stall speed with the clean wing at this altitude and weight,
    /// ft/s.
    pub clean_stall_fps: f64,
}

/// The lift at the stall angle of an aircraft of `weight_lbs` from its
/// speed envelope (`envelopes`, already weight-scaled): it lifts 1 G at the
/// 1 G row's slow edge (with the conventional hybrid flap rule, stall speed
/// x (1 - 0.25 x flaps)) and each higher row's G at that row's slow edge,
/// in straight lines between them; below the stall speed and above the top
/// row the lift goes with airspeed squared. So the wing reaches every G the
/// conventional model's envelope allows at the same speed, and the stall
/// speed is the same. (The retail rows are not a speed-squared family: the
/// AV-8's 5 G row starts far slower than five times its 1 G stall
/// pressure.) Above the envelope's ceiling the ceiling's 1 G stall speed
/// holds and the lift falls with the density.
pub fn lift_capacity(
    envelopes: &[tore_formats::aircraft::Envelope],
    altitude_ft: f64,
    weight_lbs: f64,
    flaps: f64,
    air: &AirData,
) -> LiftCapacity {
    let one = envelopes.iter().find(|e| e.g == 1);
    let inside = one.is_some_and(|e| e.speeds(altitude_ft).is_some());
    let reference_altitude = match one {
        Some(e) if !inside => {
            let top = e.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
            let bottom = e.points.iter().map(|p| p[1]).fold(f64::MAX, f64::min);
            if altitude_ft > top { top - 1. } else { bottom }
        }
        _ => altitude_ft,
    };
    let stall = one
        .and_then(|e| e.speeds(reference_altitude))
        .map_or(200., |(low, _)| low)
        .max(1.);
    let flapped = stall * (1. - 0.25 * flaps.clamp(0., 1.));
    // The rows' slow edges, rising in G and in speed.
    let mut edges = vec![(flapped, 1.)];
    if inside {
        let mut rows: Vec<_> = envelopes.iter().filter(|e| e.g > 1).collect();
        rows.sort_by_key(|e| e.g);
        for row in rows {
            if let Some((low, _)) = row.speeds(altitude_ft)
                && low > edges.last().unwrap().0
            {
                edges.push((low, f64::from(row.g)));
            }
        }
    }
    let speed = air.speed;
    let g = match edges.iter().position(|(edge, _)| *edge > speed) {
        Some(0) => (speed / flapped).powi(2),
        Some(i) => {
            let ((v0, g0), (v1, g1)) = (edges[i - 1], edges[i]);
            g0 + (g1 - g0) * (speed - v0) / (v1 - v0)
        }
        None => {
            let (v, g) = *edges.last().unwrap();
            g * (speed / v).powi(2)
        }
    };
    let reference_density = density(reference_altitude);
    LiftCapacity {
        lift_lbf: weight_lbs * g * air.density / reference_density,
        reference_qbar: 0.5 * reference_density * stall * stall,
        clean_stall_fps: stall,
    }
}

/// Everything the wing needs besides the air data.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WingInputs {
    pub shape: WingShape,
    pub roll: RollLaw,
    pub basis: Basis,
    /// Body rates [roll, pitch, yaw], rad/s.
    pub rates: [f64; 3],
    pub inertia: Inertia,
    pub weight_lbs: f64,
    pub capacity: LiftCapacity,
    /// Lift multiplier for damage (regional lift, wing damage).
    pub lift_scale: f64,
    /// G limits [negative, positive] (the conventional envelope's).
    pub limits: [f64; 2],
    /// Pilot controls [pitch, roll, yaw], -1..1.
    pub controls: [f64; 3],
    /// Effective rudder, -1..1 (deflection with damage authority and bias).
    pub rudder: f64,
    /// Non-aerodynamic forces (thrust), world axes, lbf: the wing supplies
    /// the commanded G less their share along the lift direction.
    pub other_force: Vector,
    /// Sideslip a full rudder holds, rad.
    pub rudder_slip: f64,
    /// Side force per unit sideslip velocity, 1/s, at or above the stall
    /// speed's dynamic pressure (conventional `sideslip_force`).
    pub side_force: f64,
}

/// What the wing does this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WingForces {
    /// Lift and side force, world axes, lbf.
    pub force: Vector,
    /// Aerodynamic moments, body axes.
    pub moments: Moments,
    /// Lift as a share of weight, G.
    pub lift_g: f64,
    /// The G the stick commands, within the limits.
    pub commanded_g: f64,
    /// The angle of attack the pitch law drives toward, rad.
    pub alpha_command: f64,
    /// Share of the conventional control authority: (V / Vs)², at most 1.
    pub authority: f64,
}

/// The wing's forces and moments for `air` (see the module documentation).
pub fn wing(air: &AirData, w: &WingInputs) -> WingForces {
    let shape = &w.shape;
    let basis = &w.basis;
    let [ixx, iyy, izz] = w.inertia.0;
    let [stick_pitch, stick_roll, _] = w.controls;
    let [lo, hi] = w.limits;
    let reference = w.capacity.reference_qbar.max(1e-9);
    let q_ratio = air.qbar / reference;
    // Lift, perpendicular to the air in the symmetry plane.
    let along_right = air.body[1];
    let symmetric: Vector = std::array::from_fn(|i| air.velocity[i] - along_right * basis.right[i]);
    let symmetric_speed = dot(symmetric, symmetric).sqrt();
    let lift_direction = if symmetric_speed > 1e-6 {
        unit(cross(symmetric, basis.right))
    } else {
        basis.up
    };
    let capacity = w.capacity.lift_lbf * w.lift_scale;
    let lift = capacity * lift_coefficient(shape, air.alpha);
    // The stick commands G as the conventional law does; the wing supplies
    // what the thrust does not.
    let commanded_g =
        (1. + stick_pitch * if stick_pitch > 0. { hi - 1. } else { 1. - lo }).clamp(lo, hi);
    // Thrust along the lift direction relieves the wing of a positive
    // command, never more than all of it.
    let relief = dot(w.other_force, lift_direction).clamp(0., (commanded_g * w.weight_lbs).max(0.));
    let needed = commanded_g * w.weight_lbs - relief;
    let c_command = if capacity > 1e-9 {
        needed / capacity
    } else {
        needed.signum()
    };
    // Below the stall speed the wing cannot carry the aircraft and the G
    // command gives way to the stick setting the angle of attack directly,
    // as a plain stabilator would: neutral holds the trim angle.
    let share = (capacity / (w.lift_scale * w.weight_lbs).max(1.)).clamp(0., 1.);
    let stick_alpha = if stick_pitch >= 0. {
        shape.trim_alpha + stick_pitch * (shape.stall_alpha - shape.trim_alpha)
    } else {
        shape.trim_alpha + stick_pitch * (shape.stall_alpha + shape.trim_alpha)
    };
    let alpha_command = alpha_for(shape, c_command) * share + stick_alpha * (1. - share);
    // Pitch: short period about the commanded angle of attack.
    let rate_speed = air.speed.max(RATE_SPEED_FLOOR_FPS);
    let ws = shape.short_period_rad_per_second * q_ratio.sqrt();
    let pitch_rate_command = (commanded_g - basis.up[1]) * GRAVITY / rate_speed;
    // The short period as a pitch-rate target: the conventional rate plus
    // the angle-of-attack error closing at Ws / 2ζ, no faster than the
    // shape's limit, so the nose does not whip past the turn's own rate.
    let pitch_damping = iyy * 2. * shape.short_period_damping * ws;
    let closing = ws / (2. * shape.short_period_damping) * (alpha_command - air.alpha).sin();
    let pitch_moment = pitch_damping
        * (pitch_rate_command + closing.clamp(-shape.alpha_rate_limit, shape.alpha_rate_limit));
    // Roll: the conventional roll command through a lag and the PT
    // acceleration limits.
    let roll_authority = (air.speed / (2. * w.capacity.clean_stall_fps)).clamp(0., 1.);
    let roll_command = stick_roll * w.roll.maximum * roll_authority;
    let roll_rate = w.rates[0];
    let roll_moment = if air.qbar > 1e-9 {
        let lag = (shape.roll_seconds / q_ratio.sqrt()).max(shape.roll_seconds_minimum);
        let releasing = roll_command.abs() < roll_rate.abs() && roll_command * roll_rate >= 0.;
        let limit = if releasing {
            w.roll.deceleration
        } else {
            w.roll.acceleration
        };
        ixx * ((roll_command - roll_rate) / lag).clamp(-limit, limit)
    } else {
        0.
    };
    let dihedral = -ixx * shape.dihedral * air.beta.sin() * q_ratio;
    // Yaw: weathercock, damping toward the turn's yaw rate, rudder.
    let wy = shape.weathercock_rad_per_second * q_ratio.sqrt();
    let turn_yaw = -basis.right[1] * GRAVITY / rate_speed;
    let yaw_damping = izz * 2. * shape.weathercock_damping * wy;
    let yaw_moment =
        izz * wy * wy * (air.beta.sin() + w.rudder_slip * w.rudder) + yaw_damping * turn_yaw;
    // Side force, as the conventional law's above the stall speed.
    let mass = w.weight_lbs / GRAVITY;
    let side = -mass * w.side_force * air.body[1] * q_ratio.min(1.);
    WingForces {
        force: std::array::from_fn(|i| lift_direction[i] * lift + basis.right[i] * side),
        moments: Moments {
            applied: [roll_moment + dihedral, pitch_moment, yaw_moment],
            damping: [0., pitch_damping, yaw_damping],
        },
        lift_g: lift / w.weight_lbs.max(1.),
        commanded_g,
        alpha_command,
        authority: q_ratio.min(1.),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lift_curve_is_linear_to_the_stall_then_falls_and_mirrors() {
        let s = JET_WING;
        assert_eq!(lift_coefficient(&s, 0.), 0.);
        assert!((lift_coefficient(&s, s.stall_alpha) - 1.).abs() < 1e-12);
        assert!((lift_coefficient(&s, s.stall_alpha / 2.) - 0.5).abs() < 1e-12);
        let fallen = s.stall_alpha + 15_f64.to_radians();
        assert!((lift_coefficient(&s, fallen) - 0.7).abs() < 1e-12);
        assert!((lift_coefficient(&s, 40_f64.to_radians()) - 0.7).abs() < 1e-12);
        assert!(lift_coefficient(&s, std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(lift_coefficient(&s, 3.), 0.);
        assert_eq!(
            lift_coefficient(&s, -0.1),
            -lift_coefficient(&s, 0.1),
            "mirrored"
        );
        assert!((alpha_for(&s, 0.5) - s.stall_alpha / 2.).abs() < 1e-12);
        assert_eq!(alpha_for(&s, 3.), s.stall_alpha);
    }

    #[test]
    fn air_data_signs_follow_the_body_axes() {
        let basis = Basis::new(0., 0., 0.);
        // Level, sinking and sliding right: positive alpha and beta.
        let air = AirData::new(&basis, [10., -20., 200.], 0.);
        assert!(air.alpha > 0. && air.beta > 0.);
        assert!((air.qbar / (0.5 * SEA_LEVEL_DENSITY * air.speed * air.speed) - 1.).abs() < 5e-3);
        assert!(density(30_000.) < 0.5 * SEA_LEVEL_DENSITY);
        let still = AirData::new(&basis, [0.; 3], 0.);
        assert_eq!((still.alpha, still.beta, still.qbar), (0., 0., 0.));
    }

    #[test]
    fn the_capacity_lifts_the_weight_at_the_stall_speed_and_thins_above_the_ceiling() {
        let envelopes = vec![tore_formats::aircraft::Envelope {
            g: 1,
            points: vec![[200., 0.], [300., 30_000.], [800., 30_000.], [900., 0.]],
        }];
        let basis = Basis::new(0., 0., 0.);
        let at = |altitude: f64, speed: f64, flaps: f64| {
            let air = AirData::new(&basis, [0., 0., speed], altitude);
            lift_capacity(&envelopes, altitude, 10_000., flaps, &air)
        };
        assert!((at(0., 200., 0.).lift_lbf - 10_000.).abs() < 1e-6);
        assert!((at(15_000., 250., 0.).lift_lbf - 10_000.).abs() < 1e-6);
        assert!((at(0., 400., 0.).lift_lbf - 40_000.).abs() < 1e-6);
        // Full flaps take a quarter off the stall speed.
        assert!((at(0., 150., 1.).lift_lbf - 10_000.).abs() < 1e-6);
        // Above the ceiling, at the ceiling's stall speed, the lift falls
        // with the density.
        let high = at(40_000., 300., 0.);
        assert!(high.lift_lbf < 0.8 * 10_000. && high.lift_lbf > 0.);
        assert_eq!(high.clean_stall_fps, at(29_999., 300., 0.).clean_stall_fps);
    }
}
