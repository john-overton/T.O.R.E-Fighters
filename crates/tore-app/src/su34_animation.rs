//! Su-34 rig (SU34-3): canards, tailerons, outer ailerons, flap morph, airbrake,
//! gear and plume on `SU34.SH`. The shape carries the Su-35's five state words
//! at the same offsets (`67c0` plume, `67c6` airbrake, `67cc` gear, `67d8` and
//! `67de` flaps) but its own faces, so the face tables and pivots are new; the
//! reviewed-face helpers, the control laws and the flap, flame and brake
//! motions are the Su-35's ([`crate::su35_animation`]).
//!
//! Every surface is a whole rigid panel (rule 11): the canard plate and the
//! four taileron faces turn as one panel about a lateral spindle through the
//! root midpoint, the aileron turns about its own leading hinge, the airbrake
//! about its forward edge. The fins have no rudder face and no state word, so
//! the pedals move nothing (the F-15E's precedent, decision of 2026-11-15).
use crate::{
    AppResult,
    additional_animation::turn,
    aircraft_animation::update_normal,
    flight::State,
    su35_animation::{
        Morph, aileron_angle, brake_turn, branch_in, canard_angle, flame_scale, one_in, roots_in,
        tail_angle,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    f64::consts::FRAC_PI_2,
};
use tore_formats::shape::{Face, Shape};

const NAME: &str = "SU34.SH";
/// The five state words, the same offsets from the base as the Su-35's.
const WORDS: [usize; 5] = [0x67c0, 0x67c6, 0x67cc, 0x67d8, 0x67de];
const CANARD_L: [usize; 2] = [0x34e6, 0x3597];
const CANARD_R: [usize; 2] = [0x47cc, 0x480d];
/// A taileron is a quad and a root triangle a side, each in both windings
/// (the Su-35's single pentagon, split in two here).
const TAIL_L: [usize; 4] = [0x3666, 0x3727, 0x360b, 0x3646];
const TAIL_R: [usize; 4] = [0x4837, 0x4906, 0x4714, 0x4777];
/// The outer trailing quads, both windings a side.
const AILERON_L: [usize; 2] = [0x36d6, 0x375f];
const AILERON_R: [usize; 2] = [0x4886, 0x4988];
const FIN: [usize; 8] = [
    0x37cb, 0x37ee, 0x38b9, 0x38dc, 0x49af, 0x49d6, 0x49fd, 0x4a24,
];
const BRAKE: [usize; 2] = [0x4dc9, 0x4de0];
const FLAME: [usize; 16] = [
    0x4e8e, 0x4ead, 0x4ecc, 0x4eeb, 0x4f0a, 0x4f29, 0x4f48, 0x4f67, 0x4f86, 0x4fa5, 0x4fc4, 0x4fe3,
    0x5002, 0x5021, 0x5040, 0x505f,
];
/// Gear: right main, left main, the nose plate and the plate across it, each
/// in both windings.
const GEAR_R: [usize; 2] = [0x4a7b, 0x4a9a];
const GEAR_L: [usize; 2] = [0x4ae6, 0x4b05];
const GEAR_NOSE: [usize; 4] = [0x4b69, 0x4b88, 0x4ba7, 0x4bce];
pub(crate) const GEAR: [usize; 8] = [
    0x4a7b, 0x4a9a, 0x4ae6, 0x4b05, 0x4b69, 0x4b88, 0x4ba7, 0x4bce,
];
/// The neutral flap quads (both windings a side) and the faces the word's
/// down pose draws in their place.
const FLAP_L: [(usize, usize); 2] = [(0x4cbe, 0x4d1b), (0x4cd5, 0x4d32)];
const FLAP_R: [(usize, usize); 2] = [(0x4c14, 0x4c71), (0x4c3b, 0x4c88)];

/// The tailerons' lateral spindles: the midpoint of each root edge.
const TAIL_PIVOT_R: [f32; 3] = [23.5, -49., -3.5];
const TAIL_PIVOT_L: [f32; 3] = [-23., -49., -3.5];
/// The canards' lateral spindles: the midpoint of each root chord.
const CANARD_PIVOT_R: [f32; 3] = [20., 23.5, -3.];
const CANARD_PIVOT_L: [f32; 3] = [-21., 23., -3.];
/// Each aileron's leading hinge, root then tip.
const AILERON_HINGE_R: [[f32; 3]; 2] = [[49., -29., -1.], [76., -27., -1.]];
const AILERON_HINGE_L: [[f32; 3]; 2] = [[-49., -29., -1.], [-78., -27., -1.]];
/// The airbrake's forward edge, its hinge, and the raised free edge's offset
/// from it (y -17, z +14).
const BRAKE_HINGE: [f32; 3] = [0., 43., 17.];
/// The dorsal ridge the closed brake lies along: the apex (0, 43, 17) to the
/// midpoint (0, 24, 15) of the tail edge of the spine's triangle `331a`.
const SPINE: [[f32; 3]; 2] = [[0., 43., 17.], [0., 24., 15.]];
/// The nozzle exit plane the plume grows from.
const FLAME_ROOT: f32 = -78.;
/// The main plates' fold: the plate planes (right x 25, left x -24) hang from
/// the wing at z 0; the fold line lies 1.5 units below that, so the stowed
/// plate lies half a unit under the wing skin instead of on it.
const MAIN_HINGE_Z: f32 = -1.5;
const MAIN_PLANE_R: f32 = 25.;
const MAIN_PLANE_L: f32 = -24.;
/// The nose plates fold forward about the plate's aft top corner.
const NOSE_HINGE: [f32; 3] = [0., 57., -5.];

pub struct Rig {
    flaps: BTreeMap<usize, Morph>,
}
/// The flap's down pose: the trailing edge drops 6 units and the inboard
/// trailing corner draws in 2 units; the two hinge vertices stay.
fn flap_target(p: [f32; 3]) -> [f32; 3] {
    if p[1] == -32. {
        [p[0], -30., -7.]
    } else if p[1] == -36. {
        [p[0], -36., -7.]
    } else {
        p
    }
}
/// The airbrake's closing turn (radians): from the raised plate to its free
/// edge lying on the dorsal ridge.
fn brake_closed() -> f64 {
    let free_dy = f64::from(26. - BRAKE_HINGE[1]);
    let free_dz = f64::from(31. - BRAKE_HINGE[2]);
    // The raised plate's free edge stands this far above the horizontal.
    let raised = free_dz.atan2(-free_dy);
    // The ridge falls toward the tail, so the free edge lies on it when its
    // elevation is -atan(slope).
    let slope = f64::from(SPINE[0][2] - SPINE[1][2]) / f64::from(SPINE[0][1] - SPINE[1][1]);
    raised + slope.atan()
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 22506
            || shape.faces.len() != 321
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed SU34.SH layout".into());
        }
        let on = |ids: &[usize], points: &[[f32; 3]]| roots_in(NAME, &shape, ids, points);
        on(
            &CANARD_R,
            &[[20., 16., -3.], [20., 31., -3.], [38., 12., -3.]],
        )?;
        on(
            &CANARD_L,
            &[[-21., 16., -3.], [-21., 30., -3.], [-39., 13., -3.]],
        )?;
        on(
            &TAIL_R[..2],
            &[[24., -35., -2.], [28., -68., -5.], [50., -71., -5.]],
        )?;
        on(
            &TAIL_R[2..],
            &[[24., -35., -2.], [23., -63., -5.], [28., -68., -5.]],
        )?;
        on(
            &TAIL_L[..2],
            &[[-23., -35., -2.], [-28., -68., -5.], [-50., -71., -5.]],
        )?;
        on(
            &TAIL_L[2..],
            &[[-23., -35., -2.], [-23., -63., -5.], [-28., -68., -5.]],
        )?;
        on(
            &AILERON_R,
            &[
                AILERON_HINGE_R[0],
                AILERON_HINGE_R[1],
                [49., -36., -1.],
                [76., -40., -1.],
            ],
        )?;
        on(
            &AILERON_L,
            &[
                AILERON_HINGE_L[0],
                AILERON_HINGE_L[1],
                [-49., -36., -1.],
                [-79., -40., -1.],
            ],
        )?;
        // The fins carry no rudder face: they only have to exist.
        for a in FIN {
            one_in(NAME, &shape, a)?;
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        let left = branch_in(
            NAME,
            bytes,
            &original,
            0x67d8,
            -1,
            &[0x4d1b, 0x4d32],
            &[0x4cbe, 0x4cd5],
        )?;
        // +1 removes the neutral faces and adds none (no up pose): the rig
        // never draws it, but a shape that differs is refused.
        branch_in(NAME, bytes, &original, 0x67d8, 1, &[], &[0x4cbe, 0x4cd5])?;
        let right = branch_in(
            NAME,
            bytes,
            &original,
            0x67de,
            -1,
            &[0x4c71, 0x4c88],
            &[0x4c14, 0x4c3b],
        )?;
        branch_in(NAME, bytes, &original, 0x67de, 1, &[], &[0x4c14, 0x4c3b])?;
        let mut flaps = BTreeMap::new();
        for (pairs, pose) in [(&FLAP_L, &left), (&FLAP_R, &right)] {
            for &(neutral, down) in pairs {
                let base = one_in(NAME, &shape, neutral)?;
                let drawn = one_in(NAME, pose, down)?;
                let deployed: Vec<_> = base.positions.iter().copied().map(flap_target).collect();
                if base.positions.len() != 4
                    || drawn.positions.len() != 4
                    || deployed.iter().any(|p| !drawn.positions.contains(p))
                    || base
                        .positions
                        .iter()
                        .zip(&deployed)
                        .filter(|(p, q)| p == q)
                        .count()
                        != 2
                {
                    return Err("unreviewed SU34.SH flap correspondence".into());
                }
                flaps.insert(
                    neutral,
                    Morph {
                        neutral: base.positions.clone(),
                        deployed,
                    },
                );
            }
        }
        for (word, ids) in [
            (0x67c0, &FLAME[..]),
            (0x67c6, &BRAKE[..]),
            (0x67cc, &GEAR[..]),
        ] {
            let pose = branch_in(NAME, bytes, &original, word, 1, ids, &[])?;
            shape
                .faces
                .extend(pose.faces.into_iter().filter(|f| ids.contains(&f.address)));
        }
        roots_in(NAME, &shape, &GEAR_R, &[[25., -28., -25.], [25., -14., 0.]])?;
        roots_in(
            NAME,
            &shape,
            &GEAR_L,
            &[[-24., -28., -25.], [-24., -14., 0.]],
        )?;
        roots_in(
            NAME,
            &shape,
            &GEAR_NOSE[..2],
            &[[0., 57., -5.], [0., 69., -26.]],
        )?;
        roots_in(
            NAME,
            &shape,
            &GEAR_NOSE[2..],
            &[[-2., 66., -26.], [2., 66., -5.]],
        )?;
        roots_in(NAME, &shape, &BRAKE, &[[4., 43., 17.], [-4., 26., 31.]])?;
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if let Some(m) = self.flaps.get(&a).filter(|m| m.neutral == source.positions) {
            result.positions = m.at(state.flaps.clamp(0., 1.) as f32);
            update_normal(source, &mut result);
        } else if TAIL_L.contains(&a) || TAIL_R.contains(&a) {
            let left = TAIL_L.contains(&a);
            let pivot = if left { TAIL_PIVOT_L } else { TAIL_PIVOT_R };
            turn(&mut result, pivot, [1., 0., 0.], tail_angle(state, left));
            update_normal(source, &mut result);
        } else if CANARD_L.contains(&a) || CANARD_R.contains(&a) {
            let pivot = if CANARD_L.contains(&a) {
                CANARD_PIVOT_L
            } else {
                CANARD_PIVOT_R
            };
            turn(&mut result, pivot, [1., 0., 0.], canard_angle(state));
            update_normal(source, &mut result);
        } else if AILERON_L.contains(&a) || AILERON_R.contains(&a) {
            let [root, tip] = if AILERON_L.contains(&a) {
                AILERON_HINGE_L
            } else {
                AILERON_HINGE_R
            };
            let axis = std::array::from_fn(|i| tip[i] - root[i]);
            turn(&mut result, root, axis, aileron_angle(state));
            // The two hinge vertices lie on the axis: keep them exactly.
            for (p, old) in result.positions.iter_mut().zip(&source.positions) {
                if *old == root || *old == tip {
                    *p = *old;
                }
            }
            update_normal(source, &mut result);
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear);
            update_normal(source, &mut result);
        } else if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            brake_turn(&mut result, BRAKE_HINGE, brake_closed(), state.brake);
            update_normal(source, &mut result);
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            flame_scale(&mut result, FLAME_ROOT, state.exhaust);
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
/// The gear folds as rigid plates: each main turns about the fore-aft line
/// through its plate plane 1.5 units below the wing, inward across the nacelle
/// belly to lie flat above it; the nose plate and the plate across it turn
/// forward about the plate's aft top corner into the nose.
fn gear_positions(face: &mut Face, gear: f64) {
    let closing = 1. - gear.clamp(0., 1.);
    let a = face.address;
    if GEAR_NOSE.contains(&a) {
        turn(face, NOSE_HINGE, [1., 0., 0.], FRAC_PI_2 * closing);
    } else {
        let (side, x) = if GEAR_R.contains(&a) {
            (1., MAIN_PLANE_R)
        } else {
            (-1., MAIN_PLANE_L)
        };
        turn(
            face,
            [x, 0., MAIN_HINGE_Z],
            [0., 1., 0.],
            side * FRAC_PI_2 * closing,
        );
    }
}
pub(crate) fn flame(a: usize) -> bool {
    FLAME.contains(&a)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            address,
            colors: vec![17; positions.len()],
            uv: vec![[0.1, 0.9]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
            subtype: 0xed,
        }
    }
    fn rig() -> Rig {
        let mut flaps = BTreeMap::new();
        let neutral = vec![
            [23., -22., -1.],
            [49., -29., -1.],
            [49., -36., -1.],
            [23., -32., -1.],
        ];
        let deployed = neutral.iter().copied().map(flap_target).collect();
        flaps.insert(0x4c14, Morph { neutral, deployed });
        Rig { flaps }
    }
    fn d2(a: [f32; 3], b: [f32; 3]) -> f32 {
        (0..3).map(|i| (a[i] - b[i]).powi(2)).sum()
    }
    fn rigid(a: &Face, b: &Face) -> bool {
        (0..a.positions.len()).all(|i| {
            (i + 1..a.positions.len()).all(|j| {
                (d2(a.positions[i], a.positions[j]) - d2(b.positions[i], b.positions[j])).abs()
                    < 1e-3
            })
        })
    }
    /// One rig test for the surfaces and laws: the four taileron faces and
    /// the canard plate each turn whole about their spindle on pitch and
    /// roll (right trailing edge up for a pull and a positive aileron), the
    /// aileron keeps its hinge, the flap morphs between its two poses with
    /// its hinge fixed, the airbrake closes onto the spine, the gear folds
    /// inward and forward and the fins and rudder stay put.
    #[test]
    fn su34_surfaces_turn_whole_and_the_rest_stays_put() {
        let rig = rig();
        let mut s = state();
        let tail = [
            [24., -35., -2.],
            [55., -62., -5.],
            [50., -71., -5.],
            [28., -68., -5.],
        ];
        let canard = [
            [38., 16., -3.],
            [20., 31., -3.],
            [20., 16., -3.],
            [38., 12., -3.],
        ];
        for (elevator, aileron) in [(1., 0.), (-1., 0.), (0., 1.), (0., -1.), (1., 1.)] {
            s.elevator = elevator;
            s.aileron = aileron;
            let source = face(0x4837, tail.to_vec());
            let out = rig.animate(&source, &s).unwrap();
            assert!(rigid(&source, &out));
            // The root edge midpoint (the spindle) holds: the quad's root
            // corner and the triangle's swing equal and opposite, and the
            // triangle turns by the same angle as the quad.
            let triangle = face(0x4777, vec![tail[0], tail[3], [23., -63., -5.]]);
            let moved = rig.animate(&triangle, &s).unwrap();
            assert!(rigid(&triangle, &moved));
            let mid: [f32; 3] =
                std::array::from_fn(|i| (out.positions[0][i] + moved.positions[2][i]) * 0.5);
            assert!(d2(mid, TAIL_PIVOT_R) < 1e-6);
            assert!(d2(out.positions[3], moved.positions[1]) < 1e-6);
            // Trailing tip: up for a pull or a positive aileron.
            let dz = out.positions[2][2] - tail[2][2];
            let net = 0.3 * elevator + 0.1 * aileron;
            assert!(
                dz * net as f32 > 0.,
                "pitch plus roll sign {elevator} {aileron}"
            );
        }
        // Canard: pitch only, a quarter of the elevator.
        s.aileron = 1.;
        s.elevator = 1.;
        let out = rig.animate(&face(0x47cc, canard.to_vec()), &s).unwrap();
        assert!(rigid(&face(0x47cc, canard.to_vec()), &out));
        assert!(out.positions[3][2] < -3.);
        s.elevator = 0.;
        let out = rig.animate(&face(0x47cc, canard.to_vec()), &s).unwrap();
        assert_eq!(out.positions, canard.to_vec());
        // Aileron: hinge exact, trailing edge up for a positive aileron on
        // the right and down on the left.
        for (id, pts, sign) in [
            (
                0x4886,
                vec![
                    [76., -27., -1.],
                    [49., -29., -1.],
                    [49., -36., -1.],
                    [76., -40., -1.],
                ],
                1.,
            ),
            (
                0x36d6,
                vec![
                    [-78., -27., -1.],
                    [-79., -40., -1.],
                    [-49., -36., -1.],
                    [-49., -29., -1.],
                ],
                -1.,
            ),
        ] {
            let source = face(id, pts);
            s.aileron = 1.;
            let out = rig.animate(&source, &s).unwrap();
            assert!(rigid(&source, &out));
            let hinge = source.positions.iter().position(|p| p[1] == -29.).unwrap();
            assert_eq!(out.positions[hinge], source.positions[hinge]);
            let trailing = source.positions.iter().position(|p| p[1] == -36.).unwrap();
            assert!((out.positions[trailing][2] - -1.) * sign > 0.);
        }
        // Flap: neutral at 0, the reviewed down pose at 1, hinge fixed.
        let neutral = rig.flaps[&0x4c14].neutral.clone();
        let source = face(0x4c14, neutral.clone());
        s.flaps = 0.;
        assert_eq!(rig.animate(&source, &s).unwrap().positions, neutral);
        s.flaps = 1.;
        let down = rig.animate(&source, &s).unwrap().positions;
        assert_eq!(down[0], neutral[0]);
        assert_eq!(down[1], neutral[1]);
        assert_eq!(down[3], [23., -30., -7.]);
        assert_eq!(down[2], [49., -36., -7.]);
        // Fins: no rudder motion.
        s.rudder = 1.;
        let fin = face(
            0x37cb,
            vec![[-20., -30., 38.], [-20., -42., 38.], [-20., -40., 5.]],
        );
        assert_eq!(rig.animate(&fin, &s).unwrap().positions, fin.positions);
        // Brake: hidden closed, the source pose at 1, the free edge on the
        // spine just above closed.
        let plate = vec![
            [4., 43., 17.],
            [-4., 43., 17.],
            [-4., 26., 31.],
            [4., 26., 31.],
        ];
        let source = face(0x4dc9, plate.clone());
        s.brake = 0.;
        assert!(rig.animate(&source, &s).is_none());
        s.brake = 1.;
        let open = rig.animate(&source, &s).unwrap();
        assert!(
            open.positions
                .iter()
                .zip(&plate)
                .all(|(p, q)| d2(*p, *q) < 1e-6)
        );
        s.brake = 1e-9;
        let shut = rig.animate(&source, &s).unwrap();
        let ridge = 17. - (43. - shut.positions[2][1]) * 2. / 19.;
        assert!((shut.positions[2][2] - ridge).abs() < 1e-3);
        assert_eq!(shut.positions[0], plate[0]);
        // Gear: hidden at 0; the source pose at 1; mains fold inward and keep
        // their side of the centre line; the nose plate folds forward.
        s.gear = 0.;
        assert!(rig.animate(&face(0x4a7b, vec![[0.; 3]; 3]), &s).is_none());
        let mains = [(0x4a7b, 25f32, 1f32), (0x4ae6, -24., -1.)];
        for (a, x, side) in mains {
            let source = face(
                a,
                vec![
                    [x, -28., -25.],
                    [x, -14., -25.],
                    [x, -14., 0.],
                    [x, -28., 0.],
                ],
            );
            for step in 0..=100 {
                let mut out = source.clone();
                gear_positions(&mut out, f64::from(step) / 100.);
                assert!(rigid(&source, &out));
                assert!(out.positions.iter().all(|p| p[0] * side >= -0.5));
                if step == 100 {
                    assert_eq!(out.positions, source.positions);
                }
            }
            let mut out = source.clone();
            gear_positions(&mut out, 0.);
            assert!(
                out.positions
                    .iter()
                    .all(|p| (p[2] - MAIN_HINGE_Z).abs() < 1e-3)
            );
        }
        let nose = face(
            0x4b69,
            vec![
                [0., 57., -5.],
                [0., 69., -5.],
                [0., 69., -26.],
                [0., 57., -26.],
            ],
        );
        let mut out = nose.clone();
        gear_positions(&mut out, 0.);
        assert!(rigid(&nose, &out));
        assert!(out.positions.iter().all(|p| p[1] >= 56.9 && p[2] >= -5.1));
    }
}
