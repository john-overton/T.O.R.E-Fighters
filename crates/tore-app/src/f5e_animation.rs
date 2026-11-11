//! F-5E Tiger (`F5E.SH`) own-source control surfaces (F5E-3): the two
//! all-moving stabilators (whole panels, pitch only), the two ailerons (whole
//! panels, differential roll), the two trailing-edge flaps (whole panels,
//! symmetric droop) and the single-fin rudder (one whole panel).
//! See docs/spec/variety-animation.md "F-5E control fit" and
//! docs/formats/aircraft-control-surfaces.md "F-5E control surfaces (F5E-3,
//! 2026-11-10)".
//!
//! F5E.SH ships six state words: 0x63c0 the twin afterburner plumes, 0x63c6
//! the dorsal speed brake plate, 0x63cc the gear, 0x63d8 and 0x63de the left
//! and right flap words (each deletes that wing's trailing-edge flap strip and
//! adds nothing) and 0x63e4 the rudder (swaps the rudder quad for a copy about
//! 40 degrees to starboard). The rig never sets the flap words or the rudder
//! word: it keeps the neutral flap strips and the neutral rudder drawn and
//! turns them, so a flap never vanishes in flight. The stabilators and the
//! ailerons have no state word at all (free flat plates and wing strips), so
//! their roles are fitted whole-panel assignments over the reviewed source
//! faces. There is no slat, canopy, nozzle, hook or gear-door word and none is
//! invented.
//!
//! Gear, speed brake and plume (F5E-4) are not owned by this rig yet: their
//! faces fall through to the shared fitted device groups in
//! `variety_animation` (the interim gear law). The reviewed face lists and
//! the word pins below are the place F5E-4 takes them over.
//!
//! The F-5E draws at its own size (F5E-2: the identity axis scale, size still
//! John's question), so the hinge points below are plain shape units; `sp`
//! still routes them through `scale_point` so a later scale decision cannot
//! detach a hinge.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};

const ID: AircraftId = AircraftId::F5E;
const WORDS: [usize; 6] = [0x63c0, 0x63c6, 0x63cc, 0x63d8, 0x63de, 0x63e4];

/// Stabilators: one flat 4-vertex plate per side at z -4, both windings
/// (`3b8c` and `40eb` are the back windings), sharing no vertex with any other
/// face. Right root x 5 (y -23 to -37), tip x 20 (y -30 to -35).
pub(crate) const STAB_R: [usize; 2] = [0x3a9d, 0x3b8c];
pub(crate) const STAB_L: [usize; 2] = [0x3f2c, 0x40eb];
/// Ailerons: the wing strip x 31 to 43, hinge y -5 (top layer z -3, bottom
/// layer z -4), trailing edge y -8 at the seam and y -7 at the tip, z -4. One
/// face per layer.
pub(crate) const AILERON_R: [usize; 2] = [0x3b45, 0x3b9f];
pub(crate) const AILERON_L: [usize; 2] = [0x4067, 0x41ce];
/// Flaps: the wing strip x 7 to 31 (left -7 to -31), hinge y -4 at the root
/// and -5 at the seam, trailing edge y -11 at the root and -8 at the seam.
/// Flap word 0x63de deletes the right pair and 0x63d8 the left pair.
pub(crate) const FLAP_R: [usize; 2] = [0x44d9, 0x44f8];
pub(crate) const FLAP_L: [usize; 2] = [0x45c0, 0x45df];
/// Rudder: the hinged quad, both windings. The rudder word 0x63e4 swaps it for
/// the two faces in [`RUDDER_WORD`] (never drawn).
pub(crate) const RUDDER: [usize; 2] = [0x4755, 0x4774];
const RUDDER_WORD: [usize; 2] = [0x469b, 0x46ba];

/// F5E-4 places (pinned at load, not animated here): gear word 0x63cc adds
/// three flat plates (main plates at x +14 and -15, nose plate at x 0), the
/// speed brake word 0x63c6 adds one dorsal plate and the plume word 0x63c0 adds
/// eight plume plates. Each adds faces and removes none.
const GEAR: [usize; 6] = [0x49cc, 0x49eb, 0x4a37, 0x4a56, 0x4aa2, 0x4ac1];
const BRAKE: [usize; 2] = [0x4802, 0x4819];
const FLAME: [usize; 8] = [
    0x48a7, 0x48c6, 0x48e5, 0x4904, 0x4923, 0x4942, 0x4961, 0x4980,
];

