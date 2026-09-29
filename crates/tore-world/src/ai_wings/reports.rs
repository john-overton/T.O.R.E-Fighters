//! Opinionated formation text reports. Their original recordings are
//! unknown, so no audio is assigned. Voiced combat chatter is in `chatter.rs`.
//!
//! The text lines the AI posts on the HUD, these reports and the activity
//! line, are journaled here: queued, held by their limits, replaced, dropped,
//! cancelled and shown.
use super::*;
use crate::comms::journal::{Cause, Entry, Origin, Outcome, Reason, Source};
use tore_sim::ai::formation::{Phase, Trace};

/// Ticks between one aircraft's formation reports.
const REPORT_TICKS: u64 = 1200;
/// Formation reports waiting at most; a new one pushes out the oldest.
const REPORT_QUEUE: usize = 16;

#[derive(Default)]
pub(super) struct Reports {
    states: BTreeMap<u32, ReportState>,
    pending: std::collections::VecDeque<(u32, String)>,
    /// Journal only: each pending report's queued entry, entries waiting to
    /// join the wing's journal, the mission tick, and the activity line's
    /// queued entry.
    queued: BTreeMap<u32, Entry>,
    notes: Vec<Entry>,
    clock: u64,
    activity: Option<Entry>,
}
#[derive(Default)]
struct ReportState {
    category: u8,
    last_tick: Option<u64>,
    requested: bool,
    /// The airfield activity last seen, while the aircraft is in a takeoff
    /// or landing sequence rather than in formation.
    airfield: Option<Activity>,
    /// Journal only: the report last noted as held by the 10 s limit.
    held: Option<(u8, bool)>,
}

fn formation_cause(trace: &Trace) -> Cause {
    Cause::Formation {
        phase: trace.phase,
        seconds: trace.phase_seconds,
        closure_fps: trace.closure_fps,
    }
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
    /// Airfield actors are excluded from formation chatter. Their status
    /// and recorded airport calls are owned by `airfield_radio`.
    fn observe_airfield(&mut self, id: u32, _label: &str, activity: Activity) -> bool {
        self.states.entry(id).or_default().airfield = airfield(activity).then_some(activity);
        if airfield(activity) {
            self.pending.retain(|(actor, _)| *actor != id);
            self.leave(id, Outcome::Cancelled(Reason::AirfieldSequence));
        }
        airfield(activity)
    }
    fn observe(&mut self, id: u32, label: &str, tick: u64, trace: &Trace) {
        self.clock = tick;
        let state = self.states.entry(id).or_default();
        let category = match trace.phase {
            Phase::Close => 0,
            Phase::Reposition => return,
            Phase::Trail | Phase::Breakout => 1,
            Phase::Intercept | Phase::Stabilize | Phase::Capture => 2,
        };
        if let Some(last) = state
            .last_tick
            .filter(|last| tick.saturating_sub(*last) < REPORT_TICKS)
        {
            // Journal only: a report the 10 s limit holds back, noted once.
            let request = trace.phase == Phase::Intercept
                && trace.phase_seconds >= 30.
                && trace.closure_fps <= 0.
                && !state.requested;
            if (request || category != state.category) && state.held != Some((category, request)) {
                state.held = Some((category, request));
                let remaining = (last + REPORT_TICKS).saturating_sub(tick) as f64 / 120.;
                self.notes.push(Entry::note(
                    tick as f64 / 120.,
                    label,
                    Origin::of(Source::Hud, formation_cause(trace)).by(id),
                    Outcome::Suppressed(Reason::RateLimited {
                        interval_s: REPORT_TICKS as f64 / 120.,
                        remaining,
                    }),
                ));
            }
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
            state.held = None;
            self.pending.retain(|(actor, _)| *actor != id);
            self.leave(id, Outcome::Replaced(Reason::Coalesced));
            if self.pending.len() == REPORT_QUEUE
                && let Some((oldest, _)) = self.pending.pop_front()
            {
                self.leave(
                    oldest,
                    Outcome::Dropped(Reason::QueueFull {
                        limit: REPORT_QUEUE,
                    }),
                );
            }
            let at = tick as f64 / 120.;
            let entry = Entry::note(
                at,
                label,
                Origin::of(Source::Hud, formation_cause(trace)).by(id),
                Outcome::Queued {
                    due: at,
                    expires: None,
                },
            )
            .with_text(text);
            self.notes.push(entry.clone());
            self.queued.insert(id, entry);
            self.pending.push_back((id, format!("{label}: {text}")));
        }
    }
    #[cfg(test)]
    pub(super) fn take(&mut self) -> Option<String> {
        self.pending.pop_front().map(|(_, text)| text)
    }
    /// The next report for the HUD at `tick`, journaled as shown.
    fn take_at(&mut self, tick: u64) -> Option<String> {
        self.clock = tick;
        let (id, text) = self.pending.pop_front()?;
        if let Some(mut entry) = self.queued.remove(&id) {
            let now = tick as f64 / 120.;
            entry.outcome = Outcome::Delivered {
                waited: now - entry.at,
            };
            entry.at = now;
            self.notes.push(entry);
        }
        Some(text)
    }
    /// Journal the pending report of `id` leaving the queue with `outcome`.
    fn leave(&mut self, id: u32, outcome: Outcome) {
        if let Some(mut entry) = self.queued.remove(&id) {
            entry.at = self.clock as f64 / 120.;
            entry.outcome = outcome;
            self.notes.push(entry);
        }
    }
}
impl AiWings {
    /// The next text line for the HUD: a formation report first, then the
    /// activity line. Each is journaled as shown.
    pub(super) fn take_line(&mut self) -> Option<String> {
        let tick = self.mission.tick();
        let line = self.reports.take_at(tick).or_else(|| {
            let line = self.pending_message.take();
            if let Some(mut entry) = self.reports.activity.take() {
                let now = tick as f64 / 120.;
                entry.outcome = Outcome::Delivered {
                    waited: now - entry.at,
                };
                entry.at = now;
                self.reports.notes.push(entry);
            }
            line
        });
        self.watch.journal.extend(self.reports.notes.drain(..));
        line
    }

