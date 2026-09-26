//! Write-only "why" records for the AI, kept for the replay debug panels,
//! recordings and mission logs. This is an opinionated addition requested by
//! John on 2026-09-26; the record layout is an agent decision.
//!
//! Three kinds of record live here:
//!
//! - [`ControllerTrace`]: what one [`Controller`](super::controller::Controller)
//!   considered on its latest advancing tick, from the events it ranked to
//!   the steering point it chose.
//! - [`ActorTrace`]: what the mission runtime decided around that controller
//!   for one [`AiActor`](super::mission::AiActor): ejection, the airfield
//!   gates, the mission target ranking, and the controls it produced.
//! - [`JournalEntry`]: the messages AI aircraft exchanged (attack reports,
//!   wing orders, escort priorities and missile warnings), each with its
//!   sender, recipients, content, tick and every recipient's outcome. The
//!   mission keeps them until the host drains them once per tick.
//!
//! The fourth record, the log of random draws, lives on
//! [`DecisionRandom`](super::DecisionRandom) itself.
//!
//! Rules that keep the records neutral:
//!
//! - No decision reads them. They hold copies of values the decision code
//!   computed anyway, or pure recomputations made only to explain.
//! - Recording never calls `Controller::formation_point` (it draws), nor
//!   `Controller::terrain_floor` or the controller's rate query (they count
//!   fitted fallbacks).
//! - The formation trace and the activity are decision inputs; recording
//!   only ever copies them.
//! - Records never take part in equality: two controllers with the same
//!   decision state compare equal whatever their traces say.
//! - A controller trace starts afresh on each advancing tick and is left as
//!   it was by a repeated tick. An actor trace starts afresh on every
//!   mission step, which always advances.
//!
//! The golden fingerprints in `golden_tests` prove the simulation behaves
//! exactly as it did before these records existed.

use std::collections::VecDeque;

use super::airfield::{LandingOrder, Phase as AirfieldPhase, Situation, Step};
use super::controller::{
    Completion, EmploymentCheck, MotionIntent, SearchContact, StationVerdict, StationView,
    TargetView, ThreatReport,
};
use super::engagement::{Explanation, Role, Selection, Stance};
use super::fitted::Fallback;
use super::geometry::TargetGeometry;
use super::mission::{AirfieldClearance, ObservedAttack};
use super::motion::MotionRequest;
use super::pursuit::PursuitOffset;
use super::route::FuelState;
use super::steering_adapter::AdapterOutput;
use super::tactics::{
    BehaviorChoice, LastDitchCandidate, MissileLaunch, MissileResponse, PursuitCondition,
    PursuitOffsets, Quadrant, QuadrantThresholds, TacticalSituation,
};
use super::threat::{AttackState, ScriptReason, ScriptStart, WarningDelay, WarningOutcome};
use super::weapon_service::{self, ServiceInputs, ServiceOutcome, StationId};
use super::wing::{ReceiverOutcome, WingRequest};

/// A write-only record kept beside decision state. It never takes part in
/// equality, so holders with the same decision state compare equal whatever
/// their records say.
#[derive(Clone, Debug, Default)]
pub(crate) struct Record<T>(pub(crate) T);

impl<T> PartialEq for Record<T> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// Controller trace

/// What one controller considered on its latest advancing tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ControllerTrace {
    /// The advancing tick this record describes; `None` before the first.
    pub tick: Option<u64>,
    pub path: StepPath,
    /// Events, launch warnings and the script reason (B47).
    pub events: EventTrace,
    /// Fuel judged this tick (B48). `None` when the decision stopped earlier.
    pub fuel: Option<FuelTrace>,
    /// An ordered approach was dropped because its target was no longer seen.
    pub ordered_approach_lost: bool,
    /// Target choice (B41 or the mission's choice).
    pub target: TargetTrace,
    /// Geometry against the chosen target (B01 to B03).
    pub geometry: Option<TargetGeometry>,
    /// Store choice, lock and the weapon service (B42, B45).
    pub weapons: Option<WeaponTrace>,
    /// Which motion branch produced this tick's maneuver.
    pub motion: MotionTrace,
    /// The tactical choice, when a new maneuver was chosen this tick.
    pub maneuver: Option<ManeuverTrace>,
    /// How the latest motion request became concrete numbers.
    pub resolve: Option<ResolveTrace>,
    /// Pursuit target and steering offset in force at the end of the tick.
    pub pursuit: Option<(u32, PursuitOffset)>,
}

