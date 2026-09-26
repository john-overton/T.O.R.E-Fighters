//! One pass over a whole recording, in the background, collecting what the
//! viewer needs from far away in time: flight path samples for the trails,
//! the hit point changes of ground objects (which buildings are standing at
//! any moment), and the player's pose on every tick, which drives the
//! weather. The pass decodes each chunk once and sends what it found back a
//! chunk at a time, so the viewer can play while it runs; everything it
//! builds depends only on the recording, never on how it was watched.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, mpsc};
use tore_replay::{Frame, Recording};

/// Ticks between trail samples: ten a second.
pub const SAMPLE_TICKS: u64 = 12;

/// What the weather needs from the player on one tick: where it is, which
/// way it faces and how fast it flies, as the live weather step reads them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct View {
    pub position: [f64; 3],
    /// Yaw, pitch and bank, radians.
    pub attitude: [f64; 3],
    /// Feet per second.
    pub airspeed: f64,
}

/// A guided weapon's path.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Path {
    /// The aircraft that fired it.
    pub owner: u32,
    /// Samples: the first where it was first seen, then every
    /// [`SAMPLE_TICKS`].
    pub samples: Vec<(u64, [f32; 3])>,
}

/// What one chunk contributes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Batch {
    first: u64,
    last: u64,
    aircraft: Vec<(u32, u64, [f32; 3])>,
    missiles: Vec<(u32, u32, u64, [f32; 3])>,
    surface: Vec<(u64, u32, i32)>,
    player: Vec<(u64, View)>,
}

/// Everything the pass has found so far.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracks {
    /// First tick of the recording.
    first: u64,
    /// Every aircraft's position ten times a second, and where it was first
    /// seen.
    pub aircraft: BTreeMap<u32, Vec<(u64, [f32; 3])>>,
    pub missiles: BTreeMap<u32, Path>,
    /// Ground object hit point changes in tick order: tick, object, hit points.
    pub surface: Vec<(u64, u32, i32)>,
    /// The player's view on every tick from `first`; a tick with no frame
    /// (a gap, or no player) repeats the one before it.
    player: Vec<View>,
    /// Every tick before this has been scanned.
    scanned: u64,
    done: bool,
}

fn sampled(tick: u64, seen: bool) -> bool {
    !seen || tick.is_multiple_of(SAMPLE_TICKS)
}

/// Everything `frames` hold for the tracks. `guns` are the weapon ids of
/// gun rounds, which have no trail.
fn batch(frames: &[Frame], guns: &BTreeSet<u32>, seen: &mut BTreeSet<(bool, u32)>) -> Batch {
    let mut out = Batch {
        first: frames.first().map_or(0, |f| f.tick),
        last: frames.last().map_or(0, |f| f.tick),
        ..Default::default()
    };
    for frame in frames {
        let tick = frame.tick;
        for state in &frame.aircraft {
            if sampled(tick, seen.contains(&(false, state.id))) {
                out.aircraft
                    .push((state.id, tick, state.position.map(|v| v as f32)));
            }
            seen.insert((false, state.id));
            if state.id == 0 {
                out.player.push((
                    tick,
                    View {
                        position: state.position,
                        attitude: state.attitude,
                        airspeed: state.airspeed,
                    },
                ));
            }
        }
        for projectile in frame
            .projectiles
            .iter()
            .filter(|p| !guns.contains(&p.weapon))
        {
            if sampled(tick, seen.contains(&(true, projectile.id))) {
                out.missiles.push((
                    projectile.id,
                    projectile.owner,
                    tick,
                    projectile.position.map(|v| v as f32),
                ));
            }
            seen.insert((true, projectile.id));
        }
        out.surface
            .extend(frame.surface_hp.iter().map(|&(id, hp)| (tick, id, hp)));
    }
    out
}

impl Tracks {
    pub fn new(first: u64) -> Self {
        Self {
            first,
            scanned: first,
            ..Default::default()
        }
    }

