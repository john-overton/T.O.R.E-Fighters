//! The mission core: every piece of mutable mission state in one place, so a
//! single type can advance the whole mission one fixed 120 Hz tick at a time.
//! Stage A of the multiplayer plan; see docs/ARCHITECTURE.md, "Mission core
//! and seats".
//!
//! [`World::step`] runs the simulation half of a tick in the order the redraw
//! loop used to run it. Everything the loop did for the screen, the speakers,
//! the controllers and the replay recorder comes back as [`TickOutput`], in the
//! same order, for the app to present after the step.

use crate::{
    WorldResult, ai_wings, aircraft_type, airfield_radio, combat, combat_tape, comms, crew_voice,
    mission_layout, radio_calls,
    seats::{PlaneId, Roster, SeatId, SeatInput, Slot},
    terrain,
};
use std::collections::BTreeMap;
use tore_sim::models::FlightModel;
use tore_sim::{attitude, flight};

#[cfg(test)]
mod command_tests;
mod commands;
pub use commands::{MissionCommand, OrderCall, OrderOutcome, OrderReply, Settings};
#[cfg(test)]
mod tick_tests;

/// The whole mission. The app drives it and presents it.
pub struct World {
    /// What the flight is built from; a restart rebuilds it from this.
    pub setup: Setup,
    /// Terrain queries, the airport scene and the weather clock. What the
    /// renderer draws of it is the app's `Scenery`.
    pub terrain: terrain::Terrain,
    /// Every plane with its pilot, and every seat.
    pub roster: Roster,
    /// What each human-flown plane keeps outside combat, in plane id order.
    /// Single player has one, seat 0's plane 0, and the app presents it.
    pub cockpits: Vec<Cockpit>,
    pub combat: combat::Combat,
    pub ai_wings: Option<ai_wings::AiWings>,
    /// Radio and crew voice delivery; see docs/spec/radio-chatter.md.
    pub comms: comms::Comms,
    /// What the AI wingmen report from the airfield, decided once for every
    /// seat whose wing it is.
    pub wing_status: airfield_radio::WingStatus,
    /// Weapon, hit, kill and wing radio calls; see radio_calls.rs.
    pub radio: radio_calls::Radio,
    /// Imported phrase text for composing radio lines.
    pub phrases: comms::Phrases,
    /// What the player's order call does to the radio channel; the driver
    /// decides.
    pub order_call: OrderCall,
}

/// A human-flown plane's state outside combat: its flight, where the tick
/// started it, its turbulence, its conversation with the tower and its crew's
/// voice. An AI-flown plane keeps the same things in its AI actor.
pub struct Cockpit {
    pub plane: PlaneId,
    pub flight: flight::State,
    /// The flight state at the start of the last tick.
    pub previous_flight: flight::State,
    pub turbulence: tore_sim::turbulence::Turbulence,
    pub turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng,
    pub airport_service: tore_sim::airport::Service,
    /// NAV is selected instead of a weapon, which the tower reads.
    pub airport_nav_mode: bool,
    /// The plane's side of the tower conversation, spoken to its seat.
    pub airfield_radio: airfield_radio::AirfieldRadio,
    /// The plane's crew voice, built from its aircraft type and spoken to its
    /// seat; see docs/spec/cockpit-voice.md.
    pub crew_voice: crew_voice::CrewVoice,
    /// The plane's mission result and home checks, which send its seat the
    /// "mission accomplished" and "almost home" calls.
    pub result: ai_wings::outcome::Tracker,
    /// Simulation second of the last OVERSPEED message, so it repeats at an interval.
    pub overspeed_message_at: Option<f64>,
    /// Simulation second of the last turn-back warning past the map edge.
    pub edge_message_at: Option<f64>,
}

/// The mission a flight is built from. The creator fills it when the player
/// presses Fly, and a restart rebuilds the flight from it unchanged.
#[derive(Clone, Default)]
pub struct Setup {
    /// Launch altitude and fuel of an accepted Quick Mission; `None` flies free.
    pub mission: Option<(f64, f64)>,
    /// The accepted runway start.
    pub ground_start: Option<u32>,
    /// The player flies the hybrid model, the default; `--legacy-flight` turns
    /// it off.
    pub researched_flight: bool,
    /// The restricted native research adapter's tables.
    pub native_tables: Option<std::sync::Arc<tore_sim::native::Tables>>,
    /// The AI wings, when the AI flies the Quick Mission.
    pub ai: Option<AiSetup>,
}

/// The Quick Mission's AI wings and their standing orders.
#[derive(Clone)]
pub struct AiSetup {
    pub wings: Vec<tore_sim::ai::launch::WingLaunch>,
    pub guns_only: bool,
    pub preset: ai_wings::Preset,
    pub group_objectives: [tore_sim::ai::engagement::GroupObjective; 6],
    pub group_must_survive: [bool; 6],
}

/// What a restart tells the app about the new flight.
pub struct Restarted {
    /// The airport of a ground start.
    pub ground_airport: Option<u32>,
    /// The Quick Mission layout the player was placed from.
    pub layout: Option<mission_layout::MissionLayout>,
    /// How many AI aircraft fly, when the AI flies the mission.
    pub ai_aircraft: Option<usize>,
}

