//! The coders of combat's two small state types that live beside the shared
//! helpers: a player's trigger and a falling store's velocity
//! (docs/formats/checkpoint.md). Both hold exactly what the next tick reads,
//! so every field is coded.

use super::{FallState, PlayerTrigger};

crate::checkpoint_struct!(PlayerTrigger { next_t, was_held });

crate::checkpoint_struct!(FallState { velocity_f8 });

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Models, round_trip};

    #[test]
    fn a_trigger_and_a_fall_round_trip() {
        let models = Models::default();
        for (next_t, was_held) in [(0, false), (65_535, true), (1_234, true)] {
            let trigger = PlayerTrigger { next_t, was_held };
            assert_eq!(round_trip(&trigger, &models).unwrap(), trigger);
        }
        for velocity_f8 in [0, 1, 0x5000, u16::MAX] {
            let fall = FallState { velocity_f8 };
            assert_eq!(round_trip(&fall, &models).unwrap(), fall);
        }
    }
}
