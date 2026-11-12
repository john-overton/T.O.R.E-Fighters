//! Tornado IDS (`TOR.SH`) own-source rig: the swing wing, the two whole-panel
//! tailerons, the trailing flap strips and the rudder panel (TOR-3), and the
//! three rigid gear pieces, the two airbrake plates and the two afterburner
//! plumes (TOR-4). See docs/spec/variety-animation.md "Tornado control fit" and
//! "Tornado gear, airbrake and plume fit", and docs/formats/aircraft-control-surfaces.md
//! "Tornado control rig (TOR-3)" and "Tornado gear, airbrake and burner rig (TOR-4)".
//!
//! `TOR.SH` ships six state words: 0x7dc0 the burner plates, 0x7dc6 the
//! airbrake plates, 0x7dcc the gear, 0x7dd8 and 0x7dde the left and right flap
//! words (each deletes the trailing strip and adds nothing) and 0x7de4 the
//! rudder (swaps the lower fin block for a copy skewed 1 to 3 units in X). The
//! shape has no sweep word: the wing is one fixed spread pose, 13 whole faces a
//! side, so the sweep is a **fitted rig** that turns every wing face as one
//! rigid panel. The rig never sets the flap or rudder words; it keeps the
//! strips and the rudder block drawn and turns them, so no panel ever vanishes
//! in flight. There is no slat, spoiler, canopy, hook or reverser word and
//! none is invented.
//!
//! All the hinge points below are in shape units (x right, y forward, z up, 2/3
//! ft a unit at the header scale 2) and the shape arrives with the identity
//! per-axis scale, so a panel turned about one of them stays rigid.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use tore_formats::shape::{Face, Shape};

const WORDS: [usize; 6] = [0x7dc0, 0x7dc6, 0x7dcc, 0x7dd8, 0x7dde, 0x7de4];

/// The right wing panel: 13 whole faces, the trailing strip (the last two)
/// included. Flat plates at z 0 and 1, joined to the glove fillets at x 9 to 10.
pub(crate) const WING_R: [usize; 13] = [
    0x4cf1, 0x4d2b, 0x4d54, 0x4dea, 0x4eba, 0x4edd, 0x4f00, 0x4f23, 0x4f46, 0x4f7f, 0x4fa8, 0x4ff5,
    0x501c,
];
/// The left wing panel: 13 whole faces, the trailing strip (the last two)
/// included. The source puts the left glove joint one unit further out.
pub(crate) const WING_L: [usize; 13] = [
    0x52c3, 0x52e6, 0x5309, 0x5332, 0x535b, 0x537e, 0x53a1, 0x542b, 0x5454, 0x5483, 0x54a6, 0x5549,
    0x5570,
];
/// The trailing strips the flap words delete (0x7dde right, 0x7dd8 left): two
/// layers each, part of the wing panel and hinged on its leading edge.
pub(crate) const STRIP_R: [usize; 2] = [0x4ff5, 0x501c];
pub(crate) const STRIP_L: [usize; 2] = [0x5549, 0x5570];
/// The tailerons: one 5-vertex polygon per side at z -2, two windings each.
pub(crate) const TAIL_R: [usize; 2] = [0x446a, 0x45e9];
pub(crate) const TAIL_L: [usize; 2] = [0x4176, 0x42f8];
/// The lower fin block (both windings) the rudder word skews.
pub(crate) const RUDDER: [usize; 2] = [0x5723, 0x5750];
/// Faces the rudder word 0x7de4 swaps in for the block (never drawn).
const RUDDER_WORD: [usize; 2] = [0x5625, 0x5652];
/// The glove plates (TOR-5b, 2026-11-11, John's round-3 answer T4): one thin
/// fixed plate a side, two windings each (top first), that fills the wedge the
/// swinging wing opens between the fixed glove and its own root as it sweeps
/// (TOR-3 measured 2.7 to 5 ft at 67 degrees). They are new fitted geometry,
/// not source faces, so their addresses lie past the end of the module's code
/// (28,150 bytes) and no source word can name them. The plates are fixed to
/// the fuselage: [`Rig::animate`] never moves them, and they lie wholly above
/// the wing panel (which reaches z 1 at most), so no face of either passes
/// through the other at any sweep and no seam opens inside a face.
pub(crate) const GLOVE_PLATE_R: [usize; 2] = [0x7e00, 0x7e01];
pub(crate) const GLOVE_PLATE_L: [usize; 2] = [0x7e02, 0x7e03];
/// Plate height, shape units: 0.1 above the wing's top (z 1) and the glove's
/// outer edge, so the plate never shares a plane with the sweeping panel.
pub(crate) const GLOVE_Z: f32 = 1.1;
const _: () = assert!(GLOVE_Z > 1., "above the wing panel's top");
/// Plate outlines in shape units (x right, y forward), clockwise seen from
/// above, each starting at the one reflex corner (the fan of triangles the
/// drawing cuts every face into runs from the first vertex, which sees all the
/// others). Fitted: the front corner is the glove's outer front corner
/// ((10, 8), left (-10, 8)); the outline follows the wing root's edge at full
/// sweep (the root's ends (9,1) and (9,7) land at (10.80, 0.69) and (14.82,
/// 5.15) after 42 degrees about the pivot; left (-11.80, 0.69) and (-15.82,
/// 5.15)) kept 0.3 unit inside the wing so no gap opens between plate and
/// wing, passes down the rear root edge to y -1.2, and returns along the
/// glove's outer edge (x 10, left x -10) to the front corner.
pub(crate) const GLOVE_OUTLINE_R: [[f32; 2]; 5] = [
    [11.02, 0.49],
    [10.4, -1.2],
    [10., -1.],
    [10., 8.],
    [14.78, 4.85],
];
pub(crate) const GLOVE_OUTLINE_L: [[f32; 2]; 5] = [
    [-12.02, 0.49],
    [-15.78, 4.85],
    [-10., 8.],
    [-10., -1.],
    [-11.4, -1.2],
];
/// Source glove top faces the plates take their look from (the glove's outer
/// top triangle: right `4526`, left `4232`).
const GLOVE_LOOK_R: usize = 0x4526;
const GLOVE_LOOK_L: usize = 0x4232;

