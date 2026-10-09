//! Autobalance (slice A1 of the lobby pass; plan section 6.3, John's rules
//! of 2026-10-09): the host seats each player on the side with fewer
//! humans, refuses the players' own choice of side, re-deals when the King
//! turns it on, and re-deals nothing when a player leaves. On the network
//! simulator with full client sessions (the King's tests' rig).
//!
//! Mounted inside `king_tests.rs`, whose rig it drives.

use super::*;
use crate::host::balance::BOTH_FULL;
use crate::host::sides::SIDES_BALANCED;
use crate::wire::messages::{LobbyPhase, SlotRequest};
use tore_sim::ai::launch::Side;

/// A PvP game a player hosts (the first to join is the house and the
/// King), its sides as `sides` says: 0 free, 1 locked, 2 balanced.
fn pvp(friendly: usize, enemy: usize, sides: u32) -> Rig {
    pvp_of(spec(friendly, enemy), sides)
}

fn pvp_of(spec: MissionSpec, sides: u32) -> Rig {
    Rig::new(spec, |config| {
        config.house = Some(Rig::address(0));
        config.crown = CrownRule::FirstPlayer;
        config.start = StartMode::King;
        config.restart_delay = Duration::ZERO;
        config.empty_timeout = Duration::ZERO;
        config.settings = vec![
            (number::MODE, Mode::Pvp.value()),
            (number::LOCK_SIDES, sides),
        ];
    })
}

/// `callsign` joins, and the rig runs until its game is in the lobby and
/// sees itself with a slot (or a second more passes), so the players join
/// one after another in a known order.
fn arrive(rig: &mut Rig, callsign: &str) -> usize {
    let player = rig.join(callsign);
    assert!(
        rig.run_until(Duration::from_secs(3), |r| {
            r.players[player].client.phase() == ClientPhase::Lobby
                && r.lobby(player).is_some_and(|l| l.me().is_some())
        }),
        "{callsign} reaches the lobby"
    );
    rig.run_until(Duration::from_secs(1), |r| slot(r, player).is_some());
    player
}

fn slot(rig: &Rig, player: usize) -> Option<u32> {
    rig.lobby(player).unwrap().me().unwrap().slot
}

fn ready(rig: &Rig, player: usize) -> bool {
    rig.lobby(player).unwrap().me().unwrap().ready
}

/// The planes `players` hold, in that order.
fn slots(rig: &Rig, players: &[usize]) -> Vec<Option<u32>> {
    players.iter().map(|&p| slot(rig, p)).collect()
}

/// `player` makes `request`, and the rig runs until its slot changes or a
/// refusal arrives, or a second.
fn ask(rig: &mut Rig, player: usize, request: SlotRequest) {
    let (held, refusals) = (slot(rig, player), rig.refusals(player).len());
    match request {
        SlotRequest::Take(plane) => rig.client(player).take_slot(plane),
        SlotRequest::Side(side) => rig.client(player).take_side_slot(side),
        SlotRequest::Leave => rig.client(player).leave_slot(),
        SlotRequest::Any => rig.client(player).take_any_slot(),
    }
    rig.run_until(Duration::from_secs(1), |r| {
        slot(r, player) != held || r.refusals(player).len() > refusals
    });
}

#[test]
fn joiners_go_to_the_side_with_fewer_humans_bluefor_on_a_tie() {
    // Planes 0 to 2 are Bluefor, 3 to 5 Redfor.
    let mut rig = pvp(3, 3, 2);
    let viper = arrive(&mut rig, "Viper");
    let cobra = arrive(&mut rig, "Cobra");
    let hawk = arrive(&mut rig, "Hawk");
    let mako = arrive(&mut rig, "Mako");
    // Each takes its side's lowest free slot: Bluefor on the first tie,
    // Redfor next, Bluefor on the second tie.
    assert_eq!(
        slots(&rig, &[viper, cobra, hawk, mako]),
        vec![Some(0), Some(3), Some(1), Some(4)]
    );
    assert!(
        rig.notices(viper)
            .contains(&"Autobalance put you on Bluefor.".to_owned()),
        "{:?}",
        rig.notices(viper)
    );
    assert!(
        rig.notices(cobra)
            .contains(&"Autobalance put you on Redfor.".to_owned())
    );
    // Seated, not ready: each player arms and readies itself.
    assert!(![viper, cobra, hawk, mako].iter().any(|&p| ready(&rig, p)));
    assert!(rig.refusals(viper).is_empty(), "{:?}", rig.refusals(viper));
}

