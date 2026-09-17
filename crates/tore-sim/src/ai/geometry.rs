//! Target geometry and performance predicates from
//! [`docs/spec/ai.md`](../../../../docs/spec/ai.md), "The information an
//! aircraft uses" (B01 through B05).
//!
//! Frame: positions are feet in `[x, y, z]` where `y` is world vertical (up)
//! and `x`/`z` span the horizontal plane. Heading is degrees measured in the
//! horizontal plane with 0 along `+z` and 90 along `+x`, the same sense as the
//! host autopilot's `atan2(dx, dz)` bearing. Pitch is degrees above the
//! horizontal plane. Angle errors wrap into `-180..180`.

use super::{AiError, Result, ScalarSpeed, SpeedLimits};

/// Own aircraft pose as seen by the AI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OwnPose {
    /// Feet, `[x, y_vertical, z]`.
    pub position: [f64; 3],
    pub heading_deg: f64,
    pub pitch_deg: f64,
}

/// An observed target pose in the same frame as [`OwnPose`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetPose {
    /// Feet, `[x, y_vertical, z]`.
    pub position: [f64; 3],
    pub heading_deg: f64,
    pub pitch_deg: f64,
}

/// A known target: its pose may be absent (never observed, stale beyond use,
/// or withheld). Absent pose is explicit; geometry is never computed from a
/// placeholder position.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnownTarget {
    pub pose: Option<TargetPose>,
}

/// Angular relations between own aircraft and the target. These exist only
/// when the horizontal bearing is defined (nonzero horizontal distance).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelativeAngles {
    /// Own heading to target bearing, `-180..180`.
    pub heading_error_deg: f64,
    /// Own pitch to line-of-sight pitch, `-180..180`.
    pub pitch_error_deg: f64,
    /// B01: the larger absolute error of the two above.
    pub off_beam_deg: f64,
    /// B01: off-beam strictly less than 90 degrees.
    pub ahead: bool,
    /// B02: the target's absolute horizontal bearing error toward own
    /// aircraft is at most 90 degrees (heading only; its pitch is ignored).
    pub facing: bool,
    /// Absolute difference between the two headings, `0..=180` (B11/B12).
    pub heading_difference_deg: f64,
    /// Absolute difference between the two pitches, `0..=180` (B12).
    pub pitch_difference_deg: f64,
}

/// Geometry of an assigned target relative to own aircraft.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TargetGeometry {
    /// B03: spatial separation, feet.
    pub spatial_distance_feet: f64,
    /// B03: separation with the altitude component removed, feet.
    pub horizontal_distance_feet: f64,
    /// Angular relations; `None` when the two aircraft share a horizontal
    /// position, where the horizontal bearing has no value. The spec does not
    /// state the original's behavior there, so nothing is substituted.
    pub angles: Option<RelativeAngles>,
}

/// B01 ahead boundary, degrees (strict).
pub const AHEAD_OFF_BEAM_LIMIT_DEG: f64 = 90.0;
/// B02 facing boundary, degrees (inclusive).
pub const FACING_BEARING_LIMIT_DEG: f64 = 90.0;
/// B04: margin above minimum speed that permits climbing, source speed domain.
pub const CLIMB_SPEED_MARGIN: f64 = 75.0;
/// B04: maximum-speed advantage that counts as better speed, source domain.
pub const BETTER_SPEED_MARGIN: f64 = 75.0;
/// B05: own-minus-target evaluator difference that counts as better
/// thrust-to-weight. The evaluator's scale is unresolved; this is neither a
/// physical ratio nor a percentage.
pub const BETTER_THRUST_TO_WEIGHT_MARGIN: i32 = 10;

/// Wrap an angle in degrees into `-180..180`.
pub fn wrap_degrees(value: f64) -> f64 {
    (value + 180.0).rem_euclid(360.0) - 180.0
}

fn finite(values: &[f64]) -> bool {
    values.iter().all(|v| v.is_finite())
}

