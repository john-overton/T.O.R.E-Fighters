//! The game's own addresses and the master's (slice I3): what a host tells
//! the master about itself ("Addresses and candidates" in the architecture
//! guide), and the lookup of the master's name on a thread of its own.
//!
//! The standard library cannot list a machine's addresses. Pointing an
//! unconnected UDP socket at the master's address and reading the address
//! the system chose for it (no packet is sent) gives the address of the
//! network that leads to the master, which is the one that matters: for
//! IPv4 the host's Local candidate, for IPv6 its Global IPv6 candidate when
//! it is global (`2000::/3`). Taken this way it is the address the system
//! also sends from, which matters when the system uses temporary privacy
//! addresses.

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::thread;

use super::MASTER_PORT;
use super::candidate::{Candidate, CandidateKind, canonical};
use crate::reach::{resolve, split_address};

/// The address the system would send from toward `target`, found without
/// sending anything. `None` when the system has no route there or names no
/// address (an unspecified one).
pub fn own_address_toward(target: SocketAddr) -> Option<IpAddr> {
    let unspecified = match target {
        SocketAddr::V4(_) => IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        SocketAddr::V6(_) => IpAddr::V6(Ipv6Addr::UNSPECIFIED),
    };
    let socket = UdpSocket::bind(SocketAddr::new(unspecified, 0)).ok()?;
    socket.connect(target).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}

/// True for a global unicast IPv6 address (`2000::/3`).
pub fn is_global_ipv6(ip: Ipv6Addr) -> bool {
    ip.segments()[0] & 0xe000 == 0x2000
}

/// The candidates a host on `port` lists from its own addresses toward
/// `masters`: at most one Local IPv4 (toward the first IPv4 master) and one
/// Global IPv6 (toward the first IPv6 master, kept only when it is global).
/// `own` finds the address toward a master: [`own_address_toward`] on a real
/// network, a fixed table in tests.
pub fn host_candidates(
    masters: &[SocketAddr],
    port: u16,
    own: impl Fn(SocketAddr) -> Option<IpAddr>,
) -> Vec<Candidate> {
    let mut candidates = Vec::new();
    if let Some(&v4) = masters.iter().find(|m| canonical(**m).is_ipv4())
        && let Some(IpAddr::V4(ip)) = own(canonical(v4))
    {
        candidates.push(Candidate::new(
            CandidateKind::Local,
            SocketAddr::new(IpAddr::V4(ip), port),
        ));
    }
    if let Some(&v6) = masters.iter().find(|m| canonical(**m).is_ipv6())
        && let Some(IpAddr::V6(ip)) = own(canonical(v6))
        && is_global_ipv6(ip)
    {
        candidates.push(Candidate::new(
            CandidateKind::GlobalIpv6,
            SocketAddr::new(IpAddr::V6(ip), port),
        ));
    }
    candidates
}

/// The master's second port, for the mapping test: the main port + 1
/// (26901 and 26902, or 26911 and 26912 for a test master on loopback).
/// *Agent decision:* the game is told one address for the master, and the
/// master's two port settings keep to this rule by default. `None` for a main
/// port of 65,535.
pub fn probe_address(main: SocketAddr) -> Option<SocketAddr> {
    Some(SocketAddr::new(main.ip(), main.port().checked_add(1)?))
}

/// Checks a master address as a player or an operator types it: `HOST`,
/// `HOST:PORT`, `[V6]:PORT`, the port 26901 when none is given. Nothing is
/// looked up.
pub fn parse_master(text: &str) -> Result<(String, u16), String> {
    split_address(text, MASTER_PORT).map_err(|error| format!("the master address {error}"))
}

/// The master's name looked up on a thread of its own, since a lookup can
/// take seconds and the hosting loop must not wait. A literal address needs
/// no lookup, but goes the same way.
#[derive(Debug)]
pub struct MasterLookup {
    answer: Receiver<io::Result<Vec<SocketAddr>>>,
}

