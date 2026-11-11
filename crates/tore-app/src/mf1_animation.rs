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
//! Gear, airbrake and burner (MF1-4): the gear word 0x5adc adds three flat
//! plates (both windings each), turned as whole rigid pieces: the mains retract
//! forward and inward about an axis on their own top edge (15 degrees off
//! lateral) and the nose plate retracts rearward about a lateral axis through
//! its own top forward corner, steering about its vertical axis. The airbrake word 0x5ad6 adds the
//! one dorsal plate, turned whole about its forward edge onto the spine. The
//! plume word 0x5ad0 adds the one plume (two crossed pairs of plates) that
//! stretches from the nozzle exit plane at the one Atar nozzle. Every added
//! face is scaled at load like the rest of the shape and animated here (a gear
//! of 0, a brake of 0 and an exhaust of 0 draw nothing). Real directions: the
//! F1's main gear retracts forward and the nose gear rearward (Air Vectors,
//! "The Dassault Mirage F1").
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

/// Gear word 0x5adc: three flat plates, both windings each, added by the word
/// (the mains stand at x +-3 (top) to +-7 (foot), y -5 to 4, z -6 to -17; the
/// nose in the plane x = 0, y 38 to 47, z -7 to -17). The source has no wheel,
/// strut or door beyond these plates.
pub(crate) const GEAR_MAIN_L: [usize; 2] = [0x3b7e, 0x3b9d];
pub(crate) const GEAR_MAIN_R: [usize; 2] = [0x3be9, 0x3c08];
pub(crate) const GEAR_NOSE: [usize; 2] = [0x3c54, 0x3c73];
pub(crate) const GEAR: [usize; 6] = [0x3b7e, 0x3b9d, 0x3be9, 0x3c08, 0x3c54, 0x3c73];
/// Airbrake word 0x5ad6: the one dorsal plate, both windings, hinged on its
/// forward edge (y 29, z 5) and raised aft to (y 19, z 14).
pub(crate) const BRAKE: [usize; 2] = [0x4182, 0x4199];
/// Plume word 0x5ad0: two crossed pairs of plates (x = 0 and z = -1 planes,
/// both windings), an inner segment from the nozzle exit plane y -42 to y -56
/// and an outer one on to y -81.
pub(crate) const FLAME: [usize; 8] = [
    0x4207, 0x422e, 0x4255, 0x427c, 0x42a3, 0x42ca, 0x42f1, 0x4318,
];
const FLAME_ROOT: f32 = -42.;
const FLAME_TIP: f32 = -81.;
/// The nozzle exit disc, the engine material's face: two half discs at y -42
/// either side of x = 0 (subtype 0x64). The two thin triangles at x = +-4 in the
/// same plane have no area and carry no material.
pub(crate) const NOZZLE: [usize; 2] = [0x2e92, 0x2eb3];

/// Main gear hinge: a line on each plate's own top edge, 1 unit forward of the
/// rear corner, (+-3, -4, -6), leaning 15 degrees forward of the lateral
/// direction, as a real main leg's inclined pivot does: the foot swings forward
/// and inward, so the plate lies flat inside the fuselage side (x 3 to 6, y -5
/// to 7, z -7.5 to 3) where a purely lateral axis would leave its outboard
/// corner 1 foot above the wing shoulder. The two axes mirror each other.
/// Fitted.
const MAIN_HINGE_R: [f32; 3] = [3., -4., -6.];
const MAIN_HINGE_L: [f32; 3] = [-3., -4., -6.];
const MAIN_AXIS_R: [f32; 3] = [0.965_925_8, 0.258_819_04, 0.];
const MAIN_AXIS_L: [f32; 3] = [0.965_925_8, -0.258_819_04, 0.];
/// Nose hinge: a lateral line through the plate's own top forward corner,
/// (0, 47, -7); the plate swings rearward and up into the nose bay.
const NOSE_HINGE: [f32; 3] = [0., 47., -7.];
/// Vertical steering axis through the nose plate's centre.
const NOSE_STEER: [f32; 3] = [0., 42.5, -12.];
/// Airbrake hinge: the plate's forward edge, a line along X at (y 29, z 5).
const BRAKE_HINGE: [f32; 3] = [0., 29., 5.];
/// The dorsal ridge the closed plate lies along: (0, 37, 6) forward to (0, 7, 5)
/// aft, the top line of the fuselage skin under the plate.
const SPINE_FORWARD: [f32; 3] = [0., 37., 6.];
const SPINE_AFT: [f32; 3] = [0., 7., 5.];
/// The gear stow angles in radians. The mains swing forward through 95 degrees
/// (gear down at 0). The nose plate swings rearward 110 degrees so the
/// whole plate rises above its hinge line (the underside of the nose bay
/// climbs toward the front). Fitted.
const MAIN_STOW: f64 = 95. * std::f64::consts::PI / 180.;
const NOSE_STOW: f64 = 110. * std::f64::consts::PI / 180.;

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

