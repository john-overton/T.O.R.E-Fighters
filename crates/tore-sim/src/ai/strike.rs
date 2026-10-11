//! The strike tactics (docs/spec/ai-strike.md, slice ST-T): what a striker
//! does with its ground picture, its stores and the shared release.
//!
//! One [`Strike`] per striker (an aircraft with the Strike role or an ordered
//! ground target, [`super::awareness::ground::GroundPicture::active`]). Each
//! mission step it reads the picture, the perceived threat, its own energy
//! and the solution the host worked out from the strike cockpit (the bomb or
//! rocket cue, the guided weapon's reach, a seeker's lock), and answers with
//! an [`Order`]: where to fly (a heading, a flight-path pitch and a speed for
//! the ordinary input-only steering adapter, so the aircraft flies the pass
//! through its own controls), which store to release now, and what it is
//! doing. Nothing here moves the aircraft or makes a round.
//!
//! The flow, all deterministic (John, 2026-10-11, decisions 3 and 13):
//!
//! - **Ingress** toward the target; at 10 nm (or at once when closer) the
//!   striker **chooses**: the target ([`super::targeting::select_strike_target`],
//!   SEAD radars first for an anti-radiation carrier), the store
//!   ([`choose_store`]) and the pass ([`choose_pass`]) from the perceived
//!   threat and the available speed, limited by the bombing class and the
//!   store. A pass draws its aim offset ([`aim_draw`], unguided weapons only).
//! - **Attack**: the pass's approach (retail steering frame: a point offset
//!   above the target, flown straight at), then the final: the cue is flown
//!   onto the aim point and the store is released when the shared release
//!   test passes.
//! - **Exit**: jink inside 10,000 ft, climb out, turn back at
//!   max(30,000 ft, 2R) and choose again: re-attack while the target stands
//!   and a usable store is left, else the next target, else **home**.
//! - **Defend**: a missile or gun defence the existing B47 reactions fly, or
//!   a fighter that threatens the striker, hands control back to the air
//!   logic; once the threat has gone the striker resumes at ingress if it
//!   is safe (the spec's safe test), else egresses and goes home unless it
//!   is safe by the exit distance.
//!
//! Helicopters and the gunship keep their existing behaviour (slice ST-F):
//! [`Strike::flies`] says no for them.
use super::Experience;
use super::awareness::ground::{BriefedKind, GroundPicture, RecentWarnings, ThreatLevel};
use super::controller::{Activity, OwnState};
use super::motion::SpeedRequest;
use super::targeting::{self, ObjectId, Side, StrikeSource};
use super::weapon_service::StationId;
use crate::attitude::Vector;
use crate::combat::delivery::DeliveryMode;
use crate::models::strike_class::StrikeClass;
use crate::sensors::FEET_PER_NAUTICAL_MILE;

/// Feet in a metre, for John's release circle (given in metres).
pub const FEET_PER_METRE: f64 = 1. / 0.3048;
/// The pass is chosen this far from the target, and again before each
/// re-attack (spec, "Pass choice": 10 nm, fitted).
pub const CHOICE_DISTANCE_FT: f64 = 10. * FEET_PER_NAUTICAL_MILE;
/// Straight run: the run ends this close, horizontally (retail).
pub const STRAIGHT_EXIT_FT: f64 = 2_000.;
/// Pop-up: the approach point's height over the target, the pull's start,
/// its angle over the line of sight and its length, and where the run ends
/// (retail; the pull holds corner speed, not retail's minimum: spec,
/// "Retail quirks", opinionated agent).
pub const POPUP_OFFSET_FT: f64 = 500.;
pub const POPUP_PULL_AT_FT: f64 = 10_000.;
pub const POPUP_PULL_DEG: f64 = 25.;
pub const POPUP_PULL_TICKS: u64 = 4 * 120;
pub const POPUP_EXIT_FT: f64 = 1_500.;
/// The pull goes on past its 4 s until the target is this far below the
/// horizon (fitted, agent): the AI's terrain floor (6 s of travel ahead)
/// allows only a shallow dive from a low pop, too shallow for rockets.
pub const POPUP_DIVE_DEG: f64 = 15.;
/// Rockets pop from farther out (fitted, agent): they fly along the nose, so
/// the dive after the pop must point at the target, which from a low pop the
/// terrain floor does not allow; popping at 16,000 ft gives a long shallow
/// dive instead.
pub const ROCKET_POPUP_PULL_AT_FT: f64 = 16_000.;
/// The pop-up's approach heads for its pull point until this much short of
/// it (fitted).
pub const POPUP_LEAD_FT: f64 = 2_000.;
/// Fast-high: the approach point's height over the target and where the
/// run ends (retail).
pub const FAST_HIGH_OFFSET_FT: f64 = 5_000.;
pub const FAST_HIGH_EXIT_FT: f64 = 2_000.;
/// Dive bomb: the gate's floor, the pull-out floor, the climb after it
/// (retail).
pub const DIVE_GATE_MIN_FT: f64 = 10_000.;
pub const DIVE_PULL_OUT_MIN_FT: f64 = 2_000.;
pub const DIVE_CLIMB_DEG: f64 = 25.;
pub const DIVE_CLIMB_TICKS: u64 = 10 * 120;
/// The steepest dive any fighter or attack jet flies (retail `maxDive`).
pub const MAX_DIVE_DEG: f64 = 80.;
/// The dive starts once the aim point is this far below the horizon
/// (fitted, agent). Retail rolls in at R from a height of 2R, the target 63
/// degrees down, and pulls inverted to 80 degrees. T.O.R.E's input-only
/// adapter cannot roll inverted: it noses over wings level, more slowly, and
/// the AI's terrain floor (6 s of travel ahead) holds the dive to about 50
/// degrees at 5,000 ft; so it starts at 30 degrees and the cue flying takes
/// it as steep as the floor allows.
pub const DIVE_START_DEG: f64 = 30.;
/// The approach heads for the roll-in point until this much short of it
/// (fitted).
pub const DIVE_ROLLIN_LEAD_FT: f64 = 2_000.;
/// The cue's lateral steering asks for this many times the bearing between
/// the cue and the aim, so the input-only adapter, which turns at a rate
/// proportional to the heading it is asked for, closes it faster (fitted).
pub const CUE_HEADING_GAIN: f64 = 1.;
/// Flying the cue (fitted, agent): beyond this far short of the aim the pass
/// flies its own line; the cue is flown to a point this margin short of the
/// aim with this gain on the angle; within the margin (and this tolerance)
/// the flight path is held and the cue creeps across.
pub const CUE_ACQUIRE_FT: f64 = 3_000.;
pub const CUE_MARGIN_FT: f64 = 600.;
pub const CUE_HOLD_TOLERANCE_FT: f64 = 300.;
pub const CUE_PITCH_GAIN: f64 = 3.;
/// The input-only adapter closes a pitch error over 3 s (docs/spec/ai.md,
/// "Input-only aircraft control"); a request that would ask for less than
/// [`PUSH_G`] holds the wings level (fitted, agent).
pub const ADAPTER_PITCH_S: f64 = 3.;
pub const PUSH_G: f64 = 0.3;
/// Exit: the jink while inside this horizontal distance, its legs, their
/// length, the heading step and the climb (retail: 10 to 14 degrees; the
/// middle, 12, fitted); the climb beyond it; the turn back (retail).
pub const JINK_INSIDE_FT: f64 = 10_000.;
pub const JINK_LEGS: u64 = 4;
pub const JINK_LEG_TICKS: u64 = 3 * 120;
pub const JINK_STEP_DEG: f64 = 10.;
pub const JINK_CLIMB_DEG: f64 = 12.;
pub const EXIT_CLIMB_DEG: f64 = 10.;
pub const EXIT_TURN_MIN_FT: f64 = 30_000.;
/// A heavy's exit (retail: toward 50,000 ft away).
pub const HEAVY_EXIT_FT: f64 = 50_000.;
/// The exit climb levels off this far above the altitude the strike was
/// tasked at (fitted, agent): retail climbs on, which would lift every
/// re-attack 5,000 ft higher than the last.
pub const EXIT_CLIMB_ABOVE_FT: f64 = 2_000.;
/// The turn back is over once the nose is within this of the target
/// (fitted).
pub const TURN_BACK_DEG: f64 = 15.;
/// Release floors above the ground for dive and level releases (spec:
/// 3,000 ft under threat Clear or Air, 5,000 ft under Guns or Missiles).
pub const FLOOR_CLEAR_FT: f64 = 3_000.;
pub const FLOOR_DEFENDED_FT: f64 = 5_000.;
/// A heavy's release floor: max(5,000 ft AGL, the mission's airborne
/// height), raised by threat, at most 90% of its ceiling (spec, "Heavy
/// bombers").
pub const HEAVY_MIN_FT: f64 = 5_000.;
pub const HEAVY_RAISE_GUNS_FT: f64 = 5_000.;
pub const HEAVY_RAISE_MISSILES_FT: f64 = 10_000.;
pub const HEAVY_CEILING_SHARE: f64 = 0.9;
/// John's release circle (2026-10-11): 90% of passes inside it.
pub const IN_CIRCLE_SHARE: f64 = 0.9;
/// The AI releases once the cue is within this share of its circle of the
/// drawn aim point (spec, fitted).
pub const GATE_SHARE: f64 = 0.05;
/// Warnings count this long (spec: 10 s).
pub const WARNING_TICKS: u64 = 10 * 120;
/// The defend test: an aircraft inside 5 nm facing the striker (spec,
/// fitted), "facing" meaning within this of its nose (fitted, agent).
pub const DEFEND_RANGE_FT: f64 = 5. * FEET_PER_NAUTICAL_MILE;
pub const DEFEND_FACING_DEG: f64 = 45.;
/// The safe test (spec): no hostile fighter inside 15 nm.
pub const SAFE_FIGHTER_FT: f64 = 15. * FEET_PER_NAUTICAL_MILE;
/// Retail fires a guided round only inside 10,000 ft or half its zone-1
/// maximum range, whichever is farther.
pub const GUIDED_CLOSE_FT: f64 = 10_000.;
/// Rockets: at most this many from one pass, fired at the record's cadence
/// while the cue stays inside the circle (fitted, agent).
pub const ROCKETS_PER_PASS: u32 = 8;
/// Guided weapons and seekers: the run ends this close (fitted, the
/// straight run's).
pub const GUIDED_EXIT_FT: f64 = 2_000.;
/// While every briefed target is fully attacked the striker re-chooses this
/// often (fitted).
pub const WAIT_TICKS: u64 = 2 * 120;
/// Class word bits of a SAM and an AAA unit (retail type `+0x0d`).
pub const CLASS_SAM: u16 = 0x1000;
pub const CLASS_AAA: u16 = 0x800;

