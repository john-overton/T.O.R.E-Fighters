//! Independent Su-34 whole-panel witnesses (SU34-3): the two canard plates
//! (pitch only), the two tailerons (a quad and a root triangle in both windings
//! a side, one rigid panel, pitch plus differential roll), the two outer
//! ailerons (hinged about their leading edge), the two flap morphs (down pose
//! only), the airbrake plate, the eight rigid gear plates and the plume.
//! Measures the shared Airframe output and never repeats the rig's algorithm.
//! Witness points are named in shape units (the Su-34 draws at the identity
//! per-axis scale; `scale` is feet a unit). The fins have no rudder face: the
//! pedals must move nothing. Shared witness helpers come from the F-15E and
//! Su-35 probes.
use super::f15e::{
    extent, follow, group, group_skin_gap, lowest, midpoint_gap, pin, pinned, rigidity,
    seam_opening, surfaces_pass, unchanged, whole_panel, z_direction,
};
use super::su35::virtual_point;
use super::*;

const CANARD_L: [usize; 2] = [0x34e6, 0x3597];
const CANARD_R: [usize; 2] = [0x47cc, 0x480d];
const TAIL_L: [usize; 4] = [0x3666, 0x3727, 0x360b, 0x3646];
const TAIL_R: [usize; 4] = [0x4837, 0x4906, 0x4714, 0x4777];
const AILERON_L: [usize; 2] = [0x36d6, 0x375f];
const AILERON_R: [usize; 2] = [0x4886, 0x4988];
const FLAP_L: [usize; 2] = [0x4cbe, 0x4cd5];
const FLAP_R: [usize; 2] = [0x4c14, 0x4c3b];
const FIN: [usize; 8] = [
    0x37cb, 0x37ee, 0x38b9, 0x38dc, 0x49af, 0x49d6, 0x49fd, 0x4a24,
];
const BRAKE: [usize; 2] = [0x4dc9, 0x4de0];
const FLAME: [usize; 16] = [
    0x4e8e, 0x4ead, 0x4ecc, 0x4eeb, 0x4f0a, 0x4f29, 0x4f48, 0x4f67, 0x4f86, 0x4fa5, 0x4fc4, 0x4fe3,
    0x5002, 0x5021, 0x5040, 0x505f,
];
const GEAR_R: [usize; 2] = [0x4a7b, 0x4a9a];
const GEAR_L: [usize; 2] = [0x4ae6, 0x4b05];
const NOSE: [usize; 4] = [0x4b69, 0x4b88, 0x4ba7, 0x4bce];
const GEAR: [usize; 8] = [
    0x4a7b, 0x4a9a, 0x4ae6, 0x4b05, 0x4b69, 0x4b88, 0x4ba7, 0x4bce,
];
/// Faces a control surface may move; every other face must stay at its
/// neutral position under the surface controls.
const SURFACES: [&[usize]; 8] = [
    &CANARD_L, &CANARD_R, &TAIL_L, &TAIL_R, &AILERON_L, &AILERON_R, &FLAP_L, &FLAP_R,
];
/// The closed brake must lie on the dorsal ridge this closely (feet).
const RIDGE_TOLERANCE: f32 = 0.01;
/// A stowed plate may lie this far (units) outside the skin envelope: the
/// main plates' outer end is 0.5 unit under the wing skin.
const STOW_OVERSHOOT: f32 = 0.6;

/// Every face outside the listed groups is exactly as in the reference.
fn rest_unchanged(reference: &[Face], pose: &[Face], moving: &[&[usize]], m: &mut Metrics) {
    let skip = |a: usize| moving.iter().any(|ids| ids.contains(&a));
    let before: Vec<_> = reference.iter().filter(|f| !skip(f.address)).collect();
    let after: Vec<_> = pose.iter().filter(|f| !skip(f.address)).collect();
    if before.len() != after.len()
        || before
            .iter()
            .zip(&after)
            .any(|(a, b)| a.address != b.address || a.positions != b.positions)
    {
        m.reviewed_neutral_mismatch = true;
    }
}

