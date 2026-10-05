//! In-flight radio and crew voice delivery, shared by every speaker. The rules
//! are in docs/spec/radio-chatter.md#delivery-rules and
//! docs/spec/cockpit-voice.md#when-the-crew-may-speak. Producers decide what to
//! say; this module decides whether and when it is heard. It runs on
//! simulation seconds and never changes flight, AI or combat state.
//!
//! Every decision is also written to the [`journal`], with its trigger and
//! reason, for mission recordings. The journal is write-only.
use crate::{resources::ResourceSource, seats::SeatId};
use std::collections::{BTreeMap, VecDeque};

pub mod journal;
use journal::{Entry, Journal, Origin, Outcome, Reason};

/// Seconds every delivered line holds the whole channel (native).
pub const BUSY_SECONDS: f64 = 3.;
/// Calls waiting in the channel at most; a new one pushes out the oldest.
pub const PENDING_LIMIT: usize = 64;

/// Who a multi-crew player aircraft's crew is labelled as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Crew {
    Rio,
    CoPilot,
}
impl Crew {
    pub fn label(self) -> &'static str {
        match self {
            Crew::Rio => "RIO",
            Crew::CoPilot => "CO-PILOT",
        }
    }
}

/// The crew voice of an aircraft type: PLANE flags bit 0x4 marks multi-crew,
/// object class bit 0x4000 marks the non-fighter class (native).
pub fn crew(aircraft: &tore_formats::aircraft::Aircraft) -> Option<Crew> {
    let number = |map: &BTreeMap<String, tore_formats::aircraft::Token>, key: &str| {
        map.get(key).and_then(|t| t.number().ok())
    };
    let flags = number(&aircraft.fields, "flags")?;
    if flags & 0x4 == 0 {
        return None;
    }
    let class = number(&aircraft.object, "obj_class").unwrap_or(0);
    Some(if class & 0x4000 != 0 {
        Crew::CoPilot
    } else {
        Crew::Rio
    })
}

/// Whether radio silence may drop a call when it is sent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// Routine chatter: launch, hit, kill, engage, contact, "I'm hit",
    /// friendly fire, coaching, feet wet/dry and G sounds.
    Chatter,
    /// Never silenced: missile warnings, fuel, deaths, mission result.
    Important,
}

/// How the recordings are played.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route {
    /// The serial radio channel, with a text line.
    Radio,
    /// Tower speech, independently cancellable from wing and crew speech.
    Airport,
    /// A sound played directly without text, such as the player's death scream.
    Direct,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub label: String,
    pub text: String,
    /// Recording stems without `.5K`, played in order; missing ones are skipped.
    pub stems: Vec<String>,
    pub kind: Kind,
    pub route: Route,
    /// Seconds between sending and delivery: 0, 0.5, 2 or 5 in the original.
    pub delay: f64,
    /// Who made the call and why, for the journal. Delivery never reads it.
    pub origin: Origin,
}
impl Call {
    pub fn new(label: impl Into<String>, phrase: Phrase, kind: Kind) -> Self {
        Self {
            label: label.into(),
            text: phrase.text,
            stems: phrase.stems,
            kind,
            route: Route::Radio,
            delay: 0.,
            origin: Origin::default(),
        }
    }
    pub fn after(mut self, seconds: f64) -> Self {
        self.delay = seconds;
        self
    }
    /// Attach who made the call and why.
    pub fn because(mut self, origin: Origin) -> Self {
        self.origin = origin;
        self
    }
    pub fn direct(mut self) -> Self {
        self.route = Route::Direct;
        self
    }
    pub fn airport(mut self) -> Self {
        self.route = Route::Airport;
        self
    }
    /// The HUD line, `Speaker: 'text'`.
    pub fn line(&self) -> String {
        format!("{}: '{}'", self.label, self.text)
    }
}

/// Text and recordings assembled together, so they always agree.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Phrase {
    pub text: String,
    pub stems: Vec<String>,
}
impl Phrase {
    /// A reviewed pair: the stem's imported text and its recording.
    pub fn stem(phrases: &Phrases, stem: &str) -> Self {
        Self::default().then(phrases, stem)
    }
    /// Append a reviewed pair. A stem without imported text adds only its
    /// recording, so a missing cache entry never invents words.
    pub fn then(mut self, phrases: &Phrases, stem: &str) -> Self {
        if let Some(text) = phrases.get(stem) {
            self.text.push_str(text);
        }
        self.stems.push(stem.to_string());
        self
    }
    /// Append text with no recording, or a recording with no pair.
    pub fn raw(mut self, text: &str, stem: Option<&str>) -> Self {
        self.text.push_str(text);
        if let Some(stem) = stem {
            self.stems.push(stem.to_string());
        }
        self
    }
    pub fn join(mut self, other: Phrase) -> Self {
        self.text.push_str(&other.text);
        self.stems.extend(other.stems);
        self
    }
}

