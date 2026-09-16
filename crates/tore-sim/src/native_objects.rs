//! Diagnostic native object state; no world construction or live activation.
//! Source: docs/formats/native-strip.md.

/// Diagnostic FA 0x411950: move a word angle by at most the supplied step.
/// Difference wraps as a word; magnitude is widened, including -32768.
/// Step normalization retains native dword wrapping, even for i32::MIN.
/// No time, rate, command, or terrain producer is implied by this helper.
pub fn approach_angle(current: i16, target: i16, step: i32) -> i16 {
    let step = step.wrapping_abs();
    let difference = target.wrapping_sub(current);
    if i32::from(difference).abs() <= step {
        target
    } else if difference < 0 {
        current.wrapping_sub(step as i16)
    } else {
        current.wrapping_add(step as i16)
    }
}

/// Diagnostic command deadline at FA 0x463b90, distinct from wrapping service
/// and speech deadlines. Both source operands are widened unsigned words.
/// No command execution, scheduler state or clock producer is connected here.
pub fn command_deadline(clock: u16, delay: u16) -> u16 {
    (u32::from(clock) + u32::from(delay)).min(0x7fff) as u16
}

/// Diagnostic speech delay at FA 0x48d5e0; no clock or event state is changed.
/// The signed scale gate and x86 five-bit shift count apply even to a word shift.
/// The producer of `scale` remains outside this diagnostic helper.
pub fn speech_delay(delay: u16, scale: i16) -> u16 {
    if scale > 0 {
        ((delay as u32) << (scale as u32 & 31)) as u16
    } else {
        delay
    }
}

/// Inputs to the STRIP (kind 0) service tail at FA 0x4630b0.
/// These are post-callback samples, not permission to skip the service body.
/// Priority and reference predicates must come from reviewed world producers.
#[derive(Clone, Copy, Debug)]
pub struct StripServiceTail {
    pub callback_delay: u16,
    pub deadline: u16,
    pub clock: u16,
    pub controller: u8,
    pub priority: bool,
    pub referenced: bool,
    pub speed: i32,
}

/// Explicit draw request; no hidden RNG or scheduler state is changed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StripServiceSchedule {
    At(u16),
    Draw { base: u16, upper_bound: u16 },
}

impl StripServiceTail {
    /// Native word comparisons/addition and signed speed test for kind 0 only.
    /// A callback override takes precedence over every default-delay predicate.
    pub fn schedule(self) -> StripServiceSchedule {
        if self.callback_delay != 0x7fff {
            StripServiceSchedule::At(self.clock.wrapping_add(self.callback_delay))
        } else if self.deadline >= self.clock
            || self.controller & 0x80 != 0
            || self.priority
            || self.referenced
        {
            StripServiceSchedule::At(self.clock)
        } else {
            StripServiceSchedule::Draw {
                base: self.clock.wrapping_add(2),
                upper_bound: if self.speed > 0 { 8 } else { 20 },
            }
        }
    }
}

