//! Sorting a tick into events: what the host tells every player about the
//! mission (effects, marks, destroyed ground objects, launches, gun bursts,
//! countermeasures, sounds; wing ejections come with each seat's cues; since
//! protocol 22 the surface units' gun bursts and their changed states), and
//! the mission as it stands for a player who has just been seated.
//!
//! [`Tracker`] keeps what the host has told about so far, so each tick only
//! the new things become events. It reads combat's shared state after the
//! step, so it needs no seat and runs whether or not anyone is connected.
//!
//! Surface units' rounds (protocol 22, slice N1) are kept apart from the
//! aircraft's: a surface gun's burst is one Surface burst event with its
//! schedule (from the surface tick's burst notes), closed early by a Surface
//! burst end when the controller stops short of it; flak shells are never
//! sent, since each burst in the air is an Effect; a surface missile is a
//! Launch like any other, its shooter the unit.

use crate::wire::events::{SurfaceUnitView, WireEvent};
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

/// A surface gun's burst still within its schedule: its first tick, the
/// rounds and ticks the schedule holds, the controller firing it and the
/// rounds it has let go so far.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct OpenSurfaceBurst {
    first: u64,
    rounds: u32,
    span: u64,
    engager: usize,
    fired: u32,
}

/// Ticks past a surface burst's schedule after which it is over, whatever
/// its controller says.
const SURFACE_BURST_SLACK_TICKS: u64 = 2;

/// What the host has told every player about the mission so far.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    /// The projectiles in the air at the last tick sorted: any other is
    /// this tick's. (A highest number would do for the aircraft's, but the
    /// surface's are numbered in a block of their own, protocol 22.)
    known_projectiles: HashSet<u32>,
    /// Bursts still firing: shooter and station, first and last tick, and
    /// the ticks without a round that end it.
    bursts: BTreeMap<(u32, usize), (u64, u64, u64)>,
    /// Surface guns' bursts within their schedule, by unit and hardpoint.
    surface_bursts: BTreeMap<(u32, usize), OpenSurfaceBurst>,
    effects: HashSet<EffectKey>,
    newest_mark: Option<u64>,
    /// Ground objects destroyed so far.
    destroyed: BTreeSet<u32>,
    /// Each surface unit's state as last told, by unit.
    views: BTreeMap<u32, SurfaceUnitView>,
}

