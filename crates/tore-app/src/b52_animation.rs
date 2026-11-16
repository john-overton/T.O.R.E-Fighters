//! B-52H (`B52.SH`) own-source rig (B52-3 and B52-4): the two floating
//! stabilizer quads, the two inboard flap wedges, the full-height rudder, the
//! four tandem main trucks and two wingtip outriggers (one gear word) and, since
//! B52-5, the fitted weapon bay (one belly skin cut into two doors, a lined
//! recess with rails and posts, drawn through [`crate::bay_doors`]).
//! See docs/formats/aircraft-variety.md "B-52 rig (B52-3, 2026-11-15)" and "B-52
//! bay, pylons and tail gun (B52-5, 2026-11-15)".
//!
//! B52.SH ships five state words: 0x7a20 the bay (two plates, both windings,
//! hanging from the belly; since B52-5 only reviewed, its plate is the material
//! of the lining, and the plates themselves are no longer drawn), 0x7a26 the gear (32 faces: four trucks of six and
//! two outriggers of four), 0x7a2c and 0x7a32 the left and right flap words
//! (each deletes the inboard trailing wedge and adds nothing) and 0x7a38 the
//! rudder (swaps the full-height rudder quad for a copy skewed to one side,
//! about 31 degrees). The rig never sets the flap or rudder words: it keeps the
//! neutral wedges and the neutral rudder drawn and turns them whole, so the
//! wing and the fin never show a hole. The shape has no elevator, aileron,
//! spoiler, slat, airbrake, burner or flame word and none is invented; roll is
//! not drawn (decision 11, John 2026-11-15).
//!
//! The shape's per-axis scale is the identity (it draws at 2/3 ft a unit through
//! the rig scale), and every hinge point below is still passed through
//! [`scale_point`], so a panel turned about it stays rigid.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};
use tore_sim::combat::bays::{B52_ROW_FORWARD, B52_SLAB, BayGrid, CentreBay};

const ID: AircraftId = AircraftId::B52;
const WORDS: [usize; 5] = [0x7a20, 0x7a26, 0x7a2c, 0x7a32, 0x7a38];

/// Stabilizer: one four-vertex polygon per side at z 5, both windings, sharing
/// no vertex with any other face (B52-0).
pub(crate) const STAB_R: [usize; 2] = [0x334e, 0x3361];
pub(crate) const STAB_L: [usize; 2] = [0x3167, 0x317a];
/// Root chord midpoint of each polygon, the lateral spindle (an X axis, the
/// quad is flat so it has no anhedral to tilt by). Rule 11.
const STAB_SPINDLE_R: [f32; 3] = [4.5, -100.5, 5.];
const STAB_SPINDLE_L: [f32; 3] = [-4.5, -100.5, 5.];
/// The real stabilizer range (Wikipedia, "Boeing B-52 Stratofortress", Flight
/// controls: "adjustable through 13 degrees of movement (nine up, four down)"),
/// read as nose-up trim 9 degrees (trailing edge up) and nose-down trim 4
/// degrees (trailing edge down), the sense of a pull in the fleet. Fitted
/// reading: the source does not say which end "up" means.
pub(crate) const STAB_UP: f64 = 9. * std::f64::consts::PI / 180.;
pub(crate) const STAB_DOWN: f64 = 4. * std::f64::consts::PI / 180.;

/// Stabilizer angle about +X, radians, positive lowering the trailing edge.
/// Choice (fitted, label in the docs): the drawn stabilizer is the pitch surface
/// and follows the elevator command over the real trim range, because the shape
/// has no elevator and the state carries no trim. A pull raises the trailing
/// edge by up to 9 degrees, a push lowers it by up to 4.
pub(crate) fn stabilizer_angle(state: &State) -> f64 {
    let e = state.elevator.clamp(-1., 1.);
    if e >= 0. {
        -STAB_UP * e
    } else {
        -STAB_DOWN * e
    }
}

