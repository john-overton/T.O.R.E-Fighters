//! MiG31 own-source control surfaces (all-moving stabilators, outboard
//! ailerons, inboard flaps and twin rudders), rigid gear, dorsal airbrake and
//! afterburner plume. See docs/spec/variety-animation.md "MiG-31 control fit".
//! MIG31.SH ships one rudder branch (0x6144), two flap-removal branches
//! (0x6138, 0x613e) and three device branches (0x612c gear, 0x6126 brake,
//! 0x6120 plume), but no signed pitch or roll branch, so every control role is
//! a fitted geometric assignment over exact reviewed source panels. The six
//! state words follow the shared base order flame, brake, gear, flap, flap, then
//! the rudder: 0x6120 the plume, 0x6126 the brake, 0x612c the gear, 0x6138 and
//! 0x613e the left and right flap pairs, and 0x6144 the twin rudders.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
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
/// Gear word 0x612c. One strut plate and one painted wheel card per main side,
/// and a nose strut plate with its wheel card. The strut plate and wheel card of
/// each assembly are separate source quads; both windings of each quad are one
/// whole part. No separate gear-door faces exist in the source.
const GEAR_MAIN_R: [usize; 4] = [0x3eda, 0x3ef1, 0x3f35, 0x3f5c];
const GEAR_MAIN_L: [usize; 4] = [0x3fe4, 0x3ffb, 0x403f, 0x4066];
const GEAR_NOSE: [usize; 4] = [0x40ee, 0x4105, 0x4149, 0x4168];
const GEAR: [usize; 12] = [
    0x3eda, 0x3ef1, 0x3f35, 0x3f5c, 0x3fe4, 0x3ffb, 0x403f, 0x4066, 0x40ee, 0x4105, 0x4149, 0x4168,
];
/// Main hinge line: the lateral X axis through each strut's aft-top root at
/// Y=7, Z=-10. The whole assembly swings up and forward and stows at the belly.
const MAIN_HINGE: [f32; 3] = [0., 7., -10.];
/// Nose hinge line through the nose strut's aft-top root at Y=77, Z=-10.
const NOSE_HINGE: [f32; 3] = [0., 77., -10.];
/// Fitted vertical nose steering axis at the wheel card's centre. The source
/// nose strut plate sits three units inboard of its wheel card.
const NOSE_STEER: [f32; 3] = [0., 87.5, -10.];
/// Afterburner word 0x6120: three double-sided plates per engine, crossed and
/// trailing from the nozzle exit at Y=-68 to Y=-129.
const FLAME: [usize; 12] = [
    0x45c0, 0x45f3, 0x4626, 0x4659, 0x468c, 0x46b3, 0x46da, 0x4701, 0x4728, 0x4747, 0x4766, 0x4785,
];
const FLAME_ROOT: f32 = -68.;
/// Airbrake word 0x6126: one double-sided dorsal panel hinged on the spine edge
/// at Y=50, Z=8 and raised to Y=32, Z=24.
const BRAKE: [usize; 2] = [0x3e4b, 0x3e62];
const BRAKE_HINGE: [f32; 3] = [0., 50., 8.];
/// Fitted closed angle: rotates the source raised panel flat along the spine.
const BRAKE_CLOSED: f64 = 0.72;
/// Source vertical extent of the gear group, used to pin the reviewed cards.
const GEAR_EXTENT: f32 = 20.;
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
/// Whole-assembly gear motion. Each main pair and the nose pair are rigid groups
/// that share one lateral hinge, so the strut plate and its wheel card stay
/// attached as the wheel swings forward and up into the body. The source adds no
/// separate gear-door faces. The nose group also steers about its vertical strut
/// axis before it retracts.
pub fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let closing = 1. - gear.clamp(0., 1.);
    let angle = FRAC_PI_2 * closing;
    if GEAR_NOSE.contains(&f.address) {
        if steering != 0. {
            turn(f, NOSE_STEER, [0., 0., 1.], -steering);
        }
        turn(f, NOSE_HINGE, [1., 0., 0.], angle);
    } else {
        turn(f, MAIN_HINGE, [1., 0., 0.], angle);
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
        // The gear word adds exactly the twelve reviewed cards: two whole source
        // plates per assembly, each double-sided. The source has no separate
        // gear-door faces. The strut plate and its wheel card share one lateral
        // hinge per assembly and stay rigid.
        let gear = branch(bytes, &original, 0x612c, 1, &GEAR, &[])?;
        roots(&gear, &GEAR_MAIN_R, &[[15., 7., -10.], [15., 28., -10.]])?;
        roots(&gear, &GEAR_MAIN_L, &[[-15., 7., -10.], [-15., 28., -10.]])?;
        roots(&gear, &GEAR_NOSE, &[[-3., 77., -10.], [-3., 95., -10.]])?;
        let (mut low, mut high) = (f32::INFINITY, f32::NEG_INFINITY);
        for face in gear.faces.iter().filter(|f| GEAR.contains(&f.address)) {
            if face.positions.len() != 4 {
                return Err("unreviewed MIG31.SH gear card topology".into());
            }
            for p in &face.positions {
                low = low.min(p[2]);
                high = high.max(p[2]);
            }
        }
        if high - low != GEAR_EXTENT {
            return Err("unreviewed MIG31.SH gear extent".into());
        }
        // The brake word adds one raised double-sided dorsal panel hinged on the
        // spine front edge at (0,50,8); the plume word adds six double-sided
        // plates behind the Y=-68 nozzle exit.
        let brake = branch(bytes, &original, 0x6126, 1, &BRAKE, &[])?;
        for &address in &BRAKE {
            let face = one(&brake, address)?;
            if face.positions.len() != 4
                || !face.positions.contains(&[5., 50., 8.])
                || !face.positions.contains(&[-5., 50., 8.])
            {
                return Err("unreviewed MIG31.SH brake panel".into());
            }
        }
        let flame = branch(bytes, &original, 0x6120, 1, &FLAME, &[])?;
        let mut root = f32::NEG_INFINITY;
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4 && face.positions.len() != 6 {
                return Err("unreviewed MIG31.SH flame topology".into());
            }
            for p in &face.positions {
                if p[1] < -129. || p[1] > FLAME_ROOT {
                    return Err("unreviewed MIG31.SH flame extent".into());
                }
                root = root.max(p[1]);
            }
        }
        if root != FLAME_ROOT {
            return Err("unreviewed MIG31.SH flame root".into());
        }
        for source in gear
            .faces
            .into_iter()
            .filter(|f| GEAR.contains(&f.address))
            .chain(
                brake
                    .faces
                    .into_iter()
                    .filter(|f| BRAKE.contains(&f.address)),
            )
            .chain(
                flame
                    .faces
                    .into_iter()
                    .filter(|f| FLAME.contains(&f.address)),
            )
        {
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
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
        } else if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            turn(
                &mut result,
                BRAKE_HINGE,
                [1., 0., 0.],
                BRAKE_CLOSED * (1. - state.brake.clamp(0., 1.)),
            );
        } else if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            for p in &mut result.positions {
                p[1] = FLAME_ROOT + (p[1] - FLAME_ROOT) * state.exhaust.clamp(0., 1.) as f32;
            }
        }
        Some(result)
    }
}
pub(crate) fn flame(address: usize) -> bool {
    FLAME.contains(&address)
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
    /// Every gear card is a rigid quad, the strut plate's own hinge corner does
    /// not move, and at near-stow the whole card tucks to the belly plane. The
    /// source has no separate gear-door faces, so no half-door can be left behind.
    #[test]
    fn gear_cards_stay_rigid_on_their_hinge_and_stow_at_the_belly() {
        let rig = Rig;
        let mut s = state();
        let cards: [(&[usize], [f32; 3], [f32; 3]); 3] = [
            (&GEAR_MAIN_R, [15., 7., -10.], [15., 27., -30.]),
            (&GEAR_MAIN_L, [-15., 7., -10.], [-15., 27., -30.]),
            (&GEAR_NOSE, [-3., 77., -10.], [0., 87.5, -30.]),
        ];
        for (addresses, root, tip) in cards {
            for &address in addresses {
                let source = face(address, vec![root, tip, [root[0], root[1], tip[2]]]);
                for step in 1..=400 {
                    s.gear = f64::from(step) / 400.;
                    let out = rig.animate(&source, &s).unwrap();
                    for i in 0..3 {
                        for j in i + 1..3 {
                            let a = source.positions[i];
                            let b = source.positions[j];
                            let keep = (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>().sqrt();
                            let now = (0..3)
                                .map(|k| (out.positions[i][k] - out.positions[j][k]).powi(2))
                                .sum::<f32>()
                                .sqrt();
                            assert!((keep - now).abs() < 1e-3, "face {address:x} not rigid");
                        }
                    }
                    assert!(
                        (out.positions[0][1] - root[1]).abs() < 1e-4
                            && (out.positions[0][2] - root[2]).abs() < 1e-4,
                        "face {address:x} hinge root moved"
                    );
                }
                s.gear = 1e-6;
                let stowed = rig.animate(&source, &s).unwrap();
                assert!(
                    stowed.positions.iter().all(|p| p[2] >= -10.5),
                    "face {address:x} hangs below the belly at near-stow"
                );
            }
        }
        s.gear = 0.;
        assert!(
            rig.animate(&face(GEAR_MAIN_R[0], vec![[15., 7., -10.]]), &s)
                .is_none()
        );
    }
    #[test]
    fn brake_rises_from_its_spine_hinge_and_flame_extends_from_the_nozzle() {
        let rig = Rig;
        let mut s = state();
        let brake = face(
            0x3e4b,
            vec![
                [4., 32., 24.],
                [-4., 32., 24.],
                [-5., 50., 8.],
                [5., 50., 8.],
            ],
        );
        for fraction in [0., 0.25, 0.5, 0.75, 1.] {
            s.brake = fraction;
            if fraction == 0. {
                assert!(rig.animate(&brake, &s).is_none());
                continue;
            }
            let out = rig.animate(&brake, &s).unwrap();
            assert_eq!(out.positions[2], brake.positions[2]);
            assert_eq!(out.positions[3], brake.positions[3]);
            if fraction == 1. {
                assert_eq!(out.positions, brake.positions);
            } else {
                assert!(out.positions[0][2] < 24., "brake tip did not close down");
            }
        }
        let plume = face(
            0x45c0,
            vec![
                [10., -129., -2.],
                [10., -68., -2.],
                [3., -68., -2.],
                [3., -129., -2.],
            ],
        );
        for fraction in [0., 0.5, 1.] {
            s.exhaust = fraction;
            if fraction == 0. {
                assert!(rig.animate(&plume, &s).is_none());
                continue;
            }
            let out = rig.animate(&plume, &s).unwrap();
            assert!(
                (out.positions[1][1] - FLAME_ROOT).abs() < 1e-4
                    && (out.positions[2][1] - FLAME_ROOT).abs() < 1e-4,
                "flame root moved off the nozzle"
            );
            if fraction == 1. {
                assert_eq!(out.positions, plume.positions);
            } else {
                assert!(out.positions[0][1] > -129., "flame tip did not shorten");
            }
        }
        assert!(flame(FLAME[0]));
        assert!(!flame(GEAR[0]));
    }
}
