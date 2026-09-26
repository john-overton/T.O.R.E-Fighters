//! The plain-English mission summary: what happened, who did what, every
//! shot, the comms transcript with reasons, a timeline, bookmarks with the
//! seconds around them, and anomaly flags.

use super::anomaly::{self, Thresholds};
use super::text::{
    FPS_PER_KT, Names, clock, comms_line, describe, duration, nm, num, seconds, thousands,
};
use crate::error::Result;
use crate::model::{AircraftState, Event, TICKS_PER_SECOND};
use crate::reader::{Recording, TimedEvent};
use crate::vocab::{channel, field, kind, node};
use std::collections::{BTreeMap, HashMap};
use std::io::Write;

/// Summary options.
#[derive(Clone, Debug, PartialEq)]
pub struct SummaryOptions {
    /// Anomaly thresholds.
    pub thresholds: Thresholds,
    /// Seconds shown before and after each bookmark. Default 5: the ten
    /// seconds around it.
    pub bookmark_window_s: f64,
}

impl Default for SummaryOptions {
    fn default() -> Self {
        Self {
            thresholds: Thresholds::default(),
            bookmark_window_s: 5.,
        }
    }
}

#[derive(Default)]
struct Stats {
    airborne: u64,
    max_g: Option<(f64, u64)>,
    min_g: Option<(f64, u64)>,
    lowest_msl: Option<(f64, u64)>,
    lowest_agl: Option<(f64, u64)>,
    fuel_first: Option<f64>,
    fuel_last: f64,
    fuel_used: f64,
    last: Option<AircraftState>,
    last_tick: u64,
    last_alive: Option<u64>,
    stalls: u32,
    spins: u32,
    shots: u32,
    hits: u32,
    kills: u32,
    activity: BTreeMap<String, u64>,
    current: Option<(String, u64)>,
}

fn lower(slot: &mut Option<(f64, u64)>, v: f64, tick: u64) {
    if v.is_finite() && slot.is_none_or(|(best, _)| v < best) {
        *slot = Some((v, tick));
    }
}

fn higher(slot: &mut Option<(f64, u64)>, v: f64, tick: u64) {
    if v.is_finite() && slot.is_none_or(|(best, _)| v > best) {
        *slot = Some((v, tick));
    }
}

