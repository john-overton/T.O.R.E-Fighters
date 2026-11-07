//! MiG25 own-source control surfaces: all-moving stabilators, twin rudders,
//! inboard flaps and outboard ailerons, plus the interim shared gear law.
//! See docs/spec/variety-animation.md "MiG-25 control fit".
//! MIG25.SH ships no signed pitch, roll, yaw or flap branch, so every control
//! role is a fitted geometric assignment over exact reviewed source panels.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
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
/// Gear word 0x760c cards. M25-4 owns the reviewed retraction; M25-3 keeps the
/// shared fitted rise so the whole aircraft still loads and draws.
const GEAR: [usize; 12] = [
    0x4d69, 0x4d88, 0x4da7, 0x4dc6, 0x4e2a, 0x4e49, 0x4e68, 0x4e87, 0x4eeb, 0x4f0a, 0x4f29, 0x4f48,
];
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
        // The gear word is the only device branch M25-3 needs to keep loading.
        let gear = branch(bytes, &original, 0x760c, 1, &GEAR, &[])?;
        for source in gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)) {
            shape.faces.push(source);
        }
        for &address in GEAR.iter() {
            if one(&shape, address)?.positions.len() != 4 {
                return Err("unreviewed MIG25.SH gear card topology".into());
            }
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
            // Interim shared fitted gear law: rise by the source vertical extent
            // times the remaining travel. M25-4 replaces this with rigid wheels.
            let rise = 19. * (1. - state.gear.clamp(0., 1.)) as f32;
            for p in &mut result.positions {
                p[2] += rise;
            }
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
pub(crate) fn flame(_address: usize) -> bool {
    false
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
}
