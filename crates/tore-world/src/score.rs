//! Score facts: what the mission core records for a networked game's scoring
//! each tick, for the host to tally. Stage F phase 2; see
//! docs/ARCHITECTURE.md, "Scoring".
//!
//! The host turns recording on ([`crate::world::World::set_scoring`]); single
//! player never does, so its tick and fingerprint are untouched. With it on,
//! each tick ends (after combat and the AI, before the radio drains combat's
//! strikes) with [`Recorder::record`], which only reads the world:
//!
//! - **Damage**: every projectile hit of the tick (combat's strikes), as the
//!   fraction of the victim's full hit points it took.
//! - **Kill**: every plane of the roster the tick lost (destroyed, its pilot
//!   dead or ejected, or crashed), credited as the debrief credits it: the
//!   kill combat recorded, else the last plane to hit it (the ledger's
//!   `credit`); and every other target a hit destroyed (a ground object).
//!   Each target is recorded once.
//! - **Loss**: a lost plane a human was flying.
//!
//! The pilots are the roster's at the fact's tick: who flies the shooter and
//! the victim. The host drains the facts every tick
//! ([`crate::world::World::take_score_facts`]).

use crate::seats::{Pilot, PlaneId, SeatId};
use crate::world::World;
use std::collections::BTreeSet;
use tore_sim::combat::{live::Strike, missiles::TargetRole};

// Exact checkpoints (docs/formats/checkpoint.md): the score section.
#[path = "score_checkpoint.rs"]
mod checkpoint;

/// Facts kept between drains; the oldest go first, so a driver that never
/// drains them still uses bounded memory.
pub const MAX_FACTS: usize = 1024;

/// One aircraft of the roster and who flew it at the fact's tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flown {
    pub plane: PlaneId,
    pub pilot: Pilot,
}

/// What was hit: a combat target, with its plane and pilot when it is an
/// aircraft of the roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Victim {
    /// The combat target id (a plane's id is its target id).
    pub target: u32,
    /// The plane and its pilot, when the target is a plane of the roster.
    pub flown: Option<Flown>,
    /// An aircraft or a helicopter: only these count as kills.
    pub aircraft: bool,
}

/// One fact of a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fact {
    /// `victim` was killed. `shooter` is `None` when no plane of the roster
    /// is credited (a ground site fired, the plane crashed or flew out of
    /// bounds untouched, or its last hit was its own). `pilot_aboard`: the
    /// victim's pilot had not ejected (retail counts a human killed before
    /// ejecting twice); a ground object's is false.
    Kill {
        shooter: Option<Flown>,
        victim: Victim,
        pilot_aboard: bool,
    },
    /// A hit for `fraction` of the victim's full hit points.
    Damage {
        shooter: Option<Flown>,
        victim: Victim,
        fraction: f64,
    },
    /// A human's plane was lost, by any cause: destroyed, its pilot dead or
    /// ejected.
    Loss { plane: PlaneId, seat: SeatId },
}

/// A tick's facts, in the order they happened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// The tick they belong to: the last tick stepped.
    pub tick: u64,
    pub facts: Vec<Fact>,
}

/// The mission core's recording of score facts while the host has scoring
/// on. Its only state between ticks is which targets' ends are recorded
/// (stage H's checkpoints carry it); the facts are drained every tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Recorder {
    /// Combat target ids whose kill or loss has been recorded.
    recorded: BTreeSet<u32>,
    /// Facts since the last drain.
    pending: Facts,
}

/// How a plane of the roster stands after a tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Standing {
    lost: bool,
    /// Lost by its pilot's escape: the pilot was not aboard.
    ejected: bool,
}

