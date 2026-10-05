//! Lag compensation for human gunfire (docs/ARCHITECTURE.md, "Hits and lag
//! compensation"): one second of every aircraft's hit volume, recorded once a
//! tick where the hit search reads the current one, so a gun round fired by a
//! human can be tested against the targets as the shooter's screen showed
//! them.
//!
//! The history is mission state: an exact checkpoint (stage H) must carry it,
//! and the rewinds of the rounds in flight, with the rest of combat.

use crate::attitude::{Basis, Vector};
use std::collections::VecDeque;

/// Ticks of hit volumes kept: one second at 120 Hz.
pub const HISTORY_TICKS: usize = 120;
/// The longest rewind a round can carry, ticks (500 ms). The host caps the
/// part beyond the seat's interpolation delay first; this is the whole.
pub const MAX_REWIND_TICKS: u16 = 60;

/// One aircraft's hit volume as the hit search reads it for a tick: where the
/// aircraft is, where it was the tick before (a gun round's segment is tested
/// in the aircraft's own moving frame), its attitude and its radius.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HitVolume {
    pub position: Vector,
    pub previous: Vector,
    pub basis: Basis,
    pub radius: f64,
}

/// Every aircraft's volume for one tick, in id order.
#[derive(Clone, Debug, Default)]
struct Frame {
    tick: u64,
    volumes: Vec<(u32, HitVolume)>,
}

/// The last [`HISTORY_TICKS`] ticks of hit volumes, oldest first.
#[derive(Clone, Debug, Default)]
pub struct History {
    frames: VecDeque<Frame>,
}

impl History {
    /// Records the volumes of `tick`, dropping the oldest tick beyond
    /// [`HISTORY_TICKS`]. A tick recorded again replaces nothing: ticks only
    /// move forward, and a tick older than the newest starts the history over
    /// (a combat state rebuilt for a restart starts empty anyway).
    pub fn record(&mut self, tick: u64, volumes: impl IntoIterator<Item = (u32, HitVolume)>) {
        if self.frames.back().is_some_and(|newest| newest.tick >= tick) {
            self.frames.clear();
        }
        let mut frame = if self.frames.len() >= HISTORY_TICKS {
            self.frames.pop_front().unwrap_or_default()
        } else {
            Frame::default()
        };
        frame.tick = tick;
        frame.volumes.clear();
        frame.volumes.extend(volumes);
        frame.volumes.sort_by_key(|(id, _)| *id);
        self.frames.push_back(frame);
    }

    /// Aircraft `id`'s volume `rewind` ticks before the newest recorded tick.
    /// An aircraft with no entry that far back (it did not exist yet, or the
    /// history is younger than the rewind) gives its oldest entry. `None` when
    /// the history holds nothing for it at all.
    pub fn volume(&self, id: u32, rewind: u16) -> Option<&HitVolume> {
        let newest = self.frames.back()?.tick;
        let wanted = newest.saturating_sub(u64::from(rewind));
        let start = self.frames.partition_point(|frame| frame.tick < wanted);
        self.frames.range(start..).find_map(|frame| {
            frame
                .volumes
                .binary_search_by_key(&id, |(id, _)| *id)
                .ok()
                .map(|index| &frame.volumes[index].1)
        })
    }

    /// Ticks recorded, at most [`HISTORY_TICKS`].
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    /// Nothing recorded yet.
    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// The oldest and newest ticks recorded.
    pub fn span(&self) -> Option<(u64, u64)> {
        Some((self.frames.front()?.tick, self.frames.back()?.tick))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn volume(x: f64) -> HitVolume {
        HitVolume {
            position: [x, 0., 0.],
            previous: [x - 1., 0., 0.],
            basis: Basis::new(0., 0., 0.),
            radius: 28.,
        }
    }

    #[test]
    fn keeps_one_second_and_answers_by_ticks_back() {
        let mut history = History::default();
        for tick in 1..=200u64 {
            history.record(
                tick,
                [(7, volume(tick as f64)), (3, volume(-(tick as f64)))],
            );
        }
        assert_eq!(history.len(), HISTORY_TICKS);
        assert_eq!(history.span(), Some((81, 200)));
        assert_eq!(history.volume(7, 0).unwrap().position[0], 200.);
        assert_eq!(history.volume(7, 31).unwrap().position[0], 169.);
        assert_eq!(history.volume(3, 60).unwrap().position[0], -140.);
        // Beyond the history: the oldest entry.
        assert_eq!(history.volume(7, 500).unwrap().position[0], 81.);
        assert!(history.volume(9, 0).is_none());
    }

    #[test]
    fn an_aircraft_younger_than_the_rewind_is_tested_at_its_oldest_entry() {
        let mut history = History::default();
        for tick in 1..=50u64 {
            let mut volumes = vec![(1, volume(tick as f64))];
            if tick >= 40 {
                volumes.push((2, volume(1000. + tick as f64)));
            }
            history.record(tick, volumes);
        }
        assert_eq!(history.volume(1, 30).unwrap().position[0], 20.);
        assert_eq!(history.volume(2, 30).unwrap().position[0], 1040.);
        assert_eq!(history.volume(2, 5).unwrap().position[0], 1045.);
    }

    #[test]
    fn a_tick_that_goes_back_starts_over() {
        let mut history = History::default();
        for tick in 1..=10u64 {
            history.record(tick, [(1, volume(tick as f64))]);
        }
        history.record(3, [(1, volume(-3.))]);
        assert_eq!(history.len(), 1);
        assert_eq!(history.volume(1, 60).unwrap().position[0], -3.);
    }
}

/// Rewound gun rounds in a combat step: against an AI row and another
/// ownship, the cap, and weapons that never rewind.
#[cfg(test)]
mod hit_tests {
    use super::super::tests::{fixture, target};
    use super::super::*;
    use super::{HISTORY_TICKS, MAX_REWIND_TICKS};

