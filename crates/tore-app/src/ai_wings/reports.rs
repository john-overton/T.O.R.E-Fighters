//! Opinionated formation text reports. Their original recordings are
//! unknown, so no audio is assigned. Voiced combat chatter is in `chatter.rs`.
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
    /// The airfield activity last seen, while the aircraft is in a takeoff
    /// or landing sequence rather than in formation.
    airfield: Option<Activity>,
}
/// Activities of the takeoff and landing sequences. An aircraft in one of
/// them is not flying formation, so it makes no formation reports.
fn airfield(activity: Activity) -> bool {
    matches!(
        activity,
        Activity::Waiting
            | Activity::Taxiing
            | Activity::TakingOff
            | Activity::HoldingMarshal
            | Activity::Landing
            | Activity::Landed
    )
}
impl Reports {
    /// `opinionated` (agent decision, 2026-09-23): a wingman that has left to
    /// land reports holding, landing and landed once each, using the target
    /// window's activity labels. Takeoff steps stay silent, since a whole
    /// wing starting on the ground would otherwise fill the message bar.
    /// Returns whether the aircraft is in an airfield sequence.
    fn observe_airfield(&mut self, id: u32, label: &str, activity: Activity) -> bool {
        let state = self.states.entry(id).or_default();
        if !airfield(activity) {
            state.airfield = None;
            return false;
        }
        let changed = state.airfield.replace(activity) != Some(activity);
        if changed
            && matches!(
                activity,
                Activity::HoldingMarshal | Activity::Landing | Activity::Landed
            )
        {
            self.pending.retain(|(actor, _)| *actor != id);
            if self.pending.len() == 16 {
                self.pending.pop_front();
            }
            self.pending
                .push_back((id, format!("{label}: {}", activity.label())));
        }
        true
    }
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
                && self
                    .reports
                    .observe_airfield(slot.id, &slot.label(), actor.activity())
            {
                continue;
            }
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

    #[test]
    fn airfield_reports_announce_landing_steps_once_and_skip_takeoff() {
        let mut reports = Reports::default();
        for activity in [Activity::Waiting, Activity::Taxiing, Activity::TakingOff] {
            assert!(reports.observe_airfield(1, "Friendly 1-1", activity));
        }
        assert!(reports.take().is_none(), "takeoff steps are silent");
        assert!(!reports.observe_airfield(1, "Friendly 1-1", Activity::Formation));
        let mut lines = Vec::new();
        for activity in [
            Activity::HoldingMarshal,
            Activity::HoldingMarshal,
            Activity::Landing,
            Activity::Landed,
            Activity::Landed,
        ] {
            assert!(reports.observe_airfield(1, "Friendly 1-1", activity));
            lines.extend(reports.take());
        }
        assert_eq!(
            lines,
            [
                "Friendly 1-1: Holding at marshal",
                "Friendly 1-1: Landing",
                "Friendly 1-1: Landed",
            ]
        );
        // A newer step replaces an unread older one for the same aircraft.
        reports.observe_airfield(2, "Friendly 1-2", Activity::Landing);
        reports.observe_airfield(2, "Friendly 1-2", Activity::Landed);
        assert_eq!(reports.take().as_deref(), Some("Friendly 1-2: Landed"));
        assert!(reports.take().is_none());
    }
}
