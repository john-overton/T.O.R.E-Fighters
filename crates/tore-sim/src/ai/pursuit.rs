//! B15 pursuit reference frame and speed regulation, plus the B44 lead gates.
//!
//! Spec: `docs/spec/ai.md`, sections B15 ("Pursuit reference frame and speed
//! regulation") and B44 (the "weapon-dependent lead" paragraph). Everything in
//! this file is spec-derived except the single axis convention named by
//! [`OFFSET_AXIS_CONVENTION`], which is fitted.
//!
//! Positions are world feet as `[x, y, z]` with `y` up, matching the host's
//! `attitude::Basis` (forward at heading 0 is `+z`, right is `+x`, heading
//! increases clockwise seen from above).

use super::{AiError, Result, ScalarSpeed, SpeedLimits};

/// World position in feet, `[x, y, z]`, `y` up.
pub type Position = [f64; 3];

/// B15: the three `homepos` offset operands, distances in feet, expressed in
/// the target's heading-only frame. The horizontal pair rotates with target
/// heading; `vertical_feet` stays world-vertical and ignores target pitch and
/// bank.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PursuitOffset {
    pub longitudinal_feet: f64,
    pub lateral_feet: f64,
    pub vertical_feet: f64,
}

/// The one place the lateral/longitudinal sign convention lives. B15 records
/// the signs as unresolved, so this is a **fitted** choice: lateral positive
/// is the target's right, longitudinal positive is ahead of the target.
/// Research that recovers the original signs changes [`target_frame_axes`]
/// and nothing else.
pub const OFFSET_AXIS_CONVENTION: &str =
    "fitted: lateral positive = target's right, longitudinal positive = ahead of target";

/// Horizontal unit axes `(ahead, right)` of a target flying `heading_deg`,
/// under [`OFFSET_AXIS_CONVENTION`] and the host world axes.
pub fn target_frame_axes(heading_deg: f64) -> (Position, Position) {
    let (sin, cos) = heading_deg.to_radians().sin_cos();
    let ahead = [sin, 0., cos];
    let right = [cos, 0., -sin];
    (ahead, right)
}

/// B15: the offset steering point around a target. Only target heading rotates
/// the horizontal pair; the vertical offset is added on the world up axis.
pub fn steering_point(
    target_position: Position,
    target_heading_deg: f64,
    offset: PursuitOffset,
) -> Position {
    let (ahead, right) = target_frame_axes(target_heading_deg);
    let horizontal = std::array::from_fn(|i| {
        target_position[i] + ahead[i] * offset.longitudinal_feet + right[i] * offset.lateral_feet
    });
    lifted(horizontal, offset.vertical_feet)
}

/// `point` moved `feet` along the world up axis.
fn lifted(mut point: Position, feet: f64) -> Position {
    point[1] += feet;
    point
}

/// B15 speed-regulation table on `e = actual - desired` separation, in feet,
/// before the corner-speed override and own limits. `v` is the target's
/// scalar speed, negative already treated as zero.
fn regulated_request(
    separation_error_feet: f64,
    v: ScalarSpeed,
    limits: &SpeedLimits,
) -> ScalarSpeed {
    let e = separation_error_feet;
    if e >= 5000. {
        limits.maximum
    } else if e > 1000. {
        v.plus(100.)
    } else if e > 250. {
        v.plus(50.)
    } else if e >= 50. {
        v.plus(25.)
    } else if e >= -50. {
        v
    } else if e >= -100. {
        v.plus(-25.)
    } else {
        v.plus(-50.)
    }
}

