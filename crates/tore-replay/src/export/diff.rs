//! Comparing two recordings: header differences, the first second where the
//! state checksums differ, which aircraft diverged first, and differences in
//! kills, launches, AI decisions and comms.

use super::text::{FPS_PER_KT, Names, clock, describe, num, thousands, value_text};
use crate::error::Result;
use crate::model::{AircraftState, Event, Frame, Header};
use crate::precision;
use crate::reader::{Recording, TimedEvent};
use std::collections::BTreeMap;
use std::io::Write;

/// How far apart two decoded states may be and still count as the same.
/// Defaults are a little over twice each quantity's largest rounding error,
/// so identical flights never differ, even when their keyframes fall on
/// different ticks.
#[derive(Clone, Debug, PartialEq)]
pub struct CompareOptions {
    pub position_ft: f64,
    pub angle_deg: f64,
    pub velocity_fps: f64,
    pub g: f64,
    pub fuel_lb: f64,
    pub device: f64,
    pub control: f64,
}

impl Default for CompareOptions {
    fn default() -> Self {
        Self {
            position_ft: precision::POSITION_FT * 1.5,
            angle_deg: (precision::ANGLE_RAD * 1.5).to_degrees(),
            velocity_fps: precision::VELOCITY_FPS * 1.5,
            g: precision::G * 1.5,
            fuel_lb: precision::FUEL_LB * 1.5,
            device: precision::SIGNED * 1.5,
            control: precision::CONTROL * 1.5,
        }
    }
}

/// One aircraft's difference at the first divergent tick.
#[derive(Clone, Debug, PartialEq)]
pub struct AircraftDifference {
    pub id: u32,
    /// Plain English: what differs and by how much.
    pub detail: String,
    /// Size for ranking: the position gap in feet, or 1e9 when the aircraft
    /// is missing from one recording.
    pub magnitude: f64,
}

/// The first tick where aircraft states differ.
#[derive(Clone, Debug, PartialEq)]
pub struct Divergence {
    pub tick: u64,
    /// Every aircraft that differs at that tick, largest difference first.
    pub aircraft: Vec<AircraftDifference>,
}

/// One side of an event difference: tick and description, absent when
/// that recording has run out of events in the category.
pub type EventSide = Option<(u64, String)>;

/// How one category of events differs.
#[derive(Clone, Debug, PartialEq)]
pub struct EventDifference {
    pub category: &'static str,
    pub count_a: usize,
    pub count_b: usize,
    /// The first event that differs in content: index, then each side's
    /// tick and description (absent when that side has run out).
    pub first: Option<(usize, EventSide, EventSide)>,
    /// When every event matches in content, the first one at a different
    /// tick: index and both ticks.
    pub timing: Option<(usize, u64, u64)>,
}

/// The whole comparison.
#[derive(Clone, Debug, PartialEq)]
pub struct Comparison {
    pub header: Vec<String>,
    pub entities: Vec<String>,
    pub ticks_a: Option<(u64, u64)>,
    pub ticks_b: Option<(u64, u64)>,
    pub checksums_compared: usize,
    pub last_matching_checksum: Option<u64>,
    pub first_checksum_mismatch: Option<u64>,
    pub divergence: Option<Divergence>,
    pub events: Vec<EventDifference>,
}

impl Comparison {
    /// True when nothing differs apart from the recording time.
    pub fn identical(&self) -> bool {
        self.header.iter().all(|h| h.starts_with("recorded_at"))
            && self.entities.is_empty()
            && self.ticks_a == self.ticks_b
            && self.first_checksum_mismatch.is_none()
            && self.divergence.is_none()
            && self
                .events
                .iter()
                .all(|e| e.first.is_none() && e.timing.is_none())
    }
}

