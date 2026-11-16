//! Tu-160 Blackjack (`TU160.SH`) own-source rig (T160-3): the swing wing, the two
//! whole-piece all-moving tailplanes, the source rudder strip, the two trailing
//! flap strips, the keel plates, the rigid gear plates and the four afterburner
//! plumes. See docs/formats/aircraft-variety.md "Tu-160 shape and rig (T160-2/3,
//! 2026-11-15)".
//!
//! `TU160.SH` ships six state words: 0x7ff0 the afterburner flames (16 faces),
//! 0x7ffc two ventral keel plates (4 faces), 0x8002 the gear (12 plates), 0x800e
//! and 0x8014 the left and right flap words (signed: 1 deletes the trailing
//! strip, -1 swaps it for a drooped copy) and 0x801a the rudder-strip word
//! (signed: +/-1 swaps the aft two-unit strip of the upper fin for a copy skewed
//! out 45 degrees, 2 or more deletes it). There is no sweep word: the wing is one
//! fixed spread pose, 16 whole faces a side that share no vertex with the fixed
//! glove, so the sweep is a **fitted rig** that turns the whole panel (strip
//! included) about the `swing::TU160` pivot. The rig never sets any state word: it
//! keeps the strips and the rudder strip drawn and turns them, so no panel
//! vanishes in flight. There is no aileron, spoiler, slat, fence, hook, bay-door
//! or probe geometry and none is invented.
//!
//! Every number below is a shape unit (8/3 ft, the header scale; the Tu-160 has
//! the identity per-axis scale) and is passed through [`scale_point`], so a
//! panel turned about a hinge in the scaled shape stays rigid.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};
use tore_sim::combat::swing;

const ID: AircraftId = AircraftId::Tu160;
const WORDS: [usize; 6] = [0x7ff0, 0x7ffc, 0x8002, 0x800e, 0x8014, 0x801a];

/// The right wing panel: 16 whole faces, the flap strip (the last two) included.
/// None shares a vertex with a fixed face; the root hangs 1 unit off the glove.
pub(crate) const WING_R: [usize; 16] = [
    0x4952, 0x4975, 0x498e, 0x49af, 0x49d6, 0x49f9, 0x4a22, 0x4a49, 0x4a6c, 0x4a8f, 0x4ab0, 0x4ad3,
    0x4b1c, 0x4b45, 0x4b93, 0x4bba,
];
/// The left wing panel: 16 whole faces, the flap strip (the last two) included.
pub(crate) const WING_L: [usize; 16] = [
    0x4e8e, 0x4eaf, 0x4ed8, 0x4f01, 0x4f24, 0x4f47, 0x4f60, 0x4f87, 0x4faa, 0x4fcd, 0x4fee, 0x5011,
    0x5040, 0x5067, 0x50b4, 0x50db,
];
/// The trailing flap strips the flap words delete or droop (0x8014 right, 0x800e
/// left): two layers each, the whole outer trailing edge, part of the wing panel.
pub(crate) const STRIP_R: [usize; 2] = [0x4b93, 0x4bba];
pub(crate) const STRIP_L: [usize; 2] = [0x50b4, 0x50db];
/// The drooped copies the flap words add (read for the end caps' look only).
const DROOP_R: [usize; 4] = [0x4c5a, 0x4c81, 0x4ca8, 0x4cbd];
const DROOP_L: [usize; 4] = [0x516f, 0x5196, 0x51bd, 0x51d2];
/// The all-moving tailplanes: one quad per side at z 7, two windings each.
pub(crate) const TAIL_R: [usize; 2] = [0x1daf, 0x3e5d];
pub(crate) const TAIL_L: [usize; 2] = [0x1dce, 0x3e7c];
/// The upper fin's rudder strip (the aft two units, both windings) and the two
/// faces the rudder word's +1 pose swaps in.
pub(crate) const RUDDER: [usize; 2] = [0x52e0, 0x5307];
const RUDDER_WORD_R: [usize; 2] = [0x5206, 0x522d];
const RUDDER_WORD_L: [usize; 2] = [0x5273, 0x529a];
/// Keel plates (word 0x7ffc): one plate a side, two windings each.
pub(crate) const KEEL: [usize; 4] = [0x4739, 0x4750, 0x4794, 0x47ab];
/// Gear (word 0x8002): the nose leg (two plates and a cap) and a main leg a side
/// (a long plate and a cross plate, two windings each).
const NOSE: [usize; 4] = [0x5515, 0x5534, 0x5553, 0x5572];
const MAIN_R: [usize; 4] = [0x5373, 0x539a, 0x53c1, 0x53e0];
const MAIN_L: [usize; 4] = [0x5444, 0x546b, 0x5492, 0x54b1];
pub(crate) const GEAR: [usize; 12] = [
    0x5515, 0x5534, 0x5553, 0x5572, 0x5373, 0x539a, 0x53c1, 0x53e0, 0x5444, 0x546b, 0x5492, 0x54b1,
];
/// Flame word 0x7ff0: four prisms (16 faces), one per engine, behind the nozzle
/// plane at y -11.
pub(crate) const FLAME: [usize; 16] = [
    0x5658, 0x567f, 0x56a6, 0x56cd, 0x56f4, 0x571b, 0x5742, 0x5769, 0x5790, 0x57b7, 0x57de, 0x5805,
    0x582c, 0x5853, 0x587a, 0x58a1,
];
const NOZZLE_PLANE: f32 = -11.;
const FLAME_TIP: f32 = -22.;

