//! The gunship family of the strike tactics (docs/spec/ai-strike.md,
//! "AC-130"; slice ST-F): the AC-130 circles its target on a left-hand
//! orbit whose radius and height come from where its guns reach, and fires
//! its linked guns in short bursts whenever it is on the orbit.
//!
//! - **The orbit** ([`orbit_plan`]): a slant range of 80% of the guns' reach
//!   at 35 degrees of depression, flown at the retail speed halfway between
//!   corner and maximum. The bank never exceeds the aircraft's maximum bank
//!   (30 degrees for the AC-130); the radius grows instead, up to a slant of
//!   90% of the reach, and past that the speed comes down (never below
//!   corner) so the turn fits.
//! - **Flying it** ([`orbit_steer`]): retail's circle law for the heading
//!   (the bearing to the centre plus 90 degrees, less 45 degrees per radius
//!   of distance error, held to 45 to 135), flown with an explicit bank that
//!   blends from the heading error toward the orbit's own bank as the
//!   aircraft settles, capped at the maximum bank.
//! - **Holding off**: while a known hostile SAM covers the target the
//!   gunship circles the target outside that SAM's reach, and comes in once
//!   no SAM covers it.
//! - **Firing** ([`Strike::gunship_fire`]): on the orbit the fast gun (the
//!   25 mm) fires 1 s bursts every 2 s; the slow guns (40 and 105 mm) fire
//!   every round they are due at their records' rates. Each burst aims at the target plus an offset drawn
//!   like a bomb pass's skill circle ([`super::aim_draw`]: 90% inside the
//!   circle, 10% in the ring), which the gunner then walks in: each burst's
//!   offset is halved for every burst already fired at the target, down to
//!   a sixteenth. The host trains each gun on that aim with the guns' own
//!   ballistics, inside its mount's arcs, and fires the guns that bear.
//!
//! All deterministic (John, 2026-10-11, decision 13): the draw is seeded by
//! the aircraft's seed, its id and the burst number.
use super::{
    Context, Order, Pass, Phase, Segment, Steer, StoreKind, Strike, aim_draw, angle_difference,
    bearing_deg, hold_pitch_deg, horizontal, known_position, release_circle_ft, standing,
};
use crate::ai::ScalarSpeed;
use crate::ai::awareness::ground::BriefedKind;
use crate::ai::controller::Activity;
use crate::ai::motion::SpeedRequest;
use crate::attitude::Vector;
use crate::sensors::FEET_PER_NAUTICAL_MILE;

/// Gravity, ft/s².
const G_FTPS2: f64 = 32.174;
/// The share of the guns' reach the orbit's slant range uses (spec, fitted).
pub const REACH_SHARE: f64 = 0.8;
/// The depression of the target from the orbit (spec, fitted on the
/// shipped arcs: inside the 105 mm's 45 degree elevation limit).
pub const DEPRESSION_DEG: f64 = 35.;
/// The AC-130's maximum bank (retail record `maxBank`, 30 degrees;
/// docs/formats/ai.md, "AC-130 circle").
pub const MAX_BANK_DEG: f64 = 30.;
/// The orbit is planned this far below the maximum bank, so the bank law
/// has room to correct an outward drift (fitted, agent).
pub const BANK_RESERVE_DEG: f64 = 5.;
/// The radius may grow for the bank cap until the slant reaches this share
/// of the reach; beyond it the speed comes down instead (fitted, agent).
pub const SLANT_CAP_SHARE: f64 = 0.9;
/// The guns' reach when the stores give none: the shipped guns' 13,000 ft
/// (docs/spec/ac130-linked-guns.md).
pub const DEFAULT_REACH_FT: f64 = 13_000.;
/// The heading error the bank law closes in this many seconds (fitted).
pub const HEADING_TIME_S: f64 = 4.;
/// Retail's on-orbit score: the bank blends toward the orbit's own as the
/// heading error falls under this (retail circle, 30 degrees).
pub const SETTLE_HEADING_DEG: f64 = 30.;
/// On the orbit: within this share of the radius of it, and within
/// [`SETTLE_HEADING_DEG`] of the circle law's heading (fitted, agent).
pub const ON_ORBIT_SHARE: f64 = 0.25;
/// A burst lasts [`BURST_TICKS`]; the fast gun's trigger is held for
/// [`FIRE_TICKS`] of it (fitted, agent).
pub const BURST_TICKS: u64 = 2 * 120;
pub const FIRE_TICKS: u64 = 120;
/// Walking the rounds in: each burst's drawn offset is scaled by this for
/// every burst already fired at the target, never below [`WALK_IN_FLOOR`]
/// (fitted, agent).
pub const WALK_IN: f64 = 0.5;
pub const WALK_IN_FLOOR: f64 = 0.0625;
/// Holding off under a SAM: the gunship circles the target this far outside
/// the covering SAM's reach (fitted, agent).
pub const HOLD_MARGIN_FT: f64 = 2. * FEET_PER_NAUTICAL_MILE;
/// Climb and dive held within this while flying the orbit's height (the
/// AC-130's retail `maxClimb` and `maxDive` are 20 degrees).
pub const PITCH_LIMIT_DEG: f64 = 10.;

