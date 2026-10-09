//! Two humans in each of two wings fly through a fight (docs/ARCHITECTURE.md,
//! "Mission core and seats", "How stage B lands", B7). Friendly Wing 1 and the
//! enemy's Wing 1 each have two humans and two AI wingmen, the enemy wing's
//! leader human; see `crowd.rs`. Every seat has scripted stick, radar and
//! trigger input, and the mission flies 3,000 ticks (25 seconds).
//!
//! What the run proves: it repeats exactly; every human's sensors find the
//! other humans; ownships hit each other, with friendly fire on, and hit AI
//! aircraft; the AI wingmen of a human-led wing fly on their human leader,
//! also after the lead passes to the other human; each seat hears its own
//! radio and gets its own HUD lines; and each seat's debrief inputs name its own
//! plane.
//!
//! Open-loop stick input cannot aim a gun, so each burst starts with the
//! victim moved onto the line the first round will fly (as `tick_tests.rs`
//! moves its drones into the gun's sights), and moved away again once the burst
//! has landed. The stick input still flies every plane, and the AI flies its
//! own fight around them.

use super::crowd::*;
use super::*;
use crate::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE, outcome},
    comms::journal,
};
use std::collections::BTreeSet;
use tore_input::{PilotCommand, PilotInput, Switch};
use tore_sim::{
    ai::launch::Side,
    combat::{
        ledger::Resolution,
        live::{Event, FriendlyFire},
    },
};

const TICKS: usize = 3000;

/// One gun burst: the shooter holds the trigger for 16 ticks (two rounds) with
/// the victim on the line its first round will fly, 40 ticks ahead.
struct Burst {
    start: usize,
    shooter: PlaneId,
    victim: PlaneId,
    /// What the burst is for.
    what: &'static str,
}

const BURSTS: [Burst; 6] = [
    Burst {
        start: 300,
        shooter: F_LEAD,
        victim: E_LEAD,
        what: "a friendly human hits an enemy human",
    },
    Burst {
        start: 450,
        shooter: E_HUMAN,
        victim: F_HUMAN,
        what: "an enemy human hits a friendly human",
    },
    Burst {
        start: 600,
        shooter: F_HUMAN,
        victim: F_AI[0],
        what: "a human hits its own AI wingman",
    },
    Burst {
        start: 750,
        shooter: F_LEAD,
        victim: E_AI[0],
        what: "a friendly human hits an enemy AI aircraft",
    },
    Burst {
        start: 900,
        shooter: E_LEAD,
        victim: F_AI[1],
        what: "an enemy human hits a friendly AI aircraft",
    },
    // Plane 0 is nearly finished, so this burst from its own wingman shoots
    // it down: friendly fire between humans, and the lead passes on.
    Burst {
        start: 1050,
        shooter: F_HUMAN,
        victim: F_LEAD,
        what: "a human shoots down its own leader",
    },
];

/// Rounds leave at this speed in feet per second, whatever the shooter flies.
const ROUND_FPS: f64 = 1032.;
/// The ticks the trigger is held: two rounds, fifteen ticks apart.
const HELD: usize = 16;
/// The burst's first round reaches the victim after this many ticks.
const FLIGHT: f64 = 40.;
/// The victim is moved out of the way this many ticks after the burst starts.
const CLEAR: usize = 90;
/// How far to the shooter's right the victim is moved, feet.
const SIDEWAYS: f64 = 4000.;
/// The burst's outcome is read for this many ticks after it starts.
const WATCH: usize = 160;
/// The AI wingmen fly within this many feet of their human leader while the
/// leaders turn and pull, before the first burst.
const TRAILING_FT: f64 = 3000.;
/// The ticks at which the AI wingmen are looked at: during the leaders' first
/// turn, at its end, and after the friendly lead has passed on.
const SAMPLES: [usize; 3] = [140, 280, 1200];

fn flight_of(world: &mut World, plane: PlaneId) -> &mut flight::State {
    if let Some(index) = world.cockpits.iter().position(|c| c.plane == plane) {
        return &mut world.cockpits[index].flight;
    }
    world
        .ai_wings
        .as_mut()
        .unwrap()
        .mission_mut()
        .actor_mut(plane.0)
        .expect("an AI plane")
        .flight_mut()
}

