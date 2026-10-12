//! STOL runways: the short runways the player is told about wherever a field
//! is picked or shown (slice CL-AP; John, 2026-10-11: "for short runways
//! maybe we label them as STOL so the player knows and then don't limit
//! starts there"). A label only: a STOL field is still a ground start for
//! anyone who picks it, and the tower and AI rules for short strips
//! (`crate::airport::SHORT_STRIP_FT`, John 2026-09-30) are unchanged. See
//! docs/spec/airports.md, "STOL runways".
use crate::airport::{Airport, Runway, Scene};

/// A runway shorter than this is a STOL runway, feet (`fitted`, agent
/// decision 2026-10-11). The AI F/A-18D's takeoff ground roll measured
/// 1,930 to 2,170 ft in the CL-AP probes, and fast jets in general need
/// about 2,000 to 3,000 ft at a light load; under 3,000 ft only a STOL or
/// vertical aircraft has margin. Every retail field is either a 1,074 ft
/// dirt strip or 4,060 ft and longer, so any line from 1,075 to 4,060 ft
/// picks the same 22 fields (the short strips).
pub const STOL_RUNWAY_FT: f64 = 3_000.0;

/// The tag shown after a STOL field's name.
pub const TAG: &str = "STOL";

/// Whether a runway of `length_ft` is a STOL runway.
pub fn length(length_ft: f64) -> bool {
    length_ft < STOL_RUNWAY_FT
}

/// Whether `runway` is a STOL runway.
pub fn runway(runway: &Runway) -> bool {
    length(runway.length_ft)
}

/// Whether every runway of `airport` is a STOL runway (an airport with no
/// runway in the scene is not).
pub fn airport(scene: &Scene, airport: &Airport) -> bool {
    let mut runways = airport
        .runway_objects
        .iter()
        .filter_map(|id| scene.runway(*id));
    runways.next().is_some_and(self::runway) && runways.all(self::runway)
}

/// `name` as the player sees it: with ` (STOL)` after it when `stol`.
pub fn label(name: &str, stol: bool) -> String {
    if stol {
        format!("{name} ({TAG})")
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_short_strips_are_stol_and_the_shortest_airport_is_not() {
        assert!(length(1_074.));
        assert!(!length(4_060.));
        assert!(!length(STOL_RUNWAY_FT));
        // The same fields as the short-strip rule, by design.
        for l in [1_074., 1_999., 4_060., 8_100.] {
            assert_eq!(length(l), crate::airport::short_strip_length(l), "{l}");
        }
        assert_eq!(label("Goose Green", true), "Goose Green (STOL)");
        assert_eq!(label("Kiev", false), "Kiev");
    }
}
