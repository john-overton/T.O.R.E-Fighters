//! Independent Mirage F1 whole-panel witnesses: the two all-moving tailplane
//! panels (pitch only), the two outboard trailing-edge panels acting as
//! ailerons (differential roll) and the rudder. The flap lever must move none of
//! them. Measures the shared Airframe output and never repeats the rig's
//! algorithm. Witness points are named in shape units and scaled with
//! `scale_point` (the probe geometry is the scaled shape). MF1-4 adds the three
//! rigid gear plates (mains forward, nose rearward), the single dorsal airbrake
//! plate and the one afterburner plume, with the dense gear and brake sweeps.
use super::*;
use std::collections::BTreeSet;
use tore_formats::aircraft::scale::scale_point;

const ID: AircraftId = AircraftId::Mf1;
const TAIL_R: [usize; 2] = [0x2d0e, 0x2d36];
const TAIL_L: [usize; 2] = [0x2c60, 0x2c88];
const AILERON_R: [usize; 4] = [0x3ce8, 0x3cc9, 0x3d0f, 0x3d30];
const AILERON_L: [usize; 4] = [0x3e93, 0x3e74, 0x3eba, 0x3edb];
const RUDDER: [usize; 2] = [0x40e1, 0x4108];
const GEAR_MAIN_L: [usize; 2] = [0x3b7e, 0x3b9d];
const GEAR_MAIN_R: [usize; 2] = [0x3be9, 0x3c08];
const GEAR_NOSE: [usize; 2] = [0x3c54, 0x3c73];
const GEAR: [usize; 6] = [0x3b7e, 0x3b9d, 0x3be9, 0x3c08, 0x3c54, 0x3c73];
const BRAKE: [usize; 2] = [0x4182, 0x4199];
const FLAME: [usize; 8] = [
    0x4207, 0x422e, 0x4255, 0x427c, 0x42a3, 0x42ca, 0x42f1, 0x4318,
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

const PANELS: [Panel; 5] = [
    Panel {
        name: "right tailplane",
        ids: &TAIL_R,
        right: true,
        // Root chord ends; the lateral spindle passes through their midpoint.
        pairs: &[([6., -22., -1.], [6., -41., -1.])],
        trailing: &[[6., -41., -1.], [23., -45., -1.]],
        control: Control::Elevator,
    },
    Panel {
        name: "left tailplane",
        ids: &TAIL_L,
        right: false,
        pairs: &[([6., -22., -1.], [6., -41., -1.])],
        // The left tip is one unit shorter than the right (source).
        trailing: &[[6., -41., -1.], [22., -45., -1.]],
        control: Control::Elevator,
    },
    Panel {
        name: "right aileron",
        ids: &AILERON_R,
        right: true,
        // The hinge vertex pairs (top and bottom of the wedge) at the root seam
        // and the tip; their midpoints are on the hinge line.
        pairs: &[
            ([23., -6., 2.], [23., -6., 0.]),
            ([35., -11., 2.], [35., -11., 1.]),
        ],
        trailing: &[[23., -12., 1.], [35., -15., 1.]],
        control: Control::Aileron,
    },
    Panel {
        name: "left aileron",
        ids: &AILERON_L,
        right: false,
        pairs: &[
            ([23., -6., 2.], [23., -6., 0.]),
            ([35., -11., 2.], [35., -11., 1.]),
        ],
        trailing: &[[23., -12., 1.], [35., -15., 1.]],
        control: Control::Aileron,
    },
    Panel {
        name: "rudder",
        ids: &RUDDER,
        right: true,
        // The quad's hinge edge (both vertices on the axis) as two degenerate pairs.
        pairs: &[
            ([0., -30., 3.], [0., -30., 3.]),
            ([0., -39., 22.], [0., -39., 22.]),
        ],
        trailing: &[[0., -37., 3.], [0., -44., 22.]],
        control: Control::Rudder,
    },
];

impl Panel {
    fn fix(&self, p: [f32; 3]) -> [f32; 3] {
        sp(if self.right { p } else { [-p[0], p[1], p[2]] })
    }
    /// Direction of a positive command on the trailing edge's Z: a positive
    /// elevator raises both tailplanes, a positive aileron raises the right
    /// aileron and lowers the left, the rudder swings +X (X, not Z).
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
/// of the command that panel obeys): Z for the tailplanes, ailerons and
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

fn pin(
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

fn lowest(pose: &[Face], ids: &[usize]) -> f32 {
    pose.iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| &f.positions)
        .map(|p| p[2])
        .fold(f32::INFINITY, f32::min)
}

/// Extreme of one coordinate over a group's vertices in the pose.
fn extreme(pose: &[Face], ids: &[usize], axis: usize, max: bool) -> f32 {
    let values = pose
        .iter()
        .filter(|f| ids.contains(&f.address))
        .flat_map(|f| f.positions.iter().map(move |p| p[axis]));
    if max {
        values.fold(f32::NEG_INFINITY, f32::max)
    } else {
        values.fold(f32::INFINITY, f32::min)
    }
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

/// Half width (shape units) of the fuselage section at height `z` and station
/// `y` (aft of the intake shoulder), a hull model read off the neutral skin
/// faces: the section at y 7 runs from z -8 (2 wide) through z -7 (6), z -4 to
/// 1 (7, widest 8 at z -2), z 3 (6) up to the dorsal ridge z 5 (2); farther aft
/// it narrows linearly to 0.72 of that at y -39. Negative outside the section.
fn hull_half_width(z: f32, y: f32) -> f32 {
    const SECTION: [(f32, f32); 7] = [
        (-8., 2.),
        (-7., 6.),
        (-4., 7.),
        (-2., 8.),
        (1., 7.),
        (3., 6.),
        (5., 2.),
    ];
    if !(-8. ..=5.).contains(&z) {
        return -1.;
    }
    let width = SECTION
        .windows(2)
        .find(|w| (w[0].0..=w[1].0).contains(&z))
        .map_or(-1., |w| {
            w[0].1 + (w[1].1 - w[0].1) * (z - w[0].0) / (w[1].0 - w[0].0)
        });
    let aft = ((y + 39.) / 46.).clamp(0., 1.);
    width * (0.72 + 0.28 * aft)
}

/// The three gear plates stay whole rigid pieces hinged on their own top
/// corner, the mains swing forward and the nose swings rearward, nothing hangs
/// lower than the gear-down pose, and gear down is the ground contact.
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
    // The hinges stay: each main's hinge point (on its top edge, 1 unit
    // forward of the rear corner, found between the two top corners) and the
    // nose's top forward corner.
    for (ids, x) in [(&GEAR_MAIN_R[..], 3.), (&GEAR_MAIN_L[..], -3.)] {
        let old = group(reference, ids);
        let new = group(pose, ids);
        let ends = [sp([x, -5., -6.]), sp([x, 4., -6.])];
        let found: Vec<_> = ends
            .iter()
            .map(|&e| f4::witness_positions(&old, &new, e).into_iter().next())
            .collect();
        match (found[0], found[1]) {
            (Some(a), Some(b)) => {
                let at = |a: [f32; 3], b: [f32; 3]| -> [f32; 3] {
                    std::array::from_fn(|k| a[k] + (b[k] - a[k]) / 9.)
                };
                let hinge = at(ends[0], ends[1]);
                m.max_reviewed_anchor_gap = m
                    .max_reviewed_anchor_gap
                    .max(distance(hinge, at(a, b)) * scale);
            }
            _ => m.reviewed_anchor_missing = true,
        }
    }
    pin(reference, pose, &GEAR_NOSE, sp([0., 47., -7.]), scale, m);
    // Closing never lowers the plates, except for the main feet's first few
    // degrees: the foot swings inward on an inclined axis, so its rear corner
    // dips 0.08 ft at most (the gate allows 0.1 ft).
    let reference_low = lowest(reference, &GEAR);
    let pose_low = lowest(pose, &GEAR);
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - 0.1 / scale;
    if value == 1. {
        // Ground contact: the lowest point of the gear-down pose is the scaled
        // 17 unit low point the flight model stands the aircraft on (17 units
        // at a third of a foot, times the height factor).
        let want = -17. / 3. * zs;
        m.reviewed_wheel_failed |= (pose_low * scale - want).abs() > 1e-3;
    } else {
        // Direction: a main's foot moves forward and the nose front foot aft.
        for (ids, witness, forward) in [
            (&GEAR_MAIN_R[..], sp([7., -5., -17.]), true),
            (&GEAR_MAIN_L[..], sp([-7., -5., -17.]), true),
            (&GEAR_NOSE[..], sp([0., 47., -17.]), false),
        ] {
            let qs = f4::witness_positions(&group(reference, ids), &keyed(pose), witness);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                let moved = q[1] - witness[1];
                m.reviewed_direction_failures +=
                    usize::from(if forward { moved <= 0. } else { moved >= 0. });
            }
        }
    }
    if value < 1e-5 {
        // Near stow each main lies inside the fuselage side: every vertex is
        // within the hull half-width of its height and station (the hull
        // section read off the fuselage skin faces), within a tenth of a
        // unit, between the belly (z -7.5) and the roof (z 3.5) and between
        // y -5 and 7. The nose plate rises to its hinge height or above and
        // stays between y 37 and 51 (the nose bay).
        let tol = 1e-3;
        let (xs, ys, zs) = (
            sp([1., 0., 0.])[0],
            sp([0., 1., 0.])[1],
            sp([0., 0., 1.])[2],
        );
        for ids in [&GEAR_MAIN_R[..], &GEAR_MAIN_L[..]] {
            for f in pose.iter().filter(|f| ids.contains(&f.address)) {
                for p in &f.positions {
                    let (x, y, z) = (p[0].abs() / xs, p[1] / ys, p[2] / zs);
                    m.reviewed_wheel_failed |= x > hull_half_width(z, y) + 0.1
                        || !(-7.5..=3.5).contains(&z)
                        || !(-5.01..=7.0).contains(&y);
                }
            }
        }
        m.reviewed_wheel_failed |= lowest(pose, &GEAR_NOSE) < sp([0., 0., -7.])[2] - tol
            || extreme(pose, &GEAR_NOSE, 1, false) < sp([0., 37., 0.])[1] - tol
            || extreme(pose, &GEAR_NOSE, 1, true) > sp([0., 51., 0.])[1] + tol;
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
    // The forward edge (y 29) is the hinge and never moves.
    for point in [sp([-2., 29., 5.]), sp([3., 29., 5.])] {
        pin(reference, pose, &BRAKE, point, scale, m);
    }
    let old = group(reference, &BRAKE);
    let new = group(pose, &BRAKE);
    for witness in [sp([-2., 19., 14.]), sp([2., 19., 14.])] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[2] > witness[2] + EPSILON);
        }
    }
}

