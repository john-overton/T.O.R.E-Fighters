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
    datalink, mission_layout, radio_calls,
    resources::ResourceSource,
    seats::{PlaneId, Roster, SeatId, SeatInput, Slot},
    terrain,
};
use tore_sim::models::FlightModel;
use tore_sim::{attitude, flight};

#[cfg(test)]
mod ai_wings_checkpoint_tests;
mod build;
#[cfg(test)]
mod checkpoint_scenarios;
#[cfg(test)]
mod checkpoint_tests;
// Exact checkpoints (docs/formats/checkpoint.md): the cockpits section.
#[cfg(test)]
mod build_tests;
#[path = "world_checkpoint.rs"]
pub(crate) mod checkpoint;
pub use build::{Built, Hooks, Seating};
#[cfg(test)]
mod combat_core_checkpoint_tests;
#[cfg(test)]
mod command_tests;
mod commands;
#[cfg(test)]
mod crowd;
#[cfg(test)]
mod datalink_tests;
#[cfg(test)]
mod engagement_tests;
#[cfg(test)]
mod fight_tests;
#[cfg(test)]
mod frame_tests;
mod handoff;
#[cfg(test)]
mod handoff_tests;
#[cfg(test)]
mod lagcomp_tests;
#[cfg(test)]
mod loadout_tests;
#[cfg(test)]
mod open_tests;
pub mod plane;
#[cfg(test)]
mod plane_tests;
#[cfg(test)]
mod radio_checkpoint_tests;
#[cfg(test)]
mod readout_tests;
#[cfg(test)]
mod records_checkpoint_tests;
#[cfg(test)]
mod shell_checkpoint_tests;
pub use commands::{MissionCommand, OrderOutcome, OrderReply, Settings};
#[cfg(test)]
mod phase2_seams_tests;
pub mod replies;
pub mod revive;
#[cfg(test)]
mod score_tests;
#[cfg(test)]
mod succession_tests;
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
    /// Single player has one, seat 0's plane 0, and the app presents it; an
    /// open mission has none until a human takes a plane.
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
    /// The flight data link's picture of who tracks, locks and attacks what;
    /// see docs/DATALINK.md. It observes and changes nothing.
    pub datalink: datalink::DataLink,
    /// Imported phrase text for composing radio lines.
    pub phrases: comms::Phrases,
    /// A networked game's score facts, recorded while the host has scoring
    /// on ([`Self::set_scoring`]); `None`, as in single player, records
    /// nothing.
    pub score: Option<crate::score::Recorder>,
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
    /// The flight model of every AI aircraft, a mission setting.
    pub flight_model: ai_wings::AiFlightModel,
    pub group_objectives: [tore_sim::ai::engagement::GroupObjective; 6],
    pub group_must_survive: [bool; 6],
    /// The loadouts the players chose for planes of an open mission, by
    /// plane, checked ([`crate::mission::LoadoutSpec::check_for_plane`]);
    /// every other AI aircraft carries its standard load.
    pub loadouts: std::collections::BTreeMap<u32, tore_sim::combat::loadout::Loadout>,
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
/// the order it happened. The markers say where presentation that reads the
/// simulation used to run inside the tick. Every cue that is for one human
/// names its seat, and a presenter shows only its own seat's; the markers and
/// `WingEjection` are about the mission, so every presenter shows them.
#[derive(Clone, Debug)]
pub enum Cue {
    /// A line for a seat's HUD message area.
    Message { seat: SeatId, text: String },
    /// Controller feedback for a seat.
    Feedback {
        seat: SeatId,
        event: tore_input::FeedbackEvent,
    },
    /// Tower speech for a seat's reply, or `None` to cut the tower off.
    Tower {
        seat: SeatId,
        stem: Option<&'static str>,
    },
    /// The weapon selection moved from the seat's weapon page.
    WeaponCycled { seat: SeatId },
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
    /// A seat's own order call, played at once and cutting off what is
    /// playing, though it may hold no stems.
    OrderVoice {
        seat: SeatId,
        stems: Vec<&'static str>,
    },
}

