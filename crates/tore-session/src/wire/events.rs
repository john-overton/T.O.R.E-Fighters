//! The Events section (host to client): things that happen once, for this
//! player or for everyone, repeated in every snapshot packet until the
//! client acknowledges them.
//!
//! See net-protocol.md, "Events". Each connection numbers its events (16
//! bits). [`EventQueue`] (the host) keeps every unacknowledged event and puts
//! as many as fit, oldest first, in each snapshot packet; a packet delivered
//! acknowledges the events it carried. [`EventReceiver`] (the client) hands
//! each event over once, and holds one that names a table entry whose Names
//! message has not arrived yet.
//!
//! Protocol 15 (slice G7) adds the Link event (code 17), one data link
//! journal entry about a member of the seat's flight, and the net a radio
//! call was heard on (a bit after "important").

use super::bits::{self, read_u32, read_uladder, write_uladder};
use super::inputs::{read_order, write_order};
use super::names::NameIndex;
use super::{WireError, WireResult, limits};
use std::collections::{HashSet, VecDeque};
use tore_codec::{BitReader, BitWriter};
use tore_input::FeedbackEvent;
use tore_sim::acoustics;
use tore_sim::ai::wing::PlayerOrder;
use tore_sim::combat::blast::MarkKind;
use tore_sim::combat::live::EffectKind;
use tore_world::comms::{Net, Route};
use tore_world::datalink::{ClearReason, Entry};
use tore_world::world::OrderOutcome;

/// A controller rumble, as the wire carries it: turbulence at 1/255.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rumble {
    GunFired,
    MissileLaunched,
    BombReleased,
    BombImpact,
    RocketLaunched,
    Turbulence(u8),
    AfterburnerEngaged,
    Damage,
    Crash,
}

impl Rumble {
    /// The wire's form of a feedback event.
    pub fn of(event: FeedbackEvent) -> Self {
        match event {
            FeedbackEvent::GunFired => Self::GunFired,
            FeedbackEvent::MissileLaunched => Self::MissileLaunched,
            FeedbackEvent::BombReleased => Self::BombReleased,
            FeedbackEvent::BombImpact => Self::BombImpact,
            FeedbackEvent::RocketLaunched => Self::RocketLaunched,
            FeedbackEvent::Turbulence { intensity } => Self::Turbulence(
                tore_codec::quant::quantize_unit(
                    if intensity.is_finite() {
                        intensity.clamp(0., 1.)
                    } else {
                        0.
                    },
                    255,
                )
                .unwrap_or(0) as u8,
            ),
            FeedbackEvent::AfterburnerEngaged => Self::AfterburnerEngaged,
            FeedbackEvent::Damage => Self::Damage,
            FeedbackEvent::Crash => Self::Crash,
        }
    }

    /// The feedback event it stands for.
    pub fn event(self) -> FeedbackEvent {
        match self {
            Self::GunFired => FeedbackEvent::GunFired,
            Self::MissileLaunched => FeedbackEvent::MissileLaunched,
            Self::BombReleased => FeedbackEvent::BombReleased,
            Self::BombImpact => FeedbackEvent::BombImpact,
            Self::RocketLaunched => FeedbackEvent::RocketLaunched,
            Self::Turbulence(q) => FeedbackEvent::Turbulence {
                intensity: f64::from(q) / 255.,
            },
            Self::AfterburnerEngaged => FeedbackEvent::AfterburnerEngaged,
            Self::Damage => FeedbackEvent::Damage,
            Self::Crash => FeedbackEvent::Crash,
        }
    }
}

