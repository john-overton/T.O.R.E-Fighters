//! The plain owned types of the picture: what each flight shares, as data.
//! Nothing here borrows from combat or the AI, so the exact checkpoints of
//! stage H can add encoders for these types without touching the rules.
//!
//! The picture's rates and caps are listed in `docs/DATALINK.md`, "Numbers".

use tore_sim::{ai::launch::WingId, ai::wing::PlayerOrder, sensors::Channel};

/// The picture is published at ticks divisible by this: four times a second.
pub const PUBLISH_TICKS: u64 = 30;
/// Tracks one flight's picture keeps, the ones nearest its lead.
pub const FLIGHT_TRACKS: usize = 32;
/// Tracks one seat's share of the picture lists, the ones nearest its plane.
pub const SEAT_TRACKS: usize = 24;

/// One flight, as its wing: the side and the wing's number from zero.
pub type FlightId = WingId;

/// An order for flights: friendly before enemy, then by wing number. A
/// `WingId` has no ordering of its own.
pub fn flight_key(flight: FlightId) -> (bool, u8) {
    (flight.side.is_enemy(), flight.index)
}

/// How a hostile aircraft was sensed.
///
/// Mirrors the sensors' channels, with the AI's observations mapped on: an AI
/// fixture observation counts as visual.
pub type Source = Channel;

/// One hostile aircraft a member holds, with where it was and how it moved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Track {
    /// The member that holds it. When two members hold the same aircraft the
    /// freshest report is kept, the lower plane id on a tie.
    pub reporter: u32,
    /// The aircraft held (also its combat target id).
    pub target: u32,
    /// World position, feet.
    pub position: [f64; 3],
    /// Ground-relative velocity, feet per second.
    pub velocity: [f64; 3],
    pub channel: Source,
    /// The combat tick the reporter observed it.
    pub observed: u64,
}

/// A fuel call level, as the radio judges it
/// ([fuel calls](../../../../docs/spec/radio-chatter.md#fuel-calls)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Fuel {
    #[default]
    Normal,
    Joker,
    Bingo,
    Fumes,
    Out,
}

/// What a member can still shoot at aircraft.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Weapons {
    /// An air-to-air missile left.
    #[default]
    Missiles,
    /// No missile, but gun rounds.
    GunsOnly,
    /// Nothing left to shoot at aircraft.
    Winchester,
}

/// How hurt a member is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Damage {
    #[default]
    None,
    /// Hit points hurt but above half.
    Light,
    /// Half the hit points or less.
    Heavy,
}

/// One member's coarse state, as a pilot would report it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemberStatus {
    pub plane: u32,
    pub fuel: Fuel,
    pub weapons: Weapons,
    pub damage: Damage,
}

/// What one flight's members share, as last published.
#[derive(Clone, Debug, PartialEq)]
pub struct FlightPicture {
    pub flight: FlightId,
    /// The combat tick it was published.
    pub tick: u64,
    /// The flight's tracks, nearest its lead first.
    pub tracks: Vec<Track>,
    /// Each reporting member's state, in plane id order.
    pub status: Vec<MemberStatus>,
}

/// A radar lock a member holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lock {
    pub target: u32,
    /// The combat tick it was first seen held on this target.
    pub since: u64,
}

/// How an assignment reaches its receiver.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Delivery {
    /// By the link: the target itself.
    Link,
    /// By voice: the words only. Slice G3b adds the heard point.
    Voice,
}

/// What the lead gave one member to attack. Written by the assignment slices
/// (G3a onward); stage G0 only holds the table, empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Assignment {
    pub target: u32,
    /// The member that gave it.
    pub by: u32,
    /// The combat tick it was given.
    pub tick: u64,
    pub delivery: Delivery,
    /// The order that made it.
    pub order: PlayerOrder,
    /// The receiver has held a lock on the target since.
    pub acknowledged: bool,
}

/// Who a member is attacking: an AI's chosen target, or a human's locked one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Engagement {
    pub plane: u32,
    pub target: u32,
}
