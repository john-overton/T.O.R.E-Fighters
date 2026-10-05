//! Flight data link tiers: which aircraft types can share a picture of the
//! fight with their flightmates, and with other flights. Stage G of the
//! multiplayer plan; the guide is `docs/DATALINK.md` and the design is
//! `docs/ARCHITECTURE.md`, "Flight data link".
//!
//! This is pure data and two pair rules. The picture itself lives in
//! `tore-world`, which sees human and AI aircraft alike.
//!
//! `opinionated` (agent decision, 2026-10-05, awaiting John's review): the
//! tiers are a gameplay grouping chosen from the retail manual's avionics
//! notes. They are not derived from the radar presets or the countermeasure
//! generations, which are gameplay groupings of their own.

use tore_formats::aircraft::AircraftId;

/// What an aircraft type can share, in increasing order of capability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum LinkTier {
    /// Engagements by radio only; assignments arrive as a spoken bearing,
    /// range and height.
    Voice,
    /// Engagements, locks, assignments, tracks and member state inside its own
    /// flight.
    Flight,
    /// As [`Self::Flight`], and tracks and locks with other flights of its side
    /// that are also Network.
    Network,
}

impl LinkTier {
    /// The tier's name as logs and the probe print it.
    pub fn label(self) -> &'static str {
        match self {
            Self::Voice => "voice",
            Self::Flight => "flight",
            Self::Network => "network",
        }
    }
}

/// The link tier of an aircraft type. The F-22N and the F/A-XX take the
/// F-22A's row, as they take its sensors, because the match reads
/// [`AircraftId::source`]. Every aircraft needs a row: the match has no
/// wildcard, so a new aircraft does not compile until it has one.
pub fn tier(id: AircraftId) -> LinkTier {
    match id.source() {
        // The retail manual lists a data link for each of these three: the
        // AN/ASW-25 radio link, the AN/ASW-27B digital link and MIDS.
        AircraftId::F18 | AircraftId::F14 | AircraftId::Rafale => LinkTier::Network,
        // The manual: "Intra-flight data link automatically shares tactical
        // information between two or more F-22s". The two Flankers are a
        // gameplay grouping, so each side has linked fighters.
        AircraftId::F22 | AircraftId::F22n | AircraftId::Faxx => LinkTier::Flight,
        AircraftId::Su35 | AircraftId::Su27 => LinkTier::Flight,
        // No link in the manual.
        AircraftId::Mig29
        | AircraftId::Mig23
        | AircraftId::Mig21
        | AircraftId::Su25
        | AircraftId::A4E
        | AircraftId::X31 => LinkTier::Voice,
    }
}

/// Whether two members of one flight are linked: both have at least the
/// Flight tier. Each pair is judged on its own, so in a flight of two F-22As
/// and an A-4E the F-22As are linked and the A-4E hears the radio.
pub fn flight_linked(a: LinkTier, b: LinkTier) -> bool {
    a >= LinkTier::Flight && b >= LinkTier::Flight
}

/// Whether two aircraft of different flights on the same side are linked over
/// the battle net: both have the Network tier.
pub fn net_linked(a: LinkTier, b: LinkTier) -> bool {
    a == LinkTier::Network && b == LinkTier::Network
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_selectable_aircraft_has_a_tier() {
        for id in AircraftId::SELECTABLE {
            // The function is total; the tier of a variant is its source's.
            assert_eq!(tier(id), tier(id.source()), "{id:?}");
        }
    }

    #[test]
    fn the_tier_table_is_the_design_table() {
        use AircraftId::*;
        let network = [F18, F14, Rafale];
        let flight = [F22, F22n, Faxx, Su35, Su27];
        let voice = [Mig29, Mig23, Mig21, Su25, A4E, X31];
        for id in network {
            assert_eq!(tier(id), LinkTier::Network, "{id:?}");
        }
        for id in flight {
            assert_eq!(tier(id), LinkTier::Flight, "{id:?}");
        }
        for id in voice {
            assert_eq!(tier(id), LinkTier::Voice, "{id:?}");
        }
        // Each of the fourteen selectable aircraft is in exactly one list.
        assert_eq!(
            network.len() + flight.len() + voice.len(),
            AircraftId::SELECTABLE.len()
        );
        for id in AircraftId::SELECTABLE {
            let listed = [&network[..], &flight[..], &voice[..]]
                .iter()
                .filter(|list| list.contains(&id))
                .count();
            assert_eq!(listed, 1, "{id:?}");
        }
    }

    #[test]
    fn the_f22n_and_the_faxx_resolve_to_the_f22a() {
        assert_eq!(tier(AircraftId::F22n), tier(AircraftId::F22));
        assert_eq!(tier(AircraftId::Faxx), tier(AircraftId::F22));
    }

    #[test]
    fn a_pair_is_linked_when_both_have_the_flight_tier() {
        use LinkTier::*;
        for a in [Voice, Flight, Network] {
            for b in [Voice, Flight, Network] {
                let expected = a != Voice && b != Voice;
                assert_eq!(flight_linked(a, b), expected, "{a:?} {b:?}");
                assert_eq!(flight_linked(a, b), flight_linked(b, a));
            }
        }
    }

    #[test]
    fn a_pair_is_net_linked_when_both_are_network() {
        use LinkTier::*;
        for a in [Voice, Flight, Network] {
            for b in [Voice, Flight, Network] {
                let expected = a == Network && b == Network;
                assert_eq!(net_linked(a, b), expected, "{a:?} {b:?}");
            }
        }
    }

    #[test]
    fn a_mixed_flight_is_judged_pair_by_pair() {
        // Two F-22As and an A-4E: the Raptors share, the Skyhawk hears.
        let raptor = tier(AircraftId::F22);
        let skyhawk = tier(AircraftId::A4E);
        assert!(flight_linked(raptor, raptor));
        assert!(!flight_linked(raptor, skyhawk));
        assert!(!flight_linked(skyhawk, skyhawk));
        // F-22As never join the battle net.
        assert!(!net_linked(raptor, tier(AircraftId::F14)));
    }
}
