//! Guided air-to-ground weapons through combat (slice W1, plan 11.1): the
//! laser lock at release and in the nose cone, the one first lock of a
//! weapon released with no spot, the blind fall, buddy spots, the JDAM's
//! stored point in any weather and against a moving target, the AT-2's
//! command guidance and its loss, the readiness and advisories, the time to
//! impact and the checkpoint. Records as the retail data has them
//! (docs/formats/missiles.md, "Laser and designator" and "Optical").
use super::tests::fixture;
use super::*;
use crate::airport::OrientedBox;
use crate::combat::delivery::DeliveryMode;
use crate::combat::guided::Advisory;
use crate::combat::laser::LaserState;
use crate::combat::sight::{FIGHTER_DEFAULT_LOOK, Gimbal, Sight, SightSensor};
use crate::sensors::{Weather, WeatherBand};
use tore_formats::weapons::Zone;

const REDFOR: Side = Side(2);
const TANK: u32 = 0x5000_0001;
const KNOT: f64 = 1.687_8;
/// The tank stands this far ahead of the release, feet.
const AHEAD: f64 = 12_000.;
const ALTITUDE: f64 = 8_000.;

fn zone(angle: i16, minimum: i32, maximum: i32) -> Zone {
    Zone {
        heading: angle,
        pitch: angle,
        minimum_range: minimum,
        maximum_range: maximum,
        minimum_altitude: i32::MIN,
        maximum_altitude: i32::MAX,
    }
}

/// A GBU-10E as the record has it: signature 1, flags 0x2a543, no motor,
/// retard 90, 14 ft/s² toward 278 ft/s, 6,370 units a second, 60 s.
pub(crate) fn gbu10() -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = "GBU10.JT".into();
    w.flags = 0x2a543;
    w.seeker.signature = 1;
    w.seeker.zones = [zone(16380, 0, 50_000), zone(8190, 0, 30_000)];
    w.movement.minimum_speed = 0;
    w.movement.corner_speed = 2933;
    w.movement.maximum_speed = 2933;
    w.movement.acceleration = 0;
    w.movement.deceleration = 14;
    w.movement.initial_speed = 0;
    w.movement.final_speed = 278;
    w.movement.launch_retard = 90;
    w.movement.ignite_t = 0;
    w.movement.fuel_t = 0;
    w.movement.remove_t = 240;
    w.movement.powered_turn_rate = 6370;
    w.movement.unpowered_turn_rate = 6370;
    w.movement.cruise = [0; 4];
    w.burst.actual_rounds_per_game = 1;
    w.burst.game_rounds_in_burst = 1;
    w.burst.projectiles_in_pod = 1;
    w.burst.game_burst_t = 0;
    w.damage.by_class = [400; 5];
    w.damage.fuze_arm_t = 4;
    w.damage.fuze_radius = 100;
    w.damage.collateral_radius = 0;
    w
}

/// A GBU-29 JDAM: the laser bombs' record without 0x100 and 0x400, and
/// signature 0.
pub(crate) fn gbu29() -> Weapon {
    let mut w = gbu10();
    w.source = "GBU29.JT".into();
    w.flags = 0x2a043;
    w.seeker.signature = 0;
    w.seeker.zones[1].minimum_range = 50;
    w
}

/// An AT-2 Swatter: signature 0, flags 0x2216f, a 9 s motor, the cruise
/// profile, zone 1 500 to 18,000 ft and 45 degrees.
pub(crate) fn at2() -> Weapon {
    let mut w = gbu10();
    w.source = "AT2.JT".into();
    w.flags = 0x2216f;
    w.seeker.signature = 0;
    w.seeker.zones = [zone(8190, 0, 20_000), zone(8190, 500, 18_000)];
    w.movement.acceleration = 513;
    w.movement.deceleration = 146;
    w.movement.final_speed = 1026;
    w.movement.launch_retard = 100;
    w.movement.ignite_t = 4;
    w.movement.fuel_t = 40;
    w.movement.remove_t = 120;
    w.movement.powered_turn_rate = 10920;
    w.movement.unpowered_turn_rate = 8190;
    w.movement.cruise = [29, 4, 29, 4];
    w
}

/// The Pave Spike's sight (laser, 10 nmi, a pod).
fn pave_spike() -> SightSensor {
    SightSensor {
        laser: true,
        range_ft: 60_760.,
        gimbal: Gimbal::Hemisphere,
        eye: [0., -4., 0.],
        default_look: FIGHTER_DEFAULT_LOOK,
    }
}

