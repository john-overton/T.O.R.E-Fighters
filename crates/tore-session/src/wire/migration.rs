//! Stage K's message bodies (protocol 13, slice K0): host migration and
//! rejoin ("Host migration and rejoin (stage K)" in
//! docs/formats/net-protocol.md; the design is docs/ARCHITECTURE.md, "Host
//! migration and rejoin").
//!
//! Every body here has its writer and its reader; [`super::messages`] gives
//! each its kind and carries it as a [`super::messages::Message`]. Every bound
//! is refused by the writer and by the reader alike, never cut. The standby
//! records that kind 46 carries are coded by [`crate::journal`], which keeps
//! the stream's state (each seat's last input): the message holds a record's
//! bytes as they are.
//!
//! An address is coded as the master's common fields code it (a family bit,
//! the octets, the port), and a candidate with its kind before it
//! ([`tore_net::master::candidate`]).

use super::bits::{read_count, read_long_bytes, read_u32, write_count, write_long_bytes};
use super::inputs::{Command, InputFrame, read_command, read_frame, write_command, write_frame};
use super::{Platform, WireError, WireResult, limits as wire_limits};
use std::net::SocketAddr;
use tore_codec::{BitReader, BitWriter};
use tore_net::Token;
use tore_net::master::candidate::{
    Candidate, MappingType, read_address, read_list, write_address, write_list,
};

/// Stage K's limits (net-protocol.md, "Limits").
pub mod limits {
    /// Standbys a host keeps, and a Succession names.
    pub const STANDBYS: usize = 2;
    /// Addresses of a standby, a reach target or a peer.
    pub const ADDRESSES: usize = 8;
    /// A game's own candidates in its Candidate report: Local, Mapped and
    /// Global IPv6.
    pub const OWN_CANDIDATES: usize = 3;
    /// Candidates one reach test tries, and results in its report.
    pub const REACH_CANDIDATES: usize = 3;
    /// Players a candidate opens its router to in one Reach peers.
    pub const REACH_PEERS: usize = 64;
    /// Ticks in a Backlog: 10 seconds.
    pub const BACKLOG_TICKS: usize = 1_200;
    /// Commands in a Backlog.
    pub const BACKLOG_COMMANDS: usize = 256;
    /// The longest Upload test, milliseconds.
    pub const UPLOAD_MS: u16 = 2_000;
    /// The bytes of one checkpoint chunk.
    pub const CHUNK_BYTES: usize = 4_096;
    /// Ticks in one Ticks record.
    pub const TICKS_PER_RECORD: usize = 60;
    /// Tokens a game keeps, in its data folder.
    pub const TOKENS_KEPT: usize = 32;
    /// A token's life after its player was last connected, seconds: 24
    /// hours (John, 2026-09-28).
    pub const TOKEN_LIFE_SECONDS: u32 = 86_400;
}

/// The processor a game runs on, half of its class (with the platform): a
/// standby of the host's class steps the mission alongside it (warm).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Processor {
    /// 0: a build for a processor the protocol does not name, or not said.
    #[default]
    Unknown,
    /// 1: x86-64.
    X86_64,
    /// 2: 64-bit ARM.
    Arm64,
    /// 3: 32-bit x86.
    X86,
    /// 4: any other.
    Other,
}

impl Processor {
    /// The processor this build was compiled for.
    pub const fn current() -> Self {
        if cfg!(target_arch = "x86_64") {
            Self::X86_64
        } else if cfg!(target_arch = "aarch64") {
            Self::Arm64
        } else if cfg!(target_arch = "x86") {
            Self::X86
        } else {
            Self::Other
        }
    }

    /// The wire's 3-bit code.
    pub const fn code(self) -> u8 {
        match self {
            Self::Unknown => 0,
            Self::X86_64 => 1,
            Self::Arm64 => 2,
            Self::X86 => 3,
            Self::Other => 4,
        }
    }

    /// The processor of a code; `None` for 5 to 7.
    pub const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::Unknown,
            1 => Self::X86_64,
            2 => Self::Arm64,
            3 => Self::X86,
            4 => Self::Other,
            _ => return None,
        })
    }
}

