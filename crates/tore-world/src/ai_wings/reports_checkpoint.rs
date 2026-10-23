//! The coders of the AI's formation text reports (docs/formats/checkpoint.md),
//! part of the AI wings section.
//!
//! Stage H slice H6. Coded: each aircraft's report state, the queue of reports
//! waiting for the HUD and the mission tick the last observation read.
//!
//! Skipped, all why-records (the journal's explanations of a report; no rule
//! reads them, they only decide whether a journal line is written):
//!
//! - `Reports::queued`: each pending report's queued journal entry. A report
//!   that was pending at the checkpoint is shown without the journal's
//!   "waited" line.
//! - `Reports::notes`: entries waiting to join the wing's journal. They are
//!   drained into it at the end of every step, so they are empty between
//!   ticks.
//! - `Reports::activity`: the activity line's queued journal entry.
//! - `ReportState::held`: the report last noted as held by the 10 second
//!   limit; it only stops the same journal note being written twice.

use super::{ReportState, Reports};

tore_sim::checkpoint_struct!(ReportState {
    category,
    last_tick,
    requested,
    airfield,
} skip {
    // Why-record: the journal's hold note, written once per held report.
    held = None,
});

tore_sim::checkpoint_struct!(Reports {
    states,
    pending,
    clock,
} skip {
    // Why-record: each pending report's queued journal entry.
    queued = std::collections::BTreeMap::new(),
    // Why-record: entries waiting to join the journal; drained every step.
    notes = Vec::new(),
    // Why-record: the activity line's queued journal entry.
    activity = None,
});

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::ai::controller::Activity;
    use tore_sim::ai::formation::{Phase, Trace};
    use tore_sim::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    fn trace(phase: Phase, phase_seconds: f64) -> Trace {
        Trace {
            phase,
            phase_seconds,
            slot_distance_ft: 0.,
            closure_fps: 0.,
            altitude_error_ft: 0.,
            minimum_predicted_separation_ft: 1000.,
            yielding_to: None,
            aim: [0.; 3],
            planned_velocity: None,
        }
    }

    /// Reports with two wingmen mid-conversation: one has a report waiting,
    /// one is in an airfield sequence, and one has asked for a rejoin.
    fn lived_in() -> Reports {
        let mut reports = Reports::default();
        reports.observe(1, "Friendly 1-2", 10, &trace(Phase::Breakout, 0.));
        reports.observe(2, "Friendly 1-3", 20, &trace(Phase::Close, 0.));
        reports.observe(2, "Friendly 1-3", 1300, &trace(Phase::Intercept, 31.));
        reports.observe_airfield(3, "Friendly 1-4", Activity::Taxiing);
        reports
    }

    fn coded(reports: &Reports) -> tore_sim::checkpoint::Coded {
        to_bytes(reports, &Models::default()).unwrap()
    }

    #[test]
    fn reports_round_trip_with_their_queue_and_limits() {
        let reports = lived_in();
        assert!(!reports.pending.is_empty());
        assert!(reports.states.values().any(|state| state.requested));
        assert!(
            reports
                .states
                .values()
                .any(|state| state.airfield.is_some())
        );
        let copy: Reports = round_trip(&reports, &Models::default()).unwrap();
        assert_eq!(copy.pending, reports.pending);
        assert_eq!(copy.clock, reports.clock);
        assert_eq!(copy.states.len(), reports.states.len());
        for (id, state) in &reports.states {
            let held = &copy.states[id];
            assert_eq!(
                (held.category, held.last_tick, held.requested, held.airfield),
                (
                    state.category,
                    state.last_tick,
                    state.requested,
                    state.airfield
                )
            );
        }
        assert_eq!(coded(&copy), coded(&reports));
    }

    #[test]
    fn a_restored_reports_acts_like_the_original() {
        // The 10 second limit and the one request per stall decide what is
        // said next, so both copies must say the same things from here on.
        let mut original = lived_in();
        let mut copy: Reports = from_bytes(&coded(&original), &Models::default()).unwrap();
        for (tick, phase, seconds) in [
            (1400, Phase::Intercept, 40.),
            (2600, Phase::Capture, 0.),
            (2700, Phase::Close, 0.),
            (4000, Phase::Trail, 5.),
        ] {
            for reports in [&mut original, &mut copy] {
                reports.observe(1, "Friendly 1-2", tick, &trace(phase, seconds));
                reports.observe(2, "Friendly 1-3", tick, &trace(phase, seconds));
            }
            assert_eq!(copy.take_at(tick), original.take_at(tick), "tick {tick}");
            assert_eq!(copy.take_at(tick), original.take_at(tick), "tick {tick}");
        }
        assert_eq!(coded(&copy), coded(&original));
    }

    #[test]
    fn the_journal_only_parts_are_not_coded() {
        let reports = lived_in();
        assert!(!reports.queued.is_empty(), "the fixture queued a report");
        let copy: Reports = round_trip(&reports, &Models::default()).unwrap();
        assert!(copy.queued.is_empty() && copy.activity.is_none());
        assert!(copy.states.values().all(|state| state.held.is_none()));
    }

    #[test]
    fn damaged_bytes_never_panic() {
        let whole = coded(&lived_in());
        for cut in 0..whole.body.len() {
            let mut shorter = whole.clone();
            shorter.body.truncate(cut);
            let _ = from_bytes::<Reports>(&shorter, &Models::default());
        }
    }
}
