//! The ground pages (docs/spec/air-to-ground.md, "The air-to-ground radar",
//! "FLIR ground contacts", "The HARM scope page", "Visual and sight
//! designation"), against the retail numbers in docs/formats/radar.md.
use super::*;
use crate::sensors::profile::{
    FEET_PER_NAUTICAL_MILE, InfraredProfile, Preset, RadarProfile, SensorProfiles, Volume,
};
use tore_formats::aircraft::AircraftId;

const NMI: f64 = FEET_PER_NAUTICAL_MILE;
const ALTITUDE: f64 = 5_000.;

fn volume(maximum_ft: f64, half_angle_deg: f64) -> Volume {
    Volume {
        azimuth_rad: half_angle_deg.to_radians(),
        elevation_rad: half_angle_deg.to_radians(),
        minimum_ft: 0.,
        maximum_ft,
        minimum_relative_ft: f64::NEG_INFINITY,
        maximum_relative_ft: f64::INFINITY,
    }
}

fn profiles() -> SensorProfiles {
    let preset = Preset::Advanced;
    SensorProfiles {
        aircraft: AircraftId::F18,
        radar: Some(RadarProfile {
            record: "TESTR.SEE".into(),
            search: volume(90. * NMI, 60.),
            track: volume(50. * NMI, 60.),
            look_down: 50.,
            preset,
            notch: preset.notch(),
            resistance: preset.resistance(),
            band: 0,
            source_flags: [0; 2],
            source_doppler: [0; 3],
        }),
        infrared: Some(InfraredProfile {
            record: "TESTI.SEE".into(),
            search: volume(20. * NMI, 60.),
            track: volume(10. * NMI, 60.),
        }),
        visual: Some(InfraredProfile {
            record: "TESTV.SEE".into(),
            search: volume(6. * NMI, 90.),
            track: volume(6. * NMI, 90.),
        }),
        jammer: None,
        signature: Default::default(),
    }
}

fn observer() -> Observer {
    Observer {
        position: [0., ALTITUDE, 0.],
        basis: Basis::new(0., 0., 0.),
        radar_powered: true,
        radar_failed: false,
        infrared_failed: false,
        visual_failed: false,
    }
}

/// A tank `ahead_nmi` ahead and `right_ft` to the right, on flat ground:
/// class 0x400, listed, `maxVisDist` 78 (the retail ground units' value).
fn unit(id: u32, ahead_nmi: f64, right_ft: f64) -> GroundObject {
    GroundObject {
        id,
        position: [right_ft, 10., ahead_nmi * NMI],
        velocity: [0.; 3],
        class: 0x400,
        listed: true,
        max_visible: 78,
        radar: 100.,
        infrared: 100.,
        emitting: false,
        aircraft: false,
        airborne: false,
        airfield: false,
        destroyed: false,
    }
}

fn aircraft(id: u32, ahead_nmi: f64, height_ft: f64, speed_fps: f64) -> GroundObject {
    GroundObject {
        class: 0x8000,
        aircraft: true,
        airborne: true,
        position: [0., height_ft, ahead_nmi * NMI],
        velocity: [0., 0., speed_fps],
        ..unit(id, 0., 0.)
    }
}

static GROUND: fn(f64, f64) -> f64 = |_, _| 0.;
static CLEAR: fn(Vector, Vector) -> bool = |_, _| false;

/// One fixed step of the air sensors and the ground pages.
fn step_with(
    s: &mut Sensors,
    objects: &[GroundObject],
    obscured: &dyn Fn(Vector, Vector) -> bool,
    weather: &Weather,
) -> Vec<Event> {
    let environment = Environment {
        ground: &GROUND,
        obscured,
    };
    let mut events = s.step(&observer(), &[], &environment);
    events.extend(s.step_ground(&observer(), objects, &environment, weather));
    events
}
fn step(s: &mut Sensors, objects: &[GroundObject]) -> Vec<Event> {
    step_with(s, objects, &CLEAR, &Weather::default())
}

