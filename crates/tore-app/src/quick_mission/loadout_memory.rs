//! The custom loadout the player flew, kept for the mission it was made for
//! (slice BQ2, John 2026-10-12, item 19).
//!
//! The Load Ordnance page already keeps the player's edits for the rest of a
//! session. This keeps them across a restart, but only under John's rule: the
//! loadout comes back when the mission has not changed since it was made, or
//! when a mission that was flown before is flown again. Anything else opens
//! the page on the aircraft's standard load, as before.
//!
//! "The same mission" is [`QuickMission::mission_fingerprint`]: the theater,
//! every aircraft, every other creator choice and the group objectives and
//! survival flags. The file `quick-loadouts-v1.conf` in the profile holds the
//! loadouts of the last [`KEPT`] missions flown with a custom load, newest
//! last, each under its fingerprint (agent decision: a short history is what
//! lets "fly a mission again" work after other missions have been flown in
//! between; the number is a guess at a few weeks of play). A load is written
//! when the player presses Fly on the page, so it is a loadout that really
//! flew, not an edit abandoned with Cancel; it is not written for Standard
//! load, which never opens the page.
//!
//! Safety, as for the draft: a file this build cannot read or whose version
//! is newer is left exactly as it is (nothing restored, nothing written), and
//! a saved load that no longer fits the aircraft (an import dropped a weapon,
//! a station changed) falls back to the standard load.
//!
//! Runs that skip the saved draft skip this too (the app opens no memory for
//! them): snapshots, probes, scripted runs, the smoke test and the creator
//! setting flags. The multiplayer lobby's page is a separate path and keeps
//! its own rules.

use std::path::PathBuf;
use tore_world::mission::{LoadoutSpec, StationLoad, TankLoad};

/// The file's first line. A file with another version is left alone.
const HEADER: &str = "tore-quick-loadouts 1";
/// How many missions' loadouts are kept; the oldest goes first.
pub const KEPT: usize = 8;

/// One mission's loadout.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub fingerprint: String,
    pub load: LoadoutSpec,
}

/// What the ordnance page should open on.
#[derive(Debug, PartialEq)]
pub enum Choice {
    /// The page's own load from earlier in this session.
    Session,
    /// A load kept in the profile for this mission.
    Remembered(LoadoutSpec),
    /// The aircraft's standard load.
    Standard,
}

/// A mission's lines read so far.
struct Building {
    fingerprint: String,
    fuel: Option<f64>,
    cheat: bool,
    tanks: Vec<TankLoad>,
    stations: Vec<Option<StationLoad>>,
}
impl Building {
    fn new(fingerprint: &str) -> Self {
        Self {
            fingerprint: fingerprint.into(),
            fuel: None,
            cheat: false,
            tanks: Vec::new(),
            stations: Vec::new(),
        }
    }
    fn finish(self, entries: &mut Vec<Entry>) -> Result<(), String> {
        let name = self.fingerprint;
        let fuel_lbs = self
            .fuel
            .ok_or_else(|| format!("mission {name} has no fuel line"))?;
        let stations = self
            .stations
            .into_iter()
            .enumerate()
            .map(|(at, s)| s.ok_or_else(|| format!("mission {name} lacks station {at}")))
            .collect::<Result<Vec<_>, _>>()?;
        if entries.iter().any(|e| e.fingerprint == name) {
            return Err(format!("mission {name} appears twice"));
        }
        entries.push(Entry {
            fingerprint: name,
            load: LoadoutSpec {
                fuel_lbs,
                cheat: self.cheat,
                stations,
                tanks: Some(self.tanks),
            },
        });
        Ok(())
    }
}

