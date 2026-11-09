//! Mirage 2000 (`M20.SH`) own-source control surfaces: four delta elevons
//! (inboard and outboard strip per side, one angle per side) taking pitch, a
//! differential roll and a flap droop, and the single-fin rudder with its small
//! tip triangles. The gear keeps the interim fitted rise until M2K-4.
//! See docs/spec/variety-animation.md "Mirage 2000 control fit".
//!
//! M20.SH ships six state words: 0x5890 the plume, 0x5896 the airbrake, 0x589c
//! the gear, 0x58a8 and 0x58ae the left and right flap words (each deletes the
//! inboard elevon strip and replaces it with nothing) and 0x58b4 the rudder
//! (swaps the rudder quad for a copy offset 1 to 2 units in X). The rig never
//! sets the flap words or the rudder word: it keeps the neutral strips and the
//! neutral rudder drawn and turns them, so the inboard elevon never vanishes
//! in flight. There is no signed pitch or roll branch, so every control role is
//! a fitted whole-panel assignment over the reviewed source faces.
//!
//! The shape arrives already scaled per axis (M2K-2). Every hinge point named
//! here is in shape units and is scaled with `scale_point`, so a panel turned
//! about it stays rigid.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use tore_formats::{
    aircraft::{AircraftId, scale::scale_point},
    shape::{Face, Shape},
};

const WORDS: [usize; 6] = [0x5890, 0x5896, 0x589c, 0x58a8, 0x58ae, 0x58b4];
const ID: AircraftId = AircraftId::M2000;

/// Right elevon: inboard strip (x 3 to 17, two quads) and outboard strip (x 17
/// to 33, four triangles), the two layers of one thin trailing wedge.
const ELEVON_R: [usize; 6] = [0x3969, 0x394a, 0x2176, 0x2391, 0x2190, 0x2233];
/// Left elevon: inboard strip (x -18 to -3) and outboard strip (x -33 to -18).
const ELEVON_L: [usize; 6] = [0x3a31, 0x3a50, 0x1e87, 0x23c3, 0x1ea1, 0x1f1b];
/// The inboard strips are exactly the faces the flap words delete.
const INBOARD_R: [usize; 2] = [0x394a, 0x3969];
const INBOARD_L: [usize; 2] = [0x3a31, 0x3a50];
/// Rudder: the hinged quad (both windings) and the two small triangles that
/// fill its top corner up to the fin cap (see the control fit).
const RUDDER: [usize; 4] = [0x3bc6, 0x3be5, 0x2b64, 0x2b7c];
/// Faces the rudder word 0x58b4 swaps in for the quad (never drawn).
const RUDDER_WORD: [usize; 2] = [0x3b0c, 0x3b2b];
/// Gear word 0x589c: three flat plates, both windings.
const GEAR: [usize; 6] = [0x380b, 0x382a, 0x3876, 0x3895, 0x38e1, 0x3900];

/// Elevon hinge axis: the straight line y = -29, z = -3 through the whole
/// span, along X (shape units). The hinge-edge vertices sit 1 unit above and
/// below it; the whole wedge turns about the mid axis.
const ELEVON_HINGE: [f32; 3] = [0., -29., -3.];
const ELEVON_AXIS: [f32; 3] = [1., 0., 0.];
/// Rudder hinge edge, the fin trailing edge: from (0,-35,4) up and aft to
/// (0,-40,19). Direction toward the top, aft and up, so a positive angle swings
/// the trailing edge toward +X (right), as the fleet's rudders.
const RUDDER_HINGE: [f32; 3] = [0., -35., 4.];
const RUDDER_AXIS: [f32; 3] = [0., -5., 15.];

/// Fitted magnitudes in radians, all agent decisions (2026-11-09) matching the
/// fleet: the Rafale's elevons take 0.30 pitch, 0.20 roll and 0.40 flap droop
/// and its rudder 0.35; the MiG-31 uses the same four numbers.
pub(crate) const PITCH: f64 = 0.30;
pub(crate) const ROLL: f64 = 0.20;
pub(crate) const FLAP: f64 = 0.40;
pub(crate) const RUDDER_TRAVEL: f64 = 0.35;

pub(crate) fn elevon_right(address: usize) -> bool {
    ELEVON_R.contains(&address)
}
pub(crate) fn elevon_left(address: usize) -> bool {
    ELEVON_L.contains(&address)
}
pub(crate) fn rudder(address: usize) -> bool {
    RUDDER.contains(&address)
}

