//! Where surface units point their weapons, for drawing
//! (docs/spec/surface-defenses.md, "Aiming"). Presentation only: nothing
//! here feeds the fight, which aims each round itself.
//!
//! A unit turns toward the target one of its weapons holds:
//!
//! - a **turret** (the shape's `@HardpointAngle@4` envelope: the Krivak's
//!   and Sovremennyy's guns) traverses inside its mount's arc, relative to
//!   the hull, and returns to its rest when nothing is held;
//! - a **body** (a land unit whose shape has no turret) turns as a whole
//!   about its up axis at its type's turn rate, keeps the heading it last
//!   aimed at, and lines up with its route again while it drives.
//!
//! What a weapon holds ([`Track`]) comes from the controllers' state (from
//! the moment one chooses a target, so a slow mount turns during its
//! preparation), the same phases and targets a replay records, so a flight
//! and its replay turn the same units at the same targets. [`Aims`] slews
//! toward them by mission ticks, so a part turns at its rate whatever the
//! frame rate (its goal is taken once a step); a jump in time (a replay
//! seek, a restart) sets every part straight on its goal.
use super::fire::{Arms, MountArc};
use super::{Surface, SurfaceState, UnitId};
use std::collections::BTreeMap;
use std::f64::consts::{PI, TAU};
use tore_sim::ai::surface::Phase;
use tore_sim::attitude::Vector;

/// Simulation ticks a second.
const TICKS_PER_SECOND: f64 = 120.;
/// A turret's traverse rate, degrees a second. Not in the data (only the
/// arcs are): fitted.
pub const TURRET_RATE_DEG_S: f64 = 45.;
/// A gap longer than this, or time running backwards, sets every part on
/// its goal at once instead of slewing (a seek or a restart).
pub const SNAP_TICKS: u64 = 120;
/// A unit that moved more than this since the last step drives, feet a
/// tick: its body lines up with its route.
const DRIVING_FT_PER_TICK: f64 = 0.01;
/// A burst or salvo is held this long after it began, when it is all a
/// viewer knows of an engagement (a battery launcher in a replay, a gun on
/// a network client): longer than the longest pause between bursts
/// (flak's 4 s). Fitted.
pub const BURST_HOLD_TICKS: u64 = 600;

/// Which of a unit's weapons holds a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Weapon {
    /// A controller, by index in its arms' weapons (the replay's `station`).
    Station(usize),
    /// A hardpoint, as a burst names it.
    Mount(usize),
}

/// One weapon of a unit holding a target.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Track {
    pub unit: UnitId,
    pub weapon: Weapon,
    pub target: u32,
}

/// Whether a controller in `phase` points its weapon at its target: from
/// the moment it chooses one (Prepare) while it holds it (Track, Fire,
/// Pause). Turning during the preparation lets a slow mount come round
/// before the first round leaves.
pub fn aims_in(phase: Phase) -> bool {
    phase == Phase::Prepare || phase.locked()
}

/// The targets every unit's weapons hold now: each controller that has
/// chosen one ([`aims_in`]), and each launcher of a battery whose
/// controller has.
pub fn tracks(surface: &Surface, state: &SurfaceState) -> Vec<Track> {
    let mut out = Vec::new();
    for unit in &state.units {
        for (station, engager) in unit.engagers.iter().enumerate() {
            let controller = &engager.controller;
            if let (true, Some(target)) = (aims_in(controller.phase()), controller.target()) {
                out.push(Track {
                    unit: unit.id,
                    weapon: Weapon::Station(station),
                    target,
                });
            }
        }
    }
    for battery in &surface.arsenal.batteries {
        let Some(controller) = state.batteries.get(battery.index).map(|b| &b.controller) else {
            continue;
        };
        let (true, Some(target)) = (aims_in(controller.phase()), controller.target()) else {
            continue;
        };
        for launcher in &battery.launchers {
            let Some(arms) = surface.arsenal.arms(*launcher) else {
                continue;
            };
            for (station, _) in arms.weapons.iter().enumerate().filter(|(_, w)| w.battery) {
                out.push(Track {
                    unit: *launcher,
                    weapon: Weapon::Station(station),
                    target,
                });
            }
        }
    }
    out.sort();
    out
}

