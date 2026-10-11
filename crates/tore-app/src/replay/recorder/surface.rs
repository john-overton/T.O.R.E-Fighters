//! The surface world in a recording (format 3): what the units that follow a
//! route do, what their launchers and guns hold, who they are, and the events
//! that explain them. Everything here reads state the tick already computed.
//!
//! - **Poses**: every routed unit's `RenderSnapshot.surface` entry, each
//!   frame. Standing units are part of the rebuilt scenery and cost nothing.
//! - **Stock**: a launcher's rails (all of them, whenever one changes) and a
//!   gun's spare magazines, when a supply truck's rearm or refill, or a
//!   launch, changes them.
//! - **Pieces**: the piece a parked aircraft's debris is.
//! - **Names**: a registry of the units, written once, so the log and the text
//!   exports say "SA-6 #18" and a wreck's fire is sized by the unit's hit
//!   points.
//! - **Events**: bursts, engagement phases, resupply, radar on and off, wrecks
//!   and their fire.
//!
//! Opinionated addition requested by John on 2026-10-10; see docs/REPLAYS.md.
use super::{Tick, convert, field, kind};
use crate::snapshot::RenderSnapshot;
use std::collections::{BTreeMap, BTreeSet};
use tore_replay::{self as replay, Event};
use tore_sim::combat::live::NO_SIDE;
use tore_world::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE},
    surface::{
        LAYOUT_OBJECT_BASE, SURFACE_UNIT_BASE, SURFACE_UNIT_END, UnitId, UnitKind,
        fire::{Kind, Trace},
    },
};

/// How close a crash-site fire must stand to a wreck to be its fire.
const FIRE_NEAR_FT: f64 = 60.;

/// What the recorder remembers about the surface between ticks.
#[derive(Default)]
pub(super) struct Watch {
    /// The recording takes surface tracks (its header says format 3).
    pub on: bool,
    started: bool,
    registered: BTreeSet<u32>,
    /// Each hardpoint as last recorded: rounds loaded and spare magazines.
    stock: BTreeMap<(u32, u16), (u32, Option<u32>)>,
}

impl Watch {
    pub fn new(on: bool) -> Self {
        Self {
            on,
            ..Self::default()
        }
    }

    /// The routed units as the frame records them.
    pub fn poses(&self, snapshot: &RenderSnapshot) -> Vec<replay::SurfaceState> {
        if self.on {
            convert::surface_states(&snapshot.surface)
        } else {
            Vec::new()
        }
    }

    /// The pieces of the surface owners' debris.
    pub fn pieces(&self, snapshot: &RenderSnapshot) -> Vec<(u32, u32, u8)> {
        if self.on {
            convert::debris_pieces(&snapshot.debris)
        } else {
            Vec::new()
        }
    }

    /// Whether `id` is a unit the registry names: a template unit, an added
    /// truck or radar, a parked aircraft, or a base-layout unit that acts.
    fn named(tick: &Tick<'_>, id: u32) -> bool {
        (SURFACE_UNIT_BASE..SURFACE_UNIT_END).contains(&id)
            || (LAYOUT_OBJECT_BASE..SURFACE_UNIT_BASE).contains(&id)
                && tick
                    .world
                    .surface
                    .unit(UnitId(id))
                    .is_some_and(|unit| unit.kind == UnitKind::Active)
    }

    /// The identity of surface object `id`, if the world has a row for it.
    fn info(tick: &Tick<'_>, id: u32) -> Option<replay::SurfaceInfo> {
        let row = tick.combat.state.targets.iter().find(|t| t.id == id)?;
        let unit = tick.world.surface.unit(UnitId(id));
        let name = unit
            .map(|unit| unit.name.clone())
            .or_else(|| tick.combat.ground_name(id).map(str::to_owned))
            .unwrap_or_else(|| format!("object {id:#x}"));
        let side = unit.map_or(row.side, |unit| unit.side);
        // Template objects number from their template ordinal, added units
        // from 0, and base-layout units are marked as such.
        let label = match UnitId(id).range() {
            tore_world::surface::IdRange::Template => {
                format!("{name} #{}", id - SURFACE_UNIT_BASE)
            }
            tore_world::surface::IdRange::Layout => {
                format!("{name} L{}", id - LAYOUT_OBJECT_BASE)
            }
            _ => format!("{name} +{}", id & 0x03ff_ffff),
        };
        Some(replay::SurfaceInfo {
            id,
            name,
            label,
            side: if side == FRIENDLY_SIDE {
                replay::Side::Friendly
            } else if side == ENEMY_SIDE {
                replay::Side::Enemy
            } else if side == NO_SIDE {
                replay::Side::Neutral
            } else {
                replay::Side::Unknown
            },
            hit_points: row.initial_hp,
            position: row.position,
        })
    }

