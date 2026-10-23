//! Independent exact A4E.PT/A4.SH witnesses. Authored fits remain explicit.
use super::*;
use tore_formats::shape::Shape;
const TAIL_LEFT: [usize; 4] = [0x4967, 0x4982, 0x49dd, 0x49f8];
const TAIL_RIGHT: [usize; 4] = [0x4345, 0x4361, 0x44b5, 0x44d5];
const TAIL_LEFT_FIXED: [usize; 2] = [0x4967, 0x49dd];
const TAIL_LEFT_MOVING: [usize; 2] = [0x4982, 0x49f8];
const TAIL_RIGHT_FIXED: [usize; 2] = [0x4345, 0x4361];
const TAIL_RIGHT_MOVING: [usize; 2] = [0x44b5, 0x44d5];
const ROLL_LEFT: [usize; 3] = [0x3ae9, 0x3ba2, 0x3c8d];
const ROLL_RIGHT: [usize; 3] = [0x376a, 0x37d4, 0x37f4];
const FIN: [usize; 2] = [0x455c, 0x457d];
const FLAPS: [usize; 4] = [0x51c6, 0x51ef, 0x4fde, 0x5007];
const LEFT: [usize; 2] = [0x54c9, 0x54e9];
const RIGHT: [usize; 2] = [0x5354, 0x5374];
const NOSE: [usize; 2] = [0x564f, 0x566f];
const BRACE: [usize; 2] = [0x5543, 0x5563];
const MAIN_DOORS: [usize; 8] = [
    0x528a, 0x52aa, 0x52d0, 0x52f9, 0x53ff, 0x5425, 0x5446, 0x546e,
];
const NOSE_DOORS: [usize; 2] = [0x55cc, 0x55f4];
const BRAKE: [usize; 12] = [
    0x599f, 0x59c5, 0x59e7, 0x5a17, 0x5a3d, 0x5a66, 0x5824, 0x584a, 0x586c, 0x589c, 0x58bc, 0x58e2,
];
const HOOK: [usize; 2] = [0x56ca, 0x56eb];
const HOOK_LOGICAL: [usize; 2] = [0x573f, 0x575f];
const CAPS: [usize; 4] = [0x50c6, 0x50dd, 0x4ede, 0x4f1e];
const WALLS: [usize; 4] = [0x514b, 0x5167, 0x4f63, 0x4f7f];
#[derive(Clone)]
struct Flap {
    neutral: Face,
    down: Face,
}
pub(super) struct Sources {
    gear: Vec<Face>,
    brake: Vec<Face>,
    hook: Vec<Face>,
    body: Vec<Face>,
    flaps: Vec<Flap>,
    caps: Vec<Flap>,
    walls: Vec<Face>,
    painted: Vec<(usize, [f32; 2])>,
}
fn branch(bytes: &[u8], word: usize, value: i32, ids: &[usize]) -> AppResult<Vec<Face>> {
    let faces: Vec<_> = Shape::with_state(bytes, &[(word, value)].into())?
        .faces
        .into_iter()
        .filter(|f| ids.contains(&f.address))
        .collect();
    if faces.len() != ids.len() {
        return Err("A4 witness branch missing/duplicates reviewed source faces".into());
    }
    Ok(faces)
}
impl Sources {
    pub(super) fn load(bytes: &[u8], atlas_bytes: &[u8]) -> AppResult<Self> {
        let atlas = tore_formats::Pic::parse(atlas_bytes)?;
        if (atlas.width, atlas.height) != (256, 449) {
            return Err("unreviewed A4 source atlas dimensions".into());
        }
        let neutral = Shape::parse(bytes)?;
        let ids: Vec<_> = LEFT
            .into_iter()
            .chain(RIGHT)
            .chain(NOSE)
            .chain(BRACE)
            .chain(MAIN_DOORS)
            .chain(NOSE_DOORS)
            .collect();
        let gear = branch(bytes, 0x6f96, 1, &ids)?;
        let brake = branch(bytes, 0x6f90, 1, &BRAKE)?;
        let hook = branch(bytes, 0x6fa2, 1, &HOOK)?;
        let mut flaps = Vec::new();
        for (word, bindings) in [
            (0x6fa8, [(0x51c6, 0x511d), (0x51ef, 0x50f4)]),
            (0x6fae, [(0x4fde, 0x4f35), (0x5007, 0x4ef5)]),
        ] {
            let source = Shape::with_state(bytes, &[(word, -1)].into())?;
            for (a, b) in bindings {
                let n = neutral
                    .faces
                    .iter()
                    .find(|f| f.address == a)
                    .ok_or("A4 neutral flap missing")?
                    .clone();
                let mut d = source
                    .faces
                    .iter()
                    .find(|f| f.address == b)
                    .ok_or("A4 source down flap missing")?
                    .clone();
                if n.positions.len() != 4 || d.positions.len() != 4 {
                    return Err("unreviewed A4 flap topology".into());
                }
                // The lower front is the documented neutral-root reconciliation,
                // not the raw down branch's one-unit forward displacement.
                for p in &mut d.positions {
                    if p[1] == -17. && p[2] == -8. {
                        p[1] = -18.;
                    }
                }
                d.address = a;
                flaps.push(Flap {
                    neutral: n,
                    down: d,
                });
            }
        }
        if flaps.len() != FLAPS.len()
            || !FLAPS
                .iter()
                .all(|a| flaps.iter().any(|f| f.neutral.address == *a))
        {
            return Err("A4 witness source flap identity set changed".into());
        }
        let mut caps = Vec::new();
        let mut walls = Vec::new();
        for (word, cap_ids, wall_ids) in [
            (0x6fa8, &CAPS[..2], &WALLS[..2]),
            (0x6fae, &CAPS[2..], &WALLS[2..]),
        ] {
            let shape = Shape::with_state(bytes, &[(word, -1)].into())?;
            walls.extend(
                shape
                    .faces
                    .iter()
                    .filter(|f| wall_ids.contains(&f.address))
                    .cloned(),
            );
            for f in shape.faces.iter().filter(|f| cap_ids.contains(&f.address)) {
                let mut down = f.clone();
                for p in &mut down.positions {
                    if p[1] == -17. && p[2] == -8. {
                        p[1] = -18.;
                    }
                }
                let mut n = down.clone();
                for p in &mut n.positions {
                    if p[1] == -23. && p[2] == -12. {
                        *p = flaps
                            .iter()
                            .flat_map(|f| &f.neutral.positions)
                            .find(|q| q[0] == p[0] && q[1] < -18.)
                            .copied()
                            .ok_or("A4 cap neutral trailing counterpart missing")?;
                    }
                }
                caps.push(Flap { neutral: n, down });
            }
        }
        if caps.len() != CAPS.len() || walls.len() != WALLS.len() {
            return Err("A4 source side topology missing".into());
        }
        let body = neutral.faces;
        let mut painted = Vec::new();
        for f in gear.iter().chain(&hook) {
            if f.uv.len() != f.positions.len() || f.uv.is_empty() {
                return Err("A4 source painted UV layout".into());
            }
            let lo: [usize; 2] = std::array::from_fn(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::INFINITY, f32::min) as usize
            });
            let hi: [usize; 2] = std::array::from_fn(|i| {
                f.uv.iter().map(|p| p[i]).fold(f32::NEG_INFINITY, f32::max) as usize
            });
            if hi[0] >= atlas.width || hi[1] >= atlas.height {
                return Err("A4 source UV outside atlas".into());
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
            return Err("A4 witness has no opaque source samples".into());
        }
        Ok(Self {
            gear,
            brake,
            hook,
            body,
            flaps,
            caps,
            walls,
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
        Control::Elevator => {
            for (ids, fixed, moving, shaft) in [
                (
                    TAIL_LEFT.as_slice(),
                    TAIL_LEFT_FIXED,
                    TAIL_LEFT_MOVING,
                    [-16., -54., 7.],
                ),
                (
                    TAIL_RIGHT.as_slice(),
                    TAIL_RIGHT_FIXED,
                    TAIL_RIGHT_MOVING,
                    [17., -54., 7.],
                ),
            ] {
                for p in [shaft, [0., -59., 7.], [0., -43., 7.]] {
                    pin(raw, pose, ids, p, scale, m);
                }
                skins(raw, pose, ids, scale, m);
                tail_roles(raw, pose, &fixed, &moving, scale, m);
                if value == 0. {
                    coverage(raw, pose, ids, m);
                } else {
                    signed(
                        raw,
                        pose,
                        ids,
                        [if shaft[0] < 0. { -18. } else { 19. }, -59., 7.],
                        (2, 1.),
                        (value, scale),
                        m,
                    );
                }
            }
        }
        Control::Aileron => {
            for (ids, side) in [(ROLL_LEFT.as_slice(), -1.), (ROLL_RIGHT.as_slice(), 1.)] {
                for f in raw.iter().filter(|f| ids.contains(&f.address)) {
                    for p in f.positions.iter().filter(|p| p[1] >= -18.) {
                        pin(raw, pose, ids, *p, scale, m);
                    }
                }
                skins(raw, pose, ids, scale, m);
                if value == 0. {
                    coverage(raw, pose, ids, m);
                } else {
                    for x in [21., 40.] {
                        signed(
                            raw,
                            pose,
                            ids,
                            [side * x, -25., -8.],
                            (2, side),
                            (value, scale),
                            m,
                        );
                    }
                }
            }
        }
        Control::Rudder => {
            for p in [[1., -45., 11.], [-1., -45., 11.], [0., -55., 27.]] {
                pin(raw, pose, &FIN, p, scale, m);
            }
            skins(raw, pose, &FIN, scale, m);
            if value == 0. {
                coverage(raw, pose, &FIN, m);
            } else {
                for z in [12., 27.] {
                    signed(raw, pose, &FIN, [0., -60., z], (0, 1.), (value, scale), m);
                }
            }
        }
        Control::Flaps => {
            if value == 0. {
                flap_surfaces(value, pose, scale, &source.flaps, m);
                m.reviewed_direction_failures += usize::from(
                    pose.iter()
                        .any(|f| CAPS.contains(&f.address) || WALLS.contains(&f.address)),
                );
            } else {
                let mut all = source.flaps.clone();
                all.extend(source.caps.clone());
                flap_surfaces(value, pose, scale, &all, m);
                coverage(&source.walls, pose, &WALLS, m);
            }
        }
        Control::Brake => brake(value, pose, scale, source, m),
        Control::Gear => gear(value, reference, pose, scale, source, m),
        Control::Hook => hook(value, raw, pose, scale, source, m),
        _ => {}
    }
}
fn tail_roles(
    raw: &[Face],
    pose: &[Face],
    fixed: &[usize],
    moving: &[usize],
    scale: f32,
    m: &mut Metrics,
) {
    rigid(raw, pose, moving, scale, m, false);
    for f in raw.iter().filter(|f| fixed.contains(&f.address)) {
        for p in &f.positions {
            pin(raw, pose, &[f.address], *p, scale, m);
        }
    }
}
fn gear(
    value: f64,
    reference: &[Face],
    pose: &[Face],
    scale: f32,
    source: &Sources,
    m: &mut Metrics,
) {
    let all: Vec<_> = LEFT
        .into_iter()
        .chain(RIGHT)
        .chain(NOSE)
        .chain(BRACE)
        .chain(MAIN_DOORS)
        .chain(NOSE_DOORS)
        .collect();
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| all.contains(&f.address));
        return;
    }
    let nose: Vec<_> = NOSE.into_iter().chain(BRACE).collect();
    for (ids, root) in [
        (LEFT.as_slice(), [-9., -8.5, -8.]),
        (RIGHT.as_slice(), [9., -8.5, -8.]),
        (nose.as_slice(), [0., 27., -8.]),
    ] {
        pin(&source.gear, pose, ids, root, scale, m);
        skins(&source.gear, pose, ids, scale, m);
        rigid(reference, pose, ids, scale, m, true);
    }
    for (ids, edge) in [
        (&MAIN_DOORS[..2], [[11., 0., -6.], [11., 9., -6.]]),
        (&MAIN_DOORS[2..4], [[11., -14., -7.], [11., 0., -7.]]),
        (&MAIN_DOORS[4..], [[-11., 0., -6.], [-11., 9., -6.]]),
    ] {
        let selected_ids: Vec<_> = ids
            .iter()
            .copied()
            .filter(|a| [0x528a, 0x52aa, 0x52d0, 0x52f9, 0x53ff, 0x5446].contains(a))
            .collect();
        for p in edge {
            pin(&source.gear, pose, &selected_ids, p, scale, m);
        }
        skins(&source.gear, pose, &selected_ids, scale, m);
        rigid(reference, pose, &selected_ids, scale, m, false);
    }
    for (ids, roots) in [
        (&[0x5425, 0x546e][..], [[-11., -14., -7.], [-11., 0., -7.]]),
        (NOSE_DOORS.as_slice(), [[-1., 24., -8.], [-1., 42., -9.]]),
    ] {
        for p in roots {
            pin(&source.gear, pose, ids, p, scale, m);
        }
        skins(&source.gear, pose, ids, scale, m);
        rigid(reference, pose, ids, scale, m, false);
    }
    for f in source.gear.iter().filter(|f| BRACE.contains(&f.address)) {
        for u in [184., 185., 186., 197., 198., 199.] {
            let uv = [u, 103.];
            if !source.painted.contains(&(f.address, uv)) {
                continue;
            }
            let points: Vec<_> = pose
                .iter()
                .filter(|g| g.address == f.address)
                .filter_map(|g| painted_world(g, uv))
                .collect();
            m.reviewed_anchor_missing |= points.is_empty();
            m.reviewed_wheel_failed |= points.iter().any(|p| !inside_vertical(*p, &source.body));
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
    for (a, uv) in &source.painted {
        if !LEFT.contains(a) && !RIGHT.contains(a) {
            continue;
        }
        let Some(original) = source
            .gear
            .iter()
            .find(|f| f.address == *a)
            .and_then(|f| painted_world(f, *uv))
        else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        if (original[2] + 8.).abs() > 1e-5 {
            continue;
        }
        m.reviewed_wheel_failed |= pose
            .iter()
            .find(|f| f.address == *a)
            .and_then(|f| painted_world(f, *uv))
            .is_none_or(|p| !inside_vertical(p, &source.body));
    }
    if value == 1. {
        coverage(&source.gear, pose, &all, m);
    }
    if value >= 0.25 {
        coverage(&source.gear, pose, &MAIN_DOORS, m);
        coverage(&source.gear, pose, &NOSE_DOORS, m);
    }
    if value < 1e-5 {
        for (a, uv) in &source.painted {
            if HOOK.contains(a) || [0x528a, 0x52aa, 0x53ff, 0x5446].contains(a) {
                continue;
            }
            m.reviewed_wheel_failed |= pose
                .iter()
                .find(|f| f.address == *a)
                .and_then(|f| painted_world(f, *uv))
                .is_none_or(|p| !inside_vertical(p, &source.body));
        }
        // The camouflaged doors end in their authored root plane. Their known
        // source upper corner is already above the wing, not new gear exposure.
        for f in pose
            .iter()
            .filter(|f| [0x528a, 0x52aa, 0x53ff, 0x5446].contains(&f.address))
        {
            m.reviewed_wheel_failed |= f
                .positions
                .iter()
                .any(|p| (p[2] + 6.).abs() * scale > EPSILON);
        }
        for f in pose
            .iter()
            .filter(|f| [0x52d0, 0x52f9, 0x5425, 0x546e].contains(&f.address))
        {
            m.reviewed_wheel_failed |= f
                .positions
                .iter()
                .any(|p| (p[2] + 7.).abs() * scale > EPSILON);
        }
        for f in pose.iter().filter(|f| NOSE_DOORS.contains(&f.address)) {
            m.reviewed_wheel_failed |= f
                .positions
                .iter()
                .any(|p| (18. * (p[2] + 8.) + p[1] - 24.).abs() / 325f32.sqrt() * scale > EPSILON);
        }
    }
}
fn hook(value: f64, raw: &[Face], pose: &[Face], scale: f32, source: &Sources, m: &mut Metrics) {
    let marker = [0., -277. / 11., -83. / 11.];
    m.reviewed_anchor_missing |= HOOK_LOGICAL
        .iter()
        .any(|a| pose.iter().filter(|f| f.address == *a).count() != 1);
    for a in HOOK_LOGICAL {
        if let Some(f) = pose.iter().find(|f| f.address == a) {
            let i = if a == 0x573f { 3 } else { 1 };
            m.reviewed_anchor_missing |= f.positions.len() != 5 || f.uv.len() != 5;
            if let Some(p) = f.positions.get(i) {
                m.max_reviewed_anchor_gap =
                    m.max_reviewed_anchor_gap.max(distance(*p, marker) * scale);
            }
            if f.positions.len() == 5 && f.uv.len() == 5 {
                let (front, corners) = if a == 0x573f {
                    ([2, 4], [0, 1, 2, 4])
                } else {
                    ([0, 2], [0, 2, 3, 4])
                };
                let (p, q) = (f.positions[front[0]], f.positions[front[1]]);
                let axis: [f32; 3] = std::array::from_fn(|k| q[k] - p[k]);
                let norm = axis.iter().map(|v| v * v).sum::<f32>();
                let t = axis
                    .iter()
                    .enumerate()
                    .map(|(k, v)| (marker[k] - p[k]) * v)
                    .sum::<f32>()
                    / norm;
                let projected = std::array::from_fn(|k| p[k] + axis[k] * t);
                m.max_reviewed_anchor_gap = m
                    .max_reviewed_anchor_gap
                    .max(distance(projected, marker) * scale);
                m.reviewed_direction_failures +=
                    usize::from(!t.is_finite() || !(-1e-5..=1.00001).contains(&t));
                let expected_uv: [f32; 2] = std::array::from_fn(|k| {
                    f.uv[front[0]][k] + (f.uv[front[1]][k] - f.uv[front[0]][k]) * t
                });
                m.reviewed_neutral_mismatch |= expected_uv
                    .iter()
                    .zip(f.uv[i])
                    .any(|(x, y)| (x - y).abs() > 1e-4);
                if let (Some(n), Some(d)) = (
                    raw.iter().find(|g| g.address == a),
                    source
                        .hook
                        .iter()
                        .find(|g| g.address == if a == 0x573f { 0x56eb } else { 0x56ca }),
                ) {
                    for (j, k) in corners.iter().enumerate() {
                        m.reviewed_neutral_mismatch |= f.uv[*k] != n.uv[j];
                    }
                    for j in 0..4 {
                        for k in j + 1..4 {
                            let old = distance(n.positions[j], n.positions[k]);
                            let end = distance(d.positions[j], d.positions[k]);
                            let actual = distance(f.positions[corners[j]], f.positions[corners[k]]);
                            m.reviewed_direction_failures += usize::from(
                                (actual - old).abs() > 1.001 || (end - old).abs() > 1.001,
                            );
                        }
                    }
                } else {
                    m.reviewed_anchor_missing = true;
                }
            }
        }
    }
    // Source stowed and down corner sets survive the extra collinear marker.
    if value == 0. {
        coverage(raw, pose, &HOOK_LOGICAL, m);
    }
    if value == 1. {
        for (a, b) in [(0x573f, 0x56eb), (0x575f, 0x56ca)] {
            let Some(expected) = source.hook.iter().find(|f| f.address == b) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            let Some(actual) = pose.iter().find(|f| f.address == a) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            for (p, uv) in expected.positions.iter().zip(&expected.uv) {
                m.reviewed_neutral_mismatch |= !actual
                    .positions
                    .iter()
                    .zip(&actual.uv)
                    .any(|(q, u)| distance(*p, *q) * scale <= EPSILON && u == uv);
            }
        }
    }
    if let (Some(a), Some(b)) = (
        pose.iter().find(|f| f.address == 0x573f),
        pose.iter().find(|f| f.address == 0x575f),
    ) {
        for p in &a.positions {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(
                b.positions
                    .iter()
                    .map(|q| distance(*p, *q) * scale)
                    .fold(f32::INFINITY, f32::min),
            );
        }
    }
}
fn inside_vertical(point: [f32; 3], faces: &[Face]) -> bool {
    let mut heights = Vec::new();
    for f in faces {
        for i in 1..f.positions.len().saturating_sub(1) {
            let [a, b, c] = [f.positions[0], f.positions[i], f.positions[i + 1]];
            let den = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if den.abs() < 1e-9 {
                continue;
            }
            let u = ((b[1] - c[1]) * (point[0] - c[0]) + (c[0] - b[0]) * (point[1] - c[1])) / den;
            let v = ((c[1] - a[1]) * (point[0] - c[0]) + (a[0] - c[0]) * (point[1] - c[1])) / den;
            let w = 1. - u - v;
            if u.min(v).min(w) >= -1e-5 {
                heights.push(u * a[2] + v * b[2] + w * c[2]);
            }
        }
    }
    let lo = heights.iter().copied().fold(f32::INFINITY, f32::min);
    let hi = heights.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    point[2] >= lo - 1e-3 && point[2] <= hi + 1e-3
}
fn brake(value: f64, pose: &[Face], scale: f32, source: &Sources, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| BRAKE.contains(&f.address)));
        return;
    }
    for (leaf, cavity, actuator, side) in [
        ([0x589c, 0x58bc], [0x584a, 0x58e2], [0x5824, 0x586c], 1.),
        ([0x5a17, 0x5a66], [0x599f, 0x5a3d], [0x59c5, 0x59e7], -1.),
    ] {
        for z in [-6., 0.] {
            pin(&source.brake, pose, &leaf, [side * 5., -30., z], scale, m);
        }
        skins(&source.brake, pose, &leaf, scale, m);
        rigid(&source.brake, pose, &leaf, scale, m, false);
        for f in source.brake.iter().filter(|f| cavity.contains(&f.address)) {
            for p in &f.positions {
                pin(&source.brake, pose, &[f.address], *p, scale, m);
            }
        }
        for f in source
            .brake
            .iter()
            .filter(|f| actuator.contains(&f.address))
        {
            for p in f.positions.iter().filter(|p| p[1] == -35.) {
                pin(&source.brake, pose, &[f.address], *p, scale, m);
            }
        }
        skins(&source.brake, pose, &actuator, scale, m);
        for z in [-4., -3.] {
            let point = [side * 9., -32., z];
            let ends: Vec<_> = source
                .brake
                .iter()
                .filter(|f| actuator.contains(&f.address))
                .flat_map(|f| mapped(f, pose, point))
                .collect();
            let leaf_points: Vec<_> = source
                .brake
                .iter()
                .filter(|f| leaf.contains(&f.address))
                .flat_map(|f| mapped(f, pose, point))
                .collect();
            m.reviewed_anchor_missing |= ends.is_empty() || leaf_points.is_empty();
            for a in &ends {
                for b in &leaf_points {
                    m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(distance(*a, *b) * scale);
                }
            }
        }
        if value < 1. {
            signed(
                &source.brake,
                pose,
                &leaf,
                [side * 13., -34., 0.],
                (0, -side),
                (1. - value, scale),
                m,
            );
        }
    }
    if value == 1. {
        coverage(&source.brake, pose, &BRAKE, m);
    }
    if value < 1e-5 {
        for f in source
            .brake
            .iter()
            .filter(|f| [0x589c, 0x58bc, 0x5a17, 0x5a66].contains(&f.address))
        {
            for p in f.positions.iter().filter(|p| p[1] == -34.) {
                let points = mapped(f, pose, *p);
                m.reviewed_anchor_missing |= points.is_empty();
                m.reviewed_direction_failures +=
                    usize::from(points.iter().any(|p| !inside_vertical(*p, &source.body)));
            }
        }
    }
}
fn flap_surfaces(value: f64, pose: &[Face], scale: f32, flaps: &[Flap], m: &mut Metrics) {
    let v = value.clamp(0., 1.) as f32;
    for flap in flaps {
        let Some(f) = pose.iter().find(|f| f.address == flap.neutral.address) else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        if f.positions.len() != flap.neutral.positions.len() || f.uv.len() != flap.neutral.uv.len()
        {
            m.reviewed_anchor_missing = true;
            continue;
        }
        for i in 0..f.positions.len() {
            let p = flap.neutral.positions[i];
            let q = flap.down.positions[i];
            let target = std::array::from_fn(|k| p[k] + (q[k] - p[k]) * v);
            let error = distance(target, f.positions[i]) * scale;
            m.reviewed_neutral_mismatch |= error > EPSILON;
            if p == q {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(error);
            }
        }
        if value == 0. {
            m.reviewed_neutral_mismatch |= f.uv != flap.neutral.uv;
        }
        if value == 1. {
            m.reviewed_neutral_mismatch |= f.uv != flap.down.uv;
        }
    }
    // Use corresponding neutral vertex indices, never stale UVs, to inspect
    // the original shared trailing points of these changing atlas maps.
    let mut shared: BTreeMap<[i32; 3], Vec<[f32; 3]>> = BTreeMap::new();
    for flap in flaps {
        if let Some(f) = pose.iter().find(|f| f.address == flap.neutral.address) {
            for (i, p) in flap.neutral.positions.iter().enumerate() {
                if let Some(q) = f.positions.get(i) {
                    shared
                        .entry(p.map(|v| (v * 10000.).round() as i32))
                        .or_default()
                        .push(*q);
                }
            }
        }
    }
    for points in shared.values() {
        for a in points {
            for b in points {
                m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(distance(*a, *b) * scale);
            }
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

fn failed(m: &Metrics) -> bool {
    !m.finite
        || m.reviewed_anchor_missing
        || m.max_reviewed_anchor_gap > EPSILON
        || m.max_reviewed_skin_gap > EPSILON
        || m.reviewed_rigid_panel_error > EPSILON
        || m.reviewed_neutral_mismatch
        || m.reviewed_wheel_failed
        || m.reviewed_direction_failures != 0
        || !m.new_planar_crossings.is_empty()
}
pub(super) fn combinations(
    source: &Sources,
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    for control in [Control::Gear, Control::Hook, Control::Brake] {
        let mut endpoint = neutral.clone();
        control.apply(&mut endpoint, 1.);
        let reference = airframe.animation_faces(&endpoint);
        let mut csv = String::from(
            "value,anchor_gap_ft,skin_gap_ft,rigidity_error_ft,crossings,checks_passed\n",
        );
        let mut values: Vec<_> = std::iter::once(1e-6)
            .chain((0..=200).map(|i| f64::from(i) / 200.))
            .collect();
        values.sort_by(f64::total_cmp);
        let mut poses = Vec::new();
        for (i, value) in values.into_iter().enumerate() {
            let mut state = neutral.clone();
            control.apply(&mut state, value);
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&reference, &pose, scale);
            check(
                control,
                value,
                (&source.body, &reference, &pose),
                scale,
                source,
                &mut m,
            );
            let passed = !failed(&m);
            writeln!(
                csv,
                "{value},{},{},{},{},{passed}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len()
            )?;
            if !passed {
                failures.push(format!(
                    "A4E dense {} value{value}: attachment, shape, source or containment gate",
                    control.name()
                ));
            }
            write_obj(
                &out.join(format!("dense-{}-{i}.obj", control.name())),
                &pose,
            )?;
            if i <= 1 || i % 10 == 1 {
                poses.push(pose);
            }
        }
        fs::write(out.join(format!("dense-{}.csv", control.name())), csv)?;
        contact_sheet(
            &out.join(format!("dense-{}.ppm", control.name())),
            &reference,
            &poses,
        )?;
    }
    let reference = airframe.animation_faces(neutral);
    for (label, first) in [
        ("flap-roll", Control::Flaps),
        ("pitch-roll", Control::Elevator),
    ] {
        let values = first.values();
        let mut csv =
            String::from("first,roll,anchor_gap_ft,skin_gap_ft,crossings,checks_passed\n");
        let mut poses = Vec::new();
        for value in values {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                let mut state = neutral.clone();
                first.apply(&mut state, value);
                state.aileron = roll;
                let pose = airframe.animation_faces(&state);
                let mut m = measure(&reference, &pose, scale);
                check(
                    first,
                    value,
                    (&source.body, &reference, &pose),
                    scale,
                    source,
                    &mut m,
                );
                check(
                    Control::Aileron,
                    roll,
                    (&source.body, &reference, &pose),
                    scale,
                    source,
                    &mut m,
                );
                let passed = !failed(&m);
                writeln!(
                    csv,
                    "{value},{roll},{},{},{},{passed}",
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.new_planar_crossings.len()
                )?;
                if !passed {
                    failures.push(format!(
                        "A4E {label} value{value}/roll{roll}: source surface/hinge/clearance gate"
                    ));
                }
                poses.push(pose);
            }
        }
        fs::write(out.join(format!("{label}-combinations.csv")), csv)?;
        contact_sheet(
            &out.join(format!("{label}-combinations.ppm")),
            &reference,
            &poses,
        )?;
    }
    Ok(failures)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn face(a: usize, p: Vec<[f32; 3]>) -> Face {
        super::super::tests::face(a, p)
    }
    fn synthetic_flap(a: usize) -> Flap {
        let mut neutral = face(
            a,
            vec![[0., 0., 2.], [4., 0., 2.], [4., -5., 0.], [0., -5., 0.]],
        );
        neutral.uv = vec![[0., 0.], [4., 0.], [4., 5.], [0., 5.]];
        let mut down = neutral.clone();
        down.positions[2] = [4., -3., -4.];
        down.positions[3] = [0., -3., -4.];
        down.uv = vec![[1., 0.], [5., 0.], [5., 3.], [1., 3.]];
        Flap { neutral, down }
    }
    fn intermediate(f: &Flap, v: f32) -> Face {
        let mut p = f.neutral.clone();
        for i in 0..p.positions.len() {
            p.positions[i] = std::array::from_fn(|k| {
                f.neutral.positions[i][k] + (f.down.positions[i][k] - f.neutral.positions[i][k]) * v
            });
            p.uv[i] = std::array::from_fn(|k| {
                f.neutral.uv[i][k] + (f.down.uv[i][k] - f.neutral.uv[i][k]) * v
            });
        }
        p
    }
    #[test]
    fn atlas_changing_flap_checks_vertex_correspondence_and_pinned_fronts() {
        let f = synthetic_flap(FLAPS[0]);
        for v in [0., 0.25, 0.5, 0.75, 1.] {
            let pose = intermediate(&f, v);
            let mut m = Metrics::default();
            flap_surfaces(f64::from(v), &[pose], 1., std::slice::from_ref(&f), &mut m);
            assert!(!m.reviewed_anchor_missing);
            assert!(!m.reviewed_neutral_mismatch);
            assert_eq!(m.max_reviewed_anchor_gap, 0.);
        }
        let mut bad = intermediate(&f, 1.);
        bad.positions[0][1] += 1.;
        let mut m = Metrics::default();
        flap_surfaces(1., &[bad], 1., &[f], &mut m);
        assert!(m.reviewed_neutral_mismatch);
        assert!(m.max_reviewed_anchor_gap > EPSILON);
    }
    #[test]
    fn exact_down_uv_and_free_endpoint_fail_independently() {
        let f = synthetic_flap(FLAPS[0]);
        let mut bad = intermediate(&f, 1.);
        bad.uv[2][0] += 1.;
        let mut m = Metrics::default();
        flap_surfaces(1., &[bad], 1., std::slice::from_ref(&f), &mut m);
        assert!(m.reviewed_neutral_mismatch);
        let mut bad = intermediate(&f, 1.);
        bad.positions[3][2] += 1.;
        let mut m = Metrics::default();
        flap_surfaces(1., &[bad], 1., &[f], &mut m);
        assert!(m.reviewed_neutral_mismatch);
    }
    #[test]
    fn opposite_flap_skins_detect_shared_trailing_tear_despite_new_uvs() {
        let a = synthetic_flap(FLAPS[0]);
        let b = synthetic_flap(FLAPS[1]);
        let mut pose = vec![intermediate(&a, 0.5), intermediate(&b, 0.5)];
        pose[1].positions[2][0] += 0.25;
        let mut m = Metrics::default();
        flap_surfaces(0.5, &pose, 1., &[a, b], &mut m);
        assert!(m.max_reviewed_skin_gap > EPSILON);
    }
    #[test]
    fn missing_flap_and_short_topology_are_not_neutral_passes() {
        let f = synthetic_flap(FLAPS[0]);
        let mut m = Metrics::default();
        flap_surfaces(0., &[], 1., std::slice::from_ref(&f), &mut m);
        assert!(m.reviewed_anchor_missing);
        let mut bad = f.neutral.clone();
        bad.positions.pop();
        let mut m = Metrics::default();
        flap_surfaces(0., &[bad], 1., &[f], &mut m);
        assert!(m.reviewed_anchor_missing);
    }
    #[test]
    fn painted_root_mapping_detects_movement_and_missing_face() {
        let mut f = face(
            0xf100,
            vec![[0., 0., 0.], [0., 4., 0.], [0., 4., 6.], [0., 0., 6.]],
        );
        f.uv = vec![[0., 0.], [4., 0.], [4., 6.], [0., 6.]];
        let mut m = Metrics::default();
        pin(
            std::slice::from_ref(&f),
            std::slice::from_ref(&f),
            &[f.address],
            [0., 2., 0.],
            1.,
            &mut m,
        );
        assert!(!m.reviewed_anchor_missing);
        assert_eq!(m.max_reviewed_anchor_gap, 0.);
        let mut bad = f.clone();
        for p in &mut bad.positions {
            p[0] += 0.5;
        }
        let mut m = Metrics::default();
        pin(
            std::slice::from_ref(&f),
            &[bad],
            &[f.address],
            [0., 2., 0.],
            1.,
            &mut m,
        );
        assert!(m.max_reviewed_anchor_gap > EPSILON);
        let mut m = Metrics::default();
        pin(&[f], &[], &[0xf100], [0., 2., 0.], 1., &mut m);
        assert!(m.reviewed_anchor_missing);
    }
    #[test]
    fn untextured_interior_uses_source_barycentrics_and_rejects_off_plane_point() {
        let mut f = face(0xf101, vec![[0., 0., 0.], [4., 0., 0.], [0., 4., 0.]]);
        f.uv.clear();
        let mut bad = f.clone();
        for p in &mut bad.positions {
            p[2] += 1.;
        }
        let points = mapped(&f, &[bad], [1., 1., 0.]);
        assert_eq!(points, vec![[1., 1., 1.]]);
        assert!(mapped(&f, std::slice::from_ref(&f), [1., 1., 2.]).is_empty());
    }
    #[test]
    fn original_body_sections_distinguish_containment_from_visible_protrusion() {
        let body = [
            face(
                0xf102,
                vec![
                    [-1., -2., -1.],
                    [1., -2., -1.],
                    [1., 2., -1.],
                    [-1., 2., -1.],
                ],
            ),
            face(
                0xf103,
                vec![[-1., -2., 1.], [1., -2., 1.], [1., 2., 1.], [-1., 2., 1.]],
            ),
        ];
        assert!(inside_vertical([0., 0., 0.], &body));
        assert!(!inside_vertical([2., 0., 0.], &body));
    }
    #[test]
    fn whole_assembly_check_rejects_an_independently_translated_brace_skin() {
        let ids = [0xf104, 0xf105];
        let faces = ids.map(|a| {
            face(
                a,
                vec![[0., 0., 0.], [0., 4., 0.], [0., 4., 6.], [0., 0., 6.]],
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
    fn hook_fixture() -> (Vec<Face>, Vec<Face>, Sources) {
        let root = [0., -277. / 11., -83. / 11.];
        let points = vec![
            [0., root[1] - 5., root[2] - 3.],
            [0., root[1] - 4., root[2] - 1.],
            [0., root[1] + 1., root[2] + 1.],
            [0., root[1] - 1., root[2] - 1.],
        ];
        let uv = vec![[2., 0.], [3., 1.], [4., 4.], [2., 2.]];
        let mut a = face(HOOK_LOGICAL[0], points.clone());
        a.uv = uv.clone();
        let mut b = face(HOOK_LOGICAL[1], points.into_iter().rev().collect());
        b.uv = uv.into_iter().rev().collect();
        let raw = vec![a.clone(), b.clone()];
        let mut targets = raw.clone();
        targets[0].address = 0x56eb;
        targets[1].address = 0x56ca;
        a.positions.insert(3, root);
        a.uv.insert(3, [3., 3.]);
        b.positions.insert(1, root);
        b.uv.insert(1, [3., 3.]);
        let source = Sources {
            gear: vec![],
            brake: vec![],
            hook: targets,
            body: raw.clone(),
            flaps: vec![],
            caps: vec![],
            walls: vec![],
            painted: vec![],
        };
        (raw, vec![a, b], source)
    }
    #[test]
    fn fixed_hook_marker_must_lie_on_its_front_edge_with_matching_uv() {
        let (raw, pose, source) = hook_fixture();
        let mut m = measure(&pose, &pose, 1.);
        hook(0., &raw, &pose, 1., &source, &mut m);
        assert!(!failed(&m));
        let mut bad = pose.clone();
        bad[0].positions[3][0] += 0.25;
        let mut m = Metrics::default();
        hook(0., &raw, &bad, 1., &source, &mut m);
        assert!(m.max_reviewed_anchor_gap > EPSILON);
        let mut bad = pose;
        bad[0].uv[3][0] += 0.25;
        let mut m = Metrics::default();
        hook(0., &raw, &bad, 1., &source, &mut m);
        assert!(m.reviewed_neutral_mismatch);
    }
    #[test]
    fn hook_keeps_both_source_endpoints_and_rejects_a_missing_stowed_skin() {
        let (raw, pose, source) = hook_fixture();
        let mut m = measure(&pose, &pose, 1.);
        hook(1., &raw, &pose, 1., &source, &mut m);
        assert!(!failed(&m));
        let mut bad = pose.clone();
        bad[0].positions[0][1] += 2.;
        let mut m = Metrics::default();
        hook(1., &raw, &bad, 1., &source, &mut m);
        assert!(m.reviewed_neutral_mismatch);
        let mut m = Metrics::default();
        hook(0., &raw, &pose[..1], 1., &source, &mut m);
        assert!(m.reviewed_anchor_missing);
    }
    #[test]
    fn combined_gate_rejects_measured_nonfinite_positions_and_normals() {
        let reference = vec![face(0xf106, vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]])];
        assert!(!failed(&measure(&reference, &reference, 1.)));
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut pose = reference.clone();
            pose[0].positions[1][2] = invalid;
            let m = measure(&reference, &pose, 1.);
            assert!(!m.finite);
            assert_eq!(m.max_reviewed_anchor_gap, 0.);
            assert_eq!(m.max_reviewed_skin_gap, 0.);
            assert!(failed(&m));
            let mut pose = reference.clone();
            pose[0].normal = Some([invalid, 0., 1.]);
            assert!(failed(&measure(&reference, &pose, 1.)));
        }
    }
    #[test]
    fn right_tail_roles_keep_both_forward_skins_fixed_and_test_rear_pair_together() {
        let raw: Vec<_> = TAIL_RIGHT
            .into_iter()
            .map(|a| {
                let x = if TAIL_RIGHT_FIXED.contains(&a) {
                    0.
                } else {
                    5.
                };
                let mut f = face(
                    a,
                    vec![[x, 0., 0.], [x + 2., 0., 0.], [x + 2., 2., 0.], [x, 2., 0.]],
                );
                f.uv.clear();
                f
            })
            .collect();
        let mut pose = raw.clone();
        for f in &mut pose {
            if TAIL_RIGHT_MOVING.contains(&f.address) {
                for p in &mut f.positions {
                    p[2] += 1.;
                }
            }
        }
        let mut m = measure(&raw, &pose, 1.);
        tail_roles(
            &raw,
            &pose,
            &TAIL_RIGHT_FIXED,
            &TAIL_RIGHT_MOVING,
            1.,
            &mut m,
        );
        assert!(!m.reviewed_anchor_missing);
        assert_eq!(m.max_reviewed_anchor_gap, 0.);
        assert_eq!(m.reviewed_rigid_panel_error, 0.);
        // The old positional assumption falsely pinned the first rear skin.
        let mut old = measure(&raw, &pose, 1.);
        tail_roles(
            &raw,
            &pose,
            &[TAIL_RIGHT[0], TAIL_RIGHT[2]],
            &[TAIL_RIGHT[1], TAIL_RIGHT[3]],
            1.,
            &mut old,
        );
        assert!(old.max_reviewed_anchor_gap > EPSILON);
        assert!(old.reviewed_rigid_panel_error > EPSILON);
        let mut bad = pose;
        for p in &mut bad
            .iter_mut()
            .find(|f| f.address == TAIL_RIGHT_MOVING[0])
            .unwrap()
            .positions
        {
            p[2] += 0.5;
        }
        let mut m = measure(&raw, &bad, 1.);
        tail_roles(
            &raw,
            &bad,
            &TAIL_RIGHT_FIXED,
            &TAIL_RIGHT_MOVING,
            1.,
            &mut m,
        );
        assert!(m.reviewed_rigid_panel_error > EPSILON);
    }
}

// Observe a rigid wing frame from three source/posed corners. No fitted
// rotation schedule or production transform is used for point correspondence.
