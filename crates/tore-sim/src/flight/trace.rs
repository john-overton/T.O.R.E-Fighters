//! Write-only record of the last fixed flight step: the values the flight
//! model used, the effects it applied and the inputs that caused them, for
//! the telemetry panel and replay logs.
//!
//! Every value is a copy of a number the step computed anyway, except the
//! drag breakdown in [`DragTrace`], which is recomputed from the same inputs
//! for display. Nothing in the simulation reads this record, and it takes no
//! part in `State` equality. It carries no text; callers format it.
//! See docs/FLIGHT-MODEL.md#telemetry-record.
//!
//! Units: feet, feet per second (ft/s), pounds of mass (lb), pounds of force
//! (lbf), G, radians and radians per second unless a field name says
//! otherwise. Fractions run 0..1 and stick positions -1..1. Stick arrays are
//! [pitch, roll, yaw]; rate and control-scale arrays are [roll, pitch, yaw].

use crate::aircraft_systems::{ControlCondition, RegionalEffects};
use crate::autopilot::Mode;
use crate::runway_wind::Assessment;
use crate::turbulence::Disturbance;
use tore_formats::flight_model::departure::DepartureMode;
use tore_formats::flight_model::ground::{LandingLimits, LandingSeverity};

/// Everything recorded about the last step. Reset when `State::step_surface`
/// begins. `State::apply_turbulence`, `State::jolt_from` and `State::rebound`
/// run between steps and add to the record of the step that came before them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FlightTrace {
    /// `State::ticks` after the step.
    pub tick: u64,
    /// Which code moved the aircraft.
    pub path: Path,
    /// The autopilot's part, when it was engaged as the step began.
    pub autopilot: Option<AutopilotTrace>,
    /// Damage and hydraulic effects on the stick. Recorded for the legacy,
    /// hybrid and native adapters: all three receive this stick.
    pub controls: Option<ControlTrace>,
    /// A jammed throttle. Recorded for the legacy, hybrid and native adapters.
    pub throttle_lock: Option<ThrottleLock>,
    /// The legacy or hybrid adapter's values. `None` on every other path.
    pub adapter: Option<AdapterTrace>,
    /// Ground contact: every hybrid step, and legacy steps that reach the
    /// ground floor.
    pub contact: Option<Contact>,
    /// Missile-blast rates rotating the aircraft this step, [roll, pitch,
    /// yaw] rad/s, before they fade.
    pub jolt: Option<[f64; 3]>,
    /// A missile blast that kicked the aircraft after the step.
    pub blast: Option<Blast>,
    /// Turbulence the host applied after the step. The disturbance itself is
    /// generated outside the flight model; see `turbulence::Turbulence`.
    pub turbulence: Option<Disturbance>,
    /// No crashes bounced the aircraft back from a building after the step.
    pub rebound: bool,
}

/// Which code moved the aircraft in the last step.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Path {
    /// No step has run yet.
    #[default]
    NotStepped,
    /// Crashed or destroyed: the wreck component moved the aircraft and the
    /// flight model did not run.
    Wreck,
    /// The step stopped before the aircraft moved.
    Stopped(Stop),
    /// The restricted native research adapter moved the aircraft. Only this,
    /// the control response and the throttle lock are recorded for it.
    Native,
    /// The legacy compatibility adapter (`--legacy-flight`).
    Legacy,
    /// The hybrid researched adapter, the default.
    Hybrid,
}

/// Why a step stopped before the aircraft moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stop {
    /// A native research fault halts the restricted adapter.
    NativeFault,
    /// A fatal system failure (structure or pilot) this step.
    Crashed,
}

/// The autopilot's rewrite of the stick in `State::step_surface`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AutopilotTrace {
    /// Mode as the step began, after the pilot's autopilot switches.
    pub mode: Mode,
    /// Stick the pilot gave, [pitch, roll, yaw].
    pub pilot: [f64; 3],
    /// Stick the flight model received, [pitch, roll, yaw]. Equal to `pilot`
    /// when the autopilot let go before steering.
    pub commanded: [f64; 3],
    /// Why the autopilot switched itself off during the step, if it did.
    pub released: Option<Release>,
}

/// Why the autopilot switched itself off.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// Damaged controls or hydraulics, failed flight sensors, engine power
    /// below half, or a fatal failure (`Systems::autopilot_available`).
    SystemsDamage,
    /// Wing or tail damage (`State::damage_regions`).
    AirframeDamage,
    /// Navigation failed (system 33) while following a waypoint.
    NavigationFailed,
    /// The pilot moved the stick more than 0.15.
    PilotOverride,
    /// On the ground, or crashed.
    Ground,
}

/// What the control system did to the stick (`Systems::controls`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlTrace {
    /// Stick reaching the control system, [pitch, roll, yaw], after the
    /// autopilot.
    pub requested: [f64; 3],
    /// Stick the control system passed on, [pitch, roll, yaw]. With no
    /// hydraulic pressure this is the frozen surface position.
    pub output: [f64; 3],
    /// Hydraulic pressure 0..1. Below 1 it scales every axis; at 0 the
    /// control surfaces freeze and the gear, flaps, airbrake and hook stop.
    pub hydraulic: f64,
    /// Damage to the control runs.
    pub condition: ControlCondition,
}

/// A jammed throttle (system 29).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThrottleLock {
    /// Throttle position the jam holds, 0..1.
    pub held_at: f64,
    /// Throttle lever rate the pilot asked for this step, -1..1, discarded.
    pub ignored_rate: f64,
    /// The pilot set or stepped the throttle this step, and it was ignored.
    pub ignored_setting: bool,
}

/// The legacy or hybrid adapter's values for one step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AdapterTrace {
    pub air: AirTrace,
    pub regional: RegionalTrace,
    /// Runway wind on tire grip, hybrid with the wheels on the ground.
    pub runway_wind: Option<RunwayWind>,
    /// Parked on the wheels with no speed, idle power and no stick: the
    /// attitude was held.
    pub parked_attitude: bool,
    /// Parked with weight on the wheels and idle power: horizontal motion
    /// was held.
    pub parked_position: bool,
    pub devices: Devices,
    pub power: PowerTrace,
    pub envelope: EnvelopeTrace,
    pub lift: LiftTrace,
    /// The departure update, hybrid only.
    pub departure: Option<DepartureTrace>,
    pub scaling: ScalingTrace,
    pub rotation: RotationTrace,
    pub forces: ForceTrace,
}

/// Air data the adapter used.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct AirTrace {
    /// Altitude, feet above mean sea level.
    pub altitude_ft: f64,
    /// Airspeed the step's limits and forces used, ft/s.
    pub airspeed_fps: f64,
    /// Surface wind removed from the velocity for the aerodynamics, world
    /// [x, y, z] ft/s.
    pub wind_fps: [f64; 3],
    /// Horizontal ground speed as the step began, ft/s.
    pub ground_speed_fps: f64,
    /// The wheels carried the aircraft as the step began (hybrid).
    pub on_wheels: bool,
}

/// Wing and tail damage and the penalties it produced
/// (`aircraft_systems::regional_effects`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionalTrace {
    /// Damage 0..1 of the left wing, right wing and tail
    /// (`State::damage_regions[3..6]`).
    pub damage: [f64; 3],
    /// Control authority, roll and yaw bias, lift multiplier and drag
    /// increase in percent.
    pub effects: RegionalEffects,
    /// Stick the aerodynamics received after the authority and bias,
    /// [pitch, roll, yaw].
    pub commands: [f64; 3],
}
impl Default for RegionalTrace {
    fn default() -> Self {
        Self {
            damage: [0.; 3],
            effects: crate::aircraft_systems::regional_effects([0.; 6]),
            commands: [0.; 3],
        }
    }
}

