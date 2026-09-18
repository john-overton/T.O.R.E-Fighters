//! Aircraft-owned polygon groups and fitted hinges. See docs/spec/aircraft-animation.md.
use crate::additional_animation::turn;
use crate::{aircraft_animation::split_surface, flight::State};
use tore_formats::{aircraft::AircraftId as Id, shape::Face};

pub fn canopy(a: usize) -> bool {
    matches!(
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
    )
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
fn f22_flap(address: usize) -> bool {
    matches!(
        address,
        0x437f | 0x439e | 0x43fb | 0x4416 | 0x4576 | 0x4599 | 0x45f6
    )
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
        _ => None,
    }
}
/// Split across a reviewed/fitted hinge, keeping the fixed forward skin intact.
pub fn faces(id: Id, f: &Face, s: &State) -> Vec<Face> {
    if id == Id::Faxx {
        if matches!(f.address, 0x34f7 | 0x354a | 0x37ff | 0x381e | 0x3841) {
            return Vec::new();
        }
        let opening = (f64::from(side(f)) * s.rudder).clamp(0., 1.) * 0.6;
        if f22_flap(f.address) && opening > 1e-8 {
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
        let slope = if id == Id::F22 { 0.25 } else { 0. };
        let axis = if id == Id::F22 {
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
    if id == Id::F22
        && matches!(f.address, 0x35d7 | 0x3601 | 0x362f | 0x3cc0 | 0x3d03)
        && s.bay > 0.
    {
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
            if f22_flap(a) {
                panel = Some(([sign * 17., -22., 0.], 0.4 * s.flaps));
            }
            if matches!(a, 0x43bd | 0x43dc | 0x45b8 | 0x45d7) {
                panel = Some(([sign * 42., -22., 0.], roll));
            }
            if matches!(a, 0x32ba | 0x32cc | 0x35ad | 0x35c1 | 0x3987 | 0x3ba9) {
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
pub fn gear(id: Id, f: &mut Face, fraction: f64) {
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
        Id::F22 => ([13., 0., -5.], [0., 63., -9.]),
        _ => return,
    };
    if nose {
        turn(
            f,
            front,
            [1., 0., 0.],
            -std::f64::consts::FRAC_PI_2 * (1. - fraction),
        );
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
        Id::F22 => turn(
            f,
            [0., -17., 5.],
            [1., sign * 2. / 3., sign / 3.],
            0.7 * (1. - fraction),
        ),
        _ => {}
    }
}
fn bay_doors(f: &Face, fraction: f64) -> Vec<Face> {
    let mut remaining = vec![f.clone()];
    let mut result = Vec::new();
    for (left, right, hinge, angle) in [(-8., -2., -8., 1.), (2., 7., 7., -1.)] {
        let mut next = Vec::new();
        for candidate in remaining {
            let mut inside = vec![candidate];
            for (axis, bound, sign) in [(0, left, -1.), (0, right, 1.), (1, 7., -1.), (1, 51., 1.)]
            {
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
                    [hinge, 7., -9.],
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
        let mut state = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        for address in [0x34f7, 0x354a, 0x37ff, 0x381e, 0x3841] {
            let fin = face(
                address,
                vec![[13., -55., 1.], [29., -24., 36.], [14., -11., 4.]],
            );
            assert!(faces(Id::Faxx, &fin, &state).is_empty());
            assert_eq!(faces(Id::F22, &fin, &state).len(), 1);
        }
        for sign in [-1., 1.] {
            let flap = face(
                0x437f,
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
                assert_eq!(faces(Id::F22, &flap, &state).len(), 1);
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
    fn bay_doors_open_downward_keep_outer_hinges_and_preserve_skin_area() {
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
            for hinge in [-8., 7.] {
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
    fn rudder_keeps_forward_skin_fixed_and_sweep_uses_documented_schedule() {
        let mut s = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
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
        let mut s = State::new(&crate::flight::animation_tests::profile(), [0.; 3]).unwrap();
        assert_eq!(faces(Id::Mig21, &f, &s)[0].positions, f.positions);
        s.brake = 0.5;
        let moved = faces(Id::Mig21, &f, &s);
        for p in &f.positions {
            assert!(moved.iter().flat_map(|f| &f.positions).any(|v| v == p));
        }
        assert!(moved.iter().flat_map(|f| &f.positions).any(|p| p[2] < -8.1));
    }
}