/// A weapon release sound: what a seat's plane fired.
#[derive(Clone, Debug, PartialEq)]
pub struct Release {
    /// The seat whose plane fired.
    pub seat: SeatId,
    /// The sound's name.
    pub sound: String,
    /// The station of that plane's stores it came from.
    pub station: usize,
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
    /// What became of each wing order given this tick; each names its seat.
    pub orders: Vec<OrderReply>,
    /// Combat's events for the tick.
    pub events: Vec<tore_sim::combat::live::Event>,
    /// Every seat's weapon release sounds, in the order the events came.
    pub releases: Vec<Release>,
    /// Shot outcomes the ledger resolved during the tick.
    pub outcomes: Vec<tore_sim::combat::ledger::Outcome>,
    /// The AI message journal of the tick (`None` without AI).
    pub journal: Option<tore_sim::ai::thought::JournalBatch>,
    /// The tick's sound emissions.
    pub emissions: Vec<tore_sim::acoustics::Emission>,
    /// A native research fault stopped the tick after the player's flight.
    pub fault: Option<String>,
    /// The ownship terms each human-flown plane took from combat this tick,
    /// in plane id order: what a client's copy of the plane's step reads.
    pub terms: Vec<(PlaneId, plane::OwnshipTerms)>,
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
    /// from the first cockpit. An open mission (`Seating::Open`) starts with
    /// every plane on the AI, plane 0 placed where single player's would start,
    /// and no human: every cockpit and seat goes, and humans take planes again
    /// by handoff. The app ends the old flight's recording before and starts
    /// the new one after.
    pub fn restart(
        &mut self,
        aircraft: &aircraft_type::AircraftType,
        resources: &dyn ResourceSource,
    ) -> WorldResult<Restarted> {
        let open = self.combat.is_open();
        // A fixed seed keeps headless runs deterministic.
        self.comms.restart(1);
        self.wing_status.reset();
        self.radio = Default::default();
        self.datalink = Default::default();
        if open {
            self.cockpits.clear();
        } else {
            self.cockpits.truncate(1);
        }
        self.reset_weather();
        if !open && self.cockpits.is_empty() {
            return Err("a flight needs a cockpit to restart".into());
        }
        // Where the lead of Friendly Wing 1 starts: seat 0's flight in single
        // player, and the point the open mission's wings are placed around.
        let mut lead = aircraft.start(&self.terrain);
        if let Some((altitude, fuel)) = self.setup.mission {
            lead.position[1] = altitude;
            lead.fuel = fuel;
        }
        // The accepted creator layout, reused unchanged on restart.
        let layout = self
            .setup
            .mission
            .and(self.combat.mission_layout.clone())
            .filter(|layout| layout.ground.is_some() == self.setup.ground_start.is_some());
        let parked = layout.as_ref().and_then(|layout| layout.ground.clone());
        if let Some(layout) = layout.as_ref().filter(|l| l.player_turn != 0.) {
            // Airborne: the whole scene turns so the enemy ahead stays on the
            // map.
            lead.yaw += layout.player_turn;
            let basis = attitude::Basis::new(lead.yaw, 0., 0.);
            lead.velocity =
                std::array::from_fn(|i| basis.forward[i] * lead.speed + self.terrain.wind()[i]);
        }
        if let Some(object) = self.setup.ground_start {
            let (position, heading) = match &parked {
                Some(ground) => (ground.slots[0], ground.heading),
                None => mission_layout::runway_pose(&self.terrain, object)?,
            };
            lead.position[0] = position[0];
            lead.position[2] = position[2];
            if self.setup.mission.is_none() {
                lead.position[1] = lead.position[1].max(position[1] + 5000.);
            }
            lead.yaw = heading;
            let basis = attitude::Basis::new(heading, 0., 0.);
            lead.velocity =
                std::array::from_fn(|i| basis.forward[i] * lead.speed + self.terrain.wind()[i]);
        }
        if self.setup.researched_flight {
            lead.enable_research(1)?;
        }
        if let Some(tables) = &self.setup.native_tables {
            if self.combat.range || self.setup.mission.is_some() {
                return Err("native research flight currently requires clean free flight".into());
            }
            lead.enable_native(tables.clone(), 1)?;
        }
        self.combat.reset(&mut lead)?;
        self.combat.raise_airborne_spawns(&self.terrain);
        if !open && self.combat.uses_normal_startup_defaults() {
            self.combat.apply_startup_weapons();
        }
        let ground_airport = match self.setup.ground_start {
            Some(object) => Some(match &parked {
                Some(ground) => {
                    mission_layout::place_on_runway(&self.terrain, &mut lead, ground, 0)
                        .map(|()| ground.airport)?
                }
                None => mission_layout::apply_ground_start(&self.terrain, &mut lead, object)?,
            }),
            None => None,
        };
        if let Some(cockpit) = self.cockpits.first_mut() {
            cockpit.plane = PlaneId(0);
            cockpit.crew_voice =
                crew_voice::CrewVoice::new(&aircraft.profile).for_seat(SeatId::default(), 0);
            cockpit
                .airfield_radio
                .reset(parked.as_ref().map(|g| g.runway));
            cockpit.result = ai_wings::outcome::Tracker::new(ai_wings::outcome::home_base(
                &self.terrain,
                ground_airport,
            ));
            cockpit
                .airport_service
                .reset(&self.terrain.airport_scene)
                .map_err(std::io::Error::other)?;
            // A ground start begins on NAV, and so does an aircraft with
            // nothing loaded in the selected station: an empty station is
            // never armed.
            let unlimited_ammo = self.combat.state.cheats.unlimited_ammo;
            let own = self
                .combat
                .state
                .ownship_mut(0)
                .ok_or("single player's plane 0 has an ownship")?;
            cockpit.airport_nav_mode =
                ground_airport.is_some() || !own.carries(own.selected, unlimited_ammo);
            own.armed = !cockpit.airport_nav_mode;
            if let Some(airport) = ground_airport {
                cockpit.airport_service.command(
                    &self.terrain.airport_scene,
                    airport_aircraft(&self.terrain, &lead, cockpit.airport_nav_mode),
                    tore_sim::airport::Command::SelectAirport(airport),
                );
            }
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
            let humans: &[ai_wings::HumanSlot] = if open {
                &[]
            } else {
                &[ai_wings::HumanSlot::SINGLE_PLAYER]
            };
            let mut bridge = ai_wings::AiWings::build_mission_loaded(
                &ai.wings,
                &self.combat.state.targets,
                ai.guns_only,
                resources,
                &airfields,
                humans,
                &ai.loadouts,
            )?;
            bridge.set_flight_model(ai.flight_model)?;
            bridge.apply_mission_preset(ai.preset, lead.position);
            bridge.apply_group_objectives(&ai.group_objectives, lead.position);
            bridge.apply_group_survival(&ai.group_must_survive);
            bridge.mirror_pose_out(&mut self.combat.state.targets);
            self.combat.ai_poses = !bridge.is_empty();
            ai_aircraft = Some(bridge.len());
            self.ai_wings = Some(bridge);
        }
        let ai = self.ai_wings.iter().flat_map(ai_planes);
        self.roster = if open {
            Roster::open(ai)
        } else {
            Roster::single_player(comms::crew(&aircraft.profile), ai)
        };
        self.comms
            .set_seats(self.roster.seats().iter().map(|seat| seat.id));
        if let Some(cockpit) = self.cockpits.first_mut() {
            // The designation keys skip the player's friends.
            if let Some(friends) =
                friendlies_of(self.ai_wings.as_ref(), &self.roster, cockpit.plane)
                && let Some(own) = self.combat.state.ownship_mut(cockpit.plane.0)
            {
                own.friendlies = friends;
            }
            // Draw from the placed start, including the AI's own poses.
            self.combat
                .restart_render(cockpit.plane.0, &lead, self.ai_wings.as_ref());
            cockpit.previous_flight = lead.clone();
            cockpit.flight = lead;
            cockpit.overspeed_message_at = None;
            cockpit.edge_message_at = None;
        }
        Ok(Restarted {
            ground_airport,
            layout,
            ai_aircraft,
        })
    }

