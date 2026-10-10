//! Splash (collateral) damage: every weapon whose record carries a
//! collateral radius and percent hurts what stands near the point it goes
//! off (docs/spec/missiles.md, "Splash damage").
//!
//! The rule, from the original's collateral routine (docs/formats/weapons.md,
//! "Collateral damage"):
//!
//! - Everything alive and solid within the radius takes a share of the
//!   record's damage for its class: aircraft, ground objects, ships and parked
//!   aircraft. Distance is measured to the target's surface (its box, or its
//!   sphere for an aircraft), so a large building next to the blast is
//!   reached as soon as its near wall is.
//! - The share falls off in a straight line, from the record's percent at
//!   the surface of the blast to nothing at the radius edge.
//! - A detonation in the air or on an aircraft passes the share on whole; a
//!   detonation on the ground passes on 70 percent of it for a weapon whose
//!   percent is 100 (the bombs) and half for any other.
//! - The target a round struck directly takes none (it already took the full
//!   hit). With friendly fire off, nothing of the shooter's side takes any,
//!   the shooter included. With it on the shooter can be caught in its own
//!   blast, as the manual warns (p. 126: a missile fired inside its minimum
//!   range or a bomb dropped too low); that damage credits no one.
//! - A surface unit's round reaches aircraft only (surface units do not
//!   fight each other this round, plan W2).
//!
//! Kill credit, strikes, hit records and jolts follow the shooter exactly as
//! a direct hit's do. Collateral never damages a projectile.
use super::{
    DamageSection, Event, HitRecord, INCOMING_OWNER, LocalizedDamage, MAX_HIT_RECORDS, NO_SIDE,
    OwnRow, Projectile, Side, State, Strike, damage_class, draw, sub,
};
use crate::airport::OrientedBox;
use crate::attitude::{Basis, Vector, dot};
use crate::combat::ledger::{Kill, Resolution};
use crate::combat::live::EffectKind;
use crate::combat::missiles::TargetRole;
use tore_formats::weapons::Weapon;

/// Where a round went off: in the air (or on an aircraft) its collateral
/// share passes on whole, on the ground in part ([`ground_share`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Detonation {
    /// A flak burst, a proximity burst, a round striking an aircraft.
    Air,
    /// A round striking the terrain, the sea, or a ground object or ship.
    Ground,
}

/// The percent of a record's collateral share a ground detonation passes
/// on: 70 for a weapon whose collateral percent is 100 (every bomb), 50 for
/// any other (the original's collateral call on ground impact).
pub fn ground_share(collateral_percent: i16) -> i32 {
    if collateral_percent >= 100 { 70 } else { 50 }
}

/// Who fired a burst's round, for whom it spares and whom it credits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Shooter {
    /// The shooter's side ([`NO_SIDE`] when friendly fire is on or the side
    /// is unknown): friendly fire off spares it.
    pub side: Side,
    /// A surface unit's round: its collateral reaches aircraft only.
    pub surface: bool,
    /// An ownship's round: a collateral kill moves its score.
    pub ownship: bool,
}

