//! The reliable message bodies (net-protocol.md, "Reliable messages"): the
//! transport carries each as a kind byte and a body of up to 64 KB, split
//! into fragments and put back together in order.
//!
//! Protocol 3 (slice EF4) adds the lobby's: the lobby's state, which the
//! host sends every player whenever it changes; a player's slot, loadout
//! and ready; the King's mission, start, kick and end of the mission; a
//! refused request; the host's goodbye; and the flight number that keeps
//! one flight's sections apart from the next's.

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
    // Protocol 3, the lobby (EF4). Player to host:
    pub const SLOT: u8 = 12;
    pub const LOADOUT: u8 = 13;
    pub const SET_READY: u8 = 14;
    pub const CHANGE_MISSION: u8 = 15;
    pub const START: u8 = 16;
    pub const KICK: u8 = 17;
    pub const END_MISSION: u8 = 18;
    // Host to player:
    pub const LOBBY: u8 = 19;
    pub const REFUSED: u8 = 20;
    pub const GOODBYE: u8 = 21;
    pub const FLIGHT_LOADOUTS: u8 = 22;
    // Kept for the phase 2 lobby, not sent yet: the King passes the crown
    // (23) and changes the lobby's settings (24).
}

/// Entries of a content manifest or a refusal's list.
const MANIFEST_LIMIT: usize = 8_192;
/// Stations of a loadout.
const STATIONS_LIMIT: usize = 64;
/// Planes of a roster.
const PLANES_LIMIT: usize = 256;
/// Destroyed ground objects listed at seating.
const DESTROYED_LIMIT: usize = 8_192;
/// Players in a lobby.
const PLAYERS_LIMIT: usize = 64;
/// Slots in a lobby: the planes of a mission.
const SLOTS_LIMIT: usize = 64;
/// Settings in a lobby state (phase 2).
const SETTINGS_LIMIT: usize = 64;
/// Loadouts at a flight's start: one a plane.
const LOADOUTS_LIMIT: usize = 64;
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
    /// The lobby mission's number on this host, raised with each King's
    /// change (a flight's start sends the mission again, with the players'
    /// loadouts, under the same number): a player's lobby requests name it,
    /// so one meant for an earlier mission is refused (protocol 3).
    pub number: u32,
}

impl Mission {
    /// The spec, parsed.
    pub fn spec(&self) -> Result<MissionSpec, WireError> {
        MissionSpec::from_text(&self.spec).map_err(|_| WireError::Invalid("mission spec"))
    }
}

/// The player's import cannot play the mission (client to host): the
/// resources whose hash differs or which are missing, and the plain reason.
/// In the lobby the player stays, marked unable (protocol 3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentRefused {
    /// The mission's number.
    pub mission: u32,
    pub names: Vec<String>,
    /// Why, in words, for the lobby to show ("Your game data differs ...",
    /// or the build's own error).
    pub reason: String,
}

/// Take a plane (client to host): in the lobby, hold that plane's slot (or
/// the first free one) and mark ready with the loadout chosen, or the
/// standard one; in flight, fly it now, as a player joining a flight does.
/// Stage D's Ready, kind 3, which a game with no lobby screen sends.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TakePlane {
    /// The mission's number.
    pub mission: u32,
    /// The plane wanted, or `None` for any.
    pub plane: Option<u32>,
}

/// What a player asks of its slot (client to host).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotRequest {
    /// Hold this plane's slot.
    Take(u32),
    /// Hold the first free slot, friendly wing 1's lead first.
    Any,
    /// Hold none.
    Leave,
}

/// A player's slot request (client to host).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    pub mission: u32,
    pub request: SlotRequest,
}

/// The loadout for the player's own slot (client to host); `None` is the
/// aircraft's standard load.
#[derive(Clone, Debug, PartialEq)]
pub struct Loadout {
    pub mission: u32,
    /// The slot's plane, which must be the one the player holds.
    pub plane: u32,
    pub loadout: Option<LoadoutSpec>,
}

/// Ready or not (client to host).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetReady {
    pub mission: u32,
    pub ready: bool,
}

/// The King removes a player (client to host).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Kick {
    /// The player's lobby id.
    pub player: u8,
    /// Why, in words, which the player is shown.
    pub reason: String,
}

/// Where the lobby's mission is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyPhase {
    /// Gathering: players take slots, arm and mark ready.
    Lobby,
    /// The mission flies; a player who is ready joins it in flight.
    Flying,
    /// The mission has ended: debriefs go out, then the lobby returns (or
    /// the host stops).
    Ended,
}

