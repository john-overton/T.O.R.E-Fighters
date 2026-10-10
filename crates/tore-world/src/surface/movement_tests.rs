//! Surface movement: the follower on its own (speed, turn rate, terrain, end
//! of route, destruction), then in a world built from a synthetic routed
//! template (the combat target and hit box follow, the picture lists the
//! units, a checkpoint mid-route resumes exactly). No retail data.
use super::{
    movement::{Course, Halt, Leg, Mover, unit_pose},
    tests::{HEADER, MIDDLE, fields, surface_resources, target, world_with_target},
    *,
};
use crate::{seats::SeatInput, test_support::resources::schema, world::TickOutput};
use std::f64::consts::PI;
use tore_formats::surface_unit::class;

/// The follower's step, 120 Hz.
const STEP: f64 = 1. / 120.;

/// A course east-then-north from the origin: `speed` on both legs.
fn course(speed: f64, ship: bool) -> Course {
    Course {
        legs: vec![
            Leg {
                to: [0., 3000.],
                speed,
            },
            Leg {
                to: [2000., 3000.],
                speed,
            },
        ],
        start: [0, 0, 0],
        start_angles: [0, 0, 0],
        turn_rate: 15f64.to_radians(),
        top_speed: 50.,
        acceleration: if ship { 1. } else { 5. },
        ship,
        rig: None,
    }
}

fn flat(_: f64, _: f64) -> f64 {
    256.
}

/// Steps `mover` through `ticks` ticks of a living unit on flat land.
fn run(mover: &mut Mover, course: &Course, ticks: usize) {
    for _ in 0..ticks {
        mover.step(course, true, flat);
    }
}

fn heading_degrees(mover: &Mover) -> f64 {
    mover.attitude()[0].to_degrees()
}

#[test]
fn a_unit_accelerates_to_the_leg_speed_and_no_further() {
    let course = course(50., false);
    let mut mover = Mover::start(&course, 256.);
    assert_eq!(mover.speed_feet(), 0.);
    let mut top = 0f64;
    // 5 ft/s squared: 50 ft/s after 10 s, the wanted speed never exceeded.
    for second in 1..=9 {
        run(&mut mover, &course, 120);
        assert!(
            (mover.speed_feet() - 5. * f64::from(second)).abs() < 0.05,
            "{second} s: {}",
            mover.speed_feet()
        );
    }
    run(&mut mover, &course, 120 * 5);
    for _ in 0..120 * 10 {
        run(&mut mover, &course, 1);
        top = top.max(mover.speed_feet());
        if mover.leg > 0 {
            break;
        }
    }
    assert!((mover.speed_feet().max(top) - 50.).abs() < 0.01 && top <= 50.0001);
    // The record's top speed caps a faster leg.
    let fast = Course {
        top_speed: 50.,
        ..course.clone()
    };
    let mut quick = Mover::start(&fast, 256.);
    let faster = Course {
        legs: vec![Leg {
            to: [0., 30_000.],
            speed: 80.,
        }],
        ..fast
    };
    run(&mut quick, &faster, 120 * 60);
    assert!(quick.speed_feet() <= 80.0001);
}

#[test]
fn a_tank_turns_at_its_record_rate() {
    // 2,730 units at 182 per degree is 15 degrees a second. The corner at
    // the end of the first leg is 90 degrees to the east.
    let course = Course {
        legs: vec![
            Leg {
                to: [0., 1500.],
                speed: 50.,
            },
            Leg {
                to: [1500., 1500.],
                speed: 50.,
            },
        ],
        turn_rate: 2730. / 182. * PI / 180.,
        ..course(50., false)
    };
    assert!((course.turn_rate.to_degrees() - 15.).abs() < 1e-9);
    let mut mover = Mover::start(&course, 256.);
    let mut last = heading_degrees(&mover);
    let (mut worst, mut full_rate) = (0f64, 0);
    for _ in 0..120 * 90 {
        mover.step(&course, true, flat);
        let now = heading_degrees(&mover);
        let change = ((now - last + 540.).rem_euclid(360.) - 180.).abs();
        worst = worst.max(change);
        full_rate += usize::from((change - 15. * STEP).abs() < 1e-3);
        last = now;
    }
    // Never faster than the rate, and the corner holds the full rate for
    // seconds (the unit starts the turn early, inside its turning circle).
    assert!((worst - 15. * STEP).abs() < 1e-3, "{worst}");
    assert!(full_rate > 120 * 3, "{full_rate} ticks at the full rate");
    assert_eq!(mover.halt, Halt::Arrived);
}

