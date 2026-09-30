//! A fingerprint of the whole mission tick.
//!
//! [`World::step`] runs the simulation half of one 120 Hz tick: the player's
//! flight, building contact, the weather clock, turbulence, combat, the
//! airport service, the AI and the radio. Added with the mission core
//! (docs/ARCHITECTURE.md, "Mission core and seats", "Drivers"), this test
//! flies a synthetic mission through `World::step` with scripted input, folds
//! named behaviour fields into a fingerprint every tick, and compares the
//! result with a recorded value, so a refactor that reorders, drops or
//! duplicates a step fails loudly.
//!
//! The conventions are those of `crates/tore-sim/src/golden_tests.rs`:
//!
//! - Only named fields are hashed, never the `Debug` text of a struct, so a
//!   new field never moves the fingerprint. `Debug` text is used only for
//!   enums: a variant name and the plain values it carries.
//! - The mission runs twice in one process and must match itself on every
//!   platform.
//! - The recorded value is compared only on macOS on Apple silicon, because
//!   maths library results can differ in the last bit between platforms. It
//!   is `None` until the first macOS CI run: that run fails with the value in
//!   its message, and the value goes into [`RECORDED`] in the commit that
//!   records it. Nothing can produce it on Linux.
//! - A deliberate behaviour change, or a step moved on purpose, updates the
//!   recorded value in the same commit, and the commit message says why.
//! - `TORE_GOLDEN_VERBOSE=1` prints the fingerprint every 120 ticks and at
//!   the end. Running it before and after a change shows the tick where the
//!   first difference appears.
//!
//! The mission uses synthetic fixtures only, no retail data.

use super::*;
use crate::{
    combat::fixtures,
    seats::SeatCommand,
    test_support::{aircraft, payload, spawned, terrain as world},
};
use std::fmt::{self, Write as _};
use tore_formats::aircraft::Token;
use tore_input::{PilotCommand, Switch};
use tore_sim::{
    ai::engagement::GroupObjective,
    airport::{
        Airport, Allegiance, Command, OrientedBox, Runway, Scene, Service, SourceKey, StaticObject,
    },
    combat::live::Event,
};

/// The recorded fingerprint of [`fly`], or `None` until macOS CI records it.
const RECORDED: Option<u64> = None;

/// Recorded values are compared only where they are generated.
const RECORDED_PLATFORM: bool = cfg!(all(target_os = "macos", target_arch = "aarch64"));

/// Ticks flown: ten seconds of mission time.
const TICKS: usize = 1200;

/// Order-sensitive 64-bit fingerprint. Each value is folded in as one word
/// through the SplitMix64 finalizer, a bijection, so a change to any single
/// word always changes the result. Same folding as tore-sim's golden tests.
#[derive(Clone, Copy)]
struct Fingerprint(u64);

impl Fingerprint {
    fn new() -> Self {
        Self(0x746f_7265_7469_636b)
    }

    fn word(&mut self, word: u64) {
        let mut z = (self.0 ^ word).wrapping_add(0x9e37_79b9_7f4a_7c15);
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        self.0 = z ^ (z >> 31);
    }

    fn f64(&mut self, value: f64) {
        self.word(value.to_bits());
    }

    fn vector(&mut self, value: [f64; 3]) {
        for axis in value {
            self.f64(axis);
        }
    }

    fn bool(&mut self, value: bool) {
        self.word(u64::from(value));
    }

    fn int(&mut self, value: impl Into<i64>) {
        self.word(value.into() as u64);
    }

    fn u64(&mut self, value: u64) {
        self.word(value);
    }

    /// A length or an index, recorded before the items it counts.
    fn count(&mut self, value: usize) {
        self.word(value as u64);
    }

    fn text(&mut self, text: &str) {
        self.count(text.len());
        let _ = self.write_str(text);
    }

    /// An enum's `Debug` text: its variant and any plain values it carries.
    /// Never used for a struct, which may gain fields.
    fn name(&mut self, value: &impl fmt::Debug) {
        let _ = write!(self, "{value:?}");
        self.word(u64::MAX);
    }

    fn option<T>(&mut self, value: Option<T>, record: impl FnOnce(&mut Self, T)) {
        match value {
            None => self.word(0),
            Some(value) => {
                self.word(1);
                record(self, value);
            }
        }
    }
}

impl fmt::Write for Fingerprint {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            self.word(u64::from(byte));
        }
        Ok(())
    }
}

/// What one run reports: the fingerprint, the fingerprint every 120 ticks so
/// a mismatch can say where two runs first differ, and counts of what the
/// mission did, so the scenario cannot quietly stop exercising a system.
struct Run {
    total: u64,
    parts: Vec<(usize, u64)>,
    seen: Seen,
}

#[derive(Default)]
struct Seen {
    fired: u32,
    hits: u32,
    player_damaged: u32,
    messages: u32,
    tower: u32,
    radio: u32,
    emissions: u32,
    releases: u32,
    ai_moved: bool,
}

/// A synthetic airport with one runway, ahead of and below the player, so the
/// tower, the airport service and the runway targets all have work to do.
pub(super) fn airport() -> Scene {
    let bounds = OrientedBox {
        center: [3000., 100., 30000.],
        half: [100., 10., 5000.],
        heading: 0.,
        pitch: 0.,
        bank: 0.,
    };
    Scene {
        objects: vec![StaticObject {
            id: 1000,
            source: SourceKey {
                layout: "TICK.MM".into(),
                ordinal: 0,
            },
            name: "Strip".into(),
            object_type: "STRIP.OT".into(),
            bounds,
            hit_points: 100,
            category: 0x100,
            radar_signature: 100.,
            infrared_signature: 100.,
            runway: true,
        }],
        runways: vec![Runway {
            object: 1000,
            airport: 7,
            name: "09/27".into(),
            surface: bounds,
            approach_center: bounds.center,
            elevation_ft: 100.,
            heading: 0.,
            length_ft: 10000.,
        }],
        airports: vec![Airport {
            id: 7,
            name: "Field".into(),
            runway_objects: vec![1000],
            allegiance: Allegiance::Friendly,
            neutral_permission: false,
        }],
    }
}

/// Altitudes in feet. The player flies low enough over the fixture's rising
/// ground for turbulence to act. The AI aircraft fly higher, over the drones.
const PLAYER_ALTITUDE: f64 = 900.;
const AI_ALTITUDE: f64 = 1500.;

