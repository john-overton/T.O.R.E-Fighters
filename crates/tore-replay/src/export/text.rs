//! Shared wording for the exports: names, times, numbers and one-line
//! plain-English descriptions of events.

use crate::model::{Event, TICKS_PER_SECOND, Value};
use crate::reader::Recording;
use crate::vocab::{field, kind};

/// Surface objects (airport buildings and the like) use ids from here up.
pub(crate) const SURFACE_IDS: u32 = 0x4000_0000;

pub(crate) const FT_PER_NM: f64 = 6_076.115_49;
pub(crate) const FPS_PER_KT: f64 = 1.687_809_857;

/// Mission clock for a tick: `M:SS.s`, or `H:MM:SS.s` past an hour.
pub(crate) fn clock(tick: u64) -> String {
    let tenths = tick * 10 / TICKS_PER_SECOND;
    let (hours, rest) = (tenths / 36_000, tenths % 36_000);
    let (minutes, rest) = (rest / 600, rest % 600);
    if hours > 0 {
        format!("{hours}:{minutes:02}:{:02}.{}", rest / 10, rest % 10)
    } else {
        format!("{minutes}:{:02}.{}", rest / 10, rest % 10)
    }
}

/// Seconds for a span of ticks, one decimal.
pub(crate) fn seconds(ticks: u64) -> String {
    format!("{:.1} s", ticks as f64 / TICKS_PER_SECOND as f64)
}

/// A duration as `M:SS.s`.
pub(crate) fn duration(ticks: u64) -> String {
    clock(ticks)
}

/// Rounded to `decimals`, trailing zeros trimmed, never `-0`.
pub(crate) fn num(v: f64, decimals: usize) -> String {
    if !v.is_finite() {
        return if v.is_nan() {
            "NaN".into()
        } else if v > 0. {
            "infinity".into()
        } else {
            "-infinity".into()
        };
    }
    let mut text = format!("{v:.decimals$}");
    if text.contains('.') {
        while text.ends_with('0') {
            text.pop();
        }
        if text.ends_with('.') {
            text.pop();
        }
    }
    if text == "-0" {
        text = "0".into();
    }
    text
}

/// Nautical miles from feet, always with one decimal.
pub(crate) fn nm(feet: f64) -> String {
    if feet.is_finite() {
        format!("{:.1}", feet / FT_PER_NM)
    } else {
        num(feet, 0)
    }
}

/// Whole number with thousands separators: `12,340`.
pub(crate) fn thousands(v: f64) -> String {
    if !v.is_finite() {
        return num(v, 0);
    }
    let rounded = v.round();
    let digits = format!("{:.0}", rounded.abs());
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    if rounded < 0. { format!("-{out}") } else { out }
}

/// Names for ids, from the recording's registry.
pub(crate) struct Names<'a> {
    recording: &'a Recording,
}

impl<'a> Names<'a> {
    pub fn new(recording: &'a Recording) -> Self {
        Self { recording }
    }

    /// An aircraft's label, or a description of a surface object.
    pub fn who(&self, id: u32) -> String {
        match self.recording.aircraft_info(id) {
            Some(info) if !info.label.is_empty() => info.label.clone(),
            Some(info) if !info.name.is_empty() => format!("{} {id}", info.name),
            _ if id >= SURFACE_IDS => format!("surface object {id:#x}"),
            _ => format!("aircraft {id}"),
        }
    }

    pub fn weapon(&self, id: u32) -> String {
        match self.recording.weapon_info(id) {
            Some(info) if !info.name.is_empty() => info.name.clone(),
            _ => format!("weapon {id}"),
        }
    }

    pub fn many(&self, ids: &[u32]) -> String {
        ids.iter()
            .map(|id| self.who(*id))
            .collect::<Vec<_>>()
            .join(", ")
    }
}

/// A field value as text for people.
pub(crate) fn value_text(value: &Value) -> String {
    match value {
        Value::None => "none".into(),
        Value::Bool(v) => if *v { "yes" } else { "no" }.into(),
        Value::Int(v) => v.to_string(),
        Value::Num(v) => num(*v, 2),
        Value::Text(v) => v.clone(),
        Value::Id(v) => v.to_string(),
        Value::Ids(v) => v.iter().map(u32::to_string).collect::<Vec<_>>().join(" "),
    }
}

