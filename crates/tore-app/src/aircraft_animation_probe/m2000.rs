//! Independent Mirage 2000 whole-panel witnesses: the four delta elevons (one
//! angle per side, inboard and outboard strip together), the rudder with its
//! two tip triangles, the three rigid gear plates (mains turn in about their
//! top edges, the nose plate turns aft), the single dorsal airbrake plate and
//! the afterburner plume. Measures the shared Airframe output and never repeats
//! the rig's algorithm. Witness points are named in shape units and scaled with the
//! aircraft's own per-axis scale (the probe geometry is the scaled shape).
use super::*;
use std::collections::BTreeSet;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::M2000;
const ELEVON_R: [usize; 6] = [0x3969, 0x394a, 0x2176, 0x2391, 0x2190, 0x2233];
const ELEVON_L: [usize; 6] = [0x3a31, 0x3a50, 0x1e87, 0x23c3, 0x1ea1, 0x1f1b];
const RUDDER: [usize; 4] = [0x3bc6, 0x3be5, 0x2b64, 0x2b7c];
const GEAR_MAIN_L: [usize; 2] = [0x380b, 0x382a];
const GEAR_MAIN_R: [usize; 2] = [0x3876, 0x3895];
const GEAR_NOSE: [usize; 2] = [0x38e1, 0x3900];
const GEAR: [usize; 6] = [0x380b, 0x382a, 0x3876, 0x3895, 0x38e1, 0x3900];
const BRAKE: [usize; 2] = [0x3c73, 0x3c92];
const FLAME: [usize; 8] = [
    0x3cf8, 0x3d17, 0x3d36, 0x3d55, 0x3d74, 0x3d93, 0x3db2, 0x3dd1,
];

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}
fn mirror(p: [f32; 3]) -> [f32; 3] {
    [-p[0], p[1], p[2]]
}

/// One elevon side: its faces, the hinge-edge vertex pairs whose midpoint lies
/// on the mid axis (y -29, z -3), and the trailing witnesses.
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
    fn fix(&self, p: [f32; 3]) -> [f32; 3] {
        sp(if self.right { p } else { mirror(p) })
    }
    /// Hinge-edge vertex pairs (top layer z -2, bottom layer z -4) at the wedge
    /// root, the strip seam and the tip. The tip pair is one vertex, which is on
    /// the mid axis itself.
    fn pairs(&self) -> [([f32; 3], [f32; 3]); 3] {
        let seam = if self.right { 17. } else { 18. };
        let x = |v: f32| if self.right { v } else { -v };
        [
            (sp([x(3.), -29., -2.]), sp([x(3.), -29., -4.])),
            (sp([x(seam), -29., -2.]), sp([x(seam), -29., -4.])),
            (sp([x(33.), -29., -3.]), sp([x(33.), -29., -3.])),
        ]
    }
    /// Trailing-edge vertices on the inboard and the outboard strip.
    fn trailing(&self) -> [[f32; 3]; 2] {
        [self.fix([3., -32., -3.]), self.fix([33., -32., -3.])]
    }
    /// Direction of a positive command on the trailing edge's Z: elevator up,
    /// aileron up on the right and down on the left, flaps down.
    fn sign(&self, c: Control) -> f32 {
        match c {
            Control::Elevator => 1.,
            Control::Aileron if self.right => 1.,
            Control::Aileron => -1.,
            _ => -1.,
        }
    }
}

/// Distance the midpoint of one hinge-edge pair moved, followed from the
/// reference faces to the pose faces by their first occurrence.
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

/// Largest distance (feet) any group vertex that was shared with a fixed face in
/// the reference moved: the opening of the moving/fixed seam. Not a failure
/// (a hinge vertex 1 unit off the mid axis must move); reported so the opening
/// is measured and bounded.
fn seam_opening(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
    let before = keyed(reference);
    let after = keyed(pose);
    let fixed: BTreeSet<[i64; 3]> = before
        .iter()
        .filter(|(k, _)| !ids.contains(&k.0))
        .flat_map(|(_, f)| f.positions.iter().map(|p| quantized(*p)))
        .collect();
    let mut worst = 0f32;
    for (key, face) in before.iter().filter(|(k, _)| ids.contains(&k.0)) {
        let Some(moved) = after.get(key) else {
            continue;
        };
        for (a, b) in face.positions.iter().zip(&moved.positions) {
            if fixed.contains(&quantized(*a)) {
                worst = worst.max(distance(*a, *b) * scale);
            }
        }
    }
    worst
}