/// A pass (spec, "Pass choice").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pass {
    Straight,
    PopUp,
    FastHigh,
    Dive,
    Level,
}

impl Pass {
    pub const ALL: [Self; 5] = [
        Self::Straight,
        Self::PopUp,
        Self::FastHigh,
        Self::Dive,
        Self::Level,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Straight => "straight",
            Self::PopUp => "popup",
            Self::FastHigh => "fasthigh",
            Self::Dive => "dive",
            Self::Level => "level",
        }
    }
}

/// What a store is to the strike (spec, "Weapons and releases").
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoreKind {
    /// An unguided bomb or a pack.
    Bomb,
    /// A rocket pod.
    Rocket,
    /// A laser bomb or missile.
    Laser,
    /// A JDAM.
    Coordinate,
    /// The AT-2: the helicopters' (slice ST-F), never chosen here.
    Command,
    /// A missile with its own seeker: Mavericks, AS-14, AS-30.
    Seeker,
    /// A radar air-to-ground missile (AS-7, AS-15, AGM-84A, AM-39, AS-16).
    RadarAgm,
    /// HARM and the other anti-radiation missiles.
    AntiRadiation,
}

impl StoreKind {
    /// Unguided: the skill circle applies (John, 2026-10-11).
    pub fn unguided(self) -> bool {
        matches!(self, Self::Bomb | Self::Rocket)
    }
    /// Released on a seeker's lock through the AI's own missile release.
    pub fn seeker(self) -> bool {
        matches!(self, Self::Seeker | Self::RadarAgm | Self::AntiRadiation)
    }
    /// Flown to on a level run (spec, "Other weapons").
    pub fn level_run(self) -> bool {
        matches!(self, Self::Laser | Self::Coordinate)
    }
}

/// One store as the striker sees it: the host describes its kind, the
/// mission counts its rounds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Store {
    pub station: StationId,
    pub kind: StoreKind,
    pub rounds: u32,
    /// Ticks between two rounds of a burst (rockets), at least one quarter
    /// second.
    pub cadence_ticks: u64,
    /// The farthest the AI fires it (a seeker: retail's guided rule), feet.
    pub fire_range_ft: f64,
    /// Its zone-1 minimum and maximum ranges, feet: an unguided round is
    /// released only with its cue inside them (retail zone 1).
    pub minimum_range_ft: f64,
    pub maximum_range_ft: f64,
    /// A rocket's speed for its cue's drop, ft/s; `None` for a bomb, which
    /// leaves at the aircraft's speed.
    pub round_speed_fps: Option<f64>,
}

/// What the host worked out for the striker's selection this tick, from its
/// strike cockpit: the same solutions a human's HUD shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solution {
    pub station: StationId,
    pub target: u32,
    /// Where the target stands, and whether it still stands.
    pub target_position: Vector,
    pub target_velocity: Vector,
    pub target_alive: bool,
    /// The cue's predicted impact (bomb CCIP, which the LVL cue is timed on,
    /// or the rocket's), for unguided stores.
    pub impact: Option<Vector>,
    /// Seconds from release to that impact.
    pub impact_seconds: Option<f64>,
    /// The shared release test passes now: the cue within the gate of the
    /// aim point (unguided), the guided reach, or a seeker's lock.
    pub due: bool,
    /// The sight warns LOW or FRAG: the AI does not pickle (fitted, agent).
    pub warning: bool,
    /// One of this aircraft's guided rounds is flying at the target (retail
    /// fires one guided round at a time).
    pub guided_in_flight: bool,
}

/// The host's per-step description (see the module notes).
#[derive(Clone, Debug, PartialEq)]
pub struct Inputs {
    pub class: StrikeClass,
    pub stores: Vec<Store>,
    /// A laser designator of its own: a spot is assured, so laser stores
    /// may be chosen (spec, decision 9).
    pub designator: bool,
    /// The mission's airborne height above the ground, for a heavy's floor
    /// (Quick Mission's altitude setting); `None` when unknown.
    pub mission_height_ft: Option<f64>,
    pub solution: Option<Solution>,
}

/// Where the striker flies this tick: a heading and a flight-path pitch,
/// degrees, and a speed (the ordinary adapter flies it with its controls).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Steer {
    pub heading_deg: f64,
    pub pitch_deg: f64,
    pub speed: SpeedRequest,
    pub afterburner: bool,
}

/// A store to release now, at the striker's target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Release {
    pub station: StationId,
    pub target: u32,
}

/// One step's answer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Order {
    /// `None` hands the flying to the rest of the controller (defend, home).
    pub steer: Option<Steer>,
    pub release: Option<Release>,
    pub activity: Option<Activity>,
}

/// What the strike cockpit is to hold for the host: the target, the store,
/// the bomb sight's mode, the aim and whether the laser is wanted.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Selection {
    pub target: u32,
    pub station: StationId,
    pub kind: StoreKind,
    pub mode: DeliveryMode,
    /// The aim offset from the target, feet along x and z, and the release
    /// gate, feet (zero offset for a guided store).
    pub aim_ft: [f64; 2],
    pub gate_ft: f64,
    pub laser: bool,
}

/// The step's world, as the mission hands it over.
pub struct Context<'a> {
    pub tick: u64,
    pub aircraft: u32,
    pub side: Side,
    pub own: &'a OwnState,
    pub velocity: Vector,
    pub picture: &'a GroundPicture,
    pub inputs: &'a Inputs,
    pub experience: Experience,
    pub warnings: RecentWarnings,
    /// The controller flies a B47 defence this step (a missile or gunfire).
    pub defending: bool,
    /// Fuel, damage or the mission sends it home.
    pub recovering: bool,
    /// The strike targets of the other members of its wing.
    pub mates: &'a [u32],
    /// The aircraft's ceiling, feet above sea level.
    pub ceiling_ft: f64,
}

