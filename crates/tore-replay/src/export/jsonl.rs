//! The machine-readable debug log: one JSON object per line.
//!
//! Lines, in order: `header` (with units), `aircraft` and `weapon` entities,
//! then in tick order `event`, `sample` (aircraft state at the chosen rate)
//! and `tree` (each display tree sample that changed), then `anomaly` lines
//! and a closing `footer`. Numbers that are not finite are written as `null`;
//! the anomaly lines name them.

use super::anomaly::{self, Thresholds};
use super::json::{self, Object};
use crate::error::{Result, invalid};
use crate::model::{AircraftState, Event, TICKS_PER_SECOND, TreeSample, control, device};
use crate::reader::Recording;
use std::collections::HashMap;
use std::io::Write;

/// JSONL options.
#[derive(Clone, Debug, PartialEq)]
pub struct JsonlOptions {
    /// First tick to include; `None` starts at the beginning.
    pub from_tick: Option<u64>,
    /// Last tick to include; `None` runs to the end.
    pub to_tick: Option<u64>,
    /// Only these aircraft, and events and trees about them. Events with no
    /// subject and no object are always kept.
    pub ids: Option<Vec<u32>>,
    /// Aircraft samples per second, 1/120 to 120. Default 1.
    pub sample_hz: f64,
    /// Include display tree samples. Default true.
    pub trees: bool,
    /// Anomaly thresholds.
    pub thresholds: Thresholds,
}

impl Default for JsonlOptions {
    fn default() -> Self {
        Self {
            from_tick: None,
            to_tick: None,
            ids: None,
            sample_hz: 1.,
            trees: true,
            thresholds: Thresholds::default(),
        }
    }
}

impl JsonlOptions {
    /// A range in mission seconds (tick / 120).
    pub fn seconds(mut self, from_s: Option<f64>, to_s: Option<f64>) -> Self {
        let tick = |s: f64| (s.max(0.) * TICKS_PER_SECOND as f64).round() as u64;
        self.from_tick = from_s.map(tick);
        self.to_tick = to_s.map(tick);
        self
    }
}

/// What was written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct JsonlStats {
    pub lines: u64,
    pub events: u64,
    pub samples: u64,
    pub trees: u64,
    pub anomalies: u64,
}

fn seconds(tick: u64) -> String {
    json::number(tick as f64 / TICKS_PER_SECOND as f64, 3)
}

