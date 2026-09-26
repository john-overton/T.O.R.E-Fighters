//! Command-line tools for mission recordings, run headless and without the
//! game's media: `--recording-info`, `--recording-log`, `--recording-acmi`
//! and `--recording-diff`, plus the tick-by-tick render check behind
//! `--verify-render`. These read recordings of what happened; they are not
//! the `--replay-input` and `--replay-combat` tapes, which re-simulate.
use super::convert::{self, Identities, Presentation};
use crate::{AppResult, render_snapshot::RenderSnapshot};
use std::collections::BTreeMap;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use tore_replay::{self as replay, export};

/// Options for `--recording-log`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LogOptions {
    /// Folder for `log.jsonl` and `summary.txt`.
    pub out: Option<PathBuf>,
    pub from_s: Option<f64>,
    pub to_s: Option<f64>,
    pub ids: Option<Vec<u32>>,
    /// Aircraft samples per second.
    pub rate: Option<f64>,
}

/// `0:04.2`, minutes and seconds of mission time.
fn clock(ticks: u64) -> String {
    let tenths = ticks * 10 / replay::TICKS_PER_SECOND;
    format!("{}:{:02}.{}", tenths / 600, tenths / 10 % 60, tenths % 10)
}

fn open(path: &Path) -> AppResult<replay::Recording> {
    replay::Recording::open(path).map_err(|error| format!("{}: {error}", path.display()).into())
}

/// Prints what a recording holds: who, where, when, how long, its result,
/// its aircraft and weapons, events by kind and any damage.
pub fn info(path: &Path, out: &mut impl Write) -> AppResult<()> {
    let recording = open(path)?;
    let header = recording.header();
    let world = &header.world;
    writeln!(out, "Recording   {}", path.display())?;
    writeln!(
        out,
        "State       {}",
        if recording.complete() {
            "finished normally"
        } else {
            "INCOMPLETE: the game did not finish writing it (a crash, or still recording)"
        }
    )?;
    writeln!(
        out,
        "Game        {} ({}), format {}, recorded {}",
        header.game_version, header.game_commit, header.format_version, header.recorded_at
    )?;
    writeln!(
        out,
        "Mission     {} in {} ({}), layout {}",
        header.mission.title(),
        world.theater_name,
        world.theater,
        world.layout
    )?;
    let seconds = world.time_of_day_s.max(0.) as u64;
    let [east, _, north] = world.wind_fps;
    writeln!(
        out,
        "Weather     {}{}, starts {:02}:{:02} local, wind {:.1} ft/s toward {:03.0} deg, {} with {}",
        world.weather_name,
        world
            .weather
            .map_or(String::new(), |w| format!(" (choice {w})")),
        seconds / 3600,
        seconds / 60 % 60,
        east.hypot(north),
        east.atan2(north).to_degrees().rem_euclid(360.),
        world.clouds.module,
        world
            .clouds
            .deck_ft
            .map_or("no scattered deck".to_owned(), |feet| format!(
                "a scattered deck at {feet} ft"
            ))
    )?;
    let frames = recording.frame_count();
    let aircraft = recording.aircraft().count().max(1) as u64;
    match recording.first_tick().zip(recording.last_tick()) {
        Some((first, last)) => writeln!(
            out,
            "Length      {} ({} frames, ticks {first} to {last}, {} chunks), {} bytes, {:.1} bytes per aircraft per tick",
            clock(last - first + 1),
            frames,
            recording.chunks().len(),
            recording.file_bytes(),
            recording.file_bytes() as f64 / (frames.max(1) * aircraft) as f64
        )?,
        None => writeln!(out, "Length      no frames")?,
    }
    for (from, to) in recording.gaps() {
        writeln!(out, "Gap         ticks {from} to {to} were not recorded")?;
    }
    for (key, value) in &header.extra {
        writeln!(out, "Setting     {key} = {value}")?;
    }
    match recording.footer() {
        Some(footer) => {
            let result: Vec<String> = footer
                .result
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect();
            writeln!(
                out,
                "Result      {} (end tick {})",
                result.join(", "),
                footer.end_tick
            )?
        }
        None => writeln!(out, "Result      unknown: the recording did not finish")?,
    }
    for info in recording.aircraft() {
        writeln!(
            out,
            "Aircraft    {:>3} {:<14} {} ({}), {} wing {}-{}{}{}",
            info.id,
            info.label,
            info.name,
            info.pt,
            info.side.name(),
            info.wing,
            info.member,
            if info.skill.is_empty() {
                String::new()
            } else {
                format!(", {}", info.skill)
            },
            if info.human { ", human" } else { "" }
        )?;
    }
    for weapon in recording.weapons() {
        writeln!(
            out,
            "Weapon      {:>3} {} ({}, {}){}",
            weapon.id,
            weapon.name,
            weapon.source,
            weapon.class.name(),
            weapon
                .shape
                .as_ref()
                .map_or(String::new(), |s| format!(", shape {s}"))
        )?;
    }
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for event in recording.events() {
        *kinds.entry(event.event.kind.as_str()).or_default() += 1;
    }
    writeln!(out, "Events      {} in all", recording.events().len())?;
    for (kind, count) in kinds {
        writeln!(out, "            {count:>7} {kind}")?;
    }
    for problem in recording.problems() {
        writeln!(out, "Problem     {problem}")?;
    }
    Ok(())
}

