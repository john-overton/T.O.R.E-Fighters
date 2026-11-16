//! B-1B Lancer (`B1.SH`) own-source rig (B1B-3): the swing wing (two 17 face
//! panels, flap strips riding them), the two whole-quad tailerons, the signed
//! rudder, the roll spoilers and speed brake, the six gear plates a leg group,
//! the three fitted weapon bays (B1B-5: two door leaves cut from the belly
//! polygons a bay, a lined recess, drawn through [`crate::bay_doors`]) and the
//! four afterburner plumes.
//! See docs/formats/aircraft-variety.md "B-1B shape and rig (B1B-2/3,
//! 2026-11-15)" and "B-1B Lancer stations, bays and pylons (B1B-5, 2026-11-16)".
//!
//! `B1.SH` ships seven state words: 0x7b10 the plumes (sixteen plates), 0x7b16
//! the bay walls (eight), 0x7b1c the spoilers (four faces, both sides), 0x7b22
//! the gear (twelve), 0x7b2e and 0x7b34 the left and right flaps (signed: -1
//! swaps the strip for a drooped copy and a cap, +1 deletes it) and 0x7b3a the
//! rudder (signed: +1 and -1 swap the quad for a copy skewed 3 units to the
//! right or left, 2 or more deletes it). The shape has **no sweep word**: the
//! wing is one fixed spread pose, 17 whole faces a side that share exactly one
//! vertex line, (+-10, 11, z), with the fixed glove, so the sweep is a fitted
//! rig that turns every wing face as one rigid panel about a vertical axis
//! through that line (decision B-1B 2, 2026-11-15). The rig never sets the
//! flap, rudder or spoiler words for the pieces that already exist in the
//! neutral shape: it keeps the strips and the rudder quad drawn and turns them,
//! so no panel vanishes in flight.
//!
//! Every number below is a shape unit (4/3 ft at the header scale 4) and is
//! passed through [`scale_point`], so a panel turned about a hinge in the scaled
//! shape stays rigid (the B-1B scale is the identity, kept for safety).
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};
use tore_sim::combat::bays::Opening;
use tore_sim::combat::swing::B1B;

const ID: AircraftId = AircraftId::B1b;
const WORDS: [usize; 7] = [0x7b10, 0x7b16, 0x7b1c, 0x7b22, 0x7b2e, 0x7b34, 0x7b3a];

/// The right outer wing panel: 17 whole faces, the flap strip (the two
/// `ed` faces `4667` and `4686`) included. Shared vertex line with the fixed
/// glove: (10, 11, z) only.
pub(crate) const WING_R: [usize; 17] = [
    0x4558, 0x45a7, 0x45c8, 0x460f, 0x4528, 0x45f6, 0x4579, 0x44ce, 0x4541, 0x44fc, 0x4667, 0x4686,
    0x4626, 0x45df, 0x4513, 0x44e5, 0x4590,
];
/// The left outer wing panel (the exact mirror), flap strip `4ad6`, `4af5`.
pub(crate) const WING_L: [usize; 17] = [
    0x4a37, 0x4a7e, 0x49de, 0x4a16, 0x49ae, 0x4a65, 0x49ff, 0x493d, 0x49c7, 0x496b, 0x4ad6, 0x4af5,
    0x4a95, 0x4a4e, 0x4999, 0x4954, 0x4982,
];
pub(crate) const STRIP_R: [usize; 2] = [0x4667, 0x4686];
pub(crate) const STRIP_L: [usize; 2] = [0x4ad6, 0x4af5];
/// The drooped poses' end caps (words 0x7b34 and 0x7b2e at -1): one triangle
/// closing the outboard end of a drooped strip. The rig draws it, rebuilt on
/// the neutral strip's end profile, while the flaps are down.
pub(crate) const CAP_R: usize = 0x470e;
pub(crate) const CAP_L: usize = 0x4b7d;
/// The drooped poses' own faces (never drawn; read to pin the retail droop).
const DROOP_R: [usize; 3] = [0x46d0, 0x46ef, 0x470e];
const DROOP_L: [usize; 3] = [0x4b3f, 0x4b5e, 0x4b7d];

/// The tailerons: one quad a side in two windings, no state word.
pub(crate) const TAIL_R: [usize; 2] = [0x30a0, 0x30ec];
pub(crate) const TAIL_L: [usize; 2] = [0x2f7c, 0x2ff0];
/// The rudder quad (two windings), and the signed word's poses.
pub(crate) const RUDDER: [usize; 2] = [0x4c6b, 0x4c8a];
const RUDDER_RIGHT: [usize; 2] = [0x4bb1, 0x4bd0];
const RUDDER_LEFT: [usize; 2] = [0x4c0e, 0x4c2d];
/// The spoiler plates (word 0x7b1c), two windings a side.
pub(crate) const SPOILER_R: [usize; 2] = [0x52fa, 0x5311];
pub(crate) const SPOILER_L: [usize; 2] = [0x5328, 0x533f];
pub(crate) const SPOILER: [usize; 4] = [0x52fa, 0x5311, 0x5328, 0x533f];
/// Gear word 0x7b22: per main leg a side plate (x +-2, y -5 to 3) and a cross
/// plate (y -1, x 1 to 3), two windings each; the nose leg a fore-and-aft plate
/// (x 0, y 44 to 49) and a cross plate (y 45), two windings each.
pub(crate) const MAIN_R: [usize; 4] = [0x5025, 0x5044, 0x5063, 0x5082];
pub(crate) const MAIN_L: [usize; 4] = [0x50e6, 0x5105, 0x5124, 0x5143];
pub(crate) const NOSE: [usize; 4] = [0x51a7, 0x51c6, 0x51e5, 0x5204];
pub(crate) const GEAR: [usize; 12] = [
    0x5025, 0x5044, 0x5063, 0x5082, 0x50e6, 0x5105, 0x5124, 0x5143, 0x51a7, 0x51c6, 0x51e5, 0x5204,
];
/// Bay word 0x7b16: four vertical walls (x +-3, y 10 to 21 and 22 to 32, z -1
/// to -4), two windings each. Only reviewed since B1B-5, which cuts the bays
/// from the belly polygons below: the plates are no longer drawn, and one is
/// the material of the lining.
pub(crate) const BAY: [usize; 8] = [
    0x41fc, 0x4213, 0x422a, 0x4241, 0x4258, 0x426f, 0x4286, 0x429d,
];
/// The flat belly polygons the fitted bays are cut from, both at z -1 and
/// facing down: `3617` (8 vertices, x -10 to 10, y -16 to 29) and `362e` ahead
/// of it (y 29 to 48), which the forward bay (y 22 to 32) runs into.
pub(crate) const BELLY: [usize; 2] = [0x3617, 0x362e];
/// The belly plane in shape units.
const BELLY_Z: f32 = -1.;
/// The first address of the lining's faces (above every face of the shape,
/// whose code is 27,468 bytes).
pub(crate) const LINING_BASE: usize = 0x7000;
/// A lining is the ceiling and four walls of each of the three bays.
pub(crate) const LINING: usize = 3 * 5;
/// The shape's feet a unit (4/3, its header scale).
pub(crate) const UNIT: f64 = 4. / 3.;
/// The palette index of the recess: the grey of the shape's own bay plate
/// (the one winding that is not the skin's 157).
const LINING_GREY: u8 = 144;