fn tank_box(z: f64) -> OrientedBox {
    OrientedBox {
        center: [0., 5., z],
        half: [8., 5., 15.],
        heading: 0.,
        pitch: 0.,
        bank: 0.,
    }
}

/// Aircraft 0 carrying four of `weapon`, armed and selected, with a Pave
/// Spike (`designator`), and a tank `ahead` feet north.
fn scene(weapon: Weapon, designator: bool, ahead: f64) -> State {
    let mut config = fixture(false).own().configuration().clone();
    config.stations = vec![Station {
        weapon,
        mount: [0.; 3],
        count: 4,
        internal: false,
        rack: Default::default(),
    }];
    let mut s = State::new(config, true).unwrap();
    s.targets.clear();
    s.add_ground_target(TANK, tank_box(ahead), 100, 0x400, REDFOR)
        .unwrap();
    let own = s.own_mut();
    own.armed = true;
    own.selected = 0;
    if designator {
        own.config.sight = Some(pave_spike());
        own.sight = Some(Sight::new(pave_spike()));
    }
    s
}

/// Level at `altitude`, heading north at 450 kt.
fn launcher_at(altitude: f64) -> Launcher {
    let basis = Basis::new(0., 0., 0.);
    let speed = 450. * KNOT;
    Launcher {
        position: [0., altitude, 0.],
        basis,
        speed_fps: speed,
        velocity: basis.forward.map(|v| v * speed),
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    }
}

fn launcher() -> Launcher {
    launcher_at(ALTITUDE)
}

/// Aircraft `n` flies 1,000 ft right of aircraft 0, clear of a collision.
fn abreast(launcher: Launcher, n: u32) -> Launcher {
    Launcher {
        position: [
            launcher.position[0] + 1_000. * f64::from(n),
            launcher.position[1],
            launcher.position[2],
        ],
        ..launcher
    }
}

fn step_as(s: &mut State, held: bool, launcher: Launcher) -> Vec<Event> {
    let inputs: Vec<OwnshipInput> = s
        .ownships
        .iter()
        .map(|own| OwnshipInput {
            aircraft: own.aircraft,
            held: held && own.aircraft == 0,
            launcher: abreast(launcher, own.aircraft),
        })
        .collect();
    s.step(&inputs, |_, _| 0.)
}

fn step(s: &mut State, held: bool) -> Vec<Event> {
    step_as(s, held, launcher())
}

/// Designates the tank, which the sight then tracks.
fn designate(s: &mut State) {
    s.own_mut().sensors.select_ground(TANK);
    step(s, false);
}

fn tank(s: &State) -> &Target {
    s.targets.iter().find(|t| t.id == TANK).unwrap()
}

fn flight(s: &State) -> GuidedFlight {
    let (_, flight) = s
        .guided_flights()
        .next()
        .expect("a guided weapon in flight");
    *flight
}

/// Steps until nothing is in flight, at most `seconds`; returns whether the
/// tank was destroyed.
fn fly_out(s: &mut State, seconds: f64) -> bool {
    let mut destroyed = false;
    for _ in 0..(seconds * 120.) as usize {
        if s.projectiles.is_empty() {
            return destroyed;
        }
        destroyed |= step(s, false)
            .iter()
            .any(|e| matches!(e, Event::Destroyed(TANK)));
    }
    panic!("still flying after {seconds} s");
}

#[test]
fn a_laser_bomb_with_its_own_spot_in_zone_one_guides_onto_the_tank() {
    let mut s = scene(gbu10(), true, AHEAD);
    designate(&mut s);
    // A laser weapon selected and armed lases (the automatic rule).
    step(&mut s, false);
    assert_eq!(s.own().laser.state, LaserState::Lasing);
    let sight = s.own().guided_sight.unwrap();
    assert_eq!(sight.kind, Guidance::Laser);
    assert!(sight.in_reach, "{sight:?}");
    assert_eq!(sight.advisory, None);
    let reach = sight.reach.unwrap();
    // Level at 8,000 ft the 45 degree cone starts 8,000 ft out.
    assert!((reach.minimum_ft - ALTITUDE).abs() < 30., "{reach:?}");
    assert!(reach.contains(AHEAD));
    step(&mut s, true);
    assert_eq!(s.projectiles.len(), 1);
    let p = &s.projectiles[0];
    assert_eq!(p.target, None, "the legacy pursuit never steers it");
    // Retard 90: 90 percent of 759 ft/s along the nose.
    let motion = p.motion.unwrap();
    assert!((missiles::length(motion.velocity) - 683.).abs() < 2.);
    let f = flight(&s);
    assert_eq!(f.mode, GuidedMode::Guiding);
    assert_eq!(f.spot_owner, Some(0));
    let first = f.seconds.unwrap();
    step(&mut s, false);
    let second = flight(&s).seconds.unwrap();
    assert!(second < first, "the time to impact counts down");
    assert!(fly_out(&mut s, 60.), "the tank was not destroyed");
    assert!(s.guided_impact(0).is_some());
}

