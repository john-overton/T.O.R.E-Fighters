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
fn airport() -> Scene {
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
fn mission() -> World {
    let mut terrain = world();
    terrain.airport_scene = airport();
    let mut profile = aircraft();
    // The fixture aircraft has no turbulence sensitivity of its own.
    profile.fields.insert(
        "turbulencePercent".into(),
        Token {
            kind: "word".into(),
            value: "100".into(),
            scaled: false,
        },
    );
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
    combat.state.friendlies = wings.friendly_ids();
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
    World {
        terrain,
        previous_flight: flight.clone(),
        flight,
        combat,
        ai_wings: Some(wings),
        airport_service,
        airport_nav_mode: false,
        turbulence: tore_sim::turbulence::Turbulence::default(),
        turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng::seeded(1).unwrap(),
        comms: comms::Comms::new(1),
        airfield_radio: Default::default(),
        radio: Default::default(),
        phrases,
        crew_voice: crew_voice::CrewVoice::new(&profile),
        overspeed_message_at: None,
        edge_message_at: None,
        // The step never reads the setup.
        setup: Setup::default(),
    }
}

/// The drones' target ids, one per burst.
const DRONES: [u32; 2] = [5, 6];

/// Put a drone 3,000 ft dead ahead of the player's nose, just before a burst,
/// so the gun has something to hit whatever the flight model did to the
/// heading.
fn place_drone(world: &mut World, id: u32) {
    let player = &world.flight;
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
fn script(tick: usize) -> TickInput {
    let mut input = TickInput {
        crew: Some(comms::Crew::Rio),
        ..TickInput::default()
    };
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
            .airport
            .push(AirportInput::Command(Command::SelectAirport(7))),
        90 => input
            .airport
            .push(AirportInput::Command(Command::RequestLanding)),
        110 => input.airport.push(AirportInput::NavMode),
        130 => input.airport.push(AirportInput::NavMode),
        800 => input
            .airport
            .push(AirportInput::Command(Command::RepeatReply)),
        900 => input
            .airport
            .push(AirportInput::Command(Command::CancelApproach)),
        // Weapon page: to the missile and back to the gun.
        150 => input.weapon_cycles.push(true),
        170 => input.weapon_cycles.push(false),
        _ => {}
    }
    // The trigger: a burst at the drone and a second one later.
    input.fire = matches!(tick, 200..260 | 700..730);
    input
}

fn record_event(fp: &mut Fingerprint, event: &Event) {
    match event {
        Event::Fired(station) => {
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
        Event::PlayerDamaged(amount) => {
            fp.u64(9);
            fp.int(*amount);
        }
        Event::SubsystemDamaged(station) => {
            fp.u64(10);
            fp.count(*station);
        }
        Event::PlayerDestroyed => fp.u64(11),
        Event::PilotKilled => fp.u64(12),
        Event::PlayerGroundImpact => fp.u64(13),
        Event::Defeated(id) => {
            fp.u64(14);
            fp.int(*id);
        }
        Event::Jolt(jolt) => {
            fp.u64(15);
            fp.option(jolt.target, |fp, id| fp.int(id));
            fp.vector(jolt.from);
            fp.f64(jolt.strength);
        }
    }
}

fn record_cue(fp: &mut Fingerprint, cue: &Cue) {
    match cue {
        Cue::Message(text) => {
            fp.u64(1);
            fp.text(text);
        }
        Cue::Feedback(event) => {
            fp.u64(2);
            fp.name(event);
        }
        Cue::Tower(audio) => {
            fp.u64(3);
            fp.option(*audio, |fp, name| fp.text(name));
        }
        Cue::WeaponCycled => fp.u64(4),
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
        Cue::Radio(call) => {
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
    record_player(fp, &world.flight);
    let combat = &world.combat.state;
    fp.u64(combat.tick());
    fp.int(combat.player_hp);
    let stations = combat.configuration().stations.len();
    fp.count(stations);
    for station in 0..stations {
        fp.int(combat.rounds(station));
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
    fp.bool(world.airport_nav_mode);

    fp.count(out.cues.len());
    for cue in &out.cues {
        record_cue(fp, cue);
        match cue {
            Cue::Message(_) => seen.messages += 1,
            Cue::Tower(_) => seen.tower += 1,
            Cue::Radio(_) => seen.radio += 1,
            _ => {}
        }
    }
    fp.count(out.events.len());
    for event in &out.events {
        record_event(fp, event);
        match event {
            Event::Fired(_) => seen.fired += 1,
            Event::Hit(_) => seen.hits += 1,
            Event::PlayerDamaged(_) => seen.player_damaged += 1,
            _ => {}
        }
    }
    fp.count(out.releases.len());
    for (name, station) in &out.releases {
        fp.text(name);
        fp.count(*station);
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
        world.step(&script(tick), &mut out).unwrap();
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
