//! The command phase of a tick: what each seat asked for between ticks,
//! applied to the plane the seat flies. See docs/ARCHITECTURE.md, "Seat
//! input".

use super::{Cue, SeatInput, TickOutput, World};
use crate::{
    WorldResult, ai_wings, combat, comms,
    seats::{PlaneId, SeatCommand, SeatId},
};
use tore_sim::{ai::wing::PlayerOrder, cheats::Cheats, combat::live::Command as Live};

/// A change to the mission itself, applied at the start of the tick before
/// any seat's commands. In single player the player gives them, from the
/// cheats menu; in a hosted game only the host's would count.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MissionCommand {
    /// New settings in force.
    Settings(Settings),
    /// A human takes the AI-flown `plane` from `seat`: joining, or rejoining
    /// the aircraft reserved for it ([`World::take_plane`]). The seat sends
    /// input for this tick, as it now flies a plane.
    Take { seat: SeatId, plane: PlaneId },
    /// A human gives its plane back to the AI: leaving, dropping or being
    /// kicked ([`World::give_back_plane`]). The seat sends no input for this
    /// tick.
    GiveBack { seat: SeatId },
}

/// What the player's order call does to the radio channel. Agent decision:
/// the driver sets it, so each keeps the behaviour it had before B2 gave
/// orders to the step. It is a stopgap until B4 gives each seat its own radio
/// delivery, and one place to remove when John decides the order call should
/// hold the channel for every seat whatever the audio.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum OrderCall {
    /// The call holds nothing: the AI probe, which never held the channel.
    #[default]
    Silent,
    /// The call holds the radio channel for its length.
    Spoken,
    /// As `Spoken`, and the host plays it at once, cutting off the wing lines
    /// the mixer was still playing, which the channel's journal and clock
    /// note. A live game with a sound device.
    Heard,
}

/// What became of a wing order the step applied.
#[derive(Clone, Debug, PartialEq)]
pub struct OrderReply {
    /// The seat that gave the order.
    pub seat: SeatId,
    pub order: PlayerOrder,
    pub outcome: OrderOutcome,
}

/// How a wing order ended. Each carries the line the pilot reads.
#[derive(Clone, Debug, PartialEq)]
pub enum OrderOutcome {
    /// The wing was told, and its report is `message`.
    Given { message: String },
    /// Turned away before the wing saw it: no usable landing site, or no AI
    /// wing to order.
    Refused { message: String },
    /// The wing could not take it.
    Failed { message: String },
}

/// The settings a mission runs under. What single player's flight menu
/// changes are the cheats, which include the enemy skill and guns only
/// switches the AI reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Settings {
    pub cheats: Cheats,
}