/// One elevon angle in radians, positive lowering the trailing edge. Pitch
/// raises both trailing edges for a positive elevator; a positive aileron
/// raises the right trailing edge and lowers the left; pitch plus roll is
/// clamped to the sum of their maxima, then the flap droop lowers both.
pub(crate) fn elevon_angle(right: bool, state: &State) -> f64 {
    let side = if right { 1. } else { -1. };
    let flight =
        -PITCH * state.elevator.clamp(-1., 1.) - side * ROLL * state.aileron.clamp(-1., 1.);
    flight.clamp(-(PITCH + ROLL), PITCH + ROLL) + FLAP * state.flaps.clamp(0., 1.)
}

fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("M20.SH missing reviewed face {address:x}"))?;
    if found.next().is_some() {
        return Err(format!("M20.SH duplicate reviewed face {address:x}").into());
    }
    Ok(result)
}

fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
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
        return Err(format!("unreviewed M20.SH branch {word:x}").into());
    }
    Ok(pose)
}

/// A scaled vertex equals a shape-unit point (the same single `f32` product
/// `Shape::scaled` formed, so the comparison is exact).
fn is(p: [f32; 3], shape_units: [f32; 3]) -> bool {
    p == scale_point(ID, shape_units)
}

/// Every vertex of an elevon face lies on the trailing wedge: the hinge edge
/// (y -29 at z -2, -3 or -4) or the trailing edge (y -32, z -3). This is what
/// makes the mid axis y -29, z -3 a line the whole wedge can turn about.
fn wedge_vertex(p: [f32; 3], x_limits: (f32, f32)) -> bool {
    let lo = scale_point(ID, [x_limits.0, 0., 0.])[0] - 1e-3;
    let hi = scale_point(ID, [x_limits.1, 0., 0.])[0] + 1e-3;
    let at = |y: f32, z: f32| {
        p[1] == scale_point(ID, [0., y, 0.])[1] && p[2] == scale_point(ID, [0., 0., z])[2]
    };
    (lo..=hi).contains(&p[0]) && (at(-29., -2.) || at(-29., -3.) || at(-29., -4.) || at(-32., -3.))
}

