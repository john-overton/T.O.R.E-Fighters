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
#[derive(Clone, Debug, PartialEq)]
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
    /// Stage F phase 2: frees `seat` from its lost plane, whose pilot
    /// becomes [`crate::seats::Pilot::Lost`] (docs/ARCHITECTURE.md, "Death,
    /// revival and lives"; [`World::abandon_plane`]). The seat sends no input
    /// for this tick.
    Abandon { seat: SeatId },
    /// Stage F phase 2: abandons `seat`'s lost plane and seats it in a new
    /// plane of the same aircraft at `spawn` ([`World::revive_plane`]). The
    /// seat sends input for this tick.
    Revive {
        seat: SeatId,
        spawn: Box<super::revive::Spawn>,
    },
    /// Stage K (slice K5): seats `seat`, which flies no plane, in a new
    /// plane of the lost `plane`'s aircraft and wing at `spawn`
    /// ([`World::revive_lost_plane`]): a player who returns to a game whose
    /// AI lost the aircraft reserved for it. The lost plane stays as it is.
    /// The seat sends input for this tick.
    ReviveLost {
        seat: SeatId,
        plane: PlaneId,
        spawn: Box<super::revive::Spawn>,
    },
    /// The lobby pass's slice R1: the AI respawns the lineage rooted at
    /// `root` (a plane the mission started with), whose newest plane is
    /// lost and held by no human, in a new AI plane of its wing at `spawn`
    /// ([`World::respawn_plane`]). No seat flies it.
    Respawn {
        root: PlaneId,
        spawn: Box<super::revive::Spawn>,
    },
    /// The lobby pass's slice R2: the lead hold on or off
    /// ([`super::lead_hold`]). The host turns it on for a game whose
    /// `respawn` rule is not `none`; single player never does. Off clears
    /// every owner.
    LeadHold { on: bool },
    /// The lobby pass's slice R2: the human `owner` of a flight's lead has
    /// left the game (left, kicked, or no longer kept by the host): each
    /// wing it owns passes to the next human in the flight, else back to the
    /// AI's own succession ([`super::lead_hold`]).
    LeadLeft { owner: super::lead_hold::LeadOwner },
    /// The lobby pass's follow-up F1: the player in `seat` is `callsign`
    /// ([`crate::seats::Roster::set_callsign`]). The host names a seat's
    /// player before it seats it, so the mission's HUD lines can name
    /// players ("You lead the flight until Viper flies again."); a standby
    /// replaying the journal knows the same names.
    Callsign { seat: SeatId, callsign: String },
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

