//! Small bounded user preference files, independent of retail assets and flight state.
use crate::{
    flight_ui::FlightUi,
    instruments::{Instruments, Layout},
    menu::State,
};
use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

pub fn read(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    fs::File::open(path)?
        .take(256 * 1024 + 1)
        .read_to_string(&mut text)?;
    if text.len() > 256 * 1024 {
        return Err(io::Error::other("settings file exceeds 256 KiB"));
    }
    Ok(text)
}
pub fn write(path: &Path, text: &str) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".tore-settings-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    // Cleanup is allowed only after successfully creating our own temporary file.
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub zoom: f32,
    pub rwr_range: usize,
    pub radar_range: usize,
    pub radar_mode: usize,
    pub selected: usize,
    pub small: bool,
    pub large_pages: Vec<u8>,
    pub small_pages: Vec<u8>,
    pub cockpit: bool,
    pub hud: bool,
    pub ladder: bool,
    pub brightness: i16,
    pub music: bool,
    pub effects: bool,
}
impl Preferences {
    pub fn capture(ui: &FlightUi, i: &Instruments, m: &State) -> Self {
        let (large, small) = if i.layout == Layout::Large {
            (&i.pages, &i.other_pages)
        } else {
            (&i.other_pages, &i.pages)
        };
        Self {
            zoom: ui.zoom,
            rwr_range: i.rwr_range,
            radar_range: i.radar_range,
            radar_mode: i.mode,
            selected: i.selected,
            small: i.layout == Layout::Small,
            large_pages: large.clone(),
            small_pages: small.clone(),
            cockpit: ui.cockpit,
            hud: ui.hud,
            ladder: ui.ladder,
            brightness: ui.brightness,
            music: m.music,
            effects: m.effects,
        }
    }
    pub fn apply(&self, ui: &mut FlightUi, i: &mut Instruments, m: &mut State) {
        i.layout = if self.small {
            Layout::Small
        } else {
            Layout::Large
        };
        (i.pages, i.other_pages) = if self.small {
            (self.small_pages.clone(), self.large_pages.clone())
        } else {
            (self.large_pages.clone(), self.small_pages.clone())
        };
        ui.zoom = self.zoom;
        i.rwr_range = self.rwr_range;
        i.radar_range = self.radar_range;
        i.mode = self.radar_mode;
        i.selected = self.selected.min(i.pages.len().saturating_sub(1));
        i.pressed = None;
        i.cameras.clear();
        ui.cockpit = self.cockpit;
        ui.hud = self.hud;
        ui.ladder = self.ladder;
        ui.brightness = self.brightness;
        ui.effects = self.effects;
        m.music = self.music;
        m.effects = self.effects;
    }
    pub fn text(&self) -> String {
        fn pages(p: &[u8]) -> String {
            if p.is_empty() {
                "-".into()
            } else {
                p.iter().map(u8::to_string).collect::<Vec<_>>().join(",")
            }
        }
        format!(
            "tore-preferences 2\nzoom {}\nrwr-range {}\nradar-range {}\nradar-mode {}\nselected {}\nsmall {}\nlarge-pages {}\nsmall-pages {}\ncockpit {}\nhud {}\nladder {}\nbrightness {}\nmusic {}\neffects {}\n",
            self.zoom,
            self.rwr_range,
            self.radar_range,
            self.radar_mode,
            self.selected,
            self.small,
            pages(&self.large_pages),
            pages(&self.small_pages),
            self.cockpit,
            self.hud,
            self.ladder,
            self.brightness,
            self.music,
            self.effects
        )
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 16384 {
            return Err("preferences too large".into());
        }
        let mut values = std::collections::BTreeMap::new();
        let mut lines = text.lines();
        let legacy = match lines.next() {
            Some("tore-preferences 1") => true,
            Some("tore-preferences 2") => false,
            _ => return Err("unsupported preferences version".into()),
        };
        for line in lines {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() != 2 || values.insert(fields[0], fields[1]).is_some() {
                return Err("invalid or duplicate preference".into());
            }
        }
        let get = |k| {
            values
                .get(k)
                .copied()
                .ok_or_else(|| format!("missing preference {k}"))
        };
        let boolean =
            |k| -> Result<bool, String> { get(k)?.parse().map_err(|_| format!("invalid {k}")) };
        let pages = |k, cap| -> Result<Vec<u8>, String> {
            let s = get(k)?;
            if s == "-" {
                return Ok(vec![]);
            }
            let p: Vec<u8> = s
                .split(',')
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(|_| "invalid instrument pages")?;
            if p.len() > cap || p.iter().any(|n| *n > 9) {
                return Err("instrument pages outside bounds".into());
            }
            Ok(p)
        };
        if values.len() != 14 {
            return Err("unknown preference".into());
        }
        let brightness = get("brightness")?
            .parse::<i16>()
            .map_err(|_| "invalid brightness")?;
        let brightness = if legacy {
            if !(0..=9).contains(&brightness) {
                return Err("brightness outside legacy bounds".into());
            }
            // Host migration: retain step distance from the old neutral setting.
            (brightness - 7) * 16
        } else {
            if !(-256..=256).contains(&brightness) {
                return Err("brightness outside bounds".into());
            }
            brightness
        };
        let integer = |k, max| -> Result<usize, String> {
            let n = get(k)?
                .parse::<usize>()
                .map_err(|_| format!("invalid {k}"))?;
            if n > max {
                return Err(format!("{k} outside bounds"));
            }
            Ok(n)
        };
        let zoom = get("zoom")?.parse::<f32>().map_err(|_| "invalid zoom")?;
        if !zoom.is_finite() || !(0.5..=4.).contains(&zoom) {
            return Err("zoom outside bounds".into());
        }
        Ok(Self {
            zoom,
            rwr_range: integer("rwr-range", 4)?,
            radar_range: integer("radar-range", 4)?,
            radar_mode: integer("radar-mode", 2)?,
            selected: integer("selected", 5)?,
            small: boolean("small")?,
            large_pages: pages("large-pages", 4)?,
            small_pages: pages("small-pages", 6)?,
            cockpit: boolean("cockpit")?,
            hud: boolean("hud")?,
            ladder: boolean("ladder")?,
            brightness,
            music: boolean("music")?,
            effects: boolean("effects")?,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn saving_restores_both_layouts_and_display_choices() {
        let mut ui = FlightUi::default();
        let mut i = Instruments::default();
        let mut menu = State::new(vec![], true);
        ui.cockpit = false;
        ui.zoom = 1.7;
        menu.effects = false;
        i.pages = vec![9, 5];
        i.other_pages = vec![7, 8, 4];
        i.radar_range = 1;
        let saved = Preferences::capture(&ui, &i, &menu);
        let path = std::env::temp_dir().join(format!(
            "tore-prefs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        write(&path, &saved.text()).unwrap();
        let loaded = Preferences::parse(&read(&path).unwrap()).unwrap();
        std::fs::remove_file(path).unwrap();
        ui = FlightUi::default();
        i = Instruments::default();
        loaded.apply(&mut ui, &mut i, &mut menu);
        assert_eq!(i.pages, vec![9, 5]);
        i.toggle_layout();
        assert_eq!(i.pages, vec![7, 8, 4]);
        assert!(!ui.cockpit && !menu.effects);
        assert_eq!(ui.zoom, 1.7);
        assert_eq!(i.radar_range, 1);
    }
    #[test]
    fn roundtrip_layouts_and_reject_malformed() {
        let p = Preferences {
            zoom: 1.2,
            rwr_range: 3,
            radar_range: 2,
            radar_mode: 1,
            selected: 0,
            small: true,
            large_pages: vec![9, 5],
            small_pages: vec![],
            cockpit: false,
            hud: true,
            ladder: false,
            brightness: 3,
            music: false,
            effects: true,
        };
        assert_eq!(Preferences::parse(&p.text()).unwrap(), p);
        let legacy = p.text().replace("tore-preferences 2", "tore-preferences 1");
        assert_eq!(Preferences::parse(&legacy).unwrap().brightness, -64);
        assert_eq!(
            Preferences::parse(&legacy.replace("brightness 3", "brightness 7"))
                .unwrap()
                .brightness,
            0
        );
        assert!(Preferences::parse(&legacy.replace("brightness 3", "brightness 10")).is_err());
        assert!(Preferences::parse(&p.text().replace("brightness 3", "brightness 257")).is_err());
        assert!(
            Preferences::parse(&p.text().replace("large-pages 9,5", "large-pages 1,2,3,4,5"))
                .is_err()
        );
        assert!(Preferences::parse(&(p.text() + "music true\n")).is_err());
    }
}
