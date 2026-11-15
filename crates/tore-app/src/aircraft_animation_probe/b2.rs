//! Independent B-2 whole-panel witnesses: the two elevon wedges (one angle per
//! side, pitch plus differential roll), the four drag-rudder plates (yaw side
//! and speed brake), the seven rigid gear plates and the interim bay plates.
//! Measures the shared Airframe output and never repeats the rig's algorithm.
//! Witness points are named in shape units and scaled with the aircraft's own
//! per-axis scale (the probe geometry is the scaled shape).
use super::*;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::B2;
const ELEVON_R: [usize; 4] = [0x4037, 0x404e, 0x4065, 0x407a];
const ELEVON_L: [usize; 4] = [0x40ce, 0x40e5, 0x40fc, 0x4111];
/// Drag-rudder closed quads: (address, free-edge vertex indices, upper plate).
const PLATES_R: [(usize, [usize; 2], bool); 2] = [(0x465f, [0, 1], true), (0x4679, [0, 1], false)];
const PLATES_L: [(usize, [usize; 2], bool); 2] = [(0x4600, [0, 1], true), (0x461a, [2, 3], false)];
/// The back windings of those plates and the front plate and inboard cap.
const BACK_R: [usize; 2] = [0x47f5, 0x47c1];
const BACK_L: [usize; 2] = [0x46ea, 0x471e];
const WALLS_R: [usize; 2] = [0x480f, 0x4829];
const WALLS_L: [usize; 2] = [0x4738, 0x4752];
const NOSE: [&[usize]; 3] = [&[0x4153, 0x416a], &[0x41ae, 0x41c5], &[0x4597, 0x45b6]];
const MAINS: [&[usize]; 4] = [
    &[0x438d, 0x43a4],
    &[0x449e, 0x44b5],
    &[0x441b, 0x443a],
    &[0x452c, 0x454b],
];
const GEAR: [usize; 14] = [
    0x4153, 0x416a, 0x41ae, 0x41c5, 0x4597, 0x45b6, 0x438d, 0x43a4, 0x449e, 0x44b5, 0x441b, 0x443a,
    0x452c, 0x454b,
];
const BAY: [usize; 8] = [
    0x4209, 0x4220, 0x4264, 0x427b, 0x431a, 0x4331, 0x42bf, 0x42d6,
];

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

