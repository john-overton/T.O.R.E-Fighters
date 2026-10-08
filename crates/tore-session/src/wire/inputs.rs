//! The Inputs section (client to host): the player's controls for recent
//! ticks, what the player's screen showed, and the numbered commands.
//!
//! See net-protocol.md, "Inputs". The client rounds its controls with
//! [`quantize_pilot`] before its own prediction steps them, so the host steps
//! exactly the numbers the client stepped. Single player never rounds.

use super::bits::read_u32;
use super::entity::{EntityKey, EntityKind};
use super::{WireError, WireResult, bits, limits};
use tore_codec::quant::{SIGNED_UNIT_I8_STEPS, SIGNED_UNIT_I16_STEPS, UNIT_U16_STEPS};
use tore_codec::{BitReader, BitWriter, CodecError};
use tore_input::pilot::FlightAxis;
use tore_sim::ai::wing::{Formation, PlayerApproach, PlayerBreak, PlayerOrder};
use tore_sim::airport;
use tore_sim::combat::live;
use tore_sim::flight::{PilotCommand, PilotInput, Switch};
use tore_sim::sensors::{Channel, Controls, RANGE_LADDER_NMI};
use tore_world::seats::{SeatCommand, SeatId, SeatInput, SeatView};
use tore_world::world::AirportInput;
use tore_world::world::replies::Reply;

/// One tick's continuous controls as the wire carries them: sticks at
/// 1/32,767, the throttle rate at 1/127, the throttle position at 1/65,535.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct InputFrame {
    pub pitch: i16,
    pub roll: i16,
    pub yaw: i16,
    pub throttle_rate: i8,
    pub throttle: Option<u16>,
    /// The trigger is held.
    pub trigger: bool,
    /// The scope controls: channel, range step and contact history.
    pub sensors: Controls,
    pub powered_lift: PoweredLiftInput,
}

/// Vector pitch/yaw, nacelle conversion and collective, in that order.
/// Idle fixed-wing input needs only the absent bit on the wire.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoweredLiftInput {
    pub rates: [i8; 4],
    pub positions: [Option<i16>; 4],
}
const FLIGHT_AXES: [FlightAxis; 4] = [
    FlightAxis::VectorPitch,
    FlightAxis::VectorYaw,
    FlightAxis::Conversion,
    FlightAxis::Collective,
];
fn axis_position(axis: FlightAxis, value: f64) -> Option<i16> {
    value.is_finite().then(|| {
        stick(if axis == FlightAxis::VectorYaw {
            value
        } else {
            value.clamp(0., 1.)
        })
    })
}

fn stick(value: f64) -> i16 {
    let value = tore_input::pilot::bipolar(value);
    tore_codec::quant::quantize_signed_unit(value, SIGNED_UNIT_I16_STEPS).unwrap_or(0) as i16
}

fn stick_value(q: i16) -> f64 {
    f64::from(q) / f64::from(SIGNED_UNIT_I16_STEPS)
}

fn throttle_rate(value: f64) -> i8 {
    let value = tore_input::pilot::bipolar(value);
    tore_codec::quant::quantize_signed_unit(value, SIGNED_UNIT_I8_STEPS).unwrap_or(0) as i8
}

fn throttle_position(value: f64) -> Option<u16> {
    value.is_finite().then(|| {
        tore_codec::quant::quantize_unit(value.clamp(0., 1.), UNIT_U16_STEPS).unwrap_or(0) as u16
    })
}

impl InputFrame {
    /// The frame for `pilot`'s sticks and throttle, the trigger and the scope
    /// controls, rounded to the wire's steps. A throttle position that is
    /// not finite is dropped, as [`PilotInput::bounded`] drops it.
    pub fn of(pilot: &PilotInput, trigger: bool, sensors: Controls) -> Self {
        Self {
            pitch: stick(pilot.pitch),
            roll: stick(pilot.roll),
            yaw: stick(pilot.yaw),
            throttle_rate: throttle_rate(pilot.throttle_rate),
            throttle: pilot.throttle.and_then(throttle_position),
            trigger,
            sensors,
            powered_lift: PoweredLiftInput {
                rates: [
                    pilot.vector_pitch_rate,
                    pilot.vector_yaw_rate,
                    pilot.conversion_rate,
                    pilot.collective_rate,
                ]
                .map(throttle_rate),
                positions: std::array::from_fn(|index| {
                    [
                        pilot.vector_pitch,
                        pilot.vector_yaw,
                        pilot.conversion,
                        pilot.collective,
                    ][index]
                        .and_then(|value| axis_position(FLIGHT_AXES[index], value))
                }),
            },
        }
    }

    /// The pilot input these controls stand for, with no commands.
    pub fn pilot(&self) -> PilotInput {
        PilotInput {
            pitch: stick_value(self.pitch),
            roll: stick_value(self.roll),
            yaw: stick_value(self.yaw),
            throttle_rate: f64::from(self.throttle_rate) / f64::from(SIGNED_UNIT_I8_STEPS),
            throttle: self
                .throttle
                .map(|q| f64::from(q) / f64::from(UNIT_U16_STEPS)),
            commands: Vec::new(),
            vector_pitch_rate: f64::from(self.powered_lift.rates[0])
                / f64::from(SIGNED_UNIT_I8_STEPS),
            vector_yaw_rate: f64::from(self.powered_lift.rates[1])
                / f64::from(SIGNED_UNIT_I8_STEPS),
            conversion_rate: f64::from(self.powered_lift.rates[2])
                / f64::from(SIGNED_UNIT_I8_STEPS),
            collective_rate: f64::from(self.powered_lift.rates[3])
                / f64::from(SIGNED_UNIT_I8_STEPS),
            vector_pitch: self.powered_lift.positions[0].map(stick_value),
            vector_yaw: self.powered_lift.positions[1].map(stick_value),
            conversion: self.powered_lift.positions[2].map(stick_value),
            collective: self.powered_lift.positions[3].map(stick_value),
        }
    }

