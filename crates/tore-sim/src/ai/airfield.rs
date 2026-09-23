//! Airfield sequences for AI aircraft: waiting, taxi, takeoff, marshal,
//! approach, landing, rollout and parking.
//!
//! The behaviour is specified in
//! [`docs/spec/ai-airfield.md`](../../../../docs/spec/ai-airfield.md) from the
//! recovered retail takeoff and landing handlers (source points in
//! `docs/formats/ai.md`, "Airfield takeoff and landing sequences") and the
//! manual. Every constant below names its provenance: `spec-derived` (retail
//! handler or manual page) or `fitted` (agent decision, 2026-09-23), with the
//! retail value alongside where the hybrid flight model cannot hold it.
//!
//! [`Sequence::step`] is pure and deterministic: it reads one [`Situation`]
//! and returns one [`Command`]. The mission evaluates the retail gates (turn,
//! runway free, earlier wing members down, parking slot) and passes them in.

use super::controller::Activity;
use crate::airport::{ApproachEnd, Runway};
use crate::runway_wind::FEET_PER_SECOND_PER_KNOT as KNOT;

/// A copyable view of one runway, enough for an AI aircraft to line up, take
/// off from and land on it. Built from the host's [`Runway`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunwayView {
    /// Airport identity, as used by the tower service.
    pub airport: u32,
    /// Runway object identity.
    pub object: u32,
    /// Runway centre on the approach centerline, feet.
    pub center: [f64; 3],
    /// Primary (near-end) departure heading, radians, clockwise from +Z.
    pub heading: f64,
    pub length_ft: f64,
    pub elevation_ft: f64,
    /// The airport's own taxi, takeoff, landing and parking points, when its
    /// STRIP shape supplies them.
    pub anchors: Option<AirfieldAnchors>,
}

/// World positions (feet) of one airport's STRIP template points, in the
/// roles recovered in `docs/formats/native-strip.md`. Headings are radians,
/// clockwise from +Z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirfieldAnchors {
    /// Box 0x25 (parking-area exit) then 0x26, 0x27, 0x28 (taxiway legs; 0x28
    /// sits on the extended centerline behind the takeoff spot).
    pub taxi_out: [[f64; 3]; 4],
    /// Box 0x11.
    pub takeoff_spot: [f64; 3],
    pub takeoff_heading: f64,
    /// Box 0x12: landing aim point and centre of the marshal square.
    pub landing_point: [f64; 3],
    pub landing_heading: f64,
    /// Boxes 0x29 (rollout end), 0x2a (runway exit), 0x2b (return taxiway),
    /// 0x2c (parking-area entry).
    pub taxi_in: [[f64; 3]; 4],
    /// Boxes 0x19..0x21: nine parking slots.
    pub parking: [[f64; 3]; 9],
    /// Parked heading: airport heading plus 90 degrees.
    pub parking_heading: f64,
}

impl From<&Runway> for RunwayView {
    fn from(runway: &Runway) -> Self {
        Self {
            airport: runway.airport,
            object: runway.object,
            center: runway.approach_center,
            heading: runway.heading,
            length_ft: runway.length_ft,
            elevation_ft: runway.elevation_ft,
            anchors: None,
        }
    }
}

impl RunwayView {
    pub fn with_anchors(mut self, anchors: Option<AirfieldAnchors>) -> Self {
        self.anchors = anchors;
        self
    }
    /// Threshold point of either end, at runway elevation.
    pub fn threshold(&self, end: ApproachEnd) -> [f64; 3] {
        let sign = if end == ApproachEnd::Near { -1. } else { 1. };
        [
            self.center[0] + self.heading.sin() * self.length_ft * 0.5 * sign,
            self.elevation_ft,
            self.center[2] + self.heading.cos() * self.length_ft * 0.5 * sign,
        ]
    }

    /// Heading for a departure or approach toward the given end's opposite.
    pub fn heading_from(&self, end: ApproachEnd) -> f64 {
        if end == ApproachEnd::Near {
            self.heading
        } else {
            (self.heading + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        }
    }
}

/// How an AI aircraft begins the mission on the ground.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GroundStart {
    pub runway: RunwayView,
    /// The end the wing departs from.
    pub end: ApproachEnd,
    /// Departure order within the wing: the leader is 0, so the first AI
    /// wingman of a human-led wing is 1.
    pub order: u8,
}

/// Where an aircraft is within an airfield sequence. Airborne free flight is
/// the absence of a phase. Retail B48 state numbers are in brackets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    /// [1] Stopped with brakes set until its turn and a free runway.
    Waiting,
    /// [2..5] Taxiing out along the airport's taxi anchors.
    Taxi,
    /// [6] Lining up on the runway centerline at the takeoff spot.
    LineUp,
    /// [0x11] Takeoff roll and rotation.
    TakeoffRoll,
    /// [0x12] Climbing out on runway heading.
    ClimbOut,
    /// Private route toward the airport before the landing hand-off. Free
    /// flight [0x1f] for the warning gates.
    Inbound,
    /// [0x13] Holding in the marshal square until cleared to land.
    Marshal,
    /// [0x14] Flying the three approach gates, gear down.
    Approach,
    /// [0x15] Final descent to the landing point.
    Final,
    /// [0x17] Braking on the runway after touchdown.
    Rollout,
    /// [0x1b..0x1e] Taxiing clear of the runway toward parking.
    TaxiClear,
    /// [0x1a] Parked for good.
    Parked,
}

impl Phase {
    /// The B47 warning gate reads takeoff, early-approach and late-landing
    /// states from this.
    pub fn flight_state(self) -> super::threat::FlightState {
        use super::threat::FlightState;
        match self {
            Self::Waiting | Self::Taxi | Self::LineUp | Self::TakeoffRoll | Self::ClimbOut => {
                FlightState::TakingOff
            }
            Self::Inbound => FlightState::Free,
            Self::Marshal | Self::Approach => FlightState::EarlyApproach,
            Self::Final | Self::Rollout | Self::TaxiClear | Self::Parked => {
                FlightState::LateLanding
            }
        }
    }
}

/// Why an aircraft is landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandingReason {
    /// Bug out: return to base and stop answering orders.
    BugOut,
    /// The player ordered the wing to land at a chosen airport.
    Ordered,
    /// Bingo fuel.
    Fuel,
    /// An AI wingman joining its landing leader (B48 join-landing).
    JoinLeader,
}

impl LandingReason {
    /// Whether a later disengage or formation order cancels this landing
    /// while it is still in the early approach (fitted, agent decision
    /// 2026-09-23). Bug out and fuel landings cannot be cancelled.
    pub fn cancellable(self) -> bool {
        matches!(self, Self::Ordered | Self::JoinLeader)
    }
}

/// A landing request carried on the wing channel.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LandingOrder {
    pub runway: RunwayView,
    pub reason: LandingReason,
}

// ---------------------------------------------------------------------------
// Takeoff numbers.