/// One event. Positions are in 1/32 ft, velocities in 1/64 ft/s, attitudes
/// in 2^-16 of a turn, as in the entity records.
#[derive(Clone, Debug, PartialEq)]
pub enum WireEvent {
    /// A HUD line for the seat.
    Message { text: String },
    /// A radio or crew line due now for the seat: sent as it is, the client
    /// never composes calls.
    Radio {
        route: Route,
        /// Never silenced (missile warnings, fuel, deaths, the result).
        important: bool,
        /// The net the seat heard it on: the battle net's label already
        /// starts with `Net ` (protocol 15).
        net: Net,
        label: String,
        text: String,
        stems: Vec<NameIndex>,
    },
    /// A tower recording for the seat, or `None` to cut the tower off.
    Tower { stem: Option<NameIndex> },
    /// The seat's own order call.
    OrderVoice { stems: Vec<NameIndex> },
    /// What became of a wing order the seat gave.
    OrderReply {
        order: PlayerOrder,
        outcome: OrderOutcome,
    },
    /// The seat's weapon page turned.
    WeaponCycled,
    /// A weapon release sound from the seat's plane.
    Release { sound: NameIndex, station: u8 },
    /// Any missile, rocket or bomb launch.
    Launch {
        shooter: u32,
        projectile: u32,
        weapon: NameIndex,
    },
    /// A rumble for the seat.
    Feedback { rumble: Rumble },
    /// The seat's aircraft exploded, or exploded on impact.
    YourAircraftExploded { on_impact: bool },
    /// An AI pilot ejected.
    WingEjection {
        aircraft: u32,
        message: String,
        friendly: bool,
    },
    /// A flash, hit or explosion.
    Effect {
        kind: EffectKind,
        position: [i64; 3],
        ticks: u16,
        blast: Option<u8>,
    },
    /// A crater or crash-site fire, from the event's tick.
    Mark { kind: MarkKind, position: [i64; 3] },
    /// A ground object was destroyed.
    GroundDestroyed { object: u32 },
    /// Chaff or a flare left an aircraft: the release geometry the client
    /// flies it from, its number (which chose its look) and the owner's
    /// devices of that kind left.
    Countermeasure {
        aircraft: u32,
        flare: bool,
        position: [i64; 3],
        velocity: [i64; 3],
        attitude: [u16; 3],
        number: u64,
        left: Option<u8>,
    },
    /// A gun burst from the event's tick: `length` ticks, or `None` while
    /// still firing.
    GunBurst {
        shooter: u32,
        station: u8,
        length: Option<u32>,
    },
    /// A sound emission.
    Sound {
        kind: acoustics::Kind,
        position: [i64; 3],
        /// A Mach cone crossing, already the shock's arrival.
        arrived: bool,
        /// The aircraft it came from, when one did.
        from: Option<u32>,
    },
    /// A change of the flight data link's picture about a member of the
    /// seat's flight (protocol 15, slice G7).
    Link(LinkEvent),
}

/// One data link journal entry as the wire carries it: the entry without its
/// tick, which the event carries (`tore_world::datalink::Entry`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LinkEvent {
    /// `plane` joined the picture, with whether its aircraft has a radar.
    Member { plane: u32, radar: bool },
    /// `plane` took a lock on `target`.
    Lock { plane: u32, target: u32 },
    /// `plane` let go of its lock on `target`.
    Unlock { plane: u32, target: u32 },
    /// The lead `by` gave `plane` the target `target` with `order`.
    Assign {
        plane: u32,
        target: u32,
        by: u32,
        order: PlayerOrder,
    },
    /// `plane`'s assignment of `target` ended, and why.
    Clear {
        plane: u32,
        target: u32,
        why: ClearReason,
    },
    /// `plane` locked its assigned target.
    Acknowledge { plane: u32, target: u32 },
    /// The human flying `plane` was told its flightmate `other` holds a lock
    /// on `target`.
    SortWarning { plane: u32, other: u32, target: u32 },
}

impl LinkEvent {
    /// The wire's form of a journal entry.
    pub fn of(entry: &Entry) -> Self {
        match *entry {
            Entry::Member { plane, radar, .. } => Self::Member { plane, radar },
            Entry::Lock { plane, target, .. } => Self::Lock { plane, target },
            Entry::Unlock { plane, target, .. } => Self::Unlock { plane, target },
            Entry::Assign {
                plane,
                target,
                by,
                order,
                ..
            } => Self::Assign {
                plane,
                target,
                by,
                order,
            },
            Entry::Clear {
                plane, target, why, ..
            } => Self::Clear { plane, target, why },
            Entry::Acknowledge { plane, target, .. } => Self::Acknowledge { plane, target },
            Entry::SortWarning {
                plane,
                other,
                target,
                ..
            } => Self::SortWarning {
                plane,
                other,
                target,
            },
        }
    }

    /// The journal entry it stands for, at `tick`.
    pub fn entry(self, tick: u64) -> Entry {
        match self {
            Self::Member { plane, radar } => Entry::Member { tick, plane, radar },
            Self::Lock { plane, target } => Entry::Lock {
                tick,
                plane,
                target,
            },
            Self::Unlock { plane, target } => Entry::Unlock {
                tick,
                plane,
                target,
            },
            Self::Assign {
                plane,
                target,
                by,
                order,
            } => Entry::Assign {
                tick,
                plane,
                target,
                by,
                order,
            },
            Self::Clear { plane, target, why } => Entry::Clear {
                tick,
                plane,
                target,
                why,
            },
            Self::Acknowledge { plane, target } => Entry::Acknowledge {
                tick,
                plane,
                target,
            },
            Self::SortWarning {
                plane,
                other,
                target,
            } => Entry::SortWarning {
                tick,
                plane,
                other,
                target,
            },
        }
    }

