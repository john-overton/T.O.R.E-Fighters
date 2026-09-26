//! The communication journal: every radio call, crew remark, tower line, AI
//! text line and player order, with when it happened and why.
//!
//! Producers and the channel add [`Entry`] records as they decide. The host
//! drains them once a tick with [`super::Comms::take_journal`], for example
//! into a mission recording. Entries are write-only: no rule reads one back,
//! and building one never draws a random number, so what is said and when is
//! identical whether or not anything drains the journal. Opinionated
//! addition requested by John on 2026-09-26 (docs/REPLAYS.md, "Communication
//! journal"); the shape of the records is an agent decision.
// The mission recorder drains and reads the journal, and main.rs reports the
// wing order voice and the tower's replies. Until those host hooks land, some
// of this API is read only by tests.
#![allow(dead_code)]
use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use tore_sim::ai::{
    airfield::Phase,
    controller::Activity,
    formation,
    route::FuelState,
    wing::{PlayerOrder, ReceiverOutcome},
};

use super::{Call, Kind, Route};
use crate::audio::situation::{Inputs, Rank};
use crate::crew_voice::Situation;

/// Entries kept between drains. A host that never drains keeps the newest
/// ones and counts the rest in [`Journal::lost`].
pub const CAPACITY: usize = 1024;

/// A rule without a window of its own (a release with no target, a
/// friendly-fire hit too far away) is listed at most once per aircraft in
/// this many seconds; the gun fires one release per round.
pub const REPEAT_S: f64 = 4.;

/// A bounded list of entries waiting for the host.
#[derive(Clone, Debug, Default)]
pub struct Journal {
    entries: VecDeque<Entry>,
    lost: u64,
    /// The end of each suppression window already listed, by rule and
    /// aircraft. A repeat inside the same window is not listed again.
    windows: BTreeMap<(&'static str, u32), f64>,
}

impl Journal {
    pub fn push(&mut self, entry: Entry) {
        if self.entries.len() >= CAPACITY {
            self.entries.pop_front();
            self.lost += 1;
        }
        self.entries.push_back(entry);
    }

    pub fn extend(&mut self, entries: impl IntoIterator<Item = Entry>) {
        for entry in entries {
            self.push(entry);
        }
    }

    /// Every entry since the last take, oldest first.
    pub fn take(&mut self) -> Vec<Entry> {
        self.entries.drain(..).collect()
    }

    /// The entries waiting, oldest first, without taking them.
    pub fn entries(&self) -> impl Iterator<Item = &Entry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Entries dropped because the journal was full, since it was created.
    pub fn lost(&self) -> u64 {
        self.lost
    }

    /// Whether a suppression by `rule` of aircraft `id` at `now` opens a new
    /// window, which lasts until `until`. Only the first suppression in a
    /// window is listed, so a gun burst or a stream of hits makes one entry
    /// per cooldown, not one per round.
    pub fn first_in_window(&mut self, rule: &'static str, id: u32, now: f64, until: f64) -> bool {
        if self.windows.get(&(rule, id)).is_some_and(|end| now < *end) {
            return false;
        }
        self.windows.insert((rule, id), until);
        true
    }
}

/// One thing that happened to a line, an order or a report.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Simulation seconds on the producer's clock: the channel's `now`, or
    /// the AI mission's tick divided by 120. The host stamps drained entries
    /// with its own tick.
    pub at: f64,
    /// The line's number in this flight, shared by every entry about the
    /// same call (queued, then delivered, dropped or cancelled). `None` for
    /// lines that never reached the channel.
    pub call: Option<u64>,
    /// The speaker as printed (`Red two`, `RIO`, `YOU`), or the sender.
    pub label: String,
    /// The words; for an order, the message the HUD showed.
    pub text: String,
    /// Recording stems, played in order.
    pub stems: Vec<String>,
    pub route: Option<Route>,
    pub kind: Option<Kind>,
    pub origin: Origin,
    pub outcome: Outcome,
}

impl Entry {
    /// An entry about `call`.
    pub fn call(at: f64, serial: Option<u64>, call: &Call, outcome: Outcome) -> Self {
        Self {
            at,
            call: serial,
            label: call.label.clone(),
            text: call.text.clone(),
            stems: call.stems.clone(),
            route: Some(call.route),
            kind: Some(call.kind),
            origin: call.origin.clone(),
            outcome,
        }
    }

    /// An entry that is not a channel line: an order, an AI text line, a
    /// state change or a line that was never said.
    pub fn note(at: f64, label: impl Into<String>, origin: Origin, outcome: Outcome) -> Self {
        Self {
            at,
            call: None,
            label: label.into(),
            text: String::new(),
            stems: Vec::new(),
            route: None,
            kind: None,
            origin,
            outcome,
        }
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.text = text.into();
        self
    }

    pub fn with_stems(mut self, stems: Vec<String>) -> Self {
        self.stems = stems;
        self
    }

    pub fn with_kind(mut self, route: Route, kind: Kind) -> Self {
        self.route = Some(route);
        self.kind = Some(kind);
        self
    }

    /// The tower's answer to the player's own request (Shift-A and the
    /// other airport keys), which the host prints and plays directly
    /// without the channel.
    pub fn tower_reply(at: f64, text: impl Into<String>, stem: Option<&str>) -> Self {
        Self::note(
            at,
            "Tower",
            Origin::of(Source::Tower, Cause::TowerRequest).to(Audience::Player),
            Outcome::Delivered { waited: 0. },
        )
        .with_text(text)
        .with_stems(stem.map(str::to_string).into_iter().collect())
        .with_kind(Route::Airport, Kind::Important)
    }

