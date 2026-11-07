//! MiG25 own-source control surfaces (all-moving stabilators, twin rudders,
//! inboard flaps and outboard ailerons), gear, airbrake and afterburner flame.
//! See docs/spec/variety-animation.md "MiG-25 control fit".
//! MIG25.SH ships no signed pitch, roll, yaw or flap branch, so every control
//! role is a fitted geometric assignment over exact reviewed source panels.
//! The five state words follow the shared base order flame, brake, gear, flap,
//! flap: 0x7600 adds the flame, 0x7606 the airbrake, 0x760c the gear, and
//! 0x7618/0x761e remove the left and right flap pairs (the M25-3 surfaces).
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 5] = [0x7600, 0x7606, 0x760c, 0x7618, 0x761e];
/// All-moving horizontal stabilators, opposite skins of one flat plate per side.
const STABILATOR_R: [usize; 2] = [0x3daa, 0x3e4a];
const STABILATOR_L: [usize; 2] = [0x434c, 0x43a2];
/// Outboard trailing control. A forward triangle and an aft quad per skin, so
/// the whole panel is one group and the diagonal seam never half-moves.
const AILERON_R: [usize; 4] = [0x3c96, 0x3ea3, 0x3cb9, 0x3ce2];
const AILERON_L: [usize; 4] = [0x41d1, 0x43fb, 0x421d, 0x42b1];
/// Inboard trailing control, the pair the source words 0x7618/0x761e remove.
const FLAP_R: [usize; 2] = [0x5020, 0x5047];
const FLAP_L: [usize; 2] = [0x5127, 0x514e];
/// Rear fin panel behind the source split line, both sides.
const RUDDER_R: [usize; 2] = [0x320f, 0x3237];
const RUDDER_L: [usize; 2] = [0x2c1f, 0x2cb2];
/// Gear word 0x760c. One rigid strut plate and one rigid painted wheel card per
/// main side, and a centre strut with its cross plate at the nose. Each main
/// assembly turns as a whole about the aft-top lateral hinge; the nose assembly
/// also steers about its vertical strut axis.
const GEAR_MAIN_R: [usize; 4] = [0x4d69, 0x4d88, 0x4da7, 0x4dc6];
const GEAR_MAIN_L: [usize; 4] = [0x4e2a, 0x4e49, 0x4e68, 0x4e87];
const GEAR_NOSE: [usize; 4] = [0x4eeb, 0x4f0a, 0x4f29, 0x4f48];
const GEAR: [usize; 12] = [
    0x4d69, 0x4d88, 0x4da7, 0x4dc6, 0x4e2a, 0x4e49, 0x4e68, 0x4e87, 0x4eeb, 0x4f0a, 0x4f29, 0x4f48,
];
/// Main hinge line: the lateral (X) axis through the strut's aft-top root at
/// Y=-19, Z=-10. The whole assembly swings aft-to-up and stows at the belly.
const MAIN_HINGE: [f32; 3] = [0., -19., -10.];
/// Nose hinge line through the strut's aft-top root at Y=45, Z=-9.
const NOSE_HINGE: [f32; 3] = [0., 45., -9.];
/// Vertical nose strut axis for steering, at the deployed strut centre.
const NOSE_STEER: [f32; 3] = [0., 50., -9.];
/// Afterburner word 0x7600: a crossed double-sided plate per engine, trailing
/// from the nozzle exit at Y=-70 to Y=-129.
const FLAME: [usize; 8] = [
    0x51dc, 0x5203, 0x522a, 0x5251, 0x5278, 0x529f, 0x52c6, 0x52ed,
];
const FLAME_ROOT: f32 = -70.;
/// Airbrake word 0x7606: one double-sided dorsal panel hinged at the spine
/// front edge (0,3,9), rising to (0,-13,23).
const BRAKE: [usize; 2] = [0x5367, 0x5386];
const BRAKE_HINGE: [f32; 3] = [0., 3., 9.];
/// Fitted closed angle: rotates the source raised panel flat along the spine.
const BRAKE_CLOSED: f64 = 0.72;
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("MIG25.SH missing reviewed face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("MIG25.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
/// A panel's hinge corners are covered by the union of its faces: the diagonal
/// aileron split and the two flap skins do not each carry every hinge corner,
/// but every reviewed corner must appear somewhere in the group.
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    let faces: Vec<&Face> = addresses
        .iter()
        .map(|&address| one(shape, address))
        .collect::<AppResult<_>>()?;
    for p in points {
        if !faces.iter().any(|f| f.positions.contains(p)) {
            return Err(format!("unreviewed MIG25.SH hinge corner {p:?}").into());
        }
    }
    Ok(())
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
        return Err(format!("unreviewed MIG25.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
/// Source hinge point, direction and fitted angle sign, one entry per moving
/// side. The aileron, flap and rudder lines pass through the reviewed source
/// corners the fixed panel shares, so those corners do not move and the skins
/// stay coincident. The all-moving stabilator has no fixed source corner, so its
/// line passes through the root chord midpoint instead.
fn hinge(address: usize) -> ([f32; 3], [f32; 3], f64) {
    if STABILATOR_R.contains(&address) || STABILATOR_L.contains(&address) {
        let side = if STABILATOR_R.contains(&address) {
            18.
        } else {
            -18.
        };
        ([side, -57., -1.], [1., 0., 0.], -0.30)
    } else if AILERON_R.contains(&address) {
        ([39., -29., 5.], [31., 7., -3.], -0.20)
    } else if AILERON_L.contains(&address) {
        ([-39., -29., 5.], [-31., 7., -3.], -0.20)
    } else if FLAP_R.contains(&address) {
        ([18., -25., 8.], [21., -4., -3.], 0.40)
    } else if FLAP_L.contains(&address) {
        ([-18., -25., 8.], [-21., -4., -3.], -0.40)
    } else if RUDDER_R.contains(&address) {
        ([16., -40., 7.], [7., -23., 31.], 0.35)
    } else {
        ([-16., -40., 8.], [-7., -23., 30.], 0.35)
    }
}
fn is_stabilator(a: usize) -> bool {
    STABILATOR_R.contains(&a) || STABILATOR_L.contains(&a)
}
fn is_aileron(a: usize) -> bool {
    AILERON_R.contains(&a) || AILERON_L.contains(&a)
}
fn is_flap(a: usize) -> bool {
    FLAP_R.contains(&a) || FLAP_L.contains(&a)
}
fn is_rudder(a: usize) -> bool {
    RUDDER_R.contains(&a) || RUDDER_L.contains(&a)
}
/// Whole-card gear motion. The two cards of each assembly share one lateral
/// hinge, so they stay rigid and attached as the wheel swings up into the body.
pub fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let closing = 1. - gear.clamp(0., 1.);
    let angle = FRAC_PI_2 * closing;
    if GEAR_NOSE.contains(&f.address) {
        if steering != 0. {
            turn(f, NOSE_STEER, [0., 0., 1.], -steering);
        }
        turn(f, NOSE_HINGE, [1., 0., 0.], angle);
    } else {
        turn(f, MAIN_HINGE, [1., 0., 0.], angle);
    }
}
pub struct Rig;
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 26154
            || shape.faces.len() != 320
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed MIG25.SH layout".into());
        }
        // Every moving panel is an exact reviewed source group. The hinge
        // corners that stay fixed are the vertices the fixed panel also shares,
        // except the all-moving stabilator, whose root chord is checked here and
        // whose midpoint the probe uses as its invariant.
        roots(&shape, &STABILATOR_R, &[[18., -45., -1.], [18., -69., -1.]])?;
        roots(
            &shape,
            &STABILATOR_L,
            &[[-18., -45., -1.], [-18., -69., -1.]],
        )?;
        roots(&shape, &AILERON_R, &[[39., -29., 5.], [70., -22., 2.]])?;
        roots(&shape, &AILERON_L, &[[-39., -29., 5.], [-70., -22., 2.]])?;
        roots(&shape, &FLAP_R, &[[18., -25., 8.], [39., -29., 5.]])?;
        roots(&shape, &FLAP_L, &[[-18., -25., 8.], [-39., -29., 5.]])?;
        roots(&shape, &RUDDER_R, &[[16., -40., 7.], [23., -63., 38.]])?;
        roots(&shape, &RUDDER_L, &[[-16., -40., 8.], [-23., -63., 38.]])?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words remove exactly the M25-3 flap pairs: 0x7618 the left,
        // 0x761e the right. This pins the word-to-surface mapping.
        branch(bytes, &original, 0x7618, 1, &[], &FLAP_L)?;
        branch(bytes, &original, 0x761e, 1, &[], &FLAP_R)?;
        let gear = branch(bytes, &original, 0x760c, 1, &GEAR, &[])?;
        roots(&gear, &GEAR_MAIN_R, &[[12., -19., -10.]])?;
        roots(&gear, &GEAR_MAIN_L, &[[-12., -19., -10.]])?;
        roots(&gear, &GEAR_NOSE, &[[0., 45., -9.]])?;
        for source in gear.faces.iter().filter(|f| GEAR.contains(&f.address)) {
            if source.positions.len() != 4 {
                return Err("unreviewed MIG25.SH gear card topology".into());
            }
        }
        // The brake word adds one raised double-sided dorsal panel hinged on the
        // spine front edge (0,3,9); the flame word adds eight aft quads rooted
        // at the Y=-70 nozzle exit.
        let brake = branch(bytes, &original, 0x7606, 1, &BRAKE, &[])?;
        for &address in &BRAKE {
            let face = one(&brake, address)?;
            // The hinge is the spine front edge; both corners carry it.
            if face.positions.len() != 4
                || !face.positions.contains(&[5., 3., 9.])
                || !face.positions.contains(&[-5., 3., 9.])
            {
                return Err("unreviewed MIG25.SH brake panel".into());
            }
        }
        let flame = branch(bytes, &original, 0x7600, 1, &FLAME, &[])?;
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4
                || face
                    .positions
                    .iter()
                    .map(|p| p[1])
                    .fold(f32::NEG_INFINITY, f32::max)
                    != FLAME_ROOT
            {
                return Err("unreviewed MIG25.SH flame root".into());
            }
        }
        for source in gear
            .faces
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
            )
        {
            shape.faces.push(source);
        }
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if is_stabilator(a) {
            let (pivot, axis, base) = hinge(a);
            turn(
                &mut result,
                pivot,
                axis,
                base * state.elevator.clamp(-1., 1.),
            );
            update_normal(source, &mut result);
        } else if is_rudder(a) {
            let (pivot, axis, base) = hinge(a);
            turn(&mut result, pivot, axis, base * state.rudder.clamp(-1., 1.));
            update_normal(source, &mut result);
        } else if is_flap(a) {
            let (pivot, axis, base) = hinge(a);
            turn(&mut result, pivot, axis, base * state.flaps.clamp(0., 1.));
            update_normal(source, &mut result);
        } else if is_aileron(a) {
            let (pivot, axis, base) = hinge(a);
            turn(
                &mut result,
                pivot,
                axis,
                base * state.aileron.clamp(-1., 1.),
            );
            update_normal(source, &mut result);
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
            update_normal(source, &mut result);
        } else if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            turn(
                &mut result,
                BRAKE_HINGE,
                [1., 0., 0.],
                BRAKE_CLOSED * (1. - state.brake.clamp(0., 1.)),
            );
            update_normal(source, &mut result);
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = FLAME_ROOT + (p[1] - FLAME_ROOT) * state.exhaust.clamp(0., 1.) as f32;
            }
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
pub(crate) fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}
fn polygon_normal(points: &[[f32; 3]]) -> Option<[f32; 3]> {
    let mut n = [0f64; 3];
    for (a, b) in points
        .iter()
        .zip(points.iter().cycle().skip(1))
        .take(points.len())
    {
        n[0] += f64::from(a[1] - b[1]) * f64::from(a[2] + b[2]);
        n[1] += f64::from(a[2] - b[2]) * f64::from(a[0] + b[0]);
        n[2] += f64::from(a[0] - b[0]) * f64::from(a[1] + b[1]);
    }
    let length = n.iter().map(|v| v * v).sum::<f64>().sqrt();
    (length > 1e-9).then(|| n.map(|v| (v / length) as f32))
}
fn update_normal(source: &Face, result: &mut Face) {
    let (Some(old), Some(reference), Some(mut n)) = (
        source.normal,
        polygon_normal(&source.positions),
        polygon_normal(&result.positions),
    ) else {
        return;
    };
    if [old[0], old[2], old[1]]
        .iter()
        .zip(reference)
        .map(|(a, b)| a * b)
        .sum::<f32>()
        < 0.
    {
        n = n.map(|v| -v);
    }
    result.normal = Some([n[0], n[2], n[1]]);
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
            colors: vec![31; positions.len()],
            uv: vec![[0.2, 0.8]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([0., 0., 1.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn off_hinge(a: usize, p: [f32; 3]) -> f64 {
        let (pivot, axis, _) = hinge(a);
        let offset: [f64; 3] = std::array::from_fn(|i| f64::from(p[i] - pivot[i]));
        let cross = [
            f64::from(axis[1]) * offset[2] - f64::from(axis[2]) * offset[1],
            f64::from(axis[2]) * offset[0] - f64::from(axis[0]) * offset[2],
            f64::from(axis[0]) * offset[1] - f64::from(axis[1]) * offset[0],
        ];
        cross.iter().map(|v| v * v).sum::<f64>().sqrt()
            / (axis.iter().map(|v| f64::from(v * v)).sum::<f64>().sqrt())
    }
    #[test]
    fn hinge_lines_pass_through_the_shared_source_corners() {
        // The ailerons, flaps and rudders hinge on the exact source edge the
        // fixed panel shares. The all-moving stabilators hinge on their root
        // chord midpoint because no single root corner stays put.
        for (a, points) in [
            (0x3c96, [[39., -29., 5.], [70., -22., 2.]]),
            (0x5020, [[18., -25., 8.], [39., -29., 5.]]),
            (0x320f, [[16., -40., 7.], [23., -63., 38.]]),
        ] {
            for p in points {
                assert!(off_hinge(a, p) < 1e-3, "face {a:x} point {p:?} off hinge");
            }
        }
        assert!(off_hinge(0x3daa, [18., -57., -1.]) < 1e-3);
        assert!(off_hinge(0x434c, [-18., -57., -1.]) < 1e-3);
    }
    #[test]
    fn controls_keep_their_hinge_corners_and_move_the_expected_way() {
        let rig = Rig;
        let mut s = state();
        // address, a point the hinge holds, a trailing witness, measured axis,
        // sign of a positive command, and the field that drives it.
        type Case = (
            &'static [usize],
            [f32; 3],
            [f32; 3],
            usize,
            f32,
            &'static str,
        );
        let panels: [Case; 7] = [
            (
                &STABILATOR_R,
                [18., -57., -1.],
                [18., -69., -1.],
                2,
                1.,
                "elevator",
            ),
            (
                &STABILATOR_L,
                [-18., -57., -1.],
                [-18., -69., -1.],
                2,
                1.,
                "elevator",
            ),
            (
                &RUDDER_R,
                [23., -63., 38.],
                [22., -73., 33.],
                0,
                1.,
                "rudder",
            ),
            (
                &RUDDER_L,
                [-23., -63., 38.],
                [-22., -73., 33.],
                0,
                1.,
                "rudder",
            ),
            (&FLAP_R, [39., -29., 5.], [18., -36., 6.], 2, -1., "flaps"),
            (
                &AILERON_R,
                [70., -22., 2.],
                [70., -44., 2.],
                2,
                1.,
                "aileron",
            ),
            (
                &AILERON_L,
                [-70., -22., 2.],
                [-70., -44., 2.],
                2,
                -1.,
                "aileron",
            ),
        ];
        for (addresses, anchor, witness, axis, sign, field) in panels {
            for command in [0., 1.] {
                s.elevator = 0.;
                s.rudder = 0.;
                s.flaps = 0.;
                s.aileron = 0.;
                match field {
                    "elevator" => s.elevator = command,
                    "rudder" => s.rudder = command,
                    "flaps" => s.flaps = command,
                    _ => s.aileron = command,
                }
                for &address in addresses {
                    let source = face(address, vec![anchor, witness]);
                    let out = rig.animate(&source, &s).unwrap();
                    if command == 0. {
                        assert_eq!(out.positions, source.positions, "face {address:x} at zero");
                    } else {
                        assert!(
                            (out.positions[1][axis] - witness[axis]) * sign > 1e-4,
                            "face {address:x} witness did not move the reviewed way"
                        );
                        assert!(
                            (out.positions[0][axis] - anchor[axis]).abs() < 1e-4,
                            "face {address:x} hinge anchor moved"
                        );
                    }
                }
            }
        }
    }
    /// Every gear card is a rigid quad, its own hinge corner does not move, and
    /// at near-stow the whole card tucks to the belly plane.
    #[test]
    fn gear_cards_stay_rigid_on_their_hinge_and_stow_at_the_belly() {
        let rig = Rig;
        let mut s = state();
        let cards: [(&[usize], [f32; 3], [f32; 3]); 3] = [
            (&GEAR_MAIN_R, [12., -19., -10.], [18., -19., -28.]),
            (&GEAR_MAIN_L, [-12., -19., -10.], [-18., -19., -28.]),
            (&GEAR_NOSE, [0., 45., -9.], [0., 45., -27.]),
        ];
        for (addresses, root, tip) in cards {
            for &address in addresses {
                let source = face(address, vec![root, tip, [root[0], root[1], tip[2]]]);
                for step in 1..=400 {
                    s.gear = f64::from(step) / 400.;
                    let out = rig.animate(&source, &s).unwrap();
                    let original = [root, tip, [root[0], root[1], tip[2]]];
                    for i in 0..3 {
                        for j in i + 1..3 {
                            let a = original[i];
                            let b = original[j];
                            let keep = (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt();
                            let now = (0..3)
                                .map(|k| (out.positions[i][k] - out.positions[j][k]).powi(2))
                                .sum::<f32>()
                                .sqrt();
                            assert!((keep - now).abs() < 1e-3, "face {address:x} not rigid");
                        }
                    }
                    assert!(
                        (out.positions[0][1] - root[1]).abs() < 1e-4
                            && (out.positions[0][2] - root[2]).abs() < 1e-4,
                        "face {address:x} hinge root moved"
                    );
                }
                s.gear = 1e-6;
                let stowed = rig.animate(&source, &s).unwrap();
                assert!(
                    stowed.positions.iter().all(|p| p[2] >= -10.5),
                    "face {address:x} hangs below the belly at near-stow"
                );
            }
        }
        s.gear = 0.;
        assert!(
            rig.animate(&face(GEAR_MAIN_R[0], vec![[12., -19., -10.]]), &s)
                .is_none()
        );
    }
    #[test]
    fn brake_rises_from_its_spine_hinge_and_flame_extends_from_the_nozzle() {
        let rig = Rig;
        let mut s = state();
        let brake = face(
            0x5367,
            vec![
                [5., 3., 9.],
                [-5., 3., 9.],
                [-2., -13., 23.],
                [2., -13., 23.],
            ],
        );
        for fraction in [0., 0.25, 0.5, 0.75, 1.] {
            s.brake = fraction;
            if fraction == 0. {
                assert!(rig.animate(&brake, &s).is_none());
                continue;
            }
            let out = rig.animate(&brake, &s).unwrap();
            assert_eq!(out.positions[0], brake.positions[0]);
            assert_eq!(out.positions[1], brake.positions[1]);
            if fraction == 1. {
                assert_eq!(out.positions, brake.positions);
            } else {
                assert!(out.positions[3][2] < 23., "brake tip did not close down");
            }
        }
        let flame = face(
            0x51dc,
            vec![
                [10., -129., -8.],
                [10., -70., -8.],
                [10., -70., 8.],
                [10., -129., 8.],
            ],
        );
        for fraction in [0., 0.5, 1.] {
            s.exhaust = fraction;
            if fraction == 0. {
                assert!(rig.animate(&flame, &s).is_none());
                continue;
            }
            let out = rig.animate(&flame, &s).unwrap();
            assert!(
                (out.positions[1][1] - FLAME_ROOT).abs() < 1e-4
                    && (out.positions[2][1] - FLAME_ROOT).abs() < 1e-4,
                "flame root moved off the nozzle"
            );
            if fraction == 1. {
                assert_eq!(out.positions, flame.positions);
            } else {
                assert!(out.positions[0][1] > -129., "flame tip did not shorten");
            }
        }
    }
}
