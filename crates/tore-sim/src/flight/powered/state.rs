//! The powered-lift aircraft's own state, [`LiftState`] (the flight state's
//! `lift_controls`): the pilot's lift demands and their actuators, the body
//! rates, the rotor and engine states, the pilot aids (stability level and
//! trim), the warning timers and the V-22 corridor protection's hold.
//!
//! Every field is coded exactly with the rest of the flight state
//! ([`crate::flight::exact`]), and the coders name each field, so a field
//! added here without coding it fails to compile. The wire's own state, the
//! client's prediction and every checkpoint carry it that way.
//!
//! Which part of the VTOL overhaul fills what: the body rates
//! ([`super::body`]); the rotor speed, induced velocities, disk tilts and
//! engine output (helicopters, tandem, tiltrotor); the nozzles and lift
//! engines (jets); the pilot aids ([`super::sas`]); the warning timers; the
//! corridor hold (tiltrotor). A field an aircraft has no use for keeps its
//! default.

use super::super::{DT, FlightAxis, State};
use crate::models::variety::LiftKind;
use tore_input::{LiftCommand, NozzlePreset, StabilityLevel, TrimAxis};

/// Everything a powered-lift aircraft adds to the flight state. Positions are
/// 0..1 of each lever's travel (vector yaw -1..1).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LiftState {
    /// Pilot demands: nozzle, vector yaw, nacelle conversion and collective.
    pub vector_pitch: f64,
    pub vector_yaw: f64,
    pub conversion: f64,
    pub collective: f64,
    /// Where each actuator is now.
    pub vector_pitch_actual: f64,
    pub vector_yaw_actual: f64,
    pub conversion_actual: f64,
    pub collective_actual: f64,
    /// The lift force the aircraft makes, lbf: the main rotor thrust of a
    /// rotorcraft (or its trimmed weight before it first steps), the engines'
    /// output on a jet. For the trace and the HUD; the force laws do not
    /// read it.
    pub thrust_lbf: f64,
    /// Body rates [roll, pitch, yaw], rad/s, in the axes of
    /// [`super::body`]. The flight state's `roll_rate` and `pitch_rate`
    /// mirror the first two for telemetry.
    pub body_rates: [f64; 3],
    /// Rotor speed, engines and lift engines.
    pub drive: Drive,
    /// One entry per main rotor; single-rotor aircraft use the first.
    pub rotors: [Rotor; 2],
    /// Stability level and trim.
    pub aids: PilotAids,
    /// How long each warning condition has held.
    pub warnings: Warnings,
    /// The V-22 corridor protection's nacelle demand while it overrides or
    /// holds the pilot's (0..1 of the nacelle travel); none while the pilot's
    /// own demand stands. The pilot's demand stays in `conversion`.
    pub corridor_hold: Option<f64>,
}

/// The rotor speed and the engines driving it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Drive {
    /// Rotor speed as a share of 100 percent (Nr). Zero on the jets.
    pub rotor_speed: f64,
    /// The governor's reference rotor speed: 1, or the V-22's airplane-mode
    /// share while it moves to and from it.
    pub rotor_speed_reference: f64,
    /// Lagged engine output per engine group, [main, lift engines]: shaft
    /// power in ft·lbf/s on the rotorcraft, thrust in lbf on the jets.
    pub engine_output: [f64; 2],
    /// The Yak-141's lift engines' spool, 0..1.
    pub lift_engine_spool: f64,
    /// How far the rotors have turned: the integral of the rotor speed over
    /// time, in seconds at 100 percent (VTOL overhaul slice P7b). The drawn
    /// blade angle is this times each rotor's nominal speed, so spool-up,
    /// droop and autorotation show and the blades never jump. Presentation
    /// only (nothing in the step reads it); exact-coded so the own aircraft's
    /// prediction and checkpoints keep the same blade angle. Advanced once a
    /// tick by [`Drive::advance_turns`]; zero on every aircraft without rotor
    /// speed.
    pub rotor_turns: f64,
}

impl Drive {
    /// Turns the rotors through one tick at the rotor speed they have.
    /// Adds exactly zero when the rotor speed is zero, so a fixed-wing
    /// aircraft's state is unchanged bit for bit.
    pub fn advance_turns(&mut self) {
        self.rotor_turns += self.rotor_speed * DT;
    }
}

