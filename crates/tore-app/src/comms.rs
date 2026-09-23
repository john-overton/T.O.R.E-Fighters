//! In-flight radio and crew voice delivery, shared by every speaker. The rules
//! are in docs/spec/radio-chatter.md#delivery-rules and
//! docs/spec/cockpit-voice.md#when-the-crew-may-speak. Producers decide what to
//! say; this module decides whether and when it is heard. It runs on
//! simulation seconds and never changes flight, AI or combat state.
#![allow(dead_code)] // Removed once the radio and crew producers use every part.
use std::collections::BTreeMap;

/// Seconds every delivered line holds the whole channel (native).
pub const BUSY_SECONDS: f64 = 3.;

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
        }
    }
    pub fn after(mut self, seconds: f64) -> Self {
        self.delay = seconds;
        self
    }
    pub fn direct(mut self) -> Self {
        self.route = Route::Direct;
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

pub fn phrases(resources: &BTreeMap<String, Vec<u8>>) -> Phrases {
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

/// The channel: pending calls, the busy hold, shared cooldowns, radio silence
/// and the per-call random roll.
pub struct Comms {
    pub radio_silence: bool,
    busy_until: f64,
    pending: Vec<(f64, Call)>,
    cooldowns: BTreeMap<&'static str, f64>,
    rng: u64,
}
impl Comms {
    /// A deterministic channel; `seed` fixes the variant sequence.
    pub fn new(seed: u64) -> Self {
        Self {
            radio_silence: false,
            busy_until: f64::NEG_INFINITY,
            pending: Vec::new(),
            cooldowns: BTreeMap::new(),
            rng: seed | 1,
        }
    }
    /// Clear everything tied to one flight. Radio silence is a player setting
    /// and survives.
    pub fn restart(&mut self, seed: u64) {
        let silence = self.radio_silence;
        *self = Self::new(seed);
        self.radio_silence = silence;
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
    /// Whether no line was delivered in the last three seconds. Comment
    /// producers wait for this; radio calls do not (native).
    pub fn channel_free(&self, now: f64) -> bool {
        now >= self.busy_until
    }
    /// Radio silence drops chatter when it is sent, never later.
    pub fn send(&mut self, now: f64, call: Call) {
        if self.radio_silence && call.kind == Kind::Chatter {
            return;
        }
        if self.pending.len() >= 64 {
            self.pending.remove(0);
        }
        self.pending.push((now + call.delay, call));
    }
    /// Calls due by `now`, in due then send order. Each holds the channel.
    pub fn due(&mut self, now: f64) -> Vec<Call> {
        let mut ready = Vec::new();
        let mut i = 0;
        while i < self.pending.len() {
            if self.pending[i].0 <= now {
                ready.push(self.pending.remove(i));
            } else {
                i += 1;
            }
        }
        ready.sort_by(|a, b| a.0.total_cmp(&b.0));
        if ready.iter().any(|(_, c)| c.route == Route::Radio) {
            self.busy_until = now + BUSY_SECONDS;
        }
        ready.into_iter().map(|(_, call)| call).collect()
    }
    /// Lines spoken outside this channel, such as wing orders, still hold it.
    pub fn spoken(&mut self, now: f64) {
        self.busy_until = now + BUSY_SECONDS;
    }
    /// Alt-S. Returns the HUD confirmation.
    pub fn toggle_silence(&mut self) -> &'static str {
        self.radio_silence = !self.radio_silence;
        if self.radio_silence {
            "Radio silence"
        } else {
            "Radio traffic OK"
        }
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
        c.send(0., call(Kind::Chatter, 0.5));
        c.send(0., call(Kind::Important, 0.));
        let first = c.due(0.);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].kind, Kind::Important);
        assert!(!c.channel_free(2.9));
        assert_eq!(c.due(0.5).len(), 1);
        assert!(!c.channel_free(3.4));
        assert!(c.channel_free(3.5));
        assert_eq!(c.toggle_silence(), "Radio silence");
        c.send(4., call(Kind::Chatter, 0.));
        c.send(4., call(Kind::Important, 0.));
        let kept = c.due(4.);
        assert_eq!(
            kept.len(),
            1,
            "silence drops chatter, never important calls"
        );
        // A queued call keeps the setting in force when it was sent.
        c.toggle_silence();
        c.send(5., call(Kind::Chatter, 2.));
        c.toggle_silence();
        assert_eq!(c.due(7.).len(), 1);
        c.restart(9);
        assert!(c.radio_silence, "silence is a player setting");
        assert!(c.channel_free(0.));
    }

    #[test]
    fn cooldowns_rolls_and_labels() {
        let mut c = Comms::new(3);
        assert!(c.cooldown("hit", 0., 4.));
        assert!(!c.cooldown("hit", 3.9, 4.));
        assert!(c.cooldown("hit", 4., 4.));
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
}
