//! The reliable message bodies (net-protocol.md, "Reliable messages"): the
//! transport carries each as a kind byte and a body of up to 64 KB, split
//! into fragments and put back together in order.
//!
//! Protocol 3 (slice EF4) adds the lobby's: the lobby's state, which the
//! host sends every player whenever it changes; a player's slot, loadout
//! and ready; the King's mission, start, kick and end of the mission; a
//! refused request; the host's goodbye; and the flight number that keeps
//! one flight's sections apart from the next's.
//!
//! Protocol 4 (slice EF6) adds chat: a player's line and the host's
//! delivered line ([`super::chat`]).
//!
//! Protocol 7 adds each lobby player's platform ([`Platform`]).
//!
//! Protocol 8 (slice F2-0) adds stage F phase 2's: the King's crown,
//! settings and slot locks, revival, scores, results, observers and the idle
//! aircraft's Away and Back, and the lobby state's settings, locks and
//! players' observing and away marks.
//!
//! Protocol 10 (slice L2) adds stage L's: the player's Content (its
//! Fighters Anthology build, its importer and its items' digests), the
//! host's Content gaps, and each lobby player's build.
//!
//! Protocol 12 (slice J6) adds each lobby player's connection path
//! ([`Path`]).
//!
//! Protocol 13 (slice K0) adds stage K's, host migration and rejoin, whose
//! bodies are [`super::migration`]'s: the rejoin token, the candidates and
//! their reach and upload tests, the succession, the standby stream and its
//! status, the resume with a new host, the handover, the King's release of a
//! reservation and a late rejoin; and the lobby state's standby marks and
//! reserved slots.

use super::bits::{
    self, read_count, read_long_str, read_str, read_u32, write_count, write_long_str, write_str,
};
use super::chat::{ChatLine, ChatSend};
use super::migration::{
    self, Backlog, CandidateReport, HostMoving, ReachPeers, ReachReport, ReachTest, Resume,
    Resumed, StandbyStatus, Succession, TakenOver, TokenGrant, UploadTest,
};
use super::names::ReceivedNames;
use super::{Path, Platform, WireError, WireResult, limits};
use crate::settings::{Fight, KillOwner, Respawn, ScoreTally};
use tore_codec::{BitReader, BitWriter};
use tore_formats::aircraft::AircraftId;
use tore_sim::ai::launch::{Side, WingId};
use tore_sim::combat::ledger::Tally;
use tore_sim::models::AircraftModel;
use tore_world::mission::{LoadoutSpec, MissionSpec, StationLoad, TankLoad};
use tore_world::resources::{Manifest, ManifestEntry};
use tore_world::world::plane::ExactState;
use tore_world::world::revive::Spawn;

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
    // Protocol 8, stage F phase 2 (F2-0): the King passes the crown and
    // changes the lobby's settings (player to host).
    pub const PASS_CROWN: u8 = 23;
    pub const SETTINGS: u8 = 24;
    // Protocol 4, chat (EF6). Player to host, then host to player:
    pub const CHAT_SEND: u8 = 25;
    pub const CHAT_LINE: u8 = 26;
    // Protocol 8, stage F phase 2 (F2-0). Player to host:
    pub const SLOT_LOCK: u8 = 27;
    pub const REVIVE: u8 = 28;
    // Host to player:
    pub const REVIVAL: u8 = 29;
    pub const SPAWNED: u8 = 30;
    pub const SCORES: u8 = 31;
    pub const RESULTS: u8 = 32;
    // Player to host, host to player, then player to host:
    pub const OBSERVE: u8 = 33;
    pub const OBSERVING: u8 = 34;
    pub const AWAY: u8 = 35;
    pub const BACK: u8 = 36;
    // Protocol 10, stage L (L2): the player's content (player to host),
    // then the items not every human can use (host to every player).
    pub const CONTENT: u8 = 37;
    pub const CONTENT_GAPS: u8 = 38;
    // Protocol 13, stage K (K0): host migration and rejoin.
    pub const TOKEN: u8 = 39;
    pub const CANDIDATE: u8 = 40;
    pub const REACH_TEST: u8 = 41;
    pub const REACH_PEERS: u8 = 42;
    pub const REACH_REPORT: u8 = 43;
    pub const UPLOAD_TEST: u8 = 44;
    pub const SUCCESSION: u8 = 45;
    pub const STANDBY_RECORD: u8 = 46;
    pub const STANDBY_STATUS: u8 = 47;
    pub const RESUME: u8 = 48;
    pub const RESUMED: u8 = 49;
    pub const BACKLOG: u8 = 50;
    pub const HOST_MOVING: u8 = 51;
    pub const TAKEN_OVER: u8 = 52;
    pub const RELEASE: u8 = 53;
    pub const REJOIN: u8 = 54;
}

/// The limits of stage L's messages (protocol 10; net-protocol.md, "Limits").
pub mod content_limits {
    /// Items in a Content message.
    pub const ITEMS: usize = 1_024;
    /// Gaps in a Content gaps message.
    pub const GAPS: usize = 1_024;
    /// Players named in one gap.
    pub const GAP_PLAYERS: usize = 64;
    /// An item's key, bytes of printable ASCII.
    pub const KEY_BYTES: usize = 32;
    /// A gap's label, bytes.
    pub const LABEL_BYTES: usize = 64;
    /// The importer's version, and its commit, bytes each.
    pub const IMPORTER_BYTES: usize = 64;
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
/// Settings in a lobby state, and in one King's change (phase 2).
const SETTINGS_LIMIT: usize = 64;
/// Loadouts at a flight's start: one a plane.
const LOADOUTS_LIMIT: usize = 64;
/// Objectives in a debrief.
const OBJECTIVES_LIMIT: usize = 64;
/// Players in a Scores message (phase 2).
const SCORE_PLAYERS_LIMIT: usize = 64;
/// Rows in a Results message: every plane a mission had (phase 2).
const RESULT_ROWS_LIMIT: usize = 1_024;
/// The most lives a Revival message says are left; more are unlimited.
const LIVES_LIMIT: u8 = 10;
/// Thousandths of a result row's damage: a whole aircraft.
const DAMAGE_WHOLE: u16 = 1_000;

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
    /// It was the flight's build with the players' loadouts that failed,
    /// not the lobby's mission: the player may try again after it.
    pub flight: bool,
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
    /// The player watches the flying mission with no plane of its own
    /// (protocol 8).
    pub observing: bool,
    /// The AI flies the player's plane while the player is away
    /// (protocol 8).
    pub away: bool,
    /// Why the player's import cannot play the mission, when it cannot.
    pub unable: Option<String>,
    /// The operating system the player's game runs on, as its game said
    /// when it joined (protocol 7).
    pub platform: Platform,
    /// How the player reached the host: the path of the address it joined
    /// by, as the host kept it from the Challenge answer (protocol 12).
    pub path: Path,
    /// The Fighters Anthology build the player's import came from, as its
    /// Content said; unknown until that arrives (protocol 10).
    pub build: Build,
    /// The player's game is a standby host, first or second (protocol 13).
    pub standby: StandbyMark,
}

/// A lobby player's standby mark (protocol 13): 2 bits, code 3 invalid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StandbyMark {
    #[default]
    None,
    /// The first standby: it takes over when the host is lost.
    First,
    /// The second: it takes over if the first is lost too.
    Second,
}

impl StandbyMark {
    /// The wire's code.
    pub fn code(self) -> u8 {
        match self {
            Self::None => 0,
            Self::First => 1,
            Self::Second => 2,
        }
    }

