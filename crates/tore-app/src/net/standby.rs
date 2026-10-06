//! The game's standby (stage K, slice K7a; docs/ARCHITECTURE.md, "Host
//! migration and rejoin"): a joined game that may host runs slice K2's
//! standby on a worker thread of its own, fed with the records its client
//! keeps, and reports its status to the host at most twice a second. When
//! its client says the game is due to take over (`Client::takeover_due`),
//! the session hands the standby to a new hosting thread
//! ([`crate::net::hosting::HostThread::take_over`]).
//!
//! The standby builds the flight the player holds: the spec the game's
//! client last built (the lobby's mission, then the flight with its
//! loadouts), checked against the Flight record's spec hash when there is
//! one. It also holds the words of the host's and the standby's migration
//! lines for the logs.

use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, PoisonError},
    time::Duration,
};
use tore_session::{
    Client,
    host::{ResumeNote, StandbyFigures},
    standby::{Builder, MissionKey, Note, StandbyThread},
    wire::messages::{Message, StandbyMark},
};
use tore_world::{
    mission::MissionSpec,
    resources::ResourceReads,
    world::{Seating, World},
};

/// A standby's status goes to the host this often at most (the design's
/// twice a second).
pub const STATUS_EVERY: Duration = Duration::from_millis(500);

/// The mission spec the player's game last built, shared with the standby's
/// builder on its thread.
pub type HeldSpec = Arc<Mutex<Option<MissionSpec>>>;

/// The FNV-1a 64 of `text`: a Flight record's spec hash.
pub fn spec_hash(text: &str) -> u64 {
    text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
    })
}

/// The standby's builder: the flight the player holds, built as the
/// session builds it (`World::new`, open seating), refused when a Flight
/// record names another.
pub fn builder(held: HeldSpec, resources: Arc<BTreeMap<String, Vec<u8>>>) -> Builder {
    Box::new(move |key: &MissionKey| {
        let spec = held
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or_else(|| "this game holds no mission yet".to_owned())?;
        if let Some(hash) = key.spec_hash
            && hash != spec_hash(&spec.to_text())
        {
            return Err("the host flies another mission than this game holds".into());
        }
        World::new(&spec, &ResourceReads::new(&resources), Seating::Open).map_err(|e| e.to_string())
    })
}

/// The game's standby: the worker thread and the status's pace.
#[derive(Debug)]
pub struct GameStandby {
    thread: StandbyThread,
    status_at: Option<Duration>,
}

impl GameStandby {
    /// Starts the standby's thread.
    pub fn start(
        held: HeldSpec,
        resources: Arc<BTreeMap<String, Vec<u8>>>,
    ) -> std::io::Result<Self> {
        Ok(Self {
            thread: StandbyThread::spawn(builder(held, resources))?,
            status_at: None,
        })
    }

    /// One turn after the client's update: the records the client kept go
    /// to the thread, and while the host has appointed it, its status goes
    /// to the host every [`STATUS_EVERY`]. The thread's notes since the last
    /// turn, for the logs.
    pub fn turn(&mut self, now: Duration, client: &mut Client) -> Vec<Note> {
        self.thread.records(client.take_standby_records());
        if self.thread.appointed()
            && self
                .status_at
                .is_none_or(|at| now.saturating_sub(at) >= STATUS_EVERY)
        {
            self.status_at = Some(now);
            client.request(now, Message::StandbyStatus(self.thread.status()));
        }
        self.thread.take_notes()
    }

    /// Whether the standby could take over now.
    pub fn ready(&self) -> bool {
        self.thread.ready()
    }

    /// Whether the host handed the game over to it.
    pub fn handed_over(&self) -> bool {
        self.thread.handover().is_some()
    }

    /// The thread, for the takeover.
    pub fn into_thread(self) -> StandbyThread {
        self.thread
    }
}

/// A standby's note as a line of the logs.
pub fn note_line(note: &Note) -> String {
    match note {
        Note::Appointed { warm: true } => "appointed a standby, warm".into(),
        Note::Appointed { warm: false } => "appointed a standby, cold".into(),
        Note::Dismissed => "no longer a standby".into(),
        Note::Flight { mission } => format!("flight of mission {mission} built"),
        Note::Checkpoint {
            tick,
            restored: true,
        } => format!("checkpoint at tick {tick} restored"),
        Note::Checkpoint {
            tick,
            restored: false,
        } => format!("checkpoint at tick {tick} kept"),
        Note::Refused(why) => format!("checkpoint refused: {why}"),
        Note::Warm { tick } => format!("warm again at tick {tick}"),
        Note::FellBehind { tick } => format!("fell behind at tick {tick}; cold"),
        Note::Mismatch { tick } => {
            format!("check at tick {tick} differed; waiting for a checkpoint")
        }
        Note::Overflow => "too much journal held; waiting for a checkpoint".into(),
        Note::Broken(why) => format!("behind: {why}"),
        Note::Ended(reason) => format!("mission ended ({reason:?})"),
        Note::Handover { last_tick } => format!("the host hands over after tick {last_tick}"),
    }
}

/// A host's migration note as a line of the game's log.
pub fn resume_line(note: &ResumeNote) -> String {
    match note {
        ResumeNote::TookOver {
            tick,
            replayed,
            took,
            absent,
        } => format!(
            "took the game over at tick {tick} ({replayed} ticks replayed in {} ms); {absent} players expected back",
            took.as_millis()
        ),
        ResumeNote::Resumed {
            callsign,
            flying,
            after,
        } => format!(
            "{callsign} resumed{} after {} ms",
            if *flying { " flying" } else { "" },
            after.as_millis()
        ),
        ResumeNote::Live {
            tick,
            after,
            fast_forward,
        } => format!(
            "live at tick {tick}, {} ms after the takeover, {fast_forward} ticks stepped to get there",
            after.as_millis()
        ),
        ResumeNote::Dropped { callsign } => {
            format!("{callsign} never resumed: dropped, its plane kept for it")
        }
        ResumeNote::TakenOver { by, tick } => {
            format!("player {by} says it took the game over at tick {tick}")
        }
        ResumeNote::HandedOver { to, last_tick } => {
            format!("handed the game over to player {to} after tick {last_tick}")
        }
        ResumeNote::SteppedDown => "stepped down: another game hosts now".into(),
    }
}

/// The host's standbys as one line of the game's log.
pub fn figures_line(figures: &[StandbyFigures]) -> String {
    if figures.is_empty() {
        return "standbys: none".into();
    }
    let each: Vec<String> = figures
        .iter()
        .map(|f| {
            let role = match f.role {
                StandbyMark::First => "first",
                StandbyMark::Second => "second",
                StandbyMark::None => "none",
            };
            format!(
                "{} {role}, {}, {}",
                f.callsign,
                if f.warm { "warm" } else { "cold" },
                if f.ready() { "ready" } else { "not ready" }
            )
        })
        .collect();
    format!("standbys: {}", each.join("; "))
}

#[cfg(test)]
#[path = "standby_tests.rs"]
mod tests;
