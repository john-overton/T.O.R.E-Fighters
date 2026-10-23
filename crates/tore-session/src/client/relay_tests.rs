//! The relay with real sessions on the network simulator (slice J3's
//! acceptance): a host listed on the real master through its rendezvous,
//! two bots that join through the master's relay and one that joins
//! directly, flying the network matrix's fight under its limits; and a
//! relayed bot whose game stalls, kept by keepalives framed for its channel.
//! Synthetic resources.
//!
//! Every link has the same delay and loss, so a relayed bot's path (to the
//! master, then on to the host) has twice the direct bot's round trip and
//! about twice its loss: the relay adds its delay and nothing else.

use std::collections::BTreeMap;

use super::matrix_tests::{Cell, check, fly};
use super::tests::{Player, Rig, bot_script, host_address, level_script, spec};
use super::*;
use crate::host::HostLog;
use tore_master::{Master, MasterPort, Settings};
use tore_net::master::join::{JoinConfig, JoinEvent, Joiner};
use tore_net::master::{
    Build, Candidate, CandidateKind, HostRendezvous, ListingState, Rendezvous, is_relayed,
};
use tore_net::sim::{LinkConfig, SimSocket};
use tore_net::{Datagrams, Entropy};

const MASTER: &str = "10.0.0.9:26901";
const PROBE: &str = "10.0.0.9:26902";
const MS: Duration = Duration::from_millis(1);

/// A player's script, as the rig's.
type Script = Box<dyn FnMut(Duration, &Client, Option<&RenderSnapshot>) -> Controls>;

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

/// The master protocol's build: the rig's own.
fn master_build() -> Build {
    let build = super::tests::build();
    Build {
        protocol_version: crate::wire::PROTOCOL_VERSION,
        game_version: build.version,
        game_commit: build.commit,
        release: build.release,
    }
}

/// The master, the host's rendezvous and the relayed players' joiners
/// beside a [`Rig`].
struct Relay {
    master: Master,
    main: SimSocket,
    probe: SimSocket,
    rendezvous: Rendezvous,
    /// Each relayed player's joiner, by its number in the rig.
    joiners: BTreeMap<usize, Joiner>,
    /// Each stalled relayed player's keepalive thread, as the game runs it:
    /// when it last sent.
    keepalives: BTreeMap<usize, Option<Duration>>,
}

/// A socket by reference, for the keepalive wrapper.
struct ByRef<'a>(&'a mut SimSocket);

impl Datagrams for ByRef<'_> {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> std::io::Result<()> {
        self.0.send_datagram(to, datagram)
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> std::io::Result<Option<(usize, SocketAddr)>> {
        self.0.recv_datagram(buf)
    }
}

impl Relay {
    /// A master beside `rig`'s host, the host listed on it.
    fn new(rig: &mut Rig) -> Self {
        let now = rig.net.now();
        let mut rendezvous = Rendezvous::host(
            HostRendezvous {
                build: master_build(),
                dedicated: true,
                install_id: None,
                platform: 3,
                entropy: Entropy::Seeded(3),
            },
            now,
        );
        rendezvous.set_masters(
            vec![a(MASTER)],
            vec![Candidate::new(CandidateKind::Local, host_address())],
            now,
        );
        rendezvous.set_listed(true, now);
        let mut relay = Self {
            master: Master::new(Settings::default(), Entropy::Seeded(11), 0),
            main: rig.net.bind(a(MASTER)).unwrap(),
            probe: rig.net.bind(a(PROBE)).unwrap(),
            rendezvous,
            joiners: BTreeMap::new(),
            keepalives: BTreeMap::new(),
        };
        let end = now + Duration::from_secs(3);
        while !matches!(relay.rendezvous.state(), ListingState::Listed { .. }) {
            assert!(rig.net.now() < end, "{:?}", relay.rendezvous.state());
            relay.step(rig);
        }
        relay
    }

