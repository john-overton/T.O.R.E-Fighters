//! Bug B1: the radar page of a joined game said NOT INSTALLED in the busy
//! part of a fight when the game's acknowledgements came back over a second
//! late (a debug build's three-second picture stalls, round trips of 0.6 to
//! 1.3 s). The host then codes every readout against the empty readout, and
//! the sensor flags wait behind the parts before them. Reads a real import
//! through `TORE_DATA_DIR` (the guide's mission, as `net-window-datalink-cues`
//! flies it); about 3 s in a release build, a minute in a debug one.
//!
//! Slice B2 adds the contacts at 60 snapshots a second on 300 and 600 ms
//! round trips with stalls (about 6 s more in release).
//!
//! ```sh
//! TORE_DATA_DIR=$PWD/.local/mpb-data-<topic> cargo test --release -p tore-session \
//!     --lib radar_page -- --ignored --nocapture
//! ```

use super::tests::Rig;
use super::*;
use tore_net::sim::LinkConfig;
use tore_sim::sensors::Channel;

/// The example mission of docs/DEDICATED-SERVER.md.
const GUIDE: &str = "tore-mission 1
theater UKR
condition clear
start airborne 20000
separation-nm 20
preset free
guns-only no
wing friendly 1 F18.PT 4 experienced
wing friendly 2 F14.PT 2 average
wing enemy 1 MIG29.PT 4 experienced
wing enemy 2 SU27.PT 2 ace
survive friendly 2 yes
objective enemy 2 intercept friendly 1
cheats none
";

#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; full-suite run (B1)"]
fn the_radar_stays_installed_through_stalls_on_a_slow_round_trip() {
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = tore_import::load(&directory).expect("an imported pack");
    let spec = tore_world::mission::MissionSpec::from_text(GUIDE).unwrap();
    let mut rig = Rig::with_import(
        spec,
        LinkConfig::for_round_trip(Duration::from_millis(1_100), 0., 0., 0.),
        21,
        resources,
        |_| {},
    );
    // Two of the first wing, an F/A-18D, behind the AI lead.
    let player = rig.join(
        |c| c.plane = Some(1),
        Box::new(|_, _, _| Controls::default()),
    );
    assert!(rig.run_until(Duration::from_secs(10), |r| r.seated(player)));
    rig.run(Duration::from_secs(1));
    let mut checked = 0;
    let mut empty_restarts = 0;
    for _ in 0..6 {
        // A picture's stall, then two seconds of flying.
        rig.players[player].stalled = true;
        rig.run(Duration::from_secs(3));
        rig.players[player].stalled = false;
        for _ in 0..10 {
            rig.run(Duration::from_millis(200));
            let now = rig.net.now();
            let frame = rig.players[player].client.frame(now).expect("a frame");
            let readout = frame.readout.expect("a readout");
            let wire = rig.players[player].client.wire.as_ref().unwrap();
            let (_, held) = wire.readout.latest().unwrap();
            let held = held.readout(0, None, None).unwrap();
            if !held.sensors.available(Channel::Radar) {
                empty_restarts += 1;
            }
            assert!(
                readout.sensors.available(Channel::Radar),
                "host tick {}: the radar page says NOT INSTALLED",
                rig.host.world().tick()
            );
            checked += 1;
        }
    }
    eprintln!("{checked} frames checked, {empty_restarts} with the sensor flags still waiting");
    assert!(
        empty_restarts > 0,
        "the case arose: the held readout's sensor flags waited"
    );
}

/// Frames sampled after each stall, and the radar contacts they showed.
#[derive(Debug, Default)]
struct Contacts {
    checked: usize,
    /// Frames whose host readout for the player had contacts.
    busy: usize,
    /// Contacts in the host's readouts, summed over the frames.
    host: usize,
    /// Of those, contacts the cockpit's readout showed too, by id.
    shown: usize,
    /// The same two from 1.2 seconds after each stall on.
    settled_host: usize,
    settled_shown: usize,
}

