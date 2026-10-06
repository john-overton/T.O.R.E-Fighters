//! Stage F phase 2's seams in the mission core (slice F2-0): the new seat
//! command and mission commands exist and do nothing yet, so single player
//! and the built handoffs are untouched.

use super::replies::Reply;
use super::revive::Spawn;
use super::tick_tests::mission;
use super::*;
use crate::mission::LoadoutSpec;
use crate::seats::SeatCommand;

fn input(world: &World, commands: Vec<SeatCommand>) -> SeatInput {
    SeatInput {
        seat: SeatId(0),
        tick: world.tick(),
        commands,
        ..SeatInput::default()
    }
}

/// Slice F2-R built the reply (world/replies_tests.rs): in single player the
/// plane leads its flight, so each key only says so and nothing else moves.
#[test]
fn a_single_player_wing_reply_only_says_the_plane_leads() {
    let mut world = mission();
    let mut out = TickOutput::default();
    let replies = Reply::ALL.map(SeatCommand::WingReply).to_vec();
    let input = input(&world, replies);
    world.step(&[input], &mut out).unwrap();
    let said: Vec<_> = out
        .cues
        .iter()
        .filter_map(|cue| match cue {
            Cue::Message { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(said, ["You lead this flight."; 4]);
    assert!(out.cues.iter().all(|cue| !matches!(cue, Cue::Radio { .. })));
    assert_eq!(world.tick(), 1);
}

/// Slice F2-V built both commands (world/revive_tests.rs); a plane that is
/// not lost is refused either, and the refused tick does not run.
#[test]
fn abandon_and_revive_refuse_a_plane_that_is_not_lost() {
    let mut world = mission();
    let mut out = TickOutput::default();
    let spawn = Spawn {
        position: [0., 20_000., 0.],
        heading_rad: 0.,
        speed_fps: 700.,
        loadout: LoadoutSpec {
            fuel_lbs: 10_000.,
            cheat: false,
            stations: Vec::new(),
        },
    };
    for command in [
        MissionCommand::Abandon { seat: SeatId(0) },
        MissionCommand::Revive {
            seat: SeatId(0),
            spawn: Box::new(spawn),
        },
    ] {
        let input = input(&world, Vec::new());
        let result = world.step_with(&[command], &[input], &mut out, |_, _| Ok(()));
        assert!(result.is_err());
        assert_eq!(world.tick(), 0, "the refused tick does not run");
        assert_eq!(world.roster.seat_of(PlaneId(0)), Some(SeatId(0)));
    }
}