    /// A player order the host refused before the wing saw it, for example
    /// "no AI wing" or a hostile airport for "land at selected airport".
    pub fn order_refused(at: f64, order: PlayerOrder, message: impl Into<String>) -> Self {
        let message = message.into();
        Self::note(
            at,
            "YOU",
            Origin::of(
                Source::Order,
                Cause::Order {
                    order,
                    selected: None,
                    target: None,
                },
            )
            .by(crate::ai_wings::PLAYER_ID),
            Outcome::Refused(Reason::Text(message.clone())),
        )
        .with_text(message)
    }

    pub fn source(&self) -> Source {
        self.origin.source
    }

    /// Whether the player heard or saw it: a delivered line, a line played
    /// directly, or a text line shown on the HUD.
    pub fn heard(&self) -> bool {
        matches!(self.outcome, Outcome::Delivered { .. })
    }

    /// One line of plain English, for logs and tests.
    pub fn describe(&self) -> String {
        let mut line = format!("{:.2}s {}", self.at, self.origin.source.name());
        if let Some(call) = self.call {
            line.push_str(&format!(" #{call}"));
        }
        if !self.label.is_empty() {
            line.push_str(&format!(" {}", self.label));
        }
        if !self.text.is_empty() {
            line.push_str(&format!(": '{}'", self.text));
        }
        line.push_str(&format!(" {}", self.outcome));
        line.push_str(&format!("; why: {}", self.origin.cause));
        for roll in &self.origin.rolls {
            line.push_str(&format!("; {roll}"));
        }
        line
    }
}

/// Who made a line, to whom and why. A [`Call`] carries it through the
/// channel so every entry about the call names its trigger.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Origin {
    pub source: Source,
    /// Actor id of the speaker or sender; 0 is the player.
    pub speaker: Option<u32>,
    pub audience: Audience,
    pub cause: Cause,
    /// Random draws the line used, in the order they were made.
    pub rolls: Vec<Roll>,
    /// When the line entered its producer's own queue, for tower notices,
    /// which wait there for the channel before they are sent.
    pub since: Option<f64>,
}

impl Origin {
    pub fn of(source: Source, cause: Cause) -> Self {
        Self {
            source,
            cause,
            ..Self::default()
        }
    }
    pub fn by(mut self, speaker: u32) -> Self {
        self.speaker = Some(speaker);
        self
    }
    pub fn to(mut self, audience: Audience) -> Self {
        self.audience = audience;
        self
    }
    pub fn rolls(mut self, rolls: Vec<Roll>) -> Self {
        self.rolls = rolls;
        self
    }
}

/// What kind of communication an entry is about.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Source {
    /// An aircraft's radio call: launches, hits, kills, "I'm hit", deaths,
    /// SAM and AAM launches, contacts, fuel, friendly fire, mission result.
    #[default]
    Radio,
    /// A wingman's radio answer to the player's order.
    Reply,
    /// The player's crew, or the first wingman coaching a single-seat player.
    Crew,
    /// Airport speech and the wingmen's airfield status.
    Tower,
    /// A text line the AI posts on the HUD: activity changes and formation
    /// reports.
    Hud,
    /// A player order to the wing.
    Order,
    /// An AI radio event before it becomes a call.
    Chatter,
    /// What the situation music observes.
    Music,
}

impl Source {
    pub fn name(self) -> &'static str {
        match self {
            Source::Radio => "RADIO",
            Source::Reply => "REPLY",
            Source::Crew => "CREW",
            Source::Tower => "TOWER",
            Source::Hud => "HUD",
            Source::Order => "ORDER",
            Source::Chatter => "CHATTER",
            Source::Music => "MUSIC",
        }
    }
}

/// Who a line is addressed to.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Audience {
    #[default]
    Unknown,
    /// The player's own cockpit: crew remarks and the death scream.
    Cockpit,
    /// The speaker's flight leader.
    Leader,
    /// Everyone else in the speaker's flight.
    Flight,
    /// The player alone.
    Player,
    /// The airport frequency.
    Airport,
    /// The player's wingmen: every one, or the one member number.
    Wing { member: Option<u8> },
}

impl fmt::Display for Audience {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Audience::Unknown => write!(f, "unknown"),
            Audience::Cockpit => write!(f, "the cockpit"),
            Audience::Leader => write!(f, "the flight leader"),
            Audience::Flight => write!(f, "the flight"),
            Audience::Player => write!(f, "you"),
            Audience::Airport => write!(f, "the airport frequency"),
            Audience::Wing { member: None } => write!(f, "all wingmen"),
            Audience::Wing {
                member: Some(member),
            } => write!(f, "wingman {member}"),
        }
    }
}

/// One random draw from the channel's roll, 0 to 99.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Roll {
    /// What the draw decided.
    pub rule: &'static str,
    pub value: u32,
    pub test: Test,
}

impl Roll {
    pub fn new(rule: &'static str, value: u32, test: Test) -> Self {
        Self { rule, value, test }
    }
    /// A variant chosen by the draw modulo `count`.
    pub fn pick(rule: &'static str, value: u32, count: usize) -> Self {
        Self::new(rule, value, Test::Modulo(count as u32))
    }
}

/// How a draw was read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Test {
    /// Passed when the draw is below this number.
    Below(u32),
    /// Passed when the draw is at least this number.
    AtLeast(u32),
    /// Chose `value % n`: a variant, or that many seconds.
    Modulo(u32),
    /// Chose the band the draw falls in, split at these numbers.
    Bands(&'static [u32]),
    /// Drawn, but it did not change this line.
    Unused,
}

