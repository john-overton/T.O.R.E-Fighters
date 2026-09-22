use crate::flight::DT;
#[derive(Clone, Debug, PartialEq)]
pub struct Engine {
    pub temperature: f64,
    pub power: f64,
    pub flameout: f64,
    count: u8,
    shutdown: [bool; 4],
    serious: bool,
    compressor_hits: u8,
    compressor_exposure: f64,
    restart_armed: bool,
    heat_warning: bool,
}
impl Default for Engine {
    fn default() -> Self {
        Self::new(1)
    }
}
impl Engine {
    pub fn new(count: u8) -> Self {
        Self {
            temperature: 0.,
            power: 1.,
            flameout: 0.,
            count: count.clamp(1, 4),
            shutdown: [false; 4],
            serious: false,
            compressor_hits: 0,
            compressor_exposure: 0.,
            restart_armed: false,
            heat_warning: false,
        }
    }
    pub fn count(&self) -> u8 {
        self.count
    }
    pub fn thrust_shares(&self, count: u8) -> [f64; 4] {
        let count = usize::from(count.clamp(1, 4));
        let running = (0..count).filter(|i| !self.shutdown[*i]).count();
        std::array::from_fn(|i| {
            if i < count && !self.shutdown[i] && running > 0 {
                self.available() / running as f64
            } else {
                0.
            }
        })
    }
    pub fn available(&self) -> f64 {
        if self.flameout > 0. { 0. } else { self.power }
    }
    pub fn hit(&mut self, index: usize) {
        match index {
            2 => self.power = 0.,
            4 => {
                self.flameout = 6.;
                self.restart_armed = false;
            }
            5 => self.power = (self.power - 0.25).max(0.),
            6 => {
                self.power = self.power.min(0.25);
                self.serious = true;
            }
            7 => {
                self.power = (self.power - 0.25).max(0.);
                self.compressor_hits = self.compressor_hits.saturating_add(1);
            }
            9 | 10 => {
                let preferred = if index == 9 {
                    0
                } else {
                    usize::from(self.count - 1)
                };
                let slot = if !self.shutdown[preferred] {
                    Some(preferred)
                } else {
                    (0..usize::from(self.count)).find(|i| !self.shutdown[*i])
                };
                if let Some(slot) = slot {
                    self.shutdown[slot] = true;
                    self.power = (self.power - 1. / f64::from(self.count)).max(0.);
                }
            }
            _ => {}
        }
    }
    pub fn advance(
        &mut self,
        running: bool,
        throttle: f64,
        oil_pressure: f64,
        fire: bool,
    ) -> Vec<&'static str> {
        let mut messages = Vec::new();
        if self.serious {
            self.power = (self.power - 0.005 * DT).max(0.);
        }
        if running && self.compressor_hits > 0 && throttle > 0.25 {
            self.compressor_exposure += DT * f64::from(self.compressor_hits);
            if self.compressor_exposure >= 30. && self.power > 0. {
                self.power = 0.;
                messages.push("Compressor failed: engine power lost");
            }
        }
        let heat = if fire {
            10.
        } else if running && oil_pressure < 1. {
            (1. - oil_pressure) * (0.1 + 6. * throttle)
        } else {
            -2.
        };
        self.temperature = (self.temperature + heat * DT).clamp(0., 100.);
        if self.temperature >= 100. && self.power > 0. {
            self.power = 0.;
            messages.push("Engine failed from overheating");
        }
        if self.temperature >= 75. && !self.heat_warning {
            self.heat_warning = true;
            messages.push("Engine overheating: reduce throttle");
        }
        if self.flameout > 0. {
            self.flameout = (self.flameout - DT).max(f64::EPSILON);
            self.restart_armed |= throttle <= 0.25;
            if self.flameout <= DT && self.restart_armed && throttle > 0.25 {
                self.flameout = 0.;
                messages.push("Engine restarted");
            }
        }
        messages
    }
}
