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
//! Gear, speed brake and plume (F5E-4, 2026-11-10) are this rig's too: the
//! three gear plates turn as whole rigid pieces (the mains inward about their
//! top edges, the nose forward with steering), the single dorsal speed brake
//! plate turns whole about its forward edge onto the spine, and the eight
//! plume plates stretch from the two nozzle exit planes. The engine material
//! (`engine_material.rs`) gives the F-5E its two outlets, so two nozzle glows
//! and two contrails come from the engine count of 2. See
//! docs/spec/variety-animation.md "F-5E gear, speed brake and plume fit" and
//! docs/formats/aircraft-control-surfaces.md "F-5E gear, speed brake and
//! plume (F5E-4, 2026-11-10)".
//!
//! The F-5E draws at its own size (F5E-2: the identity axis scale, size still
//! John's question), so the hinge points below are plain shape units; `sp`
//! still routes them through `scale_point` so a later scale decision cannot
//! detach a hinge.
use crate::{AppResult, additional_animation::turn, flight::State};
use std::collections::BTreeSet;
use std::f64::consts::FRAC_PI_2;
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

/// Gear word 0x63cc adds three flat plates, both windings each. The mains
/// stand in the planes x = +14 and x = -15 (y -2 to 6, z -18 to -4), the nose
/// in the plane x = 0 (y 53 to 60, z -17 to -5). The source has no wheels,
/// doors or struts apart from these plates.
pub(crate) const GEAR_MAIN_R: [usize; 2] = [0x49cc, 0x49eb];
pub(crate) const GEAR_MAIN_L: [usize; 2] = [0x4a37, 0x4a56];
pub(crate) const GEAR_NOSE: [usize; 2] = [0x4aa2, 0x4ac1];
const GEAR: [usize; 6] = [0x49cc, 0x49eb, 0x4a37, 0x4a56, 0x4aa2, 0x4ac1];
/// Main hinge: each plate's own top edge, a line along Y at z = -4 on the
/// plate's own x (shape units; the source plates are at +14 and -15).
const MAIN_HINGE_R: [f32; 3] = [14., 0., -4.];
const MAIN_HINGE_L: [f32; 3] = [-15., 0., -4.];
/// Nose hinge: a lateral line through the plate's aft top corner (y 53, z -5),
/// so the plate swings forward and up into the nose and never hangs below the
/// belly. The vertical steering axis passes through the plate centre.
const NOSE_HINGE: [f32; 3] = [0., 53., -5.];
const NOSE_STEER: [f32; 3] = [0., 56.5, 0.];
/// Speed brake word 0x63c6 adds one dorsal plate, both windings (4802 untextured,
/// 4819 textured): front edge (x +/-2, y 29, z 10), free edge (x +/-2, y 22,
/// z 16), rising aft. The front edge is the hinge.
const BRAKE: [usize; 2] = [0x4802, 0x4819];
const BRAKE_HINGE: [f32; 3] = [0., 29., 10.];
/// The free edge's mid point, open, and the dorsal ridge the closed brake lies
/// on: the source spine vertex (0, 37, 10) and the mid point (0, -34, 4) of the
/// rear pair (+/-1, -34, 4), all in face 0x3420.
const BRAKE_FREE: [f32; 3] = [0., 22., 16.];
const SPINE: [[f32; 3]; 2] = [[0., 37., 10.], [0., -34., 4.]];
const SPINE_FACE: usize = 0x3420;
/// Plume word 0x63c0: per engine two crossed plates (a vertical plate at
/// x = -3 or +3 and a horizontal plate at z = 0 centred on it), both windings,
/// from y -49 back to y -79. The nozzle exit rings are the two octagons
/// 0x42a6 (left) and 0x4454 (right) at y -48.
const FLAME_L: [usize; 4] = [0x48a7, 0x48e5, 0x4961, 0x4980];
const FLAME_R: [usize; 4] = [0x48c6, 0x4904, 0x4923, 0x4942];
const FLAME: [usize; 8] = [
    0x48a7, 0x48c6, 0x48e5, 0x4904, 0x4923, 0x4942, 0x4961, 0x4980,
];
const FLAME_ROOT: f32 = -48.;
const FLAME_TIP: f32 = -79.;
/// The two nozzle exit octagons (the engine material draws a glow on each).
pub(crate) const NOZZLES: [usize; 2] = [0x42a6, 0x4454];

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

/// A scaled vertex equals a shape-unit point (the same single `f32` product
/// `Shape::scaled` formed, so the comparison is exact).
fn is(p: [f32; 3], shape_units: [f32; 3]) -> bool {
    p == sp(shape_units)
}