/// The mission: the player low over rising ground flying toward two enemy
/// aircraft, with two friendly aircraft nearby, two drones that are moved into
/// the gun's sights before each burst and an airport further on. All four AI
/// aircraft are F/A-18D rows; the player carries the synthetic gun and missile
/// station.
pub(super) fn mission() -> World {
    let mut terrain = world();
    terrain.airport_scene = airport();
    let profile = player_aircraft();
    let mut flight = flight::State::new(&profile, [0., PLAYER_ALTITUDE, -2000.]).unwrap();
    flight.speed = 600.;
    let mut combat = fixtures::combat(Vec::new(), Vec::new());
    combat.apply_startup_weapons();
    // The AI rows: a friendly pair facing an enemy pair, closed to 9,000 ft.
    let mut targets = spawned();
    for target in &mut targets {
        target.position[1] = AI_ALTITUDE;
    }
    for target in &mut targets[2..] {
        target.position[2] = 9000.;
    }
    // Two drones for the player's gun, parked far out of everyone's sight;
    // `place_drone` moves each into the sights. Big enough that a burst hits
    // whichever way the flight model points.
    for (n, id) in DRONES.into_iter().enumerate() {
        let mut drone = targets[0].clone();
        drone.id = id;
        drone.position = [0., AI_ALTITUDE, -60_000. - 10_000. * n as f64];
        drone.velocity = [0.; 3];
        drone.radius = 150.;
        targets.push(drone);
    }
    combat.state.targets = targets;
    combat.add_airport_targets(&terrain.airport_scene).unwrap();
    let mut wings = ai_wings::AiWings::build_with(&payload(None), &combat.state.targets, 0, |_| {
        Ok((aircraft(), None))
    })
    .unwrap();
    wings.apply_mission_preset(ai_wings::Preset::Free, flight.position);
    wings.apply_group_objectives(&[GroupObjective::Inherit; 6], flight.position);
    combat.state.own_mut().friendlies = wings.friendly_ids(tore_sim::ai::launch::Side::Friendly);
    wings.mirror_pose_out(&mut combat.state.targets);
    let mut airport_service = Service::new(&terrain.airport_scene).unwrap();
    airport_service.command(
        &terrain.airport_scene,
        airport_aircraft(&terrain, &flight, false),
        Command::SelectAirport(7),
    );
    let phrases = tore_formats::radio::STEMS
        .iter()
        .map(|(stem, _)| (stem.to_string(), format!("Synthetic {stem}")))
        .collect();
    let roster = Roster::single_player(
        Some(comms::Crew::Rio),
        ai_planes(&wings).collect::<Vec<_>>(),
    );
    World {
        terrain,
        roster,
        cockpits: vec![Cockpit {
            plane: PlaneId(0),
            previous_flight: flight.clone(),
            flight,
            airport_service,
            airport_nav_mode: false,
            turbulence: tore_sim::turbulence::Turbulence::default(),
            turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng::seeded(1).unwrap(),
            airfield_radio: Default::default(),
            crew_voice: crew_voice::CrewVoice::new(&profile),
            result: Default::default(),
            overspeed_message_at: None,
            edge_message_at: None,
        }],
        combat,
        ai_wings: Some(wings),
        comms: comms::Comms::new(1),
        wing_status: Default::default(),
        radio: Default::default(),
        phrases,
        // The step never reads the setup.
        setup: Setup::default(),
    }
}

/// The player's aircraft: the fixture aircraft, which has no turbulence
/// sensitivity of its own, given some.
pub(super) fn player_aircraft() -> tore_formats::aircraft::Aircraft {
    let mut profile = aircraft();
    profile.fields.insert(
        "turbulencePercent".into(),
        Token {
            kind: "word".into(),
            value: "100".into(),
            scaled: false,
        },
    );
    profile
}

/// The drones' target ids, one per burst.
pub(super) const DRONES: [u32; 2] = [5, 6];

/// Put a drone 3,000 ft dead ahead of the player's nose, just before a burst,
/// so the gun has something to hit whatever the flight model did to the
/// heading.
pub(super) fn place_drone(world: &mut World, id: u32) {
    let player = &world.cockpits[0].flight;
    let forward = attitude::Basis::new(player.yaw, player.pitch, player.bank).forward;
    let position = player.position;
    let drone = world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|target| target.id == id)
        .unwrap();
    drone.position = std::array::from_fn(|i| position[i] + forward[i] * 3000.);
}

/// The scripted input for one tick: stick movement, a throttle change, gear
/// and flap commands, weapon-page cycles, airport commands and two bursts of
/// the trigger.
pub(super) fn script(tick: usize) -> SeatInput {
    let mut input = SeatInput::default();
    let pilot = &mut input.pilot;
    match tick {
        // Radar on and the gear up, as after takeoff.
        10 => pilot.commands.push(PilotCommand::Set(Switch::Radar, true)),
        30 => pilot.commands.push(PilotCommand::Toggle(Switch::Gear)),
        // A throttle change, then afterburner.
        100 => pilot.commands.push(PilotCommand::Throttle(0.6)),
        400 => {
            pilot.commands.push(PilotCommand::Throttle(1.));
            pilot.commands.push(PilotCommand::Set(Switch::Burner, true));
        }
        500 => pilot.commands.push(PilotCommand::Toggle(Switch::Flaps)),
        700 => pilot.commands.push(PilotCommand::Toggle(Switch::Airbrake)),
        _ => {}
    }
    // Stick: a roll, a pull, then a rudder push, with the level flight
    // between them that the gun bursts need.
    match tick {
        300..360 => pilot.roll = 0.5,
        360..420 => pilot.roll = -0.5,
        600..680 => pilot.pitch = 0.3,
        800..900 => pilot.yaw = 0.2,
        1000..1100 => pilot.pitch = -0.2,
        _ => {}
    }
    match tick {
        // Tower calls, then the navigation mode on and off.
        60 => input
            .commands
            .push(SeatCommand::Airport(AirportInput::Command(
                Command::SelectAirport(7),
            ))),
        90 => input
            .commands
            .push(SeatCommand::Airport(AirportInput::Command(
                Command::RequestLanding,
            ))),
        110 => input
            .commands
            .push(SeatCommand::Airport(AirportInput::NavMode)),
        130 => input
            .commands
            .push(SeatCommand::Airport(AirportInput::NavMode)),
        800 => input
            .commands
            .push(SeatCommand::Airport(AirportInput::Command(
                Command::RepeatReply,
            ))),
        900 => input
            .commands
            .push(SeatCommand::Airport(AirportInput::Command(
                Command::CancelApproach,
            ))),
        // Weapon page: to the missile and back to the gun.
        150 => input
            .commands
            .push(SeatCommand::CycleWeapon { forward: true }),
        170 => input
            .commands
            .push(SeatCommand::CycleWeapon { forward: false }),
        _ => {}
    }
    // The trigger: a burst at the drone and a second one later.
    input.trigger = matches!(tick, 200..260 | 700..730);
    input
}