impl fmt::Display for Roll {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = self.value;
        match self.test {
            Test::Below(n) => write!(
                f,
                "roll {value} {} {n}: {}",
                if value < n { "<" } else { ">=" },
                self.rule
            ),
            Test::AtLeast(n) => write!(
                f,
                "roll {value} {} {n}: {}",
                if value >= n { ">=" } else { "<" },
                self.rule
            ),
            Test::Modulo(n) => write!(
                f,
                "roll {value} mod {n} = {}: {}",
                value % n.max(1),
                self.rule
            ),
            Test::Bands(cuts) => write!(
                f,
                "roll {value}, band {} of {}: {}",
                cuts.iter().filter(|cut| value >= **cut).count() + 1,
                cuts.len() + 1,
                self.rule
            ),
            Test::Unused => write!(f, "roll {value} drawn, unused: {}", self.rule),
        }
    }
}

/// "you" for the player, otherwise the aircraft's id.
fn who(id: u32) -> String {
    if id == crate::ai_wings::PLAYER_ID {
        "you".into()
    } else {
        format!("aircraft {id}")
    }
}

fn feet(value: f64) -> String {
    format!("{:.0} ft", value)
}

/// A released store as the launch call reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Store {
    /// Type flags: 0x1 guided, 0x10 bomb.
    pub flags: u32,
    /// Seeker signature: 2 infrared, 3 radar.
    pub seeker: u8,
    pub phoenix: bool,
}

impl fmt::Display for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let guided = self.flags & 1 != 0;
        let text = if self.phoenix {
            "AIM-54 Phoenix"
        } else if self.flags & 0x10 != 0 {
            "bomb"
        } else {
            match (guided, self.seeker) {
                (true, 3) => "radar missile",
                (true, 2) => "infrared missile",
                (true, _) => "guided weapon",
                (false, _) => "gun or rocket",
            }
        };
        f.write_str(text)
    }
}

/// What kind of object fired the round that hit an aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Attacker {
    Aircraft,
    Aaa,
    Other,
}

/// A wingman's radio answer to the player's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WingReply {
    /// An engage variant; `aircraft` when the ordered target is an aircraft.
    Engage { aircraft: bool },
    /// "Showtime!" for "protect me".
    Showtime,
}

/// The player's airfield events the tower speaks about.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TowerEvent {
    TakeoffClearance,
    RunwayOccupied,
    Airborne,
    Farewell,
    LandingClearance,
    Wind { knots: u32 },
    LandingGrade { score: u32 },
    Welcome,
}

/// One coaching check by the crew or the single-seat wingman.
#[derive(Clone, Debug, PartialEq)]
pub struct Coaching {
    pub situation: Situation,
    /// The situation of the previous check.
    pub previous: Option<Situation>,
    /// Target range, feet, when there is a target.
    pub range_ft: Option<f64>,
    /// The rule that chose the line, or why there was none.
    pub rule: &'static str,
    /// Seconds until the next check may speak (unless the situation
    /// changes first).
    pub next_s: f64,
}

/// What the situation music observes, and why each input is on. The rank
/// is the one the inputs ask for; the mixer may keep the current score
/// (1 s lockout, the success and home scores play once, the Valkyries
/// toggle, a failed-load retry), and its choice is not visible here.
#[derive(Clone, Debug, PartialEq)]
pub struct Music {
    pub from: Option<Rank>,
    pub to: Rank,
    pub inputs: Inputs,
    /// The designated enemy aircraft and its range, feet: AIR within
    /// 40,000 ft, DANGER beyond.
    pub designated: Option<(u32, f64)>,
    /// AI aircraft aiming at the player with a guided missile ready, which
    /// holds DANGER for 4 s.
    pub aiming: Vec<u32>,
    /// Guided missiles inbound on the player.
    pub inbound: Vec<u32>,
    /// When the player was last hit, which holds AIR for 30 s.
    pub hit_at: Option<f64>,
}

/// The trigger of a line, an order or a report, with its numbers.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Cause {
    /// Nothing attached: a line the host sent without a cause.
    #[default]
    Unspecified,
    /// A store left the speaker's aircraft.
    Release { target: Option<u32>, store: Store },
    /// A round from the speaker damaged `victim` without destroying it.
    Hit { victim: u32, guided: bool },
    /// A round from the speaker destroyed `victim`.
    Kill {
        victim: u32,
        aircraft: bool,
        bomb: bool,
    },
    /// The speaker's aircraft was hit by `by` (the "I'm hit" call).
    Damaged {
        by: u32,
        attacker: Attacker,
        gun: bool,
    },
    /// The player's round hit the speaker, a friendly `range_ft` from the
    /// player.
    FriendlyFire { range_ft: f64 },
    /// The speaker saw an opposite-side launch it accepted as a warning.
    LaunchSeen { by_aircraft: bool },
    /// The speaker's aircraft was destroyed.
    Death { ejection_seat: bool },
    /// The speaker accepted the player's order.
    Reply(WingReply),
    /// The speaker's target changed: a contact report may follow.
    NewTarget { target: u32 },
    /// The speaker picked up a new target to report.
    Contact {
        target: u32,
        count: u32,
        miles: u32,
        advise: bool,
    },
    /// The speaker's fuel reached a level: 1 joker, 2 bingo, 3 fumes, 4 out.
    AiFuel { level: u8 },
    /// The mission succeeded (the debrief's evaluator, checked every 4 s).
    MissionAccomplished,
    /// After a success, the player came within 42,240 ft of the home base
    /// below 20,000 ft.
    AlmostHome { range_ft: f64, altitude_ft: f64 },
    /// The player's aircraft was destroyed without an ejection.
    Destroyed,
    /// The player's fuel state became worse than any state called before.
    Fuel { state: FuelState },
    /// A missile fired at the player.
    MissileLaunch {
        missile: u32,
        /// Seconds since launch.
        age: f64,
        /// Seeker class: 2 infrared, 3 radar.
        signature: u8,
    },
    /// The G load entered the hard band (5 G and above, -3 G and below).
    HardG { g: i32, previous: i32 },
    /// Crossings of -1 G in this minute of the mission clock.
    Crossings { count: u32 },
    /// A coaching check.
    Coaching(Box<Coaching>),
    /// Whether the crew may make comments changed.
    CrewGate {
        from: Option<Gate>,
        to: Option<Gate>,
    },
    /// The player's airfield event.
    Tower(TowerEvent),
    /// A wingman's airfield phase changed (`None`: it left the airfield).
    WingStatus {
        phase: Option<Phase>,
        go_around: bool,
    },
    /// The player's own request to the tower.
    TowerRequest,
    /// The player's order to the wing. `selected` is the designated target;
    /// `target` the one the order uses, when it is a living hostile.
    Order {
        order: PlayerOrder,
        selected: Option<u32>,
        target: Option<u32>,
    },
    /// A formation report from the wingman's formation phase.
    Formation {
        phase: formation::Phase,
        seconds: f64,
        closure_fps: f64,
    },
    /// An AI aircraft's activity changed.
    Activity { activity: Activity },
    /// The situation music's inputs changed.
    Music(Box<Music>),
}

