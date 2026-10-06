//! Sorting a tick into events: what the host tells every player about the
//! mission (effects, marks, destroyed ground objects, launches, gun bursts,
//! countermeasures, sounds; wing ejections come with each seat's cues), and
//! the mission as it stands for a player who has just been seated.
//!
//! [`Tracker`] keeps what the host has told about so far, so each tick only
//! the new things become events. It reads combat's shared state after the
//! step, so it needs no seat and runs whether or not anyone is connected.

use crate::wire::events::WireEvent;
use crate::wire::from_world;
use std::collections::{BTreeMap, BTreeSet, HashSet};
use tore_sim::combat::live::{self, DeviceNote};
use tore_sim::combat::missiles::TargetRole;
use tore_world::snapshot::{EffectPose, MarkPose};
use tore_world::world::{TickOutput, World};

/// Ticks past a gun's round interval after which a burst is over: a gun
/// station that fires no round for its interval and this many ticks more has
/// stopped. *Agent decision:* the interval comes from the weapon's burst
/// record, as combat's gun cadence spaces the rounds (a representative round
/// every burst time times 30 over the physical rounds, in ticks), so a
/// burst is one trigger pull whatever the gun.
pub const BURST_SLACK_TICKS: u64 = 2;

/// The ticks between a held gun's representative rounds.
pub fn round_interval(weapon: &tore_formats::weapons::Weapon) -> u64 {
    let burst = &weapon.burst;
    let rounds = u64::from(burst.game_rounds_in_burst.max(1))
        * u64::from(burst.actual_rounds_per_game.max(1));
    (u64::from(burst.game_burst_t.max(1)) * 30).div_ceil(rounds.max(1))
}

/// A mission-wide event before it is put in a connection's terms: a launch
/// names its weapon, which each connection's name table numbers.
#[derive(Clone, Debug, PartialEq)]
pub enum Wide {
    Event(WireEvent),
    Launch {
        shooter: u32,
        projectile: u32,
        weapon: String,
    },
}

/// One event and the host tick it belongs to.
#[derive(Clone, Debug, PartialEq)]
pub struct Timed {
    pub tick: u64,
    pub event: Wide,
}

type EffectKey = (u8, [u64; 3], Option<u8>);

fn effect_key(effect: &live::Effect) -> EffectKey {
    (
        effect.kind as u8,
        effect.position.map(f64::to_bits),
        effect.blast,
    )
}

/// What the host has told every player about the mission so far.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    /// The highest projectile number seen: newer ones are this tick's.
    newest_projectile: Option<u32>,
    /// Bursts still firing: shooter and station, first and last tick, and
    /// the ticks without a round that end it.
    bursts: BTreeMap<(u32, usize), (u64, u64, u64)>,
    effects: HashSet<EffectKey>,
    newest_mark: Option<u64>,
    /// Ground objects destroyed so far.
    destroyed: BTreeSet<u32>,
}

impl Tracker {
    /// A tracker that treats everything in `world` now as already told, so
    /// the first tick sorted is only that tick's news.
    pub fn new(world: &World) -> Self {
        let state = &world.combat.state;
        Self {
            newest_projectile: state.projectiles.iter().map(|p| p.id).max(),
            bursts: BTreeMap::new(),
            effects: state.effects.iter().map(effect_key).collect(),
            newest_mark: state.marks.iter().map(|m| m.serial).max(),
            destroyed: destroyed_ground(world),
        }
    }

    /// The ground objects destroyed so far, for a Seated message.
    pub fn destroyed(&self) -> &BTreeSet<u32> {
        &self.destroyed
    }

