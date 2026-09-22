use super::*;
fn advance(s: &mut Systems, ticks: usize, throttle: f64) {
    let mut fuel = 1000.;
    for _ in 0..ticks {
        s.advance(true, throttle, 1., 0., false, &mut fuel);
    }
}
#[test]
fn healthy_values_and_external_mass_debits_are_exact() {
    let mut s = Systems::new(2, [100., 50., 0., 0., 0., 0., 0., 0., 0.]);
    advance(&mut s, 1200, 1.);
    assert_eq!(
        (
            s.engine.temperature,
            s.oil_pressure(),
            s.fluids.hydraulic,
            s.engine.power
        ),
        (0., 1., 1., 1.)
    );
    let mut internal = 500.;
    s.consume(&mut internal, 125.);
    assert_eq!(
        (internal, s.external_lbs(), s.used_external_lbs()),
        (500., 25., 125.)
    );
    s.consume(&mut internal, 50.);
    assert_eq!((internal, s.external_lbs()), (475., 0.));
    assert!(s.messages.is_empty());
}
#[test]
fn leaks_drive_gauges_heat_controls_and_pause_does_not_advance() {
    let mut high = Systems::default();
    for index in [12, 13, 14] {
        high.hit(index, 1.);
    }
    let mut low = high.clone();
    advance(&mut high, 1200, 1.);
    advance(&mut low, 1200, 0.1);
    assert!((high.fluids.oil - 0.9).abs() < 1e-10);
    assert!((high.fluids.hydraulic - 0.9).abs() < 1e-10);
    assert!((high.oil_pressure() - 0.45).abs() < 1e-10);
    assert!(high.engine.temperature > low.engine.temperature * 5.);
    let paused = high.clone();
    for _ in 0..100 {
        let _ = high.summary(0.25);
    }
    assert_eq!(high, paused);
    advance(&mut high, 12000, 1.);
    assert_eq!(high.fluids.hydraulic, 0.);
    assert_eq!(high.fluids.oil, 0.);
    assert_eq!(high.power_available(), 0.);
    assert_eq!(high.controls([1.; 3], [0.1, 0.2, 0.3], 0), [0.1, 0.2, 0.3]);
    assert!(!high.device_free(16));
}
#[test]
fn compressor_boundary_engine_loss_and_afterburner_identity() {
    let mut safe = Systems::default();
    safe.hit(7, 0.25);
    let mut unsafe_throttle = safe.clone();
    advance(&mut safe, 3601, 0.25);
    advance(&mut unsafe_throttle, 3601, 0.251);
    assert_eq!(safe.power_available(), 0.75);
    assert_eq!(unsafe_throttle.power_available(), 0.);
    let mut twin = Systems::new(2, [0.; 9]);
    twin.hit(9, 1.);
    assert_eq!(twin.power_available(), 0.5);
    twin.hit(10, 1.);
    assert_eq!(twin.power_available(), 0.);
    twin.hit(8, 1.);
    assert!(twin.has(8));
}
#[test]
fn flameout_needs_throttle_cycle_and_fire_and_wounds_expire_once() {
    let mut s = Systems::default();
    s.hit(4, 1.);
    advance(&mut s, 800, 1.);
    assert!(s.engine.flameout > 0.);
    advance(&mut s, 1, 0.);
    advance(&mut s, 1, 0.5);
    assert_eq!(s.engine.flameout, 0.);
    for (fault, seconds) in [(3, 10), (11, 10), (15, 10), (35, 5)] {
        let mut fire = Systems::default();
        fire.hit(fault, 1.);
        advance(&mut fire, seconds * 120 - 1, 1.);
        assert!(!fire.fatal());
        advance(&mut fire, 2, 1.);
        assert!(fire.fatal());
        let count = fire.messages.len();
        advance(&mut fire, 1200, 1.);
        assert_eq!(count, fire.messages.len());
    }
    let mut pilot = Systems::default();
    pilot.hit(34, 1.);
    pilot.hit(34, 1.);
    assert_eq!(pilot.pilot.remaining, Some(450.));
    advance(&mut pilot, 450 * 120 + 1, 0.);
    assert!(pilot.fatal());
    let mut landed = Systems::default();
    landed.hit(34, 1.);
    landed.advance(true, 0., 1., 0., true, &mut 100.);
    assert_eq!(landed.pilot.remaining, None);
}
#[test]
fn every_dispatch_identity_is_bounded_and_structural_damage_is_load_sensitive() {
    for fault in 0..45 {
        let mut s = Systems::default();
        s.hit(fault, 0.63);
        assert_eq!(s.counts[fault], 1);
        if fault < 36 {
            assert_eq!(s.messages[0], label(fault));
        }
        if (19..=28).contains(&fault) {
            assert!(!s.autopilot_available());
        }
        if fault == 29 {
            assert_eq!(s.controls.throttle_lock, Some(0.63));
        }
    }
    let mut s = Systems::default();
    s.hit(30, 1.);
    for _ in 0..1000 {
        s.advance(true, 1., 4., 0.5, false, &mut 100.);
    }
    assert!(!s.fatal());
    for _ in 0..241 {
        s.advance(true, 1., 5., 0.5, false, &mut 100.);
    }
    assert!(s.fatal());
}

#[test]
fn component_damage_is_local_until_explicitly_coupled() {
    let mut fluids = Fluids::default();
    let mut engine = Engine::new(2);
    fluids.hit(12);
    assert_eq!(fluids.oil_pressure(), 0.5);
    assert_eq!(engine.temperature, 0.);
    for _ in 0..1200 {
        engine.advance(true, 1., fluids.oil_pressure(), false);
    }
    assert!((engine.temperature - 30.5).abs() < 1e-9);
    let mut pilot = Pilot::default();
    pilot.hit(34);
    assert_eq!(pilot.remaining, Some(900.));
    assert_eq!(engine.power, 1.);
    let mut controls = Controls::default();
    controls.hit(19, 1.);
    assert_eq!(controls.response([1.; 3], [0.; 3], 0, 1.), [0.5, 1., 1.]);
    assert_eq!(fluids.hydraulic, 1.);
}
#[test]
fn partial_wing_loss_has_lift_roll_drag_and_directional_penalties() {
    let left = regional_effects([0., 0., 0., 0.75, 0., 0.]);
    let right = regional_effects([0., 0., 0., 0., 0.75, 0.]);
    assert!((left.authority[1] - 0.55).abs() < 1e-9);
    assert!((left.lift - 0.7375).abs() < 1e-9);
    assert_eq!(left.drag_percent, 18.75);
    assert_eq!(left.roll_bias, -right.roll_bias);
    assert!(left.commands([0.; 3])[1] < 0.);
    assert!(right.commands([0.; 3])[1] > 0.);
    assert_eq!(damage_percent(0.92), 92);
    assert_eq!(damage_percent(0.999), 99);
    assert_eq!(damage_percent(1.), 100);
}

#[test]
fn instant_pilot_death_is_permanent_and_does_not_destroy_surviving_engines() {
    let mut s = Systems::new(2, [0.; 9]);
    s.hit(34, 0.7);
    s.kill_pilot("Pilot killed: nose lost");
    let messages = s.messages.len();
    s.kill_pilot("Pilot killed: nose lost");
    assert!(s.pilot.dead && s.fatal());
    assert_eq!(s.power_available(), 1.);
    assert_eq!(s.pilot.remaining, None);
    assert_eq!(s.messages.len(), messages);
    assert!(s.pilot.advance(true).is_empty());
    assert!(s.pilot.dead);
}
