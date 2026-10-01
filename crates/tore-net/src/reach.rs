//! Finding a host by name and checking that it answers (slice EF5): every
//! address a name gives, IPv4 first, and a probe that sends the handshake's
//! first packet to one address and says whether a host of this game answers.
//!
//! Both block, for a few seconds at most, so a caller that must not block
//! (the game's screen) runs them on a thread of its own. The probe costs the
//! host nothing: a Connect request is answered with a stateless Challenge, and
//! the probe never answers it, so no connection starts.

use crate::entropy::{Entropy, Rng};
use crate::packet::{ConnectRequest, Packet};
use crate::{HANDSHAKE_RETRY, datagram::bind_udp};
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use std::time::{Duration, Instant};

/// Why an address the player typed cannot be read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AddressError {
    /// Nothing, an empty host, or brackets that do not close or have
    /// something after them other than a port.
    Shape,
    /// The text after the colon is not a port from 1 to 65,535.
    Port(String),
}

impl std::fmt::Display for AddressError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Shape => f.write_str("needs HOST or HOST:PORT"),
            Self::Port(text) => write!(f, "port {text:?} is not 1 to 65535"),
        }
    }
}

impl std::error::Error for AddressError {}

/// Reads what a player typed: `HOST`, `HOST:PORT`, `[V6]` or `[V6]:PORT`, a
/// name or an IPv4 or IPv6 address, into a host and a port (`default_port`
/// when none is given). A bare IPv6 address (more than one colon, no
/// brackets) has no port. Spaces around the text are ignored. The host is not
/// looked up here.
pub fn split_address(text: &str, default_port: u16) -> Result<(String, u16), AddressError> {
    let text = text.trim();
    let port = |text: &str| -> Result<u16, AddressError> {
        text.parse::<u16>()
            .ok()
            .filter(|port| *port != 0)
            .ok_or_else(|| AddressError::Port(text.to_owned()))
    };
    if let Some(rest) = text.strip_prefix('[') {
        let (host, tail) = rest.split_once(']').ok_or(AddressError::Shape)?;
        if host.is_empty() {
            return Err(AddressError::Shape);
        }
        let port = match tail.strip_prefix(':') {
            Some(text) => port(text)?,
            None if tail.is_empty() => default_port,
            None => return Err(AddressError::Shape),
        };
        return Ok((host.to_owned(), port));
    }
    match text.matches(':').count() {
        0 if !text.is_empty() => Ok((text.to_owned(), default_port)),
        1 => {
            let (host, tail) = text.split_once(':').expect("one colon");
            if host.is_empty() {
                return Err(AddressError::Shape);
            }
            Ok((host.to_owned(), port(tail)?))
        }
        2.. => Ok((text.to_owned(), default_port)),
        _ => Err(AddressError::Shape),
    }
}

/// How long [`probe`] waits for one address when it is not told otherwise.
pub const PROBE_TIMEOUT: Duration = Duration::from_secs(3);

/// A random 64-bit nonce for a query or a probe.
pub fn random_nonce() -> u64 {
    Rng::new(Entropy::System).next_u64()
}

/// Every address `host` and `port` give, IPv4 ones first, each once. A
/// literal address needs no lookup; a name asks the system, which can take a
/// while.
pub fn resolve(host: &str, port: u16) -> io::Result<Vec<SocketAddr>> {
    let mut found: Vec<SocketAddr> = (host, port).to_socket_addrs()?.collect();
    // `sort_by_key` is stable: the system's order holds within a family.
    found.sort_by_key(|address| address.is_ipv6());
    let mut unique = Vec::with_capacity(found.len());
    for address in found {
        if !unique.contains(&address) {
            unique.push(address);
        }
    }
    Ok(unique)
}

/// What a probe of one address found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reach {
    /// A host of this game's protocol answered the handshake.
    Answered,
    /// A host answered with a refusal, with its text (for example another
    /// protocol version). Joining it would be refused the same way.
    Refused(String),
    /// Nothing answered in time.
    Silent,
    /// `cancel` said stop.
    Cancelled,
}

