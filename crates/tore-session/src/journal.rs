//! The journal: every change a host makes to its world, as records a standby
//! replays to hold the host's world bit for bit (stage K; docs/ARCHITECTURE.md,
//! "The journal: one door into the world", and the bytes in
//! docs/formats/net-protocol.md, "The standby stream").
//!
//! - A [`Tick`] is one tick's changes: the changes the host makes before the
//!   step (the scoring switch, an `ai-slot` revival's store cut), the
//!   mission commands, and every seat's input with the number of the last
//!   command applied for it. [`apply_tick`] steps a world with one: the host
//!   steps its own world through it every tick (slice K0), and a standby
//!   replays the same records through it, so the two cannot drift.
//! - [`drain`] takes what every tick leaves for the host to read (the score
//!   facts, combat's device notes). The host's scoring and event tracker make
//!   these drains today; a standby makes them after each replayed tick, so its
//!   world holds what the host's holds. *Agent decision (K0):* `apply_tick`
//!   leaves the drains to its caller until slice K1 hands the drained notes to
//!   the tracker as an argument.
//! - A [`Record`] is one entry of the standby stream; [`StreamWriter`] and
//!   [`StreamReader`] code them in order, each keeping every seat's last
//!   input, against which the next is coded. A message of kind 46
//!   ([`crate::wire::messages::Message::StandbyRecord`]) carries one record's
//!   bytes.
//!
//! The coders are the checkpoint trait's ([`tore_sim::checkpoint`]), so every
//! field of a seat's input and every variant of a mission command is named:
//! one added fails to compile until the journal codes it.

use crate::wire::bits::{read_long_bytes, write_long_bytes};
use crate::wire::inputs::{InputFrame, read_frame, write_frame};
use crate::wire::messages::{EndReason, StandbyMark, read_end_reason, write_end_reason};
use crate::wire::migration::limits::{CHUNK_BYTES, TICKS_PER_RECORD};
use crate::wire::{WireError, WireResult, limits};
use std::collections::BTreeMap;
use tore_codec::{BitReader, BitWriter};
use tore_sim::checkpoint::{Checkpoint, Loader, Models, Saver};
use tore_sim::combat::live::DeviceNote;
use tore_sim::flight::PilotInput;
use tore_world::WorldResult;
use tore_world::score::Facts;
use tore_world::seats::{PlaneId, SeatId, SeatInput, SeatView};
use tore_world::world::revive::RevivalWeapons;
use tore_world::world::{MissionCommand, TickOutput, World};

/// The highest record type the stream names (Handover).
pub const LAST_RECORD_TYPE: u8 = 9;
/// A Check every this many ticks (5 seconds; the plan's setting).
pub const CHECK_EVERY_TICKS: u32 = 600;
/// A cold standby's checkpoint every this many ticks (10 seconds).
pub const CHECKPOINT_EVERY_TICKS: u32 = 1_200;
/// Changes before one tick's step, at most (agent decision: a host makes a
/// few; the bound only keeps damaged bytes from asking for more).
pub const MAX_CHANGES: usize = 256;
/// Mission commands of one tick, at most (agent decision, as changes).
pub const MAX_MISSION_COMMANDS: usize = 256;
/// Seat inputs of one tick, at most: a lobby's players (agent decision; a
/// host flies at most 30 seats).
pub const MAX_SEAT_INPUTS: usize = 64;

/// A change the host makes to its world before a tick's step, outside the
/// mission commands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Scoring switched on or off ([`World::set_scoring`]): on when a
    /// mission starts flying. A standby turns scoring on only by replaying
    /// this.
    Scoring(bool),
    /// An `ai-slot` revival cuts an AI aircraft's stores before a player
    /// takes it ([`World::cut_ai_stores`]).
    StoreCut {
        plane: PlaneId,
        weapons: RevivalWeapons,
    },
}

/// One tick of the journal: what the host stepped its world with.
#[derive(Clone, Debug, Default)]
pub struct Tick {
    /// The tick stepped: the world's [`World::tick`] before the step.
    pub tick: u64,
    /// The changes before the step, in order.
    pub changes: Vec<Change>,
    /// The tick's mission commands, as given.
    pub mission: Vec<MissionCommand>,
    /// Every seat's input as the host stepped it, in the order given.
    pub inputs: Vec<SeatInput>,
    /// For each input, at the same index, the number of the last command
    /// the host has applied for that seat (0 before any): a resuming player
    /// leaves those out of its backlog.
    pub applied: Vec<u16>,
}