impl fmt::Display for Cause {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Cause::Unspecified => write!(f, "no cause recorded"),
            Cause::Release {
                target: Some(target),
                store,
            } => write!(f, "{store} release at {}", who(*target)),
            Cause::Release {
                target: None,
                store,
            } => write!(f, "{store} release with no target"),
            Cause::Hit { victim, guided } => write!(
                f,
                "{} hit on {}",
                if *guided { "guided" } else { "unguided" },
                who(*victim)
            ),
            Cause::Kill {
                victim,
                aircraft,
                bomb,
            } => write!(
                f,
                "{} destroyed ({}{})",
                who(*victim),
                if *aircraft {
                    "aircraft"
                } else {
                    "not an aircraft"
                },
                if *bomb { ", by a bomb" } else { "" }
            ),
            Cause::Damaged { by, attacker, gun } => write!(
                f,
                "hit by {} ({}{})",
                who(*by),
                match attacker {
                    Attacker::Aircraft => "aircraft",
                    Attacker::Aaa => "anti-aircraft fire",
                    Attacker::Other => "other fire",
                },
                if *gun { ", gun rounds" } else { "" }
            ),
            Cause::FriendlyFire { range_ft } => write!(
                f,
                "hit by your round, a friendly {} from you",
                feet(*range_ft)
            ),
            Cause::LaunchSeen { by_aircraft } => write!(
                f,
                "saw an enemy {} launch",
                if *by_aircraft { "air-to-air" } else { "SAM" }
            ),
            Cause::Death { ejection_seat } => write!(
                f,
                "aircraft destroyed{}",
                if *ejection_seat {
                    " (ejection seat)"
                } else {
                    ""
                }
            ),
            Cause::Reply(WingReply::Engage { aircraft }) => write!(
                f,
                "accepted your attack order ({} target)",
                if *aircraft { "aircraft" } else { "surface" }
            ),
            Cause::Reply(WingReply::Showtime) => write!(f, "accepted \"protect me\""),
            Cause::NewTarget { target } => write!(f, "new target {}", who(*target)),
            Cause::Contact {
                target,
                count,
                miles,
                advise,
            } => write!(
                f,
                "new contact {}, {count} aircraft, {miles} nm{}",
                who(*target),
                if *advise {
                    ", under formation control"
                } else {
                    ""
                }
            ),
            Cause::AiFuel { level } => write!(
                f,
                "fuel {}",
                match level {
                    1 => "joker",
                    2 => "bingo",
                    3 => "fumes",
                    _ => "out",
                }
            ),
            Cause::MissionAccomplished => write!(
                f,
                "the mission succeeded (debrief evaluator, checked every 4 s)"
            ),
            Cause::AlmostHome {
                range_ft,
                altitude_ft,
            } => write!(
                f,
                "after success, home base {} away at {} altitude",
                feet(*range_ft),
                feet(*altitude_ft)
            ),
            Cause::Destroyed => write!(f, "your aircraft was destroyed without an ejection"),
            Cause::Fuel { state } => write!(
                f,
                "fuel state {}",
                match state {
                    FuelState::Caution => "caution (joker)",
                    FuelState::Bingo => "bingo",
                    FuelState::Critical => "critical (fumes)",
                    FuelState::OutOfFuel => "out of fuel",
                    FuelState::Ok => "ok",
                    FuelState::NoManagement => "unmanaged",
                }
            ),
            Cause::MissileLaunch {
                missile,
                age,
                signature,
            } => write!(
                f,
                "{} missile {missile} launched at you {age:.1} s ago",
                match signature {
                    2 => "infrared",
                    3 => "radar",
                    _ => "other",
                }
            ),
            Cause::HardG { g, previous } => {
                write!(f, "entering hard G: {previous} G to {g} G")
            }
            Cause::Crossings { count } => {
                write!(f, "{count} crossings of -1 G this minute")
            }
            Cause::Coaching(c) => {
                write!(f, "situation {:?}", c.situation)?;
                if let Some(previous) = c.previous
                    && previous != c.situation
                {
                    write!(f, " (was {previous:?})")?;
                }
                if let Some(range) = c.range_ft {
                    write!(f, ", target {}", feet(range))?;
                }
                write!(f, ": {}; next check in {:.1} s", c.rule, c.next_s)
            }
            Cause::CrewGate { to: Some(gate), .. } => write!(f, "crew comments held: {gate}"),
            Cause::CrewGate { to: None, .. } => write!(f, "crew comments free"),
            Cause::Tower(event) => match event {
                TowerEvent::TakeoffClearance => write!(f, "on the runway, runway free"),
                TowerEvent::RunwayOccupied => write!(f, "on the runway, runway occupied"),
                TowerEvent::Airborne => write!(f, "you left the ground"),
                TowerEvent::Farewell => write!(f, "climbing out, 10 ft above the runway"),
                TowerEvent::LandingClearance => write!(f, "on approach, gear down, runway free"),
                TowerEvent::Wind { knots } => write!(f, "cleared to land, wind {knots} kt"),
                TowerEvent::LandingGrade { score } => {
                    write!(f, "you touched down, landing score {score}")
                }
                TowerEvent::Welcome => write!(f, "landed and stopped"),
            },
            Cause::WingStatus {
                go_around: true, ..
            } => write!(f, "the wingman went around"),
            Cause::WingStatus {
                phase: Some(phase), ..
            } => write!(f, "the wingman's airfield phase is now {phase:?}"),
            Cause::WingStatus { phase: None, .. } => write!(f, "the wingman left the airfield"),
            Cause::TowerRequest => write!(f, "your request to the tower"),
            Cause::Order {
                order,
                selected,
                target,
            } => {
                write!(f, "your order {order:?}")?;
                match (target, selected) {
                    (Some(target), _) => write!(f, " on {}", who(*target)),
                    (None, Some(selected)) => {
                        write!(f, " with {} designated", who(*selected))
                    }
                    (None, None) => Ok(()),
                }
            }
            Cause::Formation {
                phase,
                seconds,
                closure_fps,
            } => write!(
                f,
                "formation phase {phase:?} for {seconds:.0} s, closure {closure_fps:.0} ft/s"
            ),
            Cause::Activity { activity } => {
                write!(f, "activity changed to {}", activity.label())
            }
            Cause::Music(music) => {
                write!(f, "inputs ask for {:?}", music.to)?;
                if let Some(from) = music.from {
                    write!(f, " (was {from:?})")?;
                }
                let mut why = Vec::new();
                if music.inputs.succeeded {
                    why.push("the mission succeeded".to_string());
                }
                if music.inputs.ejected {
                    why.push("you ejected".into());
                }
                if music.inputs.launching {
                    why.push("taking off".into());
                }
                if let Some((id, range)) = music.designated {
                    why.push(format!(
                        "designated enemy {} {} {}",
                        who(id),
                        if range < crate::audio::situation::AIR_RANGE_FT {
                            "inside"
                        } else {
                            "beyond"
                        },
                        feet(crate::audio::situation::AIR_RANGE_FT)
                    ));
                }
                if let Some(at) = music.hit_at
                    && music.inputs.hit_recently
                {
                    why.push(format!("you were hit at {at:.1} s (30 s hold)"));
                }
                for id in &music.aiming {
                    why.push(format!(
                        "{} aims at you with a missile (4 s hold)",
                        who(*id)
                    ));
                }
                for id in &music.inbound {
                    why.push(format!("missile {id} guided at you"));
                }
                if music.inputs.home {
                    why.push("near home after success".into());
                }
                if music.inputs.deck {
                    why.push("on the deck".into());
                }
                if !why.is_empty() {
                    write!(f, ": {}", why.join("; "))?;
                }
                Ok(())
            }
        }
    }
}

