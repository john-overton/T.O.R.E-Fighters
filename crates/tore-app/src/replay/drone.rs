//! The replay's drone camera, adapted from the free-camera viewer
//! (`terrain::Camera::step`): W A S D move, E and Q climb and descend, Shift
//! is four times faster, the mouse wheel sets the speed and a right-drag
//! turns the view. It flies free, or follows the selected aircraft at a
//! fixed offset in world axes, so the aircraft stays where it was framed
//! while the camera travels with it. It moves in real time, so a shot can be
//! framed while playback is paused. Agent design (2026-09-26).
use crate::terrain::Camera;
use std::collections::BTreeSet;

/// Slowest and fastest drone speed, feet per second: slow enough to creep
/// around a parked aircraft, fast enough to keep up with a missile.
pub const MIN_SPEED: f64 = 20.;
pub const MAX_SPEED: f64 = 5_000.;
/// Speed a new drone starts with, about 150 knots.
pub const DEFAULT_SPEED: f64 = 250.;
/// Each wheel notch changes speed by this factor.
const WHEEL_STEP: f64 = 1.25;
/// Shift multiplies the speed by this.
const FAST: f64 = 4.;
/// Closest the drone comes to the ground, feet.
const GROUND_CLEARANCE: f64 = 10.;
/// Steepest look up or down, radians (just short of straight).
const PITCH_LIMIT: f64 = 1.55;
/// A follow camera further than this from its aircraft starts over from
/// the default offset, feet.
const FOLLOW_REACH: f64 = 20_000.;
/// The default follow offset: feet behind, to the right of and above the
/// aircraft.
const BEHIND: f64 = 250.;
const BESIDE: f64 = 90.;
const ABOVE: f64 = 60.;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// Travels with the selected aircraft.
    Follow,
    /// Stays where it is flown.
    Free,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Drone {
    pub mode: Mode,
    /// World position in free mode; offset from the aircraft in follow mode.
    place: [f64; 3],
    yaw: f64,
    pitch: f64,
    /// Feet per second.
    speed: f64,
}

fn forward(yaw: f64, pitch: f64) -> [f64; 3] {
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    [sy * cp, sp, cy * cp]
}

impl Drone {
    /// A drone at `eye` looking at `target`. `anchor` is the followed
    /// aircraft's position; without one the drone flies free.
    pub fn looking(mode: Mode, eye: [f64; 3], target: [f64; 3], anchor: Option<[f64; 3]>) -> Self {
        let d: [f64; 3] = std::array::from_fn(|i| target[i] - eye[i]);
        let horizontal = d[0].hypot(d[2]);
        let mut drone = Self {
            mode: Mode::Free,
            place: eye,
            yaw: if horizontal > 1e-9 {
                d[0].atan2(d[2])
            } else {
                0.
            },
            pitch: d[1].atan2(horizontal).clamp(-PITCH_LIMIT, PITCH_LIMIT),
            speed: DEFAULT_SPEED,
        };
        drone.set_mode(mode, anchor);
        drone
    }

    /// A drone where `camera` is, keeping its view direction. A follow drone
    /// starting far from its aircraft is placed behind, to the right of and
    /// above it, `heading` being the aircraft's heading in radians: off the
    /// aircraft's own path, so it does not fly through the flares and smoke
    /// the aircraft leaves behind.
    pub fn from_camera(
        mode: Mode,
        camera: &Camera,
        anchor: Option<[f64; 3]>,
        heading: f64,
    ) -> Self {
        let eye = camera.position.map(f64::from);
        if mode == Mode::Follow
            && let Some(anchor) = anchor
            && (0..3)
                .map(|i| (eye[i] - anchor[i]).powi(2))
                .sum::<f64>()
                .sqrt()
                > FOLLOW_REACH
        {
            let (s, c) = heading.sin_cos();
            let eye = [
                anchor[0] - s * BEHIND + c * BESIDE,
                anchor[1] + ABOVE,
                anchor[2] - c * BEHIND - s * BESIDE,
            ];
            return Self::looking(mode, eye, anchor, Some(anchor));
        }
        let mut drone = Self {
            mode: Mode::Free,
            place: eye,
            yaw: f64::from(camera.yaw),
            pitch: f64::from(camera.pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT),
            speed: DEFAULT_SPEED,
        };
        drone.set_mode(mode, anchor);
        drone
    }

