//! The listing part (slice K7a; docs/ARCHITECTURE.md, "What moves with the
//! host"): the hosting thread's coded `HostListing`, which `Host` holds
//! opaque, goes into the standby stream like every other part, a standby
//! keeps it, and a host that takes the game over has it back.

use super::super::*;
use crate::journal::{Appoint, Part, Record, StreamWriter};
use crate::standby::{MissionKey, Standby};
use crate::wire::messages::StandbyMark;
use tore_formats::aircraft::AircraftId;
use tore_net::{Entropy, LINK_ADDRESS};
use tore_world::test_support::resources::{THEATER, resources};

fn config() -> HostConfig {
    HostConfig {
        entropy: Entropy::Seeded(17),
        open_planes: OpenPlanes::All,
        house: Some(LINK_ADDRESS),
        start: StartMode::King,
        crown: CrownRule::FirstPlayer,
        ..HostConfig::new(BuildId {
            version: "0.1.3-1-gtest".into(),
            commit: "test-commit".into(),
            release: false,
        })
    }
}

/// The State records an update put in the journal, as the stream codes them.
fn parts_out(host: &mut Host, now: Duration) -> Vec<crate::journal::StatePart> {
    host.update(now);
    let (records, lost) = host.drain_journal();
    assert!(!lost);
    records
        .into_iter()
        .filter_map(|record| match record {
            journal::JournalRecord::State(part) => Some(part),
            _ => None,
        })
        .collect()
}

#[test]
fn the_listing_part_goes_through_the_stream_to_a_host_that_takes_over() {
    let resources = Arc::new(resources());
    let spec = MissionSpec::new(THEATER, AircraftId::F18);
    let mut host = Host::new(spec, Arc::clone(&resources), config()).unwrap();
    assert_eq!(host.listing_part(), None);
    let listing = b"a listing part, opaque to the host".to_vec();
    host.set_listing_part(Some(listing.clone()));
    assert_eq!(host.listing_part(), Some(&listing[..]));
    host.record_journal(true);

    // Every part goes out once, the listing among them; an unchanged one
    // does not go again.
    let first = parts_out(&mut host, Duration::ZERO);
    let listed: Vec<_> = first.iter().filter(|p| p.part == Part::Listing).collect();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].bytes, listing);
    assert!(
        parts_out(&mut host, Duration::from_millis(10))
            .iter()
            .all(|p| p.part != Part::Listing)
    );

    // A standby takes the stream in order and keeps the part.
    let mut standby = Standby::new(Box::new(|_: &MissionKey| Err("no flight".to_owned())));
    let mut writer = StreamWriter::new();
    let mut feed = |standby: &mut Standby, record: Record| {
        standby.receive(&writer.encode(&record).unwrap()).unwrap();
    };
    feed(
        &mut standby,
        Record::Appoint(Appoint {
            role: StandbyMark::First,
            warm: true,
            check_every: crate::journal::CHECK_EVERY_TICKS,
            checkpoint_every: crate::journal::CHECKPOINT_EVERY_TICKS,
            mission: host.number,
        }),
    );
    for part in first {
        feed(&mut standby, Record::State(part));
    }
    assert_eq!(standby.part(Part::Listing).unwrap().bytes, listing);
    assert!(standby.ready(), "a standby in the lobby holds all it needs");

    // The host it becomes holds the old host's listing part.
    let taken = Host::resume(
        standby.take_over().unwrap(),
        Arc::clone(&resources),
        config(),
        Resumption {
            house: 0,
            now: Duration::from_secs(1),
            present: None,
        },
    )
    .unwrap();
    assert_eq!(taken.listing_part(), Some(&listing[..]));

    // A game taken off the Internet Lobby sends an empty part: none.
    host.set_listing_part(None);
    let cleared = parts_out(&mut host, Duration::from_millis(20));
    let part = cleared.iter().find(|p| p.part == Part::Listing).unwrap();
    assert!(part.bytes.is_empty());
    let mut fresh = Host::new(
        MissionSpec::new(THEATER, AircraftId::F18),
        resources,
        config(),
    )
    .unwrap();
    fresh.set_listing_part(Some(listing));
    fresh
        .restore_part(
            Part::Listing,
            &part.bytes,
            &mut |_| ConnectionId(0),
            Duration::ZERO,
        )
        .unwrap();
    assert_eq!(fresh.listing_part(), None);
}
