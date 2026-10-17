//! Player graphics choices for the 3D view: anti-aliasing, render scale, the
//! spotting aid, terrain distance filtering, city fill distances, the
//! exploratory vegetation distances and the frame-rate counter. These are
//! opinionated host additions requested by John on 2026-09-22; the original
//! game had none of them. The vegetation rows were added on 2026-10-16, and
//! the frame-rate counter on 2026-10-17. The values and defaults are agent
//! choices. They are saved in their own small file, `graphics-v1.conf`,
//! beside the preferences.
use std::path::Path;

/// Multisample anti-aliasing. 4x is the only count every GPU must support;
/// 2x and 8x are offered only when the adapter reports them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AntiAliasing {
    Off,
    X2,
    X4,
    X8,
}
impl AntiAliasing {
    pub const ALL: [Self; 4] = [Self::Off, Self::X2, Self::X4, Self::X8];
    pub fn samples(self) -> u32 {
        match self {
            Self::Off => 1,
            Self::X2 => 2,
            Self::X4 => 4,
            Self::X8 => 8,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::X2 => "2x",
            Self::X4 => "4x",
            Self::X8 => "8x",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::X2 => "2x",
            Self::X4 => "4x",
            Self::X8 => "8x",
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.key() == text)
    }
}

/// How strongly distant aircraft are outlined against the background.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpottingAid {
    Off,
    Subtle,
    Strong,
}
impl SpottingAid {
    pub const ALL: [Self; 3] = [Self::Off, Self::Subtle, Self::Strong];
    /// Peak outline opacity handed to the shader.
    pub fn strength(self) -> f32 {
        match self {
            Self::Off => 0.,
            Self::Subtle => 0.5,
            Self::Strong => 0.85,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::Subtle => "Subtle",
            Self::Strong => "Strong",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Subtle => "subtle",
            Self::Strong => "strong",
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.key() == text)
    }
}

/// Render scale steps, in percent of the window's pixel size.
pub const RENDER_SCALES: [u32; 5] = [75, 100, 125, 150, 200];

/// City fill building draw distance steps, nautical miles (CF0 prototype;
/// John, 2026-10-10).
pub const BUILDING_DISTANCES: [u32; 3] = [10, 20, 30];
/// City fill building texture and detail distance steps, nautical miles:
/// full detail inside, plain boxes beyond.
pub const BUILDING_DETAILS: [u32; 5] = [1, 2, 5, 10, 20];
/// The defaults: geometry to 30 nm, full detail inside 5 nm (John,
/// 2026-10-10).
pub const DEFAULT_BUILDING_DISTANCE: u32 = 30;
pub const DEFAULT_BUILDING_DETAIL: u32 = 5;

/// Close-range ground detail (fields, grass, sand, rock) on the terrain.
/// Opinionated, 2026-10-16. Off draws the retail tile alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GroundTexture {
    Off,
    On,
    /// A finer grain, for a low pass.
    Fine,
}
impl GroundTexture {
    pub const ALL: [Self; 3] = [Self::Off, Self::On, Self::Fine];
    /// Shader mode: 0 off, 1 on, 2 fine.
    pub fn mode(self) -> f32 {
        match self {
            Self::Off => 0.,
            Self::On => 1.,
            Self::Fine => 2.,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Off => "Off",
            Self::On => "On",
            Self::Fine => "Fine",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::On => "on",
            Self::Fine => "fine",
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.key() == text)
    }
}

/// How thick the vegetation objects are, relative to the high setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VegDensity {
    Low,
    Medium,
    High,
    Ultra,
}
impl VegDensity {
    pub const ALL: [Self; 4] = [Self::Low, Self::Medium, Self::High, Self::Ultra];
    /// Placement multiplier. High is the tuned density.
    pub fn scale(self) -> f32 {
        match self {
            Self::Low => 0.4,
            Self::Medium => 0.7,
            Self::High => 1.,
            Self::Ultra => 1.65,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Low => "Low",
            Self::Medium => "Med",
            Self::High => "High",
            Self::Ultra => "Ultra",
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.key() == text)
    }
}

