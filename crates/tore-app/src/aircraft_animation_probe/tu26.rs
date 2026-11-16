//! Independent Tu-26 whole-panel witnesses (T26-3): the two swing panels (the
//! pivot vertex stays, the panel is rigid, the tip goes aft), the two flap
//! strips, the tailplane quads, the rudder strip, the gear (rigid legs that
//! stow inside the fuselage section), the fitted bay (T26-5: doors cut from the
//! belly, a lined recess) and the plumes.
//! Measures the shared Airframe output and never repeats the rig's algorithm:
//! the witnesses are named source vertices and rigid-body distances. Points
//! are shape units at 4/3 ft a unit (the probe's `scale`), the identity
//! per-axis scale.
use super::*;

const WING_R: [usize; 13] = [
    0x4b4c, 0x4b73, 0x498f, 0x49b2, 0x49db, 0x49fe, 0x4a21, 0x4a44, 0x4a67, 0x4a8a, 0x4ab3, 0x4ad6,
    0x4af9,
];
const WING_L: [usize; 13] = [
    0x4f8d, 0x4fb4, 0x4dd0, 0x4df9, 0x4e1c, 0x4e3f, 0x4e62, 0x4e85, 0x4ea8, 0x4ecb, 0x4ef4, 0x4f17,
    0x4f40,
];
const STRIP_R: [usize; 2] = [0x4b4c, 0x4b73];
const STRIP_L: [usize; 2] = [0x4f8d, 0x4fb4];
const TAIL_R: [usize; 5] = [0x4085, 0x40bd, 0x40eb, 0x4118, 0x413c];
const TAIL_L: [usize; 6] = [0x45a7, 0x45dd, 0x4605, 0x462c, 0x464d, 0x4670];
const RUDDER: [usize; 2] = [0x517d, 0x51a4];
const MAIN_R: [usize; 4] = [0x5210, 0x5237, 0x525e, 0x527d];
const MAIN_L: [usize; 4] = [0x52e1, 0x5308, 0x532f, 0x534e];
const NOSE: [usize; 4] = [0x53b2, 0x53d1, 0x53f0, 0x540f];
/// The 17 belly faces the fitted bay is cut from and the first address of its
/// lining, which is five faces (a ceiling and four walls).
const BELLY: [usize; 17] = [
    0x261f, 0x2664, 0x2687, 0x270a, 0x272d, 0x276b, 0x2ace, 0x2b15, 0x2b70, 0x2ec6, 0x2ee9, 0x2f57,
    0x2fb8, 0x2fdb, 0x3019, 0x3477, 0x352c,
];
const LINING_BASE: usize = 0x7000;
const LINING: usize = 5;
const FLAME: [usize; 8] = [
    0x5495, 0x54bc, 0x54e3, 0x550a, 0x5531, 0x5558, 0x557f, 0x55a6,
];

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

fn rigid(m: &mut Metrics, reference: &[Face], pose: &[Face], ids: &[usize], scale: f32) {
    let error = rigidity(reference, pose, ids, scale).min(1e9);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error);
}

/// Where the named source vertex went (the first face that has it).
fn witness(reference: &[Face], pose: &[Face], ids: &[usize], point: [f32; 3]) -> Option<[f32; 3]> {
    let old = group(reference, ids);
    let new = group(pose, ids);
    f4::witness_positions(&old, &new, point).into_iter().next()
}

/// A vertex that must not move (feet gap into the anchor metric).
fn pin(m: &mut Metrics, moved: Option<[f32; 3]>, point: [f32; 3], scale: f32) {
    match moved {
        Some(q) => {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(point, q) * scale)
        }
        None => m.reviewed_anchor_missing = true,
    }
}

fn want(m: &mut Metrics, ok: bool) {
    m.reviewed_direction_failures += usize::from(!ok);
}

fn mirror(p: [f32; 3]) -> [f32; 3] {
    [-p[0], p[1], p[2]]
}

/// One named vertex of a face (by the shape's own vertex order): the wing and
/// strip witnesses cannot be found by source coordinates because the drawn
/// spread pose is already turned 1.6 degrees from the mesh's.
fn vertex(faces: &[Face], address: usize, index: usize) -> Option<[f32; 3]> {
    faces
        .iter()
        .find(|f| f.address == address)
        .and_then(|f| f.positions.get(index).copied())
}