/// Flap wedges: the inboard trailing strip of each wing, two faces (top and
/// bottom skin) sharing the aft knife edge, the faces the flap words delete.
pub(crate) const FLAP_R: [usize; 2] = [0x4e54, 0x4e73];
pub(crate) const FLAP_L: [usize; 2] = [0x4f3b, 0x4f5a];
/// Hinge line: the mid-thickness line of the thick leading edge (the lower
/// vertices sit at z 6 and 5, the upper at z 10 and 8), inboard end then
/// outboard end. The left line runs outboard to inboard so that a positive
/// angle lowers the trailing edge on both sides.
const FLAP_HINGE_R: [[f32; 3]; 2] = [[15., 7., 8.], [53., -14., 6.5]];
const FLAP_HINGE_L: [[f32; 3]; 2] = [[-53., -14., 6.5], [-15., 7., 8.]];
/// Flap droop, radians (fitted, agent decision 2026-11-15). No cited B-52 flap
/// angle was found. The wedge's trailing corners are shared with fixed faces, so
/// a turn opens a seam of chord (8.2 units) times 2 sin(angle/2): 15 degrees
/// keeps it near 1.4 ft (2.1 units) where the source's 2.7 ft at 30 degrees
/// would show a crack.
pub(crate) const FLAP: f64 = 15. * std::f64::consts::PI / 180.;

/// Rudder: the full-height quad (both windings) in the fin plane x = 0. Its
/// forward edge, (0,-114,42) to (0,-101,11), is shared with the fixed fin
/// faces, so nothing folds.
pub(crate) const RUDDER: [usize; 2] = [0x50f0, 0x5117];
const RUDDER_BOTTOM: [f32; 3] = [0., -101., 11.];
const RUDDER_TOP: [f32; 3] = [0., -114., 42.];
/// Faces the rudder word 0x7a38 swaps in (never drawn).
const RUDDER_WORD: [usize; 2] = [0x5016, 0x503d];
/// Full travel, radians. Spec-derived: the retail word's skewed copy is a turn of
/// 31.8 degrees at the top rear vertex and 30.0 degrees at the bottom rear
/// vertex about the hinge edge, 0.54 rad on average, to +X only; the negative
/// direction is its mirror (fitted). No real B-52 rudder limit was cited
/// ("a relatively small, narrow chord rudder", Wikipedia); `load` checks the word.
pub(crate) const RUDDER_TRAVEL: f64 = 0.54;

/// Gear word 0x7a26: right and left, forward and aft truck, six faces each (a
/// pentagon profile in each of two planes, a side plate and a slanted strut
/// plate, both windings), and two outriggers of four faces (two crossed plates).
pub(crate) const TRUCKS: [[usize; 6]; 4] = [
    [0x49e7, 0x4a0a, 0x4b3d, 0x4b5c, 0x4c35, 0x4c54],
    [0x4a2d, 0x4a50, 0x4aff, 0x4b1e, 0x4bf7, 0x4c16],
    [0x4a73, 0x4ab9, 0x4b7b, 0x4b9a, 0x4c73, 0x4c92],
    [0x4a96, 0x4adc, 0x4bb9, 0x4bd8, 0x4cb1, 0x4cd0],
];
/// Which truck sits on which side: right (+X) then left, forward then aft.
const TRUCK_RIGHT: [bool; 4] = [true, false, true, false];
pub(crate) const OUTRIGGERS: [[usize; 4]; 2] = [
    [0x4779, 0x4798, 0x47b7, 0x47d6],
    [0x482c, 0x484b, 0x486a, 0x4889],
];
pub(crate) const GEAR: [usize; 32] = [
    0x49e7, 0x4a0a, 0x4b3d, 0x4b5c, 0x4c35, 0x4c54, 0x4a2d, 0x4a50, 0x4aff, 0x4b1e, 0x4bf7, 0x4c16,
    0x4a73, 0x4ab9, 0x4b7b, 0x4b9a, 0x4c73, 0x4c92, 0x4a96, 0x4adc, 0x4bb9, 0x4bd8, 0x4cb1, 0x4cd0,
    0x4779, 0x4798, 0x47b7, 0x47d6, 0x482c, 0x484b, 0x486a, 0x4889,
];
/// Truck hinge (fitted, agent decision 2026-11-15; no source hinge and no real
/// direction was found, Jane's has only "four individually retracted twin-wheel
/// main units in tandem pairs"): a line along Y on the keel line at the belly
/// edge, x 0 and z -8. A truck 8 units wide and 8 high does not fit the fuselage
/// section after a smaller turn about any hinge on its own edge; about the keel
/// it swings up the fuselage side and stows 90 degrees on, wholly inside the
/// skin (probe check, both sides: the left truck is one unit wider), and only
/// ever rises from the gear-down pose.
const TRUCK_HINGE: [f32; 3] = [0., 0., -8.];
/// Outrigger hinge: a line along Y at the leg's own x (the plane of its side
/// plate, so that plate lies flat) and at z 4.1, between the wing's lower skin
/// (z 3.7) and its upper skin (4.5 to 5.8). Fitted. The real outriggers retract
/// inward into the wing (Jane's, B52.INF); the cross plate (5 units wide) is
/// thicker than the wing at 0.8 unit, so it stands 3 units through the skins
/// near stow: the one piece that cannot lie inside the skin.
const OUTRIGGER_HINGE_Z: f32 = 4.1;
const OUTRIGGER_X: f32 = 111.;

