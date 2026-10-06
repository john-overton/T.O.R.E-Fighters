//! Independent exact F14.PT motion witnesses.
//! Body/flame references reuse only the pre-existing static f14_geometry repair.
//! Motion, shaft, alignment and endpoint checks do not call animation transforms.
use super::*;
use tore_formats::shape::Shape;
const LEFT: [usize; 4] = [0x55ca, 0x55e9, 0x5608, 0x5627];
const RIGHT: [usize; 4] = [0x5509, 0x5528, 0x5547, 0x5566];
const NOSE: [usize; 4] = [0x57a1, 0x57c0, 0x57df, 0x57fe];
const BRACE: [usize; 2] = [0x571e, 0x573d];
const PANEL: [usize; 2] = [0x56c3, 0x56da];
const BRAKE: [usize; 4] = [0x4c61, 0x4c78, 0x4c8f, 0x4ca6];
const HOOK: [usize; 2] = [0x5836, 0x584b];
const FLAME: [usize; 8] = [
    0x58c7, 0x58ee, 0x5915, 0x593c, 0x5963, 0x598a, 0x59b1, 0x59d8,
];
const FLAPS: [usize; 4] = [0x540d, 0x5434, 0x4fe9, 0x5010];
const FIN_LEFT: [usize; 2] = [0x4ac2, 0x4b19];
const FIN_RIGHT: [usize; 3] = [0x4a7b, 0x4a96, 0x4b66];
const TAIL_LEFT: [usize; 2] = [0x4828, 0x4888];
const TAIL_RIGHT: [usize; 2] = [0x49c5, 0x4a22];
pub(super) struct Sources {
    gear: Vec<Face>,
    brake: Vec<Face>,
    flame: Vec<Face>,
    body: Vec<Face>,
    down: BTreeMap<usize, Vec<[f32; 3]>>,
    painted: Vec<(usize, [f32; 2])>,
}
fn branch(bytes: &[u8], word: usize, ids: &[usize]) -> AppResult<Vec<Face>> {
    let result: Vec<_> = Shape::with_state(bytes, &[(word, 1)].into())?
        .faces
        .into_iter()
        .filter(|f| ids.contains(&f.address))
        .collect();
    if result.len() != ids.len() {
        return Err("F14 witness missing/duplicates branch faces".into());
    }
    Ok(result)
}
impl Sources {
    pub(super) fn load(bytes: &[u8], atlas_bytes: &[u8]) -> AppResult<Self> {
        let atlas = tore_formats::Pic::parse(atlas_bytes)?;
        if (atlas.width, atlas.height) != (256, 467) {
            return Err("unreviewed F14 witness atlas".into());
        }
        let ids: Vec<_> = LEFT
            .into_iter()
            .chain(RIGHT)
            .chain(NOSE)
            .chain(BRACE)
            .chain(PANEL)
            .collect();
        let gear = branch(bytes, 0x82ec, &ids)?;
        let brake = branch(bytes, 0x82e6, &BRAKE)?;
        let mut repaired = Shape::parse(bytes)?;
        repaired.faces.extend(branch(bytes, 0x82e0, &FLAME)?);
        crate::f14_geometry::repair(&mut repaired)?;
        let flame = repaired
            .faces
            .iter()
            .filter(|f| FLAME.contains(&f.address))
            .cloned()
            .collect();
        let body = repaired
            .faces
            .into_iter()
            .filter(|f| {
                !FLAME.contains(&f.address) && f.positions.iter().all(|p| p[0].abs() <= 10.)
            })
            .collect();
        let mut down = BTreeMap::new();
        for (word, bindings) in [
            (0x82fe, [(0x540d, 0x5486), (0x5434, 0x54a5)]),
            (0x8304, [(0x4fe9, 0x5062), (0x5010, 0x5081)]),
        ] {
            let shape = Shape::with_state(bytes, &[(word, -1)].into())?;
            for (a, b) in bindings {
                let f = shape
                    .faces
                    .iter()
                    .find(|f| f.address == b)
                    .ok_or("F14 witness source flap endpoint missing")?;
                // Explicit repaired reference: left alignment and existing wing lift.
                let positions = f
                    .positions
                    .iter()
                    .map(|p| [p[0] + if p[0] < 0. { -1. } else { 0. }, p[1], p[2] + 0.125])
                    .collect();
                down.insert(a, positions);
            }
        }
        let mut painted = Vec::new();
        for f in gear.iter().filter(|f| {
            LEFT.contains(&f.address)
                || RIGHT.contains(&f.address)
                || NOSE.contains(&f.address)
                || BRACE.contains(&f.address)
        }) {
            if f.uv.len() != f.positions.len() || f.uv.is_empty() {
                return Err("F14 witness painted gear UV layout".into());
            }
            let lo: [usize; 2] = std::array::from_fn(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::INFINITY, f32::min) as usize
            });
            let hi: [usize; 2] = std::array::from_fn(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::NEG_INFINITY, f32::max) as usize
            });
            if hi[0] >= atlas.width || hi[1] >= atlas.height {
                return Err("F14 witness UV outside original atlas".into());
            }
            for v in lo[1]..=hi[1] {
                for u in lo[0]..=hi[0] {
                    let i = (atlas.height - 1 - v) * atlas.width + u;
                    let uv = [u as f32, v as f32];
                    if atlas.mask[i] && atlas.pixels[i] != 255 && painted_world(f, uv).is_some() {
                        painted.push((f.address, uv));
                    }
                }
            }
        }
        if painted.is_empty() {
            return Err("F14 witness has no opaque gear samples".into());
        }
        Ok(Self {
            gear,
            brake,
            flame,
            body,
            down,
            painted,
        })
    }
}
pub(super) fn check(
    control: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    source: &Sources,
    m: &mut Metrics,
) {
    let (raw, reference, pose) = geometry;
    match control {
        Control::Elevator | Control::Aileron => {
            for (ids, side) in [(TAIL_LEFT.as_slice(), -1.), (TAIL_RIGHT.as_slice(), 1.)] {
                pin(reference, pose, ids, [side * 6., -10., 0.], scale, m);
                skins(reference, pose, ids, scale, m);
                rigid(reference, pose, ids, scale, m, false);
                if value == 0. {
                    coverage(reference, pose, ids, m);
                    let mut expected: Vec<_> = raw
                        .iter()
                        .filter(|f| ids.contains(&f.address))
                        .cloned()
                        .collect();
                    for f in &mut expected {
                        for p in &mut f.positions {
                            if side > 0. && p[0] == 6. && p[1] == -6. {
                                p[0] = 5.;
                            }
                        }
                    }
                    coverage(&expected, pose, ids, m);
                } else {
                    signed(
                        reference,
                        pose,
                        ids,
                        [side * 11., -16., 0.],
                        (
                            2,
                            if matches!(control, Control::Elevator) {
                                1.
                            } else {
                                side
                            },
                        ),
                        (value, scale),
                        m,
                    );
                }
            }
        }
        Control::Rudder => {
            for (ids, side) in [(FIN_LEFT.as_slice(), -1.), (FIN_RIGHT.as_slice(), 1.)] {
                for p in [
                    [side * 4., -11., 2.],
                    [side * 4., -13., 9.],
                    [side * 4., -6., 2.],
                ] {
                    pin(raw, pose, ids, p, scale, m);
                }
                skins(raw, pose, ids, scale, m);
                if value == 0. {
                    coverage(raw, pose, ids, m);
                } else {
                    signed(
                        raw,
                        pose,
                        ids,
                        [side * 4., -15., 9.],
                        (0, 1.),
                        (value, scale),
                        m,
                    );
                }
            }
        }
        Control::Flaps => {
            skins(reference, pose, &FLAPS, scale, m);
            for (ids, roots) in [
                (&FLAPS[..2], [[-23., -3., 1.125], [-5., -1., 1.125]]),
                (&FLAPS[2..], [[23., -3., 1.125], [5., -1., 1.125]]),
            ] {
                for p in roots {
                    pin(reference, pose, ids, p, scale, m);
                }
            }
            if value == 0. {
                coverage(reference, pose, &FLAPS, m);
                let mut expected: Vec<_> = raw
                    .iter()
                    .filter(|f| FLAPS.contains(&f.address))
                    .cloned()
                    .collect();
                for f in &mut expected {
                    for p in &mut f.positions {
                        if p[0] < 0. {
                            p[0] -= 1.;
                        }
                        p[2] += 0.125;
                    }
                }
                coverage(&expected, pose, &FLAPS, m);
            }
            for (a, expected) in &source.down {
                let before = reference.iter().find(|f| f.address == *a);
                let after = pose.iter().find(|f| f.address == *a);
                let (Some(before), Some(after)) = (before, after) else {
                    m.reviewed_anchor_missing = true;
                    continue;
                };
                m.reviewed_neutral_mismatch |= before.uv != after.uv
                    || before.colors != after.colors
                    || before.texture != after.texture
                    || before.subtype != after.subtype;
                for p in &before.positions {
                    let target = expected.iter().find(|q| p[..2] == q[..2]);
                    let actual = mapped(before, pose, *p);
                    m.reviewed_anchor_missing |= target.is_none() || actual.is_empty();
                    if let Some(q) = target {
                        let endpoint = std::array::from_fn(|i| {
                            p[i] + (q[i] - p[i]) * value.clamp(0., 1.) as f32
                        });
                        m.reviewed_neutral_mismatch |= actual
                            .iter()
                            .any(|r| distance(endpoint, *r) * scale > EPSILON);
                    }
                }
            }
            if value == 1. {
                for (a, expected) in &source.down {
                    m.reviewed_direction_failures +=
                        usize::from(!pose.iter().filter(|f| f.address == *a).any(|f| {
                            expected.iter().all(|p| {
                                f.positions
                                    .iter()
                                    .any(|q| distance(*p, *q) * scale <= EPSILON)
                            })
                        }));
                }
            }
            m.reviewed_direction_failures += usize::from(
                pose.iter()
                    .any(|f| [0x5486, 0x54a5, 0x5062, 0x5081].contains(&f.address)),
            );
        }
        Control::Sweep => {
            for (ids, side) in [(&FLAPS[..2], -1.), (&FLAPS[2..], 1.)] {
                // Fixed wing front attachment resides on neighboring source wing faces.
                let all: Vec<_> = reference
                    .iter()
                    .filter(|f| {
                        (0x4dba..=0x5434).contains(&f.address)
                            && f.positions.iter().map(|p| p[0]).sum::<f32>() * side > 0.
                    })
                    .map(|f| f.address)
                    .collect();
                pin(reference, pose, &all, [side * 8., 4., 1.125], scale, m);
                skins(reference, pose, &all, scale, m);
                rigid(reference, pose, &all, scale, m, false);
                if value == 0. {
                    coverage(reference, pose, &all, m);
                } else {
                    signed(
                        reference,
                        pose,
                        ids,
                        [side * 23., -4., 1.125],
                        (1, -1.),
                        (value, scale),
                        m,
                    );
                }
            }
        }
        Control::Gear => gear(reference, pose, value, scale, source, m),
        Control::Brake => {
            if value == 0. {
                m.reviewed_direction_failures +=
                    usize::from(pose.iter().any(|f| BRAKE.contains(&f.address)));
                return;
            }
            for ids in [&BRAKE[..2], &BRAKE[2..]] {
                let side = if ids[0] == 0x4c61 { 1. } else { -1. };
                for p in [[side * 2., -11., 1.], [0., -12., 2.]] {
                    pin(&source.brake, pose, ids, p, scale, m);
                }
                skins(&source.brake, pose, ids, scale, m);
                rigid(&source.brake, pose, ids, scale, m, false);
            }
            if value == 1. {
                coverage(&source.brake, pose, &BRAKE, m);
            }
            if value < 1e-5 {
                for f in pose.iter().filter(|f| BRAKE.contains(&f.address)) {
                    m.reviewed_direction_failures += usize::from(
                        f.positions
                            .iter()
                            .any(|p| !inside_sections(*p, &source.body)),
                    );
                }
            }
        }
        Control::Hook => {
            if value == 0. {
                m.reviewed_direction_failures +=
                    usize::from(pose.iter().any(|f| HOOK.contains(&f.address)));
                return;
            }
            pin(reference, pose, &HOOK, [0., -5., -2.], scale, m);
            skins(reference, pose, &HOOK, scale, m);
            rigid(reference, pose, &HOOK, scale, m, false);
            if value == 1. {
                coverage(reference, pose, &HOOK, m);
            } else {
                signed(
                    reference,
                    pose,
                    &HOOK,
                    [0., -12., -6.],
                    (2, 1.),
                    (1. - value, scale),
                    m,
                );
            }
        }
        Control::Exhaust => {
            if value == 0. {
                m.reviewed_direction_failures +=
                    usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
                return;
            }
            for f in &source.flame {
                for p in f.positions.iter().filter(|p| p[1] == -14.) {
                    pin(&source.flame, pose, &[f.address], *p, scale, m);
                }
            }
            skins(&source.flame, pose, &FLAME, scale, m);
            if value == 1. {
                coverage(&source.flame, pose, &FLAME, m);
            }
        }
        _ => {}
    }
}
fn gear(
    reference: &[Face],
    pose: &[Face],
    value: f64,
    scale: f32,
    source: &Sources,
    m: &mut Metrics,
) {
    let ids: Vec<_> = LEFT
        .into_iter()
        .chain(RIGHT)
        .chain(NOSE)
        .chain(BRACE)
        .chain(PANEL)
        .collect();
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| ids.contains(&f.address));
        return;
    }
    for (ids, root) in [
        (LEFT.as_slice(), [-6., 1., 0.]),
        (RIGHT.as_slice(), [6., 1., 0.]),
        (NOSE.as_slice(), [0., 17., -1.]),
        (BRACE.as_slice(), [0., 12., -1.]),
    ] {
        pin(&source.gear, pose, ids, root, scale, m);
        skins(&source.gear, pose, ids, scale, m);
        if ids != BRACE.as_slice() {
            rigid(reference, pose, ids, scale, m, true);
        }
    }
    for p in [[0., 17., -1.], [0., 21., -1.]] {
        pin(&source.gear, pose, &PANEL, p, scale, m);
    }
    skins(&source.gear, pose, &PANEL, scale, m);
    rigid(reference, pose, &PANEL, scale, m, false);
    let joint = [0., 16., -2.5];
    let brace: Vec<_> = source
        .gear
        .iter()
        .filter(|f| BRACE.contains(&f.address))
        .flat_map(|f| mapped(f, pose, joint))
        .collect();
    let wheel: Vec<_> = source
        .gear
        .iter()
        .filter(|f| NOSE.contains(&f.address))
        .flat_map(|f| mapped(f, pose, joint))
        .collect();
    m.reviewed_anchor_missing |= brace.is_empty() || wheel.is_empty();
    for a in &brace {
        for b in &wheel {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(distance(*a, *b) * scale);
        }
    }
    let left = pose
        .iter()
        .filter(|f| LEFT.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[0] * scale)
        .fold(f32::NEG_INFINITY, f32::max);
    let right = pose
        .iter()
        .filter(|f| RIGHT.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[0] * scale)
        .fold(f32::INFINITY, f32::min);
    m.reviewed_min_wheel_gap = Some(right - left);
    m.reviewed_wheel_failed |= left > -0.35 || right < 0.35 || right - left < 0.70;
    if value == 1. {
        coverage(&source.gear, pose, &ids, m);
    }
    if value < 1e-5 {
        for (a, uv) in &source.painted {
            m.reviewed_wheel_failed |= pose
                .iter()
                .find(|f| f.address == *a)
                .and_then(|f| painted_world(f, *uv))
                .is_none_or(|p| !inside_sections(p, &source.body));
        }
        for f in pose.iter().filter(|f| PANEL.contains(&f.address)) {
            m.reviewed_wheel_failed |= f
                .positions
                .iter()
                .any(|p| !inside_sections(*p, &source.body));
        }
    }
}

