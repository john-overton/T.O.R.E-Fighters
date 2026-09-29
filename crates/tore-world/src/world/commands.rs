//! The command phase of a tick: what each seat asked for between ticks,
//! applied to the plane the seat flies. See docs/ARCHITECTURE.md, "Seat
//! input".

use super::{Cue, SeatInput, TickOutput, World};
use crate::{combat, seats::SeatCommand};
use tore_sim::combat::live::Command as Live;

impl World {
    /// Applies one seat's commands, in the order given, to the plane whose
    /// cockpit is `cockpit`. Each does what the old between-frames handler did
    /// at the same moment: window events arrive between frames, so the state
    /// they saw is the state the tick starts from.
    pub(super) fn apply_seat_commands(
        &mut self,
        cockpit: usize,
        input: &SeatInput,
        out: &mut TickOutput,
    ) {
        for &command in &input.commands {
            match command {
                SeatCommand::CycleWeapon { forward } => {
                    self.cycle_cockpit_weapon(cockpit, forward);
                    out.cues.push(Cue::WeaponCycled);
                }
                SeatCommand::Airport(command) => self.airport_command(cockpit, command, out),
                SeatCommand::Combat(command) => {
                    let launcher = combat::launcher(&self.cockpits[cockpit].flight);
                    self.combat.command(command, launcher);
                }
                SeatCommand::Manual(command) => self.manual_command(cockpit, command, out),
                SeatCommand::RangeReset => self.range_reset(cockpit, out),
                SeatCommand::ReleaseChaff => self.release_countermeasure(cockpit, true, out),
                SeatCommand::ReleaseFlare => self.release_countermeasure(cockpit, false, out),
                SeatCommand::ReleaseTrigger => self.combat.cancel(),
                SeatCommand::TriggerKey {
                    down,
                    repeat,
                    blocked,
                } => self.combat.input.space(down, repeat, blocked),
            }
        }
    }

    /// A key, button or menu combat command. The arming, seeker and
    /// designation commands always work; the rest are range and development
    /// commands that need `--live-fire`.
    fn manual_command(&mut self, cockpit: usize, command: Live, out: &mut TickOutput) {
        if !self.combat.range
            && !matches!(
                command,
                Live::ToggleArm | Live::ClearDesignation | Live::ToggleSeekerMode
            )
        {
            out.cues.push(Cue::Message(
                "Manual range command requires --live-fire".into(),
            ));
            return;
        }
        self.combat.cancel();
        self.combat
            .command(command, combat::launcher(&self.cockpits[cockpit].flight));
        // Range commands can replace targets or launch a round now.
        if self.combat.range {
            self.combat
                .refresh_render(&self.cockpits[cockpit].flight, self.ai_wings.as_ref());
        }
        let flight = &mut self.cockpits[cockpit].flight;
        if let Err(error) = flight.set_payload(
            (self.combat.state.payload_lbs() - flight.systems.used_external_lbs()).max(0.),
        ) {
            out.cues.push(Cue::Message(error.to_string()));
        }
    }

    /// Puts a new target on the range.
    fn range_reset(&mut self, cockpit: usize, out: &mut TickOutput) {
        if !self.combat.range {
            out.cues.push(Cue::Message(
                "Target reset is available only with --live-fire".into(),
            ));
            return;
        }
        self.combat.cancel();
        self.combat.command(
            Live::ReplaceTarget,
            combat::launcher(&self.cockpits[cockpit].flight),
        );
        self.combat
            .refresh_render(&self.cockpits[cockpit].flight, self.ai_wings.as_ref());
    }

    /// Releases one chaff cartridge or flare, and tells the pilot how many
    /// are left, with the retail cockpit messages (FA.EXE string table).
    fn release_countermeasure(&mut self, cockpit: usize, chaff: bool, out: &mut TickOutput) {
        let flight = &self.cockpits[cockpit].flight;
        let launcher = combat::launcher(flight);
        if !launcher.alive || flight.escape.is_some() || self.combat.state.player_hp <= 0 {
            return;
        }
        let count = |state: &tore_sim::combat::live::State| {
            if chaff { state.chaff } else { state.flares }
        };
        let before = count(&self.combat.state);
        self.combat.command(
            if chaff {
                Live::ReleaseChaff
            } else {
                Live::ReleaseFlare
            },
            launcher,
        );
        let after = count(&self.combat.state);
        out.cues.push(Cue::Message(match (chaff, before) {
            (true, 0) => "Out of chaff".to_string(),
            (false, 0) => "Out of flares".to_string(),
            (true, _) => format!("Chaff launched, {after} left"),
            (false, _) => format!("Flare launched, {after} left"),
        }));
    }
}
