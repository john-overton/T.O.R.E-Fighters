//! A synthetic networked fight for tests in other crates (feature
//! `test-support`): a host on the network simulator, a mission built from the
//! synthetic resources and two bots that join, fly, fire and leave. The first
//! bot's capture and diagnostics log come out, so a game's tests can convert a
//! capture without a server.

use crate::bot::Bot;
use crate::host::{BuildId, Host, HostConfig};
use crate::{Client, ClientConfig, ClientPhase};
use std::collections::BTreeMap;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tore_formats::aircraft::AircraftId;
use tore_net::Entropy;
use tore_net::sim::{LinkConfig, SimNetwork};
use tore_world::mission::{MissionSpec, Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

/// What a fight left behind.
pub struct Fight {
    /// The first bot's capture, complete.
    pub capture: Vec<u8>,
    /// The first bot's diagnostics log.
    pub log: String,
    /// The synthetic import the capture needs to run again.
    pub resources: Arc<BTreeMap<String, Vec<u8>>>,
    /// The plane the first bot flew.
    pub plane: u32,
}

#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("poisoned"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn build() -> BuildId {
    BuildId {
        version: "0.1.3-1-gtest".into(),
        commit: "test-commit".into(),
        release: false,
    }
}

/// Two bots (callsigns Alpha and Bravo) fly two against two for `seconds`
/// after both are seated, over a link with a 150 ms round trip and 2 percent
/// loss, and leave with their debriefs.
pub fn bot_fight(seconds: u64) -> Fight {
    bot_fight_at(seconds, 10_000)
}

/// [`bot_fight`] starting `altitude_ft` high: above about 35,000 feet every
/// aircraft leaves contrails.
pub fn bot_fight_at(seconds: u64, altitude_ft: u32) -> Fight {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 2;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 2;
    spec.start = Start::Airborne { altitude_ft };
    let net = SimNetwork::new(21);
    net.set_default_link(LinkConfig::for_round_trip(
        Duration::from_millis(150),
        0.1,
        0.02,
        0.01,
    ));
    let resources = Arc::new(resources());
    let host_address = "10.0.0.1:26900".parse().expect("an address");
    let mut host_socket = net.bind(host_address).expect("a socket");
    let mut host = Host::new(
        spec,
        Arc::clone(&resources),
        HostConfig {
            entropy: Entropy::Seeded(11),
            ..HostConfig::new(build())
        },
    )
    .expect("a host");
    let capture = Shared::default();
    let log = Shared::default();
    let mut bots = Vec::new();
    for (n, callsign) in ["Alpha", "Bravo"].into_iter().enumerate() {
        let port = 40_000 + n as u16;
        let socket = net
            .bind(format!("10.0.0.2:{port}").parse().expect("an address"))
            .expect("a socket");
        let config = ClientConfig {
            entropy: Entropy::Seeded(u64::from(port)),
            ..ClientConfig::new(host_address, callsign, build())
        };
        let mut client =
            Client::connect(config, Arc::clone(&resources), net.now()).expect("a client");
        if n == 0 {
            client.set_capture(Box::new(capture.clone()));
            client.set_diagnostics(Box::new(log.clone()));
        }
        bots.push((Bot::new(client), socket));
    }
    let mut step = |bots: &mut Vec<(Bot, tore_net::sim::SimSocket)>, host: &mut Host| {
        net.advance(Duration::from_millis(1));
        let now = net.now();
        host.receive_from(now, &mut host_socket).expect("receive");
        host.update(now);
        host.transmit(&mut host_socket).expect("transmit");
        while host.poll_log().is_some() {}
        for (bot, socket) in bots.iter_mut() {
            bot.client.receive_from(now, socket).expect("receive");
            bot.update(now);
            bot.client.transmit(socket).expect("transmit");
            while bot.client.poll_event().is_some() {}
        }
    };
    let seated = |bots: &Vec<(Bot, tore_net::sim::SimSocket)>| {
        bots.iter()
            .all(|(bot, _)| bot.client.phase() == ClientPhase::Flying)
    };
    let mut waited = 0;
    while !seated(&bots) && waited < 10_000 {
        step(&mut bots, &mut host);
        waited += 1;
    }
    assert!(seated(&bots), "both bots were seated");
    let plane = bots[0].0.client.seat().expect("seated").1.0;
    for _ in 0..seconds * 1000 {
        step(&mut bots, &mut host);
    }
    for (bot, _) in bots.iter_mut() {
        let now = net.now();
        bot.client.leave_game(now);
    }
    let mut waited = 0;
    while bots
        .iter()
        .any(|(bot, _)| bot.client.phase() != ClientPhase::Closed)
        && waited < 8_000
    {
        step(&mut bots, &mut host);
        waited += 1;
    }
    drop(bots);
    let capture = capture.0.lock().expect("the capture").clone();
    let log = String::from_utf8(log.0.lock().expect("the log").clone()).expect("text");
    Fight {
        capture,
        log,
        resources,
        plane,
    }
}