fn header_fields(h: &Header) -> Vec<(String, String)> {
    let w = &h.world;
    let mut fields = vec![
        ("format_version".into(), h.format_version.to_string()),
        ("game_version".into(), h.game_version.clone()),
        ("game_commit".into(), h.game_commit.clone()),
        ("recorded_at".into(), h.recorded_at.clone()),
        ("mission".into(), h.mission.as_str().to_owned()),
        ("world.theater".into(), w.theater.clone()),
        ("world.theater_name".into(), w.theater_name.clone()),
        ("world.layout".into(), w.layout.clone()),
        ("world.weather".into(), format!("{:?}", w.weather)),
        ("world.weather_name".into(), w.weather_name.clone()),
        ("world.weather_seed".into(), format!("{:?}", w.weather_seed)),
        ("world.time_of_day_s".into(), format!("{}", w.time_of_day_s)),
        ("world.wind_fps".into(), format!("{:?}", w.wind_fps)),
        ("world.clouds.module".into(), w.clouds.module.clone()),
        (
            "world.clouds.deck_ft".into(),
            format!("{:?}", w.clouds.deck_ft),
        ),
        ("world.extent_ft".into(), format!("{:?}", w.extent_ft)),
    ];
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (k, v) in &h.extra {
        let n = seen.entry(k).or_insert(0);
        let key = if *n == 0 {
            format!("extra.{k}")
        } else {
            format!("extra.{k}#{n}")
        };
        *n += 1;
        fields.push((key, v.clone()));
    }
    fields
}

fn compare_headers(a: &Header, b: &Header) -> Vec<String> {
    let fa = header_fields(a);
    let fb = header_fields(b);
    let mb: BTreeMap<&String, &String> = fb.iter().map(|(k, v)| (k, v)).collect();
    let ma: BTreeMap<&String, &String> = fa.iter().map(|(k, v)| (k, v)).collect();
    let mut out = Vec::new();
    for (k, v) in &fa {
        match mb.get(k) {
            Some(other) if *other == v => {}
            Some(other) => out.push(format!("{k}: A {v:?}, B {other:?}")),
            None => out.push(format!("{k}: A {v:?}, B has none")),
        }
    }
    for (k, v) in &fb {
        if !ma.contains_key(k) {
            out.push(format!("{k}: A has none, B {v:?}"));
        }
    }
    out
}

fn compare_entities(a: &Recording, b: &Recording) -> Vec<String> {
    let mut out = Vec::new();
    let ids: std::collections::BTreeSet<u32> =
        a.aircraft().chain(b.aircraft()).map(|x| x.id).collect();
    for id in ids {
        match (a.aircraft_info(id), b.aircraft_info(id)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => out.push(format!(
                "aircraft {id}: A {} {} ({}), B {} {} ({})",
                x.label, x.name, x.pt, y.label, y.name, y.pt
            )),
            (Some(x), None) => out.push(format!("aircraft {id} ({}) only in A", x.label)),
            (None, Some(y)) => out.push(format!("aircraft {id} ({}) only in B", y.label)),
            (None, None) => {}
        }
    }
    let ids: std::collections::BTreeSet<u32> =
        a.weapons().chain(b.weapons()).map(|x| x.id).collect();
    for id in ids {
        match (a.weapon_info(id), b.weapon_info(id)) {
            (Some(x), Some(y)) if x == y => {}
            (Some(x), Some(y)) => out.push(format!("weapon {id}: A {}, B {}", x.name, y.name)),
            (Some(x), None) => out.push(format!("weapon {id} ({}) only in A", x.name)),
            (None, Some(y)) => out.push(format!("weapon {id} ({}) only in B", y.name)),
            (None, None) => {}
        }
    }
    out
}

/// True when a gap is beyond tolerance, or not a number at all.
fn far(gap: f64, tolerance: f64) -> bool {
    !matches!(
        gap.partial_cmp(&tolerance),
        Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
    )
}

/// Two scalars differ: identical bits never do, otherwise by tolerance.
fn apart(x: f64, y: f64, tolerance: f64) -> bool {
    x.to_bits() != y.to_bits() && far((x - y).abs(), tolerance)
}

