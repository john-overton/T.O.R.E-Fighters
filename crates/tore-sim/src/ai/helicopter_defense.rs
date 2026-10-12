//! The helicopter's defence against a missile or gunfire (docs/spec/ai-strike.md,
//! "Helicopter defence"; docs/spec/ai.md B20 and B47; slice ST-H).
//!
//! Retail's helicopter script answers an infrared or radar launch with a
//! break of plus or minus 90 degrees (`H.AI`, `ir_launch`/`radar_launch`), not
//! the fighter's jink and dive. The fighters' reactions fly corner speed with a
//! dive: a helicopter flying them loses its lift and sinks into the ground (a
//! Mi-24 under the Ukraine column's SAMs did, in the strike driver). This is
//! the helicopter's version:
//!
//! 1. **The break**: heading plus or minus 90 degrees to the threat's bearing,
//!    the side needing the smaller turn (retail: plus or minus 90 from the
//!    current heading, 50/50; T.O.R.E picks by geometry, John's decision 13,
//!    no dice). With no bearing at all, plus or minus 90 from the current
//!    heading as in retail. Retail turns toward the target instead when it is
//!    ahead; there is no "target" for a launch warning here, so the beam
//!    stands (opinionated, John 2026-10-11).
//! 2. **Speed and height are kept**: the break asks for maximum speed (the
//!    adapter flies a helicopter at that speed; at corner it settles nose down)
//!    and a flight path no steeper than 10 degrees either way, so it neither
//!    dives nor zooms (opinionated, John 2026-10-11).
//! 3. **Terrain masking**: it settles to 200 ft above the highest ground in the
//!    next six seconds and, of the breaks near the beam (60, 90 and 120
//!    degrees), takes the one that puts the most terrain between it and the
//!    threat's site over the next ten seconds, where there is any. The
//!    candidate set, the horizon and the weights are fitted (agent).
use crate::attitude::Vector;

/// Retail's break off the threat's bearing, degrees.
pub const BREAK_DEG: f64 = 90.;
/// Breaks the masking search may take instead of the beam, degrees off the
/// threat's bearing (fitted).
pub const OFFSETS_DEG: [f64; 3] = [90., 60., 120.];
/// The height it settles to over the ground, feet (the stand-off attack's
/// masked height).
pub const MASK_AGL_FT: f64 = 200.;
/// The steepest flight path it commands either way, degrees.
pub const PITCH_LIMIT_DEG: f64 = 10.;
/// How far ahead the masking search flies a candidate, seconds (fitted).
pub const LOOK_S: u32 = 10;
/// How far ahead the height floor looks, seconds (fitted).
pub const FLOOR_LOOK_S: u32 = 6;
/// How far ahead the pitch aims, seconds.
pub const PITCH_AIM_S: f64 = 4.;
/// A threat's site is seen from this far above its ground, feet (a radar
/// dish, a gun barrel).
pub const SITE_HEIGHT_FT: f64 = 40.;
/// What a second of masked path is worth against a degree of turn (fitted).
const MASKED_STEP_WEIGHT: f64 = 1.0;
const TURN_WEIGHT_PER_DEG: f64 = 1. / 90.;
const OFFSET_WEIGHT_PER_DEG: f64 = 0.5 / 30.;
/// A second of path the helicopter cannot climb clear of costs this much.
const BLOCKED_STEP_WEIGHT: f64 = 2.0;
/// A break that goes on turning the way the helicopter is already banked
/// (more than [`COMMITTED_BANK_DEG`]) is worth this much, when it is a real
/// turn (more than [`COMMITTED_TURN_DEG`]): a small correction is never
/// overshot for it.
const COMMITTED_TURN_WEIGHT: f64 = 0.75;
const COMMITTED_BANK_DEG: f64 = 10.;
const COMMITTED_TURN_DEG: f64 = 45.;

/// The helicopter's own state.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Own {
    pub position: Vector,
    pub heading_deg: f64,
    pub speed_ft_s: f64,
    /// Right positive. A turn already begun is kept: the threat selected can
    /// change from one missile to another and the beam with it.
    pub bank_deg: f64,
    /// Its maximum speed here, ft/s: it comes down no faster than it can
    /// hold, so a descent never takes it past the limit.
    pub maximum_ft_s: f64,
}

/// What it is defending against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Threat {
    /// The compass bearing from the helicopter to the threat, when known.
    pub bearing_deg: Option<f64>,
    /// Where the threat sees from (a site, else the missile), when known:
    /// the terrain between it and the helicopter is what masks.
    pub viewpoint: Option<Vector>,
    /// Breaks the tie when two sides need the same turn (the missile's id).
    pub tie: u32,
}

/// The defence's answer for this step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Break {
    pub heading_deg: f64,
    pub flight_path_pitch_deg: f64,
    /// Seconds of the next [`LOOK_S`] in which terrain hides the helicopter
    /// from the viewpoint on this heading.
    pub masked_s: u32,
    /// Seconds of that path the helicopter cannot climb clear of.
    pub blocked_s: u32,
    /// The break's angle off the threat's bearing, degrees.
    pub offset_deg: f64,
}