/// Bay word 0x7a20: two plates (x -7 and +7, y -13 to 26), both windings, hanging
/// from the belly line z -8 to z -13. Only reviewed since B52-5, which cuts the
/// bay from the belly skin below; one plate is the lining's material.
pub(crate) const BAY: [usize; 4] = [0x4dfb, 0x4e12, 0x4d5e, 0x4d75];
/// The flat belly skin the fitted bay is cut from: one six-vertex face at z -9,
/// x -5 to 5, y -46 to 88, facing down (measured, B52-5). The 6 ft wide, 26 ft
/// long opening (x +/-4.5 units, y -13 to 26) lies wholly inside it.
pub(crate) const BELLY: usize = 0x46ae;
/// The first address of the lining's faces (above every face of the shape).
pub(crate) const LINING_BASE: usize = 0x7000;

fn lining(address: usize) -> bool {
    address >= LINING_BASE
}

/// The bay's furniture, feet: a rail under the ceiling on each lane's centre
/// line, running the whole bay, and a post from the floor to the ceiling at
/// the two inner boundaries of the three row slabs, in each lane. Lanes at the
/// pitch of a 500 lb class store (fitted; stores of other sizes space their
/// own grid, so the furniture is not a measure of the load).
fn furniture(o: &CentreBay) -> (Vec<[f64; 3]>, Vec<[f64; 2]>) {
    let pitch = BayGrid::pitch(1.27, 0.45);
    let lanes = [-pitch, 0., pitch];
    let rails = lanes.map(|x| [x, o.aft, o.fore]).to_vec();
    let boundaries = [
        B52_ROW_FORWARD[1] - B52_SLAB / 2.,
        B52_ROW_FORWARD[1] + B52_SLAB / 2.,
    ];
    let posts = lanes
        .iter()
        .flat_map(|&x| boundaries.map(|y| [x, y]))
        .collect();
    (rails, posts)
}

fn right_flap(address: usize) -> bool {
    FLAP_R.contains(&address)
}
fn flap(address: usize) -> bool {
    FLAP_R.contains(&address) || FLAP_L.contains(&address)
}