impl Tick {
    /// An empty tick for `tick`.
    pub fn new(tick: u64) -> Self {
        Self {
            tick,
            ..Self::default()
        }
    }

    /// Adds a seat's input and the number of its last command applied.
    pub fn push_input(&mut self, input: SeatInput, applied: u16) {
        self.inputs.push(input);
        self.applied.push(applied);
    }
}

/// Steps `world` one tick with `tick`: the changes before the step in order,
/// then [`World::step_with`] with the tick's mission commands and inputs into
/// `out`. The host steps its world through this, and a standby replays the
/// host's records through it. A tick for another tick than the world's next
/// is refused before anything changes.
pub fn apply_tick(world: &mut World, tick: &Tick, out: &mut TickOutput) -> WorldResult<()> {
    if tick.tick != world.tick() {
        return Err(format!(
            "the journal's tick {} is not the world's next, {}",
            tick.tick,
            world.tick()
        )
        .into());
    }
    if tick.applied.len() != tick.inputs.len() {
        return Err("a journal tick's inputs and their commands applied differ in number".into());
    }
    for change in &tick.changes {
        match *change {
            Change::Scoring(on) => world.set_scoring(on),
            Change::StoreCut { plane, weapons } => world.cut_ai_stores(plane, weapons)?,
        }
    }
    world.step_with(&tick.mission, &tick.inputs, out, |_, _| Ok(()))
}

/// What every tick leaves in the world for its host to read.
#[derive(Debug, Default)]
pub struct Drained {
    /// The tick's score facts (none with scoring off).
    pub facts: Facts,
    /// Combat's chaff and flare notes.
    pub notes: Vec<DeviceNote>,
}

/// Takes what the tick just stepped left for its host: the score facts and
/// combat's device notes, as the host's scoring and event tracker take them.
/// A standby calls it after each replayed tick.
pub fn drain(world: &mut World) -> Drained {
    Drained {
        facts: world.take_score_facts(),
        notes: world.combat.state.take_device_notes(),
    }
}

/// The FNV-1a 64 of the world's checkpoint between two ticks: a Check
/// record's hash.
pub fn check_hash(world: &World) -> Result<u64, tore_sim::checkpoint::CheckpointError> {
    Ok(tore_codec::fnv1a64(&world.checkpoint()?))
}

/// A part of the session's state the stream carries (a State record's
/// first field, 8 bits). Each part's bytes are the host's own coding, read
/// by the same build only.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Part {
    Players = 1,
    Session = 2,
    Court = 3,
    Scores = 4,
    Revivals = 5,
    Rejoin = 6,
    Candidates = 7,
    Listing = 8,
}

impl Part {
    /// Every part, in code order.
    pub const ALL: [Self; 8] = [
        Self::Players,
        Self::Session,
        Self::Court,
        Self::Scores,
        Self::Revivals,
        Self::Rejoin,
        Self::Candidates,
        Self::Listing,
    ];

    /// The part of a code; `None` for 0 and 9 up.
    pub fn from_code(code: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|part| *part as u8 == code)
    }
}

/// Appoint: the game is a standby from here on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Appoint {
    /// First or second; never none.
    pub role: StandbyMark,
    pub warm: bool,
    /// A Check every this many ticks ([`CHECK_EVERY_TICKS`]).
    pub check_every: u32,
    /// When cold, a checkpoint every this many ticks
    /// ([`CHECKPOINT_EVERY_TICKS`]).
    pub checkpoint_every: u32,
    /// The lobby mission's number.
    pub mission: u32,
}

/// Flight: build the world fresh from the flight's spec, which the player
/// holds; the journal starts at its tick 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FlightRecord {
    pub mission: u32,
    /// The FNV-1a 64 of the flight's spec text.
    pub spec_hash: u64,
    /// The fresh world's [`World::mission_identity`].
    pub identity: u64,
}

/// A checkpoint's first record: the chunks follow, between other records.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckpointBegin {
    /// The tick the checkpoint holds the world before.
    pub tick: u32,
    pub length: u32,
    pub chunks: u16,
}

