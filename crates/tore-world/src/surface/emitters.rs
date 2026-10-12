//! Surface radars: who emits, when a radar is on, and HARM shutdowns
//! (docs/spec/surface-defenses.md, "RWR emitters and radar state" and "SAM
//! batteries").
//!
//! - A battery's radar element is the battery's only emitter; its launchers
//!   never emit.
//! - Any other unit with a radar weapon (a radar-seeker missile, or a
//!   radar-directed gun) or a sensor (GCI, the Red Crown picket) emits while
//!   its radar is on.
//! - The named radar vehicles and the Tall King (LTRACK, SFLUSH, SRDR1,
//!   SRDR2, KING) emit whenever they stand, unless a battery adopted them.
//! - A radar is on while a hostile aircraft is inside its detection range and
//!   for 30 s after the last one leaves (fitted); a unit whose radar weapon
//!   aims at ships (the Sarancha's SS-N-9) also while a ship of the other
//!   side is inside that weapon's detection range, so it can support the
//!   missile (agent decision, 2026-10-11). When an anti-radiation
//!   missile (AGM-88, AGM-45) aimed at it comes within 10 nm, it rolls its
//!   experience's shutdown chance once; on success it goes off for 30 s.
//!
//! The radar's state is written to its combat row (`Target::radar_emitting`),
//! which is all the RWR's passive reception and the HARM's seeker need.
use super::{
    Surface, SurfaceState, UnitId,
    fire::{Arms, Band, Contact, Kind, Scene, Trace},
};
use std::collections::BTreeMap;
use tore_sim::combat::{
    live,
    missiles::{self, Guidance},
};

/// A radar goes off this long after the last hostile leaves its range, and
/// stays off this long after a HARM shutdown (fitted).
pub const RADAR_HOLD_TICKS: u64 = 30 * 120;
/// An anti-radiation missile this close sets off the shutdown roll: 10 nm.
pub const HARM_ROLL_RANGE_FT: f64 = 10. * 6_076.;

/// The named radar objects that carry no weapon or sensor but are radars by
/// name (defined, agent): they emit while they stand. PRDR1 and PRDR2
/// ("Passive Radar") and the microwave relays never do.
pub const NAMED_RADARS: [&str; 5] = ["LTRACK.NT", "SFLUSH.NT", "SRDR1.NT", "SRDR2.NT", "KING.OT"];

/// Whether `resource` is one of the [`NAMED_RADARS`].
pub fn named_radar(resource: &str) -> bool {
    NAMED_RADARS
        .iter()
        .any(|name| name.eq_ignore_ascii_case(resource))
}

/// How a unit's radar emits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RadarRule {
    /// On while a hostile aircraft is within `range` feet, or a ground unit
    /// or ship its radar weapons aim at within `surface` feet (0: none), and
    /// 30 s after.
    Detection { range: f64, surface: f64 },
    /// A named radar: on whenever it stands.
    Always,
    /// A battery's radar element: on while a hostile is inside the battery
    /// missile's detection range or its own sensor's (`sensor`).
    Battery { sensor: f64 },
}

impl RadarRule {
    /// The range its radar looks out to, feet (0 for an always-on one).
    pub fn range(&self) -> f64 {
        match *self {
            RadarRule::Detection { range, .. } => range,
            RadarRule::Always => 0.,
            RadarRule::Battery { sensor } => sensor,
        }
    }
}

/// The radar rule of a unit: `battery` is `Some(true)` for a battery's radar
/// element, `Some(false)` for one of its launchers.
pub fn rule(
    arms: &Arms,
    named: bool,
    sensor: Option<Band>,
    battery: Option<bool>,
) -> Option<RadarRule> {
    match battery {
        Some(true) => {
            return Some(RadarRule::Battery {
                sensor: sensor.map_or(0., |band| band.max_range),
            });
        }
        Some(false) => return None,
        None => {}
    }
    let radar_weapons = || {
        arms.weapons.iter().filter(|w| match w.kind {
            Kind::Missile => w.record.seeker.signature == 3,
            Kind::Gun { radar, .. } => radar,
            Kind::Barrage { .. } => false,
        })
    };
    // The air search reaches as far as the radar weapons that aim at
    // aircraft (and the sensor); the surface search as far as those that
    // aim at ground units or ships.
    let range = radar_weapons()
        .filter(|w| w.aims_at_aircraft())
        .map(|w| w.detection.max_range)
        .chain(sensor.map(|band| band.max_range))
        .fold(None, |best: Option<f64>, range| {
            Some(best.map_or(range, |b| b.max(range)))
        });
    let surface = radar_weapons()
        // A land AAA gun that fires at the ground by John's ruling aims by
        // eye: its air-search radar stays off for ground units.
        .filter(|w| w.aims_at_any_surface() && !w.ground_fire)
        .map(|w| w.detection.max_range)
        .fold(None, |best: Option<f64>, range| {
            Some(best.map_or(range, |b| b.max(range)))
        });
    match (range, surface) {
        (None, None) if named => Some(RadarRule::Always),
        (None, None) => None,
        (range, surface) => Some(RadarRule::Detection {
            range: range.unwrap_or(0.),
            surface: surface.unwrap_or(0.),
        }),
    }
}