/// Wind on a rolling aircraft's tire grip (`runway_wind`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunwayWind {
    /// Crosswind, tailwind and headwind against the heading, in knots, with
    /// the limits of the aircraft's weight class.
    pub assessment: Assessment,
    /// Fade-in 0..1 over the first five knots of ground speed.
    pub ground_motion: f64,
    /// Wind fraction 0..1 passed to tire contact: the larger of the
    /// crosswind and tailwind fractions, times `ground_motion`. Tire grip is
    /// 1 - 0.5 x this.
    pub fraction: f64,
}

/// Gear, flaps, airbrake and hook this step.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Devices {
    pub gear: Device,
    pub flaps: Device,
    pub airbrake: Device,
    pub hook: Device,
}

/// One deployable device.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Device {
    /// Switch position: true asks for fully out.
    pub commanded: bool,
    /// Position after the step, 0..1.
    pub position: f64,
    /// Why it could not move this step, if it could not.
    pub blocked: Option<Block>,
}
impl Device {
    /// Blocked away from the position its switch asks for.
    pub fn held(&self) -> bool {
        self.blocked.is_some() && self.position != f64::from(self.commanded)
    }
}

/// Which device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceKind {
    Gear,
    Flaps,
    Airbrake,
    Hook,
}

/// Why a device could not move.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Block {
    /// No hydraulic pressure.
    NoHydraulics,
    /// Jammed: landing gear (system 16), flaps (17) or airbrake (18).
    Jammed,
}

/// Engine, fuel and thrust.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PowerTrace {
    /// Engine running after this step's fuel check.
    pub engine: bool,
    /// Internal and external fuel were empty, so the engine and afterburner
    /// were switched off this step.
    pub fuel_starved: bool,
    /// Afterburner lit.
    pub afterburner: bool,
    /// Why the afterburner stayed dark with its switch on.
    pub burner_blocked: Option<BurnerBlock>,
    /// Throttle after this step's lever rate, 0..1.
    pub throttle: f64,
    /// Throttle above which the afterburner lights, 0..1.
    pub afterburner_throttle: f64,
    /// Fuel the engine burned this step, lb/s. Zero with the engine off.
    pub fuel_flow_lbs_per_second: f64,
    /// Unlimited fuel was on: the burn was not taken from the tanks.
    pub unlimited_fuel: bool,
    /// Afterburner maximum, or military thrust x throttle, before lapse and
    /// damage, lbf. Zero with the engine off.
    pub rated_thrust_lbf: f64,
    /// The aircraft model's thrust lapse 0..1 for this altitude and speed. A
    /// value from the model's response curve, not a reason.
    pub lapse: f64,
    /// Engine power available 0..1 after damage and flameout.
    pub power_available: f64,
    /// Thrust applied: rated x lapse x power available, lbf.
    pub thrust_lbf: f64,
}

/// Why the afterburner stayed dark with its switch on, in the order the
/// model checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BurnerBlock {
    EngineOff,
    /// Afterburner failed (system 8).
    Failed,
    /// No engine power (damage or flameout).
    NoPower,
    /// This aircraft has no afterburner.
    NotFitted,
    NoFuel,
    /// Throttle at or below `PowerTrace::afterburner_throttle`.
    ThrottleLow,
}

/// Stall and top speed, control authority and the G limits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct EnvelopeTrace {
    /// Stall speed of the aircraft's 1 G envelope at this altitude, ft/s.
    pub clean_stall_fps: f64,
    /// Stall speed the step used, ft/s. The hybrid adapter multiplies the
    /// clean value by 1 - 0.25 x flaps.
    pub stall_fps: f64,
    /// Flap position 0..1.
    pub flaps: f64,
    /// Top speed of the 1 G envelope at this altitude, ft/s.
    pub top_speed_fps: f64,
    /// No 1 G envelope covers this altitude, so the step used a stall speed
    /// of 900 ft/s and a top speed of 1,000 ft/s.
    pub no_1g_envelope: bool,
    /// Control authority 0..1: (airspeed / stall speed) squared, at most 1.
    /// It scales commanded G, roll, rudder and nose alignment.
    pub authority: f64,
    /// Number of envelope rows containing this speed at this altitude.
    pub rows: u32,
    /// Lowest and highest G of those rows, [-1, 1] when none do.
    pub envelope_g: [f64; 2],
    /// Fuel plus carried stores over empty weight.
    pub loading: f64,
    /// 1 + loading x the aircraft's loaded-elevator percent / 100. Both G
    /// limits are divided by it.
    pub load_divisor: f64,
    /// Positive limit after the divisor, before Pull extra G, G.
    pub loaded_positive_g: f64,
    /// Pull extra G was on.
    pub extra_g: bool,
    /// Hybrid low-speed ceiling on positive G, when the speed is on its ramp.
    pub low_speed_ceiling: Option<LowSpeedCeiling>,
    /// Final [negative, positive] G limits.
    pub limits_g: [f64; 2],
    /// Pitch stick after regional damage.
    pub stick: f64,
    /// G asked by the stick within the limits, times authority, before the
    /// lift factors in [`LiftTrace`].
    pub stick_g: f64,
}

/// The hybrid ramp that caps positive G near the stall speed: 1 G at
/// `from_fps`, rising in a straight line to `to_g` at `to_fps`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LowSpeedCeiling {
    /// The ceiling, G. The load divisor applies to it unless Pull extra G is
    /// on.
    pub limit_g: f64,
    /// Start of the ramp: the effective stall speed, ft/s.
    pub from_fps: f64,
    /// End of the ramp: where the next envelope row above 1 G starts at this
    /// altitude, ft/s.
    pub to_fps: f64,
    /// G at the end of the ramp: that row's G, or 9 with Pull extra G if
    /// higher.
    pub to_g: f64,
    /// Position on the ramp, 0..1.
    pub fraction: f64,
}

/// Lift factors applied to the commanded G.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LiftTrace {
    /// Transonic drag percentage from the recovered helper, 0..100. It
    /// scales hybrid flap lift and flap and airbrake drag. A value, not a
    /// reason: it grows with speed against the top speed, with a transonic
    /// correction.
    pub drag_percent: f64,
    /// Hybrid flap lift multiplier, 1 + flaps x `flap_lift_f8` / 256. One on
    /// legacy.
    pub flap_factor: f64,
    /// Flap lift per full flap in 1/256 units after the gear blend; halved on
    /// the wheels.
    pub flap_lift_f8: f64,
    /// Wing damaged (system 25): commanded G x0.5.
    pub wing_damaged: bool,
    /// Commanded G after flaps, wing damage and regional lift
    /// (`Maneuver::commanded_g`).
    pub commanded_g: f64,
    /// Hybrid spin multiplier on the lift target, 1 - 0.85 x spin blend.
    pub spin_factor: f64,
    /// Lift target after the spin multiplier, G.
    pub target_g: f64,
    /// Lift after this step's lag toward the target, before stall scaling,
    /// G.
    pub lagged_g: f64,
}

/// The hybrid departure update (`Research::advance`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DepartureTrace {
    /// Departure mode before the update.
    pub mode_before: DepartureMode,
    /// Departure mode after the update.
    pub mode: DepartureMode,
    /// Spin direction before the update: 1 right, -1 left, 0 none.
    pub spinning_before: i8,
    /// Spin direction after the update.
    pub spinning: i8,
    /// Fitted drive toward a spin 0..1, from the speed deficit below stall
    /// and back stick. Zero when the aircraft data disables spin entry.
    pub drive: f64,
    /// The pro-spin check, when it ran this step.
    pub spin_check: Option<SpinCheck>,
    /// How a spin ended this step.
    pub spin_exit: Option<SpinExit>,
    /// Signed spin yaw rate after the update, rad/s.
    pub spin_rate: f64,
    /// The aircraft's maximum spin rate, rad/s.
    pub max_spin_rate: f64,
    /// The wheels were on the ground, so the departure state was cleared.
    pub on_ground: bool,
}

