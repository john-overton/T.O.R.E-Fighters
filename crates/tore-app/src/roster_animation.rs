//! Aircraft-owned polygon groups and fitted hinges. See docs/spec/aircraft-animation.md.
use crate::additional_animation::turn;
use crate::{aircraft_animation::split_surface, flight::State};
use tore_formats::{aircraft::AircraftId as Id, shape::Face};

/// Glazing panels, one list per source shape. F-22A and F-22N remodelled the
/// canopy, so their addresses do not share.
pub fn canopy(id: Id, a: usize) -> bool {
    match id {
        Id::F22 => matches!(
            a,
            0x2136
                | 0x2153
                | 0x2170
                | 0x219c
                | 0x2220
                | 0x2255
                | 0x228a
                | 0x22a4
                | 0x22bc
                | 0x2462
                | 0x247c
                | 0x2494
                | 0x2c36
                | 0x2c6c
        ),
        Id::F22n => matches!(
            a,
            0x2215
                | 0x2232
                | 0x224f
                | 0x227b
                | 0x22ff
                | 0x2334
                | 0x2369
                | 0x2383
                | 0x239b
                | 0x2541
                | 0x255b
                | 0x2573
                | 0x2c8d
                | 0x2e57
        ),
        _ => false,
    }
}
pub fn sweep(s: &State) -> f64 {
    ((s.speed / 1.68781 - 400.) / 300.).clamp(0., 1.) * 40f64.to_radians() * (1. - s.flaps)
}
fn side(f: &Face) -> f32 {
    if f.positions.iter().map(|p| p[0]).sum::<f32>() < 0. {
        -1.
    } else {
        1.
    }
}
/// Inboard trailing-edge flaps, one list per source shape.
fn f22_flap(id: Id, address: usize) -> bool {
    match id {
        Id::F22 => matches!(
            address,
            0x437f | 0x439e | 0x43fb | 0x4416 | 0x4576 | 0x4599 | 0x45f6
        ),
        Id::F22n => matches!(
            address,
            0x4477 | 0x4496 | 0x44f3 | 0x450e | 0x466e | 0x4691 | 0x46ee
        ),
        _ => false,
    }
}
fn rudder(id: Id, a: usize) -> Option<(f32, f32)> {
    match id {
        Id::Mig29 if matches!(a, 0x334e | 0x3371 | 0x4d6c | 0x4d93) => Some((-30., 18.)),
        Id::Su27 if matches!(a, 0x17f8 | 0x1831 | 0x1ae5 | 0x1af9) => Some((-43., 21.)),
        Id::Mig21 if matches!(a, 0x1bd3 | 0x1e9a | 0x1ebd) => Some((-52., 0.)),
        Id::Su25 if matches!(a, 0x572f | 0x58ed | 0x5e68 | 0x5e8f) => Some((-53., 0.)),
        Id::Mig23 if matches!(a, 0x2a66 | 0x2a89 | 0x2b20) => Some((-21., 0.)),
        Id::Su35 if matches!(a, 0x2268 | 0x2301 | 0x26a8 | 0x4edf) => Some((-30., 20.)),
        Id::F22 if matches!(a, 0x34f7 | 0x354a | 0x381e | 0x3841) => Some((-35., 13.)),
        Id::F22n if matches!(a, 0x361d | 0x3670 | 0x3903 | 0x3926) => Some((-35., 13.)),
        _ => None,
    }
}
/// Split across a reviewed/fitted hinge, keeping the fixed forward skin intact.
pub fn faces(id: Id, f: &Face, s: &State) -> Vec<Face> {
    if id == Id::Faxx {
        if matches!(f.address, 0x361d | 0x3670 | 0x38e4 | 0x3903 | 0x3926) {
            return Vec::new();
        }
        let opening = (f64::from(side(f)) * s.rudder).clamp(0., 1.) * 0.6;
        if f22_flap(Id::F22n, f.address) && opening > 1e-8 {
            return [-opening, opening]
                .into_iter()
                .map(|angle| {
                    let mut leaf = f.clone();
                    turn(&mut leaf, [side(f) * 17., -22., 0.], [1., 0., 0.], angle);
                    leaf
                })
                .collect();
        }
    }
    let id = id.source();
    let sign = side(f);
    // No switched brake faces exist on these base shapes. These explicitly
    // fitted panels are clipped from their own belly/aft-fuselage skins.
    if id == Id::Mig21 && matches!(f.address, 0x2757 | 0x276c | 0x2817) {
        return band(f, -8., 12., [0., 12., -8.], [1., 0., 0.], 0.6 * s.brake);
    }
    if id == Id::Mig23
        && matches!(
            f.address,
            0x3109 | 0x313c | 0x31a7 | 0x339f | 0x33f8 | 0x347a
        )
    {
        return band(
            f,
            -24.,
            -16.,
            [sign * 4., -16., 0.],
            [0., 0., 1.],
            f64::from(sign) * 0.6 * s.brake,
        );
    }
    if let Some((y, x)) = rudder(id, f.address) {
        if s.rudder.abs() < 1e-8 {
            return vec![f.clone()];
        }
        let slope = if matches!(id, Id::F22 | Id::F22n) {
            0.25
        } else {
            0.
        };
        let axis = if matches!(id, Id::F22 | Id::F22n) {
            [f64::from(sign) * 0.45, -0.25, 1.]
        } else {
            [0., 0., 1.]
        };
        let length = axis.iter().map(|v| v * v).sum::<f64>().sqrt();
        return split_surface(
            f,
            [sign * x, y, 0.],
            axis.map(|v| v / length),
            0.35 * s.rudder,
            |p| p[1] - y + slope * p[2],
        );
    }
    // Belly skins the bay is cut from. The F-22N belly was remodelled, so its
    // aft-right panel 0x39dc replaces the F-22A's 0x3d03 (fitted, agent choice
    // 2026-09-22: it is the only F-22N skin at z=-9 covering the clip region's
    // right half aft of y=22).
    let bay_skin = match id {
        Id::F22 => matches!(f.address, 0x35d7 | 0x3601 | 0x362f | 0x3cc0 | 0x3d03),
        Id::F22n => matches!(f.address, 0x3755 | 0x36fd | 0x3716 | 0x39ae | 0x39dc),
        _ => false,
    };
    if bay_skin && s.bay > 0. {
        return bay_doors(f, s.bay);
    }
    if id == Id::Su25 && matches!(f.address, 0x56a9 | 0x56d6 | 0x5810 | 0x58b7) {
        return split_surface(f, [0., -54., 3.], [1., 0., 0.], -0.3 * s.elevator, |p| {
            p[1] + 54.
        });
    }
    // Su-35's neutral wing skins cross its trailing-panel boundaries. Split
    // every skin, rather than rotate an entire wing or follow polygon diagonals.
    if id == Id::Su35
        && matches!(
            f.address,
            0x244b
                | 0x246e
                | 0x24c0
                | 0x24de
                | 0x2501
                | 0x251c
                | 0x2537
                | 0x254f
                | 0x25a3
                | 0x5897
                | 0x58b6
                | 0x5152
                | 0x517b
                | 0x5198
                | 0x51b5
                | 0x51d6
                | 0x51f9
                | 0x5223
                | 0x52f0
        )
    {
        let mut result = Vec::new();
        // Divide at the aileron boundary before applying the slanted hinge.
        for panel in split_surface(f, [0.; 3], [1., 0., 0.], 0., |p| p[0].abs() - 49.) {
            let outer = panel.positions.iter().map(|p| p[0].abs()).sum::<f32>()
                / panel.positions.len() as f32
                > 49.;
            let angle = if outer {
                f64::from(sign) * 0.2 * s.aileron
            } else {
                0.4 * s.flaps
            };
            let axis = [1., -f64::from(sign) * 0.2, 0.];
            let length = 1.04f64.sqrt();
            result.extend(split_surface(
                &panel,
                [sign * 19., -4., 0.],
                axis.map(|v| v / length),
                angle,
                |p| p[1] + 4. + 0.2 * (p[0].abs() - 19.),
            ));
        }
        return result;
    }
    vec![f.clone()]
}
/// Rigid movement of already separated source panels. Each address belongs to this shape.
pub fn animate(id: Id, f: &mut Face, s: &State) {
    let id = id.source();
    let a = f.address;
    let sign = side(f);
    let roll = f64::from(sign) * 0.2 * s.aileron;
    let mut panel: Option<([f32; 3], f64)> = None;
    match id {
        Id::Mig29 => {
            if matches!(a, 0x5217 | 0x523e | 0x531e | 0x5345) {
                panel = Some(([sign * 17., -6., 1.], 0.4 * s.flaps));
            }
            if matches!(a, 0x445c | 0x44fc | 0x4b23 | 0x4c61) {
                panel = Some(([sign * 36., -8., 0.], roll));
            }
            if matches!(
                a,
                0x296b
                    | 0x2989
                    | 0x4179
                    | 0x41a1
                    | 0x474f
                    | 0x4776
                    | 0x479e
                    | 0x47c5
                    | 0x4cf6
                    | 0x4d1d
                    | 0x4d44
                    | 0x4dbc
                    | 0x4de3
                    | 0x4e0a
            ) {
                panel = Some(([sign * 17., -35., 0.], -0.3 * s.elevator + roll * 0.5));
            }
        }
        Id::Su27 => {
            if matches!(a, 0x16d5 | 0x170f | 0x1928 | 0x19be) {
                panel = Some(([sign * 21., -38., -6.], -0.3 * s.elevator + roll * 0.5));
            }
            if matches!(a, 0x2e82 | 0x2e95 | 0x2db3 | 0x2dc6) {
                turn(
                    f,
                    [sign * 21., -10., -1.],
                    [1., -sign * 0.276, 0.],
                    0.4 * s.flaps + roll,
                );
                return;
            }
        }
        Id::Mig21 => {
            if matches!(a, 0x2b1b | 0x2e6b) {
                panel = Some(([sign * 7., -18., -2.], 0.4 * s.flaps));
            }
            if matches!(a, 0x2b36 | 0x2e86) {
                panel = Some(([sign * 25., -15., -2.], roll));
            }
            if matches!(
                a,
                0x2aae | 0x2ac9 | 0x2b51 | 0x2b6c | 0x2d5b | 0x2d7e | 0x2e50 | 0x2ea1
            ) {
                panel = Some(([sign * 6., -47., -2.], -0.3 * s.elevator));
            }
        }
        Id::Su25 => {
            if matches!(a, 0x5fbf | 0x5fde | 0x60a1 | 0x60c0) {
                panel = Some(([sign * 14., -7., 5.], 0.4 * s.flaps));
            }
            if matches!(a, 0x4b9d | 0x4c10 | 0x4bed | 0x4f0e | 0x4fb4 | 0x4f5e) {
                panel = Some(([sign * 42., -8., 3.], roll));
            }
        }
        Id::Mig23 => {
            if matches!(a, 0x3543 | 0x356b | 0x3730 | 0x3758) {
                panel = Some(([sign * 3., -23., 1.], -0.3 * s.elevator + roll * 0.5));
            }
            if matches!(a, 0x3e7e | 0x3ea5 | 0x4175 | 0x4194) {
                panel = Some(([sign * 7., -3., 3.], 0.4 * s.flaps));
            }
        }
        Id::Su35 => {
            if matches!(a, 0x25f2 | 0x2639 | 0x5312 | 0x535e) {
                panel = Some(([sign * 19., -40., -5.], -0.3 * s.elevator + roll * 0.5));
            }
            if matches!(a, 0x256d | 0x2580 | 0x52b0 | 0x52d7) {
                panel = Some(([sign * 20., 43., 0.], 0.25 * s.elevator));
            }
        }
        Id::F22 => {
            if f22_flap(id, a) {
                panel = Some(([sign * 17., -22., 0.], 0.4 * s.flaps));
            }
            if matches!(a, 0x43bd | 0x43dc | 0x45b8 | 0x45d7) {
                panel = Some(([sign * 42., -22., 0.], roll));
            }
            if matches!(a, 0x32ba | 0x32cc | 0x35ad | 0x35c1 | 0x3987 | 0x3ba9) {
                panel = Some(([sign * 18., -48., 1.], -0.3 * s.elevator + roll * 0.5));
            }
        }
        Id::F22n => {
            if f22_flap(id, a) {
                panel = Some(([sign * 17., -22., 0.], 0.4 * s.flaps));
            }
            if matches!(a, 0x44b5 | 0x44d4 | 0x46b0 | 0x46cf) {
                panel = Some(([sign * 42., -22., 0.], roll));
            }
            if matches!(a, 0x31eb | 0x31fd | 0x36d3 | 0x36e7 | 0x3ada | 0x3cff) {
                panel = Some(([sign * 18., -48., 1.], -0.3 * s.elevator + roll * 0.5));
            }
        }
        _ => {}
    }
    if let Some((pivot, angle)) = panel {
        turn(f, pivot, [1., 0., 0.], angle);
    }
    if id == Id::Mig23 && (0x3c9f..=0x4194).contains(&a) {
        turn(
            f,
            [sign * 9., 4., 3.],
            [0., 0., 1.],
            -f64::from(sign) * sweep(s),
        );
    }
}
/// Pivots are in each imported shape's coordinates, not borrowed from a different plane.
pub fn gear(id: Id, f: &mut Face, fraction: f64, steering: f64) {
    let id = id.source();
    let sign = side(f);
    let forward = f.positions.iter().map(|p| p[1]).sum::<f32>() / f.positions.len() as f32;
    let nose = forward > 20.;
    let (main, front) = match id {
        Id::Mig29 => ([17., 1., 0.], [0., 47., -2.]),
        Id::Su27 => ([23., -5., 0.], [0., 60., -3.]),
        Id::Mig21 => ([20., 0., -1.], [0., 50., -7.]),
        Id::Su25 => ([10., 0., -9.], [0., 40., -9.]),
        Id::Mig23 => ([3., -1., -3.], [0., 27., -3.]),
        Id::Su35 => ([23., 3., -1.], [0., 56., -1.]),
        Id::F22 | Id::F22n => ([13., 0., -5.], [0., 63., -9.]),
        _ => return,
    };
    if nose {
        turn(
            f,
            front,
            [1., 0., 0.],
            -std::f64::consts::FRAC_PI_2 * (1. - fraction),
        );
        if crate::additional_animation::steerable_nose(id, f.address) {
            turn(f, front, [0., 0., 1.], -steering);
        }
    } else {
        turn(
            f,
            [sign * main[0], main[1], main[2]],
            [0., 1., 0.],
            f64::from(sign) * std::f64::consts::FRAC_PI_2 * (1. - fraction),
        );
    }
}
pub fn brake(id: Id, f: &mut Face, fraction: f64) {
    let id = id.source();
    let sign = side(f);
    match id {
        Id::Mig29 => {
            let upper = matches!(f.address, 0x5478 | 0x548f);
            let (pivot, angle) = if upper {
                ([0., -7., 7.], (9f64 / 7.).atan())
            } else {
                ([0., -8., -1.], -(8f64 / 6.).atan())
            };
            turn(f, pivot, [1., 0., 0.], angle * (1. - fraction));
        }
        Id::Su27 => turn(
            f,
            [0., 40., 11.],
            [1., 0., 0.],
            (14f64 / 22.).atan() * (1. - fraction),
        ),
        Id::Su35 => turn(
            f,
            [0., 23., 8.],
            [1., 0., 0.],
            (10f64 / 21.).atan() * (1. - fraction),
        ),
        Id::Su25 => {
            let upper = matches!(f.address, 0x6254 | 0x626b | 0x62b0 | 0x62c7);
            let angle = if upper {
                (5f64 / 6.).atan()
            } else {
                -(4f64 / 6.).atan()
            };
            turn(
                f,
                [sign * 75., -8., 1.],
                [1., 0., 0.],
                angle * (1. - fraction),
            );
        }
        Id::F22 | Id::F22n => turn(
            f,
            [0., -17., 5.],
            [1., sign * 2. / 3., sign / 3.],
            0.7 * (1. - fraction),
        ),
        _ => {}
    }
}
/// F-22 main bays either side of the keel, as the source's open-bay walls
/// place them: each bay's edges carry the hinges of its two doors.
const BAYS: [(f32, f32); 2] = [(-8., -2.), (2., 7.)];
const BAY_AFT: f32 = 7.;
const BAY_FORE: f32 = 51.;
const BELLY: f32 = -9.;
/// The source's open-bay walls hang this far below the belly; the fitted bay
/// recess is as deep, and the doors close over it.
const BAY_DEPTH: f32 = 3.;