/// Imported phrase text keyed by stem, from `TORE_RADIO_<stem>` cache entries.
pub type Phrases = BTreeMap<String, String>;

pub fn phrases(resources: &dyn ResourceSource) -> Phrases {
    tore_formats::radio::STEMS
        .iter()
        .filter_map(|(stem, _)| {
            let bytes = resources.get(&format!("TORE_RADIO_{stem}"))?;
            (!bytes.is_empty() && bytes.len() <= 127 && bytes.iter().all(|b| (32..127).contains(b)))
                .then(|| {
                    (
                        stem.to_string(),
                        String::from_utf8_lossy(bytes).into_owned(),
                    )
                })
        })
        .collect()
}

const WORDS: [&str; 13] = [
    "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    "eleven", "twelve",
];

/// A number as the radio says it: 0..=12 as one word recording, larger values
/// digit by digit. `falling` uses the end-of-sentence intonation on the last
/// recording (native). Text is the decimal number.
pub fn number(n: u32, falling: bool) -> Phrase {
    let d = if falling { "D" } else { "" };
    let stems = if n <= 12 {
        vec![format!("^NUM{n:02}{d}")]
    } else {
        let digits: Vec<_> = n.to_string().bytes().map(|b| u32::from(b - b'0')).collect();
        let last = digits.len() - 1;
        digits
            .iter()
            .enumerate()
            .map(|(i, v)| format!("^NUM{v:02}{}", if i == last { d } else { "" }))
            .collect()
    };
    Phrase {
        text: n.to_string(),
        stems,
    }
}

/// Clock position 1..=12 of a relative bearing in degrees, clockwise from the
/// nose. Each hour covers 30 degrees centered on it (fitted: the original
/// rounding is unknown).
pub fn clock_hour(relative_bearing_deg: f64) -> u32 {
    let hour = (relative_bearing_deg.rem_euclid(360.) / 30.).round() as u32 % 12;
    if hour == 0 { 12 } else { hour }
}

/// Vertical qualifier of a clock call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Elevation {
    High,
    Level,
    Low,
}

/// "your two o'clock high". `falling` selects the sentence-final recording
/// when the call is level (native).
pub fn clock(
    phrases: &Phrases,
    your: bool,
    hour: u32,
    elevation: Elevation,
    falling: bool,
) -> Phrase {
    let hour = hour.clamp(1, 12);
    let mut p = Phrase::default();
    if your {
        p = p.then(phrases, "^YOUR");
    }
    let stem = if falling && elevation == Elevation::Level {
        format!("^CLCK{hour:02}D")
    } else {
        format!("^CLOCK{hour:02}")
    };
    p = p.raw(&format!("{} o'clock", WORDS[hour as usize]), Some(&stem));
    match elevation {
        Elevation::High => p.raw(" ", None).then(phrases, "^HIGH"),
        Elevation::Low => p.raw(" ", None).then(phrases, "^LOW"),
        Elevation::Level => p,
    }
}

/// "12 miles": whole miles, at least one. 1..=10, 20 and 30 have single
/// recordings; others are digits then "miles" (native).
pub fn miles(phrases: &Phrases, n: u32) -> Phrase {
    let n = n.max(1);
    let unit = if n == 1 { "^MILE" } else { "^MILES" };
    let text = format!("{n}{}", phrases.get(unit).map_or("", String::as_str));
    if n <= 10 || n == 20 || n == 30 {
        Phrase::default().raw(&text, Some(&format!("^MILE{n:02}")))
    } else {
        let digits = number(n, false);
        Phrase {
            text,
            stems: digits.stems.into_iter().chain([unit.to_string()]).collect(),
        }
    }
}

/// One seat a call is addressed to, with the speaker's name as that seat's
/// radio prints it: a seat hears its own voice as `YOU` (or its crew label)
/// and another aircraft's by flight colour and position.
#[derive(Clone, Debug, PartialEq)]
pub struct Hearer {
    pub seat: SeatId,
    /// The label this seat hears; `None` keeps the call's own.
    pub label: Option<String>,
    /// The words this seat hears, when they depend on where it is (a contact
    /// report's clock position); `None` keeps the call's own.
    pub words: Option<Phrase>,
}
impl Hearer {
    /// A seat that hears the call under the call's own label.
    pub fn seat(seat: SeatId) -> Self {
        Self {
            seat,
            label: None,
            words: None,
        }
    }
    /// A seat that hears the call under `label`.
    pub fn named(seat: SeatId, label: impl Into<String>) -> Self {
        Self {
            seat,
            label: Some(label.into()),
            words: None,
        }
    }
    /// The same seat hearing `words` instead of the call's own.
    pub fn saying(mut self, words: Phrase) -> Self {
        self.words = Some(words);
        self
    }
}

