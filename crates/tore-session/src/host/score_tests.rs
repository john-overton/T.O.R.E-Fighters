//! Scoring on the host (slice F2-S; docs/ARCHITECTURE.md, "Scoring"): the
//! tallies for each tally, fight and kill owner, the kill limit's end with
//! its winner or a draw, the time limit's winner, co-op's rules, and the pace
//! of the Scores message, on the network simulator with two players, one on
//! each side.
//!
//! Kills and losses reach the host the way they do in a mission: the test
//! sets the world as combat leaves it (a ledger kill and no hit points left)
//! and the world's own facts follow. Damage is fed to the tallies as facts,
//! since only a hit makes one.

use super::*;
use crate::settings::{Fight, KillOwner, Mode, ScoreTally, number};
use crate::wire::messages::{Scores, Winner};
use tore_sim::combat::ledger::Kill;
use tore_world::score::{Fact, Flown, Victim};

/// Friendly planes 0 and 1, enemy planes 2 and 3.
const FRIENDLY_HUMAN: u32 = 0;
const FRIENDLY_AI: u32 = 1;
const ENEMY_HUMAN: u32 = 2;
const ENEMY_AI: u32 = 3;

/// A host with every plane open, its settings as `settings` (number and
/// value, applied after the mode), and two players flying: Viper in plane 0
/// and Cobra in plane 2. Returns the rig and the two clients.
fn two_sides(mode: Mode, settings: &[(u8, u32)]) -> (Rig, usize, usize) {
    let mut rig = Rig::new(
        spec(2, 2, 50),
        HostConfig {
            open_planes: OpenPlanes::All,
            ..config()
        },
        LinkConfig::one_way(5 * MS),
    );
    rig.host
        .settings
        .apply(&[(number::MODE, mode.value())])
        .unwrap();
    rig.host.settings.apply(settings).unwrap();
    let viper = rig.join(|c| c.callsign = "Viper".into());
    rig.clients[viper].ready = Some(Some(FRIENDLY_HUMAN));
    let cobra = rig.join(|c| c.callsign = "Cobra".into());
    rig.clients[cobra].ready = Some(Some(ENEMY_HUMAN));
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.seated(viper) && r.seated(cobra)
    }));
    (rig, viper, cobra)
}

/// The world as combat leaves it when `owner` shoots `victim` down: the
/// ledger's kill and no hit points left.
fn shoot_down(host: &mut Host, owner: u32, victim: u32) {
    let state = &mut host.world.combat.state;
    state.ledger.kill(Kill {
        owner,
        victim,
        category: 0x8000,
        aircraft: true,
    });
    match state.ownship_mut(victim) {
        Some(own) => own.hp = 0,
        None => {
            state
                .targets
                .iter_mut()
                .find(|t| t.id == victim)
                .unwrap()
                .hp = 0;
        }
    }
}

/// The connection order of a test client's player.
fn order(rig: &Rig, callsign: &str) -> u64 {
    rig.host
        .peers
        .values()
        .find(|p| p.callsign == callsign)
        .unwrap()
        .lobby
        .order
}

fn tally(rig: &Rig, callsign: &str) -> score::Tally {
    rig.host
        .score
        .player(order(rig, callsign))
        .map(|p| p.tally)
        .unwrap_or_default()
}

/// Who flies `plane` now, as a fact names it.
fn flown(rig: &Rig, plane: u32) -> Flown {
    Flown {
        plane: PlaneId(plane),
        pilot: rig.host.world().roster.plane(PlaneId(plane)).unwrap().pilot,
    }
}

/// A hit by `shooter` on `victim` for `fraction` of its hit points.
fn damage(rig: &Rig, shooter: u32, victim: u32, fraction: f64) -> Fact {
    Fact::Damage {
        shooter: Some(flown(rig, shooter)),
        victim: Victim {
            target: victim,
            flown: Some(flown(rig, victim)),
            aircraft: true,
        },
        fraction,
    }
}

