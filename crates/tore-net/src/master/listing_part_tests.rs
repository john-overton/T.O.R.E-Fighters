//! The listing part and a listing carried on by another host (stage K,
//! slice K8): the part's bytes, a rendezvous resumed from a part, an
//! unacknowledged move, and an old host letting its listing go. The real
//! master's side is `tore-master`'s `tests/migrate.rs`.

use super::*;
use crate::SplitMix64;
use crate::master::packet::{HeartbeatAck, RelayFrame, UnknownListing};
use crate::master::{LISTING_EXPIRY, relayed_address};
use crate::packet::DiscoverPhase;

const MASTER: &str = "198.51.100.1:26901";
const NEW_HOST: &str = "203.0.113.30:26900";

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

fn config() -> HostRendezvous {
    HostRendezvous {
        build: Build {
            protocol_version: 13,
            game_version: "0.1.3".into(),
            game_commit: "abc".into(),
            release: true,
        },
        dedicated: false,
        install_id: None,
        platform: 3,
        entropy: Entropy::Seeded(4),
    }
}

fn summary() -> ListingSummary {
    ListingSummary {
        protocol_version: 13,
        capacity: 8,
        phase: DiscoverPhase::Flying,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        name: "Friday night".into(),
        ..ListingSummary::default()
    }
}

fn part() -> ListingPart {
    ListingPart {
        master_name: "master.example.org:26901".into(),
        master: a(MASTER),
        listing_id: 0x4f1c_2a9b_e07d_3e11,
        token: 0x0123_4567_89ab_cdef,
        heartbeat_secs: 30,
        keep_secs: 15,
        channels: vec![
            PartChannel {
                channel: 7,
                key: 99,
                master: a(MASTER),
            },
            PartChannel {
                channel: 0xdead_beef,
                key: 5,
                master: a(MASTER),
            },
        ],
    }
}

/// Everything the rendezvous queued, decoded where it is a master packet.
fn sent(rendezvous: &mut Rendezvous) -> Vec<(SocketAddr, MasterPacket)> {
    std::iter::from_fn(|| rendezvous.poll_transmit())
        .filter_map(|t| MasterPacket::decode(&t.datagram).ok().map(|p| (t.to, p)))
        .collect()
}

fn heartbeats(sent: &[(SocketAddr, MasterPacket)]) -> Vec<(SocketAddr, u64)> {
    sent.iter()
        .filter_map(|(to, p)| match p {
            MasterPacket::Heartbeat(beat) => Some((*to, beat.token)),
            _ => None,
        })
        .collect()
}

#[test]
fn a_part_codes_to_bytes_and_back_strictly() {
    let mut v6 = part();
    v6.master = a("[2001:db8::1]:26901");
    v6.channels[1].master = a("[2001:db8::1]:26901");
    let mut empty = part();
    empty.channels.clear();
    empty.master_name.clear();
    for part in [part(), v6, empty] {
        let bytes = part.encode();
        assert_eq!(ListingPart::decode(&bytes), Ok(part.clone()));
        // Every cut and an extra byte are refused.
        for cut in 0..bytes.len() {
            assert_eq!(ListingPart::decode(&bytes[..cut]), Err(BadListingPart));
        }
        let mut longer = bytes.clone();
        longer.push(0);
        assert_eq!(ListingPart::decode(&longer), Err(BadListingPart));
    }
    // Two channels of an IPv4 master: 1 + 1 + 24 + 7 + 16 + 4 + 1 + 2 * 15.
    assert_eq!(part().encode().len(), 84);
    // Another format, another family, an IPv4-mapped IPv6 address.
    let bytes = part().encode();
    let mut format = bytes.clone();
    format[0] = 2;
    assert!(ListingPart::decode(&format).is_err());
    let family = 2 + part().master_name.len();
    let mut other = bytes.clone();
    other[family] = 5;
    assert!(ListingPart::decode(&other).is_err());
    let mut mapped = part();
    mapped.master = a("[::ffff:198.51.100.1]:26901");
    let bytes = mapped.encode();
    // Written as IPv4, so it reads back as the IPv4 address.
    assert_eq!(ListingPart::decode(&bytes).unwrap().master, a(MASTER));
    let mut forged = vec![1, 0, 6];
    forged.extend_from_slice(
        &"::ffff:198.51.100.1"
            .parse::<std::net::Ipv6Addr>()
            .unwrap()
            .octets(),
    );
    forged.extend_from_slice(&[0; 2 + 8 + 8 + 4 + 1]);
    assert!(ListingPart::decode(&forged).is_err());
}

