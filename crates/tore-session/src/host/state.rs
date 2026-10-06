//! The session's state parts (stage K; docs/ARCHITECTURE.md, "What moves
//! with the host"): players, session, court, scores, revivals, rejoin,
//! candidates and listing, each coded whole in a child module beside the
//! state it codes and sent in a State record after the tick in which it
//! changed ([`crate::journal::Part`]). Slice K1 codes the first five:
//!
//! - **players** (`lobby_state.rs`): each player by join order;
//! - **session** (here): the session's id and clock, the settings, the
//!   missions and their number, the phase and its timers, the join counter;
//! - **court** (`king_state.rs`), **scores** (`score_state.rs`) and
//!   **revivals** (`revive_state.rs`).
//!
//! Slice K5 codes the rejoin part (tokens and the reservation table, F2-A's
//! away planes included); slices K6 (candidates) and K8
//! (listing) code theirs. The coders use the checkpoint trait and destructure
//! every field, so a field added to the state a part codes fails to compile
//! until it is coded or skipped with its class, as stage H's checkpoint
//! coders do. A part's bytes are this build's own, as a checkpoint's are.
//!
//! *Agent decisions (K1):*
//!
//! - Connection ids belong to one host: a part codes each as the player's
//!   join order ([`super::lobby::Entry::order`]), and a restore asks its
//!   caller for the connection that stands for each order.
//! - Times are the host's clock, which is its caller's: a part codes a
//!   moment as it is on that clock, so its bytes do not change as time
//!   passes, and the session part ends with the clock's reading. A restore
//!   moves each moment onto the restoring host's clock ([`Clock`]).
//! - The flying clock (when tick 0 was due and the ticks run) is not in a
//!   part: the journal's tick is the mission's time, and the takeover keeps
//!   the old host's pace (slice K4).
//! - A part is sent when its coding changed (its FNV-1a 64 differs from the
//!   one last queued, `journal.rs`; the session's clock reading is left out
//!   of its hash), so no change can be missed by a call site that forgot to
//!   say so; the cost is a few KB coded a tick while a standby listens.

use super::{ConnectionId, Host, HostConfig, Life};
use crate::journal::Part;
use crate::settings::{Store, number};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;
use tore_codec::CodecError;
use tore_sim::ai::launch::Side;
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Models, Saver, invalid};
use tore_world::mission::MissionSpec;
use tore_world::seats::SeatId;

/// The parts slice K1 codes, in the order they go into the stream.
pub(super) const JOURNALED: [Part; 6] = [
    Part::Players,
    Part::Session,
    Part::Court,
    Part::Scores,
    Part::Revivals,
    Part::Rejoin,
];

pub(super) type Result<T> = std::result::Result<T, CheckpointError>;

/// What a coder needs beside its state: each connection's join order.
pub(super) struct Saving<'a> {
    orders: &'a dyn Fn(ConnectionId) -> Option<u64>,
}

impl<'a> Saving<'a> {
    pub fn new(orders: &'a dyn Fn(ConnectionId) -> Option<u64>) -> Self {
        Self { orders }
    }

    /// The join order of `connection`. A connection the host has forgotten
    /// entirely holds nothing a part needs: its entry is left out.
    pub fn order(&self, connection: ConnectionId) -> Option<u64> {
        (self.orders)(connection)
    }
}

/// The coding host's clock reading (`old`, as the session part gives it)
/// and the restoring host's (`new`): a moment moves from one to the other
/// as far before or after the reading as it was.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::host) struct Clock {
    pub old: Duration,
    pub new: Duration,
}

impl Clock {
    /// The moment `at` of the old clock on the new one.
    pub fn moved(self, at: Duration) -> Duration {
        if at >= self.old {
            self.new + (at - self.old)
        } else {
            self.new.saturating_sub(self.old - at)
        }
    }
}

/// What a restore needs: the connection that stands for each join order (an absent player's placeholder until it resumes, slice
/// K4).
pub(super) struct Restoring<'a> {
    ids: &'a mut dyn FnMut(u64) -> ConnectionId,
    /// Every order handed out, for the journal's list.
    handed: Vec<(ConnectionId, u64)>,
}

impl<'a> Restoring<'a> {
    pub fn new(ids: &'a mut dyn FnMut(u64) -> ConnectionId) -> Self {
        Self {
            ids,
            handed: Vec::new(),
        }
    }