    /// Switches between follow and free, keeping the drone where it is. A
    /// follow drone with no aircraft to follow flies free.
    pub fn set_mode(&mut self, mode: Mode, anchor: Option<[f64; 3]>) {
        let world = self.position(anchor);
        match (mode, anchor) {
            (Mode::Follow, Some(anchor)) => {
                self.mode = Mode::Follow;
                self.place = std::array::from_fn(|i| world[i] - anchor[i]);
            }
            _ => {
                self.mode = Mode::Free;
                self.place = world;
            }
        }
    }

    /// Where the drone is, given the followed aircraft's position.
    pub fn position(&self, anchor: Option<[f64; 3]>) -> [f64; 3] {
        match (self.mode, anchor) {
            (Mode::Follow, Some(anchor)) => std::array::from_fn(|i| anchor[i] + self.place[i]),
            _ => self.place,
        }
    }

    pub fn speed(&self) -> f64 {
        self.speed
    }

    /// Mouse wheel: each notch up is 25% faster, down 20% slower.
    pub fn wheel(&mut self, notches: i32) {
        self.speed = (self.speed * WHEEL_STEP.powi(notches)).clamp(MIN_SPEED, MAX_SPEED);
    }

    /// Right-drag: turns the view by radians, right and up positive.
    pub fn look(&mut self, [yaw, pitch]: [f64; 2]) {
        self.yaw = (self.yaw + yaw).rem_euclid(std::f64::consts::TAU);
        self.pitch = (self.pitch + pitch).clamp(-PITCH_LIMIT, PITCH_LIMIT);
    }

    /// One frame of flying: `keys` holds the held movement keys (w, a, s, d,
    /// e, q), `fast` is Shift. `ground` answers the terrain height at an
    /// east and north position.
    pub fn step(
        &mut self,
        seconds: f64,
        keys: &BTreeSet<String>,
        fast: bool,
        anchor: Option<[f64; 3]>,
        ground: impl Fn(f64, f64) -> f64,
    ) {
        let seconds = seconds.clamp(0., 0.25);
        let held = |key: &str| f64::from(u8::from(keys.contains(key)));
        let (ahead, side, climb) = (
            held("w") - held("s"),
            held("d") - held("a"),
            held("e") - held("q"),
        );
        let f = forward(self.yaw, self.pitch);
        let (sy, cy) = self.yaw.sin_cos();
        let right = [cy, 0., -sy];
        let mut motion: [f64; 3] = std::array::from_fn(|i| {
            f[i] * ahead + right[i] * side + if i == 1 { climb } else { 0. }
        });
        let length = motion.iter().map(|v| v * v).sum::<f64>().sqrt();
        if length > 0. {
            let distance = self.speed * if fast { FAST } else { 1. } * seconds / length;
            motion = motion.map(|v| v * distance);
            for (place, delta) in self.place.iter_mut().zip(motion) {
                *place += delta;
            }
        }
        // Free: keep the drone itself out of the ground. Follow: the offset
        // is the pilot's framing, so only the drawn position is lifted.
        if self.mode == Mode::Free || anchor.is_none() {
            let floor = ground(self.place[0], self.place[2]) + GROUND_CLEARANCE;
            self.place[1] = self.place[1].max(floor);
        }
    }

