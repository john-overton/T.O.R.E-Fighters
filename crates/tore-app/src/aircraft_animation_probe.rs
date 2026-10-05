//! Headless geometry evidence from the same clean faces the aircraft renderer draws.
//! This is a diagnostic harness. Shared-vertex gaps are candidates for source review,
//! not a claim that every coincident moving/fixed vertex must remain attached.
use crate::{AppResult, aircraft::Airframe, flight::State};
use std::{collections::BTreeMap, fmt::Write as _, fs, path::Path};
use tore_formats::{aircraft::AircraftId, shape::Face};
mod f4;

const EPSILON: f32 = 0.0001;
const CELL_WIDTH: usize = 256;
const CELL_HEIGHT: usize = 192;

#[derive(Clone, Copy, Debug)]
enum Control {
    Elevator,
    Rudder,
    Aileron,
    Gear,
    Flaps,
    Brake,
    Hook,
    Bay,
    VectorPitch,
    VectorYaw,
    Conversion,
    Collective,
    Rotor,
    Sweep,
    GunHeading,
    GunElevation,
    Exhaust,
}
impl Control {
    const ALL: [Self; 17] = [
        Self::Elevator,
        Self::Rudder,
        Self::Aileron,
        Self::Gear,
        Self::Flaps,
        Self::Brake,
        Self::Hook,
        Self::Bay,
        Self::VectorPitch,
        Self::VectorYaw,
        Self::Conversion,
        Self::Collective,
        Self::Rotor,
        Self::Sweep,
        Self::GunHeading,
        Self::GunElevation,
        Self::Exhaust,
    ];
    fn name(self) -> &'static str {
        match self {
            Self::Elevator => "elevator",
            Self::Rudder => "rudder",
            Self::Aileron => "aileron",
            Self::Gear => "gear",
            Self::Flaps => "flaps",
            Self::Brake => "brake",
            Self::Hook => "hook",
            Self::Bay => "bay",
            Self::VectorPitch => "vector-pitch",
            Self::VectorYaw => "vector-yaw",
            Self::Conversion => "conversion",
            Self::Collective => "collective",
            Self::Rotor => "rotor",
            Self::Sweep => "wing-sweep",
            Self::GunHeading => "gun-heading",
            Self::GunElevation => "gun-elevation",
            Self::Exhaust => "exhaust",
        }
    }
    fn values(self) -> Vec<f64> {
        if matches!(self, Self::Gear) {
            return (0..=20).map(|step| f64::from(step) / 20.).collect();
        }
        if matches!(
            self,
            Self::Elevator
                | Self::Rudder
                | Self::Aileron
                | Self::VectorYaw
                | Self::GunHeading
                | Self::GunElevation
        ) {
            vec![-1., -0.5, 0., 0.5, 1.]
        } else {
            vec![0., 0.25, 0.5, 0.75, 1.]
        }
    }
    fn apply(self, state: &mut State, value: f64) {
        match self {
            Self::Elevator => state.elevator = value,
            Self::Rudder => state.rudder = value,
            Self::Aileron => state.aileron = value,
            Self::Gear => state.gear = value,
            Self::Flaps => state.flaps = value,
            Self::Brake => state.brake = value,
            Self::Hook => state.hook = value,
            Self::Bay => state.bay = value,
            Self::VectorPitch => state.lift_controls.vector_pitch_actual = value,
            Self::VectorYaw => state.lift_controls.vector_yaw_actual = value,
            Self::Conversion => state.lift_controls.conversion_actual = value,
            Self::Collective => state.lift_controls.collective_actual = value,
            Self::Rotor => {
                state.engine = true;
                state.throttle = 0.5;
                // Short sub-revolution samples of the runtime's fixed-tick phase.
                state.ticks = (value * 8.).round() as u64;
            }
            Self::Sweep => state.speed = 400. + value * 1600.,
            Self::GunHeading => state
                .gun_aim
                .iter_mut()
                .for_each(|aim| aim[0] = -0.5 + value * 0.1),
            Self::GunElevation => state
                .gun_aim
                .iter_mut()
                .for_each(|aim| aim[1] = value * 0.2),
            Self::Exhaust => state.exhaust = value,
        }
    }
    fn expectation(self, id: AircraftId, state: &State) -> Expectation {
        use AircraftId as Id;
        let helicopter = matches!(id, Id::Ah64 | Id::Mi24 | Id::Ch47);
        match self {
            Self::Elevator | Self::Rudder | Self::Aileron | Self::Flaps if helicopter => {
                Expectation::Unknown
            }
            Self::Elevator | Self::Rudder | Self::Aileron | Self::Flaps => Expectation::Required,
            Self::Gear if matches!(id, Id::Ah64 | Id::Ch47) => Expectation::Unsupported,
            Self::Gear => Expectation::Required,
            Self::Hook if state.hook_available() => Expectation::Required,
            Self::Hook => Expectation::Unsupported,
            Self::Brake if matches!(id, Id::A7 | Id::F4B | Id::F4J | Id::F4E | Id::F4G) => {
                Expectation::Required
            }
            Self::Brake => Expectation::Unknown,
            Self::Bay if matches!(id, Id::F22 | Id::F22n | Id::Faxx) => Expectation::Required,
            Self::Bay => Expectation::Unknown,
            Self::VectorPitch if matches!(id, Id::Av8 | Id::Yak141) => Expectation::Required,
            Self::VectorYaw if matches!(id, Id::Av8 | Id::Yak141) => Expectation::Unknown,
            Self::VectorPitch | Self::VectorYaw => Expectation::Unsupported,
            Self::Conversion if id == Id::V22 => Expectation::Required,
            Self::Conversion => Expectation::Unsupported,
            Self::Collective if matches!(id, Id::V22 | Id::Ah64 | Id::Mi24 | Id::Ch47) => {
                Expectation::Unknown
            }
            Self::Collective => Expectation::Unsupported,
            Self::Rotor
                if matches!(
                    id,
                    Id::C130 | Id::Ac130 | Id::E2 | Id::V22 | Id::Ah64 | Id::Mi24 | Id::Ch47
                ) =>
            {
                Expectation::Required
            }
            Self::Rotor => Expectation::Unknown,
            Self::Sweep if matches!(id, Id::F14 | Id::Mig23) => Expectation::Required,
            Self::Sweep => Expectation::Unknown,
            Self::GunHeading | Self::GunElevation if id == Id::Ac130 => Expectation::Required,
            Self::GunHeading | Self::GunElevation => Expectation::Unsupported,
            Self::Exhaust if matches!(id, Id::A7 | Id::F4B | Id::F4J | Id::F4E | Id::F4G) => {
                Expectation::Required
            }
            Self::Exhaust => Expectation::Unknown,
        }
    }
}
#[derive(Clone, Copy, PartialEq)]
enum Expectation {
    Required,
    Unsupported,
    Unknown,
}
impl Expectation {
    fn name(self) -> &'static str {
        match self {
            Self::Required => "required",
            Self::Unsupported => "not-applicable",
            Self::Unknown => "unreviewed",
        }
    }
}

