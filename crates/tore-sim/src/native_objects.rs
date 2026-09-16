//! Diagnostic native object-list state; no world construction or live activation.
//! Source: FA 0x42e540 / 0x42e5c0, docs/formats/native-strip.md.

/// Ordered candidate IDs, independently bounded by the two native capacities.
/// Callers own object lifetime and must stage this state with construction/query
/// state. Removing an ID is not a substitute for rolling back a failed operation.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CollisionCandidates {
    primary: Vec<u16>,
    secondary: Vec<u16>,
}

impl CollisionCandidates {
    pub fn primary(&self) -> &[u16] {
        &self.primary
    }

    pub fn secondary(&self) -> &[u16] {
        &self.secondary
    }

    /// Native registration silently skips ineligible, duplicate or full lists.
    /// A duplicate primary ID does not retry insertion into the secondary list.
    pub fn register(&mut self, id: u16, instance_flags: u32, type_flags: u32) {
        if instance_flags & 1 == 0
            || type_flags & 1 == 0
            || self.primary.contains(&id)
            || self.primary.len() >= 900
        {
            return;
        }
        self.primary.push(id);
        if type_flags & 0x408000 != 0 && self.secondary.len() < 450 {
            self.secondary.push(id);
        }
    }

    /// Native removal uses the current type flags, independently of instance
    /// flags. Forward compaction preserves candidate/tie order.
    pub fn unregister(&mut self, id: u16, type_flags: u32) {
        if type_flags & 1 == 0 {
            return;
        }
        remove_first(&mut self.primary, id);
        if type_flags & 0x408000 != 0 {
            remove_first(&mut self.secondary, id);
        }
    }
}

fn remove_first(ids: &mut Vec<u16>, id: u16) {
    if let Some(index) = ids.iter().position(|&candidate| candidate == id) {
        ids.remove(index);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gates_and_stable_removal_preserve_native_order() {
        let mut candidates = CollisionCandidates::default();
        candidates.register(9, 0, 0x8001);
        candidates.register(9, 1, 0x8000);
        assert!(candidates.primary().is_empty());
        for id in [9, 2, 7] {
            candidates.register(id, 1, 0x8001);
        }
        candidates.register(2, 1, 0x400001);
        candidates.unregister(2, 0x8000);
        assert_eq!(candidates.primary(), &[9, 2, 7]);
        candidates.unregister(2, 0x8001);
        assert_eq!(candidates.primary(), &[9, 7]);
        assert_eq!(candidates.secondary(), &[9, 7]);
        candidates.unregister(99, 0x8001);
        candidates.register(2, 1, 0x400001);
        assert_eq!(candidates.secondary(), &[9, 7, 2]);
    }

    #[test]
    fn independent_capacities_and_duplicate_does_not_backfill() {
        let mut candidates = CollisionCandidates::default();
        for id in 1..=901 {
            candidates.register(id, 1, 0x8001);
        }
        assert_eq!(candidates.primary().len(), 900);
        assert_eq!(candidates.secondary().len(), 450);
        assert_eq!(candidates.primary().last(), Some(&900));
        candidates.unregister(1, 0x8001);
        candidates.register(451, 1, 0x8001);
        assert_eq!(candidates.secondary().len(), 449);
        candidates.register(901, 1, 0x8001);
        assert_eq!(candidates.primary().last(), Some(&901));
        assert_eq!(candidates.secondary().last(), Some(&901));
        candidates.register(902, 1, 0x8001);
        assert_eq!(candidates.primary().len(), 900);
    }

    #[test]
    fn changed_type_flags_are_not_automatic_list_reconciliation() {
        let mut candidates = CollisionCandidates::default();
        candidates.register(4, 1, 1);
        candidates.register(4, 1, 0x8001);
        assert!(candidates.secondary().is_empty());
        candidates.unregister(4, 1);
        candidates.register(4, 1, 0x8001);
        candidates.unregister(4, 1);
        assert!(candidates.primary().is_empty());
        assert_eq!(candidates.secondary(), &[4]);
        // Removal still checks the secondary list when primary is absent.
        candidates.unregister(4, 0x8001);
        assert!(candidates.secondary().is_empty());
    }
}