fn record_event(fp: &mut Fingerprint, event: &Event) {
    match event {
        Event::Fired { station, .. } => {
            fp.u64(1);
            fp.count(*station);
        }
        Event::SeekerActivated(id) => {
            fp.u64(2);
            fp.int(*id);
        }
        Event::Pitbull(id) => {
            fp.u64(3);
            fp.int(*id);
        }
        Event::Hit(id) => {
            fp.u64(4);
            fp.int(*id);
        }
        Event::Destroyed(id) => {
            fp.u64(5);
            fp.int(*id);
        }
        Event::Airburst(id) => {
            fp.u64(6);
            fp.int(*id);
        }
        Event::Ground => fp.u64(7),
        Event::TrackLost(id) => {
            fp.u64(8);
            fp.int(*id);
        }
        Event::OwnshipDamaged { amount, .. } => {
            fp.u64(9);
            fp.int(*amount);
        }
        Event::SubsystemDamaged { index, .. } => {
            fp.u64(10);
            fp.count(*index);
        }
        Event::OwnshipDestroyed { .. } => fp.u64(11),
        Event::PilotKilled { .. } => fp.u64(12),
        Event::OwnshipGroundImpact { .. } => fp.u64(13),
        Event::Defeated(id) => {
            fp.u64(14);
            fp.int(*id);
        }
        Event::Jolt(jolt) => {
            fp.u64(15);
            // The fingerprint keeps the earlier encoding: none for the ownship.
            fp.option((jolt.target != 0).then_some(jolt.target), |fp, id| {
                fp.int(id)
            });
            fp.vector(jolt.from);
            fp.f64(jolt.strength);
        }
    }
}

fn record_cue(fp: &mut Fingerprint, cue: &Cue) {
    match cue {
        Cue::Message { text, .. } => {
            fp.u64(1);
            fp.text(text);
        }
        Cue::Feedback { event, .. } => {
            fp.u64(2);
            fp.name(event);
        }
        Cue::Tower { stem, .. } => {
            fp.u64(3);
            fp.option(*stem, |fp, name| fp.text(name));
        }
        Cue::WeaponCycled { .. } => fp.u64(4),
        Cue::Flown => fp.u64(5),
        Cue::CombatStepped => fp.u64(6),
        Cue::WingEjection {
            id,
            message,
            friendly,
        } => {
            fp.u64(7);
            fp.int(*id);
            fp.text(message);
            fp.bool(*friendly);
        }
        Cue::Picture => fp.u64(8),
        Cue::OrderVoice { stems, .. } => {
            fp.u64(10);
            fp.count(stems.len());
            for stem in stems {
                fp.text(stem);
            }
        }
        Cue::Radio { call, .. } => {
            fp.u64(9);
            fp.text(&call.label);
            fp.text(&call.text);
            fp.name(&call.kind);
            fp.name(&call.route);
            fp.f64(call.delay);
        }
    }
}

/// The player's flight state: where it is and how it flies.
fn record_player(fp: &mut Fingerprint, s: &flight::State) {
    fp.vector(s.position);
    fp.vector(s.velocity);
    fp.f64(s.yaw);
    fp.f64(s.pitch);
    fp.f64(s.bank);
    fp.f64(s.speed);
    fp.f64(s.g);
    fp.f64(s.fuel);
    fp.f64(s.throttle);
    fp.bool(s.engine);
    fp.bool(s.burner);
    for device in [s.gear, s.flaps, s.brake, s.bay] {
        fp.f64(device);
    }
    fp.f64(s.damage_fraction);
    fp.bool(s.crashed);
    fp.u64(s.ticks);
}

/// Everything one tick changed: the state it left, then what it reported.
fn record_tick(fp: &mut Fingerprint, world: &World, out: &TickOutput, seen: &mut Seen) {
    record_player(fp, &world.cockpits[0].flight);
    let combat = &world.combat.state;
    fp.u64(combat.tick());
    fp.int(combat.own().hp);
    let stations = combat.own().configuration().stations.len();
    fp.count(stations);
    for station in 0..stations {
        fp.int(combat.own().rounds(station));
    }
    fp.count(combat.projectiles.len());
    for projectile in &combat.projectiles {
        fp.int(projectile.id);
        fp.vector(projectile.position);
    }
    fp.count(combat.targets.len());
    for target in &combat.targets {
        fp.int(target.id);
        fp.vector(target.position);
        fp.int(target.hp);
    }
    if let Some(wings) = &world.ai_wings {
        let actors = wings.mission().actors();
        fp.count(actors.len());
        for actor in actors {
            fp.int(actor.id());
            fp.vector(actor.flight().position);
            fp.f64(actor.flight().yaw);
        }
    }
    fp.int(world.terrain.weather.ticks());
    fp.int(world.terrain.weather.seconds_of_day());
    fp.bool(world.cockpits[0].airport_nav_mode);

    fp.count(out.cues.len());
    for cue in &out.cues {
        record_cue(fp, cue);
        match cue {
            Cue::Message { .. } => seen.messages += 1,
            Cue::Tower { .. } => seen.tower += 1,
            Cue::Radio { .. } => seen.radio += 1,
            _ => {}
        }
    }
    fp.count(out.events.len());
    for event in &out.events {
        record_event(fp, event);
        match event {
            Event::Fired { .. } => seen.fired += 1,
            Event::Hit(_) => seen.hits += 1,
            Event::OwnshipDamaged { .. } => seen.player_damaged += 1,
            _ => {}
        }
    }
    fp.count(out.releases.len());
    for release in &out.releases {
        fp.text(&release.sound);
        fp.count(release.station);
    }
    seen.releases += out.releases.len() as u32;
    fp.count(out.outcomes.len());
    for outcome in &out.outcomes {
        fp.int(outcome.projectile);
    }
    fp.option(out.journal.as_ref(), |fp, batch| {
        fp.count(batch.entries.len());
        fp.u64(batch.dropped);
    });
    fp.count(out.emissions.len());
    for emission in &out.emissions {
        fp.name(&emission.kind);
        fp.vector(emission.position);
        fp.bool(emission.arrived);
        fp.bool(emission.own);
    }
    seen.emissions += out.emissions.len() as u32;
    fp.option(out.fault.as_deref(), |fp, fault| fp.text(fault));
}

