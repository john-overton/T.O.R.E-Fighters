//! Anomaly flags, computed from a recording at export time so flight pays
//! nothing. Each flag is a suspect worth a look, not a verdict. Thresholds
//! are fitted agent decisions (2026-09-26), listed in [`Thresholds`] and in
//! docs/REPLAYS.md.

use super::text::{Names, clock, comms_text, num, seconds};
use crate::error::Result;
use crate::model::{AircraftState, Event, Frame, TICKS_PER_SECOND, TreeSample, Value};
use crate::reader::Recording;
use crate::vocab::{channel, field, kind, node, outcome, source};
use std::collections::{HashMap, HashSet, VecDeque};

/// Anomaly kinds.
pub mod kinds {
    /// A number that is NaN or infinite, in a state, event or tree.
    pub const NON_FINITE: &str = "non_finite";
    /// A position change one tick apart that the velocity does not explain.
    pub const TELEPORT: &str = "teleport";
    /// The nose or the wings turned further in one tick than any aircraft can.
    pub const ATTITUDE_JUMP: &str = "attitude_jump";
    /// Load factor beyond the threshold while alive.
    pub const G_EXCESS: &str = "g_excess";
    /// A control axis swinging back and forth rapidly.
    pub const CONTROL_OSCILLATION: &str = "control_oscillation";
    /// An AI aircraft stayed in one activity for a long time.
    pub const AI_STUCK: &str = "ai_stuck";
    /// An AI aircraft changed activity or target many times in a short span.
    pub const AI_FLIPPING: &str = "ai_flipping";
    /// A guided weapon lost its target soon after launch.
    pub const TRACK_LOST_EARLY: &str = "track_lost_early";
    /// An aircraft ran out of fuel.
    pub const FUEL_EXHAUSTED: &str = "fuel_exhausted";
    /// An order, request or report was rejected by a recipient.
    pub const ORDER_REJECTED: &str = "order_rejected";
    /// A call or message was dropped.
    pub const CALL_DROPPED: &str = "call_dropped";
    /// A call was suppressed (a cooldown, a limit or radio silence).
    pub const CALL_SUPPRESSED: &str = "call_suppressed";
    /// A call waited in a queue longer than the threshold.
    pub const LONG_WAIT: &str = "long_wait";
    /// The same call repeated several times in a short span.
    pub const REPEATED_CALL: &str = "repeated_call";
    /// A live aircraft's telemetry put it below the terrain.
    pub const BELOW_TERRAIN: &str = "below_terrain";
    /// An AI aircraft crashed with no damage: it flew into the ground.
    pub const CRASH_UNDAMAGED: &str = "crash_undamaged";
}

/// One flag.
#[derive(Clone, Debug, PartialEq)]
pub struct Anomaly {
    /// When it started.
    pub tick: u64,
    /// The aircraft it concerns, if any.
    pub subject: Option<u32>,
    /// One of [`kinds`].
    pub kind: &'static str,
    /// Plain-English detail with the numbers.
    pub detail: String,
}

