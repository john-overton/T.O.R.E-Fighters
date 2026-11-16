//! Independent Tu-95 whole-panel witnesses: the tailplane (one right panel and
//! three left ones turning as one piece), the rudder triangle, the six flat wheel
//! plates and the four propeller discs (ten kept faces). Measures the shared
//! Airframe output and never repeats the rig's algorithm. Witness points are
//! named in shape units and scaled with the aircraft's own per-axis scale (the
//! identity: the 2/3 ft a unit is the probe's `scale`). The aileron, flaps, brake
//! and exhaust are not in the shape, so the probe requires that they move
//! nothing. The fitted bay (T95-5): the four belly triangles cut into door
//! leaves and a lined recess.
use super::*;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::Tu95;
const TAIL: [usize; 6] = [0x20f1, 0x2104, 0x2196, 0x21a9, 0x23fb, 0x240f];
const RUDDER: [usize; 2] = [0x2117, 0x2135];
/// Each plate as one rigid pair of windings: left main, right main, nose.
const PLATES: [[usize; 2]; 3] = [[0x29f9, 0x2a2f], [0x2a14, 0x2a4a], [0x2a65, 0x2a88]];
const GEAR: [usize; 6] = [0x29f9, 0x2a2f, 0x2a14, 0x2a4a, 0x2a65, 0x2a88];
/// The belly triangles the fitted bay is cut from, and the first address of the
/// lining (a ceiling, four walls and five faces a lane rail, three lanes).
const BELLY: [usize; 4] = [0x1a38, 0x1a50, 0x22a7, 0x22c1];
const LINING_BASE: usize = 0x7000;
const LINING: usize = 5 + 3 * 5;
/// The kept faces of the four discs (right inboard, right outboard, left
/// inboard, left outboard in two halves) and their hubs.
const DISCS: [(&[usize], [f32; 3]); 4] = [
    (&[0x1aa2, 0x1b27], [35.25, 47.5, -1.5]),
    (&[0x1d20, 0x1da5], [65.5, 30., -2.5]),
    (&[0x243f, 0x24c4], [-34., 47.5, -1.5]),
    (&[0x2610, 0x2694, 0x2850, 0x28d5], [-64., 30., -2.5]),
];
/// Every face of the four discs in the source (the other three phases of each
/// winding are never drawn).
const DISC_SOURCE: [usize; 40] = [
    0x1aa2, 0x1ac1, 0x1ae0, 0x1aff, 0x1b27, 0x1b46, 0x1b65, 0x1b84, 0x1d20, 0x1d3f, 0x1d5e, 0x1d7d,
    0x1da5, 0x1dc4, 0x1de3, 0x1e02, 0x243f, 0x245e, 0x247d, 0x249c, 0x24c4, 0x24e3, 0x2502, 0x2521,
    0x2610, 0x262f, 0x264e, 0x266d, 0x2694, 0x26b3, 0x26d2, 0x26f1, 0x2850, 0x286f, 0x288e, 0x28ad,
    0x28d5, 0x28f4, 0x2913, 0x2932,
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

/// Largest change of any vertex-to-vertex distance inside one group, feet.
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

/// Tailplane: the six panels stay one rigid piece, the root chord midpoint (the
/// spindle) stays, and each trailing tip corner moves up for a pull by the exact
/// turn of 10 degrees times the command about the spindle (the tip corner is 25
/// units aft of it and 1 unit below), while the leading root corner moves down.
fn tailplane(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, &TAIL);
    let new = group(pose, &TAIL);
    rigid(m, rigidity(reference, pose, &TAIL, scale));
    pin(
        m,
        pair_gap(&old, &new, (sp([0., -116., 9.]), sp([0., -94., 9.])), scale),
    );
    if value == 0. {
        same(&old, &new, m);
        return;
    }
    let turn = -10f64.to_radians() * value;
    // Rise of a point `back` units aft of and `below` units under the spindle.
    let rise = |back: f64, below: f64| (-back * turn.sin() - below * (turn.cos() - 1.)) as f32;
    for tip in [sp([33., -130., 8.]), sp([-33., -130., 8.])] {
        let moved = f4::witness_positions(&old, &new, tip);
        m.reviewed_direction_failures += usize::from(moved.is_empty());
        for q in moved {
            let dz = q[2] - tip[2];
            m.reviewed_direction_failures += usize::from(dz * value.signum() as f32 <= EPSILON);
            m.reviewed_direction_failures += usize::from((dz - rise(25., 1.)).abs() > 0.02);
        }
    }
    let lead = sp([0., -94., 9.]);
    for q in f4::witness_positions(&old, &new, lead) {
        m.reviewed_direction_failures +=
            usize::from((q[2] - lead[2]) * value.signum() as f32 >= 0.);
    }
}

