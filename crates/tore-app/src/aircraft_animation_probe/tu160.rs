//! Independent Tu-160 whole-panel witnesses (T160-3): the two swing-wing panels
//! (sweep, detents, span, root seam), the two tailplane quads (one angle per
//! side, pitch plus differential roll), the rudder strip, the two flap strips
//! and their end caps, the two keel plates, the twelve gear plates in three rigid
//! legs and the sixteen plume plates. Measures the shared Airframe output and
//! never repeats the rig's algorithm; the sweep law and the span targets are
//! restated here from the published figures.
use super::*;

const WING_R: [usize; 16] = [
    0x4952, 0x4975, 0x498e, 0x49af, 0x49d6, 0x49f9, 0x4a22, 0x4a49, 0x4a6c, 0x4a8f, 0x4ab0, 0x4ad3,
    0x4b1c, 0x4b45, 0x4b93, 0x4bba,
];
const WING_L: [usize; 16] = [
    0x4e8e, 0x4eaf, 0x4ed8, 0x4f01, 0x4f24, 0x4f47, 0x4f60, 0x4f87, 0x4faa, 0x4fcd, 0x4fee, 0x5011,
    0x5040, 0x5067, 0x50b4, 0x50db,
];
const STRIP_R: [usize; 2] = [0x4b93, 0x4bba];
const STRIP_L: [usize; 2] = [0x50b4, 0x50db];
const TAIL_R: [usize; 2] = [0x1daf, 0x3e5d];
const TAIL_L: [usize; 2] = [0x1dce, 0x3e7c];
const RUDDER: [usize; 2] = [0x52e0, 0x5307];
const KEEL: [usize; 4] = [0x4739, 0x4750, 0x4794, 0x47ab];
const NOSE: [usize; 4] = [0x5515, 0x5534, 0x5553, 0x5572];
const MAIN_R: [usize; 4] = [0x5373, 0x539a, 0x53c1, 0x53e0];
const MAIN_L: [usize; 4] = [0x5444, 0x546b, 0x5492, 0x54b1];
const GEAR: [usize; 12] = [
    0x5515, 0x5534, 0x5553, 0x5572, 0x5373, 0x539a, 0x53c1, 0x53e0, 0x5444, 0x546b, 0x5492, 0x54b1,
];
const FLAME: [usize; 16] = [
    0x5658, 0x567f, 0x56a6, 0x56cd, 0x56f4, 0x571b, 0x5742, 0x5769, 0x5790, 0x57b7, 0x57de, 0x5805,
    0x582c, 0x5853, 0x587a, 0x58a1,
];
const CAPS: [usize; 8] = [
    0x8110, 0x8111, 0x8112, 0x8113, 0x8114, 0x8115, 0x8116, 0x8117,
];
/// Feet a shape unit is (the header scale).
const FEET: f32 = 8. / 3.;
/// The fixed glove plates the rig adds (T160-5), restated.
const GLOVE: [usize; 4] = [0x8118, 0x8119, 0x811a, 0x811b];
/// The wing pivots (shape units), restated from the fitted spans.
const PIVOT_R: [f32; 2] = [4.8, 0.];
const PIVOT_L: [f32; 2] = [-4.8, 0.];
/// The five root vertex positions of a panel (x, y) in the spread pose and the
/// two heights each is drawn at, right-hand side.
const ROOT: [[f32; 2]; 5] = [[8., 2.], [9., 4.], [10., -2.], [11., -2.], [11., -5.]];

struct Side {
    right: bool,
    wing: &'static [usize],
    strip: &'static [usize],
    tail: &'static [usize],
}
const SIDES: [Side; 2] = [
    Side {
        right: true,
        wing: &WING_R,
        strip: &STRIP_R,
        tail: &TAIL_R,
    },
    Side {
        right: false,
        wing: &WING_L,
        strip: &STRIP_L,
        tail: &TAIL_L,
    },
];
impl Side {
    fn sign(&self) -> f32 {
        if self.right { 1. } else { -1. }
    }
    fn fix(&self, p: [f32; 3]) -> [f32; 3] {
        [p[0] * self.sign(), p[1], p[2]]
    }
    fn pivot(&self) -> [f32; 2] {
        if self.right { PIVOT_R } else { PIVOT_L }
    }
    /// The wing tip vertex whose turn measures the sweep.
    fn tip(&self) -> [f32; 3] {
        self.fix([34., -7., -1.])
    }
    /// The tailplane's trailing tip vertex (a witness on the free edge).
    fn tail_tip(&self) -> [f32; 3] {
        self.fix([8., -28., 7.])
    }
    /// The root chord ends (a pair; their midpoint is the lateral spindle).
    fn tail_root(&self) -> ([f32; 3], [f32; 3]) {
        ([0., -26., 7.], [0., -18., 7.])
    }
    /// The strip's leading-edge pairs (top and bottom) at the root and the tip;
    /// the midpoint of each pair is on the hinge line.
    fn strip_pairs(&self) -> [([f32; 3], [f32; 3]); 2] {
        [
            (self.fix([11., -2., 0.]), self.fix([11., -2., -1.])),
            (self.fix([32., -6., 0.]), self.fix([32., -6., -1.])),
        ]
    }
    fn strip_trailing(&self) -> [f32; 3] {
        self.fix([11., -5., -1.])
    }
}

