//! Tu-26 Backfire-C (`TU26.SH`) own-source rig (T26-3): the swing wing (13
//! whole faces a side, flap strips included, turned as one rigid panel about
//! the glove-corner vertex), the two whole tailplane quads (pitch plus
//! differential roll), the signed aft rudder strip, the two flap strips, the
//! rigid plate gear, the two afterburner plumes and, interim until T26-5, the
//! bay-door plates. See docs/formats/aircraft-variety.md "Tu-26 shape and rig
//! (T26-2/3, 2026-11-15)".
//!
//! `TU26.SH` ships six state words: 0x7880 the afterburner plumes (four prisms,
//! two per engine), 0x788c the two belly plates (read as the bay doors open,
//! not an airbrake, decision 5), 0x7892 the gear (12 faces), 0x789e and 0x78a4
//! the left and right flap words (each deletes the outer trailing strip and, at
//! -1, draws it drooped) and 0x78aa the rudder (swaps the aft strip of the fin
//! for a copy turned to one side). There is no sweep word, aileron, spoiler,
//! slat, airbrake, canopy or hook word and none is invented; roll is the
//! tailplane's differential. The rig never sets the flap or rudder words: it
//! keeps the strips drawn and turns them whole, so no surface ever vanishes in
//! flight.
//!
//! Every hinge point below is in shape units (x right, y forward, z up, 4/3 ft
//! a unit at the header scale 10), and the shape arrives with the identity
//! per-axis scale, so a panel turned about one of them stays rigid.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::shape::{Face, Shape};
use tore_sim::combat::swing::TU26 as SWING;

const WORDS: [usize; 6] = [0x7880, 0x788c, 0x7892, 0x789e, 0x78a4, 0x78aa];

/// The right swing panel: 13 whole faces, the two flap-strip faces (the first
/// two) included, 9 distinct vertices, all at z -1. Its one shared vertex with
/// the fixed glove is [`PIVOT_R`].
pub(crate) const WING_R: [usize; 13] = [
    0x4b4c, 0x4b73, 0x498f, 0x49b2, 0x49db, 0x49fe, 0x4a21, 0x4a44, 0x4a67, 0x4a8a, 0x4ab3, 0x4ad6,
    0x4af9,
];
/// The left swing panel, likewise (strip faces first).
pub(crate) const WING_L: [usize; 13] = [
    0x4f8d, 0x4fb4, 0x4dd0, 0x4df9, 0x4e1c, 0x4e3f, 0x4e62, 0x4e85, 0x4ea8, 0x4ecb, 0x4ef4, 0x4f17,
    0x4f40,
];
/// The outer trailing strips the flap words delete (0x78a4 right, 0x789e
/// left): two layers each, part of the wing panel and hinged on its own
/// leading edge.
pub(crate) const STRIP_R: [usize; 2] = [0x4b4c, 0x4b73];
pub(crate) const STRIP_L: [usize; 2] = [0x4f8d, 0x4fb4];
/// The tailplane quads: right 5 faces (two windings and three triangles), left
/// 6, no vertex shared with any other face. Roots on x +-5 (the left one has a
/// root vertex at x -4), z -2, tips on x +-13 at z 0.
pub(crate) const TAIL_R: [usize; 5] = [0x4085, 0x40bd, 0x40eb, 0x4118, 0x413c];
pub(crate) const TAIL_L: [usize; 6] = [0x45a7, 0x45dd, 0x4605, 0x462c, 0x464d, 0x4670];
/// The rudder strip (both windings) the rudder word swaps, and the faces the
/// word swaps it for (never drawn).
pub(crate) const RUDDER: [usize; 2] = [0x517d, 0x51a4];
const RUDDER_WORD: [usize; 4] = [0x50a3, 0x50ca, 0x5110, 0x5137];

/// Gear word 0x7892: the right main (a plate at x 3 and a cross plate at y -3,
/// two windings each), the left main and the nose (a plate at x 0 and a cross
/// plate at y 36).
const GEAR_MAIN_R: [usize; 4] = [0x5210, 0x5237, 0x525e, 0x527d];
const GEAR_MAIN_L: [usize; 4] = [0x52e1, 0x5308, 0x532f, 0x534e];
const GEAR_NOSE: [usize; 4] = [0x53b2, 0x53d1, 0x53f0, 0x540f];
pub(crate) const GEAR: [usize; 12] = [
    0x5210, 0x5237, 0x525e, 0x527d, 0x52e1, 0x5308, 0x532f, 0x534e, 0x53b2, 0x53d1, 0x53f0, 0x540f,
];
/// Flame word 0x7880: per engine one vertical plate and one horizontal plate
/// (a cross), both windings, from the nozzle exit plane y -29 to y -53.
pub(crate) const FLAME: [usize; 8] = [
    0x5495, 0x54bc, 0x54e3, 0x550a, 0x5531, 0x5558, 0x557f, 0x55a6,
];
/// The two nozzle exit faces on the plane y -29 (right, left).
const NOZZLE: [usize; 2] = [0x38f2, 0x3919];
const NOZZLE_PLANE: f32 = -29.;
const FLAME_TIP: f32 = -53.;
/// Belly plates word 0x788c: a plate pair each side at x +-2, y 11 to 23
/// (left, then right), hanging 3 to 4 units below the belly line.
const BAY_L: [usize; 4] = [0x47a6, 0x47bd, 0x47d4, 0x47eb];
const BAY_R: [usize; 4] = [0x4711, 0x4728, 0x473f, 0x4756];
pub(crate) const BAY: [usize; 8] = [
    0x47a6, 0x47bd, 0x47d4, 0x47eb, 0x4711, 0x4728, 0x473f, 0x4756,
];
/// The belly line the interim plates drop from (the plates' upper edge).
const BAY_TOP: f32 = -2.;