/// The lowest and highest skin height (units) over (x, y): a vertical line
/// through the fan-triangulated faces. None when it hits nothing.
fn span(faces: &[Face], x: f32, y: f32) -> Option<(f32, f32)> {
    let side = |p: [f32; 3], q: [f32; 3], r: [f32; 3]| {
        (p[0] - r[0]) * (q[1] - r[1]) - (q[0] - r[0]) * (p[1] - r[1])
    };
    let at = [x, y, 0.];
    let mut range: Option<(f32, f32)> = None;
    for f in faces {
        for i in 1..f.positions.len().saturating_sub(1) {
            let (a, b, c) = (f.positions[0], f.positions[i], f.positions[i + 1]);
            let (d1, d2, d3) = (side(at, a, b), side(at, b, c), side(at, c, a));
            if (d1 < 0. || d2 < 0. || d3 < 0.) && (d1 > 0. || d2 > 0. || d3 > 0.) {
                continue;
            }
            let (u, v) = (
                [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
                [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
            );
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            if n[2].abs() < 1e-9 {
                continue;
            }
            let z = a[2] - (n[0] * (x - a[0]) + n[1] * (y - a[1])) / n[2];
            range = Some(range.map_or((z, z), |(lo, hi)| (lo.min(z), hi.max(z))));
        }
    }
    range
}

/// Sample points over each plate of a group: a 5 by 5 grid of its quad.
fn plate_samples(pose: &[Face], ids: &[usize]) -> Vec<[f32; 3]> {
    let mut points = Vec::new();
    for f in pose.iter().filter(|f| ids.contains(&f.address)) {
        if f.positions.len() != 4 {
            continue;
        }
        let p = &f.positions;
        for i in 0..=4 {
            for j in 0..=4 {
                let (s, t) = (i as f32 / 4., j as f32 / 4.);
                points.push(std::array::from_fn(|k| {
                    (1. - s) * ((1. - t) * p[0][k] + t * p[1][k])
                        + s * ((1. - t) * p[3][k] + t * p[2][k])
                }));
            }
        }
    }
    points
}

/// How far (units) the plates' samples lie outside the airframe's skin
/// envelope (the neutral faces without the gear).
fn outside(pose: &[Face], ids: &[usize]) -> f32 {
    let skin: Vec<Face> = pose
        .iter()
        .filter(|f| !GEAR.contains(&f.address))
        .cloned()
        .collect();
    plate_samples(pose, ids)
        .into_iter()
        .map(|p| {
            // A nudge off the centre line keeps the ray off triangle edges.
            span(&skin, p[0] + 0.013, p[1] + 0.017)
                .map_or(99., |(lo, hi)| (lo - p[2]).max(p[2] - hi).max(0.))
        })
        .fold(0., f32::max)
}

/// The gear stays eight whole rigid plates in three bodies: each main plate
/// (two windings) folds inward about the fore-aft line 1.5 units under the
/// wing; the nose plate and the plate across it fold forward about the
/// plate's aft top corner. Gear down is the ground contact (the 26 unit
/// low point); closing never lowers a plate; no plate crosses the centre
/// line; near stow every plate lies inside the skin envelope, or at most
/// half a unit under the wing skin at the mains' outer end.
fn gear(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_wheel_failed |= pose.iter().any(|f| GEAR.contains(&f.address));
        return;
    }
    for ids in [&GEAR_R[..], &GEAR_L[..], &NOSE[..]] {
        let error = rigidity(reference, pose, ids, scale);
        m.reviewed_wheel_rigidity_error = m.reviewed_wheel_rigidity_error.max(error.min(1e9));
        m.reviewed_wheel_failed |= error > EPSILON;
        group_skin_gap(reference, pose, ids, scale, m);
    }
    // The fold lines stay: a virtual point on each plate's hinge is followed
    // through the observed rigid motion (the plate has no vertex on it).
    let before = keyed(reference);
    let after = keyed(pose);
    let hinge = |key: &FaceKey| -> Option<[f32; 3]> {
        if GEAR_R.contains(&key.0) {
            Some([25., -21., -1.5])
        } else if GEAR_L.contains(&key.0) {
            Some([-24., -21., -1.5])
        } else if NOSE.contains(&key.0) {
            Some([0., 57., -5.])
        } else {
            None
        }
    };
    for (key, face) in &before {
        let (Some(h), Some(now)) = (hinge(key), after.get(key)) else {
            continue;
        };
        match virtual_point(face, now, h) {
            Some(q) => {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(h, q) * scale)
            }
            None => m.reviewed_anchor_missing = true,
        }
    }
    // Neither main crosses the centre line.
    for (ids, side) in [(&GEAR_R[..], 1f32), (&GEAR_L[..], -1.)] {
        let (lo, hi) = extent(pose, ids);
        let inboard = if side > 0. { lo[0] } else { -hi[0] };
        m.reviewed_wheel_failed |= inboard < -1e-3;
    }
    let reference_low = lowest(reference, &GEAR);
    let pose_low = lowest(pose, &GEAR);
    m.reviewed_wheel_failed |= !pose_low.is_finite() || pose_low < reference_low - EPSILON;
    if value == 1. {
        // Ground contact: the nose plate's 26 unit low point at a third of a
        // foot a unit is the 8.67 ft the flight model stands the aircraft on.
        m.reviewed_wheel_failed |= (pose_low * scale + 26. / 3.).abs() > 1e-3;
    } else {
        // A main's lowest rear corner moves inboard, the nose foot forward.
        for (ids, foot, side) in [
            (&GEAR_R[..], [25., -28., -25.], 1f32),
            (&GEAR_L[..], [-24., -28., -25.], -1.),
        ] {
            let qs = follow(reference, pose, ids, foot);
            m.reviewed_direction_failures += usize::from(qs.is_empty());
            for q in qs {
                m.reviewed_direction_failures += usize::from((foot[0] - q[0]) * side <= 0.);
            }
        }
        let foot = [0., 69., -26.];
        let qs = follow(reference, pose, &NOSE, foot);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[1] <= foot[1]);
        }
    }
    if value < 1e-5 {
        // Near stow: inside the skin envelope, bar half a unit under the wing.
        for ids in [&GEAR_R[..], &GEAR_L[..], &NOSE[..]] {
            m.reviewed_wheel_failed |= outside(pose, ids) > STOW_OVERSHOOT;
        }
    }
}

