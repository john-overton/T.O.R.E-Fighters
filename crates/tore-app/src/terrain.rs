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
    /// Source palette indices, one byte per texel. Retail terrain and sky art is
    /// entirely weather-palette indexed, so the artwork is uploaded unresolved
    /// and the live palette is applied on the GPU. 255 is the water cutout.
    pub sky_indices: Vec<u8>,
    pub celestial: Option<crate::celestial::Celestial>,
    pub deck_textures: BTreeMap<String, usize>,
    pub decks: [[f32; 4]; 2],
    pub vertices: Vec<f32>,
    pub texture_indices: Vec<u8>,
    /// Authoritative environment. One instance per world, so every camera,
    /// mirror and panel resolves the same instant.
    pub weather: tore_sim::environment::Environment,
    pub weather_presentation: tore_sim::environment::Presentation,
    /// The palette resolved for the presented camera altitude this frame.
    pub palette: [[u8; 3]; 256],
    pub fog_palette: Vec<[[u8; 3]; 256]>,
    /// The resolved visibility ramp: near feet, far feet, and the 0..1 haze
    /// fractions at each, plus the haze color those distances blend toward.
    pub fog: [f32; 4],
    pub haze: [u8; 3],
}
impl World {
    pub fn for_theater(resources: &BTreeMap<String, Vec<u8>>, code: &str) -> AppResult<Self> {
        Self::for_mission(resources, code, None)
    }

