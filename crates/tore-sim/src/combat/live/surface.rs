//! Shots fired by surface units, ground units and ships: SAMs, AAA and flak
//! (docs/spec/surface-defenses.md, "SAM missiles" and "AAA and flak").
//!
//! A surface unit is a ground target row in [`State`] with its own id. Its
//! controller (slice W3, `ai::surface`) decides when and where to fire; this
//! module turns one shot into a projectile with [`State::fire_surface`] and
//! keeps what the shot needs beyond an aircraft's round:
//!
//! - **Ids** come from their own counter, from
//!   [`SURFACE_PROJECTILE_ID_BASE`] up, so they never meet the ownships'
//!   (from 0) or the AI aircraft's (from `1 << 24`).
//! - **The weapon record travels with the round** (`Projectile::weapon` is
//!   always set), so nothing looks for the owner among the ownships.
//! - **Gun rounds** are numbered within their game round
//!   (`ordinal % actualRoundsPerGame`) so `projectile_damage` splits a
//!   game round's damage over its physical rounds (the AAA tuning table's
//!   damage rule), and carry tracers as the table says.
//! - **An end tick**, set at launch: a flak shell's time fuze, where it
//!   bursts; any other round's range limit, where it goes away unseen, so a
//!   fast gun's rounds stop at the target's range instead of flying their
//!   whole life (fitted). Each live round's [`SurfaceRound`] is kept by its
//!   projectile id, as the rewinds are.
//! - **Capacity**: a surface shot is refused while fewer than
//!   [`SURFACE_PROJECTILE_RESERVE`] projectile slots would stay free, so AAA
//!   can never use up the room the aircraft's own weapons need (fitted).
//!
//! In flight a surface round differs from an aircraft's in a few fitted ways
//! (`State::step` applies them; the spec has the rules):
//!
//! - It hits aircraft and the terrain only, never a ground object or ship:
//!   surface units do not fight each other this round, and a launcher is never
//!   hit by its own missile on the rail.
//! - A surface gun round does the record's damage, never the aircraft guns'
//!   one-third rule, and never a critical (pilot or central) kill: the
//!   tuning table already matches retail damage per second. A jammer does
//!   not defeat individual gun rounds.
//! - A flak shell bursts at its time fuze, at the end of its life, or as soon
//!   as it passes within its record's fuze radius of a hostile aircraft
//!   in flight. The burst is an [`EffectKind::Flak`] effect with the record's
//!   own explosion type.
//! - A surface missile or flak burst does collateral damage: every aircraft
//!   within the record's collateral radius, other than one the round struck
//!   directly, takes the record's damage times its collateral percent, once.
//!   Friendly fire off spares the shooter's side, as for direct hits.
//!   Aircraft weapons keep today's rule (no collateral damage).
//!
//! The explosion a destroyed ground object shows comes from its unit's record
//! when the host gives one ([`State::set_ground_look`]).
use super::{
    DamageSection, EffectKind, HitRecord, LocalizedDamage, MAX_HIT_RECORDS, MAX_PROJECTILES,
    NO_SIDE, OwnRow, Projectile, Side, State, Strike, damage_class, draw, is_gun, sub,
};
use crate::attitude::{Vector, dot};
use crate::combat::gun_round::{self, service_ticks};
use crate::combat::ledger::{Kill, Resolution};
use crate::combat::live::Event;
use crate::combat::missiles::{self, Flight, LaunchMode, Motion, Rules, TargetRole, seeker};
use crate::combat::{FallState, axial_speed, commanded_speed, engine_phase, launch_speed};
use tore_formats::weapons::Weapon;

/// The first projectile id a surface shot takes (plan 2.5). AI aircraft
/// rounds count from `1 << 24`, ownships' from 0.
pub const SURFACE_PROJECTILE_ID_BASE: u32 = 0x0200_0000;
/// The last id before the theater objects' range (`0x4000_0000`); the counter
/// wraps back to [`SURFACE_PROJECTILE_ID_BASE`] past it, about a billion shots.
const SURFACE_PROJECTILE_ID_LAST: u32 = 0x3FFF_FFFF;
/// Projectile slots a surface shot always leaves free for the aircraft's own
/// weapons (fitted): surface rounds fill at most 4,000 of the 5,000.
pub const SURFACE_PROJECTILE_RESERVE: usize = 1_000;

