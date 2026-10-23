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
/// F-22A's row, as they take its sensors, because it reads
/// [`AircraftId::source`]. The answer is the aircraft's installed radar
/// record ([`AircraftId::radar`], whose match has no wildcard, so a new
/// aircraft does not compile until it says whether it has one).
///
/// The fourteen original selectable aircraft all have a radar. Of the
/// variety import's aircraft, the C-130, V-22, Mi-24, CH-47, MiG-17F, 747
/// and A310 have none (docs/formats/aircraft-variety.md); they stay linked,
/// and their players see no link cues on displays they do not have.
pub fn has_radar(id: AircraftId) -> bool {
    id.source().radar().is_some()
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
    fn the_aircraft_without_a_radar_are_the_seven_variety_types() {
        // The sensor summary of the imported data lists a radar for each of
        // the fourteen original selectable aircraft; the variety import
        // installs none on seven of its types.
        let without: Vec<AircraftId> = AircraftId::SELECTABLE
            .into_iter()
            .filter(|id| !has_radar(*id))
            .collect();
        assert_eq!(
            without,
            [
                AircraftId::C130,
                AircraftId::V22,
                AircraftId::Mi24,
                AircraftId::Ch47,
                AircraftId::Mig17,
                AircraftId::B747,
                AircraftId::A310,
            ]
        );
        assert!(AircraftId::SELECTABLE[..14].iter().all(|id| has_radar(*id)));
    }
}