fn group<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

/// Largest change of any vertex-to-vertex distance inside one group.
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

fn lowest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min)
}

fn present(pose: &[Face], ids: &[usize]) -> usize {
    pose.iter().filter(|f| ids.contains(&f.address)).count()
}

/// Where a vertex of a group went (the first witness), if it is still there.
fn follow(reference: &[Face], pose: &[Face], ids: &[usize], point: [f32; 3]) -> Option<[f32; 3]> {
    f4::witness_positions(&group(reference, ids), &keyed(pose), point)
        .first()
        .copied()
}

/// Distance the midpoint of one pair of vertices moved, feet.
fn pair_gap(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    pair: ([f32; 3], [f32; 3]),
    scale: f32,
) -> Option<f32> {
    let (a, b) = (
        follow(reference, pose, ids, pair.0)?,
        follow(reference, pose, ids, pair.1)?,
    );
    let middle: [f32; 3] = std::array::from_fn(|k| (pair.0[k] + pair.1[k]) / 2.);
    let now: [f32; 3] = std::array::from_fn(|k| (a[k] + b[k]) / 2.);
    Some(distance(middle, now) * scale)
}

fn anchor(m: &mut Metrics, gap: Option<f32>) {
    match gap {
        Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
        None => m.reviewed_anchor_missing = true,
    }
}

fn rigid(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let error = rigidity(reference, pose, ids, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
}

/// Number of faces of `ids` (by address) whose vertices differ between two poses.
fn changed(reference: &[Face], pose: &[Face], ids: &[usize]) -> usize {
    let after = keyed(pose);
    keyed(reference)
        .iter()
        .filter(|(k, f)| {
            ids.contains(&k.0) && after.get(k).is_none_or(|a| a.positions != f.positions)
        })
        .count()
}

/// Faces outside every moving group that moved: a fixed face must not.
fn fixed_moved(reference: &[Face], pose: &[Face], movers: &[&[usize]]) -> usize {
    let after = keyed(pose);
    keyed(reference)
        .iter()
        .filter(|(k, f)| {
            !movers.iter().any(|ids| ids.contains(&k.0))
                && after.get(k).is_some_and(|a| a.positions != f.positions)
        })
        .count()
}

// The sweep law, restated from the published figures (TU160.INF: 20, 35 and 65
// degrees) and the decided speed bands, to measure the drawing against.
fn smooth(t: f64) -> f64 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}
fn expected_sweep(knots: f64, flaps: f64) -> f64 {
    let command = if knots <= 250. {
        20.
    } else if knots < 320. {
        20. + 15. * smooth((knots - 250.) / 70.)
    } else if knots <= 520. {
        35.
    } else if knots < 600. {
        35. + 30. * smooth((knots - 520.) / 80.)
    } else {
        65.
    };
    20. + (command - 20.) * (1. - flaps.clamp(0., 1.))
}

/// Angle (degrees, tip aft positive) the wing tip turned about its pivot.
fn tip_turn(reference: &[Face], pose: &[Face], side: &Side) -> Option<f64> {
    let tip = side.tip();
    let q = follow(reference, pose, side.wing, tip)?;
    let pivot = side.pivot();
    let a = f64::from(tip[1] - pivot[1]).atan2(f64::from(tip[0] - pivot[0]));
    let b = f64::from(q[1] - pivot[1]).atan2(f64::from(q[0] - pivot[0]));
    let d = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    Some((if side.right { -d } else { d }).to_degrees())
}

/// Swept span in metres: twice the outermost x of both wing panels' vertices
/// (the fuselage is narrower than the wing, so the panels set the span).
fn span_metres(pose: &[Face]) -> f64 {
    let mut lo = f32::INFINITY;
    let mut hi = f32::NEG_INFINITY;
    for f in pose
        .iter()
        .filter(|f| WING_R.contains(&f.address) || WING_L.contains(&f.address))
    {
        for p in &f.positions {
            lo = lo.min(p[0]);
            hi = hi.max(p[0]);
        }
    }
    f64::from(hi - lo) * f64::from(FEET) * 0.3048
}

