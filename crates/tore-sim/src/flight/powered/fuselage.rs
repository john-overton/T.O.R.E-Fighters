//! A rotorcraft's fuselage and fixed surfaces (VTOL overhaul design 4.4 and
//! 4.5): parasite drag, the horizontal tail's speed stability, the fin's
//! weathercock stability and its share of the anti-torque load at speed,
//! and stub wings. Body axes and moment signs are those of [`super::body`].
//!
//! - **Drag**: `0.5 rho |v| (f_x u, f_y v, f_z w)` against the air velocity's
//!   body components, so a vertical descent meets the fuselage's plan area
//!   and sideways flight its side, with stores and damage adding to it.
//! - **Horizontal tail**: a pitch moment `-M_alpha qbar (alpha - alpha_t)`
//!   (angle of attack limited to the tail's stall) and the pitch damping of
//!   the tail swinging through the air, both fading as the air comes from
//!   behind.
//! - **Fin**: weathercock yaw `N_beta qbar beta`, yaw damping, and a
//!   cambered-fin yaw moment against the main rotor's torque that grows with
//!   dynamic pressure, so the pedals sit near neutral in cruise.
//! - **Stub wings**: lift `qbar S a (alpha + i)` perpendicular to the air
//!   velocity in the symmetry plane, held at the stall angle beyond it.
//!
//! The wings and tail surfaces of the jets and the V-22 are slice P4's
//! `aero.rs`; these are the helicopters' simpler surfaces.

use super::body::body_axes;
use crate::{
    attitude::{Basis, Vector, dot},
    models::variety::RotorcraftAirframe,
};

/// Airspeed below which the surfaces are ignored, ft/s.
const MINIMUM_SURFACE_SPEED: f64 = 5.;
/// The horizontal tail stalls here, rad.
const TAIL_STALL: f64 = 0.26;
/// Dynamic pressure at 120 kt at sea level, psf, where the fin carries its
/// share of the torque.
const FIN_REFERENCE_QBAR: f64 = 0.5 * 0.002_377 * (120. * 1.687_81) * (120. * 1.687_81);
/// The fin's camber moment stops growing at this multiple of its share.
const FIN_SHARE_LIMIT: f64 = 2.;

/// What the airframe adds to the forces and moments this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirframeLoads {
    /// World axes, lbf.
    pub force: Vector,
    /// Of which drag, lbf (for the trace).
    pub drag_lbf: f64,
    /// Of which stub wing lift, lbf.
    pub wing_lift_lbf: f64,
    /// Body axes [roll, pitch, yaw], ft·lbf.
    pub moments: [f64; 3],
    /// Rate damping, ft·lbf per rad/s.
    pub damping: [f64; 3],
}

/// What the airframe needs besides its parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirframeInput {
    pub basis: Basis,
    /// Air velocity, world axes, ft/s.
    pub air_velocity: Vector,
    pub density: f64,
    /// Tail arm behind the centre of gravity, ft.
    pub tail_arm_ft: f64,
    /// Drag growth from stores and damage, as a factor.
    pub drag_factor: f64,
    /// The main rotor's turn (+1 counter-clockwise from above) and its
    /// reference hover torque, ft·lbf, for the cambered fin.
    pub turn: f64,
    pub reference_torque: f64,
    /// Lift lost to wing damage, as a factor.
    pub lift_factor: f64,
}

