//! The relay on the network simulator (slice J3's acceptance in the
//! architecture guide's slice table): the real master, a host that lists
//! itself through its rendezvous and acknowledges Relay opens, and players
//! that ask for an introduction and then the relay through their `Joiner`.
//! Every link is 20 ms one way.

use std::net::SocketAddr;
use std::time::Duration;

use tore_net::master::join::{JoinConfig, JoinEvent, Joiner, RelayState};
use tore_net::master::relay::refusal_text;
use tore_net::master::{
    Build, Candidate, CandidateKind, CloseReason, HostRendezvous, ListingState, ListingSummary,
    MasterPacket, RelayFrame, RelayRequest, RelayResult, Rendezvous,
};
use tore_net::packet::DiscoverPhase;
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_net::{Datagrams, Entropy};

use crate::master::{Master, MasterPort, Settings};
use crate::relay::{GB, RelaySettings};

const MAIN: &str = "198.51.100.1:26901";
const PROBE: &str = "198.51.100.1:26902";
const HOST: &str = "203.0.113.10:26900";
const STEP: Duration = Duration::from_millis(5);

fn a(text: &str) -> SocketAddr {
    text.parse().unwrap()
}

fn build() -> Build {
    Build {
        protocol_version: 9,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        release: true,
    }
}

/// One player: its socket and its join.
struct Player {
    socket: SimSocket,
    joiner: Joiner,
    events: Vec<JoinEvent>,
    /// The game's datagrams that reached it, with where from.
    got: Vec<(SocketAddr, Vec<u8>)>,
}

struct Rig {
    net: SimNetwork,
    master: Master,
    main: SimSocket,
    probe: SimSocket,
    host: Rendezvous,
    host_socket: SimSocket,
    /// The game's datagrams that reached the host's transport.
    host_got: Vec<(SocketAddr, Vec<u8>)>,
    players: Vec<Player>,
    listing_id: u64,
}

impl Rig {
    fn new(settings: Settings) -> Self {
        let net = SimNetwork::new(23);
        net.set_default_link(LinkConfig::one_way(Duration::from_millis(20)));
        net.set_now(Duration::from_secs(100));
        let main = net.bind(a(MAIN)).unwrap();
        let probe = net.bind(a(PROBE)).unwrap();
        let host_socket = net.bind(a(HOST)).unwrap();
        let mut host = Rendezvous::host(
            HostRendezvous {
                build: build(),
                dedicated: true,
                install_id: None,
                platform: 3,
                entropy: Entropy::Seeded(3),
            },
            net.now(),
        );
        host.set_masters(
            vec![a(MAIN)],
            vec![Candidate::new(CandidateKind::Local, a(HOST))],
            net.now(),
        );
        host.set_listed(true, net.now());
        let mut rig = Self {
            master: Master::new(settings, Entropy::Seeded(11), 0),
            net,
            main,
            probe,
            host,
            host_socket,
            host_got: Vec::new(),
            players: Vec::new(),
            listing_id: 0,
        };
        let listed = rig.run_until(Duration::from_secs(3), |r| {
            matches!(r.host.state(), ListingState::Listed { .. })
        });
        assert!(listed, "the host is listed: {:?}", rig.host.state());
        let ListingState::Listed { listing_id, .. } = rig.host.state() else {
            unreachable!()
        };
        rig.listing_id = listing_id;
        rig
    }

    fn now(&self) -> Duration {
        self.net.now()
    }

    /// A player at `address` asks for an introduction; returns its number.
    fn join(&mut self, address: &str) -> usize {
        let now = self.now();
        let mut joiner = Joiner::new(
            JoinConfig {
                build: build(),
                listing_id: self.listing_id,
                entropy: Entropy::Seeded(5 + self.players.len() as u64),
            },
            now,
        );
        joiner.set_masters(
            vec![a(MAIN)],
            vec![Candidate::new(CandidateKind::Local, a(address))],
            now,
        );
        self.players.push(Player {
            socket: self.net.bind(a(address)).unwrap(),
            joiner,
            events: Vec::new(),
            got: Vec::new(),
        });
        let n = self.players.len() - 1;
        let introduced = self.run_until(Duration::from_secs(3), |r| {
            r.players[n].joiner.introduced().is_some()
        });
        assert!(introduced, "{:?}", self.players[n].events);
        n
    }