fn move_row(world: &mut World, plane: PlaneId, position: [f64; 3]) {
    if let Some(row) = world
        .combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == plane.0)
    {
        row.position = position;
    }
}

/// Puts the victim where the burst's first round will be after [`FLIGHT`]
/// ticks, less what the victim flies until then, on the shooter's velocity.
fn aim(world: &mut World, burst: &Burst) {
    let shooter = flight_of(world, burst.shooter).clone();
    let forward = attitude::Basis::new(shooter.yaw, shooter.pitch, shooter.bank).forward;
    let position: [f64; 3] = std::array::from_fn(|i| {
        shooter.position[i] + (forward[i] * ROUND_FPS - shooter.velocity[i]) / 120. * FLIGHT
    });
    let victim = flight_of(world, burst.victim);
    victim.position = position;
    victim.velocity = shooter.velocity;
    (victim.yaw, victim.pitch, victim.bank) = (shooter.yaw, shooter.pitch, shooter.bank);
    victim.speed = shooter.speed;
    move_row(world, burst.victim, position);
}

/// Moves the victim out of everyone's way, to the shooter's right.
fn clear(world: &mut World, burst: &Burst) {
    let shooter = flight_of(world, burst.shooter).clone();
    let right = attitude::Basis::new(shooter.yaw, shooter.pitch, shooter.bank).right;
    let victim = flight_of(world, burst.victim);
    let position: [f64; 3] = std::array::from_fn(|i| victim.position[i] + right[i] * SIDEWAYS);
    victim.position = position;
    move_row(world, burst.victim, position);
}

/// One seat's stick and radar for the tick: the friendly humans turn left and
/// pull, the enemy humans turn right and pull, then everyone rolls out and
/// pulls again.
fn pilot(seat: SeatId, tick: usize) -> PilotInput {
    let side = if seat.0 < 2 { 1. } else { -1. };
    let mut input = PilotInput::default();
    if tick == 10 {
        input.commands.push(PilotCommand::Set(Switch::Radar, true));
    }
    match tick {
        120..180 => input.roll = 0.4 * side,
        180..240 => input.pitch = 0.3,
        240..300 => input.roll = -0.4 * side,
        1200..1260 => input.pitch = -0.2,
        1500..1560 => input.roll = 0.5 * side,
        1560..1620 => input.pitch = 0.4,
        _ => {}
    }
    input
}

/// The plane's side, from the roster.
fn side_of(world: &World, plane: u32) -> Side {
    world.roster.plane(PlaneId(plane)).unwrap().slot.wing.side
}

/// Order-sensitive digest, the SplitMix64 fold `tick_tests.rs` uses.
struct Digest(u64);

impl Digest {
    fn new() -> Self {
        Self(0x7365_6174_7465_7374)
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
        value.into_iter().for_each(|v| self.f64(v));
    }
    fn int(&mut self, value: impl Into<i64>) {
        self.word(value.into() as u64);
    }
    fn text(&mut self, text: &str) {
        self.word(text.len() as u64);
        text.bytes().for_each(|byte| self.word(u64::from(byte)));
    }
    fn flight(&mut self, f: &flight::State) {
        self.vector(f.position);
        self.vector(f.velocity);
        for value in [f.yaw, f.pitch, f.bank, f.speed, f.fuel, f.damage_fraction] {
            self.f64(value);
        }
        self.word(u64::from(f.crashed));
    }
}

/// What one burst came to.
#[derive(Default, Debug)]
struct Landed {
    /// Rounds of the shooter that hit something.
    hits: u32,
    /// Damage events on the victim (an ownship hurt or a row hit) in the
    /// ticks the shooter's rounds landed.
    victim_hurt: u32,
}

