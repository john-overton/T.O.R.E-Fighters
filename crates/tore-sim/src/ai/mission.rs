//! Per-actor AI runtime (AI-5): actor-owned sensors, stores and movement.
//!
//! This module is the seam between the decision components and the rest of the
//! simulation. Each [`AiActor`] owns its own [`flight::State`], its own
//! [`Sensors`], its own stores and its own [`Controller`]. Nothing here reads
//! the player's combat state, the player's sensors or any shared global, and
//! motion goes exclusively through the actor's own flight model using the
//! inputs from [`steering_adapter`](super::steering_adapter).
//!
//! **Missile physics are not duplicated.** AI-5's exit evidence forbids it. A
//! weapon release produces a [`LaunchEvent`] after this module has debited the
//! actor's own ammunition through
//! [`weapon_service::release`](super::weapon_service::release); the host's
//! existing combat layer creates and flies the projectile with the same code
//! the player's shots use. In the other direction the host reports launches
//! back as [`controller::ThreatReport`]s, which is what B47 warnings are built
//! from. Free ammunition is therefore impossible here: the debit happens
//! before the event is emitted, and an empty or inhibited store refuses.
//!
//! Frames are built from one world snapshot per tick. An actor's permitted
//! targets are the objects its own sensors can see, never the whole world, so
//! one actor cannot acquire a contact another actor earned.

use tore_formats::aircraft::AircraftId;
use tore_input::PilotInput;

use crate::flight;
use crate::models::FlightModel;
use crate::research::Surface;
use crate::sensors::{self, Observable, Observer, Sensors};

use super::controller::{
    Activity, ActorIdentity, BehaviorProfile, Controller, DecisionFrame, FrameEvent, IntentBatch,
    LeaderView, MotionIntent, OwnState, RouteView, StationView, TargetView, ThreatReport, WingView,
};
use super::experience::ResolvedExperience;
use super::fitted::Fallback;
use super::steering_adapter::{ControlAdapter, ai_g_limits};
use super::threat::{DispenserStore, FlightState, SeekerClass, TimeOfDay};
use super::weapon_service::{
    self, Delay, ProjectilePacing, Rounds, StationId, StoreCapability, StoreState,
};
use super::wing::{Formation, WingControl};
use super::{Result, ScalarSpeed, SpeedLimits};

/// One carried store on an AI aircraft.
///
/// This is the actor's own inventory. It is never shared with the player's
/// stores and never refilled by this module.
#[derive(Clone, Debug, PartialEq)]
pub struct StationSpec {
    pub station: StationId,
    pub guided: bool,
    pub capability: StoreCapability,
    pub store: StoreState,
    /// Rounds debited per release (B45 actual-rounds-per-game-round).
    pub debit: u32,
    /// External mass per remaining round, using the shared live payload convention.
    pub external_round_lbs: f64,
    /// Projectiles one release creates; pod and burst metadata can make this
    /// differ from the ammunition debit (B45).
    pub projectile_count: u32,
    /// The employment envelope's angular limit, when it has one.
    pub employment_limit_deg: Option<f64>,
    pub damage_vs_category: f64,
    pub store_speed: ScalarSpeed,
    pub tracking_delay: Delay,
    pub pacing: ProjectilePacing,
    /// Maximum employment range, feet. `None` leaves the bound unrestricted.
    pub maximum_range_ft: Option<f64>,
    pub minimum_range_ft: f64,
    pub requires_radar: bool,
    pub requires_sensor: bool,
    pub employment_zone: Option<tore_formats::weapons::Zone>,
    pub mount: [f64; 3],
}

impl StationSpec {
    pub fn rounds(&self) -> Rounds {
        self.store.rounds
    }

    pub fn is_empty(&self) -> bool {
        matches!(self.store.rounds, Rounds::Finite(0))
    }
}

/// One object in the world, as the mission sees it.
///
/// The host builds these once per tick for every participant, the player
/// included. The player appears here as an ordinary object with
/// `human_controlled` set; it is never a special case in the decision path.
#[derive(Clone, Debug, PartialEq)]
pub struct WorldObject {
    pub id: u32,
    pub side: super::targeting::Side,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub heading_deg: f64,
    pub pitch_deg: f64,
    pub speed: ScalarSpeed,
    pub maximum_speed: ScalarSpeed,
    pub is_aircraft: bool,
    pub is_fighter: bool,
    pub human_controlled: bool,
    pub alive: bool,
    pub destroyed: bool,
    /// Sensor input for the actors that can see it. `None` means the object is
    /// not presented to the sensor component at all.
    pub observable: Option<Observable>,
}

/// A weapon release this tick, for the host's existing combat layer to realise.
///
/// The ammunition has already been debited from the firing actor's own store,
/// so the host must not debit again. `projectiles` is what the host should
/// create; it is not necessarily the ammunition debit (B45).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaunchEvent {
    pub actor: u32,
    pub station: StationId,
    pub target: u32,
    pub request_id: weapon_service::RequestId,
    pub projectiles: u32,
}

/// A countermeasure release this tick (B47).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceEvent {
    pub actor: u32,
    pub class: SeekerClass,
    /// Devices actually released after the dispenser debit, which stops as
    /// soon as the matching dispenser is empty.
    pub released: u8,
}

/// What one tick of the mission produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MissionOutput {
    pub launches: Vec<LaunchEvent>,
    pub devices: Vec<DeviceEvent>,
    /// Player-readable activity per actor, for a radio or status display.
    pub activities: Vec<(u32, Activity)>,
    /// Every fitted fallback applied this tick, by actor.
    pub fallbacks: Vec<(u32, Fallback)>,
    pub wing: Vec<(u32, super::wing::WingRequest)>,
}

/// Everything needed to build one AI aircraft.
pub struct ActorSetup {
    pub identity: ActorIdentity,
    pub profile: BehaviorProfile,
    pub experience: ResolvedExperience,
    pub seed: u64,
    pub flight: flight::State,
    /// The actor's own sensor component. `None` means the host supplies the
    /// permitted target list itself, which is the headless fixture path.
    pub sensors: Option<Sensors>,
    pub stations: Vec<StationSpec>,
    pub dispensers: Vec<DispenserStore>,
    pub wing_slot: u8,
    pub home_airport: Option<super::route::Position>,
}

/// One AI-flown aircraft and everything it owns.
pub struct AiActor {
    identity: ActorIdentity,
    controller: Controller,
    adapter: ControlAdapter,
    flight: flight::State,
    sensors: Option<Sensors>,
    stations: Vec<StationSpec>,
    dispensers: Vec<DispenserStore>,
    wing_slot: u8,
    home_airport: Option<super::route::Position>,
    pending_threats: Vec<ThreatReport>,
    pending_events: Vec<FrameEvent>,
    device_schedule: Vec<(u64, SeekerClass, u8)>,
    activity: Activity,
    last_input: PilotInput,
    alive: bool,
}

impl AiActor {
    pub fn new(setup: ActorSetup) -> Result<Self> {
        let controller =
            Controller::new(setup.identity, setup.profile, setup.experience, setup.seed)?;
        Ok(Self {
            identity: setup.identity,
            controller,
            adapter: ControlAdapter::new(),
            flight: setup.flight,
            sensors: setup.sensors,
            stations: setup.stations,
            dispensers: setup.dispensers,
            wing_slot: setup.wing_slot,
            home_airport: setup.home_airport,
            pending_threats: Vec::new(),
            pending_events: Vec::new(),
            device_schedule: Vec::new(),
            activity: Activity::Idle,
            last_input: PilotInput::default(),
            alive: true,
        })
    }

    pub fn id(&self) -> u32 {
        self.identity.actor.0
    }

    pub fn identity(&self) -> &ActorIdentity {
        &self.identity
    }

    pub fn flight(&self) -> &flight::State {
        &self.flight
    }

    pub fn flight_mut(&mut self) -> &mut flight::State {
        &mut self.flight
    }

    pub fn controller(&self) -> &Controller {
        &self.controller
    }

    pub fn sensors(&self) -> Option<&Sensors> {
        self.sensors.as_ref()
    }

    pub fn stations(&self) -> &[StationSpec] {
        &self.stations
    }

    pub fn set_stations(&mut self, stations: Vec<StationSpec>) {
        self.stations = stations;
    }

    pub fn set_dispensers(&mut self, dispensers: Vec<DispenserStore>) {
        self.dispensers = dispensers;
    }

    pub fn dispensers(&self) -> &[DispenserStore] {
        &self.dispensers
    }

    pub fn activity(&self) -> Activity {
        self.activity
    }

    pub fn alive(&self) -> bool {
        self.alive && !self.flight.crashed
    }

    pub fn last_input(&self) -> &PilotInput {
        &self.last_input
    }

    /// Total rounds remaining across every station, for a no-free-ammunition
    /// check in tests and reports.
    pub fn rounds_remaining(&self) -> u32 {
        self.stations
            .iter()
            .map(|s| match s.store.rounds {
                Rounds::Finite(n) => n,
                Rounds::Unlimited => 0,
            })
            .sum()
    }

    /// Report a missile launched at this actor. Only the aircraft the missile
    /// was fired at is ever told (B47); the host must not broadcast.
    pub fn report_threat(&mut self, report: ThreatReport) {
        self.pending_threats.push(report);
    }

    /// Report that this actor was hit.
    pub fn report_hit(&mut self) {
        self.pending_events.push(FrameEvent::Hit);
    }

    /// Report that an object left the world.
    pub fn report_removed(&mut self, id: u32) {
        self.pending_events.push(FrameEvent::ActorRemoved(id));
    }

    pub fn set_alive(&mut self, alive: bool) {
        self.alive = alive;
    }

    /// Deliver one wing command to this actor (B46).
    ///
    /// The four outcomes stay distinct, so a caller can tell an applied
    /// setting from a rejection and from motion actually installed.
    pub fn order(
        &mut self,
        request: super::wing::WingRequest,
        tick: u64,
    ) -> Result<super::wing::ReceiverOutcome> {
        self.controller
            .prepare_order(self.flight.yaw.to_degrees(), self.speed_limits());
        self.controller.receive_order(request, tick)
    }

    /// Set this actor's remaining internal fuel, in pounds.
    ///
    /// Used by the host when fuel is tracked outside the flight model, and by
    /// fixtures that need a specific B48 fuel state.
    pub fn set_internal_fuel(&mut self, pounds: f64) {
        self.flight.fuel = pounds;
    }
}

/// The AI side of a live mission: every AI-flown aircraft, stepped together.
///
/// The player is not an actor here. It enters as a [`WorldObject`] like any
/// other participant, so no player state can contaminate an AI decision.
pub struct AiMission {
    actors: Vec<AiActor>,
    tick: u64,
    wing_control: WingControl,
    formation: Formation,
    horizontal_spacing_ft: i32,
    vertical_spacing_ft: i32,
    external_leaders: Vec<(super::targeting::Side, u8, u32)>,
}

impl Default for AiMission {
    fn default() -> Self {
        Self::new()
    }
}

