//! The AI's rotorcraft and jet-lift control (slice H1 of the helicopter and
//! VTOL AI pass) on the synthetic records carrying the PT numbers the other
//! powered-lift tests use, flown by the scripted flights of
//! [`crate::ai::rotor_probe`]: hover holds in calm air and a wind, a moved
//! point, the vertical climb, forward flight with climbs and descents,
//! slowing to a hover, the V-22's conversion both ways, the jets' hover and
//! vertical descent, determinism and a checkpoint round trip mid-hover.

use super::hover_hold_tests::aircraft;
use crate::ai::jet_lift::JetLift;
use crate::ai::rotor_adapter::{RotorAdapter, RotorRequest};
use crate::ai::rotor_probe::{self, KT, WIND_15_KT};
use tore_formats::aircraft::AircraftId;

const ROTORCRAFT: [AircraftId; 4] = [
    AircraftId::Ah64,
    AircraftId::Mi24,
    AircraftId::Ch47,
    AircraftId::V22,
];
const JETS: [AircraftId; 2] = [AircraftId::Av8, AircraftId::Yak141];

/// Holds a commanded point within 10 ft for 60 s, and the height within
/// 10 ft, in calm air and in a 15 kt wind across the heading.
#[test]
fn the_ai_holds_a_commanded_point_in_calm_air_and_a_wind() {
    for id in ROTORCRAFT {
        for wind in [[0.; 3], WIND_15_KT] {
            let h = rotor_probe::hold(&aircraft(id), wind).unwrap();
            assert!(
                !h.crashed && h.hovering && h.off_ft < 10. && h.height_ft < 10.,
                "{id:?} {wind:?}: {h:?}"
            );
        }
    }
}

/// The point moved 500 ft (diagonally across the heading): it arrives and
/// holds without overshooting it by more than 10 ft.
#[test]
fn the_ai_moves_to_a_new_point_without_overshoot() {
    for id in ROTORCRAFT {
        let m = rotor_probe::move_point(&aircraft(id)).unwrap();
        assert!(!m.crashed && m.overshoot_ft < 10., "{id:?}: {m:?}");
        assert!(m.arrived_s.is_some() && m.off_ft < 10., "{id:?}: {m:?}");
        assert!(m.height_ft < 15., "{id:?}: {m:?}");
    }
}

/// A vertical climb from 20 ft to 500 ft at retail's 25 ft/s: the climb
/// reaches 25 ft/s within 2, the height settles within 10 ft without
/// overshooting 15, and it stays within 30 ft of the point.
#[test]
fn the_ai_climbs_vertically_at_twenty_five_feet_a_second() {
    for id in ROTORCRAFT {
        let c = rotor_probe::vertical_climb(&aircraft(id)).unwrap();
        assert!(
            !c.crashed && (23. ..27.).contains(&c.fastest_fps),
            "{id:?}: {c:?}"
        );
        assert!(
            c.top_ft < 515. && (c.end_ft - 500.).abs() < 10.,
            "{id:?}: {c:?}"
        );
        assert!(c.drift_ft < 30., "{id:?}: {c:?}");
    }
}

/// Forward flight at 40, 80 and 120 kt, climbing and descending at
/// 1,000 ft/min: over the last 30 s of a minute the climb is within
/// 150 ft/min, the speed within 5 kt and the heading within 3 degrees.
#[test]
fn the_ai_flies_forty_eighty_and_a_hundred_twenty_knots_climbing_and_descending() {
    for id in ROTORCRAFT {
        for knots in [40., 80., 120.] {
            for fpm in [1_000., -1_000.] {
                let f = rotor_probe::forward(&aircraft(id), knots, fpm).unwrap();
                let case = format!("{id:?} {knots} kt {fpm} fpm: {f:?}");
                assert!(!f.crashed && (f.climb_fpm - fpm).abs() < 150., "{case}");
                assert!(f.speed_off_kt < 5. && f.heading_off_deg < 3., "{case}");
            }
        }
    }
}

/// A 180 degree turn at 200 ft asking for 160 kt, more than these
/// helicopters make (the hold-off circle and the break ask for the maximum):
/// it turns round losing under 50 ft and does not touch the ground.
#[test]
fn the_ai_turns_low_at_full_speed_without_sinking() {
    for id in ROTORCRAFT {
        let t = rotor_probe::low_turn(&aircraft(id), 160.).unwrap();
        assert!(!t.crashed && t.lost_ft < 50., "{id:?}: {t:?}");
        assert!(t.heading_off_deg < 5., "{id:?}: {t:?}");
    }
}

