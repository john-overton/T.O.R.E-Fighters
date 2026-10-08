//! The rigid body the powered-lift aircraft fly on (VTOL overhaul design
//! 4.1): body rates are state, moments change them through a three-axis
//! inertia, and the attitude follows the rates. Deterministic f64 arithmetic
//! in a fixed order at the fixed 120 Hz tick, with no iteration.
//!
//! **Axes.** Body rates `[p, q, r]` are roll, pitch and yaw in the sign
//! convention of the conventional adapter's rates: positive roll is right
//! wing down (about `-forward`), positive pitch is nose up (about `-right`)
//! and positive yaw is nose right (about `up`). Those three axes form a
//! right-handed frame ([`body_axes`]). Moments use the same axes and signs,
//! in ft·lbf.
//!
//! **Integration.** Each tick ([`advance`]):
//!
//! 1. The rates take the applied moments, with rate damping treated
//!    implicitly so stiff rotor and stability-augmentation damping stays
//!    stable without sub-steps:
//!    `ω' = (ω + dt M / I) / (1 + dt D / I)` per axis.
//! 2. The angular momentum `I ω'` is taken into world axes.
//! 3. The attitude turns (the Rodrigues rotation of [`Basis::rotated`]) by
//!    the mean of the world rotation rate at the start and at a predicted
//!    end, where the predicted end is the attitude turned by `ω' dt` with
//!    the rates that momentum gives it there. Second order in the step.
//! 4. The new rates are that momentum expressed in the turned body axes,
//!    divided by the inertia. This is where the gyroscopic term
//!    `-ω × (I ω)` of Euler's equation comes from; with no moment the
//!    angular momentum stays exactly put in world axes, up to rounding, and
//!    the energy of a free tumble moves by parts in a million a minute.
//!
//! The steps are a fixed sequence: two rotations and three projections a
//! tick, no iteration.
//!
//! Forces and the linear motion stay in the force laws, which integrate
//! velocity and position after the attitude, as the conventional step does.

use super::super::{DT, State};
use crate::attitude::{Basis, Vector, dot};

/// Standard gravity, ft/s², for weight to mass.
pub const GRAVITY: f64 = 32.174;

/// Principal moments of inertia about the roll, pitch and yaw axes,
/// slug·ft². Every one is positive and finite.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Inertia(pub [f64; 3]);

impl Inertia {
    /// `I = m k²` for an aircraft of `weight_lbs` with radii of gyration
    /// `radii_ft` (the aircraft's [`crate::models::variety::BodyParameters`]).
    pub fn from_weight(weight_lbs: f64, radii_ft: [f64; 3]) -> Self {
        let mass = weight_lbs / GRAVITY;
        Self(radii_ft.map(|k| mass * k * k))
    }
}

/// What acts on the body during one tick, about the centre of gravity in
/// body axes [roll, pitch, yaw].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Moments {
    /// Moments from the start-of-tick state, ft·lbf.
    pub applied: [f64; 3],
    /// Rate damping, ft·lbf per rad/s, at least zero: a moment of
    /// `-damping x rate` applied implicitly.
    pub damping: [f64; 3],
}

/// The world directions of the roll, pitch and yaw axes of `basis`.
pub fn body_axes(basis: &Basis) -> [Vector; 3] {
    [basis.forward.map(|v| -v), basis.right.map(|v| -v), basis.up]
}

/// The rotation vector, world axes, rad, that turns `basis` at body `rates`
/// for `dt`. The same products in the same order as the old powered law's
/// rotation, so that law turns through here bit for bit.
pub fn rotation(basis: &Basis, rates: [f64; 3], dt: f64) -> Vector {
    let [roll, pitch, yaw] = rates;
    std::array::from_fn(|i| {
        -basis.right[i] * pitch * dt - basis.forward[i] * roll * dt + basis.up[i] * yaw * dt
    })
}

/// Angular momentum in world axes, slug·ft²/s.
pub fn angular_momentum(basis: &Basis, rates: [f64; 3], inertia: Inertia) -> Vector {
    let axes = body_axes(basis);
    std::array::from_fn(|i| {
        (0..3)
            .map(|axis| axes[axis][i] * inertia.0[axis] * rates[axis])
            .sum()
    })
}