/// Why an outcome happened, with its numbers.
#[derive(Clone, Debug, PartialEq)]
pub enum Reason {
    /// Routine chatter is dropped when sent under radio silence.
    RadioSilence,
    /// The queue was full; the oldest waiting line made room.
    QueueFull { limit: usize },
    /// A shared cooldown, `remaining` seconds of `seconds` left.
    Cooldown {
        key: &'static str,
        seconds: f64,
        remaining: f64,
    },
    /// A limit per aircraft (`id`), `remaining` seconds of `seconds` left.
    PerAircraft {
        rule: &'static str,
        id: u32,
        seconds: f64,
        remaining: f64,
    },
    /// A release with no target that is not a bomb makes no call.
    NoTarget,
    /// A friendly-fire complaint reaches the player within `limit_ft`.
    TooFar { range_ft: f64, limit_ft: f64 },
    /// A chance roll did not come up (the roll is listed).
    Chance,
    /// The speaker has no radio identity, so nobody hears it.
    NoRadioIdentity,
    /// Addressed to a flight the player is not in.
    OtherFlight,
    /// Addressed to an enemy flight.
    EnemyFlight,
    /// The player is down, so the player's flight is not listening.
    PlayerDown,
    /// Only the first two aircraft of a flight report contacts.
    OnlyFirstTwo { member: u8 },
    /// A contact is never reported twice in a row.
    SameTarget { target: u32 },
    /// The target is not a living airborne aircraft.
    NoContact { target: u32 },
    /// 15 s between one aircraft's contact reports.
    ContactCooldown { remaining: f64 },
    /// Accepting an attack order blocks contact reports for 20 s.
    EngageBlock { remaining: f64 },
    /// A missile warned while its class's 6 s limit ran: it is marked
    /// warned, so it is never called.
    WarnedDuringCooldown { key: &'static str, remaining: f64 },
    /// A newer notice from the same aircraft about the same thing.
    Coalesced,
    /// Older than the notice lifetime.
    OlderThan { seconds: f64, age: f64 },
    /// The player's aircraft was destroyed, or the pilot ejected or died.
    AircraftLost,
    /// The tower answered the player's own request instead.
    TowerReply,
    /// The runway became unusable.
    RunwayUnusable,
    /// The player left the approach.
    ApproachLeft,
    /// The wingman is down.
    WingmanDown,
    /// The wingman's airfield status changed again before it was said.
    StatusChanged,
    /// The aircraft entered a takeoff or landing sequence.
    AirfieldSequence,
    /// The aircraft is down or no longer flies formation.
    OutOfFormation,
    /// At most one line per `interval_s`; `remaining` seconds left.
    RateLimited { interval_s: f64, remaining: f64 },
    /// The line waiting for the HUD was replaced before the host took it.
    Unread,
    /// No living wingman is addressed.
    NoWingmen,
    /// Every addressed wingman bugged out and no longer answers.
    AllBuggedOut { count: usize },
    /// The order needs a living hostile target.
    NoHostileTarget,
    /// Land at selected airport needs an airport selected with Shift-A.
    NoAirportSelected,
    /// The player's wing order voice cancels wing speech still queued or
    /// playing.
    OrderVoice,
    /// A reason given by the host.
    Text(String),
}

impl fmt::Display for Reason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reason::RadioSilence => write!(f, "radio silence drops routine chatter"),
            Reason::QueueFull { limit } => write!(f, "queue full ({limit} waiting)"),
            Reason::Cooldown {
                key,
                seconds,
                remaining,
            } => write!(f, "cooldown {key} ({seconds:.0} s), {remaining:.1} s left"),
            Reason::PerAircraft {
                rule,
                id,
                seconds,
                remaining,
            } => write!(
                f,
                "{rule} for {} ({seconds:.0} s), {remaining:.1} s left",
                who(*id)
            ),
            Reason::NoTarget => write!(f, "no target and not a bomb"),
            Reason::TooFar { range_ft, limit_ft } => {
                write!(f, "{} away, beyond {}", feet(*range_ft), feet(*limit_ft))
            }
            Reason::Chance => write!(f, "the chance roll did not come up"),
            Reason::NoRadioIdentity => write!(f, "the speaker has no radio identity"),
            Reason::OtherFlight => write!(f, "speaker not in your flight"),
            Reason::EnemyFlight => write!(f, "enemy flight"),
            Reason::PlayerDown => write!(f, "you are down, so your flight is not listening"),
            Reason::OnlyFirstTwo { member } => write!(
                f,
                "only the first two aircraft of a flight report contacts (this is member {})",
                u32::from(*member) + 1
            ),
            Reason::SameTarget { target } => {
                write!(f, "{} was already reported", who(*target))
            }
            Reason::NoContact { target } => {
                write!(f, "{} is not a living airborne aircraft", who(*target))
            }
            Reason::ContactCooldown { remaining } => {
                write!(f, "contact cooldown (15 s), {remaining:.1} s left")
            }
            Reason::EngageBlock { remaining } => write!(
                f,
                "blocked after accepting an attack order (20 s), {remaining:.1} s left"
            ),
            Reason::WarnedDuringCooldown { key, remaining } => write!(
                f,
                "cooldown {key} (6 s), {remaining:.1} s left; marked as warned, never called"
            ),
            Reason::Coalesced => write!(f, "a newer notice about the same thing"),
            Reason::OlderThan { seconds, age } => {
                write!(f, "{age:.1} s old, over the {seconds:.0} s lifetime")
            }
            Reason::AircraftLost => write!(f, "your aircraft is lost"),
            Reason::TowerReply => write!(f, "the tower answered your request"),
            Reason::RunwayUnusable => write!(f, "the runway is unusable"),
            Reason::ApproachLeft => write!(f, "you left the approach"),
            Reason::WingmanDown => write!(f, "the wingman is down"),
            Reason::StatusChanged => write!(f, "the wingman's status changed again"),
            Reason::AirfieldSequence => write!(f, "in a takeoff or landing sequence"),
            Reason::OutOfFormation => write!(f, "down or out of formation"),
            Reason::RateLimited {
                interval_s,
                remaining,
            } => write!(f, "one line per {interval_s:.0} s, {remaining:.1} s left"),
            Reason::Unread => write!(f, "replaced before it was shown"),
            Reason::NoWingmen => write!(f, "no addressed wingmen"),
            Reason::AllBuggedOut { count } => {
                write!(
                    f,
                    "{count} addressed wingmen bugged out and no longer answer"
                )
            }
            Reason::NoHostileTarget => write!(f, "no valid hostile target"),
            Reason::NoAirportSelected => write!(f, "no airport selected (Shift-A)"),
            Reason::OrderVoice => write!(f, "cut off by your wing order voice"),
            Reason::Text(text) => f.write_str(text),
        }
    }
}

