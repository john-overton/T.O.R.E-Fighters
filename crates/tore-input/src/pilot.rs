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