/// The strike's phase.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// Toward the target until the choice point.
    Ingress,
    /// Too slow under threat: level at maximum speed until energy is Good.
    Regain,
    /// Flying a pass.
    Attack,
    /// Away from the target after a pass.
    Exit,
    /// Back toward the target to choose again.
    TurnBack,
    /// Every briefed target is fully attacked: wait for one.
    Wait,
    /// The air logic flies a defence.
    Defend,
    /// Done: no target or no store left, or sent home.
    Home,
}

/// A pass's stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Segment {
    /// Toward the pass's approach point.
    Approach,
    /// The pop-up's pull.
    Pull,
    /// The cue flown onto the aim point; the release.
    Final,
    /// The dive's pull-out climb.
    Climb,
}

/// One striker's strike state (see the module notes).
#[derive(Clone, Debug, PartialEq)]
pub struct Strike {
    /// The aircraft's decision seed, which the mission sets from its roster
    /// place: the skill draw's "mission seed".
    seed: u64,
    phase: Phase,
    segment: Segment,
    /// The tick the phase or segment began.
    since: u64,
    pass: Option<Pass>,
    /// The driver's forced pass and store (coverage only).
    forced: Option<Pass>,
    forced_station: Option<StationId>,
    target: Option<u32>,
    station: Option<StationId>,
    kind: Option<StoreKind>,
    /// Passes flown at the current target, from 1.
    pass_number: u32,
    aim_ft: [f64; 2],
    /// The lead on a moving target: its ground velocity times the cue's
    /// time of fall, feet along x and z (fitted, agent).
    lead_ft: [f64; 2],
    gate_ft: f64,
    /// The drawn offset lies inside the circle.
    inside: bool,
    /// The final holds this flight-path pitch, degrees, while the cue
    /// creeps across the aim; `None` while it is still flying the cue.
    holding: Option<f64>,
    /// The altitude a level run or a hold flies at, feet above sea level.
    hold_ft: Option<f64>,
    /// Rounds released on this pass and the tick the next may go.
    released: u32,
    next_release: u64,
    /// The heading the exit holds and the jink's side (+1 right, -1 left).
    exit_heading_deg: f64,
    jink_side: f64,
    /// The altitude the strike was tasked at: the exit climbs no higher
    /// than [`EXIT_CLIMB_ABOVE_FT`] over it.
    cruise_ft: Option<f64>,
    /// The last tick a warning sounded.
    warned: Option<u64>,
    /// The threat level when the pass was chosen.
    threat: ThreatLevel,
    /// It left after a defence because it was not safe: at the exit
    /// distance it resumes only if safe, else goes home.
    withdrawn: bool,
    /// The host's inputs for this step, consumed by it.
    inputs: Option<Inputs>,
}