    /// A player's relay request, and the master's answer: the event.
    fn relay(&mut self, n: usize) -> JoinEvent {
        let now = self.now();
        assert!(self.players[n].joiner.ask_relay(now));
        let answered = self.run_until(Duration::from_secs(5), |r| {
            r.players[n].joiner.relay_state() != RelayState::Asking
        });
        assert!(answered);
        self.players[n].events.pop().expect("an event")
    }

    fn step(&mut self) {
        self.net.advance(STEP);
        let now = self.net.now();
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

        let mut buf = [0u8; 1_201];
        {
            let mut routed = self.host.over(&mut self.host_socket, now);
            while let Some((len, from)) = routed.recv_datagram(&mut buf).unwrap() {
                self.host_got.push((from, buf[..len].to_vec()));
            }
        }
        if self.host.wants_summary(now) {
            self.host.set_summary(
                now,
                ListingSummary {
                    protocol_version: 9,
                    capacity: 8,
                    phase: DiscoverPhase::Lobby,
                    game_version: "0.1.3".into(),
                    game_commit: "abc".into(),
                    name: "Friday night".into(),
                    ..ListingSummary::default()
                },
            );
        }
        self.host.update(now);
        self.host.transmit(&mut self.host_socket).unwrap();

        for player in &mut self.players {
            let mut routed = player.joiner.over(&mut player.socket, now);
            while let Some((len, from)) = routed.recv_datagram(&mut buf).unwrap() {
                player.got.push((from, buf[..len].to_vec()));
            }
            player.joiner.update(now);
            while let Some(event) = player.joiner.poll_event() {
                player.events.push(event);
            }
            player.joiner.transmit(&mut player.socket).unwrap();
        }
    }

    fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&Rig) -> bool) -> bool {
        let end = self.now() + limit;
        while self.now() < end {
            self.step();
            if done(self) {
                return true;
            }
        }
        false
    }

    fn run(&mut self, time: Duration) {
        let end = self.now() + time;
        while self.now() < end {
            self.step();
        }
    }

    /// The player sends a game datagram to the relayed host.
    fn player_sends(&mut self, n: usize, datagram: &[u8]) {
        let now = self.now();
        let player = &mut self.players[n];
        let to = player
            .joiner
            .relayed()
            .unwrap_or_else(|| panic!("a channel: {:?}", player.events));
        player
            .joiner
            .over(&mut player.socket, now)
            .send_datagram(to, datagram)
            .unwrap();
    }

    /// The host sends a game datagram to the relayed player.
    fn host_sends(&mut self, to: SocketAddr, datagram: &[u8]) {
        let now = self.now();
        self.host
            .over(&mut self.host_socket, now)
            .send_datagram(to, datagram)
            .unwrap();
    }

    fn logs(&mut self) -> Vec<String> {
        std::iter::from_fn(|| self.master.poll_log()).collect()
    }
}

fn relayed(event: &JoinEvent) -> SocketAddr {
    match event {
        JoinEvent::Relayed { address } => *address,
        other => panic!("not relayed: {other:?}"),
    }
}

#[test]
fn a_channel_opens_and_carries_the_game_both_ways_unchanged() {
    let mut rig = Rig::new(Settings::default());
    let p = rig.join("192.0.2.50:40000");
    let event = rig.relay(p);
    let address = relayed(&event);
    assert_eq!(rig.master.relays().channels(), 1);
    assert_eq!(rig.host.relayed(), [address]);
    assert!(
        rig.logs()
            .iter()
            .any(|l| l.starts_with("relay opened channel=")
                && l.contains("host=203.0.113.10:26900")
                && l.contains("player=192.0.2.50:40000"))
    );

    rig.player_sends(p, b"connect request");
    rig.run(Duration::from_millis(100));
    assert_eq!(rig.host_got, [(address, b"connect request".to_vec())]);
    rig.host_sends(address, b"challenge");
    rig.run(Duration::from_millis(100));
    // The host's punches (it was introduced) are not the relay's.
    rig.players[p].got.retain(|(from, _)| *from == address);
    assert_eq!(rig.players[p].got, [(address, b"challenge".to_vec())]);
    // A game datagram of the longest kind passes whole: the routers read a
    // frame longer than the transport's buffer into their own.
    let long = vec![7u8; tore_net::MAX_DATAGRAM];
    rig.host_sends(address, &long);
    rig.run(Duration::from_millis(100));
    assert_eq!(rig.players[p].got[1], (address, long));
    let counters = rig.master.relays().counters;
    assert_eq!((counters.opened, counters.frames), (1, 3));
    // The month's figure counts the frames sent out with their headers.
    let frames = (15 + 15) + (15 + 9) + (15 + tore_net::MAX_DATAGRAM as u64);
    assert_eq!(rig.master.relays().month_bytes(), frames + 3 * 28);
    // The player's end closes it: the master tells the host.
    rig.players[p].joiner.close_relay();
    rig.run(Duration::from_millis(100));
    assert_eq!(rig.master.relays().channels(), 0);
    assert!(rig.host.relayed().is_empty());
    assert!(
        rig.logs()
            .iter()
            .any(|l| l.contains("reason=closed by an end"))
    );
}

