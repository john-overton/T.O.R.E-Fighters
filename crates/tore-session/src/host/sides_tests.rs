//! The lobby pass's side requests (slice W0; plan section 5.4): a player
//! asks for the first free slot of Bluefor or Redfor, on the network
//! simulator with full client sessions (the King's tests' rig).
//!
//! Mounted inside `king_tests.rs`, whose rig it drives.

use super::*;
use crate::host::sides::{SIDES_BALANCED, SIDES_PVP_ONLY};
use crate::wire::messages::{LobbyPhase, Message, Slot, SlotRequest};
use tore_sim::ai::launch::Side;

/// A PvP game a player hosts, sides free unless `settings` say otherwise.
fn pvp(friendly: usize, enemy: usize, settings: &[(u8, u32)]) -> Rig {
    let mut values = vec![(number::MODE, Mode::Pvp.value())];
    values.extend_from_slice(settings);
    if !values.iter().any(|(n, _)| *n == number::LOCK_SIDES) {
        values.push((number::LOCK_SIDES, 0));
    }
    Rig::new(spec(friendly, enemy), |config| {
        config.house = Some(Rig::address(0));
        config.crown = CrownRule::FirstPlayer;
        config.start = StartMode::King;
        config.restart_delay = Duration::ZERO;
        config.empty_timeout = Duration::ZERO;
        config.settings = values;
    })
}

/// `player` asks for `side`, and the rig runs until its slot changes or a
/// refusal arrives (the lobby state goes out at its own pace), or a second.
fn side(rig: &mut Rig, player: usize, side: Side) {
    let (held, refusals) = (slot(rig, player), rig.refusals(player).len());
    rig.client(player).take_side_slot(side);
    rig.run_until(Duration::from_secs(1), |r| {
        slot(r, player) != held || r.refusals(player).len() > refusals
    });
}

fn slot(rig: &Rig, player: usize) -> Option<u32> {
    rig.lobby(player).unwrap().me().unwrap().slot
}

#[test]
fn the_side_request_is_the_slot_requests_value_3_then_the_side_bit() {
    let bytes = |request| {
        Message::Slot(Slot {
            mission: 7,
            request,
        })
        .encode()
        .unwrap()
    };
    // The mission number's varint (7), then the request's two bits, then
    // the side: LSB first, so 7, then 3, then 0 or 1 in bit 2.
    assert_eq!(bytes(SlotRequest::Side(Side::Friendly)), vec![7, 0b011]);
    assert_eq!(bytes(SlotRequest::Side(Side::Enemy)), vec![7, 0b111]);
    for side in [Side::Friendly, Side::Enemy] {
        let message = Message::Slot(Slot {
            mission: 7,
            request: SlotRequest::Side(side),
        });
        let decoded = Message::decode(message.kind(), &message.encode().unwrap()).unwrap();
        assert_eq!(decoded, message);
    }
}

#[test]
fn a_side_request_takes_that_sides_first_free_slot() {
    // Planes 0 and 1 are Bluefor, 2 to 4 Redfor.
    let mut rig = pvp(2, 3, &[]);
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    let hawk = rig.join("Hawk");
    rig.gather(&[viper, cobra, hawk]);
    side(&mut rig, viper, Side::Enemy);
    assert_eq!(slot(&rig, viper), Some(2), "Redfor's lead first");
    side(&mut rig, cobra, Side::Enemy);
    assert_eq!(slot(&rig, cobra), Some(3), "the next free one");
    side(&mut rig, hawk, Side::Friendly);
    assert_eq!(slot(&rig, hawk), Some(0), "{:?}", rig.refusals(hawk));
    // Asking again for one's own side keeps the slot held.
    rig.client(cobra).take_slot(4);
    rig.run_until(Duration::from_secs(1), |r| slot(r, cobra) == Some(4));
    side(&mut rig, cobra, Side::Enemy);
    assert_eq!(slot(&rig, cobra), Some(4));
    assert!(rig.refusals(cobra).is_empty(), "{:?}", rig.refusals(cobra));
    // The holders show in every player's lobby state.
    let lobby = rig.lobby(hawk).unwrap();
    let held: Vec<(u32, bool)> = lobby
        .slots
        .iter()
        .map(|s| (s.plane, s.holder.is_some()))
        .collect();
    assert_eq!(
        held,
        vec![(0, true), (1, false), (2, true), (3, false), (4, true)]
    );
}

#[test]
fn a_full_side_is_refused_and_a_kept_slot_is_free_to_its_player_only() {
    // Bluefor has planes 0 and 1; Redfor planes 2 and 3.
    let mut rig = pvp(2, 2, &[]);
    let king = rig.join("Viper");
    let cobra = rig.join("Cobra");
    let hawk = rig.join("Hawk");
    rig.gather(&[king, cobra, hawk]);
    side(&mut rig, king, Side::Enemy);
    assert_eq!(slot(&rig, king), Some(2));
    rig.client(king).lock_slot(3, Lock::Reserved("Hawk".into()));
    rig.client(king).lock_slot(1, Lock::Closed);
    rig.run(Duration::from_millis(200));
    // Redfor's last slot is kept for Hawk: full to Cobra.
    side(&mut rig, cobra, Side::Enemy);
    assert!(
        rig.refused(cobra, kind::SLOT, "Redfor is full."),
        "{:?}",
        rig.refusals(cobra)
    );
    assert_eq!(slot(&rig, cobra), None);
    side(&mut rig, cobra, Side::Friendly);
    assert_eq!(slot(&rig, cobra), Some(0));
    // Bluefor's other slot is closed: the side is full.
    side(&mut rig, hawk, Side::Friendly);
    assert!(rig.refused(hawk, kind::SLOT, "Bluefor is full."));
    assert_eq!(slot(&rig, hawk), None);
    // The slot kept for Hawk is free to Hawk.
    side(&mut rig, hawk, Side::Enemy);
    assert_eq!(slot(&rig, hawk), Some(3));
}