fn selected<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

fn raw_uv(f: &Face, p: [f32; 3]) -> Option<[f32; 2]> {
    if f.uv.len() != f.positions.len() || f.uv.is_empty() {
        return None;
    }
    for i in 0..f.positions.len() {
        let j = (i + 1) % f.positions.len();
        let a = f.positions[i];
        let b = f.positions[j];
        let edge: [f32; 3] = std::array::from_fn(|k| b[k] - a[k]);
        let norm = edge.iter().map(|v| v * v).sum::<f32>();
        if norm == 0. {
            continue;
        }
        let t = edge
            .iter()
            .enumerate()
            .map(|(k, v)| (p[k] - a[k]) * v)
            .sum::<f32>()
            / norm;
        if (-1e-5..=1.00001).contains(&t)
            && distance(p, std::array::from_fn(|k| a[k] + t * edge[k])) < 1e-3
        {
            return Some(std::array::from_fn(|k| {
                f.uv[i][k] + t * (f.uv[j][k] - f.uv[i][k])
            }));
        }
    }
    // Interior witnesses such as the nose brace's wheel connection.
    for i in 1..f.positions.len() - 1 {
        let a = f.positions[0];
        let b = f.positions[i];
        let c = f.positions[i + 1];
        let v: [f32; 3] = std::array::from_fn(|k| b[k] - a[k]);
        let w: [f32; 3] = std::array::from_fn(|k| c[k] - a[k]);
        let q: [f32; 3] = std::array::from_fn(|k| p[k] - a[k]);
        let dot = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
        let vv = dot(v, v);
        let ww = dot(w, w);
        let vw = dot(v, w);
        let den = vv * ww - vw * vw;
        if den.abs() < 1e-8 {
            continue;
        }
        let y = (ww * dot(q, v) - vw * dot(q, w)) / den;
        let z = (vv * dot(q, w) - vw * dot(q, v)) / den;
        let x = 1. - y - z;
        if x.min(y).min(z) >= -1e-5
            && distance(p, std::array::from_fn(|k| x * a[k] + y * b[k] + z * c[k])) < 1e-3
        {
            return Some(std::array::from_fn(|k| {
                x * f.uv[0][k] + y * f.uv[i][k] + z * f.uv[i + 1][k]
            }));
        }
    }
    None
}