/// Stabilator spindle: a lateral line (along X) through the root chord
/// midpoint, (+/-5, -30, -4). The plate is flat (z -4 everywhere), so its own
/// anhedral is zero and the spindle is not tilted (the taileron standard,
/// docs/formats/aircraft-control-surfaces.md).
const STAB_SPINDLE_R: [f32; 3] = [5., -30., -4.];
const STAB_SPINDLE_L: [f32; 3] = [-5., -30., -4.];
/// Aileron hinge: the straight line y -5 along X at the mid height of the
/// strip's hinge vertices (z -3 and -4), the same wedge axis as the Mirage
/// elevons.
const AILERON_HINGE_R: [f32; 3] = [31., -5., -3.5];
const AILERON_HINGE_L: [f32; 3] = [-31., -5., -3.5];
const AILERON_AXIS: [f32; 3] = [1., 0., 0.];
/// Flap hinge: the line from the root (7, -4) to the seam (31, -5) at z -3.5,
/// so it is a fraction of a degree off the aileron's. Each axis points
/// outboard-to-inboard-agnostic: both are written pointing to +X so a positive
/// angle lowers the trailing edge on both sides.
const FLAP_HINGE_R: [f32; 3] = [7., -4., -3.5];
const FLAP_AXIS_R: [f32; 3] = [24., -1., 0.];
const FLAP_HINGE_L: [f32; 3] = [-7., -4., -3.5];
const FLAP_AXIS_L: [f32; 3] = [24., 1., 0.];
/// Rudder hinge edge, the fin's trailing edge: from (0,-31,4) up to (0,-29,22).
/// A positive angle swings the trailing edge toward +X (right), as the fleet's
/// rudders.
const RUDDER_HINGE: [f32; 3] = [0., -31., 4.];
const RUDDER_AXIS: [f32; 3] = [0., 2., 18.];

/// Fitted magnitudes in radians, all agent decisions (2026-11-10) matching the
/// fleet: the F-4 and F-104 stabilators take 0.30 pitch and no roll (the real
/// F-5E uses its stabilators for pitch only and its ailerons for roll, John,
/// 2026-11-10: "stabilators pitch only, ailerons roll"), the F-4, F-16 and
/// F-104 roll surfaces take 0.20, the Mirage and MiG-31 trailing-edge surfaces
/// droop 0.40 with the flap lever, and the fleet's rudders swing 0.35.
pub(crate) const STAB_PITCH: f64 = 0.30;
pub(crate) const AILERON_ROLL: f64 = 0.20;
pub(crate) const FLAP_DROOP: f64 = 0.40;
pub(crate) const RUDDER_TRAVEL: f64 = 0.35;

pub(crate) fn stab_right(address: usize) -> bool {
    STAB_R.contains(&address)
}
pub(crate) fn stab_left(address: usize) -> bool {
    STAB_L.contains(&address)
}
pub(crate) fn aileron_right(address: usize) -> bool {
    AILERON_R.contains(&address)
}
pub(crate) fn aileron_left(address: usize) -> bool {
    AILERON_L.contains(&address)
}
pub(crate) fn flap_right(address: usize) -> bool {
    FLAP_R.contains(&address)
}
pub(crate) fn flap_left(address: usize) -> bool {
    FLAP_L.contains(&address)
}
pub(crate) fn rudder(address: usize) -> bool {
    RUDDER.contains(&address)
}

/// One stabilator angle in radians, positive lowering the trailing edge. Both
/// panels take the same angle (pitch only, no roll term): a positive elevator
/// raises both trailing edges.
pub(crate) fn stab_angle(state: &State) -> f64 {
    -STAB_PITCH * state.elevator.clamp(-1., 1.)
}

/// One aileron angle in radians, positive lowering the trailing edge. A
/// positive aileron raises the right trailing edge and lowers the left. The
/// ailerons take no pitch and no flap droop.
pub(crate) fn aileron_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    -side * AILERON_ROLL * state.aileron.clamp(-1., 1.)
}