/// Tailplane: each quad rigid, the root chord midpoint (the lateral spindle)
/// stays, and the trailing tip moves the way the command says.
fn tail(side: &Side, net: f32, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    rigid(reference, pose, side.tail, scale, m);
    let (a, b) = side.tail_root();
    anchor(m, pair_gap(reference, pose, side.tail, (a, b), scale));
    let tip = side.tail_tip();
    match follow(reference, pose, side.tail, tip) {
        None => m.reviewed_direction_failures += 1,
        Some(q) => {
            let delta = (q[2] - tip[2]) * scale;
            if net == 0. {
                m.reviewed_neutral_mismatch |= delta.abs() > EPSILON;
            } else {
                m.reviewed_direction_failures += usize::from(delta * net <= EPSILON);
            }
        }
    }
}

/// The rudder strip: rigid, the two leading-edge vertices stay, the trailing
/// base swings toward +X for a positive pedal.
fn rudder(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    rigid(reference, pose, &RUDDER, scale, m);
    for hinge in [[0., -24., 7.], [0., -25., 14.]] {
        let moved = follow(reference, pose, &RUDDER, hinge);
        anchor(m, moved.map(|q| distance(hinge, q) * scale));
    }
    let base = [0., -26., 7.];
    match follow(reference, pose, &RUDDER, base) {
        None => m.reviewed_direction_failures += 1,
        Some(q) => {
            let dx = (q[0] - base[0]) * scale;
            if value == 0. {
                m.reviewed_neutral_mismatch |= dx.abs() > EPSILON;
            } else {
                m.reviewed_direction_failures += usize::from(dx * value.signum() as f32 <= EPSILON);
            }
        }
    }
}

/// The flap strips: each rigid, the leading-edge hinge pairs stay, the trailing
/// edge drops with the lever, and the end caps draw exactly while drooped.
fn flaps(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for side in &SIDES {
        rigid(reference, pose, side.strip, scale, m);
        for pair in side.strip_pairs() {
            anchor(m, pair_gap(reference, pose, side.strip, pair, scale));
        }
        let edge = side.strip_trailing();
        match follow(reference, pose, side.strip, edge) {
            None => m.reviewed_direction_failures += 1,
            Some(q) => {
                let drop = (edge[2] - q[2]) * scale;
                if value == 0. {
                    m.reviewed_neutral_mismatch |= drop.abs() > EPSILON;
                } else {
                    m.reviewed_direction_failures += usize::from(drop <= EPSILON);
                }
            }
        }
    }
    let caps = present(pose, &CAPS);
    m.reviewed_direction_failures += usize::from(caps != if value > 0. { 8 } else { 0 });
}

/// A turned wing: every face of the panel moved as one rigid body (strips
/// included), the tip went aft and in, and no fixed face moved.
fn wing(side: &Side, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    rigid(reference, pose, side.wing, scale, m);
    if changed(reference, pose, side.wing) == 0 {
        return;
    }
    let tip = side.tip();
    match follow(reference, pose, side.wing, tip) {
        None => m.reviewed_direction_failures += 1,
        Some(q) => {
            // Aft, and in toward the fuselage.
            m.reviewed_direction_failures += usize::from(q[1] >= tip[1] - EPSILON / scale);
            m.reviewed_direction_failures +=
                usize::from(q[0].abs() >= tip[0].abs() - EPSILON / scale);
        }
    }
    // The height never changes: a vertical axis.
    for f in pose.iter().filter(|f| side.wing.contains(&f.address)) {
        let old = reference
            .iter()
            .find(|o| o.address == f.address)
            .map(|o| o.positions.clone())
            .unwrap_or_default();
        for (a, b) in old.iter().zip(&f.positions) {
            m.reviewed_direction_failures += usize::from((a[2] - b[2]).abs() > 1e-4);
        }
    }
}

/// Keel plates: drawn only with the brake lever; the top edge stays on the
/// belly (z 0) and the plate lowers with the lever.
fn keel(value: f64, pose: &[Face], reference: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures += usize::from(present(pose, &KEEL) != 0);
        return;
    }
    m.reviewed_direction_failures += usize::from(present(pose, &KEEL) != 4);
    // The probe's reference is brake 1 (the source pose).
    rigid_scaled(reference, pose, scale, m);
    let want = -2. * value as f32;
    m.reviewed_direction_failures += usize::from((lowest(pose, &KEEL) - want).abs() > 1e-3);
    for p in pose
        .iter()
        .filter(|f| KEEL.contains(&f.address))
        .flat_map(|f| &f.positions)
    {
        m.reviewed_direction_failures += usize::from(p[2] > 1e-4);
    }
}

