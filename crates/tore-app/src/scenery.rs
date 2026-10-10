//! Presentation half of the mission's world: the land and sky art, the terrain
//! mesh, the static airport geometry, ocean motion, the resolved palette and
//! fog, the per-camera weather presentation and the render origin. The
//! simulation half is [`Terrain`]; code that needs the weather or the airport
//! scene takes `&Terrain` alongside `&Scenery`.
mod runway_cutout;
pub mod surface_art;

use crate::{
    AppResult,
    camera::Camera,
    terrain::{Overrides, Placements, Stance, Terrain},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use tore_formats::{
    Pic,
    theater::{CELL_FEET, HEIGHT_FEET, TexturePlacement, Theater},
};

/// A pure camera query; contains no clock, random generator or trail history.
pub struct ViewWeather {
    pub palette: [[u8; 3]; 256],
    pub fog_palette: Vec<[[u8; 3]; 256]>,
    pub decks: [[f32; 4]; 2],
    pub fog: [f32; 4],
    pub haze: [u8; 3],
    pub visual_bands: Vec<tore_formats::weather::Layer>,
}

/// One immutable standing airport batch. Its shared identity lets each renderer
/// upload it once, including after that renderer or the scenery is replaced.
pub struct StaticGeometry {
    pub vertices: Vec<f32>,
    pub lines: Vec<f32>,
}

struct StandingGeometry {
    /// Every placement not standing intact, with its look.
    looks: BTreeMap<u32, surface_art::Look>,
    geometry: Arc<StaticGeometry>,
}

pub struct Scenery {
    pub ocean_motion: crate::ocean::Motion,
    /// Source palette indices, one byte per texel. Retail terrain and sky art is
    /// entirely weather-palette indexed, so the artwork is uploaded unresolved
    /// and the live palette is applied on the GPU. 255 is the water cutout.
    pub sky_indices: Vec<u8>,
    pub celestial: Option<crate::celestial::Celestial>,
    pub clouds: Option<crate::clouds::Clouds>,
    pub deck_textures: BTreeMap<String, usize>,
    pub decks: [[f32; 4]; 2],
    pub vertices: Vec<f32>,
    /// Immutable per-placement geometry, filtered from combat HP for drawing.
    static_vertices: BTreeMap<u32, Vec<f32>>,
    static_lines: BTreeMap<u32, Vec<f32>>,
    standing_geometry: Option<StandingGeometry>,
    /// The surface units' wrecks, rails, moving units, sprites and pieces.
    pub surface: surface_art::SurfaceArt,
    pub texture_indices: Vec<u8>,
    pub smooth_weather: bool,
    pub visual_bands: Vec<tore_formats::weather::Layer>,
    pub no_sun_whiteout: bool,
    pub auxiliary_presentations: [tore_sim::environment::Presentation; 4],
    pub weather_presentation: tore_sim::environment::Presentation,
    /// The palette resolved for the presented camera altitude this frame.
    pub palette: [[u8; 3]; 256],
    pub fog_palette: Vec<[[u8; 3]; 256]>,
    /// The resolved visibility ramp: near feet, far feet, and the 0..1 haze
    /// fractions at each, plus the haze color those distances blend toward.
    pub fog: [f32; 4],
    pub haze: [u8; 3],
    /// This frame's render origin: moving objects' vertices are relative to it
    /// so they keep sub-inch precision where 32-bit world coordinates step
    /// 1/8 foot. Zero places them in world coordinates.
    pub origin: [f64; 3],
}

/// The launch overrides the environment asks for: `TORE_WEATHER_TIME` as
/// `HH:MM`, `TORE_WIND` as `heading,speed` in degrees and feet per second, and
/// `TORE_CLOUD_ALTITUDE` in feet. They change the simulation, so the app reads
/// them here and hands them to the terrain; a recording's identity replaces
/// them.
pub fn launch_overrides() -> AppResult<Overrides> {
    let time = match std::env::var("TORE_WEATHER_TIME") {
        Ok(text) => {
            let (h, m) = text
                .split_once(':')
                .ok_or("TORE_WEATHER_TIME needs HH:MM")?;
            Some([h.parse::<i32>()?, m.parse::<i32>()?])
        }
        Err(_) => None,
    };
    let wind = match std::env::var("TORE_WIND") {
        Ok(value) => {
            let (heading, speed) = value
                .split_once(',')
                .ok_or("TORE_WIND needs heading,speed in degrees/feet per second")?;
            Some([heading.parse::<i32>()?, speed.parse::<i32>()?])
        }
        Err(std::env::VarError::NotPresent) => None,
        Err(e) => return Err(e.into()),
    };
    let cloud_altitude = match std::env::var("TORE_CLOUD_ALTITUDE") {
        Ok(value) => Some(value.parse::<i32>()?),
        Err(_) => None,
    };
    Ok(Overrides {
        time,
        wind,
        cloud_altitude,
    })
}

/// The terrain a launch builds: `condition` picks one of the six weather
/// choices (the mission's own weather without it), and the environment's
/// launch overrides apply.
pub fn launch_terrain(
    resources: &BTreeMap<String, Vec<u8>>,
    code: &str,
    condition: Option<usize>,
) -> AppResult<Terrain> {
    Terrain::for_mission(resources, code, condition, &launch_overrides()?)
}

/// Preserve every source index. Doubling 128-square images is exact nearest
/// replication; all retail terrain images retain the same four-cell footprint.
fn append_ground_texture(out: &mut Vec<u8>, pic: &Pic) -> AppResult<()> {
    if pic.width != pic.height || !matches!(pic.width, 128 | 256) || !pic.palette.is_empty() {
        return Err("unsupported indexed ground image".into());
    }
    for y in 0..256 {
        for x in 0..256 {
            let at = (y * pic.height / 256) * pic.width + x * pic.width / 256;
            out.push(if pic.mask[at] { pic.pixels[at] } else { 255 });
        }
    }
    Ok(())
}

/// Each parked aircraft of `terrain` with its shape drawn gear down. A shape
/// that does not read is left undrawn and logged.
fn parked_shapes<'a>(
    resources: &BTreeMap<String, Vec<u8>>,
    terrain: &'a Terrain,
) -> Vec<(
    &'a tore_world::surface::parked::ParkedPose,
    tore_formats::shape::Shape,
)> {
    terrain
        .surface
        .parked_scene
        .iter()
        .filter_map(|pose| {
            let state: BTreeMap<usize, i32> =
                pose.gear_word.map(|word| (word, 1)).into_iter().collect();
            let shape = resources
                .get(&pose.shape)
                .ok_or_else(|| format!("missing {}", pose.shape))
                .and_then(|bytes| {
                    tore_formats::shape::Shape::with_state(bytes, &state).map_err(|e| e.to_string())
                });
            match shape {
                Ok(shape) => Some((pose, shape)),
                Err(error) => {
                    log::warn!("Parked aircraft {} not drawn: {error}", pose.resource);
                    None
                }
            }
        })
        .collect()
}