    /// 500 knots, feet per second.
    const CROSSING_FPS: f64 = 500. * 6076.12 / 3600.;
    /// Ticks between the trigger and the shooter's screen: a 150 ms round
    /// trip, a 100 ms interpolation delay and a tick of input margin.
    const VIEW_TICKS: u16 = 31;
    /// The step the burst starts on; the history is full enough by then.
    const FIRE: usize = 40;
    /// Steps the trigger is held.
    const BURST: usize = 3;

    /// Ownship 0 at the origin, 1,000 ft up, facing north.
    fn shooter() -> Launcher {
        Launcher {
            position: [0., 1000., 0.],
            basis: Basis::new(0., 0., 0.),
            speed_fps: 300.,
            velocity: [0., 0., 300.],
            bay_ready: true,
            radar_power: true,
            radar: false,
            jammer: false,
            alive: true,
            body_present: true,
            controls: sensors::Controls::default(),
        }
    }

    /// A state whose ownship 0 carries a gun.
    fn gunship() -> State {
        let mut s = fixture(false);
        s.own_mut().config.stations[0].weapon.source = AircraftId::F18.gun().into();
        s
    }

    /// Where ownship 0's first round crosses `range` feet north, fired on step
    /// [`FIRE`] with nothing in the way: the step after the burst starts and
    /// the round's height there.
    fn arrival(range: f64) -> (usize, f64) {
        let mut s = gunship();
        let input = |held| OwnshipInput {
            aircraft: 0,
            held,
            launcher: shooter(),
        };
        for step in 0..FIRE + 200 {
            let held = (FIRE..FIRE + BURST).contains(&step);
            s.step(&[input(held)], |_, _| 0.);
            if let Some(p) = s.projectiles.iter().min_by_key(|p| p.id)
                && p.previous[2] < range
                && p.position[2] >= range
            {
                return (step - FIRE, p.position[1]);
            }
        }
        panic!("the round never reached {range} ft");
    }

    /// Crossing east at 500 knots, placed so that `VIEW_TICKS` before the
    /// first round gets there it is dead ahead at `range`: where the
    /// shooter's delayed screen shows it as the round arrives.
    fn crossing_start(range: f64) -> (Vector, Vector) {
        let (after, height) = arrival(range);
        // A row moves before the search each step, so on step k it is
        // searched at start + (k + 1) steps of travel.
        let seen = FIRE + after - usize::from(VIEW_TICKS);
        let x = -(seen as f64 + 1.) * CROSSING_FPS / 120.;
        ([x, height, range], [CROSSING_FPS, 0., 0.])
    }

    fn damaged(events: &[Event], id: u32) -> bool {
        events.iter().any(|e| match e {
            Event::Hit(target) => *target == id,
            Event::OwnshipDamaged { aircraft, .. } => *aircraft == id,
            _ => false,
        })
    }

    /// The burst against an AI row crossing ahead, with `rewind`: the events,
    /// the state after, and the gap between each hit effect and the row as it
    /// was on the tick of the hit.
    fn at_a_row(rewind: u16) -> (Vec<Event>, State, Vec<f64>) {
        let mut s = gunship();
        let (start, velocity) = crossing_start(1500.);
        let mut row = target(7, start, 1000, 0);
        row.velocity = velocity;
        row.basis = Basis::new(std::f64::consts::FRAC_PI_2, 0., 0.);
        row.radius = 60.;
        s.targets.push(row);
        let mut events = Vec::new();
        let mut gaps = Vec::new();
        for step in 0..FIRE + 200 {
            let held = (FIRE..FIRE + BURST).contains(&step);
            let effects = s.effects.len();
            let stepped = s.step_rewound(
                &[OwnshipInput {
                    aircraft: 0,
                    held,
                    launcher: shooter(),
                }],
                &[(0, rewind)],
                |_, _| 0.,
                |_, _| false,
            );
            if damaged(&stepped, 7) {
                let row = s.targets.iter().find(|t| t.id == 7).unwrap().position;
                gaps.extend(
                    s.effects[effects.min(s.effects.len())..]
                        .iter()
                        .filter(|e| e.kind == EffectKind::Hit)
                        .map(|e| missiles::length(std::array::from_fn(|i| e.position[i] - row[i]))),
                );
            }
            events.extend(stepped);
        }
        (events, s, gaps)
    }