#[test]
fn a_spot_beyond_zone_one_leaves_the_bomb_blind_on_the_bomb_equation() {
    let mut s = scene(gbu10(), true, 40_000.);
    designate(&mut s);
    step(&mut s, true);
    // The tank is beyond zone 1 (30,000 ft): a spot out of reach leaves the
    // bomb blind, as retail, and it falls.
    let f = flight(&s);
    assert_eq!(f.mode, GuidedMode::Blind);
    assert!(f.fall.is_some());
    let id = s.projectiles[0].id;
    for _ in 0..240 {
        step(&mut s, false);
    }
    let (fall, start) = s.guided_flight(id).unwrap().fall.unwrap();
    let p = &s.projectiles[0];
    let expected = fall.position((p.age - start) as f64 / 120.);
    assert!(
        missiles::sub(p.position, expected)
            .iter()
            .all(|d| d.abs() < 1e-9)
    );
    // Two seconds of fall is 64 ft; the bomb's scatter (up to 1.5 degrees
    // up, slice ST-G) can give back 40 of them.
    assert!(p.position[1] < ALTITUDE - 20.);
}

#[test]
fn a_bomb_released_with_no_spot_locks_once_in_its_nose_cone_and_never_again() {
    let mut s = scene(gbu10(), true, AHEAD);
    designate(&mut s);
    // Ctrl+L off by hand: the laser stays off with the weapon selected.
    s.command(0, Command::ToggleLaser, launcher());
    step(&mut s, false);
    assert_eq!(s.own().laser.state, LaserState::Ready);
    let sight = s.own().guided_sight.unwrap();
    assert_eq!(sight.advisory, Some(Advisory::NoLaser));
    assert!(!sight.in_reach, "no spot, no IN RNG");
    // Released anyway: it waits and falls.
    assert_eq!(s.own().release_readiness, Readiness::Ready);
    step(&mut s, true);
    assert_eq!(flight(&s).mode, GuidedMode::Waiting);
    assert!(flight(&s).fall.is_some());
    // A release puts the laser back on the automatic rule: it lases again,
    // the spot is inside the falling bomb's 45 degree cone, and it locks.
    step(&mut s, false);
    step(&mut s, false);
    let f = flight(&s);
    assert_eq!(f.mode, GuidedMode::Guiding, "{f:?}");
    assert!(f.fall.is_none());
    // Laser off by hand: blind for good, falling from where it is.
    s.command(0, Command::ToggleLaser, launcher());
    step(&mut s, false);
    step(&mut s, false);
    assert_eq!(flight(&s).mode, GuidedMode::Blind);
    assert!(flight(&s).fall.is_some());
    s.command(0, Command::ToggleLaser, launcher());
    for _ in 0..60 {
        step(&mut s, false);
    }
    assert_eq!(s.own().laser.state, LaserState::Lasing);
    assert_eq!(
        flight(&s).mode,
        GuidedMode::Blind,
        "retail never reacquires"
    );
}

#[test]
fn a_laser_turned_off_mid_fall_leaves_the_bomb_blind_and_it_misses() {
    let mut s = scene(gbu10(), true, AHEAD);
    designate(&mut s);
    step(&mut s, true);
    for _ in 0..(5 * 120) {
        step(&mut s, false);
    }
    assert_eq!(flight(&s).mode, GuidedMode::Guiding);
    s.command(0, Command::ToggleLaser, launcher());
    step(&mut s, false);
    step(&mut s, false);
    assert_eq!(flight(&s).mode, GuidedMode::Blind);
    assert!(!fly_out(&mut s, 60.), "a blind bomb hit the tank");
    assert_eq!(tank(&s).hp, 100);
}