/// Gear word 0x7dcc adds 26 faces in three rigid pieces (TOR-4): the left main
/// leg (x -3 to -7), the right main leg (x 3 to 7), each ten faces (the wheel
/// sheet, the cross plate, the slanted strut and the top caps), and the nose
/// leg, six faces (a cross of two sheets and a slanted strut plate). Every
/// piece turns whole about a lateral axis, so no vertex stays fixed off an axis.
const GEAR_MAIN_L: [usize; 10] = [
    0x4705, 0x4724, 0x4743, 0x4762, 0x4781, 0x4798, 0x47af, 0x47c6, 0x47dd, 0x47fc,
];
const GEAR_MAIN_R: [usize; 10] = [
    0x4908, 0x4927, 0x4946, 0x4965, 0x4984, 0x499b, 0x49b2, 0x49c9, 0x49e0, 0x49ff,
];
const GEAR_NOSE: [usize; 6] = [0x4a97, 0x4ab6, 0x4ad5, 0x4af4, 0x4b13, 0x4b2a];
pub(crate) const GEAR: [usize; 26] = [
    0x4705, 0x4724, 0x4743, 0x4762, 0x4781, 0x4798, 0x47af, 0x47c6, 0x47dd, 0x47fc, 0x4908, 0x4927,
    0x4946, 0x4965, 0x4984, 0x499b, 0x49b2, 0x49c9, 0x49e0, 0x49ff, 0x4a97, 0x4ab6, 0x4ad5, 0x4af4,
    0x4b13, 0x4b2a,
];
/// Burner word 0x7dc0 adds eight plates: per nozzle one crossed pair (a plate
/// in the vertical plane and one in the horizontal plane through the nozzle
/// centre, each with both windings), 18 units long from y -25 to -43. Airbrake
/// word 0x7dc6 adds four faces: one plate a side, both windings, hinged on its
/// forward edge (y -7, z 2) and raised aft to (y -10, z 4).
const FLAME: [usize; 8] = [
    0x591e, 0x593d, 0x595c, 0x597b, 0x599a, 0x59b9, 0x59d8, 0x59f7,
];
const BRAKE: [usize; 4] = [0x57ec, 0x5803, 0x5889, 0x58a0];
const BRAKE_R: [usize; 2] = [0x57ec, 0x5803];
const BRAKE_L: [usize; 2] = [0x5889, 0x58a0];
/// The two nozzle exit octagons (subtype 0x64, y -24): left, right.
const NOZZLE: [usize; 2] = [0x3e90, 0x3eb7];
const NOZZLE_PLANE: f32 = -24.;
const FLAME_TIP: f32 = -43.;

/// Main leg bay pivots, a lateral axis (shape units) at the leg's rear edge
/// (y -1, the y of its rear foot corner, so no corner dips below the gear-down
/// contact as the leg folds) and 9 units above the foot: the leg turns forward
/// through a quarter turn and lies flat in the fuselage. Fitted so every
/// stowed vertex sits inside the hull (the probe checks it against the neutral
/// skin). The pivot's x is the leg's middle and does not matter.
const MAIN_HINGE_R: [f32; 3] = [5., -1., -2.];
const MAIN_HINGE_L: [f32; 3] = [-5., -1., -2.];
/// Nose leg bay pivot (lateral axis at the leg's rear edge, y 25) and its
/// vertical steering axis through the cross of the two sheets (y 27). Fitted
/// likewise.
const NOSE_HINGE: [f32; 3] = [0., 25., -3.5];
const NOSE_STEER: [f32; 3] = [0., 27., 0.];
/// Brake hinge: the forward edge of both plates, a line along X at y -7, z 2.
const BRAKE_HINGE: [f32; 3] = [0., -7., 2.];

#[cfg(test)]
pub(crate) use tore_sim::combat::swing::{FEET_PER_UNIT, SWEEP_MAX, pivot_feet};
/// Wing sweep pivots, vertical axis, shape units. The glove fillets are fixed
/// faces that share five vertices with each wing panel (right: (9,1,0), (9,1,1),
/// (10,-1,0), (10,-1,1), (10,-4,0)), and no single point sits on all of them.
/// The point that minimizes the worst separation of those shared vertices as the
/// wing turns is the centre of their smallest enclosing circle, the midpoint of
/// the (9,1) and (10,-4) corners: (9.5, -1.5), radius 2.55 units, so the seam
/// opens at most 2 * 2.55 * sin(turn / 2) (1.83 units, 1.2 ft at the full
/// 42 degree turn). The left glove sits one unit further out, as the source has
/// it, so its pivot is (-10.5, -1.5). Fitted.
///
/// The pivots, the sweep range and the speed schedule live in the simulation
/// (`tore_sim::combat::swing`, moved there in TOR-5 without changing a number),
/// so the stations on the swinging wing ride the same turn the drawing uses.
pub(crate) use tore_sim::combat::swing::{PIVOT_L, PIVOT_R, SWEEP_MIN};

/// Taileron law, fitted and smaller than the fleet's 0.40 clamp (John,
/// 2026-11-10, decision 20): the whole panel turns about a lateral spindle
/// through its root chord midpoint, so its forward corner (7,-8,-2), 14.5 units
/// ahead of the spindle, swings 2 * 14.5 * sin(limit / 2) units.
pub(crate) const TAIL_PITCH: f64 = 0.20;
pub(crate) const TAIL_ROLL: f64 = 0.08;
pub(crate) const TAIL_LIMIT: f64 = 0.25;
const _: () = assert!(
    TAIL_LIMIT < 0.40,
    "John: smaller than the fleet's taileron clamp"
);
/// Root chord midpoint of the right taileron (x 6, y -25 to -20, z -2); the
/// spindle is the lateral line through it, and the left side mirrors it.
pub(crate) const TAIL_SPINDLE: [f32; 3] = [6., -22.5, -2.];
/// Flap droop and rudder travel, radians, fitted to the fleet (the Rafale's
/// rudder 0.35 and flap droop 0.40).
pub(crate) const FLAP: f64 = 0.40;
pub(crate) const RUDDER_TRAVEL: f64 = 0.35;
/// Strip hinge: the line along the wing's leading edge of the strip, from the
/// glove root (10,-1) to the outer corner (30,-7). The two root vertices sit at
/// z 0 and 1, so the mid axis goes through z 0.5 at the root; the right
/// strip's pivot is the midpoint and the left one mirrors the pivot and axis
/// (the left strip runs from (-11,-1) to (-31,-7)).
const STRIP_PIVOT: [f32; 3] = [20., -4., 0.25];
const STRIP_AXIS: [f32; 3] = [20., -6., -0.5];
const STRIP_PIVOT_L: [f32; 3] = [-21., -4., 0.25];
const STRIP_AXIS_L: [f32; 3] = [-20., -6., -0.5];
/// Rudder hinge, the leading edge of the lower fin block: from (0,-21,4) up and
/// aft to (0,-26,12). A positive angle swings the trailing edge toward +X
/// (right), as the fleet's rudders do.
const RUDDER_HINGE: [f32; 3] = [0., -21., 4.];
const RUDDER_AXIS: [f32; 3] = [0., -5., 8.];

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