/// One main rotor's state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rotor {
    /// Induced velocity through the disk, ft/s (dynamic inflow).
    pub induced_fps: f64,
    /// Disk tilt from the shaft, [longitudinal (forward positive),
    /// lateral (right positive)], rad.
    pub tilt: [f64; 2],
}

/// The trim-set latch: a spring-centred stick emulating force trim.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TrimLatch {
    /// The stick adds to the trim.
    #[default]
    Free,
    /// Trim set was pressed: the next stick sample folds into the trim.
    Capture,
    /// The stick is ignored until it returns within 5 percent of centre.
    Masked,
}

/// The pilot's aids on the powered-lift aircraft (design 5.2, 5.3).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PilotAids {
    pub stability: StabilityLevel,
    /// Trim offsets [pitch, roll, pedal] added to the stick, -1..1.
    pub trim: [f64; 3],
    /// The Attitude level's reference [pitch, bank, heading], rad.
    pub attitude_reference: [f64; 3],
    pub trim_latch: TrimLatch,
}

/// Ticks each warning condition has held continuously.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Warnings {
    pub low_rotor: u32,
    pub rotor_overspeed: u32,
    pub gear_speed: u32,
    pub blade_stall: u32,
}

impl Default for LiftState {
    fn default() -> Self {
        Self {
            vector_pitch: 0.,
            vector_yaw: 0.,
            conversion: 1.,
            collective: 0.,
            vector_pitch_actual: 0.,
            vector_yaw_actual: 0.,
            conversion_actual: 1.,
            collective_actual: 0.,
            thrust_lbf: 0.,
            body_rates: [0.; 3],
            drive: Drive::default(),
            rotors: [Rotor::default(); 2],
            aids: PilotAids::default(),
            warnings: Warnings::default(),
            corridor_hold: None,
        }
    }
}

impl LiftState {
    /// The state a powered-lift aircraft of `kind` starts with before its
    /// start rule sets the levers: rotors turning at 100 percent on the
    /// rotorcraft.
    pub fn for_kind(kind: Option<LiftKind>) -> Self {
        let mut state = Self::default();
        if matches!(kind, Some(LiftKind::Helicopter | LiftKind::Tiltrotor)) {
            state.drive.rotor_speed = 1.;
            state.drive.rotor_speed_reference = 1.;
        }
        state
    }
    pub fn reset_ground(&mut self) {
        self.collective = 0.;
        self.collective_actual = 0.;
        self.thrust_lbf = 0.;
    }
    pub fn hover_fraction(&self, kind: LiftKind) -> f64 {
        match kind {
            LiftKind::VectorJet => self.vector_pitch_actual,
            // The V-22's helicopter share of its nacelle angle (slice P5).
            LiftKind::Tiltrotor => super::tiltrotor::helicopter_share(
                self.conversion_actual * crate::models::variety::V22_NACELLE_RANGE_DEGREES,
            ),
            LiftKind::Helicopter => 1.,
        }
    }
    pub(super) fn axis_mut(&mut self, axis: FlightAxis) -> &mut f64 {
        match axis {
            FlightAxis::VectorPitch => &mut self.vector_pitch,
            FlightAxis::VectorYaw => &mut self.vector_yaw,
            FlightAxis::Conversion => &mut self.conversion,
            FlightAxis::Collective => &mut self.collective,
        }
    }
}

crate::flight::exact::exact_struct!(Drive {
    rotor_speed,
    rotor_speed_reference,
    engine_output,
    lift_engine_spool,
    rotor_turns,
});
crate::flight::exact::exact_struct!(Rotor { induced_fps, tilt });
crate::flight::exact::exact_enum!(TrimLatch {
    Free = 0,
    Capture = 1,
    Masked = 2,
});
crate::flight::exact::exact_struct!(PilotAids {
    stability,
    trim,
    attitude_reference,
    trim_latch,
});
crate::flight::exact::exact_struct!(Warnings {
    low_rotor,
    rotor_overspeed,
    gear_speed,
    blade_stall,
});
crate::flight::exact::exact_struct!(LiftState {
    vector_pitch,
    vector_yaw,
    conversion,
    collective,
    vector_pitch_actual,
    vector_yaw_actual,
    conversion_actual,
    collective_actual,
    thrust_lbf,
    body_rates,
    drive,
    rotors,
    aids,
    warnings,
    corridor_hold,
});

