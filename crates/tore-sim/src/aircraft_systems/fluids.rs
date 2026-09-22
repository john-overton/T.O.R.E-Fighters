use crate::flight::DT;
#[derive(Clone, Debug, PartialEq)]
pub struct Fluids {
    pub oil: f64,
    pub hydraulic: f64,
    pump_hits: u8,
    oil_leaks: u8,
    hydraulic_leaks: u8,
    oil_warning: bool,
    hydraulic_warning: bool,
}
impl Default for Fluids {
    fn default() -> Self {
        Self {
            oil: 1.,
            hydraulic: 1.,
            pump_hits: 0,
            oil_leaks: 0,
            hydraulic_leaks: 0,
            oil_warning: false,
            hydraulic_warning: false,
        }
    }
}
impl Fluids {
    pub fn oil_pressure(&self) -> f64 {
        self.oil * 0.5f64.powi(i32::from(self.pump_hits))
    }
    pub fn hit(&mut self, index: usize) {
        match index {
            12 => self.pump_hits = self.pump_hits.saturating_add(1),
            13 => self.oil_leaks = self.oil_leaks.saturating_add(1),
            14 => self.hydraulic_leaks = self.hydraulic_leaks.saturating_add(1),
            15 => self.hydraulic = 0.,
            _ => {}
        }
    }
    pub fn advance(&mut self) -> Vec<&'static str> {
        self.oil = (self.oil - f64::from(self.oil_leaks) * 0.01 * DT).max(0.);
        self.hydraulic = (self.hydraulic - f64::from(self.hydraulic_leaks) * 0.01 * DT).max(0.);
        let mut messages = Vec::new();
        if self.oil_pressure() <= 0.25 && !self.oil_warning {
            self.oil_warning = true;
            messages.push("Oil pressure critical");
        }
        if self.hydraulic <= 0.25 && !self.hydraulic_warning {
            self.hydraulic_warning = true;
            messages.push("Hydraulic pressure critical");
        }
        messages
    }
}
