//! Independent B-52 whole-panel witnesses: the two stabilizer quads, the two
//! inboard flap wedges, the full-height rudder, the four trucks and two
//! outriggers, and the fitted weapon bay (a door pair cut from the belly, lined
//! recess). Measures the shared Airframe output
//! and never repeats the rig's algorithm. Witness points are named in shape
//! units and scaled with the aircraft's own per-axis scale (the identity: the
//! 2/3 ft a unit is the probe's `scale`).
use super::*;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::B52;
const STAB_R: [usize; 2] = [0x334e, 0x3361];
const STAB_L: [usize; 2] = [0x3167, 0x317a];
const FLAP_R: [usize; 2] = [0x4e54, 0x4e73];
const FLAP_L: [usize; 2] = [0x4f3b, 0x4f5a];
const RUDDER: [usize; 2] = [0x50f0, 0x5117];
const TRUCKS: [[usize; 6]; 4] = [
    [0x49e7, 0x4a0a, 0x4b3d, 0x4b5c, 0x4c35, 0x4c54],
    [0x4a2d, 0x4a50, 0x4aff, 0x4b1e, 0x4bf7, 0x4c16],
    [0x4a73, 0x4ab9, 0x4b7b, 0x4b9a, 0x4c73, 0x4c92],
    [0x4a96, 0x4adc, 0x4bb9, 0x4bd8, 0x4cb1, 0x4cd0],
];
const OUTRIGGERS: [[usize; 4]; 2] = [
    [0x4779, 0x4798, 0x47b7, 0x47d6],
    [0x482c, 0x484b, 0x486a, 0x4889],
];
/// The outriggers' side plates (the x = +/-111 planes), which lie flat when stowed.
const OUTRIGGER_SIDES: [[usize; 2]; 2] = [[0x4779, 0x4798], [0x482c, 0x484b]];
const GEAR: [usize; 32] = [
    0x49e7, 0x4a0a, 0x4b3d, 0x4b5c, 0x4c35, 0x4c54, 0x4a2d, 0x4a50, 0x4aff, 0x4b1e, 0x4bf7, 0x4c16,
    0x4a73, 0x4ab9, 0x4b7b, 0x4b9a, 0x4c73, 0x4c92, 0x4a96, 0x4adc, 0x4bb9, 0x4bd8, 0x4cb1, 0x4cd0,
    0x4779, 0x4798, 0x47b7, 0x47d6, 0x482c, 0x484b, 0x486a, 0x4889,
];
/// The flat belly skin the fitted bay is cut from, and where the lining's
/// addresses start. The lining is a ceiling and four walls (5 faces), a rail
/// to each of the three lanes and a post at each lane's two row boundaries (5
/// faces each).
const BELLY: usize = 0x46ae;
const LINING_BASE: usize = 0x7000;
const LINING: usize = 5 + 3 * 5 + 6 * 5;
/// The fuselage cross-section (x, z) at the gear stations, from the belly (-9)
/// to the roof (11): a stowed truck must lie inside it.
const SECTION: [[f32; 2]; 12] = [
    [-5., -9.],
    [5., -9.],
    [7., -8.],
    [8., -6.],
    [8., 5.],
    [6., 9.],
    [2., 11.],
    [-2., 11.],
    [-6., 9.],
    [-8., 5.],
    [-8., -6.],
    [-7., -8.],
];

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
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

fn rigid(m: &mut Metrics, error: f32) {
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
}

fn pin(m: &mut Metrics, gap: Option<f32>) {
    match gap {
        Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
        None => m.reviewed_anchor_missing = true,
    }
}

/// Distance the midpoint of one pair of vertices moved, in feet.
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

