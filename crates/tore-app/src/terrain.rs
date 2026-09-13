//! Renderer-independent world data and free-camera controls (feet, X east/Y up/Z north).
use crate::AppResult;
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{
    Pic,
    theater::{CELL_FEET, Environment, HEIGHT_FEET, Theater},
};

pub struct World {
    pub theater: Theater,
    pub environment: Environment,
    pub catalog: Vec<(String, String)>,
    pub sky_pixels: Vec<u8>,
    pub vertices: Vec<f32>,
    pub texture_pixels: Vec<u8>,
    pub palette: [[u8; 3]; 256],
}
impl World {
    pub fn for_theater(resources: &BTreeMap<String, Vec<u8>>, code: &str) -> AppResult<Self> {
        let required = |n: &str| {
            resources
                .get(n)
                .ok_or_else(|| format!("Missing {n}; re-import media with --import"))
        };
        let theater = Theater::parse(required(&format!("{code}.T2"))?)?;
        let environment = Environment::parse(required(&format!("{code}.MM"))?)?;
        if theater.cols < 2 || theater.rows < 2 || environment.map != format!("{code}.T2") {
            return Err("unsupported theater map/dimensions".into());
        }
        let mut catalog = Vec::new();
        for (n, b) in resources {
            if n.ends_with(".T2") {
                let t = Theater::parse(b)?;
                catalog.push((n.trim_end_matches(".T2").into(), t.name));
            }
        }
        // Explicit midday weather keyframe; no invented runtime palette colors.
        let palette = tore_formats::theater::layer_palette(required(&environment.layer)?, 2)?;
        let mut texture_pixels = Vec::new();
        let count = environment
            .textures
            .values()
            .map(|p| p.texture + 1)
            .max()
            .unwrap_or(0);
        let prefix = &code[..code.len().min(3)];
        for i in 0..count {
            let pic = Pic::parse(required(&format!("{prefix}{i}.PIC"))?)?;
            if pic.width != 256 || pic.height != 256 {
                return Err("expected 256-square terrain texture".into());
            }
            let mut rgba = pic.rgba(&palette);
            // Native terrain texture scanning tests 255 as water/cutout (0x4aa739).
            for (j, index) in pic.pixels.iter().enumerate() {
                if *index == 255 {
                    rgba[j * 4 + 3] = 0;
                }
            }
            texture_pixels.extend(rgba);
        }
        let sky = Pic::parse(required("SKY0.PIC")?)?;
        if sky.width != 256 || sky.height != 256 {
            return Err("invalid sky texture dimensions".into());
        }
        let sky_pixels = sky.rgba(&palette);
        let mut out = Self {
            theater,
            environment,
            catalog,
            sky_pixels,
            vertices: Vec::new(),
            texture_pixels,
            palette,
        };
        out.build_mesh();
        Ok(out)
    }
    fn build_mesh(&mut self) {
        let t = &self.theater;
        // Same four sample corners as 0x4a9d00. Fixed triangulation and full-resolution
        // rendering are our first GPU implementation, not the original adaptive tessellator.
        for y in 0..t.rows - 1 {
            for x in 0..t.cols - 1 {
                let c = t.cell(x, y);
                let placement = self
                    .environment
                    .textures
                    .get(&((x & !3) as i32, (y & !3) as i32));
                let layer = placement.map_or(-1.0, |p| p.texture as f32);
                let color = if c.color == 255 {
                    self.palette[223]
                } else {
                    self.palette[c.color as usize]
                };
                for (dx, dy) in [(0, 0), (0, 1), (1, 0), (1, 0), (0, 1), (1, 1)] {
                    let sample = t.cell(x + dx, y + dy);
                    let (mut u, mut v) = (((x % 4 + dx) as f32) / 4.0, ((y % 4 + dy) as f32) / 4.0);
                    // 0x4aa9ac chooses quarter-turn UV transforms; V follows north-up world.
                    match placement.map_or(0, |p| p.rotation) {
                        1 => (u, v) = (1.0 - v, u),
                        2 => (u, v) = (1.0 - u, 1.0 - v),
                        3 => (u, v) = (v, 1.0 - u),
                        _ => {}
                    }
                    self.vertices.extend_from_slice(&[
                        (x + dx) as f32 * CELL_FEET,
                        sample.elevation as f32 * HEIGHT_FEET,
                        (y + dy) as f32 * CELL_FEET,
                        u,
                        1.0 - v,
                        layer,
                        color[0] as f32 / 255.0,
                        color[1] as f32 / 255.0,
                        color[2] as f32 / 255.0,
                    ]);
                }
            }
        }
    }
    pub fn height(&self, x: f32, z: f32) -> f32 {
        let fx = (x / CELL_FEET).clamp(0.0, (self.theater.cols - 1) as f32 - 0.001);
        let fy = (z / CELL_FEET).clamp(0.0, (self.theater.rows - 1) as f32 - 0.001);
        let (ix, iy) = (fx as usize, fy as usize);
        let (u, v) = (fx - ix as f32, fy - iy as f32);
        let h = |dx, dy| self.theater.cell(ix + dx, iy + dy).elevation as f32 * HEIGHT_FEET;
        if u + v <= 1.0 {
            h(0, 0) + (h(1, 0) - h(0, 0)) * u + (h(0, 1) - h(0, 0)) * v
        } else {
            h(1, 1) + (h(0, 1) - h(1, 1)) * (1.0 - u) + (h(1, 0) - h(1, 1)) * (1.0 - v)
        }
    }
}
pub struct Camera {
    pub position: [f32; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub keys: BTreeSet<String>,
}
impl Camera {
    pub fn new() -> Self {
        Self {
            position: [1_070_000.0, 28_000.0, 590_000.0],
            yaw: 0.3,
            pitch: -0.32,
            keys: BTreeSet::new(),
        }
    }
    pub fn for_world(world: &World) -> Self {
        let mut camera = Self::new();
        if world.theater.name != "Ukraine" {
            camera.position = [
                (world.theater.cols as f32 - 1.0) * CELL_FEET * 0.5,
                28000.0,
                (world.theater.rows as f32 - 1.0) * CELL_FEET * 0.5,
            ];
        }
        camera.position[1] =
            camera.position[1].max(world.height(camera.position[0], camera.position[2]) + 3000.0);
        camera
    }
    pub fn step(&mut self, dt: f32, fast: bool, world: &World) {
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
        self.position[0] += (f * self.yaw.sin() + r * self.yaw.cos()) * speed;
        self.position[2] += (f * self.yaw.cos() - r * self.yaw.sin()) * speed;
        self.position[0] =
            self.position[0].clamp(0.0, (world.theater.cols - 1) as f32 * CELL_FEET - 1.0);
        self.position[2] =
            self.position[2].clamp(0.0, (world.theater.rows - 1) as f32 * CELL_FEET - 1.0);
        self.position[1] = (self.position[1] + h * speed).clamp(
            world.height(self.position[0], self.position[2]) + 100.0,
            400_000.0,
        );
    }
    pub fn uniform(&self, aspect: f32, fog: f32, sky: [u8; 3]) -> Vec<f32> {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        [
            self.position.to_vec(),
            vec![aspect],
            vec![cy, 0.0, -sy, 0.0],
            vec![-sy * sp, cp, -cy * sp, 0.0],
            vec![sy * cp, sp, cy * cp, 0.0],
            vec![
                sky[0] as f32 / 255.0,
                sky[1] as f32 / 255.0,
                sky[2] as f32 / 255.0,
                fog,
            ],
        ]
        .concat()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world() -> World {
        use tore_formats::theater::TerrainCell;
        let cells = [0, 4, 8, 12]
            .map(|elevation| TerrainCell {
                color: 100,
                class: 2,
                elevation,
            })
            .to_vec();
        World {
            theater: Theater {
                name: "Synthetic".into(),
                map: "T.PIC".into(),
                tiles: [1, 1],
                cells_per_tile: 2,
                cols: 2,
                rows: 2,
                cells,
                coarse: vec![],
            },
            environment: Environment::default(),
            catalog: vec![],
            vertices: vec![],
            texture_pixels: vec![],
            sky_pixels: vec![],
            palette: [[100; 3]; 256],
        }
    }
    #[test]
    fn height_matches_triangle_corners_and_center() {
        let w = world();
        assert_eq!(w.height(0.0, 0.0), 0.0);
        assert!((w.height(CELL_FEET / 2.0, CELL_FEET / 2.0) - 1536.0).abs() < 0.01);
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
        assert!(a.position[1] >= w.height(a.position[0], a.position[2]) + 99.9);
    }
}