    /// The seat's input for `tick` with these controls, the `commands` the
    /// host applies at that tick in number order (pilot commands go into the
    /// pilot input, seat commands into the seat's list, each keeping its
    /// order) and what the player's screen showed.
    pub fn seat_input(
        &self,
        seat: SeatId,
        tick: u64,
        commands: &[Command],
        view: Option<SeatView>,
    ) -> SeatInput {
        let mut pilot = self.pilot();
        let mut seat_commands = Vec::new();
        for command in commands {
            match *command {
                Command::Pilot(command) => pilot.commands.push(command),
                Command::Seat(command) => seat_commands.push(command),
            }
        }
        SeatInput {
            seat,
            tick,
            pilot,
            trigger: self.trigger,
            sensors: self.sensors,
            commands: seat_commands,
            view,
        }
    }
}

/// `pilot` exactly as the host will step it: sticks and throttle rounded to
/// the wire's steps, and each command rounded as [`quantize_command`] does.
/// The client steps this in its own prediction.
pub fn quantize_pilot(pilot: &PilotInput) -> PilotInput {
    let mut out = InputFrame::of(pilot, false, Controls::default()).pilot();
    out.commands = pilot
        .commands
        .iter()
        .map(|&c| quantize_command(c))
        .collect();
    out
}

/// A pilot command as the host will apply it: a throttle position rounded
/// to 1/65,535 and a throttle step to 1/32,767, each clamped to its range.
pub fn quantize_command(command: PilotCommand) -> PilotCommand {
    match command {
        PilotCommand::Throttle(value) => PilotCommand::Throttle(
            throttle_position(value).map_or(f64::NAN, |q| f64::from(q) / f64::from(UNIT_U16_STEPS)),
        ),
        PilotCommand::AdjustThrottle(value) => {
            PilotCommand::AdjustThrottle(stick_value(stick(value)))
        }
        PilotCommand::SetAxis(axis, value) => PilotCommand::SetAxis(
            axis,
            axis_position(axis, value).map_or(f64::NAN, stick_value),
        ),
        PilotCommand::AdjustAxis(axis, value) => {
            PilotCommand::AdjustAxis(axis, stick_value(stick(value)))
        }
        other => other,
    }
}

/// A command a player gives once: every [`SeatCommand`] and every pilot
/// command.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Command {
    Seat(SeatCommand),
    Pilot(PilotCommand),
}

/// A command with its number and the tick at which the client applied it in
/// its own prediction.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NumberedCommand {
    pub number: u16,
    pub tick: u32,
    pub command: Command,
}

/// What the player's view follows, when it is not the own cockpit.
pub type ViewSubject = EntityKey;

/// The Inputs section.
#[derive(Clone, Debug, PartialEq)]
pub struct InputsSection {
    /// The connection's flight these inputs fly: the number the Seated
    /// message gave (protocol 3). The host drops inputs of another flight.
    pub flight: u8,
    /// The last tick in the section.
    pub newest_tick: u32,
    /// One frame per tick, oldest first, ending at `newest_tick`: 1 to 24.
    pub frames: Vec<InputFrame>,
    /// The host tick the screen showed when the newest tick was sampled, as
    /// ticks before the newest.
    pub view_offset: u8,
    /// The client's interpolation delay in ticks, 0 to 63.
    pub interpolation_delay: u8,
    /// What the player's view follows (target, wing, external or fly-by
    /// view); the host sends it at the full rate.
    pub view_subject: Option<ViewSubject>,
    /// The newest snapshot tick whose own-state hash differed from the
    /// client's, or 0.
    pub mismatch: u32,
    /// The unacknowledged commands, their numbers consecutive.
    pub commands: Vec<NumberedCommand>,
}

impl InputsSection {
    /// The tick of `frames[index]`.
    pub fn frame_tick(&self, index: usize) -> u32 {
        let back = (self.frames.len() - 1 - index) as u32;
        self.newest_tick.wrapping_sub(back)
    }

    /// What the screen showed for the newest tick, for lag compensation.
    pub fn view(&self) -> SeatView {
        SeatView {
            tick: u64::from(self.newest_tick.saturating_sub(u32::from(self.view_offset))),
            interpolation_delay: self.interpolation_delay,
        }
    }

    /// The section's bytes.
    pub fn encode(&self) -> WireResult<Vec<u8>> {
        let count = self.frames.len();
        if !(1..=limits::INPUT_TICKS).contains(&count) {
            return Err(WireError::Invalid("input tick count"));
        }
        if self.newest_tick < count as u32 - 1 {
            return Err(WireError::Invalid("input ticks before tick 0"));
        }
        if self.commands.len() > limits::COMMANDS {
            return Err(WireError::TooMany {
                what: "commands",
                limit: limits::COMMANDS,
            });
        }
        if self.interpolation_delay > 63 {
            return Err(WireError::Invalid("interpolation delay"));
        }
        let mut w = BitWriter::with_capacity(64);
        let _ = w.write_bits(u64::from(self.flight), 8);
        let _ = w.write_bits(u64::from(self.newest_tick), 32);
        let _ = w.write_bits(count as u64, 5);
        let _ = w.write_bits(u64::from(self.view_offset), 8);
        let _ = w.write_bits(u64::from(self.interpolation_delay), 6);
        bits::write_option(&mut w, self.view_subject, |w, key| {
            let _ = w.write_bits(u64::from(key.kind.code()), 2);
            w.write_varint(u64::from(key.id));
        });
        let _ = w.write_bits(u64::from(self.mismatch), 32);
        let mut previous: Option<&InputFrame> = None;
        for frame in &self.frames {
            write_frame(&mut w, frame, previous)?;
            previous = Some(frame);
        }
        let _ = w.write_bits(self.commands.len() as u64, 7);
        if let Some(first) = self.commands.first() {
            let _ = w.write_bits(u64::from(first.number), 16);
        }
        for (index, command) in self.commands.iter().enumerate() {
            if let Some(first) = self.commands.first()
                && command.number != first.number.wrapping_add(index as u16)
            {
                return Err(WireError::Invalid("command numbers not consecutive"));
            }
            if command.tick > self.newest_tick {
                return Err(WireError::Invalid("command after the newest tick"));
            }
            w.write_varint(u64::from(self.newest_tick - command.tick));
            write_command(&mut w, &command.command);
        }
        Ok(bits::finish(w))
    }

