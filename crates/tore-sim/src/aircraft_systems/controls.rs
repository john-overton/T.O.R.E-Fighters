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
impl Controls {
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