fn parse_entries(text: &str) -> Result<Vec<Entry>, String> {
    let mut lines = text.lines();
    match lines.next() {
        Some(HEADER) => {}
        Some(other) => return Err(format!("unknown quick loadout header {other:?}")),
        None => return Err("empty quick loadout file".into()),
    }
    let mut entries: Vec<Entry> = Vec::new();
    let mut open: Option<Building> = None;
    for line in lines.filter(|l| !l.trim().is_empty()) {
        let words: Vec<&str> = line.split_whitespace().collect();
        let number = |word: &str| {
            word.parse::<usize>()
                .map_err(|_| format!("bad number {word:?} in {line:?}"))
        };
        let small = |word: &str| {
            word.parse::<u16>()
                .map_err(|_| format!("bad count {word:?} in {line:?}"))
        };
        if let ["mission", fingerprint] = words.as_slice() {
            if let Some(done) = open.replace(Building::new(fingerprint)) {
                done.finish(&mut entries)?;
            }
            continue;
        }
        let Some(Building {
            fuel,
            cheat,
            tanks,
            stations,
            ..
        }) = open.as_mut()
        else {
            return Err(format!("{line:?} comes before any mission line"));
        };
        match words.as_slice() {
            ["fuel", pounds] => {
                let pounds: f64 = pounds
                    .parse()
                    .map_err(|_| format!("bad fuel {pounds:?} in {line:?}"))?;
                if !pounds.is_finite() || pounds < 0. || fuel.replace(pounds).is_some() {
                    return Err(format!("bad or repeated fuel line {line:?}"));
                }
            }
            ["cheat", value] => {
                *cheat = match *value {
                    "yes" => true,
                    "no" => false,
                    other => return Err(format!("bad cheat word {other:?}")),
                }
            }
            ["tank", hardpoint, name, quantity] => {
                let hardpoint = u8::try_from(number(hardpoint)?)
                    .map_err(|_| format!("bad tank hardpoint in {line:?}"))?;
                if tanks.iter().any(|t| t.hardpoint == hardpoint) {
                    return Err(format!("repeated tank hardpoint in {line:?}"));
                }
                tanks.push(TankLoad {
                    hardpoint,
                    tank: (*name).into(),
                    quantity: small(quantity)?,
                });
            }
            ["station", at, weapon, count, quantity] => {
                let at = number(at)?;
                if at >= 64 {
                    return Err(format!("station {at} is out of range"));
                }
                if stations.len() <= at {
                    stations.resize(at + 1, None);
                }
                if stations[at].is_some() {
                    return Err(format!("repeated station line {line:?}"));
                }
                stations[at] = Some(StationLoad {
                    weapon: (*weapon).into(),
                    count: small(count)?,
                    quantity: small(quantity)?,
                });
            }
            _ => return Err(format!("unrecognised line {line:?}")),
        }
    }
    if let Some(done) = open {
        done.finish(&mut entries)?;
    }
    if entries.len() > KEPT {
        return Err(format!("more than {KEPT} missions in the file"));
    }
    Ok(entries)
}

fn entries_text(entries: &[Entry]) -> String {
    let mut text = format!("{HEADER}\n");
    for entry in entries {
        text += &format!("mission {}\n", entry.fingerprint);
        text += &format!("fuel {}\n", entry.load.fuel_lbs);
        text += &format!("cheat {}\n", if entry.load.cheat { "yes" } else { "no" });
        for tank in entry.load.tanks.iter().flatten() {
            text += &format!("tank {} {} {}\n", tank.hardpoint, tank.tank, tank.quantity);
        }
        for (at, station) in entry.load.stations.iter().enumerate() {
            text += &format!(
                "station {at} {} {} {}\n",
                station.weapon, station.count, station.quantity
            );
        }
    }
    text
}

/// A load as the profile's file keeps it and reads it back; the ordnance
/// audit checks every aircraft's loads against this.
pub fn through_the_file(load: &LoadoutSpec) -> Result<LoadoutSpec, String> {
    let text = entries_text(&[Entry {
        fingerprint: "0".into(),
        load: load.clone(),
    }]);
    let mut back = parse_entries(&text)?;
    Ok(back.remove(0).load)
}

/// The profile's loadout file and what it holds.
pub struct LoadoutMemory {
    path: PathBuf,
    /// Oldest first.
    entries: Vec<Entry>,
    /// What the file held when last read or written.
    saved: String,
    /// The file could not be read or is newer than this build: it is never
    /// used and never overwritten.
    locked: bool,
    /// The fingerprint the page in this session was last opened (or flown)
    /// for. Its loads are the session's own.
    shown: Option<String>,
}