/// The seats the host's players fly, as the tick's tally reads them.
fn seats_of(rig: &Rig) -> BTreeMap<SeatId, u64> {
    rig.host
        .peers
        .values()
        .filter_map(|p| Some((p.seat?, p.lobby.order)))
        .collect()
}

fn last_scores(rig: &Rig, client: usize) -> &Scores {
    rig.clients[client].scores.last().expect("a Scores message")
}

#[test]
fn a_human_shot_down_with_the_pilot_aboard_counts_two_and_an_ai_aircraft_one() {
    let (mut rig, viper, cobra) = two_sides(Mode::Pvp, &[(number::KILL_LIMIT, 0)]);
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_HUMAN);
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_AI);
    // Its own side's AI aircraft and an AI shooter's kill count nothing.
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, FRIENDLY_AI);
    rig.run(Duration::from_millis(50));
    assert_eq!(
        tally(&rig, "Viper"),
        score::Tally {
            kills: 3,
            losses: 0,
            damage: 0.,
        }
    );
    assert_eq!(
        tally(&rig, "Cobra"),
        score::Tally {
            kills: 0,
            losses: 1,
            damage: 0.,
        }
    );
    assert_eq!(rig.host.score.side(Side::Friendly).kills, 3);
    assert_eq!(rig.host.score.side(Side::Enemy).losses, 1);
    // Everyone flying hears within a second, ranked by kills.
    rig.run(Duration::from_millis(1100));
    for client in [viper, cobra] {
        let scores = last_scores(&rig, client);
        assert_eq!(scores.tally, ScoreTally::Kills);
        assert_eq!(scores.fight, Fight::Sides);
        assert_eq!(scores.kill_limit, 0);
        assert_eq!(
            scores
                .players
                .iter()
                .map(|p| (p.callsign.as_str(), p.side, p.kills, p.losses))
                .collect::<Vec<_>>(),
            [
                ("Viper", Some(Side::Friendly), 3, 0),
                ("Cobra", Some(Side::Enemy), 0, 1)
            ]
        );
        assert_eq!(scores.sides[0].kills, 3);
        assert_eq!(scores.sides[1].losses, 1);
        assert_eq!(scores.winner, Winner::NoneYet);
        // The PvP default time limit, ten minutes, counts down.
        assert!(scores.seconds_left.is_some_and(|s| s <= 600 && s > 590));
    }
    assert!(rig.clients.iter().all(|c| c.errors.is_empty()));
    assert!(rig.faults().is_empty());
}

#[test]
fn a_human_who_ejected_counts_one_and_an_ai_shooter_scores_nothing() {
    let (mut rig, _, _) = two_sides(Mode::Pvp, &[(number::KILL_LIMIT, 0)]);
    // Cobra is hit by Viper, then ejects: one kill, the pilot not aboard.
    rig.host.world.combat.state.ledger.damaged(Kill {
        owner: FRIENDLY_HUMAN,
        victim: ENEMY_HUMAN,
        category: 0x8000,
        aircraft: true,
    });
    let cockpit = rig
        .host
        .world
        .cockpits
        .iter_mut()
        .find(|c| c.plane.0 == ENEMY_HUMAN)
        .unwrap();
    cockpit.flight.systems.pilot.ejected = true;
    // The enemy AI shoots Viper down: a loss, and nobody's kill.
    shoot_down(&mut rig.host, ENEMY_AI, FRIENDLY_HUMAN);
    rig.run(Duration::from_millis(50));
    assert_eq!(
        (tally(&rig, "Viper").kills, tally(&rig, "Viper").losses),
        (1, 1)
    );
    assert_eq!(
        (tally(&rig, "Cobra").kills, tally(&rig, "Cobra").losses),
        (0, 1)
    );
    assert_eq!(rig.host.score.side(Side::Enemy).kills, 0);
    assert_eq!(tally(&rig, "Viper").ratio(), 1.);
    assert_eq!(tally(&rig, "Cobra").ratio(), 0.);
}