pub(crate) fn gear_face(address: usize) -> bool {
    GEAR.contains(&address)
}
pub(crate) fn flame(address: usize) -> bool {
    FLAME.contains(&address)
}
/// True for the faces of the nozzle exit disc (the engine material's face).
pub(crate) fn nozzle(address: usize) -> bool {
    NOZZLE.contains(&address)
}

/// Closed airbrake angle, radians: the turn about the forward hinge that lays
/// the raised plate along the dorsal ridge, worked out on the scaled shape so
/// the plate lies along the scaled ridge. Fitted.
pub(crate) fn brake_closed() -> f64 {
    let direction = |a: [f32; 3], b: [f32; 3]| {
        let (a, b) = (sp(a), sp(b));
        f64::atan2(f64::from(b[2] - a[2]), f64::from(b[1] - a[1]))
    };
    let open = direction(BRAKE_HINGE, [0., 19., 14.]);
    let spine = direction(SPINE_FORWARD, SPINE_AFT);
    (spine - open).rem_euclid(std::f64::consts::TAU)
}

/// Whole-plate gear motion. Gear 1 is the source pose; each plate turns as one
/// rigid body about a line on its own top edge: the mains forward and inward,
/// the nose rearward. The nose plate also steers about its vertical axis. A gear of
/// 0 draws nothing (the caller drops the face).
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let up = 1. - gear.clamp(0., 1.);
    let a = f.address;
    if GEAR_NOSE.contains(&a) {
        if steering != 0. {
            turn(f, sp(NOSE_STEER), sp([0., 0., 1.]), -steering);
        }
        turn(f, sp(NOSE_HINGE), sp([1., 0., 0.]), -NOSE_STOW * up);
    } else if GEAR_MAIN_R.contains(&a) {
        turn(f, sp(MAIN_HINGE_R), sp(MAIN_AXIS_R), MAIN_STOW * up);
    } else {
        turn(f, sp(MAIN_HINGE_L), sp(MAIN_AXIS_L), MAIN_STOW * up);
    }
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
    /// against the reviewed source, and adds the scaled faces of the gear,
    /// airbrake and plume words (they are animated here and drawn only while
    /// their demand is above 0). Every other face is a neutral face the rig
    /// turns (the control surfaces) or leaves as it is.
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 19200
            || shape.faces.len() != 236
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed MF1.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        // Every face of every moving surface is made only of its reviewed
        // vertices (so no vertex is held off a hinge or spindle that the rig
        // does not turn about), and together each side uses all of them.
        for (addresses, points) in GROUPS {
            let mut seen = BTreeSet::new();
            for &address in addresses {
                let face = one(&shape, address)?;
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
            if one(&shape, address)?.positions.len() != 5 {
                return Err(format!("unreviewed MF1.SH tailplane face {address:x}").into());
            }
        }
        // The nozzle exit disc: two half discs in the plane y -42, together
        // spanning x -4 to 4 and z -6 to 3 (the engine material's bounds).
        let mut disc = (
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        );
        for &address in &NOZZLE {
            let face = one(&shape, address)?;
            if face.positions.len() != 6
                || !face
                    .positions
                    .iter()
                    .all(|p| p[1] == sp([0., FLAME_ROOT, 0.])[1])
            {
                return Err(format!("unreviewed MF1.SH nozzle face {address:x}").into());
            }
            for p in &face.positions {
                disc = (
                    disc.0.min(p[0]),
                    disc.1.max(p[0]),
                    disc.2.min(p[2]),
                    disc.3.max(p[2]),
                );
            }
        }
        if disc
            != (
                sp([-4., 0., 0.])[0],
                sp([4., 0., 0.])[0],
                sp([0., 0., -6.])[2],
                sp([0., 0., 3.])[2],
            )
        {
            return Err("unreviewed MF1.SH nozzle disc".into());
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
        // The gear, airbrake and plume words add exactly the reviewed plates and
        // remove nothing. Each plate is one whole four-corner quad (both
        // windings) whose corners are checked against the reviewed source
        // values, so the rig refuses a shape that differs.
        let device = |word: usize, added: &[usize]| -> AppResult<Shape> {
            branch(bytes, &neutral, word, added, &[])?;
            Ok(Shape::with_state(bytes, &[(word, 1)].into())?.scaled(axis))
        };
        let gear = device(0x5adc, &GEAR)?;
        let brake = device(0x5ad6, &BRAKE)?;
        let flame = device(0x5ad0, &FLAME)?;
        let corners = |ids: &[usize], shape: &Shape, allowed: &[[f32; 3]]| -> AppResult<()> {
            for &address in ids {
                let face = one(shape, address)?;
                if face.positions.len() != 4
                    || !face
                        .positions
                        .iter()
                        .all(|&p| allowed.iter().any(|&q| p == sp(q)))
                    || !allowed.iter().all(|&q| face.positions.contains(&sp(q)))
                {
                    return Err(format!("unreviewed MF1.SH plate {address:x}").into());
                }
            }
            Ok(())
        };
        let plate = |x_top: f32, x_foot: f32| {
            [
                [x_top, -5., -6.],
                [x_top, 4., -6.],
                [x_foot, 4., -17.],
                [x_foot, -5., -17.],
            ]
        };
        corners(&GEAR_MAIN_R, &gear, &plate(3., 7.))?;
        corners(&GEAR_MAIN_L, &gear, &plate(-3., -7.))?;
        corners(
            &GEAR_NOSE,
            &gear,
            &[
                [0., 38., -7.],
                [0., 47., -7.],
                [0., 47., -17.],
                [0., 38., -17.],
            ],
        )?;
        corners(
            &BRAKE,
            &brake,
            &[
                [2., 19., 14.],
                [-2., 19., 14.],
                [-2., 29., 5.],
                [3., 29., 5.],
            ],
        )?;
        // The plume is two crossed pairs of plates, all within the reviewed
        // extent; the nozzle exit plane is the root that stays.
        let (mut root, mut tip) = (f32::NEG_INFINITY, f32::INFINITY);
        for &address in &FLAME {
            let face = one(&flame, address)?;
            if face.positions.len() != 4 {
                return Err(format!("unreviewed MF1.SH plume plate {address:x}").into());
            }
            for p in &face.positions {
                root = root.max(p[1]);
                tip = tip.min(p[1]);
            }
        }
        if root != sp([0., FLAME_ROOT, 0.])[1] || tip != sp([0., FLAME_TIP, 0.])[1] {
            return Err("unreviewed MF1.SH plume extent".into());
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

    /// The face turned for the pilot's commands, or `None` when it is not
    /// drawn (a gear, brake or exhaust of 0). Each surface and device is one
    /// rigid piece (all its faces and both windings turn together, no vertex
    /// held off its hinge), so nothing folds.
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if gear_face(a) {
            if state.gear <= 0. {
                return None;
            }
            gear_positions(&mut result, state.gear, state.nosewheel_angle());
            return Some(result);
        }
        if BRAKE.contains(&a) {
            if state.brake <= 0. {
                return None;
            }
            // Open (brake 1) is the source pose; closing turns the whole plate
            // about its forward edge down onto the spine.
            turn(
                &mut result,
                sp(BRAKE_HINGE),
                sp([1., 0., 0.]),
                brake_closed() * (1. - state.brake.clamp(0., 1.)),
            );
            return Some(result);
        }
        if flame(a) {
            if state.exhaust <= 0. {
                return None;
            }
            // The plume grows from the nozzle exit plane; every plate scales
            // from the same root, so the shared edges stay together.
            let root = sp([0., FLAME_ROOT, 0.])[1];
            for p in &mut result.positions {
                p[1] = root + (p[1] - root) * state.exhaust.clamp(0., 1.) as f32;
            }
            return Some(result);
        }
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
            return Some(result);
        };
        if angle != 0. {
            turn(&mut result, sp(pivot), sp(axis), angle);
        }
        Some(result)
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
        faces.iter().map(|f| Rig.animate(f, s).unwrap()).collect()
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
                Rig.animate(&f, &s).unwrap().positions,
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
            let out = Rig.animate(source, s).unwrap();
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
        let out = Rig.animate(source, &s).unwrap();
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
                assert_eq!(Rig.animate(f, &s).unwrap().positions, f.positions);
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
    fn the_device_words_never_overlap_the_reviewed_surfaces() {
        for address in GEAR.iter().chain(&BRAKE).chain(&FLAME).chain(&RUDDER_WORD) {
            for (ids, _) in GROUPS {
                assert!(!ids.contains(address));
            }
        }
        // The nozzle disc is neither a device nor a surface.
        for address in NOZZLE {
            assert!(!GEAR.contains(&address) && !BRAKE.contains(&address));
            assert!(!FLAME.contains(&address));
            for (ids, _) in GROUPS {
                assert!(!ids.contains(&address));
            }
        }
    }
    /// The three gear plates and the airbrake plate as synthetic faces, with
    /// the vertices the retail faces hold (shape units).
    fn devices() -> Vec<Face> {
        let plate = |x_top: f32, x_foot: f32| {
            [
                [x_top, -5., -6.],
                [x_top, 4., -6.],
                [x_foot, 4., -17.],
                [x_foot, -5., -17.],
            ]
        };
        let rev = |p: [[f32; 3]; 4]| [p[3], p[2], p[1], p[0]];
        let nose = [
            [0., 38., -7.],
            [0., 47., -7.],
            [0., 47., -17.],
            [0., 38., -17.],
        ];
        let brake = [
            [2., 19., 14.],
            [-2., 19., 14.],
            [-2., 29., 5.],
            [3., 29., 5.],
        ];
        vec![
            face(0x3b7e, &plate(-3., -7.)),
            face(0x3b9d, &rev(plate(-3., -7.))),
            face(0x3be9, &plate(3., 7.)),
            face(0x3c08, &rev(plate(3., 7.))),
            face(0x3c54, &nose),
            face(0x3c73, &rev(nose)),
            face(0x4182, &brake),
            face(0x4199, &rev(brake)),
        ]
    }
    fn lowest(faces: &[Face], ids: &[usize]) -> f32 {
        group(faces, ids)
            .iter()
            .flat_map(|f| f.positions.iter().map(|p| p[2]))
            .fold(f32::INFINITY, f32::min)
    }
    #[test]
    fn gear_down_is_the_source_pose_and_gear_up_draws_nothing() {
        let all = devices();
        let mut s = state();
        s.gear = 1.;
        for f in all.iter().filter(|f| gear_face(f.address)) {
            let out = Rig.animate(f, &s).unwrap();
            for (a, b) in out.positions.iter().zip(&f.positions) {
                for k in 0..3 {
                    assert!(
                        (a[k] - b[k]).abs() < 1e-4,
                        "{:x} moved at gear 1",
                        f.address
                    );
                }
            }
        }
        s.gear = 0.;
        assert!(
            all.iter()
                .filter(|f| gear_face(f.address))
                .all(|f| Rig.animate(f, &s).is_none())
        );
        // The scaled low point is the standing height: 17 units at a third of
        // a foot, times the height factor.
        let want = sp([0., 0., -17.])[2];
        assert!((lowest(&all, &GEAR) - want).abs() < 1e-5);
    }
    #[test]
    fn every_gear_plate_turns_as_one_rigid_piece_and_never_hangs_lower_than_gear_down() {
        let all = devices();
        let mut s = state();
        for ids in [&GEAR_MAIN_R[..], &GEAR_MAIN_L, &GEAR_NOSE] {
            let source = group(&all, ids);
            for gear in [1., 0.9, 0.75, 0.5, 0.25, 0.1, 1e-6] {
                s.gear = gear;
                let out = turned(&source, &s);
                assert!(spread(&points(&source), &points(&out)) < 1e-3);
                // The two windings stay one plate.
                let (a, b) = (&out[0].positions, &out[1].positions);
                for p in a {
                    assert!(b.iter().any(|q| (0..3).all(|k| (p[k] - q[k]).abs() < 1e-4)));
                }
                for f in &out {
                    for p in &f.positions {
                        // The main foot dips at most 0.3 units (0.1 ft) in the first
                        // degrees, swinging inward on the inclined axis.
                        assert!(p[2] >= lowest(&all, &GEAR) - 0.3);
                    }
                }
            }
        }
    }
    #[test]
    fn the_mains_retract_forward_and_the_nose_retracts_rearward_into_the_fuselage() {
        let all = devices();
        let mut s = state();
        let foot =
            |f: &[Face], p: [f32; 3]| f[0].positions.iter().position(|&q| q == sp(p)).unwrap();
        for (ids, p, forward) in [
            (&GEAR_MAIN_R[..], [7., -5., -17.], true),
            (&GEAR_MAIN_L[..], [-7., -5., -17.], true),
            (&GEAR_NOSE[..], [0., 47., -17.], false),
        ] {
            let source = group(&all, ids);
            let i = foot(&source, p);
            for gear in [0.75, 0.5, 0.25, 1e-6] {
                s.gear = gear;
                let out = turned(&source, &s);
                let dy = out[0].positions[i][1] - source[0].positions[i][1];
                assert!(
                    if forward { dy > 0. } else { dy < 0. },
                    "{ids:x?} foot moves the wrong way at {gear}"
                );
            }
        }
        // Stowed (gear 0+): the mains lie inside the fuselage side (x 2.5 to
        // 6.6, y -5 to 7, z -7.5 to 3.5 in shape units; the hull is 7 units
        // half-wide at y 7 and its belly is at z -7 there), the nose plate
        // above its hinge height.
        s.gear = 1e-6;
        let tol = 0.02;
        for ids in [&GEAR_MAIN_R[..], &GEAR_MAIN_L] {
            for p in points(&turned(&group(&all, ids), &s)) {
                let (lo, hi) = (sp([2.5, -5., -7.5]), sp([6.6, 7., 3.5]));
                assert!(
                    p[0].abs() >= lo[0] - tol && p[0].abs() <= hi[0] + tol,
                    "{p:?}"
                );
                assert!(p[1] >= lo[1] - tol && p[1] <= hi[1] + tol, "{p:?}");
                assert!(p[2] >= lo[2] - tol && p[2] <= hi[2] + tol, "{p:?}");
            }
        }
        // The two sides mirror each other.
        let right = points(&turned(&group(&all, &GEAR_MAIN_R), &s));
        let left = points(&turned(&group(&all, &GEAR_MAIN_L), &s));
        for r in &right {
            assert!(
                left.iter().any(|l| (l[0] + r[0]).abs() < 1e-3
                    && (l[1] - r[1]).abs() < 1e-3
                    && (l[2] - r[2]).abs() < 1e-3),
                "no mirror of {r:?}"
            );
        }
        let nose = points(&turned(&group(&all, &GEAR_NOSE), &s));
        let hinge = sp(NOSE_HINGE);
        assert!(nose.iter().all(|p| p[2] >= hinge[2] - tol));
        assert!(nose.iter().all(|p| p[1] >= sp([0., 37., 0.])[1] - tol));
        assert!(nose.iter().all(|p| p[1] <= sp([0., 51., 0.])[1] + tol));
    }
    #[test]
    fn the_hinge_corners_do_not_move_and_the_nose_steers_about_its_centre() {
        let all = devices();
        let mut s = state();
        s.gear = 0.4;
        let source = group(&all, &GEAR_NOSE);
        let out = turned(&source, &s);
        let corner = sp([0., 47., -7.]);
        let i = source[0]
            .positions
            .iter()
            .position(|&q| q == corner)
            .unwrap();
        for (a, b) in out[0].positions[i].iter().zip(corner) {
            assert!((a - b).abs() < 1e-4);
        }
        // A main's hinge is the point of its top edge 1 unit forward of the
        // rear corner: it stays where it is, on both sides.
        for (ids, hinge) in [
            (&GEAR_MAIN_R[..], MAIN_HINGE_R),
            (&GEAR_MAIN_L[..], MAIN_HINGE_L),
        ] {
            let source = group(&all, ids);
            let out = turned(&source, &s);
            let (a, b) = (sp([hinge[0], -5., -6.]), sp([hinge[0], 4., -6.]));
            let i = source[0].positions.iter().position(|&q| q == a).unwrap();
            let j = source[0].positions.iter().position(|&q| q == b).unwrap();
            let on_edge = |f: &Face| -> [f32; 3] {
                std::array::from_fn(|k| {
                    f.positions[i][k] + (f.positions[j][k] - f.positions[i][k]) / 9.
                })
            };
            let (before, after) = (on_edge(&source[0]), on_edge(&out[0]));
            assert!((0..3).all(|k| (before[k] - sp(hinge)[k]).abs() < 1e-4));
            assert!((0..3).all(|k| (after[k] - sp(hinge)[k]).abs() < 1e-4));
        }
        // Steering turns the nose plate about its centre: the centre stays in
        // place and the foot swings sideways, in both directions.
        s.gear = 1.;
        let nose = group(&all, &GEAR_NOSE);
        let rig = |angle: f64| {
            let mut f = nose[0].clone();
            gear_positions(&mut f, 1., angle);
            f
        };
        assert_eq!(rig(0.).positions, nose[0].positions);
        let (right, left) = (rig(0.3), rig(-0.3));
        let i = nose[0]
            .positions
            .iter()
            .position(|&q| q == sp([0., 47., -17.]))
            .unwrap();
        assert!(right.positions[i][0] * left.positions[i][0] < 0.);
        assert!(spread(&nose[0].positions, &right.positions) < 1e-3);
    }
    #[test]
    fn the_airbrake_plate_is_open_at_1_and_turns_whole_about_its_forward_edge_onto_the_spine() {
        let all = devices();
        let plate = group(&all, &BRAKE);
        let mut s = state();
        s.brake = 1.;
        assert_eq!(points(&turned(&plate, &s)), points(&plate));
        s.brake = 0.;
        assert!(plate.iter().all(|f| Rig.animate(f, &s).is_none()));
        for brake in [0.75, 0.5, 0.25, 1e-6] {
            s.brake = brake;
            let out = turned(&plate, &s);
            assert!(spread(&points(&plate), &points(&out)) < 1e-3);
            // The forward edge (y 29) is the hinge and stays.
            for (a, b) in points(&plate).iter().zip(points(&out)) {
                if a[1] == sp([0., 29., 0.])[1] {
                    assert!((0..3).all(|k| (a[k] - b[k]).abs() < 1e-3));
                }
                // Closing never raises the free edge.
                assert!(b[2] <= a[2] + 1e-4);
            }
        }
        // Closed (a hair above 0): the free edge lies along the ridge line from
        // the hinge, within a hundredth of a foot (feet = units / 3).
        s.brake = 1e-9;
        let out = turned(&plate, &s);
        let hinge = sp(BRAKE_HINGE);
        let free = out[0].positions[1];
        let spine = [
            sp(SPINE_AFT)[1] - sp(SPINE_FORWARD)[1],
            sp(SPINE_AFT)[2] - sp(SPINE_FORWARD)[2],
        ];
        let rel = [free[1] - hinge[1], free[2] - hinge[2]];
        let off = (rel[0] * spine[1] - rel[1] * spine[0]).abs() / spine[0].hypot(spine[1]);
        assert!(off / 3. < 0.01, "{off}");
        assert!(
            brake_closed() > 0.5 && brake_closed() < 1.0,
            "{}",
            brake_closed()
        );
    }
    #[test]
    fn the_plume_grows_from_the_exit_plane_and_draws_only_with_exhaust() {
        let plate = |address, points: [[f32; 3]; 4]| face(address, &points);
        let plume = [
            plate(
                0x4207,
                [
                    [0., -56., 3.],
                    [0., -56., -5.],
                    [0., -81., -5.],
                    [0., -81., 3.],
                ],
            ),
            plate(
                0x42a3,
                [
                    [0., -56., -5.],
                    [0., -42., -5.],
                    [0., -42., 3.],
                    [0., -56., 3.],
                ],
            ),
        ];
        let mut s = state();
        s.exhaust = 0.;
        assert!(plume.iter().all(|f| Rig.animate(f, &s).is_none()));
        s.exhaust = 1.;
        for f in &plume {
            assert_eq!(Rig.animate(f, &s).unwrap().positions, f.positions);
        }
        s.exhaust = 0.5;
        let out: Vec<_> = plume.iter().map(|f| Rig.animate(f, &s).unwrap()).collect();
        // The exit plane stays and the tip comes forward by half its length;
        // the shared edge at y -56 moves the same in both plates.
        assert_eq!(out[1].positions[1], plume[1].positions[1]);
        let tip = out[0].positions[2][1];
        let root = sp([0., -42., 0.])[1];
        let full = sp([0., -81., 0.])[1];
        assert!((tip - (root + (full - root) * 0.5)).abs() < 1e-4);
        // The first plate's inner edge was at y -56 and the second plate's outer
        // edge was at the same place: they stay together.
        assert_eq!(out[0].positions[0], out[1].positions[3]);
        assert!(flame(0x4207) && !flame(0x2e92) && nozzle(0x2e92) && nozzle(0x2eb3));
    }
}