/// Writes the debug log and the plain-English summary; returns the folder.
pub fn log(path: &Path, options: &LogOptions) -> AppResult<PathBuf> {
    let recording = open(path)?;
    let folder = options.out.clone().unwrap_or_else(|| sibling(path, "-log"));
    std::fs::create_dir_all(&folder)?;
    let mut jsonl = export::JsonlOptions::default().seconds(options.from_s, options.to_s);
    jsonl.ids = options.ids.clone();
    if let Some(rate) = options.rate {
        jsonl.sample_hz = rate;
    }
    let mut file = BufWriter::new(std::fs::File::create(folder.join("log.jsonl"))?);
    export::write_jsonl(&recording, &jsonl, &mut file)?;
    file.flush()?;
    let mut file = BufWriter::new(std::fs::File::create(folder.join("summary.txt"))?);
    export::write_summary(&recording, &export::SummaryOptions::default(), &mut file)?;
    file.flush()?;
    Ok(folder)
}

/// Writes a Tacview file; returns its path.
pub fn acmi(
    path: &Path,
    out: Option<PathBuf>,
    rate: Option<f64>,
    guns: bool,
) -> AppResult<PathBuf> {
    let recording = open(path)?;
    let target = out.unwrap_or_else(|| sibling(path, ".txt.acmi"));
    let mut options = export::AcmiOptions {
        guns,
        ..export::AcmiOptions::default()
    };
    if let Some(rate) = rate {
        options.sample_hz = rate;
    }
    let mut file = BufWriter::new(std::fs::File::create(&target)?);
    export::write_acmi(&recording, &options, &mut file)?;
    file.flush()?;
    Ok(target)
}

/// Prints how two recordings differ; returns whether they match.
pub fn diff(a: &Path, b: &Path, out: &mut impl Write) -> AppResult<bool> {
    let (a, b) = (open(a)?, open(b)?);
    let comparison = export::write_diff(&a, &b, &export::CompareOptions::default(), out)?;
    Ok(comparison.identical())
}

/// `name.tore-replay` becomes `name` plus `suffix`, beside it.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let stem = path
        .file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.trim_end_matches(".partial"))
        .map(|n| n.strip_suffix(".tore-replay").unwrap_or(n))
        .unwrap_or("recording");
    path.with_file_name(format!("{stem}{suffix}"))
}

/// The result of comparing a recording with the live pictures it was made
/// from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verification {
    pub compared: usize,
    pub missing: usize,
    pub differing: usize,
    /// The first difference: its tick and what differed.
    pub first: Option<(u64, String)>,
}

impl Verification {
    pub fn passed(&self) -> bool {
        self.compared > 0 && self.missing == 0 && self.differing == 0
    }
    /// One line for the probe's output.
    pub fn line(&self) -> String {
        format!(
            "verify-render: {} ticks={} missing={} differing={}{}",
            if self.passed() { "PASS" } else { "FAIL" },
            self.compared,
            self.missing,
            self.differing,
            self.first
                .as_ref()
                .map_or(String::new(), |(tick, what)| format!(
                    " first at tick {tick}: {what}"
                ))
        )
    }
}

/// Rebuilds every recorded tick and compares it with the live snapshot
/// taken at that tick, within the format's precision.
pub fn verify(path: &Path, live: &[RenderSnapshot]) -> AppResult<Verification> {
    let recording = open(path)?;
    let presentation = Presentation::from_header(recording.header());
    let identities = Identities::of(&recording);
    let mut result = Verification {
        compared: 0,
        missing: 0,
        differing: 0,
        first: None,
    };
    for snapshot in live {
        let Some(frame) = recording.frame(snapshot.tick)? else {
            result.missing += 1;
            continue;
        };
        let effects = recording.live_effects(snapshot.tick, convert::EFFECT_LOOKBACK_TICKS)?;
        let rebuilt = convert::snapshot(&frame, &effects, &presentation, &identities);
        result.compared += 1;
        if let Some(what) = convert::difference(snapshot, &rebuilt) {
            result.differing += 1;
            result.first.get_or_insert((snapshot.tick, what));
        }
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_and_sibling_names_read_naturally() {
        assert_eq!(clock(0), "0:00.0");
        assert_eq!(clock(504), "0:04.2");
        assert_eq!(clock(120 * 754), "12:34.0");
        let path = Path::new("/r/2026-09-26_1540_UKR_F18.tore-replay");
        assert_eq!(
            sibling(path, ".txt.acmi"),
            Path::new("/r/2026-09-26_1540_UKR_F18.txt.acmi")
        );
        assert_eq!(
            sibling(&tore_replay::partial_path(path), "-log"),
            Path::new("/r/2026-09-26_1540_UKR_F18-log")
        );
    }
}