    /// Journal an activity line the 2 s message limit held back.
    pub(super) fn activity_held(&mut self, id: u32, activity: Activity, remaining_ticks: u64) {
        let Some(slot) = self.slot(id) else {
            return;
        };
        let entry = Entry::note(
            self.journal_clock(),
            slot.label(),
            Origin::of(Source::Hud, Cause::Activity { activity }).by(id),
            Outcome::Suppressed(Reason::RateLimited {
                interval_s: MESSAGE_INTERVAL_TICKS as f64 / 120.,
                remaining: remaining_ticks as f64 / 120.,
            }),
        )
        .with_text(activity.label());
        self.watch.journal.push(entry);
    }

    /// Journal a new activity line, and the unread one it replaces.
    pub(super) fn activity_posted(&mut self, id: u32, activity: Activity) {
        let at = self.journal_clock();
        if self.pending_message.is_some()
            && let Some(mut old) = self.reports.activity.take()
        {
            old.at = at;
            old.outcome = Outcome::Replaced(Reason::Unread);
            self.watch.journal.push(old);
        }
        let entry = Entry::note(
            at,
            self.journal_label(id),
            Origin::of(Source::Hud, Cause::Activity { activity }).by(id),
            Outcome::Queued {
                due: at,
                expires: None,
            },
        )
        .with_text(activity.label());
        self.watch.journal.push(entry.clone());
        self.reports.activity = Some(entry);
    }

