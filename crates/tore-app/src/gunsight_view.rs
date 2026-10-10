//! The AC-130 gunsight's camera, on the host side: where the target camera
//! sits and looks (docs/spec/target-window.md, "AC-130 gunsight"). The sim
//! holds the sight's look angles per tick; this turns them into a smooth
//! picture between ticks, from the sensor turret rather than the aircraft's
//! centre. Presentation only: nothing here feeds the simulation.
use crate::{camera::Camera, instruments::gunsight::SightView, target_preview};
use tore_sim::{
    attitude::Vector,
    combat::{gunship, live::Launcher},
};

/// The camera's world position for a launcher pose: sensor dome D, the round
/// turret on the left of the belly just forward of the wing root, from the
/// sim's own eye (`gunship::eye`), the origin of every sight ray
/// (John, 2026-10-09).
pub fn eye(launcher: &Launcher) -> Vector {
    gunship::eye_position(*launcher)
}

/// `Camera::zoom` for a zoom step: the picture's vertical field is
/// [`gunship::field_of_view`], and zoom 1 is 60 degrees.
pub fn zoom_factor(step: u8) -> f32 {
    ((std::f64::consts::FRAC_PI_6).tan() / (gunship::field_of_view(step) / 2.).tan()) as f32
}

/// The view along the sight's look angles: body-relative heading and
/// elevation, so it follows the aircraft's bank and turn.
pub fn free_view(launcher: &Launcher, look: [f64; 2], step: u8) -> SightView {
    let line = gunship::direction(*launcher, look[0], look[1]);
    SightView {
        position: eye(launcher),
        yaw: line[0].atan2(line[2]) as f32,
        pitch: line[1].atan2(line[0].hypot(line[2])) as f32,
        zoom: zoom_factor(step),
    }
}

/// The view of a tracked object: the target window's usual camera (on the
/// line from the eye, at most a nautical mile behind the object) at the zoom
/// that framed the object.
pub fn tracked_view(eye: Vector, target: Vector, zoom: f32) -> SightView {
    let camera = target_preview::camera(eye, target);
    SightView {
        position: camera.position,
        yaw: camera.yaw,
        pitch: camera.pitch,
        zoom,
    }
}

/// The renderer's camera for a view.
pub fn camera(view: &SightView) -> Camera {
    let mut camera = Camera::new();
    camera.position = view.position;
    camera.yaw = view.yaw;
    camera.pitch = view.pitch;
    camera.zoom = view.zoom;
    camera.weather_slot = 4;
    camera
}

/// The sight's look as the sim last reported it, with the one before, so the
/// picture can travel between ticks (the return to the default view never
/// snaps, and a slew is smooth at any frame rate).
#[derive(Clone, Debug, Default)]
pub struct SightLook {
    seen: Option<u64>,
    previous: [f64; 2],
    current: [f64; 2],
}
/// A jump this big (radians) is a respawn or a restart, not a slew.
const JUMP: f64 = 0.5;
impl SightLook {
    /// Takes the look the readout holds for flight tick `ticks`.
    pub fn update(&mut self, ticks: u64, look: [f64; 2]) {
        match self.seen {
            None => (self.previous, self.current) = (look, look),
            Some(seen) if seen == ticks => self.current = look,
            Some(seen) => {
                let jumped = angle_between(self.current, look) > JUMP;
                self.previous = if seen + 1 == ticks && !jumped {
                    self.current
                } else {
                    look
                };
                self.current = look;
            }
        }
        self.seen = Some(ticks);
    }

    /// The look at `alpha` of the way from the last tick to this one.
    pub fn presented(&self, alpha: f64) -> [f64; 2] {
        if alpha >= 1. {
            return self.current;
        }
        let alpha = alpha.max(0.);
        let dh = wrap(self.current[0] - self.previous[0]);
        [
            wrap(self.previous[0] + dh * alpha),
            self.previous[1] + (self.current[1] - self.previous[1]) * alpha,
        ]
    }
}
fn angle_between(a: [f64; 2], b: [f64; 2]) -> f64 {
    wrap(b[0] - a[0]).abs().max((b[1] - a[1]).abs())
}
fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

/// What a frame knows of the gunsight camera.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub look: [f64; 2],
    pub view: SightView,
}

