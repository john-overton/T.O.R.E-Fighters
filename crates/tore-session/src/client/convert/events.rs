//! The host's events as a replay's events, effects and surface changes
//! (docs/ARCHITECTURE.md, "Converting a capture into a replay"). Each is put
//! on the tick the host gave; one outside the replay's ticks is left out.

use super::{Ids, angles, basis, feet};
use crate::client::seen::FlightSeen;
use crate::wire::entity::VELOCITY_STEP;
use crate::wire::events::WireEvent;
use crate::wire::names::NameIndex;
use std::collections::BTreeMap;
use tore_replay as replay;
use tore_replay::vocab::{self, field, kind, outcome, route, source, trigger};
use tore_sim::combat::blast::{self, MarkKind};
use tore_sim::combat::live::EffectKind;
use tore_world::comms::Route;

/// The tick each projectile was launched at, from the host's launch events.
pub(crate) fn launches(seen: &FlightSeen) -> BTreeMap<u32, u32> {
    let mut out = BTreeMap::new();
    for seen in &seen.events {
        if let WireEvent::Launch { projectile, .. } = &seen.event.event {
            out.entry(*projectile).or_insert(seen.event.tick);
        }
    }
    out
}

/// What one event does to a frame.
enum Put {
    Event(replay::Event),
    Effect(replay::EffectSpawn),
    Surface(u32),
}

/// A flight's events, by tick.
pub(crate) struct Events {
    put: std::vec::IntoIter<(u64, Put)>,
    next: Option<(u64, Put)>,
}

impl Events {
    pub fn new(
        seen: &FlightSeen,
        ids: Ids,
        weapons: &BTreeMap<String, u32>,
        first: u64,
        last: u64,
    ) -> Self {
        let name = |index: NameIndex| seen.names.name(index).unwrap_or_default().to_owned();
        let stems = |names: &[NameIndex]| -> String {
            names.iter().map(|n| name(*n)).collect::<Vec<_>>().join(" ")
        };
        let mut out: Vec<(u64, Put)> = Vec::new();
        for received in &seen.events {
            let tick = u64::from(received.event.tick);
            if tick < first || tick > last {
                continue;
            }
            let events = match &received.event.event {
                WireEvent::Message { text } => vec![Put::Event(
                    replay::Event::new(kind::COMMS_HUD)
                        .with_subject(0)
                        .with(field::SOURCE, source::HUD)
                        .with(field::OUTCOME, outcome::DELIVERED)
                        .with_text(text.as_str()),
                )],
                WireEvent::Radio {
                    route: how,
                    important,
                    label,
                    text,
                    stems: names,
                } => {
                    let (event_kind, producer, played) = match how {
                        Route::Radio => (kind::COMMS_RADIO, source::RADIO, route::RADIO),
                        Route::Airport => (kind::COMMS_TOWER, source::TOWER, route::TOWER),
                        Route::Direct => (kind::COMMS_CREW, source::CREW, route::DIRECT),
                    };
                    let mut event = replay::Event::new(event_kind)
                        .with(field::SPEAKER, label.as_str())
                        .with(field::SOURCE, producer)
                        .with(field::ROUTE, played)
                        .with(
                            field::KIND,
                            if *important { "important" } else { "chatter" },
                        )
                        .with(field::OUTCOME, outcome::DELIVERED)
                        .with(field::HEARD, true)
                        .with_text(text.as_str());
                    if !names.is_empty() {
                        event = event.with(field::STEMS, stems(names));
                    }
                    vec![Put::Event(event)]
                }
                WireEvent::Tower { stem: Some(stem) } => vec![Put::Event(
                    replay::Event::new(kind::COMMS_TOWER)
                        .with(field::SOURCE, source::TOWER)
                        .with(field::ROUTE, route::TOWER)
                        .with(field::OUTCOME, outcome::DELIVERED)
                        .with(field::HEARD, true)
                        .with(field::STEMS, name(*stem)),
                )],
                WireEvent::Tower { stem: None } => vec![Put::Event(
                    replay::Event::new(kind::COMMS_TOWER)
                        .with(field::SOURCE, source::TOWER)
                        .with(field::TRIGGER, trigger::CLEARANCE_CANCELLED)
                        .with(field::OUTCOME, outcome::CANCELLED)
                        .with(field::HEARD, false),
                )],
                WireEvent::OrderVoice { stems: names } => vec![Put::Event(
                    replay::Event::new(kind::COMMS_ORDER)
                        .with_subject(0)
                        .with(field::SOURCE, source::ORDER)
                        .with(field::ROUTE, route::RADIO)
                        .with(field::OUTCOME, outcome::APPLIED)
                        .with(field::STEMS, stems(names)),
                )],
                WireEvent::Release { sound, .. } => vec![Put::Event(
                    replay::Event::new(kind::AUDIO_RELEASE)
                        .with_subject(0)
                        .with(field::SOUND, name(*sound)),
                )],
                WireEvent::Launch {
                    shooter,
                    projectile,
                    weapon,
                } => {
                    let mut event = replay::Event::new(kind::WEAPON_LAUNCH)
                        .with_subject(ids.map(*shooter))
                        .with(field::PROJECTILE, replay::Value::Id(*projectile));
                    if let Some(id) = weapons.get(&name(*weapon)) {
                        event = event.with(field::WEAPON, replay::Value::Id(*id));
                    }
                    vec![Put::Event(event)]
                }
                WireEvent::WingEjection {
                    aircraft,
                    message,
                    friendly,
                } => {
                    let aircraft = ids.map(*aircraft);
                    let mut out = vec![Put::Event(
                        replay::Event::new(kind::AIRCRAFT_EJECTED)
                            .with_subject(aircraft)
                            .with_text(message.as_str()),
                    )];
                    if *friendly {
                        out.push(Put::Event(
                            replay::Event::new(kind::AUDIO_EJECTION).with_subject(aircraft),
                        ));
                    }
                    out
                }
                WireEvent::Effect {
                    kind: effect,
                    position,
                    ticks,
                    blast,
                } => vec![Put::Effect(replay::EffectSpawn {
                    kind: effect_kind(*effect, *blast),
                    position: feet(position),
                    duration_ticks: u32::from(*ticks),
                })],
                WireEvent::Mark {
                    kind: mark,
                    position,
                } => vec![Put::Effect(replay::EffectSpawn {
                    kind: match mark {
                        MarkKind::Crater(size) => replay::EffectKind::Crater(*size),
                        MarkKind::Fire => replay::EffectKind::Fire,
                    },
                    position: feet(position),
                    duration_ticks: blast::FOREVER,
                })],
                WireEvent::GroundDestroyed { object } => vec![
                    Put::Surface(*object),
                    Put::Event(
                        replay::Event::new(kind::COMBAT_DESTROYED)
                            .with_subject(*object)
                            .with(field::REASON, "destroyed"),
                    ),
                ],
                WireEvent::Countermeasure {
                    aircraft,
                    flare,
                    position,
                    velocity,
                    attitude,
                    number,
                    left,
                } => {
                    let decoy = if *flare { "flare" } else { "chaff" };
                    let mut event = replay::Event::new(kind::COMBAT_COUNTERMEASURE)
                        .with_subject(ids.map(*aircraft))
                        .with(field::DECOY, decoy)
                        .with(field::NUMBER, *number as i64)
                        .with(field::AFTER_TICK, i64::from(received.event.tick));
                    if let Some(left) = left {
                        event = event.with(field::LEFT, i64::from(*left));
                    }
                    let b = basis(angles(*attitude));
                    let numbers = feet(position)
                        .into_iter()
                        .chain(velocity.map(|v| v as f64 * VELOCITY_STEP))
                        .chain(b.right)
                        .chain(b.up)
                        .chain(b.forward);
                    let names = field::POSITION
                        .into_iter()
                        .chain(field::VELOCITY)
                        .chain(field::BASIS);
                    for (name, value) in names.zip(numbers) {
                        event = event.with(name, value);
                    }
                    vec![Put::Event(event.with_text(format!("released {decoy}")))]
                }
                WireEvent::GunBurst {
                    shooter,
                    station,
                    length,
                } => {
                    let mut event = replay::Event::new(kind::WEAPON_GUN_BURST)
                        .with_subject(ids.map(*shooter))
                        .with(field::STATION, i64::from(*station));
                    if let Some(length) = length {
                        event = event.with("length_ticks", i64::from(*length));
                    }
                    vec![Put::Event(event)]
                }
                // Kept by the replay in other ways, or not at all: the rumble
                // and the weapon page are the player's hands, the order's
                // answer is its HUD line, the aircraft's end is in its state,
                // and sounds are made again from the recorded effects.
                WireEvent::OrderReply { .. }
                | WireEvent::WeaponCycled
                | WireEvent::Feedback { .. }
                | WireEvent::YourAircraftExploded { .. }
                | WireEvent::Sound { .. } => Vec::new(),
            };
            out.extend(events.into_iter().map(|put| (tick, put)));
        }
        // By tick, each tick's events in the order they arrived.
        out.sort_by_key(|(tick, _)| *tick);
        let mut put = out.into_iter();
        let next = put.next();
        Self { put, next }
    }

