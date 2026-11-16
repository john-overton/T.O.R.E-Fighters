//! A gun row that points away from the nose (B52-6, 2026-11-15, fitted).
//!
//! An aircraft record's gun row carries `slewH`, `slewP`, `slewLimitH` and
//! `slewLimitP`: where the mount points at rest and the half-arc either side
//! of it, 182 units a degree. The B-52's M61 reads heading 32,760 (180
//! degrees) and 30 degree arcs, the Tu-26's and the Tu-95's `T23_1.JT`
//! turrets the same heading. Nothing read these before: every aircraft gun
//! fired along the nose, the B-52's tail gun included.
//!
//! The rule, in one place so every reader agrees:
//!
//! - A mount whose heading is zero is a **forward gun** and stays exactly as
//!   it was: the nose line, whatever its pitch word says (the helicopters'
//!   chain guns read a 20 degree depression that has never been applied, and
//!   is not applied now).
//! - A mount with a heading is **slewed**. Its fire direction is the
//!   mount's rest direction in the aircraft's frame: heading clockwise from
//!   the nose seen from above, pitch up, rotated with the aircraft.
//! - The arcs are what an AI gunner may swing the barrel through
//!   ([`Slew::contains`]); a human fires along the rest direction.
//!
//! Provenance: heading, pitch and arcs spec-derived (retail records); the
//! forward/slewed split, the centre-line fire for a human and the AI's swing
//! inside the arcs are fitted (agent decisions, 2026-11-15).
use crate::attitude::{Basis, Vector, dot, unit};
use std::f64::consts::PI;

/// Angle units a degree in the record words (the surface units' scale).
const UNITS_PER_DEGREE: f64 = 182.;

/// Where a gun mount points at rest and how far it may swing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slew {
    /// `[heading, pitch]` at rest, radians: heading clockwise from the nose
    /// (positive right), pitch up.
    pub rest: [f64; 2],
    /// The half-arc either side of `rest`, `[heading, pitch]` radians.
    pub limit: [f64; 2],
}

impl Slew {
    /// The slew of a record row's words (`slewH`, `slewP`, `slewLimitH`,
    /// `slewLimitP`). A row with no heading is a forward mount: all zero, so
    /// a forward gun carries exactly what it carried before.
    pub fn from_record(words: [i32; 4]) -> Self {
        if words[0] == 0 {
            return Self::default();
        }
        let angle = |w: i32| f64::from(w) / UNITS_PER_DEGREE * PI / 180.;
        Self {
            rest: [angle(words[0]), angle(words[1])],
            limit: [angle(words[2]).abs(), angle(words[3]).abs()],
        }
    }

    /// Whether the mount points away from the nose.
    pub fn is_slewed(&self) -> bool {
        self.rest[0] != 0.
    }

    /// The mount's own frame on an aircraft whose frame is `body`: the body
    /// turned by the heading about its up axis, then by the pitch about the
    /// turned right axis. A forward mount is `body` itself, bit for bit.
    pub fn basis(&self, body: Basis) -> Basis {
        if !self.is_slewed() {
            return body;
        }
        let (sh, ch) = self.rest[0].sin_cos();
        let forward: Vector = std::array::from_fn(|i| body.forward[i] * ch + body.right[i] * sh);
        let right: Vector = std::array::from_fn(|i| body.right[i] * ch - body.forward[i] * sh);
        let (sp, cp) = self.rest[1].sin_cos();
        Basis {
            right,
            up: std::array::from_fn(|i| body.up[i] * cp - forward[i] * sp),
            forward: std::array::from_fn(|i| forward[i] * cp + body.up[i] * sp),
        }
    }

    /// Whether `delta`, a vector from the muzzle in the aircraft's frame
    /// `body`, lies inside the arcs: its heading and pitch from the rest
    /// direction each within the half-arc.
    pub fn contains(&self, body: Basis, delta: Vector) -> bool {
        let mount = self.basis(body);
        let forward = dot(delta, mount.forward);
        let right = dot(delta, mount.right);
        let up = dot(delta, mount.up);
        if forward <= 0. {
            return false;
        }
        right.atan2(forward).abs() <= self.limit[0]
            && up.atan2(forward.hypot(right)).abs() <= self.limit[1]
    }

    /// The mount's rest direction in the world for an aircraft in `body`.
    pub fn direction(&self, body: Basis) -> Vector {
        self.basis(body).forward
    }