    /// Set each human-flown plane's designation skip list to the aircraft of
    /// its own side, humans and AI. Restart does it; a handoff that moves an
    /// aircraft between the AI and a human calls it again.
    pub fn refresh_friendlies(&mut self) {
        for cockpit in &self.cockpits {
            if let Some(friends) =
                friendlies_of(self.ai_wings.as_ref(), &self.roster, cockpit.plane)
                && let Some(own) = self.combat.state.ownship_mut(cockpit.plane.0)
            {
                own.friendlies = friends;
            }
        }
    }

    /// The tick the next step runs. Seat inputs name it.
    pub fn tick(&self) -> u64 {
        self.combat.state.tick()
    }

    /// Turns the recording of score facts on or off (stage F phase 2,
    /// docs/ARCHITECTURE.md "Scoring"). The host turns it on for a networked
    /// mission; single player never does. Turning it on again keeps what is
    /// recorded.
    pub fn set_scoring(&mut self, on: bool) {
        match (on, self.score.is_some()) {
            (true, false) => self.score = Some(crate::score::Recorder::default()),
            (false, _) => self.score = None,
            (true, true) => {}
        }
    }

    /// Whether score facts are recorded.
    pub fn scoring(&self) -> bool {
        self.score.is_some()
    }

    /// The score facts recorded since the last call, leaving none: empty
    /// with scoring off.
    pub fn take_score_facts(&mut self) -> crate::score::Facts {
        self.score
            .as_mut()
            .map(crate::score::Recorder::take)
            .unwrap_or_default()
    }

