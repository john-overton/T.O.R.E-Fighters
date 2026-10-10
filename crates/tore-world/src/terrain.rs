//! Simulation half of the mission's world: the T2 grid and its height, surface
//! and water queries, the airport scene and its runway anchors, and the weather
//! clock. It holds no art, palette, render origin or per-frame state (those are
//! the app's scenery) and reads no environment variable. World units
//! are feet, X east, Y up, Z north.
use crate::{WorldResult, resources::ResourceSource};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::theater::{CELL_FEET, Environment, HEIGHT_FEET, Theater};

/// The free-flight start over the Ukraine theaters, before the ground clamp.
pub const UKRAINE_START: [f64; 3] = [1_070_000.0, 28_000.0, 590_000.0];

/// What the simulation queries about the mission's ground and sky. Built from
/// the imported resources alone, so a server can build it without any art.
pub struct Terrain {
    pub theater: Theater,
    /// Exact selected MM identity, distinct from its referenced base grid.
    pub layout: String,
    /// The recovered weather choice the world was built with, if any; `None`
    /// keeps the mission's own `layer` line and time.
    #[allow(dead_code)] // Read by the mission recorder.
    pub condition: Option<usize>,
    pub environment: Environment,
    /// Immutable imported placement/airport geometry. Mutable health belongs to combat.
    pub airport_scene: tore_sim::airport::Scene,
    /// Each runway's taxi, takeoff, landing and parking points from its STRIP
    /// shape, by runway object id. A runway is absent when its shape lacks a
    /// point or a point is off the airport surface.
    pub airfield_anchors: BTreeMap<u32, tore_sim::ai::airfield::AirfieldAnchors>,
    /// Every source placement, including definitions the bounded SH projector cannot draw.
    pub static_manifest: Vec<(u32, tore_formats::mission::SourceKey, String, bool)>,
    /// The mission's own theater: its code and label (empty when the import
    /// has no label for it). It no longer lists every theater of the import.
    pub catalog: Vec<(String, String)>,
    /// Authoritative environment. One instance per world, so every camera,
    /// mirror and panel resolves the same instant.
    pub weather: tore_sim::environment::Environment,
    /// The mission's surface units: the layout's NTs and the ground target
    /// template's objects, with their ids, owners and sides. The airport
    /// scene holds a contact volume for each one whose shape reads.
    pub surface: crate::surface::Surface,
}

/// Launch settings that replace the mission's own weather start time, wind
/// and cloud deck. The app resolves them from `TORE_WEATHER_TIME`,
/// `TORE_WIND` and `TORE_CLOUD_ALTITUDE` (see
/// the app's `scenery::launch_overrides`) and passes them in, so the terrain
/// itself reads no environment variable.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Overrides {
    /// Start time: hour and minute.
    pub time: Option<[i32; 2]>,
    /// Wind heading in degrees and speed in feet per second.
    pub wind: Option<[i32; 2]>,
    /// Scattered cloud deck in feet, 0 for none.
    pub cloud_altitude: Option<i32>,
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

/// Names of the six recovered weather choices, in table order, as the
/// command line and recordings show them.
#[allow(dead_code)] // Read by the mission recorder.
pub const CONDITION_NAMES: [&str; 6] = ["clear", "cloudy", "foggy", "dawn", "sunset", "night"];

/// The launch settings a mission recording keeps for its world, resolved
/// once when it was flown. [`Terrain::for_recorded`] builds from these instead
/// of the environment variables and mission defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Recorded {
    /// Layout code without `.MM`, for example `UKR` or `~UKR1`.
    pub code: String,
    /// The weather choice, when the mission picked one.
    pub condition: Option<usize>,
    /// The resolved `.LAY` resource; `None` resolves it as a launch does.
    pub layer: Option<String>,
    /// Start time: hour and minute.
    pub time: [i32; 2],
    /// Explicit wind in degrees and feet per second, or `None` for the
    /// generated default.
    pub wind: Option<[i32; 2]>,
    /// Scattered cloud deck in feet, 0 for none.
    pub cloud_altitude: i32,
    pub weather_seed: i32,
}