/// Fly the mission for [`TICKS`] ticks and fingerprint every one.
fn fly() -> Run {
    let mut world = mission();
    let start: Vec<_> = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actors()
        .iter()
        .map(|actor| actor.flight().position)
        .collect();
    let mut fp = Fingerprint::new();
    let mut parts = Vec::new();
    let mut seen = Seen::default();
    let mut out = TickOutput::default();
    for tick in 0..TICKS {
        match tick {
            195 => place_drone(&mut world, DRONES[0]),
            695 => place_drone(&mut world, DRONES[1]),
            _ => {}
        }
        let mut input = script(tick);
        input.tick = world.tick();
        world.step(&[input], &mut out).unwrap();
        record_tick(&mut fp, &world, &out, &mut seen);
        if (tick + 1) % 120 == 0 {
            parts.push((tick + 1, fp.0));
        }
    }
    seen.ai_moved = world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actors()
        .iter()
        .zip(&start)
        .any(|(actor, start)| actor.flight().position != *start);
    Run {
        total: fp.0,
        parts,
        seen,
    }
}

#[test]
fn mission_tick_matches_recorded_fingerprint() {
    let (first, second) = (fly(), fly());
    if std::env::var_os("TORE_GOLDEN_VERBOSE").is_some() {
        for (tick, value) in &first.parts {
            eprintln!("golden world/tick {tick} {value:#018x}");
        }
        let seen = &first.seen;
        eprintln!(
            "golden world/tick seen fired={} hits={} player_damaged={} messages={} tower={} \
             radio={} releases={} emissions={}",
            seen.fired,
            seen.hits,
            seen.player_damaged,
            seen.messages,
            seen.tower,
            seen.radio,
            seen.releases,
            seen.emissions,
        );
        eprintln!("golden world/tick total {:#018x}", first.total);
    }
    let divergence = first
        .parts
        .iter()
        .zip(&second.parts)
        .find(|(a, b)| a != b)
        .map_or("the end".to_string(), |(a, _)| format!("tick {}", a.0));
    assert_eq!(
        first.total, second.total,
        "world/tick is not deterministic: two identical runs gave {:#018x} and {:#018x} \
         (first difference at {divergence})",
        first.total, second.total,
    );
    // The scenario must keep exercising the systems its name promises.
    let seen = &first.seen;
    assert!(seen.fired > 0, "the player's gun never fired");
    assert!(seen.hits > 0, "the player's burst never hit the drone");
    assert!(seen.messages > 0, "the tower and HUD never spoke");
    assert!(seen.tower > 0, "the tower never sent a cue");
    assert!(seen.radio > 0, "no radio call was delivered");
    assert!(seen.ai_moved, "the AI aircraft never moved");
    assert!(seen.emissions > 0, "no sound was emitted");
    if RECORDED_PLATFORM {
        match RECORDED {
            Some(recorded) => assert_eq!(
                first.total, recorded,
                "the mission tick changed: recorded {recorded:#018x}, now {:#018x}. \
                 A step was reordered, dropped or changed. If that is deliberate, update \
                 RECORDED in crates/tore-world/src/world/tick_tests.rs in the same commit and \
                 explain the change in the commit message. Run with TORE_GOLDEN_VERBOSE=1 \
                 before and after to see the tick where the first difference appears.",
                first.total,
            ),
            None => panic!(
                "the mission tick fingerprint is not recorded yet: set RECORDED to Some({:#018x}) \
                 in crates/tore-world/src/world/tick_tests.rs",
                first.total,
            ),
        }
    }
}

/// The mission with a second human, seat 1, as the second member of Friendly
/// Wing 1 beside the first cockpit's plane. The fixture's AI keeps flying that
/// plane's actor, which still speaks for itself; only the radio is checked.
fn two_seat_mission() -> World {
    let mut world = mission();
    let second = PlaneId(2);
    let slot = Slot {
        wing: tore_sim::ai::launch::WingId {
            side: tore_sim::ai::launch::Side::Friendly,
            index: 0,
        },
        member: 1,
    };
    let ai: Vec<_> = world
        .roster
        .planes()
        .iter()
        .filter(|plane| plane.id != PlaneId(0) && plane.id != second)
        .map(|plane| (plane.id, plane.slot))
        .collect();
    world.roster = Roster::with_humans(
        [
            (
                PlaneId(0),
                Slot::FRIENDLY_LEAD,
                SeatId(0),
                Some(comms::Crew::Rio),
            ),
            (second, slot, SeatId(1), None),
        ],
        ai,
    );
    world
        .comms
        .set_seats(world.roster.seats().iter().map(|seat| seat.id));
    let first = &world.cockpits[0];
    let mut flight = first.flight.clone();
    flight.position[0] += 300.;
    let profile = crate::test_support::profile();
    world.cockpits.push(Cockpit {
        plane: second,
        previous_flight: flight.clone(),
        flight,
        turbulence: Default::default(),
        turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng::seeded(1).unwrap(),
        airport_service: first.airport_service.clone(),
        airport_nav_mode: false,
        airfield_radio: airfield_radio::AirfieldRadio::for_seat(SeatId(1), second.0),
        crew_voice: crew_voice::CrewVoice::new(&profile).for_seat(SeatId(1), second.0),
        result: Default::default(),
        overspeed_message_at: None,
        edge_message_at: None,
    });
    world
}