#[test]
fn a_tie_in_humans_goes_to_the_side_with_more_free_slots() {
    // Bluefor 2 slots, Redfor 4: the first player goes to Redfor.
    let mut rig = pvp(2, 4, 2);
    let viper = arrive(&mut rig, "Viper");
    assert_eq!(slot(&rig, viper), Some(2));
    let cobra = arrive(&mut rig, "Cobra");
    assert_eq!(slot(&rig, cobra), Some(0), "fewer humans first");
}

#[test]
fn uneven_slots_two_and_ten_seat_six_players_two_and_four() {
    // Bluefor's wing 1 of two; Redfor's wings 1 and 2 of five each.
    let mut uneven = spec(2, 5);
    uneven.wings[4].count = 5;
    uneven.wings[4].skill = Skill::Average;
    let mut rig = pvp_of(uneven, 2);
    let players: Vec<usize> = ["Viper", "Cobra", "Hawk", "Mako", "Jester", "Iceman"]
        .into_iter()
        .map(|name| arrive(&mut rig, name))
        .collect();
    let held = slots(&rig, &players);
    let blue = held.iter().filter(|s| s.is_some_and(|p| p < 2)).count();
    let red = held.iter().filter(|s| s.is_some_and(|p| p >= 2)).count();
    assert_eq!((blue, red), (2, 4), "{held:?}");
    // Redfor first (more free slots), then Bluefor, until Bluefor is full.
    assert_eq!(
        held,
        vec![Some(2), Some(0), Some(3), Some(1), Some(4), Some(5)]
    );
}

#[test]
fn with_both_sides_full_a_joiner_waits_and_is_seated_when_a_slot_frees() {
    // Planes 0 and 1 are Bluefor, 2 Redfor.
    let mut rig = pvp(2, 1, 2);
    let king = arrive(&mut rig, "Viper");
    let cobra = arrive(&mut rig, "Cobra");
    assert_eq!(slots(&rig, &[king, cobra]), vec![Some(0), Some(2)]);
    // Bluefor's other slot is kept for a player not here yet.
    rig.client(king).lock_slot(1, Lock::Reserved("Mako".into()));
    rig.run(Duration::from_millis(200));
    let hawk = arrive(&mut rig, "Hawk");
    assert_eq!(slot(&rig, hawk), None);
    assert!(
        rig.notices(hawk).contains(&BOTH_FULL.to_owned()),
        "{:?}",
        rig.notices(hawk)
    );
    // Asking for any slot says the same.
    ask(&mut rig, hawk, SlotRequest::Any);
    assert!(
        rig.refused(hawk, kind::SLOT, BOTH_FULL),
        "{:?}",
        rig.refusals(hawk)
    );
    // Said once, not at every update.
    let told = rig.notices(hawk).iter().filter(|n| *n == BOTH_FULL).count();
    assert_eq!(told, 1);
    // Cobra leaves the game: Hawk takes its Redfor slot.
    let now = rig.net.now();
    rig.client(cobra).leave_game(now);
    assert!(rig.run_until(Duration::from_secs(3), |r| slot(r, hawk) == Some(2)));
    assert!(
        rig.notices(hawk)
            .contains(&"Autobalance put you on Redfor.".to_owned())
    );
    // Mako, whom the slot is kept for, gets it.
    let mako = arrive(&mut rig, "Mako");
    assert_eq!(slot(&rig, mako), Some(1));
}

#[test]
fn players_cannot_change_side_but_may_move_within_their_own() {
    let mut rig = pvp(3, 3, 2);
    let viper = arrive(&mut rig, "Viper");
    let cobra = arrive(&mut rig, "Cobra");
    assert_eq!(slots(&rig, &[viper, cobra]), vec![Some(0), Some(3)]);
    // The other side: a side request and a slot are refused.
    ask(&mut rig, viper, SlotRequest::Side(Side::Enemy));
    ask(&mut rig, viper, SlotRequest::Take(4));
    let refused = rig
        .refusals(viper)
        .iter()
        .filter(|(k, r)| *k == kind::SLOT && r == SIDES_BALANCED)
        .count();
    assert_eq!(refused, 2, "{:?}", rig.refusals(viper));
    assert_eq!(slot(&rig, viper), Some(0));
    // Leaving the slot (and with it the side) is refused too.
    ask(&mut rig, viper, SlotRequest::Leave);
    assert_eq!(slot(&rig, viper), Some(0));
    assert_eq!(rig.refusals(viper).len(), 3);
    // Its own side: another free slot, and the side request keeps it.
    ask(&mut rig, viper, SlotRequest::Take(2));
    assert_eq!(slot(&rig, viper), Some(2));
    ask(&mut rig, viper, SlotRequest::Side(Side::Friendly));
    assert_eq!(slot(&rig, viper), Some(2));
    assert_eq!(rig.refusals(viper).len(), 3, "{:?}", rig.refusals(viper));
    // Cobra readies in the slot the host gave it.
    rig.client(cobra).set_ready(true);
    rig.run(Duration::from_millis(200));
    assert!(ready(&rig, cobra));
}