impl AiMission {
    pub fn new() -> Self {
        Self {
            actors: Vec::new(),
            tick: 0,
            wing_control: WingControl::Loose,
            formation: Formation::Echelon,
            horizontal_spacing_ft: super::wing::PLAYER_SPACING_SPREAD_FT,
            vertical_spacing_ft: super::wing::PLAYER_STACKING_FT,
            external_leaders: Vec::new(),
        }
    }

    pub fn push(&mut self, actor: AiActor) {
        self.actors.push(actor);
    }

    /// A human leader remains a world object, never an AI-controlled actor.
    pub fn set_external_leader(&mut self, side: super::targeting::Side, wing: u8, id: u32) {
        self.external_leaders
            .retain(|(s, w, _)| *s != side || *w != wing);
        self.external_leaders.push((side, wing, id));
    }

    pub fn actors(&self) -> &[AiActor] {
        &self.actors
    }

    pub fn actors_mut(&mut self) -> &mut [AiActor] {
        &mut self.actors
    }

    pub fn actor(&self, id: u32) -> Option<&AiActor> {
        self.actors.iter().find(|a| a.id() == id)
    }

    pub fn actor_mut(&mut self, id: u32) -> Option<&mut AiActor> {
        self.actors.iter_mut().find(|a| a.id() == id)
    }

    pub fn tick(&self) -> u64 {
        self.tick
    }

    pub fn is_empty(&self) -> bool {
        self.actors.is_empty()
    }

    pub fn len(&self) -> usize {
        self.actors.len()
    }

    /// Advance every AI aircraft by one 120 Hz tick.
    ///
    /// `world` is one snapshot of every participant including the player and
    /// including the actors themselves; an actor never sees itself as a
    /// target. `ground` is the terrain height query. The mission steps each
    /// actor's own sensors, runs its controller, converts the resulting motion
    /// through the steering adapter and steps that actor's own flight model.
    pub fn step(
        &mut self,
        world: &[WorldObject],
        ground: &dyn Fn(f64, f64) -> f64,
        now: TimeOfDay,
    ) -> Result<MissionOutput> {
        let mut output = MissionOutput::default();
        let tick = self.tick;

        let traffic: Vec<_> = world
            .iter()
            .filter(|o| o.alive && !o.destroyed && o.is_aircraft)
            .map(|o| super::formation::Traffic {
                id: o.id,
                position: o.position,
                velocity: o.velocity,
                phase: self
                    .actors
                    .iter()
                    .find(|a| a.id() == o.id)
                    .and_then(|a| a.controller().formation_trace())
                    .map(|t| t.phase),
            })
            .collect();
        for index in 0..self.actors.len() {
            self.step_actor(index, world, &traffic, ground, now, tick, &mut output)?;
        }

        // Deliver after all actors have decided, so iteration order cannot
        // change which wing members observe a new command this tick.
        for (sender, request) in &output.wing {
            if let Some(actor) = self.actor(*sender).filter(|a| a.alive()) {
                let identity = *actor.identity();
                self.order_wing(identity.side, identity.wing, Some(*sender), *request)?;
            }
        }
        self.tick += 1;
        Ok(output)
    }

    #[allow(clippy::too_many_arguments)]
    fn step_actor(
        &mut self,
        index: usize,
        world: &[WorldObject],
        traffic: &[super::formation::Traffic],
        ground: &dyn Fn(f64, f64) -> f64,
        now: TimeOfDay,
        tick: u64,
        output: &mut MissionOutput,
    ) -> Result<()> {
        let actor_id = self.actors[index].id();
        let leader = self.leader_view(index, world);

        let identity = self.actors[index].identity;
        let assignments: Vec<u32> = self
            .actors
            .iter()
            .filter(|a| {
                a.alive()
                    && a.id() != actor_id
                    && a.identity.side == identity.side
                    && a.identity.wing == identity.wing
            })
            .filter_map(|a| a.controller.target())
            .collect();
        let actor = &mut self.actors[index];
        actor.controller.set_formation_observation(traffic.to_vec());
        if !actor.alive() {
            actor.activity = Activity::Destroyed;
            output.activities.push((actor_id, Activity::Destroyed));
            return Ok(());
        }

        // 1. The actor's own sensors, stepped with the actor as observer.
        let permitted = actor.observe(world, ground);

        // 2. Own state from the actor's own flight model.
        let own = actor.own_state(ground);

        // 3. The frame.
        let events = actor.drain_events(tick);
        let mut targets = actor.target_views(&permitted, world);
        for target in &mut targets {
            target.wing_attackers =
                assignments.iter().filter(|id| **id == target.id).count() as u32;
            target.terrain_blocked =
                crate::combat::live::terrain_hit(own.position, target.position, &|x, z| {
                    ground(x, z)
                })
                .is_some();
        }
        let stations = actor.station_views(&targets, &own);
        let wing = WingView {
            control: self.wing_control,
            formation: self.formation,
            horizontal_spacing_ft: self.horizontal_spacing_ft,
            vertical_spacing_ft: self.vertical_spacing_ft,
            slot: actor.wing_slot,
            leader,
            wingmen_in_formation: 0,
            wing_combat: !targets.is_empty(),
            wing_approach: false,
            // B12's wing-approach value has no recovered producer, so the
            // wing-split branch stays untried rather than being fed ordinary
            // target distance.
            wing_approach_value_ft: None,
        };
        let route = RouteView {
            home_airport: actor.home_airport,
            leader_is_ai: true,
        };
        let batch = {
            let frame = DecisionFrame {
                tick,
                own,
                targets: &targets,
                events: &events,
                stations: &stations,
                dispensers: &actor.dispensers,
                wing,
                route,
                now,
                flight_state: if own.on_ground {
                    FlightState::TakingOff
                } else {
                    FlightState::Free
                },
            };
            actor.controller.step(&frame)?
        };

        if let Some(sensors) = &mut actor.sensors
            && let Some(target) = batch.sensor.designate
        {
            sensors.designate(target);
        }
        output
            .wing
            .extend(batch.wing.iter().map(|request| (actor_id, *request)));
        for fallback in &batch.fallbacks {
            output.fallbacks.push((actor_id, *fallback));
        }
        if let Some(activity) = batch.activity {
            actor.activity = activity;
            output.activities.push((actor_id, activity));
        }

        // 4. Weapons: debit this actor's own store, then emit the event.
        for intent in &batch.weapons {
            if let Some(event) = actor.release(intent, &batch) {
                output.launches.push(event);
            }
        }

        // B47: debit each device only when its quarter-second release is due.
        if let Some(devices) = batch.devices {
            actor
                .device_schedule
                .push((tick, devices.class, devices.count));
        }
        let schedule = std::mem::take(&mut actor.device_schedule);
        for (due, class, remaining) in schedule {
            if tick < due {
                actor.device_schedule.push((due, class, remaining));
            } else if let Some(event) = actor.release_devices(class, 1) {
                output.devices.push(event);
                if remaining > 1 {
                    actor.device_schedule.push((
                        due + super::QUARTER_SECOND_TICKS,
                        class,
                        remaining - 1,
                    ));
                }
            }
        }

        // 6. Motion through the adapter and this actor's own flight model.
        actor.fly(batch.motion.as_ref(), &own, ground)?;
        Ok(())
    }

    /// The leader's pose for a wingman, taken from the leader actor itself.
    fn leader_view(&self, index: usize, world: &[WorldObject]) -> Option<LeaderView> {
        let actor = &self.actors[index];
        if actor.identity.is_leader() {
            return None;
        }
        if let Some((_, _, id)) = self
            .external_leaders
            .iter()
            .find(|(side, wing, _)| *side == actor.identity.side && *wing == actor.identity.wing)
        {
            let leader = world
                .iter()
                .find(|o| o.id == *id && o.alive && !o.destroyed)?;
            return Some(LeaderView {
                position: leader.position,
                velocity: leader.velocity,
                heading_deg: leader.heading_deg,
                speed: leader.speed,
                target: None,
                recovering: false,
            });
        }
        let leader = self.actors.iter().find(|a| {
            a.identity.is_leader()
                && a.identity.side == actor.identity.side
                && a.identity.wing == actor.identity.wing
                && a.alive()
        })?;
        let pose = world
            .iter()
            .find(|o| o.id == leader.id() && o.alive && !o.destroyed)?;
        Some(LeaderView {
            position: pose.position,
            velocity: pose.velocity,
            heading_deg: pose.heading_deg,
            speed: pose.speed,
            target: leader.controller.target(),
            recovering: matches!(leader.activity, Activity::ReturningToBase),
        })
    }

    /// Deliver one wing command to one actor (B46).
    pub fn order(
        &mut self,
        actor: u32,
        request: super::wing::WingRequest,
    ) -> Option<Result<super::wing::ReceiverOutcome>> {
        let tick = self.tick;
        self.actor_mut(actor).map(|a| a.order(request, tick))
    }

    pub fn track_ordered_approach(&mut self, actor: u32, target: u32, heading: f64, pitch: f64) {
        if let Some(actor) = self.actor_mut(actor) {
            actor
                .controller
                .track_ordered_approach(target, heading, pitch);
        }
    }

    /// A command stays inside the identified side and wing.
    pub fn order_wing(
        &mut self,
        side: super::targeting::Side,
        wing: u8,
        sender: Option<u32>,
        request: super::wing::WingRequest,
    ) -> Result<usize> {
        Ok(self
            .order_wing_report(side, wing, sender, None, request)?
            .len())
    }

    /// Per-recipient outcomes, including no-motion and rejection. A directed
    /// recipient never bypasses the side/wing boundary.
    pub fn order_wing_report(
        &mut self,
        side: super::targeting::Side,
        wing: u8,
        sender: Option<u32>,
        recipient: Option<u32>,
        request: super::wing::WingRequest,
    ) -> Result<Vec<(u32, super::wing::ReceiverOutcome)>> {
        if sender.is_some_and(|id| {
            !self
                .actor(id)
                .is_some_and(|a| a.alive() && a.identity.side == side && a.identity.wing == wing)
        }) {
            return Err(super::AiError::InvalidInput(
                "sender does not belong to live wing",
            ));
        }
        let mut recipients = Vec::new();
        for actor in &self.actors {
            if actor.alive()
                && actor.identity.side == side
                && actor.identity.wing == wing
                && recipient.is_none_or(|id| id == actor.id())
                && (Some(actor.id()) != sender
                    || matches!(
                        request,
                        super::wing::WingRequest::Spacing { .. }
                            | super::wing::WingRequest::FormationSelection(_)
                            | super::wing::WingRequest::WingControl(_)
                    ))
            {
                recipients.push(actor.id());
            }
        }
        let mut outcomes = Vec::with_capacity(recipients.len());
        for id in recipients {
            outcomes.push((id, self.order(id, request).expect("validated recipient")?));
        }
        Ok(outcomes)
    }

    pub fn set_formation(&mut self, formation: Formation) {
        self.formation = formation;
    }

    pub fn set_wing_control(&mut self, control: WingControl) {
        self.wing_control = control;
    }