/// The dorsal airbrake: open (1) is the source pose; closing turns the whole
/// two-face plate about its forward edge (y 43, z 17), never raising the free
/// edge, down onto the ridge.
fn brake(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| BRAKE.contains(&f.address)));
        return;
    }
    if value == 1. {
        unchanged(reference, pose, &BRAKE, m);
    }
    group_skin_gap(reference, pose, &BRAKE, scale, m);
    let error = rigidity(reference, pose, &BRAKE, scale);
    m.reviewed_rigid_panel_error = m.reviewed_rigid_panel_error.max(error.min(1e9));
    pin(
        reference,
        pose,
        &BRAKE,
        &[[-4., 43., 17.], [4., 43., 17.]],
        scale,
        m,
    );
    for witness in [[-4., 26., 31.], [4., 26., 31.]] {
        let qs = follow(reference, pose, &BRAKE, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            m.reviewed_direction_failures += usize::from(q[2] > witness[2] + EPSILON);
        }
    }
}

/// Each plume grows from the nozzle exit plane (y -78): the root vertices stay
/// on it, the tips pull forward as the demand falls, shared edges stay together.
fn exhaust(value: f64, reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    if value == 0. {
        m.reviewed_direction_failures +=
            usize::from(pose.iter().any(|f| FLAME.contains(&f.address)));
        return;
    }
    group_skin_gap(reference, pose, &FLAME, scale, m);
    for witness in [
        [13., -78., -14.],
        [13., -78., 0.],
        [-12., -78., -14.],
        [-12., -78., 0.],
        [-6., -78., -7.],
        [19., -78., -7.],
    ] {
        let gap = pinned(reference, pose, &FLAME, witness, scale);
        m.reviewed_anchor_missing |= !gap.is_finite();
        if gap.is_finite() {
            m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap);
        }
    }
    for witness in [
        [13., -132., -14.],
        [13., -132., 0.],
        [-12., -132., -14.],
        [-12., -132., 0.],
        [-18., -132., -7.],
        [19., -132., -7.],
    ] {
        let qs = follow(reference, pose, &FLAME, witness);
        m.reviewed_direction_failures += usize::from(qs.is_empty());
        for q in qs {
            if value < 1. {
                m.reviewed_direction_failures += usize::from(q[1] <= witness[1] + EPSILON);
            }
        }
    }
}

