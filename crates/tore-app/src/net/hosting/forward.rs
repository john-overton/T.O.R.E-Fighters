//! A hosting game asks its router to forward the game port (slice J4b of
//! stage J). The host thread starts a [`Forwarder`], which runs
//! `tore_net::portmap::PortMapper` on a thread of its own: it maps the port
//! (the library blocks for up to 5 seconds), tells the host what came of it
//! ([`News`]), renews the mapping at half its lease, asks again now and then
//! after a failure, and removes the mapping when hosting stops. See
//! docs/ARCHITECTURE.md, "Port mapping". Every choice here is an agent
//! decision unless it is credited.
//!
//! - **What the host does with the news.** The outside address of an IPv4
//!   mapping is the game's Mapped candidate
//!   (`HostListing::set_mapped`), and the lines for the player go to the
//!   lobby's Messages. The report's "Port mapping" value goes with the
//!   hosting game's telemetry Report.
//! - **Only a change is news.** A renewal that keeps the same address says
//!   nothing; a changed address, a lost mapping or a mapping at last is news.
//! - **A failure is tried again** every [`RETRY`] (10 minutes), since a
//!   router may be switched on or a lease may have lapsed; a mapping that
//!   has no end (an old device) is never renewed.
//! - **Stopping.** [`Forwarder::begin_stop`] only signals, so the removal
//!   runs while the host says goodbye to its players; [`Forwarder::finish`]
//!   waits for it, at most [`REMOVE_WAIT`]. A mapping still being asked for
//!   when the game stops is removed as soon as the library returns, if the
//!   game is still running; otherwise it lapses with its lease.
//! - **Tests** point the mapper at the fakes on loopback
//!   ([`Forward::with_config`]); none asks a real router. The real choice
//!   is [`choose`], from the Options switch.
use std::{
    net::{IpAddr, Ipv6Addr, SocketAddr},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use tore_net::{
    master::PortMapping,
    portmap::{MapError, MapReport, MapperConfig, Mapping, PortMapper, gateway},
};

/// How long after a failure the mapper asks again.
pub const RETRY: Duration = Duration::from_secs(600);
/// How long [`Forwarder::finish`] waits for the mapping to be removed: the
/// library's own limit and a little more.
pub const REMOVE_WAIT: Duration = Duration::from_millis(2500);
/// What a player is told after a failure, after the router's own words.
const RELAY_HINT: &str = "Friends outside your network may need the relay.";
/// An address that has an IPv6 route when the machine has one; connecting a
/// UDP socket to it sends nothing.
const IPV6_TARGET: SocketAddr = SocketAddr::new(
    IpAddr::V6(Ipv6Addr::new(0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x8888)),
    53,
);

/// What to ask and whom to ask: the real router, or a fake in a test.
#[derive(Clone, Debug)]
pub struct Forward {
    config: MapperConfig,
    retry: Duration,
}

impl Forward {
    /// The real router, by every protocol, and the machine's global IPv6
    /// address too if it has one. The port is set when the forwarder starts.
    pub fn system() -> Self {
        let mut config = MapperConfig::new(0);
        config.ipv6 = match gateway::local_address_toward(IPV6_TARGET) {
            Some(IpAddr::V6(ip)) if ip.segments()[0] & 0xe000 == 0x2000 => Some(ip),
            _ => None,
        };
        Self::with_config(config)
    }

    /// Asks as `config` says (a test's fakes). The port is set when the
    /// forwarder starts.
    pub fn with_config(config: MapperConfig) -> Self {
        Self {
            config,
            retry: RETRY,
        }
    }

    /// Asks again after a failure this soon, for tests.
    #[cfg(test)]
    pub fn with_retry(mut self, retry: Duration) -> Self {
        self.retry = retry;
        self
    }
}

/// The environment variable that turns port mapping off whatever the switch
/// says: the battery and agents' runs set it, so a test never asks a real
/// router.
pub const NO_MAPPING_ENV: &str = "TORE_NO_PORT_MAPPING";

/// The switch in Options: the real router while it is on, else nothing;
/// `TORE_NO_PORT_MAPPING` set (to anything) overrides the switch.
pub fn choose(port_forward: bool) -> Option<Forward> {
    choose_with(port_forward, std::env::var_os(NO_MAPPING_ENV).is_some())
}

fn choose_with(port_forward: bool, vetoed: bool) -> Option<Forward> {
    (port_forward && !vetoed).then(Forward::system)
}

/// What the mapper thread tells the host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum News {
    /// The mapping asked for, or renewed, came to this.
    Mapped {
        /// The lines for the player: what the router did, or why not.
        lines: Vec<String>,
        /// The IPv4 mapping's outside address: the Mapped candidate.
        outside: Option<SocketAddr>,
        /// The telemetry report's "Port mapping" value.
        port_mapping: PortMapping,
    },
    /// The mapping was removed as hosting stopped; a line only when that
    /// went wrong.
    Removed { lines: Vec<String> },
}

