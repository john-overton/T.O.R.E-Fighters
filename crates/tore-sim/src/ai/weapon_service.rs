//! Non-player weapon service: preparation cadence and firing (B42), ordinary
//! ammunition accounting and the seeker envelope (B45), and the device-release
//! schedule consumer from the experience spec ("Other experience effects").
//!
//! Everything here is spec-derived from [`docs/spec/ai.md`](../../../../docs/spec/ai.md)
//! and [`docs/spec/ai-experience.md`](../../../../docs/spec/ai-experience.md).
//! Every fact the service needs from the rest of the simulation (target
//! validity, station availability, lock results, terrain blocking, projectile
//! pacing, signature) is an explicit input; the component never looks anything
//! up and never substitutes a default for an unresolved rule.
//!
//! Timings are nominal simulation seconds on the quarter-second clock (B13):
//! a deadline is the sampled quarter count plus four counts per second, and it
//! is eligible once the clock reaches it. Clocks are sampled when a phase is
//! entered, never redrawn on every 120 Hz call.
//!
//! Fitted choices (agent decisions, 2026-09-17), each where the spec is silent:
//!
//! - A blocked firing path keeps the service in the Fire phase and reports
//!   `Withheld(PathBlocked)`; the spec only says the check precedes firing.
//! - The 10% half-second addition applies to the search, prepare, lock-retry
//!   and no-station delays, not to the weapon's own tracking delay.
//! - The 15 s window opens at Prepare entry and reopens after a shot or a
//!   lock loss; after expiry the service parks in `WindowExpired` until the
//!   target is lost, because the spec records no consequence.
//! - The relative-altitude and signature checks sit under B45's range
//!   checking flag together with the range limits.

use super::{AiError, DecisionRandom, QUARTER_SECOND_TICKS, Result, ScalarSpeed};
use tore_formats::aircraft::AircraftId;

/// Opaque actor identity supplied by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ActorId(pub u32);
/// Opaque target identity supplied by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TargetId(pub u32);
/// Opaque weapon station identity supplied by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StationId(pub u8);
/// Unique identity of one fire request. Feedback keyed by this identity cannot
/// fire twice ("Intents, feedback and persistence").
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RequestId(pub u64);

/// A nominal delay expressed in quarter-second counts (B13). Seconds convert
/// exactly; the clock cannot express finer fractions, so a tracking delay that
/// is not a whole number of quarter seconds is invalid input rather than being
/// rounded silently.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Delay {
    quarters: u32,
}
impl Delay {
    pub const HALF_SECOND: Self = Self { quarters: 2 };
    pub const fn seconds(seconds: u32) -> Self {
        Self {
            quarters: seconds * 4,
        }
    }
    pub const fn quarters(quarters: u32) -> Self {
        Self { quarters }
    }
    /// Convert a delay given in seconds; must be a multiple of 0.25 s.
    pub fn from_seconds(seconds: f64) -> Result<Self> {
        let quarters = seconds * 4.0;
        if !(quarters.is_finite() && quarters >= 0.0 && quarters.fract() == 0.0) {
            return Err(AiError::InvalidInput(
                "delay must be a non-negative multiple of a quarter second",
            ));
        }
        Ok(Self {
            quarters: quarters as u32,
        })
    }
    pub fn quarter_count(self) -> u32 {
        self.quarters
    }
    fn plus(self, other: Self) -> Self {
        Self {
            quarters: self.quarters + other.quarters,
        }
    }
}

/// Quarter-second clock count for a simulation tick.
pub fn quarter_clock(tick: u64) -> u64 {
    tick / QUARTER_SECOND_TICKS
}

/// B42: imported per-aircraft NPC timings. These are neither experience levels
/// nor flight capability; they only pace the weapon service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TimingProfile {
    /// Nominal no-target retry.
    pub search_delay_s: u32,
    /// Nominal initial preparation delay on the ordinary branch.
    pub prepare_delay_s: u32,
    /// Nominal preparation delay on the branch selected by the unready flag.
    pub unready_prepare_delay_s: u32,
}
impl TimingProfile {
    /// The twelve PT records in B42's table.
    pub fn for_aircraft(id: AircraftId) -> Self {
        let search_delay_s = match id {
            AircraftId::F18
            | AircraftId::Rafale
            | AircraftId::F14
            | AircraftId::X31
            | AircraftId::Su25 => 3,
            AircraftId::A4E
            | AircraftId::Mig29
            | AircraftId::Su27
            | AircraftId::Mig21
            | AircraftId::Mig23
            | AircraftId::Su35
            | AircraftId::F22
            | AircraftId::Faxx => 5,
        };
        Self {
            search_delay_s,
            prepare_delay_s: 5,
            unready_prepare_delay_s: 8,
        }
    }
    fn search_delay(&self) -> Delay {
        Delay::seconds(self.search_delay_s)
    }
    fn prepare_delay(&self, unready: bool) -> Delay {
        if unready {
            Delay::seconds(self.unready_prepare_delay_s)
        } else {
            Delay::seconds(self.prepare_delay_s)
        }
    }
}

/// B42: nominal retry after a failed lock.
pub const LOCK_RETRY: Delay = Delay::seconds(1);
/// B42: nominal retry when no suitable station resolves.
pub const NO_STATION_RETRY: Delay = Delay::seconds(2);
/// B42: preparation/lock window kept by the service.
pub const PREPARATION_WINDOW: Delay = Delay::seconds(15);
/// B42: gate (of 100) for adding half a second to a chosen pre-firing delay.
pub const HALF_SECOND_GATE: u8 = 10;
/// Experience spec: a successful device reaction postpones a finite
/// weapon-service deadline by this much.
pub const DEVICE_REACTION_POSTPONEMENT: Delay = Delay::seconds(2);

/// Result of the weapon-specific lock check, produced by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockStatus {
    /// Lock held; the weapon's own tracking delay precedes firing.
    Locked { tracking_delay: Delay },
    /// Lock not achieved this check.
    Failed,
}

/// B42: burst and reload pacing from the selected projectile and NPC policy.
/// The service carries these unchanged; how they combine into a burst is an
/// unresolved rule (see [`ServiceOutcome`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectilePacing {
    pub burst_count: u32,
    pub burst_interval: Delay,
    pub reload: Delay,
    pub startup: Delay,
}

/// Everything the service reads on one tick. All of it is host-produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ServiceInputs {
    /// The valid hostile target, if any (B41 selection is the host's job).
    pub target: Option<TargetId>,
    /// The available compatible station, if one resolves.
    pub station: Option<StationId>,
    /// B42 unready flag; its producer is unresolved, so it is an input.
    pub unready: bool,
    /// Weapon-specific lock check result for the current target and station.
    pub lock: LockStatus,
    /// True when terrain blocks the firing path (B41/B42).
    pub path_blocked: bool,
    pub pacing: ProjectilePacing,
}

/// Why firing did not happen on a tick in the firing phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WithholdReason {
    PathBlocked,
}

/// Coarse phase of the service, for observers and tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Search,
    Prepare,
    LockCheck,
    /// Lock succeeded; waiting out the weapon's tracking delay.
    Tracking,
    Fire,
    Reload,
    /// The 15 s preparation/lock window expired; consequence unresolved.
    WindowExpired,
}

/// A weapon request handed to the host launch service.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FireRequest {
    pub actor: ActorId,
    pub station: StationId,
    pub target: TargetId,
    pub request_id: RequestId,
}

/// What one `advance` call did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceOutcome {
    /// A deadline is pending and the clock has not reached it.
    Waiting(Phase),
    /// Search found no target and scheduled the aircraft's no-target retry.
    NoTargetRetry { deadline: u64 },
    /// A target exists; preparation started with its nominal delay.
    Preparing { ready_at: u64, window_ends: u64 },
    /// The target was lost; the service returned to search.
    TargetLost,
    /// No suitable station resolved; nominal 2 s retry scheduled.
    NoStationRetry { deadline: u64 },
    /// Lock failed; nominal 1 s retry scheduled.
    LockRetry { deadline: u64 },
    /// Lock succeeded; firing follows the weapon's own tracking delay.
    Locked { fire_at: u64 },
    /// Firing preconditions failed on the firing tick; the phase is kept.
    Withheld(WithholdReason),
    /// The reviewed firing branch was reached; the host must service this.
    Fire(FireRequest),
    /// After firing the station no longer resolves; no-station retry scheduled.
    StoreDepleted { deadline: u64 },
    /// B42's 15 s preparation/lock window expired before firing. The spec
    /// does not say what follows, so the service reports this and waits for
    /// the host; it only leaves this phase when the target is lost.
    WindowExpired,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum State {
    Search {
        retry_at: Option<u64>,
    },
    Prepare {
        ready_at: u64,
        window_ends: u64,
    },
    LockCheck {
        retry_at: Option<u64>,
        window_ends: u64,
    },
    Tracking {
        fire_at: u64,
    },
    Fire,
    Reload,
    WindowExpired,
}

/// Persistent per-actor weapon service state (B42). One per actor; it keeps
/// its clocks across ticks and belongs in replay/save snapshots.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponService {
    actor: ActorId,
    timing: TimingProfile,
    state: State,
    last_tick: Option<u64>,
    next_request: u64,
}
impl WeaponService {
    pub fn new(actor: ActorId, timing: TimingProfile) -> Self {
        Self {
            actor,
            timing,
            state: State::Search { retry_at: None },
            last_tick: None,
            next_request: 0,
        }
    }
    pub fn phase(&self) -> Phase {
        match self.state {
            State::Search { .. } => Phase::Search,
            State::Prepare { .. } => Phase::Prepare,
            State::LockCheck { .. } => Phase::LockCheck,
            State::Tracking { .. } => Phase::Tracking,
            State::Fire => Phase::Fire,
            State::Reload => Phase::Reload,
            State::WindowExpired => Phase::WindowExpired,
        }
    }
    /// The pending finite deadline of the current phase, in quarter counts.
    pub fn deadline(&self) -> Option<u64> {
        match self.state {
            State::Search { retry_at } => retry_at,
            State::Prepare { ready_at, .. } => Some(ready_at),
            State::LockCheck { retry_at, .. } => retry_at,
            State::Tracking { fire_at } => Some(fire_at),
            State::Fire | State::Reload | State::WindowExpired => None,
        }
    }
    /// Experience spec: a successful device reaction postpones a finite
    /// weapon-service deadline by 2 s. Returns the new deadline, or `None` when
    /// the current phase has no finite deadline to postpone.
    pub fn postpone_for_device_reaction(&mut self) -> Option<u64> {
        let extra = u64::from(DEVICE_REACTION_POSTPONEMENT.quarter_count());
        let deadline = match &mut self.state {
            State::Search {
                retry_at: Some(retry_at),
            }
            | State::LockCheck {
                retry_at: Some(retry_at),
                ..
            } => retry_at,
            State::Prepare { ready_at, .. } => ready_at,
            State::Tracking { fire_at } => fire_at,
            State::Search { retry_at: None }
            | State::LockCheck { retry_at: None, .. }
            | State::Fire
            | State::Reload
            | State::WindowExpired => return None,
        };
        *deadline += extra;
        Some(*deadline)
    }