/// A player's rejoin token (host to player, kind 39), sent at its first
/// join: the token and its life after the player was last connected.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenGrant {
    pub token: Token,
    /// Seconds; [`limits::TOKEN_LIFE_SECONDS`].
    pub life_seconds: u32,
}

/// What a player's game can do as a host (player to host, kind 40), in the
/// lobby and whenever it changes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CandidateReport {
    /// The player's "Let my game take over hosting" switch.
    pub may_host: bool,
    /// Its class: the platform (the Challenge answer's codes) and the
    /// processor.
    pub platform: Platform,
    pub processor: Processor,
    /// Its joined socket's Local, Mapped and Global IPv6 candidates, at most
    /// [`limits::OWN_CANDIDATES`]; never a Seen one.
    pub candidates: Vec<Candidate>,
    /// Its router's mapping type from the master's test; unknown untested.
    pub mapping: MappingType,
    /// The CPU measure: the mean cost of a tick of the lobby's mission,
    /// microseconds; 0 when not measured.
    pub cpu_micros: u32,
    /// The lobby mission's number it measured.
    pub cpu_mission: u32,
}

/// A game the reach test tries, or a player a candidate opens its router to:
/// its lobby id and its addresses (at most [`limits::ADDRESSES`]).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachTarget {
    pub player: u8,
    pub addresses: Vec<SocketAddr>,
}

/// Try these candidates (host to player, kind 41): Reach each
/// address five times 200 ms apart and report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachTest {
    pub test: u16,
    /// At most [`limits::REACH_CANDIDATES`].
    pub candidates: Vec<ReachTarget>,
}

/// Open the router to these players (host to a candidate, kind 42).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachPeers {
    pub test: u16,
    /// At most [`limits::REACH_PEERS`].
    pub players: Vec<ReachTarget>,
}

/// A candidate reached: which of its addresses answered and the median
/// round trip.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reached {
    /// The index of the answering address in the test's list, 0 to 15.
    pub address: u8,
    pub round_trip_ms: u16,
}

/// One candidate's result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReachResult {
    pub player: u8,
    pub reached: Option<Reached>,
}

/// A reach test's results (player to host, kind 43).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReachReport {
    pub test: u16,
    /// At most [`limits::REACH_CANDIDATES`].
    pub results: Vec<ReachResult>,
}

/// Send a paced burst of Filler (host to player, kind 44).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct UploadTest {
    pub test: u16,
    /// Bytes a second.
    pub rate: u32,
    /// At most [`limits::UPLOAD_MS`].
    pub length_ms: u16,
}

/// A ready standby in the succession.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Successor {
    pub player: u8,
    pub warm: bool,
    /// Its candidates with their kinds (the host's Seen one included), at
    /// most [`limits::ADDRESSES`].
    pub addresses: Vec<Candidate>,
}

/// The ready standbys in order (host to every player, kind 45), sent on
/// every change.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Succession {
    /// At most [`limits::STANDBYS`].
    pub standbys: Vec<Successor>,
}

/// Where a standby is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StandbyState {
    /// 0: building its copy of the world.
    #[default]
    Building,
    /// 1: ready, stepping the journal alongside the host.
    Warm,
    /// 2: ready, keeping a checkpoint and the journal since.
    Cold,
    /// 3: behind, or unable to build the mission: the host dismisses it.
    Behind,
}

/// The last Check's result.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CheckResult {
    /// 0: no check yet.
    #[default]
    None,
    /// 1: the standby's world coded to the host's hash.
    Equal,
    /// 2: it did not.
    Different,
}

/// A standby's report (standby to host, kind 47), at most twice a second.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StandbyStatus {
    /// The newest tick it holds.
    pub newest_tick: u32,
    pub state: StandbyState,
    /// Its mean step cost, microseconds.
    pub step_micros: u32,
    /// The last Check's tick and result.
    pub check_tick: u32,
    pub check: CheckResult,
    pub needs_checkpoint: bool,
}