/// What starts the mission.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartRule {
    /// The King's Start, once every player holding a slot is ready (a game
    /// a player hosts).
    King,
    /// The first player holding a slot who marks ready (a dedicated
    /// server's `start first-player`).
    FirstReady,
    /// It flies from the start (a dedicated server's `start now`).
    Flying,
}

/// One player in the lobby.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyPlayer {
    /// The player's id in this lobby, kept while connected.
    pub id: u8,
    pub callsign: String,
    /// The plane of the slot the player holds.
    pub slot: Option<u32>,
    pub ready: bool,
    /// The player sent a loadout of its own for the slot.
    pub loadout: bool,
    /// The player flies the mission now.
    pub flying: bool,
    /// Why the player's import cannot play the mission, when it cannot.
    pub unable: Option<String>,
}

/// One slot: a friendly plane of the co-op mission (or whatever the host's
/// open planes are) and who holds it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbySlot {
    pub plane: u32,
    pub wing: WingId,
    pub member: u8,
    pub aircraft: AircraftId,
    /// The id of the player holding it.
    pub holder: Option<u8>,
}

/// The lobby as the host has it (host to client, whenever it changes).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbyState {
    /// The hosting game's name.
    pub name: String,
    /// The mission's one-line summary.
    pub summary: String,
    /// The mission's number (see [`Mission::number`]).
    pub mission: u32,
    pub phase: LobbyPhase,
    pub start: StartRule,
    /// The King's player id; `None` on a dedicated server.
    pub king: Option<u8>,
    /// The id of the player whose machine runs the host (the house); `None`
    /// on a dedicated server.
    pub host: Option<u8>,
    /// The receiving player's own id.
    pub you: u8,
    /// Every player, in the order they connected.
    pub players: Vec<LobbyPlayer>,
    /// Every slot, in plane order.
    pub slots: Vec<LobbySlot>,
    /// The King's settings by number (phase 2); empty now.
    pub settings: Vec<(u8, u32)>,
}

impl LobbyState {
    /// The receiving player's own entry.
    pub fn me(&self) -> Option<&LobbyPlayer> {
        self.player(self.you)
    }

    /// The player with `id`.
    pub fn player(&self, id: u8) -> Option<&LobbyPlayer> {
        self.players.iter().find(|p| p.id == id)
    }

    /// Whether the receiving player is the King.
    pub fn is_king(&self) -> bool {
        self.king == Some(self.you)
    }

    /// The first slot nobody holds, in plane order.
    pub fn first_free_slot(&self) -> Option<u32> {
        self.slots
            .iter()
            .find(|slot| slot.holder.is_none())
            .map(|slot| slot.plane)
    }

    /// Whether every player holding a slot is ready, and at least one does:
    /// when the King's Start is accepted.
    pub fn all_ready(&self) -> bool {
        let holders: Vec<&LobbyPlayer> = self.players.iter().filter(|p| p.slot.is_some()).collect();
        !holders.is_empty() && holders.iter().all(|p| p.ready)
    }
}

/// Why the host says goodbye (host to client, just before it disconnects
/// the player).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Goodbye {
    /// The King removed the player, with the King's words.
    Kicked(String),
    /// The player who hosts the game left it, so the game is over.
    HostLeft,
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
    /// The connection's flight from here on (protocol 3): every Snapshot,
    /// Own state and Inputs section and Names message carries it, and a
    /// new flight starts its baselines, events and names afresh.
    pub flight: u8,
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
    /// The connection's flight the table belongs to (protocol 3).
    pub flight: u8,
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
    /// `restart`), or the King did.
    EndedByServer,
    /// The player who hosts the game left it (protocol 3).
    HostLeft,
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
    TakePlane(TakePlane),
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
    /// A player's slot request (client to host).
    Slot(Slot),
    /// The loadout for the player's slot (client to host).
    Loadout(Box<Loadout>),
    /// Ready or not (client to host).
    SetReady(SetReady),
    /// The King's new mission, its spec text (client to host).
    ChangeMission(String),
    /// The King starts the mission (client to host).
    Start,
    /// The King removes a player (client to host).
    Kick(Kick),
    /// The King ends the mission for everyone (client to host).
    EndMission,
    /// The lobby's state (host to client).
    Lobby(Box<LobbyState>),
    /// A lobby request refused (host to client): the refused message's kind
    /// and why.
    Refused {
        request: u8,
        reason: String,
    },
    /// Why the host is about to disconnect the player (host to client).
    Goodbye(Goodbye),
    /// The mission starts flying with these planes' loadouts (host to
    /// client): every player builds the lobby's mission again with them, so
    /// its copy holds the aircraft as the host flies them.
    FlightLoadouts(Vec<(u32, LoadoutSpec)>),
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