/// One round's detonation, applied after the round search of a step.
#[derive(Clone, Copy, Debug)]
pub(super) struct Burst {
    pub projectile: u32,
    pub owner: u32,
    pub shooter: Shooter,
    pub station: usize,
    pub position: Vector,
    pub damage: [i16; 5],
    pub radius: f64,
    pub percent: i32,
    /// [`Detonation::Air`] passes 100 percent of the share on, a ground
    /// detonation [`ground_share`].
    pub share: i32,
    pub flags: u32,
    /// The target the round struck directly, which takes no collateral.
    pub exclude: Option<u32>,
    /// The burst is the round's outcome (a flak shell that struck nothing):
    /// the ledger records it as a hit if it damaged anyone.
    pub resolves: bool,
}
impl Burst {
    pub fn new(
        p: &Projectile,
        w: &Weapon,
        shooter: Shooter,
        position: Vector,
        detonation: Detonation,
        exclude: Option<u32>,
        resolves: bool,
    ) -> Self {
        Self {
            projectile: p.id,
            owner: p.owner,
            shooter,
            station: p.station,
            position,
            damage: w.damage.by_class,
            radius: f64::from(w.damage.collateral_radius.max(0)),
            percent: i32::from(w.damage.collateral_percent.max(0)),
            share: match detonation {
                Detonation::Air => 100,
                Detonation::Ground => ground_share(w.damage.collateral_percent),
            },
            flags: w.flags,
            exclude,
            resolves,
        }
    }
    /// Whether the burst reaches anyone at all.
    pub fn collateral(&self) -> bool {
        self.radius > 0. && self.percent > 0
    }
    /// The percent of the record's damage a target whose surface is
    /// `distance` feet from the burst takes: the record's percent at the
    /// burst falling in a straight line to nothing at the radius, times the
    /// detonation's share, in whole feet and whole percent.
    pub fn percent_at(&self, distance: f64) -> i32 {
        let radius = self.radius as i64;
        let distance = distance.max(0.).floor() as i64;
        if radius <= 0 || distance >= radius {
            return 0;
        }
        let percent = i64::from(self.percent);
        ((percent - percent * distance / radius) * i64::from(self.share) / 100) as i32
    }
    /// The damage a target of `category` whose surface is `distance` feet
    /// away takes, before an ownship's damage roll.
    fn nominal(&self, category: u16, distance: f64) -> i32 {
        i32::from(self.damage[damage_class(category)]).max(0) * self.percent_at(distance) / 100
    }
}

/// Feet from `point` to the surface of `bounds` (0 inside it).
pub fn box_distance(bounds: &OrientedBox, point: Vector) -> f64 {
    let basis = Basis::new(bounds.heading, bounds.pitch, bounds.bank);
    let delta = sub(point, bounds.center);
    let local = [
        dot(delta, basis.right),
        dot(delta, basis.up),
        dot(delta, basis.forward),
    ];
    let outside: Vector = std::array::from_fn(|i| (local[i].abs() - bounds.half[i]).max(0.));
    dot(outside, outside).sqrt()
}

/// Feet from `point` to the surface of a sphere of `radius` around `center`
/// (0 inside it).
fn sphere_distance(center: Vector, radius: f64, point: Vector) -> f64 {
    let delta = sub(center, point);
    (dot(delta, delta).sqrt() - radius.max(0.)).max(0.)
}

/// Where a ground object stands: the middle of the bottom of its box.
fn foot(bounds: &OrientedBox) -> Vector {
    let up = Basis::new(bounds.heading, bounds.pitch, bounds.bank).up;
    std::array::from_fn(|i| bounds.center[i] - up[i] * bounds.half[1])
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
    /// Score changes for ownship shooters: owner and whether it is a kill.
    pub scored: &'a mut Vec<(u32, bool)>,
}

