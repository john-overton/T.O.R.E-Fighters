//! A mission with several humans in each of two wings, for the stage B seat
//! tests (`fight_tests.rs`, `succession_tests.rs`). Synthetic fixtures only.
//!
//! Friendly Wing 1 is planes 0 (seat 0, the leader), 1, 2 and 3, and the
//! enemy's Wing 1 is planes 4, 5, 6 and 7. The AI flies every plane at the
//! start except plane 0; [`crowded_mission`] then hands plane 1 to seat 1 and
//! planes 4 and 5 to seats 2 and 3 through the handoff (`World::take_plane`),
//! so each side has two humans and two AI wingmen, and the enemy wing's leader
//! is human.

use super::tick_tests::mission;
use super::*;
use crate::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE},
    combat::fixtures,
    test_support::{aircraft, target},
};
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    ai::{
        engagement::GroupObjective,
        launch::{Side, WingId, WingSelection, resolve_wings},
        threat::{DispenserStore, SeekerClass},
    },
    combat::live,
    sensors,
};

/// The friendly wing: two humans and two AI wingmen.
pub(super) const F_LEAD: PlaneId = PlaneId(0);
pub(super) const F_HUMAN: PlaneId = PlaneId(1);
pub(super) const F_AI: [PlaneId; 2] = [PlaneId(2), PlaneId(3)];
/// The enemy wing: two humans, the leader one of them, and two AI wingmen.
pub(super) const E_LEAD: PlaneId = PlaneId(4);
pub(super) const E_HUMAN: PlaneId = PlaneId(5);
pub(super) const E_AI: [PlaneId; 2] = [PlaneId(6), PlaneId(7)];

/// The human-flown planes, in seat order.
pub(super) const HUMANS: [PlaneId; 4] = [F_LEAD, F_HUMAN, E_LEAD, E_HUMAN];

/// Altitude of the whole fight, feet: clear of the fixture's rising ground.
const ALTITUDE: f64 = 10_000.;
/// The wings start this far apart, feet, nose to nose.
const GAP: f64 = 12_000.;
/// The enemy wing is this far to the friendly wing's left, feet.
const OFFSET: f64 = 2_500.;

/// The mission with every plane but plane 0 flown by the AI, armed the way a
/// built mission arms it, and the two wings placed nose to nose.
pub(super) fn ai_mission() -> World {
    let mut world = mission();
    fixtures::set_types(&mut world.combat, fixtures::types());
    let selection = |side, count, skill_level| WingSelection {
        wing: WingId::new(side, 0).unwrap(),
        aircraft: AircraftId::F18,
        count,
        skill_level,
    };
    // Friendly members 1 to 3 (the player is member 0), then four enemies.
    let payload = resolve_wings(
        &[
            selection(Side::Friendly, 3, 1),
            selection(Side::Enemy, 4, 3),
        ],
        None,
    )
    .unwrap();
    // The fixture's four AI rows and two drones give way to this fight's
    // seven; the airport's rows stay.
    let mut rows: Vec<live::Target> = Vec::new();
    for (id, x, z) in [(1, 900., -900.), (2, -900., -900.), (3, 1800., -1800.)] {
        rows.push(target(id, [x, ALTITUDE, z], 0.));
    }
    for (id, x, z) in [
        (4, 0., GAP),
        (5, -900., GAP + 900.),
        (6, 900., GAP + 900.),
        (7, -1800., GAP + 1800.),
    ] {
        // Left of the friendly wing, so the wings pass each other.
        rows.push(target(id, [x + OFFSET, ALTITUDE, z], std::f64::consts::PI));
    }
    for row in &mut rows {
        let side = if row.id <= 3 {
            FRIENDLY_SIDE
        } else {
            ENEMY_SIDE
        };
        row.side = side;
    }
    world.combat.state.targets.retain(|t| t.id > 1000);
    let scenery = std::mem::take(&mut world.combat.state.targets);
    world.combat.state.targets = rows;
    world.combat.state.targets.extend(scenery);
    let mut wings = ai_wings::AiWings::build_with(&payload, &world.combat.state.targets, 0, |_| {
        Ok((aircraft(), Some(sensor_profiles())))
    })
    .unwrap();
    let start = [0., ALTITUDE, 0.];
    wings.apply_mission_preset(ai_wings::Preset::Free, start);
    wings.apply_group_objectives(&[GroupObjective::Inherit; 6], start);
    let config = world.combat.own().configuration().clone();
    for id in 1..=7 {
        let actor = wings.mission_mut().actor_mut(id).unwrap();
        actor.set_stations(ai_wings::station_specs(&config, false));
        actor.set_dispensers(vec![
            DispenserStore {
                class: SeekerClass::Infrared,
                count: 24,
            },
            DispenserStore {
                class: SeekerClass::Radar,
                count: 40,
            },
        ]);
    }
    wings.mirror_pose_out(&mut world.combat.state.targets);
    world.combat.state.own_mut().sensors = sensors::Sensors::new(sensor_profiles());
    world.roster = Roster::single_player(Some(comms::Crew::Rio), ai_planes(&wings));
    world.ai_wings = Some(wings);
    // Plane 0 flies at the head of the friendly wing.
    {
        let cockpit = &mut world.cockpits[0];
        cockpit.flight.position = start;
        cockpit.flight.speed = 600.;
        cockpit.flight.yaw = 0.;
        cockpit.flight.velocity = [0., 0., cockpit.flight.speed];
        cockpit.previous_flight = cockpit.flight.clone();
    }
    world.refresh_friendlies();
    world
}