/// Spec-derived (hold state, 0x4bb314): the takeoff gates are re-tested every
/// 5 s while waiting; the marshal clearance is re-tested on the same cadence.
pub const GATE_RECHECK_S: f64 = 5.0;
/// Spec-derived (airport taxi speed +0xf9): 50 ft/s, 30 kt.
pub const TAXI_SPEED_FPS: f64 = 50.0;
/// Spec-derived: a quarter of the taxi speed when 5 to 45 degrees off the
/// line-up aim. Retail stops and pivots at 45 degrees or more; the hybrid
/// ground model cannot turn a stopped aircraft, so it creeps round at this
/// speed instead (fitted).
pub const SLOW_TAXI_FPS: f64 = 12.5;
pub const PIVOT_HEADING_DEG: f64 = 45.0;
pub const ALIGNED_HEADING_DEG: f64 = 5.0;
/// Spec-derived: within this distance of the takeoff spot the taxiway legs
/// are skipped (and, with anchors, only the turn gate applies).
pub const SPOT_SHORTCUT_FT: f64 = 475.0;
/// Spec-derived: an aircraft on the ground this close to the takeoff spot
/// keeps the runway from being free.
pub const SPOT_OCCUPIED_FT: f64 = 125.0;
/// Spec-derived: further than this from the spot while lining up, go back
/// to waiting.
pub const LINEUP_RESET_FT: f64 = 26_400.0;
/// Spec-derived: a taxiway leg ends within 50 ft of the line through its
/// anchor running 10000 ft back along the reversed runway heading.
pub const LEG_LINE_TOLERANCE_FT: f64 = 50.0;
pub const LEG_LINE_LENGTH_FT: f64 = 10_000.0;
/// Spec-derived: on the centerline within 50 ft, taxi straight to the spot
/// and stop within 10 ft; otherwise aim 50 ft closer along the centerline.
pub const CENTERLINE_TOLERANCE_FT: f64 = 50.0;
pub const SPOT_STOP_FT: f64 = 10.0;
pub const LINEUP_AIM_STEP_FT: f64 = 50.0;
/// Fitted fallback without airport anchors: the line-up spot is on the
/// centerline this far ahead of the aircraft's parking place.
pub const LINEUP_AHEAD_FT: f64 = 200.0;
/// Spec-derived roll: full power for 5 s after reaching minimum speed, then
/// 4 s with the nose 4 degrees above the path (+0x10b).
pub const ROLL_FULL_POWER_S: f64 = 5.0;
pub const ROTATION_S: f64 = 4.0;
pub const ROTATION_NOSE_DEG: f64 = 4.0;
/// Fitted: an aircraft still on its wheels after the rotation window asks
/// for this much nose instead. Retail moves on to the climb regardless.
pub const LATE_ROTATION_NOSE_DEG: f64 = 8.0;
/// Fitted: an afterburning aircraft lights the burner only when it passes
/// this fraction of the runway still below minimum speed. Retail burner use
/// during the roll is unknown.
pub const BURNER_RUNWAY_FRACTION: f64 = 0.5;
/// Spec-derived: climb-out flight path (+0x10b cap) and its end height above
/// terrain, where gear and flaps come up and free flight resumes.
pub const CLIMB_OUT_PITCH_DEG: f64 = 4.0;
pub const CLIMB_OUT_COMPLETE_AGL_FT: f64 = 650.0;
/// Fitted safety: the climb-out ends this long after liftoff in any case.
pub const CLIMB_OUT_TIMEOUT_S: f64 = 90.0;
/// Spec-derived: the human player counts as on its takeoff roll (state
/// 0x11) from this ground speed.
pub const PLAYER_ROLLING_FPS: f64 = 7.0;

// ---------------------------------------------------------------------------
// Landing numbers.

/// Spec-derived: nine parking slots per airport (boxes 0x19..0x21).
pub const PARKING_SLOTS: u32 = 9;
/// Spec-derived marshal square: corners 52800 ft out on each world axis
/// from the landing point; inside 26400 ft go to the +X+Z corner.
pub const MARSHAL_DISTANCE_FT: f64 = 52_800.0;
pub const MARSHAL_INNER_FT: f64 = 26_400.0;
/// Spec-derived: marshal altitude 6000 ft plus 1000 ft per wing position,
/// absolute. Fitted floor: never below 1000 ft above the airfield.
pub const MARSHAL_ALTITUDE_FT: f64 = 6_000.0;
pub const MARSHAL_STEP_FT: f64 = 1_000.0;
pub const MARSHAL_FLOOR_FT: f64 = 1_000.0;
/// Spec-derived approach: three gates on a 6 degree path back from the
/// landing point, at 4/6, 2/6 and 1/6 of the marshal distance, each done
/// within 250 ft horizontally.
pub const APPROACH_PATH_DEG: f64 = 6.0;
pub const APPROACH_GATES_FT: [f64; 3] = [35_200.0, 17_600.0, 8_800.0];
pub const GATE_CAPTURE_FT: f64 = 250.0;
/// Fitted: a gate also counts as flown once it is abeam or behind and
/// within this distance, because the hybrid model's turn radius at marshal
/// speed is far larger than the 250 ft capture circle.
pub const GATE_PASSED_FT: f64 = 5_000.0;
/// Spec-derived: at most 366 ft/s (217 kt) from the second gate on, and at
/// most 293 ft/s (174 kt) on final.
pub const GATE_SPEED_LIMIT_FPS: f64 = 366.0;
pub const FINAL_SPEED_LIMIT_FPS: f64 = 293.0;
/// Fitted: final speed as a multiple of the clean envelope minimum, under
/// the retail cap. Retail also holds the nose 17 degrees above the path on
/// final (+0x10f); the hybrid model sets its own angle of attack, so that
/// is not requested.
pub const APPROACH_SPEED_FACTOR: f64 = 1.1;
/// Fitted: glide-path correction gain and limit on final.
pub const GLIDE_CORRECTION_DEG_PER_FT: f64 = 0.02;
pub const GLIDE_CORRECTION_LIMIT_DEG: f64 = 3.0;
/// Fitted flare. Retail has no flare state; the hybrid touchdown check
/// would reject the 6 degree sink rate at the wheels.
pub const FLARE_HEIGHT_FT: f64 = 60.0;
pub const FLARE_PITCH_DEG: f64 = -1.5;
/// Fitted height-to-sink horizon. A fast descent must start easing before
/// the fixed 60 ft throttle-close height to allow the pitch controller to act.
pub const FLARE_SETTLE_S: f64 = 6.0;
/// Fitted: wings held level below this wheel height.
pub const WINGS_LEVEL_HEIGHT_FT: f64 = 50.0;
/// Fitted: final lateral look-ahead bounds (a quarter of the remaining
/// distance) and the largest intercept angle.
pub const FINAL_LOOKAHEAD_MIN_FT: f64 = 1500.0;
pub const FINAL_LOOKAHEAD_MAX_FT: f64 = 8000.0;
pub const FINAL_INTERCEPT_LIMIT_DEG: f64 = 60.0;
/// Fitted: aim point a quarter of the way down the runway when the airport
/// has no landing anchor (manual p.65-68).
pub const AIM_POINT_FRACTION: f64 = 0.25;
/// Fitted: flaps come down below 200 kt on the approach (the manual's
/// p.64 takeoff flap speed).
pub const FLAPS_SPEED_FPS: f64 = 200.0 * KNOT;
/// Fitted: the speedbrake opens when this much faster than requested
/// during the marshal, approach and final.
pub const SPEEDBRAKE_MARGIN_FPS: f64 = 50.0;