struct Shot {
    tick: u64,
    event: Event,
    projectile: Option<u32>,
    last_seen: Option<u64>,
    peak_fps: f64,
    closest: Option<(f64, u64)>,
    outcome: Option<(u64, Event)>,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn heading_deg(yaw: f64) -> f64 {
    yaw.to_degrees().rem_euclid(360.)
}

fn time_of_day(seconds: f64) -> String {
    if !seconds.is_finite() {
        return "unknown".into();
    }
    let s = seconds.rem_euclid(86_400.).round() as u64 % 86_400;
    format!("{:02}:{:02}:{:02}", s / 3_600, s / 60 % 60, s % 60)
}

fn end_state(s: &AircraftState) -> String {
    let mut words = Vec::new();
    words.push(if s.flags.alive { "alive" } else { "destroyed" });
    if s.flags.crashed {
        words.push("crashed");
    }
    if s.flags.ejected {
        words.push("pilot ejected");
    }
    if s.flags.wreck_gone {
        words.push("wreck gone");
    }
    format!(
        "{}, {} of {} hp",
        words.join(", "),
        thousands(f64::from(s.hp)),
        thousands(f64::from(s.max_hp))
    )
}

/// Kinds that belong on the key-event timeline.
fn key_event(kind_name: &str) -> bool {
    kind_name == kind::WEAPON_LAUNCH
        || kind_name == kind::WEAPON_OUTCOME
        || kind_name == kind::WEAPON_TRACK_LOST
        || kind_name == kind::WEAPON_DECOYED
        || kind_name.starts_with("combat.")
        || kind_name.starts_with("aircraft.")
        || (kind_name.starts_with("flight.") && kind_name != kind::FLIGHT_EFFECT)
        || kind_name.starts_with("system.")
        || kind_name == kind::PLAYER_BOOKMARK
}

fn heading(out: &mut impl Write, title: &str) -> Result<()> {
    writeln!(out)?;
    writeln!(out, "{title}")?;
    writeln!(out, "{}", "-".repeat(title.len()))?;
    Ok(())
}

/// Writes the summary.
pub fn write_summary(
    recording: &Recording,
    options: &SummaryOptions,
    mut out: impl Write,
) -> Result<()> {
    let names = Names::new(recording);
    let header = recording.header();
    let w = &header.world;

    // One pass over every frame.
    let mut stats: BTreeMap<u32, Stats> = recording
        .aircraft()
        .map(|a| (a.id, Stats::default()))
        .collect();
    let mut shots: Vec<Shot> = Vec::new();
    let mut shot_by_projectile: HashMap<u32, usize> = HashMap::new();
    for frame in recording.frames(0, u64::MAX) {
        let frame = frame?;
        let tick = frame.tick;
        for s in &frame.aircraft {
            let st = stats.entry(s.id).or_default();
            if s.flags.airborne {
                st.airborne += 1;
            }
            if s.flags.alive {
                st.last_alive = Some(tick);
                higher(&mut st.max_g, s.g, tick);
                lower(&mut st.min_g, s.g, tick);
                lower(&mut st.lowest_msl, s.position[1], tick);
            }
            if s.fuel_lb.is_finite() {
                match st.fuel_first {
                    None => st.fuel_first = Some(s.fuel_lb),
                    Some(_) if s.fuel_lb < st.fuel_last => st.fuel_used += st.fuel_last - s.fuel_lb,
                    Some(_) => {}
                }
                st.fuel_last = s.fuel_lb;
            }
            st.last = Some(s.clone());
            st.last_tick = tick;
        }
        for tree in &frame.trees {
            if tree.channel == channel::FLIGHT_TELEMETRY
                && let Some(agl) = tree.node(node::AGL).and_then(|n| n.value.as_f64())
                && frame
                    .aircraft
                    .iter()
                    .any(|a| a.id == tree.subject && a.flags.alive)
            {
                lower(
                    &mut stats.entry(tree.subject).or_default().lowest_agl,
                    agl,
                    tick,
                );
            }
        }
        for e in &frame.events {
            let subject = e.subject;
            match e.kind.as_str() {
                kind::WEAPON_LAUNCH => {
                    if let Some(id) = subject {
                        stats.entry(id).or_default().shots += 1;
                    }
                    let projectile = e.id(field::PROJECTILE);
                    if let Some(p) = projectile {
                        shot_by_projectile.insert(p, shots.len());
                    }
                    shots.push(Shot {
                        tick,
                        event: e.clone(),
                        projectile,
                        last_seen: None,
                        peak_fps: 0.,
                        closest: None,
                        outcome: None,
                    });
                }
                kind::WEAPON_OUTCOME => {
                    if let Some(i) = e
                        .id(field::PROJECTILE)
                        .and_then(|p| shot_by_projectile.get(&p))
                    {
                        shots[*i].outcome = Some((tick, e.clone()));
                    }
                }
                kind::COMBAT_HIT => {
                    if let Some(id) = subject {
                        stats.entry(id).or_default().hits += 1;
                    }
                }
                kind::COMBAT_DESTROYED => {
                    if let Some(killer) = e.object {
                        stats.entry(killer).or_default().kills += 1;
                    }
                }
                kind::FLIGHT_STALL if e.flag(field::ON) != Some(false) => {
                    if let Some(id) = subject {
                        stats.entry(id).or_default().stalls += 1;
                    }
                }
                kind::FLIGHT_SPIN if e.flag(field::ON) != Some(false) => {
                    if let Some(id) = subject {
                        stats.entry(id).or_default().spins += 1;
                    }
                }
                kind::AI_ACTIVITY => {
                    if let Some(id) = subject {
                        let st = stats.entry(id).or_default();
                        if let Some((activity, since)) = st.current.take() {
                            *st.activity.entry(activity).or_default() += tick - since;
                        }
                        let to = e.string(field::TO).unwrap_or("?").to_owned();
                        st.current = Some((to, tick));
                    }
                }
                _ => {}
            }
        }
        for p in &frame.projectiles {
            if let Some(&i) = shot_by_projectile.get(&p.id) {
                let shot = &mut shots[i];
                shot.last_seen = Some(tick);
                if p.speed.is_finite() {
                    shot.peak_fps = shot.peak_fps.max(p.speed);
                }
                if let Some(target) = shot.event.object
                    && let Some(t) = frame.aircraft.iter().find(|a| a.id == target)
                {
                    let d = distance(p.position, t.position);
                    lower(&mut shot.closest, d, tick);
                }
            }
        }
    }
    let end = recording.last_tick().unwrap_or(0);
    for st in stats.values_mut() {
        if let Some((activity, since)) = st.current.take() {
            // Activity time counts while the aircraft is alive.
            let until = match (&st.last, st.last_alive) {
                (Some(_), Some(alive)) => alive,
                (Some(_), None) => since,
                (None, _) => end,
            };
            *st.activity.entry(activity).or_default() += (until + 1).saturating_sub(since);
        }
    }
    let anomalies = anomaly::detect(recording, &options.thresholds)?;

    let o = &mut out;
    writeln!(o, "T.O.R.E mission summary")?;
    writeln!(o, "=======================")?;
    writeln!(o)?;
    writeln!(
        o,
        "Recording   format {}, game {} ({}), recorded {}",
        header.format_version,
        if header.game_version.is_empty() {
            "unknown"
        } else {
            &header.game_version
        },
        if header.game_commit.is_empty() {
            "unknown commit"
        } else {
            &header.game_commit
        },
        if header.recorded_at.is_empty() {
            "at an unknown time"
        } else {
            &header.recorded_at
        }
    )?;
    let theater = match (w.theater_name.is_empty(), w.theater.is_empty()) {
        (false, false) => format!("{} ({})", w.theater_name, w.theater),
        (true, false) => w.theater.clone(),
        (false, true) => w.theater_name.clone(),
        (true, true) => "an unknown theater".into(),
    };
    let layout = if w.layout.is_empty() {
        String::new()
    } else {
        format!(", layout {}", w.layout)
    };
    writeln!(
        o,
        "Mission     {} in {theater}{layout}",
        header.mission.title()
    )?;
    let weather = match (w.weather_name.is_empty(), w.weather) {
        (false, Some(i)) => format!("{} (weather {i})", w.weather_name),
        (false, None) => w.weather_name.clone(),
        (true, Some(i)) => format!("weather {i}"),
        (true, None) => "unknown weather".into(),
    };
    let wind_speed = w.wind_fps[0].hypot(w.wind_fps[2]);
    let wind = if wind_speed > 0. {
        let toward = w.wind_fps[0]
            .atan2(w.wind_fps[2])
            .to_degrees()
            .rem_euclid(360.);
        format!(
            "wind {} ft/s toward {:03.0} deg (from {:03.0})",
            num(wind_speed, 1),
            toward,
            (toward + 180.) % 360.
        )
    } else {
        "calm".into()
    };
    writeln!(
        o,
        "Conditions  {weather}, starts {} local, {wind}",
        time_of_day(w.time_of_day_s)
    )?;
    let deck = w
        .clouds
        .deck_ft
        .map(|d| format!("scattered deck at {} ft", thousands(d)))
        .unwrap_or_else(|| "no scattered deck".into());
    if !w.clouds.module.is_empty() {
        writeln!(o, "Clouds      {}, {deck}", w.clouds.module)?;
    } else if w.clouds.deck_ft.is_some() {
        writeln!(o, "Clouds      {deck}")?;
    }
    if !header.extra.is_empty() {
        let extras: Vec<String> = header
            .extra
            .iter()
            .map(|(k, v)| format!("{k}={v}"))
            .collect();
        writeln!(o, "Settings    {}", extras.join("; "))?;
    }
    match recording.footer() {
        Some(footer) if !footer.result.is_empty() => {
            let result: Vec<String> = footer
                .result
                .iter()
                .map(|(k, v)| format!("{k} {v}"))
                .collect();
            writeln!(o, "Result      {}", result.join(", "))?;
        }
        Some(_) => writeln!(o, "Result      not recorded")?,
        None => writeln!(o, "Result      unknown: the recording did not finish")?,
    }
    let (first, last) = (recording.first_tick().unwrap_or(0), end);
    let frames = recording.frame_count();
    let length = if frames > 0 { last - first + 1 } else { 0 };
    writeln!(
        o,
        "Length      {} ({} frames, ticks {} to {}), {}",
        duration(length),
        thousands(frames as f64),
        first,
        last,
        if recording.complete() {
            "finished normally"
        } else {
            "INCOMPLETE: the game did not finish writing it (a crash, or still recording)"
        }
    )?;
    for (from, to) in recording.gaps() {
        writeln!(
            o,
            "Gap         no frames from {} to {}",
            clock(from),
            clock(to)
        )?;
    }
    for problem in recording.problems() {
        writeln!(o, "Problem     {problem}")?;
    }

    heading(o, "Aircraft")?;
    if stats.is_empty() {
        writeln!(o, "none")?;
    }
    for (id, st) in &stats {
        let info = recording.aircraft_info(*id);
        let mut title = names.who(*id);
        if let Some(info) = info {
            let mut parts = vec![format!("{}, {}", info.name, info.pt)];
            let mut side = info.side.name().to_owned();
            if info.wing > 0 {
                side.push_str(&format!(" wing {}-{}", info.wing, info.member));
            }
            parts.push(side);
            if info.human {
                parts.push("human".into());
            }
            if !info.skill.is_empty() {
                parts.push(info.skill.clone());
            }
            title = format!("{title} ({})", parts.join("; "));
        }
        writeln!(o, "{title}")?;
        if st.last.is_none() {
            writeln!(o, "  not in any recorded frame")?;
            continue;
        }
        let at = |slot: Option<(f64, u64)>, unit: &str, decimals: usize| match slot {
            Some((v, t)) if decimals == 0 => format!("{} {unit} at {}", thousands(v), clock(t)),
            Some((v, t)) => format!("{} {unit} at {}", num(v, decimals), clock(t)),
            None => "n/a".into(),
        };
        writeln!(
            o,
            "  airborne {} | max {} | min {} | lowest {} | lowest {}",
            duration(st.airborne),
            at(st.max_g, "G", 2),
            at(st.min_g, "G", 2),
            at(st.lowest_msl, "ft MSL", 0),
            match st.lowest_agl {
                Some(_) => at(st.lowest_agl, "ft AGL", 0),
                None => "height above ground not recorded".into(),
            }
        )?;
        let fuel = match st.fuel_first {
            Some(start) => format!(
                "fuel {} -> {} lb (used {} lb)",
                thousands(start),
                thousands(st.fuel_last),
                thousands(st.fuel_used)
            ),
            None => "fuel not recorded".into(),
        };
        writeln!(o, "  stalls {} | spins {} | {fuel}", st.stalls, st.spins)?;
        writeln!(
            o,
            "  shots {} | hits {} | kills {} | end: {}",
            st.shots,
            st.hits,
            st.kills,
            st.last.as_ref().map_or("never seen".into(), end_state)
        )?;
        if !st.activity.is_empty() {
            let mut list: Vec<(&String, &u64)> = st.activity.iter().collect();
            list.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            let parts: Vec<String> = list
                .iter()
                .map(|(activity, ticks)| format!("{activity} {}", seconds(**ticks)))
                .collect();
            writeln!(o, "  AI activity: {}", parts.join(", "))?;
        }
    }

    heading(o, "Shots")?;
    if shots.is_empty() {
        writeln!(o, "none")?;
    }
    for (i, shot) in shots.iter().enumerate() {
        let e = &shot.event;
        let shooter = e
            .subject
            .map(|s| names.who(s))
            .unwrap_or_else(|| "someone".into());
        let target = e
            .object
            .map(|s| names.who(s))
            .unwrap_or_else(|| "no target".into());
        let weapon = e
            .id(field::WEAPON)
            .map(|w| names.weapon(w))
            .unwrap_or_else(|| "weapon".into());
        let mode = e
            .string(field::MODE)
            .map(|m| format!(" ({m})"))
            .unwrap_or_default();
        writeln!(
            o,
            "#{} {}  {shooter} -> {target}  {weapon}{mode}",
            i + 1,
            clock(shot.tick)
        )?;
        let mut geometry = Vec::new();
        if let Some(v) = e.num(field::RANGE_FT) {
            geometry.push(format!("range {} nm", nm(v)));
        }
        if let Some(v) = e.num(field::ASPECT_DEG) {
            geometry.push(format!("aspect {} deg", num(v, 0)));
        }
        if let Some(v) = e.num(field::OFF_BORESIGHT_DEG) {
            geometry.push(format!("off boresight {} deg", num(v, 0)));
        }
        if let Some(v) = e.num(field::CLOSURE_KT) {
            geometry.push(format!("closure {} kt", thousands(v)));
        }
        if let (Some(a), Some(b)) = (e.num(field::SHOOTER_ALT_FT), e.num(field::TARGET_ALT_FT)) {
            geometry.push(format!("heights {} / {} ft", thousands(a), thousands(b)));
        }
        if let (Some(a), Some(b)) = (
            e.num(field::SHOOTER_SPEED_KT),
            e.num(field::TARGET_SPEED_KT),
        ) {
            geometry.push(format!("speeds {} / {} kt", thousands(a), thousands(b)));
        }
        if !geometry.is_empty() {
            writeln!(o, "   launch: {}", geometry.join(", "))?;
        }
        let flight = match shot.last_seen {
            Some(last) => {
                let mut parts = vec![
                    format!(
                        "time of flight {}",
                        seconds((last + 1).saturating_sub(shot.tick))
                    ),
                    format!("peak speed {} kt", thousands(shot.peak_fps / FPS_PER_KT)),
                ];
                if let Some((d, t)) = shot.closest {
                    parts.push(format!(
                        "closest approach {} ft at {}",
                        thousands(d),
                        clock(t)
                    ));
                }
                parts.join(", ")
            }
            None if shot.projectile.is_some() => "never seen in flight".into(),
            None => "no projectile recorded".into(),
        };
        writeln!(o, "   flight: {flight}")?;
        let outcome = match &shot.outcome {
            Some((when, result)) => {
                let mut text = result.string(field::RESULT).unwrap_or("ended").to_owned();
                if let Some(d) = result.get(field::DAMAGE) {
                    text.push_str(&format!(", damage {}", super::text::value_text(d)));
                }
                if let Some(h) = result.get(field::HP_AFTER) {
                    text.push_str(&format!(", hp after {}", super::text::value_text(h)));
                }
                if let Some(m) = result.num(field::MISS_FT) {
                    text.push_str(&format!(", missed by {} ft", num(m, 0)));
                }
                if let Some(r) = result.string(field::REASON) {
                    text.push_str(&format!(", because {r}"));
                }
                format!("{text} ({})", clock(*when))
            }
            None => "not recorded".into(),
        };
        writeln!(o, "   outcome: {outcome}")?;
    }

    heading(o, "Communication")?;
    let comms: Vec<&TimedEvent> = recording
        .events()
        .iter()
        .filter(|e| e.event.kind.starts_with("comms.") || e.event.kind == kind::AUDIO_MUSIC)
        .collect();
    if comms.is_empty() {
        writeln!(o, "none")?;
    }
    for e in comms {
        writeln!(o, "{}  {}", clock(e.tick), comms_line(&e.event, &names))?;
    }

    heading(o, "Timeline")?;
    let timeline: Vec<&TimedEvent> = recording
        .events()
        .iter()
        .filter(|e| key_event(&e.event.kind))
        .collect();
    if timeline.is_empty() {
        writeln!(o, "none")?;
    }
    for e in timeline {
        writeln!(o, "{}  {}", clock(e.tick), describe(&e.event, &names))?;
    }

    heading(o, "Bookmarks")?;
    let bookmarks: Vec<&TimedEvent> = recording
        .events()
        .iter()
        .filter(|e| e.event.kind == kind::PLAYER_BOOKMARK)
        .collect();
    if bookmarks.is_empty() {
        writeln!(o, "none")?;
    }
    let window = (options.bookmark_window_s * TICKS_PER_SECOND as f64).round() as u64;
    for b in bookmarks {
        let note = if b.event.text.is_empty() {
            String::new()
        } else {
            format!(" \"{}\"", b.event.text)
        };
        writeln!(o, "{}{note}", clock(b.tick))?;
        let from = b.tick.saturating_sub(window);
        let to = b.tick + window;
        writeln!(o, "  events from {} to {}:", clock(from), clock(to))?;
        let mut any = false;
        for e in recording.events_between(from, to) {
            if std::ptr::eq(e, b) {
                continue;
            }
            any = true;
            writeln!(o, "    {}  {}", clock(e.tick), describe(&e.event, &names))?;
        }
        if !any {
            writeln!(o, "    none")?;
        }
        if let Some(frame) = recording.frame(b.tick)? {
            writeln!(o, "  aircraft at {}:", clock(b.tick))?;
            for s in &frame.aircraft {
                writeln!(
                    o,
                    "    {}  {} ft MSL, {} kt, heading {:03.0}, G {}, {}",
                    names.who(s.id),
                    thousands(s.position[1]),
                    thousands(s.airspeed / FPS_PER_KT),
                    heading_deg(s.attitude[0]),
                    num(s.g, 1),
                    end_state(s)
                )?;
            }
        }
    }

    heading(o, "Anomalies")?;
    if anomalies.is_empty() {
        writeln!(o, "none")?;
    }
    for a in &anomalies {
        writeln!(o, "{}", anomaly::describe(a))?;
    }
    Ok(())
}
