//! Opinionated runway-wind limits and fitted wheel-contact coupling.
use crate::attitude::Vector;

pub const FEET_PER_SECOND_PER_KNOT: f64 = 1.68781;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Limits {
    pub noticeable_knots: f64,
    pub rough_knots: f64,
    pub limit_knots: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    BelowNoticeable,
    Noticeable,
    Rough,
    Limit,
}
impl Severity {
    pub fn label(self) -> &'static str {
        match self {
            Self::BelowNoticeable => "CALM",
            Self::Noticeable => "NOTICE",
            Self::Rough => "ROUGH",
            Self::Limit => "LIMIT",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Assessment {
    pub noticeable_knots: f64,
    pub rough_knots: f64,
    pub limit_knots: f64,
    pub crosswind_knots: f64,
    pub tailwind_knots: f64,
    pub headwind_knots: f64,
    pub crosswind_fraction: f64,
    pub tailwind_fraction: f64,
    pub severity: Severity,
    pub at_limit: bool,
}
impl Assessment {
    pub fn limits(self) -> Limits {
        Limits {
            noticeable_knots: self.noticeable_knots,
            rough_knots: self.rough_knots,
            limit_knots: self.limit_knots,
        }
    }
}

pub fn limits(max_takeoff_lbs: f64) -> Option<Limits> {
    if !max_takeoff_lbs.is_finite() || max_takeoff_lbs <= 0. {
        return None;
    }
    let values = if max_takeoff_lbs < 6_000. {
        [6., 10., 15.]
    } else if max_takeoff_lbs < 20_000. {
        [8., 14., 20.]
    } else if max_takeoff_lbs < 80_000. {
        [10., 18., 30.]
    } else if max_takeoff_lbs <= 200_000. {
        [13., 22., 33.]
    } else {
        [15., 26., 38.]
    };
    Some(Limits {
        noticeable_knots: values[0],
        rough_knots: values[1],
        limit_knots: values[2],
    })
}

pub fn assessment(
    max_takeoff_lbs: f64,
    wind_world_fps: Vector,
    aircraft_heading_rad: f64,
) -> Option<Assessment> {
    let limits = limits(max_takeoff_lbs)?;
    if !wind_world_fps.iter().all(|v| v.is_finite()) || !aircraft_heading_rad.is_finite() {
        return None;
    }
    let (sin, cos) = aircraft_heading_rad.sin_cos();
    let forward = [sin, 0., cos];
    let right = [cos, 0., -sin];
    let along_fps = wind_world_fps[0] * forward[0] + wind_world_fps[2] * forward[2];
    let across_fps = wind_world_fps[0] * right[0] + wind_world_fps[2] * right[2];
    let crosswind_knots = across_fps / FEET_PER_SECOND_PER_KNOT;
    let tailwind_knots = along_fps.max(0.) / FEET_PER_SECOND_PER_KNOT;
    let headwind_knots = (-along_fps).max(0.) / FEET_PER_SECOND_PER_KNOT;
    let crosswind_fraction = crosswind_fraction(crosswind_knots.abs(), limits);
    let tailwind_fraction = (tailwind_knots / 10.).clamp(0., 1.);
    let severity = severity(crosswind_knots.abs(), limits);
    Some(Assessment {
        noticeable_knots: limits.noticeable_knots,
        rough_knots: limits.rough_knots,
        limit_knots: limits.limit_knots,
        crosswind_knots,
        tailwind_knots,
        headwind_knots,
        crosswind_fraction,
        tailwind_fraction,
        severity,
        at_limit: severity == Severity::Limit || tailwind_knots >= 10.,
    })
}

pub fn ground_motion_fraction(horizontal_ground_speed_fps: f64) -> f64 {
    if !horizontal_ground_speed_fps.is_finite() {
        return 0.;
    }
    let t = (horizontal_ground_speed_fps.max(0.) / (5. * FEET_PER_SECOND_PER_KNOT)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

fn crosswind_fraction(knots: f64, limits: Limits) -> f64 {
    if knots <= limits.noticeable_knots {
        0.
    } else if knots <= limits.rough_knots {
        0.5 * (knots - limits.noticeable_knots) / (limits.rough_knots - limits.noticeable_knots)
    } else if knots < limits.limit_knots {
        0.5 + 0.5 * (knots - limits.rough_knots) / (limits.limit_knots - limits.rough_knots)
    } else {
        1.
    }
}

fn severity(knots: f64, limits: Limits) -> Severity {
    if knots < limits.noticeable_knots {
        Severity::BelowNoticeable
    } else if knots < limits.rough_knots {
        Severity::Noticeable
    } else if knots < limits.limit_knots {
        Severity::Rough
    } else {
        Severity::Limit
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mtow_boundaries_use_the_requested_classes() {
        for (weight, expected) in [
            (5_999., [6., 10., 15.]),
            (6_000., [8., 14., 20.]),
            (19_999., [8., 14., 20.]),
            (20_000., [10., 18., 30.]),
            (79_999., [10., 18., 30.]),
            (80_000., [13., 22., 33.]),
            (200_000., [13., 22., 33.]),
            (200_001., [15., 26., 38.]),
        ] {
            let actual = limits(weight).unwrap();
            assert_eq!(
                [
                    actual.noticeable_knots,
                    actual.rough_knots,
                    actual.limit_knots
                ],
                expected
            );
        }
        assert!(limits(0.).is_none() && limits(f64::NAN).is_none());
    }

    #[test]
    fn crosswind_anchors_tailwind_and_headwind_are_distinct() {
        let fps = |knots: f64| knots * FEET_PER_SECOND_PER_KNOT;
        for (knots, fraction, severity) in [
            (9., 0., Severity::BelowNoticeable),
            (10., 0., Severity::Noticeable),
            (18., 0.5, Severity::Rough),
            (30., 1., Severity::Limit),
        ] {
            let a = assessment(40_000., [fps(knots), 0., 0.], 0.).unwrap();
            assert_eq!(a.crosswind_fraction, fraction);
            assert_eq!(a.severity, severity);
        }
        let tail = assessment(40_000., [0., 0., fps(10.)], 0.).unwrap();
        assert_eq!(tail.tailwind_fraction, 1.);
        assert!(tail.at_limit);
        let moderate_tail = assessment(40_000., [0., 0., fps(5.)], 0.).unwrap();
        assert_eq!(moderate_tail.tailwind_fraction, 0.5);
        assert!(!moderate_tail.at_limit);
        let head = assessment(40_000., [0., 0., -fps(30.)], 0.).unwrap();
        assert_eq!(head.headwind_knots, 30.);
        assert_eq!(head.tailwind_fraction, 0.);
    }

    #[test]
    fn standstill_blend_is_smooth_and_heading_rotates_components() {
        let fps = |knots: f64| knots * FEET_PER_SECOND_PER_KNOT;
        assert_eq!(ground_motion_fraction(0.), 0.);
        assert_eq!(ground_motion_fraction(fps(2.5)), 0.5);
        assert_eq!(ground_motion_fraction(fps(5.)), 1.);
        let rotated = assessment(40_000., [fps(30.), 0., 0.], std::f64::consts::FRAC_PI_2).unwrap();
        assert_eq!(rotated.tailwind_knots, 30.);
        assert!(rotated.crosswind_knots.abs() < 1e-12);
    }
}