#[test]
fn random_and_mutated_part_bytes_never_panic() {
    let mut rng = SplitMix64::new(81);
    let good = part().encode();
    for round in 0..20_000 {
        let bytes: Vec<u8> = if round % 2 == 0 {
            let len = (rng.next_u64() % 120) as usize;
            (0..len).map(|_| rng.next_u64() as u8).collect()
        } else {
            let mut bytes = good.clone();
            let at = (rng.next_u64() as usize) % bytes.len();
            bytes[at] ^= 1 << (rng.next_u64() % 8);
            bytes
        };
        if let Ok(part) = ListingPart::decode(&bytes) {
            // Whatever decodes codes to the same bytes again.
            assert_eq!(part.encode(), bytes);
        }
    }
}

#[test]
fn a_resumed_listing_heartbeats_at_once_with_the_token_and_holds_the_channels() {
    let now = Duration::from_secs(5);
    let part = part();
    let mut rendezvous = Rendezvous::resume(
        config(),
        &part,
        vec![Candidate::new(CandidateKind::Local, a(NEW_HOST))],
        now,
    );
    assert_eq!(rendezvous.state(), ListingState::Registering);
    assert!(rendezvous.listed_wanted());
    assert_eq!(
        rendezvous.relayed(),
        [relayed_address(7), relayed_address(0xdead_beef)]
    );
    // Nothing goes before the hosting loop's summary, and then a Heartbeat
    // with the old host's token, never a Register.
    rendezvous.update(now);
    assert!(heartbeats(&sent(&mut rendezvous)).is_empty());
    assert!(rendezvous.wants_summary(now));
    rendezvous.set_summary(now, summary());
    rendezvous.update(now);
    let out = sent(&mut rendezvous);
    assert_eq!(heartbeats(&out), [(a(MASTER), part.token)]);
    assert!(
        !out.iter()
            .any(|(_, p)| matches!(p, MasterPacket::Register(_)))
    );
    // A frame of a carried channel reaches the transport from its relayed
    // address.
    let frame = RelayFrame {
        channel: 7,
        key: 99,
        datagram: b"connect",
    }
    .encode()
    .unwrap();
    let (start, from) = rendezvous
        .relays
        .channels
        .unwrap_frame(now, a(MASTER), &frame)
        .unwrap();
    assert_eq!(
        (&frame[start..], from),
        (&b"connect"[..], relayed_address(7))
    );
    // The master moved it: listed here, as the master sees this game.
    let version = rendezvous.part_version();
    rendezvous.receive(
        now + Duration::from_millis(40),
        a(MASTER),
        &MasterPacket::HeartbeatAck(HeartbeatAck {
            listing_id: part.listing_id,
            seen: a(NEW_HOST),
        })
        .encode()
        .unwrap(),
    );
    assert_eq!(
        rendezvous.poll_event(),
        Some(RendezvousEvent::Listed {
            listing_id: part.listing_id,
            seen: a(NEW_HOST)
        })
    );
    assert_eq!(
        rendezvous.state(),
        ListingState::Listed {
            listing_id: part.listing_id,
            seen: a(NEW_HOST)
        }
    );
    assert_eq!(rendezvous.part_version(), version);
    // Its own part is the same listing, ready for the next migration.
    let mut own = rendezvous.listing_part().unwrap();
    assert!(own.master_name.is_empty());
    own.master_name = part.master_name.clone();
    assert_eq!(own, part);
    // From here it heartbeats as any listing: the next in 30 seconds.
    rendezvous.update(now + Duration::from_secs(10));
    assert!(heartbeats(&sent(&mut rendezvous)).is_empty());
}