    /// The mission-wide events of the tick `world` just stepped (`tick`),
    /// from its output `out` and combat's countermeasure `notes`, which the
    /// journal drained after the step (slice K1: the tracker only reads the
    /// world), oldest first.
    pub fn sort(
        &mut self,
        world: &World,
        out: &TickOutput,
        notes: Vec<DeviceNote>,
        tick: u64,
    ) -> Vec<Timed> {
        let mut events = Vec::new();
        let mut push = |tick: u64, event: Wide| events.push(Timed { tick, event });

        // Launches and gun bursts, from the projectiles new this tick.
        let state = &world.combat.state;
        let newest = self.newest_projectile;
        let mut fresh: Vec<&live::Projectile> = state
            .projectiles
            .iter()
            .filter(|p| newest.is_none_or(|n| p.id > n))
            .collect();
        fresh.sort_by_key(|p| p.id);
        for projectile in fresh {
            self.newest_projectile = Some(projectile.id);
            let weapon = state.weapon(projectile);
            if live::is_gun(weapon) {
                let key = (projectile.owner, projectile.station);
                match self.bursts.get_mut(&key) {
                    Some((_, last, _)) => *last = tick,
                    None => {
                        let gap = round_interval(weapon) + BURST_SLACK_TICKS;
                        self.bursts.insert(key, (tick, tick, gap));
                        push(
                            tick,
                            Wide::Event(WireEvent::GunBurst {
                                shooter: projectile.owner,
                                station: projectile.station.min(255) as u8,
                                length: None,
                            }),
                        );
                    }
                }
            } else {
                push(
                    tick,
                    Wide::Launch {
                        shooter: projectile.owner,
                        projectile: projectile.id,
                        weapon: weapon.source.clone(),
                    },
                );
            }
        }
        let ended: Vec<(u32, usize)> = self
            .bursts
            .iter()
            .filter(|(_, (_, last, gap))| tick - *last > *gap)
            .map(|(key, _)| *key)
            .collect();
        for key in ended {
            if let Some((first, last, _)) = self.bursts.remove(&key) {
                // The whole burst, from its first tick, now that it is over.
                push(
                    first,
                    Wide::Event(WireEvent::GunBurst {
                        shooter: key.0,
                        station: key.1.min(255) as u8,
                        length: Some((last - first + 1) as u32),
                    }),
                );
            }
        }

        // Flashes, hits and explosions new this tick.
        let now: HashSet<EffectKey> = state.effects.iter().map(effect_key).collect();
        for effect in &state.effects {
            if !self.effects.contains(&effect_key(effect)) {
                push(
                    tick,
                    Wide::Event(from_world::effect_event(&effect_pose(effect))),
                );
            }
        }
        self.effects = now;

        // Craters and crash-site fires.
        let newest_mark = self.newest_mark;
        for mark in state
            .marks
            .iter()
            .filter(|m| newest_mark.is_none_or(|n| m.serial > n))
        {
            self.newest_mark = Some(self.newest_mark.map_or(mark.serial, |n| n.max(mark.serial)));
            push(
                tick,
                Wide::Event(from_world::mark_event(&MarkPose::of(mark, state.tick()))),
            );
        }

        // Ground objects destroyed this tick.
        for id in destroyed_ground(world) {
            if self.destroyed.insert(id) {
                push(tick, Wide::Event(WireEvent::GroundDestroyed { object: id }));
            }
        }

        // Sounds.
        for emission in &out.emissions {
            push(tick, Wide::Event(from_world::sound_event(emission, None)));
        }

        // Chaff and flares.
        for note in notes {
            if let DeviceNote::Released(release) = note {
                push(
                    tick,
                    Wide::Event(from_world::countermeasure_event(&release)),
                );
            }
        }
        events
    }

    /// The events that describe the mission as it stands, for a player just
    /// seated: every crater and crash-site fire and every effect still
    /// showing, at `tick`. Destroyed ground objects travel in the Seated
    /// message; smoke and contrails already in the sky are not sent (the
    /// protocol's "first snapshot").
    pub fn standing(world: &World, tick: u64) -> Vec<Timed> {
        let state = &world.combat.state;
        let marks = state
            .marks
            .iter()
            .map(|mark| Wide::Event(from_world::mark_event(&MarkPose::of(mark, state.tick()))));
        let effects = state
            .effects
            .iter()
            .map(|effect| Wide::Event(from_world::effect_event(&effect_pose(effect))));
        marks
            .chain(effects)
            .map(|event| Timed { tick, event })
            .collect()
    }
}

fn effect_pose(effect: &live::Effect) -> EffectPose {
    EffectPose {
        kind: effect.kind,
        position: effect.position,
        ticks: effect.ticks,
        blast: effect.blast,
    }
}

/// Every ground object with no hit points left.
fn destroyed_ground(world: &World) -> BTreeSet<u32> {
    world
        .combat
        .state
        .targets
        .iter()
        .filter(|target| target.role == TargetRole::Surface && target.hp <= 0)
        .map(|target| target.id)
        .collect()
}