fn mapped(f: &Face, pose: &[Face], p: [f32; 3]) -> Vec<[f32; 3]> {
    if let Some(uv) = raw_uv(f, p) {
        return pose
            .iter()
            .filter(|g| g.address == f.address)
            .filter_map(|g| painted_world(g, uv))
            .collect();
    }
    // Untextured, unsplit paired panels retain source vertex order.
    for i in 0..f.positions.len() {
        let j = (i + 1) % f.positions.len();
        let a = f.positions[i];
        let b = f.positions[j];
        let axis: [f32; 3] = std::array::from_fn(|k| b[k] - a[k]);
        let len = axis.iter().map(|v| v * v).sum::<f32>();
        if len == 0. {
            continue;
        }
        let t = axis
            .iter()
            .enumerate()
            .map(|(k, v)| (p[k] - a[k]) * v)
            .sum::<f32>()
            / len;
        if (0. ..=1.).contains(&t)
            && distance(p, std::array::from_fn(|k| a[k] + t * axis[k])) < 1e-3
        {
            return pose
                .iter()
                .filter(|g| g.address == f.address && g.positions.len() == f.positions.len())
                .map(|g| {
                    std::array::from_fn(|k| {
                        g.positions[i][k] + t * (g.positions[j][k] - g.positions[i][k])
                    })
                })
                .collect();
        }
    }
    // A stabilator's shaft can pass through its untextured interior. Keep
    // source vertex correspondence, rather than guessing a bounding-box edge.
    for i in 1..f.positions.len().saturating_sub(1) {
        if let Some(weights) = barycentric([f.positions[0], f.positions[i], f.positions[i + 1]], p)
        {
            return pose
                .iter()
                .filter(|g| g.address == f.address && g.positions.len() == f.positions.len())
                .map(|g| {
                    std::array::from_fn(|k| {
                        weights[0] * g.positions[0][k]
                            + weights[1] * g.positions[i][k]
                            + weights[2] * g.positions[i + 1][k]
                    })
                })
                .collect();
        }
    }
    Vec::new()
}