#[test]
fn a_ship_turns_at_five_degrees_a_second() {
    let mut ship = Course {
        turn_rate: 910. / 182. * PI / 180.,
        ..course(16., true)
    };
    ship.legs = vec![
        Leg {
            to: [0., 600.],
            speed: 16.,
        },
        Leg {
            to: [3000., 600.],
            speed: 16.,
        },
    ];
    let mut mover = Mover::start(&ship, 0.);
    let mut sustained = 0;
    let mut last = heading_degrees(&mover);
    for _ in 0..120 * 120 {
        mover.step(&ship, true, |_, _| 0.);
        let now = heading_degrees(&mover);
        let change = ((now - last + 540.).rem_euclid(360.) - 180.).abs();
        assert!(change <= 5. * STEP + 1e-3, "{change}");
        sustained += usize::from(change > 0.01);
        last = now;
        // A ship stays at the level it started at.
        assert_eq!(mover.position()[1], 0.);
    }
    assert!(sustained > 120 * 10);
}

#[test]
fn a_land_unit_follows_the_terrain_height_and_tilts_with_the_slope() {
    // A ridge: 0.1 slope (about 5.7 degrees) climbing north, 0.05 across.
    let ridge = |x: f64, z: f64| 256. + 0.1 * z + 0.05 * x;
    let course = course(50., false);
    let mut mover = Mover::start(&course, ridge(0., 0.));
    for _ in 0..120 * 20 {
        mover.step(&course, true, ridge);
        let [x, y, z] = mover.position();
        assert!(
            (y - ridge(x, z)).abs() < 1e-3,
            "{y} against {}",
            ridge(x, z)
        );
    }
    // Climbing north the nose is up by the slope along the heading; the
    // up vector leans away from the uphill side.
    let basis = mover.basis();
    let normal = {
        let n = [-0.05f64, 1., -0.1];
        let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        n.map(|v| v / l)
    };
    let along: f64 = (0..3).map(|i| basis.up[i] * normal[i]).sum();
    assert!(along > 0.9999, "up leans off the surface normal: {along}");
    let [_, pitch, _] = mover.attitude();
    assert!(
        (pitch.to_degrees() - 0.1f64.atan().to_degrees()).abs() < 0.5,
        "{}",
        pitch.to_degrees()
    );
    // Flat ground is level.
    let mut level = Mover::start(&course, 256.);
    run(&mut level, &course, 100);
    assert_eq!(level.attitude()[1..], [0., 0.]);
}

#[test]
fn a_ship_stays_at_its_water_level_over_any_ground() {
    // A coast under the route rises to 900 feet; the ship stays level at the
    // sea it started on while a tank on the same route climbs.
    let coast = |_: f64, z: f64| if z < 1000. { 0. } else { 900. };
    let ship = course(16., true);
    let mut at_sea = Mover::start(&ship, 0.);
    let tank = course(16., false);
    let mut on_land = Mover::start(&tank, 0.);
    for _ in 0..120 * 200 {
        at_sea.step(&ship, true, coast);
        on_land.step(&tank, true, coast);
    }
    assert!(on_land.position()[2] > 1000.);
    assert_eq!(on_land.position()[1], 900.);
    assert_eq!(at_sea.position()[1], 0.);
    assert_eq!(at_sea.attitude()[1..], [0., 0.]);
}

