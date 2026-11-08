//! MiG31 own-source control surfaces (all-moving stabilators, outboard
//! ailerons, inboard flaps and twin rudders) plus the interim fitted gear rise.
//! See docs/spec/variety-animation.md "MiG-31 control fit".
//! MIG31.SH ships one rudder branch (0x6144) and two flap-removal branches
//! (0x6138, 0x613e) but no signed pitch or roll branch, so every control role is
//! a fitted geometric assignment over exact reviewed source panels. The six
//! state words follow the shared base order flame, brake, gear, flap, flap, then
//! the rudder: 0x6120 the plume, 0x6126 the brake, 0x612c the gear, 0x6138 and
//! 0x613e the left and right flap pairs, and 0x6144 the twin rudders.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use tore_formats::shape::{Face, Shape};
const WORDS: [usize; 6] = [0x6120, 0x6126, 0x612c, 0x6138, 0x613e, 0x6144];
/// All-moving horizontal stabilators: both windings of one flat plate per side.
const STABILATOR_R: [usize; 2] = [0x3d69, 0x3db1];
const STABILATOR_L: [usize; 2] = [0x39f6, 0x3a3e];
/// Outboard trailing control: one whole source quad per side.
const AILERON_R: [usize; 2] = [0x3ae3, 0x3c64];
const AILERON_L: [usize; 2] = [0x3893, 0x3925];
/// Inboard trailing control: exactly the pair the source flap words remove.
const FLAP_R: [usize; 2] = [0x41a6, 0x41c5];
const FLAP_L: [usize; 2] = [0x4260, 0x427f];
/// Rear fin quad the rudder word 0x6144 replaces.
const RUDDER_R: [usize; 2] = [0x448e, 0x44ad];
const RUDDER_L: [usize; 2] = [0x44eb, 0x450a];
/// The source's own deflected rudder pose (word 0x6144 set): the same two panels
/// per fin, trailing edge swung outboard on both sides.
const RUDDER_DEFLECTED: [usize; 4] = [0x431a, 0x4339, 0x4377, 0x4396];
/// Gear word 0x612c: a strut plate and a painted wheel card per main side, and a
/// centre strut with its cross plate at the nose.
const GEAR: [usize; 12] = [
    0x3eda, 0x3ef1, 0x3f35, 0x3f5c, 0x3fe4, 0x3ffb, 0x403f, 0x4066, 0x40ee, 0x4105, 0x4149, 0x4168,
];
/// Fitted interim rise law. M31-4 owns the rigid gear assemblies; until then the
/// whole gear group rises through its source vertical extent as it stows, the
/// shared fitted device law the other variety aircraft use.
const GEAR_RISE: f32 = 20.;
fn one(shape: &Shape, address: usize) -> AppResult<&Face> {
    let mut found = shape.faces.iter().filter(|f| f.address == address);
    let result = found
        .next()
        .ok_or_else(|| format!("MIG31.SH missing reviewed face{address:x}"))?;
    if found.next().is_some() {
        return Err(format!("MIG31.SH duplicate reviewed face{address:x}").into());
    }
    Ok(result)
}
/// A panel's hinge corners are covered by the union of its faces. Each control
/// surface here is one double-sided source quad, so the hinge edge is shared
/// with the fixed panel and those corners never move.
fn roots(shape: &Shape, addresses: &[usize], points: &[[f32; 3]]) -> AppResult<()> {
    let faces: Vec<&Face> = addresses
        .iter()
        .map(|&address| one(shape, address))
        .collect::<AppResult<_>>()?;
    for p in points {
        if !faces.iter().any(|f| f.positions.contains(p)) {
            return Err(format!("unreviewed MIG31.SH hinge corner {p:?}").into());
        }
    }
    Ok(())
}
fn branch(
    bytes: &[u8],
    neutral: &BTreeSet<usize>,
    word: usize,
    value: i32,
    added: &[usize],
    removed: &[usize],
) -> AppResult<Shape> {
    let pose = Shape::with_state(bytes, &[(word, value)].into())?;
    let active: BTreeSet<_> = pose.faces.iter().map(|f| f.address).collect();
    if active.difference(neutral).copied().collect::<BTreeSet<_>>()
        != added.iter().copied().collect()
        || neutral
            .difference(&active)
            .copied()
            .collect::<BTreeSet<_>>()
            != removed.iter().copied().collect()
    {
        return Err(format!("unreviewed MIG31.SH branch{word:x}={value}").into());
    }
    Ok(pose)
}
/// Source hinge point, direction and fitted angle sign, one entry per moving
/// side. The aileron, flap and rudder lines pass through the reviewed source
/// corners the fixed panel shares, so those corners do not move. The all-moving
/// stabilator has no fixed source corner, so its line passes through the root
/// chord midpoint instead.
fn hinge(address: usize) -> ([f32; 3], [f32; 3], f64) {
    if STABILATOR_R.contains(&address) {
        ([17., -40., -3.], [1., 0., 0.], -0.30)
    } else if STABILATOR_L.contains(&address) {
        ([-17., -40., -3.], [1., 0., 0.], -0.30)
    } else if AILERON_R.contains(&address) {
        ([45., -16., 2.], [36., 7., -3.], -0.20)
    } else if AILERON_L.contains(&address) {
        ([-45., -16., 2.], [-36., 7., -3.], -0.20)
    } else if FLAP_R.contains(&address) {
        ([17., -5., 4.], [28., -11., -2.], 0.40)
    } else if FLAP_L.contains(&address) {
        ([-17., -5., 4.], [-28., -11., -2.], -0.40)
    } else if RUDDER_R.contains(&address) {
        ([13., -42., 8.], [3., -9., 23.], 0.35)
    } else {
        ([-13., -42., 8.], [-3., -9., 23.], 0.35)
    }
}
fn is_stabilator(a: usize) -> bool {
    STABILATOR_R.contains(&a) || STABILATOR_L.contains(&a)
}
fn is_aileron(a: usize) -> bool {
    AILERON_R.contains(&a) || AILERON_L.contains(&a)
}
fn is_flap(a: usize) -> bool {
    FLAP_R.contains(&a) || FLAP_L.contains(&a)
}
fn is_rudder(a: usize) -> bool {
    RUDDER_R.contains(&a) || RUDDER_L.contains(&a)
}
/// The interim fitted gear rise: the whole source group translates up by its own
/// vertical extent as the gear stows, and is hidden at zero. M31-4 replaces it
/// with reviewed rigid assemblies and doors.
fn gear_positions(f: &mut Face, gear: f64) {
    let rise = GEAR_RISE * (1. - gear.clamp(0., 1.)) as f32;
    for p in &mut f.positions {
        p[2] += rise;
    }
}
pub struct Rig;
impl Rig {
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 20816
            || shape.faces.len() != 214
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed MIG31.SH layout".into());
        }
        // Every moving panel is an exact reviewed source group. The hinge corners
        // are the vertices the fixed panel also shares, except the all-moving
        // stabilator, whose root chord is checked here and whose midpoint the
        // probe uses as its invariant.
        roots(&shape, &STABILATOR_R, &[[17., -24., -3.], [17., -56., -3.]])?;
        roots(
            &shape,
            &STABILATOR_L,
            &[[-17., -24., -3.], [-17., -56., -3.]],
        )?;
        roots(&shape, &AILERON_R, &[[45., -16., 2.], [81., -9., -1.]])?;
        roots(&shape, &AILERON_L, &[[-45., -16., 2.], [-81., -9., -1.]])?;
        roots(&shape, &FLAP_R, &[[17., -5., 4.], [45., -16., 2.]])?;
        roots(&shape, &FLAP_L, &[[-17., -5., 4.], [-45., -16., 2.]])?;
        roots(&shape, &RUDDER_R, &[[13., -42., 8.], [16., -51., 31.]])?;
        roots(&shape, &RUDDER_L, &[[-13., -42., 8.], [-16., -51., 31.]])?;
        let original: BTreeSet<_> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words remove exactly the two flap pairs: 0x6138 the left,
        // 0x613e the right. This pins the word-to-surface mapping.
        branch(bytes, &original, 0x6138, 1, &[], &FLAP_L)?;
        branch(bytes, &original, 0x613e, 1, &[], &FLAP_R)?;
        // The rudder word swaps the two rear fin quads for their deflected pose.
        let rudder = branch(
            bytes,
            &original,
            0x6144,
            1,
            &RUDDER_DEFLECTED,
            &[0x448e, 0x44ad, 0x44eb, 0x450a],
        )?;
        // The deflected panels keep the same footprint and hinge edge and swing
        // their trailing edge the same way in X on both sides: a rudder-sense
        // deflection, not a different panel. This is the evidence for the fitted
        // rudder line.
        let footprint = |ids: &[usize], shape: &Shape| -> AppResult<BTreeSet<(i64, i64)>> {
            let mut out = BTreeSet::new();
            for &address in ids {
                for p in &one(shape, address)?.positions {
                    out.insert((
                        (f64::from(p[1]) * 1000.).round() as i64,
                        (f64::from(p[2]) * 1000.).round() as i64,
                    ));
                }
            }
            Ok(out)
        };
        // Both sides' -53/8 trailing corner (the source's lower rear vertex)
        // moves the same way in X under the word, so the two fins yaw together.
        let trailing = |shape: &Shape, ids: &[usize]| -> AppResult<Vec<[f32; 3]>> {
            let mut out = Vec::new();
            for &address in ids {
                for p in &one(shape, address)?.positions {
                    if p[1] == -53. && p[2] == 8. {
                        out.push(*p);
                    }
                }
            }
            Ok(out)
        };
        for (neutral_ids, deflected_ids, hinge_point) in [
            (&RUDDER_R[..], &[0x431a, 0x4339][..], [13., -42., 8.]),
            (&RUDDER_L[..], &[0x4377, 0x4396][..], [-13., -42., 8.]),
        ] {
            if footprint(neutral_ids, &shape)? != footprint(deflected_ids, &rudder)?
                || !one(&rudder, deflected_ids[0])?
                    .positions
                    .contains(&hinge_point)
            {
                return Err("unreviewed MIG31.SH rudder deflection".into());
            }
            for p in trailing(&shape, neutral_ids)? {
                if !trailing(&rudder, deflected_ids)?
                    .iter()
                    .any(|q| q[0] - p[0] > 1e-4)
                {
                    return Err("unreviewed MIG31.SH rudder sense".into());
                }
            }
        }
        // The gear word adds exactly the twelve reviewed cards.
        let gear = branch(bytes, &original, 0x612c, 1, &GEAR, &[])?;
        let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
        for face in gear.faces.iter().filter(|f| GEAR.contains(&f.address)) {
            for p in &face.positions {
                low = low.min(p[2]);
                high = high.max(p[2]);
            }
        }
        if high - low != GEAR_RISE {
            return Err("unreviewed MIG31.SH gear extent".into());
        }
        for source in gear.faces.into_iter().filter(|f| GEAR.contains(&f.address)) {
            shape.faces.push(source);
        }
        Ok((Self, shape))
    }
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if is_stabilator(a) {
            let (pivot, axis, base) = hinge(a);
            turn(
                &mut result,
                pivot,
                axis,
                base * state.elevator.clamp(-1., 1.),
            );
        } else if is_rudder(a) {
            let (pivot, axis, base) = hinge(a);
            turn(&mut result, pivot, axis, base * state.rudder.clamp(-1., 1.));
        } else if is_flap(a) {
            let (pivot, axis, base) = hinge(a);
            turn(&mut result, pivot, axis, base * state.flaps.clamp(0., 1.));
        } else if is_aileron(a) {
            let (pivot, axis, base) = hinge(a);
            turn(
                &mut result,
                pivot,
                axis,
                base * state.aileron.clamp(-1., 1.),
            );
        } else if GEAR.contains(&a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear);
        }
        Some(result)
    }
}
/// M31-4 owns the afterburner plume (word 0x6120) and the airbrake (0x6126).
pub(crate) fn flame(_address: usize) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn state() -> State {
        State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
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
    fn off_hinge(a: usize, p: [f32; 3]) -> f64 {
        let (pivot, axis, _) = hinge(a);
        let offset: [f64; 3] = std::array::from_fn(|i| f64::from(p[i] - pivot[i]));
        let cross = [
            f64::from(axis[1]) * offset[2] - f64::from(axis[2]) * offset[1],
            f64::from(axis[2]) * offset[0] - f64::from(axis[0]) * offset[2],
            f64::from(axis[0]) * offset[1] - f64::from(axis[1]) * offset[0],
        ];
        cross.iter().map(|v| v * v).sum::<f64>().sqrt()
            / (axis.iter().map(|v| f64::from(v * v)).sum::<f64>().sqrt())
    }
    #[test]
    fn hinge_lines_pass_through_the_shared_source_corners() {
        // The ailerons, flaps and rudders hinge on the exact source edge the
        // fixed panel shares. The all-moving stabilators hinge on their root
        // chord midpoint because no single root corner stays put.
        for (a, points) in [
            (0x3ae3, [[45., -16., 2.], [81., -9., -1.]]),
            (0x41a6, [[17., -5., 4.], [45., -16., 2.]]),
            (0x448e, [[13., -42., 8.], [16., -51., 31.]]),
        ] {
            for p in points {
                assert!(off_hinge(a, p) < 1e-3, "face {a:x} point {p:?} off hinge");
            }
        }
        assert!(off_hinge(0x3d69, [17., -40., -3.]) < 1e-3);
        assert!(off_hinge(0x39f6, [-17., -40., -3.]) < 1e-3);
    }
    #[test]
    fn controls_keep_their_hinge_corners_and_move_the_expected_way() {
        let rig = Rig;
        let mut s = state();
        // address, the hinge point the line holds, a trailing witness, the
        // measured axis, the sign of a positive command and the driving field.
        type Case = (
            &'static [usize],
            [f32; 3],
            [f32; 3],
            usize,
            f32,
            &'static str,
        );
        let panels: [Case; 8] = [
            (
                &STABILATOR_R,
                [17., -40., -3.],
                [35., -68., -3.],
                2,
                1.,
                "elevator",
            ),
            (
                &STABILATOR_L,
                [-17., -40., -3.],
                [-35., -68., -3.],
                2,
                1.,
                "elevator",
            ),
            (&RUDDER_R, [13., -42., 8.], [13., -53., 8.], 0, 1., "rudder"),
            (
                &RUDDER_L,
                [-13., -42., 8.],
                [-13., -53., 8.],
                0,
                1.,
                "rudder",
            ),
            (&FLAP_R, [17., -5., 4.], [45., -23., 2.], 2, -1., "flaps"),
            (&FLAP_L, [-17., -5., 4.], [-45., -23., 2.], 2, -1., "flaps"),
            (
                &AILERON_R,
                [45., -16., 2.],
                [81., -32., -1.],
                2,
                1.,
                "aileron",
            ),
            (
                &AILERON_L,
                [-45., -16., 2.],
                [-81., -32., -1.],
                2,
                -1.,
                "aileron",
            ),
        ];
        for (addresses, anchor, witness, axis, sign, field) in panels {
            for command in [0., 1.] {
                s.elevator = 0.;
                s.rudder = 0.;
                s.flaps = 0.;
                s.aileron = 0.;
                match field {
                    "elevator" => s.elevator = command,
                    "rudder" => s.rudder = command,
                    "flaps" => s.flaps = command,
                    _ => s.aileron = command,
                }
                for &address in addresses {
                    let source = face(address, vec![anchor, witness]);
                    let out = rig.animate(&source, &s).unwrap();
                    if command == 0. {
                        assert_eq!(out.positions, source.positions, "face {address:x} at zero");
                    } else {
                        assert!(
                            (out.positions[0][axis] - anchor[axis]).abs() < 1e-4,
                            "face {address:x} hinge corner moved"
                        );
                        assert!(
                            (out.positions[1][axis] - witness[axis]) * sign > 1e-4,
                            "face {address:x} witness did not move the reviewed way"
                        );
                    }
                }
            }
        }
    }
    /// Both rudders deflect the same way: a yaw, not a roll, and the flap pairs
    /// lower together. A positive aileron raises the right wing and lowers the
    /// left, which is a roll.
    #[test]
    fn paired_surfaces_move_in_the_reviewed_sense() {
        let rig = Rig;
        let mut s = state();
        s.rudder = 1.;
        let right = rig
            .animate(&face(0x448e, vec![[13., -53., 8.]]), &s)
            .unwrap();
        let left = rig
            .animate(&face(0x44eb, vec![[-13., -53., 8.]]), &s)
            .unwrap();
        assert!(right.positions[0][0] > 13. && left.positions[0][0] > -13.);
        s.rudder = 0.;
        s.flaps = 1.;
        let (rf, lf) = (
            rig.animate(&face(0x41a6, vec![[45., -23., 2.]]), &s)
                .unwrap(),
            rig.animate(&face(0x4260, vec![[-45., -23., 2.]]), &s)
                .unwrap(),
        );
        assert!(rf.positions[0][2] < 2. && lf.positions[0][2] < 2.);
        s.flaps = 0.;
        s.aileron = 1.;
        let (ra, la) = (
            rig.animate(&face(0x3ae3, vec![[45., -23., 2.]]), &s)
                .unwrap(),
            rig.animate(&face(0x3893, vec![[-45., -23., 2.]]), &s)
                .unwrap(),
        );
        assert!(ra.positions[0][2] > 2. && la.positions[0][2] < 2.);
    }
    /// The interim gear rise is a rigid translation of the whole source group,
    /// hidden at zero, and it reaches the fuselage belly plane at near-stow.
    #[test]
    fn gear_rises_rigidly_and_hides_at_zero() {
        let rig = Rig;
        let mut s = state();
        let source = face(
            GEAR[0],
            vec![[17., 7., -19.], [15., 7., -10.], [15., 28., -10.]],
        );
        s.gear = 0.;
        assert!(rig.animate(&source, &s).is_none());
        for step in 1..=20 {
            s.gear = f64::from(step) / 20.;
            let out = rig.animate(&source, &s).unwrap();
            for i in 0..source.positions.len() {
                for j in i + 1..source.positions.len() {
                    let before = (0..3)
                        .map(|k| (source.positions[i][k] - source.positions[j][k]).powi(2))
                        .sum::<f32>()
                        .sqrt();
                    let now = (0..3)
                        .map(|k| (out.positions[i][k] - out.positions[j][k]).powi(2))
                        .sum::<f32>()
                        .sqrt();
                    assert!((before - now).abs() < 1e-4, "gear card changed shape");
                }
            }
        }
        s.gear = 1e-6;
        let stowed = rig.animate(&source, &s).unwrap();
        assert!(stowed.positions.iter().all(|p| p[2] >= -10.5));
    }
}