    /// The member the change is about.
    pub fn plane(self) -> u32 {
        match self {
            Self::Member { plane, .. }
            | Self::Lock { plane, .. }
            | Self::Unlock { plane, .. }
            | Self::Assign { plane, .. }
            | Self::Clear { plane, .. }
            | Self::Acknowledge { plane, .. }
            | Self::SortWarning { plane, .. } => plane,
        }
    }

    fn write(self, w: &mut BitWriter) {
        let code = match self {
            Self::Member { .. } => 0,
            Self::Lock { .. } => 1,
            Self::Unlock { .. } => 2,
            Self::Assign { .. } => 3,
            Self::Clear { .. } => 4,
            Self::Acknowledge { .. } => 5,
            Self::SortWarning { .. } => 6,
        };
        let _ = w.write_bits(code, 3);
        w.write_varint(u64::from(self.plane()));
        match self {
            Self::Member { radar, .. } => w.write_bool(radar),
            Self::Lock { target, .. }
            | Self::Unlock { target, .. }
            | Self::Acknowledge { target, .. } => w.write_varint(u64::from(target)),
            Self::Assign {
                target, by, order, ..
            } => {
                w.write_varint(u64::from(target));
                w.write_varint(u64::from(by));
                write_order(w, order);
            }
            Self::Clear { target, why, .. } => {
                w.write_varint(u64::from(target));
                let why = match why {
                    ClearReason::Order => 0,
                    ClearReason::ReceiverLost => 1,
                    ClearReason::TargetLost => 2,
                    ClearReason::LeadChanged => 3,
                };
                let _ = w.write_bits(why, 2);
            }
            Self::SortWarning { other, target, .. } => {
                w.write_varint(u64::from(other));
                w.write_varint(u64::from(target));
            }
        }
    }

    fn read(r: &mut BitReader<'_>) -> WireResult<Self> {
        let code = r.read_bits(3)?;
        let plane = read_u32(r)?;
        Ok(match code {
            0 => Self::Member {
                plane,
                radar: r.read_bool()?,
            },
            1 => Self::Lock {
                plane,
                target: read_u32(r)?,
            },
            2 => Self::Unlock {
                plane,
                target: read_u32(r)?,
            },
            3 => Self::Assign {
                plane,
                target: read_u32(r)?,
                by: read_u32(r)?,
                order: read_order(r)?,
            },
            4 => Self::Clear {
                plane,
                target: read_u32(r)?,
                why: [
                    ClearReason::Order,
                    ClearReason::ReceiverLost,
                    ClearReason::TargetLost,
                    ClearReason::LeadChanged,
                ][r.read_bits(2)? as usize],
            },
            5 => Self::Acknowledge {
                plane,
                target: read_u32(r)?,
            },
            6 => Self::SortWarning {
                plane,
                other: read_u32(r)?,
                target: read_u32(r)?,
            },
            _ => return Err(WireError::Invalid("link event")),
        })
    }
}

const KIND_BITS: u32 = 5;

fn write_position(w: &mut BitWriter, position: &[i64; 3]) {
    for v in position {
        w.write_varint_signed(*v);
    }
}

fn read_steps(r: &mut BitReader<'_>) -> WireResult<[i64; 3]> {
    let mut out = [0; 3];
    for v in &mut out {
        *v = bits::in_range(i128::from(r.read_varint_signed()?), "event position")?;
    }
    Ok(out)
}

fn write_names(w: &mut BitWriter, names: &[NameIndex]) -> WireResult<()> {
    if names.len() > limits::STEMS {
        return Err(WireError::TooMany {
            what: "stems",
            limit: limits::STEMS,
        });
    }
    let _ = w.write_bits(names.len() as u64, 6);
    for name in names {
        name.write(w);
    }
    Ok(())
}

fn read_names(r: &mut BitReader<'_>) -> WireResult<Vec<NameIndex>> {
    let count = r.read_bits(6)? as usize;
    if count > limits::STEMS {
        return Err(WireError::TooMany {
            what: "stems",
            limit: limits::STEMS,
        });
    }
    (0..count).map(|_| NameIndex::read(r)).collect()
}

