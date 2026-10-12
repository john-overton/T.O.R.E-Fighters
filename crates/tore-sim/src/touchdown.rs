//! The one touchdown rule: may this aircraft come down on this surface?
//!
//! Hybrid ground contact ([`crate::research::Research::contact`]), the AI's
//! ejection check ([`crate::ejection::catastrophic`]) and the AI's go-around
//! check on final all ask [`assess`], so the player and the AI cannot
//! disagree. The behaviour is specified in docs/spec/touchdown.md.
//!
//! - A landable surface (an airfield's box) is accepted for every aircraft,
//!   exactly as before; its limits stay the aircraft's landing limits.
//! - Anywhere else, a fixed-wing aircraft (and the V-22 in airplane mode, the
//!   AV-8 and Yak-141 in wingborne flight) crashes, exactly as before.
//! - A helicopter, the V-22 in helicopter mode and a vectoring jet in a hover
//!   may touch down on open ground or on the flat top of any object (a
//!   building roof, a hangar, a bunker, a deck), when the surface is not
//!   water, tilts under 10 degrees, and the sink rate and ground speed are
//!   within John's limits (requested 2026-10-12, opinionated). A harder
//!   touchdown damages the gear; a much harder one is a crash.
//!
//! Provenance: the limits are opinionated (John, 2026-10-12); the hard and
//! crash bands, the nacelle and nozzle thresholds and the minimum roof size
//! are fitted (agent decisions).

use crate::{flight::State, models::variety::LiftKind, research::Surface};

/// Sink rate under which a touchdown is clean, ft/s (600 ft/min). Opinionated
/// (John, 2026-10-12).
pub const MAX_SINK_FPS: f64 = 10.;
/// Ground speed under which a helicopter's or the V-22's (helicopter mode)
/// touchdown is clean, knots. Opinionated (John, 2026-10-12).
pub const ROTOR_MAX_GROUND_KNOTS: f64 = 20.;
/// The same for the AV-8 and Yak-141 in a hover, knots. Opinionated (John,
/// 2026-10-12).
pub const HOVER_MAX_GROUND_KNOTS: f64 = 10.;
/// Steepest ground or roof that can be landed on, degrees. Opinionated (John,
/// 2026-10-12).
pub const MAX_SLOPE_DEG: f64 = 10.;
/// A sink between [`MAX_SINK_FPS`] and this damages the gear; beyond it the
/// touchdown is a crash, ft/s. Fitted: the width of the "gear damage" band is
/// an agent decision (twice John's limit).
pub const CRASH_SINK_FPS: f64 = 20.;
/// Ground speed beyond this many times the clean limit is a crash rather than
/// gear damage. Fitted.
pub const CRASH_SPEED_FACTOR: f64 = 1.5;
/// The V-22 counts as a helicopter at or past this nacelle angle, degrees of
/// its 97.5 travel (90 is straight up; its helicopter preset is 87). Fitted.
pub const HELICOPTER_MODE_NACELLE_DEG: f64 = 75.;
/// A vectoring jet counts as hovering at or past this nozzle angle, degrees
/// from aft (90 is straight down; the short-takeoff stops are 60, 40, 20).
/// Fitted.
pub const HOVER_NOZZLE_DEG: f64 = 60.;
/// Smallest half-width of an object's flat top that counts as a landing
/// surface, feet, so a pole or an antenna is not a pad. Fitted.
pub const MIN_TOP_HALF_FT: f64 = 8.;
/// How far below the aircraft's wheels a flat top may be and still support
/// it, feet (float noise and one tick's fall). Fitted.
pub const TOP_REACH_FT: f64 = 0.5;

const KNOTS_TO_FPS: f64 = 1.687_81;

/// How the aircraft is flying, for the touchdown rule.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// Lands on airfields only: fixed-wing, the V-22 in airplane mode, a
    /// vectoring jet in wingborne flight.
    Fixed,
    /// A helicopter, or the V-22 in helicopter mode.
    Rotor,
    /// An AV-8 or Yak-141 with the nozzles down.
    Hover,
}

impl Class {
    /// The class of `s` now.
    pub fn of(s: &State) -> Self {
        let Some(lift) = s.model().powered_lift() else {
            return Self::Fixed;
        };
        match lift.kind {
            LiftKind::Helicopter => Self::Rotor,
            LiftKind::Tiltrotor => {
                let range = lift
                    .tiltrotor
                    .map_or(crate::models::variety::V22_NACELLE_RANGE_DEGREES, |t| {
                        t.nacelle_range_degrees
                    });
                if s.lift_controls.conversion_actual.clamp(0., 1.) * range
                    >= HELICOPTER_MODE_NACELLE_DEG
                {
                    Self::Rotor
                } else {
                    Self::Fixed
                }
            }
            LiftKind::VectorJet => {
                if s.nozzle_degrees() >= HOVER_NOZZLE_DEG {
                    Self::Hover
                } else {
                    Self::Fixed
                }
            }
        }
    }