#[test]
fn a_frame_from_a_third_address_or_with_a_wrong_key_is_dropped() {
    let mut rig = Rig::new(Settings::default());
    let p = rig.join("192.0.2.50:40000");
    let address = relayed(&rig.relay(p));
    let channel = tore_net::master::channel_of(address).unwrap();
    rig.player_sends(p, b"mine");
    rig.run(Duration::from_millis(60));
    assert_eq!(rig.host_got.len(), 1);
    let before = rig.master.relays().dropped;
    // The channel's own key from a third address (someone who read it off
    // the wire): the keepalive wrapper made over the third's socket sends
    // exactly the player's frames.
    let third = rig.net.bind(a("192.0.2.66:5000")).unwrap();
    let mut thief = rig.players[p].joiner.keepalive_socket(third).unwrap();
    thief.send_datagram(address, b"stolen").unwrap();
    // A wrong key, from the player itself.
    let forged = RelayFrame {
        channel,
        key: 12_345,
        datagram: b"forged",
    }
    .encode()
    .unwrap();
    rig.players[p]
        .socket
        .send_datagram(a(MAIN), &forged)
        .unwrap();
    rig.run(Duration::from_millis(100));
    assert_eq!(rig.master.relays().dropped, before + 2);
    assert_eq!(rig.host_got.len(), 1, "only the player's own frame");
    // A real sender that claims a relayed address never reaches the
    // transport.
    rig.net.inject(address, a(HOST), b"claim");
    rig.run(Duration::from_millis(100));
    assert_eq!(rig.host_got.len(), 1);
    assert_eq!(rig.host.counters.relayed_claims, 1);
}

#[test]
fn a_request_needs_its_introduction_from_its_address_with_its_nonce() {
    let mut rig = Rig::new(Settings::default());
    let p = rig.join("192.0.2.50:40000");
    let id = rig.players[p].joiner.introduced().unwrap().introduction_id;
    let mut third = rig.net.bind(a("192.0.2.66:5000")).unwrap();
    for (socket, nonce, intro) in [(0, 0, id), (1, 1, id), (1, 0, id ^ 1)] {
        let request = MasterPacket::RelayRequest(RelayRequest {
            nonce,
            introduction_id: intro,
        })
        .encode()
        .unwrap();
        let s = if socket == 0 {
            &mut third
        } else {
            &mut rig.players[p].socket
        };
        s.send_datagram(a(MAIN), &request).unwrap();
        rig.run(Duration::from_millis(60));
    }
    // The third address, a wrong nonce, no such introduction: nothing.
    assert_eq!(rig.master.relays().dropped, 3);
    assert_eq!(rig.master.relays().channels(), 0);
}

/// John, 2026-10-06 (Q55), slice R1: 128 KB/s each way per channel. D12
/// measured a relayed player's busiest second at 63 to 74 KB/s at 60
/// snapshots a second, so the old 64 cut it; 128 leaves room for twice that.
#[test]
fn the_default_rate_is_128_kb_a_second_each_way() {
    assert_eq!(RelaySettings::default().rate_kb, 128);
    assert_eq!(Settings::default().relay.rate_kb, 128);
}

