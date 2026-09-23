//! The player's cockpit crew voice: dogfight coaching, range calls, feet wet
//! and dry, G strain, fuel calls, missile warnings and the death scream. The
//! behaviour is specified in docs/spec/cockpit-voice.md; the "Implementation in
//! TORE" section there lists what is fitted. Every line goes through
//! [`crate::comms`], which owns delivery, the channel hold and radio silence.
//!
//! [`CrewVoice::step`] is pure and deterministic: it reads one [`Input`]
//! snapshot per fixed 120 Hz tick and draws its random numbers from the
//! channel's roll. The host adapter at the end of this file builds the input
//! from the live flight, combat and AI state.
use crate::comms::{self, Call, Comms, Crew, Elevation, Kind, Phrase, Phrases};
use std::collections::BTreeSet;
use tore_sim::ai::route::FuelState;
use tore_sim::attitude::{Basis, Vector, dot};
use tore_sim::models::FlightModel;

/// Aircraft targets beyond 20 statute miles give no coaching (native).
const AIRCRAFT_RANGE_FT: f64 = 105_600.;
/// The single-seat wingman must be this close to the player (native).
const WINGMAN_RANGE_FT: f64 = 15_000.;
/// Targets beyond this add 2 seconds to the coaching wait (native).
const FAR_FT: f64 = 8_000.;
/// The original's feet to nautical miles divisor (native).
const FEET_PER_NM: f64 = 6_076.;
/// Seconds after launch at which a human-flown target is warned (B47, native).
const MISSILE_WARNING_AGE: f64 = 1.;
/// Seconds between the warning and the crew's call (native).
const MISSILE_CALL_DELAY: f64 = 0.5;
/// Global repeat limit of the infrared and radar warnings (native).
const MISSILE_REPEAT: f64 = 6.;

const CLOSE_SELF: &[&str] = &["^ATUS", "^YRNOSE"];
const CLOSE_WINGMAN: &[&str] = &["^ATYOU", "^OFFBEAM"];
const CLOSING: &[&str] = &["^GETGUY", "^CLOSING"];
const AFTER_PASS: &[&str] = &[
    "^YAHOO2", "^GOTNOW1", "^GOTNOW2", "^REELING", "^WORM", "^KNOCK",
];
const OFFENSIVE: &[&str] = &[
    "^YAHOO3", "^GOTNOW1", "^GOTNOW2", "^BURN1", "^FINISH", "^TKOUT",
];
const NO_TONE: &[&str] = &["^CNTTONE", "^LOCKHIM"];
const COMING_AROUND: &[&str] = &["^COMARND", "^ONTAIL", "^WESHAKE", "^INPOSI"];
const BREAK_SELF: &[&str] = &[
    "^BREAK1", "^BREAK2", "^ONOUR6", "^PLTSHT1", "^PLTSHT2", "^USOUT", "^NOTGOOD",
];
const BREAK_WINGMAN: &[&str] = &[
    "^BREAK1", "^BREAK2", "^BANDIT6", "^PLTSHT1", "^PLTSHT2", "^GETOUT", "^BANTAIL", "^MNVR",
    "^SHAKHM", "^EVASV",
];
const SKILLED: &[&str] = &["^DNTLIKE", "^GUYGOOD", "^NOROOK", "^SMMOVE", "^LKSKILL"];
const LOST_HIM: &[&str] = &["^BURN2", "^BRGARND", "^LOSTHIM", "^DO180"];
const FEET_WET: &[&str] = &["^FT_WETA", "^FT_WETB", "^FT_WETC"];
const FEET_DRY: &[&str] = &["^FT_DRYA", "^FT_DRYB", "^FT_DRYC"];
const STRAIN: &[&str] = &[
    "^GRUNT1", "^GRUNT2", "^GRUNT3", "^GRUNT4", "^BREATH2", "^BREATH3", "^BREATH4",
];
const SICK: &[&str] = &[
    "^BARF1", "^BARF2", "^BARF3", "^BARF4", "^BARF5", "^BARF6", "^BARF7",
];
/// "Aaargh..." 50%, "Oh, sh..." 25%, "Yaaaah" 25% (native).
const SCREAM: &[&str] = &["^AARRRGH", "^AARRRGH", "^OHSH", "^YAAAAAH"];

/// Where an aircraft or object is and which way it points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub position: Vector,
    pub basis: Basis,
    /// Scalar speed, feet per second.
    pub speed: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetKind {
    /// `flying` is false while the target is going down; `ace` is its skill.
    Aircraft {
        flying: bool,
        ace: bool,
    },
    Surface,
}

/// The player's designated enemy target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub id: u32,
    pub kind: TargetKind,
    pub body: Body,
}

/// The player's first wingman, the second aircraft of the player's flight.
#[derive(Clone, Debug, PartialEq)]
pub struct Wingman {
    /// His radio name, printed as the speaker.
    pub label: String,
    pub alive: bool,
    pub target: Option<u32>,
    pub position: Vector,
}

/// A missile fired at the player.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Incoming {
    pub id: u32,
    /// Seconds since launch.
    pub age: f64,
    /// Seeker signature class: 2 infrared, 3 radar.
    pub signature: u8,
}

/// One fixed tick of everything the crew can notice.
#[derive(Clone, Debug, PartialEq)]
pub struct Input {
    /// Simulation seconds.
    pub now: f64,
    /// Minute of the mission clock, for the G crossing count.
    pub clock_minute: i64,
    /// The aircraft is destroyed (crashed or shot down).
    pub crashed: bool,
    /// The pilot has ejected.
    pub ejected: bool,
    /// The pilot is dead.
    pub pilot_dead: bool,
    /// Airborne and past the takeoff and landing sequences.
    pub free_flight: bool,
    /// Within the crash countdown that drives the eject warning.
    pub doomed: bool,
    /// The flight model's G load.
    pub g: f64,
    pub own: Body,
    /// The designated target id, whatever its side, for the wingman's
    /// same-target test.
    pub designated: Option<u32>,
    /// The designated target when it is an enemy.
    pub target: Option<Target>,
    pub gun_selected: bool,
    /// A loaded missile's seeker would see the target now.
    pub missile_would_lock: bool,
    /// Corner speed, feet per second.
    pub corner_speed: f64,
    /// Whether the aircraft is over water; `None` when unknown.
    pub over_water: Option<bool>,
    pub fuel: FuelState,
    pub incoming: Vec<Incoming>,
    pub wingman: Option<Wingman>,
}

