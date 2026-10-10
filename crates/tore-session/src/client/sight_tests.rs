//! The AC-130 gunsight on the client (gunsight slice S4): the look turns
//! ahead of the host and corrects to it, and on the network simulator a
//! second player sees the gunship's barrels follow its sight. Synthetic
//! resources.

use super::prediction::Record;
use super::sight::{Corrected, SightPrediction, separation};
use super::tests::Rig;
use super::*;
use crate::host::StartMode;
use std::cell::Cell;
use std::collections::VecDeque;
use std::rc::Rc;
use tore_net::sim::LinkConfig;
use tore_sim::combat::gunship::{self, DEFAULT_LOOK, Sight};
use tore_sim::combat::live::{self, Readiness};
use tore_world::mission::Start;
use tore_world::readout::GunsightReadout;
use tore_world::test_support::resources::{THEATER, gunship_resources};

const MS: Duration = Duration::from_millis(1);

fn plane_state() -> flight::State {
    flight::State::new(&tore_world::test_support::profile(), [0., 5_000., 0.]).unwrap()
}

fn slew(sight: [i8; 2], zoom: u8) -> InputFrame {
    InputFrame::default().with_sight(sight, zoom)
}

fn host_shows(look: [f64; 2], sight: Sight, returning: bool) -> GunsightReadout {
    GunsightReadout {
        sight,
        look,
        returning,
        aim: None,
        impacts: [None; 3],
        impacts_tick: 0,
        status: [Readiness::NoTarget; 3],
        notice: None,
        zoom: 3,
    }
}

/// Steps `frames` from tick 1, keeping the predictor's history beside it.
fn predicted(frames: &[InputFrame]) -> (SightPrediction, VecDeque<Record>) {
    let mut sight = SightPrediction::default();
    let mut history = VecDeque::new();
    let plane = plane_state();
    for (i, frame) in frames.iter().enumerate() {
        let tick = i as u64 + 1;
        sight.step(tick, frame, &[], &plane);
        history.push_back(Record {
            tick,
            frame: *frame,
            commands: Vec::new(),
            hash: None,
        });
    }
    (sight, history)
}

#[test]
fn the_look_turns_by_the_sims_own_law_and_a_host_that_agrees_changes_nothing() {
    let frames: Vec<InputFrame> = (0..60)
        .map(|i| slew([127, if i < 30 { -64 } else { 0 }], 2))
        .collect();
    let (mut sight, history) = predicted(&frames);
    let mut expected = DEFAULT_LOOK;
    let mut at_20 = DEFAULT_LOOK;
    for (i, frame) in frames.iter().enumerate() {
        expected = gunship::slewed(expected, frame.sight.map(|v| f64::from(v) / 127.), 2);
        if i + 1 == 20 {
            at_20 = expected;
        }
    }
    assert_eq!(sight.look(), expected);
    // The readout of tick 21 shows the sight after the input of tick 20.
    let host = host_shows(at_20, Sight::Free, false);
    assert_eq!(
        sight.correct(21, &host, &history, &plane_state()),
        Corrected::Same
    );
    assert_eq!(sight.look(), expected);
    assert_eq!(sight.presented(), expected);
    // An older readout is not taken again.
    assert_eq!(
        sight.correct(21, &host, &history, &plane_state()),
        Corrected::Same
    );
}

