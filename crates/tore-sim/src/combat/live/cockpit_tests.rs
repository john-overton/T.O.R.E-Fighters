//! The strike cockpit (slice ST-C; docs/spec/ai-strike.md, "One cockpit, two
//! pilots"): one record for a human, a plane a human gave back and an AI
//! striker, and one release for both pilots. An AI release of a Mk 82 pack,
//! a rocket pod, a GBU-10 on its own spot and a JDAM leaves exactly the
//! rounds, bomb paths and guided flights a human's trigger leaves from the
//! same cockpit and pose.
use super::bomb_tests::{lau61, mk82};
use super::guided_tests::{gbu10, gbu29};
use super::tests::fixture;
use super::*;
use crate::airport::OrientedBox;
use crate::combat::delivery::DeliveryMode;
use crate::combat::laser::LaserState;
use crate::combat::sight::{FIGHTER_DEFAULT_LOOK, Gimbal, SightMode, SightSensor};

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

/// An aircraft carrying four of `weapon` on one station (with a Pave Spike
/// when `designator`).
fn config(weapon: Weapon, designator: bool) -> Configuration {
    let mut config = fixture(false).own().configuration().clone();
    config.stations = vec![Station {
        weapon,
        mount: [2., -3., 1.],
        count: 4,
        internal: false,
        rack: Default::default(),
    }];
    config.sight = designator.then(pave_spike);
    config
}