/// How a unit's picture turns.
#[derive(Clone, Debug, PartialEq)]
pub enum Rule {
    /// The whole unit turns about its up axis, degrees a second.
    Body { rate_deg_s: f64 },
    /// Turrets on these hardpoints turn on the hull.
    Turrets(Vec<usize>),
}

/// A turret's or body's angle, radians: a turret's traverse from its rest,
/// a body's turn from the heading the mission or its route gives it.
/// Positive is toward the right.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct UnitAim {
    pub body: f64,
    /// By hardpoint.
    pub turrets: BTreeMap<usize, f64>,
    /// Where it stood at the last step, for telling whether it drives.
    last: Option<Vector>,
}

impl UnitAim {
    /// True while anything stands away from its rest.
    pub fn turned(&self) -> bool {
        self.body != 0. || self.turrets.values().any(|a| *a != 0.)
    }
}

/// Where a unit stands for aiming: its reference point and hull heading
/// (radians, toward +x from +z).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stance {
    pub origin: Vector,
    pub heading: f64,
}

/// `angle` in -pi..pi.
pub fn wrap(angle: f64) -> f64 {
    (angle + PI).rem_euclid(TAU) - PI
}

/// The bearing of `to` from `from`, radians toward +x from +z, relative to
/// `heading`, in -pi..pi.
pub fn relative_bearing(from: Vector, heading: f64, to: Vector) -> f64 {
    wrap((to[0] - from[0]).atan2(to[2] - from[2]) - heading)
}

/// A turret's goal for a target at `relative` (radians from the hull's
/// nose): its traverse from `rest`, kept inside `limit` either side of it
/// (0 is unrestricted).
pub fn traverse_goal(relative: f64, rest: f64, limit: f64) -> f64 {
    let from_rest = wrap(relative - rest);
    if limit > 0. {
        from_rest.clamp(-limit, limit)
    } else {
        from_rest
    }
}

/// `current` moved toward `goal` by at most `step` radians. An unrestricted
/// part (`wraps`) takes the shorter way round; a part inside an arc moves
/// straight, never through the arc's far side.
pub fn slew(current: f64, goal: f64, step: f64, wraps: bool) -> f64 {
    let error = if wraps {
        wrap(goal - current)
    } else {
        goal - current
    };
    let moved = current + error.clamp(-step, step);
    if wraps { wrap(moved) } else { moved }
}

/// Every drawn unit's aim, stepped a tick at a time.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Aims {
    pub units: BTreeMap<UnitId, UnitAim>,
    /// The tick of the last step.
    pub tick: Option<u64>,
}

impl Aims {
    pub fn unit(&self, id: UnitId) -> Option<&UnitAim> {
        self.units.get(&id)
    }