#[test]
fn a_host_that_differs_restarts_the_turn_from_its_look() {
    let frames: Vec<InputFrame> = (0..60).map(|_| slew([127, 0], 1)).collect();
    let (mut sight, history) = predicted(&frames);
    // Far off (the host dropped ten ticks of slew): a snap, and the
    // prediction is the host's look turned by the 40 frames since.
    let host_look = [DEFAULT_LOOK[0] + 0.3, DEFAULT_LOOK[1]];
    let host = host_shows(host_look, Sight::Free, false);
    assert_eq!(
        sight.correct(21, &host, &history, &plane_state()),
        Corrected::Snapped
    );
    let mut expected = host_look;
    for _ in 21..=60 {
        expected = gunship::slewed(expected, [1., 0.], 1);
    }
    assert_eq!(sight.look(), expected);
    assert_eq!(sight.presented(), expected);
    assert_eq!(sight.after(60), Some(expected));
    // A small difference is drawn away over a few ticks.
    let near = [
        sight.after(40).unwrap()[0] + 0.001,
        sight.after(40).unwrap()[1],
    ];
    let mut history = history;
    let host = host_shows(near, Sight::Free, false);
    let before = sight.presented();
    assert_eq!(
        sight.correct(41, &host, &history, &plane_state()),
        Corrected::Blended
    );
    assert!(separation(sight.presented(), before) < 1e-12);
    assert!(separation(sight.look(), before) > 0.0009);
    let plane = plane_state();
    for tick in 61..=120 {
        let frame = InputFrame::default();
        sight.step(tick, &frame, &[], &plane);
        history.push_back(Record {
            tick,
            frame,
            commands: Vec::new(),
            hash: None,
        });
    }
    assert!(separation(sight.presented(), sight.look()) < 1e-6);
    assert_eq!((sight.snapped, sight.blended), (1, 1));
}

#[test]
fn l_is_predicted_and_a_track_holds_the_hosts_look() {
    let plane = plane_state();
    let mut sight = SightPrediction::default();
    let mut history = VecDeque::new();
    // The host pins the ground ahead of the slew.
    let pin = [0., 0., -5_000.];
    for tick in 1..=10 {
        sight.step(tick, &slew([0, 0], 3), &[], &plane);
        history.push_back(Record {
            tick,
            frame: slew([0, 0], 3),
            commands: Vec::new(),
            hash: None,
        });
    }
    let host = host_shows([0.1, -0.2], Sight::Pinned(pin), false);
    assert_eq!(
        sight.correct(11, &host, &history, &plane_state()),
        Corrected::Snapped
    );
    // L drops the pin to free slew where the view is: nothing moves.
    let drop = vec![Command::Seat(SeatCommand::Combat(
        live::Command::ClearDesignation,
    ))];
    let held = sight.look();
    sight.step(11, &InputFrame::default(), &drop, &plane);
    assert_eq!(sight.look(), held);
    // L again travels home at the slew speed, never a snap.
    sight.step(12, &InputFrame::default(), &drop, &plane);
    assert_eq!(sight.look(), gunship::homeward(held));
    // A track holds the host's look whatever is slewed.
    let track = host_shows([1., 0.5], Sight::Tracked(42), false);
    sight.correct(14, &track, &history, &plane_state());
    sight.step(13, &slew([127, 127], 3), &[], &plane);
    assert_eq!(sight.look(), [1., 0.5]);
}

/// Two AC-130s at 5,000 feet, flying from now on, over a link of this
/// round trip losing `loss` each way.
fn gunships(round_trip_ms: u64, loss: f64) -> Rig {
    let mut spec = tore_world::mission::MissionSpec::new(THEATER, AircraftId::Ac130);
    spec.wings[0].count = 2;
    spec.start = Start::Airborne { altitude_ft: 5_000 };
    Rig::with_import(
        spec,
        LinkConfig::for_round_trip(
            MS * round_trip_ms as u32,
            0.1,
            loss,
            if loss > 0. { 0.01 } else { 0. },
        ),
        21,
        gunship_resources(),
        |config| config.start = StartMode::Now,
    )
}

/// The gunner: slews right and down for three seconds at zoom step 2 and
/// holds. Once `pin` is set it pins the ground under the crosshair, fires
/// for a second and then slews the pin left. Ticks count from its first
/// predicted tick.
fn gunner(pin: Rc<Cell<bool>>) -> tests::Script {
    let mut first = None;
    let mut pinned_at = None;
    Box::new(move |_, client, _| {
        let Some(tick) = client.prediction().map(|p| p.tick()) else {
            return Controls::default();
        };
        let d = tick - *first.get_or_insert(tick);
        let mut controls = Controls {
            sight_zoom: 2,
            ..Controls::default()
        };
        if d < 360 {
            controls.sight = [127, -30];
        }
        match pinned_at {
            None if pin.get() => {
                pinned_at = Some(tick);
                controls.commands = vec![SeatCommand::Combat(live::Command::SightPinGround)];
            }
            Some(at) => match tick - at {
                120..220 => controls.trigger = true,
                240..360 => controls.sight = [-60, 0],
                _ => {}
            },
            None => {}
        }
        controls
    })
}

