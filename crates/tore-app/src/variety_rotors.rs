//! Fitted propeller, rotor and nacelle presentation over reviewed source faces.
//! Geometry evidence and known limits: docs/spec/rotor-presentation.md.
use crate::{additional_animation::turn, flight};
use std::f64::consts::TAU;
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

/// Fitted presentation speed of the fixed-wing aircraft's propellers,
/// revolutions per second. Constant, like a governed propeller, so the phase
/// is a plain function of the tick and a throttle change cannot move it
/// (docs/spec/rotor-presentation.md). Rotorcraft use their simulated rotor
/// speed instead ([`RotorSpin`]).
pub const REVOLUTIONS_PER_SECOND: f64 = 15.;

/// Largest drawn disk tilt from the shaft, radians [longitudinal, lateral]:
/// the simulated tilt is drawn as it is up to these fitted bounds, which
/// keep the blade tips clear of the airframe (slice P7b).
pub const SINGLE_ROTOR_TILT_LIMIT: [f64; 2] = [0.25, 0.25];
/// The CH-47's canted rotor panels overlap in plan, so its drawn tilts stay
/// within fitted bounds that keep the two disks apart under any combination
/// (the animation probe's tandem gap): the part both disks share,
/// [longitudinal, lateral], and the part where they tilt opposite ways.
/// Differential lateral tilt (the pedals) brings the overlapping edges
/// together fastest.
pub const TANDEM_TILT_LIMIT: [f64; 2] = [0.03, 0.08];
pub const TANDEM_DIFFERENTIAL_TILT_LIMIT: [f64; 2] = [0.01, 0.02];
/// The V-22's proprotors.
pub const PROPROTOR_TILT_LIMIT: [f64; 2] = [0.2, 0.2];

/// A rotorcraft's drawn rotor speeds at 100 percent, revolutions per second,
/// from its flight model's rotor table (AH-64 289 rpm, Mi-24 240, CH-47 225,
/// V-22 397), and its tail rotor's from the tail rotor's tip speed and
/// radius. The blade angle is the speed times the flight's
/// `drive.rotor_turns` (seconds at 100 percent), so the drawn blades follow
/// spool-up, droop, the V-22's 84 percent in airplane mode and autorotation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotorSpin {
    pub main: f64,
    pub tail: f64,
}

impl RotorSpin {
    /// The spin of `state`'s rotors; none when its model has no rotor table.
    pub fn of(state: &flight::State) -> Option<Self> {
        let rotor = state.model().powered_lift()?.rotor?;
        let tail = rotor
            .tail_rotor
            .map_or(0., |t| t.tip_speed_fps / (TAU * t.radius_ft));
        Some(Self {
            main: rotor.rotor_speed_rpm / 60.,
            tail,
        })
    }
    /// The main rotors' blade angle, radians in 0..2 pi.
    pub fn main_angle(&self, state: &flight::State) -> f64 {
        angle(self.main, state.lift_controls.drive.rotor_turns)
    }
    /// The tail rotor's blade angle, radians in 0..2 pi.
    pub fn tail_angle(&self, state: &flight::State) -> f64 {
        angle(self.tail, state.lift_controls.drive.rotor_turns)
    }
}

/// The angle of a rotor turning `revolutions_per_second` at 100 percent
/// after `turns` seconds at 100 percent, folded into one revolution so the
/// trigonometry stays exact over a long flight.
fn angle(revolutions_per_second: f64, turns: f64) -> f64 {
    (revolutions_per_second * turns).rem_euclid(1.) * TAU
}

/// Whether `face` belongs to `faces`.
fn of(face: &Face, faces: &[usize]) -> bool {
    faces.contains(&face.address)
}

/// The V-22's nacelle and proprotor groups, left then right.
const V22_PROPROTORS: [[usize; 6]; 2] = [
    [0x3540, 0x3567, 0x358e, 0x3675, 0x369c, 0x36c3],
    [0x2fc0, 0x2fe7, 0x300e, 0x30f5, 0x311c, 0x3143],
];
const V22_NACELLES: [[usize; 24]; 2] = [
    [
        0x33f5, 0x3412, 0x342f, 0x344c, 0x3469, 0x348b, 0x34a9, 0x34cb, 0x34e9, 0x350b, 0x3540,
        0x3567, 0x358e, 0x35f7, 0x3614, 0x3631, 0x364e, 0x3675, 0x369c, 0x36c3, 0x372c, 0x3746,
        0x3760, 0x377a,
    ],
    [
        0x2e75, 0x2e92, 0x2eaf, 0x2ecc, 0x2ee9, 0x2f0b, 0x2f29, 0x2f4b, 0x2f69, 0x2f8b, 0x2fc0,
        0x2fe7, 0x300e, 0x3077, 0x3094, 0x30b1, 0x30ce, 0x30f5, 0x311c, 0x3143, 0x31ac, 0x31c6,
        0x31e0, 0x31fa,
    ],
];

