//! Renderer-independent world data and free-camera controls (feet, X east/Y up/Z north).
use crate::AppResult;
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{
    Pic,
    theater::{CELL_FEET, Environment, HEIGHT_FEET, Theater},
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

pub struct World {
    pub ocean_motion: crate::ocean::Motion,
    pub theater: Theater,
    /// Exact selected MM identity, distinct from its referenced base grid.
    pub layout: String,
    pub land_texture: Option<usize>,
    pub environment: Environment,
    /// Immutable imported placement/airport geometry. Mutable health belongs to combat.
    pub airport_scene: tore_sim::airport::Scene,
    /// Each runway's taxi, takeoff, landing and parking points from its STRIP
    /// shape, by runway object id. A runway is absent when its shape lacks a
    /// point or a point is off the airport surface.
    pub airfield_anchors: BTreeMap<u32, tore_sim::ai::airfield::AirfieldAnchors>,
    /// Every source placement, including definitions the bounded SH projector cannot draw.
    pub static_manifest: Vec<(u32, tore_formats::mission::SourceKey, String, bool)>,
    pub catalog: Vec<(String, String)>,
    /// Source palette indices, one byte per texel. Retail terrain and sky art is
    /// entirely weather-palette indexed, so the artwork is uploaded unresolved
    /// and the live palette is applied on the GPU. 255 is the water cutout.
    pub sky_indices: Vec<u8>,
    pub celestial: Option<crate::celestial::Celestial>,
    pub clouds: Option<crate::clouds::Clouds>,
    pub deck_textures: BTreeMap<String, usize>,
    pub decks: [[f32; 4]; 2],
    pub vertices: Vec<f32>,
    /// Per-placement geometry, rebuilt into the dynamic scene from combat HP.
    pub static_vertices: BTreeMap<u32, Vec<f32>>,
    pub static_lines: BTreeMap<u32, Vec<f32>>,
    pub texture_indices: Vec<u8>,
    /// Authoritative environment. One instance per world, so every camera,
    /// mirror and panel resolves the same instant.
    pub weather: tore_sim::environment::Environment,
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
}
/// Fitted grounding: align the largest aggregate horizontal pavement layer,
/// not a terminal roof or the whole mesh's midpoint, with airport ground.
fn pavement_height(shape: &tore_formats::shape::Shape) -> f64 {
    let mut areas = BTreeMap::<u32, f64>::new();
    for face in &shape.faces {
        if face.positions.len() < 3 {
            continue;
        }
        let height = face.positions[0][2];
        if face.positions.iter().any(|p| (p[2] - height).abs() > 0.01) {
            continue;
        }
        let area = face
            .positions
            .iter()
            .zip(face.positions.iter().cycle().skip(1))
            .map(|(a, b)| f64::from(a[0]) * f64::from(b[1]) - f64::from(b[0]) * f64::from(a[1]))
            .sum::<f64>()
            .abs()
            * 0.5;
        if area > 0. {
            *areas.entry(height.to_bits()).or_default() += area;
        }
    }
    areas
        .into_iter()
        .max_by(|a, b| {
            a.1.total_cmp(&b.1)
                .then_with(|| f32::from_bits(b.0).total_cmp(&f32::from_bits(a.0)))
        })
        .map_or(0., |(height, _)| f64::from(f32::from_bits(height)))
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

/// Spec-derived: the STRIP template roles in `docs/formats/native-strip.md`
/// ("Remaining template callback boundaries"). Box midpoints are feet in the
/// shape's frame ([right, up, forward]); `place` puts one in the world. None
/// unless every point the airfield sequences use is present.
fn airfield_anchors(
    boxes: &[tore_formats::shape::ContactBox],
    heading: f64,
    place: impl Fn([f64; 3]) -> [f64; 3],
) -> Option<tore_sim::ai::airfield::AirfieldAnchors> {
    // The native lookup returns the first box with an id.
    let point = |id: u8| {
        boxes
            .iter()
            .find(|b| b.id == id)
            .map(|b| place(b.midpoint().map(f64::from)))
    };
    let points = |first: u8| -> Option<[[f64; 3]; 4]> {
        Some([
            point(first)?,
            point(first + 1)?,
            point(first + 2)?,
            point(first + 3)?,
        ])
    };
    let mut parking = [[0.; 3]; 9];
    for (slot, place) in parking.iter_mut().enumerate() {
        *place = point(0x19 + slot as u8)?;
    }
    Some(tore_sim::ai::airfield::AirfieldAnchors {
        taxi_out: points(0x25)?,
        takeoff_spot: point(0x11)?,
        // Box 0x17's recorded orientation is zero on every reviewed STRIP, so
        // the runway heading is the placed airport heading.
        takeoff_heading: heading,
        landing_point: point(0x12)?,
        // `fitted`: box 0x18's heading is not decoded. The landing aim point
        // is behind the takeoff spot, sometimes on a parallel centerline.
        // The host uses the takeoff direction for landings.
        landing_heading: heading,
        taxi_in: points(0x29)?,
        parking,
        parking_heading: (heading + std::f64::consts::FRAC_PI_2).rem_euclid(std::f64::consts::TAU),
    })
}

fn anchor_points(
    anchors: &tore_sim::ai::airfield::AirfieldAnchors,
) -> impl Iterator<Item = [f64; 3]> + '_ {
    anchors
        .taxi_out
        .iter()
        .chain([&anchors.takeoff_spot, &anchors.landing_point])
        .chain(&anchors.taxi_in)
        .chain(&anchors.parking)
        .copied()
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
        let layout = format!("{}.MM", code.trim_end_matches(".MM"));
        let base =
            tore_formats::theater::base_theater(&layout).ok_or("unknown retail map layout")?;
        let mut environment = Environment::parse(required(&layout)?)?;
        if tore_formats::theater::base_theater(&environment.map) != Some(base) {
            return Err("layout and terrain identities disagree".into());
        }
        let grid = resources
            .get(&environment.map)
            .or_else(|| resources.get(&format!("{base}.T2")))
            .ok_or("missing base terrain grid")?;
        let mut theater = Theater::parse(grid)?;
        let catalog = tore_formats::theater::map_catalog(resources)?;
        if let Some((_, label)) = catalog
            .iter()
            .find(|(id, _)| *id == code.trim_end_matches(".MM"))
        {
            theater.name.clone_from(label);
        }
        if theater.cols < 2 || theater.rows < 2 {
            return Err("unsupported theater dimensions".into());
        }
        let (layer, launch) = match condition {
            Some(index) => {
                let choice = tore_sim::environment::CONDITIONS
                    .get(index)
                    .ok_or("weather condition outside source table")?;
                (
                    tore_sim::environment::layer_resource(index, &format!("{base}.T2"))?,
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
        let wind = match std::env::var("TORE_WIND") {
            Ok(value) => {
                let (heading, speed) = value
                    .split_once(',')
                    .ok_or("TORE_WIND needs heading,speed in degrees/feet per second")?;
                Some([heading.parse::<i32>()?, speed.parse::<i32>()?])
            }
            Err(std::env::VarError::NotPresent) => environment.wind,
            Err(e) => return Err(e.into()),
        };
        let weather =
            tore_sim::environment::Environment::new(tore_sim::environment::Configuration::new(
                module,
                hour,
                minute,
                condition.map_or_else(|| environment.layer_parameter.unwrap_or(0), |i| i as i32),
                wind,
            )?);
        if weather.sample(0.).is_none() {
            return Err("mission weather layer covers no altitude at its launch time".into());
        }
        // Preserve the resolved launch identity for validation and restart.
        environment.layer = layer;
        environment.layer_parameter = Some(weather.configuration().parameter());
        environment.time = Some([hour, minute]);
        let cloud_altitude = if let Ok(value) = std::env::var("TORE_CLOUD_ALTITUDE") {
            let value = value.parse::<i32>()?;
            if !(0..=400_000).contains(&value) {
                return Err("cloud altitude outside 0..400000 feet".into());
            }
            value
        } else if let Some(choice) = condition {
            tore_sim::clouds::generated_altitude(
                choice,
                &mut tore_formats::flight_model::clock_rng::NativeRng::seeded(1)?,
            )?
        } else {
            environment.clouds.unwrap_or(0)
        };
        if !(0..=400_000).contains(&cloud_altitude) {
            return Err("mission cloud altitude outside supported range".into());
        }
        environment.clouds = Some(cloud_altitude);
        let mut texture_indices = Vec::new();
        let mut terrain_layers = BTreeMap::new();
        for placement in environment.textures.values_mut() {
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
            // This world-local layer is not written back to the imported data.
            placement.texture = layer;
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
            cloud_altitude,
        )?);
        let mut out = Self {
            ocean_motion: crate::ocean::Motion::from_environment()?,
            theater,
            layout,
            land_texture,
            environment,
            airport_scene: tore_sim::airport::Scene::default(),
            airfield_anchors: BTreeMap::new(),
            static_manifest: Vec::new(),
            catalog,
            sky_indices,
            celestial,
            clouds,
            deck_textures,
            decks: [[0., 1., -1., 0.]; 2],
            vertices: Vec::new(),
            static_vertices: BTreeMap::new(),
            static_lines: BTreeMap::new(),
            texture_indices,
            weather,
            visual_bands: Vec::new(),
            smooth_weather: match std::env::var("TORE_WEATHER_SMOOTH").as_deref() {
                Err(std::env::VarError::NotPresent) | Ok("1") => true,
                Ok("0") => false,
                _ => return Err("TORE_WEATHER_SMOOTH must be 0 or 1".into()),
            },
            no_sun_whiteout: false,
            auxiliary_presentations: std::array::from_fn(|_| {
                tore_sim::environment::Presentation::seeded(1).unwrap()
            }),
            weather_presentation: tore_sim::environment::Presentation::seeded(1)?,
            palette: [[0; 3]; 256],
            fog_palette: Vec::new(),
            fog: [0.; 4],
            haze: [0; 3],
        };
        out.resolve_palette(0.);
        out.build_mesh();
        out.build_airport_scene(resources, code.trim_end_matches(".MM"))?;
        Ok(out)
    }

    fn build_airport_scene(
        &mut self,
        resources: &BTreeMap<String, Vec<u8>>,
        code: &str,
    ) -> AppResult<()> {
        use tore_sim::airport::{
            Airport, Allegiance, OrientedBox, Runway, SourceKey, StaticObject,
        };
        let layout_name = format!("{code}.MM");
        let layout = tore_formats::mission::Layout::parse(
            &layout_name,
            resources
                .get(&layout_name)
                .ok_or_else(|| format!("missing airport layout {layout_name}"))?,
        )?;
        let mut definitions = BTreeMap::new();
        let mut shapes = BTreeMap::new();
        let mut shape_scales = BTreeMap::new();
        let mut runway_anchors = BTreeMap::new();
        let mut strip_boxes = BTreeMap::new();
        for placement in &layout.placements {
            if definitions.contains_key(&placement.object_type) {
                continue;
            }
            let definition = tore_formats::static_object::Definition::parse(
                resources.get(&placement.object_type).ok_or_else(|| {
                    format!(
                        "{}: missing placed definition {}; re-import media",
                        layout_name, placement.object_type
                    )
                })?,
            )?;
            if let Some(main_shape) = &definition.main_shape {
                let shape_bytes = resources.get(main_shape).ok_or_else(|| {
                    format!(
                        "{}: missing shape {} referred by {}; re-import media",
                        layout_name, main_shape, placement.object_type
                    )
                })?;
                let parsed = tore_formats::shape::Shape::scenery(shape_bytes);
                match parsed {
                    Ok(shape) => {
                        if definition.callbacks.iter().any(|name| name == "_STRIPProc")
                            && let Some(boxes) = tore_formats::shape::contact_boxes(shape_bytes)?
                            && let Some(anchor) = boxes.iter().find(|b| b.id == 0x11)
                        {
                            runway_anchors.insert(
                                placement.object_type.clone(),
                                anchor.midpoint().map(f64::from),
                            );
                            strip_boxes.insert(placement.object_type.clone(), boxes.clone());
                        }
                        shape_scales.insert(
                            placement.object_type.clone(),
                            tore_formats::shape::object_scale(shape_bytes)?,
                        );
                        shapes.insert(placement.object_type.clone(), shape);
                    }
                    Err(error) => eprintln!(
                        "Airport scene: {main_shape} retained without visual geometry: {error}"
                    ),
                }
            }
            definitions.insert(placement.object_type.clone(), definition);
        }
        let mut objects = Vec::new();
        let mut runways = Vec::new();
        let mut airports = Vec::new();
        let mut anchors = BTreeMap::new();
        let mut static_layers = BTreeMap::<String, crate::static_art::Image>::new();
        let mut static_float_count = 0usize;
        for placement in &layout.placements {
            let definition = &definitions[&placement.object_type];
            let shape = shapes.get(&placement.object_type);
            let id = 0x4000_0000u32
                .checked_add(placement.key.ordinal)
                .ok_or("airport object ID overflow")?;
            self.static_manifest.push((
                id,
                placement.key.clone(),
                placement.object_type.clone(),
                shape.is_some(),
            ));
            let Some(shape) = shape else {
                continue;
            };
            let ground =
                f64::from(self.height(placement.position[0] as f32, placement.position[2] as f32));
            let heading = f64::from(placement.angles[0]).to_radians();
            let runway = definition.callbacks.iter().any(|name| name == "_STRIPProc");
            // The source runway plane stays at authored ground. The renderer
            // applies a bounded static-surface depth bias without changing contact.
            let support_height = ground;
            let mut min = [f64::INFINITY; 3];
            let mut max = [f64::NEG_INFINITY; 3];
            for point in shape.faces.iter().flat_map(|face| &face.positions) {
                let mapped = [
                    f64::from(point[0]),
                    f64::from(point[2]),
                    f64::from(point[1]),
                ];
                for axis in 0..3 {
                    min[axis] = min[axis].min(mapped[axis]);
                    max[axis] = max[axis].max(mapped[axis]);
                }
            }
            if min.iter().any(|value| !value.is_finite()) {
                continue;
            }
            // The reviewed SH header exponent drives both visual and contact scale.
            let scale = shape_scales
                .get(&placement.object_type)
                .copied()
                .unwrap_or(1.0);
            for axis in 0..3 {
                min[axis] *= scale;
                max[axis] *= scale;
            }
            let half = std::array::from_fn(|axis| ((max[axis] - min[axis]) * 0.5).max(1.0));
            let pitch = f64::from(placement.angles[1]).to_radians();
            let bank = f64::from(placement.angles[2]).to_radians();
            let basis = tore_sim::attitude::Basis::new(heading, pitch, bank);
            let local_center = std::array::from_fn::<_, 3, _>(|axis| (min[axis] + max[axis]) * 0.5);
            let support_origin = [
                f64::from(placement.position[0]),
                support_height + f64::from(placement.position[1]),
                f64::from(placement.position[2]),
            ];
            let grounding_offset = if runway {
                -pavement_height(shape) * scale
            } else {
                0.
            };
            let origin = std::array::from_fn::<_, 3, _>(|axis| {
                support_origin[axis] + basis.up[axis] * grounding_offset
            });
            let center = std::array::from_fn(|axis| {
                origin[axis]
                    + basis.right[axis] * local_center[0]
                    + basis.up[axis] * local_center[1]
                    + basis.forward[axis] * local_center[2]
            });
            let bounds = OrientedBox {
                center,
                half,
                heading,
                pitch,
                bank,
            };
            objects.push(StaticObject {
                id,
                source: SourceKey {
                    layout: placement.key.layout.clone(),
                    ordinal: placement.key.ordinal,
                },
                name: placement
                    .name
                    .clone()
                    .unwrap_or_else(|| definition.display_name.clone()),
                object_type: placement.object_type.clone(),
                bounds,
                hit_points: definition.hit_points.unwrap_or(100),
                runway,
                category: definition.category,
                radar_signature: f64::from(definition.radar_signature),
                infrared_signature: f64::from(definition.infrared_signature),
            });
            if runway {
                let airport_id = u32::try_from(airports.len() + 1)?;
                // The whole airport mesh includes aprons and parallel strips.
                // Use source anchor0x11 for the fitted primary approach line,
                // rather than steering onto the overall mesh's midpoint.
                let mut approach_center = center;
                // Even a fallback centerline belongs to the plane through the
                // placement origin, not the whole airport mesh's vertical center.
                if basis.up[1].abs() > 1e-6 {
                    approach_center[1] = support_origin[1]
                        - (basis.up[0] * (center[0] - support_origin[0])
                            + basis.up[2] * (center[2] - support_origin[2]))
                            / basis.up[1];
                }
                let mut length_ft = half[2] * 2.0;
                if let Some(anchor) = runway_anchors.get(&placement.object_type)
                    && anchor[2] < max[2]
                    && anchor[2] >= min[2]
                {
                    let local = [anchor[0], 0.0, (anchor[2] + max[2]) * 0.5];
                    approach_center = std::array::from_fn(|axis| {
                        support_origin[axis]
                            + basis.right[axis] * local[0]
                            + basis.forward[axis] * local[2]
                    });
                    length_ft = max[2] - anchor[2];
                }
                if let Some(found) = strip_boxes.get(&placement.object_type).and_then(|boxes| {
                    airfield_anchors(boxes, heading, |local| {
                        std::array::from_fn(|axis| {
                            support_origin[axis]
                                + basis.right[axis] * local[0]
                                + basis.forward[axis] * local[2]
                        })
                    })
                }) {
                    anchors.insert(id, found);
                }
                runways.push(Runway {
                    object: id,
                    airport: airport_id,
                    name: placement
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("Runway {airport_id}")),
                    surface: bounds,
                    approach_center,
                    // ILS datum remains authored airport ground, independent of rendering bias.
                    elevation_ft: ground,
                    heading,
                    length_ft,
                });
                airports.push(Airport {
                    id: airport_id,
                    name: placement
                        .name
                        .clone()
                        .unwrap_or_else(|| format!("Airport {airport_id}")),
                    runway_objects: vec![id],
                    // Base free flight has no mission-side player assignment.
                    // Treat imported fields as neutral with explicit host permission.
                    allegiance: Allegiance::Neutral,
                    neutral_permission: true,
                });
            }
            let mut instance_vertices = Vec::new();
            for face in &shape.faces {
                if face.positions.len() < 3 {
                    continue;
                }
                let image = if face.texture.is_empty() || face.uv.is_empty() {
                    None
                } else {
                    let name = face.texture.to_ascii_uppercase();
                    if !static_layers.contains_key(&name) {
                        let pic = Pic::parse(
                            resources
                                .get(&name)
                                .ok_or_else(|| format!("missing static texture {name}"))?,
                        )?;
                        let first = (self.texture_indices.len() + self.sky_indices.len()) / 65536;
                        let image =
                            crate::static_art::Image::append(&pic, &mut self.sky_indices, first)?;
                        static_layers.insert(name.clone(), image);
                    }
                    static_layers.get(&name)
                };
                for triangle in 1..face.positions.len() - 1 {
                    if static_float_count + instance_vertices.len() + 30 > 32 * 1024 * 1024 / 4 {
                        return Err("static scene exceeds 32 MiB geometry budget".into());
                    }
                    let mut points = Vec::with_capacity(3);
                    for vertex_index in [0, triangle, triangle + 1] {
                        let point = face.positions[vertex_index];
                        let right = f64::from(point[0]) * scale;
                        let up = f64::from(point[2]) * scale;
                        let forward = f64::from(point[1]) * scale;
                        let position = std::array::from_fn::<_, 3, _>(|axis| {
                            origin[axis]
                                + basis.right[axis] * right
                                + basis.up[axis] * up
                                + basis.forward[axis] * forward
                        });
                        let uv = face.uv.get(vertex_index).copied().unwrap_or([0.0; 2]);
                        let uv = image.map_or([0., 0.], |image| {
                            [uv[0] + 0.5, image.height as f32 - 0.5 - uv[1]]
                        });
                        points.push([
                            position[0] as f32,
                            position[1] as f32,
                            position[2] as f32,
                            uv[0],
                            uv[1],
                            -1.0,
                            0.0,
                            0.0,
                            0.0,
                            f32::from(face.colors[vertex_index]) + f32::from(face.fog as u8) * 256.,
                        ]);
                    }
                    if let Some(image) = image {
                        image.triangle(
                            points.try_into().expect("three triangle corners"),
                            &mut instance_vertices,
                            32 * 1024 * 1024 / 4 - static_float_count,
                        )?;
                    } else {
                        instance_vertices.extend(points.into_iter().flatten());
                    }
                }
            }
            let mut line_vertices = Vec::new();
            for line in &shape.lines {
                for point in line.positions {
                    let [right, forward, up] = point.map(|v| f64::from(v) * scale);
                    let position = std::array::from_fn::<_, 3, _>(|axis| {
                        origin[axis]
                            + basis.right[axis] * right
                            + basis.up[axis] * up
                            + basis.forward[axis] * forward
                    });
                    line_vertices.extend([
                        position[0] as f32,
                        position[1] as f32,
                        position[2] as f32,
                        0.,
                        0.,
                        -1.,
                        0.,
                        0.,
                        0.,
                        f32::from(line.color) + f32::from(line.fog as u8) * 256.,
                    ]);
                }
            }
            static_float_count += line_vertices.len();
            self.static_lines.insert(id, line_vertices);
            static_float_count += instance_vertices.len();
            if static_float_count > 32 * 1024 * 1024 / 4 {
                return Err("static scene exceeds 32 MiB geometry budget".into());
            }
            self.static_vertices.insert(id, instance_vertices);
        }
        self.airport_scene = tore_sim::airport::Scene {
            objects,
            runways,
            airports,
        };
        // Every point must stand on the airport's landable surface.
        let scene = &self.airport_scene;
        anchors.retain(|_, found: &mut tore_sim::ai::airfield::AirfieldAnchors| {
            anchor_points(found).all(|p| scene.runway_surface(p[0], p[2]).is_some())
        });
        self.airfield_anchors = anchors;
        self.airport_scene.validate().map_err(|error| error.into())
    }

    /// The AI's view of one runway, with its airfield points when known.
    pub fn runway_view(&self, object: u32) -> Option<tore_sim::ai::airfield::RunwayView> {
        self.airport_scene.runway(object).map(|runway| {
            tore_sim::ai::airfield::RunwayView::from(runway)
                .with_anchors(self.airfield_anchors.get(&object).copied())
        })
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
                let layer = placement.map_or_else(
                    || self.land_texture.map_or(-1.0, |l| l as f32),
                    |p| p.texture as f32,
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

    /// Native T_Info/Collision publishes whether the winning terrain class is 1.
    /// This host samples the T2 cell under the aircraft; native object/carrier
    /// collision overrides and triangle-boundary parity remain unimplemented.
    pub fn turbulence_reduced_surface(&self, x: f64, z: f64) -> bool {
        let col = (x / f64::from(CELL_FEET))
            .floor()
            .clamp(0., (self.theater.cols - 1) as f64) as usize;
        let row = (z / f64::from(CELL_FEET))
            .floor()
            .clamp(0., (self.theater.rows - 1) as f64) as usize;
        self.theater.cell(col, row).class == 1
    }

    /// Whether the T2 cell under the point is water: terrain class 1, the
    /// class the original's collision query reports as water. Outside the grid
    /// the original's fallback cell is water too. See
    /// docs/formats/native-land-contact.md.
    pub fn over_water(&self, x: f64, z: f64) -> bool {
        let col = (x / f64::from(CELL_FEET)).floor();
        let row = (z / f64::from(CELL_FEET)).floor();
        if col < 0.
            || row < 0.
            || col >= self.theater.cols as f64
            || row >= self.theater.rows as f64
        {
            return true;
        }
        self.theater.cell(col as usize, row as usize).class == 1
    }

    /// Explicit authored standard atmosphere, shared wind and rendered terrain.
    /// No weather-derived temperature/pressure is inferred from LAY colors.
    pub fn air_data(
        &self,
        state: &crate::flight::State,
    ) -> tore_formats::Result<tore_sim::telemetry::AirData> {
        tore_sim::telemetry::AirData::sample(
            state,
            tore_sim::telemetry::EnvironmentReading {
                terrain_msl_ft: f64::from(
                    self.height(state.position[0] as f32, state.position[2] as f32),
                ),
                wind_world_fps: self.wind(),
                atmosphere: tore_sim::telemetry::Atmosphere::standard(state.position[1])?,
            },
        )
    }

    /// The terrain surface plus the environment's wind, for one fixed step.
    pub fn surface(&self, x: f64, z: f64) -> tore_sim::research::Surface {
        if let Some((_, height)) = self.airport_scene.runway_surface(x, z) {
            let mut surface = tore_sim::research::Surface::runway(height);
            surface.wind = self.wind();
            return surface;
        }
        let mut surface =
            tore_sim::research::Surface::terrain(f64::from(self.height(x as f32, z as f32)));
        surface.wind = self.wind();
        surface
    }

    pub fn visible_static_vertices(&self, targets: &[tore_sim::combat::live::Target]) -> Vec<f32> {
        self.visible_static_geometry(&self.static_vertices, targets)
    }
    pub fn visible_static_lines(&self, targets: &[tore_sim::combat::live::Target]) -> Vec<f32> {
        self.visible_static_geometry(&self.static_lines, targets)
    }
    fn visible_static_geometry(
        &self,
        geometry: &BTreeMap<u32, Vec<f32>>,
        targets: &[tore_sim::combat::live::Target],
    ) -> Vec<f32> {
        let alive: BTreeSet<u32> = targets
            .iter()
            .filter(|target| target.hp > 0)
            .map(|target| target.id)
            .collect();
        let total = geometry
            .iter()
            .filter(|(id, _)| alive.contains(id))
            .map(|(_, vertices)| vertices.len())
            .sum();
        let mut out = Vec::with_capacity(total);
        for (id, vertices) in geometry {
            if alive.contains(id) {
                out.extend_from_slice(vertices);
            }
        }
        out
    }

    /// Earliest solid building contact. Runways remain a separate surface query.
    pub fn solid_contact(
        &self,
        from: [f64; 3],
        to: [f64; 3],
        alive: impl IntoIterator<Item = u32>,
    ) -> Option<(u32, f64)> {
        let alive: BTreeSet<_> = alive.into_iter().collect();
        self.airport_scene
            .objects
            .iter()
            .filter(|object| !object.runway && alive.contains(&object.id))
            .filter_map(|object| {
                object
                    .bounds
                    .segment_fraction(from, to)
                    .map(|at| (object.id, at))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)))
    }

    /// Exactly one 120 Hz tick of environment time. Pausing means not calling it.
    pub fn step_weather(&mut self, speed_fps: f64, camera: &Camera) {
        self.weather.step();
        self.step_view_weather(camera, speed_fps);
    }

    pub fn glare_enabled(&self) -> bool {
        !self.no_sun_whiteout && self.celestial.as_ref().is_some_and(|c| c.sun_effects)
    }

    /// Each fixed camera slot advances once per simulation tick, even if hidden.
    /// Slots have independent seeded presentation state; queries never consume RNG.
    pub fn step_view_weather(&mut self, camera: &Camera, speed_fps: f64) {
        let altitude = f64::from(camera.position[1]);
        let alignment = if self.glare_enabled() {
            self.weather
                .sample(altitude)
                .and_then(|l| tore_sim::environment::sun_angles(&l, self.weather.seconds_of_day()))
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
            self.weather
                .sample(altitude)
                .and_then(|layer| crate::celestial::visual_sun_direction(&layer, &self.weather))
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
                    response * f64::from(crate::celestial::glare_strength(self, altitude, sun))
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
        presentation.step_with_alignment(&self.weather, altitude, speed_fps, alignment);
        if self.smooth_weather {
            presentation.visual_sun =
                previous_visual + (visual_target - previous_visual).clamp(-16. / 7.2, 16. / 7.2);
        }
    }

    /// Presentation only: resolves the palette for one camera altitude without
    /// advancing state, so mirrors and camera panels stay on the same instant.
    pub fn resolve_palette(&mut self, altitude_ft: f64) {
        let view = self.sample_view(altitude_ft, 0);
        self.palette = view.palette;
        self.fog_palette = view.fog_palette;
        self.decks = view.decks;
        self.fog = view.fog;
        self.haze = view.haze;
        self.visual_bands = view.visual_bands;
    }

    pub fn sample_view(&self, altitude_ft: f64, slot: usize) -> ViewWeather {
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
            .then(|| self.weather.visual_sample(altitude_ft))
            .flatten();
        let Some(layer) = self.weather.sample(altitude_ft) else {
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
            self.weather.configuration().base_palette(),
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
        out.fog_palette = self
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
    /// 0 main, 1 rear mirror, 2 forward panel, 3 other panel, 4 target.
    pub weather_slot: usize,
    pub hidden_target: Option<u32>,
    pub hidden_projectile: Option<u32>,
    pub position: [f32; 3],
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
            position: [1_070_000.0, 28_000.0, 590_000.0],
            yaw: 0.3,
            pitch: -0.32,
            roll: 0.,
            view_fraction: 1.,
            zoom: 1.,
            near_clip: 1.,
            keys: BTreeSet::new(),
        }
    }
    pub fn for_world(world: &World) -> Self {
        let mut camera = Self::new();
        if tore_formats::theater::base_theater(&world.layout) != Some("UKR") {
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
    /// Where the renderer draws a world point in a `size` pixel view, if it is
    /// in front of the camera and on screen.
    pub fn project(&self, size: [u32; 2], point: [f64; 3]) -> Option<[f64; 2]> {
        let (sy, cy) = f64::from(self.yaw).sin_cos();
        let (sp, cp) = f64::from(self.pitch).sin_cos();
        let (sr, cr) = f64::from(self.roll).sin_cos();
        let right = [cy * cr - sy * sp * sr, cp * sr, -sy * cr - cy * sp * sr];
        let up = [-cy * sr - sy * sp * cr, cp * cr, sy * sr - cy * sp * cr];
        let forward = [sy * cp, sp, cy * cp];
        let d: [f64; 3] = std::array::from_fn(|i| point[i] - f64::from(self.position[i]));
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
pub(crate) mod tests {
    use super::*;
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
    pub(crate) fn world() -> World {
        use tore_formats::theater::TerrainCell;
        let cells = [0, 4, 8, 12]
            .map(|elevation| TerrainCell {
                color: 100,
                class: 2,
                elevation,
            })
            .to_vec();
        World {
            ocean_motion: crate::ocean::Motion::default(),
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
            airport_scene: tore_sim::airport::Scene::default(),
            airfield_anchors: BTreeMap::new(),
            static_manifest: Vec::new(),
            catalog: vec![],
            vertices: vec![],
            static_vertices: BTreeMap::new(),
            static_lines: BTreeMap::new(),
            layout: "TEST.MM".into(),
            land_texture: None,
            texture_indices: vec![],
            sky_indices: vec![],
            celestial: None,
            clouds: None,
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
            visual_bands: Vec::new(),
            smooth_weather: true,
            palette: [[100; 3]; 256],
            no_sun_whiteout: false,
            auxiliary_presentations: std::array::from_fn(|_| {
                tore_sim::environment::Presentation::seeded(1).unwrap()
            }),
            weather_presentation: tore_sim::environment::Presentation::seeded(1).unwrap(),
            fog_palette: vec![[[100; 3]; 256]; 10],
        }
    }
    #[test]
    fn dominant_pavement_not_roof_controls_grounding() {
        use tore_formats::shape::{Face, FogMode, Shape};
        let face = |width: f32, length: f32, height: f32| Face {
            positions: vec![
                [0., 0., height],
                [width, 0., height],
                [width, length, height],
                [0., length, height],
            ],
            colors: vec![1; 4],
            fog: FogMode::Enabled,
            uv: vec![],
            texture: String::new(),
            subtype: 0x59,
            normal: None,
            address: 0,
        };
        let shape = Shape {
            lines: vec![],
            faces: vec![
                face(40., 100., -1.),
                face(40., 100., -1.),
                face(50., 100., 20.),
            ],
            state_words: Default::default(),
        };
        assert_eq!(pavement_height(&shape), -1.);
        assert_eq!(
            pavement_height(&Shape {
                lines: vec![],
                faces: vec![],
                state_words: Default::default()
            }),
            0.
        );
    }
    #[test]
    fn water_has_no_opaque_fallback_but_shore_art_keeps_its_geometry() {
        use tore_formats::theater::TexturePlacement;
        let mut w = world();
        w.theater.cells[0].color = 255;
        let height = w.height(2048., 2048.);
        w.build_mesh();
        assert!(
            w.vertices.is_empty(),
            "open water must expose the ocean pass"
        );
        assert_eq!(w.height(2048., 2048.), height);

        // A water-colored base cell can still contain opaque beach artwork.
        // All four rotations must retain its geometry and texture identity.
        for rotation in 0..4 {
            w.environment.textures.insert(
                (0, 0),
                TexturePlacement {
                    col: 0,
                    row: 0,
                    texture: 2,
                    rotation,
                    resource: None,
                },
            );
            w.vertices.clear();
            w.build_mesh();
            assert_eq!(w.vertices.len(), 6 * 10);
            assert!(w.vertices.chunks_exact(10).all(|v| v[5] == 2.));
        }
        w.environment.textures.clear();
        w.theater.cells[0].color = 100;
        w.vertices.clear();
        w.build_mesh();
        assert_eq!(w.vertices.len(), 6 * 10, "untextured land stays opaque");
    }

    #[test]
    fn turbulence_surface_uses_class_not_color_or_height() {
        let mut w = world();
        assert!(!w.turbulence_reduced_surface(0., 0.));
        w.theater.cells[0].class = 1;
        assert!(w.turbulence_reduced_surface(0., 0.));
        w.theater.cells[0].color = 255;
        w.theater.cells[0].elevation = 200;
        assert!(w.turbulence_reduced_surface(0., 0.));
        assert!(!w.turbulence_reduced_surface(f64::from(CELL_FEET), 0.));
    }

    #[test]
    fn strip_boxes_become_world_airfield_points() {
        use tore_formats::shape::ContactBox;
        let at = |id: u8, x: i16, z: i16| ContactBox {
            flags: 0xc0,
            id,
            pairs: [[x, x], [0, 32], [z - 10, z + 10]],
        };
        let mut boxes: Vec<_> = (0x19..=0x21)
            .map(|id| at(id, 2688, -1266 + 100 * i16::from(id - 0x19)))
            .collect();
        boxes.extend([
            at(0x11, -1723, -952),
            at(0x12, -1723, -1110),
            at(0x25, 2208, -794),
            at(0x26, 1412, -1326),
            at(0x27, -308, -1326),
            at(0x28, -1723, -1326),
            at(0x29, -1723, 1912),
            at(0x2a, 288, 1912),
            at(0x2b, 288, -1326),
            at(0x2c, 2220, -1326),
        ]);
        // An east-facing field placed at (10000, 500, 20000): local forward is
        // world +X and local right is world -Z.
        let heading = std::f64::consts::FRAC_PI_2;
        let basis = tore_sim::attitude::Basis::new(heading, 0., 0.);
        let place = |local: [f64; 3]| -> [f64; 3] {
            std::array::from_fn(|axis| {
                [10_000., 500., 20_000.][axis]
                    + basis.right[axis] * local[0]
                    + basis.forward[axis] * local[2]
            })
        };
        let anchors = airfield_anchors(&boxes, heading, place).unwrap();
        let near = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6);
        assert!(near(
            anchors.takeoff_spot,
            [10_000. - 952., 500., 20_000. + 1723.]
        ));
        assert!(near(
            anchors.landing_point,
            [10_000. - 1110., 500., 20_000. + 1723.]
        ));
        assert!(near(
            anchors.taxi_out[0],
            [10_000. - 794., 500., 20_000. - 2208.]
        ));
        assert!(near(
            anchors.taxi_in[3],
            [10_000. - 1326., 500., 20_000. - 2220.]
        ));
        assert!(near(
            anchors.parking[8],
            [10_000. - 466., 500., 20_000. - 2688.]
        ));
        assert_eq!(anchors.takeoff_heading, heading);
        assert_eq!(anchors.landing_heading, heading);
        assert_eq!(anchors.parking_heading, std::f64::consts::PI);
        assert_eq!(anchor_points(&anchors).count(), 19);
        // Any missing point means the field has no usable anchors.
        boxes.retain(|b| b.id != 0x21);
        assert!(airfield_anchors(&boxes, heading, place).is_none());
    }

    #[test]
    fn solid_contact_is_separate_from_runway_surface_and_respects_health_ids() {
        let mut world = world();
        world
            .airport_scene
            .objects
            .push(tore_sim::airport::StaticObject {
                id: 100,
                source: tore_sim::airport::SourceKey {
                    layout: "T.MM".into(),
                    ordinal: 0,
                },
                name: "Hangar".into(),
                object_type: "HANGR.OT".into(),
                bounds: tore_sim::airport::OrientedBox {
                    center: [50.0, 10.0, 50.0],
                    half: [10.0; 3],
                    heading: 0.0,
                    pitch: 0.0,
                    bank: 0.0,
                },
                hit_points: 100,
                runway: false,
                category: 0x2000,
                radar_signature: 1.0,
                infrared_signature: 0.0,
            });
        assert_eq!(
            world
                .solid_contact([0.0, 10.0, 50.0], [100.0, 10.0, 50.0], [100])
                .map(|hit| hit.0),
            Some(100)
        );
        assert!(
            world
                .solid_contact([0.0, 10.0, 50.0], [100.0, 10.0, 50.0], [])
                .is_none()
        );
        assert!(!world.surface(50.0, 50.0).landable);
    }

    #[test]
    fn whiteout_cheat_clears_all_views_without_ticks_and_preserves_sun() {
        use tore_formats::weather::shape::{Primitive, WeatherShape};
        let mut w = world();
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
        w.celestial = Some(crate::celestial::Celestial {
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
            w.step_weather(700., &forward);
            w.step_view_weather(&rear, 700.);
        }
        assert!(w.weather_presentation.sun_whitening > 0);
        assert_eq!(w.auxiliary_presentations[0].sun_whitening, 0);
        assert!(!crate::lens_flare::circles(&w, &forward, [1280, 720]).is_empty());
        let sun_geometry = w.celestial.as_ref().unwrap().sun_uniform(&w, 5000.);
        let bright = w.sample_view(5000., 0).palette;
        let ticks = w.weather.ticks();
        w.no_sun_whiteout = true;
        assert!(crate::lens_flare::circles(&w, &forward, [1280, 720]).is_empty());
        assert_ne!(bright, w.sample_view(5000., 0).palette);
        assert_eq!(
            w.sample_view(5000., 0).palette,
            w.sample_view(5000., 1).palette
        );
        assert_eq!(
            sun_geometry,
            w.celestial.as_ref().unwrap().sun_uniform(&w, 5000.)
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
        let mut w = world();
        w.weather = tore_sim::environment::Environment::new(
            tore_sim::environment::Configuration::new(module, 12, 0, 0, None).unwrap(),
        );
        let mut low = Camera::new();
        low.position[1] = 7000.;
        let mut high = Camera::new();
        high.weather_slot = 1;
        high.position[1] = 9000.;
        for _ in 0..120 {
            w.step_weather(700., &low);
            w.step_view_weather(&high, 700.);
        }
        assert_ne!(
            w.weather_presentation.tint,
            w.auxiliary_presentations[0].tint
        );
        let state = (
            w.weather.clone(),
            w.weather_presentation.clone(),
            w.auxiliary_presentations.clone(),
        );
        let low_view = w.sample_view(7000., 0);
        let high_view = w.sample_view(9000., 1);
        assert_ne!(low_view.fog, high_view.fog);
        for _ in 0..10 {
            assert_eq!(high_view.palette, w.sample_view(9000., 1).palette);
            assert_eq!(low_view.palette, w.sample_view(7000., 0).palette);
        }
        assert_eq!(
            state,
            (w.weather, w.weather_presentation, w.auxiliary_presentations)
        );
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
