//! Independent F-111F whole-panel witnesses (F111-3, 2026-11-15): the two swing
//! wing panels (rigid, about the fitted pivot, measured against the speed
//! schedule), the trailing flap strips, the tailerons, the rudder, the two rigid
//! gear assemblies, the dorsal speed brake plates and the plumes. It also
//! measures what the sweep opens or buries (the root seam, the swept span and
//! the swept panel's clearance of the tailerons) and writes it as csv for the
//! docs. Measures the shared Airframe output and never repeats the rig's
//! algorithm; witness points are named in shape units and scaled with the
//! aircraft's own per-axis scale (the probe geometry is the scaled shape).
use super::*;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::F111;
const WING_R: [usize; 13] = [
    0x3fbc, 0x3fdd, 0x3ffe, 0x4021, 0x404a, 0x4073, 0x4096, 0x40bf, 0x4109, 0x4130, 0x4153, 0x421f,
    0x4246,
];
const WING_L: [usize; 14] = [
    0x44e5, 0x450c, 0x4533, 0x455a, 0x457b, 0x459c, 0x45bd, 0x45e0, 0x4607, 0x462a, 0x4651, 0x4674,
    0x4740, 0x4767,
];
const STRIP_R: [usize; 2] = [0x421f, 0x4246];
const STRIP_L: [usize; 2] = [0x4740, 0x4767];
const TAIL_R: [usize; 6] = [0x2d72, 0x2e72, 0x2d95, 0x2e95, 0x2db8, 0x2eb8];
const TAIL_L: [usize; 6] = [0x2071, 0x20d9, 0x1f08, 0x1f2b, 0x200f, 0x202d];
const RUDDER: [usize; 2] = [0x491c, 0x4943];
const DORSAL_R: [usize; 2] = [0x4eb9, 0x4ed0];
const DORSAL_L: [usize; 2] = [0x4e8b, 0x4ea2];
const DORSAL: [usize; 4] = [0x4e8b, 0x4ea2, 0x4eb9, 0x4ed0];
const MAIN: [usize; 16] = [
    0x4bcd, 0x4bec, 0x4c0b, 0x4c2a, 0x4c49, 0x4c68, 0x4c87, 0x4ca2, 0x4cbd, 0x4cd8, 0x4aaa, 0x4ad1,
    0x4ae8, 0x4b0f, 0x4b26, 0x4b4d,
];
const NOSE: [usize; 6] = [0x4997, 0x49ae, 0x4d38, 0x4d57, 0x4d76, 0x4d95];
const GEAR: [usize; 22] = [
    0x4bcd, 0x4bec, 0x4c0b, 0x4c2a, 0x4c49, 0x4c68, 0x4c87, 0x4ca2, 0x4cbd, 0x4cd8, 0x4aaa, 0x4ad1,
    0x4ae8, 0x4b0f, 0x4b26, 0x4b4d, 0x4997, 0x49ae, 0x4d38, 0x4d57, 0x4d76, 0x4d95,
];
const FLAME: [usize; 8] = [
    0x4f4e, 0x4f6d, 0x4f8c, 0x4fab, 0x4fca, 0x4fe9, 0x5008, 0x5027,
];
/// The most a stowed gear piece may sit outside the neutral skin, feet: the
/// main assembly (10 units long and 7 high in a hull 8 to 9 high) cannot fold
/// into it; the best fold of any hinge leaves 0.65 ft, the nose leg none.
const STOW_MAIN_FT: f32 = 0.7;
const STOW_NOSE_FT: f32 = 0.3;

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

/// One side of the aircraft: its wing, strip, taileron and dorsal plates.
struct Side {
    wing: &'static [usize],
    strip: &'static [usize],
    tail: &'static [usize],
    right: bool,
}
const SIDES: [Side; 2] = [
    Side {
        wing: &WING_R,
        strip: &STRIP_R,
        tail: &TAIL_R,
        right: true,
    },
    Side {
        wing: &WING_L,
        strip: &STRIP_L,
        tail: &TAIL_L,
        right: false,
    },
];
impl Side {
    fn sign(&self) -> f32 {
        if self.right { 1. } else { -1. }
    }
    /// A right-hand shape-unit point on this side, scaled.
    fn at(&self, p: [f32; 3]) -> [f32; 3] {
        sp([p[0] * self.sign(), p[1], p[2]])
    }
    /// The wing's outer trailing corner, the witness of the sweep direction.
    fn tip(&self) -> [f32; 3] {
        self.at([55., -5., 2.])
    }
    fn pivot(&self) -> [f32; 3] {
        self.at([13., 2.5, 0.])
    }
    /// The three root vertices on the panel's root edge (x 13), the points the
    /// turn carries out of the fuselage (leading) and into it (trailing). The
    /// left panel's root is at x -12 for its lower faces and -13 for the
    /// leading ones, as the source has it.
    fn root(&self) -> [[f32; 3]; 3] {
        let x = if self.right { 13. } else { -13. };
        [
            sp([x, 10., 2.]),
            sp([if self.right { 13. } else { -12. }, 1., 2.]),
            sp([if self.right { 13. } else { -12. }, -5., 2.]),
        ]
    }
    /// The strip's front edge ends: the outer pair (x 53, y -2) and the root
    /// pair, each a lower and an upper vertex whose midpoint is on the hinge.
    fn strip_outer(&self) -> ([f32; 3], [f32; 3]) {
        (self.at([53., -2., 2.]), self.at([53., -2., 3.]))
    }
    fn strip_root(&self) -> ([f32; 3], [f32; 3]) {
        let x = if self.right { 13. } else { -12. };
        (sp([x, 1., 2.]), sp([x, 1., 3.]))
    }
    fn strip_trailing(&self) -> [f32; 3] {
        self.at([53., -5., 2.])
    }
    fn tail_root(&self) -> ([f32; 3], [f32; 3]) {
        (self.at([10., -35., 2.]), self.at([10., -17., 2.]))
    }
    fn tail_edges(&self) -> [[f32; 3]; 2] {
        [self.at([21., -39., 2.]), self.at([24., -37., 2.])]
    }
}