/// The flap droop in radians, positive lowering the trailing edge, for both
/// sides: the flap lever from 0 to 1.
pub(crate) fn flap_angle(state: &State) -> f64 {
    FLAP_DROOP * state.flaps.clamp(0., 1.)
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
        .ok_or_else(|| format!("F5E.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("F5E.SH duplicate reviewed face {address:x}").into());
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
        return Err(format!("unreviewed F5E.SH branch {word:x}").into());
    }
    Ok(())
}

/// The reviewed right-hand plates as (right face, left face, corners in shape
/// units). The left face is the mirror image in X: the same corners with the
/// sign of x reversed. Each face is one whole four-corner plate.
const PLATES: [(usize, usize, [[f32; 3]; 4]); 8] = [
    (
        0x3a9d,
        0x3f2c,
        [
            [5., -23., -4.],
            [20., -30., -4.],
            [20., -35., -4.],
            [5., -37., -4.],
        ],
    ),
    (
        0x3b8c,
        0x40eb,
        [
            [5., -23., -4.],
            [20., -30., -4.],
            [20., -35., -4.],
            [5., -37., -4.],
        ],
    ),
    (
        0x3b45,
        0x4067,
        [
            [31., -5., -3.],
            [43., -5., -3.],
            [43., -7., -4.],
            [31., -8., -4.],
        ],
    ),
    (
        0x3b9f,
        0x41ce,
        [
            [31., -8., -4.],
            [43., -7., -4.],
            [43., -5., -4.],
            [31., -5., -4.],
        ],
    ),
    (
        0x44d9,
        0x45c0,
        [
            [7., -4., -3.],
            [31., -5., -3.],
            [31., -8., -4.],
            [7., -11., -4.],
        ],
    ),
    (
        0x44f8,
        0x45df,
        [
            [7., -4., -4.],
            [7., -11., -4.],
            [31., -8., -4.],
            [31., -5., -4.],
        ],
    ),
    // The rudder is on the centreline: both entries name its two windings.
    (
        0x4755,
        0x4755,
        [
            [0., -31., 4.],
            [0., -29., 22.],
            [0., -32., 22.],
            [0., -37., 4.],
        ],
    ),
    (
        0x4774,
        0x4774,
        [
            [0., -31., 4.],
            [0., -29., 22.],
            [0., -32., 22.],
            [0., -37., 4.],
        ],
    ),
];

fn plate(shape: &Shape, address: usize, corners: &[[f32; 3]; 4], mirror: bool) -> AppResult<()> {
    let face = one(shape, address)?;
    let at = |q: [f32; 3]| sp(if mirror { [-q[0], q[1], q[2]] } else { q });
    if face.positions.len() != 4
        || !face
            .positions
            .iter()
            .all(|&p| corners.iter().any(|&q| p == at(q)))
        || !corners.iter().all(|&q| face.positions.contains(&at(q)))
    {
        return Err(format!("unreviewed F5E.SH surface face {address:x}").into());
    }
    Ok(())
}

pub struct Rig;
impl Rig {
    /// Checks the parsed neutral shape and the state words against the reviewed
    /// source. The shape is not changed: every surface is a neutral face the
    /// rig turns, and the device words stay with the shared groups.
    pub fn load(bytes: &[u8], shape: &Shape) -> AppResult<Self> {
        if tore_formats::module::code(bytes)?.0.len() != 21488
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F5E.SH layout".into());
        }
        for (right, left, corners) in &PLATES {
            plate(shape, *right, corners, false)?;
            if left != right {
                plate(shape, *left, corners, true)?;
            }
        }
        // Rudder windings: the swap word's copy is never drawn.
        let neutral: BTreeSet<usize> = shape
            .faces
            .iter()
            .map(|f| f.address)
            .filter(|a| !GEAR.contains(a) && !BRAKE.contains(a) && !FLAME.contains(a))
            .collect();
        // The flap words remove exactly that wing's strip and add nothing.
        // They pin the strips as the surfaces the source treats as flaps; the
        // rig never sets them.
        branch(bytes, &neutral, 0x63d8, &[], &FLAP_L)?;
        branch(bytes, &neutral, 0x63de, &[], &FLAP_R)?;
        // The rudder word swaps the quad for a copy offset toward +X.
        branch(bytes, &neutral, 0x63e4, &RUDDER_WORD, &RUDDER)?;
        // F5E-4 places: the device words add exactly the reviewed plates.
        branch(bytes, &neutral, 0x63cc, &GEAR, &[])?;
        branch(bytes, &neutral, 0x63c6, &BRAKE, &[])?;
        branch(bytes, &neutral, 0x63c0, &FLAME, &[])?;
        Ok(Self)
    }

    /// True for the faces this rig turns; every other face is handled by the
    /// shared groups.
    pub fn owns(&self, address: usize) -> bool {
        stab_right(address)
            || stab_left(address)
            || aileron_right(address)
            || aileron_left(address)
            || flap_right(address)
            || flap_left(address)
            || rudder(address)
    }

    /// The face turned for the pilot's commands. Each surface is one rigid
    /// panel (both windings turn together, no vertex held off its hinge), so
    /// nothing folds.
    pub fn animate(&self, source: &Face, state: &State) -> Face {
        let a = source.address;
        let mut result = source.clone();
        let (pivot, axis, angle) = if stab_right(a) {
            (STAB_SPINDLE_R, [1., 0., 0.], stab_angle(state))
        } else if stab_left(a) {
            (STAB_SPINDLE_L, [1., 0., 0.], stab_angle(state))
        } else if aileron_right(a) {
            (AILERON_HINGE_R, AILERON_AXIS, aileron_angle(true, state))
        } else if aileron_left(a) {
            (AILERON_HINGE_L, AILERON_AXIS, aileron_angle(false, state))
        } else if flap_right(a) {
            (FLAP_HINGE_R, FLAP_AXIS_R, flap_angle(state))
        } else if flap_left(a) {
            (FLAP_HINGE_L, FLAP_AXIS_L, flap_angle(state))
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
    fn plate_face(right: usize, left: usize, mirror: bool) -> Face {
        let (address, corners) = PLATES
            .iter()
            .find(|(r, l, _)| *r == right && *l == left)
            .map(|(r, l, c)| (if mirror { *l } else { *r }, *c))
            .unwrap();
        let points: Vec<[f32; 3]> = corners
            .iter()
            .map(|q| if mirror { [-q[0], q[1], q[2]] } else { *q })
            .collect();
        face(address, &points)
    }
    fn spread(a: &Face, b: &Face) -> f32 {
        let d =
            |p: [f32; 3], q: [f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt();
        let mut worst = 0f32;
        for i in 0..a.positions.len() {
            for j in i + 1..a.positions.len() {
                worst = worst.max(
                    (d(a.positions[i], a.positions[j]) - d(b.positions[i], b.positions[j])).abs(),
                );
            }
        }
        worst
    }
    #[test]
    fn neutral_commands_leave_every_surface_exactly_where_it_is() {
        let rig = Rig;
        let s = state();
        for (right, left, _) in &PLATES {
            for mirror in [false, true] {
                let f = plate_face(*right, *left, mirror);
                assert_eq!(rig.animate(&f, &s).positions, f.positions);
            }
        }
    }
    #[test]
    fn stabilators_turn_whole_for_pitch_only_and_ignore_roll() {
        let rig = Rig;
        let mut s = state();
        s.aileron = 1.;
        s.rudder = 1.;
        s.flaps = 1.;
        for (right, left) in [(0x3a9d, 0x3f2c), (0x3b8c, 0x40eb)] {
            for mirror in [false, true] {
                let f = plate_face(right, left, mirror);
                assert_eq!(rig.animate(&f, &s).positions, f.positions, "roll moved it");
            }
        }
        for elevator in [-1., -0.5, 0.5, 1.] {
            s.elevator = elevator;
            let a = plate_face(0x3a9d, 0x3f2c, false);
            let b = plate_face(0x3a9d, 0x3f2c, true);
            let (ta, tb) = (rig.animate(&a, &s), rig.animate(&b, &s));
            assert!(
                spread(&a, &ta) < 1e-3 && spread(&b, &tb) < 1e-3,
                "not rigid"
            );
            // Both sides take the same angle: the trailing tip moves the same
            // way, and the root chord midpoint (+/-5, -30, -4) stays put.
            assert!((ta.positions[3][2] - tb.positions[3][2]).abs() < 1e-4);
            // A positive elevator raises the trailing edge.
            assert_eq!(ta.positions[3][2] > a.positions[3][2], elevator > 0.);
            let mid = |f: &Face| {
                [
                    (f.positions[0][0] + f.positions[3][0]) / 2.,
                    (f.positions[0][1] + f.positions[3][1]) / 2.,
                    (f.positions[0][2] + f.positions[3][2]) / 2.,
                ]
            };
            for k in 0..3 {
                assert!((mid(&ta)[k] - mid(&a)[k]).abs() < 1e-4);
            }
        }
    }
    #[test]
    fn ailerons_roll_oppositely_and_ignore_pitch_and_flaps() {
        let rig = Rig;
        let mut s = state();
        s.elevator = 1.;
        s.flaps = 1.;
        for (right, left) in [(0x3b45, 0x4067), (0x3b9f, 0x41ce)] {
            for mirror in [false, true] {
                let f = plate_face(right, left, mirror);
                assert_eq!(rig.animate(&f, &s).positions, f.positions, "pitch moved it");
            }
        }
        s.aileron = 1.;
        let r = plate_face(0x3b9f, 0x41ce, false);
        let l = plate_face(0x3b9f, 0x41ce, true);
        let (tr, tl) = (rig.animate(&r, &s), rig.animate(&l, &s));
        // Trailing vertex (31, -8, -4) / (-31, -8, -4) is the first of the face.
        assert!(tr.positions[0][2] > r.positions[0][2], "right goes up");
        assert!(tl.positions[0][2] < l.positions[0][2], "left goes down");
        assert!(spread(&r, &tr) < 1e-3 && spread(&l, &tl) < 1e-3);
    }
    #[test]
    fn flaps_droop_both_sides_together_and_keep_their_hinge_line() {
        let rig = Rig;
        let mut s = state();
        s.flaps = 1.;
        let r = plate_face(0x44f8, 0x45df, false);
        let l = plate_face(0x44f8, 0x45df, true);
        let (tr, tl) = (rig.animate(&r, &s), rig.animate(&l, &s));
        // The trailing vertex (31, -8, -4) drops; so does (-31, -8, -4).
        assert!(tr.positions[2][2] < r.positions[2][2]);
        assert!(tl.positions[2][2] < l.positions[2][2]);
        assert!((tr.positions[2][2] - tl.positions[2][2]).abs() < 1e-4);
        assert!(spread(&r, &tr) < 1e-3 && spread(&l, &tl) < 1e-3);
        // The hinge pair's midpoint at the root, (7, -4, -3.5), stays put.
        let (a, b) = (rig.animate(&plate_face(0x44d9, 0x45c0, false), &s), tr);
        let mid = [
            (a.positions[0][0] + b.positions[0][0]) / 2.,
            (a.positions[0][1] + b.positions[0][1]) / 2.,
            (a.positions[0][2] + b.positions[0][2]) / 2.,
        ];
        for (got, want) in mid.iter().zip([7., -4., -3.5]) {
            assert!((got - want).abs() < 1e-4, "hinge midpoint moved");
        }
    }
    #[test]
    fn the_rudder_turns_whole_about_its_hinge_edge() {
        let rig = Rig;
        let mut s = state();
        for rudder in [-1., -0.5, 0.5, 1.] {
            s.rudder = rudder;
            for address in RUDDER {
                let f = plate_face(address, address, false);
                let t = rig.animate(&f, &s);
                assert!(spread(&f, &t) < 1e-3, "not rigid");
                // Hinge vertices (0,-31,4) and (0,-29,22) stay; the trailing
                // vertices swing to +X for a positive command.
                for (i, hinge) in [(0, true), (1, true), (2, false), (3, false)] {
                    let moved = (0..3)
                        .map(|k| (t.positions[i][k] - f.positions[i][k]).powi(2))
                        .sum::<f32>()
                        .sqrt();
                    if hinge {
                        assert!(moved < 1e-3, "hinge vertex moved {moved}");
                    } else {
                        assert!(
                            (t.positions[i][0] > 0.) == (rudder > 0.),
                            "swung the wrong way"
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn the_rig_owns_exactly_the_surface_faces() {
        let rig = Rig;
        for (right, left, _) in &PLATES {
            assert!(rig.owns(*right) && rig.owns(*left));
        }
        for a in GEAR.iter().chain(&BRAKE).chain(&FLAME) {
            assert!(!rig.owns(*a));
        }
    }
}
