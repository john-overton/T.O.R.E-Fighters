//! One seat's input buffer on the host: the continuous controls by tick and
//! the numbered commands, taken one tick at a time (docs/ARCHITECTURE.md,
//! "The host session"; net-protocol.md, "Inputs").
//!
//! - The controls for tick T are used at T. A tick with none, because its
//!   input is late or lost, repeats the last stick, throttle, trigger and
//!   scope controls with no commands, and counts as repeated.
//! - A command is applied exactly once, in number order, at the tick the
//!   player's game applied it, or at the next tick run when that one has
//!   already been stepped. Commands repeat in every input packet until a
//!   snapshot acknowledges them, so a number already taken is a duplicate.
//! - Commands are numbered from 1; a snapshot's "commands applied" of 0 means
//!   none yet (agent decision: the client numbers its first command 1).

use crate::wire::inputs::{Command, InputFrame, InputsSection, ViewSubject};
use std::collections::{BTreeMap, VecDeque};
use tore_world::seats::{SeatId, SeatInput, SeatView};

/// Frames further ahead of the host than this (one second) are dropped: no
/// honest client runs that far ahead, and the buffer stays bounded.
pub const MAX_AHEAD_TICKS: u64 = 120;
/// Commands a seat may have waiting at most; more are a protocol error.
pub const MAX_PENDING_COMMANDS: usize = 256;

/// What taking one tick's input found, for the host's notes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Taken {
    /// No input for the tick: the last controls were repeated.
    pub repeated: bool,
    /// A command was applied at another tick than the player's game applied
    /// it.
    pub command_moved: bool,
    /// Commands applied this tick.
    pub commands: usize,
}

/// Why an Inputs section was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputError {
    /// More commands waiting than [`MAX_PENDING_COMMANDS`].
    TooManyCommands,
}

/// One seat's inputs, received and not yet stepped.
#[derive(Clone, Debug)]
pub struct InputBuffer {
    frames: BTreeMap<u64, (InputFrame, SeatView)>,
    /// The last controls taken and the view they came with.
    last: Option<(u64, InputFrame, SeatView)>,
    /// The newest input tick received.
    newest: Option<u64>,
    /// The next command number to take.
    next_command: u16,
    /// Commands taken in order and waiting for their tick.
    pending: VecDeque<(u64, Command)>,
    /// The number of the last command applied; 0 before any.
    applied: u16,
    /// The fewest ticks of margin over the inputs received since the last
    /// report.
    margin: Option<i64>,
    /// The margin last reported.
    reported_margin: i8,
    /// Ticks repeated since the last report, and in all.
    repeats: u32,
    repeats_total: u64,
    /// What the player's view follows.
    pub view_subject: Option<ViewSubject>,
    /// The newest tick the client reported an own-state mismatch for.
    pub mismatch: u32,
}

impl Default for InputBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBuffer {
    /// An empty buffer: the first command it takes is number 1.
    pub fn new() -> Self {
        Self {
            frames: BTreeMap::new(),
            last: None,
            newest: None,
            next_command: 1,
            pending: VecDeque::new(),
            applied: 0,
            margin: None,
            reported_margin: 0,
            repeats: 0,
            repeats_total: 0,
            view_subject: None,
            mismatch: 0,
        }
    }

    /// Takes an Inputs section that arrived while `next_tick` is the host's
    /// next tick to step.
    pub fn receive(&mut self, section: &InputsSection, next_tick: u64) -> Result<(), InputError> {
        let newest_view = section.view();
        let newest_tick = u64::from(section.newest_tick);
        for (index, frame) in section.frames.iter().enumerate() {
            let tick = u64::from(section.frame_tick(index));
            if self.newest.is_none_or(|newest| tick > newest) {
                let margin = tick as i64 - next_tick as i64;
                self.margin = Some(self.margin.map_or(margin, |m| m.min(margin)));
                self.newest = Some(tick);
            }
            if tick < next_tick || tick > next_tick + MAX_AHEAD_TICKS {
                continue;
            }
            // Each frame's view is the newest's, as many ticks earlier.
            let view = SeatView {
                tick: newest_view.tick.saturating_sub(newest_tick - tick),
                interpolation_delay: newest_view.interpolation_delay,
            };
            self.frames.entry(tick).or_insert((*frame, view));
        }
        self.view_subject = section.view_subject;
        if section.mismatch != 0 && section.mismatch > self.mismatch {
            self.mismatch = section.mismatch;
        }
        for command in &section.commands {
            if command.number != self.next_command {
                // Older numbers are duplicates; a gap cannot happen, since
                // the client repeats every command not yet acknowledged.
                continue;
            }
            if self.pending.len() >= MAX_PENDING_COMMANDS {
                return Err(InputError::TooManyCommands);
            }
            self.pending
                .push_back((u64::from(command.tick), command.command));
            self.next_command = self.next_command.wrapping_add(1);
        }
        Ok(())
    }