fn lining(address: usize) -> bool {
    address >= LINING_BASE
}

/// The rudder's full-pedal deflection, radians (decision "B-1B rig", 2026-11-16,
/// B1B-5): the retail word's 43 degrees (0.75 rad) opens a 4.4 ft gap at the
/// taileron roots, 0.45 rad opens 2.6 ft. Fitted; the pedal still scales the
/// whole travel.
pub(crate) const RUDDER_LIMIT: f64 = 0.45;
/// Plume word 0x7b10: per engine a vertical plate (x +-5 and +-8) and a flat
/// one (x 4 to 6 and 7 to 9, z -3), two windings each, y -17 to -28.
pub(crate) const FLAME: [usize; 16] = [
    0x4d70, 0x4d97, 0x4dbe, 0x4de5, 0x4e0c, 0x4e33, 0x4e5a, 0x4e81, 0x4ea8, 0x4ecf, 0x4ef6, 0x4f1d,
    0x4f44, 0x4f6b, 0x4f92, 0x4fb9,
];
/// The four nozzle exit discs (subtype 0x64, y -17): left outer, left inner,
/// right inner, right outer.
pub(crate) const NOZZLE: [usize; 4] = [0x3a1f, 0x3a40, 0x40cb, 0x40f2];
const NOZZLE_PLANE: f32 = -17.;
const FLAME_TIP: f32 = -28.;

/// The wing pivot, a vertical axis through the shared vertex line (10, 11),
/// from the spec so the stations and the vapour point use the same one.
const PIVOT_R: [f32; 3] = [10., 11., 0.];
const PIVOT_L: [f32; 3] = [-10., 11., 0.];
const _: () = assert!(B1B.pivot_right[0] == 10. && B1B.pivot_right[1] == 11.);
const _: () = assert!(B1B.pivot_left[0] == -10. && B1B.pivot_left[1] == 11.);

/// The swept panel's slab is `SINK` units thinner on each side (top and
/// bottom, 0.03 units, 0.04 ft), eased in over the first 0.1 rad of turn, so
/// the root that swings inside the fuselage lies a hair inside the belly
/// polygon (z -1) and the glove's flat step (z 1) instead of coplanar with
/// them (fitted, B1B-3, the z-fight). The vertices on the pivot line stay
/// exactly where they are, so the one seam the shape has stays closed.
const SINK: f32 = 0.03;
const SINK_RAMP: f64 = 0.1;

/// Taileron law (fitted, agent decision 2026-11-15; no cited B-1B limit):
/// smaller than the fleet's 0.40 clamp, as the Tornado's. The whole quad turns
/// about the lateral line through the middle of its root chord.
pub(crate) const TAIL_PITCH: f64 = 0.16;
pub(crate) const TAIL_ROLL: f64 = 0.09;
pub(crate) const TAIL_LIMIT: f64 = 0.22;
const _: () = assert!(TAIL_LIMIT < 0.40);
/// Root chord midpoint, on the centre line: the taileron roots run from y -35
/// to y -22 at z 10 on the fin's plane.
const TAIL_SPINDLE: [f32; 3] = [0., -28.5, 10.];
/// Flap droop (fitted to the retail drooped poses, which put the trailing edge
/// 0.54 rad down at the inboard end and 0.46 at the outboard end).
pub(crate) const FLAP: f64 = 0.50;
/// Flap strip hinge: the mid-line of the strip's leading edge, from (13, 1, 0)
/// to (42, 0, 0) on the right; the left mirrors it.
const STRIP_PIVOT: [f32; 3] = [13., 1., 0.];
const STRIP_AXIS: [f32; 3] = [29., -1., 0.];
/// Rudder hinge: the leading edge of the quad, (0, -30, 10) to (0, -34, 18).
const RUDDER_HINGE: [[f32; 3]; 2] = [[0., -30., 10.], [0., -34., 18.]];
/// Spoiler hinge lines, on the glove skin (the edge at y 0; right side,
/// the left mirrors it).
const SPOILER_HINGE_R: [[f32; 3]; 2] = [[4., 0., 4.], [6., 0., 1.]];
const SPOILER_HINGE_L: [[f32; 3]; 2] = [[-4., 0., 4.], [-6., 0., 1.]];
/// Share of the spoiler travel a full aileron commands (fitted): the roll
/// spoilers open on the side of the lowering wing, up to this much, and the
/// speed brake opens both fully.
pub(crate) const SPOILER_ROLL_SHARE: f64 = 0.6;

/// Gear hinges (fitted, agent decision 2026-11-15; the shape has none). A main
/// leg's side and cross plates turn inward as one rigid piece about the
/// longitudinal line through x +-3 (the cross plate's outboard edge, so no
/// corner ever dips below the gear-down contact while it folds) at z 0, one
/// unit above the belly polygon (z -1), so the stowed plates lie inside the
/// closed well. The nose leg turns
/// aft about the lateral line through its forward top corner (y 49, z 0).
/// Timing: linear in the simulation's gear fraction, every leg together (the
/// shape has no doors to sequence).
const MAIN_HINGE_X: f32 = 3.;
const MAIN_HINGE_Z: f32 = 0.;
const NOSE_HINGE: [f32; 3] = [0., 49., 0.];
/// The nose wheel steers about a vertical axis through the fore-and-aft
/// plate's centre.
const NOSE_STEER: [f32; 3] = [0., 46.5, 0.];

/// How far the wing panels have turned from the spread pose, radians (0 spread
/// at 15 degrees, 52.5 degrees fully swept to 67.5): the spec's speed schedule
/// held spread by the flaps. It only draws the wing; no flight law reads it.
pub(crate) fn sweep_turn(state: &State) -> f64 {
    B1B.turn(state.speed, state.flaps, state.gear)
}
/// The leading-edge sweep angle for a turn, degrees.
#[cfg(test)]
pub(crate) fn sweep_degrees(turn: f64) -> f64 {
    B1B.sweep_degrees(turn)
}

/// One taileron angle in radians, positive lowering the trailing edge: a pull
/// raises both trailing edges, a positive aileron raises the right one and
/// lowers the left (the fleet's sense), the sum clamped to [`TAIL_LIMIT`].
pub(crate) fn tail_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    (-TAIL_PITCH * state.elevator.clamp(-1., 1.) - side * TAIL_ROLL * state.aileron.clamp(-1., 1.))
        .clamp(-TAIL_LIMIT, TAIL_LIMIT)
}

/// How far one spoiler is open, 0 to 1. The speed brake opens both; a roll
/// command opens the side that is going down (positive aileron, right wing
/// down) by [`SPOILER_ROLL_SHARE`] at full stick. The two add and are capped.
pub(crate) fn spoiler_open(right: bool, state: &State) -> f64 {
    let roll = state.aileron.clamp(-1., 1.);
    let side = if right { roll.max(0.) } else { (-roll).max(0.) };
    (state.brake.clamp(0., 1.) + SPOILER_ROLL_SHARE * side).min(1.)
}