#[test]
fn damage_counts_against_opponents_only_and_a_free_for_all_makes_one_side_opponents() {
    let (mut rig, _, _) = two_sides(Mode::Pvp, &[(number::KILL_LIMIT, 0)]);
    let seats = seats_of(&rig);
    for fact in [
        damage(&rig, FRIENDLY_HUMAN, ENEMY_HUMAN, 0.25),
        damage(&rig, FRIENDLY_HUMAN, ENEMY_AI, 0.5),
        // Its own side's AI aircraft: no damage to an opponent.
        damage(&rig, FRIENDLY_HUMAN, FRIENDLY_AI, 0.4),
        // The AI's hits count for nobody.
        damage(&rig, ENEMY_AI, FRIENDLY_HUMAN, 0.3),
    ] {
        rig.host.tally(fact, &seats);
    }
    assert_eq!(tally(&rig, "Viper").damage, 0.75);
    assert_eq!(rig.host.score.side(Side::Friendly).damage, 0.75);
    assert_eq!(rig.host.score.side(Side::Enemy).damage, 0.);
    assert_eq!(super::score::Tally::default().ratio(), 0.);

    // Free for all: another human of one's own side is an opponent, its AI
    // still is not. Plane 1 flown by a human is the case; here the host
    // judges Viper's hit on Cobra as if Cobra flew for Viper's side.
    let (mut rig, _, _) = two_sides(
        Mode::Pvp,
        &[
            (number::KILL_LIMIT, 0),
            (number::FIGHT, Fight::FreeForAll.value()),
        ],
    );
    let viper = flown(&rig, FRIENDLY_HUMAN);
    assert!(rig.host.opponents(
        PlaneId(FRIENDLY_HUMAN),
        Flown {
            plane: PlaneId(FRIENDLY_AI),
            pilot: Pilot::Human(SeatId(7)),
        }
    ));
    assert!(
        !rig.host
            .opponents(PlaneId(FRIENDLY_HUMAN), flown(&rig, FRIENDLY_AI))
    );
    assert!(!rig.host.opponents(PlaneId(FRIENDLY_HUMAN), viper));
    assert!(
        rig.host
            .opponents(PlaneId(FRIENDLY_HUMAN), flown(&rig, ENEMY_AI))
    );
    // By sides, the same human on one's own side is not.
    rig.host
        .settings
        .apply(&[(number::FIGHT, Fight::Sides.value())])
        .unwrap();
    assert!(!rig.host.opponents(
        PlaneId(FRIENDLY_HUMAN),
        Flown {
            plane: PlaneId(FRIENDLY_AI),
            pilot: Pilot::Human(SeatId(7)),
        }
    ));
    let seats = seats_of(&rig);
    rig.host
        .tally(damage(&rig, ENEMY_HUMAN, FRIENDLY_HUMAN, 0.125), &seats);
    assert_eq!(tally(&rig, "Cobra").damage, 0.125);
}

