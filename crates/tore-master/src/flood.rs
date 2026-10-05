//! `tore-master flood TARGET SECONDS`: a load tool an operator points at a
//! master they run, to see its limits hold ("Running it" in the operations
//! guide).
//!
//! It sends every kind of request a game may send (and malformed datagrams
//! and an unsupported version beside them) from many ports of this machine,
//! as fast as its rate allows, and counts what comes back. Meanwhile a
//! proper browser asks for the list once a second from an address of its
//! own, when the machine has one: on a loopback target it uses 127.0.0.2,
//! which Linux answers for, so the master sees it as another source.
//! Against a master elsewhere every port of this machine is one source, so
//! the browser's check is skipped and said so.
//!
//! The limits held when no port got more bytes back than it sent, and the
//! proper browser was answered at least half the time. Nothing it sends can
//! make a listing (it never answers a Challenge), and its Reports carry the
//! game version `flood`, which the master does not count.

use std::collections::BTreeMap;
use std::io::{self, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::{Duration, Instant};

use tore_net::master::packet::{MAX_MASTER_DATAGRAM, peek_kind};
use tore_net::master::{
    Browse, Build, CloseReason, Details, Heartbeat, Introduce, Keep, ListingSummary, MappingType,
    MasterPacket, Path, PortMapping, Probe, Register, Relay, RelayClose, RelayRequest, Report,
    Role, Unregister,
};
use tore_net::{Datagrams, SplitMix64, bind_udp};

use crate::telemetry::FLOOD_VERSION;

/// How the flood runs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FloodOptions {
    /// The master's main port.
    pub target: SocketAddr,
    /// How long to send.
    pub seconds: f64,
    /// Datagrams a second, all ports together.
    pub rate: u32,
    /// How many ports of this machine send.
    pub ports: usize,
    /// The seed of the random fields.
    pub seed: u64,
}

impl FloodOptions {
    /// The defaults for a target: 2,000 datagrams a second from 32 ports.
    pub fn new(target: SocketAddr, seconds: f64) -> Self {
        Self {
            target,
            seconds,
            rate: 2_000,
            ports: 32,
            seed: 1,
        }
    }
}

/// What the flood saw.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct FloodReport {
    /// Datagrams sent, by what they were.
    pub sent: BTreeMap<&'static str, u64>,
    /// Bytes sent.
    pub bytes_sent: u64,
    /// Answers received, by kind.
    pub answers: BTreeMap<String, u64>,
    /// Bytes received.
    pub bytes_answered: u64,
    /// Ports that got more bytes back than they sent.
    pub over_answered_ports: usize,
    /// The largest share of its own bytes one port got back.
    pub largest_share: f64,
    /// The proper browser: asked, answered; `None` when it was skipped.
    pub browser: Option<(u32, u32)>,
    /// Why the browser was skipped.
    pub browser_note: Option<String>,
}

impl FloodReport {
    /// True when no port was answered with more than it sent and the proper
    /// browser, if it ran, was answered at least half the time.
    pub fn held(&self) -> bool {
        self.over_answered_ports == 0
            && self
                .browser
                .is_none_or(|(asked, answered)| asked > 0 && answered * 2 >= asked)
    }

