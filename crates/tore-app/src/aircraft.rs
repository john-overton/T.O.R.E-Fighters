//! Imported Hornet geometry, cockpit and data. Does not use reference runtime code.
use crate::{
    AppResult, flight,
    menu::Sprite,
    terrain::{Camera, World},
};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{Pic, aircraft::Aircraft, font::Font, shape::Shape};
pub struct Hornet {
    pub profile: Aircraft,
    pub atlas: Pic,
    pub palette: [[u8; 3]; 256],
    pub sprites: BTreeMap<String, Sprite>,
    pub font: Font,
    pub hud_font: Font,
    pub flight_menu: Vec<tore_formats::ui::MenuNode>,
    pub equipment: BTreeMap<String, tore_formats::aircraft::Equipment>,
    pub poses: Vec<Shape>,
}
impl Hornet {
    pub fn load(data: &BTreeMap<String, Vec<u8>>) -> AppResult<Self> {
        let get = |s: &str| {
            data.get(s)
                .ok_or_else(|| format!("aircraft cache missing {s}; re-import media"))
        };
        let profile = Aircraft::parse(get("F18.PT")?)?;
        let shape = Shape::parse(get(&profile.shape)?)?;
        if ![0x7900, 0x790c, 0x7912, 0x791e]
            .iter()
            .all(|word| shape.state_words.contains(word))
            || tore_formats::module::code(get(&profile.shape)?)?.0.len() != 26934
        {
            return Err("unreviewed F18.SH device layout; preserve raw import and review its rig before flying".into());
        }
        let atlas = Pic::parse(get("_F18.PIC")?)?;
        let raw = get("PALETTE.PAL")?;
        if raw.len() != 768 || raw.iter().any(|v| *v > 63) {
            return Err("invalid aircraft palette".into());
        }
        let palette = std::array::from_fn(|i| {
            std::array::from_fn(|j| ((raw[i * 3 + j] as u16 * 255 + 31) / 63) as u8)
        });
        let frame = Pic::parse(get("~F18H.PIC")?)?;
        let mut cockpit_palette = palette;
        cockpit_palette[..frame.palette.len()].copy_from_slice(&frame.palette);
        let mut sprites = BTreeMap::new();
        for name in tore_formats::aircraft::INSTRUMENT_ART
            .iter()
            .copied()
            .chain(["~F18H.PIC", "~F18_LH.PIC", "~F18_CH.PIC", "~F18_RH.PIC"])
        {
            {
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
        let mut equipment = BTreeMap::new();
        for station in &profile.hardpoints {
            if let Some(name) = &station.store
                && [".SEE", ".ECM", ".JT"]
                    .iter()
                    .any(|ext| name.ends_with(ext))
            {
                equipment.insert(
                    name.clone(),
                    tore_formats::aircraft::Equipment::parse(name, get(name)?)?,
                );
            }
        }
        let mut poses = Vec::new();
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

        println!(
            "Aircraft: {} — {} exterior faces, {} G rows, {} hardpoints; atlas {}x{}, instrument font {}px",
            profile.name,
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
        Ok(Self {
            profile,
            atlas,
            palette,
            sprites,
            font,
            hud_font: Font::parse(get("HUD11.FNT")?)?,
            flight_menu,
            equipment,
            poses,
        })
    }
    pub fn start(&self, world: &World) -> flight::State {
        let c = Camera::for_world(world);
        let mut p = c.position.map(|v| v as f64);
        p[1] = 5000f64.max(world.height(p[0] as f32, p[2] as f32) as f64 + 2000.);
        flight::State::new(&self.profile, p)
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
    pub fn vertices(&self, s: &flight::State, camera: &Camera) -> Vec<f32> {
        let mut result = Vec::new();
        let (sy, cy) = (s.yaw as f32).sin_cos();
        let (sp, cp) = (s.pitch as f32).sin_cos();
        let (sb, cb) = (s.bank as f32).sin_cos();
        let mask = usize::from(s.engine && s.burner && s.throttle > 0.95)
            + 2 * usize::from(s.brake > 0.5)
            + 4 * usize::from(s.gear > 0.5)
            + 8 * usize::from(s.hook > 0.5);
        let orient = |p: [f32; 3]| {
            let (x, y, z) = (p[0], p[1], p[2]);
            let (x, y) = (x * cb + y * sb, -x * sb + y * cb);
            let (y, z) = (y * cp + z * sp, -y * sp + z * cp);
            [x * cy + z * sy, y, -x * sy + z * cy]
        };
        for f in &self.poses[mask].faces {
            if let Some(n) = f.normal {
                let normal = orient(n);
                let p = f.positions[0];
                let p = orient([p[0] / 3., p[2] / 3., p[1] / 3.]);
                let dot: f32 = (0..3)
                    .map(|i| normal[i] * (camera.position[i] - s.position[i] as f32 - p[i]))
                    .sum();
                if dot <= 0. {
                    continue;
                }
            }

            for i in 1..f.positions.len() - 1 {
                for j in [0, i, i + 1] {
                    let p = f.positions[j];
                    let scale = 1. / 3.;
                    let (x, y, z) = (p[0] * scale, p[2] * scale, p[1] * scale);
                    let (x, y) = (x * cb + y * sb, -x * sb + y * cb);
                    let (y, z) = (y * cp + z * sp, -y * sp + z * cp);
                    let pos = [x * cy + z * sy, y, -x * sy + z * cy];
                    let uv = if f.uv.is_empty() {
                        [0.; 2]
                    } else {
                        [
                            (f.uv[j][0] + 0.5) / self.atlas.width as f32,
                            (self.atlas.height as f32 - 0.5 - f.uv[j][1])
                                / self.atlas.height as f32,
                        ]
                    };
                    let color = self.palette[f.colors[j] as usize];
                    let textured = !f.uv.is_empty();
                    let layer = if textured {
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
                    ]);
                }
            }
        }
        result
    }
}