#[test]
fn the_kill_limit_by_side_ends_the_mission_with_the_winning_side() {
    let (mut rig, viper, cobra) = two_sides(
        Mode::Pvp,
        &[
            (number::KILL_LIMIT, 2),
            (number::KILL_OWNER, KillOwner::Side.value()),
        ],
    );
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_AI);
    rig.run(Duration::from_millis(100));
    assert_eq!(rig.host.phase(), Phase::Flying, "one kill of two");
    let tick = rig.host.world().tick();
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_HUMAN);
    assert!(rig.run_until(Duration::from_millis(200), |r| {
        r.clients[viper].ended.is_some() && r.clients[cobra].ended.is_some()
    }));
    let ended = rig
        .logs
        .iter()
        .find_map(|l| match l {
            HostLog::MissionEnded { tick, reason } => Some((*tick, *reason)),
            _ => None,
        })
        .unwrap();
    assert_eq!(ended.1, EndReason::KillLimit);
    assert!(ended.0 <= tick + 2, "ended at the tick of the kill");
    for client in [viper, cobra] {
        assert_eq!(
            rig.clients[client].ended.as_ref().unwrap().reason,
            EndReason::KillLimit
        );
        let last = last_scores(&rig, client);
        assert_eq!(last.winner, Winner::Side(Side::Friendly));
        assert_eq!(last.kill_limit, 2);
        assert_eq!(last.kill_owner, KillOwner::Side);
        assert_eq!(last.sides[0].kills, 3);
        // Each player's debrief follows.
        assert!(rig.clients[client].debrief.is_some());
    }
    assert!(rig.clients.iter().all(|c| c.errors.is_empty()));
}

#[test]
fn level_sides_at_the_kill_limit_are_a_draw() {
    let (mut rig, viper, _) = two_sides(
        Mode::Pvp,
        &[
            (number::KILL_LIMIT, 2),
            (number::KILL_OWNER, KillOwner::Total.value()),
        ],
    );
    // One AI kill each, in the same tick: total 2.
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_AI);
    shoot_down(&mut rig.host, ENEMY_HUMAN, FRIENDLY_AI);
    assert!(rig.run_until(Duration::from_millis(200), |r| {
        r.clients[viper].ended.is_some()
    }));
    assert_eq!(
        rig.clients[viper].ended.as_ref().unwrap().reason,
        EndReason::KillLimit
    );
    assert_eq!(last_scores(&rig, viper).winner, Winner::Draw);
}

#[test]
fn the_player_owner_and_a_free_for_all_name_the_best_player() {
    let (mut rig, viper, _) = two_sides(
        Mode::Pvp,
        &[
            (number::KILL_LIMIT, 1),
            (number::KILL_OWNER, KillOwner::Player.value()),
            (number::FIGHT, Fight::FreeForAll.value()),
        ],
    );
    let cobra_id = rig
        .host
        .peers
        .values()
        .find(|p| p.callsign == "Cobra")
        .unwrap()
        .lobby
        .id;
    shoot_down(&mut rig.host, ENEMY_HUMAN, FRIENDLY_AI);
    assert!(rig.run_until(Duration::from_millis(200), |r| {
        r.clients[viper].ended.is_some()
    }));
    let last = last_scores(&rig, viper);
    assert_eq!(last.fight, Fight::FreeForAll);
    assert_eq!(last.winner, Winner::Player(cobra_id));
    assert_eq!(last.players[0].callsign, "Cobra", "the winner ranks first");
}

#[test]
fn a_mission_ended_by_the_time_limit_names_the_winner_by_the_tally() {
    let (mut rig, viper, _) = two_sides(
        Mode::Pvp,
        &[
            (number::KILL_LIMIT, 0),
            (number::TALLY, ScoreTally::Damage.value()),
        ],
    );
    let seats = seats_of(&rig);
    rig.host
        .tally(damage(&rig, ENEMY_HUMAN, FRIENDLY_HUMAN, 0.5), &seats);
    // Viper has more kills, Cobra more damage: the tally decides.
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_AI);
    rig.run(Duration::from_millis(50));
    rig.host.end_mission(EndReason::TimeLimit, None);
    rig.run(Duration::from_millis(100));
    let last = last_scores(&rig, viper);
    assert_eq!(last.tally, ScoreTally::Damage);
    assert_eq!(last.winner, Winner::Side(Side::Enemy));
    assert_eq!(last.players[0].callsign, "Cobra", "ranked by damage");
    assert_eq!(last.players[0].damage, 500);
    // Ended by the operator: no winner.
    let (mut rig, viper, _) = two_sides(Mode::Pvp, &[(number::KILL_LIMIT, 0)]);
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_AI);
    rig.run(Duration::from_millis(50));
    rig.host.end();
    rig.run(Duration::from_millis(100));
    assert_eq!(last_scores(&rig, viper).winner, Winner::NoneYet);
}