    /// A bot at `address` that joins through the relay: introduced, the
    /// relay asked for at once (as `tore-bot --path relay`), then the
    /// ordinary join to the channel's relayed address. Its number.
    fn join(&mut self, rig: &mut Rig, address: &str, callsign: &str, script: Script) -> usize {
        let mut socket = rig.net.bind(a(address)).unwrap();
        let listing_id = match self.rendezvous.state() {
            ListingState::Listed { listing_id, .. } => listing_id,
            other => panic!("{other:?}"),
        };
        let now = rig.net.now();
        let mut joiner = Joiner::new(
            JoinConfig {
                build: master_build(),
                listing_id,
                entropy: Entropy::Seeded(u64::from(socket.local_addr().port())),
            },
            now,
        );
        joiner.set_masters(
            vec![a(MASTER)],
            vec![Candidate::new(CandidateKind::Local, a(address))],
            now,
        );
        let end = now + Duration::from_secs(8);
        let relayed = loop {
            assert!(rig.net.now() < end, "the relay opened for {callsign}");
            self.step(rig);
            let now = rig.net.now();
            let mut buf = [0u8; 2048];
            // Before the client, only the master's datagrams matter.
            while joiner
                .over(&mut socket, now)
                .recv_datagram(&mut buf)
                .unwrap()
                .is_some()
            {}
            joiner.update(now);
            let mut opened = None;
            while let Some(event) = joiner.poll_event() {
                match event {
                    JoinEvent::Introduced(_) => assert!(joiner.ask_relay(now)),
                    JoinEvent::Relayed { address } => opened = Some(address),
                    JoinEvent::MappingTested(_) => {}
                    other => panic!("{callsign}: {other:?}"),
                }
            }
            joiner.transmit(&mut socket).unwrap();
            if let Some(address) = opened {
                break address;
            }
        };
        let config = ClientConfig {
            entropy: Entropy::Seeded(u64::from(socket.local_addr().port()) + 7),
            callsign: callsign.into(),
            ..ClientConfig::new(relayed, "Viper", super::tests::build())
        };
        let client = Client::connect(config, Arc::clone(&rig.resources), rig.net.now()).unwrap();
        rig.players.push(Player {
            socket,
            client,
            script,
            events: Vec::new(),
            frames: Vec::new(),
            keep_frames: false,
            frame_every: Duration::from_millis(16),
            last_frame: None,
            digests: Vec::new(),
            picture: None,
            stalled: false,
        });
        let n = rig.players.len() - 1;
        self.joiners.insert(n, joiner);
        n
    }

    /// One millisecond for everyone, as [`Rig::step`], with the master
    /// beside them: the host reads and sends through its rendezvous, each
    /// relayed player through its joiner, and a stalled relayed player's
    /// keepalive thread sends its keepalive once a second, framed for its
    /// channel.
    fn step(&mut self, rig: &mut Rig) {
        rig.net.advance(MS);
        let now = rig.net.now();
        self.master
            .receive_from(now, MasterPort::Main, &mut self.main)
            .unwrap();
        self.master
            .receive_from(now, MasterPort::Probe, &mut self.probe)
            .unwrap();
        self.master.update(now);
        self.master
            .transmit(&mut self.main, Some(&mut self.probe))
            .unwrap();

        let before = rig.host.world().tick();
        rig.host
            .receive_from(now, &mut self.rendezvous.over(&mut rig.socket, now))
            .unwrap();
        rig.host.update(now);
        if self.rendezvous.wants_summary(now) {
            self.rendezvous
                .set_summary(now, rig.host.discover_answer(0).into());
        }
        self.rendezvous.update(now);
        rig.host
            .transmit(&mut self.rendezvous.over(&mut rig.socket, now))
            .unwrap();
        self.rendezvous.transmit(&mut rig.socket).unwrap();
        while let Some(log) = rig.host.poll_log() {
            rig.logs.push(log);
        }
        if rig.watch && rig.host.world().tick() > before {
            record(rig);
        }

        for (n, player) in rig.players.iter_mut().enumerate() {
            let mut joiner = self.joiners.get_mut(&n);
            if player.stalled {
                // The game's loop is held up; its keepalive thread speaks.
                if let (Some(joiner), Some(last)) = (joiner, self.keepalives.get_mut(&n)) {
                    let quiet = last.is_none_or(|at| now >= at + Duration::from_secs(1));
                    if quiet && let Some(keepalive) = player.client.keepalive_datagram() {
                        let to = player.client.server();
                        let mut framing = joiner
                            .keepalive_socket(ByRef(&mut player.socket))
                            .expect("an open channel");
                        framing.send_datagram(to, &keepalive).unwrap();
                        *last = Some(now);
                    }
                }
                continue;
            }
            match joiner.as_deref_mut() {
                Some(j) => player
                    .client
                    .receive_from(now, &mut j.over(&mut player.socket, now))
                    .unwrap(),
                None => player.client.receive_from(now, &mut player.socket).unwrap(),
            }
            let controls = (player.script)(now, &player.client, player.picture.as_ref());
            player.client.update(now, &controls);
            if player
                .last_frame
                .is_none_or(|last| now - last >= player.frame_every)
            {
                player.last_frame = Some(now);
                if let Some(frame) = player.client.frame(now) {
                    player.digests.push(frame.digest());
                    player.picture = Some(frame.picture.clone());
                }
            }
            match joiner {
                Some(j) => {
                    player
                        .client
                        .transmit(&mut j.over(&mut player.socket, now))
                        .unwrap();
                    j.update(now);
                    j.transmit(&mut player.socket).unwrap();
                }
                None => player.client.transmit(&mut player.socket).unwrap(),
            }
            while let Some(event) = player.client.poll_event() {
                player.events.push(event);
            }
        }
    }
}

/// Where every aircraft is after the host's last tick (as the rig records
/// it when watching).
fn record(rig: &mut Rig) {
    let world = rig.host.world();
    let tick = world.tick() - 1;
    for cockpit in &world.cockpits {
        rig.truth
            .insert((tick, cockpit.plane.0), cockpit.flight.position);
    }
    if let Some(wings) = &world.ai_wings {
        for actor in wings.mission().actors() {
            rig.truth
                .insert((tick, actor.id()), actor.flight().position);
        }
    }
}

