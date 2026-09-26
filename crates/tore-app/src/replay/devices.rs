//! The released chaff and flares at any tick of a recording, flown again
//! from their `combat.countermeasure` entries exactly as combat flew them:
//! the same release, the same number and so the same look, stepped one tick
//! at a time over the same ground. Nothing here remembers how the playhead
//! reached a tick, so reverse play shows what forward play showed.
//!
//! A device lives at most [`LIFE_TICKS`], so after a quiet stretch nothing
//! is left to fly, and each stretch that follows releases is flown from its
//! start. A copy is kept every [`KEY_TICKS`] inside a stretch, the
//! [`MAX_KEYS`] nearest where the playhead has been, so a jump or reverse
//! play steps at most a second from one. Opinionated addition requested by
//! John on 2026-09-26; the keying is an agent decision. See
//! docs/REPLAYS.md.
use crate::replay::convert::{self, DeviceRelease};
use crate::terrain::World;
use std::collections::BTreeMap;
use tore_replay::{TimedEvent, vocab::field, vocab::kind};
use tore_sim::combat::countermeasures::Devices;
use tore_sim::combat::live::EffectKind;

/// The longest a device lives, in ticks: a flare burns for 3,600 and its
/// last smoke puff fades within 361 more, and chaff lasts 2,400. With a
/// margin.
pub const LIFE_TICKS: u64 = 4_000;
/// Ticks between kept copies: one second.
pub const KEY_TICKS: u64 = 120;
/// Kept copies at most: about eight minutes of stretches with devices.
pub const MAX_KEYS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq)]
enum Change {
    Release(DeviceRelease),
    Cleared,
}

/// Every device of a recording, ready to fly at any tick.
pub struct DeviceTrack {
    /// Releases and range resets by the combat tick after whose step they
    /// happened, releases in number order.
    changes: Vec<(u64, Change)>,
    /// Stretches that can hold devices: from a release to the last moment
    /// any of their devices can live, merged.
    busy: Vec<(u64, u64)>,
    /// Kept copies inside busy stretches, at multiples of [`KEY_TICKS`].
    keys: BTreeMap<u64, Devices>,
    /// The tick last flown to and the devices there.
    last: Option<(u64, Devices)>,
    none: Devices,
}

impl DeviceTrack {
    /// The track of a recording's entries, in tick order.
    pub fn new(events: &[TimedEvent]) -> Self {
        let mut changes: Vec<(u64, Change)> = events
            .iter()
            .filter_map(|entry| match entry.event.kind.as_str() {
                kind::COMBAT_COUNTERMEASURE => convert::device_release(&entry.event, entry.tick)
                    .map(|release| (release.tick, Change::Release(release))),
                kind::COMBAT_COUNTERMEASURES_CLEARED => {
                    let tick = entry
                        .event
                        .get(field::AFTER_TICK)
                        .and_then(|value| value.as_i64())
                        .and_then(|tick| u64::try_from(tick).ok())
                        .unwrap_or(entry.tick);
                    Some((tick, Change::Cleared))
                }
                _ => None,
            })
            .collect();
        // Entries are in tick order; a tick's releases fly in number order.
        changes.sort_by_key(|(tick, change)| {
            (
                *tick,
                match change {
                    Change::Release(release) => release.number,
                    Change::Cleared => 0,
                },
            )
        });
        let mut busy: Vec<(u64, u64)> = Vec::new();
        for (tick, change) in &changes {
            if !matches!(change, Change::Release(_)) {
                continue;
            }
            let end = tick.saturating_add(LIFE_TICKS);
            match busy.last_mut() {
                Some((_, last)) if *tick <= *last => *last = (*last).max(end),
                _ => busy.push((*tick, end)),
            }
        }
        Self {
            changes,
            busy,
            keys: BTreeMap::new(),
            last: None,
            none: Devices::default(),
        }
    }

    /// Whether the recording released any chaff or flare.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.busy.is_empty()
    }

    /// The devices after the step of `tick` and everything released after
    /// it, over `world`'s ground.
    pub fn at(&mut self, tick: u64, world: &World) -> &Devices {
        let ground = |x: f64, z: f64| f64::from(world.height(x as f32, z as f32));
        let Some(&(start, _)) = self
            .busy
            .iter()
            .find(|(start, end)| *start <= tick && tick <= *end)
        else {
            return &self.none;
        };
        // Carry on from the tick last flown to when it is a short way
        // behind, else from the nearest kept copy, else from the start.
        let resume = self
            .last
            .take()
            .filter(|(at, _)| (start..=tick).contains(at) && tick - at <= KEY_TICKS);
        let (mut at, mut devices) = match resume {
            Some(last) => last,
            None => match self.keys.range(start..=tick).next_back() {
                Some((at, devices)) => (*at, devices.clone()),
                None => {
                    let mut devices = Devices::default();
                    self.apply(start, &mut devices);
                    (start, devices)
                }
            },
        };
        while at < tick {
            at += 1;
            devices.step(&ground);
            self.apply(at, &mut devices);
            if at.is_multiple_of(KEY_TICKS) && !self.keys.contains_key(&at) {
                self.keys.insert(at, devices.clone());
                if self.keys.len() > MAX_KEYS {
                    let farthest = self
                        .keys
                        .keys()
                        .copied()
                        .max_by_key(|key| key.abs_diff(tick))
                        .expect("more than one key");
                    self.keys.remove(&farthest);
                }
            }
        }
        &self.last.insert((tick, devices)).1
    }

    /// The releases and resets after the step of `tick`.
    fn apply(&self, tick: u64, devices: &mut Devices) {
        let first = self.changes.partition_point(|(at, _)| *at < tick);
        for (_, change) in self.changes[first..]
            .iter()
            .take_while(|(at, _)| *at == tick)
        {
            match change {
                Change::Release(device) => {
                    devices.continue_after(device.number.saturating_sub(1));
                    if device.kind == EffectKind::Chaff {
                        devices.release_chaff(device.release);
                    } else {
                        devices.release_flare(device.release);
                    }
                }
                Change::Cleared => *devices = Devices::default(),
            }
        }
    }
}