fn air_ground() -> Sensors {
    let mut s = Sensors::new(profiles());
    assert!(s.toggle_air_ground());
    s
}

fn listed(s: &Sensors) -> Vec<u32> {
    s.ground().iter().map(|g| g.id()).collect()
}

#[test]
fn the_ag_page_lists_ground_objects_by_class_flag_and_visibility_distance() {
    let mut s = air_ground();
    let mut men = unit(3, 2., 0.);
    men.max_visible = 59;
    let mut scenery = unit(4, 3., 0.);
    scenery.listed = false;
    let mut ship = unit(5, 15., 0.);
    ship.class = 0x2000;
    ship.max_visible = 0;
    let mut sam = unit(6, 4., 500.);
    sam.class = 0x1000;
    let mut sa2 = unit(7, 18., 0.);
    sa2.class = 0x1000;
    sa2.max_visible = 391;
    step(
        &mut s,
        &[
            unit(1, 5., 0.),
            unit(2, 10., 0.),
            men,
            scenery,
            ship,
            sam,
            sa2,
        ],
    );
    assert_eq!(s.mode(), Some(Mode::AirGround));
    assert_eq!(Mode::AirGround.label(), "A-G");
    // Tank 2 is beyond 3 x 78 x 256 ft (9.9 nmi); men (59) and an unlisted
    // object never show; a ship has no visibility test; an SA-2 (391) reaches
    // 49 nmi, inside the 20 nmi box at the 10 nmi range.
    assert_eq!(listed(&s), [6, 1, 5, 7]);
    let symbols: Vec<GroundSymbol> = s.ground().iter().map(|g| g.symbol()).collect();
    assert_eq!(
        symbols,
        [
            GroundSymbol::Cross,
            GroundSymbol::Dot,
            GroundSymbol::Dot,
            GroundSymbol::Cross
        ]
    );
    assert!(s.ground()[0].wide_bars() && !s.ground()[1].wide_bars());
}

#[test]
fn aircraft_show_on_ag_only_in_an_airfield_sequence_whatever_their_speed() {
    let mut s = air_ground();
    // A helicopter hovering at 60 kt and a fighter at 300 kt: free flight,
    // so neither shows (no speed test in the retail page).
    let helicopter = aircraft(1, 3., 300., 101.);
    let fighter = aircraft(2, 4., 3000., 506.);
    let mut landing = aircraft(3, 5., 200., 220.);
    landing.airfield = true;
    step(&mut s, &[helicopter, fighter, landing]);
    assert_eq!(listed(&s), [3]);
    assert_eq!(s.ground()[0].symbol(), GroundSymbol::Square);
}

#[test]
fn the_ag_page_turns_the_air_radar_off() {
    let mut s = air_ground();
    let environment = Environment {
        ground: &GROUND,
        obscured: &CLEAR,
    };
    let flying = Observable {
        id: 9,
        position: [0., ALTITUDE, 5. * NMI],
        velocity: [0., 0., -500.],
        basis: Basis::new(0., 0., 0.),
        configuration: crate::sensors::Configuration::CLEAN,
        signature: Default::default(),
        jammer: None,
        jammer_active: false,
        radar_emitting: false,
        airborne: true,
        destroyed: false,
    };
    s.step(&observer(), std::slice::from_ref(&flying), &environment);
    assert!(s.contacts().is_empty() && s.strobes().is_empty());
    assert_eq!(s.support(9), Support::RadarOff);
    // Off again, the air radar sees it.
    assert!(!s.toggle_air_ground());
    s.step(&observer(), std::slice::from_ref(&flying), &environment);
    assert_eq!(s.contacts().len(), 1);
}