/// A queued airport command: the navigation mode switch or a tower request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AirportInput {
    NavMode,
    Command(tore_sim::airport::Command),
}

/// Something the tick did for the screen, the speakers or the controllers, in
/// the order it happened. The two markers say where presentation that reads
/// the simulation used to run inside the tick.
#[derive(Clone, Debug)]
pub enum Cue {
    /// A line for the HUD message area.
    Message(String),
    /// Controller feedback.
    Feedback(tore_input::FeedbackEvent),
    /// Tower speech for a reply, or `None` to cut the tower off.
    Tower(Option<&'static str>),
    /// The weapon selection moved from the weapon page.
    WeaponCycled,
    /// The player's flight, the weather clock and turbulence have stepped:
    /// camera weather, the view rig, blackout, vapor and control sounds ran
    /// here.
    Flown,
    /// Combat has stepped: the view change on the pilot's death ran here.
    CombatStepped,
    /// An AI pilot ejected.
    WingEjection {
        id: u32,
        message: String,
        friendly: bool,
    },
    /// The tick's picture was taken: the replay recorder reads the tick here.
    Picture,
    /// A radio or crew line due now for a seat.
    Radio { seat: SeatId, call: comms::Call },
    /// The player's own order call, played at once and cutting off what is
    /// playing, though it may hold no stems.
    OrderVoice(Vec<&'static str>),
}

/// What one tick produced. Every queue inside the world that the tick fills is
/// drained into it, whether or not anyone reads it.
#[derive(Default)]
pub struct TickOutput {
    pub cues: Vec<Cue>,
    /// How many of the first `cues` the command phase produced. The app runs
    /// what belongs between the commands and the rest of the tick, such as
    /// the mission recording's notes of them, at this point.
    pub commanded: usize,
    /// What became of each wing order given this tick.
    pub orders: Vec<OrderReply>,
    /// Combat's events for the tick.
    pub events: Vec<tore_sim::combat::live::Event>,
    /// The player's weapon release sounds: the sound's name and the station.
    pub releases: Vec<(String, usize)>,
    /// Shot outcomes the ledger resolved during the tick.
    pub outcomes: Vec<tore_sim::combat::ledger::Outcome>,
    /// The AI message journal of the tick (`None` without AI).
    pub journal: Option<tore_sim::ai::thought::JournalBatch>,
    /// The tick's sound emissions.
    pub emissions: Vec<tore_sim::acoustics::Emission>,
    /// A native research fault stopped the tick after the player's flight.
    pub fault: Option<String>,
}

impl World {
    /// Restarts the weather clock and every cockpit's turbulence from the
    /// launch conditions and their fixed seeds. The camera weather is the
    /// scenery's (`Scenery::reset_presentations`); the app restarts it beside
    /// this.
    pub fn reset_weather(&mut self) {
        self.terrain.weather =
            tore_sim::environment::Environment::new(self.terrain.weather.configuration().clone());
        for cockpit in &mut self.cockpits {
            cockpit.turbulence = Default::default();
            cockpit.turbulence_rng.reseed_word(1);
        }
    }

    /// Rebuilds the flight from its setup: the radio, the weather, the
    /// player's start, combat, the airport service and the AI wings, in the
    /// order a flight has always started. Single player's seat 0 flies plane 0
    /// from the first cockpit. The app ends the old flight's recording before
    /// and starts the new one after.
    pub fn restart(
        &mut self,
        aircraft: &aircraft_type::AircraftType,
        resources: &BTreeMap<String, Vec<u8>>,
    ) -> WorldResult<Restarted> {
        // A fixed seed keeps headless runs deterministic.
        self.comms.restart(1);
        self.wing_status.reset();
        self.radio = Default::default();
        self.cockpits.truncate(1);
        self.reset_weather();
        let Some(cockpit) = self.cockpits.first_mut() else {
            return Err("a flight needs a cockpit to restart".into());
        };
        cockpit.plane = PlaneId(0);
        cockpit.crew_voice =
            crew_voice::CrewVoice::new(&aircraft.profile).for_seat(SeatId::default(), 0);
        cockpit.flight = aircraft.start(&self.terrain);
        if let Some((altitude, fuel)) = self.setup.mission {
            cockpit.flight.position[1] = altitude;
            cockpit.flight.fuel = fuel;
        }
        // The accepted creator layout, reused unchanged on restart.
        let layout = self
            .setup
            .mission
            .and(self.combat.mission_layout.clone())
            .filter(|layout| layout.ground.is_some() == self.setup.ground_start.is_some());
        let parked = layout.as_ref().and_then(|layout| layout.ground.clone());
        cockpit
            .airfield_radio
            .reset(parked.as_ref().map(|g| g.runway));
        if let Some(layout) = layout.as_ref().filter(|l| l.player_turn != 0.) {
            // Airborne: the whole scene turns so the enemy ahead stays on the
            // map.
            cockpit.flight.yaw += layout.player_turn;
            let basis = attitude::Basis::new(cockpit.flight.yaw, 0., 0.);
            cockpit.flight.velocity = std::array::from_fn(|i| {
                basis.forward[i] * cockpit.flight.speed + self.terrain.wind()[i]
            });
        }
        if let Some(object) = self.setup.ground_start {
            let (position, heading) = match &parked {
                Some(ground) => (ground.slots[0], ground.heading),
                None => mission_layout::runway_pose(&self.terrain, object)?,
            };
            cockpit.flight.position[0] = position[0];
            cockpit.flight.position[2] = position[2];
            if self.setup.mission.is_none() {
                cockpit.flight.position[1] = cockpit.flight.position[1].max(position[1] + 5000.);
            }
            cockpit.flight.yaw = heading;
            let basis = attitude::Basis::new(heading, 0., 0.);
            cockpit.flight.velocity = std::array::from_fn(|i| {
                basis.forward[i] * cockpit.flight.speed + self.terrain.wind()[i]
            });
        }
        if self.setup.researched_flight {
            cockpit.flight.enable_research(1)?;
        }
        if let Some(tables) = &self.setup.native_tables {
            if self.combat.range || self.setup.mission.is_some() {
                return Err("native research flight currently requires clean free flight".into());
            }
            cockpit.flight.enable_native(tables.clone(), 1)?;
        }
        self.combat.reset(&mut cockpit.flight)?;
        self.combat.raise_airborne_spawns(&self.terrain);
        if self.combat.uses_normal_startup_defaults() {
            self.combat.apply_startup_weapons();
        }
        let ground_airport = match self.setup.ground_start {
            Some(object) => Some(match &parked {
                Some(ground) => {
                    mission_layout::place_on_runway(&self.terrain, &mut cockpit.flight, ground, 0)
                        .map(|()| ground.airport)?
                }
                None => {
                    mission_layout::apply_ground_start(&self.terrain, &mut cockpit.flight, object)?
                }
            }),
            None => None,
        };
        cockpit.result = ai_wings::outcome::Tracker::new(ai_wings::outcome::home_base(
            &self.terrain,
            ground_airport,
        ));
        cockpit
            .airport_service
            .reset(&self.terrain.airport_scene)
            .map_err(std::io::Error::other)?;
        // A ground start begins on NAV, and so does an aircraft with nothing
        // loaded in the selected station: an empty station is never armed.
        let own = self.combat.own();
        cockpit.airport_nav_mode = ground_airport.is_some()
            || !own.carries(own.selected, self.combat.state.cheats.unlimited_ammo);
        self.combat.own_mut().armed = !cockpit.airport_nav_mode;
        if let Some(airport) = ground_airport {
            cockpit.airport_service.command(
                &self.terrain.airport_scene,
                airport_aircraft(&self.terrain, &cockpit.flight, cockpit.airport_nav_mode),
                tore_sim::airport::Command::SelectAirport(airport),
            );
        }
        // The AI bridge is built from the targets the existing spawner just
        // placed, so the AI aircraft start exactly where the straight-flight
        // fixtures would have started.
        self.ai_wings = None;
        self.combat.ai_poses = false;
        let mut ai_aircraft = None;
        if self.setup.mission.is_some()
            && let Some(ai) = &self.setup.ai
        {
            // Home runways for every aircraft; a ground start parks the
            // player's wingmen behind the player.
            let airfields = ai_wings::Airfields::from_world(
                &self.terrain,
                parked.as_ref().map(mission_layout::GroundLayout::departure),
            );
            let mut bridge = ai_wings::AiWings::build_mission(
                &ai.wings,
                &self.combat.state.targets,
                ai.guns_only,
                resources,
                &airfields,
            )?;
            bridge.apply_mission_preset(ai.preset, cockpit.flight.position);
            bridge.apply_group_objectives(&ai.group_objectives, cockpit.flight.position);
            bridge.apply_group_survival(&ai.group_must_survive);
            bridge.mirror_pose_out(&mut self.combat.state.targets);
            // The designation keys skip the player's friends.
            self.combat.own_mut().friendlies = bridge.friendly_ids();
            self.combat.ai_poses = !bridge.is_empty();
            ai_aircraft = Some(bridge.len());
            self.ai_wings = Some(bridge);
        }
        self.roster = Roster::single_player(
            comms::crew(&aircraft.profile),
            self.ai_wings.iter().flat_map(ai_planes),
        );
        self.comms
            .set_seats(self.roster.seats().iter().map(|seat| seat.id));
        // Draw from the placed start, including the AI's own poses.
        self.combat
            .restart_render(&cockpit.flight, self.ai_wings.as_ref());
        cockpit.previous_flight = cockpit.flight.clone();
        cockpit.overspeed_message_at = None;
        cockpit.edge_message_at = None;
        Ok(Restarted {
            ground_airport,
            layout,
            ai_aircraft,
        })
    }

    /// The tick the next step runs. Seat inputs name it.
    pub fn tick(&self) -> u64 {
        self.combat.state.tick()
    }

    /// Where the plane `seat` flies keeps its cockpit, if it flies one.
    pub fn cockpit_of(&self, seat: SeatId) -> Option<usize> {
        let plane = self.roster.seat(seat)?.plane?;
        self.cockpits
            .iter()
            .position(|cockpit| cockpit.plane == plane)
    }

    fn cycle_cockpit_weapon(&mut self, cockpit: usize, forward: bool) {
        self.combat.cancel();
        self.combat.command(
            if forward {
                tore_sim::combat::live::Command::NextSelection
            } else {
                tore_sim::combat::live::Command::PreviousSelection
            },
            combat::launcher(&self.cockpits[cockpit].flight),
        );
        self.cockpits[cockpit].airport_nav_mode = !self.combat.own().armed;
    }

    /// Each cockpit's input for this tick, in cockpit order: every seat that
    /// flies a plane sends exactly one, for [`Self::tick`], and no other seat
    /// sends any.
    fn cockpit_inputs<'a>(&self, inputs: &'a [SeatInput]) -> WorldResult<Vec<&'a SeatInput>> {
        let tick = self.tick();
        for input in inputs {
            if input.tick != tick {
                return Err(format!(
                    "seat {} sent input for tick {}, but the next tick is {tick}",
                    input.seat.0, input.tick
                )
                .into());
            }
            if inputs
                .iter()
                .filter(|other| other.seat == input.seat)
                .count()
                > 1
            {
                return Err(format!("seat {} sent two inputs for one tick", input.seat.0).into());
            }
            if self.cockpit_of(input.seat).is_none() {
                return Err(format!("seat {} flies no plane", input.seat.0).into());
            }
        }
        self.cockpits
            .iter()
            .map(|cockpit| {
                let seat = self.roster.seat_of(cockpit.plane).ok_or_else(|| {
                    format!("plane {} has a cockpit but no human pilot", cockpit.plane.0)
                })?;
                inputs
                    .iter()
                    .find(|input| input.seat == seat)
                    .ok_or_else(|| format!("seat {} sent no input for tick {tick}", seat.0).into())
            })
            .collect()
    }