/// Two bots through the relay and one direct fly the matrix's 60 seconds:
/// every limit of the stage D acceptance table holds for each of them, the
/// relayed ones on a path of twice the round trip and loss (the 150 ms and
/// 2 percent cell), and the host knows the two by their relayed addresses.
#[test]
fn two_relayed_bots_and_a_direct_one_fly_within_the_matrix_limits() {
    // 75 ms and 1 percent each way on every link: 150 ms and about 2
    // percent through the relay.
    let link = LinkConfig::for_round_trip(Duration::from_millis(75), 0.1, 0.01, 0.01);
    let mut rig = Rig::new(spec(4, 4, 1), link, 1_152);
    rig.watch = true;
    let mut relay = Relay::new(&mut rig);
    let direct = rig.join(|c| c.callsign = "Alpha".into(), bot_script());
    let bravo = relay.join(&mut rig, "10.0.0.3:40000", "Bravo", bot_script());
    let charlie = relay.join(&mut rig, "10.0.0.4:40000", "Charlie", bot_script());
    let bots = [direct, bravo, charlie];
    let cell = Cell {
        round_trip_ms: 150,
        loss_percent: 2,
    };
    let figures = fly(&mut rig, &bots, cell, 60, &mut |rig| relay.step(rig));
    check(cell, &figures);
    // The direct bot's own round trip is half the relayed ones'.
    let rtt = |n: usize| figures[n].round_trip.as_secs_f64() * 1_000.;
    assert!(rtt(0) < 110., "direct {} ms", rtt(0));
    for n in [1, 2] {
        assert!((130. ..230.).contains(&rtt(n)), "relayed {} ms", rtt(n));
    }
    for &n in &[bravo, charlie] {
        assert!(is_relayed(rig.players[n].client.server()));
        assert_eq!(rig.players[n].client.path(), tore_net::master::Path::Relay);
    }
    let counters = relay.master.relays().counters;
    assert_eq!(counters.opened, 2);
    assert!(counters.bytes > 1_000_000, "{counters:?}");
}

/// A relayed bot whose game stalls for 15 seconds stays connected: its
/// keepalive thread's packets go out framed for the channel and reach the
/// host from the relayed address, which logs the stall and its end.
#[test]
fn a_relayed_bot_stalled_15_seconds_stays_connected_through_framed_keepalives() {
    let mut rig = Rig::new(
        spec(2, 0, 20),
        LinkConfig::for_round_trip(Duration::from_millis(20), 0., 0., 0.),
        23,
    );
    // The keepalives are under test, not the AI's idle rule (slice F2-A),
    // which would take a plane stalled for ten seconds.
    rig.host
        .settings_for_test()
        .apply(&[(crate::settings::number::IDLE_AI, 0)])
        .unwrap();
    let mut relay = Relay::new(&mut rig);
    let player = relay.join(&mut rig, "10.0.0.3:40000", "Viper", level_script());
    let end = rig.net.now() + Duration::from_secs(5);
    while !rig.seated(player) {
        assert!(rig.net.now() < end, "{:?}", rig.players[player].events);
        relay.step(&mut rig);
    }
    for _ in 0..2_000 {
        relay.step(&mut rig);
    }
    let logs_before = rig.logs.len();
    rig.players[player].stalled = true;
    relay.keepalives.insert(player, None);
    let framed_before = relay.joiners[&player].relay_counters().frames_out;
    let forwarded_before = relay.master.relays().counters.frames;
    for _ in 0..15_000 {
        relay.step(&mut rig);
    }
    // The keepalives went through the master (the wrapper frames them
    // itself, so the joiner's own counts do not move).
    assert_eq!(
        relay.joiners[&player].relay_counters().frames_out,
        framed_before
    );
    let forwarded = relay.master.relays().counters.frames - forwarded_before;
    assert!(
        forwarded >= 14,
        "{forwarded} frames forwarded during the stall"
    );
    rig.players[player].stalled = false;
    relay.keepalives.remove(&player);
    for _ in 0..2_000 {
        relay.step(&mut rig);
    }
    assert!(!rig.closed(player), "{:?}", rig.players[player].events);
    let lines: Vec<String> = rig.logs[logs_before..]
        .iter()
        .filter_map(HostLog::stall_text)
        .collect();
    assert_eq!(lines.len(), 2, "{lines:?}");
    assert_eq!(lines[0], "seat 0 Viper: game stalled, flying neutral");
    assert!(
        lines[1].starts_with("seat 0 Viper: game back after 15."),
        "{lines:?}"
    );
    // And it leaves cleanly, through the relay.
    let now = rig.net.now();
    rig.players[player].client.leave_game(now);
    let end = now + Duration::from_secs(12);
    while !rig.closed(player) {
        assert!(rig.net.now() < end);
        relay.step(&mut rig);
    }
    assert!(rig.players[player].events.iter().any(|e| matches!(
        e,
        ClientEvent::Closed(CloseReason::Disconnected {
            reason: DisconnectReason::Left,
            by_peer: false
        })
    )));
}