/// How far one vertex moved, in feet.
fn point_gap(
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
    point: [f32; 3],
    scale: f32,
) -> Option<f32> {
    let moved = f4::witness_positions(old, new, point);
    (!moved.is_empty()).then(|| {
        moved
            .iter()
            .map(|q| distance(point, *q) * scale)
            .fold(0f32, f32::max)
    })
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

fn same(old: &BTreeMap<FaceKey, &Face>, new: &BTreeMap<FaceKey, &Face>, m: &mut Metrics) {
    for (key, face) in old {
        match new.get(key) {
            Some(after) if face.positions == after.positions => {}
            _ => m.reviewed_neutral_mismatch = true,
        }
    }
}

/// Signed distance test against the fuselage section, with a margin in units.
fn inside_section(p: [f32; 3], margin: f32) -> bool {
    let (x, z) = (p[0], p[2]);
    let mut inside = false;
    for i in 0..SECTION.len() {
        let (a, b) = (SECTION[i], SECTION[(i + 1) % SECTION.len()]);
        if (a[1] > z) != (b[1] > z) && x < (b[0] - a[0]) * (z - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        let (dx, dz) = (b[0] - a[0], b[1] - a[1]);
        let t = (((x - a[0]) * dx + (z - a[1]) * dz) / (dx * dx + dz * dz)).clamp(0., 1.);
        if ((x - a[0] - t * dx).powi(2) + (z - a[1] - t * dz).powi(2)).sqrt() < margin {
            return false;
        }
    }
    inside
}

/// Stabilizer: one rigid quad per side, the root chord midpoint (the spindle)
/// stays, and the aft tip corner moves up for a pull by the real range (9
/// degrees up, 4 down, 22.5 units from the spindle) and down for a push.
fn stabilizer(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for (ids, side) in [(&STAB_R, 1f32), (&STAB_L, -1f32)] {
        let old = group(reference, ids);
        let new = group(pose, ids);
        rigid(m, rigidity(reference, pose, ids, scale));
        pin(
            m,
            pair_gap(
                &old,
                &new,
                (sp([side * 3., -120., 5.]), sp([side * 6., -81., 5.])),
                scale,
            ),
        );
        if value == 0. {
            same(&old, &new, m);
            continue;
        }
        let tip = sp([side * 42., -123., 5.]);
        let moved = f4::witness_positions(&old, &new, tip);
        m.reviewed_direction_failures += usize::from(moved.is_empty());
        for q in moved {
            let dz = q[2] - tip[2];
            m.reviewed_direction_failures += usize::from(dz * value.signum() as f32 <= EPSILON);
            // Full travel reaches the real limit; part travel is proportional.
            let limit: f32 = if value > 0. { 9. } else { 4. };
            let want = 22.5 * limit.to_radians().sin() * value.abs() as f32;
            m.reviewed_direction_failures += usize::from((dz.abs() - want).abs() > 0.02);
        }
    }
}

/// Flap wedges: rigid, the leading-edge pair midpoints (the hinge line) stay,
/// the aft corners drop for a positive flap. The shared trailing and leading
/// corners are the reviewed seam boundaries.
fn flaps(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for (ids, side) in [(&FLAP_R, 1f32), (&FLAP_L, -1f32)] {
        let old = group(reference, ids);
        let new = group(pose, ids);
        rigid(m, rigidity(reference, pose, ids, scale));
        for pair in [
            (sp([side * 15., 7., 6.]), sp([side * 15., 7., 10.])),
            (sp([side * 53., -14., 5.]), sp([side * 53., -14., 8.])),
        ] {
            pin(m, pair_gap(&old, &new, pair, scale));
        }
        if value == 0. {
            same(&old, &new, m);
            continue;
        }
        let corner = sp([if side > 0. { 47. } else { -48. }, -20., 7.]);
        let moved = f4::witness_positions(&old, &new, corner);
        m.reviewed_direction_failures += usize::from(moved.is_empty());
        for q in moved {
            m.reviewed_direction_failures += usize::from(q[2] >= corner[2] - EPSILON);
        }
    }
    for seam in &mut m.seams {
        seam.reviewed_boundary = [
            [9., 1., 9.],
            [47., -20., 7.],
            [-9., 1., 9.],
            [-48., -20., 7.],
            [15., 7., 6.],
            [15., 7., 10.],
            [-15., 7., 6.],
            [-15., 7., 10.],
            [53., -14., 5.],
            [53., -14., 8.],
            [-53., -14., 5.],
            [-53., -14., 8.],
        ]
        .contains(&seam.point);
    }
}

/// Rudder: both windings rigid, the forward edge (the hinge) stays, the top and
/// bottom rear vertices move to +X for a positive command and -X for a
/// negative one, to the retail word's travel (0.54 rad).
fn rudder(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, &RUDDER);
    let new = group(pose, &RUDDER);
    rigid(m, rigidity(reference, pose, &RUDDER, scale));
    for hinge in [[0., -114., 42.], [0., -101., 11.]] {
        pin(m, point_gap(&old, &new, sp(hinge), scale));
    }
    if value == 0. {
        same(&old, &new, m);
        return;
    }
    // Each rear vertex sits at a known distance from the hinge line (7.378 and
    // 14.755 units) and moves out of the fin plane by that distance times the
    // sine of the angle (0.54 rad at full command).
    for (rear, radius) in [([0., -122., 42.], 7.378f32), ([0., -117., 11.], 14.755)] {
        let rear = sp(rear);
        let moved = f4::witness_positions(&old, &new, rear);
        m.reviewed_direction_failures += usize::from(moved.is_empty());
        for q in moved {
            let dx = q[0] - rear[0];
            let want = radius * (0.54 * value.abs() as f32).sin();
            m.reviewed_direction_failures += usize::from(dx * value.signum() as f32 <= EPSILON);
            m.reviewed_direction_failures += usize::from((dx.abs() - want).abs() > 0.02);
        }
    }
}

/// The six assemblies stay whole rigid pieces, never hang below the gear-down
/// pose, move up and inboard-or-up as they close, and near stow every truck lies
/// inside the fuselage section and each outrigger's side plate lies flat
/// between the wing skins (z 3.7 to 4.5). Gear down is the ground contact.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= present(pose, &GEAR) != 0;
        return;
    }
    for ids in TRUCKS
        .iter()
        .map(|t| &t[..])
        .chain(OUTRIGGERS.iter().map(|o| &o[..]))
    {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
    }
    let (reference_low, pose_low) = (lowest(reference, &GEAR), lowest(pose, &GEAR));
    // The outrigger's cross plate dips 0.11 unit (0.07 ft) below the contact line
    // near the start of its turn (it is 2 units wider than its hinge): 0.15 allowed.
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - 0.15;
    if value == 1. {
        // The sim stands the aircraft on the contact line, 14 units low.
        m.reviewed_wheel_failed |= (pose_low - sp([0., 0., -14.])[2]).abs() > 1e-3;
        return;
    }
    // Direction: a truck's outer foot rises as it closes; an outrigger's foot
    // moves inboard and up.
    for (ids, foot) in [
        (&TRUCKS[0][..], sp([13., 39., -14.])),
        (&TRUCKS[1][..], sp([-14., 39., -14.])),
        (&TRUCKS[2][..], sp([13., -32., -14.])),
        (&TRUCKS[3][..], sp([-14., -32., -14.])),
    ] {
        let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[2] <= foot[2]);
        }
    }
    for (ids, foot, inward) in [
        (&OUTRIGGERS[0][..], sp([111., -32., -14.]), -1f32),
        (&OUTRIGGERS[1][..], sp([-111., -32., -14.]), 1.),
    ] {
        let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures +=
                usize::from((q[0] - foot[0]) * inward <= 0. || q[2] <= foot[2]);
        }
    }
    if value < 1e-5 {
        for truck in &TRUCKS {
            m.reviewed_wheel_failed |= pose
                .iter()
                .filter(|f| truck.contains(&f.address))
                .flat_map(|f| &f.positions)
                .any(|p| !inside_section(*p, 0.2));
        }
        for sides in &OUTRIGGER_SIDES {
            m.reviewed_wheel_failed |= pose
                .iter()
                .filter(|f| sides.contains(&f.address))
                .flat_map(|f| &f.positions)
                .any(|p| p[2] < sp([0., 0., 3.7])[2] || p[2] > sp([0., 0., 4.5])[2]);
        }
    }
}