    /// The units to register before this tick's frame: on the first tick
    /// every unit the registry names, afterwards any `wanted` one it has
    /// not yet (a layout unit that fires, say).
    pub fn register(
        &mut self,
        tick: &Tick<'_>,
        wanted: impl IntoIterator<Item = u32>,
    ) -> Vec<replay::SurfaceInfo> {
        if !self.on {
            return Vec::new();
        }
        // The units this tick mentions that the registry lacks; a tick with
        // thousands of rounds in the air mostly names the same few.
        let mut ids: Vec<u32> = wanted
            .into_iter()
            .filter(|id| convert::is_surface_id(*id) && !self.registered.contains(id))
            .collect();
        if !self.started {
            self.started = true;
            ids.extend(
                tick.combat
                    .state
                    .targets
                    .iter()
                    .map(|t| t.id)
                    .filter(|id| Self::named(tick, *id)),
            );
        }
        ids.sort_unstable();
        ids.dedup();
        let mut out = Vec::new();
        for id in ids {
            if self.registered.contains(&id) {
                continue;
            }
            if let Some(info) = Self::info(tick, id) {
                self.registered.insert(id);
                out.push(info);
            }
        }
        out
    }

    /// Whether hardpoint `index` of `unit` is a missile rail.
    fn is_rail(tick: &Tick<'_>, unit: UnitId, index: usize) -> bool {
        tick.world.surface.arsenal.arms(unit).is_some_and(|arms| {
            arms.weapons
                .iter()
                .any(|w| w.kind == Kind::Missile && w.mounts.iter().any(|m| m.index == index))
        })
    }

    /// The launcher and magazine changes of the tick. A unit's first look
    /// is its starting stock, which a replay takes for full. When a rail
    /// changes, every rail of the unit is recorded, so a replay need not
    /// know the launcher's full load; a gun's reserve is recorded when it
    /// changes.
    pub fn stock(&mut self, tick: &Tick<'_>) -> Vec<replay::SurfaceStock> {
        if !self.on {
            return Vec::new();
        }
        let mut out = Vec::new();
        for unit in &tick.combat.surface.units {
            if unit.mounts.is_empty() {
                // Not armed yet, or a restart cleared it: look again.
                let mine = (unit.id.0, 0)..=(unit.id.0, u16::MAX);
                let known: Vec<_> = self.stock.range(mine).map(|(key, _)| *key).collect();
                for key in known {
                    self.stock.remove(&key);
                }
                continue;
            }
            let mut rail_changed = false;
            let mut changes = Vec::new();
            for (index, mount) in unit.mounts.iter().enumerate() {
                let key = (unit.id.0, index as u16);
                let now = (mount.loaded, mount.reserve);
                let Some(before) = self.stock.insert(key, now) else {
                    continue;
                };
                if Self::is_rail(tick, unit.id, index) {
                    rail_changed |= before.0 != now.0;
                } else if before.1 != now.1 {
                    changes.push(replay::SurfaceStock {
                        unit: unit.id.0,
                        mount: index as u16,
                        loaded: mount.loaded,
                        reserve: mount.reserve,
                    });
                }
            }
            if rail_changed {
                for (index, mount) in unit.mounts.iter().enumerate() {
                    if Self::is_rail(tick, unit.id, index) {
                        changes.push(replay::SurfaceStock {
                            unit: unit.id.0,
                            mount: index as u16,
                            loaded: mount.loaded,
                            reserve: None,
                        });
                    }
                }
            }
            changes.sort_by_key(|stock| (stock.unit, stock.mount));
            out.extend(changes);
        }
        out
    }