fn pinned(reference: &[Face], pose: &[Face], ids: &[usize], point: [f32; 3], scale: f32) -> f32 {
    let old: BTreeMap<_, _> = keyed(reference)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect();
    let new = keyed(pose);
    let moved = f4::witness_positions(&old, &new, point);
    if moved.is_empty() {
        f32::INFINITY
    } else {
        moved
            .iter()
            .map(|q| distance(point, *q) * scale)
            .fold(0f32, f32::max)
    }
}

fn group<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

/// Whole-wedge checks shared by the control sweep and the combination grid:
/// the seam between strips, rigidity and the hinge-pair midpoints.
fn wedge(side: &Side, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.ids);
    let new = group(pose, side.ids);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
    let error = rigidity(reference, pose, side.ids, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    for pair in side.pairs() {
        match pair_gap(&old, &new, pair, scale) {
            Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
            None => m.reviewed_anchor_missing = true,
        }
    }
}

fn rudder_group(reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, &RUDDER);
    let new = group(pose, &RUDDER);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
    let error = rigidity(reference, pose, &RUDDER, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The only fixed vertices are on the hinge edge: both stay put.
    for point in [sp([0., -35., 4.]), sp([0., -40., 19.])] {
        let gap = pinned(reference, pose, &RUDDER, point, scale);
        m.reviewed_anchor_missing |= !gap.is_finite();
        if gap.is_finite() {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
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

/// Skin gap inside one group: shared vertices of the two windings of a plate
/// must move together.
fn group_skin_gap(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let old = group(reference, ids);
    let new = group(pose, ids);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
}

/// The three gear plates stay whole rigid pieces, hinge on their own edges,
/// stow inward (mains) and aft (nose) and never hang below the gear-down pose.
/// Gear down is the ground contact: the lowest point is the scaled low point.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    let zs = sp([0., 0., 1.])[2];
    for ids in [&GEAR_MAIN_R[..], &GEAR_MAIN_L[..], &GEAR_NOSE[..]] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
        group_skin_gap(reference, pose, ids, scale, m);
    }
    // The hinge edges stay: each main's top edge and the nose forward top corner.
    for (ids, points) in [
        (
            &GEAR_MAIN_R[..],
            vec![sp([7., -12., -3.]), sp([7., -18., -3.])],
        ),
        (
            &GEAR_MAIN_L[..],
            vec![sp([-7., -12., -3.]), sp([-7., -18., -3.])],
        ),
        (&GEAR_NOSE[..], vec![sp([0., 37., -5.])]),
    ] {
        for point in points {
            let gap = pinned(reference, pose, ids, point, scale);
            m.reviewed_anchor_missing |= !gap.is_finite();
            if gap.is_finite() {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
            }
        }
    }
    // Closing never lowers the plates.
    let reference_low = lowest(reference, &GEAR);
    let pose_low = lowest(pose, &GEAR);
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // Ground contact: the lowest point of the gear-down pose is the
        // scaled 12 unit low point the flight model stands the aircraft on
        // (12 units at a third of a foot, times the height factor).
        let want = -12. / 3. * zs;
        m.reviewed_wheel_failed |= (pose_low * scale - want).abs() > 1e-3;
    } else {
        // Direction: a main's foot moves inward and a nose foot moves aft.
        for (witness, inward) in [(sp([7., -12., -12.]), -1f32), (sp([-7., -12., -12.]), 1.)] {
            let ids = if inward < 0. {
                &GEAR_MAIN_R
            } else {
                &GEAR_MAIN_L
            };
            let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), witness);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                m.reviewed_direction_failures += usize::from((q[0] - witness[0]) * inward <= 0.);
            }
        }
        let foot = sp([0., 37., -12.]);
        let qs = f4::witness_positions(&group(reference, &GEAR_NOSE), &keyed(pose), foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[1] >= foot[1]);
        }
    }
    if value < 1e-5 {
        // Near stow every plate has come up to its own hinge height (inside
        // the wing root or the fuselage): nothing hangs below it, and the
        // mains stay within the half span they started at.
        for (ids, hinge_z) in [
            (&GEAR_MAIN_R[..], sp([0., 0., -3.])[2]),
            (&GEAR_MAIN_L[..], sp([0., 0., -3.])[2]),
            (&GEAR_NOSE[..], sp([0., 0., -5.])[2]),
        ] {
            m.reviewed_wheel_failed |= lowest(pose, ids) < hinge_z - 1e-3;
        }
        let reach = pose
            .iter()
            .filter(|f| GEAR_MAIN_R.contains(&f.address) || GEAR_MAIN_L.contains(&f.address))
            .flat_map(|f| &f.positions)
            .map(|p| p[0].abs())
            .fold(0f32, f32::max);
        m.reviewed_wheel_failed |= reach > sp([7., 0., 0.])[0] + 1e-3;
    }
}