/// What a surface round carries beyond an aircraft's, by projectile id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SurfaceRound {
    /// The round's age in ticks when it ends by itself: a flak shell bursts
    /// there (its time fuze); any other round is removed unseen (it has gone
    /// past its target). `None`: it flies its record's life.
    pub end_tick: Option<u64>,
    /// A flak shell ([`is_flak`]).
    pub flak: bool,
}

/// How a destroyed ground object explodes: its unit record's explosion type
/// (`expType`: 21 for ground vehicles, 35 for ships, 15 for men) and the
/// crater it leaves on land (`craterSize`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GroundLook {
    pub explosion: u8,
    pub crater: u8,
}

/// One surface unit's shot for [`State::fire_surface`].
#[derive(Clone, Debug, PartialEq)]
pub struct SurfaceShot {
    /// The unit that fires, a ground target row's id: the round's owner, so
    /// kill credit and the debrief's SAM and AAA tallies follow it. A battery
    /// launcher fires its own missiles (its radar only supports them).
    pub owner: u32,
    /// The record, with the AAA tuning table applied for a gun.
    pub weapon: Weapon,
    /// The unit's hardpoint index (kept as the round's station).
    pub mount: usize,
    /// World position of the muzzle or rail, above the ground.
    pub position: Vector,
    /// The aim: a gun's barrel direction, a missile's launch direction.
    pub direction: Vector,
    /// The firing unit's velocity (a moving column or ship), feet a second.
    pub velocity: Vector,
    /// The aircraft aimed at. A missile's seeker target; for a gun round only
    /// aim metadata for the ledger and the warnings, never a homing target.
    pub target: Option<u32>,
    /// A guided missile's launch track from the unit's (or battery's) radar
    /// or sight: seeds the midcourse solution, as an AI launch does.
    pub observation: Option<seeker::Observation>,
    /// A gun's running round number on this mount, for the damage share and
    /// the tracer.
    pub ordinal: u64,
    /// [`SurfaceRound::end_tick`]: see [`ticks_to_range`].
    pub end_tick: Option<u64>,
}

/// Why [`State::fire_surface`] refused a shot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refused {
    /// Firing would leave fewer than [`SURFACE_PROJECTILE_RESERVE`] free
    /// projectile slots.
    Capacity,
    /// A missile record without a reviewed guidance profile (SS-N-9, ASROC):
    /// it is not fired.
    Unreviewed,
    /// A position or direction that is not finite, or a zero direction.
    Invalid,
}

/// A surface gun record whose shells burst in the air: a gun of the AAA
/// tuning table with a collateral radius (the KS-12 and KS-19 flak).
pub fn is_flak(w: &Weapon) -> bool {
    crate::combat::surface_guns::is_surface_gun(&w.source)
        && w.damage.collateral_radius > 0
        && w.damage.collateral_percent > 0
}

/// Whether the `ordinal`th round of a surface gun carries a tracer: the AAA
/// tuning table says which guns have tracers (flak, tank guns and small arms
/// do not), and those mark every third round as aircraft guns do.
pub fn surface_tracer(w: &Weapon, ordinal: u64) -> bool {
    crate::combat::surface_guns::TABLE
        .iter()
        .find(|row| row.record.eq_ignore_ascii_case(&w.source))
        .is_some_and(|row| row.tracer)
        && gun_round::tracer(w, ordinal)
}