/// Every field as `name=value`, for the generic description.
pub(crate) fn fields_text(event: &Event) -> String {
    event
        .fields
        .iter()
        .map(|(name, value)| format!("{name}={}", value_text(value)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn opt(event: &Event, name: &str) -> Option<String> {
    event.get(name).map(value_text).filter(|t| !t.is_empty())
}

fn because(event: &Event) -> String {
    opt(event, field::REASON)
        .map(|r| format!(" because {r}"))
        .unwrap_or_default()
}

fn quoted(text: &str) -> String {
    if text.is_empty() {
        String::new()
    } else {
        format!(": \"{text}\"")
    }
}

/// Comms kinds as the transcript labels them.
fn comms_label(kind_name: &str) -> &'static str {
    match kind_name {
        kind::COMMS_ORDER => "ORDER",
        kind::COMMS_REQUEST => "REQUEST",
        kind::COMMS_REPORT => "REPORT",
        kind::COMMS_DELIVERY => "ANSWER",
        kind::COMMS_RADIO => "RADIO",
        kind::COMMS_CREW => "CREW",
        kind::COMMS_TOWER => "TOWER",
        kind::COMMS_HUD => "HUD",
        kind::AUDIO_MUSIC => "MUSIC",
        _ => "COMMS",
    }
}

fn comms(event: &Event, names: &Names, pad: bool) -> String {
    let label = comms_label(&event.kind);
    if event.kind == kind::AUDIO_MUSIC {
        // The music has no speaker: which score the inputs ask for, and why.
        let change = format!(
            "situation {} -> {}",
            opt(event, field::FROM).unwrap_or_else(|| "?".into()),
            opt(event, field::TO).unwrap_or_else(|| "?".into())
        );
        let mut line = if pad {
            format!("{label:<8}{change}")
        } else {
            format!("{label} {change}")
        };
        if let Some(reason) = opt(event, field::REASON) {
            line.push_str(&format!("; why: {reason}"));
        }
        return line;
    }
    let who = event
        .string(field::SPEAKER)
        .map(str::to_owned)
        .or_else(|| event.subject.map(|id| names.who(id)))
        .unwrap_or_else(|| "someone".into());
    let mut line = if pad {
        format!("{label:<8}{who}")
    } else {
        format!("{label} {who}")
    };
    let recipients = event
        .get(field::RECIPIENTS)
        .and_then(Value::as_ids)
        .map(|ids| names.many(ids))
        .or_else(|| event.object.map(|id| names.who(id)));
    if let Some(to) = recipients {
        line.push_str(&format!(" -> {to}"));
    }
    let content = if !event.text.is_empty() {
        format!("\"{}\"", event.text)
    } else {
        opt(event, field::ORDER).unwrap_or_default()
    };
    if !content.is_empty() {
        line.push_str(&format!(": {content}"));
    }
    if let Some(outcome) = opt(event, field::OUTCOME) {
        line.push_str(&format!(" [{outcome}]"));
    }
    match event.flag(field::HEARD) {
        Some(true) => line.push_str(", heard by you"),
        Some(false) => line.push_str(", not heard"),
        None => {}
    }
    if let Some(wait) = event.num(field::WAIT_S) {
        line.push_str(&format!(", waited {} s", num(wait, 2)));
    }
    if let Some(trigger) = opt(event, field::TRIGGER) {
        line.push_str(&format!(", trigger: {trigger}"));
    }
    if let Some(reason) = opt(event, field::REASON) {
        line.push_str(&format!("; why: {reason}"));
    }
    line
}

/// One comms entry as a transcript line, label padded into a column.
pub(crate) fn comms_line(event: &Event, names: &Names) -> String {
    comms(event, names, true)
}

/// One comms entry as a phrase, for descriptions and flags.
pub(crate) fn comms_text(event: &Event, names: &Names) -> String {
    comms(event, names, false)
}

/// A one-line plain-English description of any event.
pub(crate) fn describe(event: &Event, names: &Names) -> String {
    let s = event
        .subject
        .map(|id| names.who(id))
        .unwrap_or_else(|| "someone".into());
    // " at Enemy 2-1", or nothing when there is no object.
    let to = |word: &str| {
        event
            .object
            .map(|id| format!(" {word} {}", names.who(id)))
            .unwrap_or_default()
    };
    let weapon = event.id(field::WEAPON).map(|id| names.weapon(id));
    let shot = event
        .id(field::PROJECTILE)
        .map(|id| format!("shot {id}"))
        .unwrap_or_else(|| "a shot".into());
    let from_to = || {
        format!(
            "{} -> {}",
            opt(event, field::FROM).unwrap_or_else(|| "?".into()),
            opt(event, field::TO).unwrap_or_else(|| "?".into())
        )
    };
    match event.kind.as_str() {
        kind::WEAPON_LAUNCH => {
            let mut line = format!(
                "{s} launched {}{}",
                weapon.unwrap_or_else(|| "a weapon".into()),
                to("at")
            );
            if let Some(range) = event.num(field::RANGE_FT) {
                line.push_str(&format!(" from {} nm", nm(range)));
            }
            if let Some(aspect) = event.num(field::ASPECT_DEG) {
                line.push_str(&format!(", aspect {} deg", num(aspect, 0)));
            }
            line
        }
        kind::WEAPON_SEEKER_ACTIVE => format!("{shot} from {s}: seeker active{}", to("on")),
        kind::WEAPON_PITBULL => format!("{shot} from {s} went pitbull{}", to("on")),
        kind::WEAPON_TRACK_LOST => {
            format!("{shot} from {s} lost track{}{}", to("of"), because(event))
        }
        kind::WEAPON_DECOYED => {
            let decoy = match opt(event, field::DECOY) {
                Some(d) if d == "chaff" => d,
                Some(d) => format!("a {d}"),
                None => "a decoy".into(),
            };
            let roll = match (event.num(field::ROLL), event.num(field::THRESHOLD)) {
                (Some(r), Some(t)) => format!(" (roll {} < {})", num(r, 0), num(t, 0)),
                _ => String::new(),
            };
            format!("{shot} from {s} was decoyed by {decoy}{}{roll}", to("from"))
        }
        kind::WEAPON_OUTCOME => {
            let result = opt(event, field::RESULT).unwrap_or_else(|| "ended".into());
            let mut line = format!("{shot} from {s}{}: {result}", to("at"));
            if let Some(damage) = opt(event, field::DAMAGE) {
                line.push_str(&format!(", damage {damage}"));
            }
            if let Some(hp) = opt(event, field::HP_AFTER) {
                line.push_str(&format!(", hp after {hp}"));
            }
            if let Some(miss) = event.num(field::MISS_FT) {
                line.push_str(&format!(", missed by {} ft", num(miss, 0)));
            }
            line + &because(event)
        }
        kind::COMBAT_HIT => {
            let target = event
                .object
                .map(|id| format!(" {}", names.who(id)))
                .unwrap_or_default();
            let mut line = format!("{s} hit{target}");
            if let Some(weapon) = weapon {
                line.push_str(&format!(" with {weapon}"));
            }
            if let Some(damage) = opt(event, field::DAMAGE) {
                line.push_str(&format!(" for {damage} damage"));
            }
            if let Some(hp) = opt(event, field::HP_AFTER) {
                line.push_str(&format!(", hp now {hp}"));
            }
            line
        }
        kind::COMBAT_DESTROYED => {
            let mut line = format!("{s} was destroyed{}", to("by"));
            if let Some(weapon) = weapon {
                line.push_str(&format!(" ({weapon})"));
            }
            line + &because(event)
        }
        kind::COMBAT_AIRBURST => format!("{shot} from {s} burst{}", to("near")),
        kind::COMBAT_GROUND_IMPACT => format!("{shot} from {s} hit the ground"),
        kind::AIRCRAFT_CRASHED => format!("{s} crashed{}", because(event)),
        kind::AIRCRAFT_EJECTED => format!("{s} ejected{}", because(event)),
        kind::AIRCRAFT_PILOT_KILLED => format!("{s}: pilot killed{}", because(event)),
        kind::AIRCRAFT_TOOK_OFF => format!("{s} took off"),
        kind::AIRCRAFT_LANDED => {
            let grade = opt(event, field::GRADE)
                .map(|g| format!(" ({g})"))
                .unwrap_or_default();
            format!("{s} landed{grade}")
        }
        kind::AIRCRAFT_FLAMEOUT => format!("{s} flamed out{}", because(event)),
        kind::AIRCRAFT_FUEL_OUT => format!("{s} ran out of fuel"),
        kind::FLIGHT_DEPARTURE => format!("{s} departure mode {}{}", from_to(), because(event)),
        kind::FLIGHT_STALL => match event.flag(field::ON) {
            Some(false) => format!("{s} recovered from the stall"),
            _ => format!("{s} stalled{}", because(event)),
        },
        kind::FLIGHT_SPIN => {
            let direction = opt(event, field::DIRECTION)
                .map(|d| format!(" to the {d}"))
                .unwrap_or_default();
            match event.flag(field::ON) {
                Some(false) => format!("{s} recovered from the spin"),
                _ => format!("{s} entered a spin{direction}"),
            }
        }
        kind::FLIGHT_G_LIMIT => format!(
            "{s} asked for {} G against a limit of {} G{}",
            opt(event, field::ASKED).unwrap_or_else(|| "?".into()),
            opt(event, field::LIMIT).unwrap_or_else(|| "?".into()),
            because(event)
        ),
        kind::FLIGHT_STRUCTURAL_FAILURE => format!("{s} suffered a structural failure"),
        kind::FLIGHT_EFFECT => {
            let effect = opt(event, field::EFFECT).unwrap_or_else(|| "an effect".into());
            let factor = opt(event, field::FACTOR)
                .map(|f| format!(" ({f})"))
                .unwrap_or_default();
            match event.flag(field::ON) {
                Some(false) => format!("{s}: {effect} stopped"),
                _ if event.flag(field::MOMENTARY) == Some(true) => {
                    format!("{s}: {effect}{factor}{}", because(event))
                }
                _ => format!("{s}: {effect} applied{factor}{}", because(event)),
            }
        }
        kind::AI_ACTIVITY => format!("{s} activity {}{}", from_to(), because(event)),
        kind::AI_TARGET => {
            let new = event
                .id(field::TO)
                .or(event.object)
                .map(|id| names.who(id))
                .unwrap_or_else(|| "none".into());
            let old = event
                .id(field::FROM)
                .map(|id| names.who(id))
                .unwrap_or_else(|| "none".into());
            let mut line = format!("{s} target {old} -> {new}");
            if let Some(priority) = opt(event, field::PRIORITY) {
                line.push_str(&format!(", priority {priority}"));
            }
            if let Some(score) = event.num(field::SCORE) {
                line.push_str(&format!(", score {}", thousands(score)));
            }
            line + &because(event)
        }
        kind::AI_WEAPON_PHASE => format!("{s} weapon phase {}{}", from_to(), because(event)),
        kind::AI_DEFENSE => format!(
            "{s} defends{}: {}{}",
            to("against"),
            opt(event, field::REACTION).unwrap_or_else(|| "reaction".into()),
            because(event)
        ),
        kind::AI_FALLBACK => format!("{s} fell back {}{}", from_to(), because(event)),
        kind::AI_AIRFIELD_PHASE => format!("{s} airfield phase {}{}", from_to(), because(event)),
        kind::AI_EJECTION => format!(
            "{s} ejection check: {}{}",
            opt(event, field::DECISION).unwrap_or_else(|| "?".into()),
            because(event)
        ),
        k if k.starts_with("comms.") || k == kind::AUDIO_MUSIC => comms_text(event, names),
        kind::PLAYER_COMMAND => format!(
            "{s} command {}{}",
            opt(event, field::COMMAND).unwrap_or_default(),
            quoted(&event.text)
        ),
        kind::PLAYER_BOOKMARK => format!("Bookmark{}", quoted(&event.text)),
        kind::SYSTEM_PAUSE => "Paused".into(),
        kind::SYSTEM_RESUME => "Resumed".into(),
        kind::SYSTEM_TIME_SCALE => format!(
            "Time scale set to {}x",
            opt(event, field::SCALE).unwrap_or_else(|| "?".into())
        ),
        kind::SYSTEM_CHEAT => format!(
            "Cheat {} {}",
            opt(event, field::CHEAT).unwrap_or_default(),
            if event.flag(field::ON) == Some(false) {
                "off"
            } else {
                "on"
            }
        ),
        kind::SYSTEM_RESTART => "Mission restarted".into(),
        kind::SYSTEM_END => format!("Mission ended{}", because(event)),
        kind::SYSTEM_GAP => format!(
            "Recording gap from tick {} to {}",
            opt(event, field::FROM).unwrap_or_default(),
            opt(event, field::TO).unwrap_or_default()
        ),
        kind::SYSTEM_NOTE => format!("Note{}", quoted(&event.text)),
        other => {
            let mut line = other.to_owned();
            if event.subject.is_some() {
                line.push_str(&format!(" {s}"));
            }
            line.push_str(&to("->"));
            let fields = fields_text(event);
            if !fields.is_empty() {
                line.push_str(&format!(": {fields}"));
            }
            line + &quoted(&event.text)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clocks_and_numbers_read_naturally() {
        assert_eq!(clock(0), "0:00.0");
        assert_eq!(clock(119), "0:00.9");
        assert_eq!(clock(120 * 754 + 60), "12:34.5");
        assert_eq!(clock(120 * 3_723), "1:02:03.0");
        assert_eq!(num(1.2345, 2), "1.23");
        assert_eq!(num(2.0, 3), "2");
        assert_eq!(num(-0.0001, 2), "0");
        assert_eq!(nm(18_228.35), "3.0");
        assert_eq!(thousands(1_234_567.4), "1,234,567");
        assert_eq!(thousands(-999.), "-999");
        assert_eq!(thousands(12.), "12");
    }
}