/// A player resumes with the game's new host (player to new host, kind 48).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resume {
    /// Its flight with the old host.
    pub flight: u8,
    /// Its newest predicted tick; 0 when not flying.
    pub newest_tick: u32,
    /// The number of the mission it holds and the FNV-1a 64 of its text.
    pub mission: u32,
    pub mission_hash: u64,
    pub watching: bool,
}

/// A resumed player's plane at the takeover's tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResumedFlight {
    /// The connection's new flight, never 0.
    pub flight: u8,
    pub seat: u8,
    pub plane: u32,
    /// T: the tick the new host took over at.
    pub tick: u32,
    /// The number of the last command the old host applied for the seat.
    pub last_command: u16,
    /// The plane's exact state at T, coded with no baseline.
    pub exact: Vec<u8>,
    /// The ground objects destroyed by T.
    pub destroyed: Vec<u32>,
}

/// The new host's answer to Resume (new host to player, kind 49).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Resumed {
    /// Flight 0: the player is not flying; nothing follows.
    NotFlying,
    Flying(ResumedFlight),
}

/// One tick of a Backlog: the controls as the Inputs section codes a tick,
/// and what the screen showed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BacklogTick {
    pub frame: InputFrame,
    /// The host tick the screen showed, as ticks before this one.
    pub view_offset: u8,
    /// The interpolation delay, 0 to 63 ticks.
    pub interpolation_delay: u8,
}

/// A command not yet applied, at its tick as an offset from the Backlog's
/// first tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BacklogCommand {
    pub offset: u32,
    pub command: Command,
}

/// A resumed player's inputs from T on (player to new host, kind 50).
#[derive(Clone, Debug, PartialEq)]
pub struct Backlog {
    /// The new flight, from Resumed.
    pub flight: u8,
    /// T.
    pub first_tick: u32,
    /// At most [`limits::BACKLOG_TICKS`], from T on.
    pub ticks: Vec<BacklogTick>,
    /// At most [`limits::BACKLOG_COMMANDS`], numbered from 1 in this order;
    /// each within the ticks.
    pub commands: Vec<BacklogCommand>,
}

/// The host hands over (host to every player, kind 51).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HostMoving {
    /// The standby taking over.
    pub standby: u8,
    /// The last tick the host steps.
    pub last_tick: u32,
}

/// The new host tells the old one (kind 52, the new host's own client on its
/// old connection).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TakenOver {
    pub new_host: u8,
    /// T.
    pub tick: u32,
}

fn too_many(what: &'static str, limit: usize) -> WireError {
    WireError::TooMany { what, limit }
}

fn check_count(count: usize, limit: usize, what: &'static str) -> WireResult<()> {
    if count > limit {
        return Err(too_many(what, limit));
    }
    Ok(())
}

fn address_error<E>(_: E) -> WireError {
    WireError::Invalid("address")
}

/// A count of `width` bits, refused over `limit` by the writer and the
/// reader.
fn write_small_count(w: &mut BitWriter, count: usize, width: u32) {
    let _ = w.write_bits(count as u64, width);
}

fn read_small_count(
    r: &mut BitReader<'_>,
    width: u32,
    limit: usize,
    what: &'static str,
) -> WireResult<usize> {
    let count = r.read_bits(width)? as usize;
    check_count(count, limit, what)?;
    Ok(count)
}

fn write_addresses(w: &mut BitWriter, addresses: &[SocketAddr]) -> WireResult<()> {
    check_count(addresses.len(), limits::ADDRESSES, "addresses")?;
    write_small_count(w, addresses.len(), 4);
    for address in addresses {
        write_address(w, *address);
    }
    Ok(())
}

fn read_addresses(r: &mut BitReader<'_>) -> WireResult<Vec<SocketAddr>> {
    let count = read_small_count(r, 4, limits::ADDRESSES, "addresses")?;
    (0..count)
        .map(|_| read_address(r).map_err(address_error))
        .collect()
}

fn write_target(w: &mut BitWriter, target: &ReachTarget) -> WireResult<()> {
    let _ = w.write_bits(u64::from(target.player), 8);
    write_addresses(w, &target.addresses)
}