impl News {
    /// The lines for the player.
    pub fn lines(&self) -> &[String] {
        match self {
            Self::Mapped { lines, .. } | Self::Removed { lines } => lines,
        }
    }
}

/// What a report says in the parts that are news.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Gist {
    ipv4: Result<SocketAddr, String>,
    ipv6: Option<Result<SocketAddr, String>>,
}

fn gist(report: &MapReport) -> Gist {
    let of = |result: &Result<Mapping, MapError>| match result {
        Ok(mapping) => Ok(mapping.outside),
        Err(error) => Err(error.to_string()),
    };
    Gist {
        ipv4: of(&report.ipv4),
        ipv6: report.ipv6.as_ref().map(of),
    }
}

/// The lines a player reads for a report: the IPv4 result, and the IPv6 one
/// when it worked (a failure there is common and says nothing useful).
fn lines(report: &MapReport) -> Vec<String> {
    let mut lines = vec![match &report.ipv4 {
        Ok(mapping) => mapping.to_string(),
        Err(error) => format!("{error} {RELAY_HINT}"),
    }];
    if let Some(Ok(mapping)) = &report.ipv6 {
        lines.push(mapping.to_string());
    }
    lines
}

/// The mapper thread, as the host holds it.
pub struct Forwarder {
    stop: Sender<()>,
    news: Receiver<News>,
    handle: Option<JoinHandle<()>>,
    stopping: bool,
}

impl Forwarder {
    /// Starts the thread, which maps `port` at once.
    pub fn start(forward: Forward, port: u16) -> Self {
        let Forward { mut config, retry } = forward;
        config.port = port;
        let (stop, commands) = mpsc::channel();
        let (sender, news) = mpsc::channel();
        let handle = thread::Builder::new()
            .name("tore-portmap".into())
            .spawn(move || run(PortMapper::new(config), retry, &commands, &sender))
            .ok();
        if handle.is_none() {
            log::warn!("Host: cannot start the port mapping thread");
        }
        Self {
            stop,
            news,
            handle,
            stopping: false,
        }
    }

    /// The news since the last call, oldest first.
    pub fn poll(&self) -> Vec<News> {
        self.news.try_iter().collect()
    }

    /// Tells the thread to remove the mapping and end. It does not wait.
    pub fn begin_stop(&mut self) {
        if !std::mem::replace(&mut self.stopping, true) {
            let _ = self.stop.send(());
        }
    }

    /// [`Forwarder::begin_stop`], then waits at most `limit` for the thread
    /// to end. True when it did; otherwise it is left to finish on its own.
    pub fn finish(&mut self, limit: Duration) -> bool {
        self.begin_stop();
        let Some(handle) = self.handle.take() else {
            return true;
        };
        let deadline = Instant::now() + limit;
        while !handle.is_finished() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(2));
        }
        let ended = handle.is_finished();
        if ended {
            let _ = handle.join();
        } else {
            log::warn!(
                "Host: the port mapping thread did not finish within {} ms; leaving it to finish",
                limit.as_millis()
            );
        }
        ended
    }
}

impl Drop for Forwarder {
    /// A forwarder dropped without a word still removes its mapping, but
    /// the game does not wait for it.
    fn drop(&mut self) {
        self.begin_stop();
    }
}

