//! Facts already sent to the cockpit, retained for replay object banners.
//! No new wire fields: only the viewed AI's activity and whether it aims at
//! this seat are known. Clear them when that readout is no longer current.
use crate::wire::readout::QReadout;
use tore_replay::{
    Event,
    vocab::{field, kind},
};
use tore_sim::ai::controller::Activity;
use tore_world::target_window::Pilot;

/// Same two-second silence bound as converted entity tracks.
pub const HOLD_TICKS: u64 = 240;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Facts {
    pub plane: u32,
    pub target: Option<u32>,
    pub held: bool,
    pub ai: Option<(u32, Activity, bool)>,
}

impl Facts {
    pub fn read(q: &QReadout, tick: u32) -> Option<Self> {
        // Unreceived scalar groups are not evidence that a target was cleared.
        if !q.target_groups_present() {
            return None;
        }
        let ro = q.readout(tick, None, None).ok()?;
        let target = ro.targets.view.as_ref().map(|t| t.id);
        Some(Self {
            plane: q.plane()?,
            target,
            held: target.is_some() && target != ro.targets.designated,
            ai: ro
                .target_window
                .filter(|brief| {
                    ro.targets
                        .display
                        .as_ref()
                        .is_some_and(|target| target.id == brief.id)
                })
                .and_then(|brief| match brief.pilot {
                    Pilot::Ai {
                        activity,
                        aims_at_viewer,
                        ..
                    } => Some((brief.id, activity, aims_at_viewer)),
                    _ => None,
                }),
        })
    }
}

#[derive(Default)]
pub struct Watch {
    last: Option<Facts>,
}

fn event(kind: &str, subject: u32) -> Event {
    Event::new(kind)
        .with_subject(subject)
        .with(field::SOURCE, "net.readout")
}

impl Watch {
    pub fn events(&mut self, now: Option<Facts>) -> Vec<Event> {
        let before = self.last;
        if before == now {
            return Vec::new();
        }
        self.last = now;
        let mut out = Vec::new();
        if let Some(old) = before
            && now.is_none_or(|n| n.plane != old.plane)
        {
            out.push(event(kind::PLAYER_VIEW_TARGET, old.plane));
        }
        if let Some(now) = now
            && before
                .is_none_or(|b| (b.plane, b.target, b.held) != (now.plane, now.target, now.held))
        {
            let mut e = event(kind::PLAYER_VIEW_TARGET, now.plane).with(field::HELD, now.held);
            e.object = now.target;
            out.push(e);
        }
        let old_ai = before.and_then(|b| b.ai);
        let new_ai = now.and_then(|n| n.ai);
        if let Some((id, _, _)) = old_ai
            && new_ai.is_none_or(|a| a.0 != id)
        {
            out.push(event(kind::AI_ACTIVITY, id).with(field::TO, "-"));
            out.push(event(kind::AI_TARGET, id));
        }
        if let Some((id, activity, aims)) = new_ai {
            if old_ai.is_none_or(|old| (old.0, old.1) != (id, activity)) {
                out.push(event(kind::AI_ACTIVITY, id).with(field::TO, activity.label()));
            }
            if old_ai.is_none_or(|old| (old.0, old.2) != (id, aims))
                || before.map(|b| b.plane) != now.map(|n| n.plane)
            {
                let mut e = event(kind::AI_TARGET, id);
                // False means "not this seat", not a known different target.
                e.object = now.filter(|_| aims).map(|n| n.plane);
                out.push(e);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn facts_are_changes_only_and_departing_ai_facts_are_cleared() {
        let a = Facts {
            plane: 4,
            target: Some(7),
            held: false,
            ai: Some((7, Activity::Attacking, true)),
        };
        let mut watch = Watch::default();
        assert!(watch.events(None).is_empty());
        let first = watch.events(Some(a));
        assert_eq!(first.len(), 3);
        assert_eq!(first[0].object, Some(7));
        assert_eq!(
            first[1].string(field::TO),
            Some(Activity::Attacking.label())
        );
        assert_eq!(first[2].object, Some(4));
        assert!(watch.events(Some(a)).is_empty());
        let b = Facts {
            target: None,
            ai: None,
            ..a
        };
        let cleared = watch.events(Some(b));
        assert_eq!(cleared.len(), 3);
        assert_eq!(cleared[1].string(field::TO), Some("-"));
        assert!(cleared.iter().all(|e| e.object.is_none()));
        assert_eq!(
            watch.events(Some(a)),
            first,
            "reacquisition restores the same facts"
        );
        let expired = watch.events(None);
        assert_eq!(expired.len(), 3);
        assert!(expired.iter().all(|e| e.object.is_none()));
        let not_us = Facts {
            ai: Some((7, Activity::Searching, false)),
            ..a
        };
        let facts = watch.events(Some(not_us));
        assert_eq!(facts[2].object, None, "never invent the AI's other target");
    }

    #[test]
    fn an_incomplete_readout_is_unknown() {
        assert_eq!(Facts::read(&QReadout::default(), 100), None);
    }
}