/// One chunk of a checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckpointChunk {
    pub index: u16,
    /// At most [`CHUNK_BYTES`].
    pub bytes: Vec<u8>,
}

/// A snapshot interval of the journal: consecutive ticks from `first`.
#[derive(Clone, Debug, Default)]
pub struct Ticks {
    pub first: u32,
    /// 1 to [`TICKS_PER_RECORD`], their `tick` fields `first` on.
    pub ticks: Vec<Tick>,
}

/// One part of the session's state, as it holds after `tick`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatePart {
    pub part: Part,
    pub tick: u32,
    pub bytes: Vec<u8>,
}

/// The host's world's hash between `tick` and the next.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Check {
    pub tick: u32,
    pub hash: u64,
}

/// One record of the standby stream (type, 4 bits, then its fields).
#[derive(Clone, Debug)]
pub enum Record {
    /// 0.
    Appoint(Appoint),
    /// 1: the game is no longer a standby and drops its copy.
    Dismiss,
    /// 2.
    Flight(FlightRecord),
    /// 3.
    CheckpointBegin(CheckpointBegin),
    /// 4.
    CheckpointChunk(CheckpointChunk),
    /// 5.
    Ticks(Ticks),
    /// 6.
    State(StatePart),
    /// 7.
    Check(Check),
    /// 8: the mission ended, for this reason: drop the world.
    Ended(EndReason),
    /// 9: take over once replayed to this tick, the last the host steps.
    Handover { last_tick: u32 },
}

impl Record {
    /// The record's type, the first 4 bits of its bytes.
    pub fn code(&self) -> u8 {
        match self {
            Self::Appoint(_) => 0,
            Self::Dismiss => 1,
            Self::Flight(_) => 2,
            Self::CheckpointBegin(_) => 3,
            Self::CheckpointChunk(_) => 4,
            Self::Ticks(_) => 5,
            Self::State(_) => 6,
            Self::Check(_) => 7,
            Self::Ended(_) => 8,
            Self::Handover { .. } => 9,
        }
    }
}

/// Every seat's last input in the stream, against which its next is coded.
/// An Appoint and a Flight start it afresh.
#[derive(Clone, Debug, Default)]
struct Baselines {
    /// Each seat's last input and the number of its last command applied.
    inputs: BTreeMap<SeatId, (SeatInput, u16)>,
}

impl Baselines {
    /// What `seat`'s input at `tick` is coded against: its last, moved on to
    /// `tick` (its tick, and its view's by as many ticks) with no commands,
    /// so an unchanged control costs a bit.
    fn baseline(&self, seat: SeatId, tick: u64) -> Option<(SeatInput, u16)> {
        let (last, applied) = self.inputs.get(&seat)?;
        let moved = tick.wrapping_sub(last.tick);
        let mut base = last.clone();
        base.tick = tick;
        if let Some(view) = &mut base.view {
            view.tick = view.tick.wrapping_add(moved);
        }
        base.pilot.commands.clear();
        base.commands.clear();
        Some((base, *applied))
    }
}

/// The view's coding beside its baseline's (2 bits): none, as the Inputs
/// section codes it (offset and delay), or whole.
const VIEW_NONE: u64 = 0;
const VIEW_COMPACT: u64 = 1;
const VIEW_WHOLE: u64 = 2;

/// Whether two floats are the same bits: the stream is exact.
fn same_bits(a: f64, b: f64) -> bool {
    a.to_bits() == b.to_bits()
}