impl Strike {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            phase: Phase::Ingress,
            segment: Segment::Approach,
            since: 0,
            pass: None,
            forced: None,
            forced_station: None,
            target: None,
            station: None,
            kind: None,
            pass_number: 0,
            aim_ft: [0.; 2],
            lead_ft: [0.; 2],
            gate_ft: 0.,
            inside: true,
            holding: None,
            hold_ft: None,
            released: 0,
            next_release: 0,
            exit_heading_deg: 0.,
            jink_side: 1.,
            cruise_ft: None,
            warned: None,
            threat: ThreatLevel::Clear,
            withdrawn: false,
            inputs: None,
        }
    }

    /// Whether the strike flies an aircraft of this class: helicopters and
    /// the gunship keep their existing behaviour until slice ST-F.
    pub fn flies(class: StrikeClass) -> bool {
        !matches!(class, StrikeClass::Helicopter | StrikeClass::Gunship)
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }
    pub fn segment(&self) -> Segment {
        self.segment
    }
    /// The pass being flown (or last flown), for the driver's report.
    pub fn pass(&self) -> Option<Pass> {
        self.pass
    }
    pub fn target(&self) -> Option<u32> {
        self.target
    }
    pub fn pass_number(&self) -> u32 {
        self.pass_number
    }
    /// The drawn aim offset, feet, and whether it lies inside the circle.
    pub fn aim(&self) -> ([f64; 2], bool) {
        (self.aim_ft, self.inside)
    }
    /// Done: the mission sends it home.
    pub fn home(&self) -> bool {
        self.phase == Phase::Home
    }
    /// Forces the pass and the store (the headless driver's coverage
    /// runs); the real choice is [`choose_pass`].
    pub fn force(&mut self, pass: Option<Pass>, station: Option<StationId>) {
        self.forced = pass;
        self.forced_station = station;
    }
    /// The host's description for the next step.
    pub fn set_inputs(&mut self, inputs: Inputs) {
        self.inputs = Some(inputs);
    }
    pub fn inputs(&self) -> Option<&Inputs> {
        self.inputs.as_ref()
    }
    pub(super) fn take_inputs(&mut self) -> Option<Inputs> {
        self.inputs.take()
    }

    /// What the strike cockpit must hold for the host to solve the next
    /// step.
    pub fn selection(&self) -> Option<Selection> {
        if matches!(self.phase, Phase::Home | Phase::Wait) {
            return None;
        }
        let (target, station, kind) = (self.target?, self.station?, self.kind?);
        let level = self.pass == Some(Pass::Level) && kind == StoreKind::Bomb;
        Some(Selection {
            target,
            station,
            kind,
            mode: if level {
                DeliveryMode::Level
            } else {
                DeliveryMode::Ccip
            },
            aim_ft: if kind.unguided() {
                [
                    self.aim_ft[0] + self.lead_ft[0],
                    self.aim_ft[1] + self.lead_ft[1],
                ]
            } else {
                [0.; 2]
            },
            gate_ft: self.gate_ft,
            laser: kind == StoreKind::Laser,
        })
    }

    fn enter(&mut self, phase: Phase, tick: u64) {
        self.phase = phase;
        self.segment = Segment::Approach;
        self.since = tick;
    }

    fn segment_to(&mut self, segment: Segment, tick: u64) {
        self.segment = segment;
        self.since = tick;
    }

    /// One mission step (see the module notes).
    pub fn step(&mut self, cx: &Context<'_>) -> Order {
        let own = cx.own;
        let mut order = Order {
            steer: None,
            release: None,
            activity: None,
        };
        if cx.warnings.air_to_air || cx.warnings.surface_to_air {
            self.warned = Some(cx.tick);
        }
        if self.cruise_ft.is_none() {
            self.cruise_ft = Some(own.position[1]);
        }
        if cx.recovering {
            self.phase = Phase::Home;
        }
        if self.phase == Phase::Home {
            return order;
        }
        // Defend: the existing reactions fly; resume at ingress once the
        // threat has gone (no warning in 10 s, no fighter threatening).
        if cx.defending || self.threatened(cx) {
            if self.phase != Phase::Defend {
                self.enter(Phase::Defend, cx.tick);
                self.released = 0;
            }
            return order;
        }
        if self.phase == Phase::Defend {
            if !self.quiet(cx) {
                return order;
            }
            // The threat has gone: resume when safe, else egress and look
            // again from the exit distance (spec, "Defend" and "Safe").
            if self.safe(cx) {
                self.enter(Phase::Ingress, cx.tick);
            } else {
                self.withdrawn = true;
                self.exit(cx);
            }
        }
        let solution = self.solution(cx);
        let target_at = solution
            .map(|s| s.target_position)
            .or_else(|| self.target.and_then(|id| known_position(cx.picture, id)));
        let target_alive = match (self.target, solution) {
            (Some(_), Some(s)) => s.target_alive,
            (Some(id), None) => standing(cx.picture, id),
            (None, _) => false,
        };
        // A pass in flight ends when its target falls.
        if self.phase == Phase::Attack && !target_alive {
            self.exit(cx);
        }
        match self.phase {
            Phase::Ingress | Phase::Wait => {
                let point = target_at.or_else(|| objective_point(cx.picture));
                let near = point.is_none_or(|p| horizontal(own.position, p) <= CHOICE_DISTANCE_FT);
                let waited =
                    self.phase != Phase::Wait || cx.tick.saturating_sub(self.since) >= WAIT_TICKS;
                if near && waited {
                    self.choose(cx);
                }
            }
            Phase::Regain => {
                if energy_good(own) {
                    self.choose(cx);
                }
            }
            Phase::TurnBack => {
                if let Some(point) = target_at {
                    let off =
                        angle_difference(own.heading_deg, bearing_deg(own.position, point)).abs();
                    if off <= TURN_BACK_DEG {
                        self.choose(cx);
                    }
                } else {
                    self.choose(cx);
                }
            }
            Phase::Exit => {
                if let Some(point) = target_at {
                    let away = horizontal(own.position, point);
                    if away >= self.exit_distance(cx) {
                        if !self.withdrawn {
                            self.enter(Phase::TurnBack, cx.tick);
                        } else if self.safe(cx) {
                            self.withdrawn = false;
                            self.enter(Phase::TurnBack, cx.tick);
                        } else {
                            self.enter(Phase::Home, cx.tick);
                        }
                    }
                }
            }
            Phase::Attack | Phase::Defend | Phase::Home => {}
        }
        // The choice may have changed the target or ended the strike.
        let solution = self.solution(cx);
        let target_at = solution
            .map(|s| s.target_position)
            .or_else(|| self.target.and_then(|id| known_position(cx.picture, id)));
        match self.phase {
            Phase::Home | Phase::Defend => order,
            Phase::Ingress | Phase::Wait => {
                let point = target_at.or_else(|| objective_point(cx.picture));
                order.steer = point.map(|p| self.cruise_steer(cx, p, SpeedRequest::Corner));
                order.activity = Some(Activity::Pursuing);
                order
            }
            Phase::Regain => {
                order.steer = Some(Steer {
                    heading_deg: own.heading_deg,
                    pitch_deg: 0.,
                    speed: SpeedRequest::Maximum,
                    afterburner: true,
                });
                order.activity = Some(Activity::Pursuing);
                order
            }
            Phase::TurnBack => {
                order.steer = target_at.map(|p| self.cruise_steer(cx, p, SpeedRequest::Corner));
                order.activity = Some(Activity::Pursuing);
                order
            }
            Phase::Exit => {
                order.steer = Some(self.exit_steer(cx, target_at));
                order.activity = Some(Activity::Attacking);
                order
            }
            Phase::Attack => match target_at {
                Some(point) => self.attack(cx, point, solution),
                None => {
                    self.exit(cx);
                    order.steer = Some(self.exit_steer(cx, None));
                    order
                }
            },
        }
    }

    /// The host's solution, when it is for the current selection.
    fn solution(&self, cx: &Context<'_>) -> Option<Solution> {
        cx.inputs
            .solution
            .filter(|s| Some(s.target) == self.target && Some(s.station) == self.station)
    }

    /// Whether something the striker must answer threatens it now: an
    /// aircraft inside 5 nm facing it (spec, "Defend"). Missile and gun
    /// defences arrive as `defending`.
    fn threatened(&self, cx: &Context<'_>) -> bool {
        cx.picture
            .threats()
            .fighters
            .iter()
            .any(|f| f.distance_ft <= DEFEND_RANGE_FT && f.off_nose_deg <= DEFEND_FACING_DEG)
    }

    /// No warning in the last 10 s.
    fn quiet(&self, cx: &Context<'_>) -> bool {
        self.warned
            .is_none_or(|t| cx.tick.saturating_sub(t) >= WARNING_TICKS)
    }

    /// The spec's safe test (confirmed by John, 2026-10-11): no warning in
    /// 10 s, no known hostile SAM covering the target, and no hostile fighter
    /// inside 15 nm.
    pub fn safe(&self, cx: &Context<'_>) -> bool {
        let target = self
            .target
            .and_then(|id| known_position(cx.picture, id))
            .or_else(|| cx.inputs.solution.map(|s| s.target_position));
        self.quiet(cx)
            && !cx
                .picture
                .threats()
                .fighters
                .iter()
                .any(|f| f.distance_ft <= SAFE_FIGHTER_FT)
            && target.is_none_or(|t| {
                !cx.picture
                    .threats()
                    .hostile_defenses(cx.side)
                    .any(|d| d.covers_with_missiles(t))
            })
    }

    /// The choice: target, store, pass and aim; or wait, or home.
    fn choose(&mut self, cx: &Context<'_>) {
        let inputs = cx.inputs;
        let own = cx.own;
        let Some(target) = self.pick_target(cx) else {
            let waiting = cx
                .picture
                .briefed()
                .iter()
                .any(|b| b.standing && b.side != cx.side && b.kind != BriefedKind::Ordered);
            if waiting && cx.picture.ordered().is_none() {
                self.enter(Phase::Wait, cx.tick);
            } else {
                self.enter(Phase::Home, cx.tick);
            }
            self.target = None;
            return;
        };
        let target_at = known_position(cx.picture, target)
            .or_else(|| inputs.solution.map(|s| s.target_position))
            .unwrap_or(own.position);
        let release_at = target_at;
        let threat = cx
            .picture
            .threats()
            .level(cx.side, &[target_at, release_at], cx.warnings);
        let facts = target_facts(cx.picture, target);
        let store = match self.forced_station {
            Some(station) => inputs
                .stores
                .iter()
                .find(|s| s.station == station && s.rounds > 0)
                .copied(),
            None => choose_store(
                &inputs.stores,
                inputs.class,
                threat,
                facts,
                inputs.designator,
            ),
        };
        let Some(store) = store else {
            self.enter(Phase::Home, cx.tick);
            self.target = None;
            return;
        };
        if self.target != Some(target) {
            self.pass_number = 0;
        }
        self.target = Some(target);
        self.station = Some(store.station);
        self.kind = Some(store.kind);
        self.threat = threat;
        let distance = horizontal(own.position, target_at);
        let agl = own.agl_ft;
        let radius = turn_radius_ft(own.speed.0, own.g_limit);
        let choice = match self.forced {
            Some(pass) => PassChoice::Pass(pass),
            None => choose_pass(PassInputs {
                class: inputs.class,
                kind: store.kind,
                threat,
                energy_good: energy_good(own),
                dive_gate: dive_gate(agl, distance, radius),
            }),
        };
        let pass = match choice {
            PassChoice::Regain => {
                self.enter(Phase::Regain, cx.tick);
                return;
            }
            PassChoice::Pass(pass) => pass,
        };
        self.pass = Some(pass);
        self.pass_number += 1;
        let radius_ft = release_circle_ft(cx.experience);
        let draw = aim_draw(self.seed, cx.aircraft, self.pass_number, radius_ft);
        self.aim_ft = draw.offset_ft;
        self.inside = draw.inside;
        self.jink_side = draw.side;
        self.gate_ft = GATE_SHARE * radius_ft;
        self.holding = None;
        self.released = 0;
        self.next_release = 0;
        let floor = self.floor(cx, threat);
        self.hold_ft = Some(match pass {
            Pass::Level => target_at[1] + agl.max(floor),
            Pass::FastHigh => target_at[1] + FAST_HIGH_OFFSET_FT.max(floor),
            Pass::Dive => target_at[1] + agl.max(2. * radius).max(DIVE_GATE_MIN_FT),
            Pass::PopUp => target_at[1] + POPUP_OFFSET_FT,
            Pass::Straight => target_at[1],
        });
        // A target behind is not attacked from where it stands: the exit
        // opens the distance and turns back (fitted).
        let off = angle_difference(own.heading_deg, bearing_deg(own.position, target_at)).abs();
        if off > 90. && distance < self.exit_distance(cx) {
            self.exit(cx);
            return;
        }
        self.enter(Phase::Attack, cx.tick);
    }

    /// The target the striker attacks next: the ordered one, an
    /// anti-radiation carrier's radars (SAM, then AAA), else the selector's.
    fn pick_target(&self, cx: &Context<'_>) -> Option<u32> {
        let own = cx.own.position;
        let stores = &cx.inputs.stores;
        let anti_radiation = stores
            .iter()
            .any(|s| s.kind == StoreKind::AntiRadiation && s.rounds > 0);
        let other = stores.iter().any(|s| {
            s.rounds > 0
                && s.kind != StoreKind::AntiRadiation
                && s.kind != StoreKind::Command
                && (s.kind != StoreKind::Laser || cx.inputs.designator)
        });
        let attackers = |id: u32| cx.mates.iter().filter(|m| **m == id).count() as u32;
        let usable = |_: u16| anti_radiation || other;
        let candidates = cx
            .picture
            .strike_candidates(own, cx.side, &attackers, &usable);
        let ordered = candidates
            .iter()
            .any(|c| c.source == StrikeSource::Ordered && c.valid && c.side != cx.side);
        if anti_radiation && !ordered {
            let radar = sead_order(cx.picture, cx.side, own, &attackers);
            if let Some(id) = radar.first() {
                return Some(*id);
            }
        }
        targeting::select_strike_target(ObjectId(cx.aircraft), cx.side, &candidates).map(|c| c.id.0)
    }

    /// The release floor of this pass (spec): 3,000 ft or 5,000 ft by
    /// threat; a heavy's own.
    fn floor(&self, cx: &Context<'_>, threat: ThreatLevel) -> f64 {
        match cx.inputs.class {
            StrikeClass::Heavy => heavy_floor_ft(
                cx.inputs.mission_height_ft,
                threat,
                cx.ceiling_ft - cx.own.position[1] + cx.own.agl_ft,
            ),
            _ => release_floor_ft(threat),
        }
    }

    fn exit_distance(&self, cx: &Context<'_>) -> f64 {
        let radius = turn_radius_ft(cx.own.speed.0, cx.own.g_limit);
        let base = if cx.inputs.class == StrikeClass::Heavy {
            HEAVY_EXIT_FT
        } else {
            EXIT_TURN_MIN_FT
        };
        base.max(2. * radius)
    }

    /// Leave the pass: the exit run.
    fn exit(&mut self, cx: &Context<'_>) {
        self.exit_heading_deg = cx.own.heading_deg;
        self.holding = None;
        self.enter(Phase::Exit, cx.tick);
    }

    /// Toward `point` at the cruise altitude (at least the release floor),
    /// as a heavy, at its floor.
    fn cruise_steer(&self, cx: &Context<'_>, point: Vector, speed: SpeedRequest) -> Steer {
        let own = cx.own;
        let floor = self.floor(cx, self.threat);
        let ground = own.position[1] - own.agl_ft;
        let mut altitude = self
            .cruise_ft
            .unwrap_or(own.position[1])
            .max(ground + floor);
        if cx.inputs.class != StrikeClass::Heavy {
            // Fighters keep what they have above the floor.
            altitude = altitude.max(own.position[1].min(ground + floor + 20_000.));
        }
        Steer {
            heading_deg: bearing_deg(own.position, point),
            pitch_deg: hold_pitch_deg(own.position[1], altitude),
            speed,
            afterburner: false,
        }
    }

    fn exit_steer(&self, cx: &Context<'_>, target_at: Option<Vector>) -> Steer {
        let own = cx.own;
        let away = target_at.map_or(f64::INFINITY, |p| horizontal(own.position, p));
        // A dive's exit climbs back to the dive's height for the next pass.
        let dive = (self.pass == Some(Pass::Dive))
            .then_some(self.hold_ft)
            .flatten()
            .unwrap_or(f64::MIN);
        let cap = (self.cruise_ft.unwrap_or(own.position[1]) + EXIT_CLIMB_ABOVE_FT).max(dive);
        let climbing = own.position[1] < cap;
        if away < JINK_INSIDE_FT {
            let leg = cx.tick.saturating_sub(self.since) / JINK_LEG_TICKS % JINK_LEGS;
            let step = if leg.is_multiple_of(2) {
                JINK_STEP_DEG
            } else {
                0.
            };
            Steer {
                heading_deg: wrap_deg(self.exit_heading_deg + self.jink_side * step),
                pitch_deg: if climbing { JINK_CLIMB_DEG } else { 0. },
                speed: SpeedRequest::Maximum,
                afterburner: true,
            }
        } else {
            Steer {
                heading_deg: self.exit_heading_deg,
                pitch_deg: if climbing {
                    EXIT_CLIMB_DEG
                } else {
                    hold_pitch_deg(own.position[1], cap)
                },
                speed: SpeedRequest::Maximum,
                afterburner: true,
            }
        }
    }

    /// The pass in flight.
    fn attack(&mut self, cx: &Context<'_>, target: Vector, solution: Option<Solution>) -> Order {
        let own = cx.own;
        let pass = self.pass.unwrap_or(Pass::Straight);
        let kind = self.kind.unwrap_or(StoreKind::Bomb);
        // A moving target is led by the cue's time of fall (fitted, agent).
        self.lead_ft = match solution {
            Some(s) if kind.unguided() => {
                let t = s.impact_seconds.unwrap_or(0.).clamp(0., 60.);
                [s.target_velocity[0] * t, s.target_velocity[2] * t]
            }
            _ => [0.; 2],
        };
        let aim = [
            target[0]
                + if kind.unguided() {
                    self.aim_ft[0] + self.lead_ft[0]
                } else {
                    0.
                },
            target[1],
            target[2]
                + if kind.unguided() {
                    self.aim_ft[1] + self.lead_ft[1]
                } else {
                    0.
                },
        ];
        let distance = horizontal(own.position, target);
        let radius = turn_radius_ft(own.speed.0, own.g_limit);
        let floor = self.floor(cx, self.threat);
        let mut order = Order {
            steer: None,
            release: None,
            activity: Some(Activity::Attacking),
        };
        let hold = self.hold_ft.unwrap_or(own.position[1]);
        let speed = if pass == Pass::FastHigh {
            SpeedRequest::Maximum
        } else {
            SpeedRequest::Corner
        };
        let afterburner = pass == Pass::FastHigh;
        let ahead =
            angle_difference(own.heading_deg, bearing_deg(own.position, target)).abs() < 90.;

        // The approach and the pass's own end.
        let level_like =
            pass == Pass::Level || (pass == Pass::FastHigh && !kind.seeker()) || kind.level_run();
        if self.segment == Segment::Approach {
            match pass {
                Pass::PopUp if distance < popup_pull_at(kind) && ahead && !kind.seeker() => {
                    self.segment_to(Segment::Pull, cx.tick)
                }
                Pass::Dive if depression_deg(own.position, aim) >= DIVE_START_DEG => {
                    self.segment_to(Segment::Final, cx.tick)
                }
                Pass::Straight if kind.unguided() => self.segment_to(Segment::Final, cx.tick),
                _ if level_like || kind.seeker() => self.segment_to(Segment::Final, cx.tick),
                _ => {}
            }
        }
        if self.segment == Segment::Pull
            && cx.tick.saturating_sub(self.since) >= POPUP_PULL_TICKS
            && (depression_deg(own.position, target) >= POPUP_DIVE_DEG
                || distance < 2. * POPUP_EXIT_FT)
        {
            self.segment_to(Segment::Final, cx.tick);
        }
        if self.segment == Segment::Climb {
            if cx.tick.saturating_sub(self.since) >= DIVE_CLIMB_TICKS {
                self.exit(cx);
                return Order {
                    steer: Some(self.exit_steer(cx, Some(target))),
                    ..order
                };
            }
            order.steer = Some(Steer {
                heading_deg: own.heading_deg,
                pitch_deg: DIVE_CLIMB_DEG,
                speed: SpeedRequest::Corner,
                afterburner: false,
            });
            return order;
        }
        let ended = match pass {
            _ if self.released > 0 && !self.burst_left(cx, solution, aim) => true,
            Pass::Dive => {
                (self.segment == Segment::Final
                    && own.agl_ft < (2. * radius).max(DIVE_PULL_OUT_MIN_FT))
                    || (self.segment == Segment::Approach && !ahead)
            }
            Pass::PopUp => self.segment == Segment::Final && (distance < POPUP_EXIT_FT || !ahead),
            Pass::Straight => distance < STRAIGHT_EXIT_FT,
            Pass::FastHigh => distance < FAST_HIGH_EXIT_FT && !level_like,
            Pass::Level => false,
        } || (level_like && self.overflown(cx, target, aim, solution))
            || (kind.seeker() && distance < STRAIGHT_EXIT_FT)
            || (kind.level_run() && distance < GUIDED_EXIT_FT);
        if ended {
            if pass == Pass::Dive && self.segment == Segment::Final {
                self.segment_to(Segment::Climb, cx.tick);
                order.steer = Some(Steer {
                    heading_deg: own.heading_deg,
                    pitch_deg: DIVE_CLIMB_DEG,
                    speed: SpeedRequest::Corner,
                    afterburner: false,
                });
                return order;
            }
            self.exit(cx);
            order.steer = Some(self.exit_steer(cx, Some(target)));
            return order;
        }

        // Steering.
        let steer = match self.segment {
            Segment::Approach => {
                // Every approach is flown at the aim, so the cue starts in
                // line with it (the aim is the target for a guided store).
                let along = [aim[0] - own.position[0], aim[2] - own.position[2]];
                let reach = along[0].hypot(along[1]).max(1.);
                let u = [along[0] / reach, along[1] / reach];
                let short_of = |by: f64, height: f64| -> Vector {
                    [aim[0] - u[0] * by, height, aim[2] - u[1] * by]
                };
                let point = match pass {
                    // A low approach under the radar: down to 500 ft over
                    // the target by the pull point (fitted, agent), then on
                    // at that height (retail).
                    Pass::PopUp if reach > popup_pull_at(kind) + POPUP_LEAD_FT => {
                        short_of(popup_pull_at(kind), target[1] + POPUP_OFFSET_FT)
                    }
                    Pass::PopUp => short_of(0., target[1] + POPUP_OFFSET_FT),
                    Pass::FastHigh => short_of(0., hold),
                    // The roll-in point: the dive's height, where the aim
                    // will be the dive's start angle down (fitted, agent);
                    // over the target once inside it (retail).
                    Pass::Dive => {
                        let rollin = (hold - target[1]) / DIVE_START_DEG.to_radians().tan();
                        if reach > rollin + DIVE_ROLLIN_LEAD_FT {
                            short_of(rollin, hold)
                        } else {
                            short_of(0., hold)
                        }
                    }
                    _ => short_of(0., target[1]),
                };
                let (heading, pitch) = steering_frame(own.position, point);
                Steer {
                    heading_deg: heading,
                    pitch_deg: pitch,
                    speed,
                    afterburner,
                }
            }
            Segment::Pull => Steer {
                heading_deg: bearing_deg(own.position, aim),
                pitch_deg: (-depression_deg(own.position, target) + POPUP_PULL_DEG)
                    .min(MAX_DIVE_DEG),
                speed: SpeedRequest::Corner,
                afterburner: false,
            },
            Segment::Final if level_like || kind.seeker() => {
                let heading = solution
                    .and_then(|s| s.impact)
                    .filter(|_| kind.unguided())
                    .map_or_else(
                        || bearing_deg(own.position, aim),
                        |impact| cue_heading_deg(own, cx.velocity, aim, impact),
                    );
                let pitch = if kind.seeker() && pass != Pass::Level && pass != Pass::FastHigh {
                    steering_frame(own.position, target).1
                } else {
                    hold_pitch_deg(
                        own.position[1],
                        hold.max(own.position[1] - own.agl_ft + floor),
                    )
                };
                Steer {
                    heading_deg: heading,
                    pitch_deg: pitch,
                    speed,
                    afterburner,
                }
            }
            Segment::Final => {
                let (heading, pitch) = match solution.and_then(|s| s.impact) {
                    Some(impact) => {
                        let line = steering_frame(own.position, target).1;
                        let pitch = self.fly_cue(cx, aim, impact, line);
                        (cue_heading_deg(own, cx.velocity, aim, impact), pitch)
                    }
                    None => steering_frame(own.position, aim),
                };
                // A push over (the adapter pushes negative G for a pitch far
                // below the flight path, where a banked push turns the wrong
                // way) is flown wings level (fitted, agent: retail rolls
                // inverted and pulls, which the input-only adapter cannot).
                let heading = if pushes(own, pitch) {
                    own.heading_deg
                } else {
                    heading
                };
                Steer {
                    heading_deg: heading,
                    pitch_deg: pitch.max(-MAX_DIVE_DEG),
                    speed: SpeedRequest::Corner,
                    afterburner: false,
                }
            }
            Segment::Climb => unreachable!("handled above"),
        };
        order.steer = Some(steer);

        // The release.
        if matches!(self.segment, Segment::Final | Segment::Pull)
            && let Some(s) = solution
            && cx.tick >= self.next_release
        {
            let floor_ok = match pass {
                Pass::Dive | Pass::Level => own.agl_ft >= floor,
                _ => true,
            } || !kind.unguided();
            let first = self.released == 0;
            let in_zone = !kind.unguided()
                || s.impact.is_some_and(|i| {
                    let range = (horizontal(own.position, i).powi(2)
                        + (own.position[1] - i[1]).powi(2))
                    .sqrt();
                    self.store(cx).is_none_or(|st| {
                        range >= st.minimum_range_ft
                            && (st.maximum_range_ft <= 0. || range <= st.maximum_range_ft)
                    })
                });
            let fire = in_zone
                && match kind {
                    StoreKind::Rocket => {
                        (first && s.due)
                            || (!first
                                && self.released < ROCKETS_PER_PASS
                                && s.impact.is_some_and(|i| {
                                    horizontal(i, aim) <= release_circle_ft(cx.experience)
                                }))
                    }
                    k if k.seeker() => first && s.due && !s.guided_in_flight,
                    _ => first && s.due,
                };
            if fire && floor_ok && !s.warning && self.rounds_left(cx) > 0 {
                order.release = Some(Release {
                    station: s.station,
                    target: s.target,
                });
                self.released += 1;
                self.next_release = cx.tick
                    + self
                        .store(cx)
                        .map_or(super::QUARTER_SECOND_TICKS, |st| st.cadence_ticks);
                // A bomb, a guided weapon or a seeker leaves once a pass;
                // the run then exits (a dive climbs out first).
                if kind != StoreKind::Rocket && pass != Pass::Dive {
                    self.exit(cx);
                } else if kind != StoreKind::Rocket {
                    self.segment_to(Segment::Climb, cx.tick);
                }
            }
        }
        order
    }

    fn store(&self, cx: &Context<'_>) -> Option<Store> {
        let station = self.station?;
        cx.inputs
            .stores
            .iter()
            .find(|s| s.station == station)
            .copied()
    }

    fn rounds_left(&self, cx: &Context<'_>) -> u32 {
        self.store(cx).map_or(0, |s| s.rounds)
    }

    /// A rocket burst goes on while rounds and the circle last.
    fn burst_left(&self, cx: &Context<'_>, solution: Option<Solution>, aim: Vector) -> bool {
        self.kind == Some(StoreKind::Rocket)
            && self.released < ROCKETS_PER_PASS
            && self.rounds_left(cx) > 0
            && solution
                .and_then(|s| s.impact)
                .is_some_and(|i| horizontal(i, aim) <= release_circle_ft(cx.experience))
    }

    /// The pitch that flies the cue onto the aim in a diving pass (fitted,
    /// agent): while the cue is far short, the pass's own line (`line`,
    /// degrees); else the flight path is turned by [`CUE_PITCH_GAIN`] times
    /// the angle between the cue and a point [`CUE_MARGIN_FT`] short of the
    /// aim, never more than 8 degrees above the line, until the cue is
    /// within that margin short of the aim; then the flight path is held and
    /// the cue creeps across the aim, as it does in any steady dive or level
    /// run. A cue that overshoots by the circle is flown again.
    fn fly_cue(&mut self, cx: &Context<'_>, aim: Vector, impact: Vector, line: f64) -> f64 {
        let own = cx.own;
        let Some(u) = track_unit(cx.velocity) else {
            return line;
        };
        let short = (aim[0] - impact[0]) * u[0] + (aim[2] - impact[2]) * u[1];
        if let Some(hold) = self.holding {
            if short >= -release_circle_ft(cx.experience) {
                return hold;
            }
            self.holding = None;
        }
        if short > CUE_ACQUIRE_FT {
            return line;
        }
        if (0.0..=CUE_MARGIN_FT + CUE_HOLD_TOLERANCE_FT).contains(&short) {
            self.holding = Some(own.flight_path_pitch_deg);
            return own.flight_path_pitch_deg;
        }
        let q = [
            aim[0] - u[0] * CUE_MARGIN_FT,
            aim[1],
            aim[2] - u[1] * CUE_MARGIN_FT,
        ];
        let turn = elevation_deg(own.position, q) - elevation_deg(own.position, impact);
        (own.flight_path_pitch_deg + CUE_PITCH_GAIN * turn).min(line + 8.)
    }

    /// A level-like run has passed its chance: the cue (or, with none, the
    /// aircraft) is beyond the aim point by more than the circle, or the
    /// target is behind.
    fn overflown(
        &self,
        cx: &Context<'_>,
        target: Vector,
        aim: Vector,
        solution: Option<Solution>,
    ) -> bool {
        let own = cx.own;
        let track = [cx.velocity[0], cx.velocity[2]];
        let norm = track[0].hypot(track[1]);
        if norm < 1. {
            return false;
        }
        let u = [track[0] / norm, track[1] / norm];
        let along = |from: Vector| (aim[0] - from[0]) * u[0] + (aim[2] - from[2]) * u[1];
        match solution.and_then(|s| s.impact) {
            Some(impact) if self.kind.is_some_and(StoreKind::unguided) => {
                along(impact) < -release_circle_ft(cx.experience)
            }
            _ => along(own.position) < 0. && horizontal(own.position, target) > 0.,
        }
    }
}