/// How far the wing panel has turned from its spread pose, radians (0 spread,
/// `SWEEP_MAX - SWEEP_MIN` degrees fully swept): a speed schedule, from 304
/// knots (350 mph, John 2026-11-12) to 600, like the MiG-23's, and the flaps
/// hold the wing out. This does not invent a flight
/// law; it only draws the wing.
pub(crate) fn sweep_turn(state: &State) -> f64 {
    tore_sim::combat::swing::wing_turn(state.speed, state.flaps)
}
/// The wing's leading-edge sweep angle for a turn, degrees (25 spread to 67).
pub(crate) fn sweep_degrees(turn: f64) -> f64 {
    SWEEP_MIN + turn.to_degrees()
}

/// One taileron angle in radians, positive lowering the trailing edge: pitch
/// raises both trailing edges for a positive elevator, a positive aileron raises
/// the right and lowers the left, and the sum is clamped to the taileron limit.
pub(crate) fn tail_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    (-TAIL_PITCH * state.elevator.clamp(-1., 1.) - side * TAIL_ROLL * state.aileron.clamp(-1., 1.))
        .clamp(-TAIL_LIMIT, TAIL_LIMIT)
}

/// Where a store station rides on the swing wing, and how its pylon turns
/// (TOR-3 interface for TOR-5). The real Tornado's wing pylons swivel to stay
/// streamwise as the wing sweeps (John, 2026-11-10, decision 19), so a station
/// on the movable wing keeps its heading with the airframe while its position
/// rides on the wing: the panel turns by [`StationPose::turn`], the pylon turns
/// back by the same angle against the panel, and the store hangs from it with
/// no yaw of its own.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct StationPose {
    /// The station's place now, feet right, up and forward of the aircraft's
    /// reference point (the frame the pylon and store drawing uses).
    pub mount: [f64; 3],
    /// How far the wing panel has turned from spread, radians, positive with the
    /// wing tip moving aft on either side.
    pub turn: f64,
    /// The pylon's yaw against the wing panel, radians, positive to the
    /// aircraft's right: the negative of the panel's own turn, so the store's
    /// heading stays along the fuselage whatever the sweep.
    pub swivel: f64,
    /// The store's yaw against the airframe: always zero, the pylon swivels
    /// to keep it there.
    pub heading: f64,
}

/// A station on one wing, `mount` in feet right, up and forward of the aircraft
/// at the spread pose, carried by the wing's own turn about its pivot. A mount
/// inboard of the pivot or under the fixed glove is carried by the same rigid
/// turn: only stations that hang from the movable panel may use it, and
/// TOR-5 chooses which rows those are.
pub(crate) fn wing_station(right: bool, mount: [f64; 3], turn: f64) -> StationPose {
    StationPose {
        mount: tore_sim::combat::swing::swept(right, mount, turn),
        turn,
        swivel: tore_sim::combat::swing::pylon_swivel(right, turn),
        heading: 0.,
    }
}