/// Codes one seat's input at `tick` against its baseline (protocol 14,
/// slice K3): the seat; the number of its last command applied, one bit
/// when the baseline's; the controls as the Inputs section codes them (the
/// wire's quantized frame against the baseline's), behind a bit that says
/// every control lies on the wire's grid, or else each by the checkpoint
/// trait; the two command lists behind one presence bit; the view, one bit
/// when the baseline's moved on, else none, as Inputs codes it, or whole.
/// The input's own tick is the record's. Every field is named, so a field
/// added to a seat's input or a pilot's fails to compile here.
fn write_seat_input(
    s: &mut Saver,
    input: &SeatInput,
    applied: u16,
    base: Option<&(SeatInput, u16)>,
) -> WireResult<()> {
    let SeatInput {
        seat,
        tick,
        pilot,
        trigger,
        sensors,
        commands,
        view,
    } = input;
    let PilotInput {
        pitch,
        roll,
        yaw,
        throttle_rate,
        throttle,
        commands: pilot_commands,
    } = pilot;
    let base_input = base.map(|(b, _)| b);
    let _ = s.writer().write_bits(u64::from(seat.0), 8);
    // The command number applied.
    if base.is_some_and(|(_, a)| *a == applied) {
        s.writer().write_bool(true);
    } else {
        s.writer().write_bool(false);
        let _ = s.writer().write_bits(u64::from(applied), 16);
    }
    // The controls.
    let frame = InputFrame::of(pilot, *trigger, *sensors);
    let back = frame.pilot();
    let on_grid = same_bits(back.pitch, *pitch)
        && same_bits(back.roll, *roll)
        && same_bits(back.yaw, *yaw)
        && same_bits(back.throttle_rate, *throttle_rate)
        && match (back.throttle, throttle) {
            (None, None) => true,
            (Some(a), Some(b)) => same_bits(a, *b),
            _ => false,
        };
    s.writer().write_bool(on_grid);
    if on_grid {
        let previous = base_input.map(|b| InputFrame::of(&b.pilot, b.trigger, b.sensors));
        write_frame(s.writer(), &frame, previous.as_ref())?;
    } else {
        let b = base_input.map(|b| &b.pilot);
        pitch.save(s, b.map(|b| &b.pitch))?;
        roll.save(s, b.map(|b| &b.roll))?;
        yaw.save(s, b.map(|b| &b.yaw))?;
        throttle_rate.save(s, b.map(|b| &b.throttle_rate))?;
        throttle.save(s, b.map(|b| &b.throttle))?;
        trigger.save(s, base_input.map(|b| &b.trigger))?;
        sensors.save(s, base_input.map(|b| &b.sensors))?;
    }
    // The commands.
    let any = !pilot_commands.is_empty() || !commands.is_empty();
    s.writer().write_bool(any);
    if any {
        pilot_commands.save(s, None)?;
        commands.save(s, None)?;
    }
    // The view.
    if base_input.is_some_and(|b| b.view == *view) {
        s.writer().write_bool(true);
    } else {
        s.writer().write_bool(false);
        match view {
            None => {
                let _ = s.writer().write_bits(VIEW_NONE, 2);
            }
            Some(v) if v.tick <= *tick && *tick - v.tick <= 255 && v.interpolation_delay <= 63 => {
                let w = s.writer();
                let _ = w.write_bits(VIEW_COMPACT, 2);
                let _ = w.write_bits(*tick - v.tick, 8);
                let _ = w.write_bits(u64::from(v.interpolation_delay), 6);
            }
            Some(v) => {
                let _ = s.writer().write_bits(VIEW_WHOLE, 2);
                v.save(s, None)?;
            }
        }
    }
    Ok(())
}

/// Reads a seat's input at `tick` that [`write_seat_input`] wrote, and the
/// number of its last command applied.
fn read_seat_input(
    l: &mut Loader<'_>,
    tick: u64,
    baselines: &Baselines,
) -> WireResult<(SeatInput, u16)> {
    let seat = SeatId(l.reader().read_bits(8)? as u8);
    let base = baselines.baseline(seat, tick);
    let base_input = base.as_ref().map(|(b, _)| b);
    let applied = if l.reader().read_bool()? {
        base.as_ref()
            .ok_or(invalid("a command number with no baseline"))?
            .1
    } else {
        l.reader().read_bits(16)? as u16
    };
    let (mut pilot, trigger, sensors) = if l.reader().read_bool()? {
        let previous = base_input.map(|b| InputFrame::of(&b.pilot, b.trigger, b.sensors));
        let frame = read_frame(l.reader(), previous.as_ref())?;
        (frame.pilot(), frame.trigger, frame.sensors)
    } else {
        let b = base_input.map(|b| &b.pilot);
        let pilot = PilotInput {
            pitch: Checkpoint::load(l, b.map(|b| &b.pitch))?,
            roll: Checkpoint::load(l, b.map(|b| &b.roll))?,
            yaw: Checkpoint::load(l, b.map(|b| &b.yaw))?,
            throttle_rate: Checkpoint::load(l, b.map(|b| &b.throttle_rate))?,
            throttle: Checkpoint::load(l, b.map(|b| &b.throttle))?,
            commands: Vec::new(),
        };
        let trigger = Checkpoint::load(l, base_input.map(|b| &b.trigger))?;
        let sensors = Checkpoint::load(l, base_input.map(|b| &b.sensors))?;
        (pilot, trigger, sensors)
    };
    let mut commands = Vec::new();
    if l.reader().read_bool()? {
        pilot.commands = Checkpoint::load(l, None)?;
        commands = Checkpoint::load(l, None)?;
        if pilot.commands.is_empty() && commands.is_empty() {
            return Err(invalid("commands present but none"));
        }
    }
    let view = if l.reader().read_bool()? {
        base_input.ok_or(invalid("a view with no baseline"))?.view
    } else {
        match l.reader().read_bits(2)? {
            VIEW_NONE => None,
            VIEW_COMPACT => {
                let offset = l.reader().read_bits(8)?;
                let interpolation_delay = l.reader().read_bits(6)? as u8;
                Some(SeatView {
                    tick: tick
                        .checked_sub(offset)
                        .ok_or(invalid("a view before tick 0"))?,
                    interpolation_delay,
                })
            }
            VIEW_WHOLE => Some(SeatView::load(l, None)?),
            _ => return Err(invalid("view coding")),
        }
    };
    let input = SeatInput {
        seat,
        tick,
        pilot,
        trigger,
        sensors,
        commands,
        view,
    };
    Ok((input, applied))
}

