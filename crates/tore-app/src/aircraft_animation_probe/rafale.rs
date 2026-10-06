//! RAF.SH independent attachment and posed-geometry witnesses.
use super::*;
const LEFT_FLAPS: [usize; 2] = [0x4190, 0x41af];
const RIGHT_FLAPS: [usize; 2] = [0x40a9, 0x40c8];
const CANARDS: [usize; 4] = [0x3d81, 0x3d98, 0x3d26, 0x3d3d];
const RUDDER: [usize; 2] = [0x3f08, 0x3f27];
const GEAR: [usize; 12] = [
    0x3c50, 0x3c6f, 0x3be5, 0x3c04, 0x3cbb, 0x3cda, 0x3b2f, 0x3b46, 0x3ad4, 0x3aeb, 0x3b8a, 0x3ba1,
];
const BRAKE: [usize; 4] = [0x3e1e, 0x3e35, 0x3ebb, 0x3ed2];
pub(super) struct Sources {
    gear: Vec<Face>,
    brake: Vec<Face>,
    flame: Vec<Face>,
}
impl Sources {
    pub(super) fn load(bytes: &[u8]) -> AppResult<Self> {
        let shape = tore_formats::shape::Shape::with_state(
            bytes,
            &[(0x5b50, 1), (0x5b56, 1), (0x5b62, 1)].into(),
        )?;
        let gear: Vec<_> = shape
            .faces
            .iter()
            .filter(|f| GEAR.contains(&f.address))
            .cloned()
            .collect();
        let brake: Vec<_> = shape
            .faces
            .iter()
            .filter(|f| BRAKE.contains(&f.address))
            .cloned()
            .collect();
        let flame: Vec<_> = shape
            .faces
            .iter()
            .filter(|f| (0x4265..=0x44ae).contains(&f.address))
            .cloned()
            .collect();
        if gear.len() != 12 || brake.len() != 4 || flame.len() != 16 {
            return Err("Rafale probe source branch membership changed".into());
        }
        Ok(Self { gear, brake, flame })
    }
}
fn selected<'a>(faces: &'a [Face], ids: &[usize]) -> BTreeMap<FaceKey, &'a Face> {
    keyed(faces)
        .into_iter()
        .filter(|(k, _)| ids.contains(&k.0))
        .collect()
}
fn same_skins(before: &[Face], after: &[Face], ids: &[usize], scale: f32, m: &mut Metrics) {
    let a = selected(before, ids);
    let b = selected(after, ids);
    m.reviewed_anchor_missing |= a.is_empty() || a.keys().any(|key| !b.contains_key(key));
    if let Some(gap) = shared_vertex_gaps(&a, &b, scale).first() {
        m.max_reviewed_skin_gap = m.max_reviewed_skin_gap.max(gap.gap);
    }
}
fn flaps(reference: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let after = keyed(pose);
    for (key, before) in keyed(reference)
        .into_iter()
        .filter(|(key, _)| LEFT_FLAPS.contains(&key.0) || RIGHT_FLAPS.contains(&key.0))
    {
        let Some(actual) = after.get(&key) else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        if actual.positions.len() != before.positions.len() {
            m.reviewed_anchor_missing = true;
            continue;
        }
        for (p, q) in before.positions.iter().zip(&actual.positions) {
            if p[1] == -23. {
                m.max_reviewed_anchor_gap = m.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
            }
        }
    }
    same_skins(reference, pose, &LEFT_FLAPS, scale, m);
    same_skins(reference, pose, &RIGHT_FLAPS, scale, m);
}
fn rigid(
    reference: &[Face],
    pose: &[Face],
    ids: &[usize],
    pivot: [f32; 3],
    scale: f32,
    m: &mut Metrics,
) {
    let a = selected(reference, ids);
    let b = selected(pose, ids);
    m.reviewed_anchor_missing |= a.len() != ids.len() || b.len() != ids.len();
    for (key, before) in a {
        if let Some(actual) = b.get(&key) {
            super::rotorcraft::inspect_panel(before, actual, pivot, scale, m);
        }
    }
    same_skins(reference, pose, ids, scale, m);
}
fn canards(reference: &[Face], pose: &[Face], pitch: f64, scale: f32, m: &mut Metrics) {
    rigid(reference, pose, &CANARDS[..2], [-10., 35., 1.], scale, m);
    rigid(reference, pose, &CANARDS[2..], [10., 35., 1.], scale, m);
    let after = keyed(pose);
    for (key, before) in selected(reference, &CANARDS) {
        let Some(actual) = after.get(&key) else {
            m.reviewed_anchor_missing = true;
            continue;
        };
        for (p, q) in before.positions.iter().zip(&actual.positions) {
            if pitch == 0. {
                m.reviewed_direction_failures += usize::from(distance(*p, *q) * scale > EPSILON);
            } else if p[1] > 35. {
                m.reviewed_direction_failures +=
                    usize::from((q[2] - p[2]) * pitch.signum() as f32 <= EPSILON);
            }
        }
    }
}
fn endpoint(source: &[Face], pose: &[Face], scale: f32, m: &mut Metrics) {
    let actual = keyed(pose);
    for f in source {
        let Some(g) = actual.get(&(f.address, 0)) else {
            m.reviewed_neutral_mismatch = true;
            continue;
        };
        m.reviewed_neutral_mismatch |= f.positions.len() != g.positions.len()
            || f.positions
                .iter()
                .zip(&g.positions)
                .any(|(p, q)| distance(*p, *q) * scale > EPSILON);
    }
}
pub(super) fn check(
    control: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    source: &Sources,
    m: &mut Metrics,
) {
    let (raw, reference, pose) = geometry;
    if matches!(control, Control::Gear | Control::Brake | Control::Exhaust) && value == 0. {
        let source = match control {
            Control::Gear => &source.gear,
            Control::Brake => &source.brake,
            _ => &source.flame,
        };
        m.reviewed_neutral_mismatch |= pose
            .iter()
            .any(|f| source.iter().any(|g| g.address == f.address));
        return;
    }
    let ids: Vec<usize> = match control {
        Control::Elevator => LEFT_FLAPS
            .into_iter()
            .chain(RIGHT_FLAPS)
            .chain(CANARDS)
            .collect(),
        Control::Aileron | Control::Flaps => LEFT_FLAPS.into_iter().chain(RIGHT_FLAPS).collect(),
        Control::Rudder => RUDDER.into(),
        _ => Vec::new(),
    };
    if value == 0. && !ids.is_empty() {
        endpoint(
            &raw.iter()
                .filter(|f| ids.contains(&f.address))
                .cloned()
                .collect::<Vec<_>>(),
            pose,
            scale,
            m,
        );
    }
    if matches!(
        control,
        Control::Elevator | Control::Aileron | Control::Flaps
    ) {
        flaps(reference, pose, scale, m);
        let after = keyed(pose);
        if value != 0. {
            for (key, before) in keyed(reference)
                .into_iter()
                .filter(|(key, _)| LEFT_FLAPS.contains(&key.0) || RIGHT_FLAPS.contains(&key.0))
            {
                let Some(actual) = after.get(&key) else {
                    m.reviewed_anchor_missing = true;
                    continue;
                };
                let sign = match control {
                    Control::Flaps => -1.,
                    Control::Aileron => {
                        if LEFT_FLAPS.contains(&key.0) {
                            -1.
                        } else {
                            1.
                        }
                    }
                    _ => 1.,
                };
                for (p, q) in before.positions.iter().zip(&actual.positions) {
                    if p[1] < -23. {
                        m.reviewed_direction_failures +=
                            usize::from((q[2] - p[2]) * sign * value.signum() as f32 <= EPSILON);
                    }
                }
            }
        }
    }
    match control {
        Control::Elevator => {
            canards(reference, pose, value, scale, m);
        }
        Control::Rudder => {
            rigid(reference, pose, &RUDDER, [0., -36., 5.], scale, m);
            rigid(reference, pose, &RUDDER, [0., -44., 30.], scale, m);
            if value != 0. {
                let before = selected(reference, &RUDDER);
                let after = selected(pose, &RUDDER);
                let points = super::f4::witness_positions(&before, &after, [0., -47., 30.]);
                m.reviewed_direction_failures += usize::from(points.len() != 2);
                for p in points {
                    m.reviewed_direction_failures +=
                        usize::from(p[0] * value.signum() as f32 <= EPSILON);
                }
            }
        }
        Control::Brake => {
            rigid(reference, pose, &BRAKE[..2], [2., -3., 4.], scale, m);
            rigid(reference, pose, &BRAKE[..2], [8., -3., 2.], scale, m);
            rigid(reference, pose, &BRAKE[2..], [-2., -3., 4.], scale, m);
            rigid(reference, pose, &BRAKE[2..], [-8., -3., 2.], scale, m);
            if value == 1. {
                endpoint(&source.brake, pose, scale, m);
            }
        }
        Control::Gear => {
            for (ids, pivot) in [
                (&GEAR[0..2], [-4., 14.545455, -7.]),
                (&GEAR[2..4], [4., 14.545455, -7.]),
                (&GEAR[4..6], [0., 60., -6.]),
                (&GEAR[6..8], [-2., 7., -7.]),
                (&GEAR[8..10], [2., 7., -7.]),
                (&GEAR[10..12], [-1., 52., -6.]),
                (&GEAR[10..12], [-1., 68., -5.]),
            ] {
                rigid(reference, pose, ids, pivot, scale, m);
            }
            if value == 1. {
                endpoint(&source.gear, pose, scale, m);
            }
            let mut left = f32::NEG_INFINITY;
            let mut right = f32::INFINITY;
            for f in pose.iter().filter(|f| GEAR[..4].contains(&f.address)) {
                for p in &f.positions {
                    if GEAR[..2].contains(&f.address) {
                        left = left.max(p[0] * scale);
                    } else {
                        right = right.min(p[0] * scale);
                    }
                    if value < 1e-5 {
                        m.reviewed_wheel_failed |= (p[0].abs() - 4.).abs() > EPSILON
                            || !(3.54..=14.55).contains(&p[1])
                            || !(-7.46..=2.55).contains(&p[2]);
                    }
                }
            }
            m.reviewed_min_wheel_gap = Some(right - left);
            m.reviewed_wheel_failed |= right - left < 2.66 || left > -1.33 || right < 1.33;
            m.reviewed_wheel_rigidity_error = m.reviewed_rigid_panel_error;
        }
        Control::Exhaust => {
            if value == 1. {
                endpoint(&source.flame, pose, scale, m);
            }
            let after = keyed(pose);
            for f in &source.flame {
                let Some(g) = after.get(&(f.address, 0)) else {
                    m.reviewed_anchor_missing = true;
                    continue;
                };
                for (p, q) in f.positions.iter().zip(&g.positions) {
                    if p[1] == -51. {
                        m.max_reviewed_anchor_gap =
                            m.max_reviewed_anchor_gap.max(distance(*p, *q) * scale);
                    }
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
    source: &Sources,
) -> AppResult<Vec<String>> {
    let reference = airframe.animation_faces(neutral);
    let scale = airframe.animation_scale();
    let mut failures = Vec::new();
    let mut csv = String::from("flap,pitch,roll,anchor_error_ft,skin_gap_ft,planar_crossings\n");
    let mut poses = Vec::new();
    for flap in [0., 0.25, 0.5, 0.75, 1.] {
        for pitch in [-1., -0.5, 0., 0.5, 1.] {
            for roll in [-1., -0.5, 0., 0.5, 1.] {
                let mut state = neutral.clone();
                state.flaps = flap;
                state.elevator = pitch;
                state.aileron = roll;
                let actual = airframe.animation_faces(&state);
                let mut m = measure(&reference, &actual, scale);
                flaps(&reference, &actual, scale, &mut m);
                canards(&reference, &actual, pitch, scale, &mut m);
                if flap == 0. && pitch == 0. && roll == 0. {
                    check(
                        Control::Flaps,
                        0.,
                        (&reference, &reference, &actual),
                        scale,
                        source,
                        &mut m,
                    );
                }
                writeln!(
                    csv,
                    "{flap},{pitch},{roll},{},{},{}",
                    m.max_reviewed_anchor_gap,
                    m.max_reviewed_skin_gap,
                    m.new_planar_crossings.len()
                )?;
                if !m.finite
                    || m.reviewed_anchor_missing
                    || m.max_reviewed_anchor_gap > EPSILON
                    || m.max_reviewed_skin_gap > EPSILON
                    || m.reviewed_rigid_panel_error > EPSILON
                    || m.reviewed_direction_failures != 0
                    || !m.new_planar_crossings.is_empty()
                {
                    failures.push(format!(
                        "Rafale mixed flap{flap}/pitch{pitch}/roll{roll} attachment or topology"
                    ));
                }
                if [0., 1.].contains(&flap)
                    && [-1., 1.].contains(&pitch)
                    && [-1., 1.].contains(&roll)
                {
                    poses.push(actual);
                }
            }
        }
    }
    fs::write(out.join("flap-pitch-roll-combinations.csv"), csv)?;
    contact_sheet(
        &out.join("flap-pitch-roll-extremes.ppm"),
        &reference,
        &poses,
    )?;
    Ok(failures)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flap_witness_rejects_leading_edge_detachment() {
        let source = LEFT_FLAPS.map(|a| {
            super::super::tests::face(
                a,
                vec![
                    [-10., -23., 0.],
                    [-50., -23., 0.],
                    [-50., -30., 0.],
                    [-10., -30., 0.],
                ],
            )
        });
        let mut moved = source.clone();
        for f in &mut moved {
            f.positions[0][2] = 1.;
        }
        let mut m = Metrics::default();
        flaps(&source, &moved, 1., &mut m);
        assert!(m.max_reviewed_anchor_gap > 0.9);
    }
}
