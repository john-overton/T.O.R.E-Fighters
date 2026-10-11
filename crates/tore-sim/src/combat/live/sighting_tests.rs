//! The targeting sight and the laser through combat (slice T1): the sight's
//! track designates, a designated ground object becomes the track, the
//! automatic laser and Ctrl+L, a spot that follows a moving unit, buddy
//! spots by side, and the checkpoint.
use super::tests::fixture;
use super::*;
use crate::airport::OrientedBox;
use crate::combat::laser::LaserState;
use crate::combat::sight::{FIGHTER_DEFAULT_LOOK, Gimbal, Sight, SightMode, SightSensor};

const REDFOR: Side = Side(2);
const TANK: u32 = 0x5000_0001;

fn launcher() -> Launcher {
    Launcher {
        position: [0., 3_500., 0.],
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

/// The Pave Spike's sight (PAVESPK.SEE: laser, 10 nmi, a pod).
fn pave_spike() -> SightSensor {
    SightSensor {
        laser: true,
        range_ft: 60_760.,
        gimbal: Gimbal::Hemisphere,
        eye: [0., -4., 0.],
        default_look: FIGHTER_DEFAULT_LOOK,
    }
}

fn give_sight(own: &mut Ownship, sensor: SightSensor) {
    own.config.sight = Some(sensor);
    own.sight = Some(Sight::new(sensor));
}

/// The fixture with a Pave Spike and a tank 20,000 ft ahead.
fn scene() -> State {
    let mut s = fixture(true);
    s.targets.clear();
    s.add_ground_target(
        TANK,
        OrientedBox {
            center: [0., 5., 20_000.],
            half: [8., 5., 15.],
            heading: 0.,
            pitch: 0.,
            bank: 0.,
        },
        100,
        0x400,
        REDFOR,
    )
    .unwrap();
    give_sight(s.own_mut(), pave_spike());
    s
}

fn tick(s: &mut State) {
    let inputs: Vec<OwnshipInput> = s
        .ownships
        .iter()
        .map(|own| OwnshipInput {
            aircraft: own.aircraft,
            held: false,
            launcher: launcher(),
        })
        .collect();
    s.step(&inputs, |_, _| 0.);
}

/// Makes station 0 a laser weapon (signature 1 with the designator flag).
fn laser_weapon(own: &mut Ownship) {
    let weapon = &mut own.config.stations[0].weapon;
    weapon.seeker.signature = 1;
    weapon.flags |= 0x400;
}

#[test]
fn slash_tracks_a_tank_which_becomes_the_designation_and_ctrl_l_lases_it() {
    let mut s = scene();
    tick(&mut s);
    assert_eq!(s.own().laser.state, LaserState::Off, "nothing held yet");
    s.command(0, Command::SightNext, launcher());
    tick(&mut s);
    let sight = s.own().sight.unwrap();
    assert_eq!(sight.mode, SightMode::Tracked(TANK));
    // The sight's track designates its object, whatever the radar sees.
    assert_eq!(s.own().designated(), Some(TANK));
    assert_eq!(s.own().laser.state, LaserState::Ready);
    // Ctrl+L lases it with no laser weapon selected.
    s.command(0, Command::ToggleLaser, launcher());
    tick(&mut s);
    assert_eq!(s.own().laser.state, LaserState::Lasing);
    // The spot follows the tank as it drives.
    for step in 1..=60 {
        let tank = s.targets.iter_mut().find(|t| t.id == TANK).unwrap();
        tank.position[0] += 5.;
        tank.velocity = [600., 0., 0.];
        let expected = tank.position;
        tick(&mut s);
        let spots = s.laser_spots();
        assert_eq!(spots.len(), 1, "step {step}");
        assert_eq!(spots[0].position, expected);
        assert_eq!(spots[0].target, Some(TANK));
        assert_eq!(spots[0].owner, 0);
    }
    // Ctrl+L again: ready, no spot.
    s.command(0, Command::ToggleLaser, launcher());
    tick(&mut s);
    assert_eq!(s.own().laser.state, LaserState::Ready);
    assert!(s.laser_spots().is_empty());
    // L drops the sight and the designation with it.
    s.command(0, Command::ClearDesignation, launcher());
    tick(&mut s);
    assert_eq!(s.own().laser.state, LaserState::Off);
    assert_eq!(s.own().designated(), None);
}

#[test]
fn the_laser_comes_on_with_a_laser_weapon_and_goes_with_it() {
    let mut s = scene();
    laser_weapon(s.own_mut());
    s.own_mut().armed = false;
    s.command(0, Command::SightNext, launcher());
    tick(&mut s);
    assert_eq!(s.own().laser.state, LaserState::Ready);
    // Selecting and arming the laser weapon lases (retail "automatically
    // activated").
    s.own_mut().armed = true;
    s.own_mut().selected = 0;
    tick(&mut s);
    assert_eq!(s.own().laser.state, LaserState::Lasing);
    // NAV: no laser weapon selected, none in flight.
    s.command(0, Command::SelectNav, launcher());
    tick(&mut s);
    assert_eq!(s.own().laser.state, LaserState::Ready);
}

#[test]
fn a_designated_ground_object_becomes_the_sights_track() {
    let mut s = scene();
    s.own_mut().sensors.select_ground(TANK);
    tick(&mut s);
    assert_eq!(s.own().sight.unwrap().target(), Some(TANK));
    // A pin drops the track and the designation that was the track.
    s.command(0, Command::SightPinGround, launcher());
    tick(&mut s);
    assert!(matches!(s.own().sight.unwrap().mode, SightMode::Pinned(_)));
    assert_eq!(s.own().designated(), None);
    // A destroyed tank leaves the sight pinned on the ground it looked at.
    s.own_mut().sensors.select_ground(TANK);
    tick(&mut s);
    s.targets.iter_mut().find(|t| t.id == TANK).unwrap().hp = 0;
    tick(&mut s);
    assert!(matches!(s.own().sight.unwrap().mode, SightMode::Pinned(_)));
}

#[test]
fn a_laser_weapon_homes_on_its_own_spot_first_else_a_buddys_of_its_side() {
    let mut s = scene();
    let config = s.own().config.clone();
    // Aircraft 1: a wingman on the same side with no designator. Aircraft
    // 2: the other side.
    let mut wingman = Ownship::new(1, s.own().side, config.clone(), true).unwrap();
    wingman.config.sight = None;
    wingman.sight = None;
    s.add_ownship(wingman).unwrap();
    s.add_ownship(Ownship::new(2, REDFOR, config, true).unwrap())
        .unwrap();
    let here = launcher().position;
    assert!(s.laser_spot_for(1, here).is_none(), "nobody lases yet");
    s.command(0, Command::SightNext, launcher());
    s.command(0, Command::ToggleLaser, launcher());
    tick(&mut s);
    let (own, mine) = s.laser_spot_for(0, here).unwrap();
    assert!(mine);
    assert_eq!(own.owner, 0);
    // The wingman's laser weapons use aircraft 0's spot: a buddy's.
    let (buddy, mine) = s.laser_spot_for(1, here).unwrap();
    assert!(!mine);
    assert_eq!(buddy, own);
    // The other side never uses it.
    assert!(s.laser_spot_for(2, here).is_none());
    assert!(s.buddy_spot(2, here).is_none());
}

#[test]
fn the_sight_and_the_laser_survive_a_checkpoint() {
    let mut s = scene();
    s.command(0, Command::SightNext, launcher());
    s.command(0, Command::ToggleLaser, launcher());
    tick(&mut s);
    let models = crate::checkpoint::Models::default();
    let own = s.own();
    assert_eq!(
        crate::checkpoint::round_trip(&own.sight, &models).unwrap(),
        own.sight
    );
    assert_eq!(
        crate::checkpoint::round_trip(&own.laser, &models).unwrap(),
        own.laser
    );
    let mut config = own.config.clone();
    config.sight = Some(SightSensor {
        gimbal: Gimbal::Cone {
            search: 1.,
            track: 0.75,
        },
        ..pave_spike()
    });
    config.designator_pods = vec![crate::combat::sight::DesignatorPod {
        station: 0,
        store: crate::combat::sight::DesignatorStore {
            source: "PAVEKNF.SEE".into(),
            name: "Pave Knife".into(),
            sensor: pave_spike(),
            weight_lbs: 425,
            flags: 1,
        },
    }];
    let restored = crate::checkpoint::round_trip(&config, &models).unwrap();
    assert_eq!(restored.sight, config.sight);
    assert_eq!(restored.designator_pods, config.designator_pods);
    assert_eq!(
        restored.sight_sensor(),
        Some(pave_spike()),
        "the pod comes first"
    );
}

/// A two-station loadout of the fixture: station 0 a pod station (0x400 with
/// the equipment bit 1, as A4E slot 5's 0x1785), station 1 a missile rail
/// without the pod bit.
fn loadout() -> crate::combat::loadout::Loadout {
    use tore_formats::aircraft::Hardpoint;
    let mut config = fixture(true).own().config.clone();
    config.stations.push(config.stations[0].clone());
    config.hardpoint_slots = vec![Some(0), Some(1)];
    let hardpoint = |flags| Hardpoint {
        location: 4,
        flags,
        position: [0; 3],
        store: None,
        count: 4,
        weight_class: 32,
    };
    crate::combat::loadout::Loadout {
        aircraft: config.aircraft,
        configuration: config,
        quantities: vec![0, 0],
        fuel_lbs: 1000.,
        internal_capacity_lbs: 1000.,
        empty_lbs: 10_000.,
        maximum_lbs: 40_000.,
        hardpoints: vec![hardpoint(0x1785), hardpoint(0x80)],
        tank_hardpoints: vec![],
        cheat: false,
    }
}

fn pave_knife() -> crate::combat::sight::DesignatorStore {
    crate::combat::sight::DesignatorStore {
        source: "PAVEKNF.SEE".into(),
        name: "Pave Knife".into(),
        sensor: pave_spike(),
        weight_lbs: 425,
        flags: 1,
    }
}

#[test]
fn a_designator_pod_loads_on_a_pod_station_and_gives_the_aircraft_its_sight() {
    let mut load = loadout();
    assert_eq!(load.configuration.sight_sensor(), None);
    // Only the pod station takes it, and only one.
    assert_eq!(load.designator_capacity(0, &pave_knife()), 1);
    assert_eq!(load.designator_capacity(1, &pave_knife()), 0);
    assert!(load.load_designator(1, pave_knife()).is_err());
    assert!(load.configuration.designator_pods.is_empty(), "refused");
    let before = load.configuration.external_equipment_lbs;
    load.quantities[0] = 1;
    load.load_designator(0, pave_knife()).unwrap();
    // The pod takes the station's place: no weapon, the pod's weight.
    assert_eq!(load.quantities, [0, 0]);
    assert_eq!(
        load.designator(0).map(|p| p.name.as_str()),
        Some("Pave Knife")
    );
    assert_eq!(load.configuration.external_equipment_lbs, before + 425);
    assert_eq!(load.configuration.sight_sensor(), Some(pave_spike()));
    load.validate().unwrap();
    // The ownship built from it has the sight and can lase.
    let own = Ownship::new(0, Side(1), load.configuration.clone(), true).unwrap();
    assert_eq!(own.sight.map(|s| s.sensor), Some(pave_spike()));
    // Loading a weapon on the station takes the pod off.
    load.quantities[0] = 1;
    load.refresh_equipment().unwrap();
    assert!(load.designator(0).is_none());
    assert_eq!(load.configuration.sight_sensor(), None);
    assert_eq!(load.configuration.external_equipment_lbs, before);
    // So does unloading it.
    load.quantities[0] = 0;
    load.load_designator(0, pave_knife()).unwrap();
    load.unload_designator(0).unwrap();
    assert!(load.configuration.designator_pods.is_empty());
    // Cheat loading puts it on any station.
    load.cheat = true;
    assert_eq!(load.designator_capacity(1, &pave_knife()), 1);
}
