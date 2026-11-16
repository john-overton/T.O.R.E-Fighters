//! A guided store's shots at a ground target, as the host's exact-point
//! answer gives them (docs/spec/ai-strike.md, "Stand-off reach"; T.O.R.E
//! opinionated change, John 2026-10-29: "If an attack aircraft can hit
//! outside of range safely of known threats AI should do that").
//!
//! A striker that plans a release from farther out than its usual range
//! asks the host, range by range, whether a shot from there would hit
//! ([`ShotTable::shot`]). The host answers with the same question the AI's
//! release gate asks (under the energy rules the energy model's exact-point
//! answer; under the other rules the store's zone or its glide reach), for a
//! static target on flat ground from a level launch, and caches each answer
//! by store, launch band and range step. The planner takes a range only when
//! the shots one step nearer and one step farther hit too
//! ([`reliable`]): a band where the round strikes the ground (the AT-2's
//! mid-range band from a low hover, docs/baselines/missile-energy.md) or the
//! last step of its reach is never planned.
use crate::ai::weapon_service::StationId;
use std::sync::Arc;

/// The planner's ranges lie on this grid, feet of ground range (fitted,
/// agent): the host caches one answer per step.
pub const SHOT_STEP_FT: f64 = 500.;

/// The launch a shot is asked for: level, toward the target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShotLaunch {
    /// The target's height, feet above sea level.
    pub target_ft: f64,
    /// The launch's height over the target, feet.
    pub height_ft: f64,
    /// The launcher's speed, ft/s.
    pub speed_fps: f64,
}

/// The host's answers (see the module notes).
pub trait ShotTable: Send + Sync {
    /// A shot of `station`'s store from `launch` at a static target
    /// `range_ft` of ground range away: its time of flight, seconds, when it
    /// hits; `None` when it does not (it strikes the ground, falls short, or
    /// the range lies outside the store's zone), or the host does not know
    /// the store.
    fn shot(&self, station: StationId, launch: ShotLaunch, range_ft: f64) -> Option<f64>;
}

/// The host's [`ShotTable`] for this step, if it gave one. Per-step input,
/// never held: two are equal when they are the same table.
#[derive(Clone, Default)]
pub struct Shots(pub Option<Arc<dyn ShotTable>>);

impl Shots {
    /// See [`ShotTable::shot`]; `None` with no table.
    pub fn shot(&self, station: StationId, launch: ShotLaunch, range_ft: f64) -> Option<f64> {
        self.0
            .as_ref()
            .and_then(|table| table.shot(station, launch, range_ft))
    }
    /// Whether the host gave a table.
    pub fn known(&self) -> bool {
        self.0.is_some()
    }
}

impl std::fmt::Debug for Shots {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() {
            "Shots(table)"
        } else {
            "Shots(none)"
        })
    }
}

