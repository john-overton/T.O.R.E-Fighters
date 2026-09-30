//! The flight frame and the picture for any seat (docs/ARCHITECTURE.md, "The
//! flight screen draws a frame"): the crowd's planes each get a picture in
//! which their own plane is the player and every other human-flown plane,
//! plane 0 too, is an ordinary target. Synthetic fixtures only.

use super::crowd::{E_LEAD, F_HUMAN, F_LEAD, crowded_mission, inputs};
use super::*;
use crate::snapshot::RenderSnapshot;

/// A few ticks, so every plane's pose is written.
fn flown_crowd() -> World {
    let mut world = crowded_mission();
    let mut out = TickOutput::default();
    for _ in 0..30 {
        let step = inputs(&world, |seat| SeatInput {
            seat,
            ..SeatInput::default()
        });
        world.step(&step, &mut out).unwrap();
    }
    world
}

fn picture_for(world: &World, plane: PlaneId) -> RenderSnapshot {
    let cockpit = world
        .cockpits
        .iter()
        .find(|cockpit| cockpit.plane == plane)
        .unwrap();
    world
        .combat
        .snapshot(plane.0, &cockpit.flight, world.ai_wings.as_ref())
}

#[test]
fn a_second_seats_picture_has_its_plane_as_the_player_and_plane_0_as_a_target() {
    let world = flown_crowd();
    let first = picture_for(&world, F_LEAD);
    let second = picture_for(&world, F_HUMAN);

    assert_eq!(first.player.id, F_LEAD.0);
    assert!(first.targets.iter().any(|pose| pose.id == F_HUMAN.0));
    assert!(first.targets.iter().all(|pose| pose.id != F_LEAD.0));

    assert_eq!(second.player.id, F_HUMAN.0);
    assert_eq!(
        second.player.position, world.cockpits[1].flight.position,
        "the player pose is that seat's plane"
    );
    let lead = second
        .targets
        .iter()
        .find(|pose| pose.id == F_LEAD.0)
        .expect("plane 0 is among the second seat's targets");
    assert_eq!(lead.position, world.cockpits[0].flight.position);
    assert_eq!(
        lead.aircraft,
        Some(
            world
                .combat
                .state
                .ownship(F_LEAD.0)
                .unwrap()
                .configuration()
                .aircraft
        )
    );
    assert!(second.targets.iter().all(|pose| pose.id != F_HUMAN.0));
    // The crowd's other planes are the same targets in both pictures.
    for id in [E_LEAD.0, 2, 3, 6, 7] {
        assert!(first.targets.iter().any(|pose| pose.id == id));
        assert!(second.targets.iter().any(|pose| pose.id == id));
    }
    assert_eq!(first.targets.len(), second.targets.len());
    // Each seat's pose carries its own plane's damage and stores.
    assert_eq!(
        second.player.damage.hp,
        world.combat.state.ownship(F_HUMAN.0).unwrap().hp
    );
}

#[test]
fn the_history_follows_the_presented_plane_and_a_frame_reads_it() {
    let world = flown_crowd();
    assert_eq!(world.picture_plane(), F_LEAD);
    let picture = world.combat.render_snapshot();
    assert_eq!(picture.player.id, F_LEAD.0);

    let cues = [
        Cue::Message {
            seat: SeatId(1),
            text: "for seat 1".into(),
        },
        Cue::Message {
            seat: SeatId(0),
            text: "for seat 0".into(),
        },
        Cue::Flown,
    ];
    let presented = world.presented_flight(SeatId(1), 0.5);
    let frame = world
        .flight_frame(SeatId(1), presented, picture, &cues)
        .unwrap();
    assert_eq!(frame.seat, SeatId(1));
    assert_eq!(frame.plane, F_HUMAN);
    assert_eq!(frame.flight.position, world.cockpits[1].flight.position);
    let shown: Vec<usize> = frame.cues().map(|(index, _)| index).collect();
    assert_eq!(shown, [0, 2], "the seat's own cue and the mission-wide one");
    assert!(world.flight_frame(SeatId(9), None, picture, &[]).is_none());

    // Without a blended flight the frame draws the flight as the tick left it.
    let frame = world.flight_frame(SeatId(0), None, picture, &[]).unwrap();
    assert_eq!(frame.presented().position, frame.flight.position);
    assert_eq!(frame.plane, F_LEAD);
}
