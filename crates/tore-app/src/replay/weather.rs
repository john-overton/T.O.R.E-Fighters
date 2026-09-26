//! The replay's weather: the world's environment re-stepped one tick at a
//! time exactly as live flight steps it, with a snapshot every second, so any
//! moment is at most a second of stepping away and playing backwards shows
//! the same sky as playing forwards.
//!
//! Live flight steps the weather from the view camera. A replay's camera is
//! wherever the viewer puts it, so the weather is stepped from the recorded
//! player's own forward view instead: the same input on every viewing, which
//! keeps each tick's weather fixed. That choice is fitted (agent decision,
//! 2026-09-26): the difference is only which way the sun-whitening
//! smoothing was looking. The palette is then resolved for the replay
//! camera's own altitude every frame.
//!
//! The environment's own state depends on time alone and is cheap to step,
//! so it is kept every ten seconds; the view presentation, which depends on
//! the player's path, is kept every second.
use crate::replay::tracks::{Tracks, View};
use crate::terrain::{Camera, World};
use tore_sim::environment::{Environment, Presentation};

/// Ticks between presentation snapshots: one second.
pub const VIEW_KEY_TICKS: u64 = 120;
/// Ticks between environment snapshots: ten seconds.
pub const ENVIRONMENT_KEY_TICKS: u64 = 1_200;

/// The camera live flight's forward view would have had.
fn camera(view: &View) -> Camera {
    let mut camera = Camera::new();
    camera.position = view.position.map(|v| v as f32);
    camera.yaw = view.attitude[0] as f32;
    camera.pitch = view.attitude[1] as f32;
    camera.roll = -view.attitude[2] as f32;
    camera
}

/// Weather snapshots and the tick the world's weather currently shows.
pub struct WeatherTrack {
    /// Environment at every multiple of [`ENVIRONMENT_KEY_TICKS`] built so far.
    environments: Vec<Environment>,
    /// Presentation at every multiple of [`VIEW_KEY_TICKS`] built so far.
    views: Vec<Presentation>,
    /// The newest tick built, and the weather there.
    frontier: (u64, Environment, Presentation),
    /// The recording's last tick: nothing is built beyond it.
    end: u64,
    /// The tick `world.weather` and `world.weather_presentation` hold.
    current: u64,
}

/// One live weather step, with the world's weather swapped for the given
/// state around it.
fn step(world: &mut World, environment: &mut Environment, view: &mut Presentation, input: &View) {
    std::mem::swap(&mut world.weather, environment);
    std::mem::swap(&mut world.weather_presentation, view);
    world.step_weather(input.airspeed, &camera(input));
    std::mem::swap(&mut world.weather, environment);
    std::mem::swap(&mut world.weather_presentation, view);
}

impl WeatherTrack {
    /// Starts from the world's weather as built, which is the weather at
    /// launch: tick 0. `end` is the recording's last tick.
    pub fn new(world: &World, end: u64) -> Self {
        let (environment, view) = (world.weather.clone(), world.weather_presentation.clone());
        Self {
            environments: vec![environment.clone()],
            views: vec![view.clone()],
            frontier: (0, environment, view),
            end,
            current: 0,
        }
    }

    /// The newest tick with snapshots up to it.
    #[cfg(test)]
    pub fn built(&self) -> u64 {
        self.frontier.0
    }

    /// Builds snapshots forwards by up to `budget` ticks, as far as the
    /// track pass has read the player's path.
    pub fn build(&mut self, world: &mut World, tracks: &Tracks, budget: u64) {
        let (tick, environment, view) = &mut self.frontier;
        let end = tick.saturating_add(budget).min(self.end);
        while *tick < end {
            let Some(input) = tracks.view(*tick + 1) else {
                break;
            };
            step(world, environment, view, &input);
            *tick += 1;
            if tick.is_multiple_of(VIEW_KEY_TICKS) {
                self.views.push(view.clone());
            }
            if tick.is_multiple_of(ENVIRONMENT_KEY_TICKS) {
                self.environments.push(environment.clone());
            }
        }
    }

