//! Imported aircraft geometry, cockpit and data. Does not use reference runtime code.
use crate::{
    AppResult, flight,
    menu::Sprite,
    terrain::{Camera, World},
};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{Pic, aircraft::Aircraft, font::Font, shape::Shape};
pub struct Airframe {
    pub engine_material: Option<crate::engine_material::Image>,
    nozzle_bounds: [[f32; 4]; 2],
    rig: Option<crate::additional_animation::Rig>,
    /// Wing vapor attachment, from the shape's own streamer definition.
    pub streamer: Option<tore_formats::shape::StreamerDef>,
    model: tore_sim::models::AircraftModel,
    pub profile: Aircraft,
    pub atlas: Pic,
    damage_art: crate::damage_art::DamageArt,
    pub palette: [[u8; 3]; 256],
    pub cockpit_pic: Pic,
    pub sprites: BTreeMap<String, Sprite>,
    pub font: Font,
    pub hud_font: Font,
    pub hud: tore_formats::hud::Hud,
    pub flight_menu: Vec<tore_formats::ui::MenuNode>,
    /// Imported sensor capability for this identity, resolved by record
    /// channel. Used for capability reporting and scope labels.
    pub sensors: tore_sim::sensors::SensorProfiles,
    pub poses: Vec<Shape>,
}
impl Airframe {
    pub fn load(
        data: &BTreeMap<String, Vec<u8>>,
        id: tore_formats::aircraft::AircraftId,
    ) -> AppResult<Self> {
        let get = |s: &str| {
            data.get(s)
                .ok_or_else(|| format!("aircraft cache missing {s}; re-import media"))
        };
        let mut profile = Aircraft::parse(get(id.pt())?)?;
        if profile.id != id.source() || profile.shape != format!("{}.SH", id.stem()) {
            return Err(
                "aircraft identity/shape does not match the selected retail profile".into(),
            );
        }
        profile.id = id;
        let shape = Shape::parse(get(&profile.shape)?)?;
        let streamer = tore_formats::shape::StreamerDef::parse(get(&profile.shape)?)?;
        if id == tore_formats::aircraft::AircraftId::F18
            && (![0x7900, 0x790c, 0x7912, 0x791e]
                .iter()
                .all(|word| shape.state_words.contains(word))
                || tore_formats::module::code(get(&profile.shape)?)?.0.len() != 26934)
        {
            return Err("unreviewed F18.SH device layout; preserve raw import and review its rig before flying".into());
        }
        let mut atlas = Pic::parse(get(&format!("_{}.PIC", id.stem()))?)?;
        if !atlas.palette.is_empty() {
            return Err("unreviewed aircraft atlas palette override".into());
        }
        let raw = get("PALETTE.PAL")?;
        if raw.len() != 768 || raw.iter().any(|v| *v > 63) {
            return Err("invalid aircraft palette".into());
        }
        let palette = std::array::from_fn(|i| {
            std::array::from_fn(|j| ((raw[i * 3 + j] as u16 * 255 + 31) / 63) as u8)
        });
        let frame = Pic::parse(get(id.cockpit())?)?;
        if frame.palette.len() != 64 {
            return Err("unreviewed cockpit palette prefix".into());
        }
        let mut cockpit_palette = palette;
        cockpit_palette[..frame.palette.len()].copy_from_slice(&frame.palette);
        let mut sprites = BTreeMap::new();
        let cockpit_art = [
            id.cockpit().to_string(),
            format!("~{}_LH.PIC", id.cockpit_stem()),
            format!("~{}_CH.PIC", id.cockpit_stem()),
            format!("~{}_RH.PIC", id.cockpit_stem()),
        ];
        for name in tore_formats::aircraft::INSTRUMENT_ART
            .iter()
            .copied()
            .chain(cockpit_art.iter().map(String::as_str))
        {
            {
                use tore_formats::aircraft::AircraftId;
                let absent_overlay = matches!(
                    id,
                    AircraftId::X31 | AircraftId::Mig21 | AircraftId::F22 | AircraftId::Faxx
                ) && cockpit_art[1..].iter().any(|n| n == name)
                    || matches!(id, AircraftId::Mig29 | AircraftId::Mig23 | AircraftId::Su25)
                        && name == cockpit_art[2];
                if absent_overlay {
                    continue;
                }
                let p = Pic::parse(get(name)?)?;
                sprites.insert(
                    name.into(),
                    Sprite {
                        width: p.width,
                        height: p.height,
                        rgba: p.rgba(&cockpit_palette),
                        glyphs: p.glyphs,
                    },
                );
            }
        }
        let font = Font::parse(get("WIN11.FNT")?)?;
        let sensors = tore_sim::sensors::SensorProfiles::from_source(&profile, |name| {
            get(name).cloned().map_err(std::io::Error::other)
        })?;
        let mut poses = Vec::new();
        let mut rig = None;
        if id == tore_formats::aircraft::AircraftId::F18 {
            for mask in 0..16 {
                let words = [
                    (0x7900, i32::from(mask & 1 != 0)),
                    (0x790c, i32::from(mask & 2 != 0)),
                    (0x7912, i32::from(mask & 4 != 0)),
                    (0x791e, i32::from(mask & 8 != 0)),
                ]
                .into();
                poses.push(Shape::with_state(get(&profile.shape)?, &words)?);
            }

            use crate::aircraft_animation::{Part, part};
            for (group, expected) in [
                (Part::Flame, 8),
                (Part::Nozzle, 4),
                (Part::Brake, 2),
                (Part::Hook, 2),
                (Part::GearLeft, 6),
                (Part::GearRight, 6),
                (Part::GearNose, 6),
                (Part::DoorLeft, 2),
                (Part::DoorRight, 2),
                (Part::DoorNose, 2),
                (Part::FlapLeft, 2),
                (Part::FlapRight, 2),
                (Part::TailLeft, 4),
                (Part::TailRight, 4),
            ] {
                if poses[15]
                    .faces
                    .iter()
                    .filter(|f| part(f.address) == group)
                    .count()
                    != expected
                {
                    return Err(format!(
                    "unreviewed F18 animation group {group:?}; inspect source before applying rig"
                )
                .into());
                }
            }
        } else if id == tore_formats::aircraft::AircraftId::Rafale {
            if tore_formats::module::code(get(&profile.shape)?)?.0.len() != 19334
                || shape.state_words != [0x5b50, 0x5b56, 0x5b62, 0x5b6e, 0x5b74, 0x5b7a].into()
                || shape
                    .faces
                    .iter()
                    .any(|f| !f.texture.is_empty() && f.texture != "_RAF.PIC")
            {
                return Err("unreviewed RAF.SH layout or texture references".into());
            }
            // Keep neutral flap/rudder geometry: their nonzero branches contain
            // native arithmetic outside the bounded reader's reviewed grammar.
            for gear in [0, 1] {
                poses.push(Shape::with_state(
                    get(&profile.shape)?,
                    &[(0x5b50, 1), (0x5b56, 1), (0x5b62, gear)].into(),
                )?);
            }
            crate::rafale_animation::validate(&poses)?;
        } else {
            let (new_rig, pose) = crate::additional_animation::Rig::load(id, get(&profile.shape)?)?;
            poses.push(pose);
            rig = Some(new_rig);
        }
        println!(
            "Aircraft: {}, {} exterior faces, {} G rows, {} hardpoints; atlas {}x{}, instrument font {}px",
            profile.id.label(),
            shape.faces.len(),
            profile.envelopes.len(),
            profile.hardpoints.len(),
            atlas.width,
            atlas.height,
            font.height
        );
        let flight_menu = tore_formats::ui::flight_menu(get("FMENUD.MNU")?)?;
        if flight_menu
            .iter()
            .map(|n| n.label.as_str())
            .collect::<Vec<_>>()
            != [
                "?", "Control", "Pref", "View", "Window", "Cheat", "Multi", "Map", "Pos",
            ]
            || flight_menu.iter().any(|n| n.children.is_empty())
        {
            return Err("unreviewed FA in-flight menu structure".into());
        }
        let engine_material = if crate::engine_material::outlet_count(id) > 0 {
            crate::engine_material::Image::load()?
        } else {
            None
        };
        let mut nozzle_bounds = [[
            f32::INFINITY,
            f32::NEG_INFINITY,
            f32::INFINITY,
            f32::NEG_INFINITY,
        ]; 2];
        for face in &poses[0].faces {
            if crate::engine_material::nozzle(id, face.address) {
                let group = crate::engine_material::outlet_group(id, &face.positions);
                for p in &face.positions {
                    let b = &mut nozzle_bounds[group];
                    b[0] = b[0].min(p[0]);
                    b[1] = b[1].max(p[0]);
                    b[2] = b[2].min(p[2]);
                    b[3] = b[3].max(p[2]);
                }
            }
        }
        if engine_material.is_some() {
            let count = crate::engine_material::outlet_count(id);
            if nozzle_bounds[..count]
                .iter()
                .any(|b| !b.iter().all(|v| v.is_finite()) || b[1] <= b[0] || b[3] <= b[2])
            {
                return Err("unreviewed engine face projection".into());
            }
        }
        let damage_art = crate::damage_art::DamageArt::load(id, data, &mut atlas)?;
        Ok(Self {
            engine_material,
            nozzle_bounds,
            rig,
            model: tore_sim::models::AircraftModel::for_aircraft(&profile)?,
            profile,
            atlas,
            damage_art,
            palette,
            cockpit_pic: frame,
            sprites,
            font,
            hud_font: Font::parse(get("HUD11.FNT")?)?,
            hud: tore_formats::hud::Hud::parse(get(id.hud())?)?,
            flight_menu,
            sensors,
            poses,
            streamer,
        })
    }
    /// One palette for cockpit art and HUD, retaining the original private prefix.
    pub fn cockpit_palette(&self, world: &World, altitude: f64, brightness: i16) -> [[u8; 3]; 256] {
        let mut colors = world.palette;
        let mut source = [[0; 3]; 256];
        for (out, color) in source.iter_mut().zip(&self.cockpit_pic.palette) {
            *out = color.map(|c| ((u16::from(c) * 63 + 127) / 255) as u8);
        }
        tore_formats::weather::palette::apply_hud_brightness(&mut source, brightness)
            .expect("validated HUD brightness");
        if world.smooth_weather
            && let Some(sample) = world.weather.visual_sample(altitude)
        {
            return sample.palette_with_prefix(
                Some(source[..64].try_into().expect("fixed cockpit prefix")),
                world.weather_presentation.visual_tint,
                if world.glare_enabled() {
                    world.weather_presentation.visual_sun
                } else {
                    0.
                },
            );
        }
        tore_formats::weather::palette::apply_sun_whitening(
            &mut source,
            if world.glare_enabled() {
                world.weather_presentation.sun_whitening
            } else {
                0
            },
        )
        .expect("validated cockpit palette");
        if let Some(layer) = world.weather.sample(altitude) {
            tore_formats::weather::palette::apply_tint(
                &mut source,
                layer.tint,
                world.weather_presentation.tint,
            )
            .expect("validated cockpit palette");
        }
        for i in 0..64 {
            colors[i] = source[i].map(|c| ((u16::from(c) * 255 + 31) / 63) as u8);
        }
        colors
    }
    /// The two wingtip vapor attachments in world feet for one pose. Shape
    /// CE vectors are right/up/forward, unlike the mesh's right/forward/up.
    /// Both retain the host's one-third-foot model scale.
    pub fn streamer_points(&self, s: &flight::State) -> Option<[[f64; 3]; 2]> {
        let def = self.streamer.as_ref()?;
        if self.profile.id == tore_formats::aircraft::AircraftId::F14 {
            // FA CE points do not match the quantized base F14 mesh. This
            // fitted attachment uses its reviewed wing tips and rig pivots.
            let basis = tore_sim::attitude::Basis::new(s.yaw, s.pitch, s.bank);
            return Some(std::array::from_fn(|side| {
                let (tip, pivot, sign) = if side == 0 {
                    ([-23., -4., 1.], [-4., -1., 1.], -1.)
                } else {
                    ([24., -4., 1.], [5., -1., 1.], 1.)
                };
                let offset = crate::aircraft_animation::rotate(
                    std::array::from_fn(|i| tip[i] - pivot[i]),
                    [0., 0., 1.],
                    -sign * crate::additional_animation::sweep(s),
                );
                let p: [f64; 3] =
                    std::array::from_fn(|i| f64::from(pivot[i] + offset[i]) * 4. / 3.);
                std::array::from_fn(|i| {
                    s.position[i]
                        + basis.right[i] * p[0]
                        + basis.up[i] * p[2]
                        + basis.forward[i] * p[1]
                })
            }));
        }
        if self.profile.id == tore_formats::aircraft::AircraftId::Mig23 {
            let demand =
                (crate::roster_animation::sweep(s) / 40f64.to_radians() * 32767.).round() as i16;
            let points = [
                def.attachment(0, demand).ok()?,
                def.attachment(1, demand).ok()?,
            ];
            let basis = tore_sim::attitude::Basis::new(s.yaw, s.pitch, s.bank);
            return Some(points.map(|p| {
                std::array::from_fn(|i| {
                    s.position[i]
                        + (basis.right[i] * p[0] + basis.up[i] * p[1] + basis.forward[i] * p[2])
                            / 3.
                })
            }));
        }
        streamer_world_points(def, s.position, [s.yaw, s.pitch, s.bank])
    }