fn effect_code(kind: EffectKind) -> u64 {
    match kind {
        EffectKind::Flare => 0,
        EffectKind::Chaff => 1,
        EffectKind::Launch => 2,
        EffectKind::Hit => 3,
        EffectKind::Destroyed => 4,
        EffectKind::Ground => 5,
        EffectKind::DebrisImpact => 6,
    }
}

const EFFECTS: [EffectKind; 7] = [
    EffectKind::Flare,
    EffectKind::Chaff,
    EffectKind::Launch,
    EffectKind::Hit,
    EffectKind::Destroyed,
    EffectKind::Ground,
    EffectKind::DebrisImpact,
];

fn write_sound_kind(w: &mut BitWriter, kind: acoustics::Kind) {
    use acoustics::Kind as K;
    let code = match kind {
        K::Impact => 0,
        K::Explosion => 1,
        K::Blast(_) => 2,
        K::AircraftPass => 3,
        K::MissilePass => 4,
        K::SonicBoom => 5,
        K::Chaff => 6,
        K::Flare => 7,
    };
    let _ = w.write_bits(code, 3);
    if let K::Blast(blast) = kind {
        let _ = w.write_bits(u64::from(blast), 8);
    }
}

fn read_sound_kind(r: &mut BitReader<'_>) -> WireResult<acoustics::Kind> {
    use acoustics::Kind as K;
    Ok(match r.read_bits(3)? {
        0 => K::Impact,
        1 => K::Explosion,
        2 => K::Blast(r.read_bits(8)? as u8),
        3 => K::AircraftPass,
        4 => K::MissilePass,
        5 => K::SonicBoom,
        6 => K::Chaff,
        _ => K::Flare,
    })
}

fn rumble_code(rumble: Rumble) -> u64 {
    match rumble {
        Rumble::GunFired => 0,
        Rumble::MissileLaunched => 1,
        Rumble::BombReleased => 2,
        Rumble::BombImpact => 3,
        Rumble::RocketLaunched => 4,
        Rumble::Turbulence(_) => 5,
        Rumble::AfterburnerEngaged => 6,
        Rumble::Damage => 7,
        Rumble::Crash => 8,
    }
}

impl WireEvent {
    /// The event's 5-bit code.
    pub fn code(&self) -> u64 {
        match self {
            Self::Message { .. } => 0,
            Self::Radio { .. } => 1,
            Self::Tower { .. } => 2,
            Self::OrderVoice { .. } => 3,
            Self::OrderReply { .. } => 4,
            Self::WeaponCycled => 5,
            Self::Release { .. } => 6,
            Self::Launch { .. } => 7,
            Self::Feedback { .. } => 8,
            Self::YourAircraftExploded { .. } => 9,
            Self::WingEjection { .. } => 10,
            Self::Effect { .. } => 11,
            Self::Mark { .. } => 12,
            Self::GroundDestroyed { .. } => 13,
            Self::Countermeasure { .. } => 14,
            Self::GunBurst { .. } => 15,
            Self::Sound { .. } => 16,
            Self::Link(_) => 17,
        }
    }

    /// The highest name index the event uses, if any.
    pub fn highest_name(&self) -> Option<NameIndex> {
        match self {
            Self::Radio { stems, .. } | Self::OrderVoice { stems } => stems.iter().max().copied(),
            Self::Tower { stem } => *stem,
            Self::Release { sound, .. } => Some(*sound),
            Self::Launch { weapon, .. } => Some(*weapon),
            _ => None,
        }
    }