/// `shape`'s faces with both wing panels turned `turn_by` radians from spread
/// (the rest of the airframe untouched, the flaps up): the model a swivelling
/// wing station meets and the stores preview measures against (TOR-5).
pub(crate) fn swept_shape(shape: &Shape, turn_by: f64) -> Shape {
    let faces = shape
        .faces
        .iter()
        .map(|face| {
            let mut face = face.clone();
            let a = face.address;
            if wing_right(a) {
                turn(&mut face, PIVOT_R, [0., 0., 1.], -turn_by);
            } else if wing_left(a) {
                turn(&mut face, PIVOT_L, [0., 0., 1.], turn_by);
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

/// The wingtip vapor attachment of one side (0 left, 1 right) for a wing turn,
/// feet right, up and forward of the aircraft. The shape's streamer record is
/// the spread-wing tip (third-of-a-foot CE units, whatever the header scale); it
/// is carried by the same rigid turn as the wing faces, so the vapor leaves the
/// drawn tip at every sweep. The record's own hinge (pivot (20,2,16) CE) is not
/// used: it is not the pivot the wing turns about.
pub(crate) fn streamer_point(
    def: &tore_formats::shape::StreamerDef,
    side: usize,
    turn: f64,
) -> Option<[f64; 3]> {
    let spread = def.attachment(side, 0).ok()?.map(|v| v / 3.);
    Some(wing_station(side == 1, spread, turn).mount)
}

/// Closed airbrake angle, radians: the turn about the forward hinge that lays
/// the raised plate flat at the hinge height (the dorsal skin under both plates
/// sits within 0.3 units of z 2 at the hinge and 2.0 at the free edge on the
/// right, and falls away to 1.1 at the left outer edge). Worked out from the
/// source plate: the free edge is 3 units aft and 2 units above the hinge.
/// Fitted.
pub(crate) fn brake_closed() -> f64 {
    let direction =
        |a: [f32; 3], b: [f32; 3]| f64::atan2(f64::from(b[2] - a[2]), f64::from(b[1] - a[1]));
    let open = direction(BRAKE_HINGE, [0., -10., 4.]);
    let flat = direction(BRAKE_HINGE, [0., -10., 2.]);
    (flat - open).rem_euclid(std::f64::consts::TAU)
}

/// Whole-piece gear motion (TOR-4). Each main leg turns as one rigid body about
/// a lateral axis in its bay and folds forward through a quarter turn; the nose
/// leg does the same about its own bay axis and steers about its vertical axis
/// first. Gear down (1) is the source pose; hidden at gear 0 by the caller.
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let angle = std::f64::consts::FRAC_PI_2 * (1. - gear.clamp(0., 1.));
    let a = f.address;
    if GEAR_NOSE.contains(&a) {
        if steering != 0. {
            turn(f, NOSE_STEER, [0., 0., 1.], -steering);
        }
        turn(f, NOSE_HINGE, [1., 0., 0.], angle);
    } else if GEAR_MAIN_R.contains(&a) {
        turn(f, MAIN_HINGE_R, [1., 0., 0.], angle);
    } else if GEAR_MAIN_L.contains(&a) {
        turn(f, MAIN_HINGE_L, [1., 0., 0.], angle);
    }
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("TOR.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("TOR.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let pose = Shape::with_state(bytes, &[(word, 1)].into())?;
    let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
    if active.difference(neutral).copied().collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || neutral
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed TOR.SH branch {word:x}").into());
    }
    Ok(pose)
}

/// The four glove plate faces: per side a top face and its reversed winding,
/// taking colours, texture and flags from the glove's own outer top triangle.
fn glove_plates(shape: &Shape) -> AppResult<Vec<Face>> {
    let mut faces = Vec::new();
    for (look, outline, ids) in [
        (GLOVE_LOOK_R, GLOVE_OUTLINE_R, GLOVE_PLATE_R),
        (GLOVE_LOOK_L, GLOVE_OUTLINE_L, GLOVE_PLATE_L),
    ] {
        let source = one(shape, look)?;
        if source.positions.len() != 3 || !source.positions.iter().all(|p| p[2] == 1. || p[2] == 2.)
        {
            return Err(format!("unreviewed TOR.SH glove top face {look:x}").into());
        }
        let top: Vec<[f32; 3]> = outline.iter().map(|p| [p[0], p[1], GLOVE_Z]).collect();
        let mut under = top.clone();
        under.reverse();
        for (address, positions, flip) in [(ids[0], top, 1.), (ids[1], under, -1.)] {
            let n = positions.len();
            faces.push(Face {
                address,
                colors: vec![source.colors[0]; n],
                uv: if source.uv.is_empty() {
                    Vec::new()
                } else {
                    vec![source.uv[0]; n]
                },
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

fn has(face: &Face, p: [f32; 3]) -> bool {
    face.positions.contains(&p)
}

pub struct Rig;
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 28150
            || shape.faces.len() != 287
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed TOR.SH layout".into());
        }
        // Wing panels: 13 whole faces a side, each present once. The five
        // vertices shared with the glove fillets are in the panel.
        for (ids, glove) in [
            (
                &WING_R,
                &[
                    [9., 1., 0.],
                    [9., 1., 1.],
                    [10., -1., 0.],
                    [10., -1., 1.],
                    [10., -4., 0.],
                ],
            ),
            (
                &WING_L,
                &[
                    [-10., 1., 0.],
                    [-10., 1., 1.],
                    [-11., -1., 0.],
                    [-11., -1., 1.],
                    [-11., -4., 0.],
                ],
            ),
        ] {
            for &address in ids {
                one(&shape, address)?;
            }
            for &p in glove {
                if !ids.iter().any(|&a| one(&shape, a).is_ok_and(|f| has(f, p))) {
                    return Err(format!("unreviewed TOR.SH wing glove vertex {p:?}").into());
                }
            }
        }
        // The two flap strips: the wing's trailing strips, hinged on the
        // leading edge from the glove root to the outer corner.
        for (ids, root, tip) in [
            (&STRIP_R, 10., [30., -7., 0.]),
            (&STRIP_L, -11., [-31., -7., 0.]),
        ] {
            for &address in ids {
                let face = one(&shape, address)?;
                if face.positions.len() != 4
                    || !has(face, tip)
                    || !face
                        .positions
                        .iter()
                        .any(|p| p[0] == root && p[1] == -1. && p[2] <= 1.)
                {
                    return Err(format!("unreviewed TOR.SH flap strip {address:x}").into());
                }
            }
        }
        // Tailerons: five vertices, all at z -2, with the root chord on x 6.
        for (ids, side) in [(&TAIL_R, 1f32), (&TAIL_L, -1f32)] {
            for &address in ids {
                let face = one(&shape, address)?;
                if face.positions.len() != 5
                    || !face.positions.iter().all(|p| p[2] == -2.)
                    || !has(face, [side * 6., -25., -2.])
                    || !has(face, [side * 6., -20., -2.])
                    || !has(face, [side * 7., -8., -2.])
                {
                    return Err(format!("unreviewed TOR.SH taileron {address:x}").into());
                }
            }
        }
        // Rudder: the lower fin block, a polygon in the fin plane whose
        // leading edge is the hinge.
        for &address in &RUDDER {
            let face = one(&shape, address)?;
            if face.positions.len() != 5
                || !face.positions.iter().all(|p| p[0] == 0.)
                || !has(face, RUDDER_HINGE)
                || !has(face, [0., -26., 12.])
            {
                return Err(format!("unreviewed TOR.SH rudder block {address:x}").into());
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words remove exactly the strips (0x7dd8 left, 0x7dde right)
        // and add nothing. They pin the strips as the surfaces the source
        // treats as flaps; the rig never sets them.
        branch(bytes, &original, 0x7dd8, &[], &STRIP_L)?;
        branch(bytes, &original, 0x7dde, &[], &STRIP_R)?;
        // The rudder word swaps the block for a skewed copy.
        branch(bytes, &original, 0x7de4, &RUDDER_WORD, &RUDDER)?;
        // The gear, airbrake and burner words add exactly the reviewed faces and
        // remove nothing. Every plate's corners are checked against the reviewed
        // source box, so the rig refuses a shape that differs.
        let gear = branch(bytes, &original, 0x7dcc, &GEAR, &[])?;
        let brake = branch(bytes, &original, 0x7dc6, &BRAKE, &[])?;
        let flame = branch(bytes, &original, 0x7dc0, &FLAME, &[])?;
        let inside = |face: &Face, lo: [f32; 3], hi: [f32; 3]| {
            face.positions.len() == 4
                && face
                    .positions
                    .iter()
                    .all(|p| (0..3).all(|k| p[k] >= lo[k] && p[k] <= hi[k]))
        };
        for (ids, lo, hi) in [
            (&GEAR_MAIN_R[..], [3., -2., -11.], [7., 3., -4.]),
            (&GEAR_MAIN_L[..], [-7., -2., -11.], [-3., 3., -4.]),
            (&GEAR_NOSE[..], [-1., 25., -11.], [1., 29., -5.]),
        ] {
            for &address in ids {
                if !inside(one(&gear, address)?, lo, hi) {
                    return Err(format!("unreviewed TOR.SH gear plate {address:x}").into());
                }
            }
        }
        for (ids, points) in [
            (
                &BRAKE_R,
                [[5., -7., 2.], [2., -7., 2.], [2., -10., 4.], [5., -10., 4.]],
            ),
            (
                &BRAKE_L,
                [
                    [-2., -7., 2.],
                    [-6., -7., 2.],
                    [-5., -10., 4.],
                    [-3., -10., 4.],
                ],
            ),
        ] {
            for &address in ids {
                let face = one(&brake, address)?;
                if face.positions.len() != 4 || !points.iter().all(|&q| has(face, q)) {
                    return Err(format!("unreviewed TOR.SH airbrake plate {address:x}").into());
                }
            }
        }
        // Each nozzle is one exit octagon on the plane y -24; the plume plates
        // run from one unit behind it to y -43.
        for (address, side) in NOZZLE.into_iter().zip([-1f32, 1.]) {
            let face = one(&shape, address)?;
            if face.positions.len() != 8
                || !face.positions.iter().all(|p| p[1] == NOZZLE_PLANE)
                || !face
                    .positions
                    .iter()
                    .all(|p| p[0] * side > 1. && p[0] * side < 6.)
            {
                return Err(format!("unreviewed TOR.SH nozzle {address:x}").into());
            }
        }
        let (mut root, mut tip) = (f32::NEG_INFINITY, f32::INFINITY);
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4 {
                return Err(format!("unreviewed TOR.SH plume plate {address:x}").into());
            }
            for p in &face.positions {
                root = root.max(p[1]);
                tip = tip.min(p[1]);
            }
        }
        if root != NOZZLE_PLANE - 1. || tip != FLAME_TIP {
            return Err("unreviewed TOR.SH plume extent".into());
        }
        let plates = glove_plates(&shape)?;
        shape.faces.extend(plates);
        shape.faces.extend(
            gear.faces
                .into_iter()
                .filter(|f| GEAR.contains(&f.address))
                .chain(
                    brake
                        .faces
                        .into_iter()
                        .filter(|f| BRAKE.contains(&f.address)),
                )
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
            if flaps > 0. && (STRIP_R.contains(&a) || STRIP_L.contains(&a)) {
                let (pivot, axis, angle) = if right {
                    (STRIP_PIVOT, STRIP_AXIS, FLAP * flaps)
                } else {
                    (STRIP_PIVOT_L, STRIP_AXIS_L, -FLAP * flaps)
                };
                turn(&mut result, pivot, axis, angle);
            }
            let swept = sweep_turn(state);
            if swept > 0. {
                turn(
                    &mut result,
                    if right { PIVOT_R } else { PIVOT_L },
                    [0., 0., 1.],
                    if right { -swept } else { swept },
                );
            }
        } else if tail_right(a) || tail_left(a) {
            // The whole panel turns about the lateral spindle through the root
            // chord midpoint: no vertex is held while another turns. The panel
            // is flat (every vertex at z -2), so it has no anhedral.
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
        } else if rudder(a) {
            // One rigid panel about the fin block's leading edge: the two
            // vertices on that edge stay, the rest swing out of the fin plane.
            let angle = RUDDER_TRAVEL * state.rudder.clamp(-1., 1.);
            if angle != 0. {
                turn(&mut result, RUDDER_HINGE, RUDDER_AXIS, angle);
            }
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
        } else if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            // Open (brake 1) is the source pose; closing turns the whole plate
            // about its forward edge down onto the spine.
            turn(
                &mut result,
                BRAKE_HINGE,
                [1., 0., 0.],
                brake_closed() * (1. - state.brake.clamp(0., 1.)),
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
    fn rig() -> Rig {
        Rig
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

    /// The right wing's glove vertices in (x, y), for the pivot test.
    const GLOVE_R: [[f64; 2]; 3] = [[9., 1.], [10., -1.], [10., -4.]];

    #[test]
    fn the_pivot_is_the_centre_of_the_smallest_circle_round_the_glove_vertices() {
        // No other point has a smaller worst distance to the three glove
        // corners: the seam opens by 2 r sin(turn / 2) at most.
        let radius = |c: [f64; 2]| {
            GLOVE_R
                .iter()
                .map(|p| ((p[0] - c[0]).powi(2) + (p[1] - c[1]).powi(2)).sqrt())
                .fold(0., f64::max)
        };
        let here = radius([f64::from(PIVOT_R[0]), f64::from(PIVOT_R[1])]);
        assert!((here - 2.5495).abs() < 1e-3);
        for dx in -20..=20 {
            for dy in -20..=20 {
                let c = [
                    f64::from(PIVOT_R[0]) + f64::from(dx) * 0.1,
                    f64::from(PIVOT_R[1]) + f64::from(dy) * 0.1,
                ];
                assert!(radius(c) >= here - 1e-9, "{c:?}");
            }
        }
        // The left glove is the right one shifted one unit out and mirrored.
        assert_eq!(PIVOT_L[0], -PIVOT_R[0] - 1.);
        assert_eq!(PIVOT_L[1], PIVOT_R[1]);
    }

    #[test]
    fn the_sweep_schedule_runs_from_spread_to_67_degrees_and_flaps_hold_it_out() {
        let mut s = state();
        for (kt, degrees) in [
            (0., 25.),
            (250., 25.),
            // 350 mph is 304 knots: the wing is still spread there.
            (304., 25.),
            (452., 46.),
            (600., 67.),
            (900., 67.),
        ] {
            knots(&mut s, kt);
            s.flaps = 0.;
            assert!(
                (sweep_degrees(sweep_turn(&s)) - degrees).abs() < 1e-9,
                "{kt}"
            );
            s.flaps = 1.;
            assert_eq!(sweep_turn(&s), 0.);
        }
    }

    #[test]
    fn the_wing_turns_whole_about_its_pivot_and_the_tip_goes_aft() {
        let rig = rig();
        let mut s = state();
        for (a, side) in [(0x4eba, 1f32), (0x537e, -1f32)] {
            let pts = [
                [side * 33., -10., 0.],
                [side * 30., -9., 0.],
                [side * 30., -7., 0.],
            ];
            let source = face(a, &pts);
            knots(&mut s, 200.);
            assert_eq!(rig.animate(&source, &s).unwrap().positions, pts);
            for kt in [310., 425., 600., 800.] {
                knots(&mut s, kt);
                let out = rig.animate(&source, &s).unwrap();
                assert!(out.positions[0][1] < source.positions[0][1]);
                assert!(out.positions[0][0].abs() < source.positions[0][0].abs());
                assert!(spread(&source.positions, &out.positions) < 1e-4);
                // The tip stays at its distance from the pivot.
                let pivot = if side > 0. { PIVOT_R } else { PIVOT_L };
                let r = |p: [f32; 3]| (p[0] - pivot[0]).hypot(p[1] - pivot[1]);
                assert!((r(out.positions[0]) - r(source.positions[0])).abs() < 1e-3);
            }
        }
    }

    #[test]
    fn the_flap_strips_droop_about_their_leading_edge_and_ride_with_the_sweep() {
        let rig = rig();
        let mut s = state();
        for (a, side) in [(0x501c, 1f32), (0x5570, -1f32)] {
            let (root, tip) = if side > 0. { (10., 30.) } else { (-11., -31.) };
            let pts = [
                [root, -4., 0.],
                [tip, -9., 0.],
                [tip, -7., 0.],
                [root, -1., 0.],
            ];
            let source = face(a, &pts);
            knots(&mut s, 150.);
            s.flaps = 1.;
            let out = rig.animate(&source, &s).unwrap();
            // The trailing corners drop, the strip stays rigid.
            assert!(out.positions[1][2] < source.positions[1][2] - 0.5);
            assert!(out.positions[0][2] < source.positions[0][2] - 0.5);
            assert!(spread(&source.positions, &out.positions) < 1e-4);
            // With the flaps up the strip is the wing's: the sweep carries it aft.
            s.flaps = 0.;
            knots(&mut s, 600.);
            let out = rig.animate(&source, &s).unwrap();
            assert!(out.positions[1][1] < source.positions[1][1]);
            assert!(spread(&source.positions, &out.positions) < 1e-4);
        }
    }

    #[test]
    fn whole_taileron_panels_keep_their_root_midpoint_on_pitch_and_roll() {
        let rig = rig();
        let mut s = state();
        for (a, side) in [(0x446a, 1f32), (0x4176, -1f32)] {
            let pts = [
                [side * 18., -27., -2.],
                [side * 6., -25., -2.],
                [side * 6., -20., -2.],
                [side * 7., -8., -2.],
                [side * 18., -23., -2.],
            ];
            let source = face(a, &pts);
            for control in [-1., -0.5, 0., 0.5, 1.] {
                for (elevator, aileron) in [(control, 0.), (0., control), (control, 1.)] {
                    s.elevator = elevator;
                    s.aileron = aileron;
                    let out = rig.animate(&source, &s).unwrap();
                    let mid = (out.positions[1][1] + out.positions[2][1]) * 0.5;
                    let midz = (out.positions[1][2] + out.positions[2][2]) * 0.5;
                    assert!((mid + 22.5).abs() < 1e-4 && (midz + 2.).abs() < 1e-4);
                    assert!(spread(&source.positions, &out.positions) < 1e-3);
                    let angle = tail_angle(side > 0., &s);
                    assert!(angle.abs() <= TAIL_LIMIT + 1e-12);
                    // The trailing tip moves up for a positive angle command sign.
                    let dz = out.positions[0][2] - source.positions[0][2];
                    assert!(dz * (angle as f32) <= 1e-6);
                }
            }
        }
    }

    #[test]
    fn the_taileron_limit_is_below_the_fleet_clamp_and_pitch_and_roll_have_signs() {
        let mut s = state();
        s.elevator = 1.;
        assert_eq!(tail_angle(true, &s), -TAIL_PITCH);
        s.elevator = 0.;
        s.aileron = 1.;
        assert_eq!(tail_angle(true, &s), -TAIL_ROLL);
        assert_eq!(tail_angle(false, &s), TAIL_ROLL);
        s.elevator = 1.;
        assert_eq!(tail_angle(true, &s), -TAIL_LIMIT);
        // The forward corner is 14.5 units ahead of the spindle.
        let swing = 2. * 14.5 * (TAIL_LIMIT / 2.).sin();
        assert!((swing * FEET_PER_UNIT - 2.41).abs() < 0.01);
    }

    #[test]
    fn the_rudder_block_turns_whole_about_its_leading_edge() {
        let rig = rig();
        let mut s = state();
        let pts = [
            [0., -31., 12.],
            [0., -25., 3.],
            [0., -21., 3.],
            [0., -21., 4.],
            [0., -26., 12.],
        ];
        let source = face(0x5723, &pts);
        for value in [-1., -0.5, 0., 0.5, 1.] {
            s.rudder = value;
            let out = rig.animate(&source, &s).unwrap();
            assert_eq!(out.positions[3], pts[3]);
            assert!((out.positions[4][1] - -26.).abs() < 1e-4);
            assert!((out.positions[4][2] - 12.).abs() < 1e-4);
            assert!(spread(&source.positions, &out.positions) < 1e-4);
            if value != 0. {
                for i in [0usize, 1] {
                    assert!(out.positions[i][0] * value as f32 > 0.);
                }
            } else {
                assert_eq!(out.positions, source.positions);
            }
        }
    }

    #[test]
    fn the_vapor_attachment_follows_the_swept_tip() {
        let def = tore_formats::shape::StreamerDef {
            pivot: [5120, 512, 4096],
            hinge_scale: 40,
            points: [[12288, 0, -8704]; 2],
        };
        // Spread: the record's own tip, 68 CE units out and 18 aft of the origin.
        for (side, x) in [(0usize, -68. / 3.), (1, 68. / 3.)] {
            let spread = streamer_point(&def, side, 0.).unwrap();
            assert!((spread[0] - x).abs() < 1e-9);
            assert!((spread[1] - 2. / 3.).abs() < 1e-9);
            assert!((spread[2] + 6.).abs() < 1e-9);
            let full = streamer_point(&def, side, (SWEEP_MAX - SWEEP_MIN).to_radians()).unwrap();
            // Aft and in, at the same distance from the pivot.
            assert!(full[2] < spread[2] - 5.);
            assert!(full[0].abs() < spread[0].abs() - 3.);
            let pivot = pivot_feet(side == 1);
            let r = |p: [f64; 3]| (p[0] - pivot[0]).hypot(p[2] - pivot[2]);
            assert!((r(full) - r(spread)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_wing_station_rides_the_wing_and_the_pylon_swivels_back() {
        for right in [true, false] {
            let side = if right { 1. } else { -1. };
            // The right wing tip vertex, in the station frame.
            let tip_units = [side * 33., -10., 0.];
            let mount = [
                f64::from(tip_units[0]) * FEET_PER_UNIT,
                f64::from(tip_units[2]) * FEET_PER_UNIT,
                f64::from(tip_units[1]) * FEET_PER_UNIT,
            ];
            let rig = rig();
            let mut s = state();
            for kt in [200., 300., 425., 600.] {
                knots(&mut s, kt);
                let turned = sweep_turn(&s);
                let pose = wing_station(right, mount, turned);
                let source = face(
                    if right { 0x4eba } else { 0x537e },
                    &[tip_units, [side * 30., -9., 0.], [side * 30., -7., 0.]],
                );
                let wing = rig.animate(&source, &s).unwrap().positions[0];
                // The mount lands exactly on the swept wing vertex it was given.
                assert!((pose.mount[0] - f64::from(wing[0]) * FEET_PER_UNIT).abs() < 1e-4);
                assert!((pose.mount[2] - f64::from(wing[1]) * FEET_PER_UNIT).abs() < 1e-4);
                assert_eq!(pose.mount[1], mount[1]);
                assert_eq!(pose.heading, 0.);
                assert!((pose.turn + if right { pose.swivel } else { -pose.swivel }).abs() < 1e-12);
            }
        }
    }

    #[test]
    fn each_gear_piece_turns_whole_and_folds_forward_to_a_stow_inside_the_hull() {
        let rig = rig();
        let mut s = state();
        // The right main leg's wheel sheet (the slanted side plate 0x4908), the
        // left one (0x4705) and the nose cross sheet (0x4ad5), as in the source.
        for (a, pts) in [
            (
                0x4908,
                [
                    [6., -1., -11.],
                    [5., -1., -4.],
                    [5., 3., -4.],
                    [6., 3., -11.],
                ],
            ),
            (
                0x4705,
                [
                    [-6., -1., -11.],
                    [-6., 3., -11.],
                    [-5., 3., -4.],
                    [-5., -1., -4.],
                ],
            ),
            (
                0x4ad5,
                [
                    [1., 27., -11.],
                    [-1., 27., -11.],
                    [-1., 27., -5.],
                    [1., 27., -5.],
                ],
            ),
        ] {
            let source = face(a, &pts);
            // Gear down is the source pose, so the ground contact stays -11.
            s.gear = 1.;
            assert_eq!(rig.animate(&source, &s).unwrap().positions, pts);
            // Gear up draws nothing.
            s.gear = 0.;
            assert!(rig.animate(&source, &s).is_none());
            for gear in [0.75, 0.5, 0.25, 0.1, 1e-6] {
                s.gear = gear;
                let out = rig.animate(&source, &s).unwrap();
                // Rigid, and the foot (the lowest corner) travels forward.
                assert!(spread(&source.positions, &out.positions) < 1e-4, "{a:x}");
                assert!(out.positions[0][1] > pts[0][1], "{a:x} {gear}");
                // Nothing ever hangs below the gear-down pose.
                assert!(out.positions.iter().all(|p| p[2] >= -11. - 1e-4));
            }
            // Stowed: the foot has swung up to the hinge height and forward.
            let foot = rig.animate(&source, &s).unwrap().positions[0];
            let hinge = if a == 0x4ad5 {
                NOSE_HINGE
            } else if pts[0][0] > 0. {
                MAIN_HINGE_R
            } else {
                MAIN_HINGE_L
            };
            assert!(
                (foot[1] - hinge[1] - (hinge[2] - pts[0][2])).abs() < 1e-3,
                "{a:x}"
            );
            assert!(foot[2] > pts[0][2] + 6.);
        }
    }

    #[test]
    fn the_nose_cross_steers_about_its_vertical_axis_and_keeps_its_centre() {
        let rig = rig();
        let mut s = state();
        s.gear = 1.;
        let pts = [
            [0., 25., -11.],
            [0., 29., -11.],
            [0., 29., -5.],
            [0., 25., -5.],
        ];
        let source = face(0x4a97, &pts);
        let out = rig.animate(&source, &s).unwrap();
        assert_eq!(out.positions, pts);
        // A steered wheel stays rigid and turns about x 0, y 27.
        let mut face_steered = source.clone();
        gear_positions(&mut face_steered, 1., 0.5);
        assert!(spread(&source.positions, &face_steered.positions) < 1e-4);
        let centre = |f: &Face| {
            [
                f.positions.iter().map(|p| p[0]).sum::<f32>() / 4.,
                f.positions.iter().map(|p| p[1]).sum::<f32>() / 4.,
            ]
        };
        assert!((centre(&face_steered)[1] - 27.).abs() < 1e-4);
        assert!(centre(&face_steered)[0].abs() < 1e-4);
        assert!(face_steered.positions[0][0].abs() > 0.5);
    }

    #[test]
    fn the_airbrake_plate_turns_whole_about_its_forward_edge_and_lies_flat_closed() {
        let rig = rig();
        let mut s = state();
        for (a, pts) in [
            (
                0x57ec,
                [[5., -7., 2.], [2., -7., 2.], [2., -10., 4.], [5., -10., 4.]],
            ),
            (
                0x5889,
                [
                    [-3., -10., 4.],
                    [-5., -10., 4.],
                    [-6., -7., 2.],
                    [-2., -7., 2.],
                ],
            ),
        ] {
            let source = face(a, &pts);
            s.brake = 0.;
            assert!(rig.animate(&source, &s).is_none());
            s.brake = 1.;
            assert_eq!(rig.animate(&source, &s).unwrap().positions, pts);
            assert!((brake_closed().to_degrees() - (2f64 / 3.).atan().to_degrees()).abs() < 1e-9);
            for brake in [0.75, 0.5, 0.25, 1e-9] {
                s.brake = brake;
                let out = rig.animate(&source, &s).unwrap();
                assert!(spread(&source.positions, &out.positions) < 1e-4, "{brake}");
                // The forward edge (y -7) is the hinge and stays.
                for (moved, before) in out.positions.iter().zip(&pts) {
                    if before[1] == -7. {
                        for k in 0..3 {
                            assert!((moved[k] - before[k]).abs() < 1e-4);
                        }
                    } else {
                        // The free edge only comes down as the brake closes.
                        assert!(moved[2] < before[2] - 1e-3);
                    }
                }
            }
            // Closed: the free edge is level with the hinge, flat at z 2.
            let closed = rig.animate(&source, &s).unwrap();
            for i in 0..4 {
                assert!((closed.positions[i][2] - 2.).abs() < 1e-4);
            }
        }
    }

    #[test]
    fn the_plume_grows_from_the_nozzle_exit_plane() {
        let rig = rig();
        let mut s = state();
        let pts = [
            [-3., -43., 1.],
            [-3., -25., 1.],
            [-3., -25., -3.],
            [-3., -43., -3.],
        ];
        let source = face(0x591e, &pts);
        s.exhaust = 0.;
        assert!(rig.animate(&source, &s).is_none());
        s.exhaust = 1.;
        assert_eq!(rig.animate(&source, &s).unwrap().positions, pts);
        s.exhaust = 0.5;
        let out = rig.animate(&source, &s).unwrap();
        // The tip pulls forward toward the exit plane y -24, the shared edge
        // pair keeps its length ratio, x and z do not move.
        assert!((out.positions[0][1] - (-24. + (-43. + 24.) * 0.5)).abs() < 1e-4);
        assert!((out.positions[1][1] - (-24. + (-25. + 24.) * 0.5)).abs() < 1e-4);
        for (moved, before) in out.positions.iter().zip(&pts) {
            assert_eq!(moved[0], before[0]);
            assert_eq!(moved[2], before[2]);
        }
        assert!(flame(0x591e) && flame(0x59f7) && !flame(0x57ec));
    }

    /// The point-in-outline test the drawing's own fan of triangles implies
    /// (every face is cut `p[0], p[i], p[i + 1]`).
    fn in_fan(outline: &[[f32; 2]], q: [f64; 2]) -> bool {
        let p: Vec<[f64; 2]> = outline
            .iter()
            .map(|v| [f64::from(v[0]), f64::from(v[1])])
            .collect();
        (1..p.len() - 1).any(|i| {
            let side = |a: [f64; 2], b: [f64; 2]| {
                (b[0] - a[0]) * (q[1] - a[1]) - (b[1] - a[1]) * (q[0] - a[0])
            };
            let s = [side(p[0], p[i]), side(p[i], p[i + 1]), side(p[i + 1], p[0])];
            s.iter().all(|v| *v <= 1e-9) || s.iter().all(|v| *v >= -1e-9)
        })
    }

    fn glove_source() -> Shape {
        // The two glove top triangles the plates take their look from.
        let tri = |address, pts: [[f32; 3]; 3]| {
            let mut f = face(address, &pts);
            f.normal = Some([0., 0., 1.]);
            f
        };
        Shape {
            lines: Vec::new(),
            faces: vec![
                tri(GLOVE_LOOK_R, [[7., 13., 1.], [10., 8., 1.], [10., 6., 1.]]),
                tri(
                    GLOVE_LOOK_L,
                    [[-10., 6., 1.], [-10., 8., 1.], [-7., 13., 1.]],
                ),
            ],
            billboards: Vec::new(),
            state_words: BTreeSet::new(),
        }
    }

    #[test]
    fn the_glove_plates_are_four_whole_faces_above_the_wing() {
        // TOR-5b, John 2026-11-11 (T4): per side a top face and its reversed
        // winding, one flat polygon each, wholly above the wing (top z 1).
        let plates = glove_plates(&glove_source()).unwrap();
        assert_eq!(plates.len(), 4);
        let addresses: Vec<usize> = plates.iter().map(|f| f.address).collect();
        assert_eq!(
            addresses,
            [GLOVE_PLATE_R, GLOVE_PLATE_L].concat(),
            "right pair then left pair"
        );
        for pair in plates.chunks(2) {
            let (top, under) = (&pair[0], &pair[1]);
            assert_eq!(top.positions.len(), 5);
            let reversed: Vec<_> = top.positions.iter().rev().copied().collect();
            assert_eq!(under.positions, reversed);
            assert!(top.positions.iter().all(|p| p[2] == GLOVE_Z));
            assert_eq!(under.normal.unwrap()[2], -top.normal.unwrap()[2]);
        }
        // Past the end of the module's code, so no source word can name them.
        assert!(addresses.iter().all(|a| *a >= 28150));
        // Above everything the wing panel can reach (its faces sit at z 0 and 1).
        let mut s = state();
        let rig = rig();
        for turn in [0., 0.4, 0.733] {
            s.speed = (304. + turn / 0.733 * 296.) * 1.68781;
            for plate in &plates {
                let out = rig.animate(plate, &s).unwrap();
                assert_eq!(out.positions, plate.positions, "fixed to the fuselage");
            }
        }
        // A shape without the glove face it takes its look from is refused.
        assert!(
            glove_plates(&Shape {
                faces: Vec::new(),
                ..glove_source()
            })
            .is_err()
        );
    }

    #[test]
    fn the_glove_plates_fill_the_wedge_the_wing_opens_at_full_sweep() {
        // The swept root line and the glove's outer edge bound the wedge; every
        // point between them, from the glove's front corner back to the root's
        // rear corner, is under a plate, at the full 42 degree turn.
        let turn = (SWEEP_MAX - SWEEP_MIN).to_radians();
        let swept = |right: bool, p: [f64; 2]| {
            let pivot = if right { PIVOT_R } else { PIVOT_L };
            let (dx, dy) = (p[0] - f64::from(pivot[0]), p[1] - f64::from(pivot[1]));
            let a = if right { -turn } else { turn };
            let (s, c) = a.sin_cos();
            [
                f64::from(pivot[0]) + dx * c - dy * s,
                f64::from(pivot[1]) + dx * s + dy * c,
            ]
        };
        for (right, outline, root, edge) in [
            (true, &GLOVE_OUTLINE_R, 9.0, 10.0),
            (false, &GLOVE_OUTLINE_L, -10.0, -10.0),
        ] {
            let mut covered = 0;
            for k in 0..=11 {
                // A point on the wing's root line (x 9 right, x -10 left, y 1 to 6.5).
                let on_root = swept(right, [root, 1. + f64::from(k) * 0.5]);
                assert!(in_fan(outline, on_root), "root line {on_root:?}");
                // Points between the glove's outer edge and that root point, at its y.
                if on_root[1] > 0.8 {
                    for j in 0..=8 {
                        let x = edge + (on_root[0] - edge) * f64::from(j) / 8.;
                        assert!(in_fan(outline, [x, on_root[1]]), "wedge {x} {}", on_root[1]);
                        covered += 1;
                    }
                }
            }
            assert!(covered > 50);
            // The glove's outer front corner is a plate corner.
            assert!(outline.iter().any(|v| v[0] == edge as f32 && v[1] == 8.));
            // The root's leading end lands within the margin of a plate corner.
            let end = swept(right, [root, 7.]);
            let near = outline
                .iter()
                .any(|v| (f64::from(v[0]) - end[0]).hypot(f64::from(v[1]) - end[1]) < 0.4);
            assert!(near, "leading root end {end:?}");
        }
        // The wing's spread pose keeps every plate point over the panel or ahead
        // of its leading edge: no plate point is behind the trailing edge.
        for outline in [&GLOVE_OUTLINE_R, &GLOVE_OUTLINE_L] {
            assert!(
                outline
                    .iter()
                    .all(|v| v[1] >= -1.5 && v[0].abs() >= 10. - 1e-6 && v[0].abs() <= 16.)
            );
        }
    }
}