    pub fn connection(&mut self, order: u64) -> ConnectionId {
        let id = (self.ids)(order);
        self.handed.push((id, order));
        id
    }
}

// ----- Coding helpers for types other crates own -------------------------

/// A side as one bit, every variant named.
pub(super) fn save_side(s: &mut Saver, side: Side) {
    s.writer().write_bool(match side {
        Side::Friendly => false,
        Side::Enemy => true,
    });
}

pub(super) fn load_side(l: &mut Loader<'_>) -> Result<Side> {
    Ok(if l.reader().read_bool()? {
        Side::Enemy
    } else {
        Side::Friendly
    })
}

pub(super) fn save_option_side(s: &mut Saver, side: Option<Side>) {
    s.writer().write_bool(side.is_some());
    if let Some(side) = side {
        save_side(s, side);
    }
}

pub(super) fn load_option_side(l: &mut Loader<'_>) -> Result<Option<Side>> {
    Ok(if l.reader().read_bool()? {
        Some(load_side(l)?)
    } else {
        None
    })
}

/// A duration as whole nanoseconds.
pub(super) fn save_duration(s: &mut Saver, duration: Duration) -> Result<()> {
    u64::try_from(duration.as_nanos())
        .map_err(|_| CheckpointError::Invalid("a duration past 584 years".into()))?
        .save(s, None)
}

pub(super) fn load_duration(l: &mut Loader<'_>) -> Result<Duration> {
    Ok(Duration::from_nanos(u64::load(l, None)?))
}

/// A moment of the host's clock.
pub(super) fn save_time(s: &mut Saver, at: Duration) -> Result<()> {
    save_duration(s, at)
}

/// A moment [`save_time`] coded, moved onto the restoring host's clock.
pub(super) fn load_time(l: &mut Loader<'_>, clock: Clock) -> Result<Duration> {
    Ok(clock.moved(load_duration(l)?))
}

pub(super) fn save_option<T>(
    s: &mut Saver,
    value: Option<T>,
    save: impl FnOnce(&mut Saver, T) -> Result<()>,
) -> Result<()> {
    s.writer().write_bool(value.is_some());
    match value {
        Some(value) => save(s, value),
        None => Ok(()),
    }
}

