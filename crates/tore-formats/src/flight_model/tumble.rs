//! FA warning-transition tumble and stalled movement fall. Diagnostic components:
//! callers retain native movement angles, service time, trig tables and RNG.
use super::{
    departure::DepartureMode,
    integration::{MovementAngles, wrap_angle},
    match_f24,
    rotation::{AtanTable, TrigTable, cockpit_offset, degrees_to_pa, pa_to_degrees},
};
use crate::{Result, invalid};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TumbleState {
    pub start: i32,
    pub deadline: i32,
    pub progress: i16,
    pub previous_progress: i16,
    pub direction: i16,
}
impl TumbleState {
    /// Call ONLY when warning/extended-warning expires (0x47b554 / 0x47b681).
    /// Pitch/bank are the source PA words; speed is signed forward speed fixed8.
    /// A transition while a tumble is active cancels its deadline in this build.
    pub fn warning_expired(&mut self, now: i32, pitch_pa: i16, bank_pa: i16, speed_f8: i32) {
        let eligible =
            (pitch_pa >= 0 && speed_f8 < 0) || (pitch_pa >= 0x31c4 && speed_f8 <= 110 * 256);
        if now < self.deadline || !eligible {
            self.deadline = 0;
            return;
        }
        self.start = now;
        self.deadline = now.wrapping_add(512);
        if pitch_pa >= 0 {
            self.deadline = self
                .deadline
                .wrapping_add((pitch_pa as i32 * 128) / -0x3ffc);
        }
        self.previous_progress = 0;
        self.direction = if bank_pa > 0 { -1 } else { 1 };
    }

    /// 0x47ba8c..0x47bb85: returns a signed PA heading-offset increment.
    /// Native skips the terminal sample at now == deadline; do not finish the arc.
    /// No RNG is drawn by tumble itself. The caller handles touchdown event (4,64).
    pub fn advance(
        &mut self,
        table: &TrigTable,
        now: i32,
        on_ground: bool,
        mode: DepartureMode,
    ) -> Result<Option<i16>> {
        if on_ground || mode == DepartureMode::Spinning {
            self.deadline = 0;
        }
        if now >= self.deadline {
            return Ok(None);
        }
        let duration = self.deadline.wrapping_sub(self.start);
        let elapsed = now.wrapping_sub(self.start);
        if duration <= 0 || elapsed < 0 || ![-1, 1].contains(&self.direction) {
            return Err(invalid("invalid native tumble time/direction"));
        }
        // Preserve 32-bit multiplication and signed division, then low-word SUB.
        let numerator = elapsed.wrapping_mul(32760);
        let phase = (numerator / duration) as i16;
        let phase = phase.wrapping_sub(0x3ffc);
        let progress = ((table.sin_cos(phase).sin as i32 + 32767) / 256) as i16;
        let delta = ((progress as i32 - self.previous_progress as i32) * 32760 / 256) as i16;
        self.progress = progress;
        self.previous_progress = progress;
        Ok(Some(delta.wrapping_mul(self.direction)))
    }
}

/// Native 0x451820(false) -> 0x417f00(delta,0) -> 0x451820(true).
/// This rotates MOVEMENT orientation; it is not a camera-only tumble or an
/// additive body-Euler heading. Full flight later applies forces and movement.
pub fn compose_tumble(
    table: &TrigTable,
    atan: &AtanTable,
    movement: MovementAngles,
    delta_pa: i16,
) -> Result<MovementAngles> {
    Ok(compose_tumble_state(table, atan, movement, delta_pa)?.0)
}
/// Preserve the exact PA words left in native body state before inverse conversion.
pub fn compose_tumble_state(
    table: &TrigTable,
    atan: &AtanTable,
    movement: MovementAngles,
    delta_pa: i16,
) -> Result<(MovementAngles, [i16; 3])> {
    let body = [
        degrees_to_pa(movement.heading)?,
        degrees_to_pa(movement.pitch)?,
        degrees_to_pa(movement.roll.wrapping_neg())?,
    ];
    let result = cockpit_offset(table, atan, body, delta_pa, 0);
    Ok((
        MovementAngles {
            heading: pa_to_degrees(result[0])?,
            pitch: pa_to_degrees(result[1])?,
            // The source negates the PA WORD before converting, not the result.
            roll: pa_to_degrees(result[2].wrapping_neg())?,
        },
        result,
    ))
}