    /// The camera the drone draws, never inside the ground.
    pub fn camera(&self, anchor: Option<[f64; 3]>, ground: impl Fn(f64, f64) -> f64) -> Camera {
        let mut position = self.position(anchor);
        position[1] = position[1].max(ground(position[0], position[2]) + GROUND_CLEARANCE);
        let mut camera = Camera::new();
        camera.position = position.map(|v| v as f32);
        camera.yaw = self.yaw as f32;
        camera.pitch = self.pitch as f32;
        camera.roll = 0.;
        camera.view_fraction = 1.;
        camera
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attitude::Basis;

    fn keys(held: &[&str]) -> BTreeSet<String> {
        held.iter().map(|k| (*k).to_owned()).collect()
    }

    fn near(a: [f64; 3], b: [f64; 3]) {
        assert!((0..3).all(|i| (a[i] - b[i]).abs() < 1e-6), "{a:?} != {b:?}");
    }

    const FLAT: fn(f64, f64) -> f64 = |_, _| 0.;

    #[test]
    fn keys_fly_along_the_view_and_shift_is_four_times_faster() {
        // Looking north-east and 30 degrees up.
        let mut drone = Drone::looking(Mode::Free, [0., 1_000., 0.], [1., 1_000., 1.], None);
        drone.look([0., 30f64.to_radians()]);
        assert_eq!(drone.speed(), DEFAULT_SPEED);
        drone.step(0.2, &keys(&["w"]), false, None, FLAT);
        let f = forward(std::f64::consts::FRAC_PI_4, 30f64.to_radians());
        near(
            drone.position(None),
            [f[0] * 50., 1_000. + f[1] * 50., f[2] * 50.],
        );
        let before = drone.position(None);
        drone.step(0.1, &keys(&["s"]), true, None, FLAT);
        near(
            drone.position(None),
            std::array::from_fn(|i| before[i] - f[i] * 100.),
        );
        // Right is horizontal, climb is straight up, and together they are
        // no faster than one key.
        let mut drone = Drone::looking(Mode::Free, [0., 1_000., 0.], [0., 1_000., 10.], None);
        drone.step(0.2, &keys(&["d", "e"]), false, None, FLAT);
        let side = 50. / 2f64.sqrt();
        near(drone.position(None), [side, 1_000. + side, 0.]);
        // Long frames are capped, as in the free-camera viewer.
        let mut drone = Drone::looking(Mode::Free, [0., 1_000., 0.], [0., 1_000., 10.], None);
        drone.step(5., &keys(&["w"]), false, None, FLAT);
        near(drone.position(None), [0., 1_000., 62.5]);
    }

    #[test]
    fn the_drone_stays_out_of_the_ground() {
        let hill = |x: f64, _: f64| if x > 50. { 900. } else { 0. };
        let mut drone = Drone::looking(Mode::Free, [0., 50., 0.], [0., 50., 10.], None);
        drone.step(0.25, &keys(&["q"]), false, None, hill);
        assert_eq!(drone.position(None)[1], GROUND_CLEARANCE);
        drone.step(0.25, &keys(&["d"]), false, None, hill);
        assert_eq!(drone.position(None)[1], 910.);
        assert_eq!(drone.camera(None, hill).position[1], 910.);
    }

    #[test]
    fn a_follow_drone_travels_with_its_aircraft() {
        let aircraft = [5_000., 8_000., 5_000.];
        let eye = [5_000., 8_100., 4_700.];
        let mut drone = Drone::looking(Mode::Follow, eye, aircraft, Some(aircraft));
        assert_eq!(drone.mode, Mode::Follow);
        near(drone.position(Some(aircraft)), eye);
        // The aircraft moves; the framing does not.
        let moved = [6_000., 7_500., 9_000.];
        near(drone.position(Some(moved)), [6_000., 7_600., 8_700.]);
        let camera = drone.camera(Some(moved), FLAT);
        let view = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.).forward;
        let to_aircraft = [0., -100., 300.].map(|v: f64| v / 100_000f64.sqrt());
        near(view, to_aircraft);
        // Flying moves the offset.
        drone.step(0.2, &keys(&["e"]), false, Some(moved), FLAT);
        near(drone.position(Some(moved)), [6_000., 7_650., 8_700.]);
        // Going free keeps the drone where it is; following again keeps it
        // there too.
        drone.set_mode(Mode::Free, Some(moved));
        near(drone.position(Some(aircraft)), [6_000., 7_650., 8_700.]);
        drone.set_mode(Mode::Follow, Some(aircraft));
        near(drone.position(Some(aircraft)), [6_000., 7_650., 8_700.]);
        // With nothing to follow it flies free.
        drone.set_mode(Mode::Follow, None);
        assert_eq!(drone.mode, Mode::Free);
        // A follow offset below the ground draws lifted without losing it.
        let mut low = Drone::looking(Mode::Follow, [0., -50., -300.], [0., 0., 0.], Some([0.; 3]));
        assert_eq!(low.camera(Some([0.; 3]), FLAT).position[1], 10.);
        low.step(0.5, &keys(&[]), false, Some([0.; 3]), FLAT);
        near(low.position(Some([0.; 3])), [0., -50., -300.]);
    }