/// Two seats in one flight: a call the first seat makes is one call, heard by
/// both under their own labels with one variant, while each crew voice speaks
/// only to its own seat and each seat has its own hold.
#[test]
fn every_seat_hears_a_call_once_with_the_same_variant() {
    let mut world = two_seat_mission();
    let mut out = TickOutput::default();
    let mut heard: Vec<(usize, SeatId, comms::Call)> = Vec::new();
    let mut journal = Vec::new();
    for tick in 0..TICKS {
        match tick {
            195 => place_drone(&mut world, DRONES[0]),
            695 => place_drone(&mut world, DRONES[1]),
            _ => {}
        }
        let mut first = script(tick);
        first.tick = world.tick();
        let second = SeatInput {
            seat: SeatId(1),
            tick: world.tick(),
            ..SeatInput::default()
        };
        world.step(&[first, second], &mut out).unwrap();
        for cue in &out.cues {
            if let Cue::Radio { seat, call } = cue {
                heard.push((tick, *seat, call.clone()));
            }
        }
        journal.extend(world.comms.take_journal());
    }
    // The first seat's gun hit or release is a call to its flight: both seats hear it,
    // the same words, each under the name that seat knows the speaker by.
    let hit = |seat: SeatId| {
        heard
            .iter()
            .find(|(_, s, call)| {
                *s == seat
                    && call.route == comms::Route::Radio
                    && call.origin.speaker == Some(0)
                    && call.origin.audience == comms::journal::Audience::Flight
            })
            .map(|(tick, _, call)| (*tick, call.clone()))
    };
    let (tick0, first) = hit(SeatId(0)).expect("seat 0 hears its own call to the flight");
    let (tick1, second) = hit(SeatId(1)).expect("seat 1 hears its flight leader's call");
    assert_eq!(tick0, tick1, "one call, delivered together");
    assert_eq!(first.stems, second.stems, "one variant for both");
    assert_eq!(
        (first.label.as_str(), second.label.as_str()),
        ("YOU", "Red one")
    );
    let entries: Vec<_> = journal
        .iter()
        .filter(|e| e.stems == first.stems && e.call.is_some())
        .collect();
    assert_eq!(entries.len(), 2, "one entry queues it and one delivers it");
    assert_eq!(entries[0].call, entries[1].call, "one journal number");
    assert!(entries.iter().all(|e| e.heard_by == [SeatId(0), SeatId(1)]));
    // A crew voice speaks to its own seat: each plane's scream is its own
    // seat's alone, and the tower's answers are the asking seat's.
    let screams: Vec<_> = heard
        .iter()
        .filter(|(_, _, call)| call.route == comms::Route::Direct)
        .map(|(_, seat, _)| *seat)
        .collect();
    assert_eq!(screams, [SeatId(0), SeatId(1)]);
    assert!(
        heard
            .iter()
            .filter(|(_, _, call)| call.text.contains("Field: cleared"))
            .all(|(_, seat, _)| *seat == SeatId(0)),
        "the first seat's tower request is answered to it"
    );
}

/// The radio names built from the roster are the AI's own radio members, in
/// the same order, plus the human-flown plane in Red one's place.
#[test]
fn radio_members_from_the_roster_match_the_ai_wings() {
    let world = mission();
    let wings = world.ai_wings.as_ref().unwrap();
    let members = radio_calls::members(&world.roster, Some(wings), |_| true);
    let ai = wings.radio_members();
    assert_eq!(&members[..ai.len()], &ai[..]);
    assert_eq!(members.len(), ai.len() + 1);
    let player = &members[ai.len()];
    assert_eq!(
        (
            player.id,
            player.enemy,
            player.flight,
            player.position,
            player.alive
        ),
        (0, false, 0, 0, true)
    );
}

/// The mission result call comes from the core for every seat, whatever the
/// audio: once, two seconds after the result is decided, to a seat that holds
/// it under its own label.
#[test]
fn the_core_sends_the_mission_result_to_every_seat() {
    let mut world = two_seat_mission();
    world.setup.mission = Some((5000., 3000.));
    // The fixture's enemies are already down at the first check, which would
    // disable the calls as a result decided at the start; begin each seat's
    // checks as they would with the enemies flying and then shot down.
    for cockpit in &mut world.cockpits {
        cockpit.result = ai_wings::outcome::Tracker::default();
        cockpit.result.step(0., Some(|| false), [0.; 3], true);
    }
    let mut out = TickOutput::default();
    let mut calls: Vec<(usize, SeatId, comms::Call)> = Vec::new();
    let mut journal = Vec::new();
    for tick in 0..1500 {
        let inputs = [
            SeatInput {
                seat: SeatId(0),
                tick: world.tick(),
                ..SeatInput::default()
            },
            SeatInput {
                seat: SeatId(1),
                tick: world.tick(),
                ..SeatInput::default()
            },
        ];
        world.step(&inputs, &mut out).unwrap();
        for cue in &out.cues {
            if let Cue::Radio { seat, call } = cue
                && call.stems.first().is_some_and(|stem| stem == "^MISSACC")
            {
                calls.push((tick, *seat, call.clone()));
            }
        }
        journal.extend(world.comms.take_journal());
    }
    assert_eq!(calls.len(), 2, "one call for each seat, once: {calls:?}");
    let (tick0, seat0, first) = &calls[0];
    let (tick1, seat1, second) = &calls[1];
    assert_eq!((*seat0, *seat1), (SeatId(0), SeatId(1)));
    assert_eq!(tick0, tick1);
    assert_eq!(
        (first.label.as_str(), second.label.as_str()),
        ("RIO", "YOU")
    );
    assert_eq!(first.kind, comms::Kind::Important);
    // Decided on the first 4 s check after the enemies fell, then 2 s later.
    let decided = (480 + 240) as usize;
    assert!(
        (decided..decided + 3).contains(tick0),
        "delivered at tick {tick0}"
    );
    assert!(
        journal.iter().any(
            |e| e.origin.cause == comms::journal::Cause::MissionAccomplished
                && e.heard_by == [SeatId(1)]
        ),
        "the call is journaled for each seat"
    );
}