    /// Where the plane `seat` flies keeps its cockpit, if it flies one.
    pub fn cockpit_of(&self, seat: SeatId) -> Option<usize> {
        let plane = self.roster.seat(seat)?.plane?;
        self.cockpits
            .iter()
            .position(|cockpit| cockpit.plane == plane)
    }

    fn cycle_cockpit_weapon(&mut self, cockpit: usize, forward: bool) {
        let aircraft = self.cockpits[cockpit].plane.0;
        self.combat.cancel_for(aircraft);
        self.combat.command_for(
            aircraft,
            if forward {
                tore_sim::combat::live::Command::NextSelection
            } else {
                tore_sim::combat::live::Command::PreviousSelection
            },
            combat::launcher(&self.cockpits[cockpit].flight),
        );
        self.cockpits[cockpit].airport_nav_mode = self
            .combat
            .state
            .ownship(aircraft)
            .is_none_or(|own| !own.armed);
    }

    /// Checks the seats' inputs against the planes the tick's mission commands
    /// will leave, before any of them applies: every seat that will fly a
    /// plane sends exactly one input for [`Self::tick`], and no other seat
    /// sends any. A handoff changes who flies, so a seat that takes a plane
    /// sends input for the tick and one that gives its plane back sends none.
    fn check_inputs(&self, mission: &[MissionCommand], inputs: &[SeatInput]) -> WorldResult<()> {
        let mut flying: Vec<SeatId> = self
            .roster
            .seats()
            .iter()
            .filter(|seat| seat.plane.is_some())
            .map(|seat| seat.id)
            .collect();
        for command in mission {
            match *command {
                MissionCommand::Take { seat, .. } if !flying.contains(&seat) => flying.push(seat),
                MissionCommand::GiveBack { seat } => flying.retain(|s| *s != seat),
                _ => {}
            }
        }
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
            if !flying.contains(&input.seat) {
                return Err(format!("seat {} flies no plane", input.seat.0).into());
            }
        }
        for seat in flying {
            if !inputs.iter().any(|input| input.seat == seat) {
                return Err(format!("seat {} sent no input for tick {tick}", seat.0).into());
            }
        }
        Ok(())
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
    /// seat that flies a plane, and none when nobody does (an open mission
    /// with no human). See the module documentation; the order below is the
    /// order the redraw loop ran.
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
        let stepped = self.tick();
        self.check_inputs(mission, inputs)?;
        // Mission commands first, then each seat's commands in seat order on
        // its own plane. A handoff changes who flies which plane, so the
        // seats' inputs are checked against the planes as the commands leave
        // them: a seat that took a plane sends input for this tick, a seat
        // that gave its plane back sends none.
        for command in mission {
            self.apply_mission_command(command)?;
        }
        // An open mission steps with no human at all: the AI flies on.
        let inputs = self.cockpit_inputs(inputs)?;
        let mut by_seat: Vec<(usize, &SeatInput)> = inputs.iter().copied().enumerate().collect();
        by_seat.sort_by_key(|(_, input)| input.seat);
        for (cockpit, input) in by_seat {
            self.apply_seat_commands(cockpit, input, out);
        }
        out.commanded = out.cues.len();
        commands_applied(self, out)?;
        // Each human-flown plane's step: docs/ARCHITECTURE.md, "One step for
        // a human's plane".
        for (cockpit, input) in self.cockpits.iter_mut().zip(&inputs) {
            plane::fly(
                &mut cockpit.previous_flight,
                &mut cockpit.flight,
                &input.pilot,
                &self.terrain,
                self.combat
                    .state
                    .targets
                    .iter()
                    .filter(|target| target.hp > 0)
                    .map(|target| target.id),
            );
            if let Some(error) = cockpit.flight.native_fault() {
                out.fault = Some(error.to_owned());
                return Ok(());
            }
        }
        // Weather shares the authoritative tick; pausing simply stops calling
        // it, with no elapsed-time catch-up.
        self.terrain.weather.step();
        let weather = plane::WeatherReading::of(&self.terrain.weather);
        for index in 0..self.cockpits.len() {
            let seat = self.seat_of_cockpit(index);
            let cockpit = &mut self.cockpits[index];
            let warnings = plane::after_weather(
                &mut cockpit.flight,
                &mut cockpit.turbulence,
                &mut cockpit.turbulence_rng,
                &mut cockpit.edge_message_at,
                &mut cockpit.overspeed_message_at,
                &self.terrain,
                weather,
            );
            for text in warnings.messages.into_iter().flatten() {
                out.cues.push(Cue::Message {
                    seat,
                    text: text.into(),
                });
            }
            if let Some(event) = warnings.turbulence {
                out.cues.push(Cue::Feedback { seat, event });
            }
        }
        out.cues.push(Cue::Flown);
        for (cockpit, input) in self.cockpits.iter().zip(&inputs) {
            self.combat
                .trigger(cockpit.plane.0)
                .controller
                .space(input.trigger, false, false);
        }
        if self.combat.recording_tape()
            && let Some(own) = self.cockpits.first()
        {
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
        // Lag compensation: the gun rounds of a seat that says what its screen
        // showed look back that far (docs/ARCHITECTURE.md, "Hits and lag
        // compensation"). Local seats give no view and fire as before.
        let rewinds: Vec<(u32, u16)> = self
            .cockpits
            .iter()
            .zip(&inputs)
            .map(|(cockpit, input)| (cockpit.plane.0, combat::gun_rewind(input.tick, input.view)))
            .filter(|(_, ticks)| *ticks > 0)
            .collect();
        let mut flights: Vec<(u32, &mut flight::State)> = self
            .cockpits
            .iter_mut()
            .map(|cockpit| (cockpit.plane.0, &mut cockpit.flight))
            .collect();
        let combat::Stepped { events, terms } =
            self.combat
                .step_all_rewound(&mut flights, &rewinds, &self.terrain)?;
        out.terms = terms
            .into_iter()
            .map(|(plane, terms)| (PlaneId(plane), terms))
            .collect();
        out.cues.push(Cue::CombatStepped);
        for index in 0..self.cockpits.len() {
            let seat = self.seat_of_cockpit(index);
            let cockpit = &mut self.cockpits[index];
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
                    out.cues.push(Cue::Message {
                        seat,
                        text: message.into(),
                    });
                    out.cues.push(Cue::Tower { seat, stem: None });
                }
            }
            for event in cockpit.airport_service.step(
                &self.terrain.airport_scene,
                airport_aircraft(&self.terrain, &cockpit.flight, cockpit.airport_nav_mode),
            ) {
                if matches!(event, tore_sim::airport::Event::LandingComplete { .. }) {
                    out.cues.push(Cue::Message {
                        seat,
                        text: "Landing complete".into(),
                    });
                }
            }
        }
        // Manual p.65: a human always lands first and other aircraft hold at
        // marshal. The retail condition (gear, height, speed and range) is
        // re-evaluated every tick for every human-flown plane, in id order,
        // so climbing away, raising the gear, a crash or restart release it.
        if let Some(wings) = &mut self.ai_wings {
            for cockpit in &self.cockpits {
                let [x, _, z] = cockpit.flight.position;
                wings.update_landing_priority(
                    cockpit.plane.0,
                    &self.terrain.airport_scene,
                    &cockpit.airport_service,
                    &cockpit.flight,
                    self.terrain.surface(x, z).height,
                );
            }
        }
        // Every human-flown plane's systems messages, in cockpit order, are
        // drained every tick: each queue is its seat's.
        for index in 0..self.cockpits.len() {
            let seat = self.seat_of_cockpit(index);
            for text in self.cockpits[index].flight.systems.messages.drain(..) {
                out.cues.push(Cue::Message { seat, text });
            }
        }
        // Who flies each plane an event can be about, in cockpit order.
        let flown: Vec<(u32, SeatId)> = (0..self.cockpits.len())
            .map(|index| (self.cockpits[index].plane.0, self.seat_of_cockpit(index)))
            .collect();
        for event in &events {
            use tore_sim::combat::live::Event;
            for &(plane, seat) in &flown {
                if let Some(ownship) = self.combat.state.ownship(plane)
                    && let Some(event) = combat::feedback(event, plane, ownship.configuration())
                {
                    out.cues.push(Cue::Feedback { seat, event });
                }
            }
            match event {
                Event::Jolt(jolt) => {
                    if let Some(cockpit) = self
                        .cockpits
                        .iter_mut()
                        .find(|cockpit| cockpit.plane.0 == jolt.target)
                    {
                        plane::take_event(&mut cockpit.flight, cockpit.plane.0, event);
                    } else if let Some(wings) = &mut self.ai_wings {
                        wings.jolt(jolt.target, jolt.from, jolt.strength);
                    }
                }
                Event::OwnshipDamaged { .. }
                | Event::Hit(_)
                | Event::Ground
                | Event::Destroyed(_)
                | Event::PilotKilled { .. }
                | Event::SubsystemDamaged { .. }
                | Event::Defeated(_)
                | Event::TrackLost(_)
                | Event::SeekerActivated(_)
                | Event::Pitbull(_) => {}
                Event::OwnshipDestroyed { aircraft } => {
                    if let Some(cockpit) = self
                        .cockpits
                        .iter_mut()
                        .find(|cockpit| cockpit.plane.0 == *aircraft)
                    {
                        plane::take_event(&mut cockpit.flight, cockpit.plane.0, event);
                    }
                }
                Event::Fired {
                    aircraft,
                    station: i,
                } => {
                    if let Some(&(_, seat)) = flown.iter().find(|(plane, _)| plane == aircraft)
                        && let Some(name) = self.combat.state.ownship(*aircraft).and_then(|own| {
                            own.configuration().stations[*i]
                                .weapon
                                .fire_sound
                                .as_deref()
                        })
                    {
                        out.releases.push(Release {
                            seat,
                            sound: name.to_string(),
                            station: *i,
                        });
                    }
                }
                Event::OwnshipGroundImpact { aircraft } => {
                    if let Some(&(_, seat)) = flown.iter().find(|(plane, _)| plane == aircraft) {
                        out.cues.push(Cue::Message {
                            seat,
                            text: "Your aircraft exploded on impact".into(),
                        });
                    }
                }
                Event::Airburst(id) => {
                    // Every seat reads the burst: its own plane's as "Your
                    // aircraft", any other as a destroyed aircraft.
                    for &(plane, seat) in &flown {
                        out.cues.push(Cue::Message {
                            seat,
                            text: if *id == plane {
                                "Your aircraft exploded"
                            } else {
                                "Destroyed aircraft exploded"
                            }
                            .into(),
                        });
                    }
                }
            }
        }
        // The data link reads the humans' sensors after combat and before the
        // AI step; it changes neither.
        self.datalink.before_ai(&datalink::Scene {
            tick: self.combat.state.tick(),
            roster: &self.roster,
            state: &self.combat.state,
            cockpits: &self.cockpits,
            wings: self.ai_wings.as_ref(),
        });
        // One AI tick per combat tick, immediately after it, so the AI reads
        // the damage combat just applied and then writes the authoritative pose
        // back.
        if let Some(mut bridge) = self.ai_wings.take() {
            bridge.report_weapon_hits(&events);
            // Every human-flown plane, in id order, is a world object to the
            // AI, with its own ownship's hit points and configuration.
            let mut humans = Vec::with_capacity(self.cockpits.len());
            for cockpit in &self.cockpits {
                let plane = self
                    .roster
                    .plane(cockpit.plane)
                    .ok_or("a human-flown plane belongs to the roster")?;
                // A plane combat keeps no ownship for has no hit points or
                // configuration to show the AI, so it is not one of its
                // human aircraft yet.
                let Some(ownship) = self.combat.state.ownship(cockpit.plane.0) else {
                    continue;
                };
                humans.push(ai_wings::HumanAircraft::new(
                    ai_wings::HumanSlot {
                        id: cockpit.plane.0,
                        side: plane.slot.wing.side,
                        wing: plane.slot.wing.index,
                        member: plane.slot.member,
                    },
                    &cockpit.flight,
                    ownship.hp,
                    ownship.configuration(),
                ));
            }
            let stepped = bridge.step(&mut self.combat.state, &humans, &self.terrain);
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
            self.datalink
                .after_ai(self.combat.state.tick(), self.ai_wings.as_ref());
            if let Some(text) = message {
                // Agent decision (B7a): the AI wings' one HUD line goes to
                // the seats flying in Friendly Wing 1, the wing whose
                // formation reports it carries (single player's wing).
                for index in 0..self.cockpits.len() {
                    if self.flies_in_first_friendly_wing(index) {
                        out.cues.push(Cue::Message {
                            seat: self.seat_of_cockpit(index),
                            text: text.clone(),
                        });
                    }
                }
            }
        }
        // The tick's picture: combat and the AI have both written their poses
        // for it. The mission recording reads the same picture, before the
        // radio drains this tick's strikes.
        self.advance_picture();
        out.outcomes = self.combat.state.ledger.take_outcomes();
        // The AI's messages of this tick; the journal is write-only, so
        // draining it changes nothing.
        out.journal = self
            .ai_wings
            .as_mut()
            .map(ai_wings::AiWings::take_ai_journal);
        out.cues.push(Cue::Picture);
        // A networked game's score facts, read before the radio drains this
        // tick's strikes (docs/ARCHITECTURE.md, "Scoring").
        if let Some(mut score) = self.score.take() {
            score.record(self, stepped, self.combat.state.strikes());
            self.score = Some(score);
        }
        self.step_radio(out, &events);
        out.emissions = self.combat.state.take_sound_events();
        out.events = events;
        Ok(())
    }

    /// The seat that flies the plane of `cockpit`. Every cockpit has one, as
    /// a cockpit exists only while a human flies its plane.
    fn seat_of_cockpit(&self, cockpit: usize) -> SeatId {
        self.roster
            .seat_of(self.cockpits[cockpit].plane)
            .unwrap_or_default()
    }

    /// Whether the plane of `cockpit` flies in Friendly Wing 1, the wing the
    /// AI wings' HUD line describes.
    fn flies_in_first_friendly_wing(&self, cockpit: usize) -> bool {
        self.roster
            .plane(self.cockpits[cockpit].plane)
            .is_some_and(|plane| plane.slot.wing == Slot::FRIENDLY_LEAD.wing)
    }

    /// Whether the plane in `cockpit` and its pilot are alive, as the radio
    /// hears it: its flight has not crashed and its ownship has hit points.
    fn cockpit_alive(&self, cockpit: usize) -> bool {
        let cockpit = &self.cockpits[cockpit];
        !cockpit.flight.crashed
            && self
                .combat
                .state
                .ownship(cockpit.plane.0)
                .is_none_or(|own| own.hp > 0)
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
        for cockpit in &mut self.cockpits {
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
                    aircraft: cockpit.plane.0,
                },
            );
        }
        let listeners: Vec<radio_calls::Listener> = listeners.into_iter().flatten().collect();
        let leaders = radio_calls::leaders(&self.roster, &members, self.ai_wings.as_ref());
        radio_calls::step(
            &mut self.radio,
            &mut self.comms,
            &self.phrases,
            &listeners,
            &members,
            &leaders,
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
        let seat = self.seat_of_cockpit(cockpit);
        let cockpit = &mut self.cockpits[cockpit];
        match command {
            AirportInput::NavMode => {
                cockpit.airport_nav_mode = !cockpit.airport_nav_mode;
                let plane = cockpit.plane.0;
                self.combat.cancel_for(plane);
                self.combat.command_for(
                    plane,
                    if cockpit.airport_nav_mode {
                        tore_sim::combat::live::Command::SelectNav
                    } else {
                        tore_sim::combat::live::Command::NextSelection
                    },
                    combat::launcher(&cockpit.flight),
                );
                if Some(plane) == self.combat.host_plane() {
                    self.combat.record_tape(
                        if cockpit.airport_nav_mode {
                            "airport-nav:1"
                        } else {
                            "airport-nav:0"
                        },
                        combat::launcher(&cockpit.flight),
                    );
                }
                out.cues.push(Cue::Message {
                    seat,
                    text: if cockpit.airport_nav_mode {
                        "Navigation mode selected"
                    } else {
                        "Navigation mode off"
                    }
                    .into(),
                });
            }
            AirportInput::Command(command) => {
                if Some(cockpit.plane.0) == self.combat.host_plane() {
                    self.combat.record_tape(
                        combat_tape::airport_command_name(command),
                        combat::launcher(&cockpit.flight),
                    );
                }
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
                        out.cues.push(Cue::Message {
                            seat,
                            text: airport_reply(&self.terrain, &reply),
                        });
                        out.cues.push(Cue::Tower {
                            seat,
                            stem: airport_reply_audio(&reply),
                        });
                    }
                }
            }
        }
    }
}

/// The AI's planes and where each sits, from the AI wings' slots.
/// The aircraft on `plane`'s own side, which its designation keys skip. `None`
/// without an AI bridge or when the roster does not hold the plane.
fn friendlies_of(
    wings: Option<&ai_wings::AiWings>,
    roster: &Roster,
    plane: PlaneId,
) -> Option<std::collections::BTreeSet<u32>> {
    Some(wings?.friendly_ids(roster.plane(plane)?.slot.wing.side))
}

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

/// One tick of physical turbulence at the weather clock's current reading;
/// see [`plane::step_turbulence`].
pub fn step_turbulence(
    turbulence: &mut tore_sim::turbulence::Turbulence,
    rng: &mut tore_formats::flight_model::clock_rng::NativeRng,
    flight: &mut flight::State,
    world: &terrain::Terrain,
    enabled: bool,
) -> Option<tore_input::FeedbackEvent> {
    plane::step_turbulence(
        turbulence,
        rng,
        flight,
        world,
        plane::WeatherReading::of(&world.weather),
        enabled,
    )
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
