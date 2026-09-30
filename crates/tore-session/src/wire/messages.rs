//! The reliable message bodies (net-protocol.md, "Reliable messages"): the
//! transport carries each as a kind byte and a body of up to 64 KB, split
//! into fragments and put back together in order.

use super::bits::{
    self, read_count, read_long_str, read_str, read_u32, write_count, write_long_str, write_str,
};
use super::names::ReceivedNames;
use super::{WireError, WireResult, limits};
use tore_codec::{BitReader, BitWriter};
use tore_formats::aircraft::AircraftId;
use tore_sim::ai::launch::{Side, WingId};
use tore_sim::combat::ledger::Tally;
use tore_sim::models::AircraftModel;
use tore_world::mission::{LoadoutSpec, MissionSpec, StationLoad};
use tore_world::resources::{Manifest, ManifestEntry};
use tore_world::world::plane::ExactState;

/// Message kinds, the transport's kind byte.
pub mod kind {
    pub const MISSION: u8 = 1;
    pub const CONTENT_REFUSED: u8 = 2;
    pub const READY: u8 = 3;
    pub const SEAT_REFUSED: u8 = 4;
    pub const SEATED: u8 = 5;
    pub const ROSTER: u8 = 6;
    pub const NAMES: u8 = 7;
    pub const NOTICE: u8 = 8;
    pub const LEAVE: u8 = 9;
    pub const DEBRIEF: u8 = 10;
    pub const MISSION_ENDED: u8 = 11;
}

/// Entries of a content manifest or a refusal's list.
const MANIFEST_LIMIT: usize = 8_192;
/// Stations of a loadout.
const STATIONS_LIMIT: usize = 64;
/// Planes of a roster.
const PLANES_LIMIT: usize = 256;
/// Destroyed ground objects listed at seating.
const DESTROYED_LIMIT: usize = 8_192;
/// Objectives in a debrief.
const OBJECTIVES_LIMIT: usize = 64;

/// The mission a joining client loads from its own import (host to client).
#[derive(Clone, Debug, PartialEq)]
pub struct Mission {
    /// The `MissionSpec` in its text form.
    pub spec: String,
    /// The content manifest: every resource the build read, with its hash.
    pub manifest: Manifest,
    /// The host tick now, from which the weather clock's reading follows.
    pub host_tick: u32,
    /// Combat's contrail sortie number, which sets each aircraft's contrail
    /// height.
    pub contrail_sortie: u64,
}

impl Mission {
    /// The spec, parsed.
    pub fn spec(&self) -> Result<MissionSpec, WireError> {
        MissionSpec::from_text(&self.spec).map_err(|_| WireError::Invalid("mission spec"))
    }
}

/// The resources whose hash differs or which are missing (client to host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRefused {
    pub names: Vec<String>,
}

/// The player is ready to fly (client to host).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ready {
    /// The plane wanted, or `None` for any.
    pub plane: Option<u32>,
}

/// Who flies a plane, as the roster tells it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RosterPilot {
    Ai,
    Human { seat: u8, callsign: String },
}

/// One plane of the roster.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RosterPlane {
    pub id: u32,
    pub wing: WingId,
    pub member: u8,
    pub aircraft: AircraftId,
    pub pilot: RosterPilot,
}

/// Every plane of the mission with its pilot (host to client, on every
/// change).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Roster {
    pub planes: Vec<RosterPlane>,
}

/// The player has a plane (host to client).
#[derive(Clone, Debug, PartialEq)]
pub struct Seated {
    pub seat: u8,
    pub plane: u32,
    /// The tick of the state.
    pub tick: u32,
    /// The plane's exact state at `tick`, coded with no baseline
    /// ([`ExactState::encode`]); see [`Self::exact_state`].
    pub exact: Vec<u8>,
    pub loadout: LoadoutSpec,
    pub roster: Roster,
    /// The ground objects destroyed so far; every other one stands.
    pub destroyed: Vec<u32>,
}

impl Seated {
    /// The exact state, decoded with the plane's aircraft `model`.
    pub fn exact_state(&self, model: &AircraftModel) -> WireResult<ExactState> {
        Ok(ExactState::decode(&self.exact, None, model)?)
    }
}

/// New entries of the connection's name table (host to client).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Names {
    /// The index of the first entry; reliable delivery keeps them in order.
    pub first: u16,
    pub names: Vec<String>,
}

impl Names {
    /// Adds the entries to the client's table.
    pub fn apply_to(&self, table: &mut ReceivedNames) -> WireResult<()> {
        table.apply(self)
    }
}

/// How a debriefed pilot ended the mission.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PilotStatus {
    #[default]
    Alive,
    Ejected,
    Dead,
}