impl Tracker {
    /// A tracker that treats everything in `world` now as already told, so
    /// the first tick sorted is only that tick's news.
    pub fn new(world: &World) -> Self {
        let state = &world.combat.state;
        Self {
            known_projectiles: state.projectiles.iter().map(|p| p.id).collect(),
            bursts: BTreeMap::new(),
            surface_bursts: BTreeMap::new(),
            effects: state.effects.iter().map(effect_key).collect(),
            newest_mark: state.marks.iter().map(|m| m.serial).max(),
            destroyed: destroyed_ground(world),
            views: from_world::surface_views(world)
                .into_iter()
                .map(|view| (view.unit, view))
                .collect(),
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
        let mut fresh: Vec<&live::Projectile> = state
            .projectiles
            .iter()
            .filter(|p| !self.known_projectiles.contains(&p.id))
            .collect();
        fresh.sort_by_key(|p| p.id);
        self.known_projectiles = state.projectiles.iter().map(|p| p.id).collect();
        // A surface gun's rounds of this tick, by unit and hardpoint.
        let mut surface_rounds: BTreeMap<(u32, usize), u32> = BTreeMap::new();
        for projectile in fresh {
            let weapon = state.weapon(projectile);
            if state.surface_round(projectile.id).is_some() && live::is_gun(weapon) {
                // Flak shells are told by their bursts' Effects; other
                // rounds by the surface burst events below.
                *surface_rounds
                    .entry((projectile.owner, projectile.station))
                    .or_default() += 1;
            } else if live::is_gun(weapon) {
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
        self.sort_surface_bursts(world, tick, &surface_rounds, &mut push);
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

        // Surface units whose state a client draws or shows changed.
        for view in from_world::surface_views(world) {
            if self.views.get(&view.unit) != Some(&view) {
                self.views.insert(view.unit, view.clone());
                push(tick, Wide::Event(WireEvent::SurfaceUnit(view)));
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

    /// The surface guns' bursts of the tick: each burst the surface tick
    /// began is told with its schedule; one whose controller stopped short
    /// of it (or whose unit died) is closed with the rounds it fired.
    /// `rounds` are the tick's new rounds by unit and hardpoint.
    fn sort_surface_bursts(
        &mut self,
        world: &World,
        tick: u64,
        rounds: &BTreeMap<(u32, usize), u32>,
        push: &mut impl FnMut(u64, Wide),
    ) {
        let notes = &world.combat.surface.bursts;
        let opened =
            |key: &(u32, usize)| notes.iter().any(|note| (note.unit.0, note.mount) == *key);
        // The rounds of a burst still running.
        for (key, burst) in &mut self.surface_bursts {
            if !opened(key) {
                burst.fired += rounds.get(key).copied().unwrap_or(0);
            }
        }
        // Bursts that are over.
        let alive = |id: u32| {
            world
                .combat
                .state
                .targets
                .iter()
                .any(|t| t.id == id && t.hp > 0)
        };
        let over: Vec<(u32, usize)> = self
            .surface_bursts
            .iter()
            .filter(|((unit, _), burst)| {
                let running = world
                    .combat
                    .surface
                    .unit(tore_world::surface::UnitId(*unit))
                    .and_then(|u| u.engagers.get(burst.engager))
                    .and_then(|e| e.controller.burst())
                    .is_some_and(|(start, ..)| start == burst.first);
                !running
                    || !alive(*unit)
                    || tick > burst.first + burst.span + SURFACE_BURST_SLACK_TICKS
            })
            .map(|(key, _)| *key)
            .collect();
        for key in over {
            if let Some(burst) = self.surface_bursts.remove(&key)
                && burst.fired < burst.rounds
            {
                push(
                    burst.first,
                    Wide::Event(WireEvent::SurfaceBurstEnd {
                        unit: key.0,
                        mount: key.1.min(255) as u8,
                        fired: burst.fired.min(u32::from(u16::MAX)) as u16,
                    }),
                );
            }
        }
        // Bursts begun this tick.
        for note in notes {
            let key = (note.unit.0, note.mount);
            let [x, y, z] = note.direction;
            push(
                tick,
                Wide::Event(WireEvent::SurfaceBurst {
                    unit: note.unit.0,
                    mount: note.mount.min(255) as u8,
                    target: Some(note.target),
                    aim: [
                        crate::wire::bits::turn16(x.atan2(z)),
                        crate::wire::bits::turn16(y.atan2(x.hypot(z))),
                    ],
                    rounds: note.rounds.min(u32::from(u16::MAX)) as u16,
                    span: note.span.min(u64::from(u16::MAX)) as u16,
                }),
            );
            let burst = OpenSurfaceBurst {
                first: tick,
                rounds: note.rounds,
                span: note.span,
                engager: note.engager,
                fired: rounds.get(&key).copied().unwrap_or(0),
            };
            // A burst of one tick (a single shot) is over as it begins.
            let running = note.rounds > burst.fired;
            if running {
                self.surface_bursts.insert(key, burst);
            } else {
                self.surface_bursts.remove(&key);
            }
        }
    }

    /// The events that describe the mission as it stands, for a player just
    /// seated: every crater and crash-site fire and every effect still
    /// showing, and every surface unit not as the mission built it (protocol
    /// 22), at `tick`. Destroyed ground objects travel in the Seated
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
        let initial: BTreeMap<u32, i32> =
            state.targets.iter().map(|t| (t.id, t.initial_hp)).collect();
        let surface = from_world::surface_views(world)
            .into_iter()
            .filter(|view| {
                let built = from_world::built_view(
                    &world.terrain.surface,
                    view.unit,
                    initial.get(&view.unit).copied().unwrap_or(view.hp),
                );
                *view != built
            })
            .map(|view| Wide::Event(WireEvent::SurfaceUnit(view)));
        marks
            .chain(effects)
            .chain(surface)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tore_world::surface::fire::BurstNote;
    use tore_world::surface::{SURFACE_UNIT_BASE, UnitId};
    use tore_world::test_support::surface::{spec_with_target, surface_resources, target};
    use tore_world::world::{Seating, TickOutput};

    /// The synthetic defended site with every slot manned.
    fn world() -> World {
        let spec = spec_with_target(&target("QUCITY", 3, 3, 9));
        World::new(&spec, &surface_resources(), Seating::Open).unwrap()
    }

    fn bursts(events: &[Timed]) -> Vec<(u64, WireEvent)> {
        events
            .iter()
            .filter_map(|timed| match &timed.event {
                Wide::Event(
                    event @ (WireEvent::SurfaceBurst { .. } | WireEvent::SurfaceBurstEnd { .. }),
                ) => Some((timed.tick, event.clone())),
                _ => None,
            })
            .collect()
    }

    /// A surface gun's burst is told once with its schedule, apart from the
    /// aircraft's gun bursts; one whose controller is no longer firing it is
    /// closed with the rounds it let go (here none), at its first tick.
    #[test]
    fn a_surface_burst_is_told_with_its_schedule_and_closed_when_cut_short() {
        let mut world = world();
        let mut tracker = Tracker::new(&world);
        let unit = UnitId(SURFACE_UNIT_BASE + 6);
        world.combat.surface.bursts.push(BurstNote {
            unit,
            mount: 0,
            engager: 0,
            target: 3,
            direction: [0., 1., 1.],
            rounds: 40,
            span: 90,
        });
        let out = TickOutput::default();
        let told = bursts(&tracker.sort(&world, &out, Vec::new(), 100));
        assert_eq!(told.len(), 1, "{told:?}");
        let (
            tick,
            WireEvent::SurfaceBurst {
                unit: id,
                mount,
                target,
                aim,
                rounds,
                span,
            },
        ) = &told[0]
        else {
            panic!("{told:?}");
        };
        assert_eq!((*tick, *id, *mount, *target), (100, unit.0, 0, Some(3)));
        assert_eq!((*rounds, *span), (40, 90));
        // North-going and 45 degrees up: azimuth 0, an eighth of a turn.
        assert_eq!(*aim, [0, 8_192]);
        // The next tick: the notes are gone (the surface tick clears them)
        // and no controller fires the burst, so it ends with none of its
        // rounds, told once.
        world.combat.surface.bursts.clear();
        let told = bursts(&tracker.sort(&world, &out, Vec::new(), 101));
        assert_eq!(
            told,
            [(
                100,
                WireEvent::SurfaceBurstEnd {
                    unit: unit.0,
                    mount: 0,
                    fired: 0
                }
            )]
        );
        assert!(bursts(&tracker.sort(&world, &out, Vec::new(), 102)).is_empty());
    }

    /// A surface unit whose hit points, radar or stock change is told once
    /// per change, and a player seated later is told every unit that is not
    /// as the mission built it.
    #[test]
    fn surface_unit_states_are_told_when_they_change_and_at_a_seat() {
        let mut world = world();
        let mut tracker = Tracker::new(&world);
        let out = TickOutput::default();
        let views = |events: &[Timed]| -> Vec<SurfaceUnitView> {
            events
                .iter()
                .filter_map(|t| match &t.event {
                    Wide::Event(WireEvent::SurfaceUnit(view)) => Some(view.clone()),
                    _ => None,
                })
                .collect()
        };
        assert!(views(&tracker.sort(&world, &out, Vec::new(), 1)).is_empty());
        assert!(
            views(&Tracker::standing(&world, 1)).is_empty(),
            "all as built"
        );
        let id = SURFACE_UNIT_BASE + 12;
        let row = world
            .combat
            .state
            .targets
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap();
        row.hp -= 10;
        row.radar_emitting = true;
        let hp = row.hp;
        let told = views(&tracker.sort(&world, &out, Vec::new(), 2));
        assert_eq!(told.len(), 1);
        assert_eq!((told[0].unit, told[0].hp, told[0].radar), (id, hp, true));
        assert!(views(&tracker.sort(&world, &out, Vec::new(), 3)).is_empty());
        let standing = views(&Tracker::standing(&world, 3));
        assert_eq!(standing, told, "a late seat is told the hurt bunker");
    }
}