    /// One fixed 120 Hz tick of the whole mission, with one input from every
    /// seat that flies a plane. See the module documentation; the order below
    /// is the order the redraw loop ran. Combat, the AI and the radio still
    /// serve the first cockpit only (docs/ARCHITECTURE.md, "Where the code
    /// stands").
    pub fn step(&mut self, inputs: &[SeatInput], out: &mut TickOutput) -> WorldResult<()> {
        self.step_with(&[], inputs, out, |_, _| Ok(()))
    }

    /// [`Self::step`], calling `commands_applied` once the command phase is
    /// over and before the rest of the tick runs, with the world and what the
    /// commands produced so far. A driver that reports what a command did, as
    /// the AI probe does, reads it there; an error stops the tick.
    pub fn step_observed(
        &mut self,
        inputs: &[SeatInput],
        out: &mut TickOutput,
        commands_applied: impl FnOnce(&World, &TickOutput) -> WorldResult<()>,
    ) -> WorldResult<()> {
        self.step_with(&[], inputs, out, commands_applied)
    }

    /// The whole step: `mission` commands first, then each seat's, then
    /// `commands_applied` as in [`Self::step_observed`].
    pub fn step_with(
        &mut self,
        mission: &[MissionCommand],
        inputs: &[SeatInput],
        out: &mut TickOutput,
        commands_applied: impl FnOnce(&World, &TickOutput) -> WorldResult<()>,
    ) -> WorldResult<()> {
        *out = TickOutput::default();
        let inputs = self.cockpit_inputs(inputs)?;
        let Some(&first_input) = inputs.first() else {
            return Err("a tick needs a human-flown plane".into());
        };
        // Mission commands first, then each seat's commands in seat order on
        // its own plane.
        for command in mission {
            self.apply_mission_command(command);
        }
        let mut by_seat: Vec<(usize, &SeatInput)> = inputs.iter().copied().enumerate().collect();
        by_seat.sort_by_key(|(_, input)| input.seat);
        for (cockpit, input) in by_seat {
            self.apply_seat_commands(cockpit, input, out);
        }
        out.commanded = out.cues.len();
        commands_applied(self, out)?;
        for (cockpit, input) in self.cockpits.iter_mut().zip(&inputs) {
            cockpit.previous_flight.clone_from(&cockpit.flight);
            cockpit
                .flight
                .step_surface(&input.pilot, |x, z| self.terrain.surface(x, z));
            if cockpit.flight.native.is_none()
                && self
                    .terrain
                    .solid_contact(
                        cockpit.previous_flight.position,
                        cockpit.flight.position,
                        self.combat
                            .state
                            .targets
                            .iter()
                            .filter(|target| target.hp > 0)
                            .map(|target| target.id),
                    )
                    .is_some()
            {
                if cockpit.flight.cheats.no_crashes {
                    cockpit.flight.rebound(cockpit.previous_flight.position);
                } else {
                    cockpit.flight.crashed = true;
                }
            }
            if let Some(error) = cockpit.flight.native_fault() {
                out.fault = Some(error.to_owned());
                return Ok(());
            }
        }
        // Weather shares the authoritative tick; pausing simply stops calling
        // it, with no elapsed-time catch-up.
        self.terrain.weather.step();
        for cockpit in &mut self.cockpits {
            let turbulence = !cockpit.flight.cheats.no_turbulence;
            let turbulence_cue = step_turbulence(
                &mut cockpit.turbulence,
                &mut cockpit.turbulence_rng,
                &mut cockpit.flight,
                &self.terrain,
                turbulence,
            );
            edge_and_overspeed(cockpit, &self.terrain, out);
            if let Some(cue) = turbulence_cue {
                out.cues.push(Cue::Feedback(cue));
            }
        }
        out.cues.push(Cue::Flown);
        self.combat
            .controller
            .space(first_input.trigger, false, false);
        let own = &mut self.cockpits[0];
        if self.combat.recording_tape() {
            let airport = airport_aircraft(&self.terrain, &own.flight, own.airport_nav_mode);
            self.combat.record_tape(
                format!(
                    "airport-state:{}:{}:{}",
                    u8::from(airport.nav_mode),
                    u8::from(airport.gear_down),
                    u8::from(airport.supported)
                ),
                combat::launcher(&own.flight),
            );
        }
        let events = self.combat.step(&mut own.flight, &self.terrain)?;
        out.cues.push(Cue::CombatStepped);
        for cockpit in &mut self.cockpits {
            for airport_event in cockpit.airport_service.synchronize_health(
                self.combat
                    .state
                    .targets
                    .iter()
                    .filter(|target| target.role == tore_sim::combat::missiles::TargetRole::Surface)
                    .map(|target| (target.id, target.hp)),
            ) {
                if matches!(
                    airport_event,
                    tore_sim::airport::Event::ClearanceInvalidated(_)
                ) {
                    let message = "Landing clearance cancelled: runway unavailable";
                    // Journal only: the tower speech it cuts.
                    self.comms
                        .record(comms::journal::Entry::clearance_cancelled(
                            self.combat.state.tick() as f64 / 120.,
                            message,
                        ));
                    out.cues.push(Cue::Message(message.into()));
                    out.cues.push(Cue::Tower(None));
                }
            }
            for event in cockpit.airport_service.step(
                &self.terrain.airport_scene,
                airport_aircraft(&self.terrain, &cockpit.flight, cockpit.airport_nav_mode),
            ) {
                if matches!(event, tore_sim::airport::Event::LandingComplete { .. }) {
                    out.cues.push(Cue::Message("Landing complete".into()));
                }
            }
        }
        let own = &mut self.cockpits[0];
        // Manual p.65: the player always lands first and other aircraft hold at
        // marshal. The retail condition (gear, height, speed and range) is
        // re-evaluated every tick, so climbing away, raising the gear, a crash
        // or restart release it.
        if let Some(wings) = &mut self.ai_wings {
            let [x, _, z] = own.flight.position;
            wings.update_player_landing(
                &self.terrain.airport_scene,
                &own.airport_service,
                &own.flight,
                self.terrain.surface(x, z).height,
            );
        }
        for message in own.flight.systems.messages.drain(..) {
            out.cues.push(Cue::Message(message));
        }
        for event in &events {
            use tore_sim::combat::live::Event;
            if let Some(cue) = combat::feedback(event, self.combat.own().configuration()) {
                out.cues.push(Cue::Feedback(cue));
            }
            match event {
                Event::Jolt(jolt) => match jolt.target {
                    None => own.flight.jolt_from(jolt.from, jolt.strength),
                    Some(id) => {
                        if let Some(wings) = &mut self.ai_wings {
                            wings.jolt(id, jolt.from, jolt.strength);
                        }
                    }
                },
                Event::PlayerDamaged(_)
                | Event::Hit(_)
                | Event::Ground
                | Event::Destroyed(_)
                | Event::PilotKilled
                | Event::SubsystemDamaged(_)
                | Event::Defeated(_)
                | Event::TrackLost(_)
                | Event::SeekerActivated(_)
                | Event::Pitbull(_) => {}
                Event::PlayerDestroyed => {
                    own.flight.crashed = true;
                }
                Event::Fired(i) => {
                    if let Some(name) = self.combat.own().configuration().stations[*i]
                        .weapon
                        .fire_sound
                        .as_deref()
                    {
                        out.releases.push((name.to_string(), *i));
                    }
                }
                Event::PlayerGroundImpact => {
                    out.cues
                        .push(Cue::Message("Your aircraft exploded on impact".into()));
                }
                Event::Airburst(id) => {
                    out.cues.push(Cue::Message(
                        if *id == 0 {
                            "Your aircraft exploded"
                        } else {
                            "Destroyed aircraft exploded"
                        }
                        .into(),
                    ));
                }
            }
        }
        // One AI tick per combat tick, immediately after it, so the AI reads
        // the damage combat just applied and then writes the authoritative pose
        // back.
        if let Some(mut bridge) = self.ai_wings.take() {
            bridge.report_weapon_hits(&events);
            let stepped = bridge.step(&mut self.combat.state, &own.flight, &self.terrain);
            self.combat.ai_crashes(&bridge, &self.terrain);
            for (id, message, friendly) in bridge.ejection_events.drain(..) {
                out.cues.push(Cue::WingEjection {
                    id,
                    message,
                    friendly,
                });
            }
            let message = bridge.take_message();
            self.ai_wings = Some(bridge);
            stepped?;
            if let Some(message) = message {
                out.cues.push(Cue::Message(message));
            }
        }
        // The tick's picture: combat and the AI have both written their poses
        // for it. The mission recording reads the same picture, before the
        // radio drains this tick's strikes.
        self.combat
            .advance_render(&own.flight, self.ai_wings.as_ref());
        out.outcomes = self.combat.state.ledger.take_outcomes();
        // The AI's messages of this tick; the journal is write-only, so
        // draining it changes nothing.
        out.journal = self
            .ai_wings
            .as_mut()
            .map(ai_wings::AiWings::take_ai_journal);
        out.cues.push(Cue::Picture);
        self.step_radio(out, &events);
        out.emissions = self.combat.state.take_sound_events();
        out.events = events;
        Ok(())
    }