    pub fn start(&self, world: &World) -> flight::State {
        let c = Camera::for_world(world);
        let mut p = c.position.map(|v| v as f64);
        p[1] = 5000f64.max(world.height(p[0] as f32, p[2] as f32) as f64 + 2000.);
        let mut state = flight::State::from_model(self.model.clone(), p);
        // State velocity is ground-relative; initialize the requested airspeed
        // with advection already present so the first tick does not subtract it twice.
        for (v, wind) in state.velocity.iter_mut().zip(world.wind()) {
            *v += wind;
        }
        state
    }
    /// Shared fitted instrument camera pose for fixed-tick weather and rendering.
    pub fn panel_camera(&self, state: &flight::State, page: u8) -> Camera {
        let mut camera = self.camera(state, if page == 2 { 0 } else { 2 }, Default::default());
        camera.weather_slot = usize::from(page);
        camera.view_fraction = 1.;
        if page == 3 {
            for i in 0..3 {
                camera.position[i] = state.position[i] as f32
                    + (camera.position[i] - state.position[i] as f32) * 0.5;
            }
            camera.pitch = -(30f32 / 65.).atan();
        }
        camera
    }

    pub fn camera(&self, state: &flight::State, view: u8, keys: BTreeSet<String>) -> Camera {
        let mut c = Camera::new();
        c.keys = keys;
        c.position = state.position.map(|v| v as f32);
        c.yaw = state.yaw as f32;
        c.pitch = state.pitch as f32;
        c.roll = -state.bank as f32;
        c.view_fraction = 1.;
        if view == 3 {
            c.yaw += std::f32::consts::PI;
        } else if view == 4 {
            c.pitch += 0.8;
        } else if view != 0 {
            c.view_fraction = 1.;
            let angle = state.yaw + if view == 2 { 0.8 } else { 0. };
            let dist = if view == 2 { 130. } else { 180. };
            c.position[0] -= (angle.sin() * dist) as f32;
            c.position[2] -= (angle.cos() * dist) as f32;
            c.position[1] += 60.;
            c.yaw = angle as f32;
            c.pitch = -0.3;
            c.roll = 0.;
        }
        c
    }
    /// Fitted attachment behind reviewed nozzle bounds, with a bounds-based
    /// fallback for aircraft whose outlet faces have not been reviewed.
    pub fn contrail_offsets(&self) -> Vec<[f64; 3]> {
        use tore_formats::aircraft::AircraftId;
        let count = match self.profile.id {
            AircraftId::A4E | AircraftId::X31 | AircraftId::Mig21 | AircraftId::Mig23 => 1,
            _ => 2,
        };
        let scale = f64::from(self.rig.as_ref().map_or(1. / 3., |r| r.scale()));
        (0..count)
            .map(|group| {
                let positions: Vec<_> = self.poses[0]
                    .faces
                    .iter()
                    .filter(|f| {
                        crate::engine_material::nozzle(self.profile.id, f.address)
                            && crate::engine_material::outlet_group(self.profile.id, &f.positions)
                                == group
                    })
                    .flat_map(|f| f.positions.iter())
                    .collect();
                if !positions.is_empty() {
                    let min: [f64; 3] = std::array::from_fn(|i| {
                        positions
                            .iter()
                            .map(|p| f64::from(p[i]))
                            .fold(f64::INFINITY, f64::min)
                    });
                    let max: [f64; 3] = std::array::from_fn(|i| {
                        positions
                            .iter()
                            .map(|p| f64::from(p[i]))
                            .fold(f64::NEG_INFINITY, f64::max)
                    });
                    [
                        (min[0] + max[0]) * 0.5 * scale,
                        (min[2] + max[2]) * 0.5 * scale,
                        min[1] * scale - 2.,
                    ]
                } else {
                    let positions = self.poses[0].faces.iter().flat_map(|f| &f.positions);
                    let mut aft = 0_f64;
                    let mut span = 0_f64;
                    for p in positions {
                        aft = aft.min(f64::from(p[1]) * scale);
                        span = span.max(f64::from(p[0]).abs() * scale);
                    }
                    let lateral = if count == 1 {
                        0.
                    } else {
                        span * 0.15 * if group == 0 { -1. } else { 1. }
                    };
                    [lateral, 0., aft - 2.]
                }
            })
            .collect()
    }

