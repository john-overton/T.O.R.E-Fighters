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
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PilotCommand {
    Eject,
    Toggle(Switch),
    Set(Switch, bool),
    Throttle(f64),
    AdjustThrottle(f64),
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PilotInput {
    pub pitch: f64,
    pub roll: f64,
    pub yaw: f64,
    /// Keyboard/encoder rate request; model retains its authored rate.
    pub throttle_rate: f64,
    pub throttle: Option<f64>,
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
            commands: self.commands.clone(),
        }
    }
}
