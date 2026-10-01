//! The keepalive thread: keeps a joined game's connection alive while the
//! game's own loop is stalled (slice EF-K).
//!
//! A host drops a connection it has heard nothing from for 5 seconds. A game
//! whose loop is held up (a window dragged on Windows, a long frame, a
//! screenshot on a loaded machine) sends nothing for that long though it is
//! not gone. The game hands a clone of its socket and its connection's
//! Keepalive packet ([`crate::Client::keepalive_datagram`]) to a
//! [`Keepalive`], tells it each time its loop takes a turn
//! ([`Keepalive::turned`]), and the thread sends the packet once a second
//! while the loop has not turned for a second, and only until it has not
//! turned for the bound (60 seconds by default), so a game that is truly hung
//! still times out.
//!
//! The thread only sends that one packet to that one address. It never reads
//! the socket and never touches the client's state; it stops when the
//! [`Keepalive`] is dropped. See "Keepalive" in
//! [`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md).

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::Datagrams;

/// When and for how long the thread speaks for a stalled game.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeepaliveConfig {
    /// A keepalive goes out once the game's loop has not turned for this
    /// long, and then at most once per this interval. 1 second by default.
    pub quiet: Duration,
    /// No keepalive goes out once the game's loop has not turned for this
    /// long: the host then drops the connection 5 seconds later, as before.
    /// 60 seconds by default.
    pub bound: Duration,
    /// How often the thread looks. 250 ms by default.
    pub check: Duration,
}

/// The default quiet time before a keepalive: 1 second (agent decision,
/// EF-K).
pub const KEEPALIVE_QUIET: Duration = Duration::from_secs(1);
/// The default bound: a game whose loop has not turned for 60 seconds is
/// left to time out (agent decision, EF-K).
pub const KEEPALIVE_BOUND: Duration = Duration::from_secs(60);
/// How often the thread looks by default.
pub const KEEPALIVE_CHECK: Duration = Duration::from_millis(250);

impl Default for KeepaliveConfig {
    fn default() -> Self {
        Self {
            quiet: KEEPALIVE_QUIET,
            bound: KEEPALIVE_BOUND,
            check: KEEPALIVE_CHECK,
        }
    }
}

impl KeepaliveConfig {
    /// Whether a keepalive is due, `since_turn` after the game's loop last
    /// turned and `since_sent` after the last keepalive (`None` for none).
    pub fn due(&self, since_turn: Duration, since_sent: Option<Duration>) -> bool {
        since_turn >= self.quiet
            && since_turn < self.bound
            && since_sent.is_none_or(|since| since >= self.quiet)
    }
}

/// What the game and the thread share.
struct Shared {
    origin: Instant,
    /// When the game's loop last turned, in microseconds since `origin`.
    turned: AtomicU64,
    stop: AtomicBool,
    /// Keepalives sent.
    sent: AtomicU64,
}

impl Shared {
    fn micros(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_micros()).unwrap_or(u64::MAX)
    }
}

/// A running keepalive thread for one connection. Dropping it stops the
/// thread and waits for it, which takes no longer than one send.
pub struct Keepalive {
    shared: Arc<Shared>,
    thread: Option<JoinHandle<()>>,
}

impl Keepalive {
    /// Starts the thread: it sends `datagram` to `to` on `socket` (a clone of
    /// the game's own socket, so the host sees the connection's address) as
    /// `config` says. The game's loop counts as having just turned.
    pub fn start<S>(
        socket: S,
        to: SocketAddr,
        datagram: Vec<u8>,
        config: KeepaliveConfig,
    ) -> io::Result<Self>
    where
        S: Datagrams + Send + 'static,
    {
        let shared = Arc::new(Shared {
            origin: Instant::now(),
            turned: AtomicU64::new(0),
            stop: AtomicBool::new(false),
            sent: AtomicU64::new(0),
        });
        let theirs = Arc::clone(&shared);
        let thread = thread::Builder::new()
            .name("tore-keepalive".into())
            .spawn(move || run(socket, to, &datagram, config, &theirs))?;
        Ok(Self {
            shared,
            thread: Some(thread),
        })
    }

    /// The game's loop has taken a turn (and sent what it had): the thread
    /// stays quiet for another `quiet`.
    pub fn turned(&self) {
        self.shared
            .turned
            .store(self.shared.micros(), Ordering::Relaxed);
    }