/// Mission nationality conversion at FA 0x4826c7 and 0x483d50.
/// `raw` is the parsed integer's low byte; `map_prefix` is the first byte
/// of the native map name, without stripping a leading `~` or `$`.
pub fn mission_nationality(raw: u8, map_prefix: u8) -> u8 {
    let low = raw & 0x7f;
    let adjusted = if low >= 8 { low + 1 } else { low } | (raw & 0x80);
    if !matches!(map_prefix, b'T' | b't' | b'U' | b'u' | b'K' | b'k') {
        return adjusted;
    }
    let mapped = match adjusted & 0x7f {
        5 => 23,
        6 => 24,
        13 => 22,
        14 => 20,
        15 => 21,
        other => other,
    };
    mapped | (adjusted & 0x80)
}

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
    fn angle_approach_preserves_zero_rates_and_word_crossings() {
        for (current, target, step, expected) in [
            (123, -456, 0, 123),
            (123, 123, 0, 123),
            (100, 130, 29, 129),
            (100, 130, 30, 130),
            (100, 130, -40, 130),
            (130, 100, -29, 101),
            (32760, -32760, 10, -32766),
            (-32760, 32760, 10, 32766),
            (0, i16::MIN, 1, -1),
            (0, i16::MIN, 32767, -32767),
            (0, i16::MIN, 32768, i16::MIN),
            (0, i16::MIN, -32768, i16::MIN),
            (20, -30, i32::MAX, -30),
            (20, -30, i32::MIN, 20),
        ] {
            assert_eq!(approach_angle(current, target, step), expected);
        }
    }

    #[test]
    fn command_deadline_saturates_before_word_wrap() {
        for (clock, delay, expected) in [
            (0, 0, 0),
            (100, 240, 340),
            (0x7ffe, 0, 0x7ffe),
            (0x7ffe, 1, 0x7fff),
            (0x7ffe, 2, 0x7fff),
            (0x8000, 0, 0x7fff),
            (0, 0xffff, 0x7fff),
            (0xffff, 1, 0x7fff),
            (0xffff, 0xffff, 0x7fff),
        ] {
            assert_eq!(command_deadline(clock, delay), expected);
            assert_eq!(command_deadline(delay, clock), expected);
        }
    }

    #[test]
    fn speech_delay_preserves_x86_word_shift_and_signed_gate() {
        for scale in [i16::MIN, -1, 0] {
            assert_eq!(speech_delay(0x8003, scale), 0x8003);
        }
        for (scale, expected) in [
            (1, 6),
            (15, 0x8000),
            (16, 0),
            (31, 0),
            (32, 0x8003),
            (33, 6),
            (255, 0),
            (256, 0x8003),
            (i16::MAX, 0),
        ] {
            assert_eq!(speech_delay(0x8003, scale), expected);
        }
        assert_eq!(speech_delay(0, 1), 0);
        assert_eq!(0xfffeu16.wrapping_add(speech_delay(3, 1)), 4);
    }

    #[test]
    fn strip_service_callback_override_and_clock_wrap() {
        let tail = StripServiceTail {
            callback_delay: 3,
            deadline: u16::MAX,
            clock: u16::MAX,
            controller: 0x80,
            priority: true,
            referenced: true,
            speed: 0,
        };
        assert_eq!(tail.schedule(), StripServiceSchedule::At(2));
        assert_eq!(
            StripServiceTail {
                callback_delay: 0,
                ..tail
            }
            .schedule(),
            StripServiceSchedule::At(u16::MAX)
        );
    }

    #[test]
    fn strip_service_draw_gates_and_unsigned_deadline_boundary() {
        let tail = StripServiceTail {
            callback_delay: 0x7fff,
            deadline: 0x7fff,
            clock: 0x8000,
            controller: 0,
            priority: false,
            referenced: false,
            speed: 0,
        };
        assert_eq!(
            tail.schedule(),
            StripServiceSchedule::Draw {
                base: 0x8002,
                upper_bound: 20
            }
        );
        for blocked in [
            StripServiceTail {
                deadline: 0x8000,
                ..tail
            },
            StripServiceTail {
                deadline: 0xffff,
                ..tail
            },
            StripServiceTail {
                controller: 0x80,
                ..tail
            },
            StripServiceTail {
                priority: true,
                ..tail
            },
            StripServiceTail {
                referenced: true,
                ..tail
            },
        ] {
            assert_eq!(blocked.schedule(), StripServiceSchedule::At(0x8000));
        }
        for (speed, bound) in [(i32::MIN, 20), (-1, 20), (0, 20), (1, 8), (i32::MAX, 8)] {
            assert_eq!(
                StripServiceTail {
                    clock: 0xffff,
                    controller: 0x7f,
                    speed,
                    ..tail
                }
                .schedule(),
                StripServiceSchedule::Draw {
                    base: 1,
                    upper_bound: bound
                }
            );
        }
        assert_eq!(
            StripServiceTail {
                clock: 0,
                deadline: 0xffff,
                ..tail
            }
            .schedule(),
            StripServiceSchedule::At(0)
        );
    }

    #[test]
    fn mission_nationality_preserves_high_bit_and_theater_mapping() {
        for prefix in [b'T', b't', b'U', b'u', b'K', b'k'] {
            for (raw, expected) in [(5, 23), (6, 24), (12, 22), (13, 20), (14, 21)] {
                assert_eq!(mission_nationality(raw, prefix), expected);
                assert_eq!(mission_nationality(raw | 0x80, prefix), expected | 0x80);
            }
            assert_eq!(mission_nationality(137, prefix), 138);
            assert_eq!(mission_nationality(127, prefix), 128);
            assert_eq!(mission_nationality(255, prefix), 128);
        }
        for prefix in [b'A', b'P', b'~', b'$', 0] {
            assert_eq!(mission_nationality(5, prefix), 5);
            assert_eq!(mission_nationality(12, prefix), 13);
            assert_eq!(mission_nationality(137, prefix), 138);
        }
        assert_eq!(mission_nationality(7, b'U'), 7);
        assert_eq!(mission_nationality(8, b'U'), 9);
        assert_eq!(mission_nationality(15, b'U'), 16);
    }

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