    /// Adds one chunk's findings. Chunks arrive in tick order.
    pub fn merge(&mut self, batch: Batch) {
        for (id, tick, position) in batch.aircraft {
            self.aircraft.entry(id).or_default().push((tick, position));
        }
        for (id, owner, tick, position) in batch.missiles {
            let path = self.missiles.entry(id).or_insert_with(|| Path {
                owner,
                samples: Vec::new(),
            });
            path.samples.push((tick, position));
        }
        self.surface.extend(batch.surface);
        for (tick, view) in batch.player {
            let index = tick.saturating_sub(self.first) as usize;
            // Fill a gap, or ticks before the player first appears, with the
            // nearest view before them (or this one, at the very start).
            let fill = self.player.last().copied().unwrap_or(view);
            if index > self.player.len() {
                self.player.resize(index, fill);
            }
            if index == self.player.len() {
                self.player.push(view);
            }
        }
        self.scanned = self.scanned.max(batch.last + 1);
    }

    /// Every tick before this has been scanned.
    #[cfg(test)]
    pub fn scanned(&self) -> u64 {
        self.scanned
    }

    #[cfg(test)]
    pub fn done(&self) -> bool {
        self.done
    }

    /// The player's view at `tick`, once the pass has reached it. Ticks
    /// before the first frame read as the first, ticks after the player's
    /// last frame as the last, and a recording without a player as standing
    /// still at the origin.
    pub fn view(&self, tick: u64) -> Option<View> {
        if tick >= self.scanned && !self.done {
            return None;
        }
        let index = tick.saturating_sub(self.first) as usize;
        Some(
            self.player
                .get(index)
                .or(self.player.last())
                .copied()
                .unwrap_or_default(),
        )
    }

    /// Ground objects destroyed by `tick`: the last recorded hit points are
    /// zero or less.
    pub fn destroyed(&self, tick: u64) -> BTreeSet<u32> {
        let mut hp = BTreeMap::new();
        for &(when, id, value) in &self.surface {
            if when > tick {
                break;
            }
            hp.insert(id, value);
        }
        hp.into_iter()
            .filter(|(_, value)| *value <= 0)
            .map(|(id, _)| id)
            .collect()
    }

    /// Scans a whole recording on this thread. The background pass does the
    /// same a chunk at a time.
    #[cfg(test)]
    pub fn scan(recording: &Recording) -> Self {
        let mut tracks = Self::new(recording.first_tick().unwrap_or(0));
        let guns = guns(recording);
        let mut seen = BTreeSet::new();
        for index in 0..recording.chunks().len() {
            match recording.decode_chunk(index) {
                Ok(frames) => tracks.merge(batch(&frames, &guns, &mut seen)),
                Err(error) => log::warn!("Replay: chunk {index} is unreadable: {error}"),
            }
        }
        tracks.done = true;
        tracks
    }
}

/// Weapon ids of gun rounds.
fn guns(recording: &Recording) -> BTreeSet<u32> {
    recording
        .weapons()
        .filter(|w| w.class == tore_replay::WeaponClass::Gun)
        .map(|w| w.id)
        .collect()
}

enum Message {
    Batch(Batch),
    Done,
}

/// The background pass. Dropping it stops the pass at its next chunk.
pub struct Scanner {
    receiver: mpsc::Receiver<Message>,
}

impl Scanner {
    pub fn start(recording: Arc<Recording>) -> Self {
        // A few chunks of slack; the pass waits for the viewer beyond that.
        let (sender, receiver) = mpsc::sync_channel(8);
        let spawned = std::thread::Builder::new()
            .name("replay tracks".into())
            .spawn(move || {
                let guns = guns(&recording);
                let mut seen = BTreeSet::new();
                for index in 0..recording.chunks().len() {
                    let frames = match recording.decode_chunk(index) {
                        Ok(frames) => frames,
                        Err(error) => {
                            log::warn!("Replay: chunk {index} is unreadable: {error}");
                            continue;
                        }
                    };
                    if sender
                        .send(Message::Batch(batch(&frames, &guns, &mut seen)))
                        .is_err()
                    {
                        return;
                    }
                }
                let _ = sender.send(Message::Done);
            });
        if let Err(error) = spawned {
            log::warn!("Replay: could not start the track pass: {error}");
        }
        Self { receiver }
    }

    /// Merges whatever has arrived. True when anything did.
    pub fn poll(&mut self, tracks: &mut Tracks) -> bool {
        let mut changed = false;
        while let Ok(message) = self.receiver.try_recv() {
            changed = true;
            match message {
                Message::Batch(batch) => tracks.merge(batch),
                Message::Done => tracks.done = true,
            }
        }
        changed
    }