/// Placements without a standing combat target: destroyed, or never registered.
fn fallen(
    geometry: &BTreeMap<u32, Vec<f32>>,
    targets: &[tore_sim::combat::live::Target],
) -> BTreeSet<u32> {
    let alive: BTreeSet<u32> = targets
        .iter()
        .filter(|target| target.hp > 0)
        .map(|target| target.id)
        .collect();
    geometry
        .keys()
        .filter(|id| !alive.contains(id))
        .copied()
        .collect()
}
/// Every placement's geometry except the destroyed ones, in placement order.
fn standing(geometry: &BTreeMap<u32, Vec<f32>>, destroyed: &BTreeSet<u32>) -> Vec<f32> {
    let total = geometry
        .iter()
        .filter(|(id, _)| !destroyed.contains(id))
        .map(|(_, vertices)| vertices.len())
        .sum();
    let mut out = Vec::with_capacity(total);
    for (id, vertices) in geometry {
        if !destroyed.contains(id) {
            out.extend_from_slice(vertices);
        }
    }
    out
}

impl Scenery {
    /// Grid the render origin snaps to, in feet, so it is exact in 32 bits
    /// and moves only when the camera crosses a cell.
    pub const ORIGIN_CELL: f64 = 1024.;
    /// Place this frame's render origin near `eye`, before any moving
    /// object's vertices are built. Every camera drawn this frame shares it.
    pub fn set_origin(&mut self, eye: [f64; 3]) {
        self.origin = eye.map(|v| (v / Self::ORIGIN_CELL).round() * Self::ORIGIN_CELL);
    }
    /// A world position relative to the render origin.
    pub fn relative(&self, position: [f64; 3]) -> [f64; 3] {
        std::array::from_fn(|i| position[i] - self.origin[i])
    }
    /// A world position relative to the render origin, for vertices.
    pub fn local(&self, position: [f64; 3]) -> [f32; 3] {
        self.relative(position).map(|v| v as f32)
    }