/// B15: requested scalar speed while regulating separation to a target.
///
/// - `desired_separation_feet` is the magnitude of the negative `homepos`
///   speed operand (`-750` requests 750).
/// - `actual_spatial_separation_feet` is measured to the target's unoffset
///   position, not to the steering point.
/// - Negative `target_speed` is treated as zero.
/// - When either `heading_error_deg` or `pitch_error_deg` (to the target
///   position, sign irrelevant) exceeds 90 degrees, the request is corner
///   speed. The spec attaches the own maximum/minimum queries to the ordinary
///   regulating branch only, so the corner request is returned unlimited.
/// - `minimum_exempt` is the separate aircraft-state exemption from the
///   minimum. Its producer is unresolved, so it is always an explicit input.
pub fn regulate_speed(
    desired_separation_feet: f64,
    actual_spatial_separation_feet: f64,
    target_speed: ScalarSpeed,
    heading_error_deg: f64,
    pitch_error_deg: f64,
    limits: &SpeedLimits,
    minimum_exempt: bool,
) -> ScalarSpeed {
    if heading_error_deg.abs() > 90. || pitch_error_deg.abs() > 90. {
        return limits.corner;
    }
    let v = target_speed.max(ScalarSpeed(0.));
    let e = actual_spatial_separation_feet - desired_separation_feet;
    let request = regulated_request(e, v, limits).min(limits.maximum);
    if minimum_exempt {
        request
    } else {
        request.max(limits.minimum)
    }
}

/// B44: an aircraft attitude as the lead reduction sees it, in degrees.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Attitude {
    pub heading_deg: f64,
    pub pitch_deg: f64,
}

/// B44: what is launching. Only an aircraft launcher has the 1600-foot
/// attitude-difference reduction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Launcher {
    Aircraft(Attitude),
    NonAircraft,
}

/// B44: what is being aimed at. A non-aircraft aim point gets the 20-foot
/// upward offset; an aircraft target supplies the attitude the reduction
/// compares against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AimTarget {
    Aircraft(Attitude),
    NonAircraft,
}

/// B44 lead distance gates, feet.
pub const LEAD_BYPASS_DISTANCE_FEET: f64 = 20000.;
pub const LEAD_REDUCTION_DISTANCE_FEET: f64 = 1600.;
/// B44 attitude-difference reduction, degrees: zero at or below the low
/// bound, full at or above the high bound, linear between.
pub const LEAD_REDUCTION_LOW_DEG: f64 = 10.;
pub const LEAD_REDUCTION_HIGH_DEG: f64 = 35.;
/// B44: upward offset applied to a non-aircraft aim point when not bypassed.
pub const NON_AIRCRAFT_AIM_LIFT_FEET: f64 = 20.;

/// Result of [`lead_point`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lead {
    /// The point B15 steers toward before the pursuit offset is applied.
    pub aim_point: Position,
    /// Fraction of `predicted_travel` that was applied: 0 on the long-range
    /// bypass, otherwise the reduction scale.
    pub prediction_scale: f64,
}

/// B44 reduction scale from the larger absolute heading or pitch difference
/// between two aircraft attitudes. Heading difference is wrapped to the
/// shortest arc; pitch is compared directly.
pub fn lead_reduction_scale(launcher: Attitude, target: Attitude) -> f64 {
    let heading = wrap_degrees(launcher.heading_deg - target.heading_deg).abs();
    let pitch = (launcher.pitch_deg - target.pitch_deg).abs();
    let difference = heading.max(pitch);
    ((difference - LEAD_REDUCTION_LOW_DEG) / (LEAD_REDUCTION_HIGH_DEG - LEAD_REDUCTION_LOW_DEG))
        .clamp(0., 1.)
}

fn wrap_degrees(angle: f64) -> f64 {
    (angle + 180.).rem_euclid(360.) - 180.
}

