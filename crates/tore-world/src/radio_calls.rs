//! The radio calls aircraft make about their weapons, hits, kills, damage,
//! contacts, fuel and wing orders, and who hears them. Behaviour:
//! docs/spec/radio-chatter.md. The player's combat events and the AI's
//! [`Chatter`] are the producers; `comms` owns delay, radio silence and
//! playback. Nothing here changes combat or AI state.
//!
//! Every call made, heard or not, and every call a rule holds back is
//! written to the channel's journal with its trigger, rolls and reason.
use std::collections::{BTreeMap, BTreeSet};

use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::combat::{
    live::{self, Strike},
    missiles::{self, TargetRole},
};

use crate::ai_wings::{AiWings, Chatter, Contact, FuelLevel, Member};
use crate::comms::journal::{
    self, Cause, Entry, Origin, Outcome, REPEAT_S, Reason, Roll, Source, Store, Test, WingReply,
};
use crate::comms::{self, Call, Comms, Crew, Hearer, Kind, Phrase, Phrases, Route};
use crate::seats::{Pilot, PlaneId, Roster, SeatId};

/// Flight colours, first flight first (spec-derived).
pub const FLIGHTS: [&str; 8] = [
    "Red", "Blue", "Green", "Black", "White", "Orange", "Purple", "Yellow",
];
pub(crate) const POSITIONS: [&str; 12] = [
    "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten", "eleven",
    "twelve",
];

// Recording sets in the original's table order, which the roll indexes
// (spec-derived).
pub const GENERIC_LAUNCH: [&str; 3] = ["^IMSHOT", "^MISSAWY", "^FIRMISS"];
pub const GUIDED_HITS: [&str; 5] = ["^BULLS1", "^IMPACT", "^OHYEAH", "^ALRIGHT", "^GDSHOT"];
pub const UNGUIDED_HITS: [&str; 8] = [
    "^BULLS1", "^HEDAMGE", "^MULTHIT", "^OHYEAH", "^FRAGGED", "^HOTLEAD", "^DEBRIS", "^HURTIN",
];
pub const AIRCRAFT_KILLS: [&str; 12] = [
    "^GOODHIT", "^GDKILL", "^SPLBNDT", "^IMPACT", "^YEEHAW1", "^BEAUT1", "^OHYEAH", "^DNCNT",
    "^CRSHBRN", "^WIPEOUT", "^BRKUP", "^GOFLAM",
];
pub const OTHER_KILLS: [&str; 10] = [
    "^IMPACT", "^YEEHAW2", "^BEAUT2", "^GOTHIM", "^BULLS2", "^HOOHOO", "^OHYES", "^FIRBALL",
    "^HISTORY", "^WOOH",
];
pub const HIT_BY_AIRCRAFT: [&str; 5] = ["^IMHIT1", "^IMDMGE1", "^OFFME", "^SCORCH", "^HEAT"];
pub const HIT_BY_AAA: [&str; 4] = ["^IMHIT2", "^IMDMGE2", "^IMAAA", "^EATLD"];
pub const HIT_BY_OTHER: [&str; 2] = ["^IMHIT2", "^IMDMGE2"];
/// With an ejection seat all six; otherwise the first three.
pub const DEATHS: [&str; 6] = [
    "^AARRRGH", "^OHSH", "^YAAAAAH", "^EJECT", "^SEEHELL", "^PUNCH",
];
pub const ENGAGE_AIRCRAFT: [&str; 9] = [
    "^ENGAGE", "^ISEEEM", "^SHWTIME", "^GETEM", "^YAHOO1", "^TALLYHO", "^ONHIM", "^IGO", "^IGOAF",
];
pub const FRIENDLY_FIRE: [&str; 8] = [
    "^WHTHELL", "^WTCHOUT", "^YOUNUTS", "^YOUCRZY", "^WHOSIDE", "^IMGOOD", "^GETOFF", "^IMYOUR",
];

/// Friendly-fire complaints reach the player within 10 statute miles.
pub const FRIENDLY_FIRE_FT: f64 = 52_800.;
/// Contact size and type words are said within 15 nautical miles.
const SIZE_MILES: u32 = 15;
/// The side list of a tick with no human-flown plane.
static NO_FRIENDLIES: BTreeSet<u32> = BTreeSet::new();

// Global cooldowns, shared by every speaker (spec-derived seconds).
const BOMBS: &str = "radio-bombs";
const GUN: &str = "radio-gun";
const UNGUIDED_HIT: &str = "radio-unguided-hit";
const OTHER_KILL: &str = "radio-other-kill";
const COMPLAINT: &str = "radio-friendly-fire";

/// Who a call is addressed to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    /// The speaker's flight leader.
    Leader,
    /// Everyone else in the speaker's flight.
    Flight,
    /// One aircraft alone, by its id: the shooter a friendly-fire complaint
    /// goes to.
    Plane(u32),
}
impl From<Audience> for journal::Audience {
    fn from(audience: Audience) -> Self {
        match audience {
            Audience::Leader => Self::Leader,
            Audience::Flight => Self::Flight,
            Audience::Plane(_) => Self::Player,
        }
    }
}

/// A weapon release as the launch call reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release {
    /// The store's type flags: 0x1 guided, 0x10 bomb.
    pub flags: u32,
    /// Seeker signature: 2 infrared, 3 radar.
    pub seeker: u8,
    /// The AIM-54 Phoenix, the one "Fox three" store.
    pub phoenix: bool,
    pub target: Option<u32>,
}
impl Release {
    pub fn of(weapon: &Weapon, target: Option<u32>) -> Self {
        Self {
            flags: weapon.flags,
            seeker: weapon.seeker.signature,
            phoenix: weapon.hud_name.eq_ignore_ascii_case("AIM-54"),
            target,
        }
    }
    fn bomb(&self) -> bool {
        self.flags & 0x10 != 0
    }
    fn guided(&self) -> bool {
        self.flags & 1 != 0
    }
    fn store(&self) -> Store {
        Store {
            flags: self.flags,
            seeker: self.seeker,
            phoenix: self.phoenix,
        }
    }
}

/// What kind of object fired the round that hit an aircraft.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Attacker {
    Aircraft,
    Aaa,
    Other,
}
impl From<Attacker> for journal::Attacker {
    fn from(attacker: Attacker) -> Self {
        match attacker {
            Attacker::Aircraft => Self::Aircraft,
            Attacker::Aaa => Self::Aaa,
            Attacker::Other => Self::Other,
        }
    }
}

/// "Red two": flight colour and position word. A flight past the eighth has
/// no colour in the original; TORE names it by number (fitted).
pub fn label(member: &Member) -> String {
    let position = POSITIONS
        .get(usize::from(member.position))
        .copied()
        .unwrap_or("?");
    match FLIGHTS.get(usize::from(member.flight)) {
        Some(colour) => format!("{colour} {position}"),
        None => format!("Flight {} {position}", member.flight + 1),
    }
}

/// The recordings of the flight colours (the first five flights; the others
/// have the words only), as the assignment call has them.
const COLOUR_STEMS: [&str; 5] = ["^RED", "^BLUE", "^GREEN", "^BLACK", "^WHITE"];

/// What a battle net line puts first: the speaking flight's colour, "Blue, ".
/// A flight past the eighth has no colour; it is named by number, as the radio
/// labels do (fitted).
fn battle_prefix(flight: u8) -> Phrase {
    match FLIGHTS.get(usize::from(flight)) {
        Some(colour) => Phrase::default().raw(
            &format!("{colour}, "),
            COLOUR_STEMS.get(usize::from(flight)).copied(),
        ),
        None => Phrase::default().raw(&format!("Flight {}, ", flight + 1), None),
    }
}

/// The words a seat hears over the battle net: the speaking flight's colour,
/// then what the flight heard (slice G8). A call that already begins with the
/// colour, as an attack call to the whole flight does ("Red, attack bandit"),
/// is not given it twice (agent decision).
pub fn battle_words(flight: u8, words: Phrase) -> Phrase {
    let prefix = battle_prefix(flight);
    let named = match prefix.stems.first() {
        Some(stem) => words.stems.first() == Some(stem),
        None => words.text.starts_with(prefix.text.trim_end_matches(", ")),
    };
    if named { words } else { prefix.join(words) }
}

/// "Net Blue one": the label the battle net's HUD line is spoken under.
pub fn battle_label(member: &Member) -> String {
    format!("{}{}", comms::Net::Battle.prefix(), label(member))
}

/// The seats that hear `speaker` over the battle net (slice G8): when the
/// speaker leads its flight, every living seat of its side, outside that flight,
/// that monitors the net (Alt+N). Each hears the call as `words` gives it,
/// under the colour of the speaking flight and the label `Net Blue one`. A
/// speaker that does not lead a flight, or an enemy side's, reaches no one.
/// The wing net's hearers are made as before; these are added to them.
pub fn battle_hearers(
    comms: &Comms,
    members: &[Member],
    leaders: &[(u8, u32)],
    listeners: &[Listener],
    speaker: u32,
    words: &dyn Fn(&Listener) -> Phrase,
) -> Vec<Hearer> {
    let Some(member) = members.iter().find(|m| m.id == speaker) else {
        return Vec::new();
    };
    if !leaders
        .iter()
        .any(|&(flight, plane)| flight == member.flight && plane == speaker)
    {
        return Vec::new();
    }
    let label = battle_label(member);
    listeners
        .iter()
        .filter(|listener| {
            listener.alive
                && listener.plane != speaker
                && listener.enemy == member.enemy
                && listener.flight != member.flight
                && comms.monitors_battle(listener.seat)
        })
        .map(|listener| {
            Hearer::named(listener.seat, label.clone())
                .saying(battle_words(member.flight, words(listener)))
                .on(comms::Net::Battle)
        })
        .collect()
}

/// How a call is worded for one listener, when that depends on where it is.
type SeatWords<'a> = dyn Fn(&Listener) -> Option<Phrase> + 'a;

/// One human-flown plane as the radio's listener rule sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct Listener {
    pub seat: SeatId,
    /// The id of the plane the seat flies, which its own calls are made under.
    pub plane: u32,
    /// The radio flight number of the plane's wing, as in [`Member::flight`],
    /// and whether that wing is on the enemy side.
    pub flight: u8,
    pub enemy: bool,
    /// The plane and its pilot are alive.
    pub alive: bool,
    pub position: [f64; 3],
    /// The label of the second seat of the plane's type, if it has one.
    pub crew: Option<Crew>,
}