/// What the run saw.
#[derive(Default)]
struct Seen {
    /// (observer, observed) among the humans' planes, at any tick.
    sensed: BTreeSet<(u32, u32)>,
    bursts: [Landed; BURSTS.len()],
    radio: Vec<(usize, SeatId, comms::Call)>,
    /// Every HUD line, with its tick and the seat it was for.
    messages: Vec<(usize, SeatId, String)>,
    journal: Vec<journal::Entry>,
    /// The tick each plane was first out of hit points.
    lost: Vec<(usize, u32)>,
    /// Each wing's leader every 120th tick: (tick, friendly, enemy).
    leaders: Vec<(usize, Option<u32>, Option<u32>)>,
    /// Every AI wingman at the [`SAMPLES`] ticks.
    wingmen: Vec<Wingman>,
}

struct Run {
    digest: u64,
    parts: Vec<(usize, u64)>,
    seen: Seen,
    world: World,
}

fn feet(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f64>().sqrt()
}

fn position_of(world: &World, plane: u32) -> [f64; 3] {
    if let Some(cockpit) = world.cockpits.iter().find(|c| c.plane.0 == plane) {
        return cockpit.flight.position;
    }
    world
        .ai_wings
        .as_ref()
        .unwrap()
        .mission()
        .actor(plane)
        .unwrap()
        .flight()
        .position
}

/// One AI wingman as a sample sees it.
#[derive(Debug)]
struct Wingman {
    tick: usize,
    id: u32,
    /// The plane that leads its wing.
    leader: u32,
    /// The AI has a formation to fly on its leader.
    formating: bool,
    /// Its place in the flight behind the leader.
    slot: u8,
    /// Feet from its leader.
    feet: f64,
}

/// Records, for each living AI wingman, whether it flies on its wing's
/// current leader and how far it is from it.
fn sample_wingmen(world: &World, tick: usize, seen: &mut Seen) {
    let mission = world.ai_wings.as_ref().unwrap().mission();
    for (side, ai) in [(FRIENDLY_SIDE, F_AI), (ENEMY_SIDE, E_AI)] {
        let Some(leader) = mission.wing_leader(side, 0) else {
            continue;
        };
        for wingman in ai {
            let Some(actor) = mission.actor(wingman.0).filter(|a| a.alive()) else {
                continue;
            };
            seen.wingmen.push(Wingman {
                tick,
                id: wingman.0,
                leader,
                formating: actor.controller().formation_trace().is_some(),
                slot: actor.wing_slot(),
                feet: feet(position_of(world, wingman.0), position_of(world, leader)),
            });
        }
    }
}