/// The radars' tick: each radar unit's on and off, HARM shutdown rolls, and
/// its combat row's `radar_emitting`. `ground` is the surface units a
/// controller may aim at this tick. Returns which units emit this tick.
pub(super) fn step(
    surface: &Surface,
    state: &mut SurfaceState,
    live: &mut live::State,
    scene: &Scene<'_>,
    rows: &BTreeMap<u32, usize>,
    ground: &[Contact],
) -> BTreeMap<UnitId, bool> {
    let tick = scene.tick;
    let arsenal = &surface.arsenal;
    // Anti-radiation missiles in flight, by the emitter they home on.
    let mut homing: BTreeMap<u32, Vec<(u32, [f64; 3])>> = BTreeMap::new();
    for projectile in &live.projectiles {
        let Some(target) = projectile.target else {
            continue;
        };
        if missiles::Profile::for_weapon(live.weapon(projectile))
            .is_some_and(|p| p.guidance == Guidance::Emitter)
        {
            homing
                .entry(target)
                .or_default()
                .push((projectile.id, projectile.position));
        }
    }
    state
        .harm_rolled
        .retain(|id| live.projectiles.iter().any(|p| p.id == *id));
    let mut on = BTreeMap::new();
    for arms in &arsenal.units {
        let Some(rule) = arms.radar else {
            continue;
        };
        let row = rows.get(&arms.unit.0).copied();
        let alive = row.is_some_and(|index| live.targets[index].hp > 0);
        let range = match rule {
            RadarRule::Battery { sensor } => arsenal
                .batteries
                .iter()
                .find(|b| b.radar == arms.unit)
                .map_or(sensor, |b| b.weapon.detection.max_range.max(sensor)),
            other => other.range(),
        };
        let from = state.place(arms).eye;
        let hostile = alive
            && scene.aircraft.iter().any(|aircraft| {
                arms.side != live::NO_SIDE
                    && aircraft.side != live::NO_SIDE
                    && aircraft.side != arms.side
                    && aircraft.airborne
                    && {
                        let d = [
                            aircraft.position[0] - from[0],
                            aircraft.position[1] - from[1],
                            aircraft.position[2] - from[2],
                        ];
                        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() <= range
                    }
            });
        // A radar weapon that aims at ground units or ships looks for them too.
        let hostile = hostile
            || match rule {
                RadarRule::Detection { surface: reach, .. } if alive && reach > 0. => {
                    ground.iter().any(|contact| {
                        live::opposed(arms.side, contact.side)
                            && arms.weapons.iter().any(|w| {
                                w.aims_at_surface(contact.category)
                                    && !w.ground_fire
                                    && w.detection.max_range > 0.
                            })
                            && {
                                let d = [
                                    contact.position[0] - from[0],
                                    contact.position[1] - from[1],
                                    contact.position[2] - from[2],
                                ];
                                (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() <= reach
                            }
                    })
                }
                _ => false,
            };
        let skill = arms.skill;
        let Some(unit) = state.unit_mut(arms.unit) else {
            continue;
        };
        let radar = &mut unit.radar;
        if hostile {
            radar.last_hostile = Some(tick);
        }
        let shut = radar.shutdown_until.is_some_and(|until| tick < until);
        let was = radar.on;
        let mut now = alive
            && !shut
            && (rule == RadarRule::Always
                || radar
                    .last_hostile
                    .is_some_and(|last| tick < last + RADAR_HOLD_TICKS));
        // A HARM within 10 nm: one shutdown roll per missile while it emits.
        if now && let Some(missiles) = homing.get(&arms.unit.0) {
            for (missile, position) in missiles {
                let d = [
                    position[0] - from[0],
                    position[1] - from[1],
                    position[2] - from[2],
                ];
                if (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt() > HARM_ROLL_RANGE_FT
                    || !state.harm_rolled.insert(*missile)
                {
                    continue;
                }
                let roll = super::fire::draw(&mut state.rng) % 100;
                let shuts = roll < u64::from(tore_sim::ai::surface::harm_shutdown_percent(skill));
                state.trace.push(Trace::Shutdown {
                    unit: arms.unit,
                    missile: *missile,
                    rolled: shuts,
                });
                if shuts {
                    let unit = state.unit_mut(arms.unit).expect("checked above");
                    unit.radar.shutdown_until = Some(tick + RADAR_HOLD_TICKS);
                    now = false;
                    break;
                }
            }
        }
        let unit = state.unit_mut(arms.unit).expect("checked above");
        unit.radar.on = now;
        if now != was {
            state.trace.push(Trace::Radar {
                unit: arms.unit,
                on: now,
            });
        }
        if let Some(index) = row {
            live.targets[index].radar_emitting = now;
        }
        on.insert(arms.unit, now);
    }
    on
}