fn barycentric(triangle: [[f32; 3]; 3], p: [f32; 3]) -> Option<[f32; 3]> {
    let [a, b, c] = triangle;
    let v = std::array::from_fn(|k| b[k] - a[k]);
    let w = std::array::from_fn(|k| c[k] - a[k]);
    let q = std::array::from_fn(|k| p[k] - a[k]);
    let dot = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).map(|(a, b)| a * b).sum::<f32>();
    let vv = dot(v, v);
    let ww = dot(w, w);
    let vw = dot(v, w);
    let denominator = vv * ww - vw * vw;
    if denominator.abs() < 1e-8 {
        return None;
    }
    let y = (ww * dot(q, v) - vw * dot(q, w)) / denominator;
    let z = (vv * dot(q, w) - vw * dot(q, v)) / denominator;
    let weights = [1. - y - z, y, z];
    (weights.iter().all(|w| *w >= -1e-5)
        && distance(
            p,
            std::array::from_fn(|k| weights[0] * a[k] + weights[1] * b[k] + weights[2] * c[k]),
        ) < 1e-3)
        .then_some(weights)
}

fn pin(
    source: &[Face],
    pose: &[Face],
    ids: &[usize],
    point: [f32; 3],
    scale: f32,
    m: &mut Metrics,
) {
    let actual: Vec<_> = source
        .iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| mapped(f, pose, point))
        .collect();
    m.reviewed_anchor_missing |= actual.is_empty();
    for p in actual {
        m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(point, p) * scale);
    }
}

fn coverage(source: &[Face], pose: &[Face], ids: &[usize], m: &mut Metrics) {
    m.reviewed_anchor_missing |= ids
        .iter()
        .any(|a| !source.iter().any(|f| f.address == *a) || !pose.iter().any(|f| f.address == *a));
    for f in source.iter().filter(|f| ids.contains(&f.address)) {
        for p in &f.positions {
            m.reviewed_neutral_mismatch |= !pose
                .iter()
                .filter(|g| g.address == f.address)
                .any(|g| g.positions.iter().any(|q| distance(*p, *q) <= EPSILON));
        }
    }
}

fn skins(source: &[Face], pose: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let mut groups: BTreeMap<[i32; 3], Vec<[f32; 3]>> = BTreeMap::new();
    for f in source.iter().filter(|f| ids.contains(&f.address)) {
        for p in &f.positions {
            groups
                .entry(p.map(|v| (v * 10000.).round() as i32))
                .or_default()
                .extend(mapped(f, pose, *p));
        }
    }
    for points in groups.values() {
        for a in points {
            for b in points {
                m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(distance(*a, *b) * scale);
            }
        }
    }
}

fn signed(
    source: &[Face],
    pose: &[Face],
    ids: &[usize],
    point: [f32; 3],
    direction: (usize, f32),
    travel: (f64, f32),
    m: &mut Metrics,
) {
    let (axis, sign) = direction;
    let (value, scale) = travel;
    let actual: Vec<_> = source
        .iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| mapped(f, pose, point))
        .collect();
    m.reviewed_direction_failures += usize::from(actual.is_empty());
    for p in actual {
        m.reviewed_direction_failures +=
            usize::from((p[axis] - point[axis]) * scale * sign * value.signum() as f32 <= EPSILON);
    }
}