/// The source rule that picks a spin direction: stalled or warning, drive
/// above zero, not already spinning, and spins allowed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpinCheck {
    /// Direction the rule chose: 1 right, -1 left.
    pub direction: i8,
    /// `Some(draw)` when the roll rate and bank were both zero in the rule's
    /// units, so a 50% random draw chose the direction.
    pub coin: Option<bool>,
    /// Roll rate in 1/256 degree per second, as the rule read it.
    pub roll_rate_units: i32,
    /// Bank in binary angle units (65,536 per turn), as the rule read it.
    pub bank_units: i16,
    /// The rudder pointed the same way, so the spin began.
    pub entered: bool,
}

/// How a spin ended.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpinExit {
    /// Rotation slowed within normal rudder authority, the airflow was inside
    /// 25 degrees of the nose and the rudder was not pro-spin.
    Recovered,
    /// No spins was switched on mid-spin: the rotation damps out as a stall.
    SpinsDisabled,
}

/// Stall and spin scaling of controls and lift.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScalingTrace {
    /// A stall was active this step.
    pub stalled: bool,
    /// Stall severity in 1/256 units.
    pub severity_f8: i32,
    /// Stall scaling of [roll, pitch, yaw] control.
    pub stall_controls: [f64; 3],
    /// Stall scaling of lift, 0..1.
    pub stall_lift: f64,
    /// Spin blend 0..1: spin rate over the maximum spin rate.
    pub spin_blend: f64,
    /// Control effectiveness in the spin 0..1, from rotation and airflow. It
    /// scales roll, yaw and the spin pitch response.
    pub spin_controls: f64,
    /// Final control scale, [roll, pitch, yaw].
    pub controls: [f64; 3],
}

/// Roll, pitch, trim and yaw.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RotationTrace {
    /// Which roll law ran.
    pub roll: RollLaw,
    /// Pitch rate target from the commanded G, rad/s.
    pub normal_pitch: f64,
    /// Pitch rate target from direct elevator in a spin, rad/s.
    pub spin_pitch: f64,
    /// Blend of the two by spin blend, rad/s.
    pub pitch_command: f64,
    /// Trim angle of attack from the aircraft model, rad. A value from the
    /// model's response curve, not a reason.
    pub model_trim_rad: f64,
    /// Hybrid low-speed trim blend, below twice the clean stall speed.
    pub low_speed_trim: Option<LowSpeedTrim>,
    /// Trim angle the nose aligned toward, rad.
    pub trim_rad: f64,
    /// Nose alignment rate toward the trim angle, rad/s.
    pub alignment: f64,
    /// Yaw from gravity across the wings in a banked turn, rad/s.
    pub turn_yaw: f64,
    /// Yaw from the rudder with damage bias and scaling, rad/s.
    pub rudder_yaw: f64,
    /// Powered low-speed auxiliary yaw, rad/s.
    pub auxiliary_yaw: f64,
    /// Hybrid rudder steering on the wheels, rad/s.
    pub ground_steering_yaw: f64,
    /// Hybrid spin yaw, rad/s.
    pub spin_yaw: f64,
}

/// Which roll law ran.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum RollLaw {
    /// Fitted lag toward roll stick x `limit` x authority x stall scale.
    Lag {
        /// Roll rate limit, rad/s.
        limit: f64,
        /// Target roll rate, rad/s.
        command: f64,
    },
    /// The aircraft's own control profile.
    Profile {
        /// Roll authority 0..1: airspeed over twice the stall speed, times
        /// the roll control scale.
        authority: f64,
        /// Powered low-speed auxiliary control authority 0..1, from throttle
        /// and speed.
        auxiliary: f64,
        /// Engine running with fuel; auxiliary control needs it.
        powered: bool,
        /// At or below the ground clearance, where auxiliary control is off.
        on_ground: bool,
    },
}
impl Default for RollLaw {
    fn default() -> Self {
        Self::Lag {
            limit: 0.,
            command: 0.,
        }
    }
}

/// The hybrid low-speed trim blend.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LowSpeedTrim {
    /// Stick-dependent low-speed trim angle, rad.
    pub trim_rad: f64,
    /// Blend 0..1 toward the model trim: 0 at the effective stall speed, 1 at
    /// twice the clean stall speed.
    pub blend: f64,
}

/// Mass, drag and the resulting load.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ForceTrace {
    /// Empty weight + fuel + carried stores, lb.
    pub weight_lbs: f64,
    /// Store mass the model carried, lb.
    pub carried_lbs: f64,
    /// Payload on board, lb.
    pub payload_lbs: f64,
    /// Ignore weapon weights was on: `carried_lbs` is only external fuel.
    pub ignore_weapon_weights: bool,
    pub drag: DragTrace,
    /// Achieved aerodynamic G along body up (`State::g`).
    pub achieved_g: f64,
    /// Upward support from lift, thrust and drag as a share of weight, G.
    pub support_g: f64,
    /// Wheel load 0..1: 1 - support. It scales tire scrub, rolling and brake
    /// forces.
    pub wheel_load: f64,
    /// Speed passed 6,000 ft/s and was capped: the speed before the cap.
    pub speed_capped_from_fps: Option<f64>,
}

/// Drag, lbf. The total and the capped values are the ones applied. The
/// parts are recomputed from the same inputs for display; their sum can
/// differ from `undamaged_lbf` in the last digits.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DragTrace {
    /// Drag applied.
    pub total_lbf: f64,
    /// Before regional damage.
    pub undamaged_lbf: f64,
    /// After regional damage, before the hybrid cap.
    pub uncapped_lbf: f64,
    /// Hybrid cap: the drag that would stop the aircraft in one step.
    pub cap_lbf: Option<f64>,
    /// Clean airframe drag, from speed against the top speed.
    pub airframe_lbf: f64,
    /// Extra airframe drag from fuel and stores (loaded drag percent).
    pub load_lbf: f64,
    /// Drag from pulling G above 1.
    pub pull_lbf: f64,
    /// Gear drag; zero on the wheels in the hybrid adapter.
    pub gear_lbf: f64,
    /// Flap drag, after `device_fraction`.
    pub flaps_lbf: f64,
    /// Airbrake drag, after `device_fraction`.
    pub airbrake_lbf: f64,
    /// Drag from sideslip.
    pub slip_lbf: f64,
    /// Regional damage drag increase, percent.
    pub damage_percent: f64,
    /// Hybrid: the wheels were on the ground, so gear drag was off.
    pub gear_on_wheels: bool,
    /// Flap and airbrake drag scale 0..1: drag percent / 100 on hybrid, 1 on
    /// legacy.
    pub device_fraction: f64,
}

/// Ground contact after the step.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contact {
    /// Hybrid: above the support plane.
    Airborne,
    /// Hybrid: the surface fell away more than 0.05 ft below the wheels.
    SurfaceDropped,
    /// Hybrid: wheel load at most 2% while climbing, so the wheels left the
    /// ground.
    LiftOff {
        /// Wheel load 0..1.
        wheel_load: f64,
    },
    /// Hybrid: a touchdown outside what the aircraft can land on.
    Unsafe(UnsafeTouchdown),
    /// Hybrid: on the wheels.
    Rolling(Rolling),
    /// Legacy: reached the ground floor and crashed, or bounced with No
    /// crashes.
    LegacyFloor {
        /// No crashes bounced the aircraft.
        bounced: bool,
    },
}

/// A touchdown's values and the landing limits they were checked against.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Touchdown {
    /// Bank, degrees.
    pub bank_deg: f64,
    /// Pitch, degrees.
    pub pitch_deg: f64,
    /// Speed along the nose, ft/s.
    pub forward_fps: f64,
    /// Speed to the right, ft/s.
    pub side_fps: f64,
    /// Vertical speed, ft/s, negative descending.
    pub vertical_fps: f64,
    /// The aircraft's landing limits: forward, side and descent speed in
    /// ft/s, pitch and roll in degrees.
    pub limits: LandingLimits,
}