    #[test]
    fn a_far_camera_starts_following_from_behind_the_aircraft() {
        let mut far = Camera::new();
        far.position = [0., 50_000., 0.];
        let aircraft = [100_000., 10_000., 100_000.];
        let drone = Drone::from_camera(Mode::Follow, &far, Some(aircraft), 0.);
        near(drone.position(Some(aircraft)), [100_090., 10_060., 99_750.]);
        // Facing east, right is south.
        let drone = Drone::from_camera(
            Mode::Follow,
            &far,
            Some(aircraft),
            std::f64::consts::FRAC_PI_2,
        );
        near(drone.position(Some(aircraft)), [99_750., 10_060., 99_910.]);
        // A near camera keeps its place and view.
        let mut near_camera = Camera::new();
        near_camera.position = [100_100., 10_000., 100_000.];
        near_camera.yaw = 1.;
        near_camera.pitch = -0.2;
        let drone = Drone::from_camera(Mode::Follow, &near_camera, Some(aircraft), 0.);
        near(
            drone.position(Some(aircraft)),
            [100_100., 10_000., 100_000.],
        );
        let camera = drone.camera(Some(aircraft), FLAT);
        assert_eq!((camera.yaw, camera.pitch), (1., -0.2));
    }

    #[test]
    fn the_wheel_sets_speed_from_tens_to_thousands_of_feet_per_second() {
        let mut drone = Drone::looking(Mode::Free, [0.; 3], [0., 0., 1.], None);
        drone.wheel(1);
        assert!((drone.speed() - 312.5).abs() < 1e-9);
        drone.wheel(-2);
        assert!((drone.speed() - 200.).abs() < 1e-9);
        drone.wheel(-100);
        assert_eq!(drone.speed(), MIN_SPEED);
        drone.wheel(100);
        assert_eq!(drone.speed(), MAX_SPEED);
    }

    #[test]
    fn looking_turns_freely_and_stops_short_of_straight_up() {
        let mut drone = Drone::looking(Mode::Free, [0.; 3], [0., 0., 1.], None);
        drone.look([7., 3.]);
        let camera = drone.camera(None, |_, _| -1e9);
        assert!((f64::from(camera.pitch) - PITCH_LIMIT).abs() < 1e-6);
        assert!((0. ..std::f64::consts::TAU).contains(&f64::from(camera.yaw)));
        drone.look([0., -10.]);
        assert!((f64::from(drone.camera(None, |_, _| -1e9).pitch) + PITCH_LIMIT).abs() < 1e-6);
        // Straight down at the target still gives a finite heading.
        let down = Drone::looking(Mode::Free, [0., 100., 0.], [0., 0., 0.], None);
        assert!(down.camera(None, FLAT).yaw.is_finite());
    }
}
