//! B-2 Spirit (`B2.SH`) own-source rig (B2-3 and B2-4): the two elevon wedges, the
//! split drag rudders (yaw and speed brake), the seven gear plates, and the
//! interim bay plates.
//! See docs/formats/aircraft-variety.md "B-2 rig (B2-3, 2026-11-15)".
//!
//! `B2.SH` ships three state words: 0x6486 the bay (four hanging plates, two
//! windings each), 0x648c the gear (seven plates, two windings each) and 0x6498,
//! a signed drag-rudder word (+1 swaps the right closed pair for six faces, -1
//! the left; any value of magnitude 2 or more removes a pair and adds nothing,
//! so the rig never sets a magnitude above 1). The elevons are one four-face
//! wedge per side with no state word. The rig never sets the rudder word: it
//! keeps the closed drag-rudder quads drawn and turns each about its own hinge
//! edge, so the wing never shows a hole, and it adds the retail front plate
//! and inboard cap of the open pose only while a side is open.
//!
//! Every number below is a shape unit (1/3 ft) and is passed through
//! [`scale_point`], so a panel turned about a hinge in the scaled shape stays
//! rigid (the B-2 scale is the identity, kept for safety).
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};

const ID: AircraftId = AircraftId::B2;
const WORDS: [usize; 3] = [0x6486, 0x648c, 0x6498];

/// Elevon wedge per side: a top quad, a bottom quad and an inboard triangle
/// pair, no shared vertex with any other face. The leading edge (hinge side)
/// is thick (z 0 and -6), the trailing edge is a knife edge at z -3.
const ELEVON_R: [usize; 4] = [0x4037, 0x404e, 0x4065, 0x407a];
const ELEVON_L: [usize; 4] = [0x40ce, 0x40e5, 0x40fc, 0x4111];
/// Elevon hinge lines: the leading edge's mid-thickness line (z -3), pointing
/// outboard so a positive angle lowers the trailing edge on both sides.
const ELEVON_HINGE_R: [[f32; 3]; 2] = [[63., -70., -3.], [120., -22., -3.]];
const ELEVON_HINGE_L: [[f32; 3]; 2] = [[-121., -22., -3.], [-64., -70., -3.]];
/// Fitted magnitudes in radians (agent decisions, 2026-11-15): the fleet's
/// own elevon figures (the Mirage 2000, the Rafale and the MiG-31 take 0.30
/// pitch and 0.20 roll). No cited B-2 elevon limit was found.
pub(crate) const PITCH: f64 = 0.30;
pub(crate) const ROLL: f64 = 0.20;

/// One elevon angle in radians, positive lowering the trailing edge. A pull
/// raises both trailing edges; a positive aileron raises the right trailing
/// edge and lowers the left (the sense of the fleet); the sum is clamped to
/// the two maxima (rule 12).
pub(crate) fn elevon_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    (-PITCH * state.elevator.clamp(-1., 1.) - side * ROLL * state.aileron.clamp(-1., 1.))
        .clamp(-(PITCH + ROLL), PITCH + ROLL)
}

/// Drag rudders. Each side has an upper plate hinged on the top skin edge (z 0)
/// and a lower plate hinged on the bottom edge (z -7); closed, the two quads
/// meet at the trailing edge (z -3). `front` and `back` are the retail open
/// pose's two windings of the same plate: the rig keeps the `closed` quad and
/// the `back` winding (a copy of the quad with reversed winding) turned about
/// the hinge, and uses the `front` face only to measure the open angle.
struct PlateSpec {
    right: bool,
    closed: usize,
    front: usize,
    back: usize,
    hinge: [[f32; 3]; 2],
}
const PLATES: [PlateSpec; 4] = [
    PlateSpec {
        right: true,
        closed: 0x465f,
        front: 0x47db,
        back: 0x47f5,
        hinge: [[177., -46., 0.], [215., -71., 0.]],
    },
    PlateSpec {
        right: true,
        closed: 0x4679,
        front: 0x47a7,
        back: 0x47c1,
        hinge: [[177., -46., -7.], [215., -71., -7.]],
    },
    PlateSpec {
        right: false,
        closed: 0x4600,
        front: 0x46d0,
        back: 0x46ea,
        hinge: [[-178., -46., 0.], [-218., -72., 0.]],
    },
    PlateSpec {
        right: false,
        closed: 0x461a,
        front: 0x4704,
        back: 0x471e,
        hinge: [[-178., -46., -7.], [-218., -72., -7.]],
    },
];
/// The open pose's vertical front plate and inboard cap (single faces), drawn
/// only while that side is open, exactly as the source has them.
const WALLS_R: [usize; 2] = [0x480f, 0x4829];
const WALLS_L: [usize; 2] = [0x4738, 0x4752];
/// The open pose's other plate windings, used only to read the open angle.
const OPEN_R: [usize; 6] = [0x47db, 0x47f5, 0x47a7, 0x47c1, 0x480f, 0x4829];
const OPEN_L: [usize; 6] = [0x46d0, 0x46ea, 0x4704, 0x471e, 0x4738, 0x4752];
const CLOSED_R: [usize; 2] = [0x465f, 0x4679];
const CLOSED_L: [usize; 2] = [0x4600, 0x461a];