/// Wing: rigid panel and rigid strip, the glove-corner pivot stays, the tip
/// goes aft and inboard (sweep samples above the 300 knot start), the two
/// tips mirror each other.
fn wing(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for (ids, strip) in [(&WING_R, &STRIP_R), (&WING_L, &STRIP_L)] {
        let body: Vec<usize> = ids.iter().copied().filter(|a| !strip.contains(a)).collect();
        rigid(m, reference, pose, &body, scale);
        rigid(m, reference, pose, strip, scale);
    }
    // The pivot vertices: 4a21 (right) and 4e1c (left), vertex 0.
    for address in [0x4a21, 0x4e1c] {
        match (vertex(reference, address, 0), vertex(pose, address, 0)) {
            (Some(a), Some(b)) => pin(m, Some(b), a, scale),
            _ => m.reviewed_anchor_missing = true,
        }
    }
    // The tips: 4a44 vertex 1 (right, 38, -5), 4df9 vertex 2 (left).
    let (Some(rr), Some(rp), Some(lr), Some(lp)) = (
        vertex(reference, 0x4a44, 1),
        vertex(pose, 0x4a44, 1),
        vertex(reference, 0x4df9, 2),
        vertex(pose, 0x4df9, 2),
    ) else {
        m.reviewed_anchor_missing = true;
        return;
    };
    want(
        m,
        (rp[0] + lp[0]).abs() < 1e-3 && (rp[1] - lp[1]).abs() < 1e-3,
    );
    want(m, (rr[0] + lr[0]).abs() < 1e-3);
    if value > 0. {
        want(m, rp[1] < rr[1] - 0.5 && rp[0] < rr[0] - 0.5);
    }
}

/// Strips droop about their leading edge: the hinge ends stay, the trailing
/// tip corner goes down by its distance from the hinge times sin(travel).
/// Right strip face 4b73 (trailing tip 0, hinge tip 1, hinge root 2), left
/// 4f8d (hinge root 0, trailing tip 2, hinge tip 3).
fn flaps(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    for (address, hinges, trailing) in [(0x4b73, [1, 2], 0), (0x4f8d, [0, 3], 2)] {
        for index in hinges {
            match (
                vertex(reference, address, index),
                vertex(pose, address, index),
            ) {
                (Some(a), Some(b)) => pin(m, Some(b), a, scale),
                _ => m.reviewed_anchor_missing = true,
            }
        }
        match (
            vertex(reference, address, trailing),
            vertex(pose, address, trailing),
        ) {
            (Some(a), Some(b)) => {
                let want_dz = -1.976 * (0.40 * value).sin() as f32;
                want(m, ((b[2] - a[2]) - want_dz).abs() < 0.05);
            }
            _ => m.reviewed_anchor_missing = true,
        }
    }
}

/// Tailplane quads: rigid, the aft tip rises for a pull and, for a positive
/// roll command, on the right while it falls on the left.
fn tailplane(
    control: Control,
    value: f64,
    reference: &[Face],
    pose: &[Face],
    scale: f32,
    m: &mut Metrics,
) {
    rigid(m, reference, pose, &TAIL_R, scale);
    rigid(m, reference, pose, &TAIL_L, scale);
    let tip_r = [13., -36., 0.];
    let rise = |ids: &[usize], tip| witness(reference, pose, ids, tip).map(|q| q[2] - tip[2]);
    let (Some(r), Some(l)) = (rise(&TAIL_R, tip_r), rise(&TAIL_L, mirror(tip_r))) else {
        m.reviewed_anchor_missing = true;
        return;
    };
    let v = value as f32;
    match control {
        Control::Elevator => {
            let want_rise = 14.5 * (0.14 * value).sin() as f32;
            want(
                m,
                (r - want_rise).abs() < 0.05 && (l - want_rise).abs() < 0.05,
            );
        }
        Control::Aileron => {
            let want_rise = 14.5 * (0.08 * value).sin() as f32;
            want(
                m,
                (r - want_rise).abs() < 0.05 && (l + want_rise).abs() < 0.05,
            );
        }
        _ => {}
    }
    let _ = v;
}

/// Rudder: rigid strip, the leading edge stays, the aft base vertex goes to the
/// side of the command.
fn rudder(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    rigid(m, reference, pose, &RUDDER, scale);
    for hinge in [[0., -24., 9.], [0., -31., 21.]] {
        pin(m, witness(reference, pose, &RUDDER, hinge), hinge, scale);
    }
    let aft = [0., -28., 9.];
    match witness(reference, pose, &RUDDER, aft) {
        Some(q) => {
            let x = q[0];
            want(m, value == 0. || x * value as f32 > 0.2);
            want(m, x.abs() < 3.5 * (0.5 * value.abs()).sin() as f32 + 0.05);
        }
        None => m.reviewed_anchor_missing = true,
    }
}

/// The fuselage cross-section half-widths a stowed leg must stay inside.
fn inside_main(p: [f32; 3]) -> bool {
    p[0].abs() <= 5.2 && p[2] >= -3.2 && p[2] <= 4.
}
fn inside_nose(p: [f32; 3]) -> bool {
    p[0].abs() <= 2.05 && p[2] >= -4.1 && p[2] <= 4.
}

