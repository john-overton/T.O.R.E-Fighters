//! Slice K4's client tests (docs/ARCHITECTURE.md, "On the client"), on the
//! network simulator with one host and its client: detection with no
//! standby to race, a race that finds another session's host and gives the
//! session up with the design's words, and the old host answering again,
//! which stops the race. The takeover itself is tested with the host's
//! (`host/resume_tests.rs`). Synthetic resources.

use super::migrate::{MigrationState, words};
use super::tests::{Rig, build, host_address, level_script, spec};
use super::*;
use crate::host::{Host, HostConfig};
use crate::wire::migration::{Succession, Successor};
use tore_net::master::candidate::{Candidate, CandidateKind};
use tore_net::sim::LinkConfig;

const MS: Duration = Duration::from_millis(1);

fn link() -> LinkConfig {
    LinkConfig::for_round_trip(40 * MS, 0., 0., 0.)
}

fn lost() -> LinkConfig {
    LinkConfig { loss: 1., ..link() }
}

/// A host and one flying player.
fn flying() -> (Rig, usize) {
    let mut rig = Rig::with_config(spec(2, 2, 20), link(), 23, |config| {
        config.start = crate::host::StartMode::Now;
    });
    let viper = rig.join(|_| {}, level_script());
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(viper)));
    rig.run(Duration::from_secs(1));
    (rig, viper)
}

/// A Succession naming one standby, lobby id 9, at `address`.
fn succession(address: SocketAddr) -> Message {
    Message::Succession(Box::new(Succession {
        standbys: vec![Successor {
            player: 9,
            warm: true,
            addresses: vec![Candidate::new(CandidateKind::Local, address)],
        }],
    }))
}

fn notices(rig: &Rig, player: usize) -> Vec<String> {
    rig.players[player]
        .events
        .iter()
        .filter_map(|e| match e {
            ClientEvent::Notice(text) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_client_with_no_standby_to_race_waits_out_the_timeout_as_before() {
    let (mut rig, viper) = flying();
    rig.net
        .set_link_both(host_address(), Rig::player_address(viper), lost());
    let cut = rig.net.now();
    rig.run(Duration::from_secs(2));
    assert_eq!(
        rig.players[viper].client.migration(),
        MigrationState::Steady,
        "nothing to race"
    );
    assert!(rig.run_until(Duration::from_secs(4), |r| r.closed(viper)));
    let closed = rig.net.now() - cut;
    assert!(closed >= Duration::from_secs(5) && closed < Duration::from_millis(5_200));
    assert!(notices(&rig, viper).is_empty());
}

#[test]
fn a_race_that_finds_another_session_gives_the_session_up_in_words() {
    let (mut rig, viper) = flying();
    // Another game's host where the succession says the standby is.
    let other: SocketAddr = "10.0.0.5:26900".parse().unwrap();
    let mut socket = rig.net.bind(other).unwrap();
    let mut stranger = Host::new(
        spec(2, 2, 20),
        Arc::clone(&rig.resources),
        HostConfig {
            entropy: Entropy::Seeded(99),
            ..HostConfig::new(build())
        },
    )
    .unwrap();
    rig.players[viper].client.message(succession(other));
    rig.net
        .set_link_both(host_address(), Rig::player_address(viper), lost());
    let cut = rig.net.now();
    let mut lost_at = None;
    while rig.net.now() < cut + Duration::from_secs(18) && !rig.closed(viper) {
        rig.step();
        let now = rig.net.now();
        stranger.receive_from(now, &mut socket).unwrap();
        stranger.update(now);
        stranger.transmit(&mut socket).unwrap();
        if lost_at.is_none()
            && matches!(
                rig.players[viper].client.migration(),
                MigrationState::Racing { .. }
            )
        {
            lost_at = Some(now);
        }
    }
    let lost_at = lost_at.expect("the race starts");
    let detected = lost_at - cut;
    assert!(
        detected >= Duration::from_millis(1_480) && detected < Duration::from_millis(1_600),
        "{detected:?}"
    );
    assert!(rig.closed(viper), "the session is given up");
    let gave_up = rig.net.now() - lost_at;
    assert!(
        gave_up >= Duration::from_secs(15) && gave_up < Duration::from_millis(15_100),
        "{gave_up:?}"
    );
    let said = notices(&rig, viper);
    assert!(said.contains(&words::lost("player 9")), "{said:?}");
    assert!(said.contains(&words::GAVE_UP.to_owned()), "{said:?}");
    let counts = rig.players[viper].client.migration_counts();
    assert_eq!((counts.resumed, counts.failed), (0, 1));
    assert!(
        rig.players[viper]
            .events
            .iter()
            .any(|e| matches!(e, ClientEvent::Closed(_)))
    );
}

#[test]
fn the_old_host_answering_again_stops_the_race() {
    let (mut rig, viper) = flying();
    // Nothing answers at the standby's address.
    rig.players[viper]
        .client
        .message(succession("10.0.0.6:26900".parse().unwrap()));
    rig.net
        .set_link_both(host_address(), Rig::player_address(viper), lost());
    assert!(rig.run_until(Duration::from_secs(2), |r| matches!(
        r.players[viper].client.migration(),
        MigrationState::Racing { .. }
    )));
    rig.net
        .set_link_both(host_address(), Rig::player_address(viper), link());
    assert!(
        rig.run_until(Duration::from_secs(1), |r| r.players[viper]
            .client
            .migration()
            == MigrationState::Steady),
        "the race stops"
    );
    rig.run(Duration::from_secs(5));
    assert!(rig.seated(viper), "it flies on with the old host");
    assert_eq!(rig.players[viper].client.server(), host_address());
    assert_eq!(rig.players[viper].client.migration_counts().failed, 0);
}

/// A new host's Names message that comes before any section of its flight
/// starts that flight, as the flight's first section would: its names are
/// kept, so the next Names follows on instead of being refused as out of
/// order (the client's half of the flaky `net-window-migrate`; the host's
/// half is `a_late_resume_gets_no_snapshot_before_resumed`).
#[test]
fn names_of_a_later_flight_start_it_after_a_switch() {
    use crate::wire::connection::ClientConnection;
    use crate::wire::messages::Names;
    use crate::wire::names::NameIndex;
    let (mut rig, viper) = flying();
    let client = &mut rig.players[viper].client;
    // The wire as `switch` leaves it: a new connection's, with no flight.
    client.wire = Some(ClientConnection::new(client.ticks_per_snapshot));
    let names = |first: u16, name: &str| {
        Message::Names(Names {
            flight: 0,
            first,
            names: vec![name.to_owned()],
        })
    };
    client.message(names(0, "AIM9M.JT"));
    client.message(names(1, "AIM120.JT"));
    let wire = client.wire.as_ref().unwrap();
    assert_eq!(wire.flight, Some(0));
    assert_eq!(wire.names.name(NameIndex(1)), Some("AIM120.JT"));
    assert_eq!(client.phase(), ClientPhase::Flying, "no protocol error");
}