    pub fn vertices(&self, s: &flight::State, camera: &Camera, world: &World) -> Vec<f32> {
        self.visual_vertices(s, camera, world, false)
    }
    pub fn fragment_vertices(&self, s: &flight::State, camera: &Camera, world: &World) -> Vec<f32> {
        self.visual_vertices(s, camera, world, true)
    }
    fn visual_vertices(
        &self,
        s: &flight::State,
        camera: &Camera,
        world: &World,
        fragment: bool,
    ) -> Vec<f32> {
        let mut result = Vec::new();
        let (sy, cy) = (s.yaw as f32).sin_cos();
        let (sp, cp) = (s.pitch as f32).sin_cos();
        let (sb, cb) = (s.bank as f32).sin_cos();
        let orient = |p: [f32; 3]| {
            let (x, y, z) = (p[0], p[1], p[2]);
            let (x, y) = (x * cb + y * sb, -x * sb + y * cb);
            let (y, z) = (y * cp + z * sp, -y * sp + z * cp);
            [x * cy + z * sy, y, -x * sy + z * cy]
        };
        let lighting = world
            .weather
            .sample(camera.position[1] as f64)
            .map(|layer| {
                let direction = crate::celestial::rotate(
                    [0., 0., 1.],
                    tore_sim::environment::light_angles(&layer, world.weather.seconds_of_day()),
                );
                direction.map(|v| (v * 32767.).round().clamp(-32767., 32767.) as i16)
            });
        let model_scale = self.rig.as_ref().map_or(1. / 3., |r| r.scale());
        let hornet_rig = self.profile.id == tore_formats::aircraft::AircraftId::F18;
        let damaged = crate::damage_art::DamageArt::variant(self.profile.id, s.damage_variant);
        if fragment && damaged.is_none() {
            return result;
        }
        let shape = if let Some(index) = damaged {
            if fragment {
                &self.damage_art.fragments[index]
            } else {
                &self.damage_art.bodies[index]
            }
        } else {
            &self.poses[if hornet_rig {
                15
            } else if self.rig.is_some() {
                0
            } else {
                usize::from(s.gear > 0.)
            }]
        };
        for source in shape.faces.iter().flat_map(|f| {
            if damaged.is_some() {
                vec![f.clone()]
            } else if hornet_rig {
                crate::aircraft_animation::rudder_faces(f, s)
            } else if let Some(rig) = &self.rig {
                rig.faces(f, s)
            } else {
                vec![f.clone()]
            }
        }) {
            let Some(f) = (if damaged.is_some() {
                Some(source.clone())
            } else if hornet_rig {
                crate::aircraft_animation::animate(&source, s)
            } else if let Some(rig) = &self.rig {
                rig.animate(&source, s)
            } else {
                crate::rafale_animation::animate(&source, s)
            }) else {
                continue;
            };
            let surfaces = if fragment {
                vec![f]
            } else {
                self.damage_art.surfaces(&f, &s.damage_regions, model_scale)
            };
            for f in surfaces {
                // Smooth mode submits complete geometry for camera-independent shadows.
                if !world.smooth_weather
                    && let Some(n) = f.normal
                {
                    let normal = orient(n);
                    let p = f.positions[0];
                    let p = orient([p[0] * model_scale, p[2] * model_scale, p[1] * model_scale]);
                    let dot: f32 = (0..3)
                        .map(|i| normal[i] * (camera.position[i] - s.position[i] as f32 - p[i]))
                        .sum();
                    if dot <= 0. {
                        continue;
                    }
                }

                // Stepped compatibility uses the imported per-normal light remapping.
                // Smooth surfaces receive continuous GPU lighting instead.
                // Animated world normals and light angles use the host float rig;
                // the following Q15 dot, row selection and remap order are translated.
                let light_row = if !world.smooth_weather && f.subtype & 0x20 != 0 {
                    f.normal
                        .zip(lighting)
                        .zip(world.celestial.as_ref())
                        .map(|((normal, light), celestial)| {
                            let normal =
                                orient(normal).map(|v| v.round().clamp(-32767., 32767.) as i16);
                            let amount = tore_formats::weather::lighting::amount(normal, light);
                            let (bank, row) = world.weather.configuration().lighting().row(amount);
                            (celestial.light_rows[bank] + row + 1) as f32
                        })
                        .unwrap_or(0.)
                } else {
                    0.
                };
                let flame = damaged.is_none()
                    && if hornet_rig {
                        crate::aircraft_animation::part(f.address)
                            == crate::aircraft_animation::Part::Flame
                    } else if let Some(rig) = &self.rig {
                        rig.flame(f.address)
                    } else {
                        crate::rafale_animation::part(f.address)
                            == crate::rafale_animation::Part::Flame
                    };
                let engine_face = damaged.is_none()
                    && self.engine_material.is_some()
                    && crate::engine_material::nozzle(self.profile.id, f.address);
                let engine_group =
                    crate::engine_material::outlet_group(self.profile.id, &f.positions);
                let canopy = damaged.is_none()
                    && self.profile.id.source() == tore_formats::aircraft::AircraftId::F22
                    && crate::roster_animation::canopy(f.address);
                for i in 1..f.positions.len() - 1 {
                    for j in [0, i, i + 1] {
                        let p = f.positions[j];
                        let scale = model_scale;
                        let (x, y, z) = (p[0] * scale, p[2] * scale, p[1] * scale);
                        let (x, y) = (x * cb + y * sb, -x * sb + y * cb);
                        let (y, z) = (y * cp + z * sp, -y * sp + z * cp);
                        let pos = [x * cy + z * sy, y, -x * sy + z * cy];
                        let uv = if engine_face {
                            let b = self.nozzle_bounds[engine_group];
                            [(p[0] - b[0]) / (b[1] - b[0]), (b[3] - p[2]) / (b[3] - b[2])]
                        } else if f.uv.is_empty() {
                            [0.; 2]
                        } else {
                            let region = self
                                .damage_art
                                .regions
                                .get(&f.texture)
                                .expect("reviewed aircraft texture");
                            [
                                (f.uv[j][0] + 0.5) / self.atlas.width as f32,
                                (region[2] as f32 + region[1] as f32 - 0.5 - f.uv[j][1])
                                    / self.atlas.height as f32,
                            ]
                        };
                        let cold_nozzle = damaged.is_none()
                            && !engine_face
                            && s.exhaust <= 0.
                            && if hornet_rig {
                                crate::aircraft_animation::part(f.address)
                                    == crate::aircraft_animation::Part::Nozzle
                            } else if let Some(rig) = &self.rig {
                                rig.cold_nozzle(f.address)
                            } else {
                                crate::rafale_animation::part(f.address)
                                    == crate::rafale_animation::Part::Nozzle
                            };
                        let color = if cold_nozzle {
                            [35, 36, 38]
                        } else {
                            self.palette[f.colors[j] as usize]
                        };
                        let textured = !f.uv.is_empty() && !cold_nozzle;
                        let layer = if flame && world.smooth_weather {
                            if textured { -7. } else { -6. }
                        } else if engine_face {
                            -3. - crate::engine_material::heat(s)
                        } else if canopy {
                            -5.
                        } else if textured {
                            if matches!(f.subtype, 0x4c | 0x5c | 0x6c | 0x7c) {
                                -2.
                            } else {
                                0.
                            }
                        } else {
                            -1.
                        };
                        result.extend_from_slice(&[
                            pos[0] + s.position[0] as f32,
                            pos[1] + s.position[1] as f32,
                            pos[2] + s.position[2] as f32,
                            uv[0],
                            uv[1],
                            layer,
                            color[0] as f32 / 255.,
                            color[1] as f32 / 255.,
                            color[2] as f32 / 255.,
                            // Preserve source indices for native weather remapping.
                            // The cold-nozzle material remains an authored exception.
                            if cold_nozzle || engine_face {
                                -1.
                            } else {
                                f.colors[j] as f32 + 256. * f.fog as u8 as f32 + 1024. * light_row
                            },
                        ]);
                    }
                }
            }
        }
        result
    }
}

