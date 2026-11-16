//! F-111F (`F111.SH`) own-source rig (F111-3, 2026-11-15): the swing wing
//! (two whole panels turned about the fitted pivot, the trailing flap strips
//! riding with them), the two whole-panel tailerons, the rudder, the two rigid
//! gear assemblies, the four dorsal speed brake plates and the two afterburner
//! plumes. See docs/formats/aircraft-variety.md "F-111F shape and rig
//! (F111-2/3, 2026-11-15)" and docs/spec/variety-animation.md "F-111F rig fit".
//!
//! `F111.SH` ships six state words: 0x7240 the afterburner plates, 0x7246 the
//! four dorsal panels, 0x724c the gear (22 faces), 0x7258 and 0x725e the left
//! and right flap strips (three states each: the neutral strip, a drooped
//! strip with value -1, no strip for anything else) and 0x7264 the rudder
//! (three states: neutral, skewed right with 1, left with -1, none for
//! anything else). The shape has no sweep word: the wing is one fixed spread
//! pose at the real 16 degrees, so the sweep is a **fitted rig** that turns
//! every face of each wing panel as one rigid body. The rig never sets the
//! flap or rudder words; it keeps the strips and the rudder drawn and turns
//! them, so no surface vanishes in flight. There is no slat, spoiler, belly
//! brake, gear door, canopy or hook word, and none is invented (the real
//! belly speed brake that doubles as a gear door is not in the source;
//! coordinator decision, BOMBERS-DECISIONS-B).
//!
//! The shape arrives scaled per axis (F111-2: x 0.859, y 1.0, z 0.827). Every
//! hinge named here is a shape-unit point put through `scale_point`, so a
//! panel turned about it stays rigid in feet.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};
use tore_sim::combat::bays::{F111_BAY_MOUNT_FORWARD, F111_CRADLE_POSTS, Opening};
use tore_sim::combat::swing::F111F;

const ID: AircraftId = AircraftId::F111;
const WORDS: [usize; 6] = [0x7240, 0x7246, 0x724c, 0x7258, 0x725e, 0x7264];

/// The right wing panel: 13 whole faces (the flap strip, the last two, among
/// them), x 13 to 55. Face `2e12` (a small fixed triangle at x 14 to 15 on the
/// fuselage side) is not part of it.
pub(crate) const WING_R: [usize; 13] = [
    0x3fbc, 0x3fdd, 0x3ffe, 0x4021, 0x404a, 0x4073, 0x4096, 0x40bf, 0x4109, 0x4130, 0x4153, 0x421f,
    0x4246,
];
/// The left wing panel: 14 whole faces, x -55 to -12 (the left root is one
/// unit inboard of the right one's mirror; the source's own asymmetry).
pub(crate) const WING_L: [usize; 14] = [
    0x44e5, 0x450c, 0x4533, 0x455a, 0x457b, 0x459c, 0x45bd, 0x45e0, 0x4607, 0x462a, 0x4651, 0x4674,
    0x4740, 0x4767,
];
/// The trailing flap strips the flap words replace (0x725e right, 0x7258
/// left): two layers each, part of the wing panel, hinged on its trailing edge
/// line at the strip's front edge.
pub(crate) const STRIP_R: [usize; 2] = [0x421f, 0x4246];
pub(crate) const STRIP_L: [usize; 2] = [0x4740, 0x4767];
/// The drooped strips the words add with value -1 (never drawn).
const STRIP_WORD_R: [usize; 3] = [0x4298, 0x42bf, 0x42e6];
const STRIP_WORD_L: [usize; 3] = [0x47b3, 0x47da, 0x4801];
/// The tailerons: six faces each, flat plates at z 2 (outer quad, root quad and
/// root triangle, two windings), no state word.
pub(crate) const TAIL_R: [usize; 6] = [0x2d72, 0x2e72, 0x2d95, 0x2e95, 0x2db8, 0x2eb8];
pub(crate) const TAIL_L: [usize; 6] = [0x2071, 0x20d9, 0x1f08, 0x1f2b, 0x200f, 0x202d];
/// The rudder panel (both windings) the word 0x7264 swaps for a skewed copy.
pub(crate) const RUDDER: [usize; 2] = [0x491c, 0x4943];
const RUDDER_WORD: [usize; 2] = [0x4842, 0x4869];
/// Word 0x7246 adds four dorsal plates, two windings a side, hinged on their
/// front edge (y 0, z 4) and raised aft to z 9.
pub(crate) const DORSAL_R: [usize; 2] = [0x4eb9, 0x4ed0];
pub(crate) const DORSAL_L: [usize; 2] = [0x4e8b, 0x4ea2];
pub(crate) const DORSAL: [usize; 4] = [0x4e8b, 0x4ea2, 0x4eb9, 0x4ed0];
/// Word 0x724c adds 22 faces: the main assembly (16, both legs, the cross
/// plate and the six wheel plates, one rigid piece) and the nose leg (6).
pub(crate) const GEAR_MAIN: [usize; 16] = [
    0x4bcd, 0x4bec, 0x4c0b, 0x4c2a, 0x4c49, 0x4c68, 0x4c87, 0x4ca2, 0x4cbd, 0x4cd8, 0x4aaa, 0x4ad1,
    0x4ae8, 0x4b0f, 0x4b26, 0x4b4d,
];
pub(crate) const GEAR_NOSE: [usize; 6] = [0x4997, 0x49ae, 0x4d38, 0x4d57, 0x4d76, 0x4d95];
pub(crate) const GEAR: [usize; 22] = [
    0x4bcd, 0x4bec, 0x4c0b, 0x4c2a, 0x4c49, 0x4c68, 0x4c87, 0x4ca2, 0x4cbd, 0x4cd8, 0x4aaa, 0x4ad1,
    0x4ae8, 0x4b0f, 0x4b26, 0x4b4d, 0x4997, 0x49ae, 0x4d38, 0x4d57, 0x4d76, 0x4d95,
];
/// Word 0x7240 adds eight plumes plates: per nozzle a crossed pair (a plate in
/// the vertical plane and one in the horizontal plane through the nozzle
/// centre, both windings), 25 units long from the exit plane y -36 to -61.
pub(crate) const FLAME: [usize; 8] = [
    0x4f4e, 0x4f6d, 0x4f8c, 0x4fab, 0x4fca, 0x4fe9, 0x5008, 0x5027,
];
/// The nozzle exit octagons (subtype 0x64, y -36): the left one in both
/// windings, the right one in one.
pub(crate) const NOZZLE: [usize; 3] = [0x1c77, 0x1fe8, 0x23ad];
const NOZZLE_PLANE: f32 = -36.;
const FLAME_TIP: f32 = -61.;

