//! Flight data link capability: which aircraft types have a radar, the one
//! thing the data link asks of an aircraft type. Stage G of the multiplayer
//! plan; the guide is `docs/DATALINK.md` and the design is
//! `docs/ARCHITECTURE.md`, "Flight data link".
//!
//! John, 2026-10-05: every friendly aircraft has the data link, whatever its
//! type. There are no tiers and no pairs of members that cannot share: every
//! aircraft of a side is linked with every other, in its flight and over the
//! side's battle net. An aircraft with no radar is still linked, and its AI
//! uses the picture like any other; its player simply sees no link cues on the
//! displays the aircraft does not have.
//!
//! This is pure data. The picture itself lives in `tore-world`, which sees
//! human and AI aircraft alike.

use tore_formats::aircraft::AircraftId;

pub mod sort;

/// Whether an aircraft type has a radar. The F-22N and the F/A-XX take the
/// F-22A's row, as they take its sensors, because the match reads
/// [`AircraftId::source`]. Every aircraft needs a row: the match has no
/// wildcard, so a new aircraft does not compile until it has one.
///
/// Every aircraft ported so far has a radar record, so every row is `true`;
/// `--sensor-summary` lists each one's radar. The flag is here for the first
/// aircraft that has none.
pub fn has_radar(id: AircraftId) -> bool {
    match id.source() {
        AircraftId::F18
        | AircraftId::Rafale
        | AircraftId::F14
        | AircraftId::A4E
        | AircraftId::X31
        | AircraftId::Mig29
        | AircraftId::Su27
        | AircraftId::Mig21
        | AircraftId::Su25
        | AircraftId::Mig23
        | AircraftId::Su35
        | AircraftId::F22
        | AircraftId::F22n
        | AircraftId::Faxx => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_selectable_aircraft_has_an_entry() {
        for id in AircraftId::SELECTABLE {
            // The function is total; the entry of a variant is its source's.
            assert_eq!(has_radar(id), has_radar(id.source()), "{id:?}");
        }
    }

    #[test]
    fn the_f22n_and_the_faxx_resolve_to_the_f22a() {
        assert_eq!(has_radar(AircraftId::F22n), has_radar(AircraftId::F22));
        assert_eq!(has_radar(AircraftId::Faxx), has_radar(AircraftId::F22));
    }

    #[test]
    fn every_ported_aircraft_has_a_radar() {
        // The sensor summary of the imported data lists a radar for each of
        // the fourteen selectable aircraft.
        assert!(AircraftId::SELECTABLE.into_iter().all(has_radar));
    }
}
