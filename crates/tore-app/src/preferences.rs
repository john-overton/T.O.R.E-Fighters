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
/// Nearest recovered scope setting to a saved nautical-mile value. An equal
/// distance keeps the lower setting; an old index is never reinterpreted.
fn nearest_range(nmi: f64) -> usize {
    let ladder = tore_sim::sensors::RANGE_LADDER_NMI;
    let mut best = 0;
    for index in 1..ladder.len() {
        if (ladder[index] - nmi).abs() < (ladder[best] - nmi).abs() {
            best = index;
        }
    }
    best
}
#[derive(Clone, Debug, PartialEq)]
pub struct Preferences {
    pub zoom: f32,
    pub radar_range: usize,
    pub rcs_range: usize,
    pub radar_channel: usize,
    pub radar_history: bool,
    pub selected: usize,
    pub small: bool,
    pub large_pages: Vec<u8>,
    pub small_pages: Vec<u8>,
    pub cockpit: bool,
    pub hud: bool,
    pub ladder: bool,
    /// The upper-right weapon diagnostic panel, off by default.
    pub weapon_diagnostics: bool,
    pub brightness: i16,
    pub music: bool,
    pub effects: bool,
    /// Borderless fullscreen, the default for an interactive start. A future
    /// Pref menu row hooks into this `fullscreen` field; `menu.rs` owns that
    /// row.
    // TODO: add a Pref row that toggles `Preferences::fullscreen`.
    pub fullscreen: bool,
}
impl Preferences {
    pub fn capture(ui: &FlightUi, i: &Instruments, m: &State, fullscreen: bool) -> Self {
        let (large, small) = if i.layout == Layout::Large {
            (&i.pages, &i.other_pages)
        } else {
            (&i.other_pages, &i.pages)
        };
        Self {
            zoom: ui.zoom,
            radar_range: i.radar_range,
            rcs_range: i.rcs_range,
            radar_channel: i.channel,
            radar_history: i.history,
            selected: i.selected,
            small: i.layout == Layout::Small,
            large_pages: large.clone(),
            small_pages: small.clone(),
            cockpit: ui.cockpit,
            hud: ui.hud,
            ladder: ui.ladder,
            weapon_diagnostics: ui.weapon_diagnostics,
            brightness: ui.brightness,
            music: m.music,
            effects: m.effects,
            fullscreen,
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
        i.radar_range = self.radar_range;
        i.rcs_range = self.rcs_range;
        i.channel = self.radar_channel;
        i.history = self.radar_history;
        i.selected = self.selected.min(i.pages.len().saturating_sub(1));
        i.pressed = None;
        i.cameras.clear();
        ui.cockpit = self.cockpit;
        ui.hud = self.hud;
        ui.ladder = self.ladder;
        ui.weapon_diagnostics = self.weapon_diagnostics;
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
            "tore-preferences 5\nzoom {}\nradar-range {}\nrcs-range {}\nradar-channel {}\nradar-history {}\nselected {}\nsmall {}\nlarge-pages {}\nsmall-pages {}\ncockpit {}\nhud {}\nladder {}\nweapon-diagnostics {}\nbrightness {}\nmusic {}\neffects {}\nfullscreen {}\n",
            self.zoom,
            self.radar_range,
            self.rcs_range,
            self.radar_channel,
            self.radar_history,
            self.selected,
            self.small,
            pages(&self.large_pages),
            pages(&self.small_pages),
            self.cockpit,
            self.hud,
            self.ladder,
            self.weapon_diagnostics,
            self.brightness,
            self.music,
            self.effects,
            self.fullscreen
        )
    }
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.len() > 16384 {
            return Err("preferences too large".into());
        }
        let mut values = std::collections::BTreeMap::new();
        let mut lines = text.lines();
        let version = match lines.next() {
            Some("tore-preferences 1") => 1,
            Some("tore-preferences 2") => 2,
            Some("tore-preferences 3") => 3,
            Some("tore-preferences 4") => 4,
            Some("tore-preferences 5") => 5,
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
        // Version 5 is unreleased; a file written before the diagnostics
        // field was added still loads, with the panel hidden.
        let diagnostics_saved = version >= 5 && values.contains_key("weapon-diagnostics");
        let expected = match version {
            1 | 2 => 14,
            3 => 16,
            4 => 17,
            _ => 16 + usize::from(diagnostics_saved),
        };
        if values.len() != expected {
            return Err("unknown preference".into());
        }
        let brightness = get("brightness")?
            .parse::<i16>()
            .map_err(|_| "invalid brightness")?;
        let brightness = if version == 1 {
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
        if version < 3 {
            // The retired cosmetic scope mode is still required to be present
            // and in bounds, so an older file is validated rather than guessed.
            integer("radar-mode", 2)?;
        }
        if version < 5 {
            // The retired separate RWR range is validated and dropped: the RWR
            // now follows the shared radar range, capped at 50 miles.
            integer("rwr-range", 4)?;
        }
        // Saved scope ranges migrate by their old nautical-mile value to the
        // nearest new setting, with equal distances choosing the lower one.
        let radar_range = if version < 3 {
            nearest_range([10., 20., 40., 80., 160.][integer("radar-range", 4)?])
        } else {
            integer("radar-range", tore_sim::sensors::RANGE_LADDER_NMI.len() - 1)?
        };
        Ok(Self {
            zoom,
            radar_range,
            rcs_range: if version < 3 {
                tore_sim::sensors::passive::DEFAULT_SCALE_INDEX
            } else {
                integer(
                    "rcs-range",
                    tore_sim::sensors::passive::SCALE_LADDER_NMI.len() - 1,
                )?
            },
            radar_channel: if version < 3 {
                0
            } else {
                integer("radar-channel", 1)?
            },
            radar_history: if version < 3 {
                false
            } else {
                boolean("radar-history")?
            },
            selected: integer("selected", 5)?,
            small: boolean("small")?,
            large_pages: pages("large-pages", 4)?,
            small_pages: pages("small-pages", 6)?,
            cockpit: boolean("cockpit")?,
            hud: boolean("hud")?,
            ladder: boolean("ladder")?,
            weapon_diagnostics: diagnostics_saved && boolean("weapon-diagnostics")?,
            brightness,
            music: boolean("music")?,
            effects: boolean("effects")?,
            // Files written before version 4 predate the window mode, and
            // borderless fullscreen is the default, so they start fullscreen.
            fullscreen: if version < 4 {
                true
            } else {
                boolean("fullscreen")?
            },
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
        ui.weapon_diagnostics = true;
        ui.zoom = 1.7;
        menu.effects = false;
        i.pages = vec![9, 5];
        i.other_pages = vec![7, 8, 4];
        i.radar_range = 1;
        let saved = Preferences::capture(&ui, &i, &menu, false);
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
        assert!(!ui.weapon_diagnostics);
        i = Instruments::default();
        loaded.apply(&mut ui, &mut i, &mut menu);
        assert!(ui.weapon_diagnostics);
        assert_eq!(i.pages, vec![9, 5]);
        i.toggle_layout();
        assert_eq!(i.pages, vec![7, 8, 4]);
        assert!(!ui.cockpit && !menu.effects);
        assert_eq!(ui.zoom, 1.7);
        assert_eq!(i.radar_range, 1);
        assert!(!loaded.fullscreen);
    }
    #[test]
    fn roundtrip_layouts_and_reject_malformed() {
        let p = Preferences {
            zoom: 1.2,
            radar_range: 2,
            rcs_range: 4,
            radar_channel: 1,
            radar_history: true,
            selected: 0,
            small: true,
            large_pages: vec![9, 5],
            small_pages: vec![],
            cockpit: false,
            hud: true,
            ladder: false,
            weapon_diagnostics: true,
            brightness: 3,
            music: false,
            effects: true,
            fullscreen: false,
        };
        assert_eq!(Preferences::parse(&p.text()).unwrap(), p);
        assert!(p.text().starts_with("tore-preferences 5\n"));
        assert!(!p.text().contains("rwr-range"));
        assert!(p.text().contains("\nweapon-diagnostics true\n"));
        let hidden = Preferences {
            weapon_diagnostics: false,
            ..p.clone()
        };
        assert_eq!(Preferences::parse(&hidden.text()).unwrap(), hidden);
        assert!(
            Preferences::parse(
                &p.text()
                    .replace("weapon-diagnostics true", "weapon-diagnostics 1")
            )
            .is_err()
        );
        // An unreleased version 5 file written before the field existed loads
        // with the panel hidden.
        let early = p.text().replace("weapon-diagnostics true\n", "");
        assert_eq!(Preferences::parse(&early).unwrap(), hidden);
        // A version 4 file still carries the retired RWR range, which is
        // validated and dropped, and predates the diagnostics field.
        let four = early
            .replace("tore-preferences 5", "tore-preferences 4")
            .replace("zoom 1.2\n", "zoom 1.2\nrwr-range 3\n");
        assert_eq!(Preferences::parse(&four).unwrap(), hidden);
        assert!(Preferences::parse(&(four.clone() + "weapon-diagnostics true\n")).is_err());
        let p = hidden;
        assert!(Preferences::parse(&four.replace("rwr-range 3", "rwr-range 5")).is_err());
        assert!(
            Preferences::parse(&p.text().replace("zoom 1.2\n", "zoom 1.2\nrwr-range 3\n")).is_err()
        );
        let on = Preferences {
            fullscreen: true,
            ..p.clone()
        };
        assert!(Preferences::parse(&on.text()).unwrap().fullscreen);
        // A version 3 file has no window mode, and borderless fullscreen is
        // the default, so it loads as fullscreen.
        let three = four
            .replace("tore-preferences 4", "tore-preferences 3")
            .replace("fullscreen false\n", "");
        let migrated3 = Preferences::parse(&three).unwrap();
        assert!(migrated3.fullscreen);
        assert_eq!(
            Preferences {
                fullscreen: false,
                ..migrated3
            },
            p
        );
        assert!(Preferences::parse(&(three + "fullscreen false\n")).is_err());
        // Earlier files keep loading: the retired scope mode is dropped and the
        // saved scope range migrates by its nautical-mile value.
        let old = "tore-preferences 2\nzoom 1.2\nrwr-range 3\nradar-range 2\nradar-mode 1\nselected 0\nsmall true\nlarge-pages 9,5\nsmall-pages -\ncockpit false\nhud true\nladder false\nbrightness 3\nmusic false\neffects true\n";
        let migrated = Preferences::parse(old).unwrap();
        assert_eq!(migrated.radar_range, 3);
        assert_eq!(migrated.radar_channel, 0);
        assert!(!migrated.radar_history);
        assert!(migrated.fullscreen);
        for (saved, expected) in [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)] {
            let text = old.replace("radar-range 2", &format!("radar-range {saved}"));
            assert_eq!(Preferences::parse(&text).unwrap().radar_range, expected);
        }
        let legacy = old.replace("tore-preferences 2", "tore-preferences 1");
        assert_eq!(Preferences::parse(&legacy).unwrap().brightness, -64);
        assert_eq!(
            Preferences::parse(&legacy.replace("brightness 3", "brightness 7"))
                .unwrap()
                .brightness,
            0
        );
        assert!(Preferences::parse(&legacy.replace("brightness 3", "brightness 10")).is_err());
        assert!(Preferences::parse(&old.replace("radar-mode 1", "radar-history true")).is_err());
        assert!(Preferences::parse(&p.text().replace("brightness 3", "brightness 257")).is_err());
        assert!(
            Preferences::parse(&p.text().replace("large-pages 9,5", "large-pages 1,2,3,4,5"))
                .is_err()
        );
        assert!(Preferences::parse(&(p.text() + "music true\n")).is_err());
    }
}