fn write_wing(w: &mut BitWriter, wing: WingId) {
    w.write_bool(wing.side == Side::Enemy);
    let _ = w.write_bits(u64::from(wing.index), 2);
}

fn read_wing(r: &mut BitReader<'_>) -> WireResult<WingId> {
    let side = if r.read_bool()? {
        Side::Enemy
    } else {
        Side::Friendly
    };
    WingId::new(side, r.read_bits(2)? as u8).map_err(|_| WireError::Invalid("wing"))
}

fn write_lobby(w: &mut BitWriter, lobby: &LobbyState) -> WireResult<()> {
    let LobbyState {
        name,
        summary,
        mission,
        phase,
        start,
        king,
        host,
        you,
        players,
        slots,
        settings,
    } = lobby;
    for (count, limit, what) in [
        (players.len(), PLAYERS_LIMIT, "players"),
        (slots.len(), SLOTS_LIMIT, "slots"),
        (settings.len(), SETTINGS_LIMIT, "settings"),
    ] {
        if count > limit {
            return Err(WireError::TooMany { what, limit });
        }
    }
    write_str(w, name);
    write_str(w, summary);
    w.write_varint(u64::from(*mission));
    let phase = match phase {
        LobbyPhase::Lobby => 0,
        LobbyPhase::Flying => 1,
        LobbyPhase::Ended => 2,
    };
    let _ = w.write_bits(phase, 2);
    let start = match start {
        StartRule::King => 0,
        StartRule::FirstReady => 1,
        StartRule::Flying => 2,
    };
    let _ = w.write_bits(start, 2);
    bits::write_option(w, *king, |w, id| {
        let _ = w.write_bits(u64::from(id), 8);
    });
    bits::write_option(w, *host, |w, id| {
        let _ = w.write_bits(u64::from(id), 8);
    });
    let _ = w.write_bits(u64::from(*you), 8);
    write_count(w, players.len());
    for player in players {
        let LobbyPlayer {
            id,
            callsign,
            slot,
            ready,
            loadout,
            flying,
            unable,
        } = player;
        let _ = w.write_bits(u64::from(*id), 8);
        write_str(w, callsign);
        bits::write_option(w, *slot, |w, plane| w.write_varint(u64::from(plane)));
        w.write_bool(*ready);
        w.write_bool(*loadout);
        w.write_bool(*flying);
        bits::write_option(w, unable.as_deref(), write_str);
    }
    write_count(w, slots.len());
    for slot in slots {
        w.write_varint(u64::from(slot.plane));
        write_wing(w, slot.wing);
        let _ = w.write_bits(u64::from(slot.member), 8);
        let _ = w.write_bits(aircraft_code(slot.aircraft), 4);
        bits::write_option(w, slot.holder, |w, id| {
            let _ = w.write_bits(u64::from(id), 8);
        });
    }
    write_count(w, settings.len());
    for (key, value) in settings {
        let _ = w.write_bits(u64::from(*key), 8);
        w.write_varint(u64::from(*value));
    }
    Ok(())
}

fn read_id(r: &mut BitReader<'_>) -> WireResult<u8> {
    Ok(r.read_bits(8)? as u8)
}

