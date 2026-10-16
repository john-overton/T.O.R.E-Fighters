//! Operator-authored mission sequences. The session owns timing and debriefs;
//! this module owns files, overrides, ordering, and console selection.
use crate::{
    config::{AfterEnd, Config},
    prepare::load_mission,
};
use std::{
    collections::BTreeSet,
    path::Path,
    sync::{Arc, Mutex},
};
use tore_session::host::sequence::{MissionSequence, Round};

/// Mission-scoped names come from the shared settings registry, except the
/// server-wide lobby controls. New mission settings need no second parser.
pub fn mission_setting(name: &str) -> bool {
    name == "time-limit"
        || (tore_session::settings::by_name(name).is_some()
            && ![
                "name",
                "password",
                "max-players",
                "visibility",
                "snapshot-rate",
                "host",
            ]
            .contains(&name))
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub config: Config,
    pub line: usize,
}
#[derive(Clone, Debug)]
pub struct Rotation {
    pub base: Config,
    pub entries: Vec<Entry>,
    pub shuffle: bool,
    order: Vec<usize>,
    position: usize,
    pub current: usize,
    seed: u64,
    pending: Option<Box<Rotation>>,
    forced: Option<usize>,
    pub notes: Vec<String>,
}
impl Rotation {
    pub fn read(config: &Config) -> Result<Self, String> {
        let path = config
            .rotation
            .as_ref()
            .ok_or("No rotation is configured.")?;
        let text =
            tore_import::files::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::parse(&text, path, config)
    }
    pub fn parse(text: &str, path: &Path, base: &Config) -> Result<Self, String> {
        let fail = |line, text: String| format!("{}: line {line}: {text}", path.display());
        let mut entries: Vec<Entry> = Vec::new();
        let mut header = false;
        let mut order_seen = false;
        let mut shuffle = false;
        let mut seen = BTreeSet::new();
        let mut lines = Vec::new();
        let dir = path.parent().unwrap_or(Path::new("."));
        let validate = |entry: &Entry, lines: &[(usize, &str)]| {
            entry
                .config
                .check_king_settings(lines)
                .map_err(|e| fail(if e.line == 0 { entry.line } else { e.line }, e.message))
        };
        for (index, raw) in text.lines().enumerate() {
            let line = index + 1;
            let content = raw.split('#').next().unwrap_or("").trim();
            if content.is_empty() {
                continue;
            }
            if !header {
                if content != "tore-rotation 1" {
                    return Err(fail(line, "expected `tore-rotation 1`".into()));
                }
                header = true;
                continue;
            }
            let (name, value) = content
                .split_once(char::is_whitespace)
                .unwrap_or((content, ""));
            let value = value.trim();
            if value.is_empty() {
                return Err(fail(line, format!("`{name}` needs a value")));
            }
            match name {
                "order" => {
                    if order_seen || !entries.is_empty() {
                        return Err(fail(
                            line,
                            "`order` must appear once, before the missions".into(),
                        ));
                    }
                    shuffle = match value {
                        "in-order" => false,
                        "shuffle" => true,
                        _ => return Err(fail(line, "`order` must be in-order or shuffle".into())),
                    };
                    order_seen = true;
                }
                "mission" => {
                    if let Some(entry) = entries.last() {
                        validate(entry, &lines)?;
                    }
                    let mut config = base.clone();
                    config.mission = dir.join(value);
                    entries.push(Entry { config, line });
                    seen.clear();
                    lines.clear();
                }
                _ => {
                    if !mission_setting(name) {
                        return Err(fail(
                            line,
                            format!("`{name}` is not a mission-scoped setting"),
                        ));
                    }
                    let entry = entries.last_mut().ok_or_else(|| {
                        fail(line, "a setting needs a preceding `mission FILE`".into())
                    })?;
                    if !seen.insert(name) {
                        return Err(fail(line, format!("`{name}` appears twice in this entry")));
                    }
                    entry
                        .config
                        .set(name, value, dir)
                        .map_err(|e| fail(line, e))?;
                    if name == "min-players" {
                        entry.config.min_players_at = Some((crate::config::Source::Rotation, line));
                    }
                    lines.push((line, name));
                }
            }
        }
        if let Some(entry) = entries.last() {
            validate(entry, &lines)?;
        }
        if entries.is_empty() {
            return Err(fail(
                text.lines().count().max(1),
                "rotation needs at least one mission".into(),
            ));
        }
        let order = (0..entries.len()).collect();
        Ok(Self {
            base: base.clone(),
            entries,
            shuffle,
            order,
            position: 0,
            current: 0,
            seed: 1,
            pending: None,
            forced: None,
            notes: Vec::new(),
        })
    }
    pub fn start(&mut self, seed: u64) {
        self.seed = seed.max(1);
        self.shuffle_cycle(None);
        self.current = self.order[0];
    }
    fn shuffle_cycle(&mut self, previous: Option<usize>) {
        if !self.shuffle {
            return;
        }
        for i in (1..self.order.len()).rev() {
            self.seed ^= self.seed << 13;
            self.seed ^= self.seed >> 7;
            self.seed ^= self.seed << 17;
            self.order.swap(i, self.seed as usize % (i + 1));
        }
        if self.order.len() > 1
            && let Some(previous) = previous
        {
            let first = self.order[0];
            if self.entries[previous].config.mission == self.entries[first].config.mission {
                let different_path = self.order.iter().position(|&i| {
                    self.entries[i].config.mission != self.entries[previous].config.mission
                });
                let different_entry = self.order.iter().position(|&i| i != previous);
                if let Some(i) = different_path
                    .or_else(|| (first == previous).then_some(different_entry).flatten())
                {
                    self.order.swap(0, i);
                }
            }
        }
        // Repeated paths are legal entries. Avoid adjacent paths where possible.
        for i in 1..self.order.len() {
            if self.entries[self.order[i]].config.mission
                == self.entries[self.order[i - 1]].config.mission
                && let Some(j) = (i + 1..self.order.len()).find(|&j| {
                    self.entries[self.order[j]].config.mission
                        != self.entries[self.order[i - 1]].config.mission
                })
            {
                self.order.swap(i, j);
            }
        }
    }
    pub fn heading(&self) -> String {
        format!(
            "Rotation: {} missions, {}",
            self.entries.len(),
            if self.shuffle { "shuffle" } else { "in order" }
        )
    }
    pub fn label(&self, index: usize) -> String {
        match load_mission(&self.entries[index].config.mission) {
            Ok(spec) => spec.summary(),
            Err(_) => self.entries[index].config.mission.display().to_string(),
        }
    }
    pub fn current_line(&self) -> String {
        format!(
            "Rotation {}/{}: {}",
            self.current + 1,
            self.entries.len(),
            self.label(self.current)
        )
    }
    pub fn list(&self) -> String {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, e)| {
                format!(
                    "{} {}: {}",
                    if i == self.current { "*" } else { " " },
                    i + 1,
                    e.config.mission.display()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
    pub fn goto(&mut self, n: usize) -> Result<(), String> {
        if n == 0 || n > self.entries.len() {
            return Err(format!(
                "goto needs an entry from 1 to {}",
                self.entries.len()
            ));
        }
        self.forced = Some(n - 1);
        Ok(())
    }
    pub fn reload(
        &mut self,
        mut validate: impl FnMut(&Config) -> Result<(), String>,
    ) -> Result<(), String> {
        let new = Self::read(&self.base)?;
        for entry in &new.entries {
            load_mission(&entry.config.mission)?;
            validate(&entry.config)?;
        }
        self.pending = Some(Box::new(new));
        Ok(())
    }
    fn advance(&mut self) -> bool {
        if let Some(mut new) = self.pending.take() {
            new.start(self.seed);
            new.notes = std::mem::take(&mut self.notes);
            *self = *new;
            return true;
        }
        if let Some(index) = self.forced.take() {
            self.current = index;
            self.position = self.order.iter().position(|&i| i == index).unwrap();
            return true;
        }
        if self.position + 1 == self.order.len() {
            if self.base.after_end == AfterEnd::Quit {
                return false;
            }
            self.shuffle_cycle(Some(self.current));
            self.position = 0;
        } else {
            self.position += 1;
        }
        self.current = self.order[self.position];
        true
    }
}

pub struct Shared(pub Arc<Mutex<Rotation>>);
impl MissionSequence for Shared {
    fn next(&mut self) -> Option<Result<Round, String>> {
        let mut r = self.0.lock().unwrap();
        if !r.advance() {
            return None;
        }
        let c = &r.entries[r.current].config;
        Some(load_mission(&c.mission).map(|spec| {
            Round {
                spec,
                settings: c.settings.clone(),
                time_limit: (c.time_limit_minutes > 0)
                    .then(|| std::time::Duration::from_secs(u64::from(c.time_limit_minutes) * 60)),
            }
        }))
    }
    fn attempts(&self) -> usize {
        let r = self.0.lock().unwrap();
        r.pending
            .as_ref()
            .map_or(r.entries.len(), |p| p.entries.len())
    }
    fn notice(&mut self) -> Option<String> {
        let mut original = self.0.lock().unwrap();
        let mut r = original.clone();
        let text = r
            .advance()
            .then(|| format!("Next mission: {}", r.label(r.current)));
        if let Some(text) = &text {
            original.notes.push(text.clone());
        }
        text
    }
    fn validate(
        &self,
        spec: &tore_world::mission::MissionSpec,
        world: &tore_world::world::World,
    ) -> Result<(), String> {
        let r = self.0.lock().unwrap();
        crate::prepare::validate_world(&r.entries[r.current].config, spec, world)
    }
    fn loaded(&mut self) {
        let mut r = self.0.lock().unwrap();
        let line = r.current_line();
        r.notes.push(line);
    }
    fn failed(&mut self, why: &str) {
        let mut r = self.0.lock().unwrap();
        let line = format!("Rotation {} skipped: {why}", r.current + 1);
        r.notes.push(line);
    }
}

pub fn effective_settings(c: &Config) -> Vec<String> {
    use tore_session::settings::{self, Mode, Store};
    let mut store = Store::defaults(Mode::Coop);
    let _ = store.apply(&c.settings);
    let mut lines = Vec::new();
    for setting in settings::REGISTRY {
        if mission_setting(setting.name)
            && setting.name != "time-limit"
            && let Some(value) = store.get(setting.number)
            && value != setting.coop
        {
            lines.push(format!("  {} {}", setting.name, setting.text(value)));
        }
    }
    if c.time_limit_minutes != 0 {
        lines.push(format!("  time-limit {}", c.time_limit_minutes));
    }
    if lines.is_empty() {
        lines.push("  settings: defaults".into());
    }
    lines
}

/// Console actions share the same path in the socket adapter and in-process tests.
pub fn command(
    rotation: &Arc<Mutex<Rotation>>,
    host: &mut tore_session::Host,
    command: &crate::console::Command,
) -> Result<String, String> {
    use crate::console::Command;
    let mut r = rotation.lock().unwrap();
    match command {
        Command::Rotation => Ok(r.list()),
        Command::Reload => {
            r.reload(|config| {
                let spec = load_mission(&config.mission)?;
                host.check_sequence_round(
                    Round {
                        spec,
                        settings: config.settings.clone(),
                        time_limit: (config.time_limit_minutes > 0).then(|| {
                            std::time::Duration::from_secs(
                                u64::from(config.time_limit_minutes) * 60,
                            )
                        }),
                    },
                    |spec, world| crate::prepare::validate_world(config, spec, world),
                )
            })?;
            Ok("Rotation reloaded; applies at the next mission boundary.".into())
        }
        Command::Next | Command::Goto(_) => {
            if let Command::Goto(n) = command {
                r.goto(*n)?;
            }
            drop(r);
            host.end();
            Ok("Ending the mission; rotation advances after restart-delay.".into())
        }
        _ => Err("Not a rotation command.".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(text: &str, base: &str) -> Result<Rotation, String> {
        Rotation::parse(
            text,
            Path::new("/config/maps/rotation.txt"),
            &Config::parse(base, Path::new("/config")).unwrap(),
        )
    }
    #[test]
    fn ai_off_is_a_mission_override_and_does_not_leak_to_the_next_entry() {
        let rotation = parse(
            "tore-rotation 1\nmission choice.txt\nai off\nmission fixed.txt",
            "ai on",
        )
        .unwrap();
        assert!(mission_setting("ai"));
        for (entry, expected) in rotation.entries.iter().zip([false, true]) {
            let mut settings =
                tore_session::settings::Store::defaults(tore_session::settings::Mode::Coop);
            settings.apply(&entry.config.settings).unwrap();
            assert_eq!(settings.ai(), expected);
        }
    }

    #[test]
    fn min_players_is_a_mission_override_checked_per_entry_and_does_not_leak() {
        let r = parse(
            "tore-rotation 1\nmission a.txt\nmin-players 3\nmission b.txt\nmission c.txt\nmin-players none",
            "min-players 1\nmax-players 4",
        )
        .unwrap();
        assert!(mission_setting("min-players"));
        let min = |i: usize| {
            r.entries[i]
                .config
                .settings
                .iter()
                .find(|(n, _)| *n == tore_session::settings::number::MIN_PLAYERS)
                .map(|&(_, v)| v)
        };
        // The entry's own, the server's, and the entry that turns it off.
        assert_eq!((min(0), min(1), min(2)), (Some(3), Some(1), Some(0)));
        assert!(effective_settings(&r.entries[0].config).contains(&"  min-players 3".into()));
        // An entry above the player limit is refused at its own line.
        let error = parse(
            "tore-rotation 1\nmission a.txt\nmission b.txt\nmin-players 5",
            "max-players 4",
        )
        .unwrap_err();
        assert!(
            error.contains("line 4:") && error.contains("max-players"),
            "{error}"
        );
        // The refusal in the range names the entry's line too.
        assert!(
            parse("tore-rotation 1\nmission a.txt\nmin-players 31", "")
                .unwrap_err()
                .contains("line 3:")
        );
    }

    #[test]
    fn overrides_layer_on_server_values_and_validate_effective_mode() {
        let r = parse("tore-rotation 1\nmission a.txt\n kill-limit 10\n time-limit 20\nmission a.txt\nfight free-for-all", "mode pvp\ntime-limit 5\nfriendly-fire off").unwrap();
        assert_eq!(r.entries[0].config.mission, Path::new("/config/maps/a.txt"));
        assert_eq!(r.entries[0].config.time_limit_minutes, 20);
        assert_eq!(r.entries[1].config.time_limit_minutes, 5);
        assert!(effective_settings(&r.entries[0].config).contains(&"  friendly-fire off".into()));
        assert!(
            parse("tore-rotation 1\nmission a\nkill-limit 10", "")
                .unwrap_err()
                .contains("line 3")
        );
        assert!(
            parse(
                "tore-rotation 1\nmission a\nmode co-op",
                "mode pvp\nkill-limit 10"
            )
            .unwrap_err()
            .contains("line 2")
        );
        assert!(parse("tore-rotation 1\nmission a\nkill-limit 10\nmode pvp", "").is_ok());
    }
    #[test]
    fn strict_refusals_have_source_lines() {
        for (text, line) in [
            ("tore-rotation 2", 1),
            ("", 1),
            ("tore-rotation 1\norder other", 2),
            ("tore-rotation 1\norder shuffle\norder shuffle", 3),
            ("tore-rotation 1\nmission a\norder shuffle", 3),
            ("tore-rotation 1\nmode pvp", 2),
            ("tore-rotation 1\nmission", 2),
            ("tore-rotation 1\nmission a\nmode", 3),
            ("tore-rotation 1\nmission a\nmode pvp\nmode pvp", 4),
            ("tore-rotation 1\nmission a\nrespawn maybe", 3),
            ("tore-rotation 1\nmission a\nkill-limit 4\nmode pvp", 3),
        ] {
            assert!(
                parse(text, "")
                    .unwrap_err()
                    .contains(&format!("line {line}:")),
                "{text}"
            );
        }
        for name in crate::config::SETTINGS {
            if mission_setting(name) || name == "mission" {
                continue;
            }
            let text = format!("tore-rotation 1\nmission a\n{name} bad");
            assert!(parse(&text, "").unwrap_err().contains("line 3:"), "{name}");
        }
        assert!(
            parse("tore-rotation 1\nmission a\nunknown bad", "")
                .unwrap_err()
                .contains("line 3:")
        );
        for text in ["mission a\nrotation b", "rotation b\nmission a"] {
            assert!(
                Config::parse(text, Path::new("."))
                    .unwrap_err()
                    .message
                    .contains("mutually exclusive")
            );
        }
    }
    #[test]
    fn shuffle_has_each_entry_once_per_cycle_and_no_boundary_repeat() {
        for n in 2..10 {
            let text = format!(
                "tore-rotation 1\norder shuffle\n{}",
                (0..n).map(|i| format!("mission {i}\n")).collect::<String>()
            );
            for seed in 1..20 {
                let mut r = parse(&text, "").unwrap();
                r.start(seed);
                let mut previous = None;
                for _ in 0..20 {
                    let mut seen = BTreeSet::new();
                    for _ in 0..n {
                        assert_ne!(previous, Some(r.current));
                        assert!(seen.insert(r.current));
                        previous = Some(r.current);
                        assert!(r.advance());
                    }
                    assert_eq!(seen.len(), n);
                }
            }
        }
    }
    #[test]
    fn quit_after_one_cycle_and_goto_is_one_based() {
        let mut r = parse(
            "tore-rotation 1\nmission a\nmission b\nmission c",
            "after-end quit",
        )
        .unwrap();
        r.start(1);
        assert_eq!(r.current, 0);
        assert!(r.goto(0).is_err());
        assert!(r.goto(4).is_err());
        assert!(r.advance());
        assert_eq!(r.current, 1);
        assert!(r.advance());
        assert_eq!(r.current, 2);
        assert!(!r.advance());
        let mut r = parse("tore-rotation 1\nmission a\nmission b\nmission c", "").unwrap();
        r.goto(3).unwrap();
        assert!(r.advance());
        assert_eq!(r.current, 2);
        assert!(r.advance());
        assert_eq!(r.current, 0);
    }
    #[test]
    fn reload_is_transactional_and_applies_at_the_next_boundary() {
        let dir = crate::log::tests::scratch("rotation-reload");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("rotation.txt");
        std::fs::write(dir.join("a"), crate::prepare::tests::MISSION).unwrap();
        std::fs::write(&path, "tore-rotation 1\nmission a\nmission a").unwrap();
        let mut config = Config::defaults(&dir);
        config.rotation = Some(path.clone());
        let mut r = Rotation::read(&config).unwrap();
        std::fs::write(&path, "tore-rotation 1\nmission a\nport 3").unwrap();
        assert!(r.reload(|_| Ok(())).is_err());
        assert_eq!(r.entries.len(), 2);
        std::fs::write(&path, "tore-rotation 1\nmission a\ntime-limit 20").unwrap();
        r.reload(|_| Ok(())).unwrap();
        assert_eq!(r.entries.len(), 2);
        r.advance();
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].config.time_limit_minutes, 20);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn docs_example_parses_verbatim() {
        // A Windows checkout may give the guide CRLF line ends.
        let docs = include_str!("../../../docs/DEDICATED-SERVER.md").replace("\r\n", "\n");
        let text = docs
            .split("```text\ntore-rotation 1\n")
            .nth(1)
            .unwrap()
            .split("```")
            .next()
            .unwrap();
        let r = parse(&format!("tore-rotation 1\n{text}"), "").unwrap();
        assert_eq!(r.entries.len(), 3);
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use crate::{
        options::Options,
        prepare::{
            self,
            tests::{MISSION, data_folder},
        },
    };
    use std::{fs, time::Duration};
    use tore_session::{BuildId, Host, HostConfig, Phase};
    #[test]
    fn each_rotation_entry_has_its_own_minimum_and_it_applies_again_each_round() {
        let dir = data_folder("rotation-min-players", true);
        fs::write(dir.join("one.txt"), MISSION).unwrap();
        fs::write(dir.join("two.txt"), MISSION).unwrap();
        fs::write(
            dir.join("server.conf"),
            "rotation rotation.txt\nrestart-delay 1\nmax-players 4",
        )
        .unwrap();
        fs::write(
            dir.join("rotation.txt"),
            "tore-rotation 1\nmission one.txt\nmin-players 2\nmission two.txt",
        )
        .unwrap();
        let prepared = prepare::prepare(&Options::default(), &dir).unwrap();
        assert!(
            prepared
                .rotation_report
                .contains(&"  min-players 2".to_owned())
        );
        let rotation = Arc::new(Mutex::new(prepared.rotation.unwrap()));
        let mut config = HostConfig::new(BuildId {
            version: "test".into(),
            commit: "test".into(),
            release: false,
        });
        config.restart_delay = Duration::from_secs(1);
        config.max_players = 4;
        config.settings = prepared.config.settings;
        let mut host = Host::new(prepared.spec, prepared.resources, config).unwrap();
        host.set_mission_sequence(Box::new(Shared(rotation.clone())));
        assert_eq!(host.settings().min_players(), 2);
        assert_eq!(
            host.start_wait_reason().as_deref(),
            Some("Waiting for 2 more ready players (minimum 2).")
        );
        // The second entry has none; the third round is the first again.
        host.end();
        host.update(Duration::from_secs(1));
        assert_eq!(rotation.lock().unwrap().current, 1);
        assert_eq!(host.settings().min_players(), 0);
        assert_eq!(host.start_wait_reason(), None);
        host.end();
        host.update(Duration::from_secs(2));
        assert_eq!(rotation.lock().unwrap().current, 0);
        assert_eq!(host.settings().min_players(), 2);
        assert_eq!(host.phase(), Phase::Lobby);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn prepared_rotation_runs_in_process_and_runtime_file_errors_are_skipped() {
        let dir = data_folder("rotation-in-process", true);
        fs::write(dir.join("one.txt"), MISSION).unwrap();
        fs::write(
            dir.join("two.txt"),
            MISSION.replace("separation-nm 2", "separation-nm 50"),
        )
        .unwrap();
        fs::write(
            dir.join("server.conf"),
            "rotation rotation.txt\nrestart-delay 1\nfriendly-fire off",
        )
        .unwrap();
        fs::write(
            dir.join("rotation.txt"),
            "tore-rotation 1\nmission one.txt\nmission two.txt\nmode pvp\nkill-limit 10",
        )
        .unwrap();
        let prepared = prepare::prepare(&Options::default(), &dir).unwrap();
        assert_eq!(
            prepared
                .rotation_report
                .iter()
                .filter(|s| s.starts_with("Rotation "))
                .count(),
            2
        );
        assert!(!prepared.spec.friendly_fire);
        let rotation = Arc::new(Mutex::new(prepared.rotation.unwrap()));
        let mut config = HostConfig::new(BuildId {
            version: "test".into(),
            commit: "test".into(),
            release: false,
        });
        config.restart_delay = Duration::from_secs(1);
        config.settings = prepared.config.settings;
        let mut host = Host::new(prepared.spec, prepared.resources, config).unwrap();
        host.set_mission_sequence(Box::new(Shared(rotation.clone())));
        command(&rotation, &mut host, &crate::console::Command::Next).unwrap();
        host.update(Duration::from_millis(999));
        assert_eq!(rotation.lock().unwrap().current, 0);
        host.update(Duration::from_secs(1));
        assert_eq!(rotation.lock().unwrap().current, 1);
        assert_eq!(host.settings().mode(), tore_session::settings::Mode::Pvp);
        assert!(host.mission_text().contains("separation-nm 50"));
        assert_eq!(host.phase(), Phase::Lobby);
        assert!(
            command(&rotation, &mut host, &crate::console::Command::Rotation)
                .unwrap()
                .contains("* 2:")
        );
        assert!(command(&rotation, &mut host, &crate::console::Command::Goto(0)).is_err());
        command(&rotation, &mut host, &crate::console::Command::Goto(1)).unwrap();
        host.update(Duration::from_secs(2));
        assert_eq!(rotation.lock().unwrap().current, 0);
        command(&rotation, &mut host, &crate::console::Command::Goto(2)).unwrap();
        host.update(Duration::from_secs(3));
        assert_eq!(rotation.lock().unwrap().current, 1);
        // A parser-valid reload that cannot build is refused, keeping the old cycle.
        fs::write(dir.join("bad.txt"), MISSION.replace("F18.PT", "SU27.PT")).unwrap();
        fs::write(dir.join("rotation.txt"), "tore-rotation 1\nmission bad.txt").unwrap();
        let result = command(&rotation, &mut host, &crate::console::Command::Reload);
        assert!(result.is_err());
        assert_eq!(rotation.lock().unwrap().entries.len(), 2);
        // A previously checked entry can fail at runtime. Skip it and load the next.
        fs::write(dir.join("one.txt"), "broken mission").unwrap();
        host.end();
        host.update(Duration::from_secs(4));
        assert_eq!(host.phase(), Phase::Lobby);
        assert_eq!(rotation.lock().unwrap().current, 1);
        assert!(
            rotation
                .lock()
                .unwrap()
                .notes
                .iter()
                .any(|s| s.contains("Rotation 1 skipped"))
        );
        fs::write(
            dir.join("rotation.txt"),
            "tore-rotation 1\nmission two.txt\ntime-limit 15",
        )
        .unwrap();
        command(&rotation, &mut host, &crate::console::Command::Reload).unwrap();
        assert_eq!(rotation.lock().unwrap().entries.len(), 2);
        host.end();
        host.update(Duration::from_secs(5));
        assert_eq!(rotation.lock().unwrap().entries.len(), 1);
        assert_eq!(host.settings().time_limit_seconds(), Some(900));
        fs::write(dir.join("two.txt"), "also broken").unwrap();
        host.end();
        host.update(Duration::from_secs(6));
        assert_eq!(host.phase(), Phase::Stopped);
        fs::remove_dir_all(dir).unwrap();
    }
}
