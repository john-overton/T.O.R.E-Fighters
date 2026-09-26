//! The Replays screen: the mission recordings in the replays folder, newest
//! first, the selected recording's details, and buttons to watch, keep,
//! delete and export it, plus the auto-delete settings. It is an overlay on
//! the main menu, opened from the top bar's Replays entry like the Controls
//! and Graphics screens, and drawn with their shared helpers and the
//! imported raster font. Opinionated addition requested by John on
//! 2026-09-26; the layout, wording and behaviour are agent decisions
//! (2026-09-26). See docs/REPLAYS.md#replays-screen.
//!
//! The menu never waits on a recording: a recording's details and both
//! exports are read on background threads that [`Replays::poll`] collects.
use super::cli;
use super::library::{self, Entry, Library, Rule, Settings};
use crate::controls_editor::{
    self as chrome, BUTTON, Editor as Chrome, FOCUS, GOOD, HEADER, INK, MUTED, PALE, PALE_ALT,
    PANEL, Rect, TITLE, WHITE, fit, inside, text_width,
};
use crate::menu::Canvas;
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};
use tore_formats::font::Font;
use tore_replay::vocab::kind;

/// What the host does after an input.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    None,
    /// The screen changed in answer to the player; the host plays the click.
    Changed,
    /// Back or Esc: the host closes the screen.
    Close,
    /// Watch: the host opens the replay viewer on this recording.
    Watch(PathBuf),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Button {
    Watch,
    Keep,
    Delete,
    Tacview,
    DebugLog,
    AutoDelete,
    Back,
}
const BUTTONS: [Button; 7] = [
    Button::Watch,
    Button::Keep,
    Button::Delete,
    Button::Tacview,
    Button::DebugLog,
    Button::AutoDelete,
    Button::Back,
];
impl Button {
    fn label(self) -> &'static str {
        match self {
            Button::Watch => "Watch",
            Button::Keep => "Keep",
            Button::Delete => "Delete",
            Button::Tacview => "Tacview",
            Button::DebugLog => "Debug log",
            Button::AutoDelete => "Auto-delete",
            Button::Back => "Back",
        }
    }
    /// Buttons that act on the selected recording.
    fn needs_recording(self) -> bool {
        !matches!(self, Button::AutoDelete | Button::Back)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Focus {
    /// The recordings list; its selected row is the cursor.
    List,
    Footer(usize),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Hit {
    /// A recording, by its index in the list.
    Row(usize),
    Footer(usize),
    /// An auto-delete settings row, outside its choices.
    Setting(usize),
    /// A settings row and one of its choices.
    Choice(usize, usize),
    Done,
    /// The delete confirmation: true for Delete, false for Cancel.
    Confirm(bool),
}

/// One choice in the auto-delete settings.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Choice {
    AutoDelete(bool),
    Rule(Rule),
    KeepLast(u32),
    OlderThan(u32),
}
impl Choice {
    fn label(self) -> String {
        match self {
            Choice::AutoDelete(true) => "On".into(),
            Choice::AutoDelete(false) => "Off".into(),
            Choice::Rule(Rule::KeepLast) => "Keep a number".into(),
            Choice::Rule(Rule::OlderThan) => "Delete by age".into(),
            Choice::KeepLast(n) => n.to_string(),
            Choice::OlderThan(n) => format!("{n} days"),
        }
    }
}

/// The auto-delete settings rows.
const SETTING_LABELS: [&str; 4] = ["Auto-delete", "Rule", "Keep the last", "Delete older than"];
/// Recordings the "keep the newest" rule keeps, and the age limits in days.
const KEEP_LAST_CHOICES: [u32; 5] = [5, 10, 20, 50, 100];
const OLDER_THAN_CHOICES: [u32; 4] = [7, 14, 30, 90];

/// The open auto-delete settings panel; `focus` is a row, or the Done
/// button after the last row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Panel {
    focus: usize,
}

/// The open delete confirmation.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Confirm {
    path: PathBuf,
    /// Delete has the focus rather than Cancel.
    delete: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Export {
    Tacview,
    DebugLog,
}
impl Export {
    fn title(self) -> &'static str {
        match self {
            Export::Tacview => "Tacview file",
            Export::DebugLog => "debug log",
        }
    }
}

/// An export being written on a background thread.
struct Job {
    export: Export,
    recording: PathBuf,
    name: String,
    started: Instant,
    handle: JoinHandle<Result<PathBuf, String>>,
}

/// A recording's details being read on a background thread.
struct Loader {
    path: PathBuf,
    handle: JoinHandle<Result<Details, String>>,
}

/// What a full read of a recording adds to its header and footer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Details {
    /// The player's aircraft, as the recording names it.
    pub player: Option<String>,
    /// Wings and aircraft on each side, friendly first.
    pub sides: Vec<SideCount>,
    /// Aircraft the player destroyed, counted from the recorded kills.
    pub kills: usize,
    /// Ticks of the player's bookmarks.
    pub bookmarks: Vec<u64>,
    /// First and last recorded tick; known for unfinished files too.
    pub ticks: Option<(u64, u64)>,
    /// Damaged parts the reader skipped.
    pub problems: usize,
}

/// One side's wings and aircraft.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SideCount {
    pub side: &'static str,
    pub wings: usize,
    pub aircraft: usize,
}

impl Details {
    /// Reads the whole recording: identities, events and the tick range.
    pub fn read(path: &Path) -> Result<Self, String> {
        let recording = tore_replay::Recording::open(path).map_err(|e| e.to_string())?;
        let mut sides: Vec<(tore_replay::Side, BTreeSet<u16>, usize)> = Vec::new();
        for info in recording.aircraft() {
            let at = match sides.iter().position(|(side, ..)| *side == info.side) {
                Some(at) => at,
                None => {
                    sides.push((info.side, BTreeSet::new(), 0));
                    sides.len() - 1
                }
            };
            if info.wing > 0 {
                sides[at].1.insert(info.wing);
            }
            sides[at].2 += 1;
        }
        sides.sort_by_key(|(side, ..)| side.code());
        let events = recording.events();
        let kills = events
            .iter()
            .filter(|e| {
                e.event.kind == kind::COMBAT_DESTROYED
                    && e.event.object == Some(0)
                    && e.event
                        .subject
                        .is_some_and(|s| s != 0 && recording.aircraft_info(s).is_some())
            })
            .count();
        Ok(Self {
            player: recording
                .aircraft_info(0)
                .map(|info| info.name.clone())
                .filter(|name| !name.is_empty()),
            sides: sides
                .into_iter()
                .map(|(side, wings, aircraft)| SideCount {
                    side: side.name(),
                    wings: wings.len(),
                    aircraft,
                })
                .collect(),
            kills,
            bookmarks: events
                .iter()
                .filter(|e| e.event.kind == kind::PLAYER_BOOKMARK)
                .map(|e| e.tick)
                .collect(),
            ticks: recording.first_tick().zip(recording.last_tick()),
            problems: recording.problems().len(),
        })
    }
}

// ---- layout -----------------------------------------------------------

const LIST: Rect = (6, 24, 410, 412);
const DETAILS: Rect = (422, 24, 212, 412);
const COLUMN_TOP: i32 = 42;
const LIST_TOP: i32 = 58;
const LINE_HEIGHT: i32 = 14;
const LIST_LINES: usize = 26;
/// Column x offsets from a row's left edge, and widths: started, theater,
/// aircraft, length and result. The markers sit left of the first.
const COLUMNS: [(i32, i32); 5] = [(26, 92), (120, 74), (196, 94), (290, 40), (338, 64)];
const COLUMN_TITLES: [&str; 5] = ["Started (UTC)", "Theater", "Aircraft", "Length", "Result"];
const FOOTER_Y: i32 = 456;
const FOOTER_W: i32 = 84;
const FOOTER_STEP: i32 = 90;
/// Details: label column width and line step.
const LABEL_W: i32 = 66;
const DETAIL_STEP: i32 = 14;
const PANEL_RECT: Rect = (100, 104, 440, 244);
const SETTING_TOP: i32 = 132;
const SETTING_STEP: i32 = 28;
const CHOICE_X: i32 = 226;
const CONFIRM_RECT: Rect = (140, 162, 360, 132);
/// A second click on the same row within this time watches it.
const DOUBLE_CLICK: Duration = Duration::from_millis(500);
/// Details kept in memory before the cache starts over.
const DETAIL_CACHE: usize = 512;

const OUTLINE: [u8; 4] = [110, 130, 156, 255];
/// The selected row while the buttons have the focus.
const SELECTED: [u8; 4] = [52, 74, 100, 255];
/// Result and marker colours on a pale row.
const GOOD_INK: [u8; 4] = [24, 112, 52, 255];
const BAD_INK: [u8; 4] = [160, 36, 36, 255];
const WARN_INK: [u8; 4] = [156, 92, 0, 255];
/// The same on the selected row and the dark panels.
const WARN: [u8; 4] = [240, 176, 64, 255];
const BAD: [u8; 4] = [255, 150, 140, 255];

/// How a result reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tone {
    Plain,
    Good,
    Bad,
    Warn,
    Muted,
}
impl Tone {
    fn color(self, on_pale: bool) -> [u8; 4] {
        match (self, on_pale) {
            (Tone::Plain, true) => INK,
            (Tone::Plain, false) => WHITE,
            (Tone::Good, true) => GOOD_INK,
            (Tone::Good, false) => GOOD,
            (Tone::Bad, true) => BAD_INK,
            (Tone::Bad, false) => BAD,
            (Tone::Warn, true) => WARN_INK,
            (Tone::Warn, false) => WARN,
            (Tone::Muted, true) => [96, 108, 124, 255],
            (Tone::Muted, false) => MUTED,
        }
    }
}

// ---- text -------------------------------------------------------------

/// `2026-09-26 15:40`, the UTC start a recording's name holds.
fn started(name: &str) -> String {
    match library::order_key(name) {
        Some(_) => format!("{} {}:{}", &name[..10], &name[11..13], &name[13..15]),
        None => "?".into(),
    }
}

/// `4:05`, or `1:02:03` from an hour, for a number of ticks.
fn clock(ticks: u64) -> String {
    let seconds = ticks / tore_replay::TICKS_PER_SECOND;
    if seconds >= 3600 {
        format!(
            "{}:{:02}:{:02}",
            seconds / 3600,
            seconds / 60 % 60,
            seconds % 60
        )
    } else {
        format!("{}:{:02}", seconds / 60, seconds % 60)
    }
}

/// `68 KB`, `20.4 MB`.
fn size(bytes: u64) -> String {
    const KB: f64 = 1024.;
    let b = bytes as f64;
    if b < KB * KB {
        format!("{} KB", (b / KB).ceil() as u64)
    } else if b < KB * KB * KB {
        format!("{:.1} MB", b / (KB * KB))
    } else {
        format!("{:.2} GB", b / (KB * KB * KB))
    }
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// One `_`-separated part of a recording name: 2 is the map, 3 the
/// aircraft.
fn name_part(name: &str, part: usize) -> Option<&str> {
    let stem = name.split('.').next()?;
    let stem = stem.rsplit_once('-').map_or(stem, |(head, n)| {
        if n.bytes().all(|b| b.is_ascii_digit()) && head.len() > 10 {
            head
        } else {
            stem
        }
    });
    stem.split('_').nth(part)
}

fn footer_value<'a>(entry: &'a Entry, key: &str) -> Option<&'a str> {
    let footer = entry.peek.as_ref().ok()?.footer.as_ref()?;
    footer
        .result
        .iter()
        .find(|(k, _)| k == key)
        .map(|(_, v)| v.as_str())
}

/// Finished normally, with a footer and seek index.
fn complete(entry: &Entry) -> bool {
    !entry.partial && entry.peek.as_ref().is_ok_and(|p| p.complete)
}

fn theater(entry: &Entry) -> String {
    let world = entry.peek.as_ref().ok().map(|p| &p.header.world);
    world
        .map(|w| w.theater_name.clone())
        .filter(|n| !n.is_empty())
        .or_else(|| world.map(|w| w.theater.clone()).filter(|n| !n.is_empty()))
        .or_else(|| name_part(&entry.name, 2).map(str::to_owned))
        .unwrap_or_else(|| "?".into())
}

/// The player's aircraft by its exact name, for example `F/A-18D Hornet`.
fn aircraft(entry: &Entry) -> String {
    let key = entry
        .peek
        .as_ref()
        .ok()
        .and_then(|p| p.header.extra("player.aircraft"))
        .or_else(|| name_part(&entry.name, 3));
    key.map_or_else(
        || "?".into(),
        |key| {
            super::convert::identity(key).map_or_else(|| key.to_owned(), |id| id.label().to_owned())
        },
    )
}

