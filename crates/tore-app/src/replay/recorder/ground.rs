//! The air-to-ground round in a recording (slice RA, 2026-10-11): what a
//! pilot does with bombs, rockets, lasers and guided weapons, and what those
//! weapons do. Everything here reads state the tick already computed; no new
//! per-tick tracks are written, because a replay draws no HUD (decision 19 of
//! the air-to-ground plan). The bombs' arcs are already in the frame, one
//! pose a tick; these are the events that explain them.
//!
//! - **Releases**: `weapon.release` for each trigger call that lets go of
//!   bombs or rockets, with the pack in order, the station, the sight that
//!   was in use and the rounds left.
//! - **Landings**: `weapon.dud` for a bomb that fell before its fuze armed,
//!   `weapon.cluster` for a cluster bomb's burst.
//! - **Guided weapons**: `weapon.guided` at the release and at each change of
//!   mode (waiting, guiding, blind), naming whose laser spot a laser weapon
//!   follows and the point a JDAM stored.
//! - **Pilot**: `player.laser` when an aircraft's laser designator changes
//!   state, `player.designate` when it designates or pins a ground target.
//! - **Words**: the reasons of a guided weapon's outcome and of the ground
//!   unit or aircraft it destroyed name the weapon and, for a laser weapon,
//!   whose laser it was guided on.
//!
//! Format 3 stays: all of this is event vocabulary, free-form by design.
use super::{Shot, Tick, WeaponNote};
use std::collections::{BTreeMap, BTreeSet};
use tore_replay::{
    self as replay, Event, Frame, WeaponClass,
    vocab::{delivery, designate, field, guidance, guide, kind, laser},
};
use tore_sim::combat::{
    delivery::DeliveryMode,
    laser::{Laser, LaserState, Mask},
    live::{self, GuidedFlight, GuidedMode},
    missiles::Guidance,
    sight::Sight,
};
use tore_sim::sensors::track::Mode;

/// Ticks of a pause in the trigger after which the next rocket starts a new
/// salvo: one second.
const SALVO_GAP_TICKS: u64 = replay::TICKS_PER_SECOND;

/// The three explosion types a cluster bomb's burst draws (docs/spec, the
/// retail cluster records name land explosion 18 and the burst plays 18 to
/// 20).
const CLUSTER_EXPLOSIONS: std::ops::RangeInclusive<u8> = 18..=20;

/// How close (feet) an impact effect must lie to the round it belongs to.
/// Effects are placed at the round's own position, so this only separates
/// rounds that land in the same tick.
const IMPACT_NEAR_FT: f64 = 400.;

/// Ticks between the entries that follow a laser spot or a guided weapon's
/// aim point (FX-R): ten a second at most.
const SAMPLE_TICKS: u64 = 12;

/// How far (feet) a spot or an aim point must move from the last entry to be
/// written again.
const MOVED_FT: f64 = 5.;

/// What the recorder remembers about a guided weapon between ticks.
#[derive(Clone, Copy, Debug)]
pub(super) struct GuidedNote {
    kind: Guidance,
    mode: GuidedMode,
    owner: u32,
    spot_owner: Option<u32>,
    /// Why guidance was lost, when it was.
    blind: Option<Blind>,
    /// The point it steers for, or last steered for, as of the last tick.
    point: Option<[f64; 3]>,
    /// The last aim point written, and its tick.
    written: Option<(u64, [f64; 3])>,
}

/// How guidance was lost, for an outcome's words.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Blind {
    /// It never had any: it left with no usable spot or point.
    Never,
    /// The spot owner's laser stopped.
    LaserStopped(u32),
    /// The spot left the weapon's cone.
    Cone,
    /// The launcher, target or line was lost.
    Lost,
}

/// The words for how a guided weapon ended: the clause of its outcome and
/// the phrase after its name in a loss it caused.
pub(super) struct Guided {
    pub outcome: String,
    pub phrase: String,
}

/// An aircraft's laser as last recorded.
#[derive(Clone, Copy, Debug, PartialEq)]
struct LaserNote {
    state: LaserState,
    mask: Option<Mask>,
    manual: Option<bool>,
}

/// A ground designation as last recorded: the object (none for a pin) and
/// how it was made.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Designation {
    object: Option<u32>,
    channel: &'static str,
}

/// What the recorder remembers of the air-to-ground round between ticks.
#[derive(Default)]
pub(super) struct Watch {
    guided: BTreeMap<u32, GuidedNote>,
    lasers: BTreeMap<u32, LaserNote>,
    designations: BTreeMap<u32, Designation>,
    /// The last spot written for each lasing aircraft, and its tick.
    spots: BTreeMap<u32, (u64, [f64; 3])>,
    /// The last rocket fired from a station and the number of its salvo.
    salvos: BTreeMap<(u32, usize), (u64, i64)>,
}

/// Everything one tick's events read.
pub(super) struct Context<'a> {
    pub tick: &'a Tick<'a>,
    pub frame: &'a Frame,
    /// The weapons in flight as the last tick left them, and as this one has.
    pub old: &'a BTreeMap<u32, Shot>,
    pub new: &'a BTreeMap<u32, Shot>,
    pub notes: &'a BTreeMap<u32, WeaponNote>,
    pub infos: &'a BTreeMap<u32, replay::AircraftInfo>,
}