#[test]
fn ag_never_clears_the_designation_and_another_channel_turns_it_off() {
    let mut s = air_ground();
    step(&mut s, &[unit(1, 5., 0.)]);
    assert!(s.designate(1));
    assert!(!s.toggle_air_ground());
    assert_eq!(s.selected(), Some(1), "the toggle itself keeps it");
    assert!(s.toggle_air_ground());
    s.controls.channel = Channel::Infrared;
    step(&mut s, &[unit(1, 5., 0.)]);
    assert!(!s.air_ground());
    // Without a working radar A-G does not come on.
    let mut failed = Sensors::new(profiles());
    failed.radar_failed = true;
    assert!(!failed.toggle_air_ground());
}

#[test]
fn parked_aircraft_are_on_the_flir_and_never_on_ag() {
    let parked = GroundObject {
        aircraft: true,
        airborne: false,
        class: 0x8000,
        ..unit(1, 2., 0.)
    };
    let mut s = air_ground();
    step(&mut s, std::slice::from_ref(&parked));
    assert!(s.ground().is_empty());
    let mut s = Sensors::new(profiles());
    s.controls.channel = Channel::Infrared;
    step(&mut s, std::slice::from_ref(&parked));
    assert_eq!(listed(&s), [1]);
    assert_eq!(s.ground()[0].contact.channel, Channel::Infrared);
}

#[test]
fn terrain_masks_a_ground_contact() {
    let mut s = air_ground();
    let hidden = unit(2, 6., 0.).position;
    let behind_a_ridge = move |_: Vector, to: Vector| to == hidden;
    step_with(
        &mut s,
        &[unit(1, 5., 0.), unit(2, 6., 0.)],
        &behind_a_ridge,
        &Weather::default(),
    );
    assert_eq!(listed(&s), [1]);
}

#[test]
fn the_list_holds_fifty_nearest_and_close_objects_merge() {
    // Four rings of 19 tanks, 3, 5, 7 and 9 nmi out, 6 degrees apart: far
    // enough apart not to merge.
    let mut s = air_ground();
    let crowd: Vec<GroundObject> = (0..76u32)
        .map(|i| {
            let (ring, spoke) = (f64::from(i / 19), f64::from(i % 19));
            let (r, bearing) = ((3. + 2. * ring) * NMI, (-54. + 6. * spoke).to_radians());
            GroundObject {
                position: [r * bearing.sin(), 10., r * bearing.cos()],
                ..unit(100 + i, 0., 0.)
            }
        })
        .collect();
    step(&mut s, &crowd);
    assert_eq!(s.ground().len(), MAX_GROUND_CONTACTS);
    let ids = listed(&s);
    assert!((100..138).all(|id| ids.contains(&id)), "the two near rings");
    assert!((157..176).all(|id| !ids.contains(&id)), "not the far ring");
    // At 5 nmi two tanks 100 ft apart are one dot (closer than range / 20).
    let mut s = air_ground();
    step(&mut s, &[unit(1, 5., 0.), unit(2, 5., 100.)]);
    assert_eq!(listed(&s), [1]);
}

#[test]
fn t_cycles_nearest_first_shift_t_farthest_first_and_skips_friendlies() {
    let objects = [
        unit(1, 2., 0.),
        unit(2, 4., 0.),
        unit(3, 6., 0.),
        unit(4, 8., 0.),
    ];
    let mut s = air_ground();
    step(&mut s, &objects);
    let friendly = |id: u32| id == 3;
    let mut order = Vec::new();
    for _ in 0..4 {
        assert!(s.cycle(true, friendly));
        order.push(s.selected().unwrap());
    }
    assert_eq!(order, [1, 2, 4, 1]);
    s.clear_selection();
    let mut back = Vec::new();
    for _ in 0..4 {
        assert!(s.cycle(false, friendly));
        back.push(s.selected().unwrap());
    }
    assert_eq!(back, [4, 2, 1, 4]);
    // Only contacts on the scope count: at 5 nmi, the 6 and 8 nmi ones are off.
    s.controls.range_index = 0;
    s.clear_selection();
    assert!(s.cycle(false, |_| false));
    assert_eq!(s.selected(), Some(2));
}