    /// Waits for the whole pass, for captures and tests.
    pub fn wait(&mut self, tracks: &mut Tracks) {
        while !tracks.done {
            match self.receiver.recv() {
                Ok(Message::Batch(batch)) => tracks.merge(batch),
                Ok(Message::Done) | Err(_) => tracks.done = true,
            }
        }
    }
}

#[cfg(test)]
impl Tracks {
    /// Adds the player's view for one tick, as a one-tick chunk would.
    pub(crate) fn push_view(&mut self, tick: u64, view: View) {
        self.merge(Batch {
            first: tick,
            last: tick,
            player: vec![(tick, view)],
            ..Default::default()
        });
    }

    pub(crate) fn finish(&mut self) {
        self.done = true;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::replay::fixture;

    #[test]
    fn the_background_pass_finds_what_a_direct_scan_finds() {
        let dir = crate::replay::tests::TempDir::new("tracks");
        let recording = Arc::new(fixture::recording(dir.path(), "pass"));
        let direct = Tracks::scan(&recording);
        let mut scanner = Scanner::start(Arc::clone(&recording));
        let mut tracks = Tracks::new(recording.first_tick().unwrap());
        scanner.wait(&mut tracks);
        assert_eq!(tracks, direct);
        assert!(tracks.done());
        // Aircraft are sampled ten times a second, starting where they
        // first appear.
        let lead = &tracks.aircraft[&1];
        assert_eq!(lead[0].0, fixture::FIRST);
        assert!(lead[1..].iter().all(|(tick, _)| tick % SAMPLE_TICKS == 0));
        let late = &tracks.aircraft[&fixture::LATE];
        assert_eq!(late[0].0, fixture::LATE_FROM);
        // Missiles have paths; gun rounds do not.
        assert_eq!(tracks.missiles.len(), 1);
        let path = &tracks.missiles[&fixture::MISSILE];
        assert_eq!(path.owner, 1);
        assert_eq!(path.samples[0].0, fixture::LAUNCH);
        // The gap reads as the tick before it.
        let before = tracks.view(fixture::GAP.0 - 1).unwrap();
        assert_eq!(tracks.view(fixture::GAP.0 + 5), Some(before));
        let after = tracks.view(fixture::GAP.1 + 1).unwrap().position;
        let expected = fixture::position(0, fixture::GAP.1 + 1);
        assert!((0..3).all(|i| (after[i] - expected[i]).abs() < 0.02));
        // Ticks before the first read as the first.
        assert_eq!(tracks.view(0), tracks.view(fixture::FIRST));
        assert!(tracks.view(fixture::LAST + 10).is_some());
    }

    #[test]
    fn buildings_fall_when_their_hit_points_run_out() {
        let dir = crate::replay::tests::TempDir::new("tracks-surface");
        let tracks = Tracks::scan(&fixture::recording(dir.path(), "surface"));
        let [(hit, id), (down, _)] = fixture::SURFACE;
        assert!(tracks.destroyed(hit).is_empty());
        assert!(tracks.destroyed(down - 1).is_empty());
        assert_eq!(tracks.destroyed(down), BTreeSet::from([id]));
        assert_eq!(tracks.destroyed(fixture::LAST), BTreeSet::from([id]));
    }

    #[test]
    fn views_wait_for_the_pass() {
        let mut tracks = Tracks::new(10);
        assert_eq!(tracks.view(10), None);
        let view = |x: f64| View {
            position: [x, 0., 0.],
            ..Default::default()
        };
        tracks.merge(Batch {
            first: 12,
            last: 13,
            player: vec![(12, view(1.)), (13, view(2.))],
            ..Default::default()
        });
        assert_eq!(tracks.scanned(), 14);
        // Ticks 10 and 11 had no player yet: they read as the first view.
        assert_eq!(tracks.view(10), Some(view(1.)));
        assert_eq!(tracks.view(13), Some(view(2.)));
        assert_eq!(tracks.view(14), None);
        tracks.merge(Batch {
            first: 20,
            last: 20,
            player: vec![(20, view(3.))],
            ..Default::default()
        });
        assert_eq!(tracks.view(17), Some(view(2.)));
        assert_eq!(tracks.view(20), Some(view(3.)));
    }
}