#[test]
fn a_player_leaves_its_side_before_it_chooses_the_other() {
    let mut rig = pvp(2, 2, &[]);
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    side(&mut rig, viper, Side::Friendly);
    assert_eq!(slot(&rig, viper), Some(0));
    side(&mut rig, viper, Side::Enemy);
    assert!(rig.refused(viper, kind::SLOT, "Leave Bluefor first."));
    assert_eq!(slot(&rig, viper), Some(0));
    // Leave frees the slot and with it the side.
    rig.client(viper).leave_slot();
    rig.run_until(Duration::from_secs(1), |r| slot(r, viper).is_none());
    assert_eq!(slot(&rig, viper), None);
    side(&mut rig, viper, Side::Enemy);
    assert_eq!(slot(&rig, viper), Some(2));
}

#[test]
fn side_requests_are_refused_in_co_op_and_while_the_host_balances_the_sides() {
    let mut rig = Rig::hosted(spec(2, 2));
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    side(&mut rig, viper, Side::Friendly);
    assert!(rig.refused(viper, kind::SLOT, SIDES_PVP_ONLY));
    assert_eq!(slot(&rig, viper), None);

    // Autobalance (slice A1) seats Viper on Bluefor as it joins: its own
    // side is no change, the other is refused.
    let mut rig = pvp(2, 2, &[(number::LOCK_SIDES, 2)]);
    let viper = rig.join("Viper");
    rig.gather(&[viper]);
    assert!(rig.host.settings.balanced());
    for wanted in [Side::Friendly, Side::Enemy] {
        side(&mut rig, viper, wanted);
    }
    let balanced = rig
        .refusals(viper)
        .iter()
        .filter(|(k, r)| *k == kind::SLOT && r == SIDES_BALANCED)
        .count();
    assert_eq!(balanced, 1, "{:?}", rig.refusals(viper));
    assert_eq!(slot(&rig, viper), Some(0));
}

#[test]
fn lock_sides_refuses_the_other_side_in_flight() {
    for (lock, enemy_allowed) in [(1, false), (0, true)] {
        // A server's start rule: the first ready flies the mission, which
        // keeps flying while its player is back in the lobby.
        let mut rig = Rig::new(spec(2, 2), |config| {
            config.settings = vec![
                (number::MODE, Mode::Pvp.value()),
                (number::LOCK_SIDES, lock),
            ];
        });
        let viper = rig.join("Viper");
        rig.gather(&[viper]);
        side(&mut rig, viper, Side::Friendly);
        assert_eq!(slot(&rig, viper), Some(0));
        rig.client(viper).set_ready(true);
        assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
        let now = rig.net.now();
        rig.client(viper).leave(now);
        rig.run(Duration::from_secs(1));
        rig.client(viper).leave_slot();
        rig.run_until(Duration::from_secs(1), |r| slot(r, viper).is_none());
        assert_eq!(slot(&rig, viper), None);
        assert_eq!(rig.lobby(viper).unwrap().phase, LobbyPhase::Flying);
        side(&mut rig, viper, Side::Enemy);
        if enemy_allowed {
            assert_eq!(slot(&rig, viper), Some(2), "lock sides off");
        } else {
            assert!(
                rig.refused(viper, kind::SLOT, super::king::SIDES_LOCKED),
                "{:?}",
                rig.refusals(viper)
            );
            assert_eq!(slot(&rig, viper), None);
            // Its own side is open still: plane 0, which the AI flies now.
            side(&mut rig, viper, Side::Friendly);
            assert_eq!(slot(&rig, viper), Some(0));
        }
    }
}

#[test]
fn in_flight_a_side_request_skips_a_plane_another_player_flies() {
    let mut rig = Rig::new(spec(1, 2), |config| {
        config.settings = vec![(number::MODE, Mode::Pvp.value()), (number::LOCK_SIDES, 0)];
    });
    let viper = rig.join("Viper");
    let cobra = rig.join("Cobra");
    rig.gather(&[viper, cobra]);
    side(&mut rig, viper, Side::Enemy);
    rig.client(viper).set_ready(true);
    assert!(rig.run_until(Duration::from_secs(3), |r| r.seated(viper)));
    // Cobra, in the lobby while Viper flies Redfor's lead, gets plane 2.
    side(&mut rig, cobra, Side::Enemy);
    assert_eq!(slot(&rig, cobra), Some(2), "{:?}", rig.refusals(cobra));
    // Viper leaves its aircraft and its slot; the AI flies plane 1 again,
    // so it is free once more.
    let now = rig.net.now();
    rig.client(viper).leave(now);
    rig.run(Duration::from_secs(1));
    rig.client(viper).leave_slot();
    rig.run_until(Duration::from_secs(1), |r| slot(r, viper).is_none());
    side(&mut rig, viper, Side::Enemy);
    assert_eq!(slot(&rig, viper), Some(1), "{:?}", rig.refusals(viper));
}