/// End caps of the drooped flap strips (T160-3): the source's drooped copy adds
/// one triangle at each end of the strip; here the cap is the strip's own end
/// triangle, turned with it, with both windings. Synthetic addresses past the
/// module's last state word. Right root, right tip, left root, left tip, each
/// as a front and a back winding.
const CAPS: [usize; 8] = [
    0x8110, 0x8111, 0x8112, 0x8113, 0x8114, 0x8115, 0x8116, 0x8117,
];

// Wing pivots are the swing spec's own (shape units, vertical axis).
fn pivot(right: bool) -> [f32; 3] {
    let p = if right {
        swing::TU160.pivot_right
    } else {
        swing::TU160.pivot_left
    };
    scale_point(ID, p.map(|v| v as f32))
}

/// Tailplane law, fitted (agent decision, 2026-11-15): the real horizontal tail
/// is an all-moving stabilator pair for pitch and roll (`TU160.INF`); no
/// deflection limit is cited. A pitch of 0.25 rad and a roll of 0.12 rad, summed
/// and clamped at 0.30 rad (17 degrees), smaller than the fleet's 0.40 clamp as
/// the Tornado's tailerons are: the panel turns about a lateral spindle
/// through its root chord midpoint with its root corners 4 units (10.7 ft) fore
/// and aft of it, so 0.30 rad moves them 1.2 units (3.2 ft) in z.
pub(crate) const TAIL_PITCH: f64 = 0.25;
pub(crate) const TAIL_ROLL: f64 = 0.12;
pub(crate) const TAIL_LIMIT: f64 = 0.30;
/// Root chord midpoint of the right tailplane (x 0, y -26 to -18, z 7); the
/// spindle is the lateral line through it and the left side shares it.
pub(crate) const TAIL_SPINDLE: [f32; 3] = [0., -22., 7.];
/// Flap droop, radians, fitted (agent decision, 2026-11-15). The source's drooped
/// copy measures 0.73 rad at the strip root (the trailing edge 3 units down) and is
/// not an exact rotation; 0.60 rad (34 degrees) is a rigid droop inside it.
pub(crate) const FLAP: f64 = 0.60;
/// Rudder-strip travel, radians, fitted (agent decision, 2026-11-15): the
/// retail word's pose is a skewed copy at 45 degrees (the trailing edge 2 units
/// out at the base and 1 at the tip); the rig turns the whole strip rigidly to a
/// smaller 30 degrees (pi/6 rad), which keeps the 5.3 ft chord and the look of
/// a rudder, not a split fin.
pub(crate) const RUDDER_TRAVEL: f64 = std::f64::consts::FRAC_PI_6;
/// Rudder-strip hinge: the strip's leading edge, from the base (0,-24,7) up and
/// aft to (0,-25,14). A positive angle swings the trailing edge toward +X (right).
const RUDDER_HINGE: [f32; 3] = [0., -24., 7.];
const RUDDER_AXIS: [f32; 3] = [0., -1., 7.];
/// Flap-strip hinge: the strip's leading edge at mid thickness (z -0.5), from
/// the root (11,-2) to the tip (32,-6). The right strip's axis points outboard
/// and the left's inboard, so a positive angle lowers the trailing edge on both.
const STRIP_PIVOT_R: [f32; 3] = [11., -2., -0.5];
const STRIP_AXIS_R: [f32; 3] = [21., -4., 0.];
const STRIP_PIVOT_L: [f32; 3] = [-11., -2., -0.5];
const STRIP_AXIS_L: [f32; 3] = [21., 4., 0.];