struct Side {
    ids: &'static [usize],
    right: bool,
}
const SIDES: [Side; 2] = [
    Side {
        ids: &ELEVON_R,
        right: true,
    },
    Side {
        ids: &ELEVON_L,
        right: false,
    },
];
impl Side {
    /// Leading-edge top and bottom vertex pairs at each end; the midpoint of a
    /// pair lies on the hinge line.
    fn pairs(&self) -> [([f32; 3], [f32; 3]); 2] {
        if self.right {
            [
                (sp([63., -70., 0.]), sp([63., -70., -6.])),
                (sp([120., -22., 0.]), sp([120., -22., -6.])),
            ]
        } else {
            [
                (sp([-64., -70., 0.]), sp([-64., -70., -6.])),
                (sp([-121., -22., 0.]), sp([-121., -22., -6.])),
            ]
        }
    }
    /// The two knife-edge trailing vertices.
    fn trailing(&self) -> [[f32; 3]; 2] {
        if self.right {
            [sp([69., -80., -3.]), sp([130., -29., -3.])]
        } else {
            [sp([-70., -80., -3.]), sp([-131., -29., -3.])]
        }
    }
    /// Direction of a positive command on the trailing edge's Z: a pull raises
    /// both, a positive aileron raises the right and lowers the left.
    fn sign(&self, c: Control) -> f32 {
        match c {
            Control::Elevator => 1.,
            Control::Aileron if self.right => 1.,
            Control::Aileron => -1.,
            _ => 0.,
        }
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

fn pin(m: &mut Metrics, gap: f32) {
    m.reviewed_anchor_missing |= !gap.is_finite();
    if gap.is_finite() {
        m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
    }
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

/// Distance the midpoint of one pair of vertices moved.
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

/// Whole-wedge checks: rigid, and the hinge line (the leading-edge pair
/// midpoints) stays.
fn wedge(side: &Side, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.ids);
    let new = group(pose, side.ids);
    let error = rigidity(reference, pose, side.ids, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    for pair in side.pairs() {
        match pair_gap(&old, &new, pair, scale) {
            Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
            None => m.reviewed_anchor_missing = true,
        }
    }
}

/// The z of a closed quad's first free vertex in `pose`.
fn free_z(faces: &[Face], address: usize, index: usize) -> Option<f32> {
    faces
        .iter()
        .find(|f| f.address == address)
        .map(|f| f.positions[index][2])
}

/// How open a side should be for a yaw and a brake command: the restated
/// mapping the probe checks the drawing against.
fn expected_open(right: bool, yaw: f64, brake: f64) -> f64 {
    let side = if right { yaw.max(0.) } else { (-yaw).max(0.) };
    (brake + side).min(1.)
}

/// Drag-rudder checks against the closed pose `neutral`: each plate is rigid,
/// its hinge vertices stay, its free edge rises (upper) or falls (lower) by an
/// amount that grows with the open fraction, and the walls and back windings
/// are drawn exactly when the side is open.
fn drag(open: [f64; 2], neutral: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for (right, fraction) in [(true, open[0]), (false, open[1])] {
        let (plates, back, walls) = if right {
            (&PLATES_R, &BACK_R, &WALLS_R)
        } else {
            (&PLATES_L, &BACK_L, &WALLS_L)
        };
        for &(address, free, upper) in plates {
            let error = rigidity(neutral, pose, &[address], scale);
            m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
            let hinge: Vec<usize> = (0..4).filter(|i| !free.contains(i)).collect();
            let neutral_face = neutral.iter().find(|f| f.address == address);
            let pose_face = pose.iter().find(|f| f.address == address);
            let (Some(n), Some(p)) = (neutral_face, pose_face) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            for i in hinge {
                pin(m, distance(n.positions[i], p.positions[i]) * scale);
            }
            let rise = (p.positions[free[0]][2] - n.positions[free[0]][2]) * scale;
            let want = if upper { 1. } else { -1. };
            if fraction <= 0. {
                m.reviewed_direction_failures += usize::from(rise.abs() > EPSILON);
            } else {
                m.reviewed_direction_failures += usize::from(rise * want <= EPSILON);
            }
        }
        let drawn = present(pose, back) + present(pose, walls);
        m.reviewed_direction_failures += usize::from((drawn == 4) != (fraction > 0.));
        m.reviewed_direction_failures += usize::from(drawn != 0 && drawn != 4);
    }
}

/// Free-edge height of the upper plate for a fraction, from the same measured
/// open pose: the upper plate rises monotonically with the opening.
fn monotone(rows: &[(f64, f32)]) -> bool {
    rows.windows(2)
        .all(|w| w[1].0 <= w[0].0 || w[1].1 >= w[0].1 - 1e-4)
}

/// The seven gear plates stay whole rigid pieces, stow inboard (mains) and aft
/// (nose), never hang below the gear-down pose, and at stow lie at or above
/// their own hinge height. Gear down is the ground contact: the lowest point
/// is the scaled low point.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= present(pose, &GEAR) != 0;
        return;
    }
    for ids in NOSE.iter().chain(MAINS.iter()) {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
    }
    let (reference_low, pose_low) = (lowest(reference, &GEAR), lowest(pose, &GEAR));
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // The sim stands the aircraft on its inner main plates, 33 units low.
        let want = sp([0., 0., -33.])[2];
        m.reviewed_wheel_failed |= (pose_low - want).abs() > 1e-3;
        return;
    }
    // Direction: a main's lowest corner moves inboard, the nose strut's foot
    // moves aft.
    for (ids, witness, inward) in [
        (MAINS[1], sp([69., -7., -27.]), -1f32),
        (MAINS[0], sp([-71., -7., -27.]), 1.),
        (MAINS[3], sp([62., -2., -33.]), -1.),
        (MAINS[2], sp([-65., -2., -33.]), 1.),
    ] {
        let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from((q[0] - witness[0]) * inward <= 0.);
        }
    }
    let foot = sp([0., 88., -30.]);
    let qs = f4::witness_positions(&group(reference, NOSE[2]), &keyed(pose), foot);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from(q[1] >= foot[1]);
    }
    if value < 1e-5 {
        // Near stow every piece lies at or above its own hinge height, inside
        // the closed well (z -12) or the nose belly.
        for (ids, hinge_z) in [
            (NOSE[0], -9.),
            (NOSE[1], -5.),
            (NOSE[2], -5.),
            (MAINS[0], -10.),
            (MAINS[1], -10.),
            (MAINS[2], -9.),
            (MAINS[3], -9.),
        ] {
            m.reviewed_wheel_failed |= lowest(pose, ids) < sp([0., 0., hinge_z])[2] - 1e-3;
        }
        let belly = sp([0., 0., -12.])[2];
        m.reviewed_wheel_failed |= MAINS.iter().any(|ids| lowest(pose, ids) < belly + EPSILON);
    }
}

