//! F-117A (`F117.SH`) own-source rig: the two whole ruddervator assemblies (10
//! faces a side, mixed pitch and yaw), the rigid gear (three legs, three door
//! plates and the door skins, with a door and leg schedule) and the two fitted
//! weapon bays (F117-5: six belly skins cut into two doors a bay, a lined
//! recess and a trapeze rail, drawn through [`crate::bay_doors`]).
//! See docs/spec/variety-animation.md "F-117 rig fit" and
//! docs/formats/aircraft-variety.md "F-117 rig (F117-3, 2026-11-15)".
//!
//! F117.SH ships two state words: 0x4c56 the weapons bay (replaces the belly
//! triangle 0x34e6 with its other winding 0x351d and adds two wall plates, both
//! windings each; the shape's own open pose is no longer drawn, its wall plate
//! is the material of the fitted lining) and 0x4c5c the gear (adds 31 faces). There is no flame,
//! airbrake, flap, rudder, elevon, ruddervator, canopy or hook word, and no
//! separate elevon faces: the trailing edge is the aft edge of whole wing
//! facets, so the elevons stay unrigged (decision 27, John 2026-11-15). The jet
//! has no afterburner and no airbrake, so no plume or brake is mapped and
//! nothing is invented.
//!
//! The shape arrives already scaled per axis (F117-2). Every hinge point named
//! here is in shape units and is scaled with `scale_point`, so a panel turned
//! about it stays rigid.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};

const ID: AircraftId = AircraftId::F117;
const WORDS: [usize; 2] = [0x4c56, 0x4c5c];

/// Right and left ruddervator assemblies: 10 faces each, no vertex shared with
/// any other face of the shape (F117-0).
pub(crate) const TAIL_R: [usize; 10] = [
    0x22c5, 0x22d9, 0x231a, 0x232e, 0x2383, 0x244b, 0x2460, 0x2475, 0x24b3, 0x2e40,
];
pub(crate) const TAIL_L: [usize; 10] = [
    0x23ab, 0x23bd, 0x23d0, 0x23f7, 0x240b, 0x2425, 0x2489, 0x249e, 0x24f6, 0x255e,
];
/// Root chord midpoint and the cant axis of each assembly, shape units. The
/// axis lies in the lateral plane (no fore-aft part): from the root chord
/// midpoint to the tip chord midpoint, so a turn is a pitch of the whole panel
/// tilted by its own cant and never about a swept span axis (rule 11).
const TAIL_R_HINGE: [f32; 3] = [2., -67., 1.];
const TAIL_R_AXIS: [f32; 3] = [21.5, 0., 20.];
const TAIL_L_HINGE: [f32; 3] = [-2.5, -67., 1.];
const TAIL_L_AXIS: [f32; 3] = [-21., 0., 20.];
/// Every vertex of each assembly (shape units).
const TAIL_R_POINTS: [[f32; 3]; 10] = [
    [10., -66., 10.],
    [23., -84., 21.],
    [23., -88., 21.],
    [15., -79., 13.],
    [3., -79., -1.],
    [7., -69., 5.],
    [24., -96., 21.],
    [1., -68., 0.],
    [1., -55., 3.],
    [3., -68., 0.],
];
const TAIL_L_POINTS: [[f32; 3]; 10] = [
    [-9., -65., 9.],
    [-23., -84., 21.],
    [-23., -88., 21.],
    [-24., -96., 21.],
    [-23., -88., 20.],
    [-7., -69., 5.],
    [-3., -79., -1.],
    [-1., -68., 0.],
    [-2., -55., 3.],
    [-3., -68., 0.],
];

/// Fitted magnitudes in radians, all agent decisions (2026-11-15): the real
/// ruddervator limit was not found in a public source. Each panel's angle is a
/// collective pitch term plus a differential yaw term, clamped to the limit.
pub(crate) const PITCH: f64 = 0.12;
pub(crate) const YAW: f64 = 0.08;
pub(crate) const LIMIT: f64 = 0.15;