/// A touchdown the aircraft could not land from. Any one reason is enough.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UnsafeTouchdown {
    /// No crashes bounced the aircraft instead of crashing it.
    pub bounced: bool,
    pub water: bool,
    /// The surface is not a runway.
    pub not_landable: bool,
    /// Gear position below 0.99.
    pub gear_up: bool,
    pub gear: f64,
    /// Classification against the landing limits: `WithinLimits`, `Code5`
    /// (over a limit) or `Code6` (well over one).
    pub severity: LandingSeverity,
    pub touchdown: Touchdown,
}

/// Tire forces on the wheels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rolling {
    /// The touchdown this step, if the aircraft was airborne before it.
    pub touchdown: Option<Landing>,
    /// Tire grip 0..1 left by runway wind: 1 - 0.5 x the wind fraction.
    pub wind_grip: f64,
    /// Share of sideways slip the tires removed this step, 0..1.
    pub scrub: f64,
    /// Wheel brakes applied (the airbrake switch on the ground).
    pub brakes: bool,
    /// Brake or rolling-resistance deceleration after wheel load, ft/s².
    pub deceleration_fps2: f64,
    /// The brakes held a slow aircraft still against this step's forces.
    pub brake_hold: bool,
    /// Horizontal speed before the tire forces, ft/s.
    pub ground_speed_fps: f64,
    /// Elevator at or below neutral, so the nose settled toward the runway.
    pub nose_settling: bool,
}

/// A graded landing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Landing {
    pub touchdown: Touchdown,
    /// `Some(score)` after at least five seconds of flight: 100 when the
    /// descent and bank were within half the limits, 50 otherwise. `None`
    /// for a bounce or a spawn on the runway.
    pub score: Option<u32>,
}

/// A missile blast kick (`State::jolt_from`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Blast {
    /// Body rates added, [roll, pitch, yaw] rad/s, including strength.
    pub kick: [f64; 3],
    /// Push away from the blast, ft/s.
    pub push_fps: f64,
    /// Warhead strength after clamping to 0.5..2; 1 is a 100-point warhead.
    pub strength: f64,
}

/// One effect the last step applied, with its factor or limit and the inputs
/// that caused it. [`FlightTrace::effects`] lists the active ones; values
/// every step has (limits, thrust, drag parts) stay in the trace sections.
/// Compare `std::mem::discriminant` values to notice an effect starting or
/// stopping.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// The flight model did not run: [`Path::Wreck`] or [`Path::Stopped`].
    NotFlying(Path),
    /// The restricted native research adapter ran. Its internal effects are
    /// not recorded.
    NativePath,
    /// The autopilot steered: `commanded` replaced the pilot's stick.
    Autopilot {
        mode: Mode,
        pilot: [f64; 3],
        commanded: [f64; 3],
    },
    /// The autopilot switched itself off.
    AutopilotReleased(Release),
    /// No hydraulic pressure: the control surfaces froze where they were.
    /// Devices that could not move are listed as [`Effect::DeviceHeld`].
    HydraulicsLost,
    /// Damaged control runs or low hydraulic pressure changed the stick.
    ControlResponse(ControlTrace),
    /// A jammed throttle held its position.
    ThrottleJammed(ThrottleLock),
    /// Wing or tail damage changed authority, added roll and yaw bias, cut
    /// lift and added drag.
    RegionalDamage(RegionalTrace),
    /// Wind across or behind a rolling aircraft reduced tire grip to
    /// 1 - 0.5 x `fraction`.
    RunwayWind(RunwayWind),
    /// Parked: attitude and/or horizontal motion held.
    Parked { attitude: bool, position: bool },
    /// A device could not reach its switch position.
    DeviceHeld { device: DeviceKind, state: Device },
    /// Internal and external fuel ran out: engine and afterburner off.
    FuelStarved,
    /// No thrust: the engine was off. `power_available` 0 means failed or
    /// flamed out; otherwise it was switched off.
    EngineOff { power_available: f64 },
    /// Damage reduced engine power: thrust x `power_available`.
    EnginePowerReduced { power_available: f64 },
    /// The afterburner stayed dark with its switch on.
    AfterburnerBlocked {
        cause: BurnerBlock,
        throttle: f64,
        afterburner_throttle: f64,
    },
    /// Unlimited fuel: this burn was not taken from the tanks, lb/s.
    UnlimitedFuel { fuel_flow_lbs_per_second: f64 },
    /// Ignore weapon weights: the model carried `carried_lbs` of the
    /// `payload_lbs` on board.
    IgnoreWeaponWeights { carried_lbs: f64, payload_lbs: f64 },
    /// No 1 G envelope covers this altitude: stall speed 900 ft/s and top
    /// speed 1,000 ft/s.
    NoEnvelopeAtAltitude { altitude_ft: f64 },
    /// Hybrid flaps lowered the stall speed.
    FlapStallSpeed {
        clean_stall_fps: f64,
        stall_fps: f64,
        flaps: f64,
    },
    /// Below the stall speed, authority fell with the square of the speed
    /// ratio. It scaled commanded G, roll, rudder and nose alignment.
    LowSpeedAuthority {
        authority: f64,
        airspeed_fps: f64,
        stall_fps: f64,
    },
    /// No envelope row contains this speed at this altitude: G limits of
    /// ±1 before loading.
    OutsideEnvelope { airspeed_fps: f64, altitude_ft: f64 },
    /// Fuel and stores divided both G limits by `divisor`.
    LoadedLimits { divisor: f64, loading: f64 },
    /// Pull extra G raised the positive limit.
    ExtraG { from_g: f64, limit_g: f64 },
    /// Hybrid low-speed ceiling on positive G.
    LowSpeedCeiling {
        ceiling: LowSpeedCeiling,
        airspeed_fps: f64,
        flaps: f64,
        /// Positive limit after the ceiling, G.
        positive_limit_g: f64,
    },
    /// Hybrid flap lift multiplied the commanded G by `factor`.
    FlapLift {
        factor: f64,
        flaps: f64,
        gear: f64,
        on_wheels: bool,
        drag_percent: f64,
    },
    /// Wing damaged (system 25): commanded G x0.5.
    WingDamaged,
    /// Hybrid spin rotation cut the lift target by `factor`.
    SpinLiftLoss { factor: f64, spin_blend: f64 },
    /// A stall scaled controls ([roll, pitch, yaw]) and lift by severity.
    StallScaling {
        severity_f8: i32,
        controls: [f64; 3],
        lift: f64,
    },
    /// Rotation in a spin reduced roll, yaw and spin pitch control.
    SpinControls { effectiveness: f64, spin_blend: f64 },
    /// The spin direction rule ran; see [`SpinCheck::entered`].
    SpinCheck(SpinCheck),
    /// A spin ended; `mode` is the departure mode afterwards.
    SpinEnded { exit: SpinExit, mode: DepartureMode },
    /// Wheels on the ground cleared a stall or spin.
    DepartureCleared { mode_before: DepartureMode },
    /// Hybrid low-speed trim blend moved the nose target.
    LowSpeedTrim {
        trim: LowSpeedTrim,
        model_trim_rad: f64,
        trim_rad: f64,
    },
    /// Hybrid rudder steering on the wheels, rad/s.
    GroundSteering { yaw_rate: f64 },
    /// Hybrid: gear drag off with the wheels on the ground.
    GearDragOnWheels { gear: f64 },
    /// Hybrid: flap and airbrake drag scaled by the drag percentage.
    DeviceDragScaled { fraction: f64 },
    /// Hybrid: drag capped at the value that stops the aircraft in one step.
    DragCapped { cap_lbf: f64, uncapped_lbf: f64 },
    /// Speed capped at 6,000 ft/s.
    SpeedCapped { from_fps: f64 },
    /// Hybrid: the wheels left the ground.
    LiftOff { wheel_load: f64 },
    /// Hybrid: the surface fell away from the wheels.
    SurfaceDropped,
    /// Hybrid: a touchdown, graded when it followed real flight.
    Touchdown(Landing),
    /// Hybrid: an unsafe touchdown crashed or bounced the aircraft.
    UnsafeTouchdown(UnsafeTouchdown),
    /// Legacy: the ground floor crashed or bounced the aircraft.
    LegacyFloor { bounced: bool },
    /// Hybrid: tire scrub, rolling resistance and brakes on the wheels.
    Rolling(Rolling),
    /// A missile blast kicked the aircraft.
    BlastKick(Blast),
    /// Missile-blast rates rotated the aircraft, [roll, pitch, yaw] rad/s.
    Jolt { rates: [f64; 3] },
    /// The host applied turbulence.
    Turbulence(Disturbance),
    /// No crashes bounced the aircraft back from a building.
    BuildingRebound,
}