fn bay_doors(f: &Face, fraction: f64) -> Vec<Face> {
    let mut remaining = vec![f.clone()];
    let mut result = Vec::new();
    // Two doors per bay, each half its width, hinged at the bay's edges.
    let doors = BAYS.into_iter().flat_map(|(left, right)| {
        let middle = (left + right) / 2.;
        [(left, middle, left, 1.), (middle, right, right, -1.)]
    });
    for (left, right, hinge, angle) in doors {
        let mut next = Vec::new();
        for candidate in remaining {
            let mut inside = vec![candidate];
            for (axis, bound, sign) in [
                (0, left, -1.),
                (0, right, 1.),
                (1, BAY_AFT, -1.),
                (1, BAY_FORE, 1.),
            ] {
                let mut clipped = Vec::new();
                for polygon in inside {
                    for part in split_surface(&polygon, [0.; 3], [1., 0., 0.], 0., |p| {
                        sign * (p[axis] - bound)
                    }) {
                        let mean = part
                            .positions
                            .iter()
                            .map(|p| sign * (p[axis] - bound))
                            .sum::<f32>()
                            / part.positions.len() as f32;
                        if mean > 0. {
                            next.push(part);
                        } else {
                            clipped.push(part);
                        }
                    }
                }
                inside = clipped;
            }
            for mut door in inside {
                turn(
                    &mut door,
                    [hinge, BAY_AFT, BELLY],
                    [0., 1., 0.],
                    angle * std::f64::consts::FRAC_PI_2 * fraction,
                );
                result.push(door);
            }
        }
        remaining = next;
    }
    result.extend(remaining);
    result
}