    pub(super) fn formation_reports(&mut self) {
        self.reports.clock = self.mission.tick();
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
                self.reports
                    .leave(slot.id, Outcome::Cancelled(Reason::OutOfFormation));
            }
        }
        self.watch.journal.extend(self.reports.notes.drain(..));
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
    fn airfield_reports_are_owned_by_the_radio_producer() {
        let mut reports = Reports::default();
        reports
            .pending
            .push_back((1, "Stale formation report".into()));
        for activity in [
            Activity::Waiting,
            Activity::Taxiing,
            Activity::TakingOff,
            Activity::HoldingMarshal,
            Activity::Landing,
            Activity::Landed,
        ] {
            assert!(reports.observe_airfield(1, "Wingman", activity));
            assert!(reports.take().is_none());
        }
        assert!(!reports.observe_airfield(1, "Wingman", Activity::Formation));
    }

    fn trace(phase: Phase) -> Trace {
        Trace {
            phase,
            phase_seconds: 0.,
            slot_distance_ft: 0.,
            closure_fps: 0.,
            altitude_error_ft: 0.,
            minimum_predicted_separation_ft: 1000.,
            yielding_to: None,
            aim: [0.; 3],
            planned_velocity: None,
        }
    }

    #[test]
    fn formation_reports_are_journaled_from_queue_to_the_hud() {
        let mut reports = Reports::default();
        reports.observe(1, "Wingman 1", 0, &trace(Phase::Close));
        reports.observe(1, "Wingman 1", 1, &trace(Phase::Breakout));
        reports.observe(1, "Wingman 1", 2, &trace(Phase::Intercept));
        reports.observe(1, "Wingman 1", 3, &trace(Phase::Intercept));
        assert!(reports.take_at(120).unwrap().contains("Breaking out"));
        reports.observe(2, "Wingman 2", 130, &trace(Phase::Breakout));
        assert!(reports.observe_airfield(2, "Wingman 2", Activity::Landing));
        let notes: Vec<_> = reports
            .notes
            .drain(..)
            .map(|e| (e.origin.speaker, e.text, e.outcome))
            .collect();
        let breaking = "Breaking out for separation".to_string();
        assert_eq!(
            notes,
            [
                (
                    Some(1),
                    breaking.clone(),
                    Outcome::Queued {
                        due: 1. / 120.,
                        expires: None
                    }
                ),
                (
                    Some(1),
                    String::new(),
                    Outcome::Suppressed(Reason::RateLimited {
                        interval_s: 10.,
                        remaining: 1199. / 120.
                    })
                ),
                (
                    Some(1),
                    breaking.clone(),
                    Outcome::Delivered {
                        waited: 1. - 1. / 120.
                    }
                ),
                (
                    Some(2),
                    breaking.clone(),
                    Outcome::Queued {
                        due: 130. / 120.,
                        expires: None
                    }
                ),
                (
                    Some(2),
                    breaking,
                    Outcome::Cancelled(Reason::AirfieldSequence)
                ),
            ],
            "the limit is noted once, not every tick"
        );
    }

    #[test]
    fn the_activity_line_is_journaled_held_and_shown() {
        let (mut wings, _) = super::super::tests::build(None);
        wings.announce(&[(1, Activity::Attacking)]);
        wings.announce(&[(2, Activity::Defending)]);
        assert_eq!(
            wings.take_message().as_deref(),
            Some("Friendly 2-1: Attacking")
        );
        let entries: Vec<_> = wings
            .take_journal()
            .into_iter()
            .map(|e| (e.label, e.text, e.outcome))
            .collect();
        assert_eq!(
            entries,
            [
                (
                    "Friendly 2-1".to_string(),
                    "Attacking".to_string(),
                    Outcome::Queued {
                        due: 0.,
                        expires: None
                    }
                ),
                (
                    "Friendly 2-2".to_string(),
                    "Defending".to_string(),
                    Outcome::Suppressed(Reason::RateLimited {
                        interval_s: 2.,
                        remaining: 241. / 120.
                    })
                ),
                (
                    "Friendly 2-1".to_string(),
                    "Attacking".to_string(),
                    Outcome::Delivered { waited: 0. }
                ),
            ]
        );
    }
}
