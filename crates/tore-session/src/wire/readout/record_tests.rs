//! The readout record coded in place (slice R2-host) against the coding it
//! replaced, which cloned the baseline for every part: the same bits, the
//! same readout left with the client and the same report, over random
//! readouts, changes, baseline ages and budgets.

use super::flat::Kind;
use super::{PARTS, Part, QReadout, list, scalar, schema, write_record, write_record_cloning};
use tore_net::SplitMix64;

fn below(rng: &mut SplitMix64, n: u64) -> u64 {
    rng.next_u64() % n.max(1)
}

fn value(rng: &mut SplitMix64, kind: Kind) -> i64 {
    match kind {
        Kind::Slow(super::flat::Slow::Bits(_, max)) => below(rng, max as u64 + 1) as i64,
        _ => {
            // Mostly small, sometimes large, so every ladder rung is used.
            let span = [8, 300, 40_000, 3_000_000][below(rng, 4) as usize];
            below(rng, 2 * span + 1) as i64 - span as i64
        }
    }
}

fn entry(rng: &mut SplitMix64, list: usize) -> Vec<i64> {
    schema(list).iter().map(|kind| value(rng, *kind)).collect()
}

fn random(rng: &mut SplitMix64) -> QReadout {
    let mut q = QReadout::empty();
    for group in 0..scalar::COUNT {
        let len = below(rng, 5) as usize;
        q.scalars[group] = (0..len).map(|_| below(rng, 2_001) as i64 - 1_000).collect();
    }
    for index in 0..list::COUNT {
        for _ in 0..below(rng, 14) {
            let id = below(rng, 60) as u32;
            let values = entry(rng, index);
            q.lists[index].insert(id, values);
        }
    }
    for _ in 0..below(rng, 6) {
        let id = below(rng, 30) as u32;
        let points = (0..below(rng, 12))
            .map(|_| std::array::from_fn(|_| below(rng, 20_001) as i64 - 10_000))
            .collect();
        q.trails.insert(id, points);
    }
    q
}

/// `base` with some of everything changed, removed and added.
fn changed(rng: &mut SplitMix64, base: &QReadout) -> QReadout {
    let mut q = base.clone();
    for group in 0..scalar::COUNT {
        if below(rng, 3) == 0 {
            let len = below(rng, 5) as usize;
            q.scalars[group] = (0..len).map(|_| below(rng, 2_001) as i64 - 1_000).collect();
        }
    }
    for index in 0..list::COUNT {
        let ids: Vec<u32> = q.lists[index].keys().copied().collect();
        for id in ids {
            match below(rng, 5) {
                0 => {
                    q.lists[index].remove(&id);
                }
                1 => {
                    let values = q.lists[index].get_mut(&id).unwrap();
                    let at = below(rng, values.len() as u64) as usize;
                    values[at] = value(rng, schema(index)[at]);
                }
                _ => {}
            }
        }
        for _ in 0..below(rng, 4) {
            let id = below(rng, 60) as u32;
            let values = entry(rng, index);
            q.lists[index].insert(id, values);
        }
    }
    let ids: Vec<u32> = q.trails.keys().copied().collect();
    for id in ids {
        match below(rng, 4) {
            0 => {
                q.trails.remove(&id);
            }
            1 => {
                let points = q.trails.get_mut(&id).unwrap();
                let drop = below(rng, points.len() as u64 + 1) as usize;
                points.drain(..drop);
                for _ in 0..below(rng, 4) {
                    points.push(std::array::from_fn(|_| below(rng, 20_001) as i64 - 10_000));
                }
            }
            _ => {}
        }
    }
    if below(rng, 3) == 0 {
        q.trails.insert(
            below(rng, 30) as u32,
            vec![[1, 2, 3]; below(rng, 5) as usize],
        );
    }
    q
}

#[test]
fn the_record_coded_in_place_is_the_record_the_clones_coded() {
    let mut rng = SplitMix64::new(0x005e_ed2a);
    let mut sent = 0;
    let mut waited = 0;
    for case in 0..6_000 {
        let base = if case % 7 == 0 {
            QReadout::empty()
        } else {
            random(&mut rng)
        };
        let ticks = [0, 1, 2, 4, 6, 60, 254][below(&mut rng, 7) as usize];
        let now = match below(&mut rng, 4) {
            0 => random(&mut rng),
            // What the prediction gets exactly right is not sent at all.
            1 => base.advanced(ticks),
            _ => changed(&mut rng, &base.advanced(ticks)),
        };
        let budget = [0, 30, 120, 400, 1_500, 6_000, 40_000][below(&mut rng, 7) as usize];
        let back = below(&mut rng, 128) as u8;
        let (w, after, report) = write_record(back, &now, &base, ticks, budget);
        let (old_w, old_after, old_report) = write_record_cloning(back, &now, &base, ticks, budget);
        assert_eq!(w.bit_len(), old_w.bit_len(), "case {case}");
        assert_eq!(w.finish(), old_w.finish(), "case {case}");
        assert_eq!(after, old_after, "case {case}");
        assert_eq!(report, old_report, "case {case}");
        sent += report.sent;
        waited += report.waiting;
    }
    assert!(
        sent > 10_000 && waited > 1_000,
        "{sent} sent, {waited} waited"
    );
}

#[test]
fn the_advance_in_place_is_the_advance_by_copy() {
    let mut rng = SplitMix64::new(77);
    for _ in 0..2_000 {
        let q = random(&mut rng);
        let ticks = below(&mut rng, 300) as u32;
        assert_eq!(q.advanced(ticks), q.advanced_by_copy(ticks));
    }
}

#[test]
fn every_part_is_coded_once_and_no_velocity_is_a_position() {
    // A part coded twice would be compared against what the first coding
    // left; each is coded once, so the in-place record sees the baseline.
    let mut seen = std::collections::BTreeSet::new();
    for part in PARTS {
        let key = match part {
            Part::Scalar(group) => (0, group),
            Part::List(index) => (1, index),
            Part::Trails => (2, 0),
        };
        assert!(seen.insert(key), "{part:?} twice");
    }
    for index in 0..list::COUNT {
        let layout = schema(index);
        for kind in layout {
            if let Kind::Pos { vel, .. } = *kind {
                assert!(!matches!(layout[vel], Kind::Pos { .. }), "list {index}");
            }
        }
    }
}
