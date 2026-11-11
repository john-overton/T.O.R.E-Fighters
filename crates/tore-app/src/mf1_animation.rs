//! Mirage F1 (`MF1.SH`) own-source control surfaces (MF1-3): the two all-moving
//! tailplane panels (whole panels, pitch only), the two outboard trailing-edge
//! panels acting as ailerons (whole panels, differential roll) and the single-fin
//! rudder (one whole panel).
//! See docs/spec/variety-animation.md "Mirage F1 control fit" and
//! docs/formats/aircraft-control-surfaces.md "Mirage F1 control surfaces
//! (MF1-3, 2026-11-10)".
//!
//! MF1.SH ships six state words: 0x5ad0 the afterburner plume, 0x5ad6 the dorsal
//! airbrake plate, 0x5adc the gear, 0x5ae8 and 0x5aee the left and right
//! flap-slot words (each deletes that wing's whole outboard trailing-edge
//! panel and adds nothing) and 0x5af4 the rudder (swaps the rudder quad for a
//! copy about 45 degrees to starboard). The rig never sets the flap-slot words
//! or the rudder word: it keeps the neutral outboard panels and the neutral
//! rudder drawn and turns them, so an aileron never vanishes in flight. The
//! tailplane has no state word (one free flat panel per side), so its role is a
//! fitted whole-panel assignment over the reviewed source faces, pitch only
//! (John, 2026-11-10). There is no slat, spoiler, inboard flap, canopy, nozzle
//! or hook geometry in the source and none is invented. The flap lever moves
//! nothing: the real F1's flaps are separate double-slotted surfaces and the
//! source has no flap panel (the one trailing-edge panel is at the aileron
//! position).
//!
//! Gear, airbrake and burner (MF1-4) are not owned by this rig yet: their faces
//! fall through to the shared fitted device groups in `variety_animation` (the
//! interim gear law). The reviewed face lists and word pins below ([`GEAR`],
//! [`BRAKE`], [`FLAME`]) are the places MF1-4 takes them over.
//!
//! The shape arrives already scaled per axis (MF1-2). Every hinge point named
//! here is in shape units and is scaled with [`scale_point`] (axes too: the
//! line through two scaled points is the scaled line), so a panel turned about
//! it stays rigid.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};

const ID: AircraftId = AircraftId::Mf1;
const WORDS: [usize; 6] = [0x5ad0, 0x5ad6, 0x5adc, 0x5ae8, 0x5aee, 0x5af4];

/// Tailplane: one flat five-vertex panel per side at z -1, both windings, with
/// no state word. The right tip is at x 23 and the left tip at x -22 (the source
/// differs by one unit). The root vertex (+-5, -39, -1) is shared with fixed
/// rear-fuselage faces.
pub(crate) const TAIL_R: [usize; 2] = [0x2d0e, 0x2d36];
pub(crate) const TAIL_L: [usize; 2] = [0x2c60, 0x2c88];
/// Outboard trailing-edge panels, one tapered panel per side of four faces: a
/// quad pair (top, bottom) and a thin leading wedge pair. The flap-slot words
/// 0x5aee (right) and 0x5ae8 (left) delete exactly these faces.
pub(crate) const AILERON_R: [usize; 4] = [0x3ce8, 0x3cc9, 0x3d0f, 0x3d30];
pub(crate) const AILERON_L: [usize; 4] = [0x3e93, 0x3e74, 0x3eba, 0x3edb];
/// Rudder: the hinged quad, both windings. The rudder word 0x5af4 swaps it for
/// the two faces in [`RUDDER_WORD`] (never drawn).
pub(crate) const RUDDER: [usize; 2] = [0x40e1, 0x4108];
const RUDDER_WORD: [usize; 2] = [0x4007, 0x402e];

/// MF1-4 places (pinned at load, not animated here): the gear word 0x5adc adds
/// three flat plates (mains at x +-3 to +-7, nose at x 0), the airbrake word
/// 0x5ad6 one dorsal plate (both windings) and the plume word 0x5ad0 eight
/// plume plates. Each adds faces and removes none.
const GEAR: [usize; 6] = [0x3b7e, 0x3b9d, 0x3be9, 0x3c08, 0x3c54, 0x3c73];
const BRAKE: [usize; 2] = [0x4182, 0x4199];
const FLAME: [usize; 8] = [
    0x4207, 0x422e, 0x4255, 0x427c, 0x42a3, 0x42ca, 0x42f1, 0x4318,
];

