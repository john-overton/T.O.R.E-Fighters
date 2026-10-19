//! Native device boundary. Unsafe calls are confined to platform modules.
use std::{
    collections::BTreeMap,
    collections::BTreeSet,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};
use tore_input::Event;
mod head;
pub use head::{HeadPose, HeadTracker};
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::Platform;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::Platform;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos::Platform;
#[derive(Clone, Debug)]
pub enum Kind {
    Button,
    Axis,
    Relative,
    Position,
}
#[derive(Clone, Debug)]
pub struct Control {
    pub id: String,
    pub kind: Kind,
    pub min: f64,
    pub max: f64,
    pub value: f64,
}
#[derive(Clone, Debug)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub controls: Vec<Control>,
    pub rumble: bool,
}
#[derive(Clone, Debug)]
pub enum Notification {
    Connected(Device),
    Disconnected(String),
    Input(Event),
    Warning(String),
    Feedback(String, Result<(), String>),
    Overflow,
}
enum Request {
    Rumble(String, f64, f64, Duration),
}
/// How long the device thread rests between polls. Four milliseconds only
/// while a controller is connected and the game is in flight; a controller
/// in a menu needs one poll a frame; with none connected the thread only
/// watches for hotplug, which the platform scans for on its own timer.
pub fn rest_between_polls(connected: bool, flight: bool) -> Duration {
    match (connected, flight) {
        (true, true) => Duration::from_millis(4),
        (true, false) => Duration::from_millis(16),
        (false, _) => Duration::from_millis(50),
    }
}
pub struct Backend {
    rx: mpsc::Receiver<Notification>,
    tx: mpsc::SyncSender<Request>,
    stop_requested: Arc<AtomicBool>,
    quit: Arc<AtomicBool>,
    /// The game is in flight (or otherwise needs the fast cadence).
    flight: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
}
impl Backend {
    /// No hardware access, for classic keyboard-only sessions and headless bridge tests.
    pub fn disabled() -> Self {
        let (_, rx) = mpsc::sync_channel(1);
        let (tx, _) = mpsc::sync_channel(1);
        Self {
            rx,
            tx,
            stop_requested: Arc::new(AtomicBool::new(false)),
            quit: Arc::new(AtomicBool::new(false)),
            flight: Arc::new(AtomicBool::new(false)),
            worker: None,
        }
    }

    pub fn start() -> Self {
        let (output, rx) = mpsc::sync_channel(4096);
        let (tx, commands) = mpsc::sync_channel(32);
        let quit = Arc::new(AtomicBool::new(false));
        let stopped = quit.clone();
        let stop_requested = Arc::new(AtomicBool::new(false));
        let stop = stop_requested.clone();
        let flight = Arc::new(AtomicBool::new(false));
        let in_flight = flight.clone();
        let worker = thread::spawn(move || {
            let mut platform = match Platform::new() {
                Ok(p) => p,
                Err(e) => {
                    let _ = output.send(Notification::Warning(e.to_string()));
                    return;
                }
            };
            let mut deadlines = BTreeMap::new();
            let mut overflow = false;
            let mut connected = BTreeSet::new();
            while !stopped.load(Ordering::Relaxed) {
                if stop.swap(false, Ordering::AcqRel) {
                    platform.stop();
                    deadlines.clear();
                    for _ in commands.try_iter() {} // Discard effects queued in the previous context.
                }
                for command in commands.try_iter() {
                    match command {
                        Request::Rumble(id, strong, weak, duration) => {
                            let result = platform
                                .rumble(&id, strong, weak, duration)
                                .map_err(|e| e.to_string());
                            if result.is_ok() {
                                deadlines.insert(id.clone(), Instant::now() + duration);
                            }
                            let _ = output.try_send(Notification::Feedback(id, result));
                        }
                    }
                }
                let expired: Vec<_> = deadlines
                    .iter()
                    .filter(|(_, d)| Instant::now() >= **d)
                    .map(|(id, _)| id.clone())
                    .collect();
                for id in expired {
                    platform.stop_device(&id);
                    deadlines.remove(&id);
                }
                if overflow {
                    if output.try_send(Notification::Overflow).is_ok() {
                        // Recreate native state so all devices send fresh baselines.
                        platform.stop();
                        match Platform::new() {
                            Ok(p) => platform = p,
                            Err(_) => break,
                        }
                        overflow = false;
                    } else {
                        thread::sleep(Duration::from_millis(4));
                        continue;
                    }
                }
                for event in platform.poll() {
                    match &event {
                        Notification::Connected(d) => {
                            connected.insert(d.id.clone());
                        }
                        Notification::Disconnected(id) => {
                            connected.remove(id);
                        }
                        _ => {}
                    }
                    if output.try_send(event).is_err() {
                        overflow = true;
                        platform.stop();
                        break;
                    }
                }
                // Rumble stops on time whatever the rest between polls is.
                let mut rest =
                    rest_between_polls(!connected.is_empty(), in_flight.load(Ordering::Relaxed));
                if let Some(next) = deadlines.values().min() {
                    rest = rest.min(next.saturating_duration_since(Instant::now()));
                }
                thread::park_timeout(rest);
            }
            platform.stop();
        });
        Self {
            rx,
            tx,
            quit,
            stop_requested,
            flight,
            worker: Some(worker),
        }
    }
    pub fn drain(&self) -> Vec<Notification> {
        self.rx.try_iter().take(4096).collect()
    }
    pub fn rumble(
        &self,
        id: &str,
        strong: f64,
        weak: f64,
        duration: Duration,
    ) -> Result<(), String> {
        if !strong.is_finite()
            || !weak.is_finite()
            || !(0. ..=1.).contains(&strong)
            || !(0. ..=1.).contains(&weak)
            || duration < Duration::from_millis(1)
            || duration > Duration::from_secs(2)
        {
            return Err("feedback outside bounded range".into());
        }
        self.tx
            .try_send(Request::Rumble(id.into(), strong, weak, duration))
            .map_err(|_| String::from("feedback queue full"))?;
        self.wake();
        Ok(())
    }
    pub fn stop(&self) {
        self.stop_requested.store(true, Ordering::Release);
        self.wake();
    }
    /// Ends the device thread's rest, so a command is not left waiting.
    fn wake(&self) {
        if let Some(worker) = &self.worker {
            worker.thread().unpark();
        }
    }
    /// Tells the device thread whether the game is in flight, where a
    /// connected controller is polled every 4 ms. Elsewhere it rests longer
    /// ([`rest_between_polls`]). Going to flight wakes it at once.
    pub fn set_flight(&self, flight: bool) {
        if self.flight.swap(flight, Ordering::AcqRel) != flight && flight {
            self.wake();
        }
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            worker.thread().unpark();
            let _ = worker.join();
        }
    }
}
/// Whitespace-free persistent identity component, without lossy punctuation collisions.
pub(crate) fn encode(s: &str) -> String {
    s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn the_device_thread_rests_by_what_is_connected_and_where_the_game_is() {
        let ms = |connected, flight| rest_between_polls(connected, flight).as_millis();
        assert_eq!(ms(true, true), 4, "a controller in flight keeps 4 ms");
        assert_eq!(ms(true, false), 16, "menus need a poll a frame");
        assert_eq!(ms(false, true), 50);
        assert_eq!(ms(false, false), 50);
    }
    #[test]
    fn a_disabled_backend_takes_the_flight_flag_and_stop_without_a_thread() {
        let backend = Backend::disabled();
        backend.set_flight(true);
        backend.set_flight(false);
        backend.stop();
        assert!(backend.drain().is_empty());
    }
}