/// The orbit the gunship flies about its target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OrbitPlan {
    /// Horizontal radius about the target and height above it, feet.
    pub radius_ft: f64,
    pub height_ft: f64,
    /// Straight-line range from the orbit to the target, feet, and the
    /// target's depression below the horizon from it, degrees.
    pub slant_ft: f64,
    pub depression_deg: f64,
    /// The speed it is flown at, ft/s, and the steady bank that speed and
    /// radius need, degrees.
    pub speed_fps: f64,
    pub bank_deg: f64,
}

/// The orbit from the guns' reach (spec, "AC-130"; John, 2026-10-11: "know
/// range and circle targets appropriately based on where guns can hit").
/// `reach_ft` is the shortest reach of the guns it will fire; the speeds
/// are the aircraft's corner and maximum (ft/s) at the orbit's altitude;
/// `max_bank_deg` is its maximum bank.
pub fn orbit_plan(
    reach_ft: f64,
    corner_fps: f64,
    maximum_fps: f64,
    max_bank_deg: f64,
) -> OrbitPlan {
    let reach = if reach_ft > 0. {
        reach_ft
    } else {
        DEFAULT_REACH_FT
    };
    let depression = DEPRESSION_DEG.to_radians();
    let planning_bank = (max_bank_deg - BANK_RESERVE_DEG).max(5.).to_radians();
    let turn_radius = |speed: f64| speed * speed / (G_FTPS2 * planning_bank.tan());
    let mut speed = 0.5 * (corner_fps + maximum_fps.max(corner_fps));
    let mut radius = REACH_SHARE * reach * depression.cos();
    let cap = SLANT_CAP_SHARE * reach * depression.cos();
    if turn_radius(speed) > radius {
        if turn_radius(speed) > cap {
            // The radius stops at the cap: the speed comes down to fit it,
            // never below corner (where the turn may still need more).
            speed = (G_FTPS2 * planning_bank.tan() * cap)
                .sqrt()
                .max(corner_fps)
                .min(speed);
        }
        radius = turn_radius(speed).max(radius);
    }
    let bank = (speed * speed / (G_FTPS2 * radius)).atan().to_degrees();
    OrbitPlan {
        radius_ft: radius,
        height_ft: radius * depression.tan(),
        slant_ft: radius / depression.cos(),
        depression_deg: DEPRESSION_DEG,
        speed_fps: speed,
        bank_deg: bank,
    }
}

/// The steer that flies the left-hand `orbit` about `centre` (retail's
/// circle law for the heading, an explicit bank for the turn; see the
/// module notes): its radius, its height above the centre, its speed.
/// `speed_fps` is the aircraft's own speed now.
pub fn orbit_steer(
    position: Vector,
    heading_deg: f64,
    speed_fps: f64,
    centre: Vector,
    orbit: &OrbitPlan,
    max_bank_deg: f64,
) -> Steer {
    let radius = orbit.radius_ft.max(1.);
    let (height_ft, orbit_speed_fps) = (orbit.height_ft, orbit.speed_fps);
    let distance = horizontal(position, centre);
    let offset = (90. - 45. * (distance - radius) / radius).clamp(45., 135.);
    let wanted = (bearing_deg(position, centre) + offset).rem_euclid(360.);
    let error = angle_difference(heading_deg, wanted);
    let settled =
        (1. - error.abs() / SETTLE_HEADING_DEG - (distance - radius).abs() / radius).clamp(0., 1.);
    let speed = speed_fps.max(50.);
    // A left orbit turns left: heading falls at V / R radians a second.
    let orbit_rate = -(speed / radius).to_degrees();
    let rate = error / HEADING_TIME_S + settled * orbit_rate;
    let bank = (speed * rate.to_radians() / G_FTPS2)
        .atan()
        .to_degrees()
        .clamp(-max_bank_deg, max_bank_deg);
    Steer {
        heading_deg: wanted,
        pitch_deg: hold_pitch_deg(position[1], centre[1] + height_ft)
            .clamp(-PITCH_LIMIT_DEG, PITCH_LIMIT_DEG),
        speed: SpeedRequest::Explicit(ScalarSpeed(orbit_speed_fps)),
        afterburner: false,
        bank_deg: Some(bank),
    }
}