/// The glove plates (F111-3, 2026-11-15; the Tornado's TOR-5b precedent): four
/// new faces, a plate a side in both windings, addresses past the end of the
/// module's code (25,206 bytes) so no source word can name them. The swung
/// panel's root edge turns about the pivot (13, 2.5), so at full sweep its
/// leading corner stands 4.7 ft off the fuselage and 5.4 ft of free root edge
/// shows beside the glove with open air ahead of it. The fixed plate fills that
/// wedge, from the pivot on the glove side to the glove's front corner, with
/// its outer edge the chord from that corner to where the swept leading corner
/// lands (20.26, 6.64) and (22.76, 7.02) shape units at 72.5 degrees. Spread it
/// lies wholly inside the wing's own thickness (z 2.6 between the wing's z 2 and
/// 3), so nothing shows. Fitted geometry, agent decision (the coordinator's
/// rule: add a glove plate if the seam shows). Right, then left.
pub(crate) const GLOVE_ADDRESSES: [usize; 4] = [0x7e00, 0x7e01, 0x7e02, 0x7e03];
const GLOVE_OUTLINE: [[f32; 2]; 5] = [
    [13., 2.5],
    [20.26, 6.64],
    [22.76, 7.02],
    [14., 12.],
    [13., 10.],
];
const GLOVE_Z: f32 = 2.6;
/// The wing's own top skin faces the plates take their material from (right,
/// left): colours, texture and flags, with texture coordinates fitted to the
/// skin's own mapping (F111-5, 2026-11-16; the fillet's flat look before).
const GLOVE_LOOK_R: usize = 0x40bf;
const GLOVE_LOOK_L: usize = 0x44e5;

/// The forward belly plate the bay is cut from: x +-3 units, y 13 to 34, z -6
/// at its aft edge rising to -5 at its fore edge (a quad, `3132`), 12 ft of
/// opening inside it (F111-5).
pub(crate) const BELLY: usize = 0x3132;
/// The bay lining's first address: past the module's code (0x6276) and under
/// the state words (0x7240), clear of the glove plates (0x7e00).
pub(crate) const LINING_BASE: usize = 0x7000;
/// The lining faces: a ceiling and four walls, a cradle rail and two posts (5
/// faces each).
const LINING: usize = 5 + 5 + 2 * 5;
pub(crate) fn lining(address: usize) -> bool {
    (LINING_BASE..LINING_BASE + LINING).contains(&address)
}
/// Feet to scaled shape units (the header's 2/3 ft a unit; the per-axis size
/// fit is already in the scaled shape).
const UNIT: f64 = 2. / 3.;

/// Pivot of the wing sweep, vertical axis, shape units: the centre of the
/// smallest circle round the panel's root corners (fitted, F111-0), the same
/// point [`F111F`] carries.
const PIVOT_R: [f32; 3] = [13., 2.5, 0.];
const PIVOT_L: [f32; 3] = [-13., 2.5, 0.];