/// Closed speed brake angle, radians: the turn about the hinge that carries the
/// free edge's mid point onto the dorsal ridge (the line through the source
/// spine vertices), solved on the scaled shape where the turned point meets
/// the ridge line. Fitted.
pub(crate) fn brake_closed() -> f64 {
    let hinge = sp(BRAKE_HINGE);
    let (a, b) = (sp(SPINE[0]), sp(SPINE[1]));
    let free = sp(BRAKE_FREE);
    let (dy, dz) = (f64::from(free[1] - hinge[1]), f64::from(free[2] - hinge[2]));
    // Height of the turned free edge above the ridge under it.
    let above = |angle: f64| {
        let (s, c) = angle.sin_cos();
        let y = f64::from(hinge[1]) + dy * c - dz * s;
        let z = f64::from(hinge[2]) + dy * s + dz * c;
        let ridge = f64::from(a[2])
            + (y - f64::from(a[1])) / f64::from(b[1] - a[1]) * f64::from(b[2] - a[2]);
        z - ridge
    };
    // Open, the free edge is above the ridge; a turn lowers it. Bisect.
    let (mut low, mut high) = (0., 1.2);
    for _ in 0..60 {
        let mid = (low + high) / 2.;
        if above(mid) > 0. {
            low = mid;
        } else {
            high = mid;
        }
    }
    (low + high) / 2.
}

/// The ridge height at `y` on the scaled shape (for the tests).
#[cfg(test)]
fn spine_height_at(y: f32) -> f32 {
    let (a, b) = (sp(SPINE[0]), sp(SPINE[1]));
    a[2] + (y - a[1]) / (b[1] - a[1]) * (b[2] - a[2])
}

/// Whole-plate gear motion, `gear` 1 the source pose and 0 stowed. Each main
/// plate is one rigid body that turns about its own top edge (a line along Y)
/// and swings inward under the fuselage; the nose plate turns about a lateral
/// line through its aft top corner and swings forward and up, and steers about
/// its vertical axis. Hidden (not drawn) at gear 0 by [`Rig::animate`].
pub(crate) fn gear_positions(f: &mut Face, gear: f64, steering: f64) {
    let angle = FRAC_PI_2 * (1. - gear.clamp(0., 1.));
    let a = f.address;
    if GEAR_NOSE.contains(&a) {
        if steering != 0. {
            turn(f, sp(NOSE_STEER), sp([0., 0., 1.]), -steering);
        }
        turn(f, sp(NOSE_HINGE), sp([1., 0., 0.]), angle);
    } else if GEAR_MAIN_R.contains(&a) {
        turn(f, sp(MAIN_HINGE_R), sp([0., 1., 0.]), angle);
    } else {
        turn(f, sp(MAIN_HINGE_L), sp([0., 1., 0.]), -angle);
    }
}