    /// Whether the plane in `cockpit` and its pilot are alive, as the radio
    /// hears it. Combat keeps the hit points of the first cockpit's plane
    /// only, until each human-flown plane has its own ownship (stage B1).
    fn cockpit_alive(&self, cockpit: usize) -> bool {
        let flight = &self.cockpits[cockpit].flight;
        !flight.crashed && (cockpit != 0 || self.combat.state.own().hp > 0)
    }

    /// The radio's half of a tick: the tower and crew voice of every
    /// human-flown plane, the weapon, hit and wing calls, and the calls due
    /// now, delivered to the seats that hear them. Each call is made once;
    /// every seat has its own queue and busy hold.
    fn step_radio(&mut self, out: &mut TickOutput, events: &[tore_sim::combat::live::Event]) {
        let now = self.combat.state.tick() as f64 / 120.;
        let members = radio_calls::members(&self.roster, self.ai_wings.as_ref(), |plane| {
            self.cockpits
                .iter()
                .position(|cockpit| cockpit.plane == plane)
                .is_some_and(|cockpit| self.cockpit_alive(cockpit))
        });
        let listeners: Vec<Option<radio_calls::Listener>> = self
            .cockpits
            .iter()
            .enumerate()
            .map(|(index, cockpit)| {
                let seat = self.roster.seat(self.roster.seat_of(cockpit.plane)?)?;
                let member = members.iter().find(|m| m.id == cockpit.plane.0)?;
                Some(radio_calls::Listener {
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
        // Each plane's own side of the tower conversation, then what the
        // wingmen report, decided once and queued by every seat in the wing.
        let mut flying = vec![false; self.cockpits.len()];
        for (index, cockpit) in self.cockpits.iter_mut().enumerate() {
            flying[index] = cockpit.airfield_radio.step_player(
                now,
                &self.phrases,
                &mut self.comms,
                &cockpit.flight,
                &self.terrain,
                &cockpit.airport_service,
                self.ai_wings.as_ref(),
            );
        }
        let listening: Vec<u8> = listeners
            .iter()
            .zip(&flying)
            .filter_map(|(listener, flying)| listener.as_ref().filter(|_| *flying))
            .map(|listener| listener.flight)
            .collect();
        let reports = match &self.ai_wings {
            Some(wings) if !listening.is_empty() => self.wing_status.step(
                &self.phrases,
                &mut self.comms,
                &self.terrain,
                wings,
                &members,
                &listening,
            ),
            _ => Vec::new(),
        };
        for (index, cockpit) in self.cockpits.iter_mut().enumerate() {
            let Some(listener) = &listeners[index] else {
                continue;
            };
            if !flying[index] {
                continue;
            }
            let mine: Vec<_> = reports
                .iter()
                .filter(|report| report.flight == listener.flight)
                .map(|report| report.event.clone())
                .collect();
            cockpit.airfield_radio.apply_wing(now, &mine);
            cockpit.airfield_radio.deliver(now, &mut self.comms);
        }
        for (index, cockpit) in self.cockpits.iter_mut().enumerate() {
            let slot = self
                .roster
                .plane(cockpit.plane)
                .map_or(Slot::FRIENDLY_LEAD, |plane| plane.slot);
            cockpit.crew_voice.step_host(
                &mut self.comms,
                &self.phrases,
                &crew_voice::Host {
                    flight: &cockpit.flight,
                    combat: &self.combat.state,
                    wings: self.ai_wings.as_ref(),
                    world: &self.terrain,
                    slot,
                    // Combat's player-only state is the first cockpit's until
                    // every human-flown plane has an ownship (stage B1).
                    ownship: index == 0,
                },
            );
        }
        let listeners: Vec<radio_calls::Listener> = listeners.into_iter().flatten().collect();
        radio_calls::step(
            &mut self.radio,
            &mut self.comms,
            &self.phrases,
            &listeners,
            &members,
            events,
            &mut self.combat.state,
            self.ai_wings.as_mut(),
        );
        for delivery in self.comms.due(now) {
            out.cues.push(Cue::Radio {
                seat: delivery.seat,
                call: delivery.call,
            });
        }
        self.step_results(now);
    }

    /// Each human-flown plane's mission result and home checks, every 4
    /// seconds, and the calls they send to its seat: "mission accomplished"
    /// two seconds after the result is decided and "almost home". These used
    /// to be the app's situation music's, which exists only with an audio
    /// device (docs/ARCHITECTURE.md, "Radio, orders and debrief for each
    /// seat"). They are sent after this tick's due calls, so they are
    /// delivered on the next tick, as they were.
    fn step_results(&mut self, now: f64) {
        let mission = self.setup.mission.is_some() && self.ai_wings.is_some();
        let ticks = self.combat.state.tick();
        // The checks' clock counts ticks from the flight's first one.
        let clock = ticks.saturating_sub(1) as f64 * flight::DT;
        for index in 0..self.cockpits.len() {
            let Some(seat) = self
                .roster
                .seat_of(self.cockpits[index].plane)
                .and_then(|seat| self.roster.seat(seat))
            else {
                continue;
            };
            let (seat, crew) = (seat.id, seat.crew);
            let plane = self.cockpits[index].plane.0;
            let pilot = &self.cockpits[index].flight.systems.pilot;
            let alive = self.cockpit_alive(index) && !pilot.dead && !pilot.ejected;
            let cockpit = &mut self.cockpits[index];
            let flight = &cockpit.flight;
            let [x, y, z] = flight.position;
            let on_runway = self
                .terrain
                .airport_scene
                .runway_surface(x, z)
                .is_some_and(|(_, height)| flight.supported_at(height));
            let airborne = !on_runway && !flight.research.as_ref().is_some_and(|r| r.on_ground);
            let state = &self.combat.state;
            let wings = self.ai_wings.as_ref();
            let succeeded = || ai_wings::outcome::succeeded(state, wings, plane, alive);
            let results =
                cockpit
                    .result
                    .results(clock, mission.then_some(succeeded), [x, y, z], airborne);
            let label = crew.map_or("YOU", comms::Crew::label);
            for result in results {
                self.comms.send(
                    now,
                    result.call(plane, label, &self.phrases),
                    &[comms::Hearer::seat(seat)],
                );
            }
        }
    }

    /// A queued NAV mode switch or tower request for a cockpit's plane,
    /// applied at the start of the tick in the order it was given.
    fn airport_command(&mut self, cockpit: usize, command: AirportInput, out: &mut TickOutput) {
        let seat = self
            .roster
            .seat_of(self.cockpits[cockpit].plane)
            .unwrap_or_default();
        let cockpit = &mut self.cockpits[cockpit];
        match command {
            AirportInput::NavMode => {
                cockpit.airport_nav_mode = !cockpit.airport_nav_mode;
                self.combat.cancel();
                self.combat.command(
                    if cockpit.airport_nav_mode {
                        tore_sim::combat::live::Command::SelectNav
                    } else {
                        tore_sim::combat::live::Command::NextSelection
                    },
                    combat::launcher(&cockpit.flight),
                );
                self.combat.record_tape(
                    if cockpit.airport_nav_mode {
                        "airport-nav:1"
                    } else {
                        "airport-nav:0"
                    },
                    combat::launcher(&cockpit.flight),
                );
                out.cues.push(Cue::Message(
                    if cockpit.airport_nav_mode {
                        "Navigation mode selected"
                    } else {
                        "Navigation mode off"
                    }
                    .into(),
                ));
            }
            AirportInput::Command(command) => {
                self.combat.record_tape(
                    combat_tape::airport_command_name(command),
                    combat::launcher(&cockpit.flight),
                );
                let aircraft =
                    airport_aircraft(&self.terrain, &cockpit.flight, cockpit.airport_nav_mode);
                for event in
                    cockpit
                        .airport_service
                        .command(&self.terrain.airport_scene, aircraft, command)
                {
                    if let tore_sim::airport::Event::Reply(reply) = event {
                        cockpit.airfield_radio.reply(&reply);
                        self.comms.cancel_airport(seat);
                        self.comms
                            .spoken(seat, self.combat.state.tick() as f64 / 120.);
                        // Journal only: the reply printed and played.
                        self.comms.record(comms::journal::Entry::tower_reply(
                            self.combat.state.tick() as f64 / 120.,
                            airport_reply(&self.terrain, &reply),
                            airport_reply_audio(&reply),
                        ));
                        out.cues
                            .push(Cue::Message(airport_reply(&self.terrain, &reply)));
                        out.cues.push(Cue::Tower(airport_reply_audio(&reply)));
                    }
                }
            }
        }
    }
}

/// The AI's planes and where each sits, from the AI wings' slots.
pub fn ai_planes(wings: &ai_wings::AiWings) -> impl Iterator<Item = (PlaneId, Slot)> + '_ {
    wings.slots().iter().map(|slot| {
        (
            PlaneId(slot.id),
            Slot {
                wing: tore_sim::ai::launch::WingId {
                    side: slot.side,
                    index: slot.wing_number - 1,
                },
                member: slot.member_number - 1,
            },
        )
    })
}

/// One tick of physical turbulence, applied to attitude and height only.
/// Velocity is untouched: the recovered routine is an angular and vertical
/// perturbation, not a three-dimensional wind field.
pub fn step_turbulence(
    turbulence: &mut tore_sim::turbulence::Turbulence,
    rng: &mut tore_formats::flight_model::clock_rng::NativeRng,
    flight: &mut flight::State,
    world: &terrain::Terrain,
    enabled: bool,
) -> Option<tore_input::FeedbackEvent> {
    // The joined native service explicitly selects the source disabled branch.
    if flight.native.is_some() {
        return None;
    }
    let ground = f64::from(world.height(flight.position[0] as f32, flight.position[2] as f32));
    let agl = flight.position[1] - ground;
    let conditions = tore_sim::turbulence::Conditions {
        agl_feet: agl,
        on_ground: flight.crashed || flight.research.as_ref().is_some_and(|r| r.on_ground),
        speed_fps: flight.speed,
        seconds_of_day: world.weather.seconds_of_day(),
        percent: flight.model().configuration().turbulence_percent,
        daytime_ground: world.turbulence_reduced_surface(flight.position[0], flight.position[2]),
        enabled,
        nearby_strength: 0,
    };
    let d = turbulence
        .step(world.weather.ticks(), 2, conditions, rng)
        .ok()?;
    if d == tore_sim::turbulence::Disturbance::default() {
        return None;
    }
    flight.apply_turbulence(d);
    d.shake.then(|| tore_input::FeedbackEvent::Turbulence {
        intensity: d.severity(),
    })
}

/// The player's aircraft as the airport service sees it.
pub fn airport_aircraft(
    world: &terrain::Terrain,
    flight: &flight::State,
    nav_mode: bool,
) -> tore_sim::airport::Aircraft {
    let supported = world
        .airport_scene
        .runway_surface(flight.position[0], flight.position[2])
        .is_some_and(|(_, height)| flight.supported_at(height));
    tore_sim::airport::Aircraft {
        position: flight.position,
        forward: attitude::Basis::new(flight.yaw, flight.pitch, flight.bank).forward,
        nav_mode,
        gear_down: flight.gear_down,
        supported,
        alive: !flight.crashed,
        speed_fps: flight.speed,
        ground_clearance_ft: flight.model().configuration().equipment.ground_clearance_ft,
    }
}

/// The HUD line for a tower reply.
pub fn airport_reply(world: &terrain::Terrain, reply: &tore_sim::airport::Reply) -> String {
    use tore_sim::airport::Reply;
    match reply {
        Reply::Selected { airport } => world
            .airport_scene
            .airports
            .iter()
            .find(|a| a.id == *airport)
            .map_or_else(
                || "Airport unavailable".into(),
                |a| format!("Selected {}", a.name),
            ),
        Reply::Cleared {
            airport,
            runway,
            end,
        } => {
            let name = world
                .airport_scene
                .airports
                .iter()
                .find(|a| a.id == *airport)
                .map_or("Airport", |a| a.name.as_str());
            let runway_name = world
                .airport_scene
                .runway(*runway)
                .map_or("runway", |r| r.name.as_str());
            let end = match end {
                tore_sim::airport::ApproachEnd::Near => "near approach",
                tore_sim::airport::ApproachEnd::Far => "far approach",
            };
            format!("{name}: cleared to land, {runway_name}, {end}")
        }
        Reply::Landed { airport, .. } => {
            let name = world
                .airport_scene
                .airports
                .iter()
                .find(|a| a.id == *airport)
                .map_or("Airport", |a| a.name.as_str());
            format!("{name}: welcome home, landing complete")
        }
        Reply::Declined { reason, .. } => {
            use tore_sim::airport::DeclineReason::*;
            let reason = match reason {
                NoAirport => "no airport selected",
                NoRunway => "no usable runway",
                Hostile => "airport is hostile",
                NeutralPermission => "permission is required",
                UnknownAllegiance => "airport allegiance is unknown",
                RunwayDisabled => "runway is unavailable",
            };
            format!("Landing request declined: {reason}")
        }
        Reply::Repeated(reply) => airport_reply(world, reply),
        Reply::Cancelled { .. } => "Approach cancelled".into(),
    }
}

/// The tower's recording for a reply, if it has one.
pub fn airport_reply_audio(reply: &tore_sim::airport::Reply) -> Option<&'static str> {
    use tore_sim::airport::Reply;
    match reply {
        Reply::Cleared { .. } => Some(tore_formats::radio::AIRPORT_CLEAR_TO_LAND),
        Reply::Landed { .. } => Some(tore_formats::radio::AIRPORT_WELCOME_HOME),
        Reply::Repeated(reply) => airport_reply_audio(reply),
        Reply::Selected { .. } | Reply::Declined { .. } | Reply::Cancelled { .. } => None,
    }
}

/// Beyond the map: a turn-back warning every ten seconds from 100 nautical
/// miles out, and the aircraft is lost at 105. Past the top speed: a short
/// cockpit message, repeated every four seconds. Both requested by John,
/// 2026-09-29; see docs/spec/world-edge.md and docs/spec/overspeed.md. Each
/// human-flown plane has its own warnings and its own clocks.
fn edge_and_overspeed(cockpit: &mut Cockpit, terrain: &terrain::Terrain, out: &mut TickOutput) {
    let flight = &mut cockpit.flight;
    if !flight.crashed {
        let [x, _, z] = flight.position;
        let out_nm = terrain.edge_distance_nm(x, z);
        if out_nm >= terrain::EDGE_DESTROY_NM {
            flight
                .systems
                .destroy(tore_sim::aircraft_systems::LossCause::OutOfBounds);
            flight.crashed = true;
        } else if out_nm >= terrain::EDGE_WARNING_NM {
            let now = flight.ticks as f64 * flight::DT;
            if cockpit
                .edge_message_at
                .is_none_or(|at| now - at >= 10. || now < at)
            {
                cockpit.edge_message_at = Some(now);
                out.cues.push(Cue::Message(
                    "You have left the theater: turn back now".into(),
                ));
            }
        } else {
            cockpit.edge_message_at = None;
        }
    }
    if !flight.crashed
        && flight
            .overspeed_ratio()
            .is_some_and(|r| r >= flight::OVERSPEED_SHAKE_FULL)
    {
        let now = flight.ticks as f64 * flight::DT;
        if cockpit
            .overspeed_message_at
            .is_none_or(|at| now - at >= 4. || now < at)
        {
            cockpit.overspeed_message_at = Some(now);
            out.cues.push(Cue::Message("OVERSPEED".into()));
        }
    }
}