/// The plates scale in z with the lever, so they are not rigid; check instead
/// that their x and y never change.
fn rigid_scaled(reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let after = keyed(pose);
    for (key, face) in keyed(reference).iter().filter(|(k, _)| KEEL.contains(&k.0)) {
        if let Some(a) = after.get(key) {
            for (p, q) in face.positions.iter().zip(&a.positions) {
                m.reviewed_rigid_panel_error = m
                    .reviewed_rigid_panel_error
                    .max(((p[0] - q[0]).abs().max((p[1] - q[1]).abs())) * scale);
            }
        }
    }
}

/// The plume: plates scale from the nozzle plane (y -11) and exist only with
/// the lever; the tip is at y -11 - 11 * value.
fn exhaust(value: f64, pose: &[Face], m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures += usize::from(present(pose, &FLAME) != 0);
        return;
    }
    m.reviewed_direction_failures += usize::from(present(pose, &FLAME) != 16);
    let tip = pose
        .iter()
        .filter(|f| FLAME.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[1])
        .fold(f32::INFINITY, f32::min);
    let want = -11. - 11. * value as f32;
    m.reviewed_direction_failures += usize::from((tip - want).abs() > 1e-3);
    let root = pose
        .iter()
        .filter(|f| FLAME.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[1])
        .fold(f32::NEG_INFINITY, f32::max);
    m.reviewed_direction_failures += usize::from((root + 11.).abs() > 1e-3);
}

/// The skin height range above and below a plan point in the neutral pose.
fn skin_span(neutral: &[Face], x: f32, y: f32) -> Option<(f32, f32)> {
    let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
    for face in neutral {
        let p = &face.positions;
        let mut inside = false;
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            if (a[1] > y) != (b[1] > y) && x < (b[0] - a[0]) * (y - a[1]) / (b[1] - a[1]) + a[0] {
                inside = !inside;
            }
        }
        if !inside {
            continue;
        }
        let (p0, mut plane) = (p[0], None);
        for i in 1..p.len().saturating_sub(1) {
            let u = [p[i][0] - p0[0], p[i][1] - p0[1], p[i][2] - p0[2]];
            let v = [
                p[i + 1][0] - p0[0],
                p[i + 1][1] - p0[1],
                p[i + 1][2] - p0[2],
            ];
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            if n[2].abs() > 1e-6 {
                plane = Some(n);
                break;
            }
        }
        if let Some(n) = plane {
            let z = p0[2] - (n[0] * (x - p0[0]) + n[1] * (y - p0[1])) / n[2];
            lo = lo.min(z);
            hi = hi.max(z);
        }
    }
    lo.is_finite().then_some((lo, hi))
}

/// Stowed pieces lie inside the airframe: every vertex and edge midpoint of the
/// group sits between the neutral skin's bottom and top there. The worst
/// overshoot in shape units (0 inside; infinity where there is no skin).
fn hull_overshoot(neutral: &[Face], pose: &[Face], ids: &[usize]) -> f32 {
    let mut worst = 0f32;
    for face in pose.iter().filter(|f| ids.contains(&f.address)) {
        let p = &face.positions;
        for i in 0..p.len() {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            for t in [0f32, 0.5] {
                let q: [f32; 3] = std::array::from_fn(|k| a[k] + (b[k] - a[k]) * t);
                let x = q[0] - q[0].signum() * 0.1;
                worst = worst.max(match skin_span(neutral, x, q[1] + 1e-3) {
                    Some((lo, hi)) => (lo - q[2]).max(q[2] - hi).max(0.),
                    None => f32::INFINITY,
                });
            }
        }
    }
    worst
}

/// The gear: three whole rigid legs, gear down is the ground contact (z -5),
/// closing never lowers a plate (0.1 unit allowed), the nose foot goes aft and the main feet go
/// outboard. Nothing draws at zero.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= present(pose, &GEAR) != 0;
        return;
    }
    for ids in [&NOSE[..], &MAIN_R[..], &MAIN_L[..]] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
    }
    let (reference_low, pose_low) = (lowest(reference, &GEAR), lowest(pose, &GEAR));
    // A main's cross plate swings its inboard bottom corner up to 0.08 unit
    // (0.22 ft) below the contact line over the last fifth of the arrival
    // (it hinges on the long plate, not its own edge): allowed to 0.1 unit.
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - 0.1;
    if value == 1. {
        m.reviewed_wheel_failed |= (pose_low + 5.).abs() > 1e-3;
        return;
    }
    // A main's plate foot moves outboard (into the glove, where the real legs
    // stow) and the nose leg's foot aft, or stays at the source pose once that
    // leg is down; never the other way.
    for (ids, witness, outward) in [
        (&MAIN_R[..], [1., -4., -5.], 1f32),
        (&MAIN_L[..], [-1., -4., -5.], -1.),
    ] {
        let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures +=
                usize::from((q[0] - witness[0]) * outward < -EPSILON / scale);
        }
    }
    let foot = [0., 20., -5.];
    let qs = f4::witness_positions(&group(reference, &NOSE), &keyed(pose), foot);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from(q[1] > foot[1] + EPSILON / scale);
    }
    // Near stow the legs have left the source pose altogether.
    if value < 0.05 {
        let moved = changed(reference, pose, &GEAR);
        m.reviewed_direction_failures += usize::from(moved != GEAR.len());
    }
}