fn group<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

/// Largest change of any vertex-to-vertex distance inside one group (feet).
fn rigidity(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
    let before = keyed(reference);
    let after = keyed(pose);
    let (mut old, mut now) = (Vec::new(), Vec::new());
    for (key, face) in before.iter().filter(|(k, _)| ids.contains(&k.0)) {
        let Some(after) = after.get(key) else {
            return f32::INFINITY;
        };
        if face.positions.len() != after.positions.len() {
            return f32::INFINITY;
        }
        old.extend(face.positions.iter().copied());
        now.extend(after.positions.iter().copied());
    }
    let mut error = 0f32;
    for i in 0..old.len() {
        for j in i + 1..old.len() {
            error = error.max((distance(old[i], old[j]) - distance(now[i], now[j])).abs() * scale);
        }
    }
    error
}

/// Rigid as one panel, and its shared vertices together.
fn panel(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let (old, new) = (group(reference, ids), group(pose, ids));
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
    let error = rigidity(reference, pose, ids, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
}

/// How far a vertex that stays on a hinge moved (feet); infinity when missing.
fn pinned(reference: &[Face], pose: &[Face], ids: &[usize], point: [f32; 3], scale: f32) -> f32 {
    let old = group(reference, ids);
    let moved = f4::witness_positions(&old, &keyed(pose), point);
    if moved.is_empty() {
        f32::INFINITY
    } else {
        moved
            .iter()
            .map(|q| distance(point, *q) * scale)
            .fold(0f32, f32::max)
    }
}

fn anchor(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    point: [f32; 3],
    scale: f32,
    m: &mut Metrics,
) {
    let gap = pinned(reference, pose, ids, point, scale);
    m.reviewed_anchor_missing |= !gap.is_finite();
    if gap.is_finite() {
        m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
    }
}

/// Distance the midpoint of one vertex pair moved (feet).
fn pair_gap(
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
    pair: ([f32; 3], [f32; 3]),
    scale: f32,
) -> Option<f32> {
    let find = |p: [f32; 3]| f4::witness_positions(old, new, p).into_iter().next();
    let (a, b) = (find(pair.0)?, find(pair.1)?);
    let middle: [f32; 3] = std::array::from_fn(|k| (pair.0[k] + pair.1[k]) / 2.);
    let now: [f32; 3] = std::array::from_fn(|k| (a[k] + b[k]) / 2.);
    Some(distance(middle, now) * scale)
}

fn pair_anchor(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    pair: ([f32; 3], [f32; 3]),
    scale: f32,
    m: &mut Metrics,
) {
    match pair_gap(&group(reference, ids), &group(pose, ids), pair, scale) {
        Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
        None => m.reviewed_anchor_missing = true,
    }
}

/// The fold check of rule 11: faces of a group that keep some vertices while
/// others move. A vertex on the hinge (`hinge`, exact source vertices) may stay
/// while the rest move; any other mix is a fold. Counts the faces that fold.
fn folds(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    hinge: &[[f32; 3]],
    scale: f32,
) -> usize {
    let before = keyed(reference);
    let after = keyed(pose);
    let mut count = 0;
    for (key, face) in before.iter().filter(|(k, _)| ids.contains(&k.0)) {
        let Some(moved) = after.get(key) else {
            count += 1;
            continue;
        };
        let (mut stayed, mut turned) = (false, false);
        for (a, b) in face.positions.iter().zip(&moved.positions) {
            if hinge.contains(a) {
                continue;
            }
            if distance(*a, *b) * scale > EPSILON {
                turned = true;
            } else {
                stayed = true;
            }
        }
        count += usize::from(stayed && turned);
    }
    count
}

/// Faces outside every moving group that changed at all: a fixed face must
/// never move with a control.
fn fixed_moved(reference: &[Face], pose: &[Face], movers: &[&[usize]], scale: f32) -> usize {
    let before = keyed(reference);
    let after = keyed(pose);
    before
        .iter()
        .filter(|(k, _)| !movers.iter().any(|ids| ids.contains(&k.0)))
        .filter(|(k, face)| {
            after.get(k).is_none_or(|moved| {
                moved.positions.len() != face.positions.len()
                    || face
                        .positions
                        .iter()
                        .zip(&moved.positions)
                        .any(|(a, b)| distance(*a, *b) * scale > EPSILON)
            })
        })
        .count()
}

fn unchanged(reference: &[Face], pose: &[Face], ids: &[usize], m: &mut Metrics) {
    let (old, new) = (group(reference, ids), group(pose, ids));
    for (key, face) in &old {
        match new.get(key) {
            Some(after) if face.positions == after.positions => {}
            _ => m.reviewed_neutral_mismatch = true,
        }
    }
}

fn lowest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min)
}