    /// Reads a section [`Self::encode`] wrote.
    pub fn decode(bytes: &[u8]) -> WireResult<Self> {
        let mut r = BitReader::new(bytes);
        let flight = r.read_bits(8)? as u8;
        let newest_tick = r.read_bits(32)? as u32;
        let count = r.read_bits(5)? as usize;
        if !(1..=limits::INPUT_TICKS).contains(&count) {
            return Err(WireError::Invalid("input tick count"));
        }
        if newest_tick < count as u32 - 1 {
            return Err(WireError::Invalid("input ticks before tick 0"));
        }
        let view_offset = r.read_bits(8)? as u8;
        let interpolation_delay = r.read_bits(6)? as u8;
        let view_subject = bits::read_option(&mut r, |r| {
            let kind = EntityKind::from_code(r.read_bits(2)? as u8);
            Ok(EntityKey {
                kind,
                id: read_u32(r)?,
            })
        })?;
        let mismatch = r.read_bits(32)? as u32;
        let mut frames: Vec<InputFrame> = Vec::with_capacity(count);
        for _ in 0..count {
            let frame = read_frame(&mut r, frames.last())?;
            frames.push(frame);
        }
        let command_count = r.read_bits(7)? as usize;
        if command_count > limits::COMMANDS {
            return Err(WireError::TooMany {
                what: "commands",
                limit: limits::COMMANDS,
            });
        }
        let first = if command_count > 0 {
            r.read_bits(16)? as u16
        } else {
            0
        };
        let mut commands = Vec::with_capacity(command_count);
        for index in 0..command_count {
            let back = r.read_varint()?;
            if back > u64::from(newest_tick) {
                return Err(WireError::Invalid("command before tick 0"));
            }
            commands.push(NumberedCommand {
                number: first.wrapping_add(index as u16),
                tick: newest_tick - back as u32,
                command: read_command(&mut r)?,
            });
        }
        bits::end(&mut r)?;
        Ok(Self {
            flight,
            newest_tick,
            frames,
            view_offset,
            interpolation_delay,
            view_subject,
            mismatch,
            commands,
        })
    }
}

/// Stick differences from the tick before: 4, 8 or 17 bits.
const STICK_LADDER: [u32; 3] = [4, 8, 17];

fn write_sensors(w: &mut BitWriter, sensors: &Controls) -> WireResult<()> {
    let channel = match sensors.channel {
        Channel::Radar => 0,
        Channel::Infrared => 1,
        Channel::Visual => 2,
    };
    if sensors.range_index >= RANGE_LADDER_NMI.len() {
        return Err(WireError::Invalid("scope range step"));
    }
    let _ = w.write_bits(channel, 2);
    let _ = w.write_bits(sensors.range_index as u64, 4);
    w.write_bool(sensors.history);
    Ok(())
}

fn read_sensors(r: &mut BitReader<'_>) -> WireResult<Controls> {
    let channel = match r.read_bits(2)? {
        0 => Channel::Radar,
        1 => Channel::Infrared,
        2 => Channel::Visual,
        _ => return Err(WireError::Invalid("scope channel")),
    };
    let range_index = r.read_bits(4)? as usize;
    if range_index >= RANGE_LADDER_NMI.len() {
        return Err(WireError::Invalid("scope range step"));
    }
    Ok(Controls {
        channel,
        range_index,
        history: r.read_bool()?,
    })
}

pub(crate) fn write_frame(
    w: &mut BitWriter,
    frame: &InputFrame,
    previous: Option<&InputFrame>,
) -> WireResult<()> {
    let Some(previous) = previous else {
        let _ = w.write_signed(i64::from(frame.pitch), 16);
        let _ = w.write_signed(i64::from(frame.roll), 16);
        let _ = w.write_signed(i64::from(frame.yaw), 16);
        let _ = w.write_signed(i64::from(frame.throttle_rate), 8);
        bits::write_option(w, frame.throttle, |w, q| {
            let _ = w.write_bits(u64::from(q), 16);
        });
        w.write_bool(frame.trigger);
        write_powered_lift(w, frame.powered_lift);
        return write_sensors(w, &frame.sensors);
    };
    if frame == previous {
        w.write_bool(true);
        return Ok(());
    }
    w.write_bool(false);
    let changed = [
        frame.pitch != previous.pitch,
        frame.roll != previous.roll,
        frame.yaw != previous.yaw,
        frame.throttle_rate != previous.throttle_rate,
        frame.throttle != previous.throttle,
        frame.trigger != previous.trigger,
        frame.sensors != previous.sensors,
        frame.powered_lift != previous.powered_lift,
    ];
    for flag in changed {
        w.write_bool(flag);
    }
    for (axis, before, flag) in [
        (frame.pitch, previous.pitch, changed[0]),
        (frame.roll, previous.roll, changed[1]),
        (frame.yaw, previous.yaw, changed[2]),
    ] {
        if flag {
            w.write_bucketed(i64::from(axis) - i64::from(before), &STICK_LADDER)?;
        }
    }
    if changed[3] {
        let _ = w.write_signed(i64::from(frame.throttle_rate), 8);
    }
    if changed[4] {
        bits::write_option(w, frame.throttle, |w, q| {
            let _ = w.write_bits(u64::from(q), 16);
        });
    }
    // The trigger's new value is the old one flipped: its flag says it all.
    if changed[6] {
        write_sensors(w, &frame.sensors)?;
    }
    if changed[7] {
        write_powered_lift(w, frame.powered_lift);
    }
    Ok(())
}