fn gun_devices(picture: &RenderSnapshot, plane: u32) -> Option<[f64; 6]> {
    let devices = picture.targets.iter().find(|t| t.id == plane)?.devices?;
    Some(std::array::from_fn(|i| {
        devices[tore_world::snapshot::GUN_AIM + i]
    }))
}

#[test]
fn a_second_player_sees_the_gunships_barrels_follow_its_sight() {
    barrels_follow_the_sight(gunships(80, 0.), 0);
}

/// The network matrix's worst cell (300 ms, 5 percent lost each way, 1
/// percent duplicated): the same, with a snap allowed for an input the
/// host had to repeat.
#[test]
fn the_barrels_follow_the_sight_over_a_slow_lossy_link() {
    barrels_follow_the_sight(gunships(300, 0.05), 2);
}

/// A gunner slews, holds, pins, fires and slews the pin; a watcher flies
/// beside it. The host's train, the gunner's readout and the watcher's
/// barrels agree, and the gunner's camera turned ahead of the host and
/// snapped at most `snaps` times.
fn barrels_follow_the_sight(mut rig: Rig, snaps: u64) {
    let pinned = Rc::new(Cell::new(false));
    let gunner_player = rig.join(|c| c.plane = Some(0), gunner(Rc::clone(&pinned)));
    let watcher = rig.join(|c| c.plane = Some(1), tests::level_script());
    assert!(
        rig.run_until(Duration::from_secs(5), |r| r.seated(gunner_player)
            && r.seated(watcher))
    );
    rig.players[gunner_player].keep_frames = true;
    rig.players[watcher].keep_frames = true;
    let host_guns = |rig: &Rig| {
        rig.host
            .world()
            .combat
            .state
            .ownship(0)
            .and_then(|own| own.gunship.clone())
            .expect("the gunship's guns")
    };
    let start = host_guns(&rig);
    // The slew and the hold.
    rig.run(Duration::from_secs(4));
    let slewed = host_guns(&rig);
    assert!(
        separation(slewed.look, start.look) > 0.5,
        "{:?}",
        slewed.look
    );
    assert_eq!(slewed.sight, Sight::Free);
    // The watcher saw the barrels turn as the sight slewed.
    let watched: Vec<[f64; 6]> = rig.players[watcher]
        .frames
        .iter()
        .filter_map(|f| gun_devices(&f.picture, 0))
        .collect();
    assert!(watched.len() > 100);
    let swing = |a: &[f64; 6], b: &[f64; 6]| (0..6).map(|i| (a[i] - b[i]).abs()).fold(0., f64::max);
    assert!(
        swing(watched.first().unwrap(), watched.last().unwrap()) > 0.05,
        "{:?} to {:?}",
        watched.first(),
        watched.last()
    );
    // Settled: the host's train, the gunner's readout and the watcher's
    // barrels agree within the wire's step and the drawn delay.
    let host = slewed.normalized_devices();
    let last = rig.players[gunner_player].frames.last().unwrap();
    let readout = last.readout.as_ref().unwrap();
    assert!(
        swing(&readout.stores.gun_aim, &host) < 0.03,
        "{:?} {host:?}",
        readout.stores.gun_aim
    );
    assert!(
        swing(watched.last().unwrap(), &host) < 0.03,
        "{:?} {host:?}",
        watched.last()
    );
    // The gunner's camera turned with its own hand and ends on the host's.
    let gunsight = readout.gunsight.as_ref().unwrap();
    assert!(
        separation(gunsight.look, slewed.look) < 1e-3,
        "{:?} {:?}",
        gunsight.look,
        slewed.look
    );
    let prediction = rig.players[gunner_player]
        .client
        .sight_prediction()
        .unwrap();
    assert!(
        prediction.snapped <= snaps,
        "snapped {}",
        prediction.snapped
    );

    // Pin, fire and slew the pin: the host pins, the client follows.
    pinned.set(true);
    rig.run(Duration::from_secs(4));
    let after = host_guns(&rig);
    assert!(matches!(after.sight, Sight::Pinned(_)), "{:?}", after.sight);
    assert_ne!(after.headings, slewed.headings);
    let last = rig.players[gunner_player].frames.last().unwrap();
    let gunsight = last.readout.as_ref().unwrap().gunsight.as_ref().unwrap();
    assert!(matches!(gunsight.sight, Sight::Pinned(_)));
    let prediction = rig.players[gunner_player]
        .client
        .sight_prediction()
        .unwrap();
    // The pin is the one change of mode the client cannot predict: at most
    // a small slide, never a snap.
    assert!(
        prediction.snapped <= snaps,
        "snapped {}",
        prediction.snapped
    );
    assert!(
        prediction.blended <= 2 + snaps,
        "blended {}",
        prediction.blended
    );
    let watched_now = gun_devices(&rig.players[watcher].frames.last().unwrap().picture, 0).unwrap();
    assert!(swing(&watched_now, &after.normalized_devices()) < 0.05);
}

