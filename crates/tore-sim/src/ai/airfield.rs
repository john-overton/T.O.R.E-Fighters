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
    /// [0xa] A vertical type on its spot, 5 s before it lifts off (slice H2).
    VerticalHold,
    /// [0xb] Climbing straight up at 25 ft/s.
    VerticalClimb,
    /// [0xc] A vectoring jet accelerating from its vertical climb into
    /// wingborne flight.
    Transition,
    /// [0x18] A vertical type slowing from the last approach gate to a hover
    /// over the landing point.
    HoverApproach,
    /// [0x18] The vertical descent to the landing point, or onto a parking
    /// slot at the end of an air taxi.
    VerticalDescent,
    /// [0x18] On the ground at the landing point, 5 s before it moves on.
    Settle,
    /// A helicopter or the V-22 air taxiing to its parking slot (the lead's
    /// default, accepted by John on 2026-10-12).
    AirTaxi,
}

impl Phase {
    /// The B47 warning gate reads takeoff, early-approach and late-landing
    /// states from this.
    pub fn flight_state(self) -> super::threat::FlightState {
        use super::threat::FlightState;
        match self {
            Self::Waiting
            | Self::Taxi
            | Self::LineUp
            | Self::TakeoffRoll
            | Self::ClimbOut
            | Self::VerticalHold
            | Self::VerticalClimb
            | Self::Transition => FlightState::TakingOff,
            Self::Inbound => FlightState::Free,
            Self::Marshal | Self::Approach => FlightState::EarlyApproach,
            Self::Final
            | Self::Rollout
            | Self::TaxiClear
            | Self::Parked
            | Self::HoverApproach
            | Self::VerticalDescent
            | Self::Settle
            | Self::AirTaxi => FlightState::LateLanding,
        }
    }
}

/// How an aircraft takes off and lands (slice H2, docs/spec/ai-airfield.md
/// "Vertical takeoff and landing"). Retail sends a helicopter (type flag 8)
/// and a STOVL jet to their own takeoff and landing states.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Lift {
    /// A runway takeoff and landing.
    #[default]
    Conventional,
    /// The helicopters and the V-22: a vertical takeoff from where it
    /// stands, with no taxi, and a vertical landing at the landing point
    /// followed by an air taxi to its parking slot.
    Rotor,
    /// An AV-8 or Yak-141 light enough to hover: it taxis to the takeoff
    /// spot and takes off vertically, lands vertically at the landing point
    /// and taxis to its parking slot.
    Jet,
    /// An AV-8 or Yak-141 too heavy to hover: retail's short takeoff
    /// (nozzles 60, 40 and 20 degrees) and a runway landing.
    ShortJet,
}

impl Lift {
    /// Takes off and lands vertically.
    pub fn vertical(self) -> bool {
        matches!(self, Self::Rotor | Self::Jet)
    }
}

/// Lift controls an airfield phase sets directly, on top of its control:
/// the collective of a helicopter or the V-22 and the nozzle angle of a
/// vectoring jet, degrees from aft. `None` leaves the demand where it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LiftDemand {
    pub collective: Option<f64>,
    pub nozzle_deg: Option<f64>,
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
    /// An autonomous return caused by a live system fault.
    Damage,
}