/// What happened to a line, an order or a report.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    /// Waiting: in the channel until `due`, or in the tower's own queue
    /// (from `due` at the earliest) until it `expires`.
    Queued { due: f64, expires: Option<f64> },
    /// Printed and played, after `waited` seconds in the channel. A text
    /// line from the AI: shown on the HUD.
    Delivered { waited: f64 },
    /// Thrown away when sent, or pushed out of a full queue.
    Dropped(Reason),
    /// A rule held it back before it was said.
    Suppressed(Reason),
    /// Said by the speaker, but the player's radio does not receive it.
    Unheard(Reason),
    /// A newer line took its place in a queue.
    Replaced(Reason),
    /// Too old to say.
    Expired(Reason),
    /// Taken off a queue before it was said.
    Cancelled(Reason),
    /// Delivered, then cut off while queued or playing in the mixer.
    Interrupted(Reason),
    /// An order's answers, one per addressed wingman, and the radio reply.
    Answered { answers: Vec<Answer>, reply: Reply },
    /// An order nobody received.
    Refused(Reason),
    /// A check that found nothing to say (the cause names the rule).
    Silent,
    /// A state change, such as the crew's gate or the music's inputs.
    Noted,
}

impl Outcome {
    /// The outcome's name. Shared names follow `tore_replay::vocab::outcome`;
    /// `unheard`, `interrupted`, `answered`, `silent` and `noted` are this
    /// journal's own.
    pub fn name(&self) -> &'static str {
        match self {
            Outcome::Queued { .. } => "queued",
            Outcome::Delivered { .. } => "delivered",
            Outcome::Dropped(_) => "dropped",
            Outcome::Suppressed(_) => "suppressed",
            Outcome::Unheard(_) => "unheard",
            Outcome::Replaced(_) => "replaced",
            Outcome::Expired(_) => "expired",
            Outcome::Cancelled(_) => "cancelled",
            Outcome::Interrupted(_) => "interrupted",
            Outcome::Answered { .. } => "answered",
            Outcome::Refused(_) => "rejected",
            Outcome::Silent => "silent",
            Outcome::Noted => "noted",
        }
    }

    /// The reason, when the outcome has one.
    pub fn reason(&self) -> Option<&Reason> {
        match self {
            Outcome::Dropped(r)
            | Outcome::Suppressed(r)
            | Outcome::Unheard(r)
            | Outcome::Replaced(r)
            | Outcome::Expired(r)
            | Outcome::Cancelled(r)
            | Outcome::Interrupted(r)
            | Outcome::Refused(r) => Some(r),
            _ => None,
        }
    }
}

