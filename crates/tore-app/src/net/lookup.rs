//! Finding the server a player typed, off the screen's thread (slice EF5).
//!
//! A name can take seconds to look up and an address nobody answers takes
//! seconds to give up on, so [`Lookup`] does both on a thread of its own and
//! tells the caller how it is getting on through [`Progress`] events, which
//! the caller polls each frame without blocking. Every address the name gives
//! is tried in turn, IPv4 first, with [`tore_net::reach::probe`], until one
//! answers the handshake's first step; the caller then joins that address
//! (EF7's Join). Cancelling, or dropping the lookup, stops the thread within a
//! fraction of a second.

use std::fmt;
use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread::JoinHandle;
use std::time::Duration;
use tore_net::reach::{self, PROBE_TIMEOUT, Reach};

/// How the lookup is getting on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Asking the system for the name's addresses.
    LookingUp(String),
    /// The addresses the name gave, in the order they will be tried.
    Found(Vec<SocketAddr>),
    /// Trying this one.
    Trying(SocketAddr),
    /// This one did not answer in time; the next is tried.
    NoAnswer(SocketAddr),
    /// This one answered, and will be joined. The last event.
    Reached(SocketAddr),
    /// This one is a host, and refused with this text (for example another
    /// protocol version); joining it would be refused the same way. The last
    /// event.
    Refused(SocketAddr, String),
    /// Nothing was reached, with the text to show. The last event.
    Failed(String),
}

impl Progress {
    /// True for the last event of a lookup.
    pub fn is_last(&self) -> bool {
        matches!(self, Self::Reached(_) | Self::Refused(..) | Self::Failed(_))
    }
}

impl fmt::Display for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LookingUp(host) => write!(f, "Looking up {host}..."),
            Self::Found(addresses) => {
                let list: Vec<String> = addresses.iter().map(ToString::to_string).collect();
                write!(f, "Found {}", list.join(", "))
            }
            Self::Trying(address) => write!(f, "Trying {address}..."),
            Self::NoAnswer(address) => write!(f, "No answer from {address}"),
            Self::Reached(address) => write!(f, "{address} answered"),
            Self::Refused(_, text) => write!(f, "Refused: {text}"),
            Self::Failed(text) => f.write_str(text),
        }
    }
}

/// A lookup under way.
pub struct Lookup {
    events: Receiver<Progress>,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    finished: bool,
}

impl Lookup {
    /// Starts looking up what the player typed (`HOST`, `HOST:PORT`, `[V6]`,
    /// `[V6]:PORT`, or an address), for a game that speaks `protocol_version`.
    /// A text that is not an address is an error at once, with no thread.
    pub fn start(text: &str, protocol_version: u16) -> Result<Self, String> {
        let (host, port) = reach::split_address(text, tore_net::DEFAULT_PORT)
            .map_err(|error| format!("The address {error}"))?;
        let name = host.clone();
        Ok(Self::spawn(
            host,
            move || reach::resolve(&name, port),
            protocol_version,
            PROBE_TIMEOUT,
        ))
    }

    /// Starts a lookup with its own resolver and per-address limit, for
    /// tests. `label` is what "Looking up" names.
    pub fn spawn(
        label: String,
        resolver: impl FnOnce() -> io::Result<Vec<SocketAddr>> + Send + 'static,
        protocol_version: u16,
        per_address: Duration,
    ) -> Self {
        let (send, events) = channel();
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&cancel);
        let thread = std::thread::Builder::new()
            .name("tore-lookup".into())
            .spawn(move || {
                run(
                    &label,
                    resolver,
                    protocol_version,
                    per_address,
                    &flag,
                    &send,
                )
            })
            .ok();
        Self {
            events,
            cancel,
            finished: thread.is_none(),
            thread,
        }
    }

    /// The next thing that happened, oldest first, or `None` when nothing has
    /// yet. Never blocks.
    pub fn poll(&mut self) -> Option<Progress> {
        match self.events.try_recv() {
            Ok(event) => {
                self.finished |= event.is_last();
                Some(event)
            }
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                if self.finished {
                    None
                } else {
                    // The thread ended without a last event: it was cancelled
                    // or it panicked.
                    self.finished = true;
                    if self.cancel.load(Ordering::Relaxed) {
                        None
                    } else {
                        Some(Progress::Failed("The lookup stopped unexpectedly.".into()))
                    }
                }
            }
        }
    }

    /// Stops the lookup; no more events follow.
    pub fn cancel(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.finished = true;
    }

    /// True once the last event has been taken, or the lookup was cancelled.
    #[cfg(test)]
    pub fn finished(&self) -> bool {
        self.finished
    }
}