fn fly(friendly_fire: FriendlyFire, ticks: usize) -> Run {
    let mut world = crowded_mission();
    world.combat.state.friendly_fire = friendly_fire;
    let mut out = TickOutput::default();
    let mut digest = Digest::new();
    let mut parts = Vec::new();
    let mut seen = Seen::default();
    let mut lost: BTreeSet<u32> = BTreeSet::new();
    for tick in 0..ticks {
        for burst in &BURSTS {
            if tick + 1 == burst.start {
                if burst.victim == F_LEAD {
                    // The last burst finds plane 0 with two hit points left.
                    world.combat.state.ownship_mut(0).unwrap().hp = 2;
                }
                aim(&mut world, burst);
            }
            if tick == burst.start + CLEAR {
                clear(&mut world, burst);
            }
        }
        let shooters: Vec<SeatId> = BURSTS
            .iter()
            .filter(|b| (b.start..b.start + HELD).contains(&tick))
            .filter_map(|b| world.roster.seat_of(b.shooter))
            .collect();
        let step_inputs = inputs(&world, |seat| SeatInput {
            pilot: pilot(seat, tick),
            trigger: shooters.contains(&seat),
            ..SeatInput::default()
        });
        world.step(&step_inputs, &mut out).unwrap();

        // What the tick did.
        let owners: Vec<u32> = out
            .outcomes
            .iter()
            .filter(|o| matches!(o.resolution, Resolution::Hit(_)))
            .map(|o| o.key.owner)
            .collect();
        let hurt: Vec<u32> = out
            .events
            .iter()
            .filter_map(|event| match event {
                Event::OwnshipDamaged { aircraft, .. } | Event::Hit(aircraft) => Some(*aircraft),
                _ => None,
            })
            .collect();
        for (n, burst) in BURSTS.iter().enumerate() {
            if (burst.start..burst.start + WATCH).contains(&tick)
                && owners.contains(&burst.shooter.0)
            {
                seen.bursts[n].hits +=
                    owners.iter().filter(|o| **o == burst.shooter.0).count() as u32;
                seen.bursts[n].victim_hurt +=
                    hurt.iter().filter(|v| **v == burst.victim.0).count() as u32;
            }
        }
        for a in HUMANS {
            let own = world.combat.state.ownship(a.0).unwrap();
            for b in HUMANS {
                if own.sensors.observation(b.0).is_some() {
                    seen.sensed.insert((a.0, b.0));
                }
            }
        }
        for (seat, call) in radio_of(&out) {
            digest.int(i64::from(seat.0));
            digest.text(&call.label);
            digest.text(&call.text);
            call.stems.iter().for_each(|stem| digest.text(stem));
            seen.radio.push((tick, seat, call));
        }
        for (seat, text) in messages_of(&out) {
            digest.int(i64::from(seat.0));
            digest.text(&text);
            seen.messages.push((tick, seat, text));
        }
        for entry in world.comms.take_journal() {
            digest.text(&entry.label);
            digest.text(&entry.text);
            digest.text(entry.outcome.name());
            digest.int(entry.call.map_or(-1, |n| n as i64));
            entry
                .heard_by
                .iter()
                .for_each(|seat| digest.int(i64::from(seat.0)));
            seen.journal.push(entry);
        }

        // Every plane, ownship, row and leader, and the tick's events.
        for cockpit in &world.cockpits {
            digest.int(i64::from(cockpit.plane.0));
            digest.flight(&cockpit.flight);
        }
        for own in world.combat.state.ownships() {
            for value in [own.aircraft, own.shots, own.hits, own.kills] {
                digest.int(i64::from(value));
            }
            for value in [own.hp, i32::from(own.chaff), i32::from(own.flares)] {
                digest.int(i64::from(value));
            }
            own.ammo.iter().for_each(|a| digest.int(i64::from(*a)));
        }
        let wings = world.ai_wings.as_ref().unwrap();
        for actor in wings.mission().actors() {
            digest.int(i64::from(actor.id()));
            digest.flight(actor.flight());
        }
        for row in &world.combat.state.targets {
            digest.int(i64::from(row.id));
            digest.vector(row.position);
            digest.int(i64::from(row.hp));
        }
        let leaders = [FRIENDLY_SIDE, ENEMY_SIDE].map(|side| wings.mission().wing_leader(side, 0));
        leaders
            .iter()
            .for_each(|leader| digest.int(leader.map_or(-1, i64::from)));
        digest.int(out.events.len() as i64);
        out.events
            .iter()
            .for_each(|event| digest.text(&format!("{event:?}")));

        if tick % 120 == 0 {
            seen.leaders.push((tick, leaders[0], leaders[1]));
        }
        if SAMPLES.contains(&tick) {
            sample_wingmen(&world, tick, &mut seen);
        }
        for id in 0..8u32 {
            let gone = match world.combat.state.ownship(id) {
                Some(own) => own.hp <= 0,
                None => world
                    .combat
                    .state
                    .targets
                    .iter()
                    .any(|t| t.id == id && t.hp <= 0),
            };
            if gone && lost.insert(id) {
                seen.lost.push((tick, id));
            }
        }
        if (tick + 1) % 120 == 0 {
            parts.push((tick + 1, digest.0));
        }
    }
    Run {
        digest: digest.0,
        parts,
        seen,
        world,
    }
}

fn whole_run() -> Run {
    fly(FriendlyFire::On, TICKS)
}

