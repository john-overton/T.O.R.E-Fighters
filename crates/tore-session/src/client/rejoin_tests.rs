//! The client's side of rejoin (slice K5) against a real host on the
//! network simulator: the token the host grants is kept with the session's
//! id and handed to the game's store, again when the connection ends; a game
//! that joins with the token flies its reserved plane again; and one that
//! joined without it sends a Rejoin.

use super::rejoin::{KeptToken, TokenStore};
use super::tests::{Rig, level_script, spec};
use super::*;
use std::sync::{Arc, Mutex};
use tore_net::sim::LinkConfig;
use tore_world::seats::SeatCommand;

const MS: Duration = Duration::from_millis(1);

/// A store that keeps what it is given where the test can read it.
#[derive(Clone, Default)]
struct Shelf(Arc<Mutex<Vec<KeptToken>>>);

impl TokenStore for Shelf {
    fn forget(&mut self, session_id: u64) {
        self.0
            .lock()
            .unwrap()
            .retain(|kept| kept.session_id != session_id);
    }

    fn keep(&mut self, kept: &KeptToken) {
        self.0.lock().unwrap().push(kept.clone());
    }
}

fn rig() -> Rig {
    Rig::new(spec(2, 2, 50), LinkConfig::one_way(10 * MS), 21)
}

fn flier(rig: &mut Rig, callsign: &str, token: Option<tore_net::Token>) -> usize {
    let callsign = callsign.to_owned();
    rig.join(
        move |c| {
            c.callsign = callsign;
            c.token = token;
        },
        level_script(),
    )
}

#[test]
fn the_granted_token_is_kept_with_the_session_and_handed_to_the_store() {
    let mut rig = rig();
    let shelf = Shelf::default();
    let viper = flier(&mut rig, "Viper", None);
    rig.players[viper]
        .client
        .set_token_store(Box::new(shelf.clone()));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let kept = rig.players[viper].client.token().cloned().expect("a token");
    assert_eq!(kept.session_id, rig.host.session_id_for_test());
    assert_eq!(
        (kept.callsign.as_str(), kept.life_seconds),
        ("Viper", 86_400)
    );
    assert_eq!(
        Some(kept.token),
        rig.host.token_of_for_test("Viper"),
        "the host's own record"
    );
    assert_eq!(
        shelf.0.lock().unwrap().as_slice(),
        std::slice::from_ref(&kept)
    );
    // A clean departure clears the token instead of saving it again.
    let now = rig.net.now();
    rig.players[viper].client.leave_game(now);
    assert!(rig.run_until(Duration::from_secs(4), |r| r.closed(viper)));
    let kept_list = shelf.0.lock().unwrap().clone();
    assert!(kept_list.is_empty());
    assert!(rig.players[viper].client.token().is_none());
}

#[test]
fn a_game_that_joins_with_its_token_flies_its_reserved_plane_again() {
    let mut rig = rig();
    let viper = flier(&mut rig, "Viper", None);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let plane = rig.players[viper].client.seat().map(|(_, p)| p.0).unwrap();
    let token = rig.players[viper].client.token().unwrap().token;
    // Its game stalls for good: the host drops it after 5 seconds.
    rig.players[viper].stalled = true;
    assert!(rig.run_until(Duration::from_secs(8), |r| { r.host.peers_for_test() == 0 }));
    rig.run(Duration::from_millis(200));
    let back = flier(&mut rig, "Viper", Some(token));
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(back)));
    assert_eq!(
        rig.players[back].client.seat().map(|(_, p)| p.0),
        Some(plane)
    );
    assert_eq!(rig.players[back].client.token().unwrap().token, token);
    assert!(rig.players[back].events.iter().any(
        |e| matches!(e, ClientEvent::Notice(text) if text.starts_with("Welcome back, Viper"))
    ));
}