/// The world as the radio sees it for one tick.
pub struct Scene<'a> {
    pub now: f64,
    pub phrases: &'a Phrases,
    /// Every human-flown plane, in seat order.
    pub listeners: &'a [Listener],
    /// Every plane of the mission that has a radio name, human-flown ones
    /// included.
    pub members: &'a [Member],
    /// The plane that leads each flight, by radio flight number.
    pub leaders: &'a [(u8, u32)],
    pub targets: &'a [live::Target],
    /// Friendly target ids; other non-AI targets are hostile.
    pub friendlies: &'a BTreeSet<u32>,
}
impl Scene<'_> {
    fn member(&self, id: u32) -> Option<&Member> {
        self.members.iter().find(|m| m.id == id)
    }
    fn target(&self, id: u32) -> Option<&live::Target> {
        self.targets.iter().find(|t| t.id == id)
    }
    /// The listener flying plane `id`, if a human does.
    fn human(&self, id: u32) -> Option<&Listener> {
        self.listeners.iter().find(|l| l.plane == id)
    }
    fn enemy(&self, id: u32) -> bool {
        if let Some(member) = self.member(id) {
            member.enemy
        } else {
            !self.friendlies.contains(&id) && self.human(id).is_none()
        }
    }
    fn aircraft(&self, id: u32) -> bool {
        self.member(id).is_some()
            || self.human(id).is_some()
            || self
                .target(id)
                .is_some_and(|t| t.role == TargetRole::Aircraft)
    }
    fn attacker(&self, id: u32) -> Attacker {
        if self.aircraft(id) {
            Attacker::Aircraft
        } else if self.target(id).is_some_and(|t| t.category & 0x800 != 0) {
            Attacker::Aaa
        } else {
            Attacker::Other
        }
    }
    /// Whether the plane that flies `listener` leads its flight.
    fn leads(&self, listener: &Listener) -> bool {
        self.leaders
            .iter()
            .any(|&(flight, plane)| flight == listener.flight && plane == listener.plane)
    }
    /// The listener rule: the label `listener` hears, or `None` when it is
    /// not a receiver. A plane's own calls are always heard by its seat.
    fn label(&self, listener: &Listener, speaker: u32, audience: Audience) -> Option<String> {
        if speaker == listener.plane {
            // A call that ends up addressed to the player's own aircraft uses
            // the crew label in a multi-crew aircraft.
            let alone = !self
                .members
                .iter()
                .any(|m| m.flight == listener.flight && m.id != listener.plane && m.alive);
            let to_self = audience != Audience::Flight || alone;
            return Some(match (to_self, listener.crew) {
                (true, Some(crew)) => crew.label().to_string(),
                _ => "YOU".to_string(),
            });
        }
        let member = self.member(speaker)?;
        let heard = match audience {
            Audience::Plane(plane) => plane == listener.plane,
            Audience::Flight => listener.alive && member.flight == listener.flight,
            Audience::Leader => {
                listener.alive && member.flight == listener.flight && self.leads(listener)
            }
        };
        heard.then(|| label(member))
    }
    /// The battle net's hearers of `speaker`'s call, as [`battle_hearers`].
    fn battle_hearers(
        &self,
        comms: &Comms,
        speaker: u32,
        words: &dyn Fn(&Listener) -> Phrase,
    ) -> Vec<Hearer> {
        battle_hearers(
            comms,
            self.members,
            self.leaders,
            self.listeners,
            speaker,
            words,
        )
    }
    /// Every seat that hears `speaker`'s call to `audience`, under the label
    /// that seat hears it by.
    fn hearers(&self, speaker: u32, audience: Audience) -> Vec<Hearer> {
        self.listeners
            .iter()
            .filter_map(|listener| {
                self.label(listener, speaker, audience)
                    .map(|label| Hearer::named(listener.seat, label))
            })
            .collect()
    }
    /// Why no seat heard a call from `speaker`: the most telling reason of
    /// the seats, a downed listener before another flight before the enemy.
    /// Journal only.
    fn unheard(&self, speaker: u32) -> Reason {
        let Some(member) = self.member(speaker) else {
            return Reason::NoRadioIdentity;
        };
        let mut first = None;
        for listener in self.listeners {
            let reason = if member.enemy != listener.enemy {
                Reason::EnemyFlight
            } else if member.flight != listener.flight {
                Reason::OtherFlight
            } else {
                return Reason::PlayerDown;
            };
            first.get_or_insert(reason);
        }
        first.unwrap_or(Reason::NoRadioIdentity)
    }
    /// The speaker's radio name whether or not anyone hears it, for the
    /// journal: `YOU` for a human-flown plane.
    fn name(&self, speaker: u32) -> String {
        if self.human(speaker).is_some() {
            "YOU".into()
        } else {
            self.member(speaker)
                .map_or_else(|| format!("Aircraft {speaker}"), label)
        }
    }
}

/// How the launch call read its roll: the Fox chance, a generic variant, or
/// nothing for the one-recording calls.
fn launch_roll(roll: u32, release: &Release, aircraft: bool) -> Roll {
    if release.phoenix || release.bomb() {
        Roll::new("launch call with one recording", roll, Test::Unused)
    } else if matches!(release.seeker, 2 | 3) && aircraft {
        Roll::new(
            "Fox call, else a generic call by the roll mod 3",
            roll,
            Test::Below(50),
        )
    } else {
        Roll::pick("generic launch call", roll, GENERIC_LAUNCH.len())
    }
}

/// How the contact report read its roll: the size words.
fn contact_roll(roll: u32, contact: &Contact) -> Roll {
    match contact.count {
        _ if contact.miles > SIZE_MILES => {
            Roll::new("size words beyond 15 miles", roll, Test::Unused)
        }
        2 => Roll::new(
            "size of a pair: pair of, two-ship formation, or multiple",
            roll,
            Test::Bands(&[40, 70]),
        ),
        3..=12 => Roll::new("size as a number, else multiple", roll, Test::Below(50)),
        _ => Roll::new("size words for one or many", roll, Test::Unused),
    }
}

/// Record a call a rule held back. Only the first in the rule's window is
/// listed for each speaker, so a gun burst makes one entry per cooldown.
#[allow(clippy::too_many_arguments)]
fn held(
    comms: &mut Comms,
    scene: &Scene,
    speaker: u32,
    cause: Cause,
    rolls: Vec<Roll>,
    rule: &'static str,
    until: f64,
    reason: Reason,
) {
    if comms.first_in_window(rule, speaker, scene.now, until) {
        comms.record(Entry::note(
            scene.now,
            scene.name(speaker),
            Origin::of(Source::Radio, cause).by(speaker).rolls(rolls),
            Outcome::Suppressed(reason),
        ));
    }
}

/// Record a call a shared cooldown held back.
fn cooled(
    comms: &mut Comms,
    scene: &Scene,
    speaker: u32,
    cause: Cause,
    rolls: Vec<Roll>,
    key: &'static str,
    seconds: f64,
) {
    let remaining = comms.remaining(key, scene.now);
    held(
        comms,
        scene,
        speaker,
        cause,
        rolls,
        key,
        scene.now + remaining,
        Reason::Cooldown {
            key,
            seconds,
            remaining,
        },
    );
}

/// The launch call wording for `roll`, before cooldowns. `aircraft` is
/// whether the release's target is an aircraft.
pub fn launch_phrase(phrases: &Phrases, roll: u32, release: &Release, aircraft: bool) -> Phrase {
    if release.phoenix {
        return Phrase::stem(phrases, "^FOXTHR");
    }
    if release.bomb() {
        return Phrase::stem(phrases, "^BOMBAWY");
    }
    let mut phrase = Phrase::default();
    if !release.guided() {
        // `^FIRGUN` has no recording; text only. The separator is fitted.
        phrase = phrase.then(phrases, "^FIRGUN").raw(". ", None);
    }
    let fox = match release.seeker {
        3 => Some("^FOXONE"),
        2 => Some("^FOXTWO"),
        _ => None,
    };
    let stem = match fox {
        Some(fox) if roll < 50 && aircraft => fox,
        _ => Comms::pick(roll, &GENERIC_LAUNCH),
    };
    phrase.then(phrases, stem)
}

/// "Splash one MiG-29 Fulcrum-C", voiced `^SPLASH` then `^AC<resource>`
/// when that stem fits the original's eight characters.
pub fn splash(phrases: &Phrases, aircraft: AircraftId) -> Phrase {
    let resource = aircraft.pt().trim_end_matches(".PT");
    let stem = format!("^AC{resource}");
    Phrase::stem(phrases, "^SPLASH").raw(aircraft.label(), (stem.len() <= 8).then_some(&*stem))
}

/// A kill of an aircraft always gets the generic set for the F-22 (the
/// original also does this for rotorcraft, the V-22 and the blimp, none of
/// which TORE flies).
fn always_generic(aircraft: AircraftId) -> bool {
    aircraft.pt() == "F22.PT"
}

/// The contact noun: the type name when identified, voiced "bandits" except
/// for the MiG-17, MiG-19 and MiG-21, which have their own recordings.
fn noun(phrases: &Phrases, named: Option<&str>, plural: bool) -> Phrase {
    let s = if plural { "S" } else { "" };
    let mig = named.and_then(|name| {
        ["17", "19", "21"].into_iter().find(|n| {
            name.get(..6)
                .is_some_and(|head| head.eq_ignore_ascii_case(&format!("mig-{n}")))
        })
    });
    let stem = match mig {
        Some(n) => format!("^MIG{n}{s}"),
        None => format!("^BANDIT{s}"),
    };
    match named {
        Some(name) => Phrase::default().raw(
            &format!("{name}{}", if plural { "s" } else { "" }),
            Some(&stem),
        ),
        None => Phrase::stem(phrases, &stem),
    }
}