/// The ticks a round of `w` fired from rest takes to fly `range_ft`, under the
/// same speed law the shot code flies it by (as `surface_guns::reach_ft`), or
/// `None` if it cannot get that far in its life. A controller sets a flak
/// shell's time fuze from its predicted time of flight to the lead point, and
/// may end a gun's other rounds a little past the target's range.
pub fn ticks_to_range(w: &Weapon, range_ft: f64) -> Option<u64> {
    let movement = &w.movement;
    let mut speed_f8 = launch_speed(movement, 0).ok()? * 256;
    let mut distance = 0.;
    for tick in 0..u64::from(movement.remove_t) * 30 {
        if distance >= range_ft {
            return Some(tick);
        }
        let service = service_ticks(tick);
        if w.flags & 0x40 != 0 {
            let phase = engine_phase(movement, (tick / 30) as u16, 0);
            let target = commanded_speed(movement, phase, speed_f8, 0) as i16;
            speed_f8 = axial_speed(movement, speed_f8, target, false, service).ok()?;
        }
        distance += f64::from(speed_f8) * f64::from(service) / 65_536.;
    }
    (distance >= range_ft).then_some(u64::from(movement.remove_t) * 30)
}

impl State {
    /// Fires one surface unit's shot: a gun round or a missile from a unit
    /// that is a ground target row (or any id the host owns). Returns the
    /// projectile id. The controller calls it once per physical round.
    pub fn fire_surface(&mut self, shot: SurfaceShot) -> Result<u32, Refused> {
        let finite = |v: Vector| v.iter().all(|x| x.is_finite());
        if !finite(shot.position)
            || !finite(shot.direction)
            || !finite(shot.velocity)
            || dot(shot.direction, shot.direction) < 1e-12
        {
            return Err(Refused::Invalid);
        }
        if self.projectiles.len() + SURFACE_PROJECTILE_RESERVE >= MAX_PROJECTILES {
            return Err(Refused::Capacity);
        }
        let w = &shot.weapon;
        let gun = is_gun(w);
        let profile = (self.weapon_rules == Rules::Spec)
            .then(|| missiles::Profile::for_weapon(w))
            .flatten();
        if !gun && profile.is_none() && self.weapon_rules == Rules::Spec {
            return Err(Refused::Unreviewed);
        }
        let speed = missiles::length(shot.velocity);
        let Ok(launch) = launch_speed(&w.movement, (speed * 256.) as i32) else {
            return Err(Refused::Invalid);
        };
        let direction = crate::attitude::unit(shot.direction);
        let target = if gun { None } else { shot.target };
        let guidance = profile.map(|profile| match shot.observation {
            Some(observation) if Some(observation.id) == target => {
                Flight::from_supported_launch(profile, LaunchMode::Cued, observation, shot.position)
            }
            _ => Flight::new(profile, LaunchMode::Cued, target, shot.position),
        });
        let id = self.next_surface_shot;
        self.next_surface_shot = if id >= SURFACE_PROJECTILE_ID_LAST {
            SURFACE_PROJECTILE_ID_BASE
        } else {
            id + 1
        };
        if let Some(aim) = shot.target {
            self.ledger.aim(id, aim);
        }
        let incoming = shot
            .target
            .filter(|aim| self.ownships.iter().any(|own| own.aircraft == *aim));
        let per_game = u64::from(w.burst.actual_rounds_per_game.max(1));
        let flak = gun && is_flak(w);
        self.projectiles.push(Projectile {
            id,
            owner: shot.owner,
            weapon: Some(shot.weapon.clone()),
            guidance,
            motion: profile.map(|_| Motion::launch(w, shot.velocity, shot.position[1])),
            guidance_ticks: profile.map(|p| p.guidance_ticks),
            age: 0,
            incoming,
            station: shot.mount,
            position: shot.position,
            previous: shot.position,
            direction,
            speed_f8: launch * 256,
            launched_t: (self.tick / 30) as u16,
            target,
            fall: FallState::default(),
            gun_round: gun.then(|| (shot.ordinal % per_game) as u8),
            tracer: gun && surface_tracer(w, shot.ordinal),
        });
        self.surface_rounds.insert(
            id,
            SurfaceRound {
                end_tick: shot.end_tick,
                flak,
            },
        );
        if !gun {
            self.effect(shot.position, EffectKind::Launch);
        }
        Ok(id)
    }

    /// The surface round `id` in flight, if it is one.
    pub fn surface_round(&self, id: u32) -> Option<SurfaceRound> {
        self.surface_rounds.get(&id).copied()
    }