/// A debrief objective line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebriefObjective {
    Destroy { destroyed: u32, total: u32 },
    Protect { protected: u32, total: u32 },
}

/// One column of the debrief: the seat's pilot or its wingman.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DebriefPilot {
    pub status: PilotStatus,
    /// Airframe damage, 0 to 1, exactly.
    pub damage: f64,
    pub landing_grade: Option<u32>,
    /// Why the airframe was lost, when not to combat ("overspeed").
    pub cause: Option<String>,
    /// Kills by the debrief's ten rows.
    pub kills: [u32; 10],
    pub friendly_fire: u32,
    pub air_to_air: Tally,
    pub air_to_ground: Tally,
    pub gun: Tally,
    pub bombs: Tally,
    pub enemy_aam: Tally,
    pub enemy_sam: Tally,
    pub enemy_gun: Tally,
    pub enemy_aaa: Tally,
}

/// The seat's debrief report as the single-player debrief shows it (host to
/// client). The game's `debrief::Report` maps field for field.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Debrief {
    pub success: bool,
    pub objectives: Vec<DebriefObjective>,
    pub elapsed_seconds: u64,
    pub player: DebriefPilot,
    pub wingman: Option<DebriefPilot>,
}

/// Why the mission ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EndReason {
    EveryoneLeft,
    TimeLimit,
    ServerStopping,
    /// The server's operator ended the mission (the console's `end` or
    /// `restart`).
    EndedByServer,
}

/// The host ended the mission (host to client).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MissionEnded {
    pub reason: EndReason,
    /// Seconds until the next mission, if one follows.
    pub next_in_seconds: Option<u32>,
}

/// Any reliable message of the game.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    Mission(Mission),
    ContentRefused(ContentRefused),
    Ready(Ready),
    /// Why the seat was refused: the plane is taken, destroyed, lost its
    /// pilot, not open to humans, or no plane is free. The client may ask
    /// again.
    SeatRefused(String),
    Seated(Box<Seated>),
    Roster(Roster),
    Names(Names),
    /// A HUD line, for example "Mission restarts in 30 seconds".
    Notice(String),
    /// The player ends the mission (client to host).
    Leave,
    Debrief(Box<Debrief>),
    MissionEnded(MissionEnded),
}

fn write_tally(w: &mut BitWriter, t: &Tally) {
    let Tally {
        launched,
        hit,
        damage,
        missed,
        spoofed,
        jammed,
    } = *t;
    for v in [launched, hit, damage, missed, spoofed, jammed] {
        w.write_varint(u64::from(v));
    }
}

fn read_tally(r: &mut BitReader<'_>) -> WireResult<Tally> {
    Ok(Tally {
        launched: read_u32(r)?,
        hit: read_u32(r)?,
        damage: read_u32(r)?,
        missed: read_u32(r)?,
        spoofed: read_u32(r)?,
        jammed: read_u32(r)?,
    })
}

fn write_pilot(w: &mut BitWriter, p: &DebriefPilot) {
    let DebriefPilot {
        status,
        damage,
        landing_grade,
        cause,
        kills,
        friendly_fire,
        air_to_air,
        air_to_ground,
        gun,
        bombs,
        enemy_aam,
        enemy_sam,
        enemy_gun,
        enemy_aaa,
    } = p;
    let status = match status {
        PilotStatus::Alive => 0,
        PilotStatus::Ejected => 1,
        PilotStatus::Dead => 2,
    };
    let _ = w.write_bits(status, 2);
    let _ = w.write_bits(damage.to_bits(), 64);
    bits::write_option(w, *landing_grade, |w, g| w.write_varint(u64::from(g)));
    bits::write_option(w, cause.as_deref(), write_str);
    for k in kills {
        w.write_varint(u64::from(*k));
    }
    w.write_varint(u64::from(*friendly_fire));
    for t in [
        air_to_air,
        air_to_ground,
        gun,
        bombs,
        enemy_aam,
        enemy_sam,
        enemy_gun,
        enemy_aaa,
    ] {
        write_tally(w, t);
    }
}

