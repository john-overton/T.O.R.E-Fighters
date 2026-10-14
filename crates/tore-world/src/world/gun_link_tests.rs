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
use tore_sim::autopilot::orbit::ConsoleCommand;
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

#[test]
fn jettison_works_without_the_range_and_leaves_the_gunships_guns_alone() {
    // Shift+K is a gameplay command now (John, 2026-10-10). Every AC-130 gun
    // is a fixed, internal station, so jettisoning with a gun selected
    // changes nothing: no refusal, no ammunition lost, no unlinked gun.
    let mut world = gunship();
    step(
        &mut world,
        false,
        vec![
            SeatCommand::Manual(Live::NextGunGroup),
            SeatCommand::Manual(Live::ToggleGunGroup),
        ],
    );
    let own = world.combat.state.own();
    assert!(
        own.configuration().stations.iter().all(|s| s.internal),
        "the AC-130 carries nothing external"
    );
    let (ammo, mask_before, payload) = (own.ammo.clone(), mask(&world), own.payload_lbs());
    for selected in 0..3 {
        world.combat.state.own_mut().selected = selected;
        let out = step(&mut world, false, vec![SeatCommand::Manual(Live::Jettison)]);
        assert!(!refused(&out), "Shift+K is not a range command");
    }
    let own = world.combat.state.own();
    assert_eq!(own.ammo, ammo, "no gun lost a round");
    assert_eq!(own.payload_lbs(), payload);
    assert_eq!(mask(&world), mask_before, "the link is as it was");
    // It still fires.
    for _ in 0..120 {
        step(&mut world, true, Vec::new());
    }
    assert!(world.combat.state.own().ammo != ammo, "the guns still fire");
}

fn said(out: &TickOutput, words: &str) -> bool {
    out.cues
        .iter()
        .any(|cue| matches!(cue, Cue::Message { text, .. } if text.contains(words)))
}

/// Ctrl+9 (slices nr-orbit and nr-cona): with a free sight the console opens
/// searching (no pin under the crosshair any more, John 2026-10-14); P pins
/// the ground under the crosshair and the autopilot orbits it, a new pin
/// moves the point, P again lets it go and the console searches again, and a
/// second Ctrl+9 hands the aircraft back.
#[test]
fn the_sensor_console_searches_then_orbits_the_pin() {
    use tore_sim::autopilot::{Mode, orbit::ConsolePhase};
    let mut world = gunship();
    step(&mut world, false, Vec::new());
    let sight = |world: &World| world.combat.state.own().gunship.as_ref().unwrap().sight;
    let phase = |world: &World| world.cockpits[0].flight.console_status().map(|s| s.phase);
    assert_eq!(sight(&world), Sight::Free);
    let out = step(&mut world, false, vec![SeatCommand::SensorConsole]);
    assert!(said(&out, "Sensor console: search"));
    assert_eq!(
        sight(&world),
        Sight::Free,
        "opening the console pins nothing"
    );
    assert_eq!(world.cockpits[0].flight.autopilot.mode(), Mode::Orbit);
    assert_eq!(phase(&world), Some(ConsolePhase::Search));
    assert!(world.cockpits[0].flight.orbit_status().is_none());
    // P: the sight pins the ground under its crosshair, the orbit follows.
    step(
        &mut world,
        false,
        vec![SeatCommand::Console(ConsoleCommand::Pin)],
    );
    step(&mut world, false, Vec::new());
    let Sight::Pinned(pin) = sight(&world) else {
        panic!("P pins a free sight");
    };
    assert_eq!(phase(&world), Some(ConsolePhase::Join));
    let status = world.cockpits[0].flight.orbit_status().unwrap();
    assert_eq!(status.point, pin);
    assert!(status.radius_ft >= status.minimum_radius_ft);
    // A new pin moves the point the orbit circles; the centre waits 5 s
    // with the pin standing still, then slides to it.
    let moved = [pin[0] + 2_000., pin[1], pin[2]];
    world.combat.state.own_mut().gunship.as_mut().unwrap().sight = Sight::Pinned(moved);
    for _ in 0..120 {
        step(&mut world, false, Vec::new());
    }
    let status = world.cockpits[0].flight.orbit_status().unwrap();
    assert_eq!(status.point, moved);
    assert_eq!(status.centre, pin, "waiting for the pin to stand still");
    for _ in 0..(4 * 120 + 10) {
        step(&mut world, false, Vec::new());
    }
    let status = world.cockpits[0].flight.orbit_status().unwrap();
    assert!(status.centre != pin && status.centre != moved, "sliding");
    // P again lets the pin go: the console searches on.
    step(
        &mut world,
        false,
        vec![SeatCommand::Console(ConsoleCommand::Pin)],
    );
    step(&mut world, false, Vec::new());
    assert_eq!(sight(&world), Sight::Free);
    assert_eq!(phase(&world), Some(ConsolePhase::Search));
    let out = step(&mut world, false, vec![SeatCommand::SensorConsole]);
    assert!(said(&out, "you have the aircraft"));
    assert_eq!(world.cockpits[0].flight.autopilot.mode(), Mode::Off);
    // Opened on a pin, the console orbits from the start.
    world.combat.state.own_mut().gunship.as_mut().unwrap().sight = Sight::Pinned(moved);
    let out = step(&mut world, false, vec![SeatCommand::SensorConsole]);
    assert!(said(&out, "Sensor console: orbit"));
    assert!(world.cockpits[0].flight.orbit_status().is_some());
}