/// The run repeats exactly: two fights fold the same digest of every plane,
/// every ownship's hit points and rounds, the AI's rows and the radio journal.
#[test]
fn the_fight_repeats_exactly() {
    let (first, second) = (whole_run(), whole_run());
    let divergence = first
        .parts
        .iter()
        .zip(&second.parts)
        .find(|(a, b)| a != b)
        .map_or("the end".to_string(), |(a, _)| format!("tick {}", a.0));
    assert_eq!(
        first.digest, second.digest,
        "two identical fights differ (first difference by {divergence})"
    );
    // The digest has something to fold: the fight happened.
    assert!(first.seen.radio.len() > 10 && first.seen.journal.len() > 20);
    assert_eq!(first.world.combat.state.ownships().len(), 4);
}

/// Every human's sensors find every other human, of either side, at some
/// point of the fight.
#[test]
fn every_humans_sensors_see_the_other_humans() {
    let run = whole_run();
    for a in HUMANS {
        for b in HUMANS.into_iter().filter(|b| *b != a) {
            assert!(
                run.seen.sensed.contains(&(a.0, b.0)),
                "plane {} never sensed plane {}",
                a.0,
                b.0
            );
        }
        assert!(
            !run.seen.sensed.contains(&(a.0, a.0)),
            "plane {} senses itself",
            a.0
        );
    }
}

/// Each burst's rounds land: ownships hit ownships of either side, friendly
/// fire included, and hit AI aircraft of either side.
#[test]
fn ownships_hit_each_other_and_the_ai_rows() {
    let run = whole_run();
    for (burst, landed) in BURSTS.iter().zip(&run.seen.bursts) {
        assert!(landed.hits >= 1, "no round hit: {}", burst.what);
        assert!(
            landed.victim_hurt >= 1,
            "the victim was not hurt: {}",
            burst.what
        );
    }
    // The ownships' own count of the rounds they fired, and the ledger's of
    // the rounds that hit: each seat's are its own.
    let ledger = &run.world.combat.state.ledger;
    for (plane, rounds) in [(F_LEAD, 4), (F_HUMAN, 4), (E_LEAD, 2), (E_HUMAN, 2)] {
        assert_eq!(
            run.world.combat.state.ownship(plane.0).unwrap().shots,
            rounds
        );
        let tally = ledger.total(|k| k.owner == plane.0);
        assert_eq!((tally.launched, tally.hit), (rounds, rounds));
    }
    // The last burst shot down plane 0, the first plane of the fight to go, and
    // no other human's plane.
    let (tick, plane) = run.seen.lost[0];
    assert_eq!(plane, 0, "{:?}", run.seen.lost);
    assert!(
        run.seen
            .lost
            .iter()
            .all(|(_, id)| *id == 0 || !HUMANS.iter().any(|h| h.0 == *id)),
        "{:?}",
        run.seen.lost
    );
    let last = BURSTS.last().unwrap().start;
    assert!(
        (last + 30..last + 80).contains(&tick),
        "lost at tick {tick}"
    );
    assert!(run.world.cockpits[0].flight.crashed);
    assert!(!run.world.cockpits[1].flight.crashed);
}

/// With friendly fire off, the two bursts between aircraft of one side do no
/// harm, and the four across the sides still do.
#[test]
fn with_friendly_fire_off_a_shooters_own_side_is_spared() {
    let run = fly(FriendlyFire::Off, 1300);
    for (burst, landed) in BURSTS.iter().zip(&run.seen.bursts) {
        let same_side = side_of(&run.world, burst.shooter.0) == side_of(&run.world, burst.victim.0);
        assert_eq!(
            landed.victim_hurt == 0,
            same_side,
            "{}: {landed:?}",
            burst.what
        );
    }
    assert!(
        run.seen
            .lost
            .iter()
            .all(|(_, id)| !HUMANS.iter().any(|h| h.0 == *id)),
        "{:?}",
        run.seen.lost
    );
    let mission = run.world.ai_wings.as_ref().unwrap().mission();
    assert_eq!(
        mission.wing_leader(FRIENDLY_SIDE, 0),
        Some(0),
        "nobody shot down the lead"
    );
}