/// Gear word 0x4c5c: three legs (strut and wheel plates), three door plates
/// (the open doors of the gear-down pose) and the z -6 door skins.
pub(crate) const LEG_R: [usize; 6] = [0x3286, 0x32a1, 0x32bc, 0x32d7, 0x32f2, 0x3305];
pub(crate) const LEG_L: [usize; 6] = [0x33d8, 0x33f3, 0x340e, 0x3429, 0x3444, 0x3457];
pub(crate) const LEG_NOSE: [usize; 6] = [0x3765, 0x3784, 0x37e8, 0x3807, 0x3826, 0x3845];
pub(crate) const DOOR_R: [usize; 2] = [0x3351, 0x3366];
pub(crate) const DOOR_L: [usize; 2] = [0x34a3, 0x34b8];
pub(crate) const DOOR_NOSE: [usize; 2] = [0x3897, 0x38b0];
pub(crate) const SKIN: [usize; 7] = [0x3676, 0x36fa, 0x36a1, 0x36c0, 0x36df, 0x3719, 0x3657];
pub(crate) const GEAR: [usize; 31] = [
    0x3286, 0x32a1, 0x32bc, 0x32d7, 0x32f2, 0x3305, 0x33d8, 0x33f3, 0x340e, 0x3429, 0x3444, 0x3457,
    0x3765, 0x3784, 0x37e8, 0x3807, 0x3826, 0x3845, 0x3351, 0x3366, 0x34a3, 0x34b8, 0x3897, 0x38b0,
    0x3676, 0x36fa, 0x36a1, 0x36c0, 0x36df, 0x3719, 0x3657,
];
/// Hinge lines (shape units; a Y axis at the given x and z for the mains and
/// doors, a lateral axis at the given y and z for the nose): the top edge of
/// each piece at z -4, on the outboard edge of each main leg so the stowed leg
/// rises into the wing.
const LEG_R_X: f32 = 30.;
const LEG_L_X: f32 = -31.;
const DOOR_R_X: f32 = 23.;
const DOOR_L_X: f32 = -25.;
const DOOR_NOSE_X: f32 = 3.;
const NOSE_Y: f32 = 36.;
/// The nose leg turns about a lateral line on the belly plane (z -5), not its
/// top edge (z -4): a stowed strut at the top edge would stand 0.5 ft out of the
/// cockpit roof, at the belly plane 0.2 ft (measured by the probe).
const NOSE_Z: f32 = -5.;
const HINGE_Z: f32 = -4.;
/// Vertical steering axis through the nose wheel plates.
const NOSE_STEER: [f32; 3] = [0., 40., 0.];
/// The door skins sit 1 unit under the belly in the source pose; closed, they
/// lie 1.5 units higher, inside the skin.
const SKIN_LIFT: f32 = 1.5;
/// Door timing (fitted): the doors stay open while the legs swing, so the legs
/// are stowed by gear 0.30 and the doors close from 0.30 to 0.
pub(crate) const DOOR_TIME: f64 = 0.30;

/// Bay word 0x4c56 (the shape's own open pose, no longer drawn): two wall
/// plates (x -3 and x +1, both windings) and the belly triangle's other
/// winding. They are only reviewed here; one wall plate is the material of the
/// fitted lining.
pub(crate) const BAY_WALL_L: [usize; 2] = [0x319c, 0x31b1];
pub(crate) const BAY_WALL_R: [usize; 2] = [0x31ff, 0x3214];
pub(crate) const BAY_BELLY: usize = 0x34e6;
pub(crate) const BAY_BELLY_OPEN: usize = 0x351d;
/// The six belly skins (all at z -5 units, normal down) the two fitted bay
/// openings are cut from: the main triangle, the right outer triangle, panel
/// and leading-edge strip and the left outer panel and strip (measured,
/// F117-5: together they cover both openings, and nothing else overlaps them).
pub(crate) const BELLY: [usize; 6] = [BAY_BELLY, 0x2f0a, 0x2f1d, 0x2f5d, 0x2f8e, 0x3076];
/// The fitted lining faces get addresses from here (no face of the shape is
/// near it).
pub(crate) const LINING_BASE: usize = 0x7000;

/// Whether `address` is one of the fitted lining faces.
pub(crate) fn lining(address: usize) -> bool {
    address >= LINING_BASE
}

pub(crate) fn tail_right(address: usize) -> bool {
    TAIL_R.contains(&address)
}
pub(crate) fn tail_left(address: usize) -> bool {
    TAIL_L.contains(&address)
}

