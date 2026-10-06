//! The rejoin part of the session's state (stage K slice K5;
//! docs/ARCHITECTURE.md, "What moves with the host"): every player's token
//! record and the reservation table (F2-A's away planes and the dropped
//! players'), both by join order, so the new host knows every token and
//! keeps every aircraft for its player. Every field of [`Rejoin`] and of its
//! records is named, so a field added to any of them fails to compile here
//! until it is coded or skipped with its class.
//!
//! Moments are on the session's clock (`Rejoin::offset` moves the host's
//! clock onto it), so they do not change as time passes and the part's
//! bytes change only when a token or a reservation does. A host that takes
//! over sets its own offset from the old one's ([`Rejoin::carry_clock`]) so
//! a token's 24 hours do not start again.

use super::{Record, Rejoin, Reserved};
use std::time::Duration;
use tore_net::Token;
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver};

type Result<T> = std::result::Result<T, CheckpointError>;

fn save_token(s: &mut Saver, token: Token) -> Result<()> {
    let Token(bits) = token;
    (bits as u64).save(s, None)?;
    ((bits >> 64) as u64).save(s, None)
}

fn load_token(l: &mut Loader<'_>) -> Result<Token> {
    let low = u64::load(l, None)?;
    let high = u64::load(l, None)?;
    Ok(Token(u128::from(low) | u128::from(high) << 64))
}

/// A moment as whole nanoseconds, which a session of 580 years does not
/// pass.
fn save_moment(s: &mut Saver, moment: Duration) -> Result<()> {
    u64::try_from(moment.as_nanos())
        .map_err(|_| {
            CheckpointError::Invalid("a moment of the session's clock is out of range".into())
        })?
        .save(s, None)
}

fn load_moment(l: &mut Loader<'_>) -> Result<Duration> {
    Ok(Duration::from_nanos(u64::load(l, None)?))
}

fn save_record(s: &mut Saver, record: &Record) -> Result<()> {
    let Record {
        token,
        callsign,
        lobby_id,
        slot,
        loadout,
        left,
        voided,
    } = record;
    save_token(s, *token)?;
    callsign.save(s, None)?;
    lobby_id.save(s, None)?;
    slot.save(s, None)?;
    loadout.save(s, None)?;
    s.writer().write_bool(left.is_some());
    if let Some(left) = left {
        save_moment(s, *left)?;
    }
    voided.save(s, None)
}

fn load_record(l: &mut Loader<'_>) -> Result<Record> {
    let token = load_token(l)?;
    let callsign = Checkpoint::load(l, None)?;
    let lobby_id = Checkpoint::load(l, None)?;
    let slot = Checkpoint::load(l, None)?;
    let loadout = Checkpoint::load(l, None)?;
    let left = if l.reader().read_bool()? {
        Some(load_moment(l)?)
    } else {
        None
    };
    Ok(Record {
        token,
        callsign,
        lobby_id,
        slot,
        loadout,
        left,
        voided: Checkpoint::load(l, None)?,
    })
}

tore_sim::checkpoint_struct!(Reserved {
    plane,
    lobby_id,
    callsign,
    returning,
    away,
});

/// The most records and reservations a part may hold: the host's own cap
/// and the planes a mission may have.
const MAX_RECORDS: usize = 1_024;
const MAX_RESERVED: usize = 64;

pub(in crate::host) fn save_rejoin(s: &mut Saver, rejoin: &Rejoin) -> Result<()> {
    let Rejoin {
        players,
        reserved,
        offset,
        // The host's own draws, not the session's: a new host draws its own.
        source: _,
    } = rejoin;
    save_moment(s, *offset)?;
    s.count(players.len());
    for (order, record) in players {
        order.save(s, None)?;
        save_record(s, record)?;
    }
    reserved.save(s, None)
}

/// Replaces `rejoin`'s tokens, reservations and clock offset with the part's
/// (its draws stay the host's own).
#[cfg_attr(not(test), allow(dead_code))] // slice K4's takeover restores the part
pub(in crate::host) fn load_rejoin(l: &mut Loader<'_>, rejoin: &mut Rejoin) -> Result<()> {
    let offset = load_moment(l)?;
    let count = l.count()?;
    if count > MAX_RECORDS {
        return tore_sim::checkpoint::invalid("the rejoin part holds too many players");
    }
    let mut players = std::collections::BTreeMap::new();
    for _ in 0..count {
        let order = u64::load(l, None)?;
        players.insert(order, load_record(l)?);
    }
    let reserved: std::collections::BTreeMap<u64, Reserved> = Checkpoint::load(l, None)?;
    if reserved.len() > MAX_RESERVED {
        return tore_sim::checkpoint::invalid("the rejoin part holds too many reservations");
    }
    rejoin.players = players;
    rejoin.reserved = reserved;
    rejoin.offset = offset;
    Ok(())
}