impl fmt::Display for Outcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Outcome::Queued { due, expires } => {
                write!(f, "queued (due {due:.2} s")?;
                if let Some(expires) = expires {
                    write!(f, ", expires {expires:.2} s")?;
                }
                write!(f, ")")
            }
            Outcome::Delivered { waited } => {
                write!(f, "delivered after {waited:.2} s in the queue")
            }
            Outcome::Answered { answers, reply } => {
                write!(f, "answered: ")?;
                if answers.is_empty() {
                    write!(f, "nobody")?;
                }
                for (i, answer) in answers.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{answer}")?;
                }
                write!(f, "; {reply}")
            }
            Outcome::Silent => write!(f, "nothing said"),
            Outcome::Noted => write!(f, "noted"),
            other => write!(
                f,
                "{}: {}",
                other.name(),
                other.reason().map(ToString::to_string).unwrap_or_default()
            ),
        }
    }
}

/// One wingman's answer to a player order.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    pub recipient: u32,
    /// Member number in the wing; 1 is the first wingman.
    pub member: u8,
    pub result: Answered,
    /// Silent side orders delivered first, with their outcomes: the wing
    /// control change an order implies, and an approach's target.
    pub side: Vec<(&'static str, ReceiverOutcome)>,
}

/// How a wingman answered.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Answered {
    /// The receiver's own outcome.
    Receiver(ReceiverOutcome),
    /// Rejected before delivery: its sensors cannot see the target.
    CannotSeeTarget,
    /// Skipped: it bugged out and no longer answers orders.
    BuggedOut,
    /// Skipped: flown by a human.
    Human,
    /// Skipped: already landed and parked.
    AlreadyLanded,
    /// Skipped: bug out is ignored while taking off, landing or on the
    /// ground.
    OnAirfield,
    /// Skipped: no home base to return to.
    NoBase,
}

impl Answered {
    /// `applied`, `rejected` or `skipped`.
    pub fn name(&self) -> &'static str {
        match self {
            Answered::Receiver(ReceiverOutcome::Rejected(_)) | Answered::CannotSeeTarget => {
                "rejected"
            }
            Answered::Receiver(_) => "applied",
            _ => "skipped",
        }
    }
}

impl fmt::Display for Answer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wingman {} ({}) ", self.member, who(self.recipient))?;
        match self.result {
            Answered::Receiver(ReceiverOutcome::Rejected(why)) => write!(f, "rejected: {why:?}")?,
            Answered::Receiver(ReceiverOutcome::AppliedNoMotion) => {
                write!(f, "applied without motion")?
            }
            Answered::Receiver(ReceiverOutcome::MotionInstalled(_)) => {
                write!(f, "applied with motion")?
            }
            Answered::Receiver(ReceiverOutcome::Applied(_)) => write!(f, "applied")?,
            Answered::CannotSeeTarget => write!(f, "rejected: its sensors cannot see the target")?,
            Answered::BuggedOut => write!(f, "skipped: bugged out, no longer answers")?,
            Answered::Human => write!(f, "skipped: flown by a human")?,
            Answered::AlreadyLanded => write!(f, "skipped: already landed")?,
            Answered::OnAirfield => write!(f, "skipped: taking off, landing or on the ground")?,
            Answered::NoBase => write!(f, "skipped: no base")?,
        }
        for (what, outcome) in &self.side {
            write!(f, " [{what}: {outcome:?}]")?;
        }
        Ok(())
    }
}

/// Which wingman answered an order over the radio, or why nobody did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    Replied {
        by: u32,
        reply: WingReply,
    },
    /// This order has no radio reply.
    NotExpected,
    /// The first wingman was not addressed, or bugged out.
    FirstNotAddressed {
        first: Option<u32>,
    },
    /// The first wingman's answer carries no reply: it rejected the
    /// order, could not see the target, or applied it without motion.
    FirstDidNotApply {
        first: u32,
    },
}

