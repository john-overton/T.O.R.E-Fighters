//! What the Quick Mission creator keeps between sessions, and how it opens
//! the first time (slice BQ, B22).
//!
//! **First load** is the first time the creator opens in a profile that has
//! no saved draft: each aircraft slot then takes a random aircraft from its
//! own list (the player's slot from every aircraft the game has, the AI's
//! five from what the AI can fly). The draft is written to the profile as
//! `quick-mission-v1.conf` after every menu action, so the next session
//! opens on the same mission, and the random pick is made once, not on every
//! launch (agent decision, 2026-10-12; John asked for the randomizing and the
//! persistence, not for these two rules).
//!
//! The pick is a menu choice, not part of the simulation: it comes from the
//! clock and nothing in a mission reads it. Once the mission is built it is
//! a fixed `MissionSpec`, so a mission stays deterministic. Aircraft are
//! stored by their selection key, never by list position, so a changed
//! import cannot turn one aircraft into another; every other choice is its
//! list position, checked against the list it indexes.
//!
//! Not kept: the ground target's seed (each flight draws its own), the
//! loadout on the ordnance page (it is rebuilt for the aircraft; it is kept
//! for the rest of the session), the AI mission preset (a command-line
//! choice) and anything the multiplayer lobby's creator holds.

use super::{AI_AIRCRAFT_FIELDS, FIELDS, OBJECTIVE_COUNT, QuickMission, fresh_seed};
use std::collections::BTreeMap;
use std::path::PathBuf;
use tore_formats::aircraft::AircraftId;

/// The file's first line. A file with another version is left alone.
const HEADER: &str = "tore-quick-draft 1";
/// The draft fields saved by position: the retail setup's, from 3 on, less
/// the theater (13, saved by code) and the six aircraft fields.
fn positional(id: usize) -> bool {
    (3..FIELDS).contains(&id) && id != 6 && id != 13 && !AI_AIRCRAFT_FIELDS.contains(&id)
}

/// A draft file read but not yet put to use.
#[derive(Debug, Default, PartialEq, Eq)]
struct File {
    theater: Option<String>,
    player: Option<String>,
    wings: BTreeMap<usize, String>,
    fields: BTreeMap<usize, usize>,
    objectives: BTreeMap<usize, usize>,
    survive: BTreeMap<usize, bool>,
}

impl File {
    fn parse(text: &str) -> Result<Self, String> {
        let mut lines = text.lines();
        match lines.next() {
            Some(HEADER) => {}
            Some(other) => return Err(format!("unknown quick mission draft header {other:?}")),
            None => return Err("empty quick mission draft".into()),
        }
        let mut draft = Self::default();
        for line in lines.filter(|l| !l.trim().is_empty()) {
            let words: Vec<&str> = line.split_whitespace().collect();
            let number = |word: &str| {
                word.parse::<usize>()
                    .map_err(|_| format!("bad number {word:?} in {line:?}"))
            };
            let fresh = match words.as_slice() {
                ["theater", code] => draft.theater.replace((*code).into()).is_none(),
                ["player", key] => draft.player.replace((*key).into()).is_none(),
                ["wing", field, key] => {
                    let field = number(field)?;
                    if !AI_AIRCRAFT_FIELDS.contains(&field) {
                        return Err(format!("field {field} is not a wing aircraft"));
                    }
                    draft.wings.insert(field, (*key).into()).is_none()
                }
                ["field", field, value] => {
                    let field = number(field)?;
                    if !positional(field) {
                        return Err(format!("field {field} is not a saved choice"));
                    }
                    draft.fields.insert(field, number(value)?).is_none()
                }
                ["objective", group, value] => {
                    let group = number(group)?;
                    if group >= OBJECTIVE_COUNT {
                        return Err(format!("group {group} is out of range"));
                    }
                    draft.objectives.insert(group, number(value)?).is_none()
                }
                ["survive", group, value] => {
                    let group = number(group)?;
                    if group >= OBJECTIVE_COUNT {
                        return Err(format!("group {group} is out of range"));
                    }
                    let value = match *value {
                        "yes" => true,
                        "no" => false,
                        other => return Err(format!("bad survival word {other:?}")),
                    };
                    draft.survive.insert(group, value).is_none()
                }
                _ => return Err(format!("unrecognised line {line:?}")),
            };
            if !fresh {
                return Err(format!("repeated line {line:?}"));
            }
        }
        Ok(draft)
    }
}

/// A small, fast, seeded mixer for the menu's random picks. Not the
/// simulation's generator; nothing in a mission reads it.
struct Picks(u64);
impl Picks {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) % below.max(1) as u64) as usize
    }
}