#[test]
fn a_channel_flooded_at_200_kb_a_second_passes_128() {
    let mut rig = Rig::new(Settings::default());
    let p = rig.join("192.0.2.50:40000");
    relayed(&rig.relay(p));
    // 1,000-byte datagrams (1,015-byte frames) every 5 ms: 203 KB/s.
    let start = rig.now();
    let mut counted_from = None;
    while rig.now() < start + Duration::from_secs(12) {
        rig.player_sends(p, &[1u8; 1_000]);
        rig.step();
        if counted_from.is_none() && rig.now() >= start + Duration::from_secs(4) {
            counted_from = Some(rig.host_got.len());
        }
    }
    let total_frames = rig.host_got.len() as f64 * 1_015.0;
    let steady = (rig.host_got.len() - counted_from.unwrap()) as f64 * 1_015.0 / 8.0;
    assert!(
        (126_000.0..=130_000.0).contains(&steady),
        "{steady} B/s after the burst"
    );
    // The first seconds add the burst of 256 KB.
    assert!(
        total_frames <= 128_000.0 * 12.0 + 256_000.0 + 2_000.0,
        "{total_frames}"
    );
    assert!(rig.master.relays().counters.over_rate > 500);
    assert_eq!(rig.master.relays().channels(), 1, "still open after 12 s");
    // Over its rate in every second for 30 seconds closes it.
    while rig.now() < start + Duration::from_secs(45) && rig.players[p].joiner.relayed().is_some() {
        rig.player_sends(p, &[1u8; 1_000]);
        rig.step();
    }
    // The first frame over the rate comes once the burst of 256 KB has
    // drained at the 75 KB/s of excess, about 3.4 seconds in.
    let closed_at = rig.now() - start;
    assert!(
        (Duration::from_secs(33)..=Duration::from_secs(35)).contains(&closed_at),
        "{closed_at:?}"
    );
    assert_eq!(rig.master.relays().channels(), 0);
    assert!(
        rig.players[p]
            .events
            .contains(&JoinEvent::RelayClosed(CloseReason::OverRate))
    );
    assert!(
        rig.logs()
            .iter()
            .any(|l| l.contains("reason=over its rate"))
    );
}

#[test]
fn an_idle_channel_closes_at_30_seconds_and_both_ends_are_told() {
    let mut rig = Rig::new(Settings::default());
    let p = rig.join("192.0.2.50:40000");
    relayed(&rig.relay(p));
    rig.player_sends(p, b"last");
    let last = rig.now();
    rig.run_until(Duration::from_secs(40), |r| {
        r.master.relays().channels() == 0
    });
    let closed_after = rig.now() - last;
    assert!(
        (Duration::from_secs(30)..=Duration::from_millis(30_100)).contains(&closed_after),
        "{closed_after:?}"
    );
    rig.run(Duration::from_millis(100));
    assert!(rig.host.relayed().is_empty());
    assert_eq!(
        rig.players[p].events.last(),
        Some(&JoinEvent::RelayClosed(CloseReason::Idle))
    );
    assert_eq!(rig.players[p].joiner.relayed(), None);
}

#[test]
fn refusals_say_why_in_the_players_words() {
    let cases: [(RelaySettings, RelayResult); 3] = [
        (
            RelaySettings {
                on: false,
                ..RelaySettings::default()
            },
            RelayResult::Off,
        ),
        (
            RelaySettings {
                channels: 0,
                ..RelaySettings::default()
            },
            RelayResult::Full,
        ),
        (
            RelaySettings {
                month_gb: 0,
                ..RelaySettings::default()
            },
            RelayResult::AllowanceSpent,
        ),
    ];
    for (relay, result) in cases {
        let mut rig = Rig::new(Settings {
            relay,
            ..Settings::default()
        });
        let p = rig.join("192.0.2.50:40000");
        assert_eq!(
            rig.relay(p),
            JoinEvent::RelayRefused {
                result,
                text: refusal_text(result).into()
            }
        );
    }
}

#[test]
fn two_channels_per_player_address_and_a_silent_host_is_said() {
    let mut rig = Rig::new(Settings::default());
    // Three players behind one address (one source): the third is refused.
    let mut players: Vec<usize> = (0..2)
        .map(|i| rig.join(&format!("192.0.2.50:{}", 40_000 + i)))
        .collect();
    relayed(&rig.relay(players[0]));
    relayed(&rig.relay(players[1]));
    // The source may ask twice a minute: a minute on, both channels kept
    // busy, the third asks.
    let wait = rig.now() + Duration::from_secs(61);
    while rig.now() < wait {
        for &p in &players[..2] {
            rig.player_sends(p, b"busy");
        }
        rig.run(Duration::from_secs(1));
    }
    players.push(rig.join("192.0.2.50:40002"));
    assert_eq!(rig.master.relays().channels(), 2);
    assert_eq!(
        rig.relay(players[2]),
        JoinEvent::RelayRefused {
            result: RelayResult::TooMany,
            text: refusal_text(RelayResult::TooMany).into()
        }
    );
    // A host that stopped listing never acknowledges: three Relay opens,
    // then the player is told.
    let q = rig.join("192.0.2.77:40000");
    let now = rig.now();
    rig.host.set_listed(false, now);
    assert_eq!(
        rig.relay(q),
        JoinEvent::RelayRefused {
            result: RelayResult::HostSilent,
            text: refusal_text(RelayResult::HostSilent).into()
        }
    );
}