/// Rotational kinetic energy, ft·lbf.
pub fn kinetic_energy(rates: [f64; 3], inertia: Inertia) -> f64 {
    (0..3)
        .map(|axis| 0.5 * inertia.0[axis] * rates[axis] * rates[axis])
        .sum()
}

/// One tick of the rigid body: the attitude and body rates after `dt` under
/// `moments`. See the module documentation for the steps.
pub fn advance(
    basis: Basis,
    rates: [f64; 3],
    inertia: Inertia,
    moments: Moments,
    dt: f64,
) -> (Basis, [f64; 3]) {
    let turned: [f64; 3] = std::array::from_fn(|axis| {
        let i = inertia.0[axis];
        (rates[axis] + dt * moments.applied[axis] / i) / (1. + dt * moments.damping[axis] / i)
    });
    let momentum = angular_momentum(&basis, turned, inertia);
    // Predict the turned attitude and the rates it would have, then turn by
    // the mean of the two world rotation rates: second order in the step,
    // so a free tumble keeps its energy as well as its momentum.
    let predicted = basis.rotated(rotation(&basis, turned, dt));
    let predicted_rates = body_rates(momentum, &predicted, inertia);
    let start = rotation(&basis, turned, dt);
    let end = rotation(&predicted, predicted_rates, dt);
    let next = basis.rotated(std::array::from_fn(|i| 0.5 * (start[i] + end[i])));
    (next, body_rates(momentum, &next, inertia))
}

/// The body rates of a body at `basis` carrying world angular `momentum`.
fn body_rates(momentum: Vector, basis: &Basis, inertia: Inertia) -> [f64; 3] {
    let axes = body_axes(basis);
    std::array::from_fn(|axis| dot(momentum, axes[axis]) / inertia.0[axis])
}