#[test]
fn a_game_that_joined_without_its_token_sends_a_rejoin() {
    let mut rig = rig();
    let viper = flier(&mut rig, "Viper", None);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let plane = rig.players[viper].client.seat().map(|(_, p)| p.0).unwrap();
    let token = rig.players[viper].client.token().unwrap().token;
    rig.players[viper].stalled = true;
    assert!(rig.run_until(Duration::from_secs(8), |r| { r.host.peers_for_test() == 0 }));
    rig.run(Duration::from_millis(200));
    let back = rig.join(
        |c| {
            c.callsign = "Viper".into();
            c.auto_ready = false;
        },
        level_script(),
    );
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[back].client.phase() == ClientPhase::Lobby
    }));
    rig.players[back].client.rejoin(token);
    rig.run(Duration::from_millis(400));
    rig.players[back].client.ready(None);
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(back)));
    assert_eq!(
        rig.players[back].client.seat().map(|(_, p)| p.0),
        Some(plane)
    );
}

/// Protocol 23, slice NA; slice FX-J: a game whose plane pinned its
/// targeting sight, put the A-G page up and set its laser by hand, then
/// stalled for good, joins again with its token. While it was gone the AI
/// flew the plane and the host kept the cockpit (the handoff's kept
/// cockpit), so it comes back to the same pin, the laser on by hand and the
/// A-G page, and its readout says so, the designator pod included.
#[test]
fn a_game_that_rejoins_reads_its_planes_sight_and_laser_as_it_left_them() {
    use tore_sim::combat::laser::LaserState;
    use tore_sim::combat::sight::SightMode;
    let mut spec = crate::wire::readout_a2g_tests::a2g_spec();
    spec.loadout = None;
    spec.wings[0].count = 2;
    let mut import = tore_world::test_support::surface::routed_resources();
    tore_world::test_support::resources::add_a2g_stores(&mut import);
    let mut rig = Rig::with_import(spec, LinkConfig::one_way(10 * MS), 21, import, |_| {});
    rig.host.start_now();
    let script = |run: bool| -> super::tests::Script {
        let mut level = level_script();
        let mut a2g = crate::wire::readout_a2g_tests::A2gPilot::default();
        Box::new(move |now, client, picture| {
            let mut controls = level(now, client, picture);
            if run && client.prediction().is_some() {
                a2g.drive(now, &mut controls);
                // No trigger: the standard load carries no laser bomb.
                controls.trigger = false;
            }
            controls
        })
    };
    let viper = rig.join(|c| c.callsign = "Viper".into(), script(true));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    rig.run(Duration::from_secs(5));
    let plane = rig.players[viper].client.seat().map(|(_, p)| p.0).unwrap();
    let own = rig.host.world().combat.state.ownship(plane).unwrap();
    let (sight, laser) = (own.sight.unwrap(), own.laser);
    assert!(
        matches!(sight.mode, SightMode::Pinned(_)),
        "{:?}",
        sight.mode
    );
    assert_eq!(
        laser.manual,
        Some(true),
        "Ctrl+L with no laser weapon: on by hand"
    );
    let token = rig.players[viper].client.token().unwrap().token;
    rig.players[viper].stalled = true;
    assert!(rig.run_until(Duration::from_secs(8), |r| r.host.peers_for_test() == 0));
    rig.run(Duration::from_millis(200));
    let back = rig.join(
        move |c| {
            c.callsign = "Viper".into();
            c.token = Some(token);
        },
        script(false),
    );
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(back)));
    assert_eq!(
        rig.players[back].client.seat().map(|(_, p)| p.0),
        Some(plane)
    );
    let mut read = None;
    for _ in 0..2_000 {
        rig.step();
        let now = rig.net.now();
        if let Some(readout) = rig.players[back].client.frame(now).and_then(|f| f.readout)
            && readout.sight.is_some()
            && readout.laser.is_some()
        {
            read = Some(readout);
            break;
        }
    }
    let readout = read.expect("the rejoined game reads a sight and a laser");
    let host = rig.host.world().combat.state.ownship(plane).unwrap();
    let (shown, shown_laser) = (readout.sight.unwrap(), readout.laser.unwrap());
    assert_eq!(
        host.sight.map(|s| s.mode),
        Some(sight.mode),
        "the host's pin"
    );
    // The same pin on the client, within the wire's step.
    let (SightMode::Pinned(got), SightMode::Pinned(left)) = (shown.mode, sight.mode) else {
        panic!("the sight comes back pinned: {:?}", shown.mode);
    };
    assert!(
        (0..3).all(|i| (got[i] - left[i]).abs() <= 1.),
        "{got:?} {left:?}"
    );
    // The pod's sensor.
    let (got, sent) = (shown.sensor, sight.sensor);
    assert_eq!(
        (got.laser, got.range_ft, got.gimbal, got.eye),
        (sent.laser, sent.range_ft, sent.gimbal, sent.eye)
    );
    assert!(shown_laser.designator);
    assert_eq!(shown_laser.manual, Some(true), "on by hand, as left");
    assert_eq!(host.laser.manual, Some(true));
    assert_ne!(shown_laser.state, LaserState::Off);
    assert!(readout.sensors.air_ground, "the A-G page, as left");
    assert!(host.sensors.air_ground());
    let _ = laser;
}