/// A call that is due for one seat.
#[derive(Clone, Debug, PartialEq)]
pub struct Delivery {
    pub seat: SeatId,
    pub call: Call,
}

/// A call waiting in a seat's queue.
struct Pending {
    due: f64,
    /// When it was sent.
    sent: f64,
    /// Its number in the journal.
    serial: u64,
    call: Call,
}

/// What each seat has of its own: its queue, its busy hold and its radio
/// silence setting (docs/ARCHITECTURE.md, "Radio, orders and debrief for each
/// seat").
struct Channel {
    seat: SeatId,
    radio_silence: bool,
    busy_until: f64,
    pending: Vec<Pending>,
    /// Radio-route lines delivered in the last [`BUSY_SECONDS`], which the
    /// mixer may still be playing, for [`Comms::cut_off`]. Journal only.
    recent: VecDeque<(f64, u64, Call)>,
}
impl Channel {
    fn new(seat: SeatId) -> Self {
        Self {
            seat,
            radio_silence: false,
            busy_until: f64::NEG_INFINITY,
            pending: Vec::new(),
            recent: VecDeque::new(),
        }
    }
}

/// The radio: every call is made once, with one variant roll, one journal
/// number and the shared cooldowns, and each seat that hears it queues it in
/// its own channel with its own busy hold.
pub struct Comms {
    /// One per seat, in seat order.
    channels: Vec<Channel>,
    cooldowns: BTreeMap<&'static str, f64>,
    /// Cooldowns a seat's own crew keeps, apart from the shared ones.
    seat_cooldowns: BTreeMap<(SeatId, &'static str), f64>,
    rng: u64,
    /// The last call number given out this flight.
    serial: u64,
    /// The latest simulation time the channel has seen, for entries made by
    /// methods that are not given one.
    clock: f64,
    journal: Journal,
}
impl Comms {
    /// A deterministic channel for seat 0; `seed` fixes the variant sequence.
    pub fn new(seed: u64) -> Self {
        Self::with_seats(seed, [SeatId::default()])
    }
    /// A deterministic channel with one queue for each of `seats`.
    pub fn with_seats(seed: u64, seats: impl IntoIterator<Item = SeatId>) -> Self {
        let mut comms = Self {
            channels: Vec::new(),
            cooldowns: BTreeMap::new(),
            seat_cooldowns: BTreeMap::new(),
            rng: seed | 1,
            serial: 0,
            clock: 0.,
            journal: Journal::default(),
        };
        comms.set_seats(seats);
        comms
    }
    /// Give exactly `seats` a queue: a seat already here keeps its own, a new
    /// one starts empty and a seat that is gone loses its queue.
    pub fn set_seats(&mut self, seats: impl IntoIterator<Item = SeatId>) {
        let mut wanted: Vec<SeatId> = seats.into_iter().collect();
        wanted.sort();
        wanted.dedup();
        self.channels.retain(|c| wanted.contains(&c.seat));
        for seat in wanted {
            if !self.channels.iter().any(|c| c.seat == seat) {
                self.channels.push(Channel::new(seat));
            }
        }
        self.channels.sort_by_key(|c| c.seat);
    }
    /// The seats with a queue, in seat order.
    pub fn seats(&self) -> impl Iterator<Item = SeatId> + '_ {
        self.channels.iter().map(|c| c.seat)
    }
    fn channel(&self, seat: SeatId) -> Option<&Channel> {
        self.channels.iter().find(|c| c.seat == seat)
    }
    fn channel_mut(&mut self, seat: SeatId) -> Option<&mut Channel> {
        self.channels.iter_mut().find(|c| c.seat == seat)
    }
    /// Clear everything tied to one flight, keeping the seats. Radio silence
    /// is a player setting and survives.
    pub fn restart(&mut self, seed: u64) {
        let silent: Vec<SeatId> = self
            .channels
            .iter()
            .filter(|c| c.radio_silence)
            .map(|c| c.seat)
            .collect();
        *self = Self::with_seats(
            seed,
            self.channels.iter().map(|c| c.seat).collect::<Vec<_>>(),
        );
        for channel in &mut self.channels {
            channel.radio_silence = silent.contains(&channel.seat);
        }
    }
    /// One roll from 0 to 99, as the original makes per call.
    pub fn roll(&mut self) -> u32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % 100) as u32
    }
    /// A variant by `roll` modulo the count (native).
    pub fn pick<'a>(roll: u32, variants: &[&'a str]) -> &'a str {
        variants[roll as usize % variants.len()]
    }
    /// A shared cooldown on game time: true, and restarted, when `key` is free.
    pub fn cooldown(&mut self, key: &'static str, now: f64, seconds: f64) -> bool {
        if self.cooldowns.get(key).is_some_and(|until| now < *until) {
            return false;
        }
        self.cooldowns.insert(key, now + seconds);
        true
    }
    /// A cooldown on game time that belongs to one seat, so each crew keeps its
    /// own: true, and restarted, when `key` is free for `seat`.
    pub fn seat_cooldown(
        &mut self,
        seat: SeatId,
        key: &'static str,
        now: f64,
        seconds: f64,
    ) -> bool {
        if self
            .seat_cooldowns
            .get(&(seat, key))
            .is_some_and(|until| now < *until)
        {
            return false;
        }
        self.seat_cooldowns.insert((seat, key), now + seconds);
        true
    }
    /// Seconds left on `seat`'s `key` cooldown at `now`, 0 when it is free. For
    /// the journal only.
    pub fn seat_remaining(&self, seat: SeatId, key: &'static str, now: f64) -> f64 {
        self.seat_cooldowns
            .get(&(seat, key))
            .map_or(0., |until| (until - now).max(0.))
    }
    /// Whether `key` is still cooling down, without starting it. For rules
    /// that check a cooldown on every call but restart it only on some.
    pub fn cooling(&self, key: &str, now: f64) -> bool {
        self.cooldowns.get(key).is_some_and(|until| now < *until)
    }
    /// Seconds left on `key`'s cooldown at `now`, 0 when it is free. For
    /// the journal only.
    pub fn remaining(&self, key: &str, now: f64) -> f64 {
        self.cooldowns
            .get(key)
            .map_or(0., |until| (until - now).max(0.))
    }
    /// Whether no line was delivered to `seat` in the last three seconds.
    /// Comment producers wait for this; radio calls do not (native).
    pub fn channel_free(&self, seat: SeatId, now: f64) -> bool {
        self.channel(seat).is_none_or(|c| now >= c.busy_until)
    }
    /// Whether `seat` has radio silence on.
    pub fn radio_silence(&self, seat: SeatId) -> bool {
        self.channel(seat).is_some_and(|c| c.radio_silence)
    }
    /// Send a call to the seats in `hearers`. The call is one call: it takes
    /// one journal number, and every seat that hears it queues the same words
    /// under its own label. Radio silence drops chatter when it is sent, never
    /// later, for the seat that has it on. The journal has one entry for the
    /// call's queueing and one for the seats that dropped it.
    pub fn send(&mut self, now: f64, call: Call, hearers: &[Hearer]) {
        self.clock = now;
        self.serial += 1;
        let serial = self.serial;
        let mut silenced: Vec<(SeatId, Call)> = Vec::new();
        let mut queued: Vec<(SeatId, Call)> = Vec::new();
        for hearer in hearers {
            let Some(channel) = self.channel(hearer.seat) else {
                debug_assert!(false, "seat {} has no radio channel", hearer.seat.0);
                continue;
            };
            let mut heard = call.clone();
            if let Some(label) = &hearer.label {
                heard.label.clone_from(label);
            }
            if let Some(words) = &hearer.words {
                heard.text.clone_from(&words.text);
                heard.stems.clone_from(&words.stems);
            }
            if channel.radio_silence && call.kind == Kind::Chatter {
                silenced.push((hearer.seat, heard));
            } else {
                queued.push((hearer.seat, heard));
            }
        }
        if let Some((_, first)) = silenced.first() {
            self.journal.push(
                Entry::call(
                    now,
                    Some(serial),
                    first,
                    Outcome::Dropped(Reason::RadioSilence),
                )
                .heard_by(silenced.iter().map(|(seat, _)| *seat)),
            );
        }
        let due = now + call.delay;
        for (seat, _) in &queued {
            let Some(channel) = self.channels.iter_mut().find(|c| c.seat == *seat) else {
                continue;
            };
            if channel.pending.len() >= PENDING_LIMIT {
                let oldest = channel.pending.remove(0);
                self.journal.push(
                    Entry::call(
                        now,
                        Some(oldest.serial),
                        &oldest.call,
                        Outcome::Dropped(Reason::QueueFull {
                            limit: PENDING_LIMIT,
                        }),
                    )
                    .heard_by([*seat]),
                );
            }
        }
        if let Some((_, first)) = queued.first() {
            self.journal.push(
                Entry::call(
                    now,
                    Some(serial),
                    first,
                    Outcome::Queued { due, expires: None },
                )
                .heard_by(queued.iter().map(|(seat, _)| *seat)),
            );
        }
        for (seat, heard) in queued {
            if let Some(channel) = self.channel_mut(seat) {
                channel.pending.push(Pending {
                    due,
                    sent: now,
                    serial,
                    call: heard,
                });
            }
        }
    }
    /// Send a call to every seat, each hearing it under the call's own label.
    pub fn send_all(&mut self, now: f64, call: Call) {
        let hearers: Vec<Hearer> = self.seats().map(Hearer::seat).collect();
        self.send(now, call, &hearers);
    }
    /// Calls due by `now`, for every seat, in due then send order. Each holds
    /// its seat's channel. The journal has one entry for each call, naming the
    /// seats it reached.
    pub fn due(&mut self, now: f64) -> Vec<Delivery> {
        self.clock = now;
        let mut ready: Vec<(SeatId, Pending)> = Vec::new();
        for channel in &mut self.channels {
            let mut mine = Vec::new();
            let mut i = 0;
            while i < channel.pending.len() {
                if channel.pending[i].due <= now {
                    mine.push(channel.pending.remove(i));
                } else {
                    i += 1;
                }
            }
            if mine
                .iter()
                .any(|p| matches!(p.call.route, Route::Radio | Route::Airport))
            {
                channel.busy_until = now + BUSY_SECONDS;
            }
            while channel
                .recent
                .front()
                .is_some_and(|(at, ..)| now - at >= BUSY_SECONDS)
            {
                channel.recent.pop_front();
            }
            ready.extend(mine.into_iter().map(|p| (channel.seat, p)));
        }
        ready.sort_by(|(a_seat, a), (b_seat, b)| {
            a.due
                .total_cmp(&b.due)
                .then(a.serial.cmp(&b.serial))
                .then(a_seat.cmp(b_seat))
        });
        let mut journaled: Vec<u64> = Vec::new();
        for (seat, p) in &ready {
            if !journaled.contains(&p.serial) {
                journaled.push(p.serial);
                let seats = ready
                    .iter()
                    .filter(|(_, other)| other.serial == p.serial)
                    .map(|(seat, _)| *seat);
                self.journal.push(
                    Entry::call(
                        now,
                        Some(p.serial),
                        &p.call,
                        Outcome::Delivered {
                            waited: now - p.sent,
                        },
                    )
                    .heard_by(seats),
                );
            }
            if p.call.route == Route::Radio
                && let Some(channel) = self.channels.iter_mut().find(|c| c.seat == *seat)
            {
                if channel.recent.len() >= 16 {
                    channel.recent.pop_front();
                }
                channel.recent.push_back((now, p.serial, p.call.clone()));
            }
        }
        ready
            .into_iter()
            .map(|(seat, p)| Delivery { seat, call: p.call })
            .collect()
    }
    /// Lines spoken outside this channel, such as wing orders, still hold
    /// `seat`'s.
    pub fn spoken(&mut self, seat: SeatId, now: f64) {
        self.clock = now;
        if let Some(channel) = self.channel_mut(seat) {
            channel.busy_until = now + BUSY_SECONDS;
        }
    }
    /// `seat`'s tower request was answered, so its airport calls still
    /// waiting are cancelled (the host's use). Other callers give their
    /// reason to [`Self::cancel_airport_because`].
    pub fn cancel_airport(&mut self, seat: SeatId) {
        self.cancel_airport_because(seat, Reason::TowerReply);
    }
    /// Cancel every airport call still waiting for `seat`, recording `reason`.
    pub fn cancel_airport_because(&mut self, seat: SeatId, reason: Reason) {
        let clock = self.clock;
        let Some(channel) = self.channels.iter_mut().find(|c| c.seat == seat) else {
            return;
        };
        let mut kept = Vec::with_capacity(channel.pending.len());
        for p in channel.pending.drain(..) {
            if p.call.route == Route::Airport {
                self.journal.push(
                    Entry::call(
                        clock,
                        Some(p.serial),
                        &p.call,
                        Outcome::Cancelled(reason.clone()),
                    )
                    .heard_by([seat]),
                );
            } else {
                kept.push(p);
            }
        }
        channel.pending = kept;
    }
    /// Alt-S for `seat`. Returns the HUD confirmation.
    pub fn toggle_silence(&mut self, seat: SeatId) -> &'static str {
        let Some(channel) = self.channel_mut(seat) else {
            return "Radio traffic OK";
        };
        channel.radio_silence = !channel.radio_silence;
        if channel.radio_silence {
            "Radio silence"
        } else {
            "Radio traffic OK"
        }
    }
    /// Add a producer's or the host's own entry.
    pub fn record(&mut self, entry: Entry) {
        self.journal.push(entry);
    }
    /// Add entries recorded elsewhere, such as the AI wings' orders and
    /// reports, so the host drains everything with one call.
    pub fn record_all(&mut self, entries: impl IntoIterator<Item = Entry>) {
        self.journal.extend(entries);
    }
    /// See [`Journal::first_in_window`].
    pub fn first_in_window(&mut self, rule: &'static str, id: u32, now: f64, until: f64) -> bool {
        self.journal.first_in_window(rule, id, now, until)
    }
}