/// Sends a Connect request to `address` every [`HANDSHAKE_RETRY`] until a host
/// answers, `timeout` passes or `cancel` returns true (checked about every
/// 20 ms). `protocol_version` is this game's: a host of another one answers
/// with a refusal, which is still an answer.
pub fn probe(
    address: SocketAddr,
    protocol_version: u16,
    timeout: Duration,
    cancel: &dyn Fn() -> bool,
) -> io::Result<Reach> {
    let any: IpAddr = match address {
        SocketAddr::V4(_) => Ipv4Addr::UNSPECIFIED.into(),
        SocketAddr::V6(_) => Ipv6Addr::UNSPECIFIED.into(),
    };
    let socket = bind_udp(SocketAddr::new(any, 0))?;
    let nonce = random_nonce();
    let request = Packet::ConnectRequest(ConnectRequest {
        protocol_version,
        nonce,
        game_version: String::new(),
        game_commit: String::new(),
    })
    .encode(protocol_version)
    .map_err(io::Error::other)?;
    let started = Instant::now();
    let mut last_sent: Option<Instant> = None;
    let mut buf = [0u8; crate::MAX_DATAGRAM + 1];
    loop {
        if cancel() {
            return Ok(Reach::Cancelled);
        }
        let now = Instant::now();
        if now.duration_since(started) >= timeout {
            return Ok(Reach::Silent);
        }
        if last_sent.is_none_or(|at| now.duration_since(at) >= HANDSHAKE_RETRY) {
            match socket.send_to(&request, address) {
                Ok(_) => {}
                // A network that is not there, or a full buffer: wait and
                // try again until the time is up.
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {}
                Err(error) => return Err(error),
            }
            last_sent = Some(now);
        }
        match socket.recv_from(&mut buf) {
            Ok((len, from)) if from == address => {
                match Packet::decode(&buf[..len], protocol_version) {
                    Ok(Packet::Challenge(challenge)) if challenge.nonce == nonce => {
                        return Ok(Reach::Answered);
                    }
                    Ok(Packet::Refuse(refuse)) if refuse.nonce == nonce => {
                        return Ok(Reach::Refused(refuse.text));
                    }
                    _ => {}
                }
            }
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::ConnectionReset
                ) =>
            {
                std::thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConnectDetails, Decision, RefuseReason, Server, ServerConfig, ServerEvent,
        packet::{Challenge, Refuse},
    };
    use std::net::UdpSocket;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    const V: u16 = 3;

    /// A host on loopback that answers connect requests until told to stop.
    fn host(version: u16) -> (SocketAddr, Arc<AtomicBool>, std::thread::JoinHandle<()>) {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let address = socket.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let mut server = Server::new(ServerConfig {
                entropy: Entropy::Seeded(4),
                ..ServerConfig::new(version)
            });
            let mut gate = |_: &ConnectDetails| Decision::Refuse {
                reason: RefuseReason::ShuttingDown,
                text: String::new(),
            };
            let started = Instant::now();
            let mut buf = [0u8; 1201];
            while !flag.load(Ordering::Relaxed) {
                if let Ok((len, from)) = socket.recv_from(&mut buf) {
                    server.receive(started.elapsed(), from, &buf[..len], &mut gate);
                }
                while let Some(transmit) = server.poll_transmit() {
                    socket.send_to(&transmit.datagram, transmit.to).unwrap();
                }
                while let Some(event) = server.poll_event() {
                    assert!(!matches!(event, ServerEvent::Connected { .. }));
                }
            }
        });
        (address, stop, thread)
    }

    fn never() -> bool {
        false
    }

    #[test]
    fn every_form_of_an_address_is_read() {
        let read = |text: &str| split_address(text, 26_900);
        let ok = |host: &str, port: u16| Ok((host.to_owned(), port));
        assert_eq!(read("game.example.org"), ok("game.example.org", 26_900));
        assert_eq!(read("game.example.org:1234"), ok("game.example.org", 1_234));
        assert_eq!(read("192.168.1.20"), ok("192.168.1.20", 26_900));
        assert_eq!(read("192.168.1.20:27000"), ok("192.168.1.20", 27_000));
        assert_eq!(read("::1"), ok("::1", 26_900));
        assert_eq!(read("fe80::1"), ok("fe80::1", 26_900));
        assert_eq!(read("[::1]"), ok("::1", 26_900));
        assert_eq!(read("[::1]:4000"), ok("::1", 4_000));
        assert_eq!(read("[fe80::1%eth0]:4000"), ok("fe80::1%eth0", 4_000));
        assert_eq!(read("  localhost:65535  "), ok("localhost", 65_535));
        for shape in ["", "   ", ":26900", "[::1", "[::1]x", "[]", "[]:5"] {
            assert_eq!(read(shape), Err(AddressError::Shape), "{shape:?}");
        }
        for port in [
            "host:0",
            "host:70000",
            "host:abc",
            "host:",
            "[::1]:0",
            "[::1]:",
        ] {
            assert!(matches!(read(port), Err(AddressError::Port(_))), "{port:?}");
        }
    }

    #[test]
    fn a_name_gives_every_address_ipv4_first_and_each_once() {
        assert_eq!(
            resolve("127.0.0.1", 26_900).unwrap(),
            vec!["127.0.0.1:26900".parse::<SocketAddr>().unwrap()]
        );
        assert_eq!(
            resolve("::1", 26_900).unwrap(),
            vec!["[::1]:26900".parse::<SocketAddr>().unwrap()]
        );
        // `localhost` gives IPv4 and usually IPv6; the IPv4 comes first.
        let found = resolve("localhost", 5).unwrap();
        assert!(!found.is_empty());
        let first_v6 = found.iter().position(SocketAddr::is_ipv6);
        let last_v4 = found.iter().rposition(SocketAddr::is_ipv4);
        if let (Some(v6), Some(v4)) = (first_v6, last_v4) {
            assert!(v4 < v6, "{found:?}");
        }
        let mut sorted = found.clone();
        sorted.dedup();
        assert_eq!(sorted.len(), found.len());
        assert!(resolve("no-such-host.invalid", 5).is_err());
    }

    #[test]
    fn a_host_that_answers_is_reached_and_a_dead_port_is_silent() {
        let (address, stop, thread) = host(V);
        let started = Instant::now();
        assert_eq!(
            probe(address, V, Duration::from_secs(2), &never).unwrap(),
            Reach::Answered
        );
        assert!(started.elapsed() < Duration::from_secs(1));
        // A port nothing listens on.
        let dead = UdpSocket::bind("127.0.0.1:0").unwrap();
        let dead_address = dead.local_addr().unwrap();
        let started = Instant::now();
        assert_eq!(
            probe(dead_address, V, Duration::from_millis(400), &never).unwrap(),
            Reach::Silent
        );
        assert!(started.elapsed() >= Duration::from_millis(400));
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
    }

    #[test]
    fn a_host_of_another_version_refuses_and_that_counts_as_an_answer() {
        let (address, stop, thread) = host(V + 1);
        let reach = probe(address, V, Duration::from_secs(2), &never).unwrap();
        let Reach::Refused(text) = reach else {
            panic!("{reach:?}")
        };
        assert!(text.contains("protocol version"), "{text}");
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
    }

    #[test]
    fn a_probe_can_be_cancelled() {
        let dead = UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = dead.local_addr().unwrap();
        let started = Instant::now();
        let reach = probe(address, V, Duration::from_secs(30), &|| {
            started.elapsed() > Duration::from_millis(100)
        })
        .unwrap();
        assert_eq!(reach, Reach::Cancelled);
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn a_stranger_cannot_answer_for_the_host() {
        // A packet with the wrong nonce, or from the wrong address, is no
        // answer.
        let target = UdpSocket::bind("127.0.0.1:0").unwrap();
        let address = target.local_addr().unwrap();
        target
            .set_read_timeout(Some(Duration::from_millis(500)))
            .unwrap();
        let thread = std::thread::spawn(move || {
            let mut buf = [0u8; 1201];
            let (_, from) = target.recv_from(&mut buf).unwrap();
            let wrong = Packet::Challenge(Challenge {
                nonce: 1,
                cookie: 2,
            })
            .encode(V)
            .unwrap();
            target.send_to(&wrong, from).unwrap();
            let refuse = Packet::Refuse(Refuse {
                nonce: 1,
                reason: 1,
                text: "nope".into(),
            })
            .encode(V)
            .unwrap();
            target.send_to(&refuse, from).unwrap();
        });
        let reach = probe(address, V, Duration::from_millis(600), &never).unwrap();
        thread.join().unwrap();
        assert_eq!(reach, Reach::Silent);
    }
}