fn gap(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn angle_gap(a: f64, b: f64) -> f64 {
    crate::predict::wrap_pi(a - b).abs().to_degrees()
}

/// What differs between two states of one aircraft, if anything.
fn state_difference(
    x: &AircraftState,
    y: &AircraftState,
    o: &CompareOptions,
) -> Option<AircraftDifference> {
    let mut parts = Vec::new();
    let position = gap(x.position, y.position);
    if x.position != y.position && far(position, o.position_ft) {
        parts.push(format!("{} ft apart", num(position, 2)));
    }
    for (i, name) in ["heading", "pitch", "bank"].iter().enumerate() {
        let d = angle_gap(x.attitude[i], y.attitude[i]);
        if x.attitude[i].to_bits() != y.attitude[i].to_bits() && far(d, o.angle_deg) {
            parts.push(format!("{name} differs by {} deg", num(d, 3)));
        }
    }
    let v = gap(x.velocity, y.velocity);
    if x.velocity != y.velocity && far(v, o.velocity_fps) {
        parts.push(format!("velocity differs by {} kt", num(v / FPS_PER_KT, 2)));
    }
    if apart(x.airspeed, y.airspeed, o.velocity_fps) {
        parts.push(format!(
            "airspeed {} vs {} kt",
            num(x.airspeed / FPS_PER_KT, 1),
            num(y.airspeed / FPS_PER_KT, 1)
        ));
    }
    if apart(x.g, y.g, o.g) {
        parts.push(format!("G {} vs {}", num(x.g, 3), num(y.g, 3)));
    }
    if apart(x.fuel_lb, y.fuel_lb, o.fuel_lb) {
        parts.push(format!(
            "fuel {} vs {} lb",
            num(x.fuel_lb, 1),
            num(y.fuel_lb, 1)
        ));
    }
    for (slot, name) in crate::model::device::NAMES.iter().enumerate() {
        let tolerance = if slot == crate::model::device::SPEED {
            o.velocity_fps
        } else {
            o.device
        };
        if apart(x.devices[slot], y.devices[slot], tolerance) {
            parts.push(format!(
                "{name} {} vs {}",
                num(x.devices[slot], 3),
                num(y.devices[slot], 3)
            ));
        }
    }
    if apart(x.heat, y.heat, o.device) {
        parts.push(format!("heat {} vs {}", num(x.heat, 3), num(y.heat, 3)));
    }
    for (i, name) in crate::model::control::NAMES.iter().enumerate() {
        if apart(x.controls[i], y.controls[i], o.control) {
            parts.push(format!(
                "{name} control {} vs {}",
                num(x.controls[i], 3),
                num(y.controls[i], 3)
            ));
        }
    }
    if x.flags != y.flags {
        parts.push(format!(
            "flags [{}] vs [{}]",
            x.flags.names().join(" "),
            y.flags.names().join(" ")
        ));
    }
    if x.wreck_phase != y.wreck_phase {
        parts.push(format!(
            "wreck phase {} vs {}",
            x.wreck_phase, y.wreck_phase
        ));
    }
    if (x.hp, x.max_hp) != (y.hp, y.max_hp) {
        parts.push(format!("hp {}/{} vs {}/{}", x.hp, x.max_hp, y.hp, y.max_hp));
    }
    if x.sections != y.sections || x.structural_section != y.structural_section {
        parts.push(format!(
            "damage {:?} {:?} vs {:?} {:?}",
            x.sections, x.structural_section, y.sections, y.structural_section
        ));
    }
    (!parts.is_empty()).then(|| AircraftDifference {
        id: x.id,
        detail: parts.join(", "),
        magnitude: if position.is_finite() { position } else { 1e9 },
    })
}

fn frame_differences(
    a: &Frame,
    b: &Frame,
    options: &CompareOptions,
    names: &Names,
) -> Vec<AircraftDifference> {
    let mut out = Vec::new();
    let mb: BTreeMap<u32, &AircraftState> = b.aircraft.iter().map(|s| (s.id, s)).collect();
    let ma: BTreeMap<u32, &AircraftState> = a.aircraft.iter().map(|s| (s.id, s)).collect();
    for (id, x) in &ma {
        match mb.get(id) {
            Some(y) => out.extend(state_difference(x, y, options)),
            None => out.push(AircraftDifference {
                id: *id,
                detail: format!("{} is only in A", names.who(*id)),
                magnitude: 1e9,
            }),
        }
    }
    for id in mb.keys().filter(|id| !ma.contains_key(id)) {
        out.push(AircraftDifference {
            id: *id,
            detail: format!("{} is only in B", names.who(*id)),
            magnitude: 1e9,
        });
    }
    out.sort_by(|p, q| q.magnitude.total_cmp(&p.magnitude).then(p.id.cmp(&q.id)));
    out
}

const CATEGORIES: [(&str, &[&str]); 9] = [
    ("kills", &["combat.destroyed"]),
    ("launches", &["weapon.launch"]),
    ("hits", &["combat.hit"]),
    (
        "chaff and flares",
        &["combat.countermeasure", "combat.countermeasures_cleared"],
    ),
    ("shot outcomes", &["weapon.", "combat."]),
    ("AI decisions", &["ai."]),
    ("comms", &["comms."]),
    ("flight and aircraft events", &["flight.", "aircraft."]),
    (
        "player and system events",
        &["player.", "system.", "audio."],
    ),
];

fn category(kind_name: &str) -> Option<&'static str> {
    CATEGORIES
        .iter()
        .find(|(_, prefixes)| {
            prefixes.iter().any(|p| {
                if p.ends_with('.') {
                    kind_name.starts_with(p)
                } else {
                    kind_name == *p
                }
            })
        })
        .map(|(name, _)| *name)
}