/// Tailerons: whole panels about the lateral spindle through the root chord
/// midpoint (which stays), the trailing edge rising for a positive net command.
fn tail(side: &Side, net: f32, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    panel(reference, pose, side.tail, scale, m);
    pair_anchor(reference, pose, side.tail, side.tail_root(), scale, m);
    if net == 0. {
        unchanged(reference, pose, side.tail, m);
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.tail, &[], scale);
    let (old, new) = (group(reference, side.tail), group(pose, side.tail));
    for witness in side.tail_edges() {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            let delta = (q[2] - witness[2]) * scale;
            m.reviewed_direction_failures += usize::from(delta * net.signum() <= EPSILON);
        }
    }
}

/// The flap strips: rigid, hinged on the front edge, trailing edge down.
fn strips(side: &Side, value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    panel(reference, pose, side.strip, scale, m);
    pair_anchor(reference, pose, side.strip, side.strip_outer(), scale, m);
    pair_anchor(reference, pose, side.strip, side.strip_root(), scale, m);
    if value == 0. {
        unchanged(reference, pose, side.strip, m);
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.strip, &[], scale);
    let witness = side.strip_trailing();
    let qs = f4::witness_positions(
        &group(reference, side.strip),
        &group(pose, side.strip),
        witness,
    );
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from((q[2] - witness[2]) * scale >= -EPSILON);
    }
}

fn rudder(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    panel(reference, pose, &RUDDER, scale, m);
    // The two vertices on the leading edge stay.
    let hinge = [sp([0., -31., 4.]), sp([0., -37., 18.])];
    for point in hinge {
        anchor(reference, pose, &RUDDER, point, scale, m);
    }
    if value == 0. {
        unchanged(reference, pose, &RUDDER, m);
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, &RUDDER, &hinge, scale);
    // Trailing vertices swing to +X for a positive command.
    let (old, new) = (group(reference, &RUDDER), group(pose, &RUDDER));
    for witness in [sp([0., -37., 4.]), sp([0., -40., 18.])] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures +=
                usize::from((q[0] - witness[0]) * value.signum() as f32 <= EPSILON);
        }
    }
}

/// A turned wing: every face of the panel moved as one rigid body, the tip went
/// aft and in, and no fixed face moved.
fn wing(side: &Side, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let (old, new) = (group(reference, side.wing), group(pose, side.wing));
    // The strips droop on their own with the flaps, so the rigid panel is
    // measured without them.
    let ids: Vec<usize> = side
        .wing
        .iter()
        .copied()
        .filter(|a| !side.strip.contains(a))
        .collect();
    panel(reference, pose, &ids, scale, m);
    let moved = old
        .iter()
        .any(|(k, f)| new.get(k).is_some_and(|n| n.positions != f.positions));
    if !moved {
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, &ids, &[], scale);
    let tip = side.tip();
    let qs = f4::witness_positions(&old, &new, tip);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        // Aft, and in toward the fuselage.
        m.reviewed_direction_failures += usize::from(q[1] >= tip[1] - EPSILON / scale);
        m.reviewed_direction_failures += usize::from(q[0].abs() >= tip[0].abs() - EPSILON / scale);
    }
}

/// The bottom and top of the airframe's skin at plan point (x, y), from the
/// neutral pose: the lowest and highest z of any face covering it. `None`
/// outside every face.
fn skin_span(neutral: &[Face], x: f32, y: f32) -> Option<(f32, f32)> {
    let mut range: Option<(f32, f32)> = None;
    for fixed in neutral {
        for i in 1..fixed.positions.len().saturating_sub(1) {
            let (a, b, c) = (
                fixed.positions[0],
                fixed.positions[i],
                fixed.positions[i + 1],
            );
            let d = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if d.abs() < 1e-9 {
                continue;
            }
            let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / d;
            let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / d;
            let l3 = 1. - l1 - l2;
            if l1.min(l2).min(l3) >= -1e-6 {
                let z = l1 * a[2] + l2 * b[2] + l3 * c[2];
                range = Some(range.map_or((z, z), |(lo, hi)| (lo.min(z), hi.max(z))));
            }
        }
    }
    range
}