/// The wing pivots, shape units: the glove's leading-edge corner, the one
/// vertex the panel shares with the fixed airframe (measured, T26-0). The
/// simulation's spec ([`SWING`]) carries the same point for the stations.
pub(crate) const PIVOT_R: [f32; 3] = [14., 3., -1.];
pub(crate) const PIVOT_L: [f32; 3] = [-14., 3., -1.];

/// The wing's turn from the drawn pose, radians: the spec's schedule (spread
/// to 300 knots and with flaps or gear out, full by 560 knots) plus the 1.6
/// degrees the mesh's 18.4 degrees sits under the real 20.
pub(crate) fn sweep_turn(state: &State) -> f64 {
    SWING.turn(state.speed, state.flaps, state.gear)
}

/// Tailplane law (fitted, agent decision 2026-11-15; no real Tu-22M limit was
/// found): the whole panel turns about a lateral spindle through its root
/// chord midpoint, so its aft tip corner, 14.5 units (19.3 ft) behind the
/// spindle, rises or falls by 2 * 14.5 * sin(angle / 2) units. Pitch alone
/// reaches 0.14 rad, roll adds up to 0.08 and the sum is held to 0.20 rad
/// (11.5 degrees, a 3.8 ft travel at the tip corner).
pub(crate) const TAIL_PITCH: f64 = 0.14;
pub(crate) const TAIL_ROLL: f64 = 0.08;
pub(crate) const TAIL_LIMIT: f64 = 0.20;
/// Root chord midpoint of the right tailplane (x 5, y -14 to -29, z -2); the
/// spindle is the lateral line through it, and the left side mirrors it.
pub(crate) const TAIL_SPINDLE: [f32; 3] = [5., -21.5, -2.];
/// Flap droop and rudder travel, radians (fitted). The flap word's drooped
/// copy is a 45 degree droop; 0.40 rad (23 degrees) is the fleet's flap
/// travel (the Rafale's, the Tornado's) and keeps the seam at the strip's
/// outer edge, 4 units times 2 sin(angle / 2), under 1.1 ft. The rudder word
/// is a rigid turn of 0.858 rad (49 degrees) about the strip's leading edge,
/// more than any real rudder; 0.50 rad (29 degrees) keeps the aft edge 1.7
/// units (2.3 ft) off the fin at the base.
pub(crate) const FLAP: f64 = 0.40;
pub(crate) const RUDDER_TRAVEL: f64 = 0.50;
/// The rudder word's own turn, radians, which `load` checks.
const RUDDER_WORD_ANGLE: f64 = 0.858;
/// Strip hinge: the leading edge of the strip, from the root (11, -3, -1)
/// outward to the tip (36, -7, -1). The left side mirrors the pivot and axis.
const STRIP_PIVOT: [f32; 3] = [11., -3., -1.];
const STRIP_AXIS: [f32; 3] = [25., -4., 0.];
const STRIP_PIVOT_L: [f32; 3] = [-11., -3., -1.];
const STRIP_AXIS_L: [f32; 3] = [-25., -4., 0.];
/// Rudder hinge: the leading edge of the strip, from (0, -24, 9) up and aft to
/// (0, -31, 21). A positive angle swings the trailing edge toward +X (right).
const RUDDER_HINGE: [f32; 3] = [0., -24., 9.];
const RUDDER_AXIS: [f32; 3] = [0., -7., 12.];

/// Main gear hinge (fitted, agent decision 2026-11-15): a line along Y at the
/// outside of the fuselage side wall, x +-5, z -1. The legs swing inward (the
/// real direction, "pivot inwards ... into bottom of fuselage") through a
/// quarter turn and lie flat at z 1 inside the 10 by 8 unit fuselage section;
/// every vertex of both legs stows inside it (checked against the cross
/// section at y -8 to 2: x +-5 from z -3 to 3, top z 4, keel z -4).
const MAIN_HINGE_R: [f32; 3] = [5., 0., -1.];
const MAIN_HINGE_L: [f32; 3] = [-5., 0., -1.];
/// Nose leg: a lateral axis at the plate's forward edge (y 38, z -4), folding
/// aft (the leg lies along the belly in the 4 unit wide section), and a vertical
/// steering axis through the cross of the two plates (y 36).
const NOSE_HINGE: [f32; 3] = [0., 38., -4.];
const NOSE_STEER: [f32; 3] = [0., 36., 0.];
/// Gear timing (fitted): retracting, the nose leg folds first (gear 1 to
/// 0.3), then the mains (0.7 to 0); extending, the mains come down first.
const NOSE_START: f64 = 1.0;
const NOSE_SPAN: f64 = 0.7;
const MAIN_START: f64 = 0.7;
const MAIN_SPAN: f64 = 0.7;