#[test]
fn a_scope_click_designates_any_listed_contact_even_a_friendly() {
    let mut s = air_ground();
    step(&mut s, &[unit(1, 2., 0.), unit(2, 4., 0.)]);
    assert!(s.designate(2));
    assert_eq!(s.selected(), Some(2));
    assert!(!s.designate(77), "nothing unlisted");
    step(&mut s, &[unit(1, 2., 0.), unit(2, 4., 0.)]);
    assert_eq!(s.selected(), Some(2), "a listed selection stays");
    assert_eq!(s.observation(2).map(|c| c.id), Some(2));
}

#[test]
fn a_ground_designation_drops_when_no_channel_holds_it() {
    // Beyond visual range: leaving A-G drops it.
    let mut s = air_ground();
    step(&mut s, &[unit(1, 8., 0.)]);
    assert!(s.designate(1));
    s.toggle_air_ground();
    let events = step(&mut s, &[unit(1, 8., 0.)]);
    assert_eq!(s.selected(), None);
    assert!(events.contains(&Event::SelectionCleared(1)));
    // In view: the pilot still sees it, so it stays.
    let mut s = air_ground();
    step(&mut s, &[unit(1, 3., 0.)]);
    assert!(s.designate(1));
    s.toggle_air_ground();
    for _ in 0..10 {
        step(&mut s, &[unit(1, 3., 0.)]);
    }
    assert_eq!(s.selected(), Some(1));
    // An object that leaves the world leaves the list at the next refresh.
    let mut s = air_ground();
    step(&mut s, &[unit(1, 8., 0.)]);
    assert!(s.designate(1));
    for _ in 0..GROUND_REFRESH_STEPS {
        step(&mut s, &[]);
    }
    assert!(s.ground().is_empty());
    assert_eq!(s.selected(), None);
}

#[test]
fn the_sight_track_holds_a_designation_no_channel_sees() {
    let mut s = Sensors::new(profiles());
    s.hold(1);
    for _ in 0..240 {
        step(&mut s, &[unit(1, 30., 0.)]);
    }
    assert_eq!((s.selected(), s.held()), (Some(1), Some(1)));
    s.clear_selection();
    assert_eq!((s.selected(), s.held()), (None, None));
    // Easy targeting keeps a directly chosen ground object while it stands.
    let mut s = Sensors::new(profiles());
    s.keep_selection = true;
    s.select_ground(1);
    step(&mut s, &[unit(1, 30., 0.)]);
    assert_eq!(s.selected(), Some(1));
    let mut gone = unit(1, 30., 0.);
    gone.destroyed = true;
    s.keep_selection = false;
    step(&mut s, &[gone]);
    assert_eq!(s.selected(), None);
}

#[test]
fn the_flir_lists_hot_ground_objects_to_its_page_cap_and_cloud_cuts_range() {
    let mut s = Sensors::new(profiles());
    s.controls.channel = Channel::Infrared;
    let mut cold = unit(3, 1., 0.);
    cold.infrared = 0.;
    step(&mut s, &[unit(1, 3., 0.), unit(2, 6., 0.), cold]);
    // The IR page lists ground objects only out to 4.97 nmi.
    assert_eq!(listed(&s), [1]);
    assert_eq!(s.mode(), Some(Mode::Infrared));
    assert!(s.cycle(true, |_| false), "T designates on the IR page");
    assert_eq!(s.selected(), Some(1));
    // In cloud the infrared range is a tenth: 20 nmi becomes 2.
    let cloud = Weather {
        bands: vec![WeatherBand {
            low_ft: 0.,
            high_ft: 20_000.,
            effects: [10, 50, 10, 75, 75],
        }],
    };
    let mut s = Sensors::new(profiles());
    s.controls.channel = Channel::Infrared;
    step_with(&mut s, &[unit(1, 3., 0.), unit(2, 1.5, 0.)], &CLEAR, &cloud);
    assert_eq!(listed(&s), [2]);
}