/// Tailplane spindle: a lateral line (along X) through the root chord midpoint,
/// (+-5.5, -31.5, -1) (the root chord runs from y -22 to y -41). The panel is
/// flat (z -1 everywhere), so its own anhedral is zero and the spindle is not
/// tilted (the taileron standard, docs/formats/aircraft-control-surfaces.md).
const TAIL_SPINDLE_R: [f32; 3] = [5.5, -31.5, -1.];
const TAIL_SPINDLE_L: [f32; 3] = [-5.5, -31.5, -1.];
const TAIL_AXIS: [f32; 3] = [1., 0., 0.];
/// Aileron hinge: the straight leading edge of the panel's wedge, from the
/// mid-thickness point (23, -6, 1) at the root seam to (35, -11, 1.5) at the
/// tip. The hinge vertices sit half the wedge thickness (1 unit) above and
/// below it, and the whole panel turns about this mid line. The right axis
/// points outboard and aft, so a positive angle lowers the trailing edge; the
/// left panel is the mirror, its axis written inboard and forward (the same
/// line) so a positive angle lowers it too.
const AILERON_HINGE_R: [f32; 3] = [23., -6., 1.];
const AILERON_AXIS_R: [f32; 3] = [12., -5., 0.5];
const AILERON_HINGE_L: [f32; 3] = [-23., -6., 1.];
const AILERON_AXIS_L: [f32; 3] = [12., 5., -0.5];
/// Rudder hinge edge, the fin line from (0, -30, 3) up and aft to (0, -39, 22),
/// pointing toward the top, so a positive angle swings the trailing edge toward
/// +X (right), as the fleet's rudders.
const RUDDER_HINGE: [f32; 3] = [0., -30., 3.];
const RUDDER_AXIS: [f32; 3] = [0., -9., 19.];

/// Fitted magnitudes in radians, all agent decisions (2026-11-10) matching the
/// fleet: the F-4 and F-104 all-moving tails take 0.30 pitch (the Mirage F1's
/// tailplane is pitch only, John 2026-11-10), the F-4, F-16 and F-104 roll
/// surfaces take 0.20 and the fleet's rudders swing 0.35. The retail rudder
/// word swings the quad about 0.8 rad one way; the fleet law is kept.
pub(crate) const TAIL_PITCH: f64 = 0.30;
pub(crate) const AILERON_ROLL: f64 = 0.20;
pub(crate) const RUDDER_TRAVEL: f64 = 0.35;

pub(crate) fn tail_right(address: usize) -> bool {
    TAIL_R.contains(&address)
}
pub(crate) fn tail_left(address: usize) -> bool {
    TAIL_L.contains(&address)
}
pub(crate) fn aileron_right(address: usize) -> bool {
    AILERON_R.contains(&address)
}
pub(crate) fn aileron_left(address: usize) -> bool {
    AILERON_L.contains(&address)
}
pub(crate) fn rudder(address: usize) -> bool {
    RUDDER.contains(&address)
}

/// One tailplane angle in radians, positive lowering the trailing edge. Both
/// panels take the same angle (pitch only, no roll term): a positive elevator
/// raises both trailing edges.
pub(crate) fn tail_angle(state: &State) -> f64 {
    -TAIL_PITCH * state.elevator.clamp(-1., 1.)
}

/// One aileron angle in radians, positive lowering the trailing edge. A
/// positive aileron raises the right trailing edge and lowers the left. The
/// ailerons take no pitch and no flap droop.
pub(crate) fn aileron_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    -side * AILERON_ROLL * state.aileron.clamp(-1., 1.)
}

/// The rudder angle in radians, positive swinging the trailing edge to +X.
pub(crate) fn rudder_angle(state: &State) -> f64 {
    RUDDER_TRAVEL * state.rudder.clamp(-1., 1.)
}

fn sp(p: [f32; 3]) -> [f32; 3] {
    scale_point(ID, p)
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("MF1.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("MF1.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

/// A state word adds exactly `added` and removes exactly `removed`.
fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    added: &[usize],
    removed: &[usize],
) -> AppResult<()> {
    let pose = Shape::with_state(bytes, &[(word, 1)].into())?;
    let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
    if active.difference(neutral).copied().collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || neutral
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed MF1.SH branch {word:x}").into());
    }
    Ok(())
}

/// Reviewed faces as (face addresses, vertex count per face, vertex set in
/// shape units). Every face of a group has exactly these vertices (a subset for
/// the wedge faces), and the group together uses all of them.
type Group = (&'static [usize], &'static [[f32; 3]]);