    /// The events of the tick: bursts, phases, resupply, radars (from the
    /// controllers' trace) and the wrecks of `lost`, the surface rows whose hit
    /// points ran out this tick with what they had.
    pub fn events(&mut self, tick: &Tick<'_>, lost: &[(u32, i32)], events: &mut Vec<Event>) {
        if !self.on {
            return;
        }
        let surface = &tick.combat.surface;
        let rails = |unit: UnitId| -> u32 {
            surface.unit(unit).map_or(0, |state| {
                state
                    .mounts
                    .iter()
                    .enumerate()
                    .filter(|(index, _)| Self::is_rail(tick, unit, *index))
                    .map(|(_, mount)| mount.loaded)
                    .sum()
            })
        };
        for trace in &surface.trace {
            match trace {
                Trace::Shot {
                    unit,
                    mount,
                    record,
                    target,
                    rounds,
                    refused,
                    opening,
                    flak,
                } => events.push(
                    Event::new(kind::SURFACE_BURST)
                        .with_subject(unit.0)
                        .with_object(*target)
                        .with(field::WEAPON, record.as_str())
                        .with(field::MOUNT, *mount as i64)
                        .with(field::ROUNDS, i64::from(*rounds))
                        .with(field::REFUSED, i64::from(*refused))
                        .with(field::OPENING, *opening)
                        .with(field::FLAK, *flak),
                ),
                Trace::Phase {
                    unit,
                    weapon,
                    phase,
                    target,
                } => {
                    let mut event = Event::new(kind::SURFACE_PHASE)
                        .with_subject(unit.0)
                        .with(field::STATION, *weapon as i64)
                        .with(field::TO, format!("{phase:?}").to_lowercase());
                    if let Some(target) = target {
                        event = event.with_object(*target);
                    }
                    events.push(event);
                }
                Trace::Rearm { unit } => events.push(
                    Event::new(kind::SURFACE_REARM)
                        .with_subject(unit.0)
                        .with(field::LOADED, i64::from(rails(*unit))),
                ),
                Trace::Refill { unit, mount } => {
                    let reserve = surface
                        .unit(*unit)
                        .and_then(|state| state.mounts.get(*mount))
                        .and_then(|stock| stock.reserve);
                    let mut event = Event::new(kind::SURFACE_REFILL)
                        .with_subject(unit.0)
                        .with(field::MOUNT, *mount as i64);
                    if let Some(reserve) = reserve {
                        event = event.with(field::RESERVE, i64::from(reserve));
                    }
                    events.push(event);
                }
                Trace::Radar { unit, on } => events.push(
                    Event::new(kind::SURFACE_RADAR)
                        .with_subject(unit.0)
                        .with(field::ON, *on)
                        .with(
                            field::REASON,
                            if *on {
                                "a hostile came in range"
                            } else {
                                "no hostile in range for 30 seconds"
                            },
                        ),
                ),
                Trace::Shutdown {
                    unit,
                    missile,
                    rolled,
                } => events.push(
                    Event::new(kind::SURFACE_RADAR)
                        .with_subject(unit.0)
                        .with(field::ON, false)
                        .with(field::PROJECTILE, replay::Value::Id(*missile))
                        .with(
                            field::REASON,
                            if *rolled {
                                "its crew shut it down before an anti-radiation missile arrived"
                            } else {
                                "an anti-radiation missile was inbound"
                            },
                        ),
                ),
                Trace::Swap { .. } | Trace::Battery { .. } => {}
            }
        }
        for (id, before) in lost {
            if !self.registered.contains(id) {
                continue;
            }
            let Some(row) = tick.combat.state.targets.iter().find(|t| t.id == *id) else {
                continue;
            };
            let burning = tick.combat.state.marks.iter().any(|mark| {
                mark.kind == tore_sim::combat::blast::MarkKind::Fire
                    && (mark.position[0] - row.position[0])
                        .hypot(mark.position[2] - row.position[2])
                        <= FIRE_NEAR_FT
            });
            let mut event = Event::new(kind::SURFACE_WRECK)
                .with_subject(*id)
                .with(field::HP_BEFORE, i64::from(*before))
                .with(field::BURNING, burning);
            if burning {
                event = event.with(
                    field::FIRE_FT,
                    crate::surface_fx::fire_width(row.initial_hp),
                );
            }
            if let Some(killer) = tick.combat.state.ledger.credit(*id) {
                event = event.with_object(killer.owner);
            }
            events.push(event);
        }
    }
}

/// "a" or "an" for a unit's name read aloud: designations by their first
/// letter's name (an SA-6, an M1939, an F-4; a ZSU-23, a KS-19), words by their
/// first sound (a HAWK, an Oerlikon).
pub(super) fn article(name: &str) -> &'static str {
    let first = name.chars().next().unwrap_or(' ');
    let acronym = first.is_ascii_uppercase()
        && (name.len() <= 3 || name.chars().any(|c| c.is_ascii_digit() || c == '-'));
    let vowel = if acronym {
        "AEFHILMNORSX".contains(first)
    } else {
        "AEIOUaeiou".contains(first)
    };
    if vowel { "an" } else { "a" }
}

#[cfg(test)]
mod tests {
    use super::article;

    #[test]
    fn units_are_named_aloud_with_the_right_article() {
        for (name, expected) in [
            ("SA-6", "an SA-6"),
            ("SA-2", "an SA-2"),
            ("ZSU-23-4", "a ZSU-23-4"),
            ("KS-19", "a KS-19"),
            ("M1939", "an M1939"),
            ("HAWK", "a HAWK"),
            ("T-80", "a T-80"),
            ("Tank", "a Tank"),
            ("Oerlikon", "an Oerlikon"),
            ("MIM-23", "an MIM-23"),
            ("F-5", "an F-5"),
        ] {
            assert_eq!(format!("{} {name}", article(name)), expected, "{name}");
        }
    }
}