    pub fn set_spacing(&mut self, horizontal_ft: i32, vertical_ft: i32) {
        self.horizontal_spacing_ft = super::wing::request_spacing(i64::from(horizontal_ft));
        self.vertical_spacing_ft = vertical_ft;
    }
}

impl AiActor {
    /// Step this actor's own sensors and return the ids it may engage.
    ///
    /// With a sensor component, only the actor's own contacts are permitted,
    /// so nothing becomes a target merely by existing in the world. Without
    /// one, the host's world list is the permitted list, which is the headless
    /// fixture path and is documented as such.
    fn observe(&mut self, world: &[WorldObject], ground: &dyn Fn(f64, f64) -> f64) -> Vec<u32> {
        let Some(sensors) = self.sensors.as_mut() else {
            return world
                .iter()
                .filter(|o| o.id != self.identity.actor.0)
                .map(|o| o.id)
                .collect();
        };
        let observer = Observer {
            position: self.flight.position,
            basis: crate::attitude::Basis::new(
                self.flight.yaw,
                self.flight.pitch,
                self.flight.bank,
            ),
            radar_powered: self.flight.radar,
            radar_failed: false,
            infrared_failed: false,
            visual_failed: false,
        };
        let observables: Vec<Observable> = world
            .iter()
            .filter(|o| o.id != self.identity.actor.0)
            .filter_map(|o| o.observable.clone())
            .collect();
        let environment = sensors::Environment {
            ground: &|x, z| ground(x, z),
            obscured: &|_, _| false,
        };
        sensors.step(&observer, &observables, &environment);
        let mut ids: Vec<u32> = sensors.contacts().iter().map(|c| c.id).collect();
        for contact in sensors.visual() {
            if !ids.contains(&contact.id) {
                ids.push(contact.id);
            }
        }
        ids
    }

    fn drain_events(&mut self, tick: u64) -> Vec<FrameEvent> {
        let mut events = std::mem::take(&mut self.pending_events);
        for report in self.pending_threats.drain(..) {
            events.push(FrameEvent::ThreatReported(report));
        }
        let _ = tick;
        events
    }

    /// The actor's own state, entirely from its own flight model.
    fn own_state(&self, ground: &dyn Fn(f64, f64) -> f64) -> OwnState {
        let terrain = ground(self.flight.position[0], self.flight.position[2]);
        let agl = self.flight.position[1] - terrain;
        // Terrain 1000 ft ahead of the aircraft, for the B44 floor.
        let heading = self.flight.yaw;
        let ahead_x = self.flight.position[0] + heading.sin() * 1000.0;
        let ahead_z = self.flight.position[2] + heading.cos() * 1000.0;
        let limits = self.speed_limits();
        let (positive_g, _negative_g) = self.g_limits();
        OwnState {
            position: self.flight.position,
            heading_deg: heading.to_degrees(),
            flight_path_pitch_deg: self.flight_path_pitch_deg(),
            body_pitch_offset_deg: self.flight.pitch.to_degrees() - self.flight_path_pitch_deg(),
            bank_deg: self.flight.bank.to_degrees(),
            speed: ScalarSpeed(self.flight.speed),
            limits,
            altitude_msl_ft: self.flight.position[1],
            agl_ft: agl,
            terrain_ahead_ft: ground(ahead_x, ahead_z),
            minimum_altitude_ft: MINIMUM_ALTITUDE_FT,
            at_ceiling: false,
            on_ground: agl <= GROUND_CONTACT_FT,
            g_limit: positive_g,
            roll_limit_deg_per_s: self
                .flight
                .model()
                .configuration()
                .aerodynamics
                .roll_limit_rad_per_second
                .to_degrees()
                * self.control_health(),
            maximum_bank_deg: MAXIMUM_BANK_DEG,
            alive: self.alive(),
            fuel_endurance_s: self.endurance_s(),
            time_home_s: self.time_home_s(),
            internal_fuel_lbs: self.flight.fuel,
            radar_emitting: self.flight.radar,
        }
    }

    fn flight_path_pitch_deg(&self) -> f64 {
        let v = self.flight.velocity;
        let horizontal = (v[0] * v[0] + v[2] * v[2]).sqrt();
        if horizontal == 0.0 && v[1] == 0.0 {
            return self.flight.pitch.to_degrees();
        }
        v[1].atan2(horizontal).to_degrees()
    }

    /// The loaded envelope limits at the current altitude (B04, B15).
    ///
    /// Fitted host rule, agent decision 2026-09-17: the minimum and maximum
    /// come from the widest speed band any loaded envelope permits at this
    /// altitude, and corner speed is the slowest speed at which the highest-G
    /// envelope is still available, capped at the maximum. The spec calls
    /// these "the aircraft's loaded envelope limits at its current altitude"
    /// without naming the query, so this reads the same envelope block the
    /// flight model uses rather than inventing a table.
    fn speed_limits(&self) -> SpeedLimits {
        let envelopes = &self.flight.model().configuration().aerodynamics.envelopes;
        let altitude = self.flight.position[1];
        let mut minimum = f64::INFINITY;
        let mut maximum = f64::NEG_INFINITY;
        let mut corner = f64::NAN;
        let mut best_g = i32::MIN;
        for envelope in envelopes {
            let Some((low, high)) = envelope.speeds(altitude) else {
                continue;
            };
            minimum = minimum.min(low);
            maximum = maximum.max(high);
            if envelope.g > best_g {
                best_g = envelope.g;
                corner = low;
            }
        }
        if !minimum.is_finite() || !maximum.is_finite() || maximum <= minimum {
            // No envelope covers this altitude. Fall back to the documented
            // fixture band rather than producing a non-finite limit.
            return SpeedLimits {
                minimum: ScalarSpeed(FALLBACK_MINIMUM_FPS),
                maximum: ScalarSpeed(FALLBACK_MAXIMUM_FPS),
                corner: ScalarSpeed(FALLBACK_CORNER_FPS),
            };
        }
        if !corner.is_finite() {
            corner = minimum;
        }
        SpeedLimits {
            minimum: ScalarSpeed(minimum),
            maximum: ScalarSpeed(maximum),
            corner: ScalarSpeed(corner.min(maximum)),
        }
    }

    /// The loaded G limits, already carrying the AI-only experience
    /// adjustment. A human-flown aircraft is exempt and never reaches here.
    fn g_limits(&self) -> (f64, f64) {
        let envelopes = &self.flight.model().configuration().aerodynamics.envelopes;
        let altitude = self.flight.position[1];
        let speed = self.flight.speed;
        let mut available = 1;
        for envelope in envelopes {
            if let Some((low, high)) = envelope.speeds(altitude)
                && speed >= low
                && speed <= high
                && envelope.g > available
            {
                available = envelope.g;
            }
        }
        let config = self.flight.model().configuration();
        let loading = (self.flight.fuel + self.flight.payload_lbs) / config.mass.empty_lbs;
        let loaded = f64::from(available.max(1))
            / (1.0 + loading * config.aerodynamics.loaded_elevator_percent / 100.0);
        let (positive, negative) = ai_g_limits(
            self.controller.experience().level,
            loaded,
            -loaded / 2.0,
            self.identity.human_controlled,
        );
        (
            positive * self.control_health(),
            negative * self.control_health(),
        )
    }

    /// Fitted until axis-specific AI damage is imported: remaining airframe
    /// health scales both pitch and roll authority linearly.
    fn control_health(&self) -> f64 {
        (1.0 - self.flight.damage_fraction).clamp(0.0, 1.0)
    }

    /// B48 endurance at cruise, seconds.
    ///
    /// Fitted host rule: fuel flow is the model's military flow scaled by the
    /// current throttle, floored so a shut-down engine does not report an
    /// infinite endurance. The spec's own definition needs the throttle search
    /// that holds cruise speed, which the host does not yet perform.
    fn endurance_s(&self) -> f64 {
        let propulsion = &self.flight.model().configuration().propulsion;
        let flow = (propulsion.military_fuel_lbs_per_second * self.flight.throttle.max(0.1))
            .max(MINIMUM_FUEL_FLOW_LBS_PER_S);
        self.flight.fuel / flow
    }

    fn time_home_s(&self) -> Option<f64> {
        let home = self.home_airport?;
        let dx = home.x - self.flight.position[0];
        let dz = home.z - self.flight.position[2];
        let distance = (dx * dx + dz * dz).sqrt();
        let cruise = super::route::cruise_speed(&self.speed_limits());
        if cruise.0 <= 0.0 {
            return None;
        }
        Some(distance / cruise.0)
    }

    /// Build the permitted target views for this actor.
    fn target_views(&self, permitted: &[u32], world: &[WorldObject]) -> Vec<TargetView> {
        world
            .iter()
            .filter(|o| o.id != self.identity.actor.0)
            .filter(|o| permitted.contains(&o.id))
            .filter(|o| o.alive && !o.destroyed)
            .map(|o| TargetView {
                id: o.id,
                side: o.side,
                position: o.position,
                heading_deg: o.heading_deg,
                pitch_deg: o.pitch_deg,
                speed: o.speed,
                maximum_speed: o.maximum_speed,
                is_aircraft: o.is_aircraft,
                is_fighter: o.is_fighter,
                human_controlled: o.human_controlled,
                valid: o.alive && !o.destroyed,
                type_allowed: true,
                seeker_eligible: self.seeker_eligible(o),
                wing_attackers: 0,
                terrain_blocked: false,
                sensor_supported: self
                    .sensors
                    .as_ref()
                    .is_none_or(|sensor| sensor.supports(o.id)),
            })
            .collect()
    }

    /// Whether any carried store's envelope could engage this object (B45).
    fn seeker_eligible(&self, object: &WorldObject) -> bool {
        let class = if object.is_aircraft {
            weapon_service::TargetClass::Air
        } else {
            weapon_service::TargetClass::Surface
        };
        let range = distance(self.flight.position, object.position);
        self.stations.iter().any(|s| {
            !s.store.inhibited
                && !s.is_empty()
                && weapon_service::store_eligible(s.capability, class)
                && s.maximum_range_ft.is_none_or(|max| range <= max)
        })
    }

