//! Acceptance tests E1 and E2 of the VTOL overhaul design (section 10) for
//! the Easy flight physics cheat (slices P8 and P8b), on the AH-64, Mi-24,
//! AV-8, Yak-141, CH-47 and V-22, plus the exact state round trip with the
//! cheat on and the proof that fixed-wing flight does not notice it. The
//! CH-47 and V-22 tests are in the second half of the file. The menu and session rules (E3) are tested where they
//! live, in `tore-app` and `tore-world`.
//!
//! Each E1 test flies the same scenario with the hazards and with the cheat
//! on, so a pass shows both that the hazard was really there and that the
//! cheat removes it. Where the numbers differ from the design's, the test
//! says how (P8 notes in the design document).

use super::helicopter::tests::{fly, heli, pt_aircraft, trimmed};
use super::jet::tests as jets;
use super::*;
use crate::flight::DT;
use crate::flight::{PilotCommand, PilotInput, Switch};
use tore_formats::aircraft::AircraftId::{self, Ah64, Av8, Mi24, Yak141};
use tore_input::StabilityLevel;

const KT: f64 = 1.687_81;
const HELICOPTERS: [AircraftId; 2] = [Ah64, Mi24];
const JETS: [AircraftId; 2] = [Av8, Yak141];

/// `trimmed` with the cheat set before the trim, as a start or the trim
/// routine under the cheat would do it.
fn easy_trimmed(id: AircraftId, height: f64, airspeed_fps: f64) -> State {
    let mut s = State::new(&pt_aircraft(id), [0., height, 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.cheats.easy_physics = true;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.velocity = [0.; 3];
    s.speed = 0.;
    s.lift_controls.aids.stability = StabilityLevel::Off;
    assert!(s.trim_single_rotor(airspeed_fps), "{id:?} trims");
    s
}

/// The helicopter trimmed at stability level Off, with or without the cheat.
fn helicopter(id: AircraftId, easy: bool, height: f64, airspeed_fps: f64) -> State {
    if easy {
        easy_trimmed(id, height, airspeed_fps)
    } else {
        trimmed(id, height, airspeed_fps)
    }
}

fn wrap(angle: f64) -> f64 {
    (angle + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

fn degrees(rate: f64) -> f64 {
    rate.to_degrees()
}

/// Stick and pedals that hold the attitude level and the heading north,
/// with `collective` as the lever.
fn hold(s: &State, collective: f64) -> PilotInput {
    hold_pitch(s, collective, 0.)
}

/// `hold` with the pitch attitude held at `pitch` radians instead of level.
fn hold_pitch(s: &State, collective: f64, pitch: f64) -> PilotInput {
    let [p, q, r] = s.lift_controls.body_rates;
    PilotInput {
        pitch: (2.5 * (pitch - s.pitch) - 1.2 * q).clamp(-1., 1.),
        roll: (2.5 * (0. - s.bank) - 0.4 * p).clamp(-1., 1.),
        yaw: (2. * wrap(0. - s.yaw) - 1.5 * r).clamp(-1., 1.),
        collective: Some(collective),
        ..Default::default()
    }
}

/// The hazards in force are the cheat's: all or none, for both owners.
#[test]
fn the_cheat_builds_the_hazards_in_one_place_per_owner() {
    let mut s = trimmed(Ah64, 1_000., 0.);
    assert_eq!(s.rotor_hazards(), rotor::Hazards::ALL);
    s.cheats.easy_physics = true;
    assert_eq!(s.rotor_hazards(), rotor::Hazards::NONE);
    let mut jet = jets::hover(Av8, 1_000.);
    assert_eq!(jet.jet_hazards(), jet::Hazards::default());
    jet.cheats.easy_physics = true;
    assert_eq!(
        jet.jet_hazards(),
        jet::Hazards {
            roll_off: false,
            undamped_puffers: false,
            dynamic_rollover: false,
        }
    );
}

/// The sink of a vertical descent at one hover induced velocity, after a
/// second at the hover collective and then `seconds` of full collective
/// with the attitude held, and the time it took to stop sinking.
fn vortex_ring(id: AircraftId, easy: bool, seconds: usize) -> (f64, f64, Option<f64>) {
    let mut s = helicopter(id, easy, 4_000., 0.);
    let h = heli(&s);
    let weight = s.model().configuration().mass.empty_lbs + s.fuel;
    let vh = (weight / (2. * rotor::air_density(4_000.) * h.rotor.area_ft2)).sqrt();
    s.velocity[1] = -vh;
    let lever = s.lift_controls.collective;
    fly(&mut s, 120, |s| hold(s, lever));
    let start = s.vertical_speed;
    let mut arrested = None;
    let mut after = start;
    for tick in 0..120 * seconds {
        fly(&mut s, 1, |s| hold(s, 1.));
        if s.vertical_speed >= 0. && arrested.is_none() {
            arrested = Some((tick + 1) as f64 / 120.);
        }
        if tick + 1 == 3 * 120 {
            after = s.vertical_speed;
        }
    }
    (start, after, arrested)
}

/// E1 / H10: the vortex ring state. Full collective does not arrest the
/// sink in 3 s with the hazard; with the cheat it does, the Mi-24 within
/// 3 s and the AH-64 within 7 s (its rotor's heave time constant is 4.5 s,
/// so the design's 3 s is out of reach for a 41 ft/s sink).
#[test]
fn e1_h10_full_collective_arrests_a_vertical_descent_with_the_cheat() {
    for (id, limit) in [(Ah64, 7.), (Mi24, 3.)] {
        let (start, ring, ring_arrested) = vortex_ring(id, false, 8);
        let (easy_start, easy, easy_arrested) = vortex_ring(id, true, 8);
        assert!(start < -20. && easy_start < -20., "{id:?}");
        assert!(
            ring_arrested.is_none_or(|t| t > 3.),
            "{id:?} with the hazard {ring_arrested:?}"
        );
        let seconds = easy_arrested.unwrap_or_else(|| panic!("{id:?} never arrested"));
        assert!(seconds <= limit, "{id:?} arrested in {seconds} s");
        assert!(
            easy > ring + 10. && easy > 0.4 * easy_start,
            "{id:?} after 3 s: {easy} ft/s against {ring} with the hazard"
        );
    }
}

/// E1 / H11: past the never-exceed speed the stick-frozen aircraft with the
/// hazard pitches up harder and rolls toward the retreating side; with the
/// cheat it keeps its attitude (the blowback that every fast helicopter has
/// stays) and the vibration cue still sounds.
#[test]
fn e1_h11_no_pitch_up_or_roll_past_vne_with_the_cheat() {
    let dive = |id: AircraftId, easy: bool| {
        let mut s = helicopter(id, easy, 4_000., 120. * KT);
        s.cheats.damage = crate::cheats::Damage::Invulnerable;
        s.velocity = [0., 0., 215. * KT];
        s.speed = 215. * KT;
        let (pitch, bank) = (s.pitch, s.bank);
        let lever = s.lift_controls.collective;
        let mut cue = 0;
        fly(&mut s, 240, |s| {
            cue = cue.max(s.lift_controls.warnings.blade_stall);
            PilotInput {
                collective: Some(lever),
                ..Default::default()
            }
        });
        (degrees(s.pitch - pitch), degrees(s.bank - bank), cue)
    };
    for id in HELICOPTERS {
        let (stall_pitch, stall_bank, stall_cue) = dive(id, false);
        let (pitch, bank, cue) = dive(id, true);
        assert!(
            stall_bank.abs() > 25. && stall_cue > 0,
            "{id:?} {stall_bank}"
        );
        assert!(bank.abs() < 8., "{id:?} bank {bank} against {stall_bank}");
        assert!(
            pitch < stall_pitch - 1.,
            "{id:?} pitch {pitch} against {stall_pitch}"
        );
        assert!(cue > 0, "{id:?} the vibration cue stays");
    }
}

/// E1 / H12: a 30 percent collective step with the pedals fixed, at Off.
/// The hazard swings the nose at 26 (AH-64) and 43 (Mi-24) deg/s; with the
/// cheat it stays under 1.5 deg/s (the design's 1 deg/s, but the Mi-24's
/// tail rotor side force leaves 1.2 deg/s).
#[test]
fn e1_h12_collective_does_not_yaw_the_aircraft_with_the_cheat() {
    for id in HELICOPTERS {
        let swing = |easy: bool| {
            let mut s = helicopter(id, easy, 3_000., 0.);
            let lever = s.lift_controls.collective + 0.3;
            let mut fastest: f64 = 0.;
            fly(&mut s, 240, |s| {
                fastest = fastest.max(s.lift_controls.body_rates[2].abs());
                PilotInput {
                    collective: Some(lever),
                    ..Default::default()
                }
            });
            degrees(fastest)
        };
        let torque = swing(false);
        let easy = swing(true);
        assert!(torque >= 10., "{id:?} {torque} deg/s");
        assert!(easy < 1.5, "{id:?} {easy} deg/s");
    }
}

/// E1 / H9b: an engine cut in a hover with the collective held. The rotor
/// droops past 70 percent with the hazard; with the cheat it never falls
/// below 85 percent in flight.
#[test]
fn e1_h9b_the_rotor_never_falls_below_85_percent_with_the_cheat() {
    for id in HELICOPTERS {
        let lowest = |easy: bool| {
            let mut s = helicopter(id, easy, 3_000., 0.);
            let lever = s.lift_controls.collective;
            s.command(PilotCommand::Set(Switch::Engine, false));
            let mut low: f64 = 1.;
            fly(&mut s, 120 * 8, |s| {
                low = low.min(s.lift_controls.drive.rotor_speed);
                PilotInput {
                    collective: Some(lever),
                    ..Default::default()
                }
            });
            low
        };
        assert!(lowest(false) < 0.7, "{id:?}");
        let easy = lowest(true);
        assert!((0.85..0.86).contains(&easy), "{id:?} {easy}");
    }
}

/// A helicopter on the runway with its rotor turning, 20 degrees banked
/// with the collective at 0.8 of the hover lever and the thrust leaning the
/// way it is banked.
fn banked_on_the_ground(id: AircraftId, easy: bool) -> State {
    let mut s = State::new(&pt_aircraft(id), [0., 0., 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.cheats.easy_physics = easy;
    s.start_on_runway([0., 0., 0.], 0.).unwrap();
    s.throttle = 1.;
    s.gear_down = true;
    s.gear = 1.;
    fly(&mut s, 120, |_| PilotInput::default());
    assert!(s.weight_on_wheels() && !s.crashed);
    let lever = trimmed(id, 0., 0.).lift_controls.collective;
    s.lift_controls.collective = 0.8 * lever;
    s.lift_controls.collective_actual = 0.8 * lever;
    s.bank = 20_f64.to_radians();
    s.lift_controls.rotors[0].tilt[1] = 0.02;
    s
}

/// E1 / H14: dynamic rollover. 20 degrees of bank under thrust is a crash on
/// the first tick with the hazard; with the cheat the aircraft is not tipped
/// over (it lifts off banked, and a banked touchdown past the landing limits
/// is still the normal contact rules' crash, 0.6 s later).
#[test]
fn e1_h14_no_dynamic_rollover_with_the_cheat() {
    for id in HELICOPTERS {
        let mut s = banked_on_the_ground(id, false);
        fly(&mut s, 1, |_| PilotInput::default());
        assert!(s.crashed, "{id:?} with the hazard");
        let mut s = banked_on_the_ground(id, true);
        fly(&mut s, 30, |_| PilotInput::default());
        assert!(!s.crashed, "{id:?} with the cheat");
    }
}

/// Bank after 3 s hands off, jetborne at 40 kt with 10 degrees of
/// sideslip at stability level Off, and the same at Damper.
fn roll_off(id: AircraftId, level: StabilityLevel, easy: bool) -> f64 {
    let mut s = jets::at(jets::hover(id, 1_000.), level);
    s.cheats.easy_physics = easy;
    let speed = 40. * KT;
    let slip = 10_f64.to_radians();
    s.velocity = [speed * slip.sin(), 0., speed * slip.cos()];
    s.speed = speed;
    let throttle = s.throttle;
    jets::run(
        &mut s,
        &PilotInput {
            throttle: Some(throttle),
            ..Default::default()
        },
        360,
    );
    s.bank.to_degrees().abs()
}

/// E1 / J11: the low-speed roll-off. At Off the AV-8 rolls off past 30
/// degrees in 3 s (the Yak-141 to 9); with the cheat both stay under 2.
#[test]
fn e1_j11_no_roll_off_at_off_with_the_cheat() {
    let hazard = roll_off(Av8, StabilityLevel::Off, false);
    assert!(hazard > 30., "AV-8 Off {hazard}");
    let hazard = roll_off(Yak141, StabilityLevel::Off, false);
    assert!(hazard > 5., "Yak-141 Off {hazard}");
    for id in JETS {
        let off = roll_off(id, StabilityLevel::Off, true);
        assert!(off < 2., "{id:?} Off with the cheat {off}");
        // The cheat adds no roll to a level the jet already damps.
        let damper = roll_off(id, StabilityLevel::Damper, true);
        assert!(damper < 2., "{id:?} Damper with the cheat {damper}");
    }
}

/// E1: the undamped puffers. Full stick at Off spins the puffers up to
/// rates far over the PT's; with the cheat a jet at Off flies the Damper's
/// rates, and one at the Damper is not changed.
#[test]
fn e1_puffers_get_the_dampers_rate_damping_at_off_with_the_cheat() {
    for id in JETS {
        let damper = jets::hover_rates_easy(id, StabilityLevel::Damper, false);
        let off = jets::hover_rates_easy(id, StabilityLevel::Off, false);
        let easy = jets::hover_rates_easy(id, StabilityLevel::Off, true);
        // At Off with the cheat the jet flies the Damper's law exactly (the
        // cheat's own hazard-free air), and within a share of a percent of
        // the Damper with the hazards.
        assert_eq!(
            easy,
            jets::hover_rates_easy(id, StabilityLevel::Damper, true),
            "{id:?}"
        );
        for axis in 0..3 {
            assert!(
                (easy[axis] / damper[axis] - 1.).abs() < 0.02,
                "{id:?} {easy:?} against {damper:?}"
            );
        }
        assert!(
            off[1] > 2. * damper[1],
            "{id:?} Off {off:?} Damper {damper:?}"
        );
        // Without hydraulics there is no augmentation to give.
        let mut s = jets::at(jets::hover(id, 1_000.), StabilityLevel::Off);
        s.cheats.easy_physics = true;
        s.systems.fluids.hydraulic = 0.;
        assert_eq!(s.puffer_level(s.jet_hazards()), StabilityLevel::Off);
    }
}

/// E1 / dynamic rollover on the jets: light on the wheels with the thrust
/// nearly carrying the AV-8, full roll tips it past 15 degrees and is a
/// crash with the hazard; with the cheat the normal contact rules hold it.
#[test]
fn e1_the_jets_do_not_roll_over_on_the_ground_with_the_cheat() {
    for (easy, crashed) in [(false, true), (true, false)] {
        let mut light = jets::hover(Av8, 0.);
        light.cheats.easy_physics = easy;
        light.position[1] = light.model().configuration().equipment.ground_clearance_ft + 0.001;
        light.velocity = [0., -1., 0.];
        jets::run(&mut light, &PilotInput::default(), 1);
        assert!(light.weight_on_wheels());
        light.throttle *= 1.06;
        let throttle = light.throttle;
        jets::run(
            &mut light,
            &PilotInput {
                roll: 1.,
                throttle: Some(throttle),
                ..Default::default()
            },
            240,
        );
        assert_eq!(light.crashed, crashed, "easy {easy}, bank {}", light.bank);
    }
}

/// The fastest level trim, kt, at sea level with the power the engines
/// have, under the given hazards.
fn top_speed(id: AircraftId, hazards: rotor::Hazards) -> f64 {
    let s = trimmed(id, 0., 0.);
    let h = heli(&s);
    let weight = s.model().configuration().mass.empty_lbs + s.fuel;
    let rho = rotor::air_density(0.);
    let available = h.available_power(rho, 1., 1.);
    (60..260)
        .map(f64::from)
        .take_while(|kt| {
            h.trim_with(hazards, weight, 0., kt * KT, rho, 1., StabilityLevel::Off)
                .is_some_and(|t| t.engine_power <= available)
        })
        .last()
        .unwrap()
}

/// E2 / H3: power still sets the level top speed. The AH-64 is power-limited
/// either way and is unchanged; the Mi-24's 170 kt is where retreating blade
/// stall ends its trims (power would give 177), so without the stall it flies
/// a little faster, still inside the design's 170 to 181 kt.
#[test]
fn e2_h3_power_still_sets_the_top_speed() {
    let (hazard, easy) = (
        top_speed(Ah64, rotor::Hazards::ALL),
        top_speed(Ah64, rotor::Hazards::NONE),
    );
    assert!((hazard - easy).abs() <= 0.01 * hazard, "{hazard} {easy}");
    let (hazard, easy) = (
        top_speed(Mi24, rotor::Hazards::ALL),
        top_speed(Mi24, rotor::Hazards::NONE),
    );
    assert!(
        easy >= hazard && (170. ..=181.).contains(&easy),
        "{hazard} {easy}"
    );
}

/// E2 / H6: the collective still sets a climb rate, not an acceleration,
/// and full collective still climbs 1,500 to 3,000 ft/min (the Mi-24's
/// 3,400 is the P2 figure). Within 5 percent of the hazard case at the +10
/// percent step (the design's 1 percent: the anti-torque power the cheat
/// spends exactly, the pedals spent roughly).
#[test]
fn e2_h6_the_collective_still_sets_a_climb_rate() {
    for id in HELICOPTERS {
        let climb = |easy: bool| {
            let mut s = helicopter(id, easy, 1_000., 0.);
            let lever = s.lift_controls.collective + 0.1;
            let mut late = 0.;
            fly(&mut s, 120 * 20, |s| {
                late = s.vertical_speed;
                hold(s, lever)
            });
            (s.vertical_speed, (s.vertical_speed - late).abs())
        };
        let (hazard, _) = climb(false);
        let (easy, drift) = climb(true);
        assert!(hazard > 3. && drift < 0.1, "{id:?}");
        assert!(
            (easy / hazard - 1.).abs() < 0.05,
            "{id:?} {easy} against {hazard}"
        );
        let mut full = easy_trimmed(id, 1_000., 0.);
        fly(&mut full, 120 * 20, |s| hold(s, 1.));
        let rate = full.vertical_speed * 60.;
        let window = if id == Ah64 {
            1_500. ..=3_000.
        } else {
            1_500. ..=3_600.
        };
        assert!(
            window.contains(&rate),
            "{id:?} full collective {rate} ft/min"
        );
    }
}

/// E2 / J1b: the loaded jet still cannot hover, and the clean one still
/// can, with the cheat on.
#[test]
fn e2_j1b_a_loaded_jet_still_cannot_hover() {
    for id in JETS {
        let hover = |payload: f64| {
            let mut s = jets::hover(id, 300.);
            s.cheats.easy_physics = true;
            if payload > 0. {
                s.set_payload(payload).unwrap();
            }
            s.lift_controls.drive.engine_output[0] = 33_800.;
            jets::hold_level(&mut s, 1., 120 * 10);
            s.vertical_speed
        };
        assert!(hover(4_000.) < -3., "{id:?} loaded");
        assert!(hover(0.) > -3., "{id:?} clean");
    }
}

/// A trim made under the cheat is a trim under the cheat: hands off, it
/// holds still, and the hazards-on trim of the same aircraft is not (it
/// needs the anti-torque pedal the cheat takes away).
#[test]
fn a_trim_under_the_cheat_holds_a_hover_without_the_anti_torque_pedal() {
    for id in HELICOPTERS {
        let mut s = easy_trimmed(id, 1_000., 0.);
        assert!(s.lift_controls.aids.trim[2].abs() < 0.2, "{id:?}");
        let start = s.position;
        fly(&mut s, 1200, |_| PilotInput::default());
        for (axis, (now, then)) in s.position.iter().zip(start).enumerate() {
            assert!(
                (now - then).abs() < 1.,
                "{id:?} axis {axis}: {}",
                now - then
            );
        }
        assert!(degrees(s.lift_controls.body_rates[2]).abs() < 0.2);
    }
}

/// Fixed-wing flight does not see the cheat: with it on, a conventional
/// aircraft flies a maneuvering sequence bit for bit as it does with it off.
#[test]
fn fixed_wing_flight_is_bit_identical_with_the_cheat() {
    let aircraft = [
        crate::models::variety::tests::synthetic(AircraftId::F16C),
        crate::flight::integration_tests::profile(),
    ];
    for aircraft in aircraft {
        let make = |easy: bool| {
            let mut s = State::new(&aircraft, [0., 5_000., 0.]).unwrap();
            s.enable_research(1).unwrap();
            s.cheats.easy_physics = easy;
            s.speed = 500.;
            s.velocity = [0., 0., 500.];
            s
        };
        let (mut plain, mut easy) = (make(false), make(true));
        for tick in 0..120 * 20 {
            let input = PilotInput {
                pitch: (tick as f64 / 90.).sin() * 0.6,
                roll: (tick as f64 / 70.).cos() * 0.8,
                yaw: 0.1,
                throttle: Some(0.8),
                ..Default::default()
            };
            for s in [&mut plain, &mut easy] {
                s.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
            }
        }
        easy.cheats.easy_physics = false;
        assert_eq!(plain, easy, "{:?}", aircraft.id);
    }
}

/// The wire's exact coding carries the cheat with the flight state: a
/// restored state is equal, has the cheat on, and flies on bit for bit.
#[test]
fn an_exact_round_trip_with_the_cheat_on_continues_bit_for_bit() {
    for id in [Ah64, Mi24, Av8, Yak141] {
        let mut s = if JETS.contains(&id) {
            jets::hover(id, 800.)
        } else {
            easy_trimmed(id, 800., 0.)
        };
        s.cheats.easy_physics = true;
        let input = |tick: usize| PilotInput {
            pitch: (tick as f64 / 90.).sin() * 0.2,
            roll: (tick as f64 / 70.).cos() * 0.1,
            yaw: 0.1,
            collective: Some(0.5 + 0.1 * (tick as f64 / 200.).sin()),
            ..Default::default()
        };
        for tick in 0..240 {
            s.step_surface(&input(tick), |_, _| crate::research::Surface::runway(0.));
        }
        let aircraft = if JETS.contains(&id) {
            jets::fixture(id)
        } else {
            pt_aircraft(id)
        };
        let model = crate::models::AircraftModel::for_aircraft(&aircraft).unwrap();
        let mut writer = tore_codec::BitWriter::new();
        s.write_exact(&mut writer, None).unwrap();
        let mut restored = State::read_exact(
            &mut tore_codec::BitReader::new(writer.as_bytes()),
            None,
            &model,
        )
        .unwrap();
        assert_eq!(s, restored, "{id:?}");
        assert!(restored.cheats.easy_physics, "{id:?}");
        // The cheat is part of the coding: the same state with it off codes
        // to other bytes and restores with it off.
        let mut plain = s.clone();
        plain.cheats.easy_physics = false;
        let mut other = tore_codec::BitWriter::new();
        plain.write_exact(&mut other, None).unwrap();
        assert_ne!(writer.as_bytes(), other.as_bytes(), "{id:?}");
        let off = State::read_exact(
            &mut tore_codec::BitReader::new(other.as_bytes()),
            None,
            &model,
        )
        .unwrap();
        assert!(!off.cheats.easy_physics);
        for tick in 240..1_440 {
            for state in [&mut s, &mut restored] {
                state.step_surface(&input(tick), |_, _| crate::research::Surface::runway(0.));
            }
            assert_eq!(s, restored, "{id:?} tick {tick}");
        }
    }
}

/// Switching the cheat on in flight takes effect at once and does not upset
/// the aircraft: a hazards-on hover stays put through the switch.
#[test]
fn switching_the_cheat_on_in_flight_is_smooth() {
    for id in HELICOPTERS {
        let mut s = trimmed(id, 1_000., 0.);
        fly(&mut s, 240, |_| PilotInput::default());
        s.cheats.easy_physics = true;
        let height = s.position[1];
        fly(&mut s, 240, |_| PilotInput::default());
        assert!(!s.crashed, "{id:?}");
        assert!((s.position[1] - height).abs() < 30., "{id:?}");
        assert!(degrees(s.lift_controls.body_rates[2]).abs() < 10., "{id:?}");
    }
}

/// Collective that holds `height` for the scripted hands-off pilot (the
/// stick, pedals and trim are untouched).
fn height_hold(s: &State, height: f64) -> PilotInput {
    let climb = (0.3 * (height - s.position[1])).clamp(-20., 20.);
    PilotInput {
        collective: Some(
            (s.lift_controls.collective + DT * 0.05 * (climb - s.vertical_speed)).clamp(0., 1.),
        ),
        ..Default::default()
    }
}

/// E1 / H2b with the cheat: the attitude retention. A few taps of forward
/// cyclic trim (10 percent) from a hover, the stick let go and the height
/// held by collective, settle the AH-64 in steady forward flight at 80 to
/// 100 kt and keep it there for 60 s, at Damper and at Off.
#[test]
fn e1_h2b_forward_trim_settles_hands_off_with_the_cheat() {
    for level in [StabilityLevel::Damper, StabilityLevel::Off] {
        let mut s = easy_trimmed(Ah64, 2_000., 0.);
        s.lift_controls.aids.stability = level;
        for _ in 0..2 {
            s.command(PilotCommand::Lift(tore_input::LiftCommand::TrimAdjust(
                tore_input::TrimAxis::Pitch,
                -0.05,
            )));
        }
        fly(&mut s, 120 * 90, |s| height_hold(s, 2_000.));
        let (mut slowest, mut fastest) = (f64::MAX, 0_f64);
        let (mut least, mut most) = (f64::MAX, f64::MIN);
        for _ in 0..60 {
            fly(&mut s, 120, |s| height_hold(s, 2_000.));
            let speed = s.speed / KT;
            slowest = slowest.min(speed);
            fastest = fastest.max(speed);
            least = least.min(s.pitch.to_degrees());
            most = most.max(s.pitch.to_degrees());
        }
        assert!(
            slowest >= 80. && fastest <= 100.,
            "{level:?} {slowest} to {fastest} kt"
        );
        assert!(
            fastest - slowest < 3.,
            "{level:?} still moving {slowest} {fastest}"
        );
        assert!(most - least < 1., "{level:?} pitch {least} to {most}");
        assert!((s.position[1] - 2_000.).abs() < 50., "{level:?}");
        assert!(!s.crashed);
    }
}

/// The retention is weak and about the trim attitude only: it stays under
/// the Damper's 20 percent cap, it adds nothing at the reference, it does not
/// act without the cheat, and it does not hold a drift or hover the aircraft.
#[test]
fn the_attitude_retention_is_weak_and_holds_nothing_else() {
    let sensed = sas::Sensed::default();
    for level in [StabilityLevel::Damper, StabilityLevel::Off] {
        let build = |easy: bool| {
            let mut s = if easy {
                easy_trimmed(Ah64, 1_000., 0.)
            } else {
                trimmed(Ah64, 1_000., 0.)
            };
            s.lift_controls.aids.stability = level;
            s
        };
        let lift = build(true).model().powered_lift().unwrap();
        // Nothing to do at the reference.
        let mut s = build(true);
        let at_rest = s.augment(&lift, [0.; 3], sensed).augmentation;
        assert_eq!(at_rest[..2], [0.; 2], "{level:?}");
        // Far from it, the retention adds at most its cap on pitch and bank,
        // below the Damper's.
        s.pitch += 0.6;
        s.bank -= 0.6;
        let far = s.augment(&lift, [0.; 3], sensed).augmentation;
        assert!(
            far[0].abs() <= sas::EASY_RETENTION_AUTHORITY + 1e-12 && far[0] < 0.,
            "{far:?}"
        );
        assert!(
            far[1].abs() <= sas::EASY_RETENTION_AUTHORITY + 1e-12 && far[1] > 0.,
            "{far:?}"
        );
        const { assert!(sas::EASY_RETENTION_AUTHORITY < sas::DAMPER_AUTHORITY) };
        assert_eq!(far[2], 0., "no heading hold");
        // Without the cheat, the same state gets none of it.
        let mut plain = build(false);
        plain.pitch += 0.6;
        plain.bank -= 0.6;
        let level_out = plain.augment(&lift, [0.; 3], sensed).augmentation;
        if level == StabilityLevel::Off {
            assert_eq!(level_out, [0.; 3]);
        } else {
            assert_eq!(level_out[..2], [0.; 2], "the Damper alone damps rates");
        }
        // A drift is not held: 3 ft/s sideways in a hover is still there
        // after 8 s, and the aircraft has gone somewhere.
        let mut drifting = build(true);
        drifting.velocity[0] += 3.;
        let start = drifting.position;
        fly(&mut drifting, 120 * 8, |_| PilotInput::default());
        let speed = drifting.velocity[0].hypot(drifting.velocity[2]);
        assert!(speed > 1.5, "{level:?} held the drift: {speed} ft/s");
        assert!((drifting.position[0] - start[0]).abs() > 8., "{level:?}");
    }
}

/// With the cheat off at Damper the cyclic trim keys move the cyclic only,
/// as before, and the cheat's retention is not there.
#[test]
fn without_the_cheat_the_trim_keys_move_the_cyclic_only() {
    let mut s = trimmed(Ah64, 1_000., 0.);
    s.lift_controls.aids.stability = StabilityLevel::Damper;
    let reference = s.lift_controls.aids.attitude_reference;
    s.command(PilotCommand::Lift(tore_input::LiftCommand::TrimAdjust(
        tore_input::TrimAxis::Pitch,
        -0.1,
    )));
    assert_eq!(s.lift_controls.aids.attitude_reference, reference);
    assert_eq!(
        s.lift_controls.aids.trim[0],
        s.lift_controls.aids.trim[0].min(-0.1)
    );
    let mut easy = easy_trimmed(Ah64, 1_000., 0.);
    easy.lift_controls.aids.stability = StabilityLevel::Damper;
    let reference = easy.lift_controls.aids.attitude_reference;
    easy.command(PilotCommand::Lift(tore_input::LiftCommand::TrimAdjust(
        tore_input::TrimAxis::Pitch,
        -0.1,
    )));
    let moved = easy.lift_controls.aids.attitude_reference[0] - reference[0];
    assert!((moved.to_degrees() + 5.).abs() < 1e-9, "{moved}");
}

// ---------------------------------------------------------------------
// Slice P8b: the CH-47 and the V-22.

/// The CH-47 and V-22 fixtures and helpers the tandem and tiltrotor tests
/// share with the rest of the crate.
mod heavy {
    pub(super) use super::super::tandem::tests as tandem;
    pub(super) use super::super::tiltrotor::acceptance as tilt;
}

use heavy::{tandem as ch47, tilt as v22};

/// A CH-47 trimmed level at `airspeed_fps` and `height` at stability level
/// `level`, heading north, with the cheat set before the trim (as a start
/// or the trim routine under the cheat would do it) or not.
fn tandem_state(height: f64, airspeed_fps: f64, level: StabilityLevel, easy: bool) -> State {
    let mut s = State::new(&ch47::pt_aircraft(), [0., height, 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.cheats.easy_physics = easy;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.velocity = [0.; 3];
    s.speed = 0.;
    s.lift_controls.aids.stability = level;
    assert!(s.trim_tandem(airspeed_fps), "CH-47 trims");
    s
}

/// A V-22 trimmed level at `airspeed_fps` and `height` with the nacelles at
/// `nacelle_degrees`, the cheat set before the trim or not.
fn tilt_state(
    height: f64,
    airspeed_fps: f64,
    nacelle_degrees: f64,
    level: StabilityLevel,
    easy: bool,
) -> State {
    let mut s = State::new(&v22::pt_v22(), [0., height, 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.cheats.easy_physics = easy;
    s.yaw = 0.;
    s.pitch = 0.;
    s.bank = 0.;
    s.velocity = [0.; 3];
    s.speed = 0.;
    s.gear = 0.;
    s.gear_down = false;
    s.lift_controls.aids.stability = level;
    assert!(
        s.trim_tiltrotor(airspeed_fps, nacelle_degrees),
        "V-22 trims at {airspeed_fps} ft/s, {nacelle_degrees} degrees"
    );
    s
}

/// E1 / H9b for the CH-47 and the V-22 in the hover: an engine cut with the
/// lever held drops the rotor below 70 percent with the hazard, and with the
/// cheat never below 85 percent in flight.
#[test]
fn e1_h9b_the_heavy_rotors_never_fall_below_85_percent_with_the_cheat() {
    type Maker = fn(bool) -> State;
    let cases: [(&str, Maker); 2] = [
        ("CH-47", |easy| {
            tandem_state(3_000., 0., StabilityLevel::Off, easy)
        }),
        ("V-22", |easy| {
            tilt_state(3_000., 0., 87., StabilityLevel::Off, easy)
        }),
    ];
    for (name, make) in cases {
        let lowest = |easy: bool| {
            let mut s = make(easy);
            let held = s.lift_controls.collective;
            s.command(PilotCommand::Set(Switch::Engine, false));
            let mut low: f64 = 1.;
            fly(&mut s, 120 * 8, |s| {
                low = low.min(s.lift_controls.drive.rotor_speed);
                PilotInput {
                    collective: Some(held),
                    ..Default::default()
                }
            });
            assert!(!s.crashed, "{name}");
            low
        };
        assert!(lowest(false) < 0.7, "{name}");
        let held = lowest(true);
        assert!((0.85..0.86).contains(&held), "{name} lever held {held}");
    }
}

/// The CH-47's sink at one hover induced velocity below 10 kt, after a
/// second at the hover collective and then up to `seconds` of full
/// collective with the attitude held: the sink when it began, after 3 s, and
/// the time it took to stop sinking.
fn tandem_vortex_ring(easy: bool, seconds: usize) -> (f64, f64, Option<f64>) {
    let mut s = tandem_state(4_000., 0., StabilityLevel::Off, easy);
    let m = ch47::model(&s);
    let weight = s.model().configuration().mass.empty_lbs + s.fuel;
    let vh = (weight / 2. / (2. * rotor::air_density(4_000.) * m.rotors[0].area_ft2)).sqrt();
    s.velocity[1] = -vh;
    let lever = s.lift_controls.collective;
    fly(&mut s, 120, |s| hold(s, lever));
    let start = s.vertical_speed;
    let mut arrested = None;
    let mut after = start;
    for tick in 0..120 * seconds {
        fly(&mut s, 1, |s| hold(s, 1.));
        if s.vertical_speed >= 0. && arrested.is_none() {
            arrested = Some((tick + 1) as f64 / 120.);
        }
        if tick + 1 == 3 * 120 {
            after = s.vertical_speed;
        }
    }
    (start, after, arrested)
}

/// E1 / H10 for the CH-47: full collective does not arrest the vortex ring
/// sink in 3 s with the hazard; with the cheat it does.
#[test]
fn e1_h10_the_cheat_arrests_the_tandems_vortex_ring_sink() {
    let (start, ring, ring_arrested) = tandem_vortex_ring(false, 8);
    let (easy_start, easy, easy_arrested) = tandem_vortex_ring(true, 8);
    println!("H10 CH-47 {start} {ring} {ring_arrested:?} | {easy_start} {easy} {easy_arrested:?}");
    assert!(start < -15. && easy_start < -15.);
    assert!(ring_arrested.is_none_or(|t| t > 3.), "{ring_arrested:?}");
    let seconds = easy_arrested.expect("never arrested");
    assert!(seconds <= 3., "arrested in {seconds} s");
    assert!(easy > ring + 10., "{easy} against {ring}");
}

/// E1 / H11 for the CH-47: past the never-exceed speed with the stick
/// frozen, the hazard adds nose-up pitch and the cheat does not; the
/// vibration cue stays. (The counter-rotating pair's roll tendencies oppose,
/// so, as in the tandem's own H11, roll is not asked of it.)
#[test]
fn e1_h11_the_tandem_does_not_pitch_up_past_vne_with_the_cheat() {
    let dive = |easy: bool| {
        let mut s = tandem_state(4_000., 120. * KT, StabilityLevel::Off, easy);
        s.cheats.damage = crate::cheats::Damage::Invulnerable;
        s.velocity = [0., 0., 215. * KT];
        s.speed = 215. * KT;
        let pitch = s.pitch;
        let lever = s.lift_controls.collective;
        let mut cue = 0;
        fly(&mut s, 240, |s| {
            cue = cue.max(s.lift_controls.warnings.blade_stall);
            PilotInput {
                collective: Some(lever),
                ..Default::default()
            }
        });
        (degrees(s.pitch - pitch), degrees(s.bank), cue)
    };
    let (stall_pitch, stall_bank, stall_cue) = dive(false);
    let (pitch, bank, cue) = dive(true);
    println!("H11 CH-47 {stall_pitch} {stall_bank} {stall_cue} | {pitch} {bank} {cue}");
    assert!(stall_cue > 0 && cue > 0, "the vibration cue stays");
    assert!(
        pitch < stall_pitch - 1.,
        "pitch {pitch} against {stall_pitch}"
    );
    assert!(bank.abs() <= stall_bank.abs() + 1., "{bank} {stall_bank}");
}

/// E1 / H12 for the CH-47: a 30 percent collective step with the pedals
/// fixed turns the nose less than 1 deg/s with the cheat, at Off and
/// Damper (the two torques cancel with or without it).
#[test]
fn e1_h12_collective_does_not_yaw_the_tandem_with_the_cheat() {
    for level in [StabilityLevel::Off, StabilityLevel::Damper] {
        let swing = |easy: bool| {
            let mut s = tandem_state(3_000., 0., level, easy);
            let lever = s.lift_controls.collective + 0.3;
            let mut fastest: f64 = 0.;
            fly(&mut s, 240, |s| {
                fastest = fastest.max(s.lift_controls.body_rates[2].abs());
                PilotInput {
                    collective: Some(lever),
                    ..Default::default()
                }
            });
            degrees(fastest)
        };
        let (plain, easy) = (swing(false), swing(true));
        println!("H12 CH-47 {level:?} {plain} {easy}");
        assert!(easy < 1., "{level:?} {easy} deg/s");
    }
}

/// A CH-47 on the runway, rotors turning, 20 degrees banked with the
/// collective at 0.8 of the hover lever and the thrust leaning the way it
/// is banked.
fn tandem_banked_on_the_ground(easy: bool) -> State {
    let mut s = State::new(&ch47::pt_aircraft(), [0., 0., 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.cheats.easy_physics = easy;
    s.start_on_runway([0., 0., 0.], 0.).unwrap();
    s.throttle = 1.;
    s.gear_down = true;
    s.gear = 1.;
    fly(&mut s, 120, |_| PilotInput::default());
    assert!(s.weight_on_wheels() && !s.crashed);
    let lever = tandem_state(0., 0., StabilityLevel::Off, false)
        .lift_controls
        .collective;
    s.lift_controls.collective = 0.8 * lever;
    s.lift_controls.collective_actual = 0.8 * lever;
    s.bank = 20_f64.to_radians();
    s.lift_controls.rotors[0].tilt[1] = 0.02;
    s.lift_controls.rotors[1].tilt[1] = 0.02;
    s
}

/// E1 / H14 for the CH-47: 20 degrees of bank under thrust is a crash on
/// the first tick with the hazard and is not with the cheat (the normal
/// contact rules still hold a banked touchdown to the landing limits).
#[test]
fn e1_h14_the_tandem_does_not_roll_over_with_the_cheat() {
    let mut s = tandem_banked_on_the_ground(false);
    fly(&mut s, 1, |_| PilotInput::default());
    assert!(s.crashed, "with the hazard");
    let mut s = tandem_banked_on_the_ground(true);
    fly(&mut s, 30, |_| PilotInput::default());
    assert!(!s.crashed, "with the cheat");
}

/// The V-22 hover at 87 degrees sinking at one hover induced velocity,
/// after a second at the hover lever and then up to `seconds` of full lever
/// with the attitude held: the sink when it began and the time it took to
/// stop sinking.
fn tilt_vortex_ring(easy: bool, seconds: usize) -> (f64, Option<f64>) {
    let mut s = tilt_state(4_000., 0., 87., StabilityLevel::Off, easy);
    let weight = s.model().configuration().mass.empty_lbs + s.fuel;
    let m = tiltrotor::Tiltrotor::new(
        &s.model().powered_lift().unwrap(),
        s.model().configuration(),
    )
    .unwrap();
    let vh = (weight / 2. / (2. * rotor::air_density(4_000.) * m.rotors[0].area_ft2)).sqrt();
    s.velocity[1] = -vh;
    let lever = s.lift_controls.collective;
    fly(&mut s, 120, |s| hold(s, lever));
    let start = s.vertical_speed;
    let mut arrested = None;
    for tick in 0..120 * seconds {
        fly(&mut s, 1, |s| hold(s, 1.));
        if s.vertical_speed >= 0. {
            arrested = Some((tick + 1) as f64 / 120.);
            break;
        }
    }
    (start, arrested)
}

/// E1 / H10 for the V-22 in helicopter mode: the hazard keeps the sink
/// beyond 3 s at full lever, the cheat arrests it.
#[test]
fn e1_h10_the_cheat_arrests_the_tiltrotors_vortex_ring_sink() {
    let (start, ring) = tilt_vortex_ring(false, 8);
    let (easy_start, easy) = tilt_vortex_ring(true, 8);
    println!("H10 V-22 {start} {ring:?} | {easy_start} {easy:?}");
    assert!(start < -15. && easy_start < -15.);
    assert!(ring.is_none_or(|t| t > 3.), "{ring:?}");
    assert!(easy.expect("never arrested") <= 3.);
}

/// E1 / H2b for the CH-47 with the cheat: the attitude retention. Two taps
/// of forward cyclic trim from a hover, the stick let go and the height held
/// by collective, settle in steady forward flight and keep it for 60 s, at
/// Damper and at Off. Without the retention the tandem's weak speed
/// stability would let it wander.
#[test]
fn e1_h2b_the_tandem_settles_hands_off_with_the_cheat() {
    for level in [StabilityLevel::Damper, StabilityLevel::Off] {
        let mut s = tandem_state(2_000., 0., level, true);
        for _ in 0..2 {
            s.command(PilotCommand::Lift(tore_input::LiftCommand::TrimAdjust(
                tore_input::TrimAxis::Pitch,
                -0.05,
            )));
        }
        fly(&mut s, 120 * 90, |s| height_hold(s, 2_000.));
        let (mut slowest, mut fastest) = (f64::MAX, 0_f64);
        let (mut least, mut most) = (f64::MAX, f64::MIN);
        for _ in 0..60 {
            fly(&mut s, 120, |s| height_hold(s, 2_000.));
            let speed = s.speed / KT;
            slowest = slowest.min(speed);
            fastest = fastest.max(speed);
            least = least.min(s.pitch.to_degrees());
            most = most.max(s.pitch.to_degrees());
        }
        println!("H2b CH-47 {level:?} {slowest} to {fastest} kt, pitch {least} to {most}");
        assert!(
            slowest >= 60. && fastest <= 160.,
            "{level:?} {slowest} to {fastest} kt"
        );
        assert!(
            fastest - slowest < 3.,
            "{level:?} still moving {slowest} {fastest}"
        );
        assert!(most - least < 1., "{level:?} pitch {least} to {most}");
        assert!((s.position[1] - 2_000.).abs() < 50., "{level:?}");
        assert!(!s.crashed);
    }
}

/// E2 / H3 for the CH-47: power still sets the level top speed. The Damper
/// trim's top speed is the same with and without the hazards within 1
/// percent (the rotors never reach retreating blade stall below it).
#[test]
fn e2_h3_power_still_sets_the_tandems_top_speed() {
    let top = |hazards: rotor::Hazards| {
        let s = tandem_state(0., 0., StabilityLevel::Damper, false);
        let m = ch47::model(&s);
        let weight = s.model().configuration().mass.empty_lbs + s.fuel;
        let rho = rotor::air_density(0.);
        let available = m.available_power(rho, 1., 1.);
        (60..260)
            .step_by(2)
            .map(f64::from)
            .take_while(|kt| {
                m.trim_with(
                    hazards,
                    weight,
                    0.,
                    kt * KT,
                    rho,
                    1.,
                    StabilityLevel::Damper,
                )
                .is_some_and(|t| t.engine_power <= available)
            })
            .last()
            .unwrap()
    };
    let (hazard, easy) = (top(rotor::Hazards::ALL), top(rotor::Hazards::NONE));
    println!("H3 CH-47 {hazard} {easy}");
    assert!((160. ..=170.).contains(&easy), "{easy}");
    assert!((hazard - easy).abs() <= 0.01 * hazard, "{hazard} {easy}");
}

/// The climb rate, ft/s, after 20 s of `extra` above the hover lever with
/// the attitude held at `pitch` (the V-22 needs a few degrees nose up to
/// stay over one spot), for the CH-47 and the V-22.
fn climb_after(mut s: State, extra: f64, pitch: f64) -> f64 {
    let lever = s.lift_controls.collective + extra;
    fly(&mut s, 120 * 20, |s| hold_pitch(s, lever, pitch));
    s.vertical_speed
}

/// E2 / H6 for the CH-47 and the V-22: the collective still sets a climb
/// rate, the same with the cheat within 2 percent (neither has a torque
/// reaction to take away).
#[test]
fn e2_h6_the_heavy_rotorcrafts_collective_still_sets_a_climb_rate() {
    for (name, plain, easy) in [
        (
            "CH-47",
            climb_after(
                tandem_state(1_000., 0., StabilityLevel::Damper, false),
                0.1,
                0.,
            ),
            climb_after(
                tandem_state(1_000., 0., StabilityLevel::Damper, true),
                0.1,
                0.,
            ),
        ),
        (
            "V-22",
            climb_after(
                tilt_state(1_000., 0., 87., StabilityLevel::Damper, false),
                0.1,
                0.05,
            ),
            climb_after(
                tilt_state(1_000., 0., 87., StabilityLevel::Damper, true),
                0.1,
                0.05,
            ),
        ),
    ] {
        println!("H6 {name} {plain} {easy}");
        assert!(plain > 3., "{name}");
        assert!(
            (easy / plain - 1.).abs() < 0.02,
            "{name} {easy} against {plain}"
        );
    }
}

/// E2 / T4 for the V-22: power still sets the airplane-mode top speed, the
/// same with the cheat within 1 percent (260 to 275 kt).
#[test]
fn e2_t4_power_still_sets_the_tiltrotors_top_speed() {
    let top = |easy: bool| {
        let mut s = State::new(&v22::pt_v22(), [0., 0., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.cheats.easy_physics = easy;
        s.lift_controls.aids.stability = StabilityLevel::Damper;
        (200..330)
            .map(f64::from)
            .take_while(|kt| s.clone().trim_tiltrotor(kt * KT, 0.))
            .last()
            .unwrap_or(0.)
    };
    let (hazard, easy) = (top(false), top(true));
    println!("T4 V-22 {hazard} {easy}");
    assert!((260. ..=275.).contains(&easy), "{easy}");
    assert!((hazard - easy).abs() <= 0.01 * hazard, "{hazard} {easy}");
}

fn true_airspeed(kcas: f64, height: f64) -> f64 {
    kcas * KT / (rotor::air_density(height) / rotor::sea_level_density()).sqrt()
}

/// E1 / T3 with the cheat, side by side with the hazard case: commanding the
/// helicopter preset at 180 KCAS, protection acts exactly as it does without
/// the cheat (the nacelles stop at the upper edge, CONV shows, the demand is
/// kept), and the nacelle paths agree to a degree.
#[test]
fn e1_t3_corridor_protection_acts_the_same_with_the_cheat() {
    let aft = |easy: bool| {
        let mut s = tilt_state(
            3_000.,
            true_airspeed(180., 3_000.),
            0.,
            StabilityLevel::Damper,
            easy,
        );
        let corridor = tiltrotor::Tiltrotor::new(
            &s.model().powered_lift().unwrap(),
            s.model().configuration(),
        )
        .unwrap()
        .tilt
        .corridor;
        let lever = s.lift_controls.collective;
        let mut protecting = false;
        for _ in 0..120 * 10 {
            fly(&mut s, 1, |s| PilotInput {
                collective: Some(lever),
                conversion: Some(87. / 97.5),
                ..hold(s, lever)
            });
            let kcas = tiltrotor::kcas(s.speed, rotor::air_density(s.position[1]));
            let limit = tiltrotor::aft_limit_degrees(corridor, 97.5, kcas);
            assert!(s.nacelle_degrees() <= limit + 0.5, "easy {easy}");
            protecting |= s.lift_controls.corridor_hold.is_some();
        }
        assert!(protecting && s.conversion_corridor().unwrap().protecting);
        assert_eq!(s.lift_controls.conversion, 87. / 97.5, "the demand is kept");
        s.nacelle_degrees()
    };
    let (hazard, easy) = (aft(false), aft(true));
    println!("T3 V-22 aft {hazard} {easy}");
    assert!((hazard - easy).abs() < 1., "{hazard} {easy}");
    assert!(easy < 60., "stopped at the edge, not driven aft: {easy}");
}

/// E1 / T6 with the cheat: a rolling landing with the nacelles left at 45
/// degrees strikes once it slows below 10 kt, cheat or not (the rotor
/// strike is a contact rule, not a hazard).
#[test]
fn e1_t6_the_rotor_strike_still_happens_with_the_cheat() {
    for easy in [false, true] {
        let mut s = State::new(&v22::pt_v22(), [0., 0., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.cheats.unlimited_fuel = true;
        s.cheats.easy_physics = easy;
        s.lift_controls.aids.stability = StabilityLevel::Damper;
        s.start_on_runway([0., 0., 0.], 0.).unwrap();
        s.gear_down = true;
        s.gear = 1.;
        s.throttle = 1.;
        s.lift_controls.conversion = 87. / 97.5;
        s.lift_controls.conversion_actual = 87. / 97.5;
        fly(&mut s, 120, |_| PilotInput {
            collective: Some(0.),
            ..Default::default()
        });
        assert!(s.weight_on_wheels() && !s.crashed, "easy {easy}");
        let speed = 60. * KT;
        s.velocity = [0., 0., speed];
        s.speed = speed;
        s.lift_controls.conversion = 45. / 97.5;
        s.lift_controls.conversion_actual = 45. / 97.5;
        let mut struck_at = None;
        for _ in 0..120 * 60 {
            let before = s.velocity[0].hypot(s.velocity[2]);
            fly(&mut s, 1, |_| PilotInput {
                collective: Some(0.),
                ..Default::default()
            });
            if s.crashed {
                struck_at = Some(before);
                break;
            }
        }
        let struck_at = struck_at.expect("rotor strike") / KT;
        assert!(
            (9. ..10.5).contains(&struck_at),
            "easy {easy}: {struck_at} kt"
        );
    }
}

/// E1 / H9b, the floor itself, with the rotor speed forced low: the floor is
/// 85 percent or the aircraft's rotor speed reference if that is lower, so
/// on the downstops (reference 84 percent) a rotor knocked down to 70 percent
/// with the engines out is lifted to 84, not to 85, and in the hover to 85.
/// Without the cheat nothing lifts it. On the wheels the floor does not
/// apply.
#[test]
fn e1_the_floor_is_the_lower_of_85_percent_and_the_reference() {
    let after_one_tick = |nacelle: f64, speed: f64, easy: bool| {
        let mut s = tilt_state(3_000., speed, nacelle, StabilityLevel::Damper, easy);
        s.command(PilotCommand::Set(Switch::Engine, false));
        let reference = s.lift_controls.drive.rotor_speed_reference;
        s.lift_controls.drive.rotor_speed = 0.70;
        fly(&mut s, 1, |s| hold(s, s.lift_controls.collective));
        (reference, s.lift_controls.drive.rotor_speed)
    };
    let airplane = true_airspeed(200., 3_000.);
    let (reference, floored) = after_one_tick(0., airplane, true);
    assert!((reference - 0.84).abs() < 0.005, "{reference}");
    assert!((floored - reference.min(0.85)).abs() < 1e-9, "{floored}");
    let (_, plain) = after_one_tick(0., airplane, false);
    assert!(plain < 0.72, "{plain}");
    let (reference, floored) = after_one_tick(87., 0., true);
    assert!((reference - 1.).abs() < 1e-9);
    assert!((floored - 0.85).abs() < 1e-9, "{floored}");
    let (_, plain) = after_one_tick(87., 0., false);
    assert!(plain < 0.72, "{plain}");
}

/// What the cheat's attitude retention adds to the pitch and bank
/// augmentation of a V-22 with the nacelles at `nacelle_degrees` and the
/// attitude `error` rad off the reference, at `level`: the same state's
/// augmentation with the cheat on less the one without.
fn retention_added(nacelle_degrees: f64, level: StabilityLevel, error: f64) -> [f64; 2] {
    let sensed = sas::Sensed::default();
    let make = |easy: bool| {
        // Only the stability law is asked, so the nacelles may sit where no
        // hover trim exists.
        let mut s = tilt_state(3_000., 0., 87., level, easy);
        s.lift_controls.conversion = nacelle_degrees / 97.5;
        s.lift_controls.conversion_actual = nacelle_degrees / 97.5;
        s.pitch += error;
        s.bank -= error;
        s
    };
    let lift = make(false).model().powered_lift().unwrap();
    let on = make(true).augment(&lift, [0.; 3], sensed).augmentation;
    let off = make(false).augment(&lift, [0.; 3], sensed).augmentation;
    [on[0] - off[0], on[1] - off[1]]
}

/// The retention follows the V-22's helicopter mode: the full weak
/// retention from 75 degrees up (limited to the cheat's authority, below the
/// Damper's), exactly the helicopter share of it in between, none from 30
/// degrees down. The Attitude level has its own and takes none.
#[test]
fn the_tiltrotors_retention_is_full_in_helicopter_mode_and_fades_to_nothing() {
    for level in [StabilityLevel::Damper, StabilityLevel::Off] {
        // Far off the reference: pinned at the cap, pitch and bank opposed.
        let hover = retention_added(87., level, 0.6);
        assert!(
            (hover[0] + sas::EASY_RETENTION_AUTHORITY).abs() < 1e-9
                && (hover[1] - sas::EASY_RETENTION_AUTHORITY).abs() < 1e-9,
            "{level:?} {hover:?}"
        );
        // A small error, inside the cap: gain times error, scaled by the share.
        let small = 0.05;
        let full = retention_added(87., level, small);
        assert!(
            full[0] < 0.
                && full[1] > 0.
                && full
                    .iter()
                    .all(|v| v.abs() < sas::EASY_RETENTION_AUTHORITY - 0.01),
            "{level:?} {full:?}"
        );
        let at_75 = retention_added(75., level, small);
        assert!(
            (at_75[0] - full[0]).abs() < 1e-12 && (at_75[1] - full[1]).abs() < 1e-12,
            "{level:?} full at 75: {at_75:?}"
        );
        for degrees in [40., 52., 65.] {
            let share = tiltrotor::helicopter_share(degrees);
            assert!(share > 0. && share < 1., "{degrees}");
            let part = retention_added(degrees, level, small);
            assert!(
                (part[0] - share * full[0]).abs() < 1e-9
                    && (part[1] - share * full[1]).abs() < 1e-9,
                "{level:?} {degrees}: {part:?} against {share} of {full:?}"
            );
        }
        assert_eq!(
            retention_added(30., level, small),
            [0.; 2],
            "{level:?} none at 30"
        );
        assert_eq!(
            retention_added(0., level, 0.6),
            [0.; 2],
            "{level:?} none in airplane mode"
        );
    }
    assert_eq!(retention_added(87., StabilityLevel::Attitude, 0.6), [0.; 2]);
}

/// In helicopter mode the retention pulls a disturbed attitude back toward
/// the trim: a 10 degree nose-up, hands off at Off, is smaller 4 s later
/// with the cheat than without. In airplane mode the cheat changes nothing
/// about how the aircraft flies the same disturbance.
#[test]
fn the_retention_pulls_the_tiltrotor_back_in_helicopter_mode_only() {
    let error_after = |nacelle: f64, speed: f64, easy: bool| {
        let mut s = tilt_state(3_000., speed, nacelle, StabilityLevel::Off, easy);
        s.pitch += 10_f64.to_radians();
        s.bank += 10_f64.to_radians();
        let lever = s.lift_controls.collective;
        let (pitch, bank) = (s.pitch - 10_f64.to_radians(), s.bank - 10_f64.to_radians());
        fly(&mut s, 120 * 4, |_| PilotInput {
            collective: Some(lever),
            ..Default::default()
        });
        degrees((s.pitch - pitch).abs().max((s.bank - bank).abs()))
    };
    let (plain, easy) = (error_after(87., 0., false), error_after(87., 0., true));
    println!("retention V-22 hover {plain} {easy}");
    assert!(easy < plain - 1., "helicopter mode {easy} against {plain}");
    let speed = true_airspeed(200., 3_000.);
    let (plain, easy) = (error_after(0., speed, false), error_after(0., speed, true));
    println!("retention V-22 airplane {plain} {easy}");
    assert!(
        (easy - plain).abs() < 0.1,
        "airplane mode {easy} against {plain}"
    );
}

/// A V-22 mid-conversion with the cheat on (protection holding the
/// nacelles) and one in a hover restore exactly from the wire's exact coding
/// and fly on bit for bit; the cheat is part of the coding.
#[test]
fn a_tiltrotor_with_the_cheat_restores_exactly_mid_flight() {
    let mut converting = tilt_state(1_000., 0., 87., StabilityLevel::Damper, true);
    let lever = converting.lift_controls.collective;
    fly(&mut converting, 120 * 6, |s| PilotInput {
        conversion_rate: -1.,
        pitch: -0.15,
        collective: Some(lever),
        ..hold(s, lever)
    });
    assert!((10. ..85.).contains(&converting.nacelle_degrees()));
    let hovering = tilt_state(800., 0., 87., StabilityLevel::Off, true);
    let model = crate::models::AircraftModel::for_aircraft(&v22::pt_v22()).unwrap();
    for s in [converting, hovering] {
        let mut writer = tore_codec::BitWriter::new();
        s.write_exact(&mut writer, None).unwrap();
        let mut restored = State::read_exact(
            &mut tore_codec::BitReader::new(writer.as_bytes()),
            None,
            &model,
        )
        .unwrap();
        assert_eq!(s, restored);
        assert!(restored.cheats.easy_physics);
        let mut plain = s.clone();
        plain.cheats.easy_physics = false;
        let mut other = tore_codec::BitWriter::new();
        plain.write_exact(&mut other, None).unwrap();
        assert_ne!(writer.as_bytes(), other.as_bytes());
        let mut original = s.clone();
        for tick in 0..1_200 {
            let input = PilotInput {
                pitch: (tick as f64 / 90.).sin() * 0.2,
                roll: (tick as f64 / 70.).cos() * 0.1,
                yaw: 0.1,
                collective: Some(0.6 + 0.2 * (tick as f64 / 200.).sin()),
                conversion_rate: if tick < 600 { -1. } else { 1. },
                ..Default::default()
            };
            for state in [&mut original, &mut restored] {
                state.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
            }
            assert_eq!(original, restored, "tick {tick}");
        }
    }
}

/// A CH-47 mid-hover and mid-autorotation with the cheat on restores
/// exactly from the wire's exact coding and flies on bit for bit; the cheat
/// is part of the coding.
#[test]
fn a_tandem_with_the_cheat_restores_exactly_mid_flight() {
    let mut hovering = tandem_state(800., 0., StabilityLevel::Damper, true);
    fly(&mut hovering, 120, |s| PilotInput {
        roll: 0.1,
        yaw: -0.2,
        collective: Some(s.lift_controls.collective + 0.05),
        ..Default::default()
    });
    let mut autorotating = tandem_state(2_000., 80. * KT, StabilityLevel::Damper, true);
    autorotating.command(PilotCommand::Set(Switch::Engine, false));
    fly(&mut autorotating, 360, |_| PilotInput {
        collective: Some(0.1),
        pitch: 0.05,
        ..Default::default()
    });
    assert!(autorotating.lift_controls.drive.engine_output[0] == 0.);
    let model = crate::models::AircraftModel::for_aircraft(&ch47::pt_aircraft()).unwrap();
    for s in [hovering, autorotating] {
        let mut writer = tore_codec::BitWriter::new();
        s.write_exact(&mut writer, None).unwrap();
        let mut restored = State::read_exact(
            &mut tore_codec::BitReader::new(writer.as_bytes()),
            None,
            &model,
        )
        .unwrap();
        assert_eq!(s, restored);
        assert!(restored.cheats.easy_physics);
        let mut plain = s.clone();
        plain.cheats.easy_physics = false;
        let mut other = tore_codec::BitWriter::new();
        plain.write_exact(&mut other, None).unwrap();
        assert_ne!(writer.as_bytes(), other.as_bytes());
        let mut original = s.clone();
        for tick in 0..1_200 {
            let input = PilotInput {
                pitch: (tick as f64 / 90.).sin() * 0.2,
                roll: (tick as f64 / 70.).cos() * 0.1,
                yaw: 0.1,
                collective: Some(0.3 + 0.2 * (tick as f64 / 200.).sin()),
                ..Default::default()
            };
            for state in [&mut original, &mut restored] {
                state.step_surface(&input, |_, _| crate::research::Surface::runway(0.));
            }
            assert_eq!(original, restored, "tick {tick}");
        }
    }
}

/// A V-22 on the runway, nacelles at the helicopter preset, rotors turning,
/// 20 degrees banked with the lever at 0.8 of the hover lever and the thrust
/// leaning the way it is banked.
fn tilt_banked_on_the_ground(easy: bool) -> State {
    let mut s = State::new(&v22::pt_v22(), [0., 0., 0.]).unwrap();
    s.enable_research(1).unwrap();
    s.cheats.unlimited_fuel = true;
    s.cheats.easy_physics = easy;
    s.lift_controls.aids.stability = StabilityLevel::Off;
    s.start_on_runway([0., 0., 0.], 0.).unwrap();
    s.gear_down = true;
    s.gear = 1.;
    s.throttle = 1.;
    s.lift_controls.conversion = 87. / 97.5;
    s.lift_controls.conversion_actual = 87. / 97.5;
    fly(&mut s, 120, |_| PilotInput {
        collective: Some(0.),
        ..Default::default()
    });
    assert!(s.weight_on_wheels() && !s.crashed);
    let lever = tilt_state(0., 0., 87., StabilityLevel::Off, false)
        .lift_controls
        .collective;
    s.lift_controls.collective = 0.8 * lever;
    s.lift_controls.collective_actual = 0.8 * lever;
    s.bank = 20_f64.to_radians();
    s.lift_controls.rotors[0].tilt[1] = 0.02;
    s.lift_controls.rotors[1].tilt[1] = 0.02;
    s
}

/// E1 / H14 for the V-22: dynamic rollover is gated by the cheat. 20 degrees
/// of bank under thrust is a crash on the first tick with the hazard and is
/// not with the cheat.
#[test]
fn e1_h14_the_tiltrotor_does_not_roll_over_with_the_cheat() {
    let mut s = tilt_banked_on_the_ground(false);
    fly(&mut s, 1, |s| PilotInput {
        collective: Some(s.lift_controls.collective),
        ..Default::default()
    });
    assert!(s.crashed, "with the hazard");
    let mut s = tilt_banked_on_the_ground(true);
    fly(&mut s, 30, |s| PilotInput {
        collective: Some(s.lift_controls.collective),
        ..Default::default()
    });
    assert!(!s.crashed, "with the cheat");
}