    /// The seat's input for `tick`, the host's next tick, and what taking it
    /// found. Frames older than `tick` are dropped.
    pub fn take(&mut self, seat: SeatId, tick: u64) -> (SeatInput, Taken) {
        let mut taken = Taken::default();
        while let Some((&oldest, _)) = self.frames.first_key_value() {
            if oldest >= tick {
                break;
            }
            self.frames.pop_first();
        }
        let (frame, view) = match self.frames.remove(&tick) {
            Some((frame, view)) => (frame, Some(view)),
            None => {
                taken.repeated = true;
                self.repeats = self.repeats.saturating_add(1);
                self.repeats_total += 1;
                match self.last {
                    // The view moves on with the repeated ticks.
                    Some((last_tick, frame, view)) => (
                        frame,
                        Some(SeatView {
                            tick: view.tick + tick.saturating_sub(last_tick),
                            ..view
                        }),
                    ),
                    None => (InputFrame::default(), None),
                }
            }
        };
        if let Some(view) = view {
            self.last = Some((tick, frame, view));
        }
        let mut commands = Vec::new();
        while let Some(&(at, command)) = self.pending.front() {
            // A command waits for its tick, unless it names one implausibly
            // far ahead, which is applied now rather than held.
            if at > tick && at <= tick + MAX_AHEAD_TICKS {
                break;
            }
            self.pending.pop_front();
            taken.command_moved |= at != tick;
            commands.push(command);
            self.applied = self.applied.wrapping_add(1);
        }
        taken.commands = commands.len();
        (frame.seat_input(seat, tick, &commands, view), taken)
    }

    /// The newest input tick received, or 0.
    pub fn newest(&self) -> u32 {
        self.newest.unwrap_or(0) as u32
    }

    /// The highest command number applied, every one before it applied too;
    /// 0 before any.
    pub fn applied(&self) -> u16 {
        self.applied
    }

    /// The snapshot header's input margin and inputs repeated since the last
    /// report, and starts the next report. With no input received since, the
    /// margin stays as last reported.
    pub fn report(&mut self) -> (i8, u8) {
        if let Some(margin) = self.margin.take() {
            self.reported_margin = margin.clamp(i64::from(i8::MIN), i64::from(i8::MAX)) as i8;
        }
        let repeats = self.repeats.min(u32::from(u8::MAX)) as u8;
        self.repeats = 0;
        (self.reported_margin, repeats)
    }

    /// The margin last reported.
    pub fn margin(&self) -> i8 {
        self.reported_margin
    }

    /// Ticks repeated since the seat joined.
    pub fn repeats_total(&self) -> u64 {
        self.repeats_total
    }