fn read_pilot(r: &mut BitReader<'_>) -> WireResult<DebriefPilot> {
    let status = match r.read_bits(2)? {
        0 => PilotStatus::Alive,
        1 => PilotStatus::Ejected,
        2 => PilotStatus::Dead,
        _ => return Err(WireError::Invalid("pilot status")),
    };
    let damage = f64::from_bits(r.read_bits(64)?);
    let landing_grade = bits::read_option(r, read_u32)?;
    let cause = bits::read_option(r, read_str)?;
    let mut kills = [0; 10];
    for k in &mut kills {
        *k = read_u32(r)?;
    }
    Ok(DebriefPilot {
        status,
        damage,
        landing_grade,
        cause,
        kills,
        friendly_fire: read_u32(r)?,
        air_to_air: read_tally(r)?,
        air_to_ground: read_tally(r)?,
        gun: read_tally(r)?,
        bombs: read_tally(r)?,
        enemy_aam: read_tally(r)?,
        enemy_sam: read_tally(r)?,
        enemy_gun: read_tally(r)?,
        enemy_aaa: read_tally(r)?,
    })
}

fn aircraft_code(id: AircraftId) -> u64 {
    AircraftId::SELECTABLE
        .iter()
        .position(|a| *a == id)
        .unwrap_or(0) as u64
}

fn read_aircraft(r: &mut BitReader<'_>) -> WireResult<AircraftId> {
    AircraftId::SELECTABLE
        .get(r.read_bits(4)? as usize)
        .copied()
        .ok_or(WireError::Invalid("aircraft"))
}

fn write_roster(w: &mut BitWriter, roster: &Roster) -> WireResult<()> {
    if roster.planes.len() > PLANES_LIMIT {
        return Err(WireError::TooMany {
            what: "planes",
            limit: PLANES_LIMIT,
        });
    }
    write_count(w, roster.planes.len());
    for plane in &roster.planes {
        w.write_varint(u64::from(plane.id));
        w.write_bool(plane.wing.side == Side::Enemy);
        let _ = w.write_bits(u64::from(plane.wing.index), 2);
        let _ = w.write_bits(u64::from(plane.member), 8);
        let _ = w.write_bits(aircraft_code(plane.aircraft), 4);
        match &plane.pilot {
            RosterPilot::Ai => w.write_bool(false),
            RosterPilot::Human { seat, callsign } => {
                w.write_bool(true);
                let _ = w.write_bits(u64::from(*seat), 8);
                write_str(w, callsign);
            }
        }
    }
    Ok(())
}

fn read_roster(r: &mut BitReader<'_>) -> WireResult<Roster> {
    let count = read_count(r, PLANES_LIMIT, "planes")?;
    let mut planes = Vec::with_capacity(count);
    for _ in 0..count {
        let id = read_u32(r)?;
        let side = if r.read_bool()? {
            Side::Enemy
        } else {
            Side::Friendly
        };
        let wing =
            WingId::new(side, r.read_bits(2)? as u8).map_err(|_| WireError::Invalid("wing"))?;
        let member = r.read_bits(8)? as u8;
        let aircraft = read_aircraft(r)?;
        let pilot = if r.read_bool()? {
            RosterPilot::Human {
                seat: r.read_bits(8)? as u8,
                callsign: read_str(r)?,
            }
        } else {
            RosterPilot::Ai
        };
        planes.push(RosterPlane {
            id,
            wing,
            member,
            aircraft,
            pilot,
        });
    }
    Ok(Roster { planes })
}

fn write_loadout(w: &mut BitWriter, loadout: &LoadoutSpec) -> WireResult<()> {
    if loadout.stations.len() > STATIONS_LIMIT {
        return Err(WireError::TooMany {
            what: "stations",
            limit: STATIONS_LIMIT,
        });
    }
    let _ = w.write_bits(loadout.fuel_lbs.to_bits(), 64);
    w.write_bool(loadout.cheat);
    write_count(w, loadout.stations.len());
    for station in &loadout.stations {
        write_str(w, &station.weapon);
        let _ = w.write_bits(u64::from(station.count), 16);
        let _ = w.write_bits(u64::from(station.quantity), 16);
    }
    Ok(())
}

fn read_loadout(r: &mut BitReader<'_>) -> WireResult<LoadoutSpec> {
    let fuel_lbs = f64::from_bits(r.read_bits(64)?);
    let cheat = r.read_bool()?;
    let count = read_count(r, STATIONS_LIMIT, "stations")?;
    let mut stations = Vec::with_capacity(count);
    for _ in 0..count {
        stations.push(StationLoad {
            weapon: read_str(r)?,
            count: r.read_bits(16)? as u16,
            quantity: r.read_bits(16)? as u16,
        });
    }
    Ok(LoadoutSpec {
        fuel_lbs,
        cheat,
        stations,
    })
}

fn write_strings(
    w: &mut BitWriter,
    names: &[String],
    limit: usize,
    what: &'static str,
) -> WireResult<()> {
    if names.len() > limit {
        return Err(WireError::TooMany { what, limit });
    }
    write_count(w, names.len());
    for name in names {
        write_str(w, name);
    }
    Ok(())
}