/// Rudder: both windings rigid, the forward edge (the hinge) stays, and the free
/// vertex moves to +X for a positive command and -X for a negative one by its
/// distance from the hinge line (11.69 units) times the sine of 25 degrees times
/// the command.
fn rudder(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, &RUDDER);
    let new = group(pose, &RUDDER);
    rigid(m, rigidity(reference, pose, &RUDDER, scale));
    for hinge in [[0., -112., 42.], [0., -121., 3.]] {
        pin(m, point_gap(&old, &new, sp(hinge), scale));
    }
    if value == 0. {
        same(&old, &new, m);
        return;
    }
    // Distance of (0,-124,42) from the line (0,-121,3) to (0,-112,42), in the
    // fin plane: |v|^2 - (v.d)^2 / |d|^2 with v = (0,-3,39), d = (0,9,39).
    let radius = (1530f64 - 1494f64 * 1494f64 / 1602.).sqrt() as f32;
    let free = sp([0., -124., 42.]);
    let moved = f4::witness_positions(&old, &new, free);
    m.reviewed_direction_failures += usize::from(moved.is_empty());
    for q in moved {
        let dx = q[0] - free[0];
        let want = radius * (25f32.to_radians() * value.abs() as f32).sin();
        m.reviewed_direction_failures += usize::from(dx * value.signum() as f32 <= EPSILON);
        m.reviewed_direction_failures += usize::from((dx.abs() - want).abs() > 0.02);
    }
}

/// The nacelle and fuselage the plates stow into, in shape units: a main plate
/// ends between y 0.99 and 9, z -7 to 0.3 (the nacelle underside to its ridge),
/// the nose plate between y 69 and 76, z -7 to -1 (the keel to well inside).
fn stowed_inside(plate: usize, p: [f32; 3]) -> bool {
    let eps = 0.01;
    if plate == 2 {
        (68.99..=76. + eps).contains(&p[1]) && (-7. - eps..=-0.99).contains(&p[2])
    } else {
        (0.99..=9. + eps).contains(&p[1]) && (-7. - eps..=0.3).contains(&p[2])
    }
}

/// Where a plate's rear bottom corner (relative to its forward top pivot: 6
/// units aft and 8 down for a main, 6 aft and 7 down for the nose) lies after a
/// rearward in-plane turn of `phi` radians.
fn corner(phi: f64, aft: f64, down: f64) -> (f64, f64) {
    (
        -aft * phi.cos() - down * phi.sin(),
        aft * phi.sin() - down * phi.cos(),
    )
}

/// The three plates stay whole rigid pieces, never hang below the gear-down pose,
/// keep their forward top corner fixed, fold rearward and up on the fitted
/// timing (mains over 80 percent of the retraction, the nose plate over the last
/// 80 percent) and near stow lie inside the nacelle or the fuselage. Gear down is
/// the ground contact.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= present(pose, &GEAR) != 0;
        return;
    }
    m.reviewed_wheel_failed |= present(pose, &GEAR) != 6;
    for plate in &PLATES {
        let error = rigidity(reference, pose, plate, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
    }
    let (reference_low, pose_low) = (lowest(reference, &GEAR), lowest(pose, &GEAR));
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // The sim stands the aircraft on the contact line, 14 units low.
        m.reviewed_wheel_failed |= (pose_low - sp([0., 0., -14.])[2]).abs() > 1e-3;
        return;
    }
    let progress = 1. - value;
    for (i, (plate, pivot, aft, down, nose)) in [
        (&PLATES[0], [-33., 9., -6.], 6., 8., false),
        (&PLATES[1], [34., 9., -6.], 6., 8., false),
        (&PLATES[2], [0., 76., -7.], 6., 7., true),
    ]
    .into_iter()
    .enumerate()
    {
        let old = group(reference, plate);
        let new = keyed(pose);
        // The forward top corner is the pivot: it never moves.
        pin(m, point_gap(&old, &new, sp(pivot), scale));
        // Direction and timing: the rear bottom corner is where the fitted turn
        // puts it (rearward and up).
        let fraction = if nose {
            ((progress - 0.2) / 0.8).clamp(0., 1.)
        } else {
            (progress / 0.8).clamp(0., 1.)
        };
        let (dy, dz) = corner(fraction * std::f64::consts::FRAC_PI_2, aft, down);
        let start = sp([pivot[0], pivot[1] - aft as f32, pivot[2] - down as f32]);
        let moved = f4::witness_positions(&old, &new, start);
        m.reviewed_direction_failures += usize::from(moved.is_empty());
        for q in moved {
            let want = sp([pivot[0], pivot[1] + dy as f32, pivot[2] + dz as f32]);
            m.reviewed_direction_failures += usize::from(distance(q, want) > 0.02);
            m.reviewed_direction_failures += usize::from(q[2] < start[2] - EPSILON);
        }
        if value < 1e-5 {
            m.reviewed_wheel_failed |= pose
                .iter()
                .filter(|f| plate.contains(&f.address))
                .flat_map(|f| &f.positions)
                .any(|p| !stowed_inside(i, *p));
        }
    }
}