#[test]
fn a_wingman_without_a_designator_drops_on_a_buddys_spot() {
    let mut s = scene(gbu10(), true, AHEAD);
    let config = s.own().config.clone();
    let mut wingman = Ownship::new(1, s.own().side, config, true).unwrap();
    wingman.config.sight = None;
    wingman.sight = None;
    wingman.armed = true;
    s.add_ownship(wingman).unwrap();
    // The lead lases the tank by hand with nothing selected.
    s.own_mut().armed = false;
    designate(&mut s);
    s.command(0, Command::ToggleLaser, launcher());
    step(&mut s, false);
    assert_eq!(s.own().laser.state, LaserState::Lasing);
    step(&mut s, false);
    let sight = s.ownship(1).unwrap().guided_sight.unwrap();
    assert!(sight.buddy && sight.in_reach, "{sight:?}");
    // The wingman releases.
    let inputs = [
        OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: launcher(),
        },
        OwnshipInput {
            aircraft: 1,
            held: true,
            launcher: abreast(launcher(), 1),
        },
    ];
    s.step(&inputs, |_, _| 0.);
    let f = flight(&s);
    assert_eq!(
        (f.owner, f.mode, f.spot_owner),
        (1, GuidedMode::Guiding, Some(0))
    );
    assert!(fly_out(&mut s, 60.));
}

#[test]
fn a_spot_of_the_other_side_is_never_used() {
    let mut s = scene(gbu10(), true, AHEAD);
    let config = s.own().config.clone();
    let mut enemy = Ownship::new(1, REDFOR, config, true).unwrap();
    enemy.config.sight = None;
    enemy.sight = None;
    enemy.armed = true;
    s.add_ownship(enemy).unwrap();
    s.own_mut().armed = false;
    designate(&mut s);
    s.command(0, Command::ToggleLaser, launcher());
    step(&mut s, false);
    step(&mut s, false);
    assert_eq!(s.own().laser.state, LaserState::Lasing);
    let sight = s.ownship(1).unwrap().guided_sight.unwrap();
    assert_eq!(sight.advisory, Some(Advisory::NoLaser));
}

#[test]
fn a_jdam_flies_to_the_point_stored_at_release_in_any_weather_and_misses_a_mover() {
    // Cloud between the aircraft and the tank does not touch a JDAM.
    let overcast = Weather {
        bands: vec![WeatherBand {
            low_ft: 2_000.,
            high_ft: 5_000.,
            effects: [10, 50, 10, 75, 100],
        }],
    };
    let mut s = scene(gbu29(), false, AHEAD);
    s.set_sensor_weather(overcast.clone());
    step(&mut s, false);
    // Nothing designated and no sight: it is held back.
    assert_eq!(s.own().release_readiness, Readiness::NoTarget);
    assert_eq!(
        s.own().guided_sight.unwrap().advisory,
        Some(Advisory::NoTarget)
    );
    step(&mut s, true);
    assert!(s.projectiles.is_empty());
    designate(&mut s);
    assert_eq!(s.own().release_readiness, Readiness::Ready);
    step(&mut s, false);
    step(&mut s, true);
    let f = flight(&s);
    assert_eq!(f.kind, Guidance::Coordinate);
    assert_eq!(f.mode, GuidedMode::Guiding);
    assert_eq!(f.point.unwrap(), tank(&s).position);
    assert!(fly_out(&mut s, 60.), "the JDAM missed a standing tank");

    // The same pass at a tank that drives off after the release: the bomb
    // lands on the stored point and the tank is gone from it.
    let mut s = scene(gbu29(), false, AHEAD);
    s.set_sensor_weather(overcast);
    designate(&mut s);
    step(&mut s, true);
    let stored = flight(&s).point.unwrap();
    let mut destroyed = false;
    let mut last = s.projectiles[0].position;
    let mut driven = 0.;
    while !s.projectiles.is_empty() {
        driven += 50. / 120.;
        let mut place = tank_box(AHEAD);
        place.center[0] += driven;
        assert!(s.move_ground_target(TANK, place, [50., 0., 0.], true));
        last = s.projectiles[0].position;
        destroyed |= step(&mut s, false)
            .iter()
            .any(|e| matches!(e, Event::Destroyed(TANK)));
    }
    assert!(!destroyed);
    assert!(
        crate::combat::guided::ground_range(last, stored) < 60.,
        "it came down {last:?}, not on the stored point {stored:?}"
    );
}