impl FlightTrace {
    /// The legacy or hybrid adapter moved the aircraft.
    pub fn flew(&self) -> bool {
        matches!(self.path, Path::Legacy | Path::Hybrid)
    }

    /// The effects the last step applied, roughly in the order it applied
    /// them.
    pub fn effects(&self) -> Vec<Effect> {
        let mut out = Vec::new();
        match self.path {
            Path::NotStepped => {}
            Path::Wreck | Path::Stopped(_) => out.push(Effect::NotFlying(self.path)),
            Path::Native => out.push(Effect::NativePath),
            Path::Legacy | Path::Hybrid => {}
        }
        if let Some(a) = self.autopilot {
            if a.commanded != a.pilot {
                out.push(Effect::Autopilot {
                    mode: a.mode,
                    pilot: a.pilot,
                    commanded: a.commanded,
                });
            }
            if let Some(release) = a.released {
                out.push(Effect::AutopilotReleased(release));
            }
        }
        if let Some(c) = self.controls {
            let healthy = ControlCondition::default();
            if c.hydraulic <= 0. {
                out.push(Effect::HydraulicsLost);
            } else if c.hydraulic < 1. || c.condition != healthy {
                out.push(Effect::ControlResponse(c));
            }
        }
        if let Some(lock) = self.throttle_lock {
            out.push(Effect::ThrottleJammed(lock));
        }
        if let Some(t) = &self.adapter {
            adapter_effects(t, self.path == Path::Hybrid, &mut out);
        }
        match self.contact {
            Some(Contact::LiftOff { wheel_load }) => out.push(Effect::LiftOff { wheel_load }),
            Some(Contact::SurfaceDropped) => out.push(Effect::SurfaceDropped),
            Some(Contact::Unsafe(u)) => out.push(Effect::UnsafeTouchdown(u)),
            Some(Contact::Rolling(r)) => {
                if let Some(landing) = r.touchdown {
                    out.push(Effect::Touchdown(landing));
                }
                out.push(Effect::Rolling(r));
            }
            Some(Contact::LegacyFloor { bounced }) => out.push(Effect::LegacyFloor { bounced }),
            Some(Contact::Airborne) | None => {}
        }
        if let Some(rates) = self.jolt {
            out.push(Effect::Jolt { rates });
        }
        if let Some(blast) = self.blast {
            out.push(Effect::BlastKick(blast));
        }
        if let Some(d) = self.turbulence {
            out.push(Effect::Turbulence(d));
        }
        if self.rebound {
            out.push(Effect::BuildingRebound);
        }
        out
    }
}

fn adapter_effects(t: &AdapterTrace, hybrid: bool, out: &mut Vec<Effect>) {
    if t.regional.damage.iter().any(|d| *d > 0.) {
        out.push(Effect::RegionalDamage(t.regional));
    }
    if let Some(wind) = t.runway_wind
        && wind.fraction > 0.
    {
        out.push(Effect::RunwayWind(wind));
    }
    if t.parked_attitude || t.parked_position {
        out.push(Effect::Parked {
            attitude: t.parked_attitude,
            position: t.parked_position,
        });
    }
    let d = t.devices;
    for (device, state) in [
        (DeviceKind::Gear, d.gear),
        (DeviceKind::Flaps, d.flaps),
        (DeviceKind::Airbrake, d.airbrake),
        (DeviceKind::Hook, d.hook),
    ] {
        if state.held() {
            out.push(Effect::DeviceHeld { device, state });
        }
    }
    let p = t.power;
    if p.fuel_starved {
        out.push(Effect::FuelStarved);
    } else if !p.engine {
        out.push(Effect::EngineOff {
            power_available: p.power_available,
        });
    } else if p.power_available < 1. {
        out.push(Effect::EnginePowerReduced {
            power_available: p.power_available,
        });
    }
    if let Some(cause) = p.burner_blocked {
        out.push(Effect::AfterburnerBlocked {
            cause,
            throttle: p.throttle,
            afterburner_throttle: p.afterburner_throttle,
        });
    }
    if p.unlimited_fuel && p.fuel_flow_lbs_per_second > 0. {
        out.push(Effect::UnlimitedFuel {
            fuel_flow_lbs_per_second: p.fuel_flow_lbs_per_second,
        });
    }
    let f = t.forces;
    if f.ignore_weapon_weights && f.carried_lbs != f.payload_lbs {
        out.push(Effect::IgnoreWeaponWeights {
            carried_lbs: f.carried_lbs,
            payload_lbs: f.payload_lbs,
        });
    }
    let e = t.envelope;
    if e.no_1g_envelope {
        out.push(Effect::NoEnvelopeAtAltitude {
            altitude_ft: t.air.altitude_ft,
        });
    }
    if e.stall_fps < e.clean_stall_fps {
        out.push(Effect::FlapStallSpeed {
            clean_stall_fps: e.clean_stall_fps,
            stall_fps: e.stall_fps,
            flaps: e.flaps,
        });
    }
    if e.authority < 1. {
        out.push(Effect::LowSpeedAuthority {
            authority: e.authority,
            airspeed_fps: t.air.airspeed_fps,
            stall_fps: e.stall_fps,
        });
    }
    if e.rows == 0 {
        out.push(Effect::OutsideEnvelope {
            airspeed_fps: t.air.airspeed_fps,
            altitude_ft: t.air.altitude_ft,
        });
    }
    if e.load_divisor != 1. {
        out.push(Effect::LoadedLimits {
            divisor: e.load_divisor,
            loading: e.loading,
        });
    }
    if e.extra_g && e.loaded_positive_g < crate::cheats::EXTRA_G {
        out.push(Effect::ExtraG {
            from_g: e.loaded_positive_g,
            limit_g: crate::cheats::EXTRA_G,
        });
    }
    if let Some(ceiling) = e.low_speed_ceiling {
        out.push(Effect::LowSpeedCeiling {
            ceiling,
            airspeed_fps: t.air.airspeed_fps,
            flaps: e.flaps,
            positive_limit_g: e.limits_g[1],
        });
    }
    let l = t.lift;
    if l.flap_factor != 1. {
        out.push(Effect::FlapLift {
            factor: l.flap_factor,
            flaps: e.flaps,
            gear: d.gear.position,
            on_wheels: t.air.on_wheels,
            drag_percent: l.drag_percent,
        });
    }
    if l.wing_damaged {
        out.push(Effect::WingDamaged);
    }
    let s = t.scaling;
    if l.spin_factor != 1. {
        out.push(Effect::SpinLiftLoss {
            factor: l.spin_factor,
            spin_blend: s.spin_blend,
        });
    }
    if s.stalled {
        out.push(Effect::StallScaling {
            severity_f8: s.severity_f8,
            controls: s.stall_controls,
            lift: s.stall_lift,
        });
    }
    if s.spin_controls < 1. {
        out.push(Effect::SpinControls {
            effectiveness: s.spin_controls,
            spin_blend: s.spin_blend,
        });
    }
    if let Some(dep) = t.departure {
        if let Some(check) = dep.spin_check {
            out.push(Effect::SpinCheck(check));
        }
        if let Some(exit) = dep.spin_exit {
            out.push(Effect::SpinEnded {
                exit,
                mode: dep.mode,
            });
        }
        if dep.on_ground && (dep.mode_before != DepartureMode::Normal || dep.spinning_before != 0) {
            out.push(Effect::DepartureCleared {
                mode_before: dep.mode_before,
            });
        }
    }
    let r = t.rotation;
    if let Some(trim) = r.low_speed_trim {
        out.push(Effect::LowSpeedTrim {
            trim,
            model_trim_rad: r.model_trim_rad,
            trim_rad: r.trim_rad,
        });
    }
    if r.ground_steering_yaw != 0. {
        out.push(Effect::GroundSteering {
            yaw_rate: r.ground_steering_yaw,
        });
    }
    let drag = f.drag;
    if drag.gear_on_wheels && d.gear.position > 0. {
        out.push(Effect::GearDragOnWheels {
            gear: d.gear.position,
        });
    }
    if hybrid && drag.device_fraction < 1. && (d.flaps.position > 0. || d.airbrake.position > 0.) {
        out.push(Effect::DeviceDragScaled {
            fraction: drag.device_fraction,
        });
    }
    if let Some(cap) = drag.cap_lbf
        && drag.uncapped_lbf > cap
    {
        out.push(Effect::DragCapped {
            cap_lbf: cap,
            uncapped_lbf: drag.uncapped_lbf,
        });
    }
    if let Some(from_fps) = f.speed_capped_from_fps {
        out.push(Effect::SpeedCapped { from_fps });
    }
}