/// Slice FX-J: a laser bomber (the synthetic F/A-18D's pod and two GBU-10s,
/// a Cheat loading) releases on its pin and its game stalls for good while
/// the bomb falls. The host drops it and the AI flies the plane, the laser
/// the player left keeps the spot, and the bomb guides on; the game joins
/// again with its token, finds its laser and pin, and the bomb ends on the
/// spot, guiding the whole way.
#[test]
fn a_laser_bomb_guides_on_across_a_drop_and_a_rejoin() {
    use crate::settings::{LoadoutRule, number};
    use tore_sim::combat::live::GuidedMode;
    use tore_sim::combat::sight::SightMode;
    let mut spec = crate::wire::readout_a2g_tests::a2g_spec();
    spec.loadout = None;
    spec.wings[0].count = 2;
    let mut import = tore_world::test_support::surface::routed_resources();
    tore_world::test_support::resources::add_a2g_stores(&mut import);
    let mut rig = Rig::with_import(spec, LinkConfig::one_way(10 * MS), 21, import, |config| {
        config
            .settings
            .push((number::LOADOUTS, LoadoutRule::Any.value()));
    });
    // The run: the A-G page and the laser bomb at once, the sight slewed
    // down for 3.5 s (onto the ground some 18,000 ft ahead, inside the pod's
    // reach in this weather) and pinned with the laser off by hand (the spot
    // is beyond the bomb's release zone, which would leave it blind; with
    // no spot it waits, the release puts the laser back on and it locks as
    // it falls), the trigger at 4.5 s.
    let script = |run: bool| -> super::tests::Script {
        use tore_sim::combat::live::Command as Combat;
        let mut level = level_script();
        let mut since = None;
        let mut given = 0;
        Box::new(move |now, client, picture| {
            let mut controls = level(now, client, picture);
            if !run || client.prediction().is_none() {
                return controls;
            }
            let at = now.saturating_sub(*since.get_or_insert(now));
            if given == 0 {
                controls.commands.extend([
                    SeatCommand::Combat(Combat::ToggleAirGround),
                    SeatCommand::CycleWeapon { forward: true },
                ]);
                given = 1;
            }
            if at < Duration::from_millis(3_500) {
                controls.sight = [0, -127];
            } else if given == 1 {
                controls.commands.extend([
                    SeatCommand::Combat(Combat::SightPinGround),
                    SeatCommand::Combat(Combat::ToggleLaser),
                ]);
                given = 2;
            }
            controls.trigger =
                (Duration::from_millis(4_500)..Duration::from_millis(4_750)).contains(&at);
            controls
        })
    };
    let viper = rig.join(
        |c| {
            c.callsign = "Viper".into();
            c.auto_ready = false;
        },
        script(true),
    );
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[viper].client.lobby().is_some()
    }));
    rig.players[viper].client.take_any_slot();
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[viper]
            .client
            .lobby()
            .and_then(|l| l.me())
            .is_some_and(|me| me.slot.is_some())
    }));
    rig.players[viper]
        .client
        .send_loadout(Some(tore_world::test_support::resources::a2g_loadout()));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[viper]
            .client
            .lobby()
            .and_then(|l| l.me())
            .is_some_and(|me| me.loadout)
    }));
    rig.players[viper].client.set_ready(true);
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(viper)));
    let plane = rig.players[viper].client.seat().map(|(_, p)| p.0).unwrap();
    // The bomb leaves four seconds after the game flies, and locks.
    let guiding = |r: &Rig| {
        r.host
            .world()
            .combat
            .state
            .guided_flights()
            .find(|(_, f)| f.owner == plane)
            .filter(|(_, f)| f.mode == GuidedMode::Guiding)
            .map(|(id, _)| id)
    };
    assert!(
        rig.run_until(Duration::from_secs(8), |r| guiding(r).is_some()),
        "the bomb guides"
    );
    let bomb = guiding(&rig).unwrap();
    let pin = rig
        .host
        .world()
        .combat
        .state
        .ownship(plane)
        .unwrap()
        .sight
        .unwrap()
        .mode;
    assert!(matches!(pin, SightMode::Pinned(_)), "{pin:?}");
    let token = rig.players[viper].client.token().unwrap().token;
    rig.players[viper].stalled = true;
    // Every tick from here the bomb guides, until it ends.
    let mut modes = Vec::new();
    let mut gap = false;
    let watch = |r: &Rig, modes: &mut Vec<GuidedMode>, gap: &mut bool| {
        let state = &r.host.world().combat.state;
        if let Some(flight) = state.guided_flight(bomb) {
            modes.push(flight.mode);
        }
        if state.ownship(plane).is_none() && state.kept(plane).is_some() {
            *gap = true;
        }
    };
    assert!(rig.run_until(Duration::from_secs(8), |r| {
        watch(r, &mut modes, &mut gap);
        r.host.peers_for_test() == 0
    }));
    assert!(
        rig.run_until(Duration::from_secs(1), |r| {
            watch(r, &mut modes, &mut gap);
            gap
        }),
        "the AI flies the plane with the cockpit kept"
    );
    rig.run(Duration::from_millis(200));
    let back = rig.join(
        move |c| {
            c.callsign = "Viper".into();
            c.token = Some(token);
        },
        script(false),
    );
    assert!(rig.run_until(Duration::from_secs(4), |r| {
        watch(r, &mut modes, &mut gap);
        r.seated(back)
    }));
    assert_eq!(
        rig.players[back].client.seat().map(|(_, p)| p.0),
        Some(plane)
    );
    let own = rig.host.world().combat.state.ownship(plane).unwrap();
    assert_eq!(own.sight.map(|s| s.mode), Some(pin), "the same pin");
    assert!(own.laser.lasing() || own.laser.point.is_some());
    assert!(
        rig.host.world().combat.state.guided_flight(bomb).is_some(),
        "the bomb is still in the air at the rejoin"
    );
    assert!(rig.run_until(Duration::from_secs(60), |r| {
        watch(r, &mut modes, &mut gap);
        r.host.world().combat.state.guided_flight(bomb).is_none()
    }));
    assert!(
        modes.iter().all(|m| *m == GuidedMode::Guiding),
        "the bomb guided the whole way: {:?}",
        modes.iter().filter(|m| **m != GuidedMode::Guiding).count()
    );
    assert!(rig.host.world().combat.state.guided_impact(plane).is_some());
}

