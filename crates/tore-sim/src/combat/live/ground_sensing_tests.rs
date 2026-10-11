//! The ground pages through combat (slice G1): the A-G command, silence to
//! anti-radiation seekers, registered ground objects on the page, the HARM
//! page's seeker choice and the Easy targeting cheat's nearest ground target.
use super::tests::fixture;
use super::*;
use crate::airport::OrientedBox;

const REDFOR: Side = Side(2);

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

fn tank(ahead_ft: f64) -> OrientedBox {
    OrientedBox {
        center: [0., 5., ahead_ft],
        half: [8., 5., 15.],
        heading: 0.,
        pitch: 0.,
        bank: 0.,
    }
}

/// The fixture with its range target gone and `ground` tanks registered as a
/// template's units would be: class 0x400, listed, `maxVisDist` 78.
fn scene(ground: &[(u32, f64, Side)]) -> State {
    let mut s = fixture(true);
    s.targets.clear();
    for &(id, ahead, side) in ground {
        s.add_ground_target(id, tank(ahead), 100, 0x400, side)
            .unwrap();
        assert!(s.set_ground_listing(
            id,
            GroundListing {
                flags: 0x821,
                max_visible: 78,
            }
        ));
    }
    s
}

fn tick(s: &mut State) {
    s.step(
        &[OwnshipInput {
            aircraft: 0,
            held: false,
            launcher: launcher(),
        }],
        |_, _| 0.,
    );
}

#[test]
fn the_ag_command_lists_registered_ground_objects_and_the_radar_falls_silent() {
    let mut s = scene(&[
        (0x5000_0001, 12_000., REDFOR),
        (0x5000_0002, 30_000., REDFOR),
    ]);
    assert!(ownship_target(s.own(), launcher()).radar_emitting);
    s.command(0, Command::ToggleAirGround, launcher());
    assert!(s.own().sensors.air_ground());
    tick(&mut s);
    let ids: Vec<u32> = s.own().sensors.ground().iter().map(|g| g.id()).collect();
    assert_eq!(ids, [0x5000_0001, 0x5000_0002]);
    assert_eq!(s.own().sensors.mode(), Some(sensors::Mode::AirGround));
    // The A-G page does not emit for anti-radiation seekers.
    assert!(!ownship_target(s.own(), launcher()).radar_emitting);
    // T designates the nearest; the HUD and the views follow it.
    s.command(0, Command::Designate, launcher());
    assert_eq!(s.own().designated(), Some(0x5000_0001));
    for _ in 0..10 {
        tick(&mut s);
    }
    assert_eq!(s.own().designated(), Some(0x5000_0001));
    assert_eq!(
        s.own_view().display_target().map(|t| t.id),
        Some(0x5000_0001)
    );
    // An object without the listing flag never shows.
    let mut s = scene(&[(0x5000_0003, 12_000., REDFOR)]);
    s.set_ground_listing(0x5000_0003, GroundListing::default());
    s.command(0, Command::ToggleAirGround, launcher());
    tick(&mut s);
    assert!(s.own().sensors.ground().is_empty());
}

#[test]
fn an_aircraft_shows_on_ag_only_while_the_host_says_it_is_in_an_airfield_sequence() {
    let mut s = scene(&[]);
    let mut landing = super::tests::target(0x0100_0001, [0., 300., 15_000.], 50, 0x8000);
    landing.velocity = [0., 0., -100.];
    s.targets.push(landing);
    s.command(0, Command::ToggleAirGround, launcher());
    tick(&mut s);
    assert!(s.own().sensors.ground().is_empty());
    s.set_airfield_attached(0x0100_0001, true);
    for _ in 0..sensors::track::GROUND_REBUILD_STEPS {
        tick(&mut s);
    }
    let listed: Vec<_> = s
        .own()
        .sensors
        .ground()
        .iter()
        .map(|g| g.symbol())
        .collect();
    assert_eq!(listed, [sensors::GroundSymbol::Square]);
}

#[test]
fn ctrl_x_designates_the_nearest_hostile_ground_object_only_with_easy_targeting() {
    let mut s = scene(&[
        (0x5000_0001, 40_000., REDFOR),
        (0x5000_0002, 20_000., REDFOR),
        (0x5000_0003, 5_000., NO_SIDE),
        (0x5000_0004, 8_000., DEFAULT_OWNSHIP_SIDE),
    ]);
    s.command(0, Command::NearestGround, launcher());
    assert_eq!(s.own().designated(), None);
    s.cheats.easy_targeting = true;
    s.command(0, Command::NearestGround, launcher());
    assert_eq!(s.own().designated(), Some(0x5000_0002));
    // Easy targeting keeps it although no page lists it.
    for _ in 0..60 {
        tick(&mut s);
    }
    assert_eq!(s.own().designated(), Some(0x5000_0002));
}

#[test]
fn the_harm_page_uses_the_shortest_range_anti_radiation_seeker_aboard() {
    let mut s = fixture(true);
    assert_eq!(ground_sensing::harm_seeker(s.own()), None);
    let own = s.own_mut();
    let mut agm88 = own.config.stations[0].clone();
    agm88.weapon.seeker.signature = 4;
    agm88.weapon.seeker.zones[0].maximum_range = 200_000;
    let mut agm45 = agm88.clone();
    agm45.weapon.seeker.zones[0].maximum_range = 60_000;
    own.config.stations = vec![agm88.clone(), agm45];
    own.ammo = vec![1, 1];
    assert_eq!(
        ground_sensing::harm_seeker(s.own()).map(|v| v.maximum_ft),
        Some(60_000.)
    );
    // Fired off, the AGM-45 no longer counts.
    s.own_mut().ammo[1] = 0;
    assert_eq!(
        ground_sensing::harm_seeker(s.own()).map(|v| v.maximum_ft),
        Some(200_000.)
    );
}

#[test]
fn the_ground_facts_survive_a_checkpoint() {
    let mut s = scene(&[(0x5000_0001, 12_000., REDFOR)]);
    s.set_airfield_attached(7, true);
    s.set_sensor_weather(sensors::Weather {
        bands: vec![sensors::WeatherBand {
            low_ft: 0.,
            high_ft: 9_000.,
            effects: [10, 50, 10, 75, 75],
        }],
    });
    let facts = |s: &State| {
        (
            s.ground_listing(0x5000_0001),
            s.airfield_attached(7),
            s.sensor_weather().clone(),
        )
    };
    let models = crate::checkpoint::Models::default();
    let mut restored = scene(&[]);
    restored.ground_listing = crate::checkpoint::round_trip(&s.ground_listing, &models).unwrap();
    restored.airfield = crate::checkpoint::round_trip(&s.airfield, &models).unwrap();
    restored.sensor_weather = crate::checkpoint::round_trip(&s.sensor_weather, &models).unwrap();
    assert_eq!(facts(&restored), facts(&s));
}
