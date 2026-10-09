//! Device-independent, tick-owned pilot commands. Positive pitch pulls up.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Switch {
    Gear,
    Flaps,
    Airbrake,
    Hook,
    Bay,
    Engine,
    Burner,
    Radar,
    Jammer,
    Autopilot,
    WaypointAutopilot,
    /// Hover hold, the autopilot mode of the helicopters and the V-22 (VTOL
    /// overhaul, slice P9). Refused, with a message, on every other aircraft.
    HoverHold,
}
/// Extra powered-lift demands. Positions are 0..1 except signed vector yaw.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FlightAxis {
    VectorPitch,
    VectorYaw,
    Conversion,
    Collective,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PilotCommand {
    Eject,
    Toggle(Switch),
    Set(Switch, bool),
    Throttle(f64),
    AdjustThrottle(f64),
    SetAxis(FlightAxis, f64),
    AdjustAxis(FlightAxis, f64),
    NeutralVector,
    /// The powered-lift aircraft's discrete controls: stability level, trim
    /// and nozzle steps (one wire command code with a sub-code).
    Lift(LiftCommand),
}
/// How much stability augmentation a powered-lift aircraft flies with. Every
/// level is limited-authority feedback on top of the pilot's inputs; Damper
/// is the default (John, 2026-10-08).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum StabilityLevel {
    /// No augmentation: natural rotor and aerodynamic damping only.
    Off,
    /// Rate damping, torque feed-forward and turn coordination.
    #[default]
    Damper,
    /// Damper plus attitude command about the trimmed attitude.
    Attitude,
}
impl StabilityLevel {
    pub const ALL: [Self; 3] = [Self::Off, Self::Damper, Self::Attitude];
    /// The level the cycle key moves to: Off, Damper, Attitude, then Off.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Damper,
            Self::Damper => Self::Attitude,
            Self::Attitude => Self::Off,
        }
    }
}
/// The axes of the cyclic and pedal trim, in stick travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TrimAxis {
    Pitch,
    Roll,
    Pedal,
}
/// The vectoring jets' nozzle presets.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum NozzlePreset {
    /// Shift+Z: nozzles aft (0 degrees), or from the braking stop to vertical.
    Forward,
    /// Shift+X: nozzles vertical (90 degrees), or from vertical to the
    /// braking stop.
    Vertical,
}
/// The trim keys (design 5.3). A press moves the trim a tap's worth at
/// once; held past the delay it then moves at the rate, in steps a few ticks
/// apart, so taps are an exact 2 percent and a held key about 10 percent a
/// second. The resolver turns the trim rate axes into
/// [`LiftCommand::TrimAdjust`] commands on these numbers. Stepping at 20 Hz
/// rather than every tick keeps a held key to 20 commands a second on the
/// wire (fitted, agent decision 2026-10-08).
pub mod trim_keys {
    /// One tap, share of full travel.
    pub const TAP: f64 = 0.02;
    /// The held rate, share of full travel a second.
    pub const RATE_PER_SECOND: f64 = 0.1;
    /// Ticks a key is held before the rate starts (0.2 s).
    pub const DELAY_TICKS: u32 = 24;
    /// Ticks between the held rate's steps (20 Hz).
    pub const STEP_TICKS: u32 = 6;
    /// The simulation tick, s (the fixed 120 Hz).
    pub const TICK_SECONDS: f64 = 1. / 120.;
    /// The trim one tick of the held rate adds at full deflection.
    pub const PER_TICK: f64 = RATE_PER_SECOND * TICK_SECONDS;
    /// One step of the held rate at full deflection.
    pub const STEP: f64 = PER_TICK * STEP_TICKS as f64;
}
/// The powered-lift pilot commands of the VTOL overhaul (design section 5).
/// Each is applied once, at the start of its tick, on the aircraft it suits;
/// the others ignore it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LiftCommand {
    /// Select a stability level.
    SetStability(StabilityLevel),
    /// Move to the next stability level (Ctrl+Shift+A).
    CycleStability,
    /// Trim set: the current stick plus trim becomes the trim.
    TrimSet,
    /// Move the trim on one axis by a signed share of full travel, -1..1.
    TrimAdjust(TrimAxis, f64),
    /// Return the trim to centre.
    TrimCentre,
    /// One 10-degree nozzle step, down (X) or up (Z).
    NozzleStep { down: bool },
    /// A nozzle preset (Shift+Z, Shift+X).
    NozzlePreset(NozzlePreset),
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PilotInput {
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
    /// Keyboard/encoder rate request; model retains its authored rate.
    pub throttle_rate: f64,
    pub throttle: Option<f64>,
    pub vector_pitch_rate: f64,
    pub vector_yaw_rate: f64,
    pub conversion_rate: f64,
    pub collective_rate: f64,
    pub vector_pitch: Option<f64>,
    pub vector_yaw: Option<f64>,
    pub conversion: Option<f64>,
    pub collective: Option<f64>,
    /// Ordered, consumed once at the start of this tick.
    pub commands: Vec<PilotCommand>,
}
pub fn bipolar(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(-1., 1.)
    } else {
        0.
    }
}
impl PilotInput {
    pub fn bounded(&self) -> Self {
        Self {
            pitch: bipolar(self.pitch),
            roll: bipolar(self.roll),
            yaw: bipolar(self.yaw),
            throttle_rate: bipolar(self.throttle_rate),
            throttle: self
                .throttle
                .filter(|v| v.is_finite())
                .map(|v| v.clamp(0., 1.)),
            vector_pitch_rate: bipolar(self.vector_pitch_rate),
            vector_yaw_rate: bipolar(self.vector_yaw_rate),
            conversion_rate: bipolar(self.conversion_rate),
            collective_rate: bipolar(self.collective_rate),
            vector_pitch: position(self.vector_pitch),
            vector_yaw: self.vector_yaw.filter(|v| v.is_finite()).map(bipolar),
            conversion: position(self.conversion),
            collective: position(self.collective),
            commands: self.commands.clone(),
        }
    }
}

fn position(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite()).map(|v| v.clamp(0., 1.))
}