/// A fingerprint of every flare and chaff cloud's exact state, private
/// fields included, to check a rebuilt track against the devices combat
/// flew. The count of releases so far is left out: nothing draws it, and a
/// replay numbers each release from its entry.
pub fn digest(devices: &Devices) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    format!("{:?} {:?}", devices.flares, devices.chaff).hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_replay::Event;
    use tore_sim::attitude::Basis;
    use tore_sim::combat::countermeasures::Release;

    fn released(tick: u64, number: u64, kind: EffectKind, x: f64) -> TimedEvent {
        let device = DeviceRelease {
            owner: 3,
            kind,
            release: Release {
                position: [x, 3_000., 1_000.],
                velocity: [0., 0., 600.],
                basis: Basis::new(0.2, 0.1, 0.3),
            },
            number,
            tick,
        };
        TimedEvent {
            tick,
            event: convert::device_event(&device, None),
        }
    }

    /// Flies `releases` the way combat does, one tick at a time, and returns
    /// the devices after each tick.
    fn flown(entries: &[TimedEvent], last: u64, world: &World) -> Vec<Devices> {
        let ground = |x: f64, z: f64| f64::from(world.height(x as f32, z as f32));
        let mut devices = Devices::default();
        let mut states = Vec::new();
        for tick in 0..=last {
            if tick > 0 {
                devices.step(&ground);
            }
            for entry in entries.iter().filter(|e| e.tick == tick) {
                match entry.event.kind.as_str() {
                    kind::COMBAT_COUNTERMEASURE => {
                        let device = convert::device_release(&entry.event, tick).unwrap();
                        if device.kind == EffectKind::Chaff {
                            devices.release_chaff(device.release);
                        } else {
                            devices.release_flare(device.release);
                        }
                    }
                    _ => devices = Devices::default(),
                }
            }
            states.push(devices.clone());
        }
        states
    }

    #[test]
    fn any_tick_in_any_order_matches_flying_every_tick_in_order() {
        let world = crate::terrain::tests::world();
        let mut entries = vec![
            released(10, 1, EffectKind::Flare, 100.),
            released(10, 2, EffectKind::Chaff, 120.),
            released(600, 3, EffectKind::Flare, 140.),
            released(601, 4, EffectKind::Flare, 160.),
        ];
        // A range reset, then numbering starts again.
        entries.push(TimedEvent {
            tick: 5_000,
            event: Event::new(kind::COMBAT_COUNTERMEASURES_CLEARED),
        });
        entries.push(released(5_000, 1, EffectKind::Flare, 180.));
        entries.push(released(9_990, 2, EffectKind::Chaff, 200.));
        let last = 14_500;
        let truth = flown(&entries, last, &world);
        let mut track = DeviceTrack::new(&entries);
        assert!(!track.is_empty());
        // Forwards, far jumps both ways, reverse play and small steps.
        let visits = (0..300)
            .chain([4_700, 12, 4_999, 5_000, 5_001, 14_500, 3_000])
            .chain((9_900..10_300).rev())
            .chain((2_000..2_200).step_by(7))
            .chain([13_990, 13_991, 13_989, 8_000, 0]);
        for tick in visits {
            let devices = track.at(tick, &world);
            let expected = &truth[tick as usize];
            assert!(
                devices.flares == expected.flares && devices.chaff == expected.chaff,
                "devices at tick {tick} differ"
            );
            assert_eq!(digest(devices), digest(expected));
        }
        // Quiet stretches have nothing: every device was gone by then.
        assert!(truth[4_700].flares.is_empty() && truth[4_700].chaff.is_empty());
        assert!(track.keys.len() <= MAX_KEYS);
        // The flares really lived, landed or burned out in these stretches.
        assert!(truth[1_000].flares.len() == 6 && !truth[1_000].chaff.is_empty());
    }

    #[test]
    fn a_recording_without_devices_has_none_at_every_tick() {
        let world = crate::terrain::tests::world();
        let mut track = DeviceTrack::new(&[]);
        assert!(track.is_empty());
        for tick in [0, 5, 100_000] {
            let devices = track.at(tick, &world);
            assert!(devices.flares.is_empty() && devices.chaff.is_empty());
        }
    }
}