    /// Ground speed under which a touchdown is clean, knots; `None` for a
    /// class that may not land off an airfield.
    pub fn clean_ground_knots(self) -> Option<f64> {
        match self {
            Self::Fixed => None,
            Self::Rotor => Some(ROTOR_MAX_GROUND_KNOTS),
            Self::Hover => Some(HOVER_MAX_GROUND_KNOTS),
        }
    }

    /// Whether this class may touch down away from an airfield at all.
    pub fn can_land_off_airfield(self) -> bool {
        self != Self::Fixed
    }
}

/// Why a touchdown is refused: a crash.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Water,
    /// Off an airfield, for an aircraft that may not land there.
    NotLandable,
    /// The ground or roof tilts past [`MAX_SLOPE_DEG`].
    Steep,
    /// Ground speed beyond [`CRASH_SPEED_FACTOR`] times the clean limit.
    TooFast,
    /// Sink beyond [`CRASH_SINK_FPS`].
    TooHard,
}

/// What a touchdown on a surface comes to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Within every limit (or on an airfield): no damage.
    Safe,
    /// Over a clean limit but short of a crash: the gear is damaged.
    GearDamage,
    /// A crash.
    Refused(Refusal),
}

/// The result of [`assess`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touchdown {
    pub class: Class,
    pub verdict: Verdict,
    /// Sink rate, ft/s, positive downward (zero or negative when climbing).
    pub sink_fps: f64,
    pub ground_speed_knots: f64,
}

impl Touchdown {
    /// The touchdown is a crash.
    pub fn is_refused(&self) -> bool {
        matches!(self.verdict, Verdict::Refused(_))
    }
    /// The touchdown damages the gear.
    pub fn damages_gear(&self) -> bool {
        self.verdict == Verdict::GearDamage
    }
}

/// May `s`, moving as it is now, touch down on `surface`? Uses the aircraft's
/// class now, its vertical and ground velocity. The gear position and the
/// landing-limit classification (attitude, forward and side speed) stay with
/// the callers, as they were.
pub fn assess(s: &State, surface: &Surface) -> Touchdown {
    assess_with(
        Class::of(s),
        -s.velocity[1],
        s.velocity[0].hypot(s.velocity[2]) / KNOTS_TO_FPS,
        surface,
    )
}

/// The landing-limit classification of `s` touching down on `surface`
/// ([`tore_formats::flight_model::ground::landing_severity`]). On an airfield
/// and for every fixed-wing aircraft it is the aircraft's own limits, as it
/// always was. Away from an airfield, an aircraft that may land there is held
/// to its roll and pitch limits only: this module's sink and ground speed
/// limits replace the record's speed terms, which are a fighter's.
pub fn severity(
    limits: tore_formats::flight_model::ground::LandingLimits,
    s: &State,
    surface: &Surface,
) -> tore_formats::flight_model::ground::LandingSeverity {
    use tore_formats::flight_model::ground::landing_severity;
    let basis = crate::attitude::Basis::new(s.yaw, s.pitch, s.bank);
    let mut forward = crate::attitude::dot(s.velocity, basis.forward);
    let mut side = crate::attitude::dot(s.velocity, basis.right);
    let mut vertical = s.velocity[1];
    if !surface.landable && !surface.water && Class::of(s).can_land_off_airfield() {
        (forward, side, vertical) = (0., 0., 0.);
    }
    landing_severity(
        limits,
        (s.bank.to_degrees() * 256.) as i32,
        (s.pitch.to_degrees() * 256.) as i32,
        (forward * 256.) as i32,
        (side * 256.) as i32,
        vertical as i16,
    )
}

/// The rule itself, on plain numbers (see [`assess`]).
pub fn assess_with(
    class: Class,
    sink_fps: f64,
    ground_speed_knots: f64,
    surface: &Surface,
) -> Touchdown {
    let verdict = verdict(class, sink_fps, ground_speed_knots, surface);
    Touchdown {
        class,
        verdict,
        sink_fps,
        ground_speed_knots,
    }
}