/// The surfaces for one combined command (`elevator` and `aileron` in -1..=1,
/// `flaps` in 0..=1). A surface with no net command must be exactly the
/// reference; one with a net command must be a rigid whole panel with its
/// anchor held, no skin gap and its trailing edge on the signed way (the flap
/// is the source's own morph, not a rigid turn). The rudder is not a
/// parameter: nothing may react to it, the fins least of all.
fn surfaces(
    reference: &[Face],
    pose: &[Face],
    scale: f32,
    m: &mut Metrics,
    elevator: f64,
    aileron: f64,
    flaps: f64,
) {
    unchanged(reference, pose, &FIN, m);
    rest_unchanged(reference, pose, &SURFACES, m);
    // Tailerons: a pull raises both trailing edges; a positive aileron raises
    // the right and lowers the left. Pitch (3 parts) outweighs roll (1 part).
    for (ids, side, corners, tip, root) in [
        (
            &TAIL_R[..],
            1f32,
            [[24., -35., -2.], [23., -63., -5.]],
            [50., -71., -5.],
            [24., -35., -2.],
        ),
        (
            &TAIL_L[..],
            -1.,
            [[-23., -35., -2.], [-23., -63., -5.]],
            [-50., -71., -5.],
            [-23., -35., -2.],
        ),
    ] {
        let net = 3. * elevator as f32 + side * aileron as f32;
        if net == 0. {
            unchanged(reference, pose, ids, m);
            continue;
        }
        whole_panel(reference, pose, ids, scale, m);
        let (old, new) = (group(reference, ids), group(pose, ids));
        match midpoint_gap(&old, &new, corners, scale) {
            Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
            None => m.reviewed_anchor_missing = true,
        }
        z_direction(&old, &new, tip, net.signum(), scale, m);
        z_direction(&old, &new, root, -net.signum(), scale, m);
    }
    // Canards: pitch only, a pull raises the leading edge (the aft tip drops).
    for (ids, corners, tip) in [
        (
            &CANARD_R[..],
            [[20., 16., -3.], [20., 31., -3.]],
            [38., 12., -3.],
        ),
        (
            &CANARD_L[..],
            [[-21., 16., -3.], [-21., 30., -3.]],
            [-39., 13., -3.],
        ),
    ] {
        if elevator == 0. {
            unchanged(reference, pose, ids, m);
            continue;
        }
        whole_panel(reference, pose, ids, scale, m);
        let (old, new) = (group(reference, ids), group(pose, ids));
        match midpoint_gap(&old, &new, corners, scale) {
            Some(gap) => m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(gap),
            None => m.reviewed_anchor_missing = true,
        }
        z_direction(&old, &new, tip, -(elevator as f32).signum(), scale, m);
    }
    // Ailerons: one panel about the leading hinge (both ends stay), the
    // trailing edge up for a positive net (positive aileron on the right).
    for (ids, side, hinge, trailing) in [
        (
            &AILERON_R[..],
            1f32,
            [[49., -29., -1.], [76., -27., -1.]],
            [[49., -36., -1.], [76., -40., -1.]],
        ),
        (
            &AILERON_L[..],
            -1.,
            [[-49., -29., -1.], [-78., -27., -1.]],
            [[-49., -36., -1.], [-79., -40., -1.]],
        ),
    ] {
        let net = side * aileron as f32;
        if net == 0. {
            unchanged(reference, pose, ids, m);
            continue;
        }
        whole_panel(reference, pose, ids, scale, m);
        pin(reference, pose, ids, &hinge, scale, m);
        let (old, new) = (group(reference, ids), group(pose, ids));
        for witness in trailing {
            z_direction(&old, &new, witness, net.signum(), scale, m);
        }
    }
    // Flaps: the source's own morph (both windings), hinge edge fixed, the
    // trailing edge down; at full travel the reviewed down vertices.
    for (ids, hinge, trailing, down) in [
        (
            &FLAP_R[..],
            [[23., -22., -1.], [49., -29., -1.]],
            [[23., -32., -1.], [49., -36., -1.]],
            [[23., -30., -7.], [49., -36., -7.]],
        ),
        (
            &FLAP_L[..],
            [[-23., -22., -1.], [-49., -29., -1.]],
            [[-23., -32., -1.], [-49., -36., -1.]],
            [[-23., -30., -7.], [-49., -36., -7.]],
        ),
    ] {
        if flaps == 0. {
            unchanged(reference, pose, ids, m);
            continue;
        }
        group_skin_gap(reference, pose, ids, scale, m);
        pin(reference, pose, ids, &hinge, scale, m);
        let (old, new) = (group(reference, ids), group(pose, ids));
        for witness in trailing {
            z_direction(&old, &new, witness, -1., scale, m);
        }
        if flaps == 1. {
            for (from, to) in trailing.into_iter().zip(down) {
                let qs = follow(reference, pose, ids, from);
                m.reviewed_anchor_missing |= qs.is_empty();
                m.reviewed_neutral_mismatch |=
                    qs.into_iter().any(|q| distance(q, to) * scale > EPSILON);
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
        Control::Gear => {
            gear(value, reference, pose, scale, m);
            rest_unchanged(reference, pose, &[&GEAR], m);
        }
        Control::Brake => {
            brake(value, reference, pose, scale, m);
            rest_unchanged(reference, pose, &[&BRAKE], m);
        }
        Control::Exhaust => {
            exhaust(value, reference, pose, scale, m);
            rest_unchanged(reference, pose, &[&FLAME], m);
        }
        Control::Elevator => surfaces(reference, pose, scale, m, value, 0., 0.),
        Control::Aileron => surfaces(reference, pose, scale, m, 0., value, 0.),
        Control::Flaps => surfaces(reference, pose, scale, m, 0., 0., value),
        // No rudder motion (decision 2026-11-15): the fins and every surface
        // stay at the neutral faces.
        Control::Rudder => surfaces(reference, pose, scale, m, 0., 0., 0.),
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
    failures.extend(dense_gear(airframe, neutral, out, &original, scale)?);
    failures.extend(dense_brake(airframe, neutral, out, scale)?);
    failures.extend(grid(airframe, neutral, out, &original, scale, false)?);
    failures.extend(grid(airframe, neutral, out, &original, scale, true)?);
    failures.extend(dense_surfaces(airframe, neutral, out, &original, scale)?);
    seam_openings(airframe, neutral, out, &original, scale)?;
    Ok(failures)
}

/// The 25 pitch and roll poses (`flaps` false: flaps stay up, so the flap
/// faces must be untouched) or the 25 flap and roll poses at half pitch
/// (`flaps` true): every surface a whole rigid panel with its anchor held,
/// nothing else moved, every trailing edge on the sign of its net command.
fn grid(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
    flaps: bool,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    let mut rows = String::from(if flaps {
        "flaps,roll,finite,anchor_gap_ft,skin_gap_ft,rigidity_ft,crossings,direction_failures,passed\n"
    } else {
        "pitch,roll,finite,anchor_gap_ft,skin_gap_ft,rigidity_ft,taileron_seam_ft,aileron_seam_ft,crossings,direction_failures,passed\n"
    });
    let mut poses = Vec::new();
    let rows_one: [f64; 5] = [-1., -0.5, 0., 0.5, 1.];
    let rows_two: [f64; 5] = if flaps {
        [0., 0.25, 0.5, 0.75, 1.]
    } else {
        rows_one
    };
    for &first in &rows_two {
        for &roll in &rows_one {
            let mut state = neutral.clone();
            let (pitch, flap) = if flaps { (0.5, first) } else { (first, 0.) };
            state.elevator = pitch;
            state.aileron = roll;
            state.flaps = flap;
            let pose = airframe.animation_faces(&state);
            let mut m = measure(original, &pose, scale);
            surfaces(original, &pose, scale, &mut m, pitch, roll, flap);
            let all = |_: [f32; 3]| true;
            let tail = seam_opening(original, &pose, &TAIL_R, scale, &all)
                .max(seam_opening(original, &pose, &TAIL_L, scale, &all));
            let ail = seam_opening(original, &pose, &AILERON_R, scale, &all)
                .max(seam_opening(original, &pose, &AILERON_L, scale, &all));
            let passed = surfaces_pass(&m);
            if flaps {
                writeln!(
                    rows,
                    "{first},{roll},{},{},{},{},{},{},{passed}",
                    m.finite,
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.reviewed_rigid_panel_error,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures
                )?;
            } else {
                writeln!(
                    rows,
                    "{first},{roll},{},{},{},{},{tail},{ail},{},{},{passed}",
                    m.finite,
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.reviewed_rigid_panel_error,
                    m.new_planar_crossings.len(),
                    m.reviewed_direction_failures
                )?;
            }
            if !passed {
                failures.push(format!(
                    "Su-34 {} {first}/roll{roll}: whole-panel anchor, skin, rigidity or direction witness",
                    if flaps { "flap" } else { "pitch" }
                ));
            }
            poses.push(pose);
        }
    }
    let name = if flaps {
        "flap-roll-combinations"
    } else {
        "pitch-roll-combinations"
    };
    fs::write(out.join(format!("{name}.csv")), rows)?;
    contact_sheet(&out.join(format!("{name}.ppm")), original, &poses)?;
    Ok(failures)
}

/// Dense sweeps of the three surface controls (21 values each: elevator and
/// aileron -1 to 1 in 0.1 steps, flaps 0 to 1 in 0.05 steps).
fn dense_surfaces(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<Vec<String>> {
    let mut failures = Vec::new();
    for (name, control, low, step) in [
        ("elevator", Control::Elevator, -1., 0.1),
        ("aileron", Control::Aileron, -1., 0.1),
        ("flaps", Control::Flaps, 0., 0.05),
    ] {
        let mut csv = String::from(
            "value,anchor_gap_ft,skin_gap_ft,rigidity_ft,crossings,direction_failures,passed\n",
        );
        for i in 0..=20 {
            let value = ((low + f64::from(i) * step) * 1000.).round() / 1000.;
            let mut state = neutral.clone();
            control.apply(&mut state, value);
            let pose = airframe.animation_faces(&state);
            let mut m = measure(original, &pose, scale);
            check(control, value, (original, original, &pose), scale, &mut m);
            let passed = surfaces_pass(&m);
            writeln!(
                csv,
                "{value},{},{},{},{},{},{passed}",
                m.max_reviewed_anchor_gap,
                m.max_reviewed_skin_gap,
                m.reviewed_rigid_panel_error,
                m.new_planar_crossings.len(),
                m.reviewed_direction_failures
            )?;
            if !passed {
                failures.push(format!(
                    "Su-34 dense {name} value{value}: whole-panel, anchor or direction gate"
                ));
            }
        }
        fs::write(out.join(format!("dense-{name}.csv")), csv)?;
    }
    Ok(failures)
}

/// The seam opening of every moving surface at full travel (feet).
fn seam_openings(
    airframe: &Airframe,
    neutral: &State,
    out: &Path,
    original: &[Face],
    scale: f32,
) -> AppResult<()> {
    let mut rows = String::from("command,surface,hinge_edge_opening_ft,other_opening_ft\n");
    type Edit = fn(&mut State);
    let commands: [(&str, Edit); 6] = [
        ("pitch-up", |s| s.elevator = 1.),
        ("pitch-down", |s| s.elevator = -1.),
        ("roll-right", |s| s.aileron = 1.),
        ("roll-left", |s| s.aileron = -1.),
        ("flaps-down", |s| s.flaps = 1.),
        ("pitch-up-roll-right", |s| {
            s.elevator = 1.;
            s.aileron = 1.;
        }),
    ];
    // A taileron's root edge is |x| at most 24; a canard's, |x| at most 21;
    // an aileron or a flap hinges on its leading edge (y at least -29.5).
    let at_root = |p: [f32; 3]| p[0].abs() <= 24.;
    let at_canard_root = |p: [f32; 3]| p[0].abs() <= 21.;
    let at_front = |p: [f32; 3]| p[1] >= -29.5;
    type Hinge<'a> = &'a dyn Fn([f32; 3]) -> bool;
    let surfaces: [(&str, &[usize], Hinge); 8] = [
        ("taileron-right", &TAIL_R, &at_root),
        ("taileron-left", &TAIL_L, &at_root),
        ("canard-right", &CANARD_R, &at_canard_root),
        ("canard-left", &CANARD_L, &at_canard_root),
        ("aileron-right", &AILERON_R, &at_front),
        ("aileron-left", &AILERON_L, &at_front),
        ("flap-right", &FLAP_R, &at_front),
        ("flap-left", &FLAP_L, &at_front),
    ];
    for (command, edit) in commands {
        let mut state = neutral.clone();
        edit(&mut state);
        let pose = airframe.animation_faces(&state);
        for (name, ids, hinge) in surfaces {
            let edge = seam_opening(original, &pose, ids, scale, hinge);
            let other = seam_opening(original, &pose, ids, scale, &|p| !hinge(p));
            writeln!(rows, "{command},{name},{edge},{other}")?;
        }
    }
    fs::write(out.join("seam-opening.csv"), rows)?;
    Ok(())
}

/// 202 gear values (0 to 1 in 200 steps plus a near-stow sample): every pose
/// rigid, hinged, grounded at 1, folded the signed way and, near stow, inside
/// the skin envelope. The CSV also records how far each plate group lies
/// outside the skin (units) so the fold across the nacelle belly can be read.
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
    let mut csv = String::from(
        "value,anchor_gap_ft,skin_gap_ft,rigidity_ft,crossings,main_outside_units,nose_outside_units,checks_passed\n",
    );
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
        let main_outside = outside(&pose, &GEAR_R).max(outside(&pose, &GEAR_L));
        let nose_outside = outside(&pose, &NOSE);
        let passed = m.finite
            && !m.reviewed_anchor_missing
            && m.max_reviewed_anchor_gap <= EPSILON
            && m.max_reviewed_skin_gap <= EPSILON
            && m.reviewed_direction_failures == 0
            && !m.reviewed_wheel_failed
            && !m.reviewed_neutral_mismatch
            && m.new_planar_crossings.is_empty();
        writeln!(
            csv,
            "{value},{},{},{},{},{main_outside},{nose_outside},{passed}",
            m.max_reviewed_anchor_gap,
            m.max_reviewed_skin_gap,
            m.reviewed_wheel_rigidity_error,
            m.new_planar_crossings.len()
        )?;
        if !passed {
            failures.push(format!(
                "Su-34 dense gear value{value}: rigid, hinge, direction or stow gate"
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

/// Brake sweep: the closed pose (a near-zero sample) lays the free edge's mid
/// point on the dorsal ridge within a hundredth of a foot, and every pose is
/// rigid with the hinge in place.
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
    // The ridge: from the apex (0, 43, 17) to (0, 24, 15).
    let ridge = |y: f32| 17. - (43. - y) * 2. / 19.;
    let mut csv = String::from("value,rigidity_ft,hinge_gap_ft,free_edge_off_spine_ft,passed\n");
    for value in [1e-6, 0.1, 0.25, 0.5, 0.75, 0.9, 1.] {
        let mut state = neutral.clone();
        state.brake = value;
        let pose = airframe.animation_faces(&state);
        let rigid = rigidity(&reference, &pose, &BRAKE, scale);
        let hinge_gap = pinned(&reference, &pose, &BRAKE, [4., 43., 17.], scale);
        let (mut y, mut z, mut n) = (0f32, 0f32, 0f32);
        for q in follow(&reference, &pose, &BRAKE, [4., 26., 31.])
            .into_iter()
            .chain(follow(&reference, &pose, &BRAKE, [-4., 26., 31.]))
        {
            y += q[1];
            z += q[2];
            n += 1.;
        }
        let off = if n > 0. {
            ((z / n) - ridge(y / n)).abs() * scale
        } else {
            f32::INFINITY
        };
        let closed = value < 1e-5;
        let passed =
            rigid <= EPSILON && hinge_gap <= EPSILON && (!closed || off <= RIDGE_TOLERANCE);
        writeln!(csv, "{value},{rigid},{hinge_gap},{off},{passed}")?;
        if !passed {
            failures.push(format!(
                "Su-34 brake value{value}: rigid, hinge or spine gate"
            ));
        }
    }
    fs::write(out.join("brake-sweep.csv"), csv)?;
    Ok(failures)
}