impl MasterLookup {
    /// Starts looking up `host` and `port`.
    pub fn start(host: &str, port: u16) -> Self {
        let (sender, answer) = mpsc::channel();
        let host = host.to_owned();
        let spawned = thread::Builder::new()
            .name("tore-master-lookup".into())
            .spawn({
                let sender = sender.clone();
                move || {
                    let _ = sender.send(resolve(&host, port));
                }
            });
        if let Err(error) = spawned {
            let _ = sender.send(Err(error));
        }
        Self { answer }
    }

    /// The addresses, IPv4 first, once the lookup has ended; `None` while it
    /// runs. A lookup that gives no address is an error.
    pub fn poll(&mut self) -> Option<Result<Vec<SocketAddr>, String>> {
        match self.answer.try_recv() {
            Ok(Ok(found)) if found.is_empty() => Some(Err("no address".into())),
            Ok(Ok(found)) => Some(Ok(found)),
            Ok(Err(error)) => Some(Err(error.to_string())),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => Some(Err("the lookup stopped".into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn address(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    #[test]
    fn a_host_lists_its_local_ipv4_and_only_a_global_ipv6() {
        let masters = [
            address("198.51.100.1:26901"),
            address("[2001:db8::1]:26901"),
        ];
        let table = |ula: bool| {
            move |to: SocketAddr| match to {
                SocketAddr::V4(_) => Some("192.168.1.20".parse().unwrap()),
                SocketAddr::V6(_) if ula => Some("fd00::5".parse().unwrap()),
                SocketAddr::V6(_) => Some("2001:db8:5::20".parse().unwrap()),
            }
        };
        assert_eq!(
            host_candidates(&masters, 26900, table(false)),
            vec![
                Candidate::new(CandidateKind::Local, address("192.168.1.20:26900")),
                Candidate::new(CandidateKind::GlobalIpv6, address("[2001:db8:5::20]:26900")),
            ]
        );
        assert_eq!(host_candidates(&masters, 26900, table(true)).len(), 1);
        assert_eq!(host_candidates(&masters[..1], 7, table(false)).len(), 1);
        assert!(host_candidates(&masters, 7, |_| None).is_empty());
        // A master reached at an IPv4-mapped address counts as IPv4.
        let mapped = [address("[::ffff:198.51.100.1]:26901")];
        assert_eq!(
            host_candidates(&mapped, 7, table(false))[0].kind,
            CandidateKind::Local
        );
    }

    #[test]
    fn global_ipv6_is_2000_slash_3() {
        for (text, global) in [
            ("2001:db8::1", true),
            ("2a00::1", true),
            ("3fff::1", true),
            ("fe80::1", false),
            ("fd00::1", false),
            ("::1", false),
            ("100::1", false),
        ] {
            assert_eq!(is_global_ipv6(text.parse().unwrap()), global, "{text}");
        }
    }

    #[test]
    fn the_probe_port_follows_the_main_port() {
        assert_eq!(super::super::MASTER_PORT + 1, super::super::PROBE_PORT);
        assert_eq!(
            probe_address(address("198.51.100.1:26901")),
            Some(address("198.51.100.1:26902"))
        );
        assert_eq!(
            probe_address(address("127.0.0.1:26911")),
            Some(address("127.0.0.1:26912"))
        );
        assert_eq!(probe_address(address("127.0.0.1:65535")), None);
    }

    #[test]
    fn a_master_address_takes_the_default_port() {
        assert_eq!(
            parse_master("master.example.org").unwrap(),
            ("master.example.org".into(), 26901)
        );
        assert_eq!(parse_master("127.0.0.1:26911").unwrap().1, 26911);
        assert_eq!(parse_master("[::1]:26911").unwrap().0, "::1");
        assert!(
            parse_master("host:0")
                .unwrap_err()
                .contains("master address")
        );
    }

    #[test]
    fn a_literal_master_needs_no_name_server_and_loopback_is_its_own_way() {
        let mut lookup = MasterLookup::start("127.0.0.1", 26911);
        let started = Instant::now();
        let answer = loop {
            if let Some(answer) = lookup.poll() {
                break answer;
            }
            assert!(started.elapsed() < Duration::from_secs(5));
            thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(answer.unwrap(), vec![address("127.0.0.1:26911")]);
        assert_eq!(
            own_address_toward(address("127.0.0.1:26911")),
            Some("127.0.0.1".parse().unwrap())
        );
    }
}
