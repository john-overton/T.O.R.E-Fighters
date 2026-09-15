//! Reviewed departure dispatch, joined in native movement coordinates.
//! This is a diagnostic stage, not the normal-control/force/full-flight update.
use super::{
    clock_rng::NativeRng,
    departure::{
        self, DepartureMode, DepartureProfile, SpinInput, SpinMotion, SpinState, StallState,
    },
    integration::MovementAngles,
    rotation::{AtanTable, TrigTable, degrees_to_pa},
    tumble::{FallInput, TumbleState, compose_tumble_state, stalled_fall},
};
use crate::{Result, invalid};

/// 0x47b4ec..0x47b50c and 0x47b9e8: flag 0x20 moves the requested
/// whole-G row one step toward the [-1,1] interval. It does not clamp G to 2.
pub fn initial_envelope_g(g_f8: i32, global_flags: u32) -> i32 {
    let g = g_f8 >> 8;
    if global_flags & 0x20 == 0 {
        g
    } else if g > 1 {
        g - 1
    } else if g < -1 {
        g + 1
    } else {
        g
    }
}

/// Source-selected envelope results. Keep current-G severity, initial-warning
/// classification, bounded-G stall predicate and clean 1G reference distinct.
#[derive(Clone, Copy, Debug)]
pub struct EnvelopeInputs {
    pub initial_class: u8,
    pub bounded_g_class: u8,
    pub current_g_stall_fps: i32,
    pub clean_stall_fps: i32,
    /// First 1G polygon vertex (0x49d1b0), not altitude-adjusted intersection.
    pub minimum_lift_fps: i32,
}
impl EnvelopeInputs {
    /// Resolve distinct native queries against reviewed typed PT polygons.
    /// The caller supplies native G state, not an adapter accelerometer reading.
    pub fn resolve(
        envelopes: &[crate::aircraft::Envelope],
        structure: [i16; 2],
        g_f8: i32,
        altitude_f8: i32,
        speed_f8: i32,
        flaps: bool,
        flags: u32,
    ) -> Result<Self> {
        let limits = |g: i32| -> Result<Option<super::Limits>> {
            envelopes
                .iter()
                .find(|e| e.g == g)
                .map(|e| super::envelope_limits(e, altitude_f8, flaps, structure))
                .transpose()
        };
        let class = |g| -> Result<u8> {
            Ok(limits(g)?
                .as_ref()
                .map_or(1, |l| super::envelope_class(l, speed_f8)))
        };
        let stall = |g| -> Result<i32> {
            Ok(limits(g)?
                .ok_or_else(|| invalid("missing native severity/clean envelope"))?
                .minimum
                .max(1))
        };
        let clean = envelopes
            .iter()
            .find(|e| e.g == 1)
            .ok_or_else(|| invalid("missing native 1G envelope"))?;
        let first = clean
            .points
            .first()
            .ok_or_else(|| invalid("empty native 1G envelope"))?[0];
        Ok(Self {
            initial_class: class(initial_envelope_g(g_f8, flags))?,
            bounded_g_class: class(departure::stall_envelope_g(g_f8))?,
            current_g_stall_fps: stall(g_f8 >> 8)?,
            clean_stall_fps: stall(1)?,
            minimum_lift_fps: first as i32,
        })
    }
    pub fn below_stall(self, vertical_thrust_support: bool) -> bool {
        self.bounded_g_class == 1 && !vertical_thrust_support
    }
}

