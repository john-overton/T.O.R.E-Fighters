//! A hosting game asks its router to forward the game port (slice J4b of
//! stage J): the mapper thread is `tore_net::portmap::keeper`, shared with the
//! dedicated server; this module gives it the names the host thread uses
//! and the Options switch's choice.
use std::io::Write;
use std::time::{Duration, Instant};
pub use tore_net::portmap::keeper::{
    Keeper as Forwarder, KeeperConfig as Forward, News, REMOVE_WAIT,
};

/// The real router while the Options switch is on, else nothing. *Agent
/// decision:* never in a test build (`cfg!(test)`), by construction: the
/// only way a game asks the real router is this function, so no test of the
/// game can reach one, whatever the settings file or the environment say.
/// The tests that need a mapper give the host a fake gateway directly
/// ([`crate::net::hosting::HostThread::start_forwarded`]).
pub fn choose(port_forward: bool) -> Option<Forward> {
    if cfg!(test) {
        return None;
    }
    tore_net::portmap::keeper::choose(port_forward)
}

/// The real router for `--map-port`, which asks for it by name: only the
/// environment's veto can stop it.
pub fn choose_explicit() -> Option<Forward> {
    if cfg!(test) {
        return None;
    }
    tore_net::portmap::keeper::choose(true)
}

/// `tore-app --map-port SECONDS` (slice J4b): asks the router to forward
/// `port` as a hosting game does, prints what it did, holds the mapping for
/// `hold` and removes it, for a player's or an operator's own check that
/// their router answers. Waits at most `first_answer` for the first answer.
/// False when the router did not forward the port. *Agent decision:* the
/// command is explicit, so it asks the router whatever `choose` says; it
/// still refuses when `TORE_NO_PORT_MAPPING` is set.
pub fn map_port(
    forward: Option<Forward>,
    port: u16,
    hold: Duration,
    first_answer: Duration,
    out: &mut impl Write,
) -> std::io::Result<bool> {
    let Some(forward) = forward else {
        writeln!(
            out,
            "Port mapping is turned off here ({} is set).",
            tore_net::portmap::keeper::NO_MAPPING_ENV
        )?;
        return Ok(false);
    };
    writeln!(out, "Asking the router to forward UDP port {port}...")?;
    let mut keeper = Forwarder::start(forward, port);
    let mut mapped = None;
    let started = Instant::now();
    let mut ends = started + first_answer;
    while Instant::now() < ends {
        for news in keeper.poll() {
            for line in news.lines() {
                writeln!(out, "{line}")?;
            }
            if let News::Mapped { outside, .. } = news {
                if mapped.is_none() {
                    ends = Instant::now() + hold;
                    if outside.is_some() {
                        writeln!(out, "Holding it for {:.0} seconds.", hold.as_secs_f64())?;
                    }
                }
                mapped = Some(outside.is_some());
            }
        }
        if mapped == Some(false) {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    keeper.finish(REMOVE_WAIT);
    for news in keeper.poll() {
        for line in news.lines() {
            writeln!(out, "{line}")?;
        }
    }
    if mapped == Some(true) {
        writeln!(out, "The port forward was removed.")?;
    } else if mapped.is_none() {
        writeln!(out, "The router gave no answer in time.")?;
    }
    Ok(mapped == Some(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `--map-port` against fakes on loopback: it prints the result in the
    /// player's words, holds, and leaves the router's table empty; a router
    /// that refuses is reported and the command says it failed.
    #[test]
    fn map_port_prints_holds_and_removes() {
        use tore_net::portmap::MapperConfig;
        use tore_net::portmap::fake::{FakeGateway, FakeGatewayConfig};
        let mapper = |router: &FakeGateway| {
            Forward::with_config(MapperConfig {
                upnp: false,
                gateway: Some(router.address()),
                gateway_v6: Some("[::1]:9".parse().unwrap()),
                entropy: tore_net::Entropy::Seeded(1),
                ..MapperConfig::new(0)
            })
        };
        let router =
            FakeGateway::start("127.0.0.1:0".parse().unwrap(), FakeGatewayConfig::default())
                .unwrap();
        let mut out = Vec::new();
        let ok = map_port(
            Some(mapper(&router)),
            31_005,
            Duration::from_millis(300),
            Duration::from_secs(8),
            &mut out,
        )
        .unwrap();
        assert!(ok);
        assert_eq!(
            String::from_utf8(out).unwrap(),
            "Asking the router to forward UDP port 31005...\n\
             Your router forwards UDP port 31005 (PCP). Friends can join at 203.0.113.5:31005.\n\
             Holding it for 0 seconds.\n\
             The port forward was removed.\n"
        );
        assert!(router.mappings().is_empty());
        let refusing = FakeGateway::start(
            "127.0.0.1:0".parse().unwrap(),
            FakeGatewayConfig {
                refuse: Some(2),
                ..FakeGatewayConfig::default()
            },
        )
        .unwrap();
        let mut out = Vec::new();
        let ok = map_port(
            Some(mapper(&refusing)),
            31_006,
            Duration::from_millis(300),
            Duration::from_secs(8),
            &mut out,
        )
        .unwrap();
        assert!(!ok);
        let text = String::from_utf8(out).unwrap();
        assert!(
            text.contains("Your router refused to forward the port"),
            "{text}"
        );
        assert!(!text.contains("Holding"), "{text}");
        // Vetoed: nothing is asked.
        let mut out = Vec::new();
        assert!(!map_port(None, 1, Duration::ZERO, Duration::ZERO, &mut out).unwrap());
        assert!(
            String::from_utf8(out)
                .unwrap()
                .contains("TORE_NO_PORT_MAPPING")
        );
    }

    /// The hosts the tests build ask no router: `HostThread::start` and
    /// `start_listed` pass no mapper, and the game's own choice is `None` in
    /// a test build even with the switch on.
    #[test]
    fn a_test_build_never_chooses_the_real_router() {
        assert!(choose(true).is_none());
        assert!(choose(false).is_none());
    }
}