#[test]
fn an_at2_is_steered_while_the_launcher_holds_it_and_turning_away_loses_it() {
    // A helicopter 300 ft up, the tank 6,000 ft ahead.
    let hover = |heading: f64| Launcher {
        position: [0., 300., 0.],
        basis: Basis::new(heading, 0., 0.),
        speed_fps: 60.,
        velocity: Basis::new(heading, 0., 0.).forward.map(|v| v * 60.),
        ..launcher()
    };
    let mut s = scene(at2(), false, 6_000.);
    step_as(&mut s, false, hover(0.));
    assert_eq!(s.own().release_readiness, Readiness::NoTarget);
    s.own_mut().sensors.select_ground(TANK);
    step_as(&mut s, false, hover(0.));
    assert_eq!(s.own().release_readiness, Readiness::Ready);
    step_as(&mut s, true, hover(0.));
    let f = flight(&s);
    assert_eq!(
        (f.kind, f.mode, f.target),
        (Guidance::Command, GuidedMode::Guiding, Some(TANK))
    );
    let mut destroyed = false;
    while !s.projectiles.is_empty() {
        destroyed |= step_as(&mut s, false, hover(0.))
            .iter()
            .any(|e| matches!(e, Event::Destroyed(TANK)));
    }
    assert!(destroyed, "the AT-2 missed with the launcher holding it");

    // Again, and the helicopter turns away two seconds in: lost for good.
    let mut s = scene(at2(), false, 6_000.);
    s.own_mut().sensors.select_ground(TANK);
    step_as(&mut s, false, hover(0.));
    step_as(&mut s, true, hover(0.));
    for _ in 0..240 {
        step_as(&mut s, false, hover(0.));
    }
    assert_eq!(flight(&s).mode, GuidedMode::Guiding);
    step_as(&mut s, false, hover(std::f64::consts::FRAC_PI_2));
    assert_eq!(flight(&s).mode, GuidedMode::Blind);
    for _ in 0..120 {
        step_as(&mut s, false, hover(0.));
    }
    assert_eq!(flight(&s).mode, GuidedMode::Blind, "a lost AT-2 stays lost");
}

#[test]
fn guided_flights_round_trip_a_checkpoint() {
    let mut s = scene(gbu10(), true, AHEAD);
    designate(&mut s);
    s.command(0, Command::ToggleLaser, launcher());
    step(&mut s, false);
    step(&mut s, true);
    step(&mut s, false);
    let models = crate::checkpoint::Models::default();
    assert_eq!(
        crate::checkpoint::round_trip(&s.guided, &models).unwrap(),
        s.guided
    );
    for _ in 0..240 {
        step(&mut s, false);
    }
    assert_eq!(
        crate::checkpoint::round_trip(&s.guided, &models).unwrap(),
        s.guided
    );
    s.guided_impacts.insert(7, 99);
    assert_eq!(
        crate::checkpoint::round_trip(&s.guided_impacts, &models).unwrap(),
        s.guided_impacts
    );
}

/// The aircraft at `z` feet along its heading, level at `ALTITUDE`.
fn at_z(z: f64) -> Launcher {
    Launcher {
        position: [0., ALTITUDE, z],
        ..launcher()
    }
}

/// Flies the level pass from `z = 0` for `seconds` with the trigger as `held`
/// says, and returns when (seconds) and where (ground range, feet) the first
/// guided weapon appeared and how many did.
fn bar_pass(
    s: &mut State,
    held: impl Fn(f64) -> bool,
    seconds: f64,
) -> (Option<(f64, f64)>, usize) {
    let speed = 450. * KNOT;
    let mut first = None;
    for tick in 0..(seconds * 120.) as u64 {
        let t = tick as f64 / 120.;
        step_as(s, held(t), at_z(speed * t));
        if first.is_none() && !s.projectiles.is_empty() {
            let range = s.targets.iter().find(|t| t.id == TANK).unwrap().position[2] - speed * t;
            first = Some((t, range));
        }
    }
    (first, s.projectiles.len() + s.guided_impacts.len())
}