fn read_target(r: &mut BitReader<'_>) -> WireResult<ReachTarget> {
    Ok(ReachTarget {
        player: r.read_bits(8)? as u8,
        addresses: read_addresses(r)?,
    })
}

pub(super) fn write_token(w: &mut BitWriter, grant: &TokenGrant) {
    grant.token.write(w);
    w.write_varint(u64::from(grant.life_seconds));
}

pub(super) fn read_token(r: &mut BitReader<'_>) -> WireResult<TokenGrant> {
    Ok(TokenGrant {
        token: Token::read(r)?,
        life_seconds: read_u32(r)?,
    })
}

pub(super) fn write_candidate(w: &mut BitWriter, report: &CandidateReport) -> WireResult<()> {
    let CandidateReport {
        may_host,
        platform,
        processor,
        candidates,
        mapping,
        cpu_micros,
        cpu_mission,
    } = report;
    check_count(candidates.len(), limits::OWN_CANDIDATES, "candidates")?;
    w.write_bool(*may_host);
    let _ = w.write_bits(u64::from(platform.code()), 3);
    let _ = w.write_bits(u64::from(processor.code()), 3);
    write_list(w, candidates, true).map_err(|_| WireError::Invalid("candidate"))?;
    let _ = w.write_bits(u64::from(mapping.code()), MappingType::BITS);
    w.write_varint(u64::from(*cpu_micros));
    w.write_varint(u64::from(*cpu_mission));
    Ok(())
}

pub(super) fn read_candidate(r: &mut BitReader<'_>) -> WireResult<CandidateReport> {
    let may_host = r.read_bool()?;
    let platform =
        Platform::from_code(r.read_bits(3)? as u8).ok_or(WireError::Invalid("platform"))?;
    let processor =
        Processor::from_code(r.read_bits(3)? as u8).ok_or(WireError::Invalid("processor"))?;
    let candidates = read_list(r, true).map_err(|_| WireError::Invalid("candidate"))?;
    check_count(candidates.len(), limits::OWN_CANDIDATES, "candidates")?;
    let mapping = MappingType::from_code(r.read_bits(MappingType::BITS)?)
        .ok_or(WireError::Invalid("mapping type"))?;
    Ok(CandidateReport {
        may_host,
        platform,
        processor,
        candidates,
        mapping,
        cpu_micros: read_u32(r)?,
        cpu_mission: read_u32(r)?,
    })
}

pub(super) fn write_reach_test(w: &mut BitWriter, test: &ReachTest) -> WireResult<()> {
    check_count(
        test.candidates.len(),
        limits::REACH_CANDIDATES,
        "reach candidates",
    )?;
    let _ = w.write_bits(u64::from(test.test), 16);
    write_small_count(w, test.candidates.len(), 2);
    for target in &test.candidates {
        write_target(w, target)?;
    }
    Ok(())
}

pub(super) fn read_reach_test(r: &mut BitReader<'_>) -> WireResult<ReachTest> {
    let test = r.read_bits(16)? as u16;
    let count = read_small_count(r, 2, limits::REACH_CANDIDATES, "reach candidates")?;
    let candidates = (0..count)
        .map(|_| read_target(r))
        .collect::<WireResult<_>>()?;
    Ok(ReachTest { test, candidates })
}

pub(super) fn write_reach_peers(w: &mut BitWriter, peers: &ReachPeers) -> WireResult<()> {
    check_count(peers.players.len(), limits::REACH_PEERS, "reach peers")?;
    let _ = w.write_bits(u64::from(peers.test), 16);
    write_count(w, peers.players.len());
    for target in &peers.players {
        write_target(w, target)?;
    }
    Ok(())
}

pub(super) fn read_reach_peers(r: &mut BitReader<'_>) -> WireResult<ReachPeers> {
    let test = r.read_bits(16)? as u16;
    let count = read_count(r, limits::REACH_PEERS, "reach peers")?;
    let players = (0..count)
        .map(|_| read_target(r))
        .collect::<WireResult<_>>()?;
    Ok(ReachPeers { test, players })
}