impl QuickMission {
    /// Gives every aircraft slot a random aircraft from its own list: the
    /// player's slot from the whole catalog (unless `keep_player`, as when
    /// `--aircraft` named one), the five AI slots from what the AI can fly.
    /// `seed` makes the pick repeatable, for tests and previews.
    pub fn randomize_aircraft(&mut self, seed: u64, keep_player: bool) {
        let mut picks = Picks(seed);
        if !keep_player && !self.aircraft_files.is_empty() {
            self.draft.values[6] = picks.next(self.aircraft_files.len());
        }
        if !self.wing_files.is_empty() {
            for field in AI_AIRCRAFT_FIELDS {
                self.draft.values[field] = picks.next(self.wing_files.len());
            }
        }
        // The same tidy-up a click on the aircraft would do (a pilot the AI
        // cannot fly leads wing 1 alone); the opening says nothing about it.
        self.apply(6, self.draft.values[6]);
        self.notice = None;
    }
    /// The player flies `id` (a command-line choice over a saved draft).
    pub fn choose_player_aircraft(&mut self, id: AircraftId) {
        if let Some(at) = self
            .aircraft_files
            .iter()
            .position(|key| key == id.selection_key())
        {
            self.apply(6, at);
            self.notice = None;
        }
    }
    /// The draft as the file keeps it.
    pub fn remembered(&self) -> String {
        let mut text = format!("{HEADER}\n");
        if let Some(code) = self.theater_codes.get(self.draft.values[13]) {
            text += &format!("theater {code}\n");
        }
        if let Some(key) = self.aircraft_files.get(self.draft.values[6]) {
            text += &format!("player {key}\n");
        }
        for field in AI_AIRCRAFT_FIELDS {
            if let Some(key) = self.wing_files.get(self.draft.values[field]) {
                text += &format!("wing {field} {key}\n");
            }
        }
        for field in (3..FIELDS).filter(|id| positional(*id)) {
            text += &format!("field {field} {}\n", self.draft.values[field]);
        }
        for group in 0..OBJECTIVE_COUNT {
            if let Some(at) = Self::objective_choices(group)
                .iter()
                .position(|(_, objective)| *objective == self.group_objectives[group])
            {
                text += &format!("objective {group} {at}\n");
            }
            text += &format!(
                "survive {group} {}\n",
                if self.group_must_survive[group] {
                    "yes"
                } else {
                    "no"
                }
            );
        }
        text
    }
    /// Puts a saved draft into the creator. A file that does not parse
    /// changes nothing. A choice the lists no longer have (an aircraft the
    /// import dropped, a position past the end of a list) keeps the creator's
    /// own value for it.
    pub fn recall(&mut self, text: &str) -> Result<(), String> {
        let saved = File::parse(text)?;
        if let Some(code) = &saved.theater
            && self.theater_codes.contains(code)
            && let Some(index) = self.theater_catalog.iter().position(|c| c == code)
        {
            self.theater(index);
        }
        for (field, at) in &saved.fields {
            if *at < self.values(*field).len() {
                self.draft.values[*field] = *at;
            }
        }
        if let Some(at) = saved
            .player
            .and_then(|key| self.aircraft_files.iter().position(|k| *k == key))
        {
            self.draft.values[6] = at;
        }
        for (field, key) in &saved.wings {
            if let Some(at) = self.wing_files.iter().position(|k| k == key) {
                self.draft.values[*field] = at;
            }
        }
        for (group, at) in &saved.objectives {
            if let Some((_, objective)) = Self::objective_choices(*group).get(*at) {
                self.group_objectives[*group] = *objective;
            }
        }
        for (group, keep) in &saved.survive {
            self.group_must_survive[*group] = *keep;
        }
        let player = self.draft.values[6];
        self.apply(6, player);
        self.notice = None;
        Ok(())
    }
    /// Whether the creator's draft is the player's own, to keep: not the
    /// lobby's, and not another player's mission read-only.
    fn keeps_draft(&self) -> bool {
        !self.lobby && self.view.is_none()
    }
}

/// The saved draft's file and what it held when last written.
pub struct Memory {
    path: PathBuf,
    saved: String,
    /// The draft came from the file, not from a first load.
    pub recalled: bool,
}