impl LoadoutMemory {
    /// Reads the profile's file. A missing file is an empty memory; an
    /// unreadable one is left alone.
    pub fn open(path: PathBuf) -> Self {
        let (entries, saved, locked) = match crate::preferences::read(&path) {
            Ok(text) => match parse_entries(&text) {
                Ok(entries) => (entries, text, false),
                Err(error) => {
                    log::warn!("Quick Mission loadouts not loaded: {error}; preserving the file");
                    (Vec::new(), String::new(), true)
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                (Vec::new(), String::new(), false)
            }
            Err(error) => {
                log::warn!("Quick Mission loadouts not loaded: {error}; preserving the file");
                (Vec::new(), String::new(), true)
            }
        };
        Self {
            path,
            entries,
            saved,
            locked,
            shown: None,
        }
    }

    /// What the page should open on for the mission `fingerprint`.
    /// `session_has_one` says the page already holds a load for the same
    /// aircraft from earlier in the session.
    ///
    /// 1. The page last opened for this very mission and still holds its
    ///    load (edits not flown yet included): keep it.
    /// 2. Else a load flown for this mission before: restore it.
    /// 3. Else a load from earlier in the session, as the page has always
    ///    done when only other choices changed: keep it.
    /// 4. Else the standard load.
    pub fn choose(&self, fingerprint: &str, session_has_one: bool) -> Choice {
        if session_has_one && self.shown.as_deref() == Some(fingerprint) {
            return Choice::Session;
        }
        if let Some(entry) = self
            .entries
            .iter()
            .find(|e| !self.locked && e.fingerprint == fingerprint)
        {
            return Choice::Remembered(entry.load.clone());
        }
        if session_has_one {
            Choice::Session
        } else {
            Choice::Standard
        }
    }

    /// The page opened for the mission `fingerprint`.
    pub fn opened_for(&mut self, fingerprint: &str) {
        self.shown = Some(fingerprint.to_owned());
    }

    /// The load flown for the mission `fingerprint`. Replaces that mission's
    /// earlier entry, drops the oldest past [`KEPT`], and writes the file
    /// when it changed. A failure is logged; the next Fly tries again.
    pub fn flown(&mut self, fingerprint: &str, load: LoadoutSpec) {
        self.shown = Some(fingerprint.to_owned());
        if self.locked {
            return;
        }
        self.entries.retain(|e| e.fingerprint != fingerprint);
        self.entries.push(Entry {
            fingerprint: fingerprint.to_owned(),
            load,
        });
        while self.entries.len() > KEPT {
            self.entries.remove(0);
        }
        let text = entries_text(&self.entries);
        if text == self.saved {
            return;
        }
        match crate::preferences::write(&self.path, &text) {
            Ok(()) => self.saved = text,
            Err(error) => log::warn!("Quick Mission loadouts not saved: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::full_catalog;
    use super::*;

    fn load(seed: u16) -> LoadoutSpec {
        LoadoutSpec {
            fuel_lbs: 6000.5 + f64::from(seed),
            cheat: seed % 2 == 1,
            stations: vec![
                StationLoad {
                    weapon: "AIM9M.JT".into(),
                    count: 2,
                    quantity: seed,
                },
                StationLoad {
                    weapon: "GUN.JT".into(),
                    count: 0,
                    quantity: 0,
                },
            ],
            tanks: Some(vec![TankLoad {
                hardpoint: 3,
                tank: "TANK370.GAS".into(),
                quantity: 1,
            }]),
        }
    }
    fn unique_file() -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        std::env::temp_dir()
            .join(format!(
                "tore-bq2-{}-{}",
                std::process::id(),
                COUNT.fetch_add(1, Ordering::Relaxed)
            ))
            .join("quick-loadouts-v1.conf")
    }
    fn cleanup(path: &std::path::Path) {
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn a_loadout_round_trips_through_the_text_exactly() {
        let entries = vec![
            Entry {
                fingerprint: "00ff".into(),
                load: load(3),
            },
            Entry {
                fingerprint: "1234".into(),
                load: load(4),
            },
        ];
        let text = entries_text(&entries);
        assert_eq!(parse_entries(&text).unwrap(), entries);
        assert_eq!(entries_text(&parse_entries(&text).unwrap()), text);
    }

    #[test]
    fn a_file_that_does_not_parse_is_refused() {
        let good = entries_text(&[Entry {
            fingerprint: "00ff".into(),
            load: load(3),
        }]);
        for bad in [
            String::new(),
            "tore-quick-loadouts 2\n".to_string(),
            good.replace(HEADER, "other"),
            format!("{HEADER}\nfuel 10\n"),
            format!("{good}mystery 1\n"),
            format!("{good}fuel 9\n"),
            format!("{good}station 0 X.JT 1 1\n"),
            format!("{good}station 99 X.JT 1 1\n"),
            format!("{good}tank 3 TANK370.GAS 1\n"),
            format!("{good}cheat maybe\n"),
            good.replace("fuel 6003.5", "fuel nan"),
            good.replace("fuel 6003.5", "fuel -5"),
            good.replace("station 0 ", "station 1 "),
            format!("{good}{good}").replacen(HEADER, "", 1),
            good.replace("station 0 AIM9M.JT 2 3\n", ""),
        ] {
            assert!(parse_entries(&bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_same_mission_gets_its_loadout_back_after_a_restart() {
        let path = unique_file();
        let mut first = LoadoutMemory::open(path.clone());
        assert_eq!(first.choose("aaaa", false), Choice::Standard);
        first.flown("aaaa", load(3));
        let second = LoadoutMemory::open(path.clone());
        assert_eq!(second.choose("aaaa", false), Choice::Remembered(load(3)));
        cleanup(&path);
    }

    #[test]
    fn a_changed_mission_gets_the_standard_load() {
        let path = unique_file();
        let mut first = LoadoutMemory::open(path.clone());
        first.flown("aaaa", load(3));
        let second = LoadoutMemory::open(path.clone());
        assert_eq!(second.choose("bbbb", false), Choice::Standard);
        cleanup(&path);
    }

    #[test]
    fn any_changed_field_of_the_draft_changes_the_fingerprint() {
        let base = full_catalog();
        let same = base.mission_fingerprint();
        assert_eq!(full_catalog().mission_fingerprint(), same);
        // Every list choice but field 18 is part of the mission.
        for field in (3..super::super::FIELDS)
            .filter(|f| *f != 6 && *f != 13 && *f != 18)
            .filter(|f| base.values(*f).len() > 1)
        {
            let mut q = full_catalog();
            let next = (q.draft.values[field] + 1) % q.values(field).len();
            q.draft.values[field] = next;
            assert_ne!(q.mission_fingerprint(), same, "field {field}");
        }
        // The aircraft, the theater, the objectives and the survival flags.
        let mut q = full_catalog();
        q.draft.values[6] = (q.draft.values[6] + 1) % q.aircraft_files.len();
        assert_ne!(q.mission_fingerprint(), same, "player aircraft");
        let mut q = full_catalog();
        q.theater(5);
        assert_ne!(q.mission_fingerprint(), same, "theater");
        let mut q = full_catalog();
        q.group_must_survive[2] = !q.group_must_survive[2];
        assert_ne!(q.mission_fingerprint(), same, "survival");
        let mut q = full_catalog();
        q.group_objectives[4] = tore_sim::ai::engagement::GroupObjective::Cap;
        assert_ne!(q.mission_fingerprint(), same, "objective");
        // Standard or Custom load is not part of the mission.
        let mut q = full_catalog();
        q.draft.values[18] = (q.draft.values[18] + 1) % q.values(18).len();
        assert_eq!(q.mission_fingerprint(), same, "field 18");
    }

    #[test]
    fn flying_a_mission_again_after_others_restores_its_load() {
        let path = unique_file();
        let mut memory = LoadoutMemory::open(path.clone());
        memory.flown("aaaa", load(1));
        memory.flown("bbbb", load(2));
        memory.flown("cccc", load(4));
        // Back to the first mission, in the same session and after a restart.
        assert_eq!(memory.choose("aaaa", true), Choice::Remembered(load(1)));
        let again = LoadoutMemory::open(path.clone());
        assert_eq!(again.choose("aaaa", false), Choice::Remembered(load(1)));
        assert_eq!(again.choose("bbbb", false), Choice::Remembered(load(2)));
        cleanup(&path);
    }

    #[test]
    fn the_oldest_mission_is_forgotten_past_the_limit() {
        let path = unique_file();
        let mut memory = LoadoutMemory::open(path.clone());
        for n in 0..=KEPT {
            memory.flown(&format!("{n:04x}"), load(n as u16));
        }
        let again = LoadoutMemory::open(path.clone());
        assert_eq!(again.choose("0000", false), Choice::Standard);
        assert_eq!(again.choose("0001", false), Choice::Remembered(load(1)));
        assert_eq!(
            again.choose(&format!("{KEPT:04x}"), false),
            Choice::Remembered(load(KEPT as u16))
        );
        // Flying a kept mission again replaces its entry, not adds one.
        let mut memory = again;
        memory.flown("0001", load(9));
        assert_eq!(
            LoadoutMemory::open(path.clone()).entries.len(),
            KEPT,
            "still at the limit"
        );
        assert_eq!(memory.choose("0001", false), Choice::Remembered(load(9)));
        cleanup(&path);
    }

    #[test]
    fn edits_not_flown_yet_beat_the_file_in_the_same_session() {
        let path = unique_file();
        let mut memory = LoadoutMemory::open(path.clone());
        memory.flown("aaaa", load(1));
        // The page opened for the same mission and was edited, then Cancel:
        // reopening keeps the edits, not the last flown load.
        memory.opened_for("aaaa");
        assert_eq!(memory.choose("aaaa", true), Choice::Session);
        // Another mission with no entry keeps the session's load, as the
        // page always did when only other choices changed.
        assert_eq!(memory.choose("zzzz", true), Choice::Session);
        // But a mission flown before gets its own load.
        memory.flown("bbbb", load(2));
        memory.opened_for("bbbb");
        assert_eq!(memory.choose("aaaa", true), Choice::Remembered(load(1)));
        // A different aircraft's session load is never reused.
        assert_eq!(memory.choose("zzzz", false), Choice::Standard);
        cleanup(&path);
    }

    #[test]
    fn an_unreadable_or_newer_file_is_left_alone() {
        for text in [
            "tore-quick-loadouts 9\nfuture 1\n",
            "garbage",
            "tore-quick-loadouts 1\nstation 0 X.JT x 1\n",
        ] {
            let path = unique_file();
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            let mut memory = LoadoutMemory::open(path.clone());
            assert_eq!(memory.choose("aaaa", false), Choice::Standard);
            memory.flown("aaaa", load(3));
            assert_eq!(memory.choose("aaaa", false), Choice::Standard);
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
            cleanup(&path);
        }
    }
}