#[test]
fn a_held_trigger_releases_a_jdam_once_at_the_bar_and_it_hits() {
    let mut s = scene(gbu29(), false, 60_000.);
    designate(&mut s);
    // Pass the tank is 60,000 ft ahead, the aircraft at 8,000 ft and 450 kt.
    s.command(0, Command::ToggleSeekerMode, at_z(0.));
    assert_eq!(s.own().delivery_mode, DeliveryMode::Level);
    let (first, count) = bar_pass(&mut s, |_| true, 70.);
    let (t, range) = first.expect("a JDAM was released");
    assert_eq!(count, 1, "one pass, one release");
    // The reach at 8,000 ft and 450 kt is about 8,000 to 19,000 ft; the bar
    // stands two fifths in from the far edge.
    assert!(
        (13_000. ..16_500.).contains(&range),
        "released at {range} ft after {t} s"
    );
    assert!(s.own().rounds(0) == 3);
    assert!(fly_out(&mut s, 70.), "the tank was not destroyed");
}

#[test]
fn a_press_before_the_bar_waits_and_ccip_releases_on_the_press() {
    // Pressed and let go well before the bar: nothing falls.
    let mut s = scene(gbu29(), false, 60_000.);
    designate(&mut s);
    s.command(0, Command::ToggleSeekerMode, at_z(0.));
    let (first, _) = bar_pass(&mut s, |t| t < 0.5, 40.);
    assert!(first.is_none(), "{first:?}");
    assert_eq!(s.own().rounds(0), 4);
    // The same press in CCIP drops at once.
    let mut s = scene(gbu29(), false, 60_000.);
    designate(&mut s);
    assert_eq!(s.own().delivery_mode, DeliveryMode::Ccip);
    step_as(&mut s, true, at_z(0.));
    assert_eq!(s.projectiles.len(), 1);
}

#[test]
fn a_selected_jdam_starts_in_lvl_on_a_level_pass_with_a_target() {
    let mut s = scene(gbu29(), false, 40_000.);
    designate(&mut s);
    let level = at_z(0.);
    s.command(0, Command::SelectNav, level);
    s.command(0, Command::NextSelection, level);
    s.command(0, Command::NextSelection, level);
    assert_eq!(s.own().selected, 0);
    assert_eq!(s.own().delivery_mode, DeliveryMode::Level);
    s.own_mut().armed = true;
    step_as(&mut s, false, level);
    let sight = s.own().guided_sight.unwrap();
    assert!(sight.level.is_some(), "{sight:?}");
    // Ctrl+M returns to the plain scale.
    s.command(0, Command::ToggleSeekerMode, level);
    step_as(&mut s, false, level);
    assert!(s.own().guided_sight.unwrap().level.is_none());
}

#[test]
fn a_target_far_off_the_stem_holds_the_trigger_without_releasing() {
    // 9,000 ft right of the track at 14,000 ft range is 33 degrees off.
    let mut s = scene(gbu29(), false, 60_000.);
    s.targets
        .iter_mut()
        .find(|t| t.id == TANK)
        .unwrap()
        .position[0] = 9_000.;
    designate(&mut s);
    s.command(0, Command::ToggleSeekerMode, at_z(0.));
    let (first, _) = bar_pass(&mut s, |_| true, 60.);
    assert!(
        first.is_none(),
        "released with the aim off the stem: {first:?}"
    );
}

#[test]
fn a_laser_bomb_has_the_t_only_with_a_spot_to_home_on() {
    let mut s = scene(gbu10(), true, 40_000.);
    designate(&mut s);
    s.command(0, Command::ToggleSeekerMode, at_z(0.));
    // Laser off by hand: NO LASER, no T, the trigger is a plain press.
    s.command(0, Command::ToggleLaser, at_z(0.));
    step_as(&mut s, false, at_z(0.));
    let sight = s.own().guided_sight.unwrap();
    assert_eq!(sight.advisory, Some(Advisory::NoLaser));
    assert!(sight.level.is_none());
    // Laser back on: a spot and the T.
    s.command(0, Command::ToggleLaser, at_z(0.));
    step_as(&mut s, false, at_z(0.));
    step_as(&mut s, false, at_z(0.));
    assert!(s.own().guided_sight.unwrap().level.is_some());
}