/// The guide's mission as Two at 60 snapshots a second on a `round_trip`
/// link, with `stalls` stalls of the player's game of `stall` each, then two
/// seconds of flying sampled every 200 ms.
fn radar_contacts(round_trip: Duration, stall: Duration, stalls: usize) -> Contacts {
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = tore_import::load(&directory).expect("an imported pack");
    let spec = tore_world::mission::MissionSpec::from_text(GUIDE).unwrap();
    let mut rig = Rig::with_import(
        spec,
        LinkConfig::for_round_trip(round_trip, 0., 0., 0.),
        21,
        resources,
        |config| config.snapshot_rate = 60,
    );
    let player = rig.join(
        |c| c.plane = Some(1),
        Box::new(|_, _, _| Controls::default()),
    );
    assert!(rig.run_until(Duration::from_secs(10), |r| r.seated(player)));
    rig.run(Duration::from_secs(2));
    let mut counts = Contacts::default();
    for _ in 0..stalls {
        rig.players[player].stalled = true;
        rig.run(stall);
        rig.players[player].stalled = false;
        for sample in 0..10 {
            rig.run(Duration::from_millis(200));
            let now = rig.net.now();
            let frame = rig.players[player].client.frame(now).expect("a frame");
            let shown = frame.readout.expect("a readout");
            let world = rig.host.world();
            let cockpit = world
                .cockpits
                .iter()
                .find(|c| c.plane.0 == 1)
                .expect("Two flies");
            let host = world
                .combat
                .cockpit_readout(
                    1,
                    tore_world::combat::launcher(&cockpit.flight),
                    world.ai_wings.as_ref(),
                    Some(cockpit),
                )
                .expect("Two's readout");
            counts.checked += 1;
            counts.busy += usize::from(!host.sensors.contacts.is_empty());
            for contact in &host.sensors.contacts {
                let seen = usize::from(shown.sensors.contact(contact.id).is_some());
                counts.host += 1;
                counts.shown += seen;
                if sample >= 5 {
                    counts.settled_host += 1;
                    counts.settled_shown += seen;
                }
            }
        }
    }
    counts
}

/// Slice B2 (B1's follow-up): at 60 snapshots a second a 600 ms round trip
/// puts every acknowledgement over 31 snapshots late, and a 300 ms one with
/// a stall does after it. Before protocol 16 the host then coded every
/// readout against the empty readout, and in this fight the radar's
/// contacts did not all reach the cockpit: before protocol 16, 273 of 280
/// at 300 ms with 1-second stalls, 211 of 280 at 600 ms, and from 1.2
/// seconds after 3-second stalls at 600 ms only 48 of 140. With a window of
/// 127 snapshots (2.1 s) every contact arrives through a stall that the
/// round trip and the stall together keep inside the window; a 3-second
/// stall is beyond it and starts again from empty, so there the contacts
/// must be back from 1.2 seconds after it (all of them, at both round
/// trips).
#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; full-suite run (B2)"]
fn the_radar_shows_contacts_on_a_slow_round_trip_at_60_a_second() {
    let mut failures = Vec::new();
    for (round_trip, stall) in [(300, 1_000), (600, 1_000), (300, 3_000), (600, 3_000)] {
        let counts = radar_contacts(
            Duration::from_millis(round_trip),
            Duration::from_millis(stall),
            4,
        );
        eprintln!("round trip {round_trip} ms, stalls of {stall} ms: {counts:?}");
        assert!(
            counts.busy * 2 >= counts.checked,
            "{round_trip} ms: a busy fight, the host's radar holds contacts"
        );
        let (shown, host) = if stall < 2_000 {
            (counts.shown, counts.host)
        } else {
            (counts.settled_shown, counts.settled_host)
        };
        // Every one: the flight is seeded, and the build showed all of them.
        if shown * 100 < host * 99 {
            failures.push(format!(
                "{round_trip} ms, stalls of {stall} ms: {shown} of the host's {host} contacts \
                 on the radar page"
            ));
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