/// The four discs: ten kept faces, none of the other source phases, every disc
/// rigid, every hub and every vertex's height along the shaft unchanged, and
/// every vertex of every disc turned about its hub by the same angle, the fitted
/// 15 revolutions a second at the sample's tick (the tick is `round(value * 8)`).
fn rotor(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let kept: usize = DISCS.iter().map(|(faces, _)| faces.len()).sum();
    m.reviewed_direction_failures += usize::from(present(pose, &DISC_SOURCE) != kept);
    let want = (value * 8.).round() * crate::flight::DT * std::f64::consts::TAU * 15.;
    for (faces, hub) in DISCS {
        let hub = sp(hub);
        rigid(m, rigidity(reference, pose, faces, scale));
        let (old, new) = (group(reference, faces), keyed(pose));
        for (key, face) in &old {
            let Some(after) = new.get(key) else {
                m.reviewed_anchor_missing = true;
                continue;
            };
            for (p, q) in face.positions.iter().zip(&after.positions) {
                // The shaft is the Y axis: depth and radius stay.
                m.reviewed_rigid_panel_error = m
                    .reviewed_rigid_panel_error
                    .max(((p[1] - q[1]).abs() * scale).min(1e9));
                let radius =
                    |v: &[f32; 3]| ((v[0] - hub[0]).powi(2) + (v[2] - hub[2]).powi(2)).sqrt();
                m.reviewed_rigid_panel_error = m
                    .reviewed_rigid_panel_error
                    .max(((radius(p) - radius(q)).abs() * scale).min(1e9));
                let turned = f64::from((q[2] - hub[2]).atan2(q[0] - hub[0]))
                    - f64::from((p[2] - hub[2]).atan2(p[0] - hub[0]));
                // Positive turns about +Y carry +X toward -Z.
                let off = (turned + want).rem_euclid(std::f64::consts::TAU);
                m.reviewed_direction_failures +=
                    usize::from(off.min(std::f64::consts::TAU - off) > 0.01);
            }
        }
    }
}

/// The fitted bay: shut (0) the belly triangles are whole (their lowest point is
/// the keel's, z -7 units) and no lining draws; open (above 0) the lining draws
/// and the leaves hang from the hinge lines (x +/-2.75 ft at the mean belly
/// height, within the V skin's 0.2 unit). A leaf's free edge is a point of the
/// keel's V, `dz` below the hinge height (1.1 to 1.45 units: the keel is z -7
/// units, the skin on the cut line -6.67 to -6.85), so a quarter-turn sweep puts
/// it `dz * cos - leaf * sin` below the hinge height (`leaf` the leaf's width).
fn bay(value: f64, _reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let o = tore_sim::combat::bays::opening(ID).expect("Tu-95 bay");
    let hinge_z = o.belly as f32 / scale;
    let lining = pose.iter().filter(|f| f.address >= LINING_BASE).count();
    let skins: Vec<&Face> = pose.iter().filter(|f| BELLY.contains(&f.address)).collect();
    let lowest = skins
        .iter()
        .flat_map(|f| f.positions.iter())
        .map(|p| p[2])
        .fold(f32::MAX, f32::min);
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(lining != 0 || skins.len() != 4 || lowest < -7. - 1e-4);
        return;
    }
    m.reviewed_direction_failures += usize::from(lining != LINING);
    // A vertex of the skin stays near each hinge line.
    let leaf = o.outer as f32 / scale;
    for side in [leaf, -leaf] {
        let held = skins
            .iter()
            .flat_map(|f| f.positions.iter())
            .any(|p| (p[0] - side).abs() < 0.4 && (p[2] - hinge_z).abs() < 0.4);
        m.reviewed_anchor_missing |= !held;
    }
    let turn = value as f32 * std::f32::consts::FRAC_PI_2;
    let at = |dz: f32| (hinge_z + dz * turn.cos() - leaf * turn.sin()).min(-7.);
    let (low, high) = (at(-7. - hinge_z - 0.02), at(-6.67 - hinge_z + 0.02));
    m.reviewed_direction_failures += usize::from(lowest < low - 0.02 || lowest > high + 0.02);
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
        Control::Elevator => tailplane(value, reference, pose, scale, m),
        Control::Rudder => rudder(value, reference, pose, scale, m),
        Control::Gear => gear(value, reference, pose, scale, m),
        Control::Rotor => rotor(value, reference, pose, scale, m),
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
    // Pitch against yaw: the tailplane and the rudder are independent panels, so
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
            tailplane(pitch, &original, &pose, scale, &mut m);
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
                    "Tu-95 pitch{pitch}/yaw{yaw}: whole-panel hinge, rigidity or direction witness"
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
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
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
                "Tu-95 dense gear value{value}: rigid, direction, stow or ground gate"
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
