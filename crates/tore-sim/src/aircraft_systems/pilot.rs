use crate::flight::DT;
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pilot {
    pub(super) remaining: Option<f64>,
    warning: bool,
    pub dead: bool,
    pub ejected: bool,
}
impl Pilot {
    /// Immediate lethal injury, distinct from the timed wound progression.
    pub fn kill(&mut self) -> bool {
        if self.dead {
            return false;
        }
        self.dead = true;
        self.remaining = None;
        true
    }
    pub fn hit(&mut self, index: usize) {
        if index == 34 && !self.dead && !self.ejected {
            self.remaining = Some(self.remaining.map_or(900., |t| t * 0.5));
            self.warning = false;
        }
    }
    pub fn advance(&mut self, landed: bool) -> Vec<&'static str> {
        let mut messages = Vec::new();
        if self.dead {
            return messages;
        }
        if landed && self.remaining.take().is_some() {
            messages.push("Pilot wounds treated after landing");
        }
        if let Some(t) = &mut self.remaining {
            *t -= DT;
            if *t <= 0. {
                self.dead = true;
                messages.push("Pilot died from wounds");
            } else if *t <= 60. && !self.warning {
                self.warning = true;
                messages.push("Pilot condition critical: land immediately");
            }
        }
        messages
    }
}