#[test]
fn a_unit_stops_on_the_last_point_and_stays() {
    let course = course(50., false);
    let mut mover = Mover::start(&course, 256.);
    let mut ticks = 0;
    while mover.halt == Halt::Moving && ticks < 120 * 600 {
        mover.step(&course, true, flat);
        ticks += 1;
    }
    assert_eq!(mover.halt, Halt::Arrived);
    assert_eq!(mover.speed, 0);
    // Exactly on the closing leg's point.
    assert_eq!(mover.position()[0], 2000.);
    assert_eq!(mover.position()[2], 3000.);
    // The route is 5,000 feet: about 100 s at 50 ft/s plus the corner and the
    // braking, well under a few minutes.
    assert!((100..200).contains(&(ticks / 120)), "{} s", ticks / 120);
    let at = mover.position();
    run(&mut mover, &course, 120 * 30);
    assert_eq!(mover.position(), at);
    assert_eq!(mover.halt, Halt::Arrived);
}

#[test]
fn a_destroyed_unit_stops_where_it_died() {
    let course = course(50., false);
    let mut mover = Mover::start(&course, 256.);
    run(&mut mover, &course, 120 * 30);
    assert!(mover.speed_feet() > 40.);
    mover.step(&course, false, flat);
    assert_eq!(mover.halt, Halt::Destroyed);
    assert_eq!(mover.speed, 0);
    let at = mover.position();
    // It does not come back to life, even if told it is.
    run(&mut mover, &course, 120 * 30);
    assert_eq!(mover.position(), at);
    assert_eq!(mover.halt, Halt::Destroyed);
}

#[test]
fn the_state_is_exact_and_repeats() {
    let course = course(50., false);
    let (mut a, mut b) = (Mover::start(&course, 256.), Mover::start(&course, 256.));
    run(&mut a, &course, 5000);
    // Two runs in two pieces and in one are the same state.
    run(&mut b, &course, 1234);
    let halfway = b;
    run(&mut b, &course, 5000 - 1234);
    assert_eq!(a, b);
    assert_ne!(a, halfway);
}

// ---- in a world ---------------------------------------------------------

/// A synthetic movable tank: turn rate 2,730, top speed 50.
fn mover_nt(stem: &str, class: u16, turn: i32, max_speed: i32) -> Vec<u8> {
    let object = |name: &str| -> Option<String> {
        Some(match name {
            "structType" => "3".into(),
            "typeSize" => "186".into(),
            "ot_names" => "ot_names".into(),
            "shape" => "shape".into(),
            "obj_class" => class.to_string(),
            "hitPoints" => "100".into(),
            "expType" => "21".into(),
            "craterSize" => "6".into(),
            "utilProc" => "_GVProc".into(),
            "_turnRate" => turn.to_string(),
            "_maxSpeed" => max_speed.to_string(),
            "_cornerSpeed" => "50".into(),
            "_acc" | "_dacc" => "50".into(),
            _ => return None,
        })
    };
    let npc = |name: &str| -> Option<String> { (name == "numHards").then(|| "0".to_owned()) };
    let mut text = String::from(HEADER);
    text += &fields(schema::OBJECT, &object);
    text += &fields(schema::NPC, &npc);
    text += &format!(
        ":ot_names\nstring \"{stem}\"\nstring \"Synthetic {stem}\"\nstring \"{stem}.NT\"\n"
    );
    text += &format!(":shape\nstring \"{stem}.SH\"\nend\n");
    text.into_bytes()
}

/// One waypoint block of a route.
fn waypoint(index: usize, flags: &str, head: &str, at: [i32; 3], speed: i32) -> String {
    format!(
        "\tw_index {index}\r\n\tw_flags {flags}\r\n\tw_goal 0\r\n\tw_next 0\r\n\tw_pos2 {head} {} {} {}\r\n\tw_speed {speed}\r\n\tw_wng 0 0 0 0\r\n\tw_react 0 0 0\r\n\tw_searchDist 0\r\n\tw_preferredTargetId 0\r\n\tw_name \r\n\r\n",
        at[0], at[1], at[2]
    )
}