impl LandingReason {
    /// Whether a later disengage or formation order cancels this landing
    /// while it is still in the early approach (fitted, agent decision
    /// 2026-09-23). Bug out, fuel and damage landings cannot be cancelled.
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
// Vertical takeoff and landing numbers (slice H2).

/// Spec-derived (state 0xa): a vertical type holds 5 s on its spot before
/// it lifts off, and (state 0x18) 5 s on the ground after a vertical
/// landing.
pub const VERTICAL_HOLD_S: f64 = 5.0;
/// Spec-derived (state 0xb): the vertical climb, ft/s, and where it ends
/// above the ground: 500 ft for a helicopter, which then finishes, and
/// 800 ft for a jet, which accelerates.
pub const VERTICAL_CLIMB_FPS: f64 = 25.0;
pub const ROTOR_CLIMB_AGL_FT: f64 = 500.0;
pub const JET_CLIMB_AGL_FT: f64 = 800.0;
/// Fitted: the climb counts as done this close below its height, and ends
/// this long after liftoff in any case.
pub const CLIMB_REACHED_FT: f64 = 25.0;
pub const VERTICAL_CLIMB_TIMEOUT_S: f64 = 60.0;
/// Spec-derived (state 0xc): a jet accelerates to its minimum speed plus
/// 73 ft/s before it is wingborne. Fitted: it gives up after this long.
pub const TRANSITION_MARGIN_FPS: f64 = 73.0;
/// Fitted (agent decision, slice H2): the jet's hover law holds the nozzles
/// at 60 degrees or more, which cannot carry it to wingborne speed, so at
/// this share of its minimum speed the wingborne steering takes over, at
/// full power on a gentle climb, and the nozzles go aft from 60 degrees to
/// none as the speed builds to the end of the transition.
pub const TRANSITION_HANDOVER_SHARE: f64 = 0.6;
pub const TRANSITION_CLIMB_DEG: f64 = 2.0;
pub const TRANSITION_TIMEOUT_S: f64 = 60.0;
/// Spec-derived short takeoff (states 0xd..0x10): a 10 degree climb to
/// 250 ft above the ground, nozzles at 60, 40 and 20 degrees.
pub const SHORT_CLIMB_DEG: f64 = 10.0;
pub const SHORT_CLIMB_AGL_FT: f64 = 250.0;
pub const SHORT_NOZZLES_DEG: [f64; 3] = [60.0, 40.0, 20.0];
/// Fitted: on the short takeoff roll the nozzles come down to their first
/// angle at this share of the minimum speed (retail rolls to the minimum
/// speed with them down already; the hybrid jet would then lift off at a
/// crawl and settle back).
pub const SHORT_NOZZLE_SPEED_SHARE: f64 = 0.7;
/// Fitted (agent decision, 2026-10-12): a jet hovers when its engines hold
/// its weight vertically at no more than this throttle; heavier, it makes
/// a short takeoff or a runway landing (the manual: "fully loaded ... never
/// be able to fully hover").
pub const HOVER_THROTTLE_LIMIT: f64 = 0.9;
/// Spec-derived: nozzles vertical for the hold, aft once wingborne.
pub const HOVER_NOZZLE_DEG: f64 = 90.0;
/// Spec-derived (state 0x13): a helicopter's marshal square has corners
/// 10,000 ft out (inside half that, the +X+Z corner), and it holds at
/// (3 maximum + corner) / 4.
pub const ROTOR_MARSHAL_DISTANCE_FT: f64 = 10_000.0;
pub const ROTOR_MARSHAL_INNER_FT: f64 = 5_000.0;
/// Fitted (agent decision, 2026-10-12): a helicopter holds this high above
/// the airfield, plus [`ROTOR_MARSHAL_STEP_FT`] per wing position. Retail's
/// helicopter marshal height is not recovered; the fighters' 6,000 ft plus
/// 1,000 ft per position would send a helicopter up to 8,000 ft to land.
pub const ROTOR_MARSHAL_AGL_FT: f64 = 2_000.0;
pub const ROTOR_MARSHAL_STEP_FT: f64 = 500.0;
/// Spec-derived (state 0x14 for vertical types): the gates stand 500 ft
/// higher, a fourth gate stands 6,000 ft out, and a helicopter flies them
/// at no more than 146 ft/s (86 kt).
pub const VERTICAL_GATE_RAISE_FT: f64 = 500.0;
pub const FOURTH_GATE_FT: f64 = 6_000.0;
pub const VERTICAL_GATE_SPEED_FPS: f64 = 146.0;
/// Fitted: the gate a helicopter's approach begins at ([`Sequence::first_gate`]).
pub const ROTOR_FIRST_GATE: usize = 2;
/// Fitted: from the last gate a vertical type slows to a hover over the
/// landing point at this deceleration, and starts down within this of it
/// and this slow.
pub const HOVER_DECELERATION_FPS2: f64 = 4.0;
pub const OVER_POINT_FT: f64 = 50.0;
pub const OVER_POINT_FPS: f64 = 10.0;
/// Spec-derived (state 0x18): retail's vertical descent is 50 ft/s.
/// Fitted (agent decision, slice H2): the hybrid physics cannot arrest that
/// near the ground (an AH-64 at full collective makes 1.05 g and touched
/// down at 28 ft/s, an AV-8 at 26 ft/s), so every vertical type comes down
/// at no more than [`JET_DESCENT_FPS`], 20 ft/s, sinks no faster than a
/// twentieth of its wheel height a second, and no slower than 5 ft/s, so it
/// arrives at half the shared touchdown rule's 10 ft/s
/// (docs/spec/touchdown.md).
pub const VERTICAL_DESCENT_FPS: f64 = 50.0;
pub const JET_DESCENT_FPS: f64 = 20.0;
/// Fitted: the descent's sink builds up at this rate, ft/s², from the
/// hover, so the throttle or collective is not chopped at once.
pub const DESCENT_RAMP_FPS2: f64 = 5.0;
pub const DESCENT_SHARE_PER_S: f64 = 0.05;
pub const TOUCHDOWN_SINK_FPS: f64 = 5.0;
/// Fitted: below this wheel height the descent goes on only while the
/// shared touchdown rule would accept a touchdown now; otherwise it holds
/// its height until it would.
pub const TOUCHDOWN_CHECK_FT: f64 = 30.0;
/// Fitted: the descent is set at this wheel height below the ground, so
/// it ends on the wheels.
pub const SET_DOWN_FT: f64 = -10.0;
/// Fitted: the air taxi to parking flies this high, at most this fast
/// (15 kt), and sets down within this of the slot.
pub const AIR_TAXI_AGL_FT: f64 = 20.0;
pub const AIR_TAXI_FPS: f64 = 25.0;
pub const AIR_TAXI_REACHED_FT: f64 = 15.0;
/// Fitted: a vertical type holds its heading toward a target farther than
/// this, and the target's heading closer in.
pub const AIR_TAXI_TURN_FT: f64 = 100.0;
/// Fitted: the hover laws close a vertical climb's or descent's point at
/// no more than this, ft/s.
pub const HOVER_CREEP_FPS: f64 = 10.0;

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
/// Approach: three gates back from the landing point, at 4/6, 2/6 and 1/6
/// of the marshal distance (spec-derived), each done within 250 ft
/// horizontally, on the player's 3 degree ILS glide path angle
/// (opinionated, requested by John on 2026-09-29; retail used 6 degrees,
/// recorded in docs/formats/ai.md).
pub const APPROACH_PATH_DEG: f64 = crate::airport::GLIDE_SLOPE_DEGREES;
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
/// would reject the full-path sink rate at the wheels (6 degrees in retail,
/// 3 degrees since 2026-09-29, about 12 ft/s at 137 kt, eased to the 1.5
/// degree minimum, about 6 ft/s).
pub const FLARE_HEIGHT_FT: f64 = 60.0;
pub const FLARE_PITCH_DEG: f64 = -1.5;
/// Fitted height-to-sink horizon. A fast descent must start easing before
/// the fixed 60 ft throttle-close height to allow the pitch controller to act.
/// 4 s since 2026-09-29 (6 s before): with the weight-scaled stall speeds the
/// finals are faster and 6 s floated a high F/A-18D past the go-around point.
pub const FLARE_SETTLE_S: f64 = 4.0;
/// Fitted (agent decision, 2026-09-29): wheel clearance over the ground
/// short of the threshold below which a final holds level.
pub const FINAL_TERRAIN_CLEARANCE_FT: f64 = 40.0;
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
/// Fitted (agent decision, 2026-09-29): how far below the final path, and
/// down to what height, a final is abandoned for a go-around.
pub const PATH_LOST_FT: f64 = 150.0;
pub const PATH_LOST_FLOOR_FT: f64 = 100.0;
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
/// Fitted (agent decision, slice CL-AP 2026-10-11): approaching an anchor
/// it stops at, a taxiing aircraft holds the speed from which this
/// deceleration stops it there (never below [`SLOW_TAXI_FPS`]). At full
/// taxi speed the stop used to run about 90 ft past the anchor, off the end
/// of a taxiway corner.
pub const STOP_APPROACH_DECEL_FPS2: f64 = 2.0;
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
/// Fitted (agent decision, slice CL-AP 2026-10-11): on the ground a held
/// heading error builds a rudder trim at this rate per degree per second,
/// up to [`GROUND_TRIM_LIMIT`], so a steady crosswind on the takeoff roll,
/// the rollout or the taxiway no longer leaves the aircraft holding a
/// heading 7 degrees off and running onto the grass (a Vladivostok takeoff
/// drifted 158 ft off the centre line). It learns only while rolling and
/// within [`GROUND_TRIM_ERROR_DEG`] of the heading it wants, so a turn onto
/// the next taxiway does not wind it up. The first value, 0.02, still let a
/// Redfor wingman at Burevestnik (KURILE 3) creep 110 ft east of the centre
/// line before the trim built up, so it is 0.06.
pub const GROUND_TRIM_GAIN: f64 = 0.06;
pub const GROUND_TRIM_LIMIT: f64 = 0.6;
pub const GROUND_TRIM_ERROR_DEG: f64 = 15.0;
/// Below this ground speed the rudder trim is held, feet per second.
const GROUND_TRIM_SPEED_FPS: f64 = 5.0;
/// Fitted: taxi pure-pursuit look-ahead.
const TAXI_LOOKAHEAD_FT: f64 = 80.0;
/// Fitted (agent decision, slice CL-AP 2026-10-11, for John's "ensure AI
/// actually taxi on taxiways instead of grass"): taxiing along a leg the
/// aircraft aims at the point this far ahead of it on the line from the
/// leg's start to its anchor, not at the anchor itself. Aiming at an anchor
/// thousands of feet away turned a steady nosewheel bias (a crosswind) into
/// a drift of over 100 ft off the runway or taxiway; along the line it is a
/// few feet. 120 ft keeps the heading loop well damped at taxi speed.
pub const TAXI_LEG_LOOKAHEAD_FT: f64 = 120.0;

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
    /// On the approach, the highest ground on the straight line to the gate
    /// being flown to, feet above sea level (`f64::MIN` elsewhere).
    pub terrain_to_gate_ft: f64,
    /// Slowest speed with the fitted approach G margin in hand (0 when the
    /// model has no low-speed lift ramp).
    pub approach_minimum_fps: f64,
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
    /// A vertical type: the shared touchdown rule
    /// ([`crate::touchdown`]) would accept a touchdown now, at this sink
    /// and ground speed on the ground below (slice H2). Always true for a
    /// conventional aircraft, which never reads it.
    pub touchdown_safe: bool,
    /// A vectoring jet: its engines hold its weight, stores included, in a
    /// hover at no more than [`HOVER_THROTTLE_LIMIT`] (slice H2). Read at
    /// the takeoff spot to choose the vertical or the short takeoff.
    pub hover_capable: bool,
    /// A helicopter or the V-22 flying home past threat rings it knows: the
    /// point it flies to first, world X/Z (slice H2, the H4 follow-up).
    /// `None` flies straight home.
    pub detour: Option<[f64; 2]>,
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
    /// A hover or slow flight (slice H1, for the vertical sequences of
    /// slice H2): flown by the rotor adapter on a helicopter or the V-22
    /// and by the jet-lift law on the AV-8 and Yak-141.
    Hover(super::rotor_adapter::HoverGuidance),
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
    /// Collective and nozzle set directly (vertical types only).
    pub lift: LiftDemand,
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
    /// A taxiway queue starts on this leg instead of returning to parking.
    queue_leg: Option<usize>,
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
    /// Rudder trim learned on the ground ([`GROUND_TRIM_GAIN`]).
    ground_trim: f64,
    /// How it takes off and lands (slice H2).
    lift: Lift,
    /// A vertical phase's held heading, degrees (slice H2).
    hover_heading: f64,
}