    /// Commands taken and not yet applied.
    pub fn pending_commands(&self) -> usize {
        self.pending.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::inputs::NumberedCommand;
    use tore_sim::flight::{PilotCommand, Switch};

    const SEAT: SeatId = SeatId(3);

    fn frame(pitch: i16) -> InputFrame {
        InputFrame {
            pitch,
            ..InputFrame::default()
        }
    }

    /// A section of `frames` ending at `newest`, with `commands` numbered
    /// from `first`, each applied at its tick.
    fn section(newest: u32, frames: &[i16], first: u16, commands: &[u32]) -> InputsSection {
        InputsSection {
            flight: 0,
            newest_tick: newest,
            frames: frames.iter().map(|&p| frame(p)).collect(),
            view_offset: 10,
            interpolation_delay: 12,
            view_subject: None,
            mismatch: 0,
            commands: commands
                .iter()
                .enumerate()
                .map(|(i, &tick)| NumberedCommand {
                    number: first.wrapping_add(i as u16),
                    tick,
                    command: Command::Pilot(PilotCommand::Toggle(Switch::Gear)),
                })
                .collect(),
        }
    }

    fn pitch(input: &SeatInput) -> f64 {
        input.pilot.pitch
    }

    #[test]
    fn controls_for_a_tick_are_used_at_that_tick() {
        let mut buffer = InputBuffer::new();
        buffer
            .receive(&section(102, &[100, 101, 102], 1, &[]), 100)
            .unwrap();
        for (tick, want) in [(100, 100), (101, 101), (102, 102)] {
            let (input, taken) = buffer.take(SEAT, tick);
            assert_eq!(input.seat, SEAT);
            assert_eq!(input.tick, tick);
            assert_eq!(pitch(&input), frame(want).pilot().pitch);
            assert!(!taken.repeated);
            // The view is the newest's, as many ticks earlier.
            assert_eq!(
                input.view,
                Some(SeatView {
                    tick: tick - 10,
                    interpolation_delay: 12
                })
            );
        }
        assert_eq!(buffer.newest(), 102);
        // Every frame arrived early: the fewest ticks ahead was 0 (tick 100
        // was needed next).
        assert_eq!(buffer.report(), (0, 0));
    }

    #[test]
    fn a_missing_tick_repeats_the_last_controls_with_no_commands() {
        let mut buffer = InputBuffer::new();
        buffer.receive(&section(100, &[7], 1, &[]), 100).unwrap();
        let (_, taken) = buffer.take(SEAT, 100);
        assert!(!taken.repeated);
        let (input, taken) = buffer.take(SEAT, 101);
        assert!(taken.repeated);
        assert_eq!(pitch(&input), frame(7).pilot().pitch);
        assert!(input.commands.is_empty() && input.pilot.commands.is_empty());
        // The view moves on with the repeated tick.
        assert_eq!(input.view.unwrap().tick, 91);
        let (_, taken) = buffer.take(SEAT, 102);
        assert!(taken.repeated);
        assert_eq!(buffer.report().1, 2);
        assert_eq!(buffer.report().1, 0, "the count restarts at each report");
        assert_eq!(buffer.repeats_total(), 2);
    }

    #[test]
    fn a_seat_with_no_input_yet_flies_neutral() {
        let mut buffer = InputBuffer::new();
        let (input, taken) = buffer.take(SEAT, 5);
        assert!(taken.repeated);
        assert_eq!(pitch(&input), 0.);
        assert_eq!(input.view, None);
    }

    #[test]
    fn late_frames_are_dropped_and_counted_as_late() {
        let mut buffer = InputBuffer::new();
        // The host already stepped 100 to 104: frames for them are late.
        buffer
            .receive(&section(104, &[100, 101, 102, 103, 104], 1, &[]), 105)
            .unwrap();
        let (input, taken) = buffer.take(SEAT, 105);
        assert!(taken.repeated, "nothing for 105");
        assert_eq!(pitch(&input), 0.);
        // The worst of them, tick 100, arrived five ticks after it was
        // needed.
        assert_eq!(buffer.report().0, -5);
    }

    #[test]
    fn early_frames_wait_for_their_tick_and_far_ones_are_dropped() {
        let mut buffer = InputBuffer::new();
        buffer.receive(&section(110, &[110], 1, &[]), 100).unwrap();
        buffer
            .receive(&section(100 + 121 + 5, &[9], 1, &[]), 100)
            .unwrap();
        assert_eq!(buffer.report().0, 10);
        for tick in 100..110 {
            assert!(buffer.take(SEAT, tick).1.repeated);
        }
        let (input, taken) = buffer.take(SEAT, 110);
        assert!(!taken.repeated);
        assert_eq!(pitch(&input), frame(110).pilot().pitch);
    }

    #[test]
    fn duplicated_frames_keep_the_first_copy() {
        let mut buffer = InputBuffer::new();
        buffer.receive(&section(101, &[1, 2], 1, &[]), 100).unwrap();
        // The same ticks again, as a repeat carries them (and a forged change
        // is ignored).
        buffer
            .receive(&section(102, &[5, 6, 3], 1, &[]), 100)
            .unwrap();
        let got: Vec<f64> = (100..103).map(|t| pitch(&buffer.take(SEAT, t).0)).collect();
        assert_eq!(got, [1, 2, 3].map(|p| frame(p).pilot().pitch).to_vec());
    }

    #[test]
    fn commands_apply_once_in_number_order_at_their_tick() {
        let mut buffer = InputBuffer::new();
        // Commands 1 and 2 at tick 102, 3 at 104; the packet comes twice.
        let packet = section(104, &[0, 0, 0, 0, 0], 1, &[102, 102, 104]);
        buffer.receive(&packet, 100).unwrap();
        buffer.receive(&packet, 100).unwrap();
        let mut applied = Vec::new();
        for tick in 100..106 {
            let (input, taken) = buffer.take(SEAT, tick);
            assert!(!taken.command_moved);
            applied.push(input.pilot.commands.len());
        }
        assert_eq!(applied, [0, 0, 2, 0, 1, 0]);
        assert_eq!(buffer.applied(), 3);
        // The same commands repeated after they were applied are duplicates:
        // a toggle never toggles twice.
        buffer
            .receive(&section(106, &[0], 1, &[102, 102, 104]), 106)
            .unwrap();
        let (input, _) = buffer.take(SEAT, 106);
        assert!(input.pilot.commands.is_empty());
        assert_eq!(buffer.applied(), 3);
    }

    #[test]
    fn a_late_command_applies_at_the_next_tick_run() {
        let mut buffer = InputBuffer::new();
        buffer.take(SEAT, 100);
        buffer.take(SEAT, 101);
        // Command 1 was for tick 100, which the host already stepped.
        buffer.receive(&section(102, &[0], 1, &[100]), 102).unwrap();
        let (input, taken) = buffer.take(SEAT, 102);
        assert_eq!(input.pilot.commands.len(), 1);
        assert!(taken.command_moved);
        assert_eq!(buffer.applied(), 1);
    }

    #[test]
    fn a_command_whose_packet_came_after_a_later_one_keeps_its_order() {
        let mut buffer = InputBuffer::new();
        // A packet with commands 2 and 3 only cannot be taken before 1.
        buffer
            .receive(&section(100, &[0], 2, &[100, 100]), 100)
            .unwrap();
        assert_eq!(buffer.pending_commands(), 0);
        buffer
            .receive(&section(100, &[0], 1, &[100, 100, 100]), 100)
            .unwrap();
        let (input, _) = buffer.take(SEAT, 100);
        assert_eq!(input.pilot.commands.len(), 3);
        assert_eq!(buffer.applied(), 3);
    }

    #[test]
    fn seat_commands_go_to_the_seat_list_in_order() {
        use tore_world::seats::SeatCommand;
        let mut buffer = InputBuffer::new();
        let mut packet = section(100, &[0], 1, &[100, 100, 100]);
        packet.commands[0].command = Command::Seat(SeatCommand::ReleaseFlare);
        packet.commands[2].command = Command::Seat(SeatCommand::ReleaseChaff);
        buffer.receive(&packet, 100).unwrap();
        let (input, taken) = buffer.take(SEAT, 100);
        assert_eq!(taken.commands, 3);
        assert_eq!(
            input.commands,
            [SeatCommand::ReleaseFlare, SeatCommand::ReleaseChaff]
        );
        assert_eq!(input.pilot.commands.len(), 1);
    }

    #[test]
    fn too_many_waiting_commands_are_refused() {
        let mut buffer = InputBuffer::new();
        let mut number = 1u16;
        let mut refused = false;
        for _ in 0..8 {
            let ticks = [150u32; 64];
            match buffer.receive(&section(100, &[0], number, &ticks), 100) {
                Ok(()) => number = number.wrapping_add(64),
                Err(InputError::TooManyCommands) => refused = true,
            }
        }
        assert!(refused);
    }
}