/// "Contact, pair of bandits, your two o'clock high, 12 miles, please
/// advise." The size-word split for two aircraft (40/30/30) and for three
/// to twelve (50/50) reads the call's roll low to high (fitted order).
pub fn contact_phrase(phrases: &Phrases, roll: u32, contact: &Contact) -> Phrase {
    let mut p = Phrase::stem(phrases, "^CONTACT");
    if contact.miles <= SIZE_MILES {
        let mut two_ship = false;
        let size = match contact.count {
            0 | 1 => None,
            2 if roll < 40 => Some(Phrase::stem(phrases, "^PAIROF")),
            2 if roll < 70 => {
                two_ship = true;
                None
            }
            n @ 3..=12 if roll < 50 => Some(Phrase::stem(phrases, &format!("^NUM{n:02}"))),
            _ => Some(Phrase::stem(phrases, "^MULTPLE")),
        };
        if let Some(size) = size {
            p = p.join(size).raw(" ", None);
        }
        p = p.join(noun(phrases, contact.named.as_deref(), contact.count > 1));
        if two_ship {
            p = p.then(phrases, "^2SHFORM");
        }
        p = p.raw(", ", None);
    }
    p = p.join(comms::clock(
        phrases,
        true,
        contact.hour,
        contact.elevation,
        false,
    ));
    if contact.miles >= 1 {
        p = p.raw(", ", None).join(comms::miles(phrases, contact.miles));
    }
    if contact.advise {
        p = p.then(phrases, "^PLSADVS");
    }
    p
}

/// Per-aircraft radio limits for one flight.
#[derive(Default)]
pub struct Radio {
    /// Per shooter: unguided hits are announced at most every 8 seconds.
    unguided_hits: BTreeMap<u32, f64>,
    /// Per aircraft: "I'm hit" from gun rounds at most every 8 seconds.
    bullet_hits: BTreeMap<u32, f64>,
    /// Calls made this flight, heard or not, and those addressed to the player.
    pub made: u32,
    pub heard: u32,
}
impl Radio {
    /// Make a call, once: one variant, one journal number. The listener rule
    /// decides which seats hear it and by what label; each of them queues the
    /// call in its own channel. A call no seat hears is journaled and goes no
    /// further.
    #[allow(clippy::too_many_arguments)]
    fn say(
        &mut self,
        comms: &mut Comms,
        scene: &Scene,
        speaker: u32,
        audience: Audience,
        phrase: Phrase,
        kind: Kind,
        delay: f64,
        origin: Origin,
    ) {
        self.say_to(
            comms, scene, speaker, audience, phrase, kind, delay, origin, None, false,
        );
    }

    /// [`Self::say`], where `words` may give a seat its own wording of the
    /// call, from where its plane is, and `battle` repeats the call over the
    /// battle net when the speaker leads its flight (slice G8).
    #[allow(clippy::too_many_arguments)]
    fn say_to(
        &mut self,
        comms: &mut Comms,
        scene: &Scene,
        speaker: u32,
        audience: Audience,
        phrase: Phrase,
        kind: Kind,
        delay: f64,
        origin: Origin,
        words: Option<&SeatWords>,
        battle: bool,
    ) {
        self.made += 1;
        let origin = origin.by(speaker).to(audience.into());
        let mut hearers = scene.hearers(speaker, audience);
        if let Some(words) = words {
            for hearer in &mut hearers {
                hearer.words = scene
                    .listeners
                    .iter()
                    .find(|l| l.seat == hearer.seat)
                    .and_then(words);
            }
        }
        if battle {
            // The monitors hear what the flight hears, in their own geometry
            // where the call has one, with the flight's colour in front.
            let own = |listener: &Listener| {
                words
                    .and_then(|words| words(listener))
                    .unwrap_or_else(|| phrase.clone())
            };
            hearers.extend(scene.battle_hearers(comms, speaker, &own));
        }
        let Some(first) = hearers.first() else {
            comms.record(
                Entry::note(
                    scene.now,
                    scene.name(speaker),
                    origin,
                    Outcome::Unheard(scene.unheard(speaker)),
                )
                .with_text(phrase.text)
                .with_stems(phrase.stems)
                .with_kind(Route::Radio, kind),
            );
            return;
        };
        self.heard += 1;
        // A human's own calls are voiced when they are sent.
        let delay = if scene.human(speaker).is_some() {
            0.
        } else {
            delay
        };
        let label = first.label.clone().unwrap_or_default();
        comms.send(
            scene.now,
            Call::new(label, phrase, kind).after(delay).because(origin),
            &hearers,
        );
    }

    /// A weapon release: to the shooter's flight after half a second.
    pub fn release(&mut self, comms: &mut Comms, scene: &Scene, speaker: u32, release: Release) {
        let cause = Cause::Release {
            target: release.target,
            store: release.store(),
        };
        if release.target.is_none() && !release.bomb() {
            held(
                comms,
                scene,
                speaker,
                cause,
                Vec::new(),
                "release without a target",
                scene.now + REPEAT_S,
                Reason::NoTarget,
            );
            return;
        }
        if !release.phoenix {
            if release.bomb() {
                if !comms.cooldown(BOMBS, scene.now, 4.) {
                    cooled(comms, scene, speaker, cause, Vec::new(), BOMBS, 4.);
                    return;
                }
            } else if !release.guided() && !comms.cooldown(GUN, scene.now, 4.) {
                // `fitted`: the gun's 4 s cooldown silences the whole call,
                // so a burst of rounds makes one call, not one per round.
                cooled(comms, scene, speaker, cause, Vec::new(), GUN, 4.);
                return;
            }
        }
        let roll = comms.roll();
        let aircraft = release.target.is_some_and(|t| scene.aircraft(t));
        let phrase = launch_phrase(scene.phrases, roll, &release, aircraft);
        let rolls = vec![launch_roll(roll, &release, aircraft)];
        self.say(
            comms,
            scene,
            speaker,
            Audience::Flight,
            phrase,
            Kind::Chatter,
            0.5,
            Origin::of(Source::Radio, cause).rolls(rolls),
        );
    }

    /// A projectile damaged something: hit, kill, "I'm hit" and
    /// friendly-fire calls.
    pub fn strike(&mut self, comms: &mut Comms, scene: &Scene, strike: &Strike) {
        let shooter = strike.owner;
        let victim = strike.victim;
        let same_side = scene.enemy(shooter) == scene.enemy(victim);
        if strike.destroyed {
            if !same_side {
                self.kill(comms, scene, shooter, victim, strike);
            }
            return;
        }
        if !same_side {
            self.hit(comms, scene, shooter, strike);
            if scene.aircraft(victim) {
                self.damaged(comms, scene, victim, shooter, strike);
            }
        } else if let Some(shooter_seat) = scene.human(shooter)
            && scene.human(victim).is_none()
            && scene.aircraft(victim)
        {
            let Some(range_ft) = scene
                .target(victim)
                .map(|t| missiles::length(missiles::sub(t.position, shooter_seat.position)))
            else {
                return;
            };
            let cause = Cause::FriendlyFire { range_ft };
            if range_ft <= FRIENDLY_FIRE_FT {
                if comms.cooldown(COMPLAINT, scene.now, 6.) {
                    let roll = comms.roll();
                    let phrase = Phrase::stem(scene.phrases, Comms::pick(roll, &FRIENDLY_FIRE));
                    let rolls = vec![Roll::pick("complaint", roll, FRIENDLY_FIRE.len())];
                    self.say(
                        comms,
                        scene,
                        victim,
                        Audience::Plane(shooter),
                        phrase,
                        Kind::Chatter,
                        2.,
                        Origin::of(Source::Radio, cause).rolls(rolls),
                    );
                } else {
                    cooled(comms, scene, victim, cause, Vec::new(), COMPLAINT, 6.);
                }
            } else {
                held(
                    comms,
                    scene,
                    victim,
                    cause,
                    Vec::new(),
                    "friendly fire too far away",
                    scene.now + REPEAT_S,
                    Reason::TooFar {
                        range_ft,
                        limit_ft: FRIENDLY_FIRE_FT,
                    },
                );
            }
        }
    }

    fn hit(&mut self, comms: &mut Comms, scene: &Scene, shooter: u32, strike: &Strike) {
        let now = scene.now;
        let unguided = strike.weapon_flags & 1 == 0;
        let cause = Cause::Hit {
            victim: strike.victim,
            guided: !unguided,
        };
        if unguided && scene.aircraft(shooter) {
            if let Some(until) = self
                .unguided_hits
                .get(&shooter)
                .copied()
                .filter(|t| now < *t)
            {
                let rule = "unguided hits per shooter";
                let reason = Reason::PerAircraft {
                    rule,
                    id: shooter,
                    seconds: 8.,
                    remaining: until - now,
                };
                held(
                    comms,
                    scene,
                    shooter,
                    cause,
                    Vec::new(),
                    rule,
                    until,
                    reason,
                );
                return;
            }
            self.unguided_hits.insert(shooter, now + 8.);
        }
        let roll = comms.roll();
        let (stem, count) = if unguided {
            if !comms.cooldown(UNGUIDED_HIT, now, 4.) {
                let rolls = vec![Roll::new("hit call", roll, Test::Unused)];
                cooled(comms, scene, shooter, cause, rolls, UNGUIDED_HIT, 4.);
                return;
            }
            (Comms::pick(roll, &UNGUIDED_HITS), UNGUIDED_HITS.len())
        } else {
            (Comms::pick(roll, &GUIDED_HITS), GUIDED_HITS.len())
        };
        let phrase = Phrase::stem(scene.phrases, stem);
        let rolls = vec![Roll::pick("hit call", roll, count)];
        self.say(
            comms,
            scene,
            shooter,
            Audience::Flight,
            phrase,
            Kind::Chatter,
            0.5,
            Origin::of(Source::Radio, cause).rolls(rolls),
        );
    }