impl Sequence {
    /// A ground start at `position`, holding with brakes set until its turn.
    /// `slot` is the parking slot it starts in, when it starts in one.
    pub fn departure(start: GroundStart, position: [f64; 3], slot: Option<u32>) -> Self {
        let mut sequence = Self::new(Kind::Departure, Phase::Waiting, start.runway, start.end);
        sequence.order = start.order;
        sequence.slot = slot;
        if slot.is_none()
            && let Some(anchors) = start.runway.anchors
        {
            sequence.queue_leg = (1..4).rev().find(|&leg| {
                segment_distance(position, anchors.taxi_out[leg - 1], anchors.taxi_out[leg]) <= 1.0
            });
        }
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

    /// Fitted (agent decision, 2026-09-29): a runway without anchors, whose
    /// landing end was chosen by the arrival side because the wind did not
    /// decide, lands on the other end instead when the ground stands above
    /// the 3 degree path to the chosen end and less so to the other. An
    /// anchored airport's landing heading, and a wind choice, stand.
    pub fn prefer_clear_approach(
        &mut self,
        wind: [f64; 3],
        max_takeoff_lbs: f64,
        ground: &dyn Fn(f64, f64) -> f64,
    ) {
        if self.runway.anchors.is_some() || self.is_departure() {
            return;
        }
        let headwind = |end: ApproachEnd| {
            crate::runway_wind::assessment(max_takeoff_lbs, wind, self.runway.heading_from(end))
                .map_or(0.0, |a| a.headwind_knots - a.tailwind_knots)
        };
        if (headwind(ApproachEnd::Near) - headwind(ApproachEnd::Far)).abs() >= 1.0 {
            return;
        }
        let other = match self.end {
            ApproachEnd::Near => ApproachEnd::Far,
            ApproachEnd::Far => ApproachEnd::Near,
        };
        let chosen = path_obstruction_ft(&self.runway, self.end, ground);
        if chosen > 0.0 && path_obstruction_ft(&self.runway, other, ground) < chosen {
            self.end = other;
        }
    }

    fn new(kind: Kind, phase: Phase, runway: RunwayView, end: ApproachEnd) -> Self {
        Self {
            kind,
            phase,
            runway,
            end,
            order: 0,
            leg: 0,
            queue_leg: None,
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
            ground_trim: 0.0,
            lift: Lift::Conventional,
            hover_heading: 0.0,
        }
    }

    /// Take off or land as `lift` says (slice H2). A helicopter's departure
    /// lifts off from where it stands, so it has no taxi queue.
    pub fn with_lift(mut self, lift: Lift) -> Self {
        self.lift = lift;
        if lift == Lift::Rotor && self.is_departure() {
            self.queue_leg = None;
        }
        self
    }

    pub fn lift(&self) -> Lift {
        self.lift
    }

    /// The sequence put straight into `phase` (tests of later phases).
    #[cfg(test)]
    pub(crate) fn at_phase(mut self, phase: Phase, heading_deg: f64) -> Self {
        self.phase = phase;
        self.hover_heading = heading_deg;
        self
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
                // A hover or a slow descent near the ground is the plan, not
                // a hazard (slice H2).
                | Phase::VerticalHold
                | Phase::VerticalClimb
                | Phase::Transition
                | Phase::HoverApproach
                | Phase::VerticalDescent
                | Phase::Settle
                | Phase::AirTaxi
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

    /// Takeoff spot for a departure; the airport's own spot otherwise. A
    /// helicopter lifts off where it stands, so it keeps no spot clear.
    pub fn takeoff_spot(&self) -> Option<[f64; 3]> {
        if self.is_departure() {
            (self.lift != Lift::Rotor).then_some(self.spot)
        } else {
            self.runway.anchors.map(|a| a.takeoff_spot)
        }
    }

    /// Retail turn gate: states 1..3 (holding, taxi to anchor 0, first
    /// taxiway leg) keep later wing members waiting.
    pub fn holds_followers(&self) -> bool {
        match self.phase {
            Phase::Waiting => true,
            Phase::Taxi => self.queue_leg.is_some() || self.leg <= 1,
            _ => false,
        }
    }

    /// Retail runway-free gate: states 2, 3, 6..0x11, 0x14..0x19 and 0x1b
    /// keep the runway busy. Taxiing clear counts as free at once. Of the
    /// vertical states, the hold (0xa) and the vertical landing (0x18,
    /// with its 5 s on the ground) block; the climb (0xb), the
    /// acceleration (0xc) and the air taxi to parking do not.
    pub fn blocks_runway(&self) -> bool {
        match self.phase {
            Phase::Taxi => self.queue_leg.is_some() || self.leg <= 1,
            Phase::LineUp
            | Phase::TakeoffRoll
            | Phase::Approach
            | Phase::Final
            | Phase::Rollout
            | Phase::VerticalHold
            | Phase::HoverApproach
            | Phase::Settle => true,
            Phase::VerticalDescent => self.park.is_none() || self.lift != Lift::Rotor,
            _ => false,
        }
    }

    /// Down at this airport after a landing: on its wheels or air taxiing
    /// to its slot (a later wing member may then come in).
    pub fn landed(&self) -> bool {
        !self.is_departure()
            && (matches!(
                self.phase,
                Phase::Rollout | Phase::TaxiClear | Phase::Parked | Phase::Settle | Phase::AirTaxi
            ) || (self.phase == Phase::VerticalDescent
                && self.lift == Lift::Rotor
                && self.park.is_some()))
    }

    /// Landing point: the player's ILS aim point on the landing end,
    /// [`crate::airport::AIM_PAST_THRESHOLD_FT`] past its threshold on the
    /// runway plane (opinionated, requested by John on 2026-09-29, so AI and
    /// player fly one glide path). With anchors the end is the one facing the
    /// airport's landing heading. Retail aimed at the landing anchor (box
    /// 0x12), or without anchors a quarter of the way down the runway.
    pub fn landing_point(&self) -> [f64; 3] {
        let end = self.landing_end();
        let threshold = self.runway.threshold(end);
        let heading = self.runway.heading_from(end);
        let aim = crate::airport::AIM_PAST_THRESHOLD_FT;
        [
            threshold[0] + heading.sin() * aim,
            self.runway.elevation_ft,
            threshold[2] + heading.cos() * aim,
        ]
    }

    /// The runway end landed on: with anchors, the end whose approach
    /// heading is nearest the airport's landing heading.
    pub fn landing_end(&self) -> ApproachEnd {
        match self.runway.anchors {
            Some(anchors) => {
                let off = |end| {
                    wrap_deg((self.runway.heading_from(end) - anchors.landing_heading).to_degrees())
                        .abs()
                };
                if off(ApproachEnd::Near) <= off(ApproachEnd::Far) {
                    ApproachEnd::Near
                } else {
                    ApproachEnd::Far
                }
            }
            None => self.end,
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

    /// Approach gate `k` (0..3, and 3 for a vertical type's fourth): on the
    /// approach path back from the landing point. A vertical type's gates
    /// stand [`VERTICAL_GATE_RAISE_FT`] higher (retail state 0x14).
    pub fn gate(&self, k: usize) -> [f64; 3] {
        let vertical = self.lift.vertical();
        let distance = if vertical && k >= 3 {
            FOURTH_GATE_FT
        } else {
            APPROACH_GATES_FT[k.min(2)]
        };
        let mut p = self.point(self.landing_point(), -distance, 0.0);
        p[1] = self.landing_point()[1]
            + crate::airport::glide_path_height_ft(
                distance - crate::airport::AIM_PAST_THRESHOLD_FT,
            )
            + if vertical {
                VERTICAL_GATE_RAISE_FT
            } else {
                0.0
            };
        p
    }

    /// How many approach gates this aircraft flies: three, four for a
    /// vertical type.
    pub fn gates(&self) -> usize {
        if self.lift.vertical() { 4 } else { 3 }
    }

    /// The first approach gate flown. Fitted (agent decision, slice H2): a
    /// helicopter flies only the gates inside its 10,000 ft marshal square
    /// (8,800 and 6,000 ft out); flying out to the 35,200 ft gate at 86 kt
    /// and back took nine minutes.
    pub fn first_gate(&self) -> usize {
        if self.lift == Lift::Rotor {
            ROTOR_FIRST_GATE
        } else {
            0
        }
    }

    /// Marshal corner for a position: by the aircraft's octant around the
    /// landing point, so the hold circulates +Z, +X, -Z, -X. A helicopter's
    /// square is the smaller retail one, and lower (fitted).
    pub fn marshal_corner(&self, position: [f64; 3], wing_position: u8) -> [f64; 3] {
        let center = self.landing_point();
        let rotor = self.lift == Lift::Rotor;
        let (distance, inner) = if rotor {
            (ROTOR_MARSHAL_DISTANCE_FT, ROTOR_MARSHAL_INNER_FT)
        } else {
            (MARSHAL_DISTANCE_FT, MARSHAL_INNER_FT)
        };
        let (dx, dz) = (position[0] - center[0], position[2] - center[2]);
        let (sx, sz) = if dx.hypot(dz) <= inner {
            (1.0, 1.0)
        } else {
            match dx.atan2(dz).to_degrees().rem_euclid(360.0) {
                b if !(45.0..315.0).contains(&b) => (1.0, 1.0),
                b if b < 135.0 => (1.0, -1.0),
                b if b < 225.0 => (-1.0, -1.0),
                _ => (-1.0, 1.0),
            }
        };
        let altitude = if rotor {
            center[1] + ROTOR_MARSHAL_AGL_FT + ROTOR_MARSHAL_STEP_FT * f64::from(wing_position)
        } else {
            (MARSHAL_ALTITUDE_FT + MARSHAL_STEP_FT * f64::from(wing_position))
                .max(center[1] + MARSHAL_FLOOR_FT)
        };
        [
            center[0] + sx * distance,
            altitude,
            center[2] + sz * distance,
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
        // Each ground phase learns its own trim: the wind acts on a rolling
        // and a taxiing aircraft differently.
        self.ground_trim = 0.0;
        self.leg = 0;
        self.leg_origin = s.position;
        self.stopping = false;
        self.timer = None;
        self.next_check = s.tick;
    }

    /// Enter a vertical phase (slice H2): its clock starts now, and it
    /// holds `heading_deg`.
    fn enter_vertical(&mut self, phase: Phase, s: &Situation, heading_deg: f64) {
        self.enter(phase, s);
        self.timer = Some(s.tick);
        self.hover_heading = heading_deg;
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
            Phase::Waiting if self.lift == Lift::Rotor => {
                // Retail state 1 to 0xa: a helicopter lifts off where it
                // stands, on its turn with the runway free.
                if due && s.turn_clear && s.runway_free {
                    self.enter_vertical(Phase::VerticalHold, s, s.heading_deg);
                }
            }
            Phase::Waiting => {
                let anchored = self.runway.anchors.is_some();
                let near = anchored && to_spot <= SPOT_SHORTCUT_FT;
                if due && s.turn_clear && ((near && self.queue_leg.is_none()) || s.runway_free) {
                    // Fitted (agent decision, slice CL-AP 2026-10-11): an
                    // aircraft queued on the taxi-out legs taxis them out
                    // even when it waits within the shortcut distance of
                    // the spot. The fourth of a wing waits on the parallel
                    // taxiway abeam the spot, and went straight to the
                    // takeoff roll from there, on the taxiway.
                    self.enter(
                        if anchored && (!near || self.queue_leg.is_some()) {
                            Phase::Taxi
                        } else {
                            Phase::LineUp
                        },
                        s,
                    );
                    if self.phase == Phase::Taxi
                        && let Some(leg) = self.queue_leg
                    {
                        self.leg = leg;
                    }
                }
            }
            Phase::Taxi => {
                let anchors = self.runway.anchors.expect("taxi legs need anchors");
                if (to_spot <= SPOT_SHORTCUT_FT && self.queue_leg.is_none()) || self.leg > 3 {
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
                        // A jet decides at the spot, with its stores on: a
                        // vertical takeoff if it can hover, else retail's
                        // short takeoff.
                        if matches!(self.lift, Lift::Jet | Lift::ShortJet) {
                            self.lift = if s.hover_capable {
                                Lift::Jet
                            } else {
                                Lift::ShortJet
                            };
                        }
                        if self.lift == Lift::Jet {
                            // Retail: a STOVL jet on the takeoff spot
                            // takes off vertically (0xa).
                            let course = self.takeoff_course().to_degrees();
                            self.enter_vertical(Phase::VerticalHold, s, course);
                        } else {
                            self.enter(Phase::TakeoffRoll, s);
                        }
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
                // Retail short takeoff (0xd..0x10): done at its minimum
                // speed plus 73 ft/s once 250 ft up.
                let short_done = self.lift == Lift::ShortJet
                    && s.agl_ft >= SHORT_CLIMB_AGL_FT
                    && s.speed_fps >= s.minimum_speed_fps + TRANSITION_MARGIN_FPS;
                if short_done
                    || s.agl_ft >= CLIMB_OUT_COMPLETE_AGL_FT
                    || since >= CLIMB_OUT_TIMEOUT_S
                {
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
                        self.leg = self.first_gate();
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
                    if self.leg + 1 < self.gates() {
                        self.next_leg(s);
                    } else if self.lift.vertical() {
                        // Retail 0x18: a vertical type comes to a hover.
                        let course = self.landing_course().to_degrees();
                        self.enter_vertical(Phase::HoverApproach, s, course);
                    } else {
                        self.enter(Phase::Final, s);
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
            Phase::VerticalHold
            | Phase::VerticalClimb
            | Phase::Transition
            | Phase::HoverApproach
            | Phase::VerticalDescent
            | Phase::Settle
            | Phase::AirTaxi => complete = self.vertical_transition(s),
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
        if self.path_lost(s) {
            return true;
        }
        (-GO_AROUND_GATE_FT..0.0).contains(&along)
            && (cross.abs() > GO_AROUND_CROSS_TRACK_FT
                || wrap_deg(self.course().to_degrees() - s.heading_deg).abs()
                    > GO_AROUND_HEADING_DEG)
    }

    /// The height the 6 degree final path asks for here, and the wheel
    /// height above the runway.
    fn final_path(&self, s: &Situation) -> (f64, f64) {
        let (along, _) = self.frame(s.position, self.landing_point());
        let height = s.position[1] - self.runway.elevation_ft - s.ground_clearance_ft;
        (
            crate::airport::glide_path_height_ft(-along - crate::airport::AIM_PAST_THRESHOLD_FT),
            height,
        )
    }

    /// Fitted (agent decision, 2026-09-29): a final that has sunk more than
    /// [`PATH_LOST_FT`] below its path while still above
    /// [`PATH_LOST_FLOOR_FT`] goes around while it has height to climb away,
    /// instead of sinking on until the ground is a second away.
    fn path_lost(&self, s: &Situation) -> bool {
        let (path, height) = self.final_path(s);
        height > PATH_LOST_FLOOR_FT && height < path - PATH_LOST_FT
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

    /// Rudder for a ground heading: the proportional rudder plus the trim
    /// a held error has built up ([`GROUND_TRIM_GAIN`]).
    fn steer(&mut self, s: &Situation, heading_deg: f64) -> f64 {
        let error = wrap_deg(heading_deg - s.heading_deg);
        if horizontal(s.velocity) >= GROUND_TRIM_SPEED_FPS && error.abs() < GROUND_TRIM_ERROR_DEG {
            self.ground_trim = (self.ground_trim + GROUND_TRIM_GAIN * error / 120.0)
                .clamp(-GROUND_TRIM_LIMIT, GROUND_TRIM_LIMIT);
        }
        (ground_yaw(s, heading_deg) + self.ground_trim).clamp(-1.0, 1.0)
    }

    /// Where the current taxi leg starts: the previous anchor, or for the
    /// first leg out the start position and for the first leg in the
    /// landing point (so the rollout end is reached along the runway's
    /// centre line). Without anchors, where the leg began.
    fn taxi_leg_start(&self) -> [f64; 3] {
        match (self.phase, self.runway.anchors) {
            (Phase::Taxi, Some(anchors)) if self.leg > 0 => anchors.taxi_out[self.leg.min(4) - 1],
            (Phase::TaxiClear, Some(anchors)) if self.leg > 0 => {
                anchors.taxi_in[self.leg.min(4) - 1]
            }
            (Phase::TaxiClear, Some(_)) => self.landing_point(),
            _ => self.leg_origin,
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
            lift: LiftDemand::default(),
        };
        match self.phase {
            // A helicopter waits with its engines governed and the
            // collective down (the throttle is its engines' power lever);
            // parked for good it shuts down (slice H2).
            Phase::Waiting if self.lift == Lift::Rotor => {
                command.control = ground(1.0, 0.0, 0.0);
                command.lift.collective = Some(0.0);
            }
            Phase::Parked if self.lift == Lift::Rotor => {
                command.lift.collective = Some(0.0);
            }
            Phase::Waiting | Phase::Parked => {}
            Phase::Taxi | Phase::TaxiClear => {
                let target = if self.phase == Phase::Taxi {
                    self.runway
                        .anchors
                        .map_or(self.spot, |a| a.taxi_out[self.leg.min(3)])
                } else {
                    self.taxi_in_target()
                };
                let aim = leg_aim(
                    s.position,
                    self.taxi_leg_start(),
                    target,
                    TAXI_LEG_LOOKAHEAD_FT,
                );
                let heading = bearing_deg(s.position, aim);
                let error = wrap_deg(heading - s.heading_deg).abs();
                let parking_close = self.phase == Phase::TaxiClear
                    && self.parking_leg()
                    && horizontal_distance(s.position, target) <= PARK_THROTTLE_OFF_FT;
                if self.stopping || parking_close {
                    command.control = ground(0.0, 0.0, self.steer(s, heading));
                } else {
                    let mut speed = taxi_speed(error);
                    // Slow for an anchor it stops at (the first one out,
                    // every one in), so it stops there, not 90 ft on.
                    let stops = if self.phase == Phase::Taxi {
                        self.leg == 0
                    } else {
                        !self.parking_leg()
                    };
                    if stops {
                        let d = horizontal_distance(s.position, target);
                        speed = speed.min(
                            (2.0 * STOP_APPROACH_DECEL_FPS2 * d)
                                .sqrt()
                                .max(SLOW_TAXI_FPS),
                        );
                    }
                    let (throttle, brakes) = taxi_throttle(horizontal(s.velocity), speed);
                    command.control = ground(throttle, 0.0, self.steer(s, heading));
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
                    let mut back = (distance - LINEUP_AIM_STEP_FT).max(0.0);
                    // Fitted (agent decision, slice CL-AP 2026-10-11): never
                    // aim farther back than the last taxi anchor, which
                    // stands on the centre line behind the spot where the
                    // taxiway meets it. Aiming as far back as it was away
                    // took a wingman queued on that taxiway across the
                    // grass behind it.
                    if let Some(anchors) = self.runway.anchors {
                        let (along, cross) = self.frame(anchors.taxi_out[3], self.spot);
                        if along < 0.0 && cross.abs() <= CENTERLINE_TOLERANCE_FT {
                            back = back.min(-along);
                        }
                    }
                    let aim = self.point(self.spot, -back, 0.0);
                    let heading = bearing_deg(s.position, aim);
                    (heading, taxi_speed(wrap_deg(heading - s.heading_deg).abs()))
                };
                if self.stopping {
                    command.control = ground(0.0, 0.0, self.steer(s, heading));
                } else {
                    let (throttle, brakes) = taxi_throttle(horizontal(s.velocity), speed);
                    command.control = ground(throttle, 0.0, self.steer(s, heading));
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
                command.control = ground(1.0, pitch, self.steer(s, heading));
                command.brakes = false;
                command.flaps_down = true;
                command.afterburner = s.has_afterburner
                    && along > self.runway.length_ft * BURNER_RUNWAY_FRACTION
                    && s.speed_fps < s.minimum_speed_fps;
                if self.lift == Lift::ShortJet {
                    // Retail short takeoff: the nozzles come down for the
                    // last of the roll (fitted share of the minimum speed).
                    command.lift.nozzle_deg = Some(
                        if s.speed_fps >= SHORT_NOZZLE_SPEED_SHARE * s.minimum_speed_fps {
                            SHORT_NOZZLES_DEG[0]
                        } else {
                            0.0
                        },
                    );
                }
            }
            Phase::ClimbOut => {
                let short = self.lift == Lift::ShortJet;
                // Retail short takeoff: 10 degrees with the nozzles at 40
                // up to 250 ft, then at 20 while it gathers speed.
                let low = short && s.agl_ft < SHORT_CLIMB_AGL_FT;
                command.control = Control::Air(AirGuidance {
                    heading_deg: course,
                    flight_path_pitch_deg: if low {
                        SHORT_CLIMB_DEG
                    } else {
                        CLIMB_OUT_PITCH_DEG
                    },
                    speed_fps: s.maximum_speed_fps,
                    wings_level: height < WINGS_LEVEL_HEIGHT_FT,
                    terrain_floor: false,
                    full_power: true,
                });
                command.brakes = false;
                command.gear_down = !complete;
                command.flaps_down = !complete;
                command.afterburner = s.has_afterburner && s.speed_fps < s.minimum_speed_fps;
                if short {
                    command.lift.nozzle_deg = Some(if complete {
                        0.0
                    } else if low {
                        SHORT_NOZZLES_DEG[1]
                    } else {
                        SHORT_NOZZLES_DEG[2]
                    });
                }
            }
            Phase::Inbound => {
                let (mut target, speed) = self.inbound_target(s);
                // Round the threat rings it knows (slice H2).
                if let Some([x, z]) = s.detour {
                    target = [x, target[1], z];
                }
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
                command.control = Control::Air(fly_to(s, target, self.marshal_speed(s)));
                command.gear_down = false;
                command.flaps_down = false;
            }
            Phase::Approach => {
                let speed = match self.lift {
                    // Retail: a helicopter at most 146 ft/s on the gates.
                    Lift::Rotor => self.marshal_speed(s).min(VERTICAL_GATE_SPEED_FPS),
                    // A jet slows to its final speed for the fourth gate,
                    // where it comes to the hover.
                    Lift::Jet if self.leg >= 3 => final_speed(s),
                    _ if self.leg == 0 => self.marshal_speed(s),
                    _ => self.marshal_speed(s).min(GATE_SPEED_LIMIT_FPS),
                };
                command.control = Control::Air(fly_to(s, self.gate(self.leg), speed));
                self.flaps_up = self.flaps_up && s.speed_fps >= FLAPS_SPEED_FPS;
                command.flaps_down = !self.flaps_up;
            }
            Phase::Final => {
                let (along, cross) = self.frame(s.position, self.landing_point());
                let to_aim = -along;
                // The player's ILS path, in wheel height above the runway.
                let path = crate::airport::glide_path_height_ft(
                    to_aim - crate::airport::AIM_PAST_THRESHOLD_FT,
                );
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
                // Fitted (agent decision, 2026-09-29): short of the threshold,
                // where terrain can stand above the runway (KURILE 3, NSK 6),
                // hold level rather than descend with the wheels closer than
                // FINAL_TERRAIN_CLEARANCE_FT to the ground below.
                if to_aim > crate::airport::AIM_PAST_THRESHOLD_FT
                    && s.agl_ft - s.ground_clearance_ft < FINAL_TERRAIN_CLEARANCE_FT
                {
                    pitch = pitch.max(0.0);
                }
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
                command.control = ground(0.0, ROLLOUT_ELEVATOR, self.steer(s, heading));
                command.flaps_down = true;
            }
            Phase::VerticalHold
            | Phase::VerticalClimb
            | Phase::Transition
            | Phase::HoverApproach
            | Phase::VerticalDescent
            | Phase::Settle
            | Phase::AirTaxi => self.vertical_command(s, complete, &mut command),
        }
        if self.phase == Phase::TaxiClear && self.lift == Lift::Jet {
            // After a vertical landing a jet taxis in with its nozzles aft.
            command.lift.nozzle_deg = Some(0.0);
        }
        // Fitted (agent decision, 2026-09-29): on the approach the ground
        // that counts is the ground on the way to the gate being flown to,
        // and a hold stops the descent without holding the heading, so the
        // aircraft keeps steering for its gate above the terrain and joins
        // the path once past it. On the 3 degree path the lower gates put
        // hills beyond them inside the 12,000 ft look-ahead, and holding the
        // heading flew the aircraft away from its gate for minutes (NSK 6).
        let approach = self.phase == Phase::Approach;
        let terrain = if approach {
            s.terrain_to_gate_ft
        } else {
            s.terrain_ahead_ft
        };
        if let Control::Air(guidance) = &mut command.control
            && matches!(
                self.phase,
                Phase::ClimbOut | Phase::Inbound | Phase::Marshal | Phase::Approach
            )
            && terrain > self.landing_point()[1] + TERRAIN_IGNORED_FT
        {
            let clearance = s.position[1] - terrain;
            if clearance < TERRAIN_CLIMB_FT || (!approach && clearance < TERRAIN_HOLD_FT) {
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
            // Fitted (agent decision, 2026-09-29): never with the aircraft
            // below its final path; the drag then only deepens the sink.
            let below_path = self.phase == Phase::Final && {
                let (path, height) = self.final_path(s);
                height < path
            };
            command.brakes = !guidance.full_power
                && !flaring
                && !below_path
                && s.speed_fps > guidance.speed_fps + SPEEDBRAKE_MARGIN_FPS;
        }
        command
    }

    /// The private route leg: toward the airport at the route's altitude and
    /// speed, through the B48 route command.
    fn inbound_target(&self, s: &Situation) -> ([f64; 3], f64) {
        let target = self.landing_point();
        let distance = horizontal_distance(s.position, target);
        let (altitude, speed) = match self.route {
            // A helicopter goes home at its own cruise and at its marshal
            // height over the field, not on the fighters' route (fitted,
            // slice H2; the cruise is slice H3's).
            _ if self.lift == Lift::Rotor => (target[1] + ROTOR_MARSHAL_AGL_FT, s.cruise_speed_fps),
            Some(route) => route,
            None => (target[1], s.cruise_speed_fps),
        };
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
            Phase::Taxi | Phase::LineUp | Phase::TaxiClear | Phase::AirTaxi => Activity::Taxiing,
            Phase::TakeoffRoll
            | Phase::ClimbOut
            | Phase::VerticalHold
            | Phase::VerticalClimb
            | Phase::Transition => Activity::TakingOff,
            Phase::Inbound => Activity::ReturningToBase,
            Phase::Marshal => Activity::HoldingMarshal,
            Phase::Approach
            | Phase::Final
            | Phase::Rollout
            | Phase::HoverApproach
            | Phase::VerticalDescent
            | Phase::Settle => Activity::Landing,
            Phase::Parked => Activity::Landed,
        }
    }

    /// Spec-derived marshal and first-gate speed: (maximum + corner) / 2;
    /// a helicopter's (3 maximum + corner) / 4.
    fn marshal_speed(&self, s: &Situation) -> f64 {
        if self.lift == Lift::Rotor {
            (3.0 * s.maximum_speed_fps + s.corner_speed_fps) / 4.0
        } else {
            (s.maximum_speed_fps + s.corner_speed_fps) / 2.0
        }
    }
}

/// Final speed: the fitted multiple of minimum speed under the retail cap.
fn final_speed(s: &Situation) -> f64 {
    (APPROACH_SPEED_FACTOR * s.minimum_speed_fps)
        .max(s.approach_minimum_fps)
        .min(FINAL_SPEED_LIMIT_FPS)
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

/// How far, in feet, the ground stands above the 3 degree approach path
/// to `end` anywhere from the aim point out to the first gate (negative
/// when the path is clear by that much everywhere).
pub fn path_obstruction_ft(
    runway: &RunwayView,
    end: ApproachEnd,
    ground: &dyn Fn(f64, f64) -> f64,
) -> f64 {
    let threshold = runway.threshold(end);
    let heading = runway.heading_from(end);
    let (sin, cos) = (heading.sin(), heading.cos());
    let aim = crate::airport::AIM_PAST_THRESHOLD_FT;
    let mut worst = f64::MIN;
    let mut out = 0.0;
    while out <= APPROACH_GATES_FT[0] {
        // `out` feet before the aim point, along the approach.
        let back = out - aim;
        let (x, z) = (threshold[0] - sin * back, threshold[2] - cos * back);
        let path = runway.elevation_ft + crate::airport::glide_path_height_ft(back);
        worst = worst.max(ground(x, z) - path);
        out += 1_000.0;
    }
    worst
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

/// The point `lookahead` feet ahead of `position`'s projection on the line
/// from `from` to `to`, never past `to` (see [`TAXI_LEG_LOOKAHEAD_FT`]).
fn leg_aim(position: [f64; 3], from: [f64; 3], to: [f64; 3], lookahead: f64) -> [f64; 3] {
    let leg = sub(to, from);
    let length = horizontal(leg);
    if length < 1.0 {
        return to;
    }
    let along = (dot2(sub(position, from), leg) / length).clamp(0.0, length);
    let at = ((along + lookahead) / length).min(1.0);
    [from[0] + leg[0] * at, to[1], from[2] + leg[2] * at]
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

    /// On the final course `out_ft` before the landing point, `height_ft`
    /// above the runway, at `speed_fps`.
    fn on_final(seq: &Sequence, out_ft: f64, height_ft: f64, speed_fps: f64) -> Situation {
        let lp = seq.landing_point();
        let course = seq.course();
        let position = [
            lp[0] - out_ft * course.sin(),
            height_ft + 8.,
            lp[2] - out_ft * course.cos(),
        ];
        Situation {
            tick: 1,
            position,
            velocity: [speed_fps * course.sin(), 0., speed_fps * course.cos()],
            heading_deg: course.to_degrees(),
            body_pitch_deg: 0.,
            speed_fps,
            on_ground: false,
            agl_ft: height_ft,
            terrain_ahead_ft: 0.,
            ground_clearance_ft: 8.,
            minimum_speed_fps: 150.,
            terrain_to_gate_ft: f64::MIN,
            approach_minimum_fps: 230.,
            maximum_speed_fps: 1500.,
            corner_speed_fps: 400.,
            cruise_speed_fps: 440.,
            has_afterburner: true,
            wind: [0.; 3],
            wing_position: 0,
            turn_clear: true,
            runway_free: true,
            wing_landed: true,
            free_slot: Some(0),
            touchdown_safe: true,
            hover_capable: true,
            detour: None,
        }
    }

    #[test]
    fn a_final_below_its_path_goes_around_early_and_never_holds_the_speedbrake() {
        // John, 2026-09-29: AI finals hold their path (fitted numbers).
        let path = |out: f64| out * APPROACH_PATH_DEG.to_radians().tan();
        let final_step = |height: f64, speed: f64| {
            let mut seq = landing();
            seq.phase = Phase::Final;
            let step = seq.step(&on_final(&seq, 8000., height, speed));
            (step, seq.phase())
        };
        // Fast and above the path: the speedbrake opens and the final goes on.
        let (step, phase) = final_step(path(8000.) + 100., 400.);
        assert!(step.command.brakes && !step.go_around);
        assert_eq!(phase, Phase::Final);
        // Fast but below the path: no speedbrake.
        let (step, _) = final_step(path(8000.) - 50., 400.);
        assert!(!step.command.brakes && !step.go_around);
        // More than PATH_LOST_FT below it: go around while there is height.
        let (step, phase) = final_step(path(8000.) - PATH_LOST_FT - 20., 240.);
        assert!(step.go_around);
        assert_eq!(phase, Phase::Marshal);
        // Under the floor the ordinary flare and landing rules apply instead.
        let (step, _) = final_step(PATH_LOST_FLOOR_FT - 20., 240.);
        assert!(!step.go_around);
    }

    #[test]
    fn the_approach_minds_only_the_ground_on_the_way_to_its_gate() {
        // 2026-09-29: holding the heading over hills beyond the gate flew a
        // pair away from its gates for 700 s at NSK 6.
        let guide = |to_gate: f64, ahead: f64| {
            let mut seq = landing();
            seq.phase = Phase::Approach;
            seq.leg = 1;
            let gate = seq.gate(1);
            // 20,000 ft short of the gate, abeam to the west, heading north.
            let mut s = on_final(&seq, 40_000., 1_500., 360.);
            s.position[0] = gate[0] - 20_000.;
            s.heading_deg = 0.;
            s.terrain_ahead_ft = ahead;
            s.terrain_to_gate_ft = to_gate;
            match seq.step(&s).command.control {
                Control::Air(g) => (g.heading_deg, g.flight_path_pitch_deg, g.wings_level),
                other => panic!("{other:?}"),
            }
        };
        let (free_heading, _, _) = guide(0., 0.);
        assert!(
            free_heading.abs() > 10.,
            "steers for the gate: {free_heading}"
        );
        // Hills beyond the gate change nothing.
        assert_eq!(guide(0., 1_400.), guide(0., 0.));
        // Hills on the way: no descent, but still steering for the gate.
        let (heading, pitch, level) = guide(1_000., 0.);
        assert_eq!(heading, free_heading);
        assert!(pitch >= 0. && !level);
        // Close above them: climb, wings level.
        let (_, pitch, level) = guide(1_200., 0.);
        assert!(pitch >= TERRAIN_CLIMB_DEG && level);
    }

    #[test]
    fn a_final_over_high_ground_short_of_the_threshold_holds_level() {
        let pitch = |agl: f64| {
            let mut seq = landing();
            seq.phase = Phase::Final;
            let mut s = on_final(&seq, 3000., 105., 240.);
            s.agl_ft = agl;
            match seq.step(&s).command.control {
                Control::Air(g) => g.flight_path_pitch_deg,
                other => panic!("{other:?}"),
            }
        };
        assert!(pitch(113.) < -1.0, "over flat ground it descends");
        assert!(pitch(8. + FINAL_TERRAIN_CLEARANCE_FT - 5.) >= 0.0);
    }

    #[test]
    fn the_final_speed_keeps_the_approach_lift_margin() {
        let seq = landing();
        let mut s = on_final(&seq, 5000., 525., 240.);
        assert_eq!(final_speed(&s), 230.);
        s.approach_minimum_fps = 0.;
        assert!((final_speed(&s) - APPROACH_SPEED_FACTOR * 150.).abs() < 1e-9);
        s.approach_minimum_fps = 400.;
        assert_eq!(final_speed(&s), FINAL_SPEED_LIMIT_FPS);
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
    fn approach_gates_sit_on_the_player_ils_path() {
        // John, 2026-09-29: the AI flies the player's 3 degree ILS path to
        // its aim point 1,000 ft past the threshold (retail: 6 degrees to a
        // quarter of the way down, gates about 3,700, 1,850 and 925 ft up).
        let s = landing();
        let aim = s.landing_point();
        assert_eq!(aim, [0., 0., -3000.]);
        for (k, (distance, height)) in [(35_200., 1_845.), (17_600., 922.), (8_800., 461.)]
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
            terrain_to_gate_ft: f64::MIN,
            approach_minimum_fps: 0.,
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
            touchdown_safe: true,
            hover_capable: true,
            detour: None,
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

    /// A field laid out like the redrawn base plans: parallel taxiway 450 ft
    /// right, cross taxiway 190 ft behind the threshold, apron beyond.
    fn anchored() -> RunwayView {
        let p = |x: f64, z: f64| [x, 0., z];
        RunwayView {
            center: [0., 0., 4000.],
            anchors: Some(AirfieldAnchors {
                taxi_out: [p(450., 1300.), p(450., -190.), p(225., -190.), p(0., -190.)],
                takeoff_spot: p(0., 0.),
                takeoff_heading: 0.,
                landing_point: p(0., 1000.),
                landing_heading: 0.,
                taxi_in: [p(0., 7750.), p(225., 7750.), p(420., 7750.), p(450., 4600.)],
                parking: std::array::from_fn(|k| p(900., 2000. + 250. * k as f64)),
                parking_heading: -std::f64::consts::FRAC_PI_2,
            }),
            ..runway()
        }
    }

    fn on_ground(position: [f64; 3], heading_deg: f64, speed_fps: f64) -> Situation {
        let h = heading_deg.to_radians();
        Situation {
            position,
            velocity: [speed_fps * h.sin(), 0., speed_fps * h.cos()],
            heading_deg,
            speed_fps,
            on_ground: true,
            agl_ft: 8.,
            ..on_final(&landing(), 0., 0., speed_fps)
        }
    }

    /// Slice CL-AP: taxiing along a leg the aircraft aims a little ahead on
    /// the leg's line, so a sideways error is steered out within a few
    /// hundred feet instead of being carried toward an anchor far away.
    #[test]
    fn taxiing_aims_along_the_leg_not_at_its_far_anchor() {
        let from = [0., 0., 0.];
        let to = [0., 0., 3000.];
        let aim = leg_aim([60., 0., 500.], from, to, TAXI_LEG_LOOKAHEAD_FT);
        assert_eq!(aim, [0., 0., 620.]);
        // Past the end it aims at the anchor.
        assert_eq!(
            leg_aim([0., 0., 2950.], from, to, TAXI_LEG_LOOKAHEAD_FT),
            to
        );
        // From the side of the line, the heading back to it is steep.
        let heading = bearing_deg([60., 0., 500.], aim);
        assert!(heading < -25., "{heading}");
    }

    /// Slice CL-AP: a heading error held on the ground (a crosswind on the
    /// roll) builds rudder trim; a large error (a turn) does not.
    #[test]
    fn a_held_heading_error_on_the_ground_builds_rudder_trim() {
        let start = GroundStart {
            runway: anchored(),
            end: ApproachEnd::Near,
            order: 1,
        };
        let mut seq = Sequence::departure(start, [0., 0., -100.], None);
        let s = on_ground([0., 0., 500.], 355., 150.);
        let first = seq.steer(&s, 0.);
        for _ in 0..240 {
            seq.steer(&s, 0.);
        }
        let later = seq.steer(&s, 0.);
        assert!(later > first + 0.1, "{first} -> {later}");
        assert!(seq.ground_trim <= GROUND_TRIM_LIMIT);
        let mut turning = Sequence::departure(start, [0., 0., -100.], None);
        for _ in 0..240 {
            turning.steer(&on_ground([0., 0., 500.], 270., 20.), 0.);
        }
        assert_eq!(turning.ground_trim, 0.0);
    }

    /// Slice CL-AP: the fourth of a wing waits on the parallel taxiway
    /// abeam the takeoff spot, inside the leg shortcut distance. It taxis
    /// its legs out; it neither lines up nor rolls from the taxiway.
    #[test]
    fn a_wingman_queued_abeam_the_spot_taxis_its_legs() {
        let start = GroundStart {
            runway: anchored(),
            end: ApproachEnd::Near,
            order: 3,
        };
        let position = [450., 0., -4.];
        let mut seq = Sequence::departure(start, position, None);
        let s = on_ground(position, 180., 0.);
        seq.step(&s);
        assert_eq!(seq.phase(), Phase::Taxi);
        assert_eq!(seq.leg(), 1);
        let s = Situation {
            tick: 2,
            ..on_ground([450., 0., -60.], 180., 20.)
        };
        seq.step(&s);
        assert_eq!(seq.phase(), Phase::Taxi, "the shortcut skipped its legs");
    }

    /// Slice CL-AP: lining up from the cross taxiway the aircraft aims no
    /// farther back than the anchor where that taxiway meets the centre
    /// line, so it stays on the taxiway rather than cutting behind it.
    #[test]
    fn lining_up_aims_no_farther_back_than_the_last_anchor() {
        let start = GroundStart {
            runway: anchored(),
            end: ApproachEnd::Near,
            order: 1,
        };
        let position = [-400., 0., -190.];
        let mut seq = Sequence::departure(start, position, None);
        seq.queue_leg = None;
        let s = on_ground(position, 90., 10.);
        seq.step(&s);
        assert_eq!(seq.phase(), Phase::LineUp);
        let step = seq.step(&Situation { tick: 2, ..s });
        let Control::Ground { yaw, .. } = step.command.control else {
            panic!("ground control");
        };
        // Heading 90 toward (0, -190) needs no turn; the old aim 350 ft
        // behind the spot asked for a right turn toward the grass.
        assert!(yaw.abs() < 0.05, "{yaw}");
    }

    /// Slice CL-AP: approaching an anchor it stops at, a taxiing aircraft
    /// slows so it stops there rather than 90 ft on.
    #[test]
    fn taxiing_slows_for_the_anchor_it_stops_at() {
        let mut seq = landing();
        seq.runway = anchored();
        seq.phase = Phase::TaxiClear;
        seq.leg = 2;
        seq.slot = Some(0);
        seq.leg_origin = [225., 0., 7750.];
        let ground = |seq: &mut Sequence, s: &Situation| {
            let step = seq.step(s);
            match step.command.control {
                Control::Ground { throttle, .. } => (throttle, step.command.brakes),
                Control::Air(_) | Control::Hover(_) => panic!("ground control"),
            }
        };
        let near = on_ground([380., 0., 7750.], 90., 50.);
        assert!(
            ground(&mut seq.clone(), &near).1,
            "no brakes 40 ft before the stop at 50 ft/s"
        );
        let mut far = seq.clone();
        far.leg = 1;
        far.leg_origin = [0., 0., 7750.];
        let (throttle, brakes) = ground(&mut far, &on_ground([-300., 0., 7750.], 90., 40.));
        assert!(!brakes && throttle > 0.0, "{throttle} {brakes}");
    }

    // -----------------------------------------------------------------------
    // Slice H2: the vertical phases.

    fn hover_of(command: &Command) -> crate::ai::rotor_adapter::HoverGuidance {
        match command.control {
            Control::Hover(h) => h,
            other => panic!("hover control, got {other:?}"),
        }
    }

    fn departure(lift: Lift, position: [f64; 3]) -> Sequence {
        let start = GroundStart {
            runway: runway(),
            end: ApproachEnd::Near,
            order: 1,
        };
        Sequence::departure(start, position, None).with_lift(lift)
    }

    /// Retail 1 to 0xa to 0xb: a helicopter lifts off where it stands on
    /// its turn with the runway free (no taxi), holds 5 s with the
    /// collective down and its engines governed, climbs straight up at
    /// 25 ft/s over its spot, and finishes at 500 ft.
    #[test]
    fn a_helicopter_lifts_off_where_it_stands_holds_five_seconds_and_climbs_to_500_ft() {
        let spot = [300., 8., -3000.];
        let mut seq = departure(Lift::Rotor, spot);
        let mut s = on_ground(spot, 90., 0.);
        s.tick = 0;
        let step = seq.step(&s);
        assert_eq!(seq.phase(), Phase::VerticalHold);
        assert!(step.command.brakes && step.command.lift.collective == Some(0.0));
        assert!(
            matches!(step.command.control, Control::Ground { throttle, .. } if throttle == 1.0)
        );
        assert!(seq.blocks_runway() && !seq.holds_followers());
        for tick in 1..600 {
            s.tick = tick;
            seq.step(&s);
            assert_eq!(seq.phase(), Phase::VerticalHold, "{tick}");
        }
        s.tick = 600;
        let step = seq.step(&s);
        assert_eq!(seq.phase(), Phase::VerticalClimb);
        assert!(!seq.blocks_runway(), "retail 0xb leaves the runway free");
        let h = hover_of(&step.command);
        assert_eq!(h.point, [300., -3000.]);
        assert_eq!(h.height_agl_ft, ROTOR_CLIMB_AGL_FT);
        assert_eq!(h.max_climb_fps, VERTICAL_CLIMB_FPS);
        assert_eq!(h.heading_deg, 90.);
        assert_eq!(step.command.activity, Activity::TakingOff);
        // 470 ft up: still climbing; 475 ft: done, gear up.
        s.on_ground = false;
        s.agl_ft = 470. + s.ground_clearance_ft;
        s.tick = 700;
        assert!(!seq.step(&s).complete);
        s.agl_ft = ROTOR_CLIMB_AGL_FT - CLIMB_REACHED_FT + s.ground_clearance_ft;
        let step = seq.step(&s);
        assert!(step.complete && !step.command.gear_down);
    }

    /// A hovering jet taxis to the takeoff spot, then takes off vertically
    /// (nozzles down for the hold), climbs to 800 ft and accelerates to its
    /// minimum speed plus 73 ft/s, the nozzles going aft at the end.
    #[test]
    fn a_hovering_jet_lines_up_then_takes_off_vertically_to_800_ft_and_accelerates() {
        let mut seq = departure(Lift::Jet, [40., 0., -3500.]);
        seq.phase = Phase::LineUp;
        seq.stopping = true;
        let s = on_ground(seq.spot, 0., 0.);
        let step = seq.step(&s);
        assert_eq!(seq.phase(), Phase::VerticalHold);
        assert_eq!(step.command.lift.nozzle_deg, Some(HOVER_NOZZLE_DEG));
        let mut s = Situation {
            tick: s.tick + 600,
            ..s
        };
        seq.step(&s);
        assert_eq!(seq.phase(), Phase::VerticalClimb);
        s.on_ground = false;
        s.agl_ft = JET_CLIMB_AGL_FT;
        s.tick = 700;
        let step = seq.step(&s);
        assert!(!step.complete);
        assert_eq!(seq.phase(), Phase::Transition);
        let h = hover_of(&seq.step(&s).command);
        let target = s.minimum_speed_fps + TRANSITION_MARGIN_FPS;
        assert!((h.velocity_fps[1] - target).abs() < 1e-9 && h.velocity_fps[0].abs() < 1e-9);
        s.speed_fps = target;
        let step = seq.step(&s);
        assert!(step.complete);
        assert_eq!(step.command.lift.nozzle_deg, Some(0.0));
    }

    /// A jet decides at the takeoff spot, stores on: too heavy to hover, it
    /// rolls for the short takeoff instead of holding for a vertical one.
    #[test]
    fn a_jet_too_heavy_at_the_spot_rolls_for_the_short_takeoff() {
        let mut seq = departure(Lift::Jet, [40., 0., -3500.]);
        seq.phase = Phase::LineUp;
        seq.stopping = true;
        let s = Situation {
            hover_capable: false,
            ..on_ground(seq.spot, 0., 0.)
        };
        seq.step(&s);
        assert_eq!(
            (seq.phase(), seq.lift()),
            (Phase::TakeoffRoll, Lift::ShortJet)
        );
    }

    /// Retail's short takeoff for a jet too heavy to hover: nozzles down to
    /// 60 degrees late in the roll, 40 to 250 ft at a 10 degree climb, then
    /// 20 until it is fast enough, then aft.
    #[test]
    fn a_heavy_jet_makes_the_retail_short_takeoff() {
        let mut seq = departure(Lift::ShortJet, [40., 0., -3500.]);
        seq.phase = Phase::TakeoffRoll;
        let nozzle = |seq: &mut Sequence, s: &Situation| seq.step(s).command.lift.nozzle_deg;
        let mut s = on_ground(seq.spot, 0., 50.);
        assert_eq!(nozzle(&mut seq, &s), Some(0.0));
        s.speed_fps = SHORT_NOZZLE_SPEED_SHARE * s.minimum_speed_fps;
        assert_eq!(nozzle(&mut seq, &s), Some(SHORT_NOZZLES_DEG[0]));
        s.on_ground = false;
        s.agl_ft = 100.;
        seq.step(&s);
        assert_eq!(seq.phase(), Phase::ClimbOut);
        let step = seq.step(&s);
        assert_eq!(step.command.lift.nozzle_deg, Some(SHORT_NOZZLES_DEG[1]));
        assert!(matches!(step.command.control,
            Control::Air(g) if g.flight_path_pitch_deg == SHORT_CLIMB_DEG));
        s.agl_ft = 300.;
        assert_eq!(nozzle(&mut seq, &s), Some(SHORT_NOZZLES_DEG[2]));
        s.speed_fps = s.minimum_speed_fps + TRANSITION_MARGIN_FPS;
        let step = seq.step(&s);
        assert!(step.complete && step.command.lift.nozzle_deg == Some(0.0));
    }

    fn vertical_landing(lift: Lift) -> Sequence {
        landing().with_lift(lift)
    }

    /// Retail 0x13/0x14 for vertical types: a helicopter's marshal square
    /// is 10,000 ft, lower (fitted); four gates 500 ft higher, the fourth
    /// 6,000 ft out, at most 146 ft/s for a helicopter.
    #[test]
    fn a_vertical_type_flies_four_raised_gates_and_a_helicopter_a_small_marshal_square() {
        let fixed = landing();
        let heli = vertical_landing(Lift::Rotor);
        assert_eq!((fixed.gates(), heli.gates()), (3, 4));
        assert_eq!(
            (fixed.first_gate(), heli.first_gate()),
            (0, ROTOR_FIRST_GATE)
        );
        assert_eq!(vertical_landing(Lift::Jet).first_gate(), 0);
        // Cleared from the marshal, a helicopter starts at the 8,800 ft gate.
        let mut cleared = heli.clone();
        cleared.step(&on_final(&heli, 60_000., 2_500., 140.));
        assert_eq!((cleared.phase(), cleared.leg()), (Phase::Approach, 2));
        for k in 0..3 {
            assert!((heli.gate(k)[1] - fixed.gate(k)[1] - VERTICAL_GATE_RAISE_FT).abs() < 1e-9);
        }
        let aim = heli.landing_point();
        assert!((heli.gate(3)[2] - (aim[2] - FOURTH_GATE_FT)).abs() < 1e-9);
        let corner = heli.marshal_corner([aim[0], 3000., aim[2] + 60_000.], 1);
        assert_eq!(
            corner,
            [
                aim[0] + ROTOR_MARSHAL_DISTANCE_FT,
                aim[1] + ROTOR_MARSHAL_AGL_FT + ROTOR_MARSHAL_STEP_FT,
                aim[2] + ROTOR_MARSHAL_DISTANCE_FT
            ]
        );
        let mut seq = heli.clone();
        seq.phase = Phase::Approach;
        let s = on_final(&seq, 40_000., 2_000., 140.);
        let Control::Air(g) = seq.step(&s).command.control else {
            panic!("air guidance");
        };
        assert!(g.speed_fps <= VERTICAL_GATE_SPEED_FPS);
        // The last gate leads to the hover approach, not the final.
        seq.leg = 3;
        let gate = seq.gate(3);
        let s = Situation {
            position: [gate[0], gate[1] + 8., gate[2]],
            ..s
        };
        seq.step(&s);
        assert_eq!(seq.phase(), Phase::HoverApproach);
        assert!(seq.blocks_runway());
    }

    /// Retail 0x18: over the landing point, slow, it comes straight down,
    /// at 50 ft/s high up and 5 ft/s at the wheels; within 30 ft of the
    /// ground it goes on only while the shared touchdown rule would accept
    /// a touchdown; down, it sits 5 s, then a helicopter air taxis to its
    /// slot, sets down there and shuts down, and a jet taxis in.
    #[test]
    fn the_vertical_landing_comes_down_over_the_point_sits_and_goes_to_parking() {
        for lift in [Lift::Rotor, Lift::Jet] {
            let mut seq = vertical_landing(lift).at_phase(Phase::HoverApproach, 0.);
            seq.slot = Some(2);
            let aim = seq.landing_point();
            let mut s = on_ground([aim[0] + 30., 0., aim[2]], 0., 0.);
            s.on_ground = false;
            s.agl_ft = 700.;
            s.velocity = [0., 0., 20.];
            seq.step(&s);
            assert_eq!(seq.phase(), Phase::HoverApproach, "too fast to start down");
            s.velocity = [0., 0., 5.];
            seq.step(&s);
            assert_eq!(seq.phase(), Phase::VerticalDescent, "{lift:?}");
            // The sink builds up from the hover.
            let h = hover_of(&seq.step(&s).command);
            assert_eq!(h.max_descent_fps, TOUCHDOWN_SINK_FPS);
            assert_eq!(h.height_agl_ft, SET_DOWN_FT);
            s.tick += 20 * 120;
            let h = hover_of(&seq.step(&s).command);
            assert_eq!(h.max_descent_fps, JET_DESCENT_FPS);
            s.agl_ft = 20. + s.ground_clearance_ft;
            let h = hover_of(&seq.step(&s).command);
            assert_eq!(h.max_descent_fps, TOUCHDOWN_SINK_FPS);
            // The touchdown rule would refuse: it holds its height.
            s.touchdown_safe = false;
            let h = hover_of(&seq.step(&s).command);
            assert!((h.height_agl_ft - 20.).abs() < 1e-9, "{h:?}");
            s.touchdown_safe = true;
            s.on_ground = true;
            s.agl_ft = s.ground_clearance_ft;
            s.tick = 1_000;
            seq.step(&s);
            assert_eq!(seq.phase(), Phase::Settle);
            assert!(seq.blocks_runway() && seq.landed());
            s.tick = 1_000 + 599;
            seq.step(&s);
            assert_eq!(seq.phase(), Phase::Settle);
            s.tick = 1_600;
            let step = seq.step(&s);
            if lift == Lift::Jet {
                assert_eq!(seq.phase(), Phase::TaxiClear);
                assert_eq!(step.command.lift.nozzle_deg, Some(0.0));
                continue;
            }
            assert_eq!(seq.phase(), Phase::AirTaxi);
            assert!(!seq.blocks_runway() && seq.landed());
            let slot = seq.park.unwrap();
            // Straight up over the landing point first.
            let h = hover_of(&step.command);
            assert_eq!(
                (h.point, h.height_agl_ft),
                ([s.position[0], s.position[2]], AIR_TAXI_AGL_FT)
            );
            assert_eq!(step.command.activity, Activity::Taxiing);
            // Up: on to the slot.
            s.on_ground = false;
            s.agl_ft = AIR_TAXI_AGL_FT + s.ground_clearance_ft;
            let h = hover_of(&seq.step(&s).command);
            assert_eq!(
                (h.point, h.height_agl_ft),
                ([slot[0], slot[2]], AIR_TAXI_AGL_FT)
            );
            assert_eq!(h.max_speed_fps, AIR_TAXI_FPS);
            // Over the slot: down, then parked with the engines shut down.
            s.on_ground = false;
            s.position = [slot[0] + 5., 30., slot[2]];
            seq.step(&s);
            assert_eq!(seq.phase(), Phase::VerticalDescent);
            assert!(!seq.blocks_runway());
            let h = hover_of(&seq.step(&s).command);
            assert_eq!(h.point, [slot[0], slot[2]]);
            s.on_ground = true;
            seq.step(&s);
            assert_eq!(seq.phase(), Phase::Parked);
            let step = seq.step(&s);
            assert_eq!(step.command.lift.collective, Some(0.0));
            assert!(
                matches!(step.command.control, Control::Ground { throttle, .. } if throttle == 0.0)
            );
        }
    }

    #[test]
    fn the_descent_rate_runs_from_twenty_down_to_five_feet_a_second() {
        use super::vertical::{descent_limit, descent_rate};
        assert_eq!(descent_rate(1_200.), JET_DESCENT_FPS);
        assert_eq!(descent_rate(200.), 10.);
        assert_eq!(descent_rate(10.), TOUCHDOWN_SINK_FPS);
        assert_eq!(descent_limit(800., 1., false), TOUCHDOWN_SINK_FPS);
        assert_eq!(descent_limit(800., 3., false), 15.);
        assert_eq!(descent_limit(800., 20., false), JET_DESCENT_FPS);
        assert_eq!(descent_limit(800., 0., true), TOUCHDOWN_SINK_FPS);
    }

    #[test]
    fn the_vertical_phases_guard_ejection_and_map_to_flight_states() {
        use Phase::*;
        let mut seq = vertical_landing(Lift::Rotor);
        for phase in [VerticalHold, VerticalClimb, Transition] {
            assert_eq!(phase.flight_state(), FlightState::TakingOff);
            seq.phase = phase;
            assert!(seq.guards_ejection());
        }
        for phase in [HoverApproach, VerticalDescent, Settle, AirTaxi] {
            assert_eq!(phase.flight_state(), FlightState::LateLanding);
            seq.phase = phase;
            assert!(seq.guards_ejection());
        }
    }

    /// The inbound leg flies to the detour point the mission found round a
    /// threat ring, at the route's height.
    #[test]
    fn the_inbound_leg_flies_to_the_detour_point() {
        let mut seq = Sequence::landing(
            LandingOrder {
                runway: runway(),
                reason: LandingReason::Ordered,
            },
            [0., 2000., -200_000.],
            [0.; 3],
            20_000.,
            Some((2000., 200.)),
        )
        .with_lift(Lift::Rotor);
        let mut s = on_final(&seq, 197_000., 2000., 200.);
        let straight = seq.step(&s).command.control;
        s.detour = Some([-50_000., -150_000.]);
        let Control::Air(g) = seq.step(&s).command.control else {
            panic!("air");
        };
        assert_ne!(Control::Air(g), straight);
        assert!((g.heading_deg - (-45.)).abs() < 0.5, "{}", g.heading_deg);
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "airfield_checkpoint.rs"]
mod checkpoint;

// The vertical takeoff and landing phases (slice H2).
#[path = "airfield_vertical.rs"]
pub mod vertical;