    /// Projectile slots surface shots may still take this tick.
    pub fn surface_capacity(&self) -> usize {
        MAX_PROJECTILES
            .saturating_sub(SURFACE_PROJECTILE_RESERVE)
            .saturating_sub(self.projectiles.len())
    }

    /// Sets the explosion and crater a ground object shows when destroyed,
    /// from its unit record. False if `id` is not a ground object.
    pub fn set_ground_look(&mut self, id: u32, look: GroundLook) -> bool {
        if !self.ground_bounds.contains_key(&id) {
            return false;
        }
        self.ground_looks.insert(id, look);
        true
    }

    /// The look [`State::set_ground_look`] gave `id`.
    pub fn ground_look(&self, id: u32) -> Option<GroundLook> {
        self.ground_looks.get(&id).copied()
    }

    /// Moves ground object `id`, a surface unit that follows a route: its
    /// contact volume becomes `bounds` and its target row follows (aim
    /// point in the upper half of the volume, as registration puts it,
    /// orientation and ground-relative velocity). Hit points and everything
    /// else stay. False if `id` is not a ground object or `bounds` is not a
    /// valid volume.
    pub fn move_ground_target(
        &mut self,
        id: u32,
        bounds: crate::airport::OrientedBox,
        velocity: Vector,
    ) -> bool {
        if !bounds.valid() || !self.ground_bounds.contains_key(&id) {
            return false;
        }
        let Some(target) = self.targets.iter_mut().find(|target| target.id == id) else {
            return false;
        };
        let basis = crate::attitude::Basis::new(bounds.heading, bounds.pitch, bounds.bank);
        target.position =
            std::array::from_fn(|i| bounds.center[i] + basis.up[i] * bounds.half[1] * 0.5);
        target.basis = basis;
        target.velocity = velocity;
        self.ground_bounds.insert(id, bounds);
        true
    }

    /// Ground object `id`'s contact volume now.
    pub fn ground_bounds(&self, id: u32) -> Option<crate::airport::OrientedBox> {
        self.ground_bounds.get(&id).copied()
    }
}

/// A surface round's burst, applied after the round search of a step.
#[derive(Clone, Copy, Debug)]
pub(super) struct Burst {
    pub projectile: u32,
    pub owner: u32,
    pub owner_side: Side,
    pub station: usize,
    pub position: Vector,
    pub damage: [i16; 5],
    pub radius: f64,
    pub percent: i32,
    pub flags: u32,
    /// The aircraft the round struck directly, which takes no collateral.
    pub exclude: Option<u32>,
    /// The burst is the round's outcome (a flak shell that struck nothing):
    /// the ledger records it as a hit if it damaged anyone.
    pub resolves: bool,
}
impl Burst {
    pub fn new(
        p: &Projectile,
        w: &Weapon,
        owner_side: Side,
        position: Vector,
        exclude: Option<u32>,
        resolves: bool,
    ) -> Self {
        Self {
            projectile: p.id,
            owner: p.owner,
            owner_side,
            station: p.station,
            position,
            damage: w.damage.by_class,
            radius: f64::from(w.damage.collateral_radius.max(0)),
            percent: i32::from(w.damage.collateral_percent.max(0)),
            flags: w.flags,
            exclude,
            resolves,
        }
    }
    /// Whether the burst reaches anyone at all.
    pub fn collateral(&self) -> bool {
        self.radius > 0. && self.percent > 0
    }
    fn nominal(&self, category: u16) -> i32 {
        i32::from(self.damage[damage_class(category)]).max(0) * self.percent / 100
    }
}

/// An ownship's share of a burst, in the step's ownship-hit form: row, amount,
/// section, direct gun hit, owner and weapon flags.
pub(super) type OwnshipHit = (usize, i32, DamageSection, bool, u32, u32);

/// The tick's shared lists a burst adds to.
pub(super) struct Outputs<'a> {
    pub events: &'a mut Vec<Event>,
    pub strikes: &'a mut Vec<Strike>,
    pub ownship_hits: &'a mut Vec<OwnshipHit>,
    pub impacts: &'a mut Vec<(Vector, EffectKind, u8, u8)>,
}

