use crate::flight::DT;

/// Fitted aerodynamic loss from the same regions used for visible tears.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RegionalEffects {
    pub authority: [f64; 3],
    pub roll_bias: f64,
    pub yaw_bias: f64,
    pub lift: f64,
    pub drag_percent: f64,
}
pub fn regional_effects(regions: [f64; 6]) -> RegionalEffects {
    let [left, right, tail] = [regions[3], regions[4], regions[5]].map(|v| v.clamp(0., 1.));
    RegionalEffects {
        authority: [1. - 0.8 * tail, 1. - 0.6 * left.max(right), 1. - 0.8 * tail],
        roll_bias: 0.35 * (right - left),
        yaw_bias: 0.15 * tail,
        lift: (1. - 0.35 * (left + right) - 0.2 * tail).max(0.15),
        drag_percent: 25. * (left + right) + 15. * tail,
    }
}

impl RegionalEffects {
    /// Aerodynamic response, separate from the rendered actuator positions.
    pub fn commands(self, requested: [f64; 3]) -> [f64; 3] {
        [
            requested[0] * self.authority[0],
            (requested[1] * self.authority[1] + self.roll_bias).clamp(-1., 1.),
            (requested[2] * self.authority[2] + self.yaw_bias).clamp(-1., 1.),
        ]
    }
}
/// Why an airframe was lost outside combat damage. Requested by John on
/// 2026-09-29 (docs/spec/overspeed.md, docs/spec/world-edge.md).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LossCause {
    /// Flown at 1.5 times the aircraft's own top speed or faster.
    Overspeed,
    /// Flown 105 nautical miles beyond the edge of the theater.
    OutOfBounds,
}
impl LossCause {
    /// The word a recording, replay log or debrief line uses.
    pub fn label(self) -> &'static str {
        match self {
            Self::Overspeed => "overspeed",
            Self::OutOfBounds => "out of bounds",
        }
    }
    pub fn message(self) -> &'static str {
        match self {
            Self::Overspeed => "Airframe failed: overspeed",
            Self::OutOfBounds => "Aircraft lost: out of bounds",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Structure {
    pub failed: bool,
    /// Set when the airframe was lost to overspeed or leaving the map.
    pub cause: Option<LossCause>,
    pub wing_damage: bool,
    fire_remaining: Option<f64>,
    weak: bool,
    exposure: f64,
}
impl Structure {
    pub fn fire_seconds(&self) -> Option<f64> {
        self.fire_remaining
    }
    pub fn burning(&self) -> bool {
        self.fire_remaining.is_some()
    }
    pub fn hit(&mut self, index: usize) {
        match index {
            3 | 11 | 15 => self.ignite(10.),
            25 => self.wing_damage = true,
            26 => self.failed = true,
            30 => self.weak = true,
            35 => self.ignite(5.),
            _ => {}
        }
    }
    fn ignite(&mut self, seconds: f64) {
        self.fire_remaining = Some(self.fire_remaining.map_or(seconds, |t| t.min(seconds)));
    }
    pub fn advance(&mut self, g: f64, damage: f64) -> Vec<&'static str> {
        let mut messages = Vec::new();
        if let Some(t) = &mut self.fire_remaining {
            *t -= DT;
            if *t <= 0. {
                self.failed = true;
                messages.push("Aircraft destroyed by fire");
            }
        }
        if self.weak && g.abs() > (9. * (1. - damage)).max(2.) {
            self.exposure += DT;
            if self.exposure >= 2. {
                self.failed = true;
                messages.push("Airframe failed under G load");
            }
        }
        messages
    }
}

crate::flight::exact::exact_enum!(LossCause {
    Overspeed = 0,
    OutOfBounds = 1,
});
crate::flight::exact::exact_struct!(Structure {
    failed,
    cause,
    wing_damage,
    fire_remaining,
    weak,
    exposure,
});