/// Retail's pass choice outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassChoice {
    Pass(Pass),
    /// Too slow under threat: regain speed first.
    Regain,
}

/// What the pass choice reads.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PassInputs {
    pub class: StrikeClass,
    pub kind: StoreKind,
    pub threat: ThreatLevel,
    pub energy_good: bool,
    pub dive_gate: bool,
}

/// The pass, a fixed function of the bombing class, the store, the
/// perceived threat and the available speed (spec, "Pass choice"; John,
/// 2026-10-11, decision 3: no dice).
pub fn choose_pass(i: PassInputs) -> PassChoice {
    use PassChoice::Pass as P;
    if i.class == StrikeClass::Heavy {
        return P(Pass::Level);
    }
    match i.kind {
        k if k.seeker() => P(if i.energy_good {
            Pass::FastHigh
        } else {
            Pass::Straight
        }),
        // The spec's pop-up under threat Missiles is not flown with rockets
        // (fitted, agent): after a low pop the dive that points the nose at
        // the target is steeper than the AI's terrain floor allows, so the
        // run would end without a rocket. A straight run with its shallow
        // dive it is, under every threat.
        StoreKind::Rocket => P(Pass::Straight),
        StoreKind::Laser | StoreKind::Coordinate => P(Pass::Level),
        _ => {
            if !i.energy_good && i.threat >= ThreatLevel::Guns {
                PassChoice::Regain
            } else if i.threat == ThreatLevel::Missiles {
                P(Pass::PopUp)
            } else if i.dive_gate {
                P(Pass::Dive)
            } else {
                P(Pass::Straight)
            }
        }
    }
}