impl State {
    /// Applies one burst's collateral damage to everything in its radius:
    /// target rows at once, ownships through the step's ownship hits.
    /// `water` says whether a point lies over the sea (a ground object
    /// destroyed there leaves no crater).
    pub(super) fn apply_burst(
        &mut self,
        burst: &Burst,
        rows: &[OwnRow],
        friendly_fire_off: bool,
        water: &dyn Fn(f64, f64) -> bool,
        out: Outputs<'_>,
    ) {
        let spares = |side: Side| {
            friendly_fire_off && burst.shooter.side != NO_SIDE && side == burst.shooter.side
        };
        let skipped = |id: u32, hp: i32, present: bool, side: Side| {
            Some(id) == burst.exclude || hp <= 0 || !present || spares(side)
        };
        let mut total: u32 = 0;
        if burst.collateral() {
            for (n, r) in rows.iter().enumerate() {
                let t = &r.target;
                if skipped(t.id, t.hp, t.body_present(), t.side) {
                    continue;
                }
                let base = burst.nominal(
                    t.category,
                    sphere_distance(t.position, t.radius, burst.position),
                );
                if base <= 0 {
                    continue;
                }
                let amount = super::super::systems::damage_amount(
                    u16::try_from(base).unwrap_or(u16::MAX),
                    100,
                    draw(&mut self.rng, 40) as u8,
                );
                let section = LocalizedDamage::section(burst.position, t);
                // Caught in its own blast: the damage is credited to no one.
                let owner = if t.id == burst.owner {
                    INCOMING_OWNER
                } else {
                    burst.owner
                };
                out.ownship_hits
                    .push((n, amount, section, false, owner, burst.flags));
                out.events.push(Event::Jolt(super::Jolt {
                    target: t.id,
                    from: burst.position,
                    strength: f64::from(base) / 100.,
                }));
                total = total.saturating_add(u32::try_from(amount).unwrap_or(0));
            }
            let State {
                targets,
                ground_bounds,
                ground_looks,
                rng,
                history,
                ledger,
                tick,
                ..
            } = self;
            for t in targets.iter_mut() {
                let aircraft = t.role == TargetRole::Aircraft;
                if (burst.shooter.surface && !aircraft)
                    || skipped(t.id, t.hp, t.body_present(), t.side)
                {
                    continue;
                }
                let bounds = ground_bounds.get(&t.id);
                let distance = bounds.map_or_else(
                    || sphere_distance(t.position, t.radius, burst.position),
                    |bounds| box_distance(bounds, burst.position),
                );
                let nominal = burst.nominal(t.category, distance);
                if nominal <= 0 {
                    continue;
                }
                if aircraft {
                    out.events.push(Event::Jolt(super::Jolt {
                        target: t.id,
                        from: burst.position,
                        strength: f64::from(nominal) / 100.,
                    }));
                }
                let applied = nominal.min(t.hp);
                t.hp -= applied;
                if aircraft {
                    let section = LocalizedDamage::section(burst.position, t);
                    t.localized_damage.record(section, nominal, t.initial_hp);
                    if t.hp > 0 {
                        t.faults.hit(nominal, t.initial_hp, |n| draw(rng, n));
                    }
                }
                if history.len() == MAX_HIT_RECORDS {
                    history.remove(0);
                }
                history.push(HitRecord {
                    tick: *tick,
                    target: t.id,
                    station: burst.station,
                    class: damage_class(t.category),
                    nominal,
                    applied,
                    hp_after: t.hp,
                });
                // Caught in its own blast: no credit and no strike.
                let credited = t.id != burst.owner;
                let credit = Kill {
                    owner: burst.owner,
                    victim: t.id,
                    category: t.category,
                    aircraft,
                };
                if applied > 0 && credited {
                    ledger.damaged(credit);
                }
                out.events.push(Event::Hit(t.id));
                if credited {
                    out.strikes.push(Strike {
                        owner: burst.owner,
                        victim: t.id,
                        weapon_flags: burst.flags,
                        destroyed: t.hp == 0,
                        amount: applied,
                    });
                }
                if t.hp == 0 {
                    if credited {
                        ledger.kill(credit);
                        if burst.shooter.ownship {
                            out.scored.push((burst.owner, true));
                        }
                    }
                    out.events.push(Event::Destroyed(t.id));
                    // An aircraft explodes as aircraft do; a ground object as
                    // its unit record says, where it stands, leaving its
                    // crater on land.
                    let (position, explosion, crater) = match bounds {
                        Some(bounds) if !aircraft => {
                            let at = foot(bounds);
                            match ground_looks.get(&t.id) {
                                Some(look) => (
                                    at,
                                    crate::combat::blast::ground_object(Some(look.explosion)),
                                    if water(at[0], at[2]) { 0 } else { look.crater },
                                ),
                                None => (at, crate::combat::blast::GROUND_OBJECT, 0),
                            }
                        }
                        _ if aircraft => (t.position, crate::combat::blast::AIRCRAFT, 0),
                        _ => (t.position, crate::combat::blast::GROUND_OBJECT, 0),
                    };
                    out.impacts
                        .push((position, EffectKind::Destroyed, explosion, crater));
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
#[path = "collateral_tests.rs"]
mod tests;