    /// Chosen pre-firing delay with B42's 10% half-second addition.
    fn gated(delay: Delay, random: &mut DecisionRandom) -> Delay {
        if random.chance(HALF_SECOND_GATE) {
            delay.plus(Delay::HALF_SECOND)
        } else {
            delay
        }
    }

    fn enter_search(&mut self) -> ServiceOutcome {
        self.state = State::Search { retry_at: None };
        ServiceOutcome::TargetLost
    }

    fn enter_prepare(
        &mut self,
        now: u64,
        unready: bool,
        random: &mut DecisionRandom,
    ) -> ServiceOutcome {
        let delay = Self::gated(self.timing.prepare_delay(unready), random);
        let ready_at = now + u64::from(delay.quarter_count());
        let window_ends = now + u64::from(PREPARATION_WINDOW.quarter_count());
        self.state = State::Prepare {
            ready_at,
            window_ends,
        };
        ServiceOutcome::Preparing {
            ready_at,
            window_ends,
        }
    }

    fn no_station_retry(
        &mut self,
        now: u64,
        window_ends: u64,
        random: &mut DecisionRandom,
    ) -> ServiceOutcome {
        let deadline = now + u64::from(Self::gated(NO_STATION_RETRY, random).quarter_count());
        self.state = State::Prepare {
            ready_at: deadline,
            window_ends,
        };
        ServiceOutcome::NoStationRetry { deadline }
    }

    /// Advance the service by one simulation tick. Ticks must not go
    /// backwards; a repeated tick (paused host) changes nothing.
    pub fn advance(
        &mut self,
        tick: u64,
        inputs: &ServiceInputs,
        random: &mut DecisionRandom,
    ) -> Result<ServiceOutcome> {
        if self.last_tick.is_some_and(|last| tick < last) {
            return Err(AiError::InvalidInput("weapon service tick went backwards"));
        }
        self.last_tick = Some(tick);
        let now = quarter_clock(tick);
        Ok(match self.state {
            State::Search { retry_at } => {
                if retry_at.is_some_and(|at| now < at) {
                    return Ok(ServiceOutcome::Waiting(Phase::Search));
                }
                if inputs.target.is_some() {
                    self.enter_prepare(now, inputs.unready, random)
                } else {
                    let deadline = now
                        + u64::from(
                            Self::gated(self.timing.search_delay(), random).quarter_count(),
                        );
                    self.state = State::Search {
                        retry_at: Some(deadline),
                    };
                    ServiceOutcome::NoTargetRetry { deadline }
                }
            }
            State::Prepare {
                ready_at,
                window_ends,
            } => {
                if inputs.target.is_none() {
                    return Ok(self.enter_search());
                }
                if now >= window_ends {
                    self.state = State::WindowExpired;
                    return Ok(ServiceOutcome::WindowExpired);
                }
                if now < ready_at {
                    return Ok(ServiceOutcome::Waiting(Phase::Prepare));
                }
                if inputs.station.is_none() {
                    return Ok(self.no_station_retry(now, window_ends, random));
                }
                self.state = State::LockCheck {
                    retry_at: None,
                    window_ends,
                };
                self.check_lock(now, inputs, random)
            }
            State::LockCheck {
                retry_at,
                window_ends,
            } => {
                if inputs.target.is_none() {
                    return Ok(self.enter_search());
                }
                if now >= window_ends {
                    self.state = State::WindowExpired;
                    return Ok(ServiceOutcome::WindowExpired);
                }
                if retry_at.is_some_and(|at| now < at) {
                    return Ok(ServiceOutcome::Waiting(Phase::LockCheck));
                }
                if inputs.station.is_none() {
                    return Ok(self.no_station_retry(now, window_ends, random));
                }
                self.check_lock(now, inputs, random)
            }
            State::Tracking { fire_at } => {
                if inputs.target.is_none() {
                    return Ok(self.enter_search());
                }
                if now < fire_at {
                    return Ok(ServiceOutcome::Waiting(Phase::Tracking));
                }
                self.state = State::Fire;
                self.fire(now, inputs, random)
            }
            State::Fire => self.fire(now, inputs, random),
            State::Reload => {
                if inputs.target.is_none() {
                    return Ok(self.enter_search());
                }
                if inputs.station.is_none() {
                    let deadline =
                        now + u64::from(Self::gated(NO_STATION_RETRY, random).quarter_count());
                    let window_ends = now + u64::from(PREPARATION_WINDOW.quarter_count());
                    self.state = State::Prepare {
                        ready_at: deadline,
                        window_ends,
                    };
                    return Ok(ServiceOutcome::StoreDepleted { deadline });
                }
                return Err(AiError::UnspecifiedRule(
                    "B42 burst/reload pacing after a launch with a resolving station",
                ));
            }
            State::WindowExpired => {
                if inputs.target.is_none() {
                    self.enter_search()
                } else {
                    ServiceOutcome::WindowExpired
                }
            }
        })
    }

    fn check_lock(
        &mut self,
        now: u64,
        inputs: &ServiceInputs,
        random: &mut DecisionRandom,
    ) -> ServiceOutcome {
        let State::LockCheck { window_ends, .. } = self.state else {
            unreachable!("check_lock runs in the lock-check state");
        };
        match inputs.lock {
            LockStatus::Locked { tracking_delay } => {
                let fire_at = now + u64::from(tracking_delay.quarter_count());
                self.state = State::Tracking { fire_at };
                ServiceOutcome::Locked { fire_at }
            }
            LockStatus::Failed => {
                let deadline = now + u64::from(Self::gated(LOCK_RETRY, random).quarter_count());
                self.state = State::LockCheck {
                    retry_at: Some(deadline),
                    window_ends,
                };
                ServiceOutcome::LockRetry { deadline }
            }
        }
    }

    /// The reviewed firing branch: valid hostile target, available compatible
    /// station, lock and an unblocked path all precede the request.
    fn fire(
        &mut self,
        now: u64,
        inputs: &ServiceInputs,
        random: &mut DecisionRandom,
    ) -> ServiceOutcome {
        let Some(target) = inputs.target else {
            return self.enter_search();
        };
        let Some(station) = inputs.station else {
            let window_ends = now + u64::from(PREPARATION_WINDOW.quarter_count());
            return self.no_station_retry(now, window_ends, random);
        };
        if !matches!(inputs.lock, LockStatus::Locked { .. }) {
            let window_ends = now + u64::from(PREPARATION_WINDOW.quarter_count());
            self.state = State::LockCheck {
                retry_at: None,
                window_ends,
            };
            return self.check_lock(now, inputs, random);
        }
        if inputs.path_blocked {
            return ServiceOutcome::Withheld(WithholdReason::PathBlocked);
        }
        let request_id = RequestId(self.next_request);
        self.next_request += 1;
        self.state = State::Reload;
        ServiceOutcome::Fire(FireRequest {
            actor: self.actor,
            station,
            target,
            request_id,
        })
    }
}

// ---------------------------------------------------------------------------
// B45: ammunition accounting
// ---------------------------------------------------------------------------

/// Rounds held by one store.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounds {
    /// The unlimited-store sentinel: releases succeed without decrement.
    Unlimited,
    Finite(u32),
}

/// One weapon station's inventory state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StoreState {
    /// The host's inhibition reasons collapse to this flag for accounting.
    pub inhibited: bool,
    pub rounds: Rounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebitRefusal {
    Inhibited,
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DebitOutcome {
    Refused(DebitRefusal),
    /// The unlimited sentinel: nothing changed.
    Unlimited,
    /// Finite rounds reduced. `partial` marks a final debit that was smaller
    /// than the requested amount because the remainder was smaller.
    Debited {
        remaining: u32,
        partial: bool,
    },
}

/// B45 ordinary ammunition accounting. `debit` is the equipment's
/// actual-rounds-per-game-round amount and must be positive.
pub fn debit_ammunition(store: &mut StoreState, debit: u32) -> Result<DebitOutcome> {
    if debit == 0 {
        return Err(AiError::InvalidInput("ammunition debit must be positive"));
    }
    if store.inhibited {
        return Ok(DebitOutcome::Refused(DebitRefusal::Inhibited));
    }
    match store.rounds {
        Rounds::Unlimited => Ok(DebitOutcome::Unlimited),
        Rounds::Finite(0) => Ok(DebitOutcome::Refused(DebitRefusal::Empty)),
        Rounds::Finite(remaining) => {
            let partial = remaining < debit;
            let remaining = remaining.saturating_sub(debit);
            store.rounds = Rounds::Finite(remaining);
            Ok(DebitOutcome::Debited { remaining, partial })
        }
    }
}

/// Whether the released weapon keeps its target. B45: the lower-level release
/// routine can clear an invalid or terrain-blocked target and still release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetRetention {
    Retained(TargetId),
    ClearedInvalid,
    ClearedTerrainBlocked,
}