/// The source's open-bay pose, recast as the lining of a recess behind the
/// animated doors (fitted, agent choice 2026-09-28). The source hangs a
/// two-sided wall at each door hinge and, on F-22A, a textured bay interior
/// just below the belly; drawn as they are, they read as a second set of open
/// doors over a flat panel. Each wall's bay-facing side is raised into the
/// fuselage as a side of the recess and closes its half of the recess ends; the
/// interior becomes the recess ceiling. F-22N has no interior, and its texture
/// repaints that art, so its walls also roof the recess in their own grey.
pub fn bay_lining(id: Id, f: &Face) -> Vec<Face> {
    let count = f.positions.len() as f32;
    let x = f.positions.iter().map(|p| p[0]).sum::<f32>() / count;
    let Some((left, right)) = BAYS
        .into_iter()
        .find(|(left, right)| (left - 0.5..=right + 0.5).contains(&x))
    else {
        return Vec::new();
    };
    let top = BELLY + BAY_DEPTH;
    // Normal storage is right/up/forward.
    let Some(normal) = f.normal else {
        return Vec::new();
    };
    if normal[1].abs() > normal[0].abs() {
        let mut ceiling = f.clone();
        for p in &mut ceiling.positions {
            p[0] = if (p[0] - left).abs() < (p[0] - right).abs() {
                left
            } else {
                right
            };
            p[1] = if p[1] < (BAY_AFT + BAY_FORE) / 2. {
                BAY_AFT
            } else {
                BAY_FORE
            };
            p[2] = top;
        }
        return vec![ceiling];
    }
    let middle = (left + right) / 2.;
    // Only the side facing into the bay lines the recess.
    if normal[0].signum() != (middle - x).signum() {
        return Vec::new();
    }
    let panel = |positions: [[f32; 3]; 4], normal: [f32; 3]| {
        let mut panel = f.clone();
        panel.colors = vec![f.colors[0]; 4];
        panel.uv = vec![f.uv.first().copied().unwrap_or_default(); 4];
        panel.positions = positions.to_vec();
        panel.normal = Some(normal);
        panel
    };
    let mut lining = vec![
        panel(
            [
                [x, BAY_AFT, BELLY],
                [x, BAY_FORE, BELLY],
                [x, BAY_FORE, top],
                [x, BAY_AFT, top],
            ],
            normal,
        ),
        panel(
            [
                [x, BAY_AFT, BELLY],
                [x, BAY_AFT, top],
                [middle, BAY_AFT, top],
                [middle, BAY_AFT, BELLY],
            ],
            [0., 0., 32765.],
        ),
        panel(
            [
                [x, BAY_FORE, BELLY],
                [middle, BAY_FORE, BELLY],
                [middle, BAY_FORE, top],
                [x, BAY_FORE, top],
            ],
            [0., 0., -32765.],
        ),
    ];
    if id == Id::F22n {
        lining.push(panel(
            [
                [x, BAY_AFT, top],
                [x, BAY_FORE, top],
                [middle, BAY_FORE, top],
                [middle, BAY_AFT, top],
            ],
            [0., -32765., 0.],
        ));
    }
    lining
}

