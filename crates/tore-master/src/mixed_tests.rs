//! Released master-1/game-67 and current master-2/game-71 populations sharing
//! one master. Every request and reply crosses the real wire codec.
use crate::{Master, MasterPort, Settings};
use std::{net::SocketAddr, time::Duration};
use tore_net::{Entropy, master::*};

fn addr(port: u16) -> SocketAddr {
    ([127, 0, 0, 1], port).into()
}
fn build(version: u16) -> Build {
    Build {
        protocol_version: if version == 1 { 67 } else { 71 },
        game_version: "0.2.0".into(),
        game_commit: "host-commit".into(),
        compatibility: if version == 1 { 0 } else { 0x3FA9C21B },
        release: true,
    }
}
fn summary(version: u16, name: &str) -> ListingSummary {
    let b = build(version);
    ListingSummary {
        protocol_version: b.protocol_version,
        game_version: b.game_version,
        game_commit: b.game_commit,
        compatibility: b.compatibility,
        name: name.into(),
        capacity: 6,
        ..Default::default()
    }
}
struct Rig {
    master: Master,
    now: Duration,
}
impl Rig {
    fn new() -> Self {
        Self {
            master: Master::new(Settings::default(), Entropy::Seeded(456), 0),
            now: Duration::from_secs(100),
        }
    }
    fn drain(&mut self) -> Vec<(SocketAddr, u16, MasterPacket)> {
        std::iter::from_fn(|| self.master.poll_transmit())
            .map(|out| {
                let version = u16::from_le_bytes([out.datagram[5], out.datagram[6]]);
                (
                    out.to,
                    version,
                    MasterPacket::decode(&out.datagram).unwrap(),
                )
            })
            .collect()
    }
    fn send(
        &mut self,
        version: u16,
        from: SocketAddr,
        packet: MasterPacket,
    ) -> Vec<(SocketAddr, u16, MasterPacket)> {
        self.now += Duration::from_secs(1);
        self.master.receive(
            self.now,
            MasterPort::Main,
            from,
            &packet.encode_in(version).unwrap(),
        );
        self.drain()
    }
    fn reply(&mut self, version: u16, from: SocketAddr, packet: MasterPacket) -> MasterPacket {
        let replies = self.send(version, from, packet);
        let (_, received, packet) = replies
            .into_iter()
            .find(|(to, _, _)| *to == from)
            .expect("reply to requester");
        assert_eq!(
            received, version,
            "reply must use the requester's released layout"
        );
        packet
    }
    fn register(&mut self, version: u16, port: u16) -> Listed {
        let mut request = Register {
            nonce: u64::from(port),
            cookie: 0,
            build: build(version),
            dedicated: true,
            telemetry: false,
            install_id: 0,
            platform: 3,
            candidates: Vec::new(),
            summary: summary(
                version,
                if version == 1 {
                    "Released 0.2.0"
                } else {
                    "New code host"
                },
            ),
        };
        let MasterPacket::Challenge(challenge) =
            self.reply(version, addr(port), MasterPacket::Register(request.clone()))
        else {
            panic!("challenge")
        };
        request.cookie = challenge.cookie;
        let MasterPacket::Listed(listed) =
            self.reply(version, addr(port), MasterPacket::Register(request))
        else {
            panic!("listed")
        };
        listed
    }
    fn page(&mut self, version: u16, other: bool, different: bool) -> Page {
        let mut b = build(version);
        b.game_commit = "different-client-commit".into();
        if different {
            if version == 1 {
                b.game_version = "0.1.2".into();
            } else {
                b.compatibility ^= 1;
            }
        }
        let MasterPacket::Page(page) = self.reply(
            version,
            addr(41000 + version),
            MasterPacket::Browse(Browse {
                nonce: 99,
                build: b,
                other_builds: other,
                full_games: true,
                cursor: 0,
            }),
        ) else {
            panic!("page")
        };
        page
    }
    fn introduce(
        &mut self,
        version: u16,
        player: SocketAddr,
        listing: u64,
        nonce: u64,
    ) -> Vec<(SocketAddr, u16, MasterPacket)> {
        let mut b = build(version);
        b.game_commit = "another-commit".into();
        let mut request = Introduce {
            nonce,
            cookie: 0,
            listing_id: listing,
            build: b,
            mapping: MappingType::Unknown,
            candidates: Vec::new(),
        };
        let MasterPacket::Challenge(challenge) =
            self.reply(version, player, MasterPacket::Introduce(request.clone()))
        else {
            panic!("challenge")
        };
        request.cookie = challenge.cookie;
        self.send(version, player, MasterPacket::Introduce(request))
    }
}