/// Positions use source X/right, Y/forward, Z/up coordinates before render scale.
pub fn animate(id: AircraftId, face: &mut Face, state: &flight::State) {
    let phase = if state.engine && state.fuel > 0. {
        state.ticks as f64 * flight::DT * TAU * REVOLUTIONS_PER_SECOND
    } else {
        0.
    };
    let tilt = |rotor: usize| state.lift_controls.rotors[rotor].tilt;
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
            // Counter-clockwise from above, as the flight's rotor turns.
            if of(
                face,
                &[
                    0x3553, 0x3572, 0x3591, 0x35b7, 0x35d6, 0x35f5, 0x41e6, 0x4209, 0x422c, 0x4255,
                    0x4278, 0x429b, 0x44c8, 0x44eb, 0x450e, 0x4537, 0x455a, 0x457d,
                ],
            ) && let Some(spin) = RotorSpin::of(state)
            {
                turn(face, [0., 5., 19.], [0., 0., 1.], spin.main_angle(state));
                disk_tilt(face, [0., 5., 19.], tilt(0), SINGLE_ROTOR_TILT_LIMIT);
            }
            if of(face, &[0x4320, 0x4343, 0x4366, 0x4459, 0x447c, 0x449f])
                && let Some(spin) = RotorSpin::of(state)
            {
                turn(face, [-5., -93., 18.], [1., 0., 0.], spin.tail_angle(state));
            }
        }
        AircraftId::Mi24 => {
            // Clockwise from above, as the flight's rotor turns.
            if of(
                face,
                &[
                    0x4d5f, 0x4d8a, 0x4db9, 0x4de4, 0x4f2e, 0x4f59, 0x4f88, 0x4fb3,
                ],
            ) && let Some(spin) = RotorSpin::of(state)
            {
                turn(face, [0., -3., 23.], [0., 1., 44.], -spin.main_angle(state));
                disk_tilt(face, [0., -3., 23.], tilt(0), SINGLE_ROTOR_TILT_LIMIT);
            }
            if of(face, &[0x2f65, 0x2f8c, 0x2fab, 0x30c1, 0x30e8, 0x3107])
                && let Some(spin) = RotorSpin::of(state)
            {
                turn(
                    face,
                    [-5., -112., 27.],
                    [1., 0., 0.],
                    spin.tail_angle(state),
                );
            }
        }
        AircraftId::Ch47 => {
            // The front rotor turns counter-clockwise from above and the rear
            // clockwise, as the flight's tandem does; the front is rotor 0.
            let tilts = || tandem_tilts(tilt(0), tilt(1));
            if of(face, &[0x3ae5, 0x3c63])
                && let Some(spin) = RotorSpin::of(state)
            {
                turn(face, [0., 67., 15.], [0., 9., 137.], spin.main_angle(state));
                disk_tilt(face, [0., 67., 15.], tilts()[0], [f64::INFINITY; 2]);
            }
            if of(face, &[0x2067, 0x3e0c])
                && let Some(spin) = RotorSpin::of(state)
            {
                turn(
                    face,
                    [0., -45., 29.],
                    [0., 9., 136.],
                    -spin.main_angle(state),
                );
                disk_tilt(face, [0., -45., 29.], tilts()[1], [f64::INFINITY; 2]);
            }
        }
        AircraftId::V22 => {
            // The left proprotor turns clockwise seen from above in the hover
            // and the right counter-clockwise, as the flight's do; each is
            // drawn in its nacelle's frame (shaft forward along source Y at
            // 0 degrees), then the nacelle turns to its simulated angle.
            let main = RotorSpin::of(state).map_or(0., |spin| spin.main_angle(state));
            for (side, (hub_x, sign)) in [(-102., -1.), (102., 1.)].into_iter().enumerate() {
                if of(face, &V22_PROPROTORS[side]) {
                    let hub = [hub_x, 51., 16.];
                    turn(face, hub, [0., 1., 0.], sign * main);
                    proprotor_tilt(face, hub, tilt(side));
                }
                if of(face, &V22_NACELLES[side]) {
                    turn(
                        face,
                        [hub_x, 0., 12.],
                        [1., 0., 0.],
                        state.nacelle_degrees().to_radians(),
                    );
                }
            }
        }
        _ => {}
    }
}