/// The imported layout of one theater with the definitions and shapes its
/// placements name. Both halves of the airport scene build from it: the
/// terrain's objects and runways, and the scenery's static geometry.
pub struct Placements {
    pub layout: tore_formats::mission::Layout,
    pub definitions: BTreeMap<String, tore_formats::static_object::Definition>,
    pub shapes: BTreeMap<String, tore_formats::shape::Shape>,
    shape_scales: BTreeMap<String, f64>,
    /// STRIP anchor 0x11 midpoint by object type, in the shape's frame.
    runway_anchors: BTreeMap<String, [f64; 3]>,
    strip_boxes: BTreeMap<String, Vec<tore_formats::shape::ContactBox>>,
    /// Main shapes the bounded projector could not read, with the reason. Such
    /// a placement stays in the manifest without geometry.
    pub unreadable: Vec<(String, String)>,
    /// The ground target template's placements, with their surface ids
    /// (`0x5000_0000` range), after the layout's.
    pub surface: Vec<(u32, tore_formats::mission::Placement)>,
}

/// Where one placed shape stands in the world.
pub struct Stance {
    pub scale: f64,
    pub min: [f64; 3],
    pub max: [f64; 3],
    pub half: [f64; 3],
    pub heading: f64,
    pub pitch: f64,
    pub bank: f64,
    pub basis: tore_sim::attitude::Basis,
    pub support_origin: [f64; 3],
    /// The shape's origin, after a runway is grounded on its pavement.
    pub origin: [f64; 3],
    pub center: [f64; 3],
    pub runway: bool,
}

/// A runway's length in feet from its shape's extent along the runway (`min_z`
/// to `max_z`, scaled) and the STRIP anchor 0x11 midpoint's position along it:
/// the run from the anchor to the far end when the anchor lies inside the
/// shape, else the whole extent.
pub fn runway_length_ft(min_z: f64, max_z: f64, anchor_z: Option<f64>) -> f64 {
    match anchor_z {
        Some(anchor) if anchor < max_z && anchor >= min_z => max_z - anchor,
        _ => ((max_z - min_z) * 0.5).max(1.0) * 2.0,
    }
}

/// The runway length of an airport object type (a STRIP definition such as
/// `AIRPORT.OT`), read from the imported resources, or `None` when the type is
/// not an airport or its shape cannot be read. The same length the airport
/// scene gives the runway of a placement of this type, without building the
/// theater: the Quick Mission creator uses it to keep short strips off its
/// ground-start list (`tore_sim::airport::SHORT_STRIP_FT`).
pub fn strip_length_ft(resources: &dyn ResourceSource, object_type: &str) -> Option<f64> {
    let definition =
        tore_formats::static_object::Definition::parse(resources.get(object_type)?).ok()?;
    if !definition.callbacks.iter().any(|name| name == "_STRIPProc") {
        return None;
    }
    let shape_bytes = resources.get(definition.main_shape.as_ref()?)?;
    let shape = tore_formats::shape::Shape::scenery(shape_bytes).ok()?;
    let scale = tore_formats::shape::object_scale(shape_bytes).ok()?;
    let (mut min, mut max) = (f64::INFINITY, f64::NEG_INFINITY);
    for point in shape.faces.iter().flat_map(|face| &face.positions) {
        // The shape's third coordinate is the runway's forward axis.
        min = min.min(f64::from(point[1]));
        max = max.max(f64::from(point[1]));
    }
    if !min.is_finite() || !max.is_finite() {
        return None;
    }
    let anchor = tore_formats::shape::contact_boxes(shape_bytes)
        .ok()
        .flatten()
        .and_then(|boxes| boxes.iter().find(|b| b.id == 0x11).map(|b| b.midpoint()[2]))
        .map(f64::from);
    Some(runway_length_ft(min * scale, max * scale, anchor))
}

