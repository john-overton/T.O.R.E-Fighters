//! A host and players on the network simulator, stepped on its virtual clock.

#![allow(dead_code)]

use std::net::SocketAddr;
use std::time::Duration;

use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_net::{
    AcceptInfo, Client, ClientConfig, ClientEvent, ConnectDetails, ConnectionId, Decision, Entropy,
    Gate, RefuseReason, Server, ServerConfig, ServerEvent,
};

pub const VERSION: u16 = 1;
pub const MS: Duration = Duration::from_millis(1);

pub fn host_addr() -> SocketAddr {
    SocketAddr::from(([10, 0, 0, 1], 26_900))
}

pub fn player_addr(n: u16) -> SocketAddr {
    SocketAddr::from(([10, 0, 1, (n % 250) as u8 + 1], 40_000 + n))
}

/// Accepts every join with the right password; can refuse by rule and can
/// reject sections of one kind.
#[derive(Debug, Default)]
pub struct TestGate {
    pub password: String,
    pub seen: Vec<ConnectDetails>,
    pub reject_section_kind: Option<u8>,
    pub checked: usize,
}

impl Gate for TestGate {
    fn accept(&mut self, details: &ConnectDetails) -> Decision {
        self.seen.push(details.clone());
        if details.password != self.password {
            return Decision::Refuse {
                reason: RefuseReason::WrongPassword,
                text: "Wrong password.".into(),
            };
        }
        Decision::Accept(AcceptInfo {
            session_id: 77,
            ticks_per_second: 120,
            ticks_per_snapshot: 4,
            host_tick: 1_000,
        })
    }

    fn check_section(&mut self, _: ConnectionId, kind: u8, _: &[u8]) -> bool {
        self.checked += 1;
        self.reject_section_kind != Some(kind)
    }
}

pub struct Player {
    pub client: Client,
    pub socket: SimSocket,
    pub events: Vec<(Duration, ClientEvent)>,
}

pub struct World {
    pub net: SimNetwork,
    pub server: Server,
    pub host_socket: SimSocket,
    pub host_events: Vec<(Duration, ServerEvent)>,
    pub gate: TestGate,
    pub players: Vec<Player>,
    pub seed: u64,
}

impl World {
    pub fn new(seed: u64, link: LinkConfig) -> Self {
        Self::with_config(seed, link, ServerConfig::new(VERSION))
    }

    pub fn with_config(seed: u64, link: LinkConfig, mut config: ServerConfig) -> Self {
        let net = SimNetwork::new(seed);
        net.set_default_link(link);
        config.entropy = Entropy::Seeded(seed ^ 0x5EED);
        let host_socket = net.bind(host_addr()).unwrap();
        Self {
            net,
            server: Server::new(config),
            host_socket,
            host_events: Vec::new(),
            gate: TestGate::default(),
            players: Vec::new(),
            seed,
        }
    }

    pub fn now(&self) -> Duration {
        self.net.now()
    }

    pub fn player_config(&self, callsign: &str, n: u16) -> ClientConfig {
        ClientConfig {
            game_version: "0.1.3".into(),
            game_commit: "5b7b9fd".into(),
            entropy: Entropy::Seeded(self.seed.wrapping_mul(31).wrapping_add(u64::from(n))),
            ..ClientConfig::new(VERSION, callsign)
        }
    }

    /// Starts a join from player address `n`; returns the player's index.
    pub fn join_with(&mut self, config: ClientConfig, n: u16) -> usize {
        let socket = self.net.bind(player_addr(n)).unwrap();
        let client = Client::connect(config, host_addr(), self.now()).unwrap();
        self.players.push(Player {
            client,
            socket,
            events: Vec::new(),
        });
        self.players.len() - 1
    }

    pub fn join(&mut self, callsign: &str) -> usize {
        let n = self.players.len() as u16;
        let config = self.player_config(callsign, n);
        self.join_with(config, n)
    }

    /// One step: the clock moves by `step`, then the host and each player
    /// read, update and send, in that order.
    pub fn step(&mut self, step: Duration) {
        self.net.advance(step);
        let now = self.now();
        self.server
            .receive_from(&mut self.host_socket, now, &mut self.gate)
            .unwrap();
        self.server.update(now);
        self.server.transmit(&mut self.host_socket).unwrap();
        while let Some(event) = self.server.poll_event() {
            self.host_events.push((now, event));
        }
        for p in &mut self.players {
            p.client.receive_from(&mut p.socket, now).unwrap();
            p.client.update(now);
            p.client.transmit(&mut p.socket).unwrap();
            while let Some(event) = p.client.poll_event() {
                p.events.push((now, event));
            }
        }
    }

    /// Steps 1 ms at a time until `done` or `limit` of virtual time passes.
    pub fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&World) -> bool) -> bool {
        let end = self.now() + limit;
        while self.now() < end {
            self.step(MS);
            if done(self) {
                return true;
            }
        }
        false
    }

    pub fn run_for(&mut self, time: Duration) {
        let end = self.now() + time;
        while self.now() < end {
            self.step(MS);
        }
    }

    pub fn connected(&self, player: usize) -> bool {
        self.players[player]
            .events
            .iter()
            .any(|(_, e)| matches!(e, ClientEvent::Connected(_)))
    }

    pub fn connection_of(&self, player: usize) -> ConnectionId {
        self.players[player].client.welcome().unwrap().connection
    }
}
