//! Fitted propeller, rotor and nacelle presentation over reviewed source faces.
//! Geometry evidence and known limits: docs/spec/rotor-presentation.md.
use crate::{additional_animation::turn, flight};
use std::f64::consts::{FRAC_PI_2, TAU};
use tore_formats::{aircraft::AircraftId, shape::Face};

/// Select one original blade image phase per facing panel. The bounded shape
/// decoder exposes all coincident phases; drawing all of them overlays blur art.
/// Frame zero is a fitted presentation choice, not recovered retail sequencing.
pub fn keep_face(id: AircraftId, address: usize) -> bool {
    let alternatives: &[usize] = match id {
        AircraftId::C130 => &[
            0x1b63, 0x1b82, 0x1ba1, 0x1be8, 0x1c07, 0x1c26, 0x206d, 0x208c, 0x20ab, 0x20f2, 0x2111,
            0x2130, 0x23a4, 0x23c3, 0x23e2, 0x2429, 0x2448, 0x2467, 0x2664, 0x2683, 0x26a2, 0x26e9,
            0x2708, 0x2727,
        ],
        AircraftId::Ac130 => &[
            0x2b37, 0x2b89, 0x2bdb, 0x2c2d, 0x2c7f, 0x2cd1, 0x2d1e, 0x2d66, 0x2e44, 0x2e96, 0x2ee8,
            0x2f3a, 0x2f8c, 0x2fde, 0x302b, 0x3073, 0x3fa1, 0x4097, 0x413b, 0x4188, 0x42ae, 0x43a4,
            0x4448, 0x4495, 0x3ff3, 0x4045, 0x40e9, 0x41d0, 0x4300, 0x4352, 0x43f6, 0x44dd,
        ],
        AircraftId::E2 => &[
            0x46cc, 0x46ef, 0x473c, 0x475f, 0x48e1, 0x4904, 0x49b4, 0x49d7,
        ],
        AircraftId::V22 => &[
            0x3567, 0x358e, 0x369c, 0x36c3, 0x2fe7, 0x300e, 0x311c, 0x3143,
        ],
        AircraftId::Ah64 => &[
            0x3572, 0x3591, 0x35d6, 0x35f5, 0x4209, 0x422c, 0x4278, 0x429b, 0x44eb, 0x450e, 0x455a,
            0x457d, 0x4343, 0x4366, 0x447c, 0x449f,
        ],
        AircraftId::Mi24 => &[
            0x4d8a, 0x4de4, 0x4f59, 0x4fb3, 0x2f8c, 0x2fab, 0x30e8, 0x3107,
        ],
        AircraftId::Ch47 => &[
            0x2092, 0x20bd, 0x20e8, 0x3b10, 0x3b3b, 0x3b66, 0x3c8e, 0x3cb9, 0x3ce4, 0x3e37, 0x3e62,
            0x3e8d,
        ],
        _ => &[],
    };
    !alternatives.contains(&address)
}