/// What the striker knows of a target for the store choice.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TargetFacts {
    /// It has a radar (a briefed or perceived defence's radar).
    pub radar: bool,
    /// It emits now.
    pub emitting: bool,
}

/// The store for a target (spec, "Weapons and releases", "Store choice";
/// fitted order, agent): an anti-radiation missile at a radar; under threat
/// Missiles stand-off stores first; else bombs, then guided bombs, seekers
/// and rockets. A laser store only with a spot assured (its own
/// designator, decision 9). A heavy carries no rockets into a pass. The
/// AT-2 and guns are not chosen here.
pub fn choose_store(
    stores: &[Store],
    class: StrikeClass,
    threat: ThreatLevel,
    target: TargetFacts,
    designator: bool,
) -> Option<Store> {
    use StoreKind as K;
    let order: &[K] = if threat == ThreatLevel::Missiles {
        &[
            K::AntiRadiation,
            K::Seeker,
            K::RadarAgm,
            K::Coordinate,
            K::Laser,
            K::Bomb,
            K::Rocket,
        ]
    } else {
        &[
            K::AntiRadiation,
            K::Bomb,
            K::Coordinate,
            K::Laser,
            K::Seeker,
            K::RadarAgm,
            K::Rocket,
        ]
    };
    order.iter().find_map(|kind| {
        let allowed = match kind {
            K::AntiRadiation => target.radar || target.emitting,
            K::Laser => designator,
            K::Rocket => class != StrikeClass::Heavy,
            K::Command => false,
            _ => true,
        };
        allowed
            .then(|| {
                stores
                    .iter()
                    .find(|s| s.kind == *kind && s.rounds > 0)
                    .copied()
            })
            .flatten()
    })
}