/// How far a stowed piece reaches outside the neutral skin, in feet: every
/// vertex and edge midpoint of the group must lie between the lowest and
/// highest fixed face over its plan position; the worst excess (a point with no
/// face over it counts as 1000).
fn stow_overshoot(neutral: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
    let fixed: Vec<Face> = neutral
        .iter()
        .filter(|f| !GEAR.contains(&f.address))
        .cloned()
        .collect();
    let mut worst = 0f32;
    for face in pose.iter().filter(|f| ids.contains(&f.address)) {
        let n = face.positions.len();
        for i in 0..n {
            let (p, q) = (face.positions[i], face.positions[(i + 1) % n]);
            for s in [p, std::array::from_fn(|k| (p[k] + q[k]) / 2.)] {
                worst = worst.max(match skin_span(&fixed, s[0], s[1]) {
                    Some((lo, hi)) => (lo - s[2]).max(s[2] - hi).max(0.) * scale,
                    None => 1000.,
                });
            }
        }
    }
    worst
}

/// Gear: the main assembly and the nose leg are each one rigid piece, grounded
/// at 1, never lowered while closing, folding forward, hidden at 0.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    // The two leg tops at (x, -4, -4) lie on the main assembly's hinge axis and
    // stay; no other vertex of either piece does.
    let hinge = [sp([-8., -4., -4.]), sp([6., -4., -4.])];
    for (ids, on_hinge) in [(&MAIN[..], &hinge[..]), (&NOSE[..], &[][..])] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
        m.reviewed_direction_failures += folds(reference, pose, ids, on_hinge, scale);
    }
    for point in hinge {
        anchor(reference, pose, &MAIN, point, scale, m);
    }
    let (reference_low, pose_low) = (lowest(reference, &GEAR), lowest(pose, &GEAR));
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // Ground contact: the lowest point of the gear-down pose is the scaled
        // 11 unit low point the flight model stands the aircraft on.
        let want = sp([0., 0., -11.])[2];
        m.reviewed_wheel_failed |= (pose_low * scale - want * scale).abs() > 1e-3;
    } else {
        // Direction: both feet move forward as the gear folds.
        for (witness, ids) in [
            (sp([6., -4., -10.]), &MAIN[..]),
            (sp([0., 39., -11.]), &NOSE[..]),
        ] {
            let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), witness);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                m.reviewed_direction_failures += usize::from(q[1] <= witness[1]);
            }
        }
    }
}

/// The dorsal speed brake: four plates, one rigid piece a plate, hinged on the
/// front edge (which stays); open (1) is the source pose; shut they lie flat at
/// the hinge height.
fn brake(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| DORSAL.contains(&f.address));
        return;
    }
    let hinge = [
        sp([2., 0., 4.]),
        sp([8., 0., 4.]),
        sp([-2., 0., 4.]),
        sp([-8., 0., 4.]),
    ];
    for ids in [&DORSAL_R[..], &DORSAL_L[..]] {
        panel(reference, pose, ids, scale, m);
        m.reviewed_direction_failures += folds(reference, pose, ids, &hinge, scale);
    }
    for point in hinge {
        let ids = if point[0] > 0. { &DORSAL_R } else { &DORSAL_L };
        anchor(reference, pose, ids, point, scale, m);
    }
    if value < 1. {
        // The free edge comes down as the plate closes.
        for witness in [sp([8., -7., 9.]), sp([-8., -7., 9.])] {
            let qs = f4::witness_positions(&group(reference, &DORSAL), &keyed(pose), witness);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                m.reviewed_direction_failures += usize::from(q[2] >= witness[2] - EPSILON / scale);
            }
        }
    }
}

/// Each plume grows from the nozzle exit plane (y -36): the root edge is on the
/// plane at 0 and the tips pull forward to it, x and z never change.
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| FLAME.contains(&f.address));
        return;
    }
    let plane = sp([0., -36., 0.])[1];
    let (old, new) = (group(reference, &FLAME), group(pose, &FLAME));
    m.reviewed_wheel_failed |= old.len() != FLAME.len() || new.len() != FLAME.len();
    for (key, face) in &old {
        let Some(after) = new.get(key) else {
            continue;
        };
        for (a, b) in face.positions.iter().zip(&after.positions) {
            let want = plane + (a[1] - plane) * value as f32;
            m.reviewed_direction_failures +=
                usize::from((b[1] - want).abs() * scale > EPSILON || a[0] != b[0] || a[2] != b[2]);
        }
    }
}

