//! Tab, the gun key (docs/spec/air-to-ground.md, "The gun key"): the gun
//! fires whatever store is selected, with the trigger's own rules.
use super::tests::fixture;
use super::*;

fn launcher() -> Launcher {
    Launcher {
        position: [0., 3000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 300.,
        velocity: [0., 0., 300.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    }
}

/// An M61 in station 0 (internal, 500 rounds) and two Mk 82s in station 1
/// (external, a bomb), the bomb selected.
fn gun_and_bomb() -> State {
    let mut config = fixture(false).own().configuration().clone();
    let mut gun = config.stations[0].clone();
    gun.weapon.source = "M61.JT".into();
    gun.weapon.burst.actual_rounds_per_game = 2;
    gun.weapon.burst.game_rounds_in_burst = 4;
    gun.weapon.burst.game_burst_t = 1;
    gun.count = 500;
    gun.internal = true;
    let mut bomb = config.stations[0].clone();
    bomb.weapon.source = "MK82.JT".into();
    bomb.weapon.flags = 0x22012;
    bomb.weapon.movement.minimum_speed = 0;
    bomb.weapon.movement.maximum_speed = 1466;
    bomb.weapon.movement.initial_speed = 0;
    bomb.weapon.movement.final_speed = 0;
    bomb.weapon.movement.launch_retard = 100;
    bomb.weapon.movement.remove_t = 240;
    bomb.weapon.burst.actual_rounds_per_game = 1;
    bomb.weapon.burst.game_rounds_in_burst = 1;
    bomb.weapon.burst.game_burst_t = 0;
    bomb.weapon.seeker.signature = 0;
    bomb.count = 2;
    bomb.internal = false;
    config.stations = vec![gun, bomb];
    let mut s = State::new(config, true).unwrap();
    s.own_mut().selected = 1;
    s
}

fn step(s: &mut State, held: bool) -> Vec<Event> {
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held,
            launcher: launcher(),
        }],
        |_, _| -10_000.,
    )
}

fn shots_from(events: &[Event], station: usize) -> usize {
    events
        .iter()
        .filter(|e| matches!(e, Event::Fired { aircraft: 0, station: s } if *s == station))
        .count()
}

/// Steps `ticks` and returns the (gun, bomb) rounds fired.
fn run(s: &mut State, held: bool, ticks: usize) -> (usize, usize) {
    let mut totals = (0, 0);
    for _ in 0..ticks {
        let events = step(s, held);
        totals.0 += shots_from(&events, 0);
        totals.1 += shots_from(&events, 1);
    }
    totals
}

#[test]
fn tab_fires_the_gun_while_a_bomb_is_selected_and_leaves_the_selection_alone() {
    let mut s = gun_and_bomb();
    assert_eq!(run(&mut s, false, 30), (0, 0), "nothing fires unasked");
    s.command(0, Command::GunKeyDown, launcher());
    let (gun, bomb) = run(&mut s, false, 120);
    assert!(gun > 0, "the gun fired");
    assert_eq!(bomb, 0, "the bomb stayed on its pylon");
    assert_eq!(s.own().selected, 1, "the selection did not move");
    assert_eq!(s.own().rounds(1), 2, "no bomb left");
    assert_eq!(usize::from(s.own().rounds(0)), 500 - gun, "ammo depleted");
    assert!(s.projectiles.iter().all(|p| p.station == 0));
    // Tab up: it stops.
    s.command(0, Command::GunKeyUp, launcher());
    assert_eq!(run(&mut s, false, 120), (0, 0));
}

#[test]
fn tab_fires_at_the_rate_of_the_selected_gun_on_the_trigger() {
    let mut selected = gun_and_bomb();
    selected.own_mut().selected = 0;
    let trigger = run(&mut selected, true, 120);
    let mut other = gun_and_bomb();
    other.command(0, Command::GunKeyDown, launcher());
    let tab = run(&mut other, false, 120);
    assert!(trigger.0 > 20, "{trigger:?}");
    assert_eq!(tab, trigger, "same rounds in the same two seconds");
    assert_eq!(selected.own().rounds(0), other.own().rounds(0));
    assert_eq!(
        selected.projectiles.iter().filter(|p| p.tracer).count(),
        other.projectiles.iter().filter(|p| p.tracer).count(),
        "same tracers"
    );
}