impl State {
    /// Applies one burst's collateral damage to every aircraft in its radius:
    /// AI aircraft rows at once, ownships through the step's ownship hits.
    pub(super) fn apply_burst(
        &mut self,
        burst: &Burst,
        rows: &[OwnRow],
        friendly_fire_off: bool,
        out: Outputs<'_>,
    ) {
        let spares = |side: Side| {
            friendly_fire_off && burst.owner_side != NO_SIDE && side == burst.owner_side
        };
        let within = |position: Vector| {
            let d = sub(position, burst.position);
            dot(d, d) <= burst.radius * burst.radius
        };
        let mut total: u32 = 0;
        if burst.collateral() {
            for (n, r) in rows.iter().enumerate() {
                let t = &r.target;
                if Some(t.id) == burst.exclude
                    || t.hp <= 0
                    || !t.body_present()
                    || spares(t.side)
                    || !within(t.position)
                {
                    continue;
                }
                let base = burst.nominal(t.category);
                if base <= 0 {
                    continue;
                }
                let amount = super::super::systems::damage_amount(
                    u16::try_from(base).unwrap_or(u16::MAX),
                    100,
                    draw(&mut self.rng, 40) as u8,
                );
                let section = LocalizedDamage::section(burst.position, t);
                out.ownship_hits
                    .push((n, amount, section, false, burst.owner, burst.flags));
                out.events.push(Event::Jolt(super::Jolt {
                    target: t.id,
                    from: burst.position,
                    strength: f64::from(base) / 100.,
                }));
                total = total.saturating_add(u32::try_from(amount).unwrap_or(0));
            }
            for t in &mut self.targets {
                if t.role != TargetRole::Aircraft
                    || Some(t.id) == burst.exclude
                    || t.hp <= 0
                    || !t.body_present()
                    || spares(t.side)
                    || !within(t.position)
                {
                    continue;
                }
                let nominal = burst.nominal(t.category);
                if nominal <= 0 {
                    continue;
                }
                out.events.push(Event::Jolt(super::Jolt {
                    target: t.id,
                    from: burst.position,
                    strength: f64::from(nominal) / 100.,
                }));
                let section = LocalizedDamage::section(burst.position, t);
                let applied = nominal.min(t.hp);
                t.hp -= applied;
                t.localized_damage.record(section, nominal, t.initial_hp);
                if t.hp > 0 {
                    let rng = &mut self.rng;
                    t.faults.hit(nominal, t.initial_hp, |n| draw(rng, n));
                }
                if self.history.len() == MAX_HIT_RECORDS {
                    self.history.remove(0);
                }
                self.history.push(HitRecord {
                    tick: self.tick,
                    target: t.id,
                    station: burst.station,
                    class: damage_class(t.category),
                    nominal,
                    applied,
                    hp_after: t.hp,
                });
                let credit = Kill {
                    owner: burst.owner,
                    victim: t.id,
                    category: t.category,
                    aircraft: true,
                };
                if applied > 0 {
                    self.ledger.damaged(credit);
                }
                out.events.push(Event::Hit(t.id));
                out.strikes.push(Strike {
                    owner: burst.owner,
                    victim: t.id,
                    weapon_flags: burst.flags,
                    destroyed: t.hp == 0,
                    amount: applied,
                });
                if t.hp == 0 {
                    self.ledger.kill(credit);
                    out.events.push(Event::Destroyed(t.id));
                    out.impacts.push((
                        t.position,
                        EffectKind::Destroyed,
                        crate::combat::blast::AIRCRAFT,
                        0,
                    ));
                }
                total = total.saturating_add(u32::try_from(applied).unwrap_or(0));
            }
        }
        if burst.resolves {
            self.ledger.resolve(
                burst.projectile,
                if total > 0 {
                    Resolution::Hit(total)
                } else {
                    Resolution::Missed
                },
            );
        }
    }
}

#[cfg(test)]
#[path = "surface_tests.rs"]
mod tests;