/// Holds the trace on `State` without taking part in its equality: two
/// states that will behave the same compare equal whatever explains their
/// last step, so no comparison of states ever reads the trace.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Slot(pub(crate) FlightTrace);
impl PartialEq for Slot {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attitude::Basis;
    use crate::flight::integration_tests::profile;
    use crate::flight::{PilotCommand, PilotInput, State, Switch};
    use crate::models::FlightModel;
    use crate::research::Surface;

    const KNOT: f64 = crate::runway_wind::FEET_PER_SECOND_PER_KNOT;

    fn airborne(hybrid: bool, altitude: f64, speed: f64) -> State {
        let mut s = State::new(&profile(), [0., altitude, 0.]).unwrap();
        if hybrid {
            s.enable_research(1).unwrap();
        }
        s.yaw = 0.;
        s.speed = speed;
        s.velocity = Basis::new(0., 0., 0.).forward.map(|axis| axis * speed);
        s
    }

    fn step(s: &mut State, input: PilotInput) {
        s.step_surface(&input, |_, _| Surface::terrain(0.));
    }

    fn adapter(s: &State) -> AdapterTrace {
        s.trace().adapter.expect("the legacy or hybrid adapter ran")
    }

    fn find<T>(s: &State, pick: impl Fn(&Effect) -> Option<T>) -> Option<T> {
        s.trace().effects().iter().find_map(pick)
    }

    fn has(s: &State, wanted: impl Fn(&Effect) -> bool) -> bool {
        s.trace().effects().iter().any(wanted)
    }

    #[test]
    fn each_step_starts_a_clean_record_and_stamps_its_path_and_tick() {
        let mut s = airborne(true, 5000., 400.);
        assert_eq!(s.trace().path, Path::NotStepped);
        assert!(s.trace().effects().is_empty());
        step(&mut s, PilotInput::default());
        assert_eq!(s.trace().path, Path::Hybrid);
        assert_eq!(s.trace().tick, s.ticks);
        assert!(adapter(&s).departure.is_some());
        // Turbulence applied after a step belongs to that step's record
        // until the next step begins.
        let gust = Disturbance {
            vertical_fps: 2.,
            ..Default::default()
        };
        s.apply_turbulence(gust);
        assert!(has(&s, |e| *e == Effect::Turbulence(gust)));
        step(&mut s, PilotInput::default());
        assert_eq!(s.trace().turbulence, None);

        let mut legacy = airborne(false, 5000., 400.);
        step(&mut legacy, PilotInput::default());
        assert_eq!(legacy.trace().path, Path::Legacy);
        assert!(adapter(&legacy).departure.is_none());
        assert_eq!(legacy.trace().contact, None);

        let mut wreck = airborne(true, 5000., 400.);
        wreck.crashed = true;
        step(&mut wreck, PilotInput::default());
        assert_eq!(wreck.trace().effects(), [Effect::NotFlying(Path::Wreck)]);
        assert!(wreck.trace().adapter.is_none() && wreck.trace().controls.is_none());
    }