    /// Puts the weather at `tick` into `world`. Until the track pass has
    /// read that far it shows the newest weather it can, and the next call
    /// catches up; the tick it shows is returned.
    pub fn seek(&mut self, world: &mut World, tracks: &Tracks, tick: u64) -> u64 {
        if tick > self.frontier.0 {
            self.build(world, tracks, tick - self.frontier.0);
        }
        let tick = tick.min(self.frontier.0);
        // A short step forwards continues from where the world is.
        if tick < self.current || tick - self.current > VIEW_KEY_TICKS {
            let key = tick / VIEW_KEY_TICKS * VIEW_KEY_TICKS;
            let slot = (key / ENVIRONMENT_KEY_TICKS) as usize;
            let mut environment = self.environments[slot].clone();
            for _ in slot as u64 * ENVIRONMENT_KEY_TICKS..key {
                environment.step();
            }
            world.weather = environment;
            world.weather_presentation = self.views[(key / VIEW_KEY_TICKS) as usize].clone();
            self.current = key;
        }
        while self.current < tick {
            let Some(input) = tracks.view(self.current + 1) else {
                break;
            };
            world.step_weather(input.airspeed, &camera(&input));
            self.current += 1;
        }
        self.current
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(ticks: u64) -> Tracks {
        let mut tracks = Tracks::new(1);
        for tick in 1..=ticks {
            let t = tick as f64;
            tracks.push_view(
                tick,
                View {
                    position: [
                        50_000. + t * 3.,
                        2_000. + (t * 0.01).sin() * 1_500.,
                        70_000.,
                    ],
                    attitude: [t * 0.001, 0.1, (t * 0.02).sin()],
                    airspeed: 300. + t % 500.,
                },
            );
        }
        tracks.finish();
        tracks
    }

    #[test]
    fn snapshots_equal_stepping_every_tick_from_launch() {
        let tracks = tracks(3_000);
        let mut sequential = crate::terrain::tests::world();
        let mut states = vec![(
            sequential.weather.clone(),
            sequential.weather_presentation.clone(),
        )];
        for tick in 1..=3_000 {
            let input = tracks.view(tick).unwrap();
            sequential.step_weather(input.airspeed, &camera(&input));
            states.push((
                sequential.weather.clone(),
                sequential.weather_presentation.clone(),
            ));
        }
        let mut world = crate::terrain::tests::world();
        let mut track = WeatherTrack::new(&world, 3_000);
        // Forwards a tick at a time, far jumps both ways, reverse play and
        // a small step back.
        let visits = (0..=300)
            .chain([2_999, 5, 1_199, 1_200, 1_201, 2_400, 119, 3_000])
            .chain((2_700..=2_900).rev())
            .chain([2_899, 2_901, 2_898]);
        for tick in visits {
            assert_eq!(track.seek(&mut world, &tracks, tick), tick);
            let (environment, view) = &states[tick as usize];
            assert!(
                world.weather == *environment && world.weather_presentation == *view,
                "weather at tick {tick}"
            );
        }
        assert_eq!(track.built(), 3_000);
        // Nothing is built past the end.
        track.build(&mut world, &tracks, 10_000);
        assert_eq!(track.built(), 3_000);
        assert_eq!(track.seek(&mut world, &tracks, 5_000), 3_000);
        assert_eq!(track.views.len(), 26);
        assert_eq!(track.environments.len(), 3);
    }

    #[test]
    fn weather_waits_for_the_track_pass() {
        let mut tracks = Tracks::new(1);
        for tick in 1..=100 {
            tracks.push_view(tick, View::default());
        }
        let mut world = crate::terrain::tests::world();
        let mut track = WeatherTrack::new(&world, 1_000);
        assert_eq!(track.seek(&mut world, &tracks, 500), 100);
        assert_eq!(world.weather.ticks(), 213);
        // Budgeted building stops at the pass too.
        track.build(&mut world, &tracks, 1_000);
        assert_eq!(track.built(), 100);
    }
}