    /// `condition` selects one of the six recovered weather choices; without it
    /// the mission's own `layer` line and time are used unchanged.
    pub fn for_mission(
        resources: &BTreeMap<String, Vec<u8>>,
        code: &str,
        condition: Option<usize>,
    ) -> AppResult<Self> {
        let required = |n: &str| {
            resources
                .get(n)
                .ok_or_else(|| format!("Missing {n}; re-import media with --import"))
        };
        let theater = Theater::parse(required(&format!("{code}.T2"))?)?;
        let mut environment = Environment::parse(required(&format!("{code}.MM"))?)?;
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
        let (layer, launch) = match condition {
            Some(index) => {
                let choice = tore_sim::environment::CONDITIONS
                    .get(index)
                    .ok_or("weather condition outside source table")?;
                (
                    tore_sim::environment::layer_resource(index, &environment.map)?,
                    Some([
                        choice.seconds_of_day / 3600,
                        choice.seconds_of_day / 60 % 60,
                    ]),
                )
            }
            None => (environment.layer.clone(), environment.time),
        };
        let module = tore_formats::weather::Module::parse(required(&layer)?)?;
        let [hour, minute] = match std::env::var("TORE_WEATHER_TIME") {
            Ok(text) => {
                let (h, m) = text
                    .split_once(':')
                    .ok_or("TORE_WEATHER_TIME needs HH:MM")?;
                [h.parse::<i32>()?, m.parse::<i32>()?]
            }
            Err(_) => launch.unwrap_or([12, 0]),
        };
        let weather =
            tore_sim::environment::Environment::new(tore_sim::environment::Configuration::new(
                module,
                hour,
                minute,
                condition.map_or_else(|| environment.layer_parameter.unwrap_or(0), |i| i as i32),
                environment.wind,
            )?);
        if weather.sample(0.).is_none() {
            return Err("mission weather layer covers no altitude at its launch time".into());
        }
        // Preserve the resolved launch identity for validation and restart.
        environment.layer = layer;
        environment.layer_parameter = Some(weather.configuration().parameter());
        environment.time = Some([hour, minute]);
        let mut texture_indices = Vec::new();
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
            if !pic.palette.is_empty() {
                return Err("terrain texture overrides the weather palette".into());
            }
            // Native terrain texture scanning tests 255 as water/cutout (0x4aa739);
            // a masked-out texel is equally transparent, so it reuses that index.
            texture_indices.extend(
                pic.pixels
                    .iter()
                    .zip(&pic.mask)
                    .map(|(index, visible)| if *visible { *index } else { 255 }),
            );
        }
        let mut sky_indices = Vec::new();
        let mut deck_textures = BTreeMap::new();
        for layer in weather.configuration().layers() {
            for deck in &layer.decks {
                if deck.name.is_empty() || deck_textures.contains_key(&deck.name) {
                    continue;
                }
                let pic = Pic::parse(required(&deck.name)?)?;
                if pic.width != 256 || pic.height != 256 || !pic.palette.is_empty() {
                    return Err(format!("{}: unsupported indexed deck image", deck.name).into());
                }
                deck_textures.insert(
                    deck.name.clone(),
                    (texture_indices.len() + sky_indices.len()) / (256 * 256),
                );
                sky_indices.extend_from_slice(&pic.pixels);
            }
        }
        // Keep a valid texture array even for LAY families with no named decks.
        if sky_indices.is_empty() {
            sky_indices.resize(256 * 256, 255);
        }
        let celestial = Some(crate::celestial::Celestial::load(
            resources,
            &mut sky_indices,
            texture_indices.len() / 65536,
            weather.configuration().sun_fill(),
            weather.configuration().shades(),
        )?);
        let mut out = Self {
            theater,
            environment,
            catalog,
            sky_indices,
            celestial,
            deck_textures,
            decks: [[0., 1., -1., 0.]; 2],
            vertices: Vec::new(),
            texture_indices,
            weather,
            weather_presentation: tore_sim::environment::Presentation::seeded(1)?,
            palette: [[0; 3]; 256],
            fog_palette: Vec::new(),
            fog: [0.; 4],
            haze: [0; 3],
        };
        out.resolve_palette(0.);
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
                // 0x4aa739 treats 255 as water and draws it with the terrain
                // ramp's last entry. The index is resolved on the GPU each frame.
                let index = if c.color == 255 {
                    223.
                } else {
                    f32::from(c.color)
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
                        0.0,
                        0.0,
                        0.0,
                        index,
                    ]);
                }
            }
        }
    }
    /// The mission's steady wind in world feet per second.
    pub fn wind(&self) -> [f64; 3] {
        self.weather.configuration().wind_world_fps()
    }

    /// The terrain surface plus the environment's wind, for one fixed step.
    pub fn surface(&self, x: f64, z: f64) -> tore_sim::research::Surface {
        let mut surface =
            tore_sim::research::Surface::terrain(f64::from(self.height(x as f32, z as f32)));
        surface.wind = self.wind();
        surface
    }

    /// Exactly one 120 Hz tick of environment time. Pausing means not calling it.
    pub fn step_weather(&mut self, altitude_ft: f64, speed_fps: f64) {
        self.weather.step();
        self.weather_presentation
            .step(&self.weather, altitude_ft, speed_fps);
    }

    /// Presentation only: resolves the palette for one camera altitude without
    /// advancing state, so mirrors and camera panels stay on the same instant.
    pub fn resolve_palette(&mut self, altitude_ft: f64) {
        let Some(layer) = self.weather.sample(altitude_ft) else {
            return;
        };
        self.decks = layer.decks.clone().map(|deck| {
            [
                deck.altitude_feet as f32,
                2_f32.powi(deck.tile_exponent),
                self.deck_textures
                    .get(&deck.name)
                    .map_or(-1., |i| *i as f32),
                0.,
            ]
        });
        self.palette = tore_formats::weather::expand_tinted(
            self.weather.configuration().base_palette(),
            &layer,
            self.weather_presentation.tint,
        );
        self.fog_palette = self
            .weather
            .configuration()
            .shade_remap(layer.shade)
            .levels
            .iter()
            .map(|indices| indices.map(|index| self.palette[usize::from(index)]))
            .collect();
        let feet = |v: i32| (f64::from(v) * tore_formats::weather::DISTANCE_FEET) as f32;
        self.fog = [
            feet(layer.fog_near),
            feet(layer.fog_far),
            layer.fog_near_density.clamp(0, 256) as f32 / 256.,
            layer.fog_far_density.clamp(0, 256) as f32 / 256.,
        ];
        // Six-bit source components, the same expansion the palette ramps use.
        self.haze = layer.shade.map(|c| ((u16::from(c) * 255 + 31) / 63) as u8);
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
    pub roll: f32,
    pub view_fraction: f32,
    pub zoom: f32,
    pub keys: BTreeSet<String>,
}
impl Camera {
    pub fn new() -> Self {
        Self {
            position: [1_070_000.0, 28_000.0, 590_000.0],
            yaw: 0.3,
            pitch: -0.32,
            roll: 0.,
            view_fraction: 1.,
            zoom: 1.,
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
    pub fn uniform(&self, aspect: f32, fog: [f32; 4], sky: [u8; 3]) -> Vec<f32> {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();
        let (sr, cr) = self.roll.sin_cos();
        let right = [cy * cr - sy * sp * sr, cp * sr, -sy * cr - cy * sp * sr];
        let up = [-cy * sr - sy * sp * cr, cp * cr, sy * sr - cy * sp * cr];
        [
            self.position.to_vec(),
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
            texture_indices: vec![],
            sky_indices: vec![],
            celestial: None,
            deck_textures: BTreeMap::new(),
            decks: [[0., 1., -1., 0.]; 2],
            fog: [0., 1., 0., 0.],
            haze: [0; 3],
            weather: tore_sim::environment::Environment::new(
                tore_sim::environment::Configuration::new(
                    tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(
                        1,
                    ))
                    .unwrap(),
                    12,
                    0,
                    0,
                    None,
                )
                .unwrap(),
            ),
            palette: [[100; 3]; 256],
            weather_presentation: tore_sim::environment::Presentation::seeded(1).unwrap(),
            fog_palette: vec![[[100; 3]; 256]; 10],
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