pub(super) fn check(
    c: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    m: &mut Metrics,
) {
    let (_, reference, pose) = geometry;
    match c {
        Control::Elevator | Control::Aileron => {
            for side in &SIDES {
                // Elevator up raises both; a positive aileron raises the right.
                let net = if value == 0. {
                    0.
                } else if matches!(c, Control::Aileron) {
                    side.sign() * value.signum() as f32
                } else {
                    value.signum() as f32
                };
                tail(side, net, reference, pose, scale, m);
            }
            m.reviewed_neutral_mismatch |=
                fixed_moved(reference, pose, &[&TAIL_R, &TAIL_L], scale) != 0;
        }
        Control::Flaps => {
            for side in &SIDES {
                strips(side, value, reference, pose, scale, m);
            }
            m.reviewed_neutral_mismatch |=
                fixed_moved(reference, pose, &[&STRIP_R, &STRIP_L], scale) != 0;
        }
        Control::Rudder => {
            rudder(value, reference, pose, scale, m);
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&RUDDER], scale) != 0;
        }
        Control::Sweep => {
            for side in &SIDES {
                wing(side, reference, pose, scale, m);
            }
            m.reviewed_neutral_mismatch |=
                fixed_moved(reference, pose, &[&WING_R, &WING_L], scale) != 0;
            if value == 0. {
                m.reviewed_neutral_mismatch |= geometry_hash(reference) != geometry_hash(pose);
            }
        }
        Control::Gear => {
            gear(value, reference, pose, scale, m);
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&GEAR], scale) != 0;
        }
        Control::Brake => {
            brake(value, reference, pose, scale, m);
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&DORSAL], scale) != 0;
        }
        Control::Exhaust => {
            exhaust(value, reference, pose, scale, m);
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&FLAME], scale) != 0;
        }
        _ => {}
    }
}

fn passed(m: &Metrics) -> bool {
    m.finite
        && !m.reviewed_anchor_missing
        && m.max_reviewed_anchor_gap <= EPSILON
        && m.max_reviewed_skin_gap <= EPSILON
        && m.reviewed_rigid_panel_error <= EPSILON
        && m.reviewed_direction_failures == 0
        && !m.reviewed_neutral_mismatch
        && m.new_planar_crossings.is_empty()
}

fn at_knots(neutral: &State, knots: f64) -> State {
    let mut state = neutral.clone();
    state.speed = knots * 1.68781;
    state
}

/// Angle (radians) the wing tip turned about its pivot between two poses, the
/// witness that measures the sweep without repeating the schedule (the right
/// wing's clockwise turn and the left's counter-clockwise one are positive).
fn tip_turn(reference: &[Face], pose: &[Face], side: &Side) -> Option<f64> {
    let (old, new) = (group(reference, side.wing), group(pose, side.wing));
    let tip = side.tip();
    let q = *f4::witness_positions(&old, &new, tip).first()?;
    let pivot = side.pivot();
    let a = f64::from(tip[1] - pivot[1]).atan2(f64::from(tip[0] - pivot[0]));
    let b = f64::from(q[1] - pivot[1]).atan2(f64::from(q[0] - pivot[0]));
    let d = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    Some(if side.right { -d } else { d })
}

/// The smallest distance (feet) from a vertex of the moving groups `a` to a
/// triangle of the faces `b` and the other way, in `pose`: how close two
/// surfaces come.
fn closest(pose: &[Face], a: &[usize], b: &[usize], scale: f32) -> f32 {
    let one_way = |from: &[usize], to: &[usize]| {
        let mut best = f32::INFINITY;
        for face in pose.iter().filter(|f| from.contains(&f.address)) {
            for p in &face.positions {
                for t in pose.iter().filter(|f| to.contains(&f.address)) {
                    for i in 1..t.positions.len().saturating_sub(1) {
                        best = best.min(f117::triangle_distance(
                            *p,
                            t.positions[0],
                            t.positions[i],
                            t.positions[i + 1],
                        ));
                    }
                }
            }
        }
        best
    };
    one_way(a, b).min(one_way(b, a)) * scale
}

/// Plan-view overlap (any vertex of one group inside a face of the other),
/// the wing against the taileron, counted as vertices.
fn plan_inside(pose: &[Face], a: &[usize], b: &[usize]) -> usize {
    let inside = |p: [f32; 3], t: &Face| {
        (1..t.positions.len().saturating_sub(1)).any(|i| {
            let (u, v, w) = (t.positions[0], t.positions[i], t.positions[i + 1]);
            let s = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
                (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
            };
            let (d1, d2, d3) = (s(p, u, v), s(p, v, w), s(p, w, u));
            !((d1 < 0. || d2 < 0. || d3 < 0.) && (d1 > 0. || d2 > 0. || d3 > 0.))
        })
    };
    let count = |from: &[usize], to: &[usize]| {
        pose.iter()
            .filter(|f| from.contains(&f.address))
            .flat_map(|f| f.positions.iter())
            .filter(|p| {
                pose.iter()
                    .filter(|t| to.contains(&t.address))
                    .any(|t| inside(**p, t))
            })
            .count()
    };
    count(a, b) + count(b, a)
}

pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    // The 25-pose pitch and roll grid at the spread and the fully swept wing.
    for (knots, name) in [(200., "16deg"), (650., "72deg")] {
        let spread = at_knots(neutral, knots);
        let original = airframe.animation_faces(&spread);
        let mut rows = String::from(
            "pitch,roll,finite,anchor_gap_ft,skin_gap_ft,rigidity_ft,fixed_faces_moved,crossings,direction_failures\n",
        );
        let mut poses = Vec::new();
        for pitch in [-1., -0.5, 0., 0.5, 1.] {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                let mut state = spread.clone();
                state.elevator = pitch;
                state.aileron = roll;
                let pose = airframe.animation_faces(&state);
                let mut m = measure(&original, &pose, scale);
                for side in &SIDES {
                    // The law's weights are 0.20 for pitch and 0.10 for roll.
                    let net = (2. * pitch + f64::from(side.sign()) * roll) as f32;
                    tail(side, net, &original, &pose, scale, &mut m);
                }
                let fixed = fixed_moved(&original, &pose, &[&TAIL_R, &TAIL_L], scale);
                m.reviewed_neutral_mismatch |= fixed != 0;
                writeln!(
                    rows,
                    "{pitch},{roll},{},{},{},{},{fixed},{},{}",
                    m.finite,
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.reviewed_rigid_panel_error,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures
                )?;
                if !passed(&m) {
                    failures.push(format!(
                        "F-111F {name} pitch{pitch}/roll{roll}: whole-panel spindle, rigidity, fold or direction witness"
                    ));
                }
                poses.push(pose);
            }
        }
        fs::write(
            out.join(format!("pitch-roll-combinations-{name}.csv")),
            rows,
        )?;
        contact_sheet(
            &out.join(format!("pitch-roll-combinations-{name}.ppm")),
            &original,
            &poses,
        )?;
    }

    // The sweep against the speed schedule and the flaps: the wing panel
    // (without its strips, which droop on their own) stays rigid, the turn
    // follows the documented law, and the flaps hold the wing out.
    let original = airframe.animation_faces(&at_knots(neutral, 200.));
    let mut rows = String::from(
        "knots,flaps,expected_turn_deg,measured_turn_deg_right,measured_turn_deg_left,rigidity_ft,skin_gap_ft,passed\n",
    );
    let mut poses = Vec::new();
    for knots in [200., 250., 300., 400., 500., 600., 650., 700.] {
        for flaps in [0., 0.25, 0.5, 0.75, 1.] {
            let mut state = at_knots(neutral, knots);
            state.flaps = flaps;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            // Spread to 250 knots, fully swept (72.5 - 16 degrees) at 650.
            let expected = ((knots - 250.) / 400f64).clamp(0., 1.) * 56.5 * (1. - flaps);
            let mut measured = [0f64; 2];
            let mut rigid = 0f32;
            for (k, side) in SIDES.iter().enumerate() {
                let ids: Vec<usize> = side
                    .wing
                    .iter()
                    .copied()
                    .filter(|a| !side.strip.contains(a))
                    .collect();
                rigid = rigid.max(rigidity(&original, &pose, &ids, scale));
                panel(&original, &pose, &ids, scale, &mut m);
                measured[k] = tip_turn(&original, &pose, side)
                    .unwrap_or(f64::NAN)
                    .to_degrees();
            }
            let ok = m.finite
                && rigid <= EPSILON
                && m.max_reviewed_skin_gap <= EPSILON
                && m.new_planar_crossings.is_empty()
                && measured.iter().all(|t| (t - expected).abs() < 0.01);
            writeln!(
                rows,
                "{knots},{flaps},{expected:.3},{:.3},{:.3},{rigid},{},{ok}",
                measured[0], measured[1], m.max_reviewed_skin_gap
            )?;
            if !ok {
                failures.push(format!(
                    "F-111F sweep {knots} kt/flaps {flaps}: expected {expected:.2} deg, measured {:.2} / {:.2}, rigidity {rigid}",
                    measured[0], measured[1]
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("sweep-flap-combinations.csv"), rows)?;
    contact_sheet(&out.join("sweep-flap-combinations.ppm"), &original, &poses)?;

    // Root seam, swept span and taileron clearance at each sweep angle, in
    // feet. The seam is the distance from each root vertex of the swept panel
    // to the nearest fixed face (the leading one opens, the trailing one is
    // buried in the fuselage); the span is twice the farthest wing vertex
    // from the centreline; the clearance is the closest approach of the swept
    // panel to the taileron (neutral and at the pitch limits) and the count of
    // vertices of one inside the plan of the other.
    let fixed_ids = |faces: &[Face]| -> Vec<usize> {
        faces
            .iter()
            .map(|f| f.address)
            .filter(|a| {
                !WING_R.contains(a)
                    && !WING_L.contains(a)
                    && !TAIL_R.contains(a)
                    && !TAIL_L.contains(a)
                    && !GEAR.contains(a)
                    && !DORSAL.contains(a)
                    && !FLAME.contains(a)
                    && !RUDDER.contains(a)
            })
            .collect()
    };
    let fixed = fixed_ids(&original);
    let mut rows = String::from(
        "sweep_deg,side,root_lead_gap_ft,root_mid_gap_ft,root_trail_gap_ft,root_lead_travel_ft,span_ft,tail_gap_neutral_ft,tail_gap_up_ft,tail_gap_down_ft,plan_vertices_inside\n",
    );
    for degrees in [16., 20., 30., 40., 45., 50., 60., 70., 72.5] {
        let knots = tore_sim::combat::swing::F111F.speed_for_sweep(degrees) / 1.68781;
        let pose = airframe.animation_faces(&at_knots(neutral, knots));
        let tail_pose = |elevator: f64| {
            let mut state = at_knots(neutral, knots);
            state.elevator = elevator;
            airframe.animation_faces(&state)
        };
        let (up, down) = (tail_pose(1.), tail_pose(-1.));
        let span = 2.
            * pose
                .iter()
                .filter(|f| WING_R.contains(&f.address) || WING_L.contains(&f.address))
                .flat_map(|f| f.positions.iter())
                .map(|p| p[0].abs())
                .fold(0f32, f32::max)
            * scale;
        for side in &SIDES {
            let old = group(&original, side.wing);
            let new = group(&pose, side.wing);
            let mut gaps = [0f32; 3];
            let mut travel = 0f32;
            for (k, root) in side.root().into_iter().enumerate() {
                if let Some(q) = f4::witness_positions(&old, &new, root).first() {
                    if k == 0 {
                        travel = distance(root, *q) * scale;
                    }
                    let mut nearest = f32::INFINITY;
                    for face in pose.iter().filter(|f| fixed.contains(&f.address)) {
                        for i in 1..face.positions.len().saturating_sub(1) {
                            nearest = nearest.min(f117::triangle_distance(
                                *q,
                                face.positions[0],
                                face.positions[i],
                                face.positions[i + 1],
                            ));
                        }
                    }
                    gaps[k] = nearest * scale;
                }
            }
            let wing_ids = side.wing;
            let tail_ids = side.tail;
            let (gap0, gap_up, gap_down) = (
                closest(&pose, wing_ids, tail_ids, scale),
                closest(&up, wing_ids, tail_ids, scale),
                closest(&down, wing_ids, tail_ids, scale),
            );
            let inside = plan_inside(&pose, wing_ids, tail_ids);
            writeln!(
                rows,
                "{degrees},{},{},{},{},{travel},{span},{gap0},{gap_up},{gap_down},{inside}",
                if side.right { "right" } else { "left" },
                gaps[0],
                gaps[1],
                gaps[2]
            )?;
            // The swept panel never enters the taileron: no vertex of one in
            // the plan of the other and the closest approach stays clear.
            if inside > 0 || gap0.min(gap_up).min(gap_down) < 0.5 {
                failures.push(format!(
                    "F-111F wing enters the taileron at {degrees} deg ({inside} vertices inside, closest {:.2} ft)",
                    gap0.min(gap_up).min(gap_down)
                ));
            }
        }
    }
    fs::write(out.join("sweep-seam-span-taileron.csv"), rows)?;

    // Taileron forward corner swing at the limit, and the seam openings of the
    // strip and rudder corners at full travel.
    let mut rows = String::from("surface,command,corner,swing_ft\n");
    let corner = |ids: &[usize], pose: &[Face], point: [f32; 3]| -> f32 {
        f4::witness_positions(&group(&original, ids), &keyed(pose), point)
            .first()
            .map_or(f32::NAN, |q| distance(point, *q) * scale)
    };
    for (name, elevator, aileron) in [
        ("pitch-up", 1., 0.),
        ("pitch-down", -1., 0.),
        ("roll-right", 0., 1.),
        ("pitch-up+roll-right", 1., 1.),
    ] {
        let mut state = at_knots(neutral, 200.);
        state.elevator = elevator;
        state.aileron = aileron;
        let pose = airframe.animation_faces(&state);
        for side in &SIDES {
            for (label, point) in [
                ("root leading (10,-17)", [10., -17., 2.]),
                ("root trailing (8,-30)", [8., -30., 2.]),
                ("tip trailing (21,-39)", [21., -39., 2.]),
            ] {
                writeln!(
                    rows,
                    "taileron-{},{name},{label},{}",
                    if side.right { "right" } else { "left" },
                    corner(side.tail, &pose, side.at(point))
                )?;
            }
        }
    }
    fs::write(out.join("taileron-corner-swing.csv"), rows)?;

    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_brake(airframe, neutral, out, scale)?);
    failures.extend(dense_exhaust(airframe, neutral, out, scale)?);
    Ok(failures)
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, whole, grounded at 1, folding forward, and near stow each piece
/// inside the airframe's skin within its tolerance.
fn dense_gear(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut gear_endpoint = neutral.clone();
    Control::Gear.apply(&mut gear_endpoint, 1.);
    let reference = airframe.animation_faces(&gear_endpoint);
    let mut csv = String::from(
        "value,rigidity_ft,crossings,main_overshoot_ft,nose_overshoot_ft,checks_passed\n",
    );
    let mut values: Vec<_> = std::iter::once(1e-6)
        .chain((0..=200).map(|i| f64::from(i) / 200.))
        .collect();
    values.sort_by(f64::total_cmp);
    let mut poses = Vec::new();
    for (i, value) in values.into_iter().enumerate() {
        let mut state = neutral.clone();
        state.gear = value;
        let pose = airframe.animation_faces(&state);
        let mut m = measure(&reference, &pose, scale);
        check(
            Control::Gear,
            value,
            (original, &reference, &pose),
            scale,
            &mut m,
        );
        // Near stow (every piece folded as far as it goes) the pieces lie
        // inside the skin within their tolerance, the nose leg's all inside.
        let (main, nose) = if value > 0. && value < 1e-5 {
            (
                stow_overshoot(original, &pose, &MAIN, scale),
                stow_overshoot(original, &pose, &NOSE, scale),
            )
        } else {
            (0., 0.)
        };
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && main <= STOW_MAIN_FT
            && nose <= STOW_NOSE_FT
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{main},{nose},{passed}",
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "F-111F dense gear value{value}: rigid, direction, stow or ground gate"
            ));
        }
        if i <= 1 || i % 10 == 1 {
            poses.push(pose);
        }
    }
    fs::write(out.join("dense-gear.csv"), csv)?;
    contact_sheet(&out.join("dense-gear.ppm"), &reference, &poses)?;
    Ok(failures)
}