/// The list's one-word result.
fn result(entry: &Entry) -> (String, Tone) {
    let Ok(peek) = &entry.peek else {
        return ("Unreadable".into(), Tone::Muted);
    };
    if peek.footer.is_none() || entry.partial {
        return ("Incomplete".into(), Tone::Warn);
    }
    match footer_value(entry, "outcome") {
        Some("success") => ("Success".into(), Tone::Good),
        Some("failure") => ("Failure".into(), Tone::Bad),
        Some(other) => (other.into(), Tone::Plain),
        None => (
            match footer_value(entry, "end") {
                Some("end flight" | "end mission") => "Ended",
                Some("exit") => "Quit",
                Some("restart") => "Restarted",
                Some("probe finished") => "Probe",
                _ => "Finished",
            }
            .into(),
            Tone::Plain,
        ),
    }
}

/// The details panel's result: outcome, how the flight ended, the pilot.
fn result_detail(entry: &Entry) -> String {
    let (word, _) = result(entry);
    if footer_value(entry, "end").is_none() {
        return match &entry.peek {
            Err(_) => "Unknown: the file cannot be read".into(),
            Ok(_) => "Unknown: the recording did not finish".into(),
        };
    }
    let mut parts = Vec::new();
    match footer_value(entry, "end") {
        Some("end mission") => parts.push("mission ended".to_owned()),
        Some("end flight") => parts.push("flight ended".into()),
        Some("exit") => parts.push("game quit".into()),
        Some("restart") => parts.push("flight restarted".into()),
        Some(other) => parts.push(other.into()),
        None => {}
    }
    match footer_value(entry, "player") {
        Some("alive") => parts.push("pilot alive".into()),
        Some("ejected") => parts.push("pilot ejected".into()),
        Some("dead") => parts.push("pilot killed".into()),
        _ => {}
    }
    if footer_value(entry, "outcome").is_some() {
        format!("{word}: {}", parts.join(", "))
    } else {
        let text = parts.join(", ");
        let mut chars = text.chars();
        chars.next().map_or(word, |first| {
            first.to_uppercase().collect::<String>() + chars.as_str()
        })
    }
}

/// Text cut in the middle to fit `width`, keeping the start and the end,
/// so a long path still shows its file name.
fn fit_middle(font: &Font, text: &str, width: i32) -> String {
    if text_width(font, text) <= width {
        return text.into();
    }
    let dots = text_width(font, "..");
    let head_budget = (width - dots) * 2 / 5;
    let mut head = String::new();
    for c in text.chars() {
        let mut next = head.clone();
        next.push(c);
        if text_width(font, &next) > head_budget {
            break;
        }
        head = next;
    }
    let tail_budget = width - dots - text_width(font, &head);
    let mut tail = String::new();
    for c in text.chars().rev() {
        let next = format!("{c}{tail}");
        if text_width(font, &next) > tail_budget {
            break;
        }
        tail = next;
    }
    format!("{head}..{tail}")
}