fn read_stick(r: &mut BitReader<'_>) -> WireResult<i16> {
    let q = r.read_signed(16)?;
    if q < -i64::from(SIGNED_UNIT_I16_STEPS) {
        return Err(WireError::Invalid("stick"));
    }
    Ok(q as i16)
}

fn read_rate(r: &mut BitReader<'_>) -> WireResult<i8> {
    let q = r.read_signed(8)?;
    if q < -i64::from(SIGNED_UNIT_I8_STEPS) {
        return Err(WireError::Invalid("throttle rate"));
    }
    Ok(q as i8)
}

pub(crate) fn write_powered_lift(w: &mut BitWriter, input: PoweredLiftInput) {
    let present = input != PoweredLiftInput::default();
    w.write_bool(present);
    if present {
        for rate in input.rates {
            let _ = w.write_signed(i64::from(rate), 8);
        }
        for position in input.positions {
            bits::write_option(w, position, |w, value| {
                let _ = w.write_signed(i64::from(value), 16);
            });
        }
    }
}
pub(crate) fn read_powered_lift(r: &mut BitReader<'_>) -> WireResult<PoweredLiftInput> {
    if !r.read_bool()? {
        return Ok(PoweredLiftInput::default());
    }
    let mut input = PoweredLiftInput::default();
    for rate in &mut input.rates {
        *rate = read_rate(r)?;
    }
    for (index, position) in input.positions.iter_mut().enumerate() {
        *position = bits::read_option(r, read_stick)?;
        if index != 1 && position.is_some_and(|value| value < 0) {
            return Err(WireError::Invalid("powered-lift position"));
        }
    }
    if input == PoweredLiftInput::default() {
        return Err(CodecError::NonCanonical.into());
    }
    Ok(input)
}

pub(crate) fn read_frame(
    r: &mut BitReader<'_>,
    previous: Option<&InputFrame>,
) -> WireResult<InputFrame> {
    let Some(previous) = previous else {
        return Ok(InputFrame {
            pitch: read_stick(r)?,
            roll: read_stick(r)?,
            yaw: read_stick(r)?,
            throttle_rate: read_rate(r)?,
            throttle: bits::read_option(r, |r| Ok(r.read_bits(16)? as u16))?,
            trigger: r.read_bool()?,
            powered_lift: read_powered_lift(r)?,
            sensors: read_sensors(r)?,
        });
    };
    if r.read_bool()? {
        return Ok(*previous);
    }
    let mut changed = [false; 8];
    for flag in &mut changed {
        *flag = r.read_bool()?;
    }
    if !changed.contains(&true) {
        return Err(CodecError::NonCanonical.into());
    }
    let mut frame = *previous;
    for (index, axis) in [&mut frame.pitch, &mut frame.roll, &mut frame.yaw]
        .into_iter()
        .enumerate()
    {
        if changed[index] {
            let value = i64::from(*axis) + r.read_bucketed(&STICK_LADDER)?;
            if value == i64::from(*axis) {
                return Err(CodecError::NonCanonical.into());
            }
            if value.unsigned_abs() > u64::from(SIGNED_UNIT_I16_STEPS) {
                return Err(WireError::Invalid("stick"));
            }
            *axis = value as i16;
        }
    }
    if changed[3] {
        frame.throttle_rate = read_rate(r)?;
    }
    if changed[4] {
        frame.throttle = bits::read_option(r, |r| Ok(r.read_bits(16)? as u16))?;
    }
    if changed[5] {
        frame.trigger = !frame.trigger;
    }
    if changed[6] {
        frame.sensors = read_sensors(r)?;
    }
    if changed[7] {
        frame.powered_lift = read_powered_lift(r)?;
    }
    if (changed[3] && frame.throttle_rate == previous.throttle_rate)
        || (changed[4] && frame.throttle == previous.throttle)
        || (changed[6] && frame.sensors == previous.sensors)
        || (changed[7] && frame.powered_lift == previous.powered_lift)
    {
        return Err(CodecError::NonCanonical.into());
    }
    Ok(frame)
}

// Command codes. The numbers are the wire's; the golden test pins them.
const CYCLE_WEAPON: u64 = 0;
const NAV_MODE: u64 = 1;
const SELECT_AIRPORT: u64 = 2;
const REQUEST_LANDING: u64 = 3;
const REPEAT_REPLY: u64 = 4;
const CANCEL_APPROACH: u64 = 5;
const COMBAT: u64 = 6;
const MANUAL: u64 = 7;
const RANGE_RESET: u64 = 8;
const RELEASE_CHAFF: u64 = 9;
const RELEASE_FLARE: u64 = 10;
const RELEASE_TRIGGER: u64 = 11;
const RADIO_SILENCE: u64 = 12;
const WING_RECIPIENT: u64 = 13;
const WING_ORDER: u64 = 14;
const WING_FORMATION_CYCLE: u64 = 15;
const TRIGGER_KEY: u64 = 16;
const EJECT: u64 = 17;
const TOGGLE: u64 = 18;
const SET: u64 = 19;
const THROTTLE: u64 = 20;
const ADJUST_THROTTLE: u64 = 21;
/// Protocol 8 (stage F phase 2): a wingman's reply, then its kind in 2 bits.
const WING_REPLY: u64 = 22;
/// Stage G (slice G8): Alt+N, start or stop monitoring the battle net. No
/// fields. The protocol version stays for the wire slice (G7) to bump.
const BATTLE_NET: u64 = 23;
/// Protocol 18 (powered lift): set a vector, conversion or collective axis,
/// then its 2-bit axis code and an optional position.
const SET_FLIGHT_AXIS: u64 = 24;
/// Protocol 18: step a powered-lift axis, then its 2-bit axis code and a
/// signed 16-bit step.
const ADJUST_FLIGHT_AXIS: u64 = 25;
/// Protocol 18: return the thrust vector to neutral. No fields.
const NEUTRAL_VECTOR: u64 = 26;
const COMMAND_BITS: u32 = 5;