/// The first lining address and one bay's lining faces (a ceiling, four walls and
/// the five faces of its rail), restated from the rig.
const LINING_BASE: usize = 0x8200;
const LINING: usize = 2 * 10;
/// The fitted bays, restated: forward bay then aft, feet forward (aft end, fore
/// end) and the belly plane's height in shape units (the keel skin at -1, the
/// flat belly at 0), half-width 3.8 ft.
const BAYS: [([f32; 2], f32); 2] = [([14., 56.], -1.), ([-112. / 3., 14. / 3.], 0.)];
const BAY_HALF_FT: f32 = 3.8;

/// The fitted bays: shut (0) the belly is the shape's own and no lining draws;
/// open (above 0) the lining draws (two bays of ten faces) and, from a sixth of the
/// travel on (the leaves have flattened), the lowest vertex over each bay is the
/// free edge of a door leaf, `3.8 ft * sin(90 degrees * value)` under the bay's
/// belly plane; fully open every vertex of every leaf hangs on a hinge line
/// (x +/-3.8 ft) within the leaf's own width.
fn bay(value: f64, _reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let lining = pose
        .iter()
        .filter(|f| (LINING_BASE..LINING_BASE + 0x80).contains(&f.address))
        .count();
    if value == 0. {
        // Shut: no lining, and no face hangs below a bay's belly plane over it
        // (the reference of this control is the open pose, so the shut skin is
        // checked on its own).
        let hanging = BAYS.iter().any(|&(feet, belly)| {
            pose.iter().any(|f| {
                !(LINING_BASE..LINING_BASE + 0x80).contains(&f.address)
                    && f.positions.iter().all(|p| {
                        p[0].abs() <= BAY_HALF_FT / scale
                            && p[1] >= feet[0] / scale
                            && p[1] <= feet[1] / scale
                    })
                    && f.positions.iter().any(|p| p[2] < belly - 1e-3)
            })
        });
        m.reviewed_neutral_mismatch |= lining != 0 || hanging;
        return;
    }
    m.reviewed_direction_failures += usize::from(lining != LINING);
    let half = BAY_HALF_FT / scale;
    for (feet, belly) in BAYS {
        let (aft, fore) = (feet[0] / scale, feet[1] / scale);
        // The faces over this bay: every drawn face inside its plan box except
        // the lining and the fixed glove plates; the leaves are the ones the rig
        // moved, which hang at the hinge lines.
        let over = |f: &&Face| {
            !(LINING_BASE..LINING_BASE + 0x80).contains(&f.address)
                && f.positions
                    .iter()
                    .all(|p| p[0].abs() <= half + 1e-3 && p[1] >= aft - 1e-3 && p[1] <= fore + 1e-3)
        };
        let faces: Vec<&Face> = pose.iter().filter(over).collect();
        // A vertex stays on each hinge line at the belly plane.
        for side in [-half, half] {
            let held = faces.iter().flat_map(|f| f.positions.iter()).any(|p| {
                (p[0] - side).abs() < 1e-3
                    && (p[2] - belly).abs() < 1e-3
                    && p[1] > aft
                    && p[1] < fore
            });
            m.reviewed_anchor_missing |= !held;
        }
        let lowest = faces
            .iter()
            .flat_map(|f| f.positions.iter())
            .map(|p| p[2])
            .fold(f32::MAX, f32::min);
        if value >= 1. / 6. {
            let hang = half * (value as f32 * std::f32::consts::FRAC_PI_2).sin();
            m.reviewed_direction_failures += usize::from((belly - lowest - hang).abs() > 2e-3);
        }
        if value >= 1. {
            // Fully open: the hung leaves are two or more faces on the hinge
            // lines, no leaf vertex off them.
            let hung: Vec<&&Face> = faces
                .iter()
                .filter(|f| f.positions.iter().any(|p| p[2] < belly - 0.5))
                .collect();
            m.reviewed_direction_failures += usize::from(hung.len() < 2);
            for f in hung {
                m.reviewed_direction_failures +=
                    usize::from(f.positions.iter().any(|p| (p[0].abs() - half).abs() > 2e-3));
            }
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
                // A pull raises both trailing edges; a positive aileron raises
                // the right and lowers the left.
                let net = if value == 0. {
                    0.
                } else if matches!(c, Control::Aileron) {
                    side.sign() * value.signum() as f32
                } else {
                    value.signum() as f32
                };
                tail(side, net, reference, pose, scale, m);
            }
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&TAIL_R, &TAIL_L]) != 0;
        }
        Control::Flaps => {
            flaps(value, reference, pose, scale, m);
            m.reviewed_neutral_mismatch |=
                fixed_moved(reference, pose, &[&STRIP_R, &STRIP_L, &CAPS]) != 0;
        }
        Control::Rudder => {
            rudder(value, reference, pose, scale, m);
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&RUDDER]) != 0;
        }
        Control::Sweep => {
            for side in &SIDES {
                wing(side, reference, pose, scale, m);
            }
            m.reviewed_neutral_mismatch |= fixed_moved(reference, pose, &[&WING_R, &WING_L]) != 0;
            if value == 0. {
                m.reviewed_neutral_mismatch |= geometry_hash(reference) != geometry_hash(pose);
            }
        }
        Control::Gear => gear(value, reference, pose, scale, m),
        Control::Brake => keel(value, pose, reference, scale, m),
        Control::Exhaust => exhaust(value, pose, m),
        Control::Bay => bay(value, reference, pose, scale, m),
        _ => {}
    }
}