/// How far one drag rudder is open, 0 to 1. A yaw command opens the side the
/// aircraft yaws toward (positive rudder, right); the speed brake opens both.
/// The two add and are capped at fully open, so a brake of 1 shows both fully
/// open whatever the yaw, and a part-brake still shows the yaw difference.
pub(crate) fn open_fraction(right: bool, state: &State) -> f64 {
    let yaw = state.rudder.clamp(-1., 1.);
    let side = if right { yaw.max(0.) } else { (-yaw).max(0.) };
    (state.brake.clamp(0., 1.) + side).min(1.)
}

/// Gear plates (both windings each). Mains: an outer door leaf and an inner
/// wheel plate per side, in planes x = const; nose: a front leaf (y 71), a rear
/// leaf (y 97) and the strut and wheel plate (x 0, y 88 to 96).
const NOSE_FRONT: [usize; 2] = [0x4153, 0x416a];
const NOSE_REAR: [usize; 2] = [0x41ae, 0x41c5];
const NOSE_STRUT: [usize; 2] = [0x4597, 0x45b6];
const MAIN_DOOR_L: [usize; 2] = [0x438d, 0x43a4];
const MAIN_DOOR_R: [usize; 2] = [0x449e, 0x44b5];
const MAIN_WHEEL_L: [usize; 2] = [0x441b, 0x443a];
const MAIN_WHEEL_R: [usize; 2] = [0x452c, 0x454b];
pub(crate) const GEAR: [usize; 14] = [
    0x4153, 0x416a, 0x41ae, 0x41c5, 0x4597, 0x45b6, 0x438d, 0x43a4, 0x449e, 0x44b5, 0x441b, 0x443a,
    0x452c, 0x454b,
];
/// Hinge axes (fitted, agent decision 2026-11-15; the shape has no gear
/// hinge). A main plate turns inward about a line along Y raised 2 units
/// (door) or 3 units (wheel plate) above its own top edge (z -12), so the
/// stowed plate lies inside the closed well and never coplanar with the
/// belly (z -12). A nose leaf turns aft about the lateral line along its own
/// top edge; the strut turns aft about the lateral line through its forward
/// top corner.
const MAIN_DOOR_HINGE_Z: f32 = -10.;
const MAIN_WHEEL_HINGE_Z: f32 = -9.;
const NOSE_FRONT_HINGE: [f32; 3] = [0., 71., -9.];
const NOSE_REAR_HINGE: [f32; 3] = [0., 97., -5.];
const NOSE_STRUT_HINGE: [f32; 3] = [0., 96., -5.];
/// The nose wheel steers about a vertical axis through the strut plate's centre.
const NOSE_STEER: [f32; 3] = [0., 92., 0.];

/// Bay word 0x6486: four vertical plates hanging from the belly (z -12) to
/// z -25 at x -27, -3, 1 and 26. Interim until the B2-5 doors.
pub(crate) const BAY: [usize; 8] = [
    0x4209, 0x4220, 0x4264, 0x427b, 0x431a, 0x4331, 0x42bf, 0x42d6,
];
const BAY_TOP_Z: f32 = -12.;