/// Writes one command.
pub(crate) fn write_command(w: &mut BitWriter, command: &Command) {
    let mut code = |value: u64| {
        let _ = w.write_bits(value, COMMAND_BITS);
    };
    match *command {
        Command::Seat(seat) => match seat {
            SeatCommand::CycleWeapon { forward } => {
                code(CYCLE_WEAPON);
                w.write_bool(forward);
            }
            SeatCommand::Airport(AirportInput::NavMode) => code(NAV_MODE),
            SeatCommand::Airport(AirportInput::Command(command)) => match command {
                airport::Command::SelectAirport(id) => {
                    code(SELECT_AIRPORT);
                    w.write_varint(u64::from(id));
                }
                airport::Command::RequestLanding => code(REQUEST_LANDING),
                airport::Command::RepeatReply => code(REPEAT_REPLY),
                airport::Command::CancelApproach => code(CANCEL_APPROACH),
            },
            SeatCommand::Combat(command) => {
                code(COMBAT);
                write_live(w, command);
            }
            SeatCommand::Manual(command) => {
                code(MANUAL);
                write_live(w, command);
            }
            SeatCommand::RangeReset => code(RANGE_RESET),
            SeatCommand::ReleaseChaff => code(RELEASE_CHAFF),
            SeatCommand::ReleaseFlare => code(RELEASE_FLARE),
            SeatCommand::ReleaseTrigger => code(RELEASE_TRIGGER),
            SeatCommand::RadioSilence => code(RADIO_SILENCE),
            SeatCommand::WingRecipient(recipient) => {
                code(WING_RECIPIENT);
                bits::write_option(w, recipient, |w, member| {
                    let _ = w.write_bits(u64::from(member), 8);
                });
            }
            SeatCommand::WingOrder(order) => {
                code(WING_ORDER);
                write_order(w, order);
            }
            SeatCommand::WingFormationCycle => code(WING_FORMATION_CYCLE),
            SeatCommand::TriggerKey {
                down,
                repeat,
                blocked,
            } => {
                code(TRIGGER_KEY);
                w.write_bool(down);
                w.write_bool(repeat);
                w.write_bool(blocked);
            }
            SeatCommand::WingReply(reply) => {
                code(WING_REPLY);
                let _ = w.write_bits(reply_code(reply), 2);
            }
            SeatCommand::BattleNet => code(BATTLE_NET),
        },
        Command::Pilot(pilot) => match quantize_command(pilot) {
            PilotCommand::Eject => code(EJECT),
            PilotCommand::Toggle(switch) => {
                code(TOGGLE);
                let _ = w.write_bits(switch_code(switch), 4);
            }
            PilotCommand::Set(switch, on) => {
                code(SET);
                let _ = w.write_bits(switch_code(switch), 4);
                w.write_bool(on);
            }
            PilotCommand::Throttle(value) => {
                code(THROTTLE);
                // Not finite rounds to no position: the flight ignores it.
                bits::write_option(w, throttle_position(value), |w, q| {
                    let _ = w.write_bits(u64::from(q), 16);
                });
            }
            PilotCommand::AdjustThrottle(value) => {
                code(ADJUST_THROTTLE);
                let _ = w.write_signed(i64::from(stick(value)), 16);
            }
            PilotCommand::SetAxis(axis, value) => {
                code(SET_FLIGHT_AXIS);
                let _ = w.write_bits(flight_axis_code(axis), 2);
                bits::write_option(w, axis_position(axis, value), |w, q| {
                    let _ = w.write_signed(i64::from(q), 16);
                });
            }
            PilotCommand::AdjustAxis(axis, value) => {
                code(ADJUST_FLIGHT_AXIS);
                let _ = w.write_bits(flight_axis_code(axis), 2);
                let _ = w.write_signed(i64::from(stick(value)), 16);
            }
            PilotCommand::NeutralVector => code(NEUTRAL_VECTOR),
        },
    }
}

/// Reads one command.
pub(crate) fn read_command(r: &mut BitReader<'_>) -> WireResult<Command> {
    let seat = |command| Ok(Command::Seat(command));
    match r.read_bits(COMMAND_BITS)? {
        CYCLE_WEAPON => seat(SeatCommand::CycleWeapon {
            forward: r.read_bool()?,
        }),
        NAV_MODE => seat(SeatCommand::Airport(AirportInput::NavMode)),
        SELECT_AIRPORT => seat(SeatCommand::Airport(AirportInput::Command(
            airport::Command::SelectAirport(read_u32(r)?),
        ))),
        REQUEST_LANDING => seat(SeatCommand::Airport(AirportInput::Command(
            airport::Command::RequestLanding,
        ))),
        REPEAT_REPLY => seat(SeatCommand::Airport(AirportInput::Command(
            airport::Command::RepeatReply,
        ))),
        CANCEL_APPROACH => seat(SeatCommand::Airport(AirportInput::Command(
            airport::Command::CancelApproach,
        ))),
        COMBAT => seat(SeatCommand::Combat(read_live(r)?)),
        MANUAL => seat(SeatCommand::Manual(read_live(r)?)),
        RANGE_RESET => seat(SeatCommand::RangeReset),
        RELEASE_CHAFF => seat(SeatCommand::ReleaseChaff),
        RELEASE_FLARE => seat(SeatCommand::ReleaseFlare),
        RELEASE_TRIGGER => seat(SeatCommand::ReleaseTrigger),
        RADIO_SILENCE => seat(SeatCommand::RadioSilence),
        WING_RECIPIENT => seat(SeatCommand::WingRecipient(bits::read_option(r, |r| {
            Ok(r.read_bits(8)? as u8)
        })?)),
        WING_ORDER => seat(SeatCommand::WingOrder(read_order(r)?)),
        WING_FORMATION_CYCLE => seat(SeatCommand::WingFormationCycle),
        TRIGGER_KEY => seat(SeatCommand::TriggerKey {
            down: r.read_bool()?,
            repeat: r.read_bool()?,
            blocked: r.read_bool()?,
        }),
        WING_REPLY => seat(SeatCommand::WingReply(Reply::ALL[r.read_bits(2)? as usize])),
        BATTLE_NET => seat(SeatCommand::BattleNet),
        EJECT => Ok(Command::Pilot(PilotCommand::Eject)),
        TOGGLE => Ok(Command::Pilot(PilotCommand::Toggle(read_switch(r)?))),
        SET => {
            let switch = read_switch(r)?;
            Ok(Command::Pilot(PilotCommand::Set(switch, r.read_bool()?)))
        }
        THROTTLE => {
            let q = bits::read_option(r, |r| Ok(r.read_bits(16)? as u16))?;
            Ok(Command::Pilot(PilotCommand::Throttle(
                q.map_or(f64::NAN, |q| f64::from(q) / f64::from(UNIT_U16_STEPS)),
            )))
        }
        ADJUST_THROTTLE => Ok(Command::Pilot(PilotCommand::AdjustThrottle(stick_value(
            read_stick(r)?,
        )))),
        SET_FLIGHT_AXIS => {
            let axis = FLIGHT_AXES[r.read_bits(2)? as usize];
            let value = bits::read_option(r, read_stick)?;
            if axis != FlightAxis::VectorYaw && value.is_some_and(|v| v < 0) {
                return Err(WireError::Invalid("flight axis position"));
            }
            Ok(Command::Pilot(PilotCommand::SetAxis(
                axis,
                value.map_or(f64::NAN, stick_value),
            )))
        }
        ADJUST_FLIGHT_AXIS => {
            let axis = FLIGHT_AXES[r.read_bits(2)? as usize];
            Ok(Command::Pilot(PilotCommand::AdjustAxis(
                axis,
                stick_value(read_stick(r)?),
            )))
        }
        NEUTRAL_VECTOR => Ok(Command::Pilot(PilotCommand::NeutralVector)),
        _ => Err(WireError::Invalid("command")),
    }
}