pub(super) fn write_reach_report(w: &mut BitWriter, report: &ReachReport) -> WireResult<()> {
    check_count(
        report.results.len(),
        limits::REACH_CANDIDATES,
        "reach results",
    )?;
    let _ = w.write_bits(u64::from(report.test), 16);
    write_small_count(w, report.results.len(), 2);
    for result in &report.results {
        let _ = w.write_bits(u64::from(result.player), 8);
        w.write_bool(result.reached.is_some());
        if let Some(reached) = result.reached {
            if usize::from(reached.address) >= 16 {
                return Err(WireError::Invalid("reached address"));
            }
            let _ = w.write_bits(u64::from(reached.address), 4);
            let _ = w.write_bits(u64::from(reached.round_trip_ms), 16);
        }
    }
    Ok(())
}

pub(super) fn read_reach_report(r: &mut BitReader<'_>) -> WireResult<ReachReport> {
    let test = r.read_bits(16)? as u16;
    let count = read_small_count(r, 2, limits::REACH_CANDIDATES, "reach results")?;
    let mut results = Vec::with_capacity(count);
    for _ in 0..count {
        let player = r.read_bits(8)? as u8;
        let reached = if r.read_bool()? {
            Some(Reached {
                address: r.read_bits(4)? as u8,
                round_trip_ms: r.read_bits(16)? as u16,
            })
        } else {
            None
        };
        results.push(ReachResult { player, reached });
    }
    Ok(ReachReport { test, results })
}

pub(super) fn write_upload_test(w: &mut BitWriter, test: &UploadTest) -> WireResult<()> {
    if test.length_ms > limits::UPLOAD_MS {
        return Err(WireError::Invalid("upload test length"));
    }
    let _ = w.write_bits(u64::from(test.test), 16);
    w.write_varint(u64::from(test.rate));
    let _ = w.write_bits(u64::from(test.length_ms), 16);
    Ok(())
}

pub(super) fn read_upload_test(r: &mut BitReader<'_>) -> WireResult<UploadTest> {
    let test = r.read_bits(16)? as u16;
    let rate = read_u32(r)?;
    let length_ms = r.read_bits(16)? as u16;
    if length_ms > limits::UPLOAD_MS {
        return Err(WireError::Invalid("upload test length"));
    }
    Ok(UploadTest {
        test,
        rate,
        length_ms,
    })
}

pub(super) fn write_succession(w: &mut BitWriter, succession: &Succession) -> WireResult<()> {
    check_count(succession.standbys.len(), limits::STANDBYS, "standbys")?;
    write_small_count(w, succession.standbys.len(), 2);
    for standby in &succession.standbys {
        let _ = w.write_bits(u64::from(standby.player), 8);
        w.write_bool(standby.warm);
        write_list(w, &standby.addresses, false).map_err(|_| WireError::Invalid("candidate"))?;
    }
    Ok(())
}

pub(super) fn read_succession(r: &mut BitReader<'_>) -> WireResult<Succession> {
    let count = read_small_count(r, 2, limits::STANDBYS, "standbys")?;
    let mut standbys = Vec::with_capacity(count);
    for _ in 0..count {
        standbys.push(Successor {
            player: r.read_bits(8)? as u8,
            warm: r.read_bool()?,
            addresses: read_list(r, false).map_err(|_| WireError::Invalid("candidate"))?,
        });
    }
    Ok(Succession { standbys })
}

pub(super) fn write_status(w: &mut BitWriter, status: &StandbyStatus) {
    let StandbyStatus {
        newest_tick,
        state,
        step_micros,
        check_tick,
        check,
        needs_checkpoint,
    } = status;
    let _ = w.write_bits(u64::from(*newest_tick), 32);
    let state = match state {
        StandbyState::Building => 0,
        StandbyState::Warm => 1,
        StandbyState::Cold => 2,
        StandbyState::Behind => 3,
    };
    let _ = w.write_bits(state, 2);
    w.write_varint(u64::from(*step_micros));
    let _ = w.write_bits(u64::from(*check_tick), 32);
    let check = match check {
        CheckResult::None => 0,
        CheckResult::Equal => 1,
        CheckResult::Different => 2,
    };
    let _ = w.write_bits(check, 2);
    w.write_bool(*needs_checkpoint);
}