    fn kill(&mut self, comms: &mut Comms, scene: &Scene, shooter: u32, victim: u32, s: &Strike) {
        let roll = comms.roll();
        let aircraft = scene.aircraft(victim);
        let bomb = s.weapon_flags & 0x10 != 0;
        let cause = Cause::Kill {
            victim,
            aircraft,
            bomb,
        };
        let (phrase, rolls) = if aircraft {
            let named = scene
                .target(victim)
                .and_then(|t| t.aircraft)
                .filter(|id| !always_generic(*id));
            // A fresh 40% roll picks the generic set; it is drawn only for
            // a named type.
            let second = named.map(|_| comms.roll());
            let splash_roll = second.map(|value| {
                Roll::new(
                    "\"Splash one\" with the type name",
                    value,
                    Test::AtLeast(40),
                )
            });
            match (named, second) {
                (Some(id), Some(value)) if value >= 40 => (
                    splash(scene.phrases, id),
                    [
                        Some(Roll::new("kill call", roll, Test::Unused)),
                        splash_roll,
                    ],
                ),
                _ => (
                    Phrase::stem(scene.phrases, Comms::pick(roll, &AIRCRAFT_KILLS)),
                    [
                        Some(Roll::pick("kill call", roll, AIRCRAFT_KILLS.len())),
                        splash_roll,
                    ],
                ),
            }
        } else {
            if comms.cooling(OTHER_KILL, scene.now) {
                let rolls = vec![Roll::new("kill call", roll, Test::Unused)];
                cooled(comms, scene, shooter, cause, rolls, OTHER_KILL, 4.);
                return;
            }
            // Only a bomb kill starts the 4 s cooldown.
            if bomb {
                comms.cooldown(OTHER_KILL, scene.now, 4.);
            }
            (
                Phrase::stem(scene.phrases, Comms::pick(roll, &OTHER_KILLS)),
                [Some(Roll::pick("kill call", roll, OTHER_KILLS.len())), None],
            )
        };
        self.say(
            comms,
            scene,
            shooter,
            Audience::Flight,
            phrase,
            Kind::Chatter,
            0.5,
            Origin::of(Source::Radio, cause).rolls(rolls.into_iter().flatten().collect()),
        );
    }

    /// "I'm hit", by the aircraft that took an enemy round.
    fn damaged(&mut self, comms: &mut Comms, scene: &Scene, victim: u32, by: u32, s: &Strike) {
        let gun = s.weapon_flags & 0x80 != 0;
        let attacker = scene.attacker(by);
        let cause = Cause::Damaged {
            by,
            attacker: attacker.into(),
            gun,
        };
        if gun {
            if let Some(until) = self
                .bullet_hits
                .get(&victim)
                .copied()
                .filter(|t| scene.now < *t)
            {
                let rule = "\"I'm hit\" from gun rounds per aircraft";
                let reason = Reason::PerAircraft {
                    rule,
                    id: victim,
                    seconds: 8.,
                    remaining: until - scene.now,
                };
                held(comms, scene, victim, cause, Vec::new(), rule, until, reason);
                return;
            }
            self.bullet_hits.insert(victim, scene.now + 8.);
        }
        let roll = comms.roll();
        let set: &[&str] = match attacker {
            Attacker::Aircraft => &HIT_BY_AIRCRAFT,
            Attacker::Aaa => &HIT_BY_AAA,
            Attacker::Other => &HIT_BY_OTHER,
        };
        let phrase = Phrase::stem(scene.phrases, Comms::pick(roll, set));
        let rolls = vec![Roll::pick("\"I'm hit\" call", roll, set.len())];
        self.say(
            comms,
            scene,
            victim,
            Audience::Flight,
            phrase,
            Kind::Chatter,
            0.,
            Origin::of(Source::Radio, cause).rolls(rolls),
        );
    }

    /// "You're the Wingleader now": five seconds after the lead changed, to
    /// the new leader when a human flies it, in the previous leader's voice if
    /// that pilot is alive to say it, else as a HUD line alone (agent
    /// proposal, docs/spec/radio-chatter.md). When the AI takes the lead nobody
    /// hears anything, and nothing is journaled beyond the change itself.
    fn wingleader(
        &mut self,
        comms: &mut Comms,
        scene: &Scene,
        previous: u32,
        leader: u32,
        previous_pilot_alive: bool,
        cause: Cause,
    ) {
        let Some(listener) = scene.human(leader) else {
            return;
        };
        let origin = Origin::of(Source::Radio, cause)
            .by(previous)
            .to(journal::Audience::Player);
        let (label, phrase) = if previous_pilot_alive {
            (
                scene.label(listener, previous, Audience::Plane(leader)),
                Phrase::stem(scene.phrases, "^WNGLDR"),
            )
        } else {
            // Nobody is left to say it: the words print with no voice.
            let text = Phrase::stem(scene.phrases, "^WNGLDR").text;
            (
                Some("Flight".to_string()),
                Phrase::default().raw(&text, None),
            )
        };
        self.made += 1;
        let Some(label) = label else {
            return;
        };
        self.heard += 1;
        // Not silenced, and after five seconds whoever says it.
        comms.send(
            scene.now,
            Call::new(&label, phrase, Kind::Important)
                .after(5.)
                .because(origin),
            &[Hearer::named(listener.seat, label)],
        );
    }

    /// One AI radio event.
    pub fn chatter(&mut self, comms: &mut Comms, scene: &Scene, event: &Chatter) {
        let p = |stem: &str| Phrase::stem(scene.phrases, stem);
        let radio = |cause| Origin::of(Source::Radio, cause);
        match event {
            Chatter::Release { speaker, release } => self.release(comms, scene, *speaker, *release),
            Chatter::LaunchWarning {
                speaker,
                by_aircraft,
            } => {
                let stem = if *by_aircraft { "^MISSLCH" } else { "^SAMLCH" };
                self.say(
                    comms,
                    scene,
                    *speaker,
                    Audience::Flight,
                    p(stem),
                    Kind::Important,
                    0.5,
                    radio(Cause::LaunchSeen {
                        by_aircraft: *by_aircraft,
                    }),
                );
            }
            Chatter::Death {
                speaker,
                ejection_seat,
            } => {
                let roll = comms.roll();
                let set = if *ejection_seat {
                    &DEATHS[..]
                } else {
                    &DEATHS[..3]
                };
                let phrase = p(Comms::pick(roll, set));
                let rolls = vec![Roll::pick("death call", roll, set.len())];
                self.say(
                    comms,
                    scene,
                    *speaker,
                    Audience::Flight,
                    phrase,
                    Kind::Important,
                    0.,
                    radio(Cause::Death {
                        ejection_seat: *ejection_seat,
                    })
                    .rolls(rolls),
                );
            }
            Chatter::Engage { speaker, aircraft } => {
                let roll = comms.roll();
                let (stem, roll) = if *aircraft {
                    (
                        Comms::pick(roll, &ENGAGE_AIRCRAFT),
                        Roll::pick("engage reply", roll, ENGAGE_AIRCRAFT.len()),
                    )
                } else {
                    (
                        "^ENGAGE",
                        Roll::new("engage reply for a surface target", roll, Test::Unused),
                    )
                };
                self.say(
                    comms,
                    scene,
                    *speaker,
                    Audience::Leader,
                    p(stem),
                    Kind::Chatter,
                    2.,
                    Origin::of(
                        Source::Reply,
                        Cause::Reply(WingReply::Engage {
                            aircraft: *aircraft,
                        }),
                    )
                    .rolls(vec![roll]),
                );
            }
            Chatter::Showtime { speaker } => self.say(
                comms,
                scene,
                *speaker,
                Audience::Leader,
                p("^SHWTIME"),
                Kind::Important,
                2.,
                Origin::of(Source::Reply, Cause::Reply(WingReply::Showtime)),
            ),
            Chatter::Contact {
                speaker,
                target,
                contact,
            } => {
                let roll = comms.roll();
                let phrase = contact_phrase(scene.phrases, roll, contact);
                let rolls = vec![contact_roll(roll, contact)];
                // Every seat hears the same variant, worded from where its
                // own plane is: its own clock position, height, range and
                // whether the type can be named.
                let own_view = |listener: &Listener| {
                    let view = contact.views.iter().find(|v| v.plane == listener.plane)?;
                    let seen = Contact {
                        named: view.named.clone(),
                        hour: view.hour,
                        elevation: view.elevation,
                        miles: view.miles,
                        ..contact.clone()
                    };
                    Some(contact_phrase(scene.phrases, roll, &seen))
                };
                self.say_to(
                    comms,
                    scene,
                    *speaker,
                    Audience::Flight,
                    phrase,
                    Kind::Chatter,
                    0.5,
                    radio(Cause::Contact {
                        target: *target,
                        count: contact.count,
                        miles: contact.miles,
                        advise: contact.advise,
                    })
                    .rolls(rolls),
                    Some(&own_view),
                    true,
                );
            }
            Chatter::Leadership {
                speaker,
                leader,
                previous_pilot_alive,
                ..
            } => {
                let cause = Cause::Leadership {
                    new: *leader,
                    previous: *speaker,
                    previous_pilot_alive: *previous_pilot_alive,
                };
                comms.record(Entry::note(
                    scene.now,
                    scene.name(*speaker),
                    radio(cause.clone()),
                    Outcome::Noted,
                ));
                self.wingleader(
                    comms,
                    scene,
                    *speaker,
                    *leader,
                    *previous_pilot_alive,
                    cause,
                );
            }
            Chatter::Fuel { speaker, level } => {
                let stem = match level {
                    FuelLevel::Joker => "^JOKER",
                    FuelLevel::Bingo => "^BINGO",
                    FuelLevel::Fumes => "^IMFUMES",
                    FuelLevel::Out => "^OUTFUEL",
                };
                self.say(
                    comms,
                    scene,
                    *speaker,
                    Audience::Leader,
                    p(stem),
                    Kind::Important,
                    0.,
                    radio(Cause::AiFuel {
                        level: *level as u8,
                    }),
                );
            }
        }
    }
}

/// The radio names of a mission's planes, human-flown ones included: the
/// AI's in the order of its slots, then the human-flown planes the AI has no
/// slot for. Flights are numbered as the AI wings numbered them (the flight
/// of Friendly Wing 1 first, then the other populated friendly wings, then the
/// enemy wings), from the roster instead of the AI's slots, so a wing with only
/// humans has a number too. `human_alive` says whether a human-flown plane and
/// its pilot are alive.
pub fn members(
    roster: &Roster,
    wings: Option<&AiWings>,
    human_alive: impl Fn(PlaneId) -> bool,
) -> Vec<Member> {
    let mut flights: Vec<(bool, u8)> = vec![(false, 1)];
    for plane in roster.planes() {
        let key = (plane.slot.wing.side.is_enemy(), plane.slot.wing.index + 1);
        if !flights.contains(&key) {
            flights.push(key);
        }
    }
    flights.sort();
    let member = |plane: &crate::seats::Plane| {
        let key = (plane.slot.wing.side.is_enemy(), plane.slot.wing.index + 1);
        Member {
            id: plane.id.0,
            enemy: key.0,
            flight: flights.iter().position(|f| *f == key).unwrap_or(0) as u8,
            position: plane.slot.member,
            alive: match plane.pilot {
                Pilot::Human(_) => human_alive(plane.id),
                // A lost plane's pilot is dead or out of it (phase 2).
                Pilot::Lost => false,
                Pilot::Ai => wings
                    .and_then(|w| w.mission().actor(plane.id.0))
                    .is_some_and(|actor| actor.alive()),
            },
        }
    };
    let slots = wings.map_or(&[][..], |w| w.slots());
    let mut out: Vec<Member> = slots
        .iter()
        .filter_map(|slot| roster.plane(PlaneId(slot.id)))
        .map(member)
        .collect();
    out.extend(
        roster
            .planes()
            .iter()
            .filter(|plane| !slots.iter().any(|slot| slot.id == plane.id.0))
            .map(member),
    );
    out
}