/// Content that must match: everything but the tick.
fn content(e: &Event) -> String {
    let fields: Vec<String> = e
        .fields
        .iter()
        .map(|(k, v)| format!("{k}={}", value_text(v)))
        .collect();
    format!(
        "{}|{:?}|{:?}|{}|{}",
        e.kind,
        e.subject,
        e.object,
        fields.join(","),
        e.text
    )
}

fn compare_events(a: &Recording, b: &Recording) -> Vec<EventDifference> {
    let names_a = Names::new(a);
    let names_b = Names::new(b);
    let split = |r: &Recording| {
        let mut map: BTreeMap<&'static str, Vec<TimedEvent>> = BTreeMap::new();
        for e in r.events() {
            if let Some(c) = category(&e.event.kind) {
                map.entry(c).or_default().push(e.clone());
            }
        }
        map
    };
    let ea = split(a);
    let eb = split(b);
    let empty = Vec::new();
    let mut out = Vec::new();
    for (name, _) in CATEGORIES {
        let xa = ea.get(name).unwrap_or(&empty);
        let xb = eb.get(name).unwrap_or(&empty);
        if xa.is_empty() && xb.is_empty() {
            continue;
        }
        let first = (0..xa.len().max(xb.len())).find(|i| {
            xa.get(*i).map(|e| content(&e.event)) != xb.get(*i).map(|e| content(&e.event))
        });
        let first = first.map(|i| {
            (
                i,
                xa.get(i).map(|e| (e.tick, describe(&e.event, &names_a))),
                xb.get(i).map(|e| (e.tick, describe(&e.event, &names_b))),
            )
        });
        let timing = if first.is_none() {
            xa.iter()
                .zip(xb)
                .enumerate()
                .find(|(_, (x, y))| x.tick != y.tick)
                .map(|(i, (x, y))| (i, x.tick, y.tick))
        } else {
            None
        };
        out.push(EventDifference {
            category: name,
            count_a: xa.len(),
            count_b: xb.len(),
            first,
            timing,
        });
    }
    out
}

/// Compares two recordings.
pub fn compare(a: &Recording, b: &Recording, options: &CompareOptions) -> Result<Comparison> {
    let span = |r: &Recording| r.first_tick().zip(r.last_tick());
    let ca: BTreeMap<u64, u64> = a.checksums().iter().copied().collect();
    let mut compared = 0;
    let mut last_match = None;
    let mut mismatch = None;
    for (tick, sum_b) in b.checksums() {
        if let Some(sum_a) = ca.get(tick) {
            compared += 1;
            if sum_a == sum_b {
                last_match = Some(*tick);
            } else {
                mismatch = Some(*tick);
                break;
            }
        }
    }
    let names = Names::new(a);
    let start = match (a.first_tick(), b.first_tick()) {
        (Some(x), Some(y)) => Some(last_match.unwrap_or(x.max(y)).max(x.max(y))),
        _ => None,
    };
    let mut divergence = None;
    if let Some(start) = start {
        let mut fb = b.frames(start, u64::MAX).peekable();
        for frame_a in a.frames(start, u64::MAX) {
            let frame_a = frame_a?;
            while fb
                .peek()
                .is_some_and(|f| f.as_ref().is_ok_and(|f| f.tick < frame_a.tick))
            {
                fb.next();
            }
            if matches!(fb.peek(), Some(Err(_)))
                && let Some(Err(error)) = fb.next()
            {
                return Err(error);
            }
            let frame_b = match fb.peek() {
                Some(Ok(f)) if f.tick == frame_a.tick => f,
                _ => continue,
            };
            let differences = frame_differences(&frame_a, frame_b, options, &names);
            if !differences.is_empty() {
                divergence = Some(Divergence {
                    tick: frame_a.tick,
                    aircraft: differences,
                });
                break;
            }
        }
    }
    Ok(Comparison {
        header: compare_headers(a.header(), b.header()),
        entities: compare_entities(a, b),
        ticks_a: span(a),
        ticks_b: span(b),
        checksums_compared: compared,
        last_matching_checksum: last_match,
        first_checksum_mismatch: mismatch,
        divergence,
        events: compare_events(a, b),
    })
}