impl Context<'_> {
    fn who(&self, id: u32) -> String {
        super::who(self.infos, id)
    }

    fn note(&self, shot: &Shot) -> Option<&WeaponNote> {
        self.notes.get(&shot.weapon)
    }

    /// The row of a ground object by id: a surface object of the mission
    /// (a unit, a ship, a parked aircraft), never an aircraft of the flights,
    /// which may stand on a runway for a while.
    fn ground_row(&self, id: u32) -> Option<&live::Target> {
        if !super::convert::is_surface_id(id) {
            return None;
        }
        self.tick
            .combat
            .state
            .targets
            .iter()
            .find(|row| row.id == id && !row.airborne)
    }
}

/// Whether a bomb that reaches the ground on this step lands before its
/// fuze arms. `shot` is the round as the previous tick left it, and
/// `clock` the quarter-second clock of the impact step (the step that
/// removed the round). A bomb with its own motion (a guided one) counts its
/// age in ticks, quarter seconds of 30 ticks; others count the quarter-second
/// clock since the release, as combat's fuze test does.
pub(super) fn is_dud(shot: &Shot, note: &WeaponNote, clock: u16) -> bool {
    if note.class != WeaponClass::Bomb {
        return false;
    }
    if shot.motion {
        shot.age < u64::from(note.arm_quarters) * 30
    } else {
        clock.wrapping_sub(shot.launched_t) < note.arm_quarters
    }
}

/// The quarter-second clock of the step that has just run, from the combat
/// tick after it.
pub(super) fn impact_clock(state: &live::State) -> u16 {
    (state.tick().saturating_sub(1) / 30) as u16
}

/// Seconds a round flew when it landed, from its age at the previous tick
/// (one step short of the impact step), in 120 Hz ticks.
fn flown_s(shot: &Shot) -> f64 {
    (shot.age + 1) as f64 / replay::TICKS_PER_SECOND as f64
}

/// "a GBU-10", "an Mk 82": a weapon's name read aloud.
fn aloud(name: &str) -> String {
    format!("{} {name}", super::surface::article(name))
}

/// One aircraft's cockpit as the pilot events read it: an ownship, or the
/// cockpit a human left in an aircraft the AI flies now (slice FX-K). The
/// kept laser and sight step every tick and a laser weapon in flight keeps
/// guiding on the kept spot, so the recording follows them like a flown
/// cockpit's.
struct Cockpit<'a> {
    aircraft: u32,
    laser: &'a Laser,
    sight: Option<&'a Sight>,
    /// The designated object.
    designated: Option<u32>,
    /// The ownship, when a human flies; none for a kept cockpit.
    own: Option<&'a live::Ownship>,
}

impl Cockpit<'_> {
    /// The AI flies the aircraft and this is what its human left.
    fn kept(&self) -> bool {
        self.own.is_none()
    }
}

/// Every cockpit of the tick in aircraft order: the ownships, and the
/// cockpits kept in aircraft the AI flies.
fn cockpits(state: &live::State) -> Vec<Cockpit<'_>> {
    let mut all: Vec<Cockpit<'_>> = state
        .ownships()
        .iter()
        .map(|own| Cockpit {
            aircraft: own.aircraft,
            laser: &own.laser,
            sight: own.sight.as_ref(),
            designated: own.designated(),
            own: Some(own),
        })
        .collect();
    all.extend(
        state
            .kept_cockpits()
            .filter(|(aircraft, _)| state.ownship(*aircraft).is_none())
            .map(|(aircraft, kept)| Cockpit {
                aircraft,
                laser: &kept.laser,
                sight: kept.sight.as_ref(),
                designated: kept.picks.selected,
                own: None,
            }),
    );
    all.sort_by_key(|cockpit| cockpit.aircraft);
    all
}

/// The laser of an aircraft a human flies or left its cockpit in.
fn laser_of(state: &live::State, aircraft: u32) -> Option<&Laser> {
    state
        .ownship(aircraft)
        .map(|own| &own.laser)
        .or_else(|| state.kept(aircraft).map(|kept| &kept.laser))
}

impl Watch {
    /// How a guided weapon was guided at the end of its flight, from what
    /// the last tick knew of it: a clause for its outcome ("it was guided on
    /// Enemy 1-2's laser") and a phrase for the loss it caused ("guided on
    /// Enemy 1-2's laser", after the weapon's name). `None` for a weapon that
    /// was not guided.
    pub fn how_guided(&self, projectile: u32, who: impl Fn(u32) -> String) -> Option<Guided> {
        let note = self.guided.get(&projectile)?;
        let (outcome, phrase) = match (note.kind, note.mode) {
            (Guidance::Laser, GuidedMode::Guiding) => {
                let laser = match note.spot_owner {
                    Some(owner) if owner == note.owner => "the shooter's own laser".to_owned(),
                    Some(owner) => format!("{} laser", possessive(&who(owner))),
                    None => "a laser spot".to_owned(),
                };
                (
                    format!("it was guided on {laser}"),
                    format!("guided on {laser}"),
                )
            }
            (Guidance::Laser, GuidedMode::Waiting) => (
                "it was released with no laser spot and never locked one".to_owned(),
                "released with no laser spot, never locked".to_owned(),
            ),
            (Guidance::Laser, GuidedMode::Blind) => {
                let cause = match note.blind {
                    Some(Blind::LaserStopped(owner)) => {
                        format!("when {} laser went off", possessive(&who(owner)))
                    }
                    Some(Blind::Cone) => "when the spot left its cone".to_owned(),
                    _ => "at release, with no spot in reach".to_owned(),
                };
                (
                    format!("it went blind {cause}"),
                    format!("gone blind {cause}"),
                )
            }
            (Guidance::Coordinate, GuidedMode::Guiding) => (
                "it flew to the point stored at its release".to_owned(),
                "flown to the point stored at release".to_owned(),
            ),
            (Guidance::Command, GuidedMode::Guiding) => (
                "it was steered at its target".to_owned(),
                "steered at its target".to_owned(),
            ),
            (Guidance::Coordinate, _) => (
                "it fell blind: no point was inside its release zone".to_owned(),
                "dropped with no point in its release zone".to_owned(),
            ),
            (Guidance::Command, _) => (
                "it lost its launcher's guidance".to_owned(),
                "that lost its launcher's guidance".to_owned(),
            ),
            _ => return None,
        };
        Some(Guided { outcome, phrase })
    }