#[test]
fn a_player_leaving_re_deals_nothing_and_the_next_joiner_fills_the_gap() {
    let mut rig = pvp(3, 3, 2);
    let viper = arrive(&mut rig, "Viper");
    let cobra = arrive(&mut rig, "Cobra");
    let hawk = arrive(&mut rig, "Hawk");
    let mako = arrive(&mut rig, "Mako");
    assert_eq!(
        slots(&rig, &[viper, cobra, hawk, mako]),
        vec![Some(0), Some(3), Some(1), Some(4)]
    );
    rig.client(cobra).set_ready(true);
    rig.run(Duration::from_millis(200));
    // Hawk (Bluefor) leaves: Bluefor 1, Redfor 2, and nobody moves.
    let now = rig.net.now();
    rig.client(hawk).leave_game(now);
    rig.run(Duration::from_secs(2));
    assert_eq!(
        slots(&rig, &[viper, cobra, mako]),
        vec![Some(0), Some(3), Some(4)]
    );
    assert!(ready(&rig, cobra), "a ready mark stands");
    // The next joiner goes to the smaller side, in its lowest free slot.
    let jester = arrive(&mut rig, "Jester");
    assert_eq!(slot(&rig, jester), Some(1));
}

#[test]
fn turning_balance_on_re_deals_with_the_fewest_moves_the_newest_first() {
    // Sides free: everyone picks Bluefor.
    let mut rig = pvp(4, 4, 0);
    let king = arrive(&mut rig, "Viper");
    let cobra = arrive(&mut rig, "Cobra");
    let hawk = arrive(&mut rig, "Hawk");
    let mako = arrive(&mut rig, "Mako");
    let jester = arrive(&mut rig, "Jester");
    for (player, plane) in [(king, 0), (cobra, 1), (hawk, 2), (mako, 3)] {
        ask(&mut rig, player, SlotRequest::Take(plane));
        rig.client(player).set_ready(true);
    }
    rig.client(cobra).send_loadout(Some(standard()));
    rig.run(Duration::from_millis(300));
    assert_eq!(slot(&rig, jester), None, "sides free: no seat given");
    assert!([king, cobra, hawk, mako].iter().all(|&p| ready(&rig, p)));
    rig.change(king, &[(number::LOCK_SIDES, 2)]);
    rig.run(Duration::from_millis(300));
    // Jester, without a slot, goes to Redfor first (4 to 1); then the newest
    // of Bluefor moves until the sides are within one: Mako (3 to 2).
    assert_eq!(
        slots(&rig, &[king, cobra, hawk, mako, jester]),
        vec![Some(0), Some(1), Some(2), Some(5), Some(4)]
    );
    assert!(
        rig.notices(mako)
            .contains(&"Autobalance moved you to Redfor.".to_owned()),
        "{:?}",
        rig.notices(mako)
    );
    assert!(!ready(&rig, mako), "a mover's ready mark goes");
    assert!(
        [king, cobra, hawk].iter().all(|&p| ready(&rig, p)),
        "the others' stand"
    );
    // Cobra did not move, so its loadout stands.
    assert!(rig.lobby(cobra).unwrap().me().unwrap().loadout);
}

#[test]
fn the_king_moves_last_only_when_nobody_else_can() {
    let mut rig = pvp(3, 1, 0);
    let king = arrive(&mut rig, "Viper");
    let cobra = arrive(&mut rig, "Cobra");
    let hawk = arrive(&mut rig, "Hawk");
    for (player, plane) in [(king, 0), (cobra, 1), (hawk, 2)] {
        ask(&mut rig, player, SlotRequest::Take(plane));
    }
    // Redfor's only slot is kept for the King: free to the King alone.
    rig.client(king)
        .lock_slot(3, Lock::Reserved("Viper".into()));
    rig.run(Duration::from_millis(200));
    rig.change(king, &[(number::LOCK_SIDES, 2)]);
    rig.run(Duration::from_millis(300));
    assert_eq!(
        slots(&rig, &[king, cobra, hawk]),
        vec![Some(3), Some(1), Some(2)]
    );
    assert!(
        rig.notices(king)
            .contains(&"Autobalance moved you to Redfor.".to_owned())
    );
}

