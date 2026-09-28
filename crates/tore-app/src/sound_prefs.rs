//! The Sound/Music Prefs settings: nine slider levels and the channel swap,
//! saved in their own file and handed to the mixer as [`Volumes`]. Behaviour
//! and provenance: [spec](../../../docs/spec/sound-prefs.md).
use crate::audio::Volumes;
use std::path::Path;

/// The dialog's sliders, in the order the file and the screen list them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Slider {
    Overall,
    Engine,
    WeaponLock,
    Rwr,
    StallWarn,
    RadioMsg,
    InFlightMusic,
    OtherMusic,
    StereoSeparation,
}
impl Slider {
    pub const ALL: [Slider; 9] = [
        Slider::Overall,
        Slider::Engine,
        Slider::WeaponLock,
        Slider::Rwr,
        Slider::StallWarn,
        Slider::RadioMsg,
        Slider::InFlightMusic,
        Slider::OtherMusic,
        Slider::StereoSeparation,
    ];
    fn key(self) -> &'static str {
        match self {
            Slider::Overall => "overall",
            Slider::Engine => "engine",
            Slider::WeaponLock => "weapon-lock",
            Slider::Rwr => "rwr",
            Slider::StallWarn => "stall-warn",
            Slider::RadioMsg => "radio-msg",
            Slider::InFlightMusic => "in-flight-music",
            Slider::OtherMusic => "other-music",
            Slider::StereoSeparation => "stereo-separation",
        }
    }
}

/// Highest slider level; levels are percent of the full level.
pub const MAX: u8 = 100;
/// The original's levels for a new or unreadable profile, in
/// [`Slider::ALL`] order.
pub const DEFAULTS: [u8; 9] = [75, 80, 60, 50, 80, 95, 75, 75, 80];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    /// Each slider's level, 0 to [`MAX`], in [`Slider::ALL`] order.
    pub levels: [u8; 9],
    /// Swap Left/Right Channels.
    pub swap: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            levels: DEFAULTS,
            swap: false,
        }
    }
}
impl Settings {
    pub fn level(&self, slider: Slider) -> u8 {
        self.levels[slider as usize]
    }
    pub fn set(&mut self, slider: Slider, level: u8) {
        self.levels[slider as usize] = level.min(MAX);
    }
    /// A profile saved before this screen existed: its Music and Effects
    /// switches become the music and Overall levels.
    pub fn from_legacy(music: bool, effects: bool) -> Self {
        let mut settings = Self::default();
        if !music {
            settings.set(Slider::InFlightMusic, 0);
            settings.set(Slider::OtherMusic, 0);
        }
        if !effects {
            settings.set(Slider::Overall, 0);
        }
        settings
    }
    /// Mixer levels relative to each slider's default: TORE's mix is what
    /// the original defaults sound like, and a slider scales its sounds by
    /// its level over its default, linearly as the original does.
    pub fn volumes(&self) -> Volumes {
        let level =
            |slider: Slider| f32::from(self.level(slider)) / f32::from(DEFAULTS[slider as usize]);
        Volumes {
            overall: level(Slider::Overall),
            engine: level(Slider::Engine),
            weapon_lock: level(Slider::WeaponLock),
            rwr: level(Slider::Rwr),
            stall: level(Slider::StallWarn),
            radio: level(Slider::RadioMsg),
            flight_music: level(Slider::InFlightMusic),
            other_music: level(Slider::OtherMusic),
            separation: self.level(Slider::StereoSeparation),
            swap: self.swap,
        }
    }
    pub fn text(&self) -> String {
        let mut text = String::from("tore-sound 1\n");
        for slider in Slider::ALL {
            text += &format!("{} {}\n", slider.key(), self.level(slider));
        }
        text += &format!("swap-channels {}\n", self.swap);
        text
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 4096 {
            return Err("sound settings too large".into());
        }
        let mut lines = text.lines();
        if lines.next() != Some("tore-sound 1") {
            return Err("unsupported sound settings version".into());
        }
        let mut values = std::collections::BTreeMap::new();
        for line in lines {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() != 2 || values.insert(fields[0], fields[1]).is_some() {
                return Err("invalid or duplicate sound setting".into());
            }
        }
        if values.len() != Slider::ALL.len() + 1 {
            return Err("unknown or missing sound setting".into());
        }
        let mut settings = Self::default();
        for slider in Slider::ALL {
            let key = slider.key();
            let level = values
                .get(key)
                .and_then(|v| v.parse::<u8>().ok())
                .filter(|v| *v <= MAX)
                .ok_or(format!("invalid {key}"))?;
            settings.set(slider, level);
        }
        settings.swap = values
            .get("swap-channels")
            .and_then(|v| v.parse().ok())
            .ok_or("invalid swap-channels")?;
        Ok(settings)
    }
    /// The saved settings; a missing file falls back to `fallback`, an
    /// unreadable or malformed one to the defaults.
    pub fn load(path: &Path, fallback: Self) -> Self {
        match crate::preferences::read(path) {
            Ok(text) => Self::parse(&text).unwrap_or_else(|e| {
                log::warn!("Sound settings not loaded: {e}; using the defaults");
                Self::default()
            }),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => fallback,
            Err(e) => {
                log::warn!("Sound settings not loaded: {e}; using the defaults");
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
        let mut s = Settings::default();
        for (i, slider) in Slider::ALL.into_iter().enumerate() {
            s.set(slider, i as u8 * 10);
        }
        s.swap = true;
        assert_eq!(Settings::parse(&s.text()).unwrap(), s);
        let text = s.text();
        for broken in [
            text.replace("tore-sound 1", "tore-sound 2"),
            text.replace("overall 0\n", ""),
            text.replace("engine 10", "engine 101"),
            text.replace("engine 10", "engine -1"),
            text.replace("swap-channels true", "swap-channels 1"),
            text.clone() + "music 5\n",
            text.clone() + "engine 5\n",
        ] {
            assert!(Settings::parse(&broken).is_err(), "{broken}");
        }
    }
    #[test]
    fn levels_become_mixer_volumes() {
        let mut s = Settings::default();
        let v = s.volumes();
        assert_eq!((v.overall, v.engine, v.rwr, v.radio), (1., 1., 1., 1.));
        assert_eq!(v.separation, 80);
        assert!(!v.swap);
        s.set(Slider::Overall, 0);
        s.set(Slider::Rwr, MAX);
        s.set(Slider::RadioMsg, 19);
        s.set(Slider::StereoSeparation, 0);
        s.swap = true;
        let v = s.volumes();
        assert_eq!((v.overall, v.rwr, v.radio, v.separation), (0., 2., 0.2, 0));
        assert!(v.swap);
        s.set(Slider::Engine, 200);
        assert_eq!(s.level(Slider::Engine), MAX);
    }
    #[test]
    fn legacy_switches_carry_over() {
        let quiet = Settings::from_legacy(false, true);
        assert_eq!(quiet.level(Slider::InFlightMusic), 0);
        assert_eq!(quiet.level(Slider::OtherMusic), 0);
        assert_eq!(quiet.level(Slider::Overall), 75);
        let silent = Settings::from_legacy(true, false);
        assert_eq!(silent.level(Slider::Overall), 0);
        assert_eq!(silent.level(Slider::OtherMusic), 75);
        assert_eq!(Settings::from_legacy(true, true), Settings::default());
    }
}