/// Slice ST-K (docs/spec/ai-strike.md, "Takeover and handback"): a laser
/// bomber (the synthetic F/A-18D's pod and two GBU-10s, a Cheat loading)
/// sets up its pass at the column from 5,000 ft: the A-G page and the bomb,
/// the nearest ground object designated (Ctrl+X, Easy targeting), which the
/// sight tracks, LVL (Ctrl+M) and the laser on by hand (Ctrl+L twice: the
/// automatic laser off by hand, then on by hand). Its game stalls for good
/// before the release. The host drops it and the AI flies the same pass:
/// the same target and bomb in LVL with the laser on by hand, the bomb
/// guiding on it to the end. The game joins again with its token and finds
/// its cockpit as it left it, one bomb fewer.
#[test]
fn a_laser_pass_dropped_mid_run_is_finished_by_the_ai_and_found_on_the_rejoin() {
    use crate::settings::{LoadoutRule, number};
    use tore_sim::combat::delivery::DeliveryMode;
    use tore_sim::combat::live::{Command as Combat, GuidedMode};
    use tore_sim::combat::sight::SightMode;
    let mut spec = crate::wire::readout_a2g_tests::a2g_spec();
    spec.loadout = None;
    spec.wings[0].count = 2;
    spec.start = tore_world::mission::Start::Airborne { altitude_ft: 5_000 };
    spec.cheats.easy_targeting = true;
    let mut import = tore_world::test_support::surface::routed_resources();
    tore_world::test_support::resources::add_a2g_stores(&mut import);
    let mut rig = Rig::with_import(spec, LinkConfig::one_way(10 * MS), 21, import, |config| {
        config
            .settings
            .push((number::LOADOUTS, LoadoutRule::Any.value()));
        // The mission waits out the AI's pass with nobody connected.
        config.empty_timeout = Duration::from_secs(300);
    });
    let script = |run: bool| -> super::tests::Script {
        let mut level = level_script();
        let mut since = None;
        let mut given = 0;
        Box::new(move |now, client, picture| {
            let mut controls = level(now, client, picture);
            if !run || client.prediction().is_none() {
                return controls;
            }
            let at = now.saturating_sub(*since.get_or_insert(now));
            let due = [0, 500, 1_000, 1_500, 2_000];
            if given < due.len() && at >= Duration::from_millis(due[given]) {
                controls.commands.extend(match given {
                    0 => vec![
                        SeatCommand::Combat(Combat::ToggleAirGround),
                        SeatCommand::CycleWeapon { forward: true },
                    ],
                    1 => vec![SeatCommand::Combat(Combat::NearestGround)],
                    2 => vec![SeatCommand::Combat(Combat::ToggleSeekerMode)],
                    _ => vec![SeatCommand::Combat(Combat::ToggleLaser)],
                });
                given += 1;
            }
            controls
        })
    };
    let viper = rig.join(
        |c| {
            c.callsign = "Viper".into();
            c.auto_ready = false;
        },
        script(true),
    );
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[viper].client.lobby().is_some()
    }));
    rig.players[viper].client.take_any_slot();
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[viper]
            .client
            .lobby()
            .and_then(|l| l.me())
            .is_some_and(|me| me.slot.is_some())
    }));
    rig.players[viper]
        .client
        .send_loadout(Some(tore_world::test_support::resources::a2g_loadout()));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.players[viper]
            .client
            .lobby()
            .and_then(|l| l.me())
            .is_some_and(|me| me.loadout)
    }));
    rig.players[viper].client.set_ready(true);
    assert!(rig.run_until(Duration::from_secs(5), |r| r.seated(viper)));
    let plane = rig.players[viper].client.seat().map(|(_, p)| p.0).unwrap();
    rig.run(Duration::from_secs(4));
    // What the player set up, as the host holds it.
    let left = {
        let own = rig.host.world().combat.state.ownship(plane).unwrap();
        let target = own.designated().expect("Ctrl+X designates");
        assert_eq!(own.delivery_mode, DeliveryMode::Level);
        assert_eq!(own.laser.manual, Some(true), "on by hand");
        // (It lases once the target is inside the pod's reach.)
        assert_eq!(own.sight.map(|s| s.mode), Some(SightMode::Tracked(target)));
        (
            target,
            own.selected,
            own.armed,
            own.launch_mode,
            own.sensors.air_ground(),
            own.rounds(own.selected),
        )
    };
    let (target, selected, armed, launch, air_ground, rounds) = left;
    assert_eq!(rounds, 2);
    assert!(
        rig.host
            .world()
            .combat
            .state
            .guided_flights()
            .next()
            .is_none(),
        "nothing released yet"
    );
    let token = rig.players[viper].client.token().unwrap().token;
    rig.players[viper].stalled = true;
    assert!(rig.run_until(Duration::from_secs(8), |r| r.host.peers_for_test() == 0));
    // The AI flies the pass with the human's cockpit: released on the same
    // target, the mode and the hand setting kept every step, the bomb
    // guiding to its end.
    let mut bomb = None;
    let mut modes = Vec::new();
    let mut kept_ok = true;
    assert!(
        rig.run_until(Duration::from_secs(150), |r| {
            let state = &r.host.world().combat.state;
            if let Some(kept) = state.kept(plane) {
                kept_ok &= kept.delivery_mode == DeliveryMode::Level
                    && kept.laser.manual == Some(true)
                    && kept.selected == selected;
            }
            if bomb.is_none() {
                bomb = state
                    .guided_flights()
                    .find(|(_, f)| f.owner == plane)
                    .map(|(id, _)| id);
            }
            match bomb.and_then(|id| state.guided_flight(id)) {
                Some(flight) => {
                    modes.push(flight.mode);
                    false
                }
                None => bomb.is_some(),
            }
        }),
        "the AI released and the bomb ended"
    );
    assert!(kept_ok, "the AI kept the human's mode and laser");
    let locked = modes
        .iter()
        .position(|m| *m == GuidedMode::Guiding)
        .expect("the bomb guided");
    assert!(modes[locked..].iter().all(|m| *m == GuidedMode::Guiding));
    assert!(rig.host.world().combat.state.guided_impact(plane).is_some());
    // The rejoin: the cockpit as left, one bomb fewer.
    rig.run(Duration::from_millis(200));
    let back = rig.join(
        move |c| {
            c.callsign = "Viper".into();
            c.token = Some(token);
        },
        script(false),
    );
    assert!(rig.run_until(Duration::from_secs(4), |r| r.seated(back)));
    assert_eq!(
        rig.players[back].client.seat().map(|(_, p)| p.0),
        Some(plane)
    );
    let own = rig.host.world().combat.state.ownship(plane).unwrap();
    assert_eq!(
        (
            own.selected,
            own.armed,
            own.delivery_mode,
            own.launch_mode,
            own.laser.manual,
            own.sensors.air_ground()
        ),
        (
            selected,
            armed,
            DeliveryMode::Level,
            launch,
            Some(true),
            air_ground
        )
    );
    assert_eq!(own.rounds(selected), rounds - 1);
    let standing = rig
        .host
        .world()
        .combat
        .state
        .targets
        .iter()
        .any(|t| t.id == target && t.hp > 0);
    if standing {
        assert_eq!(own.designated(), Some(target));
    }
}

