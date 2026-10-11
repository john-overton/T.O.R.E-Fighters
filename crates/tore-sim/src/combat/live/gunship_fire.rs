//! The AI gunship's linked guns (docs/spec/ai-strike.md, "AC-130"; slice
//! ST-F): how an AI-flown AC-130 trains each gun on its aim point and fires
//! it, through the one release a human's trigger takes
//! ([`State::release_round`]).
//!
//! Each gun is trained the way the player's gunsight trains it
//! ([`super::super::gunship::State::update`]): the aim point is led by the
//! round's time of flight and raised by its drop, as the gun's own
//! ballistics give them ([`gunsight::solve_observed`]), from the gun's
//! muzzle; then the train is walked until the gunsight's pipper (the march
//! of the trajectory the simulation flies) sits on the aim, as a gunner
//! puts the pipper on the target. This replaces retail's AI aim, 20 ft above the target with no
//! drop (docs/formats/ai.md, "AC-130 guns on the orbit"). A gun fires only
//! when that train lies inside its mount's arcs (half-widths about the
//! left-facing neutral: 25 mm 60 and 60 degrees, 40 mm 45 and 45, 105 mm 25
//! in heading and 45 in elevation), the aim lies inside its zone-1 range,
//! its own airframe is clear and the terrain leaves the line open. The AI
//! trains its guns at once instead of slewing them at 30 degrees a second:
//! on the orbit the target moves through the arcs far slower than that
//! (fitted, agent).
use super::cockpit::{Round, StrikeCockpit};
use super::{Configuration, Launcher, MAX_PROJECTILES, NO_SIDE, State, terrain_hit};
use crate::attitude::{Vector, dot};
use crate::combat::delivery::DeliveryMode;
use crate::combat::gunship::{self, ELEVATION_ARC, HEADING_ARC};
use crate::combat::gunship_impact::{self, Impact, Shot};
use crate::combat::gunsight::{self, TargetObservation};
use crate::combat::laser::Laser;
use crate::combat::missiles::LaunchMode;
use crate::combat::sight::{short_of, world_mount};
use std::f64::consts::FRAC_PI_2;
use tore_formats::weapons::Weapon;

/// A gun's train for the AI: barrel heading and elevation in the aircraft's
/// frame, radians (heading -90 degrees is abeam left), and the shot's time
/// of flight and range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunTrain {
    pub heading: f64,
    pub elevation: f64,
    pub seconds: f64,
    pub range_ft: f64,
}

/// Why a gun does not bear on the aim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoShot {
    /// The ballistics give no solution (beyond the round's flight).
    Solution,
    /// The train lies outside the mount's arcs.
    Arc,
    /// The aim lies outside the gun's zone-1 range.
    Range,
    /// The gun would fire into its own airframe.
    Airframe,
    /// The terrain blocks the line.
    Terrain,
}

/// The pipper is walked onto the aim only from within this far of it
/// (fitted, agent): farther off, the round meets the terrain short of it.
const WALK_LIMIT_FT: f64 = 500.;

