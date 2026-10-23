//! Independent exact F18.PT source witnesses, not copied motion formulas.
//! Retained main stow and telescoping brace are explicitly fitted agent choices.
use super::*;
use tore_formats::shape::Shape;
const LEFT: [usize; 6] = [0x4d33, 0x4d60, 0x4d8d, 0x4dac, 0x4dcb, 0x4dea];
const RIGHT: [usize; 6] = [0x4bfa, 0x4c27, 0x4c54, 0x4c73, 0x4c92, 0x4cb1];
const NOSE: [usize; 4] = [0x4f64, 0x4f83, 0x4fa2, 0x4fc1];
const BRACE: [usize; 2] = [0x4ee1, 0x4f00];
const DOORS: [usize; 6] = [0x4b69, 0x4b80, 0x4abe, 0x4ad5, 0x4e86, 0x4e9d];
const FIN: [usize; 4] = [0x5467, 0x548e, 0x54d4, 0x54fb];
const TAIL_LEFT: [usize; 4] = [0x449f, 0x453c, 0x4563, 0x458a];
const TAIL_RIGHT: [usize; 4] = [0x486b, 0x4894, 0x4992, 0x49b9];
const FLAP_LEFT: [usize; 2] = [0x525b, 0x5282];
const FLAP_RIGHT: [usize; 2] = [0x5154, 0x517b];
const BRAKE: [usize; 2] = [0x5059, 0x5080];
const HOOK: [usize; 2] = [0x4a03, 0x4a22];
const FLAME: [usize; 8] = [
    0x5310, 0x5337, 0x535e, 0x5385, 0x53ac, 0x53d3, 0x53fa, 0x5421,
];
pub(super) struct Sources {
    gear: Vec<Face>,
    brake: Vec<Face>,
    hook: Vec<Face>,
    flame: Vec<Face>,
    body: Vec<Face>,
    painted: Vec<(usize, [f32; 2])>,
}
fn branch(bytes: &[u8], word: usize, ids: &[usize]) -> AppResult<Vec<Face>> {
    let result: Vec<_> = Shape::with_state(bytes, &[(word, 1)].into())?
        .faces
        .into_iter()
        .filter(|f| ids.contains(&f.address))
        .collect();
    if result.len() != ids.len() {
        return Err("F18 witness branch missing/duplicates source faces".into());
    }
    Ok(result)
}
impl Sources {
    pub(super) fn load(bytes: &[u8], atlas_bytes: &[u8]) -> AppResult<Self> {
        let atlas = tore_formats::Pic::parse(atlas_bytes)?;
        if (atlas.width, atlas.height) != (256, 644) {
            return Err("unreviewed F18 witness atlas dimensions".into());
        }
        let ids: Vec<_> = LEFT
            .into_iter()
            .chain(RIGHT)
            .chain(NOSE)
            .chain(BRACE)
            .chain(DOORS)
            .collect();
        let gear = branch(bytes, 0x7912, &ids)?;
        let brake = branch(bytes, 0x790c, &BRAKE)?;
        let hook = branch(bytes, 0x791e, &HOOK)?;
        let flame = branch(bytes, 0x7900, &FLAME)?;
        let body = Shape::parse(bytes)?
            .faces
            .into_iter()
            .filter(|f| f.positions.iter().all(|p| p[0].abs() <= 14.))
            .collect();
        let mut painted = Vec::new();
        // Lower main tire side sheets, all nose-wheel/strut and brace artwork.
        // The retained upper main strut joint is deliberately not hidden.
        for f in gear.iter().filter(|f| {
            [0x4c54, 0x4c73, 0x4d8d, 0x4dac].contains(&f.address)
                || NOSE.contains(&f.address)
                || BRACE.contains(&f.address)
        }) {
            if f.uv.len() != f.positions.len() || f.uv.is_empty() {
                return Err("F18 witness painted gear UV layout".into());
            }
            let lo: [usize; 2] = std::array::from_fn(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::INFINITY, f32::min) as usize
            });
            let hi: [usize; 2] = std::array::from_fn(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::NEG_INFINITY, f32::max) as usize
            });
            if hi[0] >= atlas.width || hi[1] >= atlas.height {
                return Err("F18 witness UV outside original atlas".into());
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
            return Err("F18 witness original atlas has no gear samples".into());
        }
        Ok(Self {
            gear,
            brake,
            hook,
            flame,
            body,
            painted,
        })
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
    Vec::new()
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
            for (ids, point, shaft, rollsign) in [
                (
                    TAIL_LEFT.as_slice(),
                    [-34., -66., -1.],
                    [-10.448276, -43., 0.],
                    -1.,
                ),
                (
                    TAIL_RIGHT.as_slice(),
                    [34., -66., -1.],
                    [10.448276, -43., 0.],
                    1.,
                ),
            ] {
                pin(raw, pose, ids, shaft, scale, m);
                skins(raw, pose, ids, scale, m);
                if value == 0. {
                    coverage(raw, pose, ids, m);
                } else {
                    signed(
                        raw,
                        pose,
                        ids,
                        point,
                        (
                            2,
                            if matches!(control, Control::Elevator) {
                                1.
                            } else {
                                rollsign
                            },
                        ),
                        (value, scale),
                        m,
                    );
                }
            }
        }
        Control::Flaps => {
            for (ids, roots, trailing) in [
                (
                    FLAP_LEFT.as_slice(),
                    [
                        [-13., -8., 4.],
                        [-38., -10., 2.],
                        [-13., -8., 3.],
                        [-38., -10., 1.],
                    ],
                    [-14., -20., 3.],
                ),
                (
                    FLAP_RIGHT.as_slice(),
                    [
                        [13., -7., 4.],
                        [39., -10., 2.],
                        [13., -7., 3.],
                        [39., -10., 1.],
                    ],
                    [14., -20., 3.],
                ),
            ] {
                for root in roots {
                    pin(raw, pose, ids, root, scale, m);
                }
                skins(raw, pose, ids, scale, m);
                if value == 0. {
                    coverage(raw, pose, ids, m);
                } else {
                    signed(raw, pose, ids, trailing, (2, -1.), (value, scale), m);
                }
            }
        }
        Control::Rudder => {
            for (ids, roots, front, trailing) in [
                (
                    &FIN[..2],
                    [[8., -32., 4.], [20., -39., 30.]],
                    [[8., -12., 4.], [20., -32., 30.]],
                    [8., -40., 4.],
                ),
                (
                    &FIN[2..],
                    [[-8., -32., 4.], [-19., -39., 30.]],
                    [[-8., -12., 4.], [-19., -32., 30.]],
                    [-8., -40., 4.],
                ),
            ] {
                for point in roots.into_iter().chain(front) {
                    pin(raw, pose, ids, point, scale, m);
                }
                skins(raw, pose, ids, scale, m);
                if value == 0. {
                    coverage(raw, pose, ids, m);
                } else {
                    signed(raw, pose, ids, trailing, (0, 1.), (value, scale), m);
                }
            }
        }
        Control::Brake | Control::Hook => {
            let (faces, ids) = if matches!(control, Control::Brake) {
                (&source.brake, BRAKE.as_slice())
            } else {
                (&source.hook, HOOK.as_slice())
            };
            if value == 0. {
                m.reviewed_direction_failures +=
                    usize::from(pose.iter().any(|f| ids.contains(&f.address)));
                return;
            }
            if matches!(control, Control::Brake) {
                for p in [[3., -28., 5.], [-4., -28., 5.]] {
                    pin(faces, pose, ids, p, scale, m);
                }
            } else {
                pin(faces, pose, ids, [0., -37.5, -3.], scale, m);
            }
            skins(faces, pose, ids, scale, m);
            rigid(faces, pose, ids, scale, m, false);
            if value == 1. {
                coverage(faces, pose, ids, m);
            }
        }
        Control::Gear => {
            for (ids, p) in [
                (LEFT.as_slice(), [-8., -7., -6.]),
                (RIGHT.as_slice(), [9., -7., -6.]),
            ] {
                pin(&source.gear, pose, ids, p, scale, m);
                skins(reference, pose, ids, scale, m);
                rigid(reference, pose, ids, scale, m, true);
            }
            if value == 0. {
                m.reviewed_wheel_failed |= pose.iter().any(|f| {
                    NOSE.contains(&f.address)
                        || BRACE.contains(&f.address)
                        || DOORS.contains(&f.address)
                });
            } else {
                pin(&source.gear, pose, &NOSE, [0., 55., -6.], scale, m);
                pin(&source.gear, pose, &BRACE, [0., 44., -5.5], scale, m);
                rigid(reference, pose, &NOSE, scale, m, true);
                skins(reference, pose, &NOSE, scale, m);
                skins(reference, pose, &BRACE, scale, m);
                for (ids, roots) in [
                    (&DOORS[..2], [[-3., -13., -6.], [-3., 0., -6.]]),
                    (&DOORS[2..4], [[4., -13., -6.], [4., 0., -6.]]),
                    (&DOORS[4..], [[-1., 53., -6.], [-1., 70., -6.]]),
                ] {
                    for p in roots {
                        pin(&source.gear, pose, ids, p, scale, m);
                    }
                    skins(reference, pose, ids, scale, m);
                }
                // Link the brace lower-center to the actual posed wheel surface.
                let brace = source
                    .gear
                    .iter()
                    .filter(|f| BRACE.contains(&f.address))
                    .flat_map(|f| mapped(f, pose, [0., 54., -11.]))
                    .collect::<Vec<_>>();
                let wheel = source
                    .gear
                    .iter()
                    .filter(|f| [0x4f64, 0x4f83].contains(&f.address))
                    .flat_map(|f| mapped(f, pose, [0., 54., -11.]))
                    .collect::<Vec<_>>();
                m.reviewed_anchor_missing |= brace.is_empty() || wheel.is_empty();
                for a in &brace {
                    for b in &wheel {
                        m.max_reviewed_skin_gap =
                            m.max_reviewed_skin_gap.max(distance(*a, *b) * scale);
                    }
                }
                if value == 1. {
                    coverage(
                        &source.gear,
                        pose,
                        &LEFT
                            .into_iter()
                            .chain(RIGHT)
                            .chain(NOSE)
                            .chain(BRACE)
                            .chain(DOORS)
                            .collect::<Vec<_>>(),
                        m,
                    );
                }
            }
            let left = pose
                .iter()
                .filter(|f| LEFT.contains(&f.address))
                .flat_map(|f| f.positions.iter())
                .map(|p| p[0] * scale)
                .fold(f32::NEG_INFINITY, f32::max);
            let right = pose
                .iter()
                .filter(|f| RIGHT.contains(&f.address))
                .flat_map(|f| f.positions.iter())
                .map(|p| p[0] * scale)
                .fold(f32::INFINITY, f32::min);
            m.reviewed_min_wheel_gap = Some(right - left);
            m.reviewed_wheel_failed |= left > -0.4 || right < 0.45 || right - left < 0.90;
            if value > 0. && value < 1e-5 {
                for f in pose.iter().filter(|f| DOORS.contains(&f.address)) {
                    m.reviewed_wheel_failed |= f
                        .positions
                        .iter()
                        .any(|p| !inside_sections(*p, &source.body));
                }
            }
            if value < 1e-5 {
                for (a, uv) in &source.painted {
                    if value == 0. && (NOSE.contains(a) || BRACE.contains(a)) {
                        continue;
                    }
                    m.reviewed_wheel_failed |= pose
                        .iter()
                        .find(|f| f.address == *a)
                        .and_then(|f| painted_world(f, *uv))
                        .is_none_or(|p| !inside_sections(p, &source.body));
                }
            }
        }
        Control::Exhaust => {
            if value == 0. {
                m.reviewed_direction_failures +=
                    usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
            } else {
                m.reviewed_anchor_missing |=
                    pose.iter().filter(|f| FLAME.contains(&f.address)).count() != FLAME.len();
                for f in &source.flame {
                    for p in f.positions.iter().filter(|p| p[1] == -60.) {
                        pin(&source.flame, pose, &[f.address], *p, scale, m);
                    }
                }
                skins(&source.flame, pose, &FLAME, scale, m);
                if value == 1. {
                    coverage(&source.flame, pose, &FLAME, m);
                }
            }
        }
        _ => {}
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

