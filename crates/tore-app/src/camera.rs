//! The free camera and its view geometry (feet, X east, Y up, Z north). It is
//! presentation: the simulation never reads it.
use crate::terrain::{Terrain, UKRAINE_START};
use std::collections::BTreeSet;
use tore_formats::theater::CELL_FEET;

pub struct Camera {
    /// 0 main, 1 rear mirror, 2 forward panel, 3 other panel, 4 target.
    pub weather_slot: usize,
    pub hidden_target: Option<u32>,
    pub hidden_projectile: Option<u32>,
    pub position: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub view_fraction: f32,
    pub zoom: f32,
    /// Near clipping distance in feet; target magnification fits it to the subject.
    pub near_clip: f32,
    pub keys: BTreeSet<String>,
}
impl Camera {
    pub fn new() -> Self {
        Self {
            weather_slot: 0,
            hidden_target: None,
            hidden_projectile: None,
            position: UKRAINE_START,
            yaw: 0.3,
            pitch: -0.32,
            roll: 0.,
            view_fraction: 1.,
            zoom: 1.,
            near_clip: 1.,
            keys: BTreeSet::new(),
        }
    }
    pub fn for_world(world: &Terrain) -> Self {
        let mut camera = Self::new();
        camera.position = world.free_flight_start();
        camera
    }
    pub fn step(&mut self, dt: f32, fast: bool, world: &Terrain) {
        let dt = dt.clamp(0.0, 0.05);
        let k = |s: &str| f32::from(self.keys.contains(s));
        self.yaw += (k("d") - k("a")) * dt;
        self.pitch = (self.pitch + (k("w") - k("s")) * dt).clamp(-1.5, 1.5);
        let (f, r, h) = (
            k("ArrowUp") - k("ArrowDown"),
            k("ArrowRight") - k("ArrowLeft"),
            k("e") + k("PageUp") - k("q") - k("PageDown"),
        );
        let norm = (f * f + r * r + h * h).sqrt().max(1.0);
        let speed = dt * 12_000.0 * if fast { 8.0 } else { 1.0 } / norm;
        self.position[0] += f64::from((f * self.yaw.sin() + r * self.yaw.cos()) * speed);
        self.position[2] += f64::from((f * self.yaw.cos() - r * self.yaw.sin()) * speed);
        self.position[0] = self.position[0].clamp(
            0.0,
            f64::from((world.theater.cols - 1) as f32 * CELL_FEET - 1.0),
        );
        self.position[2] = self.position[2].clamp(
            0.0,
            f64::from((world.theater.rows - 1) as f32 * CELL_FEET - 1.0),
        );
        let ground = world.height(self.position[0] as f32, self.position[2] as f32);
        self.position[1] =
            (self.position[1] + f64::from(h * speed)).clamp(f64::from(ground + 100.0), 400_000.0);
    }
    /// Where the renderer draws a world point in a `size` pixel view, if it is
    /// in front of the camera and on screen.
    pub fn project(&self, size: [u32; 2], point: [f64; 3]) -> Option<[f64; 2]> {
        let (sy, cy) = f64::from(self.yaw).sin_cos();
        let (sp, cp) = f64::from(self.pitch).sin_cos();
        let (sr, cr) = f64::from(self.roll).sin_cos();
        let right = [cy * cr - sy * sp * sr, cp * sr, -sy * cr - cy * sp * sr];
        let up = [-cy * sr - sy * sp * cr, cp * cr, sy * sr - cy * sp * cr];
        let forward = [sy * cp, sp, cy * cp];
        let d: [f64; 3] = std::array::from_fn(|i| point[i] - self.position[i]);
        let dot = |a: [f64; 3]| a[0] * d[0] + a[1] * d[1] + a[2] * d[2];
        let z = dot(forward);
        if z <= 1. {
            return None;
        }
        let [w, h] = size.map(f64::from);
        let focal = h / 2. * 3f64.sqrt() * f64::from(self.zoom);
        let x = w / 2. + focal * dot(right) / z;
        let y = h / 2. - focal * dot(up) / z;
        ((0. ..w).contains(&x) && (0. ..h).contains(&y)).then_some([x, y])
    }
    pub fn uniform(&self, aspect: f32, fog: [f32; 4], sky: [u8; 3]) -> Vec<f32> {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let (sr, cr) = self.roll.sin_cos();
        let right = [cy * cr - sy * sp * sr, cp * sr, -sy * cr - cy * sp * sr];
        let up = [-cy * sr - sy * sp * cr, cp * cr, sy * sr - cy * sp * cr];
        [
            self.position.map(|v| v as f32).to_vec(),
            vec![aspect],
            vec![right[0], right[1], right[2], 0.],
            vec![up[0], up[1], up[2], self.zoom],
            vec![sy * cp, sp, cy * cp, 0.0],
            vec![
                sky[0] as f32 / 255.0,
                sky[1] as f32 / 255.0,
                sky[2] as f32 / 255.0,
                0.0,
            ],
            fog.to_vec(),
        ]
        .concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::terrain::tests::world;
    #[test]
    fn variant_label_does_not_move_the_inspection_camera() {
        let mut w = world();
        w.layout = "UKR.MM".into();
        let base = Camera::for_world(&w).position;
        w.layout = "~UKR1.MM".into();
        w.theater.name = "Ukraine (UKR1)".into();
        assert_eq!(Camera::for_world(&w).position, base);
        w.layout = "~FRA0.MM".into();
        assert_ne!(Camera::for_world(&w).position, base);
    }
    #[test]
    fn projection_matches_the_renderer_view() {
        let mut camera = Camera::new();
        camera.position = [0., 1000., 0.];
        camera.yaw = 0.;
        camera.pitch = 0.;
        camera.roll = 0.;
        camera.zoom = 1.;
        let size = [800, 600];
        // Straight ahead is the centre.
        assert_eq!(camera.project(size, [0., 1000., 5000.]), Some([400., 300.]));
        // 30 degrees up is the top edge of the 60 degree tall view.
        let up = camera.project(
            size,
            [0., 1000. + 5000. * (30f64).to_radians().tan() * 0.99, 5000.],
        );
        assert!(up.is_some_and(|[_, y]| (2. ..4.).contains(&y)));
        // Behind the camera or off the side is not on screen.
        assert_eq!(camera.project(size, [0., 1000., -5000.]), None);
        assert_eq!(camera.project(size, [9000., 1000., 5000.]), None);
    }
    #[test]
    fn camera_speed_is_time_based_and_clearing_keys_stops_motion() {
        let w = world();
        let mut a = Camera::new();
        a.position = [2000.0, 10000.0, 2000.0];
        a.yaw = 0.0;
        a.keys.insert("ArrowUp".into());
        let mut b = Camera::new();
        b.position = a.position;
        b.yaw = 0.0;
        b.keys = a.keys.clone();
        a.step(0.04, false, &w);
        b.step(0.02, false, &w);
        b.step(0.02, false, &w);
        assert_eq!(a.position, b.position);
        let pos = a.position;
        a.keys.clear();
        a.step(0.04, false, &w);
        assert_eq!(a.position, pos);
    }
    #[test]
    fn shift_accelerates_and_altitude_cannot_cross_mesh() {
        let w = world();
        let mut a = Camera::new();
        a.position = [2000.0, 10000.0, 2000.0];
        a.yaw = 0.0;
        a.keys.insert("ArrowUp".into());
        a.step(0.01, true, &w);
        assert!((a.position[2] - 2960.0).abs() < 0.1);
        a.position[1] = 0.0;
        a.keys.clear();
        a.keys.insert("q".into());
        a.step(0.05, false, &w);
        assert!(
            a.position[1] >= f64::from(w.height(a.position[0] as f32, a.position[2] as f32) + 99.9)
        );
    }
}