/// The CH-47's front and rear disk tilts as drawn: their shared and
/// opposite parts each within the tandem bounds.
pub fn tandem_tilts(front: [f64; 2], rear: [f64; 2]) -> [[f64; 2]; 2] {
    let mut drawn = [[0.; 2]; 2];
    for axis in 0..2 {
        let common = ((front[axis] + rear[axis]) / 2.)
            .clamp(-TANDEM_TILT_LIMIT[axis], TANDEM_TILT_LIMIT[axis]);
        let opposite = ((front[axis] - rear[axis]) / 2.).clamp(
            -TANDEM_DIFFERENTIAL_TILT_LIMIT[axis],
            TANDEM_DIFFERENTIAL_TILT_LIMIT[axis],
        );
        drawn[0][axis] = common + opposite;
        drawn[1][axis] = common - opposite;
    }
    drawn
}

/// A main rotor disk tilted from its shaft (source up) by the flight's
/// `tilt` [longitudinal forward, lateral right] in radians, within `limit`:
/// a forward tilt leans the disk's lift forward, a right tilt right. Spin
/// comes first; the mast centre stays fixed. Individual blade feathering is
/// not represented by the original flat texture panels.
fn disk_tilt(face: &mut Face, hub: [f32; 3], tilt: [f64; 2], limit: [f64; 2]) {
    let [long, lat] = [
        tilt[0].clamp(-limit[0], limit[0]),
        tilt[1].clamp(-limit[1], limit[1]),
    ];
    turn(face, hub, [1., 0., 0.], -long);
    turn(face, hub, [0., 1., 0.], lat);
}