/// Whether the aircraft flies on the orbit: near its radius and on the
/// circle law's heading.
pub fn on_orbit(position: Vector, heading_deg: f64, centre: Vector, radius_ft: f64) -> bool {
    let radius = radius_ft.max(1.);
    let distance = horizontal(position, centre);
    let offset = (90. - 45. * (distance - radius) / radius).clamp(45., 135.);
    let wanted = bearing_deg(position, centre) + offset;
    (distance - radius).abs() <= ON_ORBIT_SHARE * radius
        && angle_difference(heading_deg, wanted).abs() <= SETTLE_HEADING_DEG
}

/// The gunship's guns on the orbit, for the host to fire: the target, the
/// burst's aim offset from it on the ground (feet along x and z), the
/// burst's number at this target, whether its draw lies inside the skill
/// circle, and whether the burst's trigger is held this tick (the fast gun
/// fires only then; the slow guns fire every round they are due).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GunFire {
    pub target: u32,
    pub aim_ft: [f64; 2],
    pub burst: u32,
    pub inside: bool,
    pub held: bool,
}

/// The walk-in factor of the `burst`th burst at a target (from 1).
pub fn walk_in(burst: u32) -> f64 {
    WALK_IN
        .powi(burst.saturating_sub(1).min(16) as i32)
        .max(WALK_IN_FLOOR)
}

impl Strike {
    /// The gunship's guns at `tick` while it is on the orbit and firing;
    /// `None` otherwise. The host trains and fires the guns.
    pub fn gunship_fire(&self, tick: u64) -> Option<GunFire> {
        let firing = self.phase == Phase::Attack
            && self.segment == Segment::Final
            && self.kind == Some(StoreKind::Gun)
            && self.pass_number > 0;
        firing.then_some(GunFire {
            target: self.target?,
            aim_ft: self.aim_ft,
            burst: self.pass_number,
            inside: self.inside,
            held: tick.saturating_sub(self.since) % BURST_TICKS < FIRE_TICKS,
        })
    }
}

/// The guns' reach: the shortest zone-1 range among the guns with rounds.
fn reach(cx: &Context<'_>) -> f64 {
    cx.inputs
        .stores
        .iter()
        .filter(|s| s.kind == StoreKind::Gun && s.rounds > 0 && s.maximum_range_ft > 0.)
        .map(|s| s.maximum_range_ft)
        .fold(f64::INFINITY, f64::min)
}

/// The orbit for this step's speeds.
pub(super) fn plan(cx: &Context<'_>) -> OrbitPlan {
    let reach = reach(cx);
    orbit_plan(
        if reach.is_finite() { reach } else { 0. },
        cx.own.limits.corner.0,
        cx.own.limits.maximum.0,
        MAX_BANK_DEG.min(cx.own.maximum_bank_deg),
    )
}

/// The radius a SAM covering `target` leaves the gunship to circle at: the
/// farthest any covering hostile SAM reaches from the target, plus the
/// margin. `None` while no SAM covers the target.
fn hold_off_radius(cx: &Context<'_>, target: Vector) -> Option<f64> {
    cx.picture
        .threats()
        .hostile_defenses(cx.side)
        .filter(|d| d.covers_with_missiles(target))
        .map(|d| horizontal(d.position, target) + d.facts.missile_reach_ft + HOLD_MARGIN_FT)
        .reduce(f64::max)
}