/// The four dogfight situations and the non-dogfight cases.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Situation {
    /// No usable target: feet wet and feet dry only.
    Clear,
    /// A target that gives no lines: one going down, or an aircraft when the
    /// player is not fighter class.
    Silent,
    Surface {
        ahead: bool,
    },
    HeadOn,
    Offensive,
    Defensive,
    Neutral,
}

/// Target-relative angles and range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Geometry {
    pub range: f64,
    /// Angle between our nose and the target, degrees.
    pub off_nose: f64,
    /// Our azimuth off his nose, signed, degrees.
    pub his_azimuth: f64,
    /// Our elevation off his nose, degrees.
    pub his_elevation: f64,
    /// His bearing clockwise from our heading, degrees.
    pub bearing: f64,
    /// His heading relative to ours, clockwise, degrees.
    pub heading_delta: f64,
    /// His nose pitch, degrees.
    pub his_pitch: f64,
}

fn wrap(degrees: f64) -> f64 {
    (degrees + 180.).rem_euclid(360.) - 180.
}
fn length(v: Vector) -> f64 {
    dot(v, v).sqrt()
}
fn azimuth(v: Vector) -> f64 {
    v[0].atan2(v[2]).to_degrees()
}
fn elevation(v: Vector) -> f64 {
    v[1].atan2(v[0].hypot(v[2])).to_degrees()
}

pub fn geometry(own: &Body, target: &Body) -> Geometry {
    let to_him: Vector = std::array::from_fn(|i| target.position[i] - own.position[i]);
    let to_us = to_him.map(|v| -v);
    let range = length(to_him);
    let off_nose = (dot(to_him, own.basis.forward) / range.max(1e-9))
        .clamp(-1., 1.)
        .acos()
        .to_degrees();
    let [his_heading, his_pitch, _] = target.basis.angles().map(f64::to_degrees);
    let [our_heading, _, _] = own.basis.angles().map(f64::to_degrees);
    Geometry {
        range,
        off_nose,
        his_azimuth: wrap(azimuth(to_us) - his_heading),
        his_elevation: elevation(to_us) - his_pitch,
        bearing: wrap(azimuth(to_him) - our_heading),
        heading_delta: wrap(his_heading - our_heading),
        his_pitch,
    }
}

/// Head-on, offensive, defensive or neutral by the spec's two 90 degree
/// cones; the other cases by target kind and range.
pub fn classify(target: Option<&Target>, geometry: Option<&Geometry>, fighter: bool) -> Situation {
    let (Some(target), Some(g)) = (target, geometry) else {
        return Situation::Clear;
    };
    let ahead = g.off_nose <= 90.;
    match target.kind {
        TargetKind::Surface => Situation::Surface { ahead },
        TargetKind::Aircraft { .. } if g.range >= AIRCRAFT_RANGE_FT => Situation::Clear,
        TargetKind::Aircraft { flying: false, .. } => Situation::Silent,
        TargetKind::Aircraft { .. } if !fighter => Situation::Silent,
        TargetKind::Aircraft { .. } => match (ahead, g.his_azimuth.abs() <= 90.) {
            (true, true) => Situation::HeadOn,
            (true, false) => Situation::Offensive,
            (false, true) => Situation::Defensive,
            (false, false) => Situation::Neutral,
        },
    }
}

/// How a position call ends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Tail {
    Plain,
    Left,
    Right,
    Away,
}

/// "He's at [your] two o'clock[, turning left | , heading away]".
fn position_call(phrases: &Phrases, you: bool, g: &Geometry, away: bool) -> Phrase {
    let tail = if away {
        Tail::Away
    } else if (20.0..=160.0).contains(&g.heading_delta.abs()) {
        // Fitted: the original carries an angle whose meaning is unknown.
        // TORE uses his heading relative to ours.
        if g.heading_delta > 0. {
            Tail::Right
        } else {
            Tail::Left
        }
    } else {
        Tail::Plain
    };
    let hour = comms::clock_hour(g.bearing);
    let start = Phrase::stem(phrases, if you { "^HESYOUR" } else { "^HESAT" });
    let call = start.join(comms::clock(
        phrases,
        false,
        hour,
        Elevation::Level,
        tail == Tail::Plain,
    ));
    match tail {
        Tail::Plain => call,
        Tail::Left => call.raw(", turning ", None).then(phrases, "^TURNLFT"),
        Tail::Right => call.raw(", turning ", None).then(phrases, "^TURNRGT"),
        Tail::Away => call.then(phrases, "^HEADAWY"),
    }
}

/// "12 miles, three o'clock, in range".
fn range_call(phrases: &Phrases, miles: u32, bearing: f64) -> Phrase {
    let hour = comms::clock_hour(bearing);
    let in_range = miles <= 2;
    let mut call = comms::miles(phrases, miles);
    if hour != 12 {
        call = call.raw(", ", None).join(comms::clock(
            phrases,
            false,
            hour,
            Elevation::Level,
            !in_range,
        ));
    }
    if in_range {
        call = call.then(phrases, "^INRANGE");
    }
    call
}

/// Who says the comment lines this tick.
struct Speaker {
    label: String,
    /// The single-seat wingman, speaking the "you" wording.
    wingman: bool,
}

/// Per-flight crew voice memory.
#[derive(Clone, Debug, PartialEq)]
pub struct CrewVoice {
    crew: Option<Crew>,
    fighter: bool,
    /// Earliest time of the next coaching remark.
    next: f64,
    last_situation: Option<Situation>,
    last_nm: Option<u32>,
    last_g: Option<i32>,
    crossings: u32,
    crossing_minute: i64,
    wet: Option<bool>,
    /// 0 none, 1 joker, 2 bingo, 3 fumes, 4 out of gas.
    fuel_said: u8,
    warned: BTreeSet<u32>,
    crashed: bool,
    home: Option<Vector>,
}

impl CrewVoice {
    /// A fresh flight in `aircraft`: its multi-crew flag and fighter class.
    pub fn new(aircraft: &tore_formats::aircraft::Aircraft) -> Self {
        let class = aircraft
            .object
            .get("obj_class")
            .and_then(|t| t.number().ok())
            .unwrap_or(0);
        Self::with(comms::crew(aircraft), class & 0x4000 == 0)
    }
    fn with(crew: Option<Crew>, fighter: bool) -> Self {
        Self {
            crew,
            fighter,
            next: f64::NEG_INFINITY,
            last_situation: None,
            last_nm: None,
            last_g: None,
            crossings: 0,
            crossing_minute: i64::MIN,
            wet: None,
            fuel_said: 0,
            warned: BTreeSet::new(),
            crashed: false,
            home: None,
        }
    }
    /// The home point for fuel calls: the first position seen this flight.
    pub fn home(&mut self, position: Vector) -> Vector {
        *self.home.get_or_insert(position)
    }

