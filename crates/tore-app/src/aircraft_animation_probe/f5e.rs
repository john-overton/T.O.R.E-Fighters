//! Independent F-5E whole-panel witnesses: the two all-moving stabilators
//! (pitch only), the two ailerons (differential roll), the two trailing-edge
//! flaps (symmetric droop) and the rudder. Measures the shared Airframe output
//! and never repeats the rig's algorithm. Witness points are named in shape
//! units and scaled with `scale_point` (the F-5E's scale is the identity).
//! Gear, speed brake and plume (F5E-4) are not witnessed here yet.
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

pub(super) fn check(
    c: Control,
    value: f64,
    geometry: (&[Face], &[Face], &[Face]),
    scale: f32,
    m: &mut Metrics,
) {
    let (_, reference, pose) = geometry;
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
    Ok(failures)
}