    /// Builds the art for a finished `terrain` from the same imported
    /// resources: ground and sky textures, the terrain mesh and the static
    /// airport geometry.
    pub fn build(resources: &BTreeMap<String, Vec<u8>>, terrain: &Terrain) -> AppResult<Self> {
        let required = |n: &str| {
            resources
                .get(n)
                .ok_or_else(|| format!("Missing {n}; re-import media with --import"))
        };
        let code = terrain.layout.trim_end_matches(".MM");
        let base = tore_formats::theater::base_theater(&terrain.layout)
            .ok_or("unknown retail map layout")?;
        let weather = &terrain.weather;
        let mut texture_indices = Vec::new();
        let mut terrain_layers = BTreeMap::new();
        let mut ground_layers = BTreeMap::new();
        for (key, placement) in &terrain.environment.textures {
            let name = placement.resource_name(base);
            let layer = if let Some(layer) = terrain_layers.get(&name) {
                *layer
            } else {
                let pic = Pic::parse(required(&name)?)?;
                let layer = texture_indices.len() / 65536;
                append_ground_texture(&mut texture_indices, &pic)?;
                terrain_layers.insert(name, layer);
                layer
            };
            ground_layers.insert(*key, layer);
        }
        let land_name = format!("{}LAND.PIC", &base[..1]);
        let land = resources
            .get(&land_name)
            .or_else(|| resources.get("LAND.PIC"));
        let land_texture = if let Some(bytes) = land {
            let layer = texture_indices.len() / 65536;
            append_ground_texture(&mut texture_indices, &Pic::parse(bytes)?)?;
            Some(layer)
        } else {
            None
        };
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
            weather.configuration().lighting(),
        )?);
        let clouds = Some(crate::clouds::Clouds::load(
            resources,
            &mut sky_indices,
            texture_indices.len() / 65536,
            terrain.environment.clouds.unwrap_or(0),
        )?);
        let mut out = Self {
            ocean_motion: crate::ocean::Motion::from_environment()?,
            sky_indices,
            celestial,
            clouds,
            deck_textures,
            decks: [[0., 1., -1., 0.]; 2],
            vertices: Vec::new(),
            static_vertices: BTreeMap::new(),
            static_lines: BTreeMap::new(),
            standing_geometry: None,
            surface: surface_art::SurfaceArt::default(),
            texture_indices,
            smooth_weather: match std::env::var("TORE_WEATHER_SMOOTH").as_deref() {
                Err(std::env::VarError::NotPresent) | Ok("1") => true,
                Ok("0") => false,
                _ => return Err("TORE_WEATHER_SMOOTH must be 0 or 1".into()),
            },
            visual_bands: Vec::new(),
            no_sun_whiteout: false,
            auxiliary_presentations: std::array::from_fn(|_| {
                tore_sim::environment::Presentation::seeded(1).unwrap()
            }),
            weather_presentation: tore_sim::environment::Presentation::seeded(1)?,
            palette: [[0; 3]; 256],
            fog_palette: Vec::new(),
            fog: [0.; 4],
            haze: [0; 3],
            origin: [0.; 3],
        };
        out.resolve_palette(terrain, 0.);
        out.vertices = Self::build_mesh(
            &terrain.theater,
            &terrain.environment.textures,
            &ground_layers,
            land_texture,
        );
        out.build_static_geometry(resources, terrain, code)?;
        out.recess_airport_terrain(terrain);
        Ok(out)
    }

    /// Restarts the camera weather from its fixed seeds, as a flight's start
    /// restarts the weather clock.
    pub fn reset_presentations(&mut self) {
        self.weather_presentation = tore_sim::environment::Presentation::seeded(1)
            .expect("fixed valid weather presentation seed");
        self.auxiliary_presentations = std::array::from_fn(|_| self.weather_presentation.clone());
    }

    /// The static airport geometry, one entry per drawable placement, under a
    /// 32 MiB budget. Its textures are appended to the sky array.
    fn build_static_geometry(
        &mut self,
        resources: &BTreeMap<String, Vec<u8>>,
        terrain: &Terrain,
        code: &str,
    ) -> AppResult<()> {
        self.standing_geometry = None;
        // The layout's placements and the ground target's, as the terrain
        // built its scene.
        let sources = Placements::for_terrain(resources, terrain, code)?;
        for (main_shape, error) in &sources.unreadable {
            log::warn!("Airport scene: {main_shape} retained without visual geometry: {error}");
        }
        let mut static_float_count = 0usize;
        // Every drawn shape with its id, type, scale, orientation and origin.
        let mut drawn: Vec<(
            u32,
            Option<&str>,
            &tore_formats::shape::Shape,
            surface_art::Stand,
            f64,
        )> = Vec::new();
        for placed in sources.placed() {
            let (id, placement) = placed?;
            let Some(shape) = sources.shapes.get(&placement.object_type) else {
                continue;
            };
            let ground = f64::from(
                terrain.height(placement.position[0] as f32, placement.position[2] as f32),
            );
            let Some(Stance {
                scale,
                basis,
                origin,
                support_origin,
                ..
            }) = sources.stance(placement, ground)
            else {
                continue;
            };
            // How far the shape stands above its placement point (a land
            // unit on its lowest point): a moving unit keeps it.
            let lift = (0..3)
                .map(|i| (origin[i] - support_origin[i]) * basis.up[i])
                .sum();
            drawn.push((
                id,
                Some(placement.object_type.as_str()),
                shape,
                surface_art::Stand {
                    scale,
                    basis,
                    origin,
                },
                lift,
            ));
        }
        // The parked aircraft, gear down at the aircraft convention, where
        // the terrain placed them (docs/spec/surface-defenses.md, "Parked
        // aircraft"). Like every placement they hide when destroyed.
        let parked = parked_shapes(resources, terrain);
        drawn.extend(parked.iter().map(|(pose, shape)| {
            (
                pose.id.0,
                None,
                shape,
                surface_art::Stand {
                    scale: pose.scale,
                    basis: pose.basis,
                    origin: pose.origin,
                },
                0.,
            )
        }));
        let mut art = surface_art::SurfaceArt::default();
        let first_page = self.texture_indices.len();
        let mut build = surface_art::Build {
            resources,
            terrain,
            definitions: &sources.definitions,
            first_page,
            pages: &mut self.sky_indices,
        };
        for (id, object_type, shape, stand, lift) in drawn {
            surface_art::preload(shape, resources, first_page, build.pages, &mut art.layers)?;
            let mut geometry = surface_art::Drawn::default();
            surface_art::emit(
                shape,
                &stand,
                &art.layers,
                &mut geometry,
                surface_art::BUDGET - static_float_count,
            )?;
            match object_type {
                Some(object_type) => {
                    // Wrecks, rails, carrier parts and sprites; a unit that
                    // follows a route is drawn every frame instead.
                    let (parts, moves) =
                        art.add(&mut build, id, object_type, shape, &stand, lift)?;
                    if moves {
                        continue;
                    }
                    geometry.vertices.extend(parts.vertices);
                    geometry.lines.extend(parts.lines);
                }
                None => {
                    if let Some((pose, _)) = parked.iter().find(|(pose, _)| pose.id.0 == id) {
                        art.add_parked(&mut build, id, &pose.shape, pose.scale);
                    }
                }
            }
            static_float_count += geometry.lines.len();
            self.static_lines.insert(id, geometry.lines);
            static_float_count += geometry.vertices.len();
            if static_float_count > surface_art::BUDGET {
                return Err("static scene exceeds 32 MiB geometry budget".into());
            }
            self.static_vertices.insert(id, geometry.vertices);
        }
        self.surface = art;
        Ok(())
    }

    /// The terrain triangles: `textures` are the shoreline and ground art
    /// placements and `layers` the texture layer each was uploaded to.
    fn build_mesh(
        t: &Theater,
        textures: &BTreeMap<(i32, i32), TexturePlacement>,
        layers: &BTreeMap<(i32, i32), usize>,
        land_texture: Option<usize>,
    ) -> Vec<f32> {
        let mut vertices = Vec::new();
        // Same four sample corners as 0x4a9d00. Fixed triangulation and full-resolution
        // rendering are our first GPU implementation, not the original adaptive tessellator.
        for y in 0..t.rows - 1 {
            for x in 0..t.cols - 1 {
                let c = t.cell(x, y);
                let key = ((x & !3) as i32, (y & !3) as i32);
                let placement = textures.get(&key);
                let layer = placement.map_or_else(
                    || land_texture.map_or(-1.0, |l| l as f32),
                    |_| layers[&key] as f32,
                );
                // Untextured water reveals the shared ocean/horizon pass. A
                // shoreline texture defines coverage even on a water base cell.
                if placement.is_none() && c.color == 255 {
                    continue;
                }
                let index = f32::from(c.color);
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
                    vertices.extend_from_slice(&[
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
        vertices
    }
    /// Split at footprint edges before lowering the rendered ground, so
    /// neighboring terrain and all physics queries retain their original data.
    fn recess_airport_terrain(&mut self, terrain: &Terrain) {
        self.vertices = runway_cutout::terrain(&self.vertices, &terrain.airport_scene.runways);
    }

    /// Number of source triangle vertices, before destroyed placements hide.
    pub fn static_vertex_count(&self) -> usize {
        self.static_vertices.values().map(|v| v.len() / 10).sum()
    }

    /// Placement geometry whose object still has a standing combat target.
    pub fn visible_static_vertices(&self, targets: &[tore_sim::combat::live::Target]) -> Vec<f32> {
        self.visible_static_vertices_where(&fallen(&self.static_vertices, targets))
    }
    #[cfg(test)]
    pub fn visible_static_lines(&self, targets: &[tore_sim::combat::live::Target]) -> Vec<f32> {
        self.visible_static_lines_where(&fallen(&self.static_lines, targets))
    }
    /// Placement geometry except the `destroyed` objects, for a mission
    /// replay that recorded which objects were destroyed.
    pub fn visible_static_vertices_where(&self, destroyed: &BTreeSet<u32>) -> Vec<f32> {
        standing(&self.static_vertices, destroyed)
    }
    #[cfg(test)]
    pub fn visible_static_lines_where(&self, destroyed: &BTreeSet<u32>) -> Vec<f32> {
        standing(&self.static_lines, destroyed)
    }

    /// Reuse one current batch while every placement keeps its look. A
    /// missing combat target hides its placement; one with no HP shows its
    /// wreck when it keeps one (docs/spec/surface-defenses.md, "Destroyed
    /// looks"), else hides; a launcher shows the loads `surface` gives its
    /// rails.
    pub fn static_geometry(
        &mut self,
        targets: &[tore_sim::combat::live::Target],
        surface: &tore_world::surface::SurfaceState,
    ) -> &Arc<StaticGeometry> {
        let hp: BTreeMap<u32, i32> = targets.iter().map(|t| (t.id, t.hp)).collect();
        let looks = self
            .placement_ids()
            .into_iter()
            .filter_map(|id| {
                let known = hp.get(&id);
                let look = self.surface.look(
                    id,
                    known.is_some_and(|hp| *hp > 0),
                    known.is_some(),
                    Some(surface),
                )?;
                Some((id, look))
            })
            .collect();
        self.static_geometry_with(looks)
    }

    /// Recorded destruction can move backwards when a replay seeks. Keep
    /// only the current set's batch, so destruction history cannot grow it.
    /// A destroyed placement shows its wreck when it keeps one.
    pub fn static_geometry_where(&mut self, hidden: &BTreeSet<u32>) -> &Arc<StaticGeometry> {
        let looks = hidden
            .iter()
            .filter_map(|id| Some((*id, self.surface.look(*id, false, true, None)?)))
            .collect();
        self.static_geometry_with(looks)
    }

    /// Every placement id with standing geometry or lines, ascending.
    fn placement_ids(&self) -> BTreeSet<u32> {
        self.static_vertices
            .keys()
            .chain(self.static_lines.keys())
            .copied()
            .collect()
    }

    fn static_geometry_with(
        &mut self,
        looks: BTreeMap<u32, surface_art::Look>,
    ) -> &Arc<StaticGeometry> {
        if self
            .standing_geometry
            .as_ref()
            .is_none_or(|cached| cached.looks != looks)
        {
            let mut vertices = Vec::new();
            let mut lines = Vec::new();
            for id in self.placement_ids() {
                match looks.get(&id) {
                    None => {
                        vertices.extend_from_slice(
                            self.static_vertices.get(&id).map_or(&[][..], Vec::as_slice),
                        );
                        lines.extend_from_slice(
                            self.static_lines.get(&id).map_or(&[][..], Vec::as_slice),
                        );
                    }
                    Some(look) => {
                        if let Some(drawn) = self.surface.drawn(id, look) {
                            vertices.extend_from_slice(&drawn.vertices);
                            lines.extend_from_slice(&drawn.lines);
                        }
                    }
                }
            }
            self.standing_geometry = Some(StandingGeometry {
                looks,
                geometry: Arc::new(StaticGeometry { vertices, lines }),
            });
        }
        &self.standing_geometry.as_ref().unwrap().geometry
    }

    /// This frame's moving surface geometry, relative to the render origin
    /// (set it first): routed units, men and deck crew facing `camera`, and
    /// parked aircraft pieces, for [`crate::sim_renderer::SimRenderer::surface_units`].
    /// `standing` says whether an object stands.
    pub fn surface_vertices(
        &self,
        picture: &crate::snapshot::RenderSnapshot,
        standing: &dyn Fn(u32) -> bool,
        camera: &Camera,
    ) -> Vec<f32> {
        self.surface.frame(
            picture,
            standing,
            camera,
            self.origin,
            picture.tick as f64 / 120.,
        )
    }

    pub fn glare_enabled(&self) -> bool {
        !self.no_sun_whiteout && self.celestial.as_ref().is_some_and(|c| c.sun_effects)
    }

    /// Each fixed camera slot advances once per simulation tick, even if hidden.
    /// Slots have independent seeded presentation state; queries never consume RNG.
    pub fn step_view_weather(&mut self, terrain: &Terrain, camera: &Camera, speed_fps: f64) {
        let altitude = camera.position[1];
        let alignment = if self.glare_enabled() {
            terrain
                .weather
                .sample(altitude)
                .and_then(|l| {
                    tore_sim::environment::sun_angles(&l, terrain.weather.seconds_of_day())
                })
                .map_or(-1., |a| {
                    let sun = crate::celestial::rotate([0., 0., 1.], a);
                    let view = camera.uniform(1., [0.; 4], [0; 3]);
                    (0..3)
                        .map(|i| f64::from(sun[i]) * f64::from(view[12 + i]))
                        .sum()
                })
        } else {
            -1.
        };
        let visual_target = if self.smooth_weather && self.glare_enabled() {
            terrain
                .weather
                .sample(altitude)
                .and_then(|layer| crate::celestial::visual_sun_direction(&layer, &terrain.weather))
                .map_or(0., |sun| {
                    let view = camera.uniform(1., [0.; 4], [0; 3]);
                    let alignment: f64 = (0..3)
                        .map(|i| f64::from(sun[i]) * f64::from(view[12 + i]))
                        .sum();
                    let response = (((alignment.clamp(-1., 1.) * (32767. * 32767. / 65536.))
                        .floor()
                        - 15564.)
                        / 3.)
                        .clamp(0., 255.);
                    response
                        * f64::from(crate::celestial::glare_strength(
                            terrain, self, altitude, sun,
                        ))
                })
        } else {
            0.
        };
        let presentation = if camera.weather_slot == 0 {
            &mut self.weather_presentation
        } else {
            &mut self.auxiliary_presentations[camera.weather_slot - 1]
        };
        let previous_visual = presentation.visual_sun;
        presentation.step_with_alignment(&terrain.weather, altitude, speed_fps, alignment);
        if self.smooth_weather {
            presentation.visual_sun =
                previous_visual + (visual_target - previous_visual).clamp(-16. / 7.2, 16. / 7.2);
        }
    }

    /// Presentation only: resolves the palette for one camera altitude without
    /// advancing state, so mirrors and camera panels stay on the same instant.
    pub fn resolve_palette(&mut self, terrain: &Terrain, altitude_ft: f64) {
        let view = self.sample_view(terrain, altitude_ft, 0);
        self.palette = view.palette;
        self.fog_palette = view.fog_palette;
        self.decks = view.decks;
        self.fog = view.fog;
        self.haze = view.haze;
        self.visual_bands = view.visual_bands;
    }

    pub fn sample_view(&self, terrain: &Terrain, altitude_ft: f64, slot: usize) -> ViewWeather {
        let presentation = if slot == 0 {
            &self.weather_presentation
        } else {
            &self.auxiliary_presentations[slot - 1]
        };
        let mut out = ViewWeather {
            palette: self.palette,
            fog_palette: self.fog_palette.clone(),
            decks: self.decks,
            fog: self.fog,
            haze: self.haze,
            visual_bands: Vec::new(),
        };
        let visual = self
            .smooth_weather
            .then(|| terrain.weather.visual_sample(altitude_ft))
            .flatten();
        let Some(layer) = terrain.weather.sample(altitude_ft) else {
            return out;
        };
        out.decks = layer.decks.clone().map(|deck| {
            [
                deck.altitude_feet as f32,
                2_f32.powi(deck.tile_exponent),
                self.deck_textures
                    .get(&deck.name)
                    .map_or(-1., |i| *i as f32),
                0.,
            ]
        });
        // Texture selection and draw flags retain native scheduling. Only the
        // color/fog parameters below use fractional-time presentation samples.
        let layer = visual.as_ref().map_or(layer.clone(), |s| s.layer.clone());
        out.palette = tore_formats::weather::expand_effects(
            terrain.weather.configuration().base_palette(),
            &layer,
            presentation.tint,
            if self.glare_enabled() {
                presentation.sun_whitening
            } else {
                0
            },
        );
        if let Some(visual) = visual {
            out.visual_bands = visual.bands.clone();
            out.palette = visual.palette(
                presentation.visual_tint,
                if self.glare_enabled() {
                    presentation.visual_sun
                } else {
                    0.
                },
            );
        }
        out.fog_palette = terrain
            .weather
            .configuration()
            .shade_remap(layer.shade)
            .levels
            .iter()
            .map(|indices| indices.map(|index| out.palette[usize::from(index)]))
            .collect();
        let feet = |v: i32| (f64::from(v) * tore_formats::weather::DISTANCE_FEET) as f32;
        out.fog = [
            feet(layer.fog_near),
            feet(layer.fog_far),
            layer.fog_near_density.clamp(0, 256) as f32 / 256.,
            layer.fog_far_density.clamp(0, 256) as f32 / 256.,
        ];
        // Six-bit source components, the same expansion the palette ramps use.
        out.haze = layer.shade.map(|c| ((u16::from(c) * 255 + 31) / 63) as u8);
        out
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    /// A scenery with no art: the palette and fog the old synthetic world had.
    pub(crate) fn scenery() -> Scenery {
        Scenery {
            ocean_motion: crate::ocean::Motion::default(),
            sky_indices: vec![],
            celestial: None,
            clouds: None,
            deck_textures: BTreeMap::new(),
            decks: [[0., 1., -1., 0.]; 2],
            vertices: vec![],
            static_vertices: BTreeMap::new(),
            static_lines: BTreeMap::new(),
            standing_geometry: None,
            surface: surface_art::SurfaceArt::default(),
            texture_indices: vec![],
            smooth_weather: true,
            visual_bands: Vec::new(),
            no_sun_whiteout: false,
            auxiliary_presentations: std::array::from_fn(|_| {
                tore_sim::environment::Presentation::seeded(1).unwrap()
            }),
            weather_presentation: tore_sim::environment::Presentation::seeded(1).unwrap(),
            palette: [[100; 3]; 256],
            fog_palette: vec![[[100; 3]; 256]; 10],
            fog: [0., 1., 0., 0.],
            haze: [0; 3],
            origin: [0.; 3],
        }
    }
    #[test]
    fn smaller_ground_art_preserves_every_source_texel_and_water() {
        let mut pic = Pic {
            width: 128,
            height: 128,
            pixels: vec![93; 128 * 128],
            mask: vec![true; 128 * 128],
            palette: vec![],
            glyphs: vec![],
        };
        pic.pixels[1] = 255;
        pic.pixels[128 * 127 + 127] = 17;
        pic.mask[128] = false;
        let mut bytes = Vec::new();
        append_ground_texture(&mut bytes, &pic).unwrap();
        assert_eq!(bytes.len(), 65536);
        for y in 0..256 {
            for x in 0..256 {
                let at = (y / 2) * 128 + x / 2;
                assert_eq!(
                    bytes[y * 256 + x],
                    if pic.mask[at] { pic.pixels[at] } else { 255 }
                );
            }
        }
        pic.width = 127;
        assert!(append_ground_texture(&mut Vec::new(), &pic).is_err());
    }
    #[test]
    fn listed_destroyed_objects_hide_as_their_fallen_targets_do() {
        let mut w = scenery();
        for id in [1, 2, 3] {
            w.static_vertices.insert(id, vec![id as f32; 10]);
            w.static_lines.insert(id, vec![-(id as f32); 10]);
        }
        // Object 1 stands, 2 was destroyed and 3 never had a target.
        let mut targets = tore_world::test_support::spawned();
        targets.truncate(2);
        targets[1].hp = 0;
        let expected = |ids: &[u32], sign: f32| -> Vec<f32> {
            ids.iter()
                .flat_map(|id| vec![sign * *id as f32; 10])
                .collect()
        };
        assert_eq!(w.visible_static_vertices(&targets), expected(&[1], 1.));
        assert_eq!(w.visible_static_lines(&targets), expected(&[1], -1.));
        let destroyed = BTreeSet::from([2]);
        assert_eq!(
            w.visible_static_vertices_where(&destroyed),
            expected(&[1, 3], 1.)
        );
        assert_eq!(
            w.visible_static_lines_where(&destroyed),
            expected(&[1, 3], -1.)
        );
    }

    /// No surface unit state: no launcher shows its loads.
    fn quiet() -> tore_world::surface::SurfaceState {
        tore_world::surface::SurfaceState::new(0, Vec::new())
    }

    #[test]
    fn a_destroyed_placement_with_a_wreck_shows_it_in_its_place() {
        let mut scene = scenery();
        scene.static_vertices = BTreeMap::from([(1, vec![1.; 30]), (2, vec![2.; 30])]);
        scene.static_lines = BTreeMap::from([(2, vec![-2.; 20])]);
        scene.surface = surface_art::SurfaceArt::default().with_wreck(
            2,
            surface_art::Drawn {
                vertices: vec![9.; 30],
                lines: vec![-9.; 20],
            },
        );
        let mut targets = tore_world::test_support::spawned();
        targets.truncate(2);
        let standing = Arc::clone(scene.static_geometry(&targets, &quiet()));
        assert_eq!(standing.vertices, [vec![1.; 30], vec![2.; 30]].concat());
        targets[1].hp = 0;
        let wrecked = Arc::clone(scene.static_geometry(&targets, &quiet()));
        assert_eq!(wrecked.vertices, [vec![1.; 30], vec![9.; 30]].concat());
        assert_eq!(wrecked.lines, vec![-9.; 20]);
        // A replay that recorded the loss shows the same wreck; one that
        // did not shows the placement standing.
        let destroyed = BTreeSet::from([2]);
        assert_eq!(
            scene.static_geometry_where(&destroyed).vertices,
            wrecked.vertices
        );
        assert_eq!(
            scene.static_geometry_where(&BTreeSet::new()).vertices,
            standing.vertices
        );
        // Placement 1 has no wreck: destroyed, it is gone.
        targets[0].hp = 0;
        assert_eq!(
            scene.static_geometry(&targets, &quiet()).vertices,
            vec![9.; 30]
        );
    }

    #[test]
    fn static_cache_tracks_standing_objects_and_preserves_lines() {
        let mut scene = scenery();
        scene.static_vertices =
            BTreeMap::from([(1, vec![1.; 30]), (2, vec![2.; 60]), (3, vec![3.; 30])]);
        // Include line-only placement 4 and never-registered placement 3.
        scene.static_lines =
            BTreeMap::from([(1, vec![-1.; 20]), (2, vec![-2.; 40]), (4, vec![-4.; 20])]);
        let mut targets = tore_world::test_support::spawned();
        targets.truncate(2);
        let mut line_only = targets[0].clone();
        line_only.id = 4;
        targets.push(line_only);
        let full = Arc::clone(scene.static_geometry(&targets, &quiet()));
        assert_eq!(full.vertices, scene.visible_static_vertices(&targets));
        assert_eq!(full.lines, scene.visible_static_lines(&targets));
        assert_eq!(full.vertices.len(), 90);
        assert_eq!(full.lines.len(), 80);

        targets[0].position = [1234., 5000., 8765.];
        targets[1].hp = 1;
        scene.set_origin([30_000., 1000., -20_000.]);
        scene.palette = [[17; 3]; 256];
        assert!(Arc::ptr_eq(
            &full,
            scene.static_geometry(&targets, &quiet())
        ));

        targets[1].hp = 0;
        targets[2].hp = 0;
        let damaged = Arc::clone(scene.static_geometry(&targets, &quiet()));
        assert!(!Arc::ptr_eq(&full, &damaged));
        assert_eq!(damaged.vertices, scene.visible_static_vertices(&targets));
        assert_eq!(damaged.lines, scene.visible_static_lines(&targets));
        assert_eq!(damaged.vertices, vec![1.; 30]);
        assert_eq!(damaged.lines, vec![-1.; 20]);
        assert!(Arc::ptr_eq(
            &damaged,
            scene.static_geometry(&targets, &quiet())
        ));

        let empty = Arc::clone(scene.static_geometry(&[], &quiet()));
        assert!(empty.vertices.is_empty() && empty.lines.is_empty());
        // Restart revives targets without requiring a new renderer.
        targets[1].hp = 100;
        targets[2].hp = 100;
        let restored = scene.static_geometry(&targets, &quiet());
        assert!(!Arc::ptr_eq(&empty, restored));
        assert_eq!(restored.vertices, full.vertices);
        assert_eq!(restored.lines, full.lines);
    }

    #[test]
    fn static_cache_replay_seeks_replace_the_current_batch() {
        let mut scene = scenery();
        scene.static_vertices = BTreeMap::from([(1, vec![1.; 30]), (2, vec![2.; 30])]);
        scene.static_lines = BTreeMap::from([(2, vec![-2.; 20]), (3, vec![-3.; 20])]);
        let none = BTreeSet::new();
        let full = Arc::downgrade(scene.static_geometry_where(&none));
        let destroyed = BTreeSet::from([2, 3]);
        let expected = (
            scene.visible_static_vertices_where(&destroyed),
            scene.visible_static_lines_where(&destroyed),
        );
        let damaged = scene.static_geometry_where(&destroyed);
        assert_eq!(
            (&damaged.vertices, &damaged.lines),
            (&expected.0, &expected.1)
        );
        assert!(
            full.upgrade().is_none(),
            "only the current batch is retained"
        );
        let damaged = Arc::downgrade(damaged);
        let expected = (
            scene.visible_static_vertices_where(&none),
            scene.visible_static_lines_where(&none),
        );
        let restored = scene.static_geometry_where(&none);
        assert_eq!(
            (&restored.vertices, &restored.lines),
            (&expected.0, &expected.1)
        );
        assert!(damaged.upgrade().is_none());
    }

    #[test]
    fn static_cache_scene_replacement_has_a_distinct_upload_identity() {
        let mut first = scenery();
        let mut next = scenery();
        first.static_vertices.insert(1, vec![1.; 30]);
        next.static_vertices.insert(1, vec![2.; 30]);
        first.static_lines.insert(1, vec![3.; 20]);
        next.static_lines.insert(1, vec![4.; 20]);
        let none = BTreeSet::new();
        let previous = first.static_geometry_where(&none);
        let replacement = next.static_geometry_where(&none);
        assert!(!Arc::ptr_eq(previous, replacement));
        assert_ne!(previous.vertices, replacement.vertices);
        assert_ne!(previous.lines, replacement.lines);
    }
    /// A world's identity survives a recording header, and rebuilding from it
    /// restores the launch settings exactly, whatever set the wind.
    #[test]
    fn water_has_no_opaque_fallback_but_shore_art_keeps_its_geometry() {
        let mut terrain = tore_world::test_support::terrain();
        terrain.theater.cells[0].color = 255;
        let height = terrain.height(2048., 2048.);
        let mut textures = BTreeMap::new();
        let mesh = |terrain: &Terrain, textures: &_, layers: &_| {
            Scenery::build_mesh(&terrain.theater, textures, layers, None)
        };
        assert!(
            mesh(&terrain, &textures, &BTreeMap::new()).is_empty(),
            "open water must expose the ocean pass"
        );
        assert_eq!(terrain.height(2048., 2048.), height);

        // A water-colored base cell can still contain opaque beach artwork.
        // All four rotations must retain its geometry and texture identity.
        let layers = BTreeMap::from([((0, 0), 2)]);
        for rotation in 0..4 {
            textures.insert(
                (0, 0),
                TexturePlacement {
                    col: 0,
                    row: 0,
                    texture: 2,
                    rotation,
                    resource: None,
                },
            );
            let vertices = mesh(&terrain, &textures, &layers);
            assert_eq!(vertices.len(), 6 * 10);
            assert!(vertices.chunks_exact(10).all(|v| v[5] == 2.));
        }
        textures.clear();
        terrain.theater.cells[0].color = 100;
        assert_eq!(
            mesh(&terrain, &textures, &BTreeMap::new()).len(),
            6 * 10,
            "untextured land stays opaque"
        );
    }

    #[test]
    fn whiteout_cheat_clears_all_views_without_ticks_and_preserves_sun() {
        use tore_formats::weather::shape::{Primitive, WeatherShape};
        let mut w = tore_world::test_support::terrain();
        let mut s = scenery();
        let mut module =
            tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(1))
                .unwrap();
        let l = &mut module.layers[0];
        l.flags = 8;
        l.start_seconds = 0;
        l.end_seconds = 86399;
        l.sunrise_seconds = 0;
        l.sunset_seconds = 86400;
        let angles = tore_sim::environment::sun_angles(l, 9 * 3600).unwrap();
        w.weather = tore_sim::environment::Environment::new(
            tore_sim::environment::Configuration::new(module, 9, 0, 0, None).unwrap(),
        );
        let empty = WeatherShape {
            primitives: vec![],
            scale_exponent: 8,
            publishes_point: false,
        };
        s.celestial = Some(crate::celestial::Celestial {
            sun: WeatherShape {
                primitives: vec![Primitive::Circle {
                    center: [0., 0., 160.],
                    diameter: 4,
                    fill: 254,
                }],
                ..empty.clone()
            },
            moon: empty.clone(),
            stars: empty,
            sun_effects: true,
            moon_texture: 0,
            moon_uv: [0., 0., 1., 1.],
            sun_remap: 0,
            shade_rows: BTreeMap::new(),
            light_rows: [0; 2],
            flare: tore_formats::weather::flare::Layout {
                circles: vec![tore_formats::weather::flare::Circle {
                    offset_percent: 50,
                    radius: 10,
                    fill: 265,
                }],
            },
        });
        let sun = crate::celestial::rotate([0., 0., 1.], angles);
        let mut forward = Camera::new();
        forward.position[1] = 5000.;
        forward.yaw = sun[0].atan2(sun[2]);
        forward.pitch = sun[1].asin() - 0.1;
        let mut rear = Camera::new();
        rear.weather_slot = 1;
        rear.position = forward.position;
        rear.yaw = forward.yaw + std::f32::consts::PI;
        rear.pitch = -forward.pitch;
        for _ in 0..240 {
            w.weather.step();
            s.step_view_weather(&w, &forward, 700.);
            s.step_view_weather(&w, &rear, 700.);
        }
        assert!(s.weather_presentation.sun_whitening > 0);
        assert_eq!(s.auxiliary_presentations[0].sun_whitening, 0);
        assert!(!crate::lens_flare::circles(&w, &s, &forward, [1280, 720]).is_empty());
        let sun_geometry = s.celestial.as_ref().unwrap().sun_uniform(&w, &s, 5000.);
        let bright = s.sample_view(&w, 5000., 0).palette;
        let ticks = w.weather.ticks();
        s.no_sun_whiteout = true;
        assert!(crate::lens_flare::circles(&w, &s, &forward, [1280, 720]).is_empty());
        assert_ne!(bright, s.sample_view(&w, 5000., 0).palette);
        assert_eq!(
            s.sample_view(&w, 5000., 0).palette,
            s.sample_view(&w, 5000., 1).palette
        );
        assert_eq!(
            sun_geometry,
            s.celestial.as_ref().unwrap().sun_uniform(&w, &s, 5000.)
        );
        assert_eq!(ticks, w.weather.ticks());
    }

    #[test]
    fn camera_weather_is_altitude_local_and_query_order_is_pure() {
        let mut module =
            tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(2))
                .unwrap();
        for layer in &mut module.layers {
            layer.start_seconds = 0;
            layer.end_seconds = 86399;
        }
        module.layers[0].high_feet = 8000;
        module.layers[0].fog_far = 100;
        module.layers[0].tint_scalar = 200;
        module.layers[1].low_feet = 7500;
        module.layers[1].fog_far = 1000;
        module.layers[1].tint_scalar = 0;
        let mut w = tore_world::test_support::terrain();
        let mut s = scenery();
        w.weather = tore_sim::environment::Environment::new(
            tore_sim::environment::Configuration::new(module, 12, 0, 0, None).unwrap(),
        );
        let mut low = Camera::new();
        low.position[1] = 7000.;
        let mut high = Camera::new();
        high.weather_slot = 1;
        high.position[1] = 9000.;
        for _ in 0..120 {
            w.weather.step();
            s.step_view_weather(&w, &low, 700.);
            s.step_view_weather(&w, &high, 700.);
        }
        assert_ne!(
            s.weather_presentation.tint,
            s.auxiliary_presentations[0].tint
        );
        let state = (
            w.weather.clone(),
            s.weather_presentation.clone(),
            s.auxiliary_presentations.clone(),
        );
        let low_view = s.sample_view(&w, 7000., 0);
        let high_view = s.sample_view(&w, 9000., 1);
        assert_ne!(low_view.fog, high_view.fog);
        for _ in 0..10 {
            assert_eq!(high_view.palette, s.sample_view(&w, 9000., 1).palette);
            assert_eq!(low_view.palette, s.sample_view(&w, 7000., 0).palette);
        }
        assert_eq!(
            state,
            (w.weather, s.weather_presentation, s.auxiliary_presentations)
        );
    }
}