/// Word-wraps `text` into at most `lines` lines of `width`, breaking after
/// spaces and path separators, or inside a word too long for a line. The
/// last line is cut with `..` when text remains.
fn wrap(font: &Font, text: &str, width: i32, lines: usize) -> Vec<String> {
    let mut tokens: Vec<String> = Vec::new();
    let mut token = String::new();
    for c in text.chars() {
        token.push(c);
        if matches!(c, ' ' | '/' | '\\' | '_') {
            tokens.push(std::mem::take(&mut token));
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    let mut out: Vec<String> = vec![String::new()];
    for token in tokens {
        let line = out.last_mut().expect("never empty");
        let joined = format!("{line}{token}");
        if text_width(font, joined.trim_end()) <= width {
            *line = joined;
            continue;
        }
        let mut rest = token.as_str();
        if !line.is_empty() {
            out.push(String::new());
        }
        // A word longer than a line breaks inside it.
        while text_width(font, rest.trim_end()) > width {
            let mut cut = 0;
            for (i, c) in rest.char_indices() {
                if text_width(font, &rest[..i + c.len_utf8()]) > width {
                    break;
                }
                cut = i + c.len_utf8();
            }
            let cut = cut.max(rest.chars().next().map_or(1, char::len_utf8));
            *out.last_mut().expect("never empty") = rest[..cut].to_owned();
            rest = &rest[cut..];
            out.push(String::new());
        }
        *out.last_mut().expect("never empty") = rest.to_owned();
    }
    let mut out: Vec<String> = out
        .into_iter()
        .map(|l| l.trim_end().to_owned())
        .filter(|l| !l.is_empty())
        .collect();
    if out.len() > lines {
        out.truncate(lines);
        if let Some(last) = out.last_mut() {
            *last = ellipsis(font, last, width);
        }
    }
    out
}

/// Like [`wrap`], but text too long for `lines` keeps its end, marked with
/// a leading `..`, so a path still shows its last folders.
fn wrap_tail(font: &Font, text: &str, width: i32, lines: usize) -> Vec<String> {
    let mut all = wrap(font, text, width, usize::MAX);
    if all.len() <= lines {
        return all;
    }
    let mut kept = all.split_off(all.len() - lines);
    let mut first = kept[0].as_str();
    while !first.is_empty() && text_width(font, &format!("..{first}")) > width {
        let mut chars = first.chars();
        chars.next();
        first = chars.as_str();
    }
    kept[0] = format!("..{first}");
    kept
}

/// `text` followed by `..`, cut to fit `width`: more text follows.
fn ellipsis(font: &Font, text: &str, width: i32) -> String {
    let dots = text_width(font, "..");
    let mut out = String::new();
    for c in text.trim_end().chars() {
        let mut next = out.clone();
        next.push(c);
        if text_width(font, &next) + dots > width {
            break;
        }
        out = next;
    }
    out.trim_end().to_owned() + ".."
}

/// Sorts recordings newest first; see [`library::order_key`].
fn newest_first(entries: &mut [Entry]) {
    entries
        .sort_by_cached_key(|e| std::cmp::Reverse((library::order_key(&e.name), e.name.clone())));
}

/// Darkens the canvas under a dialog.
fn dim(pixels: &mut [u8]) {
    for pixel in pixels.chunks_exact_mut(4) {
        for c in &mut pixel[..3] {
            *c = (u16::from(*c) * 11 / 20) as u8;
        }
    }
}

/// A small padlock, 9 by 9 pixels: the recording is kept.
fn lock(pixels: &mut [u8], (x, y): (i32, i32), color: [u8; 4], hole: [u8; 4]) {
    let mut canvas = Canvas(pixels);
    canvas.rect((x + 2, y, 5, 1), color);
    canvas.rect((x + 1, y + 1, 1, 3), color);
    canvas.rect((x + 7, y + 1, 1, 3), color);
    canvas.rect((x, y + 4, 9, 5), color);
    canvas.rect((x + 4, y + 5, 1, 2), hole);
}

/// A small warning triangle, 9 by 8 pixels: the recording is incomplete.
fn warning(pixels: &mut [u8], (x, y): (i32, i32), color: [u8; 4], mark: [u8; 4]) {
    let mut canvas = Canvas(pixels);
    for row in 0..8 {
        let half = (row + 1) / 2;
        canvas.rect((x + 4 - half, y + row, 2 * half + 1, 1), color);
    }
    canvas.rect((x + 4, y + 2, 1, 3), mark);
    canvas.rect((x + 4, y + 6, 1, 1), mark);
}

// ---- the screen -------------------------------------------------------

pub struct Replays {
    /// The recordings folder; `None` for the synthetic snapshot list.
    library: Option<Library>,
    settings: Settings,
    /// Newest first.
    entries: Vec<Entry>,
    selected: usize,
    scroll: usize,
    focus: Focus,
    pressed: Option<Hit>,
    /// The last row clicked and when, for a double-click.
    last_click: Option<(usize, Instant)>,
    /// The status line.
    pub message: String,
    /// Shown at the top right: where the screen was opened from.
    context: &'static str,
    panel: Option<Panel>,
    confirm: Option<Confirm>,
    job: Option<Job>,
    loader: Option<Loader>,
    details: HashMap<PathBuf, Result<Details, String>>,
    /// Recordings the next cleanup would delete, when known.
    plan: Option<usize>,
    /// Watch was pressed and the host has not drawn the screen since.
    watching: bool,
    /// Another screen, the replay viewer, has shown instead of this one.
    covered: bool,
}

const HINT: &str = "Enter watches, Delete removes, Tab reaches the buttons, Esc backs out.";

impl Replays {
    /// Opens the screen on `library`: applies the auto-delete rule, as the
    /// screen opening is one of the times cleanup runs, then lists the
    /// recordings.
    pub fn open(library: Option<Library>, context: &'static str) -> Self {
        let Some(library) = library else {
            return Self::with_entries(
                None,
                Vec::new(),
                Settings::default(),
                context,
                "The recordings folder is not available.".into(),
            );
        };
        let settings = library.settings();
        let cleanup = library.cleanup(&settings, SystemTime::now(), &[]);
        for path in &cleanup.deleted {
            log::info!("Recording auto-deleted: {}", path.display());
        }
        for (path, error) in &cleanup.failed {
            log::info!("Recording not deleted: {}: {error}", path.display());
        }
        let mut message = match cleanup.deleted.len() {
            0 => HINT.to_owned(),
            n => format!(
                "Auto-delete removed {}.",
                plural(n, "older recording", "older recordings")
            ),
        };
        if !cleanup.failed.is_empty() {
            message = format!(
                "Auto-delete could not remove {}; see the session log.",
                plural(cleanup.failed.len(), "recording", "recordings")
            );
        }
        let entries = library.list();
        Self::with_entries(Some(library), entries, settings, context, message)
    }

    fn with_entries(
        library: Option<Library>,
        mut entries: Vec<Entry>,
        settings: Settings,
        context: &'static str,
        message: String,
    ) -> Self {
        newest_first(&mut entries);
        let focus = if entries.is_empty() {
            Focus::Footer(BUTTONS.len() - 1)
        } else {
            Focus::List
        };
        let mut screen = Self {
            library,
            settings,
            entries,
            selected: 0,
            scroll: 0,
            focus,
            pressed: None,
            last_click: None,
            message,
            context,
            panel: None,
            confirm: None,
            job: None,
            loader: None,
            details: HashMap::new(),
            plan: None,
            watching: false,
            covered: false,
        };
        screen.refresh_plan();
        screen
    }

    /// Lists the recordings again, keeping the selected one when it is
    /// still there. Nothing is deleted: cleanup runs only when the screen
    /// opens.
    pub fn refresh(&mut self) {
        (self.watching, self.covered) = (false, false);
        let Some(library) = &self.library else {
            return;
        };
        let selected = self.current().map(|entry| entry.path.clone());
        let mut entries = library.list();
        newest_first(&mut entries);
        self.settings = library.settings();
        self.entries = entries;
        // A recording another game was still writing may have grown.
        self.details.clear();
        self.selected = selected
            .and_then(|path| self.entries.iter().position(|e| e.path == path))
            .unwrap_or(0);
        if self.entries.is_empty() {
            self.focus = Focus::Footer(BUTTONS.len() - 1);
        }
        self.select(self.selected);
        self.refresh_plan();
    }

    /// Another screen, the replay viewer, is showing instead of this one;
    /// the Replays screen stays open underneath.
    pub fn covered(&mut self) {
        self.covered = true;
    }

    /// The host is drawing the screen. `notice` is a menu notice, such as
    /// why the viewer could not open a recording, for the status line.
    /// Returns true when the screen shows again after the replay viewer,
    /// so the host lists the recordings again.
    pub fn shown(&mut self, notice: Option<String>) -> bool {
        let noticed = notice.is_some();
        if let Some(notice) = notice {
            self.message = notice;
        }
        let watched = std::mem::take(&mut self.watching);
        let back = std::mem::take(&mut self.covered);
        if watched && !back && !noticed {
            // Watch went nowhere: a build without the viewer.
            self.message = "The replay viewer is not available in this build.".into();
        }
        back
    }

    /// A synthetic list for `--snapshot-state replays`: no files are read.
    pub fn preview(context: &'static str) -> Self {
        let (entries, details) = preview::entries();
        let mut settings = Settings::default();
        settings
            .kept
            .extend(entries.iter().filter(|e| e.kept).map(|e| e.name.clone()));
        let mut screen = Self::with_entries(None, entries, settings, context, HINT.into());
        screen.select(1);
        screen.details = details;
        screen.plan = Some(3);
        screen
    }

    /// Snapshot helper: `""` is the list, `settings` opens the auto-delete
    /// panel on its "keep the newest" row and `delete` the confirmation.
    pub fn preview_state(&mut self, state: &str) -> Result<(), String> {
        match state {
            "" => {}
            "settings" => self.panel = Some(Panel { focus: 2 }),
            "delete" => {
                self.run(Button::Delete);
            }
            _ => {
                return Err(
                    "replays snapshot state must be replays, replays-settings or replays-delete"
                        .into(),
                );
            }
        }
        Ok(())
    }

    pub fn cancel_press(&mut self) {
        self.pressed = None;
    }

    // ---- model ----------------------------------------------------------

    fn current(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    fn select(&mut self, index: usize) {
        self.selected = index.min(self.entries.len().saturating_sub(1));
        self.keep_visible();
    }

    fn keep_visible(&mut self) {
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + LIST_LINES {
            self.scroll = self.selected + 1 - LIST_LINES;
        }
        self.scroll = self
            .scroll
            .min(self.entries.len().saturating_sub(LIST_LINES));
    }

    fn enabled(&self, button: Button) -> bool {
        !button.needs_recording() || self.current().is_some()
    }

    fn save(&self, settings: &Settings) -> Result<(), String> {
        match &self.library {
            Some(library) => library.save_settings(settings).map_err(|e| e.to_string()),
            None => Ok(()),
        }
    }

    fn refresh_plan(&mut self) {
        if let Some(library) = &self.library {
            self.plan = Some(library.plan(&self.settings, SystemTime::now(), &[]).len());
        }
    }

    /// The loaded details of the selected recording.
    fn current_details(&self) -> Option<&Result<Details, String>> {
        self.details.get(&self.current()?.path)
    }

    /// The recording's length in ticks: from its seek index, or for an
    /// unfinished file from a full read.
    fn length(&self, entry: &Entry) -> Option<u64> {
        let ticks = entry.peek.as_ref().ok().and_then(|p| p.ticks).or_else(|| {
            match self.details.get(&entry.path) {
                Some(Ok(details)) => details.ticks,
                _ => None,
            }
        })?;
        Some(ticks.1.saturating_sub(ticks.0) + 1)
    }

    // ---- actions --------------------------------------------------------

    fn run(&mut self, button: Button) -> Outcome {
        if button.needs_recording() && self.current().is_none() {
            self.message = "There are no recordings yet: every flight records itself.".into();
            return Outcome::Changed;
        }
        match button {
            Button::Watch => self.watch(),
            Button::Keep => self.toggle_keep(),
            Button::Delete => self.ask_delete(),
            Button::Tacview => self.export(Export::Tacview),
            Button::DebugLog => self.export(Export::DebugLog),
            Button::AutoDelete => {
                self.panel = Some(Panel { focus: 0 });
                self.refresh_plan();
                Outcome::Changed
            }
            Button::Back => Outcome::Close,
        }
    }

    fn watch(&mut self) -> Outcome {
        let Some(entry) = self.current() else {
            return Outcome::None;
        };
        if let Err(error) = &entry.peek {
            self.message = format!("{} cannot be read: {error}", entry.name);
            return Outcome::Changed;
        }
        let path = entry.path.clone();
        self.watching = true;
        Outcome::Watch(path)
    }

    fn toggle_keep(&mut self) -> Outcome {
        let Some(entry) = self.current() else {
            return Outcome::None;
        };
        let (name, keep) = (entry.name.clone(), !entry.kept);
        if keep && self.settings.kept.len() >= library::MAX_KEPT {
            self.message = format!(
                "At most {} recordings can be kept; stop keeping one first.",
                library::MAX_KEPT
            );
            return Outcome::Changed;
        }
        let mut settings = self.settings.clone();
        if keep {
            settings.kept.insert(name.clone());
        } else {
            settings.kept.remove(&name);
        }
        match self.save(&settings) {
            Ok(()) => {
                self.settings = settings;
                self.entries[self.selected].kept = keep;
                self.refresh_plan();
                self.message = if keep {
                    format!("Keeping {name}: auto-delete never removes it.")
                } else {
                    format!("{name} is no longer kept: auto-delete may remove it.")
                };
            }
            Err(error) => self.message = format!("Keep not saved: {error}"),
        }
        Outcome::Changed
    }

    fn ask_delete(&mut self) -> Outcome {
        let Some(entry) = self.current() else {
            return Outcome::None;
        };
        if self
            .job
            .as_ref()
            .is_some_and(|job| job.recording == entry.path)
        {
            self.message = format!(
                "{} is being exported; delete it when the export finishes.",
                entry.name
            );
            return Outcome::Changed;
        }
        self.confirm = Some(Confirm {
            path: entry.path.clone(),
            delete: true,
        });
        Outcome::Changed
    }

    fn delete(&mut self, path: &Path) -> Outcome {
        self.confirm = None;
        let Some(index) = self.entries.iter().position(|e| e.path == path) else {
            return Outcome::Changed;
        };
        // Only a file the listing found in the recordings folder.
        if let Some(library) = &self.library
            && path.parent() != Some(library.folder().as_path())
        {
            self.message = "Only recordings in the replays folder can be deleted.".into();
            return Outcome::Changed;
        }
        if let Err(error) = std::fs::remove_file(path) {
            self.message = format!("{} was not deleted: {error}", self.entries[index].name);
            return Outcome::Changed;
        }
        log::info!("Recording deleted: {}", path.display());
        let entry = self.entries.remove(index);
        self.details.remove(&entry.path);
        self.message = format!("Deleted {}.", entry.name);
        if self.settings.kept.contains(&entry.name) {
            let mut settings = self.settings.clone();
            settings.kept.remove(&entry.name);
            match self.save(&settings) {
                Ok(()) => self.settings = settings,
                Err(error) => {
                    self.message = format!("Deleted {}, but not saved: {error}", entry.name)
                }
            }
        }
        if self.entries.is_empty() {
            self.focus = Focus::Footer(BUTTONS.len() - 1);
        }
        self.select(self.selected);
        self.refresh_plan();
        Outcome::Changed
    }

    fn export(&mut self, export: Export) -> Outcome {
        let Some(entry) = self.current() else {
            return Outcome::None;
        };
        if let Some(job) = &self.job {
            self.message = format!(
                "Still writing the {} for {}; one export at a time.",
                job.export.title(),
                job.name
            );
            return Outcome::Changed;
        }
        let (path, name) = (entry.path.clone(), entry.name.clone());
        let source = path.clone();
        let spawned = std::thread::Builder::new()
            .name("tore-replay-export".into())
            .spawn(move || {
                let written = match export {
                    Export::Tacview => cli::acmi(&source, None, None, false),
                    Export::DebugLog => cli::log(&source, &cli::LogOptions::default()),
                }
                .map_err(|error| error.to_string());
                // Logged here too, in case the screen closed meanwhile.
                match &written {
                    Ok(out) => log::info!("Replay export written: {}", out.display()),
                    Err(error) => {
                        log::info!("Replay export of {} failed: {error}", source.display())
                    }
                }
                written
            });
        match spawned {
            Ok(handle) => {
                self.job = Some(Job {
                    export,
                    recording: path,
                    name,
                    started: Instant::now(),
                    handle,
                });
            }
            Err(error) => self.message = format!("The export could not start: {error}"),
        }
        Outcome::Changed
    }

    // ---- auto-delete settings ---------------------------------------------

    fn choices(&self, row: usize) -> Vec<Choice> {
        let numbers = |standard: &[u32], current: u32| {
            let mut list = standard.to_vec();
            if !list.contains(&current) {
                list.push(current);
                list.sort_unstable();
            }
            list
        };
        match row {
            0 => vec![Choice::AutoDelete(true), Choice::AutoDelete(false)],
            1 => vec![Choice::Rule(Rule::KeepLast), Choice::Rule(Rule::OlderThan)],
            2 => numbers(&KEEP_LAST_CHOICES, self.settings.keep_last)
                .into_iter()
                .map(Choice::KeepLast)
                .collect(),
            _ => numbers(&OLDER_THAN_CHOICES, self.settings.older_than_days)
                .into_iter()
                .map(Choice::OlderThan)
                .collect(),
        }
    }

    fn chosen(&self, choice: Choice) -> bool {
        let s = &self.settings;
        match choice {
            Choice::AutoDelete(on) => s.auto_delete == on,
            Choice::Rule(rule) => s.rule == rule,
            Choice::KeepLast(n) => s.keep_last == n,
            Choice::OlderThan(n) => s.older_than_days == n,
        }
    }

    /// Whether a settings row is what auto-delete currently follows.
    fn row_active(&self, row: usize) -> bool {
        let s = &self.settings;
        match row {
            0 => true,
            1 => s.auto_delete,
            2 => s.auto_delete && s.rule == Rule::KeepLast,
            _ => s.auto_delete && s.rule == Rule::OlderThan,
        }
    }

    /// What the current settings do, in a sentence.
    fn rule_text(&self) -> String {
        let s = &self.settings;
        match (s.auto_delete, s.rule) {
            (false, _) => "Auto-delete is off: recordings stay until you delete them.".into(),
            (true, Rule::KeepLast) => format!(
                "Auto-delete keeps the newest {} recordings, and every kept one.",
                s.keep_last
            ),
            (true, Rule::OlderThan) => format!(
                "Auto-delete removes recordings older than {} days, except kept ones.",
                s.older_than_days
            ),
        }
    }

    /// Chooses a setting; the settings file is saved at once. Picking a
    /// number also picks its rule.
    fn choose(&mut self, choice: Choice) -> Outcome {
        let mut settings = self.settings.clone();
        match choice {
            Choice::AutoDelete(on) => settings.auto_delete = on,
            Choice::Rule(rule) => settings.rule = rule,
            Choice::KeepLast(n) => {
                settings.keep_last = n;
                settings.rule = Rule::KeepLast;
            }
            Choice::OlderThan(n) => {
                settings.older_than_days = n;
                settings.rule = Rule::OlderThan;
            }
        }
        if settings == self.settings {
            return Outcome::None;
        }
        match self.save(&settings) {
            Ok(()) => {
                self.settings = settings;
                self.refresh_plan();
                self.message = format!("{} Saved.", self.rule_text());
            }
            Err(error) => self.message = format!("Auto-delete settings not saved: {error}"),
        }
        Outcome::Changed
    }

    /// Left and right: the next choice in that direction, stopping at the
    /// ends.
    fn step_choice(&mut self, row: usize, delta: i32) -> Outcome {
        let choices = self.choices(row);
        let at = choices.iter().position(|c| self.chosen(*c)).unwrap_or(0) as i32;
        let next = (at + delta).clamp(0, choices.len() as i32 - 1) as usize;
        self.choose(choices[next])
    }

    /// Enter: the next choice, wrapping.
    fn cycle_choice(&mut self, row: usize) -> Outcome {
        let choices = self.choices(row);
        let at = choices.iter().position(|c| self.chosen(*c)).unwrap_or(0);
        self.choose(choices[(at + 1) % choices.len()])
    }

    // ---- input ----------------------------------------------------------

    /// Keyboard input, and controller menu actions mapped to arrow keys,
    /// Enter and Escape. A held key repeats movement only, never an action.
    pub fn key(&mut self, key: &str, shift: bool, repeat: bool) -> Outcome {
        let movement = matches!(
            key,
            "ArrowUp" | "ArrowDown" | "ArrowLeft" | "ArrowRight" | "PageUp" | "PageDown"
        );
        if repeat && !movement {
            return Outcome::None;
        }
        if let Some(confirm) = &mut self.confirm {
            return match key {
                "Escape" => {
                    self.confirm = None;
                    Outcome::Changed
                }
                "ArrowLeft" | "ArrowRight" | "Tab" => {
                    confirm.delete = !confirm.delete;
                    Outcome::None
                }
                "Enter" | "Space" if confirm.delete => {
                    let path = confirm.path.clone();
                    self.delete(&path)
                }
                "Enter" | "Space" => {
                    self.confirm = None;
                    Outcome::Changed
                }
                _ => Outcome::None,
            };
        }
        if let Some(panel) = &mut self.panel {
            let rows = SETTING_LABELS.len() + 1;
            match key {
                "Escape" => {
                    self.panel = None;
                    return Outcome::Changed;
                }
                "ArrowUp" => panel.focus = panel.focus.saturating_sub(1),
                "ArrowDown" => panel.focus = (panel.focus + 1).min(rows - 1),
                "Tab" => {
                    panel.focus = if shift {
                        (panel.focus + rows - 1) % rows
                    } else {
                        (panel.focus + 1) % rows
                    }
                }
                "ArrowLeft" | "ArrowRight" if panel.focus < SETTING_LABELS.len() => {
                    let row = panel.focus;
                    return self.step_choice(row, if key == "ArrowLeft" { -1 } else { 1 });
                }
                "Enter" | "Space" if panel.focus < SETTING_LABELS.len() => {
                    let row = panel.focus;
                    return self.cycle_choice(row);
                }
                "Enter" | "Space" => {
                    self.panel = None;
                    return Outcome::Changed;
                }
                _ => {}
            }
            return Outcome::None;
        }
        let last = self.entries.len().saturating_sub(1);
        match (key, self.focus) {
            ("Escape", _) => Outcome::Close,
            ("ArrowUp", Focus::List) => {
                self.select(self.selected.saturating_sub(1));
                Outcome::None
            }
            ("ArrowDown", Focus::List) if self.selected < last => {
                self.select(self.selected + 1);
                Outcome::None
            }
            ("ArrowDown", Focus::List) => {
                if !repeat {
                    self.focus = Focus::Footer(self.next_button(None, false));
                }
                Outcome::None
            }
            ("ArrowUp", Focus::Footer(_)) if !self.entries.is_empty() => {
                self.focus = Focus::List;
                Outcome::None
            }
            ("PageUp" | "PageDown" | "Home" | "End", _) if !self.entries.is_empty() => {
                let target = match key {
                    "PageUp" => self.selected.saturating_sub(LIST_LINES),
                    "PageDown" => (self.selected + LIST_LINES).min(last),
                    "Home" => 0,
                    _ => last,
                };
                self.focus = Focus::List;
                self.select(target);
                Outcome::None
            }
            ("ArrowLeft", Focus::Footer(i)) => {
                self.focus = Focus::Footer(self.next_button(Some(i), true));
                Outcome::None
            }
            ("ArrowRight", Focus::Footer(i)) => {
                self.focus = Focus::Footer(self.next_button(Some(i), false));
                Outcome::None
            }
            ("Tab", focus) => {
                let order = self.order();
                let at = order.iter().position(|f| *f == focus).unwrap_or(0);
                let n = order.len();
                self.focus = order[if shift { at + n - 1 } else { at + 1 } % n];
                Outcome::None
            }
            ("Enter" | "Space", Focus::List) => self.run(Button::Watch),
            ("Enter" | "Space", Focus::Footer(i)) => self.run(BUTTONS[i]),
            ("Delete" | "Backspace", _) => self.run(Button::Delete),
            _ => Outcome::None,
        }
    }

    /// Focus order for Tab: the list, then every usable button.
    fn order(&self) -> Vec<Focus> {
        let list = (!self.entries.is_empty()).then_some(Focus::List);
        list.into_iter()
            .chain(
                (0..BUTTONS.len())
                    .filter(|i| self.enabled(BUTTONS[*i]))
                    .map(Focus::Footer),
            )
            .collect()
    }

    /// The next usable button left or right of `from` (or the first one),
    /// staying at the ends.
    fn next_button(&self, from: Option<usize>, left: bool) -> usize {
        let usable: Vec<usize> = (0..BUTTONS.len())
            .filter(|i| self.enabled(BUTTONS[*i]))
            .collect();
        match from {
            None => usable[0],
            Some(i) if left => usable.iter().rev().find(|u| **u < i).copied().unwrap_or(i),
            Some(i) => usable.iter().find(|u| **u > i).copied().unwrap_or(i),
        }
    }

    /// A wheel notch scrolls the list three rows, up for a positive count,
    /// or moves between the settings rows.
    pub fn wheel(&mut self, notches: i32) -> Outcome {
        if self.confirm.is_some() {
            return Outcome::None;
        }
        if let Some(panel) = &mut self.panel {
            let last = SETTING_LABELS.len() as i32;
            panel.focus = (panel.focus as i32 - notches).clamp(0, last) as usize;
            return Outcome::None;
        }
        let max = self.entries.len().saturating_sub(LIST_LINES) as i32;
        self.scroll = (self.scroll as i32 - notches * 3).clamp(0, max) as usize;
        Outcome::None
    }

    /// Left button: a click needs press and release on the same control.
    pub fn pointer(&mut self, point: Option<(f64, f64)>, down: bool) -> Outcome {
        let hit = point.and_then(|p| self.hit(p));
        if down {
            self.pressed = hit;
            return Outcome::None;
        }
        let pressed = self.pressed.take();
        if pressed != hit {
            return Outcome::None;
        }
        hit.map_or(Outcome::None, |hit| self.activate(hit))
    }

    fn activate(&mut self, hit: Hit) -> Outcome {
        match hit {
            Hit::Row(index) => {
                let now = Instant::now();
                let double = index == self.selected
                    && self.last_click.is_some_and(|(row, at)| {
                        row == index && now.duration_since(at) < DOUBLE_CLICK
                    });
                self.focus = Focus::List;
                self.select(index);
                if double {
                    self.last_click = None;
                    return self.run(Button::Watch);
                }
                self.last_click = Some((index, now));
                Outcome::Changed
            }
            Hit::Footer(i) => {
                if !self.enabled(BUTTONS[i]) {
                    return Outcome::None;
                }
                self.focus = Focus::Footer(i);
                self.run(BUTTONS[i])
            }
            Hit::Setting(row) => {
                if let Some(panel) = &mut self.panel {
                    panel.focus = row;
                }
                Outcome::None
            }
            Hit::Choice(row, i) => {
                if let Some(panel) = &mut self.panel {
                    panel.focus = row;
                }
                let choice = self.choices(row)[i];
                match self.choose(choice) {
                    // Picking the current choice still answers the click.
                    Outcome::None => Outcome::Changed,
                    outcome => outcome,
                }
            }
            Hit::Done => {
                self.panel = None;
                Outcome::Changed
            }
            Hit::Confirm(true) => match self.confirm.as_ref().map(|c| c.path.clone()) {
                Some(path) => self.delete(&path),
                None => Outcome::None,
            },
            Hit::Confirm(false) => {
                self.confirm = None;
                Outcome::Changed
            }
        }
    }

    // ---- background work ------------------------------------------------

    /// Collects finished exports and details and starts reading the
    /// selected recording's details. Returns true while work is running,
    /// so the host keeps drawing.
    pub fn poll(&mut self) -> bool {
        if self
            .job
            .as_ref()
            .is_some_and(|job| job.handle.is_finished())
            && let Some(job) = self.job.take()
        {
            let written = job
                .handle
                .join()
                .unwrap_or_else(|_| Err("the export stopped unexpectedly".into()));
            self.message = match written {
                Ok(path) => match job.export {
                    Export::Tacview => format!("Tacview file written: {}", path.display()),
                    Export::DebugLog => format!("Debug log written: {}", path.display()),
                },
                Err(error) => format!(
                    "The {} for {} was not written: {error}",
                    job.export.title(),
                    job.name
                ),
            };
        }
        if self
            .loader
            .as_ref()
            .is_some_and(|loader| loader.handle.is_finished())
            && let Some(loader) = self.loader.take()
        {
            let details = loader
                .handle
                .join()
                .unwrap_or_else(|_| Err("the reader stopped unexpectedly".into()));
            if self.details.len() >= DETAIL_CACHE {
                self.details.clear();
            }
            self.details.insert(loader.path, details);
        }
        if self.loader.is_none()
            && let Some(entry) = self.current()
            && !self.details.contains_key(&entry.path)
        {
            let path = entry.path.clone();
            let source = path.clone();
            match std::thread::Builder::new()
                .name("tore-replay-details".into())
                .spawn(move || Details::read(&source))
            {
                Ok(handle) => self.loader = Some(Loader { path, handle }),
                Err(error) => {
                    self.details.insert(path, Err(error.to_string()));
                }
            }
        }
        self.job.is_some() || self.loader.is_some()
    }

    /// The status line: the message, or an export's progress.
    fn status(&self) -> String {
        match &self.job {
            Some(job) => {
                let elapsed = job.started.elapsed();
                let dots = ".".repeat(1 + (elapsed.as_millis() / 400 % 3) as usize);
                format!(
                    "Writing the {} for {}{dots} {} s",
                    job.export.title(),
                    job.name,
                    elapsed.as_secs()
                )
            }
            None => self.message.clone(),
        }
    }

    // ---- pointer --------------------------------------------------------

    fn row_rect(row: usize) -> Rect {
        (
            LIST.0 + 2,
            LIST_TOP + row as i32 * LINE_HEIGHT,
            LIST.2 - 7,
            LINE_HEIGHT - 1,
        )
    }
    fn footer_rect(i: usize) -> Rect {
        (8 + i as i32 * FOOTER_STEP, FOOTER_Y, FOOTER_W, 18)
    }
    fn setting_rect(row: usize) -> Rect {
        (
            PANEL_RECT.0 + 6,
            SETTING_TOP + row as i32 * SETTING_STEP,
            PANEL_RECT.2 - 12,
            SETTING_STEP - 4,
        )
    }
    fn choice_rect(&self, row: usize, i: usize) -> Rect {
        let r = Self::setting_rect(row);
        let (w, gap) = match row {
            0 => (54, 6),
            1 => (100, 6),
            _ => (44, 4),
        };
        (CHOICE_X + i as i32 * (w + gap), r.1 + 3, w, r.3 - 6)
    }
    fn done_rect() -> Rect {
        let p = PANEL_RECT;
        (p.0 + p.2 - 108, p.1 + p.3 - 26, 100, 18)
    }
    fn confirm_rect(delete: bool) -> Rect {
        let c = CONFIRM_RECT;
        let x = if delete { c.0 + 64 } else { c.0 + c.2 - 164 };
        (x, c.1 + c.3 - 28, 100, 18)
    }
    fn hit(&self, p: (f64, f64)) -> Option<Hit> {
        if self.confirm.is_some() {
            return [true, false]
                .into_iter()
                .find(|delete| inside(p, Self::confirm_rect(*delete)))
                .map(Hit::Confirm);
        }
        if self.panel.is_some() {
            for row in 0..SETTING_LABELS.len() {
                if !inside(p, Self::setting_rect(row)) {
                    continue;
                }
                return Some(
                    (0..self.choices(row).len())
                        .find(|i| inside(p, self.choice_rect(row, *i)))
                        .map_or(Hit::Setting(row), |i| Hit::Choice(row, i)),
                );
            }
            return inside(p, Self::done_rect()).then_some(Hit::Done);
        }
        for row in 0..LIST_LINES {
            let index = self.scroll + row;
            if index >= self.entries.len() {
                break;
            }
            if inside(p, Self::row_rect(row)) {
                return Some(Hit::Row(index));
            }
        }
        (0..BUTTONS.len())
            .find(|i| inside(p, Self::footer_rect(*i)))
            .map(Hit::Footer)
    }

    // ---- drawing --------------------------------------------------------

    pub fn draw(&self, pixels: &mut [u8], font: &Font) {
        chrome::title_bar(
            pixels,
            font,
            "REPLAYS   |   Mission recordings",
            self.context,
        );
        self.draw_list(pixels, font);
        self.draw_details(pixels, font);
        chrome::message_bar(pixels, font, &fit_middle(font, &self.status(), 620));
        for i in 0..BUTTONS.len() {
            self.draw_button(pixels, font, i);
        }
        if self.panel.is_some() {
            self.draw_panel(pixels, font);
        }
        if let Some(confirm) = &self.confirm {
            self.draw_confirm(pixels, font, confirm);
        }
    }

    fn modal(&self) -> bool {
        self.panel.is_some() || self.confirm.is_some()
    }

    fn draw_list(&self, pixels: &mut [u8], font: &Font) {
        let panel = LIST;
        Canvas(pixels).rect(panel, PANEL);
        let total: u64 = self.entries.iter().map(|e| e.bytes).sum();
        let title = if self.entries.is_empty() {
            "RECORDINGS".to_owned()
        } else {
            format!("RECORDINGS ({}, {})", self.entries.len(), size(total))
        };
        Chrome::text(
            pixels,
            font,
            panel,
            TITLE,
            &title,
            (panel.0 + 6, panel.1 + 5),
        );
        // The legend for the markers, at the top right.
        let right = panel.0 + panel.2 - 6;
        let incomplete = "INCOMPLETE";
        let x = right - text_width(font, incomplete);
        Chrome::text(pixels, font, panel, MUTED, incomplete, (x, panel.1 + 5));
        warning(pixels, (x - 13, panel.1 + 5), WARN, PANEL);
        let kept = "KEPT";
        let x = x - 24 - text_width(font, kept);
        Chrome::text(pixels, font, panel, MUTED, kept, (x, panel.1 + 5));
        lock(pixels, (x - 13, panel.1 + 4), GOOD, PANEL);

        let header = (panel.0 + 2, COLUMN_TOP, panel.2 - 4, 14);
        Canvas(pixels).rect(header, HEADER);
        for (i, title) in COLUMN_TITLES.iter().enumerate() {
            let (x, w) = COLUMNS[i];
            let x = if i == 3 {
                x + w - text_width(font, title)
            } else {
                x
            };
            Chrome::text(
                pixels,
                font,
                header,
                TITLE,
                title,
                (header.0 + x, header.1 + 3),
            );
        }
        if self.entries.is_empty() {
            let lines = [
                "No recordings yet.",
                "",
                "Every flight records itself: fly a Quick Mission or",
                "Free Flight and it appears here when the flight ends.",
            ];
            for (i, text) in lines.iter().enumerate() {
                Chrome::text(
                    pixels,
                    font,
                    panel,
                    WHITE,
                    text,
                    (panel.0 + 12, LIST_TOP + 10 + i as i32 * 15),
                );
            }
        }
        for row in 0..LIST_LINES {
            let index = self.scroll + row;
            if index >= self.entries.len() {
                break;
            }
            self.draw_row(pixels, font, row, index);
        }
        let total = self.entries.len();
        if total > LIST_LINES {
            let track = (
                panel.0 + panel.2 - 4,
                LIST_TOP,
                2,
                LIST_LINES as i32 * LINE_HEIGHT - 1,
            );
            Canvas(pixels).rect(track, HEADER);
            let h = (track.3 * LIST_LINES as i32 / total as i32).max(8);
            let y = track.1 + (track.3 - h) * self.scroll as i32 / (total - LIST_LINES) as i32;
            Canvas(pixels).rect((track.0, y, 2, h), TITLE);
        }
    }

    fn draw_row(&self, pixels: &mut [u8], font: &Font, row: usize, index: usize) {
        let entry = &self.entries[index];
        let r = Self::row_rect(row);
        let selected = index == self.selected;
        let focused = selected && self.focus == Focus::List;
        let on_pale = !selected;
        let fill = if focused {
            FOCUS
        } else if selected {
            SELECTED
        } else if row.is_multiple_of(2) {
            PALE
        } else {
            PALE_ALT
        };
        Canvas(pixels).rect(r, fill);
        if selected {
            Canvas(pixels).rect((r.0, r.1, 3, r.3), GOOD);
        }
        if entry.kept {
            let color = if on_pale { GOOD_INK } else { GOOD };
            lock(pixels, (r.0 + 5, r.1 + 2), color, fill);
        }
        if !complete(entry) {
            let color = if on_pale { WARN_INK } else { WARN };
            warning(pixels, (r.0 + 15, r.1 + 2), color, fill);
        }
        let ink = if on_pale { INK } else { WHITE };
        let length = self.length(entry).map_or_else(|| "?".into(), clock);
        let (result, tone) = result(entry);
        let cells = [
            (started(&entry.name), ink),
            (theater(entry), ink),
            (aircraft(entry), ink),
            (length, ink),
            (result, tone.color(on_pale)),
        ];
        for (i, (text, color)) in cells.iter().enumerate() {
            let (x, w) = COLUMNS[i];
            let text = fit(font, text, w - 2);
            let x = if i == 3 {
                r.0 + x + w - text_width(font, &text)
            } else {
                r.0 + x
            };
            Chrome::text(pixels, font, r, *color, &text, (x, r.1 + 2));
        }
    }

    fn draw_details(&self, pixels: &mut [u8], font: &Font) {
        let panel = DETAILS;
        Canvas(pixels).rect(panel, PANEL);
        Chrome::text(
            pixels,
            font,
            panel,
            TITLE,
            "DETAILS",
            (panel.0 + 6, panel.1 + 5),
        );
        let x = panel.0 + 8;
        let width = panel.2 - 16;
        let Some(entry) = self.current() else {
            let mut y = panel.1 + 26;
            for line in wrap(
                font,
                "Select a recording to see who flew, where and how it ended.",
                width,
                4,
            ) {
                Chrome::text(pixels, font, panel, MUTED, &line, (x, y));
                y += DETAIL_STEP;
            }
            self.draw_folder(pixels, font, y);
            return;
        };
        let (state, state_color) = if !complete(entry) {
            ("INCOMPLETE", WARN)
        } else if entry.kept {
            ("KEPT", GOOD)
        } else {
            ("", MUTED)
        };
        let state_x = panel.0 + panel.2 - 6 - text_width(font, state);
        Chrome::text(
            pixels,
            font,
            panel,
            state_color,
            state,
            (state_x, panel.1 + 5),
        );

        let header = entry.peek.as_ref().ok().map(|p| &p.header);
        let details = self.current_details();
        let reading = || match details {
            None => "Reading...".to_owned(),
            Some(Err(_)) => "Unknown".to_owned(),
            Some(Ok(_)) => String::new(),
        };
        let loaded = match details {
            Some(Ok(details)) => Some(details),
            _ => None,
        };
        let weather = header.map_or_else(
            || "?".to_owned(),
            |h| {
                let seconds = h.world.time_of_day_s.max(0.) as u64;
                let name = if h.world.weather_name.is_empty() {
                    "Unknown".to_owned()
                } else {
                    h.world.weather_name.clone()
                };
                format!(
                    "{name}, {:02}:{:02} local",
                    seconds / 3600 % 24,
                    seconds / 60 % 60
                )
            },
        );
        let wings = loaded.map_or_else(reading, |d| {
            d.sides
                .iter()
                .map(|s| format!("{} {}", s.wings, s.side))
                .collect::<Vec<_>>()
                .join(", ")
        });
        let counts = loaded.map_or_else(reading, |d| {
            let total: usize = d.sides.iter().map(|s| s.aircraft).sum();
            let parts: Vec<String> = d
                .sides
                .iter()
                .map(|s| format!("{} {}", s.aircraft, s.side))
                .collect();
            format!("{total}: {}", parts.join(", "))
        });
        let kills = footer_value(entry, "kills")
            .map(str::to_owned)
            .or_else(|| loaded.map(|d| d.kills.to_string()))
            .unwrap_or_else(reading);
        let first = loaded.and_then(|d| d.ticks).map_or(0, |t| t.0);
        let bookmarks = loaded.map_or_else(reading, |d| match d.bookmarks.len() {
            0 => "None".to_owned(),
            n => {
                let times: Vec<String> = d
                    .bookmarks
                    .iter()
                    .map(|t| clock(t.saturating_sub(first)))
                    .collect();
                format!("{n} at {}", times.join(", "))
            }
        });
        let player = aircraft(entry);
        let lines: Vec<(&str, String)> = vec![
            ("Started", format!("{} UTC", started(&entry.name))),
            (
                "Mission",
                header.map_or_else(|| "?".into(), |h| h.mission.title()),
            ),
            ("Theater", theater(entry)),
            ("Weather", weather),
            ("Player", player),
            ("Wings", wings),
            ("Aircraft", counts),
            (
                "Length",
                self.length(entry).map_or_else(|| "?".into(), clock),
            ),
            ("Result", result_detail(entry)),
            ("Kills", kills),
            ("Bookmarks", bookmarks),
            ("Size", size(entry.bytes)),
        ];
        let mut y = panel.1 + 24;
        let value_x = x + LABEL_W;
        let value_w = panel.0 + panel.2 - 8 - value_x;
        for (label, value) in &lines {
            Chrome::text(
                pixels,
                font,
                panel,
                MUTED,
                &label.to_ascii_uppercase(),
                (x, y),
            );
            for line in wrap(font, value, value_w, 2) {
                Chrome::text(pixels, font, panel, WHITE, &line, (value_x, y));
                y += DETAIL_STEP;
            }
        }
        Chrome::text(pixels, font, panel, MUTED, "FILE", (x, y));
        y += DETAIL_STEP;
        let file = if entry.partial {
            format!("{}.partial", entry.name)
        } else {
            entry.name.clone()
        };
        for line in wrap(font, &file, width, 2) {
            Chrome::text(pixels, font, panel, WHITE, &line, (x, y));
            y += DETAIL_STEP;
        }
        y += 4;
        Canvas(pixels).rect((x, y, width, 1), BUTTON);
        y += 7;
        let state = match (&entry.peek, details) {
            (Err(error), _) => (format!("Unreadable: {error}"), WARN),
            (_, Some(Ok(d))) if d.problems > 0 => (
                format!(
                    "Damaged: {} skipped, the rest plays",
                    plural(d.problems, "part", "parts")
                ),
                WARN,
            ),
            _ if !complete(entry) => (
                "Incomplete: the game stopped before finishing it, so it has no result".into(),
                WARN,
            ),
            _ => ("Finished normally".into(), GOOD),
        };
        let keep = if entry.kept {
            "Kept: auto-delete never removes it"
        } else {
            "Not kept: auto-delete may remove it"
        };
        for (text, color) in [state, (keep.to_owned(), MUTED)] {
            for line in wrap(font, &text, width, 3) {
                Chrome::text(pixels, font, panel, color, &line, (x, y));
                y += DETAIL_STEP;
            }
        }
        self.draw_folder(pixels, font, y);
    }

    /// Where the recordings and their exports are, at the foot of the
    /// details panel when it fits below the text that ends at `end`.
    fn draw_folder(&self, pixels: &mut [u8], font: &Font, end: i32) {
        let Some(library) = &self.library else {
            return;
        };
        let panel = DETAILS;
        let (x, width) = (panel.0 + 8, panel.2 - 16);
        let folder = library.folder().display().to_string();
        let lines = wrap_tail(font, &folder, width, 3);
        let mut y = panel.1 + panel.3 - 6 - (lines.len() as i32 + 1) * DETAIL_STEP;
        if y < end + 4 {
            return;
        }
        Chrome::text(
            pixels,
            font,
            panel,
            MUTED,
            "RECORDINGS AND EXPORTS ARE IN",
            (x, y),
        );
        for line in lines {
            y += DETAIL_STEP;
            Chrome::text(pixels, font, panel, MUTED, &line, (x, y));
        }
    }

    fn draw_button(&self, pixels: &mut [u8], font: &Font, i: usize) {
        let button = BUTTONS[i];
        let r = Self::footer_rect(i);
        if !self.enabled(button) {
            Canvas(pixels).rect(r, HEADER);
            let label = button.label();
            let x = r.0 + (r.2 - text_width(font, label)) / 2;
            Chrome::text(pixels, font, r, MUTED, label, (x, r.1 + 4));
            return;
        }
        let focused = self.focus == Focus::Footer(i) && !self.modal();
        if button != Button::Keep {
            chrome::footer_button(pixels, font, r, button.label(), focused);
            return;
        }
        // Keep shows its state as a tick box.
        Canvas(pixels).rect(r, if focused { FOCUS } else { PALE });
        let color = if focused { WHITE } else { INK };
        let label = button.label();
        let x = r.0 + 6 + (r.2 - 6 - text_width(font, label)) / 2;
        let tick = (x - 14, r.1 + 4, 9, 9);
        Canvas(pixels).outline(tick, color);
        if self.current().is_some_and(|e| e.kept) {
            Canvas(pixels).rect((tick.0 + 2, tick.1 + 2, 5, 5), color);
        }
        Chrome::text(pixels, font, r, color, label, (x, r.1 + 4));
    }

    fn draw_panel(&self, pixels: &mut [u8], font: &Font) {
        dim(pixels);
        let p = PANEL_RECT;
        let focus = self.panel.map_or(0, |panel| panel.focus);
        Canvas(pixels).rect((p.0 + 3, p.1 + 3, p.2, p.3), [12, 16, 22, 255]);
        Canvas(pixels).rect(p, PANEL);
        Canvas(pixels).outline(p, OUTLINE);
        Chrome::text(
            pixels,
            font,
            p,
            TITLE,
            "AUTO-DELETE OLD RECORDINGS",
            (p.0 + 8, p.1 + 7),
        );
        let (state, color) = if self.settings.auto_delete {
            ("ON", GOOD)
        } else {
            ("OFF", MUTED)
        };
        let x = p.0 + p.2 - 8 - text_width(font, state);
        Chrome::text(pixels, font, p, color, state, (x, p.1 + 7));
        for (row, label) in SETTING_LABELS.iter().enumerate() {
            let r = Self::setting_rect(row);
            if focus == row {
                Canvas(pixels).rect(r, FOCUS);
                Canvas(pixels).rect((r.0, r.1, 3, r.3), GOOD);
            }
            let active = self.row_active(row);
            let y = r.1 + (r.3 - font.height as i32) / 2;
            Chrome::text(
                pixels,
                font,
                r,
                if active { WHITE } else { MUTED },
                label,
                (r.0 + 10, y),
            );
            for (i, choice) in self.choices(row).into_iter().enumerate() {
                let c = self.choice_rect(row, i);
                let color = if self.chosen(choice) {
                    Canvas(pixels).rect(c, if active { PALE } else { BUTTON });
                    if active { INK } else { WHITE }
                } else {
                    Canvas(pixels).outline(c, if active { OUTLINE } else { HEADER });
                    if active { WHITE } else { MUTED }
                };
                let label = fit(font, &choice.label(), c.2 - 4);
                let x = c.0 + (c.2 - text_width(font, &label)) / 2;
                let y = c.1 + (c.3 - font.height as i32) / 2;
                Chrome::text(pixels, font, c, color, &label, (x, y));
            }
        }
        let x = p.0 + 12;
        let width = p.2 - 24;
        let mut y = SETTING_TOP + SETTING_LABELS.len() as i32 * SETTING_STEP + 4;
        let plan = match self.plan {
            _ if !self.settings.auto_delete => None,
            Some(0) => Some("The next cleanup deletes nothing.".to_owned()),
            Some(n) => Some(format!(
                "The next cleanup deletes {}.",
                plural(n, "recording", "recordings")
            )),
            None => None,
        };
        let notes = [
            (Some(self.rule_text()), WHITE),
            (plan, TITLE),
            (
                Some(
                    "Cleanup runs when a flight ends and when this screen opens. Changes are saved at once."
                        .to_owned(),
                ),
                MUTED,
            ),
        ];
        for (text, color) in notes {
            let Some(text) = text else {
                continue;
            };
            for line in wrap(font, &text, width, 2) {
                Chrome::text(pixels, font, p, color, &line, (x, y));
                y += DETAIL_STEP;
            }
        }
        chrome::footer_button(
            pixels,
            font,
            Self::done_rect(),
            "Done",
            focus == SETTING_LABELS.len(),
        );
    }

    fn draw_confirm(&self, pixels: &mut [u8], font: &Font, confirm: &Confirm) {
        dim(pixels);
        let c = CONFIRM_RECT;
        Canvas(pixels).rect((c.0 + 3, c.1 + 3, c.2, c.3), [12, 16, 22, 255]);
        Canvas(pixels).rect(c, PANEL);
        Canvas(pixels).outline(c, OUTLINE);
        let (x, width) = (c.0 + 10, c.2 - 20);
        Chrome::text(
            pixels,
            font,
            c,
            TITLE,
            "DELETE THIS RECORDING?",
            (x, c.1 + 8),
        );
        let Some(entry) = self.entries.iter().find(|e| e.path == confirm.path) else {
            return;
        };
        Chrome::text(
            pixels,
            font,
            c,
            WHITE,
            &fit(font, &entry.name, width),
            (x, c.1 + 26),
        );
        let about = format!(
            "{} UTC, {}, {}",
            started(&entry.name),
            theater(entry),
            aircraft(entry)
        );
        Chrome::text(
            pixels,
            font,
            c,
            MUTED,
            &fit(font, &about, width),
            (x, c.1 + 40),
        );
        let mut notes = Vec::new();
        if entry.kept {
            notes.push("It is marked Keep; deleting removes it anyway.");
        }
        notes.push("It cannot be undone. Its Tacview file and debug log stay.");
        let mut y = c.1 + 58;
        for note in notes {
            for line in wrap(font, note, width, 2) {
                Chrome::text(pixels, font, c, TITLE, &line, (x, y));
                y += DETAIL_STEP;
            }
        }
        for delete in [true, false] {
            chrome::footer_button(
                pixels,
                font,
                Self::confirm_rect(delete),
                if delete { "Delete" } else { "Cancel" },
                confirm.delete == delete,
            );
        }
    }
}

/// The synthetic recordings `--snapshot-state replays` shows.
mod preview {
    use super::{Details, SideCount};
    use crate::replay::library::Entry;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use tore_replay::{Footer, Header, MissionKind, Peek};

    struct Sample {
        name: &'static str,
        theater: (&'static str, &'static str),
        weather: &'static str,
        aircraft: &'static str,
        mission: MissionKind,
        seconds: u64,
        /// End, outcome, player and kills; `None` for an unfinished file.
        result: Option<(&'static str, Option<&'static str>, &'static str, u32)>,
        kept: bool,
    }

    pub(super) fn entries() -> (Vec<Entry>, HashMap<PathBuf, Result<Details, String>>) {
        use MissionKind::{FreeFlight, QuickMission};
        let samples = [
            Sample {
                name: "2026-09-26_1540_UKR_F18.tore-replay",
                theater: ("UKR", "Ukraine"),
                weather: "clear",
                aircraft: "F18.PT",
                mission: QuickMission,
                seconds: 612,
                result: Some(("end mission", Some("success"), "alive", 3)),
                kept: false,
            },
            Sample {
                name: "2026-09-26_1512_KURILE_FAXX.tore-replay",
                theater: ("KURILE", "Kuril Islands"),
                weather: "cloudy",
                aircraft: "faxx",
                mission: QuickMission,
                seconds: 845,
                result: Some(("end mission", Some("failure"), "ejected", 1)),
                kept: true,
            },
            Sample {
                name: "2026-09-26_1458_UKR_F18-2.tore-replay",
                theater: ("UKR", "Ukraine"),
                weather: "dawn",
                aircraft: "F18.PT",
                mission: FreeFlight,
                seconds: 187,
                result: Some(("end flight", None, "alive", 0)),
                kept: false,
            },
            Sample {
                name: "2026-09-26_1458_UKR_F18.tore-replay",
                theater: ("UKR", "Ukraine"),
                weather: "clear",
                aircraft: "F18.PT",
                mission: FreeFlight,
                seconds: 44,
                result: Some(("restart", None, "alive", 0)),
                kept: false,
            },
            Sample {
                name: "2026-09-25_2231_BAL_RAFALE.tore-replay",
                theater: ("BAL", "The Baltics"),
                weather: "night",
                aircraft: "RAFALE.PT",
                mission: QuickMission,
                seconds: 1_322,
                result: None,
                kept: false,
            },
            Sample {
                name: "2026-09-25_2140_NSK_SU27.tore-replay",
                theater: ("NSK", "North and South Korea"),
                weather: "foggy",
                aircraft: "SU27.PT",
                mission: QuickMission,
                seconds: 903,
                result: Some(("end mission", Some("success"), "alive", 5)),
                kept: true,
            },
            Sample {
                name: "2026-09-25_1907_EGY_F14.tore-replay",
                theater: ("EGY", "Egypt"),
                weather: "sunset",
                aircraft: "F14.PT",
                mission: QuickMission,
                seconds: 3_725,
                result: Some(("exit", Some("failure"), "alive", 2)),
                kept: false,
            },
            Sample {
                name: "2026-09-24_1616_TVIET_MIG29.tore-replay",
                theater: ("TVIET", "North Vietnam"),
                weather: "clear",
                aircraft: "MIG29.PT",
                mission: QuickMission,
                seconds: 540,
                result: Some(("end mission", Some("failure"), "dead", 0)),
                kept: false,
            },
            Sample {
                name: "2026-09-24_0902_GRE_A4E.tore-replay",
                theater: ("GRE", "Greece"),
                weather: "cloudy",
                aircraft: "A4E.PT",
                mission: FreeFlight,
                seconds: 1_096,
                result: Some(("exit", None, "alive", 0)),
                kept: false,
            },
            Sample {
                name: "2026-09-23_2012_PGU_F22.tore-replay",
                theater: ("PGU", "Persian Gulf"),
                weather: "clear",
                aircraft: "F22.PT",
                mission: QuickMission,
                seconds: 760,
                result: Some(("end mission", Some("success"), "alive", 4)),
                kept: false,
            },
        ];
        let mut entries = Vec::new();
        for s in samples {
            let ticks = s.seconds * tore_replay::TICKS_PER_SECOND;
            let header = Header {
                recorded_at: format!(
                    "{}T{}:{}:00Z",
                    &s.name[..10],
                    &s.name[11..13],
                    &s.name[13..15]
                ),
                mission: s.mission,
                world: tore_replay::model::World {
                    theater: s.theater.0.into(),
                    theater_name: s.theater.1.into(),
                    layout: format!("{}.MM", s.theater.0),
                    weather: Some(0),
                    weather_name: s.weather.into(),
                    time_of_day_s: 12. * 3600.,
                    ..Default::default()
                },
                extra: vec![("player.aircraft".into(), s.aircraft.into())],
                ..Header::default()
            };
            let footer = s.result.map(|(end, outcome, player, kills)| {
                let mut result = vec![("end".to_owned(), end.to_owned())];
                if let Some(outcome) = outcome {
                    result.push(("outcome".into(), outcome.into()));
                    result.push(("player".into(), player.into()));
                }
                result.push(("kills".into(), kills.to_string()));
                Footer {
                    end_tick: ticks - 1,
                    result,
                }
            });
            // About the size a real recording of this length has.
            let bytes = ticks * 60 + 9_000;
            entries.push(Entry {
                path: PathBuf::from("replays").join(if footer.is_some() {
                    s.name.to_owned()
                } else {
                    format!("{}.partial", s.name)
                }),
                name: s.name.into(),
                bytes,
                partial: footer.is_none(),
                kept: s.kept,
                peek: Ok(Peek {
                    complete: footer.is_some(),
                    ticks: footer.as_ref().map(|_| (0, ticks - 1)),
                    frames: if footer.is_some() { ticks } else { 0 },
                    file_bytes: bytes,
                    footer,
                    header,
                }),
            });
        }
        // An unreadable file keeps its place by name.
        entries.push(Entry {
            path: PathBuf::from("replays/2026-09-23_1105_UKR_F18.tore-replay"),
            name: "2026-09-23_1105_UKR_F18.tore-replay".into(),
            bytes: 4_096,
            partial: false,
            kept: false,
            peek: Err("damaged recording: a chunk checksum does not match".into()),
        });
        let details = Details {
            player: Some("F/A-XX".into()),
            sides: vec![
                SideCount {
                    side: "friendly",
                    wings: 2,
                    aircraft: 6,
                },
                SideCount {
                    side: "enemy",
                    wings: 2,
                    aircraft: 8,
                },
            ],
            kills: 1,
            bookmarks: vec![4_932, 61_224],
            ticks: Some((0, 845 * tore_replay::TICKS_PER_SECOND - 1)),
            problems: 0,
        };
        let details = HashMap::from([(entries[1].path.clone(), Ok(details))]);
        (entries, details)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::tests::TempDir;
    use tore_replay::{
        AircraftFlags, AircraftInfo, AircraftState, Event, Footer, Frame, Header, MissionKind,
        Writer,
    };

    /// A synthetic font: every byte a 5 pixel advance with one dot.
    fn font() -> Font {
        Font {
            height: 8,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 5,
                    pixels: vec![(1, 3)],
                })
                .collect(),
        }
    }
    fn center(r: Rect) -> (f64, f64) {
        ((r.0 + r.2 / 2) as f64, (r.1 + r.3 / 2) as f64)
    }
    fn click(s: &mut Replays, r: Rect) -> Outcome {
        s.pointer(Some(center(r)), true);
        s.pointer(Some(center(r)), false)
    }
    fn press(s: &mut Replays, key: &str) -> Outcome {
        s.key(key, false, false)
    }
    fn names(s: &Replays) -> Vec<&str> {
        s.entries.iter().map(|e| e.name.as_str()).collect()
    }
    /// Polls until the background work is done.
    fn settle(s: &mut Replays) {
        let start = Instant::now();
        while s.poll() {
            assert!(
                start.elapsed() < Duration::from_secs(60),
                "work never finished"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    /// Writes two seconds of a synthetic Quick Mission into the library:
    /// the player and one enemy in each of two wings, a bookmark at tick 60
    /// and the player's kill at tick 120. An unfinished recording keeps its
    /// `.partial` name.
    fn record(library: &Library, name: &str, finished: bool) -> PathBuf {
        std::fs::create_dir_all(library.folder()).unwrap();
        let header = Header {
            mission: MissionKind::QuickMission,
            world: tore_replay::model::World {
                theater: "UKR".into(),
                theater_name: "Ukraine".into(),
                layout: "UKR.MM".into(),
                weather: Some(0),
                weather_name: "clear".into(),
                time_of_day_s: 43_200.,
                ..Default::default()
            },
            extra: vec![("player.aircraft".into(), "F18.PT".into())],
            ..Header::default()
        };
        let path = library.folder().join(name);
        let mut writer = Writer::create(&path, &header).unwrap();
        for (id, side, wing, label) in [
            (0, tore_replay::Side::Friendly, 1, "You"),
            (1, tore_replay::Side::Enemy, 1, "Enemy 1-1"),
            (2, tore_replay::Side::Enemy, 2, "Enemy 2-1"),
        ] {
            writer
                .register_aircraft(&AircraftInfo {
                    id,
                    pt: "F18.PT".into(),
                    name: "F/A-18D".into(),
                    label: label.into(),
                    side,
                    wing,
                    member: 1,
                    human: id == 0,
                    ..AircraftInfo::default()
                })
                .unwrap();
        }
        for tick in 0..240 {
            let mut frame = Frame {
                tick,
                ..Frame::default()
            };
            for id in 0..3 {
                frame.aircraft.push(AircraftState {
                    id,
                    position: [4_000. * f64::from(id) + tick as f64, 5_000., 0.],
                    velocity: [120., 0., 0.],
                    airspeed: 120.,
                    g: 1.,
                    flags: AircraftFlags {
                        alive: true,
                        airborne: true,
                        engine_on: true,
                        ..AircraftFlags::default()
                    },
                    hp: 100,
                    max_hp: 100,
                    fuel_lb: 5_000.,
                    ..AircraftState::default()
                });
            }
            if tick == 60 {
                frame.events.push(
                    Event::new(kind::PLAYER_BOOKMARK)
                        .with_subject(0)
                        .with_text("Bookmark 1"),
                );
            }
            if tick == 120 {
                frame.events.push(
                    Event::new(kind::COMBAT_DESTROYED)
                        .with_subject(1)
                        .with_object(0),
                );
            }
            writer.push(&frame).unwrap();
        }
        if !finished {
            drop(writer);
            return tore_replay::partial_path(&path);
        }
        writer
            .finish(&Footer {
                end_tick: 239,
                result: vec![
                    ("end".into(), "end mission".into()),
                    ("outcome".into(), "success".into()),
                    ("player".into(), "alive".into()),
                    ("kills".into(), "1".into()),
                ],
            })
            .unwrap()
    }

    /// A screen over `n` synthetic recordings, one a minute apart.
    fn many(n: usize) -> Replays {
        let (samples, _) = preview::entries();
        let entries = (0..n)
            .map(|i| {
                let mut entry = samples[0].clone();
                entry.name = format!(
                    "2026-09-{:02}_{:02}{:02}_UKR_F18.tore-replay",
                    1 + i / 1440,
                    i / 60 % 24,
                    i % 60
                );
                entry.path = PathBuf::from("replays").join(&entry.name);
                entry
            })
            .collect();
        Replays::with_entries(None, entries, Settings::default(), "Main menu", HINT.into())
    }

    #[test]
    fn text_reads_naturally() {
        assert_eq!(
            started("2026-09-26_1540_UKR_F18-2.tore-replay"),
            "2026-09-26 15:40"
        );
        assert_eq!(started("notes.txt"), "?");
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(120 * 754), "12:34");
        assert_eq!(clock(120 * 3_725), "1:02:05");
        assert_eq!(size(69_270), "68 KB");
        assert_eq!(size(20 * 1024 * 1024 + 400 * 1024), "20.4 MB");
        let name = "2026-09-26_1540_UKR1_FAXX-12.tore-replay";
        assert_eq!(name_part(name, 2), Some("UKR1"));
        assert_eq!(name_part(name, 3), Some("FAXX"));
        let font = font();
        // The start and the file name survive the cut.
        let fitted = fit_middle(
            &font,
            "Tacview file written: /a/b/c/d/e/f/name.txt.acmi",
            150,
        );
        assert!(text_width(&font, &fitted) <= 150, "{fitted}");
        assert!(fitted.starts_with("Tacview"), "{fitted}");
        assert!(fitted.ends_with("name.txt.acmi"), "{fitted}");
        // Ten characters a line: after spaces and separators, or inside a
        // word too long for a line.
        assert_eq!(
            wrap(&font, "one two three four", 50, 3),
            ["one two", "three four"]
        );
        assert_eq!(
            wrap(&font, "/data/replays/2026-09-26_1540", 50, 4),
            ["/data/", "replays/", "2026-09-26", "_1540"]
        );
        assert_eq!(
            wrap(&font, "one two three four five six", 50, 2),
            ["one two", "three fo.."]
        );
        // A long path keeps its end instead.
        let path = "/home/pilot/.local/share/T.O.R.E-Fighters/replays";
        let lines = wrap_tail(&font, path, 50, 2);
        assert_eq!(lines, ["..ghters/", "replays"]);
        assert_eq!(
            wrap_tail(&font, "/data/replays", 50, 2),
            ["/data/", "replays"]
        );
        for line in wrap_tail(&font, path, 50, 3) {
            assert!(text_width(&font, &line) <= 50, "{line}");
        }
    }

    #[test]
    fn lists_newest_first_with_same_minute_flights_in_order() {
        let dir = TempDir::new("screen-order");
        let library = Library::new(dir.path());
        for name in [
            "2026-09-26_1540_UKR_F18.tore-replay",
            "2026-09-26_1540_UKR_F18-2.tore-replay",
            "2026-09-26_1540_UKR_F18-10.tore-replay",
            "2026-09-25_2359_KURILE_FAXX.tore-replay",
        ] {
            record(&library, name, true);
        }
        record(&library, "2026-09-26_1600_BAL_RAFALE.tore-replay", false);
        std::fs::write(library.folder().join("notes.txt"), "not a recording").unwrap();
        let expected = [
            "2026-09-26_1600_BAL_RAFALE.tore-replay",
            "2026-09-26_1540_UKR_F18-10.tore-replay",
            "2026-09-26_1540_UKR_F18-2.tore-replay",
            "2026-09-26_1540_UKR_F18.tore-replay",
            "2026-09-25_2359_KURILE_FAXX.tore-replay",
        ];
        let s = Replays::open(Some(library.clone()), "Main menu");
        assert_eq!(names(&s), expected);
        assert!(s.entries[0].partial && !complete(&s.entries[0]));
        assert_eq!(result(&s.entries[0]), ("Incomplete".into(), Tone::Warn));
        assert_eq!(result(&s.entries[1]), ("Success".into(), Tone::Good));
        assert_eq!(s.message, HINT, "nothing was old enough to delete");
        // The library itself lists in the same order.
        let listed: Vec<String> = library.list().into_iter().map(|e| e.name).collect();
        assert_eq!(listed, expected);
        // A list handed over in any order is shown newest first.
        let (mut entries, _) = preview::entries();
        entries.reverse();
        let s = Replays::with_entries(None, entries, Settings::default(), "Main menu", HINT.into());
        assert!(
            s.entries
                .windows(2)
                .all(|w| library::order_key(&w[0].name) > library::order_key(&w[1].name)),
            "{:?}",
            names(&s)
        );
    }

    #[test]
    fn hit_tests_find_rows_buttons_and_dialog_controls() {
        let mut s = Replays::preview("Main menu");
        let rows = s.entries.len();
        assert_eq!(s.hit(center(Replays::row_rect(0))), Some(Hit::Row(0)));
        assert_eq!(
            s.hit(center(Replays::row_rect(rows - 1))),
            Some(Hit::Row(rows - 1))
        );
        assert_eq!(
            s.hit(center(Replays::row_rect(rows))),
            None,
            "past the list"
        );
        for i in 0..BUTTONS.len() {
            assert_eq!(s.hit(center(Replays::footer_rect(i))), Some(Hit::Footer(i)));
        }
        // Between buttons, the title, the column titles and the details.
        let (x, y, w, _) = Replays::footer_rect(0);
        assert_eq!(s.hit(((x + w + 2) as f64, (y + 5) as f64)), None);
        assert_eq!(s.hit((20., 8.)), None);
        assert_eq!(s.hit((60., f64::from(COLUMN_TOP + 5))), None);
        assert_eq!(s.hit((500., 200.)), None);
        // Rows follow the scroll.
        let mut long = many(40);
        long.scroll = 5;
        assert_eq!(long.hit(center(Replays::row_rect(0))), Some(Hit::Row(5)));
        assert_eq!(
            long.hit(center(Replays::row_rect(LIST_LINES - 1))),
            Some(Hit::Row(5 + LIST_LINES - 1))
        );
        // The settings panel takes every click.
        s.panel = Some(Panel { focus: 0 });
        assert_eq!(s.hit(center(Replays::row_rect(0))), None);
        assert_eq!(s.hit(center(Replays::footer_rect(6))), None);
        for row in 0..SETTING_LABELS.len() {
            for i in 0..s.choices(row).len() {
                assert_eq!(
                    s.hit(center(s.choice_rect(row, i))),
                    Some(Hit::Choice(row, i))
                );
            }
            let r = Replays::setting_rect(row);
            assert_eq!(
                s.hit(((r.0 + 20) as f64, (r.1 + r.3 / 2) as f64)),
                Some(Hit::Setting(row))
            );
        }
        assert_eq!(s.hit(center(Replays::done_rect())), Some(Hit::Done));
        // So does the delete confirmation.
        s.panel = None;
        s.run(Button::Delete);
        assert_eq!(
            s.hit(center(Replays::confirm_rect(true))),
            Some(Hit::Confirm(true))
        );
        assert_eq!(
            s.hit(center(Replays::confirm_rect(false))),
            Some(Hit::Confirm(false))
        );
        assert_eq!(s.hit(center(Replays::row_rect(0))), None);
        assert_eq!(s.hit(center(Replays::footer_rect(1))), None);
    }

    #[test]
    fn keyboard_moves_through_the_list_and_the_buttons() {
        let mut s = Replays::preview("Main menu");
        let last = s.entries.len() - 1;
        assert_eq!((s.focus, s.selected), (Focus::List, 1));
        press(&mut s, "ArrowUp");
        press(&mut s, "ArrowUp");
        assert_eq!(s.selected, 0, "the top row stops the cursor");
        press(&mut s, "End");
        assert_eq!(s.selected, last);
        press(&mut s, "ArrowDown");
        assert_eq!(s.focus, Focus::Footer(0), "down from the last row");
        press(&mut s, "ArrowRight");
        press(&mut s, "ArrowRight");
        assert_eq!(s.focus, Focus::Footer(2));
        press(&mut s, "ArrowLeft");
        assert_eq!(s.focus, Focus::Footer(1));
        press(&mut s, "ArrowUp");
        assert_eq!((s.focus, s.selected), (Focus::List, last));
        press(&mut s, "Home");
        assert_eq!(s.selected, 0);
        // Tab walks the list and every button, and wraps; Shift+Tab goes back.
        let mut seen = vec![s.focus];
        for _ in 0..BUTTONS.len() {
            press(&mut s, "Tab");
            seen.push(s.focus);
        }
        let expected: Vec<Focus> = std::iter::once(Focus::List)
            .chain((0..BUTTONS.len()).map(Focus::Footer))
            .collect();
        assert_eq!(seen, expected);
        press(&mut s, "Tab");
        assert_eq!(s.focus, Focus::List);
        s.key("Tab", true, false);
        assert_eq!(s.focus, Focus::Footer(BUTTONS.len() - 1));
        // Page keys, which repeat, and the wheel on a long list.
        let mut s = many(60);
        press(&mut s, "PageDown");
        assert_eq!(s.selected, LIST_LINES);
        assert_eq!(s.scroll, 1, "the cursor stays in view");
        s.key("PageDown", false, true);
        press(&mut s, "PageDown");
        assert_eq!(s.selected, 59);
        assert_eq!(s.scroll, 60 - LIST_LINES);
        press(&mut s, "PageUp");
        assert_eq!(s.selected, 59 - LIST_LINES);
        s.wheel(100);
        assert_eq!(s.scroll, 0);
        s.wheel(-2);
        assert_eq!(s.scroll, 6);
        s.wheel(-100);
        assert_eq!(s.scroll, 60 - LIST_LINES);
        assert_eq!(s.selected, 59 - LIST_LINES, "the wheel only scrolls");
        // Esc leaves.
        assert_eq!(press(&mut s, "Escape"), Outcome::Close);
    }

    #[test]
    fn watch_sends_the_selected_recording() {
        let mut s = Replays::preview("Main menu");
        let path = s.entries[1].path.clone();
        assert_eq!(press(&mut s, "Enter"), Outcome::Watch(path.clone()));
        assert_eq!(s.key("Enter", false, true), Outcome::None, "a held key");
        assert_eq!(click(&mut s, Replays::footer_rect(0)), Outcome::Watch(path));
        // A click selects; a second click on the same row watches it.
        let row = Replays::row_rect(2);
        assert_eq!(click(&mut s, row), Outcome::Changed);
        assert_eq!((s.selected, s.focus), (2, Focus::List));
        assert_eq!(
            click(&mut s, row),
            Outcome::Watch(s.entries[2].path.clone())
        );
        assert_eq!(
            click(&mut s, row),
            Outcome::Changed,
            "a third click starts over"
        );
        // Press and release must land on the same row.
        s.pointer(Some(center(Replays::row_rect(3))), true);
        assert_eq!(
            s.pointer(Some(center(Replays::row_rect(4))), false),
            Outcome::None
        );
        assert_eq!(s.selected, 2);
        // An unreadable recording says why instead.
        press(&mut s, "End");
        assert!(s.entries[s.selected].peek.is_err());
        assert_eq!(press(&mut s, "Enter"), Outcome::Changed);
        assert!(s.message.contains("cannot be read"), "{}", s.message);
    }

    #[test]
    fn the_screen_waits_under_the_viewer_and_lists_again_after() {
        // Without a viewer, Watch says so the next time the screen draws.
        let mut s = Replays::preview("Main menu");
        assert!(matches!(press(&mut s, "Enter"), Outcome::Watch(_)));
        assert!(!s.shown(None));
        assert_eq!(
            s.message,
            "The replay viewer is not available in this build."
        );
        // A viewer that cannot open the file explains through a notice.
        press(&mut s, "Enter");
        assert!(!s.shown(Some("Could not open the replay: damaged".into())));
        assert_eq!(s.message, "Could not open the replay: damaged");
        // Drawing again changes nothing.
        assert!(!s.shown(None));
        assert_eq!(s.message, "Could not open the replay: damaged");
        // Back from the viewer: the list is read again, the selection kept.
        let dir = TempDir::new("screen-return");
        let library = Library::new(dir.path());
        let watched = record(&library, "2026-09-25_1200_UKR_F18.tore-replay", true);
        record(&library, "2026-09-26_1200_UKR_F18.tore-replay", true);
        let mut s = Replays::open(Some(library.clone()), "Main menu");
        press(&mut s, "ArrowDown");
        assert_eq!(press(&mut s, "Enter"), Outcome::Watch(watched.clone()));
        s.covered();
        record(&library, "2026-09-27_1200_UKR_F18.tore-replay", true);
        let message = s.message.clone();
        assert!(s.shown(None));
        assert_eq!(s.message, message);
        s.refresh();
        assert_eq!(s.entries.len(), 3);
        assert_eq!(s.current().map(|e| &e.path), Some(&watched));
        assert_eq!(s.selected, 2);
        assert!(!s.shown(None), "only once");
        // A refresh by the host itself clears the marks too.
        press(&mut s, "Enter");
        s.covered();
        s.refresh();
        assert!(!s.shown(None));
    }

    #[test]
    fn keep_toggles_and_is_saved() {
        let dir = TempDir::new("screen-keep");
        let library = Library::new(dir.path());
        let old = "2026-09-25_1200_UKR_F18.tore-replay";
        let new = "2026-09-26_1200_UKR_F18.tore-replay";
        record(&library, old, true);
        record(&library, new, true);
        let mut s = Replays::open(Some(library.clone()), "Main menu");
        assert_eq!(names(&s), [new, old]);
        assert_eq!(s.plan, Some(0));
        press(&mut s, "ArrowDown");
        assert_eq!(click(&mut s, Replays::footer_rect(1)), Outcome::Changed);
        assert!(s.entries[1].kept);
        assert!(library.settings().kept.contains(old));
        assert!(s.message.starts_with("Keeping"), "{}", s.message);
        // Kept survives a fresh listing and keep-the-last-one cleanup.
        let settings = Settings {
            keep_last: 1,
            ..library.settings()
        };
        library.save_settings(&settings).unwrap();
        let mut s = Replays::open(Some(library.clone()), "Main menu");
        assert_eq!(names(&s), [new, old]);
        assert!(s.entries[1].kept && !s.entries[0].kept);
        // Stop keeping it from the keyboard: Tab to the button, Enter.
        press(&mut s, "ArrowDown");
        press(&mut s, "Tab");
        press(&mut s, "Tab");
        assert_eq!(s.focus, Focus::Footer(1));
        assert_eq!(press(&mut s, "Enter"), Outcome::Changed);
        assert!(!s.entries[1].kept);
        assert!(library.settings().kept.is_empty());
        assert_eq!(s.plan, Some(1), "now the next cleanup would take it");
        // The next opening applies the rule.
        let s = Replays::open(Some(library.clone()), "Main menu");
        assert_eq!(names(&s), [new]);
        assert!(
            s.message.contains("removed 1 older recording"),
            "{}",
            s.message
        );
    }

    #[test]
    fn delete_asks_first_then_removes_the_file() {
        let dir = TempDir::new("screen-delete");
        let library = Library::new(dir.path());
        let first = record(&library, "2026-09-25_1200_UKR_F18.tore-replay", true);
        let second = record(&library, "2026-09-26_1200_UKR_F18.tore-replay", true);
        let mut settings = library.settings();
        settings
            .kept
            .insert("2026-09-25_1200_UKR_F18.tore-replay".into());
        library.save_settings(&settings).unwrap();
        let mut s = Replays::open(Some(library.clone()), "Main menu");
        press(&mut s, "ArrowDown");
        assert_eq!(s.key("Delete", false, true), Outcome::None, "a held key");
        assert!(s.confirm.is_none());
        // Esc, and Cancel, keep it.
        assert_eq!(press(&mut s, "Delete"), Outcome::Changed);
        assert!(s.confirm.as_ref().is_some_and(|c| c.delete));
        assert_eq!(press(&mut s, "Escape"), Outcome::Changed);
        assert!(s.confirm.is_none() && first.exists());
        press(&mut s, "Backspace");
        press(&mut s, "ArrowRight");
        assert_eq!(press(&mut s, "Enter"), Outcome::Changed);
        assert!(s.confirm.is_none() && first.exists());
        assert_eq!(click(&mut s, Replays::footer_rect(2)), Outcome::Changed);
        assert_eq!(
            click(&mut s, Replays::confirm_rect(false)),
            Outcome::Changed
        );
        assert!(first.exists());
        // Delete, then Enter: gone from the disk, the list and the kept set.
        press(&mut s, "Delete");
        assert_eq!(press(&mut s, "Enter"), Outcome::Changed);
        assert!(!first.exists() && second.exists());
        assert_eq!(names(&s), ["2026-09-26_1200_UKR_F18.tore-replay"]);
        assert!(library.settings().kept.is_empty());
        assert_eq!(s.selected, 0);
        assert!(s.message.starts_with("Deleted"), "{}", s.message);
        // The last one by mouse; then only Auto-delete and Back remain.
        click(&mut s, Replays::footer_rect(2));
        assert_eq!(click(&mut s, Replays::confirm_rect(true)), Outcome::Changed);
        assert!(!second.exists());
        assert!(s.entries.is_empty());
        assert_eq!(s.focus, Focus::Footer(6));
        assert_eq!(s.order(), [Focus::Footer(5), Focus::Footer(6)]);
        assert_eq!(click(&mut s, Replays::footer_rect(0)), Outcome::None);
        assert_eq!(press(&mut s, "Delete"), Outcome::Changed);
        assert!(s.confirm.is_none());
        assert!(s.message.starts_with("There are no recordings"));
        assert_eq!(press(&mut s, "Enter"), Outcome::Close, "Back");
    }

    #[test]
    fn auto_delete_settings_change_and_are_saved() {
        let dir = TempDir::new("screen-settings");
        let library = Library::new(dir.path());
        for day in 1..=7 {
            record(
                &library,
                &format!("2026-09-{day:02}_1200_UKR_F18.tore-replay"),
                true,
            );
        }
        let mut s = Replays::open(Some(library.clone()), "Main menu");
        assert_eq!(click(&mut s, Replays::footer_rect(5)), Outcome::Changed);
        assert_eq!(s.panel, Some(Panel { focus: 0 }));
        assert_eq!(s.plan, Some(0), "keep the last 20 of 7");
        // Keep the last 5, by mouse: two older recordings would go.
        let five = s.choice_rect(2, 0);
        assert_eq!(click(&mut s, five), Outcome::Changed);
        assert_eq!(library.settings().keep_last, 5);
        assert_eq!(s.plan, Some(2));
        assert!(s.message.contains("newest 5"), "{}", s.message);
        // Off, by keyboard on the first row.
        s.panel = Some(Panel { focus: 0 });
        assert_eq!(press(&mut s, "ArrowRight"), Outcome::Changed);
        assert!(!library.settings().auto_delete);
        assert_eq!(s.plan, Some(0));
        assert_eq!(
            press(&mut s, "ArrowRight"),
            Outcome::None,
            "stops at the end"
        );
        press(&mut s, "Enter");
        assert!(library.settings().auto_delete, "Enter cycles");
        // An age picks its rule too.
        press(&mut s, "ArrowDown");
        press(&mut s, "ArrowDown");
        press(&mut s, "ArrowDown");
        assert_eq!(s.panel, Some(Panel { focus: 3 }));
        press(&mut s, "ArrowRight");
        let saved = library.settings();
        assert_eq!((saved.rule, saved.older_than_days), (Rule::OlderThan, 90));
        press(&mut s, "ArrowLeft");
        press(&mut s, "ArrowLeft");
        assert_eq!(library.settings().older_than_days, 14);
        // The rule row switches back without touching the numbers.
        let keep = s.choice_rect(1, 0);
        click(&mut s, keep);
        let saved = library.settings();
        assert_eq!(
            (saved.rule, saved.keep_last, saved.older_than_days),
            (Rule::KeepLast, 5, 14)
        );
        assert_eq!(s.settings, saved);
        // The wheel moves between rows; Done and Esc close.
        s.wheel(-10);
        assert_eq!(s.panel, Some(Panel { focus: 4 }));
        assert_eq!(press(&mut s, "Enter"), Outcome::Changed);
        assert!(s.panel.is_none());
        s.focus = Focus::Footer(5);
        press(&mut s, "Enter");
        assert!(s.panel.is_some());
        assert_eq!(press(&mut s, "Escape"), Outcome::Changed);
        assert!(s.panel.is_none());
        assert_eq!(press(&mut s, "Escape"), Outcome::Close);
        // A saved number the choices lack is shown and chosen.
        let settings = Settings {
            keep_last: 37,
            ..library.settings()
        };
        library.save_settings(&settings).unwrap();
        let s = Replays::open(Some(library.clone()), "Main menu");
        let choices = s.choices(2);
        assert_eq!(choices.len(), 6);
        assert!(s.chosen(Choice::KeepLast(37)));
        assert_eq!(choices[3], Choice::KeepLast(37), "in order");
    }

    #[test]
    fn details_and_exports_are_read_in_the_background() {
        let dir = TempDir::new("screen-exports");
        let library = Library::new(dir.path());
        let path = record(&library, "2026-09-26_1540_UKR_F18.tore-replay", true);
        let mut s = Replays::open(Some(library.clone()), "Main menu");
        // Details: sides, the kill and the bookmark from the events.
        settle(&mut s);
        let details = s.current_details().unwrap().as_ref().unwrap().clone();
        assert_eq!(details.player.as_deref(), Some("F/A-18D"));
        let side = |side, wings, aircraft| SideCount {
            side,
            wings,
            aircraft,
        };
        assert_eq!(details.sides, [side("friendly", 1, 1), side("enemy", 2, 2)]);
        assert_eq!((details.kills, details.bookmarks), (1, vec![60]));
        assert_eq!(details.ticks, Some((0, 239)));
        // Tacview, then a second export while the first is still unclaimed.
        assert_eq!(click(&mut s, Replays::footer_rect(3)), Outcome::Changed);
        assert!(
            s.status().starts_with("Writing the Tacview file"),
            "{}",
            s.status()
        );
        click(&mut s, Replays::footer_rect(4));
        assert!(s.message.contains("one export at a time"), "{}", s.message);
        // Deleting a recording being exported waits too.
        press(&mut s, "Delete");
        assert!(s.confirm.is_none());
        settle(&mut s);
        let acmi = library.folder().join("2026-09-26_1540_UKR_F18.txt.acmi");
        assert_eq!(
            s.message,
            format!("Tacview file written: {}", acmi.display())
        );
        assert!(
            std::fs::read_to_string(&acmi)
                .unwrap()
                .starts_with("FileType=text/acmi/tacview")
        );
        // The debug log goes into a folder beside the recording.
        click(&mut s, Replays::footer_rect(4));
        settle(&mut s);
        let folder = library.folder().join("2026-09-26_1540_UKR_F18-log");
        assert_eq!(
            s.message,
            format!("Debug log written: {}", folder.display())
        );
        assert!(folder.join("summary.txt").is_file());
        assert!(folder.join("log.jsonl").is_file());
        // Exports are not recordings: the list is unchanged.
        let s = Replays::open(Some(library.clone()), "Main menu");
        assert_eq!(s.entries.len(), 1);
        assert_eq!(s.entries[0].path, path);
        // A failed export says why.
        let mut s = Replays::preview("Main menu");
        s.export(Export::Tacview);
        settle(&mut s);
        assert!(s.message.contains("was not written"), "{}", s.message);
    }

    #[test]
    fn unfinished_recordings_get_their_length_from_a_full_read() {
        let dir = TempDir::new("screen-partial");
        let library = Library::new(dir.path());
        record(&library, "2026-09-26_1540_UKR_F18.tore-replay", false);
        let mut s = Replays::open(Some(library), "Main menu");
        assert_eq!(s.length(&s.entries[0]), None);
        settle(&mut s);
        assert_eq!(s.length(&s.entries[0]), Some(240));
        assert_eq!(
            result_detail(&s.entries[0]),
            "Unknown: the recording did not finish"
        );
    }

    #[test]
    fn draws_every_state_without_panicking() {
        let font = font();
        let mut pixels = vec![0; 640 * 480 * 4];
        let pixel = |pixels: &[u8], (x, y): (i32, i32)| {
            let at = (y as usize * 640 + x as usize) * 4;
            [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
        };
        let mut s = Replays::preview("Main menu");
        s.draw(&mut pixels, &font);
        assert_eq!(pixel(&pixels, (2, 2)), PANEL, "title strip");
        let row = Replays::row_rect(1);
        assert_eq!(pixel(&pixels, (row.0 + 200, row.1)), FOCUS, "the cursor");
        let row = Replays::row_rect(2);
        assert_eq!(pixel(&pixels, (row.0 + 200, row.1)), PALE);
        let row = Replays::row_rect(3);
        assert_eq!(pixel(&pixels, (row.0 + 200, row.1)), PALE_ALT);
        let keep = Replays::footer_rect(1);
        assert_eq!(pixel(&pixels, (keep.0 + 1, keep.1 + 1)), PALE);
        // With the buttons focused the cursor row stays marked.
        press(&mut s, "Tab");
        s.draw(&mut pixels, &font);
        let row = Replays::row_rect(1);
        assert_eq!(pixel(&pixels, (row.0 + 200, row.1)), SELECTED);
        let watch = Replays::footer_rect(0);
        assert_eq!(pixel(&pixels, (watch.0 + 1, watch.1 + 1)), FOCUS);
        // Dialogs darken what is behind them.
        for state in ["settings", "delete"] {
            let mut s = Replays::preview("Main menu");
            s.preview_state(state).unwrap();
            s.draw(&mut pixels, &font);
            let behind = pixel(&pixels, (2, 2));
            assert!(behind[0] < PANEL[0], "{state}: {behind:?}");
        }
        assert!(Replays::preview("x").preview_state("other").is_err());
        // A long list and an empty folder.
        let mut s = many(60);
        s.select(40);
        s.draw(&mut pixels, &font);
        let dir = TempDir::new("screen-empty");
        let mut s = Replays::open(Some(Library::new(dir.path())), "Main menu");
        s.draw(&mut pixels, &font);
        let watch = Replays::footer_rect(0);
        assert_eq!(
            pixel(&pixels, (watch.0 + 1, watch.1 + 1)),
            HEADER,
            "disabled"
        );
        s.run(Button::AutoDelete);
        s.draw(&mut pixels, &font);
    }
}