    /// Steps every unit of `rules` to `tick`. `tracks` are what its weapons
    /// hold now, `stance` where a unit stands and `target` where a target is
    /// (none: gone, and nothing is aimed at it). `arc` gives a turret its
    /// mount ([`mount_arc`]).
    pub fn step(
        &mut self,
        tick: u64,
        rules: &BTreeMap<UnitId, Rule>,
        tracks: &[Track],
        arc: &dyn Fn(UnitId, usize) -> Option<(usize, MountArc)>,
        stance: &dyn Fn(UnitId) -> Option<Stance>,
        target: &dyn Fn(u32) -> Option<Vector>,
    ) {
        let ticks = match self.tick {
            Some(last) if tick < last => {
                self.units.clear();
                None
            }
            Some(last) if tick - last <= SNAP_TICKS => Some(tick - last),
            _ => None,
        };
        self.tick = Some(tick);
        if ticks == Some(0) {
            return;
        }
        for (id, rule) in rules {
            let Some(at) = stance(*id) else {
                continue;
            };
            let held: Vec<&Track> = tracks.iter().filter(|t| t.unit == *id).collect();
            let aim = self.units.entry(*id).or_default();
            let driving = match (aim.last, ticks) {
                (Some(last), Some(n)) => {
                    let d = (0..3)
                        .map(|i| (at.origin[i] - last[i]).powi(2))
                        .sum::<f64>();
                    d.sqrt() > DRIVING_FT_PER_TICK * n as f64
                }
                _ => false,
            };
            aim.last = Some(at.origin);
            match rule {
                Rule::Body { rate_deg_s } => {
                    // The lowest weapon's target, as the unit's first weapon
                    // (a 2S6's gun before its missiles).
                    let point = held.iter().find_map(|t| target(t.target));
                    let goal = if driving {
                        Some(0.)
                    } else {
                        point.map(|p| relative_bearing(at.origin, at.heading, p))
                    };
                    if let Some(goal) = goal {
                        aim.body = match ticks {
                            Some(n) => slew(
                                aim.body,
                                goal,
                                rate_deg_s.to_radians() * n as f64 / TICKS_PER_SECOND,
                                true,
                            ),
                            None => goal,
                        };
                    }
                }
                Rule::Turrets(hardpoints) => {
                    for hardpoint in hardpoints {
                        let arc = arc(*id, *hardpoint);
                        let goal = arc.and_then(|(station, mount)| {
                            let point = held
                                .iter()
                                .filter(|t| match t.weapon {
                                    Weapon::Station(s) => s == station,
                                    Weapon::Mount(m) => m == *hardpoint,
                                })
                                .find_map(|t| target(t.target))?;
                            let [x, _, z] = mount.offset;
                            let (sin, cos) = at.heading.sin_cos();
                            let from = [
                                at.origin[0] + x * cos + z * sin,
                                at.origin[1],
                                at.origin[2] - x * sin + z * cos,
                            ];
                            Some(traverse_goal(
                                relative_bearing(from, at.heading, point),
                                mount.rest[0].to_radians(),
                                mount.limit[0].to_radians(),
                            ))
                        });
                        let wraps = arc.is_none_or(|(_, m)| m.limit[0] <= 0.);
                        let goal = goal.unwrap_or(0.);
                        let now = aim.turrets.entry(*hardpoint).or_default();
                        *now = match ticks {
                            Some(n) => slew(
                                *now,
                                goal,
                                TURRET_RATE_DEG_S.to_radians() * n as f64 / TICKS_PER_SECOND,
                                wraps,
                            ),
                            None => goal,
                        };
                    }
                }
            }
        }
    }
}

/// The weapon (by station) that fires from `hardpoint` of `arms`, and that
/// mount's arc.
pub fn mount_arc(arms: &Arms, hardpoint: usize) -> Option<(usize, MountArc)> {
    arms.weapons.iter().enumerate().find_map(|(station, w)| {
        w.mounts
            .iter()
            .find(|m| m.index == hardpoint)
            .map(|m| (station, *m))
    })
}

/// A recent burst's target as a track, while it is held: what a viewer that
/// knows only the bursts (a network client, a battery launcher in a
/// replay) aims with. `bursts` are (tick it began, unit, hardpoint, target).
pub fn burst_tracks(tick: u64, bursts: &[(u64, u32, usize, u32)]) -> Vec<Track> {
    let mut latest: BTreeMap<(u32, usize), (u64, u32)> = BTreeMap::new();
    for &(when, unit, mount, target) in bursts {
        if when <= tick && tick - when <= BURST_HOLD_TICKS {
            let entry = latest.entry((unit, mount)).or_insert((when, target));
            if when >= entry.0 {
                *entry = (when, target);
            }
        }
    }
    latest
        .into_iter()
        .map(|((unit, mount), (_, target))| Track {
            unit: UnitId(unit),
            weapon: Weapon::Mount(mount),
            target,
        })
        .collect()
}

#[cfg(test)]
#[path = "aim_tests.rs"]
mod tests;