/// The AI wingmen of a human-led wing fly on their human leader while it
/// turns and pulls, and, once the friendly lead is shot down, close up on the
/// human who leads now, re-formed behind it.
#[test]
fn the_ai_wingmen_fly_on_their_human_leader() {
    let run = whole_run();
    let seen = &run.seen;
    let at = |tick| seen.wingmen.iter().filter(move |w| w.tick == tick);
    // Before the fight: every wingman, of both wings, follows a human leader.
    for tick in [140, 280] {
        assert_eq!(at(tick).count(), 4, "tick {tick}");
        for w in at(tick) {
            assert!(w.leader == 0 || w.leader == 4, "{w:?}");
            assert!(w.formating, "{w:?}");
            assert!(w.feet < TRAILING_FT, "{w:?}");
        }
    }
    // The leaders: friendly plane 0 then plane 1, enemy plane 4 throughout.
    let leaders = |tick| {
        seen.leaders
            .iter()
            .find(|l| l.0 == tick)
            .map(|l| (l.1, l.2))
    };
    assert_eq!(leaders(1080), Some((Some(0), Some(4))));
    assert_eq!(leaders(1200), Some((Some(1), Some(4))));
    assert!(seen.leaders.iter().all(|l| l.2 == Some(4)));
    // After: the friendly wingmen follow the other human and their slots close
    // up behind it; the enemy wing is as it was.
    let after: Vec<&Wingman> = at(1200).collect();
    assert_eq!(after.iter().filter(|w| w.id < 4).count(), 2, "{after:?}");
    for w in &after {
        let (leader, slot) = match w.id {
            2 => (1, 1),
            3 => (1, 2),
            6 => (4, 2),
            _ => (4, 3),
        };
        assert_eq!((w.leader, w.slot), (leader, slot), "{w:?}");
        assert!(w.formating || w.id > 3, "{w:?}");
    }
    // A wingman that a burst moved away from the flight closes on the new
    // leader over the rest of the fight.
    let far = at(1200).find(|w| w.id == 2).unwrap().feet;
    let end = feet(position_of(&run.world, 2), position_of(&run.world, 1));
    assert!(
        end < far,
        "plane 2 was {far} ft and is {end} ft from its leader"
    );
}