/// Inventory effect of one release operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InventoryChange {
    /// Human control plus the global unlimited-ammunition setting.
    GlobalUnlimitedBypass,
    /// Atomic policy: allocation failed, so nothing was debited.
    NotDebited,
    Debit(DebitOutcome),
}

/// Projectiles created by one release operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileCreation {
    None,
    /// Pod/burst metadata can make one release create several projectiles;
    /// the count is host metadata, not the ammunition debit.
    Created {
        count: u32,
    },
}

/// B45 service report: inventory change, projectile creation and target
/// retention are three distinct facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseReport {
    pub inventory: InventoryChange,
    pub projectiles: ProjectileCreation,
    pub target: TargetRetention,
}

/// One release operation's host-supplied facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReleaseRequest {
    /// Actual rounds per game round for this equipment (positive).
    pub debit: u32,
    /// Projectiles this operation creates when allocation succeeds.
    pub projectile_count: u32,
    /// Whether the host could allocate the projectile(s).
    pub allocation_succeeds: bool,
    pub target: TargetRetention,
    /// The global unlimited-ammunition bypass requires human control.
    pub human_controlled: bool,
    pub global_unlimited: bool,
    /// Opinionated (agent choice, 2026-09-17): allocate before debiting so a
    /// failed allocation never consumes ammunition. B45 records that the
    /// original debits first; the default `false` keeps that order.
    pub atomic_release: bool,
}

/// B45 release accounting around the host's projectile allocation.
pub fn release(store: &mut StoreState, request: &ReleaseRequest) -> Result<ReleaseReport> {
    let created = || {
        if request.allocation_succeeds {
            ProjectileCreation::Created {
                count: request.projectile_count,
            }
        } else {
            ProjectileCreation::None
        }
    };
    if request.human_controlled && request.global_unlimited {
        return Ok(ReleaseReport {
            inventory: InventoryChange::GlobalUnlimitedBypass,
            projectiles: created(),
            target: request.target,
        });
    }
    if request.atomic_release && !request.allocation_succeeds {
        return Ok(ReleaseReport {
            inventory: InventoryChange::NotDebited,
            projectiles: ProjectileCreation::None,
            target: request.target,
        });
    }
    let debit = debit_ammunition(store, request.debit)?;
    let projectiles = match debit {
        DebitOutcome::Refused(_) => ProjectileCreation::None,
        DebitOutcome::Unlimited | DebitOutcome::Debited { .. } => created(),
    };
    Ok(ReleaseReport {
        inventory: InventoryChange::Debit(debit),
        projectiles,
        target: request.target,
    })
}

// ---------------------------------------------------------------------------
// B45: seeker envelope
// ---------------------------------------------------------------------------

/// The selected equipment profile's envelope. `None` is the sentinel that
/// disables that individual bound.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeekerEnvelope {
    /// When false, range, vertical and signature checks are skipped. Angles
    /// are governed separately by their own sentinels.
    pub range_checking: bool,
    pub min_range_ft: Option<f64>,
    pub max_range_ft: Option<f64>,
    /// Inclusive limits on target altitude relative to the observer.
    pub min_relative_altitude_ft: Option<f64>,
    pub max_relative_altitude_ft: Option<f64>,
    /// Inclusive angular limits in the observer/mount frame, degrees.
    pub horizontal_limit_deg: Option<f64>,
    pub vertical_limit_deg: Option<f64>,
}

/// Target geometry in the observer/mount frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SeekerGeometry {
    pub range_ft: f64,
    pub relative_altitude_ft: f64,
    pub horizontal_error_deg: f64,
    pub vertical_error_deg: f64,
    /// Final signature percentage. Aspect, look-down and speed-sensitive
    /// modifiers are unresolved, so the host supplies the final value.
    pub signature_percent: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeRejection {
    BelowMinimumRange,
    AboveMaximumRange,
    BelowVerticalLimit,
    AboveVerticalLimit,
    ZeroSignature,
    SignatureAdjustedRangeExceedsMaximum,
    OutsideAngularLimits,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnvelopeResult {
    Eligible,
    Rejected(EnvelopeRejection),
}

/// B45 geometric and signature envelope check. Passing it is one of several
/// separate eligibility checks; it is not launch authorization.
pub fn envelope_check(profile: &SeekerEnvelope, geometry: &SeekerGeometry) -> EnvelopeResult {
    use EnvelopeRejection as R;
    if profile.range_checking {
        if profile
            .min_range_ft
            .is_some_and(|min| geometry.range_ft < min)
        {
            return EnvelopeResult::Rejected(R::BelowMinimumRange);
        }
        if profile
            .max_range_ft
            .is_some_and(|max| geometry.range_ft > max)
        {
            return EnvelopeResult::Rejected(R::AboveMaximumRange);
        }
        if profile
            .min_relative_altitude_ft
            .is_some_and(|min| geometry.relative_altitude_ft < min)
        {
            return EnvelopeResult::Rejected(R::BelowVerticalLimit);
        }
        if profile
            .max_relative_altitude_ft
            .is_some_and(|max| geometry.relative_altitude_ft > max)
        {
            return EnvelopeResult::Rejected(R::AboveVerticalLimit);
        }
        if let Some(max) = profile.max_range_ft {
            if geometry.signature_percent == 0 {
                return EnvelopeResult::Rejected(R::ZeroSignature);
            }
            let effective = geometry.range_ft * 100.0 / f64::from(geometry.signature_percent);
            if effective > max {
                return EnvelopeResult::Rejected(R::SignatureAdjustedRangeExceedsMaximum);
            }
        }
    }
    if angles_inside(profile, geometry) {
        EnvelopeResult::Eligible
    } else {
        EnvelopeResult::Rejected(R::OutsideAngularLimits)
    }
}

fn angles_inside(profile: &SeekerEnvelope, geometry: &SeekerGeometry) -> bool {
    if profile.horizontal_limit_deg.is_none() && profile.vertical_limit_deg.is_none() {
        return true;
    }
    // A single sentinel leaves that one bound unrestricted.
    let h_limit = profile.horizontal_limit_deg.unwrap_or(f64::INFINITY);
    let v_limit = profile.vertical_limit_deg.unwrap_or(f64::INFINITY);
    let h = geometry.horizontal_error_deg.abs();
    let v = geometry.vertical_error_deg.abs();
    if v > 90.0 {
        h <= h_limit.max(90.0) || v >= 180.0 - v_limit
    } else {
        h <= h_limit && v <= v_limit
    }
}

// ---------------------------------------------------------------------------
// B45: two envelopes per store
// ---------------------------------------------------------------------------

/// B45: which of a store's two envelopes a check is made against. Zone 0 is
/// what the seeker can acquire (sensor and target search); zone 1 is what the
/// store may be employed against (lock, launch, store choice and in-flight
/// support). Callers state the role so the two are never mixed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EnvelopeRole {
    /// Zone 0: acquisition.
    Acquisition,
    /// Zone 1: employment.
    Employment,
}

/// B45: the two envelopes carried by every store, each with its own range,
/// relative altitude and angular limits. The values come from the imported
/// weapon records, not from this module.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StoreEnvelopes {
    pub acquisition: SeekerEnvelope,
    pub employment: SeekerEnvelope,
}
impl StoreEnvelopes {
    pub fn envelope(&self, role: EnvelopeRole) -> &SeekerEnvelope {
        match role {
            EnvelopeRole::Acquisition => &self.acquisition,
            EnvelopeRole::Employment => &self.employment,
        }
    }
    /// [`envelope_check`] against the envelope named by `role`.
    pub fn check(&self, role: EnvelopeRole, geometry: &SeekerGeometry) -> EnvelopeResult {
        envelope_check(self.envelope(role), geometry)
    }
}

// ---------------------------------------------------------------------------
// B45: signature and detection range
// ---------------------------------------------------------------------------
//
// Only the stated pieces are implemented. The starting stored signatures, the
// radar attitude/configuration model, the hot-engine heat window, the weather
// scale and the "lit target" state are host/sensor inputs.

/// B45: a signature above this percentage does not extend reach beyond the
/// profile maximum.
pub const SIGNATURE_FULL_PERCENT: u32 = 100;
/// B45: rear cone elevation half-angle, degrees off the target's tail.
pub const REAR_CONE_ELEVATION_DEG: f64 = 40.0;
/// B45: rear cone azimuth half-angle, degrees off the target's tail.
pub const REAR_CONE_AZIMUTH_DEG: f64 = 140.0;
/// B45: an observer pointing within this many degrees of vertical bypasses
/// the aspect penalty.
pub const VERTICAL_POINTING_TOLERANCE_DEG: f64 = 30.0;
/// B45: an infrared sensor sees at least double the stored signature while
/// the target is hot, and never below this percentage.
pub const INFRARED_HOT_FLOOR_PERCENT: u32 = 200;
/// B45: first radar configuration bonus, points.
pub const RADAR_BONUS_A_PERCENT: u32 = 33;
/// B45: second radar configuration bonus, points.
pub const RADAR_BONUS_B_PERCENT: u32 = 25;
/// B45: each radar bonus is floored at this percentage after adding.
pub const RADAR_BONUS_FLOOR_PERCENT: u32 = 100;
/// B45: a lit target's visual and laser signature divides by 1 at or inside
/// this range at night.
pub const NIGHT_DIVISOR_START_FT: f64 = 1500.0;
/// B45: the night divisor reaches its full value at this range.
pub const NIGHT_DIVISOR_END_FT: f64 = 4500.0;
/// B45: the full night divisor.
pub const NIGHT_DIVISOR_MAX: f64 = 5.0;
/// B45: the naked eye is lifted to at least this percentage inside
/// [`VISUAL_FLOOR_FULL_FT`].
pub const VISUAL_FLOOR_PERCENT: u32 = 75;
/// B45: range inside which the visual floor applies in full.
pub const VISUAL_FLOOR_FULL_FT: f64 = 200.0;
/// B45: range at which the visual floor has blended back to the weather value.
pub const VISUAL_FLOOR_BLEND_END_FT: f64 = 1500.0;
/// B45: look-down rejection reaches full strength at this down angle.
pub const LOOK_DOWN_FULL_ANGLE_DEG: f64 = 45.0;
/// B45: look-down rejection vanishes at this height above ground.
pub const LOOK_DOWN_VANISH_AGL_FT: f64 = 5000.0;