/// The dorsal plate: open (1) is the source pose, closing turns the whole
/// plate about its forward edge, never raising its free (aft) edge.
fn brake(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| BRAKE.contains(&f.address)));
        return;
    }
    if value == 1. {
        let old = group(reference, &BRAKE);
        let new = group(pose, &BRAKE);
        for (key, face) in &old {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
    }
    group_skin_gap(reference, pose, &BRAKE, scale, m);
    let error = rigidity(reference, pose, &BRAKE, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The forward edge (y 24) is the hinge and never moves.
    for point in [sp([-1., 24., 5.]), sp([1., 24., 5.])] {
        let gap = pinned(reference, pose, &BRAKE, point, scale);
        m.reviewed_anchor_missing |= !gap.is_finite();
        if gap.is_finite() {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
        }
    }
    let old = group(reference, &BRAKE);
    let new = group(pose, &BRAKE);
    for witness in [sp([-1., 17., 10.]), sp([1., 17., 10.])] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[2] > witness[2] + EPSILON);
        }
    }
}

/// The plume grows from the nozzle exit plane (y -44): the exit corners stay,
/// the tips pull forward as the demand falls, and shared edges stay together.
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
        return;
    }
    group_skin_gap(reference, pose, &FLAME, scale, m);
    let old = group(reference, &FLAME);
    let new = group(pose, &FLAME);
    for witness in [
        sp([0., -44., 3.]),
        sp([0., -44., -4.]),
        sp([4., -44., 0.]),
        sp([-3., -44., 0.]),
    ] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_anchor_missing |= qs.is_empty();
        for q in qs {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(witness, q) * scale);
        }
    }
    for witness in [
        sp([0., -80., 3.]),
        sp([0., -80., -4.]),
        sp([4., -80., 0.]),
        sp([-3., -80., 0.]),
    ] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            if value < 1. {
                m.reviewed_direction_failures += usize::from(q[1] <= witness[1] + EPSILON);
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
        Control::Gear => return gear(value, reference, pose, scale, m),
        Control::Brake => return brake(value, reference, pose, scale, m),
        Control::Exhaust => return exhaust(value, reference, pose, scale, m),
        _ => {}
    }
    match c {
        Control::Elevator | Control::Aileron | Control::Flaps => {
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
            rudder_group(reference, pose, scale, m);
            let old = group(reference, &RUDDER);
            let new = group(pose, &RUDDER);
            if value == 0. {
                for (key, face) in &old {
                    match new.get(key) {
                        Some(after) if face.positions == after.positions => {}
                        _ => m.reviewed_neutral_mismatch = true,
                    }
                }
                return;
            }
            // Trailing vertices swing to +X for a positive command.
            for witness in [sp([0., -39., 5.]), sp([0., -42., 19.]), sp([0., -42., 21.])] {
                let qs = f4::witness_positions(&old, &new, witness);
                m.reviewed_direction_failures += usize::from(qs.is_empty());
                for q in qs {
                    m.reviewed_direction_failures +=
                        usize::from((q[0] - witness[0]) * value.signum() as f32 <= EPSILON);
                }
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
    let mut rows = String::from(
        "pitch,roll,finite,anchor_gap_ft,skin_gap_ft,rigidity_ft,seam_opening_ft,crossings,direction_failures\n",
    );
    let mut poses = Vec::new();
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.aileron = roll;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            let mut opening = 0f32;
            for side in &SIDES {
                wedge(side, &original, &pose, scale, &mut m);
                opening = opening.max(seam_opening(&original, &pose, side.ids, scale));
                // Direction: the net command on a trailing edge has the sign of
                // 3 * pitch (pull raises both) plus 2 * roll (up on the right,
                // down on the left); equal and opposite terms cancel.
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
                && m.max_reviewed_skin_gap <= EPSILON
                && m.reviewed_rigid_panel_error <= EPSILON
                && m.reviewed_direction_failures == 0
                && m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{pitch},{roll},{},{},{},{},{opening},{},{}",
                m.finite,
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "Mirage 2000 pitch{pitch}/roll{roll}: whole-wedge hinge, seam, rigidity or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("pitch-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("pitch-roll-combinations.ppm"), &original, &poses)?;
    // The seam opening of the rudder tip and of each elevon at full travel, so
    // the accepted corner openings are measured next to the grid.
    let mut rows = String::from("surface,command,seam_opening_ft\n");
    for (name, edit) in [
        ("elevon-pitch-up", 0),
        ("elevon-pitch-down", 1),
        ("elevon-flaps", 2),
        ("rudder-right", 3),
        ("rudder-left", 4),
    ] {
        let mut state = neutral.clone();
        match edit {
            0 => state.elevator = 1.,
            1 => state.elevator = -1.,
            2 => state.flaps = 1.,
            3 => state.rudder = 1.,
            _ => state.rudder = -1.,
        }
        let pose = airframe.animation_faces(&state);
        let ids: &[usize] = if edit >= 3 { &RUDDER } else { &ELEVON_R };
        writeln!(
            rows,
            "{name},full,{}",
            seam_opening(&original, &pose, ids, scale)
        )?;
    }
    fs::write(out.join("seam-opening.csv"), rows)?;
    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_brake(airframe, neutral, out, scale)?);
    Ok(failures)
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, hinged, grounded at 1 and tucked at the hinge height near 0.
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
            (original, &reference, &pose),
            scale,
            &mut m,
        );
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.max_reviewed_skin_gap <= EPSILON
            && m.reviewed_direction_failures == 0
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
                "Mirage 2000 dense gear value{value}: rigid, hinge, direction or stow gate"
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

/// Airbrake sweep: the closed pose (a near-zero sample) lays the free edge on
/// the spine line, within a hundredth of a foot, and every pose is rigid.
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
    let hinge = sp([0., 24., 5.]);
    let spine = {
        let (a, b) = (sp([0., 25., 5.]), sp([0., -1., 4.]));
        [b[1] - a[1], b[2] - a[2]]
    };
    let mut csv = String::from("value,rigidity_ft,hinge_gap_ft,free_edge_off_spine_ft,passed\n");
    for value in [1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.brake = value;
        let pose = airframe.animation_faces(&state);
        let rigid = rigidity(&reference, &pose, &BRAKE, scale);
        let hinge_gap = pinned(&reference, &pose, &BRAKE, hinge_edge(), scale);
        let front = pose
            .iter()
            .find(|f| BRAKE.contains(&f.address))
            .map(|f| [f.positions[0][1] - hinge[1], f.positions[0][2] - hinge[2]])
            .unwrap_or([0.; 2]);
        let length = (spine[0] * spine[0] + spine[1] * spine[1]).sqrt();
        let off = ((front[0] * spine[1] - front[1] * spine[0]) / length).abs() * scale;
        let closed = value < 1e-5;
        let passed = rigid <= EPSILON && hinge_gap <= EPSILON && (!closed || off <= 0.01);
        writeln!(csv, "{value},{rigid},{hinge_gap},{off},{passed}")?;
        if !passed {
            failures.push(format!(
                "Mirage 2000 brake value{value}: rigid, hinge or spine gate"
            ));
        }
    }
    fs::write(out.join("brake-sweep.csv"), csv)?;
    Ok(failures)
}

fn hinge_edge() -> [f32; 3] {
    sp([1., 24., 5.])
}
