use crate::flight::DT;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Fuel {
    pub external: [f64; 9],
    initial_external: f64,
    leaks: u8,
}
impl Fuel {
    pub fn new(external: [f64; 9]) -> Self {
        Self {
            initial_external: external.iter().sum(),
            external,
            leaks: 0,
        }
    }
    pub fn external_lbs(&self) -> f64 {
        self.external.iter().sum()
    }
    pub fn used_lbs(&self) -> f64 {
        self.initial_external - self.external_lbs()
    }
    pub fn hit(&mut self, index: usize) {
        if index == 1 {
            self.leaks = self.leaks.saturating_add(1);
        }
    }
    pub fn advance(&self, internal: &mut f64) {
        *internal = (*internal - f64::from(self.leaks) * 4. * DT).max(0.);
    }
    pub fn consume(&mut self, internal: &mut f64, mut pounds: f64) {
        if !pounds.is_finite() || pounds <= 0. {
            return;
        }
        for fuel in &mut self.external {
            let debit = fuel.min(pounds);
            *fuel -= debit;
            pounds -= debit;
        }
        *internal = (*internal - pounds).max(0.);
    }
}