/// B44: the predictive-weapon lead applied before B15's pursuit offset.
///
/// `predicted_travel` is the target displacement the weapon's speed estimator
/// and prediction time would produce. Both are unresolved, so the caller
/// supplies it; `None` inside the prediction range means the caller asked
/// this module to compute the estimator, which is an unspecified rule.
///
/// - Spatial distance of 20000 feet or more bypasses prediction and the
///   20-foot non-aircraft lift; the aim point is the target position.
/// - An aircraft launcher within 1600 feet of an aircraft target scales the
///   displacement by [`lead_reduction_scale`].
/// - An aircraft launcher within 1600 feet of a non-aircraft target has no
///   recovered rule (the spec compares "the two aircraft"); unspecified.
pub fn lead_point(
    launcher_position: Position,
    launcher: Launcher,
    target_position: Position,
    target: AimTarget,
    predicted_travel: Option<Position>,
) -> Result<Lead> {
    let distance = distance_between(launcher_position, target_position);
    if !distance.is_finite() {
        return Err(AiError::InvalidInput("non-finite lead geometry"));
    }
    if distance >= LEAD_BYPASS_DISTANCE_FEET {
        return Ok(Lead {
            aim_point: target_position,
            prediction_scale: 0.,
        });
    }
    let Some(travel) = predicted_travel else {
        return Err(AiError::UnspecifiedRule(
            "B44 lead speed estimator and prediction time",
        ));
    };
    let prediction_scale = match (launcher, target) {
        (Launcher::Aircraft(own), AimTarget::Aircraft(theirs))
            if distance < LEAD_REDUCTION_DISTANCE_FEET =>
        {
            lead_reduction_scale(own, theirs)
        }
        (Launcher::Aircraft(_), AimTarget::NonAircraft)
            if distance < LEAD_REDUCTION_DISTANCE_FEET =>
        {
            return Err(AiError::UnspecifiedRule(
                "B44 aircraft launcher within 1600 feet of a non-aircraft target",
            ));
        }
        _ => 1.,
    };
    let lift = match target {
        AimTarget::Aircraft(_) => 0.,
        AimTarget::NonAircraft => NON_AIRCRAFT_AIM_LIFT_FEET,
    };
    let aim_point: Position =
        std::array::from_fn(|i| target_position[i] + travel[i] * prediction_scale);
    Ok(Lead {
        aim_point: lifted(aim_point, lift),
        prediction_scale,
    })
}