    #[test]
    fn a_rewound_burst_hits_a_row_where_the_shooter_saw_it_and_misses_without() {
        let (events, s, gaps) = at_a_row(VIEW_TICKS);
        assert!(damaged(&events, 7), "{events:?}");
        assert!(s.ownship(0).unwrap().hits > 0);
        // The hit shows on the aircraft as it is now, not 31 ticks (218 ft)
        // back where the shooter saw it.
        assert!(!gaps.is_empty());
        assert!(gaps.iter().all(|&gap| gap < 60.), "{gaps:?}");
        let (none, _, _) = at_a_row(0);
        assert!(!damaged(&none, 7), "{none:?}");
    }

    #[test]
    fn a_rewound_burst_hits_another_ownship_where_the_shooter_saw_it() {
        let run = |rewind: u16| {
            let mut s = gunship();
            let config = s.own().configuration().clone();
            s.add_ownship(Ownship::new(1, Side(2), config, true).unwrap())
                .unwrap();
            let (start, velocity) = crossing_start(600.);
            let mut events = Vec::new();
            for step in 0..FIRE + 200 {
                let held = (FIRE..FIRE + BURST).contains(&step);
                // The other ownship's position is its launcher's, which the
                // host moves before the step.
                let moved = (step + 1) as f64 / 120.;
                let target = Launcher {
                    position: std::array::from_fn(|i| start[i] + velocity[i] * moved),
                    basis: Basis::new(std::f64::consts::FRAC_PI_2, 0., 0.),
                    speed_fps: CROSSING_FPS,
                    velocity,
                    ..shooter()
                };
                events.extend(s.step_rewound(
                    &[
                        OwnshipInput {
                            aircraft: 0,
                            held,
                            launcher: shooter(),
                        },
                        OwnshipInput {
                            aircraft: 1,
                            held: false,
                            launcher: target,
                        },
                    ],
                    &[(0, rewind)],
                    |_, _| 0.,
                    |_, _| false,
                ));
            }
            events
        };
        let events = run(VIEW_TICKS);
        assert!(damaged(&events, 1), "{events:?}");
        assert!(!damaged(&events, 0));
        let none = run(0);
        assert!(!damaged(&none, 1), "{none:?}");
    }

    /// Ownship 0 fires one burst from `station`'s weapon with a rewind of
    /// `given`; the rewinds its rounds carry.
    fn carried(gun: bool, given: u16) -> Vec<u16> {
        let mut s = if gun { gunship() } else { fixture(false) };
        let mut carried = Vec::new();
        for step in 0..20 {
            s.step_rewound(
                &[OwnshipInput {
                    aircraft: 0,
                    held: step < 10,
                    launcher: shooter(),
                }],
                &[(0, given)],
                |_, _| 0.,
                |_, _| false,
            );
            for p in &s.projectiles {
                carried.push(s.rewind_of(p.id));
            }
        }
        assert!(!carried.is_empty(), "nothing was fired");
        carried
    }

    #[test]
    fn a_rewind_is_capped_and_only_gun_rounds_carry_one() {
        assert!(carried(true, VIEW_TICKS).iter().all(|&r| r == VIEW_TICKS));
        assert!(carried(true, 500).iter().all(|&r| r == MAX_REWIND_TICKS));
        // Rockets, missiles and bombs never rewind.
        assert!(carried(false, VIEW_TICKS).iter().all(|&r| r == 0));
    }

    #[test]
    fn every_aircraft_is_recorded_once_a_tick_and_rounds_leave_no_rewind_behind() {
        let mut s = gunship();
        s.targets.push(target(7, [0., 1000., 5000.], 100, 0));
        for step in 0..1200 {
            s.step_rewound(
                &[OwnshipInput {
                    aircraft: 0,
                    held: step < 5,
                    launcher: shooter(),
                }],
                &[(0, VIEW_TICKS)],
                |_, _| 0.,
                |_, _| false,
            );
        }
        let history = s.hit_volumes();
        assert_eq!(history.len(), HISTORY_TICKS);
        assert_eq!(history.span(), Some((1081, 1200)));
        assert!(history.volume(0, 0).is_some() && history.volume(7, 0).is_some());
        assert!(s.projectiles.is_empty());
        assert!(s.rewinds.is_empty());
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "rewind_checkpoint.rs"]
mod checkpoint;