/// Feet per shape unit of a placed object: the reviewed SH header exponent
/// (`2^(e-8)`), for buildings and surface units alike. Every placed object's
/// drawn size and contact box (and so a surface unit's hit box) comes from
/// this one value. Open question (surface-AI round, 2026-10-10): at this
/// scale surface units draw about three times their real size (a Krivak
/// 1,216 ft long against a real 405); a correction for NT units, once
/// established, belongs here and nowhere else.
pub fn placed_shape_scale(object_type: &str, shape_bytes: &[u8]) -> WorldResult<f64> {
    let _ = object_type;
    Ok(tore_formats::shape::object_scale(shape_bytes)?)
}

impl Placements {
    pub fn load(resources: &dyn ResourceSource, code: &str) -> WorldResult<Self> {
        let layout_name = format!("{code}.MM");
        let layout = tore_formats::mission::Layout::parse(
            &layout_name,
            resources
                .get(&layout_name)
                .ok_or_else(|| format!("missing airport layout {layout_name}"))?,
        )?;
        let mut out = Self {
            layout: tore_formats::mission::Layout {
                resource: String::new(),
                map: None,
                sides: Default::default(),
                placements: Vec::new(),
            },
            definitions: BTreeMap::new(),
            shapes: BTreeMap::new(),
            shape_scales: BTreeMap::new(),
            runway_anchors: BTreeMap::new(),
            strip_boxes: BTreeMap::new(),
            unreadable: Vec::new(),
            surface: Vec::new(),
        };
        for placement in &layout.placements {
            out.add_type(resources, &layout_name, &placement.object_type)?;
        }
        out.layout = layout;
        Ok(out)
    }

    /// The layout's placements and the surface's template placements, as the
    /// terrain built its scene from them: what the scenery draws.
    pub fn for_terrain(
        resources: &dyn ResourceSource,
        terrain: &Terrain,
        code: &str,
    ) -> WorldResult<Self> {
        let mut out = Self::load(resources, code)?;
        out.add_surface(resources, &terrain.surface)?;
        Ok(out)
    }

    /// Adds the surface's template placements and reads their types.
    pub fn add_surface(
        &mut self,
        resources: &dyn ResourceSource,
        surface: &crate::surface::Surface,
    ) -> WorldResult<()> {
        for (id, placement) in &surface.placements {
            self.add_type(resources, &placement.key.layout, &placement.object_type)?;
            self.surface.push((id.0, placement.clone()));
        }
        Ok(())
    }

    /// Every placement with its object id: the layout's (`0x4000_0000` plus
    /// ordinal), then the template's.
    pub fn placed(
        &self,
    ) -> impl Iterator<Item = WorldResult<(u32, &tore_formats::mission::Placement)>> + '_ {
        self.layout
            .placements
            .iter()
            .map(|placement| {
                crate::surface::UnitId::layout(placement.key.ordinal)
                    .map(|id| (id.0, placement))
                    .ok_or_else(|| "airport object ID overflow".into())
            })
            .chain(
                self.surface
                    .iter()
                    .map(|(id, placement)| Ok((*id, placement))),
            )
    }

    /// Reads a placed type's definition and main shape, once.
    fn add_type(
        &mut self,
        resources: &dyn ResourceSource,
        source: &str,
        object_type: &str,
    ) -> WorldResult<()> {
        if self.definitions.contains_key(object_type) {
            return Ok(());
        }
        let definition = tore_formats::static_object::Definition::parse(
            resources.get(object_type).ok_or_else(|| {
                format!("{source}: missing placed definition {object_type}; re-import media")
            })?,
        )?;
        if let Some(main_shape) = &definition.main_shape {
            let shape_bytes = resources.get(main_shape).ok_or_else(|| {
                format!(
                    "{source}: missing shape {main_shape} referred by {object_type}; re-import media"
                )
            })?;
            let parsed = tore_formats::shape::Shape::scenery(shape_bytes);
            match parsed {
                Ok(shape) => {
                    if definition.callbacks.iter().any(|name| name == "_STRIPProc")
                        && let Some(boxes) = tore_formats::shape::contact_boxes(shape_bytes)?
                        && let Some(anchor) = boxes.iter().find(|b| b.id == 0x11)
                    {
                        self.runway_anchors
                            .insert(object_type.to_owned(), anchor.midpoint().map(f64::from));
                        self.strip_boxes
                            .insert(object_type.to_owned(), boxes.clone());
                    }
                    self.shape_scales.insert(
                        object_type.to_owned(),
                        placed_shape_scale(object_type, shape_bytes)?,
                    );
                    self.shapes.insert(object_type.to_owned(), shape);
                }
                Err(error) => self
                    .unreadable
                    .push((main_shape.clone(), error.to_string())),
            }
        }
        self.definitions.insert(object_type.to_owned(), definition);
        Ok(())
    }

    /// Where `placement` stands on ground `ground` feet high, or `None` when it
    /// has no drawable shape or the shape has no finite extent.
    pub fn stance(
        &self,
        placement: &tore_formats::mission::Placement,
        ground: f64,
    ) -> Option<Stance> {
        let definition = self.definitions.get(&placement.object_type)?;
        let shape = self.shapes.get(&placement.object_type)?;
        let heading = f64::from(placement.angles[0]).to_radians();
        let runway = definition.callbacks.iter().any(|name| name == "_STRIPProc");
        // The source runway plane stays at authored ground. The renderer
        // applies a bounded static-surface depth bias without changing contact.
        let support_height = ground;
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        // A viewer-facing sprite (the men) spans its width across either
        // horizontal axis and its height upward from its centre.
        let sprites = shape.billboards.iter().flat_map(|sprite| {
            let [w, h] = sprite.size.map(|v| v * 0.5);
            let c = sprite.center;
            [
                [c[0] - w, c[1] - w, c[2] - h],
                [c[0] + w, c[1] + w, c[2] + h],
            ]
        });
        for point in shape
            .faces
            .iter()
            .flat_map(|face| face.positions.iter().copied())
            .chain(sprites)
        {
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
            return None;
        }
        // The reviewed SH header exponent drives both visual and contact scale.
        let scale = self
            .shape_scales
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
        Some(Stance {
            scale,
            min,
            max,
            half,
            heading,
            pitch,
            bank,
            basis,
            support_origin,
            origin,
            center,
            runway,
        })
    }
}