impl Memory {
    /// Opens the creator the way the profile left it: on the saved draft, or
    /// on a first load's random aircraft when there is none. `None` (and the
    /// creator as it was) when the file cannot be read or is not one this
    /// build understands; the file is then left alone, never overwritten.
    pub fn open(quick: &mut QuickMission, path: PathBuf, keep_player: bool) -> Option<Self> {
        match crate::preferences::read(&path) {
            Ok(text) => match quick.recall(&text) {
                Ok(()) => Some(Self {
                    saved: quick.remembered(),
                    path,
                    recalled: true,
                }),
                Err(error) => {
                    log::warn!("Quick Mission draft not loaded: {error}; preserving the file");
                    None
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                quick.randomize_aircraft(
                    u64::from(fresh_seed()) << 32 | u64::from(fresh_seed()),
                    keep_player,
                );
                Some(Self {
                    path,
                    saved: String::new(),
                    recalled: false,
                })
            }
            Err(error) => {
                log::warn!("Quick Mission draft not loaded: {error}; preserving the file");
                None
            }
        }
    }
    /// Writes the draft when it changed. A failure is logged and tried again
    /// at the next change.
    pub fn save(&mut self, quick: &QuickMission) {
        if !quick.keeps_draft() {
            return;
        }
        let text = quick.remembered();
        if text == self.saved {
            return;
        }
        match crate::preferences::write(&self.path, &text) {
            Ok(()) => self.saved = text,
            Err(error) => log::warn!("Quick Mission draft not saved: {error}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{full_catalog, setup};
    use super::*;
    use tore_sim::ai::{
        engagement::GroupObjective,
        launch::{Side, WingId},
    };

    /// A creator with a spread of choices made, none of them a default.
    fn busy() -> QuickMission {
        let mut q = full_catalog();
        q.theater(5);
        q.draft.values[4] = 3;
        q.draft.values[5] = 2;
        q.draft.values[14] = 2;
        q.draft.values[17] = 4;
        q.draft.values[19] = 2;
        q.draft.values[6] = 4;
        for (n, field) in AI_AIRCRAFT_FIELDS.into_iter().enumerate() {
            q.draft.values[field] = 2 + n;
        }
        q.group_objectives[0] = GroupObjective::Intercept(WingId::new(Side::Enemy, 1).unwrap());
        q.group_objectives[4] = GroupObjective::Cap;
        q.group_must_survive[2] = true;
        q
    }
    fn unique_file(name: &str) -> PathBuf {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNT: AtomicUsize = AtomicUsize::new(0);
        std::env::temp_dir()
            .join(format!(
                "tore-bq-{}-{}",
                std::process::id(),
                COUNT.fetch_add(1, Ordering::Relaxed)
            ))
            .join(name)
    }

    #[test]
    fn a_draft_comes_back_from_its_file_as_the_same_mission() {
        let q = busy();
        let text = q.remembered();
        let mut back = full_catalog();
        back.recall(&text).unwrap();
        assert_eq!(back.draft, q.draft);
        assert_eq!(back.group_objectives, q.group_objectives);
        assert_eq!(back.group_must_survive, q.group_must_survive);
        assert_eq!(back.remembered(), text, "writing it again changes nothing");
        // Aircraft go by key, not by place.
        assert!(text.contains(&format!("player {}", q.aircraft_files[4])));
        assert!(text.contains("theater "));
    }

    #[test]
    fn a_file_that_does_not_parse_changes_nothing() {
        let good = busy().remembered();
        for bad in [
            String::new(),
            "tore-quick-draft 2\n".to_string(),
            good.replace("tore-quick-draft 1", "something else"),
            format!("{good}field 4 1\n"),
            format!("{good}field 6 1\n"),
            format!("{good}field 13 1\n"),
            format!("{good}wing 6 F18.PT\n"),
            format!("{good}objective 6 0\n"),
            format!("{good}survive 0 maybe\n"),
            format!("{good}mystery 1\n"),
            format!("{good}field 99 1\n"),
            format!("{good}field 7 x\n"),
        ] {
            let mut q = full_catalog();
            let before = (q.draft.clone(), q.group_objectives, q.group_must_survive);
            assert!(q.recall(&bad).is_err(), "{bad:?}");
            assert_eq!(
                (q.draft.clone(), q.group_objectives, q.group_must_survive),
                before,
                "{bad:?}"
            );
        }
    }

    #[test]
    fn a_choice_the_lists_no_longer_have_keeps_the_creators_own() {
        let mut q = full_catalog();
        let before = q.draft.clone();
        let text = "tore-quick-draft 1\ntheater ZZZ\nplayer NOSUCH.PT\nwing 9 NOSUCH.PT\nfield 7 9999\nobjective 0 999\n";
        q.recall(text).unwrap();
        assert_eq!(q.draft, before);
        assert_eq!(q.group_objectives[0], GroupObjective::Inherit);
    }

    #[test]
    fn the_ai_slots_of_a_saved_draft_resolve_against_the_ais_own_list() {
        // A key the player may fly but the AI cannot is not taken by a wing.
        let mut q = full_catalog();
        let unflyable = q
            .aircraft_files
            .iter()
            .find(|key| !q.wing_files.contains(key))
            .cloned();
        if let Some(key) = unflyable {
            let before = q.draft.values[9];
            q.recall(&format!("tore-quick-draft 1\nwing 9 {key}\n"))
                .unwrap();
            assert_eq!(q.draft.values[9], before);
        }
    }

    #[test]
    fn randomizing_picks_only_aircraft_each_slot_may_have() {
        let mut spread = vec![std::collections::BTreeSet::new(); 6];
        for seed in 0..300u64 {
            let mut q = full_catalog();
            q.randomize_aircraft(seed.wrapping_mul(0x1234_5678_9abc_def1), false);
            let player = &q.aircraft_files[q.draft.values[6]];
            assert!(AircraftId::parse(player).is_ok(), "{player}");
            spread[0].insert(q.draft.values[6]);
            for (n, field) in AI_AIRCRAFT_FIELDS.into_iter().enumerate() {
                let key = &q.wing_files[q.draft.values[field]];
                let id = AircraftId::parse(key).unwrap();
                assert!(id.ai_flyable(), "{key} is not AI-flyable");
                spread[n + 1].insert(q.draft.values[field]);
            }
            assert!(q.notice.is_none(), "the opening says nothing");
        }
        for (slot, seen) in spread.iter().enumerate() {
            assert!(seen.len() > 3, "slot {slot} varied too little: {seen:?}");
        }
    }

    #[test]
    fn randomizing_repeats_for_a_seed_and_can_leave_the_players_slot() {
        let make = |seed, keep| {
            let mut q = full_catalog();
            q.draft.values[6] = 1;
            q.randomize_aircraft(seed, keep);
            q.draft
        };
        assert_eq!(make(7, false), make(7, false));
        assert_ne!(make(7, false), make(8, false));
        assert_eq!(make(7, true).values[6], 1);
        assert_eq!(make(99, true).values[6], 1);
    }

    #[test]
    fn a_pilot_the_ai_cannot_fly_still_opens_with_wing_one_just_him() {
        // Whatever the player's slot draws, the creator stays valid: wing 1
        // is alone when the AI cannot fly the pilot's aircraft.
        let mut q = full_catalog();
        let hand = q
            .aircraft_files
            .iter()
            .position(|key| AircraftId::parse(key).is_ok_and(|id| !id.ai_flyable()));
        if let Some(at) = hand {
            q.draft.values[4] = 3;
            q.apply(6, at);
            q.notice = None;
            q.randomize_aircraft(1, true);
            assert_eq!(q.draft.values[4], 1);
            assert!(q.notice.is_none());
        }
    }

    #[test]
    fn the_first_load_randomizes_and_later_ones_keep_the_saved_draft() {
        let path = unique_file("quick-mission-v1.conf");
        let mut first = setup();
        let mut memory = Memory::open(&mut first, path.clone(), false).expect("a first load");
        assert!(!memory.recalled);
        assert!(!path.exists(), "nothing is written before a change");
        memory.save(&first);
        assert!(path.exists());
        // The next session opens on exactly that draft, whatever it would draw.
        let mut second = setup();
        let memory = Memory::open(&mut second, path.clone(), false).expect("the saved draft");
        assert!(memory.recalled);
        assert_eq!(second.draft, first.draft);
        // A change the player makes after the end of a flight or back from
        // the ordnance page is saved the same way.
        let mut third = setup();
        let mut memory = Memory::open(&mut third, path.clone(), false).unwrap();
        third.apply(5, 3);
        memory.save(&third);
        let mut fourth = setup();
        Memory::open(&mut fourth, path.clone(), false).unwrap();
        assert_eq!(fourth.draft.values[5], 3);
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn an_unreadable_or_newer_file_is_left_alone() {
        for text in [
            "tore-quick-draft 9\nfuture 1\n",
            "garbage",
            "tore-quick-draft 1\nfield 4 x\n",
        ] {
            let path = unique_file("quick-mission-v1.conf");
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            let mut q = setup();
            assert!(Memory::open(&mut q, path.clone(), false).is_none());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), text);
            let _ = std::fs::remove_dir_all(path.parent().unwrap());
        }
    }

    #[test]
    fn the_lobbys_creator_is_not_saved() {
        let path = unique_file("quick-mission-v1.conf");
        let mut q = setup();
        let mut memory = Memory::open(&mut q, path.clone(), false).unwrap();
        q.enter_lobby();
        q.apply(5, 3);
        memory.save(&q);
        assert!(!path.exists());
        q.leave_lobby();
        memory.save(&q);
        assert!(path.exists());
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }
}