/// An object that follows a route, and the route.
fn routed(ty: &str, at: [i32; 3], angle: i32, alias: i32, legs: &[[i32; 3]], speed: i32) -> String {
    let mut text = format!(
        "obj\r\n\ttype {ty}\r\n\tpos {} {} {}\r\n\tangle {angle} 0 0\r\n\tnationality2 137\r\n\tflags $93\r\n\tspeed 0\r\n\talias {alias}\r\n\t.\r\n",
        at[0], at[1], at[2]
    );
    text += &format!("waypoint2 {}\r\n", legs.len() + 2);
    text += &waypoint(0, "1", "0 0", at, 0);
    for (n, leg) in legs.iter().enumerate() {
        text += &waypoint(n + 1, "$4", "1 0", *leg, speed);
    }
    text += &waypoint(legs.len() + 1, "2", "0 0", [0, 0, 0], 0);
    text += &format!("  w_for {alias}\r\n\t.\r\n");
    text
}

const TANK_ID: u32 = SURFACE_UNIT_BASE;
const BOAT_ID: u32 = SURFACE_UNIT_BASE + 1;
const STANDING_ID: u32 = SURFACE_UNIT_BASE + 2;

/// The synthetic import with a routed template: a tank on a short route with
/// a corner, a boat (which keeps its water level) and a tank that
/// stands still.
fn routed_resources() -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut r = surface_resources();
    let shape = r["F18.SH"].clone();
    for (stem, class, turn, max) in [
        ("MOVER", class::TANK, 2730, 50),
        ("BOAT", class::SHIP, 910, 50),
        ("STAND", class::TANK, 2730, 50),
    ] {
        r.insert(format!("{stem}.NT"), mover_nt(stem, class, turn, max));
        r.insert(format!("{stem}.SH"), shape.clone());
    }
    r.insert("BOAT_A.SH".into(), shape);
    let z = MIDDLE + 8000;
    let mut text = String::from("textFormat\r\n");
    text += &routed(
        "MOVER.NT",
        [MIDDLE, 0, z],
        180,
        -1,
        &[[MIDDLE, 0, z - 800], [MIDDLE + 800, 0, z - 800]],
        50,
    );
    text += &routed(
        "BOAT.NT",
        [MIDDLE + 2000, 0, z],
        180,
        -2,
        &[[MIDDLE + 2000, 0, z - 5000]],
        16,
    );
    text += "obj\r\n\ttype STAND.NT\r\n\tpos 524288 0 540000\r\n\tangle 0 0 0\r\n\tnationality2 137\r\n\tflags $13\r\n\tspeed 0\r\n\talias -3\r\n\t.\r\n";
    r.insert("~QUCOL.M".into(), text.into_bytes());
    r
}

fn world() -> crate::world::World {
    world_with_target(&routed_resources(), &target("QUCOL", 0, 0, 3))
}

fn step(world: &mut crate::world::World, ticks: usize) {
    let mut out = TickOutput::default();
    for _ in 0..ticks {
        let input = SeatInput {
            tick: world.tick(),
            ..SeatInput::default()
        };
        world.step(&[input], &mut out).unwrap();
    }
}

fn row(world: &crate::world::World, id: u32) -> tore_sim::combat::live::Target {
    world
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == id)
        .unwrap()
        .clone()
}

fn mover_of(world: &crate::world::World, id: u32) -> Option<Mover> {
    world.combat.surface.unit(UnitId(id)).unwrap().mover
}

#[test]
fn routes_become_courses_from_the_record() {
    let w = world();
    let courses = &w.terrain.surface.courses;
    assert_eq!(courses.len(), 2, "the standing tank has no course");
    let tank = &courses[&UnitId(TANK_ID)];
    assert_eq!(tank.legs.len(), 2);
    assert_eq!(tank.legs[0].speed, 50.);
    assert!((tank.turn_rate.to_degrees() - 15.).abs() < 1e-9);
    assert_eq!(tank.acceleration, 5.);
    assert!(!tank.ship && tank.rig.is_some());
    assert!((tank.length() - 1600.).abs() < 1e-9);
    let boat = &courses[&UnitId(BOAT_ID)];
    assert!(boat.ship);
    assert_eq!(boat.acceleration, 1.);
    assert!((boat.turn_rate.to_degrees() - 5.).abs() < 1e-9);
    assert_eq!(boat.legs[0].speed, 16.);
    assert!(!courses.contains_key(&UnitId(STANDING_ID)));
}