/// The SEAD order of an anti-radiation carrier's targets (spec, "SEAD";
/// John, decision 15): the objective's SAM radars, then AAA with radar,
/// then any other radar, each nearest first, standing and hostile, with
/// fewer than two other attackers.
pub fn sead_order(
    picture: &GroundPicture,
    side: Side,
    own: Vector,
    attackers: &dyn Fn(u32) -> u32,
) -> Vec<u32> {
    let mut ranked: Vec<(u8, f64, u32)> = picture
        .briefed()
        .iter()
        .filter(|b| b.standing && b.side != side)
        .filter(|b| attackers(b.id) < targeting::STRIKE_ATTACKER_ALLOWANCE)
        .filter_map(|b| {
            let facts = b.defense?;
            if !facts.radar {
                return None;
            }
            let rank = if facts.missile_reach_ft > 0. || b.class & CLASS_SAM != 0 {
                0
            } else if facts.gun_reach_ft > 0. || b.class & CLASS_AAA != 0 {
                1
            } else {
                2
            };
            Some((rank, horizontal(own, b.position), b.id))
        })
        .collect();
    ranked.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
    ranked.into_iter().map(|(_, _, id)| id).collect()
}

/// The radius of John's release circle by skill (2026-10-11): Ace 100 m,
/// 75 m more for each level down.
pub fn release_circle_ft(level: Experience) -> f64 {
    let metres = match level {
        Experience::Ace => 100.,
        Experience::Experienced => 175.,
        Experience::Average => 250.,
        Experience::Novice => 325.,
    };
    metres * FEET_PER_METRE
}

/// One pass's seeded draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AimDraw {
    /// The aim point's ground offset from the target, feet along x and z.
    pub offset_ft: [f64; 2],
    /// It lies inside the circle (90% of passes).
    pub inside: bool,
    /// The exit jink's side, +1 right or -1 left.
    pub side: f64,
}

/// The pass's aim offset (spec, "Skill and release precision"): a
/// deterministic function of the mission's seed, the aircraft and the pass
/// number, the same on every host and in a replay. 90% of passes inside the
/// circle of radius `radius_ft`, uniform by area (distance `R sqrt(u)`); 10%
/// in the ring from R to 2R (distance `R sqrt(1 + 3u)`); a uniform bearing.
/// It owns no generator state: the same three inputs give the same draw.
pub fn aim_draw(seed: u64, aircraft: u32, pass: u32, radius_ft: f64) -> AimDraw {
    let mut key = seed
        ^ u64::from(aircraft).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ u64::from(pass).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
        ^ 0x5354_5249_4B45_0001;
    let mut unit = || {
        key = mix(key);
        (key >> 11) as f64 / (1u64 << 53) as f64
    };
    let band = unit();
    let spread = unit();
    let turn = unit();
    let side = unit();
    let inside = band < IN_CIRCLE_SHARE;
    let distance = if inside {
        radius_ft * spread.sqrt()
    } else {
        radius_ft * (1. + 3. * spread).sqrt()
    };
    let angle = turn * std::f64::consts::TAU;
    AimDraw {
        offset_ft: [distance * angle.sin(), distance * angle.cos()],
        inside,
        side: if side < 0.5 { -1. } else { 1. },
    }
}

