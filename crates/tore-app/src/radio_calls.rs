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

use crate::ai_wings::{AiWings, Chatter, Contact, FuelLevel, Member, PLAYER_ID};
use crate::comms::journal::{
    self, Cause, Entry, Origin, Outcome, REPEAT_S, Reason, Roll, Source, Store, Test, WingReply,
};
use crate::comms::{self, Call, Comms, Crew, Kind, Phrase, Phrases, Route};

/// Flight colours, first flight first (spec-derived).
pub const FLIGHTS: [&str; 8] = [
    "Red", "Blue", "Green", "Black", "White", "Orange", "Purple", "Yellow",
];
const POSITIONS: [&str; 12] = [
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
    /// The player alone.
    Player,
}
impl From<Audience> for journal::Audience {
    fn from(audience: Audience) -> Self {
        match audience {
            Audience::Leader => Self::Leader,
            Audience::Flight => Self::Flight,
            Audience::Player => Self::Player,
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

/// The world as the radio sees it for one tick.
pub struct Scene<'a> {
    pub now: f64,
    pub phrases: &'a Phrases,
    /// The player's crew voice, in a multi-crew aircraft.
    pub crew: Option<Crew>,
    pub player_alive: bool,
    pub player_position: [f64; 3],
    pub members: &'a [Member],
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
    fn enemy(&self, id: u32) -> bool {
        if id == PLAYER_ID {
            false
        } else if let Some(member) = self.member(id) {
            member.enemy
        } else {
            !self.friendlies.contains(&id)
        }
    }
    fn aircraft(&self, id: u32) -> bool {
        id == PLAYER_ID
            || self.member(id).is_some()
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
    fn in_player_flight(&self, member: &Member) -> bool {
        !member.enemy && member.flight == 0
    }
    /// The listener rule: the label the player hears, or `None` when the
    /// player is not a receiver. The player's own calls are always heard.
    fn label(&self, speaker: u32, audience: Audience) -> Option<String> {
        if speaker == PLAYER_ID {
            // A call that ends up addressed to the player's own aircraft uses
            // the crew label in a multi-crew aircraft.
            let alone = !self
                .members
                .iter()
                .any(|m| self.in_player_flight(m) && m.alive);
            let to_self = audience != Audience::Flight || alone;
            return Some(match (to_self, self.crew) {
                (true, Some(crew)) => crew.label().to_string(),
                _ => "YOU".to_string(),
            });
        }
        let member = self.member(speaker)?;
        let heard = match audience {
            Audience::Player => true,
            Audience::Flight | Audience::Leader => {
                self.player_alive && self.in_player_flight(member)
            }
        };
        heard.then(|| label(member))
    }
    /// Why [`Self::label`] found the player is not a receiver. Journal only.
    fn unheard(&self, speaker: u32) -> Reason {
        match self.member(speaker) {
            None => Reason::NoRadioIdentity,
            Some(member) if member.enemy => Reason::EnemyFlight,
            Some(member) if !self.in_player_flight(member) => Reason::OtherFlight,
            Some(_) => Reason::PlayerDown,
        }
    }
    /// The speaker's radio name whether or not the player hears it, for the
    /// journal.
    fn name(&self, speaker: u32) -> String {
        if speaker == PLAYER_ID {
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
    /// Make a call. The listener rule decides whether the player hears it;
    /// an unheard call is journaled and goes no further.
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
        self.made += 1;
        let origin = origin.by(speaker).to(audience.into());
        let Some(label) = scene.label(speaker, audience) else {
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
        // The player's own calls are voiced when they are sent.
        let delay = if speaker == PLAYER_ID { 0. } else { delay };
        comms.send(
            scene.now,
            Call::new(label, phrase, kind).after(delay).because(origin),
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
        let victim = strike.victim.unwrap_or(PLAYER_ID);
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
        } else if shooter == PLAYER_ID && victim != PLAYER_ID && scene.aircraft(victim) {
            let Some(range_ft) = scene
                .target(victim)
                .map(|t| missiles::length(missiles::sub(t.position, scene.player_position)))
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
                        Audience::Player,
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
            victim: strike.victim.unwrap_or(PLAYER_ID),
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
                self.say(
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

/// One fixed tick of radio calls: the player's releases in `events`, every
/// projectile hit since the last tick, and the AI's radio events.
#[allow(clippy::too_many_arguments)]
pub fn step(
    radio: &mut Radio,
    comms: &mut Comms,
    phrases: &Phrases,
    crew: Option<Crew>,
    events: &[live::Event],
    state: &mut live::State,
    wings: Option<&mut AiWings>,
    player: &crate::flight::State,
) {
    let strikes = state.take_strikes();
    let (members, chatter) = match wings {
        Some(wings) => {
            // The wing's orders, reports and chatter records join the
            // channel's journal, so the host drains one journal.
            comms.record_all(wings.take_journal());
            (wings.radio_members(), std::mem::take(&mut wings.chatter))
        }
        None => (Vec::new(), Vec::new()),
    };
    let scene = Scene {
        now: state.tick() as f64 / 120.,
        phrases,
        crew,
        player_alive: state.player_hp > 0 && !player.crashed,
        player_position: player.position,
        members: &members,
        targets: &state.targets,
        friendlies: &state.friendlies,
    };
    for event in events {
        if let live::Event::Fired(station) = event {
            let weapon = &state.configuration().stations[*station].weapon;
            // The round's own target, else the designation.
            let target = state
                .projectiles
                .iter()
                .rev()
                .find(|p| p.owner == live::PLAYER_OWNER && p.station == *station)
                .and_then(|p| p.target)
                .or(state.designated());
            radio.release(comms, &scene, PLAYER_ID, Release::of(weapon, target));
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
    /// Red two (1), Blue one (2), enemy Black one (3).
    fn members() -> Vec<Member> {
        vec![
            member(1, false, 0, 1),
            member(2, false, 1, 0),
            member(3, true, 2, 0),
        ]
    }
    fn target(id: u32, position: [f64; 3]) -> live::Target {
        let mut t = crate::ai_wings::tests::spawned().remove(0);
        t.id = id;
        t.position = position;
        t.aircraft = Some(AircraftId::Mig29);
        t
    }
    struct World {
        phrases: Phrases,
        members: Vec<Member>,
        targets: Vec<live::Target>,
        friendlies: BTreeSet<u32>,
    }
    impl World {
        fn new() -> Self {
            Self {
                phrases: phrases(),
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
                crew: None,
                player_alive: true,
                player_position: [0.; 3],
                members: &self.members,
                targets: &self.targets,
                friendlies: &self.friendlies,
            }
        }
    }
    fn strike(owner: u32, victim: Option<u32>, flags: u32, destroyed: bool) -> Strike {
        Strike {
            owner,
            victim,
            weapon_flags: flags,
            destroyed,
        }
    }
    fn lines(comms: &mut Comms, now: f64) -> Vec<String> {
        comms.due(now).iter().map(Call::line).collect()
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
        assert_eq!(s.label(1, Audience::Flight).as_deref(), Some("Red two"));
        assert_eq!(s.label(1, Audience::Leader).as_deref(), Some("Red two"));
        assert_eq!(s.label(2, Audience::Flight), None, "another flight");
        assert_eq!(s.label(3, Audience::Flight), None, "the enemy");
        assert_eq!(s.label(2, Audience::Player).as_deref(), Some("Blue one"));
        assert_eq!(s.label(PLAYER_ID, Audience::Flight).as_deref(), Some("YOU"));
        let dead = Scene {
            player_alive: false,
            ..w.scene(0.)
        };
        assert_eq!(dead.label(1, Audience::Leader), None);
        // Alone in a two-seat aircraft, the player's own call is the RIO's.
        let alone = vec![member(2, false, 1, 0)];
        let rio = Scene {
            crew: Some(Crew::Rio),
            members: &alone,
            ..w.scene(0.)
        };
        assert_eq!(
            rio.label(PLAYER_ID, Audience::Flight).as_deref(),
            Some("RIO")
        );
        let led = Scene {
            crew: Some(Crew::CoPilot),
            ..w.scene(0.)
        };
        assert_eq!(
            led.label(PLAYER_ID, Audience::Flight).as_deref(),
            Some("YOU")
        );
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
            radio.strike(
                &mut comms,
                &w.scene(f64::from(t)),
                &strike(1, Some(3), 1, false),
            );
        }
        let guided: Vec<_> = lines(&mut comms, 10.);
        assert!(guided.iter().filter(|l| l.starts_with("Red two")).count() == 3);
        // Unguided: Red two at 20 s, then its own 8 s limit; the player at
        // 21 s is inside the 4 s global cooldown and says nothing.
        let mut radio = Radio::default();
        let gun = strike(1, Some(3), 0x80, false);
        radio.strike(&mut comms, &w.scene(20.), &gun);
        radio.strike(
            &mut comms,
            &w.scene(21.),
            &strike(PLAYER_ID, Some(3), 0x80, false),
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
        let kill = |flags| strike(PLAYER_ID, Some(3), flags, true);
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
        let gun = strike(3, None, 0x80, false);
        radio.strike(&mut comms, &w.scene(0.), &gun);
        radio.strike(&mut comms, &w.scene(7.9), &gun);
        radio.strike(&mut comms, &w.scene(8.), &strike(3, None, 1, false));
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
        radio.strike(
            &mut comms,
            &w.scene(0.),
            &strike(PLAYER_ID, Some(2), 0x80, false),
        );
        radio.strike(
            &mut comms,
            &w.scene(5.9),
            &strike(PLAYER_ID, Some(1), 0x80, false),
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
            &strike(PLAYER_ID, Some(2), 0x80, false),
        );
        assert!(lines(&mut comms, 30.).is_empty(), "beyond 52,800 ft");
        // AI-on-AI friendly fire is never complained about.
        radio.strike(&mut comms, &w.scene(40.), &strike(1, Some(2), 0x80, false));
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
        assert_eq!(replies[0].stems, ["^ENGAGE"]);
        assert_eq!(replies[0].kind, Kind::Chatter);
        assert_eq!(replies[1].kind, Kind::Important);
        // Radio silence drops the engage reply but never "Showtime!".
        comms.toggle_silence();
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
        let down = Scene {
            player_alive: false,
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
        let gun = strike(1, Some(3), 0x80, false);
        radio.strike(&mut comms, &w.scene(20.), &gun);
        radio.strike(
            &mut comms,
            &w.scene(21.),
            &strike(PLAYER_ID, Some(3), 0x80, false),
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
            radio.strike(
                &mut comms,
                &w.scene(0.),
                &strike(PLAYER_ID, Some(3), 1, true),
            );
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
}