/// Gear: each leg rigid; at the source pose (value 1) the pose is the reference;
/// near stow every leg vertex is inside the fuselage section.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        // Hidden: nothing of the gear is drawn.
        want(m, !pose.iter().any(|f| GEAR_ALL.contains(&f.address)));
        return;
    }
    for ids in [&MAIN_R[..], &MAIN_L[..], &NOSE[..]] {
        rigid(m, reference, pose, ids, scale);
    }
    if value == 1. {
        let (old, new) = (group(reference, &GEAR_ALL), group(pose, &GEAR_ALL));
        for (key, face) in &old {
            if new.get(key).is_none_or(|n| n.positions != face.positions) {
                m.reviewed_neutral_mismatch = true;
            }
        }
    }
    if value > 0. && value <= 0.05 {
        for face in pose
            .iter()
            .filter(|f| MAIN_R.contains(&f.address) || MAIN_L.contains(&f.address))
        {
            m.reviewed_wheel_failed |= !face.positions.iter().all(|p| inside_main(*p));
        }
        for face in pose.iter().filter(|f| NOSE.contains(&f.address)) {
            m.reviewed_wheel_failed |= !face.positions.iter().all(|p| inside_nose(*p));
        }
    }
}
const GEAR_ALL: [usize; 12] = [
    0x5210, 0x5237, 0x525e, 0x527d, 0x52e1, 0x5308, 0x532f, 0x534e, 0x53b2, 0x53d1, 0x53f0, 0x540f,
];

/// The fitted bay: shut (0) every belly face is whole, nothing hangs below the
/// keel line (z -4) and no lining draws; open (above 0) the lining draws, a
/// vertex of the cut belly stays on each hinge line (x +-2.2125 units at the
/// belly plane z -3) and the lowest point of the two leaves, the keel's
/// centre edge (2.2125 units inboard of the hinge and one unit under it), is
/// `-3 - 2.2125 sin(a) - cos(a)` for the door angle `a` of 90 degrees times
/// the value.
fn bay(value: f64, _reference: &[Face], pose: &[Face], _scale: f32, m: &mut Metrics) {
    let o = tore_sim::combat::bays::opening(AircraftId::Tu26).expect("Tu-26 bay");
    let half = (o.outer / (4. / 3.)) as f32;
    let lining = pose.iter().filter(|f| f.address >= LINING_BASE).count();
    let skin: Vec<&Face> = pose.iter().filter(|f| BELLY.contains(&f.address)).collect();
    let lowest = skin
        .iter()
        .flat_map(|f| f.positions.iter())
        .map(|p| p[2])
        .fold(f32::MAX, f32::min);
    if value == 0. {
        want(m, lining == 0 && skin.len() == BELLY.len() && lowest == -4.);
        return;
    }
    want(m, lining == LINING);
    for x in [half, -half] {
        want(
            m,
            skin.iter()
                .flat_map(|f| f.positions.iter())
                .any(|p| (p[0] - x).abs() < 1e-3 && (p[2] + 3.).abs() < 1e-3),
        );
    }
    let a = value as f32 * std::f32::consts::FRAC_PI_2;
    want(m, (lowest - (-3. - half * a.sin() - a.cos())).abs() < 2e-3);
}

/// Plumes: scaled from the nozzle plane y -29 to the source's -53.
fn exhaust(value: f64, pose: &[Face], m: &mut Metrics) {
    let tip = pose
        .iter()
        .filter(|f| FLAME.contains(&f.address))
        .flat_map(|f| f.positions.iter().map(|p| p[1]))
        .fold(f32::INFINITY, f32::min);
    if value == 0. {
        want(m, tip.is_infinite());
    } else {
        want(m, (tip - (-29. - 24. * value as f32)).abs() < 1e-3);
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
        Control::Elevator | Control::Aileron => tailplane(c, value, reference, pose, scale, m),
        Control::Rudder => rudder(value, reference, pose, scale, m),
        Control::Flaps => flaps(value, reference, pose, scale, m),
        Control::Sweep => wing(value, reference, pose, scale, m),
        Control::Gear => gear(value, reference, pose, scale, m),
        Control::Bay => bay(value, reference, pose, scale, m),
        Control::Exhaust => exhaust(value, pose, m),
        _ => {}
    }
}