    /// `mount`, an offset in the aircraft's frame `body` given by its
    /// `[right, up, forward]` components, expressed in the mount's own
    /// frame, so a sight that places the muzzle with the mount's basis
    /// finds the same point.
    pub fn mount_in_frame(&self, body: Basis, mount: Vector) -> Vector {
        if !self.is_slewed() {
            return mount;
        }
        let frame = self.basis(body);
        let world: Vector = std::array::from_fn(|i| {
            body.right[i] * mount[0] + body.up[i] * mount[1] + body.forward[i] * mount[2]
        });
        [
            dot(world, unit(frame.right)),
            dot(world, unit(frame.up)),
            dot(world, unit(frame.forward)),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aft() -> Slew {
        Slew::from_record([32_760, 0, 5_460, 5_460])
    }

    #[test]
    fn a_record_heading_of_32760_is_180_degrees_with_30_degree_arcs() {
        let slew = aft();
        assert!((slew.rest[0] - PI).abs() < 1e-12);
        assert!(slew.rest[1].abs() < 1e-12);
        assert!((slew.limit[0] - 30_f64.to_radians()).abs() < 1e-12);
        assert!(slew.is_slewed());
    }

    #[test]
    fn a_forward_mount_is_the_nose_whatever_its_pitch_word_says() {
        // The helicopters' chain guns read a 20 degree depression.
        let slew = Slew::from_record([0, -3_640, 8_190, 8_190]);
        assert_eq!(slew, Slew::default());
        let body = Basis::new(0.7, 0.2, 0.1);
        assert_eq!(slew.basis(body), body);
        assert_eq!(slew.direction(body), body.forward);
        assert_eq!(slew.mount_in_frame(body, [1., 2., 3.]), [1., 2., 3.]);
    }

    #[test]
    fn an_aft_mount_fires_back_along_the_tail_and_turns_with_the_aircraft() {
        for (yaw, pitch, bank) in [(0., 0., 0.), (1.1, 0.3, 0.4), (-2.5, -0.2, 1.2)] {
            let body = Basis::new(yaw, pitch, bank);
            let direction = aft().direction(body);
            for i in 0..3 {
                assert!(
                    (direction[i] + body.forward[i]).abs() < 1e-9,
                    "{direction:?}"
                );
            }
            let mount = aft().basis(body);
            assert!((dot(mount.right, mount.forward)).abs() < 1e-9);
            assert!((dot(mount.up, mount.forward)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_turret_that_points_up_and_back_tilts_with_its_pitch_word() {
        // The Tu-95's dorsal turret: heading 180, pitch 20 up.
        let slew = Slew::from_record([32_760, 3_640, 8_190, 910]);
        let body = Basis::new(0., 0., 0.);
        let direction = slew.direction(body);
        assert!(direction[2] < -0.9 && direction[1] > 0.3, "{direction:?}");
    }

    #[test]
    fn the_arcs_bound_the_swing_either_side_of_the_rest_direction() {
        let slew = aft();
        let body = Basis::new(0., 0., 0.);
        // Dead astern, 20 degrees off to either side, 40 degrees off, ahead.
        assert!(slew.contains(body, [0., 0., -1.]));
        for degrees in [20_f64, -20.] {
            let a = degrees.to_radians();
            assert!(slew.contains(body, [a.sin(), 0., -a.cos()]));
        }
        let a = 40_f64.to_radians();
        assert!(!slew.contains(body, [a.sin(), 0., -a.cos()]));
        assert!(!slew.contains(body, [0., a.sin(), -a.cos()]));
        assert!(!slew.contains(body, [0., 0., 1.]));
    }

    #[test]
    fn the_mount_offset_in_the_mounts_frame_places_the_muzzle_at_the_same_point() {
        let slew = aft();
        let body = Basis::new(0.8, 0.1, -0.3);
        let mount = [0.5, 2., -92.];
        let direct: Vector = std::array::from_fn(|i| {
            body.right[i] * mount[0] + body.up[i] * mount[1] + body.forward[i] * mount[2]
        });
        let frame = slew.basis(body);
        let local = slew.mount_in_frame(body, mount);
        let placed: Vector = std::array::from_fn(|i| {
            frame.right[i] * local[0] + frame.up[i] * local[1] + frame.forward[i] * local[2]
        });
        for i in 0..3 {
            assert!((direct[i] - placed[i]).abs() < 1e-9);
        }
    }
}