    /// One fixed tick. Sends any lines to `comms`.
    pub fn step(&mut self, input: &Input, comms: &mut Comms, phrases: &Phrases) {
        let now = input.now;
        if input.crashed && !self.crashed && !input.ejected {
            let stem = Comms::pick(comms.roll(), SCREAM);
            comms.send(
                now,
                Call::new("", Phrase::stem(phrases, stem), Kind::Important).direct(),
            );
        }
        self.crashed = input.crashed;
        let g = input.g.floor() as i32;
        let previous_g = self.last_g.replace(g);
        if input.crashed || input.ejected || input.pilot_dead {
            return;
        }
        if let Some(crew) = self.crew {
            self.fuel_call(crew, input.fuel, comms, phrases, now);
            self.missile_warnings(crew, &input.incoming, comms, phrases, now);
        }
        if !comms.channel_free(now) || input.doomed {
            return;
        }
        if comms.radio_silence {
            // The feet wet state keeps tracking so a later call is never stale.
            if input.over_water.is_some() {
                self.wet = input.over_water;
            }
            return;
        }
        if !input.free_flight {
            return;
        }
        let Some(speaker) = self.speaker(input) else {
            return;
        };
        if !speaker.wingman
            && let Some((stems, wait)) = self.g_sound(previous_g, g, input.clock_minute, comms)
        {
            let stem = Comms::pick(comms.roll(), stems);
            comms.send(
                now,
                Call::new(speaker.label, Phrase::stem(phrases, stem), Kind::Chatter),
            );
            self.next = self.next.max(now) + wait;
            return;
        }
        let geometry = input.target.as_ref().map(|t| geometry(&input.own, &t.body));
        let situation = classify(input.target.as_ref(), geometry.as_ref(), self.fighter);
        if self.last_situation != Some(situation) {
            self.next = f64::NEG_INFINITY;
        }
        if self.next > now {
            return;
        }
        let far = geometry.is_some_and(|g| g.range > FAR_FT);
        self.next = now + 4. + f64::from(comms.roll() % 4) + if far { 2. } else { 0. };
        let previous = self.last_situation.replace(situation);
        let nm = geometry.map(|g| (g.range / FEET_PER_NM) as u32);
        let previous_nm = std::mem::replace(&mut self.last_nm, nm);
        let line = match (situation, geometry) {
            (Situation::Clear, _) => self.feet(input.over_water, comms),
            (Situation::Silent, _) | (Situation::Surface { ahead: false }, _) => None,
            (Situation::Surface { ahead: true }, Some(g)) => {
                let nm = nm.unwrap_or(0);
                if previous_nm.is_some_and(|p| p > 10) && nm <= 10 {
                    Some((Line::Stems(&["^APPTRGT"]), 0.))
                } else if previous_nm != Some(nm) && nm >= 1 {
                    let rounded = (g.range / FEET_PER_NM).round() as u32;
                    Some((Line::Phrase(range_call(phrases, rounded, g.bearing)), 0.))
                } else {
                    None
                }
            }
            (situation, Some(g)) => {
                self.dogfight(situation, previous, &g, input, &speaker, comms, phrases)
            }
            (_, None) => None,
        };
        let Some((phrase, wait)) = line else {
            return;
        };
        match phrase {
            Line::Stems(stems) => {
                let stem = Comms::pick(comms.roll(), stems);
                self.next += wait;
                comms.send(
                    now,
                    Call::new(speaker.label, Phrase::stem(phrases, stem), Kind::Chatter),
                );
            }
            Line::Phrase(phrase) => {
                self.next += wait;
                comms.send(now, Call::new(speaker.label, phrase, Kind::Chatter));
            }
        }
    }

    /// The crew of a two-seater, else the first wingman when he qualifies.
    fn speaker(&self, input: &Input) -> Option<Speaker> {
        if let Some(crew) = self.crew {
            return Some(Speaker {
                label: crew.label().into(),
                wingman: false,
            });
        }
        let w = input.wingman.as_ref()?;
        let offset: Vector = std::array::from_fn(|i| w.position[i] - input.own.position[i]);
        (w.alive && w.target == input.designated && length(offset) <= WINGMAN_RANGE_FT).then(|| {
            Speaker {
                label: w.label.clone(),
                wingman: true,
            }
        })
    }

