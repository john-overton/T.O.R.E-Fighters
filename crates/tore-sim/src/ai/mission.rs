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

use super::{defense, engagement};
use crate::combat::threats::{MissileSnapshot, Receiver, ThreatRecord, ThreatService};
use crate::flight;
use crate::models::FlightModel;
use crate::research::Surface;
use crate::sensors::{self, Observable, Observer, Sensors};

use super::awareness::{self, Memory, Observation, ObservationSource};
use super::controller::{
    Activity, ActorIdentity, BehaviorProfile, Controller, DecisionFrame, FrameEvent, IntentBatch,
    LeaderView, MotionIntent, OwnState, RouteView, SearchContact, StationView, TargetView,
    ThreatReport, WingView,
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
    /// Supported by the ground (wheels on a runway), not flying.
    pub on_ground: bool,
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
    /// Accepted opposite-side launch warnings as (actor, launcher), for the
    /// radio's "SAM launch" and "AAM launch" calls.
    pub launch_calls: Vec<(u32, u32)>,
}

/// The mission's airfield decisions for one actor this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct AirfieldClearance {
    turn: bool,
    runway_free: bool,
    wing_landed: bool,
    free_slot: Option<u32>,
    leader_landing: bool,
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

/// Perceived attack information shared with assigned escorts. The bearing is
/// world-relative, not an invented target position or fire-control solution.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObservedAttack {
    pub report: engagement::ThreatReport,
    pub bearing_world_deg: Option<f64>,
    pub observed_tick: u64,
    /// Observed projectile identity, never a hidden launcher identity.
    pub event_id: Option<u32>,
}

