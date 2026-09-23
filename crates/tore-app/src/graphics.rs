//! Player graphics choices for the 3D view: anti-aliasing, render scale, the
//! spotting aid and terrain distance filtering. These are
//! opinionated host additions requested by John on 2026-09-22; the original
//! game had none of them. The values and defaults are agent choices. They are
//! saved in their own small file, `graphics-v1.conf`, beside the preferences.
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub anti_aliasing: AntiAliasing,
    /// Percent; one of `RENDER_SCALES`.
    pub render_scale: u32,
    pub spotting_aid: SpottingAid,
    pub terrain_filtering: bool,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            anti_aliasing: AntiAliasing::X4,
            render_scale: 100,
            spotting_aid: SpottingAid::Subtle,
            terrain_filtering: true,
        }
    }
}
impl Options {
    /// Every addition off: the unmodified presentation, for pixel tests.
    pub fn original() -> Self {
        Self {
            anti_aliasing: AntiAliasing::Off,
            render_scale: 100,
            spotting_aid: SpottingAid::Off,
            terrain_filtering: false,
        }
    }
    pub fn scale(&self) -> f32 {
        self.render_scale as f32 / 100.
    }
    pub fn text(&self) -> String {
        format!(
            "tore-graphics 1\nanti-aliasing {}\nrender-scale {}\nspotting-aid {}\nterrain-filtering {}\n",
            self.anti_aliasing.key(),
            self.render_scale,
            self.spotting_aid.key(),
            self.terrain_filtering,
        )
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
        if values.len() != 4 {
            return Err("unknown or missing graphics setting".into());
        }
        let get = |k: &str| values.get(k).copied().ok_or(format!("missing {k}"));
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
                _ => return Err(format!("unknown graphics flag {flag}")),
            }
        }
        Ok(())
    }
    /// A missing or unreadable file falls back to the defaults.
    pub fn load(path: &Path) -> Self {
        crate::preferences::read(path)
            .ok()
            .and_then(|text| Self::parse(&text).ok())
            .unwrap_or_default()
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
        };
        assert_eq!(Options::parse(&options.text()).unwrap(), options);
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