/// A mission whose result is decided before the flight starts (the fixture's
/// enemies are down at the first check), or a flight with no mission, never
/// sends the calls.
#[test]
fn no_result_call_without_a_mission_or_when_decided_at_the_start() {
    for setup in [None, Some((5000., 3000.))] {
        let mut world = mission();
        world.setup.mission = setup;
        let mut out = TickOutput::default();
        for _ in 0..1500 {
            let input = SeatInput {
                tick: world.tick(),
                ..SeatInput::default()
            };
            world.step(&[input], &mut out).unwrap();
            assert!(
                !out.cues
                    .iter()
                    .any(|cue| matches!(cue, Cue::Radio { call, .. }
                    if call.stems.first().is_some_and(|s| s == "^MISSACC"))),
                "{setup:?}"
            );
        }
    }
}

/// The mission with a second human-flown plane, 50, that combat carries as a
/// second ownship. Plane 50 is no AI aircraft, so no target row shares its id.
fn two_ownship_mission() -> World {
    let mut world = mission();
    let second = PlaneId(50);
    let slot = Slot {
        wing: tore_sim::ai::launch::WingId {
            side: tore_sim::ai::launch::Side::Friendly,
            index: 0,
        },
        member: 5,
    };
    let ai: Vec<_> = world
        .roster
        .planes()
        .iter()
        .filter(|plane| plane.id != PlaneId(0))
        .map(|plane| (plane.id, plane.slot))
        .collect();
    world.roster = Roster::with_humans(
        [
            (
                PlaneId(0),
                Slot::FRIENDLY_LEAD,
                SeatId(0),
                Some(comms::Crew::Rio),
            ),
            (second, slot, SeatId(1), None),
        ],
        ai,
    );
    world
        .comms
        .set_seats(world.roster.seats().iter().map(|seat| seat.id));
    let first = &world.cockpits[0];
    let mut flight = first.flight.clone();
    flight.position[0] += 300.;
    let profile = crate::test_support::profile();
    world.cockpits.push(Cockpit {
        plane: second,
        previous_flight: flight.clone(),
        flight,
        turbulence: Default::default(),
        turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng::seeded(1).unwrap(),
        airport_service: first.airport_service.clone(),
        airport_nav_mode: false,
        airfield_radio: airfield_radio::AirfieldRadio::for_seat(SeatId(1), second.0),
        crew_voice: crew_voice::CrewVoice::new(&profile).for_seat(SeatId(1), second.0),
        result: Default::default(),
        overspeed_message_at: None,
        edge_message_at: None,
    });
    let config = world.combat.own().configuration().clone();
    let mut ownship =
        tore_sim::combat::live::Ownship::new(second.0, world.combat.own().side, config, true)
            .unwrap();
    ownship.armed = true;
    world.combat.add_ownship(ownship, Vec::new()).unwrap();
    world
}

fn tick_both(world: &mut World, held: [bool; 2], out: &mut TickOutput) {
    let tick = world.tick();
    let inputs: Vec<_> = (0..2)
        .map(|n| SeatInput {
            seat: SeatId(n as u8),
            tick,
            trigger: held[n],
            ..SeatInput::default()
        })
        .collect();
    world.step(&inputs, out).unwrap();
}

/// Combat steps every human-flown plane: each plane's own damage reaches its
/// own flight, and each seat's trigger fires its own ownship.
#[test]
fn combat_steps_each_human_flown_plane_with_its_own_ownship() {
    let mut world = two_ownship_mission();
    let mut out = TickOutput::default();
    // A hit on the second plane only.
    let launcher = combat::launcher(&world.cockpits[1].flight);
    world
        .combat
        .command_for(50, tore_sim::combat::live::Command::DamagePlayer, launcher);
    tick_both(&mut world, [false, false], &mut out);
    assert!(
        out.events
            .iter()
            .any(|e| matches!(e, Event::OwnshipDamaged { aircraft: 50, .. }))
    );
    assert!(
        !out.events
            .iter()
            .any(|e| matches!(e, Event::OwnshipDamaged { aircraft: 0, .. }))
    );
    assert!(world.cockpits[1].flight.damage_fraction > 0.);
    assert_eq!(world.cockpits[0].flight.damage_fraction, 0.);
    let hp = |world: &World, plane: u32| world.combat.state.ownship(plane).unwrap().hp;
    let capacity = world.combat.own().configuration().damage_capacity;
    assert_eq!(hp(&world, 0), capacity);
    assert!(hp(&world, 50) < capacity);
    // Each seat's trigger fires its own ownship's gun.
    let mut fired = Vec::new();
    for _ in 0..30 {
        tick_both(&mut world, [false, true], &mut out);
        fired.extend(out.events.iter().filter_map(|e| match e {
            Event::Fired { aircraft, .. } => Some(*aircraft),
            _ => None,
        }));
    }
    assert!(
        !fired.is_empty() && fired.iter().all(|aircraft| *aircraft == 50),
        "{fired:?}"
    );
    assert!(world.combat.state.ownship(50).unwrap().shots > 0);
    assert_eq!(world.combat.state.ownship(0).unwrap().shots, 0);
    // A plane whose ownship is destroyed is dead to the radio, the other is not.
    world.combat.state.ownship_mut(50).unwrap().hp = 0;
    assert!(world.cockpit_alive(0));
    assert!(!world.cockpit_alive(1));
}

/// Every human-flown plane is in the tick's picture: the first as the player,
/// the other as an ordinary aircraft, and taking one out of combat gives back
/// its ownship as it stands.
#[test]
fn every_human_flown_plane_is_in_the_picture_and_leaves_with_its_ownship() {
    let mut world = two_ownship_mission();
    let mut out = TickOutput::default();
    tick_both(&mut world, [false, false], &mut out);
    let snapshot = world.combat.render_snapshot();
    assert_eq!(snapshot.player.id, 0);
    let other = snapshot
        .targets
        .iter()
        .find(|t| t.id == 50)
        .expect("the second human-flown plane is drawn");
    assert_eq!(other.position, world.cockpits[1].flight.position);
    assert!(other.airborne && !other.crashed);
    assert_eq!(
        snapshot.targets.iter().filter(|t| t.id == 50).count(),
        1,
        "once"
    );
    assert!(snapshot.models.contains(&other.aircraft.unwrap()));
    // The first plane stays the only player; giving the second back returns
    // its damage and countermeasures.
    world.combat.state.ownship_mut(50).unwrap().chaff = 3;
    let given_back = world.combat.remove_ownship(50).expect("it is an ownship");
    assert_eq!((given_back.aircraft, given_back.chaff), (50, 3));
    assert!(world.combat.state.ownship(50).is_none());
    assert!(
        world.combat.remove_ownship(0).is_none(),
        "the presented plane stays"
    );
    tick_both(&mut world, [false, false], &mut out);
    assert!(
        world
            .combat
            .render_snapshot()
            .targets
            .iter()
            .all(|t| t.id != 50)
    );
}