    /// The events of the tick, appended to `events`.
    pub fn events(&mut self, cx: &Context<'_>, events: &mut Vec<Event>) {
        let before = events.len();
        self.releases(cx, events);
        self.landings(cx, events);
        self.name_the_cause(cx, &mut events[..before]);
        self.guided(cx, events);
        self.lasers(cx, events);
        self.laser_spots(cx, events);
        self.designations(cx, events);
        self.gone(cx, events);
    }

    /// An aircraft that has no cockpit any more, a human's or a kept one (it
    /// was destroyed or left the mission while its laser was on or its target
    /// designated), ends its laser and designation in the recording, as its
    /// weapons lose the spot (slice FX-K).
    fn gone(&mut self, cx: &Context<'_>, events: &mut Vec<Event>) {
        let present: BTreeSet<u32> = cockpits(&cx.tick.combat.state)
            .iter()
            .map(|cockpit| cockpit.aircraft)
            .collect();
        let gone: BTreeSet<u32> = self
            .lasers
            .keys()
            .chain(self.designations.keys())
            .chain(self.spots.keys())
            .copied()
            .filter(|aircraft| !present.contains(aircraft))
            .collect();
        for aircraft in gone {
            self.spots.remove(&aircraft);
            if let Some(note) = self.lasers.remove(&aircraft)
                && note.state != LaserState::Off
            {
                events.push(
                    Event::new(kind::PLAYER_LASER)
                        .with_subject(aircraft)
                        .with(field::STATE, laser::OFF)
                        .with(field::ON, false)
                        .with(field::HOW, "automatic")
                        .with(field::REASON, "its aircraft is gone"),
                );
            }
            if let Some(before) = self.designations.remove(&aircraft) {
                let mut event = Event::new(kind::PLAYER_DESIGNATE)
                    .with_subject(aircraft)
                    .with(field::CHANNEL, designate::CLEARED);
                if let Some(from) = before.object {
                    event = event.with(field::FROM, replay::Value::Id(from));
                }
                events.push(event.with(field::REASON, "its aircraft is gone"));
            }
        }
    }