/// A V-22 proprotor disk tilted from its shaft in the nacelle's frame at 0
/// degrees (shaft forward along source Y; the flight's "forward" cyclic leans
/// it down, as `tiltrotor::frame` has it), before the nacelle turns.
fn proprotor_tilt(face: &mut Face, hub: [f32; 3], tilt: [f64; 2]) {
    let [long, lat] = [
        tilt[0].clamp(-PROPROTOR_TILT_LIMIT[0], PROPROTOR_TILT_LIMIT[0]),
        tilt[1].clamp(-PROPROTOR_TILT_LIMIT[1], PROPROTOR_TILT_LIMIT[1]),
    ];
    turn(face, hub, [1., 0., 0.], -long);
    turn(face, hub, [0., 0., 1.], -lat);
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
    fn a_throttle_change_never_moves_the_blades() {
        let source = face(0x1b44, vec![[27., 23., 6.], [37., 23., 6.]]);
        for tick in [0, 1, 7, 1200, 40_000] {
            let mut s = state();
            s.engine = true;
            s.ticks = tick;
            let mut reference = source.clone();
            s.throttle = 0.;
            animate(AircraftId::C130, &mut reference, &s);
            for throttle in [0.25, 0.5, 1., 7., -3.] {
                s.throttle = throttle;
                let mut moved = source.clone();
                animate(AircraftId::C130, &mut moved, &s);
                assert_eq!(moved.positions, reference.positions, "{tick} {throttle}");
            }
        }
    }
    #[test]
    fn blades_turn_the_same_angle_every_tick() {
        // The phase advances by one fixed step a tick, so consecutive ticks are
        // continuous through any throttle change between them.
        let source = face(0x1b44, vec![[27., 23., 6.], [37., 23., 6.]]);
        let angle = |tick: u64| {
            let mut s = state();
            s.engine = true;
            s.ticks = tick;
            let mut moved = source.clone();
            animate(AircraftId::C130, &mut moved, &s);
            let tip = moved.positions[1];
            f64::from(tip[2] - 6.).atan2(f64::from(tip[0] - 27.))
        };
        let step = TAU * REVOLUTIONS_PER_SECOND * flight::DT;
        for tick in [0u64, 10, 999] {
            // The signed turn between ticks, folded to (-pi, pi].
            let mut turned = (angle(tick + 1) - angle(tick)).rem_euclid(TAU);
            if turned > TAU / 2. {
                turned -= TAU;
            }
            assert!(
                (turned.abs() - step).abs() < 1e-3,
                "{tick}: {turned} against {step}"
            );
        }
    }
    fn powered(id: AircraftId) -> flight::State {
        flight::State::new(&tore_world::test_support::powered_profile(id), [0.; 3]).unwrap()
    }
    /// The angle of a point's offset from `hub` in the plane of two source
    /// axes, radians.
    fn angle_in(p: [f32; 3], hub: [f32; 3], x: usize, y: usize) -> f64 {
        f64::from(p[y] - hub[y]).atan2(f64::from(p[x] - hub[x]))
    }
    /// The signed turn from `a` to `b`, folded into (-pi, pi].
    fn folded(a: f64, b: f64) -> f64 {
        let turned = (b - a).rem_euclid(TAU);
        if turned > TAU / 2. {
            turned - TAU
        } else {
            turned
        }
    }

    #[test]
    fn nacelles_draw_at_the_true_nacelle_angle() {
        // 0 to 97.5 degrees of travel (slice P5), not conversion x 90.
        let mut s = powered(AircraftId::V22);
        let hub = [102., 0., 12.];
        for degrees in [0., 45., 87., 90., 97.5] {
            s.lift_controls.conversion_actual = degrees / 97.5;
            assert!((s.nacelle_degrees() - degrees).abs() < 1e-9);
            let mut nacelle = face(0x2e75, vec![hub, [102., 10., 12.]]);
            animate(AircraftId::V22, &mut nacelle, &s);
            assert_eq!(nacelle.positions[0], hub);
            let drawn = angle_in(nacelle.positions[1], hub, 1, 2).to_degrees();
            assert!((drawn - degrees).abs() < 1e-3, "{degrees}: {drawn}");
            let mut left = face(0x33f5, vec![[-102., 0., 12.], [-102., 10., 12.]]);
            animate(AircraftId::V22, &mut left, &s);
            let drawn = angle_in(left.positions[1], [-102., 0., 12.], 1, 2).to_degrees();
            assert!((drawn - degrees).abs() < 1e-3, "left {degrees}: {drawn}");
        }
        // At 90 degrees the nacelle stands vertical and its normal turns with it.
        s.lift_controls.conversion_actual = 90. / 97.5;
        let mut nacelle = face(0x2e75, vec![hub, [102., 10., 12.]]);
        animate(AircraftId::V22, &mut nacelle, &s);
        assert!(nacelle.positions[1][1].abs() < 0.001);
        assert!((nacelle.positions[1][2] - 22.).abs() < 0.001);
        assert!(nacelle.normal.unwrap()[1].abs() > 32766.);
    }

    #[test]
    fn rotor_speeds_come_from_the_flight_models_rotor_tables() {
        for (id, rpm) in [
            (AircraftId::Ah64, 289.),
            (AircraftId::Mi24, 240.),
            (AircraftId::Ch47, 225.),
            (AircraftId::V22, 397.),
        ] {
            let spin = RotorSpin::of(&powered(id)).unwrap();
            assert!((spin.main * 60. - rpm).abs() < 1e-9, "{id:?}");
        }
        // The single rotors' tail rotors turn several times faster.
        for id in [AircraftId::Ah64, AircraftId::Mi24] {
            let spin = RotorSpin::of(&powered(id)).unwrap();
            assert!(spin.tail > 3. * spin.main, "{id:?}");
        }
        assert_eq!(RotorSpin::of(&powered(AircraftId::Av8)), None);
        assert_eq!(RotorSpin::of(&state()), None);
    }

    /// The AH-64 main rotor's blade angle, counter-clockwise from above.
    fn ah64_blade(s: &flight::State) -> f64 {
        let hub = [0., 5., 19.];
        let mut blade = face(0x3553, vec![hub, [10., 5., 19.]]);
        animate(AircraftId::Ah64, &mut blade, s);
        assert_eq!(blade.positions[0], hub);
        angle_in(blade.positions[1], hub, 0, 1)
    }

    #[test]
    fn the_drawn_rotor_follows_the_simulated_rotor_speed_without_jumping() {
        let mut s = powered(AircraftId::Ah64);
        let spin = RotorSpin::of(&s).unwrap();
        assert_eq!(s.lift_controls.drive.rotor_turns, 0.);
        assert!(ah64_blade(&s).abs() < 1e-6, "turns 0 is the source pose");
        // Spool-up from rest, the governed 100 percent, a droop, the V-22's
        // 84 percent and an autorotation's overspeed, one tick each, with the
        // engine off throughout: the blades follow the rotor, not the engine.
        s.engine = false;
        let speeds: Vec<f64> = (0..=40)
            .map(|i| f64::from(i) / 40.)
            .chain([1., 1., 0.93, 0.84, 0.84, 1.05, 1.1, 0.3, 0.])
            .collect();
        let mut before = ah64_blade(&s);
        for nr in speeds {
            s.lift_controls.drive.rotor_speed = nr;
            s.lift_controls.drive.advance_turns();
            let after = ah64_blade(&s);
            let expected = TAU * spin.main * nr * flight::DT;
            assert!(
                (folded(before, after) - expected).abs() < 1e-5,
                "at {nr}: turned {} against {expected}",
                folded(before, after)
            );
            before = after;
        }
        // Stopped, the blades stay where they stopped.
        let resting = ah64_blade(&s);
        s.lift_controls.drive.advance_turns();
        assert!((ah64_blade(&s) - resting).abs() < 1e-9);
        // A long flight keeps the angle exact.
        s.lift_controls.drive.rotor_turns = 36_000.25;
        let expected = (spin.main * 36_000.25).rem_euclid(1.) * TAU;
        assert!(folded(expected, ah64_blade(&s)).abs() < 1e-4);
    }

    #[test]
    fn tail_rotors_turn_at_their_own_speed() {
        for (id, address, hub) in [
            (AircraftId::Ah64, 0x4320, [-5., -93., 18.]),
            (AircraftId::Mi24, 0x2f65, [-5., -112., 27.]),
        ] {
            let mut s = powered(id);
            let spin = RotorSpin::of(&s).unwrap();
            let blade = |s: &flight::State| {
                let mut f = face(address, vec![hub, [hub[0], hub[1] + 3., hub[2]]]);
                animate(id, &mut f, s);
                assert_eq!(f.positions[0], hub);
                angle_in(f.positions[1], hub, 1, 2)
            };
            s.lift_controls.drive.rotor_turns = 10.;
            let before = blade(&s);
            s.lift_controls.drive.advance_turns();
            let expected = TAU * spin.tail * flight::DT;
            let turned = folded(before, blade(&s));
            assert!(
                (turned - expected).abs() < 1e-4,
                "{id:?}: {turned} {expected}"
            );
        }
    }

    #[test]
    fn rotors_turn_the_way_the_flight_turns_them() {
        use tore_sim::models::variety::{RotorLayout, RotorRotation};
        // AH-64 counter-clockwise and Mi-24 clockwise from above, as their
        // rotor tables say.
        for (id, address, hub) in [
            (AircraftId::Ah64, 0x3553, [0., 5., 19.]),
            (AircraftId::Mi24, 0x4d5f, [0., -3., 23.]),
        ] {
            let mut s = powered(id);
            let RotorLayout::Single { rotation, .. } =
                s.model().powered_lift().unwrap().rotor.unwrap().layout
            else {
                panic!("{id:?} is a single rotor");
            };
            let sign = |s: &flight::State| {
                let mut f = face(address, vec![hub, [hub[0] + 10., hub[1], hub[2]]]);
                animate(id, &mut f, s);
                angle_in(f.positions[1], hub, 0, 1)
            };
            s.lift_controls.drive.rotor_turns = 0.01;
            let counter_clockwise = sign(&s) > 0.;
            assert_eq!(
                counter_clockwise,
                rotation == RotorRotation::CounterClockwise,
                "{id:?}"
            );
        }
    }

    #[test]
    fn the_tandem_rotors_counter_rotate_about_their_own_masts() {
        let mut s = powered(AircraftId::Ch47);
        let blade = |s: &flight::State, address: usize, hub: [f32; 3]| {
            let mut f = face(address, vec![hub, [hub[0] + 10., hub[1], hub[2]]]);
            animate(AircraftId::Ch47, &mut f, s);
            assert_eq!(f.positions[0], hub);
            let p = f.positions[1];
            assert!(
                (p.iter().zip(hub).map(|(a, b)| (a - b).powi(2)).sum::<f32>() - 100.).abs() < 0.001
            );
            angle_in(p, hub, 0, 1)
        };
        let (front, rear) = ((0x3ae5, [0., 67., 15.]), (0x2067, [0., -45., 29.]));
        assert!(blade(&s, front.0, front.1).abs() < 1e-6);
        s.lift_controls.drive.rotor_turns = 0.02;
        let spin = RotorSpin::of(&s).unwrap();
        let expected = TAU * spin.main * 0.02;
        // The front counter-clockwise from above, the rear clockwise (the
        // flight's tandem turns them +1 and -1).
        assert!((blade(&s, front.0, front.1) - expected).abs() < 0.01);
        assert!((blade(&s, rear.0, rear.1) + expected).abs() < 0.01);
    }

    #[test]
    fn the_disks_tilt_with_the_flights_rotor_tilt() {
        // The disk's lift direction, from a panel's normal after the pose.
        let lift = |id: AircraftId, address: usize, hub: [f32; 3], s: &flight::State| {
            let mut f = face(
                address,
                vec![
                    hub,
                    [hub[0] + 10., hub[1], hub[2]],
                    [hub[0], hub[1] + 10., hub[2]],
                ],
            );
            animate(id, &mut f, s);
            assert_eq!(f.positions[0], hub);
            let [o, a, b] = [0, 1, 2].map(|i| f.positions[i].map(f64::from));
            let u: [f64; 3] = std::array::from_fn(|k| a[k] - o[k]);
            let v: [f64; 3] = std::array::from_fn(|k| b[k] - o[k]);
            let n = [
                u[1] * v[2] - u[2] * v[1],
                u[2] * v[0] - u[0] * v[2],
                u[0] * v[1] - u[1] * v[0],
            ];
            let length = n.iter().map(|x| x * x).sum::<f64>().sqrt();
            n.map(|x| x / length)
        };
        for (id, address, hub, rotor, limit) in [
            (
                AircraftId::Ah64,
                0x3553,
                [0., 5., 19.],
                0,
                SINGLE_ROTOR_TILT_LIMIT,
            ),
            (
                AircraftId::Ch47,
                0x3ae5,
                [0., 67., 15.],
                0,
                [TANDEM_TILT_LIMIT[0] + TANDEM_DIFFERENTIAL_TILT_LIMIT[0], 0.],
            ),
            (
                AircraftId::Ch47,
                0x2067,
                [0., -45., 29.],
                1,
                [TANDEM_TILT_LIMIT[0] + TANDEM_DIFFERENTIAL_TILT_LIMIT[0], 0.],
            ),
        ] {
            let mut s = powered(id);
            let level = lift(id, address, hub, &s);
            assert!((level[2] - 1.).abs() < 1e-6);
            // Forward cyclic leans the lift forward by the tilt, right right
            // (both of the CH-47's rotors alike, within its shared bounds).
            let set = |s: &mut flight::State, tilt: [f64; 2]| {
                s.lift_controls.rotors[rotor].tilt = tilt;
                if id == AircraftId::Ch47 {
                    s.lift_controls.rotors[1 - rotor].tilt = tilt;
                }
            };
            set(&mut s, [0.02, 0.]);
            let n = lift(id, address, hub, &s);
            assert!((n[1] - 0.02f64.sin()).abs() < 1e-4, "{id:?} {n:?}");
            assert!(n[0].abs() < 1e-6);
            set(&mut s, [0., -0.03]);
            let n = lift(id, address, hub, &s);
            assert!((n[0] + 0.03f64.sin()).abs() < 1e-4, "{id:?} {n:?}");
            // Beyond the drawn bound the disk stops at it (the CH-47's front
            // disk alone: its shared and opposite parts both at their bounds).
            s.lift_controls.rotors = Default::default();
            s.lift_controls.rotors[rotor].tilt = [1., 0.];
            let n = lift(id, address, hub, &s);
            assert!((n[1] - limit[0].sin()).abs() < 1e-4, "{id:?} {n:?}");
        }
        // The AH-64's second rotor slot does not move its disk.
        let mut s = powered(AircraftId::Ah64);
        s.lift_controls.rotors[1].tilt = [0.04, 0.05];
        let n = lift(AircraftId::Ah64, 0x3553, [0., 5., 19.], &s);
        assert!((n[2] - 1.).abs() < 1e-6, "{n:?}");
        // The stick alone no longer tilts a disk: the rotor's tilt does.
        let mut s = powered(AircraftId::Ah64);
        [s.elevator, s.aileron, s.rudder] = [1., -1., 1.];
        let n = lift(AircraftId::Ah64, 0x3553, [0., 5., 19.], &s);
        assert!((n[2] - 1.).abs() < 1e-6);
    }

    #[test]
    fn the_proprotors_tilt_in_their_nacelles_frame() {
        let mut s = powered(AircraftId::V22);
        let hub = [102., 51., 16.];
        // Airplane mode: the shaft points forward and a forward cyclic leans
        // the disk down; the right proprotor is rotor 1.
        s.lift_controls.conversion_actual = 0.;
        s.lift_controls.rotors[1].tilt = [0.1, 0.];
        let mut tip = face(0x2fc0, vec![hub, [102., 61., 16.]]);
        animate(AircraftId::V22, &mut tip, &s);
        assert!((tip.positions[1][2] - (16. - 10. * 0.1f32.sin())).abs() < 1e-3);
        // In the hover the same tilt leans the disk forward.
        s.lift_controls.conversion_actual = 90. / 97.5;
        let mut tip = face(0x2fc0, vec![hub, [102., 61., 16.]]);
        animate(AircraftId::V22, &mut tip, &s);
        // The hub [102, 51, 16] stands at [102, -4, 63] with the nacelle up.
        let p = tip.positions[1];
        let shaft = [p[0] - 102., p[1] + 4., p[2] - 63.];
        assert!((shaft[1] - 10. * 0.1f32.sin()).abs() < 1e-3, "{shaft:?}");
        assert!(shaft[2] > 9.9, "{shaft:?}");
        // The left proprotor is rotor 0 and stays put.
        let mut left = face(0x3540, vec![[-102., 51., 16.], [-102., 61., 16.]]);
        s.lift_controls.conversion_actual = 0.;
        animate(AircraftId::V22, &mut left, &s);
        assert!((left.positions[1][2] - 16.).abs() < 1e-4);
    }

    #[test]
    fn the_tandem_disks_share_their_tilt_within_bounds() {
        // Opposite lateral tilts (the pedals) are bounded hardest.
        let drawn = tandem_tilts([0., 0.15], [0., -0.15]);
        assert_eq!(drawn[0][1], TANDEM_DIFFERENTIAL_TILT_LIMIT[1]);
        assert_eq!(drawn[1][1], -TANDEM_DIFFERENTIAL_TILT_LIMIT[1]);
        // A shared tilt within bounds is drawn as it is.
        assert_eq!(tandem_tilts([0.02, 0.05], [0.02, 0.05]), [[0.02, 0.05]; 2]);
        // Shared and opposite parts add.
        let drawn = tandem_tilts([0.03, 0.07], [0.01, 0.05]);
        for (a, b) in drawn.iter().flatten().zip([0.03, 0.07, 0.01, 0.05]) {
            assert!((a - b).abs() < 1e-12);
        }
    }

    #[test]
    fn helicopter_disks_stay_rigid_on_fixed_masts_under_combined_tilts_and_turns() {
        for (id, address, pivot, rotor) in [
            (AircraftId::Ah64, 0x3553, [0., 5., 19.], 0),
            (AircraftId::Mi24, 0x4d5f, [0., -3., 23.], 0),
            (AircraftId::Ch47, 0x2067, [0., -45., 29.], 1),
            (AircraftId::Ch47, 0x3ae5, [0., 67., 15.], 0),
        ] {
            let source = face(
                address,
                vec![
                    pivot,
                    [pivot[0] + 13., pivot[1], pivot[2]],
                    [pivot[0], pivot[1] + 17., pivot[2]],
                ],
            );
            for long in [-0.3, 0., 0.3] {
                for lat in [-0.3, 0., 0.3] {
                    for turns in [0., 0.013, 7.77] {
                        let mut s = powered(id);
                        s.lift_controls.drive.rotor_turns = turns;
                        s.lift_controls.rotors[rotor].tilt = [long, lat];
                        let mut moved = source.clone();
                        animate(id, &mut moved, &s);
                        assert_eq!(moved.positions[0], pivot);
                        for i in 0..3 {
                            for j in i + 1..3 {
                                let distance = |a: [f32; 3], b: [f32; 3]| {
                                    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f32>()
                                };
                                assert!(
                                    (distance(source.positions[i], source.positions[j])
                                        - distance(moved.positions[i], moved.positions[j]))
                                    .abs()
                                        < 0.003
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