/// B45: detection range is the profile maximum times the final signature
/// percentage over 100; a signature above 100 does not extend reach. A
/// `None` maximum is the unbounded sentinel and stays unbounded.
pub fn detection_range_feet(
    profile_max_range: Option<f64>,
    final_signature_percent: u32,
) -> Option<f64> {
    let scale = f64::from(final_signature_percent.min(SIGNATURE_FULL_PERCENT))
        / f64::from(SIGNATURE_FULL_PERCENT);
    profile_max_range.map(|max| max * scale)
}

/// B45: the observer is inside the target's rear cone when its elevation off
/// the tail is within 40 degrees and its azimuth off the tail within 140
/// degrees, both inclusive.
pub fn in_rear_cone(elevation_off_tail_deg: f64, azimuth_off_tail_deg: f64) -> bool {
    elevation_off_tail_deg.abs() <= REAR_CONE_ELEVATION_DEG
        && azimuth_off_tail_deg.abs() <= REAR_CONE_AZIMUTH_DEG
}

/// B45: the observer is pointing within 30 degrees of straight up or straight
/// down (pitch magnitude at least 60 degrees, inclusive).
pub fn pointing_vertical(pitch_deg: f64) -> bool {
    90.0 - pitch_deg.abs() <= VERTICAL_POINTING_TOLERANCE_DEG
}

/// B45: the seeker's aspect penalty is subtracted unless the observer is
/// inside the rear cone or pointing near vertical.
pub fn aspect_penalty_applies(
    observer_in_rear_cone: bool,
    observer_pointing_vertical: bool,
) -> bool {
    !observer_in_rear_cone && !observer_pointing_vertical
}

/// B45: subtract the seeker's aspect penalty (saturating at zero) when it
/// applies. A 100 percent penalty (the AA-2) cannot see a target from the
/// front; the Sidewinder family carries 30 or 20 and radar missiles 0.
pub fn apply_aspect_penalty(signature_percent: u32, penalty_percent: u32, applies: bool) -> u32 {
    if applies {
        signature_percent.saturating_sub(penalty_percent)
    } else {
        signature_percent
    }
}

/// B45: an infrared sensor sees at least double, never below 200 percent,
/// while the target is in the hot-engine state or its recent heat window.
/// Whether the target is hot is a host input.
pub fn infrared_boost(signature_percent: u32, hot_engine: bool) -> u32 {
    if hot_engine {
        signature_percent
            .saturating_mul(2)
            .max(INFRARED_HOT_FLOOR_PERCENT)
    } else {
        signature_percent
    }
}

/// B45: two radar configuration bonuses of 33 and 25 points, each floored at
/// 100 after adding. The bonus conditions are host inputs; the attitude and
/// configuration model that produces the starting percentage is not stated.
pub fn radar_configuration_bonus(signature_percent: u32, bonus_a: bool, bonus_b: bool) -> u32 {
    let mut signature = signature_percent;
    if bonus_a {
        signature = signature
            .saturating_add(RADAR_BONUS_A_PERCENT)
            .max(RADAR_BONUS_FLOOR_PERCENT);
    }
    if bonus_b {
        signature = signature
            .saturating_add(RADAR_BONUS_B_PERCENT)
            .max(RADAR_BONUS_FLOOR_PERCENT);
    }
    signature
}

/// B45: a passive emitter seeker ignores stored signatures: 100 percent while
/// the target radiates, otherwise 0.
pub fn emitter_signature(target_emitting: bool) -> u32 {
    if target_emitting {
        SIGNATURE_FULL_PERCENT
    } else {
        0
    }
}

/// B45: at night a lit target's visual and laser signature divides by up to
/// 5 between 1500 and 4500 ft: 1 at or inside 1500 ft, rising linearly to 5
/// at 4500 ft, 5 beyond. The caller applies it only at night to lit targets.
pub fn night_visual_divisor(range_feet: f64) -> f64 {
    let span = NIGHT_DIVISOR_END_FT - NIGHT_DIVISOR_START_FT;
    let fraction = ((range_feet - NIGHT_DIVISOR_START_FT) / span).clamp(0.0, 1.0);
    1.0 + (NIGHT_DIVISOR_MAX - 1.0) * fraction
}

/// B45: the naked eye is lifted to at least 75 percent within 200 ft and
/// blended back to the weather-scaled value by 1500 ft. Between those ranges
/// the floor falls linearly from 75 to the weather value; the result is never
/// below the weather value itself.
pub fn visual_close_range_floor(range_feet: f64, weather_percent: u32) -> u32 {
    let weather = f64::from(weather_percent);
    let span = VISUAL_FLOOR_BLEND_END_FT - VISUAL_FLOOR_FULL_FT;
    let fraction = ((range_feet - VISUAL_FLOOR_FULL_FT) / span).clamp(0.0, 1.0);
    let floor =
        f64::from(VISUAL_FLOOR_PERCENT) + (weather - f64::from(VISUAL_FLOOR_PERCENT)) * fraction;
    floor.max(weather).round() as u32
}

/// B45: look-down rejection strength in `0..=1`. It reaches full strength at
/// 45 degrees down and on the deck, and vanishes at 5000 ft above ground:
/// the larger of the angle term (down angle over 45, capped at 1) and the
/// height term (1 minus height over 5000, floored at 0). An upward angle
/// contributes nothing.
pub fn look_down_rejection(down_angle_deg: f64, target_agl_feet: f64) -> f64 {
    let angle_term = (down_angle_deg / LOOK_DOWN_FULL_ANGLE_DEG).clamp(0.0, 1.0);
    let height_term = (1.0 - target_agl_feet / LOOK_DOWN_VANISH_AGL_FT).clamp(0.0, 1.0);
    angle_term.max(height_term)
}

// ---------------------------------------------------------------------------
// B45: target-class eligibility and store selection
// ---------------------------------------------------------------------------

/// B45: the one class bit a target falls under for store eligibility.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetClass {
    Air,
    Surface,
}

/// B45: which target classes a store may be used against.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct StoreCapability {
    pub air: bool,
    pub surface: bool,
}
impl StoreCapability {
    /// Air-to-air missiles: aircraft only.
    pub const AIR_TO_AIR_MISSILE: Self = Self {
        air: true,
        surface: false,
    };
    /// Bombs, rockets and ground missiles: surface targets only.
    pub const SURFACE_STORE: Self = Self {
        air: false,
        surface: true,
    };
    /// Guns: both.
    pub const GUN: Self = Self {
        air: true,
        surface: true,
    };
}

/// B45: a store is eligible against a target when its class bit is set.
pub fn store_eligible(capability: StoreCapability, class: TargetClass) -> bool {
    match class {
        TargetClass::Air => capability.air,
        TargetClass::Surface => capability.surface,
    }
}

/// B45: guided angular term base, points minus pointing error in degrees.
pub const GUIDED_ANGLE_BASE: f64 = 100.0;
/// B45: guns and unguided stores score twice (50 minus pointing error).
pub const UNGUIDED_ANGLE_BASE: f64 = 50.0;
/// B45: multiplier on the unguided angular term.
pub const UNGUIDED_ANGLE_SCALE: f64 = 2.0;
/// B45: bonus for a guided store beyond [`GUIDED_RANGE_BONUS_BEYOND_FT`].
pub const GUIDED_RANGE_BONUS: f64 = 50.0;
/// B45: range a guided store must exceed (strictly) to earn the bonus.
pub const GUIDED_RANGE_BONUS_BEYOND_FT: f64 = 1500.0;
/// B45: damage against the target's category is divided by this.
pub const DAMAGE_SCORE_DIVISOR: f64 = 25.0;
/// B45 store choice considers at most this many usable stations. More is
/// invalid input, not a silent truncation.
pub const STORE_CANDIDATE_LIMIT: usize = 10;

/// B45: one usable station against the current target, as resolved by the
/// host. Every field is an input; the score combines them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StoreCandidate {
    pub station: StationId,
    pub guided: bool,
    /// Pointing error in degrees when the employment envelope passed, `None`
    /// when it did not (the angular term is then zero).
    pub employment_fit: Option<f64>,
    /// The store's hit chance against the target. B45 records the producing
    /// routine as opaque and not yet stated, so it is an input here.
    pub hit_chance: f64,
    pub range_feet: f64,
    /// The store's damage against the target's category.
    pub damage_vs_category: f64,
}

/// B45 store score: the employment-envelope angular term (100 minus pointing
/// error for guided stores, twice 50 minus error for guns and unguided
/// stores, zero when the envelope failed), plus hit chance, plus 50 for a
/// guided store beyond 1500 ft, plus damage against the target's category
/// over 25.
pub fn store_score(candidate: &StoreCandidate) -> f64 {
    let angle = match candidate.employment_fit {
        Some(error_deg) if candidate.guided => GUIDED_ANGLE_BASE - error_deg,
        Some(error_deg) => UNGUIDED_ANGLE_SCALE * (UNGUIDED_ANGLE_BASE - error_deg),
        None => 0.0,
    };
    let range_bonus = if candidate.guided && candidate.range_feet > GUIDED_RANGE_BONUS_BEYOND_FT {
        GUIDED_RANGE_BONUS
    } else {
        0.0
    };
    angle + candidate.hit_chance + range_bonus + candidate.damage_vs_category / DAMAGE_SCORE_DIVISOR
}

/// B45: index of the highest-scoring candidate; the first wins a tie. `None`
/// for an empty slice; more than [`STORE_CANDIDATE_LIMIT`] candidates is
/// invalid input.
pub fn select_store(candidates: &[StoreCandidate]) -> Result<Option<usize>> {
    if candidates.len() > STORE_CANDIDATE_LIMIT {
        return Err(AiError::InvalidInput(
            "store selection considers at most ten candidates",
        ));
    }
    let mut best: Option<(usize, f64)> = None;
    for (index, candidate) in candidates.iter().enumerate() {
        let score = store_score(candidate);
        if best.is_none_or(|(_, best_score)| score > best_score) {
            best = Some((index, score));
        }
    }
    Ok(best.map(|(index, _)| index))
}