    /// Strain on a hard pull or push, and the -1 G crossing count.
    fn g_sound(
        &mut self,
        previous: Option<i32>,
        g: i32,
        minute: i64,
        comms: &mut Comms,
    ) -> Option<(&'static [&'static str], f64)> {
        let previous = previous?;
        if (previous >= -1) != (g >= -1) {
            if minute != self.crossing_minute {
                self.crossing_minute = minute;
                self.crossings = 0;
            }
            self.crossings += 1;
            if self.crossings == 18 {
                return Some((&["^EASEUP"], 5.));
            }
            if self.crossings >= 20 {
                self.crossings = 0;
                return Some((SICK, 15.));
            }
            return None;
        }
        let hard = |g: i32| !(-3 < g && g < 5);
        (hard(g) && !hard(previous) && comms.roll() < 30).then_some((STRAIN, 3.))
    }

    fn feet(&mut self, over_water: Option<bool>, comms: &mut Comms) -> Option<(Line, f64)> {
        let changed = match (self.wet, over_water) {
            (Some(was), Some(now)) if was != now => Some(now),
            _ => None,
        };
        if over_water.is_some() {
            self.wet = over_water;
        }
        match changed {
            Some(wet) => Some((Line::Stems(if wet { FEET_WET } else { FEET_DRY }), 10.)),
            None => {
                self.next += 5. + f64::from(comms.roll() % 5);
                None
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn dogfight(
        &self,
        situation: Situation,
        previous: Option<Situation>,
        g: &Geometry,
        input: &Input,
        speaker: &Speaker,
        comms: &mut Comms,
        phrases: &Phrases,
    ) -> Option<(Line, f64)> {
        let you = speaker.wingman;
        let own = !you;
        let position = |away| Line::Phrase(position_call(phrases, you, g, away));
        let (flying_ace, target_pitch) = match input.target.map(|t| t.kind) {
            Some(TargetKind::Aircraft { ace, .. }) => (ace, g.his_pitch),
            _ => (false, 0.),
        };
        match situation {
            Situation::HeadOn => {
                if g.range > 10_000. && comms.roll() < 50 {
                    Some((position(false), 2.))
                } else if g.range < 5_000.
                    && g.his_azimuth.abs() <= 10.
                    && g.his_elevation.abs() <= 10.
                {
                    Some((
                        Line::Stems(if you { CLOSE_WINGMAN } else { CLOSE_SELF }),
                        0.,
                    ))
                } else if g.range < 20_000. {
                    Some((Line::Stems(CLOSING), 0.))
                } else {
                    None
                }
            }
            Situation::Offensive => {
                let closure = input.own.speed - input.target.map_or(0., |t| t.body.speed);
                if previous == Some(Situation::HeadOn) {
                    Some((Line::Stems(AFTER_PASS), 4.))
                } else if own
                    && input.gun_selected
                    && g.range >= 5_000.
                    && input.missile_would_lock
                    && comms.roll() < 25
                {
                    Some((Line::Stems(&["^SWCMISS"]), 4.))
                } else if g.range < 1_200. && closure >= 146. {
                    Some((Line::Stems(&["^DONTOVR"]), 4.))
                } else if you && comms.roll() < 25 {
                    Some((Line::Stems(NO_TONE), 4.))
                } else if g.range >= 10_000. && comms.roll() < 50 {
                    Some((position(false), 2.))
                } else {
                    Some((Line::Stems(OFFENSIVE), 3. + 4.))
                }
            }
            Situation::Defensive => {
                if previous == Some(Situation::Neutral) {
                    Some((Line::Stems(COMING_AROUND), 0.))
                } else if g.off_nose > 160. && g.his_azimuth.abs() <= 30. && input.g.abs() <= 3. {
                    Some((
                        Line::Stems(if you { BREAK_WINGMAN } else { BREAK_SELF }),
                        0.,
                    ))
                } else if own && input.own.speed > input.corner_speed + 110. && comms.roll() < 25 {
                    Some((Line::Stems(&["^SLOWDWN"]), 0.))
                } else if flying_ace && comms.roll() < 25 {
                    Some((Line::Stems(SKILLED), 0.))
                } else {
                    Some((position(false), 0.))
                }
            }
            Situation::Neutral => {
                if previous == Some(Situation::HeadOn) {
                    Some((Line::Stems(LOST_HIM), 3.))
                } else if (60.0..=120.0).contains(&target_pitch.abs()) && comms.roll() < 25 {
                    Some((Line::Stems(&["^VERTICL"]), 0.))
                } else {
                    Some((position(true), 0.))
                }
            }
            _ => None,
        }
    }

    /// Each fuel state once per flight; a worse state marks the milder ones.
    fn fuel_call(
        &mut self,
        crew: Crew,
        fuel: FuelState,
        comms: &mut Comms,
        phrases: &Phrases,
        now: f64,
    ) {
        let (level, stem) = match fuel {
            FuelState::Caution => (1, "^JOKER"),
            FuelState::Bingo => (2, "^BINGO"),
            FuelState::Critical => (3, "^WEFUMES"),
            FuelState::OutOfFuel => (4, "^OUTGAS"),
            FuelState::Ok | FuelState::NoManagement => return,
        };
        if level > self.fuel_said {
            self.fuel_said = level;
            comms.send(
                now,
                Call::new(crew.label(), Phrase::stem(phrases, stem), Kind::Important),
            );
        }
    }

    /// Once per missile, one second after launch, by seeker class.
    fn missile_warnings(
        &mut self,
        crew: Crew,
        incoming: &[Incoming],
        comms: &mut Comms,
        phrases: &Phrases,
        now: f64,
    ) {
        self.warned
            .retain(|id| incoming.iter().any(|missile| missile.id == *id));
        for missile in incoming {
            if missile.age < MISSILE_WARNING_AGE || !self.warned.insert(missile.id) {
                continue;
            }
            let stem = match missile.signature {
                2 => comms
                    .cooldown("crew-infrared-warning", now, MISSILE_REPEAT)
                    .then_some("^ATOLFLR"),
                3 => comms
                    .cooldown("crew-radar-warning", now, MISSILE_REPEAT)
                    .then_some("^APEXCHF"),
                _ => Some("^MISSBRK"),
            };
            if let Some(stem) = stem {
                comms.send(
                    now,
                    Call::new(crew.label(), Phrase::stem(phrases, stem), Kind::Important)
                        .after(MISSILE_CALL_DELAY),
                );
            }
        }
    }
}

/// A chosen line: a variant set, or a composed call.
enum Line {
    Stems(&'static [&'static str]),
    Phrase(Phrase),
}

/// The loaded envelope's speed band and corner speed at the aircraft's
/// altitude. Fitted, the same host rule as the AI's `speed_limits`: corner is
/// the slowest speed of the highest-G envelope.
fn speed_limits(flight: &crate::flight::State) -> tore_sim::ai::SpeedLimits {
    use tore_sim::ai::{ScalarSpeed, SpeedLimits, mission};
    let envelopes = &flight.model().configuration().aerodynamics.envelopes;
    let (mut minimum, mut maximum, mut corner, mut best) =
        (f64::INFINITY, f64::NEG_INFINITY, f64::NAN, i32::MIN);
    for envelope in envelopes {
        let Some((low, high)) = envelope.speeds(flight.position[1]) else {
            continue;
        };
        minimum = minimum.min(low);
        maximum = maximum.max(high);
        if envelope.g > best {
            best = envelope.g;
            corner = low;
        }
    }
    if !minimum.is_finite() || !maximum.is_finite() || maximum <= minimum {
        return SpeedLimits {
            minimum: ScalarSpeed(mission::FALLBACK_MINIMUM_FPS),
            maximum: ScalarSpeed(mission::FALLBACK_MAXIMUM_FPS),
            corner: ScalarSpeed(mission::FALLBACK_CORNER_FPS),
        };
    }
    SpeedLimits {
        minimum: ScalarSpeed(minimum),
        maximum: ScalarSpeed(maximum),
        corner: ScalarSpeed(if corner.is_finite() { corner } else { minimum }.min(maximum)),
    }
}

/// B48 fuel state for the player. Fitted, the AI host's rule: endurance is
/// all remaining fuel at the military flow scaled by the current throttle
/// (floored at 10%), time home is the straight distance to `home` at cruise.
fn fuel_state(flight: &crate::flight::State, home: Vector) -> FuelState {
    let propulsion = &flight.model().configuration().propulsion;
    let flow = (propulsion.military_fuel_lbs_per_second * flight.throttle.max(0.1))
        .max(tore_sim::ai::mission::MINIMUM_FUEL_FLOW_LBS_PER_S);
    let endurance = (flight.fuel + flight.systems.external_lbs()) / flow;
    let cruise = tore_sim::ai::route::cruise_speed(&speed_limits(flight)).0;
    let distance = (home[0] - flight.position[0]).hypot(home[2] - flight.position[2]);
    let time_home = (cruise > 0.).then(|| distance / cruise);
    tore_sim::ai::route::fuel_state(endurance, time_home).unwrap_or(FuelState::NoManagement)
}

/// Whether a loaded air-to-air missile's own seeker would see `target` from
/// the player's aircraft now. Fitted stand-in for the original's seeker
/// evaluation: TORE's seeker model with no terrain masking.
fn missile_would_lock(
    state: &tore_sim::combat::live::State,
    launcher: tore_sim::combat::live::Launcher,
    target: &tore_sim::combat::live::Target,
) -> bool {
    use tore_sim::combat::missiles::{Profile, seeker};
    let view = seeker::View {
        position: launcher.position,
        basis: launcher.basis,
        cap: None,
        obscured: &|_, _| false,
    };
    state
        .configuration()
        .stations
        .iter()
        .enumerate()
        .any(|(i, station)| {
            let w = &station.weapon;
            state.rounds(i) > 0
                && matches!(w.seeker.signature, 2 | 3)
                && !tore_sim::combat::live::is_gun(w)
                && Profile::for_weapon(w).is_some_and(|profile| {
                    profile.guidance_available(launcher.radar_power)
                        && seeker::observe(w, profile, &view, target).is_some()
                })
        })
}

/// The live state the crew voice reads, borrowed field by field from the app.
pub struct Host<'a> {
    pub flight: &'a crate::flight::State,
    pub combat: &'a tore_sim::combat::live::State,
    pub wings: Option<&'a crate::ai_wings::AiWings>,
    pub world: &'a crate::terrain::World,
}