/// Taileron law, fitted (agent decision 2026-11-15; the real limits were not
/// found): the whole panel turns about a lateral spindle through its root
/// chord midpoint, so its forward root corner (10, -17), nine units ahead of
/// the spindle, swings 2 * 9 * sin(limit / 2) units. Pitch lowers or raises
/// both trailing edges, roll differs, the sum is clamped.
pub(crate) const TAIL_PITCH: f64 = 0.20;
pub(crate) const TAIL_ROLL: f64 = 0.10;
pub(crate) const TAIL_LIMIT: f64 = 0.25;
/// Root chord midpoint of the right taileron (x 10, y -17 to -35, z 2); the
/// spindle is the lateral line through it and the left side mirrors it.
pub(crate) const TAIL_SPINDLE: [f32; 3] = [10., -26., 2.];
/// Flap droop and rudder travel, radians, fitted (agent decision 2026-11-15).
/// The droop is the source's own drooped strip, 27 degrees at the root and 18
/// at the tip, taken as 0.40 (23 degrees); the rudder is the source's skewed
/// pose (34 degrees at the base) taken a little under, 0.50 (29 degrees).
pub(crate) const FLAP: f64 = 0.40;
pub(crate) const RUDDER_TRAVEL: f64 = 0.50;
/// Strip hinge: the front edge of the trailing strip, a line from the panel
/// root (13, 1) to the outer corner (53, -2) at the strip's mid height (the
/// upper layer's front edge is at z 3, the lower layer's at 2). The pivot is
/// the root point and the axis points outboard.
const STRIP_PIVOT_R: [f32; 3] = [13., 1., 2.5];
const STRIP_AXIS_R: [f32; 3] = [40., -3., 0.];
const STRIP_PIVOT_L: [f32; 3] = [-12., 1., 2.5];
const STRIP_AXIS_L: [f32; 3] = [-41., -3., 0.];
/// Rudder hinge: the leading edge of the rudder panel, from (0, -31, 4) up and
/// aft to (0, -37, 18). A positive angle swings the trailing edge toward +X.
const RUDDER_HINGE: [f32; 3] = [0., -31., 4.];
const RUDDER_AXIS: [f32; 3] = [0., -6., 14.];
/// Dorsal plate hinge: the front edge, a line along X at y 0, z 4.
const DORSAL_HINGE: [f32; 3] = [0., 0., 4.];
/// The main assembly turns about a lateral axis (shape units) at its rear
/// edge (y -4, so no corner dips below the gear-down contact as the legs fold)
/// and in the fuselage (z -4, the leg tops), forward and up through
/// [`MAIN_FOLD`]; the nose leg about a lateral axis at the belly, a quarter
/// turn; both are fitted so the stowed pieces lie as far inside the skin as a
/// gear this large can (the main assembly is 10 units long and 7 high in a
/// hull 8 to 9 units high: the best fold of any hinge leaves 0.65 ft of it
/// out), and the nose leg steers first about the vertical axis through the
/// nose wheel plates (the cross at y 41).
const MAIN_HINGE: [f32; 3] = [0., -4., -4.];
const MAIN_FOLD: f64 = 150. * PI / 180.;
const NOSE_HINGE: [f32; 3] = [0., 36., -7.];
const NOSE_FOLD: f64 = PI / 2.;
const NOSE_STEER: [f32; 3] = [0., 41., 0.];

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

pub(crate) fn wing_right(address: usize) -> bool {
    WING_R.contains(&address)
}
pub(crate) fn wing_left(address: usize) -> bool {
    WING_L.contains(&address)
}
pub(crate) fn tail_right(address: usize) -> bool {
    TAIL_R.contains(&address)
}
pub(crate) fn tail_left(address: usize) -> bool {
    TAIL_L.contains(&address)
}
pub(crate) fn rudder(address: usize) -> bool {
    RUDDER.contains(&address)
}

/// How far the wing panel has turned from its drawn pose (the real 16
/// degrees), radians: the speed schedule of [`F111F`] (spread to 250 knots,
/// fully swept at 650), held spread by the flaps, and held to 26 degrees or
/// less while a store hangs on a fixed outer pylon (`state.wing_hold`, F111-5).
/// This does not invent a flight law; it only draws the wing. Update (F111-8,
/// 2026-11-16): the wing moves at the spec's sweep rate (3 degrees a second),
/// so a frame whose flight carries the wing's present turn
/// (`state.wing_turn`, set by the world for the aircraft it flies) draws that;
/// a state without one (a replay, another aircraft's pose) draws the
/// schedule's turn.
pub(crate) fn sweep_turn(state: &State) -> f64 {
    state.wing_turn.unwrap_or_else(|| {
        F111F.turn_carrying(state.speed, state.flaps, state.gear, state.wing_hold)
    })
}

/// One taileron angle in radians, positive lowering the trailing edge: pitch
/// raises both trailing edges for a positive elevator, a positive aileron raises
/// the right and lowers the left, and the sum is clamped to the taileron limit.
pub(crate) fn tail_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    (-TAIL_PITCH * state.elevator.clamp(-1., 1.) - side * TAIL_ROLL * state.aileron.clamp(-1., 1.))
        .clamp(-TAIL_LIMIT, TAIL_LIMIT)
}

/// Closed dorsal plate angle, radians: the turn about the front hinge that lays
/// the raised plate flat at the hinge height, worked out from the scaled source
/// plate (its free edge is 7 units aft and 5 units above the hinge). Fitted.
pub(crate) fn dorsal_closed() -> f64 {
    let open = sp([0., -7., 9.]);
    let hinge = sp(DORSAL_HINGE);
    let (dy, dz) = (f64::from(open[1] - hinge[1]), f64::from(open[2] - hinge[2]));
    // About +X a point behind the hinge goes down for a positive angle; the
    // closed plate has no height over the hinge.
    (-dz / dy).atan()
}