impl Drop for Lookup {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        // The thread notices within one probe step; it is not waited for, so
        // a name still being looked up (which cannot be interrupted) never
        // holds the screen.
        drop(self.thread.take());
    }
}

fn run(
    label: &str,
    resolver: impl FnOnce() -> io::Result<Vec<SocketAddr>>,
    protocol_version: u16,
    per_address: Duration,
    cancel: &AtomicBool,
    send: &Sender<Progress>,
) {
    let cancelled = || cancel.load(Ordering::Relaxed);
    let say = |event: Progress| send.send(event).is_ok();
    if !say(Progress::LookingUp(label.to_owned())) {
        return;
    }
    let addresses = match resolver() {
        Ok(addresses) if !addresses.is_empty() => addresses,
        Ok(_) => {
            let _ = say(Progress::Failed(format!("Cannot find the server {label}.")));
            return;
        }
        Err(error) => {
            let _ = say(Progress::Failed(format!(
                "Cannot find the server {label}: {error}"
            )));
            return;
        }
    };
    if cancelled() || !say(Progress::Found(addresses.clone())) {
        return;
    }
    for address in &addresses {
        if cancelled() || !say(Progress::Trying(*address)) {
            return;
        }
        match reach::probe(*address, protocol_version, per_address, &cancelled) {
            Ok(Reach::Answered) => {
                let _ = say(Progress::Reached(*address));
                return;
            }
            Ok(Reach::Refused(text)) => {
                let _ = say(Progress::Refused(*address, text));
                return;
            }
            Ok(Reach::Silent) => {
                if !say(Progress::NoAnswer(*address)) {
                    return;
                }
            }
            Ok(Reach::Cancelled) => return,
            // This address cannot be tried at all (no route, no such
            // family): the same as no answer, and the next is tried.
            Err(_) => {
                if !say(Progress::NoAnswer(*address)) {
                    return;
                }
            }
        }
    }
    let tried: Vec<String> = addresses.iter().map(ToString::to_string).collect();
    let _ = say(Progress::Failed(format!(
        "No answer from {label} ({}).",
        tried.join(", ")
    )));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::UdpSocket;
    use std::time::Instant;
    use tore_net::{ConnectDetails, Decision, Entropy, RefuseReason, Server, ServerConfig};

    const V: u16 = 3;

    /// A host on loopback that answers connect requests until stopped.
    fn host(version: u16) -> (SocketAddr, Arc<AtomicBool>, JoinHandle<()>) {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_millis(20)))
            .unwrap();
        let address = socket.local_addr().unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stop);
        let thread = std::thread::spawn(move || {
            let mut server = Server::new(ServerConfig {
                entropy: Entropy::Seeded(2),
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
            }
        });
        (address, stop, thread)
    }

    /// Every event until the last, within five seconds.
    fn all(lookup: &mut Lookup) -> Vec<Progress> {
        let started = Instant::now();
        let mut events = Vec::new();
        while started.elapsed() < Duration::from_secs(5) {
            match lookup.poll() {
                Some(event) => {
                    let last = event.is_last();
                    events.push(event);
                    if last {
                        break;
                    }
                }
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
        events
    }

    #[test]
    fn a_dead_address_is_tried_and_then_a_live_one() {
        let dead = UdpSocket::bind("127.0.0.1:0").unwrap();
        let dead_address = dead.local_addr().unwrap();
        let (live, stop, thread) = host(V);
        let mut lookup = Lookup::spawn(
            "game.example.org".into(),
            move || Ok(vec![dead_address, live]),
            V,
            Duration::from_millis(300),
        );
        let started = Instant::now();
        let events = all(&mut lookup);
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        assert_eq!(
            events,
            vec![
                Progress::LookingUp("game.example.org".into()),
                Progress::Found(vec![dead_address, live]),
                Progress::Trying(dead_address),
                Progress::NoAnswer(dead_address),
                Progress::Trying(live),
                Progress::Reached(live),
            ]
        );
        // The dead address cost its limit and the live one almost nothing.
        assert!(started.elapsed() >= Duration::from_millis(300));
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(lookup.finished());
    }

    #[test]
    fn every_address_dead_fails_with_the_list() {
        let a = UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let b = UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let mut lookup = Lookup::spawn(
            "gone".into(),
            move || Ok(vec![a, b]),
            V,
            Duration::from_millis(150),
        );
        let events = all(&mut lookup);
        let Some(Progress::Failed(text)) = events.last() else {
            panic!("{events:?}")
        };
        assert!(text.contains("No answer from gone") && text.contains(&a.to_string()));
        assert_eq!(
            events
                .iter()
                .filter(|e| matches!(e, Progress::NoAnswer(_)))
                .count(),
            2
        );
    }

    #[test]
    fn a_name_that_does_not_resolve_fails_and_a_host_of_another_version_is_refused() {
        let mut lookup = Lookup::start("no-such-host.invalid:4000", V).unwrap();
        let events = all(&mut lookup);
        assert!(matches!(
            events.last(),
            Some(Progress::Failed(text)) if text.starts_with("Cannot find the server no-such-host.invalid")
        ));
        let (address, stop, thread) = host(V + 1);
        let mut lookup = Lookup::start(&address.to_string(), V).unwrap();
        let events = all(&mut lookup);
        stop.store(true, Ordering::Relaxed);
        thread.join().unwrap();
        let Some(Progress::Refused(from, text)) = events.last() else {
            panic!("{events:?}")
        };
        assert_eq!(*from, address);
        assert!(text.contains("protocol version"));
        assert!(events.last().unwrap().to_string().starts_with("Refused: "));
    }

    #[test]
    fn a_bad_address_is_refused_at_once_and_a_lookup_can_be_cancelled() {
        assert!(Lookup::start("", V).is_err());
        assert!(Lookup::start("host:99999", V).is_err());
        let dead = UdpSocket::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let mut lookup = Lookup::spawn(
            "x".into(),
            move || Ok(vec![dead]),
            V,
            Duration::from_secs(30),
        );
        let started = Instant::now();
        while !matches!(lookup.poll(), Some(Progress::Trying(_))) {
            assert!(started.elapsed() < Duration::from_secs(3));
            std::thread::sleep(Duration::from_millis(5));
        }
        lookup.cancel();
        assert!(lookup.finished());
        std::thread::sleep(Duration::from_millis(100));
        assert_eq!(lookup.poll(), None);
    }

    #[test]
    fn the_screen_is_never_blocked_by_a_slow_lookup() {
        // A resolver that takes a second leaves poll() answering at once.
        let mut lookup = Lookup::spawn(
            "slow".into(),
            || {
                std::thread::sleep(Duration::from_millis(500));
                Err(io::Error::other("timed out"))
            },
            V,
            Duration::from_millis(100),
        );
        let started = Instant::now();
        let mut polls = 0;
        while !lookup.finished() && started.elapsed() < Duration::from_secs(3) {
            let before = Instant::now();
            let _ = lookup.poll();
            assert!(before.elapsed() < Duration::from_millis(50));
            polls += 1;
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(polls > 5);
    }
}