/// The wingtip vapour attachment of one side (0 left, 1 right) for a wing turn,
/// feet right, up and forward of the aircraft. The record's streamer is the
/// spread-wing trailing tip (pivot (40, 0, 44) CE units, which is exactly the
/// sweep pivot (10, 11) units at 4/3 ft a unit, and a tip point at (68, 0, -4)
/// ft, the drawn tip (51, -3)); it is carried by the same rigid turn as the
/// wing, so the vapour leaves the drawn tip at every sweep.
pub(crate) fn streamer_point(
    def: &tore_formats::shape::StreamerDef,
    side: usize,
    turn: f64,
) -> Option<[f64; 3]> {
    let spread = def.attachment(side, 0).ok()?.map(|v| v / 3.);
    Some(B1B.swept(side == 1, spread, turn))
}

fn sub(a: [f32; 3], b: [f32; 3]) -> [f64; 3] {
    std::array::from_fn(|k| f64::from(a[k]) - f64::from(b[k]))
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a.iter().zip(b).map(|(x, y)| x * y).sum()
}
fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    let length = dot(a, a).sqrt();
    a.map(|v| v / length)
}

/// A hinge line, the turn that carries a closed pose to the retail open pose.
#[derive(Clone, Copy, Debug)]
struct Hinge {
    pivot: [f32; 3],
    axis: [f32; 3],
    /// Right-handed about `axis`, radians, closed to open.
    angle: f64,
}

/// The mid-point of a face's two vertices that are off the hinge line `a`-`b`
/// (more than half a unit away): the free edge of a hinged quad.
fn free_mid(face: &Face, a: [f32; 3], b: [f32; 3]) -> AppResult<[f64; 3]> {
    let axis = unit(sub(b, a));
    let free: Vec<_> = face
        .positions
        .iter()
        .filter(|&&p| {
            let v = sub(p, a);
            let along = dot(v, axis);
            (dot(v, v) - along * along).sqrt() > 0.5
        })
        .collect();
    if free.len() != 2 {
        return Err(format!(
            "B1.SH hinged quad {:x} has no two-vertex free edge",
            face.address
        )
        .into());
    }
    Ok(std::array::from_fn(|k| {
        f64::from(free[0][k] + free[1][k]) / 2.
    }))
}

/// The signed turn about the line `a` to `b` (right-handed) that carries the
/// direction `from` to the direction `to`, both taken perpendicular to it.
fn signed_angle(a: [f32; 3], b: [f32; 3], from: [f64; 3], to: [f64; 3]) -> f64 {
    let axis = unit(sub(b, a));
    let perp = |v: [f64; 3]| {
        let along = dot(v, axis);
        unit(std::array::from_fn(|k| v[k] - along * axis[k]))
    };
    let (v, w) = (perp(from), perp(to));
    dot(axis, cross(v, w)).atan2(dot(v, w))
}

fn hinge_of(a: [f32; 3], b: [f32; 3], angle: f64) -> Hinge {
    let (a, b) = (scale_point(ID, a), scale_point(ID, b));
    Hinge {
        pivot: a,
        axis: std::array::from_fn(|k| b[k] - a[k]),
        angle,
    }
}

pub struct Rig {
    /// The three bays, forward to aft ([`tore_sim::combat::bays::openings`]).
    bays: Vec<Opening>,
    /// The rudder quad's hinge and the turn to the +1 pose.
    rudder: Hinge,
    /// The spoiler hinges, right then left, and the turn from the closed
    /// (lying aft on the glove skin) pose to the open (source) pose.
    spoilers: [Hinge; 2],
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("B1.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("B1.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

/// A state word set to `value` adds exactly `added` and removes exactly
/// `removed`; the pose is returned (unscaled).
fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    value: i32,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let pose = Shape::with_state(bytes, &[(word, value)].into())?;
    let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
    if active.difference(neutral).copied().collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || neutral
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed B1.SH branch {word:x} = {value}").into());
    }
    Ok(pose)
}

fn has(face: &Face, p: [f32; 3]) -> bool {
    face.positions.contains(&p)
}

