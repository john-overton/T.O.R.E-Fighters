//! Gun linking, the gunsight and the trigger on the ordinary AC-130 path.
//!
//! John reported (2026-10-09) that the gun-link keys only worked with
//! `--live-fire`. The world refused every manual combat command outside the
//! range except arming, the seeker mode and clearing the designation, and the
//! gun-group commands were among the refused. These tests fly the synthetic
//! AC-130 as a plain single-player mission, with `combat.range` off exactly
//! as a Quick Mission, a campaign mission or a multiplayer flight has it, and
//! drive the commands the way the keys and the gamepad send them.

use super::*;
use crate::{
    mission::{MissionSpec, Start},
    seats::SeatCommand,
    test_support::resources::{THEATER, gunship_resources},
};
use tore_formats::aircraft::AircraftId;
use tore_sim::combat::{gunship::Sight, live::Command as Live};

/// A synthetic AC-130 over the synthetic ground, in the ordinary game: no
/// range, no tape, the airborne startup load.
fn gunship() -> World {
    let mut spec = MissionSpec::new(THEATER, AircraftId::Ac130);
    spec.start = Start::Airborne { altitude_ft: 5_000 };
    let world = World::new(&spec, &gunship_resources(), Seating::SinglePlayer).unwrap();
    assert!(!world.combat.range, "this is not the live-fire range");
    assert!(world.combat.uses_normal_startup_defaults());
    world
}

fn step(world: &mut World, trigger: bool, commands: Vec<SeatCommand>) -> TickOutput {
    let mut out = TickOutput::default();
    world
        .step(
            &[SeatInput {
                tick: world.tick(),
                trigger,
                commands,
                ..SeatInput::default()
            }],
            &mut out,
        )
        .unwrap();
    out
}

fn mask(world: &World) -> u8 {
    world
        .combat
        .state
        .own()
        .gunship
        .as_ref()
        .expect("an AC-130 has a gun group")
        .mask()
}

/// The group mask the cockpit readout (and so the network and a replay) shows.
fn shown_mask(world: &World) -> u8 {
    let launcher = crate::combat::launcher(&world.cockpits[0].flight);
    world
        .cockpit_readout(SeatId(0), launcher)
        .unwrap()
        .stores
        .gun_group
}

fn refused(out: &TickOutput) -> bool {
    out.cues.iter().any(
        |cue| matches!(cue, Cue::Message { text, .. } if text.contains("requires --live-fire")),
    )
}

#[test]
fn linking_works_without_the_live_fire_range() {
    let mut world = gunship();
    step(&mut world, false, Vec::new());
    assert_eq!(mask(&world), 0b001, "the first gun alone to start");
    assert_eq!(shown_mask(&world), 0b001);
    // Ctrl+7 then Ctrl+8, as the keys send them (the app queues Manual).
    for expected in [0b011, 0b111] {
        let out = step(
            &mut world,
            false,
            vec![
                SeatCommand::Manual(Live::NextGunGroup),
                SeatCommand::Manual(Live::ToggleGunGroup),
            ],
        );
        assert!(!refused(&out), "a gun-group key is not a range command");
        assert_eq!(mask(&world), expected);
        assert_eq!(shown_mask(&world), expected, "the readout shows the link");
    }
    // Toggling the candidate again takes it back out.
    step(
        &mut world,
        false,
        vec![SeatCommand::Manual(Live::ToggleGunGroup)],
    );
    assert_eq!(mask(&world), 0b011);
}

#[test]
fn a_linked_group_fires_every_member_without_the_range() {
    let mut world = gunship();
    step(
        &mut world,
        false,
        vec![
            SeatCommand::Manual(Live::NextGunGroup),
            SeatCommand::Manual(Live::ToggleGunGroup),
            SeatCommand::Manual(Live::NextGunGroup),
            SeatCommand::Manual(Live::ToggleGunGroup),
        ],
    );
    assert_eq!(mask(&world), 0b111);
    let before = world.combat.state.own().ammo.clone();
    for _ in 0..240 {
        step(&mut world, true, Vec::new());
    }
    let after = &world.combat.state.own().ammo;
    let stations = world.combat.state.own().gunship.clone().unwrap().stations;
    for station in stations.into_iter().flatten() {
        assert!(
            after[station] < before[station],
            "station {station} fired: {before:?} then {after:?}"
        );
    }
}

#[test]
fn the_other_gunsight_commands_work_without_the_range() {
    let mut world = gunship();
    step(&mut world, false, Vec::new());
    // Backslash pins the ground (nothing is held under the crosshair), and L
    // drops the pin: neither is a range command.
    let out = step(
        &mut world,
        false,
        vec![SeatCommand::Combat(Live::SightPinGround)],
    );
    assert!(!refused(&out));
    let sight = |world: &World| world.combat.state.own().gunship.as_ref().unwrap().sight;
    assert!(matches!(sight(&world), Sight::Pinned(_)), "pinned");
    let out = step(
        &mut world,
        false,
        vec![SeatCommand::Manual(Live::ClearDesignation)],
    );
    assert!(!refused(&out));
    assert_eq!(sight(&world), Sight::Free, "L drops the pin");
    // The slew and zoom ride the seat input.
    let mut out = TickOutput::default();
    for tick in world.tick()..world.tick() + 60 {
        world
            .step(
                &[SeatInput {
                    tick,
                    sight: [127, 0],
                    sight_zoom: 3,
                    ..SeatInput::default()
                }],
                &mut out,
            )
            .unwrap();
    }
    let guns = world.combat.state.own().gunship.clone().unwrap();
    assert_ne!(guns.look, tore_sim::combat::gunship::DEFAULT_LOOK, "slewed");
    assert_eq!(guns.zoom(), 3);
}

#[test]
fn range_only_commands_are_still_refused_outside_the_range() {
    let mut world = gunship();
    let out = step(
        &mut world,
        false,
        vec![SeatCommand::Manual(Live::CycleClass)],
    );
    assert!(refused(&out));
}