    fn station_views(&self, targets: &[TargetView], own: &OwnState) -> Vec<StationView> {
        // Point the stores at the target this actor is actually holding, not
        // at whichever object happens to come first in the world list. The
        // retained target is last tick's; the controller re-selects inside
        // `step`, and a one tick lag on a pointing error is immaterial next to
        // aiming at the wrong aircraft. With nothing retained, the nearest
        // permitted candidate is the best available guess.
        let retained = self.controller.target();
        let target = retained
            .and_then(|id| targets.iter().find(|t| t.id == id))
            .or_else(|| {
                targets.iter().min_by(|a, b| {
                    distance(own.position, a.position)
                        .partial_cmp(&distance(own.position, b.position))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            });
        self.stations
            .iter()
            .map(|s| {
                let pointing = target
                    .map(|t| pointing_error_deg(own, t.position))
                    .unwrap_or(180.0);
                let mut view = StationView {
                    station: s.station,
                    guided: s.guided,
                    capability: s.capability,
                    inhibited: s.store.inhibited,
                    rounds: s.store.rounds,
                    pointing_error_deg: pointing,
                    employment_limit_deg: s.employment_limit_deg,
                    employment_fit: s
                        .employment_limit_deg
                        .is_none_or(|limit| pointing <= limit)
                        .then_some(pointing),
                    minimum_range_ft: s.minimum_range_ft,
                    maximum_range_ft: s.maximum_range_ft,
                    requires_radar: s.requires_radar,
                    requires_sensor: s.requires_sensor,
                    employment_zone: s.employment_zone,
                    mount: s.mount,
                    damage_vs_category: s.damage_vs_category,
                    store_speed: s.store_speed,
                    tracking_delay: s.tracking_delay,
                    pacing: s.pacing,
                };
                view.employment_fit = target.and_then(|target| view.employment_error(own, target));
                view
            })
            .collect()
    }

    /// Debit this actor's own ammunition and produce the launch event.
    ///
    /// Returns `None` when the debit refuses, so an inhibited or empty station
    /// can never produce a projectile. The debit happens first, exactly as
    /// B45 records, and the host is told how many projectiles to create.
    fn release(
        &mut self,
        intent: &super::controller::WeaponIntent,
        _batch: &IntentBatch,
    ) -> Option<LaunchEvent> {
        let index = self
            .stations
            .iter()
            .position(|s| s.station == intent.request.station)?;
        let spec = &mut self.stations[index];
        let request = weapon_service::ReleaseRequest {
            debit: spec.debit.max(1),
            projectile_count: spec.projectile_count.max(1),
            allocation_succeeds: true,
            target: weapon_service::TargetRetention::Retained(intent.request.target),
            // No AI path is human controlled, so the global unlimited
            // ammunition bypass can never apply here (B45).
            human_controlled: false,
            global_unlimited: false,
            atomic_release: false,
        };
        let before = spec.rounds();
        let report = weapon_service::release(&mut spec.store, &request).ok()?;
        if let (Rounds::Finite(before), Rounds::Finite(after)) = (before, spec.rounds()) {
            self.flight.payload_lbs = (self.flight.payload_lbs
                - f64::from(before - after) * spec.external_round_lbs)
                .max(0.0);
        }
        let created = match report.projectiles {
            weapon_service::ProjectileCreation::Created { count } => count,
            weapon_service::ProjectileCreation::None => return None,
        };
        Some(LaunchEvent {
            actor: self.identity.actor.0,
            station: intent.request.station,
            target: intent.request.target.0,
            request_id: intent.request.request_id,
            projectiles: created,
        })
    }

    /// Debit this actor's own dispensers (B47).
    ///
    /// Release stops as soon as the matching dispenser is empty, and the other
    /// class is never substituted.
    fn release_devices(&mut self, class: SeekerClass, count: u8) -> Option<DeviceEvent> {
        let index = super::threat::select_dispenser(&self.dispensers, class)?;
        let mut released = 0;
        for _ in 0..count {
            match super::threat::debit_device(&mut self.dispensers[index], false) {
                Ok(_) => released += 1,
                Err(_) => break,
            }
        }
        if released == 0 {
            return None;
        }
        Some(DeviceEvent {
            actor: self.identity.actor.0,
            class,
            released,
        })
    }

    /// Convert this tick's maneuver into controls and step this actor's own
    /// flight model. Only that model may advance the aircraft state.
    fn fly(
        &mut self,
        intent: Option<&MotionIntent>,
        own: &OwnState,
        ground: &dyn Fn(f64, f64) -> f64,
    ) -> Result<()> {
        let input = match intent {
            Some(intent) => {
                let floor = self.controller.terrain_floor(&dummy_frame(own))?;
                let output = self.adapter.controls(
                    &self.flight,
                    intent,
                    &own.limits,
                    own.g_limit,
                    own.roll_limit_deg_per_s,
                    own.maximum_bank_deg,
                    floor,
                    flight::DT,
                )?;
                output.input
            }
            None => PilotInput::default(),
        };
        self.flight
            .step_surface(&input, |x, z| Surface::terrain(ground(x, z)));
        self.last_input = input;
        Ok(())
    }
}

/// A frame carrying only what the terrain floor needs.
fn dummy_frame(own: &OwnState) -> DecisionFrame<'static> {
    DecisionFrame {
        tick: 0,
        own: *own,
        targets: &[],
        events: &[],
        stations: &[],
        dispensers: &[],
        wing: WingView {
            control: WingControl::Loose,
            formation: Formation::Echelon,
            horizontal_spacing_ft: 2048,
            vertical_spacing_ft: 512,
            slot: 1,
            leader: None,
            wingmen_in_formation: 0,
            wing_combat: false,
            wing_approach: false,
            wing_approach_value_ft: None,
        },
        route: RouteView {
            home_airport: None,
            leader_is_ai: true,
        },
        now: TimeOfDay(0),
        flight_state: FlightState::Free,
    }
}

/// The record's minimum-altitude value; 300 in every inspected record (B44).
pub const MINIMUM_ALTITUDE_FT: f64 = 300.0;
/// Fitted: AGL at or below this counts as ground contact for the B44 overrides.
pub const GROUND_CONTACT_FT: f64 = 5.0;
/// Fitted: the reference maximum bank for the B44 authority curve.
///
/// The spec says the reference bank is "the aircraft's own maximum bank" but
/// the imported records do not expose one, so the host uses a single value for
/// every ported fighter. Agent decision, 2026-09-17.
pub const MAXIMUM_BANK_DEG: f64 = 80.0;
/// Fitted fixture band used only when no envelope covers the altitude.
pub const FALLBACK_MINIMUM_FPS: f64 = 220.0;
/// See [`FALLBACK_MINIMUM_FPS`].
pub const FALLBACK_MAXIMUM_FPS: f64 = 1600.0;
/// See [`FALLBACK_MINIMUM_FPS`].
pub const FALLBACK_CORNER_FPS: f64 = 700.0;
/// Fitted floor on fuel flow so a shut-down engine cannot report an infinite
/// endurance.
pub const MINIMUM_FUEL_FLOW_LBS_PER_S: f64 = 0.01;

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt()
}

/// Angle between the actor's nose and a point, in degrees.
fn pointing_error_deg(own: &OwnState, point: [f64; 3]) -> f64 {
    let dx = point[0] - own.position[0];
    let dy = point[1] - own.position[1];
    let dz = point[2] - own.position[2];
    let horizontal = (dx * dx + dz * dz).sqrt();
    if horizontal == 0.0 && dy == 0.0 {
        return 0.0;
    }
    let bearing = dx.atan2(dz).to_degrees();
    let elevation = dy.atan2(horizontal).to_degrees();
    let heading_error = wrap_signed(bearing - own.heading_deg).abs();
    let pitch_error = (elevation - own.flight_path_pitch_deg).abs();
    heading_error.max(pitch_error)
}

fn wrap_signed(value: f64) -> f64 {
    let mut v = value % 360.0;
    if v > 180.0 {
        v -= 360.0;
    }
    if v < -180.0 {
        v += 360.0;
    }
    v
}

/// Build one AI actor's stores from a simple description.
///
/// Convenience for hosts and fixtures that do not carry a full imported
/// loadout. It creates finite ammunition only; there is no unlimited path.
pub fn simple_stations(
    air_to_air_rounds: u32,
    gun_rounds: u32,
    store_speed: ScalarSpeed,
) -> Vec<StationSpec> {
    let mut stations = Vec::new();
    if air_to_air_rounds > 0 {
        stations.push(StationSpec {
            station: StationId(0),
            guided: true,
            capability: StoreCapability::AIR_TO_AIR_MISSILE,
            store: StoreState {
                inhibited: false,
                rounds: Rounds::Finite(air_to_air_rounds),
            },
            debit: 1,
            external_round_lbs: 0.0,
            projectile_count: 1,
            employment_limit_deg: Some(30.0),
            damage_vs_category: 100.0,
            store_speed,
            tracking_delay: Delay::seconds(1),
            pacing: ProjectilePacing {
                burst_count: 1,
                burst_interval: Delay::seconds(0),
                reload: Delay::seconds(2),
                startup: Delay::seconds(0),
            },
            minimum_range_ft: 0.0,
            requires_radar: false,
            requires_sensor: false,
            employment_zone: None,
            mount: [0.0; 3],
            maximum_range_ft: Some(40000.0),
        });
    }
    if gun_rounds > 0 {
        stations.push(StationSpec {
            station: StationId(1),
            guided: false,
            capability: StoreCapability::GUN,
            store: StoreState {
                inhibited: false,
                rounds: Rounds::Finite(gun_rounds),
            },
            debit: 10,
            external_round_lbs: 0.0,
            projectile_count: 10,
            employment_limit_deg: Some(5.0),
            damage_vs_category: 10.0,
            store_speed: ScalarSpeed(3000.0),
            tracking_delay: Delay::quarters(1),
            pacing: ProjectilePacing {
                burst_count: 10,
                burst_interval: Delay::quarters(1),
                reload: Delay::quarters(2),
                startup: Delay::seconds(0),
            },
            minimum_range_ft: 0.0,
            requires_radar: false,
            requires_sensor: false,
            employment_zone: None,
            mount: [0.0; 3],
            maximum_range_ft: Some(6000.0),
        });
    }
    stations
}

/// The standard two-class dispenser fit for an AI fighter.
pub fn simple_dispensers(each: u32) -> Vec<DispenserStore> {
    vec![
        DispenserStore {
            class: SeekerClass::Infrared,
            count: each,
        },
        DispenserStore {
            class: SeekerClass::Radar,
            count: each,
        },
    ]
}