/// Gear hinges (fitted, agent decision 2026-11-15; the shape has no gear hinge).
/// The nose leg turns aft about a lateral line through the leg's aft edge
/// (y 22, z 1: one unit inside the belly skin at z 0) and steers about a vertical
/// line through its middle first. Each main leg turns **outward** as one rigid
/// piece about a line along Y at its long plate (x +-1, z 1), a quarter turn, so
/// the stowed plates lie flat in the glove and nacelle region between the
/// fuselage and the inboard engine, where `TU160.INF` says the real legs go (the
/// probe measures 0 ft of the stowed legs outside the neutral skin).
const NOSE_HINGE: [f32; 3] = [0., 22., 1.];
const NOSE_STEER: [f32; 3] = [0., 21., 0.];
const MAIN_HINGE_R: [f32; 3] = [1., 0., 1.];
const MAIN_HINGE_L: [f32; 3] = [-1., 0., 1.];
/// Gear timing (fitted): the share of the gear travel (0 up, 1 down) each leg
/// uses. The nose leg is down by 0.8 and starts to move at once; the mains start
/// at 0.2 and arrive at 1.
const NOSE_WINDOW: [f64; 2] = [0., 0.8];
const MAIN_WINDOW: [f64; 2] = [0.2, 1.];

/// The wing's turn from its spread pose, radians: the speed law of
/// [`swing::TU160`], held spread by the flaps (20, 35 and 65 degrees).
pub(crate) fn sweep_turn(state: &State) -> f64 {
    swing::TU160.turn(state.speed, state.flaps, state.gear)
}
/// The wing's leading-edge sweep angle for a turn, degrees (20 spread to 65).
#[cfg(test)]
pub(crate) fn sweep_degrees(turn: f64) -> f64 {
    swing::TU160.sweep_degrees(turn)
}

/// One tailplane angle in radians, positive lowering the trailing edge: pitch
/// raises both trailing edges for a positive elevator, a positive aileron raises
/// the right and lowers the left, and the sum is clamped at the limit (rule 12).
pub(crate) fn tail_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    (-TAIL_PITCH * state.elevator.clamp(-1., 1.) - side * TAIL_ROLL * state.aileron.clamp(-1., 1.))
        .clamp(-TAIL_LIMIT, TAIL_LIMIT)
}

/// The wingtip vapour attachment of one side (0 left, 1 right) for a wing turn,
/// feet right, up and forward of the aircraft. The shape's streamer record
/// (the Tu-26 copy) measures out at the drawn wing tip (93.3 ft out, 2.7 ft down,
/// 16 ft aft: the tip vertex (34,-7,-1) is 90.7 ft out), so it is taken as the
/// spread tip and carried by the wing's own rigid turn about its pivot, as the
/// Tornado's is: the vapour leaves the drawn tip at every sweep.
pub(crate) fn streamer_point(
    def: &tore_formats::shape::StreamerDef,
    side: usize,
    turn: f64,
) -> Option<[f64; 3]> {
    let record = def.attachment(side, 0).ok()?.map(|v| v / 3.);
    // The record's order is right, up, forward in feet.
    Some(swing::TU160.swept(side == 1, record, turn))
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("TU160.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("TU160.SH duplicate reviewed face {address:x}").into());
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
        return Err(format!("unreviewed TU160.SH branch {word:x} = {value}").into());
    }
    Ok(pose)
}

fn has(face: &Face, p: [f32; 3]) -> bool {
    face.positions.contains(&p)
}

/// A cap face: `template`'s look on the three `points` (the strip's end
/// triangle), `back` reversing the winding. The stored normal is the polygon's,
/// on the side the front winding faces.
fn cap(template: &Face, address: usize, points: [[f32; 3]; 3], back: bool) -> Face {
    let mut positions: Vec<[f32; 3]> = points.to_vec();
    if back {
        positions.reverse();
    }
    let mut face = Face {
        address,
        colors: template.colors.iter().copied().take(3).collect(),
        uv: template.uv.iter().copied().take(3).collect(),
        positions,
        texture: template.texture.clone(),
        subtype: template.subtype,
        normal: template.normal,
        fog: template.fog,
    };
    if face.colors.len() < 3 {
        face.colors = vec![template.colors.first().copied().unwrap_or(31); 3];
    }
    if let (Some(old), Some(n)) = (
        template.normal,
        crate::aircraft_animation::polygon_normal(&face.positions),
    ) {
        // Stored normals are right, up, forward; the polygon's is right,
        // forward, up. Each winding takes its own polygon's direction at the
        // template's length.
        let length = old.iter().map(|v| v * v).sum::<f32>().sqrt().max(1e-6);
        face.normal = Some([n[0] * length, n[2] * length, n[1] * length]);
    }
    face
}