// ---------------------------------------------------------------------------
// B45: in-flight support
// ---------------------------------------------------------------------------

/// B45: the launcher-support facts a guided weapon's equipment can require,
/// gathered by the host on each in-flight update. Support flags stay separate
/// rather than collapsing into one universal support channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SupportRequirement {
    /// The equipment requires launcher support at all.
    pub requires_launcher: bool,
    pub launcher_alive: bool,
    pub launcher_emitting: bool,
    pub launcher_human: bool,
    /// For a human launcher: the pilot still holds the target in radar.
    pub human_launcher_holds_target_in_radar: bool,
}

/// B45: result of a guided weapon's per-update target re-check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrackOutcome {
    Tracking,
    /// The weapon simply loses its target: it is not destroyed, flies on
    /// unguided until its normal lifetime ends, and does not reacquire.
    TargetLost,
}

/// B45 in-flight target re-check. Only the angular limits of the employment
/// envelope apply (`employment_angles_pass` is the host's result of that
/// check); range limits do not, so the weapon never loses its target by
/// closing inside its own launch minimum range. When support is required the
/// launcher must be alive; an AI launcher's support does not lapse on its own
/// while the missile keeps asking, whereas a human launcher's support lapses
/// when the pilot stops emitting or no longer holds the target in radar.
pub fn in_flight_track_check(
    employment_angles_pass: bool,
    support: &SupportRequirement,
) -> TrackOutcome {
    if !employment_angles_pass {
        return TrackOutcome::TargetLost;
    }
    if support.requires_launcher {
        if !support.launcher_alive {
            return TrackOutcome::TargetLost;
        }
        if support.launcher_human
            && !(support.launcher_emitting && support.human_launcher_holds_target_in_radar)
        {
            return TrackOutcome::TargetLost;
        }
    }
    TrackOutcome::Tracking
}

/// B45: the AI radar-on check extends the emission-valid deadline to at least
/// this far ahead (10 s on the quarter-second clock).
pub const AI_SUPPORT_EXTENSION_QUARTERS: u64 = 40;

/// B45 AI launcher support check. Requires the actor's emission-enabled state
/// and extends its emission-valid deadline to at least 10 s ahead, returning
/// the new deadline; `None` when emission is not enabled. This is not ten
/// seconds of guidance after radar shutdown.
pub fn ai_launcher_support(
    emission_enabled: bool,
    now_quarters: u64,
    deadline_quarters: u64,
) -> Option<u64> {
    emission_enabled.then(|| deadline_quarters.max(now_quarters + AI_SUPPORT_EXTENSION_QUARTERS))
}

/// B45: a target moving slower than this (0x2400/256 = 36 ft/s) counts as
/// stationary for the passive emitter exception.
pub const PASSIVE_EMITTER_STATIONARY_FPS: ScalarSpeed = ScalarSpeed(36.0);

/// B45: a passive emitter weapon keeps tracking a stationary emitter that has
/// shut down but loses a moving one. `threshold` is normally
/// [`PASSIVE_EMITTER_STATIONARY_FPS`]; a target at or above it is moving.
pub fn passive_emitter_keeps_track(
    target_emitting: bool,
    target_speed: ScalarSpeed,
    threshold: ScalarSpeed,
) -> bool {
    target_emitting || target_speed < threshold
}

// ---------------------------------------------------------------------------
// Device release schedule (experience spec, "Other experience effects")
// ---------------------------------------------------------------------------

/// Result of one device launch as reported by the host device-launch routine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceLaunch {
    Succeeded,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceScheduleOutcome {
    /// Next request rescheduled one quarter-second later.
    Rescheduled { next_due: u64 },
    /// The requested count is exhausted.
    Completed,
    /// A failed launch disables the schedule.
    Disabled,
}