impl ControllerTrace {
    pub(crate) fn begin(tick: u64) -> Self {
        Self {
            tick: Some(tick),
            ..Self::default()
        }
    }
}

/// Which way through the controller's step the tick went.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StepPath {
    /// No advancing tick has run yet.
    #[default]
    NotRun,
    /// The full decision ran.
    Decided,
    /// The aircraft was already destroyed; nothing was decided.
    Destroyed,
}

/// Events delivered this tick and the script reason they produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventTrace {
    /// The aircraft was hit this tick.
    pub hit: bool,
    /// Targets that left the world or became unavailable and were the
    /// current target, so the target was dropped.
    pub lost_targets: Vec<u32>,
    /// Every launch warning handled this tick, in order.
    pub warnings: Vec<WarningTrace>,
    /// The highest-ranked reason among this tick's events.
    pub highest: Option<ScriptReason>,
    /// The reason the running script was started with, before this tick.
    pub previous: Option<ScriptReason>,
    /// Whether the new reason restarted the script or let it resume.
    pub start: Option<ScriptStart>,
}

/// One launch warning as the controller handled it (B47).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WarningTrace {
    pub report: ThreatReport,
    /// Delivered from this aircraft's own queue because its delay ran out.
    pub from_queue: bool,
    /// The state the delay rule used: ordinary flight, or attacking with or
    /// without the launcher as its target.
    pub attack_state: AttackState,
    /// Quarter seconds between launch and warning.
    pub delay: WarningDelay,
    /// The tick the warning takes effect.
    pub due_tick: Option<u64>,
    pub step: WarningStep,
}

/// What happened to one launch warning.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WarningStep {
    /// The delay rule does not warn this kind of target.
    NotWarned,
    /// Held until its delay runs out. `already_queued` repeats a report the
    /// queue already held.
    Queued { already_queued: bool },
    /// The warning arrived: the gates, the countermeasure roll and the
    /// reaction (B47).
    Received {
        outcome: WarningOutcome,
        /// It produced a "SAM launch" or "AAM launch" radio call.
        launch_call: bool,
        /// Devices were released, so the weapon service's pending deadline
        /// moved to this quarter-second count (`None`: nothing pending).
        weapons_postponed_to: Option<u64>,
    },
}

/// Fuel on this tick (B48).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FuelTrace {
    pub internal_fuel_lbs: f64,
    /// Endurance at the current fuel flow, seconds.
    pub endurance_s: f64,
    /// Time to reach home at cruise, seconds; `None` without a home.
    pub time_home_s: Option<f64>,
    /// `None` when the fuel inputs could not be judged.
    pub state: Option<FuelState>,
    /// Bingo or critical: the aircraft is heading home.
    pub recovering: bool,
}

/// How the target was chosen.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TargetTrace {
    /// The target held before this tick.
    pub previous: Option<u32>,
    pub path: TargetPath,
    pub chosen: Option<u32>,
    /// A visible target ended an investigation of an old contact.
    pub search_ended: bool,
}

/// Which target-choice path ran.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum TargetPath {
    #[default]
    NotRun,
    /// Weapons are held: no target.
    HoldFire,
    /// The mission's engagement policy chose (see the actor trace for its
    /// ranking); `refused` says why the controller could not use it.
    Mission {
        requested: Option<u32>,
        refused: Option<MissionTargetRefusal>,
    },
    /// The current target is kept: valid and inside 20,000 ft (B41).
    Retained { distance_ft: f64 },
    /// B41 ranking over the permitted targets; the winner's score, feet.
    Ranked {
        candidates: usize,
        score_ft: Option<f64>,
    },
}

/// Why the controller could not use the mission's target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MissionTargetRefusal {
    NotObserved,
    Own,
    SameSide,
    Invalid,
    TypeNotAllowed,
    NoUsableWeapon,
}