/// Write an aircraft's sweep evidence without constructing a window or renderer.
/// Missing required movement fails after writing the report and contact sheets.
pub(crate) fn run(data: &BTreeMap<String, Vec<u8>>, id: AircraftId, out: &Path) -> AppResult<()> {
    fs::create_dir_all(out)?;
    let airframe = Airframe::load(data, id)?;
    let mut neutral = State::new(&airframe.profile, [0.; 3])?;
    neutral.engine = false;
    neutral.exhaust = 0.;
    neutral.gear = 0.;
    neutral.flaps = 0.;
    neutral.brake = 0.;
    neutral.hook = 0.;
    neutral.bay = 0.;
    neutral.elevator = 0.;
    neutral.rudder = 0.;
    neutral.aileron = 0.;
    neutral.lift_controls.vector_pitch_actual = 0.;
    neutral.lift_controls.vector_yaw_actual = 0.;
    neutral.lift_controls.conversion_actual = 0.;
    neutral.gun_aim = [[-0.5, 0.]; 3];
    neutral.ticks = 0;
    let base = airframe.animation_faces(&neutral);
    let scale = airframe.animation_scale();
    let raw = tore_formats::shape::Shape::parse(
        data.get(&airframe.profile.shape)
            .ok_or_else(|| format!("missing {}", airframe.profile.shape))?,
    )?;
    write_obj(&out.join("source-neutral.obj"), &raw.faces)?;
    write_obj(&out.join("runtime-neutral.obj"), &base)?;
    contact_sheet(
        &out.join("neutral-vs-source.ppm"),
        &raw.faces,
        std::slice::from_ref(&base),
    )?;
    let mut report = format!(
        "{{\"aircraft\":\"{}\",\"shape\":\"{}\",\"source_units_to_feet\":{scale},\"neutral_faces\":{},\"controls\":[",
        id.pt(),
        airframe.profile.shape,
        base.len()
    );
    let mut notes = format!(
        "{} headless animation sweep\nActual Airframe::animation_faces output; no camera, texture shading or renderer.\nRows: sample values in ascending order. Columns: top (X,Y), side (Y,Z), rear (X,Z).\nGrey: unchanged. Orange: moved or newly visible. Pale blue: reference outline.\nCandidate seam gaps are measured only among vertices coincident in the reference pose.\nThey include intentional moving/fixed boundaries and require source review.\nAll geometry distances in JSON are feet. OBJ vertices retain source coordinates.\n\ncontrol expectation status reversible intermediate_unique\n",
        id.pt()
    );
    let mut failures = Vec::new();
    notes.push_str("Exhaust sweeps the source flame geometry demand directly. It does not test or grant afterburner capability.\n");
    if id == AircraftId::A7 {
        notes.push_str("A7 is non-afterburning. Its fixed fin end strips and flap-adjacent fixed fairings retain their source positions. Their reviewed non-hinge control clearances are reported separately from unresolved seam candidates.\n");
    }
    notes.push('\n');
    for (index, control) in Control::ALL.into_iter().enumerate() {
        let expectation = control.expectation(id, &neutral);
        let values = control.values();
        let poses: Vec<_> = values
            .iter()
            .map(|value| {
                let mut state = neutral.clone();
                control.apply(&mut state, *value);
                airframe.animation_faces(&state)
            })
            .collect();
        // A visible endpoint gives hidden/retracted devices their own stable vertex map.
        let reference = if matches!(
            control,
            Control::Gear | Control::Brake | Control::Hook | Control::Bay | Control::Exhaust
        ) {
            &poses[poses.len() - 1]
        } else {
            &base
        };
        let mut metrics: Vec<_> = poses
            .iter()
            .map(|pose| measure(reference, pose, scale))
            .collect();
        if id == AircraftId::A7 {
            for ((value, pose), metric) in values.iter().zip(&poses).zip(&mut metrics) {
                a7_reviewed_checks(
                    control,
                    *value,
                    (&raw.faces, reference, pose),
                    scale,
                    metric,
                );
            }
        }
        if matches!(
            id,
            AircraftId::F4B | AircraftId::F4J | AircraftId::F4E | AircraftId::F4G
        ) {
            for ((value, pose), metric) in values.iter().zip(&poses).zip(&mut metrics) {
                f4::check(
                    id,
                    control,
                    *value,
                    (&raw.faces, reference, pose),
                    scale,
                    metric,
                );
            }
        }
        let moved = poses
            .iter()
            .any(|pose| geometry_hash(pose) != geometry_hash(&base));
        let all_finite = metrics.iter().all(|metric| metric.finite);
        let reversible = geometry_hash(&base) == geometry_hash(&airframe.animation_faces(&neutral));
        let unique = poses
            .iter()
            .map(|pose| geometry_hash(pose))
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        let status = match (moved, expectation) {
            (true, Expectation::Unsupported) => "unexpected-motion",
            (true, _) => "moved",
            (false, Expectation::Required) => "missing",
            (false, Expectation::Unsupported) => "not-applicable",
            (false, Expectation::Unknown) => "unreviewed",
        };
        if !all_finite
            || !reversible
            || (expectation == Expectation::Unsupported && moved)
            || (expectation == Expectation::Required && (!moved || unique < 3))
            || metrics.iter().any(|metric| {
                metric.reviewed_anchor_missing
                    || metric.max_reviewed_anchor_gap > EPSILON
                    || metric.max_reviewed_skin_gap > EPSILON
                    || metric.reviewed_direction_failures != 0
                    || metric.reviewed_neutral_mismatch
                    || metric.reviewed_wheel_failed
            })
        {
            failures.push(format!("{}: status={status}, finite={all_finite}, reversible={reversible}, unique={unique}, anchor_missing={}, anchor_gap_ft={}, skin_gap_ft={}, direction_failures={}, neutral_mismatch={}, wheel_failed={}", control.name(), metrics.iter().any(|m| m.reviewed_anchor_missing), metrics.iter().map(|m| m.max_reviewed_anchor_gap).fold(0_f32, f32::max), metrics.iter().map(|m| m.max_reviewed_skin_gap).fold(0_f32, f32::max), metrics.iter().map(|m| m.reviewed_direction_failures).sum::<usize>(), metrics.iter().any(|m| m.reviewed_neutral_mismatch), metrics.iter().any(|m| m.reviewed_wheel_failed)));
        }
        writeln!(
            notes,
            "{} {} {status} {reversible} {unique}",
            control.name(),
            expectation.name()
        )?;
        if index != 0 {
            report.push(',');
        }
        write!(
            report,
            "{{\"control\":\"{}\",\"expectation\":\"{}\",\"status\":\"{status}\",\"reversible\":{reversible},\"distinct_poses\":{unique},\"samples\":[",
            control.name(),
            expectation.name()
        )?;
        for (sample, ((value, pose), metric)) in values.iter().zip(&poses).zip(&metrics).enumerate()
        {
            if sample != 0 {
                report.push(',');
            }
            write!(report, "{{\"value\":{value},")?;
            metric.json(&mut report)?;
            report.push('}');
            if metric.finite {
                write_obj(&out.join(format!("{}-{sample}.obj", control.name())), pose)?;
            }
        }
        report.push_str("]}");
        contact_sheet(
            &out.join(format!("{}.ppm", control.name())),
            reference,
            &poses,
        )?;
    }
    report.push_str("]}\n");
    fs::write(out.join("report.json"), report)?;
    fs::write(out.join("index.txt"), notes)?;
    println!(
        "animation probe {}: {} neutral faces, {} required/check failures; {}",
        id.pt(),
        base.len(),
        failures.len(),
        out.display()
    );
    if !failures.is_empty() {
        return Err(format!("animation probe failures: {}", failures.join("; ")).into());
    }
    Ok(())
}