/// Whether the plan-view point `p` (shape units, right and forward) lies in
/// the convex polygon `face` (edges included).
fn contains(face: &Face, p: [f32; 2]) -> bool {
    let n = face.positions.len();
    let sides: Vec<f32> = (0..n)
        .map(|i| {
            let (a, b) = (face.positions[i], face.positions[(i + 1) % n]);
            (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
        })
        .collect();
    sides.iter().all(|&c| c >= -1e-4) || sides.iter().all(|&c| c <= 1e-4)
}

impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 27468
            || shape.faces.len() != 314
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed B1.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // The wing panels: 17 whole faces a side, each present once, and the
        // only vertices they share with any fixed face are on the pivot line
        // (+-10, 11): the one seam, which a turn about that line never opens.
        for (ids, side) in [(&WING_R, 1f32), (&WING_L, -1f32)] {
            for &address in ids {
                one(&shape, address)?;
            }
            let panel: BTreeSet<[u32; 3]> = shape
                .faces
                .iter()
                .filter(|f| ids.contains(&f.address))
                .flat_map(|f| f.positions.iter().map(|p| p.map(f32::to_bits)))
                .collect();
            let shared: BTreeSet<[u32; 3]> = shape
                .faces
                .iter()
                .filter(|f| !WING_R.contains(&f.address) && !WING_L.contains(&f.address))
                .flat_map(|f| f.positions.iter().map(|p| p.map(f32::to_bits)))
                .filter(|p| panel.contains(p))
                .collect();
            let on_pivot = |p: &[u32; 3]| {
                let p = p.map(f32::from_bits);
                p[0] == side * 10. && p[1] == 11.
            };
            if shared.is_empty() || !shared.iter().all(on_pivot) {
                return Err("unreviewed B1.SH wing seam (not the (10, 11) line)".into());
            }
        }
        // Flap strips: whole wedge pairs on the panel's trailing edge, the
        // leading edge (13, 1) to (42, 0) the hinge; the drooped poses and the
        // end caps are the signed words' (-1 swaps, +1 deletes).
        for (ids, side) in [(&STRIP_R, 1f32), (&STRIP_L, -1f32)] {
            for &address in ids {
                let face = one(&shape, address)?;
                if face.positions.len() != 4
                    || !has(face, [side * 13., -3., 0.])
                    || !has(face, [side * 42., -3., 0.])
                {
                    return Err(format!("unreviewed B1.SH flap strip {address:x}").into());
                }
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words: -1 swaps a strip for a drooped copy and a cap, +1
        // removes the strip and adds nothing.
        let droop_r = branch(bytes, &original, 0x7b34, -1, &DROOP_R, &STRIP_R)?;
        let droop_l = branch(bytes, &original, 0x7b2e, -1, &DROOP_L, &STRIP_L)?;
        branch(bytes, &original, 0x7b34, 1, &[], &STRIP_R)?;
        branch(bytes, &original, 0x7b2e, 1, &[], &STRIP_L)?;
        // The cap: the drooped pose's triangle with its free vertex put back on
        // the neutral strip's end profile (42, -3, 0), so it turns with the
        // strip it closes.
        let mut caps = Vec::new();
        for (pose, address, side) in [(&droop_r, CAP_R, 1f32), (&droop_l, CAP_L, -1f32)] {
            let template = one(pose, address)?;
            let mut cap = template.clone();
            let [lead_top, lead_bottom] = [[side * 42., 0., 1.], [side * 42., 0., -1.]];
            if cap.positions.len() != 3 || !has(&cap, lead_top) || !has(&cap, lead_bottom) {
                return Err(format!("unreviewed B1.SH flap cap {address:x}").into());
            }
            for p in &mut cap.positions {
                if *p != lead_top && *p != lead_bottom {
                    *p = [side * 42., -3., 0.];
                }
            }
            caps.push(cap.scaled(axis));
        }
        // The rudder word: +1 and -1 swap the quad for a skewed copy; the rig
        // reads the +1 copy for the turn and never sets the word.
        let right = branch(bytes, &original, 0x7b3a, 1, &RUDDER_RIGHT, &RUDDER)?.scaled(axis);
        branch(bytes, &original, 0x7b3a, -1, &RUDDER_LEFT, &RUDDER)?;
        let (a, b) = (
            scale_point(ID, RUDDER_HINGE[0]),
            scale_point(ID, RUDDER_HINGE[1]),
        );
        for &address in &RUDDER {
            let face = one(&shape, address)?;
            if face.positions.len() != 4 || !has(face, a) || !has(face, b) {
                return Err(format!("unreviewed B1.SH rudder quad {address:x}").into());
            }
        }
        let rudder = hinge_of(
            RUDDER_HINGE[0],
            RUDDER_HINGE[1],
            signed_angle(
                a,
                b,
                {
                    let m = free_mid(one(&shape, RUDDER[0])?, a, b)?;
                    std::array::from_fn(|k| m[k] - f64::from(a[k]))
                },
                {
                    let m = free_mid(one(&right, RUDDER_RIGHT[0])?, a, b)?;
                    std::array::from_fn(|k| m[k] - f64::from(a[k]))
                },
            ),
        );
        // The spoiler word adds the four plates (two windings a side) and
        // removes nothing. Closed, a plate lies aft along the glove skin from
        // its hinge edge; the source pose is the open one.
        let spoil = branch(bytes, &original, 0x7b1c, 1, &SPOILER, &[])?.scaled(axis);
        let mut spoilers = Vec::new();
        for (ids, hinge) in [
            (&SPOILER_R, &SPOILER_HINGE_R),
            (&SPOILER_L, &SPOILER_HINGE_L),
        ] {
            let (a, b) = (scale_point(ID, hinge[0]), scale_point(ID, hinge[1]));
            for &address in ids {
                let face = one(&spoil, address)?;
                if face.positions.len() != 4 || !has(face, a) || !has(face, b) {
                    return Err(format!("unreviewed B1.SH spoiler plate {address:x}").into());
                }
            }
            let free = free_mid(one(&spoil, ids[0])?, a, b)?;
            let middle: [f64; 3] = std::array::from_fn(|k| f64::from(a[k] + b[k]) / 2.);
            let open: [f64; 3] = std::array::from_fn(|k| free[k] - middle[k]);
            // Aft along the skin: the aircraft's -y.
            spoilers.push(hinge_of(
                hinge[0],
                hinge[1],
                signed_angle(a, b, [0., -1., 0.], open),
            ));
        }
        // The gear, bay and plume words add exactly the reviewed faces and
        // remove nothing; the plates' corners are checked against the source.
        let gear = branch(bytes, &original, 0x7b22, 1, &GEAR, &[])?.scaled(axis);
        let bay = branch(bytes, &original, 0x7b16, 1, &BAY, &[])?.scaled(axis);
        let flame = branch(bytes, &original, 0x7b10, 1, &FLAME, &[])?.scaled(axis);
        let inside = |face: &Face, lo: [f32; 3], hi: [f32; 3]| {
            face.positions.len() == 4
                && face
                    .positions
                    .iter()
                    .all(|p| (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k]))
        };
        for (ids, lo, hi) in [
            (&MAIN_R[..], [1., -5., -8.], [3., 3., -1.]),
            (&MAIN_L[..], [-3., -5., -8.], [-1., 3., -1.]),
            (&NOSE[..], [-1., 44., -8.], [2., 49., -1.]),
        ] {
            for &address in ids {
                if !inside(one(&gear, address)?, lo, hi) {
                    return Err(format!("unreviewed B1.SH gear plate {address:x}").into());
                }
            }
        }
        for &address in &BAY {
            let face = one(&bay, address)?;
            if !inside(face, [-3., 10., -4.], [3., 32., -1.])
                || !face.positions.iter().all(|p| p[0].abs() == 3.)
            {
                return Err(format!("unreviewed B1.SH bay wall {address:x}").into());
            }
        }
        // Each engine's exit disc is on the plane y -17; the plume plates run
        // from it to y -28 (the nozzle plane is the root every plate scales
        // from, so the plates that share its edge stay together).
        for address in NOZZLE {
            let face = one(&shape, address)?;
            if face.positions.len() != 6 || !face.positions.iter().all(|p| p[1] == NOZZLE_PLANE) {
                return Err(format!("unreviewed B1.SH nozzle {address:x}").into());
            }
        }
        let (mut root, mut tip) = (f32::NEG_INFINITY, f32::INFINITY);
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4 {
                return Err(format!("unreviewed B1.SH plume plate {address:x}").into());
            }
            for p in &face.positions {
                root = root.max(p[1]);
                tip = tip.min(p[1]);
            }
        }
        if root != NOZZLE_PLANE || tip != FLAME_TIP {
            return Err("unreviewed B1.SH plume extent".into());
        }
        // The belly polygons the fitted bays are cut from: whole, flat at the
        // belly plane and facing down, and between them they cover every corner
        // of the three openings.
        let bays: Vec<Opening> = tore_sim::combat::bays::openings(ID);
        if bays.len() != 3 {
            return Err("B1B bay openings missing".into());
        }
        let plane = scale_point(ID, [0., 0., BELLY_Z])[2];
        for &address in &BELLY {
            let face = one(&shape, address)?;
            if !face.normal.is_some_and(|n| n[1] < -32000.)
                || !face.positions.iter().all(|p| (p[2] - plane).abs() < 1e-4)
            {
                return Err(format!("unreviewed B1.SH belly skin {address:x}").into());
            }
        }
        let belly: Vec<&Face> = BELLY
            .iter()
            .map(|&a| one(&shape, a))
            .collect::<Result<_, _>>()?;
        for o in &bays {
            for (x, y) in [
                (-o.outer, o.aft),
                (o.outer, o.aft),
                (-o.outer, o.fore),
                (o.outer, o.fore),
            ] {
                let point = [(x / UNIT) as f32, (y / UNIT) as f32];
                if !belly.iter().any(|f| contains(f, point)) {
                    return Err("unreviewed B1.SH belly skin under a bay".into());
                }
            }
        }
        // The recess, drawn only while the doors are open, takes the material
        // of the shape's own bay plate (no longer drawn itself).
        let template = one(&bay, BAY[0])?.clone();
        let lining_faces = bays
            .iter()
            .flat_map(|o| {
                crate::bay_doors::centre_lining_in(&template, o, UNIT, &[], &[], [LINING_GREY; 3])
            })
            .enumerate()
            .map(|(k, mut f)| {
                f.address = LINING_BASE + k;
                f
            })
            .collect::<Vec<_>>();
        debug_assert_eq!(lining_faces.len(), LINING);
        let mut extra: Vec<Face> = Vec::new();
        extra.extend(
            spoil
                .faces
                .into_iter()
                .filter(|f| SPOILER.contains(&f.address)),
        );
        extra.extend(gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)));
        extra.extend(lining_faces);
        extra.extend(
            flame
                .faces
                .into_iter()
                .filter(|f| FLAME.contains(&f.address)),
        );
        extra.extend(caps.iter().cloned());
        shape.faces.extend(extra);
        let [spoiler_r, spoiler_l]: [Hinge; 2] =
            spoilers.try_into().map_err(|_| "two spoiler hinges")?;
        Ok((
            Self {
                bays,
                rudder,
                spoilers: [spoiler_r, spoiler_l],
            },
            shape,
        ))
    }

    /// A wing panel face (and the strip and cap on it) for the state: the
    /// slab sinks a hair as the root swings under the skin, the strip droops
    /// about its leading edge in the panel's own frame, then the whole panel
    /// turns about the pivot.
    fn wing(&self, source: &Face, right: bool, state: &State) -> Face {
        let a = source.address;
        let mut result = source.clone();
        let swept = sweep_turn(state);
        if swept > 0. {
            let sink = SINK * (swept / SINK_RAMP).min(1.) as f32;
            let pivot = if right { PIVOT_R } else { PIVOT_L };
            for p in &mut result.positions {
                if !(p[0] == pivot[0] && p[1] == pivot[1]) {
                    p[2] *= 1. - sink;
                }
            }
        }
        let flaps = state.flaps.clamp(0., 1.);
        if flaps > 0. && (STRIP_R.contains(&a) || STRIP_L.contains(&a) || a == CAP_R || a == CAP_L)
        {
            let (pivot, direction, angle) = if right {
                (STRIP_PIVOT, STRIP_AXIS, FLAP * flaps)
            } else {
                (
                    [-STRIP_PIVOT[0], STRIP_PIVOT[1], STRIP_PIVOT[2]],
                    [-STRIP_AXIS[0], STRIP_AXIS[1], STRIP_AXIS[2]],
                    -FLAP * flaps,
                )
            };
            turn(
                &mut result,
                scale_point(ID, pivot),
                scale_point(ID, direction),
                angle,
            );
        }
        if swept > 0. {
            turn(
                &mut result,
                scale_point(ID, if right { PIVOT_R } else { PIVOT_L }),
                [0., 0., 1.],
                if right { -swept } else { swept },
            );
        }
        result
    }

    /// The faces a source face becomes: with the bay doors off their seat
    /// the two belly polygons are cut into the door leaves of the three bays,
    /// turned down about their hinges by `state.bay` (the doors' 1 second
    /// travel is the flight's); the lining draws only then.
    pub fn faces(&self, source: &Face, state: &State) -> Vec<Face> {
        let a = source.address;
        if BELLY.contains(&a) && state.bay > 0. {
            return crate::bay_doors::centre_doors_all(source, state.bay, &self.bays, UNIT);
        }
        if lining(a) && state.bay <= 0. {
            return Vec::new();
        }
        vec![source.clone()]
    }

    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let right = WING_R.contains(&a) || a == CAP_R;
        if right || WING_L.contains(&a) || a == CAP_L {
            // The end cap closes a drooped strip only; hidden while it is up.
            if (a == CAP_R || a == CAP_L) && state.flaps <= 0. {
                return None;
            }
            return Some(self.wing(source, right, state));
        }
        if TAIL_R.contains(&a) || TAIL_L.contains(&a) {
            let mut result = source.clone();
            let angle = tail_angle(TAIL_R.contains(&a), state);
            if angle != 0. {
                turn(
                    &mut result,
                    scale_point(ID, TAIL_SPINDLE),
                    scale_point(ID, [1., 0., 0.]),
                    angle,
                );
            }
            return Some(result);
        }
        if RUDDER.contains(&a) {
            let mut result = source.clone();
            let angle =
                self.rudder.angle.clamp(-RUDDER_LIMIT, RUDDER_LIMIT) * state.rudder.clamp(-1., 1.);
            if angle != 0. {
                turn(&mut result, self.rudder.pivot, self.rudder.axis, angle);
            }
            return Some(result);
        }
        if SPOILER.contains(&a) {
            let side = SPOILER_R.contains(&a);
            let open = spoiler_open(side, state);
            if open <= 0. {
                return None;
            }
            let hinge = &self.spoilers[usize::from(!side)];
            let mut result = source.clone();
            // Open (1) is the source pose; closing turns the whole plate about
            // its hinge edge down onto the glove skin.
            turn(
                &mut result,
                hinge.pivot,
                hinge.axis,
                -hinge.angle * (1. - open),
            );
            return Some(result);
        }
        if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            let mut result = source.clone();
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
            return Some(result);
        }
        // The lining draws only while the doors are off their seat.
        if lining(a) && state.bay <= 0. {
            return None;
        }
        if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            // The plume grows from the nozzle plane; every plate scales from
            // the same root, so the plates that share an edge stay together.
            let root = scale_point(ID, [0., NOZZLE_PLANE, 0.])[1];
            let mut result = source.clone();
            for p in &mut result.positions {
                p[1] = root + (p[1] - root) * state.exhaust.clamp(0., 1.) as f32;
            }
            return Some(result);
        }
        Some(source.clone())
    }
}