impl Comparison {
    /// The comparison in plain English.
    pub fn write_text(&self, a: &Recording, mut out: impl Write) -> Result<()> {
        let names = Names::new(a);
        let o = &mut out;
        writeln!(o, "T.O.R.E recording comparison (A against B)")?;
        writeln!(o, "==========================================")?;
        writeln!(o)?;
        let span = |s: Option<(u64, u64)>| match s {
            Some((x, y)) => format!("{} to {} (ticks {x} to {y})", clock(x), clock(y)),
            None => "no frames".into(),
        };
        writeln!(o, "A covers    {}", span(self.ticks_a))?;
        writeln!(o, "B covers    {}", span(self.ticks_b))?;
        writeln!(o)?;
        writeln!(o, "Header")?;
        writeln!(o, "------")?;
        if self.header.is_empty() {
            writeln!(o, "identical")?;
        }
        for line in &self.header {
            writeln!(o, "{line}")?;
        }
        if !self.entities.is_empty() {
            writeln!(o)?;
            writeln!(o, "Aircraft and weapons")?;
            writeln!(o, "--------------------")?;
            for line in &self.entities {
                writeln!(o, "{line}")?;
            }
        }
        writeln!(o)?;
        writeln!(o, "State checksums")?;
        writeln!(o, "---------------")?;
        match (self.checksums_compared, self.first_checksum_mismatch) {
            (0, _) => writeln!(o, "no second where both recordings hold a checksum")?,
            (n, None) => writeln!(o, "all {n} shared checksums match")?,
            (n, Some(tick)) => {
                writeln!(
                    o,
                    "first differs at {} (tick {tick}), after {} compared",
                    clock(tick),
                    n
                )?;
                match self.last_matching_checksum {
                    Some(last) => writeln!(o, "last match at {} (tick {last})", clock(last))?,
                    None => writeln!(o, "no earlier checksum matched")?,
                }
            }
        }
        writeln!(o)?;
        writeln!(o, "First divergence")?;
        writeln!(o, "----------------")?;
        match &self.divergence {
            None => writeln!(o, "no aircraft state differs beyond rounding")?,
            Some(d) => {
                writeln!(o, "at {} (tick {}):", clock(d.tick), d.tick)?;
                for x in &d.aircraft {
                    writeln!(o, "  {}: {}", names.who(x.id), x.detail)?;
                }
            }
        }
        writeln!(o)?;
        writeln!(o, "Events")?;
        writeln!(o, "------")?;
        if self.events.is_empty() {
            writeln!(o, "none in either recording")?;
        }
        for e in &self.events {
            let counts = if e.count_a == e.count_b {
                format!("{} each", thousands(e.count_a as f64))
            } else {
                format!(
                    "A {}, B {}",
                    thousands(e.count_a as f64),
                    thousands(e.count_b as f64)
                )
            };
            match (&e.first, e.timing) {
                (None, None) => writeln!(o, "{}: same ({counts})", e.category)?,
                (None, Some((i, x, y))) => writeln!(
                    o,
                    "{}: same events ({counts}), first timing difference at #{}: A {} B {}",
                    e.category,
                    i + 1,
                    clock(x),
                    clock(y)
                )?,
                (Some((i, x, y)), _) => {
                    writeln!(o, "{}: differ ({counts}), first at #{}:", e.category, i + 1)?;
                    let side = |s: &Option<(u64, String)>| match s {
                        Some((t, text)) => format!("{}  {text}", clock(*t)),
                        None => "nothing (ran out)".into(),
                    };
                    writeln!(o, "  A {}", side(x))?;
                    writeln!(o, "  B {}", side(y))?;
                }
            }
        }
        writeln!(o)?;
        writeln!(
            o,
            "Verdict: {}",
            if self.identical() {
                "the recordings match"
            } else {
                "the recordings differ"
            }
        )?;
        Ok(())
    }
}

/// Compares and writes the text in one call.
pub fn write_diff(
    a: &Recording,
    b: &Recording,
    options: &CompareOptions,
    out: impl Write,
) -> Result<Comparison> {
    let comparison = compare(a, b, options)?;
    comparison.write_text(a, out)?;
    Ok(comparison)
}
