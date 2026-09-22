//! Native device boundary. Unsafe calls are confined to platform modules.
use std::{
    collections::BTreeMap,
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
pub struct Backend {
    rx: mpsc::Receiver<Notification>,
    tx: mpsc::SyncSender<Request>,
    stop_requested: Arc<AtomicBool>,
    quit: Arc<AtomicBool>,
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
                    if output.try_send(event).is_err() {
                        overflow = true;
                        platform.stop();
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(4));
            }
            platform.stop();
        });
        Self {
            rx,
            tx,
            quit,
            stop_requested,
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
            .map_err(|_| "feedback queue full".into())
    }
    pub fn stop(&self) {
        self.stop_requested.store(true, Ordering::Release);
    }
}
impl Drop for Backend {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
/// Whitespace-free persistent identity component, without lossy punctuation collisions.
pub(crate) fn encode(s: &str) -> String {
    s.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
