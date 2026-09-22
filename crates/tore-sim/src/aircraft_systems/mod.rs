//! Component-owned ownship damage. See docs/spec/systems-damage.md.
mod controls;
mod engine;
mod fluids;
mod fuel;
mod pilot;
mod structure;
pub use controls::Controls;
pub use engine::Engine;
pub use fluids::Fluids;
pub use fuel::Fuel;
pub use pilot::Pilot;
pub use structure::{RegionalEffects, Structure, regional_effects};

#[derive(Clone, Debug, PartialEq)]
pub struct Systems {
    /// Source occurrence counters are provenance and duplicate-delivery guards.
    pub counts: [u8; 45],
    pub engine: Engine,
    pub fluids: Fluids,
    pub fuel: Fuel,
    pub controls: Controls,
    pub structure: Structure,
    pub pilot: Pilot,
    last_impact_notice: Option<u64>,
    pub messages: Vec<String>,
}
impl Default for Systems {
    fn default() -> Self {
        Self {
            counts: [0; 45],
            engine: Engine::default(),
            fluids: Fluids::default(),
            fuel: Fuel::default(),
            controls: Controls::default(),
            structure: Structure::default(),
            pilot: Pilot::default(),
            last_impact_notice: None,
            messages: Vec::new(),
        }
    }
}
/// Authored concise labels for reviewed source event identities, not retail strings.
pub fn label(index: usize) -> &'static str {
    match index {
        0 => "Airframe hit",
        1 => "Fuel leak",
        2 => "Fuel feed failed",
        3 => "Fuel fire",
        4 => "Engine flameout: cycle throttle to restart",
        5 => "Engine power reduced",
        6 => "Severe engine damage",
        7 => "Compressor damaged: throttle below 25%",
        8 => "Afterburner failed",
        9 | 10 => "Engine shutdown",
        11 => "Engine fire",
        12 => "Oil pump damaged: reduce throttle",
        13 => "Oil leak",
        14 => "Hydraulic leak",
        15 => "Hydraulic fire",
        16 => "Landing gear jammed",
        17 => "Flaps jammed",
        18 => "Airbrake jammed",
        19 => "Elevator damaged",
        20 => "Elevator bent",
        21 => "Ailerons damaged",
        22 => "Ailerons bent",
        23 => "Rudder damaged",
        24 => "Rudder bent",
        25 => "Wing damaged",
        26 => "Wing destroyed",
        27 => "Control linkage damaged",
        28 => "Flight controls unstable",
        29 => "Throttle jammed",
        30 => "Structure weakened: avoid high G",
        31 => "Flight sensors failed",
        32 => "Instrument display failed",
        33 => "Navigation failed",
        34 => "Pilot wounded: return to base",
        35 => "Critical fire: explosion imminent",
        _ => "Equipment damaged",
    }
}

pub fn damage_percent(damage: f64) -> u32 {
    ((damage.clamp(0., 1.) * 100.).round() as u32).min(if damage >= 1. { 100 } else { 99 })
}
impl Systems {
    pub fn new(engines: u8, external: [f64; 9]) -> Self {
        Self {
            engine: Engine::new(engines),
            fuel: Fuel::new(external),
            ..Self::default()
        }
    }
    pub fn notify(&mut self, message: impl Into<String>) {
        if self.messages.len() < 64 {
            self.messages.push(message.into());
        }
    }
    pub fn report_impact(&mut self, tick: u64, damage: f64) {
        if self
            .last_impact_notice
            .is_none_or(|last| tick.saturating_sub(last) >= 480)
        {
            self.last_impact_notice = Some(tick);
            self.notify(format!("Aircraft hit: {}% damage", damage_percent(damage)));
        }
    }
    pub fn kill_pilot(&mut self, reason: &str) {
        if self.pilot.kill() {
            self.notify(reason);
        }
    }
    pub fn has(&self, index: usize) -> bool {
        self.counts[index] > 0
    }
    pub fn oil_pressure(&self) -> f64 {
        self.fluids.oil_pressure()
    }
    pub fn external_lbs(&self) -> f64 {
        self.fuel.external_lbs()
    }
    pub fn used_external_lbs(&self) -> f64 {
        self.fuel.used_lbs()
    }
    pub fn power_available(&self) -> f64 {
        self.engine.available()
    }
    pub fn fatal(&self) -> bool {
        self.structure.failed || self.pilot.dead
    }
    pub fn autopilot_available(&self) -> bool {
        !self.counts[19..=28].iter().any(|n| *n > 0)
            && !self.has(14)
            && !self.has(15)
            && !self.has(31)
            && self.engine.power >= 0.5
            && !self.fatal()
    }
    pub fn hit(&mut self, index: usize, throttle: f64) {
        if index >= self.counts.len() {
            return;
        }
        self.counts[index] = self.counts[index].saturating_add(1);
        if index < 36 {
            self.notify(label(index));
        }
        self.engine.hit(index);
        self.fluids.hit(index);
        self.fuel.hit(index);
        self.controls.hit(index, throttle);
        self.structure.hit(index);
        self.pilot.hit(index);
    }
    pub fn consume(&mut self, internal: &mut f64, pounds: f64) {
        self.fuel.consume(internal, pounds);
    }
    /// Sole coupling step; components own their timing and failure state.
    pub fn advance(
        &mut self,
        running: bool,
        throttle: f64,
        g: f64,
        damage: f64,
        landed: bool,
        fuel: &mut f64,
    ) {
        if self.fatal() {
            return;
        }
        self.fuel.advance(fuel);
        let mut messages = self.fluids.advance();
        messages.extend(self.engine.advance(
            running,
            throttle,
            self.fluids.oil_pressure(),
            self.structure.burning(),
        ));
        messages.extend(self.structure.advance(g, damage));
        messages.extend(self.pilot.advance(landed));
        for message in messages {
            self.notify(message);
        }
    }
    pub fn controls(&self, requested: [f64; 3], held: [f64; 3], tick: u64) -> [f64; 3] {
        self.controls
            .response(requested, held, tick, self.fluids.hydraulic)
    }
    pub fn device_free(&self, index: usize) -> bool {
        self.fluids.hydraulic > 0. && !self.has(index)
    }
    pub fn summary(&self, damage: f64) -> String {
        format!(
            "Damage {}% | TEMP {:.0}% OIL {:.0}% HYD {:.0}% | Power {:.0}%",
            damage_percent(damage),
            self.engine.temperature,
            self.oil_pressure() * 100.,
            self.fluids.hydraulic * 100.,
            self.power_available() * 100.
        )
    }
}
#[cfg(test)]
mod tests;