/// Store choice, lock and the weapon service on this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct WeaponTrace {
    /// Air or surface, from the target.
    pub class: weapon_service::TargetClass,
    /// Every carried store and why it can or cannot be used. Empty when there
    /// is no target.
    pub stations: Vec<StationTrace>,
    pub chosen: Option<StationId>,
    pub lock: LockTrace,
    /// Exactly what the weapon service was given.
    pub inputs: ServiceInputs,
    pub phase_before: weapon_service::Phase,
    /// `None` when the service stopped on an unresolved rule.
    pub outcome: Option<ServiceOutcome>,
    /// The fitted burst-pacing rule restarted the service from search.
    pub burst_pacing_restart: bool,
    pub phase_after: weapon_service::Phase,
    /// The service's pending deadline after this tick, quarter-second counts.
    pub deadline: Option<u64>,
}

/// One carried store against the current target.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StationTrace {
    /// Position in the actor's station list.
    pub index: usize,
    pub station: StationId,
    /// `None` when the store was not examined because ten usable stores came
    /// first (the B45 candidate limit).
    pub verdict: Option<StationVerdict>,
    /// Every envelope reason, for a store outside its employment envelope.
    pub employment: Option<EmploymentCheck>,
    /// The fitted hit-chance term, for a usable store.
    pub hit_chance: Option<f64>,
    /// The B45 store score, for a usable store; the highest wins.
    pub score: Option<f64>,
}

/// Whether the weapon lock holds, and each condition behind it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LockTrace {
    pub locked: bool,
    pub has_target: bool,
    pub has_station: bool,
    /// The target is ahead (off-beam under 90 degrees); `None` when its
    /// bearing is undefined.
    pub target_ahead: Option<bool>,
    pub terrain_blocked: bool,
}

/// Which motion branch won this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MotionTrace {
    pub branch: MotionBranch,
    /// The quarter-second clock when the ordinary motion choice ran.
    pub quarter: Option<u64>,
    /// When a new tactical choice was due, as the choice began.
    pub choice_due_at: Option<u64>,
    /// When the next tactical choice is due, after this tick.
    pub next_choice_at: Option<u64>,
    /// The running maneuver's completion rule fired this tick.
    pub active_finished: Option<bool>,
    /// A launch reason, recovery or hold fire cancelled a search.
    pub search_cancelled: bool,
    /// An ordered approach ended (arrived, lost its target or was overruled).
    pub ordered_approach_ended: bool,
    /// The maneuver finished with no new reason, so the script reason cleared.
    pub reason_cleared: bool,
}

/// The branch that produced this tick's maneuver, in the order the
/// controller tries them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum MotionBranch {
    #[default]
    None,
    /// The shared missile-defense policy is flying the aircraft.
    MissileDefense { heading_deg: f64, pitch_deg: f64 },
    /// Flying back toward an escorted aircraft or a patrol region.
    MissionRejoin { point: [f64; 3] },
    /// Searching along a bearing an escort was given.
    SearchBearing { bearing_deg: f64 },
    /// An ordered approach is still closing on its target.
    OrderedApproach {
        target: u32,
        distance_ft: f64,
        /// How much of the ordered offset still applies (1 far, 0 at 2000 ft).
        offset_scale: f64,
    },
    /// An ordered maneuver is still running.
    OrderedMotion,
    /// Investigating an old contact: flying to it, or orbiting over it.
    Search {
        contact: SearchContact,
        distance_ft: f64,
        orbiting: bool,
    },
    /// An Ace gave up a search after two minutes.
    SearchAbandoned { contact: SearchContact },
    /// Flying the formation slot.
    Formation(FormationTrace),
    /// The running maneuver continues until its completion rule fires.
    ActiveContinues,
    /// A new tactical choice is not due yet; the running maneuver continues.
    NotYetDue,
    /// A new tactical choice was made (see the maneuver trace).
    NewManeuver,
}

/// The formation branch's numbers (B43 slot and the fitted guidance).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FormationTrace {
    /// The slot point in world feet, variation included.
    pub slot_point: [f64; 3],
    /// Where the guidance steers.
    pub aim: [f64; 3],
    pub speed_fps: f64,
    pub bank_deg: Option<f64>,
    /// Close formation flight: the tighter bank limit applies.
    pub close: bool,
    pub burner: bool,
    /// The formation, spacing or stacking changed, so the slot moved.
    pub slot_changed: bool,
    /// The previous formation request is still running.
    pub continuing: bool,
}