fn rigid(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    scale: f32,
    m: &mut Metrics,
    wheel: bool,
) {
    let before = selected(reference, ids);
    let after = selected(pose, ids);
    let mut old: Vec<[f32; 3]> = Vec::new();
    let mut actual: Vec<[f32; 3]> = Vec::new();
    let mut failed = before.len() != ids.len();
    for (key, f) in before {
        let Some(g) = after.get(&key) else {
            failed = true;
            continue;
        };
        if f.positions.len() != g.positions.len() {
            failed = true;
            continue;
        }
        old.extend(f.positions.iter().copied());
        actual.extend(g.positions.iter().copied());
    }
    let mut error = 0f32;
    for i in 0..old.len() {
        for j in i + 1..old.len() {
            error = error
                .max((distance(old[i], old[j]) - distance(actual[i], actual[j])).abs() * scale);
        }
    }
    if wheel {
        m.reviewed_wheel_failed |= failed || error > EPSILON;
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error);
    } else {
        m.reviewed_anchor_missing |= failed;
        m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error);
    }
}

fn painted_world(face: &Face, uv: [f32; 2]) -> Option<[f32; 3]> {
    if face.uv.len() != face.positions.len() || face.uv.len() < 3 {
        return None;
    }
    for i in 1..face.uv.len() - 1 {
        let (a, b, c) = (face.uv[0], face.uv[i], face.uv[i + 1]);
        let den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
        if den.abs() < 1e-9 {
            continue;
        }
        let x = ((b[1] - c[1]) * (uv[0] - c[0]) + (c[0] - b[0]) * (uv[1] - c[1])) / den;
        let y = ((c[1] - a[1]) * (uv[0] - c[0]) + (a[0] - c[0]) * (uv[1] - c[1])) / den;
        let z = 1. - x - y;
        if x.min(y).min(z) >= -1e-6 {
            return Some(std::array::from_fn(|k| {
                x * face.positions[0][k] + y * face.positions[i][k] + z * face.positions[i + 1][k]
            }));
        }
    }
    None
}

fn orient(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f32 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}

fn hull(mut points: Vec<[f32; 2]>) -> Vec<[f32; 2]> {
    points.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    points.dedup();
    let chain = |iter: Vec<[f32; 2]>| {
        let mut result = Vec::new();
        for p in iter {
            while result.len() > 1
                && orient(result[result.len() - 2], result[result.len() - 1], p) <= 0.
            {
                result.pop();
            }
            result.push(p);
        }
        result.pop();
        result
    };
    let mut lower = chain(points.clone());
    points.reverse();
    lower.extend(chain(points));
    lower
}

fn inside_sections(point: [f32; 3], faces: &[Face]) -> bool {
    let mut points = Vec::new();
    for f in faces {
        for (a, b) in f
            .positions
            .iter()
            .zip(f.positions.iter().cycle().skip(1))
            .take(f.positions.len())
        {
            if a[1] != b[1] && a[1].min(b[1]) <= point[1] && point[1] <= a[1].max(b[1]) {
                let t = (point[1] - a[1]) / (b[1] - a[1]);
                points.push([a[0] + (b[0] - a[0]) * t, a[2] + (b[2] - a[2]) * t]);
            }
        }
    }
    let boundary = hull(points);
    boundary.len() >= 3
        && boundary
            .iter()
            .zip(boundary.iter().cycle().skip(1))
            .take(boundary.len())
            .all(|(a, b)| orient(*a, *b, [point[0], point[2]]) >= -1e-3)
}

