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
    AppResult, ai_wings, aircraft, airfield_radio, attitude, combat, combat_tape, comms,
    crew_voice, flight, quick_mission, radio_calls, terrain,
};
use std::collections::BTreeMap;
use tore_sim::models::FlightModel;

/// The whole mission. The app drives it and presents it.
pub struct World {
    /// What the flight is built from; a restart rebuilds it from this.
    pub setup: Setup,
    /// Terrain queries, the airport scene and the weather clock. Until the
    /// crate move splits it, it also holds the scenery the renderer draws.
    pub terrain: terrain::Terrain,
    /// The player's flight state.
    pub flight: flight::State,
    /// The player's flight state at the start of the last tick.
    pub previous_flight: flight::State,
    pub combat: combat::Combat,
    pub ai_wings: Option<ai_wings::AiWings>,
    pub airport_service: tore_sim::airport::Service,
    pub airport_nav_mode: bool,
    pub turbulence: tore_sim::turbulence::Turbulence,
    pub turbulence_rng: tore_formats::flight_model::clock_rng::NativeRng,
    /// Radio and crew voice delivery; see docs/spec/radio-chatter.md.
    pub comms: comms::Comms,
    pub airfield_radio: airfield_radio::AirfieldRadio,
    /// Weapon, hit, kill and wing radio calls; see radio_calls.rs.
    pub radio: radio_calls::Radio,
    /// Imported phrase text for composing radio lines.
    pub phrases: comms::Phrases,
    /// The player's crew voice; see docs/spec/cockpit-voice.md.
    pub crew_voice: crew_voice::CrewVoice,
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
    pub layout: Option<quick_mission::MissionLayout>,
    /// How many AI aircraft fly, when the AI flies the mission.
    pub ai_aircraft: Option<usize>,
}

/// A queued airport command: the navigation mode switch or a tower request.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AirportInput {
    NavMode,
    Command(tore_sim::airport::Command),
}

/// Everything the app hands one tick.
#[derive(Clone, Debug, Default)]
pub struct TickInput {
    /// The player's stick, throttle and pilot commands for this tick.
    pub pilot: flight::PilotInput,
    /// The bound fire control is held.
    pub fire: bool,
    /// Weapon-page buttons since the last tick, in order; true steps forward.
    pub weapon_cycles: Vec<bool>,
    /// Queued airport commands, navigation-page selections included, in order.
    pub airport: Vec<AirportInput>,
    /// The player's crew, which labels radio calls to the cockpit.
    pub crew: Option<comms::Crew>,
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
    /// A radio or crew line due now.
    Radio(comms::Call),
}

/// What one tick produced. Every queue inside the world that the tick fills is
/// drained into it, whether or not anyone reads it.
#[derive(Default)]
pub struct TickOutput {
    pub cues: Vec<Cue>,
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
    /// Restarts the weather clock, turbulence and the camera weather from the
    /// launch conditions and their fixed seeds.
    pub fn reset_weather(&mut self) {
        self.terrain.weather_presentation = tore_sim::environment::Presentation::seeded(1)
            .expect("fixed valid weather presentation seed");
        self.terrain.auxiliary_presentations =
            std::array::from_fn(|_| self.terrain.weather_presentation.clone());
        self.terrain.weather =
            tore_sim::environment::Environment::new(self.terrain.weather.configuration().clone());
        self.turbulence = Default::default();
        self.turbulence_rng.reseed_word(1);
    }