    /// One `weapon.release` for each trigger call that let go of bombs or
    /// rockets: the new rounds of a tick, grouped by shooter and station.
    fn releases(&mut self, cx: &Context<'_>, events: &mut Vec<Event>) {
        type Call<'a> = Vec<(u32, &'a Shot, WeaponClass)>;
        let mut calls: BTreeMap<(u32, usize), Call<'_>> = BTreeMap::new();
        for (id, shot) in cx.new {
            if cx.old.contains_key(id) {
                continue;
            }
            let Some(class) = cx
                .note(shot)
                .map(|n| n.class)
                .filter(|class| matches!(class, WeaponClass::Bomb | WeaponClass::Rocket))
            else {
                continue;
            };
            calls
                .entry((shot.owner, shot.station))
                .or_default()
                .push((*id, shot, class));
        }
        let tick = cx.frame.tick;
        for ((owner, station), rounds) in calls {
            let (first, shot, class) = rounds[0];
            let ship = cx.tick.combat.state.ownship(owner);
            let guided = cx.tick.combat.state.guided_flight(first).is_some();
            let how = if class == WeaponClass::Rocket {
                delivery::ROCKET
            } else if guided {
                delivery::GUIDED
            } else {
                match ship.map(|own| own.delivery_mode) {
                    Some(DeliveryMode::Ccip) => delivery::CCIP,
                    Some(DeliveryMode::Level) => delivery::LEVEL,
                    None => delivery::UNGUIDED,
                }
            };
            let mut event = Event::new(kind::WEAPON_RELEASE)
                .with_subject(owner)
                .with(field::WEAPON, replay::Value::Id(shot.weapon))
                .with(field::CLASS, class.name())
                .with(field::STATION, station as i64)
                .with(field::COUNT, rounds.len() as i64)
                .with(field::PROJECTILE, replay::Value::Id(first))
                .with(
                    field::PROJECTILES,
                    rounds.iter().map(|(id, ..)| *id).collect::<Vec<u32>>(),
                )
                .with(field::DELIVERY, how);
            if let Some(own) = ship {
                event = event.with(field::LEFT, i64::from(own.rounds(station)));
                if let Some(row) = own.designated().and_then(|id| cx.ground_row(id)) {
                    event = event.with_object(row.id).with(
                        field::RANGE_FT,
                        super::distance(shot.position, row.position),
                    );
                }
            }
            if class == WeaponClass::Rocket {
                let salvo = match self.salvos.get(&(owner, station)) {
                    Some((last, number)) if tick.saturating_sub(*last) <= SALVO_GAP_TICKS => {
                        number + 1
                    }
                    _ => 1,
                };
                self.salvos.insert((owner, station), (tick, salvo));
                event = event.with(field::SALVO, salvo);
            }
            events.push(event);
        }
    }

    /// Duds and cluster bursts: the round left the picture on a step that
    /// found the ground.
    fn landings(&mut self, cx: &Context<'_>, events: &mut Vec<Event>) {
        let clock = impact_clock(&cx.tick.combat.state);
        let missed: BTreeSet<u32> = cx
            .tick
            .outcomes
            .iter()
            .filter(|o| o.resolution == tore_sim::combat::ledger::Resolution::Missed)
            .map(|o| o.projectile)
            .collect();
        // The ground bursts the tick drew, each matched once to the round
        // that vanished nearest it.
        let mut bursts: Vec<([f64; 3], u8)> = cx
            .frame
            .new_effects
            .iter()
            .filter_map(|e| match e.kind {
                replay::EffectKind::Blast {
                    on: replay::Strike::Ground,
                    explosion,
                } => Some((e.position, explosion)),
                _ => None,
            })
            .collect();
        for (id, shot) in cx.old {
            if cx.new.contains_key(id) {
                continue;
            }
            let Some(note) = cx.note(shot) else { continue };
            if note.class != WeaponClass::Bomb {
                continue;
            }
            let nearest = bursts
                .iter()
                .enumerate()
                .map(|(i, (at, explosion))| (i, super::distance(shot.position, *at), *explosion))
                .filter(|(_, away, _)| *away <= IMPACT_NEAR_FT)
                .min_by(|a, b| a.1.total_cmp(&b.1));
            let at = nearest.map_or(shot.position, |(i, ..)| bursts[i].0);
            events.push(self.impact(cx, *id, shot, at));
            if !missed.contains(id) {
                continue;
            }
            if is_dud(shot, note, clock) {
                if let Some((i, ..)) = nearest {
                    bursts.remove(i);
                }
                let mut event = Event::new(kind::WEAPON_DUD)
                    .with_subject(shot.owner)
                    .with(field::PROJECTILE, replay::Value::Id(*id))
                    .with(field::WEAPON, replay::Value::Id(shot.weapon));
                for (name, value) in field::POSITION.into_iter().zip(at) {
                    event = event.with(name, value);
                }
                events.push(
                    event
                        .with(field::ARM_S, f64::from(note.arm_quarters) / 4.)
                        .with(field::FLOWN_S, super::trees::round(flown_s(shot), 2))
                        .with(
                            field::REASON,
                            format!(
                                "it landed {:.1} s after release, before its {:.2} s fuze armed",
                                flown_s(shot),
                                f64::from(note.arm_quarters) / 4.
                            ),
                        ),
                );
            } else if let Some((i, _, explosion)) = nearest
                && CLUSTER_EXPLOSIONS.contains(&explosion)
                && note.cluster
            {
                bursts.remove(i);
                let mut event = Event::new(kind::WEAPON_CLUSTER)
                    .with_subject(shot.owner)
                    .with(field::PROJECTILE, replay::Value::Id(*id))
                    .with(field::WEAPON, replay::Value::Id(shot.weapon));
                for (name, value) in field::POSITION.into_iter().zip(at) {
                    event = event.with(name, value);
                }
                events.push(event.with(field::EXPLOSION, i64::from(explosion)));
            }
        }
    }

    /// `weapon.impact`: where a bomb or guided weapon landed, how far from
    /// the point it steered for and from the target its shooter had
    /// designated (FX-R). The replay draws it, so a failed drop shows where
    /// the bomb went and what it was following.
    fn impact(&self, cx: &Context<'_>, id: u32, shot: &Shot, at: [f64; 3]) -> Event {
        let note = self.guided.get(&id);
        let mut event = Event::new(kind::WEAPON_IMPACT)
            .with_subject(shot.owner)
            .with(field::PROJECTILE, replay::Value::Id(id))
            .with(field::WEAPON, replay::Value::Id(shot.weapon))
            .with(field::GUIDED, note.is_some());
        for (name, value) in field::POSITION.into_iter().zip(at) {
            event = event.with(name, value);
        }
        // The shooter's designation, or the one its human left when the AI
        // flies the aircraft now.
        let state = &cx.tick.combat.state;
        let target = state
            .ownship(shot.owner)
            .and_then(|own| own.designated())
            .or_else(|| state.kept(shot.owner).and_then(|kept| kept.picks.selected))
            .and_then(|target| cx.ground_row(target));
        let target_ft = target.map(|row| super::trees::round(super::distance(at, row.position), 1));
        if let Some(row) = target {
            event = event.with_object(row.id);
        }
        if let Some(feet) = target_ft {
            event = event.with(field::TARGET_FT, feet);
        }
        let mut miss_ft = None;
        if let Some(note) = note {
            event = event.with(field::MODE, mode_name(note.mode));
            if let Some(point) = note.point {
                for (name, value) in field::AIM.into_iter().zip(point) {
                    event = event.with(name, value);
                }
                let miss = super::trees::round(super::distance(at, point), 1);
                event = event.with(field::MISS_FT, miss);
                miss_ft = Some(miss);
            }
        }
        let designated = target_ft
            .map(|feet| {
                format!(
                    ", {} ft from the designated target",
                    super::trees::thousands(feet)
                )
            })
            .unwrap_or_default();
        let reason = match (note, miss_ft) {
            (Some(note), Some(miss)) => {
                let steering = if note.mode == GuidedMode::Blind {
                    "the point it was steering for before it went blind"
                } else {
                    "the point it was steering for"
                };
                format!(
                    "it landed {} ft from {steering}{designated}",
                    super::trees::thousands(miss)
                )
            }
            (Some(_), None) => format!("it fell with no point to steer for{designated}"),
            (None, _) if target_ft.is_some() => {
                format!("it fell unguided{designated}")
            }
            (None, _) => "it fell unguided".to_owned(),
        };
        event.with(field::REASON, reason)
    }

    /// Adds the weapon and the words to a ground unit's or aircraft's loss
    /// that an air-to-ground round of its killer caused this tick. `events`
    /// are the events of the tick so far, which hold its wrecks and kills.
    fn name_the_cause(&self, cx: &Context<'_>, events: &mut [Event]) {
        for event in events.iter_mut() {
            if event.kind != kind::SURFACE_WRECK && event.kind != kind::COMBAT_DESTROYED {
                continue;
            }
            let (Some(killer), Some(victim)) = (event.object, event.subject) else {
                continue;
            };
            // The killer's air-to-ground rounds that ended this tick, the
            // one nearest the victim first.
            let at = cx
                .tick
                .combat
                .state
                .targets
                .iter()
                .find(|row| row.id == victim)
                .map(|row| row.position);
            let Some((id, shot, note)) = cx
                .old
                .iter()
                .filter(|(id, shot)| shot.owner == killer && !cx.new.contains_key(id))
                .filter_map(|(id, shot)| {
                    cx.note(shot)
                        .filter(|note| note.air_to_ground)
                        .map(|note| (id, shot, note))
                })
                .min_by(|a, b| {
                    let away = |s: &Shot| at.map_or(0., |at| super::distance(s.position, at));
                    away(a.1).total_cmp(&away(b.1))
                })
            else {
                continue;
            };
            if event.get(field::WEAPON).is_none() {
                event
                    .fields
                    .push((field::WEAPON.to_owned(), replay::Value::Id(shot.weapon)));
                event
                    .fields
                    .push((field::PROJECTILE.to_owned(), replay::Value::Id(*id)));
            }
            if event.get(field::REASON).is_none() {
                let weapon = aloud(&note.name);
                let reason = match self.how_guided(*id, |who| cx.who(who)) {
                    Some(how) => format!("it was hit by {weapon} {}", how.phrase),
                    None => format!("it was hit by {weapon}"),
                };
                event.fields.push((field::REASON.to_owned(), reason.into()));
            }
        }
    }

    /// `weapon.guided`: the release and each change of mode.
    fn guided(&mut self, cx: &Context<'_>, events: &mut Vec<Event>) {
        let state = &cx.tick.combat.state;
        let mut seen = BTreeSet::new();
        for (id, flight) in state.guided_flights() {
            seen.insert(id);
            let before = self.guided.get(&id).copied();
            if before.is_some_and(|note| note.mode == flight.mode) {
                self.keep(cx, id, flight, before, events);
                continue;
            }
            let weapon = cx.new.get(&id).map(|shot| shot.weapon);
            let blind =
                (flight.mode == GuidedMode::Blind).then(|| why_blind(state, flight, before));
            let mut event = Event::new(kind::WEAPON_GUIDED)
                .with_subject(flight.owner)
                .with(field::PROJECTILE, replay::Value::Id(id))
                .with(field::GUIDANCE, guidance_name(flight.kind))
                .with(field::MODE, mode_name(flight.mode))
                .with(field::RELEASED, before.is_none());
            if let Some(weapon) = weapon {
                event = event.with(field::WEAPON, replay::Value::Id(weapon));
            }
            if let Some(before) = before {
                event = event.with(field::FROM, mode_name(before.mode));
            }
            if let Some(target) = flight.target {
                event = event.with_object(target);
            }
            if let Some(owner) = flight.spot_owner {
                event = event
                    .with(field::SPOT_OWNER, replay::Value::Id(owner))
                    .with(field::OWN, owner == flight.owner);
            }
            if let Some(point) = flight.point
                && flight.mode == GuidedMode::Guiding
            {
                for (name, value) in field::POSITION.into_iter().zip(point) {
                    event = event.with(name, value);
                }
                event = event.with(
                    field::STORED,
                    before.is_none() && flight.kind == Guidance::Coordinate,
                );
            }
            events.push(event.with(
                field::REASON,
                mode_reason(cx, flight, before.is_none(), blind),
            ));
            self.guided.insert(
                id,
                GuidedNote {
                    kind: flight.kind,
                    mode: flight.mode,
                    owner: flight.owner,
                    spot_owner: flight.spot_owner,
                    blind,
                    point: flight.point,
                    // The event above wrote the point when it was guiding.
                    written: flight
                        .point
                        .filter(|_| flight.mode == GuidedMode::Guiding)
                        .map(|point| (cx.frame.tick, point)),
                },
            );
        }
        // Weapons that are gone keep their note for the tick that ends
        // them (the outcomes were written before this ran), then go.
        self.guided.retain(|id, _| seen.contains(id));
    }

    /// Keeps the note of a weapon whose mode did not change, following the
    /// spot owner it may have moved to and the point it steers for, and
    /// writes `weapon.aim` when a guiding weapon's point has moved (FX-R).
    fn keep(
        &mut self,
        cx: &Context<'_>,
        id: u32,
        flight: &GuidedFlight,
        before: Option<GuidedNote>,
        events: &mut Vec<Event>,
    ) {
        let Some(note) = before else { return };
        let tick = cx.frame.tick;
        let mut written = note.written;
        if flight.mode == GuidedMode::Guiding
            && let Some(point) = flight.point
            && moved(written, tick, point)
        {
            written = Some((tick, point));
            let mut event = Event::new(kind::WEAPON_AIM)
                .with_subject(flight.owner)
                .with(field::PROJECTILE, replay::Value::Id(id));
            if let Some(owner) = flight.spot_owner {
                event = event.with(field::SPOT_OWNER, replay::Value::Id(owner));
            }
            for (name, value) in field::POSITION.into_iter().zip(point) {
                event = event.with(name, value);
            }
            events.push(event);
        }
        self.guided.insert(
            id,
            GuidedNote {
                spot_owner: flight.spot_owner.or(note.spot_owner),
                point: flight.point.or(note.point),
                written,
                ..note
            },
        );
    }

    /// `player.laser`: each aircraft's laser designator when its state, mask
    /// or manual setting changes.
    fn lasers(&mut self, cx: &Context<'_>, events: &mut Vec<Event>) {
        for own in cockpits(&cx.tick.combat.state) {
            let now = LaserNote {
                state: own.laser.state,
                mask: own.laser.mask,
                manual: own.laser.manual,
            };
            let before = self.lasers.insert(own.aircraft, now);
            let was = before.map_or(LaserState::Off, |note| note.state);
            // Only the state and the mask are news; the manual setting
            // decides the state and is worded with it.
            if now.state == was && before.is_none_or(|note| note.mask == now.mask) {
                continue;
            }
            if before.is_none() && now.state == LaserState::Off {
                continue;
            }
            let mut event = Event::new(kind::PLAYER_LASER)
                .with_subject(own.aircraft)
                .with(field::STATE, laser_state(now.state))
                .with(field::ON, now.state == LaserState::Lasing)
                .with(
                    field::HOW,
                    match now.manual {
                        Some(_) => "by hand",
                        None => "automatic",
                    },
                );
            if let Some(mask) = now.mask.filter(|_| now.state == LaserState::Masked) {
                event = event.with(
                    field::MASK,
                    match mask {
                        Mask::Range => "range",
                        Mask::Gimbal => "gimbal",
                        Mask::Terrain => "terrain",
                    },
                );
            }
            if let Some(target) = own.laser.target {
                event = event.with_object(target);
            }
            event = event.with(field::RANGE_FT, super::trees::round(own.laser.range_ft, 1));
            if let (LaserState::Lasing, Some(point)) = (now.state, own.laser.point) {
                for (name, value) in field::POSITION.into_iter().zip(point) {
                    event = event.with(name, value);
                }
                self.spots.insert(own.aircraft, (cx.frame.tick, point));
            }
            if own.kept() {
                event = event.with(field::KEPT, true);
            }
            events.push(event);
        }
    }

    /// `player.laser_spot`: where a lasing aircraft's spot is, when it has
    /// moved since the last entry (FX-R). The state events say when a laser
    /// comes on, goes off or is masked; these let the replay follow the spot
    /// between them.
    fn laser_spots(&mut self, cx: &Context<'_>, events: &mut Vec<Event>) {
        let tick = cx.frame.tick;
        for own in cockpits(&cx.tick.combat.state) {
            let spot = own
                .laser
                .point
                .filter(|_| own.laser.state == LaserState::Lasing);
            let Some(point) = spot else {
                self.spots.remove(&own.aircraft);
                continue;
            };
            if !moved(self.spots.get(&own.aircraft).copied(), tick, point) {
                continue;
            }
            self.spots.insert(own.aircraft, (tick, point));
            let mut event = Event::new(kind::PLAYER_LASER_SPOT).with_subject(own.aircraft);
            if let Some(target) = own.laser.target {
                event = event.with_object(target);
            }
            for (name, value) in field::POSITION.into_iter().zip(point) {
                event = event.with(name, value);
            }
            if own.kept() {
                event = event.with(field::KEPT, true);
            }
            events.push(event);
        }
    }

    /// `player.designate`: an aircraft's ground designation or pin, when it
    /// changes. An air target never makes an entry.
    fn designations(&mut self, cx: &Context<'_>, events: &mut Vec<Event>) {
        for own in cockpits(&cx.tick.combat.state) {
            let tracked = own.sight.and_then(|sight| sight.target());
            let object = own
                .designated
                .filter(|id| cx.ground_row(*id).is_some())
                .or(tracked.filter(|id| cx.ground_row(*id).is_some()));
            let held = own.sight.and_then(|sight| sight.held_point());
            let pin = held.filter(|_| object.is_none() && tracked.is_none());
            let before = self.designations.get(&own.aircraft).copied();
            let now = match (object, pin) {
                (Some(id), _) => Some(Designation {
                    object: Some(id),
                    channel: match own.own {
                        Some(ship) => match ship.sensors.ground_page() {
                            Some(Mode::AirGround) if ship.ground_track(id) => designate::RADAR,
                            Some(Mode::Infrared) => designate::INFRARED,
                            Some(Mode::Harm) => designate::HARM,
                            _ if tracked == Some(id) => designate::SIGHT,
                            _ => designate::VISUAL,
                        },
                        // The AI has no pages to say how: the designation
                        // the human made stays as it was written.
                        None => match before {
                            Some(was) if was.object == Some(id) => was.channel,
                            _ if tracked == Some(id) => designate::SIGHT,
                            _ => designate::VISUAL,
                        },
                    },
                }),
                (None, Some(_)) => Some(Designation {
                    object: None,
                    channel: designate::PIN,
                }),
                (None, None) => None,
            };
            // A pin is news when it appears or moves a long way; its point
            // drifts a little as the sight follows it.
            if now == before {
                continue;
            }
            match now {
                Some(now) => self.designations.insert(own.aircraft, now),
                None => self.designations.remove(&own.aircraft),
            };
            let mut event = Event::new(kind::PLAYER_DESIGNATE)
                .with_subject(own.aircraft)
                .with(
                    field::CHANNEL,
                    now.map_or(designate::CLEARED, |designation| designation.channel),
                );
            if let Some(id) = now.and_then(|d| d.object) {
                event = event.with_object(id);
            }
            if let Some(from) = before.and_then(|d| d.object) {
                event = event.with(field::FROM, replay::Value::Id(from));
            }
            if let (Some(Designation { object: None, .. }), Some(point)) = (now, held) {
                for (name, value) in field::POSITION.into_iter().zip(point) {
                    event = event.with(name, value);
                }
            }
            if own.kept() {
                event = event.with(field::KEPT, true);
            }
            events.push(event);
        }
    }
}