    /// Writes the event's code and fields.
    pub fn write(&self, w: &mut BitWriter) -> WireResult<()> {
        let _ = w.write_bits(self.code(), KIND_BITS);
        match self {
            Self::Message { text } => bits::write_str(w, text),
            Self::Radio {
                route,
                important,
                net,
                label,
                text,
                stems,
            } => {
                let route = match route {
                    Route::Radio => 0,
                    Route::Airport => 1,
                    Route::Direct => 2,
                };
                let _ = w.write_bits(route, 2);
                w.write_bool(*important);
                w.write_bool(*net == Net::Battle);
                bits::write_str(w, label);
                bits::write_str(w, text);
                write_names(w, stems)?;
            }
            Self::Tower { stem } => bits::write_option(w, *stem, |w, stem| stem.write(w)),
            Self::OrderVoice { stems } => write_names(w, stems)?,
            Self::OrderReply { order, outcome } => {
                write_order(w, *order);
                let (code, message) = match outcome {
                    OrderOutcome::Given { message } => (0, message),
                    OrderOutcome::Refused { message } => (1, message),
                    OrderOutcome::Failed { message } => (2, message),
                };
                let _ = w.write_bits(code, 2);
                bits::write_str(w, message);
            }
            Self::WeaponCycled => {}
            Self::Release { sound, station } => {
                sound.write(w);
                let _ = w.write_bits(u64::from(*station), 8);
            }
            Self::Launch {
                shooter,
                projectile,
                weapon,
            } => {
                w.write_varint(u64::from(*shooter));
                w.write_varint(u64::from(*projectile));
                weapon.write(w);
            }
            Self::Feedback { rumble } => {
                let _ = w.write_bits(rumble_code(*rumble), 4);
                if let Rumble::Turbulence(q) = rumble {
                    let _ = w.write_bits(u64::from(*q), 8);
                }
            }
            Self::YourAircraftExploded { on_impact } => w.write_bool(*on_impact),
            Self::WingEjection {
                aircraft,
                message,
                friendly,
            } => {
                w.write_varint(u64::from(*aircraft));
                bits::write_str(w, message);
                w.write_bool(*friendly);
            }
            Self::Effect {
                kind,
                position,
                ticks,
                blast,
            } => {
                let _ = w.write_bits(effect_code(*kind), 3);
                write_position(w, position);
                let _ = w.write_bits(u64::from(*ticks), 16);
                bits::write_option(w, *blast, |w, blast| {
                    let _ = w.write_bits(u64::from(blast), 8);
                });
            }
            Self::Mark { kind, position } => {
                match kind {
                    MarkKind::Crater(size) => {
                        w.write_bool(false);
                        let _ = w.write_bits(u64::from(*size), 8);
                    }
                    MarkKind::Fire => w.write_bool(true),
                }
                write_position(w, position);
            }
            Self::GroundDestroyed { object } => w.write_varint(u64::from(*object)),
            Self::Countermeasure {
                aircraft,
                flare,
                position,
                velocity,
                attitude,
                number,
                left,
            } => {
                w.write_varint(u64::from(*aircraft));
                w.write_bool(*flare);
                write_position(w, position);
                write_position(w, velocity);
                for angle in attitude {
                    let _ = w.write_bits(u64::from(*angle), 16);
                }
                w.write_varint(*number);
                bits::write_option(w, *left, |w, left| {
                    let _ = w.write_bits(u64::from(left), 8);
                });
            }
            Self::GunBurst {
                shooter,
                station,
                length,
            } => {
                w.write_varint(u64::from(*shooter));
                let _ = w.write_bits(u64::from(*station), 8);
                // 0 while firing, else the length.
                w.write_varint(length.map_or(0, |l| u64::from(l) + 1));
            }
            Self::Sound {
                kind,
                position,
                arrived,
                from,
            } => {
                write_sound_kind(w, *kind);
                write_position(w, position);
                w.write_bool(*arrived);
                bits::write_option(w, *from, |w, from| w.write_varint(u64::from(from)));
            }
            Self::Link(link) => link.write(w),
        }
        Ok(())
    }