// Go-around (fitted safety; retail has none). Inside the gate a badly
// aligned final is abandoned and flown again from the marshal.
pub const GO_AROUND_GATE_FT: f64 = 6_000.0;
pub const GO_AROUND_CROSS_TRACK_FT: f64 = 300.0;
pub const GO_AROUND_HEADING_DEG: f64 = 30.0;
/// Still airborne this fraction of the runway past the aim point.
pub const GO_AROUND_OVERRUN_FRACTION: f64 = 0.35;
/// A bounce above this wheel height during the rollout is a go-around.
pub const BOUNCE_HEIGHT_FT: f64 = 15.0;
/// Fitted go-around climb (John, 2026-09-23: abort and try again rather
/// than eject): full power on the landing course at this flight-path angle
/// (level until above minimum speed) until this height above the runway or
/// this long, then the marshal. The gear comes up above
/// [`GO_AROUND_GEAR_UP_FT`] and the flaps above [`FLAPS_SPEED_FPS`].
pub const GO_AROUND_PITCH_DEG: f64 = 8.0;
pub const GO_AROUND_CLIMB_FT: f64 = 1_000.0;
pub const GO_AROUND_CLIMB_S: f64 = 60.0;
pub const GO_AROUND_GEAR_UP_FT: f64 = 200.0;
/// Fitted: a hazardous final steeper than the steepest commanded glide
/// goes around even when the projected touchdown is over pavement.
pub const GO_AROUND_DESCENT_DEG: f64 = APPROACH_PATH_DEG + GLIDE_CORRECTION_LIMIT_DEG;
/// Fitted terrain clearance on the way to the runway (agent decision,
/// 2026-09-23; retail steers the gates by the B44 floor alone, which looks
/// 1,000 ft ahead). Real approaches such as Simferopol's from the south pass
/// through mountains, so on the route home, at marshal, on the gates and in
/// a go-around the aircraft neither descends within
/// [`TERRAIN_HOLD_FT`] of the highest terrain in the next
/// [`TERRAIN_LOOKAHEAD_FT`] nor stays within [`TERRAIN_CLIMB_FT`] of it,
/// where it climbs at [`TERRAIN_CLIMB_DEG`]. Terrain less than
/// [`TERRAIN_IGNORED_FT`] above the landing point is ignored, so the final
/// gates over a flat airfield are flown as before.
pub const TERRAIN_LOOKAHEAD_FT: f64 = 12_000.0;
pub const TERRAIN_HOLD_FT: f64 = 1_000.0;
pub const TERRAIN_CLIMB_FT: f64 = 500.0;
pub const TERRAIN_CLIMB_DEG: f64 = 20.0;
pub const TERRAIN_IGNORED_FT: f64 = 300.0;
/// Fitted: elevator held during the rollout. Retail holds the nose 8
/// degrees up for 2 s at 146 ft/s; on the hybrid model that lifts the
/// aircraft off again.
pub const ROLLOUT_ELEVATOR: f64 = -0.5;
/// Spec-derived: the rollout ends below 1 ft/s.
pub const ROLLOUT_STOPPED_FPS: f64 = 1.0;
/// Fitted: a taxi-in anchor counts as reached within this distance.
pub const ANCHOR_REACHED_FT: f64 = 30.0;
/// Spec-derived: throttle closed within 200 ft of the parking slot.
pub const PARK_THROTTLE_OFF_FT: f64 = 200.0;

// After landing without airport anchors (fitted).
/// Parking spots start this far before the far end of the runway.
pub const PARK_FROM_FAR_END_FT: f64 = 400.0;
/// Each lower slot moves the spot back by this much.
pub const PARK_SPACING_FT: f64 = 250.0;
/// Spots alternate this far right and left of the centerline.
pub const PARK_OFFSET_FT: f64 = 40.0;
/// A spot is at least this far ahead of where the rollout ended.
pub const PARK_MINIMUM_ROLL_FT: f64 = 150.0;
/// And never closer than this to the far end.
pub const PARK_END_MARGIN_FT: f64 = 60.0;
/// Below this ground speed an aircraft counts as stopped.
pub const STOPPED_FPS: f64 = 0.5;

/// Fitted inversion of the hybrid nosewheel steering in `flight.rs`: full
/// rudder turns a rolling aircraft at about 0.3 rad/s from 40 ft/s up.
const NOSEWHEEL_YAW_RATE_RAD_S: f64 = 0.3;
const NOSEWHEEL_FULL_SPEED_FPS: f64 = 40.0;
/// Fitted: ground heading error, degrees, to requested turn rate, deg/s.
const GROUND_HEADING_GAIN: f64 = 1.0;
/// Fitted: taxi pure-pursuit look-ahead.
const TAXI_LOOKAHEAD_FT: f64 = 80.0;

/// What the aircraft knows about itself and its clearances this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Situation {
    pub tick: u64,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub heading_deg: f64,
    pub body_pitch_deg: f64,
    /// Air-relative speed, ft/s.
    pub speed_fps: f64,
    /// Wheels supported by the researched contact model.
    pub on_ground: bool,
    /// Height above the terrain under the aircraft.
    pub agl_ft: f64,
    /// Highest terrain along the track over the next
    /// [`TERRAIN_LOOKAHEAD_FT`], feet above sea level.
    pub terrain_ahead_ft: f64,
    /// Wheel clearance: position height minus this is wheel height.
    pub ground_clearance_ft: f64,
    /// Loaded envelope limits at the current altitude.
    pub minimum_speed_fps: f64,
    pub maximum_speed_fps: f64,
    pub corner_speed_fps: f64,
    /// B48 cruise speed.
    pub cruise_speed_fps: f64,
    pub has_afterburner: bool,
    /// World wind at the aircraft, ft/s.
    pub wind: [f64; 3],
    /// Position in the wing, leader 0.
    pub wing_position: u8,
    /// Turn gate: every earlier wing member is past its first taxi leg and
    /// a human leader is airborne.
    pub turn_clear: bool,
    /// Runway-free gate at this airport.
    pub runway_free: bool,
    /// Every earlier wing member landing here is on the ground.
    pub wing_landed: bool,
    /// The parking slot this aircraft may hold, if any is free.
    pub free_slot: Option<u32>,
}

/// Direct inputs for a supported aircraft, or guidance for an airborne one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Control {
    /// Throttle 0..1, elevator and rudder -1..1; ailerons stay neutral.
    Ground {
        throttle: f64,
        pitch: f64,
        yaw: f64,
    },
    Air(AirGuidance),
}

/// Airborne guidance, flown through the ordinary steering adapter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AirGuidance {
    pub heading_deg: f64,
    pub flight_path_pitch_deg: f64,
    pub speed_fps: f64,
    /// Hold the wings level instead of banking toward the heading.
    pub wings_level: bool,
    /// Apply the B44 terrain floor.
    pub terrain_floor: bool,
    /// Full military power regardless of the speed error.
    pub full_power: bool,
}

/// One tick's airfield output.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Command {
    pub control: Control,
    pub gear_down: bool,
    pub flaps_down: bool,
    /// Wheel brakes; the host shares this switch with the airbrake.
    pub brakes: bool,
    pub afterburner: bool,
    pub activity: Activity,
}

/// What [`Sequence::step`] produced.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub command: Command,
    /// The takeoff sequence has finished: hand back to normal flight.
    pub complete: bool,
    /// This tick abandoned a final (fitted go-around or bounce).
    pub go_around: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Departure,
    Landing(LandingReason),
}

/// One aircraft's running takeoff or landing sequence.
#[derive(Clone, Debug, PartialEq)]
pub struct Sequence {
    kind: Kind,
    phase: Phase,
    runway: RunwayView,
    /// Departure: the end departed from. Landing: the end crossed first.
    end: ApproachEnd,
    order: u8,
    /// Sub-step within the phase: taxi leg, approach gate.
    leg: usize,
    leg_origin: [f64; 3],
    /// Braking to a stop before the next leg.
    stopping: bool,
    /// Takeoff spot (departure).
    spot: [f64; 3],
    /// Roll: tick minimum speed was reached.
    timer: Option<u64>,
    next_check: u64,
    liftoff_tick: Option<u64>,
    slot: Option<u32>,
    /// Private route leg flown before the landing hand-off, (altitude, speed).
    route: Option<(f64, f64)>,
    marshal_target: Option<[f64; 3]>,
    park: Option<[f64; 3]>,
    flaps_up: bool,
    go_arounds: u32,
    /// Tick a go-around climb began; the marshal hold waits for it to end.
    climb_away: Option<u64>,
    /// A hazard was judged not catastrophic: abort at the next step.
    abort: bool,
}

impl Sequence {
    /// A ground start at `position`, holding with brakes set until its turn.
    /// `slot` is the parking slot it starts in, when it starts in one.
    pub fn departure(start: GroundStart, position: [f64; 3], slot: Option<u32>) -> Self {
        let mut sequence = Self::new(Kind::Departure, Phase::Waiting, start.runway, start.end);
        sequence.order = start.order;
        sequence.slot = slot;
        sequence.spot = match start.runway.anchors {
            Some(anchors) => anchors.takeoff_spot,
            None => {
                // Fitted fallback: the centerline point ahead of the parking place.
                let (along, _) = sequence.frame(position, sequence.runway.threshold(start.end));
                sequence.point(
                    sequence.runway.threshold(start.end),
                    along + LINEUP_AHEAD_FT,
                    0.0,
                )
            }
        };
        sequence
    }

