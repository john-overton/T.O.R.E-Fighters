//! Independent exact F31.SH source witnesses and fitted attachment contracts.
//! Prototype auxiliary-rate demands remain separate from VTOL lift controls.
use super::*;
use tore_formats::shape::Shape;
const LEFT: [usize; 6] = [0x409b, 0x40ba, 0x40d9, 0x40f8, 0x4117, 0x4136];
const RIGHT: [usize; 6] = [0x3f84, 0x3fa3, 0x3fc2, 0x3fe1, 0x4000, 0x401f];
const NOSE: [usize; 2] = [0x41ed, 0x420c];
const BRACE: [usize; 2] = [0x4182, 0x41a1];
const MAIN_PANEL: [usize; 4] = [0x3e43, 0x3e5a, 0x3e9e, 0x3eb5];
const NOSE_DOOR: [usize; 2] = [0x3ef9, 0x3f10];
const BRAKE: [usize; 4] = [0x47d0, 0x47e7, 0x4825, 0x483c];
const FLAME: [usize; 4] = [0x48a0, 0x48c7, 0x48ee, 0x4915];
const INNER_LEFT: [usize; 2] = [0x4773, 0x4792];
const INNER_RIGHT: [usize; 2] = [0x468c, 0x46ab];
const OUTER_LEFT: [usize; 3] = [0x30f2, 0x31aa, 0x3af8];
const OUTER_RIGHT: [usize; 3] = [0x3856, 0x3789, 0x3b17];
const FIN: [usize; 2] = [0x451b, 0x4532];
const CLOSURES: [usize; 2] = [0x4733, 0x464c];
const RAW_DOWN: [usize; 4] = [0x46f5, 0x4714, 0x460e, 0x462d];
const RAW_YAW: [usize; 4] = [0x45b5, 0x45cc, 0x4568, 0x457f];
const CANARD: [usize; 4] = [0x42b3, 0x42ca, 0x4258, 0x426f];
const PADDLE: [usize; 6] = [0x4342, 0x4381, 0x4406, 0x442e, 0x44b2, 0x44da];
pub(super) struct Sources {
    gear: Vec<Face>,
    brake: Vec<Face>,
    flame: Vec<Face>,
    body: Vec<Face>,
    down: BTreeMap<usize, Vec<[f32; 3]>>,
    negative: BTreeMap<usize, Vec<[f32; 3]>>,
    positive: BTreeMap<usize, Vec<[f32; 3]>>,
    closures: Vec<Face>,
    painted: Vec<(usize, [f32; 2])>,
}
fn branch(bytes: &[u8], word: usize, value: i32, ids: &[usize]) -> AppResult<Vec<Face>> {
    let faces: Vec<_> = Shape::with_state(bytes, &[(word, value)].into())?
        .faces
        .into_iter()
        .filter(|f| ids.contains(&f.address))
        .collect();
    if faces.len() != ids.len() {
        return Err("F31 witness source branch missing/duplicates reviewed faces".into());
    }
    Ok(faces)
}
impl Sources {
    pub(super) fn load(bytes: &[u8], atlas_bytes: &[u8]) -> AppResult<Self> {
        let atlas = tore_formats::Pic::parse(atlas_bytes)?;
        if (atlas.width, atlas.height) != (256, 399) {
            return Err("unreviewed F31 witness atlas dimensions".into());
        }
        let ids: Vec<_> = LEFT
            .into_iter()
            .chain(RIGHT)
            .chain(NOSE)
            .chain(BRACE)
            .chain(MAIN_PANEL)
            .chain(NOSE_DOOR)
            .collect();
        let gear = branch(bytes, 0x65b2, 1, &ids)?;
        let brake = branch(bytes, 0x65a6, 1, &BRAKE)?;
        let flame = branch(bytes, 0x65a0, 1, &FLAME)?;
        let body = Shape::parse(bytes)?
            .faces
            .into_iter()
            .filter(|f| f.positions.iter().all(|p| p[0].abs() <= 10.))
            .collect();
        let mut down = BTreeMap::new();
        let mut closures = Vec::new();
        for (word, bindings, closure) in [
            (
                0x65be,
                [
                    (0x4773, 0x46f5, [0, 1, 2, 3]),
                    (0x4792, 0x4714, [1, 2, 3, 0]),
                ],
                0x4733,
            ),
            (
                0x65c4,
                [
                    (0x468c, 0x460e, [0, 1, 2, 3]),
                    (0x46ab, 0x462d, [3, 0, 1, 2]),
                ],
                0x464c,
            ),
        ] {
            let source = Shape::with_state(bytes, &[(word, -1)].into())?;
            for (a, b, order) in bindings {
                let f = source
                    .faces
                    .iter()
                    .find(|f| f.address == b)
                    .ok_or("F31 source down flap missing")?;
                if f.positions.len() != 4 {
                    return Err("F31 source down topology changed".into());
                }
                down.insert(a, order.map(|i| f.positions[i]).into());
            }
            closures.push(
                source
                    .faces
                    .into_iter()
                    .find(|f| f.address == closure)
                    .ok_or("F31 source flap closure missing")?,
            );
        }
        let mut negative = BTreeMap::new();
        let mut positive = BTreeMap::new();
        for (value, bindings) in [
            (
                -1,
                [
                    (0x451b, 0x45b5, [3, 0, 1, 2]),
                    (0x4532, 0x45cc, [1, 2, 3, 0]),
                ],
            ),
            (
                1,
                [
                    (0x451b, 0x4568, [2, 1, 0, 3]),
                    (0x4532, 0x457f, [0, 3, 2, 1]),
                ],
            ),
        ] {
            let source = Shape::with_state(bytes, &[(0x65ca, value)].into())?;
            for (a, b, order) in bindings {
                let f = source
                    .faces
                    .iter()
                    .find(|f| f.address == b)
                    .ok_or("F31 signed rudder source missing")?;
                if f.positions.len() != 4 {
                    return Err("F31 signed rudder source topology changed".into());
                }
                let target = order.map(|i| f.positions[i]).into();
                if value < 0 {
                    negative.insert(a, target);
                } else {
                    positive.insert(a, target);
                }
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
                return Err("F31 witness painted gear UV layout".into());
            }
            let lo: [usize; 2] = std::array::from_fn(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::INFINITY, f32::min) as usize
            });
            let hi: [usize; 2] = std::array::from_fn(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::NEG_INFINITY, f32::max) as usize
            });
            if hi[0] >= atlas.width || hi[1] >= atlas.height {
                return Err("F31 witness UV outside original atlas".into());
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
            return Err("F31 witness has no original opaque gear samples".into());
        }
        Ok(Self {
            gear,
            brake,
            flame,
            body,
            down,
            negative,
            positive,
            closures,
            painted,
        })
    }
}
fn endpoint(map: &BTreeMap<usize, Vec<[f32; 3]>>, pose: &[Face], scale: f32, m: &mut Metrics) {
    for (a, positions) in map {
        m.reviewed_direction_failures += usize::from(!pose.iter().any(|f| {
            f.address == *a
                && f.positions.len() == positions.len()
                && f.positions
                    .iter()
                    .zip(positions)
                    .all(|(a, b)| distance(*a, *b) * scale <= EPSILON)
        }));
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
        Control::Elevator | Control::Aileron | Control::Flaps => {
            for (ids, roots, trailing, rollsign) in [
                (
                    INNER_LEFT.as_slice(),
                    [
                        [-20., -17., -5.],
                        [-4., -17., -5.],
                        [-20., -17., -6.],
                        [-4., -17., -6.],
                    ],
                    [[-20., -21., -6.], [-4., -21., -6.]],
                    -1.,
                ),
                (
                    INNER_RIGHT.as_slice(),
                    [
                        [20., -17., -5.],
                        [5., -17., -5.],
                        [20., -17., -6.],
                        [5., -17., -6.],
                    ],
                    [[20., -21., -6.], [5., -21., -6.]],
                    1.,
                ),
                (
                    OUTER_LEFT.as_slice(),
                    [
                        [-20., -17., -5.],
                        [-31., -17., -6.],
                        [-20., -17., -6.],
                        [-31., -17., -6.],
                    ],
                    [[-20., -21., -6.], [-31., -21., -6.]],
                    -1.,
                ),
                (
                    OUTER_RIGHT.as_slice(),
                    [
                        [20., -17., -5.],
                        [31., -17., -6.],
                        [20., -17., -6.],
                        [31., -17., -6.],
                    ],
                    [[20., -21., -6.], [31., -21., -6.]],
                    1.,
                ),
            ] {
                for point in roots {
                    pin(raw, pose, ids, point, scale, m);
                }
                skins(raw, pose, ids, scale, m);
                if value == 0. {
                    coverage(raw, pose, ids, m);
                } else if matches!(control, Control::Flaps) && ids.len() == 3 {
                    for f in raw.iter().filter(|f| ids.contains(&f.address)) {
                        for p in &f.positions {
                            pin(raw, pose, ids, *p, scale, m);
                        }
                    }
                } else {
                    let sign = if matches!(control, Control::Flaps) {
                        -1.
                    } else if matches!(control, Control::Elevator) {
                        1.
                    } else {
                        rollsign
                    };
                    for p in trailing {
                        signed(raw, pose, ids, p, (2, sign), (value, scale), m);
                    }
                }
            }
            if matches!(control, Control::Elevator) {
                for (ids, shaft, trailing) in [
                    (&CANARD[..2], [-32f32 / 9., 54., 1.], [-11., 51., 1.]),
                    (&CANARD[2..], [34f32 / 9., 54., 1.], [11., 51., 1.]),
                ] {
                    pin(raw, pose, ids, shaft, scale, m);
                    skins(raw, pose, ids, scale, m);
                    rigid(reference, pose, ids, scale, m, false);
                    if value != 0. {
                        signed(raw, pose, ids, trailing, (2, -1.), (value, scale), m);
                    }
                }
            }
            if matches!(control, Control::Flaps) {
                m.reviewed_direction_failures +=
                    usize::from(pose.iter().any(|f| RAW_DOWN.contains(&f.address)));
                for a in CLOSURES {
                    m.reviewed_direction_failures += usize::from(
                        pose.iter().filter(|f| f.address == a).count() != usize::from(value > 0.),
                    );
                }
                if value == 1. {
                    endpoint(&source.down, pose, scale, m);
                    coverage(&source.closures, pose, &CLOSURES, m);
                }
                if value > 0. {
                    for (a, ids) in [(0x4733, INNER_LEFT), (0x464c, INNER_RIGHT)] {
                        if let Some(f) = pose.iter().find(|f| f.address == a) {
                            for p in &f.positions {
                                m.reviewed_anchor_missing |=
                                    !pose.iter().filter(|g| ids.contains(&g.address)).any(|g| {
                                        g.positions
                                            .iter()
                                            .any(|q| distance(*p, *q) * scale <= EPSILON)
                                    });
                            }
                        }
                    }
                }
            }
        }
        Control::Rudder => {
            for p in [[0., -35., 8.], [0., -39., 19.]] {
                pin(raw, pose, &FIN, p, scale, m);
            }
            skins(raw, pose, &FIN, scale, m);
            m.reviewed_direction_failures +=
                usize::from(pose.iter().any(|f| RAW_YAW.contains(&f.address)));
            if value == 0. {
                coverage(raw, pose, &FIN, m);
            } else {
                for p in [[0., -40., 8.], [0., -42., 19.]] {
                    signed(raw, pose, &FIN, p, (0, 1.), (value, scale), m);
                }
                if value == -1. {
                    endpoint(&source.negative, pose, scale, m);
                }
                if value == 1. {
                    endpoint(&source.positive, pose, scale, m);
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
            for (ids, x) in [(&BRAKE[..2], 6.), (&BRAKE[2..], -6.)] {
                for z in [-2., 4.] {
                    pin(&source.brake, pose, ids, [x, -12., z], scale, m);
                }
                skins(&source.brake, pose, ids, scale, m);
                rigid(&source.brake, pose, ids, scale, m, false);
            }
            if value == 1. {
                coverage(&source.brake, pose, &BRAKE, m);
            }
            if value < 1. {
                for (ids, point, sign) in [
                    (&BRAKE[..2], [13., -19., 4.], -1.),
                    (&BRAKE[2..], [-13., -19., 4.], 1.),
                ] {
                    signed(
                        &source.brake,
                        pose,
                        ids,
                        point,
                        (0, sign),
                        (1. - value, scale),
                        m,
                    );
                }
            }
        }
        Control::VectorPitch | Control::VectorYaw => {
            for (ids, roots, trailing) in [
                (
                    &PADDLE[..2],
                    [[5., -41., -1.], [6., -41., 1.]],
                    [5., -46., 1.],
                ),
                (
                    &PADDLE[2..4],
                    [[-3., -41., -1.], [-4., -41., 1.]],
                    [-4., -46., 1.],
                ),
                (
                    &PADDLE[4..],
                    [[-1., -41., 5.], [2., -41., 5.]],
                    [0., -46., 5.],
                ),
            ] {
                for p in roots {
                    pin(raw, pose, ids, p, scale, m);
                }
                skins(raw, pose, ids, scale, m);
                rigid(raw, pose, ids, scale, m, false);
                if value == 0. {
                    coverage(raw, pose, ids, m);
                } else if matches!(control, Control::VectorYaw) && ids[0] == 0x44b2 {
                    for f in raw.iter().filter(|f| ids.contains(&f.address)) {
                        for p in &f.positions {
                            pin(raw, pose, ids, *p, scale, m);
                        }
                    }
                } else {
                    signed(
                        raw,
                        pose,
                        ids,
                        trailing,
                        (
                            if matches!(control, Control::VectorPitch) {
                                2
                            } else {
                                0
                            },
                            -1.,
                        ),
                        (value, scale),
                        m,
                    );
                }
            }
        }
        Control::Exhaust => {
            if value == 0. {
                m.reviewed_direction_failures +=
                    usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
                return;
            }
            m.reviewed_anchor_missing |=
                pose.iter().filter(|f| FLAME.contains(&f.address)).count() != FLAME.len();
            for f in &source.flame {
                for p in f.positions.iter().filter(|p| p[1] == -41.) {
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
/// Full-exhaust combined prototype demands. The caller supplies the actual
/// auxiliary-rate pose, not lift_controls, for the 5 by 5 signed demand grid.
pub(super) fn check_vector_combo(
    geometry: (&[Face], &[Face]),
    scale: f32,
    source: &Sources,
    m: &mut Metrics,
) {
    let (raw, pose) = geometry;
    for (ids, roots) in [
        (&PADDLE[..2], [[5., -41., -1.], [6., -41., 1.]]),
        (&PADDLE[2..4], [[-3., -41., -1.], [-4., -41., 1.]]),
        (&PADDLE[4..], [[-1., -41., 5.], [2., -41., 5.]]),
    ] {
        for p in roots {
            pin(raw, pose, ids, p, scale, m);
        }
        skins(raw, pose, ids, scale, m);
        rigid(raw, pose, ids, scale, m, false);
    }
    coverage(raw, pose, &[0x29f7], m);
    m.reviewed_anchor_missing |= raw.iter().all(|f| f.address != 0x29f7);
    pin(&source.flame, pose, &FLAME, [0., -41., 0.], scale, m);
    skins(&source.flame, pose, &FLAME, scale, m);
    rigid(&source.flame, pose, &FLAME, scale, m, false);
    plume_aperture(&source.flame, pose, raw, m);
}

// Compare posed source plume-front witnesses with the fixed nozzle aperture.
// This does not reconstruct the runtime's two angle rotations.
fn plume_aperture(flame: &[Face], pose: &[Face], raw: &[Face], m: &mut Metrics) {
    let Some(nozzle) = raw.iter().find(|f| f.address == 0x29f7) else {
        m.reviewed_anchor_missing = true;
        return;
    };
    let aperture = hull(nozzle.positions.iter().map(|p| [p[0], p[2]]).collect());
    m.reviewed_anchor_missing |= aperture.len() < 3;
    for f in flame {
        for p in f.positions.iter().filter(|p| p[1] == -41.) {
            let actual = mapped(f, pose, *p);
            m.reviewed_anchor_missing |= actual.is_empty();
            for q in actual {
                // Fitted envelope: front stays within 3 source units of the
                // original nozzle plane. Its projected cross stays inside it.
                m.reviewed_direction_failures += usize::from(
                    (q[1] + 40.).abs() > 3.001
                        || aperture
                            .iter()
                            .zip(aperture.iter().cycle().skip(1))
                            .take(aperture.len())
                            .any(|(a, b)| orient(*a, *b, [q[0], q[2]]) < -1e-3),
                );
            }
        }
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
    let all: Vec<_> = LEFT
        .into_iter()
        .chain(RIGHT)
        .chain(NOSE)
        .chain(BRACE)
        .chain(MAIN_PANEL)
        .chain(NOSE_DOOR)
        .collect();
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| all.contains(&f.address));
        return;
    }
    for (ids, p) in [
        (LEFT.as_slice(), [-4., -12., -5.]),
        (RIGHT.as_slice(), [4., -12., -5.]),
        (NOSE.as_slice(), [0., 33., -5.]),
        (BRACE.as_slice(), [0., 23.5, -5.]),
    ] {
        pin(&source.gear, pose, ids, p, scale, m);
        skins(&source.gear, pose, ids, scale, m);
        if ids != BRACE.as_slice() {
            rigid(reference, pose, ids, scale, m, true);
        }
    }
    for (ids, roots) in [
        (&MAIN_PANEL[..2], [[1., -14., -5.], [1., -1., -5.]]),
        (&MAIN_PANEL[2..], [[0., -14., -5.], [0., -1., -5.]]),
        (&NOSE_DOOR[..], [[-2., 24., -5.], [-2., 36., -5.]]),
    ] {
        for p in roots {
            pin(&source.gear, pose, ids, p, scale, m);
        }
        skins(&source.gear, pose, ids, scale, m);
        rigid(reference, pose, ids, scale, m, false);
    }
    let connection = [0., 32.5, -9.5];
    let brace = source
        .gear
        .iter()
        .filter(|f| BRACE.contains(&f.address))
        .flat_map(|f| mapped(f, pose, connection))
        .collect::<Vec<_>>();
    let nose = source
        .gear
        .iter()
        .filter(|f| NOSE.contains(&f.address))
        .flat_map(|f| mapped(f, pose, connection))
        .collect::<Vec<_>>();
    m.reviewed_anchor_missing |= brace.is_empty() || nose.is_empty();
    for a in &brace {
        for b in &nose {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(distance(*a, *b) * scale);
        }
    }
    let left = pose
        .iter()
        .filter(|f| [0x40d9, 0x40f8].contains(&f.address))
        .flat_map(|f| f.positions.iter())
        .map(|p| p[0] * scale)
        .fold(f32::NEG_INFINITY, f32::max);
    let right = pose
        .iter()
        .filter(|f| [0x3fc2, 0x3fe1].contains(&f.address))
        .flat_map(|f| f.positions.iter())
        .map(|p| p[0] * scale)
        .fold(f32::INFINITY, f32::min);
    m.reviewed_min_wheel_gap = Some(right - left);
    m.reviewed_wheel_failed |= left > -0.35 || right < 0.35 || right - left < 0.70;
    if value == 1. {
        coverage(&source.gear, pose, &all, m);
    }
    if value < 1e-5 {
        for (a, uv) in &source.painted {
            m.reviewed_wheel_failed |= pose
                .iter()
                .find(|f| f.address == *a)
                .and_then(|f| painted_world(f, *uv))
                .is_none_or(|p| !inside_sections(p, &source.body));
        }
        for f in pose
            .iter()
            .filter(|f| MAIN_PANEL.contains(&f.address) || NOSE_DOOR.contains(&f.address))
        {
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
    fn source_endpoint_gate_rejects_a_wrong_trailing_target_without_relaxing_the_front() {
        let expected: [(usize, Vec<[f32; 3]>); 1] = [(
            0xf001,
            vec![[0., 1., 3.], [0., 4., 3.], [0., 4., -1.], [0., 1., -1.]],
        )];
        let map = expected.clone().into();
        let good = synthetic(0xf001, expected[0].1.clone());
        let mut m = Metrics::default();
        endpoint(&map, std::slice::from_ref(&good), 1., &mut m);
        assert_eq!(m.reviewed_direction_failures, 0);
        let mut bad = good;
        bad.positions[2][2] += 1.;
        let mut m = Metrics::default();
        endpoint(&map, &[bad], 1., &mut m);
        assert!(m.reviewed_direction_failures > 0);
    }
    #[test]
    fn independent_painted_root_gate_rejects_translation_and_missing_faces() {
        let mut source = synthetic(
            0xf002,
            vec![[0., 0., 0.], [0., 4., 0.], [0., 4., 6.], [0., 0., 6.]],
        );
        source.uv = vec![[0., 0.], [4., 0.], [4., 6.], [0., 6.]];
        let mut m = Metrics::default();
        pin(
            std::slice::from_ref(&source),
            std::slice::from_ref(&source),
            &[source.address],
            [0., 2., 0.],
            1.,
            &mut m,
        );
        assert!(!m.reviewed_anchor_missing);
        assert_eq!(m.max_reviewed_anchor_gap, 0.);
        let mut shifted = source.clone();
        for p in &mut shifted.positions {
            p[0] += 0.25;
        }
        let mut m = Metrics::default();
        pin(
            std::slice::from_ref(&source),
            &[shifted],
            &[source.address],
            [0., 2., 0.],
            1.,
            &mut m,
        );
        assert!(m.max_reviewed_anchor_gap > EPSILON);
        let mut m = Metrics::default();
        pin(&[source], &[], &[0xf002], [0., 2., 0.], 1., &mut m);
        assert!(m.reviewed_anchor_missing);
    }
    #[test]
    fn whole_crossed_assembly_gate_rejects_independently_moved_tire_sheet() {
        let ids = [0xf003, 0xf004];
        let faces = ids.map(|a| {
            synthetic(
                a,
                vec![[1., 2., 3.], [1., 5., 3.], [1., 5., 7.], [1., 2., 7.]],
            )
        });
        let mut m = Metrics::default();
        rigid(&faces, &faces, &ids, 1., &mut m, true);
        assert!(!m.reviewed_wheel_failed);
        let mut bad = faces.clone();
        for p in &mut bad[1].positions {
            p[1] += 0.5;
        }
        let mut m = Metrics::default();
        rigid(&faces, &bad, &ids, 1., &mut m, true);
        assert!(m.reviewed_wheel_failed);
        let mut m = Metrics::default();
        rigid(&faces, &faces[..1], &ids, 1., &mut m, true);
        assert!(m.reviewed_wheel_failed);
    }
    #[test]
    fn independent_nozzle_gate_rejects_plume_aperture_escape() {
        let nozzle = synthetic(
            0x29f7,
            vec![
                [-4., -40., -4.],
                [4., -40., -4.],
                [4., -40., 4.],
                [-4., -40., 4.],
            ],
        );
        let mut front = synthetic(
            FLAME[0],
            vec![[-2., -41., 0.], [2., -41., 0.], [0., -60., 0.]],
        );
        front.uv = vec![[0., 0.], [4., 0.], [2., 19.]];
        let mut m = Metrics::default();
        plume_aperture(
            std::slice::from_ref(&front),
            std::slice::from_ref(&front),
            std::slice::from_ref(&nozzle),
            &mut m,
        );
        assert_eq!(m.reviewed_direction_failures, 0);
        assert!(!m.reviewed_anchor_missing);
        let mut escaped = front.clone();
        escaped.positions[0][0] = -5.;
        let mut m = Metrics::default();
        plume_aperture(&[front], &[escaped], &[nozzle], &mut m);
        assert!(m.reviewed_direction_failures > 0);
    }
}
