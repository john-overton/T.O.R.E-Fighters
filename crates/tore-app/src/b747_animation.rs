//! B747.SH source endpoints/art and fitted independent surfaces with rigid gear.
//! The lower-right flap uses a common subdivision to preserve both source diagonals.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 4] = [0x8960, 0x896c, 0x8972, 0x8978];
const RUDDER: [usize; 2] = [0x62ec, 0x6316];
const LEFT_TAIL: [usize; 11] = [
    0x3823, 0x3845, 0x3863, 0x3ae6, 0x3b08, 0x3b26, 0x557e, 0x55a9, 0x55cf, 0x57af, 0x57cd,
];
const RIGHT_TAIL: [usize; 8] = [
    0x46e7, 0x472f, 0x4755, 0x5512, 0x5530, 0x5651, 0x5670, 0x5692,
];
const LEFT_ROLL: [usize; 4] = [0x2937, 0x2976, 0x2e64, 0x2ea1];
const RIGHT_ROLL: [usize; 4] = [0x4bd9, 0x4c1c, 0x5187, 0x51aa];
const LEFT_GEAR: [usize; 16] = [
    0x5f16, 0x5f3d, 0x5f64, 0x5f8b, 0x5fb2, 0x5fd1, 0x5ff0, 0x600f, 0x602e, 0x604d, 0x606c, 0x608b,
    0x60aa, 0x60c9, 0x60e8, 0x6107,
];
const RIGHT_GEAR: [usize; 16] = [
    0x5c31, 0x5c58, 0x5c7f, 0x5ca6, 0x5ccd, 0x5cec, 0x5d0b, 0x5d2a, 0x5d49, 0x5d68, 0x5d87, 0x5da6,
    0x5dc5, 0x5de4, 0x5e03, 0x5e22,
];
const NOSE: [usize; 4] = [0x616b, 0x618a, 0x61a9, 0x61c8];
const EPS: f32 = 1e-4;
struct Morph {
    neutral: Vec<[f32; 3]>,
    deployed: Face,
}
pub struct Rig {
    flaps: BTreeMap<usize, Vec<Morph>>,
}
#[derive(Clone, Copy)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
    color: u8,
}
fn vertex(f: &Face, i: usize) -> Vertex {
    Vertex {
        position: f.positions[i],
        uv: f.uv[i],
        color: f.colors[i],
    }
}
fn middle(a: Vertex, b: Vertex) -> Vertex {
    Vertex {
        position: std::array::from_fn(|i| (a.position[i] + b.position[i]) * 0.5),
        uv: std::array::from_fn(|i| (a.uv[i] + b.uv[i]) * 0.5),
        color: a.color,
    }
}
fn polygon(proto: &Face, address: usize, vertices: &[Vertex]) -> Face {
    let mut f = proto.clone();
    f.address = address;
    f.positions = vertices.iter().map(|v| v.position).collect();
    f.uv = vertices.iter().map(|v| v.uv).collect();
    f.colors = vertices.iter().map(|v| v.color).collect();
    f
}
fn validate_textured(f: &Face, count: usize) -> AppResult<()> {
    if f.positions.len() != count
        || f.uv.len() != count
        || f.colors.len() != count
        || f.colors.iter().any(|c| *c != f.colors[0])
    {
        return Err("B747.SH unreviewed flap topology or vertex shading".into());
    }
    Ok(())
}
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut faces = shape.faces.iter().filter(|f| f.address == address);
    let f = faces
        .next()
        .ok_or_else(|| format!("B747.SH missing reviewed face {address:x}"))?;
    if faces.next().is_some() {
        return Err("B747.SH duplicate reviewed face".into());
    }
    Ok(f)
}
fn roots(shape: &Shape, ids: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    for &a in ids {
        if !points
            .iter()
            .all(|p| one(shape, a).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("B747.SH missing attachment {a:x}").into());
        }
    }
    Ok(())
}
fn branch(
    bytes: &[u8],
    original: &BTreeSet<usize>,
    word: usize,
    value: i32,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let shape = Shape::with_state(bytes, &[(word, value)].into())?;
    let ids: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
    if ids.difference(original).copied().collect::<BTreeSet<_>>() != added.iter().copied().collect()
        || original.difference(&ids).copied().collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed B747.SH branch {word:x}={value}").into());
    }
    Ok(shape)
}
fn tail(a: usize) -> bool {
    LEFT_TAIL.contains(&a) || RIGHT_TAIL.contains(&a)
}
fn roll(a: usize) -> bool {
    LEFT_ROLL.contains(&a) || RIGHT_ROLL.contains(&a)
}
fn gear(a: usize) -> bool {
    LEFT_GEAR.contains(&a) || RIGHT_GEAR.contains(&a) || NOSE.contains(&a)
}
fn tail_cut(p: [f32; 3]) -> f32 {
    p[1] + 166. + 0.5 * (p[0].abs() - 8.)
}
fn roll_cut(p: [f32; 3]) -> f32 {
    p[1] + 57. + (p[0].abs() - 113.) * 23. / 41.
}
fn roll_panel(f: &Face) -> bool {
    roll(f.address)
        && f.positions
            .iter()
            .all(|p| (118. - EPS..=145. + EPS).contains(&p[0].abs()) && roll_cut(*p) <= EPS)
}
fn split(f: &Face, d: impl Fn([f32; 3]) -> f32) -> Vec<Face> {
    crate::aircraft_animation::split_surface(f, [0.; 3], [1., 0., 0.], 0., d)
        .into_iter()
        .filter(|f| normal(&f.positions).is_some())
        .collect()
}
fn register(
    prepared: &mut Vec<Face>,
    flaps: &mut BTreeMap<usize, Vec<Morph>>,
    neutral: Face,
    deployed: Face,
) {
    flaps.entry(neutral.address).or_default().push(Morph {
        neutral: neutral.positions.clone(),
        deployed,
    });
    prepared.push(neutral);
}
fn right_lower(base: &Face, down_a: &Face, down_b: &Face) -> AppResult<Vec<(Face, Face)>> {
    validate_textured(base, 4)?;
    validate_textured(down_a, 3)?;
    validate_textured(down_b, 3)?;
    if down_a.positions[0] != down_b.positions[0]
        || down_a.positions[1] != down_b.positions[2]
        || down_a.uv[0] != down_b.uv[0]
        || down_a.uv[1] != down_b.uv[2]
    {
        return Err("B747.SH deployed lower flap diagonal does not join".into());
    }
    let n: [Vertex; 4] = std::array::from_fn(|i| vertex(base, i));
    let d = [
        vertex(down_b, 1),
        vertex(down_b, 2),
        vertex(down_a, 2),
        vertex(down_b, 0),
    ];
    let nq = middle(n[0], n[2]);
    let dq = middle(d[1], d[3]);
    let mut result = Vec::new();
    for i in 0..4 {
        let j = (i + 1) % 4;
        let target = if i == 0 || i == 3 { down_b } else { down_a };
        result.push((
            polygon(base, base.address, &[n[i], n[j], nq]),
            polygon(target, base.address, &[d[i], d[j], dq]),
        ));
    }
    Ok(result)
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 31108
            || shape.faces.len() != 373
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed B747.SH animation layout".into());
        }
        roots(&shape, &RUDDER, &[[0., -155., 15.], [0., -186., 58.]])?;
        roots(&shape, &[0x3845], &[[-8., -139., 12.], [-4., -176., 12.]])?;
        roots(
            &shape,
            &[0x472f, 0x4755],
            &[[8., -139., 12.], [4., -176., 12.]],
        )?;
        for a in LEFT_TAIL
            .iter()
            .chain(&RIGHT_TAIL)
            .chain(&LEFT_ROLL)
            .chain(&RIGHT_ROLL)
        {
            one(&shape, *a)?;
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        for (value, ids) in [(1, [0x6206, 0x6230]), (-1, [0x6279, 0x62a3])] {
            let pose = branch(bytes, &original, 0x8978, value, &ids, &RUDDER)?;
            roots(&pose, &ids, &[[0., -155., 15.], [0., -186., 58.]])?;
        }
        let left = branch(
            bytes,
            &original,
            0x896c,
            -1,
            &[0x6454, 0x6473, 0x648e],
            &[0x64d4, 0x64f3],
        )?;
        branch(bytes, &original, 0x896c, 1, &[], &[0x64d4, 0x64f3])?;
        let right = branch(
            bytes,
            &original,
            0x8972,
            -1,
            &[0x636b, 0x638a, 0x63a5],
            &[0x63eb, 0x640a],
        )?;
        branch(bytes, &original, 0x8972, 1, &[], &[0x63eb, 0x640a])?;
        let mut flaps = BTreeMap::new();
        let mut prepared = Vec::new();
        for (base, target, pose) in [(0x64d4, 0x6454, &left), (0x63eb, 0x636b, &right)] {
            let n = one(&shape, base)?;
            let d = one(pose, target)?;
            validate_textured(n, 4)?;
            validate_textured(d, 4)?;
            let mapped = polygon(
                d,
                base,
                &[vertex(d, 2), vertex(d, 3), vertex(d, 0), vertex(d, 1)],
            );
            register(&mut prepared, &mut flaps, n.clone(), mapped);
        }
        let base = one(&shape, 0x64f3)?;
        validate_textured(base, 4)?;
        for (ids, target) in [([0, 1, 2], 0x6473), ([0, 2, 3], 0x648e)] {
            let d = one(&left, target)?;
            validate_textured(d, 3)?;
            let n = polygon(base, base.address, &ids.map(|i| vertex(base, i)));
            let mut d = d.clone();
            d.address = base.address;
            register(&mut prepared, &mut flaps, n, d);
        }
        for (n, d) in right_lower(
            one(&shape, 0x640a)?,
            one(&right, 0x638a)?,
            one(&right, 0x63a5)?,
        )? {
            register(&mut prepared, &mut flaps, n, d);
        }
        for pieces in flaps.values() {
            for morph in pieces {
                for (p, q) in morph.neutral.iter().zip(&morph.deployed.positions) {
                    if (p[0].abs() == 15. && (p[1] == -4. || p[1] == -3.)
                        || p[0].abs() == 60. && p[1] == -18.)
                        && p != q
                    {
                        return Err("B747.SH flap hinge correspondence moved".into());
                    }
                }
            }
        }
        let ids: Vec<_> = LEFT_GEAR
            .iter()
            .chain(&RIGHT_GEAR)
            .chain(&NOSE)
            .copied()
            .collect();
        let down = branch(bytes, &original, 0x8960, 1, &ids, &[])?;
        branch(bytes, &original, 0x8960, -1, &[], &[])?;
        roots(
            &down,
            &[0x5ccd, 0x5cec],
            &[[9., -27., -10.], [2., -27., -17.]],
        )?;
        roots(
            &down,
            &[0x5fb2, 0x5fd1],
            &[[-9., -27., -10.], [-2., -27., -17.]],
        )?;
        roots(
            &down,
            &[0x5d0b, 0x5d2a],
            &[[9., -9., -10.], [2., -9., -17.]],
        )?;
        roots(
            &down,
            &[0x5ff0, 0x600f],
            &[[-9., -9., -10.], [-2., -9., -17.]],
        )?;
        roots(
            &down,
            &[0x616b, 0x618a],
            &[[0., 94., -16.], [0., 104., -16.]],
        )?;
        let flap_prepared = prepared;
        let mut prepared = Vec::new();
        for f in shape.faces {
            if flaps.contains_key(&f.address) {
                prepared.extend(
                    flap_prepared
                        .iter()
                        .filter(|p| p.address == f.address)
                        .cloned(),
                );
                continue;
            }
            if tail(f.address) {
                prepared.extend(split(&f, tail_cut));
            } else if roll(f.address) {
                let pieces = split(&f, |p| p[0].abs() - 118.);
                let pieces: Vec<_> = pieces
                    .into_iter()
                    .flat_map(|p| split(&p, |q| q[0].abs() - 145.))
                    .collect();
                prepared.extend(pieces.into_iter().flat_map(|p| split(&p, roll_cut)));
            } else {
                prepared.push(f);
            }
        }
        prepared.extend(down.faces.into_iter().filter(|f| gear(f.address)));
        shape.faces = prepared;
        validate_gear(&shape)?;
        Ok((Self { flaps }, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        if let Some(parts) = self.flaps.get(&source.address) {
            let amount = state.flaps.clamp(0., 1.) as f32;
            if amount == 0. {
                return Some(source.clone());
            }
            let Some(morph) = parts.iter().find(|m| m.neutral == source.positions) else {
                return Some(source.clone());
            };
            let mut f = morph.deployed.clone();
            if amount < 1. {
                f.positions = morph
                    .neutral
                    .iter()
                    .zip(&morph.deployed.positions)
                    .map(|(p, q)| std::array::from_fn(|i| p[i] + amount * (q[i] - p[i])))
                    .collect();
                update_normal(&morph.deployed, &mut f);
            }
            return Some(f);
        }
        let mut f = source.clone();
        let a = f.address;
        if RUDDER.contains(&a) && state.rudder != 0. {
            turn(
                &mut f,
                [0., -155., 15.],
                [0., -31., 43.],
                0.35 * state.rudder.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if [[0., -155., 15.], [0., -186., 58.]].contains(q) {
                    *p = *q;
                }
            }
        } else if tail(a)
            && source.positions.iter().all(|p| tail_cut(*p) <= EPS)
            && state.elevator != 0.
        {
            let side = if LEFT_TAIL.contains(&a) { -1. } else { 1. };
            turn(
                &mut f,
                [side * 8., -166., 12.],
                [1., -side * 0.5, side * 4. / 43.],
                -0.30 * state.elevator.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if tail_cut(*q).abs() <= EPS || q[0].abs() == 4. && q[1] == -176. {
                    *p = *q;
                }
            }
        } else if roll_panel(source) && state.aileron != 0. {
            let side = if LEFT_ROLL.contains(&a) { -1. } else { 1. };
            turn(
                &mut f,
                [side * 113., -57., 4.5],
                [side * 41., -23., 4.5],
                -0.20 * state.aileron.clamp(-1., 1.),
            );
            for (p, q) in f.positions.iter_mut().zip(&source.positions) {
                if roll_cut(*q).abs() <= EPS {
                    *p = *q;
                }
            }
        } else if gear(a) {
            if state.gear <= 0. {
                return None;
            }
            gear_pose(&mut f, state.gear);
            return Some(f);
        }
        if f.positions != source.positions {
            update_normal(source, &mut f);
        }
        Some(f)
    }
}
fn gear_pose(f: &mut Face, deployed: f64) {
    let closing = 1. - deployed.clamp(0., 1.);
    if closing == 0. {
        return;
    }
    let (pivot, axis, angle) = if NOSE.contains(&f.address) {
        ([0., 100.5, -16.], [1., 0., 0.], -165f64.to_radians())
    } else if LEFT_GEAR.contains(&f.address) {
        ([-6.5, 0., -12.5], [0., 1., 0.], std::f64::consts::PI)
    } else {
        ([6.5, 0., -12.5], [0., 1., 0.], -std::f64::consts::PI)
    };
    turn(f, pivot, axis, angle * closing);
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}
fn validate_gear(shape: &Shape) -> AppResult<()> {
    for sample in 0..=20 {
        let g = f64::from(sample) / 20.;
        let mut left = f32::NEG_INFINITY;
        let mut right = f32::INFINITY;
        for f in shape.faces.iter().filter(|f| gear(f.address)) {
            let mut q = f.clone();
            gear_pose(&mut q, g);
            for (i, p) in f.positions.iter().enumerate() {
                let b = q.positions[i];
                if !b.iter().all(|v| v.is_finite()) || g == 1. && *p != b {
                    return Err("B747.SH invalid rigid gear endpoint".into());
                }
                for (j, r) in f.positions.iter().enumerate().skip(i + 1) {
                    if (distance(*p, *r) - distance(b, q.positions[j])).abs() > EPS {
                        return Err("B747.SH gear card lost rigidity".into());
                    }
                }
                if p[2] <= -23. {
                    if LEFT_GEAR.contains(&f.address) {
                        left = left.max(b[0]);
                    } else if RIGHT_GEAR.contains(&f.address) {
                        right = right.min(b[0]);
                    }
                }
                if g == 0. {
                    let (lo, hi) = if NOSE.contains(&f.address) {
                        ([-3., 93.49, -16.91], [3., 106.78, -0.79])
                    } else if LEFT_GEAR.contains(&f.address) {
                        ([-11., -35., -15.], [-2., -1., 5.])
                    } else {
                        ([2., -35., -15.], [11., -1., 5.])
                    };
                    if (0..3).any(|k| b[k] < lo[k] - 0.001 || b[k] > hi[k] + 0.001) {
                        return Err("B747.SH rigid gear exceeds reviewed body stow bounds".into());
                    }
                }
            }
        }
        if right - left < 3.99 {
            return Err("B747.SH lower wheel cards crossed".into());
        }
    }
    Ok(())
}

fn normal(points: &[[f32; 3]]) -> Option<[f32; 3]> {
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
    let len = n.iter().map(|v| v * v).sum::<f64>().sqrt();
    (len > 1e-8).then(|| n.map(|v| (v / len) as f32))
}
fn update_normal(source: &Face, result: &mut Face) {
    let (Some(old), Some(reference), Some(mut n)) = (
        source.normal,
        normal(&source.positions),
        normal(&result.positions),
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
    fn panel(a: usize, p: Vec<[f32; 3]>, texture: &str, color: u8) -> Face {
        let uv = p.iter().map(|p| [p[0], p[1]]).collect();
        Face {
            address: a,
            colors: vec![color; p.len()],
            uv,
            positions: p,
            texture: texture.into(),
            subtype: 0xed,
            normal: Some([0., 1., 0.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    #[test]
    fn changing_flap_diagonal_preserves_both_surfaces_and_source_art() {
        let base = panel(
            0x640a,
            vec![[0., 0., 0.], [0., 1., 0.], [1., 1., 1.], [1., 0., 0.]],
            "NEUTRAL",
            17,
        );
        let mut a = panel(
            0x638a,
            vec![[1., 0., 0.], [0., 1., 0.], [1., 1., 1.]],
            "DEPLOYED",
            22,
        );
        let mut b = panel(
            0x63a5,
            vec![[1., 0., 0.], [0., 0., 0.], [0., 1., 0.]],
            "DEPLOYED",
            22,
        );
        for f in [&mut a, &mut b] {
            f.subtype = 0x64;
            for uv in &mut f.uv {
                uv[0] += 10.;
                uv[1] += 20.;
            }
        }
        let parts = right_lower(&base, &a, &b).unwrap();
        let mut prepared = Vec::new();
        let mut flaps = BTreeMap::new();
        for (n, d) in parts {
            register(&mut prepared, &mut flaps, n, d);
        }
        let rig = Rig { flaps };
        assert_eq!(prepared.len(), 4);
        for fraction in [0., 0.25, 0.5, 0.75, 1.] {
            let mut s = state();
            s.flaps = fraction;
            for n in &prepared {
                let f = rig.animate(n, &s).unwrap();
                assert_eq!(f.positions[2], [0.5, 0.5, 0.5 * (1. - fraction as f32)]);
                if fraction == 0. {
                    assert_eq!(f.positions, n.positions);
                    assert_eq!(f.uv, n.uv);
                    assert_eq!(f.texture, "NEUTRAL");
                    assert_eq!(f.colors, [17; 3]);
                } else {
                    assert_eq!(f.texture, "DEPLOYED");
                    assert_eq!(f.colors, [22; 3]);
                    assert_eq!(f.subtype, 0x64);
                    assert_eq!(f.uv[2], [10.5, 20.5]);
                }
            }
        }
    }
    #[test]
    fn canted_tail_axis_keeps_near_hinge_skin_aft_of_its_fixed_cut() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let source = panel(
            0x5651,
            vec![[37., -189., 15.], [24., -183., 14.], [35., -180., 15.]],
            "SYNTHETIC",
            17,
        );
        for amount in [-1., -0.5, 0., 0.5, 1.] {
            let mut s = state();
            s.elevator = amount;
            let result = rig.animate(&source, &s).unwrap();
            assert!(result.positions.iter().all(|p| tail_cut(*p) < 0.));
            if amount != 0. {
                assert!((result.positions[2][2] - 15.) * amount as f32 > 0.);
            } else {
                assert_eq!(result.positions, source.positions);
            }
        }
    }
    #[test]
    fn full_bogie_and_nose_cards_remain_rigid_at_201_positions() {
        let mut faces = Vec::new();
        for (side, addresses) in [
            (-1., [0x602e, 0x606c, 0x60aa, 0x60e8]),
            (1., [0x5d49, 0x5da6, 0x5d87, 0x5e03]),
        ] {
            for (a, y) in addresses.into_iter().zip([-5., -14., -23., -32.]) {
                faces.push(panel(
                    a,
                    vec![
                        [side * 4., y, -23.],
                        [side * 11., y, -23.],
                        [side * 11., y, -30.],
                        [side * 4., y, -30.],
                    ],
                    "SYNTHETIC",
                    17,
                ));
            }
        }
        faces.push(panel(
            0x616b,
            vec![
                [0., 104., -30.],
                [0., 104., -16.],
                [0., 94., -16.],
                [0., 94., -30.],
            ],
            "SYNTHETIC",
            17,
        ));
        faces.push(panel(
            0x61a9,
            vec![
                [-3., 100., -30.],
                [3., 100., -30.],
                [3., 100., -16.],
                [-3., 100., -16.],
            ],
            "SYNTHETIC",
            17,
        ));
        let shape = Shape {
            billboards: Vec::new(),
            faces,
            lines: Vec::new(),
            state_words: BTreeSet::new(),
        };
        validate_gear(&shape).unwrap();
        for step in 0..=200 {
            for f in &shape.faces {
                let mut q = f.clone();
                gear_pose(&mut q, f64::from(step) / 200.);
                for (i, p) in f.positions.iter().enumerate() {
                    for (j, r) in f.positions.iter().enumerate().skip(i + 1) {
                        assert!(
                            (distance(*p, *r) - distance(q.positions[i], q.positions[j])).abs()
                                < EPS
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn neutral_rudder_edge_is_exact_and_fixed_upper_fin_is_not_reassigned() {
        let rig = Rig {
            flaps: BTreeMap::new(),
        };
        let rudder = panel(
            0x62ec,
            vec![
                [0., -186., 58.],
                [0., -155., 15.],
                [0., -176., 15.],
                [0., -195., 58.],
            ],
            "SYNTHETIC",
            17,
        );
        let cap = panel(
            0x56ed,
            vec![
                [0., -195., 58.],
                [0., -197., 64.],
                [0., -182., 64.],
                [0., -186., 58.],
            ],
            "SYNTHETIC",
            17,
        );
        for amount in [-1., 0., 1.] {
            let mut s = state();
            s.rudder = amount;
            let f = rig.animate(&rudder, &s).unwrap();
            assert_eq!(f.positions[..2], rudder.positions[..2]);
            assert_eq!(rig.animate(&cap, &s).unwrap().positions, cap.positions);
            if amount == 0. {
                assert_eq!(f.positions, rudder.positions);
            } else {
                assert!(f.positions[2][0] * amount as f32 > 0.);
            }
        }
    }
}
