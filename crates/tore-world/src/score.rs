//! Score facts: what the mission core records for a networked game's scoring
//! each tick, for the host to tally. Stage F phase 2; see
//! docs/ARCHITECTURE.md, "Scoring".
//!
//! Slice F2-0 adds only the types the host and the mission core share.
//! Recording them (`World::set_scoring` and the tick's call) is slice F2-S's;
//! single player never turns scoring on, so its tick and fingerprint are
//! untouched.

use crate::seats::{Pilot, PlaneId, SeatId};

/// One aircraft of the roster and who flew it at the fact's tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Flown {
    pub plane: PlaneId,
    pub pilot: Pilot,
}

/// What was hit: a combat target, with its plane and pilot when it is an
/// aircraft of the roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Victim {
    /// The combat target id (a plane's id is its target id).
    pub target: u32,
    /// The plane and its pilot, when the target is a plane of the roster.
    pub flown: Option<Flown>,
    /// An aircraft or a helicopter: only these count as kills.
    pub aircraft: bool,
}

/// One fact of a tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fact {
    /// `victim` was killed. `shooter` is `None` when no plane of the roster
    /// fired (a ground site). `pilot_aboard`: the victim's pilot had not
    /// ejected (retail counts a human killed before ejecting twice).
    Kill {
        shooter: Option<Flown>,
        victim: Victim,
        pilot_aboard: bool,
    },
    /// A hit for `fraction` of the victim's full hit points.
    Damage {
        shooter: Option<Flown>,
        victim: Victim,
        fraction: f64,
    },
    /// A human's plane was lost, by any cause: destroyed, its pilot dead or
    /// ejected.
    Loss { plane: PlaneId, seat: SeatId },
}

/// A tick's facts, in the order they happened.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Facts {
    /// The tick they belong to.
    pub tick: u64,
    pub facts: Vec<Fact>,
}