#[test]
fn the_predicted_camera_never_leaves_the_hemisphere_below_the_aircraft() {
    // Slewing up at the widest step passes the horizon in a second and holds
    // there; no tick, and no drawn offset, is above it.
    let frames: Vec<InputFrame> = (0..300).map(|_| slew([30, 127], 1)).collect();
    let mut sight = SightPrediction::default();
    let plane = plane_state();
    for (i, frame) in frames.iter().enumerate() {
        sight.step(i as u64 + 1, frame, &[], &plane);
        assert!(sight.look()[1] <= gunship::GIMBAL_TOP, "tick {i}");
        assert!(sight.presented()[1] <= gunship::GIMBAL_TOP, "tick {i}");
    }
    assert_eq!(sight.look()[1], gunship::GIMBAL_TOP);
    // The heading kept turning against the limit.
    assert!(separation([sight.look()[0], 0.], [DEFAULT_LOOK[0], 0.]) > 0.1);
    // A drawn correction cannot carry it above either.
    let (mut sight, history) = predicted(&frames);
    let host = host_shows([0.5, -0.001], Sight::Free, false);
    sight.correct(41, &host, &history, &plane);
    assert!(sight.presented()[1] <= gunship::GIMBAL_TOP);
    assert!(sight.look()[1] <= gunship::GIMBAL_TOP);
}

#[test]
fn a_pin_above_the_hemisphere_leaves_the_predicted_camera_at_the_limit() {
    let plane = plane_state();
    // High above the aircraft: the camera can only look at its horizon.
    let pin = [
        plane.position[0] - 3_000.,
        plane.position[1] + 4_000.,
        plane.position[2],
    ];
    let mut sight = SightPrediction::default();
    let host = host_shows(
        [-std::f64::consts::FRAC_PI_2, 0.],
        Sight::Pinned(pin),
        false,
    );
    sight.correct(5, &host, &VecDeque::new(), &plane);
    assert_eq!(sight.look()[1], 0.);
    // Slewing up goes nowhere; slewing down turns the bearing but the camera
    // stays on the limit while the pin is still above it.
    for tick in 6..20 {
        sight.step(tick, &slew([0, 127], 3), &[], &plane);
        assert_eq!(sight.look()[1], 0.);
    }
    for tick in 20..40 {
        sight.step(tick, &slew([0, -127], 3), &[], &plane);
        assert!(sight.look()[1] <= 0.);
        assert!(sight.presented()[1] <= 0.);
    }
    // A pin below the horizon is looked at, from the dome rather than the
    // aircraft's centre.
    let low = [
        plane.position[0] - 3_000.,
        plane.position[1] - 3_000.,
        plane.position[2],
    ];
    let mut sight = SightPrediction::default();
    let host = host_shows(
        [-std::f64::consts::FRAC_PI_2, -0.8],
        Sight::Pinned(low),
        false,
    );
    sight.correct(5, &host, &VecDeque::new(), &plane);
    let launcher = tore_world::combat::launcher(&plane);
    let from_eye = gunship::body_angles(
        launcher,
        std::array::from_fn(|i| low[i] - gunship::eye_position(launcher)[i]),
    );
    assert_eq!(sight.look(), from_eye);
    assert!(from_eye[1] < 0.);
}
