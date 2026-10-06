//! The court part of the session's state (stage K slice K1;
//! docs/ARCHITECTURE.md, "What moves with the host"): the slot locks, the
//! sides by callsign, a crowned server's own mission and when it was left
//! empty. Every field of [`Court`] is named, so a field added to it fails to
//! compile here until it is coded or skipped with its class.

use super::super::state::{
    Clock, Result, load_option, load_side, load_spec, load_time, save_option, save_side, save_spec,
    save_time,
};
use super::Court;
use crate::wire::messages::Lock;
use std::collections::BTreeMap;
use tore_sim::checkpoint::{Checkpoint, Loader, Saver, invalid};

fn save_lock(s: &mut Saver, lock: &Lock) -> Result<()> {
    match lock {
        Lock::Open => s.writer().write_varint(0),
        Lock::Closed => s.writer().write_varint(1),
        Lock::Reserved(callsign) => {
            s.writer().write_varint(2);
            callsign.save(s, None)?;
        }
    }
    Ok(())
}

fn load_lock(l: &mut Loader<'_>) -> Result<Lock> {
    Ok(match l.reader().read_varint()? {
        0 => Lock::Open,
        1 => Lock::Closed,
        2 => Lock::Reserved(String::load(l, None)?),
        other => return invalid(format!("a slot lock has no variant {other}")),
    })
}

pub(in crate::host) fn save_court(s: &mut Saver, court: &Court) -> Result<()> {
    let Court {
        locks,
        sides,
        file_spec,
        vacant_since,
    } = court;
    s.count(locks.len());
    for (plane, lock) in locks {
        plane.save(s, None)?;
        save_lock(s, lock)?;
    }
    s.count(sides.len());
    for (callsign, side) in sides {
        callsign.save(s, None)?;
        save_side(s, *side);
    }
    save_option(s, file_spec.as_ref(), save_spec)?;
    save_option(s, *vacant_since, save_time)
}

/// The court [`save_court`] coded, its moment moved by `clock`.
pub(in crate::host) fn load_court(l: &mut Loader<'_>, clock: Clock) -> Result<Court> {
    let mut locks = BTreeMap::new();
    for _ in 0..l.count()? {
        let plane = u32::load(l, None)?;
        let lock = load_lock(l)?;
        if locks.insert(plane, lock).is_some() {
            return invalid("a slot locked twice");
        }
    }
    let mut sides = BTreeMap::new();
    for _ in 0..l.count()? {
        let callsign = String::load(l, None)?;
        if sides.insert(callsign, load_side(l)?).is_some() {
            return invalid("a callsign's side twice");
        }
    }
    Ok(Court {
        locks,
        sides,
        file_spec: load_option(l, load_spec)?,
        vacant_since: load_option(l, |l| load_time(l, clock))?,
    })
}

#[cfg(test)]
mod tests {
    use super::super::super::state::{from_bytes, to_bytes};
    use super::*;
    use std::time::Duration;
    use tore_formats::aircraft::AircraftId;
    use tore_sim::ai::launch::Side;
    use tore_world::mission::MissionSpec;

    /// A court with every field filled codes, restores on a clock an hour
    /// on, and codes again to the same bytes from that clock.
    #[test]
    fn a_full_court_restores_on_another_clock() {
        let court = Court {
            locks: BTreeMap::from([
                (1, Lock::Closed),
                (3, Lock::Reserved("Viper".into())),
                (4, Lock::Open),
            ]),
            sides: BTreeMap::from([
                ("Cobra".to_owned(), Side::Enemy),
                ("Viper".to_owned(), Side::Friendly),
            ]),
            file_spec: Some(MissionSpec::new(
                tore_world::test_support::resources::THEATER,
                AircraftId::F18,
            )),
            vacant_since: Some(Duration::from_secs(40)),
        };
        let bytes = to_bytes(|s| save_court(s, &court)).unwrap();
        let clock = Clock {
            old: Duration::from_secs(50),
            new: Duration::from_secs(3_650),
        };
        let restored = from_bytes(&bytes, |l| load_court(l, clock)).unwrap();
        assert_eq!(restored.locks, court.locks);
        assert_eq!(restored.sides, court.sides);
        assert_eq!(restored.file_spec, court.file_spec);
        assert_eq!(restored.vacant_since, Some(Duration::from_secs(3_640)));
        // Damaged bytes are refused, never a panic.
        for cut in 0..bytes.len() {
            let _ = from_bytes(&bytes[..cut], |l| load_court(l, clock));
        }
    }
}