/// The airframe's loads now.
pub fn airframe_loads(p: &RotorcraftAirframe, i: &AirframeInput) -> AirframeLoads {
    let basis = i.basis;
    let v = i.air_velocity;
    let rho = i.density;
    let speed = dot(v, v).sqrt();
    let [u, side, w] = [dot(v, basis.forward), dot(v, basis.right), dot(v, basis.up)];
    let [fx, fy, fz] = p.flat_plate_ft2;
    let half = 0.5 * rho * speed * i.drag_factor;
    let drag_body = [-half * fx * u, -half * fy * side, -half * fz * w];
    let mut force: Vector = std::array::from_fn(|k| {
        drag_body[0] * basis.forward[k] + drag_body[1] * basis.right[k] + drag_body[2] * basis.up[k]
    });
    let drag_lbf = dot(force, force).sqrt();
    let mut loads = AirframeLoads {
        drag_lbf,
        ..Default::default()
    };
    if speed < MINIMUM_SURFACE_SPEED {
        loads.force = force;
        return loads;
    }
    let qbar = 0.5 * rho * speed * speed;
    // Surfaces work in air from ahead; they fade out as it comes from the
    // side or behind.
    let ahead = (u / speed).clamp(0., 1.);
    let alpha = (-w).atan2(u.max(1e-9));
    let beta = (side / speed).clamp(-1., 1.).asin();
    let tail_alpha = (alpha - p.tail_trim_degrees.to_radians()).clamp(-TAIL_STALL, TAIL_STALL);
    loads.moments[1] = -p.tail_pitch_ft3 * qbar * tail_alpha * ahead;
    loads.damping[1] = p.tail_pitch_ft3 * 0.5 * rho * speed * i.tail_arm_ft * ahead;
    let camber = -i.turn
        * p.fin_torque_share
        * i.reference_torque
        * (qbar * ahead * ahead / FIN_REFERENCE_QBAR).min(FIN_SHARE_LIMIT);
    loads.moments[2] = p.fin_yaw_ft3 * qbar * beta.clamp(-TAIL_STALL, TAIL_STALL) * ahead + camber;
    loads.damping[2] = p.fin_yaw_ft3 * 0.5 * rho * speed * i.tail_arm_ft * ahead;
    if let Some(wing) = p.stub_wing
        && u > 0.
    {
        let plane = (u * u + w * w).sqrt();
        let stall = wing.stall_degrees.to_radians();
        let angle = (alpha + wing.incidence_degrees.to_radians()).clamp(-stall, stall);
        let lift = 0.5 * rho * plane * plane * wing.lift_area_ft2 * angle * i.lift_factor;
        // Perpendicular to the air velocity in the symmetry plane.
        let direction: Vector =
            std::array::from_fn(|k| (u * basis.up[k] - w * basis.forward[k]) / plane);
        for (f, d) in force.iter_mut().zip(direction) {
            *f += lift * d;
        }
        loads.wing_lift_lbf = lift;
    }
    loads.force = force;
    loads
}

/// The body moment components [roll, pitch, yaw] of a world moment vector.
pub fn body_moments(basis: &Basis, moment: Vector) -> [f64; 3] {
    body_axes(basis).map(|axis| dot(moment, axis))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn airframe() -> RotorcraftAirframe {
        RotorcraftAirframe {
            flat_plate_ft2: [40., 200., 300.],
            tail_pitch_ft3: 2_000.,
            tail_trim_degrees: 0.,
            fin_yaw_ft3: 1_500.,
            fin_torque_share: 0.5,
            half_track_ft: 3.,
            stub_wing: None,
        }
    }

    fn input(velocity: Vector) -> AirframeInput {
        AirframeInput {
            basis: Basis::new(0., 0., 0.),
            air_velocity: velocity,
            density: 0.002_377,
            tail_arm_ft: 30.,
            drag_factor: 1.,
            turn: 1.,
            reference_torque: 40_000.,
            lift_factor: 1.,
        }
    }

    #[test]
    fn drag_opposes_the_airflow_along_each_body_axis() {
        // Level basis: forward is +z, right +x, up +y.
        let forward = airframe_loads(&airframe(), &input([0., 0., 200.]));
        assert!((forward.force[2] + 0.5 * 0.002_377 * 200. * 200. * 40.).abs() < 1e-9);
        let falling = airframe_loads(&airframe(), &input([0., -50., 0.]));
        assert!(falling.force[1] > 0. && falling.force[0] == 0.);
        // The fuselage plan area stops a vertical fall harder than its
        // frontal area stops forward flight.
        let fall = airframe_loads(&airframe(), &input([0., -100., 0.]));
        let fly = airframe_loads(&airframe(), &input([0., 0., 100.]));
        assert!(fall.drag_lbf > 5. * fly.drag_lbf);
    }

    #[test]
    fn the_tail_and_fin_restore_and_the_fin_carries_torque_at_speed() {
        // Nose above the flight path: nose-down moment. Air from the right
        // (sideslip right): nose right, into it.
        let climbing = airframe_loads(&airframe(), &input([0., -20., 200.]));
        assert!(climbing.moments[1] < 0.);
        let slipping = airframe_loads(&airframe(), &input([20., 0., 200.]));
        assert!(
            slipping.moments[2] > airframe_loads(&airframe(), &input([0., 0., 200.])).moments[2]
        );
        // A counter-clockwise rotor's torque yaws the nose right; the fin
        // pushes it left, by its share at 120 kt.
        let cruise = airframe_loads(&airframe(), &input([0., 0., 120. * 1.687_81]));
        assert!(
            (cruise.moments[2] + 0.5 * 40_000.).abs() < 1.,
            "{}",
            cruise.moments[2]
        );
        let hover = airframe_loads(&airframe(), &input([0., 0., 1.]));
        assert_eq!(hover.moments, [0.; 3]);
    }
}