pub struct Rig;
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 28716
            || shape.faces.len() != 316
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed TU160.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // The wing panels: 16 whole faces a side, each present once, spanning
        // x 8 to 34 and sharing no vertex with any face outside the panel (the
        // review's measurement: the panel is one rigid body about its pivot).
        for (ids, side) in [(&WING_R, 1f32), (&WING_L, -1f32)] {
            let mut inside = BTreeSet::new();
            for &address in ids {
                let face = one(&shape, address)?;
                if !face
                    .positions
                    .iter()
                    .all(|p| p[0] * side >= 8. && p[0] * side <= 34. && (-7. ..=4.).contains(&p[1]))
                {
                    return Err(format!("unreviewed TU160.SH wing face {address:x}").into());
                }
                inside.extend(face.positions.iter().map(|p| p.map(f32::to_bits)));
            }
            for face in shape.faces.iter().filter(|f| !ids.contains(&f.address)) {
                // The left panel's own mirror image is another panel.
                if !(WING_R.contains(&face.address) || WING_L.contains(&face.address))
                    && face
                        .positions
                        .iter()
                        .any(|p| inside.contains(&p.map(f32::to_bits)))
                {
                    return Err(format!(
                        "TU160.SH wing panel shares a vertex with face {:x}",
                        face.address
                    )
                    .into());
                }
            }
        }
        // The flap strips: the whole outer trailing edge, four vertices each,
        // the leading edge on the hinge line (z 0 top, z -1 bottom).
        for (ids, side) in [(&STRIP_R, 1f32), (&STRIP_L, -1f32)] {
            for &address in ids {
                let face = one(&shape, address)?;
                if face.positions.len() != 4
                    || !has(face, [side * 11., -5., -1.])
                    || !has(face, [side * 32., -7., -1.])
                {
                    return Err(format!("unreviewed TU160.SH flap strip {address:x}").into());
                }
            }
        }
        // Tailplanes: one quad a side at z 7, whole in both windings.
        for (ids, side) in [(&TAIL_R, 1f32), (&TAIL_L, -1f32)] {
            for &address in ids {
                let face = one(&shape, address)?;
                if face.positions.len() != 4
                    || !face.positions.iter().all(|p| p[2] == 7.)
                    || !has(face, [0., -26., 7.])
                    || !has(face, [0., -18., 7.])
                    || !has(face, [side * 8., -28., 7.])
                {
                    return Err(format!("unreviewed TU160.SH tailplane {address:x}").into());
                }
            }
        }
        // Rudder strip: the aft two units of the upper fin in the plane x 0.
        for &address in &RUDDER {
            let face = one(&shape, address)?;
            if face.positions.len() != 4
                || !face.positions.iter().all(|p| p[0] == 0.)
                || !has(face, [0., -24., 7.])
                || !has(face, [0., -25., 14.])
                || !has(face, [0., -26., 7.])
                || !has(face, [0., -26., 14.])
            {
                return Err(format!("unreviewed TU160.SH rudder strip {address:x}").into());
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // Flap words: 1 deletes the strip and adds nothing; -1 swaps it for the
        // drooped copy. The rudder word swaps the strip for a skewed copy.
        let drooped_r = branch(bytes, &original, 0x8014, -1, &DROOP_R, &STRIP_R)?;
        let drooped_l = branch(bytes, &original, 0x800e, -1, &DROOP_L, &STRIP_L)?;
        branch(bytes, &original, 0x8014, 1, &[], &STRIP_R)?;
        branch(bytes, &original, 0x800e, 1, &[], &STRIP_L)?;
        branch(bytes, &original, 0x801a, 1, &RUDDER_WORD_R, &RUDDER)?;
        branch(bytes, &original, 0x801a, -1, &RUDDER_WORD_L, &RUDDER)?;
        // Gear, keel and flame words add exactly the reviewed plates and remove
        // nothing. Each plate is checked against the reviewed source box.
        let gear = branch(bytes, &original, 0x8002, 1, &GEAR, &[])?.scaled(axis);
        let keel = branch(bytes, &original, 0x7ffc, 1, &KEEL, &[])?.scaled(axis);
        let flame = branch(bytes, &original, 0x7ff0, 1, &FLAME, &[])?.scaled(axis);
        let inside = |face: &Face, lo: [f32; 3], hi: [f32; 3]| {
            face.positions.len() == 4
                && face
                    .positions
                    .iter()
                    .all(|p| (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k]))
        };
        for (ids, lo, hi) in [
            (&NOSE[..], [-1., 20., -5.], [1., 22., 0.]),
            (&MAIN_R[..], [0., -4., -5.], [2., 2., 0.]),
            (&MAIN_L[..], [-2., -4., -5.], [0., 2., 0.]),
            (&KEEL[..], [-1., 6., -2.], [1., 16., 0.]),
        ] {
            for &address in ids {
                let pose = if KEEL.contains(&address) {
                    &keel
                } else {
                    &gear
                };
                if !inside(
                    one(pose, address)?,
                    scale_point(ID, lo),
                    scale_point(ID, hi),
                ) {
                    return Err(format!("unreviewed TU160.SH plate {address:x}").into());
                }
            }
        }
        // Four prisms, each plate 11 units long from the nozzle plane to y -22.
        let (mut root, mut tip) = (f32::NEG_INFINITY, f32::INFINITY);
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4 {
                return Err(format!("unreviewed TU160.SH plume plate {address:x}").into());
            }
            for p in &face.positions {
                root = root.max(p[1]);
                tip = tip.min(p[1]);
            }
        }
        if root != scale_point(ID, [0., NOZZLE_PLANE, 0.])[1]
            || tip != scale_point(ID, [0., FLAME_TIP, 0.])[1]
        {
            return Err("unreviewed TU160.SH plume extent".into());
        }
        // The flap end caps: the strip's own end triangles (the top leading
        // vertex, the trailing vertex and the bottom leading vertex of each
        // end), looks borrowed from the source's drooped root cap.
        let mut caps = Vec::new();
        let mut addresses = CAPS.into_iter();
        for (pose, droop, side) in [(&drooped_r, DROOP_R, 1f32), (&drooped_l, DROOP_L, -1f32)] {
            let template = one(pose, droop[2])?;
            let ends = [
                [
                    [side * 11., -2., 0.],
                    [side * 11., -5., -1.],
                    [side * 11., -2., -1.],
                ],
                [
                    [side * 32., -6., 0.],
                    [side * 32., -7., -1.],
                    [side * 32., -6., -1.],
                ],
            ];
            for points in ends {
                let points = points.map(|p| scale_point(ID, p));
                for back in [false, true] {
                    let address = addresses.next().ok_or("eight cap addresses")?;
                    caps.push(cap(template, address, points, back));
                }
            }
        }
        let mut extra: Vec<Face> = Vec::new();
        extra.extend(gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)));
        extra.extend(keel.faces.into_iter().filter(|f| KEEL.contains(&f.address)));
        extra.extend(
            flame
                .faces
                .into_iter()
                .filter(|f| FLAME.contains(&f.address)),
        );
        extra.extend(caps);
        shape.faces.extend(extra);
        Ok((Self, shape))
    }

    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        let right = WING_R.contains(&a);
        let cap = CAPS.iter().position(|c| *c == a);
        if right || WING_L.contains(&a) || cap.is_some() {
            // The flap strips droop about their leading edge first, in the
            // wing's own frame, then the whole panel (strips and caps included)
            // turns about the wing pivot.
            let flaps = state.flaps.clamp(0., 1.);
            let side_right = right || cap.is_some_and(|i| i < 4);
            if cap.is_some() && flaps <= 0. {
                return None;
            }
            if flaps > 0. && (STRIP_R.contains(&a) || STRIP_L.contains(&a) || cap.is_some()) {
                let (p, ax, angle) = if side_right {
                    (STRIP_PIVOT_R, STRIP_AXIS_R, FLAP * flaps)
                } else {
                    (STRIP_PIVOT_L, STRIP_AXIS_L, FLAP * flaps)
                };
                turn(&mut result, scale_point(ID, p), scale_point(ID, ax), angle);
            }
            let swept = sweep_turn(state);
            if swept != 0. {
                turn(
                    &mut result,
                    pivot(side_right),
                    scale_point(ID, [0., 0., 1.]),
                    if side_right { -swept } else { swept },
                );
            }
        } else if TAIL_R.contains(&a) || TAIL_L.contains(&a) {
            // The whole quad turns about the lateral spindle through the root
            // chord midpoint: no vertex is held while another turns.
            let angle = tail_angle(TAIL_R.contains(&a), state);
            if angle != 0. {
                turn(
                    &mut result,
                    scale_point(ID, TAIL_SPINDLE),
                    scale_point(ID, [1., 0., 0.]),
                    angle,
                );
            }
        } else if RUDDER.contains(&a) {
            // One rigid strip about the upper fin's trailing edge: the two
            // vertices on that edge stay, the rest swing out of the fin plane.
            let angle = RUDDER_TRAVEL * state.rudder.clamp(-1., 1.);
            if angle != 0. {
                turn(
                    &mut result,
                    scale_point(ID, RUDDER_HINGE),
                    scale_point(ID, RUDDER_AXIS),
                    angle,
                );
            }
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
        } else if KEEL.contains(&a) {
            let brake = state.brake.clamp(0., 1.) as f32;
            if brake <= 0. {
                return None;
            }
            // The plates drop out of the belly with the brake lever (fitted):
            // their top edge stays on the belly skin (z 0).
            let top = scale_point(ID, [0., 0., 0.])[2];
            for p in &mut result.positions {
                p[2] = top + (p[2] - top) * brake;
            }
        } else if FLAME.contains(&a) {
            let exhaust = state.exhaust.clamp(0., 1.) as f32;
            if exhaust <= 0. {
                return None;
            }
            // The plume grows from the nozzle plane; every plate scales from
            // the same root, so the shared edges stay together.
            let root = scale_point(ID, [0., NOZZLE_PLANE, 0.])[1];
            for p in &mut result.positions {
                p[1] = root + (p[1] - root) * exhaust;
            }
        }
        Some(result)
    }
}