/// The tactical choice made this tick (B10 to B14, B48).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ManeuverTrace {
    pub path: ManeuverPath,
    pub situation: Option<TacticalSituation>,
    /// Ahead or behind, facing or facing away.
    pub quadrant: Option<Quadrant>,
    /// The experience table row for that quadrant.
    pub thresholds: Option<QuadrantThresholds>,
    pub choice: Option<BehaviorChoice>,
    /// A fitted rule that stood in for an unresolved branch.
    pub fallback: Option<Fallback>,
    pub last_ditch: Option<LastDitchCandidate>,
    /// The bounded motion request the choice produced.
    pub request: Option<MotionRequest>,
}

impl ManeuverTrace {
    pub(crate) fn new(path: ManeuverPath, request: Option<MotionRequest>) -> Self {
        Self {
            path,
            situation: None,
            quadrant: None,
            thresholds: None,
            choice: None,
            fallback: None,
            last_ditch: None,
            request,
        }
    }
}

/// Which part of the tactical choice ran.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ManeuverPath {
    /// Bingo or critical fuel: heading home (B48).
    ReturnToBase { home_heading_deg: i32 },
    /// A missile launch reason (B10).
    MissileReaction {
        launch: MissileLaunch,
        response: MissileResponse,
    },
    /// No target: straight flight.
    NoTarget,
    /// The target is directly above or below, so its bearing is undefined.
    NoBearing,
    /// Evade or hit reason (B11).
    Evasion,
    /// The ordinary approach choice (B12).
    Approach,
}

/// How a motion request became concrete numbers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolveTrace {
    pub request: MotionRequest,
    pub pitch: PitchSource,
    /// The speed the request resolved to before any pursuit regulation.
    pub speed_fps: f64,
    pub pursuit: Option<PursuitTrace>,
    pub completion: Completion,
    /// The fitted completion-axis rule chose the axis.
    pub completion_axis_fitted: bool,
}

/// Where the requested flight-path pitch came from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PitchSource {
    Explicit {
        pitch_deg: f64,
    },
    /// The fitted engagement pitch (B10/B13).
    Engagement {
        relative_altitude_ft: f64,
        horizontal_distance_ft: f64,
        can_climb: bool,
        pitch_deg: f64,
    },
}

/// The pursuit steering point (B15) chosen with a new maneuver.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PursuitTrace {
    pub condition: PursuitCondition,
    /// The drawn offsets from the target, feet.
    pub offsets: PursuitOffsets,
    pub steering_point: [f64; 3],
    /// The regulated speed, when the request asked for corner speed.
    pub regulated_speed_fps: Option<f64>,
}

/// The maneuver a controller is flying, for display.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveManeuverView {
    pub intent: MotionIntent,
    /// The tick the maneuver was submitted.
    pub submitted_tick: u64,
    pub formation: bool,
    pub search: bool,
    pub ordered: bool,
}

// ---------------------------------------------------------------------------
// Actor trace

/// What the mission runtime decided for one actor on its latest step.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ActorTrace {
    /// The mission tick this record describes; `None` before the first step.
    pub tick: Option<u64>,
    pub path: ActorPath,
    /// The airfield gates the mission computed for this actor.
    pub clearance: AirfieldClearance,
    pub ejection: Option<EjectionTrace>,
    /// A join-the-leader landing started this tick (B48).
    pub join_landing: Option<LandingOrder>,
    pub airfield: Option<AirfieldTrace>,
    /// Launch warnings dropped before the controller saw them.
    pub dropped_warnings: Vec<(ThreatReport, DropReason)>,
    /// Remembered attacks forgotten this tick.
    pub expired_attacks: Vec<(ObservedAttack, ExpiryReason)>,
    /// What the actor's own sensors permitted as targets this tick.
    pub targets: Vec<TargetView>,
    pub engagement: Option<EngagementTrace>,
    /// Why the mission sent the controller back toward a point.
    pub rejoin: Option<RejoinTrace>,
    /// Bearing given to search along, from an attack on an escorted aircraft.
    pub search_cue_deg: Option<f64>,
    /// The old contact the controller may investigate.
    pub search_contact: Option<SearchContact>,
    /// The target the stores were aimed at when their envelopes were judged.
    pub station_aim: Option<u32>,
    pub stations: Vec<StationView>,
    /// A bingo landing order was issued this tick (B48).
    pub bingo_landing: Option<LandingOrder>,
    pub fly: Option<FlyTrace>,
}