/// The sweep against the flaps and the gear: either holds the wing at the
/// spread pose whatever the speed (the whole wing, strips included, is then the
/// zero-speed pose with the strips drooped by the flaps); and the wing
/// sweeps, rigidly, with the flap strip drooped only when the flaps are in.
pub(super) fn combinations(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut rows = String::from("knots,flaps,gear,tip_x_ft,tip_y_ft,wing_rigid_ft\n");
    let tip = [38., -5., -1.];
    let mut spread_tip = None;
    for (knots, flaps, gear) in [
        (200., 0., 0.),
        (450., 0., 0.),
        (700., 0., 0.),
        (700., 1., 0.),
        (700., 0., 1.),
        (700., 0.5, 0.5),
    ] {
        let mut state = neutral.clone();
        state.speed = knots * 1.68781;
        state.flaps = flaps;
        state.gear = gear;
        let pose = airframe.animation_faces(&state);
        let body: Vec<usize> = WING_R
            .iter()
            .copied()
            .filter(|a| !STRIP_R.contains(a))
            .collect();
        let error = rigidity(&original, &pose, &body, scale);
        let Some(q) = vertex(&pose, 0x4a44, 1) else {
            failures.push("Tu-26 sweep grid: the wing tip is missing".to_string());
            continue;
        };
        writeln!(
            rows,
            "{knots},{flaps},{gear},{},{},{error}",
            q[0] * scale,
            q[1] * scale
        )?;
        if error > EPSILON {
            failures.push(format!(
                "Tu-26 {knots} kt flaps {flaps} gear {gear}: wing not rigid"
            ));
        }
        if flaps >= 1. || gear >= 1. {
            // Held spread: the same tip as the zero-speed pose.
            let rest = vertex(&original, 0x4a44, 1).unwrap_or(tip);
            let held = distance(q, rest) * scale;
            if held > EPSILON {
                failures.push(format!(
                    "Tu-26 flaps {flaps} gear {gear} at {knots} kt did not hold the wing spread"
                ));
            }
        }
        if knots == 200. {
            spread_tip = Some(q);
        } else if flaps == 0. && gear == 0. && spread_tip.is_some_and(|s| q[1] >= s[1]) {
            failures.push(format!("Tu-26 {knots} kt: the tip did not go aft"));
        }
    }
    fs::write(out.join("sweep-grid.csv"), rows)?;
    failures.extend(pitch_roll(airframe, neutral, out)?);
    failures.extend(dense_gear(airframe, neutral, out)?);
    Ok(failures)
}

/// Pitch against roll: the two tailplanes are independent whole panels, so
/// every pose is rigid and each tip rises by 14.5 units times the sine of its
/// own clamped angle (pitch 0.14, roll 0.08 either way, held to 0.20 rad).
pub(super) fn pitch_roll(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut rows = String::from("pitch,roll,finite,rigidity_ft,right_rise,left_rise,passed\n");
    let mut poses = Vec::new();
    for pitch in [-1., -0.5, 0., 0.5, 1.] {
        for roll in [-1., -0.5, 0., 0.5, 1.] {
            let mut state = neutral.clone();
            state.elevator = pitch;
            state.aileron = roll;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(&original, &pose, scale);
            rigid(&mut m, &original, &pose, &TAIL_R, scale);
            rigid(&mut m, &original, &pose, &TAIL_L, scale);
            let tip = [13., -36., 0.];
            let rise =
                |ids: &[usize], tip| witness(&original, &pose, ids, tip).map(|q| q[2] - tip[2]);
            let (r, l) = (
                rise(&TAIL_R, tip).unwrap_or(f32::NAN),
                rise(&TAIL_L, mirror(tip)).unwrap_or(f32::NAN),
            );
            let angle = |side: f64| (-(0.14 * pitch) + side * 0.08 * roll).clamp(-0.2, 0.2);
            // A positive angle lowers the edge; the right panel's roll term
            // raises it for a positive aileron.
            let (want_r, want_l) = (
                -14.5 * angle(-1.).sin() as f32,
                -14.5 * angle(1.).sin() as f32,
            );
            let passed = m.finite
                && m.reviewed_rigid_panel_error <= EPSILON
                && (r - want_r).abs() < 0.06
                && (l - want_l).abs() < 0.06
                && m.new_planar_crossings.is_empty();
            writeln!(
                rows,
                "{pitch},{roll},{},{},{r},{l},{passed}",
                m.finite, m.reviewed_rigid_panel_error
            )?;
            if !passed {
                failures.push(format!("Tu-26 pitch{pitch}/roll{roll}: tailplane witness"));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("pitch-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("pitch-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid per leg, the source pose at 1 and every leg inside the fuselage near
/// stow.
pub(super) fn dense_gear(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
) -> AppResult<Vec<String>> {
    let original = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut gear_endpoint = neutral.clone();
    Control::Gear.apply(&mut gear_endpoint, 1.);
    let reference = airframe.animation_faces(&gear_endpoint);
    let mut csv = String::from("value,rigidity_ft,crossings,checks_passed\n");
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
            && m.reviewed_rigid_panel_error <= EPSILON
            && !m.reviewed_wheel_failed
            && !m.reviewed_neutral_mismatch
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{passed}",
            m.reviewed_rigid_panel_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "Tu-26 dense gear value{value}: rigid, stow or source pose"
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