/// A wingman's reply's code: its place in [`Reply::ALL`]. The match is
/// exhaustive so a new kind cannot go uncoded.
fn reply_code(reply: Reply) -> u64 {
    match reply {
        Reply::Engaging => 0,
        Reply::Winchester => 1,
        Reply::BingoFuel => 2,
        Reply::NeedHelp => 3,
    }
}

fn flight_axis_code(axis: FlightAxis) -> u64 {
    match axis {
        FlightAxis::VectorPitch => 0,
        FlightAxis::VectorYaw => 1,
        FlightAxis::Conversion => 2,
        FlightAxis::Collective => 3,
    }
}

const SWITCHES: [Switch; 11] = [
    Switch::Gear,
    Switch::Flaps,
    Switch::Airbrake,
    Switch::Hook,
    Switch::Bay,
    Switch::Engine,
    Switch::Burner,
    Switch::Radar,
    Switch::Jammer,
    Switch::Autopilot,
    Switch::WaypointAutopilot,
];

/// A switch's code: its place in [`SWITCHES`]. The match is exhaustive so a
/// new switch cannot go uncoded.
fn switch_code(switch: Switch) -> u64 {
    match switch {
        Switch::Gear => 0,
        Switch::Flaps => 1,
        Switch::Airbrake => 2,
        Switch::Hook => 3,
        Switch::Bay => 4,
        Switch::Engine => 5,
        Switch::Burner => 6,
        Switch::Radar => 7,
        Switch::Jammer => 8,
        Switch::Autopilot => 9,
        Switch::WaypointAutopilot => 10,
    }
}

fn read_switch(r: &mut BitReader<'_>) -> WireResult<Switch> {
    SWITCHES
        .get(r.read_bits(4)? as usize)
        .copied()
        .ok_or(WireError::Invalid("switch"))
}

/// Combat's commands without a value, in code order; the three with a value
/// follow as codes 23 to 25.
const LIVE: [live::Command; 23] = [
    live::Command::NextWeapon,
    live::Command::NextSelection,
    live::Command::PreviousSelection,
    live::Command::SelectNav,
    live::Command::AdvanceFromEmpty,
    live::Command::ToggleSeekerMode,
    live::Command::CompatibilityWeapons,
    live::Command::ClearRange,
    live::Command::ToggleTargetRadar,
    live::Command::Designate,
    live::Command::DesignatePrevious,
    live::Command::DesignateVisual,
    live::Command::ClearDesignation,
    live::Command::ToggleArm,
    live::Command::Jettison,
    live::Command::ReplaceTarget,
    live::Command::CycleClass,
    live::Command::FailStation,
    live::Command::DamagePlayer,
    live::Command::Incoming,
    live::Command::ToggleTargetJammer,
    live::Command::ReleaseChaff,
    live::Command::ReleaseFlare,
];
const TARGET_HEAT: u64 = 23;
const TARGET_DISTANCE: u64 = 24;
const DESIGNATE_TARGET: u64 = 25;
const NEXT_GUN_GROUP: u64 = 26;
const TOGGLE_GUN_GROUP: u64 = 27;

/// A combat command's code, exhaustively, so a new command cannot go
/// uncoded.
fn live_code(command: live::Command) -> u64 {
    use live::Command as C;
    match command {
        C::NextWeapon => 0,
        C::NextSelection => 1,
        C::PreviousSelection => 2,
        C::SelectNav => 3,
        C::AdvanceFromEmpty => 4,
        C::ToggleSeekerMode => 5,
        C::CompatibilityWeapons => 6,
        C::ClearRange => 7,
        C::ToggleTargetRadar => 8,
        C::Designate => 9,
        C::DesignatePrevious => 10,
        C::DesignateVisual => 11,
        C::ClearDesignation => 12,
        C::ToggleArm => 13,
        C::Jettison => 14,
        C::ReplaceTarget => 15,
        C::CycleClass => 16,
        C::FailStation => 17,
        C::DamagePlayer => 18,
        C::Incoming => 19,
        C::ToggleTargetJammer => 20,
        C::ReleaseChaff => 21,
        C::ReleaseFlare => 22,
        C::TargetHeat(_) => TARGET_HEAT,
        C::TargetDistance(_) => TARGET_DISTANCE,
        C::DesignateTarget(_) => DESIGNATE_TARGET,
        C::NextGunGroup => NEXT_GUN_GROUP,
        C::ToggleGunGroup => TOGGLE_GUN_GROUP,
    }
}