fn band(f: &Face, rear: f32, front: f32, pivot: [f32; 3], axis: [f32; 3], angle: f64) -> Vec<Face> {
    if angle.abs() < 1e-8 {
        return vec![f.clone()];
    }
    let mut fixed = Vec::new();
    let mut inside = vec![f.clone()];
    for (bound, sign) in [(rear, -1.), (front, 1.)] {
        let mut next = Vec::new();
        for polygon in inside {
            for part in split_surface(&polygon, [0.; 3], [1., 0., 0.], 0., |p| {
                sign * (p[1] - bound)
            }) {
                if part
                    .positions
                    .iter()
                    .map(|p| sign * (p[1] - bound))
                    .sum::<f32>()
                    > 0.
                {
                    fixed.push(part);
                } else {
                    next.push(part);
                }
            }
        }
        inside = next;
    }
    for mut panel in inside {
        turn(&mut panel, pivot, axis, angle);
        fixed.push(panel);
    }
    fixed
}

#[cfg(test)]
mod tests {
    use super::*;
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            colors: vec![1; positions.len()],
            uv: positions.iter().map(|p| [p[0], p[1]]).collect(),
            positions,
            address,
            texture: String::new(),
            subtype: 0,
            normal: Some([0., 1., 0.]),
            fog: Default::default(),
        }
    }
    #[test]
    fn faxx_hides_fins_and_opens_only_commanded_flap_about_fixed_hinge() {
        let mut state = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        let fin_shape = vec![[13., -55., 1.], [29., -24., 36.], [14., -11., 4.]];
        for address in [0x361d, 0x3670, 0x38e4, 0x3903, 0x3926] {
            let fin = face(address, fin_shape.clone());
            assert!(faces(Id::Faxx, &fin, &state).is_empty());
            assert_eq!(faces(Id::F22n, &fin, &state).len(), 1);
        }
        // The F-22A keeps every fin it always had.
        for address in [0x34f7, 0x354a, 0x37ff, 0x381e, 0x3841] {
            let fin = face(address, fin_shape.clone());
            assert_eq!(faces(Id::F22, &fin, &state).len(), 1);
        }
        for sign in [-1., 1.] {
            let flap = face(
                0x4477,
                vec![
                    [sign * 17., -22., 0.],
                    [sign * 20., -22., 0.],
                    [sign * 20., -32., 0.],
                ],
            );
            assert_eq!(faces(Id::Faxx, &flap, &state)[0].positions, flap.positions);
            for demand in [0.5, 1.] {
                state.rudder = f64::from(sign) * demand;
                let leaves = faces(Id::Faxx, &flap, &state);
                assert_eq!(leaves.len(), 2);
                for (leaf, direction) in leaves.iter().zip([-1., 1.]) {
                    assert_eq!(leaf.positions[0], flap.positions[0]);
                    assert_eq!(leaf.positions[1], flap.positions[1]);
                    assert_eq!(leaf.uv, flap.uv);
                    let angle = direction * 0.6 * demand;
                    assert!((f64::from(leaf.positions[2][2]) + 10. * angle.sin()).abs() < 1e-5);
                    assert!(
                        (f64::from(leaf.positions[2][1]) + 22. + 10. * angle.cos()).abs() < 1e-5
                    );
                }
                assert_eq!(faces(Id::F22n, &flap, &state).len(), 1);
                state.rudder = -state.rudder;
                assert_eq!(faces(Id::Faxx, &flap, &state).len(), 1);
            }
            state.rudder = 0.;
        }
    }
    #[test]
    fn clamshell_brakes_close_together_and_keep_their_forward_edge() {
        for side in [-1., 1.] {
            for (address, z) in [(0x6254, 11.), (0x6282, -7.)] {
                let mut f = face(
                    address,
                    vec![
                        [side * 75., -8., 1.],
                        [side * 76., -8., 1.],
                        [side * 76., -20., z],
                    ],
                );
                let original = f.clone();
                brake(Id::Su25, &mut f, 0.);
                assert_eq!(f.positions[0], original.positions[0]);
                assert_eq!(f.positions[1], original.positions[1]);
                assert!((f.positions[2][2] - 1.).abs() < 1e-5);
                let mut half = original.clone();
                brake(Id::Su25, &mut half, 0.5);
                assert!((half.positions[2][2] - 1.).abs() < (z - 1.).abs());
                assert!((half.positions[2][2] - 1.).abs() > 0.1);
            }
        }
    }
    #[test]
    fn bay_doors_open_downward_on_their_edge_hinges_and_preserve_skin_area() {
        let source = face(
            0x35d7,
            vec![
                [-10., 0., -9.],
                [10., 0., -9.],
                [10., 55., -9.],
                [-10., 55., -9.],
            ],
        );
        let area = |f: &Face| -> f32 {
            (1..f.positions.len() - 1)
                .map(|i| {
                    let a: [f32; 3] =
                        std::array::from_fn(|j| f.positions[i][j] - f.positions[0][j]);
                    let b: [f32; 3] =
                        std::array::from_fn(|j| f.positions[i + 1][j] - f.positions[0][j]);
                    let cross = [
                        a[1] * b[2] - a[2] * b[1],
                        a[2] * b[0] - a[0] * b[2],
                        a[0] * b[1] - a[1] * b[0],
                    ];
                    cross.iter().map(|v| v * v).sum::<f32>().sqrt() * 0.5
                })
                .sum()
        };
        for fraction in [0., 0.5, 1.] {
            let pieces = bay_doors(&source, fraction);
            assert!((pieces.iter().map(area).sum::<f32>() - 1100.).abs() < 0.01);
            assert!(
                pieces
                    .iter()
                    .flat_map(|f| &f.positions)
                    .all(|p| p[2] <= -9. + 1e-5)
            );
            for hinge in [-8., -2., 2., 7.] {
                assert!(
                    pieces
                        .iter()
                        .flat_map(|f| &f.positions)
                        .any(|p| (p[0] - hinge).abs() < 1e-5
                            && (p[1] - 7.).abs() < 1e-5
                            && (p[2] + 9.).abs() < 1e-5)
                );
            }
            for f in pieces {
                assert_eq!(f.positions.len(), f.uv.len());
                assert!(
                    f.uv.iter()
                        .all(|p| p[0] >= -10. && p[0] <= 10. && p[1] >= 0. && p[1] <= 55.)
                );
            }
        }
    }
    #[test]
    fn bay_walls_line_a_closed_recess_and_the_interior_becomes_its_ceiling() {
        let wall = |x: f32, normal: f32| Face {
            normal: Some([normal, 0., 0.]),
            ..face(
                0x4bcb,
                vec![[x, 51., -9.], [x, 51., -12.], [x, 7., -12.], [x, 7., -9.]],
            )
        };
        // Facing the bay: side wall, both end-wall halves, and the F-22N roof.
        let inward = bay_lining(Id::F22, &wall(-8., 32765.));
        assert_eq!(inward.len(), 3);
        assert_eq!(bay_lining(Id::F22n, &wall(-8., 32765.)).len(), 4);
        assert!(
            inward
                .iter()
                .flat_map(|f| &f.positions)
                .all(|p| (-9. ..=-6.).contains(&p[2]) && (-8. ..=-5.).contains(&p[0]))
        );
        // The side facing away would sit inside the fuselage.
        assert!(bay_lining(Id::F22, &wall(-8., -32765.)).is_empty());
        assert!(bay_lining(Id::F22, &wall(7., 32765.)).is_empty());
        assert_eq!(bay_lining(Id::F22, &wall(7., -32765.)).len(), 3);
        let interior = Face {
            normal: Some([0., -32765., 0.]),
            ..face(
                0x49da,
                vec![
                    [-3., 8., -10.],
                    [-3., 52., -10.],
                    [-8., 52., -10.],
                    [-8., 8., -10.],
                ],
            )
        };
        let ceiling = bay_lining(Id::F22, &interior);
        assert_eq!(
            ceiling[0].positions,
            vec![
                [-2., 7., -6.],
                [-2., 51., -6.],
                [-8., 51., -6.],
                [-8., 7., -6.]
            ]
        );
        assert_eq!(ceiling[0].uv, interior.uv);
    }
    #[test]
    fn rudder_keeps_forward_skin_fixed_and_sweep_uses_documented_schedule() {
        let mut s = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        let f = face(
            0x1bd3,
            vec![
                [0., -60., 0.],
                [0., -45., 0.],
                [0., -45., 20.],
                [0., -60., 20.],
            ],
        );
        s.rudder = 1.;
        let moved = faces(Id::Mig21, &f, &s);
        for p in [[0., -45., 0.], [0., -45., 20.]] {
            assert!(moved.iter().flat_map(|f| &f.positions).any(|v| *v == p));
        }
        assert!(
            moved
                .iter()
                .flat_map(|f| &f.positions)
                .any(|p| p[0].abs() > 1.)
        );
        for (knots, angle) in [
            (300., 0.),
            (400., 0.),
            (550., 20.),
            (700., 40.),
            (900., 40.),
        ] {
            s.flaps = 0.;
            s.speed = knots * 1.68781;
            assert!((sweep(&s).to_degrees() - angle).abs() < 1e-9);
            s.flaps = 1.;
            assert_eq!(sweep(&s), 0.);
        }
    }
    #[test]
    fn brake_band_moves_only_reviewed_strip() {
        let f = face(
            0x2757,
            vec![
                [-2., -20., -8.],
                [2., -20., -8.],
                [2., 20., -8.],
                [-2., 20., -8.],
            ],
        );
        let mut s = State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap();
        assert_eq!(faces(Id::Mig21, &f, &s)[0].positions, f.positions);
        s.brake = 0.5;
        let moved = faces(Id::Mig21, &f, &s);
        for p in &f.positions {
            assert!(moved.iter().flat_map(|f| &f.positions).any(|v| v == p));
        }
        assert!(moved.iter().flat_map(|f| &f.positions).any(|p| p[2] < -8.1));
    }
}