/// Dorsal plate sweep: every pose rigid with the hinge in place, the open pose
/// is the source plate, and the shut pose (a near-zero sample) lays the free
/// edge flat at the hinge height.
fn dense_brake(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut open = neutral.clone();
    Control::Brake.apply(&mut open, 1.);
    let reference = airframe.animation_faces(&open);
    let mut csv = String::from("value,anchor_gap_ft,rigidity_ft,free_edge_over_hinge_ft,passed\n");
    let hinge_z = sp([0., 0., 4.])[2];
    for value in [0., 1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.brake = value;
        let pose = airframe.animation_faces(&state);
        let mut m = measure(&reference, &pose, scale);
        brake(value, &reference, &pose, scale, &mut m);
        let free = pose
            .iter()
            .filter(|f| DORSAL.contains(&f.address))
            .flat_map(|f| f.positions.iter())
            .map(|p| p[2])
            .fold(f32::MIN, f32::max);
        let over = if value > 0. {
            (free - hinge_z) * scale
        } else {
            0.
        };
        let shut_flat = value >= 1e-3 || over.abs() < 1e-3;
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.reviewed_rigid_panel_error <= EPSILON
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && shut_flat;
        writeln!(
            csv,
            "{value},{},{},{over},{passed}",
            m.max_reviewed_anchor_gap, m.reviewed_rigid_panel_error
        )?;
        if !passed {
            failures.push(format!(
                "F-111F dorsal brake value{value}: rigid, hinge, direction or shut-flat gate"
            ));
        }
    }
    fs::write(out.join("brake-sweep.csv"), csv)?;
    Ok(failures)
}

/// Exhaust sweep: both plumes grow from the exit plane to their full length at
/// 1, the root edge on the plane and the tips at `25 * value` units behind it.
fn dense_exhaust(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut open = neutral.clone();
    Control::Exhaust.apply(&mut open, 1.);
    let reference = airframe.animation_faces(&open);
    let mut csv = String::from("value,direction_failures,rigidity_ft,passed\n");
    for value in [0., 1e-6, 0.1, 0.25, 0.5, 0.75, 1.] {
        let mut state = neutral.clone();
        state.exhaust = value;
        let pose = airframe.animation_faces(&state);
        let mut m = measure(&reference, &pose, scale);
        exhaust(value, &reference, &pose, scale, &mut m);
        let passed = m.finite && m.reviewed_direction_failures == 0 && !m.reviewed_wheel_failed;
        writeln!(
            csv,
            "{value},{},{},{passed}",
            m.reviewed_direction_failures, m.max_edge_change
        )?;
        if !passed {
            failures.push(format!("F-111F exhaust value{value}: growth gate"));
        }
    }
    fs::write(out.join("exhaust-sweep.csv"), csv)?;
    Ok(failures)
}