const TAIL_R_POINTS: [[f32; 3]; 5] = [
    [6., -22., -1.],
    [5., -39., -1.],
    [6., -41., -1.],
    [23., -45., -1.],
    [23., -39., -1.],
];
const TAIL_L_POINTS: [[f32; 3]; 5] = [
    [-6., -22., -1.],
    [-5., -39., -1.],
    [-6., -41., -1.],
    [-22., -45., -1.],
    [-22., -39., -1.],
];
const AILERON_R_POINTS: [[f32; 3]; 8] = [
    [35., -15., 1.],
    [23., -12., 1.],
    [23., -8., 2.],
    [35., -11., 2.],
    [35., -11., 1.],
    [23., -8., 1.],
    [23., -6., 2.],
    [23., -6., 0.],
];
const AILERON_L_POINTS: [[f32; 3]; 8] = [
    [-35., -15., 1.],
    [-23., -12., 1.],
    [-23., -8., 2.],
    [-35., -11., 2.],
    [-35., -11., 1.],
    [-23., -8., 1.],
    [-23., -6., 2.],
    [-23., -6., 0.],
];
const RUDDER_POINTS: [[f32; 3]; 4] = [
    [0., -37., 3.],
    [0., -30., 3.],
    [0., -39., 22.],
    [0., -44., 22.],
];
const GROUPS: [Group; 5] = [
    (&TAIL_R, &TAIL_R_POINTS),
    (&TAIL_L, &TAIL_L_POINTS),
    (&AILERON_R, &AILERON_R_POINTS),
    (&AILERON_L, &AILERON_L_POINTS),
    (&RUDDER, &RUDDER_POINTS),
];