fn streamer_world_points(
    def: &tore_formats::shape::StreamerDef,
    position: [f64; 3],
    attitude: [f64; 3],
) -> Option<[[f64; 3]; 2]> {
    let basis = tore_sim::attitude::Basis::new(attitude[0], attitude[1], attitude[2]);
    let mut points = [[0.; 3]; 2];
    for (side, out) in points.iter_mut().enumerate() {
        // Non-swing-wing aircraft use the source CE neutral hinge.
        let p = def.attachment(side, 0).ok()?;
        let (x, up, forward) = (p[0] / 3., p[1] / 3., p[2] / 3.);
        *out = std::array::from_fn(|k| {
            position[k] + basis.right[k] * x + basis.up[k] * up + basis.forward[k] * forward
        });
    }
    Some(points)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ce_uses_up_then_forward_and_follows_body_axes() {
        let def = tore_formats::shape::StreamerDef {
            pivot: [0; 3],
            hinge_scale: 0,
            points: [[12 * 256, 6 * 256, -15 * 256]; 2],
        };
        let q = std::f64::consts::FRAC_PI_2;
        for (attitude, expected) in [
            ([0.; 3], [4., 2., -5.]),
            ([q, 0., 0.], [-5., 2., -4.]),
            ([0., q, 0.], [4., -5., -2.]),
            ([0., 0., q], [2., -4., -5.]),
        ] {
            let actual = streamer_world_points(&def, [100.; 3], attitude).unwrap()[1];
            for i in 0..3 {
                assert!((actual[i] - 100. - expected[i]).abs() < 1e-10);
            }
        }
        assert_eq!(
            streamer_world_points(&def, [0.; 3], [0.; 3]).unwrap()[0],
            [-4., 2., -5.]
        );
    }
}