type FaceKey = (usize, usize);
fn keyed(faces: &[Face]) -> BTreeMap<FaceKey, &Face> {
    let mut occurrences = BTreeMap::new();
    faces
        .iter()
        .map(|face| {
            let ordinal = occurrences.entry(face.address).or_insert(0);
            let key = (face.address, *ordinal);
            *ordinal += 1;
            (key, face)
        })
        .collect()
}
fn distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (0..3).map(|i| (a[i] - b[i]).powi(2)).sum::<f32>().sqrt()
}
fn finite(faces: &[Face]) -> bool {
    faces.iter().all(|face| {
        face.positions.iter().flatten().all(|v| v.is_finite())
            && face.normal.is_none_or(|n| n.iter().all(|v| v.is_finite()))
    })
}
fn geometry_hash(faces: &[Face]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for face in faces {
        for value in std::iter::once(face.address as u64)
            .chain(std::iter::once(face.positions.len() as u64))
            .chain(
                face.positions
                    .iter()
                    .flatten()
                    .map(|v| u64::from(v.to_bits())),
            )
        {
            hash = (hash ^ value).wrapping_mul(0x100000001b3);
        }
    }
    hash
}
#[derive(Default)]
struct Metrics {
    finite: bool,
    changed: Vec<FaceKey>,
    added: usize,
    removed: usize,
    topology_changes: usize,
    moved_vertices: usize,
    sides: [usize; 3],
    max_displacement: f32,
    max_edge_change: f32,
    max_coincident_skin_gap: f32,
    max_reviewed_skin_gap: f32,
    max_reviewed_anchor_gap: f32,
    reviewed_anchor_missing: bool,
    reviewed_direction_failures: usize,
    reviewed_neutral_mismatch: bool,
    reviewed_control_z_delta: [f32; 2],
    reviewed_min_wheel_gap: Option<f32>,
    reviewed_wheel_rigidity_error: f32,
    reviewed_wheel_failed: bool,
    seams: Vec<Seam>,
}
struct Seam {
    point: [f32; 3],
    gap: f32,
    addresses: Vec<usize>,
    reviewed_boundary: bool,
}
fn measure(reference: &[Face], pose: &[Face], scale: f32) -> Metrics {
    let before = keyed(reference);
    let after = keyed(pose);
    let mut metric = Metrics {
        finite: finite(pose),
        ..Metrics::default()
    };
    metric.removed = before.keys().filter(|key| !after.contains_key(key)).count();
    for (key, face) in &after {
        let Some(old) = before.get(key) else {
            metric.added += 1;
            continue;
        };
        if old.positions.len() != face.positions.len() {
            metric.topology_changes += 1;
            continue;
        }
        let mut changed = false;
        for (&a, &b) in old.positions.iter().zip(&face.positions) {
            let d = distance(a, b) * scale;
            metric.max_displacement = metric.max_displacement.max(d);
            if d > EPSILON {
                changed = true;
                metric.moved_vertices += 1;
            }
        }
        if changed {
            metric.changed.push(*key);
            let x = old.positions.iter().map(|point| point[0]).sum::<f32>()
                / old.positions.len() as f32;
            metric.sides[if x < -EPSILON {
                0
            } else if x > EPSILON {
                2
            } else {
                1
            }] += 1;
        }
        // Rigidity evidence is per polygon, including its diagonals, not just boundary edges.
        for i in 0..face.positions.len() {
            for j in i + 1..face.positions.len() {
                metric.max_edge_change = metric.max_edge_change.max(
                    (distance(face.positions[i], face.positions[j])
                        - distance(old.positions[i], old.positions[j]))
                    .abs()
                        * scale,
                );
            }
        }
    }
    if metric.finite && finite(reference) {
        metric.seams = shared_vertex_gaps(&before, &after, scale);
        metric.max_coincident_skin_gap = coincident_skin_gap(&before, &after, scale);
    }
    metric
}
// Opposite skins with the same source vertices must map every vertex together,
// even when their windings differ. This separates skin faults from joint gaps.
fn coincident_skin_gap(
    before: &BTreeMap<FaceKey, &Face>,
    after: &BTreeMap<FaceKey, &Face>,
    scale: f32,
) -> f32 {
    let mut panels: BTreeMap<Vec<[i64; 3]>, Vec<FaceKey>> = BTreeMap::new();
    for (key, face) in before {
        let mut points: Vec<_> = face.positions.iter().map(|p| quantized(*p)).collect();
        points.sort();
        panels.entry(points).or_default().push(*key);
    }
    let mut gap = 0_f32;
    for keys in panels.values().filter(|keys| keys.len() > 1) {
        let mut points: BTreeMap<[i64; 3], Vec<[f32; 3]>> = BTreeMap::new();
        for key in keys {
            let old = before[key];
            let Some(new) = after.get(key) else {
                continue;
            };
            if old.positions.len() != new.positions.len() {
                continue;
            }
            for (a, b) in old.positions.iter().zip(&new.positions) {
                points.entry(quantized(*a)).or_default().push(*b);
            }
        }
        for vertices in points.values() {
            for (i, a) in vertices.iter().enumerate() {
                for b in &vertices[i + 1..] {
                    gap = gap.max(distance(*a, *b) * scale);
                }
            }
        }
    }
    gap
}
fn quantized(point: [f32; 3]) -> [i64; 3] {
    point.map(|v| (f64::from(v) * 1000.).round() as i64)
}
fn wheel_separation(
    reference: &[Face],
    pose: &[Face],
    scale: f32,
    travel: f64,
    metric: &mut Metrics,
    groups: (&[usize], &[usize]),
    minimum_gap: f32,
) {
    let right: Vec<_> = pose
        .iter()
        .filter(|f| groups.0.contains(&f.address))
        .collect();
    let left: Vec<_> = pose
        .iter()
        .filter(|f| groups.1.contains(&f.address))
        .collect();
    if travel == 0. {
        metric.reviewed_wheel_failed = !right.is_empty() || !left.is_empty();
        return;
    }
    if right.len() != 2 || left.len() != 2 {
        metric.reviewed_wheel_failed = true;
        return;
    }
    let right_min = right
        .iter()
        .flat_map(|f| &f.positions)
        .map(|p| p[0] * scale)
        .fold(f32::INFINITY, f32::min);
    let left_max = left
        .iter()
        .flat_map(|f| &f.positions)
        .map(|p| p[0] * scale)
        .fold(f32::NEG_INFINITY, f32::max);
    let gap = right_min - left_max;
    metric.reviewed_min_wheel_gap = Some(gap);
    let wheels: Vec<_> = right
        .iter()
        .chain(&left)
        .map(|face| (**face).clone())
        .collect();
    let original: Vec<_> = reference
        .iter()
        .filter(|f| groups.0.contains(&f.address) || groups.1.contains(&f.address))
        .cloned()
        .collect();
    metric.reviewed_wheel_rigidity_error = measure(&original, &wheels, scale).max_edge_change;
    metric.reviewed_wheel_failed = gap < minimum_gap
        || right_min < minimum_gap / 2.
        || left_max > -minimum_gap / 2.
        || metric.reviewed_wheel_rigidity_error > EPSILON;
}
// These attachment coordinates and skin identities are reviewed A7.SH data,
// not a second implementation of its fitted rotation or morph algorithms.
fn a7_reviewed_checks(
    control: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    metric: &mut Metrics,
) {
    let (raw, reference, pose) = geometry;
    if matches!(control, Control::Gear) {
        wheel_separation(
            reference,
            pose,
            scale,
            value,
            metric,
            (&[0x4ceb, 0x4d0a], &[0x4e6c, 0x4e8b]),
            0.70,
        );
    }
    for seam in &mut metric.seams {
        seam.reviewed_boundary = match control {
            Control::Rudder => [[0., -63., 9.], [0., -70., 38.]].contains(&seam.point),
            Control::Flaps => [
                [-7., -18., 8.],
                [7., -19., 8.],
                [-33., -25., 5.],
                [34., -25., 5.],
            ]
            .contains(&seam.point),
            Control::Aileron => [[-33., -25., 5.], [34., -25., 5.]].contains(&seam.point),
            _ => false,
        };
    }
    let (addresses, anchors): (&[usize], &[[f32; 3]]) = match control {
        Control::Rudder => (&[0x4674, 0x4693], &[[0., -65., 38.], [0., -53., 9.]]),
        Control::Hook if value > 0. => (&[0x5057, 0x5076], &[[0., -16., -14.], [0., -18., -11.]]),
        _ => (&[], &[]),
    };
    for address in addresses {
        let Some(actual) = pose.iter().find(|f| f.address == *address) else {
            metric.reviewed_anchor_missing = true;
            continue;
        };
        // The hook only exists in the deployed reference; rudder neutral must
        // come from the raw source, never an already-deflected replacement.
        let original = raw.iter().chain(reference).find(|f| f.address == *address);
        let Some(original) = original else {
            metric.reviewed_anchor_missing = true;
            continue;
        };
        for anchor in anchors {
            let Some(i) = original.positions.iter().position(|p| p == anchor) else {
                metric.reviewed_anchor_missing = true;
                continue;
            };
            if let Some(position) = actual.positions.get(i) {
                metric.max_reviewed_anchor_gap = metric
                    .max_reviewed_anchor_gap
                    .max(distance(*anchor, *position) * scale);
            } else {
                metric.reviewed_anchor_missing = true;
            }
        }
    }
    let pairs: &[(usize, usize)] = match control {
        Control::Rudder => &[(0x4674, 0x4693)],
        Control::Elevator => &[(0x2be6, 0x2c59), (0x1f3a, 0x3506)],
        Control::Aileron => &[(0x4280, 0x42a1), (0x1cf9, 0x1d17)],
        Control::Flaps => &[(0x4a6a, 0x4a91), (0x4963, 0x498a)],
        Control::Hook if value > 0. => &[(0x5057, 0x5076)],
        _ => &[],
    };
    for (a, b) in pairs {
        let selected: Vec<_> = reference
            .iter()
            .filter(|f| f.address == *a || f.address == *b)
            .cloned()
            .collect();
        let actual: Vec<_> = pose
            .iter()
            .filter(|f| f.address == *a || f.address == *b)
            .cloned()
            .collect();
        let gaps = shared_vertex_gaps(&keyed(&selected), &keyed(&actual), scale);
        if let Some(gap) = gaps.first() {
            metric.max_reviewed_skin_gap = metric.max_reviewed_skin_gap.max(gap.gap);
        }
    }
    let trailing: &[([usize; 2], [f32; 3], f32)] = match control {
        Control::Elevator => &[
            ([0x2be6, 0x2c59], [-5., -64., 0.], 1.),
            ([0x1f3a, 0x3506], [5., -64., 0.], 1.),
        ],
        Control::Aileron => &[
            ([0x4280, 0x42a1], [-33., -25., 5.], -1.),
            ([0x1cf9, 0x1d17], [34., -25., 5.], 1.),
        ],
        _ => &[],
    };
    for (side, (addresses, point, positive_sign)) in trailing.iter().enumerate() {
        for address in addresses {
            let original = raw.iter().find(|face| face.address == *address);
            let actual = pose.iter().find(|face| face.address == *address);
            let Some((original, actual)) = original.zip(actual) else {
                metric.reviewed_direction_failures += 1;
                continue;
            };
            if value == 0. {
                metric.reviewed_neutral_mismatch |= original.positions != actual.positions;
                continue;
            }
            let moved = original
                .positions
                .iter()
                .position(|p| p == point)
                .and_then(|i| actual.positions.get(i));
            if let Some(moved) = moved {
                let delta = (moved[2] - point[2]) * scale;
                metric.reviewed_control_z_delta[side] = delta;
                if delta * *positive_sign * value.signum() as f32 <= EPSILON {
                    metric.reviewed_direction_failures += 1;
                }
            } else {
                metric.reviewed_direction_failures += 1;
            }
        }
    }
}
fn shared_vertex_gaps(
    before: &BTreeMap<FaceKey, &Face>,
    after: &BTreeMap<FaceKey, &Face>,
    scale: f32,
) -> Vec<Seam> {
    let mut groups: BTreeMap<[i64; 3], Vec<(FaceKey, [f32; 3])>> = BTreeMap::new();
    for (key, face) in before {
        let Some(moved) = after.get(key) else {
            continue;
        };
        if face.positions.len() != moved.positions.len() {
            continue;
        }
        for (a, b) in face.positions.iter().zip(&moved.positions) {
            groups.entry(quantized(*a)).or_default().push((*key, *b));
        }
    }
    let mut seams = Vec::new();
    for (point, vertices) in groups {
        let mut gap = 0_f32;
        for (i, (key, a)) in vertices.iter().enumerate() {
            for (other, b) in &vertices[i + 1..] {
                if key != other {
                    gap = gap.max(distance(*a, *b) * scale);
                }
            }
        }
        if gap > EPSILON {
            seams.push(Seam {
                point: point.map(|v| v as f32 / 1000.),
                gap,
                reviewed_boundary: false,
                addresses: vertices
                    .iter()
                    .map(|(key, _)| key.0)
                    .collect::<std::collections::BTreeSet<_>>()
                    .into_iter()
                    .collect(),
            });
        }
    }
    seams.sort_by(|a, b| b.gap.total_cmp(&a.gap));
    seams
}
impl Metrics {
    fn json(&self, out: &mut String) -> std::fmt::Result {
        write!(
            out,
            "\"reviewed_wheel_failed\":{},\"reviewed_wheel_rigidity_error_ft\":{},\"reviewed_min_wheel_gap_ft\":{},",
            self.reviewed_wheel_failed,
            self.reviewed_wheel_rigidity_error,
            self.reviewed_min_wheel_gap
                .map_or_else(|| "null".into(), |value| value.to_string())
        )?;
        write!(
            out,
            "\"reviewed_moving_fixed_boundary_count\":{},",
            self.seams
                .iter()
                .filter(|seam| seam.reviewed_boundary)
                .count()
        )?;
        write!(
            out,
            "\"reviewed_direction_failures\":{},\"reviewed_neutral_mismatch\":{},\"reviewed_control_point_z_delta_left_right_ft\":{:?},",
            self.reviewed_direction_failures,
            self.reviewed_neutral_mismatch,
            self.reviewed_control_z_delta
        )?;
        write!(
            out,
            "\"reviewed_anchor_missing\":{},\"max_reviewed_anchor_gap_ft\":{},\"max_reviewed_skin_gap_ft\":{},",
            self.reviewed_anchor_missing, self.max_reviewed_anchor_gap, self.max_reviewed_skin_gap
        )?;
        write!(
            out,
            "\"finite\":{},\"faces\":{{\"changed\":{},\"added\":{},\"removed\":{},\"topology_changes\":{}}},\"moved_vertices\":{},\"moved_faces_left_center_right\":{:?},\"max_displacement_ft\":{},\"max_polygon_distance_change_ft\":{},\"max_coincident_skin_gap_ft\":{},\"candidate_seam_count\":{},\"changed_addresses\":[",
            self.finite,
            self.changed.len(),
            self.added,
            self.removed,
            self.topology_changes,
            self.moved_vertices,
            self.sides,
            self.max_displacement,
            self.max_edge_change,
            self.max_coincident_skin_gap,
            self.seams
                .iter()
                .filter(|seam| !seam.reviewed_boundary)
                .count()
        )?;
        for (i, key) in self.changed.iter().enumerate() {
            if i != 0 {
                out.push(',');
            }
            write!(out, "\"{:x}:{}\"", key.0, key.1)?;
        }
        out.push_str("],\"largest_candidate_seams\":[");
        for (i, seam) in self.seams.iter().take(20).enumerate() {
            if i != 0 {
                out.push(',');
            }
            write!(
                out,
                "{{\"reference_point_source\":{:?},\"gap_ft\":{},\"addresses\":{:?},\"reviewed_moving_fixed_boundary\":{}}}",
                seam.point, seam.gap, seam.addresses, seam.reviewed_boundary
            )?;
        }
        out.push(']');
        Ok(())
    }
}
fn write_obj(path: &Path, faces: &[Face]) -> AppResult<()> {
    let mut text = String::from("# Actual clean Airframe animation faces, source coordinates\n");
    let mut index = 1;
    for face in faces {
        writeln!(text, "g source_{:x}", face.address)?;
        for p in &face.positions {
            writeln!(text, "v {} {} {}", p[0], p[1], p[2])?;
        }
        text.push('f');
        for i in 0..face.positions.len() {
            write!(text, " {}", index + i)?;
        }
        text.push('\n');
        index += face.positions.len();
    }
    fs::write(path, text)?;
    Ok(())
}