/// 0x47b207..0x47b250. Source commands [roll,pitch,rudder], ±256 domain.
pub fn ground_controls(mut controls: [i32; 3], speed_f8: i32, first_1g_speed: i32) -> [i32; 3] {
    if speed_f8 >> 8 < first_1g_speed.min(73) {
        controls[1] = 0;
    }
    if speed_f8 < 5 * 256 {
        controls[0] = 0;
        controls[2] = 0;
    }
    controls
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct StageState {
    pub departure: StallState,
    pub spin: SpinState,
    pub tumble: TumbleState,
    pub movement: MovementAngles,
    /// Body response rates [roll,pitch,yaw], signed degrees fixed8.
    pub body_rates_f8: [i32; 3],
    pub speed_f8: i32,
    /// Native display offsets [bank,AoA,slip], distinct from movement angles.
    pub offsets_f8: [i32; 3],
}
#[derive(Clone, Copy, Debug)]
pub struct StageInput {
    pub now: i32,
    pub ticks: i16,
    pub on_ground: bool,
    /// Existing cp body-bank word; FMFlight refreshes pitch from movement at
    /// entry but does not reconstruct this bank word there.
    pub body_bank_pa: i16,
    pub global_flags: u32,
    pub extended_warning: bool,
    pub vertical_thrust_support: bool,
    pub thrust_vector_f8: i32,
    pub throttle_f8: i32,
    pub controls: [i32; 3],
    /// Caller-owned cp flag 0x02000000; kept independently of departure mode.
    pub recovery_locked: bool,
    pub envelopes: EnvelopeInputs,
    pub lift_scale_f8: i32,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StageOutput {
    pub controls: [i32; 3],
    pub lift_scale_f8: i32,
    pub severity_f8: i32,
    /// Source spin branch jumps over normal controls even on its recovery tick.
    pub run_normal_controls: bool,
    pub recovered_spin: bool,
    pub tumble_applied: bool,
    pub tumble_body_pa: Option<[i16; 3]>,
    pub recovery_locked: bool,
}
impl StageState {
    /// Atomic diagnostic stage: failed input/composition cannot consume RNG/state.
    pub fn advance(
        &mut self,
        p: &DepartureProfile,
        t: &TrigTable,
        atan: &AtanTable,
        rng: &mut NativeRng,
        i: StageInput,
    ) -> Result<StageOutput> {
        if i.ticks < 0
            || i.controls.iter().any(|x| !(-256..=256).contains(x))
            || i.envelopes.current_g_stall_fps <= 0
            || i.envelopes.clean_stall_fps <= 0
            || p.warning_delay < 0
            || p.stall_delay < 0
            || p.severity < 0
            || p.pitch_down < 0
        {
            return Err(invalid("invalid native departure stage inputs"));
        }
        let mut next = *self;
        let mut next_rng = rng.clone();
        let out = next.advance_inner(p, t, atan, &mut next_rng, i)?;
        *self = next;
        *rng = next_rng;
        Ok(out)
    }
    fn advance_inner(
        &mut self,
        p: &DepartureProfile,
        t: &TrigTable,
        atan: &AtanTable,
        rng: &mut NativeRng,
        i: StageInput,
    ) -> Result<StageOutput> {
        let mut out = StageOutput {
            controls: i.controls,
            lift_scale_f8: i.lift_scale_f8,
            severity_f8: 0,
            run_normal_controls: true,
            recovered_spin: false,
            tumble_applied: false,
            tumble_body_pa: None,
            recovery_locked: i.recovery_locked,
        };
        let pitch_pa = degrees_to_pa(self.movement.pitch)?;
        let bank_pa = i.body_bank_pa;
        if i.on_ground {
            self.departure.mode = DepartureMode::Normal;
            out.controls =
                ground_controls(out.controls, self.speed_f8, i.envelopes.minimum_lift_fps);
        }
        let spin_input = SpinInput {
            pitch_stick: out.controls[1],
            rudder: out.controls[2],
            throttle_f8: i.throttle_f8,
            speed_f8: self.speed_f8,
            clean_stall_fps: i.envelopes.clean_stall_fps,
            thrust_vector_f8: i.thrust_vector_f8,
            inhibited: i.global_flags & 0x40000 != 0,
        };
        if !spin_input.inhibited
            && p.spin_entry != 2
            && i.thrust_vector_f8 > -45 * 256
            && matches!(
                self.departure.mode,
                DepartureMode::Warning | DepartureMode::Stalled
            )
        {
            let tie = self.body_rates_f8[0] == 0 && bank_pa == 0;
            let direction =
                departure::spin_direction(self.body_rates_f8[0], bank_pa, tie && rng.chance(50)?);
            if departure::spin_entry(p, self.departure.mode, spin_input, direction) {
                self.spin = SpinState::entered(direction, i.recovery_locked)?;
                self.departure.mode = DepartureMode::Spinning;
            }
        }
        if self.departure.mode == DepartureMode::Spinning {
            self.spin.recovery_locked = i.recovery_locked;
            let mut motion = SpinMotion {
                body_rates_f8: self.body_rates_f8,
                movement_pitch_f8: self.movement.pitch,
                movement_roll_f8: self.movement.roll,
                speed_f8: self.speed_f8,
                bank_offset_f8: self.offsets_f8[0],
                aoa_offset_f8: self.offsets_f8[1],
                slip_offset_f8: self.offsets_f8[2],
            };
            out.recovered_spin = self.spin.advance(&mut motion, p, spin_input, i.ticks)?;
            self.body_rates_f8 = motion.body_rates_f8;
            self.movement.pitch = motion.movement_pitch_f8;
            self.movement.roll = motion.movement_roll_f8;
            self.speed_f8 = motion.speed_f8;
            self.offsets_f8 = [
                motion.bank_offset_f8,
                motion.aoa_offset_f8,
                motion.slip_offset_f8,
            ];
            out.recovery_locked = self.spin.recovery_locked;
            out.run_normal_controls = false;
            if out.recovered_spin {
                self.departure.mode = DepartureMode::Normal;
            }
            // 0x47b9e3 jumps to 0x47c682, bypassing tumble and normal controls.
            return Ok(out);
        }
        let before = self.departure;
        if before.mode == DepartureMode::Stalled {
            out.severity_f8 = departure::stall_severity(
                p,
                before.elapsed,
                self.speed_f8 >> 8,
                i.envelopes.current_g_stall_fps,
            )?;
            let draw = if self.movement.roll == 0 {
                Some(rng.below(256)? as u8)
            } else {
                None
            };
            self.movement = stalled_fall(
                t,
                self.movement,
                FallInput {
                    now: i.now,
                    tumble_deadline: self.tumble.deadline,
                    ticks: i.ticks,
                    pitch_pa,
                    severity_f8: out.severity_f8,
                    pitch_down: p.pitch_down,
                    zero_roll_draw: draw,
                },
            )?;
            (out.controls, out.lift_scale_f8) =
                departure::stall_authority(out.severity_f8, out.controls, out.lift_scale_f8);
        }
        let below = i.envelopes.below_stall(i.vertical_thrust_support);
        self.departure.advance(
            p,
            below,
            below && i.envelopes.initial_class == 1,
            i.extended_warning,
            false,
            i.ticks,
        )?;
        if below
            && matches!(
                before.mode,
                DepartureMode::Warning | DepartureMode::ExtendedWarning
            )
            && before.mode != self.departure.mode
        {
            self.tumble
                .warning_expired(i.now, pitch_pa, bank_pa, self.speed_f8);
        }
        if let Some(delta) = self
            .tumble
            .advance(t, i.now, i.on_ground, self.departure.mode)?
        {
            let (movement, body) = compose_tumble_state(t, atan, self.movement, delta)?;
            self.movement = movement;
            out.tumble_body_pa = Some(body);
            out.tumble_applied = true;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables() -> (TrigTable, AtanTable) {
        (
            TrigTable::parse(&[0; 642]).unwrap(),
            AtanTable::parse(&[0; 1028]).unwrap(),
        )
    }
    fn profile() -> DepartureProfile {
        DepartureProfile {
            warning_delay: 4,
            stall_delay: 4,
            severity: 256,
            pitch_down: 30,
            spin_entry: 0,
            spin_exit: -2,
            spin_yaw: [120, 180],
            spin_aoa: [30, 70],
            spin_bank: [15, 5],
        }
    }
    fn input() -> StageInput {
        StageInput {
            now: 1000,
            ticks: 2,
            on_ground: false,
            body_bank_pa: 0,
            global_flags: 0,
            extended_warning: false,
            vertical_thrust_support: false,
            thrust_vector_f8: 0,
            throttle_f8: 0,
            controls: [0; 3],
            recovery_locked: false,
            lift_scale_f8: 256,
            envelopes: EnvelopeInputs {
                initial_class: 1,
                bounded_g_class: 1,
                current_g_stall_fps: 200,
                clean_stall_fps: 200,
                minimum_lift_fps: 100,
            },
        }
    }
    #[test]
    fn source_queries_keep_four_envelope_roles_distinct() {
        let envelopes: Vec<_> = (0..=4)
            .map(|g| crate::aircraft::Envelope {
                g,
                points: vec![
                    [100. + 100. * g as f64, 0.],
                    [100. + 100. * g as f64, 10000.],
                    [1000., 10000.],
                    [1500., 0.],
                ],
            })
            .collect();
        let plain =
            EnvelopeInputs::resolve(&envelopes, [1600, 1800], 4 * 256, 0, 450 * 256, false, 0)
                .unwrap();
        assert_eq!(plain.initial_class, 1);
        assert_eq!(plain.bounded_g_class, 0);
        assert_eq!(plain.current_g_stall_fps, 500);
        assert_eq!(plain.clean_stall_fps, 200);
        assert_eq!(plain.minimum_lift_fps, 200);
        assert!(!plain.below_stall(false));
        let eased =
            EnvelopeInputs::resolve(&envelopes, [1600, 1800], 4 * 256, 0, 450 * 256, true, 0x20)
                .unwrap();
        assert_eq!(eased.initial_class, 0);
        assert_eq!(eased.current_g_stall_fps, 500);
        assert_eq!(eased.clean_stall_fps, 150);
        assert_eq!(eased.minimum_lift_fps, 200);
        let slow =
            EnvelopeInputs::resolve(&envelopes, [1600, 1800], 256, 0, 100 * 256, false, 0).unwrap();
        assert!(slow.below_stall(false));
        assert!(!slow.below_stall(true));
    }

    #[test]
    fn difficulty_rows_and_ground_thresholds_preserve_source_boundaries() {
        for (g, normal, easy) in [
            (-4, -4, -3),
            (-1, -1, -1),
            (0, 0, 0),
            (1, 1, 1),
            (2, 2, 1),
            (9, 9, 8),
        ] {
            assert_eq!(initial_envelope_g(g * 256, 0), normal);
            assert_eq!(initial_envelope_g(g * 256, 0x20), easy);
        }
        assert_eq!(initial_envelope_g(-1, 0), -1);
        assert_eq!(ground_controls([256; 3], 5 * 256 - 1, 200), [0, 0, 0]);
        assert_eq!(ground_controls([256; 3], 5 * 256, 200), [256, 0, 256]);
        assert_eq!(ground_controls([256; 3], 73 * 256, 200), [256; 3]);
    }
    #[test]
    fn stage_enters_warning_then_spin_and_recovery_skips_normal_controls() {
        let (t, a) = tables();
        let mut s = StageState {
            speed_f8: 180 * 256,
            body_rates_f8: [256, 0, 0],
            ..Default::default()
        };
        let mut rng = NativeRng::seeded(1).unwrap();
        let p = profile();
        let i = StageInput {
            controls: [0, 256, 256],
            ..input()
        };
        assert!(
            s.advance(&p, &t, &a, &mut rng, i)
                .unwrap()
                .run_normal_controls
        );
        assert_eq!(s.departure.mode, DepartureMode::Warning);
        let o = s.advance(&p, &t, &a, &mut rng, i).unwrap();
        assert!(!o.run_normal_controls);
        assert_eq!(s.departure.mode, DepartureMode::Spinning);
        assert!(s.movement.pitch < 0 && s.movement.roll > 0);
        assert!(s.speed_f8 > 180 * 256);
        s.speed_f8 = 250 * 256;
        let recover = StageInput {
            ticks: 256,
            controls: [0, -1, -200],
            ..i
        };
        let o = s.advance(&p, &t, &a, &mut rng, recover).unwrap();
        assert!(o.recovered_spin && !o.run_normal_controls);
        assert_eq!(s.departure.mode, DepartureMode::Normal);
    }
    #[test]
    fn spin_direction_uses_supplied_body_bank_not_movement_roll() {
        let (t, a) = tables();
        let mut s = StageState {
            departure: StallState {
                mode: DepartureMode::Warning,
                elapsed: 0,
            },
            speed_f8: 180 * 256,
            movement: MovementAngles {
                roll: 5 * 256,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut rng = NativeRng::seeded(1).unwrap();
        s.advance(
            &profile(),
            &t,
            &a,
            &mut rng,
            StageInput {
                body_bank_pa: 100,
                controls: [0, 256, -256],
                ..input()
            },
        )
        .unwrap();
        assert_eq!(s.departure.mode, DepartureMode::Spinning);
        assert_eq!(s.spin.direction, -1);
    }

    #[test]
    fn expiry_schedules_tumble_and_errors_preserve_state_and_rng() {
        let (t, a) = tables();
        let mut s = StageState {
            departure: StallState {
                mode: DepartureMode::Warning,
                elapsed: 2,
            },
            speed_f8: 100 * 256,
            movement: MovementAngles {
                pitch: 75 * 256,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut rng = NativeRng::seeded(1).unwrap();
        let o = s.advance(&profile(), &t, &a, &mut rng, input()).unwrap();
        assert!(o.tumble_applied);
        assert_eq!(s.departure.mode, DepartureMode::Stalled);
        assert!(s.tumble.deadline > 1000);
        let before = s;
        let rng_before = rng.clone();
        assert!(
            s.advance(
                &profile(),
                &t,
                &a,
                &mut rng,
                StageInput {
                    ticks: -1,
                    ..input()
                }
            )
            .is_err()
        );
        assert_eq!(s, before);
        assert_eq!(rng, rng_before);
    }
    #[test]
    fn stalled_severity_uses_current_g_reference_and_preincrement_time() {
        let (t, a) = tables();
        let mut s = StageState {
            departure: StallState {
                mode: DepartureMode::Stalled,
                elapsed: 1024,
            },
            speed_f8: 100 * 256,
            movement: MovementAngles {
                roll: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut rng = NativeRng::seeded(1).unwrap();
        let out = s
            .advance(
                &profile(),
                &t,
                &a,
                &mut rng,
                StageInput {
                    controls: [256; 3],
                    global_flags: 0x40000,
                    ..input()
                },
            )
            .unwrap();
        assert_eq!(out.severity_f8, 256);
        assert_eq!(out.controls, [37, 150, 37]);
        assert_eq!(out.lift_scale_f8, 0);
        assert_eq!(s.departure.elapsed, 1026);
    }
}