#[test]
fn an_unacknowledged_move_is_asked_every_3_seconds_for_a_listings_expiry() {
    let start = Duration::from_secs(5);
    let mut rendezvous = Rendezvous::resume(config(), &part(), Vec::new(), start);
    rendezvous.set_summary(start, summary());
    let mut beats = Vec::new();
    let mut now = start;
    while now <= start + LISTING_EXPIRY + Duration::from_secs(1) {
        rendezvous.update(now);
        for _ in heartbeats(&sent(&mut rendezvous)) {
            beats.push(now - start);
        }
        if now < start + LISTING_EXPIRY {
            assert_eq!(rendezvous.state(), ListingState::Registering, "{now:?}");
        }
        now += Duration::from_millis(100);
    }
    // At 0, 3, 6, ... seconds while the master may be waiting out its
    // once-a-minute rule; silent only after a listing's expiry.
    assert_eq!(beats[..4], [0, 3, 6, 9].map(Duration::from_secs));
    assert!(beats.len() >= 30, "{beats:?}");
    assert_eq!(rendezvous.state(), ListingState::Silent);
    assert!(
        std::iter::from_fn(|| rendezvous.poll_event()).any(|e| e == RendezvousEvent::MasterSilent)
    );
}

#[test]
fn an_unknown_token_after_a_move_lists_the_game_afresh() {
    let now = Duration::from_secs(5);
    let part = part();
    let mut rendezvous = Rendezvous::resume(config(), &part, Vec::new(), now);
    rendezvous.set_summary(now, summary());
    rendezvous.update(now);
    sent(&mut rendezvous);
    let version = rendezvous.part_version();
    rendezvous.receive(
        now,
        a(MASTER),
        &MasterPacket::UnknownListing(UnknownListing { token: part.token })
            .encode()
            .unwrap(),
    );
    assert_ne!(rendezvous.part_version(), version);
    assert_eq!(rendezvous.listing_part(), None);
    rendezvous.update(now + Duration::from_millis(10));
    assert!(
        sent(&mut rendezvous)
            .iter()
            .any(|(to, p)| *to == a(MASTER) && matches!(p, MasterPacket::Register(_)))
    );
}

#[test]
fn releasing_a_listing_sends_nothing_and_forgets_its_channels() {
    let now = Duration::from_secs(5);
    let part = part();
    let mut rendezvous = Rendezvous::resume(config(), &part, Vec::new(), now);
    rendezvous.set_summary(now, summary());
    rendezvous.update(now);
    let version = rendezvous.part_version();
    // Queued and not yet sent: dropped with the rest.
    let mut released = rendezvous.release().unwrap();
    released.master_name = part.master_name.clone();
    assert_eq!(released, part);
    assert_ne!(rendezvous.part_version(), version);
    assert!(rendezvous.poll_transmit().is_none());
    assert!(rendezvous.relayed().is_empty());
    assert_eq!(rendezvous.state(), ListingState::Off);
    assert_eq!(rendezvous.listing_part(), None);
    assert_eq!(rendezvous.poll_event(), Some(RendezvousEvent::Unlisted));
    // Later updates send nothing either: no Unregister, no Relay close.
    rendezvous.update(now + Duration::from_secs(60));
    assert!(rendezvous.poll_transmit().is_none());
    // A frame of a forgotten channel no longer reaches the transport.
    let frame = RelayFrame {
        channel: 7,
        key: 99,
        datagram: b"late",
    }
    .encode()
    .unwrap();
    assert!(
        rendezvous
            .relays
            .channels
            .unwrap_frame(now, a(MASTER), &frame)
            .is_none()
    );
    assert_eq!(rendezvous.release(), None);
}

#[test]
fn a_host_listing_resumes_from_the_parts_master_and_names_it_in_its_own() {
    let part = part();
    let mut listing =
        HostListing::resume(&part, config(), 26900, Duration::ZERO).expect("the name parses");
    assert_eq!(listing.master_text(), "master.example.org:26901");
    assert_eq!(listing.rendezvous().master(), Some(a(MASTER)));
    assert_eq!(listing.part(), Some(part.clone()));
    let version = listing.part_version();
    assert_eq!(listing.release(), Some(part.clone()));
    assert_ne!(listing.part_version(), version);
    assert_eq!(listing.part(), None);
    let mut bad = part;
    bad.master_name = "bad:0".into();
    assert!(HostListing::resume(&bad, config(), 26900, Duration::ZERO).is_err());
}