fn write_live(w: &mut BitWriter, command: live::Command) {
    let _ = w.write_bits(live_code(command), 5);
    match command {
        live::Command::TargetHeat(heat) => {
            let _ = w.write_bits(u64::from(heat), 8);
        }
        live::Command::TargetDistance(distance) => w.write_varint(u64::from(distance)),
        live::Command::DesignateTarget(id) => w.write_varint(u64::from(id)),
        _ => {}
    }
}

fn read_live(r: &mut BitReader<'_>) -> WireResult<live::Command> {
    Ok(match r.read_bits(5)? {
        TARGET_HEAT => live::Command::TargetHeat(r.read_bits(8)? as u8),
        TARGET_DISTANCE => live::Command::TargetDistance(read_u32(r)?),
        DESIGNATE_TARGET => live::Command::DesignateTarget(read_u32(r)?),
        NEXT_GUN_GROUP => live::Command::NextGunGroup,
        TOGGLE_GUN_GROUP => live::Command::ToggleGunGroup,
        code => *LIVE
            .get(code as usize)
            .ok_or(WireError::Invalid("combat command"))?,
    })
}

const BREAKS: [PlayerBreak; 5] = PlayerBreak::ALL;
const APPROACHES: [PlayerApproach; 4] = PlayerApproach::ALL;
const FORMATIONS: [Formation; 3] = Formation::ALL;

/// Writes a wing order (the Order reply event carries one too).
pub(crate) fn write_order(w: &mut BitWriter, order: PlayerOrder) {
    let mut code = |value: u64| {
        let _ = w.write_bits(value, 4);
    };
    match order {
        PlayerOrder::EngageMyTarget => code(0),
        PlayerOrder::ProtectMe => code(1),
        PlayerOrder::AttackOnContact => code(2),
        PlayerOrder::EngageFromFormation => code(3),
        PlayerOrder::Disengage => code(4),
        PlayerOrder::Break(side) => {
            code(5);
            let _ = w.write_bits(
                BREAKS.iter().position(|b| *b == side).unwrap_or(0) as u64,
                3,
            );
        }
        PlayerOrder::Approach(side) => {
            code(6);
            let _ = w.write_bits(
                APPROACHES.iter().position(|a| *a == side).unwrap_or(0) as u64,
                2,
            );
        }
        PlayerOrder::Formation(formation) => {
            code(7);
            let _ = w.write_bits(
                FORMATIONS.iter().position(|f| *f == formation).unwrap_or(0) as u64,
                2,
            );
        }
        PlayerOrder::Spacing => code(8),
        PlayerOrder::Stacking => code(9),
        PlayerOrder::ControlToggle => code(10),
        PlayerOrder::BugOut => code(11),
        PlayerOrder::LandAtSelected => code(12),
        PlayerOrder::Sort => code(13),
    }
}