    /// Rebuilds the flight from its setup: the radio, the weather, the
    /// player's start, combat, the airport service and the AI wings, in the
    /// order a flight has always started. The app ends the old flight's
    /// recording before and starts the new one after.
    pub fn restart(
        &mut self,
        aircraft: &aircraft::Airframe,
        resources: &BTreeMap<String, Vec<u8>>,
    ) -> AppResult<Restarted> {
        // A fixed seed keeps headless runs deterministic.
        self.comms.restart(1);
        self.crew_voice = crew_voice::CrewVoice::new(&aircraft.profile);
        self.radio = Default::default();
        self.reset_weather();
        self.flight = aircraft.start(&self.terrain);
        if let Some((altitude, fuel)) = self.setup.mission {
            self.flight.position[1] = altitude;
            self.flight.fuel = fuel;
        }
        // The accepted creator layout, reused unchanged on restart.
        let layout = self
            .setup
            .mission
            .and(self.combat.mission_layout.clone())
            .filter(|layout| layout.ground.is_some() == self.setup.ground_start.is_some());
        let parked = layout.as_ref().and_then(|layout| layout.ground.clone());
        self.airfield_radio.reset(parked.as_ref().map(|g| g.runway));
        if let Some(layout) = layout.as_ref().filter(|l| l.player_turn != 0.) {
            // Airborne: the whole scene turns so the enemy ahead stays on the
            // map.
            self.flight.yaw += layout.player_turn;
            let basis = attitude::Basis::new(self.flight.yaw, 0., 0.);
            self.flight.velocity = std::array::from_fn(|i| {
                basis.forward[i] * self.flight.speed + self.terrain.wind()[i]
            });
        }
        if let Some(object) = self.setup.ground_start {
            let (position, heading) = match &parked {
                Some(ground) => (ground.slots[0], ground.heading),
                None => quick_mission::runway_pose(&self.terrain, object)?,
            };
            self.flight.position[0] = position[0];
            self.flight.position[2] = position[2];
            if self.setup.mission.is_none() {
                self.flight.position[1] = self.flight.position[1].max(position[1] + 5000.);
            }
            self.flight.yaw = heading;
            let basis = attitude::Basis::new(heading, 0., 0.);
            self.flight.velocity = std::array::from_fn(|i| {
                basis.forward[i] * self.flight.speed + self.terrain.wind()[i]
            });
        }
        if self.setup.researched_flight {
            self.flight.enable_research(1)?;
        }
        if let Some(tables) = &self.setup.native_tables {
            if self.combat.range || self.setup.mission.is_some() {
                return Err("native research flight currently requires clean free flight".into());
            }
            self.flight.enable_native(tables.clone(), 1)?;
        }
        self.combat.reset(&mut self.flight)?;
        self.combat.raise_airborne_spawns(&self.terrain);
        if self.combat.uses_normal_startup_defaults() {
            self.combat.apply_startup_weapons();
        }
        let ground_airport = match self.setup.ground_start {
            Some(object) => Some(match &parked {
                Some(ground) => {
                    quick_mission::place_on_runway(&self.terrain, &mut self.flight, ground, 0)
                        .map(|()| ground.airport)?
                }
                None => quick_mission::apply_ground_start(&self.terrain, &mut self.flight, object)?,
            }),
            None => None,
        };
        self.airport_service
            .reset(&self.terrain.airport_scene)
            .map_err(std::io::Error::other)?;
        // A ground start begins on NAV, and so does an aircraft with nothing
        // loaded in the selected station: an empty station is never armed.
        self.airport_nav_mode =
            ground_airport.is_some() || !self.combat.state.carries(self.combat.state.selected);
        self.combat.state.armed = !self.airport_nav_mode;
        if let Some(airport) = ground_airport {
            self.airport_service.command(
                &self.terrain.airport_scene,
                airport_aircraft(&self.terrain, &self.flight, self.airport_nav_mode),
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
                parked.as_ref().map(quick_mission::GroundLayout::departure),
            );
            let mut bridge = ai_wings::AiWings::build_mission(
                &ai.wings,
                &self.combat.state.targets,
                ai.guns_only,
                resources,
                &airfields,
            )?;
            bridge.apply_mission_preset(ai.preset, self.flight.position);
            bridge.apply_group_objectives(&ai.group_objectives, self.flight.position);
            bridge.apply_group_survival(&ai.group_must_survive);
            bridge.mirror_pose_out(&mut self.combat.state.targets);
            self.combat.ai_poses = !bridge.is_empty();
            ai_aircraft = Some(bridge.len());
            self.ai_wings = Some(bridge);
        }
        // Draw from the placed start, including the AI's own poses.
        self.combat
            .restart_render(&self.flight, self.ai_wings.as_ref());
        self.previous_flight = self.flight.clone();
        self.overspeed_message_at = None;
        self.edge_message_at = None;
        Ok(Restarted {
            ground_airport,
            layout,
            ai_aircraft,
        })
    }

    /// The simulation half of the weapon selector: step the player's weapon
    /// selection. The navigation mode follows the arming.
    pub fn cycle_weapon(&mut self, forward: bool) {
        self.combat.cancel();
        self.combat.command(
            if forward {
                tore_sim::combat::live::Command::NextSelection
            } else {
                tore_sim::combat::live::Command::PreviousSelection
            },
            combat::launcher(&self.flight),
        );
        self.airport_nav_mode = !self.combat.state.armed;
    }