/// How far a gear leg has come down, 0 (stowed) to 1, in its timing window.
fn leg(gear: f64, window: [f64; 2]) -> f64 {
    ((gear.clamp(0., 1.) - window[0]) / (window[1] - window[0])).clamp(0., 1.)
}

/// Whole-piece gear motion. Gear down (1) is the source pose; the caller hides
/// the gear at 0. The nose leg steers about its vertical line, then folds aft;
/// each main leg folds outward, both as one rigid piece.
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
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
        let angle = FRAC_PI_2 * (1. - leg(gear, NOSE_WINDOW));
        turn(
            f,
            scale_point(ID, NOSE_HINGE),
            scale_point(ID, [1., 0., 0.]),
            -angle,
        );
    } else if MAIN_R.contains(&a) || MAIN_L.contains(&a) {
        let angle = FRAC_PI_2 * (1. - leg(gear, MAIN_WINDOW));
        let (hinge, outward) = if MAIN_R.contains(&a) {
            (MAIN_HINGE_R, -1.)
        } else {
            (MAIN_HINGE_L, 1.)
        };
        turn(
            f,
            scale_point(ID, hinge),
            scale_point(ID, [0., 1., 0.]),
            outward * angle,
        );
    }
}

/// The afterburner plumes.
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
            subtype: 0xee,
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
    fn knots(s: &mut State, kt: f64) {
        s.speed = kt * swing::FPS_PER_KNOT;
    }
    /// One test for the whole rig, on synthetic faces named in shape units: the
    /// wing panel holds its detents (20 degrees to 250 kt, 35 from 320 to 520
    /// kt, 65 from 600 kt, held spread by the flaps) and turns whole about the
    /// fitted pivot with the strip drooping first; the tailplane takes pitch plus
    /// differential roll; the rudder strip swings its trailing edge toward the
    /// pedal; the keel plates and gear fold whole; the plume grows from the
    /// nozzle plane.
    #[test]
    fn the_tu160_rig_holds_its_detents_and_turns_whole_panels_about_their_hinges() {
        let rig = Rig;
        let mut s = state();
        // The three detents, and smooth in between.
        let degrees = |s: &State| sweep_degrees(sweep_turn(s));
        for (kt, want) in [
            (0., 20.),
            (250., 20.),
            (320., 35.),
            (450., 35.),
            (520., 35.),
            (600., 65.),
            (900., 65.),
        ] {
            knots(&mut s, kt);
            assert!(
                (degrees(&s) - want).abs() < 1e-9,
                "{kt} kt: {}",
                degrees(&s)
            );
        }
        knots(&mut s, 285.);
        let mid = degrees(&s);
        assert!(mid > 20.5 && mid < 34.5, "between detents: {mid}");
        // Flaps hold the wing spread whatever the speed.
        knots(&mut s, 700.);
        s.flaps = 1.;
        assert_eq!(sweep_turn(&s), 0.);
        s.flaps = 0.;

        // The right panel turns whole, tip aft, about the pivot; the left mirrors.
        let panel = [
            face(
                0x4a22,
                &[[32., -4., -1.], [32., -3., 0.], [9., 4., 0.], [8., 2., -1.]],
            ),
            face(
                0x49f9,
                &[[8., 2., 0.], [32., -4., 0.], [32., -6., 0.], [11., -2., 0.]],
            ),
        ];
        let panel_left: Vec<Face> = panel
            .iter()
            .map(|f| {
                let mut g = f.clone();
                g.address = 0x4f60;
                g.positions.iter_mut().for_each(|p| p[0] = -p[0]);
                g
            })
            .collect();
        knots(&mut s, 700.);
        let right: Vec<[f32; 3]> = panel
            .iter()
            .flat_map(|f| rig.animate(f, &s).unwrap().positions)
            .collect();
        let left: Vec<[f32; 3]> = panel_left
            .iter()
            .flat_map(|f| rig.animate(f, &s).unwrap().positions)
            .collect();
        let before: Vec<[f32; 3]> = panel.iter().flat_map(|f| f.positions.clone()).collect();
        assert!(spread(&before, &right) < 1e-3, "right panel not rigid");
        let tip_before = scale_point(ID, [32., -4., 0.]);
        let tip = rig.animate(&panel[1], &s).unwrap().positions[1];
        // Fully swept (45 degrees of turn): the tip is 28 units from the pivot,
        // 28 sin 45 aft of its place, and 4.8 + 27.2 cos 45 ... out.
        assert!(tip[1] < tip_before[1] - 15. && tip[0] < tip_before[0] - 5.);
        for (r, l) in right.iter().zip(&left) {
            assert!((r[0] + l[0]).abs() < 1e-4 && (r[1] - l[1]).abs() < 1e-4);
        }
        // The pivot itself stays: a point on the pivot line does not move.
        let at_pivot = face(0x49f9, &[[4.8, 0., 0.]]);
        let moved = rig.animate(&at_pivot, &s).unwrap().positions[0];
        assert!((moved[0] - at_pivot.positions[0][0]).abs() < 1e-4);
        assert!((moved[1] - at_pivot.positions[0][1]).abs() < 1e-4);

        // Flaps droop the trailing edge down and the hinge stays; end caps
        // draw only while drooped.
        knots(&mut s, 100.);
        let strip = face(
            0x4b93,
            &[
                [32., -7., -1.],
                [11., -5., -1.],
                [11., -2., 0.],
                [32., -6., 0.],
            ],
        );
        let cap_face = face(0x8110, &[[11., -2., 0.], [11., -5., -1.], [11., -2., -1.]]);
        assert!(rig.animate(&cap_face, &s).is_none());
        s.flaps = 1.;
        let down = rig.animate(&strip, &s).unwrap();
        assert!(
            down.positions[1][2] < strip.positions[1][2] - 1.,
            "trailing edge drops"
        );
        let lead = rig.animate(&face(0x4b93, &[[11., -2., 0.]]), &s).unwrap();
        assert!((lead.positions[0][2] - 0.).abs() < 0.3 && (lead.positions[0][1] + 2.).abs() < 0.3);
        assert!(spread(&strip.positions, &down.positions) < 1e-3);
        assert!(rig.animate(&cap_face, &s).is_some());
        s.flaps = 0.;

        // Tailplane: pitch lowers both trailing edges together, roll in opposite
        // directions, the limit holds.
        let edge_r = face(0x1daf, &[[8., -28., 7.]]);
        let edge_l = face(0x1dce, &[[-8., -28., 7.]]);
        let drop =
            |s: &State, f: &Face| rig.animate(f, s).unwrap().positions[0][2] - f.positions[0][2];
        s.elevator = 1.;
        assert!(drop(&s, &edge_r) > 0.01 || drop(&s, &edge_r) < -0.01);
        let (pr, pl) = (drop(&s, &edge_r), drop(&s, &edge_l));
        assert!(
            pr * pl > 0. && (pr - pl).abs() < 1e-4,
            "pull moves both the same way"
        );
        assert!(pr > 0., "a pull raises the trailing edge: {pr}");
        s.elevator = 0.;
        s.aileron = 1.;
        let (rr, rl) = (drop(&s, &edge_r), drop(&s, &edge_l));
        assert!(
            rr * rl < 0. && (rr + rl).abs() < 1e-4,
            "roll moves them opposite ways"
        );
        s.aileron = 1.;
        s.elevator = -1.;
        let a = tail_angle(true, &s).abs().max(tail_angle(false, &s).abs());
        assert!(a <= TAIL_LIMIT + 1e-12);
        s.elevator = 0.;
        s.aileron = 0.;

        // Rudder strip: the trailing edge swings toward the pedal, the hinge edge
        // stays, the strip stays rigid.
        let strip = face(
            0x52e0,
            &[
                [0., -26., 7.],
                [0., -24., 7.],
                [0., -25., 14.],
                [0., -26., 14.],
            ],
        );
        s.rudder = 1.;
        let right_turn = rig.animate(&strip, &s).unwrap();
        s.rudder = -1.;
        let left_turn = rig.animate(&strip, &s).unwrap();
        assert!(right_turn.positions[0][0] > 0.5 && left_turn.positions[0][0] < -0.5);
        assert!(
            right_turn.positions[1] == strip.positions[1] || {
                let d = right_turn.positions[1];
                let o = strip.positions[1];
                (0..3).all(|k| (d[k] - o[k]).abs() < 1e-5)
            }
        );
        assert!(spread(&strip.positions, &right_turn.positions) < 1e-3);
        s.rudder = 0.;

        // Keel plates draw only with the lever and drop with it; the gear folds
        // whole and is hidden up.
        let keel = face(
            0x4739,
            &[[1., 6., 0.], [1., 6., -2.], [1., 16., -2.], [1., 16., 0.]],
        );
        assert!(rig.animate(&keel, &s).is_none());
        s.brake = 0.5;
        let half = rig.animate(&keel, &s).unwrap();
        assert!((half.positions[1][2] - scale_point(ID, [0., 0., -1.])[2]).abs() < 1e-5);
        assert_eq!(half.positions[0][2], keel.positions[0][2]);
        s.brake = 0.;
        let nose = [
            face(
                0x5515,
                &[[0., 20., -5.], [0., 22., -5.], [0., 22., 0.], [0., 20., 0.]],
            ),
            face(
                0x5553,
                &[
                    [-1., 22., -5.],
                    [1., 22., -5.],
                    [1., 22., 0.],
                    [-1., 22., 0.],
                ],
            ),
        ];
        let main_r = [
            face(
                0x5373,
                &[[1., -4., -5.], [1., 2., -5.], [1., 2., 0.], [1., -4., 0.]],
            ),
            face(
                0x53c1,
                &[[0., -1., 0.], [2., -1., 0.], [2., -1., -5.], [0., -1., -5.]],
            ),
        ];
        let main_l: Vec<Face> = main_r
            .iter()
            .map(|f| {
                let mut g = f.clone();
                g.address = if f.address == 0x5373 { 0x5444 } else { 0x5492 };
                g.positions.iter_mut().for_each(|p| p[0] = -p[0]);
                g
            })
            .collect();
        assert!(rig.animate(&nose[0], &s).is_none());
        let before = |faces: &[Face]| -> Vec<[f32; 3]> {
            faces.iter().flat_map(|f| f.positions.clone()).collect()
        };
        for gear in [1., 0.75, 0.4, 0.05] {
            s.gear = gear;
            for set in [&nose[..], &main_r[..], &main_l[..]] {
                let turned: Vec<[f32; 3]> = set
                    .iter()
                    .flat_map(|f| rig.animate(f, &s).unwrap().positions)
                    .collect();
                assert!(
                    spread(&before(set), &turned) < 1e-3,
                    "gear {gear}: leg not rigid"
                );
            }
        }
        // Down is the source pose; the nose foot goes aft and the mains' feet go
        // outboard into the glove (the left mirrors the right) as the gear comes up.
        s.gear = 1.;
        let down = rig.animate(&nose[0], &s).unwrap();
        assert_eq!(down.positions, nose[0].positions);
        s.gear = 0.05;
        let foot = rig.animate(&nose[0], &s).unwrap().positions[0];
        assert!(foot[1] < scale_point(ID, [0., 20., 0.])[1] - 1.);
        let r = rig.animate(&main_r[0], &s).unwrap().positions[0];
        let l = rig.animate(&main_l[0], &s).unwrap().positions[0];
        assert!(r[0] > scale_point(ID, [1., 0., 0.])[0] + 1.);
        assert!(l[0] < scale_point(ID, [-1., 0., 0.])[0] - 1.);
        assert!((r[0] + l[0]).abs() < 1e-4);
        s.gear = 0.;

        // The plume grows from the nozzle plane (y -11).
        let plate = face(
            0x5658,
            &[
                [-5., -22., -3.],
                [-5., -22., -1.],
                [-5., -11., -1.],
                [-5., -11., -3.],
            ],
        );
        assert!(rig.animate(&plate, &s).is_none());
        s.exhaust = 0.5;
        let half = rig.animate(&plate, &s).unwrap();
        assert!((half.positions[0][1] - scale_point(ID, [0., -16.5, 0.])[1]).abs() < 1e-4);
        assert_eq!(half.positions[2][1], plate.positions[2][1]);
        the_vapour_point_follows_the_sweep();
    }

    /// The wingtip vapour point is the streamer record at the spread tip (the
    /// retail record, 93.3 ft out, 2.7 ft down, 16 ft aft) and rides the wing's
    /// turn about its pivot: at 65 degrees (45 degrees of turn) it is 58.4 ft out
    /// and 68.2 ft aft, mirrored on the left.
    fn the_vapour_point_follows_the_sweep() {
        let def = tore_formats::shape::StreamerDef {
            pivot: [20480, 0, 10240],
            hinge_scale: 0,
            points: [[51200, -2048, -22528], [51200, -2048, -22528]],
        };
        let spread = |side| streamer_point(&def, side, 0.).unwrap();
        let (right, left) = (spread(1), spread(0));
        assert!((right[0] - 93.333).abs() < 0.01 && (left[0] + 93.333).abs() < 0.01);
        assert!((right[1] + 2.667).abs() < 0.01 && (right[2] + 16.).abs() < 0.01);
        let turn = 45f64.to_radians();
        let (r, l) = (
            streamer_point(&def, 1, turn).unwrap(),
            streamer_point(&def, 0, turn).unwrap(),
        );
        assert!(
            (r[0] - 58.4).abs() < 0.1 && (r[2] + 68.2).abs() < 0.1,
            "{r:?}"
        );
        assert!((r[0] + l[0]).abs() < 1e-9 && (r[2] - l[2]).abs() < 1e-9);
        assert_eq!(r[1], right[1]);
    }
}