fn fold(gear: f64, start: f64, span: f64) -> f64 {
    FRAC_PI_2 * ((start - gear.clamp(0., 1.)) / span).clamp(0., 1.)
}

pub(crate) fn wing_right(address: usize) -> bool {
    WING_R.contains(&address)
}
pub(crate) fn wing_left(address: usize) -> bool {
    WING_L.contains(&address)
}
pub(crate) fn strip(address: usize) -> bool {
    STRIP_R.contains(&address) || STRIP_L.contains(&address)
}
pub(crate) fn tail_right(address: usize) -> bool {
    TAIL_R.contains(&address)
}
pub(crate) fn tail_left(address: usize) -> bool {
    TAIL_L.contains(&address)
}

/// One tailplane angle in radians, positive lowering the trailing edge: pitch
/// raises both trailing edges for a positive elevator, a positive aileron raises
/// the right and lowers the left, and the sum is clamped to the limit.
pub(crate) fn tail_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    (-TAIL_PITCH * state.elevator.clamp(-1., 1.) - side * TAIL_ROLL * state.aileron.clamp(-1., 1.))
        .clamp(-TAIL_LIMIT, TAIL_LIMIT)
}

/// Whole-piece gear motion. The mains turn inward about a line along Y, the
/// nose leg steers about its vertical axis and folds aft about a lateral line.
/// Gear down (1) is the source pose; hidden at gear 0 by the caller.
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let a = f.address;
    if GEAR_NOSE.contains(&a) {
        if steering != 0. {
            turn(f, NOSE_STEER, [0., 0., 1.], -steering);
        }
        turn(
            f,
            NOSE_HINGE,
            [1., 0., 0.],
            -fold(gear, NOSE_START, NOSE_SPAN),
        );
    } else if GEAR_MAIN_R.contains(&a) {
        turn(
            f,
            MAIN_HINGE_R,
            [0., 1., 0.],
            fold(gear, MAIN_START, MAIN_SPAN),
        );
    } else if GEAR_MAIN_L.contains(&a) {
        turn(
            f,
            MAIN_HINGE_L,
            [0., 1., 0.],
            -fold(gear, MAIN_START, MAIN_SPAN),
        );
    }
}

/// The wingtip vapor attachment of one side (0 left, 1 right) for a wing turn,
/// feet right, up and forward of the aircraft. The shape's streamer record
/// (third-of-a-foot CE units, hinge scale 0, its pivot added) puts the spread
/// point 52 ft out, 1.3 ft down and 10.7 ft aft: the drawn wing's tip plane
/// (z -1 is 1.3 ft down), one unit (1.3 ft) outboard of the tip edge at x 38
/// and level with its trailing half (y -5 to -9), so no fix is needed at the
/// spread pose. The record has no hinge, so the point is carried by the same
/// rigid turn about the pivot as the wing faces and leaves the drawn tip at
/// every sweep (T26-3).
pub(crate) fn streamer_point(
    def: &tore_formats::shape::StreamerDef,
    side: usize,
    turn: f64,
) -> Option<[f64; 3]> {
    let record = def.attachment(side, 0).ok()?.map(|v| v / 3.);
    Some(streamer_mount(record, side == 1, turn))
}

/// The record's point (feet right, up, forward, the spread pose) carried by the
/// wing's turn.
fn streamer_mount(record: [f64; 3], right: bool, turn: f64) -> [f64; 3] {
    SWING.swept(right, record, turn)
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("TU26.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("TU26.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

/// A state word set to `value` adds exactly `added` and removes exactly
/// `removed`.
fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    (word, value): (usize, i32),
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
        return Err(format!("unreviewed TU26.SH branch {word:x}").into());
    }
    Ok(pose)
}

fn bits(p: &[f32; 3]) -> [u32; 3] {
    p.map(f32::to_bits)
}

/// The turn about the line `bottom` to `top` that carries a closed free vertex
/// to the word's, radians right-handed about bottom to top.
fn turn_angle(bottom: [f32; 3], top: [f32; 3], from: [f32; 3], to: [f32; 3]) -> f64 {
    let sub = |a: [f32; 3], b: [f32; 3]| -> [f64; 3] {
        std::array::from_fn(|k| f64::from(a[k]) - f64::from(b[k]))
    };
    let dot = |a: [f64; 3], b: [f64; 3]| -> f64 { a.iter().zip(b).map(|(x, y)| x * y).sum() };
    let axis = sub(top, bottom);
    let length = dot(axis, axis).sqrt();
    let u = axis.map(|v| v / length);
    let perp = |p: [f32; 3]| {
        let v = sub(p, bottom);
        let along = dot(v, u);
        std::array::from_fn::<f64, 3, _>(|k| v[k] - along * u[k])
    };
    let (v, w) = (perp(from), perp(to));
    let cross = [
        v[1] * w[2] - v[2] * w[1],
        v[2] * w[0] - v[0] * w[2],
        v[0] * w[1] - v[1] * w[0],
    ];
    dot(u, cross).atan2(dot(v, w))
}