pub(super) fn read_status(r: &mut BitReader<'_>) -> WireResult<StandbyStatus> {
    let newest_tick = r.read_bits(32)? as u32;
    let state = match r.read_bits(2)? {
        0 => StandbyState::Building,
        1 => StandbyState::Warm,
        2 => StandbyState::Cold,
        _ => StandbyState::Behind,
    };
    let step_micros = read_u32(r)?;
    let check_tick = r.read_bits(32)? as u32;
    let check = match r.read_bits(2)? {
        0 => CheckResult::None,
        1 => CheckResult::Equal,
        2 => CheckResult::Different,
        _ => return Err(WireError::Invalid("check result")),
    };
    Ok(StandbyStatus {
        newest_tick,
        state,
        step_micros,
        check_tick,
        check,
        needs_checkpoint: r.read_bool()?,
    })
}

pub(super) fn write_resume(w: &mut BitWriter, resume: &Resume) {
    let Resume {
        flight,
        newest_tick,
        mission,
        mission_hash,
        watching,
    } = resume;
    let _ = w.write_bits(u64::from(*flight), 8);
    let _ = w.write_bits(u64::from(*newest_tick), 32);
    w.write_varint(u64::from(*mission));
    let _ = w.write_bits(*mission_hash, 64);
    w.write_bool(*watching);
}

pub(super) fn read_resume(r: &mut BitReader<'_>) -> WireResult<Resume> {
    Ok(Resume {
        flight: r.read_bits(8)? as u8,
        newest_tick: r.read_bits(32)? as u32,
        mission: read_u32(r)?,
        mission_hash: r.read_bits(64)?,
        watching: r.read_bool()?,
    })
}

pub(super) fn write_resumed(w: &mut BitWriter, resumed: &Resumed) -> WireResult<()> {
    let ResumedFlight {
        flight,
        seat,
        plane,
        tick,
        last_command,
        exact,
        destroyed,
    } = match resumed {
        Resumed::NotFlying => {
            let _ = w.write_bits(0, 8);
            return Ok(());
        }
        Resumed::Flying(flight) => flight,
    };
    if *flight == 0 {
        return Err(WireError::Invalid("resumed flight"));
    }
    let _ = w.write_bits(u64::from(*flight), 8);
    let _ = w.write_bits(u64::from(*seat), 8);
    w.write_varint(u64::from(*plane));
    let _ = w.write_bits(u64::from(*tick), 32);
    let _ = w.write_bits(u64::from(*last_command), 16);
    write_long_bytes(w, exact);
    super::messages::write_destroyed(w, destroyed)
}

pub(super) fn read_resumed(r: &mut BitReader<'_>) -> WireResult<Resumed> {
    let flight = r.read_bits(8)? as u8;
    if flight == 0 {
        return Ok(Resumed::NotFlying);
    }
    Ok(Resumed::Flying(ResumedFlight {
        flight,
        seat: r.read_bits(8)? as u8,
        plane: read_u32(r)?,
        tick: r.read_bits(32)? as u32,
        last_command: r.read_bits(16)? as u16,
        exact: read_long_bytes(r, wire_limits::MESSAGE, "exact state bytes")?,
        destroyed: super::messages::read_destroyed(r)?,
    }))
}

