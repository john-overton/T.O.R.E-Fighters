//! Independent F-5E whole-panel witnesses: the two all-moving stabilators
//! (pitch only), the two ailerons (differential roll), the two trailing-edge
//! flaps (symmetric droop) and the rudder. Measures the shared Airframe output
//! and never repeats the rig's algorithm. Witness points are named in shape
//! units and scaled with `scale_point` (the F-5E's scale is the identity).
//! F5E-4 adds the three rigid gear plates (mains turn in about their top
//! edges, the nose plate turns forward), the single dorsal speed brake plate
//! and the two afterburner plumes, with the dense gear and brake sweeps.
use super::*;
use std::collections::BTreeSet;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::F5E;
const STAB_R: [usize; 2] = [0x3a9d, 0x3b8c];
const STAB_L: [usize; 2] = [0x3f2c, 0x40eb];
const AILERON_R: [usize; 2] = [0x3b45, 0x3b9f];
const AILERON_L: [usize; 2] = [0x4067, 0x41ce];
const FLAP_R: [usize; 2] = [0x44d9, 0x44f8];
const FLAP_L: [usize; 2] = [0x45c0, 0x45df];
const RUDDER: [usize; 2] = [0x4755, 0x4774];
const GEAR_MAIN_R: [usize; 2] = [0x49cc, 0x49eb];
const GEAR_MAIN_L: [usize; 2] = [0x4a37, 0x4a56];
const GEAR_NOSE: [usize; 2] = [0x4aa2, 0x4ac1];
const GEAR: [usize; 6] = [0x49cc, 0x49eb, 0x4a37, 0x4a56, 0x4aa2, 0x4ac1];
const BRAKE: [usize; 2] = [0x4802, 0x4819];
const FLAME: [usize; 8] = [
    0x48a7, 0x48c6, 0x48e5, 0x4904, 0x4923, 0x4942, 0x4961, 0x4980,
];

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

/// One surface side: its faces, the vertex pairs on its hinge or spindle whose
/// midpoint lies on the turning axis, the trailing witnesses and which control
/// moves it (and with what sign on the trailing edge's Z).
struct Panel {
    name: &'static str,
    ids: &'static [usize],
    right: bool,
    /// Pairs whose midpoint is on the axis (a pair may be one repeated point).
    pairs: &'static [([f32; 3], [f32; 3])],
    /// Trailing witnesses (shape units, right-hand values; mirrored for left).
    trailing: &'static [[f32; 3]],
    control: Control,
}

const PANELS: [Panel; 7] = [
    Panel {
        name: "right stabilator",
        ids: &STAB_R,
        right: true,
        pairs: &[([5., -23., -4.], [5., -37., -4.])],
        trailing: &[[5., -37., -4.], [20., -35., -4.]],
        control: Control::Elevator,
    },
    Panel {
        name: "left stabilator",
        ids: &STAB_L,
        right: false,
        pairs: &[([5., -23., -4.], [5., -37., -4.])],
        trailing: &[[5., -37., -4.], [20., -35., -4.]],
        control: Control::Elevator,
    },
    Panel {
        name: "right aileron",
        ids: &AILERON_R,
        right: true,
        pairs: &[
            ([31., -5., -3.], [31., -5., -4.]),
            ([43., -5., -3.], [43., -5., -4.]),
        ],
        trailing: &[[31., -8., -4.], [43., -7., -4.]],
        control: Control::Aileron,
    },
    Panel {
        name: "left aileron",
        ids: &AILERON_L,
        right: false,
        pairs: &[
            ([31., -5., -3.], [31., -5., -4.]),
            ([43., -5., -3.], [43., -5., -4.]),
        ],
        trailing: &[[31., -8., -4.], [43., -7., -4.]],
        control: Control::Aileron,
    },
    Panel {
        name: "right flap",
        ids: &FLAP_R,
        right: true,
        pairs: &[
            ([7., -4., -3.], [7., -4., -4.]),
            ([31., -5., -3.], [31., -5., -4.]),
        ],
        trailing: &[[7., -11., -4.], [31., -8., -4.]],
        control: Control::Flaps,
    },
    Panel {
        name: "left flap",
        ids: &FLAP_L,
        right: false,
        pairs: &[
            ([7., -4., -3.], [7., -4., -4.]),
            ([31., -5., -3.], [31., -5., -4.]),
        ],
        trailing: &[[7., -11., -4.], [31., -8., -4.]],
        control: Control::Flaps,
    },
    Panel {
        name: "rudder",
        ids: &RUDDER,
        right: true,
        // The quad's hinge edge (both vertices on the axis) as two degenerate pairs.
        pairs: &[
            ([0., -31., 4.], [0., -31., 4.]),
            ([0., -29., 22.], [0., -29., 22.]),
        ],
        trailing: &[[0., -37., 4.], [0., -32., 22.]],
        control: Control::Rudder,
    },
];