impl ActorTrace {
    pub(crate) fn begin(tick: u64) -> Self {
        Self {
            tick: Some(tick),
            ..Self::default()
        }
    }
}

/// Which way through the mission step the actor went.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ActorPath {
    #[default]
    NotRun,
    /// Destroyed, or the pilot has ejected: nothing flies.
    Destroyed,
    /// A training target flying straight and level.
    Dummy,
    /// A takeoff or landing sequence flew the aircraft.
    Airfield,
    /// The ordinary controller decided.
    Controller,
}

/// The automatic ejection check this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EjectionTrace {
    /// The pilot has already ejected; only the escape runs.
    pub escape_running: bool,
    /// The hazard found, with seconds to impact.
    pub assessment: Option<crate::ejection::Assessment>,
    /// The airfield phase whose abort rule judged the hazard.
    pub guarded_phase: Option<AirfieldPhase>,
    /// The hazard was a catastrophe, so the pilot may still eject.
    pub catastrophic: Option<bool>,
    /// A go-around or abort was requested instead of an ejection.
    pub go_around: Option<bool>,
    /// The pilot ejected this tick.
    pub ejected: bool,
}

/// One tick of a takeoff or landing sequence.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirfieldTrace {
    /// The stored landing order started its sequence this tick.
    pub began_landing: bool,
    /// Why the aircraft left the sequence for free flight, if it did.
    pub left: Option<AirfieldExit>,
    /// What the sequence knew, when it ran.
    pub situation: Option<Situation>,
    /// What it commanded. The controls it produced are in the actor
    /// trace's `fly` record.
    pub step: Option<Step>,
}

/// Why an aircraft left a landing sequence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AirfieldExit {
    /// Its leader is neither landing nor on the ground (wing abort).
    WingAbort,
    /// A missile threat interrupted the early approach (B47).
    MissileThreat { defending: bool, warned: bool },
}

/// The mission's engagement policy for this actor this tick.
#[derive(Clone, Debug, PartialEq)]
pub struct EngagementTrace {
    /// Holding formation until released; only self-defense applies.
    pub neutral: bool,
    /// The role and stance actually used (the neutral gate when neutral).
    pub role: Role,
    pub stance: Stance,
    /// Every remembered attack and whether it is acted on.
    pub reports: Vec<ReportTrace>,
    pub selection: Option<Selection>,
    /// An escort beyond its leash is flying back to its charge.
    pub must_rejoin: bool,
    /// The ranking behind the selection.
    pub explanation: Explanation,
}

/// One remembered attack and whether the actor acts on it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ReportTrace {
    pub attack: ObservedAttack,
    /// `None` when the actor acts on it.
    pub ignored: Option<IgnoreReason>,
}

/// Why the mission sent the controller toward a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RejoinTrace {
    pub point: [f64; 3],
    pub reason: RejoinReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RejoinReason {
    /// An escort went beyond its 10 nm leash.
    EscortLeash,
    /// A patrol left its region.
    OutsidePatrol,
}

/// Maneuver to controls: what was asked, what the adapter asked of the
/// flight model, and what the flight model delivered.
#[derive(Clone, Debug, PartialEq)]
pub struct FlyTrace {
    /// The maneuver flown; `None` leaves the controls neutral.
    pub intent: Option<MotionIntent>,
    /// The B44 terrain floor, degrees.
    pub terrain_floor_deg: Option<f64>,
    /// The adapter's whole output: the controls, its fallbacks and the
    /// rate-limited attitude it requested.
    pub adapter: Option<AdapterOutput>,
    /// The attitude and speed after the flight model stepped.
    pub achieved: Achieved,
}

/// The aircraft's attitude and speed after its flight step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Achieved {
    pub heading_deg: f64,
    pub flight_path_pitch_deg: f64,
    pub bank_deg: f64,
    pub speed_fps: f64,
}

// ---------------------------------------------------------------------------
// Journal