/// How far one ruddervator's trailing edge is raised, in radians (negative
/// lowers it): collective for pitch (a pull raises both), differential for yaw
/// (a right pedal lowers the right panel's edge and raises the left's, which
/// swings both trailing edges toward +X), the sum clamped to the limit.
pub(crate) fn raise(right: bool, state: &State) -> f64 {
    let pitch = PITCH * state.elevator.clamp(-1., 1.);
    let yaw = YAW * state.rudder.clamp(-1., 1.);
    let yaw = if right { -yaw } else { yaw };
    (pitch + yaw).clamp(-LIMIT, LIMIT)
}

/// The turn about the outward cant axis that raises a trailing edge by `up`:
/// the right-hand turn about the outward axis lowers the right panel's edge and
/// raises the left's.
fn turn_angle(right: bool, up: f64) -> f64 {
    if right { -up } else { up }
}

fn leg_angle(gear: f64) -> f64 {
    FRAC_PI_2 * ((1. - gear.clamp(0., 1.)) / (1. - DOOR_TIME)).min(1.)
}
fn door_closed(gear: f64) -> f64 {
    ((DOOR_TIME - gear.clamp(0., 1.)) / DOOR_TIME).clamp(0., 1.)
}

/// Whole-piece gear motion. A leg turns rigidly about the top edge of its own
/// outboard side (the mains swing inward, the nose swings forward) to lie flat
/// inside the airframe; a door turns about its own top edge; the door skins
/// lift into the skin. The nose leg steers first about its vertical axis.
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let a = f.address;
    let leg = leg_angle(gear);
    let door = FRAC_PI_2 * door_closed(gear);
    let at = |x: f32, y: f32| scale_point(ID, [x, y, HINGE_Z]);
    let along_y = scale_point(ID, [0., 1., 0.]);
    if LEG_R.contains(&a) {
        turn(f, at(LEG_R_X, 0.), along_y, leg);
    } else if LEG_L.contains(&a) {
        turn(f, at(LEG_L_X, 0.), along_y, -leg);
    } else if LEG_NOSE.contains(&a) {
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
            scale_point(ID, [0., NOSE_Y, NOSE_Z]),
            scale_point(ID, [1., 0., 0.]),
            leg,
        );
    } else if DOOR_R.contains(&a) {
        turn(f, at(DOOR_R_X, 0.), along_y, door);
    } else if DOOR_L.contains(&a) {
        turn(f, at(DOOR_L_X, 0.), along_y, -door);
    } else if DOOR_NOSE.contains(&a) {
        turn(f, at(DOOR_NOSE_X, 0.), along_y, door);
    } else if SKIN.contains(&a) {
        let lift = scale_point(ID, [0., 0., SKIN_LIFT])[2] * door_closed(gear) as f32;
        for p in &mut f.positions {
            p[2] += lift;
        }
    }
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("F117.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("F117.SH duplicate reviewed face {address:x}").into());
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
        return Err(format!("unreviewed F117.SH branch {word:x}").into());
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
    if lo != scale_point(ID, min) || hi != scale_point(ID, max) {
        return Err(format!("unreviewed F117.SH piece {:x}", ids[0]).into());
    }
    Ok(())
}