fn read_lobby(r: &mut BitReader<'_>) -> WireResult<LobbyState> {
    let name = read_str(r)?;
    let summary = read_str(r)?;
    let mission = read_u32(r)?;
    let phase = match r.read_bits(2)? {
        0 => LobbyPhase::Lobby,
        1 => LobbyPhase::Flying,
        2 => LobbyPhase::Ended,
        _ => return Err(WireError::Invalid("lobby phase")),
    };
    let start = match r.read_bits(2)? {
        0 => StartRule::King,
        1 => StartRule::FirstReady,
        2 => StartRule::Flying,
        _ => return Err(WireError::Invalid("start rule")),
    };
    let king = bits::read_option(r, read_id)?;
    let host = bits::read_option(r, read_id)?;
    let you = read_id(r)?;
    let count = read_count(r, PLAYERS_LIMIT, "players")?;
    let mut players = Vec::with_capacity(count);
    for _ in 0..count {
        players.push(LobbyPlayer {
            id: read_id(r)?,
            callsign: read_str(r)?,
            slot: bits::read_option(r, read_u32)?,
            ready: r.read_bool()?,
            loadout: r.read_bool()?,
            flying: r.read_bool()?,
            unable: bits::read_option(r, read_str)?,
        });
    }
    let count = read_count(r, SLOTS_LIMIT, "slots")?;
    let mut slots = Vec::with_capacity(count);
    for _ in 0..count {
        slots.push(LobbySlot {
            plane: read_u32(r)?,
            wing: read_wing(r)?,
            member: r.read_bits(8)? as u8,
            aircraft: read_aircraft(r)?,
            holder: bits::read_option(r, read_id)?,
        });
    }
    let count = read_count(r, SETTINGS_LIMIT, "settings")?;
    let mut settings = Vec::with_capacity(count);
    for _ in 0..count {
        settings.push((read_id(r)?, read_u32(r)?));
    }
    Ok(LobbyState {
        name,
        summary,
        mission,
        phase,
        start,
        king,
        host,
        you,
        players,
        slots,
        settings,
    })
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
            Self::TakePlane(_) => kind::READY,
            Self::SeatRefused(_) => kind::SEAT_REFUSED,
            Self::Seated(_) => kind::SEATED,
            Self::Roster(_) => kind::ROSTER,
            Self::Names(_) => kind::NAMES,
            Self::Notice(_) => kind::NOTICE,
            Self::Leave => kind::LEAVE,
            Self::Debrief(_) => kind::DEBRIEF,
            Self::MissionEnded(_) => kind::MISSION_ENDED,
            Self::Slot(_) => kind::SLOT,
            Self::Loadout(_) => kind::LOADOUT,
            Self::SetReady(_) => kind::SET_READY,
            Self::ChangeMission(_) => kind::CHANGE_MISSION,
            Self::Start => kind::START,
            Self::Kick(_) => kind::KICK,
            Self::EndMission => kind::END_MISSION,
            Self::Lobby(_) => kind::LOBBY,
            Self::Refused { .. } => kind::REFUSED,
            Self::Goodbye(_) => kind::GOODBYE,
            Self::FlightLoadouts(_) => kind::FLIGHT_LOADOUTS,
        }
    }

    /// Whether a player's game sends this message (the rest are the
    /// host's).
    pub fn from_player(&self) -> bool {
        matches!(
            self,
            Self::ContentRefused(_)
                | Self::TakePlane(_)
                | Self::Leave
                | Self::Slot(_)
                | Self::Loadout(_)
                | Self::SetReady(_)
                | Self::ChangeMission(_)
                | Self::Start
                | Self::Kick(_)
                | Self::EndMission
        )
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
                w.write_varint(u64::from(m.number));
            }
            Self::ContentRefused(c) => {
                w.write_varint(u64::from(c.mission));
                write_strings(&mut w, &c.names, MANIFEST_LIMIT, "names")?;
                write_str(&mut w, &c.reason);
            }
            Self::TakePlane(take) => {
                w.write_varint(u64::from(take.mission));
                bits::write_option(&mut w, take.plane, |w, plane| {
                    w.write_varint(u64::from(plane))
                });
            }
            Self::SeatRefused(text) | Self::Notice(text) => write_str(&mut w, text),
            Self::Seated(s) => {
                let _ = w.write_bits(u64::from(s.flight), 8);
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
                let _ = w.write_bits(u64::from(names.flight), 8);
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
                    EndReason::HostLeft => 4,
                };
                let _ = w.write_bits(reason, 3);
                bits::write_option(&mut w, m.next_in_seconds, |w, s| {
                    w.write_varint(u64::from(s))
                });
            }
            Self::Slot(slot) => {
                w.write_varint(u64::from(slot.mission));
                match slot.request {
                    SlotRequest::Take(plane) => {
                        let _ = w.write_bits(0, 2);
                        w.write_varint(u64::from(plane));
                    }
                    SlotRequest::Any => {
                        let _ = w.write_bits(1, 2);
                    }
                    SlotRequest::Leave => {
                        let _ = w.write_bits(2, 2);
                    }
                }
            }
            Self::Loadout(load) => {
                w.write_varint(u64::from(load.mission));
                w.write_varint(u64::from(load.plane));
                w.write_bool(load.loadout.is_some());
                if let Some(loadout) = &load.loadout {
                    write_loadout(&mut w, loadout)?;
                }
            }
            Self::SetReady(ready) => {
                w.write_varint(u64::from(ready.mission));
                w.write_bool(ready.ready);
            }
            Self::ChangeMission(spec) => write_long_str(&mut w, spec),
            Self::Start | Self::EndMission => {}
            Self::Kick(kick) => {
                let _ = w.write_bits(u64::from(kick.player), 8);
                write_str(&mut w, &kick.reason);
            }
            Self::Lobby(lobby) => write_lobby(&mut w, lobby)?,
            Self::Refused { request, reason } => {
                let _ = w.write_bits(u64::from(*request), 8);
                write_str(&mut w, reason);
            }
            Self::Goodbye(goodbye) => match goodbye {
                Goodbye::Kicked(reason) => {
                    let _ = w.write_bits(0, 2);
                    write_str(&mut w, reason);
                }
                Goodbye::HostLeft => {
                    let _ = w.write_bits(1, 2);
                }
            },
            Self::FlightLoadouts(loadouts) => {
                if loadouts.len() > LOADOUTS_LIMIT {
                    return Err(WireError::TooMany {
                        what: "loadouts",
                        limit: LOADOUTS_LIMIT,
                    });
                }
                write_count(&mut w, loadouts.len());
                for (plane, loadout) in loadouts {
                    w.write_varint(u64::from(*plane));
                    write_loadout(&mut w, loadout)?;
                }
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
                    number: read_u32(r)?,
                })
            }
            kind::CONTENT_REFUSED => Self::ContentRefused(ContentRefused {
                mission: read_u32(r)?,
                names: read_strings(r, MANIFEST_LIMIT, "names")?,
                reason: read_str(r)?,
            }),
            kind::READY => Self::TakePlane(TakePlane {
                mission: read_u32(r)?,
                plane: bits::read_option(r, read_u32)?,
            }),
            kind::SEAT_REFUSED => Self::SeatRefused(read_str(r)?),
            kind::SEATED => {
                let flight = r.read_bits(8)? as u8;
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
                    flight,
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
                let flight = r.read_bits(8)? as u8;
                let first = r.read_bits(16)? as u16;
                let names = read_strings(r, limits::NAMES, "names")?;
                if usize::from(first) + names.len() > limits::NAMES {
                    return Err(WireError::TooMany {
                        what: "names",
                        limit: limits::NAMES,
                    });
                }
                Self::Names(Names {
                    flight,
                    first,
                    names,
                })
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
                let reason = match r.read_bits(3)? {
                    0 => EndReason::EveryoneLeft,
                    1 => EndReason::TimeLimit,
                    2 => EndReason::ServerStopping,
                    3 => EndReason::EndedByServer,
                    4 => EndReason::HostLeft,
                    _ => return Err(WireError::Invalid("end reason")),
                };
                Self::MissionEnded(MissionEnded {
                    reason,
                    next_in_seconds: bits::read_option(r, read_u32)?,
                })
            }
            kind::SLOT => {
                let mission = read_u32(r)?;
                let request = match r.read_bits(2)? {
                    0 => SlotRequest::Take(read_u32(r)?),
                    1 => SlotRequest::Any,
                    2 => SlotRequest::Leave,
                    _ => return Err(WireError::Invalid("slot request")),
                };
                Self::Slot(Slot { mission, request })
            }
            kind::LOADOUT => Self::Loadout(Box::new(Loadout {
                mission: read_u32(r)?,
                plane: read_u32(r)?,
                loadout: bits::read_option(r, read_loadout)?,
            })),
            kind::SET_READY => Self::SetReady(SetReady {
                mission: read_u32(r)?,
                ready: r.read_bool()?,
            }),
            kind::CHANGE_MISSION => {
                Self::ChangeMission(read_long_str(r, limits::MESSAGE, "mission text")?)
            }
            kind::START => Self::Start,
            kind::KICK => Self::Kick(Kick {
                player: read_id(r)?,
                reason: read_str(r)?,
            }),
            kind::END_MISSION => Self::EndMission,
            kind::LOBBY => Self::Lobby(Box::new(read_lobby(r)?)),
            kind::REFUSED => Self::Refused {
                request: read_id(r)?,
                reason: read_str(r)?,
            },
            kind::FLIGHT_LOADOUTS => {
                let count = read_count(r, LOADOUTS_LIMIT, "loadouts")?;
                let mut loadouts = Vec::with_capacity(count);
                for _ in 0..count {
                    loadouts.push((read_u32(r)?, read_loadout(r)?));
                }
                Self::FlightLoadouts(loadouts)
            }
            kind::GOODBYE => Self::Goodbye(match r.read_bits(2)? {
                0 => Goodbye::Kicked(read_str(r)?),
                1 => Goodbye::HostLeft,
                _ => return Err(WireError::Invalid("goodbye")),
            }),
            _ => return Err(WireError::Invalid("message kind")),
        };
        bits::end(r)?;
        Ok(message)
    }
}
