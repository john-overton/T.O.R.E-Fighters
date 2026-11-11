//! Independent Tornado whole-panel witnesses (TOR-3): the swing wing (13 faces a
//! side turned as one rigid panel about the glove pivot), the two tailerons, the
//! trailing flap strips and the rudder block. Measures the shared Airframe
//! output and never repeats the rig's algorithm: the witnesses are named source
//! vertices, rigid-body distances and the glove seam. TOR-4 owns the gear, the
//! airbrake plates and the burner plates, which this module leaves alone.
use super::*;
use crate::tornado_animation::{
    PIVOT_L, PIVOT_R, STRIP_L, STRIP_R, TAIL_L, TAIL_R, WING_L, WING_R,
};

const RUDDER: [usize; 2] = [0x5723, 0x5750];
/// Shape units to feet (the probe scale, header scale 2).
const FEET: f32 = 2. / 3.;

fn mirror(p: [f32; 3]) -> [f32; 3] {
    [-p[0], p[1], p[2]]
}

/// One side of the aircraft: its wing, strip and taileron faces.
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
    /// A right-hand source point on this side.
    fn fix(&self, p: [f32; 3]) -> [f32; 3] {
        if self.right { p } else { mirror(p) }
    }
    fn sign(&self) -> f32 {
        if self.right { 1. } else { -1. }
    }
    /// The glove vertices shared with the fixed fillets (x, y, z), as the shape
    /// has them: the left glove is one unit further out.
    fn glove(&self) -> [[f32; 3]; 5] {
        let x = |a: f32, b: f32| if self.right { a } else { b };
        [
            [x(9., -10.), 1., 0.],
            [x(9., -10.), 1., 1.],
            [x(10., -11.), -1., 0.],
            [x(10., -11.), -1., 1.],
            [x(10., -11.), -4., 0.],
        ]
    }
    /// The wing tip vertex, the witness of the sweep direction.
    fn tip(&self) -> [f32; 3] {
        [if self.right { 33. } else { -34. }, -10., 0.]
    }
    fn strip_tip(&self) -> [f32; 3] {
        [if self.right { 30. } else { -31. }, -7., 0.]
    }
    fn strip_trailing(&self) -> [f32; 3] {
        [if self.right { 30. } else { -31. }, -9., 0.]
    }
    fn strip_root(&self) -> ([f32; 3], [f32; 3]) {
        let x = if self.right { 10. } else { -11. };
        ([x, -1., 0.], [x, -1., 1.])
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

/// Skin gap inside one group: shared vertices of its faces must move together.
fn group_skin_gap(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let old = group(reference, ids);
    let new = group(pose, ids);
    if let Some(g) = shared_vertex_gaps(&old, &new, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(g.gap);
    }
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

/// The glove seam of one wing: how far each wing vertex shared with a fixed
/// fillet moved (feet), and the outboard (opening) and inboard (overlap) parts.
#[derive(Clone, Copy, Default)]
struct GloveSeam {
    separation: f32,
    opening: f32,
    overlap: f32,
}
fn glove_seam(reference: &[Face], pose: &[Face], side: &Side, scale: f32) -> GloveSeam {
    let before = keyed(reference);
    let after = keyed(pose);
    let mut seam = GloveSeam::default();
    for (key, face) in before.iter().filter(|(k, _)| side.wing.contains(&k.0)) {
        let Some(moved) = after.get(key) else {
            continue;
        };
        for (a, b) in face.positions.iter().zip(&moved.positions) {
            if side.glove().contains(a) {
                seam.separation = seam.separation.max(distance(*a, *b) * scale);
                let outboard = (b[0] - a[0]) * side.sign() * scale;
                seam.opening = seam.opening.max(outboard);
                seam.overlap = seam.overlap.max(-outboard);
            }
        }
    }
    seam
}

/// The taileron checks shared by the control sweep and the grid. `net` is the
/// sign of the commanded trailing-edge motion (up positive) for this side.
fn tail(side: &Side, net: f32, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.tail);
    let new = group(pose, side.tail);
    group_skin_gap(reference, pose, side.tail, scale, m);
    let error = rigidity(reference, pose, side.tail, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The spindle runs through the midpoint of the root chord (x 6, y -25 to -20).
    let pair = (side.fix([6., -25., -2.]), side.fix([6., -20., -2.]));
    match pair_gap(&old, &new, pair, scale) {
        Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
        None => m.reviewed_anchor_missing = true,
    }
    if net == 0. {
        for (key, face) in &old {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.tail, &[], scale);
    // The trailing tip moves up for a positive command.
    for witness in [side.fix([18., -27., -2.]), side.fix([18., -23., -2.])] {
        let qs = f4::witness_positions(&old, &new, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            let delta = (q[2] - witness[2]) * scale;
            m.reviewed_direction_failures += usize::from(delta * net.signum() <= EPSILON);
        }
    }
}

/// The trailing strips: rigid, hinged on the leading edge, drooping for flaps.
fn strips(side: &Side, value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, side.strip);
    let new = group(pose, side.strip);
    group_skin_gap(reference, pose, side.strip, scale, m);
    let error = rigidity(reference, pose, side.strip, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The outer hinge corner stays; the root pair's midpoint stays on the axis.
    let gap = pinned(reference, pose, side.strip, side.strip_tip(), scale);
    m.reviewed_anchor_missing |= !gap.is_finite();
    if gap.is_finite() {
        m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
    }
    match pair_gap(&old, &new, side.strip_root(), scale) {
        Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
        None => m.reviewed_anchor_missing = true,
    }
    if value == 0. {
        for (key, face) in &old {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.strip, &[side.strip_tip()], scale);
    let witness = side.strip_trailing();
    let qs = f4::witness_positions(&old, &new, witness);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        m.reviewed_direction_failures += usize::from((q[2] - witness[2]) * scale >= -EPSILON);
    }
}

fn rudder(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let old = group(reference, &RUDDER);
    let new = group(pose, &RUDDER);
    group_skin_gap(reference, pose, &RUDDER, scale, m);
    let error = rigidity(reference, pose, &RUDDER, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    // The two vertices on the leading edge stay.
    let hinge = [[0., -21., 4.], [0., -26., 12.]];
    for point in hinge {
        let gap = pinned(reference, pose, &RUDDER, point, scale);
        m.reviewed_anchor_missing |= !gap.is_finite();
        if gap.is_finite() {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
        }
    }
    if value == 0. {
        for (key, face) in &old {
            match new.get(key) {
                Some(after) if face.positions == after.positions => {}
                _ => m.reviewed_neutral_mismatch = true,
            }
        }
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, &RUDDER, &hinge, scale);
    // Trailing vertices swing to +X for a positive command.
    for witness in [[0., -25., 3.], [0., -31., 12.]] {
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
    let old = group(reference, side.wing);
    let new = group(pose, side.wing);
    group_skin_gap(reference, pose, side.wing, scale, m);
    let error = rigidity(reference, pose, side.wing, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    let moved = old
        .iter()
        .any(|(k, f)| new.get(k).is_some_and(|n| n.positions != f.positions));
    if !moved {
        return;
    }
    m.reviewed_direction_failures += folds(reference, pose, side.wing, &[], scale);
    let tip = side.tip();
    let qs = f4::witness_positions(&old, &new, tip);
    m.reviewed_direction_failures += usize::from(qs.is_empty());
    for q in qs {
        // Aft, and in toward the fuselage.
        m.reviewed_direction_failures += usize::from(q[1] >= tip[1] - EPSILON / scale);
        m.reviewed_direction_failures += usize::from(q[0].abs() >= tip[0].abs() - EPSILON / scale);
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
/// witness that measures the sweep without repeating the schedule.
fn tip_turn(reference: &[Face], pose: &[Face], side: &Side) -> Option<f64> {
    let old = group(reference, side.wing);
    let new = group(pose, side.wing);
    let tip = side.tip();
    let q = *f4::witness_positions(&old, &new, tip).first()?;
    let pivot = if side.right { PIVOT_R } else { PIVOT_L };
    let a = f64::from(tip[1] - pivot[1]).atan2(f64::from(tip[0] - pivot[0]));
    let b = f64::from(q[1] - pivot[1]).atan2(f64::from(q[0] - pivot[0]));
    // Wrap the difference, then count a clockwise turn (the right wing's sweep)
    // and a counter-clockwise one (the left's) as positive.
    let d = (b - a + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI;
    Some(if side.right { -d } else { d })
}

pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    // The 25-pose pitch and roll grid at the two ends of the sweep.
    for (knots, name) in [(200., "25deg"), (650., "67deg")] {
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
                    // The law's weights are 0.20 for pitch and 0.08 for roll.
                    let net = (5. * pitch + 2. * f64::from(side.sign()) * roll) as f32;
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
                        "Tornado {name} pitch{pitch}/roll{roll}: whole-panel spindle, rigidity, fold or direction witness"
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
        "knots,flaps,expected_turn_deg,measured_turn_deg_right,measured_turn_deg_left,rigidity_ft,skin_gap_ft,seam_separation_ft,seam_opening_ft,seam_overlap_ft,passed\n",
    );
    let mut poses = Vec::new();
    for knots in [200., 250., 300., 400., 500., 600., 700.] {
        for flaps in [0., 0.25, 0.5, 0.75, 1.] {
            let mut state = at_knots(neutral, knots);
            state.flaps = flaps;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            let expected = ((knots - 250.) / 350.).clamp(0., 1.) * 42. * (1. - flaps);
            let mut measured = [0f64; 2];
            let mut seam = GloveSeam::default();
            let mut rigid = 0f32;
            for (k, side) in SIDES.iter().enumerate() {
                // Without the strips: they droop independently with the flaps.
                let panel: Vec<usize> = side
                    .wing
                    .iter()
                    .copied()
                    .filter(|a| !side.strip.contains(a))
                    .collect();
                rigid = rigid.max(rigidity(&original, &pose, &panel, scale));
                group_skin_gap(&original, &pose, &panel, scale, &mut m);
                measured[k] = tip_turn(&original, &pose, side)
                    .unwrap_or(f64::NAN)
                    .to_degrees();
                let s = glove_seam(&original, &pose, side, scale);
                seam.separation = seam.separation.max(s.separation);
                seam.opening = seam.opening.max(s.opening);
                seam.overlap = seam.overlap.max(s.overlap);
            }
            let ok = m.finite
                && rigid <= EPSILON
                && m.max_reviewed_skin_gap <= EPSILON
                && m.new_planar_crossings.is_empty()
                && measured.iter().all(|t| (t - expected).abs() < 0.01);
            writeln!(
                rows,
                "{knots},{flaps},{expected:.3},{:.3},{:.3},{rigid},{},{},{},{},{ok}",
                measured[0],
                measured[1],
                m.max_reviewed_skin_gap,
                seam.separation,
                seam.opening,
                seam.overlap
            )?;
            if !ok {
                failures.push(format!(
                    "Tornado sweep {knots} kt/flaps {flaps}: expected {expected:.2} deg, measured {:.2} / {:.2}, rigidity {rigid}",
                    measured[0], measured[1]
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("sweep-flap-combinations.csv"), rows)?;
    contact_sheet(&out.join("sweep-flap-combinations.ppm"), &original, &poses)?;

    // Glove seam at each sweep angle: how far the shared wing vertices pull away
    // from the fixed fillets (separation), and the outboard (visible opening)
    // and inboard (overlap into the glove) parts, per side, in feet.
    let mut rows = String::from(
        "sweep_deg,turn_deg,side,seam_separation_ft,seam_opening_ft,seam_overlap_ft,bound_ft\n",
    );
    for degrees in [25., 30., 35., 40., 45., 50., 55., 60., 65., 67.] {
        // Pick the speed that gives this sweep angle in the documented schedule.
        let knots = 250. + (degrees - 25.) / 42. * 350.;
        let pose = airframe.animation_faces(&at_knots(neutral, knots));
        let turn = degrees - 25.;
        for side in &SIDES {
            let seam = glove_seam(&original, &pose, side, scale);
            // The enclosing circle's radius is 2.55 units about the pivot.
            let bound = 2. * 2.5495 * (turn.to_radians() / 2.).sin() as f32 * FEET;
            writeln!(
                rows,
                "{degrees},{turn},{},{},{},{},{bound}",
                if side.right { "right" } else { "left" },
                seam.separation,
                seam.opening,
                seam.overlap
            )?;
            if seam.separation > bound + 1e-3 {
                failures.push(format!(
                    "Tornado glove seam at {degrees} deg exceeds its enclosing-circle bound"
                ));
            }
        }
    }
    fs::write(out.join("sweep-seam.csv"), rows)?;

    // Taileron forward corner at the limit, and the seam openings of the strip
    // and rudder corners at full travel.
    let mut rows = String::from("surface,command,corner,swing_ft\n");
    let corner = |ids: &[usize], pose: &[Face], point: [f32; 3]| -> f32 {
        let old = group(&original, ids);
        f4::witness_positions(&old, &keyed(pose), point)
            .first()
            .map_or(f32::NAN, |q| distance(point, *q) * scale)
    };
    for (name, elevator, aileron) in [
        ("pitch-up", 1., 0.),
        ("pitch-down", -1., 0.),
        ("roll-right", 0., 1.),
        ("pitch-up+roll-right", 1., 1.),
        ("pitch-down+roll-right", -1., 1.),
    ] {
        let mut state = at_knots(neutral, 200.);
        state.elevator = elevator;
        state.aileron = aileron;
        let pose = airframe.animation_faces(&state);
        for side in &SIDES {
            for (label, point) in [
                ("forward corner (7,-8,-2)", [7., -8., -2.]),
                ("tip leading (18,-23,-2)", [18., -23., -2.]),
                ("tip trailing (18,-27,-2)", [18., -27., -2.]),
            ] {
                writeln!(
                    rows,
                    "taileron-{},{name},{label},{}",
                    if side.right { "right" } else { "left" },
                    corner(side.tail, &pose, side.fix(point))
                )?;
            }
        }
    }
    fs::write(out.join("taileron-corner-swing.csv"), rows)?;

    let mut rows = String::from("surface,command,seam_opening_ft\n");
    for (name, edit) in [
        ("flap-strip-right", 0),
        ("flap-strip-left", 1),
        ("rudder-right", 2),
        ("rudder-left", 3),
    ] {
        let mut state = at_knots(neutral, 200.);
        match edit {
            0 | 1 => state.flaps = 1.,
            2 => state.rudder = 1.,
            _ => state.rudder = -1.,
        }
        let pose = airframe.animation_faces(&state);
        let ids: &[usize] = match edit {
            0 => &STRIP_R,
            1 => &STRIP_L,
            _ => &RUDDER,
        };
        writeln!(
            rows,
            "{name},full,{}",
            seam_opening(&original, &pose, ids, scale)
        )?;
    }
    fs::write(out.join("seam-opening.csv"), rows)?;

    failures.extend(pylon_poses(airframe, neutral, out, &original, scale)?);
    Ok(failures)
}

/// Largest distance (feet) any group vertex that was shared with a fixed face
/// in the reference moved: the opening of the moving/fixed seam. Reported, not
/// a failure (a hinge vertex a little off its axis must move).
fn seam_opening(reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) -> f32 {
    let before = keyed(reference);
    let after = keyed(pose);
    let fixed: std::collections::BTreeSet<[i64; 3]> = before
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

/// The wing-station interface for TOR-5 (`tornado_animation::wing_station`):
/// a mount given on a wing vertex lands exactly on that vertex of the swept
/// panel, the pylon swivels back by the panel's turn and the store keeps its
/// heading. Writes `pylon-poses.csv`.
fn pylon_poses(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    use crate::tornado_animation::{sweep_turn, wing_station};
    let mut failures = Vec::new();
    let mut rows = String::from(
        "knots,side,sweep_deg,turn_deg,swivel_deg,heading_deg,mount_right_ft,mount_up_ft,mount_fwd_ft,vertex_gap_ft\n",
    );
    for knots in [200., 300., 400., 500., 600., 700.] {
        let state = at_knots(neutral, knots);
        let pose = airframe.animation_faces(&state);
        for side in &SIDES {
            let tip = side.tip();
            let old = group(original, side.wing);
            let new = group(&pose, side.wing);
            let Some(q) = f4::witness_positions(&old, &new, tip).first().copied() else {
                failures.push("Tornado pylon witness: tip vertex missing".into());
                continue;
            };
            // The mount named on the tip vertex, in feet right, up and forward.
            let mount = [tip[0], tip[2], tip[1]].map(|v| f64::from(v * FEET));
            let station = wing_station(side.right, mount, sweep_turn(&state));
            let gap = (station.mount[0] - f64::from(q[0] * scale))
                .hypot(station.mount[2] - f64::from(q[1] * scale));
            writeln!(
                rows,
                "{knots},{},{:.3},{:.3},{:.3},{:.3},{:.4},{:.4},{:.4},{gap}",
                if side.right { "right" } else { "left" },
                crate::tornado_animation::sweep_degrees(station.turn),
                station.turn.to_degrees(),
                station.swivel.to_degrees(),
                station.heading.to_degrees(),
                station.mount[0],
                station.mount[1],
                station.mount[2],
            )?;
            if gap > f64::from(EPSILON)
                || station.heading != 0.
                || (station.turn + station.swivel * if side.right { 1. } else { -1. }).abs() > 1e-12
            {
                failures.push(format!(
                    "Tornado wing station off the swept wing ({} side, {knots} kt): gap {gap} ft",
                    if side.right { "right" } else { "left" }
                ));
            }
        }
    }
    fs::write(out.join("pylon-poses.csv"), rows)?;
    Ok(failures)
}