impl Terrain {
    /// The terrain with the mission's own weather and no launch overrides.
    pub fn for_theater(resources: &dyn ResourceSource, code: &str) -> WorldResult<Self> {
        Self::for_mission(resources, code, None, &Overrides::default())
    }

    /// `condition` selects one of the six recovered weather choices; without it
    /// the mission's own `layer` line and time are used unchanged. `overrides`
    /// replace the start time, wind and cloud deck.
    pub fn for_mission(
        resources: &dyn ResourceSource,
        code: &str,
        condition: Option<usize>,
        overrides: &Overrides,
    ) -> WorldResult<Self> {
        Self::build(resources, code, condition, None, overrides, None)
    }

    /// [`Self::for_mission`] with the Quick Mission ground target, whose
    /// template's units join the surface and the airport scene.
    pub fn for_mission_with(
        resources: &dyn ResourceSource,
        code: &str,
        condition: Option<usize>,
        overrides: &Overrides,
        target: Option<&crate::surface::resolve::GroundTarget>,
    ) -> WorldResult<Self> {
        Self::build(resources, code, condition, None, overrides, target)
    }

    /// Rebuilds the world a mission recording was flown in from its recorded,
    /// resolved launch settings: layout, weather choice and layer, start time,
    /// wind and cloud deck. No override applies, so a replay looks the same
    /// whatever the viewer's settings are.
    #[allow(dead_code)] // Used by the mission replay viewer.
    pub fn for_recorded(resources: &dyn ResourceSource, recorded: &Recorded) -> WorldResult<Self> {
        Self::build(
            resources,
            &recorded.code,
            recorded.condition,
            Some(recorded),
            &Overrides::default(),
            None,
        )
    }