/// Positions use source X/right, Y/forward, Z/up coordinates before render scale.
pub fn animate(id: AircraftId, face: &mut Face, state: &flight::State) {
    let phase = if state.engine && state.fuel > 0. {
        state.ticks as f64 * flight::DT * TAU * (5. + 15. * state.throttle.clamp(0., 1.))
    } else {
        0.
    };
    match id {
        AircraftId::C130 => {
            if [
                0x1b44, 0x1b63, 0x1b82, 0x1ba1, 0x1bc9, 0x1be8, 0x1c07, 0x1c26,
            ]
            .contains(&face.address)
            {
                turn(face, [27., 23., 6.], [0., 1., 0.], phase * 1.);
            }
            if [
                0x204e, 0x206d, 0x208c, 0x20ab, 0x20d3, 0x20f2, 0x2111, 0x2130,
            ]
            .contains(&face.address)
            {
                turn(face, [-27., 23., 5.5], [0., 1., 0.], phase * 1.);
            }
            if [
                0x2385, 0x23a4, 0x23c3, 0x23e2, 0x240a, 0x2429, 0x2448, 0x2467,
            ]
            .contains(&face.address)
            {
                turn(face, [-53., 23., 5.5], [0., 1., 0.], phase * 1.);
            }
            if [
                0x2645, 0x2664, 0x2683, 0x26a2, 0x26ca, 0x26e9, 0x2708, 0x2727,
            ]
            .contains(&face.address)
            {
                turn(face, [53., 23., 6.], [0., 1., 0.], phase * 1.);
            }
        }
        AircraftId::Ac130 => {
            if [
                0x2b10, 0x2b37, 0x2c06, 0x2c2d, 0x2c58, 0x2c7f, 0x2d44, 0x2d66, 0x2e1d, 0x2e44,
                0x2f13, 0x2f3a, 0x2f65, 0x2f8c, 0x3051, 0x3073,
            ]
            .contains(&face.address)
            {
                turn(face, [-52., 14., 2.5], [0., 1., 0.], phase * 1.);
            }
            if [
                0x2b62, 0x2b89, 0x2bb4, 0x2bdb, 0x2caa, 0x2cd1, 0x2cfc, 0x2d1e, 0x2e6f, 0x2e96,
                0x2ec1, 0x2ee8, 0x2fb7, 0x2fde, 0x3009, 0x302b,
            ]
            .contains(&face.address)
            {
                turn(face, [-26., 14., 2.5], [0., 1., 0.], phase * 1.);
            }
            if [
                0x3fcc, 0x3ff3, 0x401e, 0x4045, 0x40c2, 0x40e9, 0x41ae, 0x41d0, 0x42d9, 0x4300,
                0x432b, 0x4352, 0x43cf, 0x43f6, 0x44bb, 0x44dd,
            ]
            .contains(&face.address)
            {
                turn(face, [26., 14., 2.5], [0., 1., 0.], phase * 1.);
            }
            if [
                0x3f7a, 0x3fa1, 0x4070, 0x4097, 0x4114, 0x413b, 0x4166, 0x4188, 0x4287, 0x42ae,
                0x437d, 0x43a4, 0x4421, 0x4448, 0x4473, 0x4495,
            ]
            .contains(&face.address)
            {
                turn(face, [52., 14., 2.5], [0., 1., 0.], phase * 1.);
            }
        }
        AircraftId::E2 => {
            if [0x46a9, 0x46cc, 0x46ef, 0x4719, 0x473c, 0x475f].contains(&face.address) {
                turn(face, [-16., 18., -1.], [0., 1., 0.], phase * 1.);
            }
            if [0x48be, 0x48e1, 0x4904, 0x4991, 0x49b4, 0x49d7].contains(&face.address) {
                turn(face, [17., 18., -1.], [0., 1., 0.], phase * 1.);
            }
        }
        AircraftId::Ah64 => {
            if [
                0x3553, 0x3572, 0x3591, 0x35b7, 0x35d6, 0x35f5, 0x41e6, 0x4209, 0x422c, 0x4255,
                0x4278, 0x429b, 0x44c8, 0x44eb, 0x450e, 0x4537, 0x455a, 0x457d,
            ]
            .contains(&face.address)
            {
                turn(face, [0., 5., 19.], [0., 0., 1.], phase * 1.);
            }
            if [0x4320, 0x4343, 0x4366, 0x4459, 0x447c, 0x449f].contains(&face.address) {
                turn(face, [-5., -93., 18.], [1., 0., 0.], phase * 3.);
            }
        }
        AircraftId::Mi24 => {
            if [
                0x4d5f, 0x4d8a, 0x4db9, 0x4de4, 0x4f2e, 0x4f59, 0x4f88, 0x4fb3,
            ]
            .contains(&face.address)
            {
                turn(face, [0., -3., 23.], [0., 0., 1.], phase * 1.);
            }
            if [0x2f65, 0x2f8c, 0x2fab, 0x30c1, 0x30e8, 0x3107].contains(&face.address) {
                turn(face, [-5., -112., 27.], [1., 0., 0.], phase * 3.);
            }
        }
        AircraftId::Ch47 => {
            if [0x2067, 0x3e0c].contains(&face.address) {
                turn(face, [0., -45., 29.], [0., 0., 1.], phase);
            }
            if [0x3ae5, 0x3c63].contains(&face.address) {
                turn(face, [0., 67., 15.], [0., 0., 1.], -phase);
            }
        }
        AircraftId::V22 => {
            if [0x3540, 0x3567, 0x358e, 0x3675, 0x369c, 0x36c3].contains(&face.address) {
                turn(face, [-102., 51., 16.], [0., 1., 0.], -phase);
            }
            if [0x2fc0, 0x2fe7, 0x300e, 0x30f5, 0x311c, 0x3143].contains(&face.address) {
                turn(face, [102., 51., 16.], [0., 1., 0.], phase * 1.);
            }
            if [
                0x33f5, 0x3412, 0x342f, 0x344c, 0x3469, 0x348b, 0x34a9, 0x34cb, 0x34e9, 0x350b,
                0x3540, 0x3567, 0x358e, 0x35f7, 0x3614, 0x3631, 0x364e, 0x3675, 0x369c, 0x36c3,
                0x372c, 0x3746, 0x3760, 0x377a,
            ]
            .contains(&face.address)
            {
                turn(
                    face,
                    [-102., 0., 12.],
                    [1., 0., 0.],
                    state.lift_controls.conversion_actual.clamp(0., 1.) * FRAC_PI_2,
                );
            }
            if [
                0x2e75, 0x2e92, 0x2eaf, 0x2ecc, 0x2ee9, 0x2f0b, 0x2f29, 0x2f4b, 0x2f69, 0x2f8b,
                0x2fc0, 0x2fe7, 0x300e, 0x3077, 0x3094, 0x30b1, 0x30ce, 0x30f5, 0x311c, 0x3143,
                0x31ac, 0x31c6, 0x31e0, 0x31fa,
            ]
            .contains(&face.address)
            {
                turn(
                    face,
                    [102., 0., 12.],
                    [1., 0., 0.],
                    state.lift_controls.conversion_actual.clamp(0., 1.) * FRAC_PI_2,
                );
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn face(address: usize, positions: Vec<[f32; 3]>) -> Face {
        Face {
            positions,
            colors: vec![12; 3],
            texture: String::new(),
            uv: vec![],
            subtype: 76,
            normal: Some([0., 0., 32767.]),
            address,
            fog: Default::default(),
        }
    }
    fn state() -> flight::State {
        flight::State::new(&tore_world::test_support::profile(), [0.; 3]).unwrap()
    }
    #[test]
    fn phase_selection_preserves_one_original_image_per_rotor_panel() {
        for (id, frames) in [
            (AircraftId::C130, &[0x1b44, 0x1b63, 0x1b82, 0x1ba1][..]),
            (AircraftId::Ac130, &[0x2b10, 0x2b37][..]),
            (AircraftId::E2, &[0x46a9, 0x46cc, 0x46ef][..]),
            (AircraftId::V22, &[0x3540, 0x3567, 0x358e][..]),
            (AircraftId::Ah64, &[0x3553, 0x3572, 0x3591][..]),
            (AircraftId::Mi24, &[0x4d5f, 0x4d8a][..]),
            (AircraftId::Ch47, &[0x3ae5, 0x3b10, 0x3b3b, 0x3b66][..]),
        ] {
            assert!(keep_face(id, frames[0]));
            assert_eq!(
                frames
                    .iter()
                    .filter(|address| keep_face(id, **address))
                    .count(),
                1
            );
            assert!(keep_face(id, 0x100));
            assert!(keep_face(AircraftId::F18, frames[1]));
        }
        assert!(keep_face(AircraftId::Ch47, 0x2067));
        assert!(keep_face(AircraftId::Ch47, 0x3e0c));
        assert!(keep_face(AircraftId::Ch47, 0x3c63));
    }

    #[test]
    fn prop_rotation_keeps_hub_and_unselected_airframe_fixed() {
        let mut s = state();
        s.engine = true;
        s.throttle = 0.5;
        s.ticks = 3;
        let source = face(0x1b44, vec![[27., 23., 6.], [37., 23., 6.]]);
        let mut moved = source.clone();
        animate(AircraftId::C130, &mut moved, &s);
        assert_eq!(moved.positions[0], source.positions[0]);
        assert_ne!(moved.positions[1], source.positions[1]);
        let tip = moved.positions[1];
        assert!(((tip[0] - 27.).powi(2) + (tip[2] - 6.).powi(2) - 100.).abs() < 0.001);
        let mut fixed = source.clone();
        fixed.address = 0x100;
        animate(AircraftId::C130, &mut fixed, &s);
        assert_eq!(fixed.positions, source.positions);
        s.engine = false;
        let mut stopped = source.clone();
        animate(AircraftId::C130, &mut stopped, &s);
        assert_eq!(stopped.positions, source.positions);
    }
    #[test]
    fn nacelle_hinge_follows_actual_conversion_and_moves_normals() {
        let mut s = state();
        s.engine = false;
        s.lift_controls.conversion_actual = 1.;
        let mut nacelle = face(0x2e75, vec![[102., 0., 12.], [102., 10., 12.]]);
        animate(AircraftId::V22, &mut nacelle, &s);
        assert_eq!(nacelle.positions[0], [102., 0., 12.]);
        assert!(nacelle.positions[1][1].abs() < 0.001);
        assert!((nacelle.positions[1][2] - 22.).abs() < 0.001);
        assert!(nacelle.normal.unwrap()[1].abs() > 32766.);
    }
    #[test]
    fn tandem_front_source_group_rotates_about_its_existing_mast() {
        let mut s = state();
        s.engine = false;
        let mut blade = face(0x3ae5, vec![[0., 67., 15.], [10., 67., 15.]]);
        animate(AircraftId::Ch47, &mut blade, &s);
        assert_eq!(blade.positions[0], [0., 67., 15.]);
        assert_eq!(blade.positions[1], [10., 67., 15.]);
        s.engine = true;
        s.ticks = 1;
        animate(AircraftId::Ch47, &mut blade, &s);
        assert_eq!(blade.positions[0], [0., 67., 15.]);
        assert_ne!(blade.positions[1], [10., 67., 15.]);
        assert!(
            (blade.positions[1][0].powi(2) + (blade.positions[1][1] - 67.).powi(2) - 100.).abs()
                < 0.001
        );
    }
}