pub struct Rig;
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 26812
            || shape.faces.len() != 310
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed TU26.SH layout".into());
        }
        // Each wing panel: 13 whole faces, exactly one vertex (the glove
        // corner, the pivot) shared with any face outside the panel, the strip
        // faces hinged on the panel's own leading edge (rule 11).
        for (ids, pivot, side) in [(&WING_R, PIVOT_R, 1f32), (&WING_L, PIVOT_L, -1f32)] {
            let mut inside = BTreeSet::new();
            for &address in ids {
                let face = one(&shape, address)?;
                if face.positions.iter().any(|p| p[2] != -1.) {
                    return Err(format!("unreviewed TU26.SH wing face {address:x}").into());
                }
                inside.extend(face.positions.iter().map(bits));
            }
            let outside: BTreeSet<_> = shape
                .faces
                .iter()
                .filter(|f| !WING_R.contains(&f.address) && !WING_L.contains(&f.address))
                .flat_map(|f| f.positions.iter().map(bits))
                .collect();
            let shared: Vec<_> = inside.intersection(&outside).collect();
            if inside.len() != 9 || shared != [&bits(&pivot)] {
                return Err("unreviewed TU26.SH wing panel (vertices or pivot)".into());
            }
            for &address in if side > 0. { &STRIP_R } else { &STRIP_L } {
                let face = one(&shape, address)?;
                let (root, tip) = ([side * 11., -3., -1.], [side * 36., -7., -1.]);
                if face.positions.len() != 4
                    || !face.positions.contains(&root)
                    || !face.positions.contains(&tip)
                {
                    return Err(format!("unreviewed TU26.SH flap strip {address:x}").into());
                }
            }
        }
        // The pivots the simulation carries stations on are the same points.
        if SWING.pivot_right[..2] != PIVOT_R.map(f64::from)[..2]
            || SWING.pivot_left[..2] != PIVOT_L.map(f64::from)[..2]
        {
            return Err("TU26.SH wing pivot differs from the swing spec".into());
        }
        // Tailplane: whole panels at z -2 to 0 sharing no vertex with any other
        // face, the root on x +-5 (one left vertex on x -4), the tip on x +-13.
        let others: BTreeSet<_> = shape
            .faces
            .iter()
            .filter(|f| !TAIL_R.contains(&f.address) && !TAIL_L.contains(&f.address))
            .flat_map(|f| f.positions.iter().map(bits))
            .collect();
        for (ids, side) in [(&TAIL_R[..], 1f32), (&TAIL_L[..], -1f32)] {
            for &address in ids {
                let face = one(&shape, address)?;
                if face
                    .positions
                    .iter()
                    .any(|p| p[2] < -2. || p[2] > 0. || p[0] * side < 4. || p[0] * side > 13.)
                    || face.positions.iter().any(|p| others.contains(&bits(p)))
                {
                    return Err(format!("unreviewed TU26.SH tailplane face {address:x}").into());
                }
            }
        }
        // The rudder strip: a quad in the fin plane whose leading edge is the hinge.
        for &address in &RUDDER {
            let face = one(&shape, address)?;
            if face.positions.len() != 4
                || !face.positions.iter().all(|p| p[0] == 0.)
                || !face.positions.contains(&RUDDER_HINGE)
                || !face.positions.contains(&[0., -31., 21.])
            {
                return Err(format!("unreviewed TU26.SH rudder strip {address:x}").into());
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words delete exactly the strips (0x789e left, 0x78a4 right).
        branch(bytes, &original, (0x789e, 1), &[], &STRIP_L)?;
        branch(bytes, &original, (0x78a4, 1), &[], &STRIP_R)?;
        // The rudder word swaps the strip for a copy turned about its leading
        // edge: the turn is the one this rig's travel is fitted under, and -1
        // is its mirror.
        for (value, added, sign) in [(1, &RUDDER_WORD[..2], 1.), (-1, &RUDDER_WORD[2..], -1.)] {
            let pose = branch(bytes, &original, (0x78aa, value), added, &RUDDER)?;
            let angle = turn_angle(
                RUDDER_HINGE,
                [0., -31., 21.],
                [0., -28., 9.],
                [3. * sign as f32, -27., 9.],
            );
            if (angle - sign * RUDDER_WORD_ANGLE).abs() > 0.01 {
                return Err("unreviewed TU26.SH rudder word angle".into());
            }
            if !pose
                .faces
                .iter()
                .filter(|f| added.contains(&f.address))
                .any(|f| f.positions.contains(&[3. * sign as f32, -27., 9.]))
            {
                return Err("unreviewed TU26.SH rudder word corner".into());
            }
        }
        // The gear, flame and bay words add exactly the reviewed faces and
        // remove nothing; every plate's corners are checked against the reviewed
        // source box, so the rig refuses a shape that differs.
        let gear = branch(bytes, &original, (0x7892, 1), &GEAR, &[])?;
        let flame = branch(bytes, &original, (0x7880, 1), &FLAME, &[])?;
        let bay = branch(bytes, &original, (0x788c, 1), &BAY, &[])?;
        let inside = |face: &Face, lo: [f32; 3], hi: [f32; 3]| {
            face.positions.len() == 4
                && face
                    .positions
                    .iter()
                    .all(|p| (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k]))
        };
        for (ids, lo, hi) in [
            (&GEAR_MAIN_R[..], [1., -8., -10.], [5., 2., -2.]),
            (&GEAR_MAIN_L[..], [-5., -8., -10.], [-1., 2., -2.]),
            (&GEAR_NOSE[..], [-1., 34., -11.], [1., 38., -4.]),
            (&BAY_R[..], [2., 11., -5.], [2., 23., -1.]),
            (&BAY_L[..], [-2., 11., -5.], [-2., 23., -1.]),
        ] {
            let pose = if ids.len() == 4 && lo[1] == 11. {
                &bay
            } else {
                &gear
            };
            for &address in ids {
                if !inside(one(pose, address)?, lo, hi) {
                    return Err(format!("unreviewed TU26.SH plate {address:x}").into());
                }
            }
        }
        // Each nozzle is one exit octagon on the plane y -29; the plume plates
        // run from it to y -53.
        for (address, side) in NOZZLE.into_iter().zip([1f32, -1.]) {
            let face = one(&shape, address)?;
            if face.positions.len() != 8
                || !face.positions.iter().all(|p| p[1] == NOZZLE_PLANE)
                || !face
                    .positions
                    .iter()
                    .all(|p| p[0] * side >= 0. && p[0] * side <= 4.)
            {
                return Err(format!("unreviewed TU26.SH nozzle {address:x}").into());
            }
        }
        let (mut root, mut tip) = (f32::NEG_INFINITY, f32::INFINITY);
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4 {
                return Err(format!("unreviewed TU26.SH plume plate {address:x}").into());
            }
            for p in &face.positions {
                root = root.max(p[1]);
                tip = tip.min(p[1]);
            }
        }
        if root != NOZZLE_PLANE || tip != FLAME_TIP {
            return Err("unreviewed TU26.SH plume extent".into());
        }
        shape.faces.extend(
            gear.faces
                .into_iter()
                .filter(|f| GEAR.contains(&f.address))
                .chain(bay.faces.into_iter().filter(|f| BAY.contains(&f.address)))
                .chain(
                    flame
                        .faces
                        .into_iter()
                        .filter(|f| FLAME.contains(&f.address)),
                ),
        );
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        let right = wing_right(a);
        if right || wing_left(a) {
            // The trailing strips droop about their leading edge first, in the
            // wing's own frame, then the whole panel (strips included) turns
            // about the glove pivot.
            let flaps = state.flaps.clamp(0., 1.);
            if flaps > 0. && strip(a) {
                let (pivot, axis, angle) = if right {
                    (STRIP_PIVOT, STRIP_AXIS, FLAP * flaps)
                } else {
                    (STRIP_PIVOT_L, STRIP_AXIS_L, -FLAP * flaps)
                };
                turn(&mut result, pivot, axis, angle);
            }
            let swept = sweep_turn(state);
            if swept != 0. {
                turn(
                    &mut result,
                    if right { PIVOT_R } else { PIVOT_L },
                    [0., 0., 1.],
                    if right { -swept } else { swept },
                );
            }
        } else if tail_right(a) || tail_left(a) {
            // The whole panel turns about the lateral spindle through the root
            // chord midpoint: no vertex is held while another turns.
            let right = tail_right(a);
            let angle = tail_angle(right, state);
            if angle != 0. {
                let side = if right { 1. } else { -1. };
                turn(
                    &mut result,
                    [side * TAIL_SPINDLE[0], TAIL_SPINDLE[1], TAIL_SPINDLE[2]],
                    [1., 0., 0.],
                    angle,
                );
            }
        } else if RUDDER.contains(&a) {
            // One rigid strip about its leading edge: the two vertices on that
            // edge stay, the rest swing out of the fin plane.
            let angle = RUDDER_TRAVEL * state.rudder.clamp(-1., 1.);
            if angle != 0. {
                turn(&mut result, RUDDER_HINGE, RUDDER_AXIS, angle);
            }
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
        } else if BAY.contains(&a) {
            // Interim bay-door pose (T26-5 cuts the real bay): the plates hang
            // from the belly line as the bay opens.
            let bay = state.bay.clamp(0., 1.) as f32;
            if bay <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[2] = BAY_TOP + (p[2] - BAY_TOP) * bay;
            }
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            // The plume grows from the nozzle exit plane; every plate scales
            // from the same root, so the shared edges stay together.
            for p in &mut result.positions {
                p[1] = NOZZLE_PLANE + (p[1] - NOZZLE_PLANE) * state.exhaust.clamp(0., 1.) as f32;
            }
        }
        Some(result)
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
    fn face(address: usize, points: &[[f32; 3]]) -> Face {
        Face {
            address,
            colors: vec![31; points.len()],
            uv: vec![[0.2, 0.8]; points.len()],
            positions: points.to_vec(),
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([0., 0., 1.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn distance(p: [f32; 3], q: [f32; 3]) -> f32 {
        (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt()
    }
    fn spread(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
        let mut worst = 0f32;
        for i in 0..a.len() {
            for j in i + 1..a.len() {
                worst = worst.max((distance(a[i], a[j]) - distance(b[i], b[j])).abs());
            }
        }
        worst
    }
    fn knots(s: &mut State, kt: f64) {
        s.speed = kt * 1.68781;
    }
    /// The panel's turn about the glove-corner pivot gives the leading edge's
    /// sweep: the angle of the panel's own leading edge, (14, 3) to (38, -5) at
    /// the drawn pose, plus the turn, degrees from the lateral axis.
    fn leading_sweep(turned: &[[f32; 3]]) -> f64 {
        let (dx, dy) = (
            f64::from(turned[1][0] - turned[0][0]),
            f64::from(turned[0][1] - turned[1][1]),
        );
        dy.atan2(dx).to_degrees()
    }

    /// One test for the whole rig, on synthetic faces named in shape units: the
    /// swing wing (the 1.6 degree rest offset, the schedule's 300 and 560 knot
    /// ends, flaps and gear holding it spread, a 65.8 ft swept span, the
    /// pivot staying and the seam it leaves), the flap strip, the tailplane, the
    /// rudder, the gear and the interim plates, the plume and the vapor point.
    #[test]
    fn the_tu26_rig_turns_whole_panels_about_their_hinges() {
        let rig = Rig;
        let moved = |s: &State, f: &Face| rig.animate(f, s).unwrap();
        let mut s0 = state();
        s0.speed = 0.;

        // Wing, right panel: the tip edge (38, -5) and the leading edge from the
        // pivot; at rest the turn is the 1.6 degrees that lift the mesh's 18.4
        // degrees to the real 20, spread holds to 300 knots, the sweep is
        // linear and full (65 degrees) by 560 knots, and flaps or gear hold it
        // spread whatever the speed.
        let panel = face(0x4a44, &[[14., 3., -1.], [38., -5., -1.], [37., -6., -1.]]);
        let mut s = s0.clone();
        let rest = moved(&s, &panel);
        assert_eq!(rest.positions[0], panel.positions[0], "pivot stays");
        assert!((leading_sweep(&rest.positions) - 20.).abs() < 0.05);
        knots(&mut s, 300.);
        assert_eq!(moved(&s, &panel).positions, rest.positions);
        knots(&mut s, 430.);
        let half = leading_sweep(&moved(&s, &panel).positions);
        assert!((half - (20. + 45. / 2.)).abs() < 0.2, "half sweep {half}");
        knots(&mut s, 560.);
        let full = moved(&s, &panel);
        assert!((leading_sweep(&full.positions) - 65.).abs() < 0.05);
        assert!(spread(&panel.positions, &full.positions) < 1e-3, "rigid");
        knots(&mut s, 900.);
        assert_eq!(moved(&s, &panel).positions, full.positions, "no more");
        s.flaps = 1.;
        assert_eq!(moved(&s, &panel).positions, rest.positions, "flaps hold");
        s.flaps = 0.;
        s.gear = 1.;
        assert_eq!(moved(&s, &panel).positions, rest.positions, "gear holds");
        // Swept span: the tip goes from 38 units out to 24.66, a 65.8 ft span
        // against the spread 101.3 ft (0.649, the real ratio 0.680).
        let (tip_x, tip_y) = (full.positions[1][0], full.positions[1][1]);
        assert!((tip_x - 24.66).abs() < 0.05 && (tip_y + 19.94).abs() < 0.05);
        assert!((2. * tip_x * 4. / 3. - 65.8).abs() < 0.2);
        // The seam: the second leading root vertex (14, 2) moves under a foot
        // from where it was, and the trailing root corners swing into the glove
        // slab (x 7.6 and 4.0, z -1 under the glove's top at 0 to -1).
        s.gear = 0.;
        knots(&mut s, 560.);
        let root = [
            face(0x4a21, &[[14., 3., -1.], [37., -6., -1.], [14., 2., -1.]]),
            face(
                0x4af9,
                &[
                    [11., -3., -1.],
                    [36., -7., -1.],
                    [37., -6., -1.],
                    [14., 2., -1.],
                ],
            ),
            face(
                0x4b73,
                &[
                    [36., -9., -1.],
                    [36., -7., -1.],
                    [11., -3., -1.],
                    [10., -7., -1.],
                ],
            ),
        ];
        let seam = moved(&s, &root[0]).positions[2];
        let seam_ft = distance(seam, [14., 2., -1.]) * 4. / 3.;
        assert!(seam_ft < 1.2, "root seam {seam_ft} ft");
        let corner = moved(&s, &root[2]).positions;
        assert!(corner[2][0] > 7.4 && corner[2][0] < 7.8, "{:?}", corner[2]);
        assert!(corner[3][0] > 3.8 && corner[3][0] < 4.2, "{:?}", corner[3]);
        // The left panel is the mirror.
        let left = face(
            0x4e1c,
            &[[-14., 3., -1.], [-14., 2., -1.], [-37., -6., -1.]],
        );
        let l = moved(&s, &left).positions;
        let r = moved(
            &s,
            &face(0x4a21, &[[14., 3., -1.], [14., 2., -1.], [37., -6., -1.]]),
        )
        .positions;
        for k in 0..3 {
            assert!((l[k][0] + r[k][0]).abs() < 1e-4 && (l[k][1] - r[k][1]).abs() < 1e-4);
        }
        // The vapor point follows the drawn tip: the record's 52 ft out, 10.7 ft
        // aft at rest (turned 1.6 degrees), 30.9 ft out and 30.3 ft aft at full
        // sweep, the left side the mirror.
        let record = [52., -1.33, -10.67];
        let [x0, _, f0] = streamer_mount(record, true, SWING.turn(0., 0., 0.));
        assert!(
            (x0 - 51.6).abs() < 0.2 && (f0 + 11.6).abs() < 0.3,
            "{x0} {f0}"
        );
        let full_turn = SWING.turn(1000., 0., 0.);
        let [x1, _, f1] = streamer_mount(record, true, full_turn);
        let [xl, _, fl] = streamer_mount([-52., -1.33, -10.67], false, full_turn);
        assert!(
            (x1 - 30.9).abs() < 0.2 && (f1 + 30.3).abs() < 0.3,
            "{x1} {f1}"
        );
        assert!((xl + x1).abs() < 1e-9 && (fl - f1).abs() < 1e-9);

        // Flap strip: drooped whole about its leading edge by up to 0.40 rad,
        // the hinge vertices stay at the root and the tip, the left mirrors.
        let strip_r = face(
            0x4b73,
            &[
                [36., -9., -1.],
                [36., -7., -1.],
                [11., -3., -1.],
                [10., -7., -1.],
            ],
        );
        let strip_l = face(
            0x4f8d,
            &[
                [-11., -3., -1.],
                [-10., -7., -1.],
                [-36., -9., -1.],
                [-36., -7., -1.],
            ],
        );
        // The strips ride the wing's 1.6 degree rest turn: compare with that pose.
        let rest_r = moved(&s0, &strip_r);
        let rest_l = moved(&s0, &strip_l);
        let mut s = s0.clone();
        s.flaps = 1.;
        let down = moved(&s, &strip_r);
        assert!(spread(&strip_r.positions, &down.positions) < 1e-3);
        assert!(distance(down.positions[1], rest_r.positions[1]) < 1e-3);
        assert!(distance(down.positions[2], rest_r.positions[2]) < 1e-3);
        let drop = rest_r.positions[0][2] - down.positions[0][2];
        assert!((drop - 1.976 * 0.40f32.sin()).abs() < 0.02, "droop {drop}");
        let down_l = moved(&s, &strip_l);
        assert!(down_l.positions[2][2] < rest_l.positions[2][2] - 0.5);
        assert!(distance(down_l.positions[0], rest_l.positions[0]) < 1e-3);
        s.flaps = 0.;
        assert_eq!(moved(&s, &strip_r).positions, rest_r.positions);

        // Tailplane: a pull raises both trailing edges, roll is differential,
        // the root chord midpoint stays and the panel is rigid within the limit.
        let tail = face(
            0x40eb,
            &[
                [13., -29., 0.],
                [5., -18., -2.],
                [5., -27., -2.],
                [5., -29., -2.],
                [13., -36., 0.],
            ],
        );
        let tail_l = face(
            0x45dd,
            &[
                [-5., -18., -2.],
                [-4., -27., -2.],
                [-5., -29., -2.],
                [-13., -36., 0.],
            ],
        );
        let mut s = s0.clone();
        s.elevator = 1.;
        let up = moved(&s, &tail);
        assert!(spread(&tail.positions, &up.positions) < 1e-3);
        let rise = up.positions[4][2] - tail.positions[4][2];
        assert!(
            (rise - 14.5 * TAIL_PITCH.sin() as f32).abs() < 0.06 && rise > 0.,
            "{rise}"
        );
        assert!(moved(&s, &tail_l).positions[3][2] > tail_l.positions[3][2]);
        s.elevator = 5.;
        assert_eq!(moved(&s, &tail).positions, up.positions, "clamped");
        s.elevator = 0.;
        s.aileron = 1.;
        let right = moved(&s, &tail).positions[4][2] - tail.positions[4][2];
        let left = moved(&s, &tail_l).positions[3][2] - tail_l.positions[3][2];
        assert!(right > 0. && left < 0., "differential {right} {left}");
        s.elevator = 1.;
        let full = moved(&s, &tail).positions[4][2] - tail.positions[4][2];
        assert!(
            (full - 14.5 * TAIL_LIMIT.sin() as f32).abs() < 0.06,
            "limit {full}"
        );
        s.elevator = 0.;
        s.aileron = 0.;
        assert_eq!(moved(&s, &tail).positions, tail.positions);

        // Rudder: the leading edge stays, the aft edge goes to +X for a positive
        // command and -X for a negative one, within the travel.
        let rudder = face(
            0x517d,
            &[
                [0., -28., 9.],
                [0., -24., 9.],
                [0., -31., 21.],
                [0., -33., 21.],
            ],
        );
        let mut s = s0.clone();
        s.rudder = 1.;
        let to_right = moved(&s, &rudder);
        assert!(spread(&rudder.positions, &to_right.positions) < 1e-3);
        for k in [1, 2] {
            assert!(distance(to_right.positions[k], rudder.positions[k]) < 1e-3);
        }
        let x = to_right.positions[0][0];
        assert!((1.6..1.8).contains(&x), "base aft vertex x {x}");
        s.rudder = -1.;
        assert!((moved(&s, &rudder).positions[0][0] + x).abs() < 1e-3);
        s.rudder = 0.;
        assert_eq!(moved(&s, &rudder).positions, rudder.positions);

        // Gear: hidden at 0, the source pose at 1, whole legs between; at 0.05
        // each main leg is inside the fuselage section (x +-5, z -3 to 4) and
        // the nose leg inside the nose (x +-2, z -4 to 4).
        let main = [
            face(
                0x5210,
                &[
                    [3., -8., -10.],
                    [3., -8., -2.],
                    [3., 2., -2.],
                    [3., 2., -10.],
                ],
            ),
            face(
                0x525e,
                &[
                    [5., -3., -10.],
                    [5., -3., -2.],
                    [1., -3., -2.],
                    [1., -3., -10.],
                ],
            ),
        ];
        let nose = [
            face(
                0x53b2,
                &[
                    [0., 34., -11.],
                    [0., 38., -11.],
                    [0., 38., -4.],
                    [0., 34., -4.],
                ],
            ),
            face(
                0x53f0,
                &[
                    [-1., 36., -11.],
                    [1., 36., -11.],
                    [1., 36., -4.],
                    [-1., 36., -4.],
                ],
            ),
        ];
        let mut s = s0.clone();
        s.gear = 0.;
        assert!(rig.animate(&main[0], &s).is_none());
        s.gear = 1.;
        for f in main.iter().chain(&nose) {
            assert_eq!(moved(&s, f).positions, f.positions);
        }
        s.gear = 0.5;
        for f in main.iter() {
            let g = moved(&s, f);
            assert!(spread(&f.positions, &g.positions) < 1e-3);
            assert!(g.positions.iter().any(|p| p[2] > -10.));
        }
        s.gear = 0.02;
        for f in main.iter() {
            for p in moved(&s, f).positions {
                assert!(
                    p[0].abs() <= 5.2 && p[2] >= -3.2 && p[2] <= 4.,
                    "main {p:?}"
                );
            }
        }
        for f in &nose {
            for p in moved(&s, f).positions {
                assert!(p[0].abs() <= 2. && p[2] >= -4.1 && p[2] <= 4., "nose {p:?}");
            }
        }
        // Steering swings the nose leg about its own axis only on the ground.
        let mut s = s0.clone();
        s.gear = 1.;
        assert_eq!(moved(&s, &nose[0]).positions, nose[0].positions);
        // A mirror: the left main folds toward the centre line too.
        let left_main = face(
            0x52e1,
            &[
                [-3., -8., -2.],
                [-3., 2., -2.],
                [-3., 2., -10.],
                [-3., -8., -10.],
            ],
        );
        s.gear = 0.02;
        for p in moved(&s, &left_main).positions {
            assert!(p[0] >= -5.2 && p[2] <= 4., "left main {p:?}");
        }

        // Bay plates: hidden shut, the source pose open, dropping from the belly
        // line between. Plume: grows from the nozzle plane, hidden at 0.
        let plate = face(
            0x47a6,
            &[
                [-2., 11., -5.],
                [-2., 11., -2.],
                [-2., 18., -2.],
                [-2., 18., -5.],
            ],
        );
        let mut s = s0.clone();
        assert!(rig.animate(&plate, &s).is_none());
        s.bay = 1.;
        assert_eq!(moved(&s, &plate).positions, plate.positions);
        s.bay = 0.5;
        assert!((moved(&s, &plate).positions[0][2] + 3.5).abs() < 1e-6);
        let flame = face(
            0x5495,
            &[
                [-2., -53., -1.],
                [-2., -53., 2.],
                [-2., -29., 2.],
                [-2., -29., -1.],
            ],
        );
        let mut s = s0.clone();
        assert!(rig.animate(&flame, &s).is_none());
        s.exhaust = 1.;
        assert_eq!(moved(&s, &flame).positions, flame.positions);
        s.exhaust = 0.5;
        assert_eq!(moved(&s, &flame).positions[0][1], -41.);
        assert_eq!(moved(&s, &flame).positions[2][1], -29.);
    }
}