#[test]
fn turning_balance_on_with_sides_within_one_moves_nobody() {
    let mut rig = pvp(3, 3, 1);
    let king = arrive(&mut rig, "Viper");
    let cobra = arrive(&mut rig, "Cobra");
    let hawk = arrive(&mut rig, "Hawk");
    for (player, plane) in [(king, 0), (cobra, 1), (hawk, 3)] {
        ask(&mut rig, player, SlotRequest::Take(plane));
        rig.client(player).set_ready(true);
    }
    rig.run(Duration::from_millis(200));
    rig.change(king, &[(number::LOCK_SIDES, 2)]);
    rig.run(Duration::from_millis(300));
    assert_eq!(
        slots(&rig, &[king, cobra, hawk]),
        vec![Some(0), Some(1), Some(3)]
    );
    assert!([king, cobra, hawk].iter().all(|&p| ready(&rig, p)));
    // Turning it off clears the ready marks, as a change of sides does.
    rig.change(king, &[(number::LOCK_SIDES, 1)]);
    rig.run(Duration::from_millis(200));
    assert!(![king, cobra, hawk].iter().any(|&p| ready(&rig, p)));
}

#[test]
fn a_mission_change_keeps_sides_and_re_seats_the_players_it_freed() {
    let mut rig = pvp(3, 3, 2);
    let king = arrive(&mut rig, "Viper");
    let cobra = arrive(&mut rig, "Cobra");
    let hawk = arrive(&mut rig, "Hawk");
    // Hawk moves to Bluefor's last slot, which the new mission drops.
    ask(&mut rig, hawk, SlotRequest::Take(2));
    assert_eq!(
        slots(&rig, &[king, cobra, hawk]),
        vec![Some(0), Some(3), Some(2)]
    );
    // Two and two: planes 0 and 1 Bluefor, 2 and 3 Redfor. Viper keeps
    // plane 0; Cobra keeps plane 3 (now Redfor's second); Hawk's plane 2 is
    // Redfor's now, a slot it still holds.
    rig.client(king).change_mission(&spec(2, 2));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.lobby(king).is_some_and(|l| l.slots.len() == 4)
    }));
    rig.run(Duration::from_millis(300));
    assert_eq!(
        slots(&rig, &[king, cobra, hawk]),
        vec![Some(0), Some(3), Some(2)]
    );
    // One Bluefor slot: Hawk's plane 2 goes; Hawk is seated again by the
    // rule (Bluefor 1, Redfor 1: the tie goes to more free slots).
    rig.client(king).change_mission(&spec(1, 3));
    rig.run(Duration::from_secs(1));
    let held = slots(&rig, &[king, cobra, hawk]);
    assert_eq!(held[0], Some(0));
    assert!(held[2].is_some(), "Hawk is seated again: {held:?}");
    assert_ne!(held[2], held[1]);
}

#[test]
fn a_late_joiner_in_flight_goes_to_the_smaller_side_and_joins_there() {
    // A server's start rule: the first ready flies the mission.
    let mut rig = Rig::new(spec(2, 2), |config| {
        config.settings = vec![(number::MODE, Mode::Pvp.value()), (number::LOCK_SIDES, 2)];
    });
    let viper = arrive(&mut rig, "Viper");
    assert_eq!(slot(&rig, viper), Some(0));
    rig.client(viper).set_ready(true);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    let cobra = arrive(&mut rig, "Cobra");
    assert_eq!(rig.lobby(cobra).unwrap().phase, LobbyPhase::Flying);
    assert_eq!(slot(&rig, cobra), Some(2), "Redfor's free AI aircraft");
    // A Bluefor plane is refused; Join flies the slot given.
    ask(&mut rig, cobra, SlotRequest::Take(1));
    assert!(rig.refused(cobra, kind::SLOT, SIDES_BALANCED));
    rig.client(cobra).set_ready(true);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(cobra)));
    let flying = rig
        .lobby(cobra)
        .unwrap()
        .players
        .iter()
        .find(|p| p.callsign == "Cobra")
        .and_then(|p| p.slot);
    assert_eq!(flying, Some(2));
}

#[test]
fn co_op_has_no_autobalance() {
    let mut rig = Rig::hosted(spec(2, 2));
    let king = arrive(&mut rig, "Viper");
    rig.change(king, &[(number::LOCK_SIDES, 2)]);
    assert!(
        rig.refusals(king)
            .iter()
            .any(|(k, r)| *k == kind::SETTINGS && r.contains("only in PvP")),
        "{:?}",
        rig.refusals(king)
    );
    let cobra = arrive(&mut rig, "Cobra");
    assert_eq!(slot(&rig, cobra), None, "nobody seats a co-op player");
}