/// The train of gun `slot` (0 the 25 mm, 1 the 40 mm, 2 the 105 mm) on
/// `aim` from `launcher`, with the gun's own ballistics, or why it does not
/// bear (see the module notes).
pub fn ai_gun_train(
    slot: usize,
    weapon: &Weapon,
    launcher: &Launcher,
    aim: TargetObservation,
    launch_tick: u64,
    ground: &impl Fn(f64, f64) -> f64,
) -> Result<GunTrain, NoShot> {
    // The gunsight's lead and drop give the train; the pipper's march (the
    // trajectory the simulation flies, [`gunship_impact::impact`]) then
    // walks it until the pipper sits on the aim, as a gunner puts the pipper
    // on the target.
    let mut goal = aim;
    let (mut heading, mut elevation, mut solution) = train_on(slot, weapon, launcher, goal)?;
    for _ in 0..3 {
        let Some(Impact::Ground { point, seconds, .. }) = gunship_impact::impact(
            slot,
            weapon,
            launcher,
            heading,
            elevation,
            Shot {
                target: None,
                launch_tick,
            },
            ground,
        ) else {
            break;
        };
        let want = [
            aim.position[0] + aim.velocity[0] * seconds,
            aim.position[2] + aim.velocity[2] * seconds,
        ];
        let error = [want[0] - point[0], want[1] - point[2]];
        // Settled, or the round meets something else first (a ridge on
        // the line): no walking the pipper there.
        let off = error[0].hypot(error[1]);
        if !(1. ..=WALK_LIMIT_FT).contains(&off) {
            break;
        }
        goal.position[0] += error[0];
        goal.position[2] += error[1];
        (heading, elevation, solution) = train_on(slot, weapon, launcher, goal)?;
    }
    let off = heading + FRAC_PI_2;
    let off = (off + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    if off.abs() > HEADING_ARC[slot] || elevation.abs() > ELEVATION_ARC[slot] {
        return Err(NoShot::Arc);
    }
    let zone = weapon.seeker.zones[1];
    if solution.range_ft > solution.maximum_range_ft
        || solution.range_ft < f64::from(zone.minimum_range.max(0))
    {
        return Err(NoShot::Range);
    }
    if !gunship::clear_airframe(slot, heading, elevation) {
        return Err(NoShot::Airframe);
    }
    let muzzle = gunship::muzzle(slot, *launcher, heading, elevation);
    if terrain_hit(muzzle, short_of(muzzle, aim.position), ground).is_some() {
        return Err(NoShot::Terrain);
    }
    Ok(GunTrain {
        heading,
        elevation,
        seconds: solution.seconds,
        range_ft: solution.range_ft,
    })
}

/// The train that the gunsight's lead and drop put on `aim`, with its
/// solution (the muzzle moves with the train: two passes settle it).
fn train_on(
    slot: usize,
    weapon: &Weapon,
    launcher: &Launcher,
    aim: TargetObservation,
) -> Result<(f64, f64, gunsight::Solution), NoShot> {
    let pivot = world_mount(*launcher, gunship::pivot(slot));
    let (mut heading, mut elevation) = (-FRAC_PI_2, 0.);
    let mut solution = None;
    for _ in 0..2 {
        let mount = gunship::local_muzzle(slot, heading, elevation);
        let found = gunsight::solve_observed(weapon, launcher, mount, Some(aim))
            .ok()
            .flatten()
            .ok_or(NoShot::Solution)?;
        let toward: Vector = std::array::from_fn(|i| {
            aim.position[i]
                + aim.velocity[i] * found.seconds
                + if i == 1 { found.drop_ft } else { 0. }
                - pivot[i]
        });
        let right = dot(toward, launcher.basis.right);
        let forward = dot(toward, launcher.basis.forward);
        let up = dot(toward, launcher.basis.up);
        heading = right.atan2(forward);
        elevation = up.atan2(right.hypot(forward));
        solution = Some(found);
    }
    let solution = solution.ok_or(NoShot::Solution)?;
    Ok((heading, elevation, solution))
}

/// Whether a held gun fires a round on `tick`: the record's cadence (a
/// burst's span over its physical rounds; 4, 72 and 720 ticks for the
/// AC-130's 25, 40 and 105 mm), counted from tick 0 so no gun ever beats
/// its rate.
pub fn ai_gun_due(weapon: &Weapon, tick: u64) -> bool {
    let span = crate::combat::gun_round::release_tick(weapon, 0, 1).max(1);
    tick.is_multiple_of(span)
}

/// A gun slower than two rounds a second (the AC-130's 40 and 105 mm):
/// the AI fires it every round it is due rather than in bursts (fitted,
/// agent).
pub fn ai_gun_slow(weapon: &Weapon) -> bool {
    crate::combat::gun_round::release_tick(weapon, 0, 1) >= 60
}

/// One AI gunship round: everything [`State::release_ai_gun_round`] needs.
pub struct AiGunRound<'a> {
    /// The projectile number.
    pub id: u32,
    pub aircraft: u32,
    pub config: &'a Configuration,
    /// The station and the gun's slot.
    pub station: usize,
    pub slot: usize,
    pub launcher: Launcher,
    pub train: GunTrain,
    /// The round's place among the gun's rounds (its burst position and
    /// tracer).
    pub ordinal: u64,
}

impl State {
    /// Fires one round of an AI gunship's gun along `round.train` through
    /// the shared release, as a human's trigger does: from the muzzle, at
    /// the muzzle speed, carrying the gun's record. The caller has debited
    /// the round. False when there is no room or no such station.
    pub fn release_ai_gun_round(
        &mut self,
        round: AiGunRound<'_>,
        ground: &impl Fn(f64, f64) -> f64,
    ) -> bool {
        if self.projectiles.len() >= MAX_PROJECTILES {
            return false;
        }
        let Some(station) = round.config.stations.get(round.station) else {
            return false;
        };
        let w = &station.weapon;
        let side = self
            .targets
            .iter()
            .find(|t| t.id == round.aircraft)
            .map_or(NO_SIDE, |t| t.side);
        let mut cockpit = StrikeCockpit {
            target: None,
            selected: round.station,
            armed: true,
            launch_mode: LaunchMode::Cued,
            delivery_mode: DeliveryMode::default(),
            sight: None,
            laser: Laser::default(),
        };
        let launched_t = (self.tick / 30) as u16;
        self.release_round(
            &mut cockpit,
            Round {
                id: round.id,
                owner: round.aircraft,
                side,
                index: round.station,
                station,
                round: 0,
                launcher: round.launcher,
                launched_t,
                record: Some(w.clone()),
                incoming: None,
                gun_pose: Some((round.slot, round.train.heading, round.train.elevation)),
                guidance: None,
                seeker_target: None,
                gun_round: Some(
                    (round.ordinal % u64::from(w.burst.actual_rounds_per_game.max(1))) as u8,
                ),
                tracer: crate::combat::gun_round::tracer(w, round.ordinal),
            },
            &[],
            ground,
        )
    }
}

#[cfg(test)]
#[path = "gunship_fire_tests.rs"]
mod tests;