pub(super) fn write_backlog(w: &mut BitWriter, backlog: &Backlog) -> WireResult<()> {
    let Backlog {
        flight,
        first_tick,
        ticks,
        commands,
    } = backlog;
    check_count(ticks.len(), limits::BACKLOG_TICKS, "backlog ticks")?;
    check_count(commands.len(), limits::BACKLOG_COMMANDS, "backlog commands")?;
    let _ = w.write_bits(u64::from(*flight), 8);
    let _ = w.write_bits(u64::from(*first_tick), 32);
    let _ = w.write_bits(ticks.len() as u64, 16);
    let mut previous: Option<&InputFrame> = None;
    for tick in ticks {
        if tick.interpolation_delay > 63 {
            return Err(WireError::Invalid("interpolation delay"));
        }
        write_frame(w, &tick.frame, previous)?;
        previous = Some(&tick.frame);
        let _ = w.write_bits(u64::from(tick.view_offset), 8);
        let _ = w.write_bits(u64::from(tick.interpolation_delay), 6);
    }
    write_count(w, commands.len());
    for command in commands {
        if command.offset as usize >= ticks.len() {
            return Err(WireError::Invalid("backlog command after its ticks"));
        }
        w.write_varint(u64::from(command.offset));
        write_command(w, &command.command);
    }
    Ok(())
}

pub(super) fn read_backlog(r: &mut BitReader<'_>) -> WireResult<Backlog> {
    let flight = r.read_bits(8)? as u8;
    let first_tick = r.read_bits(32)? as u32;
    let count = r.read_bits(16)? as usize;
    check_count(count, limits::BACKLOG_TICKS, "backlog ticks")?;
    // Each tick takes at least its frame's bit and its view's 14.
    if count > r.bits_remaining() / 15 {
        return Err(tore_codec::CodecError::UnexpectedEnd.into());
    }
    let mut ticks: Vec<BacklogTick> = Vec::with_capacity(count);
    for _ in 0..count {
        let frame = read_frame(r, ticks.last().map(|t| &t.frame))?;
        ticks.push(BacklogTick {
            frame,
            view_offset: r.read_bits(8)? as u8,
            interpolation_delay: r.read_bits(6)? as u8,
        });
    }
    let count = read_count(r, limits::BACKLOG_COMMANDS, "backlog commands")?;
    let mut commands = Vec::with_capacity(count);
    for _ in 0..count {
        let offset = read_u32(r)?;
        if offset as usize >= ticks.len() {
            return Err(WireError::Invalid("backlog command after its ticks"));
        }
        commands.push(BacklogCommand {
            offset,
            command: read_command(r)?,
        });
    }
    Ok(Backlog {
        flight,
        first_tick,
        ticks,
        commands,
    })
}

/// A standby record's body as kind 46 carries it: at least its type, a code
/// the stream names (0 to 9).
pub(super) fn check_record(record: &[u8]) -> WireResult<()> {
    match record.first() {
        Some(first) if first & 0x0F <= crate::journal::LAST_RECORD_TYPE => Ok(()),
        _ => Err(WireError::Invalid("standby record")),
    }
}

pub(super) fn write_record(w: &mut BitWriter, record: &[u8]) -> WireResult<()> {
    check_record(record)?;
    w.write_bytes(record);
    Ok(())
}

pub(super) fn read_record(r: &mut BitReader<'_>) -> WireResult<Vec<u8>> {
    let bytes = r.read_bytes(r.bits_remaining() / 8)?;
    check_record(&bytes)?;
    Ok(bytes)
}

pub(super) fn write_host_moving(w: &mut BitWriter, moving: &HostMoving) {
    let _ = w.write_bits(u64::from(moving.standby), 8);
    let _ = w.write_bits(u64::from(moving.last_tick), 32);
}

pub(super) fn read_host_moving(r: &mut BitReader<'_>) -> WireResult<HostMoving> {
    Ok(HostMoving {
        standby: r.read_bits(8)? as u8,
        last_tick: r.read_bits(32)? as u32,
    })
}

pub(super) fn write_taken_over(w: &mut BitWriter, taken: &TakenOver) {
    let _ = w.write_bits(u64::from(taken.new_host), 8);
    let _ = w.write_bits(u64::from(taken.tick), 32);
}

pub(super) fn read_taken_over(r: &mut BitReader<'_>) -> WireResult<TakenOver> {
    Ok(TakenOver {
        new_host: r.read_bits(8)? as u8,
        tick: r.read_bits(32)? as u32,
    })
}