#[derive(Clone, Copy, Debug)]
pub struct FallInput {
    pub now: i32,
    pub tumble_deadline: i32,
    pub ticks: i16,
    pub pitch_pa: i16,
    pub severity_f8: i32,
    pub pitch_down: i16,
    /// Required only when movement roll is exactly zero. Source draws bound 256
    /// at 0x4562e0 and examines bit zero; do not substitute a chance(50) draw.
    pub zero_roll_draw: Option<u8>,
}
/// FA 0x47b2e2..0x47b36f. Invoke in the stalled branch, before timer advancement.
pub fn stalled_fall(
    table: &TrigTable,
    movement: MovementAngles,
    i: FallInput,
) -> Result<MovementAngles> {
    if i.ticks < 0 || !(0..=256).contains(&i.severity_f8) || i.pitch_down < 0 {
        return Err(invalid("invalid stalled fall inputs"));
    }
    let cos = (table.sin_cos(i.pitch_pa).cos as i32).clamp(0, 32767);
    let pitch_factor = if i.pitch_pa < 0 { cos } else { 32767 };
    let pitch_rate = pitch_factor
        .wrapping_mul(i.pitch_down as i32)
        .wrapping_mul(i.severity_f8)
        / 32767;
    let roll_rate = cos.wrapping_mul(i.severity_f8).wrapping_mul(20) / 32767;
    let positive = movement.roll > 0
        || (movement.roll == 0
            && i.zero_roll_draw
                .ok_or_else(|| invalid("missing explicit zero-roll RNG draw"))?
                & 1
                != 0);
    Ok(MovementAngles {
        pitch: if i.now >= i.tumble_deadline {
            match_f24(movement.pitch, -90 * 256, pitch_rate, i.ticks)
        } else {
            movement.pitch
        },
        roll: wrap_angle(match_f24(
            movement.roll,
            if positive { 90 * 256 } else { -90 * 256 },
            roll_rate,
            i.ticks,
        )),
        ..movement
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn table(sin: i16, cos: i16) -> TrigTable {
        let mut words = [sin; 321];
        words[64..128].fill(cos);
        let bytes: Vec<_> = words.into_iter().flat_map(i16::to_le_bytes).collect();
        TrigTable::parse(&bytes).unwrap()
    }
    #[test]
    fn scheduling_boundaries_direction_and_active_cancellation() {
        for (pitch, speed, eligible) in [
            (0, -1, true),
            (-1, -1, false),
            (0x31c3, 0, false),
            (0x31c4, 110 * 256, true),
            (0x31c4, 110 * 256 + 1, false),
        ] {
            for bank in [-1, 0, 1] {
                let mut s = TumbleState::default();
                s.warning_expired(1000, pitch, bank, speed);
                assert_eq!(s.deadline > 1000, eligible);
                if eligible {
                    assert_eq!(s.deadline, 1512 + (pitch as i32 * 128) / -0x3ffc);
                    assert_eq!(s.direction, if bank > 0 { -1 } else { 1 });
                    s.warning_expired(1001, pitch, bank, speed);
                    assert_eq!(s.deadline, 0);
                }
            }
        }
    }
    #[test]
    fn progress_is_incremental_deadline_is_exclusive_and_ground_spin_cancel() {
        let mut s = TumbleState::default();
        s.warning_expired(1000, 0, 0, -1);
        let t = table(32767, 32767);
        assert_eq!(
            s.advance(&t, 1256, false, DepartureMode::Stalled).unwrap(),
            Some(32632)
        );
        assert_eq!(s.progress, 255);
        assert_eq!(
            s.advance(&t, 1257, false, DepartureMode::Stalled).unwrap(),
            Some(0)
        );
        assert_eq!(
            s.advance(&t, 1512, false, DepartureMode::Stalled).unwrap(),
            None
        );
        for (ground, mode) in [
            (true, DepartureMode::Stalled),
            (false, DepartureMode::Spinning),
        ] {
            let mut s = TumbleState::default();
            s.warning_expired(1000, 0, 0, -1);
            assert_eq!(s.advance(&t, 1001, ground, mode).unwrap(), None);
            assert_eq!(s.deadline, 0);
        }
    }
    #[test]
    fn fall_requires_only_the_exact_zero_draw_and_suppresses_pitch_during_tumble() {
        let t = table(0, 32767);
        let m = MovementAngles {
            heading: 0,
            pitch: 0,
            roll: 0,
        };
        let i = FallInput {
            now: 10,
            tumble_deadline: 0,
            ticks: 256,
            pitch_pa: 0,
            severity_f8: 256,
            pitch_down: 30,
            zero_roll_draw: None,
        };
        assert!(stalled_fall(&t, m, i).is_err());
        for draw in [0, 1, 254, 255] {
            let out = stalled_fall(
                &t,
                m,
                FallInput {
                    zero_roll_draw: Some(draw),
                    ..i
                },
            )
            .unwrap();
            assert_eq!(out.pitch, -30 * 256);
            assert_eq!(out.roll, if draw & 1 != 0 { 20 * 256 } else { -20 * 256 });
        }
        let out = stalled_fall(
            &t,
            MovementAngles { roll: 1, ..m },
            FallInput {
                tumble_deadline: 11,
                ..i
            },
        )
        .unwrap();
        assert_eq!(out.pitch, 0);
        assert_eq!(out.roll, 1 + 20 * 256);
        let out = stalled_fall(&t, MovementAngles { roll: -1, ..m }, i).unwrap();
        assert_eq!(out.roll, -1 - 20 * 256);
    }
}