// The host's journal hooks: the mission recorder drains the journal once a
// tick, and main.rs reports the wing order voice, which cuts wing speech off.
impl Comms {
    /// Speech outside the channel cut off the wing and crew lines the mixer
    /// may still be playing for `seat`, such as the player's wing order voice,
    /// which the host plays with interruption. Journal only: the lines listed
    /// are the radio lines delivered in the last [`BUSY_SECONDS`], which the
    /// channel hold still counts as speaking. A longer backlog in the mixer
    /// is not visible here.
    pub fn cut_off(&mut self, seat: SeatId, now: f64, reason: Reason) {
        self.clock = now;
        let Some(channel) = self.channels.iter_mut().find(|c| c.seat == seat) else {
            return;
        };
        for (at, serial, call) in channel.recent.drain(..) {
            if now - at < BUSY_SECONDS {
                self.journal.push(
                    Entry::call(
                        now,
                        Some(serial),
                        &call,
                        Outcome::Interrupted(reason.clone()),
                    )
                    .heard_by([seat]),
                );
            }
        }
    }
    /// Every journal entry since the last take, oldest first. The host
    /// calls this once a tick.
    pub fn take_journal(&mut self) -> Vec<Entry> {
        self.journal.take()
    }
    /// The journal, read without taking.
    pub fn journal(&self) -> &Journal {
        &self.journal
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table() -> Phrases {
        [
            ("^YOUR", "your "),
            ("^HIGH", "high"),
            ("^LOW", "low"),
            ("^MILE", " mile"),
            ("^MILES", " miles"),
            ("^CONTACT", "Contact, "),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
    }
    fn call(kind: Kind, delay: f64) -> Call {
        Call::new("Red two", Phrase::stem(&table(), "^CONTACT"), kind).after(delay)
    }
    const S0: SeatId = SeatId(0);
    const S1: SeatId = SeatId(1);
    /// Both seats and a call for each of them.
    fn two_seats() -> Comms {
        Comms::with_seats(1, [S0, S1])
    }

    #[test]
    fn numbers_words_digits_and_falling_intonation() {
        assert_eq!(number(7, false).stems, ["^NUM07"]);
        assert_eq!(number(12, true).stems, ["^NUM12D"]);
        assert_eq!(number(270, true).stems, ["^NUM02", "^NUM07", "^NUM00D"]);
        assert_eq!(number(270, true).text, "270");
    }

    #[test]
    fn clock_and_miles_compose_text_with_recordings() {
        let p = table();
        let c = clock(&p, true, 2, Elevation::High, false);
        assert_eq!(c.text, "your two o'clock high");
        assert_eq!(c.stems, ["^YOUR", "^CLOCK02", "^HIGH"]);
        assert_eq!(
            clock(&p, false, 9, Elevation::Level, true).stems,
            ["^CLCK09D"]
        );
        assert_eq!(clock_hour(0.), 12);
        assert_eq!(clock_hour(60.), 2);
        assert_eq!(clock_hour(-90.), 9);
        assert_eq!(clock_hour(344.), 11);
        assert_eq!(miles(&p, 0).stems, ["^MILE01"]);
        assert_eq!(miles(&p, 1).text, "1 mile");
        assert_eq!(miles(&p, 20).stems, ["^MILE20"]);
        let m = miles(&p, 12);
        assert_eq!(m.text, "12 miles");
        assert_eq!(m.stems, ["^NUM12", "^MILES"]);
        let m = miles(&p, 35);
        assert_eq!(m.stems, ["^NUM03", "^NUM05", "^MILES"]);
    }

    #[test]
    fn delay_order_busy_hold_and_silence() {
        let mut c = Comms::new(1);
        c.send_all(0., call(Kind::Chatter, 0.5));
        c.send_all(0., call(Kind::Important, 0.));
        let first = c.due(0.);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].call.kind, Kind::Important);
        assert!(!c.channel_free(S0, 2.9));
        assert_eq!(c.due(0.5).len(), 1);
        assert!(!c.channel_free(S0, 3.4));
        assert!(c.channel_free(S0, 3.5));
        assert_eq!(c.toggle_silence(S0), "Radio silence");
        c.send_all(4., call(Kind::Chatter, 0.));
        c.send_all(4., call(Kind::Important, 0.));
        let kept = c.due(4.);
        assert_eq!(
            kept.len(),
            1,
            "silence drops chatter, never important calls"
        );
        // A queued call keeps the setting in force when it was sent.
        c.toggle_silence(S0);
        c.send_all(5., call(Kind::Chatter, 2.));
        c.toggle_silence(S0);
        assert_eq!(c.due(7.).len(), 1);
        c.restart(9);
        assert!(c.radio_silence(S0), "silence is a player setting");
        assert!(c.channel_free(S0, 0.));
    }