    /// Puts the events of `tick` into `frame`.
    pub fn fill(&mut self, tick: u64, frame: &mut replay::Frame) {
        while let Some((at, _)) = &self.next {
            if *at > tick {
                break;
            }
            let (_, put) = self.next.take().expect("an event");
            match put {
                Put::Event(event) => frame.events.push(event),
                Put::Effect(effect) => frame.new_effects.push(effect),
                Put::Surface(object) => frame.surface_hp.push((object, 0)),
            }
            self.next = self.put.next();
        }
        frame
            .new_effects
            .truncate(replay::limits::MAX_EFFECTS_PER_TICK);
        frame.events.truncate(replay::limits::MAX_EVENTS_PER_TICK);
        let _ = vocab::heard;
    }
}

/// A live effect as a recording keeps it: a hit, kill or ground strike with
/// a reviewed explosion type keeps that type (as `replay/convert.rs` in the
/// app does for the single-player recorder).
fn effect_kind(effect: EffectKind, explosion: Option<u8>) -> replay::EffectKind {
    let on = match effect {
        EffectKind::Hit => Some(replay::Strike::Hit),
        EffectKind::Destroyed => Some(replay::Strike::Destroyed),
        EffectKind::Ground => Some(replay::Strike::Ground),
        _ => None,
    };
    if let (Some(on), Some(explosion)) = (on, explosion.filter(|b| blast::explosion(*b).is_some()))
    {
        return replay::EffectKind::Blast { on, explosion };
    }
    match effect {
        EffectKind::Flare => replay::EffectKind::Flare,
        EffectKind::Chaff => replay::EffectKind::Chaff,
        EffectKind::Launch => replay::EffectKind::Launch,
        EffectKind::Hit => replay::EffectKind::Hit,
        EffectKind::Destroyed => replay::EffectKind::Destroyed,
        EffectKind::Ground => replay::EffectKind::Ground,
        EffectKind::DebrisImpact => replay::EffectKind::DebrisImpact,
    }
}