    /// The lines the tool prints.
    pub fn lines(&self, options: &FloodOptions) -> Vec<String> {
        let list = |map: &mut dyn Iterator<Item = (String, u64)>| {
            map.map(|(k, v)| format!("{k} {v}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let sent: u64 = self.sent.values().sum();
        let answered: u64 = self.answers.values().sum();
        let mut lines = vec![
            format!(
                "flood {} for {} s from {} ports at {} datagrams a second",
                options.target, options.seconds, options.ports, options.rate
            ),
            format!(
                "sent {sent} datagrams ({} bytes): {}",
                self.bytes_sent,
                list(&mut self.sent.iter().map(|(k, v)| (k.to_string(), *v)))
            ),
            format!(
                "answered {answered} ({} bytes): {}",
                self.bytes_answered,
                if self.answers.is_empty() {
                    "nothing".into()
                } else {
                    list(&mut self.answers.iter().map(|(k, v)| (k.clone(), *v)))
                }
            ),
            format!(
                "ports answered with more bytes than they sent: {} (the largest share of its own bytes one port got back: {:.3})",
                self.over_answered_ports, self.largest_share
            ),
        ];
        lines.push(match (self.browser, &self.browser_note) {
            (Some((asked, answered)), _) => {
                format!(
                    "a browse from another address during the flood: answered {answered} of {asked}"
                )
            }
            (None, Some(note)) => {
                format!("a browse from another address during the flood: skipped, {note}")
            }
            (None, None) => "a browse from another address during the flood: skipped".into(),
        });
        lines.push(if self.held() {
            "limits held".into()
        } else {
            "limits NOT held".into()
        });
        lines
    }
}

/// The kinds the flood sends, in turn.
const KINDS: [&str; 16] = [
    "register",
    "register-bad-cookie",
    "heartbeat",
    "keep",
    "unregister",
    "browse",
    "details",
    "probe",
    "probe-second-port",
    "introduce",
    "relay-request",
    "relay",
    "relay-close",
    "report",
    "garbage",
    "unsupported-version",
];

fn build() -> Build {
    Build {
        protocol_version: 0,
        game_version: FLOOD_VERSION.into(),
        game_commit: "flood".into(),
        release: false,
    }
}

/// One datagram of `kind`, with random fields; and where it goes.
fn datagram(kind: &str, rng: &mut SplitMix64, target: SocketAddr) -> (SocketAddr, Vec<u8>) {
    let n = rng.next_u64();
    let summary = ListingSummary {
        name: "flood".into(),
        ..ListingSummary::default()
    };
    let packet = match kind {
        "register" | "register-bad-cookie" => MasterPacket::Register(Register {
            nonce: n,
            cookie: if kind == "register" {
                0
            } else {
                rng.next_u64() | 1
            },
            build: build(),
            dedicated: false,
            telemetry: false,
            install_id: 0,
            platform: 0,
            candidates: Vec::new(),
            summary,
        }),
        "heartbeat" => MasterPacket::Heartbeat(Heartbeat {
            token: n,
            change: 0,
            candidates: Vec::new(),
            summary,
        }),
        "keep" => MasterPacket::Keep(Keep { token: n }),
        "unregister" => MasterPacket::Unregister(Unregister { token: n }),
        "browse" => MasterPacket::Browse(Browse {
            nonce: n,
            build: build(),
            other_builds: true,
            full_games: true,
            cursor: 0,
        }),
        "details" => MasterPacket::Details(Details {
            nonce: n,
            listing_id: rng.next_u64(),
        }),
        "probe" | "probe-second-port" => MasterPacket::Probe(Probe { nonce: n }),
        "introduce" => MasterPacket::Introduce(Introduce {
            nonce: n,
            cookie: 0,
            listing_id: rng.next_u64(),
            build: build(),
            mapping: MappingType::Unknown,
            candidates: Vec::new(),
        }),
        "relay-request" => MasterPacket::RelayRequest(RelayRequest {
            nonce: n,
            introduction_id: rng.next_u64(),
        }),
        "relay" => MasterPacket::Relay(Relay {
            channel: n as u32,
            key: (n >> 32) as u32,
            datagram: vec![0x5a; 1_200],
        }),
        "relay-close" => MasterPacket::RelayClose(RelayClose {
            channel: n as u32,
            key: (n >> 32) as u32,
            reason: CloseReason::Closed,
        }),
        "report" => MasterPacket::Report(Report {
            install_id: n | 1,
            role: Role::Player,
            game_version: FLOOD_VERSION.into(),
            platform: 0,
            minutes: 0,
            humans: 0,
            path: Path::Relay,
            connect_tenths: 0,
            mapping: MappingType::Unknown,
            port_mapping: PortMapping::NotTried,
            relayed_kb: 0,
            players_by_path: [0; 6],
            migrations: 0,
            failed_migrations: 0,
        }),
        "garbage" => {
            let len = 1 + rng.below(1_300) as usize;
            let bytes = (0..len).map(|_| rng.next_u64() as u8).collect();
            return (target, bytes);
        }
        _ => {
            // A Browse in a version no master speaks: answered with
            // Unsupported, never longer than it.
            let browse = MasterPacket::Browse(Browse {
                nonce: n,
                build: build(),
                other_builds: false,
                full_games: false,
                cursor: 0,
            });
            let version = 0xff00 | (n as u16 & 0xff);
            return (target, browse.encode_in(version).unwrap_or_default());
        }
    };
    let to = if kind == "probe-second-port" {
        SocketAddr::new(target.ip(), target.port().wrapping_add(1))
    } else {
        target
    };
    (to, packet.encode().expect("the flood's packets encode"))
}

/// The address the proper browser asks from, for a loopback target.
fn browser_address(target: SocketAddr) -> Result<SocketAddr, String> {
    match target.ip() {
        IpAddr::V4(v4) if v4.is_loopback() => {
            let other = Ipv4Addr::new(127, 0, 0, 2);
            if v4 == other {
                Ok(SocketAddr::new(Ipv4Addr::new(127, 0, 0, 3).into(), 0))
            } else {
                Ok(SocketAddr::new(other.into(), 0))
            }
        }
        _ => Err("every port of this machine is one source to a master elsewhere".into()),
    }
}

fn unspecified(target: SocketAddr) -> SocketAddr {
    match target {
        SocketAddr::V4(_) => SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0),
        SocketAddr::V6(_) => SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), 0),
    }
}