pub struct Rig;
impl Rig {
    /// Checks the parsed neutral shape and the state words against the reviewed
    /// source, then adds the gear, speed brake and plume plates (each a whole
    /// reviewed quad, both windings) to the shape. Every surface is a neutral
    /// face the rig turns; the flap words and the rudder word stay pinned and
    /// never set.
    pub fn load(bytes: &[u8], mut shape: Shape) -> AppResult<(Self, Shape)> {
        if tore_formats::module::code(bytes)?.0.len() != 21488
            || shape.faces.len() != 253
            || shape.state_words != WORDS.into_iter().collect()
        {
            return Err("unreviewed F5E.SH layout".into());
        }
        let axis = crate::variety_animation::axis_scale(ID);
        for (right, left, corners) in &PLATES {
            plate(&shape, *right, corners, false)?;
            if left != right {
                plate(&shape, *left, corners, true)?;
            }
        }
        let neutral: BTreeSet<usize> = shape.faces.iter().map(|f| f.address).collect();
        // The flap words remove exactly that wing's strip and add nothing.
        // They pin the strips as the surfaces the source treats as flaps; the
        // rig never sets them.
        branch(bytes, &neutral, 0x63d8, &[], &FLAP_L)?;
        branch(bytes, &neutral, 0x63de, &[], &FLAP_R)?;
        // The rudder word swaps the quad for a copy offset toward +X.
        branch(bytes, &neutral, 0x63e4, &RUDDER_WORD, &RUDDER)?;
        // The gear, speed brake and plume words add exactly the reviewed
        // plates and remove nothing.
        branch(bytes, &neutral, 0x63cc, &GEAR, &[])?;
        branch(bytes, &neutral, 0x63c6, &BRAKE, &[])?;
        branch(bytes, &neutral, 0x63c0, &FLAME, &[])?;
        let pose = |word: usize| -> AppResult<Shape> {
            Ok(Shape::with_state(bytes, &[(word, 1)].into())?.scaled(axis))
        };
        let (gear, brake, flame) = (pose(0x63cc)?, pose(0x63c6)?, pose(0x63c0)?);
        let corners = |ids: &[usize], shape: &Shape, allowed: &[[f32; 3]]| -> AppResult<()> {
            for &address in ids {
                let face = one(shape, address)?;
                if face.positions.len() != 4
                    || !face
                        .positions
                        .iter()
                        .all(|&p| allowed.iter().any(|&q| is(p, q)))
                    || !allowed.iter().all(|&q| face.positions.contains(&sp(q)))
                {
                    return Err(format!("unreviewed F5E.SH plate {address:x}").into());
                }
            }
            Ok(())
        };
        let main = |x: f32| [[x, 6., -18.], [x, -2., -18.], [x, -2., -4.], [x, 6., -4.]];
        corners(&GEAR_MAIN_R, &gear, &main(14.))?;
        corners(&GEAR_MAIN_L, &gear, &main(-15.))?;
        corners(
            &GEAR_NOSE,
            &gear,
            &[
                [0., 53., -17.],
                [0., 53., -5.],
                [0., 60., -5.],
                [0., 60., -17.],
            ],
        )?;
        corners(
            &BRAKE,
            &brake,
            &[
                [2., 29., 10.],
                [-2., 29., 10.],
                [-2., 22., 16.],
                [2., 22., 16.],
            ],
        )?;
        // The dorsal ridge the closed brake lies on must be in the neutral shape.
        let ridge = one(&shape, SPINE_FACE)?;
        // The ridge's rear end is the pair (+/-1, -34, 4), whose mid point is
        // the second spine point.
        if !ridge.positions.contains(&sp(SPINE[0]))
            || !ridge.positions.contains(&sp([1., -34., 4.]))
            || !ridge.positions.contains(&sp([-1., -34., 4.]))
        {
            return Err("unreviewed F5E.SH spine face".into());
        }
        // The two nozzle exit octagons the engine glows are drawn on.
        for address in NOZZLES {
            let face = one(&shape, address)?;
            if face.positions.len() != 8
                || !face.positions.iter().all(|p| p[1] == sp([0., -48., 0.])[1])
            {
                return Err(format!("unreviewed F5E.SH nozzle {address:x}").into());
            }
        }
        // The plume: per engine a vertical and a horizontal plate pair, each
        // four corners from y -49 back to y -79, the left pair at x <= 0 and
        // the right at x >= 0; the nozzle exit plane (y -48) is the root.
        for (ids, left) in [(&FLAME_L, true), (&FLAME_R, false)] {
            for &address in ids {
                let face = one(&flame, address)?;
                let (lo, hi) = (sp([0., FLAME_TIP, 0.])[1], sp([0., FLAME_ROOT - 1., 0.])[1]);
                if face.positions.len() != 4
                    || face.positions.iter().any(|p| {
                        p[1] < lo || p[1] > hi || (if left { p[0] > 0. } else { p[0] < 0. })
                    })
                    || !face.positions.iter().any(|p| p[1] == lo)
                    || !face.positions.iter().any(|p| p[1] == hi)
                {
                    return Err(format!("unreviewed F5E.SH plume plate {address:x}").into());
                }
            }
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

    /// The face for the pilot's commands, or `None` where it is not drawn:
    /// the gear at gear 0, the brake at brake 0, the plume with no exhaust.
    /// Each surface is one rigid panel (both windings turn together, no
    /// vertex held off its hinge), so nothing folds.
    pub fn animate(&self, source: &Face, state: &State) -> Option<Face> {
        let a = source.address;
        let mut result = source.clone();
        if GEAR.contains(&a) {
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
        if FLAME.contains(&a) {
            if state.exhaust <= 0. {
                return None;
            }
            // Each plume grows from its nozzle exit plane; every plate scales
            // from the same root, so the shared edges stay together.
            let root = sp([0., FLAME_ROOT, 0.])[1];
            for p in &mut result.positions {
                p[1] = root + (p[1] - root) * state.exhaust.clamp(0., 1.) as f32;
            }
            return Some(result);
        }
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
            return Some(result);
        };
        if angle != 0. {
            turn(&mut result, sp(pivot), sp(axis), angle);
        }
        Some(result)
    }
}

/// True for the plume plates (the renderer draws them additively).
pub(crate) fn flame(address: usize) -> bool {
    FLAME.contains(&address)
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
                assert_eq!(rig.animate(&f, &s).unwrap().positions, f.positions);
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
                assert_eq!(
                    rig.animate(&f, &s).unwrap().positions,
                    f.positions,
                    "roll moved it"
                );
            }
        }
        for elevator in [-1., -0.5, 0.5, 1.] {
            s.elevator = elevator;
            let a = plate_face(0x3a9d, 0x3f2c, false);
            let b = plate_face(0x3a9d, 0x3f2c, true);
            let (ta, tb) = (rig.animate(&a, &s).unwrap(), rig.animate(&b, &s).unwrap());
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
                assert_eq!(
                    rig.animate(&f, &s).unwrap().positions,
                    f.positions,
                    "pitch moved it"
                );
            }
        }
        s.aileron = 1.;
        let r = plate_face(0x3b9f, 0x41ce, false);
        let l = plate_face(0x3b9f, 0x41ce, true);
        let (tr, tl) = (rig.animate(&r, &s).unwrap(), rig.animate(&l, &s).unwrap());
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
        let (tr, tl) = (rig.animate(&r, &s).unwrap(), rig.animate(&l, &s).unwrap());
        // The trailing vertex (31, -8, -4) drops; so does (-31, -8, -4).
        assert!(tr.positions[2][2] < r.positions[2][2]);
        assert!(tl.positions[2][2] < l.positions[2][2]);
        assert!((tr.positions[2][2] - tl.positions[2][2]).abs() < 1e-4);
        assert!(spread(&r, &tr) < 1e-3 && spread(&l, &tl) < 1e-3);
        // The hinge pair's midpoint at the root, (7, -4, -3.5), stays put.
        let (a, b) = (
            rig.animate(&plate_face(0x44d9, 0x45c0, false), &s).unwrap(),
            tr,
        );
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
                let t = rig.animate(&f, &s).unwrap();
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
    fn gear_plate(address: usize, points: &[[f32; 3]]) -> Face {
        face(address, points)
    }
    fn main_plate(address: usize, x: f32) -> Face {
        gear_plate(
            address,
            &[[x, 6., -18.], [x, -2., -18.], [x, -2., -4.], [x, 6., -4.]],
        )
    }
    fn nose_plate(address: usize) -> Face {
        gear_plate(
            address,
            &[
                [0., 53., -17.],
                [0., 53., -5.],
                [0., 60., -5.],
                [0., 60., -17.],
            ],
        )
    }
    fn brake_plate(address: usize) -> Face {
        gear_plate(
            address,
            &[
                [2., 29., 10.],
                [-2., 29., 10.],
                [-2., 22., 16.],
                [2., 22., 16.],
            ],
        )
    }
    fn flame_plate(address: usize) -> Face {
        gear_plate(
            address,
            &[
                [-3., -49., -3.],
                [-3., -49., 3.],
                [-3., -79., 3.],
                [-3., -79., -3.],
            ],
        )
    }
    #[test]
    fn the_gear_plates_hide_at_zero_and_show_the_source_pose_at_one() {
        let rig = Rig;
        let mut s = state();
        for plate in [
            main_plate(0x49cc, 14.),
            main_plate(0x4a37, -15.),
            nose_plate(0x4aa2),
        ] {
            s.gear = 0.;
            assert!(rig.animate(&plate, &s).is_none());
            s.gear = 1.;
            assert_eq!(rig.animate(&plate, &s).unwrap().positions, plate.positions);
        }
    }
    #[test]
    fn each_main_plate_swings_inward_about_its_top_edge_and_stows_flat_under_the_wing_root() {
        let rig = Rig;
        let mut s = state();
        for (address, x, inward) in [
            (0x49cc, 14., -1f32),
            (0x49eb, 14., -1.),
            (0x4a37, -15., 1.),
            (0x4a56, -15., 1.),
        ] {
            let plate = main_plate(address, x);
            for gear in [0.75, 0.5, 0.25, 1e-6] {
                s.gear = gear;
                let out = rig.animate(&plate, &s).unwrap();
                assert!(spread(&plate, &out) < 1e-3, "{gear} not rigid");
                // The top edge (z -4) is the hinge and stays.
                for k in [2usize, 3] {
                    for axis in 0..3 {
                        assert!(
                            (out.positions[k][axis] - plate.positions[k][axis]).abs() < 1e-3,
                            "top edge moved at {gear}"
                        );
                    }
                }
                // The feet move inward and up, never outboard or down, and
                // never fore or aft.
                for k in [0usize, 1] {
                    assert!((out.positions[k][0] - plate.positions[k][0]) * inward > 0.);
                    assert!(out.positions[k][2] >= plate.positions[k][2] - 1e-4);
                    assert!((out.positions[k][1] - plate.positions[k][1]).abs() < 1e-3);
                }
            }
            // Near stow the plate lies level at the hinge height.
            s.gear = 1e-6;
            let out = rig.animate(&plate, &s).unwrap();
            for p in &out.positions {
                assert!(
                    (p[2] - sp([0., 0., -4.])[2]).abs() < 1e-2,
                    "stowed plate not level"
                );
            }
        }
    }
    #[test]
    fn the_nose_plate_swings_forward_and_up_about_its_aft_top_corner_and_steers() {
        let rig = Rig;
        let mut s = state();
        let plate = nose_plate(0x4aa2);
        for gear in [0.75, 0.5, 0.25, 1e-6] {
            s.gear = gear;
            let out = rig.animate(&plate, &s).unwrap();
            assert!(spread(&plate, &out) < 1e-3, "{gear} not rigid");
            // The aft top corner (0, 53, -5) is the hinge and stays.
            for axis in 0..3 {
                assert!((out.positions[1][axis] - plate.positions[1][axis]).abs() < 1e-3);
            }
            // The foot (0, 53, -17) moves forward and up.
            assert!(out.positions[0][1] > plate.positions[0][1]);
            assert!(out.positions[0][2] >= plate.positions[0][2]);
        }
        // Near stow nothing hangs below the belly (z -5).
        s.gear = 1e-6;
        let out = rig.animate(&plate, &s).unwrap();
        assert!(
            out.positions
                .iter()
                .all(|p| p[2] > sp([0., 0., -5.])[2] - 1e-2)
        );
        // Steering turns the plate about its vertical axis and stays rigid.
        let mut steered = plate.clone();
        gear_positions(&mut steered, 1., 0.3);
        assert!(spread(&plate, &steered) < 1e-3);
        assert_ne!(steered.positions, plate.positions);
        // The steering axis is vertical through the plate centre (y 56.5).
        for (a, b) in steered.positions.iter().zip(&plate.positions) {
            assert!((a[2] - b[2]).abs() < 1e-4);
        }
    }
    #[test]
    fn the_speed_brake_hides_at_zero_and_closes_onto_the_dorsal_ridge() {
        let rig = Rig;
        let mut s = state();
        let plate = brake_plate(0x4802);
        s.brake = 0.;
        assert!(rig.animate(&plate, &s).is_none());
        s.brake = 1.;
        assert_eq!(rig.animate(&plate, &s).unwrap().positions, plate.positions);
        for brake in [0.75, 0.5, 0.25, 1e-6] {
            s.brake = brake;
            let out = rig.animate(&plate, &s).unwrap();
            assert!(spread(&plate, &out) < 1e-3, "{brake} not rigid");
            // The forward edge (y 29, z 10) is the hinge and stays.
            for k in [0usize, 1] {
                for axis in 0..3 {
                    assert!((out.positions[k][axis] - plate.positions[k][axis]).abs() < 1e-3);
                }
            }
            // The free edge comes down, never up.
            assert!(out.positions[2][2] <= plate.positions[2][2] + 1e-4);
        }
        s.brake = 1e-6;
        let out = rig.animate(&plate, &s).unwrap();
        let free = out.positions[2];
        assert!(
            (free[2] - spine_height_at(free[1])).abs() < 1e-2,
            "closed free edge off the ridge"
        );
    }
    #[test]
    fn the_plume_hides_with_no_exhaust_and_stretches_from_the_exit_plane() {
        let rig = Rig;
        let mut s = state();
        let plate = flame_plate(0x48a7);
        s.exhaust = 0.;
        assert!(rig.animate(&plate, &s).is_none());
        s.exhaust = 1.;
        assert_eq!(rig.animate(&plate, &s).unwrap().positions, plate.positions);
        s.exhaust = 0.5;
        let out = rig.animate(&plate, &s).unwrap();
        // Scaled about the exit plane y -48: -49 -> -48.5, -79 -> -63.5.
        assert_eq!(out.positions[0][1], sp([0., -48.5, 0.])[1]);
        assert_eq!(out.positions[2][1], sp([0., -63.5, 0.])[1]);
        // X and Z are untouched.
        assert_eq!(out.positions[0][0], plate.positions[0][0]);
        assert_eq!(out.positions[2][2], plate.positions[2][2]);
    }
    #[test]
    fn flame_lists_the_eight_plume_plates_and_nothing_else() {
        for a in FLAME {
            assert!(flame(a));
        }
        for a in GEAR.iter().chain(&BRAKE).chain(&NOZZLES) {
            assert!(!flame(*a));
        }
        assert_eq!(FLAME_L.len() + FLAME_R.len(), FLAME.len());
    }
}