/// Slowing from 120 kt to a hover over a point 3,000 ft ahead: it loses
/// under 50 ft on the way and ends holding the point.
#[test]
fn the_ai_slows_from_a_hundred_twenty_knots_to_a_hover() {
    for id in ROTORCRAFT {
        let w = rotor_probe::slow_to_hover(&aircraft(id)).unwrap();
        assert!(!w.crashed && w.hovering && w.lost_ft < 50., "{id:?}: {w:?}");
        assert!(w.off_ft < 10. && w.ground_speed_kt < 2., "{id:?}: {w:?}");
    }
}

/// The V-22 converts from a hover to airplane mode on its schedule as it
/// accelerates toward 220 kt, and back to a hover, without the corridor
/// protection ever holding it, within 300 ft of its height.
#[test]
fn the_v22_converts_both_ways() {
    let c = rotor_probe::convert(&aircraft(AircraftId::V22)).unwrap();
    assert!(
        !c.crashed && c.out_nacelle_deg < 1. && c.out_kt > 200.,
        "{c:?}"
    );
    assert!(
        c.hovering && c.back_nacelle_deg > 80. && c.back_off_ft < 10.,
        "{c:?}"
    );
    assert!(c.height_ft < 300. && c.corridor_ticks == 0, "{c:?}");
}

/// The AV-8 and Yak-141 hold a hover over a point within 10 ft for 60 s
/// and descend vertically onto the ground at 5 ft/s without a crash.
#[test]
fn the_jets_hold_a_hover_and_descend_vertically() {
    for id in JETS {
        let j = rotor_probe::jet_hover(&aircraft(id)).unwrap();
        assert!(
            !j.crashed && j.off_ft < 10. && j.height_ft < 10.,
            "{id:?}: {j:?}"
        );
        assert!(
            j.touchdown_fps.is_some_and(|v| v > -8.) && j.landing_off_ft < 15.,
            "{id:?}: {j:?}"
        );
    }
}

/// The same requests fly bit for bit the same twice, and a rotor adapter
/// (and a jet-lift law) restored from a checkpoint mid-hover flies on bit
/// for bit.
#[test]
fn the_rotor_adapter_is_deterministic_and_restores_mid_hover() {
    let wind = [10. * KT, 0., -6. * KT];
    let ah64 = aircraft(AircraftId::Ah64);
    let run = || {
        let mut s = rotor_probe::hovering(&ah64, 100., wind).unwrap();
        let mut adapter = RotorAdapter::new();
        let mut guidance = rotor_probe::hold_here(&s, 150.);
        guidance.point[0] += 120.;
        for _ in 0..120 * 10 {
            rotor_probe::tick(&mut s, &mut adapter, RotorRequest::Hover(guidance), wind);
        }
        (s, adapter, guidance)
    };
    let (a, adapter_a, guidance) = run();
    let (b, adapter_b, _) = run();
    assert_eq!(a, b);
    assert_eq!(adapter_a, adapter_b);
    let models = crate::checkpoint::Models::default();
    let mut restored = crate::checkpoint::round_trip(&adapter_a, &models).unwrap();
    assert_eq!(restored, adapter_a);
    let (mut original, mut copy) = (a.clone(), a);
    let mut adapter = adapter_a;
    for _ in 0..1_200 {
        rotor_probe::tick(
            &mut original,
            &mut adapter,
            RotorRequest::Hover(guidance),
            wind,
        );
        rotor_probe::tick(
            &mut copy,
            &mut restored,
            RotorRequest::Hover(guidance),
            wind,
        );
        assert_eq!(copy, original);
    }
    let av8 = aircraft(AircraftId::Av8);
    let jet = rotor_probe::hovering(&av8, 300., [0.; 3]).unwrap();
    let mut law = JetLift::new();
    let _ = law.controls(&jet, &rotor_probe::hold_here(&jet, 290.), 0.);
    assert_eq!(crate::checkpoint::round_trip(&law, &models).unwrap(), law);
}

/// The probe reports every flight for each rotorcraft and a line for each
/// jet.
#[test]
fn the_rotor_probe_reports_each_powered_lift_aircraft() {
    for id in ROTORCRAFT {
        let lines = rotor_probe::report(&aircraft(id));
        let expected = if id == AircraftId::V22 { 13 } else { 12 };
        assert_eq!(lines.len(), expected, "{id:?}: {lines:#?}");
        assert!(lines.iter().all(|l| !l.contains("CRASHED")), "{lines:#?}");
    }
    for id in JETS {
        let lines = rotor_probe::report(&aircraft(id));
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("touchdown=-"), "{lines:?}");
    }
}