/// A nozzle demand within half a degree of `degrees` counts as at it.
const PRESET_TOLERANCE_DEGREES: f64 = 0.5;
/// One nozzle key press, degrees (manual p. 62).
const NOZZLE_STEP_DEGREES: f64 = 10.;
/// The vertical nozzle preset, degrees (manual).
const NOZZLE_VERTICAL_DEGREES: f64 = 90.;

impl State {
    /// Applies a powered-lift command at the start of a tick. Commands that
    /// do not suit the aircraft, the legacy adapter or the native research
    /// path change nothing.
    ///
    /// - Stability level: every powered-lift aircraft. Entering the
    ///   Attitude level takes the current attitude as its reference.
    /// - Trim (set, adjust, centre): the helicopters and the V-22. At the
    ///   Attitude level the cyclic trim keys move the reference attitude
    ///   instead; trim centre also levels it. Trim set is completed by the
    ///   per-tick latch in [`super::sas`].
    /// - Nozzle steps and presets: the vectoring jets, on the demand, in
    ///   degrees of the PT's nozzle range.
    pub(crate) fn command_lift(&mut self, command: LiftCommand) {
        if self.research.is_none() || self.native.is_some() {
            return;
        }
        let Some(lift) = self.model().powered_lift() else {
            return;
        };
        let attitude = [self.pitch, self.bank, self.yaw];
        let easy_physics = self.cheats.easy_physics;
        let aids = &mut self.lift_controls.aids;
        let rotorcraft = lift.kind != LiftKind::VectorJet;
        // At the Attitude level the cyclic trim keys move the attitude it
        // returns to instead of the cyclic; with the Easy flight physics
        // cheat's retention (below Attitude) they do both, so the retention
        // pulls toward the attitude the trimmed cyclic leads to.
        let attitude_level = aids.stability == StabilityLevel::Attitude;
        let retention = easy_physics && rotorcraft && !attitude_level;
        match command {
            // Entering the Attitude level holds the attitude it finds (slice
            // P6, agent decision 2026-10-08).
            LiftCommand::SetStability(_) | LiftCommand::CycleStability => {
                aids.stability = match command {
                    LiftCommand::SetStability(level) => level,
                    _ => aids.stability.next(),
                };
                if !attitude_level && aids.stability == StabilityLevel::Attitude {
                    aids.attitude_reference = super::sas::reference_of(attitude);
                }
            }
            LiftCommand::TrimSet if rotorcraft => aids.trim_latch = TrimLatch::Capture,
            LiftCommand::TrimAdjust(axis, amount) if rotorcraft && amount.is_finite() => {
                let amount = amount.clamp(-1., 1.);
                let index = match axis {
                    TrimAxis::Pitch => 0,
                    TrimAxis::Roll => 1,
                    TrimAxis::Pedal => 2,
                };
                // At the Attitude level the cyclic trim keys move the
                // attitude it returns to instead (design 5.3).
                if attitude_level && index < 2 {
                    super::sas::adjust_reference(aids, index, amount);
                } else {
                    aids.trim[index] = (aids.trim[index] + amount).clamp(-1., 1.);
                    if retention && index < 2 {
                        super::sas::adjust_reference(aids, index, amount);
                    }
                }
            }
            LiftCommand::TrimCentre if rotorcraft => {
                aids.trim = [0.; 3];
                aids.attitude_reference[0] = 0.;
                aids.attitude_reference[1] = 0.;
            }
            LiftCommand::NozzleStep { down } => {
                if let Some(jet) = lift.jet {
                    let step = NOZZLE_STEP_DEGREES / jet.nozzle_range_degrees;
                    let demand = &mut self.lift_controls.vector_pitch;
                    *demand = (*demand + if down { step } else { -step }).clamp(0., 1.);
                }
            }
            LiftCommand::NozzlePreset(preset) => {
                if let Some(jet) = lift.jet {
                    let range = jet.nozzle_range_degrees;
                    let current = self.lift_controls.vector_pitch * range;
                    let at = |degrees: f64| (current - degrees).abs() <= PRESET_TOLERANCE_DEGREES;
                    let vertical = (NOZZLE_VERTICAL_DEGREES / range).min(1.);
                    self.lift_controls.vector_pitch = match preset {
                        NozzlePreset::Vertical if at(NOZZLE_VERTICAL_DEGREES) => 1.,
                        NozzlePreset::Vertical => vertical,
                        NozzlePreset::Forward if at(range) => vertical,
                        NozzlePreset::Forward => 0.,
                    };
                }
            }
            LiftCommand::TrimSet | LiftCommand::TrimAdjust(..) | LiftCommand::TrimCentre => {}
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::flight::exact::Exact;
    use tore_formats::aircraft::AircraftId;

    /// `base` with every field the VTOL overhaul added set to a value of its
    /// own, none of them a default, for the round-trip tests.
    pub(crate) fn busy(base: LiftState) -> LiftState {
        LiftState {
            body_rates: [0.25, -0.125, 0.0625],
            drive: Drive {
                rotor_speed: 0.93,
                rotor_speed_reference: 0.84,
                engine_output: [1.5e6, 9_000.],
                lift_engine_spool: 0.4,
                rotor_turns: 1_234.567,
            },
            rotors: [
                Rotor {
                    induced_fps: 41.5,
                    tilt: [0.03, -0.01],
                },
                Rotor {
                    induced_fps: 38.25,
                    tilt: [-0.02, 0.005],
                },
            ],
            aids: PilotAids {
                stability: StabilityLevel::Attitude,
                trim: [0.1, -0.05, 0.02],
                attitude_reference: [-0.08, 0.15, 2.5],
                trim_latch: TrimLatch::Masked,
            },
            warnings: Warnings {
                low_rotor: 3,
                rotor_overspeed: 0,
                gear_speed: 120,
                blade_stall: 7,
            },
            corridor_hold: Some(0.55),
            ..base
        }
    }

    #[test]
    fn the_lift_state_codes_exactly_against_itself_and_against_none() {
        let state = busy(LiftState::default());
        for base in [None, Some(LiftState::default()), Some(state)] {
            let mut w = tore_codec::BitWriter::new();
            state.write(&mut w, base.as_ref()).unwrap();
            let bytes = w.finish();
            let back = LiftState::read(&mut tore_codec::BitReader::new(&bytes), base.as_ref());
            assert_eq!(back.unwrap(), state);
        }
        let mut damaged = busy(LiftState::default());
        damaged.aids.trim_latch = TrimLatch::Capture;
        assert_ne!(damaged, state);
    }

    fn hybrid(id: AircraftId) -> State {
        let aircraft = crate::models::variety::tests::synthetic(id);
        let mut s = State::new(&aircraft, [0., 1_000., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s
    }

    #[test]
    fn rotorcraft_start_with_their_rotors_turning() {
        for id in [AircraftId::Ah64, AircraftId::V22] {
            let s = hybrid(id);
            assert_eq!(s.lift_controls.drive.rotor_speed, 1., "{id:?}");
            assert_eq!(s.lift_controls.drive.rotor_speed_reference, 1., "{id:?}");
        }
        assert_eq!(
            hybrid(AircraftId::F16C).lift_controls.drive,
            Drive::default()
        );
        // A vectoring jet has no rotor; its engine turns at the start
        // throttle (slice P4).
        let jet = hybrid(AircraftId::Av8).lift_controls.drive;
        assert_eq!((jet.rotor_speed, jet.rotor_speed_reference), (0., 0.));
        assert!(jet.engine_output[0] > 0. && jet.engine_output[1] == 0.);
        assert_eq!(
            hybrid(AircraftId::Ah64).lift_controls.aids.stability,
            StabilityLevel::Damper
        );
    }

    #[test]
    fn the_rotors_turn_at_their_rotor_speed_on_every_adapter() {
        let input = crate::flight::PilotInput::default();
        let ground = |_: f64, _: f64| crate::research::Surface::runway(0.);
        // The hybrid model: the rotor speed integrated a tick at a time, at
        // the start of each tick's speed.
        for id in [AircraftId::Ah64, AircraftId::Ch47, AircraftId::V22] {
            let mut s = hybrid(id);
            s.start_airborne([0.; 3]);
            let mut expected = s.lift_controls.drive.rotor_turns;
            for _ in 0..240 {
                expected += s.lift_controls.drive.rotor_speed * DT;
                s.step_surface(&input, ground);
            }
            let turns = s.lift_controls.drive.rotor_turns;
            assert!(
                (turns - expected).abs() < 1e-12,
                "{id:?}: {turns} {expected}"
            );
            assert!((1.5..2.5).contains(&turns), "{id:?}: {turns}");
        }
        // The legacy adapter's rotorcraft keep their 100 percent and turn too.
        let mut legacy = State::new(
            &crate::models::variety::tests::synthetic(AircraftId::Mi24),
            [0., 1_000., 0.],
        )
        .unwrap();
        for _ in 0..120 {
            legacy.step_surface(&input, ground);
        }
        assert!((legacy.lift_controls.drive.rotor_turns - 1.).abs() < 1e-9);
        // An aircraft without rotors never moves it off exact zero.
        for id in [AircraftId::F16C, AircraftId::Av8] {
            let mut s = hybrid(id);
            for _ in 0..120 {
                s.step_surface(&input, ground);
            }
            assert_eq!(s.lift_controls.drive.rotor_turns.to_bits(), 0, "{id:?}");
        }
    }

    #[test]
    fn the_presented_rotors_turn_and_tilt_between_ticks() {
        let mut before = hybrid(AircraftId::Ch47);
        before.lift_controls.drive.rotor_turns = 10.;
        before.lift_controls.rotors[1].tilt = [0.01, -0.02];
        let mut after = before.clone();
        after.lift_controls.drive.rotor_turns = 10. + DT;
        after.lift_controls.rotors[1].tilt = [0.03, 0.02];
        let presented = after.presented(&before, 0.25);
        assert!((presented.lift_controls.drive.rotor_turns - (10. + DT / 4.)).abs() < 1e-12);
        let tilt = presented.lift_controls.rotors[1].tilt;
        assert!((tilt[0] - 0.015).abs() < 1e-12 && (tilt[1] + 0.01).abs() < 1e-12);
    }

    #[test]
    fn stability_and_trim_commands_suit_their_aircraft() {
        use crate::flight::PilotCommand::Lift;
        let mut heli = hybrid(AircraftId::Ah64);
        heli.command(Lift(LiftCommand::CycleStability));
        assert_eq!(heli.lift_controls.aids.stability, StabilityLevel::Attitude);
        heli.command(Lift(LiftCommand::CycleStability));
        assert_eq!(heli.lift_controls.aids.stability, StabilityLevel::Off);
        heli.command(Lift(LiftCommand::SetStability(StabilityLevel::Damper)));
        assert_eq!(heli.lift_controls.aids.stability, StabilityLevel::Damper);
        heli.command(Lift(LiftCommand::TrimAdjust(TrimAxis::Pitch, -0.02)));
        heli.command(Lift(LiftCommand::TrimAdjust(TrimAxis::Pedal, 3.)));
        heli.command(Lift(LiftCommand::TrimAdjust(TrimAxis::Roll, f64::NAN)));
        assert_eq!(heli.lift_controls.aids.trim, [-0.02, 0., 1.]);
        heli.command(Lift(LiftCommand::TrimSet));
        assert_eq!(heli.lift_controls.aids.trim_latch, TrimLatch::Capture);
        heli.command(Lift(LiftCommand::TrimCentre));
        assert_eq!(heli.lift_controls.aids.trim, [0.; 3]);
        // The jets have no cyclic trim, and conventional aircraft and the
        // legacy adapter take no powered-lift command at all.
        let mut jet = hybrid(AircraftId::Av8);
        let before = jet.lift_controls;
        jet.command(Lift(LiftCommand::TrimAdjust(TrimAxis::Pitch, 0.1)));
        jet.command(Lift(LiftCommand::TrimSet));
        assert_eq!(jet.lift_controls, before);
        let mut fighter = hybrid(AircraftId::F16C);
        let before = fighter.lift_controls;
        fighter.command(Lift(LiftCommand::CycleStability));
        fighter.command(Lift(LiftCommand::NozzleStep { down: true }));
        assert_eq!(fighter.lift_controls, before);
        let aircraft = crate::models::variety::tests::synthetic(AircraftId::Ah64);
        let mut legacy = State::new(&aircraft, [0., 1_000., 0.]).unwrap();
        let before = legacy.lift_controls;
        legacy.command(Lift(LiftCommand::CycleStability));
        assert_eq!(legacy.lift_controls, before);
    }

    /// The key parts of the design's J3, J8, J9 and J12 (slice P6): the
    /// manual's key sequences set the nozzle demand they name. The nozzles'
    /// travel and what the aircraft does are P4's.
    #[test]
    fn the_manuals_nozzle_key_sequences_set_their_demands() {
        use crate::flight::PilotCommand::Lift;
        let step = |down| Lift(LiftCommand::NozzleStep { down });
        let preset = |p| Lift(LiftCommand::NozzlePreset(p));
        let degrees = |s: &State| (s.lift_controls.vector_pitch * 100.).round();
        for id in [AircraftId::Av8, AircraftId::Yak141] {
            let mut jet = hybrid(id);
            // J3: Shift+X for the vertical takeoff, Z three times at 500 ft
            // (60 degrees), Shift+Z past stall speed (0).
            jet.lift_controls.vector_pitch = 0.;
            jet.command(preset(NozzlePreset::Vertical));
            assert_eq!(degrees(&jet), 90., "{id:?}");
            for expected in [80., 70., 60.] {
                jet.command(step(false));
                assert_eq!(degrees(&jet), expected, "{id:?}");
            }
            jet.command(preset(NozzlePreset::Forward));
            assert_eq!(degrees(&jet), 0., "{id:?}");
            // J8: Shift+X twice, the braking stop.
            jet.command(preset(NozzlePreset::Vertical));
            jet.command(preset(NozzlePreset::Vertical));
            assert_eq!(degrees(&jet), 100., "{id:?}");
            // J12: Shift+Z from the stop goes to vertical, again to 0.
            jet.command(preset(NozzlePreset::Forward));
            assert_eq!(degrees(&jet), 90., "{id:?}");
            jet.command(preset(NozzlePreset::Forward));
            assert_eq!(degrees(&jet), 0., "{id:?}");
            // J9: X four times for the short takeoff, 40 degrees.
            for _ in 0..4 {
                jet.command(step(true));
            }
            assert_eq!(degrees(&jet), 40., "{id:?}");
        }
    }

    #[test]
    fn nozzle_keys_step_ten_degrees_and_presets_follow_the_manual() {
        use crate::flight::PilotCommand::Lift;
        let mut jet = hybrid(AircraftId::Av8);
        let degrees = |s: &State| (s.lift_controls.vector_pitch * 100.).round();
        jet.lift_controls.vector_pitch = 0.;
        for expected in [10., 20., 30.] {
            jet.command(Lift(LiftCommand::NozzleStep { down: true }));
            assert_eq!(degrees(&jet), expected);
        }
        jet.command(Lift(LiftCommand::NozzleStep { down: false }));
        assert_eq!(degrees(&jet), 20.);
        jet.command(Lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)));
        assert_eq!(degrees(&jet), 90.);
        jet.command(Lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)));
        assert_eq!(degrees(&jet), 100.);
        jet.command(Lift(LiftCommand::NozzleStep { down: true }));
        assert_eq!(degrees(&jet), 100.);
        jet.command(Lift(LiftCommand::NozzlePreset(NozzlePreset::Forward)));
        assert_eq!(degrees(&jet), 90.);
        jet.command(Lift(LiftCommand::NozzlePreset(NozzlePreset::Forward)));
        assert_eq!(degrees(&jet), 0.);
        jet.command(Lift(LiftCommand::NozzleStep { down: false }));
        assert_eq!(degrees(&jet), 0.);
        // Only the demand moves; the nozzles travel to it in the step.
        assert_eq!(jet.lift_controls.vector_pitch_actual, 0.);
        // No nozzles on a helicopter.
        let mut heli = hybrid(AircraftId::Mi24);
        let before = heli.lift_controls;
        heli.command(Lift(LiftCommand::NozzlePreset(NozzlePreset::Vertical)));
        assert_eq!(heli.lift_controls, before);
    }
}