/// The most entries the mission journal holds between drains. Older entries
/// are dropped first and counted.
pub const JOURNAL_LIMIT: usize = 4096;

/// One message between aircraft, with every recipient's outcome.
#[derive(Clone, Debug, PartialEq)]
pub struct JournalEntry {
    /// The mission tick it happened on.
    pub tick: u64,
    /// The aircraft that sent it (the player's id for the player); `None`
    /// for the host.
    pub sender: Option<u32>,
    pub message: Message,
    pub receipts: Vec<Receipt>,
}

/// One recipient's outcome.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Receipt {
    pub actor: u32,
    pub outcome: Outcome,
}

/// What was sent.
#[derive(Clone, Debug, PartialEq)]
pub enum Message {
    /// An attack perceived by the defended aircraft (the sender), shared
    /// with its wing leader and escorts. The attacker is known only when it
    /// was independently identified; the bearing is world-relative.
    AttackEvidence(ObservedAttack),
    /// A neutral AI leader released itself to free target selection after a
    /// perceived attack on its wing or a charge.
    FreeSelection { trigger: ObservedAttack },
    /// A wing command over the wing channel (B43, B46). Boxed: a landing
    /// order carries the airport's anchor points.
    WingRequest(Box<WingRequest>),
    /// An escort's mission target or its priority changed.
    EscortPriority { selection: Option<Selection> },
    /// A launch warning addressed to one aircraft (B47). The sender is the
    /// launcher.
    MissileWarning(ThreatReport),
}

/// What happened at one recipient.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Outcome {
    /// Held for delivery on the next tick.
    Queued,
    /// Delivered, and the recipient acts on it.
    Delivered,
    /// Delivered or offered, but not acted on.
    Ignored(IgnoreReason),
    /// Forgotten.
    Expired(ExpiryReason),
    /// A wing command's outcome: applied, rejected with its reason, applied
    /// without motion, or motion installed.
    Order(ReceiverOutcome),
    /// A launch warning held until its delay runs out.
    WarningDue { due_tick: u64, already_queued: bool },
    /// A launch warning arrived.
    WarningReceived {
        outcome: WarningOutcome,
        launch_call: bool,
    },
    /// A launch warning dropped before the controller saw it.
    WarningDropped(DropReason),
}

/// Why an attack report was not acted on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IgnoreReason {
    /// The report names a different defended aircraft than its reporter.
    NotTheDefendedAircraft,
    /// The reporter is neither an AI aircraft nor a registered human leader.
    UnknownReporter,
    /// The recipient left the mission before delivery.
    RecipientGone,
    /// Holding formation since a recall at this tick; the attack was seen
    /// before it.
    SeenBeforeRecall { recalled_at: u64 },
    /// The projectile was already known when the recall came.
    KnownAtRecall { event_id: u32 },
}

/// Why a remembered attack was forgotten.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpiryReason {
    /// Not refreshed for this many ticks (the limit is 240).
    Age { ticks: u64 },
    /// The identified attacker is no longer alive.
    AttackerGone,
}

/// Why a launch warning was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropReason {
    /// Warnings are ignored while taking off and landing (B47).
    Airfield { phase: AirfieldPhase },
    /// Training targets ignore warnings.
    TrainingTarget,
    /// The delay rule does not warn this kind of target.
    NotWarned,
}

/// The mission's bounded message journal.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Journal {
    entries: VecDeque<JournalEntry>,
    dropped: u64,
}

impl Journal {
    pub(crate) fn push(&mut self, entry: JournalEntry) {
        if self.entries.len() == JOURNAL_LIMIT {
            self.entries.pop_front();
            self.dropped = self.dropped.saturating_add(1);
        }
        self.entries.push_back(entry);
    }

    pub(crate) fn take(&mut self) -> JournalBatch {
        JournalBatch {
            entries: std::mem::take(&mut self.entries).into(),
            dropped: std::mem::take(&mut self.dropped),
        }
    }

    /// The entries not yet drained, oldest first.
    pub fn entries(&self) -> impl Iterator<Item = &JournalEntry> {
        self.entries.iter()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Entries drained from the journal, oldest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JournalBatch {
    pub entries: Vec<JournalEntry>,
    /// Entries lost because the journal filled before this drain.
    pub dropped: u64,
}