impl CrewVoice {
    /// One fixed tick from the live state, before due radio lines are delivered.
    pub fn step_host(&mut self, comms: &mut Comms, phrases: &Phrases, host: &Host<'_>) {
        let input = self.input(host);
        self.step(&input, comms, phrases);
    }

    fn input(&mut self, host: &Host<'_>) -> Input {
        use tore_sim::combat::live::is_gun;
        use tore_sim::combat::missiles::TargetRole;
        let flight = host.flight;
        let state = host.combat;
        let launcher = crate::combat::launcher(flight);
        let home = self.home(flight.position);
        let wings = host.wings;
        let ace = |id: u32| {
            wings
                .and_then(|w| w.mission().actor(id))
                .is_some_and(|a| a.experience().level == tore_sim::ai::Experience::Ace)
        };
        let designated = state.designated();
        let target = designated
            .filter(|id| !state.friendlies.contains(id))
            .and_then(|id| state.targets.iter().find(|t| t.id == id))
            .filter(|t| t.role == TargetRole::Aircraft || t.hp > 0);
        let gun_selected = state.armed
            && state
                .configuration()
                .stations
                .get(state.selected)
                .is_some_and(|s| is_gun(&s.weapon));
        let missile_would_lock = gun_selected
            && target.is_some_and(|t| {
                t.role == TargetRole::Aircraft && missile_would_lock(state, launcher, t)
            });
        let target = target.map(|t| Target {
            id: t.id,
            kind: match t.role {
                TargetRole::Aircraft => TargetKind::Aircraft {
                    flying: t.airborne && t.hp > 0 && t.wreck.is_none(),
                    ace: ace(t.id),
                },
                TargetRole::Surface => TargetKind::Surface,
            },
            body: Body {
                position: t.position,
                basis: t.basis,
                speed: length(t.velocity),
            },
        });
        let incoming = state
            .projectiles
            .iter()
            .filter(|p| p.incoming)
            .filter_map(|p| {
                let w = p.weapon(state.configuration());
                (w.seeker.signature != 0 && !is_gun(w)).then_some(Incoming {
                    id: p.id,
                    age: p.age as f64 / 120.,
                    signature: w.seeker.signature,
                })
            })
            .collect();
        let wingman = wings.and_then(|w| {
            let actor = w.mission().actors().iter().find(|a| {
                let identity = a.identity();
                identity.side == crate::ai_wings::FRIENDLY_SIDE
                    && identity.wing == 0
                    && identity.member == 1
            })?;
            Some(Wingman {
                label: w
                    .slot(actor.id())
                    .map_or_else(|| "Wingman".into(), |s| s.label()),
                alive: actor.alive(),
                target: actor.controller().target(),
                position: actor.flight().position,
            })
        });
        let on_ground = flight.research.as_ref().is_some_and(|r| r.on_ground);
        let world = host.world;
        Input {
            now: state.tick() as f64 / 120.,
            clock_minute: i64::from(world.weather.seconds_of_day()) / 60,
            crashed: flight.crashed,
            ejected: flight.escape.is_some(),
            pilot_dead: flight.systems.pilot.dead,
            free_flight: !on_ground && !flight.gear_down,
            doomed: tore_sim::ejection::assess(flight, |x, z| {
                f64::from(world.height(x as f32, z as f32))
            })
            .is_some(),
            g: flight.g,
            own: Body {
                position: flight.position,
                basis: launcher.basis,
                speed: flight.speed,
            },
            designated,
            target,
            gun_selected,
            missile_would_lock,
            corner_speed: speed_limits(flight).corner.0,
            over_water: Some(world.over_water(flight.position[0], flight.position[2])),
            fuel: fuel_state(flight, home),
            incoming,
            wingman,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic phrase text written for the tests.
    fn phrases() -> Phrases {
        [
            ("^HESAT", "He's at "),
            ("^HESYOUR", "He's at your "),
            ("^TURNLFT", "left"),
            ("^TURNRGT", "right"),
            ("^HEADAWY", ", heading away"),
            ("^MILE", " mile"),
            ("^MILES", " miles"),
            ("^INRANGE", ", in range"),
            ("^EASEUP", "Ease up"),
            ("^JOKER", "Joker"),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
    }

    fn body(position: Vector, heading_deg: f64) -> Body {
        Body {
            position,
            basis: Basis::new(heading_deg.to_radians(), 0., 0.),
            speed: 600.,
        }
    }

    /// Level at 10,000 ft heading north, nothing happening.
    fn input() -> Input {
        Input {
            now: 0.,
            clock_minute: 0,
            crashed: false,
            ejected: false,
            pilot_dead: false,
            free_flight: true,
            doomed: false,
            g: 1.,
            own: body([0., 10_000., 0.], 0.),
            designated: None,
            target: None,
            gun_selected: false,
            missile_would_lock: false,
            corner_speed: 700.,
            over_water: None,
            fuel: FuelState::Ok,
            incoming: Vec::new(),
            wingman: None,
        }
    }

    /// An enemy aircraft `north` feet ahead (negative behind), heading as given.
    fn with_aircraft(mut input: Input, north: f64, heading_deg: f64) -> Input {
        input.designated = Some(7);
        input.target = Some(Target {
            id: 7,
            kind: TargetKind::Aircraft {
                flying: true,
                ace: false,
            },
            body: body([0., 10_000., north], heading_deg),
        });
        input
    }

    fn situation(input: &Input, fighter: bool) -> Situation {
        let g = input.target.map(|t| geometry(&input.own, &t.body));
        classify(input.target.as_ref(), g.as_ref(), fighter)
    }

    /// Step once and deliver, returning what was heard.
    fn run(voice: &mut CrewVoice, comms: &mut Comms, input: &Input) -> Vec<Call> {
        voice.step(input, comms, &phrases());
        comms.due(input.now)
    }

    fn rio() -> CrewVoice {
        CrewVoice::with(Some(Crew::Rio), true)
    }

    #[test]
    fn geometry_classifies_the_four_situations_and_the_range_limit() {
        let i = input();
        assert_eq!(
            situation(&with_aircraft(i.clone(), 10_000., 180.), true),
            Situation::HeadOn
        );
        assert_eq!(
            situation(&with_aircraft(i.clone(), 10_000., 0.), true),
            Situation::Offensive
        );
        assert_eq!(
            situation(&with_aircraft(i.clone(), -10_000., 0.), true),
            Situation::Defensive
        );
        assert_eq!(
            situation(&with_aircraft(i.clone(), -10_000., 180.), true),
            Situation::Neutral
        );
        // Just inside and at 20 statute miles.
        assert_eq!(
            situation(&with_aircraft(i.clone(), 105_599., 180.), true),
            Situation::HeadOn
        );
        assert_eq!(
            situation(&with_aircraft(i.clone(), 105_600., 180.), true),
            Situation::Clear
        );
        // A non-fighter gets no dogfight, a falling target gives nothing.
        assert_eq!(
            situation(&with_aircraft(i.clone(), 10_000., 180.), false),
            Situation::Silent
        );
        let mut falling = with_aircraft(i.clone(), 10_000., 180.);
        falling.target.as_mut().unwrap().kind = TargetKind::Aircraft {
            flying: false,
            ace: false,
        };
        assert_eq!(situation(&falling, true), Situation::Silent);
        let mut ground = with_aircraft(i.clone(), -500_000., 0.);
        ground.target.as_mut().unwrap().kind = TargetKind::Surface;
        assert_eq!(
            situation(&ground, true),
            Situation::Surface { ahead: false }
        );
        assert_eq!(situation(&i, true), Situation::Clear);
        // His bearing and heading relative to ours.
        let g = geometry(&body([0.; 3], 90.), &body([0., 0., 1000.], 0.));
        assert!(
            (g.bearing + 90.).abs() < 1e-9,
            "north of an east-flying jet is 9 o'clock"
        );
        assert_eq!(comms::clock_hour(g.bearing), 9);
        assert!((g.heading_delta + 90.).abs() < 1e-9);
    }

    #[test]
    fn coaching_waits_four_to_seven_seconds_plus_two_when_far() {
        for (distance, extra) in [(-5_000., 0.), (-9_000., 2.)] {
            let mut voice = rio();
            let mut comms = Comms::new(11);
            // He is on our tail, pointing at us: "Break!" every time.
            let mut i = with_aircraft(input(), distance, 0.);
            let mut times = Vec::new();
            for tick in 0..120 * 120 {
                i.now = f64::from(tick) / 120.;
                if !run(&mut voice, &mut comms, &i).is_empty() {
                    times.push(i.now);
                }
            }
            assert!(times.len() > 10);
            assert_eq!(times[0], 0., "a new situation speaks at once");
            for pair in times.windows(2) {
                let gap = pair[1] - pair[0];
                assert!(
                    (4. + extra - 1e-9..=7. + extra + 1e-9).contains(&gap),
                    "gap {gap} at {distance} ft"
                );
            }
            let distinct: BTreeSet<_> = times
                .windows(2)
                .map(|p| ((p[1] - p[0]) * 10.).round() as i64)
                .collect();
            assert_eq!(distinct.len(), 4, "all four waits occur");
        }
    }

    #[test]
    fn a_situation_change_speaks_once_the_channel_is_free() {
        let mut voice = rio();
        let mut comms = Comms::new(3);
        let mut i = with_aircraft(input(), -5_000., 0.);
        let first = run(&mut voice, &mut comms, &i);
        assert_eq!(first[0].label, "RIO");
        assert!(BREAK_SELF.contains(&first[0].stems[0].as_str()));
        // He overshoots into neutral 2 s later: the channel is still held.
        i = with_aircraft(input(), -5_000., 180.);
        i.now = 2.;
        assert!(run(&mut voice, &mut comms, &i).is_empty());
        i.now = 3.;
        let heard = run(&mut voice, &mut comms, &i);
        assert_eq!(heard.len(), 1, "the wait is cancelled by the change");
        assert_eq!(heard[0].text, "He's at six o'clock, heading away");
        assert_eq!(heard[0].stems, ["^HESAT", "^CLOCK06", "^HEADAWY"]);
        // Back on our tail after a neutral moment: "He's coming around".
        i = with_aircraft(input(), -5_000., 0.);
        i.now = 6.;
        let heard = run(&mut voice, &mut comms, &i);
        assert!(COMING_AROUND.contains(&heard[0].stems[0].as_str()));
    }

    #[test]
    fn position_and_range_calls_compose_text_and_recordings() {
        let p = phrases();
        let g = geometry(&body([0.; 3], 0.), &body([1_732., 0., 1_000.], 270.));
        let call = position_call(&p, true, &g, false);
        assert_eq!(call.text, "He's at your two o'clock, turning left");
        assert_eq!(call.stems, ["^HESYOUR", "^CLOCK02", "^TURNLFT"]);
        let straight = geometry(&body([0.; 3], 0.), &body([0., 0., 5_000.], 0.));
        let call = position_call(&p, false, &straight, false);
        assert_eq!(call.text, "He's at twelve o'clock");
        assert_eq!(call.stems, ["^HESAT", "^CLCK12D"]);
        let call = range_call(&p, 2, 90.);
        assert_eq!(call.text, "2 miles, three o'clock, in range");
        assert_eq!(call.stems, ["^MILE02", "^CLOCK03", "^INRANGE"]);
        let call = range_call(&p, 7, 0.);
        assert_eq!(call.text, "7 miles");
        assert_eq!(call.stems, ["^MILE07"]);
    }

    #[test]
    fn surface_targets_get_approach_and_whole_mile_range_calls() {
        let mut voice = rio();
        let mut comms = Comms::new(5);
        let mut heard = Vec::new();
        for (step, nm) in [11.6, 10.9, 10.6, 9.4, 1.6].into_iter().enumerate() {
            let mut i = with_aircraft(input(), nm * FEET_PER_NM, 0.);
            i.target.as_mut().unwrap().kind = TargetKind::Surface;
            i.now = step as f64 * 20.;
            heard.extend(run(&mut voice, &mut comms, &i).into_iter().map(|c| c.text));
        }
        assert_eq!(
            heard,
            ["12 miles", "", "9 miles", "2 miles, in range"],
            "range, approaching target (no text in the test table), nothing \
             for the same whole mile, then range calls"
        );
    }

    #[test]
    fn g_crossings_warn_on_the_eighteenth_and_sicken_on_the_twentieth() {
        struct Pilot {
            voice: CrewVoice,
            comms: Comms,
            input: Input,
            tick: u32,
            heard: Vec<(u32, String)>,
        }
        impl Pilot {
            /// One tick at `g`, keeping what was heard.
            fn fly(&mut self, g: f64) {
                self.tick += 1;
                self.input.now = f64::from(self.tick) / 120.;
                self.input.g = g;
                for call in run(&mut self.voice, &mut self.comms, &self.input) {
                    self.heard.push((self.tick, call.stems[0].clone()));
                }
            }
            /// `n` crossings of -1 G, one per tick.
            fn cross(&mut self, n: usize) {
                for _ in 0..n {
                    let g = if self.input.g >= -1. { -1.5 } else { 0. };
                    self.fly(g);
                }
            }
            /// `ticks` without crossing.
            fn hold(&mut self, ticks: usize) {
                let g = self.input.g;
                for _ in 0..ticks {
                    self.fly(g);
                }
            }
        }
        let mut pilot = Pilot {
            voice: rio(),
            comms: Comms::new(9),
            input: input(),
            tick: 0,
            heard: Vec::new(),
        };
        pilot.fly(0.);
        pilot.cross(17);
        assert!(pilot.heard.is_empty());
        pilot.cross(1);
        assert_eq!(
            pilot.heard,
            [(19, "^EASEUP".to_string())],
            "tick 1 sets the baseline"
        );
        // The channel is held for 3 s: those crossings do not count.
        pilot.cross(359);
        assert_eq!(pilot.heard.len(), 1);
        pilot.cross(1);
        assert_eq!(pilot.heard.len(), 1, "the 19th counted crossing");
        pilot.cross(1);
        assert_eq!(pilot.heard.len(), 2, "the 20th counted crossing");
        assert!(SICK.contains(&pilot.heard[1].1.as_str()));
        // The count restarts after being sick, and at every clock minute.
        pilot.hold(360);
        pilot.input.clock_minute = 1;
        pilot.cross(17);
        pilot.input.clock_minute = 2;
        pilot.cross(17);
        assert_eq!(
            pilot.heard.len(),
            2,
            "17 and 17 across a minute say nothing"
        );
        pilot.cross(1);
        assert_eq!(pilot.heard.len(), 3);
        assert_eq!(pilot.heard[2].1, "^EASEUP");
    }

    #[test]
    fn strain_is_a_thirty_percent_chance_on_entering_hard_g() {
        let mut strained = 0;
        for seed in 0..2_000 {
            let mut voice = rio();
            let mut comms = Comms::new(seed);
            let mut i = input();
            i.g = 4.9;
            run(&mut voice, &mut comms, &i);
            i.now = 1. / 120.;
            i.g = 5.2;
            let heard = run(&mut voice, &mut comms, &i);
            if heard.iter().any(|c| STRAIN.contains(&c.stems[0].as_str())) {
                strained += 1;
            }
            // Already hard: no second chance while staying there.
            i.now = 4.;
            i.g = 6.;
            assert!(
                run(&mut voice, &mut comms, &i)
                    .iter()
                    .all(|c| !STRAIN.contains(&c.stems[0].as_str()))
            );
        }
        assert!((500..700).contains(&strained), "{strained} of 2000");
        // Pushing below -2 G counts too; the wingman never grunts.
        let mut voice = rio();
        let mut comms = Comms::new(1);
        let mut i = input();
        i.g = -2.;
        run(&mut voice, &mut comms, &i);
        assert_eq!(
            voice.g_sound(Some(-2), -3, 0, &mut Comms::new(2)).is_some(),
            Comms::new(2).roll() < 30
        );
    }

    #[test]
    fn fuel_calls_are_said_once_and_a_worse_state_skips_milder_ones() {
        let mut voice = rio();
        let mut comms = Comms::new(1);
        comms.toggle_silence();
        let mut heard = Vec::new();
        for (t, fuel) in [
            FuelState::Ok,
            FuelState::Caution,
            FuelState::Caution,
            FuelState::Critical,
            FuelState::Bingo,
            FuelState::OutOfFuel,
        ]
        .into_iter()
        .enumerate()
        {
            let mut i = input();
            i.now = t as f64 * 10.;
            i.fuel = fuel;
            heard.extend(
                run(&mut voice, &mut comms, &i)
                    .into_iter()
                    .map(|c| c.stems[0].clone()),
            );
        }
        assert_eq!(
            heard,
            ["^JOKER", "^WEFUMES", "^OUTGAS"],
            "radio silence keeps them"
        );
        let mut single = CrewVoice::with(None, true);
        let mut i = input();
        i.fuel = FuelState::Bingo;
        assert!(run(&mut single, &mut comms, &i).is_empty());
    }

    #[test]
    fn missile_warnings_follow_seeker_class_with_shared_repeat_limits() {
        let mut voice = rio();
        let mut comms = Comms::new(1);
        let missile = |id, age, signature| Incoming { id, age, signature };
        let mut heard = Vec::new();
        let mut tick = |now: f64, incoming: Vec<Incoming>, heard: &mut Vec<(f64, String)>| {
            let mut i = input();
            i.now = now;
            i.incoming = incoming;
            for call in run(&mut voice, &mut comms, &i) {
                heard.push((now, call.stems[0].clone()));
            }
        };
        tick(0., vec![missile(1, 0.9, 2)], &mut heard);
        assert!(heard.is_empty(), "warned one second after launch");
        tick(0.1, vec![missile(1, 1.0, 2)], &mut heard);
        tick(
            0.6,
            vec![missile(1, 1.5, 2), missile(2, 1.0, 2)],
            &mut heard,
        );
        tick(
            1.1,
            vec![missile(2, 1.5, 2), missile(3, 1.0, 3), missile(4, 1.0, 5)],
            &mut heard,
        );
        tick(
            1.6,
            vec![missile(5, 1.0, 5), missile(6, 1.0, 3)],
            &mut heard,
        );
        tick(2.1, vec![], &mut heard);
        tick(6.1, vec![missile(7, 1.0, 2)], &mut heard);
        tick(6.6, vec![], &mut heard);
        assert_eq!(
            heard,
            [
                (0.6, "^ATOLFLR".to_string()),
                (1.6, "^APEXCHF".into()),
                (1.6, "^MISSBRK".into()),
                (2.1, "^MISSBRK".into()),
                (6.6, "^ATOLFLR".into()),
            ],
            "half a second late; infrared and radar 6 s apart, others always"
        );
        let mut single = CrewVoice::with(None, true);
        let mut i = input();
        i.incoming = vec![missile(9, 2., 3)];
        single.step(&i, &mut comms, &phrases());
        assert!(comms.due(10.).is_empty(), "single seat: tones only");
    }

    #[test]
    fn a_single_seat_player_is_coached_by_a_close_wingman_on_the_same_target() {
        let wingman = |position: Vector, alive, target| Wingman {
            label: "Friendly 1-2".into(),
            alive,
            target,
            position,
        };
        let cases = [
            (wingman([500., 10_000., -500.], true, Some(7)), true),
            (wingman([0., 10_000., -15_000.], true, Some(7)), true),
            (wingman([0., 10_000., -15_001.], true, Some(7)), false),
            (wingman([500., 10_000., -500.], true, Some(8)), false),
            (wingman([500., 10_000., -500.], false, Some(7)), false),
        ];
        for (w, speaks) in cases {
            let mut voice = CrewVoice::with(None, true);
            let mut comms = Comms::new(4);
            let mut i = with_aircraft(input(), -5_000., 0.);
            i.wingman = Some(w);
            let heard = run(&mut voice, &mut comms, &i);
            assert_eq!(!heard.is_empty(), speaks);
            if speaks {
                assert_eq!(heard[0].label, "Friendly 1-2");
                assert!(BREAK_WINGMAN.contains(&heard[0].stems[0].as_str()));
            }
        }
        // He never makes G sounds for the player.
        for seed in 0..100 {
            let mut voice = CrewVoice::with(None, true);
            let mut comms = Comms::new(seed);
            let mut i = input();
            i.wingman = Some(wingman([500., 10_000., -500.], true, None));
            i.g = 4.;
            run(&mut voice, &mut comms, &i);
            i.now = 1. / 120.;
            i.g = 6.;
            assert!(run(&mut voice, &mut comms, &i).is_empty());
        }
        // No wingman: silence, even with a target.
        let mut voice = CrewVoice::with(None, true);
        let mut comms = Comms::new(4);
        assert!(run(&mut voice, &mut comms, &with_aircraft(input(), -5_000., 0.)).is_empty());
    }

    #[test]
    fn feet_wet_and_dry_track_silently_under_radio_silence() {
        let mut voice = rio();
        let mut comms = Comms::new(8);
        let mut i = input();
        i.over_water = Some(false);
        assert!(
            run(&mut voice, &mut comms, &i).is_empty(),
            "the first sample is silent"
        );
        i.now = 20.;
        i.over_water = Some(true);
        let heard = run(&mut voice, &mut comms, &i);
        assert!(FEET_WET.contains(&heard[0].stems[0].as_str()));
        comms.toggle_silence();
        i.now = 40.;
        i.over_water = Some(false);
        assert!(run(&mut voice, &mut comms, &i).is_empty());
        comms.toggle_silence();
        i.now = 60.;
        assert!(
            run(&mut voice, &mut comms, &i).is_empty(),
            "no stale feet dry"
        );
    }

    #[test]
    fn the_player_screams_when_destroyed_unless_ejected() {
        let mut voice = CrewVoice::with(None, true);
        let mut comms = Comms::new(2);
        let mut i = input();
        i.crashed = true;
        let heard = run(&mut voice, &mut comms, &i);
        assert_eq!(heard.len(), 1);
        assert_eq!(heard[0].route, comms::Route::Direct);
        assert_eq!(heard[0].kind, Kind::Important);
        assert!(SCREAM.contains(&heard[0].stems[0].as_str()));
        i.now = 1.;
        assert!(run(&mut voice, &mut comms, &i).is_empty(), "once");
        let mut voice = rio();
        let mut i = input();
        i.ejected = true;
        i.crashed = true;
        assert!(run(&mut voice, &mut comms, &i).is_empty());
        let counts = (0..4000u64).fold([0; 3], |mut n, seed| {
            let stem = Comms::pick(Comms::new(seed).roll(), SCREAM);
            n[["^AARRRGH", "^OHSH", "^YAAAAAH"]
                .iter()
                .position(|s| *s == stem)
                .unwrap()] += 1;
            n
        });
        assert!(
            counts[0] > 1800 && counts[1] > 800 && counts[2] > 800,
            "{counts:?}"
        );
    }

    #[test]
    fn offensive_switch_to_missiles_and_overshoot() {
        let mut switched = 0;
        for seed in 0..400 {
            let mut voice = rio();
            let mut comms = Comms::new(seed);
            let mut i = with_aircraft(input(), 6_000., 0.);
            i.gun_selected = true;
            i.missile_would_lock = true;
            let heard = run(&mut voice, &mut comms, &i);
            if heard[0].stems[0] == "^SWCMISS" {
                switched += 1;
            }
        }
        assert!((60..140).contains(&switched), "about 25%: {switched}");
        let mut voice = rio();
        let mut comms = Comms::new(1);
        let mut i = with_aircraft(input(), 1_000., 0.);
        i.target.as_mut().unwrap().body.speed = 600. - 146.;
        assert_eq!(run(&mut voice, &mut comms, &i)[0].stems, ["^DONTOVR"]);
    }
}