#[test]
fn released_and_current_browsers_see_only_their_population_even_with_other_versions() {
    let mut r = Rig::new();
    let old = r.register(1, 40001);
    let new = r.register(2, 40002);
    assert_eq!(r.master.listings().len(), 2);
    for other in [false, true] {
        for (version, id, code) in [(1, old.listing_id, 0), (2, new.listing_id, 0x3FA9C21B)] {
            let page = r.page(version, other, false);
            assert_eq!(page.matching, 1);
            assert_eq!(page.entries.len(), 1);
            assert_eq!(page.entries[0].listing_id, id);
            assert_eq!(page.entries[0].compatibility, code);
            assert!(page.entries[0].other_build.is_none());
            assert!(r.page(version, false, true).entries.is_empty());
            let page = r.page(version, true, true);
            assert_eq!(page.entries.len(), 1);
            assert!(page.entries[0].other_build.is_some());
        }
    }
    for (version, own, other) in [
        (1, old.listing_id, new.listing_id),
        (2, new.listing_id, old.listing_id),
    ] {
        for (id, visible) in [(own, true), (other, false)] {
            let MasterPacket::ListingDetails(details) = r.reply(
                version,
                addr(42000 + version),
                MasterPacket::Details(Details {
                    nonce: 1,
                    listing_id: id,
                }),
            ) else {
                panic!("details")
            };
            assert_eq!(details.summary.is_some(), visible);
        }
    }
    // An old heartbeat still gets an old acknowledgment after all the new requests.
    let packet = MasterPacket::Heartbeat(Heartbeat {
        token: old.token,
        change: 1,
        candidates: Vec::new(),
        summary: summary(1, "Updated old host"),
    });
    assert!(matches!(
        r.reply(1, addr(40001), packet),
        MasterPacket::HeartbeatAck(_)
    ));
    assert_eq!(r.page(1, false, false).entries[0].name, "Updated old host");
    r.send(
        2,
        addr(40001),
        MasterPacket::Unregister(Unregister { token: old.token }),
    );
    assert_eq!(
        r.master.listings().len(),
        2,
        "wrong-version token cannot remove an old listing"
    );
    r.send(
        1,
        addr(40001),
        MasterPacket::Unregister(Unregister { token: old.token }),
    );
    assert!(r.page(1, true, false).entries.is_empty());
    assert_eq!(r.page(2, true, false).matching, 1);
}

#[test]
fn mixed_introductions_retries_and_relays_keep_their_original_version() {
    let mut r = Rig::new();
    let old = r.register(1, 40001);
    let new = r.register(2, 40002);
    for (version, other) in [(1, new.listing_id), (2, old.listing_id)] {
        let replies = r.introduce(version, addr(43000 + version), other, 1);
        assert_eq!(replies.len(), 1);
        assert_eq!(replies[0].1, version);
        assert!(
            matches!(&replies[0].2,MasterPacket::Introduction(i) if i.result==IntroductionResult::NoListing)
        );
    }
    let mut channels = Vec::new();
    for (version, host, listing) in [(1, addr(40001), old), (2, addr(40002), new)] {
        let player = addr(44000 + version);
        let nonce = 10 + u64::from(version);
        let replies = r.introduce(version, player, listing.listing_id, nonce);
        assert!(replies.iter().all(|(_, v, _)| *v == version));
        assert!(
            replies
                .iter()
                .any(|(to, _, p)| *to == host && matches!(p, MasterPacket::Meet(_)))
        );
        let id = replies
            .iter()
            .find_map(|(_, _, p)| match p {
                MasterPacket::Introduction(i) => Some(i.introduction_id),
                _ => None,
            })
            .unwrap();
        // Force a different request version before the asynchronous retry.
        r.page(if version == 1 { 2 } else { 1 }, false, false);
        r.now += Duration::from_millis(300);
        r.master.update(r.now);
        let retries = r.drain();
        assert!(
            retries.iter().any(|(to, v, p)| *to == host
                && *v == version
                && matches!(p, MasterPacket::Meet(_)))
        );
        let opens = r.send(
            version,
            player,
            MasterPacket::RelayRequest(RelayRequest {
                nonce,
                introduction_id: id,
            }),
        );
        let open = opens
            .iter()
            .find_map(|(to, v, p)| match p {
                MasterPacket::RelayOpen(o) if *to == host => {
                    assert_eq!(*v, version);
                    Some(*o)
                }
                _ => None,
            })
            .unwrap();
        let offers = r.send(
            version,
            host,
            MasterPacket::RelayOpenAck(RelayOpenAck {
                token: listing.token,
                channel: open.channel,
            }),
        );
        assert!(offers.iter().any(|(to, v, p)| *to == player
            && *v == version
            && matches!(p,MasterPacket::RelayOffer(o) if o.result==RelayResult::Open)));
        let frame = MasterPacket::Relay(Relay {
            channel: open.channel,
            key: open.key,
            datagram: vec![1, 2, 3],
        });
        let frames = r.send(version, player, frame.clone());
        assert_eq!(frames, vec![(host, version, frame.clone())]);
        assert!(
            r.send(3 - version, player, frame).is_empty(),
            "wrong-generation relay frame is dropped"
        );
        channels.push((version, host, player, open));
    }
    r.master.stop();
    let closes = r.drain();
    assert_eq!(closes.len(), 12);
    for (version, host, player, open) in channels {
        assert!(
            closes
                .iter()
                .filter(|(to, _, _)| *to == host || *to == player)
                .all(|(_, v, p)| *v == version
                    && matches!(p,MasterPacket::RelayClose(c) if c.channel==open.channel))
        );
    }
}

#[test]
fn registrations_cannot_put_a_game_in_the_other_generation() {
    let mut r = Rig::new();
    for (version, game_version) in [(1, 71), (2, 70)] {
        let mut identity = build(version);
        identity.protocol_version = game_version;
        let mut data = summary(version, "Wrong generation");
        data.protocol_version = game_version;
        let request = Register {
            nonce: 1,
            cookie: 0,
            build: identity,
            dedicated: true,
            telemetry: false,
            install_id: 0,
            platform: 3,
            candidates: Vec::new(),
            summary: data,
        };
        assert!(
            r.send(
                version,
                addr(45000 + version),
                MasterPacket::Register(request)
            )
            .is_empty()
        );
    }
    assert_eq!(r.master.listings().len(), 0);
    for version in [1, 2] {
        let MasterPacket::ProbeAnswer(answer) = r.reply(
            version,
            addr(46000 + version),
            MasterPacket::Probe(Probe { nonce: 4 }),
        ) else {
            panic!("probe")
        };
        assert_eq!(answer.nonce, 4);
    }
}