/// Whole-piece gear motion. The main assembly and the nose leg each turn as one
/// rigid body about a lateral axis and fold forward; the nose leg steers about
/// its vertical axis first. Gear down (1) is the source pose; hidden at gear 0
/// by the caller.
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let fold = 1. - gear.clamp(0., 1.);
    let a = f.address;
    if GEAR_NOSE.contains(&a) {
        if steering != 0. {
            turn(f, sp(NOSE_STEER), sp([0., 0., 1.]), -steering);
        }
        turn(f, sp(NOSE_HINGE), sp([1., 0., 0.]), NOSE_FOLD * fold);
    } else if GEAR_MAIN.contains(&a) {
        turn(f, sp(MAIN_HINGE), sp([1., 0., 0.]), MAIN_FOLD * fold);
    }
}

/// The wingtip vapor attachment of one side (0 left, 1 right) for a wing turn,
/// feet right, up and forward of the aircraft. The shape's streamer record is
/// the spread tip in third-of-a-foot CE units (82 units out, 55 on the drawn
/// panel, which the size fit scales on the record's own factor into the wing),
/// so the vapor leaves the drawn tip instead: the outer corner (55, 0, 2) of
/// the panel, scaled and carried by the same rigid turn as the wing faces
/// (an agent decision, as the Tornado's, TOR-12).
pub(crate) fn streamer_point(side: usize, turn: f64) -> [f64; 3] {
    let tip = sp([55., 0., 2.]);
    let feet = tore_sim::combat::swing::FEET_PER_UNIT;
    let right = side == 1;
    let spread = [
        f64::from(if right { tip[0] } else { -tip[0] }) * feet,
        f64::from(tip[2]) * feet,
        f64::from(tip[1]) * feet,
    ];
    F111F.swept(right, spread, turn)
}

/// `shape`'s faces with both wing panels turned `turn_by` radians from spread
/// (the rest of the airframe untouched, the flaps up): the model a swivelling
/// wing station meets and the stores preview measures against.
pub(crate) fn swept_shape(shape: &Shape, turn_by: f64) -> Shape {
    let faces = shape
        .faces
        .iter()
        .map(|face| {
            let mut face = face.clone();
            let a = face.address;
            if wing_right(a) {
                turn(&mut face, sp(PIVOT_R), [0., 0., 1.], -turn_by);
            } else if wing_left(a) {
                turn(&mut face, sp(PIVOT_L), [0., 0., 1.], turn_by);
            }
            face
        })
        .collect();
    Shape {
        lines: Vec::new(),
        faces,
        billboards: Vec::new(),
        state_words: BTreeSet::new(),
    }
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("F111.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("F111.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

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
        return Err(format!("unreviewed F111.SH branch {word:x}").into());
    }
    Ok(pose)
}

/// The exact extent of a group of faces equals a shape-unit box (the same
/// single `f32` product `Shape::scaled` formed).
fn extent(shape: &Shape, ids: &[usize], min: [f32; 3], max: [f32; 3]) -> AppResult<()> {
    let (mut lo, mut hi) = ([f32::INFINITY; 3], [f32::NEG_INFINITY; 3]);
    for &address in ids {
        for p in &one(shape, address)?.positions {
            for k in 0..3 {
                lo[k] = lo[k].min(p[k]);
                hi[k] = hi[k].max(p[k]);
            }
        }
    }
    if lo != sp(min) || hi != sp(max) {
        return Err(format!("unreviewed F111.SH piece {:x}", ids[0]).into());
    }
    Ok(())
}

/// The four glove plate faces, scaled, each taking colours, texture and flags
/// from the fillet's own top triangle on its side. The outline is counter
/// clockwise seen from above on the right (the top winding is clockwise, as the
/// source's), the left is its mirror.
fn glove_plates(shape: &Shape) -> AppResult<Vec<Face>> {
    let mut faces = Vec::new();
    let mut addresses = GLOVE_ADDRESSES.into_iter();
    for (look, side) in [(GLOVE_LOOK_R, 1f32), (GLOVE_LOOK_L, -1f32)] {
        let source = one(shape, look)?;
        if source.positions.len() != 4 || source.colors.is_empty() || source.uv.len() != 4 {
            return Err(format!("unreviewed F111.SH glove top face {look:x}").into());
        }
        let outline: Vec<[f32; 3]> = GLOVE_OUTLINE
            .iter()
            .map(|p| sp([side * p[0], p[1], GLOVE_Z]))
            .collect();
        // The skin's own texture mapping, fitted by least squares (an affine
        // map from the plan position to the texel) so the plate carries the
        // wing's pattern across the glove instead of one texel.
        let fit = uv_fit(source);
        let mut reversed = outline.clone();
        reversed.reverse();
        // Right: the outline is the underside and its reverse the top; the
        // mirror turns both over.
        let (top, under) = if side > 0. {
            (reversed, outline)
        } else {
            (outline, reversed)
        };
        for (positions, flip) in [(top, 1.), (under, -1.)] {
            let n = positions.len();
            faces.push(Face {
                address: addresses.next().ok_or("four glove addresses")?,
                colors: vec![source.colors[0]; n],
                uv: positions.iter().map(&fit).collect(),
                positions,
                texture: source.texture.clone(),
                subtype: source.subtype,
                normal: source.normal.map(|v| v.map(|c| c * flip)),
                fog: source.fog,
            });
        }
    }
    Ok(faces)
}