    /// Start a landing from free flight. With airport anchors the retail
    /// landing heading is used; without them the runway end is chosen by
    /// wind when one end has at least a knot more headwind, otherwise the
    /// end facing the aircraft's arrival (fitted). `route` is the private
    /// return leg `(altitude_ft, speed_fps)` flown to the B48 hand-off.
    pub fn landing(
        order: LandingOrder,
        position: [f64; 3],
        wind: [f64; 3],
        max_takeoff_lbs: f64,
        route: Option<(f64, f64)>,
    ) -> Self {
        let end = choose_landing_end(&order.runway, position, wind, max_takeoff_lbs);
        let phase = if route.is_some() {
            Phase::Inbound
        } else {
            Phase::Marshal
        };
        let mut sequence = Self::new(Kind::Landing(order.reason), phase, order.runway, end);
        sequence.route = route;
        sequence.flaps_up = true;
        sequence
    }

    fn new(kind: Kind, phase: Phase, runway: RunwayView, end: ApproachEnd) -> Self {
        Self {
            kind,
            phase,
            runway,
            end,
            order: 0,
            leg: 0,
            leg_origin: [0.0; 3],
            stopping: false,
            spot: runway.center,
            timer: None,
            next_check: 0,
            liftoff_tick: None,
            slot: None,
            route: None,
            marshal_target: None,
            park: None,
            flaps_up: false,
            go_arounds: 0,
            climb_away: None,
            abort: false,
        }
    }

    /// Ask for an abort instead of an ejection (opinionated, John
    /// 2026-09-23): on the approach gates or final it goes around; holding
    /// at marshal it climbs away first; taking off it simply carries on.
    pub fn request_go_around(&mut self) {
        self.abort = true;
    }

    /// Whether this phase judges a hazard as abort-or-eject rather than the
    /// ordinary ejection rule: the takeoff roll and climb-out, and the
    /// landing from the marshal to the end of the rollout.
    pub fn guards_ejection(&self) -> bool {
        matches!(
            self.phase,
            Phase::TakeoffRoll
                | Phase::ClimbOut
                | Phase::Marshal
                | Phase::Approach
                | Phase::Final
                | Phase::Rollout
        )
    }