    /// Keepalives sent so far.
    pub fn sent(&self) -> u64 {
        self.shared.sent.load(Ordering::Relaxed)
    }
}

impl Drop for Keepalive {
    fn drop(&mut self) {
        self.shared.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.thread().unpark();
            let _ = thread.join();
        }
    }
}

/// The thread: look every `check`, send when due, until stopped.
fn run<S: Datagrams>(
    mut socket: S,
    to: SocketAddr,
    datagram: &[u8],
    config: KeepaliveConfig,
    shared: &Shared,
) {
    let mut last_sent: Option<u64> = None;
    loop {
        thread::park_timeout(config.check);
        if shared.stop.load(Ordering::Relaxed) {
            return;
        }
        let now = shared.micros();
        let since_turn = now.saturating_sub(shared.turned.load(Ordering::Relaxed));
        let since_sent = last_sent.map(|at| Duration::from_micros(now.saturating_sub(at)));
        if config.due(Duration::from_micros(since_turn), since_sent) {
            // A send the system cannot take is dropped, as the network might
            // drop it; the next is a second away.
            let _ = socket.send_datagram(to, datagram);
            last_sent = Some(now);
            shared.sent.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bind_udp;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn due_only_after_a_quiet_second_and_before_the_bound() {
        let config = KeepaliveConfig::default();
        assert!(!config.due(999 * MS, None));
        assert!(config.due(1000 * MS, None));
        // Once a second at most.
        assert!(!config.due(1500 * MS, Some(500 * MS)));
        assert!(config.due(2000 * MS, Some(1000 * MS)));
        // Not once the loop has been stalled for the bound.
        assert!(config.due(59_999 * MS, Some(1000 * MS)));
        assert!(!config.due(60_000 * MS, Some(1000 * MS)));
        assert!(!config.due(600_000 * MS, None));
    }

    /// A keepalive comes about once a quiet interval while the loop is
    /// stalled, none while it turns, none past the bound, and dropping the
    /// handle stops the thread at once.
    #[test]
    fn the_thread_speaks_only_for_a_stalled_loop_within_the_bound() {
        let mut host = bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
        let to = host.local_addr().unwrap();
        let game = bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
        let from = game.local_addr().unwrap();
        let config = KeepaliveConfig {
            quiet: 100 * MS,
            bound: 700 * MS,
            check: 10 * MS,
        };
        let keepalive =
            Keepalive::start(game.try_clone().unwrap(), to, vec![7; 9], config).unwrap();
        // Turning every 20 ms keeps it quiet. A sleep of 20 ms can last 150
        // on a macOS CI runner (slice EF-X), and a turn that late is a stall
        // the thread rightly speaks for, so only those may bring one.
        let start = Instant::now();
        let mut last = Instant::now();
        let mut stalls = 0;
        while start.elapsed() < 400 * MS {
            keepalive.turned();
            thread::sleep(20 * MS);
            stalls += u64::from(last.elapsed() >= config.quiet);
            last = Instant::now();
        }
        assert!(
            keepalive.sent() <= stalls,
            "{} keepalives while the loop turned, {stalls} turns late by the quiet time",
            keepalive.sent()
        );
        let quiet = keepalive.sent();
        // Stalled: one about every 100 ms from 100 ms on, then none past
        // 700 ms.
        thread::sleep(1500 * MS);
        let sent = keepalive.sent();
        assert!((3..=7).contains(&(sent - quiet)), "{sent} keepalives");
        thread::sleep(300 * MS);
        assert_eq!(keepalive.sent(), sent, "none past the bound");
        let mut buf = [0u8; 64];
        let mut received = 0;
        while let Some((len, source)) = host.recv_datagram(&mut buf).unwrap() {
            assert_eq!((len, source, buf[0]), (9, from, 7));
            received += 1;
        }
        assert_eq!(received, sent);
        // A turn starts it again.
        keepalive.turned();
        thread::sleep(300 * MS);
        assert!(keepalive.sent() > sent);
        let stopping = Instant::now();
        drop(keepalive);
        assert!(stopping.elapsed() < 1000 * MS, "{:?}", stopping.elapsed());
    }
}