/// Whole-piece gear motion. Gear down (1) is the source pose; nothing is
/// drawn at 0 (the caller hides it). The nose strut also steers.
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let angle = FRAC_PI_2 * (1. - gear.clamp(0., 1.));
    let a = f.address;
    let nose = |f: &mut Face, hinge: [f32; 3]| {
        turn(
            f,
            scale_point(ID, hinge),
            scale_point(ID, [1., 0., 0.]),
            -angle,
        );
    };
    let main = |f: &mut Face, x: f32, z: f32, inward: f64| {
        turn(
            f,
            scale_point(ID, [x, 0., z]),
            scale_point(ID, [0., 1., 0.]),
            inward * angle,
        );
    };
    if NOSE_STRUT.contains(&a) {
        if steering != 0. {
            turn(
                f,
                scale_point(ID, NOSE_STEER),
                scale_point(ID, [0., 0., 1.]),
                -steering,
            );
        }
        nose(f, NOSE_STRUT_HINGE);
    } else if NOSE_FRONT.contains(&a) {
        nose(f, NOSE_FRONT_HINGE);
    } else if NOSE_REAR.contains(&a) {
        nose(f, NOSE_REAR_HINGE);
    } else if MAIN_DOOR_R.contains(&a) {
        main(f, 69., MAIN_DOOR_HINGE_Z, 1.);
    } else if MAIN_DOOR_L.contains(&a) {
        main(f, -71., MAIN_DOOR_HINGE_Z, -1.);
    } else if MAIN_WHEEL_R.contains(&a) {
        main(f, 62., MAIN_WHEEL_HINGE_Z, 1.);
    } else if MAIN_WHEEL_L.contains(&a) {
        main(f, -65., MAIN_WHEEL_HINGE_Z, -1.);
    }
}

/// One drag-rudder plate as the rig turns it.
struct Plate {
    right: bool,
    closed: usize,
    back: usize,
    pivot: [f32; 3],
    axis: [f32; 3],
    /// Angle to the retail open pose, radians, right-handed about `axis`.
    open_angle: f64,
    closed_positions: Vec<[f32; 3]>,
    closed_normal: Option<[f32; 3]>,
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

/// The turn about the line `a` to `b` that carries the closed quad's free
/// edge (its vertices off the hinge line) to the open quad's free edge,
/// measured on the free edges' midpoints, radians (right-handed about a to b).
fn open_angle(a: [f32; 3], b: [f32; 3], closed: &Face, open: &Face) -> AppResult<f64> {
    let axis = sub(b, a);
    let length = dot(axis, axis).sqrt();
    let axis = axis.map(|v| v / length);
    let free_mid = |face: &Face| -> AppResult<[f32; 3]> {
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
            return Err("B2.SH drag rudder plate does not have a two-vertex free edge".into());
        }
        Ok(std::array::from_fn(|k| (free[0][k] + free[1][k]) / 2.))
    };
    let perp = |p: [f32; 3]| {
        let v = sub(p, a);
        let along = dot(v, axis);
        std::array::from_fn::<f64, 3, _>(|k| v[k] - along * axis[k])
    };
    let (v, w) = (perp(free_mid(closed)?), perp(free_mid(open)?));
    Ok(dot(axis, cross(v, w)).atan2(dot(v, w)))
}

pub struct Rig {
    plates: Vec<Plate>,
}
impl Plate {
    fn new(spec: &PlateSpec, closed: &Face, open_front: &Face) -> AppResult<Self> {
        let (a, b) = (
            scale_point(ID, spec.hinge[0]),
            scale_point(ID, spec.hinge[1]),
        );
        // Whole quad with exactly its two hinge-edge vertices on the hinge line.
        let on = |p: &[f32; 3]| *p == a || *p == b;
        if closed.positions.len() != 4 || closed.positions.iter().filter(|p| on(p)).count() != 2 {
            return Err(format!("unreviewed B2.SH drag rudder plate {:x}", spec.closed).into());
        }
        Ok(Self {
            right: spec.right,
            closed: spec.closed,
            back: spec.back,
            pivot: a,
            axis: std::array::from_fn(|k| b[k] - a[k]),
            open_angle: open_angle(a, b, closed, open_front)?,
            closed_positions: closed.positions.clone(),
            closed_normal: closed.normal,
        })
    }
    /// The plate at `fraction` open: the closed quad (or its reversed-winding
    /// twin, built on the retail `back` face as a template) turned about the
    /// hinge line, as one rigid panel.
    fn pose(&self, template: &Face, fraction: f64) -> Face {
        let mut face = template.clone();
        if template.address == self.back {
            face.positions = self.closed_positions.iter().rev().copied().collect();
            face.normal = self.closed_normal.map(|n| n.map(|v| -v));
        } else {
            face.positions = self.closed_positions.clone();
            face.normal = self.closed_normal;
        }
        if fraction > 0. {
            turn(&mut face, self.pivot, self.axis, self.open_angle * fraction);
        }
        face
    }
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("B2.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("B2.SH duplicate reviewed face {address:x}").into());
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
        return Err(format!("unreviewed B2.SH branch {word:x} = {value}").into());
    }
    Ok(pose)
}

impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 21668
            || shape.faces.len() != 268
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed B2.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // The elevon wedges: every face whole, and every vertex a wedge vertex
        // (a leading-edge vertex at z 0 or -6, a knife-edge vertex at z -3 or
        // the inboard point), so the hinge line can carry the whole wedge.
        for (ids, hinge) in [(&ELEVON_R, &ELEVON_HINGE_R), (&ELEVON_L, &ELEVON_HINGE_L)] {
            let lo = hinge[0][0].min(hinge[1][0]) - 12.;
            let hi = hinge[0][0].max(hinge[1][0]) + 12.;
            for &address in ids {
                let face = one(&shape, address)?;
                let ok = matches!(face.positions.len(), 3 | 4)
                    && face.positions.iter().all(|p| {
                        let u = [p[0] / axis[0], p[1] / axis[1], p[2] / axis[2]];
                        (lo..=hi).contains(&u[0])
                            && [0., -3., -6.].iter().any(|z| (u[2] - z).abs() < 1e-3)
                    });
                if !ok {
                    return Err(format!("unreviewed B2.SH elevon face {address:x}").into());
                }
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // Gear and bay words add exactly the reviewed plates and remove nothing.
        let gear = branch(bytes, &original, 0x648c, 1, &GEAR, &[])?.scaled(axis);
        let bay = branch(bytes, &original, 0x6486, 1, &BAY, &[])?.scaled(axis);
        // The rudder word: +1 swaps the right closed pair for six faces and -1
        // the left; the rig reads them for the open angle, the back winding
        // template, the front plate and the cap.
        let right = branch(bytes, &original, 0x6498, 1, &OPEN_R, &CLOSED_R)?.scaled(axis);
        let left = branch(bytes, &original, 0x6498, -1, &OPEN_L, &CLOSED_L)?.scaled(axis);
        let mut plates = Vec::new();
        let mut extra: Vec<Face> = Vec::new();
        for spec in &PLATES {
            let pose = if spec.right { &right } else { &left };
            plates.push(Plate::new(
                spec,
                one(&shape, spec.closed)?,
                one(pose, spec.front)?,
            )?);
            extra.push(one(pose, spec.back)?.clone());
        }
        for (pose, walls) in [(&right, WALLS_R), (&left, WALLS_L)] {
            for address in walls {
                extra.push(one(pose, address)?.clone());
            }
        }
        extra.extend(gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)));
        extra.extend(bay.faces.into_iter().filter(|f| BAY.contains(&f.address)));
        shape.faces.extend(extra);
        Ok((Self { plates }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        if let Some(right) = [(&ELEVON_R, true), (&ELEVON_L, false)]
            .iter()
            .find_map(|(ids, right)| ids.contains(&a).then_some(*right))
        {
            let hinge = if right {
                &ELEVON_HINGE_R
            } else {
                &ELEVON_HINGE_L
            };
            let mut result = source.clone();
            // One angle per side turns every vertex of the wedge about the
            // hinge line; no vertex is fixed off the line (rule 11).
            turn(
                &mut result,
                scale_point(ID, hinge[0]),
                scale_point(
                    ID,
                    [hinge[1][0] - hinge[0][0], hinge[1][1] - hinge[0][1], 0.],
                ),
                elevon_angle(right, state),
            );
            return Some(result);
        }
        if let Some(plate) = self.plates.iter().find(|p| p.closed == a || p.back == a) {
            let open = open_fraction(plate.right, state);
            if plate.back == a && open <= 0. {
                return None;
            }
            return Some(plate.pose(source, open));
        }
        if WALLS_R.contains(&a) || WALLS_L.contains(&a) {
            return (open_fraction(WALLS_R.contains(&a), state) > 0.).then(|| source.clone());
        }
        if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            let mut result = source.clone();
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
            return Some(result);
        }
        if BAY.contains(&a) {
            let bay = state.bay.clamp(0., 1.) as f32;
            if bay <= 0. {
                return None;
            }
            // The plates drop out of the belly as the bay opens (interim,
            // fitted): their top edge stays on the belly line.
            let top = scale_point(ID, [0., 0., BAY_TOP_Z])[2];
            let mut result = source.clone();
            for p in &mut result.positions {
                p[2] = top + (p[2] - top) * bay;
            }
            return Some(result);
        }
        Some(source.clone())
    }
}
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
    fn points(faces: &[Face]) -> Vec<[f32; 3]> {
        faces.iter().flat_map(|f| f.positions.clone()).collect()
    }
    /// One test for the whole rig, on synthetic faces named in shape units: the
    /// elevon wedge turns rigidly with pitch and differential roll, a drag
    /// rudder opens on the yaw side and both open with the brake as one rigid
    /// plate about its hinge edge, the gear folds whole and the bay drops.
    #[test]
    fn the_b2_rig_turns_whole_panels_about_their_hinges() {
        // The right elevon wedge, as the retail faces have it.
        let wedge = vec![
            face(
                0x4037,
                &[
                    [69., -80., -3.],
                    [130., -29., -3.],
                    [120., -22., -6.],
                    [63., -70., -6.],
                ],
            ),
            face(
                0x404e,
                &[
                    [130., -29., -3.],
                    [69., -80., -3.],
                    [63., -70., 0.],
                    [120., -22., 0.],
                ],
            ),
            face(
                0x4065,
                &[[63., -70., -6.], [59., -74., -3.], [69., -80., -3.]],
            ),
            face(
                0x407a,
                &[[63., -70., 0.], [69., -80., -3.], [59., -74., -3.]],
            ),
        ];
        let mut s = state();
        let rig = Rig { plates: Vec::new() };
        let trailing = |s: &State, address: usize, edge: [f32; 3]| {
            let out = rig.animate(&face(address, &[edge]), s).unwrap();
            out.positions[0][2] - scale_point(ID, edge)[2]
        };
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
            let turned: Vec<Face> = wedge.iter().map(|f| rig.animate(f, &s).unwrap()).collect();
            assert!(
                spread(&points(&wedge), &points(&turned)) < 1e-3,
                "{elevator}/{aileron}: wedge not rigid"
            );
        }
        // Pitch moves both trailing edges the same way, roll opposite ways.
        let (right_edge, left_edge) = ([130., -29., -3.], [-131., -29., -3.]);
        s.aileron = 0.;
        s.elevator = 1.;
        assert!(trailing(&s, 0x4037, right_edge) > 0.01 && trailing(&s, 0x40ce, left_edge) > 0.01);
        s.elevator = -1.;
        assert!(
            trailing(&s, 0x4037, right_edge) < -0.01 && trailing(&s, 0x40ce, left_edge) < -0.01
        );
        s.elevator = 0.;
        s.aileron = 1.;
        assert!(trailing(&s, 0x4037, right_edge) > 0.01 && trailing(&s, 0x40ce, left_edge) < -0.01);
        // The hinge line stays: the midpoint of a leading-edge pair is fixed.
        s.elevator = 1.;
        s.aileron = 1.;
        let pair = [
            rig.animate(&face(0x404e, &[[63., -70., 0.]]), &s)
                .unwrap()
                .positions[0],
            rig.animate(&face(0x4037, &[[63., -70., -6.]]), &s)
                .unwrap()
                .positions[0],
        ];
        let mid = scale_point(ID, [63., -70., -3.]);
        for k in 0..3 {
            assert!(((pair[0][k] + pair[1][k]) / 2. - mid[k]).abs() < 1e-3);
        }
        assert!((elevon_angle(true, &s) + (PITCH + ROLL)).abs() < 1e-9);
        s.elevator = 5.;
        s.aileron = -5.;
        assert!((elevon_angle(true, &s) - (ROLL - PITCH)).abs() < 1e-9);

        // Drag rudders: the right upper plate and its closed, open and brake poses.
        let closed = face(
            0x465f,
            &[
                [205., -79., -3.],
                [167., -53., -3.],
                [177., -46., 0.],
                [215., -71., 0.],
            ],
        );
        let open = face(
            0x47db,
            &[
                [177., -46., 0.],
                [215., -71., 0.],
                [206., -77., 7.],
                [167., -53., 7.],
            ],
        );
        let spec = &PLATES[0];
        let plate = Plate::new(spec, &closed, &open).unwrap();
        assert!(
            plate.open_angle < -0.7 && plate.open_angle > -0.9,
            "upper plate lifts about 46 degrees"
        );
        let mut s = state();
        s.rudder = 1.;
        assert_eq!(
            (open_fraction(true, &s), open_fraction(false, &s)),
            (1., 0.)
        );
        s.rudder = -0.5;
        assert_eq!(
            (open_fraction(true, &s), open_fraction(false, &s)),
            (0., 0.5)
        );
        s.rudder = 0.;
        s.brake = 1.;
        assert_eq!(
            (open_fraction(true, &s), open_fraction(false, &s)),
            (1., 1.)
        );
        s.brake = 0.5;
        s.rudder = 1.;
        assert_eq!(
            (open_fraction(true, &s), open_fraction(false, &s)),
            (1., 0.5)
        );
        let before = plate.pose(&closed, 0.).positions;
        assert_eq!(before, closed.positions);
        for fraction in [0.25, 0.5, 1.] {
            let posed = plate.pose(&closed, fraction);
            assert!(
                spread(&closed.positions, &posed.positions) < 1e-3,
                "plate not rigid"
            );
            // The two hinge vertices stay on the hinge line.
            for k in [2usize, 3] {
                for axis in 0..3 {
                    assert!((posed.positions[k][axis] - closed.positions[k][axis]).abs() < 1e-3);
                }
            }
            // The free edge rises toward the open pose (z +7 at the end).
            assert!(posed.positions[0][2] > closed.positions[0][2] + 0.1);
        }
        let end = plate.pose(&closed, 1.);
        assert!(
            (end.positions[1][2] - scale_point(ID, [0., 0., 7.])[2]).abs() < 2.,
            "free edge ends near the retail open pose"
        );
        // The back winding is the same plate reversed.
        let back = plate.pose(&face(spec.back, &[[0.; 3]; 4]), 1.);
        let mut reversed = end.positions.clone();
        reversed.reverse();
        assert_eq!(back.positions.len(), 4);
        for (a, b) in back.positions.iter().zip(&reversed) {
            for k in 0..3 {
                assert!((a[k] - b[k]).abs() < 1e-3);
            }
        }

        // Gear: hidden at 0, the source pose at 1, whole pieces between.
        let rig = Rig { plates: Vec::new() };
        let wheel = face(
            0x452c,
            &[
                [62., -2., -33.],
                [62., -2., -12.],
                [62., 20., -12.],
                [62., 20., -33.],
            ],
        );
        let strut = face(
            0x4597,
            &[
                [0., 88., -30.],
                [0., 96., -31.],
                [0., 96., -5.],
                [0., 88., -6.],
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
                // Nothing ever hangs below the gear-down pose.
                let low = piece
                    .positions
                    .iter()
                    .map(|p| p[2])
                    .fold(f32::INFINITY, f32::min);
                assert!(out.positions.iter().all(|p| p[2] >= low - 1e-3));
            }
        }
        // Near stow the wheel plate lies inside the closed well, above the
        // belly (z -12): inboard of its gear-down x and at its hinge height.
        let mut s = state();
        s.gear = 1e-6;
        let out = rig.animate(&wheel, &s).unwrap();
        let hinge_z = scale_point(ID, [0., 0., MAIN_WHEEL_HINGE_Z])[2];
        assert!(
            out.positions
                .iter()
                .all(|p| (p[2] - hinge_z).abs() < 1e-2 && p[0] < wheel.positions[0][0] + 1e-3)
        );
        // The bay plates drop from the belly line as the bay opens.
        let plate = face(
            0x4209,
            &[
                [-27., -10., -12.],
                [-27., -10., -25.],
                [-27., 48., -25.],
                [-27., 48., -12.],
            ],
        );
        let mut s = state();
        s.bay = 0.;
        assert!(rig.animate(&plate, &s).is_none());
        s.bay = 1.;
        assert_eq!(rig.animate(&plate, &s).unwrap().positions, plate.positions);
        s.bay = 0.5;
        let half = rig.animate(&plate, &s).unwrap();
        assert!((half.positions[1][2] - scale_point(ID, [0., 0., -18.5])[2]).abs() < 1e-3);
        assert_eq!(half.positions[0][2], plate.positions[0][2]);
    }
}