/// The plume grows from the nozzle exit plane (y -42): the exit corners stay,
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
        sp([0., -42., 3.]),
        sp([0., -42., -5.]),
        sp([4., -42., -1.]),
        sp([-3., -42., -1.]),
    ] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_anchor_missing |= qs.is_empty();
        for q in qs {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(witness, q) * scale);
        }
    }
    for witness in [
        sp([0., -81., 3.]),
        sp([0., -81., -5.]),
        sp([4., -81., -1.]),
        sp([-3., -81., -1.]),
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
            // the tailplanes are pitch only, the ailerons roll only, the
            // rudder the pedals only, and the flap lever moves none of them.
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
                    // Pitch only for the tailplanes, roll only for the
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
                    "Mirage F1 pitch{pitch}/roll{roll}: whole-panel hinge, rigidity or direction witness"
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
        ("tailplane-pitch-up", 0),
        ("tailplane-pitch-down", 1),
        ("aileron-roll-right", 2),
        ("aileron-roll-left", 3),
        ("rudder-right", 4),
        ("rudder-left", 5),
    ] {
        let mut state = neutral.clone();
        match edit {
            0 => state.elevator = 1.,
            1 => state.elevator = -1.,
            2 => state.aileron = 1.,
            3 => state.aileron = -1.,
            4 => state.rudder = 1.,
            _ => state.rudder = -1.,
        }
        let pose = airframe.animation_faces(&state);
        for panel in PANELS.iter().filter(|p| {
            matches!(
                (edit, p.control),
                (0 | 1, Control::Elevator) | (2 | 3, Control::Aileron) | (4 | 5, Control::Rudder)
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
/// rigid, hinged, grounded at 1 and tucked above the hinge height near 0.
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
                "Mirage F1 dense gear value{value}: rigid, hinge, direction or stow gate"
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
/// the ridge line, within a hundredth of a foot, and every pose is rigid.
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
    let hinge = sp([0., 29., 5.]);
    let spine = {
        let (a, b) = (sp([0., 37., 6.]), sp([0., 7., 5.]));
        [b[1] - a[1], b[2] - a[2]]
    };
    let mut csv = String::from("value,rigidity_ft,hinge_gap_ft,free_edge_off_spine_ft,passed\n");
    for value in [1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.brake = value;
        let pose = airframe.animation_faces(&state);
        let rigid = rigidity(&reference, &pose, &BRAKE, scale);
        let hinge_gap = pinned(&reference, &pose, &BRAKE, sp([3., 29., 5.]), scale);
        // The first vertex of the first winding is a free-edge corner (2,19,14).
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
                "Mirage F1 brake value{value}: rigid, hinge or spine gate"
            ));
        }
    }
    fs::write(out.join("brake-sweep.csv"), csv)?;
    Ok(failures)
}