/// The console's heading, altitude and speed keys reach the autopilot
/// through the seat command and answer with what they set; with the console
/// closed they say so.
#[test]
fn the_console_keys_step_the_search() {
    let mut world = gunship();
    step(&mut world, false, Vec::new());
    let out = step(
        &mut world,
        false,
        vec![SeatCommand::Console(ConsoleCommand::Altitude(1))],
    );
    assert!(said(&out, "Sensor console is closed"));
    step(&mut world, false, vec![SeatCommand::SensorConsole]);
    let before = world.cockpits[0].flight.console_status().unwrap();
    let out = step(
        &mut world,
        false,
        vec![
            SeatCommand::Console(ConsoleCommand::Heading(2)),
            SeatCommand::Console(ConsoleCommand::Altitude(-2)),
            SeatCommand::Console(ConsoleCommand::Speed(100)),
        ],
    );
    assert!(said(&out, "Search heading"));
    assert!(said(&out, "Search altitude"));
    assert!(said(&out, "Speed"));
    let after = world.cockpits[0].flight.console_status().unwrap();
    let turned = (after.heading_deg - before.heading_deg).rem_euclid(360.);
    assert!(turned > 5. && turned <= 15., "{turned}");
    assert!(after.altitude_ft < before.altitude_ft);
    assert!(after.speed_kt > before.speed_kt);
}

/// P on an aircraft without the gunsight says so.
#[test]
fn the_pin_key_is_the_gunships() {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.start = Start::Airborne { altitude_ft: 5_000 };
    let mut world = World::new(&spec, &gunship_resources(), Seating::SinglePlayer).unwrap();
    let out = step(
        &mut world,
        false,
        vec![SeatCommand::Console(ConsoleCommand::Pin)],
    );
    assert!(said(&out, "no gunsight"));
}

/// An aircraft with no gunsight refuses the console.
#[test]
fn the_sensor_console_is_the_ac130s() {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.start = Start::Airborne { altitude_ft: 5_000 };
    let mut world = World::new(&spec, &gunship_resources(), Seating::SinglePlayer).unwrap();
    let out = step(&mut world, false, vec![SeatCommand::SensorConsole]);
    assert!(said(&out, "No sensor console"));
    assert_eq!(
        world.cockpits[0].flight.autopilot.mode(),
        tore_sim::autopilot::Mode::Off
    );
}

/// In the console a slew off a track drops it to a pin on the ground under
/// the object (slice nr-cona); no slew, or a pin, is left alone.
#[test]
fn a_console_slew_drops_a_track_to_a_pin() {
    let mut guns = tore_sim::combat::gunship::State {
        sight: Sight::Tracked(7),
        aim: Some([1_000., 40., 2_000.]),
        ..Default::default()
    };
    assert!(!super::commands::drop_track_on_slew(
        &mut guns,
        [0, 0],
        |_, _| 12.
    ));
    assert_eq!(guns.sight, Sight::Tracked(7));
    assert!(super::commands::drop_track_on_slew(
        &mut guns,
        [64, 0],
        |_, _| 12.
    ));
    assert_eq!(guns.sight, Sight::Pinned([1_000., 12., 2_000.]));
    assert!(!super::commands::drop_track_on_slew(
        &mut guns,
        [64, 0],
        |_, _| 12.
    ));
}