    /// Reads an event [`Self::write`] wrote.
    pub fn read(r: &mut BitReader<'_>) -> WireResult<Self> {
        Ok(match r.read_bits(KIND_BITS)? {
            0 => Self::Message {
                text: bits::read_str(r)?,
            },
            1 => {
                let route = match r.read_bits(2)? {
                    0 => Route::Radio,
                    1 => Route::Airport,
                    2 => Route::Direct,
                    _ => return Err(WireError::Invalid("radio route")),
                };
                Self::Radio {
                    route,
                    important: r.read_bool()?,
                    net: if r.read_bool()? {
                        Net::Battle
                    } else {
                        Net::Wing
                    },
                    label: bits::read_str(r)?,
                    text: bits::read_str(r)?,
                    stems: read_names(r)?,
                }
            }
            2 => Self::Tower {
                stem: bits::read_option(r, NameIndex::read)?,
            },
            3 => Self::OrderVoice {
                stems: read_names(r)?,
            },
            4 => {
                let order = read_order(r)?;
                let code = r.read_bits(2)?;
                let message = bits::read_str(r)?;
                let outcome = match code {
                    0 => OrderOutcome::Given { message },
                    1 => OrderOutcome::Refused { message },
                    2 => OrderOutcome::Failed { message },
                    _ => return Err(WireError::Invalid("order outcome")),
                };
                Self::OrderReply { order, outcome }
            }
            5 => Self::WeaponCycled,
            6 => Self::Release {
                sound: NameIndex::read(r)?,
                station: r.read_bits(8)? as u8,
            },
            7 => Self::Launch {
                shooter: read_u32(r)?,
                projectile: read_u32(r)?,
                weapon: NameIndex::read(r)?,
            },
            8 => {
                let rumble = match r.read_bits(4)? {
                    0 => Rumble::GunFired,
                    1 => Rumble::MissileLaunched,
                    2 => Rumble::BombReleased,
                    3 => Rumble::BombImpact,
                    4 => Rumble::RocketLaunched,
                    5 => Rumble::Turbulence(r.read_bits(8)? as u8),
                    6 => Rumble::AfterburnerEngaged,
                    7 => Rumble::Damage,
                    8 => Rumble::Crash,
                    _ => return Err(WireError::Invalid("rumble")),
                };
                Self::Feedback { rumble }
            }
            9 => Self::YourAircraftExploded {
                on_impact: r.read_bool()?,
            },
            10 => Self::WingEjection {
                aircraft: read_u32(r)?,
                message: bits::read_str(r)?,
                friendly: r.read_bool()?,
            },
            11 => Self::Effect {
                kind: *EFFECTS
                    .get(r.read_bits(3)? as usize)
                    .ok_or(WireError::Invalid("effect"))?,
                position: read_steps(r)?,
                ticks: r.read_bits(16)? as u16,
                blast: bits::read_option(r, |r| Ok(r.read_bits(8)? as u8))?,
            },
            12 => {
                let kind = if r.read_bool()? {
                    MarkKind::Fire
                } else {
                    MarkKind::Crater(r.read_bits(8)? as u8)
                };
                Self::Mark {
                    kind,
                    position: read_steps(r)?,
                }
            }
            13 => Self::GroundDestroyed {
                object: read_u32(r)?,
            },
            14 => Self::Countermeasure {
                aircraft: read_u32(r)?,
                flare: r.read_bool()?,
                position: read_steps(r)?,
                velocity: read_steps(r)?,
                attitude: [
                    r.read_bits(16)? as u16,
                    r.read_bits(16)? as u16,
                    r.read_bits(16)? as u16,
                ],
                number: r.read_varint()?,
                left: bits::read_option(r, |r| Ok(r.read_bits(8)? as u8))?,
            },
            15 => Self::GunBurst {
                shooter: read_u32(r)?,
                station: r.read_bits(8)? as u8,
                length: match read_u32(r)? {
                    0 => None,
                    n => Some(n - 1),
                },
            },
            16 => Self::Sound {
                kind: read_sound_kind(r)?,
                position: read_steps(r)?,
                arrived: r.read_bool()?,
                from: bits::read_option(r, read_u32)?,
            },
            17 => Self::Link(LinkEvent::read(r)?),
            _ => return Err(WireError::Invalid("event kind")),
        })
    }
}

/// Event number differences after the first: zero, 4 bits, 10 bits or a
/// varint.
const NUMBER_LADDER: [u32; 3] = [0, 4, 10];

/// One event as the section carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct SectionEvent {
    pub number: u16,
    /// Ticks before the snapshot's tick.
    pub ticks_back: u32,
    pub event: WireEvent,
}

/// An Events section as read.
#[derive(Clone, Debug, PartialEq)]
pub struct EventsSection {
    pub events: Vec<SectionEvent>,
}

impl EventsSection {
    /// The section's bytes; numbers must ascend (wrapping).
    pub fn encode(&self) -> WireResult<Vec<u8>> {
        let mut w = BitWriter::with_capacity(256);
        write_header(&mut w, self.events.len())?;
        let mut previous: Option<u16> = None;
        for event in &self.events {
            let mut body = BitWriter::new();
            event.event.write(&mut body)?;
            write_event(&mut w, previous, event.number, event.ticks_back, &body)?;
            previous = Some(event.number);
        }
        Ok(bits::finish(w))
    }