/// Whether an aircraft has a reviewed retail donor.
pub fn is_ported(aircraft: AircraftId) -> bool {
    AircraftId::ALL.contains(&aircraft.source())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::controller::{BehaviorFamily, MissionRole};
    use crate::ai::experience::ExperienceOrigin;
    use crate::ai::targeting::Side;
    use crate::ai::{Experience, weapon_service::ActorId};

    fn aircraft_index(id: AircraftId) -> usize {
        AircraftId::ALL.iter().position(|item| *item == id).unwrap()
    }

    // Distinct synthetic capabilities, never presented as measured retail data.
    fn synthetic_profile(id: AircraftId) -> tore_formats::aircraft::Aircraft {
        let mut profile = crate::flight::integration_tests::profile();
        let index = aircraft_index(id);
        profile.id = id;
        profile.name = match id {
            AircraftId::F18 => "F/A-18D",
            AircraftId::Rafale => "RAFALE",
            AircraftId::F14 => "F-14",
            AircraftId::A4E => "A-4E",
            AircraftId::X31 => "X-31",
            AircraftId::Mig29 => "MiG-29",
            AircraftId::Su27 => "Su-27",
            AircraftId::Mig21 => "MiG-21",
            AircraftId::Su25 => "Su-25",
            AircraftId::Mig23 => "MiG-23",
            AircraftId::Su35 => "Su-35",
            AircraftId::F22 | AircraftId::Faxx => "F-22",
        }
        .into();
        profile.shape = format!("{}.SH", id.stem());
        profile.fields.get_mut("aftThrust").unwrap().value =
            if matches!(id, AircraftId::A4E | AircraftId::Su25) {
                "0".into()
            } else {
                (400 + index * 10).to_string()
            };
        for envelope in &mut profile.envelopes {
            for point in &mut envelope.points {
                point[0] *= 0.85 + index as f64 * 0.025;
            }
        }
        profile
    }

    fn profile() -> BehaviorProfile {
        BehaviorProfile {
            family: BehaviorFamily::FighterStrike,
            role: MissionRole::AirToAir,
        }
    }

    fn resolved(level: Experience) -> ResolvedExperience {
        ResolvedExperience {
            level,
            origin: ExperienceOrigin::QuickMission { selected: level },
        }
    }

    fn flight_state(position: [f64; 3], yaw: f64) -> flight::State {
        let aircraft = crate::flight::integration_tests::profile();
        let mut state = flight::State::new(&aircraft, position).unwrap();
        state.yaw = yaw;
        let basis = crate::attitude::Basis::new(yaw, 0.0, 0.0);
        state.velocity = basis.forward.map(|v| v * state.speed);
        state
    }

    fn setup(id: u32, side: u32, member: u8, position: [f64; 3], yaw: f64) -> ActorSetup {
        ActorSetup {
            identity: ActorIdentity {
                actor: ActorId(id),
                side: Side(side),
                wing: 0,
                member,
                aircraft: AircraftId::F18,
                human_controlled: false,
            },
            profile: profile(),
            experience: resolved(Experience::Experienced),
            seed: u64::from(id) * 31 + 7,
            flight: flight_state(position, yaw),
            sensors: None,
            stations: simple_stations(4, 500, ScalarSpeed(2000.0)),
            dispensers: simple_dispensers(30),
            wing_slot: member.max(1),
            home_airport: Some(crate::ai::route::Position { x: 0.0, z: 0.0 }),
        }
    }

    fn object(actor: &AiActor, side: u32) -> WorldObject {
        WorldObject {
            id: actor.id(),
            side: Side(side),
            position: actor.flight().position,
            velocity: actor.flight().velocity,
            heading_deg: actor.flight().yaw.to_degrees(),
            pitch_deg: actor.flight().pitch.to_degrees(),
            speed: ScalarSpeed(actor.flight().speed),
            maximum_speed: ScalarSpeed(1600.0),
            is_aircraft: true,
            is_fighter: true,
            human_controlled: false,
            alive: actor.alive(),
            destroyed: false,
            observable: None,
        }
    }

    fn flat(_x: f64, _z: f64) -> f64 {
        0.0
    }

    /// Two aircraft, opposite sides, converging head on.
    fn one_v_one() -> AiMission {
        let mut mission = AiMission::new();
        mission.push(AiActor::new(setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0)).unwrap());
        mission.push(
            AiActor::new(setup(
                2,
                2,
                0,
                [0.0, 20000.0, 40000.0],
                std::f64::consts::PI,
            ))
            .unwrap(),
        );
        mission
    }

    fn world_of(mission: &AiMission) -> Vec<WorldObject> {
        mission
            .actors()
            .iter()
            .map(|a| object(a, a.identity().side.0))
            .collect()
    }

    fn run(mission: &mut AiMission, ticks: u64) -> MissionOutput {
        let mut combined = MissionOutput::default();
        for _ in 0..ticks {
            let world = world_of(mission);
            let out = mission.step(&world, &flat, TimeOfDay(0)).unwrap();
            combined.launches.extend(out.launches);
            combined.devices.extend(out.devices);
            combined.fallbacks.extend(out.fallbacks);
            combined.activities.extend(out.activities);
        }
        combined
    }

    #[test]
    fn the_ai_actually_launches_missiles_at_an_enemy() {
        let mut mission = one_v_one();
        let missiles_before: u32 = mission
            .actors()
            .iter()
            .map(|a| match a.stations()[0].store.rounds {
                Rounds::Finite(n) => n,
                Rounds::Unlimited => 0,
            })
            .sum();
        let out = run(&mut mission, 7200);
        assert!(
            !out.launches.is_empty(),
            "sixty seconds of a head-on 1v1 produced no launch at all"
        );
        let missiles_after: u32 = mission
            .actors()
            .iter()
            .map(|a| match a.stations()[0].store.rounds {
                Rounds::Finite(n) => n,
                Rounds::Unlimited => 0,
            })
            .sum();
        assert!(
            missiles_after < missiles_before,
            "the guided store was never chosen: {missiles_before} to {missiles_after}"
        );
        assert!(
            out.activities
                .iter()
                .any(|(_, a)| *a == Activity::Attacking)
        );
    }

    #[test]
    fn a_warned_ace_releases_countermeasures_and_defends() {
        let mut mission = one_v_one();
        run(&mut mission, 120);
        // Repeated warnings from a third aircraft, so B47's "launcher is the
        // current target" and same-side exemptions cannot apply.
        for tick in 0..40 {
            mission.actor_mut(1).unwrap().report_threat(ThreatReport {
                missile_id: 500 + tick,
                seeker: SeekerClass::Infrared,
                launcher_id: 999,
                launcher_same_side: false,
                distance_at_launch_ft: 3000.0,
                launch_tick: 0,
            });
            run(&mut mission, 120);
        }
        let dispensers = mission.actor(1).unwrap().dispensers();
        let infrared = dispensers
            .iter()
            .find(|d| d.class == SeekerClass::Infrared)
            .unwrap();
        assert!(
            infrared.count < 30,
            "an Ace under repeated infrared launches never released a flare"
        );
        // The matching class only: radar decoys are never substituted.
        let radar = dispensers
            .iter()
            .find(|d| d.class == SeekerClass::Radar)
            .unwrap();
        assert_eq!(
            radar.count, 30,
            "a radar decoy was released for an infrared launch"
        );
    }

    #[test]
    fn a_mission_steps_every_actor_and_advances_its_tick() {
        let mut mission = one_v_one();
        assert_eq!(mission.tick(), 0);
        run(&mut mission, 120);
        assert_eq!(mission.tick(), 120);
        assert_eq!(mission.len(), 2);
    }

    #[test]
    fn each_actor_flies_its_own_model_and_actually_moves() {
        let mut mission = one_v_one();
        let before: Vec<[f64; 3]> = mission
            .actors()
            .iter()
            .map(|a| a.flight().position)
            .collect();
        run(&mut mission, 240);
        for (index, actor) in mission.actors().iter().enumerate() {
            let moved = distance(before[index], actor.flight().position);
            assert!(moved > 1000.0, "actor {index} moved only {moved} ft");
        }
    }

    #[test]
    fn two_actors_hold_independent_state() {
        let mut mission = one_v_one();
        run(&mut mission, 240);
        let a = mission.actor(1).unwrap();
        let b = mission.actor(2).unwrap();
        assert_ne!(a.flight().position, b.flight().position);
        assert_ne!(a.flight().yaw, b.flight().yaw);
        // Neither actor's stores are the other's.
        assert_eq!(a.stations().len(), b.stations().len());
        assert!(!std::ptr::eq(a.stations(), b.stations()));
    }

    #[test]
    fn repeated_runs_with_the_same_seed_are_identical() {
        let mut first = one_v_one();
        let mut second = one_v_one();
        let a = run(&mut first, 600);
        let b = run(&mut second, 600);
        assert_eq!(a.launches, b.launches);
        assert_eq!(a.devices, b.devices);
        assert_eq!(a.activities, b.activities);
        for (x, y) in first.actors().iter().zip(second.actors()) {
            assert_eq!(x.flight().position, y.flight().position);
            assert_eq!(x.flight().yaw, y.flight().yaw);
            assert_eq!(x.rounds_remaining(), y.rounds_remaining());
        }
    }

    #[test]
    fn an_actor_never_targets_itself() {
        let mut mission = one_v_one();
        run(&mut mission, 240);
        for actor in mission.actors() {
            assert_ne!(actor.controller().target(), Some(actor.id()));
        }
    }

    #[test]
    fn an_actor_never_targets_its_own_side() {
        let mut mission = AiMission::new();
        mission.push(AiActor::new(setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0)).unwrap());
        mission.push(AiActor::new(setup(2, 1, 1, [0.0, 20000.0, 10000.0], 0.0)).unwrap());
        run(&mut mission, 600);
        for actor in mission.actors() {
            assert_eq!(actor.controller().target(), None, "engaged a friendly");
        }
    }

    #[test]
    fn ammunition_is_debited_and_never_free() {
        let mut mission = one_v_one();
        let start: u32 = mission.actors().iter().map(|a| a.rounds_remaining()).sum();
        let out = run(&mut mission, 3600);
        let end: u32 = mission.actors().iter().map(|a| a.rounds_remaining()).sum();
        if out.launches.is_empty() {
            // Nothing fired, so nothing may have been spent either.
            assert_eq!(start, end);
        } else {
            assert!(
                end < start,
                "rounds were not debited by {} launches",
                out.launches.len()
            );
        }
    }

    #[test]
    fn an_empty_store_stops_producing_launches() {
        let mut mission = AiMission::new();
        let mut a = setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0);
        // One missile only, and no gun.
        a.stations = simple_stations(1, 0, ScalarSpeed(2000.0));
        mission.push(AiActor::new(a).unwrap());
        mission.push(
            AiActor::new(setup(
                2,
                2,
                0,
                [0.0, 20000.0, 20000.0],
                std::f64::consts::PI,
            ))
            .unwrap(),
        );
        let out = run(&mut mission, 7200);
        let fired = out.launches.iter().filter(|l| l.actor == 1).count();
        assert!(fired <= 1, "one missile produced {fired} launches");
        assert_eq!(
            mission.actor(1).unwrap().rounds_remaining(),
            1 - fired as u32
        );
    }

    #[test]
    fn an_inhibited_store_never_launches() {
        let mut mission = AiMission::new();
        let mut a = setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0);
        for station in &mut a.stations {
            station.store.inhibited = true;
        }
        mission.push(AiActor::new(a).unwrap());
        mission.push(
            AiActor::new(setup(
                2,
                2,
                0,
                [0.0, 20000.0, 20000.0],
                std::f64::consts::PI,
            ))
            .unwrap(),
        );
        let out = run(&mut mission, 3600);
        assert!(out.launches.iter().all(|l| l.actor != 1));
    }

    #[test]
    fn a_launch_event_carries_a_unique_request_identity() {
        let mut mission = one_v_one();
        let out = run(&mut mission, 7200);
        let mut seen = Vec::new();
        for launch in &out.launches {
            let key = (launch.actor, launch.request_id);
            assert!(!seen.contains(&key), "duplicate launch {key:?}");
            seen.push(key);
        }
    }

    #[test]
    fn a_threat_report_reaches_only_the_aircraft_it_names() {
        let mut mission = one_v_one();
        run(&mut mission, 120);
        mission.actor_mut(1).unwrap().report_threat(ThreatReport {
            missile_id: 500,
            seeker: SeekerClass::Infrared,
            launcher_id: 999,
            launcher_same_side: false,
            distance_at_launch_ft: 4000.0,
            launch_tick: 0,
        });
        let out = run(&mut mission, 1200);
        // Only actor 1 can have released devices from that warning.
        assert!(out.devices.iter().all(|d| d.actor == 1));
    }

    #[test]
    fn countermeasures_come_out_of_the_actors_own_dispensers() {
        let mut mission = one_v_one();
        let before = mission.actor(1).unwrap().dispensers()[0].count;
        mission.actor_mut(1).unwrap().report_threat(ThreatReport {
            missile_id: 500,
            seeker: SeekerClass::Infrared,
            launcher_id: 999,
            launcher_same_side: false,
            distance_at_launch_ft: 1000.0,
            launch_tick: 0,
        });
        let out = run(&mut mission, 2400);
        if out.devices.iter().any(|d| d.actor == 1) {
            let after = mission.actor(1).unwrap().dispensers()[0].count;
            assert!(after < before, "devices were released without a debit");
        }
    }

    #[test]
    fn a_destroyed_actor_stops_flying_and_reports_destroyed() {
        let mut mission = one_v_one();
        run(&mut mission, 60);
        mission.actor_mut(1).unwrap().set_alive(false);
        let frozen = mission.actor(1).unwrap().flight().position;
        let out = run(&mut mission, 120);
        assert_eq!(mission.actor(1).unwrap().flight().position, frozen);
        assert!(out.activities.contains(&(1, Activity::Destroyed)));
    }

    #[test]
    fn every_ported_aircraft_runs_a_one_v_one_at_every_level() {
        for aircraft in AircraftId::ALL {
            for level in Experience::ALL {
                let mut mission = AiMission::new();
                for (index, (id, side, yaw, z)) in [
                    (1u32, 1u32, 0.0, 0.0),
                    (2u32, 2u32, std::f64::consts::PI, 30000.0),
                ]
                .into_iter()
                .enumerate()
                {
                    let mut s = setup(id, side, 0, [0.0, 20000.0, z], yaw);
                    s.identity.aircraft = aircraft;
                    let profile = synthetic_profile(aircraft);
                    s.flight = flight::State::new(&profile, s.flight.position).unwrap();
                    s.flight.yaw = yaw;
                    s.flight.velocity = crate::attitude::Basis::new(yaw, 0.0, 0.0)
                        .forward
                        .map(|v| v * s.flight.speed);
                    s.stations = simple_stations(
                        2 + index as u32,
                        200 + 10 * aircraft_index(aircraft) as u32,
                        ScalarSpeed(1500.0 + aircraft_index(aircraft) as f64 * 50.0),
                    );
                    s.experience = resolved(level);
                    s.seed = 101 + index as u64;
                    mission.push(AiActor::new(s).unwrap());
                }
                run(&mut mission, 600);
                for actor in mission.actors() {
                    assert!(
                        actor.flight().position.iter().all(|v| v.is_finite()),
                        "{aircraft:?} {level:?} produced a non-finite position"
                    );
                }
            }
        }
    }

    #[test]
    fn a_two_v_two_runs_with_leaders_and_wingmen() {
        let mut mission = AiMission::new();
        mission.push(AiActor::new(setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0)).unwrap());
        mission.push(AiActor::new(setup(2, 1, 1, [2000.0, 20000.0, -2000.0], 0.0)).unwrap());
        mission.push(
            AiActor::new(setup(
                3,
                2,
                0,
                [0.0, 20000.0, 40000.0],
                std::f64::consts::PI,
            ))
            .unwrap(),
        );
        mission.push(
            AiActor::new(setup(
                4,
                2,
                1,
                [2000.0, 20000.0, 42000.0],
                std::f64::consts::PI,
            ))
            .unwrap(),
        );
        run(&mut mission, 1200);
        assert_eq!(mission.len(), 4);
        for actor in mission.actors() {
            assert!(actor.flight().position.iter().all(|v| v.is_finite()));
        }
        // The wingmen are not leaders and the leaders are.
        assert!(mission.actor(1).unwrap().identity().is_leader());
        assert!(!mission.actor(2).unwrap().identity().is_leader());
    }

    #[test]
    fn each_wing_follows_only_its_own_leader_including_the_human() {
        let mut mission = AiMission::new();
        mission.set_external_leader(Side(1), 0, 0);
        for (id, side, wing, member) in [
            (1, 1, 0, 1),
            (2, 1, 1, 0),
            (3, 1, 1, 1),
            (4, 2, 1, 0),
            (5, 2, 1, 1),
        ] {
            let mut actor = setup(id, side, member, [id as f64 * 1000., 20000., 0.], 0.);
            actor.identity.wing = wing;
            mission.push(AiActor::new(actor).unwrap());
        }
        let mut world = world_of(&mission);
        let mut player = world[0].clone();
        player.id = 0;
        player.position = [90000., 20000., 0.];
        player.human_controlled = true;
        world.push(player);
        assert_eq!(mission.leader_view(0, &world).unwrap().position[0], 90000.);
        assert!(mission.leader_view(1, &world).is_none());
        assert_eq!(mission.leader_view(2, &world).unwrap().position[0], 2000.);
        assert_eq!(mission.leader_view(4, &world).unwrap().position[0], 4000.);
        world.last_mut().unwrap().alive = false;
        assert!(mission.leader_view(0, &world).is_none());
    }

    #[test]
    fn formation_recovers_from_ahead_or_behind_and_holds_through_turns() {
        for (turn, initial_offset) in [0.0_f64, 1.5, -1.5]
            .into_iter()
            .flat_map(|turn| [-800., 0., 800.].map(|offset| (turn, offset)))
        {
            let mut mission = AiMission::new();
            mission.set_spacing(512, 0);
            mission.set_external_leader(Side(1), 0, 0);
            let mut start = setup(1, 1, 1, [512., 20000., -512. + initial_offset], 0.);
            // Isolate formation from the synthetic model's bingo return.
            start.home_airport = None;
            start.flight.speed = 800.;
            start.flight.velocity = [0., 0., 800.];
            mission.push(AiActor::new(start).unwrap());
            let mut leader = object(mission.actor(1).unwrap(), 1);
            leader.id = 0;
            leader.human_controlled = true;
            leader.position = [0., 20000., 0.];
            let mut last_error = 0.;
            for tick in 0..14400 {
                let heading = (turn * (tick as f64 / 120. - 30.).max(0.)).to_radians();
                leader.heading_deg = heading.to_degrees();
                leader.velocity = [800. * heading.sin(), 0., 800. * heading.cos()];
                let mut world = world_of(&mission);
                world.push(leader.clone());
                mission.step(&world, &flat, TimeOfDay(0)).unwrap();
                for i in 0..3 {
                    leader.position[i] += leader.velocity[i] / 120.;
                }
                let slot = [
                    leader.position[0] + 512. * heading.cos() - 512. * heading.sin(),
                    20000.,
                    leader.position[2] - 512. * heading.sin() - 512. * heading.cos(),
                ];
                last_error = distance(mission.actor(1).unwrap().flight().position, slot);
                if tick > 7200 {
                    assert!(
                        last_error < 150.,
                        "turn {turn}, start {initial_offset}, tick {tick}: slot error {last_error}"
                    );
                }
            }
            assert!(last_error < 150.);
        }
    }

    #[test]
    fn formation_decisions_do_not_depend_on_actor_iteration_order() {
        fn scene(reverse: bool) -> AiMission {
            let mut m = AiMission::new();
            m.set_external_leader(Side(1), 0, 0);
            m.set_spacing(512, 0);
            for member in 1..=4 {
                let offset =
                    super::super::wing::formation_slot_point(Formation::Echelon, member, 512, 0)
                        .unwrap();
                let mut start = setup(
                    u32::from(member),
                    1,
                    member,
                    [offset[0], 20000., offset[2]],
                    0.,
                );
                start.home_airport = None;
                m.push(AiActor::new(start).unwrap());
            }
            if reverse {
                m.actors.reverse();
            }
            m
        }
        let mut a = scene(false);
        let mut b = scene(true);
        let mut leader = object(a.actor(1).unwrap(), 1);
        leader.id = 0;
        leader.position = [0., 20000., 0.];
        leader.heading_deg = 180.;
        leader.velocity = [0., 0., -800.];
        for _ in 0..1800 {
            for mission in [&mut a, &mut b] {
                let mut world = world_of(mission);
                world.push(leader.clone());
                mission.step(&world, &flat, TimeOfDay(0)).unwrap();
            }
            for id in 1..=4 {
                assert_eq!(a.actor(id).unwrap().flight(), b.actor(id).unwrap().flight());
                assert_eq!(
                    a.actor(id).unwrap().controller().formation_trace(),
                    b.actor(id).unwrap().controller().formation_trace()
                );
            }
            leader.position[2] -= 800. / 120.;
        }
    }

    #[test]
    fn formation_diving_reversal_uses_separate_physical_approaches() {
        use super::super::formation::Phase;
        for (turn, climb, repeat) in [
            (-1.0, -1.0, false),
            (1.0, -1.0, false),
            (1.0, 1.0, false),
            (-1.0, -1.0, true),
        ] {
            let mut mission = AiMission::new();
            mission.set_spacing(512, 0);
            mission.set_external_leader(Side(1), 0, 0);
            for member in 1..=4 {
                let offset =
                    super::super::wing::formation_slot_point(Formation::Echelon, member, 512, 0)
                        .unwrap();
                let mut start = setup(
                    u32::from(member),
                    1,
                    member,
                    [offset[0], 20000., offset[2]],
                    0.,
                );
                start.home_airport = None;
                start.flight.speed = 800.;
                start.flight.velocity = [0., 0., 800.];
                mission.push(AiActor::new(start).unwrap());
            }
            let mut leader = object(mission.actor(1).unwrap(), 1);
            leader.id = 0;
            leader.position = [0., 20000., 0.];
            leader.human_controlled = true;
            let mut minimum = f64::INFINITY;
            let mut phases = [[false; 6]; 4];
            let mut last = [Phase::Close; 4];
            for tick in 0..36000 {
                let t = tick as f64 / 120.;
                let progress = ((t - 10.) / 20.).clamp(0., 1.);
                let reversal = if repeat {
                    ((t - 70.) / 20.).clamp(0., 1.)
                } else {
                    0.
                };
                leader.heading_deg = turn * 180. * (progress - reversal);
                leader.pitch_deg = climb
                    * 25.
                    * ((progress * std::f64::consts::PI).sin()
                        - (reversal * std::f64::consts::PI).sin());
                leader.velocity =
                    super::super::formation::velocity(leader.heading_deg, leader.pitch_deg, 800.);
                let mut world = world_of(&mission);
                world.push(leader.clone());
                let before: Vec<_> = mission
                    .actors()
                    .iter()
                    .map(|a| a.flight().clone())
                    .collect();
                mission.step(&world, &flat, TimeOfDay(0)).unwrap();
                for i in 0..3 {
                    leader.position[i] += leader.velocity[i] / 120.;
                }
                let mut achieved = world_of(&mission);
                achieved.push(leader.clone());
                for (i, actor) in mission.actors().iter().enumerate() {
                    let mut replay = before[i].clone();
                    replay.step_surface(actor.last_input(), |_, _| Surface::terrain(0.));
                    assert_eq!(replay, *actor.flight());
                    let trace = actor.controller().formation_trace().unwrap();
                    phases[i][trace.phase as usize] = true;
                    if trace.phase != last[i] {
                        eprintln!(
                            "turn {turn} t {t:.1} actor {} {:?} distance {:.0} closure {:.0} yielding {:?}",
                            actor.id(),
                            trace.phase,
                            trace.slot_distance_ft,
                            trace.closure_fps,
                            trace.yielding_to
                        );
                        last[i] = trace.phase;
                    }
                    assert!(!actor.flight().crashed);
                    for other in achieved.iter().filter(|o| o.id != actor.id()) {
                        minimum = minimum.min(distance(actor.flight().position, other.position));
                    }
                }
            }
            eprintln!(
                "turn {turn}, climb {climb}, repeat {repeat}: minimum separation {minimum:.1}"
            );
            for (i, actor) in mission.actors().iter().enumerate() {
                let trace = actor.controller().formation_trace().unwrap();
                eprintln!(
                    "actor {} final {:?} slot distance {:.0}",
                    actor.id(),
                    trace.phase,
                    trace.slot_distance_ft
                );
                assert!(phases[i][Phase::Intercept as usize]);
                assert!(phases[i][Phase::Capture as usize]);
                assert_eq!(trace.phase, Phase::Close);
                assert!(trace.slot_distance_ft < 250.);
            }
            assert!(minimum > 250., "unsafe separation {minimum}");
        }
    }

    #[test]
    fn an_idle_wingman_flies_toward_its_own_delta_slot() {
        let mut mission = AiMission::new();
        mission.set_spacing(512, 0);
        mission.push(AiActor::new(setup(1, 1, 0, [0., 20000., 0.], 0.)).unwrap());
        mission.push(AiActor::new(setup(2, 1, 1, [-3000., 20000., -512.], 0.)).unwrap());
        let initial_error = 3512.;
        run(&mut mission, 1200);
        let leader = mission.actor(1).unwrap().flight();
        let wingman = mission.actor(2).unwrap().flight();
        let slot = [
            leader.position[0] + 512.,
            leader.position[1],
            leader.position[2] - 512.,
        ];
        assert!(
            distance(wingman.position, slot) < initial_error,
            "wingman did not close on its slot: {:?} vs {:?}",
            wingman.position,
            slot
        );
        assert_eq!(mission.actor(2).unwrap().activity(), Activity::Formation);
    }

    #[test]
    fn a_wingman_obeys_break_formation_and_engage_orders() {
        use crate::ai::wing::{
            AppliedSetting, Formation as F, PlayerBreak, ReceiverOutcome, TargetId, TargetOrder,
            WingRequest,
        };
        let mut mission = AiMission::new();
        mission.push(AiActor::new(setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0)).unwrap());
        mission.push(AiActor::new(setup(2, 1, 1, [2000.0, 20000.0, -2000.0], 0.0)).unwrap());
        mission.push(
            AiActor::new(setup(
                3,
                2,
                0,
                [0.0, 20000.0, 30000.0],
                std::f64::consts::PI,
            ))
            .unwrap(),
        );
        run(&mut mission, 240);

        // Break: motion is installed and the wingman's heading actually moves.
        let heading_before = mission.actor(2).unwrap().flight().yaw;
        let outcome = mission
            .order(2, PlayerBreak::Right.request())
            .unwrap()
            .unwrap();
        assert!(matches!(outcome, ReceiverOutcome::MotionInstalled(_)));
        // Allow physical roll reversal and lift response to develop.
        run(&mut mission, 720);
        let heading_after = mission.actor(2).unwrap().flight().yaw;
        assert!(
            (heading_after - heading_before).abs() > 0.1,
            "the wingman did not turn on a break order"
        );

        // Formation: a setting, not a relocation.
        let outcome = mission
            .order(2, WingRequest::FormationSelection(F::LineAstern))
            .unwrap()
            .unwrap();
        assert!(matches!(
            outcome,
            ReceiverOutcome::Applied(AppliedSetting::FormationSelection { .. })
        ));

        // Engage: the named target is taken.
        let outcome = mission
            .order(
                2,
                WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(3))),
            )
            .unwrap()
            .unwrap();
        assert!(matches!(
            outcome,
            ReceiverOutcome::Applied(AppliedSetting::TargetOrder { .. })
        ));
        assert_eq!(mission.actor(2).unwrap().controller().target(), Some(3));
    }

    #[test]
    fn an_actor_on_bingo_fuel_turns_for_home() {
        let mut mission = one_v_one();
        // Put actor 1 a long way from home and nearly dry, so endurance falls
        // under time-to-home plus five minutes (B48 bingo).
        {
            let actor = mission.actor_mut(1).unwrap();
            actor.flight_mut().position = [0.0, 20000.0, 400_000.0];
            actor.set_internal_fuel(120.0);
        }
        let out = run(&mut mission, 240);
        assert!(
            out.activities
                .iter()
                .any(|(id, a)| *id == 1 && *a == Activity::ReturningToBase),
            "a bingo actor never reported returning to base"
        );
        // The leader and singleton route home is fitted; it must be recorded.
        assert!(
            out.fallbacks
                .iter()
                .any(|(id, f)| *id == 1 && *f == Fallback::LeaderReturnToBase)
        );
    }

    #[test]
    fn an_actor_out_of_fuel_reports_it() {
        let mut mission = one_v_one();
        mission.actor_mut(1).unwrap().set_internal_fuel(0.0);
        let out = run(&mut mission, 120);
        assert!(
            out.activities
                .iter()
                .any(|(id, a)| *id == 1 && *a == Activity::OutOfFuel)
        );
    }

    #[test]
    fn stores_point_at_the_retained_target_not_the_first_world_object() {
        // Two enemies: a far one listed first and a near one listed second.
        // The station view must describe the actor's own retained target.
        let mut mission = AiMission::new();
        mission.push(AiActor::new(setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0)).unwrap());
        mission.push(
            AiActor::new(setup(
                2,
                2,
                0,
                [0.0, 20000.0, 90000.0],
                std::f64::consts::PI,
            ))
            .unwrap(),
        );
        mission.push(
            AiActor::new(setup(3, 2, 0, [0.0, 20000.0, 9000.0], std::f64::consts::PI)).unwrap(),
        );
        run(&mut mission, 240);
        let actor = mission.actor(1).unwrap();
        let retained = actor.controller().target().expect("no target retained");
        let world = world_of(&mission);
        let own = actor.own_state(&flat);
        let permitted: Vec<u32> = world.iter().map(|o| o.id).collect();
        let targets = actor.target_views(&permitted, &world);
        let views = actor.station_views(&targets, &own);
        let expected = targets.iter().find(|t| t.id == retained).unwrap();
        let expected_error = pointing_error_deg(&own, expected.position);
        assert!(
            (views[0].pointing_error_deg - expected_error).abs() < 1e-9,
            "stores aimed at the wrong aircraft"
        );
    }

    #[test]
    fn fitted_fallbacks_are_reported_per_actor() {
        let mut mission = one_v_one();
        let out = run(&mut mission, 600);
        assert!(!out.fallbacks.is_empty());
        for (actor, _) in &out.fallbacks {
            assert!(mission.actor(*actor).is_some());
        }
        // The engagement pitch is unavoidable once an actor has a target.
        assert!(
            out.fallbacks
                .iter()
                .any(|(_, f)| *f == Fallback::EngagementPitch)
        );
    }

    #[test]
    fn speed_limits_come_from_the_loaded_envelopes() {
        let mission = one_v_one();
        let actor = mission.actor(1).unwrap();
        let limits = actor.speed_limits();
        assert!(limits.minimum.0 > 0.0);
        assert!(limits.maximum.0 > limits.minimum.0);
        assert!(limits.corner.0 <= limits.maximum.0);
        assert!(limits.corner.0 >= limits.minimum.0);
    }

    #[test]
    fn the_experience_g_adjustment_reaches_the_actor() {
        let novice = {
            let mut s = setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0);
            s.experience = resolved(Experience::Novice);
            AiActor::new(s).unwrap()
        };
        let ace = {
            let mut s = setup(2, 1, 0, [0.0, 20000.0, 0.0], 0.0);
            s.experience = resolved(Experience::Ace);
            AiActor::new(s).unwrap()
        };
        let (novice_g, _) = novice.g_limits();
        let (ace_g, _) = ace.g_limits();
        assert!(
            novice_g < ace_g,
            "Novice {novice_g} should lose a G against Ace {ace_g}"
        );
        assert!(novice_g >= 2.0, "the floor is 2 G, got {novice_g}");
    }

    #[test]
    fn simple_stations_and_dispensers_are_finite() {
        let stations = simple_stations(4, 500, ScalarSpeed(2000.0));
        assert_eq!(stations.len(), 2);
        assert!(
            stations
                .iter()
                .all(|s| !matches!(s.store.rounds, Rounds::Unlimited))
        );
        assert_eq!(simple_dispensers(30).len(), 2);
        assert!(simple_stations(0, 0, ScalarSpeed(2000.0)).is_empty());
    }

    #[test]
    fn every_ported_aircraft_is_recognised() {
        for aircraft in AircraftId::ALL {
            assert!(is_ported(aircraft));
        }
    }

    #[test]
    fn pointing_error_takes_the_larger_axis() {
        let own = OwnState {
            position: [0.0, 0.0, 0.0],
            heading_deg: 0.0,
            flight_path_pitch_deg: 0.0,
            body_pitch_offset_deg: 0.0,
            bank_deg: 0.0,
            speed: ScalarSpeed(800.0),
            limits: SpeedLimits {
                minimum: ScalarSpeed(220.0),
                maximum: ScalarSpeed(1600.0),
                corner: ScalarSpeed(700.0),
            },
            altitude_msl_ft: 0.0,
            agl_ft: 0.0,
            terrain_ahead_ft: 0.0,
            minimum_altitude_ft: 300.0,
            at_ceiling: false,
            on_ground: false,
            g_limit: 7.0,
            roll_limit_deg_per_s: 180.0,
            maximum_bank_deg: 80.0,
            alive: true,
            fuel_endurance_s: 3600.0,
            time_home_s: None,
            internal_fuel_lbs: 8000.0,
            radar_emitting: true,
        };
        // Straight ahead is zero error.
        assert!(pointing_error_deg(&own, [0.0, 0.0, 1000.0]).abs() < 1e-9);
        // Ninety degrees right.
        assert!((pointing_error_deg(&own, [1000.0, 0.0, 0.0]) - 90.0).abs() < 1e-9);
        // Directly above is ninety in the pitch axis.
        assert!((pointing_error_deg(&own, [0.0, 1000.0, 0.0]) - 90.0).abs() < 1e-9);
    }
    #[test]
    fn scheduled_devices_are_single_quarter_second_releases_and_stop_when_empty() {
        let mut mission = AiMission::new();
        let mut actor = AiActor::new(setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0)).unwrap();
        actor.dispensers[0].count = 2;
        actor.device_schedule.push((0, SeekerClass::Infrared, 3));
        mission.push(actor);
        let mut releases = Vec::new();
        for tick in 0..100 {
            let output = mission.step(&[], &flat, TimeOfDay(tick)).unwrap();
            for device in output.devices {
                releases.push((tick, device.released));
            }
        }
        assert_eq!(releases, [(0, 1), (30, 1)]);
        assert_eq!(mission.actor(1).unwrap().dispensers[0].count, 0);
        assert_eq!(mission.actor(1).unwrap().dispensers[1].count, 30);
    }

    #[test]
    fn releases_remove_only_the_mass_of_external_rounds_actually_debited() {
        let mut actor = AiActor::new(setup(1, 1, 0, [0., 20000., 0.], 0.)).unwrap();
        actor.stations = simple_stations(2, 10, ScalarSpeed(2000.));
        actor.stations[0].external_round_lbs = 250.;
        actor.flight.set_payload(600.).unwrap(); // 100 lb fixed equipment remains.
        let mut intent = super::super::controller::WeaponIntent {
            request: weapon_service::FireRequest {
                actor: ActorId(1),
                station: StationId(0),
                target: super::super::weapon_service::TargetId(2),
                request_id: super::super::weapon_service::RequestId(1),
            },
        };
        for expected in [350., 100.] {
            assert!(actor.release(&intent, &IntentBatch::default()).is_some());
            assert_eq!(actor.flight.payload_lbs, expected);
        }
        assert!(actor.release(&intent, &IntentBatch::default()).is_none());
        assert_eq!(actor.flight.payload_lbs, 100.);
        intent.request.station = StationId(1);
        assert!(actor.release(&intent, &IntentBatch::default()).is_some());
        assert_eq!(
            actor.flight.payload_lbs, 100.,
            "internal gun uses the shared zero external-mass convention"
        );
    }

    #[test]
    fn station_scoring_receives_error_and_failed_envelopes() {
        let actor = AiActor::new(setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0)).unwrap();
        let own = actor.own_state(&flat);
        let mut target = object(&actor, 2);
        target.id = 2;
        target.position = [0.0, 20000.0, 1000.0];
        let views = actor.target_views(&[2], &[target.clone()]);
        assert_eq!(
            actor.station_views(&views, &own)[0].employment_fit,
            Some(0.0)
        );
        target.position = [2000.0, 20000.0, 1000.0];
        let views = actor.target_views(&[2], &[target]);
        assert_eq!(actor.station_views(&views, &own)[0].employment_fit, None);
    }

    #[test]
    fn wing_orders_do_not_cross_side_or_wing_boundaries() {
        use super::super::wing::{PlayerBreak, TargetOrder, WingRequest};
        let mut mission = AiMission::new();
        for (id, side, wing) in [(1, 1, 0), (2, 1, 1), (3, 2, 0)] {
            let mut s = setup(id, side, 1, [0.0, 20000.0, 0.0], 0.0);
            s.identity.wing = wing;
            mission.push(AiActor::new(s).unwrap());
        }
        run(&mut mission, 1);
        assert_eq!(
            mission
                .order_wing(
                    super::super::targeting::Side(1),
                    0,
                    None,
                    PlayerBreak::Right.request()
                )
                .unwrap(),
            1
        );
        mission
            .order_wing(
                super::super::targeting::Side(1),
                0,
                None,
                WingRequest::TargetAssignment(TargetOrder::HoldFire),
            )
            .unwrap();
        run(&mut mission, 2);
        assert_eq!(mission.actor(1).unwrap().controller.target(), None);
        assert!(mission.actor(2).unwrap().controller.target().is_some());
        assert!(mission.actor(3).unwrap().controller.target().is_some());
    }

    #[test]
    fn all_models_move_only_through_recorded_inputs_including_reversal_and_damage() {
        use super::super::{
            controller::Completion,
            motion::{Bank, Duration, MotionRequest, PitchRequest, SpeedRequest},
            steering::CommandMode,
        };
        for aircraft in AircraftId::ALL {
            for (health, hybrid) in [(1.0, false), (0.25, false), (1.0, true), (0.25, true)] {
                let mut setup = setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0);
                setup.identity.aircraft = aircraft;
                setup.flight =
                    flight::State::new(&synthetic_profile(aircraft), setup.flight.position)
                        .unwrap();
                if hybrid {
                    setup.flight.enable_research(1).unwrap();
                }
                setup.flight.damage_fraction = 1.0 - health;
                setup.flight.set_payload(500.0).unwrap();
                let mut actor = AiActor::new(setup).unwrap();
                for tick in 0..600 {
                    if tick == 450 {
                        actor.set_internal_fuel(0.0);
                    }
                    let heading = if tick < 300 { 90 } else { 270 };
                    let own = actor.own_state(&flat);
                    let before = actor.flight.clone();
                    let intent = MotionIntent {
                        formation_flight: false,
                        afterburner: tick < 300,
                        id: 1,
                        request: MotionRequest::new(
                            heading,
                            PitchRequest::Explicit(0),
                            Bank::Unconstrained,
                            SpeedRequest::Corner,
                            Duration::Timed(5),
                        ),
                        heading_deg: f64::from(heading),
                        flight_path_pitch_deg: 0.0,
                        speed: own.limits.corner,
                        bank: Bank::Unconstrained,
                        completion: Completion::Deadline(super::super::motion::Deadline(300)),
                        steering_point: None,
                        mode: CommandMode::OtherState,
                    };
                    actor.fly(Some(&intent), &own, &flat).unwrap();
                    let mut replay = before;
                    replay.step_surface(actor.last_input(), |x, z| Surface::terrain(flat(x, z)));
                    assert_eq!(
                        actor.flight, replay,
                        "{aircraft:?}: AI motion differs from input replay"
                    );
                    assert!(actor.flight.position.iter().all(|v| v.is_finite()));
                    if tick >= 450 {
                        assert!(!actor.flight.engine);
                    }
                }
            }
        }
    }

    #[test]
    fn wing_assignments_reach_live_ranking_without_counting_other_wings() {
        use super::super::wing::{TargetId, TargetOrder, WingRequest};
        let mut mission = AiMission::new();
        for (id, side, wing, z) in [
            (1, 1, 0, 0.0),
            (2, 1, 0, 0.0),
            (3, 1, 0, 0.0),
            (4, 1, 1, 0.0),
            (10, 2, 0, 9000.0),
            (11, 2, 0, 12000.0),
        ] {
            let mut setup = setup(id, side, 0, [0.0, 20000.0, z], 0.0);
            setup.identity.wing = wing;
            mission.push(AiActor::new(setup).unwrap());
        }
        for (actor, target) in [(2, 10), (3, 10), (4, 11)] {
            mission
                .order(
                    actor,
                    WingRequest::TargetAssignment(TargetOrder::ConcreteTarget(TargetId(target))),
                )
                .unwrap()
                .unwrap();
        }
        run(&mut mission, 1);
        assert_eq!(mission.actor(1).unwrap().controller.target(), Some(11));
    }

    #[test]
    fn equal_aligned_stores_keep_the_first_station_instead_of_rewarding_narrow_cones() {
        let mut setup = setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0);
        setup.stations.truncate(1);
        let mut narrow = setup.stations[0].clone();
        narrow.station = StationId(7);
        narrow.employment_limit_deg = Some(10.0);
        setup.stations.push(narrow);
        let mut actor = AiActor::new(setup).unwrap();
        let own = actor.own_state(&flat);
        let mut target = object(&actor, 2);
        target.id = 2;
        target.position[2] = 2000.0;
        let targets = actor.target_views(&[2], &[target]);
        let stations = actor.station_views(&targets, &own);
        let mut fired = None;
        for tick in 0..2400 {
            let mut frame = dummy_frame(&own);
            frame.tick = tick;
            frame.targets = &targets;
            frame.stations = &stations;
            let output = actor.controller.step(&frame).unwrap();
            if let Some(weapon) = output.weapons.first() {
                fired = Some(weapon.request.station);
                break;
            }
        }
        assert_eq!(fired, Some(StationId(0)));
    }
    #[test]
    fn terrain_between_live_actors_blocks_release_without_spending_ammunition() {
        let mut mission = AiMission::new();
        mission.push(AiActor::new(setup(1, 1, 0, [0.0, 20000.0, 0.0], 0.0)).unwrap());
        mission.push(
            AiActor::new(setup(2, 2, 0, [0.0, 20000.0, 5000.0], std::f64::consts::PI)).unwrap(),
        );
        let initial: Vec<_> = mission.actors.iter().map(|a| a.flight.clone()).collect();
        let ammunition: Vec<_> = mission
            .actors
            .iter()
            .map(AiActor::rounds_remaining)
            .collect();
        let ridge = |_x: f64, z: f64| {
            if (2000.0..3000.0).contains(&z) {
                21000.0
            } else {
                0.0
            }
        };
        for tick in 0..2400 {
            for (actor, flight) in mission.actors.iter_mut().zip(&initial) {
                actor.flight = flight.clone();
            }
            let world = world_of(&mission);
            assert!(
                mission
                    .step(&world, &ridge, TimeOfDay(tick))
                    .unwrap()
                    .launches
                    .is_empty()
            );
        }
        assert_eq!(
            mission
                .actors
                .iter()
                .map(AiActor::rounds_remaining)
                .collect::<Vec<_>>(),
            ammunition
        );
    }
    #[test]
    fn directed_delivery_checks_sender_recipient_and_initial_physical_heading() {
        use super::super::targeting::Side;
        use super::super::wing::{PlayerBreak, ReceiverOutcome};
        let mut mission = AiMission::new();
        for (id, side) in [(1, 1), (2, 1), (3, 2)] {
            mission.push(AiActor::new(setup(id, side, 1, [0., 20000., 0.], 90.)).unwrap());
        }
        let report = mission
            .order_wing_report(Side(1), 0, Some(1), Some(3), PlayerBreak::Left.request())
            .unwrap();
        assert!(report.is_empty());
        assert!(
            mission
                .order_wing_report(Side(1), 0, Some(3), Some(2), PlayerBreak::Left.request())
                .is_err()
        );
        let heading = mission.actor(2).unwrap().flight().yaw.to_degrees().round() as i32;
        let report = mission
            .order_wing_report(Side(1), 0, Some(1), Some(2), PlayerBreak::Right.request())
            .unwrap();
        assert_eq!(report.len(), 1);
        let ReceiverOutcome::MotionInstalled(motion) = report[0].1 else {
            panic!("no motion");
        };
        assert_eq!(motion.heading_deg, (heading + 170).rem_euclid(360));
        assert!(motion.speed.0 > 0., "first-tick order needs real limits");
    }
}