fn header_line(recording: &Recording) -> String {
    let h = recording.header();
    let w = &h.world;
    let world = Object::new()
        .str("theater", &w.theater)
        .str("theater_name", &w.theater_name)
        .str("layout", &w.layout)
        .raw(
            "weather",
            w.weather.map_or("null".into(), |v| v.to_string()),
        )
        .str("weather_name", &w.weather_name)
        .raw(
            "weather_seed",
            w.weather_seed.map_or("null".into(), |v| v.to_string()),
        )
        .raw("time_of_day_s", json::exact(w.time_of_day_s))
        .raw(
            "wind_fps",
            format!(
                "[{}]",
                w.wind_fps
                    .iter()
                    .map(|v| json::exact(*v))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        )
        .raw(
            "clouds",
            Object::new()
                .str("module", &w.clouds.module)
                .raw(
                    "deck_ft",
                    w.clouds.deck_ft.map_or("null".into(), json::exact),
                )
                .finish(),
        )
        .raw(
            "extent_ft",
            w.extent_ft.map_or("null".into(), |e| {
                format!("[{},{}]", json::exact(e[0]), json::exact(e[1]))
            }),
        )
        .finish();
    let mut extra = Object::new();
    for (k, v) in &h.extra {
        extra = extra.str(k, v);
    }
    let units = Object::new()
        .str("t", "seconds of mission time (tick / 120)")
        .str("pos_ft", "feet: X east, Y up (above sea level), Z north")
        .str(
            "att_deg",
            "degrees: yaw clockwise from north, pitch nose up, bank right wing down",
        )
        .str("vel_fps", "feet per second, ground relative")
        .str("airspeed_fps", "feet per second")
        .str("tas_kt", "knots")
        .str("g", "load factor")
        .str("fuel_lb", "pounds")
        .str(
            "devices",
            "0 to 1, except elevator, aileron and rudder (-1 to 1) and speed (feet per second)",
        )
        .str("controls", "pitch, roll and yaw -1 to 1, throttle 0 to 1")
        .finish();
    let problems = format!(
        "[{}]",
        recording
            .problems()
            .iter()
            .map(|p| json::string(p))
            .collect::<Vec<_>>()
            .join(",")
    );
    Object::new()
        .str("type", "header")
        .int("format", h.format_version)
        .str("game_version", &h.game_version)
        .str("game_commit", &h.game_commit)
        .str("recorded_at", &h.recorded_at)
        .str("mission", h.mission.as_str())
        .raw("world", world)
        .raw("extra", extra.finish())
        .int("ticks_per_second", TICKS_PER_SECOND)
        .raw(
            "first_tick",
            recording
                .first_tick()
                .map_or("null".into(), |t| t.to_string()),
        )
        .raw(
            "last_tick",
            recording
                .last_tick()
                .map_or("null".into(), |t| t.to_string()),
        )
        .bool("complete", recording.complete())
        .raw("problems", problems)
        .raw("units", units)
        .finish()
}

fn event_line(tick: u64, e: &Event) -> String {
    let mut fields = Object::new();
    for (name, value) in &e.fields {
        fields = fields.raw(name, json::value(value));
    }
    let mut line = Object::new()
        .str("type", "event")
        .int("tick", tick)
        .raw("t", seconds(tick))
        .str("kind", &e.kind);
    if let Some(s) = e.subject {
        line = line.int("subject", s);
    }
    if let Some(o) = e.object {
        line = line.int("object", o);
    }
    line = line.raw("fields", fields.finish());
    if !e.text.is_empty() {
        line = line.str("text", &e.text);
    }
    line.finish()
}

fn sample_line(tick: u64, s: &AircraftState) -> String {
    let mut devices = Object::new();
    for (slot, name) in device::NAMES.iter().enumerate() {
        let decimals = if slot == device::SPEED { 2 } else { 3 };
        devices = devices.num(name, s.devices[slot], decimals);
    }
    let mut controls = Object::new();
    for (i, name) in control::NAMES.iter().enumerate() {
        controls = controls.num(name, s.controls[i], 3);
    }
    let flags = format!(
        "[{}]",
        s.flags
            .names()
            .iter()
            .map(|f| json::string(f))
            .collect::<Vec<_>>()
            .join(",")
    );
    Object::new()
        .str("type", "sample")
        .int("tick", tick)
        .raw("t", seconds(tick))
        .int("id", s.id)
        .raw("pos_ft", json::numbers(&s.position, 2))
        .raw(
            "att_deg",
            json::numbers(&s.attitude.map(f64::to_degrees), 3),
        )
        .raw("vel_fps", json::numbers(&s.velocity, 2))
        .num("airspeed_fps", s.airspeed, 2)
        .num("tas_kt", s.airspeed / super::text::FPS_PER_KT, 1)
        .num("g", s.g, 3)
        .num("fuel_lb", s.fuel_lb, 1)
        .num("heat", s.heat, 3)
        .int("hp", s.hp)
        .int("max_hp", s.max_hp)
        .raw(
            "sections",
            format!(
                "[{}]",
                s.sections
                    .iter()
                    .map(i32::to_string)
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        )
        .raw(
            "structural",
            s.structural_section
                .map_or("null".into(), |v| v.to_string()),
        )
        .int("wreck", s.wreck_phase)
        .raw("flags", flags)
        .raw("devices", devices.finish())
        .raw("controls", controls.finish())
        .finish()
}

fn tree_line(tick: u64, tree: &TreeSample) -> String {
    let nodes: Vec<String> = tree
        .nodes
        .iter()
        .map(|n| {
            let mut o = Object::new()
                .int("depth", n.depth)
                .str("label", &n.label)
                .raw("value", json::value(&n.value));
            if !n.unit.is_empty() {
                o = o.str("unit", &n.unit);
            }
            if !n.note.is_empty() {
                o = o.str("note", &n.note);
            }
            o.finish()
        })
        .collect();
    Object::new()
        .str("type", "tree")
        .int("tick", tick)
        .raw("t", seconds(tick))
        .int("subject", tree.subject)
        .str("channel", &tree.channel)
        .raw("nodes", format!("[{}]", nodes.join(",")))
        .finish()
}

/// Writes the log. Returns counts of what was written.
pub fn write_jsonl(
    recording: &Recording,
    options: &JsonlOptions,
    mut out: impl Write,
) -> Result<JsonlStats> {
    if !(options.sample_hz.is_finite() && options.sample_hz > 0. && options.sample_hz <= 120.) {
        return Err(invalid(format!(
            "samples per second must be above 0 and at most 120, not {}",
            options.sample_hz
        )));
    }
    let step = (TICKS_PER_SECOND as f64 / options.sample_hz)
        .round()
        .max(1.) as u64;
    let from = options.from_tick.unwrap_or(0);
    let to = options.to_tick.unwrap_or(u64::MAX);
    let wanted = |id: Option<u32>| match (&options.ids, id) {
        (None, _) => true,
        (Some(ids), Some(id)) => ids.contains(&id),
        (Some(_), None) => false,
    };
    let event_wanted = |e: &Event| {
        options.ids.is_none()
            || (e.subject.is_none() && e.object.is_none())
            || wanted(e.subject)
            || wanted(e.object)
    };
    let mut stats = JsonlStats::default();
    let line = |out: &mut dyn Write, text: String, stats: &mut JsonlStats| -> Result<()> {
        out.write_all(text.as_bytes())?;
        out.write_all(b"\n")?;
        stats.lines += 1;
        Ok(())
    };
    line(&mut out, header_line(recording), &mut stats)?;
    for a in recording.aircraft().filter(|a| wanted(Some(a.id))) {
        let text = Object::new()
            .str("type", "aircraft")
            .int("id", a.id)
            .str("pt", &a.pt)
            .str("name", &a.name)
            .str("label", &a.label)
            .str("side", a.side.name())
            .int("wing", a.wing)
            .int("member", a.member)
            .str("skill", &a.skill)
            .bool("human", a.human)
            .finish();
        line(&mut out, text, &mut stats)?;
    }
    for w in recording.weapons() {
        let text = Object::new()
            .str("type", "weapon")
            .int("id", w.id)
            .str("source", &w.source)
            .raw(
                "shape",
                w.shape.as_deref().map_or("null".into(), json::string),
            )
            .str("name", &w.name)
            .str("class", w.class.name())
            .finish();
        line(&mut out, text, &mut stats)?;
    }
    let first = recording.first_tick().unwrap_or(0);
    let mut last_trees: HashMap<(u32, String), TreeSample> = HashMap::new();
    for frame in recording.frames(from, to) {
        let frame = frame?;
        let tick = frame.tick;
        for e in frame.events.iter().filter(|e| event_wanted(e)) {
            line(&mut out, event_line(tick, e), &mut stats)?;
            stats.events += 1;
        }
        if (tick - first).is_multiple_of(step) {
            for s in frame.aircraft.iter().filter(|s| wanted(Some(s.id))) {
                line(&mut out, sample_line(tick, s), &mut stats)?;
                stats.samples += 1;
            }
        }
        if options.trees {
            for tree in frame.trees.iter().filter(|t| wanted(Some(t.subject))) {
                let key = (tree.subject, tree.channel.clone());
                if last_trees.get(&key) == Some(tree) {
                    continue;
                }
                line(&mut out, tree_line(tick, tree), &mut stats)?;
                stats.trees += 1;
                last_trees.insert(key, tree.clone());
            }
        }
    }
    for a in anomaly::detect(recording, &options.thresholds)? {
        if a.tick < from
            || a.tick > to
            || !(options.ids.is_none() || a.subject.is_none() || wanted(a.subject))
        {
            continue;
        }
        let mut o = Object::new()
            .str("type", "anomaly")
            .int("tick", a.tick)
            .raw("t", seconds(a.tick))
            .str("kind", a.kind);
        if let Some(s) = a.subject {
            o = o.int("subject", s);
        }
        line(&mut out, o.str("detail", &a.detail).finish(), &mut stats)?;
        stats.anomalies += 1;
    }
    let mut footer = Object::new().str("type", "footer");
    match recording.footer() {
        Some(f) => {
            let mut result = Object::new();
            for (k, v) in &f.result {
                result = result.str(k, v);
            }
            footer = footer
                .int("end_tick", f.end_tick)
                .raw("result", result.finish());
        }
        None => footer = footer.raw("end_tick", "null").raw("result", "null"),
    }
    let text = footer.bool("complete", recording.complete()).finish();
    line(&mut out, text, &mut stats)?;
    Ok(stats)
}