impl Panel {
    fn fix(&self, p: [f32; 3]) -> [f32; 3] {
        sp(if self.right { p } else { [-p[0], p[1], p[2]] })
    }
    /// Direction of a positive command on the trailing edge's Z: a positive
    /// elevator raises both stabilators, a positive aileron raises the right
    /// aileron and lowers the left, flaps lower, the rudder swings +X (X, not Z).
    fn sign(&self) -> f32 {
        match self.control {
            Control::Elevator => 1.,
            Control::Aileron if self.right => 1.,
            Control::Aileron => -1.,
            _ => -1.,
        }
    }
}

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

/// Largest distance (feet) any group vertex that was shared with a fixed face
/// in the reference moved: the opening of the moving/fixed seam. Measured and
/// reported, not a failure.
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

fn group<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}

/// Rigidity, skin gap and hinge-pair midpoints of one panel.
fn whole(panel: &Panel, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, panel.ids);
    let new = group(pose, panel.ids);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
    let error = rigidity(reference, pose, panel.ids, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    for (a, b) in panel.pairs {
        match pair_gap(&old, &new, (panel.fix(*a), panel.fix(*b)), scale) {
            Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
            None => m.reviewed_anchor_missing = true,
        }
    }
}

fn unchanged(old: &BTreeMap<FaceKey, &Face>, new: &BTreeMap<FaceKey, &Face>) -> bool {
    old.iter()
        .all(|(key, face)| new.get(key).is_some_and(|a| a.positions == face.positions))
}

/// Direction of one panel's trailing witnesses for a command `net` (the sign
/// of the command that panel obeys): Z for the stabilators, ailerons and
/// flaps, X for the rudder. Returns the number of failures.
fn direction(panel: &Panel, reference: &[Face], pose: &[Face], scale: f32, net: f32) -> usize {
    let old = group(reference, panel.ids);
    let new = group(pose, panel.ids);
    let mut failures = 0;
    for &witness in panel.trailing {
        let witness = panel.fix(witness);
        let qs = f4::witness_positions(&old, &new, witness);
        failures += usize::from(qs.is_empty());
        for q in qs {
            let (delta, want) = if matches!(panel.control, Control::Rudder) {
                ((q[0] - witness[0]) * scale, net)
            } else {
                ((q[2] - witness[2]) * scale, net * panel.sign())
            };
            failures += usize::from(delta * want.signum() <= EPSILON);
        }
    }
    failures
}