    fn build(
        resources: &dyn ResourceSource,
        code: &str,
        condition: Option<usize>,
        recorded: Option<&Recorded>,
        overrides: &Overrides,
        target: Option<&crate::surface::resolve::GroundTarget>,
    ) -> WorldResult<Self> {
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
        // The label comes from this mission's own theater only, so a bad
        // grid of another theater cannot fail this mission and a manifest
        // holds one grid, not sixteen.
        let id = code.trim_end_matches(".MM");
        let mut catalog = Vec::new();
        if let Some(label) = resources.theater_label(id)? {
            theater.name.clone_from(&label);
            catalog.push((id.to_owned(), label));
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
        // A recording names the layer it resolved, so a later change to the
        // choice table cannot change an old replay's sky.
        let layer = recorded.and_then(|r| r.layer.clone()).unwrap_or(layer);
        let module = tore_formats::weather::Module::parse(required(&layer)?)?;
        let [hour, minute] = match recorded {
            Some(recorded) => recorded.time,
            None => overrides.time.or(launch).unwrap_or([12, 0]),
        };
        let wind = match recorded {
            Some(recorded) => recorded.wind,
            None => overrides.wind.or(environment.wind),
        };
        let mut configuration = tore_sim::environment::Configuration::new(
            module,
            hour,
            minute,
            condition.map_or_else(|| environment.layer_parameter.unwrap_or(0), |i| i as i32),
            wind,
        )?;
        if let Some(recorded) = recorded {
            configuration = configuration.with_weather_seed(recorded.weather_seed)?;
        }
        let weather = tore_sim::environment::Environment::new(configuration);
        if weather.sample(0.).is_none() {
            return Err("mission weather layer covers no altitude at its launch time".into());
        }
        // Preserve the resolved launch identity for validation and restart.
        environment.layer = layer;
        environment.layer_parameter = Some(weather.configuration().parameter());
        environment.time = Some([hour, minute]);
        let cloud_altitude = if let Some(recorded) = recorded {
            recorded.cloud_altitude
        } else if let Some(value) = overrides.cloud_altitude {
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
        let mut out = Self {
            theater,
            layout,
            condition,
            environment,
            airport_scene: tore_sim::airport::Scene::default(),
            airfield_anchors: BTreeMap::new(),
            static_manifest: Vec::new(),
            catalog,
            weather,
            surface: Default::default(),
        };
        out.build_airport_scene(resources, code.trim_end_matches(".MM"), target)?;
        Ok(out)
    }

    /// The airport scene: every placement's contact volume, and each runway's
    /// surface, approach line and airfield anchors. The scenery builds the
    /// matching geometry from the same [`Placements`].
    fn build_airport_scene(
        &mut self,
        resources: &dyn ResourceSource,
        code: &str,
        target: Option<&crate::surface::resolve::GroundTarget>,
    ) -> WorldResult<()> {
        use tore_sim::airport::{
            Airport, Allegiance, OrientedBox, Runway, SourceKey, StaticObject,
        };
        let mut sources = Placements::load(resources, code)?;
        self.surface = Self::resolve_surface(resources, &sources.layout, target)?;
        sources.add_surface(resources, &self.surface)?;
        let mut objects = Vec::new();
        let mut runways = Vec::new();
        let mut airports = Vec::new();
        let mut anchors = BTreeMap::new();
        for placed in sources.placed() {
            let (id, placement) = placed?;
            let definition = &sources.definitions[&placement.object_type];
            self.static_manifest.push((
                id,
                placement.key.clone(),
                placement.object_type.clone(),
                sources.shapes.contains_key(&placement.object_type),
            ));
            if !sources.shapes.contains_key(&placement.object_type) {
                continue;
            }
            let ground =
                f64::from(self.height(placement.position[0] as f32, placement.position[2] as f32));
            let Some(stance) = sources.stance(placement, ground) else {
                continue;
            };
            let Stance {
                min,
                max,
                half,
                heading,
                pitch,
                bank,
                basis,
                support_origin,
                center,
                runway,
                ..
            } = stance;
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
                let anchor = sources.runway_anchors.get(&placement.object_type);
                let length_ft = runway_length_ft(min[2], max[2], anchor.map(|a| a[2]));
                if let Some(anchor) = anchor
                    && anchor[2] < max[2]
                    && anchor[2] >= min[2]
                {
                    let local = [anchor[0], 0.0, (anchor[2] + max[2]) * 0.5];
                    approach_center = std::array::from_fn(|axis| {
                        support_origin[axis]
                            + basis.right[axis] * local[0]
                            + basis.forward[axis] * local[2]
                    });
                }
                if let Some(found) =
                    sources
                        .strip_boxes
                        .get(&placement.object_type)
                        .and_then(|boxes| {
                            airfield_anchors(boxes, heading, |local| {
                                std::array::from_fn(|axis| {
                                    support_origin[axis]
                                        + basis.right[axis] * local[0]
                                        + basis.forward[axis] * local[2]
                                })
                            })
                        })
                {
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
        }
        // A unit is in the scene, and so a combat target, when its shape gave
        // it a contact volume.
        let placed: BTreeSet<u32> = objects.iter().map(|o: &StaticObject| o.id).collect();
        for unit in &mut self.surface.units {
            unit.in_scene = placed.contains(&unit.id.0);
        }
        // A unit that follows a route takes its hit box from the scene's.
        let mut courses = std::mem::take(&mut self.surface.courses);
        for (id, course) in &mut courses {
            if let Some(object) = objects.iter().find(|object| object.id == id.0) {
                let ground = f64::from(self.height(course.start[0] as f32, course.start[2] as f32));
                course.fit(&object.bounds, ground);
            }
        }
        self.surface.courses = courses;
        // The parked aircraft stand on the ground or their carrier's deck.
        let (parked, unreadable) =
            crate::surface::parked::place(resources, &self.surface, &|x, z| {
                f64::from(self.height(x as f32, z as f32))
            });
        self.surface.parked_scene = parked;
        self.surface.unreadable.extend(unreadable);
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

    /// The surface a layout and an optional ground target resolve to.
    fn resolve_surface(
        resources: &dyn ResourceSource,
        layout: &tore_formats::mission::Layout,
        target: Option<&crate::surface::resolve::GroundTarget>,
    ) -> WorldResult<crate::surface::Surface> {
        use crate::surface::{catalog::Catalog, resolve};
        let mut catalog = Catalog::new(resources);
        let base = resolve::layout(layout, &mut catalog);
        let mut unresolved = None;
        let template = match target {
            Some(target) => {
                let name = format!("~{}.M", target.stem.to_ascii_uppercase());
                match resources.get(&name) {
                    Some(bytes) => {
                        let template = tore_formats::quick_template::Template::parse(&name, bytes)?;
                        Some(resolve::template(
                            &template,
                            target,
                            &mut catalog,
                            layout.map.as_deref(),
                        )?)
                    }
                    // An import made before it kept the templates: the
                    // mission flies, and says why nothing stands there.
                    None => {
                        unresolved = Some(format!(
                            "the import has no ground target template {name}; re-import media"
                        ));
                        None
                    }
                }
            }
            None => None,
        };
        let mut surface = resolve::surface(base, template)?;
        surface.unresolved = unresolved;
        // The units that follow a route (Quick Mission columns and ships).
        for unit in surface.units.iter().filter(|unit| unit.route.is_some()) {
            if let Ok(entry) = catalog.entry(&unit.resource)
                && let Some(record) = &entry.unit
                && let Some(course) = crate::surface::movement::Course::of(unit, record)
            {
                surface.courses.insert(unit.id, course);
            }
        }
        Ok(surface)
    }

    /// The AI's view of one runway, with its airfield points when known.
    pub fn runway_view(&self, object: u32) -> Option<tore_sim::ai::airfield::RunwayView> {
        self.airport_scene.runway(object).map(|runway| {
            tore_sim::ai::airfield::RunwayView::from(runway)
                .with_anchors(self.airfield_anchors.get(&object).copied())
        })
    }

    /// How far a point is beyond the edge of the theater's map rectangle, in
    /// nautical miles, measured from the nearest point of the rectangle; zero
    /// inside it. Terrain runs from 0 to `(cells - 1) * cell` on each axis.
    pub fn edge_distance_nm(&self, x: f64, z: f64) -> f64 {
        edge_distance_nm(self.theater.cols, self.theater.rows, x, z)
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
        state: &tore_sim::flight::State,
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

    /// Where a free flight and the free camera start: over the Ukraine
    /// default in the Ukraine theaters and over the middle of the grid in
    /// the others, at 28,000 feet or 3,000 feet above the ground there,
    /// whichever is higher.
    pub fn free_flight_start(&self) -> [f64; 3] {
        let mut position = UKRAINE_START;
        if tore_formats::theater::base_theater(&self.layout) != Some("UKR") {
            position = [
                f64::from((self.theater.cols as f32 - 1.0) * CELL_FEET * 0.5),
                28000.0,
                f64::from((self.theater.rows as f32 - 1.0) * CELL_FEET * 0.5),
            ];
        }
        let ground = self.height(position[0] as f32, position[2] as f32);
        position[1] = position[1].max(f64::from(ground + 3000.0));
        position
    }

    /// The highest [`Self::height`] or [`Self::surface`] answers over each
    /// square of the terrain grid: the square's highest corner cell (heights
    /// blend them) or the highest runway surface over it.
    pub fn ground_ceiling(&self) -> tore_sim::ground_ceiling::GroundCeiling {
        let theater = &self.theater;
        let mut ceiling = tore_sim::ground_ceiling::GroundCeiling::from_grid(
            theater.cols,
            theater.rows,
            f64::from(CELL_FEET),
            |col, row| f64::from(f32::from(theater.cell(col, row).elevation) * HEIGHT_FEET),
        );
        for runway in &self.airport_scene.runways {
            if let Some((lo, hi, top)) = runway.surface_extent() {
                ceiling.raise(lo, hi, top);
            }
        }
        ceiling
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
/// Distance beyond the edge of a `cols` by `rows` terrain grid, in nautical
/// miles, from the nearest point of the map rectangle (zero inside it).
pub fn edge_distance_nm(cols: usize, rows: usize, x: f64, z: f64) -> f64 {
    let cell = f64::from(tore_formats::theater::CELL_FEET);
    let extent = |cells: usize| cells.saturating_sub(1) as f64 * cell;
    let beyond = |value: f64, extent: f64| (0. - value).max(value - extent).max(0.);
    beyond(x, extent(cols)).hypot(beyond(z, extent(rows))) / FEET_PER_NAUTICAL_MILE
}

/// Feet in a nautical mile.
pub const FEET_PER_NAUTICAL_MILE: f64 = 6_076.115_49;
/// The player is warned to turn back this far beyond the theater's edge, and any
/// aircraft is lost 5 nautical miles further out. `opinionated` (requested by
/// John, 2026-09-29); the distances and the unit are agent decisions.
pub const EDGE_WARNING_NM: f64 = 100.;
pub const EDGE_DESTROY_NM: f64 = 105.;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_runway_is_measured_from_the_anchor_to_the_far_end() {
        // The anchor inside the shape: the run from it to the far end.
        assert_eq!(runway_length_ft(-600., 600., Some(-537.)), 1_137.);
        // No anchor, or one outside the shape: the whole extent.
        assert_eq!(runway_length_ft(-600., 600., None), 1_200.);
        assert_eq!(runway_length_ft(-600., 600., Some(700.)), 1_200.);
        assert_eq!(runway_length_ft(-600., 600., Some(-700.)), 1_200.);
        // The far end itself is outside, the near end inside.
        assert_eq!(runway_length_ft(-600., 600., Some(600.)), 1_200.);
        assert_eq!(runway_length_ft(-600., 600., Some(-600.)), 1_200.);
        // The smallest extent the scene gives a runway is two feet.
        assert_eq!(runway_length_ft(0., 0.5, None), 2.);
    }
    #[test]
    fn strip_length_needs_an_airport_definition_in_the_resources() {
        assert_eq!(strip_length_ft(&BTreeMap::new(), "AIRPORT.OT"), None);
        let junk = BTreeMap::from([("AIRPORT.OT".to_string(), vec![0u8; 8])]);
        assert_eq!(strip_length_ft(&junk, "AIRPORT.OT"), None);
    }
    #[test]
    fn the_map_edge_distance_is_measured_from_the_nearest_point_of_the_rectangle() {
        // 209 by 201 cells of 8,192 ft: the map runs 0..1,703,936 by 0..1,638,400.
        let d = |x: f64, z: f64| edge_distance_nm(209, 201, x, z);
        assert_eq!(d(0., 0.), 0.);
        assert_eq!(d(1_703_936., 1_638_400.), 0.);
        assert_eq!(d(800_000., 800_000.), 0.);
        let nm = FEET_PER_NAUTICAL_MILE;
        assert!((d(-100. * nm, 800_000.) - 100.).abs() < 1e-9);
        assert!((d(1_703_936. + 105. * nm, 0.) - 105.).abs() < 1e-9);
        assert!((d(800_000., 1_638_400. + 3. * nm) - 3.).abs() < 1e-9);
        // Past a corner the nearest point is the corner: 3-4-5 miles.
        assert!((d(-3. * nm, -4. * nm) - 5.).abs() < 1e-9);
    }
    use crate::test_support::terrain as world;
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
            billboards: Vec::new(),
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
                billboards: Vec::new(),
                lines: vec![],
                faces: vec![],
                state_words: Default::default()
            }),
            0.
        );
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
    fn height_matches_triangle_corners_and_center() {
        let w = world();
        assert_eq!(w.height(0.0, 0.0), 0.0);
        assert!((w.height(CELL_FEET / 2.0, CELL_FEET / 2.0) - 1536.0).abs() < 0.01);
    }

    /// Wherever the ceiling calls a sight line clear, every point of it is
    /// above the ground the AI is given, on and off the grid and over a
    /// sloped runway.
    #[test]
    fn the_ground_ceiling_never_clears_a_line_through_the_ground() {
        use tore_formats::theater::TerrainCell;
        use tore_sim::airport::{OrientedBox, Runway};
        let mut seed = 0x51_7cc1_b727_220a_u64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut w = world();
        let (cols, rows) = (12, 9);
        w.theater.cols = cols;
        w.theater.rows = rows;
        w.theater.cells = (0..cols * rows)
            .map(|_| TerrainCell {
                color: 100,
                class: 2,
                elevation: (next() % 32) as u8,
            })
            .collect();
        let surface = OrientedBox {
            center: [
                3.3 * f64::from(CELL_FEET),
                9_000.,
                4.1 * f64::from(CELL_FEET),
            ],
            half: [150., 20., 6_000.],
            heading: 0.6,
            pitch: 0.03,
            bank: 0.01,
        };
        w.airport_scene.runways.push(Runway {
            object: 1,
            airport: 1,
            name: "R".into(),
            surface,
            approach_center: surface.center,
            elevation_ft: 9_000.,
            heading: 0.6,
            length_ft: 12_000.,
        });
        let ceiling = w.ground_ceiling();
        // Only the runway, standing above the highest hill (31 steps of 256
        // feet), keeps this line from being clear.
        let over = |height: f64| {
            let c = surface.center;
            ceiling.clear_above([c[0] - 100., height, c[2]], [c[0] + 100., height, c[2]])
        };
        assert!(!over(8_500.));
        assert!(over(9_500.));
        let span = f64::from(CELL_FEET) * 14.;
        let mut cleared = 0;
        for _ in 0..20_000 {
            let mut point = || {
                [
                    (next() % 1_000_000) as f64 / 1e6 * span - f64::from(CELL_FEET),
                    (next() % 12_000) as f64,
                    (next() % 1_000_000) as f64 / 1e6 * span - f64::from(CELL_FEET),
                ]
            };
            let a = point();
            let mut b = point();
            // Mostly short lines, as a pilot sees a tracer nearby.
            if next() % 4 != 0 {
                b = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * 0.05);
            }
            if !ceiling.clear_above(a, b) {
                continue;
            }
            cleared += 1;
            for step in 0..=64 {
                let t = f64::from(step) / 64.;
                let p: [f64; 3] = std::array::from_fn(|i| a[i] + (b[i] - a[i]) * t);
                let terrain = f64::from(w.height(p[0] as f32, p[2] as f32));
                let ground = w.surface(p[0], p[2]).height;
                assert!(p[1] > terrain && p[1] > ground, "{a:?} {b:?} at {t}");
            }
        }
        assert!(
            cleared > 1_000,
            "too few lines cleared to mean anything: {cleared}"
        );
    }
}