fn read_strings(
    r: &mut BitReader<'_>,
    limit: usize,
    what: &'static str,
) -> WireResult<Vec<String>> {
    let count = read_count(r, limit, what)?;
    // Each string takes at least its length byte.
    if count > r.bits_remaining() / 8 {
        return Err(tore_codec::CodecError::UnexpectedEnd.into());
    }
    (0..count).map(|_| read_str(r)).collect()
}

impl Message {
    /// The transport's kind byte.
    pub fn kind(&self) -> u8 {
        match self {
            Self::Mission(_) => kind::MISSION,
            Self::ContentRefused(_) => kind::CONTENT_REFUSED,
            Self::Ready(_) => kind::READY,
            Self::SeatRefused(_) => kind::SEAT_REFUSED,
            Self::Seated(_) => kind::SEATED,
            Self::Roster(_) => kind::ROSTER,
            Self::Names(_) => kind::NAMES,
            Self::Notice(_) => kind::NOTICE,
            Self::Leave => kind::LEAVE,
            Self::Debrief(_) => kind::DEBRIEF,
            Self::MissionEnded(_) => kind::MISSION_ENDED,
        }
    }

    /// The body's bytes, at most 64 KB.
    pub fn encode(&self) -> WireResult<Vec<u8>> {
        let mut w = BitWriter::with_capacity(64);
        match self {
            Self::Mission(m) => {
                write_long_str(&mut w, &m.spec);
                if m.manifest.entries.len() > MANIFEST_LIMIT {
                    return Err(WireError::TooMany {
                        what: "manifest entries",
                        limit: MANIFEST_LIMIT,
                    });
                }
                write_count(&mut w, m.manifest.entries.len());
                for entry in &m.manifest.entries {
                    write_str(&mut w, &entry.name);
                    bits::write_option(&mut w, entry.hash, |w, hash| {
                        let _ = w.write_bits(hash, 64);
                    });
                }
                let _ = w.write_bits(u64::from(m.host_tick), 32);
                w.write_varint(m.contrail_sortie);
            }
            Self::ContentRefused(c) => write_strings(&mut w, &c.names, MANIFEST_LIMIT, "names")?,
            Self::Ready(ready) => {
                bits::write_option(&mut w, ready.plane, |w, plane| {
                    w.write_varint(u64::from(plane))
                });
            }
            Self::SeatRefused(text) | Self::Notice(text) => write_str(&mut w, text),
            Self::Seated(s) => {
                let _ = w.write_bits(u64::from(s.seat), 8);
                w.write_varint(u64::from(s.plane));
                let _ = w.write_bits(u64::from(s.tick), 32);
                bits::write_long_bytes(&mut w, &s.exact);
                write_loadout(&mut w, &s.loadout)?;
                write_roster(&mut w, &s.roster)?;
                if s.destroyed.len() > DESTROYED_LIMIT {
                    return Err(WireError::TooMany {
                        what: "destroyed objects",
                        limit: DESTROYED_LIMIT,
                    });
                }
                write_count(&mut w, s.destroyed.len());
                for id in &s.destroyed {
                    w.write_varint(u64::from(*id));
                }
            }
            Self::Roster(roster) => write_roster(&mut w, roster)?,
            Self::Names(names) => {
                let _ = w.write_bits(u64::from(names.first), 16);
                write_strings(&mut w, &names.names, limits::NAMES, "names")?;
            }
            Self::Leave => {}
            Self::Debrief(d) => {
                w.write_bool(d.success);
                if d.objectives.len() > OBJECTIVES_LIMIT {
                    return Err(WireError::TooMany {
                        what: "objectives",
                        limit: OBJECTIVES_LIMIT,
                    });
                }
                write_count(&mut w, d.objectives.len());
                for objective in &d.objectives {
                    let (protect, done, total) = match *objective {
                        DebriefObjective::Destroy { destroyed, total } => (false, destroyed, total),
                        DebriefObjective::Protect { protected, total } => (true, protected, total),
                    };
                    w.write_bool(protect);
                    w.write_varint(u64::from(done));
                    w.write_varint(u64::from(total));
                }
                w.write_varint(d.elapsed_seconds);
                write_pilot(&mut w, &d.player);
                bits::write_option(&mut w, d.wingman.as_ref(), write_pilot);
            }
            Self::MissionEnded(m) => {
                let reason = match m.reason {
                    EndReason::EveryoneLeft => 0,
                    EndReason::TimeLimit => 1,
                    EndReason::ServerStopping => 2,
                    EndReason::EndedByServer => 3,
                };
                let _ = w.write_bits(reason, 2);
                bits::write_option(&mut w, m.next_in_seconds, |w, s| {
                    w.write_varint(u64::from(s))
                });
            }
        }
        let bytes = bits::finish(w);
        if bytes.len() > limits::MESSAGE {
            return Err(WireError::TooMany {
                what: "message bytes",
                limit: limits::MESSAGE,
            });
        }
        Ok(bytes)
    }