/// Fitted thresholds. Defaults are agent decisions (2026-09-26).
#[derive(Clone, Debug, PartialEq)]
pub struct Thresholds {
    /// A one-tick position change this far from what the average velocity
    /// explains is a teleport. Quantization adds at most 1/32 ft.
    pub teleport_ft: f64,
    /// Nose or wing direction change in one tick, degrees. 20 degrees in a
    /// tick is 2,400 degrees per second, far past any aircraft.
    pub attitude_jump_deg: f64,
    /// Load factor above this, while alive, is flagged.
    pub g_high: f64,
    /// Load factor below this, while alive, is flagged.
    pub g_low: f64,
    /// An AI aircraft in one activity longer than this, seconds.
    pub stuck_s: f64,
    /// Activity or target changes within `flip_window_s` that count as flipping.
    pub flip_count: usize,
    pub flip_window_s: f64,
    /// Control reversals of at least `oscillation_swing` within
    /// `oscillation_window_s` that count as oscillation.
    pub oscillation_reversals: usize,
    pub oscillation_window_s: f64,
    pub oscillation_swing: f64,
    /// Track loss within this many seconds of launch.
    pub early_track_loss_s: f64,
    /// Queue waits longer than this, seconds.
    pub long_wait_s: f64,
    /// The same call this many times within `repeat_window_s`.
    pub repeat_count: usize,
    pub repeat_window_s: f64,
    /// Telemetry height above ground below this, feet, while alive.
    pub below_terrain_ft: f64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            teleport_ft: 50.,
            attitude_jump_deg: 20.,
            g_high: 9.5,
            g_low: -4.5,
            stuck_s: 300.,
            flip_count: 6,
            flip_window_s: 10.,
            oscillation_reversals: 8,
            oscillation_window_s: 2.,
            oscillation_swing: 0.5,
            early_track_loss_s: 2.,
            long_wait_s: 3.,
            repeat_count: 3,
            repeat_window_s: 10.,
            below_terrain_ft: -5.,
        }
    }
}

fn ticks(seconds: f64) -> u64 {
    (seconds * TICKS_PER_SECOND as f64).round().max(0.) as u64
}

/// Nose and up vectors for `[yaw, pitch, bank]`.
fn basis(a: [f64; 3]) -> ([f64; 3], [f64; 3]) {
    let (sy, cy) = a[0].sin_cos();
    let (sp, cp) = a[1].sin_cos();
    let (sb, cb) = a[2].sin_cos();
    (
        [sy * cp, sp, cy * cp],
        [cy * sb - sy * sp * cb, cp * cb, -sy * sb - cy * sp * cb],
    )
}

fn angle_between(a: [f64; 3], b: [f64; 3]) -> f64 {
    let dot = (a[0] * b[0] + a[1] * b[1] + a[2] * b[2]).clamp(-1., 1.);
    dot.acos().to_degrees()
}

fn non_finite_fields(s: &AircraftState) -> Vec<&'static str> {
    let mut out = Vec::new();
    let mut check = |name: &'static str, values: &[f64]| {
        if values.iter().any(|v| !v.is_finite()) {
            out.push(name);
        }
    };
    check("position", &s.position);
    check("attitude", &s.attitude);
    check("velocity", &s.velocity);
    check("airspeed", &[s.airspeed]);
    check("g", &[s.g]);
    check("devices", &s.devices);
    check("heat", &[s.heat]);
    check("fuel", &[s.fuel_lb]);
    check("controls", &s.controls);
    out
}

fn value_finite(v: &Value) -> bool {
    !matches!(v, Value::Num(n) if !n.is_finite())
}

struct Episode {
    start: u64,
    last: u64,
    peak: f64,
    detail: String,
}

/// Direction-change counter with hysteresis.
#[derive(Default)]
struct Zigzag {
    anchor: Option<f64>,
    direction: i8,
    reversals: VecDeque<u64>,
    reported_until: u64,
}