/// Whole-piece gear motion. Gear down (1) is the source pose; nothing is drawn
/// at 0 (the caller hides it). The nose leg also steers.
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let angle = FRAC_PI_2 * (1. - gear.clamp(0., 1.));
    let a = f.address;
    if NOSE.contains(&a) {
        if steering != 0. {
            turn(
                f,
                scale_point(ID, NOSE_STEER),
                scale_point(ID, [0., 0., 1.]),
                -steering,
            );
        }
        turn(
            f,
            scale_point(ID, NOSE_HINGE),
            scale_point(ID, [1., 0., 0.]),
            -angle,
        );
    } else if MAIN_R.contains(&a) || MAIN_L.contains(&a) {
        let inward = if MAIN_R.contains(&a) { 1. } else { -1. };
        let x = if MAIN_R.contains(&a) {
            MAIN_HINGE_X
        } else {
            -MAIN_HINGE_X
        };
        turn(
            f,
            scale_point(ID, [x, 0., MAIN_HINGE_Z]),
            scale_point(ID, [0., 1., 0.]),
            inward * angle,
        );
    }
}

/// The plume plates, drawn as the afterburner flame.
pub(crate) fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    /// A synthetic face whose vertices are named in shape units.
    fn face(address: usize, points: &[[f32; 3]]) -> Face {
        let positions: Vec<[f32; 3]> = points.iter().map(|&p| scale_point(ID, p)).collect();
        Face {
            address,
            colors: vec![31; positions.len()],
            uv: vec![[0.2, 0.8]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0x61,
            normal: Some([0., 0., 1.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn spread(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
        let d =
            |p: [f32; 3], q: [f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt();
        let mut worst = 0f32;
        for i in 0..a.len() {
            for j in i + 1..a.len() {
                worst = worst.max((d(a[i], a[j]) - d(b[i], b[j])).abs());
            }
        }
        worst
    }
    /// The plan-view rigidity: the slab thins by [`SINK`] with the sweep, so
    /// only the distances in the horizontal plane are kept exactly.
    fn planar(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
        let flat = |p: &[[f32; 3]]| p.iter().map(|q| [q[0], q[1], 0.]).collect::<Vec<_>>();
        spread(&flat(a), &flat(b))
    }
    fn points(faces: &[Face]) -> Vec<[f32; 3]> {
        faces.iter().flat_map(|f| f.positions.clone()).collect()
    }
    fn knots(s: &mut State, kt: f64) {
        s.speed = kt * 1.68781;
    }
    fn rig() -> Rig {
        // The hinges and turns the retail poses give, worked out from the
        // retail vertices the same way `load` does.
        let angle =
            |a: [f32; 3], b: [f32; 3], from: [f64; 3], to: [f64; 3]| signed_angle(a, b, from, to);
        let spoiler = |hinge: [[f32; 3]; 2], free: [f64; 3]| {
            let middle = [
                f64::from(hinge[0][0] + hinge[1][0]) / 2.,
                f64::from(hinge[0][1] + hinge[1][1]) / 2.,
                f64::from(hinge[0][2] + hinge[1][2]) / 2.,
            ];
            let open = std::array::from_fn(|k| free[k] - middle[k]);
            hinge_of(
                hinge[0],
                hinge[1],
                angle(hinge[0], hinge[1], [0., -1., 0.], open),
            )
        };
        let rudder_from = |p: [f64; 3]| -> [f64; 3] {
            std::array::from_fn(|k| p[k] - f64::from(RUDDER_HINGE[0][k]))
        };
        Rig {
            bays: tore_sim::combat::bays::openings(ID),
            rudder: hinge_of(
                RUDDER_HINGE[0],
                RUDDER_HINGE[1],
                angle(
                    RUDDER_HINGE[0],
                    RUDDER_HINGE[1],
                    rudder_from([0., -36., 14.]),
                    rudder_from([2.5, -35., 14.]),
                ),
            ),
            spoilers: [
                spoiler(SPOILER_HINGE_R, [7.5, -4., 5.5]),
                spoiler(SPOILER_HINGE_L, [-7.5, -4., 5.5]),
            ],
        }
    }

    /// One test for the whole rig, on synthetic faces named in shape units:
    /// the wing panel turns rigidly about (10, 11) with the pivot-line vertex
    /// fixed, the swept span, the flap strip droops and rides the panel, the
    /// taileron turns with pitch and differential roll, the rudder hinge
    /// holds, the spoiler opens on the roll side, the gear folds whole and
    /// the flame grows from the nozzle plane.
    #[test]
    fn the_b1b_rig_sweeps_the_wing_and_turns_whole_panels_about_their_hinges() {
        let rig = rig();
        let mut s = state();
        // The right wing: a top face and a bottom face on the pivot line, and
        // the trailing flap strip, as the retail faces have them.
        let top = face(
            0x45c8,
            &[[50., 2., 1.], [42., 0., 1.], [13., 1., 1.], [10., 11., 1.]],
        );
        let bottom = face(
            0x460f,
            &[
                [10., 11., -1.],
                [13., 1., -1.],
                [42., 0., -1.],
                [50., 2., -1.],
            ],
        );
        let strip = face(
            0x4667,
            &[[42., -3., 0.], [13., -3., 0.], [13., 1., 1.], [42., 0., 1.]],
        );
        let tip = face(0x4528, &[[51., -3., 0.], [51., 2., 0.], [50., 2., 1.]]);
        let wing = [top.clone(), bottom.clone(), strip.clone(), tip.clone()];
        // Spread below 280 kt: the retail pose exactly.
        knots(&mut s, 200.);
        for f in &wing {
            assert_eq!(rig.animate(f, &s).unwrap().positions, f.positions);
        }
        // The schedule: spread to 280 kt, fully swept (67.5 degrees, a 52.5
        // degree turn) from 520 kt, linear between, flaps hold it spread.
        for (kt, flaps, degrees) in [
            (280., 0., 15.),
            (400., 0., 15. + 52.5 / 2.),
            (520., 0., 67.5),
            (600., 0., 67.5),
            (600., 1., 15.),
            (520., 0.5, 15. + 52.5 * 0.5),
        ] {
            knots(&mut s, kt);
            s.flaps = flaps;
            assert!(
                (sweep_degrees(sweep_turn(&s)) - degrees).abs() < 1e-6,
                "{kt} {flaps}"
            );
        }
        // Fully swept: every vertex turned 52.5 degrees about the vertical
        // axis through (10, 11), the pivot-line vertex stays, panel rigid,
        // the tip aft and in, the swept span about 74 ft.
        knots(&mut s, 520.);
        s.flaps = 0.;
        let turned: Vec<Face> = wing.iter().map(|f| rig.animate(f, &s).unwrap()).collect();
        assert!(
            planar(&points(&wing), &points(&turned)) < 2e-3,
            "wing not rigid"
        );
        // And in height, the slab thins by at most SINK a side.
        assert!(spread(&points(&wing), &points(&turned)) < 4. * SINK + 2e-3);
        let pin = |faces: &[Face], z: f32| {
            faces
                .iter()
                .flat_map(|f| f.positions.iter())
                .find(|p| p[0] == 10. && p[1] == 11. && p[2] == z)
                .copied()
        };
        assert_eq!(
            pin(&turned, -1.),
            Some([10., 11., -1.]),
            "the seam vertex moved"
        );
        let tip_out = turned[3].positions[1];
        // (51, 2) swung 52.5 degrees about (10, 11): about (27.8, -27.0).
        assert!(
            (tip_out[0] - 27.8).abs() < 0.2 && (tip_out[1] + 27.0).abs() < 0.2,
            "{tip_out:?}"
        );
        let span_ft = 2. * f64::from(tip_out[0]) * 4. / 3.;
        assert!((span_ft - 74.2).abs() < 0.6, "swept span {span_ft} ft");
        // The mirror: the left wing turns by the same angle the other way.
        let left = face(0x4a37, &[[-10., 11., 1.], [-10., 11., 0.], [-50., 2., 0.]]);
        let l = rig.animate(&left, &s).unwrap();
        let r = rig
            .animate(
                &face(0x4558, &[[10., 11., 1.], [10., 11., 0.], [50., 2., 0.]]),
                &s,
            )
            .unwrap();
        for k in 0..3 {
            for i in 0..3 {
                let want = if k == 0 {
                    -r.positions[i][k]
                } else {
                    r.positions[i][k]
                };
                assert!((l.positions[i][k] - want).abs() < 1e-4);
            }
        }
        // The slab sinks a hair at full sweep and not at all spread.
        assert!((turned[0].positions[1][2] - (1. - SINK)).abs() < 1e-6);
        assert_eq!(turned[1].positions[0][2], -1.);
        // The flap strip droops about its leading edge and rides the panel:
        // flaps hold the wing spread, so the droop is measured spread.
        knots(&mut s, 200.);
        s.flaps = 1.;
        let down = rig.animate(&strip, &s).unwrap();
        assert!(spread(&strip.positions, &down.positions) < 1e-3);
        assert!(
            down.positions[1][2] < -1.5,
            "trailing edge droops: {:?}",
            down.positions[1]
        );
        assert!(down.positions[1][1] > -3., "and comes forward");
        let tipped = rig.animate(&top, &s).unwrap();
        assert_eq!(tipped.positions, top.positions, "the panel itself stays");
        // At full sweep the strip sweeps with the panel (flaps up here).
        knots(&mut s, 520.);
        s.flaps = 0.;
        let swept_strip = rig.animate(&strip, &s).unwrap();
        assert!((swept_strip.positions[0][0] - 13.).abs() > 5.);
        // The end cap draws only with the flaps out.
        let cap = face(CAP_R, &[[42., -3., 0.], [42., 0., 1.], [42., 0., -1.]]);
        assert!(rig.animate(&cap, &s).is_none());
        s.flaps = 0.5;
        assert!(rig.animate(&cap, &s).is_some());

        // Tailerons: whole quad, pitch together, roll differentially.
        let taileron = |address, side: f32| {
            face(
                address,
                &[
                    [0., -35., 10.],
                    [0., -22., 10.],
                    [side * 14., -36., 10.],
                    [side * 15., -40., 10.],
                ],
            )
        };
        let (right, left) = (taileron(0x30a0, 1.), taileron(0x2f7c, -1.));
        let mut s = state();
        let trailing =
            |s: &State, f: &Face| rig.animate(f, s).unwrap().positions[3][2] - f.positions[3][2];
        for (elevator, aileron) in [
            (1., 0.),
            (-1., 0.),
            (0., 1.),
            (0., -1.),
            (1., -1.),
            (-1., 1.),
        ] {
            s.elevator = elevator;
            s.aileron = aileron;
            for f in [&right, &left] {
                let t = rig.animate(f, &s).unwrap();
                assert!(
                    spread(&f.positions, &t.positions) < 1e-3,
                    "taileron not rigid"
                );
                // The root chord's midpoint is the spindle: it stays.
                let a = f.positions[0];
                let b = f.positions[1];
                let mid = [(a[0] + b[0]) / 2., (a[1] + b[1]) / 2., (a[2] + b[2]) / 2.];
                let now = [
                    (t.positions[0][0] + t.positions[1][0]) / 2.,
                    (t.positions[0][1] + t.positions[1][1]) / 2.,
                    (t.positions[0][2] + t.positions[1][2]) / 2.,
                ];
                for k in 0..3 {
                    assert!((mid[k] - now[k]).abs() < 1e-3);
                }
            }
        }
        s.aileron = 0.;
        s.elevator = 1.;
        assert!(trailing(&s, &right) > 0.01 && trailing(&s, &left) > 0.01);
        s.elevator = 0.;
        s.aileron = 1.;
        assert!(trailing(&s, &right) > 0.01 && trailing(&s, &left) < -0.01);
        s.elevator = 5.;
        s.aileron = -5.;
        assert!((tail_angle(true, &s) - (TAIL_ROLL - TAIL_PITCH)).abs() < 1e-9);
        s.elevator = 1.;
        s.aileron = 1.;
        assert!((tail_angle(true, &s) + TAIL_LIMIT).abs() < 1e-9 || tail_angle(true, &s) < 0.);

        // Rudder: the two hinge vertices stay, the trailing edge swings to the
        // signed side, rigid.
        let rudder = face(
            0x4c6b,
            &[
                [0., -35., 10.],
                [0., -30., 10.],
                [0., -34., 18.],
                [0., -37., 18.],
            ],
        );
        let mut s = state();
        for value in [-1., -0.5, 0.5, 1.] {
            s.rudder = value;
            let t = rig.animate(&rudder, &s).unwrap();
            assert!(spread(&rudder.positions, &t.positions) < 1e-3);
            for k in 0..3 {
                assert!((t.positions[1][k] - rudder.positions[1][k]).abs() < 1e-4);
                assert!((t.positions[2][k] - rudder.positions[2][k]).abs() < 1e-4);
            }
            assert!(
                t.positions[0][0] * value as f32 > 0.1,
                "trailing edge to the pedal side"
            );
        }
        s.rudder = 0.;
        assert_eq!(
            rig.animate(&rudder, &s).unwrap().positions,
            rudder.positions
        );
        // B1B-5: full pedal is 0.45 rad, not the retail word's 0.75: the free
        // vertex (4.47 units off the hinge line) moves 4.47 sin 0.45 to the side.
        s.rudder = 1.;
        let full = rig.animate(&rudder, &s).unwrap();
        assert!((f64::from(full.positions[0][0]) - 4.472_136 * RUDDER_LIMIT.sin()).abs() < 1e-3);
        assert!(rig.rudder.angle.abs() > RUDDER_LIMIT);
        s.rudder = -1.;
        let other = rig.animate(&rudder, &s).unwrap();
        assert!((full.positions[0][0] + other.positions[0][0]).abs() < 1e-4);

        // Spoilers: one open fraction a side, hidden closed, the source pose
        // fully open, the hinge edge always on the glove skin.
        let plate = face(
            0x52fa,
            &[[4., 0., 4.], [6., -4., 7.], [9., -4., 4.], [6., 0., 1.]],
        );
        let mut s = state();
        assert!(rig.animate(&plate, &s).is_none());
        s.aileron = 1.;
        assert!(
            rig.animate(&plate, &s).is_some(),
            "right wing down opens the right spoiler"
        );
        let left_plate = face(
            0x5328,
            &[[-4., 0., 4.], [-6., 0., 1.], [-9., -4., 4.], [-6., -4., 7.]],
        );
        assert!(rig.animate(&left_plate, &s).is_none());
        s.aileron = 0.;
        s.brake = 1.;
        assert_eq!(rig.animate(&plate, &s).unwrap().positions, plate.positions);
        s.brake = 0.5;
        let half = rig.animate(&plate, &s).unwrap();
        assert!(spread(&plate.positions, &half.positions) < 1e-3);
        for k in 0..3 {
            assert!((half.positions[0][k] - plate.positions[0][k]).abs() < 1e-4);
        }
        assert!(
            half.positions[1][2] < plate.positions[1][2] - 0.1,
            "closing lowers the free edge"
        );
        assert!((spoiler_open(true, &s) - 0.5).abs() < 1e-12);
        s.brake = 1.;
        s.aileron = 1.;
        assert_eq!(spoiler_open(true, &s), 1.);

        // Gear: hidden at 0, the source pose at 1, whole pieces between, the
        // stowed pieces inside the belly.
        let wheel = face(
            0x5025,
            &[[2., -5., -1.], [2., 3., -1.], [2., 3., -8.], [2., -5., -8.]],
        );
        let strut = face(
            0x51a7,
            &[
                [0., 44., -1.],
                [0., 49., -1.],
                [0., 49., -8.],
                [0., 44., -8.],
            ],
        );
        for piece in [&wheel, &strut] {
            let mut s = state();
            s.gear = 0.;
            assert!(rig.animate(piece, &s).is_none());
            s.gear = 1.;
            assert_eq!(rig.animate(piece, &s).unwrap().positions, piece.positions);
            for gear in [0.75, 0.5, 0.25, 1e-6] {
                s.gear = gear;
                let out = rig.animate(piece, &s).unwrap();
                assert!(spread(&piece.positions, &out.positions) < 1e-3);
                let low = piece
                    .positions
                    .iter()
                    .map(|p| p[2])
                    .fold(f32::INFINITY, f32::min);
                assert!(out.positions.iter().all(|p| p[2] >= low - 1e-3));
            }
        }
        let mut s = state();
        s.gear = 1e-6;
        let stowed = rig.animate(&wheel, &s).unwrap();
        // The main plate lies flat at the hinge height plus its offset from
        // the hinge line (1 unit), inboard of x 2.
        assert!(
            stowed
                .positions
                .iter()
                .all(|p| (p[2] - 1.).abs() < 1e-2 && p[0] < 2.01)
        );
        let nose = rig.animate(&strut, &s).unwrap();
        assert!(
            nose.positions
                .iter()
                .all(|p| p[1] <= 49.01 && p[2] >= -1e-3)
        );

        // The interim bay walls (B1B-3) are no longer drawn since B1B-5, the
        // fitted bays replace them (`the_b1b_bays_cut_six_door_leaves_from_the_belly_and_line_the_recess`):
        // the source plate comes back as it is, whatever the bay. Flames grow from the nozzle plane.
        let wall = face(
            0x41fc,
            &[
                [3., 21., -1.],
                [3., 21., -4.],
                [3., 10., -4.],
                [3., 10., -1.],
            ],
        );
        let mut s = state();
        for bay in [0., 0.5, 1.] {
            s.bay = bay;
            assert_eq!(rig.animate(&wall, &s).unwrap().positions, wall.positions);
        }
        let plume = face(
            0x4d70,
            &[
                [8., -17., -3.],
                [8., -17., -2.],
                [8., -28., -2.],
                [8., -28., -3.],
            ],
        );
        let mut s = state();
        assert!(rig.animate(&plume, &s).is_none());
        s.exhaust = 1.;
        assert_eq!(rig.animate(&plume, &s).unwrap().positions, plume.positions);
        s.exhaust = 0.5;
        let p = rig.animate(&plume, &s).unwrap();
        assert!((p.positions[2][1] + 22.5).abs() < 1e-5 && p.positions[0][1] == -17.);
        assert!(flame(0x4d70) && !flame(0x4558));
    }

    /// The vapour point rides the swept tip: spread it is the drawn
    /// trailing tip (+-68 ft, 4 ft aft), swept 52.5 degrees the same rigid
    /// turn the wing faces take.
    #[test]
    fn the_vapour_point_follows_the_swept_tip() {
        // The record's streamer: pivot (40, 0, 44) and tip (164, 0, -56) CE
        // units a side, in 24.8 fixed point, hinge scale 40.
        let def = tore_formats::shape::StreamerDef {
            pivot: [10240, 0, 11264],
            hinge_scale: 40,
            points: [[41984, 0, -14336], [41984, 0, -14336]],
        };
        for side in 0..2 {
            let sign = if side == 1 { 1. } else { -1. };
            let spread = streamer_point(&def, side, 0.).unwrap();
            assert!((spread[0] - sign * 68.).abs() < 1e-9, "{spread:?}");
            assert!(spread[1].abs() < 1e-9 && (spread[2] + 4.).abs() < 1e-9);
            let swept = streamer_point(&def, side, B1B.max_turn()).unwrap();
            // (51, -3) units turned 52.5 degrees about (10, 11): about
            // (23.9, -30.0) units, in feet 31.8 and -40.0.
            assert!((swept[0] - sign * 31.8).abs() < 0.3, "{swept:?}");
            assert!((swept[2] + 40.0).abs() < 0.3, "{swept:?}");
        }
    }

    /// B1B-5: the bays cut the belly polygons into six door leaves while the
    /// doors are off their seat (two a bay, the forward bay's from both
    /// polygons), draw their lining then, and leave the skin whole shut.
    #[test]
    fn the_b1b_bays_cut_six_door_leaves_from_the_belly_and_line_the_recess() {
        let rig = rig();
        let big = face(
            BELLY[0],
            &[
                [-9., -11., -1.],
                [-3., -16., -1.],
                [3., -16., -1.],
                [9., -11., -1.],
                [10., 11., -1.],
                [5., 29., -1.],
                [-5., 29., -1.],
                [-10., 11., -1.],
            ],
        );
        let fore = face(
            BELLY[1],
            &[
                [-2., 48., -1.],
                [-5., 29., -1.],
                [5., 29., -1.],
                [2., 48., -1.],
            ],
        );
        let ceiling = face(LINING_BASE, &[[-3., 10., 4.], [3., 10., 4.], [3., 21., 4.]]);
        let mut s = state();
        s.bay = 0.;
        assert_eq!(rig.faces(&big, &s).len(), 1);
        assert_eq!(rig.faces(&fore, &s).len(), 1);
        assert!(rig.faces(&ceiling, &s).is_empty() && rig.animate(&ceiling, &s).is_none());
        s.bay = 1.;
        let hanging = |faces: Vec<Face>| {
            faces
                .into_iter()
                .filter(|f| f.positions.iter().any(|p| p[2] < -1. - 2.))
                .collect::<Vec<_>>()
        };
        let low = hanging(rig.faces(&big, &s));
        let high = hanging(rig.faces(&fore, &s));
        // Each of the two belly polygons gives two leaves in the bays it reaches:
        // the big one the intermediate, aft and forward bays' (so six), the one
        // ahead the forward bay's two.
        assert_eq!((low.len(), high.len()), (6, 2));
        // A leaf is 3 units wide, hangs 3 units below the belly plane, on x +/-3.
        for leaf in low.iter().chain(&high) {
            let bottom = leaf.positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
            assert!((bottom - (-1. - 3.)).abs() < 1e-3, "{bottom}");
            assert!(
                leaf.positions
                    .iter()
                    .all(|p| (p[0].abs() - 3.).abs() < 1e-3)
            );
        }
        // The forward bay's leaves from the two polygons share the hinge line
        // and the seam: one runs y 22 to 29, the other 29 to 32.
        let reach = |leaves: &[Face], lo: f32, hi: f32| {
            leaves.iter().any(|f| {
                f.positions
                    .iter()
                    .all(|p| p[1] >= lo - 1e-3 && p[1] <= hi + 1e-3)
                    && f.positions.iter().any(|p| (p[1] - lo).abs() < 1e-3)
            })
        };
        assert!(reach(&low, 22., 29.) && reach(&high, 29., 32.));
        assert_eq!(rig.faces(&ceiling, &s).len(), 1);
        // Half open the leaves hang on a quarter turn's sine.
        s.bay = 0.5;
        let lowest = rig
            .faces(&big, &s)
            .iter()
            .flat_map(|f| f.positions.clone())
            .map(|p| p[2])
            .fold(f32::MAX, f32::min);
        assert!((lowest - (-1. - 3. * (FRAC_PI_2 as f32 * 0.5).sin())).abs() < 1e-3);
    }
}