/// Interim bay plates: drawn only while the bay is open, their top edge stays
/// on the belly (z -12), and the lowest point is the scaled -12 - 13 * value.
fn bay(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures += usize::from(present(pose, &BAY) != 0);
        return;
    }
    m.reviewed_direction_failures += usize::from(present(pose, &BAY) != 8);
    for point in [
        sp([-27., -10., -12.]),
        sp([-3., 48., -12.]),
        sp([1., -10., -12.]),
        sp([26., 48., -12.]),
    ] {
        pin(m, pinned(reference, pose, &BAY, point, scale));
    }
    let want = sp([0., 0., -12. - 13. * value as f32])[2];
    m.reviewed_direction_failures += usize::from((lowest(pose, &BAY) - want).abs() > 1e-3);
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
        Control::Gear => return gear(value, reference, pose, scale, m),
        Control::Bay => return bay(value, reference, pose, scale, m),
        _ => {}
    }
    match c {
        Control::Elevator | Control::Aileron => {
            for side in &SIDES {
                wedge(side, reference, pose, scale, m);
                let old = group(reference, side.ids);
                let new = group(pose, side.ids);
                if value == 0. {
                    for (key, face) in &old {
                        match new.get(key) {
                            Some(after) if face.positions == after.positions => {}
                            _ => m.reviewed_neutral_mismatch = true,
                        }
                    }
                    continue;
                }
                for witness in side.trailing() {
                    let qs = f4::witness_positions(&old, &new, witness);
                    m.reviewed_direction_failures += usize::from(qs.is_empty());
                    for q in qs {
                        let delta = (q[2] - witness[2]) * scale;
                        m.reviewed_direction_failures +=
                            usize::from(delta * side.sign(c) * value.signum() as f32 <= EPSILON);
                    }
                }
            }
        }
        Control::Rudder => {
            // Reference is the neutral pose: yaw right opens the right side.
            drag(
                [
                    expected_open(true, value, 0.),
                    expected_open(false, value, 0.),
                ],
                reference,
                pose,
                scale,
                m,
            );
        }
        Control::Brake => {
            // Reference is brake 1 (both open). Compare each pose against the
            // closed pose rebuilt from the same plates at value 0.
            if value == 0. {
                m.reviewed_direction_failures += usize::from(
                    present(pose, &BACK_R)
                        + present(pose, &BACK_L)
                        + present(pose, &WALLS_R)
                        + present(pose, &WALLS_L)
                        != 0,
                );
            } else {
                for ids in [&PLATES_R, &PLATES_L] {
                    for &(address, _, _) in ids {
                        let error = rigidity(reference, pose, &[address], scale);
                        m.reviewed_rigid_panel_error =
                            m.reviewed_rigid_panel_error.max(error.min(1e9));
                    }
                }
                // Both sides open together: an upper plate's free edge is as
                // high on the left as on the right, to the retail mirror error.
                let (r, l) = (
                    free_z(pose, 0x465f, 0).unwrap_or(f32::NAN),
                    free_z(pose, 0x4600, 0).unwrap_or(f32::NAN),
                );
                let gap = (r - l).abs() * scale;
                m.reviewed_direction_failures += usize::from(gap.is_nan() || gap >= 0.1);
            }
        }
        _ => {}
    }
}

pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut rows =
        String::from("pitch,roll,finite,anchor_gap_ft,rigidity_ft,crossings,direction_failures\n");
    let mut poses = Vec::new();
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.aileron = roll;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            for side in &SIDES {
                wedge(side, &original, &pose, scale, &mut m);
                // The net command on a trailing edge has the sign of 3 pitch
                // (a pull raises both) plus 2 roll (up on the right, down on
                // the left); equal and opposite terms cancel.
                let net = 3. * pitch + if side.right { 2. } else { -2. } * roll;
                if net != 0. {
                    let old = group(&original, side.ids);
                    let new = group(&pose, side.ids);
                    for witness in side.trailing() {
                        let qs = f4::witness_positions(&old, &new, witness);
                        m.reviewed_direction_failures += usize::from(qs.is_empty());
                        for q in qs {
                            let delta = (q[2] - witness[2]) * scale;
                            m.reviewed_direction_failures +=
                                usize::from(delta * net.signum() as f32 <= EPSILON);
                        }
                    }
                }
            }
            let passed = m.finite
                && !m.reviewed_anchor_missing
                && m.max_reviewed_anchor_gap <= EPSILON
                && m.reviewed_rigid_panel_error <= EPSILON
                && m.reviewed_direction_failures == 0
                && m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{pitch},{roll},{},{},{},{},{}",
                m.finite,
                m.max_reviewed_anchor_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "B-2 pitch{pitch}/roll{roll}: whole-wedge hinge, rigidity or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("pitch-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("pitch-roll-combinations.ppm"), &original, &poses)?;

    // Yaw (drag rudder side) against the speed brake: every pose rigid, the
    // hinge edges stay, each side open exactly as far as the restated mapping
    // says, and the open fraction never shrinks as yaw or brake grows.
    let mut rows = String::from(
        "yaw,brake,open_right,open_left,finite,anchor_gap_ft,rigidity_ft,crossings,direction_failures,monotone\n",
    );
    let mut poses = Vec::new();
    let mut by_yaw: BTreeMap<i64, Vec<(f64, f32)>> = BTreeMap::new();
    for yaw in [-1., -0.5, 0., 0.5, 1.] {
        for brake in [0., 0.25, 0.5, 0.75, 1.] {
            let mut state = neutral.clone();
            state.rudder = yaw;
            state.brake = brake;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            let open = [
                expected_open(true, yaw, brake),
                expected_open(false, yaw, brake),
            ];
            drag(open, &original, &pose, scale, &mut m);
            let rise = free_z(&pose, 0x465f, 0).unwrap_or(f32::NAN)
                - free_z(&original, 0x465f, 0).unwrap_or(f32::NAN);
            by_yaw
                .entry((yaw * 100.) as i64)
                .or_default()
                .push((brake, rise));
            let passed = m.finite
                && !m.reviewed_anchor_missing
                && m.max_reviewed_anchor_gap <= EPSILON
                && m.reviewed_rigid_panel_error <= EPSILON
                && m.reviewed_direction_failures == 0
                && m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{yaw},{brake},{},{},{},{},{},{},{},true",
                open[0],
                open[1],
                m.finite,
                m.max_reviewed_anchor_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "B-2 yaw{yaw}/brake{brake}: drag-rudder hinge, rigidity or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    for (yaw, series) in &by_yaw {
        if !monotone(series) {
            failures.push(format!(
                "B-2 yaw {yaw}: the right drag rudder closes as the brake grows"
            ));
        }
    }
    fs::write(out.join("yaw-brake-combinations.csv"), rows)?;
    contact_sheet(&out.join("yaw-brake-combinations.ppm"), &original, &poses)?;
    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    Ok(failures)
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, never lower than gear down, grounded at 1 and tucked near 0.
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
    let mut csv = String::from("value,anchor_gap_ft,rigidity_ft,crossings,checks_passed\n");
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
        let passed = m.finite
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{},{passed}",
            m.max_reviewed_anchor_gap,
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "B-2 dense gear value{value}: rigid, direction, stow or ground gate"
            ));
        }
        write_obj(&out.join(format!("dense-gear-{i}.obj")), &pose)?;
        if i <= 1 || i % 10 == 1 {
            poses.push(pose);
        }
    }
    fs::write(out.join("dense-gear.csv"), csv)?;
    contact_sheet(&out.join("dense-gear.ppm"), &reference, &poses)?;
    Ok(failures)
}