/// One AI-flown aircraft and everything it owns.
pub struct AiActor {
    identity: ActorIdentity,
    controller: Controller,
    adapter: ControlAdapter,
    flight: flight::State,
    sensors: Option<Sensors>,
    awareness: Memory,
    search_target: Option<u32>,
    missile_threats: ThreatService,
    defense_state: defense::DefenseState,
    last_defense: Option<defense::DefenseDecision>,
    assignment: engagement::Assignment,
    mission_policy: engagement::Policy,
    observed_attacks: Vec<ObservedAttack>,
    neutral: bool,
    formation_order_tick: Option<u64>,
    ignored_attack_ids: Vec<u32>,
    received_emitters: Vec<sensors::passive::Emitter>,
    stations: Vec<StationSpec>,
    dispensers: Vec<DispenserStore>,
    wing_slot: u8,
    home_airport: Option<super::route::Position>,
    /// The runway this aircraft returns to, when the host knows one.
    home_runway: Option<super::airfield::RunwayView>,
    /// Set when the aircraft begins the mission parked on a runway.
    ground_start: Option<super::airfield::GroundStart>,
    /// The landing this aircraft is flying or will resume, if any.
    landing_order: Option<super::airfield::LandingOrder>,
    /// Set once a bug-out order is accepted; the aircraft then ignores orders.
    bugged_out: bool,
    /// A join-the-leader landing was cancelled by order; it is not re-joined
    /// until the leader is no longer recovering (fitted, 2026-09-23).
    join_cancelled: bool,
    /// The running takeoff or landing sequence, if any.
    airfield: Option<super::airfield::Sequence>,
    /// Draws for the private return route, separate from decision draws.
    route_random: super::DecisionRandom,
    pending_threats: Vec<ThreatReport>,
    pending_events: Vec<FrameEvent>,
    device_schedule: Vec<(u64, SeekerClass, u8)>,
    activity: Activity,
    last_input: PilotInput,
    alive: bool,
    dummy: bool,
    escape_monitor: crate::ejection::Monitor,
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
            awareness: Memory::new(setup.experience, setup.identity.side),
            search_target: None,
            missile_threats: ThreatService::new(setup.identity.actor.0),
            defense_state: defense::DefenseState::default(),
            last_defense: None,
            assignment: engagement::Assignment::default(),
            mission_policy: engagement::Policy::default(),
            observed_attacks: Vec::new(),
            // Standalone simulation fixtures supply authorized assignments.
            // Quick Mission explicitly initializes every wing as neutral.
            neutral: false,
            formation_order_tick: None,
            ignored_attack_ids: Vec::new(),
            received_emitters: Vec::new(),
            stations: setup.stations,
            dispensers: setup.dispensers,
            wing_slot: setup.wing_slot,
            home_airport: setup.home_airport,
            home_runway: None,
            ground_start: None,
            landing_order: None,
            bugged_out: false,
            join_cancelled: false,
            airfield: None,
            route_random: super::DecisionRandom::seeded(setup.seed ^ 0x6c61_6e64_696e_6721),
            pending_threats: Vec::new(),
            pending_events: Vec::new(),
            device_schedule: Vec::new(),
            activity: Activity::Idle,
            last_input: PilotInput::default(),
            escape_monitor: crate::ejection::Monitor::seeded(
                setup.seed ^ u64::from(setup.identity.actor.0).wrapping_mul(0x9e3779b97f4a7c15),
            ),
            alive: true,
            dummy: false,
        })
    }

    /// Begin the mission parked on `start.runway`. The flight model must
    /// already be on the researched adapter and placed with
    /// `flight::State::start_on_runway`: the position read here picks the
    /// parking slot (with airport anchors) or the line-up spot (without).
    pub fn start_on_ground(&mut self, start: super::airfield::GroundStart) {
        self.home_runway = Some(start.runway);
        self.ground_start = Some(start);
        let position = self.flight.position;
        // With airport anchors a ground start sitting on a parking slot holds it.
        let slot = start.runway.anchors.and_then(|anchors| {
            (0..super::airfield::PARKING_SLOTS).find(|&k| {
                let p = anchors.parking[k as usize];
                (p[0] - position[0]).hypot(p[2] - position[2]) <= 100.0
            })
        });
        self.airfield = Some(super::airfield::Sequence::departure(start, position, slot));
    }

    pub fn ground_start(&self) -> Option<&super::airfield::GroundStart> {
        self.ground_start.as_ref()
    }

    /// The runway used for return to base and bingo fuel.
    pub fn set_home_runway(&mut self, runway: Option<super::airfield::RunwayView>) {
        self.home_runway = runway;
    }

    pub fn home_runway(&self) -> Option<&super::airfield::RunwayView> {
        self.home_runway.as_ref()
    }

    /// The landing this aircraft is flying, or will resume after a missile
    /// defense interrupts its early approach.
    pub fn landing_order(&self) -> Option<&super::airfield::LandingOrder> {
        self.landing_order.as_ref()
    }

    /// True once a bug-out order was accepted. It stays true: the aircraft
    /// returns to base and no longer answers wing orders (manual p.160).
    pub fn bugged_out(&self) -> bool {
        self.bugged_out
    }

    /// The current airfield phase, `None` in free flight.
    pub fn airfield_phase(&self) -> Option<super::airfield::Phase> {
        self.airfield.as_ref().map(|s| s.phase())
    }

    /// The running takeoff or landing sequence, for diagnostics.
    pub fn airfield(&self) -> Option<&super::airfield::Sequence> {
        self.airfield.as_ref()
    }

    /// Opinionated training target requested on 2026-09-21.
    pub fn set_dummy(&mut self) {
        self.dummy = true;
        self.sensors = None;
        self.flight.radar = false;
        self.flight.pitch = 0.;
        self.flight.bank = 0.;
        self.flight.speed = super::launch::DUMMY_SPEED_FPS;
        self.flight.velocity = crate::attitude::Basis::new(self.flight.yaw, 0., 0.)
            .forward
            .map(|v| v * super::launch::DUMMY_SPEED_FPS);
    }

    pub fn is_dummy(&self) -> bool {
        self.dummy
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

    /// Current observations and frozen records, exposed for diagnostics only.
    pub fn awareness(&self) -> &Memory {
        &self.awareness
    }

    pub fn assignment(&self) -> &engagement::Assignment {
        &self.assignment
    }

    pub fn set_assignment(&mut self, assignment: engagement::Assignment) {
        self.assignment = assignment;
        self.mission_policy.reset();
        self.controller.set_mission_target(None);
        self.controller
            .set_mission_hold_fire(self.assignment.stance == engagement::Stance::WeaponsHold);
        self.controller.set_mission_rejoin(None);
        self.search_target = None;
        self.controller.set_search_contact(None);
    }

    pub fn perceived_attacks(&self) -> &[ObservedAttack] {
        &self.observed_attacks
    }

    pub fn is_neutral(&self) -> bool {
        self.neutral
    }

    pub fn return_to_formation(&mut self, tick: u64) {
        self.neutral = true;
        self.formation_order_tick = Some(tick);
        self.ignored_attack_ids = self
            .observed_attacks
            .iter()
            .filter_map(|a| a.event_id)
            .chain(self.missile_threats.records().map(|r| r.missile_id))
            .collect();
        self.ignored_attack_ids.sort_unstable();
        self.ignored_attack_ids.dedup();
        self.controller.return_to_formation();
        self.activity = Activity::Formation;
        self.search_target = None;
        self.mission_policy.reset();
        if let Some(sensors) = self.sensors.as_mut() {
            sensors.clear_selection();
        }
    }

    fn permits_attack_response(&self, attack: &ObservedAttack) -> bool {
        !self.neutral
            || (self
                .formation_order_tick
                .is_none_or(|tick| attack.observed_tick > tick)
                && attack
                    .event_id
                    .is_none_or(|id| !self.ignored_attack_ids.contains(&id)))
    }

    fn remember_attack(&mut self, attack: ObservedAttack) {
        if let Some(previous) = self.observed_attacks.iter_mut().find(|previous| {
            previous.report == attack.report && previous.event_id == attack.event_id
        }) {
            *previous = attack;
        } else {
            self.observed_attacks.push(attack);
        }
    }

    pub fn missile_threats(&self) -> impl Iterator<Item = &ThreatRecord> {
        self.missile_threats.records()
    }

    pub fn defense_decision(&self) -> Option<defense::DefenseDecision> {
        self.last_defense
    }

    pub fn stations(&self) -> &[StationSpec] {
        &self.stations
    }

    pub fn experience(&self) -> ResolvedExperience {
        self.controller.experience()
    }

    /// Change skill mid-flight, for decisions and awareness alike.
    pub fn set_experience(&mut self, experience: ResolvedExperience) {
        self.controller.set_experience(experience);
        self.awareness.set_experience(experience);
    }

    pub fn stations_mut(&mut self) -> &mut [StationSpec] {
        &mut self.stations
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
        if !alive {
            self.awareness.clear();
            self.missile_threats.clear();
            self.defense_state = defense::DefenseState::default();
            self.last_defense = None;
            self.controller.set_missile_defense(None);
            self.search_target = None;
            self.controller.set_search_contact(None);
        }
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
        use super::wing::{ReceiverOutcome, RejectReason, TargetOrder, WingRequest};
        if self.dummy {
            return Ok(ReceiverOutcome::Rejected(RejectReason::Dummy));
        }
        // Manual p.160: a wingman that bugged out no longer answers.
        if self.bugged_out {
            return Ok(ReceiverOutcome::Rejected(RejectReason::BuggedOut));
        }
        let phase = self.airfield_phase();
        if phase == Some(super::airfield::Phase::Parked) {
            return Ok(ReceiverOutcome::Rejected(RejectReason::Landed));
        }
        if let WingRequest::Land(order) = request {
            // Retail bug out is ignored in any airport state or on the ground;
            // the private route home is free flight and does not count.
            if order.reason == super::airfield::LandingReason::BugOut
                && (phase.is_some_and(|p| p != super::airfield::Phase::Inbound)
                    || self.flight.research.as_ref().is_some_and(|r| r.on_ground))
            {
                return Ok(ReceiverOutcome::Rejected(RejectReason::OnAirfield));
            }
            self.accept_landing(order, tick);
        } else if matches!(
            request,
            WingRequest::FormationSelection(_)
                | WingRequest::TargetAssignment(TargetOrder::HoldFire)
        ) && self.landing_order.is_some_and(|o| o.reason.cancellable())
            && phase.is_none_or(|p| {
                matches!(
                    p,
                    super::airfield::Phase::Inbound
                        | super::airfield::Phase::Marshal
                        | super::airfield::Phase::Approach
                )
            })
        {
            // Fitted: disengage and formation orders cancel an ordered
            // landing that has not yet reached final. A cancelled join
            // stays cancelled until the leader is next airborne.
            if self
                .landing_order
                .is_some_and(|o| o.reason == super::airfield::LandingReason::JoinLeader)
            {
                self.join_cancelled = true;
            }
            self.landing_order = None;
            if self.airfield.as_ref().is_some_and(|s| !s.is_departure()) {
                self.leave_airfield();
            }
        }
        self.controller
            .prepare_order(self.flight.yaw.to_degrees(), self.speed_limits());
        let outcome = self.controller.receive_order(request, tick)?;
        if matches!(
            outcome,
            super::wing::ReceiverOutcome::Applied(_)
                | super::wing::ReceiverOutcome::MotionInstalled(_)
        ) {
            match request {
                super::wing::WingRequest::TargetAssignment(
                    super::wing::TargetOrder::ConcreteTarget(id),
                ) => {
                    self.neutral = false;
                    self.set_assignment(engagement::Assignment {
                        role: engagement::Role::Intercept,
                        stance: engagement::Stance::EngageAssigned,
                        destroy_ids: vec![id.0],
                        ..engagement::Assignment::default()
                    });
                }
                super::wing::WingRequest::TargetAssignment(super::wing::TargetOrder::HoldFire) => {
                    self.return_to_formation(tick);
                }
                super::wing::WingRequest::FormationSelection(_) => self.return_to_formation(tick),
                super::wing::WingRequest::TargetAssignment(
                    super::wing::TargetOrder::FreeSelection,
                ) => {
                    self.neutral = false;
                }
                _ => {}
            }
        }
        Ok(outcome)
    }

    /// Take a landing order. A bug out also leaves the wing for good.
    fn accept_landing(&mut self, order: super::airfield::LandingOrder, tick: u64) {
        use super::airfield::Phase;
        if order.reason == super::airfield::LandingReason::BugOut {
            self.bugged_out = true;
            self.neutral = true;
            self.formation_order_tick = Some(tick);
            self.controller.return_to_formation();
            self.search_target = None;
        }
        self.landing_order = Some(order);
        // A landing already committed to final at the same runway continues;
        // any other landing restarts toward the new runway. A departure in
        // progress finishes its climb-out first.
        if let Some(sequence) = &self.airfield
            && !sequence.is_departure()
        {
            let committed = sequence.runway().object == order.runway.object
                && !matches!(
                    sequence.phase(),
                    Phase::Inbound | Phase::Marshal | Phase::Approach
                );
            if !committed {
                self.leave_airfield();
            }
        }
    }

    /// Back to free flight from a landing sequence: the takeoff-finish tail
    /// (retail `0x4bbfe0`) raises the gear and flaps and closes the
    /// speedbrake, so nothing is left hanging in combat.
    fn leave_airfield(&mut self) {
        use tore_input::{PilotCommand, Switch};
        self.airfield = None;
        self.flight.command(PilotCommand::Set(Switch::Gear, false));
        self.flight.command(PilotCommand::Set(Switch::Flaps, false));
        self.flight
            .command(PilotCommand::Set(Switch::Airbrake, false));
    }

    /// Start the landing sequence for the stored order. Legacy-model aircraft
    /// switch to the researched model here so runway contact is modelled.
    fn begin_landing(
        &mut self,
        order: super::airfield::LandingOrder,
        own: &OwnState,
        surface: &dyn Fn(f64, f64) -> Surface,
    ) {
        if self.flight.research.is_none() {
            let seed = (self.identity.actor.0 as i32).wrapping_mul(7919) | 1;
            if self.flight.enable_research(seed).is_err() {
                // The restricted native adapter cannot land: keep the
                // ordinary return-to-base heading instead.
                self.landing_order = None;
                return;
            }
        }
        // B48 join-landing enters the marshal directly; every other landing
        // first flies the private route home.
        let joining = order.reason == super::airfield::LandingReason::JoinLeader;
        let route = super::route::bingo_route(
            super::route::WingCrew {
                wingman_ai: true,
                leader_ai: true,
            },
            super::route::Position {
                x: order.runway.center[0],
                z: order.runway.center[2],
            },
            &own.limits,
            &mut self.route_random,
        )
        .map(|r| (f64::from(r.altitude_ft), r.speed.0))
        .filter(|_| !joining);
        let wind = surface(order.runway.center[0], order.runway.center[2]).wind;
        self.airfield = Some(super::airfield::Sequence::landing(
            order,
            self.flight.position,
            wind,
            self.flight.model().configuration().mass.max_takeoff_lbs,
            route,
        ));
    }

    /// One tick of a takeoff or landing sequence. Returns `false` when the
    /// aircraft is in free flight and the ordinary controller should run.
    #[allow(clippy::too_many_arguments)]
    fn airfield_tick(
        &mut self,
        clearance: &AirfieldClearance,
        own: &OwnState,
        tick: u64,
        ground: &dyn Fn(f64, f64) -> f64,
        surface: &dyn Fn(f64, f64) -> Surface,
        output: &mut MissionOutput,
    ) -> Result<bool> {
        use super::airfield::{Control, LandingReason, Phase, Situation};
        let defending = self.last_defense.is_some_and(|d| d.motion.is_some());
        let warned = self.pending_threats.iter().any(|r| !r.launcher_same_side);
        if self.airfield.is_none()
            && let Some(order) = self.landing_order
            && !defending
            && !warned
        {
            self.begin_landing(order, own, surface);
        }
        let Some(phase) = self.airfield.as_ref().map(|s| s.phase()) else {
            return Ok(false);
        };
        if !clearance.leader_landing
            && matches!(phase, Phase::Inbound | Phase::Marshal | Phase::Approach)
            && self
                .landing_order
                .is_some_and(|o| o.reason == LandingReason::JoinLeader)
        {
            // Retail wing abort: gear and flaps up, free flight.
            self.landing_order = None;
            self.leave_airfield();
            return Ok(false);
        }
        match phase.flight_state() {
            // B47: a warning in the first approach states abandons the
            // approach; the landing resumes once the threat has gone.
            // Free flight on the inbound route reacts like any other flight.
            FlightState::EarlyApproach | FlightState::Free if defending || warned => {
                self.leave_airfield();
                return Ok(false);
            }
            // B47: warnings are ignored while taking off and landing.
            _ => {
                self.pending_threats.clear();
                self.pending_events.clear();
            }
        }
        let config = self.flight.model().configuration();
        let situation = Situation {
            tick,
            position: self.flight.position,
            velocity: self.flight.velocity,
            heading_deg: self.flight.yaw.to_degrees(),
            body_pitch_deg: self.flight.pitch.to_degrees(),
            speed_fps: self.flight.speed,
            on_ground: self.flight.research.as_ref().is_some_and(|r| r.on_ground),
            ground_clearance_ft: config.equipment.ground_clearance_ft,
            agl_ft: own.agl_ft,
            terrain_ahead_ft: {
                let [x, _, z] = self.flight.position;
                let [vx, _, vz] = self.flight.velocity;
                let speed = vx.hypot(vz).max(1.0);
                (0..=6)
                    .map(|i| {
                        let d = super::airfield::TERRAIN_LOOKAHEAD_FT * f64::from(i) / 6.0;
                        ground(x + vx / speed * d, z + vz / speed * d)
                    })
                    .fold(f64::MIN, f64::max)
            },
            minimum_speed_fps: own.limits.minimum.0,
            maximum_speed_fps: own.limits.maximum.0,
            corner_speed_fps: own.limits.corner.0,
            cruise_speed_fps: super::route::cruise_speed(&own.limits).0,
            has_afterburner: config.propulsion.afterburner_thrust_lbf > 0.0,
            wind: surface(self.flight.position[0], self.flight.position[2]).wind,
            wing_position: self.identity.member,
            turn_clear: clearance.turn,
            runway_free: clearance.runway_free,
            wing_landed: clearance.wing_landed,
            free_slot: clearance.free_slot,
        };
        let sequence = self.airfield.as_mut().expect("phase read above");
        let step = sequence.step(&situation);
        if step.complete {
            self.airfield = None;
        }
        let command = step.command;
        self.activity = command.activity;
        output.activities.push((self.id(), command.activity));
        let mut input = match command.control {
            Control::Ground {
                throttle,
                pitch,
                yaw,
            } => PilotInput {
                pitch,
                roll: 0.0,
                yaw,
                throttle: Some(throttle),
                ..PilotInput::default()
            },
            Control::Air(guidance) => {
                let clock = super::motion::CommandClock::at_tick(tick);
                let bank = if guidance.wings_level {
                    super::motion::Bank::Explicit(0)
                } else {
                    super::motion::Bank::Unconstrained
                };
                let duration = super::motion::Duration::Timed(1);
                let intent = MotionIntent {
                    id: 0,
                    request: super::motion::MotionRequest::new(
                        guidance.heading_deg.round() as i32,
                        super::motion::PitchRequest::Explicit(
                            guidance.flight_path_pitch_deg.round() as i32,
                        ),
                        bank,
                        super::motion::SpeedRequest::Explicit(ScalarSpeed(guidance.speed_fps)),
                        duration,
                    ),
                    heading_deg: guidance.heading_deg,
                    flight_path_pitch_deg: guidance.flight_path_pitch_deg,
                    speed: ScalarSpeed(guidance.speed_fps),
                    bank,
                    completion: super::controller::Completion::Deadline(
                        super::motion::deadline_for(duration, clock).expect("timed guidance"),
                    ),
                    steering_point: None,
                    mode: super::steering::CommandMode::OtherState,
                    formation_flight: false,
                    afterburner: command.afterburner,
                };
                let floor = if guidance.terrain_floor {
                    self.controller.terrain_floor(&dummy_frame(own))?
                } else {
                    None
                };
                let mut input = self
                    .adapter
                    .controls(
                        &self.flight,
                        &intent,
                        &own.limits,
                        own.g_limit,
                        own.roll_limit_deg_per_s,
                        own.maximum_bank_deg,
                        floor,
                        flight::DT,
                    )?
                    .input;
                if guidance.full_power {
                    input.throttle = Some(1.0);
                }
                input
            }
        };
        input.commands = vec![
            tore_input::PilotCommand::Set(tore_input::Switch::Burner, command.afterburner),
            tore_input::PilotCommand::Set(tore_input::Switch::Gear, command.gear_down),
            tore_input::PilotCommand::Set(tore_input::Switch::Flaps, command.flaps_down),
            tore_input::PilotCommand::Set(tore_input::Switch::Airbrake, command.brakes),
        ];
        self.fly_input(input, ground, surface);
        Ok(true)
    }

    /// The home position for fuel planning: the home airport, or the home
    /// runway's centre when only the runway is known.
    fn home(&self) -> Option<super::route::Position> {
        self.home_airport.or_else(|| {
            self.home_runway.map(|r| super::route::Position {
                x: r.center[0],
                z: r.center[2],
            })
        })
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
    missiles: Vec<MissileSnapshot>,
    player_assignment: engagement::Assignment,
    must_survive: Vec<u32>,
    pending_attack_reports: Vec<(u32, ObservedAttack)>,
    /// Airport where the human player holds landing clearance.
    priority_landing: Option<u32>,
    /// External leaders seen airborne, so a later touchdown reads as landing.
    airborne_seen: Vec<u32>,
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
            missiles: Vec::new(),
            player_assignment: engagement::Assignment::default(),
            must_survive: Vec::new(),
            pending_attack_reports: Vec::new(),
            priority_landing: None,
            airborne_seen: Vec::new(),
        }
    }

    /// The human player is landing at this airport (retail: gear down, below
    /// 4000 ft above ground, at most 953 ft/s, within 25000 ft of a friendly
    /// airport). It keeps that runway busy: AI aircraft landing there hold at
    /// marshal and none start a takeoff until it is cleared with `None`
    /// (manual p.65: "your aircraft always receives first landing
    /// clearance"). A plain store, cheap and idempotent to call every tick.
    pub fn set_priority_landing(&mut self, airport: Option<u32>) {
        self.priority_landing = airport;
    }

    pub fn priority_landing(&self) -> Option<u32> {
        self.priority_landing
    }

    pub fn must_survive(&self) -> &[u32] {
        &self.must_survive
    }

    /// Mission requirements do not silently replace aircraft combat orders.
    pub fn set_must_survive(&mut self, mut ids: Vec<u32>) {
        ids.sort_unstable();
        ids.dedup();
        self.must_survive = ids;
    }

    pub fn player_assignment(&self) -> &engagement::Assignment {
        &self.player_assignment
    }

    pub fn set_player_assignment(&mut self, assignment: engagement::Assignment) {
        self.player_assignment = assignment;
    }

    /// Quick Mission startup permission is independent of its group objectives.
    pub fn start_in_formation(&mut self) {
        for actor in &mut self.actors {
            actor.return_to_formation(self.tick);
            actor.formation_order_tick = None;
        }
    }

    /// Queue a legitimately perceived attack for the observer and its assigned
    /// friendly escorts on the next simulation step. Unknown attackers remain
    /// unknown; these reports never populate aircraft observation or memory.
    pub fn report_attack(&mut self, receiver: u32, report: engagement::ThreatReport) {
        self.report_attack_bearing(receiver, report, None);
    }

    pub fn report_attack_bearing(
        &mut self,
        receiver: u32,
        report: engagement::ThreatReport,
        bearing_world_deg: Option<f64>,
    ) {
        self.report_attack_evidence(receiver, report, bearing_world_deg, None);
    }

    pub fn report_attack_evidence(
        &mut self,
        receiver: u32,
        report: engagement::ThreatReport,
        bearing_world_deg: Option<f64>,
        event_id: Option<u32>,
    ) {
        if report.defended_id != receiver {
            return;
        }
        let identity = self
            .actor(receiver)
            .map(|a| (a.identity.side, a.identity.wing))
            .or_else(|| {
                self.external_leaders
                    .iter()
                    .find(|(_, _, id)| *id == receiver)
                    .map(|(side, wing, _)| (*side, *wing))
            });
        let Some((side, wing)) = identity else {
            return;
        };
        let attack = ObservedAttack {
            report,
            bearing_world_deg,
            observed_tick: self.tick,
            event_id,
        };
        for actor in &self.actors {
            if actor.alive()
                && actor.identity.side == side
                && (actor.id() == receiver
                    || actor.assignment.protected_ids.contains(&receiver)
                    || (actor.identity.is_leader() && actor.identity.wing == wing))
            {
                let key = (actor.id(), attack);
                if !self.pending_attack_reports.contains(&key) {
                    self.pending_attack_reports.push(key);
                }
            }
        }
    }

    /// Complete current missile lifecycle snapshot, shared with the player
    /// receiver. The observation service, not the controller, reads this data.
    pub fn set_missiles(&mut self, missiles: Vec<MissileSnapshot>) {
        self.missiles = missiles;
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
        self.step_with_surface(world, ground, &|x, z| Surface::terrain(ground(x, z)), now)
    }

    /// [`Self::step`] with the host's full surface query, so runways are
    /// landable for aircraft on the researched flight model. `terrain` is the
    /// plain terrain height: aircraft on the legacy adapter keep using it for
    /// their height above the ground, terrain floor and escape, as before.
    pub fn step_with_surface(
        &mut self,
        world: &[WorldObject],
        terrain: &dyn Fn(f64, f64) -> f64,
        surface: &dyn Fn(f64, f64) -> Surface,
        now: TimeOfDay,
    ) -> Result<MissionOutput> {
        let mut output = MissionOutput::default();
        for (receiver, report) in std::mem::take(&mut self.pending_attack_reports) {
            if let Some(actor) = self.actor_mut(receiver) {
                actor.remember_attack(report);
            }
        }
        let tick = self.tick;
        self.track_airborne(world);

        let traffic: Vec<_> = world
            .iter()
            .filter(|o| o.alive && !o.destroyed && o.is_aircraft)
            .map(|o| super::formation::Traffic {
                id: o.id,
                position: o.position,
                velocity: o.velocity,
                planned_velocity: self
                    .actors
                    .iter()
                    .find(|a| a.id() == o.id)
                    .and_then(|a| a.controller().formation_trace())
                    .and_then(|t| t.planned_velocity),
                phase: self
                    .actors
                    .iter()
                    .find(|a| a.id() == o.id)
                    .and_then(|a| a.controller().formation_trace())
                    .map(|t| t.phase),
            })
            .collect();
        for index in 0..self.actors.len() {
            self.step_actor(
                index,
                world,
                &traffic,
                terrain,
                surface,
                now,
                tick,
                &mut output,
            )?;
        }

        // Share only evidence produced this tick, after all controllers have
        // run. Recipients consume it next tick regardless of actor order.
        let fresh_reports: Vec<_> = self
            .actors
            .iter()
            .flat_map(|actor| {
                actor
                    .observed_attacks
                    .iter()
                    .filter(move |attack| {
                        attack.observed_tick == tick && attack.report.defended_id == actor.id()
                    })
                    .map(move |attack| (actor.id(), *attack))
            })
            .collect();
        for (receiver, attack) in fresh_reports {
            self.report_attack_evidence(
                receiver,
                attack.report,
                attack.bearing_world_deg,
                attack.event_id,
            );
        }

        // Neutral AI leaders respond to an actual perceived attack, never to
        // mere contact acquisition. Orders take effect after all decisions.
        let releases: Vec<_> = self
            .actors
            .iter()
            .filter(|leader| {
                leader.alive()
                    && leader.identity.is_leader()
                    && leader.neutral
                    && leader.assignment.stance != engagement::Stance::WeaponsHold
                    && !self.external_leaders.iter().any(|(side, wing, _)| {
                        *side == leader.identity.side && *wing == leader.identity.wing
                    })
                    && leader.observed_attacks.iter().any(|attack| {
                        leader.permits_attack_response(attack)
                            && (leader
                                .assignment
                                .protected_ids
                                .contains(&attack.report.defended_id)
                                || self.actors.iter().any(|member| {
                                    member.id() == attack.report.defended_id
                                        && member.identity.side == leader.identity.side
                                        && member.identity.wing == leader.identity.wing
                                }))
                    })
            })
            .map(AiActor::id)
            .collect();
        for leader in releases {
            let request =
                super::wing::WingRequest::TargetAssignment(super::wing::TargetOrder::FreeSelection);
            self.order(leader, request).expect("live leader")?;
            output.wing.push((leader, request));
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
        terrain: &dyn Fn(f64, f64) -> f64,
        surface: &dyn Fn(f64, f64) -> Surface,
        now: TimeOfDay,
        tick: u64,
        output: &mut MissionOutput,
    ) -> Result<()> {
        // Researched aircraft stand on runways, so their ground is the full
        // surface; legacy aircraft keep the terrain-only height.
        let runway_height = |x, z| surface(x, z).height;
        let ground: &dyn Fn(f64, f64) -> f64 = if self.actors[index].flight.research.is_some() {
            &runway_height
        } else {
            terrain
        };
        let actor_id = self.actors[index].id();
        let mut leader = self.leader_view(index, world);
        let clearance = self.airfield_clearance(index, world);
        if leader.as_ref().is_none_or(|l| !l.recovering) {
            self.actors[index].join_cancelled = false;
        }
        let join = self.join_landing(index, leader.as_ref());
        // Fitted: a wingman does not formate on a human leader parked on the
        // ground; it flies free until the leader is airborne again.
        if leader.is_some_and(|l| l.on_ground) {
            leader = None;
        }

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
        if !actor.dummy {
            if actor.flight.escape.is_some() {
                actor.flight.step_escape(ground);
            } else {
                let mut assessment = crate::ejection::assess(&actor.flight, ground);
                // Opinionated (John, 2026-09-23): taking off or landing, only
                // a catastrophe ejects; any other hazard aborts the landing.
                if let (Some(found), Some(sequence)) = (assessment, actor.airfield.as_mut())
                    && sequence.guards_ejection()
                {
                    let [x, y, z] = actor.flight.position;
                    let below = surface(x, z);
                    let landing = sequence.landing_point();
                    let field = crate::ejection::AirfieldContext {
                        agl_ft: y - below.height,
                        landable_below: below.landable,
                        landing_distance_ft: (landing[0] - x).hypot(landing[2] - z),
                    };
                    if !crate::ejection::catastrophic(&actor.flight, found, field) {
                        // Normal landing geometry (a low, sinking final over
                        // the runway) needs nothing; a final that would touch
                        // down off the runway or gear up goes around; any
                        // hazard on the gates or at marshal aborts too. The
                        // takeoff carries on at full power.
                        let reach = found.impact_seconds.min(10.);
                        let touchdown = surface(
                            x + actor.flight.velocity[0] * reach,
                            z + actor.flight.velocity[2] * reach,
                        );
                        let abort = match sequence.phase() {
                            super::airfield::Phase::Marshal | super::airfield::Phase::Approach => {
                                true
                            }
                            super::airfield::Phase::Final => {
                                let descent = (-actor.flight.velocity[1])
                                    .atan2(actor.flight.velocity[0].hypot(actor.flight.velocity[2]))
                                    .to_degrees();
                                !touchdown.landable
                                    || actor.flight.gear < 0.99
                                    || descent > super::airfield::GO_AROUND_DESCENT_DEG
                            }
                            _ => false,
                        };
                        if abort {
                            sequence.request_go_around();
                        }
                        assessment = None;
                    }
                }
                if actor.escape_monitor.step(assessment).is_some() {
                    actor.flight.eject();
                }
            }
        }

        if !actor.alive() {
            actor.awareness.clear();
            actor.search_target = None;
            actor.controller.set_search_contact(None);
            actor.activity = Activity::Destroyed;
            output.activities.push((actor_id, Activity::Destroyed));
            return Ok(());
        }

        if actor.dummy {
            for i in 0..3 {
                actor.flight.position[i] += actor.flight.velocity[i] * flight::DT;
            }
            actor.flight.ticks += 1;
            actor.pending_threats.clear();
            actor.pending_events.clear();
            actor.activity = Activity::Idle;
            output.activities.push((actor_id, Activity::Idle));
            return Ok(());
        }

        // 1. The actor's own sensors, stepped with the actor as observer.
        let mut targets = actor.observe(tick, world, ground);

        // 2. Own state from the actor's own flight model.
        let own = actor.own_state(ground);
        actor.update_missile_defense(tick, &self.missiles, &own, ground);

        // Takeoff and landing sequences replace combat and formation flying.
        if actor.landing_order.is_none()
            && let Some(order) = join
        {
            actor.landing_order = Some(order);
        }
        if actor.airfield_tick(&clearance, &own, tick, ground, surface, output)? {
            return Ok(());
        }

        // 3. The frame.
        let events = actor.drain_events(tick);
        for target in &mut targets {
            target.wing_attackers =
                assignments.iter().filter(|id| **id == target.id).count() as u32;
            target.terrain_blocked =
                crate::combat::live::terrain_hit(own.position, target.position, &|x, z| {
                    ground(x, z)
                })
                .is_some();
        }
        actor.observed_attacks.retain(|attack| {
            tick.saturating_sub(attack.observed_tick) < 240
                && attack
                    .report
                    .attacker_id
                    .is_none_or(|id| world.iter().any(|o| o.id == id && o.alive && !o.destroyed))
        });
        let emitter_ids: Vec<_> = actor
            .received_emitters
            .iter()
            .map(|emitter| emitter.id)
            .collect();
        let incoming: Vec<_> = actor
            .missile_threats
            .records()
            .filter(|r| !r.stale && r.targeting_receiver)
            .copied()
            .collect();
        for record in incoming {
            let attacker_id =
                if record.source == crate::combat::threats::EvidenceSource::ElectronicSupported {
                    record.radar_bearing_deg.and_then(|bearing| {
                        engagement::identify_supporting_attacker(
                            own.position,
                            own.heading_deg,
                            bearing,
                            &targets,
                            &emitter_ids,
                            identity.side,
                        )
                    })
                } else {
                    None
                };
            actor.remember_attack(ObservedAttack {
                report: engagement::ThreatReport {
                    attacker_id,
                    defended_id: actor_id,
                },
                bearing_world_deg: Some(
                    (own.heading_deg + record.radar_bearing_deg.unwrap_or(record.bearing_deg))
                        .rem_euclid(360.),
                ),
                observed_tick: tick,
                event_id: Some(record.missile_id),
            });
        }
        let reports: Vec<_> = actor
            .observed_attacks
            .iter()
            .filter(|attack| actor.permits_attack_response(attack))
            .map(|a| a.report)
            .collect();
        let protected: Vec<_> = world
            .iter()
            .filter(|object| {
                object.side == identity.side && actor.assignment.protected_ids.contains(&object.id)
            })
            .map(|object| engagement::ProtectedView {
                id: object.id,
                position: object.position,
                velocity: object.velocity,
                alive: object.alive && !object.destroyed,
            })
            .collect();
        let neutral_assignment = engagement::Assignment {
            role: engagement::Role::Disengage,
            stance: if actor.assignment.stance == engagement::Stance::WeaponsHold {
                engagement::Stance::WeaponsHold
            } else {
                engagement::Stance::SelfDefense
            },
            ..engagement::Assignment::default()
        };
        let permission = if actor.neutral {
            &neutral_assignment
        } else {
            &actor.assignment
        };
        let selection = actor.mission_policy.select(
            actor_id,
            identity.side,
            own.position,
            permission,
            &targets,
            &protected,
            &reports,
            actor.controller.target(),
            2,
        );
        actor
            .controller
            .set_mission_hold_fire(actor.assignment.stance == engagement::Stance::WeaponsHold);
        actor.controller.set_mission_target(selection.map(|s| s.id));
        let rejoin = if actor.mission_policy.must_rejoin() {
            protected
                .iter()
                .filter(|p| p.alive)
                .min_by(|a, b| {
                    distance(own.position, a.position)
                        .total_cmp(&distance(own.position, b.position))
                })
                .map(|p| p.position)
        } else if !actor.neutral && actor.assignment.role == engagement::Role::CombatAirPatrol {
            actor
                .assignment
                .patrol
                .filter(|region| !region.contains(own.position))
                .map(|region| region.center_ft)
        } else {
            None
        };
        actor.controller.set_mission_rejoin(rejoin);
        let cue = (rejoin.is_none()
            && selection.is_none()
            && !actor.neutral
            && actor.assignment.stance == engagement::Stance::ProtectAssigned)
            .then(|| {
                actor
                    .observed_attacks
                    .iter()
                    .filter(|attack| {
                        actor
                            .assignment
                            .protected_ids
                            .contains(&attack.report.defended_id)
                            && attack.report.attacker_id.is_none()
                    })
                    .max_by_key(|attack| attack.observed_tick)
                    .and_then(|attack| attack.bearing_world_deg)
            })
            .flatten();
        actor.controller.set_mission_search_bearing(cue);
        actor.update_search_contact(&protected);
        let stations = actor.station_views(&targets, &own);
        let wing = WingView {
            control: self.wing_control,
            formation: self.formation,
            horizontal_spacing_ft: self.horizontal_spacing_ft,
            vertical_spacing_ft: self.vertical_spacing_ft,
            slot: if !actor.neutral
                && actor.identity.is_leader()
                && actor.assignment.role == engagement::Role::Escort
            {
                1 + actor.identity.wing * 3
            } else {
                actor.wing_slot
            },
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
            home_airport: actor.home(),
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
                flight_state: actor
                    .airfield
                    .as_ref()
                    .map(|s| s.phase().flight_state())
                    .unwrap_or(if own.on_ground {
                        FlightState::TakingOff
                    } else {
                        FlightState::Free
                    }),
            };
            actor.controller.step(&frame)?
        };

        // B48 bingo: with a known home runway the aircraft now lands there.
        // Leaders and singletons use the same fitted rule.
        if matches!(
            batch.fuel_state,
            Some(super::route::FuelState::Bingo | super::route::FuelState::Critical)
        ) && actor.landing_order.is_none()
            && let Some(runway) = actor.home_runway
        {
            actor.landing_order = Some(super::airfield::LandingOrder {
                runway,
                reason: super::airfield::LandingReason::Fuel,
            });
        }

        // Choosing a currently visible target is the only way to fill or
        // replace the Novice's one remembered hostile. Losing contact leaves
        // that frozen record intact until its own deadline.
        actor.awareness.select_target(actor.controller.target());
        if actor.controller.target().is_some() {
            actor.search_target = None;
        }
        if let Some(sensors) = &mut actor.sensors {
            if batch.sensor.clear_designation {
                sensors.clear_selection();
            }
            if let Some(target) = batch.sensor.designate {
                sensors.designate(target);
            }
        }
        output
            .wing
            .extend(batch.wing.iter().map(|request| (actor_id, *request)));
        for fallback in &batch.fallbacks {
            output.fallbacks.push((actor_id, *fallback));
        }
        output.launch_calls.extend(
            batch
                .launch_calls
                .iter()
                .map(|launcher| (actor_id, *launcher)),
        );
        if let Some(mut activity) = batch.activity {
            if actor.neutral && matches!(activity, Activity::Idle | Activity::Searching) {
                activity = Activity::Formation;
            }
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
        if let Some(burst) = actor.last_defense.and_then(|d| d.burst) {
            let mut classes = Vec::new();
            for index in 0..burst.chaff.max(burst.flares) {
                if index < burst.chaff {
                    classes.push(SeekerClass::Radar);
                }
                if index < burst.flares {
                    classes.push(SeekerClass::Infrared);
                }
            }
            classes.retain(|class| {
                actor
                    .dispensers
                    .iter()
                    .any(|d| d.class == *class && d.count > 0)
            });
            // Keep one schedule across simultaneous missiles. Each release
            // debits its actual dispenser when due; an empty class never
            // prevents the other class or a defensive maneuver.
            for (index, &class) in classes.iter().enumerate() {
                actor.device_schedule.push((
                    tick + index as u64 * super::QUARTER_SECOND_TICKS,
                    class,
                    1,
                ));
            }
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
        actor.fly(batch.motion.as_ref(), &own, ground, surface)?;
        Ok(())
    }

    /// The leader's pose for a wingman, taken from the leader actor itself.
    fn leader_view(&self, index: usize, world: &[WorldObject]) -> Option<LeaderView> {
        let actor = &self.actors[index];
        if !actor.neutral && actor.assignment.role == engagement::Role::Escort {
            let same_assignment_leader = self.actors.iter().any(|leader| {
                leader.id() != actor.id()
                    && leader.alive()
                    && leader.identity.side == actor.identity.side
                    && leader.identity.wing == actor.identity.wing
                    && leader.identity.is_leader()
                    && leader.assignment.role == engagement::Role::Escort
                    && leader.assignment.protected_ids == actor.assignment.protected_ids
            });
            if actor.identity.is_leader() || !same_assignment_leader {
                return world
                    .iter()
                    .filter(|o| {
                        o.id != actor.id()
                            && o.side == actor.identity.side
                            && o.alive
                            && !o.destroyed
                            && actor.assignment.protected_ids.contains(&o.id)
                    })
                    .min_by_key(|o| o.id)
                    .map(|leader| LeaderView {
                        position: leader.position,
                        heading_deg: leader.heading_deg,
                        velocity: leader.velocity,
                        speed: leader.speed,
                        target: None,
                        recovering: false,
                        on_ground: false,
                    });
            }
        }
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
            // A human leader that touches down after flying is landing.
            return Some(LeaderView {
                position: leader.position,
                velocity: leader.velocity,
                heading_deg: leader.heading_deg,
                speed: leader.speed,
                target: None,
                recovering: leader.on_ground && self.airborne_seen.contains(&leader.id),
                on_ground: leader.on_ground,
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
            recovering: matches!(leader.activity, Activity::ReturningToBase)
                || leader.airfield.as_ref().is_some_and(|s| !s.is_departure()),
            on_ground: false,
        })
    }

    /// Record which aircraft have been airborne, so a human leader's later
    /// touchdown reads as a landing.
    fn track_airborne(&mut self, world: &[WorldObject]) {
        for object in world.iter().filter(|o| o.alive && !o.destroyed) {
            let on_ground = match self.actor(object.id) {
                Some(actor) => actor.flight.research.as_ref().is_some_and(|r| r.on_ground),
                None => object.on_ground,
            };
            if !on_ground && !self.airborne_seen.contains(&object.id) {
                self.airborne_seen.push(object.id);
            }
        }
    }

    /// The retail takeoff and landing gates for one actor (spec in
    /// docs/spec/ai-airfield.md): turn order, runway free, earlier wing
    /// members down, and a parking slot.
    fn airfield_clearance(&self, index: usize, world: &[WorldObject]) -> AirfieldClearance {
        use super::airfield::{PARKING_SLOTS, PLAYER_ROLLING_FPS, SPOT_OCCUPIED_FT};
        let actor = &self.actors[index];
        let (side, wing, member) = (
            actor.identity.side,
            actor.identity.wing,
            actor.identity.member,
        );
        let wingmates = || {
            self.actors.iter().filter(move |a| {
                a.id() != actor.id()
                    && a.alive()
                    && a.identity.side == side
                    && a.identity.wing == wing
            })
        };
        let external_leader = self
            .external_leaders
            .iter()
            .find(|(s, w, _)| *s == side && *w == wing)
            .and_then(|(_, _, id)| {
                world
                    .iter()
                    .find(|o| o.id == *id && o.alive && !o.destroyed)
            });
        // Wing abort (retail 0x4bc2a4): a joining wingman whose leader is
        // neither landing nor on the ground stops landing.
        let leader_landing = if member == 0 {
            true
        } else if let Some(leader) = external_leader {
            leader.on_ground || self.priority_landing.is_some()
        } else {
            wingmates()
                .find(|a| a.identity.is_leader())
                .is_none_or(|leader| {
                    leader.airfield.as_ref().is_some_and(|s| !s.is_departure())
                        || leader.landing_order.is_some()
                        || leader.flight.research.as_ref().is_some_and(|r| r.on_ground)
                })
        };
        let Some(sequence) = actor.airfield.as_ref() else {
            return AirfieldClearance {
                leader_landing,
                ..AirfieldClearance::default()
            };
        };
        let airport = sequence.runway().airport;
        let at_airport = || {
            self.actors
                .iter()
                .filter(move |a| a.id() != actor.id() && a.alive())
                .filter_map(move |a| {
                    a.airfield
                        .as_ref()
                        .filter(|s| s.runway().airport == airport)
                        .map(|s| (a, s))
                })
        };

        // Turn gate: every earlier wing member past its first taxiway leg;
        // a human leader must be airborne.
        let turn = wingmates().all(|a| {
            a.identity.member >= member || a.airfield.as_ref().is_none_or(|s| !s.holds_followers())
        }) && external_leader.is_none_or(|leader| member == 0 || !leader.on_ground);

        // Runway-free gate.
        let spot = sequence.takeoff_spot();
        let near_spot = |position: [f64; 3]| {
            spot.is_some_and(|spot| {
                (position[0] - spot[0]).hypot(position[2] - spot[2]) <= SPOT_OCCUPIED_FT
            })
        };
        let runway = sequence.runway();
        let busy_actor = at_airport().any(|(a, s)| {
            s.blocks_runway()
                || (a.flight.research.as_ref().is_some_and(|r| r.on_ground)
                    && s.phase() != super::airfield::Phase::Parked
                    && near_spot(a.flight.position))
        });
        let busy_human = world.iter().any(|o| {
            o.alive
                && !o.destroyed
                && o.human_controlled
                && self.actor(o.id).is_none()
                && o.on_ground
                && (near_spot(o.position)
                    || (o.velocity[0].hypot(o.velocity[2]) >= PLAYER_ROLLING_FPS
                        && (o.position[0] - runway.center[0])
                            .hypot(o.position[2] - runway.center[2])
                            <= super::airfield::LINEUP_RESET_FT))
        });
        let runway_free = !busy_actor && !busy_human && self.priority_landing != Some(airport);

        // Earlier wing members landing here must be down first.
        let wing_landed = wingmates().all(|a| {
            a.identity.member >= member
                || !a.airfield.as_ref().is_some_and(|s| {
                    !s.is_departure()
                        && s.runway().airport == airport
                        && !a.flight.research.as_ref().is_some_and(|r| r.on_ground)
                })
        });

        // Parking slot: keep one already held, else the lowest free one.
        let free_slot = sequence.slot().or_else(|| {
            (0..PARKING_SLOTS).find(|slot| !at_airport().any(|(_, s)| s.slot() == Some(*slot)))
        });

        AirfieldClearance {
            turn,
            runway_free,
            wing_landed,
            free_slot,
            leader_landing,
        }
    }

    /// B48 join-landing: an AI wingman within 10000 ft of a landing leader
    /// and 40000 ft of its airport lands there too. A human leader's airport
    /// is taken to be the wingman's home runway (fitted).
    fn join_landing(
        &self,
        index: usize,
        leader: Option<&LeaderView>,
    ) -> Option<super::airfield::LandingOrder> {
        let actor = &self.actors[index];
        let leader = leader.filter(|l| l.recovering)?;
        if actor.identity.is_leader()
            || actor.landing_order.is_some()
            || actor.airfield.is_some()
            || actor.bugged_out
            || actor.join_cancelled
        {
            return None;
        }
        let runway = self
            .actors
            .iter()
            .find(|a| {
                a.identity.is_leader()
                    && a.identity.side == actor.identity.side
                    && a.identity.wing == actor.identity.wing
                    && a.alive()
            })
            .and_then(|a| {
                a.airfield
                    .as_ref()
                    .filter(|s| !s.is_departure())
                    .map(|s| *s.runway())
                    .or(a.landing_order.map(|o| o.runway))
                    .or(a.home_runway)
            })
            .or(actor.home_runway)?;
        let position = actor.flight.position;
        let join = super::route::join_leader_landing(&super::route::JoinLandingInputs {
            leader_recovering: true,
            distance_to_leader_ft: distance(position, leader.position),
            distance_to_leader_airport_ft: Some(
                (position[0] - runway.center[0]).hypot(position[2] - runway.center[2]),
            ),
        });
        join.then_some(super::airfield::LandingOrder {
            runway,
            reason: super::airfield::LandingReason::JoinLeader,
        })
    }

    /// Deliver one wing command to one actor (B46).
    pub fn order(
        &mut self,
        actor: u32,
        request: super::wing::WingRequest,
    ) -> Option<Result<super::wing::ReceiverOutcome>> {
        let tick = self.tick;
        let recalling = matches!(
            request,
            super::wing::WingRequest::FormationSelection(_)
                | super::wing::WingRequest::TargetAssignment(super::wing::TargetOrder::HoldFire)
        );
        // A report perceived before recall may still be in next-tick delivery.
        // Remember its identity too, so its subsequent refresh is not a new shot.
        let pending_ids: Vec<_> = self
            .pending_attack_reports
            .iter()
            .filter(|(receiver, _)| recalling && *receiver == actor)
            .filter_map(|(_, attack)| attack.event_id)
            .collect();
        self.actor_mut(actor).map(|a| {
            let outcome = a.order(request, tick);
            if recalling && matches!(outcome, Ok(super::wing::ReceiverOutcome::Applied(_))) {
                a.ignored_attack_ids.extend(pending_ids);
                a.ignored_attack_ids.sort_unstable();
                a.ignored_attack_ids.dedup();
            }
            outcome
        })
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
    /// Collect fresh measurements, then update frozen aircraft memory. Only
    /// the current observation set is allowed into combat target selection.
    fn observe(
        &mut self,
        tick: u64,
        world: &[WorldObject],
        ground: &dyn Fn(f64, f64) -> f64,
    ) -> Vec<TargetView> {
        let live: Vec<u32> = world
            .iter()
            .filter(|o| o.alive && !o.destroyed)
            .map(|o| o.id)
            .collect();
        self.awareness.prune_lifecycle(&live);
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
        let contacts = if let Some(sensors) = self.sensors.as_mut() {
            let observables: Vec<Observable> = world
                .iter()
                .filter(|o| o.id != self.identity.actor.0)
                .filter_map(|o| o.observable.clone())
                .collect();
            let obscured = |from, to| crate::combat::live::terrain_hit(from, to, &ground).is_some();
            let environment = sensors::Environment {
                ground,
                obscured: &obscured,
            };
            sensors.step(&observer, &observables, &environment);
            self.received_emitters = if self.flight.systems.counts[32] <= 1 {
                sensors::passive::emitters(
                    &observer,
                    &observables,
                    sensors.contacts(),
                    &environment,
                )
            } else {
                Vec::new()
            };
            Some(sensors.contacts().to_vec())
        } else {
            self.received_emitters.clear();
            None
        };
        let mut observations = Vec::new();
        // Aircraft on the ground are not air targets.
        for object in world.iter().filter(|o| {
            o.id != self.identity.actor.0
                && o.alive
                && !o.destroyed
                && o.is_aircraft
                && !o.on_ground
        }) {
            if let Some(contacts) = &contacts {
                for contact in contacts
                    .iter()
                    .filter(|c| c.id == object.id && !c.destroyed)
                {
                    observations.push(Observation {
                        target: self.observed_target(object, contact.position),
                        velocity: contact.velocity,
                        source: match contact.channel {
                            sensors::Channel::Radar => ObservationSource::Radar,
                            sensors::Channel::Infrared => ObservationSource::Infrared,
                            sensors::Channel::Visual => ObservationSource::Visual,
                        },
                    });
                }
                // Pilot attention has its own circular, skill-scaled cone.
                // Imported visual equipment remains unchanged for player use.
                // Cloud/night visibility is not supplied by this host yet;
                // the explicit None limit is the fitted clear-air assumption.
                if let Some(observable) = object.observable.as_ref()
                    && !observable.destroyed
                    && observable.airborne
                    && awareness::visual_eligible(
                        self.controller.experience().level,
                        observer.position,
                        self.flight.yaw.to_degrees(),
                        self.flight.pitch.to_degrees(),
                        observable.position,
                        None,
                        crate::combat::live::terrain_hit(
                            observer.position,
                            observable.position,
                            &ground,
                        )
                        .is_none(),
                    )
                {
                    observations.push(Observation {
                        target: self.observed_target(object, observable.position),
                        velocity: observable.velocity,
                        source: ObservationSource::Visual,
                    });
                }
            } else {
                // Explicit sensorless synthetic/replay fixtures supply the
                // whole permitted list. Production actor loading must never
                // select this path as a fallback after a sensor import error.
                observations.push(Observation {
                    target: self.observed_target(object, object.position),
                    velocity: object.velocity,
                    source: ObservationSource::Fixture,
                });
            }
        }
        self.awareness.observe(tick, &observations);
        self.awareness
            .current_observations()
            .map(|snapshot| snapshot.target)
            .collect()
    }

    /// Choose a stable, lost hostile observation for investigation. Current
    /// observations stay in the ordinary selector; a frozen record never does.
    fn update_search_contact(&mut self, protected: &[engagement::ProtectedView]) {
        if self.neutral {
            self.search_target = None;
            self.controller.set_search_contact(None);
            return;
        }
        let lost = |snapshot: &&awareness::Snapshot| {
            snapshot.target.side != self.identity.side
                && self.mission_policy.allows_investigation(
                    &self.assignment,
                    &snapshot.target,
                    protected,
                    &self
                        .observed_attacks
                        .iter()
                        .map(|a| a.report)
                        .collect::<Vec<_>>(),
                    self.id(),
                )
                && !self
                    .awareness
                    .current_observations()
                    .any(|current| current.target.id == snapshot.target.id)
        };
        let preferred = self.controller.target().or(self.search_target);
        let snapshot = preferred
            .and_then(|id| self.awareness.snapshot(id))
            .filter(lost)
            .or_else(|| {
                self.awareness.remembered().filter(lost).min_by(|a, b| {
                    distance(self.flight.position, a.target.position)
                        .total_cmp(&distance(self.flight.position, b.target.position))
                        .then_with(|| a.target.id.cmp(&b.target.id))
                })
            });
        self.search_target = snapshot.map(|s| s.target.id);
        self.controller
            .set_search_contact(snapshot.map(|s| SearchContact {
                id: s.target.id,
                position: s.target.position,
                observed_tick: s.last_observed_tick,
            }));
    }

    fn update_missile_defense(
        &mut self,
        tick: u64,
        missiles: &[MissileSnapshot],
        own: &OwnState,
        ground: &dyn Fn(f64, f64) -> f64,
    ) {
        self.missile_threats.observe(
            tick,
            Receiver {
                id: self.id(),
                position: own.position,
                velocity: self.flight.velocity,
                heading_deg: own.heading_deg,
                pitch_deg: own.body_pitch_deg(),
                skill: self.controller.experience().level,
                rwr_operating: self.flight.systems.counts[32] <= 1,
                visual_operating: true,
                visibility_limit_ft: None,
            },
            missiles,
            |from, to| crate::combat::live::terrain_hit(from, to, &ground).is_none(),
        );
        let speed = own.speed.0.max(125.0);
        let g = own
            .g_limit
            .max(1.0)
            .min(1.0 / own.maximum_bank_deg.to_radians().cos().max(0.01));
        let bank = (1.0 / g).acos().to_degrees();
        let coordinated_rate = (32.174 * (g * g - 1.0).sqrt() / speed).to_degrees();
        let contacts: Vec<_> = self.missile_threats.records().copied().collect();
        self.last_defense = defense::decide(
            tick,
            self.controller.experience().level,
            defense::DefenseOwn {
                position: own.position,
                velocity: self.flight.velocity,
                heading_deg: own.heading_deg,
                flight_path_pitch_deg: own.flight_path_pitch_deg,
                speed_ft_s: own.speed.0,
                bank_deg: own.bank_deg,
                usable_turn_rate_deg_s: coordinated_rate * 0.5,
                usable_pitch_rate_deg_s: (32.174 * (g - 1.0) / speed).to_degrees() * 0.5,
                roll_in_time_s: (bank + own.bank_deg.abs()) / own.roll_limit_deg_per_s.max(1.0)
                    + 0.7,
                dive_speed_safe: own.speed.0 + 32.174 * 20_f64.to_radians().sin() * 5.0
                    <= own.limits.maximum.0,
            },
            &contacts,
            &mut self.defense_state,
            |position| ground(position[0], position[2]),
        );
        if self.last_defense.is_none() {
            self.defense_state.clear_threat();
        }
        self.controller
            .set_missile_defense(self.last_defense.and_then(|d| d.motion).map(|motion| {
                super::controller::MissileDefense {
                    heading_deg: motion.heading_deg,
                    pitch_deg: motion.flight_path_pitch_deg,
                }
            }));
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
        let home = self.home()?;
        let dx = home.x - self.flight.position[0];
        let dz = home.z - self.flight.position[2];
        let distance = (dx * dx + dz * dz).sqrt();
        let cruise = super::route::cruise_speed(&self.speed_limits());
        if cruise.0 <= 0.0 {
            return None;
        }
        Some(distance / cruise.0)
    }

    /// Capture permitted classification/attitude metadata at the time of an
    /// observation. The measured position comes from that observation, not a
    /// later lookup of a remembered object ID in the world.
    fn observed_target(&self, object: &WorldObject, position: [f64; 3]) -> TargetView {
        TargetView {
            id: object.id,
            side: object.side,
            position,
            heading_deg: object.heading_deg,
            pitch_deg: object.pitch_deg,
            speed: object.speed,
            maximum_speed: object.maximum_speed,
            is_aircraft: object.is_aircraft,
            is_fighter: object.is_fighter,
            human_controlled: object.human_controlled,
            valid: object.alive && !object.destroyed,
            type_allowed: true,
            seeker_eligible: self.seeker_eligible(object.is_aircraft),
            wing_attackers: 0,
            terrain_blocked: false,
            sensor_supported: self.sensors.as_ref().is_none_or(|s| s.supports(object.id)),
        }
    }

    /// Whether any carried store's envelope could engage this object (B45).
    fn seeker_eligible(&self, is_aircraft: bool) -> bool {
        let class = if is_aircraft {
            weapon_service::TargetClass::Air
        } else {
            weapon_service::TargetClass::Surface
        };
        self.stations.iter().any(|s| {
            !s.store.inhibited
                && !s.is_empty()
                && weapon_service::store_eligible(s.capability, class)
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
        surface: &dyn Fn(f64, f64) -> Surface,
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
        self.fly_input(input, ground, surface);
        Ok(())
    }

    fn fly_input(
        &mut self,
        input: PilotInput,
        ground: &dyn Fn(f64, f64) -> f64,
        surface: &dyn Fn(f64, f64) -> Surface,
    ) {
        // Airborne actors on the legacy adapter keep the terrain-only surface
        // they have always used; researched actors see runways and wind.
        if self.flight.research.is_some() {
            self.flight.step_surface(&input, surface);
        } else {
            self.flight
                .step_surface(&input, |x, z| Surface::terrain(ground(x, z)));
        }
        self.last_input = input;
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
    pub(super) fn synthetic_profile(id: AircraftId) -> tore_formats::aircraft::Aircraft {
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
            AircraftId::F22 | AircraftId::F22n | AircraftId::Faxx => "F-22",
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

    pub(super) fn setup(
        id: u32,
        side: u32,
        member: u8,
        position: [f64; 3],
        yaw: f64,
    ) -> ActorSetup {
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

    pub(super) fn object(actor: &AiActor, side: u32) -> WorldObject {
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
            on_ground: false,
            observable: None,
        }
    }

    pub(super) fn flat(_x: f64, _z: f64) -> f64 {
        0.0
    }

    pub(super) fn perception_actor(level: Experience) -> AiActor {
        let mut setup = setup(1, 1, 0, [0., 20000., 0.], 0.);
        setup.experience = resolved(level);
        setup.sensors = Some(Sensors::new(sensors::SensorProfiles {
            aircraft: AircraftId::F18,
            radar: None,
            infrared: None,
            visual: None,
            jammer: None,
            signature: sensors::SignatureProfile::default(),
        }));
        AiActor::new(setup).unwrap()
    }

    pub(super) fn visible_object(actor: &AiActor, id: u32, position: [f64; 3]) -> WorldObject {
        let mut target = object(actor, 2);
        target.id = id;
        target.position = position;
        target.observable = Some(Observable {
            id,
            position,
            velocity: target.velocity,
            basis: crate::attitude::Basis::new(0., 0., 0.),
            configuration: sensors::Configuration::default(),
            signature: sensors::SignatureProfile::default(),
            jammer: None,
            jammer_active: false,
            radar_emitting: false,
            airborne: true,
            destroyed: false,
        });
        target
    }

    #[test]
    fn ejection_preempts_weapons_and_keeps_pilot_descent_running_without_ai_control() {
        let mut mission = one_v_one();
        let actor = &mut mission.actors[0];
        let mut source = crate::flight::integration_tests::profile();
        source.fields.get_mut("flags").unwrap().value = "16".into();
        actor.flight = flight::State::new(&source, [0., 10000., 0.]).unwrap();
        actor.flight.crashed = true;
        actor.escape_monitor = crate::ejection::Monitor::seeded(0);
        let id = actor.id();
        let out = run(&mut mission, 120);
        assert!(!out.launches.iter().any(|e| e.actor == id));
        let actor = mission.actor(id).unwrap();
        assert!(actor.flight.escape.is_some());
        let aircraft_position = actor.flight.position;
        let pilot_position = actor.flight.escape.as_ref().unwrap().position;
        run(&mut mission, 120);
        let actor = mission.actor(id).unwrap();
        assert_eq!(
            actor.flight.position, aircraft_position,
            "combat owns the wreck"
        );
        assert_ne!(
            actor.flight.escape.as_ref().unwrap().position,
            pilot_position
        );
        assert!(!actor.alive());
    }

    #[test]
    fn skill_attention_is_independent_of_imported_visual_hardware_and_blocks_terrain() {
        for level in Experience::ALL {
            let mut actor = perception_actor(level);
            let range = awareness::visual_range_feet(level);
            let target = visible_object(&actor, 2, [0., 20000., range]);
            assert_eq!(
                actor.observe(0, std::slice::from_ref(&target), &flat).len(),
                1
            );
            let too_far = visible_object(&actor, 2, [0., 20000., range + 1.]);
            assert!(actor.observe(1, &[too_far], &flat).is_empty());
            let behind = visible_object(&actor, 2, [0., 20000., -1000.]);
            assert!(actor.observe(2, &[behind], &flat).is_empty());
            let ridge = |_: f64, z: f64| {
                if z > range / 4. && z < range * 0.75 {
                    21000.
                } else {
                    0.
                }
            };
            assert!(actor.observe(3, &[target], &ridge).is_empty());
        }
    }

    #[test]
    fn measured_pose_is_frozen_and_lifecycle_removes_hidden_memories() {
        let mut actor = perception_actor(Experience::Ace);
        let mut target = visible_object(&actor, 2, [0., 20000., 5000.]);
        // A sensor snapshot can differ from later host body metadata. The
        // measured position must be the one retained and used for geometry.
        target.position = [90000., 22000., 90000.];
        let views = actor.observe(10, std::slice::from_ref(&target), &flat);
        assert_eq!(views[0].position, [0., 20000., 5000.]);
        let frozen = *actor.awareness.snapshot(2).unwrap();
        target.observable = None;
        target.position = [-90000., 1000., -90000.];
        target.velocity = [100., -100., 500.];
        assert!(
            actor
                .observe(11, std::slice::from_ref(&target), &flat)
                .is_empty()
        );
        assert_eq!(actor.awareness.snapshot(2), Some(&frozen));
        actor.observe(1_000_000, std::slice::from_ref(&target), &flat);
        assert_eq!(actor.awareness.snapshot(2), Some(&frozen));
        target.destroyed = true;
        actor.observe(1_000_001, &[target], &flat);
        assert!(actor.awareness.snapshot(2).is_none());
    }

    #[test]
    fn novice_kill_does_not_recover_a_forgotten_hostile_without_a_new_observation() {
        let mut actor = perception_actor(Experience::Novice);
        let mut first = visible_object(&actor, 2, [0., 20000., 5000.]);
        let mut second = visible_object(&actor, 3, [1000., 20000., 5000.]);
        assert_eq!(
            actor
                .observe(0, &[first.clone(), second.clone()], &flat)
                .len(),
            2
        );
        actor.awareness.select_target(Some(2));
        assert!(actor.awareness.snapshot(3).is_none());
        first.destroyed = true;
        second.observable = None;
        assert!(actor.observe(1, &[first.clone(), second], &flat).is_empty());
        actor.update_search_contact(&[]);
        assert!(actor.awareness.remembered().next().is_none());
        assert_eq!(actor.search_target, None);
        let fresh = visible_object(&actor, 3, [1000., 20000., 5000.]);
        assert_eq!(actor.observe(2, &[first, fresh], &flat).len(), 1);
        actor.awareness.select_target(Some(3));
        assert!(actor.awareness.snapshot(3).is_some());
    }

    #[test]
    fn lost_contact_inside_retention_distance_searches_without_firing_then_reacquires() {
        let mut mission = AiMission::new();
        mission.push(perception_actor(Experience::Novice));
        let target = visible_object(mission.actor(1).unwrap(), 2, [0., 20000., 5000.]);
        mission
            .step(std::slice::from_ref(&target), &flat, TimeOfDay(0))
            .unwrap();
        assert_eq!(mission.actor(1).unwrap().controller.target(), Some(2));
        let snapshot = *mission.actor(1).unwrap().awareness.snapshot(2).unwrap();
        let rounds = mission.actor(1).unwrap().rounds_remaining();
        let mut hidden = target.clone();
        hidden.observable = None;
        hidden.position = [3000., 20000., 4000.];
        for tick in 1..240 {
            let output = mission
                .step(std::slice::from_ref(&hidden), &flat, TimeOfDay(tick))
                .unwrap();
            let actor = mission.actor(1).unwrap();
            assert_eq!(actor.controller.target(), None);
            assert_eq!(actor.activity(), Activity::Searching);
            assert!(output.launches.is_empty());
            assert_eq!(actor.awareness.snapshot(2), Some(&snapshot));
        }
        assert_eq!(mission.actor(1).unwrap().rounds_remaining(), rounds);
        // Place a fresh observation in front of the actor after the search turn.
        let actor = mission.actor(1).unwrap();
        let forward =
            crate::attitude::Basis::new(actor.flight.yaw, actor.flight.pitch, actor.flight.bank)
                .forward;
        let position = std::array::from_fn(|i| actor.flight.position[i] + forward[i] * 5000.);
        let reacquired = visible_object(actor, 2, position);
        mission.step(&[reacquired], &flat, TimeOfDay(240)).unwrap();
        assert_eq!(mission.actor(1).unwrap().controller.target(), Some(2));
        assert_ne!(mission.actor(1).unwrap().activity(), Activity::Searching);
        assert_eq!(
            mission
                .actor(1)
                .unwrap()
                .awareness
                .snapshot(2)
                .unwrap()
                .last_observed_tick,
            240
        );
    }

    #[test]
    fn thirty_sensor_owned_actors_replay_observations_and_search_identically() {
        fn mission() -> AiMission {
            let mut mission = AiMission::new();
            for id in 1..=30 {
                let mut actor = perception_actor(Experience::ALL[(id as usize - 1) % 4]);
                // Each actor owns an independent sensor component and identity.
                let mut setup = setup(
                    id,
                    if id % 2 == 0 { 1 } else { 2 },
                    0,
                    [id as f64 * 250., 20000., id as f64 * 100.],
                    0.,
                );
                setup.experience = actor.controller.experience();
                setup.sensors = actor.sensors.take();
                mission.push(AiActor::new(setup).unwrap());
            }
            mission
        }
        fn advance(mission: &mut AiMission, tick: u64) -> MissionOutput {
            let world: Vec<_> = mission
                .actors()
                .iter()
                .map(|a| {
                    let mut o = visible_object(a, a.id(), a.flight.position);
                    o.side = a.identity.side;
                    o
                })
                .collect();
            mission.step(&world, &flat, TimeOfDay(tick)).unwrap()
        }
        let mut a = mission();
        let mut b = mission();
        for tick in 0..120 {
            assert_eq!(advance(&mut a, tick), advance(&mut b, tick));
            for (a, b) in a.actors().iter().zip(b.actors()) {
                assert_eq!(a.awareness(), b.awareness());
                assert_eq!(a.flight(), b.flight());
                assert_eq!(a.controller(), b.controller());
            }
        }
    }

    pub(super) fn enable_test_radar(actor: &mut AiActor) {
        let volume = sensors::Volume {
            azimuth_rad: 60_f64.to_radians(),
            elevation_rad: 60_f64.to_radians(),
            minimum_ft: 0.,
            maximum_ft: 20. * sensors::FEET_PER_NAUTICAL_MILE,
            minimum_relative_ft: f64::NEG_INFINITY,
            maximum_relative_ft: f64::INFINITY,
        };
        let preset = sensors::Preset::Advanced;
        actor.sensors.as_mut().unwrap().profiles.radar = Some(sensors::RadarProfile {
            record: "TEST.SEE".into(),
            search: volume,
            track: volume,
            look_down: 0.,
            preset,
            notch: preset.notch(),
            resistance: preset.resistance(),
            band: 0,
            source_flags: [0; 2],
            source_doppler: [0; 3],
        });
        actor.sensors.as_mut().unwrap().controls.range_index = 0;
    }

    #[test]
    fn radar_refreshes_memory_beyond_skill_visual_range_and_terrain_masks_both_channels() {
        for level in Experience::ALL {
            let mut actor = perception_actor(level);
            enable_test_radar(&mut actor);
            let target = visible_object(
                &actor,
                2,
                [0., 20000., 6. * sensors::FEET_PER_NAUTICAL_MILE],
            );
            assert_eq!(
                actor.observe(1, std::slice::from_ref(&target), &flat).len(),
                1
            );
            actor.awareness.select_target(Some(2));
            let snapshot = actor.awareness.snapshot(2).unwrap();
            assert_eq!(snapshot.source_ticks.radar, Some(1));
            assert_eq!(snapshot.source_ticks.visual, None);
            actor.observe(2, std::slice::from_ref(&target), &flat);
            assert_eq!(actor.awareness.snapshot(2).unwrap().last_observed_tick, 2);
            let ridge = |_: f64, z: f64| if z > 10000. && z < 20000. { 21000. } else { 0. };
            assert!(actor.observe(3, &[target], &ridge).is_empty());
            assert_eq!(actor.awareness.snapshot(2).unwrap().last_observed_tick, 2);
        }
    }

    #[test]
    fn hold_fire_clears_live_sensor_designation_without_erasing_memory() {
        let mut actor = perception_actor(Experience::Ace);
        enable_test_radar(&mut actor);
        let target = visible_object(&actor, 2, [0., 20000., 5000.]);
        let mut mission = AiMission::new();
        mission.push(actor);
        mission
            .step(std::slice::from_ref(&target), &flat, TimeOfDay(0))
            .unwrap();
        assert_eq!(
            mission.actor(1).unwrap().sensors().unwrap().selected(),
            Some(2)
        );
        mission
            .actor_mut(1)
            .unwrap()
            .order(
                super::super::wing::WingRequest::TargetAssignment(
                    super::super::wing::TargetOrder::HoldFire,
                ),
                1,
            )
            .unwrap();
        let output = mission.step(&[target], &flat, TimeOfDay(1)).unwrap();
        assert!(output.launches.is_empty());
        assert_eq!(
            mission.actor(1).unwrap().sensors().unwrap().selected(),
            None
        );
        assert!(mission.actor(1).unwrap().awareness.snapshot(2).is_some());
    }

    fn incoming_snapshot(
        guidance: crate::combat::missiles::Guidance,
        position: [f64; 3],
    ) -> MissileSnapshot {
        MissileSnapshot {
            id: 99,
            owner: 8,
            position,
            velocity: [0., 0., 2000.],
            guidance,
            target: Some(1),
            radar_active: false,
            radar_acquired: false,
            supported: guidance == crate::combat::missiles::Guidance::Supported,
            supporting_radar_position: Some([0., 20000., -60000.]),
            alive: true,
        }
    }

    #[test]
    fn supported_warning_defends_immediately_and_debits_two_chaff_on_schedule() {
        use crate::combat::missiles::Guidance;
        let mut mission = AiMission::new();
        let mut actor = perception_actor(Experience::Novice);
        actor
            .order(
                super::super::wing::WingRequest::TargetAssignment(
                    super::super::wing::TargetOrder::HoldFire,
                ),
                0,
            )
            .unwrap();
        let initial = actor.dispensers()[1].count;
        mission.push(actor);
        mission.set_missiles(vec![incoming_snapshot(
            Guidance::Supported,
            [0., 20000., -1000.],
        )]);
        let mut releases = Vec::new();
        for tick in 0..90 {
            let output = mission.step(&[], &flat, TimeOfDay(tick)).unwrap();
            assert!(output.launches.is_empty());
            if tick == 0 {
                let actor = mission.actor(1).unwrap();
                assert_eq!(actor.activity(), Activity::Defending);
                assert!(actor.missile_threats().any(|r| r.targeting_receiver));
            }
            for device in output.devices {
                releases.push((tick, device.class, device.released));
            }
        }
        assert_eq!(
            releases,
            [(0, SeekerClass::Radar, 1), (30, SeekerClass::Radar, 1)]
        );
        assert_eq!(mission.actor(1).unwrap().dispensers()[1].count, initial - 2);
    }

    #[test]
    fn silent_and_unseen_missiles_cannot_trigger_ai_defense() {
        use crate::combat::missiles::Guidance;
        for guidance in [Guidance::Active, Guidance::Infrared, Guidance::Emitter] {
            let mut mission = AiMission::new();
            mission.push(perception_actor(Experience::Novice));
            mission.set_missiles(vec![incoming_snapshot(guidance, [0., 20000., -1000.])]);
            for tick in 0..60 {
                let output = mission.step(&[], &flat, TimeOfDay(tick)).unwrap();
                let actor = mission.actor(1).unwrap();
                assert!(actor.defense_decision().is_none(), "{guidance:?}");
                assert!(actor.missile_threats().next().is_none());
                assert!(output.devices.is_empty());
            }
        }
    }

    #[test]
    fn pitbull_acquisition_and_receiver_failure_gate_the_same_ai_service() {
        use crate::combat::missiles::Guidance;
        let mut mission = AiMission::new();
        mission.push(perception_actor(Experience::Novice));
        let mut missile = incoming_snapshot(Guidance::Active, [0., 20000., -1000.]);
        missile.radar_active = true;
        mission.set_missiles(vec![missile]);
        mission.step(&[], &flat, TimeOfDay(0)).unwrap();
        assert!(mission.actor(1).unwrap().defense_decision().is_none());
        missile.radar_acquired = true;
        mission.set_missiles(vec![missile]);
        mission.step(&[], &flat, TimeOfDay(1)).unwrap();
        assert_eq!(mission.actor(1).unwrap().activity(), Activity::Defending);
        let mut failed = AiMission::new();
        let mut actor = perception_actor(Experience::Novice);
        actor.flight_mut().systems.counts[32] = 2;
        failed.push(actor);
        failed.set_missiles(vec![missile]);
        failed.step(&[], &flat, TimeOfDay(0)).unwrap();
        assert!(failed.actor(1).unwrap().defense_decision().is_none());
    }

    #[test]
    fn a_visually_observed_ir_threat_requests_a_mixed_burst_without_hidden_classification() {
        use crate::combat::missiles::Guidance;
        let mut mission = AiMission::new();
        mission.push(perception_actor(Experience::Novice));
        let mut missile = incoming_snapshot(Guidance::Infrared, [0., 20000., 5000.]);
        missile.velocity = [0., 0., -3000.];
        mission.set_missiles(vec![missile]);
        let first = mission.step(&[], &flat, TimeOfDay(0)).unwrap();
        assert!(first.devices.is_empty());
        missile.position[2] -= 25.;
        mission.set_missiles(vec![missile]);
        let output = mission.step(&[], &flat, TimeOfDay(1)).unwrap();
        let actor = mission.actor(1).unwrap();
        assert_eq!(actor.activity(), Activity::Defending);
        assert_eq!(
            actor.defense_decision().unwrap().burst,
            Some(defense::BurstRequest::MIXED)
        );
        assert!(actor.missile_threats().all(|r| r.guidance_class.is_none()));
        assert_eq!(output.devices.len(), 1);
    }

    #[test]
    fn a_distant_novice_maneuvers_without_devices_while_an_ace_preserves_its_flight() {
        use crate::combat::missiles::Guidance;
        for skill in [Experience::Novice, Experience::Ace] {
            let mut mission = AiMission::new();
            let mut actor = perception_actor(skill);
            actor.dispensers.clear();
            mission.push(actor);
            mission.set_missiles(vec![incoming_snapshot(
                Guidance::Supported,
                [0., 20000., -60000.],
            )]);
            let output = mission.step(&[], &flat, TimeOfDay(0)).unwrap();
            let actor = mission.actor(1).unwrap();
            let decision = actor.defense_decision().unwrap();
            assert_eq!(decision.motion.is_some(), skill == Experience::Novice);
            assert!(decision.burst.is_none());
            assert!(output.devices.is_empty());
            assert!(decision.debug.estimated_threat_time_s.unwrap() > 30.);
        }
    }

    #[test]
    fn dummy_holds_400_knots_without_decisions_and_stops_on_death() {
        let mut mission = one_v_one();
        for actor in mission.actors_mut() {
            actor.set_dummy();
        }
        let initial: Vec<_> = mission
            .actors()
            .iter()
            .map(|a| {
                (
                    a.flight.position,
                    a.flight.velocity,
                    a.flight.fuel,
                    a.rounds_remaining(),
                )
            })
            .collect();
        assert_eq!(
            mission
                .actor_mut(1)
                .unwrap()
                .order(
                    super::super::wing::WingRequest::Break {
                        heading_offset_deg: 90,
                        pitch_deg: 0
                    },
                    0
                )
                .unwrap(),
            super::super::wing::ReceiverOutcome::Rejected(super::super::wing::RejectReason::Dummy)
        );
        for tick in 0..1200 {
            mission.actor_mut(1).unwrap().report_hit();
            let world = world_of(&mission);
            let output = mission.step(&world, &|_, _| 0., TimeOfDay(tick)).unwrap();
            assert!(
                output.launches.is_empty() && output.devices.is_empty() && output.wing.is_empty()
            );
        }
        for (actor, (position, velocity, fuel, rounds)) in mission.actors().iter().zip(initial) {
            for i in 0..3 {
                assert!((actor.flight.position[i] - position[i] - velocity[i] * 10.).abs() < 1e-6);
            }
            assert_eq!(actor.flight.velocity, velocity);
            assert_eq!(actor.flight.speed, super::super::launch::DUMMY_SPEED_FPS);
            assert_eq!(actor.flight.fuel, fuel);
            assert_eq!(actor.rounds_remaining(), rounds);
            assert_eq!(actor.controller.target(), None);
            assert_eq!(actor.flight.ticks, 1200);
            assert_eq!(actor.flight.pitch, 0.);
            assert_eq!(actor.flight.bank, 0.);
        }
        let position = mission.actor(1).unwrap().flight.position;
        mission.actor_mut(1).unwrap().set_alive(false);
        mission
            .step(&world_of(&mission), &|_, _| 0., TimeOfDay(1200))
            .unwrap();
        assert_eq!(mission.actor(1).unwrap().flight.position, position);
        assert_eq!(mission.actor(1).unwrap().activity(), Activity::Destroyed);
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
            let mut maximum_altitude_error = 0.0_f64;
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
                if tick > 3600 && turn == 0. {
                    maximum_altitude_error = maximum_altitude_error.max(
                        (mission.actor(1).unwrap().flight().position[1] - leader.position[1]).abs(),
                    );
                }
                if tick > 7200 {
                    assert!(
                        last_error < 150.,
                        "turn {turn}, start {initial_offset}, tick {tick}: slot error {last_error}"
                    );
                }
            }
            assert!(last_error < 150.);
            if turn == 0. {
                eprintln!(
                    "straight formation offset {initial_offset}: maximum altitude error {maximum_altitude_error:.2} ft"
                );
                assert!(maximum_altitude_error < 10.);
            }
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
                start.flight.speed = 800.;
                start.flight.velocity = [0., 0., 800.];
                m.push(AiActor::new(start).unwrap());
            }
            if reverse {
                m.actors.reverse();
            }
            m
        }
        for initial_heading in [0., 180.] {
            let mut a = scene(false);
            let mut b = scene(true);
            let mut leader = object(a.actor(1).unwrap(), 1);
            leader.id = 0;
            leader.position = [0., 20000., 0.];
            leader.heading_deg = initial_heading;
            leader.velocity = super::super::formation::velocity(initial_heading, 0., 800.);
            for tick in 0..1800 {
                for mission in [&mut a, &mut b] {
                    if initial_heading == 0. && matches!(tick, 300 | 900) {
                        mission
                            .order_wing(
                                Side(1),
                                0,
                                None,
                                super::super::wing::WingRequest::FormationSelection(
                                    if tick == 300 {
                                        Formation::LineAbreast
                                    } else {
                                        Formation::LineAstern
                                    },
                                ),
                            )
                            .unwrap();
                    }
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
                leader.position[2] += leader.velocity[2] / 120.;
            }
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
            let mut phases = [[false; 7]; 4];
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
        let targets: Vec<_> = world
            .iter()
            .filter(|o| o.id != actor.id())
            .map(|o| actor.observed_target(o, o.position))
            .collect();
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
        let views = [actor.observed_target(&target, target.position)];
        assert_eq!(
            actor.station_views(&views, &own)[0].employment_fit,
            Some(0.0)
        );
        target.position = [2000.0, 20000.0, 1000.0];
        let views = [actor.observed_target(&target, target.position)];
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
                    actor
                        .fly(Some(&intent), &own, &flat, &|x, z| {
                            Surface::terrain(flat(x, z))
                        })
                        .unwrap();
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
        let targets = [actor.observed_target(&target, target.position)];
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
    #[test]
    fn routine_formation_changes_use_local_physical_paths() {
        use super::super::{
            formation::Phase,
            wing::{WingRequest, formation_slot_point},
        };
        let mut cases = Vec::new();
        for from in Formation::ALL {
            for to in Formation::ALL {
                if from != to {
                    cases.push((from, to, 512, 512, 0, 0.0_f64));
                }
            }
        }
        cases.extend([
            (Formation::Echelon, Formation::Echelon, 512, 2048, 0, 0.),
            (Formation::Echelon, Formation::Echelon, 2048, 512, 0, 0.),
            (
                Formation::Echelon,
                Formation::LineAbreast,
                512,
                512,
                512,
                0.,
            ),
            (
                Formation::LineAbreast,
                Formation::Echelon,
                512,
                512,
                -512,
                0.75,
            ),
            (
                Formation::LineAbreast,
                Formation::Echelon,
                512,
                512,
                -512,
                1.5,
            ),
        ]);
        for (from, to, old_spacing, new_spacing, stacking, turn) in cases {
            let mut mission = AiMission::new();
            mission.set_spacing(old_spacing, 0);
            mission.set_formation(from);
            mission.set_external_leader(Side(1), 0, 0);
            for member in 1..=4 {
                let offset = formation_slot_point(from, member, old_spacing, 0).unwrap();
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
            let mut phases = [Phase::Close; 4];
            let mut entered = [false; 4];
            for tick in 0..36000 {
                if tick == 1200 {
                    mission
                        .order_wing(Side(1), 0, None, WingRequest::FormationSelection(to))
                        .unwrap();
                    mission.set_spacing(new_spacing, stacking);
                }
                if turn > 0. {
                    if tick == 3600 {
                        mission
                            .order_wing(
                                Side(1),
                                0,
                                None,
                                WingRequest::FormationSelection(Formation::LineAstern),
                            )
                            .unwrap();
                        mission.set_spacing(512, 0);
                    }
                    let heading = (turn * (tick as f64 / 120. - 10.).clamp(0., 60.)).to_radians();
                    leader.heading_deg = heading.to_degrees();
                    leader.velocity = [800. * heading.sin(), 0., 800. * heading.cos()];
                }
                let mut world = world_of(&mission);
                world.push(leader.clone());
                let before: Vec<_> = mission
                    .actors()
                    .iter()
                    .map(|a| a.flight().clone())
                    .collect();
                mission.step(&world, &flat, TimeOfDay(0)).unwrap();
                for (position, velocity) in leader.position.iter_mut().zip(leader.velocity) {
                    *position += velocity / 120.;
                }
                let mut achieved = world_of(&mission);
                achieved.push(leader.clone());
                for (i, actor) in mission.actors().iter().enumerate() {
                    let mut replay = before[i].clone();
                    replay.step_surface(actor.last_input(), |_, _| Surface::terrain(0.));
                    assert_eq!(replay, *actor.flight());
                    let trace = actor.controller().formation_trace().unwrap();
                    entered[i] |= trace.phase == Phase::Reposition;
                    if trace.phase != phases[i] {
                        eprintln!(
                            "{from:?}->{to:?} {old_spacing}->{new_spacing} V{stacking} turn={turn} t={:.1} actor={} phase={:?} error={:.0}",
                            tick as f64 / 120.,
                            actor.id(),
                            trace.phase,
                            trace.slot_distance_ft
                        );
                        phases[i] = trace.phase;
                    }
                    for other in achieved.iter().filter(|o| o.id != actor.id()) {
                        minimum = minimum.min(distance(actor.flight().position, other.position));
                    }
                    assert!(
                        turn > 0.75 || !matches!(trace.phase, Phase::Breakout | Phase::Intercept),
                        "routine change entered recovery: {trace:?}"
                    );
                }
            }
            eprintln!(
                "{from:?}->{to:?} {old_spacing}->{new_spacing} V{stacking} turn={turn}: minimum={minimum:.1}"
            );
            assert!(minimum > 250.);
            for (i, actor) in mission.actors().iter().enumerate() {
                let t = actor.controller().formation_trace().unwrap();
                assert!(entered[i]);
                assert_eq!(t.phase, Phase::Close, "unfinished: {t:?}");
                assert!(t.slot_distance_ft < 100.);
            }
        }
    }
}

#[cfg(test)]
#[path = "engagement_integration_tests.rs"]
mod engagement_integration_tests;

#[cfg(test)]
#[path = "airfield_integration_tests.rs"]
mod airfield_integration_tests;