/// How far a reference vertex of one group moved in the pose (feet).
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
/// stow inward (mains) and forward (nose) and never hang below the gear-down
/// pose. Gear down is the ground contact: the lowest point is the scaled low
/// point (18 units) the flight model stands the aircraft on.
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
    // The hinge edges stay: each main's top edge and the nose's aft top corner.
    for (ids, points) in [
        (
            &GEAR_MAIN_R[..],
            vec![sp([14., -2., -4.]), sp([14., 6., -4.])],
        ),
        (
            &GEAR_MAIN_L[..],
            vec![sp([-15., -2., -4.]), sp([-15., 6., -4.])],
        ),
        (&GEAR_NOSE[..], vec![sp([0., 53., -5.])]),
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
        // scaled 18 unit low point (a third of a foot a unit).
        let want = -18. / 3. * zs;
        m.reviewed_wheel_failed |= (pose_low * scale - want).abs() > 1e-3;
    } else {
        // Direction: a main's foot moves inward and the nose foot moves
        // forward.
        for (witness, inward) in [(sp([14., -2., -18.]), -1f32), (sp([-15., -2., -18.]), 1.)] {
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
        let foot = sp([0., 53., -17.]);
        let qs = f4::witness_positions(&group(reference, &GEAR_NOSE), &keyed(pose), foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[1] <= foot[1]);
        }
    }
    if value < 1e-5 {
        // Near stow every plate has come up to its own hinge height (under
        // the wing root or inside the nose): nothing hangs below it, and the
        // mains stay within the half span they started at.
        for (ids, hinge_z) in [
            (&GEAR_MAIN_R[..], sp([0., 0., -4.])[2]),
            (&GEAR_MAIN_L[..], sp([0., 0., -4.])[2]),
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
        m.reviewed_wheel_failed |= reach > sp([15., 0., 0.])[0] + 1e-3;
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
    // The forward edge (y 29, z 10) is the hinge and never moves.
    for point in [sp([-2., 29., 10.]), sp([2., 29., 10.])] {
        let gap = pinned(reference, pose, &BRAKE, point, scale);
        m.reviewed_anchor_missing |= !gap.is_finite();
        if gap.is_finite() {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
        }
    }
    let old = group(reference, &BRAKE);
    let new = group(pose, &BRAKE);
    for witness in [sp([-2., 22., 16.]), sp([2., 22., 16.])] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[2] > witness[2] + EPSILON);
        }
    }
}