struct Detector<'a> {
    t: &'a Thresholds,
    names: Names<'a>,
    human: HashSet<u32>,
    out: Vec<Anomaly>,
    previous: HashMap<u32, (u64, AircraftState)>,
    episodes: HashMap<(u32, &'static str), Episode>,
    touched: HashSet<(u32, &'static str)>,
    zigzags: HashMap<(u32, usize), Zigzag>,
    activity: HashMap<u32, (String, u64)>,
    changes: HashMap<(u32, &'static str), VecDeque<u64>>,
    launches: HashMap<u32, u64>,
    out_of_fuel: HashSet<u32>,
    calls: HashMap<(Option<u32>, String), VecDeque<u64>>,
}

impl<'a> Detector<'a> {
    fn flag(&mut self, tick: u64, subject: Option<u32>, kind: &'static str, detail: String) {
        self.out.push(Anomaly {
            tick,
            subject,
            kind,
            detail,
        });
    }

    /// Keeps an episode open for this tick, tracking its peak.
    fn episode(&mut self, id: u32, kind: &'static str, tick: u64, value: f64, detail: String) {
        self.touched.insert((id, kind));
        let episode = self.episodes.entry((id, kind)).or_insert(Episode {
            start: tick,
            last: tick,
            peak: value,
            detail: detail.clone(),
        });
        episode.last = tick;
        if value.abs() > episode.peak.abs() {
            episode.peak = value;
            episode.detail = detail;
        }
    }

    /// Closes episodes that did not continue this tick.
    fn close_untouched(&mut self, force: bool) {
        let keys: Vec<_> = self
            .episodes
            .keys()
            .filter(|k| force || !self.touched.contains(*k))
            .copied()
            .collect();
        for key in keys {
            if let Some(e) = self.episodes.remove(&key) {
                let span = e.last - e.start + 1;
                let detail = match span {
                    1 => e.detail,
                    2..12 => format!("{} (for {span} ticks)", e.detail),
                    _ => format!("{} (for {})", e.detail, seconds(span)),
                };
                self.flag(e.start, Some(key.0), key.1, detail);
            }
        }
        self.touched.clear();
    }

    fn aircraft(&mut self, tick: u64, s: &AircraftState, restarted: bool) {
        let who = self.names.who(s.id);
        let bad = non_finite_fields(s);
        if !bad.is_empty() {
            let detail = format!("{who} has non-finite {}", bad.join(", "));
            self.episode(s.id, kinds::NON_FINITE, tick, 1., detail);
        }
        if let Some((last_tick, last)) = self.previous.remove(&s.id)
            && last_tick + 1 == tick
            && !restarted
            && bad.is_empty()
            && non_finite_fields(&last).is_empty()
        {
            let dt = 1. / TICKS_PER_SECOND as f64;
            let moved: Vec<f64> = (0..3).map(|i| s.position[i] - last.position[i]).collect();
            let expected: Vec<f64> = (0..3)
                .map(|i| (s.velocity[i] + last.velocity[i]) * 0.5 * dt)
                .collect();
            let error = (0..3)
                .map(|i| (moved[i] - expected[i]).powi(2))
                .sum::<f64>()
                .sqrt();
            if error > self.t.teleport_ft {
                let distance = moved.iter().map(|v| v * v).sum::<f64>().sqrt();
                let explained = expected.iter().map(|v| v * v).sum::<f64>().sqrt();
                let detail = format!(
                    "{who} moved {} ft in one tick; its velocity explains {} ft",
                    num(distance, 1),
                    num(explained, 1)
                );
                self.flag(tick, Some(s.id), kinds::TELEPORT, detail);
            }
            let (f0, u0) = basis(last.attitude);
            let (f1, u1) = basis(s.attitude);
            let turn = angle_between(f0, f1).max(angle_between(u0, u1));
            if turn > self.t.attitude_jump_deg {
                let detail = format!("{who} turned {} deg in one tick", num(turn, 1));
                self.flag(tick, Some(s.id), kinds::ATTITUDE_JUMP, detail);
            }
        }
        if s.flags.alive && (s.g > self.t.g_high || s.g < self.t.g_low) {
            let limit = if s.g > 0. {
                self.t.g_high
            } else {
                self.t.g_low
            };
            let detail = format!(
                "{who} at {} G, beyond the {} G threshold",
                num(s.g, 2),
                num(limit, 1)
            );
            self.episode(s.id, kinds::G_EXCESS, tick, s.g, detail);
        }
        if s.flags.alive && s.flags.engine_on && s.fuel_lb <= 0. && self.out_of_fuel.insert(s.id) {
            let detail = format!("{who} has no fuel with the engine running");
            self.flag(tick, Some(s.id), kinds::FUEL_EXHAUSTED, detail);
        }
        if s.flags.alive {
            for axis in 0..3 {
                self.zigzag(tick, s.id, axis, s.controls[axis]);
            }
        }
        self.previous.insert(s.id, (tick, s.clone()));
    }

    fn zigzag(&mut self, tick: u64, id: u32, axis: usize, v: f64) {
        if !v.is_finite() {
            return;
        }
        let swing = self.t.oscillation_swing;
        let window = ticks(self.t.oscillation_window_s);
        let needed = self.t.oscillation_reversals;
        let z = self.zigzags.entry((id, axis)).or_default();
        let anchor = *z.anchor.get_or_insert(v);
        let mut reversed = false;
        match z.direction {
            1 if v > anchor => z.anchor = Some(v),
            1 if anchor - v >= swing => {
                z.direction = -1;
                z.anchor = Some(v);
                reversed = true;
            }
            -1 if v < anchor => z.anchor = Some(v),
            -1 if v - anchor >= swing => {
                z.direction = 1;
                z.anchor = Some(v);
                reversed = true;
            }
            0 if (v - anchor).abs() >= swing => {
                z.direction = if v > anchor { 1 } else { -1 };
                z.anchor = Some(v);
            }
            _ => {}
        }
        if !reversed {
            return;
        }
        z.reversals.push_back(tick);
        while z.reversals.front().is_some_and(|t| tick - t > window) {
            z.reversals.pop_front();
        }
        if z.reversals.len() >= needed && tick > z.reported_until {
            z.reported_until = tick + window;
            let start = *z.reversals.front().unwrap_or(&tick);
            let detail = format!(
                "{} {} control reversed {} times in {}",
                self.names.who(id),
                crate::model::control::NAMES[axis],
                z.reversals.len(),
                seconds(tick - start)
            );
            self.flag(start, Some(id), kinds::CONTROL_OSCILLATION, detail);
        }
    }

    fn change(&mut self, tick: u64, id: u32, what: &'static str, latest: String) {
        let window = ticks(self.t.flip_window_s);
        let needed = self.t.flip_count;
        let list = self.changes.entry((id, what)).or_default();
        list.push_back(tick);
        while list.front().is_some_and(|t| tick - t > window) {
            list.pop_front();
        }
        if list.len() >= needed {
            let start = list.front().copied().unwrap_or(tick);
            let count = list.len();
            list.clear();
            let detail = format!(
                "{} changed {what} {count} times in {} (latest {latest})",
                self.names.who(id),
                seconds(tick - start)
            );
            self.flag(start, Some(id), kinds::AI_FLIPPING, detail);
        }
    }

    fn close_activity(&mut self, id: u32, until: u64) {
        if let Some((activity, since)) = self.activity.remove(&id)
            && until - since > ticks(self.t.stuck_s)
            && !self.human.contains(&id)
        {
            let detail = format!(
                "{} stayed in {activity} for {} without a change",
                self.names.who(id),
                super::text::duration(until - since)
            );
            self.flag(since, Some(id), kinds::AI_STUCK, detail);
        }
    }

    fn event(&mut self, tick: u64, e: &Event, frame: &Frame) {
        for (name, value) in &e.fields {
            if !value_finite(value) {
                let detail = format!("{} field {name} is not a finite number", e.kind);
                self.flag(tick, e.subject, kinds::NON_FINITE, detail);
            }
        }
        match e.kind.as_str() {
            kind::AI_ACTIVITY => {
                if let Some(id) = e.subject {
                    let to = e.string(field::TO).unwrap_or("?").to_owned();
                    let from = e.string(field::FROM).unwrap_or("?").to_owned();
                    self.close_activity(id, tick);
                    self.activity.insert(id, (to.clone(), tick));
                    self.change(tick, id, "activity", format!("{from} -> {to}"));
                }
            }
            kind::AI_TARGET => {
                if let Some(id) = e.subject {
                    let to = e
                        .id(field::TO)
                        .or(e.object)
                        .map(|t| self.names.who(t))
                        .unwrap_or_else(|| "none".into());
                    self.change(tick, id, "target", to);
                }
            }
            kind::WEAPON_LAUNCH => {
                if let Some(p) = e.id(field::PROJECTILE) {
                    self.launches.insert(p, tick);
                }
            }
            kind::WEAPON_TRACK_LOST => {
                if let Some(p) = e.id(field::PROJECTILE)
                    && let Some(&launched) = self.launches.get(&p)
                    && tick - launched <= ticks(self.t.early_track_loss_s)
                {
                    let detail = format!(
                        "shot {p} from {} lost track {} after launch",
                        e.subject.map(|s| self.names.who(s)).unwrap_or_default(),
                        seconds(tick - launched)
                    );
                    self.flag(tick, e.subject, kinds::TRACK_LOST_EARLY, detail);
                }
            }
            kind::AIRCRAFT_FUEL_OUT => {
                if let Some(id) = e.subject
                    && self.out_of_fuel.insert(id)
                {
                    let detail = format!("{} ran out of fuel", self.names.who(id));
                    self.flag(tick, Some(id), kinds::FUEL_EXHAUSTED, detail);
                }
            }
            kind::AIRCRAFT_CRASHED => {
                if let Some(id) = e.subject
                    && !self.human.contains(&id)
                    && let Some(s) = frame.aircraft.iter().find(|a| a.id == id)
                    && s.hp == s.max_hp
                {
                    let detail = format!(
                        "{} crashed with no damage ({} of {} hp)",
                        self.names.who(id),
                        s.hp,
                        s.max_hp
                    );
                    self.flag(tick, Some(id), kinds::CRASH_UNDAMAGED, detail);
                }
            }
            k if k.starts_with("comms.") => self.comms(tick, e),
            _ => {}
        }
    }

    fn comms(&mut self, tick: u64, e: &Event) {
        // The comms phrase already carries the outcome, wait and reason.
        let line = comms_text(e, &self.names);
        // Routine holds are not suspects: the HUD's rate limit for AI lines
        // and the AI's rules for which radio events become calls. A drop is
        // always a suspect.
        let routine = e.kind == kind::COMMS_HUD || e.string(field::SOURCE) == Some(source::CHATTER);
        match e.string(field::OUTCOME) {
            Some(outcome::REJECTED) => {
                self.flag(tick, e.subject, kinds::ORDER_REJECTED, line.clone());
            }
            Some(outcome::DROPPED) => self.flag(tick, e.subject, kinds::CALL_DROPPED, line.clone()),
            Some(outcome::SUPPRESSED) if !routine => {
                self.flag(tick, e.subject, kinds::CALL_SUPPRESSED, line.clone());
            }
            _ => {}
        }
        if e.num(field::WAIT_S)
            .is_some_and(|wait| wait > self.t.long_wait_s)
        {
            self.flag(tick, e.subject, kinds::LONG_WAIT, line);
        }
        let spoken = matches!(
            e.kind.as_str(),
            kind::COMMS_RADIO | kind::COMMS_CREW | kind::COMMS_TOWER | kind::COMMS_HUD
        );
        // Each saying once: not a line's queued or cut-off entry, nor one
        // that was never said.
        let played = !matches!(
            e.string(field::OUTCOME),
            Some(
                outcome::DROPPED
                    | outcome::SUPPRESSED
                    | outcome::CANCELLED
                    | outcome::QUEUED
                    | outcome::INTERRUPTED
                    | outcome::REPLACED
                    | outcome::EXPIRED
            )
        );
        if spoken && played && !e.text.is_empty() {
            let window = ticks(self.t.repeat_window_s);
            let needed = self.t.repeat_count;
            let list = self.calls.entry((e.subject, e.text.clone())).or_default();
            list.push_back(tick);
            while list.front().is_some_and(|t| tick - t > window) {
                list.pop_front();
            }
            if list.len() >= needed {
                let start = list.front().copied().unwrap_or(tick);
                let count = list.len();
                list.clear();
                let detail = format!(
                    "the same call {count} times in {}: \"{}\"",
                    seconds(tick - start),
                    e.text
                );
                self.flag(start, e.subject, kinds::REPEATED_CALL, detail);
            }
        }
    }

    fn tree(&mut self, tick: u64, tree: &TreeSample, frame: &Frame) {
        for n in &tree.nodes {
            if !value_finite(&n.value) {
                let detail = format!(
                    "{} line \"{}\" of {} is not a finite number",
                    tree.channel,
                    n.label,
                    self.names.who(tree.subject)
                );
                self.flag(tick, Some(tree.subject), kinds::NON_FINITE, detail);
            }
        }
        if tree.channel != channel::FLIGHT_TELEMETRY {
            return;
        }
        let alive = frame
            .aircraft
            .iter()
            .find(|a| a.id == tree.subject)
            .is_some_and(|a| a.flags.alive && !a.flags.crashed);
        if let Some(agl) = tree.node(node::AGL).and_then(|n| n.value.as_f64())
            && alive
            && agl < self.t.below_terrain_ft
        {
            let detail = format!(
                "{} is {} ft below the terrain while alive",
                self.names.who(tree.subject),
                num(-agl, 0)
            );
            self.episode(tree.subject, kinds::BELOW_TERRAIN, tick, agl, detail);
        }
    }

    fn frame(&mut self, frame: &Frame) {
        let tick = frame.tick;
        let restarted = frame.events.iter().any(|e| e.kind == kind::SYSTEM_RESTART);
        for s in &frame.aircraft {
            self.aircraft(tick, s, restarted);
        }
        for p in &frame.projectiles {
            let values = p
                .position
                .iter()
                .chain(&p.previous)
                .chain(&p.direction)
                .chain([&p.speed]);
            if values.clone().any(|v| !v.is_finite()) {
                let detail = format!(
                    "shot {} from {} has a non-finite position, direction or speed",
                    p.id,
                    self.names.who(p.owner)
                );
                self.episode(p.owner, kinds::NON_FINITE, tick, 1., detail);
            }
        }
        for e in &frame.events {
            self.event(tick, e, frame);
        }
        for tree in &frame.trees {
            self.tree(tick, tree, frame);
        }
        self.close_untouched(false);
    }
}

/// Every anomaly in the recording, in tick order.
pub fn detect(recording: &Recording, thresholds: &Thresholds) -> Result<Vec<Anomaly>> {
    let mut d = Detector {
        t: thresholds,
        names: Names::new(recording),
        human: recording
            .aircraft()
            .filter(|a| a.human)
            .map(|a| a.id)
            .collect(),
        out: Vec::new(),
        previous: HashMap::new(),
        episodes: HashMap::new(),
        touched: HashSet::new(),
        zigzags: HashMap::new(),
        activity: HashMap::new(),
        changes: HashMap::new(),
        launches: HashMap::new(),
        out_of_fuel: HashSet::new(),
        calls: HashMap::new(),
    };
    let w = &recording.header().world;
    let header_numbers = [w.time_of_day_s]
        .iter()
        .chain(&w.wind_fps)
        .chain(w.clouds.deck_ft.iter())
        .chain(w.extent_ft.iter().flatten())
        .all(|v| v.is_finite());
    if !header_numbers {
        d.flag(
            recording.first_tick().unwrap_or(0),
            None,
            kinds::NON_FINITE,
            "the header's world settings hold a non-finite number".into(),
        );
    }
    let mut last = 0;
    for frame in recording.frames(0, u64::MAX) {
        let frame = frame?;
        last = frame.tick;
        d.frame(&frame);
    }
    d.close_untouched(true);
    let open: Vec<u32> = d.activity.keys().copied().collect();
    for id in open {
        d.close_activity(id, last);
    }
    let mut out = d.out;
    out.sort_by(|a, b| {
        (a.tick, a.kind, a.subject)
            .cmp(&(b.tick, b.kind, b.subject))
            .then_with(|| a.detail.cmp(&b.detail))
    });
    Ok(out)
}

/// One anomaly as a text line.
pub fn describe(a: &Anomaly) -> String {
    format!("{}  {:<20} {}", clock(a.tick), a.kind, a.detail)
}