fn distance_between(a: Position, b: Position) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIMITS: SpeedLimits = SpeedLimits {
        minimum: ScalarSpeed(200.),
        maximum: ScalarSpeed(900.),
        corner: ScalarSpeed(450.),
    };

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn regulate(error: f64) -> ScalarSpeed {
        regulate_speed(
            750.,
            750. + error,
            ScalarSpeed(500.),
            0.,
            0.,
            &LIMITS,
            false,
        )
    }

    #[test]
    fn steering_point_rotates_horizontal_pair_with_heading_only() {
        let offset = PursuitOffset {
            longitudinal_feet: 100.,
            lateral_feet: 10.,
            vertical_feet: 5.,
        };
        // Heading 0: ahead is +z, right is +x.
        let p = steering_point([0., 0., 0.], 0., offset);
        assert!(
            close(p[0], 10.) && close(p[1], 5.) && close(p[2], 100.),
            "{p:?}"
        );
        // Heading 90: ahead is +x, right is -z.
        let p = steering_point([1., 2., 3.], 90., offset);
        assert!(
            close(p[0], 101.) && close(p[1], 7.) && close(p[2], -7.),
            "{p:?}"
        );
        // Heading 180: ahead is -z, right is -x.
        let p = steering_point([0., 0., 0.], 180., offset);
        assert!(
            close(p[0], -10.) && close(p[1], 5.) && close(p[2], -100.),
            "{p:?}"
        );
    }

    #[test]
    fn vertical_offset_ignores_heading() {
        let offset = PursuitOffset {
            vertical_feet: -300.,
            ..Default::default()
        };
        for heading in [0., 45., 135., 270., 359.] {
            let p = steering_point([10., 20., 30.], heading, offset);
            assert!(
                close(p[0], 10.) && close(p[1], -280.) && close(p[2], 30.),
                "{p:?}"
            );
        }
    }

    #[test]
    fn separation_bands_each_side_of_every_boundary() {
        // e >= 5000: own maximum.
        assert_eq!(regulate(5000.), ScalarSpeed(900.));
        assert_eq!(regulate(4999.), ScalarSpeed(600.));
        // 1000 < e < 5000: v + 100; e == 1000 belongs to v + 50.
        assert_eq!(regulate(1001.), ScalarSpeed(600.));
        assert_eq!(regulate(1000.), ScalarSpeed(550.));
        // 250 < e <= 1000: v + 50; e == 250 belongs to v + 25.
        assert_eq!(regulate(251.), ScalarSpeed(550.));
        assert_eq!(regulate(250.), ScalarSpeed(525.));
        // 50 <= e <= 250: v + 25; e == 50 is inclusive here.
        assert_eq!(regulate(50.), ScalarSpeed(525.));
        assert_eq!(regulate(49.), ScalarSpeed(500.));
        // -50 <= e < 50: v.
        assert_eq!(regulate(-50.), ScalarSpeed(500.));
        assert_eq!(regulate(-51.), ScalarSpeed(475.));
        // -100 <= e < -50: v - 25; below -100: v - 50.
        assert_eq!(regulate(-100.), ScalarSpeed(475.));
        assert_eq!(regulate(-101.), ScalarSpeed(450.));
    }

    #[test]
    fn error_is_measured_from_the_desired_separation() {
        // Desired 2000, actual 2049: e = 49, band v.
        let s = regulate_speed(2000., 2049., ScalarSpeed(300.), 0., 0., &LIMITS, false);
        assert_eq!(s, ScalarSpeed(300.));
        // Desired 2000, actual 2050: e = 50, band v + 25.
        let s = regulate_speed(2000., 2050., ScalarSpeed(300.), 0., 0., &LIMITS, false);
        assert_eq!(s, ScalarSpeed(325.));
    }

    #[test]
    fn off_beam_rejects_to_corner_speed() {
        let s = |h, p| regulate_speed(750., 3000., ScalarSpeed(500.), h, p, &LIMITS, false);
        assert_eq!(s(90., 0.), ScalarSpeed(600.));
        assert_eq!(s(0., 90.), ScalarSpeed(600.));
        assert_eq!(s(91., 0.), ScalarSpeed(450.));
        assert_eq!(s(-91., 0.), ScalarSpeed(450.));
        assert_eq!(s(0., 91.), ScalarSpeed(450.));
        assert_eq!(s(0., -91.), ScalarSpeed(450.));
    }

    #[test]
    fn own_limits_bound_the_ordinary_request() {
        // v + 100 = 950 exceeds the 900 maximum.
        let s = regulate_speed(750., 3000., ScalarSpeed(850.), 0., 0., &LIMITS, false);
        assert_eq!(s, ScalarSpeed(900.));
        // v - 50 = 150 falls below the 200 minimum unless exempt.
        let slow = |exempt| regulate_speed(750., 500., ScalarSpeed(200.), 0., 0., &LIMITS, exempt);
        assert_eq!(slow(false), ScalarSpeed(200.));
        assert_eq!(slow(true), ScalarSpeed(150.));
    }

    #[test]
    fn negative_target_speed_is_zero() {
        let s = regulate_speed(750., 1500., ScalarSpeed(-400.), 0., 0., &LIMITS, true);
        assert_eq!(s, ScalarSpeed(50.));
        let s = regulate_speed(750., 750., ScalarSpeed(-400.), 0., 0., &LIMITS, true);
        assert_eq!(s, ScalarSpeed(0.));
    }

    fn attitude(heading_deg: f64, pitch_deg: f64) -> Attitude {
        Attitude {
            heading_deg,
            pitch_deg,
        }
    }

    #[test]
    fn lead_reduction_scale_boundaries() {
        let own = attitude(0., 0.);
        assert!(close(lead_reduction_scale(own, attitude(10., 0.)), 0.));
        assert!(close(lead_reduction_scale(own, attitude(5., 0.)), 0.));
        assert!(close(lead_reduction_scale(own, attitude(22.5, 0.)), 0.5));
        assert!(close(lead_reduction_scale(own, attitude(0., -22.5)), 0.5));
        assert!(close(lead_reduction_scale(own, attitude(35., 0.)), 1.));
        assert!(close(lead_reduction_scale(own, attitude(0., 80.)), 1.));
        // The larger of the two differences governs.
        assert!(close(lead_reduction_scale(own, attitude(15., 35.)), 1.));
        // Heading difference uses the shortest arc.
        assert!(close(
            lead_reduction_scale(attitude(355., 0.), attitude(5., 0.)),
            0.
        ));
        assert!(close(
            lead_reduction_scale(attitude(350., 0.), attitude(12.5, 0.)),
            0.5
        ));
    }

    fn lead_at(distance: f64, launcher: Launcher, target: AimTarget) -> Result<Lead> {
        lead_point(
            [0., 0., 0.],
            launcher,
            [0., 0., distance],
            target,
            Some([100., 0., 0.]),
        )
    }

    #[test]
    fn lead_reduction_applies_only_inside_1600_feet() {
        let own = Launcher::Aircraft(attitude(0., 0.));
        let level = AimTarget::Aircraft(attitude(0., 0.));
        let inside = lead_at(1599., own, level).unwrap();
        assert!(close(inside.prediction_scale, 0.));
        assert!(close(inside.aim_point[0], 0.));
        let outside = lead_at(1600., own, level).unwrap();
        assert!(close(outside.prediction_scale, 1.));
        assert!(close(outside.aim_point[0], 100.));
        // Inside 1600 feet with a 22.5 degree difference: half the travel.
        let half = lead_at(1599., own, AimTarget::Aircraft(attitude(22.5, 0.))).unwrap();
        assert!(close(half.prediction_scale, 0.5));
        assert!(close(half.aim_point[0], 50.));
        // A non-aircraft launcher is never reduced.
        let gun = lead_at(100., Launcher::NonAircraft, level).unwrap();
        assert!(close(gun.prediction_scale, 1.));
    }

    #[test]
    fn long_range_bypasses_prediction_and_lift() {
        let own = Launcher::Aircraft(attitude(0., 0.));
        let near = lead_at(19999., own, AimTarget::NonAircraft).unwrap();
        assert!(close(near.prediction_scale, 1.));
        assert!(close(near.aim_point[0], 100.) && close(near.aim_point[1], 20.));
        let far = lead_at(20000., own, AimTarget::NonAircraft).unwrap();
        assert!(close(far.prediction_scale, 0.));
        assert_eq!(far.aim_point, [0., 0., 20000.]);
        // Beyond the bypass no estimator is needed at all.
        let far = lead_point(
            [0., 0., 0.],
            own,
            [0., 0., 25000.],
            AimTarget::NonAircraft,
            None,
        );
        assert_eq!(far.unwrap().aim_point, [0., 0., 25000.]);
    }

    #[test]
    fn non_aircraft_lift_only_for_non_aircraft_targets() {
        let own = Launcher::Aircraft(attitude(0., 0.));
        let aircraft = lead_at(5000., own, AimTarget::Aircraft(attitude(0., 0.))).unwrap();
        assert!(close(aircraft.aim_point[1], 0.));
        let ground = lead_at(5000., own, AimTarget::NonAircraft).unwrap();
        assert!(close(ground.aim_point[1], 20.));
    }

    #[test]
    fn unspecified_estimator_and_branches_are_errors() {
        let own = Launcher::Aircraft(attitude(0., 0.));
        let r = lead_point(
            [0., 0., 0.],
            own,
            [0., 0., 5000.],
            AimTarget::NonAircraft,
            None,
        );
        assert!(matches!(r, Err(AiError::UnspecifiedRule(_))));
        let r = lead_at(1000., own, AimTarget::NonAircraft);
        assert!(matches!(r, Err(AiError::UnspecifiedRule(_))));
        let r = lead_point(
            [f64::NAN, 0., 0.],
            own,
            [0.; 3],
            AimTarget::NonAircraft,
            None,
        );
        assert!(matches!(r, Err(AiError::InvalidInput(_))));
    }
}