impl PartialEq for Shots {
    fn eq(&self, other: &Self) -> bool {
        match (&self.0, &other.0) {
            (None, None) => true,
            (Some(a), Some(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

/// `range_ft` on the planner's grid (the nearest step, at least one).
pub fn on_grid(range_ft: f64) -> f64 {
    (range_ft / SHOT_STEP_FT).round().max(1.) * SHOT_STEP_FT
}

/// A shot from `range_ft` (on the grid) hits reliably: it hits, and so do
/// the shots one step nearer and one step farther. Its time of flight when
/// it does. The farther shot is asked last, so a range whose own shot
/// misses costs one answer.
pub fn reliable(
    shots: &Shots,
    station: StationId,
    launch: ShotLaunch,
    range_ft: f64,
) -> Option<f64> {
    let range = on_grid(range_ft);
    let seconds = shots.shot(station, launch, range)?;
    shots.shot(station, launch, range - SHOT_STEP_FT)?;
    shots.shot(station, launch, range + SHOT_STEP_FT)?;
    Some(seconds)
}

/// The grid ranges past `beyond_ft`, nearest first, out to `cap_ft` less one
/// step (the farthest a reliable shot can be).
pub fn ranges_beyond(beyond_ft: f64, cap_ft: f64) -> impl Iterator<Item = f64> {
    let first = (beyond_ft / SHOT_STEP_FT).floor() as i64 + 1;
    let last = ((cap_ft - SHOT_STEP_FT) / SHOT_STEP_FT).floor() as i64;
    (first.max(1)..=last).map(|k| k as f64 * SHOT_STEP_FT)
}

/// The farthest ground range from `height_ft` over the target that still
/// lies inside a slant reach of `slant_ft`.
pub fn ground_reach(slant_ft: f64, height_ft: f64) -> f64 {
    (slant_ft * slant_ft - height_ft * height_ft).max(0.).sqrt()
}

/// A seeker fired from high up is aimed so that the target stands this share
/// of the seeker's look-down limit below the nose (fitted, agent, T160-9,
/// 2026-11-16): the nose pitches a little in flight, and a lock on the
/// limit's edge is lost with it.
pub const LOOK_DOWN_SHARE: f64 = 0.8;
/// The most of a seeker's zone-1 maximum range the AI fires at when it must
/// fire from farther than its usual range (fitted, agent, T160-9).
pub const REACH_SHARE: f64 = 0.95;

/// The slant range the AI fires a seeker from, `height_ft` over its target.
/// `fire_ft` is its usual fire range (10,000 ft or half the zone's maximum
/// `max_ft`, whichever is farther) and `look_deg` the seeker's look-down
/// limit. When the target stands within the limit at the usual range (the
/// height no more than `fire_ft` times the limit's sine) the usual range
/// stands, exactly as before; from higher up the seeker can lock nothing
/// inside it, so the range is where the target stands
/// [`LOOK_DOWN_SHARE`] of the limit below the nose, capped at
/// [`REACH_SHARE`] of the maximum (and never less than the usual range).
pub fn seeker_fire_range(fire_ft: f64, max_ft: f64, look_deg: f64, height_ft: f64) -> f64 {
    if look_deg >= 90. || look_deg <= 0. || height_ft <= fire_ft * look_deg.to_radians().sin() {
        return fire_ft;
    }
    let wanted = height_ft / (look_deg * LOOK_DOWN_SHARE).to_radians().sin();
    wanted.min(max_ft * REACH_SHARE).max(fire_ft)
}

#[cfg(test)]
mod seeker_range_tests {
    use super::seeker_fire_range;

    #[test]
    fn the_usual_range_stands_where_the_seeker_can_lock_in_it() {
        // An AS-16 (zone 40,000 ft, 45 degrees down, usual range 20,000 ft)
        // from 14,000 ft, a Maverick-like 10,000 ft from 7,000 ft, and every
        // seeker with no limit.
        assert_eq!(seeker_fire_range(20_000., 40_000., 45., 14_000.), 20_000.);
        assert_eq!(seeker_fire_range(10_000., 20_000., 45., 7_000.), 10_000.);
        assert_eq!(seeker_fire_range(10_000., 20_000., 180., 50_000.), 10_000.);
        assert_eq!(seeker_fire_range(10_000., 20_000., 0., 50_000.), 10_000.);
    }

    #[test]
    fn a_high_flier_fires_from_where_the_target_is_inside_the_look_down_limit() {
        // 20,000 ft up the AS-16 fires from 34,000 ft slant (36 degrees down).
        let range = seeker_fire_range(20_000., 40_000., 45., 20_000.);
        assert!(
            (range - 20_000. / 36f64.to_radians().sin()).abs() < 1e-6,
            "{range}"
        );
        assert!(range > 33_000. && range < 35_000.);
        // From 30,000 ft the wanted range is past the zone's maximum: capped.
        assert_eq!(seeker_fire_range(20_000., 40_000., 45., 30_000.), 38_000.);
    }
}