impl Recorder {
    /// The targets whose end is recorded, in id order.
    pub fn recorded(&self) -> impl Iterator<Item = u32> + '_ {
        self.recorded.iter().copied()
    }

    /// The facts since the last drain, leaving none.
    pub fn take(&mut self) -> Facts {
        let tick = self.pending.tick;
        std::mem::replace(
            &mut self.pending,
            Facts {
                tick,
                facts: Vec::new(),
            },
        )
    }

    fn push(&mut self, fact: Fact) {
        if self.pending.facts.len() == MAX_FACTS {
            self.pending.facts.remove(0);
        }
        self.pending.facts.push(fact);
    }

    /// Records tick `tick`'s facts from `world` as the tick left it and the
    /// tick's `strikes`. Reads only.
    pub fn record(&mut self, world: &World, tick: u64, strikes: &[Strike]) {
        self.pending.tick = tick;
        for strike in strikes {
            if strike.amount <= 0 {
                continue;
            }
            let Some(full) = full_hit_points(world, strike.victim) else {
                continue;
            };
            self.push(Fact::Damage {
                shooter: flown(world, strike.owner),
                victim: victim(world, strike.victim),
                fraction: f64::from(strike.amount) / f64::from(full.max(1)),
            });
        }
        // Every plane the tick lost, in id order.
        for plane in world.roster.planes() {
            let id = plane.id.0;
            if self.recorded.contains(&id) || !standing(world, plane.id).lost {
                continue;
            }
            self.recorded.insert(id);
            let shooter = world
                .combat
                .state
                .ledger
                .credit(id)
                .filter(|kill| kill.owner != id)
                .and_then(|kill| flown(world, kill.owner));
            self.push(Fact::Kill {
                shooter,
                victim: Victim {
                    target: id,
                    flown: Some(Flown {
                        plane: plane.id,
                        pilot: plane.pilot,
                    }),
                    aircraft: true,
                },
                pilot_aboard: !standing(world, plane.id).ejected,
            });
            if let Pilot::Human(seat) = plane.pilot {
                self.push(Fact::Loss {
                    plane: plane.id,
                    seat,
                });
            }
        }
        // Any other target a hit destroyed: a ground object, or an aircraft
        // outside the roster.
        for strike in strikes.iter().filter(|s| s.destroyed) {
            if world.roster.plane(PlaneId(strike.victim)).is_some()
                || !self.recorded.insert(strike.victim)
            {
                continue;
            }
            let victim = victim(world, strike.victim);
            self.push(Fact::Kill {
                shooter: flown(world, strike.owner),
                victim,
                pilot_aboard: victim.aircraft,
            });
        }
    }
}

/// The plane of the roster `owner` is, with who flies it now.
fn flown(world: &World, owner: u32) -> Option<Flown> {
    world.roster.plane(PlaneId(owner)).map(|plane| Flown {
        plane: plane.id,
        pilot: plane.pilot,
    })
}

/// The combat target `target` as a victim.
fn victim(world: &World, target: u32) -> Victim {
    let flown = flown(world, target);
    Victim {
        target,
        flown,
        aircraft: flown.is_some()
            || world
                .combat
                .state
                .targets
                .iter()
                .any(|t| t.id == target && t.role == TargetRole::Aircraft),
    }
}

/// The full hit points of the combat target `target`: a human-flown plane's
/// ownship's capacity, or a combat row's starting hit points.
fn full_hit_points(world: &World, target: u32) -> Option<i32> {
    if let Some(own) = world.combat.state.ownship(target) {
        return Some(own.configuration().damage_capacity);
    }
    world
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == target)
        .map(|t| t.initial_hp)
}

/// Whether the plane `plane` is lost, as the handoff and the debrief judge
/// it: its flight crashed, its hit points gone, its pilot dead or escaped. A
/// human-flown plane reads its cockpit and ownship, an AI one its actor and
/// combat row; a plane with neither is lost.
fn standing(world: &World, plane: PlaneId) -> Standing {
    let of = |flight: &tore_sim::flight::State, hp: i32, alive: bool| {
        let pilot = &flight.systems.pilot;
        let ejected = !pilot.dead && (pilot.ejected || flight.escape.is_some());
        Standing {
            lost: !alive || flight.crashed || hp <= 0 || pilot.dead || ejected,
            ejected,
        }
    };
    if let Some(cockpit) = world.cockpits.iter().find(|c| c.plane == plane) {
        let hp = world.combat.state.ownship(plane.0).map_or(1, |own| own.hp);
        return of(&cockpit.flight, hp, true);
    }
    let actor = world
        .ai_wings
        .as_ref()
        .and_then(|wings| wings.mission().actor(plane.0));
    match actor {
        Some(actor) => {
            let hp = world
                .combat
                .state
                .targets
                .iter()
                .find(|t| t.id == plane.0)
                .map_or(1, |t| t.hp);
            of(actor.flight(), hp, actor.alive())
        }
        None => Standing {
            lost: true,
            ejected: false,
        },
    }
}