    #[test]
    fn cooldowns_rolls_and_labels() {
        let mut c = Comms::new(3);
        assert!(c.cooldown("hit", 0., 4.));
        assert!(!c.cooldown("hit", 3.9, 4.));
        assert!(c.cooldown("hit", 4., 4.));
        assert!(c.cooling("hit", 7.9) && !c.cooling("hit", 8.) && !c.cooling("kill", 0.));
        let rolls: Vec<_> = (0..1000).map(|_| c.roll()).collect();
        assert!(rolls.iter().all(|r| *r < 100));
        assert!(rolls.iter().any(|r| *r < 10) && rolls.iter().any(|r| *r > 90));
        assert_eq!(Comms::pick(9, &["a", "b", "c"]), "a");
        assert_eq!(call(Kind::Chatter, 0.).line(), "Red two: 'Contact, '");
        assert_eq!(Crew::Rio.label(), "RIO");
        let mut a = Comms::new(5);
        let mut b = Comms::new(5);
        assert_eq!(
            (0..20).map(|_| a.roll()).collect::<Vec<_>>(),
            (0..20).map(|_| b.roll()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn the_journal_follows_a_call_from_queue_to_delivery() {
        let mut c = Comms::new(1);
        c.send_all(1., call(Kind::Chatter, 0.5));
        assert!(c.due(1.4).is_empty());
        assert_eq!(c.due(1.5).len(), 1);
        let entries = c.take_journal();
        assert_eq!(entries.len(), 2);
        assert_eq!(
            entries[0].outcome,
            Outcome::Queued {
                due: 1.5,
                expires: None
            }
        );
        assert_eq!(entries[1].outcome, Outcome::Delivered { waited: 0.5 });
        assert_eq!(entries[1].at, 1.5);
        assert_eq!(entries[0].call, Some(1));
        assert_eq!(entries[1].call, Some(1), "one number per call");
        assert_eq!(entries[1].label, "Red two");
        assert_eq!(entries[1].stems, ["^CONTACT"]);
        assert!(entries[1].heard() && !entries[0].heard());
        assert!(c.take_journal().is_empty(), "taking empties the journal");
    }

    #[test]
    fn radio_silence_drops_chatter_and_the_journal_says_so() {
        let mut c = Comms::new(1);
        c.toggle_silence(S0);
        c.send_all(0., call(Kind::Chatter, 0.));
        c.send_all(0., call(Kind::Important, 0.));
        let entries = c.take_journal();
        assert_eq!(entries[0].outcome, Outcome::Dropped(Reason::RadioSilence));
        assert_eq!(
            entries[1].outcome,
            Outcome::Queued {
                due: 0.,
                expires: None
            }
        );
        assert_eq!(c.due(0.).len(), 1, "the journal changes nothing");
    }

    #[test]
    fn a_full_queue_pushes_out_the_oldest_call() {
        let mut c = Comms::new(1);
        for i in 0..=PENDING_LIMIT {
            c.send_all(0., call(Kind::Chatter, 10. + i as f64));
        }
        let entries = c.take_journal();
        let dropped: Vec<_> = entries
            .iter()
            .filter(|e| matches!(e.outcome, Outcome::Dropped(_)))
            .collect();
        assert_eq!(dropped.len(), 1);
        assert_eq!(
            dropped[0].outcome,
            Outcome::Dropped(Reason::QueueFull { limit: 64 })
        );
        assert_eq!(dropped[0].call, Some(1), "the oldest call");
        assert_eq!(c.due(100.).len(), PENDING_LIMIT);
    }

    #[test]
    fn cancellations_and_a_wing_order_voice_are_journaled() {
        let mut c = Comms::new(1);
        c.send_all(0., call(Kind::Important, 5.).airport());
        c.send_all(0., call(Kind::Chatter, 5.));
        c.cancel_airport_because(S0, Reason::AircraftLost);
        let cancelled = c.take_journal().pop().unwrap();
        assert_eq!(cancelled.outcome, Outcome::Cancelled(Reason::AircraftLost));
        assert_eq!(cancelled.route, Some(Route::Airport));
        assert_eq!(c.due(5.).len(), 1, "the radio call stays");
        // A radio line 1 s old may still be playing; one 3 s old has ended,
        // and airport speech has its own queue in the mixer.
        c.send_all(7., call(Kind::Chatter, 0.));
        c.send_all(7., call(Kind::Chatter, 0.).airport());
        c.due(7.);
        c.take_journal();
        c.cut_off(S0, 8., Reason::OrderVoice);
        let cut = c.take_journal();
        assert_eq!(cut.len(), 1, "{cut:?}");
        assert_eq!(cut[0].outcome, Outcome::Interrupted(Reason::OrderVoice));
        assert_eq!(cut[0].route, Some(Route::Radio));
        c.cut_off(S0, 8.5, Reason::OrderVoice);
        assert!(c.take_journal().is_empty(), "each line is cut off once");
    }

    #[test]
    fn a_call_is_one_call_heard_by_each_seat_under_its_own_label() {
        let mut c = two_seats();
        let hearers = [Hearer::named(S0, "YOU"), Hearer::named(S1, "Red one")];
        c.send(1., call(Kind::Chatter, 0.5), &hearers);
        assert!(c.due(1.4).is_empty());
        let due = c.due(1.5);
        assert_eq!(due.len(), 2);
        assert_eq!((due[0].seat, due[0].call.label.as_str()), (S0, "YOU"));
        assert_eq!((due[1].seat, due[1].call.label.as_str()), (S1, "Red one"));
        assert_eq!(due[0].call.stems, due[1].call.stems, "one variant");
        let entries = c.take_journal();
        assert_eq!(entries.len(), 2, "one entry queues it, one delivers it");
        assert_eq!(entries[0].call, Some(1));
        assert_eq!(entries[1].call, Some(1), "one number for both seats");
        assert_eq!(entries[0].heard_by, [S0, S1]);
        assert_eq!(entries[1].heard_by, [S0, S1]);
    }

    #[test]
    fn each_seat_has_its_own_busy_hold_queue_and_silence() {
        let mut c = two_seats();
        c.send(0., call(Kind::Chatter, 0.), &[Hearer::seat(S0)]);
        c.due(0.);
        assert!(!c.channel_free(S0, 1.));
        assert!(c.channel_free(S1, 1.), "seat 1 heard nothing");
        c.spoken(S1, 2.);
        assert!(c.channel_free(S0, 3.) && !c.channel_free(S1, 3.));
        // Silence is a seat's own setting: it drops chatter for that seat only.
        assert_eq!(c.toggle_silence(S1), "Radio silence");
        assert!(!c.radio_silence(S0) && c.radio_silence(S1));
        c.take_journal();
        c.send_all(10., call(Kind::Chatter, 0.));
        let due = c.due(10.);
        assert_eq!(due.iter().map(|d| d.seat).collect::<Vec<_>>(), [S0]);
        let entries = c.take_journal();
        assert_eq!(entries[0].outcome, Outcome::Dropped(Reason::RadioSilence));
        assert_eq!(entries[0].heard_by, [S1]);
        assert_eq!(entries[1].heard_by, [S0]);
        assert_eq!(entries[0].call, entries[1].call, "still one call");
        // A full queue pushes out the oldest call of that seat only.
        for i in 0..=PENDING_LIMIT {
            c.send(
                20.,
                call(Kind::Important, 30. + i as f64),
                &[Hearer::seat(S0)],
            );
        }
        c.send(20., call(Kind::Important, 30.), &[Hearer::seat(S1)]);
        assert_eq!(c.due(200.).len(), PENDING_LIMIT + 1);
        // Cancelling airport speech and cutting off speech are per seat.
        c.send_all(220., call(Kind::Important, 5.).airport());
        c.cancel_airport(S0);
        assert_eq!(c.due(225.).iter().map(|d| d.seat).collect::<Vec<_>>(), [S1]);
    }

    #[test]
    fn each_crew_keeps_its_own_cooldown() {
        let mut c = two_seats();
        assert!(c.seat_cooldown(S0, "warn", 0., 6.));
        assert!(!c.seat_cooldown(S0, "warn", 3., 6.));
        assert!(c.seat_cooldown(S1, "warn", 3., 6.));
        assert_eq!(c.seat_remaining(S0, "warn", 3.), 3.);
        c.restart(2);
        assert_eq!(c.seats().collect::<Vec<_>>(), [S0, S1]);
        assert!(c.seat_cooldown(S0, "warn", 3., 6.));
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "comms_checkpoint.rs"]
mod checkpoint;