/// The AI hears of every human-flown plane with an ownship, in id order, each
/// with its place in its wing from the roster; a plane's side sets its
/// designation skip list.
#[test]
fn the_ai_is_handed_every_human_flown_plane_with_its_place_in_its_wing() {
    let mut world = two_ownship_mission();
    let mut out = TickOutput::default();
    for _ in 0..3 {
        tick_both(&mut world, [false, false], &mut out);
    }
    let humans = world.ai_wings.as_ref().unwrap().mission().humans();
    let seen: Vec<_> = humans.iter().map(|h| (h.id, h.wing, h.member)).collect();
    assert_eq!(seen, [(0, 0, 0), (50, 0, 5)]);
    // Each ownship skips the aircraft of its own side, humans included.
    world.refresh_friendlies();
    for plane in [0, 50] {
        let friends = &world.combat.state.ownship(plane).unwrap().friendlies;
        assert!(friends.contains(&0) && friends.contains(&50), "{friends:?}");
    }
}

/// A human wingman takes the lead: the previous leader is alive to say it, and
/// only the new leader's seat hears "You're the Wingleader now", five seconds
/// after the change.
#[test]
fn a_human_wingman_who_takes_the_lead_hears_the_call() {
    let mut world = two_seat_mission();
    let mut out = TickOutput::default();
    let mut heard = Vec::new();
    for tick in 0..900 {
        if tick == 100 {
            world
                .ai_wings
                .as_mut()
                .unwrap()
                .chatter
                .push(ai_wings::Chatter::Leadership {
                    speaker: 1,
                    side: tore_sim::ai::launch::Side::Friendly,
                    wing_number: 1,
                    leader: 2,
                    previous_pilot_alive: true,
                });
        }
        let inputs = [SeatId(0), SeatId(1)].map(|seat| SeatInput {
            seat,
            tick: world.tick(),
            ..SeatInput::default()
        });
        world.step(&inputs, &mut out).unwrap();
        for cue in &out.cues {
            if let Cue::Radio { seat, call } = cue
                && call.stems.first().is_some_and(|stem| stem == "^WNGLDR")
            {
                heard.push((tick, *seat, call.clone()));
            }
        }
    }
    assert_eq!(heard.len(), 1, "{heard:?}");
    let (tick, seat, call) = &heard[0];
    assert_eq!(*seat, SeatId(1));
    assert!((100 + 600..100 + 603).contains(tick), "tick {tick}");
    assert_eq!(call.kind, comms::Kind::Important);
}

/// The radio's flight leaders are the AI mission's current leaders.
#[test]
fn radio_leaders_are_the_ai_missions_current_leaders() {
    let world = mission();
    let wings = world.ai_wings.as_ref().unwrap();
    let members = radio_calls::members(&world.roster, Some(wings), |_| true);
    let leaders = radio_calls::leaders(&world.roster, &members, Some(wings));
    assert!(
        leaders.contains(&(0, 0)),
        "Red one leads the first flight: {leaders:?}"
    );
    for (flight, leader) in &leaders {
        assert!(
            members
                .iter()
                .any(|m| m.id == *leader && m.flight == *flight)
        );
    }
}

/// Two seats' inputs for the next tick, with `commands` for the seat they name.
fn tick_seats(
    world: &mut World,
    held: [bool; 2],
    commands: [Vec<SeatCommand>; 2],
    out: &mut TickOutput,
) {
    let tick = world.tick();
    let [first, second] = commands;
    let inputs: Vec<_> = [first, second]
        .into_iter()
        .enumerate()
        .map(|(n, commands)| SeatInput {
            seat: SeatId(n as u8),
            tick,
            trigger: held[n],
            commands,
            ..SeatInput::default()
        })
        .collect();
    world.step(&inputs, out).unwrap();
}

/// [`two_ownship_mission`] with both ownships' weapons making a fire sound,
/// which the synthetic weapons lack.
fn sounding_mission() -> World {
    let mut world = two_ownship_mission();
    let mut config = world.combat.own().configuration().clone();
    for station in &mut config.stations {
        station.weapon.fire_sound = Some("&TESTFIRE".into());
    }
    let side = world.combat.own().side;
    for plane in [50, 0] {
        world.combat.state.remove_ownship(plane).unwrap();
        let mut ownship =
            tore_sim::combat::live::Ownship::new(plane, side, config.clone(), true).unwrap();
        ownship.armed = true;
        world.combat.add_ownship(ownship, Vec::new()).unwrap();
    }
    world
}

/// The messages a tick gave `seat`, in order.
fn messages_for(out: &TickOutput, seat: SeatId) -> Vec<&str> {
    out.cues
        .iter()
        .filter_map(|cue| match cue {
            Cue::Message { seat: to, text } if *to == seat => Some(text.as_str()),
            _ => None,
        })
        .collect()
}

/// Every seat-specific output names the seat it is for: a seat's commands,
/// controller feedback and weapon release sounds come back under its own
/// name, and never under another's.
#[test]
fn each_seat_gets_its_own_messages_feedback_and_releases() {
    let mut world = sounding_mission();
    let mut out = TickOutput::default();
    let (one, two) = (SeatId(0), SeatId(1));
    // A command's line goes to the seat that gave it.
    tick_seats(
        &mut world,
        [false, false],
        [
            vec![SeatCommand::ReleaseFlare],
            vec![SeatCommand::ReleaseChaff],
        ],
        &mut out,
    );
    let flare = messages_for(&out, one);
    let chaff = messages_for(&out, two);
    assert!(
        matches!(&flare[..], [text] if text.contains("lares") || text.starts_with("Flare launched")),
        "{flare:?}"
    );
    assert!(
        matches!(&chaff[..], [text] if text.contains("haff")),
        "{chaff:?}"
    );
    assert_eq!(out.commanded, 2);
    // A seat's trigger fires its own plane: the sound and the rumble are its.
    let mut released = Vec::new();
    let mut felt = Vec::new();
    for (held, seat) in [([false, true], two), ([true, false], one)] {
        for _ in 0..30 {
            tick_seats(&mut world, held, [vec![], vec![]], &mut out);
            released.extend(out.releases.iter().map(|release| release.seat));
            felt.extend(out.cues.iter().filter_map(|cue| match cue {
                Cue::Feedback { seat, event } => Some((*seat, *event)),
                _ => None,
            }));
        }
        assert!(
            released.contains(&seat),
            "{seat:?} never fired: {released:?}"
        );
        assert!(
            felt.iter()
                .any(|(to, event)| *to == seat
                    && matches!(event, tore_input::FeedbackEvent::GunFired)),
            "{seat:?} felt nothing: {felt:?}"
        );
        // Nothing of the other seat's.
        assert!(released.iter().all(|to| *to == seat), "{released:?}");
        assert!(felt.iter().all(|(to, _)| *to == seat), "{felt:?}");
        released.clear();
        felt.clear();
        for _ in 0..600 {
            tick_seats(&mut world, [false, false], [vec![], vec![]], &mut out);
        }
    }
}

