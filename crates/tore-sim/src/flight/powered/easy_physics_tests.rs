//! Acceptance tests E1 and E2 of the VTOL overhaul design (section 10) for
//! the Easy flight physics cheat (slice P8), on the AH-64, Mi-24, AV-8 and
//! Yak-141 (the CH-47 and V-22 join when their slices merge), plus the exact
//! state round trip with the cheat on and the proof that fixed-wing flight
//! does not notice it. The menu and session rules (E3) are tested where they
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
    let [p, q, r] = s.lift_controls.body_rates;
    PilotInput {
        pitch: (2.5 * (0. - s.pitch) - 1.2 * q).clamp(-1., 1.),
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
