//! Opinionated text reports. No unverified original audio is assigned.
use super::*;
use tore_sim::ai::formation::{Phase, Trace};

#[derive(Default)]
pub(super) struct Reports {
    states: BTreeMap<u32, ReportState>,
    pending: std::collections::VecDeque<(u32, String)>,
}
#[derive(Default)]
struct ReportState {
    category: u8,
    last_tick: Option<u64>,
    requested: bool,
}
impl Reports {
    fn observe(&mut self, id: u32, label: &str, tick: u64, trace: &Trace) {
        let state = self.states.entry(id).or_default();
        let category = match trace.phase {
            Phase::Close => 0,
            Phase::Reposition => return,
            Phase::Trail | Phase::Breakout => 1,
            Phase::Intercept | Phase::Stabilize | Phase::Capture => 2,
        };
        if state
            .last_tick
            .is_some_and(|last| tick.saturating_sub(last) < 1200)
        {
            return;
        }
        let request = trace.phase == Phase::Intercept
            && trace.phase_seconds >= 30.
            && trace.closure_fps <= 0.
            && !state.requested;
        let text = if request {
            state.requested = true;
            Some("Not closing. Request steady heading and speed for rejoin")
        } else if category != state.category {
            Some(match category {
                0 => "In position",
                1 if trace.phase == Phase::Breakout => "Breaking out for separation",
                1 => "Taking a safe trailing position",
                _ => "Rejoining",
            })
        } else {
            None
        };
        if category == 0 {
            state.requested = false;
        }
        state.category = category;
        if let Some(text) = text {
            state.last_tick = Some(tick);
            self.pending.retain(|(actor, _)| *actor != id);
            if self.pending.len() == 16 {
                self.pending.pop_front();
            }
            self.pending.push_back((id, format!("{label}: {text}")));
        }
    }
    pub(super) fn take(&mut self) -> Option<String> {
        self.pending.pop_front().map(|(_, text)| text)
    }
}
impl AiWings {
    pub(super) fn formation_reports(&mut self) {
        for slot in &self.slots {
            if slot.side != launch::Side::Friendly || slot.wing_number != 1 {
                continue;
            }
            let Some(actor) = self.mission.actor(slot.id) else {
                continue;
            };
            if actor.alive()
                && let Some(trace) = actor.controller().formation_trace()
            {
                self.reports
                    .observe(slot.id, &slot.label(), self.mission.tick(), &trace);
            } else {
                self.reports.states.remove(&slot.id);
                self.reports.pending.retain(|(id, _)| *id != slot.id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reports_follow_state_and_cooldown_without_repeating_requests() {
        let mut reports = Reports::default();
        let mut t = Trace {
            phase: Phase::Close,
            phase_seconds: 0.,
            slot_distance_ft: 0.,
            closure_fps: 0.,
            altitude_error_ft: 0.,
            minimum_predicted_separation_ft: 1000.,
            yielding_to: None,
            aim: [0.; 3],
            planned_velocity: None,
        };
        reports.observe(1, "Wingman 1", 0, &t);
        assert!(reports.take().is_none());
        t.phase = Phase::Breakout;
        reports.observe(1, "Wingman 1", 1, &t);
        assert!(reports.take().unwrap().contains("Breaking out"));
        t.phase = Phase::Intercept;
        reports.observe(1, "Wingman 1", 2, &t);
        assert!(reports.take().is_none());
        reports.observe(1, "Wingman 1", 1201, &t);
        assert!(reports.take().unwrap().contains("Rejoining"));
        t.phase = Phase::Reposition;
        reports.observe(1, "Wingman 1", 2500, &t);
        assert!(
            reports.take().is_none(),
            "a slot transition is not completed capture"
        );
        t.phase = Phase::Intercept;
        t.phase_seconds = 30.;
        reports.observe(1, "Wingman 1", 3601, &t);
        assert!(reports.take().unwrap().contains("Request steady"));
        reports.observe(1, "Wingman 1", 5000, &t);
        assert!(reports.take().is_none());
        t.phase = Phase::Close;
        reports.observe(1, "Wingman 1", 6000, &t);
        assert!(reports.take().unwrap().contains("In position"));
    }
}
