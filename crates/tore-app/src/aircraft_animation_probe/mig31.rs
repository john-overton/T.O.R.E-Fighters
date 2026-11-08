//! Independent MiG31 whole-panel witnesses: one hinge line per control surface
//! (all-moving stabilators, outboard ailerons, inboard flaps and twin rudders).
//! Measures the shared Airframe output, never repeats animation algorithms.
use super::*;
const STABILATOR_R: [usize; 2] = [0x3d69, 0x3db1];
const STABILATOR_L: [usize; 2] = [0x39f6, 0x3a3e];
const AILERON_R: [usize; 2] = [0x3ae3, 0x3c64];
const AILERON_L: [usize; 2] = [0x3893, 0x3925];
const FLAP_R: [usize; 2] = [0x41a6, 0x41c5];
const FLAP_L: [usize; 2] = [0x4260, 0x427f];
const RUDDER_R: [usize; 2] = [0x448e, 0x44ad];
const RUDDER_L: [usize; 2] = [0x44eb, 0x450a];
/// address group, the hinge points held fixed (a mid-chord pair for the
/// all-moving stabilator), the trailing witness, its measured axis and the sign
/// of a positive command. Both faces of every panel share one entry.
type Panel = (&'static [usize], &'static [[f32; 3]], [f32; 3], usize, f32);
fn panels(c: Control) -> &'static [Panel] {
    match c {
        Control::Elevator => &[
            (
                &STABILATOR_R,
                &[[17., -24., -3.], [17., -56., -3.]],
                [35., -68., -3.],
                2,
                1.,
            ),
            (
                &STABILATOR_L,
                &[[-17., -24., -3.], [-17., -56., -3.]],
                [-35., -68., -3.],
                2,
                1.,
            ),
        ],
        Control::Rudder => &[
            (
                &RUDDER_R,
                &[[13., -42., 8.], [16., -51., 31.]],
                [13., -53., 8.],
                0,
                1.,
            ),
            (
                &RUDDER_L,
                &[[-13., -42., 8.], [-16., -51., 31.]],
                [-13., -53., 8.],
                0,
                1.,
            ),
        ],
        Control::Flaps => &[
            (
                &FLAP_R,
                &[[17., -5., 4.], [45., -16., 2.]],
                [45., -23., 2.],
                2,
                -1.,
            ),
            (
                &FLAP_L,
                &[[-17., -5., 4.], [-45., -16., 2.]],
                [-45., -23., 2.],
                2,
                -1.,
            ),
        ],
        Control::Aileron => &[
            (
                &AILERON_R,
                &[[45., -16., 2.], [81., -9., -1.]],
                [81., -32., -1.],
                2,
                1.,
            ),
            (
                &AILERON_L,
                &[[-45., -16., 2.], [-81., -9., -1.]],
                [-81., -32., -1.],
                2,
                -1.,
            ),
        ],
        _ => &[],
    }
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
pub(super) fn check(
    c: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    m: &mut Metrics,
) {
    let panels = panels(c);
    if panels.is_empty() {
        return;
    }
    let (_, reference, pose) = geometry;
    for &(ids, anchor, witness, axis, sign) in panels {
        let old: BTreeMap<_, _> = keyed(reference)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        let new: BTreeMap<_, _> = keyed(pose)
            .into_iter()
            .filter(|(k, _)| ids.contains(&k.0))
            .collect();
        // Whole-surface gate: every face of the panel shares one group, so a
        // half-moved face shows as a seam gap.
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
        if matches!(c, Control::Elevator) {
            // The all-moving stabilator keeps its root chord midpoint, not a
            // single corner, so the anchor pair is averaged.
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
                    "MiG31 flap{flap}/roll{roll} whole-panel hinge, seam or direction witness"
                ));
            }
            poses.push(pose);
        }
    }
    fs::write(out.join("flap-roll-combinations.csv"), rows)?;
    contact_sheet(&out.join("flap-roll-combinations.ppm"), &original, &poses)?;
    Ok(failures)
}
