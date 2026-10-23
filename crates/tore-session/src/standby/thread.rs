//! The standby's worker thread (slice K2; docs/ARCHITECTURE.md, "On the
//! standby's side"): runs a [`Standby`] off the frame loop, in the game and
//! in `tore-bot`. The game hands it the records its client keeps
//! ([`crate::Client::take_standby_records`]) and reads back the status it
//! sends the host (at most twice a second, the sender's to pace) and the
//! notes for its net log; a takeover takes the standby's world and parts
//! from it, and stopping it joins the thread.
//!
//! The thread steps a warm copy in slices of [`SLICE`], reading new records
//! and requests between them, and builds the spare world when it has nothing
//! to step.

use super::{Budget, Builder, Note, Standby, Takeover};
use crate::wire::migration::StandbyStatus;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// How long the thread steps before it reads its requests again.
pub const SLICE: Duration = Duration::from_millis(20);
/// How long an idle thread waits for a request before it looks again.
const IDLE: Duration = Duration::from_millis(50);

enum Request {
    Records(Vec<Vec<u8>>),
    TakeOver(Sender<Result<Takeover, String>>),
    Stop,
}

/// What the thread shows its owner, refreshed after every slice.
#[derive(Debug, Default)]
struct Shown {
    status: StandbyStatus,
    ready: bool,
    appointed: bool,
    handover: Option<u32>,
    /// The ticks received and not yet stepped or held since the base.
    backlog: usize,
    notes: Vec<Note>,
    /// Records refused as unreadable.
    refused: u64,
}

/// A [`Standby`] on a worker thread of its own.
pub struct StandbyThread {
    requests: Sender<Request>,
    shown: Arc<Mutex<Shown>>,
    handle: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for StandbyThread {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StandbyThread")
            .field("status", &self.status())
            .finish_non_exhaustive()
    }
}

impl StandbyThread {
    /// Starts the thread, building its worlds with `builder`.
    pub fn spawn(builder: Builder) -> std::io::Result<Self> {
        let (requests, inbox) = mpsc::channel();
        let shown = Arc::new(Mutex::new(Shown::default()));
        let theirs = Arc::clone(&shown);
        let handle = std::thread::Builder::new()
            .name("tore-standby".into())
            .spawn(move || run(Standby::new(builder), &inbox, &theirs))?;
        Ok(Self {
            requests,
            shown,
            handle: Some(handle),
        })
    }

    /// Hands the thread records, in the order the host sent them.
    pub fn records(&self, records: Vec<Vec<u8>>) {
        if !records.is_empty() {
            let _ = self.requests.send(Request::Records(records));
        }
    }

    fn shown<T>(&self, read: impl FnOnce(&mut Shown) -> T) -> T {
        read(&mut self.shown.lock().unwrap_or_else(PoisonError::into_inner))
    }

    /// The status the standby reports to the host, as of the thread's last
    /// slice.
    pub fn status(&self) -> StandbyStatus {
        self.shown(|s| s.status)
    }

    /// Whether the standby could take over now.
    pub fn ready(&self) -> bool {
        self.shown(|s| s.ready)
    }

    /// Whether the host has appointed it.
    pub fn appointed(&self) -> bool {
        self.shown(|s| s.appointed)
    }

    /// The last tick a handing-over host steps, once its Handover arrived:
    /// time to take over.
    pub fn handover(&self) -> Option<u32> {
        self.shown(|s| s.handover)
    }

    /// The ticks received and not yet stepped (warm) or held since the base
    /// (cold).
    pub fn backlog(&self) -> usize {
        self.shown(|s| s.backlog)
    }

    /// Records refused as unreadable so far.
    pub fn refused(&self) -> u64 {
        self.shown(|s| s.refused)
    }

    /// The notes since the last call, for the net log.
    pub fn take_notes(&self) -> Vec<Note> {
        self.shown(|s| std::mem::take(&mut s.notes))
    }

    /// Takes over: the thread replays what it holds to its end and hands
    /// back the world and the parts, then ends. Refused when the standby is
    /// not ready, or when the thread has gone.
    pub fn take_over(mut self) -> Result<Takeover, String> {
        let (reply, answer) = mpsc::channel();
        if self.requests.send(Request::TakeOver(reply)).is_err() {
            self.join()?;
            return Err("the standby thread has stopped".into());
        }
        let result = answer
            .recv()
            .map_err(|_| "the standby thread stopped before taking over".to_owned());
        self.join()?;
        result?
    }

    /// Stops the thread and waits for it. An error says it had panicked.
    pub fn stop(mut self) -> Result<(), String> {
        let _ = self.requests.send(Request::Stop);
        self.join()
    }

    fn join(&mut self) -> Result<(), String> {
        match self.handle.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| "the standby thread panicked".to_owned()),
            None => Ok(()),
        }
    }
}

impl Drop for StandbyThread {
    fn drop(&mut self) {
        let _ = self.requests.send(Request::Stop);
        let _ = self.join();
    }
}

/// The thread's loop: read every request waiting, step a slice, build the
/// spare when idle, show the status; until a takeover or a stop.
fn run(mut standby: Standby, inbox: &Receiver<Request>, shown: &Mutex<Shown>) {
    let mut refused = 0;
    loop {
        let mut first = if standby.has_work() {
            match inbox.try_recv() {
                Ok(request) => Some(request),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Disconnected) => return,
            }
        } else {
            match inbox.recv_timeout(IDLE) {
                Ok(request) => Some(request),
                Err(RecvTimeoutError::Timeout) => None,
                Err(RecvTimeoutError::Disconnected) => return,
            }
        };
        while let Some(request) = first.take() {
            match request {
                Request::Records(records) => {
                    for record in &records {
                        if standby.receive(record).is_err() {
                            refused += 1;
                        }
                    }
                }
                Request::TakeOver(reply) => {
                    let _ = reply.send(standby.take_over());
                    return;
                }
                Request::Stop => return,
            }
            first = inbox.try_recv().ok();
        }
        if standby.has_work() {
            standby.step(Budget::Until(Instant::now() + SLICE));
        } else {
            standby.prepare();
        }
        let mut s = shown.lock().unwrap_or_else(PoisonError::into_inner);
        s.status = standby.status();
        s.ready = standby.ready();
        s.appointed = standby.appointed().is_some();
        s.handover = standby.handover();
        s.backlog = standby.backlog();
        s.notes.extend(standby.take_notes());
        s.refused = refused;
    }
}