/// One step of the gunship (see the module notes).
pub(super) fn step(s: &mut Strike, cx: &Context<'_>) -> Order {
    let own = cx.own;
    let mut order = Order {
        steer: None,
        release: None,
        activity: None,
    };
    // A defence the air logic flies; back to the orbit once quiet.
    if cx.defending || s.threatened(cx) {
        if s.phase != Phase::Defend {
            s.enter(Phase::Defend, cx.tick);
        }
        return order;
    }
    if s.phase == Phase::Defend {
        if !s.quiet(cx) {
            return order;
        }
        s.enter(Phase::Ingress, cx.tick);
    }
    let guns = cx
        .inputs
        .stores
        .iter()
        .find(|st| st.kind == StoreKind::Gun && st.rounds > 0)
        .copied();
    let Some(gun) = guns else {
        s.enter(Phase::Home, cx.tick);
        s.target = None;
        return order;
    };
    // The target: kept while it stands, else the next.
    let alive = |s: &Strike| match (s.target, cx.inputs.solution) {
        (Some(id), Some(sol)) if sol.target == id => sol.target_alive,
        (Some(id), _) => standing(cx.picture, id),
        (None, _) => false,
    };
    if !alive(s) {
        match s.pick_target(cx) {
            Some(id) => {
                if s.target != Some(id) {
                    s.pass_number = 0;
                }
                s.target = Some(id);
                s.station = Some(gun.station);
                s.kind = Some(StoreKind::Gun);
                s.pass = Some(Pass::Orbit);
                if s.phase != Phase::Ingress {
                    s.enter(Phase::Ingress, cx.tick);
                }
            }
            None => {
                let waiting = cx
                    .picture
                    .briefed()
                    .iter()
                    .any(|b| b.standing && b.side != cx.side && b.kind != BriefedKind::Ordered);
                s.target = None;
                s.enter(
                    if waiting && cx.picture.ordered().is_none() {
                        Phase::Wait
                    } else {
                        Phase::Home
                    },
                    cx.tick,
                );
                return order;
            }
        }
    }
    let Some(target) = s.target else {
        return order;
    };
    let Some(target_at) = cx
        .inputs
        .solution
        .filter(|sol| sol.target == target)
        .map(|sol| sol.target_position)
        .or_else(|| known_position(cx.picture, target))
    else {
        return order;
    };
    let plan = plan(cx);
    let max_bank = MAX_BANK_DEG.min(own.maximum_bank_deg);
    let speed = own.speed.0;
    // Holding off while a SAM covers the target (spec, opinionated agent).
    if let Some(radius) = hold_off_radius(cx, target_at) {
        if s.phase != Phase::Wait {
            s.enter(Phase::Wait, cx.tick);
        }
        order.steer = Some(orbit_steer(
            own.position,
            own.heading_deg,
            speed,
            target_at,
            &OrbitPlan {
                radius_ft: radius.max(plan.radius_ft),
                ..plan
            },
            max_bank,
        ));
        order.activity = Some(Activity::Pursuing);
        return order;
    }
    let orbiting = on_orbit(own.position, own.heading_deg, target_at, plan.radius_ft);
    match (s.phase, orbiting) {
        (Phase::Attack, true) => {
            if s.segment != Segment::Final {
                s.segment_to(Segment::Final, cx.tick);
            }
        }
        (Phase::Attack, false) => {
            if s.segment == Segment::Final {
                s.segment_to(Segment::Approach, cx.tick);
            }
        }
        (_, true) => {
            s.enter(Phase::Attack, cx.tick);
            s.segment_to(Segment::Final, cx.tick);
        }
        (_, false) => {
            if s.phase != Phase::Ingress {
                s.enter(Phase::Ingress, cx.tick);
            }
        }
    }
    // A new burst: draw its aim and walk it in.
    if s.phase == Phase::Attack
        && s.segment == Segment::Final
        && cx.tick.saturating_sub(s.since).is_multiple_of(BURST_TICKS)
    {
        s.pass_number += 1;
        let draw = aim_draw(
            s.seed,
            cx.aircraft,
            s.pass_number,
            release_circle_ft(cx.experience),
        );
        let walk = walk_in(s.pass_number);
        s.aim_ft = [draw.offset_ft[0] * walk, draw.offset_ft[1] * walk];
        s.inside = draw.inside;
    }
    order.steer = Some(orbit_steer(
        own.position,
        own.heading_deg,
        speed,
        target_at,
        &plan,
        max_bank,
    ));
    order.activity = Some(if s.phase == Phase::Attack {
        Activity::Attacking
    } else {
        Activity::Pursuing
    });
    order
}

#[cfg(test)]
#[path = "family_gunship_tests.rs"]
mod tests;