/// The gunsight camera's host state: the look smoother and the zoom the last
/// framing of a tracked object chose.
#[derive(Clone, Debug)]
pub struct Sight {
    look: SightLook,
    /// `Camera::zoom` of the last automatic framing of a tracked object.
    pub fitted_zoom: f32,
}
impl Default for Sight {
    fn default() -> Self {
        Self {
            look: SightLook::default(),
            fitted_zoom: 1.,
        }
    }
}
impl Sight {
    /// This frame's look and view. `tracked` is the sight track's presented
    /// position when the sight follows an object.
    pub fn frame(
        &mut self,
        sight_look: [f64; 2],
        ticks: u64,
        alpha: f64,
        launcher: &Launcher,
        zoom: u8,
        tracked: Option<Vector>,
    ) -> Frame {
        self.look.update(ticks, sight_look);
        let look = self.look.presented(alpha);
        let view = match tracked {
            Some(position) => tracked_view(eye(launcher), position, self.fitted_zoom),
            None => free_view(launcher, look, zoom),
        };
        Frame { look, view }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launcher(yaw: f64, pitch: f64, bank: f64) -> Launcher {
        let airframe = crate::combat_view::render_hash_tests::hornet_airframe(false);
        let mut state = airframe.start(&tore_world::test_support::terrain());
        state.position = [1000., 5000., 2000.];
        (state.yaw, state.pitch, state.bank) = (yaw, pitch, bank);
        crate::combat::launcher(&state)
    }

    #[test]
    fn the_zoom_ladder_halves_the_field_each_step() {
        for (step, degrees) in [
            (1, 30.),
            (2, 15.),
            (3, 7.5),
            (4, 3.75),
            (5, 1.875),
            (6, 0.9375),
        ] {
            let zoom = f64::from(zoom_factor(step));
            let field = 2. * (30_f64.to_radians().tan() / zoom).atan().to_degrees();
            assert!((field - degrees).abs() < 1e-4, "step {step}: {field}");
        }
        // The widest step is a little over twice the fixed 60 degree view.
        assert!((zoom_factor(1) - 2.155).abs() < 0.001);
    }

    #[test]
    fn the_default_view_looks_abeam_left_and_down_from_the_turret() {
        let still = launcher(0., 0., 0.);
        let view = free_view(&still, gunship::DEFAULT_LOOK, 3);
        // Abeam left is west (negative x) when the nose points north.
        assert!((f64::from(view.yaw) + std::f64::consts::FRAC_PI_2).abs() < 1e-6);
        assert!((f64::from(view.pitch).to_degrees() + 25.).abs() < 1e-4);
        // The camera is the turret, not the aircraft's centre: left, below, forward.
        assert!(view.position[0] < still.position[0]);
        assert!(view.position[1] < still.position[1]);
        assert!(view.position[2] > still.position[2]);
        // Banking the aircraft carries the view with it: the same look, a bank
        // away, points a different way in the world.
        let banked = free_view(&launcher(0., 0., 0.4), gunship::DEFAULT_LOOK, 3);
        assert!((banked.pitch - view.pitch).abs() > 0.1);
    }

    #[test]
    fn a_point_on_the_line_of_sight_projects_to_the_centre() {
        let still = launcher(0.7, 0.05, 0.3);
        let view = free_view(&still, [-1.2, -0.4], 3);
        let line = gunship::direction(still, -1.2, -0.4);
        let point = std::array::from_fn(|i| view.position[i] + line[i] * 8000.);
        let (x, y) = view.project(point).expect("in front");
        assert!((x - 69.).abs() < 1e-3 && (y - 57.).abs() < 1e-3, "{x} {y}");
        // Half a field up lands on the top edge.
        let up = std::array::from_fn(|i| {
            view.position[i]
                + line[i] * 8000.
                + [
                    -view.yaw.sin() * view.pitch.sin(),
                    view.pitch.cos(),
                    -view.yaw.cos() * view.pitch.sin(),
                ][i] as f64
                    * 8000.
                    * (gunship::field_of_view(3) / 2.).tan()
        });
        let (_, top) = view.project(up).expect("in front");
        assert!(top.abs() < 0.01, "{top}");
    }

    #[test]
    fn the_look_travels_between_ticks_and_the_return_never_snaps() {
        let mut look = SightLook::default();
        look.update(10, [-1.0, -0.4]);
        assert_eq!(look.presented(0.5), [-1.0, -0.4]);
        look.update(11, [-1.0 + 0.0033, -0.4]);
        let halfway = look.presented(0.5);
        assert!((halfway[0] - (-1.0 + 0.00165)).abs() < 1e-12);
        let end = look.presented(1.);
        assert!((end[0] - (-1.0 + 0.0033)).abs() < 1e-12 && end[1] == -0.4);
        // The same tick with a corrected look (a client's prediction) replaces the end.
        look.update(11, [-0.9, -0.4]);
        assert_eq!(look.presented(1.), [-0.9, -0.4]);
        // A skipped tick or a jump takes the new look at once; no sweep through the view.
        look.update(14, [2.0, -0.1]);
        assert_eq!(look.presented(0.), [2.0, -0.1]);
        look.update(15, [-2.0, -0.1]);
        assert_eq!(look.presented(0.), [-2.0, -0.1]);
    }

    #[test]
    fn the_heading_travels_the_short_way_across_the_back() {
        let mut look = SightLook::default();
        look.update(1, [3.139, 0.]);
        look.update(2, [-3.139, 0.]);
        let middle = look.presented(0.5);
        assert!(middle[0].abs() > 3.13, "{middle:?}");
    }
}