/// Each seat hears its own radio: a call reaches only seats on its speaker's
/// side, the seats of one flight hear one variant, and each rule that names
/// one seat reaches that seat alone.
#[test]
fn each_seat_hears_its_own_radio() {
    let run = whole_run();
    let world = &run.world;
    let seat_side =
        |seat: SeatId| side_of(world, world.roster.seat(seat).unwrap().plane.unwrap().0);
    // A radio call from an aircraft reaches a seat on that aircraft's side.
    let mut heard = [0; 2];
    for (tick, seat, call) in &run.seen.radio {
        if call.route == comms::Route::Radio
            && let Some(speaker) = call.origin.speaker
        {
            assert_eq!(
                side_of(world, speaker),
                seat_side(*seat),
                "tick {tick}: seat {} heard plane {speaker}: {call:?}",
                seat.0
            );
            heard[usize::from(seat.0 / 2)] += 1;
        }
    }
    assert!(heard.iter().all(|n| *n > 3), "both sides talk: {heard:?}");
    // The journal says the same: every seat a call was queued for or
    // delivered to is on the speaker's side.
    for entry in &run.seen.journal {
        if let (Some(speaker), Some(comms::Route::Radio)) = (entry.origin.speaker, entry.route) {
            for seat in &entry.heard_by {
                assert_eq!(side_of(world, speaker), seat_side(*seat), "{entry:?}");
            }
        }
    }
    // Both seats of a flight hear one call at the same tick with one variant.
    let mut shared = 0;
    for flight in [[SeatId(0), SeatId(1)], [SeatId(2), SeatId(3)]] {
        for (tick, _, call) in run.seen.radio.iter().filter(|r| r.1 == flight[0]) {
            let twin = run.seen.radio.iter().find(|r| {
                r.0 == *tick && r.1 == flight[1] && r.2.origin.cause == call.origin.cause
            });
            if let Some((_, _, twin)) = twin {
                assert_eq!(
                    call.stems, twin.stems,
                    "tick {tick}: one variant for the flight"
                );
                assert_eq!(call.text, twin.text);
                shared += 1;
            }
        }
    }
    assert!(shared >= 3, "only {shared} calls reached a whole flight");
    // The friendly-fire complaint goes to the shooter's seat alone.
    let complaints: Vec<SeatId> = run
        .seen
        .radio
        .iter()
        .filter(|r| matches!(r.2.origin.cause, journal::Cause::FriendlyFire { .. }))
        .map(|r| r.1)
        .collect();
    assert_eq!(complaints, [SeatId(1)]);
    // Only the seat that leads the flight hears its wingmen's fuel calls.
    let fuel: Vec<_> = run
        .seen
        .radio
        .iter()
        .filter(|r| matches!(r.2.origin.cause, journal::Cause::AiFuel { .. }))
        .collect();
    assert!(!fuel.is_empty());
    for (tick, seat, call) in fuel {
        let leading = run
            .seen
            .leaders
            .iter()
            .rev()
            .find(|l| l.0 <= *tick)
            .unwrap();
        let leader = if seat.0 < 2 { leading.1 } else { leading.2 };
        assert_eq!(
            leader,
            world.roster.seat(*seat).unwrap().plane.map(|p| p.0),
            "tick {tick}: seat {} heard a leader call: {call:?}",
            seat.0
        );
    }
    // The lead passes to seat 1 when plane 0 is shot down: the call is that
    // seat's alone, five seconds later, a HUD line because the previous
    // leader is dead.
    let leader_calls: Vec<_> = run
        .seen
        .radio
        .iter()
        .filter(|r| matches!(r.2.origin.cause, journal::Cause::Leadership { .. }))
        .collect();
    assert_eq!(leader_calls.len(), 1, "{leader_calls:?}");
    let (tick, seat, call) = leader_calls[0];
    let lost = run.seen.lost[0].0;
    assert_eq!(*seat, SeatId(1));
    assert!((lost + 600..lost + 603).contains(tick), "tick {tick}");
    assert_eq!(call.label, "Flight");
    assert!(
        call.stems.is_empty() && call.text.contains("WNGLDR"),
        "{call:?}"
    );
}

/// What a seat's debrief reads from the world, built as the app's
/// `debrief::capture` builds it for the seat's plane: every human-flown plane
/// and every AI aircraft as a fate, friendly when on the seat's side, and the
/// plane's own requirements.
fn debrief_inputs(
    world: &World,
    seat: SeatId,
) -> (u32, Vec<outcome::Aircraft>, outcome::Requirements) {
    let plane = world.roster.seat(seat).unwrap().plane.unwrap();
    let side = side_of(world, plane.0);
    let mut fates: Vec<outcome::Aircraft> = world
        .cockpits
        .iter()
        .map(|cockpit| {
            let pilot = &cockpit.flight.systems.pilot;
            let hp = world
                .combat
                .state
                .ownship(cockpit.plane.0)
                .map_or(0, |o| o.hp);
            outcome::Aircraft {
                id: cockpit.plane.0,
                friendly: side_of(world, cockpit.plane.0) == side,
                alive: !cockpit.flight.crashed && hp > 0 && !pilot.dead && !pilot.ejected,
            }
        })
        .collect();
    let wings = world.ai_wings.as_ref().unwrap();
    fates.extend(outcome::ai_aircraft(wings, side));
    let requirements = outcome::Requirements::of(
        wings,
        plane.0,
        side,
        &world.revival.objective_lineages(&world.roster),
    );
    (plane.0, fates, requirements)
}