    #[test]
    fn identical_runs_record_identical_traces() {
        // Equal records also prove the trace never holds a NaN.
        let airborne_run = |hybrid: bool| {
            let mut s = airborne(hybrid, 3000., 250.);
            s.damage_regions[4] = 0.3;
            s.systems.hit(21, s.throttle);
            (0..600)
                .map(|tick| {
                    step(
                        &mut s,
                        PilotInput {
                            pitch: if tick < 300 { 1. } else { -0.5 },
                            roll: 0.3,
                            yaw: if tick > 200 { 1. } else { 0. },
                            throttle: Some(if tick < 300 { 0. } else { 1. }),
                            ..Default::default()
                        },
                    );
                    *s.trace()
                })
                .collect::<Vec<_>>()
        };
        let runway_run = || {
            let windy = |_: f64, _: f64| Surface {
                wind: [15., 0., -6.],
                ..Surface::runway(1024.)
            };
            let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
            s.enable_research(1).unwrap();
            s.start_on_runway([0., 1024., 0.], 0.).unwrap();
            (0..600)
                .map(|tick| {
                    let input = PilotInput {
                        throttle: Some(if tick < 60 { 0. } else { 1. }),
                        pitch: if tick > 400 { 0.5 } else { 0. },
                        commands: if tick == 60 {
                            vec![PilotCommand::Set(Switch::Airbrake, false)]
                        } else {
                            Vec::new()
                        },
                        ..Default::default()
                    };
                    s.step_surface(&input, windy);
                    *s.trace()
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(airborne_run(false), airborne_run(false));
        assert_eq!(airborne_run(true), airborne_run(true));
        assert_eq!(runway_run(), runway_run());
    }

    #[test]
    fn the_trace_takes_no_part_in_state_equality() {
        let mut s = airborne(true, 5000., 400.);
        step(&mut s, PilotInput::default());
        let before = s.clone();
        s.trace.0 = FlightTrace::default();
        assert_eq!(s, before);
        assert_ne!(s.trace(), before.trace());
    }

    #[test]
    fn a_stall_scales_controls_and_lift_and_records_the_spin_check() {
        // The fixture's clean stall speed at 5,000 ft is 205 ft/s.
        let mut s = airborne(true, 5000., 150.);
        s.research.as_mut().unwrap().departure.mode = DepartureMode::Stalled;
        step(
            &mut s,
            PilotInput {
                pitch: 1.,
                ..Default::default()
            },
        );
        let a = adapter(&s);
        assert!(a.scaling.stalled);
        assert_eq!(s.maneuver.lift_g, a.lift.lagged_g * a.scaling.stall_lift);
        assert!(has(&s, |e| matches!(
            e,
            Effect::StallScaling { controls, lift, .. }
                if *controls == a.scaling.stall_controls && *lift == a.scaling.stall_lift
        )));
        let authority = find(&s, |e| match e {
            Effect::LowSpeedAuthority { authority, .. } => Some(*authority),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            authority,
            (a.air.airspeed_fps / a.envelope.stall_fps).powi(2)
        );
        assert!(authority < 0.6);
        let check = a.departure.unwrap().spin_check.unwrap();
        assert!(!check.entered, "no rudder, so no spin");
        assert!(
            check.coin.is_some(),
            "wings level with no roll: a random draw picked the direction"
        );
    }

    #[test]
    fn gear_flap_and_airbrake_drag_parts_add_up_to_the_drag_applied() {
        for hybrid in [false, true] {
            let mut s = airborne(hybrid, 5000., 300.);
            (
                s.gear,
                s.gear_down,
                s.flaps,
                s.flaps_down,
                s.brake,
                s.brake_out,
            ) = (1., true, 1., true, 1., true);
            step(&mut s, PilotInput::default());
            let a = adapter(&s);
            let d = a.forces.drag;
            let w = a.forces.weight_lbs;
            let c = s.model().configuration();
            assert_eq!(d.gear_lbf, w * c.native.drag.gear as f64 / 256.);
            assert_eq!(
                d.flaps_lbf,
                w * c.native.drag.flaps as f64 * d.device_fraction / 256.
            );
            assert_eq!(
                d.airbrake_lbf,
                w * c.native.drag.airbrake as f64 * d.device_fraction / 256.
            );
            let parts = d.airframe_lbf
                + d.load_lbf
                + d.pull_lbf
                + d.gear_lbf
                + d.flaps_lbf
                + d.airbrake_lbf
                + d.slip_lbf;
            assert!((parts - d.undamaged_lbf).abs() <= 1e-9 * d.undamaged_lbf);
            assert_eq!(d.uncapped_lbf, d.undamaged_lbf, "no damage");
            let e = a.envelope;
            if hybrid {
                assert_eq!(d.device_fraction, a.lift.drag_percent / 100.);
                assert!(d.device_fraction < 1.);
                assert!(has(&s, |e| matches!(e, Effect::DeviceDragScaled { .. })));
                assert_eq!(e.stall_fps, e.clean_stall_fps * 0.75);
                assert!(has(&s, |effect| matches!(
                    effect,
                    Effect::FlapStallSpeed { flaps, .. } if *flaps == 1.
                )));
                let factor = find(&s, |e| match e {
                    Effect::FlapLift { factor, .. } => Some(*factor),
                    _ => None,
                })
                .unwrap();
                assert_eq!(factor, 1. + a.lift.flap_lift_f8 / 256.);
                assert!(factor > 1.);
            } else {
                assert_eq!(d.device_fraction, 1.);
                assert_eq!(e.stall_fps, e.clean_stall_fps);
                assert_eq!(a.lift.flap_factor, 1.);
                assert!(!has(&s, |e| matches!(e, Effect::FlapLift { .. })));
            }
        }
    }

    #[test]
    fn regional_damage_records_the_penalties_it_applied() {
        let mut s = airborne(true, 5000., 400.);
        s.damage_regions[3] = 0.5;
        s.damage_regions[5] = 0.2;
        step(
            &mut s,
            PilotInput {
                pitch: 0.2,
                roll: 0.5,
                ..Default::default()
            },
        );
        let a = adapter(&s);
        let r = a.regional;
        assert_eq!(r.damage, [0.5, 0., 0.2]);
        assert_eq!(
            r.effects,
            crate::aircraft_systems::regional_effects(s.damage_regions)
        );
        assert!((r.effects.lift - (1. - 0.35 * 0.5 - 0.2 * 0.2)).abs() < 1e-12);
        assert!((r.effects.authority[1] - 0.7).abs() < 1e-12);
        assert!((r.effects.roll_bias + 0.175).abs() < 1e-12);
        assert!((r.effects.drag_percent - 15.5).abs() < 1e-12);
        let output = s.trace().controls.unwrap().output;
        assert_eq!(r.commands, r.effects.commands(output));
        let drag = a.forces.drag;
        assert_eq!(drag.damage_percent, r.effects.drag_percent);
        assert_eq!(
            drag.uncapped_lbf,
            drag.undamaged_lbf * (1. + r.effects.drag_percent / 100.)
        );
        assert!(has(&s, |e| *e == Effect::RegionalDamage(r)));
    }

    #[test]
    fn the_low_speed_ceiling_names_the_ramp_it_applied() {
        // With full flaps the effective stall speed at 1,024 ft is about 151
        // ft/s and the next envelope row starts at about 201 ft/s.
        let mut s = airborne(true, 1024., 180.);
        (s.flaps, s.flaps_down) = (1., true);
        step(
            &mut s,
            PilotInput {
                pitch: 1.,
                ..Default::default()
            },
        );
        let a = adapter(&s);
        let e = a.envelope;
        let ceiling = e.low_speed_ceiling.unwrap();
        assert_eq!(
            Some(ceiling.limit_g),
            crate::flight::low_speed_positive_g_ceiling(
                s.model().configuration(),
                a.air.altitude_ft,
                a.air.airspeed_fps,
                e.stall_fps,
                false,
            )
        );
        assert_eq!(ceiling.from_fps, e.stall_fps);
        assert!(ceiling.to_fps > a.air.airspeed_fps && ceiling.fraction > 0.);
        assert_eq!(ceiling.limit_g, 1. + ceiling.fraction * (ceiling.to_g - 1.));
        assert_eq!(e.limits_g[1], ceiling.limit_g / e.load_divisor);
        assert!(has(&s, |effect| matches!(
            effect,
            Effect::LowSpeedCeiling { positive_limit_g, .. } if *positive_limit_g == e.limits_g[1]
        )));
    }

    #[test]
    fn pull_extra_g_raises_the_positive_limit_to_nine() {
        for extra_g in [false, true] {
            let mut s = airborne(true, 5000., 600.);
            s.cheats.extra_g = extra_g;
            step(
                &mut s,
                PilotInput {
                    pitch: 1.,
                    ..Default::default()
                },
            );
            let e = adapter(&s).envelope;
            // Every fixture envelope row reaches 6 G; fuel and stores divide it.
            assert_eq!(e.envelope_g[1], 6.);
            assert_eq!(e.loaded_positive_g, 6. / e.load_divisor);
            assert!(has(&s, |effect| matches!(
                effect,
                Effect::LoadedLimits { divisor, .. } if *divisor == e.load_divisor
            )));
            let raised = find(&s, |effect| match effect {
                Effect::ExtraG { from_g, limit_g } => Some((*from_g, *limit_g)),
                _ => None,
            });
            if extra_g {
                assert_eq!(e.limits_g[1], 9.);
                assert_eq!(raised, Some((e.loaded_positive_g, 9.)));
            } else {
                assert_eq!(e.limits_g[1], e.loaded_positive_g);
                assert_eq!(raised, None);
            }
        }
    }

    #[test]
    fn runway_crosswind_reduces_tire_grip() {
        // The fixture weighs 15,000 lb at most: crosswind limits 8, 14 and 20
        // knots, so 17 knots is three quarters of the way to the limit.
        let crosswind = |_: f64, _: f64| Surface {
            wind: [17. * KNOT, 0., 0.],
            ..Surface::runway(1024.)
        };
        let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.start_on_runway([0., 1024., 0.], 0.).unwrap();
        s.step_surface(&PilotInput::default(), crosswind);
        assert!(has(&s, |e| *e
            == Effect::Parked {
                attitude: true,
                position: true
            }));
        assert_eq!(adapter(&s).runway_wind.unwrap().ground_motion, 0.);
        (s.brake_out, s.throttle) = (false, 1.);
        for _ in 0..600 {
            s.step_surface(&PilotInput::default(), crosswind);
            if adapter(&s).air.ground_speed_fps > 10. * KNOT {
                break;
            }
        }
        // The aircraft weathervanes a little, so the crosswind against its
        // heading is just under 17 knots.
        let wind = adapter(&s).runway_wind.unwrap();
        let a = wind.assessment;
        assert!((a.crosswind_knots - 17.).abs() < 0.1);
        assert!(
            (a.crosswind_fraction - (0.5 + 0.5 * (a.crosswind_knots.abs() - 14.) / 6.)).abs()
                < 1e-12
        );
        assert_eq!(wind.ground_motion, 1.);
        assert_eq!(
            wind.fraction,
            a.crosswind_fraction.max(a.tailwind_fraction) * wind.ground_motion
        );
        assert!((wind.fraction - 0.75).abs() < 0.01);
        let Some(Contact::Rolling(rolling)) = s.trace().contact else {
            panic!("still on the wheels");
        };
        assert_eq!(rolling.wind_grip, 1. - 0.5 * wind.fraction);
        assert!(!rolling.brakes && rolling.touchdown.is_none());
        assert!(has(&s, |e| *e == Effect::RunwayWind(wind)));
        assert!(has(&s, |e| matches!(e, Effect::GearDragOnWheels { .. })));
    }

    #[test]
    fn hydraulic_loss_freezes_the_surfaces_and_holds_the_devices() {
        let mut s = airborne(true, 5000., 400.);
        step(
            &mut s,
            PilotInput {
                pitch: 0.5,
                ..Default::default()
            },
        );
        s.systems.fluids.hydraulic = 0.;
        let held = [s.elevator, s.aileron, s.rudder];
        step(
            &mut s,
            PilotInput {
                pitch: -1.,
                roll: 1.,
                commands: vec![PilotCommand::Set(Switch::Gear, true)],
                ..Default::default()
            },
        );
        let controls = s.trace().controls.unwrap();
        assert_eq!(controls.requested, [-1., 1., 0.]);
        assert_eq!(controls.output, held);
        assert_eq!([s.elevator, s.aileron, s.rudder], held);
        assert!(has(&s, |e| *e == Effect::HydraulicsLost));
        let gear = find(&s, |e| match e {
            Effect::DeviceHeld {
                device: DeviceKind::Gear,
                state,
            } => Some(*state),
            _ => None,
        })
        .unwrap();
        assert_eq!(
            (gear.commanded, gear.position, gear.blocked),
            (true, 0., Some(Block::NoHydraulics))
        );
    }

    #[test]
    fn damaged_controls_and_wing_record_their_factors() {
        let fly = |hits: &[usize]| {
            let mut s = airborne(true, 5000., 500.);
            for hit in hits {
                s.systems.hit(*hit, s.throttle);
            }
            step(
                &mut s,
                PilotInput {
                    pitch: 0.8,
                    ..Default::default()
                },
            );
            s
        };
        let healthy = fly(&[]);
        let damaged = fly(&[19, 25]);
        let controls = damaged.trace().controls.unwrap();
        assert_eq!(controls.condition.authority, [0.5, 1., 1.]);
        assert_eq!(controls.output[0], 0.4);
        assert!(has(&damaged, |e| *e == Effect::ControlResponse(controls)));
        assert!(has(&damaged, |e| *e == Effect::WingDamaged));
        assert!(!has(&healthy, |e| matches!(
            e,
            Effect::ControlResponse(_) | Effect::WingDamaged
        )));
        // Half the stick, then half the lift from the damaged wing.
        let stick = |s: &State| adapter(s).envelope.stick_g;
        assert!(stick(&damaged) < stick(&healthy));
        assert_eq!(
            adapter(&damaged).lift.commanded_g,
            stick(&damaged) * adapter(&damaged).lift.flap_factor * 0.5
        );
    }

    #[test]
    fn throttle_fuel_and_afterburner_effects_name_their_cause() {
        let mut jammed = airborne(true, 5000., 400.);
        jammed.systems.hit(29, jammed.throttle);
        step(
            &mut jammed,
            PilotInput {
                throttle_rate: 1.,
                ..Default::default()
            },
        );
        assert_eq!(
            jammed.trace().throttle_lock,
            Some(ThrottleLock {
                held_at: 0.7,
                ignored_rate: 1.,
                ignored_setting: false,
            })
        );
        assert_eq!(jammed.throttle, 0.7);

        let mut burner = airborne(true, 5000., 400.);
        (burner.burner, burner.throttle) = (true, 0.5);
        step(&mut burner, PilotInput::default());
        assert!(has(&burner, |e| matches!(
            e,
            Effect::AfterburnerBlocked {
                cause: BurnerBlock::ThrottleLow,
                ..
            }
        )));

        let mut empty = airborne(true, 5000., 400.);
        empty.fuel = 0.;
        step(&mut empty, PilotInput::default());
        let power = adapter(&empty).power;
        assert!(power.fuel_starved && !power.engine);
        assert_eq!((power.thrust_lbf, power.fuel_flow_lbs_per_second), (0., 0.));
        assert!(has(&empty, |e| *e == Effect::FuelStarved));

        let mut unlimited = airborne(true, 5000., 400.);
        unlimited.cheats.unlimited_fuel = true;
        step(&mut unlimited, PilotInput::default());
        let power = adapter(&unlimited).power;
        assert_eq!(
            power.thrust_lbf,
            power.rated_thrust_lbf * power.lapse * power.power_available
        );
        assert!(has(&unlimited, |e| *e
            == Effect::UnlimitedFuel {
                fuel_flow_lbs_per_second: power.fuel_flow_lbs_per_second,
            }));
    }

    #[test]
    fn the_autopilot_rewrite_and_its_release_are_recorded() {
        let mut s = airborne(true, 5000., 400.);
        s.command(PilotCommand::Toggle(Switch::Autopilot));
        s.yaw = 0.5;
        step(&mut s, PilotInput::default());
        let record = s.trace().autopilot.unwrap();
        assert_eq!(record.mode, crate::autopilot::Mode::Heading);
        assert_eq!(record.pilot, [0.; 3]);
        assert_ne!(record.commanded, record.pilot);
        assert_eq!(record.released, None);
        assert!(has(&s, |e| matches!(e, Effect::Autopilot { .. })));
        step(
            &mut s,
            PilotInput {
                roll: 0.5,
                ..Default::default()
            },
        );
        let record = s.trace().autopilot.unwrap();
        assert_eq!(record.released, Some(Release::PilotOverride));
        assert_eq!(record.commanded, record.pilot);
        step(&mut s, PilotInput::default());
        assert_eq!(s.trace().autopilot, None, "off since the override");

        s.command(PilotCommand::Toggle(Switch::Autopilot));
        s.damage_regions[4] = 0.1;
        step(&mut s, PilotInput::default());
        assert_eq!(
            s.trace().autopilot.unwrap().released,
            Some(Release::AirframeDamage)
        );
    }

    #[test]
    fn a_blast_kick_and_the_jolt_that_follows_are_recorded() {
        let mut s = airborne(true, 5000., 400.);
        step(&mut s, PilotInput::default());
        let blast = [s.position[0] + 30., s.position[1] - 40., s.position[2]];
        s.jolt_from(blast, 1.);
        let kick = s.trace().blast.unwrap();
        assert_eq!((kick.strength, kick.push_fps), (1., 15.));
        assert_eq!(kick.kick, s.jolt);
        let rates = s.jolt;
        step(&mut s, PilotInput::default());
        assert_eq!(s.trace().blast, None);
        assert!(has(&s, |e| *e == Effect::Jolt { rates }));
    }

    #[test]
    fn legacy_floor_contact_and_the_speed_cap_are_recorded() {
        for no_crashes in [false, true] {
            let mut s = airborne(false, 1., 400.);
            s.cheats.no_crashes = no_crashes;
            s.velocity[1] = -100.;
            step(&mut s, PilotInput::default());
            assert_eq!(
                s.trace().contact,
                Some(Contact::LegacyFloor {
                    bounced: no_crashes
                })
            );
            assert_eq!(s.crashed, !no_crashes);
        }
        let mut fast = airborne(false, 30_000., 6_500.);
        step(&mut fast, PilotInput::default());
        let capped = adapter(&fast).forces.speed_capped_from_fps.unwrap();
        assert!(capped > 6_000.);
        assert_eq!(fast.speed, 6_000.);
        assert!(has(&fast, |e| *e == Effect::SpeedCapped { from_fps: capped }));
    }
}
