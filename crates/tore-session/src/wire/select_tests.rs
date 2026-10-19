//! Slice S11: the entity sender's choice of records against the one it
//! replaced (a hash map of the entities, the chosen writers cloned), over
//! random entities, relevances, budgets, deliveries and losses.

use super::*;
use crate::wire::priority::Relevance;
use crate::wire::samples;

struct Random(u64);

impl Random {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: u64) -> u64 {
        self.next() % n
    }
}

fn record_bits(records: &[(EntityKey, BitWriter)]) -> Vec<(EntityKey, usize, Vec<u8>)> {
    records
        .iter()
        .map(|(key, w)| (*key, w.bit_len(), w.as_bytes().to_vec()))
        .collect()
}

#[test]
fn the_sorted_choice_picks_and_codes_what_the_hash_map_choice_did() {
    for seed in 1..=6u64 {
        let mut random = Random(0x9e37_79b9_7f4a_7c15 ^ seed);
        let tps = [1, 2, 4][seed as usize % 3];
        let mut new = EntitySender::new(tps);
        let mut old = EntitySender::new(tps);
        // Many entities: the samples under several ids each.
        let base: Vec<Entity> = samples::entities();
        let mut sequence = 0u16;
        let mut keys = BTreeSet::new();
        let mut seen = EntityReport::default();
        for step in 0..400u32 {
            let tick = step * tps;
            let mut entities: Vec<(Entity, Relevance)> = Vec::new();
            for copy in 0..12u32 {
                for sample in &base {
                    if random.below(10) == 0 {
                        continue;
                    }
                    let mut entity = samples::moved(sample, i64::from(step % 7));
                    entity.id = sample.id + copy * 1_000;
                    let relevance = Relevance {
                        distance_ft: random.below(400_000) as f64,
                        own_flight: random.below(9) == 0,
                        viewed: random.below(40) == 0,
                        ..Relevance::NEAR
                    };
                    entities.push((entity, relevance));
                }
            }
            entities.sort_by_key(|(e, _)| e.key());
            entities.dedup_by_key(|(e, _)| e.key());
            if random.below(5) == 0 {
                // Any order serves.
                entities.reverse();
            }
            let header = samples::header(tick);
            let budget = 40 + random.below(1_400) as usize;
            let a = new.select(&header, &entities, budget).unwrap();
            let b = old.select_scan(&header, &entities, budget).unwrap();
            assert_eq!(
                record_bits(&a.0),
                record_bits(&b.0),
                "seed {seed} step {step}"
            );
            assert_eq!(a.1, b.1, "seed {seed} step {step}");
            seen.sent += a.1.sent;
            seen.full += a.1.full;
            seen.removed += a.1.removed;
            seen.waiting += a.1.waiting;
            seen.not_due += a.1.not_due;
            keys.extend(entities.iter().map(|(e, _)| e.key()));
            match random.below(6) {
                0 => {
                    new.discard();
                    old.discard();
                }
                _ => {
                    new.sent(sequence);
                    old.sent(sequence);
                    // Deliver or lose an earlier packet now and then.
                    let back = sequence.wrapping_sub(random.below(4) as u16);
                    if random.below(3) == 0 {
                        new.lost(back);
                        old.lost(back);
                    } else {
                        new.delivered(back);
                        old.delivered(back);
                    }
                    sequence = sequence.wrapping_add(1);
                }
            }
            assert_eq!(new.removals_pending(), old.removals_pending());
            for key in &keys {
                assert_eq!(new.acknowledged(*key), old.acknowledged(*key));
            }
        }
        // Deltas, removals, waiting and far entities all came up.
        assert!(seen.full < seen.sent, "{seen:?}");
        assert!(
            seen.removed > 0 && seen.waiting > 0 && seen.not_due > 0,
            "{seen:?}"
        );
        // A list with a key twice is refused by both.
        let twice = vec![(base[0], Relevance::NEAR), (base[0], Relevance::NEAR)];
        let header = samples::header(0);
        assert!(new.select(&header, &twice, 1_000).is_err());
        assert!(old.select_scan(&header, &twice, 1_000).is_err());
    }
}