/// Each seat's debrief inputs name its own plane: its requirements, the fates
/// friendly or hostile from its own side, and the kills, friendly fire and
/// rounds credited to it alone.
#[test]
fn each_seats_debrief_inputs_name_its_own_plane() {
    let run = whole_run();
    let world = &run.world;
    let ledger = &world.combat.state.ledger;
    for (seat, plane) in [(0u8, 0u32), (1, 1), (2, 4), (3, 5)] {
        let (id, fates, requirements) = debrief_inputs(world, SeatId(seat));
        assert_eq!(id, plane, "seat {seat}");
        let standing = outcome::Standing {
            ledger,
            plane: id,
            aircraft: &fates,
            requirements: &requirements,
        };
        let friendly = seat < 2;
        // The plane is in its own list, friendly to itself, and the sides
        // swap with the seat.
        assert!(standing.friendly(id));
        for other in HUMANS {
            assert_eq!(
                standing.friendly(other.0),
                (other.0 < 4) == friendly,
                "seat {seat} on plane {}",
                other.0
            );
        }
        // What each plane must destroy: every aircraft of the other side the
        // mission started with, the human-flown ones too (the lobby pass's
        // follow-up F1; before it only the AI's rows were known).
        let hostile: &[u32] = if friendly {
            &[4, 5, 6, 7]
        } else {
            &[0, 1, 2, 3]
        };
        assert_eq!(requirements.destroy, hostile, "seat {seat}");
        assert!(requirements.protect.is_empty());
        // Plane 0, shot down, is one of the enemy side's targets.
        assert_eq!(standing.destroyed(), u32::from(!friendly), "seat {seat}");
        // The kill of plane 0 is credited to plane 1, and no kill to any
        // other seat.
        let kills = standing.kills();
        assert!(
            kills.iter().any(|k| k.owner == 1 && k.victim == 0),
            "seat {seat}: {kills:?}"
        );
        assert!(
            kills
                .iter()
                .all(|k| k.owner == 1 || !HUMANS.iter().any(|h| h.0 == k.owner)),
            "seat {seat}: {kills:?}"
        );
        assert_eq!(standing.friendly_fire(), seat == 1, "seat {seat}");
        assert!(!standing.succeeded());
        // Its own plane's fate: only plane 0 is out.
        let own = fates.iter().find(|a| a.id == id).unwrap();
        assert_eq!(own.alive, seat != 0);
        // The ledger's totals are the seat's own rounds.
        let own_rounds = ledger.total(|k| k.owner == id);
        let expect = if seat < 2 { 4 } else { 2 };
        assert_eq!(own_rounds.launched, expect, "seat {seat}");
        assert_eq!(own_rounds.hit, expect, "seat {seat}");
    }
}

/// Each seat gets its own HUD lines: the hit line of a seat's own plane, a
/// hit or two later, and the AI wings' lines only for the seats that fly in
/// Friendly Wing 1.
#[test]
fn each_seat_gets_its_own_hud_lines() {
    let run = whole_run();
    let lines = run.seen.messages.iter();
    // A burst that hits plane 4 gives seat 2 the hit line and nobody else, and
    // the burst that hits plane 1 gives it to seat 1 alone.
    let hit_lines = |from: usize, to: usize| -> Vec<SeatId> {
        lines
            .clone()
            .filter(|m| (from..to).contains(&m.0) && m.2.starts_with("Aircraft hit"))
            .map(|m| m.1)
            .collect()
    };
    assert_eq!(
        hit_lines(BURSTS[0].start, BURSTS[0].start + WATCH),
        [SeatId(2)]
    );
    assert_eq!(
        hit_lines(BURSTS[1].start, BURSTS[1].start + WATCH),
        [SeatId(1)]
    );
    // The AI wings' lines ("Friendly 1-3: Defending") reach both seats of
    // Friendly Wing 1 on the same tick, and neither enemy seat.
    let wing: Vec<_> = lines
        .clone()
        .filter(|m| m.2.starts_with("Friendly "))
        .collect();
    assert!(wing.len() >= 4, "{wing:?}");
    assert!(wing.iter().all(|m| m.1 < SeatId(2)), "{wing:?}");
    for m in &wing {
        assert!(
            wing.iter()
                .any(|other| other.0 == m.0 && other.2 == m.2 && other.1 != m.1),
            "the other friendly seat missed {m:?}"
        );
    }
    assert!(
        lines
            .filter(|m| m.1 >= SeatId(2))
            .all(|m| m.2.starts_with("Aircraft hit")),
        "an enemy seat was shown a line that is not its own"
    );
}