#[test]
fn enter_takes_the_visible_contact_nearest_the_centre_then_steps_right() {
    let mut s = air_ground();
    let objects = [
        unit(1, 3., -2_000.),
        unit(2, 3., 300.),
        unit(3, 3., 2_000.),
        unit(4, 9., 0.),
    ];
    step(&mut s, &objects);
    assert!(s.ground().iter().any(|g| g.id() == 4 && !g.visible));
    let position = observer().position;
    let basis = observer().basis;
    let mut picks = Vec::new();
    for _ in 0..4 {
        assert!(s.select_visual(position, basis, |_| false));
        picks.push(s.selected().unwrap());
    }
    // Nearest the centre first, then left to right, wrapping; never the one
    // beyond visual range.
    assert_eq!(picks, [2, 3, 1, 2]);
}

#[test]
fn the_harm_page_lists_emitters_only_inside_its_cone() {
    let mut s = Sensors::new(profiles());
    s.set_harm_seeker(Some(volume(60_000., 45.)));
    s.controls.channel = Channel::Harm;
    let emitting = |mut o: GroundObject| {
        o.emitting = true;
        o
    };
    let mut ship = emitting(unit(3, 4., 0.));
    ship.class = 0x2000;
    let mut radar = emitting(unit(1, 5., 0.));
    radar.class = 0x1000;
    let abeam = emitting(unit(4, 0.5, 6_000.));
    let flying = emitting(aircraft(5, 6., 4_000., 500.));
    let low = emitting(aircraft(6, 6., 150., 500.));
    step(&mut s, &[radar, unit(2, 4., 0.), ship, abeam, flying, low]);
    assert_eq!(s.mode(), Some(Mode::Harm));
    assert_eq!(listed(&s), [1, 5]);
    assert_eq!(s.ground()[0].symbol(), GroundSymbol::Cross);
    assert_eq!(s.ground()[1].symbol(), GroundSymbol::Square);
    assert!(s.contacts().is_empty(), "no air radar on the HARM page");
    // Without a seeker the channel is not available and falls back to radar.
    let mut none = Sensors::new(profiles());
    none.controls.channel = Channel::Harm;
    assert!(!none.available(Channel::Harm));
    step(&mut none, &[]);
    assert_ne!(none.mode(), Some(Mode::Harm));
}

#[test]
fn the_ag_list_rebuilds_every_six_seconds_and_follows_its_objects() {
    let mut s = air_ground();
    step(&mut s, &[unit(1, 5., 0.)]);
    // A tank that drives in after the rebuild waits for the next one.
    let mut objects = vec![unit(1, 5., 0.), unit(2, 4., 0.)];
    for _ in 1..GROUND_REBUILD_STEPS {
        objects[0].position[0] += 1.;
        step(&mut s, &objects);
    }
    assert_eq!(listed(&s), [1]);
    assert!(
        s.ground()[0].contact.position[0] > 600.,
        "it follows its tank"
    );
    step(&mut s, &objects);
    assert_eq!(listed(&s), [2, 1]);
}

#[test]
fn weather_percent_is_the_least_layer_the_span_meets() {
    let weather = Weather {
        bands: vec![
            WeatherBand {
                low_ft: 0.,
                high_ft: 3_000.,
                effects: [10, 50, 10, 100, 100],
            },
            WeatherBand {
                low_ft: 3_000.,
                high_ft: 8_000.,
                effects: [75, 100, 125, 75, 75],
            },
        ],
    };
    assert_eq!(weather.percent(selector::INFRARED, 10_000., 9_000.), 100.);
    assert_eq!(weather.percent(selector::INFRARED, 5_000., 6_000.), 100.);
    assert_eq!(weather.percent(selector::INFRARED, 5_000., 10.), 10.);
    assert_eq!(weather.percent(selector::RADAR, 5_000., 10.), 75.);
    assert_eq!(Weather::default().percent(selector::RADAR, 0., 1.), 100.);
}