fn verdict(class: Class, sink: f64, knots: f64, surface: &Surface) -> Verdict {
    if surface.water {
        return Verdict::Refused(Refusal::Water);
    }
    if surface.landable {
        return Verdict::Safe;
    }
    let Some(clean_knots) = class.clean_ground_knots() else {
        return Verdict::Refused(Refusal::NotLandable);
    };
    if surface.slope_deg > MAX_SLOPE_DEG {
        return Verdict::Refused(Refusal::Steep);
    }
    if knots > clean_knots * CRASH_SPEED_FACTOR {
        return Verdict::Refused(Refusal::TooFast);
    }
    if sink > CRASH_SINK_FPS {
        return Verdict::Refused(Refusal::TooHard);
    }
    if sink > MAX_SINK_FPS || knots > clean_knots {
        return Verdict::GearDamage;
    }
    Verdict::Safe
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ground() -> Surface {
        Surface::terrain(100.)
    }
    fn rule(class: Class, sink: f64, knots: f64, surface: Surface) -> Verdict {
        assess_with(class, sink, knots, &surface).verdict
    }

    #[test]
    fn fixed_wing_off_the_airfield_is_always_refused() {
        for (sink, knots) in [(0., 0.), (5., 5.), (200., 300.)] {
            assert_eq!(
                rule(Class::Fixed, sink, knots, ground()),
                Verdict::Refused(Refusal::NotLandable)
            );
        }
    }

    #[test]
    fn an_airfield_accepts_every_class_without_the_off_field_limits() {
        for class in [Class::Fixed, Class::Rotor, Class::Hover] {
            assert_eq!(
                rule(class, 90., 200., Surface::runway(0.)),
                Verdict::Safe,
                "{class:?}"
            );
        }
    }

    #[test]
    fn water_is_refused_everywhere_but_an_airfield_flag_does_not_save_it_either() {
        let mut water = ground();
        water.water = true;
        for class in [Class::Fixed, Class::Rotor, Class::Hover] {
            assert_eq!(rule(class, 1., 1., water), Verdict::Refused(Refusal::Water));
        }
    }

    #[test]
    fn sink_limit_on_both_sides_for_a_helicopter() {
        assert_eq!(rule(Class::Rotor, 10., 0., ground()), Verdict::Safe);
        assert_eq!(rule(Class::Rotor, 10.01, 0., ground()), Verdict::GearDamage);
        assert_eq!(rule(Class::Rotor, 20., 0., ground()), Verdict::GearDamage);
        assert_eq!(
            rule(Class::Rotor, 20.01, 0., ground()),
            Verdict::Refused(Refusal::TooHard)
        );
        // Climbing is no sink at all.
        assert_eq!(rule(Class::Rotor, -3., 0., ground()), Verdict::Safe);
    }

    #[test]
    fn ground_speed_limit_on_both_sides_per_class() {
        assert_eq!(rule(Class::Rotor, 2., 20., ground()), Verdict::Safe);
        assert_eq!(rule(Class::Rotor, 2., 20.1, ground()), Verdict::GearDamage);
        assert_eq!(rule(Class::Rotor, 2., 30., ground()), Verdict::GearDamage);
        assert_eq!(
            rule(Class::Rotor, 2., 30.1, ground()),
            Verdict::Refused(Refusal::TooFast)
        );
        assert_eq!(rule(Class::Hover, 2., 10., ground()), Verdict::Safe);
        assert_eq!(rule(Class::Hover, 2., 10.1, ground()), Verdict::GearDamage);
        assert_eq!(
            rule(Class::Hover, 2., 15.1, ground()),
            Verdict::Refused(Refusal::TooFast)
        );
        // 15 kt is clean for a helicopter and damaging for a hovering jet.
        assert_eq!(rule(Class::Rotor, 2., 15., ground()), Verdict::Safe);
        assert_eq!(rule(Class::Hover, 2., 15., ground()), Verdict::GearDamage);
    }

    #[test]
    fn slope_limit_on_both_sides() {
        let mut tilted = ground();
        tilted.slope_deg = 10.;
        assert_eq!(rule(Class::Rotor, 2., 2., tilted), Verdict::Safe);
        tilted.slope_deg = 10.01;
        assert_eq!(
            rule(Class::Rotor, 2., 2., tilted),
            Verdict::Refused(Refusal::Steep)
        );
        assert_eq!(
            rule(Class::Hover, 2., 2., tilted),
            Verdict::Refused(Refusal::Steep)
        );
    }

    #[test]
    fn a_flat_roof_is_ground_that_follows_the_same_limits() {
        // A roof arrives as a non-landable, level surface at the roof height.
        let roof = Surface::terrain(80.);
        assert_eq!(rule(Class::Rotor, 5., 5., roof), Verdict::Safe);
        assert_eq!(rule(Class::Rotor, 15., 5., roof), Verdict::GearDamage);
        assert_eq!(
            rule(Class::Rotor, 25., 5., roof),
            Verdict::Refused(Refusal::TooHard)
        );
        assert_eq!(
            rule(Class::Fixed, 1., 1., roof),
            Verdict::Refused(Refusal::NotLandable)
        );
    }

    #[test]
    fn the_wire_free_helper_reports_its_inputs() {
        let t = assess_with(Class::Rotor, 4., 12., &ground());
        assert_eq!(t.class, Class::Rotor);
        assert!(!t.is_refused() && !t.damages_gear());
        assert_eq!((t.sink_fps, t.ground_speed_knots), (4., 12.));
    }
}
