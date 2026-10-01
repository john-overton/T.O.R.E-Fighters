//! The host's answer to "who is hosting here?" (slice EF5): a game's summary
//! built from the lobby's state, for the game list of someone who has not
//! joined. The packets are `tore-net`'s (kinds 8 and 9, outside the versioned
//! messages); this is the one place the host fills one in.

use super::{Host, Life, Stage};
use std::net::SocketAddr;
use tore_net::packet::{Discover, DiscoverAnswer, DiscoverPhase};

impl Host {
    /// Queues the answer to a discover query from `from`, in every phase.
    pub(super) fn answer_discover(&mut self, from: SocketAddr, query: &Discover) {
        let answer = self.discover_answer(query.nonce);
        self.server.answer_discover(from, answer);
    }

    /// What a discover query with `nonce` is answered: the game's name and
    /// mission, its players and capacity, the King, whether it needs a
    /// password, and the phase. The transport fits it to the query's length.
    pub fn discover_answer(&self, nonce: u64) -> DiscoverAnswer {
        let mut peers: Vec<_> = self
            .peers
            .values()
            .filter(|peer| !matches!(peer.stage, Stage::Closing { .. }))
            .collect();
        peers.sort_by_key(|peer| peer.lobby.order);
        let players = peers.len();
        let capacity = self.capacity();
        let phase = match self.life {
            Life::Lobby => DiscoverPhase::Lobby,
            Life::Flying => DiscoverPhase::Flying,
            Life::Ended { .. } | Life::Stopped => DiscoverPhase::Closed,
        };
        DiscoverAnswer {
            nonce,
            protocol_version: crate::wire::PROTOCOL_VERSION,
            game_version: self.config.build.version.clone(),
            game_commit: self.config.build.commit.clone(),
            session_id: self.session_id,
            name: self.config.name.clone(),
            summary: self.spec.summary(),
            players: u8::try_from(players).unwrap_or(u8::MAX),
            capacity: u8::try_from(capacity).unwrap_or(u8::MAX),
            password: self.config.password.is_some(),
            full: players >= capacity,
            phase,
            king: peers
                .iter()
                .find(|peer| peer.king)
                .map(|peer| peer.callsign.clone())
                .unwrap_or_default(),
            callsigns: peers.iter().map(|peer| peer.callsign.clone()).collect(),
            truncated: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::PROTOCOL_VERSION;
    use std::sync::Arc;
    use std::time::Duration;
    use tore_formats::aircraft::AircraftId;
    use tore_net::packet::{DISCOVER_LEN, Packet};
    use tore_net::{Client, ClientConfig, Entropy};
    use tore_world::mission::{MissionSpec, Skill, Start};
    use tore_world::test_support::resources::{THEATER, resources};

    use super::super::{BuildId, HostConfig, OpenPlanes, StartMode};

    const MS: Duration = Duration::from_millis(1);

    fn build() -> BuildId {
        BuildId {
            version: "0.1.3-1-gtest".into(),
            commit: "test-commit".into(),
            release: false,
        }
    }

    fn spec(wings: usize) -> MissionSpec {
        let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
        for wing in 0..wings {
            spec.wings[wing].count = 5;
            spec.wings[wing].skill = Skill::Average;
        }
        spec.wings[3].count = 2;
        spec.separation_nm = 20;
        spec.start = Start::Airborne {
            altitude_ft: 10_000,
        };
        spec
    }

    fn host_address() -> SocketAddr {
        "10.0.0.1:26900".parse().unwrap()
    }

    fn host(spec: MissionSpec, configure: impl FnOnce(&mut HostConfig)) -> Host {
        let mut config = HostConfig {
            name: "Friday night".into(),
            entropy: Entropy::Seeded(11),
            ..HostConfig::new(build())
        };
        configure(&mut config);
        Host::new(spec, Arc::new(resources()), config).unwrap()
    }

    /// Players joined to `host` by the transport's handshake alone, their
    /// callsigns in joining order.
    fn join(host: &mut Host, callsigns: &[String]) -> Vec<Client> {
        let build = host.config().build.clone();
        let password = host.config().password.clone().unwrap_or_default();
        let mut clients: Vec<(SocketAddr, Client)> = callsigns
            .iter()
            .enumerate()
            .map(|(i, callsign)| {
                let address: SocketAddr =
                    format!("10.0.1.{}:{}", i + 1, 40_000 + i).parse().unwrap();
                let config = ClientConfig {
                    game_version: build.version.clone(),
                    game_commit: build.commit.clone(),
                    password: password.clone(),
                    entropy: Entropy::Seeded(100 + i as u64),
                    ..ClientConfig::new(PROTOCOL_VERSION, callsign)
                };
                (
                    address,
                    Client::connect(config, host_address(), Duration::ZERO).unwrap(),
                )
            })
            .collect();
        for step in 1..=60u32 {
            let now = step * 5 * MS;
            for (address, client) in &mut clients {
                client.update(now);
                while let Some(transmit) = client.poll_transmit() {
                    host.receive(now, *address, &transmit.datagram);
                }
            }
            host.update(now);
            while let Some(transmit) = host.poll_transmit() {
                if let Some((_, client)) = clients.iter_mut().find(|(a, _)| *a == transmit.to) {
                    client.receive(now, host_address(), &transmit.datagram);
                }
            }
        }
        clients.into_iter().map(|(_, client)| client).collect()
    }

    fn query(version: u16, nonce: u64) -> Vec<u8> {
        Packet::Discover(tore_net::packet::Discover {
            protocol_version: version,
            nonce,
        })
        .encode(version)
        .unwrap()
    }

    /// Asks the host over the wire, as a stranger would, and decodes what it
    /// sends back to the asker.
    fn ask(host: &mut Host, version: u16, nonce: u64) -> (usize, DiscoverAnswer) {
        let asker: SocketAddr = "10.0.9.9:5000".parse().unwrap();
        let query = query(version, nonce);
        host.receive(Duration::from_secs(1), asker, &query);
        let sent = host.poll_transmit().expect("an answer");
        assert_eq!(sent.to, asker);
        assert!(
            sent.datagram.len() <= query.len(),
            "{} bytes against a query of {}",
            sent.datagram.len(),
            query.len()
        );
        let Ok(Packet::DiscoverAnswer(answer)) = Packet::decode(&sent.datagram, PROTOCOL_VERSION)
        else {
            panic!("not an answer")
        };
        (sent.datagram.len(), answer)
    }

    #[test]
    fn an_empty_game_answers_with_its_summary() {
        let mission = spec(1);
        let summary = mission.summary();
        let mut host = host(mission, |_| {});
        let (_, answer) = ask(&mut host, PROTOCOL_VERSION, 77);
        assert_eq!(answer.nonce, 77);
        assert_eq!(answer.protocol_version, PROTOCOL_VERSION);
        assert_eq!(
            (answer.game_version.as_str(), answer.game_commit.as_str()),
            ("0.1.3-1-gtest", "test-commit")
        );
        assert_eq!(
            (answer.name.as_str(), answer.summary),
            ("Friday night", summary)
        );
        assert_eq!((answer.players, answer.capacity), (0, 5));
        assert_eq!(answer.phase, DiscoverPhase::Lobby);
        assert!(!answer.password && !answer.full && !answer.truncated);
        assert!(answer.king.is_empty() && answer.callsigns.is_empty());
        assert_ne!(answer.session_id, 0);
    }

    #[test]
    fn players_the_king_and_the_password_are_listed_in_joining_order() {
        let mut host = host(spec(1), |config| {
            config.king = Some("10.0.1.1:40000".parse().unwrap());
            config.start = StartMode::King;
            config.password = Some("secret".into());
            config.max_players = 2;
        });
        let _clients = join(&mut host, &["Viper".into(), "Maverick 1".into()]);
        let (_, answer) = ask(&mut host, PROTOCOL_VERSION, 1);
        assert_eq!(answer.callsigns, ["Viper", "Maverick 1"]);
        assert_eq!(answer.king, "Viper");
        assert_eq!((answer.players, answer.capacity), (2, 2));
        assert!(answer.password && answer.full);
    }

    #[test]
    fn a_later_build_is_answered_and_every_phase_answers() {
        let mut host = host(spec(1), |config| config.start = StartMode::Now);
        // The asker is of another protocol version: still answered, and the
        // answer says which version this host is.
        let (_, answer) = ask(&mut host, PROTOCOL_VERSION + 5, 3);
        assert_eq!(answer.protocol_version, PROTOCOL_VERSION);
        assert_eq!(answer.phase, DiscoverPhase::Flying);
        host.end();
        let (_, answer) = ask(&mut host, PROTOCOL_VERSION, 4);
        assert_eq!(answer.phase, DiscoverPhase::Closed);
        host.stop();
        let (_, answer) = ask(&mut host, PROTOCOL_VERSION, 5);
        assert_eq!(answer.phase, DiscoverPhase::Closed);
    }

    #[test]
    fn the_largest_lobby_is_answered_in_full_within_its_query() {
        // 30 players at 15 characters each, the longest game name and the
        // longest build texts.
        let mut mission = spec(3);
        for wing in 3..6 {
            mission.wings[wing].count = 5;
            mission.wings[wing].skill = Skill::Average;
        }
        let mut host = host(mission, |config| {
            config.name = "n".repeat(64);
            config.open_planes = OpenPlanes::All;
            config.build.version = "9".repeat(40);
            config.build.commit = "f".repeat(40);
        });
        let callsigns: Vec<String> = (0..30).map(|i| format!("Callsign_{i:06}")).collect();
        let _clients = join(&mut host, &callsigns);
        let (size, answer) = ask(&mut host, PROTOCOL_VERSION, 9);
        assert_eq!(answer.players, 30);
        assert_eq!(answer.callsigns, callsigns);
        assert!(!answer.truncated);
        assert!(size <= DISCOVER_LEN, "{size} bytes");
    }
}