// Observe a rigid wing frame from three source/posed corners. No fitted
// rotation schedule or production transform is used for point correspondence.
fn observed_point(before: &Face, after: &Face, point: [f32; 3]) -> Option<[f32; 3]> {
    if before.positions.len() != after.positions.len() {
        return None;
    }
    type V = [f64; 3];
    let sub = |a: V, b: V| std::array::from_fn(|k| a[k] - b[k]);
    let dot = |a: V, b: V| a.iter().zip(b).map(|(a, b)| a * b).sum::<f64>();
    let cross = |a: V, b: V| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let unit = |v: V| {
        let length = dot(v, v).sqrt();
        (length > 1e-9).then(|| v.map(|x| x / length))
    };
    let origin = before.positions.first()?.map(f64::from);
    let i = (1..before.positions.len()).find(|i| {
        dot(
            sub(before.positions[*i].map(f64::from), origin),
            sub(before.positions[*i].map(f64::from), origin),
        ) > 1e-9
    })?;
    let j = (1..before.positions.len()).find(|j| {
        dot(
            cross(
                sub(before.positions[i].map(f64::from), origin),
                sub(before.positions[*j].map(f64::from), origin),
            ),
            cross(
                sub(before.positions[i].map(f64::from), origin),
                sub(before.positions[*j].map(f64::from), origin),
            ),
        ) > 1e-9
    })?;
    let basis = |f: &Face| -> Option<[V; 3]> {
        let o = f.positions[0].map(f64::from);
        let x = unit(sub(f.positions[i].map(f64::from), o))?;
        let z = unit(cross(x, sub(f.positions[j].map(f64::from), o)))?;
        Some([x, cross(z, x), z])
    };
    let a = basis(before)?;
    let b = basis(after)?;
    let v = sub(point.map(f64::from), origin);
    Some(std::array::from_fn(|k| {
        (after.positions[0][k] as f64
            + (0..3).map(|axis| dot(v, a[axis]) * b[axis][k]).sum::<f64>()) as f32
    }))
}
fn wing_observer<'a>(
    reference: &'a [Face],
    pose: &'a [Face],
    left: bool,
) -> Option<(&'a Face, &'a Face)> {
    let address = if left { 0x5269 } else { 0x4de1 };
    Some((
        reference.iter().find(|f| f.address == address)?,
        pose.iter().find(|f| f.address == address)?,
    ))
}
fn combined_wings(
    reference: &[Face],
    pose: &[Face],
    flap: f64,
    scale: f32,
    source: &Sources,
    m: &mut Metrics,
) {
    for (ids, side) in [(&FLAPS[..2], -1.), (&FLAPS[2..], 1.)] {
        let Some((before, after)) = wing_observer(reference, pose, side < 0.) else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        let root = [side * 8., 4., 1.125];
        if let Some(p) = observed_point(before, after, root) {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(p, root) * scale);
        } else {
            m.reviewed_anchor_missing = true;
        }
        let all: Vec<_> = reference
            .iter()
            .filter(|f| {
                (0x4dba..=0x5434).contains(&f.address)
                    && !FLAPS.contains(&f.address)
                    && f.positions.iter().map(|p| p[0]).sum::<f32>() * side > 0.
            })
            .map(|f| f.address)
            .collect();
        rigid(reference, pose, &all, scale, m, false);
        skins(reference, pose, &all, scale, m);
        skins(reference, pose, ids, scale, m);
        for f in reference.iter().filter(|f| all.contains(&f.address)) {
            for p in &f.positions {
                let actual = mapped(f, pose, *p);
                let expected = observed_point(before, after, *p);
                m.reviewed_anchor_missing |= actual.is_empty() || expected.is_none();
                if let Some(expected) = expected {
                    m.reviewed_neutral_mismatch |= actual
                        .iter()
                        .any(|q| distance(*q, expected) * scale > EPSILON);
                }
            }
        }
        for f in reference.iter().filter(|f| ids.contains(&f.address)) {
            let Some(target) = source.down.get(&f.address) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            let actual: Vec<_> = pose.iter().filter(|g| g.address == f.address).collect();
            m.reviewed_anchor_missing |= actual.len() != 1;
            if let Some(g) = actual.first() {
                m.reviewed_neutral_mismatch |= g.uv != f.uv
                    || g.colors != f.colors
                    || g.texture != f.texture
                    || g.subtype != f.subtype;
            }
            for p in &f.positions {
                let Some(q) = target.iter().find(|q| q[..2] == p[..2]) else {
                    m.reviewed_anchor_missing = true;
                    continue;
                };
                let local = std::array::from_fn(|k| p[k] + (q[k] - p[k]) * flap as f32);
                let Some(expected) = observed_point(before, after, local) else {
                    m.reviewed_anchor_missing = true;
                    continue;
                };
                let actual = mapped(f, pose, *p);
                m.reviewed_anchor_missing |= actual.is_empty();
                m.reviewed_neutral_mismatch |= actual
                    .iter()
                    .any(|q| distance(*q, expected) * scale > EPSILON);
            }
        }
    }
}
fn combined_tail(
    reference: &[Face],
    pose: &[Face],
    pitch: f64,
    roll: f64,
    scale: f32,
    m: &mut Metrics,
) {
    for (ids, side) in [(TAIL_LEFT.as_slice(), -1.), (TAIL_RIGHT.as_slice(), 1.)] {
        pin(reference, pose, ids, [side * 6., -10., 0.], scale, m);
        skins(reference, pose, ids, scale, m);
        rigid(reference, pose, ids, scale, m, false);
        let witness = [side * 11., -16., 0.];
        let actual: Vec<_> = reference
            .iter()
            .filter(|f| ids.contains(&f.address))
            .flat_map(|f| mapped(f, pose, witness))
            .collect();
        m.reviewed_anchor_missing |= actual.is_empty();
        let expected = -0.30 * pitch - f64::from(side) * 0.20 * roll;
        for p in actual {
            let mut angle = f64::from(p[2]).atan2(f64::from(p[1] + 10.)) - std::f64::consts::PI;
            if angle < -std::f64::consts::PI {
                angle += std::f64::consts::TAU;
            }
            m.reviewed_direction_failures +=
                usize::from((angle - expected).abs() > 1e-5 || angle.abs() > 0.5 + 1e-5);
        }
    }
}
fn combined_failed(m: &Metrics) -> bool {
    !m.finite
        || m.reviewed_anchor_missing
        || m.max_reviewed_anchor_gap > EPSILON
        || m.max_reviewed_skin_gap > EPSILON
        || m.reviewed_rigid_panel_error > EPSILON
        || m.reviewed_neutral_mismatch
        || m.reviewed_direction_failures != 0
        || !m.new_planar_crossings.is_empty()
}
pub(super) fn combinations(
    source: &Sources,
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut poses = Vec::new();
    let mut csv = String::from(
        "knots,flap,roll,anchor_gap_ft,skin_gap_ft,rigidity_error_ft,crossings,direction_failures,endpoint_material_passed\n",
    );
    for knots in [400., 475., 550., 625., 700.] {
        for flap in [0., 0.25, 0.5, 0.75, 1.] {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                let mut state = neutral.clone();
                state.speed = knots * 1.68781;
                state.flaps = flap;
                state.aileron = roll;
                let pose = airframe.animation_faces(&state);
                let mut m = measure(&original, &pose, scale);
                combined_wings(&original, &pose, flap, scale, source, &mut m);
                combined_tail(&original, &pose, 0., roll, scale, &mut m);
                writeln!(
                    csv,
                    "{knots},{flap},{roll},{},{},{},{},{},{}",
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.reviewed_rigid_panel_error,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures,
                    !m.reviewed_neutral_mismatch
                )?;
                if combined_failed(&m) {
                    failures.push(format!("F14 speed{knots}/flap{flap}/roll{roll} wing/tail attachments or source endpoints"));
                }
                poses.push(pose);
            }
        }
    }
    fs::write(out.join("sweep-flap-roll-combinations.csv"), csv)?;
    contact_sheet(
        &out.join("sweep-flap-roll-combinations.ppm"),
        &original,
        &poses,
    )?;
    poses.clear();
    let mut csv = String::from(
        "pitch,roll,anchor_gap_ft,skin_gap_ft,rigidity_error_ft,crossings,direction_failures\n",
    );
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.aileron = roll;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            combined_tail(&original, &pose, pitch, roll, scale, &mut m);
            writeln!(
                csv,
                "{pitch},{roll},{},{},{},{},{}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if combined_failed(&m) {
                failures.push(format!(
                    "F14 pitch{pitch}/roll{roll} tail shaft, rigidity or combined angle"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("pitch-roll-combinations.csv"), csv)?;
    contact_sheet(&out.join("pitch-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn face(address: usize, points: Vec<[f32; 3]>) -> Face {
        let mut f = super::super::tests::face(address, points);
        f.uv.clear();
        f
    }
    fn empty_sources() -> Sources {
        Sources {
            gear: Vec::new(),
            brake: Vec::new(),
            flame: Vec::new(),
            body: Vec::new(),
            down: BTreeMap::new(),
            painted: Vec::new(),
        }
    }
    fn tails() -> Vec<Face> {
        [(TAIL_LEFT, -1.), (TAIL_RIGHT, 1.)]
            .into_iter()
            .flat_map(|(ids, side)| {
                let points = vec![
                    [side * 5., -6., 0.],
                    [side * 12., -6., 0.],
                    [side * 11., -16., 0.],
                    [side * 5., -16., 0.],
                ];
                ids.into_iter().map(move |a| face(a, points.clone()))
            })
            .collect()
    }
    // Deliberate quarter-turn fixtures, independent of the production schedule.
    fn posed_tails(reference: &[Face], roll: bool, reverse: bool) -> Vec<Face> {
        reference
            .iter()
            .map(|f| {
                let mut g = f.clone();
                let side = f.positions[0][0].signum();
                let sign = if roll { side } else { 1. } * if reverse { -1. } else { 1. };
                for p in &mut g.positions {
                    p[2] = -(p[1] + 10.) * sign;
                    p[1] = -10.;
                }
                g
            })
            .collect()
    }
    #[test]
    fn untextured_interior_shaft_is_fixed_but_translation_is_rejected() {
        let before = tails();
        let mut after = posed_tails(&before, false, false);
        let mut m = Metrics::default();
        pin(
            &before,
            &after,
            &TAIL_LEFT,
            [-6., -10., 0.],
            4. / 3.,
            &mut m,
        );
        assert!(!m.reviewed_anchor_missing);
        assert!(m.max_reviewed_anchor_gap <= EPSILON);
        for f in &mut after {
            for p in &mut f.positions {
                p[2] += 0.01;
            }
        }
        pin(
            &before,
            &after,
            &TAIL_LEFT,
            [-6., -10., 0.],
            4. / 3.,
            &mut m,
        );
        assert!(m.max_reviewed_anchor_gap > EPSILON);
        assert!(barycentric([[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]], [0.2, 0.2, 1.]).is_none());
    }
    #[test]
    fn primary_witness_accepts_pitch_and_differential_roll_but_rejects_reversal() {
        let before = tails();
        for (c, roll) in [(Control::Elevator, false), (Control::Aileron, true)] {
            for reverse in [false, true] {
                let after = posed_tails(&before, roll, reverse);
                let mut m = Metrics::default();
                check(
                    c,
                    1.,
                    (&before, &before, &after),
                    4. / 3.,
                    &empty_sources(),
                    &mut m,
                );
                assert!(!m.reviewed_anchor_missing);
                assert!(m.max_reviewed_anchor_gap <= EPSILON);
                assert!(m.reviewed_rigid_panel_error <= EPSILON);
                assert_eq!(m.reviewed_direction_failures > 0, reverse);
            }
        }
    }
    #[test]
    fn split_textured_hinge_checks_every_piece_and_detects_a_single_torn_skin() {
        let mut source = face(
            FIN_LEFT[0],
            vec![[0., 0., 0.], [2., 0., 0.], [2., 2., 0.], [0., 2., 0.]],
        );
        source.uv = vec![[0., 0.], [2., 0.], [2., 2.], [0., 2.]];
        let mut first = source.clone();
        first.positions = vec![[0., 0., 0.], [1., 0., 0.], [1., 2., 0.], [0., 2., 0.]];
        first.uv = vec![[0., 0.], [1., 0.], [1., 2.], [0., 2.]];
        let mut second = source.clone();
        second.positions = vec![[1., 0., 0.], [2., 0., 0.], [2., 2., 0.], [1., 2., 0.]];
        second.uv = vec![[1., 0.], [2., 0.], [2., 2.], [1., 2.]];
        let mut after = vec![first, second];
        let mut m = Metrics::default();
        pin(
            std::slice::from_ref(&source),
            &after,
            &FIN_LEFT[..1],
            [1., 1., 0.],
            1.,
            &mut m,
        );
        assert!(!m.reviewed_anchor_missing && m.max_reviewed_anchor_gap == 0.);
        for p in &mut after[1].positions {
            p[2] += 0.05;
        }
        pin(&[source], &after, &FIN_LEFT[..1], [1., 1., 0.], 1., &mut m);
        assert!(m.max_reviewed_anchor_gap > EPSILON);
    }
    #[test]
    fn opposite_skin_and_whole_wheel_distortion_are_rejected() {
        let before = tails();
        let mut after = before.clone();
        after[1].positions[2][2] = 0.1;
        let mut m = Metrics::default();
        skins(&before, &after, &TAIL_LEFT, 4. / 3., &mut m);
        assert!(m.max_reviewed_skin_gap > EPSILON);
        rigid(&before, &after, &TAIL_LEFT, 4. / 3., &mut m, true);
        assert!(m.reviewed_wheel_failed);
        assert!(m.reviewed_wheel_rigidity_error > EPSILON);
    }
    #[test]
    fn painted_point_uses_atlas_barycentrics_and_all_split_pieces() {
        let mut f = face(NOSE[0], vec![[0., 0., 0.], [4., 0., 0.], [0., 4., 0.]]);
        f.uv = vec![[10., 10.], [14., 10.], [10., 14.]];
        assert_eq!(painted_world(&f, [11., 11.]), Some([1., 1., 0.]));
        assert_eq!(painted_world(&f, [15., 15.]), None);
        let uv = raw_uv(&f, [1., 1., 0.]).unwrap();
        assert_eq!(uv, [11., 11.]);
        let mut shifted = f.clone();
        for p in &mut shifted.positions {
            p[2] += 2.;
        }
        let points = mapped(&f, &[f.clone(), shifted], [1., 1., 0.]);
        assert_eq!(points, [[1., 1., 0.], [1., 1., 2.]]);
    }
    #[test]
    fn stow_section_rejects_outside_and_missing_body() {
        let body = vec![
            face(
                1,
                vec![[-2., 0., -2.], [-2., 4., -2.], [2., 4., -2.], [2., 0., -2.]],
            ),
            face(
                2,
                vec![[-2., 0., 2.], [-2., 4., 2.], [2., 4., 2.], [2., 0., 2.]],
            ),
        ];
        assert!(inside_sections([0., 2., 0.], &body));
        assert!(inside_sections([2., 2., 2.], &body));
        assert!(!inside_sections([2.01, 2., 0.], &body));
        assert!(!inside_sections([0., 5., 0.], &body));
        assert!(!inside_sections([0., 2., 0.], &[]));
    }
    #[test]
    fn missing_paired_wheel_faces_do_not_pass_rigidity() {
        let before = tails();
        let mut m = Metrics::default();
        rigid(&before, &before[..1], &TAIL_LEFT, 1., &mut m, true);
        assert!(m.reviewed_wheel_failed);
    }
    #[test]
    fn source_flap_interpolation_rejects_whole_panel_droop_and_art_changes() {
        let before: Vec<_> = [(FLAPS[..2].to_vec(), -1.), (FLAPS[2..].to_vec(), 1.)]
            .into_iter()
            .flat_map(|(ids, side)| {
                ids.into_iter().map(move |a| {
                    let mut f = face(
                        a,
                        vec![
                            [side * 23., -4., 1.125],
                            [side * 23., -3., 1.125],
                            [side * 5., -1., 1.125],
                            [side * 5., -3., 1.125],
                        ],
                    );
                    f.uv = vec![[0., 0.], [0., 4.], [20., 4.], [20., 0.]];
                    f
                })
            })
            .collect();
        let mut source = empty_sources();
        for f in &before {
            let mut points = f.positions.clone();
            points[3][2] -= 1.;
            source.down.insert(f.address, points);
        }
        let mut after = before.clone();
        for f in &mut after {
            f.positions[3][2] -= 0.5;
        }
        let mut m = Metrics::default();
        check(
            Control::Flaps,
            0.5,
            (&before, &before, &after),
            4. / 3.,
            &source,
            &mut m,
        );
        assert!(!m.reviewed_anchor_missing && !m.reviewed_neutral_mismatch);
        assert!(m.max_reviewed_anchor_gap <= EPSILON && m.max_reviewed_skin_gap <= EPSILON);
        for f in &mut after {
            f.positions[0][2] -= 0.5;
        }
        let mut m = Metrics::default();
        check(
            Control::Flaps,
            0.5,
            (&before, &before, &after),
            4. / 3.,
            &source,
            &mut m,
        );
        assert!(m.reviewed_neutral_mismatch);
        after = before.clone();
        for f in &mut after {
            f.positions[3][2] -= 0.5;
        }
        after[0].uv[0][0] += 1.;
        let mut m = Metrics::default();
        check(
            Control::Flaps,
            0.5,
            (&before, &before, &after),
            4. / 3.,
            &source,
            &mut m,
        );
        assert!(m.reviewed_neutral_mismatch);
    }
    #[test]
    fn observed_wing_frame_handles_duplicate_corners_and_rejects_degeneration() {
        let before = face(
            0x5269,
            vec![[2., 0., 1.], [2., 0., 1.], [0., 0., 1.], [0., 2., 1.]],
        );
        let mut after = before.clone();
        for p in &mut after.positions {
            *p = [3. - p[1], 4. + p[0], p[2]];
        }
        let q = observed_point(&before, &after, [1., 1., 0.5]).unwrap();
        assert!(distance(q, [2., 5., 0.5]) < EPSILON);
        let mut collapsed = after.clone();
        collapsed.positions.fill([0.; 3]);
        assert!(observed_point(&before, &collapsed, [1., 1., 0.5]).is_none());
    }
    #[test]
    fn mixed_tail_witness_accepts_half_radian_limit_and_rejects_wrong_sum() {
        let reference = tails();
        for (pitch, roll, left, right) in [
            (1., 1., -0.1, -0.5),
            (1., -1., -0.5, -0.1),
            (-1., 1., 0.5, 0.1),
            (-1., -1., 0.1, 0.5),
        ] {
            let mut pose = reference.clone();
            for f in &mut pose {
                let angle: f64 = if f.positions[0][0] < 0. { left } else { right };
                let (sin, cos) = angle.sin_cos();
                for p in &mut f.positions {
                    let y = f64::from(p[1] + 10.);
                    p[1] = (-10. + y * cos) as f32;
                    p[2] = (y * sin) as f32;
                }
            }
            let mut m = measure(&reference, &pose, 4. / 3.);
            combined_tail(&reference, &pose, pitch, roll, 4. / 3., &mut m);
            assert!(!combined_failed(&m));
            for f in &mut pose {
                for p in &mut f.positions {
                    p[2] = -p[2];
                }
            }
            let mut m = measure(&reference, &pose, 4. / 3.);
            combined_tail(&reference, &pose, pitch, roll, 4. / 3., &mut m);
            assert!(m.reviewed_direction_failures > 0);
        }
    }
    #[test]
    fn combined_gate_rejects_nonfinite_geometry_even_with_zero_reviewed_gaps() {
        let reference = vec![face(1, vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]])];
        assert!(!combined_failed(&measure(&reference, &reference, 1.)));
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut pose = reference.clone();
            pose[0].positions[1][2] = invalid;
            let m = measure(&reference, &pose, 1.);
            assert!(!m.finite);
            // NaN distances need not increase these maxima. The finite flag
            // itself must make the coupled pose fail.
            assert_eq!(m.max_reviewed_anchor_gap, 0.);
            assert_eq!(m.max_reviewed_skin_gap, 0.);
            assert!(combined_failed(&m));
        }
        let mut pose = reference.clone();
        pose[0].normal = Some([f32::NAN, 0., 1.]);
        assert!(combined_failed(&measure(&reference, &pose, 1.)));
    }
}
