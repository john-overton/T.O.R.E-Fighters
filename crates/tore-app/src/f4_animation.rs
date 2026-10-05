//! Reviewed F4J/F4E/F4 surfaces with per-shape fitted continuous motion.
//! Neutral skins and source endpoints are retained; imported code never executes.
use crate::{AppResult, flight::State};
use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::FRAC_PI_2;
use tore_formats::{
    aircraft::AircraftId as Id,
    shape::{Face, Shape},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Family {
    J,
    E,
    G,
}
#[derive(Clone, Copy)]
struct Spec {
    family: Family,
    code: usize,
    count: usize,
    words: &'static [usize],
    flame: &'static [usize],
    brake: &'static [usize],
    gear: &'static [usize],
    hook: &'static [usize],
    rudder: [usize; 2],
    rudder_edge: [[f32; 3]; 2],
    tails: [[usize; 2]; 2],
    tail_edges: [[[f32; 3]; 2]; 2],
    roll: &'static [usize],
    flap_words: [usize; 2],
    flap_faces: [[usize; 2]; 2],
}
fn specification(id: Id) -> AppResult<Spec> {
    let spec = match id {
        Id::F4B | Id::F4J => Spec {
            family: Family::J,
            code: 25222,
            count: 309,
            words: &[0x7250, 0x7256, 0x725c, 0x7268, 0x726e, 0x7274, 0x727a],
            flame: &[
                0x534c, 0x5373, 0x539a, 0x53c1, 0x53e8, 0x540f, 0x5436, 0x545d, 0x54d3, 0x54fa,
                0x5521, 0x5548, 0x556f, 0x5596, 0x55bd, 0x55e4,
            ],
            brake: &[0x4b12, 0x4b31, 0x4b50, 0x4bde, 0x4bfd, 0x4c1c],
            gear: &[
                0x4c68, 0x4c87, 0x4ceb, 0x4d0a, 0x4d29, 0x4d48, 0x4d94, 0x4db3, 0x4e17, 0x4e36,
                0x4e55, 0x4e74, 0x4ec0, 0x4edf, 0x4f2b, 0x4f4a, 0x4fc6, 0x4fe5, 0x5004, 0x5023,
                0x5042, 0x5061,
            ],
            hook: &[0x509f, 0x50be],
            rudder: [0x4a55, 0x4a7c],
            rudder_edge: [[0., -79., 10.], [0., -88., 29.]],
            tails: [[0x346e, 0x3489], [0x3613, 0x362e]],
            tail_edges: [
                [[-4., -66., 7.], [-1., -90., 7.]],
                [[4., -65., 7.], [1., -90., 7.]],
            ],
            roll: &[0x1eeb, 0x1f10, 0x1f94, 0x216c, 0x2199],
            flap_words: [0x726e, 0x7274],
            flap_faces: [[0x52af, 0x52d6], [0x519f, 0x51c6]],
        },
        Id::F4E => Spec {
            family: Family::E,
            code: 20000,
            count: 213,
            words: &[0x5df0, 0x5df6, 0x5dfc, 0x5e08, 0x5e0e, 0x5e14],
            flame: &[
                0x4341, 0x4360, 0x437f, 0x439e, 0x43bd, 0x43dc, 0x43fb, 0x441a,
            ],
            brake: &[0x429c, 0x42b3],
            gear: &[0x4466, 0x4485, 0x44d1, 0x44f0, 0x453c, 0x455b],
            hook: &[],
            rudder: [0x41ef, 0x420e],
            rudder_edge: [[0., -57., 8.], [0., -62., 21.]],
            tails: [[0x2b15, 0x2b30], [0x2405, 0x2420]],
            tail_edges: [
                [[-4., -45., 6.], [-1., -65., 6.]],
                [[4., -45., 6.], [1., -65., 6.]],
            ],
            roll: &[0x2d6a, 0x2d88, 0x3104, 0x31c7],
            flap_words: [0x5e08, 0x5e0e],
            flap_faces: [[0x404a, 0x4069], [0x3f53, 0x3f72]],
        },
        Id::F4G => Spec {
            family: Family::G,
            code: 21456,
            count: 233,
            words: &[0x63a0, 0x63a6, 0x63ac, 0x63b8, 0x63be, 0x63c4],
            flame: &[
                0x426a, 0x4289, 0x42a8, 0x42c7, 0x42e6, 0x4305, 0x4324, 0x4343,
            ],
            brake: &[0x41d5, 0x41ec],
            gear: &[0x438f, 0x43ae, 0x43fa, 0x4419, 0x4465, 0x4484],
            hook: &[],
            rudder: [0x4118, 0x413f],
            rudder_edge: [[0., -66., 9.], [0., -71., 23.]],
            tails: [[0x281c, 0x282f], [0x233f, 0x2352]],
            tail_edges: [
                [[-4., -54., 7.], [-1., -74., 7.]],
                [[4., -54., 7.], [1., -74., 7.]],
            ],
            roll: &[0x2a40, 0x2adb, 0x2d81, 0x2de9],
            flap_words: [0x63b8, 0x63be],
            flap_faces: [[0x3f73, 0x3f92], [0x3e9c, 0x3ebb]],
        },
        _ => return Err("aircraft has no reviewed F-4 family rig".into()),
    };
    Ok(spec)
}
struct Morph {
    neutral: Vec<[f32; 3]>,
    down: Vec<[f32; 3]>,
    closure: bool,
}
pub struct Rig {
    spec: Spec,
    flaps: BTreeMap<usize, Morph>,
    flame_root: f32,
}
fn source_face(shape: &Shape, address: usize) -> AppResult<&Face> {
    shape
        .faces
        .iter()
        .find(|f| f.address == address)
        .ok_or_else(|| format!("F-4 source lacks reviewed face {address:x}").into())
}
fn require_edge(shape: &Shape, addresses: &[usize], edge: [[f32; 3]; 2]) -> AppResult<()> {
    for &address in addresses {
        if !edge
            .iter()
            .all(|p| source_face(shape, address).is_ok_and(|f| f.positions.contains(p)))
        {
            return Err(format!("unreviewed F-4 source hinge at {address:x}").into());
        }
    }
    Ok(())
}
fn flap_targets(family: Family, side: f32) -> [([f32; 3], [f32; 3]); 2] {
    match family {
        Family::J => {
            let inner = if side < 0. { -12. } else { 13. };
            [
                ([inner, -34., -8.], [side * 13., -33., -12.]),
                ([side * 43., -41., -8.], [side * 43., -39., -12.]),
            ]
        }
        Family::E => [
            ([side * 11., -25., -5.], [side * 11., -22., -12.]),
            ([side * 39., -27., -5.], [side * 39., -26., -10.]),
        ],
        Family::G => [
            ([side * 12., -32., -4.], [side * 12., -30., -11.]),
            ([side * 40., -35., -4.], [side * 40., -33., -9.]),
        ],
    }
}
fn flap_down_faces(family: Family, left: bool) -> &'static [usize] {
    match (family, left) {
        (Family::J, true) => &[0x520c, 0x522d, 0x524e, 0x526f],
        (Family::J, false) => &[0x50fc, 0x511d, 0x513e, 0x515f],
        (Family::E, true) => &[0x40bb, 0x40e2, 0x4101],
        (Family::E, false) => &[0x3fc4, 0x3feb, 0x400a],
        (Family::G, true) => &[0x3fd4, 0x3feb, 0x400a],
        (Family::G, false) => &[0x3efd, 0x3f14, 0x3f33],
    }
}
impl Rig {
    pub fn load(id: Id, bytes: &[u8], mut neutral: Shape) -> AppResult<(Self, Shape)> {
        let spec = specification(id)?;
        if tore_formats::module::code(bytes)?.0.len() != spec.code
            || neutral.faces.len() != spec.count
            || neutral.state_words != spec.words.iter().copied().collect()
        {
            return Err("unreviewed F-4 source animation layout".into());
        }
        require_edge(&neutral, &spec.rudder, spec.rudder_edge)?;
        for side in 0..2 {
            require_edge(&neutral, &spec.tails[side], spec.tail_edges[side])?;
        }
        for &address in spec.roll {
            source_face(&neutral, address)?;
        }
        let original: BTreeSet<_> = neutral.faces.iter().map(|f| f.address).collect();
        let mut flame_root = f32::NEG_INFINITY;
        let devices = [
            (spec.words[0], spec.flame),
            (spec.words[1], spec.brake),
            (spec.words[2], spec.gear),
        ];
        for (word, expected) in devices
            .into_iter()
            .chain((!spec.hook.is_empty()).then_some((0x7268, spec.hook)))
        {
            let pose = Shape::with_state(bytes, &[(word, 1)].into())?;
            let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
            if !original.is_subset(&active)
                || active
                    .difference(&original)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != expected.iter().copied().collect()
            {
                return Err(format!("unreviewed F-4 source device branch {word:x}").into());
            }
            for f in pose
                .faces
                .into_iter()
                .filter(|f| expected.contains(&f.address))
            {
                if spec.flame.contains(&f.address) {
                    for p in &f.positions {
                        flame_root = flame_root.max(p[1]);
                    }
                }
                neutral.faces.push(f);
            }
        }
        let mut flaps = BTreeMap::new();
        for side in 0..2 {
            let down = Shape::with_state(bytes, &[(spec.flap_words[side], -1)].into())?;
            let active: BTreeSet<_> = down.faces.iter().map(|f| f.address).collect();
            let expected = flap_down_faces(spec.family, side == 0);
            if active
                .difference(&original)
                .copied()
                .collect::<BTreeSet<_>>()
                != expected.iter().copied().collect()
                || original
                    .difference(&active)
                    .copied()
                    .collect::<BTreeSet<_>>()
                    != spec.flap_faces[side].into_iter().collect()
            {
                return Err("unreviewed F-4 source down-flap branch".into());
            }
            let targets = flap_targets(spec.family, if side == 0 { -1. } else { 1. });
            let down_points: BTreeSet<_> = expected
                .iter()
                .flat_map(|&a| {
                    source_face(&down, a)
                        .unwrap()
                        .positions
                        .iter()
                        .map(|p| p.map(f32::to_bits))
                })
                .collect();
            for &address in &spec.flap_faces[side] {
                let base = source_face(&neutral, address)?;
                if base.positions.len() != 4 {
                    return Err("unreviewed F-4 neutral flap topology".into());
                }
                let target: Vec<_> = base
                    .positions
                    .iter()
                    .map(|p| targets.iter().find(|(a, _)| a == p).map_or(*p, |(_, b)| *b))
                    .collect();
                if target
                    .iter()
                    .any(|p| !down_points.contains(&p.map(f32::to_bits)))
                    || base
                        .positions
                        .iter()
                        .filter(|p| targets.iter().any(|(a, _)| a == *p))
                        .count()
                        != 2
                {
                    return Err("unreviewed F-4 flap vertex mapping".into());
                }
                flaps.insert(
                    address,
                    Morph {
                        neutral: base.positions.clone(),
                        down: target,
                        closure: false,
                    },
                );
            }
            if spec.family != Family::J {
                let closure = source_face(&down, *expected.last().unwrap())?.clone();
                if closure.positions.len() != 3
                    || closure
                        .positions
                        .iter()
                        .filter(|p| **p == targets[1].1)
                        .count()
                        != 1
                {
                    return Err("unreviewed F-4 flap closure topology".into());
                }
                flaps.insert(
                    closure.address,
                    Morph {
                        neutral: closure
                            .positions
                            .iter()
                            .map(|p| if *p == targets[1].1 { targets[1].0 } else { *p })
                            .collect(),
                        down: closure.positions.clone(),
                        closure: true,
                    },
                );
                neutral.faces.push(closure);
            }
        }
        if spec.family == Family::J {
            neutral.faces = neutral
                .faces
                .into_iter()
                .flat_map(|f| {
                    if spec.roll.contains(&f.address) {
                        crate::aircraft_animation::split_surface(
                            &f,
                            [0.; 3],
                            [1., 0., 0.],
                            0.,
                            roll_distance,
                        )
                    } else {
                        vec![f]
                    }
                })
                .collect();
        }
        let rig = Self {
            spec,
            flaps,
            flame_root,
        };
        rig.validate_device_anchors(&neutral)?;
        Ok((rig, neutral))
    }
    pub fn flame(&self, address: usize) -> bool {
        self.spec.flame.contains(&address)
    }
    fn validate_device_anchors(&self, shape: &Shape) -> AppResult<()> {
        let spec = self.spec;
        for &a in spec.gear {
            let f = source_face(shape, a)?;
            let motion =
                gear_motion(spec.family, a).ok_or("F-4 gear face has no reviewed assembly")?;
            if motion
                .anchor_z
                .is_some_and(|z| f.positions.iter().filter(|p| p[2] == z).count() != 2)
            {
                return Err("unreviewed F-4 nose gear upper edge".into());
            }
        }
        if !spec.hook.is_empty() {
            require_edge(shape, spec.hook, [[0., -55., -7.], [0., -56., -5.]])?;
        }
        Ok(())
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let mut result = source.clone();
        let a = source.address;
        if let Some(morph) = self.flaps.get(&a) {
            let travel = state.flaps.clamp(0., 1.) as f32;
            if morph.closure && travel == 0. {
                return None;
            }
            result.positions = morph
                .neutral
                .iter()
                .zip(&morph.down)
                .map(|(n, d)| std::array::from_fn(|i| n[i] + (d[i] - n[i]) * travel))
                .collect();
            update_normal(source, &mut result);
        } else if self.spec.rudder.contains(&a) {
            let [root, tip] = self.spec.rudder_edge;
            turn(
                &mut result,
                root,
                std::array::from_fn(|i| tip[i] - root[i]),
                state.rudder.clamp(-1., 1.) * 0.35,
            );
        } else if let Some(side) = self.spec.tails.iter().position(|group| group.contains(&a)) {
            let roots = self.spec.tail_edges[side];
            let pivot = std::array::from_fn(|i| (roots[0][i] + roots[1][i]) * 0.5);
            constrained_turn(
                source,
                &mut result,
                pivot,
                [1., 0., 0.],
                -state.elevator.clamp(-1., 1.) * 0.30,
                |p| roots.contains(&p),
            );
        } else if self.spec.roll.contains(&a) {
            let side = if source.positions.iter().map(|p| p[0]).sum::<f32>() < 0. {
                -1.
            } else {
                1.
            };
            let (root, tip, trail) = roll_geometry(self.spec.family, side);
            let axis = std::array::from_fn(|i| tip[i] - root[i]);
            if self.spec.family == Family::J {
                if source.positions.iter().all(|p| roll_distance(*p) <= 1e-4) {
                    turn(
                        &mut result,
                        root,
                        axis,
                        -state.aileron.clamp(-1., 1.) * 0.20,
                    );
                }
            } else {
                constrained_turn(
                    source,
                    &mut result,
                    root,
                    axis,
                    -state.aileron.clamp(-1., 1.) * 0.20,
                    |p| p != trail,
                );
            }
        } else if self.spec.brake.contains(&a) {
            let travel = state.brake.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            let (pivot, angle) = match self.spec.family {
                Family::J => ([0., 23., 11.], (10f64 / 12.).atan()),
                Family::E => ([0., 27., 10.], std::f64::consts::FRAC_PI_4),
                Family::G => ([0., 21., 11.], std::f64::consts::FRAC_PI_4),
            };
            constrained_turn(
                source,
                &mut result,
                pivot,
                [1., 0., 0.],
                angle * (1. - travel),
                |p| p[1] == pivot[1],
            );
        } else if self.spec.hook.contains(&a) {
            let travel = state.hook.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            constrained_turn(
                source,
                &mut result,
                [0., -55.5, -6.],
                [1., 0., 0.],
                -1.20 * (1. - travel),
                |p| [[0., -55., -7.], [0., -56., -5.]].contains(&p),
            );
        } else if self.spec.gear.contains(&a) {
            let travel = state.gear.clamp(0., 1.);
            if travel == 0. {
                return None;
            }
            let motion = gear_motion(self.spec.family, a).expect("validated F-4 gear group");
            constrained_turn(
                source,
                &mut result,
                motion.pivot,
                motion.axis,
                motion.angle * (1. - travel),
                |p| motion.anchor_z.is_some_and(|z| p[2] == z),
            );
        } else if self.flame(a) {
            let travel = state.exhaust.max(0.) as f32;
            if travel == 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = self.flame_root + (p[1] - self.flame_root) * travel;
            }
            update_normal(source, &mut result);
        }
        Some(result)
    }
}
fn roll_distance(p: [f32; 3]) -> f32 {
    p[1] + 32. + 0.35 * (p[0].abs() - 43.)
}
fn roll_geometry(family: Family, side: f32) -> ([f32; 3], [f32; 3], [f32; 3]) {
    match family {
        Family::J => (
            [side * 43., -32., -8.],
            [side * 63., -39., -4.],
            [side * 43., -41., -8.],
        ),
        Family::E => (
            [side * 39., -21., -5.5],
            [side * 57., -29., 0.],
            [side * 39., -27., -5.],
        ),
        Family::G => (
            [side * 40., -29., -4.5],
            [if side < 0. { -59. } else { 58. }, -37., 2.],
            [side * 40., -35., -4.],
        ),
    }
}
struct Motion {
    pivot: [f32; 3],
    axis: [f32; 3],
    angle: f64,
    anchor_z: Option<f32>,
}
fn gear_motion(family: Family, a: usize) -> Option<Motion> {
    let (pivot, axis, angle, anchor_z) = match (family, a) {
        (Family::J, 0x4c68 | 0x4c87) => ([13., -7., -8.], [0., 1., 0.], -FRAC_PI_2, None),
        (Family::J, 0x4d94 | 0x4db3) => ([-13., -7., -8.], [0., 1., 0.], FRAC_PI_2, None),
        (Family::J, 0x4ceb | 0x4d0a | 0x4d29 | 0x4d48) => {
            ([29., -14.5, -8.], [0., 1., 0.], 1.45, None)
        }
        (Family::J, 0x4e17 | 0x4e36 | 0x4e55 | 0x4e74) => {
            ([-29., -14.5, -8.], [0., 1., 0.], -1.45, None)
        }
        (Family::J, 0x4ec0 | 0x4edf) => ([0., 68., -8.], [1., 0., 0.], FRAC_PI_2, None),
        (Family::J, 0x4f2b | 0x4f4a) => ([2., 52.5, -8.], [0., 1., 0.], FRAC_PI_2, None),
        (Family::J, 0x4fc6 | 0x4fe5) => ([0., 65., -8.], [1., 0., 0.], -FRAC_PI_2, Some(-8.)),
        (Family::J, 0x5004 | 0x5023 | 0x5042 | 0x5061) => {
            ([0., 65., -8.], [1., 0., 0.], -FRAC_PI_2, None)
        }
        (Family::E, 0x4466 | 0x4485) => ([22., -3., -6.], [0., 1., 0.], 1.45, None),
        (Family::E, 0x44d1 | 0x44f0) => ([-21., -3., -6.], [0., 1., 0.], -1.45, None),
        (Family::E, 0x453c | 0x455b) => ([0., 53.5, -7.], [1., 0., 0.], -FRAC_PI_2, Some(-7.)),
        (Family::G, 0x438f | 0x43ae) => ([22., -8., -5.], [0., 1., 0.], 1.45, None),
        (Family::G, 0x43fa | 0x4419) => ([-22., -8., -5.], [0., 1., 0.], -1.45, None),
        (Family::G, 0x4465 | 0x4484) => ([0., 48.5, -6.], [1., 0., 0.], -FRAC_PI_2, Some(-6.)),
        _ => return None,
    };
    Some(Motion {
        pivot,
        axis,
        angle,
        anchor_z,
    })
}
fn constrained_turn(
    source: &Face,
    result: &mut Face,
    pivot: [f32; 3],
    axis: [f32; 3],
    angle: f64,
    fixed: impl Fn([f32; 3]) -> bool,
) {
    if angle == 0. {
        return;
    }
    turn(result, pivot, axis, angle);
    for (a, b) in source.positions.iter().zip(&mut result.positions) {
        if fixed(*a) {
            *b = *a;
        }
    }
    update_normal(source, result);
}
fn turn(face: &mut Face, pivot: [f32; 3], axis: [f32; 3], angle: f64) {
    if angle == 0. {
        return;
    }
    let length = axis
        .iter()
        .map(|v| f64::from(*v).powi(2))
        .sum::<f64>()
        .sqrt();
    let axis = axis.map(|v| f64::from(v) / length);
    let (sin, cos) = angle.sin_cos();
    let rotate = |p: [f32; 3]| -> [f32; 3] {
        let p = p.map(f64::from);
        let dot = axis.iter().zip(p).map(|(a, b)| a * b).sum::<f64>();
        let cross = [
            axis[1] * p[2] - axis[2] * p[1],
            axis[2] * p[0] - axis[0] * p[2],
            axis[0] * p[1] - axis[1] * p[0],
        ];
        std::array::from_fn(|i| (p[i] * cos + cross[i] * sin + axis[i] * dot * (1. - cos)) as f32)
    };
    for p in &mut face.positions {
        let v = rotate(std::array::from_fn(|i| p[i] - pivot[i]));
        *p = std::array::from_fn(|i| pivot[i] + v[i]);
    }
    if let Some(n) = face.normal {
        let n = rotate([n[0], n[2], n[1]]);
        face.normal = Some([n[0], n[2], n[1]]);
    }
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
    let len = n.iter().map(|v| v * v).sum::<f64>().sqrt();
    (len > 1e-9).then(|| n.map(|v| (v / len) as f32))
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
    fn rig(id: Id) -> Rig {
        Rig {
            spec: specification(id).unwrap(),
            flaps: BTreeMap::new(),
            flame_root: 0.,
        }
    }
    fn synthetic(address: usize, positions: Vec<[f32; 3]>) -> Face {
        let len = positions.len();
        Face {
            address,
            positions,
            colors: vec![20; len],
            fog: Default::default(),
            uv: vec![[0.25, 0.75]; len],
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([1., 0., 0.]),
        }
    }
    fn close(a: [f32; 3], b: [f32; 3]) {
        assert!(
            a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4),
            "{a:?} != {b:?}"
        );
    }
    fn material(a: &Face, b: &Face) {
        assert_eq!(a.uv, b.uv);
        assert_eq!(a.colors, b.colors);
        assert_eq!(a.texture, b.texture);
        assert_eq!(a.subtype, b.subtype);
        assert!(b.positions.iter().flatten().all(|v| v.is_finite()));
        assert!(b.normal.unwrap().iter().all(|v| v.is_finite()));
    }
    #[test]
    fn all_family_rudders_start_neutral_and_hold_both_diagonal_hinge_points() {
        for id in [Id::F4B, Id::F4J, Id::F4E, Id::F4G] {
            let r = rig(id);
            let [a, b] = r.spec.rudder_edge;
            let witness = [0., a[1] - 7., a[2] + 2.];
            let f = synthetic(r.spec.rudder[0], vec![a, b, witness]);
            let twin = synthetic(r.spec.rudder[1], vec![witness, b, a]);
            let mut s = state();
            for input in [-1., -0.5, 0., 0.5, 1.] {
                s.rudder = input;
                let moved = r.animate(&f, &s).unwrap();
                let other = r.animate(&twin, &s).unwrap();
                close(moved.positions[0], a);
                close(moved.positions[1], b);
                for (p, q) in moved.positions.iter().zip(other.positions.iter().rev()) {
                    close(*p, *q);
                }
                if input == 0. {
                    assert_eq!(moved.positions, f.positions);
                } else {
                    assert_eq!(
                        moved.positions[2][0].signum(),
                        input as f32 / input.abs() as f32
                    );
                }
                material(&f, &moved);
            }
        }
    }
    #[test]
    fn pitch_raises_both_sides_without_detaching_either_tail_root() {
        for id in [Id::F4J, Id::F4E, Id::F4G] {
            let r = rig(id);
            let mut s = state();
            for input in [-1., -0.5, 0., 0.5, 1.] {
                s.elevator = input;
                for side in 0..2 {
                    let [a, b] = r.spec.tail_edges[side];
                    let witness = [if side == 0 { -20. } else { 20. }, b[1] - 5., a[2] - 4.];
                    let f = synthetic(r.spec.tails[side][0], vec![a, b, witness]);
                    let twin = synthetic(r.spec.tails[side][1], vec![witness, b, a]);
                    let moved = r.animate(&f, &s).unwrap();
                    let other = r.animate(&twin, &s).unwrap();
                    close(moved.positions[0], a);
                    close(moved.positions[1], b);
                    for (p, q) in moved.positions.iter().zip(other.positions.iter().rev()) {
                        close(*p, *q);
                    }
                    if input != 0. {
                        assert_eq!(
                            (moved.positions[2][2] - witness[2]).signum(),
                            input.signum() as f32
                        );
                    }
                    material(&f, &moved);
                }
            }
        }
    }
    #[test]
    fn outward_roll_axes_need_no_second_side_sign_and_leave_hinges_fixed() {
        for id in [Id::F4J, Id::F4E, Id::F4G] {
            let r = rig(id);
            let mut s = state();
            for input in [-1., -0.5, 0., 0.5, 1.] {
                s.aileron = input;
                for side in [-1., 1.] {
                    let (root, tip, trail) = roll_geometry(r.spec.family, side);
                    let address = r
                        .spec
                        .roll
                        .iter()
                        .copied()
                        .find(|a| match (r.spec.family, side < 0.) {
                            (Family::J, true) => *a == 0x216c,
                            (Family::J, false) => *a == 0x1f10,
                            (Family::E, true) => *a == 0x3104,
                            (Family::E, false) => *a == 0x2d6a,
                            (Family::G, true) => *a == 0x2d81,
                            (Family::G, false) => *a == 0x2a40,
                        })
                        .unwrap();
                    let f = synthetic(address, vec![root, tip, trail]);
                    let moved = r.animate(&f, &s).unwrap();
                    close(moved.positions[0], root);
                    close(moved.positions[1], tip);
                    if input != 0. {
                        assert_eq!(
                            (moved.positions[2][2] - trail[2]).signum(),
                            (input as f32 * side).signum()
                        );
                    }
                    material(&f, &moved);
                }
            }
        }
    }
    #[test]
    fn flap_morph_keeps_attachment_points_and_shared_skin_vertices() {
        let mut r = rig(Id::F4E);
        let a = [3., 2., 1.];
        let b = [9., 2., 1.];
        let c = [9., -2., 1.];
        let d = [3., -2., 1.];
        let down_c = [9., -1., -4.];
        let down_d = [3., -1., -5.];
        let f = synthetic(0x404a, vec![a, b, c, d]);
        let twin = synthetic(0x4069, vec![d, c, b, a]);
        for (face, down) in [
            (&f, vec![a, b, down_c, down_d]),
            (&twin, vec![down_d, down_c, b, a]),
        ] {
            r.flaps.insert(
                face.address,
                Morph {
                    neutral: face.positions.clone(),
                    down,
                    closure: false,
                },
            );
        }
        let mut s = state();
        for travel in [0., 0.25, 0.5, 0.75, 1.] {
            s.flaps = travel;
            let moved = r.animate(&f, &s).unwrap();
            let other = r.animate(&twin, &s).unwrap();
            close(moved.positions[0], a);
            close(moved.positions[1], b);
            for (p, q) in moved.positions.iter().zip(other.positions.iter().rev()) {
                close(*p, *q);
            }
            if travel == 0. {
                assert_eq!(moved.positions, f.positions);
            }
            if travel == 1. {
                close(moved.positions[2], down_c);
                close(moved.positions[3], down_d);
            }
            material(&f, &moved);
        }
    }
    #[test]
    fn main_gear_upper_edges_stay_attached_and_bounds_never_cross_at_21_poses() {
        for id in [Id::F4J, Id::F4E, Id::F4G] {
            let r = rig(id);
            let addresses = match r.spec.family {
                Family::J => [0x4ceb, 0x4e17],
                Family::E => [0x4466, 0x44d1],
                Family::G => [0x438f, 0x43fa],
            };
            let mut s = state();
            for sample in 1..=21 {
                s.gear = sample as f64 / 21.;
                let mut boundaries = vec![];
                for address in addresses {
                    let m = gear_motion(r.spec.family, address).unwrap();
                    let top_a = [m.pivot[0], m.pivot[1] - 3., m.pivot[2]];
                    let top_b = [m.pivot[0], m.pivot[1] + 3., m.pivot[2]];
                    let f = synthetic(
                        address,
                        vec![
                            top_a,
                            top_b,
                            [top_b[0], top_b[1], top_b[2] - 18.],
                            [top_a[0], top_a[1], top_a[2] - 18.],
                        ],
                    );
                    let moved = r.animate(&f, &s).unwrap();
                    close(moved.positions[0], top_a);
                    close(moved.positions[1], top_b);
                    let closest = moved
                        .positions
                        .iter()
                        .map(|p| p[0].abs())
                        .fold(f32::INFINITY, f32::min);
                    assert!(closest > 2., "main assembly crossed center at {sample}");
                    boundaries.push(closest);
                    material(&f, &moved);
                }
                assert!(boundaries[0] + boundaries[1] > 4.);
            }
        }
    }
    #[test]
    fn nose_gear_panel_roots_remain_fixed_while_distal_geometry_folds_aft() {
        for (id, address) in [(Id::F4J, 0x4fc6), (Id::F4E, 0x453c), (Id::F4G, 0x4465)] {
            let r = rig(id);
            let m = gear_motion(r.spec.family, address).unwrap();
            let z = m.anchor_z.unwrap();
            let a = [0., m.pivot[1] - 4., z];
            let b = [0., m.pivot[1] + 4., z];
            let f = synthetic(
                address,
                vec![a, b, [0., b[1], z - 12.], [0., a[1], z - 12.]],
            );
            let mut s = state();
            for sample in 1..=21 {
                s.gear = sample as f64 / 21.;
                let moved = r.animate(&f, &s).unwrap();
                close(moved.positions[0], a);
                close(moved.positions[1], b);
                if s.gear < 1. {
                    assert!(moved.positions[2][1] < f.positions[2][1]);
                }
                material(&f, &moved);
            }
        }
    }
    #[test]
    fn device_endpoints_and_fixed_body_are_preserved_and_hook_stows_upward() {
        let r = rig(Id::F4J);
        let mut s = state();
        let hook = synthetic(
            0x509f,
            vec![[0., -55., -7.], [0., -56., -5.], [0., -68., -19.]],
        );
        let brake = synthetic(
            0x4b12,
            vec![[2., 23., 12.], [-2., 23., 12.], [0., 11., 22.]],
        );
        s.hook = 1.;
        s.brake = 1.;
        assert_eq!(r.animate(&hook, &s).unwrap().positions, hook.positions);
        assert_eq!(r.animate(&brake, &s).unwrap().positions, brake.positions);
        s.hook = 0.5;
        s.brake = 0.5;
        let moved = r.animate(&hook, &s).unwrap();
        close(moved.positions[0], hook.positions[0]);
        close(moved.positions[1], hook.positions[1]);
        assert!(moved.positions[2][2] > hook.positions[2][2]);
        let moved = r.animate(&brake, &s).unwrap();
        close(moved.positions[0], brake.positions[0]);
        close(moved.positions[1], brake.positions[1]);
        assert!(moved.positions[2][2] < brake.positions[2][2]);
        s.hook = 0.;
        s.brake = 0.;
        assert!(r.animate(&hook, &s).is_none());
        assert!(r.animate(&brake, &s).is_none());
        let body = synthetic(1, vec![[0., 0., 0.], [2., 1., 0.], [0., 2., 3.]]);
        s.elevator = 1.;
        s.aileron = 1.;
        s.rudder = 1.;
        assert_eq!(r.animate(&body, &s).unwrap().positions, body.positions);
    }
}
