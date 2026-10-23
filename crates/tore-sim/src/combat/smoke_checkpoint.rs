//! The coders of smoke and contrail puffs and their outlets
//! (docs/formats/checkpoint.md, stage H slice H3b).
//!
//! Presentation, but the next tick reads it back: every puff ages and moves,
//! the contrail outlets decide whether the next puff is laid, and the tick
//! count sets the release cadence. At worst a full mission holds thousands of
//! contrail puffs, so each puff is coded against the one before it, as the
//! design asks: neighbours in the list share most of their high bits.

use super::{Kind, MAX_CONTRAIL_PUFFS, Puff, Smoke};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};
use std::collections::VecDeque;

crate::checkpoint_enum!(Kind {
    Missile = 0,
    Aircraft = 1,
    Contrail = 2,
    Burning = 3,
});

crate::checkpoint_struct!(Puff {
    position,
    kind,
    age,
    drift,
});

impl Checkpoint for Smoke {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        let Smoke {
            puffs,
            wind,
            ticks,
            outlets,
        } = self;
        s.count(puffs.len());
        let mut previous: Option<&Puff> = None;
        for puff in puffs {
            puff.save(s, previous)?;
            previous = Some(puff);
        }
        wind.save(s, base.map(|b| &b.wind))?;
        ticks.save(s, base.map(|b| &b.ticks))?;
        outlets.save(s, base.map(|b| &b.outlets))
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let count = l.count()?;
        // The most any step lets the list grow to; more would never be
        // trimmed again.
        if count > MAX_CONTRAIL_PUFFS {
            return invalid(format!(
                "{count} smoke puffs, more than the {MAX_CONTRAIL_PUFFS} a mission can hold"
            ));
        }
        let mut puffs = VecDeque::with_capacity(count);
        for _ in 0..count {
            let puff = Puff::load(l, puffs.back())?;
            puffs.push_back(puff);
        }
        Ok(Smoke {
            puffs,
            wind: Checkpoint::load(l, base.map(|b| &b.wind))?,
            ticks: Checkpoint::load(l, base.map(|b| &b.ticks))?,
            outlets: Checkpoint::load(l, base.map(|b| &b.outlets))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Coded, Models, from_bytes, round_trip, to_bytes};

    /// A smoke list with every kind of puff, a wind, contrail outlets that
    /// move and a column over a burning site.
    fn busy() -> Smoke {
        Smoke {
            wind: [12., 0., -7.5],
            ..Smoke::default()
        }
    }

    /// One tick of the scripted sources: two missiles, an aircraft, three
    /// moving contrail outlets (one leaves at tick 700) and a burning site.
    fn tick(smoke: &mut Smoke, n: u64) {
        let t = n as f64;
        smoke.step([
            ([100. + t, 5000., 40. * t], Kind::Missile),
            ([-100. + t * 0.5, 7000., 30. * t], Kind::Missile),
            ([0., 3000. - t * 0.1, 200. * t], Kind::Aircraft),
            ([1000., 520., 2000.], Kind::Burning),
        ]);
        let mut outlets = vec![
            (3, [0., 31_000., 250. * t]),
            (4, [20., 31_000., 250. * t + 3.]),
        ];
        if n < 700 {
            outlets.push((9, [-60., 33_000., 300. * t]));
        }
        smoke.contrails(outlets);
    }

    #[test]
    fn a_restored_copy_evolves_identically_for_600_ticks() {
        let models = Models::default();
        let mut original = busy();
        for n in 0..1500 {
            tick(&mut original, n);
        }
        let kinds: std::collections::BTreeSet<_> =
            original.puffs.iter().map(|p| p.kind as u8).collect();
        assert_eq!(kinds.len(), 4, "every kind of puff is in the list");
        assert!(original.puffs.len() > 200);
        let mut copy = round_trip(&original, &models).unwrap();
        assert_eq!(copy, original);
        let before = to_bytes(&original, &models).unwrap();
        for n in 1500..2100 {
            tick(&mut original, n);
            tick(&mut copy, n);
            assert_eq!(copy, original, "tick {n}");
            if n % 30 == 0 {
                assert_eq!(
                    to_bytes(&copy, &models).unwrap(),
                    to_bytes(&original, &models).unwrap(),
                    "coding at tick {n}"
                );
            }
        }
        assert_ne!(to_bytes(&original, &models).unwrap(), before);
        assert_eq!(
            to_bytes(&copy, &models).unwrap(),
            to_bytes(&original, &models).unwrap()
        );
    }

    #[test]
    fn coding_each_puff_against_the_one_before_is_smaller_than_against_zero() {
        let models = Models::default();
        let mut smoke = busy();
        for n in 0..3000 {
            tick(&mut smoke, n);
        }
        let chained = to_bytes(&smoke, &models).unwrap().body.len();
        let mut alone = Saver::with_models(models.clone());
        for puff in &smoke.puffs {
            puff.save(&mut alone, None).unwrap();
        }
        let separate = alone.finish_section().len();
        println!(
            "{} puffs: {chained} bytes chained, {separate} bytes each against zero",
            smoke.puffs.len()
        );
        assert!(chained < separate);
    }

    #[test]
    fn a_list_past_the_mission_cap_is_refused() {
        let models = Models::default();
        let mut s = Saver::with_models(models.clone());
        s.count(MAX_CONTRAIL_PUFFS + 1);
        // Enough bits left for the count to pass its first check.
        for _ in 0..=MAX_CONTRAIL_PUFFS {
            s.writer().write_bool(false);
        }
        let coded = Coded {
            body: s.finish_section(),
            records: Vec::new(),
        };
        assert!(from_bytes::<Smoke>(&coded, &models).is_err());
    }

    #[test]
    fn damaged_bytes_never_panic() {
        let models = Models::default();
        let mut smoke = busy();
        for n in 0..400 {
            tick(&mut smoke, n);
        }
        let coded = to_bytes(&smoke, &models).unwrap();
        for cut in 0..coded.body.len() {
            let cut_coded = Coded {
                body: coded.body[..cut].to_vec(),
                records: Vec::new(),
            };
            let _ = from_bytes::<Smoke>(&cut_coded, &models);
        }
        let mut seed = 0x2545_f491_4f6c_dd1d_u64;
        for _ in 0..2000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut body = coded.body.clone();
            let at = (seed as usize >> 8) % body.len();
            body[at] ^= 1 << (seed & 7);
            let _ = from_bytes::<Smoke>(
                &Coded {
                    body,
                    records: Vec::new(),
                },
                &models,
            );
        }
    }
}