/// Compute the target's geometry (B01 through B03).
///
/// Errors: an absent target pose or any non-finite input is
/// [`AiError::InvalidInput`].
pub fn target_geometry(own: &OwnPose, target: &KnownTarget) -> Result<TargetGeometry> {
    let target = target
        .pose
        .ok_or(AiError::InvalidInput("target pose unknown"))?;
    if !finite(&own.position)
        || !finite(&[own.heading_deg, own.pitch_deg])
        || !finite(&target.position)
        || !finite(&[target.heading_deg, target.pitch_deg])
    {
        return Err(AiError::InvalidInput("non-finite pose"));
    }

    let dx = target.position[0] - own.position[0];
    let dy = target.position[1] - own.position[1];
    let dz = target.position[2] - own.position[2];
    let horizontal_distance_feet = dx.hypot(dz);
    let spatial_distance_feet = horizontal_distance_feet.hypot(dy);

    let angles = (horizontal_distance_feet > 0.0).then(|| {
        let bearing_deg = dx.atan2(dz).to_degrees();
        let line_of_sight_pitch_deg = dy.atan2(horizontal_distance_feet).to_degrees();
        let heading_error_deg = wrap_degrees(bearing_deg - own.heading_deg);
        let pitch_error_deg = wrap_degrees(line_of_sight_pitch_deg - own.pitch_deg);
        let off_beam_deg = heading_error_deg.abs().max(pitch_error_deg.abs());
        let reverse_bearing_deg = wrap_degrees(bearing_deg + 180.0);
        let target_bearing_error_deg = wrap_degrees(reverse_bearing_deg - target.heading_deg);
        RelativeAngles {
            heading_error_deg,
            pitch_error_deg,
            off_beam_deg,
            ahead: off_beam_deg < AHEAD_OFF_BEAM_LIMIT_DEG,
            facing: target_bearing_error_deg.abs() <= FACING_BEARING_LIMIT_DEG,
            heading_difference_deg: wrap_degrees(target.heading_deg - own.heading_deg).abs(),
            pitch_difference_deg: wrap_degrees(target.pitch_deg - own.pitch_deg).abs(),
        }
    });

    Ok(TargetGeometry {
        spatial_distance_feet,
        horizontal_distance_feet,
        angles,
    })
}

/// B04: climbing is permitted when current speed is at least the current
/// minimum plus [`CLIMB_SPEED_MARGIN`].
pub fn can_climb(current: ScalarSpeed, limits: &SpeedLimits) -> bool {
    current >= limits.minimum.plus(CLIMB_SPEED_MARGIN)
}

/// B04: own maximum speed exceeds the target's by at least
/// [`BETTER_SPEED_MARGIN`].
pub fn better_speed(own_max: ScalarSpeed, target_max: ScalarSpeed) -> bool {
    own_max.0 - target_max.0 >= BETTER_SPEED_MARGIN
}

/// B05: own-minus-target performance evaluator is at least
/// [`BETTER_THRUST_TO_WEIGHT_MARGIN`]. Both values are in the recovered
/// evaluator scale, which research has not tied to a physical quantity.
pub fn better_thrust_to_weight(own_eval: i32, target_eval: i32) -> bool {
    own_eval - target_eval >= BETTER_THRUST_TO_WEIGHT_MARGIN
}

#[cfg(test)]
mod tests {
    use super::*;

    const RANGE: f64 = 10_000.0;

    fn own(heading_deg: f64, pitch_deg: f64) -> OwnPose {
        OwnPose {
            position: [0.0, 0.0, 0.0],
            heading_deg,
            pitch_deg,
        }
    }

    /// Target dead ahead on the `+z` axis at `RANGE` feet, level, with the
    /// given heading.
    fn target_on_z(heading_deg: f64) -> KnownTarget {
        KnownTarget {
            pose: Some(TargetPose {
                position: [0.0, 0.0, RANGE],
                heading_deg,
                pitch_deg: 0.0,
            }),
        }
    }

    fn angles(own: &OwnPose, target: &KnownTarget) -> RelativeAngles {
        target_geometry(own, target).unwrap().angles.unwrap()
    }

    #[test]
    fn b01_ahead_is_strict_on_heading_error() {
        for (offset, ahead) in [(89.0, true), (90.0, false), (91.0, false)] {
            let a = angles(&own(-offset, 0.0), &target_on_z(180.0));
            assert_eq!(a.heading_error_deg, offset);
            assert_eq!(a.pitch_error_deg, 0.0);
            assert_eq!(a.off_beam_deg, offset);
            assert_eq!(a.ahead, ahead, "heading offset {offset}");
            let a = angles(&own(offset, 0.0), &target_on_z(180.0));
            assert_eq!(a.heading_error_deg, -offset);
            assert_eq!(a.ahead, ahead, "heading offset -{offset}");
        }
    }

    #[test]
    fn b01_ahead_is_strict_on_pitch_error() {
        for (offset, ahead) in [(89.0, true), (90.0, false), (91.0, false)] {
            let a = angles(&own(0.0, -offset), &target_on_z(180.0));
            assert_eq!(a.heading_error_deg, 0.0);
            assert_eq!(a.pitch_error_deg, offset);
            assert_eq!(a.off_beam_deg, offset);
            assert_eq!(a.ahead, ahead, "pitch offset {offset}");
        }
    }