/// SplitMix64's finaliser.
fn mix(mut z: u64) -> u64 {
    z = z.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The release floor of a fighter's or attack jet's dive or level release
/// (spec): 3,000 ft under Clear or Air, 5,000 ft under Guns or Missiles.
pub fn release_floor_ft(threat: ThreatLevel) -> f64 {
    match threat {
        ThreatLevel::Clear | ThreatLevel::Air => FLOOR_CLEAR_FT,
        ThreatLevel::Guns | ThreatLevel::Missiles => FLOOR_DEFENDED_FT,
    }
}

/// A heavy's minimum release height above the ground (spec, "Heavy
/// bombers"; John, 2026-10-11): max(5,000 ft, the mission's airborne
/// height), plus 5,000 ft under Guns and 10,000 ft under Missiles, never
/// above 90% of its ceiling (`ceiling_agl_ft`, above the same ground).
pub fn heavy_floor_ft(
    mission_height_ft: Option<f64>,
    threat: ThreatLevel,
    ceiling_agl_ft: f64,
) -> f64 {
    let base = HEAVY_MIN_FT.max(mission_height_ft.unwrap_or(0.));
    let raise = match threat {
        ThreatLevel::Clear | ThreatLevel::Air => 0.,
        ThreatLevel::Guns => HEAVY_RAISE_GUNS_FT,
        ThreatLevel::Missiles => HEAVY_RAISE_MISSILES_FT,
    };
    let cap = HEAVY_CEILING_SHARE * ceiling_agl_ft;
    if cap.is_finite() && cap > 0. {
        (base + raise).min(cap)
    } else {
        base + raise
    }
}

/// The retail turn radius, V² over 43.6 G, feet (V in ft/s).
pub fn turn_radius_ft(speed_fps: f64, g: f64) -> f64 {
    speed_fps * speed_fps / (43.6 * g.max(1.))
}

/// Retail's dive gate without its dice: height above the ground and
/// horizontal distance both at least max(10,000 ft, 2R).
pub fn dive_gate(agl_ft: f64, distance_ft: f64, radius_ft: f64) -> bool {
    let min = DIVE_GATE_MIN_FT.max(2. * radius_ft);
    agl_ft >= min && distance_ft >= min
}

/// Energy is Good at or above the corner speed (spec).
pub fn energy_good(own: &OwnState) -> bool {
    own.speed.0 >= own.limits.corner.0
}

/// The retail steering frame (spec, "The steering frame"): straight at the
/// point; a pitch within 3 degrees snaps level while the point is 3,000 ft
/// or more away; clamped to the 80 degree climb and dive limits. Returns
/// (heading, pitch) in degrees.
pub fn steering_frame(own: Vector, point: Vector) -> (f64, f64) {
    let heading = bearing_deg(own, point);
    let run = horizontal(own, point);
    let rise = point[1] - own[1];
    let mut pitch = rise.atan2(run).to_degrees();
    if pitch.abs() < 3. && (run * run + rise * rise).sqrt() >= 3_000. {
        pitch = 0.;
    }
    (heading, pitch.clamp(-MAX_DIVE_DEG, MAX_DIVE_DEG))
}

/// The heading that moves the cue onto the aim across the track (fitted,
/// agent): the body heading (which the adapter steers, and which a bomb
/// leaves along) turned by [`CUE_HEADING_GAIN`] times the angle, seen from
/// the aircraft, between the aim and the line the cue sweeps along the
/// track.
pub fn cue_heading_deg(own: &OwnState, velocity: Vector, aim: Vector, impact: Vector) -> f64 {
    let Some(u) = track_unit(velocity) else {
        return bearing_deg(own.position, aim);
    };
    let right = [u[1], -u[0]];
    let cross = (aim[0] - impact[0]) * right[0] + (aim[2] - impact[2]) * right[1];
    let reach = horizontal(own.position, aim).max(500.);
    // `right` points to the right of the track (headings turn clockwise,
    // from +z toward +x), so an aim to the right turns the heading up.
    let correction = (CUE_HEADING_GAIN * cross.atan2(reach).to_degrees()).clamp(-30., 30.);
    wrap_deg(own.heading_deg + correction)
}

/// Where a pop-up pulls up, feet from the target.
fn popup_pull_at(kind: StoreKind) -> f64 {
    if kind == StoreKind::Rocket {
        ROCKET_POPUP_PULL_AT_FT
    } else {
        POPUP_PULL_AT_FT
    }
}

/// Whether flying `pitch` (degrees) asks the adapter for less than
/// [`PUSH_G`]: a push, in which a banked aircraft turns the wrong way.
pub fn pushes(own: &OwnState, pitch: f64) -> bool {
    let error = (pitch - own.flight_path_pitch_deg).to_radians();
    let g = own.flight_path_pitch_deg.to_radians().cos()
        + own.speed.0.max(100.) * error / ADAPTER_PITCH_S / 32.174;
    g < PUSH_G
}

/// The horizontal direction of travel, when the aircraft moves.
fn track_unit(velocity: Vector) -> Option<[f64; 2]> {
    let norm = velocity[0].hypot(velocity[2]);
    (norm > 1.).then(|| [velocity[0] / norm, velocity[2] / norm])
}

/// How far above the horizon `point` is, degrees (negative below).
pub fn elevation_deg(own: Vector, point: Vector) -> f64 {
    -depression_deg(own, point)
}

/// The pitch that holds `altitude`: toward a point 6,000 ft ahead at that
/// altitude, within 10 degrees, with the steering frame's 3 degree snap.
pub fn hold_pitch_deg(current_ft: f64, altitude_ft: f64) -> f64 {
    let pitch = (altitude_ft - current_ft).atan2(6_000.).to_degrees();
    if pitch.abs() < 0.5 {
        0.
    } else {
        pitch.clamp(-10., 10.)
    }
}

/// How far below the horizon `point` is, degrees (negative above).
pub fn depression_deg(own: Vector, point: Vector) -> f64 {
    (own[1] - point[1])
        .atan2(horizontal(own, point).max(1e-6))
        .to_degrees()
}

pub fn horizontal(a: Vector, b: Vector) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

/// Heading from `from` to `to`, degrees, 0 along +z, clockwise to +x.
pub fn bearing_deg(from: Vector, to: Vector) -> f64 {
    wrap_deg((to[0] - from[0]).atan2(to[2] - from[2]).to_degrees())
}

fn wrap_deg(deg: f64) -> f64 {
    deg.rem_euclid(360.)
}

/// `to - from`, the short way, -180..180.
pub fn angle_difference(from: f64, to: f64) -> f64 {
    (to - from + 180.).rem_euclid(360.) - 180.
}

/// Where the picture knows `id` stands.
fn known_position(picture: &GroundPicture, id: u32) -> Option<Vector> {
    picture
        .briefed()
        .iter()
        .find(|b| b.id == id)
        .map(|b| b.position)
        .or_else(|| {
            picture
                .seen()
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.position)
        })
        .or_else(|| picture.contact(id).map(|c| c.position))
}

fn standing(picture: &GroundPicture, id: u32) -> bool {
    match picture.briefed().iter().find(|b| b.id == id) {
        Some(b) => b.standing,
        None => picture.seen().iter().any(|s| s.id == id),
    }
}

fn objective_point(picture: &GroundPicture) -> Option<Vector> {
    picture.objective().and_then(|o| o.point)
}

fn target_facts(picture: &GroundPicture, id: u32) -> TargetFacts {
    let briefed = picture.briefed().iter().find(|b| b.id == id);
    let seen = picture.seen().iter().find(|s| s.id == id);
    let radar = briefed
        .and_then(|b| b.defense)
        .or_else(|| seen.and_then(|s| s.defense))
        .is_some_and(|d| d.radar);
    let emitting =
        seen.is_some_and(|s| s.emitting) || picture.contact(id).is_some_and(|c| c.emitting);
    TargetFacts { radar, emitting }
}

#[path = "strike_checkpoint.rs"]
mod checkpoint;

#[cfg(test)]
#[path = "strike_tests.rs"]
mod tests;