    /// Reads a section.
    pub fn decode(bytes: &[u8]) -> WireResult<Self> {
        let mut r = BitReader::new(bytes);
        let count = bits::read_count(&mut r, limits::EVENTS, "events")?;
        if count == 0 {
            return Err(WireError::Invalid("empty events section"));
        }
        let mut events = Vec::with_capacity(count.min(64));
        let mut number = r.read_bits(16)? as u16;
        for index in 0..count {
            if index > 0 {
                let step = read_uladder(&mut r, &NUMBER_LADDER)?;
                if step >= u64::from(limits::EVENTS as u32) {
                    return Err(WireError::Invalid("event number"));
                }
                number = number.wrapping_add(step as u16 + 1);
            }
            let ticks_back = read_u32(&mut r)?;
            events.push(SectionEvent {
                number,
                ticks_back,
                event: WireEvent::read(&mut r)?,
            });
        }
        bits::end(&mut r)?;
        Ok(Self { events })
    }
}

fn write_header(w: &mut BitWriter, count: usize) -> WireResult<()> {
    if count == 0 || count > limits::EVENTS {
        return Err(WireError::Invalid("events section count"));
    }
    bits::write_count(w, count);
    Ok(())
}

fn write_event(
    w: &mut BitWriter,
    previous: Option<u16>,
    number: u16,
    ticks_back: u32,
    body: &BitWriter,
) -> WireResult<()> {
    match previous {
        None => {
            let _ = w.write_bits(u64::from(number), 16);
        }
        Some(previous) => {
            let step = number.wrapping_sub(previous).wrapping_sub(1);
            if usize::from(step) >= limits::EVENTS {
                return Err(WireError::Invalid("event numbers"));
            }
            write_uladder(w, u64::from(step), &NUMBER_LADDER);
        }
    }
    w.write_varint(u64::from(ticks_back));
    bits::append(w, body);
    Ok(())
}

#[derive(Clone, Debug)]
struct Queued {
    number: u16,
    tick: u32,
    body: BitWriter,
}

/// What one Events section held.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EventsReport {
    pub sent: usize,
    /// Unacknowledged events left out for room.
    pub waiting: usize,
    pub bytes: usize,
}

/// The host's queue for one connection: every event not yet acknowledged.
#[derive(Clone, Debug, Default)]
pub struct EventQueue {
    next: u16,
    queue: VecDeque<Queued>,
    /// Packets not yet heard of, and the events each carried.
    packets: VecDeque<(Option<u16>, Vec<u16>)>,
}

impl EventQueue {
    /// An empty queue.
    pub fn new() -> Self {
        Self::default()
    }

    /// Queues `event`, which happened at host tick `tick`, and returns its
    /// number. More than 1,024 unacknowledged events is
    /// [`WireError::TooFarBehind`]: the connection ends.
    pub fn push(&mut self, tick: u32, event: &WireEvent) -> WireResult<u16> {
        if self.queue.len() >= limits::EVENTS {
            return Err(WireError::TooFarBehind);
        }
        let mut body = BitWriter::new();
        event.write(&mut body)?;
        let number = self.next;
        self.next = self.next.wrapping_add(1);
        self.queue.push_back(Queued { number, tick, body });
        Ok(number)
    }

    /// Unacknowledged events.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// True when every event is acknowledged.
    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    /// About how many bytes the waiting events take, for sharing the packet.
    pub fn waiting_bytes(&self) -> usize {
        self.queue
            .iter()
            .map(|q| q.body.bit_len() / 8 + 5)
            .sum::<usize>()
            + 3
    }

    /// Builds the Events section for a snapshot of `tick`: every
    /// unacknowledged event, oldest first, while the section stays within
    /// `budget` bytes; the oldest one always goes when it fits `room`, so a
    /// long one is never starved. `None` when nothing is waiting or nothing
    /// fits. Staged like [`super::snapshot::EntitySender::build`].
    pub fn build(
        &mut self,
        tick: u32,
        budget: usize,
        room: usize,
    ) -> WireResult<(Option<Vec<u8>>, EventsReport)> {
        self.discard();
        let mut report = EventsReport::default();
        if self.queue.is_empty() {
            return Ok((None, report));
        }
        let mut w = BitWriter::with_capacity(budget.min(1_200));
        // Room for a count up to 1,024 (two bytes).
        let header_bits = 16;
        let mut numbers = Vec::new();
        let mut bits_used = header_bits;
        let mut chosen: Vec<(u16, u32, &BitWriter)> = Vec::new();
        let mut previous: Option<u16> = None;
        for queued in &self.queue {
            let mut probe = BitWriter::new();
            let ticks_back = tick.saturating_sub(queued.tick);
            write_event(
                &mut probe,
                previous,
                queued.number,
                ticks_back,
                &queued.body,
            )?;
            let limit = if chosen.is_empty() {
                room.max(budget)
            } else {
                budget
            };
            if (bits_used + probe.bit_len()).div_ceil(8) > limit {
                break;
            }
            bits_used += probe.bit_len();
            chosen.push((queued.number, ticks_back, &queued.body));
            previous = Some(queued.number);
        }
        report.waiting = self.queue.len() - chosen.len();
        if chosen.is_empty() {
            return Ok((None, report));
        }
        write_header(&mut w, chosen.len())?;
        let mut previous: Option<u16> = None;
        for (number, ticks_back, body) in &chosen {
            write_event(&mut w, previous, *number, *ticks_back, body)?;
            previous = Some(*number);
            numbers.push(*number);
        }
        let bytes = bits::finish(w);
        report.sent = numbers.len();
        report.bytes = bytes.len();
        self.packets.push_back((None, numbers));
        Ok((Some(bytes), report))
    }