    #[test]
    fn b01_off_beam_takes_the_larger_error() {
        let a = angles(&own(-30.0, -89.0), &target_on_z(180.0));
        assert_eq!(a.off_beam_deg, 89.0);
        assert!(a.ahead);
        let a = angles(&own(-90.0, -30.0), &target_on_z(180.0));
        assert_eq!(a.off_beam_deg, 90.0);
        assert!(!a.ahead);
    }

    #[test]
    fn b02_facing_is_inclusive_and_uses_heading_only() {
        // The target's bearing toward own aircraft is 180 degrees.
        for (target_heading, facing) in [(91.0, true), (90.0, true), (89.0, false)] {
            let a = angles(&own(0.0, 0.0), &target_on_z(target_heading));
            assert_eq!(a.facing, facing, "target heading {target_heading}");
            let a = angles(&own(0.0, 0.0), &target_on_z(360.0 - target_heading));
            assert_eq!(
                a.facing,
                facing,
                "target heading {}",
                360.0 - target_heading
            );
        }
        // A steep target pitch does not affect facing.
        let steep = KnownTarget {
            pose: Some(TargetPose {
                position: [0.0, 0.0, RANGE],
                heading_deg: 90.0,
                pitch_deg: 89.0,
            }),
        };
        assert!(angles(&own(0.0, 0.0), &steep).facing);
    }

    #[test]
    fn b03_horizontal_distance_removes_altitude() {
        let own = OwnPose {
            position: [1200.0, 5000.0, -400.0],
            heading_deg: 45.0,
            pitch_deg: 0.0,
        };
        let target = KnownTarget {
            pose: Some(TargetPose {
                position: [1200.0, 8000.0, -400.0],
                heading_deg: 45.0,
                pitch_deg: 0.0,
            }),
        };
        let g = target_geometry(&own, &target).unwrap();
        assert_eq!(g.horizontal_distance_feet, 0.0);
        assert_eq!(g.spatial_distance_feet, 3000.0);
        assert_eq!(g.angles, None);

        let offset = KnownTarget {
            pose: Some(TargetPose {
                position: [1200.0 + 3000.0, 5000.0 + 4000.0, -400.0],
                heading_deg: 0.0,
                pitch_deg: 0.0,
            }),
        };
        let g = target_geometry(&own, &offset).unwrap();
        assert_eq!(g.horizontal_distance_feet, 3000.0);
        assert_eq!(g.spatial_distance_feet, 5000.0);
    }

    #[test]
    fn heading_and_pitch_differences_wrap() {
        let target = KnownTarget {
            pose: Some(TargetPose {
                position: [0.0, 0.0, RANGE],
                heading_deg: 350.0,
                pitch_deg: -20.0,
            }),
        };
        let a = angles(&own(10.0, 10.0), &target);
        assert_eq!(a.heading_difference_deg, 20.0);
        assert_eq!(a.pitch_difference_deg, 30.0);
        let a = angles(&own(175.0, 0.0), &target_on_z(-175.0));
        assert_eq!(a.heading_difference_deg, 10.0);
    }

    #[test]
    fn unknown_or_invalid_poses_are_rejected() {
        let unknown = KnownTarget { pose: None };
        assert_eq!(
            target_geometry(&own(0.0, 0.0), &unknown),
            Err(AiError::InvalidInput("target pose unknown"))
        );
        let nan = KnownTarget {
            pose: Some(TargetPose {
                position: [f64::NAN, 0.0, RANGE],
                heading_deg: 0.0,
                pitch_deg: 0.0,
            }),
        };
        assert!(matches!(
            target_geometry(&own(0.0, 0.0), &nan),
            Err(AiError::InvalidInput(_))
        ));
    }

    #[test]
    fn b04_climb_boundary_is_minimum_plus_75() {
        let limits = SpeedLimits {
            minimum: ScalarSpeed(200.0),
            maximum: ScalarSpeed(900.0),
            corner: ScalarSpeed(450.0),
        };
        assert!(!can_climb(ScalarSpeed(274.0), &limits));
        assert!(can_climb(ScalarSpeed(275.0), &limits));
        assert!(can_climb(ScalarSpeed(276.0), &limits));
    }

    #[test]
    fn b04_better_speed_boundary_is_75() {
        let target = ScalarSpeed(800.0);
        assert!(!better_speed(ScalarSpeed(874.0), target));
        assert!(better_speed(ScalarSpeed(875.0), target));
        assert!(better_speed(ScalarSpeed(876.0), target));
    }

    #[test]
    fn b05_thrust_to_weight_boundary_is_10() {
        assert!(!better_thrust_to_weight(59, 50));
        assert!(better_thrust_to_weight(60, 50));
        assert!(better_thrust_to_weight(61, 50));
    }
}
