//! Resupply: a live friendly supply truck within 0.1 mile refills the rails
//! and magazines of every unit near it (docs/spec/surface-defenses.md,
//! "Resupply"; John, 2026-10-10).
//!
//! The world calls [`step`] before the surface tick and [`deliver`] after it:
//!
//! 1. [`step`] works out, for every armed unit that is not a ship, whether a
//!    live supply truck of its side stands within [`RESUPPLY_RADIUS_FT`]
//!    (horizontal, from the units' poses this tick, so a truck on a route is
//!    measured where it is) and sets [`SurfaceUnitState::supply`] to it. The
//!    controllers read that flag the same tick, so a destroyed truck stops
//!    resupply at once and an empty gun swaps a magazine from a truck that
//!    has just arrived. It then runs the timers and reports what completed.
//! 2. The surface tick runs ([`super::fire::step`]).
//! 3. [`deliver`] applies what completed. The reserve refill lands after the
//!    controllers' swap so a gun that draws its magazine from the truck in
//!    the same tick still has its refilled reserve: the two are independent
//!    deliveries of the same truck.
//!
//! Rules (all John's, except the per-system rearm times, which are fitted
//! inside his 5 to 10 minute range):
//!
//! - **Rails.** While a unit has an empty missile rail and a truck is in
//!   range, one rearm timer runs ([`rearm_seconds`]: 600 s for SA-2, SA-3 and
//!   HAWK, 420 s for the other vehicle launchers, 300 s for MANPADS teams).
//!   When it completes every empty rail of the unit is refilled together.
//!   The timer restarts from zero when the truck dies or leaves range, and
//!   when nothing is empty.
//! - **Magazines.** A gun's reserve holds spare magazines (two on land). With
//!   a truck in range and the reserve below that, a refill timer for the
//!   mount runs for the gun's swap period (60 s towed guns, small vehicles
//!   and troops, 120 s self-propelled and ship guns) and adds one magazine;
//!   it also restarts when the truck is lost. An empty reserve with a truck
//!   in range lets the controller swap from the truck instead of going
//!   Empty.
//! - Trucks carry unlimited stock. Tanks, APCs and troops resupply the same
//!   way as any armed unit. Ships are never resupplied.
//!
//! The timers are tick counts in [`Resupply`], kept in the unit's state and
//! checkpoint. Nothing here reads the clock, so two machines with the same
//! state agree.
use super::{
    RESUPPLY_RADIUS_FT, Surface, SurfaceState, SurfaceUnitState, Unit, UnitId, fire::Trace,
};
use std::collections::BTreeSet;
use tore_sim::{
    ai::surface::Arm,
    combat::{live, missiles::TargetRole},
};

/// Ticks a second.
const TPS: u32 = 120;
/// Rearm time of the large launchers: SA-2, SA-3 and HAWK (fitted within
/// John's range of 5 to 10 minutes).
pub const LARGE_LAUNCHER_REARM_S: u32 = 600;
/// Rearm time of the other vehicle launchers (SA-6, Roland, SA-15, SA-9,
/// SA-13, Crotale, Chaparral, 2S6, SCUD; fitted).
pub const VEHICLE_LAUNCHER_REARM_S: u32 = 420;
/// Rearm time of the shoulder-launched teams: FIM-92, Mistral, SA-7, SA-14,
/// SA-16 (fitted).
pub const MANPADS_REARM_S: u32 = 300;

/// Seconds a launcher of type `resource` (`SA6.NT`) takes to rearm from a
/// truck.
pub fn rearm_seconds(resource: &str) -> u32 {
    let stem = resource
        .rsplit_once('.')
        .map_or(resource, |(stem, _)| stem)
        .to_ascii_uppercase();
    match stem.as_str() {
        "SA2A" | "SA2" | "SA3" | "HAWK" => LARGE_LAUNCHER_REARM_S,
        "FIM92" | "MIS" | "SA7" | "SA14" | "SA16" => MANPADS_REARM_S,
        _ => VEHICLE_LAUNCHER_REARM_S,
    }
}

/// A unit's running resupply timers, in ticks since they started.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resupply {
    /// The rail rearm timer: runs while a truck is in range and a rail is
    /// empty.
    pub rearm: u32,
    /// The reserve refill timer of each hardpoint (indexed like the unit's
    /// mounts; empty until one runs).
    pub refill: Vec<u32>,
}

impl Resupply {
    /// Every timer back to zero: the truck is gone or nothing is wanted.
    pub fn stop(&mut self) {
        self.rearm = 0;
        self.refill.iter_mut().for_each(|t| *t = 0);
    }
    /// True while any timer runs.
    pub fn running(&self) -> bool {
        self.rearm > 0 || self.refill.iter().any(|t| *t > 0)
    }
}

/// What the timers completed this tick, for [`deliver`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Delivery {
    /// Units whose empty rails are refilled.
    rails: Vec<UnitId>,
    /// Guns (unit and hardpoint) that gain a spare magazine.
    magazines: Vec<(UnitId, usize)>,
}