    /// The mark of a code; `None` for 3.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::None),
            1 => Some(Self::First),
            2 => Some(Self::Second),
            _ => None,
        }
    }
}

/// One slot: a friendly plane of the co-op mission (or whatever the host's
/// open planes are) and who holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LobbySlot {
    pub plane: u32,
    pub wing: WingId,
    pub member: u8,
    pub aircraft: AircraftId,
    /// The id of the player holding it.
    pub holder: Option<u8>,
    /// The King's lock on it (protocol 8).
    pub lock: Lock,
    /// The callsign of the player the plane is reserved for (protocol 13):
    /// a dropped player's plane, or an idle one's, which the AI flies and
    /// nobody else takes.
    pub reserved: Option<String>,
}

/// A slot's lock, which only the King sets (protocol 8).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum Lock {
    /// Any player may take it.
    #[default]
    Open,
    /// The AI flies it; nobody takes it.
    Closed,
    /// Only the player with this callsign takes it.
    Reserved(String),
}

/// The King locks, closes or reserves a slot (client to host, kind 27).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotLock {
    /// The mission's number.
    pub mission: u32,
    pub plane: u32,
    pub lock: Lock,
}

/// The King's change of settings (client to host, kind 24), applied all or
/// none.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SettingsChange {
    /// Settings by number and value ([`crate::settings`]); none when only
    /// the name or the password changes.
    pub values: Vec<(u8, u32)>,
    /// The game's new name.
    pub name: Option<String>,
    /// The password's change.
    pub password: Option<PasswordChange>,
}

/// A change of the game's password.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PasswordChange {
    Clear,
    /// Set it: 1 to 255 bytes.
    Set(String),
}

/// The seat's plane is lost (host to client, kind 29): whether and when the
/// player may fly again.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Revival {
    /// The respawn rule in force.
    pub rule: Respawn,
    /// Revivals left this mission, 0 to 10; `None` for unlimited.
    pub lives: Option<u8>,
    /// Seconds until the player may fly again.
    pub wait_seconds: u32,
    /// Why it waits or cannot, in words ("No lives left.").
    pub why: Option<String>,
}

/// A revival's new plane (host to every player, kind 30), which every
/// client adds to its copy of the mission.
#[derive(Clone, Debug, PartialEq)]
pub struct Spawned {
    pub plane: u32,
    /// The tick it appears at.
    pub tick: u32,
    pub wing: WingId,
    pub member: u8,
    pub aircraft: AircraftId,
    /// Where and how it appears, with its stores.
    pub spawn: Spawn,
}

/// One player's score.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerScore {
    /// The player's lobby id.
    pub id: u8,
    pub callsign: String,
    /// The side it flies for, when it has flown.
    pub side: Option<Side>,
    pub kills: u32,
    pub losses: u32,
    /// Damage to opponents, in thousandths of an aircraft.
    pub damage: u32,
}

/// One side's score.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SideScore {
    pub kills: u32,
    pub losses: u32,
    /// Thousandths of an aircraft.
    pub damage: u32,
}

/// Who has won.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Winner {
    #[default]
    NoneYet,
    Side(Side),
    /// The player with this lobby id.
    Player(u8),
    Draw,
}

/// The scores (host to client, kind 31).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scores {
    pub tally: ScoreTally,
    pub fight: Fight,
    /// Seconds left of the time limit, when there is one.
    pub seconds_left: Option<u32>,
    /// The kill limit, 0 for none (at most 15).
    pub kill_limit: u8,
    pub kill_owner: KillOwner,
    /// At most 64.
    pub players: Vec<PlayerScore>,
    /// The friendly side's, then the enemy side's.
    pub sides: [SideScore; 2],
    pub winner: Winner,
}

/// A result row's status at the end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ResultStatus {
    #[default]
    Alive,
    Ejected,
    Dead,
    /// Taken out of the mission to make room for a revival.
    Retired,
}

/// Shots fired and hits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Shots {
    pub launched: u32,
    pub hit: u32,
}

/// One plane's row of the results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResultRow {
    pub plane: u32,
    pub wing: WingId,
    pub member: u8,
    pub aircraft: AircraftId,
    /// The callsign of its last human pilot; `None` when only the AI flew
    /// it.
    pub callsign: Option<String>,
    pub status: ResultStatus,
    /// Airframe damage in thousandths, 0 to 1,000.
    pub damage: u16,
    pub aircraft_kills: u32,
    pub other_kills: u32,
    pub friendly_fire: u32,
    pub air_to_air: Shots,
    pub gun: Shots,
    pub air_to_ground: Shots,
}

/// Every plane's results at the mission's end (host to client, kind 32).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Results {
    pub reason: EndReason,
    /// At most 1,024.
    pub rows: Vec<ResultRow>,
    /// The final scores, in PvP.
    pub scores: Option<Scores>,
}

/// What an observer's camera follows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Subject {
    None,
    /// An aircraft, by its plane id.
    Aircraft(u32),
    /// A point, whole feet.
    Point([i32; 3]),
}

/// Start or stop watching (client to host, kind 33).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Observe {
    Stop,
    Watch(Subject),
}

/// An observer flight's start (inside [`Observing::Started`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObserverFlight {
    /// The connection's new flight.
    pub flight: u8,
    /// The delay in seconds.
    pub delay_seconds: u8,
    /// The tick the first snapshot will show.
    pub tick: u32,
    pub roster: Roster,
    /// The ground objects destroyed by that tick.
    pub destroyed: Vec<u32>,
}

/// The observer flight starts or ends (host to client, kind 34).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observing {
    Ended,
    Started(ObserverFlight),
}

/// The Fighters Anthology build an import came from (protocol 10): 2 bits,
/// code 3 invalid.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Build {
    /// The import does not say (a pack made before stage L with no import
    /// report), or the player's Content has not arrived yet.
    #[default]
    Unknown,
    /// Fighters Anthology 1.0, the disc.
    V10,
    /// Fighters Anthology 1.02F.
    V102F,
}

impl Build {
    /// The wire's code.
    pub fn code(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::V10 => 1,
            Self::V102F => 2,
        }
    }

    /// The build of a wire code; `None` for code 3 and above.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Unknown),
            1 => Some(Self::V10),
            2 => Some(Self::V102F),
            _ => None,
        }
    }
}

/// The T.O.R.E that made an import: its version and commit, each at most
/// [`content_limits::IMPORTER_BYTES`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Importer {
    pub version: String,
    pub commit: String,
}

/// What a content item is (protocol 10). The order is the wire's, which
/// sorts items by kind first.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ItemKind {
    /// An aircraft, keyed by the creator's selection key (`F18.PT`, `faxx`).
    Aircraft,
    /// A theater, keyed by its code (`UKR`).
    Theater,
    /// A weapon, keyed by its record's name (`AIM9X.JT`).
    Weapon,
    /// The data every mission reads beyond its own items; its key is empty.
    Shared,
}

impl ItemKind {
    /// The wire's 2-bit code.
    pub fn code(self) -> u8 {
        match self {
            Self::Aircraft => 0,
            Self::Theater => 1,
            Self::Weapon => 2,
            Self::Shared => 3,
        }
    }

    /// The kind of a 2-bit code.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Aircraft),
            1 => Some(Self::Theater),
            2 => Some(Self::Weapon),
            3 => Some(Self::Shared),
            _ => None,
        }
    }
}