/// Reads a wing order.
pub(crate) fn read_order(r: &mut BitReader<'_>) -> WireResult<PlayerOrder> {
    Ok(match r.read_bits(4)? {
        0 => PlayerOrder::EngageMyTarget,
        1 => PlayerOrder::ProtectMe,
        2 => PlayerOrder::AttackOnContact,
        3 => PlayerOrder::EngageFromFormation,
        4 => PlayerOrder::Disengage,
        5 => PlayerOrder::Break(
            *BREAKS
                .get(r.read_bits(3)? as usize)
                .ok_or(WireError::Invalid("break"))?,
        ),
        6 => PlayerOrder::Approach(APPROACHES[r.read_bits(2)? as usize]),
        7 => PlayerOrder::Formation(
            *FORMATIONS
                .get(r.read_bits(2)? as usize)
                .ok_or(WireError::Invalid("formation"))?,
        ),
        8 => PlayerOrder::Spacing,
        9 => PlayerOrder::Stacking,
        10 => PlayerOrder::ControlToggle,
        11 => PlayerOrder::BugOut,
        12 => PlayerOrder::LandAtSelected,
        13 => PlayerOrder::Sort,
        _ => return Err(WireError::Invalid("wing order")),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every top-level command code is its own and fits the 5-bit field, and
    /// combat's value-less codes end where its valued ones begin; two
    /// branches once both took 22 and 23, which a decoder reads as the first.
    #[test]
    fn command_codes_are_unique_and_fit_their_fields() {
        let codes = [
            CYCLE_WEAPON,
            NAV_MODE,
            SELECT_AIRPORT,
            REQUEST_LANDING,
            REPEAT_REPLY,
            CANCEL_APPROACH,
            COMBAT,
            MANUAL,
            RANGE_RESET,
            RELEASE_CHAFF,
            RELEASE_FLARE,
            RELEASE_TRIGGER,
            RADIO_SILENCE,
            WING_RECIPIENT,
            WING_ORDER,
            WING_FORMATION_CYCLE,
            TRIGGER_KEY,
            EJECT,
            TOGGLE,
            SET,
            THROTTLE,
            ADJUST_THROTTLE,
            WING_REPLY,
            BATTLE_NET,
            SET_FLIGHT_AXIS,
            ADJUST_FLIGHT_AXIS,
            NEUTRAL_VECTOR,
        ];
        let unique: std::collections::BTreeSet<u64> = codes.iter().copied().collect();
        assert_eq!(unique.len(), codes.len(), "a command code is used twice");
        assert!(codes.iter().all(|c| *c < 1 << COMMAND_BITS));
        let live = [
            TARGET_HEAT,
            TARGET_DISTANCE,
            DESIGNATE_TARGET,
            NEXT_GUN_GROUP,
            TOGGLE_GUN_GROUP,
        ];
        assert_eq!(TARGET_HEAT, LIVE.len() as u64);
        let unique: std::collections::BTreeSet<u64> = live.iter().copied().collect();
        assert_eq!(unique.len(), live.len());
        assert!(live.iter().all(|c| *c < 32));
        // And every sample command reads back as itself.
        for command in super::super::samples::commands() {
            let mut w = BitWriter::new();
            write_command(&mut w, &command);
            let bytes = w.finish();
            let mut r = BitReader::new(&bytes);
            let back = read_command(&mut r).unwrap();
            let expected = match command {
                Command::Pilot(pilot) => Command::Pilot(quantize_command(pilot)),
                other => other,
            };
            assert_eq!(format!("{back:?}"), format!("{expected:?}"));
        }
    }

    #[test]
    fn powered_lift_positions_and_releases_survive_delta_frames() {
        let neutral = InputFrame::default();
        let active = InputFrame::of(
            &PilotInput {
                vector_pitch: Some(0.9),
                vector_yaw: Some(-0.7),
                conversion: Some(0.3),
                collective: Some(0.),
                vector_pitch_rate: 0.5,
                vector_yaw_rate: -0.5,
                conversion_rate: 1.,
                collective_rate: -1.,
                ..PilotInput::default()
            },
            false,
            Controls::default(),
        );
        let frames = [neutral, active, active, neutral];
        let mut writer = BitWriter::new();
        let mut previous = None;
        for frame in &frames {
            write_frame(&mut writer, frame, previous).unwrap();
            previous = Some(frame);
        }
        let bytes = writer.finish();
        let mut reader = BitReader::new(&bytes);
        let mut previous = None;
        for expected in frames {
            let decoded = read_frame(&mut reader, previous.as_ref()).unwrap();
            assert_eq!(decoded, expected);
            previous = Some(decoded);
        }
        assert_eq!(active.pilot().collective, Some(0.));
        assert_eq!(neutral.pilot().collective, None);
    }

    #[test]
    fn powered_lift_decoder_rejects_invalid_unsigned_positions() {
        let bad = PoweredLiftInput {
            positions: [Some(-1), None, None, None],
            ..Default::default()
        };
        let mut writer = BitWriter::new();
        write_powered_lift(&mut writer, bad);
        assert!(read_powered_lift(&mut BitReader::new(&writer.finish())).is_err());
    }

    #[test]
    fn quantized_controls_are_what_the_host_steps() {
        let pilot = PilotInput {
            pitch: 0.123_456_789,
            roll: -2.,
            yaw: f64::NAN,
            throttle_rate: 0.5,
            throttle: Some(0.333_333_333),
            commands: vec![
                PilotCommand::Throttle(0.777_777),
                PilotCommand::AdjustThrottle(0.05),
                PilotCommand::Toggle(Switch::Gear),
                PilotCommand::SetAxis(FlightAxis::Collective, 0.625),
                PilotCommand::AdjustAxis(FlightAxis::Conversion, 0.1),
                PilotCommand::NeutralVector,
            ],
            vector_pitch: Some(0.75),
            vector_yaw: Some(-0.6),
            conversion_rate: 0.2,
            collective_rate: -0.4,
            ..PilotInput::default()
        };
        let q = quantize_pilot(&pilot);
        assert_eq!(q.roll, -1.);
        assert_eq!(q.yaw, 0.);
        // Rounding twice changes nothing.
        assert_eq!(quantize_pilot(&q), q);
        let frame = InputFrame::of(&pilot, true, Controls::default());
        let mut from_frame = frame.pilot();
        from_frame.commands = q.commands.clone();
        assert_eq!(from_frame, q);
        // The commands decode as they were rounded.
        for command in &pilot.commands {
            let mut w = BitWriter::new();
            write_command(&mut w, &Command::Pilot(*command));
            let bytes = w.finish();
            let back = read_command(&mut BitReader::new(&bytes)).unwrap();
            assert_eq!(back, Command::Pilot(quantize_command(*command)));
        }
    }

    #[test]
    fn every_wing_order_reads_back_and_sort_is_code_13() {
        let mut orders = vec![
            PlayerOrder::EngageMyTarget,
            PlayerOrder::ProtectMe,
            PlayerOrder::AttackOnContact,
            PlayerOrder::EngageFromFormation,
            PlayerOrder::Disengage,
            PlayerOrder::Spacing,
            PlayerOrder::Stacking,
            PlayerOrder::ControlToggle,
            PlayerOrder::BugOut,
            PlayerOrder::LandAtSelected,
            PlayerOrder::Sort,
        ];
        orders.extend(BREAKS.map(PlayerOrder::Break));
        orders.extend(APPROACHES.map(PlayerOrder::Approach));
        orders.extend(FORMATIONS.map(PlayerOrder::Formation));
        for order in orders {
            let mut w = BitWriter::new();
            write_order(&mut w, order);
            let bytes = w.finish();
            assert_eq!(read_order(&mut BitReader::new(&bytes)).unwrap(), order);
        }
        let mut w = BitWriter::new();
        write_order(&mut w, PlayerOrder::Sort);
        assert_eq!(w.finish(), [13]);
    }

    #[test]
    fn the_battle_net_key_reads_back_and_is_command_23() {
        let command = Command::Seat(SeatCommand::BattleNet);
        let mut w = BitWriter::new();
        write_command(&mut w, &command);
        let bytes = w.finish();
        assert_eq!(bytes, [23], "five bits, no fields");
        assert_eq!(read_command(&mut BitReader::new(&bytes)).unwrap(), command);
    }

    #[test]
    fn every_code_reads_back_as_its_command() {
        for (index, command) in LIVE.iter().enumerate() {
            assert_eq!(live_code(*command), index as u64);
        }
        for (index, switch) in SWITCHES.iter().enumerate() {
            assert_eq!(switch_code(*switch), index as u64);
        }
    }
}