/// Whether a point is worth a new entry: nothing was written yet, or it is
/// at least [`SAMPLE_TICKS`] after the last entry and more than [`MOVED_FT`]
/// from it.
fn moved(last: Option<(u64, [f64; 3])>, tick: u64, point: [f64; 3]) -> bool {
    last.is_none_or(|(at, was)| {
        tick.saturating_sub(at) >= SAMPLE_TICKS && super::distance(was, point) > MOVED_FT
    })
}

/// "Enemy 1-2's", and "your" for the player's `You`.
fn possessive(name: &str) -> String {
    if name == "You" {
        "your".to_owned()
    } else {
        format!("{name}'s")
    }
}

fn guidance_name(kind: Guidance) -> &'static str {
    match kind {
        Guidance::Laser => guidance::LASER,
        Guidance::Coordinate => guidance::COORDINATE,
        _ => guidance::COMMAND,
    }
}

fn mode_name(mode: GuidedMode) -> &'static str {
    match mode {
        GuidedMode::Waiting => guide::WAITING,
        GuidedMode::Guiding => guide::GUIDING,
        GuidedMode::Blind => guide::BLIND,
    }
}

fn laser_state(state: LaserState) -> &'static str {
    match state {
        LaserState::Off => laser::OFF,
        LaserState::Ready => laser::READY,
        LaserState::Lasing => laser::LASING,
        LaserState::Masked => laser::MASKED,
    }
}