pub(super) fn load_option<T>(
    l: &mut Loader<'_>,
    load: impl FnOnce(&mut Loader<'_>) -> Result<T>,
) -> Result<Option<T>> {
    if l.reader().read_bool()? {
        Ok(Some(load(l)?))
    } else {
        Ok(None)
    }
}

/// A mission as its text, which reads back as the same mission.
pub(super) fn save_spec(s: &mut Saver, spec: &MissionSpec) -> Result<()> {
    spec.to_text().save(s, None)
}

pub(super) fn load_spec(l: &mut Loader<'_>) -> Result<MissionSpec> {
    let text = String::load(l, None)?;
    MissionSpec::from_text(&text).map_err(|error| CheckpointError::Invalid(error.to_string()))
}

/// An address: the family (one bit, 0 for IPv4), its bytes and the port.
pub(super) fn save_address(s: &mut Saver, address: SocketAddr) -> Result<()> {
    let w = s.writer();
    match address.ip() {
        IpAddr::V4(ip) => {
            w.write_bool(false);
            w.write_bytes(&ip.octets());
        }
        IpAddr::V6(ip) => {
            w.write_bool(true);
            w.write_bytes(&ip.octets());
        }
    }
    address.port().save(s, None)
}

pub(super) fn load_address(l: &mut Loader<'_>) -> Result<SocketAddr> {
    let r = l.reader();
    let ip = if r.read_bool()? {
        let bytes: [u8; 16] = r
            .read_bytes(16)?
            .try_into()
            .map_err(|_| CodecError::ValueOutOfRange)?;
        IpAddr::from(bytes)
    } else {
        let bytes: [u8; 4] = r
            .read_bytes(4)?
            .try_into()
            .map_err(|_| CodecError::ValueOutOfRange)?;
        IpAddr::from(bytes)
    };
    Ok(SocketAddr::new(ip, u16::load(l, None)?))
}

/// A part's bytes: its coder's bits, padded to a byte, with no shared
/// records.
pub(super) fn to_bytes(save: impl FnOnce(&mut Saver) -> Result<()>) -> Result<Vec<u8>> {
    let mut s = Saver::new();
    save(&mut s)?;
    let bytes = s.finish_section();
    if !s.into_records().is_empty() {
        return invalid("a state part with shared records");
    }
    Ok(bytes)
}

/// Reads a part's bytes with `load`, which must read every bit.
pub(super) fn from_bytes<T>(
    bytes: &[u8],
    load: impl FnOnce(&mut Loader<'_>) -> Result<T>,
) -> Result<T> {
    let models = Models::default();
    let mut l = Loader::new(bytes, &[], &models);
    let value = load(&mut l)?;
    l.finish()?;
    Ok(value)
}

// ----- The session part ----------------------------------------------------

/// The lifecycle and its timers.
fn save_life(s: &mut Saver, life: Life) -> Result<()> {
    match life {
        Life::Lobby => s.writer().write_varint(0),
        Life::Flying => s.writer().write_varint(1),
        Life::Ended { next_at, stop_at } => {
            s.writer().write_varint(2);
            save_option(s, next_at, save_time)?;
            save_time(s, stop_at)?;
        }
        Life::Stopped => s.writer().write_varint(3),
    }
    Ok(())
}

fn load_life(l: &mut Loader<'_>, clock: Clock) -> Result<Life> {
    Ok(match l.reader().read_varint()? {
        0 => Life::Lobby,
        1 => Life::Flying,
        2 => Life::Ended {
            next_at: load_option(l, |l| load_time(l, clock))?,
            stop_at: load_time(l, clock)?,
        },
        3 => Life::Stopped,
        other => return invalid(format!("the lifecycle has no phase {other}")),
    })
}

/// The settings that a host's configuration sets past the King's lists
/// ([`Store::from_config`]), and the password, which has a field of its
/// own: coded beside the list, not in it.
const SETTINGS_BESIDE: [u8; 3] = [number::PASSWORD, number::MAX_PLAYERS, number::TIME_LIMIT];

/// The settings store: every registry value, the name and the password.
fn save_settings(s: &mut Saver, settings: &Store) -> Result<()> {
    settings.lobby_list().save(s, None)?;
    settings.name().to_owned().save(s, None)?;
    settings.password().map(str::to_owned).save(s, None)
}

/// The store [`save_settings`] coded. The store keeps its values private,
/// so it is built as a host's configuration builds one, with the values
/// that may pass the King's lists given as the configuration gives them,
/// and checked against the list after.
fn load_settings(l: &mut Loader<'_>, config: &HostConfig) -> Result<Store> {
    let list = Vec::<(u8, u32)>::load(l, None)?;
    let name = String::load(l, None)?;
    let password = Option::<String>::load(l, None)?;
    let value = |n: u8| {
        list.iter()
            .find(|(number, _)| *number == n)
            .map(|(_, v)| *v)
    };
    let restoring = HostConfig {
        settings: list
            .iter()
            .filter(|(number, _)| !SETTINGS_BESIDE.contains(number))
            .copied()
            .collect(),
        name,
        password,
        max_players: value(number::MAX_PLAYERS).unwrap_or_default() as usize,
        time_limit: value(number::TIME_LIMIT)
            .filter(|seconds| *seconds > 0)
            .map(|seconds| Duration::from_secs(u64::from(seconds))),
        ..config.clone()
    };
    let store = Store::from_config(&restoring);
    if store.lobby_list() != list {
        return invalid("the settings do not restore as they were");
    }
    Ok(store)
}

impl Host {
    /// The connections' join orders: the players' now, then those the
    /// journal remembers from this mission.
    fn orders(&self) -> impl Fn(ConnectionId) -> Option<u64> + '_ {
        move |connection| {
            self.peers
                .get(&connection)
                .map(|peer| peer.lobby.order)
                .or_else(|| self.journal.orders.get(&connection).copied())
        }
    }

    /// The coding of `part` now.
    #[cfg_attr(not(test), allow(dead_code))] // Slice K4 codes the parts.
    pub(super) fn encode_part(&self, part: Part) -> Result<Vec<u8>> {
        Ok(self.encode_part_hashed(part)?.0)
    }

    /// The coding of `part` now, and the hash that tells whether it changed:
    /// of its bytes, the session part's without its clock reading.
    pub(super) fn encode_part_hashed(&self, part: Part) -> Result<(Vec<u8>, u64)> {
        if part == Part::Session {
            let stable = to_bytes(|s| self.save_session(s))?;
            let bytes = to_bytes(|s| {
                self.save_session(s)?;
                save_time(s, self.now)
            })?;
            return Ok((bytes, tore_codec::fnv1a64(&stable)));
        }
        let bytes = self.encode_other(part)?;
        let hash = tore_codec::fnv1a64(&bytes);
        Ok((bytes, hash))
    }

    fn encode_other(&self, part: Part) -> Result<Vec<u8>> {
        let orders = self.orders();
        let saving = Saving::new(&orders);
        match part {
            Part::Players => to_bytes(|s| super::lobby::state::save_players(s, &saving, self)),
            Part::Session => invalid("the session part codes with its clock"),
            Part::Court => to_bytes(|s| super::king::state::save_court(s, &self.court)),
            Part::Scores => to_bytes(|s| super::score::state::save_scores(s, &self.score)),
            Part::Revivals => {
                to_bytes(|s| super::revive::state::save_revivals(s, &saving, &self.revival))
            }
            Part::Rejoin => to_bytes(|s| super::rejoin::save_rejoin(s, &self.rejoin)),
            Part::Candidates | Part::Listing => {
                tore_sim::checkpoint::not_covered("the candidates and listing parts")
            }
        }
    }

    /// The old host's clock reading a session part ends with: what the
    /// times of every part are moved from when they are restored.
    #[cfg_attr(not(test), allow(dead_code))] // Slice K4 restores the parts.
    pub(super) fn session_clock(&self, bytes: &[u8]) -> Result<Duration> {
        let unmoved = Clock {
            old: Duration::ZERO,
            new: Duration::ZERO,
        };
        from_bytes(bytes, |l| load_session(l, unmoved, &self.config)).map(|s| s.clock)
    }

    /// The players a players part holds, in join order, each with the
    /// connection `ids` gives for its order (which the journal remembers):
    /// the records slice K4 holds absent until each player resumes
    /// ([`super::lobby::state::PlayerState::into_peer`], which moves its
    /// times onto this host's clock).
    #[cfg_attr(not(test), allow(dead_code))] // Slice K4 restores the parts.
    pub(super) fn restore_players(
        &mut self,
        bytes: &[u8],
        ids: &mut dyn FnMut(u64) -> ConnectionId,
    ) -> Result<Vec<(ConnectionId, super::lobby::state::PlayerState)>> {
        let players = super::lobby::state::load_players(bytes)?;
        Ok(players
            .into_iter()
            .map(|player| {
                let id = ids(player.lobby.order);
                self.journal.orders.insert(id, player.lobby.order);
                (id, player)
            })
            .collect())
    }

    /// Restores `part` from `bytes` into this host's structures: the
    /// session, the court, the scores and the revivals in place, each
    /// connection the connection `ids` gives for its join order (and the
    /// journal remembers each such order), each moment moved from the old
    /// host's clock reading `old_clock` ([`Self::session_clock`]) onto this
    /// host's. The players part restores with [`Self::restore_players`].
    #[cfg_attr(not(test), allow(dead_code))] // Slice K4 restores the parts.
    pub(super) fn restore_part(
        &mut self,
        part: Part,
        bytes: &[u8],
        ids: &mut dyn FnMut(u64) -> ConnectionId,
        old_clock: Duration,
    ) -> Result<()> {
        let clock = Clock {
            old: old_clock,
            new: self.now,
        };
        let mut restoring = Restoring::new(ids);
        match part {
            Part::Session => {
                let config = self.config.clone();
                let session = from_bytes(bytes, |l| load_session(l, clock, &config))?;
                session.into_host(self);
            }
            Part::Court => {
                self.court = from_bytes(bytes, |l| super::king::state::load_court(l, clock))?;
            }
            Part::Scores => {
                self.score = from_bytes(bytes, super::score::state::load_scores)?;
            }
            Part::Revivals => {
                self.revival = from_bytes(bytes, |l| {
                    super::revive::state::load_revivals(l, &mut restoring)
                })?;
            }
            Part::Rejoin => {
                // Its moments are on the session's clock, the old host's
                // plus the offset the part carries: this host's offset is
                // set so the clock goes on from the old host's reading.
                let rejoin = &mut self.rejoin;
                from_bytes(bytes, |l| super::rejoin::load_rejoin(l, rejoin))?;
                let session_clock = old_clock + rejoin.offset;
                rejoin.carry_clock(session_clock, self.now);
            }
            Part::Players => return invalid("the players part restores on its own"),
            Part::Candidates | Part::Listing => {
                return tore_sim::checkpoint::not_covered("the candidates and listing parts");
            }
        }
        for (id, order) in restoring.handed {
            self.journal.orders.insert(id, order);
        }
        Ok(())
    }

    /// The session part. Every field of the host is named: a field added
    /// to `Host` fails to compile here until it is coded or skipped with its
    /// class.
    fn save_session(&self, s: &mut Saver) -> Result<()> {
        let Host {
            // Setup: the host's own, which a new host builds from its game.
            config: _,
            resources: _,
            compat: _,
            // Rebuilt with the world from the spec.
            manifest: _,
            // The world: the journal and checkpoints carry it.
            world: _,
            // Connection state: the transport's, a new host's own.
            server: _,
            // The other parts: players, scores, revivals, court.
            peers: _,
            score: _,
            revival: _,
            court: _,
            // F2-A's asks and flight counts are per tick; its away table is
            // the rejoin part's (slice K5).
            idle: _,
            // Stage K's own: K3's standbys, K4's resume, K5's tokens and
            // reservations (the rejoin part), K6's candidates (the
            // candidates part), and the journal itself.
            standbys: _,
            resuming: _,
            rejoin: _,
            succession: _,
            journal: _,
            // The flying clock: the journal's tick is the mission's time,
            // and the takeover keeps the old host's pace (slice K4).
            origin: _,
            ticks_run: _,
            // The host's clock: its reading ends the part, outside the
            // change hash (`encode_part_hashed`).
            now: _,
            // Not moved (docs/ARCHITECTURE.md, "What moves with the host"):
            // the event tracker, built from the world; the observers' rings;
            // the tick's output (scratch, filled by each step); the logs and
            // the tick costs; the dirty flags (a new host sends everything).
            tracker: _,
            stream: _,
            out: _,
            logs: _,
            costs: _,
            overloads: _,
            roster_dirty: _,
            lobby_dirty: _,
            // Tests only.
            #[cfg(test)]
                unforeseen_log: _,
            #[cfg(test)]
                test_designations: _,
            #[cfg(test)]
                snapshot_executor: _,
            // Coded.
            session_id,
            settings,
            spec,
            spec_text,
            number,
            life,
            ever_seated,
            empty_since,
            joins,
            next_id,
            gives,
        } = self;
        session_id.save(s, None)?;
        save_settings(s, settings)?;
        save_spec(s, spec)?;
        spec_text.save(s, None)?;
        number.save(s, None)?;
        save_life(s, *life)?;
        ever_seated.save(s, None)?;
        save_option(s, *empty_since, save_time)?;
        joins.save(s, None)?;
        next_id.save(s, None)?;
        gives.save(s, None)
    }
}

/// The session part as read, before it goes into a host.
#[derive(Debug)]
pub(super) struct Session {
    pub session_id: u64,
    pub settings: Store,
    pub spec: MissionSpec,
    pub spec_text: String,
    pub number: u32,
    pub life: Life,
    pub ever_seated: bool,
    pub empty_since: Option<Duration>,
    pub joins: u64,
    pub next_id: u8,
    pub gives: Vec<(SeatId, String)>,
    /// The old host's clock reading when the part was coded.
    pub clock: Duration,
}

fn load_session(l: &mut Loader<'_>, clock: Clock, config: &HostConfig) -> Result<Session> {
    Ok(Session {
        session_id: u64::load(l, None)?,
        settings: load_settings(l, config)?,
        spec: load_spec(l)?,
        spec_text: String::load(l, None)?,
        number: u32::load(l, None)?,
        life: load_life(l, clock)?,
        ever_seated: bool::load(l, None)?,
        empty_since: load_option(l, |l| load_time(l, clock))?,
        joins: u64::load(l, None)?,
        next_id: u8::load(l, None)?,
        gives: Vec::load(l, None)?,
        clock: load_duration(l)?,
    })
}

impl Session {
    /// Puts the session into `host`. The world and the flying clock are the
    /// takeover's (slice K4): this leaves them.
    fn into_host(self, host: &mut Host) {
        let Session {
            session_id,
            clock: _,
            settings,
            spec,
            spec_text,
            number,
            life,
            ever_seated,
            empty_since,
            joins,
            next_id,
            gives,
        } = self;
        host.session_id = session_id;
        host.server.set_reach_session(Some(session_id));
        host.settings = settings;
        host.spec = spec;
        host.spec_text = spec_text;
        host.number = number;
        host.life = life;
        host.ever_seated = ever_seated;
        host.empty_since = empty_since;
        host.joins = joins;
        host.next_id = next_id;
        host.gives = gives;
    }
}
