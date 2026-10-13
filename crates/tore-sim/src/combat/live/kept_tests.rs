//! What a human leaves set in a cockpit it gives back to the AI (slice
//! FX-J; docs/ARCHITECTURE.md, "What a returning player keeps"): every part
//! comes back whole on the ownship built for the next human, each is checked
//! against the aircraft as it is then, the sight and the laser step on while
//! the AI flies (a laser bomb the human left guiding keeps its spot), an
//! aircraft that is lost takes its entry with it, and a checkpoint carries
//! the entries.
use super::guided_tests::gbu10;
use super::tests::fixture;
use super::*;
use crate::airport::OrientedBox;
use crate::combat::delivery::DeliveryMode;
use crate::combat::laser::LaserState;
use crate::combat::sight::{FIGHTER_DEFAULT_LOOK, Gimbal, Sight, SightMode, SightSensor};

const TANK: u32 = 0x5000_0001;
const REDFOR: Side = Side(2);
const KNOT: f64 = 1.687_8;

fn pave_spike() -> SightSensor {
    SightSensor {
        laser: true,
        range_ft: 60_760.,
        gimbal: Gimbal::Hemisphere,
        eye: [0., -4., 0.],
        default_look: FIGHTER_DEFAULT_LOOK,
    }
}

/// Level at 8,000 ft heading north at 450 kt, `seconds` into the run.
fn launcher_after(seconds: f64) -> Launcher {
    let basis = Basis::new(0., 0., 0.);
    let speed = 450. * KNOT;
    Launcher {
        position: [0., 8_000., speed * seconds],
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

/// Aircraft 0 with a Pave Spike and four GBU-10s selected and armed, and a
/// tank 12,000 ft north.
fn scene() -> State {
    let mut config = fixture(false).own().configuration().clone();
    let mut gun = config.stations[0].clone();
    gun.weapon.source = tore_formats::aircraft::AircraftId::F18
        .gun()
        .unwrap()
        .into();
    config.stations = vec![
        gun,
        Station {
            weapon: gbu10(),
            mount: [0.; 3],
            count: 4,
            internal: false,
            rack: Default::default(),
        },
    ];
    config.sight = Some(pave_spike());
    let mut s = State::new(config, true).unwrap();
    s.targets.clear();
    s.add_ground_target(
        TANK,
        OrientedBox {
            center: [0., 5., 12_000.],
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
    let own = s.own_mut();
    own.sight = Some(Sight::new(pave_spike()));
    own.armed = true;
    own.selected = 1;
    s
}

/// One tick: the ownships fly the run at `seconds`; an AI-flown aircraft 0
/// (its row) flies it too, as the AI would.
fn tick(s: &mut State, held: bool, seconds: f64) {
    let launcher = launcher_after(seconds);
    if let Some(row) = s.targets.iter_mut().find(|t| t.id == 0) {
        row.position = launcher.position;
        row.velocity = launcher.velocity;
        row.basis = launcher.basis;
    }
    let inputs: Vec<OwnshipInput> = s
        .ownships
        .iter()
        .map(|own| OwnshipInput {
            aircraft: own.aircraft,
            held,
            launcher,
        })
        .collect();
    s.step(&inputs, |_, _| 0.);
}

/// The human gives aircraft 0 back: its row joins the targets, its cockpit
/// is kept.
fn give_back(s: &mut State, seconds: f64) {
    let own = s.remove_ownship(0).unwrap();
    let kept = own.kept();
    let back = own.into_ai(AiPose {
        launcher: launcher_after(seconds),
        on_ground: false,
    });
    s.targets.push(back.row);
    s.keep(0, kept);
}

/// A human takes aircraft 0 again: a fresh ownship, the kept cockpit put
/// back on it, its row gone.
fn take(s: &mut State) {
    let kept = s.take_kept(0).expect("a cockpit is kept for aircraft 0");
    let row = s.targets.iter().position(|t| t.id == 0).unwrap();
    s.targets.remove(row);
    let mut own = Ownship::new(0, kept.side, scene().own().config.clone(), true).unwrap();
    own.restore(&kept, false, false);
    s.add_ownship(own).unwrap();
}

#[test]
fn every_part_of_a_kept_cockpit_comes_back() {
    let mut s = scene();
    s.own_mut().sensors.select_ground(TANK);
    tick(&mut s, false, 0.);
    let own = s.own_mut();
    own.launch_mode = LaunchMode::Boresight;
    own.delivery_mode = DeliveryMode::Level;
    own.ever_loaded[0] = false;
    own.shots = 3;
    own.hits = 2;
    own.kills = 1;
    own.laser.manual = Some(true);
    let sight = own.sight.as_mut().unwrap();
    sight.input.deflection = [40, -12];
    sight.input.zoom = 4;
    sight.look = [0.2, -0.4];
    assert!(own.sensors.toggle_air_ground() || own.sensors.air_ground());
    let before = own.clone();
    let kept = before.kept();
    assert_eq!(
        kept.sight.unwrap().input.deflection,
        [0; 2],
        "the slew is let go"
    );
    assert_eq!(kept.sight.unwrap().input.zoom, 4);
    let mut fresh = Ownship::new(0, before.side, before.config.clone(), true).unwrap();
    assert_ne!(fresh.selected, before.selected);
    fresh.restore(&kept, false, false);
    assert_eq!(
        (fresh.selected, fresh.armed),
        (before.selected, before.armed),
        "the weapon"
    );
    assert_eq!(fresh.launch_mode, LaunchMode::Boresight);
    assert_eq!(fresh.delivery_mode, DeliveryMode::Level);
    assert_eq!(fresh.ever_loaded, before.ever_loaded);
    let (a, b) = (fresh.sight.unwrap(), before.sight.unwrap());
    assert_eq!(
        (a.mode, a.look, a.input.zoom),
        (b.mode, b.look, b.input.zoom)
    );
    assert_eq!(a.mode, SightMode::Tracked(TANK));
    assert_eq!(fresh.laser, before.laser);
    assert_eq!(fresh.sensors.picks(), before.sensors.picks());
    assert_eq!(fresh.designated(), Some(TANK));
    assert!(fresh.sensors.air_ground());
    assert_eq!(
        (fresh.hud_selection, fresh.sight_hold),
        (before.hud_selection, before.sight_hold)
    );
    assert_eq!((fresh.shots, fresh.hits, fresh.kills), (3, 2, 1));
}

#[test]
fn a_kept_cockpit_is_checked_against_the_aircraft_as_it_is_now() {
    let mut s = scene();
    tick(&mut s, false, 0.);
    let kept = s.own().kept();
    let config = s.own().config.clone();
    // The bombs went while the AI flew: the selection advances, as from an
    // emptied station.
    let mut emptied = Ownship::new(0, Side(1), config.clone(), true).unwrap();
    emptied.ammo[1] = 0;
    emptied.restore(&kept, false, false);
    assert!(emptied.armed && emptied.selected == 0, "on to the gun");
    // Guns only: the bomb selection moves to the gun.
    let mut guns = Ownship::new(0, Side(1), config.clone(), true).unwrap();
    guns.restore(&kept, true, false);
    assert_eq!((guns.armed, guns.selected), (true, 0));
    // An aircraft without the sight takes none back, nor the laser.
    let mut plain = config;
    plain.sight = None;
    let mut bare = Ownship::new(0, Side(1), plain, true).unwrap();
    let mut lasing = kept.clone();
    lasing.laser.manual = Some(true);
    bare.restore(&lasing, false, false);
    assert!(bare.sight.is_none());
    assert_eq!(bare.laser, Default::default());
}

/// The human releases a laser bomb on the tank and drops: the AI flies the
/// aircraft on the same line, the laser the human left keeps the spot on the
/// tank, the bomb follows it all the way, and the human who takes the
/// aircraft back finds the laser lasing. Without the kept laser the bomb
/// would go blind (the old rule, checked against the same run).
#[test]
fn a_laser_bomb_follows_the_kept_laser_while_the_ai_flies_and_after_the_return() {
    let mut runs = Vec::new();
    for keep in [true, false] {
        let mut s = scene();
        s.own_mut().sensors.select_ground(TANK);
        tick(&mut s, false, 0.);
        tick(&mut s, false, 1. / 120.);
        assert_eq!(s.own().laser.state, LaserState::Lasing, "automatic rule");
        tick(&mut s, true, 2. / 120.);
        let (id, flight) = s.guided_flights().next().map(|(i, f)| (i, *f)).unwrap();
        assert_eq!(flight.mode, GuidedMode::Guiding);
        let mut t = 3u32;
        let mut seconds = || {
            t += 1;
            f64::from(t) / 120.
        };
        let mut at = 0.;
        for _ in 0..120 {
            at = seconds();
            tick(&mut s, false, at);
        }
        give_back(&mut s, at);
        if !keep {
            s.take_kept(0);
        }
        let mut modes = Vec::new();
        for _ in 0..(3 * 120) {
            tick(&mut s, false, seconds());
            if let Some(f) = s.guided_flight(id) {
                modes.push(f.mode);
            }
        }
        if keep {
            assert!(modes.iter().all(|m| *m == GuidedMode::Guiding), "{modes:?}");
            let spots = s.laser_spots();
            assert_eq!(spots.len(), 1);
            assert_eq!((spots[0].owner, spots[0].target), (0, Some(TANK)));
            take(&mut s);
            assert_eq!(s.own().laser.state, LaserState::Lasing);
            assert_eq!(s.own().sight.unwrap().mode, SightMode::Tracked(TANK));
            assert!(s.kept(0).is_none(), "taken");
        } else {
            assert_eq!(modes.last(), Some(&GuidedMode::Blind));
        }
        let mut destroyed = false;
        for _ in 0..(60 * 120) {
            if s.guided_flight(id).is_none() {
                break;
            }
            tick(&mut s, false, seconds());
            destroyed |= s.targets.iter().any(|t| t.id == TANK && t.hp <= 0);
        }
        runs.push(destroyed);
    }
    assert_eq!(runs, [true, false], "kept: a hit; not kept: blind");
}

/// A kept laser off by hand stays off while the AI flies: the bomb goes
/// blind, as the human chose.
#[test]
fn a_laser_left_off_by_hand_stays_off_while_the_ai_flies() {
    let mut s = scene();
    s.own_mut().sensors.select_ground(TANK);
    tick(&mut s, false, 0.);
    tick(&mut s, true, 1. / 120.);
    let (id, _) = s.guided_flights().next().map(|(i, f)| (i, *f)).unwrap();
    s.command(0, Command::ToggleLaser, launcher_after(2. / 120.));
    give_back(&mut s, 2. / 120.);
    for t in 3..60 {
        tick(&mut s, false, f64::from(t) / 120.);
    }
    assert_eq!(s.kept(0).unwrap().laser.manual, Some(false));
    assert!(s.laser_spots().is_empty());
    assert_eq!(s.guided_flight(id).unwrap().mode, GuidedMode::Blind);
}

/// The entry goes with an aircraft that is lost while the AI flies it, and a
/// checkpoint carries the entries whole.
#[test]
fn a_kept_cockpit_survives_a_checkpoint_and_goes_with_a_lost_aircraft() {
    let mut s = scene();
    s.own_mut().sensors.select_ground(TANK);
    tick(&mut s, false, 0.);
    give_back(&mut s, 0.);
    tick(&mut s, false, 1. / 120.);
    let models = crate::checkpoint::Models::default();
    assert_eq!(
        crate::checkpoint::round_trip(&s.kept, &models).unwrap(),
        s.kept
    );
    s.targets.iter_mut().find(|t| t.id == 0).unwrap().hp = 0;
    tick(&mut s, false, 2. / 120.);
    assert!(s.kept(0).is_none());
}