#[test]
fn co_op_keeps_scores_but_no_kill_limit_no_free_for_all_and_no_winner() {
    // A co-op mission, both players on the friendly side.
    let mut rig = Rig::new(spec(2, 2, 50), config(), LinkConfig::one_way(5 * MS));
    let viper = rig.join(|c| c.callsign = "Viper".into());
    let hawk = rig.join(|c| c.callsign = "Hawk".into());
    assert!(rig.run_until(Duration::from_secs(3), |r| {
        r.seated(viper) && r.seated(hawk)
    }));
    assert_eq!(rig.host.settings.mode(), Mode::Coop);
    // The registry keeps co-op's scoring settings, which do not apply.
    assert_eq!(rig.host.fight(), Fight::Sides);
    assert_eq!(rig.host.kill_limit(), None);
    // In co-op both enemy planes are the AI's.
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_HUMAN);
    shoot_down(&mut rig.host, FRIENDLY_HUMAN, ENEMY_AI);
    rig.run(Duration::from_millis(1200));
    assert_eq!(rig.host.phase(), Phase::Flying);
    let scores = last_scores(&rig, hawk);
    assert_eq!(scores.players[0].kills, 2);
    assert_eq!(scores.kill_limit, 0);
    assert_eq!(
        scores.seconds_left, None,
        "co-op has no time limit by default"
    );
    rig.host.end_mission(EndReason::TimeLimit, None);
    rig.run(Duration::from_millis(100));
    assert_eq!(last_scores(&rig, hawk).winner, Winner::NoneYet);
}

#[test]
fn scores_go_out_at_most_once_a_second_and_only_when_they_change() {
    let (mut rig, viper, cobra) = two_sides(Mode::Pvp, &[(number::KILL_LIMIT, 0)]);
    // The first Scores reaches each player within a second of seating.
    rig.run(Duration::from_millis(1100));
    let first = rig.clients[viper].scores.len();
    assert!(first >= 1);
    // Nothing changes: nothing more.
    rig.run(Duration::from_secs(2));
    assert_eq!(rig.clients[viper].scores.len(), first);
    // Damage every tick for three seconds: one message a second at most.
    let start = rig.clients[viper].scores.len();
    let end = rig.net.now() + Duration::from_secs(3);
    let mut fed = 0;
    while rig.net.now() < end {
        let seats = seats_of(&rig);
        rig.host
            .tally(damage(&rig, FRIENDLY_HUMAN, ENEMY_HUMAN, 0.001), &seats);
        fed += 1;
        rig.step();
    }
    assert!(fed > 1000);
    let sent = rig.clients[viper].scores.len() - start;
    assert!((2..=4).contains(&sent), "{sent} messages in three seconds");
    // A late joiner gets the scores once seated, with itself listed.
    let late = rig.join(|c| c.callsign = "Hawk".into());
    rig.clients[late].ready = Some(Some(FRIENDLY_AI));
    assert!(rig.run_until(Duration::from_secs(2), |r| r.seated(late)));
    rig.run(Duration::from_millis(1100));
    let scores = last_scores(&rig, late);
    assert_eq!(scores.players.len(), 3);
    assert!(scores.players.iter().any(|p| p.callsign == "Hawk"));
    // A player who leaves leaves the list.
    rig.clients[cobra].leave();
    assert!(rig.run_until(Duration::from_secs(4), |r| r.closed(cobra)));
    rig.run(Duration::from_millis(1100));
    assert!(
        last_scores(&rig, viper)
            .players
            .iter()
            .all(|p| p.callsign != "Cobra")
    );
    assert!(rig.clients.iter().all(|c| c.errors.is_empty()));
}