#[cfg(test)]
mod tests {
    use super::*;
    fn synthetic(a: usize, p: Vec<[f32; 3]>) -> Face {
        super::super::tests::face(a, p)
    }
    #[test]
    fn source_uv_witness_follows_split_occurrences_and_detects_a_moved_hinge() {
        let mut raw = synthetic(
            0xf001,
            vec![[0., 0., 0.], [0., 4., 0.], [0., 4., 6.], [0., 0., 6.]],
        );
        raw.uv = vec![[0., 0.], [4., 0.], [4., 6.], [0., 6.]];
        let mut a = synthetic(
            raw.address,
            vec![
                raw.positions[0],
                [0., 2., 0.],
                [0., 2., 6.],
                raw.positions[3],
            ],
        );
        a.uv = vec![[0., 0.], [2., 0.], [2., 6.], [0., 6.]];
        let mut b = synthetic(
            raw.address,
            vec![
                [0., 2., 0.],
                raw.positions[1],
                raw.positions[2],
                [0., 2., 6.],
            ],
        );
        b.uv = vec![[2., 0.], [4., 0.], [4., 6.], [2., 6.]];
        let mut m = Metrics::default();
        pin(
            std::slice::from_ref(&raw),
            &[a.clone(), b.clone()],
            &[raw.address],
            [0., 2., 0.],
            1.,
            &mut m,
        );
        assert!(!m.reviewed_anchor_missing);
        assert_eq!(m.max_reviewed_anchor_gap, 0.);
        b.positions[0][0] = 1.;
        let mut m = Metrics::default();
        pin(&[raw], &[a, b], &[0xf001], [0., 2., 0.], 1., &mut m);
        assert!(m.max_reviewed_anchor_gap > EPSILON);
    }
    #[test]
    fn whole_assembly_rigidity_rejects_independent_sheet_translation_and_missing_faces() {
        let source = LEFT
            .into_iter()
            .map(|a| {
                synthetic(
                    a,
                    vec![[-8., 0., 0.], [-8., 2., 0.], [-8., 2., 3.], [-8., 0., 3.]],
                )
            })
            .collect::<Vec<_>>();
        let mut m = Metrics::default();
        rigid(&source, &source, &LEFT, 1., &mut m, true);
        assert!(!m.reviewed_wheel_failed);
        let mut shifted = source.clone();
        for p in &mut shifted[0].positions {
            p[1] += 0.25;
        }
        let mut m = Metrics::default();
        rigid(&source, &shifted, &LEFT, 1., &mut m, true);
        assert!(m.reviewed_wheel_failed);
        let mut m = Metrics::default();
        rigid(&source, &source[..5], &LEFT, 1., &mut m, true);
        assert!(m.reviewed_wheel_failed);
    }
    #[test]
    fn neutral_vertex_coverage_requires_both_source_sides_of_a_partition() {
        let original = synthetic(
            0xf002,
            vec![[0., 0., 0.], [0., 4., 0.], [0., 4., 6.], [0., 0., 6.]],
        );
        let a = synthetic(
            original.address,
            vec![
                original.positions[0],
                [0., 2., 0.],
                [0., 2., 6.],
                original.positions[3],
            ],
        );
        let b = synthetic(
            original.address,
            vec![
                [0., 2., 0.],
                original.positions[1],
                original.positions[2],
                [0., 2., 6.],
            ],
        );
        let mut m = Metrics::default();
        coverage(
            std::slice::from_ref(&original),
            &[a.clone(), b],
            &[original.address],
            &mut m,
        );
        assert!(!m.reviewed_neutral_mismatch);
        let mut m = Metrics::default();
        coverage(&[original], &[a], &[0xf002], &mut m);
        assert!(m.reviewed_neutral_mismatch);
    }
}