impl Delivery {
    pub fn is_empty(&self) -> bool {
        self.rails.is_empty() && self.magazines.is_empty()
    }
}

/// Where a unit stands on the ground plane this tick: x and z, feet.
/// `surface::movement::unit_pose` gives it (a unit on a route answers from
/// its mover, a standing one from its placement).
pub type Horizontal<'a> = &'a dyn Fn(&Unit, Option<&SurfaceUnitState>) -> [f64; 2];

fn within_reach(from: [f64; 2], to: [f64; 2]) -> bool {
    let (dx, dz) = (from[0] - to[0], from[1] - to[1]);
    dx * dx + dz * dz <= RESUPPLY_RADIUS_FT * RESUPPLY_RADIUS_FT
}

/// The first half of the tick: sets every armed unit's `supply` flag, runs
/// the timers and returns what completed. Cheap in a mission with no trucks.
pub fn step(
    surface: &Surface,
    state: &mut SurfaceState,
    combat: &live::State,
    at: Horizontal<'_>,
) -> Delivery {
    let mut delivery = Delivery::default();
    if surface.trucks.is_empty() {
        return delivery;
    }
    let alive: BTreeSet<u32> = combat
        .targets
        .iter()
        .filter(|t| t.role == TargetRole::Surface && t.hp > 0)
        .map(|t| t.id)
        .collect();
    // The live trucks, where they stand now.
    let trucks: Vec<(live::Side, [f64; 2])> = surface
        .trucks
        .iter()
        .filter(|truck| alive.contains(&truck.id.0))
        .filter_map(|truck| {
            let unit = surface.unit(truck.id)?;
            Some((unit.side, at(unit, state.unit(truck.id))))
        })
        .collect();
    for arms in &surface.arsenal.units {
        // Ships have no trucks; units with nothing to refill need none.
        if arms.ship || arms.weapons.is_empty() {
            continue;
        }
        let (Some(unit), Some(now)) = (surface.unit(arms.unit), state.unit(arms.unit)) else {
            continue;
        };
        if !now.armed {
            continue;
        }
        let reach = alive.contains(&arms.unit.0) && {
            let here = at(unit, Some(now));
            trucks
                .iter()
                .any(|(side, truck)| *side == unit.side && within_reach(here, *truck))
        };
        let Some(slot) = state.unit_mut(arms.unit) else {
            continue;
        };
        slot.supply = reach;
        if !reach {
            slot.resupply.stop();
            continue;
        }
        // Rails: one timer for every empty rail of the unit.
        if slot.empty_rails(arms).is_empty() {
            slot.resupply.rearm = 0;
        } else {
            // Counted before the check, so a rearm that starts on tick S
            // completes on tick S + period, as a controller's deadline does.
            if slot.resupply.rearm >= rearm_seconds(&unit.resource) * TPS {
                slot.resupply.rearm = 0;
                delivery.rails.push(arms.unit);
            } else {
                slot.resupply.rearm += 1;
            }
        }
        // Magazines: one timer per gun mount whose reserve is below full.
        if slot.resupply.refill.len() < slot.mounts.len() {
            slot.resupply.refill.resize(slot.mounts.len(), 0);
        }
        for weapon in &arms.weapons {
            let Arm::Gun { swap, .. } = weapon.profile.arm else {
                continue;
            };
            let index = weapon.mounts[0].index;
            let wanted = match (
                arms.loads.get(index).and_then(|full| full.reserve),
                slot.mounts.get(index).and_then(|have| have.reserve),
            ) {
                (Some(full), Some(have)) => have < full,
                // A ship's unlimited reserve needs nothing.
                _ => false,
            };
            let Some(timer) = slot.resupply.refill.get_mut(index) else {
                continue;
            };
            if !wanted {
                *timer = 0;
                continue;
            }
            // The delivery tick starts the next period, so refills come
            // exactly one swap period apart and land on the tick a swap
            // started with the truck would complete.
            if u64::from(*timer) >= swap {
                *timer = 1;
                delivery.magazines.push((arms.unit, index));
            } else {
                *timer += 1;
            }
        }
    }
    delivery
}

/// The second half: applies what [`step`] completed and traces it.
pub fn deliver(surface: &Surface, state: &mut SurfaceState, delivery: Delivery) {
    for id in delivery.rails {
        let (Some(arms), Some(slot)) = (surface.arsenal.arms(id), state.unit_mut(id)) else {
            continue;
        };
        slot.refill_rails(arms);
        state.trace.push(Trace::Rearm { unit: id });
    }
    for (id, mount) in delivery.magazines {
        let (Some(arms), Some(slot)) = (surface.arsenal.arms(id), state.unit_mut(id)) else {
            continue;
        };
        let full = arms.loads.get(mount).and_then(|load| load.reserve);
        if let (Some(full), Some(stock)) = (full, slot.mounts.get_mut(mount))
            && let Some(reserve) = &mut stock.reserve
        {
            *reserve = (*reserve + 1).min(full);
        }
        state.trace.push(Trace::Refill { unit: id, mount });
    }
}