fn passed(m: &Metrics) -> bool {
    m.finite
        && !m.reviewed_anchor_missing
        && m.max_reviewed_anchor_gap <= EPSILON
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

/// Smallest distance in plan (feet) from a point to the nearest fixed face's
/// projection, 0 inside it, with the vertical offset (feet, positive above the
/// skin) where it is covered.
fn plan_cover(fixed: &[Face], p: [f32; 3]) -> (f32, f32) {
    if let Some((lo, hi)) = skin_span(fixed, p[0], p[1] + 1e-3) {
        let over = if p[2] > hi {
            p[2] - hi
        } else if p[2] < lo {
            p[2] - lo
        } else {
            0.
        };
        return (0., over * FEET);
    }
    let mut best = f32::INFINITY;
    for face in fixed {
        let q = &face.positions;
        for i in 0..q.len() {
            let (a, b) = (q[i], q[(i + 1) % q.len()]);
            let ab = [b[0] - a[0], b[1] - a[1]];
            let ap = [p[0] - a[0], p[1] - a[1]];
            let length = ab[0] * ab[0] + ab[1] * ab[1];
            let t = if length > 0. {
                ((ap[0] * ab[0] + ap[1] * ab[1]) / length).clamp(0., 1.)
            } else {
                0.
            };
            let d = (ap[0] - t * ab[0]).hypot(ap[1] - t * ab[1]);
            best = best.min(d);
        }
    }
    (best * FEET, 0.)
}

pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    // The 25-pose pitch and roll grid at the spread and the swept wing.
    for (knots, name) in [(200., "20deg"), (700., "65deg")] {
        let spread = at_knots(neutral, knots);
        let original = airframe.animation_faces(&spread);
        let mut rows = String::from(
            "pitch,roll,finite,anchor_gap_ft,rigidity_ft,fixed_faces_moved,crossings,direction_failures\n",
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
                    // The law's weights are 0.25 for pitch and 0.12 for roll,
                    // the sum clamped at 0.30: the sign of the net command.
                    let net = (25. * pitch + 12. * f64::from(side.sign()) * roll) as f32;
                    tail(side, net, &original, &pose, scale, &mut m);
                }
                let fixed = fixed_moved(&original, &pose, &[&TAIL_R, &TAIL_L]);
                m.reviewed_neutral_mismatch |= fixed != 0;
                writeln!(
                    rows,
                    "{pitch},{roll},{},{},{},{fixed},{},{}",
                    m.finite,
                    m.max_reviewed_anchor_gap,
                    m.reviewed_rigid_panel_error,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures
                )?;
                if !passed(&m) {
                    failures.push(format!(
                        "Tu-160 {name} pitch{pitch}/roll{roll}: whole-quad spindle, rigidity or direction witness"
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

    // The sweep against the speed schedule and the flaps: each wing panel
    // (strip included) stays one rigid body, the measured tip turn follows the
    // restated law (20 degrees to 250 kt, 35 from 320 to 520, 65 from 600,
    // smoothstep between, the flaps hold it spread), and the span at the three
    // detents is near the published 55.7, 50.7 and 35.6 m.
    let original = airframe.animation_faces(&at_knots(neutral, 100.));
    let mut rows = String::from(
        "knots,flaps,expected_sweep_deg,measured_right_deg,measured_left_deg,span_m,rigidity_ft,passed\n",
    );
    let mut poses = Vec::new();
    let mut spans = String::from("detent_deg,span_m,real_span_m,difference_percent\n");
    for knots in [
        100., 250., 270., 285., 300., 320., 400., 520., 560., 600., 700., 900.,
    ] {
        for flaps in [0., 0.5, 1.] {
            let mut state = at_knots(neutral, knots);
            state.flaps = flaps;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            // The flap strip droops on its own with the lever, so the panel is
            // checked without its strip; the strip is checked in `flaps`.
            let mut worst = 0f32;
            let mut measured = [0f64; 2];
            let expected = expected_sweep(knots, flaps);
            for (k, side) in SIDES.iter().enumerate() {
                let panel: Vec<usize> = side
                    .wing
                    .iter()
                    .copied()
                    .filter(|a| !side.strip.contains(a))
                    .collect();
                worst = worst.max(rigidity(&original, &pose, &panel, scale));
                // The sweep's turn is measured against the 20 degree pose.
                measured[k] = 20. + tip_turn(&original, &pose, side).unwrap_or(f64::NAN);
            }
            m.reviewed_rigid_panel_error = worst;
            let span = span_metres(&pose);
            let ok = m.finite
                && worst <= EPSILON
                && m.new_planar_crossings.is_empty()
                && measured.iter().all(|t| (t - expected).abs() < 0.01);
            writeln!(
                rows,
                "{knots},{flaps},{expected:.3},{:.3},{:.3},{span:.2},{worst},{ok}",
                measured[0], measured[1]
            )?;
            if !ok {
                failures.push(format!(
                    "Tu-160 sweep {knots} kt/flaps {flaps}: expected {expected:.2} deg, measured {:.2} / {:.2}, rigidity {worst}",
                    measured[0], measured[1]
                ));
            }
            if flaps == 0. && [100., 400., 700.].contains(&knots) {
                let (degrees, real) = if knots == 100. {
                    (20., 55.7)
                } else if knots == 400. {
                    (35., 50.7)
                } else {
                    (65., 35.6)
                };
                let difference = (span - real) / real * 100.;
                writeln!(spans, "{degrees},{span:.2},{real},{difference:.2}")?;
                // The pivot is fitted to the three spans: within 2 percent.
                if difference.abs() > 2. {
                    failures.push(format!(
                        "Tu-160 span at {degrees} degrees is {span:.2} m against {real} m"
                    ));
                }
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("sweep-flap-combinations.csv"), rows)?;
    fs::write(out.join("sweep-span.csv"), spans)?;
    contact_sheet(&out.join("sweep-flap-combinations.ppm"), &original, &poses)?;

    // The root seam, as measured: the panel's ten root vertices (five in plan,
    // two heights) against the fixed airframe, at each sweep angle. Plan
    // distance is how far a root vertex is from the nearest fixed surface seen
    // from above (0 where the fixed skin covers it); vertical is its height
    // above (+) or below (-) that skin where covered. Reported, not a failure:
    // the panel shares no vertex with the glove (the 1 unit hang, 2.7 ft, is in
    // the source).
    let fixed: Vec<Face> = original
        .iter()
        .filter(|f| !WING_R.contains(&f.address) && !WING_L.contains(&f.address))
        .cloned()
        .collect();
    let mut rows = String::from(
        "sweep_deg,turn_deg,side,worst_plan_gap_ft,worst_over_skin_ft,vertices_uncovered\n",
    );
    // The same measure vertex by vertex for the right panel at the three
    // detents, so the front and rear root corners can be told apart.
    let mut vertices = String::from("sweep_deg,root_x,root_y,z,plan_gap_ft,over_skin_ft\n");
    for degrees in [20., 35., 65.] {
        let knots = tore_sim::combat::swing::TU160.speed_for_sweep(degrees) / 1.68781;
        let pose = airframe.animation_faces(&at_knots(neutral, knots));
        let old = group(&original, &WING_R);
        let new = group(&pose, &WING_R);
        for root in ROOT {
            for z in [0., -1.] {
                let at = [root[0], root[1], z];
                if let Some(q) = f4::witness_positions(&old, &new, at).first().copied() {
                    let (gap, vertical) = plan_cover(&fixed, q);
                    writeln!(
                        vertices,
                        "{degrees},{},{},{z},{gap:.2},{vertical:.2}",
                        root[0], root[1]
                    )?;
                }
            }
        }
    }
    fs::write(out.join("sweep-seam-vertices.csv"), vertices)?;
    for degrees in [20., 25., 30., 35., 40., 45., 50., 55., 60., 65.] {
        let knots = tore_sim::combat::swing::TU160.speed_for_sweep(degrees) / 1.68781;
        let pose = airframe.animation_faces(&at_knots(neutral, knots));
        for side in &SIDES {
            let old = group(&original, side.wing);
            let new = group(&pose, side.wing);
            let (mut plan, mut over, mut open) = (0f32, 0f32, 0usize);
            for root in ROOT {
                for z in [0., -1.] {
                    let at = side.fix([root[0], root[1], z]);
                    let Some(q) = f4::witness_positions(&old, &new, at).first().copied() else {
                        continue;
                    };
                    let (gap, vertical) = plan_cover(&fixed, q);
                    plan = plan.max(gap);
                    over = over.max(vertical);
                    open += usize::from(gap > 0.);
                }
            }
            writeln!(
                rows,
                "{degrees},{},{},{plan:.2},{over:.2},{open}",
                degrees - 20.,
                if side.right { "right" } else { "left" }
            )?;
        }
    }
    fs::write(out.join("sweep-seam.csv"), rows)?;

    // The rudder strip at full travel against the pedal, and the flap strip's
    // seam openings (how far a vertex shared with a fixed face moved), reported.
    let mut rows = String::from("surface,command,seam_opening_ft\n");
    for (name, rudder, flaps) in [
        ("rudder-right", 1., 0.),
        ("rudder-left", -1., 0.),
        ("flaps", 0., 1.),
    ] {
        let mut state = at_knots(neutral, 200.);
        state.rudder = rudder;
        state.flaps = flaps;
        let pose = airframe.animation_faces(&state);
        let ids: &[usize] = if flaps > 0. {
            &[0x4b93, 0x4bba, 0x50b4, 0x50db]
        } else {
            &RUDDER
        };
        writeln!(
            rows,
            "{name},full,{}",
            seam_opening(&original, &pose, ids, scale)
        )?;
    }
    fs::write(out.join("seam-opening.csv"), rows)?;

    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    Ok(failures)
}

/// Largest distance (feet) any group vertex that was shared with a face outside
/// the group in the reference moved: the opening of the moving/fixed seam.
fn seam_opening(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
    let before = keyed(reference);
    let after = keyed(pose);
    let key = |p: [f32; 3]| p.map(|v| (v * 1000.).round() as i64);
    let fixed: std::collections::BTreeSet<[i64; 3]> = before
        .iter()
        .filter(|(k, _)| !ids.contains(&k.0))
        .flat_map(|(_, f)| f.positions.iter().map(|p| key(*p)))
        .collect();
    let mut worst = 0f32;
    for (k, face) in before.iter().filter(|(k, _)| ids.contains(&k.0)) {
        let Some(moved) = after.get(k) else {
            continue;
        };
        for (a, b) in face.positions.iter().zip(&moved.positions) {
            if fixed.contains(&key(*a)) {
                worst = worst.max(distance(*a, *b) * scale);
            }
        }
    }
    worst
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every leg
/// rigid, never lower than gear down, grounded at 1, folding the right way. The
/// worst overshoot of the stowed legs outside the neutral skin is written for
/// the report and gated at a small bound.
fn dense_gear(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut endpoint = neutral.clone();
    Control::Gear.apply(&mut endpoint, 1.);
    let reference = airframe.animation_faces(&endpoint);
    // The hull the legs stow into is the shape's own skin: the fixed glove plates
    // (T160-5) are the rig's addition and are not part of it.
    let skin: Vec<Face> = original
        .iter()
        .filter(|f| !GLOVE.contains(&f.address))
        .cloned()
        .collect();
    let mut csv = String::from("value,rigidity_ft,crossings,hull_overshoot_ft,checks_passed\n");
    let mut values: Vec<_> = std::iter::once(1e-6)
        .chain((0..=200).map(|i| f64::from(i) / 200.))
        .collect();
    values.sort_by(f64::total_cmp);
    let mut poses = Vec::new();
    let mut worst_overshoot = 0f32;
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
        let overshoot = if value > 0. && value < 0.2 {
            hull_overshoot(&skin, &pose, &GEAR) * FEET
        } else {
            0.
        };
        if overshoot.is_finite() {
            worst_overshoot = worst_overshoot.max(overshoot);
        }
        let passed = m.finite
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{overshoot},{passed}",
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "Tu-160 dense gear value{value}: rigid, direction or ground gate"
            ));
        }
        if i <= 1 || i % 10 == 1 {
            poses.push(pose);
        }
    }
    fs::write(out.join("dense-gear.csv"), csv)?;
    fs::write(
        out.join("dense-gear-hull.txt"),
        format!("worst overshoot outside the neutral skin below gear 0.2: {worst_overshoot} ft\n"),
    )?;
    contact_sheet(&out.join("dense-gear.ppm"), &reference, &poses)?;
    // Gate: while the legs are stowing (below 0.2 of the travel) no vertex sits
    // more than 1.5 ft outside the skin (the plates are one unit, 2.7 ft, from
    // the belly: they fold into the hull, not through it).
    if worst_overshoot > 1.5 {
        failures.push(format!(
            "Tu-160 stowed gear pokes {worst_overshoot} ft outside the hull"
        ));
    }
    Ok(failures)
}