pub struct Rig;
impl Rig {
    /// Checks the parsed (already scaled) neutral shape and the state words
    /// against the reviewed source. The shape is not changed: every surface is a
    /// neutral face the rig turns, and the device words stay with the shared
    /// groups until MF1-4.
    pub fn load(bytes: &[u8], shape: &Shape) -> AppResult<Self> {
        if tore_formats::module::code(bytes)?.0.len() != 19200
            || shape.faces.len() != 236
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed MF1.SH layout".into());
        }
        // Every face of every moving surface is made only of its reviewed
        // vertices (so no vertex is held off a hinge or spindle that the rig
        // does not turn about), and together each side uses all of them.
        for (addresses, points) in GROUPS {
            let mut seen = BTreeSet::new();
            for &address in addresses {
                let face = one(shape, address)?;
                let ok = matches!(face.positions.len(), 3..=5)
                    && face
                        .positions
                        .iter()
                        .all(|&p| points.iter().any(|&q| p == sp(q)));
                if !ok {
                    return Err(format!("unreviewed MF1.SH surface face {address:x}").into());
                }
                seen.extend((0..points.len()).filter(|&i| face.positions.contains(&sp(points[i]))));
            }
            if seen.len() != points.len() {
                return Err(format!("unreviewed MF1.SH surface {:x}", addresses[0]).into());
            }
        }
        // The tailplane panels are five-vertex flat plates, the quads four
        // vertices; pin the counts that matter to the rigid-panel claim.
        for &address in TAIL_R.iter().chain(&TAIL_L) {
            if one(shape, address)?.positions.len() != 5 {
                return Err(format!("unreviewed MF1.SH tailplane face {address:x}").into());
            }
        }
        let neutral: BTreeSet<usize> = shape
            .faces
            .iter()
            .map(|f| f.address)
            .filter(|a| !GEAR.contains(a) && !BRAKE.contains(a) && !FLAME.contains(a))
            .collect();
        // The flap-slot words remove exactly that wing's outboard panel and add
        // nothing. They pin the panels as the surfaces the source's slot order
        // treats as flaps; the rig never sets them.
        branch(bytes, &neutral, 0x5ae8, &[], &AILERON_L)?;
        branch(bytes, &neutral, 0x5aee, &[], &AILERON_R)?;
        // The rudder word swaps the quad for a copy offset toward +X.
        branch(bytes, &neutral, 0x5af4, &RUDDER_WORD, &RUDDER)?;
        // MF1-4 places: the device words add exactly the reviewed faces.
        branch(bytes, &neutral, 0x5adc, &GEAR, &[])?;
        branch(bytes, &neutral, 0x5ad6, &BRAKE, &[])?;
        branch(bytes, &neutral, 0x5ad0, &FLAME, &[])?;
        Ok(Self)
    }

    /// True for the faces this rig turns; every other face is handled by the
    /// shared groups (MF1-4: gear, airbrake and burner move here).
    pub fn owns(&self, address: usize) -> bool {
        tail_right(address)
            || tail_left(address)
            || aileron_right(address)
            || aileron_left(address)
            || rudder(address)
    }

    /// The face turned for the pilot's commands. Each surface is one rigid
    /// panel (all its faces and both windings turn together, no vertex held off
    /// its hinge), so nothing folds.
    pub fn animate(&self, source: &Face, state: &State) -> Face {
        let a = source.address;
        let mut result = source.clone();
        let (pivot, axis, angle) = if tail_right(a) {
            (TAIL_SPINDLE_R, TAIL_AXIS, tail_angle(state))
        } else if tail_left(a) {
            (TAIL_SPINDLE_L, TAIL_AXIS, tail_angle(state))
        } else if aileron_right(a) {
            (AILERON_HINGE_R, AILERON_AXIS_R, aileron_angle(true, state))
        } else if aileron_left(a) {
            (AILERON_HINGE_L, AILERON_AXIS_L, aileron_angle(false, state))
        } else if rudder(a) {
            (RUDDER_HINGE, RUDDER_AXIS, rudder_angle(state))
        } else {
            return result;
        };
        if angle != 0. {
            turn(&mut result, sp(pivot), sp(axis), angle);
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    fn face(address: usize, points: &[[f32; 3]]) -> Face {
        let positions: Vec<[f32; 3]> = points.iter().map(|&p| sp(p)).collect();
        Face {
            address,
            colors: vec![31; positions.len()],
            uv: vec![[0.2, 0.8]; positions.len()],
            positions,
            texture: "SYNTHETIC".into(),
            subtype: 0xed,
            normal: Some([0., 0., 1.]),
            fog: tore_formats::shape::FogMode::Enabled,
        }
    }
    /// The reviewed surfaces as synthetic faces: each address with the vertices
    /// the retail face holds (neutral dump, shape units).
    fn surfaces() -> Vec<Face> {
        let p = |a: &[[f32; 3]], i: &[usize]| i.iter().map(|&k| a[k]).collect::<Vec<_>>();
        vec![
            face(0x2d0e, &TAIL_R_POINTS),
            face(0x2d36, &p(&TAIL_R_POINTS, &[1, 0, 4, 3, 2])),
            face(0x2c60, &TAIL_L_POINTS),
            face(0x2c88, &p(&TAIL_L_POINTS, &[0, 4, 3, 2, 1])),
            face(0x3ce8, &p(&AILERON_R_POINTS, &[0, 1, 2, 3])),
            face(0x3cc9, &p(&AILERON_R_POINTS, &[4, 5, 1, 0])),
            face(0x3d0f, &p(&AILERON_R_POINTS, &[6, 3, 2])),
            face(0x3d30, &p(&AILERON_R_POINTS, &[5, 4, 7])),
            face(0x3e93, &p(&AILERON_L_POINTS, &[0, 3, 2, 1])),
            face(0x3e74, &p(&AILERON_L_POINTS, &[4, 0, 1, 5])),
            face(0x3eba, &p(&AILERON_L_POINTS, &[6, 2, 3])),
            face(0x3edb, &p(&AILERON_L_POINTS, &[5, 7, 4])),
            face(0x40e1, &RUDDER_POINTS),
            face(0x4108, &p(&RUDDER_POINTS, &[3, 2, 1, 0])),
        ]
    }
    fn group(faces: &[Face], ids: &[usize]) -> Vec<Face> {
        faces
            .iter()
            .filter(|f| ids.contains(&f.address))
            .cloned()
            .collect()
    }
    fn turned(faces: &[Face], s: &State) -> Vec<Face> {
        faces.iter().map(|f| Rig.animate(f, s)).collect()
    }
    fn points(faces: &[Face]) -> Vec<[f32; 3]> {
        faces.iter().flat_map(|f| f.positions.clone()).collect()
    }
    fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
        (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt()
    }
    fn spread(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
        let mut worst = 0f32;
        for i in 0..a.len() {
            for j in i + 1..a.len() {
                worst = worst.max((dist(a[i], a[j]) - dist(b[i], b[j])).abs());
            }
        }
        worst
    }
    #[test]
    fn neutral_commands_leave_every_surface_exactly_where_it_is() {
        let s = state();
        for f in surfaces() {
            assert_eq!(
                Rig.animate(&f, &s).positions,
                f.positions,
                "{:x}",
                f.address
            );
        }
    }
    #[test]
    fn every_surface_turns_as_one_rigid_panel_with_no_seam_between_its_faces() {
        let all = surfaces();
        for ids in [&TAIL_R[..], &TAIL_L, &AILERON_R, &AILERON_L, &RUDDER] {
            let source = group(&all, ids);
            for (elevator, aileron, rudder) in [
                (1., 0., 0.),
                (-1., 0., 0.),
                (0., 1., 0.),
                (0., -1., 0.),
                (0., 0., 1.),
                (0., 0., -1.),
                (0.5, -1., 0.5),
            ] {
                let mut s = state();
                s.elevator = elevator;
                s.aileron = aileron;
                s.rudder = rudder;
                let out = turned(&source, &s);
                let (before, after) = (points(&source), points(&out));
                assert!(
                    spread(&before, &after) < 1e-3,
                    "{:x}: not rigid at {elevator}/{aileron}/{rudder}",
                    ids[0]
                );
                // A vertex shared by two faces of the panel stays one point.
                for (i, p) in before.iter().enumerate() {
                    for (j, q) in before.iter().enumerate() {
                        if p == q {
                            assert_eq!(after[i], after[j], "seam opened inside {:x}", ids[0]);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn the_tailplane_is_pitch_only_and_both_sides_move_together() {
        let all = surfaces();
        let tails = group(&all, &[&TAIL_R[..], &TAIL_L].concat());
        let mut s = state();
        s.aileron = 1.;
        s.rudder = 1.;
        s.flaps = 1.;
        assert_eq!(
            points(&turned(&tails, &s)),
            points(&tails),
            "tail took roll"
        );
        s.aileron = 0.;
        for elevator in [1., 0.5, -0.5, -1.] {
            s.elevator = elevator;
            let out = turned(&tails, &s);
            // Trailing root vertex of each side: a positive elevator raises it.
            for (address, root) in [(0x2d0e, [6., -41., -1.]), (0x2c60, [-6., -41., -1.])] {
                let before = tails.iter().find(|f| f.address == address).unwrap();
                let after = out.iter().find(|f| f.address == address).unwrap();
                let i = before
                    .positions
                    .iter()
                    .position(|&p| p == sp(root))
                    .unwrap();
                assert!(
                    (after.positions[i][2] - before.positions[i][2]) * elevator as f32 > 0.,
                    "{address:x}: wrong sense at {elevator}"
                );
            }
            // Mirror sides turn by the same amount (equal root trailing rise).
            let rise = |address, root: [f32; 3]| {
                let before = tails.iter().find(|f| f.address == address).unwrap();
                let after = out.iter().find(|f| f.address == address).unwrap();
                let i = before
                    .positions
                    .iter()
                    .position(|&p| p == sp(root))
                    .unwrap();
                after.positions[i][2] - before.positions[i][2]
            };
            assert!((rise(0x2d0e, [6., -41., -1.]) - rise(0x2c60, [-6., -41., -1.])).abs() < 1e-5);
        }
    }
    #[test]
    fn the_spindle_is_lateral_through_the_root_chord_midpoint() {
        // The root chord ends (6,-22,-1) and (6,-41,-1) have their midpoint on
        // the spindle, so it stays put; the spindle is along X (no span tilt).
        let all = surfaces();
        let tail = group(&all, &TAIL_R);
        let mut s = state();
        s.elevator = 1.;
        let out = turned(&tail, &s);
        let (a, b) = (sp([6., -22., -1.]), sp([6., -41., -1.]));
        let find = |faces: &[Face], p: [f32; 3]| {
            faces[0].positions[tail[0].positions.iter().position(|&q| q == p).unwrap()]
        };
        let mid =
            |x: [f32; 3], y: [f32; 3]| std::array::from_fn::<f32, 3, _>(|k| (x[k] + y[k]) / 2.);
        let (was, now) = (mid(a, b), mid(find(&out, a), find(&out, b)));
        for k in 0..3 {
            assert!((was[k] - now[k]).abs() < 1e-4, "root midpoint moved on {k}");
        }
        // Every vertex keeps its x: a lateral axis only moves y and z.
        for (p, q) in points(&tail).iter().zip(points(&out)) {
            assert!((p[0] - q[0]).abs() < 1e-5, "span axis tilt");
        }
    }
    #[test]
    fn ailerons_roll_differentially_and_take_no_pitch_or_flap() {
        let all = surfaces();
        let ails = group(&all, &[&AILERON_R[..], &AILERON_L].concat());
        let mut s = state();
        s.elevator = 1.;
        s.rudder = 1.;
        s.flaps = 1.;
        assert_eq!(
            points(&turned(&ails, &s)),
            points(&ails),
            "pitch or flap leaked"
        );
        s.elevator = 0.;
        s.rudder = 0.;
        s.flaps = 0.;
        let trailing = |s: &State, right: bool| {
            let (address, edge) = if right {
                (0x3ce8, [35., -15., 1.])
            } else {
                (0x3e93, [-35., -15., 1.])
            };
            let source = ails.iter().find(|f| f.address == address).unwrap();
            let out = Rig.animate(source, s);
            let i = source
                .positions
                .iter()
                .position(|&p| p == sp(edge))
                .unwrap();
            out.positions[i][2] - sp(edge)[2]
        };
        s.aileron = 1.;
        assert!(
            trailing(&s, true) > 0.01 && trailing(&s, false) < -0.01,
            "positive aileron: right up, left down"
        );
        s.aileron = -1.;
        assert!(trailing(&s, true) < -0.01 && trailing(&s, false) > 0.01);
        // The wedge hinge pair keeps its midpoint on the axis.
        s.aileron = 1.;
        let source = ails.iter().find(|f| f.address == 0x3d0f).unwrap();
        let out = Rig.animate(source, &s);
        let (i, j) = (
            source
                .positions
                .iter()
                .position(|&p| p == sp([23., -6., 2.]))
                .unwrap(),
            source
                .positions
                .iter()
                .position(|&p| p == sp([35., -11., 2.]))
                .unwrap(),
        );
        // (23,-6,2) is one end of the root hinge pair; the pair's other end is
        // on the bottom wedge. Its distance to the axis cannot change.
        let axis_distance = |p: [f32; 3]| {
            let (o, d) = (sp(AILERON_HINGE_R), sp(AILERON_AXIS_R));
            let r: [f32; 3] = std::array::from_fn(|k| p[k] - o[k]);
            let t =
                (0..3).map(|k| r[k] * d[k]).sum::<f32>() / (0..3).map(|k| d[k] * d[k]).sum::<f32>();
            dist(r, std::array::from_fn(|k| t * d[k]))
        };
        for k in [i, j] {
            assert!(
                (axis_distance(source.positions[k]) - axis_distance(out.positions[k])).abs() < 1e-4
            );
        }
    }
    #[test]
    fn the_flap_lever_moves_nothing_and_no_panel_is_ever_dropped() {
        let all = surfaces();
        let mut s = state();
        for flaps in [0., 0.5, 1.] {
            s.flaps = flaps;
            for f in &all {
                assert!(Rig.owns(f.address));
                assert_eq!(Rig.animate(f, &s).positions, f.positions);
            }
        }
    }
    #[test]
    fn the_rudder_is_one_rigid_panel_keeping_its_hinge_edge() {
        let all = surfaces();
        let panel = group(&all, &RUDDER);
        let mut s = state();
        for rudder in [-1., -0.5, 0.5, 1.] {
            s.rudder = rudder;
            let out = turned(&panel, &s);
            assert!(spread(&points(&panel), &points(&out)) < 1e-3);
            for edge in [[0., -30., 3.], [0., -39., 22.]] {
                let p = sp(edge);
                for (a, b) in points(&panel).iter().zip(points(&out)) {
                    if *a == p {
                        assert!(
                            (0..3).all(|k| (b[k] - p[k]).abs() < 1e-3),
                            "hinge vertex moved"
                        );
                    }
                }
            }
            // A positive command swings the trailing edge to +X.
            let trailing = out[0].positions[3][0];
            assert!(trailing * rudder as f32 > 0., "wrong sense at {rudder}");
        }
    }
    #[test]
    fn faces_the_rig_does_not_own_are_left_to_the_shared_groups() {
        for address in GEAR.iter().chain(&BRAKE).chain(&FLAME) {
            assert!(!Rig.owns(*address));
        }
        // The word faces never overlap the reviewed surfaces.
        for address in GEAR.iter().chain(&BRAKE).chain(&FLAME).chain(&RUDDER_WORD) {
            for (ids, _) in GROUPS {
                assert!(!ids.contains(address));
            }
        }
    }
}