/// Whole-piece gear motion for one face of the assemblies, `gear` 1 the source
/// pose. Every piece turns about a line along Y by up to 90 degrees; fitted
/// timing: all six move together, linearly with the gear fraction (no stagger).
/// The hinge lines are shape-unit points scaled with `scale_point`.
pub(crate) fn gear_positions(f: &mut Face, gear: f64) {
    let closing = FRAC_PI_2 * (1. - gear.clamp(0., 1.));
    if closing == 0. {
        return;
    }
    let a = f.address;
    let axis = scale_point(ID, [0., 1., 0.]);
    if let Some(i) = TRUCKS.iter().position(|t| t.contains(&a)) {
        // A right truck turns +X to +Z (negative about +Y), a left one the mirror.
        let side = if TRUCK_RIGHT[i] { -1. } else { 1. };
        turn(f, scale_point(ID, TRUCK_HINGE), axis, side * closing);
    } else if let Some(i) = OUTRIGGERS.iter().position(|o| o.contains(&a)) {
        // Inward: down goes toward the fuselage, positive about +Y on the right.
        let (x, side) = if i == 0 {
            (OUTRIGGER_X, 1.)
        } else {
            (-OUTRIGGER_X, -1.)
        };
        turn(
            f,
            scale_point(ID, [x, 0., OUTRIGGER_HINGE_Z]),
            axis,
            side * closing,
        );
    }
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("B52.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("B52.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

/// A state word set to 1 adds exactly `added` and removes exactly `removed`.
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
        return Err(format!("unreviewed B52.SH branch {word:x}").into());
    }
    Ok(pose)
}

fn is(p: [f32; 3], shape_units: [f32; 3]) -> bool {
    p == scale_point(ID, shape_units)
}

