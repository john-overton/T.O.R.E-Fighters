use crate::flight::DT;
#[derive(Clone, Debug, PartialEq)]
pub struct Controls {
    pub throttle_lock: Option<f64>,
    authority: [f64; 3],
    bias: [f64; 3],
    damaged_linkage: bool,
    unstable: bool,
}
impl Default for Controls {
    fn default() -> Self {
        Self {
            throttle_lock: None,
            authority: [1.; 3],
            bias: [0.; 3],
            damaged_linkage: false,
            unstable: false,
        }
    }
}
/// Read-only view of the control-run damage that shapes the stick response.
/// Axes are [pitch, roll, yaw].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ControlCondition {
    /// Multiplier per axis. Each hit on the elevator, ailerons or rudder
    /// (systems 19, 21 and 23) halves its axis.
    pub authority: [f64; 3],
    /// Offset per axis added to the stick. A bent elevator, aileron or rudder
    /// (systems 20, 22 and 24) sets 0.25 (-0.25 for the ailerons); a damaged
    /// rudder (23) adds 0.1 yaw when there is no yaw offset yet.
    pub bias: [f64; 3],
    /// Control linkage damaged (system 27): every axis x0.3.
    pub damaged_linkage: bool,
    /// Flight controls unstable (system 28): every axis oscillates by up to
    /// 0.2 at 2 Hz.
    pub unstable: bool,
}
impl Default for ControlCondition {
    fn default() -> Self {
        Controls::default().condition()
    }
}
impl Controls {
    /// The current damage state, for telemetry. Reading it changes nothing.
    pub fn condition(&self) -> ControlCondition {
        ControlCondition {
            authority: self.authority,
            bias: self.bias,
            damaged_linkage: self.damaged_linkage,
            unstable: self.unstable,
        }
    }
    pub fn hit(&mut self, index: usize, throttle: f64) {
        match index {
            19 | 21 | 23 => {
                let axis = (index - 19) / 2;
                self.authority[axis] *= 0.5;
                if index == 23 && self.bias[2] == 0. {
                    self.bias[2] = 0.1;
                }
            }
            20 | 22 | 24 => {
                let axis = (index - 20) / 2;
                self.bias[axis] = if axis == 1 { -0.25 } else { 0.25 };
            }
            27 => self.damaged_linkage = true,
            28 => self.unstable = true,
            29 => self.throttle_lock = Some(throttle),
            _ => {}
        }
    }
    pub fn response(
        &self,
        requested: [f64; 3],
        held: [f64; 3],
        tick: u64,
        pressure: f64,
    ) -> [f64; 3] {
        if pressure <= 0. {
            return held;
        }
        let unstable = if self.unstable {
            0.2 * (tick as f64 * DT * std::f64::consts::TAU * 2.).sin()
        } else {
            0.
        };
        std::array::from_fn(|axis| {
            ((requested[axis] * self.authority[axis] + self.bias[axis] + unstable)
                * pressure
                * if self.damaged_linkage { 0.3 } else { 1. })
            .clamp(-1., 1.)
        })
    }
}