    /// One fixed 120 Hz tick of the whole mission. See the module
    /// documentation; the order below is the order the redraw loop ran.
    pub fn step(&mut self, input: &TickInput, out: &mut TickOutput) -> AppResult<()> {
        *out = TickOutput::default();
        for &forward in &input.weapon_cycles {
            self.cycle_weapon(forward);
            out.cues.push(Cue::WeaponCycled);
        }
        for &command in &input.airport {
            self.airport_command(command, out);
        }
        self.previous_flight.clone_from(&self.flight);
        self.flight
            .step_surface(&input.pilot, |x, z| self.terrain.surface(x, z));
        if self.flight.native.is_none()
            && self
                .terrain
                .solid_contact(
                    self.previous_flight.position,
                    self.flight.position,
                    self.combat
                        .state
                        .targets
                        .iter()
                        .filter(|target| target.hp > 0)
                        .map(|target| target.id),
                )
                .is_some()
        {
            if self.flight.cheats.no_crashes {
                self.flight.rebound(self.previous_flight.position);
            } else {
                self.flight.crashed = true;
            }
        }
        if let Some(error) = self.flight.native_fault() {
            out.fault = Some(error.to_owned());
            return Ok(());
        }
        // Weather shares the authoritative tick; pausing simply stops calling
        // it, with no elapsed-time catch-up.
        self.terrain.weather.step();
        let turbulence = !self.flight.cheats.no_turbulence;
        let turbulence_cue = step_turbulence(
            &mut self.turbulence,
            &mut self.turbulence_rng,
            &mut self.flight,
            &self.terrain,
            turbulence,
        );
        self.edge_and_overspeed(out);
        if let Some(cue) = turbulence_cue {
            out.cues.push(Cue::Feedback(cue));
        }
        out.cues.push(Cue::Flown);
        self.combat.controller.space(input.fire, false, false);
        if let Some(recorder) = &mut self.combat.recorder {
            let airport = airport_aircraft(&self.terrain, &self.flight, self.airport_nav_mode);
            recorder.record(
                &format!(
                    "airport-state:{}:{}:{}",
                    u8::from(airport.nav_mode),
                    u8::from(airport.gear_down),
                    u8::from(airport.supported)
                ),
                combat::launcher(&self.flight),
            );
        }
        let events = self.combat.step(&mut self.flight, &self.terrain)?;
        out.cues.push(Cue::CombatStepped);
        for airport_event in self.airport_service.synchronize_health(
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
        for event in self.airport_service.step(
            &self.terrain.airport_scene,
            airport_aircraft(&self.terrain, &self.flight, self.airport_nav_mode),
        ) {
            if matches!(event, tore_sim::airport::Event::LandingComplete { .. }) {
                out.cues.push(Cue::Message("Landing complete".into()));
            }
        }
        // Manual p.65: the player always lands first and other aircraft hold at
        // marshal. The retail condition (gear, height, speed and range) is
        // re-evaluated every tick, so climbing away, raising the gear, a crash
        // or restart release it.
        if let Some(wings) = &mut self.ai_wings {
            let [x, _, z] = self.flight.position;
            wings.update_player_landing(
                &self.terrain.airport_scene,
                &self.airport_service,
                &self.flight,
                self.terrain.surface(x, z).height,
            );
        }
        for message in self.flight.systems.messages.drain(..) {
            out.cues.push(Cue::Message(message));
        }
        for event in &events {
            use tore_sim::combat::live::Event;
            if let Some(cue) = combat::feedback(event, self.combat.state.configuration()) {
                out.cues.push(Cue::Feedback(cue));
            }
            match event {
                Event::Jolt(jolt) => match jolt.target {
                    None => self.flight.jolt_from(jolt.from, jolt.strength),
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
                    self.flight.crashed = true;
                }
                Event::Fired(i) => {
                    if let Some(name) = self.combat.state.configuration().stations[*i]
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
            let stepped = bridge.step(&mut self.combat.state, &self.flight, &self.terrain);
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
            .advance_render(&self.flight, self.ai_wings.as_ref());
        out.outcomes = self.combat.state.ledger.take_outcomes();
        // The AI's messages of this tick; the journal is write-only, so
        // draining it changes nothing.
        out.journal = self
            .ai_wings
            .as_mut()
            .map(ai_wings::AiWings::take_ai_journal);
        out.cues.push(Cue::Picture);
        self.airfield_radio.step(
            self.combat.state.tick() as f64 / 120.,
            &self.phrases,
            &mut self.comms,
            &self.flight,
            &self.terrain,
            &self.airport_service,
            self.ai_wings.as_ref(),
        );
        self.crew_voice.step_host(
            &mut self.comms,
            &self.phrases,
            &crew_voice::Host {
                flight: &self.flight,
                combat: &self.combat.state,
                wings: self.ai_wings.as_ref(),
                world: &self.terrain,
            },
        );
        radio_calls::step(
            &mut self.radio,
            &mut self.comms,
            &self.phrases,
            input.crew,
            &events,
            &mut self.combat.state,
            self.ai_wings.as_mut(),
            &self.flight,
        );
        for call in self.comms.due(self.combat.state.tick() as f64 / 120.) {
            out.cues.push(Cue::Radio(call));
        }
        out.emissions = self.combat.state.take_sound_events();
        out.events = events;
        Ok(())
    }

    /// Beyond the map: a turn-back warning every ten seconds from 100
    /// nautical miles out, and the aircraft is lost at 105. Past the top
    /// speed: a short cockpit message, repeated every four seconds. Both
    /// requested by John, 2026-09-29; see docs/spec/world-edge.md and
    /// docs/spec/overspeed.md.
    fn edge_and_overspeed(&mut self, out: &mut TickOutput) {
        if !self.flight.crashed {
            let [x, _, z] = self.flight.position;
            let out_nm = self.terrain.edge_distance_nm(x, z);
            if out_nm >= terrain::EDGE_DESTROY_NM {
                self.flight
                    .systems
                    .destroy(tore_sim::aircraft_systems::LossCause::OutOfBounds);
                self.flight.crashed = true;
            } else if out_nm >= terrain::EDGE_WARNING_NM {
                let now = self.flight.ticks as f64 * flight::DT;
                if self
                    .edge_message_at
                    .is_none_or(|at| now - at >= 10. || now < at)
                {
                    self.edge_message_at = Some(now);
                    out.cues.push(Cue::Message(
                        "You have left the theater: turn back now".into(),
                    ));
                }
            } else {
                self.edge_message_at = None;
            }
        }
        if !self.flight.crashed
            && self
                .flight
                .overspeed_ratio()
                .is_some_and(|r| r >= flight::OVERSPEED_SHAKE_FULL)
        {
            let now = self.flight.ticks as f64 * flight::DT;
            if self
                .overspeed_message_at
                .is_none_or(|at| now - at >= 4. || now < at)
            {
                self.overspeed_message_at = Some(now);
                out.cues.push(Cue::Message("OVERSPEED".into()));
            }
        }
    }

    /// A queued NAV mode switch or tower request, applied at the start of the
    /// tick in the order it was given.
    fn airport_command(&mut self, command: AirportInput, out: &mut TickOutput) {
        match command {
            AirportInput::NavMode => {
                self.airport_nav_mode = !self.airport_nav_mode;
                self.combat.cancel();
                self.combat.command(
                    if self.airport_nav_mode {
                        tore_sim::combat::live::Command::SelectNav
                    } else {
                        tore_sim::combat::live::Command::NextSelection
                    },
                    combat::launcher(&self.flight),
                );
                if let Some(recorder) = &mut self.combat.recorder {
                    recorder.record(
                        if self.airport_nav_mode {
                            "airport-nav:1"
                        } else {
                            "airport-nav:0"
                        },
                        combat::launcher(&self.flight),
                    );
                }
                out.cues.push(Cue::Message(
                    if self.airport_nav_mode {
                        "Navigation mode selected"
                    } else {
                        "Navigation mode off"
                    }
                    .into(),
                ));
            }
            AirportInput::Command(command) => {
                if let Some(recorder) = &mut self.combat.recorder {
                    recorder.record(
                        &combat_tape::airport_command_name(command),
                        combat::launcher(&self.flight),
                    );
                }
                let aircraft = airport_aircraft(&self.terrain, &self.flight, self.airport_nav_mode);
                for event in
                    self.airport_service
                        .command(&self.terrain.airport_scene, aircraft, command)
                {
                    if let tore_sim::airport::Event::Reply(reply) = event {
                        self.airfield_radio.reply(&reply);
                        self.comms.cancel_airport();
                        self.comms.spoken(self.combat.state.tick() as f64 / 120.);
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