/// One item of a player's content: its kind, key and digest. Items order
/// by kind, then key (bytewise), as the wire wants them, so sorting a list
/// of distinct items makes it ready to send.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentItem {
    pub kind: ItemKind,
    /// At most [`content_limits::KEY_BYTES`] of printable ASCII; empty for
    /// the shared item and only for it.
    pub key: String,
    /// FNV-1a 64 over the names the item reads and their hashes.
    pub digest: u64,
}

/// The player's content (client to host, kind 37), its first message after
/// Accepted.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Content {
    /// The Fighters Anthology build the import came from.
    pub build: Build,
    /// The T.O.R.E that made the import, when the import says.
    pub importer: Option<Importer>,
    /// 1 to 1,024 items, sorted by kind and then key, no key twice in a
    /// kind.
    pub items: Vec<ContentItem>,
}

/// A player named in a gap.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GapPlayer {
    /// The player's lobby id.
    pub id: u8,
    /// The player has the item with another digest than the host's; `false`
    /// when it lacks the item.
    pub differs: bool,
}

/// An item not every human can use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Gap {
    pub kind: ItemKind,
    /// As [`ContentItem::key`].
    pub key: String,
    /// The host's name for the item, at most
    /// [`content_limits::LABEL_BYTES`]; empty when the host lacks it.
    pub label: String,
    /// The host's own import lacks the item.
    pub host_lacks: bool,
    /// The players who cannot use it, by lobby id ascending, at most 64;
    /// at least one unless the host lacks the item.
    pub players: Vec<GapPlayer>,
}

/// The items not every human can use (host to every player, kind 38); each
/// replaces the last.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ContentGaps {
    /// The build the host's own import came from.
    pub host_build: Build,
    /// The T.O.R.E that made the host's import, when it says.
    pub host_importer: Option<Importer>,
    /// At most 1,024, in Content's order.
    pub gaps: Vec<Gap>,
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
    /// The kill limit was reached (protocol 8, stage F phase 2).
    KillLimit,
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
    FlightLoadouts(FlightLoadouts),
    /// A player's chat line (client to host, protocol 4).
    ChatSend(ChatSend),
    /// A chat line the host delivers (host to client, protocol 4): another
    /// player's, the reader's own sent back, or the host's words to it.
    ChatLine(ChatLine),
    // Protocol 8, stage F phase 2 (docs/formats/net-protocol.md, "Phase 2").
    /// The King gives the crown to the player with this lobby id (client to
    /// host).
    PassCrown(u8),
    /// The King changes the lobby's settings (client to host).
    Settings(Box<SettingsChange>),
    /// The King locks, closes or reserves a slot (client to host).
    SlotLock(Box<SlotLock>),
    /// Fly again after a loss, by the respawn rule (client to host): the
    /// mission's number. Answered by a Seated message or a Refused.
    Revive {
        mission: u32,
    },
    /// The seat's plane is lost (host to client).
    Revival(Box<Revival>),
    /// A revival's new plane (host to every player).
    Spawned(Box<Spawned>),
    /// The scores (host to client).
    Scores(Box<Scores>),
    /// Every plane's results at the mission's end (host to client).
    Results(Box<Results>),
    /// Start or stop watching (client to host).
    Observe(Observe),
    /// The observer flight starts or ends (host to client).
    Observing(Box<Observing>),
    /// The game has been away for the `idle-ai` setting's seconds (client
    /// to host).
    Away,
    /// The player touched the flight controls: take the plane back (client
    /// to host). Answered by a Seated message or a Refused.
    Back,
    // Protocol 10, stage L (docs/formats/net-protocol.md, "Compatibility").
    /// The player's content, its first message after Accepted (client to
    /// host).
    Content(Box<Content>),
    /// The items not every human can use (host to every player).
    ContentGaps(Box<ContentGaps>),
    // Protocol 13, stage K (docs/formats/net-protocol.md, "Host migration
    // and rejoin"); the bodies are `super::migration`'s.
    /// The player's rejoin token (host to player).
    Token(TokenGrant),
    /// What the player's game can do as a host (player to host).
    Candidate(Box<CandidateReport>),
    /// Try these candidates (host to player).
    ReachTest(Box<ReachTest>),
    /// Open the router to these players (host to a candidate).
    ReachPeers(Box<ReachPeers>),
    /// A reach test's results (player to host).
    ReachReport(Box<ReachReport>),
    /// Send a paced burst of Filler (host to player).
    UploadTest(UploadTest),
    /// The ready standbys in order (host to every player).
    Succession(Box<Succession>),
    /// One record of the standby stream, as [`crate::journal`] coded it
    /// (host to a standby).
    StandbyRecord(Vec<u8>),
    /// A standby's report (standby to host).
    StandbyStatus(StandbyStatus),
    /// A player resumes with the new host (player to host).
    Resume(Resume),
    /// The new host's answer to Resume (host to player).
    Resumed(Box<Resumed>),
    /// The resumed player's inputs from the takeover on (player to host).
    Backlog(Box<Backlog>),
    /// The host hands over (host to every player).
    HostMoving(HostMoving),
    /// The new host tells the old one, from its own client's old
    /// connection (player to host).
    TakenOver(TakenOver),
    /// The King ends the reservation of this plane (player to host).
    Release(u32),
    /// A player that joined without its token sends it (player to host).
    Rejoin(tore_net::Token),
}

/// A flight's loadouts and what they add to the content check.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FlightLoadouts {
    /// Each loaded plane and its loadout.
    pub loadouts: Vec<(u32, LoadoutSpec)>,
    /// The resources the flight's build read that the lobby's mission did
    /// not (a loadout's other weapons), with their hashes: a player
    /// compares its own as at the mission's content check.
    pub manifest: Manifest,
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
        .get(r.read_bits(6)? as usize)
        .copied()
        .ok_or(WireError::Invalid("aircraft"))
}

fn write_manifest(w: &mut BitWriter, manifest: &Manifest) -> WireResult<()> {
    if manifest.entries.len() > MANIFEST_LIMIT {
        return Err(WireError::TooMany {
            what: "manifest entries",
            limit: MANIFEST_LIMIT,
        });
    }
    write_count(w, manifest.entries.len());
    for entry in &manifest.entries {
        write_str(w, &entry.name);
        bits::write_option(w, entry.hash, |w, hash| {
            let _ = w.write_bits(hash, 64);
        });
    }
    Ok(())
}

fn read_manifest(r: &mut BitReader<'_>) -> WireResult<Manifest> {
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
    Ok(Manifest { entries })
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
            observing,
            away,
            unable,
            platform,
            path,
            build,
            standby,
        } = player;
        let _ = w.write_bits(u64::from(*id), 8);
        write_str(w, callsign);
        bits::write_option(w, *slot, |w, plane| w.write_varint(u64::from(plane)));
        w.write_bool(*ready);
        w.write_bool(*loadout);
        w.write_bool(*flying);
        w.write_bool(*observing);
        w.write_bool(*away);
        // The standby mark follows the away bit (protocol 13).
        let _ = w.write_bits(u64::from(standby.code()), 2);
        bits::write_option(w, unable.as_deref(), write_str);
        let _ = w.write_bits(u64::from(platform.code()), PLATFORM_BITS);
        let _ = w.write_bits(u64::from(path.code()), Path::BITS);
        write_build(w, *build);
    }
    write_count(w, slots.len());
    for slot in slots {
        let LobbySlot {
            plane,
            wing,
            member,
            aircraft,
            holder,
            lock,
            reserved,
        } = slot;
        w.write_varint(u64::from(*plane));
        write_wing(w, *wing);
        let _ = w.write_bits(u64::from(*member), 8);
        let _ = w.write_bits(aircraft_code(*aircraft), 6);
        bits::write_option(w, *holder, |w, id| {
            let _ = w.write_bits(u64::from(id), 8);
        });
        write_lock(w, lock);
        bits::write_option(w, reserved.as_deref(), write_str);
    }
    write_count(w, settings.len());
    for (key, value) in settings {
        let _ = w.write_bits(u64::from(*key), 8);
        w.write_varint(u64::from(*value));
    }
    Ok(())
}

