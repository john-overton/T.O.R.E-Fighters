//! The coders of an AI aircraft's incoming gunfire awareness
//! (docs/formats/checkpoint.md, stage H slice H4).
//!
//! `Round` is the host's per-tick input and `Trace` an explanation built
//! for the debug panels; neither is held between ticks. Every field of
//! `Service` is coded.

use super::{Cue, Evidence, Service};

crate::checkpoint_enum!(Evidence {
    VisualTrajectory = 0,
    ClosePass = 1,
    Hit = 2,
});

crate::checkpoint_struct!(Cue {
    evidence,
    round,
    observed_tick,
    time_to_danger_s,
    bearing_world_deg,
});

crate::checkpoint_struct!(Service {
    samples,
    cue,
    pending_hit,
    episode,
});

/// Coded exactly as the `BTreeMap<u32, Vector>` it replaced: a count, then
/// each id (no baseline) and its position against the baseline's position
/// for the same id. Ids must arrive strictly ascending. The spare list is
/// scratch and is not coded.
impl crate::checkpoint::Checkpoint for super::Samples {
    fn save(
        &self,
        s: &mut crate::checkpoint::Saver,
        base: Option<&Self>,
    ) -> Result<(), crate::checkpoint::CheckpointError> {
        s.count(self.by_id.len());
        for (id, position) in &self.by_id {
            id.save(s, None)?;
            position.save(s, base.and_then(|b| b.get(*id)))?;
        }
        Ok(())
    }
    fn load(
        l: &mut crate::checkpoint::Loader<'_>,
        base: Option<&Self>,
    ) -> Result<Self, crate::checkpoint::CheckpointError> {
        use crate::checkpoint::Checkpoint;
        let count = l.count()?;
        let mut by_id: Vec<(u32, crate::attitude::Vector)> = Vec::with_capacity(count);
        for _ in 0..count {
            let id = u32::load(l, None)?;
            if by_id.last().is_some_and(|(last, _)| *last >= id) {
                return crate::checkpoint::invalid("map keys out of order");
            }
            let position = Checkpoint::load(l, base.and_then(|b| b.get(id)))?;
            by_id.push((id, position));
        }
        Ok(super::Samples::from_sorted(by_id))
    }
}

#[cfg(test)]
mod tests {
    use super::super::Samples;
    use crate::checkpoint::{Checkpoint, Loader, Models, Saver};
    use std::collections::BTreeMap;

    /// The samples code to the same bytes as the map they replaced, with and
    /// without a baseline, and read back the same.
    #[test]
    fn samples_code_as_the_map_did() {
        let map = |pairs: &[(u32, f64)]| -> BTreeMap<u32, [f64; 3]> {
            pairs
                .iter()
                .map(|(id, x)| (*id, [*x, 2. * x, -x]))
                .collect()
        };
        let samples = |map: &BTreeMap<u32, [f64; 3]>| {
            Samples::from_sorted(map.iter().map(|(id, at)| (*id, *at)).collect())
        };
        let cases = [
            map(&[]),
            map(&[(3, 1.5)]),
            map(&[
                (1, 10.),
                (7, -3.25),
                (900, 1e9),
                (u32::MAX, f64::MIN_POSITIVE),
            ]),
            map(&[(1, 10.), (8, -3.25), (900, 1e9 + 1.)]),
        ];
        let models = Models::default();
        for current in &cases {
            for base in std::iter::once(None).chain(cases.iter().map(Some)) {
                let mut s = Saver::new();
                current.save(&mut s, base).unwrap();
                let expected = s.finish_section();
                let mut s = Saver::new();
                let base_samples = base.map(samples);
                samples(current)
                    .save(&mut s, base_samples.as_ref())
                    .unwrap();
                let actual = s.finish_section();
                assert_eq!(actual, expected);
                let mut l = Loader::new(&actual, &[], &models);
                let loaded = Samples::load(&mut l, base_samples.as_ref()).unwrap();
                l.finish().unwrap();
                assert_eq!(loaded, samples(current));
                assert_eq!(format!("{loaded:?}"), format!("{current:?}"));
            }
        }
    }
}