/// Consumer side of the launch-reaction device schedule. The per-level gate
/// (35/50/75/90) lives in the tactics component; this takes its result.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeviceReleaseSchedule {
    remaining: u8,
    next_due: Option<u64>,
}
impl DeviceReleaseSchedule {
    /// Start a schedule when the reaction gate passed. `count` is the
    /// requested number of releases, 2 or 3. The initial request is due on
    /// the current clock count (the bounded draw with bound 1 is zero).
    pub fn begin(gate_passed: bool, count: u8, tick: u64) -> Result<Option<Self>> {
        if !(2..=3).contains(&count) {
            return Err(AiError::InvalidInput("device release count must be 2 or 3"));
        }
        if !gate_passed {
            return Ok(None);
        }
        Ok(Some(Self {
            remaining: count,
            next_due: Some(quarter_clock(tick)),
        }))
    }
    pub fn next_due(&self) -> Option<u64> {
        self.next_due
    }
    pub fn remaining(&self) -> u8 {
        self.remaining
    }
    /// True when a request is scheduled and the clock has reached it.
    pub fn due(&self, tick: u64) -> bool {
        self.next_due.is_some_and(|due| quarter_clock(tick) >= due)
    }
    /// Record the host's launch result for a due request. A success also
    /// postpones the weapon service's finite deadline by 2 s.
    pub fn record(
        &mut self,
        launch: DeviceLaunch,
        tick: u64,
        service: &mut WeaponService,
    ) -> Result<DeviceScheduleOutcome> {
        if !self.due(tick) {
            return Err(AiError::InvalidInput(
                "device launch recorded before it was due",
            ));
        }
        match launch {
            DeviceLaunch::Failed => {
                self.next_due = None;
                Ok(DeviceScheduleOutcome::Disabled)
            }
            DeviceLaunch::Succeeded => {
                service.postpone_for_device_reaction();
                self.remaining -= 1;
                if self.remaining == 0 {
                    self.next_due = None;
                    Ok(DeviceScheduleOutcome::Completed)
                } else {
                    let next_due = quarter_clock(tick) + 1;
                    self.next_due = Some(next_due);
                    Ok(DeviceScheduleOutcome::Rescheduled { next_due })
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const Q: u64 = QUARTER_SECOND_TICKS;

    fn inputs() -> ServiceInputs {
        ServiceInputs {
            target: Some(TargetId(7)),
            station: Some(StationId(2)),
            unready: false,
            lock: LockStatus::Locked {
                tracking_delay: Delay::seconds(1),
            },
            path_blocked: false,
            pacing: ProjectilePacing {
                burst_count: 1,
                burst_interval: Delay::seconds(0),
                reload: Delay::seconds(0),
                startup: Delay::seconds(0),
            },
        }
    }

    /// A seed whose first percentage draw is at or above the 10% gate, so the
    /// first chosen delay is not extended. Tests that need several ungated
    /// draws probe deeper.
    fn ungated_seed(draws: usize) -> u64 {
        (0..u64::MAX)
            .find(|&seed| {
                let mut r = DecisionRandom::seeded(seed);
                (0..draws).all(|_| !r.chance(HALF_SECOND_GATE))
            })
            .expect("an ungated seed exists")
    }

    fn service(id: AircraftId) -> WeaponService {
        WeaponService::new(ActorId(1), TimingProfile::for_aircraft(id))
    }

    #[test]
    fn timing_profile_covers_all_twelve() {
        let three = [
            AircraftId::F18,
            AircraftId::Rafale,
            AircraftId::F14,
            AircraftId::X31,
            AircraftId::Su25,
        ];
        for id in AircraftId::ALL {
            let profile = TimingProfile::for_aircraft(id);
            let expected = if three.contains(&id) { 3 } else { 5 };
            assert_eq!(profile.search_delay_s, expected, "{id:?}");
            assert_eq!(profile.prepare_delay_s, 5);
            assert_eq!(profile.unready_prepare_delay_s, 8);
        }
    }

    #[test]
    fn no_target_retry_uses_each_timing_group() {
        for id in AircraftId::ALL {
            let mut random = DecisionRandom::seeded(ungated_seed(1));
            let mut service = service(id);
            let no_target = ServiceInputs {
                target: None,
                ..inputs()
            };
            let start = 8 * Q;
            let expected =
                start / Q + 4 * u64::from(TimingProfile::for_aircraft(id).search_delay_s);
            assert_eq!(
                service.advance(start, &no_target, &mut random).unwrap(),
                ServiceOutcome::NoTargetRetry { deadline: expected },
                "{id:?}"
            );
            // Not eligible one tick before the deadline, eligible on it.
            let before = expected * Q - 1;
            assert_eq!(
                service.advance(before, &no_target, &mut random).unwrap(),
                ServiceOutcome::Waiting(Phase::Search)
            );
            assert!(matches!(
                service
                    .advance(expected * Q, &no_target, &mut random)
                    .unwrap(),
                ServiceOutcome::NoTargetRetry { .. }
            ));
        }
    }

    #[test]
    fn preparation_uses_ordinary_and_unready_delays() {
        let mut random = DecisionRandom::seeded(ungated_seed(2));
        let mut ordinary = service(AircraftId::F18);
        assert_eq!(
            ordinary.advance(0, &inputs(), &mut random).unwrap(),
            ServiceOutcome::Preparing {
                ready_at: 20,
                window_ends: 60
            }
        );
        let mut unready = service(AircraftId::F18);
        let flagged = ServiceInputs {
            unready: true,
            ..inputs()
        };
        assert_eq!(
            unready.advance(0, &flagged, &mut random).unwrap(),
            ServiceOutcome::Preparing {
                ready_at: 32,
                window_ends: 60
            }
        );
    }

    #[test]
    fn failed_lock_retries_after_one_second() {
        let mut random = DecisionRandom::seeded(ungated_seed(3));
        let mut service = service(AircraftId::Mig29);
        let failing = ServiceInputs {
            lock: LockStatus::Failed,
            ..inputs()
        };
        service.advance(0, &failing, &mut random).unwrap();
        assert_eq!(
            service.advance(20 * Q, &failing, &mut random).unwrap(),
            ServiceOutcome::LockRetry { deadline: 24 }
        );
        assert_eq!(
            service.advance(24 * Q - 1, &failing, &mut random).unwrap(),
            ServiceOutcome::Waiting(Phase::LockCheck)
        );
        assert_eq!(
            service.advance(24 * Q, &failing, &mut random).unwrap(),
            ServiceOutcome::LockRetry { deadline: 28 }
        );
    }

    #[test]
    fn missing_station_retries_after_two_seconds() {
        let mut random = DecisionRandom::seeded(ungated_seed(3));
        let mut service = service(AircraftId::F22);
        let no_station = ServiceInputs {
            station: None,
            ..inputs()
        };
        service.advance(0, &no_station, &mut random).unwrap();
        assert_eq!(
            service.advance(20 * Q, &no_station, &mut random).unwrap(),
            ServiceOutcome::NoStationRetry { deadline: 28 }
        );
        assert_eq!(
            service.advance(27 * Q, &no_station, &mut random).unwrap(),
            ServiceOutcome::Waiting(Phase::Prepare)
        );
        assert_eq!(
            service.advance(28 * Q, &no_station, &mut random).unwrap(),
            ServiceOutcome::NoStationRetry { deadline: 36 }
        );
    }

    #[test]
    fn half_second_addition_happens_ten_percent_of_the_time() {
        let mut extended = 0;
        let runs: u32 = 20_000;
        for seed in 0..runs {
            let mut random = DecisionRandom::seeded(u64::from(seed));
            let mut service = service(AircraftId::F18);
            match service.advance(0, &inputs(), &mut random).unwrap() {
                ServiceOutcome::Preparing { ready_at: 22, .. } => extended += 1,
                ServiceOutcome::Preparing { ready_at: 20, .. } => {}
                other => panic!("{other:?}"),
            }
        }
        let share = f64::from(extended) / f64::from(runs);
        assert!((0.09..0.11).contains(&share), "{share}");
    }

    #[test]
    fn repeated_tick_never_advances_and_backwards_is_rejected() {
        let mut random = DecisionRandom::seeded(ungated_seed(2));
        let mut service = service(AircraftId::F18);
        service.advance(10, &inputs(), &mut random).unwrap();
        let snapshot = service.clone();
        for _ in 0..500 {
            assert_eq!(
                service.advance(10, &inputs(), &mut random).unwrap(),
                ServiceOutcome::Waiting(Phase::Prepare)
            );
        }
        assert_eq!(service, snapshot);
        assert_eq!(
            service.advance(9, &inputs(), &mut random),
            Err(AiError::InvalidInput("weapon service tick went backwards"))
        );
    }

    fn fire_once(
        service: &mut WeaponService,
        start: u64,
        random: &mut DecisionRandom,
    ) -> FireRequest {
        let mut tick = start;
        loop {
            match service.advance(tick, &inputs(), random).unwrap() {
                ServiceOutcome::Fire(request) => return request,
                ServiceOutcome::Withheld(_) | ServiceOutcome::WindowExpired => panic!(),
                _ => tick += Q,
            }
        }
    }

    #[test]
    fn firing_follows_lock_and_tracking_delay_with_unique_request_ids() {
        let mut random = DecisionRandom::seeded(ungated_seed(2));
        let mut service = service(AircraftId::Su27);
        assert!(matches!(
            service.advance(0, &inputs(), &mut random).unwrap(),
            ServiceOutcome::Preparing { ready_at: 20, .. }
        ));
        assert_eq!(
            service.advance(20 * Q, &inputs(), &mut random).unwrap(),
            ServiceOutcome::Locked { fire_at: 24 }
        );
        assert_eq!(
            service.advance(23 * Q, &inputs(), &mut random).unwrap(),
            ServiceOutcome::Waiting(Phase::Tracking)
        );
        let first = match service.advance(24 * Q, &inputs(), &mut random).unwrap() {
            ServiceOutcome::Fire(request) => request,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            first,
            FireRequest {
                actor: ActorId(1),
                station: StationId(2),
                target: TargetId(7),
                request_id: RequestId(0),
            }
        );
        // Burst pacing beyond B42 is unresolved.
        assert_eq!(
            service.advance(25 * Q, &inputs(), &mut random),
            Err(AiError::UnspecifiedRule(
                "B42 burst/reload pacing after a launch with a resolving station"
            ))
        );
        // Losing the target returns to search; a later engagement gets a new id.
        let lost = ServiceInputs {
            target: None,
            ..inputs()
        };
        assert_eq!(
            service.advance(26 * Q, &lost, &mut random).unwrap(),
            ServiceOutcome::TargetLost
        );
        let mut random = DecisionRandom::seeded(ungated_seed(4));
        let second = fire_once(&mut service, 27 * Q, &mut random);
        assert_eq!(second.request_id, RequestId(1));
        assert_ne!(first.request_id, second.request_id);
    }

    #[test]
    fn depleted_store_after_firing_changes_state() {
        let mut random = DecisionRandom::seeded(ungated_seed(3));
        let mut service = service(AircraftId::A4E);
        fire_once(&mut service, 0, &mut random);
        assert_eq!(service.phase(), Phase::Reload);
        let depleted = ServiceInputs {
            station: None,
            ..inputs()
        };
        assert_eq!(
            service.advance(30 * Q, &depleted, &mut random).unwrap(),
            ServiceOutcome::StoreDepleted { deadline: 38 }
        );
        assert_eq!(service.phase(), Phase::Prepare);
    }

    #[test]
    fn blocked_path_withholds_fire() {
        let mut random = DecisionRandom::seeded(ungated_seed(2));
        let mut service = service(AircraftId::F14);
        service.advance(0, &inputs(), &mut random).unwrap();
        service.advance(20 * Q, &inputs(), &mut random).unwrap();
        let blocked = ServiceInputs {
            path_blocked: true,
            ..inputs()
        };
        assert_eq!(
            service.advance(24 * Q, &blocked, &mut random).unwrap(),
            ServiceOutcome::Withheld(WithholdReason::PathBlocked)
        );
        assert_eq!(service.phase(), Phase::Fire);
        assert!(matches!(
            service.advance(25 * Q, &inputs(), &mut random).unwrap(),
            ServiceOutcome::Fire(_)
        ));
    }

    #[test]
    fn preparation_window_expiry_is_reported_not_guessed() {
        let mut random = DecisionRandom::seeded(ungated_seed(64));
        let mut service = service(AircraftId::Mig21);
        let failing = ServiceInputs {
            lock: LockStatus::Failed,
            ..inputs()
        };
        service.advance(0, &failing, &mut random).unwrap();
        let mut tick = 20 * Q;
        while tick < 60 * Q {
            assert_ne!(
                service.advance(tick, &failing, &mut random).unwrap(),
                ServiceOutcome::WindowExpired
            );
            tick += Q;
        }
        assert_eq!(
            service.advance(60 * Q, &failing, &mut random).unwrap(),
            ServiceOutcome::WindowExpired
        );
        assert_eq!(service.phase(), Phase::WindowExpired);
        let lost = ServiceInputs {
            target: None,
            ..failing
        };
        assert_eq!(
            service.advance(61 * Q, &lost, &mut random).unwrap(),
            ServiceOutcome::TargetLost
        );
    }

    #[test]
    fn delay_from_seconds_requires_quarter_multiples() {
        assert_eq!(Delay::from_seconds(1.25), Ok(Delay::quarters(5)));
        assert!(Delay::from_seconds(0.1).is_err());
        assert!(Delay::from_seconds(-1.0).is_err());
    }

    #[test]
    fn debit_cases() {
        let mut inhibited = StoreState {
            inhibited: true,
            rounds: Rounds::Finite(10),
        };
        assert_eq!(
            debit_ammunition(&mut inhibited, 1),
            Ok(DebitOutcome::Refused(DebitRefusal::Inhibited))
        );
        assert_eq!(inhibited.rounds, Rounds::Finite(10));

        let mut empty = StoreState {
            inhibited: false,
            rounds: Rounds::Finite(0),
        };
        assert_eq!(
            debit_ammunition(&mut empty, 1),
            Ok(DebitOutcome::Refused(DebitRefusal::Empty))
        );

        let mut unlimited = StoreState {
            inhibited: false,
            rounds: Rounds::Unlimited,
        };
        assert_eq!(
            debit_ammunition(&mut unlimited, 5),
            Ok(DebitOutcome::Unlimited)
        );
        assert_eq!(unlimited.rounds, Rounds::Unlimited);

        let mut ordinary = StoreState {
            inhibited: false,
            rounds: Rounds::Finite(10),
        };
        assert_eq!(
            debit_ammunition(&mut ordinary, 4),
            Ok(DebitOutcome::Debited {
                remaining: 6,
                partial: false
            })
        );
        assert_eq!(
            debit_ammunition(&mut ordinary, 4),
            Ok(DebitOutcome::Debited {
                remaining: 2,
                partial: false
            })
        );
        assert_eq!(
            debit_ammunition(&mut ordinary, 4),
            Ok(DebitOutcome::Debited {
                remaining: 0,
                partial: true
            })
        );
        assert_eq!(
            debit_ammunition(&mut ordinary, 4),
            Ok(DebitOutcome::Refused(DebitRefusal::Empty))
        );
        assert!(debit_ammunition(&mut ordinary, 0).is_err());
    }

    #[test]
    fn release_reports_three_facts_and_orders_debit_before_allocation() {
        let request = ReleaseRequest {
            debit: 1,
            projectile_count: 2,
            allocation_succeeds: false,
            target: TargetRetention::ClearedTerrainBlocked,
            human_controlled: false,
            global_unlimited: true,
            atomic_release: false,
        };
        let mut store = StoreState {
            inhibited: false,
            rounds: Rounds::Finite(3),
        };
        // Original order: the debit happens even though allocation fails.
        assert_eq!(
            release(&mut store, &request).unwrap(),
            ReleaseReport {
                inventory: InventoryChange::Debit(DebitOutcome::Debited {
                    remaining: 2,
                    partial: false
                }),
                projectiles: ProjectileCreation::None,
                target: TargetRetention::ClearedTerrainBlocked,
            }
        );
        // Opinionated atomic policy: nothing debited on allocation failure.
        let atomic = ReleaseRequest {
            atomic_release: true,
            ..request
        };
        assert_eq!(
            release(&mut store, &atomic).unwrap().inventory,
            InventoryChange::NotDebited
        );
        assert_eq!(store.rounds, Rounds::Finite(2));
        // Global unlimited needs human control; AI is not exempt.
        let human = ReleaseRequest {
            human_controlled: true,
            allocation_succeeds: true,
            ..request
        };
        assert_eq!(
            release(&mut store, &human).unwrap(),
            ReleaseReport {
                inventory: InventoryChange::GlobalUnlimitedBypass,
                projectiles: ProjectileCreation::Created { count: 2 },
                target: TargetRetention::ClearedTerrainBlocked,
            }
        );
        assert_eq!(store.rounds, Rounds::Finite(2));
    }

    fn profile() -> SeekerEnvelope {
        SeekerEnvelope {
            range_checking: true,
            min_range_ft: Some(1000.0),
            max_range_ft: Some(30000.0),
            min_relative_altitude_ft: Some(-5000.0),
            max_relative_altitude_ft: Some(5000.0),
            horizontal_limit_deg: Some(30.0),
            vertical_limit_deg: Some(20.0),
        }
    }
    fn geometry(range_ft: f64) -> SeekerGeometry {
        SeekerGeometry {
            range_ft,
            relative_altitude_ft: 0.0,
            horizontal_error_deg: 0.0,
            vertical_error_deg: 0.0,
            signature_percent: 100,
        }
    }

    #[test]
    fn envelope_range_and_vertical_limits_are_inclusive() {
        let p = profile();
        assert_eq!(
            envelope_check(&p, &geometry(1000.0)),
            EnvelopeResult::Eligible
        );
        assert_eq!(
            envelope_check(&p, &geometry(999.9)),
            EnvelopeResult::Rejected(EnvelopeRejection::BelowMinimumRange)
        );
        assert_eq!(
            envelope_check(&p, &geometry(30000.0)),
            EnvelopeResult::Eligible
        );
        assert_eq!(
            envelope_check(&p, &geometry(30000.1)),
            EnvelopeResult::Rejected(EnvelopeRejection::AboveMaximumRange)
        );
        let high = SeekerGeometry {
            relative_altitude_ft: 5000.0,
            ..geometry(2000.0)
        };
        assert_eq!(envelope_check(&p, &high), EnvelopeResult::Eligible);
        let too_high = SeekerGeometry {
            relative_altitude_ft: 5000.1,
            ..geometry(2000.0)
        };
        assert_eq!(
            envelope_check(&p, &too_high),
            EnvelopeResult::Rejected(EnvelopeRejection::AboveVerticalLimit)
        );
        let too_low = SeekerGeometry {
            relative_altitude_ft: -5000.1,
            ..geometry(2000.0)
        };
        assert_eq!(
            envelope_check(&p, &too_low),
            EnvelopeResult::Rejected(EnvelopeRejection::BelowVerticalLimit)
        );
    }

    #[test]
    fn envelope_signature_scales_effective_range() {
        let p = profile();
        // 20000 ft at 50% signature is 40000 ft effective: outside 30000.
        let dim = SeekerGeometry {
            signature_percent: 50,
            ..geometry(20000.0)
        };
        assert_eq!(
            envelope_check(&p, &dim),
            EnvelopeResult::Rejected(EnvelopeRejection::SignatureAdjustedRangeExceedsMaximum)
        );
        // 15000 ft at 50% is exactly 30000: inclusive.
        let edge = SeekerGeometry {
            signature_percent: 50,
            ..geometry(15000.0)
        };
        assert_eq!(envelope_check(&p, &edge), EnvelopeResult::Eligible);
        let zero = SeekerGeometry {
            signature_percent: 0,
            ..geometry(2000.0)
        };
        assert_eq!(
            envelope_check(&p, &zero),
            EnvelopeResult::Rejected(EnvelopeRejection::ZeroSignature)
        );
        let unbounded = SeekerEnvelope {
            max_range_ft: None,
            ..p
        };
        assert_eq!(envelope_check(&unbounded, &zero), EnvelopeResult::Eligible);
        let unchecked = SeekerEnvelope {
            range_checking: false,
            ..p
        };
        assert_eq!(envelope_check(&unchecked, &zero), EnvelopeResult::Eligible);
    }

    #[test]
    fn envelope_angles_forward_hemisphere_and_beyond_ninety() {
        let p = profile();
        let at = |h: f64, v: f64| SeekerGeometry {
            horizontal_error_deg: h,
            vertical_error_deg: v,
            ..geometry(2000.0)
        };
        assert_eq!(
            envelope_check(&p, &at(30.0, 20.0)),
            EnvelopeResult::Eligible
        );
        assert_eq!(
            envelope_check(&p, &at(-30.0, -20.0)),
            EnvelopeResult::Eligible
        );
        assert_eq!(
            envelope_check(&p, &at(30.1, 0.0)),
            EnvelopeResult::Rejected(EnvelopeRejection::OutsideAngularLimits)
        );
        assert_eq!(
            envelope_check(&p, &at(0.0, 20.1)),
            EnvelopeResult::Rejected(EnvelopeRejection::OutsideAngularLimits)
        );
        // Beyond 90 vertical: horizontal within max(30, 90) = 90 passes.
        assert_eq!(
            envelope_check(&p, &at(90.0, 100.0)),
            EnvelopeResult::Eligible
        );
        // Or vertical at least 180 - 20 = 160.
        assert_eq!(
            envelope_check(&p, &at(120.0, 160.0)),
            EnvelopeResult::Eligible
        );
        assert_eq!(
            envelope_check(&p, &at(120.0, 159.9)),
            EnvelopeResult::Rejected(EnvelopeRejection::OutsideAngularLimits)
        );
        // Both sentinels bypass angles, not range.
        let open = SeekerEnvelope {
            horizontal_limit_deg: None,
            vertical_limit_deg: None,
            ..p
        };
        assert_eq!(
            envelope_check(&open, &at(170.0, 45.0)),
            EnvelopeResult::Eligible
        );
        assert_eq!(
            envelope_check(&open, &SeekerGeometry { ..geometry(500.0) }),
            EnvelopeResult::Rejected(EnvelopeRejection::BelowMinimumRange)
        );
    }

    #[test]
    fn device_schedule_rerequests_each_quarter_and_postpones_service() {
        assert_eq!(
            DeviceReleaseSchedule::begin(true, 1, 0),
            Err(AiError::InvalidInput("device release count must be 2 or 3"))
        );
        assert_eq!(DeviceReleaseSchedule::begin(false, 2, 0).unwrap(), None);
        let mut random = DecisionRandom::seeded(ungated_seed(2));
        let mut service = service(AircraftId::F18);
        service.advance(0, &inputs(), &mut random).unwrap();
        assert_eq!(service.deadline(), Some(20));

        let tick = 10 * Q;
        let mut schedule = DeviceReleaseSchedule::begin(true, 3, tick)
            .unwrap()
            .unwrap();
        assert!(schedule.due(tick));
        assert_eq!(
            schedule
                .record(DeviceLaunch::Succeeded, tick, &mut service)
                .unwrap(),
            DeviceScheduleOutcome::Rescheduled { next_due: 11 }
        );
        assert_eq!(service.deadline(), Some(28));
        assert!(!schedule.due(11 * Q - 1));
        assert!(schedule.due(11 * Q));
        assert!(
            schedule
                .record(DeviceLaunch::Succeeded, 10 * Q, &mut service)
                .is_err()
        );
        assert_eq!(
            schedule
                .record(DeviceLaunch::Succeeded, 11 * Q, &mut service)
                .unwrap(),
            DeviceScheduleOutcome::Rescheduled { next_due: 12 }
        );
        assert_eq!(service.deadline(), Some(36));
        assert_eq!(
            schedule
                .record(DeviceLaunch::Succeeded, 12 * Q, &mut service)
                .unwrap(),
            DeviceScheduleOutcome::Completed
        );
        assert_eq!(schedule.next_due(), None);
        // Every successful reaction postpones, the final one included.
        assert_eq!(service.deadline(), Some(44));

        let mut failing = DeviceReleaseSchedule::begin(true, 2, 0).unwrap().unwrap();
        assert_eq!(
            failing
                .record(DeviceLaunch::Failed, 0, &mut service)
                .unwrap(),
            DeviceScheduleOutcome::Disabled
        );
        assert!(!failing.due(100 * Q));
        assert_eq!(service.deadline(), Some(44));
    }

    #[test]
    fn postponement_needs_a_finite_deadline() {
        let mut service = service(AircraftId::F18);
        assert_eq!(service.postpone_for_device_reaction(), None);
    }

    // B45: envelopes, signature, eligibility, store choice, in-flight support

    #[test]
    fn store_envelopes_check_the_named_role() {
        let acquisition = SeekerEnvelope {
            max_range_ft: Some(60000.0),
            ..profile()
        };
        let envelopes = StoreEnvelopes {
            acquisition,
            employment: profile(),
        };
        assert_eq!(envelopes.envelope(EnvelopeRole::Acquisition), &acquisition);
        assert_eq!(envelopes.envelope(EnvelopeRole::Employment), &profile());
        let g = geometry(45000.0);
        assert_eq!(
            envelopes.check(EnvelopeRole::Acquisition, &g),
            EnvelopeResult::Eligible
        );
        assert_eq!(
            envelopes.check(EnvelopeRole::Employment, &g),
            EnvelopeResult::Rejected(EnvelopeRejection::AboveMaximumRange)
        );
    }

    #[test]
    fn detection_range_scales_with_signature_and_caps_at_full() {
        assert_eq!(detection_range_feet(Some(20000.0), 0), Some(0.0));
        assert_eq!(detection_range_feet(Some(20000.0), 50), Some(10000.0));
        assert_eq!(detection_range_feet(Some(20000.0), 100), Some(20000.0));
        assert_eq!(detection_range_feet(Some(20000.0), 150), Some(20000.0));
        assert_eq!(detection_range_feet(None, 50), None);
    }

    #[test]
    fn rear_cone_and_vertical_boundaries_are_inclusive() {
        assert!(in_rear_cone(40.0, 0.0));
        assert!(!in_rear_cone(41.0, 0.0));
        assert!(in_rear_cone(-40.0, 140.0));
        assert!(!in_rear_cone(0.0, 141.0));
        assert!(in_rear_cone(0.0, -140.0));
        assert!(pointing_vertical(60.0));
        assert!(pointing_vertical(-60.0));
        assert!(pointing_vertical(90.0));
        assert!(!pointing_vertical(59.0));
        assert!(!pointing_vertical(0.0));
    }

    #[test]
    fn aspect_penalty_blocks_from_the_front_only() {
        assert!(aspect_penalty_applies(false, false));
        assert!(!aspect_penalty_applies(true, false));
        assert!(!aspect_penalty_applies(false, true));
        // The AA-2's 100 percent penalty: nothing from the front, full from behind.
        let front = aspect_penalty_applies(in_rear_cone(0.0, 180.0), pointing_vertical(0.0));
        assert_eq!(apply_aspect_penalty(100, 100, front), 0);
        let rear = aspect_penalty_applies(in_rear_cone(10.0, 20.0), pointing_vertical(0.0));
        assert_eq!(apply_aspect_penalty(100, 100, rear), 100);
        // Sidewinder 30 from the front, saturating below zero.
        assert_eq!(apply_aspect_penalty(80, 30, true), 50);
        assert_eq!(apply_aspect_penalty(20, 30, true), 0);
        // Vertical bypass at 60 degrees, not 61 degrees short of it.
        assert_eq!(
            apply_aspect_penalty(
                100,
                100,
                aspect_penalty_applies(false, pointing_vertical(60.0))
            ),
            100
        );
        assert_eq!(
            apply_aspect_penalty(
                100,
                100,
                aspect_penalty_applies(false, pointing_vertical(59.0))
            ),
            0
        );
    }

    #[test]
    fn infrared_boost_doubles_with_a_floor_of_200() {
        assert_eq!(infrared_boost(50, true), 200);
        assert_eq!(infrared_boost(100, true), 200);
        assert_eq!(infrared_boost(150, true), 300);
        assert_eq!(infrared_boost(50, false), 50);
    }

    #[test]
    fn radar_bonuses_add_and_floor_at_100() {
        assert_eq!(radar_configuration_bonus(10, true, false), 100);
        assert_eq!(radar_configuration_bonus(10, false, true), 100);
        assert_eq!(radar_configuration_bonus(10, true, true), 125);
        assert_eq!(radar_configuration_bonus(80, true, false), 113);
        assert_eq!(radar_configuration_bonus(80, true, true), 138);
        assert_eq!(radar_configuration_bonus(10, false, false), 10);
    }

    #[test]
    fn emitter_signature_is_all_or_nothing() {
        assert_eq!(emitter_signature(true), 100);
        assert_eq!(emitter_signature(false), 0);
    }

    #[test]
    fn night_divisor_rises_from_1500_to_4500_ft() {
        assert_eq!(night_visual_divisor(0.0), 1.0);
        assert_eq!(night_visual_divisor(1500.0), 1.0);
        assert_eq!(night_visual_divisor(3000.0), 3.0);
        assert_eq!(night_visual_divisor(4500.0), 5.0);
        assert_eq!(night_visual_divisor(9000.0), 5.0);
    }

    #[test]
    fn visual_floor_lifts_close_targets_and_blends_back() {
        assert_eq!(visual_close_range_floor(0.0, 25), 75);
        assert_eq!(visual_close_range_floor(200.0, 25), 75);
        assert_eq!(visual_close_range_floor(850.0, 25), 50);
        assert_eq!(visual_close_range_floor(1500.0, 25), 25);
        assert_eq!(visual_close_range_floor(3000.0, 25), 25);
        // Clear weather is never lowered by the floor.
        assert_eq!(visual_close_range_floor(100.0, 100), 100);
        assert_eq!(visual_close_range_floor(850.0, 100), 100);
    }

    #[test]
    fn look_down_rejection_by_angle_and_height() {
        assert_eq!(look_down_rejection(45.0, 5000.0), 1.0);
        assert_eq!(look_down_rejection(90.0, 5000.0), 1.0);
        assert_eq!(look_down_rejection(22.5, 5000.0), 0.5);
        assert_eq!(look_down_rejection(0.0, 0.0), 1.0);
        assert_eq!(look_down_rejection(0.0, 2500.0), 0.5);
        assert_eq!(look_down_rejection(0.0, 5000.0), 0.0);
        assert_eq!(look_down_rejection(0.0, 10000.0), 0.0);
        assert_eq!(look_down_rejection(-30.0, 10000.0), 0.0);
        assert_eq!(look_down_rejection(22.5, 0.0), 1.0);
    }

    #[test]
    fn eligibility_matrix() {
        use StoreCapability as C;
        use TargetClass as T;
        assert!(store_eligible(C::AIR_TO_AIR_MISSILE, T::Air));
        assert!(!store_eligible(C::AIR_TO_AIR_MISSILE, T::Surface));
        assert!(!store_eligible(C::SURFACE_STORE, T::Air));
        assert!(store_eligible(C::SURFACE_STORE, T::Surface));
        assert!(store_eligible(C::GUN, T::Air));
        assert!(store_eligible(C::GUN, T::Surface));
    }

    fn candidate(station: u8) -> StoreCandidate {
        StoreCandidate {
            station: StationId(station),
            guided: true,
            employment_fit: Some(10.0),
            hit_chance: 0.0,
            range_feet: 1000.0,
            damage_vs_category: 0.0,
        }
    }

    #[test]
    fn store_score_terms() {
        let guided = candidate(1);
        assert_eq!(store_score(&guided), 90.0);
        let unguided = StoreCandidate {
            guided: false,
            ..guided
        };
        assert_eq!(store_score(&unguided), 80.0);
        let failed = StoreCandidate {
            employment_fit: None,
            hit_chance: 7.0,
            ..guided
        };
        assert_eq!(store_score(&failed), 7.0);
        let at_limit = StoreCandidate {
            range_feet: 1500.0,
            ..guided
        };
        assert_eq!(store_score(&at_limit), 90.0);
        let beyond = StoreCandidate {
            range_feet: 1501.0,
            ..guided
        };
        assert_eq!(store_score(&beyond), 140.0);
        let unguided_beyond = StoreCandidate {
            guided: false,
            range_feet: 1501.0,
            ..guided
        };
        assert_eq!(store_score(&unguided_beyond), 80.0);
        let damage = StoreCandidate {
            damage_vs_category: 250.0,
            ..guided
        };
        assert_eq!(store_score(&damage), 100.0);
    }

    #[test]
    fn select_store_takes_the_highest_and_the_first_on_a_tie() {
        let low = StoreCandidate {
            employment_fit: Some(40.0),
            ..candidate(1)
        };
        let high = StoreCandidate {
            range_feet: 2000.0,
            ..candidate(2)
        };
        assert_eq!(select_store(&[low, high, candidate(3)]), Ok(Some(1)));
        assert_eq!(select_store(&[candidate(1), candidate(2)]), Ok(Some(0)));
        assert_eq!(select_store(&[]), Ok(None));
        let ten = vec![candidate(1); STORE_CANDIDATE_LIMIT];
        assert_eq!(select_store(&ten), Ok(Some(0)));
        let eleven = vec![candidate(1); STORE_CANDIDATE_LIMIT + 1];
        assert!(select_store(&eleven).is_err());
    }

    #[test]
    fn in_flight_track_check_cases() {
        let ai = SupportRequirement {
            requires_launcher: true,
            launcher_alive: true,
            launcher_emitting: false,
            launcher_human: false,
            human_launcher_holds_target_in_radar: false,
        };
        assert_eq!(in_flight_track_check(true, &ai), TrackOutcome::Tracking);
        assert_eq!(in_flight_track_check(false, &ai), TrackOutcome::TargetLost);
        let dead = SupportRequirement {
            launcher_alive: false,
            ..ai
        };
        assert_eq!(in_flight_track_check(true, &dead), TrackOutcome::TargetLost);
        let unsupported = SupportRequirement {
            requires_launcher: false,
            launcher_alive: false,
            ..ai
        };
        assert_eq!(
            in_flight_track_check(true, &unsupported),
            TrackOutcome::Tracking
        );
        let human = SupportRequirement {
            launcher_human: true,
            launcher_emitting: true,
            human_launcher_holds_target_in_radar: true,
            ..ai
        };
        assert_eq!(in_flight_track_check(true, &human), TrackOutcome::Tracking);
        let human_off = SupportRequirement {
            launcher_emitting: false,
            ..human
        };
        assert_eq!(
            in_flight_track_check(true, &human_off),
            TrackOutcome::TargetLost
        );
        let human_dropped = SupportRequirement {
            human_launcher_holds_target_in_radar: false,
            ..human
        };
        assert_eq!(
            in_flight_track_check(true, &human_dropped),
            TrackOutcome::TargetLost
        );
    }

    #[test]
    fn ai_launcher_support_extends_the_deadline_ten_seconds() {
        assert_eq!(ai_launcher_support(true, 100, 0), Some(140));
        assert_eq!(ai_launcher_support(true, 100, 130), Some(140));
        assert_eq!(ai_launcher_support(true, 100, 200), Some(200));
        assert_eq!(ai_launcher_support(false, 100, 200), None);
    }

    #[test]
    fn passive_emitter_keeps_stationary_shutdown_targets() {
        let t = PASSIVE_EMITTER_STATIONARY_FPS;
        assert!(passive_emitter_keeps_track(true, ScalarSpeed(500.0), t));
        assert!(passive_emitter_keeps_track(false, ScalarSpeed(0.0), t));
        assert!(passive_emitter_keeps_track(false, ScalarSpeed(35.9), t));
        assert!(!passive_emitter_keeps_track(false, ScalarSpeed(36.0), t));
        assert!(!passive_emitter_keeps_track(false, ScalarSpeed(500.0), t));
    }
}