fn wrap(value: f64) -> f64 {
    value.rem_euclid(360.)
}

fn delta(from: f64, to: f64) -> f64 {
    (to - from + 180.).rem_euclid(360.) - 180.
}

fn along(position: Vector, heading_deg: f64, feet: f64) -> [f64; 2] {
    let h = heading_deg.to_radians();
    [position[0] + h.sin() * feet, position[2] + h.cos() * feet]
}

/// What flying `heading` for [`LOOK_S`] seconds is worth.
struct Run {
    masked_s: u32,
    blocked_s: u32,
    /// The highest ground-plus-mask height in the first [`FLOOR_LOOK_S`]
    /// seconds.
    floor_ft: f64,
}

fn fly(
    own: Own,
    heading: f64,
    viewpoint: Option<Vector>,
    ground: &impl Fn(f64, f64) -> f64,
) -> Run {
    let speed = own.speed_ft_s.max(60.);
    let climb = speed * PITCH_LIMIT_DEG.to_radians().sin();
    let mut floor_ft = f64::MIN;
    let mut masked_s = 0;
    let mut blocked_s = 0;
    let mut altitude = own.position[1];
    for second in 1..=LOOK_S {
        let [x, z] = along(own.position, heading, speed * f64::from(second));
        let required = ground(x, z) + MASK_AGL_FT;
        if second <= FLOOR_LOOK_S {
            floor_ft = floor_ft.max(required);
        }
        // It may come down at the pitch limit and go up at it, and no lower
        // than the mask height over the ground it is crossing.
        altitude = if required >= altitude {
            if required > altitude + climb {
                blocked_s += 1;
            }
            (altitude + climb).min(required)
        } else {
            (altitude - climb).max(required)
        };
        if let Some(site) = viewpoint {
            let at = [x, altitude, z];
            let eye = [site[0], site[1] + SITE_HEIGHT_FT, site[2]];
            if crate::combat::live::terrain_hit(eye, at, ground).is_some() {
                masked_s += 1;
            }
        }
    }
    Run {
        masked_s,
        blocked_s,
        floor_ft,
    }
}

/// The break for this step.
pub fn decide(own: Own, threat: Threat, ground: impl Fn(f64, f64) -> f64) -> Break {
    let side_tie = if threat.tie.is_multiple_of(2) {
        1.
    } else {
        -1.
    };
    let (reference, offsets): (f64, &[f64]) = match threat.bearing_deg {
        Some(bearing) => (bearing, &OFFSETS_DEG),
        None => (own.heading_deg, &OFFSETS_DEG[..1]),
    };
    let mut best: Option<(f64, Break, f64)> = None;
    for &offset in offsets {
        // The tie rule's side first, and only a strictly better score
        // replaces a candidate, so a threat dead ahead or astern breaks the
        // same way every time.
        for side in [side_tie, -side_tie] {
            let heading = wrap(reference + side * offset);
            let run = fly(own, heading, threat.viewpoint, &ground);
            let turn = delta(own.heading_deg, heading).abs();
            let committed = own.bank_deg.abs() > COMMITTED_BANK_DEG
                && turn > COMMITTED_TURN_DEG
                && delta(own.heading_deg, heading).signum() == own.bank_deg.signum();
            let score = f64::from(run.masked_s) * MASKED_STEP_WEIGHT
                + if committed { COMMITTED_TURN_WEIGHT } else { 0. }
                - f64::from(run.blocked_s) * BLOCKED_STEP_WEIGHT
                - turn * TURN_WEIGHT_PER_DEG
                - (offset - BREAK_DEG).abs() * OFFSET_WEIGHT_PER_DEG;
            if best.as_ref().is_none_or(|(top, _, _)| score > *top + 1e-9) {
                best = Some((
                    score,
                    Break {
                        heading_deg: heading,
                        flight_path_pitch_deg: 0.,
                        masked_s: run.masked_s,
                        blocked_s: run.blocked_s,
                        offset_deg: offset,
                    },
                    run.floor_ft,
                ));
            }
        }
    }
    let (_, mut chosen, floor_ft) = best.expect("at least one candidate");
    let aim = (own.speed_ft_s.max(60.) * PITCH_AIM_S).max(1.);
    let down = if own.speed_ft_s > own.maximum_ft_s {
        0.
    } else {
        PITCH_LIMIT_DEG
    };
    chosen.flight_path_pitch_deg = (floor_ft - own.position[1])
        .atan2(aim)
        .to_degrees()
        .clamp(-down, PITCH_LIMIT_DEG);
    if chosen.flight_path_pitch_deg.abs() < 0.5 {
        chosen.flight_path_pitch_deg = 0.;
    }
    chosen
}

#[cfg(test)]
#[path = "helicopter_defense_tests.rs"]
mod tests;