/// The host's side of one standby's stream: codes records in order.
#[derive(Clone, Debug, Default)]
pub struct StreamWriter {
    baselines: Baselines,
}

/// The standby's side: reads records in the order written.
#[derive(Clone, Debug, Default)]
pub struct StreamReader {
    baselines: Baselines,
}

fn invalid(what: &'static str) -> WireError {
    WireError::Invalid(what)
}

fn write_role(w: &mut BitWriter, role: StandbyMark) -> WireResult<()> {
    if role == StandbyMark::None {
        return Err(invalid("standby role"));
    }
    let _ = w.write_bits(u64::from(role.code()), 2);
    Ok(())
}

fn read_role(r: &mut BitReader<'_>) -> WireResult<StandbyMark> {
    match StandbyMark::from_code(r.read_bits(2)? as u8) {
        Some(StandbyMark::None) | None => Err(invalid("standby role")),
        Some(role) => Ok(role),
    }
}

fn read_u32(r: &mut BitReader<'_>) -> WireResult<u32> {
    crate::wire::bits::read_u32(r)
}

fn read_count(r: &mut BitReader<'_>, limit: usize, what: &'static str) -> WireResult<usize> {
    crate::wire::bits::read_count(r, limit, what)
}

fn check_count(count: usize, limit: usize, what: &'static str) -> WireResult<()> {
    if count > limit {
        return Err(WireError::TooMany { what, limit });
    }
    Ok(())
}

fn write_change(w: &mut BitWriter, change: Change) {
    match change {
        Change::Scoring(on) => {
            w.write_bool(false);
            w.write_bool(on);
        }
        Change::StoreCut { plane, weapons } => {
            w.write_bool(true);
            w.write_varint(u64::from(plane.0));
            let _ = w.write_bits(u64::from(weapons.value()), 2);
        }
    }
}

fn read_change(r: &mut BitReader<'_>) -> WireResult<Change> {
    if !r.read_bool()? {
        return Ok(Change::Scoring(r.read_bool()?));
    }
    let plane = PlaneId(read_u32(r)?);
    let weapons =
        RevivalWeapons::from_value(r.read_bits(2)? as u32).ok_or(invalid("revival weapons"))?;
    Ok(Change::StoreCut { plane, weapons })
}

impl StreamWriter {
    /// A stream with no baselines.
    pub fn new() -> Self {
        Self::default()
    }