/// A plane's own systems messages are drained every tick into its own seat,
/// second cockpit included, so no queue grows unread.
#[test]
fn every_cockpits_systems_messages_are_drained_into_its_seat() {
    let mut world = two_ownship_mission();
    let mut out = TickOutput::default();
    for tick in 0..5 {
        world.cockpits[1]
            .flight
            .systems
            .notify(format!("second {tick}"));
        world.cockpits[0]
            .flight
            .systems
            .notify(format!("first {tick}"));
        tick_seats(&mut world, [false, false], [vec![], vec![]], &mut out);
        assert!(world.cockpits[0].flight.systems.messages.is_empty());
        assert!(world.cockpits[1].flight.systems.messages.is_empty());
        let first = messages_for(&out, SeatId(0));
        let second = messages_for(&out, SeatId(1));
        assert!(
            first.contains(&format!("first {tick}").as_str()),
            "{first:?}"
        );
        assert!(
            second.contains(&format!("second {tick}").as_str()),
            "{second:?}"
        );
        assert!(!first.iter().any(|text| text.starts_with("second")));
        assert!(!second.iter().any(|text| text.starts_with("first")));
    }
}

/// The ground sensor's refusal is a message for the seat that pressed the gear
/// key, and only that seat: the plane with weight on its wheels keeps its gear
/// down, and the other seat's plane raises its gear in the air without a word.
#[test]
fn a_refused_gear_press_is_a_message_for_its_own_seat_only() {
    let mut world = two_ownship_mission();
    let mut out = TickOutput::default();
    for cockpit in &mut world.cockpits {
        cockpit.flight.enable_research(1).unwrap();
        cockpit.flight.gear_down = true;
        cockpit.flight.gear = 1.;
    }
    // Seat 1's plane has weight on its wheels; seat 0's is flying.
    world.cockpits[1]
        .flight
        .research
        .as_mut()
        .unwrap()
        .on_ground = true;
    assert!(world.cockpits[1].flight.weight_on_wheels());
    assert!(!world.cockpits[0].flight.weight_on_wheels());
    let tick = world.tick();
    let inputs: Vec<_> = (0..2)
        .map(|n| SeatInput {
            seat: SeatId(n),
            tick,
            pilot: tore_input::PilotInput {
                commands: vec![tore_input::PilotCommand::Set(
                    tore_input::Switch::Gear,
                    false,
                )],
                ..Default::default()
            },
            ..SeatInput::default()
        })
        .collect();
    world.step(&inputs, &mut out).unwrap();
    let message = tore_sim::flight::GROUND_SENSOR_MESSAGE;
    assert_eq!(messages_for(&out, SeatId(1)), [message], "{:?}", out.cues);
    assert!(!messages_for(&out, SeatId(0)).contains(&message));
    assert!(world.cockpits[1].flight.gear_down);
    assert!(!world.cockpits[0].flight.gear_down);
}

/// A plane's airburst reads "Your aircraft exploded" to its own seat and
/// "Destroyed aircraft exploded" to every other, as single player reads it
/// for the first aircraft and the rest.
#[test]
fn an_airburst_reads_differently_to_the_seat_that_flew_the_plane() {
    let mut world = two_ownship_mission();
    let mut out = TickOutput::default();
    let mut wreck = tore_sim::wreck::Wreck::new(50, 0, [0.; 3]);
    wreck.phase = tore_sim::wreck::Phase::Exploded;
    world.cockpits[1].flight.wreck = Some(wreck);
    tick_seats(&mut world, [false, false], [vec![], vec![]], &mut out);
    assert!(
        out.events
            .iter()
            .any(|event| matches!(event, Event::Airburst(50)))
    );
    assert_eq!(
        messages_for(&out, SeatId(1))
            .iter()
            .filter(|text| **text == "Your aircraft exploded")
            .count(),
        1
    );
    assert!(!messages_for(&out, SeatId(0)).contains(&"Your aircraft exploded"));
    assert_eq!(
        messages_for(&out, SeatId(0))
            .iter()
            .filter(|text| **text == "Destroyed aircraft exploded")
            .count(),
        1
    );
}

/// The AI wings' HUD line goes to the seats flying in Friendly Wing 1 only.
#[test]
fn the_ai_wings_line_goes_to_the_seats_in_friendly_wing_one() {
    let mut world = two_ownship_mission();
    assert!(world.flies_in_first_friendly_wing(0));
    assert!(world.flies_in_first_friendly_wing(1));
    let enemy = tore_sim::ai::launch::WingId {
        side: tore_sim::ai::launch::Side::Enemy,
        index: 0,
    };
    let ai: Vec<_> = world
        .roster
        .planes()
        .iter()
        .filter(|plane| plane.id != PlaneId(0) && plane.id != PlaneId(50))
        .map(|plane| (plane.id, plane.slot))
        .collect();
    world.roster = Roster::with_humans(
        [
            (PlaneId(0), Slot::FRIENDLY_LEAD, SeatId(0), None),
            (
                PlaneId(50),
                Slot {
                    wing: enemy,
                    member: 0,
                },
                SeatId(1),
                None,
            ),
        ],
        ai,
    );
    assert!(world.flies_in_first_friendly_wing(0));
    assert!(!world.flies_in_first_friendly_wing(1));
}