#[test]
fn a_buddy_spot_must_be_visible_from_the_shooter_at_lock_and_release() {
    let ridge = |_: f64, z: f64| {
        if (5000. ..=7000.).contains(&z) {
            7000.
        } else {
            0.
        }
    };
    for w in [gbu10(), {
        let mut w = at2();
        w.source = "AS30.JT".into();
        w.seeker.signature = 1;
        w.flags |= 0x400;
        w
    }] {
        let s = scene(w.clone(), false, AHEAD);
        let spots = [crate::combat::laser::LaserSpot {
            owner: 1,
            side: s.own().side,
            position: tank(&s).position,
            target: Some(TANK),
        }];
        let weather = Weather::default();
        let release = |ground: &dyn Fn(f64, f64) -> f64| {
            guided::release(
                &w,
                Guidance::Laser,
                guided::Releaser {
                    aircraft: s.own().aircraft,
                    side: s.own().side,
                    cockpit: &s.own().cockpit,
                },
                0,
                &launcher(),
                launcher().position,
                launcher().velocity,
                &spots,
                &s.targets,
                &weather,
                &ground,
            )
        };
        assert_eq!(release(&|_, _| 0.).mode, GuidedMode::Guiding);
        assert_eq!(release(&ridge).mode, GuidedMode::Blind, "{}", w.source);
        let sight = guided::sight(
            s.own().aircraft,
            s.own().side,
            &s.own().cockpit,
            &s.own().config,
            &launcher(),
            &guided::SightContext {
                spots: &spots,
                targets: &s.targets,
                weather: &weather,
                ground: &ridge,
            },
        )
        .unwrap();
        assert_eq!(sight.advisory, Some(Advisory::Masked));
        assert!(sight.in_reach, "MASK does not alter the energy envelope");
    }
}

#[test]
fn a_command_weapon_has_mask_advisory_and_no_guidance_through_a_ridge() {
    let mut s = scene(at2(), true, AHEAD);
    designate(&mut s);
    let ridge = |_: f64, z: f64| {
        if (5000. ..=7000.).contains(&z) {
            7000.
        } else {
            0.
        }
    };
    let w = &s.own().config.stations[0].weapon;
    let weather = Weather::default();
    let flight = guided::release(
        w,
        Guidance::Command,
        guided::Releaser {
            aircraft: s.own().aircraft,
            side: s.own().side,
            cockpit: &s.own().cockpit,
        },
        0,
        &launcher(),
        launcher().position,
        launcher().velocity,
        &[],
        &s.targets,
        &weather,
        &ridge,
    );
    assert_eq!(flight.mode, GuidedMode::Blind);
    let sight = guided::sight(
        s.own().aircraft,
        s.own().side,
        &s.own().cockpit,
        &s.own().config,
        &launcher(),
        &guided::SightContext {
            spots: &[],
            targets: &s.targets,
            weather: &weather,
            ground: &ridge,
        },
    )
    .unwrap();
    assert_eq!(sight.advisory, Some(Advisory::Masked));
    assert!(sight.in_reach);
}

#[test]
fn a_late_first_laser_lock_checks_terrain_but_an_existing_lock_keeps_its_rules() {
    let mut s = scene(gbu10(), true, AHEAD);
    designate(&mut s);
    s.command(0, Command::ToggleLaser, launcher());
    step(&mut s, false);
    step(&mut s, true);
    let mut flight = flight(&s);
    assert_eq!(flight.mode, GuidedMode::Waiting);
    let mut p = s.projectiles[0].clone();
    let point = tank(&s).position;
    s.own_mut().laser.state = LaserState::Lasing;
    s.own_mut().laser.point = Some(point);
    let w = gbu10();
    let ridge = |_: f64, z: f64| {
        if (5000. ..=7000.).contains(&z) {
            7000.
        } else {
            0.
        }
    };
    let steer =
        |flight: &mut GuidedFlight, p: &mut Projectile, ground: &dyn Fn(f64, f64) -> f64| {
            guided::steer(
                flight,
                p,
                &w,
                &guided::Context {
                    ships: s.ownships(),
                    cockpits: s.cockpits_with(s.ownships()),
                    rows: &[],
                    targets: &s.targets,
                    ground: &ground,
                },
            );
        };
    steer(&mut flight, &mut p, &ridge);
    assert_eq!(flight.mode, GuidedMode::Waiting);
    steer(&mut flight, &mut p, &|_, _| 0.);
    assert_eq!(flight.mode, GuidedMode::Guiding);
    steer(&mut flight, &mut p, &ridge);
    assert_eq!(
        flight.mode,
        GuidedMode::Guiding,
        "terrain does not break an established laser lock"
    );
}