#[test]
fn a_spent_allowance_refuses_new_channels_and_closes_open_ones() {
    let mut rig = Rig::new(Settings {
        relay: RelaySettings {
            month_gb: 1,
            ..RelaySettings::default()
        },
        ..Settings::default()
    });
    let p = rig.join("192.0.2.50:40000");
    // Just under 95 percent: the channel opens.
    rig.master
        .relays_mut()
        .resume_month((2026, 10), 95 * GB / 100 - 1);
    relayed(&rig.relay(p));
    // A frame crosses 95 percent: new channels are refused.
    rig.player_sends(p, b"over");
    rig.run(Duration::from_millis(60));
    let q = rig.join("192.0.2.77:40000");
    assert_eq!(
        rig.relay(q),
        JoinEvent::RelayRefused {
            result: RelayResult::AllowanceSpent,
            text: "The Internet Lobby's relay is full for this month.".into()
        }
    );
    assert!(
        rig.logs()
            .iter()
            .any(|l| l.contains("95 percent of 1 GB relayed this month"))
    );
    // At 100 percent the open channel closes.
    rig.master.relays_mut().resume_month((2026, 10), GB);
    rig.run(Duration::from_millis(100));
    assert_eq!(rig.master.relays().channels(), 0);
    assert_eq!(
        rig.players[p].events.last(),
        Some(&JoinEvent::RelayClosed(CloseReason::AllowanceSpent))
    );
}

#[test]
fn a_stopping_master_closes_every_channel() {
    let mut rig = Rig::new(Settings::default());
    let p = rig.join("192.0.2.50:40000");
    relayed(&rig.relay(p));
    rig.master.stop();
    rig.master
        .transmit(&mut rig.main, Some(&mut rig.probe))
        .unwrap();
    rig.run(Duration::from_millis(100));
    assert_eq!(
        rig.players[p].events.last(),
        Some(&JoinEvent::RelayClosed(CloseReason::Stopping))
    );
    assert!(rig.host.relayed().is_empty());
}

/// The month's figure is read back when the master starts again, so a
/// spent allowance stays spent across a restart, and written when it stops.
#[test]
fn the_months_figure_survives_a_restart() {
    use crate::Config;
    use crate::log::{self, StateFiles};
    use crate::run::Running;
    use tore_net::Listen;

    let dir = std::env::temp_dir().join(format!(
        "tore-master-relay-test-{}-{}",
        std::process::id(),
        log::unix_seconds()
    ));
    let files = StateFiles::open(&dir).unwrap();
    let month = log::month_of(log::day_of(log::unix_seconds()));
    files.write_relay_month(month, GB).unwrap();
    let mut config = Config::defaults(&dir);
    config.listen = Listen::Address(std::net::Ipv4Addr::LOCALHOST.into());
    config.port = 0;
    config.probe_port = 0;
    config.settings.relay.month_gb = 1;
    let (mut running, notes) = Running::bind(config, Entropy::Seeded(1), true).unwrap();
    let relays = running.master().relays();
    assert_eq!((relays.month(), relays.month_bytes()), (Some(month), GB));
    assert!(
        notes
            .iter()
            .any(|n| n.contains("relay this month") && n.contains("of 1 GB")),
        "{notes:?}"
    );
    // A request now is refused for the allowance (the simulator test above
    // shows the player's words), and stopping writes the figure again.
    std::fs::remove_file(files.relay_path(month)).unwrap();
    let mut out = Vec::new();
    running.finish(&mut out).unwrap();
    assert_eq!(files.read_relay_month(month), Some(GB));
    std::fs::remove_dir_all(&dir).unwrap();
}