/// A least-squares affine map from a face's plan position (x, y) to its
/// texture coordinates, from the face's own vertices.
fn uv_fit(face: &Face) -> impl Fn(&[f32; 3]) -> [f32; 2] + use<> {
    // Normal equations for u (or v) = a x + b y + c over the vertices.
    let n = face.positions.len() as f64;
    let (mut sx, mut sy, mut sxx, mut sxy, mut syy) = (0., 0., 0., 0., 0.);
    for p in &face.positions {
        let (x, y) = (f64::from(p[0]), f64::from(p[1]));
        sx += x;
        sy += y;
        sxx += x * x;
        sxy += x * y;
        syy += y * y;
    }
    let m = [[sxx, sxy, sx], [sxy, syy, sy], [sx, sy, n]];
    let det3 = |m: [[f64; 3]; 3]| {
        m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
            - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
            + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0])
    };
    let det = det3(m);
    let solve = |channel: usize| -> [f64; 3] {
        let (mut bx, mut by, mut b) = (0., 0., 0.);
        for (p, uv) in face.positions.iter().zip(&face.uv) {
            let t = f64::from(uv[channel]);
            bx += f64::from(p[0]) * t;
            by += f64::from(p[1]) * t;
            b += t;
        }
        let rhs = [bx, by, b];
        std::array::from_fn(|k| {
            let mut c = m;
            for r in 0..3 {
                c[r][k] = rhs[r];
            }
            det3(c) / det
        })
    };
    let (u, v) = (solve(0), solve(1));
    move |p: &[f32; 3]| {
        let (x, y) = (f64::from(p[0]), f64::from(p[1]));
        [
            (u[0] * x + u[1] * y + u[2]) as f32,
            (v[0] * x + v[1] * y + v[2]) as f32,
        ]
    }
}