    /// The record's bytes, for a message of kind 46. A Ticks record moves
    /// every seat's baseline on; an Appoint or a Flight starts them afresh.
    /// A record that breaks a bound is refused and changes nothing.
    pub fn encode(&mut self, record: &Record) -> WireResult<Vec<u8>> {
        let mut s = Saver::new();
        let mut baselines = self.baselines.clone();
        let _ = s.writer().write_bits(u64::from(record.code()), 4);
        match record {
            Record::Appoint(appoint) => {
                let Appoint {
                    role,
                    warm,
                    check_every,
                    checkpoint_every,
                    mission,
                } = appoint;
                let w = s.writer();
                write_role(w, *role)?;
                w.write_bool(*warm);
                w.write_varint(u64::from(*check_every));
                w.write_varint(u64::from(*checkpoint_every));
                w.write_varint(u64::from(*mission));
                baselines = Baselines::default();
            }
            Record::Dismiss => {}
            Record::Flight(flight) => {
                let FlightRecord {
                    mission,
                    spec_hash,
                    identity,
                } = flight;
                let w = s.writer();
                w.write_varint(u64::from(*mission));
                let _ = w.write_bits(*spec_hash, 64);
                let _ = w.write_bits(*identity, 64);
                baselines = Baselines::default();
            }
            Record::CheckpointBegin(begin) => {
                let CheckpointBegin {
                    tick,
                    length,
                    chunks,
                } = begin;
                let w = s.writer();
                let _ = w.write_bits(u64::from(*tick), 32);
                let _ = w.write_bits(u64::from(*length), 32);
                let _ = w.write_bits(u64::from(*chunks), 16);
            }
            Record::CheckpointChunk(chunk) => {
                check_count(chunk.bytes.len(), CHUNK_BYTES, "checkpoint chunk bytes")?;
                let w = s.writer();
                let _ = w.write_bits(u64::from(chunk.index), 16);
                write_long_bytes(w, &chunk.bytes);
            }
            Record::Ticks(ticks) => write_ticks(&mut s, ticks, &mut baselines)?,
            Record::State(part) => {
                let StatePart { part, tick, bytes } = part;
                let w = s.writer();
                let _ = w.write_bits(*part as u64, 8);
                let _ = w.write_bits(u64::from(*tick), 32);
                write_long_bytes(w, bytes);
            }
            Record::Check(check) => {
                let w = s.writer();
                let _ = w.write_bits(u64::from(check.tick), 32);
                let _ = w.write_bits(check.hash, 64);
            }
            Record::Ended(reason) => write_end_reason(s.writer(), *reason),
            Record::Handover { last_tick } => {
                let _ = s.writer().write_bits(u64::from(*last_tick), 32);
            }
        }
        let bytes = s.finish_section();
        if !s.into_records().is_empty() {
            return Err(invalid("a journal record with shared records"));
        }
        // The record must fit a message, its kind's body.
        check_count(bytes.len(), limits::MESSAGE, "standby record bytes")?;
        self.baselines = baselines;
        Ok(bytes)
    }
}

fn write_ticks(s: &mut Saver, ticks: &Ticks, baselines: &mut Baselines) -> WireResult<()> {
    let Ticks { first, ticks } = ticks;
    if !(1..=TICKS_PER_RECORD).contains(&ticks.len()) {
        return Err(invalid("ticks in a record"));
    }
    if u64::from(*first) + ticks.len() as u64 - 1 > u64::from(u32::MAX) {
        return Err(invalid("journal tick"));
    }
    let _ = s.writer().write_bits(u64::from(*first), 32);
    let _ = s.writer().write_bits(ticks.len() as u64, 8);
    for (index, tick) in ticks.iter().enumerate() {
        let Tick {
            tick: number,
            changes,
            mission,
            inputs,
            applied,
        } = tick;
        if *number != u64::from(*first) + index as u64 {
            return Err(invalid("journal ticks not consecutive"));
        }
        check_count(changes.len(), MAX_CHANGES, "changes")?;
        check_count(mission.len(), MAX_MISSION_COMMANDS, "mission commands")?;
        check_count(inputs.len(), MAX_SEAT_INPUTS, "seat inputs")?;
        if applied.len() != inputs.len() {
            return Err(invalid("commands applied"));
        }
        s.writer().write_varint(changes.len() as u64);
        for change in changes {
            write_change(s.writer(), *change);
        }
        s.writer().write_varint(mission.len() as u64);
        for command in mission {
            command.save(s, None)?;
        }
        s.writer().write_varint(inputs.len() as u64);
        for (input, applied) in inputs.iter().zip(applied) {
            if input.tick != *number {
                return Err(invalid("a seat input for another tick"));
            }
            let base = baselines.baseline(input.seat, *number);
            write_seat_input(s, input, *applied, base.as_ref())?;
            baselines
                .inputs
                .insert(input.seat, (input.clone(), *applied));
        }
    }
    Ok(())
}