pub struct Rig {
    opening: tore_sim::combat::bays::Opening,
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 15470
            || shape.faces.len() != 207
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F117.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // Each ruddervator assembly is exactly its reviewed vertex set, so the
        // hinge points above are the assembly's own root and tip midpoints.
        for (ids, points) in [(&TAIL_R, &TAIL_R_POINTS), (&TAIL_L, &TAIL_L_POINTS)] {
            for &address in ids {
                let face = one(&shape, address)?;
                if !face
                    .positions
                    .iter()
                    .all(|&p| points.iter().any(|&q| p == scale_point(ID, q)))
                {
                    return Err(format!("unreviewed F117.SH ruddervator face {address:x}").into());
                }
            }
        }
        // The belly triangle the bay word swaps.
        one(&shape, BAY_BELLY)?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The gear word adds exactly the 31 reviewed plates and removes nothing;
        // the bay word swaps the belly triangle for its other winding and adds
        // the four wall faces.
        let gear = branch(bytes, &original, 0x4c5c, &GEAR, &[])?.scaled(axis);
        let bay_added = [
            BAY_WALL_L[0],
            BAY_WALL_L[1],
            BAY_WALL_R[0],
            BAY_WALL_R[1],
            BAY_BELLY_OPEN,
        ];
        let bay = branch(bytes, &original, 0x4c56, &bay_added, &[BAY_BELLY])?.scaled(axis);
        // Every hinge is the top edge of its piece (z -4) or a reviewed lateral
        // line; the extents pin the pieces the hinges were chosen for.
        for (ids, min, max) in [
            (&LEG_R[..], [26., -20., -21.], [30., -8., -4.]),
            (&LEG_L[..], [-31., -20., -21.], [-27., -8., -4.]),
            (&LEG_NOSE[..], [-2., 36., -20.], [1., 53., -1.]),
            (&DOOR_R[..], [23., -9., -11.], [23., 8., -4.]),
            (&DOOR_L[..], [-25., -9., -11.], [-25., 8., -4.]),
            (&DOOR_NOSE[..], [3., 39., -9.], [3., 62., -4.]),
        ] {
            extent(&gear, ids, min, max)?;
        }
        for &address in &SKIN {
            let face = one(&gear, address)?;
            if !face
                .positions
                .iter()
                .all(|p| p[2] == scale_point(ID, [0., 0., -6.])[2])
            {
                return Err(format!("unreviewed F117.SH door skin {address:x}").into());
            }
        }
        extent(&bay, &BAY_WALL_L, [-3., -27., -12.], [-3., 22., -4.])?;
        extent(&bay, &BAY_WALL_R, [1., -27., -12.], [1., 22., -4.])?;
        // The six belly skins the openings are cut from lie on the belly plane
        // (z -5 units) and face down; the fitted opening's belly height is that
        // plane.
        let opening = tore_sim::combat::bays::opening(ID).ok_or("F117 bay opening missing")?;
        for &address in &BELLY {
            let face = one(&shape, address)?;
            if !face.normal.is_some_and(|n| n[1] < -32000.)
                || !face.positions.iter().all(|p| {
                    (f64::from(p[2]) - f64::from(crate::bay_doors::units(opening.belly))).abs()
                        < 1e-3
                })
            {
                return Err(format!("unreviewed F117.SH belly skin {address:x}").into());
            }
        }
        // The recess, drawn only while the doors are open, takes the material
        // of the shape's own bay wall.
        let template = one(&bay, BAY_WALL_L[1])?.clone();
        let lining: Vec<Face> = crate::bay_doors::lining(&template, &opening)
            .into_iter()
            .enumerate()
            .map(|(k, mut f)| {
                f.address = LINING_BASE + k;
                f
            })
            .collect();
        let drawn: Vec<Face> = gear
            .faces
            .into_iter()
            .filter(|f| GEAR.contains(&f.address))
            .chain(lining)
            .collect();
        shape.faces.extend(drawn);
        Ok((Self { opening }, shape))
    }
    /// The faces a source face becomes: with the bay doors off their seat the
    /// six belly skins are cut into the two doors of each bay, turned down
    /// about their hinges by `state.bay` (the doors' 1 second travel is the
    /// flight's); the lining draws only then.
    pub fn faces(&self, source: &Face, state: &State) -> Vec<Face> {
        let a = source.address;
        if BELLY.contains(&a) && state.bay > 0. {
            return crate::bay_doors::belly_doors(source, state.bay, &self.opening);
        }
        if lining(a) && state.bay <= 0. {
            return Vec::new();
        }
        vec![source.clone()]
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if tail_right(a) || tail_left(a) {
            // One angle per side turns every vertex of the assembly about the
            // cant axis through its root chord midpoint.
            let right = tail_right(a);
            let (hinge, axis) = if right {
                (TAIL_R_HINGE, TAIL_R_AXIS)
            } else {
                (TAIL_L_HINGE, TAIL_L_AXIS)
            };
            turn(
                &mut result,
                scale_point(ID, hinge),
                scale_point(ID, axis),
                turn_angle(right, raise(right, state)),
            );
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
        } else if lining(a) && state.bay <= 0. {
            return None;
        }
        Some(result)
    }
}
/// The F-117 has no afterburner and the shape has no plume word.
pub(crate) fn flame(_address: usize) -> bool {
    false
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
    #[test]
    fn the_v_tail_mixes_pitch_and_yaw_and_every_piece_turns_whole() {
        let rig = Rig {
            opening: tore_sim::combat::bays::opening(ID).unwrap(),
        };
        let mut s = state();
        // The right and left trailing-edge root corners (shape units).
        let right = face(0x22d9, &[[3., -79., -1.], [7., -69., 5.], [24., -96., 21.]]);
        let left = face(
            0x23f7,
            &[[-3., -79., -1.], [-7., -69., 5.], [-24., -96., 21.]],
        );
        let edge = |f: &Face| f.positions[0];
        let go = |rig: &Rig, s: &State, f: &Face| rig.animate(f, s).unwrap();
        // Pull: both trailing edges rise and move toward the centreline.
        s.elevator = 1.;
        let (r, l) = (go(&rig, &s, &right), go(&rig, &s, &left));
        assert!(edge(&r)[2] > edge(&right)[2] && edge(&l)[2] > edge(&left)[2]);
        assert!(edge(&r)[0] < edge(&right)[0] && edge(&l)[0] > edge(&left)[0]);
        // Collective: the two edges rise by about the same amount (the source is
        // not an exact mirror, so within a fifth).
        let (rise_r, rise_l) = (edge(&r)[2] - edge(&right)[2], edge(&l)[2] - edge(&left)[2]);
        assert!(rise_r > 0. && (rise_r - rise_l).abs() < 0.2 * rise_r);
        // Push lowers them.
        s.elevator = -1.;
        assert!(edge(&go(&rig, &s, &right))[2] < edge(&right)[2]);
        // A right pedal swings both trailing edges toward +X, one down and one up.
        s.elevator = 0.;
        s.rudder = 1.;
        let (r, l) = (go(&rig, &s, &right), go(&rig, &s, &left));
        assert!(edge(&r)[0] > edge(&right)[0] && edge(&l)[0] > edge(&left)[0]);
        assert!(edge(&r)[2] < edge(&right)[2] && edge(&l)[2] > edge(&left)[2]);
        // The limit holds, and every face of a panel stays rigid.
        s.elevator = 1.;
        for (rudder, side) in [(1., &right), (-1., &right), (1., &left), (-1., &left)] {
            s.rudder = rudder;
            let turned = go(&rig, &s, side);
            assert!(spread(&side.positions, &turned.positions) < 1e-3);
            assert!(
                (0..3).all(|k| (turned.positions[0][k] - side.positions[0][k]).abs() < 12.)
                    && raise(true, &s).abs() <= LIMIT
                    && raise(false, &s).abs() <= LIMIT
            );
        }
        // The gear is hidden at 0, whole and at its source corners at 1, and the
        // door skins sit at the source height while the doors are open.
        s.elevator = 0.;
        s.rudder = 0.;
        let leg = face(
            0x3286,
            &[[28., -8., -4.], [28., -19., -4.], [28., -19., -21.]],
        );
        let skin = face(0x3676, &[[32., 5., -6.], [27., 8., -6.], [24., 5., -6.]]);
        s.gear = 0.;
        assert!(rig.animate(&leg, &s).is_none());
        s.gear = 1.;
        let same = |a: &Face, b: &Face| {
            spread(&a.positions, &b.positions) < 1e-4
                && a.positions
                    .iter()
                    .zip(&b.positions)
                    .all(|(p, q)| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-4))
        };
        assert!(same(&rig.animate(&leg, &s).unwrap(), &leg));
        assert!(same(&rig.animate(&skin, &s).unwrap(), &skin));
        s.gear = 0.05;
        assert!(rig.animate(&skin, &s).unwrap().positions[0][2] > skin.positions[0][2]);
        // The bay cuts the six belly skins into door leaves while it is off its
        // seat and draws its lining then; shut, the belly is whole and the
        // lining is gone.
        let belly = face(
            BAY_BELLY,
            &[[-39., -37., -5.], [39., -37., -5.], [0., 55., -5.]],
        );
        let ceiling = face(
            LINING_BASE,
            &[[1., -27., 4.], [17., -27., 4.], [17., 21., 4.]],
        );
        s.bay = 0.;
        assert_eq!(rig.faces(&belly, &s).len(), 1);
        assert!(rig.faces(&ceiling, &s).is_empty() && rig.animate(&ceiling, &s).is_none());
        s.bay = 1.;
        let open = rig.faces(&belly, &s);
        assert!(open.len() > 1);
        assert!(open.iter().any(|f| {
            f.positions
                .iter()
                .any(|p| p[2] < belly.positions[0][2] - 7.)
        }));
        assert_eq!(rig.faces(&ceiling, &s).len(), 1);
    }
}