/// Why a guiding weapon went blind this tick, from the spot owner's laser.
fn why_blind(state: &live::State, flight: &GuidedFlight, before: Option<GuidedNote>) -> Blind {
    match (flight.kind, before) {
        (_, None) => Blind::Never,
        (Guidance::Laser, Some(note)) => {
            let owner = note.spot_owner.or(flight.spot_owner);
            match owner {
                Some(owner)
                    if laser_of(state, owner).is_none_or(|laser| {
                        !matches!(laser.state, LaserState::Lasing | LaserState::Masked)
                    }) =>
                {
                    Blind::LaserStopped(owner)
                }
                _ => Blind::Cone,
            }
        }
        _ => Blind::Lost,
    }
}

/// The words for a guided weapon's release or change of mode.
fn mode_reason(
    cx: &Context<'_>,
    flight: &GuidedFlight,
    released: bool,
    blind: Option<Blind>,
) -> String {
    let own = flight.spot_owner == Some(flight.owner);
    let spot = flight
        .spot_owner
        .map(|owner| cx.who(owner))
        .unwrap_or_default();
    match (flight.kind, flight.mode, released) {
        (Guidance::Laser, GuidedMode::Guiding, true) if own => {
            "it follows the shooter's own laser spot".to_owned()
        }
        (Guidance::Laser, GuidedMode::Guiding, true) => {
            format!("it follows {} laser spot (buddy lasing)", possessive(&spot))
        }
        (Guidance::Laser, GuidedMode::Guiding, false) => {
            format!("it locked {} laser spot", possessive(&spot))
        }
        (Guidance::Laser, GuidedMode::Waiting, _) => {
            "no laser spot yet: it locks the first spot inside its nose cone".to_owned()
        }
        (Guidance::Laser, GuidedMode::Blind, true) => {
            "every laser spot was out of its reach at release".to_owned()
        }
        (Guidance::Coordinate, GuidedMode::Guiding, true) => {
            "it flies to the point stored at release".to_owned()
        }
        (Guidance::Coordinate, GuidedMode::Blind, true) => {
            "no point lay inside its release zone".to_owned()
        }
        (Guidance::Command, GuidedMode::Guiding, true) => {
            "its launcher steers it at the designated object".to_owned()
        }
        (Guidance::Command, GuidedMode::Blind, true) => {
            "no designated object lay inside its launcher's cone with a clear line".to_owned()
        }
        (_, GuidedMode::Blind, false) => match blind {
            Some(Blind::LaserStopped(owner)) => {
                format!("{} laser went off", possessive(&cx.who(owner)))
            }
            Some(Blind::Cone) => "the laser spot left its 45 degree cone".to_owned(),
            _ => "its launcher, its target or the line to it was lost".to_owned(),
        },
        _ => "guidance changed".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bomb(arm_quarters: u16) -> WeaponNote {
        WeaponNote {
            name: "GBU-10".into(),
            class: WeaponClass::Bomb,
            arm_quarters,
            cluster: false,
            air_to_ground: true,
        }
    }

    #[test]
    fn a_bomb_is_a_dud_until_its_fuze_arms() {
        let note = bomb(4);
        // Released on quarter second 10; the clock must reach 14.
        let free_fall = Shot {
            launched_t: 10,
            ..Shot::default()
        };
        assert!(is_dud(&free_fall, &note, 13));
        assert!(!is_dud(&free_fall, &note, 14));
        // The clock wraps like combat's 16-bit one.
        let wrapped = Shot {
            launched_t: u16::MAX - 1,
            ..Shot::default()
        };
        assert!(is_dud(&wrapped, &note, 0));
        assert!(!is_dud(&wrapped, &note, 2));
        // A guided bomb counts ticks of age, 30 to the quarter second.
        let guided = |age| Shot {
            motion: true,
            age,
            ..Shot::default()
        };
        assert!(is_dud(&guided(119), &note, 0));
        assert!(!is_dud(&guided(120), &note, 0));
        // Only bombs fuse this way.
        let rocket = WeaponNote {
            class: WeaponClass::Rocket,
            ..bomb(4)
        };
        assert!(!is_dud(&free_fall, &rocket, 10));
    }

    #[test]
    fn a_point_is_written_again_when_it_moved_far_enough_and_long_enough_ago() {
        let here = [100., 0., 200.];
        // Nothing written yet.
        assert!(moved(None, 0, here));
        // Moved but too soon, then late but not moved, then both.
        assert!(!moved(Some((0, here)), 11, [200., 0., 200.]));
        assert!(!moved(Some((0, here)), 120, [103., 0., 204.]));
        assert!(moved(Some((0, here)), 12, [200., 0., 200.]));
        // Just over five feet counts; exactly five does not.
        assert!(!moved(Some((0, here)), 12, [105., 0., 200.]));
        assert!(moved(Some((0, here)), 12, [105.1, 0., 200.]));
    }

    #[test]
    fn a_guided_weapon_names_whose_laser_it_followed() {
        let mut watch = Watch::default();
        let note = |mode, spot_owner, blind| GuidedNote {
            kind: Guidance::Laser,
            mode,
            owner: 0,
            spot_owner,
            blind,
            point: None,
            written: None,
        };
        let who = |id: u32| {
            if id == 0 {
                "You".into()
            } else {
                format!("Enemy 1-{id}")
            }
        };
        watch
            .guided
            .insert(1, note(GuidedMode::Guiding, Some(0), None));
        watch
            .guided
            .insert(2, note(GuidedMode::Guiding, Some(2), None));
        watch.guided.insert(
            3,
            note(GuidedMode::Blind, Some(0), Some(Blind::LaserStopped(0))),
        );
        watch
            .guided
            .insert(4, note(GuidedMode::Blind, None, Some(Blind::Never)));
        let words = |id| watch.how_guided(id, who).expect("a guided weapon");
        assert_eq!(words(1).outcome, "it was guided on the shooter's own laser");
        // The example of the plan: a loss that names the weapon and whose
        // laser it rode.
        assert_eq!(words(2).phrase, "guided on Enemy 1-2's laser");
        assert_eq!(words(3).outcome, "it went blind when your laser went off");
        assert_eq!(
            words(4).outcome,
            "it went blind at release, with no spot in reach"
        );
        assert!(watch.how_guided(9, who).is_none(), "an unguided round");
        assert_eq!(aloud("GBU-10"), "a GBU-10");
        assert_eq!(aloud("AGM-88"), "an AGM-88");
        assert_eq!(possessive("You"), "your");
        assert_eq!(possessive("Enemy 1-2"), "Enemy 1-2's");
    }

    /// A laser weapon whose shooter was given back to the AI goes blind
    /// because of the kept laser, not because "the laser went off" when the
    /// ownship left (slice FX-K).
    #[test]
    fn a_weapon_of_a_given_back_aircraft_reads_the_kept_laser_for_why_it_went_blind() {
        use crate::combat_view::render_hash_tests as fixture;
        use tore_sim::combat::live::{DEFAULT_OWNSHIP_SIDE, Ownship};
        let combat = fixture::combat(fixture::models(), Vec::new());
        let config = combat.state.own().configuration().clone();
        let mut state = live::State::without_ownships();
        let mut ship = Ownship::new(7, DEFAULT_OWNSHIP_SIDE, config, false).unwrap();
        ship.laser.state = LaserState::Lasing;
        ship.laser.point = Some([0., 0., 0.]);
        state.keep(7, ship.kept());
        let flight = GuidedFlight {
            kind: Guidance::Laser,
            mode: GuidedMode::Blind,
            owner: 7,
            station: 0,
            spot_owner: Some(7),
            point: None,
            target: None,
            fall: None,
            seconds: None,
        };
        let before = Some(GuidedNote {
            kind: Guidance::Laser,
            mode: GuidedMode::Guiding,
            owner: 7,
            spot_owner: Some(7),
            blind: None,
            point: None,
            written: None,
        });
        assert_eq!(why_blind(&state, &flight, before), Blind::Cone);
        let mut kept = state.take_kept(7).unwrap();
        kept.laser.state = LaserState::Off;
        state.keep(7, kept);
        assert_eq!(why_blind(&state, &flight, before), Blind::LaserStopped(7));
        assert!(laser_of(&state, 7).is_some() && laser_of(&state, 8).is_none());
    }
}
