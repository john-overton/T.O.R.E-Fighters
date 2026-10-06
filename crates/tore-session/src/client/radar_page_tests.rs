//! Bug B1: the radar page of a joined game said NOT INSTALLED in the busy
//! part of a fight when the game's acknowledgements came back over a second
//! late (a debug build's three-second picture stalls, round trips of 0.6 to
//! 1.3 s). The host then codes every readout against the empty readout, and
//! the sensor flags wait behind the parts before them. Reads a real import
//! through `TORE_DATA_DIR` (the guide's mission, as `net-window-datalink-cues`
//! flies it); about 3 s in a release build, a minute in a debug one.
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