#[test]
fn a_moving_tank_keeps_its_target_and_hit_box_with_it() {
    let mut w = world();
    let start = row(&w, TANK_ID);
    let box0 = w.combat.state.ground_bounds(TANK_ID).unwrap();
    assert!(mover_of(&w, TANK_ID).is_none());
    step(&mut w, 1200);
    let mover = mover_of(&w, TANK_ID).unwrap();
    assert!(mover.speed_feet() > 40.);
    // It drove south (heading 180) and stands on the 256 ft ground.
    let [x, y, z] = mover.position();
    assert!(z < f64::from(MIDDLE + 8000) - 100., "{z}");
    assert!((x - f64::from(MIDDLE)).abs() < 1. && (y - 256.).abs() < 1e-6);
    // The target row and the contact box are where the pose is.
    let now = row(&w, TANK_ID);
    let bounds = w.combat.state.ground_bounds(TANK_ID).unwrap();
    assert_ne!(now.position, start.position);
    assert_eq!(
        bounds,
        mover.bounds(&w.terrain.surface.courses[&UnitId(TANK_ID)].rig.unwrap())
    );
    let moved: Vec<f64> = (0..3).map(|i| bounds.center[i] - box0.center[i]).collect();
    let shift: Vec<f64> = (0..3)
        .map(|i| now.position[i] - start.position[i])
        .collect();
    for i in 0..3 {
        assert!(
            (moved[i] - shift[i]).abs() < 1.0,
            "axis {i}: {moved:?} {shift:?}"
        );
    }
    assert_eq!(bounds.half, box0.half);
    // Ground-relative velocity is the pose's.
    let speed = now.velocity[0].hypot(now.velocity[2]);
    assert!((speed - mover.speed_feet()).abs() < 1e-6);
    // The standing tank has not moved.
    assert!(mover_of(&w, STANDING_ID).is_none());
    // The unit pose answers for both.
    let surface = w.terrain.surface.clone();
    let unit = surface.unit(UnitId(TANK_ID)).unwrap();
    let pose = unit_pose(unit, w.combat.surface.unit(unit.id), &w.terrain);
    assert_eq!(pose.position, mover.position());
    let standing = surface.unit(UnitId(STANDING_ID)).unwrap();
    let pose = unit_pose(standing, w.combat.surface.unit(standing.id), &w.terrain);
    assert_eq!(pose.position, [524288., 256., 540000.]);
    assert_eq!(pose.velocity, [0.; 3]);
}

#[test]
fn a_route_runs_to_its_end_and_stops_there() {
    let mut w = world();
    step(&mut w, 120 * 50);
    let mover = mover_of(&w, TANK_ID).unwrap();
    assert_eq!(mover.halt, Halt::Arrived);
    // The route's last point, east of the corner.
    assert_eq!(mover.position()[0], f64::from(MIDDLE + 800));
    assert_eq!(mover.position()[2], f64::from(MIDDLE + 8000 - 800));
    let before = row(&w, TANK_ID).position;
    step(&mut w, 600);
    assert_eq!(row(&w, TANK_ID).position, before);
    assert_eq!(row(&w, TANK_ID).velocity, [0.; 3]);
}