pub struct Rig {
    /// The interim fitted gear rise: the whole source group lifts by its own
    /// scaled vertical extent as the gear stows.
    gear_rise: f32,
}
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 18624
            || shape.faces.len() != 206
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed M20.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // Elevons: every face is a reviewed three or four vertex piece of the
        // trailing wedge, and each side's six faces are exactly one wedge.
        for (ids, x_limits) in [(&ELEVON_R, (3., 33.)), (&ELEVON_L, (-33., -3.))] {
            for &address in ids {
                let face = one(&shape, address)?;
                if !matches!(face.positions.len(), 3 | 4)
                    || !face.positions.iter().all(|&p| wedge_vertex(p, x_limits))
                {
                    return Err(format!("unreviewed M20.SH elevon face {address:x}").into());
                }
            }
        }
        // The mid axis passes through the outboard tip vertex, which is the one
        // vertex of the wedge that is on it.
        if !one(&shape, 0x2391)?
            .positions
            .contains(&scale_point(ID, [33., -29., -3.]))
            || !one(&shape, 0x23c3)?
                .positions
                .contains(&scale_point(ID, [-33., -29., -3.]))
        {
            return Err("unreviewed M20.SH elevon tip".into());
        }
        // Rudder: the quad's leading edge is the hinge, and the two triangles
        // share its top trailing corner.
        for &address in &RUDDER {
            let face = one(&shape, address)?;
            let ok = face.positions.iter().all(|&p| {
                [
                    [0., -35., 4.],
                    [0., -40., 19.],
                    [0., -39., 5.],
                    [0., -42., 19.],
                    [0., -42., 21.],
                ]
                .iter()
                .any(|&q| is(p, q))
            });
            let quad = address == 0x3bc6 || address == 0x3be5;
            if !ok
                || !face.positions.contains(&scale_point(ID, [0., -40., 19.]))
                || (quad && !face.positions.contains(&scale_point(ID, [0., -35., 4.])))
            {
                return Err(format!("unreviewed M20.SH rudder face {address:x}").into());
            }
        }
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words remove exactly the inboard strips, 0x58a8 the left and
        // 0x58ae the right, and add nothing. They pin the strips as the
        // surfaces the source treats as flaps; the rig never sets them.
        branch(bytes, &original, 0x58a8, &[], &INBOARD_L)?;
        branch(bytes, &original, 0x58ae, &[], &INBOARD_R)?;
        // The rudder word swaps the quad for a copy offset in +X.
        branch(bytes, &original, 0x58b4, &RUDDER_WORD, &[0x3bc6, 0x3be5])?;
        // The gear word adds exactly the three reviewed plates.
        let gear = branch(bytes, &original, 0x589c, &GEAR, &[])?.scaled(axis);
        let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
        for face in gear.faces.iter().filter(|f| GEAR.contains(&f.address)) {
            for p in &face.positions {
                low = low.min(p[2]);
                high = high.max(p[2]);
            }
        }
        for face in gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)) {
            shape.faces.push(face);
        }
        Ok((
            Self {
                gear_rise: (high - low).max(1.),
            },
            shape,
        ))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if elevon_right(a) || elevon_left(a) {
            // One angle per side, shared by the inboard and outboard strips,
            // turns every vertex of the wedge about the mid axis. Pitch and
            // roll sum, clamp to the two maxima, then the flap droop adds.
            let angle = elevon_angle(elevon_right(a), state);
            turn(
                &mut result,
                scale_point(ID, ELEVON_HINGE),
                scale_point(ID, ELEVON_AXIS),
                angle,
            );
        } else if rudder(a) {
            // The quad and the two tip triangles are one rigid panel about the
            // fin trailing edge; a fixed vertex is kept only on that edge.
            turn(
                &mut result,
                scale_point(ID, RUDDER_HINGE),
                scale_point(ID, RUDDER_AXIS),
                RUDDER_TRAVEL * state.rudder.clamp(-1., 1.),
            );
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            let rise = self.gear_rise * (1. - state.gear.clamp(0., 1.)) as f32;
            for p in &mut result.positions {
                p[2] += rise;
            }
        }
        Some(result)
    }
}
/// The plume word is mapped by M2K-4.
pub(crate) fn flame(_address: usize) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    /// A synthetic face whose vertices are named in shape units and scaled the
    /// way the loader scales the real shape.
    fn face(address: usize, points: &[[f32; 3]]) -> Face {
        let positions: Vec<[f32; 3]> = points.iter().map(|&p| scale_point(ID, p)).collect();
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
    fn rig() -> Rig {
        Rig { gear_rise: 10. }
    }
    fn spread(a: &[[f32; 3]], b: &[[f32; 3]]) -> f32 {
        let d =
            |p: [f32; 3], q: [f32; 3]| (0..3).map(|k| (p[k] - q[k]).powi(2)).sum::<f32>().sqrt();
        let mut worst = 0f32;
        for i in 0..a.len() {
            for j in i + 1..a.len() {
                worst = worst.max((d(a[i], a[j]) - d(b[i], b[j])).abs());
            }
        }
        worst
    }
    /// The elevon wedge, both layers and both strips of the right side, in
    /// shape units: the shared seam vertices are the same bits in every face.
    fn right_wedge() -> Vec<Face> {
        vec![
            face(
                0x3969,
                &[
                    [17., -29., -2.],
                    [17., -32., -3.],
                    [3., -32., -3.],
                    [3., -29., -2.],
                ],
            ),
            face(
                0x394a,
                &[
                    [3., -32., -3.],
                    [17., -32., -3.],
                    [17., -29., -4.],
                    [3., -29., -4.],
                ],
            ),
            face(
                0x2176,
                &[[33., -32., -3.], [17., -32., -3.], [17., -29., -2.]],
            ),
            face(
                0x2391,
                &[[33., -29., -3.], [33., -32., -3.], [17., -29., -2.]],
            ),
            face(
                0x2190,
                &[[33., -29., -3.], [17., -32., -3.], [33., -32., -3.]],
            ),
            face(
                0x2233,
                &[[17., -32., -3.], [33., -29., -3.], [17., -29., -4.]],
            ),
        ]
    }
    #[test]
    fn a_whole_side_turns_as_one_rigid_wedge_with_no_seam_between_strips() {
        let rig = rig();
        let mut s = state();
        let source = right_wedge();
        for (elevator, aileron, flaps) in [
            (1., 0., 0.),
            (-1., 0., 0.),
            (0., 1., 0.),
            (0., -1., 0.),
            (0.5, -1., 1.),
            (-1., 1., 1.),
        ] {
            s.elevator = elevator;
            s.aileron = aileron;
            s.flaps = flaps;
            let turned: Vec<Face> = source.iter().map(|f| rig.animate(f, &s).unwrap()).collect();
            let before: Vec<[f32; 3]> = source.iter().flat_map(|f| f.positions.clone()).collect();
            let after: Vec<[f32; 3]> = turned.iter().flat_map(|f| f.positions.clone()).collect();
            assert!(
                spread(&before, &after) < 1e-3,
                "{elevator}/{aileron}/{flaps}: not rigid"
            );
            // The seam vertex (17,-32,-3) is one point in all four faces that
            // hold it, so a shared vertex cannot open between strips.
            let seam = scale_point(ID, [17., -32., -3.]);
            let moved: Vec<[f32; 3]> = source
                .iter()
                .zip(&turned)
                .flat_map(|(a, b)| {
                    a.positions
                        .iter()
                        .zip(&b.positions)
                        .filter(|(p, _)| **p == seam)
                        .map(|(_, q)| *q)
                })
                .collect();
            assert!(
                moved.len() >= 4 && moved.iter().all(|q| *q == moved[0]),
                "seam opened"
            );
        }
    }
    #[test]
    fn the_mid_axis_stays_put_and_the_hinge_pair_keeps_its_midpoint() {
        let rig = rig();
        let mut s = state();
        s.elevator = 1.;
        s.flaps = 0.5;
        let pair = [
            scale_point(ID, [17., -29., -2.]),
            scale_point(ID, [17., -29., -4.]),
        ];
        let source = face(
            0x3969,
            &[[17., -29., -2.], [17., -29., -4.], [33., -29., -3.]],
        );
        let out = rig.animate(&source, &s).unwrap();
        let mid =
            |a: [f32; 3], b: [f32; 3]| std::array::from_fn::<f32, 3, _>(|k| (a[k] + b[k]) / 2.);
        let (was, now) = (
            mid(pair[0], pair[1]),
            mid(out.positions[0], out.positions[1]),
        );
        for k in 0..3 {
            assert!(
                (was[k] - now[k]).abs() < 1e-4,
                "pair midpoint moved on axis {k}"
            );
        }
        // The tip vertex is on the mid axis.
        let tip = scale_point(ID, [33., -29., -3.]);
        for (got, want) in out.positions[2].iter().zip(tip) {
            assert!((got - want).abs() < 1e-4, "axis vertex moved");
        }
        // The off-axis hinge vertices move by less than their offset times
        // the angle: that is the accepted wedge opening.
        let angle = elevon_angle(true, &s).abs() as f32;
        let offset = (scale_point(ID, [0., 0., 1.])[2]) * angle;
        let moved = (0..3)
            .map(|k| (out.positions[0][k] - pair[0][k]).powi(2))
            .sum::<f32>()
            .sqrt();
        assert!(moved <= offset * 1.01, "{moved} against {offset}");
    }
    #[test]
    fn elevons_pitch_together_roll_oppositely_and_droop_with_flaps() {
        let mut s = state();
        let trailing = |s: &State, right: bool| {
            let (address, edge) = if right {
                (0x2176, [33., -32., -3.])
            } else {
                (0x1e87, [-33., -32., -3.])
            };
            let out = rig().animate(&face(address, &[edge]), s).unwrap();
            out.positions[0][2] - scale_point(ID, edge)[2]
        };
        s.elevator = 1.;
        assert!(
            trailing(&s, true) > 0.01 && trailing(&s, false) > 0.01,
            "pull raises both trailing edges"
        );
        s.elevator = -1.;
        assert!(trailing(&s, true) < -0.01 && trailing(&s, false) < -0.01);
        s.elevator = 0.;
        s.aileron = 1.;
        assert!(
            trailing(&s, true) > 0.01 && trailing(&s, false) < -0.01,
            "positive aileron: right up, left down"
        );
        s.aileron = 0.;
        s.flaps = 1.;
        assert!(
            trailing(&s, true) < -0.01 && trailing(&s, false) < -0.01,
            "flaps lower both"
        );
        s.flaps = 0.;
        assert_eq!(trailing(&s, true), 0.);
    }
    #[test]
    fn pitch_plus_roll_is_clamped_to_the_sum_of_the_maxima() {
        let mut s = state();
        s.elevator = 1.;
        s.aileron = 1.;
        // Right: pull raises, positive aileron raises. Left: pull raises, roll lowers.
        assert!((elevon_angle(true, &s) + (PITCH + ROLL)).abs() < 1e-9);
        assert!((elevon_angle(false, &s) + (PITCH - ROLL)).abs() < 1e-9);
        s.elevator = 5.;
        s.aileron = 5.;
        assert!(elevon_angle(true, &s) >= -(PITCH + ROLL) - 1e-9);
        s.flaps = 1.;
        assert!((elevon_angle(true, &s) - (FLAP - PITCH - ROLL)).abs() < 1e-9);
        s.elevator = -1.;
        s.aileron = -1.;
        assert!((elevon_angle(true, &s) - (FLAP + PITCH + ROLL)).abs() < 1e-9);
    }
    #[test]
    fn the_inboard_strip_never_vanishes() {
        // Whatever the flap lever says, the strips the source flap words delete
        // are animated, never dropped.
        let mut s = state();
        for flaps in [0., 0.5, 1.] {
            s.flaps = flaps;
            for a in INBOARD_R.into_iter().chain(INBOARD_L) {
                assert!(rig().animate(&face(a, &[[3., -29., -2.]]), &s).is_some());
            }
        }
    }
    #[test]
    fn the_rudder_panel_with_its_tip_triangles_is_rigid_and_keeps_its_hinge_edge() {
        let mut s = state();
        let panel = [
            face(
                0x3bc6,
                &[
                    [0., -39., 5.],
                    [0., -35., 4.],
                    [0., -40., 19.],
                    [0., -42., 19.],
                ],
            ),
            face(
                0x3be5,
                &[
                    [0., -42., 19.],
                    [0., -40., 19.],
                    [0., -35., 4.],
                    [0., -39., 5.],
                ],
            ),
            face(0x2b64, &[[0., -42., 21.], [0., -42., 19.], [0., -40., 19.]]),
            face(0x2b7c, &[[0., -40., 19.], [0., -42., 19.], [0., -42., 21.]]),
        ];
        for rudder in [-1., -0.5, 0.5, 1.] {
            s.rudder = rudder;
            let turned: Vec<Face> = panel
                .iter()
                .map(|f| rig().animate(f, &s).unwrap())
                .collect();
            let before: Vec<[f32; 3]> = panel.iter().flat_map(|f| f.positions.clone()).collect();
            let after: Vec<[f32; 3]> = turned.iter().flat_map(|f| f.positions.clone()).collect();
            assert!(spread(&before, &after) < 1e-3, "rudder {rudder}: not rigid");
            // The hinge edge vertices stay where the fixed fin holds them.
            for edge in [[0., -35., 4.], [0., -40., 19.]] {
                let p = scale_point(ID, edge);
                for (a, b) in panel.iter().zip(&turned) {
                    for (q, r) in a.positions.iter().zip(&b.positions) {
                        if *q == p {
                            assert!(
                                (0..3).all(|k| (r[k] - p[k]).abs() < 1e-3),
                                "hinge vertex moved"
                            );
                        }
                    }
                }
            }
            // A positive command swings the trailing edge to +X.
            let trailing = turned[0].positions[3][0];
            assert!(trailing * rudder as f32 > 0., "wrong sense at {rudder}");
        }
        s.rudder = 0.;
        let out = rig().animate(&panel[0], &s).unwrap();
        assert_eq!(out.positions, panel[0].positions);
    }
    #[test]
    fn the_gear_plates_hide_at_zero_and_rise_rigidly_as_they_stow() {
        let rig = rig();
        let mut s = state();
        let plate = face(
            0x380b,
            &[
                [7., -12., -12.],
                [7., -18., -12.],
                [7., -18., -3.],
                [7., -12., -3.],
            ],
        );
        s.gear = 0.;
        assert!(rig.animate(&plate, &s).is_none());
        s.gear = 1.;
        assert_eq!(rig.animate(&plate, &s).unwrap().positions, plate.positions);
        s.gear = 0.5;
        let half = rig.animate(&plate, &s).unwrap();
        assert!((half.positions[0][2] - plate.positions[0][2] - 5.).abs() < 1e-4);
        assert!(spread(&plate.positions, &half.positions) < 1e-4);
    }
}