/// How far the close-range ground detail reaches, nautical miles.
pub const GROUND_TEXTURE_DISTANCES: [u32; 3] = [1, 2, 4];
/// How far vegetation objects are drawn, nautical miles.
pub const VEGETATION_DISTANCES: [u32; 4] = [2, 4, 6, 8];
/// Full vegetation meshes inside this, cheaper crosses beyond, nautical miles.
pub const VEGETATION_DETAILS: [u32; 4] = [1, 2, 3, 4];
/// Tuned for an RTX 4070 at 4K: detail close in, objects out to 4 nm, full
/// meshes inside 2 nm, high density (2026-10-16).
pub const DEFAULT_GROUND_TEXTURE: GroundTexture = GroundTexture::On;
pub const DEFAULT_GROUND_TEXTURE_DISTANCE: u32 = 2;
pub const DEFAULT_VEGETATION_DISTANCE: u32 = 4;
pub const DEFAULT_VEGETATION_DETAIL: u32 = 2;
pub const DEFAULT_VEGETATION_DENSITY: VegDensity = VegDensity::High;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub anti_aliasing: AntiAliasing,
    /// Percent; one of `RENDER_SCALES`.
    pub render_scale: u32,
    pub spotting_aid: SpottingAid,
    pub terrain_filtering: bool,
    /// City fill buildings draw out to this, nautical miles; one of
    /// `BUILDING_DISTANCES`.
    pub building_distance_nm: u32,
    /// City fill buildings draw with full texture and detail inside this,
    /// nautical miles; one of `BUILDING_DETAILS`.
    pub building_detail_nm: u32,
    /// Close-range ground detail. One of `GroundTexture::ALL`.
    pub ground_texture: GroundTexture,
    /// How far that detail reaches, nautical miles; one of
    /// `GROUND_TEXTURE_DISTANCES`.
    pub ground_texture_distance_nm: u32,
    /// Vegetation objects draw out to this, nautical miles; one of
    /// `VEGETATION_DISTANCES`.
    pub vegetation_distance_nm: u32,
    /// Full vegetation meshes inside this, nautical miles; one of
    /// `VEGETATION_DETAILS`.
    pub vegetation_detail_nm: u32,
    /// How thick the vegetation is. One of `VegDensity::ALL`.
    pub vegetation_density: VegDensity,
    /// The frames-per-second readout in the window's top right. Off by
    /// default (John, 2026-10-17). `TORE_SHOW_FPS=1` shows it for one run
    /// without saving.
    pub show_frame_rate: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            anti_aliasing: AntiAliasing::X4,
            render_scale: 100,
            spotting_aid: SpottingAid::Subtle,
            terrain_filtering: true,
            building_distance_nm: DEFAULT_BUILDING_DISTANCE,
            building_detail_nm: DEFAULT_BUILDING_DETAIL,
            ground_texture: DEFAULT_GROUND_TEXTURE,
            ground_texture_distance_nm: DEFAULT_GROUND_TEXTURE_DISTANCE,
            vegetation_distance_nm: DEFAULT_VEGETATION_DISTANCE,
            vegetation_detail_nm: DEFAULT_VEGETATION_DETAIL,
            vegetation_density: DEFAULT_VEGETATION_DENSITY,
            show_frame_rate: false,
        }
    }
}
impl Options {
    /// Every addition off: the unmodified presentation, for pixel tests.
    /// Vegetation objects keep their distances, as the city buildings do;
    /// the ground detail texture turns off with the other image filters.
    pub fn original() -> Self {
        Self {
            anti_aliasing: AntiAliasing::Off,
            render_scale: 100,
            spotting_aid: SpottingAid::Off,
            terrain_filtering: false,
            building_distance_nm: DEFAULT_BUILDING_DISTANCE,
            building_detail_nm: DEFAULT_BUILDING_DETAIL,
            ground_texture: GroundTexture::Off,
            ground_texture_distance_nm: DEFAULT_GROUND_TEXTURE_DISTANCE,
            vegetation_distance_nm: DEFAULT_VEGETATION_DISTANCE,
            vegetation_detail_nm: DEFAULT_VEGETATION_DETAIL,
            vegetation_density: DEFAULT_VEGETATION_DENSITY,
            show_frame_rate: false,
        }
    }
    pub fn scale(&self) -> f32 {
        self.render_scale as f32 / 100.
    }
    /// The settings file. The city fill building distances are written only
    /// when they differ from the defaults, so a default file is unchanged.
    pub fn text(&self) -> String {
        let mut text = format!(
            "tore-graphics 1\nanti-aliasing {}\nrender-scale {}\nspotting-aid {}\nterrain-filtering {}\n",
            self.anti_aliasing.key(),
            self.render_scale,
            self.spotting_aid.key(),
            self.terrain_filtering,
        );
        if self.building_distance_nm != DEFAULT_BUILDING_DISTANCE {
            text += &format!("building-distance {}\n", self.building_distance_nm);
        }
        if self.building_detail_nm != DEFAULT_BUILDING_DETAIL {
            text += &format!("building-detail {}\n", self.building_detail_nm);
        }
        if self.ground_texture != DEFAULT_GROUND_TEXTURE {
            text += &format!("vegetation-texture {}\n", self.ground_texture.key());
        }
        if self.ground_texture_distance_nm != DEFAULT_GROUND_TEXTURE_DISTANCE {
            text += &format!(
                "vegetation-texture-distance {}\n",
                self.ground_texture_distance_nm
            );
        }
        if self.vegetation_distance_nm != DEFAULT_VEGETATION_DISTANCE {
            text += &format!("vegetation-distance {}\n", self.vegetation_distance_nm);
        }
        if self.vegetation_detail_nm != DEFAULT_VEGETATION_DETAIL {
            text += &format!("vegetation-detail {}\n", self.vegetation_detail_nm);
        }
        if self.vegetation_density != DEFAULT_VEGETATION_DENSITY {
            text += &format!("vegetation-density {}\n", self.vegetation_density.key());
        }
        if self.show_frame_rate {
            text += "frame-rate true\n";
        }
        text
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 4096 {
            return Err("graphics settings too large".into());
        }
        let mut lines = text.lines();
        if lines.next() != Some("tore-graphics 1") {
            return Err("unsupported graphics settings version".into());
        }
        let mut values = std::collections::BTreeMap::new();
        for line in lines {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() != 2 || values.insert(fields[0], fields[1]).is_some() {
                return Err("invalid or duplicate graphics setting".into());
            }
        }
        let optional = [
            "building-distance",
            "building-detail",
            "vegetation-texture",
            "vegetation-texture-distance",
            "vegetation-distance",
            "vegetation-detail",
            "vegetation-density",
            "frame-rate",
        ]
        .iter()
        .filter(|k| values.contains_key(*k))
        .count();
        if values.len() != 4 + optional {
            return Err("unknown or missing graphics setting".into());
        }
        let get = |k: &str| values.get(k).copied().ok_or(format!("missing {k}"));
        let step = |k: &str, steps: &[u32], default: u32| match values.get(k) {
            None => Ok(default),
            Some(v) => v
                .parse::<u32>()
                .ok()
                .filter(|n| steps.contains(n))
                .ok_or(format!("invalid {k}")),
        };
        let boolean = |k: &str| get(k)?.parse::<bool>().map_err(|_| format!("invalid {k}"));
        let render_scale = get("render-scale")?
            .parse::<u32>()
            .ok()
            .filter(|s| RENDER_SCALES.contains(s))
            .ok_or("invalid render-scale")?;
        Ok(Self {
            anti_aliasing: AntiAliasing::parse(get("anti-aliasing")?)
                .ok_or("invalid anti-aliasing")?,
            render_scale,
            spotting_aid: SpottingAid::parse(get("spotting-aid")?).ok_or("invalid spotting-aid")?,
            terrain_filtering: boolean("terrain-filtering")?,
            building_distance_nm: step(
                "building-distance",
                &BUILDING_DISTANCES,
                DEFAULT_BUILDING_DISTANCE,
            )?,
            building_detail_nm: step(
                "building-detail",
                &BUILDING_DETAILS,
                DEFAULT_BUILDING_DETAIL,
            )?,
            ground_texture: match values.get("vegetation-texture") {
                None => DEFAULT_GROUND_TEXTURE,
                Some(v) => GroundTexture::parse(v).ok_or("invalid vegetation-texture")?,
            },
            ground_texture_distance_nm: step(
                "vegetation-texture-distance",
                &GROUND_TEXTURE_DISTANCES,
                DEFAULT_GROUND_TEXTURE_DISTANCE,
            )?,
            vegetation_distance_nm: step(
                "vegetation-distance",
                &VEGETATION_DISTANCES,
                DEFAULT_VEGETATION_DISTANCE,
            )?,
            vegetation_detail_nm: step(
                "vegetation-detail",
                &VEGETATION_DETAILS,
                DEFAULT_VEGETATION_DETAIL,
            )?,
            vegetation_density: match values.get("vegetation-density") {
                None => DEFAULT_VEGETATION_DENSITY,
                Some(v) => VegDensity::parse(v).ok_or("invalid vegetation-density")?,
            },
            show_frame_rate: match values.get("frame-rate") {
                None => false,
                Some(_) => boolean("frame-rate")?,
            },
        })
    }
    /// Command-line overrides for one run; they are not saved by themselves.
    pub fn apply_flags(&mut self, flags: &[(String, String)]) -> Result<(), String> {
        let switch = |flag: &str, value: &str| match value {
            "on" => Ok(true),
            "off" => Ok(false),
            _ => Err(format!("{flag} needs on or off")),
        };
        for (flag, value) in flags {
            match flag.as_str() {
                "--original-graphics" => *self = Self::original(),
                "--anti-aliasing" => {
                    self.anti_aliasing = AntiAliasing::parse(value)
                        .ok_or("--anti-aliasing needs off, 2x, 4x or 8x")?
                }
                "--render-scale" => {
                    self.render_scale = value
                        .parse()
                        .ok()
                        .filter(|s| RENDER_SCALES.contains(s))
                        .ok_or("--render-scale needs 75, 100, 125, 150 or 200")?
                }
                "--spotting-aid" => {
                    self.spotting_aid = SpottingAid::parse(value)
                        .ok_or("--spotting-aid needs off, subtle or strong")?
                }
                "--terrain-filtering" => self.terrain_filtering = switch(flag, value)?,
                "--building-distance" => {
                    self.building_distance_nm = value
                        .parse()
                        .ok()
                        .filter(|n| BUILDING_DISTANCES.contains(n))
                        .ok_or("--building-distance needs 10, 20 or 30")?
                }
                "--building-detail" => {
                    self.building_detail_nm = value
                        .parse()
                        .ok()
                        .filter(|n| BUILDING_DETAILS.contains(n))
                        .ok_or("--building-detail needs 1, 2, 5, 10 or 20")?
                }
                "--vegetation-texture" => {
                    self.ground_texture = GroundTexture::parse(value)
                        .ok_or("--vegetation-texture needs off, on or fine")?
                }
                "--vegetation-texture-distance" => {
                    self.ground_texture_distance_nm = value
                        .parse()
                        .ok()
                        .filter(|n| GROUND_TEXTURE_DISTANCES.contains(n))
                        .ok_or("--vegetation-texture-distance needs 1, 2 or 4")?
                }
                "--vegetation-distance" => {
                    self.vegetation_distance_nm = value
                        .parse()
                        .ok()
                        .filter(|n| VEGETATION_DISTANCES.contains(n))
                        .ok_or("--vegetation-distance needs 2, 4, 6 or 8")?
                }
                "--vegetation-detail" => {
                    self.vegetation_detail_nm = value
                        .parse()
                        .ok()
                        .filter(|n| VEGETATION_DETAILS.contains(n))
                        .ok_or("--vegetation-detail needs 1, 2, 3 or 4")?
                }
                "--vegetation-density" => {
                    self.vegetation_density = VegDensity::parse(value)
                        .ok_or("--vegetation-density needs low, medium, high or ultra")?
                }
                _ => return Err(format!("unknown graphics flag {flag}")),
            }
        }
        Ok(())
    }
    /// A missing file gives the defaults. An unreadable or damaged one does
    /// too, with a warning in the session log saying why, like the other
    /// settings files.
    pub fn load(path: &Path) -> Self {
        match crate::preferences::read(path) {
            Ok(text) => Self::parse(&text).unwrap_or_else(|error| {
                log::warn!(
                    "Graphics options not loaded from {}: {error}; using the defaults",
                    path.display()
                );
                Self::default()
            }),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                log::warn!(
                    "Graphics options not loaded from {}: {error}; using the defaults",
                    path.display()
                );
                Self::default()
            }
        }
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        crate::preferences::write(path, &self.text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn roundtrip_and_reject_malformed() {
        let options = Options {
            anti_aliasing: AntiAliasing::X8,
            render_scale: 150,
            spotting_aid: SpottingAid::Strong,
            terrain_filtering: false,
            building_distance_nm: 20,
            building_detail_nm: 2,
            ground_texture: GroundTexture::Fine,
            ground_texture_distance_nm: 4,
            vegetation_distance_nm: 6,
            vegetation_detail_nm: 1,
            vegetation_density: VegDensity::Low,
            show_frame_rate: true,
        };
        assert_eq!(Options::parse(&options.text()).unwrap(), options);
        // The default building and vegetation distances are not written, and
        // a file without them reads as the defaults.
        let defaults = Options::default().text();
        assert!(!defaults.contains("building"));
        assert!(!defaults.contains("vegetation"));
        assert!(!defaults.contains("vegetation-texture"));
        assert!(!defaults.contains("frame-rate"));
        assert!(options.text().contains("frame-rate true"));
        assert!(
            Options::parse(
                &options
                    .text()
                    .replace("building-detail 2", "building-detail 3")
            )
            .is_err()
        );
        assert_eq!(
            Options::parse(&Options::default().text()).unwrap(),
            Options::default()
        );
        let text = options.text();
        assert!(Options::parse(&text.replace("150", "140")).is_err());
        assert!(Options::parse(&text.replace("8x", "16x")).is_err());
        assert!(Options::parse(&(text.clone() + "terrain-filtering true\n")).is_err());
        assert!(Options::parse(&(text.clone() + "sun-glint true\n")).is_err());
        assert!(Options::parse(&text.replace("tore-graphics 1", "tore-graphics 2")).is_err());
        assert!(Options::parse("tore-graphics 1\n").is_err());
    }
    #[test]
    fn a_missing_or_damaged_file_gives_the_defaults() {
        let dir = std::env::temp_dir().join(format!("tore-graphics-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(Options::load(&dir.join("none.conf")), Options::default());
        let bad = dir.join("bad.conf");
        std::fs::write(&bad, b"\xff\xfe not a config").unwrap();
        assert_eq!(Options::load(&bad), Options::default());
        let good = dir.join("good.conf");
        let options = Options {
            render_scale: 150,
            ..Options::default()
        };
        options.save(&good).unwrap();
        assert_eq!(Options::load(&good), options);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn flags_override_in_order() {
        let flag = |f: &str, v: &str| (f.to_owned(), v.to_owned());
        let mut options = Options::default();
        options
            .apply_flags(&[
                flag("--original-graphics", ""),
                flag("--spotting-aid", "strong"),
                flag("--render-scale", "150"),
            ])
            .unwrap();
        assert_eq!(options.anti_aliasing, AntiAliasing::Off);
        assert_eq!(options.spotting_aid, SpottingAid::Strong);
        assert_eq!(options.render_scale, 150);
        assert!(
            options
                .apply_flags(&[flag("--render-scale", "110")])
                .is_err()
        );
        assert!(
            options
                .apply_flags(&[flag("--terrain-filtering", "yes")])
                .is_err()
        );
    }
}