#[test]
fn a_destroyed_tank_stops_where_it_died_and_a_boat_sails_on() {
    let mut w = world();
    step(&mut w, 300);
    let at = mover_of(&w, TANK_ID).unwrap().position();
    assert!(at[2] < f64::from(MIDDLE + 8000));
    w.combat
        .state
        .targets
        .iter_mut()
        .find(|t| t.id == TANK_ID)
        .unwrap()
        .hp = 0;
    step(&mut w, 2);
    let stopped = mover_of(&w, TANK_ID).unwrap();
    assert_eq!(stopped.halt, Halt::Destroyed);
    step(&mut w, 600);
    assert_eq!(mover_of(&w, TANK_ID).unwrap(), stopped);
    // The picture lists it as a wreck where it died.
    let poses = &w.combat.render_snapshot().surface;
    let pose = poses.iter().find(|p| p.id.0 == TANK_ID).unwrap();
    assert!(pose.wrecked);
    assert_eq!(pose.position, stopped.position());
    // The boat sails its own route at its own speed, level at the height
    // it started at, whatever happens to the tank.
    let boat = mover_of(&w, BOAT_ID).unwrap();
    assert_eq!(boat.halt, Halt::Moving);
    assert!(boat.speed_feet() > 0.);
    assert!(boat.position()[2] < f64::from(MIDDLE + 8000));
    assert_eq!(boat.position()[1], 256.);
    assert_eq!(boat.attitude()[1..], [0., 0.]);
}

#[test]
fn the_picture_lists_the_routed_units_and_blends_them() {
    let mut w = world();
    step(&mut w, 300);
    let current = w.combat.render_snapshot().clone();
    let previous = w.combat.previous_snapshot().unwrap().clone();
    // Both routed units, not the standing one; in id order.
    let ids: Vec<u32> = current.surface.iter().map(|p| p.id.0).collect();
    assert_eq!(ids, [TANK_ID, BOAT_ID]);
    let (a, b) = (&previous.surface[0], &current.surface[0]);
    assert_ne!(a.position, b.position);
    let halfway = crate::snapshot::interpolate(Some(&previous), &current, 0.5);
    for i in 0..3 {
        let mid = (a.position[i] + b.position[i]) / 2.;
        assert!((halfway.surface[0].position[i] - mid).abs() < 1e-9);
    }
    let end = crate::snapshot::interpolate(Some(&previous), &current, 1.);
    assert_eq!(end.surface[0].position, b.position);
}

#[test]
fn a_checkpoint_mid_route_resumes_the_same_march() {
    let mut a = world();
    step(&mut a, 700);
    let bytes = a.checkpoint().unwrap();
    let mid = mover_of(&a, TANK_ID).unwrap();
    assert_eq!(mid.halt, Halt::Moving);
    let mut b = world();
    b.restore(&bytes).unwrap();
    assert_eq!(b.combat.surface, a.combat.surface);
    assert_eq!(mover_of(&b, TANK_ID), Some(mid));
    // The restored world's target and box are the checkpointed ones.
    assert_eq!(row(&b, TANK_ID).position, row(&a, TANK_ID).position);
    assert_eq!(
        b.combat.state.ground_bounds(TANK_ID),
        a.combat.state.ground_bounds(TANK_ID)
    );
    // And both go on alike, through the corner to the end of the route.
    for _ in 0..8 {
        step(&mut a, 700);
        step(&mut b, 700);
        assert_eq!(b.combat.surface, a.combat.surface);
        assert_eq!(row(&b, TANK_ID).position, row(&a, TANK_ID).position);
        assert_eq!(
            b.combat.state.ground_bounds(TANK_ID),
            a.combat.state.ground_bounds(TANK_ID)
        );
    }
    assert_eq!(mover_of(&a, TANK_ID).unwrap().halt, Halt::Arrived);
    // A state that started over lands on the same march.
    let mut c = world();
    step(&mut c, 700 * 9);
    assert_eq!(c.combat.surface, a.combat.surface);
}

#[test]
fn a_restart_puts_the_column_back_at_its_start() {
    let mut w = world();
    let start = row(&w, TANK_ID).position;
    step(&mut w, 600);
    assert_ne!(row(&w, TANK_ID).position, start);
    w.combat.reset(&mut w.cockpits[0].flight).unwrap();
    assert!(mover_of(&w, TANK_ID).is_none());
}