    /// The staged section went out in the packet numbered `sequence`.
    pub fn sent(&mut self, sequence: u16) {
        if let Some(packet) = self.packets.back_mut()
            && packet.0.is_none()
        {
            packet.0 = Some(sequence);
        }
        while self.packets.len() > 64 {
            self.packets.pop_front();
        }
    }

    /// The staged section was not sent.
    pub fn discard(&mut self) {
        if self.packets.back().is_some_and(|p| p.0.is_none()) {
            self.packets.pop_back();
        }
    }

    /// The packet numbered `sequence` was delivered: its events are
    /// acknowledged.
    pub fn delivered(&mut self, sequence: u16) {
        let Some(index) = self.packets.iter().position(|p| p.0 == Some(sequence)) else {
            return;
        };
        if let Some((_, numbers)) = self.packets.remove(index) {
            let numbers: HashSet<u16> = numbers.into_iter().collect();
            self.queue.retain(|q| !numbers.contains(&q.number));
        }
    }

    /// The packet numbered `sequence` was lost.
    pub fn lost(&mut self, sequence: u16) {
        self.packets.retain(|p| p.0 != Some(sequence));
    }
}

/// An event handed to the client, once.
#[derive(Clone, Debug, PartialEq)]
pub struct ReceivedEvent {
    pub number: u16,
    /// The host tick it happened at.
    pub tick: u32,
    pub event: WireEvent,
}

/// Event numbers the client remembers, to drop repeats: four times the
/// host's limit of unacknowledged events.
const SEEN: usize = 4 * limits::EVENTS;

/// The client's side of the events: repeats dropped, and events that name a
/// table entry not yet arrived held until it does.
#[derive(Clone, Debug, Default)]
pub struct EventReceiver {
    seen: HashSet<u16>,
    order: VecDeque<u16>,
    held: Vec<ReceivedEvent>,
}

impl EventReceiver {
    /// A receiver that has seen nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// The new events of a section that came with the snapshot of `tick`,
    /// with `names` table entries known, in number order.
    pub fn receive(
        &mut self,
        section: &EventsSection,
        tick: u32,
        names: usize,
    ) -> Vec<ReceivedEvent> {
        let mut out = Vec::new();
        for event in &section.events {
            if !self.seen.insert(event.number) {
                continue;
            }
            self.order.push_back(event.number);
            while self.order.len() > SEEN {
                if let Some(old) = self.order.pop_front() {
                    self.seen.remove(&old);
                }
            }
            let received = ReceivedEvent {
                number: event.number,
                tick: tick.saturating_sub(event.ticks_back),
                event: event.event.clone(),
            };
            if Self::ready(&received, names) && self.held.is_empty() {
                out.push(received);
            } else {
                self.held.push(received);
            }
        }
        out.extend(self.names_arrived(names));
        out
    }

    fn ready(event: &ReceivedEvent, names: usize) -> bool {
        event
            .event
            .highest_name()
            .is_none_or(|index| usize::from(index.0) < names)
    }

    /// Events held for names, now that `names` entries are known, in the
    /// order they arrived; one still waiting holds back those after it.
    pub fn names_arrived(&mut self, names: usize) -> Vec<ReceivedEvent> {
        let ready = self
            .held
            .iter()
            .position(|event| !Self::ready(event, names))
            .unwrap_or(self.held.len());
        self.held.drain(..ready).collect()
    }

    /// Events waiting for their names.
    pub fn held(&self) -> usize {
        self.held.len()
    }
}