/// Every vertex of `face` is one of `allowed` (shape units).
fn only(face: &Face, count: usize, allowed: &[[f32; 3]]) -> bool {
    face.positions.len() == count
        && face
            .positions
            .iter()
            .all(|&p| allowed.iter().any(|&q| is(p, q)))
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

pub struct Rig {
    bay: CentreBay,
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 27204
            || shape.faces.len() != 271
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed B52.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // Stabilizer: the reviewed four corners, both windings.
        for (ids, side) in [(STAB_R, 1.), (STAB_L, -1.)] {
            for address in ids {
                let corners = [
                    [side * 42., -123., 5.],
                    [side * 42., -115., 5.],
                    [side * 6., -81., 5.],
                    [side * 3., -120., 5.],
                ];
                if !only(one(&shape, address)?, 4, &corners) {
                    return Err(format!("unreviewed B52.SH stabilizer face {address:x}").into());
                }
            }
        }
        // Flap wedges: whole faces with the reviewed corners; the hinge midpoints
        // are the leading-edge pair midpoints.
        for (ids, left) in [(FLAP_R, 1.), (FLAP_L, -1.)] {
            let corners = [
                [left * 15., 7., 6.],
                [left * 15., 7., 10.],
                [left * 9., 1., 9.],
                [left * 53., -14., 5.],
                [left * 53., -14., 8.],
                [left * 47., -20., 7.],
                [left * 48., -20., 7.],
            ];
            for address in ids {
                if !only(one(&shape, address)?, 4, &corners) {
                    return Err(format!("unreviewed B52.SH flap face {address:x}").into());
                }
            }
        }
        for address in RUDDER {
            let face = one(&shape, address)?;
            let corners = [
                [0., -122., 42.],
                [0., -114., 42.],
                [0., -101., 11.],
                [0., -117., 11.],
            ];
            if !only(face, 4, &corners)
                || !face.positions.contains(&scale_point(ID, RUDDER_TOP))
                || !face.positions.contains(&scale_point(ID, RUDDER_BOTTOM))
            {
                return Err(format!("unreviewed B52.SH rudder face {address:x}").into());
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words each delete exactly one wedge and add nothing (0x7a2c the
        // left, 0x7a32 the right); the rig pins them and never sets them.
        branch(bytes, &original, 0x7a2c, &[], &FLAP_L)?;
        branch(bytes, &original, 0x7a32, &[], &FLAP_R)?;
        // The rudder word swaps the quad for a skewed copy; check that its turn
        // is the travel this rig uses (the free vertices at the top and bottom
        // rear, within 3 degrees of 0.54 rad each).
        let word = branch(bytes, &original, 0x7a38, &RUDDER_WORD, &RUDDER)?.scaled(axis);
        for (from, to) in [
            ([0., -122., 42.], [4., -121., 42.]),
            ([0., -117., 11.], [8., -116., 11.]),
        ] {
            let angle = turn_angle(
                scale_point(ID, RUDDER_BOTTOM),
                scale_point(ID, RUDDER_TOP),
                scale_point(ID, from),
                scale_point(ID, to),
            );
            if (angle - RUDDER_TRAVEL).abs() > 0.06 {
                return Err("unreviewed B52.SH rudder word angle".into());
            }
            if !word
                .faces
                .iter()
                .filter(|f| RUDDER_WORD.contains(&f.address))
                .any(|f| f.positions.contains(&scale_point(ID, to)))
            {
                return Err("unreviewed B52.SH rudder word corner".into());
            }
        }
        // Gear and bay words add exactly the reviewed plates and remove nothing.
        let gear = branch(bytes, &original, 0x7a26, &GEAR, &[])?.scaled(axis);
        let bay = branch(bytes, &original, 0x7a20, &BAY, &[])?.scaled(axis);
        // The belly skin the fitted bay is cut from: whole, flat, facing down,
        // wider and longer than the opening.
        let opening = tore_sim::combat::bays::centre_bay(ID).ok_or("B52 bay opening missing")?;
        let belly = one(&shape, BELLY)?;
        let feet = |p: &[f32; 3]| [p[0], p[1], p[2]].map(|v| f64::from(v) * opening.unit);
        if !belly.normal.is_some_and(|n| n[1] < -32000.)
            || !belly.positions.iter().all(|p| {
                (feet(p)[2] - opening.belly).abs() < 1e-3 && feet(p)[0].abs() >= opening.half
            })
            || !belly.positions.iter().any(|p| feet(p)[1] <= opening.aft)
            || !belly.positions.iter().any(|p| feet(p)[1] >= opening.fore)
        {
            return Err("unreviewed B52.SH belly skin".into());
        }
        for truck in &TRUCKS {
            for &address in truck {
                if one(&gear, address)?.positions.is_empty() {
                    return Err("unreviewed B52.SH truck".into());
                }
            }
        }
        // The recess, drawn only while the doors are open, takes the material
        // of the shape's own bay plate (one of the interim plates, no longer
        // drawn itself).
        let template = one(&bay, BAY[0])?.clone();
        let (rails, posts) = furniture(&opening);
        let lining_faces = crate::bay_doors::centre_lining(&template, &opening, &rails, &posts)
            .into_iter()
            .enumerate()
            .map(|(k, mut f)| {
                f.address = LINING_BASE + k;
                f
            });
        let mut extra: Vec<Face> = gear
            .faces
            .into_iter()
            .filter(|f| GEAR.contains(&f.address))
            .collect();
        extra.extend(lining_faces);
        shape.faces.extend(extra);
        Ok((Self { bay: opening }, shape))
    }
    /// The faces a source face becomes: with the bay doors off their seat
    /// the belly skin is cut into the two door leaves, turned down about
    /// their hinges by `state.bay` (the doors' 1 second travel is the
    /// flight's); the lining draws only then.
    pub fn faces(&self, source: &Face, state: &State) -> Vec<Face> {
        let a = source.address;
        if a == BELLY && state.bay > 0. {
            return crate::bay_doors::centre_doors(source, state.bay, &self.bay);
        }
        if lining(a) && state.bay <= 0. {
            return Vec::new();
        }
        vec![source.clone()]
    }

    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if let Some(right) = [(&STAB_R, true), (&STAB_L, false)]
            .iter()
            .find_map(|(ids, right)| ids.contains(&a).then_some(*right))
        {
            let spindle = if right {
                STAB_SPINDLE_R
            } else {
                STAB_SPINDLE_L
            };
            let angle = stabilizer_angle(state);
            if angle != 0. {
                turn(
                    &mut result,
                    scale_point(ID, spindle),
                    scale_point(ID, [1., 0., 0.]),
                    angle,
                );
            }
            return Some(result);
        }
        if flap(a) {
            let angle = FLAP * state.flaps.clamp(0., 1.);
            if angle > 0. {
                let hinge = if right_flap(a) {
                    &FLAP_HINGE_R
                } else {
                    &FLAP_HINGE_L
                };
                turn(
                    &mut result,
                    scale_point(ID, hinge[0]),
                    scale_point(ID, std::array::from_fn(|k| hinge[1][k] - hinge[0][k])),
                    angle,
                );
            }
            return Some(result);
        }
        if RUDDER.contains(&a) {
            let angle = RUDDER_TRAVEL * state.rudder.clamp(-1., 1.);
            if angle != 0. {
                turn(
                    &mut result,
                    scale_point(ID, RUDDER_BOTTOM),
                    scale_point(
                        ID,
                        std::array::from_fn(|k| RUDDER_TOP[k] - RUDDER_BOTTOM[k]),
                    ),
                    angle,
                );
            }
            return Some(result);
        }
        if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear);
            return Some(result);
        }
        if lining(a) && state.bay <= 0. {
            return None;
        }
        Some(result)
    }
}
/// The B-52 has no afterburner and no flame word.
pub(crate) fn flame(_address: usize) -> bool {
    false
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
    /// One test for the whole rig, on synthetic faces named in shape units: the
    /// stabilizer, flaps, rudder and gear turn rigidly about their hinges in the
    /// right sense and within their limits, the flap seam is what the doc says,
    /// every truck stows inside the fuselage section and the bay plates drop.
    #[test]
    fn the_b52_rig_turns_whole_panels_about_their_hinges() {
        let rig = Rig {
            bay: tore_sim::combat::bays::centre_bay(ID).unwrap(),
        };
        let moved = |s: &State, f: &Face| rig.animate(f, s).unwrap();
        let s0 = state();

        // Stabilizer: a pull raises the trailing tip by up to 9 degrees, a push
        // lowers it by up to 4; the root midpoint stays; the quad is rigid.
        let stab = face(
            0x334e,
            &[
                [42., -123., 5.],
                [42., -115., 5.],
                [6., -81., 5.],
                [3., -120., 5.],
            ],
        );
        let mut s = s0.clone();
        s.elevator = 1.;
        let up = moved(&s, &stab);
        assert!(spread(&stab.positions, &up.positions) < 1e-3);
        let rise = up.positions[0][2] - stab.positions[0][2];
        let lead = scale_point(ID, [6., -81., 5.]);
        assert!((rise - 22.5 * STAB_UP.sin() as f32).abs() < 0.02 && rise > 0.);
        assert!(
            up.positions[2][2] < lead[2],
            "the leading root corner drops"
        );
        s.elevator = -1.;
        let down = moved(&s, &stab);
        assert!(
            (down.positions[0][2] - stab.positions[0][2] + 22.5 * STAB_DOWN.sin() as f32).abs()
                < 0.02
        );
        s.elevator = 5.;
        assert_eq!(moved(&s, &stab).positions, up.positions, "clamped");
        s.elevator = 0.;
        assert_eq!(moved(&s, &stab).positions, stab.positions);
        // Roll is not drawn: the aileron moves nothing.
        s.aileron = 1.;
        assert_eq!(moved(&s, &stab).positions, stab.positions);

        // Flaps: the wedge turns whole, the leading midpoints stay, a positive
        // flap lowers the aft corners, and the seam at the trailing corner is
        // about 1.4 ft at the 15 degree limit.
        let wedge = [
            face(
                0x4e54,
                &[
                    [53., -14., 5.],
                    [15., 7., 6.],
                    [9., 1., 9.],
                    [47., -20., 7.],
                ],
            ),
            face(
                0x4e73,
                &[
                    [47., -20., 7.],
                    [9., 1., 9.],
                    [15., 7., 10.],
                    [53., -14., 8.],
                ],
            ),
        ];
        let mut s = s0.clone();
        s.flaps = 1.;
        let turned: Vec<Face> = wedge.iter().map(|f| moved(&s, f)).collect();
        let before: Vec<_> = wedge.iter().flat_map(|f| f.positions.clone()).collect();
        let after: Vec<_> = turned.iter().flat_map(|f| f.positions.clone()).collect();
        assert!(spread(&before, &after) < 1e-3, "wedge not rigid");
        let mid =
            |p: [f32; 3], q: [f32; 3]| -> [f32; 3] { std::array::from_fn(|k| (p[k] + q[k]) / 2.) };
        let hinge = mid(turned[0].positions[1], turned[1].positions[2]);
        assert!(distance(hinge, scale_point(ID, [15., 7., 8.])) < 1e-3);
        assert!(turned[0].positions[2][2] < wedge[0].positions[2][2]);
        let seam_ft = distance(turned[0].positions[2], wedge[0].positions[2]) * 2. / 3.;
        assert!((1.3..1.5).contains(&seam_ft), "seam {seam_ft} ft");
        s.flaps = 0.;
        assert_eq!(moved(&s, &wedge[0]).positions, wedge[0].positions);
        // The left wedge lowers its aft corner too.
        let left = face(
            0x4f3b,
            &[
                [-53., -14., 5.],
                [-48., -20., 7.],
                [-9., 1., 9.],
                [-15., 7., 6.],
            ],
        );
        s.flaps = 1.;
        assert!(moved(&s, &left).positions[1][2] < left.positions[1][2] - 0.1);

        // Rudder: the hinge vertices stay, the top rear vertex goes to +X for a
        // positive command and -X for a negative one, within the travel.
        let rudder = face(
            0x50f0,
            &[
                [0., -122., 42.],
                [0., -114., 42.],
                [0., -101., 11.],
                [0., -117., 11.],
            ],
        );
        let mut s = s0.clone();
        s.rudder = 1.;
        let right = moved(&s, &rudder);
        assert!(spread(&rudder.positions, &right.positions) < 1e-3);
        for k in [1, 2] {
            assert!(distance(right.positions[k], rudder.positions[k]) < 1e-3);
        }
        let x = right.positions[0][0];
        assert!((3.5..4.5).contains(&x), "top rear vertex x {x}");
        s.rudder = -1.;
        assert!((moved(&s, &rudder).positions[0][0] + x).abs() < 1e-3);
        s.rudder = 0.;
        assert_eq!(moved(&s, &rudder).positions, rudder.positions);

        // Gear: hidden at 0, the source pose at 1, whole pieces between, never
        // below the gear-down pose, and at near stow each truck is inside the
        // fuselage section (side x 8 from z -6 to 5, narrowing above).
        let truck = [
            face(
                0x49e7,
                &[
                    [5., 39., -14.],
                    [5., 39., -9.],
                    [8., 39., -6.],
                    [13., 39., -6.],
                    [13., 39., -14.],
                ],
            ),
            face(
                0x4b3d,
                &[
                    [12., 34., -14.],
                    [12., 45., -14.],
                    [12., 45., -9.],
                    [12., 34., -9.],
                ],
            ),
            face(
                0x4c35,
                &[
                    [10., 45., -9.],
                    [7., 45., -7.],
                    [7., 34., -7.],
                    [10., 34., -9.],
                ],
            ),
        ];
        let outrigger = [
            face(
                0x4779,
                &[
                    [111., -32., -14.],
                    [111., -32., 3.],
                    [111., -38., 3.],
                    [111., -38., -14.],
                ],
            ),
            face(
                0x47b7,
                &[
                    [113., -35., -14.],
                    [113., -35., 3.],
                    [108., -35., 3.],
                    [108., -35., -14.],
                ],
            ),
        ];
        let mut s = s0.clone();
        s.gear = 0.;
        assert!(rig.animate(&truck[0], &s).is_none());
        assert!(rig.animate(&outrigger[0], &s).is_none());
        s.gear = 1.;
        assert_eq!(moved(&s, &truck[0]).positions, truck[0].positions);
        let low = scale_point(ID, [0., 0., -14.])[2];
        for piece in truck.iter().chain(&outrigger) {
            for gear in [0.9, 0.75, 0.5, 0.25, 0.1, 1e-6] {
                s.gear = gear;
                let out = moved(&s, piece);
                assert!(spread(&piece.positions, &out.positions) < 1e-3);
                // The outrigger's cross plate is 2 units wider than its hinge, so it dips 0.11
                // unit (0.07 ft) below the contact line near the start of the turn.
                assert!(out.positions.iter().all(|p| p[2] >= low - 0.15));
            }
        }
        s.gear = 1e-6;
        // The section above the side wall narrows 0.5 unit a unit up to the roof.
        let inside =
            |p: [f32; 3]| p[2] > -6. && p[2] < 9. && p[0].abs() < 8. - (p[2] - 5.).max(0.) * 0.5;
        for piece in &truck {
            assert!(moved(&s, piece).positions.iter().all(|&p| inside(p)));
            // The left truck (mirror) stows the same way.
            let mirror = Face {
                address: 0x4a2d,
                positions: piece
                    .positions
                    .iter()
                    .map(|p| [-p[0], p[1], p[2]])
                    .collect(),
                ..piece.clone()
            };
            assert!(moved(&s, &mirror).positions.iter().all(|&p| inside(p)));
        }
        // The outrigger's side plate lies flat between the wing skins at z 4.1,
        // inboard of its own x, in the stowed pose.
        let plate = moved(&s, &outrigger[0]);
        assert!(
            plate
                .positions
                .iter()
                .all(|p| (p[2] - 4.1).abs() < 1e-2 && p[0] < 111.01)
        );

        // The bay cuts the belly skin into two door leaves while it is off its
        // seat and draws its lining then; shut, the belly is whole and the
        // lining is gone.
        let belly = face(
            BELLY,
            &[
                [-5., -46., -9.],
                [5., -46., -9.],
                [5., 88., -9.],
                [-5., 88., -9.],
            ],
        );
        let ceiling = face(
            LINING_BASE,
            &[[-4.5, -13., 0.], [4.5, -13., 0.], [4.5, 26., 0.]],
        );
        let mut s = s0;
        s.bay = 0.;
        assert_eq!(rig.faces(&belly, &s).len(), 1);
        assert!(rig.faces(&ceiling, &s).is_empty() && rig.animate(&ceiling, &s).is_none());
        s.bay = 1.;
        let open = rig.faces(&belly, &s);
        let leaves: Vec<_> = open
            .iter()
            .filter(|f| f.positions.iter().any(|p| p[2] < -9. - 3.))
            .collect();
        assert_eq!(leaves.len(), 2, "two doors hang from the belly");
        // Each leaf is 3 ft (4.5 units) wide, so it hangs 4.5 units below the
        // belly plane, on x +/-4.5 units.
        for leaf in leaves {
            let low = leaf.positions.iter().map(|p| p[2]).fold(f32::MAX, f32::min);
            assert!((low - (-9. - 4.5)).abs() < 1e-3, "{low}");
            assert!(
                leaf.positions
                    .iter()
                    .all(|p| (p[0].abs() - 4.5).abs() < 1e-3)
            );
        }
        assert_eq!(rig.faces(&ceiling, &s).len(), 1);
        // Half open the leaves hang on a quarter turn's sine.
        s.bay = 0.5;
        let half = rig.faces(&belly, &s);
        let low = half
            .iter()
            .flat_map(|f| f.positions.iter())
            .map(|p| p[2])
            .fold(f32::MAX, f32::min);
        assert!(
            (low - (-9. - 4.5 * (FRAC_PI_2 as f32 * 0.5).sin())).abs() < 1e-3,
            "{low}"
        );
    }
}