/// The name of an answer's kind.
fn answer_name(datagram: &[u8]) -> String {
    match MasterPacket::decode(datagram) {
        Ok(packet) => format!("{:?}", packet.kind()).to_lowercase(),
        Err(_) => match peek_kind(datagram) {
            Some(kind) => format!("{kind:?} (undecoded)").to_lowercase(),
            None => "unknown".into(),
        },
    }
}

/// Runs the flood and gives what it saw; prints a progress line a second.
pub fn flood(options: FloodOptions, out: &mut dyn Write) -> io::Result<FloodReport> {
    let mut sockets = Vec::with_capacity(options.ports);
    for _ in 0..options.ports.max(1) {
        sockets.push(bind_udp(unspecified(options.target))?);
    }
    let mut sent_bytes = vec![0u64; sockets.len()];
    let mut got_bytes = vec![0u64; sockets.len()];
    let mut report = FloodReport::default();
    let (mut browser, note) = match browser_address(options.target).and_then(|address| {
        bind_udp(address).map_err(|e| format!("{address} could not be bound ({e})"))
    }) {
        Ok(socket) => (Some(socket), None),
        Err(note) => (None, Some(note)),
    };
    report.browser_note = note;
    let mut browser_asked = 0u32;
    let mut browser_answered = 0u32;
    let mut browser_nonce = None;
    let mut next_browse = Instant::now() + Duration::from_millis(500);

    let mut rng = SplitMix64::new(options.seed);
    let start = Instant::now();
    let length = Duration::from_secs_f64(options.seconds.max(0.0));
    let settle = Duration::from_secs(1);
    let mut due = 0f64;
    let mut last = start;
    let mut turn = 0usize;
    let mut next_progress = start + Duration::from_secs(1);
    let mut buf = [0u8; MAX_MASTER_DATAGRAM + 1];
    loop {
        let now = Instant::now();
        let elapsed = now - start;
        if elapsed >= length + settle {
            break;
        }
        if elapsed < length {
            due += (now - last).as_secs_f64() * f64::from(options.rate);
            while due >= 1.0 {
                due -= 1.0;
                let kind = KINDS[turn % KINDS.len()];
                let port = (turn / KINDS.len()) % sockets.len();
                turn += 1;
                let (to, bytes) = datagram(kind, &mut rng, options.target);
                if sockets[port].send_datagram(to, &bytes).is_ok() {
                    *report.sent.entry(kind).or_default() += 1;
                    sent_bytes[port] += bytes.len() as u64;
                }
            }
            if let Some(socket) = &mut browser
                && now >= next_browse
            {
                next_browse = now + Duration::from_secs(1);
                let nonce = rng.next_u64();
                let browse = MasterPacket::Browse(Browse {
                    nonce,
                    build: build(),
                    other_builds: true,
                    full_games: true,
                    cursor: 0,
                });
                if let Ok(bytes) = browse.encode() {
                    socket.send_datagram(options.target, &bytes)?;
                    browser_asked += 1;
                    browser_nonce = Some(nonce);
                }
            }
        }
        last = now;
        for (port, socket) in sockets.iter_mut().enumerate() {
            while let Some((len, _)) = socket.recv_datagram(&mut buf)? {
                got_bytes[port] += len as u64;
                *report.answers.entry(answer_name(&buf[..len])).or_default() += 1;
            }
        }
        if let Some(socket) = &mut browser {
            while let Some((len, _)) = socket.recv_datagram(&mut buf)? {
                if let Ok(MasterPacket::Page(page)) = MasterPacket::decode(&buf[..len])
                    && Some(page.nonce) == browser_nonce
                {
                    browser_answered += 1;
                    browser_nonce = None;
                }
            }
        }
        if now >= next_progress && elapsed < length {
            next_progress += Duration::from_secs(1);
            let sent: u64 = report.sent.values().sum();
            let answered: u64 = report.answers.values().sum();
            writeln!(
                out,
                "flood {:.0} s: sent {sent}, answered {answered}",
                elapsed.as_secs_f64()
            )?;
            out.flush()?;
        }
        std::thread::sleep(Duration::from_micros(500));
    }
    report.bytes_sent = sent_bytes.iter().sum();
    report.bytes_answered = got_bytes.iter().sum();
    for (sent, got) in sent_bytes.iter().zip(&got_bytes) {
        if got > sent {
            report.over_answered_ports += 1;
        }
        if *sent > 0 {
            report.largest_share = report.largest_share.max(*got as f64 / *sent as f64);
        }
    }
    if browser.is_some() {
        report.browser = Some((browser_asked, browser_answered));
    }
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_kind_encodes_and_padded_ones_are_full_length() {
        let target: SocketAddr = "127.0.0.1:26911".parse().unwrap();
        let mut rng = SplitMix64::new(3);
        for kind in KINDS {
            let (to, bytes) = datagram(kind, &mut rng, target);
            assert!(!bytes.is_empty(), "{kind}");
            assert!(bytes.len() <= MAX_MASTER_DATAGRAM + 100, "{kind}");
            if kind == "probe-second-port" {
                assert_eq!(to.port(), 26912);
            }
            match kind {
                "register" | "register-bad-cookie" | "browse" => assert_eq!(bytes.len(), 1_200),
                "details" | "introduce" => assert_eq!(bytes.len(), 1_000),
                "probe" => assert_eq!(bytes.len(), 64),
                _ => {}
            }
        }
    }

    #[test]
    fn the_verdict_needs_no_over_answer_and_a_browse_answered() {
        let mut report = FloodReport::default();
        assert!(report.held());
        report.browser = Some((10, 4));
        assert!(!report.held());
        report.browser = Some((10, 5));
        assert!(report.held());
        report.over_answered_ports = 1;
        assert!(!report.held());
        let options = FloodOptions::new("127.0.0.1:1".parse().unwrap(), 1.0);
        assert_eq!(report.lines(&options).last().unwrap(), "limits NOT held");
    }
}