impl StreamReader {
    /// A stream with no baselines.
    pub fn new() -> Self {
        Self::default()
    }

    /// Reads a record [`StreamWriter::encode`] wrote, the records in the
    /// order written. Damaged bytes are refused and change nothing.
    pub fn decode(&mut self, bytes: &[u8]) -> WireResult<Record> {
        check_count(bytes.len(), limits::MESSAGE, "standby record bytes")?;
        let models = Models::default();
        let mut l = Loader::new(bytes, &[], &models);
        let mut baselines = self.baselines.clone();
        let record = match l.reader().read_bits(4)? {
            0 => {
                let r = l.reader();
                let appoint = Appoint {
                    role: read_role(r)?,
                    warm: r.read_bool()?,
                    check_every: read_u32(r)?,
                    checkpoint_every: read_u32(r)?,
                    mission: read_u32(r)?,
                };
                baselines = Baselines::default();
                Record::Appoint(appoint)
            }
            1 => Record::Dismiss,
            2 => {
                let r = l.reader();
                let flight = FlightRecord {
                    mission: read_u32(r)?,
                    spec_hash: r.read_bits(64)?,
                    identity: r.read_bits(64)?,
                };
                baselines = Baselines::default();
                Record::Flight(flight)
            }
            3 => {
                let r = l.reader();
                Record::CheckpointBegin(CheckpointBegin {
                    tick: r.read_bits(32)? as u32,
                    length: r.read_bits(32)? as u32,
                    chunks: r.read_bits(16)? as u16,
                })
            }
            4 => {
                let r = l.reader();
                Record::CheckpointChunk(CheckpointChunk {
                    index: r.read_bits(16)? as u16,
                    bytes: read_long_bytes(r, CHUNK_BYTES, "checkpoint chunk bytes")?,
                })
            }
            5 => Record::Ticks(read_ticks(&mut l, &mut baselines)?),
            6 => {
                let r = l.reader();
                let part = Part::from_code(r.read_bits(8)? as u8).ok_or(invalid("state part"))?;
                Record::State(StatePart {
                    part,
                    tick: r.read_bits(32)? as u32,
                    bytes: read_long_bytes(r, limits::MESSAGE, "state part bytes")?,
                })
            }
            7 => {
                let r = l.reader();
                Record::Check(Check {
                    tick: r.read_bits(32)? as u32,
                    hash: r.read_bits(64)?,
                })
            }
            8 => Record::Ended(read_end_reason(l.reader())?),
            9 => Record::Handover {
                last_tick: l.reader().read_bits(32)? as u32,
            },
            _ => return Err(invalid("standby record type")),
        };
        l.finish().map_err(|_| WireError::Trailing)?;
        self.baselines = baselines;
        Ok(record)
    }
}

fn read_ticks(l: &mut Loader<'_>, baselines: &mut Baselines) -> WireResult<Ticks> {
    let first = l.reader().read_bits(32)? as u32;
    let count = l.reader().read_bits(8)? as usize;
    if !(1..=TICKS_PER_RECORD).contains(&count) {
        return Err(invalid("ticks in a record"));
    }
    if u64::from(first) + count as u64 - 1 > u64::from(u32::MAX) {
        return Err(invalid("journal tick"));
    }
    let mut ticks = Vec::with_capacity(count);
    for index in 0..count {
        let number = u64::from(first) + index as u64;
        let mut tick = Tick::new(number);
        let changes = read_count(l.reader(), MAX_CHANGES, "changes")?;
        for _ in 0..changes {
            tick.changes.push(read_change(l.reader())?);
        }
        let commands = read_count(l.reader(), MAX_MISSION_COMMANDS, "mission commands")?;
        for _ in 0..commands {
            tick.mission.push(MissionCommand::load(l, None)?);
        }
        let inputs = read_count(l.reader(), MAX_SEAT_INPUTS, "seat inputs")?;
        for _ in 0..inputs {
            let (input, applied) = read_seat_input(l, number, baselines)?;
            baselines
                .inputs
                .insert(input.seat, (input.clone(), applied));
            tick.push_input(input, applied);
        }
        ticks.push(tick);
    }
    Ok(Ticks { first, ticks })
}