#[test]
fn tab_with_the_gun_selected_does_not_double_the_rate() {
    let mut a = gun_and_bomb();
    a.own_mut().selected = 0;
    let trigger = run(&mut a, true, 120);
    let mut b = gun_and_bomb();
    b.own_mut().selected = 0;
    b.command(0, Command::GunKeyDown, launcher());
    let both = run(&mut b, true, 120);
    assert_eq!(both, trigger);
}

#[test]
fn the_trigger_still_drops_the_selected_bomb_while_tab_fires_the_gun() {
    let mut s = gun_and_bomb();
    s.command(0, Command::GunKeyDown, launcher());
    let (gun, bomb) = run(&mut s, true, 60);
    assert!(gun > 0);
    assert_eq!(bomb, 1, "one bomb a press");
    assert_eq!(s.own().rounds(1), 1);
    assert_eq!(s.own().selected, 1);
}

#[test]
fn tab_is_held_back_as_the_trigger_is() {
    // Disarmed (NAV): nothing fires.
    let mut s = gun_and_bomb();
    s.own_mut().armed = false;
    s.command(0, Command::GunKeyDown, launcher());
    assert_eq!(run(&mut s, false, 60), (0, 0));
    // Dry gun: nothing fires and nothing breaks.
    let mut s = gun_and_bomb();
    s.own_mut().ammo[0] = 0;
    s.command(0, Command::GunKeyDown, launcher());
    assert_eq!(run(&mut s, false, 60), (0, 0));
    // A lost launcher: nothing fires.
    let mut s = gun_and_bomb();
    s.command(0, Command::GunKeyDown, launcher());
    let dead = Launcher {
        alive: false,
        ..launcher()
    };
    let events = s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: dead,
        }],
        |_, _| -10_000.,
    );
    assert_eq!(shots_from(&events, 0), 0);
}

#[test]
fn a_released_trigger_lets_go_of_tab_and_the_bay_request_is_left_alone() {
    let mut s = gun_and_bomb();
    s.command(0, Command::GunKeyDown, launcher());
    assert!(run(&mut s, false, 30).0 > 0);
    s.release(0);
    assert_eq!(run(&mut s, false, 60), (0, 0), "the release lets go of Tab");
    // A bomb waiting on closed bay doors keeps waiting while Tab fires.
    let mut s = gun_and_bomb();
    let closed = Launcher {
        bay_ready: false,
        ..launcher()
    };
    s.command(0, Command::GunKeyDown, closed);
    let mut fired = (0, 0);
    for tick in 0..30 {
        let events = s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: tick == 0,
                launcher: closed,
            }],
            |_, _| -10_000.,
        );
        fired.0 += shots_from(&events, 0);
        fired.1 += shots_from(&events, 1);
    }
    assert!(fired.0 > 0, "the gun fires through closed doors");
    assert_eq!(fired.1, 0);
    assert!(
        s.own().bay_demand(s.tick),
        "the bomb's request still stands"
    );
}

#[test]
fn what_the_selected_store_shows_is_not_changed_by_the_gun_pass() {
    let mut a = gun_and_bomb();
    let mut b = gun_and_bomb();
    b.command(0, Command::GunKeyDown, launcher());
    for _ in 0..30 {
        step(&mut a, false);
        step(&mut b, false);
    }
    assert_eq!(a.own().release_readiness, b.own().release_readiness);
    assert_eq!(a.own().launch_mode, b.own().launch_mode);
}

#[test]
fn the_gun_key_station_is_the_first_gun_with_rounds() {
    let s = gun_and_bomb();
    let config = s.own().configuration();
    assert_eq!(key_gun_station(config, |i| s.own().rounds(i)), Some(0));
    assert_eq!(
        key_gun_station(config, |_| 0),
        Some(0),
        "dry, still the gun"
    );
    let mut no_gun = fixture(false).own().configuration().clone();
    no_gun.stations.truncate(1);
    assert_eq!(key_gun_station(&no_gun, |_| 9), None);
}