/// Orthographic diagnostic outlines, not the game's texture/shading renderer.
/// Animation geometry comes only from the shared Airframe boundary above.
fn contact_sheet(path: &Path, reference: &[Face], poses: &[Vec<Face>]) -> AppResult<()> {
    let width = CELL_WIDTH * 3;
    let height = CELL_HEIGHT * poses.len();
    let mut pixels = vec![18_u8; width * height * 3];
    let before = keyed(reference);
    for (column, axes) in [[0, 1], [1, 2], [0, 2]].into_iter().enumerate() {
        let mut min = [f32::INFINITY; 2];
        let mut max = [f32::NEG_INFINITY; 2];
        for point in reference
            .iter()
            .chain(poses.iter().flatten())
            .flat_map(|f| &f.positions)
        {
            if !point.iter().all(|v| v.is_finite()) {
                continue;
            }
            for i in 0..2 {
                min[i] = min[i].min(point[axes[i]]);
                max[i] = max[i].max(point[axes[i]]);
            }
        }
        let factor = ((CELL_WIDTH - 24) as f32 / (max[0] - min[0]).max(1.))
            .min((CELL_HEIGHT - 24) as f32 / (max[1] - min[1]).max(1.));
        let center = std::array::from_fn::<_, 2, _>(|i| (max[i] + min[i]) * 0.5);
        for (row, pose) in poses.iter().enumerate() {
            let project = |p: [f32; 3]| -> [i32; 2] {
                [
                    column as i32 * CELL_WIDTH as i32
                        + CELL_WIDTH as i32 / 2
                        + ((p[axes[0]] - center[0]) * factor) as i32,
                    row as i32 * CELL_HEIGHT as i32 + CELL_HEIGHT as i32 / 2
                        - ((p[axes[1]] - center[1]) * factor) as i32,
                ]
            };
            for face in reference {
                outline(&mut pixels, width, height, face, &project, [38, 60, 78]);
            }
            for (key, face) in keyed(pose) {
                let changed = before
                    .get(&key)
                    .is_none_or(|old| old.positions != face.positions);
                outline(
                    &mut pixels,
                    width,
                    height,
                    face,
                    &project,
                    if changed {
                        [255, 157, 65]
                    } else {
                        [153, 158, 165]
                    },
                );
            }
        }
    }
    let mut bytes = format!("P6\n{width} {height}\n255\n").into_bytes();
    bytes.extend(pixels);
    fs::write(path, bytes)?;
    Ok(())
}
fn outline(
    pixels: &mut [u8],
    width: usize,
    height: usize,
    face: &Face,
    project: &impl Fn([f32; 3]) -> [i32; 2],
    color: [u8; 3],
) {
    if face.positions.iter().flatten().any(|v| !v.is_finite()) {
        return;
    }
    for i in 0..face.positions.len() {
        let a = project(face.positions[i]);
        let b = project(face.positions[(i + 1) % face.positions.len()]);
        let steps = (a[0] - b[0]).abs().max((a[1] - b[1]).abs()).max(1);
        for step in 0..=steps {
            let x = a[0] + (b[0] - a[0]) * step / steps;
            let y = a[1] + (b[1] - a[1]) * step / steps;
            if x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height {
                let index = (y as usize * width + x as usize) * 3;
                pixels[index..index + 3].copy_from_slice(&color);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            address,
            colors: vec![1; positions.len()],
            uv: vec![[0.; 2]; positions.len()],
            positions,
            normal: None,
            texture: String::new(),
            subtype: 0,
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    #[test]
    fn coincident_opposite_skins_keep_one_seam_when_one_skin_tears() {
        let a = face(1, vec![[0., 0., 0.], [2., 0., 0.], [0., 3., 0.]]);
        let b = face(2, vec![[0., 3., 0.], [2., 0., 0.], [0., 0., 0.]]);
        let before = vec![a, b];
        let mut after = before.clone();
        after[1].positions[0][2] = 6.;
        let result = measure(&before, &after, 1. / 3.);
        assert_eq!(result.changed, [(2, 0)]);
        assert_eq!(result.seams.len(), 1);
        assert_eq!(result.seams[0].addresses, [1, 2]);
        assert_eq!(result.seams[0].gap, 2.);
        assert_eq!(result.max_coincident_skin_gap, 2.);
        assert!(result.max_edge_change > 0.);
    }
    #[test]
    fn shared_rigid_motion_and_split_face_occurrences_do_not_invent_tears() {
        let before = vec![
            face(7, vec![[0., 0., 0.], [1., 0., 0.], [0., 1., 0.]]),
            face(7, vec![[1., 0., 0.], [1., 1., 0.], [0., 1., 0.]]),
        ];
        let mut after = before.clone();
        for face in &mut after {
            for point in &mut face.positions {
                point[2] += 3.;
            }
        }
        let result = measure(&before, &after, 1. / 3.);
        assert_eq!(result.changed, [(7, 0), (7, 1)]);
        assert!(result.seams.is_empty());
        assert_eq!(result.max_edge_change, 0.);
        assert_eq!(result.max_displacement, 1.);
        assert_ne!(geometry_hash(&before), geometry_hash(&after));
        assert_eq!(geometry_hash(&before), geometry_hash(&before.clone()));
    }
    #[test]
    fn hidden_faces_topology_and_nonfinite_are_not_counted_as_zero_motion() {
        let before = vec![face(1, vec![[0.; 3]; 3])];
        let after = vec![face(2, vec![[0.; 3]; 3])];
        let metric = measure(&before, &after, 1.);
        assert_eq!((metric.added, metric.removed), (1, 1));
        let topology = measure(&before, &[face(1, vec![[0.; 3]; 4])], 1.);
        assert_eq!(topology.topology_changes, 1);
        assert!(!measure(&before, &[face(1, vec![[f32::NAN; 3]; 3])], 1.).finite);
    }
    #[test]
    fn reviewed_a7_hinges_gate_motion_and_missing_replacement_skins() {
        let raw = vec![
            face(
                0x4674,
                vec![[0., -65., 38.], [0., -53., 9.], [0., -80., 15.]],
            ),
            face(
                0x4693,
                vec![[0., -80., 15.], [0., -53., 9.], [0., -65., 38.]],
            ),
        ];
        let mut correct = raw.clone();
        correct[0].positions[2][0] = 3.;
        correct[1].positions[0][0] = 3.;
        let mut metric = measure(&raw, &correct, 1. / 3.);
        a7_reviewed_checks(
            Control::Rudder,
            1.,
            (&raw, &raw, &correct),
            1. / 3.,
            &mut metric,
        );
        assert!(!metric.reviewed_anchor_missing);
        assert_eq!(metric.max_reviewed_anchor_gap, 0.);
        assert_eq!(metric.max_reviewed_skin_gap, 0.);
        correct[1].positions[2][0] += 3.;
        let mut torn = measure(&raw, &correct, 1. / 3.);
        a7_reviewed_checks(
            Control::Rudder,
            1.,
            (&raw, &raw, &correct),
            1. / 3.,
            &mut torn,
        );
        assert_eq!(torn.max_reviewed_anchor_gap, 1.);
        assert_eq!(torn.max_reviewed_skin_gap, 1.);
        let mut missing = Metrics::default();
        a7_reviewed_checks(
            Control::Rudder,
            0.,
            (&raw, &raw, &[]),
            1. / 3.,
            &mut missing,
        );
        assert!(missing.reviewed_anchor_missing);
    }
    #[test]
    fn reviewed_roll_direction_rejects_two_wings_moving_together() {
        let raw = vec![
            face(
                0x4280,
                vec![[-33., -19., 4.], [-57., -31., 3.], [-33., -25., 5.]],
            ),
            face(
                0x42a1,
                vec![[-33., -19., 6.], [-33., -25., 5.], [-57., -31., 3.]],
            ),
            face(
                0x1cf9,
                vec![[34., -19., 4.], [34., -25., 5.], [57., -31., 3.]],
            ),
            face(
                0x1d17,
                vec![[34., -19., 6.], [57., -31., 3.], [34., -25., 5.]],
            ),
        ];
        for demand in [-1., -0.5, 0.5, 1.] {
            let mut actual = raw.clone();
            for face in &mut actual {
                for point in &mut face.positions {
                    if point[1] == -25. {
                        point[2] += demand as f32 * if point[0] < 0. { -1. } else { 1. };
                    }
                }
            }
            let mut metric = Metrics::default();
            a7_reviewed_checks(
                Control::Aileron,
                demand,
                (&raw, &raw, &actual),
                1. / 3.,
                &mut metric,
            );
            assert_eq!(metric.reviewed_direction_failures, 0);
            assert_eq!(metric.max_reviewed_skin_gap, 0.);
        }
        let mut wrong = raw.clone();
        for face in &mut wrong {
            for point in &mut face.positions {
                if point[1] == -25. {
                    point[2] += 1.;
                }
            }
        }
        let mut metric = Metrics::default();
        a7_reviewed_checks(
            Control::Aileron,
            1.,
            (&raw, &raw, &wrong),
            1. / 3.,
            &mut metric,
        );
        assert_eq!(metric.reviewed_direction_failures, 2);
        let mut zero = Metrics::default();
        a7_reviewed_checks(
            Control::Aileron,
            0.,
            (&raw, &raw, &wrong),
            1. / 3.,
            &mut zero,
        );
        assert!(zero.reviewed_neutral_mismatch);
    }
    #[test]
    fn wheel_bounds_reject_crossing_and_dimension_changes_at_dense_gear_samples() {
        let source = vec![
            face(1, vec![[2., 0., 0.], [3., 0., 0.], [2., 1., 0.]]),
            face(2, vec![[2., 0., 0.], [2., 1., 0.], [3., 0., 0.]]),
            face(3, vec![[-2., 0., 0.], [-3., 0., 0.], [-2., 1., 0.]]),
            face(4, vec![[-2., 0., 0.], [-2., 1., 0.], [-3., 0., 0.]]),
        ];
        assert_eq!(Control::Gear.values().len(), 21);
        for travel in Control::Gear
            .values()
            .into_iter()
            .filter(|travel| *travel > 0.)
        {
            let mut metric = Metrics::default();
            wheel_separation(
                &source,
                &source,
                1.,
                travel,
                &mut metric,
                (&[1, 2], &[3, 4]),
                0.70,
            );
            assert!(!metric.reviewed_wheel_failed);
            assert_eq!(metric.reviewed_min_wheel_gap, Some(4.));
        }
        let mut crossed = source.clone();
        crossed[0].positions[0][0] = -3.;
        let mut metric = Metrics::default();
        wheel_separation(
            &source,
            &crossed,
            1.,
            0.5,
            &mut metric,
            (&[1, 2], &[3, 4]),
            0.70,
        );
        assert!(metric.reviewed_wheel_failed);
        assert!(metric.reviewed_wheel_rigidity_error > 0.);
    }
}