fn run(mut mapper: PortMapper, retry: Duration, commands: &Receiver<()>, news: &Sender<News>) {
    let mut last: Option<Gist> = None;
    let mut first = true;
    loop {
        let report = if std::mem::take(&mut first) {
            mapper.map()
        } else {
            mapper.renew()
        };
        let now = gist(&report);
        if last.as_ref() != Some(&now) {
            last = Some(now);
            let outside = report.ipv4.as_ref().ok().map(|mapping| mapping.outside);
            let port_mapping = PortMapping::from_code(u64::from(report.telemetry()))
                .unwrap_or(PortMapping::NotTried);
            let _ = news.send(News::Mapped {
                lines: lines(&report),
                outside,
                port_mapping,
            });
        }
        let wait = match mapper.next_renewal() {
            Some(at) => at.saturating_duration_since(Instant::now()),
            None if report.ipv4.is_ok() => Duration::from_secs(3600),
            None => retry,
        };
        match commands.recv_timeout(wait) {
            Err(RecvTimeoutError::Timeout) => {}
            // Told to stop, or the host is gone.
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
        }
    }
    let held = mapper.mapping().is_some() || mapper.ipv6_mapping().is_some();
    let lines = match mapper.remove() {
        Ok(()) => {
            if held {
                log::info!("Host: the router's port forward was removed");
            }
            Vec::new()
        }
        Err(error) => vec![format!(
            "The port forward could not be removed ({error}); it ends within an hour."
        )],
    };
    let _ = news.send(News::Removed { lines });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use tore_net::Entropy;
    use tore_net::portmap::fake::{FakeGateway, FakeGatewayConfig, FakeUpnp, FakeUpnpConfig};

    const PORT: u16 = 26900;
    const OUTSIDE: Ipv4Addr = Ipv4Addr::new(203, 0, 113, 5);

    /// A mapper config that asks only the fakes, on loopback: no SSDP
    /// multicast, no gateway lookup, no IPv6.
    fn config(upnp: Option<&FakeUpnp>, gateway: Option<&FakeGateway>) -> MapperConfig {
        MapperConfig {
            upnp: upnp.is_some(),
            pcp: gateway.is_some(),
            ssdp: upnp.map_or((Ipv4Addr::LOCALHOST, 9).into(), FakeUpnp::ssdp_address),
            gateway: Some(gateway.map_or((Ipv4Addr::LOCALHOST, 9).into(), FakeGateway::address)),
            gateway_v6: Some((Ipv6Addr::LOCALHOST, 9).into()),
            entropy: Entropy::Seeded(7),
            ..MapperConfig::new(PORT)
        }
    }

    fn gateway(change: impl FnOnce(&mut FakeGatewayConfig)) -> FakeGateway {
        let mut config = FakeGatewayConfig::default();
        change(&mut config);
        FakeGateway::start((Ipv4Addr::LOCALHOST, 0).into(), config).unwrap()
    }

    fn upnp() -> FakeUpnp {
        FakeUpnp::start(FakeUpnpConfig::default()).unwrap()
    }

    /// Waits up to `limit` for news that `done` accepts.
    fn wait_for(
        forwarder: &Forwarder,
        limit: Duration,
        mut done: impl FnMut(&News) -> bool,
    ) -> Option<News> {
        let started = Instant::now();
        while started.elapsed() < limit {
            for news in forwarder.poll() {
                if done(&news) {
                    return Some(news);
                }
            }
            thread::sleep(Duration::from_millis(5));
        }
        None
    }

    fn mapped(news: &News) -> bool {
        matches!(news, News::Mapped { .. })
    }

    #[test]
    fn the_switch_off_asks_nothing_and_on_asks_the_real_router() {
        assert!(choose_with(false, false).is_none());
        assert!(
            choose_with(true, true).is_none(),
            "the environment vetoes the switch"
        );
        let forward = choose_with(true, false).expect("on");
        assert!(forward.config.upnp && forward.config.pcp);
        assert_eq!(forward.retry, RETRY);
    }

    #[test]
    fn it_maps_tells_the_address_and_removes_the_mapping_on_finish() {
        let router = gateway(|_| {});
        let mut forwarder =
            Forwarder::start(Forward::with_config(config(None, Some(&router))), 31_001);
        let news = wait_for(&forwarder, Duration::from_secs(8), mapped).expect("news");
        let News::Mapped {
            lines,
            outside,
            port_mapping,
        } = news
        else {
            unreachable!()
        };
        // The port is the hosting game's, not the config's.
        assert_eq!(outside, Some(SocketAddr::new(OUTSIDE.into(), 31_001)));
        assert_eq!(port_mapping, PortMapping::Pcp);
        assert_eq!(
            lines,
            ["Your router forwards UDP port 31001 (PCP). Friends can join at 203.0.113.5:31001."]
        );
        assert_eq!(router.mappings().len(), 1);
        assert!(forwarder.finish(REMOVE_WAIT));
        assert!(router.mappings().is_empty());
        assert_eq!(forwarder.poll(), [News::Removed { lines: Vec::new() }]);
    }

    #[test]
    fn a_mapping_by_upnp_is_removed_too() {
        let device = upnp();
        let mut forwarder =
            Forwarder::start(Forward::with_config(config(Some(&device), None)), PORT);
        let news = wait_for(&forwarder, Duration::from_secs(8), mapped).expect("news");
        assert!(matches!(
            news,
            News::Mapped {
                port_mapping: PortMapping::Upnp,
                outside: Some(_),
                ..
            }
        ));
        assert_eq!(device.mappings().len(), 1);
        assert!(forwarder.finish(REMOVE_WAIT));
        assert!(device.mappings().is_empty());
    }

    #[test]
    fn a_router_that_answers_nothing_is_reported_plainly_and_asked_again() {
        let router = gateway(|c| c.silent = true);
        let mut forward = config(None, Some(&router));
        forward.budget = Duration::from_millis(300);
        let forward = Forward::with_config(forward).with_retry(Duration::from_millis(200));
        let mut forwarder = Forwarder::start(forward, PORT);
        let news = wait_for(&forwarder, Duration::from_secs(5), mapped).expect("news");
        assert_eq!(
            news,
            News::Mapped {
                lines: vec![
                    "Your router did not answer a request to forward the port (UPnP, NAT-PMP or PCP). \
                     Friends outside your network may need the relay."
                        .to_owned()
                ],
                outside: None,
                port_mapping: PortMapping::Failed,
            }
        );
        // It asks again; the router comes back; that is news.
        let asked = router.pcp_requests();
        router.set_silent(false);
        let news = wait_for(&forwarder, Duration::from_secs(5), mapped).expect("mapped later");
        assert!(
            matches!(
                news,
                News::Mapped {
                    outside: Some(_),
                    ..
                }
            ),
            "{news:?}"
        );
        assert!(router.pcp_requests() > asked);
        assert!(forwarder.finish(REMOVE_WAIT));
        assert!(router.mappings().is_empty());
    }

    #[test]
    fn a_router_behind_another_is_reported_and_maps_nothing() {
        let router = gateway(|c| c.outside = Ipv4Addr::new(192, 168, 0, 7));
        let mut forwarder =
            Forwarder::start(Forward::with_config(config(None, Some(&router))), PORT);
        let News::Mapped {
            lines,
            outside,
            port_mapping,
        } = wait_for(&forwarder, Duration::from_secs(8), mapped).expect("news")
        else {
            unreachable!()
        };
        assert_eq!(outside, None);
        assert_eq!(port_mapping, PortMapping::SecondRouter);
        assert_eq!(
            lines,
            [
                "Your router is behind another one, so the port could not be opened to the internet. \
              Friends outside your network may need the relay."
            ]
        );
        assert!(forwarder.finish(REMOVE_WAIT));
        assert!(router.mappings().is_empty());
    }

    #[test]
    fn a_renewal_that_changes_nothing_is_no_news_and_the_mapping_stays_held() {
        // A lease of two seconds is renewed after one.
        let router = gateway(|c| c.max_lifetime = Some(2));
        let mut forwarder =
            Forwarder::start(Forward::with_config(config(None, Some(&router))), PORT);
        wait_for(&forwarder, Duration::from_secs(8), mapped).expect("news");
        let before = router.pcp_requests();
        thread::sleep(Duration::from_millis(2600));
        assert!(router.pcp_requests() > before, "it renewed");
        assert_eq!(router.mappings().len(), 1);
        assert!(forwarder.poll().is_empty(), "the same address is no news");
        assert!(forwarder.finish(REMOVE_WAIT));
        assert!(router.mappings().is_empty());
    }

    #[test]
    fn finish_ends_within_its_limit_with_a_router_that_is_silent_at_the_end() {
        let router = gateway(|_| {});
        let mut forward = config(None, Some(&router));
        forward.remove_budget = Duration::from_millis(300);
        let mut forwarder = Forwarder::start(Forward::with_config(forward), PORT);
        wait_for(&forwarder, Duration::from_secs(8), mapped).expect("news");
        router.set_silent(true);
        let started = Instant::now();
        assert!(forwarder.finish(REMOVE_WAIT));
        assert!(started.elapsed() < REMOVE_WAIT);
        // The mapping lapses on its own at the router; the player is told.
        let removed = forwarder.poll();
        assert!(
            matches!(
                removed.as_slice(),
                [News::Removed { lines }] if lines.len() == 1 && lines[0].contains("ends within an hour")
            ),
            "{removed:?}"
        );
    }
}