/// The bay: where the belly plate is cut and how the plate slopes there.
pub struct Rig {
    bay: Opening,
    /// The plate's height at the opening's aft edge and its rise a unit
    /// forward, scaled shape units.
    belly: (f32, f32),
}
impl Rig {
    /// A rig with the bay's opening and a level belly, for the tests that move
    /// synthetic faces.
    #[cfg(test)]
    fn synthetic() -> Self {
        Self {
            bay: tore_sim::combat::bays::opening(ID).expect("F-111F bay"),
            belly: (-5., 0.),
        }
    }
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 25206
            || shape.faces.len() != 254
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F111.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // The two wing panels, the strips, the tailerons and the rudder are
        // exactly the reviewed extents.
        extent(&shape, &WING_R, [13., -5., 2.], [55., 12., 3.])?;
        extent(&shape, &WING_L, [-55., -5., 2.], [-12., 12., 3.])?;
        extent(&shape, &STRIP_R, [13., -5., 2.], [53., 1., 3.])?;
        extent(&shape, &STRIP_L, [-53., -5., 2.], [-12., 1., 3.])?;
        extent(&shape, &TAIL_R, [8., -39., 2.], [24., -17., 2.])?;
        extent(&shape, &TAIL_L, [-24., -39., 2.], [-8., -17., 2.])?;
        extent(&shape, &RUDDER, [0., -40., 4.], [0., -31., 18.])?;
        for &address in &NOZZLE {
            let face = one(&shape, address)?;
            if face.positions.len() != 8
                || !face.positions.iter().all(|p| p[1] == NOZZLE_PLANE)
                || !face
                    .positions
                    .iter()
                    .all(|p| p[0].abs() > 1. && p[0].abs() < 8.)
            {
                return Err(format!("unreviewed F111.SH nozzle {address:x}").into());
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words replace exactly the strips (0x7258 left, 0x725e
        // right), the rudder word the panel; none is ever set by the rig.
        branch(bytes, &original, 0x7258, 1, &[], &STRIP_L)?;
        branch(bytes, &original, 0x725e, 1, &[], &STRIP_R)?;
        branch(bytes, &original, 0x7258, -1, &STRIP_WORD_L, &STRIP_L)?;
        branch(bytes, &original, 0x725e, -1, &STRIP_WORD_R, &STRIP_R)?;
        branch(bytes, &original, 0x7264, 1, &RUDDER_WORD, &RUDDER)?;
        // The gear, dorsal and burner words add exactly the reviewed faces and
        // remove nothing; every piece is checked against the reviewed box.
        let gear = branch(bytes, &original, 0x724c, 1, &GEAR, &[])?.scaled(axis);
        let dorsal = branch(bytes, &original, 0x7246, 1, &DORSAL, &[])?.scaled(axis);
        let flame = branch(bytes, &original, 0x7240, 1, &FLAME, &[])?.scaled(axis);
        extent(&gear, &GEAR_MAIN, [-8., -4., -10.], [6., 6., -3.])?;
        extent(&gear, &GEAR_NOSE, [-1., 38., -11.], [1., 46., -5.])?;
        extent(&dorsal, &DORSAL_R, [2., -7., 4.], [8., 0., 9.])?;
        extent(&dorsal, &DORSAL_L, [-8., -7., 4.], [-2., 0., 9.])?;
        extent(
            &flame,
            &FLAME,
            [-7., FLAME_TIP, -3.],
            [7., NOZZLE_PLANE, 3.],
        )?;
        let plates = glove_plates(&shape)?;
        // The bay's belly plate, its slope and the lining (F111-5).
        let bay = tore_sim::combat::bays::opening(ID).ok_or("F111 bay opening missing")?;
        let plate = one(&shape, BELLY)?.clone();
        let belly = belly_line(&plate, (bay.aft / UNIT) as f32)?;
        let wall = one(&gear, GEAR_MAIN[0])?.colors[0];
        let mut lining_faces = lining_faces(&plate, &bay, belly, wall);
        for (k, f) in lining_faces.iter_mut().enumerate() {
            f.address = LINING_BASE + k;
        }
        shape.faces.extend(plates);
        shape.faces.extend(lining_faces);
        shape.faces.extend(
            gear.faces
                .into_iter()
                .filter(|f| GEAR.contains(&f.address))
                .chain(
                    dorsal
                        .faces
                        .into_iter()
                        .filter(|f| DORSAL.contains(&f.address)),
                )
                .chain(
                    flame
                        .faces
                        .into_iter()
                        .filter(|f| FLAME.contains(&f.address)),
                ),
        );
        Ok((Self { bay, belly }, shape))
    }
    /// The faces a source face becomes: with the bay doors off their seat the
    /// belly plate is cut into the two door leaves, turned down about their
    /// hinges by `state.bay`; the lining draws only then.
    pub fn faces(&self, source: &Face, state: &State) -> Vec<Face> {
        let a = source.address;
        if a == BELLY && state.bay > 0. {
            let half = (self.bay.outer / UNIT) as f32;
            return crate::roster_animation::cut_doors_sloped(
                source,
                state.bay,
                [(-half, 0., -half, 1.), (0., half, half, -1.)],
                (
                    (self.bay.aft / UNIT) as f32,
                    (self.bay.fore / UNIT) as f32,
                    self.belly.0,
                    self.belly.1,
                ),
            );
        }
        if lining(a) && state.bay <= 0. {
            return Vec::new();
        }
        vec![source.clone()]
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        if lining(a) && state.bay <= 0. {
            return None;
        }
        let mut result = source.clone();
        let right = wing_right(a);
        if right || wing_left(a) {
            // The trailing strips droop about their front edge first, in the
            // wing's own frame, then the whole panel (strips included) turns
            // about the wing pivot.
            let flaps = state.flaps.clamp(0., 1.);
            if flaps > 0. && (STRIP_R.contains(&a) || STRIP_L.contains(&a)) {
                let (pivot, axis, angle) = if right {
                    (STRIP_PIVOT_R, STRIP_AXIS_R, FLAP * flaps)
                } else {
                    (STRIP_PIVOT_L, STRIP_AXIS_L, -FLAP * flaps)
                };
                turn(&mut result, sp(pivot), sp(axis), angle);
            }
            let swept = sweep_turn(state);
            if swept > 0. {
                turn(
                    &mut result,
                    sp(if right { PIVOT_R } else { PIVOT_L }),
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
                    sp([side * TAIL_SPINDLE[0], TAIL_SPINDLE[1], TAIL_SPINDLE[2]]),
                    [1., 0., 0.],
                    angle,
                );
            }
        } else if rudder(a) {
            // One rigid panel about its leading edge: the two vertices on that
            // edge stay, the rest swing out of the fin plane.
            let angle = RUDDER_TRAVEL * state.rudder.clamp(-1., 1.);
            if angle != 0. {
                turn(&mut result, sp(RUDDER_HINGE), sp(RUDDER_AXIS), angle);
            }
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
        } else if DORSAL.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            // Open (brake 1) is the source pose; closing turns the whole plate
            // about its front edge down onto the spine.
            turn(
                &mut result,
                sp(DORSAL_HINGE),
                [1., 0., 0.],
                dorsal_closed() * (1. - state.brake.clamp(0., 1.)),
            );
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
/// The belly plate's height at the opening's `aft` edge and its rise for each
/// unit forward, from the plate's own vertices: a quad whose height depends on
/// y alone, facing down.
fn belly_line(plate: &Face, aft: f32) -> AppResult<(f32, f32)> {
    let p = &plate.positions;
    let (lo, hi) = (
        p.iter().min_by(|a, b| a[1].total_cmp(&b[1])),
        p.iter().max_by(|a, b| a[1].total_cmp(&b[1])),
    );
    let (Some(lo), Some(hi)) = (lo, hi) else {
        return Err("unreviewed F111.SH belly plate".into());
    };
    let flat = p.iter().all(|q| {
        (q[2] - (lo[2] + (q[1] - lo[1]) * (hi[2] - lo[2]) / (hi[1] - lo[1]))).abs() < 1e-3
    });
    if p.len() != 4
        || hi[1] <= lo[1]
        || !flat
        || !plate.normal.is_some_and(|n| n[1] < -30000.)
        || aft < lo[1]
    {
        return Err("unreviewed F111.SH belly plate".into());
    }
    let slope = (hi[2] - lo[2]) / (hi[1] - lo[1]);
    Ok((lo[2] + (aft - lo[1]) * slope, slope))
}

/// The bay's lining: a ceiling, four walls whose lower edges follow the sloping
/// belly plate, a cradle rail under the ceiling and the cradle's two posts,
/// drawn in the grey of the shape's own gear plates, closed on every side so
/// no sky shows through the cut.
fn lining_faces(
    template: &Face,
    o: &Opening,
    (belly_aft, slope): (f32, f32),
    colour: u8,
) -> Vec<Face> {
    let middle = F111_BAY_MOUNT_FORWARD - tore_sim::combat::bays::B52_ORIGIN_AHEAD;
    let rails = [[0., middle - 4.2, middle + 4.2]];
    let posts = [
        [0., middle - F111_CRADLE_POSTS],
        [0., middle + F111_CRADLE_POSTS],
    ];
    let aft = (o.aft / UNIT) as f32;
    let floor = crate::bay_doors::units_at_pub(o.belly, UNIT);
    let mut faces = crate::bay_doors::centre_lining(template, o, UNIT, &rails, &posts);
    for f in &mut faces {
        f.colors = vec![colour; f.positions.len()];
        for p in &mut f.positions {
            if (p[2] - floor).abs() < 1e-4 {
                p[2] = belly_aft + (p[1] - aft) * slope;
            }
        }
    }
    faces
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
    /// A synthetic face whose vertices are named in shape units and scaled the
    /// way the loader scales the real shape.
    fn face(address: usize, points: &[[f32; 3]]) -> Face {
        let positions: Vec<[f32; 3]> = points.iter().map(|&p| sp(p)).collect();
        Face {
            address,
            colors: vec![31; positions.len()],
            uv: vec![[0.2, 0.8]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
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
        s.speed = kt * 1.68781;
    }

    /// F111-8 (2026-11-16): a frame whose flight carries the wing's present turn
    /// (the world sets it for the aircraft combat moves at the sweep rate) is
    /// drawn at that turn; without one the drawing takes the schedule's.
    #[test]
    fn the_wing_is_drawn_at_the_present_turn_when_the_frame_carries_one() {
        let mut s = state();
        knots(&mut s, 650.);
        let schedule = sweep_turn(&s);
        assert!((F111F.sweep_degrees(schedule) - 72.5).abs() < 1e-9);
        s.wing_turn = Some(F111F.max_turn() * 0.5);
        assert_eq!(sweep_turn(&s), F111F.max_turn() * 0.5);
        s.wing_turn = None;
        assert_eq!(sweep_turn(&s), schedule);
    }

    #[test]
    fn the_rig_sweeps_the_wing_about_the_f111_pivot_and_moves_every_surface_whole() {
        let rig = Rig::synthetic();
        let mut s = state();
        // The wing: the schedule is the spec's, the tip goes aft and in about
        // the pivot, the panel stays rigid, flaps hold it out.
        for (a, side) in [(0x4021, 1f32), (0x45e0, -1f32)] {
            let wing = face(
                a,
                &[
                    [side * 55., 0., 2.],
                    [side * 53., -5., 2.],
                    [side * 13., 10., 3.],
                ],
            );
            knots(&mut s, 200.);
            assert_eq!(rig.animate(&wing, &s).unwrap().positions, wing.positions);
            knots(&mut s, 650.);
            assert!((F111F.sweep_degrees(sweep_turn(&s)) - 72.5).abs() < 1e-9);
            let out = rig.animate(&wing, &s).unwrap();
            assert!(out.positions[0][1] < wing.positions[0][1]);
            assert!(out.positions[0][0].abs() < wing.positions[0][0].abs());
            assert!(spread(&wing.positions, &out.positions) < 1e-4);
            let pivot = sp(if side > 0. { PIVOT_R } else { PIVOT_L });
            let r = |p: [f32; 3]| (p[0] - pivot[0]).hypot(p[1] - pivot[1]);
            assert!((r(out.positions[0]) - r(wing.positions[0])).abs() < 1e-3);
            // Flaps hold the wing out.
            s.flaps = 1.;
            assert_eq!(sweep_turn(&s), 0.);
            s.flaps = 0.;
        }
        knots(&mut s, 200.);
        // Tailerons: pull raises both trailing edges, roll differs, whole panels.
        let tail = face(0x2d72, &[[10., -35., 2.], [10., -17., 2.], [21., -39., 2.]]);
        s.elevator = 1.;
        let up = rig.animate(&tail, &s).unwrap();
        assert!(up.positions[0][2] > tail.positions[0][2]);
        assert!(spread(&tail.positions, &up.positions) < 1e-4);
        s.elevator = 0.;
        s.aileron = 1.;
        let (r, l) = (tail_angle(true, &s), tail_angle(false, &s));
        assert!(r < 0. && l > 0. && r.abs() <= TAIL_LIMIT && l.abs() <= TAIL_LIMIT);
        s.aileron = 0.;
        // Rudder: the hinge edge stays, the trailing edge swings to +X.
        let rudder = face(0x491c, &[[0., -37., 18.], [0., -40., 18.], [0., -31., 4.]]);
        s.rudder = 1.;
        let turned = rig.animate(&rudder, &s).unwrap();
        assert!(
            (0..3).all(|k| (turned.positions[0][k] - rudder.positions[0][k]).abs() < 1e-4),
            "the hinge vertex stays"
        );
        assert!(turned.positions[1][0] > rudder.positions[1][0]);
        s.rudder = 0.;
        // Gear hidden at 0, whole at its source corners at 1; dorsal plates
        // hidden shut, flat at brake 0+; the plume hidden at 0.
        let leg = face(0x4c49, &[[6., -4., -10.], [6., 1., -10.], [6., 1., -4.]]);
        s.gear = 0.;
        assert!(rig.animate(&leg, &s).is_none());
        s.gear = 1.;
        assert_eq!(rig.animate(&leg, &s).unwrap().positions, leg.positions);
        s.gear = 0.5;
        assert!(rig.animate(&leg, &s).unwrap().positions[0][2] > leg.positions[0][2]);
        let plate = face(0x4eb9, &[[8., -7., 9.], [2., -7., 9.], [2., 0., 4.]]);
        s.brake = 0.;
        assert!(rig.animate(&plate, &s).is_none());
        s.brake = 1.;
        assert_eq!(rig.animate(&plate, &s).unwrap().positions, plate.positions);
        s.brake = 1e-6;
        let shut = rig.animate(&plate, &s).unwrap();
        assert!((shut.positions[0][2] - sp([0., 0., 4.])[2]).abs() < 1e-3);
        let burner = face(0x4f4e, &[[-4., -61., 3.], [-4., -36., 3.]]);
        s.exhaust = 0.;
        assert!(rig.animate(&burner, &s).is_none());
        s.exhaust = 0.5;
        assert!((rig.animate(&burner, &s).unwrap().positions[0][1] + 48.5).abs() < 1e-4);
        assert!(flame(0x4f4e) && !flame(0x1c77));
    }

    /// F111-5: shut, the belly plate is whole and nothing is lined; open, the
    /// plate is cut into two leaves hanging on their hinge lines (in the
    /// sloping plate) and the lining draws.
    #[test]
    fn the_bay_cuts_the_belly_plate_into_two_leaves_and_draws_its_lining_only_open() {
        let mut plate = face(
            BELLY,
            &[
                [-3., 13., -6.],
                [3., 13., -6.],
                [3., 34., -5.],
                [-3., 34., -5.],
            ],
        );
        plate.normal = Some([0., -32765., 0.]);
        let opening = tore_sim::combat::bays::opening(ID).unwrap();
        let belly = belly_line(&plate, (opening.aft / UNIT) as f32).unwrap();
        // The plate's height at the aft edge of the opening: -6 units rising a
        // unit over 21, times the z factor, at the opening's aft edge (14.505).
        let aft = (opening.aft / UNIT) as f32;
        let expect =
            sp([0., 0., -6.])[2] + (aft - 13.) * (sp([0., 0., 1.])[2] - sp([0., 0., 0.])[2]) / 21.;
        assert!((belly.0 - expect).abs() < 1e-4, "{belly:?}");
        let rig = Rig {
            bay: opening,
            belly,
        };
        let mut s = state();
        s.bay = 0.;
        assert_eq!(rig.faces(&plate, &s).len(), 1);
        let lining_face = Face {
            address: LINING_BASE,
            ..plate.clone()
        };
        assert!(rig.faces(&lining_face, &s).is_empty() && rig.animate(&lining_face, &s).is_none());
        s.bay = 1.;
        assert_eq!(rig.faces(&lining_face, &s).len(), 1);
        let open = rig.faces(&plate, &s);
        assert!(open.len() > 1);
        // Both leaves hang the opening's half width under their hinge: the lowest
        // vertex is 1.5 ft (2.25 units) under the plate at the aft edge.
        let lowest = open
            .iter()
            .flat_map(|f| f.positions.iter())
            .map(|p| p[2])
            .fold(f32::MAX, f32::min);
        let hang = (opening.outer / UNIT) as f32;
        assert!(
            (belly.0 - lowest - hang).abs() < 0.03,
            "{lowest} against {}",
            belly.0 - hang
        );
        // Half open, half the swing.
        s.bay = 0.5;
        let half = rig.faces(&plate, &s);
        let lowest_half = half
            .iter()
            .flat_map(|f| f.positions.iter())
            .map(|p| p[2])
            .fold(f32::MAX, f32::min);
        assert!((belly.0 - lowest_half - hang * std::f32::consts::FRAC_PI_4.sin()).abs() < 0.03);
    }
}