/// A lobby player's platform code: 3 bits, codes 0 to 7, of which the
/// protocol names 0 to 3 (unknown, Windows, macOS, Linux).
const PLATFORM_BITS: u32 = 3;

fn read_platform(r: &mut BitReader<'_>) -> WireResult<Platform> {
    Platform::from_code(r.read_bits(PLATFORM_BITS)? as u8).ok_or(WireError::Invalid("platform"))
}

/// A lobby player's connection path: 3 bits, the master's codes 0 to 5 (a
/// local network, an address, a mapped port, IPv6, punched, the relay).
fn read_path(r: &mut BitReader<'_>) -> WireResult<Path> {
    Path::from_code(r.read_bits(Path::BITS)?).ok_or(WireError::Invalid("path"))
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
            observing: r.read_bool()?,
            away: r.read_bool()?,
            standby: StandbyMark::from_code(r.read_bits(2)? as u8)
                .ok_or(WireError::Invalid("standby mark"))?,
            unable: bits::read_option(r, read_str)?,
            platform: read_platform(r)?,
            path: read_path(r)?,
            build: read_build(r)?,
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
            lock: read_lock(r)?,
            reserved: bits::read_option(r, read_str)?,
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
        let _ = w.write_bits(aircraft_code(plane.aircraft), 6);
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
    w.write_bool(loadout.tanks.is_some());
    if let Some(tanks) = &loadout.tanks {
        if tanks.len() > 9 {
            return Err(WireError::TooMany {
                what: "tanks",
                limit: 9,
            });
        }
        write_count(w, tanks.len());
        for tank in tanks {
            let _ = w.write_bits(u64::from(tank.hardpoint), 8);
            write_str(w, &tank.tank);
            let _ = w.write_bits(u64::from(tank.quantity), 16);
        }
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
    let tanks = if r.read_bool()? {
        let count = read_count(r, 9, "tanks")?;
        let mut tanks = Vec::with_capacity(count);
        for _ in 0..count {
            tanks.push(TankLoad {
                hardpoint: r.read_bits(8)? as u8,
                tank: read_str(r)?,
                quantity: r.read_bits(16)? as u16,
            });
        }
        Some(tanks)
    } else {
        None
    };
    Ok(LoadoutSpec {
        tanks,
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

// ----- Protocol 8: stage F phase 2 -------------------------------------

pub(crate) fn write_end_reason(w: &mut BitWriter, reason: EndReason) {
    let reason = match reason {
        EndReason::EveryoneLeft => 0,
        EndReason::TimeLimit => 1,
        EndReason::ServerStopping => 2,
        EndReason::EndedByServer => 3,
        EndReason::HostLeft => 4,
        EndReason::KillLimit => 5,
    };
    let _ = w.write_bits(reason, 3);
}

pub(crate) fn read_end_reason(r: &mut BitReader<'_>) -> WireResult<EndReason> {
    Ok(match r.read_bits(3)? {
        0 => EndReason::EveryoneLeft,
        1 => EndReason::TimeLimit,
        2 => EndReason::ServerStopping,
        3 => EndReason::EndedByServer,
        4 => EndReason::HostLeft,
        5 => EndReason::KillLimit,
        _ => return Err(WireError::Invalid("end reason")),
    })
}

fn write_lock(w: &mut BitWriter, lock: &Lock) {
    match lock {
        Lock::Open => {
            let _ = w.write_bits(0, 2);
        }
        Lock::Closed => {
            let _ = w.write_bits(1, 2);
        }
        Lock::Reserved(callsign) => {
            let _ = w.write_bits(2, 2);
            write_str(w, callsign);
        }
    }
}

fn read_lock(r: &mut BitReader<'_>) -> WireResult<Lock> {
    Ok(match r.read_bits(2)? {
        0 => Lock::Open,
        1 => Lock::Closed,
        2 => Lock::Reserved(read_str(r)?),
        _ => return Err(WireError::Invalid("slot lock")),
    })
}

fn write_side(w: &mut BitWriter, side: Side) {
    w.write_bool(side == Side::Enemy);
}

fn read_side(r: &mut BitReader<'_>) -> WireResult<Side> {
    Ok(if r.read_bool()? {
        Side::Enemy
    } else {
        Side::Friendly
    })
}

/// A choice of `bits` bits, refused when the code names none.
fn read_choice<T>(
    r: &mut BitReader<'_>,
    bits: u32,
    what: &'static str,
    from: impl FnOnce(u32) -> Option<T>,
) -> WireResult<T> {
    from(r.read_bits(bits)? as u32).ok_or(WireError::Invalid(what))
}

fn write_settings_change(w: &mut BitWriter, change: &SettingsChange) -> WireResult<()> {
    if change.values.len() > SETTINGS_LIMIT {
        return Err(WireError::TooMany {
            what: "settings",
            limit: SETTINGS_LIMIT,
        });
    }
    write_count(w, change.values.len());
    for (number, value) in &change.values {
        let _ = w.write_bits(u64::from(*number), 8);
        w.write_varint(u64::from(*value));
    }
    bits::write_option(w, change.name.as_deref(), write_str);
    match &change.password {
        None => w.write_bool(false),
        Some(PasswordChange::Clear) => {
            w.write_bool(true);
            w.write_bool(false);
        }
        Some(PasswordChange::Set(password)) => {
            if password.is_empty() {
                return Err(WireError::Invalid("password"));
            }
            w.write_bool(true);
            w.write_bool(true);
            write_str(w, password);
        }
    }
    Ok(())
}

fn read_settings_change(r: &mut BitReader<'_>) -> WireResult<SettingsChange> {
    let count = read_count(r, SETTINGS_LIMIT, "settings")?;
    let mut values = Vec::with_capacity(count);
    for _ in 0..count {
        values.push((read_id(r)?, read_u32(r)?));
    }
    let name = bits::read_option(r, read_str)?;
    let password = bits::read_option(r, |r| {
        if !r.read_bool()? {
            return Ok(PasswordChange::Clear);
        }
        let password = read_str(r)?;
        if password.is_empty() {
            return Err(WireError::Invalid("password"));
        }
        Ok(PasswordChange::Set(password))
    })?;
    Ok(SettingsChange {
        values,
        name,
        password,
    })
}

fn write_revival(w: &mut BitWriter, revival: &Revival) -> WireResult<()> {
    let _ = w.write_bits(u64::from(revival.rule.value()), 2);
    match revival.lives {
        None => w.write_bool(true),
        Some(lives) if lives <= LIVES_LIMIT => {
            w.write_bool(false);
            let _ = w.write_bits(u64::from(lives), 4);
        }
        Some(_) => return Err(WireError::Invalid("lives")),
    }
    w.write_varint(u64::from(revival.wait_seconds));
    bits::write_option(w, revival.why.as_deref(), write_str);
    Ok(())
}

fn read_revival(r: &mut BitReader<'_>) -> WireResult<Revival> {
    let rule = read_choice(r, 2, "respawn rule", Respawn::from_value)?;
    let lives = if r.read_bool()? {
        None
    } else {
        let lives = r.read_bits(4)? as u8;
        if lives > LIVES_LIMIT {
            return Err(WireError::Invalid("lives"));
        }
        Some(lives)
    };
    Ok(Revival {
        rule,
        lives,
        wait_seconds: read_u32(r)?,
        why: bits::read_option(r, read_str)?,
    })
}

fn write_spawned(w: &mut BitWriter, spawned: &Spawned) -> WireResult<()> {
    w.write_varint(u64::from(spawned.plane));
    let _ = w.write_bits(u64::from(spawned.tick), 32);
    write_wing(w, spawned.wing);
    let _ = w.write_bits(u64::from(spawned.member), 8);
    let _ = w.write_bits(aircraft_code(spawned.aircraft), 6);
    let spawn = &spawned.spawn;
    for value in spawn
        .position
        .iter()
        .chain([&spawn.heading_rad, &spawn.speed_fps])
    {
        let _ = w.write_bits(value.to_bits(), 64);
    }
    write_loadout(w, &spawn.loadout)
}

fn read_spawned(r: &mut BitReader<'_>) -> WireResult<Spawned> {
    let plane = read_u32(r)?;
    let tick = r.read_bits(32)? as u32;
    let wing = read_wing(r)?;
    let member = r.read_bits(8)? as u8;
    let aircraft = read_aircraft(r)?;
    let mut float = || -> WireResult<f64> { Ok(f64::from_bits(r.read_bits(64)?)) };
    let position = [float()?, float()?, float()?];
    let heading_rad = float()?;
    let speed_fps = float()?;
    Ok(Spawned {
        plane,
        tick,
        wing,
        member,
        aircraft,
        spawn: Spawn {
            position,
            heading_rad,
            speed_fps,
            loadout: read_loadout(r)?,
        },
    })
}

fn write_scores(w: &mut BitWriter, scores: &Scores) -> WireResult<()> {
    let Scores {
        tally,
        fight,
        seconds_left,
        kill_limit,
        kill_owner,
        players,
        sides,
        winner,
    } = scores;
    if players.len() > SCORE_PLAYERS_LIMIT {
        return Err(WireError::TooMany {
            what: "score players",
            limit: SCORE_PLAYERS_LIMIT,
        });
    }
    if *kill_limit > 15 {
        return Err(WireError::Invalid("kill limit"));
    }
    let _ = w.write_bits(u64::from(tally.value()), 2);
    let _ = w.write_bits(u64::from(fight.value()), 1);
    bits::write_option(w, *seconds_left, |w, s| w.write_varint(u64::from(s)));
    let _ = w.write_bits(u64::from(*kill_limit), 4);
    let _ = w.write_bits(u64::from(kill_owner.value()), 2);
    write_count(w, players.len());
    for player in players {
        let _ = w.write_bits(u64::from(player.id), 8);
        write_str(w, &player.callsign);
        bits::write_option(w, player.side, write_side);
        for value in [player.kills, player.losses, player.damage] {
            w.write_varint(u64::from(value));
        }
    }
    for side in sides {
        for value in [side.kills, side.losses, side.damage] {
            w.write_varint(u64::from(value));
        }
    }
    match winner {
        Winner::NoneYet => {
            let _ = w.write_bits(0, 2);
        }
        Winner::Side(side) => {
            let _ = w.write_bits(1, 2);
            write_side(w, *side);
        }
        Winner::Player(id) => {
            let _ = w.write_bits(2, 2);
            let _ = w.write_bits(u64::from(*id), 8);
        }
        Winner::Draw => {
            let _ = w.write_bits(3, 2);
        }
    }
    Ok(())
}

fn read_scores(r: &mut BitReader<'_>) -> WireResult<Scores> {
    let tally = read_choice(r, 2, "tally", ScoreTally::from_value)?;
    let fight = read_choice(r, 1, "fight", Fight::from_value)?;
    let seconds_left = bits::read_option(r, read_u32)?;
    let kill_limit = r.read_bits(4)? as u8;
    let kill_owner = read_choice(r, 2, "kill owner", KillOwner::from_value)?;
    let count = read_count(r, SCORE_PLAYERS_LIMIT, "score players")?;
    let mut players = Vec::with_capacity(count);
    for _ in 0..count {
        players.push(PlayerScore {
            id: read_id(r)?,
            callsign: read_str(r)?,
            side: bits::read_option(r, read_side)?,
            kills: read_u32(r)?,
            losses: read_u32(r)?,
            damage: read_u32(r)?,
        });
    }
    let mut sides = [SideScore::default(); 2];
    for side in &mut sides {
        *side = SideScore {
            kills: read_u32(r)?,
            losses: read_u32(r)?,
            damage: read_u32(r)?,
        };
    }
    let winner = match r.read_bits(2)? {
        0 => Winner::NoneYet,
        1 => Winner::Side(read_side(r)?),
        2 => Winner::Player(read_id(r)?),
        _ => Winner::Draw,
    };
    Ok(Scores {
        tally,
        fight,
        seconds_left,
        kill_limit,
        kill_owner,
        players,
        sides,
        winner,
    })
}

fn write_results(w: &mut BitWriter, results: &Results) -> WireResult<()> {
    if results.rows.len() > RESULT_ROWS_LIMIT {
        return Err(WireError::TooMany {
            what: "result rows",
            limit: RESULT_ROWS_LIMIT,
        });
    }
    write_end_reason(w, results.reason);
    write_count(w, results.rows.len());
    for row in &results.rows {
        if row.damage > DAMAGE_WHOLE {
            return Err(WireError::Invalid("result damage"));
        }
        w.write_varint(u64::from(row.plane));
        write_wing(w, row.wing);
        let _ = w.write_bits(u64::from(row.member), 8);
        let _ = w.write_bits(aircraft_code(row.aircraft), 6);
        bits::write_option(w, row.callsign.as_deref(), write_str);
        let status = match row.status {
            ResultStatus::Alive => 0,
            ResultStatus::Ejected => 1,
            ResultStatus::Dead => 2,
            ResultStatus::Retired => 3,
        };
        let _ = w.write_bits(status, 2);
        let _ = w.write_bits(u64::from(row.damage), 10);
        for value in [
            row.aircraft_kills,
            row.other_kills,
            row.friendly_fire,
            row.air_to_air.launched,
            row.air_to_air.hit,
            row.gun.launched,
            row.gun.hit,
            row.air_to_ground.launched,
            row.air_to_ground.hit,
        ] {
            w.write_varint(u64::from(value));
        }
    }
    w.write_bool(results.scores.is_some());
    if let Some(scores) = &results.scores {
        write_scores(w, scores)?;
    }
    Ok(())
}

fn read_shots(r: &mut BitReader<'_>) -> WireResult<Shots> {
    Ok(Shots {
        launched: read_u32(r)?,
        hit: read_u32(r)?,
    })
}

fn read_results(r: &mut BitReader<'_>) -> WireResult<Results> {
    let reason = read_end_reason(r)?;
    let count = read_count(r, RESULT_ROWS_LIMIT, "result rows")?;
    // Each row takes at least 36 bits.
    if count > r.bits_remaining() / 36 {
        return Err(tore_codec::CodecError::UnexpectedEnd.into());
    }
    let mut rows = Vec::with_capacity(count);
    for _ in 0..count {
        let plane = read_u32(r)?;
        let wing = read_wing(r)?;
        let member = r.read_bits(8)? as u8;
        let aircraft = read_aircraft(r)?;
        let callsign = bits::read_option(r, read_str)?;
        let status = match r.read_bits(2)? {
            0 => ResultStatus::Alive,
            1 => ResultStatus::Ejected,
            2 => ResultStatus::Dead,
            _ => ResultStatus::Retired,
        };
        let damage = r.read_bits(10)? as u16;
        if damage > DAMAGE_WHOLE {
            return Err(WireError::Invalid("result damage"));
        }
        rows.push(ResultRow {
            plane,
            wing,
            member,
            aircraft,
            callsign,
            status,
            damage,
            aircraft_kills: read_u32(r)?,
            other_kills: read_u32(r)?,
            friendly_fire: read_u32(r)?,
            air_to_air: read_shots(r)?,
            gun: read_shots(r)?,
            air_to_ground: read_shots(r)?,
        });
    }
    Ok(Results {
        reason,
        rows,
        scores: bits::read_option(r, read_scores)?,
    })
}

fn write_observe(w: &mut BitWriter, observe: Observe) {
    match observe {
        Observe::Stop => w.write_bool(false),
        Observe::Watch(subject) => {
            w.write_bool(true);
            match subject {
                Subject::None => {
                    let _ = w.write_bits(0, 2);
                }
                Subject::Aircraft(plane) => {
                    let _ = w.write_bits(1, 2);
                    w.write_varint(u64::from(plane));
                }
                Subject::Point(point) => {
                    let _ = w.write_bits(2, 2);
                    for value in point {
                        w.write_varint_signed(i64::from(value));
                    }
                }
            }
        }
    }
}

fn read_observe(r: &mut BitReader<'_>) -> WireResult<Observe> {
    if !r.read_bool()? {
        return Ok(Observe::Stop);
    }
    Ok(Observe::Watch(match r.read_bits(2)? {
        0 => Subject::None,
        1 => Subject::Aircraft(read_u32(r)?),
        2 => Subject::Point([bits::read_i32(r)?, bits::read_i32(r)?, bits::read_i32(r)?]),
        _ => return Err(WireError::Invalid("observer subject")),
    }))
}

pub(super) fn write_destroyed(w: &mut BitWriter, destroyed: &[u32]) -> WireResult<()> {
    if destroyed.len() > DESTROYED_LIMIT {
        return Err(WireError::TooMany {
            what: "destroyed objects",
            limit: DESTROYED_LIMIT,
        });
    }
    write_count(w, destroyed.len());
    for id in destroyed {
        w.write_varint(u64::from(*id));
    }
    Ok(())
}

pub(super) fn read_destroyed(r: &mut BitReader<'_>) -> WireResult<Vec<u32>> {
    let count = read_count(r, DESTROYED_LIMIT, "destroyed objects")?;
    if count > r.bits_remaining() / 8 {
        return Err(tore_codec::CodecError::UnexpectedEnd.into());
    }
    (0..count).map(|_| read_u32(r)).collect()
}

fn write_observing(w: &mut BitWriter, observing: &Observing) -> WireResult<()> {
    match observing {
        Observing::Ended => w.write_bool(false),
        Observing::Started(start) => {
            w.write_bool(true);
            let _ = w.write_bits(u64::from(start.flight), 8);
            let _ = w.write_bits(u64::from(start.delay_seconds), 8);
            let _ = w.write_bits(u64::from(start.tick), 32);
            write_roster(w, &start.roster)?;
            write_destroyed(w, &start.destroyed)?;
        }
    }
    Ok(())
}

fn read_observing(r: &mut BitReader<'_>) -> WireResult<Observing> {
    if !r.read_bool()? {
        return Ok(Observing::Ended);
    }
    Ok(Observing::Started(ObserverFlight {
        flight: r.read_bits(8)? as u8,
        delay_seconds: r.read_bits(8)? as u8,
        tick: r.read_bits(32)? as u32,
        roster: read_roster(r)?,
        destroyed: read_destroyed(r)?,
    }))
}

// ----- Protocol 10: stage L, compatibility ------------------------------

fn write_build(w: &mut BitWriter, build: Build) {
    let _ = w.write_bits(u64::from(build.code()), 2);
}

fn read_build(r: &mut BitReader<'_>) -> WireResult<Build> {
    Build::from_code(r.read_bits(2)? as u8).ok_or(WireError::Invalid("build"))
}

/// A text field of at most `limit` bytes, refused by the writer and the
/// reader alike when longer.
fn check_len(text: &str, limit: usize, what: &'static str) -> WireResult<()> {
    if text.len() > limit {
        return Err(WireError::TooMany { what, limit });
    }
    Ok(())
}

fn check_importer(importer: &Importer) -> WireResult<()> {
    check_len(
        &importer.version,
        content_limits::IMPORTER_BYTES,
        "importer version bytes",
    )?;
    check_len(
        &importer.commit,
        content_limits::IMPORTER_BYTES,
        "importer commit bytes",
    )
}

fn write_importer(w: &mut BitWriter, importer: Option<&Importer>) -> WireResult<()> {
    if let Some(importer) = importer {
        check_importer(importer)?;
    }
    bits::write_option(w, importer, |w, importer| {
        write_str(w, &importer.version);
        write_str(w, &importer.commit);
    });
    Ok(())
}

fn read_importer(r: &mut BitReader<'_>) -> WireResult<Option<Importer>> {
    bits::read_option(r, |r| {
        let importer = Importer {
            version: read_str(r)?,
            commit: read_str(r)?,
        };
        check_importer(&importer)?;
        Ok(importer)
    })
}

/// An item's key: at most 32 bytes of printable ASCII, empty for the
/// shared item and only for it.
fn check_key(kind: ItemKind, key: &str) -> WireResult<()> {
    check_len(key, content_limits::KEY_BYTES, "key bytes")?;
    if !key.bytes().all(|b| (b' '..=b'~').contains(&b)) {
        return Err(WireError::Invalid("content key"));
    }
    if (kind == ItemKind::Shared) != key.is_empty() {
        return Err(WireError::Invalid("content key"));
    }
    Ok(())
}

/// Items and gaps come sorted by kind, then key, each once: `previous` is
/// the last one's.
fn check_order(previous: Option<(ItemKind, &str)>, kind: ItemKind, key: &str) -> WireResult<()> {
    if previous.is_some_and(|previous| previous >= (kind, key)) {
        return Err(WireError::Invalid("content order"));
    }
    Ok(())
}

fn write_item_key(w: &mut BitWriter, kind: ItemKind, key: &str) {
    let _ = w.write_bits(u64::from(kind.code()), 2);
    write_str(w, key);
}

fn read_item_key(r: &mut BitReader<'_>) -> WireResult<(ItemKind, String)> {
    let kind = ItemKind::from_code(r.read_bits(2)? as u8).ok_or(WireError::Invalid("item kind"))?;
    let key = read_str(r)?;
    check_key(kind, &key)?;
    Ok((kind, key))
}

/// The fewest bits an item takes: its kind, its key's length and its digest.
const ITEM_BITS: usize = 2 + 8 + 64;
/// The fewest bits a gap takes: its kind, its key's and label's lengths,
/// the host's bit and the players' count.
const GAP_BITS: usize = 2 + 8 + 8 + 1 + 8;

fn write_content(w: &mut BitWriter, content: &Content) -> WireResult<()> {
    let Content {
        build,
        importer,
        items,
    } = content;
    if items.len() > content_limits::ITEMS {
        return Err(WireError::TooMany {
            what: "content items",
            limit: content_limits::ITEMS,
        });
    }
    if items.is_empty() {
        return Err(WireError::Invalid("content items"));
    }
    let mut previous = None;
    for item in items {
        check_key(item.kind, &item.key)?;
        check_order(previous, item.kind, &item.key)?;
        previous = Some((item.kind, item.key.as_str()));
    }
    write_build(w, *build);
    write_importer(w, importer.as_ref())?;
    write_count(w, items.len());
    for item in items {
        write_item_key(w, item.kind, &item.key);
        let _ = w.write_bits(item.digest, 64);
    }
    Ok(())
}

fn read_content(r: &mut BitReader<'_>) -> WireResult<Content> {
    let build = read_build(r)?;
    let importer = read_importer(r)?;
    let count = read_count(r, content_limits::ITEMS, "content items")?;
    if count == 0 {
        return Err(WireError::Invalid("content items"));
    }
    if count > r.bits_remaining() / ITEM_BITS {
        return Err(tore_codec::CodecError::UnexpectedEnd.into());
    }
    let mut items: Vec<ContentItem> = Vec::with_capacity(count);
    for _ in 0..count {
        let (kind, key) = read_item_key(r)?;
        check_order(
            items.last().map(|item| (item.kind, item.key.as_str())),
            kind,
            &key,
        )?;
        items.push(ContentItem {
            kind,
            key,
            digest: r.read_bits(64)?,
        });
    }
    Ok(Content {
        build,
        importer,
        items,
    })
}

/// A gap's own rules: its key, its label's length (empty when the host
/// lacks the item), its players (at most 64, by id ascending, at least one
/// unless the host lacks the item).
fn check_gap(gap: &Gap) -> WireResult<()> {
    check_key(gap.kind, &gap.key)?;
    check_len(&gap.label, content_limits::LABEL_BYTES, "label bytes")?;
    if gap.host_lacks && !gap.label.is_empty() {
        return Err(WireError::Invalid("gap label"));
    }
    check_gap_players(gap.host_lacks, &gap.players)
}

fn check_gap_players(host_lacks: bool, players: &[GapPlayer]) -> WireResult<()> {
    if players.len() > content_limits::GAP_PLAYERS {
        return Err(WireError::TooMany {
            what: "gap players",
            limit: content_limits::GAP_PLAYERS,
        });
    }
    if players.is_empty() && !host_lacks {
        return Err(WireError::Invalid("gap players"));
    }
    if players.windows(2).any(|pair| pair[0].id >= pair[1].id) {
        return Err(WireError::Invalid("gap players"));
    }
    Ok(())
}

fn write_content_gaps(w: &mut BitWriter, gaps: &ContentGaps) -> WireResult<()> {
    let ContentGaps {
        host_build,
        host_importer,
        gaps,
    } = gaps;
    if gaps.len() > content_limits::GAPS {
        return Err(WireError::TooMany {
            what: "gaps",
            limit: content_limits::GAPS,
        });
    }
    let mut previous = None;
    for gap in gaps {
        check_gap(gap)?;
        check_order(previous, gap.kind, &gap.key)?;
        previous = Some((gap.kind, gap.key.as_str()));
    }
    write_build(w, *host_build);
    write_importer(w, host_importer.as_ref())?;
    write_count(w, gaps.len());
    for gap in gaps {
        write_item_key(w, gap.kind, &gap.key);
        write_str(w, &gap.label);
        w.write_bool(gap.host_lacks);
        write_count(w, gap.players.len());
        for player in &gap.players {
            let _ = w.write_bits(u64::from(player.id), 8);
            w.write_bool(player.differs);
        }
    }
    Ok(())
}

fn read_content_gaps(r: &mut BitReader<'_>) -> WireResult<ContentGaps> {
    let host_build = read_build(r)?;
    let host_importer = read_importer(r)?;
    let count = read_count(r, content_limits::GAPS, "gaps")?;
    if count > r.bits_remaining() / GAP_BITS {
        return Err(tore_codec::CodecError::UnexpectedEnd.into());
    }
    let mut gaps: Vec<Gap> = Vec::with_capacity(count);
    for _ in 0..count {
        let (kind, key) = read_item_key(r)?;
        check_order(
            gaps.last().map(|gap| (gap.kind, gap.key.as_str())),
            kind,
            &key,
        )?;
        let label = read_str(r)?;
        let host_lacks = r.read_bool()?;
        let players = read_count(r, content_limits::GAP_PLAYERS, "gap players")?;
        let players = (0..players)
            .map(|_| {
                Ok(GapPlayer {
                    id: read_id(r)?,
                    differs: r.read_bool()?,
                })
            })
            .collect::<WireResult<Vec<_>>>()?;
        let gap = Gap {
            kind,
            key,
            label,
            host_lacks,
            players,
        };
        check_gap(&gap)?;
        gaps.push(gap);
    }
    Ok(ContentGaps {
        host_build,
        host_importer,
        gaps,
    })
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
            Self::ChatSend(_) => kind::CHAT_SEND,
            Self::ChatLine(_) => kind::CHAT_LINE,
            Self::PassCrown(_) => kind::PASS_CROWN,
            Self::Settings(_) => kind::SETTINGS,
            Self::SlotLock(_) => kind::SLOT_LOCK,
            Self::Revive { .. } => kind::REVIVE,
            Self::Revival(_) => kind::REVIVAL,
            Self::Spawned(_) => kind::SPAWNED,
            Self::Scores(_) => kind::SCORES,
            Self::Results(_) => kind::RESULTS,
            Self::Observe(_) => kind::OBSERVE,
            Self::Observing(_) => kind::OBSERVING,
            Self::Away => kind::AWAY,
            Self::Back => kind::BACK,
            Self::Content(_) => kind::CONTENT,
            Self::ContentGaps(_) => kind::CONTENT_GAPS,
            Self::Token(_) => kind::TOKEN,
            Self::Candidate(_) => kind::CANDIDATE,
            Self::ReachTest(_) => kind::REACH_TEST,
            Self::ReachPeers(_) => kind::REACH_PEERS,
            Self::ReachReport(_) => kind::REACH_REPORT,
            Self::UploadTest(_) => kind::UPLOAD_TEST,
            Self::Succession(_) => kind::SUCCESSION,
            Self::StandbyRecord(_) => kind::STANDBY_RECORD,
            Self::StandbyStatus(_) => kind::STANDBY_STATUS,
            Self::Resume(_) => kind::RESUME,
            Self::Resumed(_) => kind::RESUMED,
            Self::Backlog(_) => kind::BACKLOG,
            Self::HostMoving(_) => kind::HOST_MOVING,
            Self::TakenOver(_) => kind::TAKEN_OVER,
            Self::Release(_) => kind::RELEASE,
            Self::Rejoin(_) => kind::REJOIN,
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
                | Self::ChatSend(_)
                | Self::PassCrown(_)
                | Self::Settings(_)
                | Self::SlotLock(_)
                | Self::Revive { .. }
                | Self::Observe(_)
                | Self::Away
                | Self::Back
                | Self::Content(_)
                | Self::Candidate(_)
                | Self::ReachReport(_)
                | Self::StandbyStatus(_)
                | Self::Resume(_)
                | Self::Backlog(_)
                | Self::TakenOver(_)
                | Self::Release(_)
                | Self::Rejoin(_)
        )
    }

    /// The body's bytes, at most 64 KB.
    pub fn encode(&self) -> WireResult<Vec<u8>> {
        let mut w = BitWriter::with_capacity(64);
        match self {
            Self::Mission(m) => {
                write_long_str(&mut w, &m.spec);
                write_manifest(&mut w, &m.manifest)?;
                let _ = w.write_bits(u64::from(m.host_tick), 32);
                w.write_varint(m.contrail_sortie);
                w.write_varint(u64::from(m.number));
            }
            Self::ContentRefused(c) => {
                w.write_varint(u64::from(c.mission));
                write_strings(&mut w, &c.names, MANIFEST_LIMIT, "names")?;
                write_str(&mut w, &c.reason);
                w.write_bool(c.flight);
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
                write_end_reason(&mut w, m.reason);
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
            Self::FlightLoadouts(flight) => {
                if flight.loadouts.len() > LOADOUTS_LIMIT {
                    return Err(WireError::TooMany {
                        what: "loadouts",
                        limit: LOADOUTS_LIMIT,
                    });
                }
                write_count(&mut w, flight.loadouts.len());
                for (plane, loadout) in &flight.loadouts {
                    w.write_varint(u64::from(*plane));
                    write_loadout(&mut w, loadout)?;
                }
                write_manifest(&mut w, &flight.manifest)?;
            }
            Self::ChatSend(send) => send.write(&mut w),
            Self::ChatLine(line) => line.write(&mut w),
            Self::PassCrown(player) => {
                let _ = w.write_bits(u64::from(*player), 8);
            }
            Self::Settings(change) => write_settings_change(&mut w, change)?,
            Self::SlotLock(lock) => {
                w.write_varint(u64::from(lock.mission));
                w.write_varint(u64::from(lock.plane));
                write_lock(&mut w, &lock.lock);
            }
            Self::Revive { mission } => w.write_varint(u64::from(*mission)),
            Self::Revival(revival) => write_revival(&mut w, revival)?,
            Self::Spawned(spawned) => write_spawned(&mut w, spawned)?,
            Self::Scores(scores) => write_scores(&mut w, scores)?,
            Self::Results(results) => write_results(&mut w, results)?,
            Self::Observe(observe) => write_observe(&mut w, *observe),
            Self::Observing(observing) => write_observing(&mut w, observing)?,
            Self::Away | Self::Back => {}
            Self::Content(content) => write_content(&mut w, content)?,
            Self::ContentGaps(gaps) => write_content_gaps(&mut w, gaps)?,
            Self::Token(grant) => migration::write_token(&mut w, grant),
            Self::Candidate(report) => migration::write_candidate(&mut w, report)?,
            Self::ReachTest(test) => migration::write_reach_test(&mut w, test)?,
            Self::ReachPeers(peers) => migration::write_reach_peers(&mut w, peers)?,
            Self::ReachReport(report) => migration::write_reach_report(&mut w, report)?,
            Self::UploadTest(test) => migration::write_upload_test(&mut w, test)?,
            Self::Succession(succession) => migration::write_succession(&mut w, succession)?,
            Self::StandbyRecord(record) => migration::write_record(&mut w, record)?,
            Self::StandbyStatus(status) => migration::write_status(&mut w, status),
            Self::Resume(resume) => migration::write_resume(&mut w, resume),
            Self::Resumed(resumed) => migration::write_resumed(&mut w, resumed)?,
            Self::Backlog(backlog) => migration::write_backlog(&mut w, backlog)?,
            Self::HostMoving(moving) => migration::write_host_moving(&mut w, moving),
            Self::TakenOver(taken) => migration::write_taken_over(&mut w, taken),
            Self::Release(plane) => w.write_varint(u64::from(*plane)),
            Self::Rejoin(token) => token.write(&mut w),
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
                let manifest = read_manifest(r)?;
                Self::Mission(Mission {
                    spec,
                    manifest,
                    host_tick: r.read_bits(32)? as u32,
                    contrail_sortie: r.read_varint()?,
                    number: read_u32(r)?,
                })
            }
            kind::CONTENT_REFUSED => Self::ContentRefused(ContentRefused {
                mission: read_u32(r)?,
                names: read_strings(r, MANIFEST_LIMIT, "names")?,
                reason: read_str(r)?,
                flight: r.read_bool()?,
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
                let reason = read_end_reason(r)?;
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
                Self::FlightLoadouts(FlightLoadouts {
                    loadouts,
                    manifest: read_manifest(r)?,
                })
            }
            kind::CHAT_SEND => Self::ChatSend(ChatSend::read(r)?),
            kind::CHAT_LINE => Self::ChatLine(ChatLine::read(r)?),
            kind::GOODBYE => Self::Goodbye(match r.read_bits(2)? {
                0 => Goodbye::Kicked(read_str(r)?),
                1 => Goodbye::HostLeft,
                _ => return Err(WireError::Invalid("goodbye")),
            }),
            kind::PASS_CROWN => Self::PassCrown(read_id(r)?),
            kind::SETTINGS => Self::Settings(Box::new(read_settings_change(r)?)),
            kind::SLOT_LOCK => Self::SlotLock(Box::new(SlotLock {
                mission: read_u32(r)?,
                plane: read_u32(r)?,
                lock: read_lock(r)?,
            })),
            kind::REVIVE => Self::Revive {
                mission: read_u32(r)?,
            },
            kind::REVIVAL => Self::Revival(Box::new(read_revival(r)?)),
            kind::SPAWNED => Self::Spawned(Box::new(read_spawned(r)?)),
            kind::SCORES => Self::Scores(Box::new(read_scores(r)?)),
            kind::RESULTS => Self::Results(Box::new(read_results(r)?)),
            kind::OBSERVE => Self::Observe(read_observe(r)?),
            kind::OBSERVING => Self::Observing(Box::new(read_observing(r)?)),
            kind::AWAY => Self::Away,
            kind::BACK => Self::Back,
            kind::CONTENT => Self::Content(Box::new(read_content(r)?)),
            kind::CONTENT_GAPS => Self::ContentGaps(Box::new(read_content_gaps(r)?)),
            kind::TOKEN => Self::Token(migration::read_token(r)?),
            kind::CANDIDATE => Self::Candidate(Box::new(migration::read_candidate(r)?)),
            kind::REACH_TEST => Self::ReachTest(Box::new(migration::read_reach_test(r)?)),
            kind::REACH_PEERS => Self::ReachPeers(Box::new(migration::read_reach_peers(r)?)),
            kind::REACH_REPORT => Self::ReachReport(Box::new(migration::read_reach_report(r)?)),
            kind::UPLOAD_TEST => Self::UploadTest(migration::read_upload_test(r)?),
            kind::SUCCESSION => Self::Succession(Box::new(migration::read_succession(r)?)),
            kind::STANDBY_RECORD => Self::StandbyRecord(migration::read_record(r)?),
            kind::STANDBY_STATUS => Self::StandbyStatus(migration::read_status(r)?),
            kind::RESUME => Self::Resume(migration::read_resume(r)?),
            kind::RESUMED => Self::Resumed(Box::new(migration::read_resumed(r)?)),
            kind::BACKLOG => Self::Backlog(Box::new(migration::read_backlog(r)?)),
            kind::HOST_MOVING => Self::HostMoving(migration::read_host_moving(r)?),
            kind::TAKEN_OVER => Self::TakenOver(migration::read_taken_over(r)?),
            kind::RELEASE => Self::Release(read_u32(r)?),
            kind::REJOIN => Self::Rejoin(tore_net::Token::read(r)?),
            _ => return Err(WireError::Invalid("message kind")),
        };
        bits::end(r)?;
        Ok(message)
    }
}