#[cfg_attr(not(test), allow(dead_code))] // slice K4's takeover calls them
impl Rejoin {
    /// The session's clock now, for a host whose own clock reads `now`.
    pub(in crate::host) fn session_clock(&self, now: Duration) -> Duration {
        now + self.offset
    }

    /// Carries the old host's session clock over: the clock read
    /// `session_clock` when this host's own read `now`.
    pub(in crate::host) fn carry_clock(&mut self, session_clock: Duration, now: Duration) {
        self.offset = session_clock.saturating_sub(now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use tore_net::Entropy;
    use tore_sim::checkpoint::{Models, from_bytes, to_bytes};
    use tore_world::mission::{LoadoutSpec, StationLoad};
    use tore_world::seats::PlaneId;

    struct Part<'a>(&'a Rejoin);

    impl Checkpoint for Part<'_> {
        fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<()> {
            save_rejoin(s, self.0)
        }
        fn load(_: &mut Loader<'_>, _: Option<&Self>) -> Result<Self> {
            unreachable!("loaded by load_rejoin")
        }
    }

    struct Loaded(Rejoin);

    impl Checkpoint for Loaded {
        fn save(&self, _: &mut Saver, _: Option<&Self>) -> Result<()> {
            unreachable!("saved by save_rejoin")
        }
        fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self> {
            let mut rejoin = Rejoin::new(Entropy::Seeded(77));
            load_rejoin(l, &mut rejoin)?;
            Ok(Self(rejoin))
        }
    }

    fn rejoin() -> Rejoin {
        let mut rejoin = Rejoin::new(Entropy::Seeded(5));
        rejoin.offset = Duration::from_secs(3_600);
        let a = rejoin.issue(3, "Viper", 1);
        let b = rejoin.issue(9, "Cobra", 4);
        assert_ne!(a, b);
        let viper = rejoin.players.get_mut(&3).unwrap();
        viper.slot = Some(PlaneId(2));
        viper.loadout = Some(LoadoutSpec {
            fuel_lbs: 9_000.5,
            cheat: false,
            stations: vec![StationLoad {
                weapon: "AIM9X.JT".into(),
                count: 2,
                quantity: 2,
            }],
        });
        viper.left = Some(Duration::from_millis(7_123_456));
        let cobra = rejoin.players.get_mut(&9).unwrap();
        cobra.voided = true;
        rejoin.reserved = BTreeMap::from([
            (
                3,
                Reserved {
                    plane: PlaneId(2),
                    lobby_id: 1,
                    callsign: "Viper".into(),
                    returning: false,
                    away: false,
                },
            ),
            (
                11,
                Reserved {
                    plane: PlaneId(5),
                    lobby_id: 6,
                    callsign: "Hawk".into(),
                    returning: true,
                    away: true,
                },
            ),
        ]);
        rejoin
    }

    #[test]
    fn the_part_round_trips_tokens_reservations_and_the_clock() {
        let models = Models::default();
        let before = rejoin();
        let coded = to_bytes(&Part(&before), &models).unwrap();
        let Loaded(after) = from_bytes::<Loaded>(&coded, &models).unwrap();
        assert_eq!(after.players, before.players);
        assert_eq!(after.reserved, before.reserved);
        assert_eq!(after.offset, before.offset);
        // The same part codes to the same bytes again.
        let again = to_bytes(&Part(&after), &models).unwrap();
        assert_eq!(again.body, coded.body);
    }

    #[test]
    fn damaged_part_bytes_are_refused_without_a_panic() {
        let models = Models::default();
        let coded = to_bytes(&Part(&rejoin()), &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut shorter = coded.clone();
            shorter.body.truncate(cut);
            assert!(
                from_bytes::<Loaded>(&shorter, &models).is_err(),
                "cut {cut}"
            );
        }
    }

    #[test]
    fn a_host_that_takes_over_carries_the_session_clock_on() {
        let mut old = rejoin();
        let left = Duration::from_secs(1_000) + old.offset;
        old.players.get_mut(&3).unwrap().left = Some(left);
        // The old host's own clock read 2,000 s; the new host's reads 50 s.
        let session = old.session_clock(Duration::from_secs(2_000));
        let mut new = Rejoin::new(Entropy::Seeded(1));
        new.carry_clock(session, Duration::from_secs(50));
        assert_eq!(
            new.session_clock(Duration::from_secs(50)),
            old.session_clock(Duration::from_secs(2_000))
        );
        // A token's life is counted from the same moment on either host.
        let after = new.session_clock(Duration::from_secs(60));
        assert_eq!(
            after - left,
            Duration::from_secs(2_000 + 3_600 + 10 - 1_000 - 3_600)
        );
    }
}