/// The manual combat commands a player has in every flight. Everything else a
/// key can send (class cycling, station faults, damage, incoming fixtures,
/// target ECM) is a range or development command.
///
/// Jettison (Shift+K) is gameplay too: it drops the selected external stores in
/// a Quick Mission, a campaign mission and a multiplayer flight, not only on
/// the `--live-fire` range (John, 2026-10-10). It only ever empties a
/// non-internal station, so an aircraft with nothing external, the AC-130
/// among them, is left as it was.
///
/// The gun-group commands are gameplay: an AC-130 crew links its guns in a
/// Quick Mission, a campaign mission and a multiplayer flight, not only on the
/// `--live-fire` range (John, 2026-10-09).
pub(crate) fn works_outside_range(command: Live) -> bool {
    matches!(
        command,
        Live::ToggleArm
            | Live::ClearDesignation
            | Live::ToggleSeekerMode
            | Live::NextGunGroup
            | Live::ToggleGunGroup
            | Live::Jettison
    )
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
            MissionCommand::Take { seat, plane } => {
                self.take_plane(*seat, *plane)?;
                // A player back from away retakes the lead it owns (R2).
                self.lead_taken(*seat, *plane);
            }
            MissionCommand::GiveBack { seat } => {
                let plane = self.roster.seat(*seat).and_then(|s| s.plane);
                self.give_back_plane(*seat)?;
                // The plane keeps the lead it owns for its player (R2).
                if let Some(plane) = plane {
                    self.lead_given_back(*seat, plane);
                }
            }
            // Stage F phase 2's revival (slice F2-V; world/revive.rs).
            MissionCommand::Abandon { seat } => {
                self.abandon_plane(*seat)?;
            }
            MissionCommand::Revive { seat, spawn } => {
                self.revive_plane(*seat, spawn)?;
            }
            MissionCommand::ReviveLost { seat, plane, spawn } => {
                self.revive_lost_plane(*seat, *plane, spawn)?;
                // A player back from away, whose plane the AI lost, owns its
                // flight's lead again from its new plane (R2).
                self.lead_taken(*seat, *plane);
            }
            // The lobby pass's AI respawn (slice R1; world/revive.rs).
            MissionCommand::Respawn { root, spawn } => {
                self.respawn_plane(*root, spawn)?;
            }
            // The lobby pass's lead hold (slice R2; world/lead_hold.rs).
            MissionCommand::LeadHold { on } => self.set_lead_hold(*on),
            MissionCommand::LeadLeft { owner } => self.lead_left(*owner),
            MissionCommand::Callsign { seat, callsign } => {
                self.roster.set_callsign(*seat, callsign.clone());
            }
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
                // The call to the flight (slice F2-R; world/replies.rs).
                SeatCommand::WingReply(reply) => {
                    self.wing_reply(plane, seat, cockpit, reply, out);
                }
                SeatCommand::BattleNet => {
                    let message = self.comms.toggle_battle(seat);
                    out.cues.push(Cue::Message {
                        seat,
                        text: message.into(),
                    });
                }
            }
        }
    }

    /// A key, button or menu combat command. The arming, seeker, designation,
    /// gun-group and jettison commands always work; the rest are range and
    /// development commands that need `--live-fire`.
    fn manual_command(&mut self, cockpit: usize, command: Live, out: &mut TickOutput) {
        let seat = self.seat_of_cockpit(cockpit);
        if !self.combat.range && !works_outside_range(command) {
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
            self.refresh_picture();
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
        self.refresh_picture();
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

    /// Repeats a flight lead's assignment call over the battle net (slice G8):
    /// every living seat of the lead's side, in another flight, that monitors
    /// it hears `words` with the lead's flight colour in front, under the label
    /// `Net Blue one`. The wing net's own hearing of the call is the caller's
    /// (the order voice, or the radio). The call is important, so radio
    /// silence never drops it. Nothing happens, and nothing is journaled, while
    /// no seat monitors the net, so single player is unchanged.
    pub(super) fn battle_net_call(
        &mut self,
        plane: PlaneId,
        words: comms::Phrase,
        delay: f64,
        cause: comms::journal::Cause,
    ) {
        if !self
            .comms
            .seats()
            .any(|seat| self.comms.monitors_battle(seat))
        {
            return;
        }
        let now = self.combat.state.tick() as f64 / 120.;
        let members = crate::radio_calls::members(&self.roster, self.ai_wings.as_ref(), |plane| {
            self.cockpits
                .iter()
                .position(|cockpit| cockpit.plane == plane)
                .is_some_and(|cockpit| self.cockpit_alive(cockpit))
        });
        let listeners: Vec<crate::radio_calls::Listener> = self
            .cockpits
            .iter()
            .enumerate()
            .filter_map(|(index, cockpit)| {
                let seat = self.roster.seat(self.roster.seat_of(cockpit.plane)?)?;
                let member = members.iter().find(|m| m.id == cockpit.plane.0)?;
                Some(crate::radio_calls::Listener {
                    seat: seat.id,
                    plane: cockpit.plane.0,
                    flight: member.flight,
                    enemy: member.enemy,
                    alive: self.cockpit_alive(index),
                    position: cockpit.flight.position,
                    crew: seat.crew,
                })
            })
            .collect();
        let leaders = crate::radio_calls::leaders(&self.roster, &members, self.ai_wings.as_ref());
        let hearers = crate::radio_calls::battle_hearers(
            &self.comms,
            &members,
            &leaders,
            &listeners,
            plane.0,
            &|_| words.clone(),
        );
        let Some(first) = hearers.first() else {
            return;
        };
        let label = first.label.clone().unwrap_or_default();
        let call = crate::datalink::calls::assignment_call(label, words)
            .after(delay)
            .because(comms::journal::Origin::of(comms::journal::Source::Order, cause).by(plane.0));
        self.comms.send(now, call, &hearers);
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
        if order == PlayerOrder::Sort {
            return self.sort_order(plane, seat, cockpit, out);
        }
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
                self.roster.redfor(self.cockpits[cockpit].plane),
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
        // The attack call names the flight by its radio colour.
        let flight = crate::radio_calls::members(&self.roster, self.ai_wings.as_ref(), |_| true)
            .iter()
            .find(|member| member.id == plane.0)
            .map(|member| member.flight);
        // A wingman that cannot see the target itself still takes an Engage
        // order when a flightmate's track of it is in the picture (G3b).
        let datalink = &self.datalink;
        let result = self.ai_wings.as_mut().map(|wings| {
            wings.command_linked(
                plane.0,
                order,
                selected,
                recipient,
                site.as_ref(),
                flight,
                &|target| datalink.tracked(plane.0, target),
            )
        });
        let outcome = match result {
            Some(Ok(mut report)) => {
                // An attack order that only human wingmen took has no AI
                // wingman's place to be worded from: the lead says it as the
                // first human wingman hears it (slice F2-R).
                if report.radio.is_empty() {
                    report.radio = self.human_attack_stems(plane, order, recipient, &report);
                }
                // The lead's order becomes assignments: written for the
                // members it reached, or cleared for them (slice G3a).
                self.datalink.assign(
                    self.combat.state.tick(),
                    plane.0,
                    order,
                    &report.reached,
                    report.target,
                );
                // The order voice is played at once, cuts off the wing lines
                // still playing and holds the seat's radio channel for its
                // length, whether or not a sound device plays it (John,
                // 2026-09-29).
                self.comms
                    .cut_off(seat, now, comms::journal::Reason::OrderVoice);
                if !report.radio.is_empty() {
                    self.comms.spoken(seat, now);
                }
                // A lead's assignment call is also on the battle net, for the
                // seats of other flights that monitor it (slice G8).
                if report.target.is_some() && !report.radio.is_empty() {
                    let words = report
                        .radio
                        .iter()
                        .fold(comms::Phrase::default(), |words, stem| {
                            words.then(&self.phrases, stem)
                        });
                    let cause = comms::journal::Cause::Order {
                        order,
                        selected: report.target,
                        target: report.target,
                    };
                    self.battle_net_call(plane, words, 0., cause);
                }
                // Each human wingman the order addressed hears it as a radio
                // call on its own channel (slice F2-R).
                self.call_human_wingmen(plane, order, recipient, &report);
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

// The sort order (stage G, slice G3c).
#[path = "commands_sort.rs"]
mod sort;

// Exact coding for the host's journal (stage K, slice K0).
#[path = "commands_checkpoint.rs"]
mod checkpoint;