/// Level at 8,000 ft heading north at 450 kt, standing where it is (each
/// tick starts from the same pose, so both pilots release from it).
fn launcher() -> Launcher {
    let basis = Basis::new(0., 0., 0.);
    let speed = 450. * KNOT;
    Launcher {
        position: [0., 8_000., 0.],
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

fn with_tank(mut s: State) -> State {
    s.targets
        .retain(|t| t.id == 0 && t.role == TargetRole::Aircraft);
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
    s
}

/// A human flies aircraft 0 with the store selected and armed.
fn human(weapon: Weapon, designator: bool) -> State {
    let mut s = with_tank(State::new(config(weapon, designator), true).unwrap());
    let own = s.own_mut();
    own.armed = true;
    own.selected = 0;
    s
}

/// The AI flies aircraft 0 (a row standing at the launcher) with a cockpit
/// of its own, the store selected and armed, the tank its target.
fn ai(weapon: Weapon, designator: bool) -> State {
    let config = config(weapon, designator);
    let mut s = State::open_mission();
    let l = launcher();
    s.add_dummy(&config, l.position, l.basis, DEFAULT_OWNSHIP_SIDE);
    let mut s = with_tank(s);
    let cockpit = s
        .ai_cockpit_mut(0, DEFAULT_OWNSHIP_SIDE, &config)
        .expect("the AI flies aircraft 0");
    cockpit.selected = 0;
    cockpit.armed = true;
    cockpit.target = Some(StrikeTarget::Object(TANK));
    s
}

/// One tick from the launcher's pose: the human's trigger is `held`; the
/// AI's row stands at the same pose.
fn tick(s: &mut State, held: bool) -> Vec<Event> {
    let l = launcher();
    if let Some(row) = s.targets.iter_mut().find(|t| t.id == 0) {
        row.position = l.position;
        row.velocity = l.velocity;
        row.basis = l.basis;
    }
    let inputs: Vec<OwnshipInput> = s
        .ownships
        .iter()
        .map(|own| OwnshipInput {
            aircraft: own.aircraft,
            held,
            launcher: l,
        })
        .collect();
    s.step(&inputs, |_, _| 0.)
}

/// The AI's release of station 0, numbered from 1 << 24 as the AI's rounds
/// are.
fn ai_release(s: &mut State, config: &Configuration) -> std::result::Result<Vec<u32>, Readiness> {
    let mut next = 1u32 << 24;
    s.release_strike(
        StrikeRelease {
            aircraft: 0,
            config,
            station: 0,
            launcher: launcher(),
            incoming: None,
            // What the human's station holds after the same release.
            rounds_after: {
                let s = &config.stations[0];
                (u32::from(s.count) * super::pod_rounds(&s.weapon))
                    .saturating_sub(u32::from(s.weapon.burst.actual_rounds_per_game).max(1))
            },
        },
        || {
            let id = next;
            next += 1;
            id
        },
        &|_, _| 0.,
    )
}

/// What a round is, apart from its number and whose record it reads.
fn shape(p: &Projectile) -> String {
    format!(
        "{:?} {:?}",
        (
            p.owner,
            p.station,
            p.position,
            p.previous,
            p.direction,
            p.speed_f8,
            p.motion,
        ),
        (
            p.age,
            p.launched_t,
            p.target,
            p.guidance.is_some(),
            p.incoming,
            p.gun_round,
            p.tracer,
        )
    )
}

/// The rounds, bomb paths and guided flights in flight, in release order. A
/// bomb is told by where and how fast it left, not by the way it flies on:
/// every bomb takes its own scatter, drawn from the tick and its round number
/// (slice ST-LV), so a human's and an AI's bombs leave alike and fly apart.
fn released(s: &State) -> Vec<String> {
    s.projectiles
        .iter()
        .map(|p| match s.bomb_motion(p.id) {
            Some(motion) => format!(
                "{:?} {:?} {:?}",
                (p.owner, p.station, p.previous, p.launched_t, p.age),
                (p.target, p.guidance.is_some(), p.incoming, p.gun_round),
                (
                    motion.origin,
                    // The scatter turns the release without changing its
                    // speed.
                    (dot(motion.velocity(0.), motion.velocity(0.)).sqrt() * 1e6).round() / 1e6,
                    s.guided_flight(p.id),
                ),
            ),
            None => format!("{} None {:?}", shape(p), s.guided_flight(p.id)),
        })
        .collect()
}

/// A human's press and an AI's release from the same cockpit and pose,
/// after `warm` quiet ticks (so the sights and lasers settle), and the tick
/// that flies the rounds: both leave the same rounds.
fn same_release(
    weapon: Weapon,
    designator: bool,
    warm: usize,
    prepare: impl Fn(&mut State, &Configuration),
) {
    let config = config(weapon.clone(), designator);
    let mut person = human(weapon.clone(), designator);
    let mut robot = ai(weapon, designator);
    prepare(&mut person, &config);
    prepare(&mut robot, &config);
    for _ in 0..warm {
        tick(&mut person, false);
        tick(&mut robot, false);
    }
    tick(&mut person, true);
    let ids = ai_release(&mut robot, &config).expect("the AI's release is ready");
    tick(&mut robot, false);
    assert!(!ids.is_empty());
    assert_eq!(
        robot.projectiles.iter().map(|p| p.id).collect::<Vec<_>>(),
        ids
    );
    assert!(
        robot
            .projectiles
            .iter()
            .all(|p| p.weapon.as_ref() == Some(&config.stations[0].weapon)),
        "an AI round carries its record"
    );
    assert_eq!(released(&person), released(&robot));
}

/// The human designates the tank; the AI's cockpit already has it.
fn designate(s: &mut State, _: &Configuration) {
    if let Some(own) = s.ownships.first_mut() {
        own.sensors.select_ground(TANK);
    }
}

/// The AI lases by hand (the human's laser runs on the automatic rule).
fn designate_and_lase(s: &mut State, config: &Configuration) {
    designate(s, config);
    if s.ownships.is_empty() {
        s.ai_cockpit_mut(0, DEFAULT_OWNSHIP_SIDE, config)
            .unwrap()
            .laser
            .manual = Some(true);
    }
}

#[test]
fn a_human_a_kept_cockpit_and_an_ai_striker_hold_one_record() {
    let mut s = human(gbu10(), true);
    s.own_mut().sensors.select_ground(TANK);
    tick(&mut s, false);
    // The human's cockpit is the ownship's record; the designation is its
    // target.
    let flying: StrikeCockpit = s.own().cockpit.clone();
    assert_eq!(s.strike_cockpit(0), Some(&flying));
    assert_eq!(flying.target, Some(StrikeTarget::Object(TANK)));
    assert_eq!(flying.sight.unwrap().mode, SightMode::Tracked(TANK));
    assert_eq!(s.own().selected, flying.selected, "an ownship reads as it");
    let config = s.own().configuration().clone();
    assert!(
        s.ai_cockpit_mut(0, Side(1), &config).is_none(),
        "the AI has no cockpit while a human flies"
    );
    // Given back: the kept cockpit is the same record, and the AI flies with
    // it.
    let own = s.remove_ownship(0).unwrap();
    s.keep(0, own.kept());
    s.targets.push(
        own.into_ai(AiPose {
            launcher: launcher(),
            on_ground: false,
        })
        .row,
    );
    let kept: &StrikeCockpit = &s.kept(0).unwrap().cockpit;
    assert_eq!(kept.target, flying.target);
    assert_eq!(
        (
            kept.selected,
            kept.armed,
            kept.launch_mode,
            kept.delivery_mode
        ),
        (
            flying.selected,
            flying.armed,
            flying.launch_mode,
            flying.delivery_mode
        )
    );
    assert_eq!(kept.laser, flying.laser);
    assert_eq!(s.strike_cockpit(0), Some(kept));
    s.ai_cockpit_mut(0, Side(1), &config).unwrap().delivery_mode = DeliveryMode::Level;
    assert_eq!(
        s.kept(0).unwrap().delivery_mode,
        DeliveryMode::Level,
        "the AI flying a given-back plane edits the kept cockpit"
    );
    assert_eq!(s.ai_cockpits().count(), 0);
    // An AI striker that never had a human gets its own, made once.
    let striker = s.ai_cockpit_mut(9, REDFOR, &config).unwrap();
    assert!(!striker.armed && striker.sight.is_some());
    striker.target = Some(StrikeTarget::Point([1., 2., 3.]));
    assert_eq!(
        s.strike_cockpit(9).unwrap().target,
        Some(StrikeTarget::Point([1., 2., 3.]))
    );
    assert_eq!(s.ai_cockpits().count(), 1);
}

#[test]
fn an_ai_mk82_pack_leaves_as_a_humans_does() {
    let mut pack = mk82();
    pack.burst.game_rounds_in_burst = 6;
    same_release(pack, false, 2, designate);
}

#[test]
fn an_ai_rocket_pod_fires_one_rocket_as_a_humans_does() {
    same_release(lau61(), false, 2, designate);
}

#[test]
fn an_ai_gbu10_on_its_own_spot_leaves_guiding_as_a_humans_does() {
    same_release(gbu10(), true, 3, designate_and_lase);
    // The AI's sight took its target and its laser lights it; the bomb
    // follows that laser to the tank.
    let config = config(gbu10(), true);
    let mut robot = ai(gbu10(), true);
    designate_and_lase(&mut robot, &config);
    for _ in 0..3 {
        tick(&mut robot, false);
    }
    let striker = robot.strike_cockpit(0).unwrap();
    assert_eq!(striker.sight.unwrap().mode, SightMode::Tracked(TANK));
    assert_eq!(striker.laser.state, LaserState::Lasing);
    let ids = ai_release(&mut robot, &config).unwrap();
    let flight = robot.guided_flight(ids[0]).unwrap();
    assert_eq!(
        (flight.mode, flight.spot_owner),
        (GuidedMode::Guiding, Some(0))
    );
    let mut destroyed = false;
    for _ in 0..(60 * 120) {
        if robot.projectiles.is_empty() {
            break;
        }
        destroyed |= tick(&mut robot, false)
            .iter()
            .any(|e| matches!(e, Event::Destroyed(TANK)));
    }
    assert!(destroyed, "the AI's laser bomb missed");
}

#[test]
fn an_ai_with_no_laze_support_drops_a_laser_bomb_by_hand_like_a_dumb_bomb() {
    // Slice ST-G; John, 2026-10-12: laser bombs dropped like dumb bombs.
    let ground = |_: f64, _: f64| 0.;
    let config = config(gbu10(), false);
    let mut robot = ai(gbu10(), false);
    tick(&mut robot, false);
    // A guided bomb has no bomb sight for a human; the AI that drops it by
    // hand gets the bomb equation's CCIP.
    assert!(
        robot
            .strike_delivery(
                0,
                &config,
                &launcher(),
                next(&robot, &config),
                false,
                &ground
            )
            .is_none()
    );
    let solution = robot
        .strike_delivery(
            0,
            &config,
            &launcher(),
            next(&robot, &config),
            true,
            &ground,
        )
        .expect("a bomb solution");
    let ccip = solution.ccip.expect("a CCIP");
    let release = crate::combat::ballistics::ReleaseState::from_mount(
        &launcher(),
        &config.stations[0],
        next(&robot, &config),
        0,
    );
    assert_eq!(
        ccip.impact,
        crate::combat::ballistics::predict_impact(release, ground)
            .unwrap()
            .position
    );
    // Due where that impact meets the aim, judged on the ballistic sight.
    let tank = robot
        .targets
        .iter()
        .find(|t| t.id == TANK)
        .unwrap()
        .position;
    let onto = ReleaseAim {
        offset_ft: [ccip.impact[0] - tank[0], ccip.impact[2] - tank[2]],
        gate_ft: 20.,
        ballistic: true,
    };
    assert!(robot.strike_release_due(
        0,
        &config,
        &launcher(),
        next(&robot, &config),
        onto,
        &ground
    ));
    let short = ReleaseAim {
        offset_ft: [onto.offset_ft[0], onto.offset_ft[1] + 500.],
        ..onto
    };
    assert!(!robot.strike_release_due(
        0,
        &config,
        &launcher(),
        next(&robot, &config),
        short,
        &ground
    ));
    // Let go with no spot, it waits for one and falls on the bomb equation
    // from the release velocity turned by the dumb bomb's scatter, each
    // bomb its own.
    let mut ids = Vec::new();
    for first_id in [1u32 << 24, (1 << 24) + 7] {
        let mut next = first_id;
        let released = robot
            .release_strike(
                StrikeRelease {
                    aircraft: 0,
                    config: &config,
                    station: 0,
                    launcher: launcher(),
                    incoming: None,
                    rounds_after: 0,
                },
                || {
                    next += 1;
                    next - 1
                },
                &ground,
            )
            .unwrap();
        ids.push(released[0]);
    }
    let flights: Vec<_> = ids
        .iter()
        .map(|id| robot.guided_flight(*id).expect("a guided flight"))
        .collect();
    let launch = crate::combat::guided::release_velocity(&gbu10(), &launcher());
    let mut turns = Vec::new();
    for flight in &flights {
        assert_eq!(flight.mode, GuidedMode::Waiting);
        let (fall, start) = flight.fall.expect("a ballistic fall");
        assert_eq!(start, 0);
        let velocity = fall.velocity(0.);
        let speed = |v: Vector| v.iter().map(|c| c * c).sum::<f64>().sqrt();
        assert!((speed(velocity) - speed(launch)).abs() < 1e-6);
        let along = velocity.iter().zip(launch).map(|(a, b)| a * b).sum::<f64>()
            / (speed(velocity) * speed(launch));
        let turn = along.clamp(-1., 1.).acos().to_degrees();
        assert!(turn > 0. && turn < 2.2, "{turn}");
        turns.push(turn);
    }
    assert_ne!(turns[0], turns[1], "each bomb has its own scatter");
}

#[test]
fn an_ai_jdam_stores_its_targets_point_as_a_humans_does() {
    same_release(gbu29(), false, 2, designate);
    // With no target the AI's JDAM is held back, as a human's is.
    let config = config(gbu29(), false);
    let mut robot = ai(gbu29(), false);
    robot
        .ai_cockpit_mut(0, DEFAULT_OWNSHIP_SIDE, &config)
        .unwrap()
        .target = None;
    tick(&mut robot, false);
    assert_eq!(ai_release(&mut robot, &config), Err(Readiness::NoTarget));
    assert!(robot.projectiles.is_empty());
    // A bare ground point is enough for a JDAM.
    let point = [40., 0., 11_000.];
    robot
        .ai_cockpit_mut(0, DEFAULT_OWNSHIP_SIDE, &config)
        .unwrap()
        .target = Some(StrikeTarget::Point(point));
    let ids = ai_release(&mut robot, &config).unwrap();
    let flight = robot.guided_flight(ids[0]).unwrap();
    assert_eq!(
        (flight.mode, flight.point, flight.target),
        (GuidedMode::Guiding, Some(point), None)
    );
}

#[test]
fn the_ai_judges_a_bomb_where_its_path_meets_the_terrain_not_the_ground_under_it() {
    // Slices ST-LV and ST-G. The tank stands on a 600 ft plateau; the ground
    // under the aircraft, 12,000 ft short of it, is at sea level. Retail's
    // pipper worked the fall out against the ground under the release point;
    // the pipper (and so the AI's CCIP cue) now finds where the path meets
    // the terrain, which is the plateau.
    let ground = |_: f64, z: f64| if z > 8_000. { 600. } else { 0. };
    let bombs = config(mk82(), false);
    let mut robot = ai(mk82(), false);
    tick(&mut robot, false);
    let solution = robot
        .strike_delivery(0, &bombs, &launcher(), next(&robot, &bombs), false, &ground)
        .expect("a bomb solution");
    let ccip = solution.ccip.unwrap();
    assert!((ccip.impact[1] - 600.).abs() < 1e-9);
    let tank = robot
        .targets
        .iter()
        .find(|t| t.id == TANK)
        .unwrap()
        .position;
    let plateau = crate::combat::ballistics::impact_at(ccip.release, 600.).unwrap();
    assert!((ccip.impact[2] - plateau.position[2]).abs() < 1e-3);
    assert_eq!(solution.cue(600.).map(|c| c.position), Some(ccip.impact));
    // Retail's reading, over the sea-level ground under the release, lands
    // 600 to 800 ft beyond it.
    let flat = crate::combat::ballistics::predict_impact_flat(ccip.release, ground).unwrap();
    let long = flat.position[2] - plateau.position[2];
    assert!(long > 600. && long < 800., "{long}");
    let aim_at = |point: Vector| ReleaseAim {
        offset_ft: [point[0] - tank[0], point[2] - tank[2]],
        gate_ft: 20.,
        ballistic: false,
    };
    // Aimed where the bomb really lands (on the plateau): due.
    assert!(robot.strike_release_due(
        0,
        &bombs,
        &launcher(),
        next(&robot, &bombs),
        aim_at(plateau.position),
        &ground
    ));
    // Aimed at retail's point, beyond where the bomb lands: not due, the
    // bomb would land short of it.
    assert!(!robot.strike_release_due(
        0,
        &bombs,
        &launcher(),
        next(&robot, &bombs),
        aim_at(flat.position),
        &ground
    ));
}

#[test]
fn the_ai_reads_the_same_solutions_and_releases_on_them() {
    let ground = |_: f64, _: f64| 0.;
    let bombs = config(mk82(), false);
    let mut person = human(mk82(), false);
    person.own_mut().sensors.select_ground(TANK);
    let mut robot = ai(mk82(), false);
    tick(&mut person, false);
    tick(&mut robot, false);
    let solution = robot
        .strike_delivery(0, &bombs, &launcher(), next(&robot, &bombs), false, &ground)
        .expect("a bomb solution");
    let human = person.own().delivery.clone().expect("the human's solution");
    assert_eq!(solution.ccip, human.ccip);
    assert_eq!(solution.mode, human.mode);
    // 12,000 ft out a level Mk 82 does not reach the tank: not due on a
    // 100 m gate at the tank. An aim offset that puts the aim point on the
    // impact makes it due; the gate alone decides how close is close enough.
    let gate = 328.;
    assert!(!robot.strike_release_due(
        0,
        &bombs,
        &launcher(),
        next(&robot, &bombs),
        ReleaseAim::on_target(gate),
        &ground
    ));
    let impact = solution.ccip.unwrap().impact;
    let tank = robot
        .targets
        .iter()
        .find(|t| t.id == TANK)
        .unwrap()
        .position;
    let onto = ReleaseAim {
        offset_ft: [impact[0] - tank[0], impact[2] - tank[2]],
        gate_ft: gate,
        ballistic: false,
    };
    assert!(robot.strike_release_due(0, &bombs, &launcher(), next(&robot, &bombs), onto, &ground));
    let near = ReleaseAim {
        offset_ft: [onto.offset_ft[0], onto.offset_ft[1] + gate + 10.],
        gate_ft: gate,
        ballistic: false,
    };
    assert!(!robot.strike_release_due(0, &bombs, &launcher(), next(&robot, &bombs), near, &ground));
    assert_eq!(onto.point(tank)[1], tank[1]);
    // A guided bomb reads the human's guided sight.
    let jdams = config(gbu29(), false);
    let mut robot = ai(gbu29(), false);
    tick(&mut robot, false);
    let sight = robot
        .strike_guided_sight(0, &jdams, &launcher(), &ground)
        .expect("a guided sight");
    let tank = robot
        .targets
        .iter()
        .find(|t| t.id == TANK)
        .unwrap()
        .position;
    assert_eq!(sight.aim, Some(tank));
    assert_eq!(
        robot.strike_readiness(0, &jdams, launcher()),
        Readiness::Ready
    );
    // Smart weapons take no skill offset: a guided release ignores the aim.
    let wide = ReleaseAim {
        offset_ft: [2_000., -3_000.],
        gate_ft: 0.,
        ballistic: false,
    };
    assert_eq!(
        robot.strike_release_due(0, &jdams, &launcher(), next(&robot, &jdams), wide, &ground),
        robot.strike_release_due(
            0,
            &jdams,
            &launcher(),
            next(&robot, &jdams),
            ReleaseAim::on_target(328.),
            &ground
        )
    );
}

#[test]
fn ai_cockpits_survive_a_checkpoint_and_go_with_a_lost_aircraft() {
    let config = config(gbu10(), true);
    let mut s = ai(gbu10(), true);
    designate_and_lase(&mut s, &config);
    tick(&mut s, false);
    ai_release(&mut s, &config).unwrap();
    tick(&mut s, false);
    let models = crate::checkpoint::Models::default();
    assert_eq!(
        crate::checkpoint::round_trip(&s.ai_cockpits, &models).unwrap(),
        s.ai_cockpits
    );
    // The whole state: the restored one steps on identically.
    let mut copy = crate::checkpoint::round_trip(&s, &models).unwrap();
    assert_eq!(copy.strike_cockpit(0), s.strike_cockpit(0));
    for _ in 0..240 {
        tick(&mut s, false);
        tick(&mut copy, false);
    }
    assert!(!s.projectiles.is_empty());
    assert_eq!(released(&s), released(&copy));
    assert_eq!(copy.strike_cockpit(0), s.strike_cockpit(0));
    // An ownship's cockpit round-trips with it.
    let person = human(gbu10(), true);
    let own: Ownship = crate::checkpoint::round_trip(person.own(), &models).unwrap();
    assert_eq!(own.cockpit, person.own().cockpit);
    // The entry goes with its aircraft.
    s.targets.iter_mut().find(|t| t.id == 0).unwrap().hp = 0;
    tick(&mut s, false);
    assert!(s.strike_cockpit(0).is_none());
}

/// B3 (docs/spec/stores-on-pylons.md): a paired station's stores leave from
/// the record's position and its mirror in turn, left first, a human's
/// presses and the AI's releases alike; the pack stagger and the height stay.
/// Four on a wing row hang from two pylons a wing (BP2): outboard first.
#[test]
fn a_paired_station_alternates_sides_for_a_human_and_the_ai() {
    let paired = |mut c: Configuration| {
        c.stations[0].rack = crate::combat::hardpoints::Rack {
            location: 4,
            paired: true,
            ..Default::default()
        };
        c
    };
    let config = paired(config(mk82(), false));
    let origin_x = |s: &State, id: u32| s.bomb_motion(id).expect("a bomb").origin[0];

    let mut person = with_tank(State::new(config.clone(), true).unwrap());
    person.own_mut().armed = true;
    person.own_mut().selected = 0;
    designate(&mut person, &config);
    tick(&mut person, false);
    tick(&mut person, false);
    let mut human = Vec::new();
    for _ in 0..4 {
        let before: Vec<u32> = person.projectiles.iter().map(|p| p.id).collect();
        tick(&mut person, true);
        for _ in 0..60 {
            tick(&mut person, false);
        }
        let fresh: Vec<u32> = person
            .projectiles
            .iter()
            .map(|p| p.id)
            .filter(|id| !before.contains(id))
            .collect();
        assert_eq!(fresh.len(), 1, "one bomb a press");
        human.push(origin_x(&person, fresh[0]));
        assert!((person.bomb_motion(fresh[0]).unwrap().origin[1] - (8_000. - 3.)).abs() < 1e-9);
    }
    let s = crate::combat::hardpoints::DUAL_PYLON_SPREAD;
    let want = [-(2. + s), 2. + s, -(2. - s), 2. - s];
    assert!(
        human.iter().zip(want).all(|(x, w)| (x - w).abs() < 1e-9),
        "{human:?}"
    );

    let mut robot = ai(mk82(), false);
    designate(&mut robot, &config);
    tick(&mut robot, false);
    tick(&mut robot, false);
    let mut machine = Vec::new();
    for rounds_after in [3, 2, 1, 0] {
        let mut next = 1u32 << 24 | rounds_after;
        let ids = robot
            .release_strike(
                StrikeRelease {
                    aircraft: 0,
                    config: &config,
                    station: 0,
                    launcher: launcher(),
                    incoming: None,
                    rounds_after,
                },
                || {
                    next += 16;
                    next
                },
                &|_, _| 0.,
            )
            .expect("the AI's release is ready");
        machine.push(origin_x(&robot, ids[0]));
    }
    assert_eq!(machine, human);
}

/// The bomb sight predicts from the pylon the next bomb leaves (B3
/// follow-up, John 2026-10-12): before each press the CCIP's release point
/// is on the pylon the press's bomb then leaves from, left first.
#[test]
fn the_bomb_sight_predicts_from_the_pylon_the_next_bomb_leaves() {
    let mut config = config(mk82(), false);
    config.stations[0].rack = crate::combat::hardpoints::Rack {
        location: 4,
        paired: true,
        ..Default::default()
    };
    let mut person = with_tank(State::new(config.clone(), true).unwrap());
    person.own_mut().armed = true;
    person.own_mut().selected = 0;
    designate(&mut person, &config);
    tick(&mut person, false);
    tick(&mut person, false);
    let mut sight = Vec::new();
    let mut dropped = Vec::new();
    for _ in 0..4 {
        let release = person
            .own()
            .delivery
            .as_ref()
            .unwrap()
            .ccip
            .unwrap()
            .release;
        let launcher = launcher().position;
        sight.push(release.position[0] - launcher[0]);
        let before: Vec<u32> = person.projectiles.iter().map(|p| p.id).collect();
        tick(&mut person, true);
        let fresh = person
            .projectiles
            .iter()
            .map(|p| p.id)
            .find(|id| !before.contains(id))
            .expect("a bomb");
        dropped.push(person.bomb_motion(fresh).unwrap().origin[0] - launcher[0]);
        for _ in 0..60 {
            tick(&mut person, false);
        }
    }
    let rounded = |v: &[f64]| {
        v.iter()
            .map(|x| (x * 1e6).round() / 1e6)
            .collect::<Vec<_>>()
    };
    // Four on a wing row hang from two pylons a wing, 1.4 ft either side
    // of the record's 2 ft: outboard left, outboard right, then inboard.
    assert_eq!(rounded(&sight), [-3.4, 3.4, -0.6, 0.6]);
    assert_eq!(rounded(&sight), rounded(&dropped));
}

/// Where the AI's next store leaves: the selected station's own position
/// (these stations are unpaired).
fn next(robot: &State, config: &Configuration) -> crate::attitude::Vector {
    robot
        .strike_cockpit(0)
        .and_then(|c| config.stations.get(c.selected))
        .map_or([0.; 3], |s| {
            s.next_release_point(u32::from(s.count) * super::pod_rounds(&s.weapon))
        })
}
