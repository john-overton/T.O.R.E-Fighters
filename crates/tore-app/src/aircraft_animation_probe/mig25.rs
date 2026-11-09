//! Independent MiG25 whole-panel witnesses: one hinge line per control surface,
//! whole rigid gear assemblies with painted roots and near-stow, a dorsal
//! airbrake and the afterburner plume. Measures the shared Airframe output,
//! never repeats animation algorithms.
use super::*;
const STABILATOR_R: [usize; 2] = [0x3daa, 0x3e4a];
const STABILATOR_L: [usize; 2] = [0x434c, 0x43a2];
const AILERON_R: [usize; 4] = [0x3c96, 0x3ea3, 0x3cb9, 0x3ce2];
const AILERON_L: [usize; 4] = [0x41d1, 0x43fb, 0x421d, 0x42b1];
const FLAP_R: [usize; 2] = [0x5020, 0x5047];
const FLAP_L: [usize; 2] = [0x5127, 0x514e];
const RUDDER_R: [usize; 2] = [0x320f, 0x3237];
const RUDDER_L: [usize; 2] = [0x2c1f, 0x2cb2];
const GEAR_MAIN_R: [usize; 4] = [0x4d69, 0x4d88, 0x4da7, 0x4dc6];
const GEAR_MAIN_L: [usize; 4] = [0x4e2a, 0x4e49, 0x4e68, 0x4e87];
const GEAR_NOSE: [usize; 4] = [0x4eeb, 0x4f0a, 0x4f29, 0x4f48];
const GEAR: [usize; 12] = [
    0x4d69, 0x4d88, 0x4da7, 0x4dc6, 0x4e2a, 0x4e49, 0x4e68, 0x4e87, 0x4eeb, 0x4f0a, 0x4f29, 0x4f48,
];
const FLAME: [usize; 8] = [
    0x51dc, 0x5203, 0x522a, 0x5251, 0x5278, 0x529f, 0x52c6, 0x52ed,
];
const BRAKE: [usize; 2] = [0x5367, 0x5386];
/// address group, a hinge point held fixed (a mid-chord pair for the all-moving
/// stabilator), the trailing witness, its measured axis and a positive-command
/// sign. Both skins and both diagonal triangles of every panel share one entry.
type Panel = (&'static [usize], &'static [[f32; 3]], [f32; 3], usize, f32);
fn panels(c: Control) -> &'static [Panel] {
    match c {
        Control::Elevator => &[
            (
                &STABILATOR_R,
                &[[18., -45., -1.], [18., -69., -1.]],
                [18., -69., -1.],
                2,
                1.,
            ),
            (
                &STABILATOR_L,
                &[[-18., -45., -1.], [-18., -69., -1.]],
                [-18., -69., -1.],
                2,
                1.,
            ),
        ],
        Control::Rudder => &[
            (&RUDDER_R, &[[23., -63., 38.]], [22., -73., 33.], 0, 1.),
            (&RUDDER_L, &[[-23., -63., 38.]], [-22., -73., 33.], 0, 1.),
        ],
        Control::Flaps => &[
            (
                &FLAP_R,
                &[[18., -25., 8.], [39., -29., 5.]],
                [18., -36., 6.],
                2,
                -1.,
            ),
            (
                &FLAP_L,
                &[[-18., -25., 8.], [-39., -29., 5.]],
                [-18., -36., 6.],
                2,
                -1.,
            ),
        ],
        Control::Aileron => &[
            (
                &AILERON_R,
                &[[39., -29., 5.], [70., -22., 2.]],
                [70., -44., 2.],
                2,
                1.,
            ),
            (
                &AILERON_L,
                &[[-39., -29., 5.], [-70., -22., 2.]],
                [-70., -44., 2.],
                2,
                -1.,
            ),
            // The all-moving stabilators take differential roll too: a positive
            // aileron raises the right trailing edge and lowers the left, the
            // same sense as the outboard ailerons. The root chord midpoint stays
            // the anchor.
            (
                &STABILATOR_R,
                &[[18., -45., -1.], [18., -69., -1.]],
                [18., -69., -1.],
                2,
                1.,
            ),
            (
                &STABILATOR_L,
                &[[-18., -45., -1.], [-18., -69., -1.]],
                [-18., -69., -1.],
                2,
                -1.,
            ),
        ],
        _ => &[],
    }
}
/// True when the group is one all-moving stabilator plate, whose invariant is
/// its root chord midpoint rather than any single corner.
fn stabilator_panel(ids: &[usize]) -> bool {
    ids == STABILATOR_R.as_slice() || ids == STABILATOR_L.as_slice()
}
/// The midpoint of every occurrence of the given source corners, followed from
/// reference to pose. The all-moving stabilator has no fixed source corner, so
/// its root chord midpoint is the invariant instead.
fn midpoint_gap(
    old: &BTreeMap<FaceKey, &Face>,
    new: &BTreeMap<FaceKey, &Face>,
    corners: &[[f32; 3]],
    scale: f32,
) -> Option<f32> {
    let mut moved = Vec::new();
    for (key, face) in old {
        for (i, p) in face.positions.iter().enumerate() {
            if corners.contains(p)
                && let Some(q) = new.get(key).and_then(|f| f.positions.get(i))
            {
                moved.push(*q);
            }
        }
    }
    if moved.is_empty() {
        return None;
    }
    let average: [f32; 3] =
        std::array::from_fn(|k| moved.iter().map(|p| p[k]).sum::<f32>() / moved.len() as f32);
    let middle: [f32; 3] =
        std::array::from_fn(|k| corners.iter().map(|p| p[k]).sum::<f32>() / corners.len() as f32);
    Some(distance(average, middle) * scale)
}
/// Maximum change of any vertex-to-vertex distance inside one group. A rigid
/// assembly preserves every distance; a bent card or a card left behind does not.
fn rigidity(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
    let before = keyed(reference);
    let after = keyed(pose);
    let mut old = Vec::new();
    let mut now = Vec::new();
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
    let old = keyed(reference);
    let new = keyed(pose);
    let mut gap = 0f32;
    let selected: BTreeMap<_, _> = old
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect();
    let moved = f4::witness_positions(&selected, &new, point);
    gap = gap.max(if moved.is_empty() {
        f32::INFINITY
    } else {
        moved
            .iter()
            .map(|q| distance(point, *q) * scale)
            .fold(0f32, f32::max)
    });
    gap
}
fn lowest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min)
}
pub(super) fn check(
    c: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    m: &mut Metrics,
) {
    let (_, reference, pose) = geometry;
    if matches!(c, Control::Gear) {
        gear(value, reference, pose, scale, m);
        return;
    }
    if matches!(c, Control::Brake) {
        brake(value, reference, pose, scale, m);
        return;
    }
    if matches!(c, Control::Exhaust) {
        exhaust(value, reference, pose, scale, m);
        return;
    }
    for &(ids, anchor, witness, axis, sign) in panels(c) {
        let old: BTreeMap<_, _> = keyed(reference)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        // Whole-surface gate: every face of the panel shares one group, so a
        // half-moved skin or diagonal triangle shows as a seam gap.
        if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
            m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
        }
        if value == 0. {
            for (key, face) in &old {
                match new.get(key) {
                    Some(after) if face.positions == after.positions => {}
                    _ => m.reviewed_neutral_mismatch = true,
                }
            }
            continue;
        }
        if matches!(c, Control::Elevator) || stabilator_panel(ids) {
            // The all-moving stabilator keeps its root chord midpoint, not a
            // single corner, so the anchor pair is averaged. It does this on
            // pitch and on differential roll alike.
            match midpoint_gap(&old, &new, anchor, scale) {
                Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
                None => m.reviewed_anchor_missing = true,
            }
        } else {
            for p in anchor {
                let qs = f4::witness_positions(&old, &new, *p);
                m.reviewed_anchor_missing |= qs.is_empty();
                for q in qs {
                    m.max_reviewed_anchor_gap =
                        m.max_reviewed_anchor_gap.max(distance(*p, q) * scale);
                }
            }
        }
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            let delta = (q[axis] - witness[axis]) * scale;
            m.reviewed_direction_failures +=
                usize::from(delta * sign * value.signum() as f32 <= EPSILON);
        }
    }
}
/// The two cards of each gear assembly share one lateral hinge and move as one
/// rigid body: the strut and painted wheel card cannot separate, the hinge root
/// cannot move, the whole group rises as it stows, and at near-stow nothing
/// hangs below the fuselage belly. The two main sides keep their own clearance.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    for ids in [&GEAR_MAIN_R, &GEAR_MAIN_L, &GEAR_NOSE] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
    }
    for (ids, root) in [
        (&GEAR_MAIN_R, [12., -19., -10.]),
        (&GEAR_MAIN_L, [-12., -19., -10.]),
        (&GEAR_NOSE, [0., 45., -9.]),
    ] {
        let gap = pinned(reference, pose, ids, root, scale);
        m.reviewed_anchor_missing |= !gap.is_finite();
        if gap.is_finite() {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
        }
    }
    // The wheel cards must stay on their own side of the centreline.
    let right = pose
        .iter()
        .filter(|f| GEAR_MAIN_R.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[0] * scale)
        .fold(f32::INFINITY, f32::min);
    let left = pose
        .iter()
        .filter(|f| GEAR_MAIN_L.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[0] * scale)
        .fold(f32::NEG_INFINITY, f32::max);
    m.reviewed_min_wheel_gap = Some(right - left);
    m.reviewed_wheel_failed |= right - left < 0.70;
    // Closing must raise the assembly, never lower it.
    let reference_low = lowest(reference, &GEAR);
    let pose_low = lowest(pose, &GEAR);
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value < 1e-5 {
        // Near-stow: the whole group tucks to the fuselage belly plane (-10).
        m.reviewed_wheel_failed |= pose_low < -10.5;
        for f in pose.iter().filter(|f| GEAR.contains(&f.address)) {
            m.reviewed_wheel_failed |= f.positions.iter().any(|p| p[2] < -10.5);
        }
    }
}
fn brake(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| BRAKE.contains(&f.address)));
        return;
    }
    if value == 1. {
        let old = keyed(reference);
        let new = keyed(pose);
        for (key, face) in old.iter().filter(|(k, _)| BRAKE.contains(&k.0)) {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
    }
    // The two faces are opposite skins of one quad: shared vertices must move
    // together, so the panel cannot half-open about its hinge.
    let old: BTreeMap<_, _> = keyed(reference)
        .into_iter()
        .filter(|(k, _)| BRAKE.contains(&k.0))
        .collect();
    let new: BTreeMap<_, _> = keyed(pose)
        .into_iter()
        .filter(|(k, _)| BRAKE.contains(&k.0))
        .collect();
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
    if rigidity(reference, pose, &BRAKE, scale) > EPSILON {
        m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(1.);
    }
    // Both front corners sit on the spine hinge and never move.
    m.max_reviewed_anchor_gap = m
        .max_reviewed_anchor_gap
        .max(pinned(reference, pose, &BRAKE, [5., 3., 9.], scale))
        .max(pinned(reference, pose, &BRAKE, [-5., 3., 9.], scale));
    // Closing lowers the tip; the source raised pose is the open endpoint.
    for witness in [[2., -13., 23.], [-2., -13., 23.]] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            if q[2] > witness[2] + EPSILON {
                m.reviewed_direction_failures += 1;
            }
        }
    }
}
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
        return;
    }
    let old: BTreeMap<_, _> = keyed(reference)
        .into_iter()
        .filter(|(k, _)| FLAME.contains(&k.0))
        .collect();
    let new: BTreeMap<_, _> = keyed(pose)
        .into_iter()
        .filter(|(k, _)| FLAME.contains(&k.0))
        .collect();
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
    // The four nozzle-edge corners stay at Y=-70 as the plume shortens.
    for witness in [
        [10., -70., 8.],
        [-10., -70., 8.],
        [18., -70., 0.],
        [-18., -70., 0.],
    ] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_anchor_missing |= qs.is_empty();
        for q in qs {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(witness, q) * scale);
        }
    }
    // The plume tip pulls forward toward the nozzle as the demand falls.
    for witness in [[10., -129., 8.], [-10., -129., -8.], [18., -129., 0.]] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            if q[1] < witness[1] - EPSILON {
                m.reviewed_direction_failures += 1;
            }
        }
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
        String::from("flap,roll,finite,anchor_gap_ft,skin_gap_ft,crossings,direction_failures\n");
    let mut poses = Vec::new();
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.flaps = flap;
            state.aileron = roll;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            check(
                Control::Flaps,
                flap,
                (&original, &original, &pose),
                scale,
                &mut m,
            );
            check(
                Control::Aileron,
                roll,
                (&original, &original, &pose),
                scale,
                &mut m,
            );
            let passed = m.finite
                && !m.reviewed_anchor_missing
                && m.max_reviewed_anchor_gap <= EPSILON
                && m.max_reviewed_skin_gap <= EPSILON
                && !m.reviewed_neutral_mismatch
                && m.reviewed_direction_failures == 0
                && m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{flap},{roll},{},{},{},{},{}",
                m.finite,
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "MiG25 flap{flap}/roll{roll} whole-panel hinge, seam or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    // Dense gear sweep: the whole assembly stays rigid and rises to the belly,
    // including a near-stow sample before hiding.
    let mut gear_endpoint = neutral.clone();
    Control::Gear.apply(&mut gear_endpoint, 1.);
    let reference = airframe.animation_faces(&gear_endpoint);
    let mut csv =
        String::from("value,anchor_gap_ft,skin_gap_ft,rigidity_ft,crossings,checks_passed\n");
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
            (&original, &reference, &pose),
            scale,
            &mut m,
        );
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.max_reviewed_skin_gap <= EPSILON
            && !m.reviewed_wheel_failed
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{},{},{passed}",
            m.max_reviewed_anchor_gap,
            m.max_reviewed_skin_gap,
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "MiG25 dense gear value{value}: rigid/root/stow gate"
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