impl World {
    /// Puts a mission command into force: every human-flown plane, combat and
    /// the AI wings take the new cheats.
    ///
    /// A handoff the mission refuses (see [`World::can_take`] and
    /// [`World::can_give_back`]) is an error, which stops the tick before
    /// anything after it in the list changes: a host checks first.
    pub(super) fn apply_mission_command(&mut self, command: &MissionCommand) -> WorldResult<()> {
        match command {
            MissionCommand::Settings(Settings { cheats }) => {
                for cockpit in &mut self.cockpits {
                    cockpit.flight.cheats = *cheats;
                }
                self.combat.state.cheats = *cheats;
                if let Some(wings) = &mut self.ai_wings {
                    wings.set_enemy_skill(cheats.enemy_ai);
                    wings.set_guns_only(cheats.guns_only);
                }
            }
            MissionCommand::Take { seat, plane } => self.take_plane(*seat, *plane)?,
            MissionCommand::GiveBack { seat } => self.give_back_plane(*seat)?,
        }
        Ok(())
    }

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
        self.cockpits[cockpit].flight.sensors = input.sensors;
        let (plane, seat) = (self.cockpits[cockpit].plane, input.seat);
        for &command in &input.commands {
            match command {
                SeatCommand::CycleWeapon { forward } => {
                    self.cycle_cockpit_weapon(cockpit, forward);
                    out.cues.push(Cue::WeaponCycled { seat });
                }
                SeatCommand::Airport(command) => self.airport_command(cockpit, command, out),
                SeatCommand::Combat(command) => {
                    let launcher = combat::launcher(&self.cockpits[cockpit].flight);
                    self.combat.command_for(plane.0, command, launcher);
                }
                SeatCommand::Manual(command) => self.manual_command(cockpit, command, out),
                SeatCommand::RangeReset => self.range_reset(cockpit, out),
                SeatCommand::ReleaseChaff => self.release_countermeasure(cockpit, true, out),
                SeatCommand::ReleaseFlare => self.release_countermeasure(cockpit, false, out),
                SeatCommand::ReleaseTrigger => self.combat.cancel_for(plane.0),
                SeatCommand::WingRecipient(recipient) => {
                    self.roster.set_wing_recipient(seat, recipient);
                }
                SeatCommand::WingOrder(order) => self.wing_order(plane, seat, cockpit, order, out),
                SeatCommand::WingFormationCycle => {
                    let recipient = self.roster.seat(seat).and_then(|s| s.wing_recipient);
                    match &self.ai_wings {
                        Some(wings) => {
                            let order =
                                PlayerOrder::Formation(wings.next_formation(plane.0, recipient));
                            self.wing_order(plane, seat, cockpit, order, out);
                        }
                        None => out.cues.push(Cue::Message {
                            seat,
                            text: "Wing order unavailable: no AI wing".into(),
                        }),
                    }
                }
                SeatCommand::RadioSilence => {
                    let message = self.comms.toggle_silence(input.seat);
                    out.cues.push(Cue::Message {
                        seat,
                        text: message.into(),
                    });
                }
                SeatCommand::TriggerKey {
                    down,
                    repeat,
                    blocked,
                } => self
                    .combat
                    .trigger(plane.0)
                    .input
                    .space(down, repeat, blocked),
            }
        }
    }

    /// A key, button or menu combat command. The arming, seeker and
    /// designation commands always work; the rest are range and development
    /// commands that need `--live-fire`.
    fn manual_command(&mut self, cockpit: usize, command: Live, out: &mut TickOutput) {
        let seat = self.seat_of_cockpit(cockpit);
        if !self.combat.range
            && !matches!(
                command,
                Live::ToggleArm | Live::ClearDesignation | Live::ToggleSeekerMode
            )
        {
            out.cues.push(Cue::Message {
                seat,
                text: "Manual range command requires --live-fire".into(),
            });
            return;
        }
        let aircraft = self.cockpits[cockpit].plane.0;
        self.combat.cancel_for(aircraft);
        self.combat.command_for(
            aircraft,
            command,
            combat::launcher(&self.cockpits[cockpit].flight),
        );
        // Range commands can replace targets or launch a round now.
        if self.combat.range {
            self.combat
                .refresh_render(&self.cockpits[cockpit].flight, self.ai_wings.as_ref());
        }
        let flight = &mut self.cockpits[cockpit].flight;
        let payload = self
            .combat
            .state
            .ownship(aircraft)
            .map_or(0., tore_sim::combat::live::Ownship::payload_lbs);
        if let Err(error) =
            flight.set_payload((payload - flight.systems.used_external_lbs()).max(0.))
        {
            out.cues.push(Cue::Message {
                seat,
                text: error.to_string(),
            });
        }
    }

    /// Puts a new target on the range.
    fn range_reset(&mut self, cockpit: usize, out: &mut TickOutput) {
        if !self.combat.range {
            out.cues.push(Cue::Message {
                seat: self.seat_of_cockpit(cockpit),
                text: "Target reset is available only with --live-fire".into(),
            });
            return;
        }
        let aircraft = self.cockpits[cockpit].plane.0;
        self.combat.cancel_for(aircraft);
        self.combat.command_for(
            aircraft,
            Live::ReplaceTarget,
            combat::launcher(&self.cockpits[cockpit].flight),
        );
        self.combat
            .refresh_render(&self.cockpits[cockpit].flight, self.ai_wings.as_ref());
    }

    /// Releases one chaff cartridge or flare, and tells the pilot how many
    /// are left, with the retail cockpit messages (FA.EXE string table).
    fn release_countermeasure(&mut self, cockpit: usize, chaff: bool, out: &mut TickOutput) {
        let aircraft = self.cockpits[cockpit].plane.0;
        let flight = &self.cockpits[cockpit].flight;
        let launcher = combat::launcher(flight);
        let Some(own) = self.combat.state.ownship(aircraft) else {
            return;
        };
        if !launcher.alive || flight.escape.is_some() || own.hp <= 0 {
            return;
        }
        let count = |state: &tore_sim::combat::live::State| {
            state
                .ownship(aircraft)
                .map_or(0, |own| if chaff { own.chaff } else { own.flares })
        };
        let before = count(&self.combat.state);
        self.combat.command_for(
            aircraft,
            if chaff {
                Live::ReleaseChaff
            } else {
                Live::ReleaseFlare
            },
            launcher,
        );
        let after = count(&self.combat.state);
        out.cues.push(Cue::Message {
            seat: self.seat_of_cockpit(cockpit),
            text: match (chaff, before) {
                (true, 0) => "Out of chaff".to_string(),
                (false, 0) => "Out of flares".to_string(),
                (true, _) => format!("Chaff launched, {after} left"),
                (false, _) => format!("Flare launched, {after} left"),
            },
        });
    }

    /// An Alt-key order from the seat `seat`, flying `plane` from `cockpit`.
    /// It goes to the AI wings as that plane's order, to its own wing,
    /// addressed as the seat's recipient says, with the aircraft the seat has
    /// designated. Only a plane leading its wing may order it.
    fn wing_order(
        &mut self,
        plane: PlaneId,
        seat: SeatId,
        cockpit: usize,
        order: PlayerOrder,
        out: &mut TickOutput,
    ) {
        let now = self.combat.state.tick() as f64 / 120.;
        let recipient = self.roster.seat(seat).and_then(|s| s.wing_recipient);
        let selected = self
            .combat
            .state
            .view(self.cockpits[cockpit].plane.0)
            .and_then(|view| view.designated());
        // Land at selected airport uses the airport Shift-N selected for the
        // tower.
        let site = if order == PlayerOrder::LandAtSelected {
            match ai_wings::AiWings::landing_site(
                &self.terrain.airport_scene,
                &self.terrain.airfield_anchors,
                &self.cockpits[cockpit].airport_service,
            ) {
                Ok(site) => Some(site),
                Err(message) if self.ai_wings.is_some() => {
                    // Journal only: refused before the wing saw it.
                    self.comms.record(comms::journal::Entry::order_refused(
                        now,
                        order,
                        message.clone(),
                    ));
                    out.cues.push(Cue::Message {
                        seat,
                        text: message.clone(),
                    });
                    out.orders.push(OrderReply {
                        seat,
                        order,
                        outcome: OrderOutcome::Refused { message },
                    });
                    return;
                }
                Err(_) => None,
            }
        } else {
            None
        };
        let result = self
            .ai_wings
            .as_mut()
            .map(|wings| wings.command_at(plane.0, order, selected, recipient, site.as_ref()));
        let outcome = match result {
            Some(Ok(report)) => {
                // The order voice is played at once, and cuts off the wing
                // lines the mixer was still playing.
                if self.order_call == OrderCall::Heard {
                    self.comms
                        .cut_off(seat, now, comms::journal::Reason::OrderVoice);
                }
                if self.order_call != OrderCall::Silent && !report.radio.is_empty() {
                    self.comms.spoken(seat, now);
                }
                out.cues.push(Cue::OrderVoice {
                    seat,
                    stems: report.radio,
                });
                out.cues.push(Cue::Message {
                    seat,
                    text: report.message.clone(),
                });
                OrderOutcome::Given {
                    message: report.message,
                }
            }
            Some(Err(error)) => {
                let message = error.to_string();
                self.comms.record(comms::journal::Entry::order_refused(
                    now,
                    order,
                    message.clone(),
                ));
                out.cues.push(Cue::Message {
                    seat,
                    text: message.clone(),
                });
                OrderOutcome::Failed { message }
            }
            None => {
                let message = "Wing order unavailable: no AI wing".to_owned();
                self.comms.record(comms::journal::Entry::order_refused(
                    now,
                    order,
                    message.clone(),
                ));
                out.cues.push(Cue::Message {
                    seat,
                    text: message.clone(),
                });
                OrderOutcome::Refused { message }
            }
        };
        out.orders.push(OrderReply {
            seat,
            order,
            outcome,
        });
    }
}