/// Each plume grows from its nozzle exit plane (y -48): the roots (y -49)
/// never pass the exit plane, the tips pull forward as the demand falls, and
/// the shared edges stay together.
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
        return;
    }
    group_skin_gap(reference, pose, &FLAME, scale, m);
    let old = group(reference, &FLAME);
    let new = group(pose, &FLAME);
    let exit = sp([0., -48., 0.])[1];
    for witness in [
        sp([-3., -49., 3.]),
        sp([-3., -49., -3.]),
        sp([3., -49., 3.]),
        sp([3., -49., -3.]),
        sp([-6., -49., 0.]),
        sp([5., -49., 0.]),
    ] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_anchor_missing |= qs.is_empty();
        for q in qs {
            // The root edge lies between the exit plane and its source row,
            // and its X and Z never change.
            m.reviewed_direction_failures +=
                usize::from(q[1] > exit + EPSILON || q[1] < witness[1] - EPSILON);
            m.max_reviewed_anchor_gap = m
                .max_reviewed_anchor_gap
                .max(((q[0] - witness[0]).abs() + (q[2] - witness[2]).abs()) * scale);
        }
    }
    for witness in [
        sp([-3., -79., 3.]),
        sp([-3., -79., -3.]),
        sp([3., -79., 3.]),
        sp([3., -79., -3.]),
        sp([-6., -79., 0.]),
        sp([5., -79., 0.]),
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
    if !matches!(
        c,
        Control::Elevator | Control::Aileron | Control::Flaps | Control::Rudder
    ) {
        return;
    }
    for panel in &PANELS {
        let old = group(reference, panel.ids);
        let new = group(pose, panel.ids);
        whole(panel, reference, pose, scale, m);
        if panel.control.name() != c.name() || value == 0. {
            // An untouched surface (a different control, or neutral) stays put:
            // the stabilators are pitch only, the ailerons roll only, the
            // flaps take the flap lever only, the rudder the pedals only.
            if !unchanged(&old, &new) {
                if value == 0. {
                    m.reviewed_neutral_mismatch = true;
                } else {
                    m.reviewed_direction_failures += 1;
                }
            }
            continue;
        }
        m.reviewed_direction_failures += direction(panel, reference, pose, scale, value as f32);
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
            for panel in &PANELS {
                whole(panel, &original, &pose, scale, &mut m);
                opening = opening.max(seam_opening(&original, &pose, panel.ids, scale));
                let command = match panel.control {
                    Control::Elevator => pitch,
                    Control::Aileron => roll,
                    _ => 0.,
                };
                if command == 0. {
                    // Pitch only for the stabilators, roll only for the
                    // ailerons, nothing for the flaps and rudder.
                    if !unchanged(&group(&original, panel.ids), &group(&pose, panel.ids)) {
                        m.reviewed_direction_failures += 1;
                    }
                } else {
                    m.reviewed_direction_failures +=
                        direction(panel, &original, &pose, scale, command as f32);
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
                    "F-5E pitch{pitch}/roll{roll}: whole-panel hinge, rigidity or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("pitch-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("pitch-roll-combinations.ppm"), &original, &poses)?;
    // The accepted corner openings, measured at full travel: each surface
    // against the faces it shares a vertex with in the neutral pose.
    let mut rows = String::from("surface,command,seam_opening_ft\n");
    for (name, edit) in [
        ("stabilator-pitch-up", 0),
        ("stabilator-pitch-down", 1),
        ("aileron-roll-right", 2),
        ("aileron-roll-left", 3),
        ("flap-full", 4),
        ("rudder-right", 5),
        ("rudder-left", 6),
        ("flap-full-aileron-right", 7),
    ] {
        let mut state = neutral.clone();
        match edit {
            0 => state.elevator = 1.,
            1 => state.elevator = -1.,
            2 => state.aileron = 1.,
            3 => state.aileron = -1.,
            4 => state.flaps = 1.,
            5 => state.rudder = 1.,
            6 => state.rudder = -1.,
            _ => {
                state.flaps = 1.;
                state.aileron = 1.;
            }
        }
        let pose = airframe.animation_faces(&state);
        for panel in PANELS.iter().filter(|p| {
            matches!(
                (edit, p.control),
                (0 | 1, Control::Elevator)
                    | (2 | 3, Control::Aileron)
                    | (4, Control::Flaps)
                    | (5 | 6, Control::Rudder)
                    | (7, Control::Flaps | Control::Aileron)
            )
        }) {
            writeln!(
                rows,
                "{name} {},full,{}",
                panel.name,
                seam_opening(&original, &pose, panel.ids, scale)
            )?;
        }
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
                "F-5E dense gear value{value}: rigid, hinge, direction or stow gate"
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

/// Speed brake sweep: the closed pose (a near-zero sample) lays the free edge
/// on the dorsal ridge line, within a hundredth of a foot, and every pose is
/// rigid with the forward edge on its hinge.
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
    let (a, b) = (sp([0., 37., 10.]), sp([0., -34., 4.]));
    let ridge = |y: f32| a[2] + (y - a[1]) / (b[1] - a[1]) * (b[2] - a[2]);
    let mut csv = String::from("value,rigidity_ft,hinge_gap_ft,free_edge_off_ridge_ft,passed\n");
    for value in [1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.brake = value;
        let pose = airframe.animation_faces(&state);
        let rigid = rigidity(&reference, &pose, &BRAKE, scale);
        let hinge_gap = pinned(&reference, &pose, &BRAKE, sp([2., 29., 10.]), scale);
        // The free edge is the aft end of the plate: its lowest-Y vertex.
        let free = pose
            .iter()
            .filter(|f| BRAKE.contains(&f.address))
            .flat_map(|f| f.positions.iter().copied())
            .min_by(|p, q| p[1].total_cmp(&q[1]))
            .unwrap_or([0.; 3]);
        let off = (free[2] - ridge(free[1])).abs() * scale;
        let closed = value < 1e-5;
        let passed = rigid <= EPSILON && hinge_gap <= EPSILON && (!closed || off <= 0.01);
        writeln!(csv, "{value},{rigid},{hinge_gap},{off},{passed}")?;
        if !passed {
            failures.push(format!(
                "F-5E brake value{value}: rigid, hinge or ridge gate"
            ));
        }
    }
    fs::write(out.join("brake-sweep.csv"), csv)?;
    Ok(failures)
}