/// The fitted bay: shut (0) the belly skin is whole, nothing hangs below the
/// belly plane and no lining draws; open (above 0) the lining draws and the two
/// doors hang on their hinge lines (the outer edges, x +/-3 ft, at the belly
/// plane) with their free edges `3 ft * sin(90 degrees * value)` below it.
fn bay(value: f64, _reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let o = tore_sim::combat::bays::centre_bay(ID).expect("B-52 bay");
    let belly = sp([0., 0., -9.])[2];
    let lining = pose.iter().filter(|f| f.address >= LINING_BASE).count();
    let skins: Vec<&Face> = pose.iter().filter(|f| f.address == BELLY).collect();
    let lowest = skins
        .iter()
        .flat_map(|f| f.positions.iter())
        .map(|p| p[2])
        .fold(f32::MAX, f32::min);
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(lining != 0 || skins.len() != 1 || lowest < belly - 1e-4);
        return;
    }
    m.reviewed_direction_failures += usize::from(lining != LINING);
    // A vertex stays on each hinge line at the belly plane.
    for feet in [o.half, -o.half] {
        let hinge = feet as f32 / scale;
        let held = skins
            .iter()
            .flat_map(|f| f.positions.iter())
            .any(|p| (p[0] - hinge).abs() < 1e-3 && (p[2] - belly).abs() < 1e-3);
        m.reviewed_anchor_missing |= !held;
    }
    let leaf = o.half as f32 / scale;
    let hang = leaf * (value as f32 * std::f32::consts::FRAC_PI_2).sin();
    m.reviewed_direction_failures += usize::from((belly - lowest - hang).abs() > 2e-3);
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
        Control::Elevator => stabilizer(value, reference, pose, scale, m),
        Control::Rudder => rudder(value, reference, pose, scale, m),
        Control::Flaps => flaps(value, reference, pose, scale, m),
        Control::Gear => gear(value, reference, pose, scale, m),
        Control::Bay => bay(value, reference, pose, scale, m),
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
    // Pitch against yaw: the stabilizer and the rudder are independent panels, so
    // every pose is rigid, pinned and in the right sense whatever the other does.
    let mut rows =
        String::from("pitch,yaw,finite,anchor_gap_ft,rigidity_ft,crossings,direction_failures\n");
    let mut poses = Vec::new();
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for yaw in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.rudder = yaw;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            stabilizer(pitch, &original, &pose, scale, &mut m);
            rudder(yaw, &original, &pose, scale, &mut m);
            let passed = m.finite
                && !m.reviewed_anchor_missing
                && m.max_reviewed_anchor_gap <= EPSILON
                && m.reviewed_rigid_panel_error <= EPSILON
                && m.reviewed_direction_failures == 0
                && !m.reviewed_neutral_mismatch
                && m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{pitch},{yaw},{},{},{},{},{}",
                m.finite,
                m.max_reviewed_anchor_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "B-52 pitch{pitch}/yaw{yaw}: whole-panel hinge, rigidity or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("pitch-yaw-combinations.csv"), rows)?;
    contact_sheet(&out.join("pitch-yaw-combinations.ppm"), &original, &poses)?;
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
                "B-52 dense gear value{value}: rigid, direction, stow or ground gate"
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