/// The plane that leads each flight, by radio flight number: the AI mission's
/// current leader of the wing (docs/ARCHITECTURE.md, "Lead succession"), or the
/// wing's first member where the AI names none, as with no AI wings.
pub fn leaders(roster: &Roster, members: &[Member], wings: Option<&AiWings>) -> Vec<(u8, u32)> {
    let mut leaders: Vec<(u8, u32, u8)> = Vec::new();
    for member in members {
        match leaders
            .iter_mut()
            .find(|(flight, ..)| *flight == member.flight)
        {
            Some(lead) if member.position < lead.2 => {
                *lead = (member.flight, member.id, member.position)
            }
            Some(_) => {}
            None => leaders.push((member.flight, member.id, member.position)),
        }
    }
    leaders
        .into_iter()
        .map(|(flight, first, _)| {
            let current = members
                .iter()
                .find(|m| m.flight == flight)
                .and_then(|m| roster.plane(PlaneId(m.id)))
                .and_then(|plane| {
                    let side = if plane.slot.wing.side.is_enemy() {
                        crate::ai_wings::ENEMY_SIDE
                    } else {
                        crate::ai_wings::FRIENDLY_SIDE
                    };
                    wings?.mission().wing_leader(side, plane.slot.wing.index)
                });
            (flight, current.unwrap_or(first))
        })
        .collect()
}