#[test]
fn disconnect_forgets_only_this_session_and_ignores_delayed_grants() {
    let mut rig = rig();
    let shelf = Shelf::default();
    let viper = flier(&mut rig, "Viper", None);
    rig.players[viper]
        .client
        .set_token_store(Box::new(shelf.clone()));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let grant = rig.players[viper].client.token().unwrap().clone();
    let mut other = grant.clone();
    other.session_id ^= 1;
    shelf.clone().keep(&other);
    let now = rig.net.now();
    rig.players[viper].client.disconnect(now);
    rig.players[viper]
        .client
        .rejoin_token(crate::wire::migration::TokenGrant {
            token: grant.token,
            life_seconds: grant.life_seconds,
        });
    rig.players[viper].client.rejoin_closed();
    assert!(rig.players[viper].client.token().is_none());
    assert_eq!(*shelf.0.lock().unwrap(), vec![other]);
}

#[test]
fn ending_only_the_flight_keeps_rejoin_identity() {
    let mut rig = rig();
    let viper = flier(&mut rig, "Viper", None);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let token = rig.players[viper].client.token().unwrap().token;
    rig.players[viper].client.leave(rig.net.now());
    assert!(
        rig.run_until(Duration::from_secs(3), |r| r.players[viper].client.phase()
            == ClientPhase::Lobby)
    );
    assert_eq!(rig.players[viper].client.token().unwrap().token, token);
    assert_eq!(rig.host.token_of_for_test("Viper"), Some(token));
}

#[test]
fn cleanup_after_connection_loss_keeps_the_rejoin_token() {
    let mut rig = rig();
    let shelf = Shelf::default();
    let viper = flier(&mut rig, "Viper", None);
    rig.players[viper]
        .client
        .set_token_store(Box::new(shelf.clone()));
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let token = rig.players[viper].client.token().unwrap().token;
    rig.players[viper]
        .client
        .net
        .disconnect(tore_net::DisconnectReason::Timeout);
    assert!(rig.run_until(Duration::from_secs(1), |r| r.closed(viper)));
    rig.players[viper].client.disconnect(rig.net.now());
    assert_eq!(rig.players[viper].client.token().unwrap().token, token);
    assert_eq!(shelf.0.lock().unwrap().last().unwrap().token, token);
}