impl State {
    /// One tick of the rigid body for the force laws of slices P2 and P4:
    /// turns the attitude, updates the body rates in `lift_controls`,
    /// mirrors roll and pitch into `roll_rate` and `pitch_rate` for
    /// telemetry, and returns the new basis. Velocity and position are the
    /// caller's.
    pub fn advance_body(&mut self, inertia: Inertia, moments: Moments) -> Basis {
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let (next, rates) = advance(basis, self.lift_controls.body_rates, inertia, moments, DT);
        [self.yaw, self.pitch, self.bank] = next.angles();
        self.lift_controls.body_rates = rates;
        self.roll_rate = rates[0];
        self.pitch_rate = rates[1];
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attitude::cross;

    fn length(v: Vector) -> f64 {
        dot(v, v).sqrt()
    }

    #[test]
    fn the_body_axes_are_right_handed_and_match_the_rate_signs() {
        let basis = Basis::new(0.4, -0.3, 0.7);
        let [roll, pitch, yaw] = body_axes(&basis);
        let z = cross(roll, pitch);
        assert!(z.iter().zip(yaw).all(|(a, b)| (a - b).abs() < 1e-12));
        // Positive pitch raises the nose, positive roll lowers the right
        // wing and positive yaw moves the nose right.
        let level = Basis::new(0., 0., 0.);
        let turned = |rates| level.rotated(rotation(&level, rates, 0.01));
        assert!(turned([0., 1., 0.]).forward[1] > 0.);
        assert!(turned([1., 0., 0.]).right[1] < 0.);
        assert!(dot(turned([0., 0., 1.]).forward, level.right) > 0.);
    }

    /// An asymmetric body spinning about its unstable middle axis (yaw here)
    /// tumbles over and over with no moment. For a minute its angular
    /// momentum in world axes stays put to rounding and its energy within a
    /// part in ten thousand.
    #[test]
    fn a_torque_free_tumble_conserves_angular_momentum() {
        let inertia = Inertia([5_000., 40_000., 25_000.]);
        let mut basis = Basis::new(0.3, 0.2, -0.1);
        let mut rates = [0.01, 0.01, 1.];
        let start = angular_momentum(&basis, rates, inertia);
        let energy = kinetic_energy(rates, inertia);
        let mut flipped = false;
        let mut largest_drift: f64 = 0.;
        for _ in 0..120 * 60 {
            (basis, rates) = advance(basis, rates, inertia, Moments::default(), DT);
            let now = angular_momentum(&basis, rates, inertia);
            let drift = length(std::array::from_fn(|i| now[i] - start[i])) / length(start);
            largest_drift = largest_drift.max(drift);
            flipped |= rates[2] < 0.;
            assert!(rates.iter().all(|r| r.is_finite()));
        }
        assert!(
            largest_drift < 1e-9,
            "angular momentum drifted {largest_drift}"
        );
        let energy_ratio = kinetic_energy(rates, inertia) / energy;
        assert!(
            (energy_ratio - 1.).abs() < 1e-4,
            "energy moved by {energy_ratio}"
        );
        // A real tumble, not a steady spin: the yaw rate reversed.
        assert!(flipped);
    }

    #[test]
    fn a_spin_about_a_principal_axis_stays_a_steady_spin() {
        let inertia = Inertia([5_000., 30_000., 33_000.]);
        let mut basis = Basis::new(0., 0., 0.);
        let mut rates = [2., 0., 0.];
        for _ in 0..1200 {
            (basis, rates) = advance(basis, rates, inertia, Moments::default(), DT);
        }
        assert!((rates[0] - 2.).abs() < 1e-12 && rates[1].abs() < 1e-12);
        assert!(rates[2].abs() < 1e-12);
    }

    #[test]
    fn a_moment_accelerates_and_damping_settles_at_its_ratio() {
        let inertia = Inertia([1_000., 8_000., 9_000.]);
        let moments = Moments {
            applied: [0., 4_000., 0.],
            damping: [0., 16_000., 0.],
        };
        let mut basis = Basis::new(0., 0., 0.);
        let mut rates = [0.; 3];
        // The first tick's step is the implicit formula's.
        (basis, rates) = advance(basis, rates, inertia, moments, DT);
        let expected = (DT * 4_000. / 8_000.) / (1. + DT * 16_000. / 8_000.);
        assert!((rates[1] - expected).abs() < 1e-15);
        for _ in 0..1200 {
            (basis, rates) = advance(basis, rates, inertia, moments, DT);
        }
        assert!((rates[1] - 0.25).abs() < 1e-9, "{}", rates[1]);
        assert!(rates[0].abs() < 1e-12 && rates[2].abs() < 1e-12);
        // Stiff damping, two hundred times the inertia per second, stays
        // stable and monotonic at 120 Hz.
        let stiff = Moments {
            applied: [0.; 3],
            damping: [200_000., 0., 0.],
        };
        let mut rates = [3., 0., 0.];
        let mut previous = rates[0];
        for _ in 0..30 {
            (basis, rates) = advance(basis, rates, inertia, stiff, DT);
            assert!(rates[0] > 0. && rates[0] < previous, "{rates:?} {previous}");
            previous = rates[0];
        }
        assert!(rates[0] < 1e-11);
    }

    #[test]
    fn inertia_follows_the_weight_and_the_state_mirrors_its_rates() {
        let inertia = Inertia::from_weight(GRAVITY * 1_000., [3., 8., 9.]);
        assert_eq!(inertia, Inertia([9_000., 64_000., 81_000.]));
        let aircraft =
            crate::models::variety::tests::synthetic(tore_formats::aircraft::AircraftId::Ah64);
        let mut s = State::new(&aircraft, [0., 1_000., 0.]).unwrap();
        let before = Basis::new(s.yaw, s.pitch, s.bank);
        let moments = Moments {
            applied: [9_000., 0., 0.],
            ..Default::default()
        };
        let basis = s.advance_body(inertia, moments);
        assert!((s.lift_controls.body_rates[0] - DT).abs() < 1e-12);
        assert_eq!(s.roll_rate, s.lift_controls.body_rates[0]);
        assert_eq!(s.pitch_rate, s.lift_controls.body_rates[1]);
        assert!(basis.right[1] < before.right[1]);
    }
}