/// [`ai_mission`] with seats 1, 2 and 3 taking planes 1, 4 and 5.
pub(super) fn crowded_mission() -> World {
    let mut world = ai_mission();
    for (seat, plane) in [
        (SeatId(1), F_HUMAN),
        (SeatId(2), E_LEAD),
        (SeatId(3), E_HUMAN),
    ] {
        world.take_plane(seat, plane).unwrap();
    }
    world
}

/// An input for every seat that flies, as `script` makes it for that seat.
pub(super) fn inputs(world: &World, mut script: impl FnMut(SeatId) -> SeatInput) -> Vec<SeatInput> {
    let tick = world.tick();
    world
        .roster
        .seats()
        .iter()
        .filter(|seat| seat.plane.is_some())
        .map(|seat| SeatInput {
            seat: seat.id,
            tick,
            ..script(seat.id)
        })
        .collect()
}

/// A radar that reaches 90 nautical miles and a visual channel that reaches
/// 10, so any two aircraft of the fight can find each other once the radar is
/// on.
fn sensor_profiles() -> sensors::SensorProfiles {
    let volume = |nmi: f64| sensors::Volume {
        azimuth_rad: 1.,
        elevation_rad: 1.,
        minimum_ft: 0.,
        maximum_ft: nmi * sensors::FEET_PER_NAUTICAL_MILE,
        minimum_relative_ft: f64::NEG_INFINITY,
        maximum_relative_ft: f64::INFINITY,
    };
    sensors::SensorProfiles {
        aircraft: AircraftId::F18,
        radar: Some(sensors::RadarProfile {
            record: "SYNTHETIC.SEE".into(),
            search: volume(90.),
            track: volume(50.),
            look_down: 0.,
            preset: sensors::Preset::Advanced,
            notch: sensors::Preset::Advanced.notch(),
            resistance: sensors::Preset::Advanced.resistance(),
            band: 0,
            source_flags: [0; 2],
            source_doppler: [0; 3],
        }),
        infrared: None,
        visual: Some(sensors::profile::VisualProfile {
            record: "SYNTHETIC.VIS".into(),
            search: volume(10.),
            track: volume(5.),
        }),
        jammer: None,
        signature: sensors::SignatureProfile::default(),
    }
}

/// The radio calls the tick delivered, with the seat each went to. Read
/// through this so the cue's shape lives in one place.
pub(super) fn radio_of(out: &TickOutput) -> Vec<(SeatId, comms::Call)> {
    out.cues
        .iter()
        .filter_map(|cue| match cue {
            Cue::Radio { seat, call } => Some((*seat, call.clone())),
            _ => None,
        })
        .collect()
}