    /// Reads a body of the transport's `kind`.
    pub fn decode(kind: u8, body: &[u8]) -> WireResult<Self> {
        if body.len() > limits::MESSAGE {
            return Err(WireError::TooMany {
                what: "message bytes",
                limit: limits::MESSAGE,
            });
        }
        let mut r = BitReader::new(body);
        let r = &mut r;
        let message = match kind {
            kind::MISSION => {
                let spec = read_long_str(r, limits::MESSAGE, "mission text")?;
                let count = read_count(r, MANIFEST_LIMIT, "manifest entries")?;
                if count > r.bits_remaining() / 9 {
                    return Err(tore_codec::CodecError::UnexpectedEnd.into());
                }
                let mut entries = Vec::with_capacity(count);
                for _ in 0..count {
                    entries.push(ManifestEntry {
                        name: read_str(r)?,
                        hash: bits::read_option(r, |r| Ok(r.read_bits(64)?))?,
                    });
                }
                Self::Mission(Mission {
                    spec,
                    manifest: Manifest { entries },
                    host_tick: r.read_bits(32)? as u32,
                    contrail_sortie: r.read_varint()?,
                })
            }
            kind::CONTENT_REFUSED => Self::ContentRefused(ContentRefused {
                names: read_strings(r, MANIFEST_LIMIT, "names")?,
            }),
            kind::READY => Self::Ready(Ready {
                plane: bits::read_option(r, read_u32)?,
            }),
            kind::SEAT_REFUSED => Self::SeatRefused(read_str(r)?),
            kind::SEATED => {
                let seat = r.read_bits(8)? as u8;
                let plane = read_u32(r)?;
                let tick = r.read_bits(32)? as u32;
                let exact = bits::read_long_bytes(r, limits::MESSAGE, "exact state bytes")?;
                let loadout = read_loadout(r)?;
                let roster = read_roster(r)?;
                let count = read_count(r, DESTROYED_LIMIT, "destroyed objects")?;
                if count > r.bits_remaining() / 8 {
                    return Err(tore_codec::CodecError::UnexpectedEnd.into());
                }
                let destroyed = (0..count).map(|_| read_u32(r)).collect::<WireResult<_>>()?;
                Self::Seated(Box::new(Seated {
                    seat,
                    plane,
                    tick,
                    exact,
                    loadout,
                    roster,
                    destroyed,
                }))
            }
            kind::ROSTER => Self::Roster(read_roster(r)?),
            kind::NAMES => {
                let first = r.read_bits(16)? as u16;
                let names = read_strings(r, limits::NAMES, "names")?;
                if usize::from(first) + names.len() > limits::NAMES {
                    return Err(WireError::TooMany {
                        what: "names",
                        limit: limits::NAMES,
                    });
                }
                Self::Names(Names { first, names })
            }
            kind::NOTICE => Self::Notice(read_str(r)?),
            kind::LEAVE => Self::Leave,
            kind::DEBRIEF => {
                let success = r.read_bool()?;
                let count = read_count(r, OBJECTIVES_LIMIT, "objectives")?;
                let mut objectives = Vec::with_capacity(count);
                for _ in 0..count {
                    let protect = r.read_bool()?;
                    let done = read_u32(r)?;
                    let total = read_u32(r)?;
                    objectives.push(if protect {
                        DebriefObjective::Protect {
                            protected: done,
                            total,
                        }
                    } else {
                        DebriefObjective::Destroy {
                            destroyed: done,
                            total,
                        }
                    });
                }
                Self::Debrief(Box::new(Debrief {
                    success,
                    objectives,
                    elapsed_seconds: r.read_varint()?,
                    player: read_pilot(r)?,
                    wingman: bits::read_option(r, read_pilot)?,
                }))
            }
            kind::MISSION_ENDED => {
                let reason = match r.read_bits(2)? {
                    0 => EndReason::EveryoneLeft,
                    1 => EndReason::TimeLimit,
                    2 => EndReason::ServerStopping,
                    _ => EndReason::EndedByServer,
                };
                Self::MissionEnded(MissionEnded {
                    reason,
                    next_in_seconds: bits::read_option(r, read_u32)?,
                })
            }
            _ => return Err(WireError::Invalid("message kind")),
        };
        bits::end(r)?;
        Ok(message)
    }
}