impl fmt::Display for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Reply::Replied { by, reply } => write!(
                f,
                "{} replies {} two seconds later",
                who(*by),
                match reply {
                    WingReply::Engage { .. } => "with an engage call",
                    WingReply::Showtime => "\"Showtime!\"",
                }
            ),
            Reply::NotExpected => write!(f, "no radio reply for this order"),
            Reply::FirstNotAddressed { first: Some(first) } => write!(
                f,
                "no reply: the first wingman ({}) was not addressed or bugged out",
                who(*first)
            ),
            Reply::FirstNotAddressed { first: None } => {
                write!(f, "no reply: no first wingman")
            }
            Reply::FirstDidNotApply { first } => write!(
                f,
                "no reply: the first wingman ({}) rejected it, could not see the target, \
                 or took it without motion",
                who(*first)
            ),
        }
    }
}

/// Why the crew may not comment. Fuel calls and missile warnings are not
/// gated; the 3 s channel hold is not listed as a gate, since every
/// delivered line shows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Gate {
    /// The aircraft is destroyed, or the pilot ejected or died.
    Lost,
    /// Within the crash countdown that drives the eject warning.
    EjectDanger,
    RadioSilence,
    /// On the ground, or the gear is down.
    NotFreeFlight,
    /// No crew, and the first wingman cannot coach.
    NoSpeaker(NoSpeaker),
}

impl Gate {
    /// Whether two gates hold for the same reason, ignoring their numbers.
    pub fn same(&self, other: &Gate) -> bool {
        match (self, other) {
            (Gate::NoSpeaker(a), Gate::NoSpeaker(b)) => {
                std::mem::discriminant(a) == std::mem::discriminant(b)
            }
            _ => std::mem::discriminant(self) == std::mem::discriminant(other),
        }
    }
}

/// Why nobody can coach a single-seat player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NoSpeaker {
    /// No crew and no first wingman.
    NoWingman,
    WingmanDown,
    /// The wingman's target is not the player's.
    OtherTarget {
        his: Option<u32>,
        ours: Option<u32>,
    },
    /// The wingman is beyond 15,000 ft.
    TooFar {
        range_ft: f64,
    },
}

impl fmt::Display for Gate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Gate::Lost => write!(f, "aircraft lost, ejected or pilot dead"),
            Gate::EjectDanger => write!(f, "inside the eject warning"),
            Gate::RadioSilence => write!(f, "radio silence"),
            Gate::NotFreeFlight => write!(f, "not in free flight (on the ground or gear down)"),
            Gate::NoSpeaker(NoSpeaker::NoWingman) => write!(f, "no crew and no wingman"),
            Gate::NoSpeaker(NoSpeaker::WingmanDown) => write!(f, "no crew; the wingman is down"),
            Gate::NoSpeaker(NoSpeaker::OtherTarget { his, ours }) => write!(
                f,
                "no crew; the wingman's target ({}) is not yours ({})",
                his.map_or("none".into(), who),
                ours.map_or("none".into(), who)
            ),
            Gate::NoSpeaker(NoSpeaker::TooFar { range_ft }) => write!(
                f,
                "no crew; the wingman is {} away, beyond 15000 ft",
                feet(*range_ft)
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(at: f64) -> Entry {
        Entry::note(
            at,
            "Red two",
            Origin::of(Source::Radio, Cause::Unspecified),
            Outcome::Noted,
        )
    }

    #[test]
    fn the_journal_is_bounded_and_counts_what_it_lost() {
        let mut journal = Journal::default();
        for i in 0..CAPACITY + 5 {
            journal.push(entry(i as f64));
        }
        assert_eq!(journal.len(), CAPACITY);
        assert_eq!(journal.lost(), 5);
        let taken = journal.take();
        assert_eq!(taken[0].at, 5., "the oldest entries went first");
        assert!(journal.is_empty());
        assert_eq!(journal.lost(), 5, "taking keeps the count");
    }

    #[test]
    fn a_suppression_is_listed_once_per_window() {
        let mut journal = Journal::default();
        assert!(journal.first_in_window("gun", 0, 0., 4.));
        assert!(!journal.first_in_window("gun", 0, 3.9, 4.));
        assert!(journal.first_in_window("gun", 1, 3.9, 4.), "per aircraft");
        assert!(journal.first_in_window("gun", 0, 4., 8.), "a new window");
        assert!(!journal.first_in_window("gun", 0, 7., 11.));
    }

    #[test]
    fn rolls_and_reasons_read_as_plain_english() {
        assert_eq!(
            Roll::new("Fox call", 37, Test::Below(50)).to_string(),
            "roll 37 < 50: Fox call"
        );
        assert_eq!(
            Roll::new("splash", 12, Test::AtLeast(40)).to_string(),
            "roll 12 < 40: splash"
        );
        assert_eq!(
            Roll::pick("variant", 13, 8).to_string(),
            "roll 13 mod 8 = 5: variant"
        );
        assert_eq!(
            Roll::new("size", 55, Test::Bands(&[40, 70])).to_string(),
            "roll 55, band 2 of 3: size"
        );
        let outcome = Outcome::Suppressed(Reason::PerAircraft {
            rule: "unguided hits per shooter",
            id: 3,
            seconds: 8.,
            remaining: 2.5,
        });
        assert_eq!(
            outcome.to_string(),
            "suppressed: unguided hits per shooter for aircraft 3 (8 s), 2.5 s left"
        );
        let release = Cause::Release {
            target: Some(3),
            store: Store {
                flags: 1,
                seeker: 3,
                phoenix: false,
            },
        };
        assert_eq!(release.to_string(), "radar missile release at aircraft 3");
        assert!(
            Gate::NoSpeaker(NoSpeaker::TooFar { range_ft: 1. })
                .same(&Gate::NoSpeaker(NoSpeaker::TooFar { range_ft: 2. }))
        );
        assert!(!Gate::RadioSilence.same(&Gate::Lost));
    }
}