/// One fixed tick of radio calls: the releases in `events`, every projectile
/// hit since the last tick, and the AI's radio events. Each call is made
/// once and every seat in `listeners` that hears it queues it.
#[allow(clippy::too_many_arguments)]
pub fn step(
    radio: &mut Radio,
    comms: &mut Comms,
    phrases: &Phrases,
    listeners: &[Listener],
    members: &[Member],
    leaders: &[(u8, u32)],
    events: &[live::Event],
    state: &mut live::State,
    wings: Option<&mut AiWings>,
) {
    let strikes = state.take_strikes();
    let chatter = match wings {
        Some(wings) => {
            // The wing's orders, reports and chatter records join the
            // channel's journal, so the host drains one journal.
            comms.record_all(wings.take_journal());
            std::mem::take(&mut wings.chatter)
        }
        None => Vec::new(),
    };
    let scene = Scene {
        now: state.tick() as f64 / 120.,
        phrases,
        listeners,
        members,
        leaders,
        targets: &state.targets,
        // The first human's side list: only an id that is not a roster
        // member reads it. With no human there is none.
        friendlies: state
            .ownships()
            .first()
            .map_or(&NO_FRIENDLIES, |own| &own.friendlies),
    };
    for event in events {
        if let live::Event::Fired { aircraft, station } = event
            && let Some(own) = state.ownship(*aircraft)
        {
            let weapon = &own.configuration().stations[*station].weapon;
            // The round's own target, else the designation.
            let target = state
                .projectiles
                .iter()
                .rev()
                .find(|p| p.owner == *aircraft && p.station == *station)
                .and_then(|p| p.target)
                .or(own.designated());
            radio.release(comms, &scene, *aircraft, Release::of(weapon, target));
        }
    }
    for strike in &strikes {
        radio.strike(comms, &scene, strike);
    }
    for event in &chatter {
        radio.chatter(comms, &scene, event);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_wings::PLAYER_ID;
    use crate::comms::Elevation;

    fn phrases() -> Phrases {
        [
            ("^CONTACT", "Contact, "),
            ("^PAIROF", "pair of"),
            ("^MULTPLE", "multiple"),
            ("^NUM03", "three"),
            ("^BANDIT", "bandit"),
            ("^BANDITS", "bandits"),
            ("^2SHFORM", ", two-ship formation"),
            ("^YOUR", "your "),
            ("^HIGH", "high"),
            ("^LOW", "low"),
            ("^MILES", " miles"),
            ("^MILE", " mile"),
            ("^PLSADVS", ", please advise"),
            ("^FIRGUN", "I'm using my gun"),
            ("^SPLASH", "Splash one "),
        ]
        .into_iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect()
    }
    fn member(id: u32, enemy: bool, flight: u8, position: u8) -> Member {
        Member {
            id,
            enemy,
            flight,
            position,
            alive: true,
        }
    }
    /// The player (0, Red one), Red two (1), Blue one (2), enemy Black one (3).
    fn members() -> Vec<Member> {
        vec![
            member(0, false, 0, 0),
            member(1, false, 0, 1),
            member(2, false, 1, 0),
            member(3, true, 2, 0),
        ]
    }
    fn target(id: u32, position: [f64; 3]) -> live::Target {
        let mut t = crate::test_support::spawned().remove(0);
        t.id = id;
        t.position = position;
        t.aircraft = Some(AircraftId::Mig29);
        t
    }
    /// A human-flown plane in `flight`, heard at seat `seat`.
    fn listener(seat: u8, plane: u32, flight: u8) -> Listener {
        Listener {
            seat: SeatId(seat),
            plane,
            flight,
            enemy: false,
            alive: true,
            position: [0.; 3],
            crew: None,
        }
    }
    struct World {
        phrases: Phrases,
        listeners: Vec<Listener>,
        leaders: Vec<(u8, u32)>,
        members: Vec<Member>,
        targets: Vec<live::Target>,
        friendlies: BTreeSet<u32>,
    }
    impl World {
        fn new() -> Self {
            Self {
                phrases: phrases(),
                listeners: vec![listener(0, 0, 0)],
                leaders: vec![(0, 0), (1, 2), (2, 3)],
                members: members(),
                targets: vec![
                    target(1, [1000., 0., 0.]),
                    target(2, [52_800., 0., 0.]),
                    target(3, [0., 0., 30_000.]),
                ],
                friendlies: [1, 2].into(),
            }
        }
        fn scene(&self, now: f64) -> Scene<'_> {
            Scene {
                now,
                phrases: &self.phrases,
                listeners: &self.listeners,
                members: &self.members,
                leaders: &self.leaders,
                targets: &self.targets,
                friendlies: &self.friendlies,
            }
        }
    }
    fn strike(owner: u32, victim: u32, flags: u32, destroyed: bool) -> Strike {
        Strike {
            owner,
            victim,
            weapon_flags: flags,
            destroyed,
            amount: 1,
        }
    }
    fn lines(comms: &mut Comms, now: f64) -> Vec<String> {
        comms.due(now).iter().map(|d| d.call.line()).collect()
    }
    const IR: Release = Release {
        flags: 1,
        seeker: 2,
        phoenix: false,
        target: Some(3),
    };

    #[test]
    fn every_listed_recording_is_a_reviewed_stem() {
        let reviewed: BTreeSet<_> = tore_formats::radio::STEMS.iter().map(|(s, _)| *s).collect();
        for stem in GENERIC_LAUNCH
            .iter()
            .chain(&GUIDED_HITS)
            .chain(&UNGUIDED_HITS)
            .chain(&AIRCRAFT_KILLS)
            .chain(&OTHER_KILLS)
            .chain(&HIT_BY_AIRCRAFT)
            .chain(&HIT_BY_AAA)
            .chain(&HIT_BY_OTHER)
            .chain(&DEATHS)
            .chain(&ENGAGE_AIRCRAFT)
            .chain(&FRIENDLY_FIRE)
        {
            assert!(reviewed.contains(stem), "{stem}");
        }
    }

    #[test]
    fn variants_are_the_roll_modulo_the_count() {
        assert_eq!(Comms::pick(13, &UNGUIDED_HITS), "^HOTLEAD");
        assert_eq!(Comms::pick(99, &AIRCRAFT_KILLS), "^IMPACT");
        assert_eq!(Comms::pick(8, &ENGAGE_AIRCRAFT), "^IGOAF");
        assert_eq!(Comms::pick(4, &DEATHS), "^SEEHELL");
        assert_eq!(Comms::pick(4, &DEATHS[..3]), "^OHSH");
        assert_eq!(Comms::pick(10, &FRIENDLY_FIRE), "^YOUNUTS");
    }

    #[test]
    fn launch_calls_follow_the_first_matching_rule() {
        let p = phrases();
        let phoenix = Release {
            phoenix: true,
            seeker: 3,
            ..IR
        };
        assert_eq!(launch_phrase(&p, 99, &phoenix, true).stems, ["^FOXTHR"]);
        let bomb = Release { flags: 0x10, ..IR };
        assert_eq!(launch_phrase(&p, 0, &bomb, false).stems, ["^BOMBAWY"]);
        let radar = Release { seeker: 3, ..IR };
        assert_eq!(launch_phrase(&p, 49, &radar, true).stems, ["^FOXONE"]);
        assert_eq!(launch_phrase(&p, 49, &IR, true).stems, ["^FOXTWO"]);
        // The other half of the roll, or a surface target: one of three.
        assert_eq!(launch_phrase(&p, 50, &IR, true).stems, ["^FIRMISS"]);
        assert_eq!(launch_phrase(&p, 0, &IR, false).stems, ["^IMSHOT"]);
        let gun = Release {
            flags: 0x80,
            seeker: 0,
            ..IR
        };
        let g = launch_phrase(&p, 2, &gun, true);
        assert_eq!(g.stems, ["^FIRGUN", "^FIRMISS"]);
        assert!(g.text.starts_with("I'm using my gun. "));
    }

    #[test]
    fn launch_cooldowns_and_targetless_releases() {
        let w = World::new();
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        let bomb = Release {
            flags: 0x10,
            target: None,
            ..IR
        };
        radio.release(&mut comms, &w.scene(0.), PLAYER_ID, bomb);
        radio.release(&mut comms, &w.scene(3.9), PLAYER_ID, bomb);
        radio.release(&mut comms, &w.scene(4.), PLAYER_ID, bomb);
        assert_eq!(radio.made, 2, "bombs away has a 4 s global cooldown");
        let targetless = Release { target: None, ..IR };
        radio.release(&mut comms, &w.scene(5.), PLAYER_ID, targetless);
        assert_eq!(radio.made, 2, "no target, not a bomb: no call");
        let gun = Release {
            flags: 0x80,
            seeker: 0,
            ..IR
        };
        for t in 0..40 {
            radio.release(&mut comms, &w.scene(10. + f64::from(t) / 10.), 1, gun);
        }
        assert_eq!(radio.made, 3, "one gun call per 4 s");
        // The player's calls play at once; a wingman's after half a second.
        assert_eq!(lines(&mut comms, 10.).len(), 2);
        assert_eq!(lines(&mut comms, 10.5).len(), 1);
    }

    #[test]
    fn listeners_hear_their_own_flight_and_labels_name_the_speaker() {
        let w = World::new();
        let s = w.scene(0.);
        let hears = |s: &Scene, speaker, audience| s.label(&s.listeners[0], speaker, audience);
        assert_eq!(hears(&s, 1, Audience::Flight).as_deref(), Some("Red two"));
        assert_eq!(hears(&s, 1, Audience::Leader).as_deref(), Some("Red two"));
        assert_eq!(hears(&s, 2, Audience::Flight), None, "another flight");
        assert_eq!(hears(&s, 3, Audience::Flight), None, "the enemy");
        assert_eq!(
            hears(&s, 2, Audience::Plane(0)).as_deref(),
            Some("Blue one")
        );
        assert_eq!(hears(&s, 0, Audience::Flight).as_deref(), Some("YOU"));
        let mut down = listener(0, 0, 0);
        down.alive = false;
        let listeners = [down];
        let dead = Scene {
            listeners: &listeners,
            ..w.scene(0.)
        };
        assert_eq!(hears(&dead, 1, Audience::Leader), None);
        // Alone in a two-seat aircraft, the player's own call is the RIO's.
        let alone = vec![member(0, false, 0, 0), member(2, false, 1, 0)];
        let mut rio_listener = listener(0, 0, 0);
        rio_listener.crew = Some(Crew::Rio);
        let listeners = [rio_listener.clone()];
        let rio = Scene {
            listeners: &listeners,
            members: &alone,
            ..w.scene(0.)
        };
        assert_eq!(hears(&rio, 0, Audience::Flight).as_deref(), Some("RIO"));
        rio_listener.crew = Some(Crew::CoPilot);
        let listeners = [rio_listener];
        let led = Scene {
            listeners: &listeners,
            ..w.scene(0.)
        };
        assert_eq!(hears(&led, 0, Audience::Flight).as_deref(), Some("YOU"));
        assert_eq!(label(&member(9, true, 7, 9)), "Yellow ten");
        assert_eq!(label(&member(9, true, 8, 0)), "Flight 9 one");
    }

    #[test]
    fn hit_calls_limit_unguided_hits_per_shooter_and_globally() {
        let w = World::new();
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        // Guided hits: no cooldown.
        for t in 0..3 {
            radio.strike(&mut comms, &w.scene(f64::from(t)), &strike(1, 3, 1, false));
        }
        let guided: Vec<_> = lines(&mut comms, 10.);
        assert!(guided.iter().filter(|l| l.starts_with("Red two")).count() == 3);
        // Unguided: Red two at 20 s, then its own 8 s limit; the player at
        // 21 s is inside the 4 s global cooldown and says nothing.
        let mut radio = Radio::default();
        let gun = strike(1, 3, 0x80, false);
        radio.strike(&mut comms, &w.scene(20.), &gun);
        radio.strike(
            &mut comms,
            &w.scene(21.),
            &strike(PLAYER_ID, 3, 0x80, false),
        );
        radio.strike(&mut comms, &w.scene(25.), &gun);
        radio.strike(&mut comms, &w.scene(28.), &gun);
        let calls = lines(&mut comms, 30.);
        assert_eq!(calls.len(), 2, "{calls:?}");
        // The enemy's "I'm hit" is unheard but still limited per aircraft.
        assert_eq!(radio.made, 2 + 2);
    }

    #[test]
    fn kill_calls_name_aircraft_and_limit_other_kills_after_bombs() {
        let p = phrases();
        let s = splash(&p, AircraftId::Mig29);
        assert_eq!(s.text, "Splash one MiG-29 Fulcrum-C");
        assert_eq!(s.stems, ["^SPLASH", "^ACMIG29"]);
        assert_eq!(splash(&p, AircraftId::Rafale).stems, ["^SPLASH"]);
        assert!(always_generic(AircraftId::F22) && !always_generic(AircraftId::F22n));
        let mut w = World::new();
        w.targets[2].role = TargetRole::Surface;
        w.targets[2].aircraft = None;
        w.members.pop();
        w.friendlies.clear();
        w.friendlies.extend([1, 2]);
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        let kill = |flags| strike(PLAYER_ID, 3, flags, true);
        radio.strike(&mut comms, &w.scene(0.), &kill(1));
        radio.strike(&mut comms, &w.scene(0.1), &kill(0x10));
        radio.strike(&mut comms, &w.scene(3.9), &kill(1));
        radio.strike(&mut comms, &w.scene(4.1), &kill(1));
        assert_eq!(radio.made, 3, "only the bomb kill starts the cooldown");
    }

    #[test]
    fn im_hit_depends_on_the_attacker_and_limits_gun_rounds() {
        let w = World::new();
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        let gun = strike(3, PLAYER_ID, 0x80, false);
        radio.strike(&mut comms, &w.scene(0.), &gun);
        radio.strike(&mut comms, &w.scene(7.9), &gun);
        radio.strike(&mut comms, &w.scene(8.), &strike(3, PLAYER_ID, 1, false));
        let calls = lines(&mut comms, 9.);
        let hit: Vec<_> = calls.iter().filter(|l| l.starts_with("YOU")).collect();
        assert_eq!(hit.len(), 2, "{calls:?}");
        assert_eq!(w.scene(0.).attacker(3), Attacker::Aircraft);
    }

    #[test]
    fn friendly_fire_reaches_the_player_within_ten_miles() {
        let mut w = World::new();
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        radio.strike(&mut comms, &w.scene(0.), &strike(PLAYER_ID, 2, 0x80, false));
        radio.strike(
            &mut comms,
            &w.scene(5.9),
            &strike(PLAYER_ID, 1, 0x80, false),
        );
        assert!(lines(&mut comms, 1.9).is_empty(), "two seconds later");
        let complaint = lines(&mut comms, 2.);
        assert_eq!(complaint.len(), 1);
        assert!(complaint[0].starts_with("Blue one: "), "{complaint:?}");
        assert!(lines(&mut comms, 9.).is_empty(), "6 s global cooldown");
        w.targets[1].position[0] = FRIENDLY_FIRE_FT + 1.;
        radio.strike(
            &mut comms,
            &w.scene(20.),
            &strike(PLAYER_ID, 2, 0x80, false),
        );
        assert!(lines(&mut comms, 30.).is_empty(), "beyond 52,800 ft");
        // AI-on-AI friendly fire is never complained about.
        radio.strike(&mut comms, &w.scene(40.), &strike(1, 2, 0x80, false));
        assert!(lines(&mut comms, 50.).is_empty());
    }

    #[test]
    fn contact_reports_compose_size_noun_clock_miles_and_advice() {
        let p = phrases();
        let mut c = Contact {
            count: 2,
            named: None,
            hour: 2,
            elevation: Elevation::High,
            miles: 12,
            advise: true,
            views: Vec::new(),
        };
        let report = contact_phrase(&p, 39, &c);
        assert_eq!(
            report.text,
            "Contact, pair of bandits, your two o'clock high, 12 miles, please advise"
        );
        assert_eq!(
            report.stems,
            [
                "^CONTACT", "^PAIROF", "^BANDITS", "^YOUR", "^CLOCK02", "^HIGH", "^NUM12",
                "^MILES", "^PLSADVS"
            ]
        );
        assert!(
            contact_phrase(&p, 40, &c)
                .text
                .contains("bandits, two-ship formation, ")
        );
        assert!(contact_phrase(&p, 70, &c).text.contains("multiple bandits"));
        c.count = 3;
        assert!(contact_phrase(&p, 49, &c).text.contains("three bandits"));
        assert!(contact_phrase(&p, 50, &c).text.contains("multiple bandits"));
        c.count = 13;
        assert!(contact_phrase(&p, 0, &c).text.contains("multiple"));
        c.count = 1;
        c.named = Some("MiG-21".into());
        c.advise = false;
        let named = contact_phrase(&p, 0, &c);
        assert_eq!(
            named.text,
            "Contact, MiG-21, your two o'clock high, 12 miles"
        );
        assert!(named.stems.contains(&"^MIG21".to_string()));
        c.named = Some("MiG-29".into());
        c.count = 2;
        let named = contact_phrase(&p, 0, &c);
        assert!(named.text.contains("pair of MiG-29s"));
        assert!(named.stems.contains(&"^BANDITS".to_string()));
        // Beyond 15 miles: no size or noun.
        c.miles = 16;
        c.elevation = Elevation::Level;
        assert_eq!(
            contact_phrase(&p, 0, &c).text,
            "Contact, your two o'clock, 16 miles"
        );
    }

    #[test]
    fn ai_events_use_their_audience_delay_and_kind() {
        let w = World::new();
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        let s = w.scene(0.);
        radio.chatter(
            &mut comms,
            &s,
            &Chatter::Engage {
                speaker: 1,
                aircraft: false,
            },
        );
        radio.chatter(&mut comms, &s, &Chatter::Showtime { speaker: 1 });
        radio.chatter(
            &mut comms,
            &s,
            &Chatter::Death {
                speaker: 1,
                ejection_seat: true,
            },
        );
        radio.chatter(
            &mut comms,
            &s,
            &Chatter::Fuel {
                speaker: 2,
                level: FuelLevel::Bingo,
            },
        );
        radio.chatter(
            &mut comms,
            &s,
            &Chatter::LaunchWarning {
                speaker: 1,
                by_aircraft: false,
            },
        );
        assert_eq!(radio.made, 5);
        assert_eq!(
            radio.heard, 4,
            "Blue one's fuel call goes to its own leader"
        );
        assert_eq!(comms.due(0.).len(), 1, "the death call has no delay");
        assert_eq!(comms.due(0.5).len(), 1, "SAM launch after half a second");
        let replies = comms.due(2.);
        assert_eq!(replies.len(), 2, "engage and showtime after two seconds");
        assert_eq!(replies[0].call.stems, ["^ENGAGE"]);
        assert_eq!(replies[0].call.kind, Kind::Chatter);
        assert_eq!(replies[1].call.kind, Kind::Important);
        // Radio silence drops the engage reply but never "Showtime!".
        comms.toggle_silence(SeatId(0));
        radio.chatter(
            &mut comms,
            &s,
            &Chatter::Engage {
                speaker: 1,
                aircraft: true,
            },
        );
        radio.chatter(&mut comms, &s, &Chatter::Showtime { speaker: 1 });
        assert_eq!(comms.due(5.).len(), 1);
    }

    #[test]
    fn unheard_calls_are_journaled_with_the_listener_rule() {
        let w = World::new();
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        radio.release(&mut comms, &w.scene(0.), 2, IR);
        let at_red_two = Release {
            target: Some(1),
            ..IR
        };
        radio.release(&mut comms, &w.scene(0.), 3, at_red_two);
        let mut lost = listener(0, 0, 0);
        lost.alive = false;
        let listeners = [lost];
        let down = Scene {
            listeners: &listeners,
            ..w.scene(0.)
        };
        radio.release(&mut comms, &down, 1, IR);
        let journal = comms.take_journal();
        let unheard: Vec<_> = journal
            .iter()
            .map(|e| (e.label.as_str(), e.outcome.clone()))
            .collect();
        assert_eq!(
            unheard,
            [
                ("Blue one", Outcome::Unheard(Reason::OtherFlight)),
                ("Green one", Outcome::Unheard(Reason::EnemyFlight)),
                ("Red two", Outcome::Unheard(Reason::PlayerDown)),
            ]
        );
        assert_eq!((radio.made, radio.heard), (3, 0), "the counters agree");
        let blue = &journal[0];
        assert_eq!(blue.origin.speaker, Some(2));
        assert_eq!(blue.origin.audience, journal::Audience::Flight);
        assert_eq!(
            blue.origin.cause,
            Cause::Release {
                target: Some(3),
                store: Store {
                    flags: 1,
                    seeker: 2,
                    phoenix: false
                }
            }
        );
        assert_eq!(
            blue.origin.rolls[0].test,
            Test::Below(50),
            "an infrared shot at an aircraft: the Fox two chance"
        );
        assert_eq!(
            blue.origin.cause.to_string(),
            "infrared missile release at aircraft 3"
        );
    }

    #[test]
    fn hit_limits_are_journaled_once_per_window_with_the_time_left() {
        let w = World::new();
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        let gun = strike(1, 3, 0x80, false);
        radio.strike(&mut comms, &w.scene(20.), &gun);
        radio.strike(
            &mut comms,
            &w.scene(21.),
            &strike(PLAYER_ID, 3, 0x80, false),
        );
        radio.strike(&mut comms, &w.scene(22.), &gun);
        radio.strike(&mut comms, &w.scene(23.), &gun);
        let held: Vec<_> = comms
            .take_journal()
            .into_iter()
            .filter(|e| matches!(e.outcome, Outcome::Suppressed(_)))
            .map(|e| (e.label, e.outcome, e.origin.rolls))
            .collect();
        assert_eq!(
            held,
            [
                (
                    "YOU".to_string(),
                    Outcome::Suppressed(Reason::Cooldown {
                        key: UNGUIDED_HIT,
                        seconds: 4.,
                        remaining: 3.
                    }),
                    vec![Roll::new("hit call", held[0].2[0].value, Test::Unused)]
                ),
                (
                    "Green one".to_string(),
                    Outcome::Suppressed(Reason::PerAircraft {
                        rule: "\"I'm hit\" from gun rounds per aircraft",
                        id: 3,
                        seconds: 8.,
                        remaining: 7.
                    }),
                    vec![]
                ),
                (
                    "Red two".to_string(),
                    Outcome::Suppressed(Reason::PerAircraft {
                        rule: "unguided hits per shooter",
                        id: 1,
                        seconds: 8.,
                        remaining: 6.
                    }),
                    vec![]
                ),
            ],
            "the roll a held hit call still draws is listed; repeats in a window are not"
        );
    }

    #[test]
    fn kill_calls_journal_the_splash_roll_and_its_threshold() {
        let w = World::new();
        let mut splashes = 0;
        for seed in 0..60 {
            let mut comms = Comms::new(seed);
            let mut radio = Radio::default();
            radio.strike(&mut comms, &w.scene(0.), &strike(PLAYER_ID, 3, 1, true));
            let entry = comms.take_journal().remove(0);
            let rolls = &entry.origin.rolls;
            assert_eq!(rolls.len(), 2, "a named type draws a second roll");
            assert_eq!(rolls[1].test, Test::AtLeast(40));
            let splash = rolls[1].value >= 40;
            splashes += usize::from(splash);
            assert_eq!(entry.stems[0] == "^SPLASH", splash);
            assert_eq!(
                rolls[0].test,
                if splash {
                    Test::Unused
                } else {
                    Test::Modulo(AIRCRAFT_KILLS.len() as u32)
                }
            );
            assert_eq!(
                entry.origin.cause,
                Cause::Kill {
                    victim: 3,
                    aircraft: true,
                    bomb: false
                }
            );
        }
        assert!((20..60).contains(&splashes), "{splashes}");
    }

    #[test]
    fn each_side_hears_only_its_own_flights_and_a_complaint_goes_to_the_shooter() {
        // Seat 0 flies Red one; seat 1 flies plane 5, the enemy Black two.
        let mut w = World::new();
        w.members.push(member(5, true, 2, 1));
        let mut enemy = listener(1, 5, 2);
        enemy.enemy = true;
        w.listeners.push(enemy);
        let mut comms = Comms::with_seats(1, [SeatId(0), SeatId(1)]);
        let mut radio = Radio::default();
        let s = w.scene(0.);
        // Red two's call goes to seat 0 alone, Black one's to seat 1 alone.
        radio.release(&mut comms, &s, 1, IR);
        radio.release(
            &mut comms,
            &s,
            3,
            Release {
                target: Some(1),
                ..IR
            },
        );
        let mut seats = Vec::new();
        for delivery in comms.due(1.) {
            seats.push((delivery.seat, delivery.call.label));
        }
        assert_eq!(
            seats,
            [
                (SeatId(0), "Red two".to_string()),
                (SeatId(1), "Green one".to_string())
            ],
            "the other side's flight is never heard"
        );
        assert_eq!((radio.made, radio.heard), (2, 2));
        // A friendly-fire complaint is addressed to the shooter's plane.
        radio.strike(&mut comms, &s, &strike(5, 3, 0x80, false));
        let complaints = comms.due(9.);
        assert_eq!(complaints.len(), 1);
        assert_eq!(complaints[0].seat, SeatId(1));
        assert_eq!(complaints[0].call.label, "Green one");
        // Each seat's busy hold is its own: only seat 1 heard the complaint.
        assert!(!comms.channel_free(SeatId(1), 10.));
        assert!(comms.channel_free(SeatId(0), 10.));
    }

    #[test]
    fn a_human_new_leader_hears_the_wingleader_call_after_five_seconds() {
        // Seat 1 flies plane 5, a Red wingman; Red two (1) led and is gone.
        let mut w = World::new();
        w.members.push(member(5, false, 0, 2));
        w.listeners.push(listener(1, 5, 0));
        let mut comms = Comms::with_seats(1, [SeatId(0), SeatId(1)]);
        let mut radio = Radio::default();
        let change = |leader, alive| Chatter::Leadership {
            speaker: 1,
            side: tore_sim::ai::launch::Side::Friendly,
            wing_number: 1,
            leader,
            previous_pilot_alive: alive,
        };
        let mut p = phrases();
        p.insert("^WNGLDR".into(), "You're the Wingleader now".into());
        w.phrases = p;
        // The previous leader is alive: its voice, five seconds later, to seat 1.
        radio.chatter(&mut comms, &w.scene(10.), &change(5, true));
        assert!(comms.due(14.9).is_empty());
        let due = comms.due(15.);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].seat, SeatId(1));
        assert_eq!(due[0].call.label, "Red two");
        assert_eq!(due[0].call.stems, ["^WNGLDR"]);
        assert_eq!(due[0].call.kind, Kind::Important);
        // Its pilot is gone: the words print, with no recording.
        radio.chatter(&mut comms, &w.scene(20.), &change(5, false));
        let due = comms.due(25.);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].seat, SeatId(1));
        assert_eq!(due[0].call.line(), "Flight: 'You're the Wingleader now'");
        assert!(due[0].call.stems.is_empty());
        // The AI taking the lead is a note in the journal and nothing else.
        comms.take_journal();
        radio.chatter(&mut comms, &w.scene(30.), &change(2, true));
        assert!(comms.due(40.).is_empty());
        let journal = comms.take_journal();
        assert_eq!(journal.len(), 1, "{journal:?}");
        assert_eq!(journal[0].outcome, Outcome::Noted);
    }

    #[test]
    fn each_seat_hears_a_contact_report_from_its_own_position() {
        use crate::ai_wings::ContactView;
        let mut w = World::new();
        w.members.push(member(5, false, 0, 2));
        w.listeners.push(listener(1, 5, 0));
        let view = |plane, hour, miles| ContactView {
            plane,
            named: None,
            hour,
            elevation: Elevation::Level,
            miles,
        };
        let contact = Contact {
            views: vec![view(0, 12, 20), view(5, 3, 8)],
            count: 1,
            named: None,
            hour: 12,
            elevation: Elevation::Level,
            miles: 20,
            advise: false,
        };
        let mut comms = Comms::with_seats(1, [SeatId(0), SeatId(1)]);
        let mut radio = Radio::default();
        radio.chatter(
            &mut comms,
            &w.scene(0.),
            &Chatter::Contact {
                speaker: 1,
                target: 3,
                contact,
            },
        );
        let due = comms.due(1.);
        assert_eq!(due.len(), 2);
        assert_eq!(due[0].call.text, "Contact, your twelve o'clock, 20 miles");
        assert_eq!(
            due[1].call.text, "Contact, bandit, your three o'clock, 8 miles",
            "inside 15 miles the size words are said"
        );
        assert_eq!(due[0].call.stems[0], due[1].call.stems[0], "one call");
        assert_eq!(
            comms
                .take_journal()
                .iter()
                .filter(|e| e.call == Some(1))
                .count(),
            2
        );
    }

    // The battle net (stage G, slice G8).

    /// A Blue flight of its own, with a human in Blue two (seat 1, plane 5)
    /// and an enemy human (seat 2, plane 6), beside the Red player (seat 0).
    fn net_world() -> World {
        let mut w = World::new();
        w.members.push(member(5, false, 1, 1));
        w.members.push(member(6, true, 2, 1));
        w.listeners.push(listener(1, 5, 1));
        let mut enemy = listener(2, 6, 2);
        enemy.enemy = true;
        w.listeners.push(enemy);
        w
    }
    fn net_contact() -> Contact {
        use crate::ai_wings::ContactView;
        let view = |plane, hour, miles| ContactView {
            plane,
            named: None,
            hour,
            elevation: Elevation::High,
            miles,
        };
        Contact {
            views: vec![view(0, 12, 20), view(5, 2, 12), view(6, 6, 9)],
            count: 1,
            named: None,
            hour: 2,
            elevation: Elevation::High,
            miles: 12,
            advise: false,
        }
    }
    fn report(radio: &mut Radio, comms: &mut Comms, w: &World, speaker: u32) {
        radio.chatter(
            comms,
            &w.scene(0.),
            &Chatter::Contact {
                speaker,
                target: 3,
                contact: net_contact(),
            },
        );
    }
    const SEATS: [SeatId; 3] = [SeatId(0), SeatId(1), SeatId(2)];

    #[test]
    fn a_leads_contact_report_reaches_the_monitors_of_its_side_in_other_flights() {
        let w = net_world();
        let mut comms = Comms::with_seats(1, SEATS);
        let mut radio = Radio::default();
        // Nobody monitors: the report is the flight's alone, as it always was.
        report(&mut radio, &mut comms, &w, 2);
        let due = comms.due(1.);
        assert_eq!(
            due.iter().map(|d| (d.seat, d.call.net)).collect::<Vec<_>>(),
            [(SeatId(1), comms::Net::Wing)]
        );
        assert_eq!(
            due[0].call.line(),
            "Blue one: 'Contact, bandit, your two o'clock high, 12 miles'"
        );
        let plain = comms.take_journal();
        assert!(plain.iter().all(|e| e.heard_by == [SeatId(1)]));
        // Red one monitors, and so does the enemy human, who is on the other
        // side and hears nothing of it.
        assert_eq!(comms.toggle_battle(SeatId(0)), "Monitoring battle net");
        comms.toggle_battle(SeatId(2));
        report(&mut radio, &mut comms, &w, 2);
        let due = comms.due(5.);
        assert_eq!(
            due.iter().map(|d| (d.seat, d.call.net)).collect::<Vec<_>>(),
            [
                (SeatId(0), comms::Net::Battle),
                (SeatId(1), comms::Net::Wing)
            ]
        );
        // The monitor hears the speaking flight's colour, then the report
        // from where its own aircraft is, under the net label.
        assert_eq!(
            due[0].call.line(),
            "Net Blue one: 'Blue, Contact, your twelve o'clock high, 20 miles'"
        );
        assert_eq!(due[0].call.stems[0], "^BLUE");
        assert_eq!(due[0].call.kind, Kind::Chatter);
        assert_eq!(
            due[1].call.line(),
            "Blue one: 'Contact, bandit, your two o'clock high, 12 miles'",
            "the flight hears it as it always did"
        );
        // One call: one number, one entry per step, listing every seat, and
        // it reads as the flight's.
        let journal = comms.take_journal();
        let entries: Vec<_> = journal.iter().filter(|e| e.call.is_some()).collect();
        assert_eq!(entries.len(), 2);
        for entry in entries {
            assert_eq!(entry.net, comms::Net::Wing);
            assert_eq!(entry.label, "Blue one");
            let mut seats = entry.heard_by.clone();
            seats.sort();
            assert_eq!(seats, [SeatId(0), SeatId(1)]);
        }
        assert_eq!((radio.made, radio.heard), (2, 2));
    }

    #[test]
    fn only_a_flights_lead_is_repeated_and_never_to_its_own_flight_or_the_dead() {
        let mut w = net_world();
        let mut comms = Comms::with_seats(1, SEATS);
        let mut radio = Radio::default();
        comms.toggle_battle(SeatId(0));
        comms.toggle_battle(SeatId(1));
        // Blue three is not Blue's lead: only its flight hears it, and a
        // monitor in the flight hears it once, as the flight does.
        w.members.push(member(7, false, 1, 2));
        report(&mut radio, &mut comms, &w, 7);
        let due = comms.due(1.);
        assert_eq!(
            due.iter().map(|d| (d.seat, d.call.net)).collect::<Vec<_>>(),
            [(SeatId(1), comms::Net::Wing)],
            "Red one monitors and does not hear a wingman"
        );
        // Blue one leads: Red one hears it over the net, Blue two (a
        // monitor, in the flight) hears it as the flight does.
        report(&mut radio, &mut comms, &w, 2);
        let due = comms.due(3.);
        assert_eq!(
            due.iter().map(|d| (d.seat, d.call.net)).collect::<Vec<_>>(),
            [
                (SeatId(0), comms::Net::Battle),
                (SeatId(1), comms::Net::Wing)
            ]
        );
        // A dead monitor hears nothing of it.
        w.listeners[0].alive = false;
        report(&mut radio, &mut comms, &w, 2);
        let due = comms.due(7.);
        assert_eq!(due.iter().map(|d| d.seat).collect::<Vec<_>>(), [SeatId(1)]);
    }

    #[test]
    fn a_call_only_the_battle_net_hears_is_that_nets_and_one_nobody_hears_is_as_before() {
        // Red's player alone: Blue's lead is another flight.
        let w = World::new();
        let mut comms = Comms::new(1);
        let mut radio = Radio::default();
        report(&mut radio, &mut comms, &w, 2);
        let journal = comms.take_journal();
        assert_eq!(journal.len(), 1);
        assert_eq!(journal[0].outcome, Outcome::Unheard(Reason::OtherFlight));
        assert_eq!(journal[0].net, comms::Net::Wing, "journaled as it was");
        assert_eq!((radio.made, radio.heard), (1, 0));
        // Monitoring, Red one hears it.
        comms.toggle_battle(SeatId(0));
        report(&mut radio, &mut comms, &w, 2);
        let due = comms.due(1.);
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].call.label, "Net Blue one");
        assert_eq!(due[0].call.net, comms::Net::Battle);
        let journal = comms.take_journal();
        assert!(!journal.is_empty());
        for entry in &journal {
            assert_eq!(entry.net, comms::Net::Battle);
            assert_eq!(entry.label, "Net Blue one");
            assert_eq!(entry.heard_by, [SeatId(0)]);
        }
        assert_eq!((radio.made, radio.heard), (2, 1));
    }

    #[test]
    fn enemy_leads_reach_only_the_enemy_monitors() {
        let w = net_world();
        let mut comms = Comms::with_seats(1, SEATS);
        let mut radio = Radio::default();
        for seat in SEATS {
            comms.toggle_battle(seat);
        }
        // Black one (plane 3, the enemy flight's lead) is heard by the enemy
        // human in its own flight, as the flight; no friendly seat hears it.
        report(&mut radio, &mut comms, &w, 3);
        let due = comms.due(1.);
        assert_eq!(due.iter().map(|d| d.seat).collect::<Vec<_>>(), [SeatId(2)]);
        assert_eq!(due[0].call.net, comms::Net::Wing);
    }

    #[test]
    fn radio_silence_drops_a_contact_report_on_the_net_and_an_attack_call_comes_through() {
        let w = net_world();
        let mut comms = Comms::with_seats(1, SEATS);
        let mut radio = Radio::default();
        comms.toggle_battle(SeatId(0));
        comms.toggle_silence(SeatId(0));
        report(&mut radio, &mut comms, &w, 2);
        assert_eq!(
            comms.due(1.).iter().map(|d| d.seat).collect::<Vec<_>>(),
            [SeatId(1)]
        );
        // The assignment call is important: silence never drops it.
        let words = Phrase::default().raw("Two, attack bandit", Some("^ATTACK"));
        let hearers = battle_hearers(&comms, &w.members, &w.leaders, &w.listeners, 2, &|_| {
            words.clone()
        });
        assert_eq!(hearers.len(), 1);
        assert_eq!(hearers[0].seat, SeatId(0));
        comms.send(2., Call::new("Blue one", words, Kind::Important), &hearers);
        let due = comms.due(2.);
        assert_eq!(due.len(), 1);
        assert_eq!(
            due[0].call.line(),
            "Net Blue one: 'Blue, Two, attack bandit'"
        );
        assert_eq!(due[0].call.stems, ["^BLUE", "^ATTACK"]);
    }

    #[test]
    fn the_battle_net_names_its_flight_by_colour_and_past_the_eighth_by_number() {
        let words = || Phrase::default().raw("Contact", Some("^CONTACT"));
        let red = battle_words(0, words());
        assert_eq!(
            (red.text.as_str(), red.stems.as_slice()),
            (
                "Red, Contact",
                &["^RED".to_string(), "^CONTACT".to_string()][..]
            )
        );
        // Orange has the word and no recording.
        let orange = battle_words(5, words());
        assert_eq!(orange.text, "Orange, Contact");
        assert_eq!(orange.stems, ["^CONTACT"]);
        let ninth = battle_words(8, words());
        assert_eq!(ninth.text, "Flight 9, Contact");
        // A call to the whole flight already begins with the colour.
        let to_flight = Phrase::default()
            .raw("Blue", Some("^BLUE"))
            .raw(", attack bandit", Some("^ATTACK"));
        assert_eq!(battle_words(1, to_flight.clone()), to_flight);
        let to_ninth = Phrase::default().raw("Flight 9, attack bandit", Some("^ATTACK"));
        assert_eq!(battle_words(8, to_ninth.clone()), to_ninth);
        assert_eq!(
            battle_words(0, to_flight.clone()).stems[0],
            "^RED",
            "another flight's name is not this flight's"
        );
        assert_eq!(battle_label(&member(2, false, 1, 0)), "Net Blue one");
        assert_eq!(battle_label(&member(2, false, 8, 3)), "Net Flight 9 four");
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "radio_calls_checkpoint.rs"]
mod checkpoint;