    /// True while a go-around is still climbing away from the runway.
    pub fn climbing_away(&self) -> bool {
        self.climb_away.is_some()
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn runway(&self) -> &RunwayView {
        &self.runway
    }

    pub fn end(&self) -> ApproachEnd {
        self.end
    }

    /// Departure order, meaningful for a departure only.
    pub fn order(&self) -> u8 {
        self.order
    }

    /// Taxi leg or approach gate within the current phase.
    pub fn leg(&self) -> usize {
        self.leg
    }

    pub fn is_departure(&self) -> bool {
        self.kind == Kind::Departure
    }

    pub fn landing_reason(&self) -> Option<LandingReason> {
        match self.kind {
            Kind::Landing(reason) => Some(reason),
            Kind::Departure => None,
        }
    }

    /// The parking slot held at this airport.
    pub fn slot(&self) -> Option<u32> {
        self.slot
    }

    pub fn go_arounds(&self) -> u32 {
        self.go_arounds
    }

    /// Takeoff spot for a departure; the airport's own spot otherwise.
    pub fn takeoff_spot(&self) -> Option<[f64; 3]> {
        if self.is_departure() {
            Some(self.spot)
        } else {
            self.runway.anchors.map(|a| a.takeoff_spot)
        }
    }

    /// Retail turn gate: states 1..3 (holding, taxi to anchor 0, first
    /// taxiway leg) keep later wing members waiting.
    pub fn holds_followers(&self) -> bool {
        match self.phase {
            Phase::Waiting => true,
            Phase::Taxi => self.leg <= 1,
            _ => false,
        }
    }

    /// Retail runway-free gate: states 2, 3, 6..0x11, 0x14..0x19 and 0x1b
    /// keep the runway busy. Taxiing clear counts as free at once.
    pub fn blocks_runway(&self) -> bool {
        match self.phase {
            Phase::Taxi => self.leg <= 1,
            Phase::LineUp
            | Phase::TakeoffRoll
            | Phase::Approach
            | Phase::Final
            | Phase::Rollout => true,
            _ => false,
        }
    }

    /// Landing point: the airport's landing anchor, or a quarter of the way
    /// down the chosen end.
    pub fn landing_point(&self) -> [f64; 3] {
        match self.runway.anchors {
            Some(anchors) => anchors.landing_point,
            None => self.point(
                self.runway.threshold(self.end),
                self.runway.length_ft * AIM_POINT_FRACTION,
                0.0,
            ),
        }
    }

    fn landing_course(&self) -> f64 {
        self.runway
            .anchors
            .map_or(self.runway.heading_from(self.end), |a| a.landing_heading)
    }

    fn takeoff_course(&self) -> f64 {
        self.runway
            .anchors
            .map_or(self.runway.heading_from(self.end), |a| a.takeoff_heading)
    }

    fn course(&self) -> f64 {
        if self.is_departure() {
            self.takeoff_course()
        } else {
            self.landing_course()
        }
    }

    /// Approach gate `k` (0..3): on the 6 degree path back from the landing point.
    pub fn gate(&self, k: usize) -> [f64; 3] {
        let distance = APPROACH_GATES_FT[k.min(2)];
        let mut p = self.point(self.landing_point(), -distance, 0.0);
        p[1] = self.landing_point()[1] + distance * APPROACH_PATH_DEG.to_radians().tan();
        p
    }

    /// Marshal corner for a position: by the aircraft's octant around the
    /// landing point, so the hold circulates +Z, +X, -Z, -X.
    pub fn marshal_corner(&self, position: [f64; 3], wing_position: u8) -> [f64; 3] {
        let center = self.landing_point();
        let (dx, dz) = (position[0] - center[0], position[2] - center[2]);
        let (sx, sz) = if dx.hypot(dz) <= MARSHAL_INNER_FT {
            (1.0, 1.0)
        } else {
            match dx.atan2(dz).to_degrees().rem_euclid(360.0) {
                b if !(45.0..315.0).contains(&b) => (1.0, 1.0),
                b if b < 135.0 => (1.0, -1.0),
                b if b < 225.0 => (-1.0, -1.0),
                _ => (-1.0, 1.0),
            }
        };
        let altitude = (MARSHAL_ALTITUDE_FT + MARSHAL_STEP_FT * f64::from(wing_position))
            .max(center[1] + MARSHAL_FLOOR_FT);
        [
            center[0] + sx * MARSHAL_DISTANCE_FT,
            altitude,
            center[2] + sz * MARSHAL_DISTANCE_FT,
        ]
    }

    /// Along-course and right-of-course distances from `origin`.
    fn frame(&self, position: [f64; 3], origin: [f64; 3]) -> (f64, f64) {
        let h = self.course();
        let (dx, dz) = (position[0] - origin[0], position[2] - origin[2]);
        (dx * h.sin() + dz * h.cos(), dx * h.cos() - dz * h.sin())
    }

    fn point(&self, origin: [f64; 3], along: f64, cross: f64) -> [f64; 3] {
        let h = self.course();
        [
            origin[0] + h.sin() * along + h.cos() * cross,
            origin[1],
            origin[2] + h.cos() * along - h.sin() * cross,
        ]
    }

    fn enter(&mut self, phase: Phase, s: &Situation) {
        self.phase = phase;
        self.leg = 0;
        self.leg_origin = s.position;
        self.stopping = false;
        self.timer = None;
        self.next_check = s.tick;
    }

    fn next_leg(&mut self, s: &Situation) {
        self.leg += 1;
        self.leg_origin = s.position;
        self.stopping = false;
    }

    /// Advance one tick.
    pub fn step(&mut self, s: &Situation) -> Step {
        let height = s.position[1] - self.runway.elevation_ft - s.ground_clearance_ft;
        let mut complete = false;
        let mut go_around = false;
        let stopped = horizontal(s.velocity) < STOPPED_FPS;
        let due = s.tick >= self.next_check;
        if due {
            self.next_check = s.tick + recheck_ticks(GATE_RECHECK_S);
        }
        let to_spot = horizontal_distance(s.position, self.spot);
        let aborting = std::mem::take(&mut self.abort);
        if let Some(start) = self.climb_away
            && (height >= GO_AROUND_CLIMB_FT || seconds(s.tick - start) >= GO_AROUND_CLIMB_S)
        {
            self.climb_away = None;
        }
        // Phase transitions first, so the command matches the new phase.
        match self.phase {
            Phase::Waiting => {
                let anchored = self.runway.anchors.is_some();
                let near = anchored && to_spot <= SPOT_SHORTCUT_FT;
                if due && s.turn_clear && (near || s.runway_free) {
                    self.enter(
                        if anchored && !near {
                            Phase::Taxi
                        } else {
                            Phase::LineUp
                        },
                        s,
                    );
                }
            }
            Phase::Taxi => {
                let anchors = self.runway.anchors.expect("taxi legs need anchors");
                if to_spot <= SPOT_SHORTCUT_FT || self.leg > 3 {
                    self.enter(Phase::LineUp, s);
                } else if self.stopping {
                    if stopped {
                        self.next_leg(s);
                    }
                } else {
                    let anchor = anchors.taxi_out[self.leg];
                    if self.leg == 0 {
                        let leg = sub(anchor, self.leg_origin);
                        if dot2(sub(s.position, anchor), leg) >= 0.0
                            || horizontal_distance(s.position, anchor) < ANCHOR_REACHED_FT
                        {
                            // Retail stops dead at anchor 0.
                            self.stopping = true;
                        }
                    } else {
                        let back = self.point(anchor, -LEG_LINE_LENGTH_FT, 0.0);
                        if segment_distance(s.position, anchor, back) <= LEG_LINE_TOLERANCE_FT {
                            if self.leg == 3 {
                                self.enter(Phase::LineUp, s);
                            } else {
                                self.next_leg(s);
                            }
                        }
                    }
                }
            }
            Phase::LineUp => {
                if to_spot > LINEUP_RESET_FT {
                    self.enter(Phase::Waiting, s);
                } else {
                    let (along, _) = self.frame(s.position, self.spot);
                    if -along <= SPOT_STOP_FT {
                        self.stopping = true;
                    }
                    if self.stopping && stopped {
                        self.enter(Phase::TakeoffRoll, s);
                    }
                }
            }
            Phase::TakeoffRoll => {
                if s.speed_fps >= s.minimum_speed_fps {
                    self.timer.get_or_insert(s.tick);
                }
                if !s.on_ground {
                    self.liftoff_tick = Some(s.tick);
                    self.enter(Phase::ClimbOut, s);
                }
            }
            Phase::ClimbOut => {
                let since = seconds(s.tick - self.liftoff_tick.unwrap_or(s.tick));
                if s.agl_ft >= CLIMB_OUT_COMPLETE_AGL_FT || since >= CLIMB_OUT_TIMEOUT_S {
                    complete = true;
                    self.flaps_up = true;
                    self.slot = None;
                }
            }
            Phase::Inbound => {
                if horizontal_distance(s.position, self.landing_point())
                    <= super::route::LANDING_HANDOFF_FT
                {
                    self.enter(Phase::Marshal, s);
                }
            }
            Phase::Marshal => {
                if aborting && self.climb_away.is_none() {
                    self.climb_away = Some(s.tick);
                }
                if due && self.climb_away.is_none() {
                    self.marshal_target = Some(self.marshal_corner(s.position, s.wing_position));
                    let slot = self.slot.or(s.free_slot);
                    if s.runway_free && s.wing_landed && slot.is_some() {
                        self.slot = slot;
                        self.enter(Phase::Approach, s);
                    }
                }
            }
            Phase::Approach => {
                let gate = self.gate(self.leg);
                let distance = horizontal_distance(s.position, gate);
                let heading = s.heading_deg.to_radians();
                let ahead = (gate[0] - s.position[0]) * heading.sin()
                    + (gate[2] - s.position[2]) * heading.cos();
                if aborting {
                    go_around = true;
                } else if distance <= GATE_CAPTURE_FT
                    || (ahead <= 0.0 && distance <= GATE_PASSED_FT)
                {
                    if self.leg == 2 {
                        self.enter(Phase::Final, s);
                    } else {
                        self.next_leg(s);
                    }
                }
            }
            Phase::Final => {
                if s.on_ground {
                    self.enter(Phase::Rollout, s);
                } else if aborting || self.abandon_final(s) {
                    go_around = true;
                }
            }
            Phase::Rollout => {
                if !s.on_ground && height > BOUNCE_HEIGHT_FT {
                    go_around = true;
                } else if s.on_ground && horizontal(s.velocity) < ROLLOUT_STOPPED_FPS {
                    self.park = Some(self.park_point(s));
                    self.enter(Phase::TaxiClear, s);
                }
            }
            Phase::TaxiClear => {
                if self.stopping {
                    if stopped {
                        self.next_leg(s);
                    }
                } else {
                    let target = self.taxi_in_target();
                    let reached = if self.parking_leg() {
                        horizontal_distance(s.position, target) <= PARK_THROTTLE_OFF_FT && stopped
                    } else {
                        horizontal_distance(s.position, target) <= ANCHOR_REACHED_FT
                            || dot2(sub(s.position, target), sub(target, self.leg_origin)) >= 0.0
                    };
                    if reached {
                        if self.parking_leg() {
                            self.enter(Phase::Parked, s);
                        } else {
                            // Retail stops at each taxi-back anchor.
                            self.stopping = true;
                        }
                    }
                }
            }
            Phase::Parked => {}
        }
        if go_around {
            self.go_arounds += 1;
            self.flaps_up = true;
            self.enter(Phase::Marshal, s);
            self.climb_away = Some(s.tick);
        }
        let command = self.command(s, height, complete);
        Step {
            command,
            complete,
            go_around,
        }
    }

    fn abandon_final(&self, s: &Situation) -> bool {
        let (along, cross) = self.frame(s.position, self.landing_point());
        if along > self.runway.length_ft * GO_AROUND_OVERRUN_FRACTION {
            return true;
        }
        (-GO_AROUND_GATE_FT..0.0).contains(&along)
            && (cross.abs() > GO_AROUND_CROSS_TRACK_FT
                || wrap_deg(self.course().to_degrees() - s.heading_deg).abs()
                    > GO_AROUND_HEADING_DEG)
    }

    /// Whether the current taxi-clear leg is the one into the parking spot.
    fn parking_leg(&self) -> bool {
        self.runway.anchors.is_none() || self.leg >= 4
    }

    fn taxi_in_target(&self) -> [f64; 3] {
        match self.runway.anchors {
            Some(anchors) if self.leg < 4 => anchors.taxi_in[self.leg],
            Some(anchors) => {
                anchors.parking[self.slot.unwrap_or(0).min(PARKING_SLOTS - 1) as usize]
            }
            None => self.park.unwrap_or(self.spot),
        }
    }

    /// Fitted parking spot without anchors: toward the far end, one spot per
    /// slot, alternating sides of the centerline.
    fn park_point(&self, s: &Situation) -> [f64; 3] {
        let threshold = self.runway.threshold(self.end);
        let (along, _) = self.frame(s.position, threshold);
        let slot = self.slot.unwrap_or(0);
        let spot =
            (self.runway.length_ft - PARK_FROM_FAR_END_FT - f64::from(slot) * PARK_SPACING_FT)
                .max(along + PARK_MINIMUM_ROLL_FT)
                .min(self.runway.length_ft - PARK_END_MARGIN_FT);
        let side = if slot.is_multiple_of(2) { 1.0 } else { -1.0 };
        self.point(threshold, spot, side * PARK_OFFSET_FT)
    }

    fn command(&mut self, s: &Situation, height: f64, complete: bool) -> Command {
        let course = self.course().to_degrees();
        let ground = |throttle: f64, pitch: f64, yaw: f64| Control::Ground {
            throttle,
            pitch,
            yaw,
        };
        let mut command = Command {
            control: ground(0.0, 0.0, 0.0),
            gear_down: true,
            flaps_down: !self.flaps_up,
            brakes: true,
            afterburner: false,
            activity: self.activity(),
        };
        match self.phase {
            Phase::Waiting | Phase::Parked => {}
            Phase::Taxi | Phase::TaxiClear => {
                let target = if self.phase == Phase::Taxi {
                    self.runway
                        .anchors
                        .map_or(self.spot, |a| a.taxi_out[self.leg.min(3)])
                } else {
                    self.taxi_in_target()
                };
                let heading = bearing_deg(s.position, target);
                let error = wrap_deg(heading - s.heading_deg).abs();
                let parking_close = self.phase == Phase::TaxiClear
                    && self.parking_leg()
                    && horizontal_distance(s.position, target) <= PARK_THROTTLE_OFF_FT;
                if self.stopping || parking_close {
                    command.control = ground(0.0, 0.0, ground_yaw(s, heading));
                } else {
                    let speed = taxi_speed(error);
                    let (throttle, brakes) = taxi_throttle(horizontal(s.velocity), speed);
                    command.control = ground(throttle, 0.0, ground_yaw(s, heading));
                    command.brakes = brakes;
                }
                if self.phase == Phase::TaxiClear {
                    command.flaps_down = false;
                }
            }
            Phase::LineUp => {
                let (along, cross) = self.frame(s.position, self.spot);
                let remaining = -along;
                let (heading, speed) = if cross.abs() <= CENTERLINE_TOLERANCE_FT {
                    // On the centerline: straight to the spot, slowing to stop.
                    let heading = course
                        + (-cross)
                            .atan2(remaining.max(TAXI_LOOKAHEAD_FT))
                            .to_degrees();
                    let speed = (remaining * 0.5)
                        .clamp(5.0, TAXI_SPEED_FPS)
                        .min(taxi_speed(wrap_deg(heading - s.heading_deg).abs()));
                    (heading, speed)
                } else {
                    let distance = horizontal_distance(s.position, self.spot);
                    let aim = self.point(self.spot, -(distance - LINEUP_AIM_STEP_FT).max(0.0), 0.0);
                    let heading = bearing_deg(s.position, aim);
                    (heading, taxi_speed(wrap_deg(heading - s.heading_deg).abs()))
                };
                if self.stopping {
                    command.control = ground(0.0, 0.0, ground_yaw(s, heading));
                } else {
                    let (throttle, brakes) = taxi_throttle(horizontal(s.velocity), speed);
                    command.control = ground(throttle, 0.0, ground_yaw(s, heading));
                    command.brakes = brakes;
                }
            }
            Phase::TakeoffRoll => {
                let rotating = self
                    .timer
                    .map(|t| seconds(s.tick - t) - ROLL_FULL_POWER_S)
                    .filter(|t| *t >= 0.0);
                let requested = match rotating {
                    Some(t) if t < ROTATION_S => ROTATION_NOSE_DEG,
                    Some(_) => LATE_ROTATION_NOSE_DEG,
                    None => 0.0,
                };
                // B44: no pitch-up on the ground below minimum speed.
                let target = super::steering::ground_pitch_override(
                    0.0,
                    requested,
                    super::ScalarSpeed(s.speed_fps),
                    super::ScalarSpeed(s.minimum_speed_fps),
                    false,
                )
                .unwrap_or(0.0);
                let pitch = if target > 0.0 {
                    ((target - s.body_pitch_deg) * 0.15 + target / 16.0).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (along, cross) = self.frame(s.position, self.spot);
                let heading = course + (-cross).atan2(400.0).to_degrees().clamp(-5.0, 5.0);
                command.control = ground(1.0, pitch, ground_yaw(s, heading));
                command.brakes = false;
                command.flaps_down = true;
                command.afterburner = s.has_afterburner
                    && along > self.runway.length_ft * BURNER_RUNWAY_FRACTION
                    && s.speed_fps < s.minimum_speed_fps;
            }
            Phase::ClimbOut => {
                command.control = Control::Air(AirGuidance {
                    heading_deg: course,
                    flight_path_pitch_deg: CLIMB_OUT_PITCH_DEG,
                    speed_fps: s.maximum_speed_fps,
                    wings_level: height < WINGS_LEVEL_HEIGHT_FT,
                    terrain_floor: false,
                    full_power: true,
                });
                command.brakes = false;
                command.gear_down = !complete;
                command.flaps_down = !complete;
                command.afterburner = s.has_afterburner && s.speed_fps < s.minimum_speed_fps;
            }
            Phase::Inbound => {
                let (target, speed) = self.inbound_target(s);
                command.control = Control::Air(fly_to(s, target, speed));
                command.gear_down = false;
                command.flaps_down = false;
                command.brakes = false;
            }
            Phase::Marshal if self.climb_away.is_some() => {
                // Go-around: full power, wings free, on the landing course.
                command.control = Control::Air(AirGuidance {
                    heading_deg: course,
                    flight_path_pitch_deg: if s.speed_fps < s.minimum_speed_fps {
                        0.0
                    } else {
                        GO_AROUND_PITCH_DEG
                    },
                    speed_fps: s.maximum_speed_fps,
                    wings_level: false,
                    terrain_floor: true,
                    full_power: true,
                });
                command.gear_down = height < GO_AROUND_GEAR_UP_FT;
                command.flaps_down = s.speed_fps < FLAPS_SPEED_FPS;
                command.brakes = false;
            }
            Phase::Marshal => {
                let target = self
                    .marshal_target
                    .unwrap_or_else(|| self.marshal_corner(s.position, s.wing_position));
                command.control = Control::Air(fly_to(s, target, marshal_speed(s)));
                command.gear_down = false;
                command.flaps_down = false;
            }
            Phase::Approach => {
                let speed = if self.leg == 0 {
                    marshal_speed(s)
                } else {
                    marshal_speed(s).min(GATE_SPEED_LIMIT_FPS)
                };
                command.control = Control::Air(fly_to(s, self.gate(self.leg), speed));
                self.flaps_up = self.flaps_up && s.speed_fps >= FLAPS_SPEED_FPS;
                command.flaps_down = !self.flaps_up;
            }
            Phase::Final => {
                let (along, cross) = self.frame(s.position, self.landing_point());
                let to_aim = -along;
                let path = to_aim.max(0.0) * APPROACH_PATH_DEG.to_radians().tan();
                let mut pitch = -APPROACH_PATH_DEG
                    + ((path - height) * GLIDE_CORRECTION_DEG_PER_FT)
                        .clamp(-GLIDE_CORRECTION_LIMIT_DEG, GLIDE_CORRECTION_LIMIT_DEG);
                // Begin easing at a height proportional to descent speed,
                // before a fast final consumes the pitch response time.
                // Keep at least the fitted 1.5 degree descent to the wheels.
                let flare = (-height / (FLARE_SETTLE_S * s.speed_fps.max(1.0)))
                    .atan()
                    .to_degrees()
                    .min(FLARE_PITCH_DEG);
                pitch = pitch.max(flare);
                let lookahead =
                    (to_aim * 0.25).clamp(FINAL_LOOKAHEAD_MIN_FT, FINAL_LOOKAHEAD_MAX_FT);
                let h = self.course();
                let crosswind = s.wind[0] * h.cos() - s.wind[2] * h.sin();
                let correction = -(crosswind / s.speed_fps.max(1.0)).clamp(-0.5, 0.5).asin();
                let heading = course
                    + (-cross)
                        .atan2(lookahead)
                        .to_degrees()
                        .clamp(-FINAL_INTERCEPT_LIMIT_DEG, FINAL_INTERCEPT_LIMIT_DEG)
                    + correction.to_degrees();
                self.flaps_up = self.flaps_up && s.speed_fps >= FLAPS_SPEED_FPS;
                command.control = Control::Air(AirGuidance {
                    heading_deg: heading,
                    flight_path_pitch_deg: pitch,
                    // Fitted: the throttle closes in the flare so the
                    // aircraft settles instead of floating on its speed hold.
                    speed_fps: if height < FLARE_HEIGHT_FT {
                        s.minimum_speed_fps
                    } else {
                        final_speed(s)
                    },
                    wings_level: height < WINGS_LEVEL_HEIGHT_FT,
                    terrain_floor: false,
                    full_power: false,
                });
                command.flaps_down = !self.flaps_up;
            }
            Phase::Rollout => {
                let (_, cross) = self.frame(s.position, self.landing_point());
                let heading = course + (-cross).atan2(500.0).to_degrees().clamp(-10.0, 10.0);
                command.control = ground(0.0, ROLLOUT_ELEVATOR, ground_yaw(s, heading));
                command.flaps_down = true;
            }
        }
        if let Control::Air(guidance) = &mut command.control
            && matches!(
                self.phase,
                Phase::ClimbOut | Phase::Inbound | Phase::Marshal | Phase::Approach
            )
            && s.terrain_ahead_ft > self.landing_point()[1] + TERRAIN_IGNORED_FT
        {
            let clearance = s.position[1] - s.terrain_ahead_ft;
            if clearance < TERRAIN_HOLD_FT {
                // Preserve lift while clearing a ridge. A steep turn toward
                // the next marshal corner could otherwise undo the climb.
                guidance.wings_level = true;
                guidance.heading_deg = s.heading_deg;
            }
            if clearance < TERRAIN_CLIMB_FT {
                guidance.flight_path_pitch_deg =
                    guidance.flight_path_pitch_deg.max(TERRAIN_CLIMB_DEG);
                guidance.full_power = true;
            } else if clearance < TERRAIN_HOLD_FT {
                guidance.flight_path_pitch_deg = guidance.flight_path_pitch_deg.max(0.0);
            }
        }
        // Not in the flare: a speedbrake there makes the aircraft balloon.
        let flaring = self.phase == Phase::Final
            && height
                < (s.speed_fps * FLARE_SETTLE_S * GO_AROUND_DESCENT_DEG.to_radians().tan())
                    .max(FLARE_HEIGHT_FT);
        if let Control::Air(guidance) = command.control
            && matches!(self.phase, Phase::Marshal | Phase::Approach | Phase::Final)
        {
            command.brakes = !guidance.full_power
                && !flaring
                && s.speed_fps > guidance.speed_fps + SPEEDBRAKE_MARGIN_FPS;
        }
        command
    }

    /// The private route leg: toward the airport at the route's altitude and
    /// speed, through the B48 route command.
    fn inbound_target(&self, s: &Situation) -> ([f64; 3], f64) {
        let target = self.landing_point();
        let distance = horizontal_distance(s.position, target);
        let (altitude, speed) = self.route.unwrap_or((target[1], s.cruise_speed_fps));
        let command = super::route::route_command(
            &super::route::RouteInputs {
                waypoint: Some(super::route::Waypoint {
                    altitude_ft: altitude,
                    speed: super::ScalarSpeed(speed),
                    landing: true,
                    distance_ft: distance,
                }),
                has_airport: true,
                // The private route carries no leader altitude jitter.
                wing_leader: false,
                limits: super::SpeedLimits {
                    minimum: super::ScalarSpeed(s.minimum_speed_fps),
                    maximum: super::ScalarSpeed(s.maximum_speed_fps.max(s.minimum_speed_fps)),
                    corner: super::ScalarSpeed(s.corner_speed_fps),
                },
            },
            &mut super::DecisionRandom::seeded(0),
        );
        match command {
            Ok(super::route::RouteCommand::Fly {
                altitude_ft, speed, ..
            }) => ([target[0], altitude_ft, target[2]], speed.0),
            _ => ([target[0], altitude, target[2]], speed),
        }
    }

    fn activity(&self) -> Activity {
        match self.phase {
            Phase::Waiting => Activity::Waiting,
            Phase::Taxi | Phase::LineUp | Phase::TaxiClear => Activity::Taxiing,
            Phase::TakeoffRoll | Phase::ClimbOut => Activity::TakingOff,
            Phase::Inbound => Activity::ReturningToBase,
            Phase::Marshal => Activity::HoldingMarshal,
            Phase::Approach | Phase::Final | Phase::Rollout => Activity::Landing,
            Phase::Parked => Activity::Landed,
        }
    }
}

/// Spec-derived marshal and first-gate speed: (maximum + corner) / 2.
fn marshal_speed(s: &Situation) -> f64 {
    (s.maximum_speed_fps + s.corner_speed_fps) / 2.0
}

/// Final speed: the fitted multiple of minimum speed under the retail cap.
fn final_speed(s: &Situation) -> f64 {
    (APPROACH_SPEED_FACTOR * s.minimum_speed_fps).min(FINAL_SPEED_LIMIT_FPS)
}

/// Fitted runway-end choice; see [`Sequence::landing`].
pub fn choose_landing_end(
    runway: &RunwayView,
    position: [f64; 3],
    wind: [f64; 3],
    max_takeoff_lbs: f64,
) -> ApproachEnd {
    let headwind = |end: ApproachEnd| {
        crate::runway_wind::assessment(max_takeoff_lbs, wind, runway.heading_from(end))
            .map_or(0.0, |a| a.headwind_knots - a.tailwind_knots)
    };
    let (near, far) = (headwind(ApproachEnd::Near), headwind(ApproachEnd::Far));
    if (near - far).abs() >= 1.0 {
        return if near > far {
            ApproachEnd::Near
        } else {
            ApproachEnd::Far
        };
    }
    // Arrival side: an aircraft behind the near threshold lands toward the
    // runway heading.
    let along = (position[0] - runway.center[0]) * runway.heading.sin()
        + (position[2] - runway.center[2]) * runway.heading.cos();
    if along <= 0.0 {
        ApproachEnd::Near
    } else {
        ApproachEnd::Far
    }
}

/// Fly toward a point: heading straight at it, flight path toward its
/// altitude over at least 5000 ft, within 10 degrees (fitted).
fn fly_to(s: &Situation, target: [f64; 3], speed: f64) -> AirGuidance {
    let dx = target[0] - s.position[0];
    let dz = target[2] - s.position[2];
    AirGuidance {
        heading_deg: dx.atan2(dz).to_degrees(),
        flight_path_pitch_deg: (target[1] - s.position[1])
            .atan2(dx.hypot(dz).max(5000.0))
            .to_degrees()
            .clamp(-10.0, 10.0),
        speed_fps: speed,
        wings_level: false,
        terrain_floor: true,
        full_power: false,
    }
}

/// Spec-derived taxi speed by heading error: 50 ft/s under 5 degrees,
/// otherwise a quarter of it (see [`SLOW_TAXI_FPS`] for the pivot case).
fn taxi_speed(heading_error_deg: f64) -> f64 {
    if heading_error_deg < ALIGNED_HEADING_DEG {
        TAXI_SPEED_FPS
    } else {
        SLOW_TAXI_FPS
    }
}

/// Fitted taxi speed control: gentle power below the target, brakes well
/// above it.
fn taxi_throttle(speed: f64, target: f64) -> (f64, bool) {
    if speed > target + 8.0 {
        (0.0, true)
    } else {
        ((0.05 + (target - speed) * 0.02).clamp(0.0, 0.4), false)
    }
}

/// Rudder for a ground heading. The requested turn rate is bounded by the
/// B44 ground turn floor (35 deg/s) and inverted through the fitted
/// nosewheel authority.
fn ground_yaw(s: &Situation, heading_deg: f64) -> f64 {
    let error = wrap_deg(heading_deg - s.heading_deg);
    let limit = super::steering::GROUND_TURN_RATE_FLOOR_DEG_PER_S;
    let rate = (error * GROUND_HEADING_GAIN)
        .clamp(-limit, limit)
        .to_radians();
    let authority = NOSEWHEEL_YAW_RATE_RAD_S
        * (s.speed_fps.max(horizontal(s.velocity)) / NOSEWHEEL_FULL_SPEED_FPS).clamp(0.0, 1.0);
    (rate / authority.max(0.05)).clamp(-1.0, 1.0)
}

fn recheck_ticks(seconds: f64) -> u64 {
    (seconds * 120.0).round() as u64
}

fn bearing_deg(from: [f64; 3], to: [f64; 3]) -> f64 {
    (to[0] - from[0]).atan2(to[2] - from[2]).to_degrees()
}

fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn dot2(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[2] * b[2]
}

/// Horizontal distance from `p` to the segment `a`..`b`.
fn segment_distance(p: [f64; 3], a: [f64; 3], b: [f64; 3]) -> f64 {
    let ab = sub(b, a);
    let length = dot2(ab, ab);
    let t = if length > 0.0 {
        (dot2(sub(p, a), ab) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    horizontal_distance(p, [a[0] + ab[0] * t, 0.0, a[2] + ab[2] * t])
}

fn horizontal(v: [f64; 3]) -> f64 {
    v[0].hypot(v[2])
}

fn horizontal_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

fn seconds(ticks: u64) -> f64 {
    ticks as f64 / 120.0
}

fn wrap_deg(value: f64) -> f64 {
    (value + 180.0).rem_euclid(360.0) - 180.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::threat::FlightState;

    fn runway() -> RunwayView {
        RunwayView {
            airport: 1,
            object: 2,
            center: [0., 0., 0.],
            heading: 0.,
            length_ft: 8000.,
            elevation_ft: 0.,
            anchors: None,
        }
    }

    fn landing() -> Sequence {
        Sequence::landing(
            LandingOrder {
                runway: runway(),
                reason: LandingReason::Ordered,
            },
            [0., 5000., -90_000.],
            [0.; 3],
            40_000.,
            None,
        )
    }

    #[test]
    fn phases_map_to_the_b47_flight_states() {
        use Phase::*;
        for phase in [Waiting, Taxi, LineUp, TakeoffRoll, ClimbOut] {
            assert_eq!(phase.flight_state(), FlightState::TakingOff);
        }
        assert_eq!(Inbound.flight_state(), FlightState::Free);
        for phase in [Marshal, Approach] {
            assert_eq!(phase.flight_state(), FlightState::EarlyApproach);
        }
        for phase in [Final, Rollout, TaxiClear, Parked] {
            assert_eq!(phase.flight_state(), FlightState::LateLanding);
        }
    }

    #[test]
    fn landing_end_follows_the_wind_then_the_arrival_side() {
        let r = runway();
        let south = [0., 5000., -50_000.];
        let north = [0., 5000., 50_000.];
        // Calm: land toward the side the aircraft is not on.
        assert_eq!(
            choose_landing_end(&r, south, [0.; 3], 40_000.),
            ApproachEnd::Near
        );
        assert_eq!(
            choose_landing_end(&r, north, [0.; 3], 40_000.),
            ApproachEnd::Far
        );
        // A 15 kt wind blowing toward the south favours landing north.
        let from_north = [0., 0., -15. * KNOT];
        assert_eq!(
            choose_landing_end(&r, north, from_north, 40_000.),
            ApproachEnd::Near
        );
        // Half a knot is not enough to override the arrival side.
        let breath = [0., 0., -0.4 * KNOT];
        assert_eq!(
            choose_landing_end(&r, north, breath, 40_000.),
            ApproachEnd::Far
        );
    }

    #[test]
    fn approach_gates_sit_on_the_retail_six_degree_path() {
        let s = landing();
        let aim = s.landing_point();
        assert_eq!(aim, [0., 0., -2000.]);
        for (k, (distance, height)) in [(35_200., 3_700.), (17_600., 1_850.), (8_800., 925.)]
            .into_iter()
            .enumerate()
        {
            let gate = s.gate(k);
            assert!((gate[2] - (aim[2] - distance)).abs() < 1e-6);
            assert!((gate[1] - height).abs() < 5., "{gate:?}");
        }
    }

    #[test]
    fn marshal_corners_circulate_clockwise_by_octant() {
        let s = landing();
        let aim = s.landing_point();
        let far = 60_000.;
        let corner = |dx: f64, dz: f64| {
            let c = s.marshal_corner([aim[0] + dx, 5000., aim[2] + dz], 2);
            ((c[0] - aim[0]).signum(), (c[2] - aim[2]).signum(), c[1])
        };
        assert_eq!(corner(0., far), (1., 1., 8_000.));
        assert_eq!(corner(far, 0.), (1., -1., 8_000.));
        assert_eq!(corner(0., -far), (-1., -1., 8_000.));
        assert_eq!(corner(-far, 0.), (-1., 1., 8_000.));
        // Inside 26400 ft always the +X+Z corner.
        assert_eq!(corner(-10_000., -10_000.), (1., 1., 8_000.));
    }

    #[test]
    fn runway_and_turn_gates_follow_the_retail_state_lists() {
        let start = GroundStart {
            runway: runway(),
            end: ApproachEnd::Near,
            order: 1,
        };
        let mut s = Sequence::departure(start, [40., 0., -3500.], None);
        // Fallback spot: on the centerline 200 ft ahead.
        assert!((s.spot[0]).abs() < 1e-9 && (s.spot[2] + 3300.).abs() < 1e-9);
        let cases = [
            (Phase::Waiting, 0, true, false),
            (Phase::Taxi, 0, true, true),
            (Phase::Taxi, 1, true, true),
            (Phase::Taxi, 2, false, false),
            (Phase::LineUp, 0, false, true),
            (Phase::TakeoffRoll, 0, false, true),
            (Phase::ClimbOut, 0, false, false),
            (Phase::Marshal, 0, false, false),
            (Phase::Approach, 0, false, true),
            (Phase::Final, 0, false, true),
            (Phase::Rollout, 0, false, true),
            (Phase::TaxiClear, 0, false, false),
            (Phase::Parked, 0, false, false),
        ];
        for (phase, leg, holds, blocks) in cases {
            s.phase = phase;
            s.leg = leg;
            assert_eq!(s.holds_followers(), holds, "{phase:?} {leg}");
            assert_eq!(s.blocks_runway(), blocks, "{phase:?} {leg}");
        }
    }

    #[test]
    fn waiting_rechecks_the_gates_every_five_seconds() {
        let start = GroundStart {
            runway: runway(),
            end: ApproachEnd::Near,
            order: 1,
        };
        let mut s = Sequence::departure(start, [40., 0., -3500.], None);
        let mut situation = Situation {
            tick: 0,
            position: [40., 8., -3500.],
            velocity: [0.; 3],
            heading_deg: 0.,
            body_pitch_deg: 0.,
            speed_fps: 0.,
            on_ground: true,
            agl_ft: 8.,
            terrain_ahead_ft: 0.,
            ground_clearance_ft: 8.,
            minimum_speed_fps: 170.,
            maximum_speed_fps: 1500.,
            corner_speed_fps: 400.,
            cruise_speed_fps: 440.,
            has_afterburner: true,
            wind: [0.; 3],
            wing_position: 1,
            turn_clear: false,
            runway_free: false,
            wing_landed: true,
            free_slot: None,
        };
        let step = s.step(&situation);
        assert_eq!(step.command.activity, Activity::Waiting);
        assert!(step.command.brakes && step.command.gear_down);
        // Cleared one tick later: still waiting until the next 5 s check.
        situation.turn_clear = true;
        situation.runway_free = true;
        for tick in 1..600 {
            situation.tick = tick;
            s.step(&situation);
            assert_eq!(s.phase(), Phase::Waiting, "{tick}");
        }
        situation.tick = 600;
        s.step(&situation);
        assert_eq!(s.phase(), Phase::LineUp);
    }
}
