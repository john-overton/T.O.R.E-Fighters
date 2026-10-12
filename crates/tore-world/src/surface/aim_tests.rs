//! The aim mapping: bearings, arcs, slew rates, driving, seeks and burst
//! holds, on synthetic units. No retail data.
use super::*;
use crate::surface::fire::MountArc;

const UNIT: UnitId = UnitId(0x4000_0001);
const SHIP: UnitId = UnitId(0x4000_0002);
const JET: u32 = 7;

fn deg(radians: f64) -> f64 {
    radians.to_degrees()
}

#[test]
fn bearings_are_relative_to_the_hull_and_positive_to_the_right() {
    let at = [0., 0., 0.];
    // North of a unit facing north: dead ahead; east: a quarter right.
    assert!(relative_bearing(at, 0., [0., 50., 100.]).abs() < 1e-12);
    assert!((deg(relative_bearing(at, 0., [100., 0., 0.])) - 90.).abs() < 1e-9);
    // Facing east, north is a quarter left.
    assert!((deg(relative_bearing(at, PI / 2., [0., 0., 100.])) + 90.).abs() < 1e-9);
    // Always within half a turn.
    let b = relative_bearing(at, 3., [0., 0., -100.]);
    assert!(b.abs() <= PI);
}

#[test]
fn a_turret_goal_stays_inside_its_arc() {
    let rest = PI; // an aft mount
    let limit = 150f64.to_radians();
    // A target dead astern: no traverse.
    assert!(traverse_goal(PI, rest, limit).abs() < 1e-12);
    // 30 degrees right of the bow is 150 left of the stern: the arc's edge.
    assert!((deg(traverse_goal(30f64.to_radians(), rest, limit)) + 150.).abs() < 1e-9);
    // Dead ahead lies outside the arc: held at the nearer edge.
    assert!((deg(traverse_goal(0., rest, limit)).abs() - 150.).abs() < 1e-9);
    // Unrestricted (0) follows the target all the way round.
    assert!((deg(traverse_goal(-170f64.to_radians(), 0., 0.)) + 170.).abs() < 1e-9);
}

#[test]
fn slewing_is_rate_limited_and_never_crosses_an_arcs_far_side() {
    // Inside an arc, from 140 left to 140 right the long way, through rest.
    let from = (-140f64).to_radians();
    let to = 140f64.to_radians();
    let step = 10f64.to_radians();
    let next = slew(from, to, step, false);
    assert!((deg(next) + 130.).abs() < 1e-9);
    // Unrestricted takes the short way across the stern.
    let next = slew(from, to, step, true);
    assert!((deg(next) + 150.).abs() < 1e-9);
    // A goal within a step is reached exactly.
    assert!((slew(0.1, 0.15, step, true) - 0.15).abs() < 1e-12);
}

fn body_rules(rate: f64) -> BTreeMap<UnitId, Rule> {
    BTreeMap::from([(UNIT, Rule::Body { rate_deg_s: rate })])
}

fn track(unit: UnitId, station: usize) -> Track {
    Track {
        unit,
        weapon: Weapon::Station(station),
        target: JET,
    }
}

/// Steps `aims` one tick at a time from `from` through `to`.
fn run(
    aims: &mut Aims,
    rules: &BTreeMap<UnitId, Rule>,
    tracks: &[Track],
    arc: &dyn Fn(UnitId, usize) -> Option<(usize, MountArc)>,
    stance: &dyn Fn(UnitId) -> Option<Stance>,
    jet: Vector,
    (from, to): (u64, u64),
) {
    for tick in from..=to {
        aims.step(tick, rules, tracks, arc, stance, &|id| {
            (id == JET).then_some(jet)
        });
    }
}

#[test]
fn a_body_turns_at_its_rate_and_keeps_its_heading_after() {
    let rules = body_rules(15.);
    let still = |_: UnitId| {
        Some(Stance {
            origin: [0., 0., 0.],
            heading: 0.,
        })
    };
    let east = [10_000., 2_000., 0.];
    let mut aims = Aims::default();
    let tracks = [track(UNIT, 0)];
    // The first step sets things where they are; nothing is held yet.
    run(&mut aims, &rules, &[], &|_, _| None, &still, east, (0, 0));
    // A T-72 (15 degrees a second) turns a quarter right in 6 s.
    run(
        &mut aims,
        &rules,
        &tracks,
        &|_, _| None,
        &still,
        east,
        (1, 120),
    );
    assert!((deg(aims.unit(UNIT).unwrap().body) - 15.).abs() < 1e-6);
    run(
        &mut aims,
        &rules,
        &tracks,
        &|_, _| None,
        &still,
        east,
        (121, 720),
    );
    assert!((deg(aims.unit(UNIT).unwrap().body) - 90.).abs() < 1e-6);
    // Nothing held: it stays pointed where it last aimed.
    run(
        &mut aims,
        &rules,
        &[],
        &|_, _| None,
        &still,
        east,
        (721, 1_000),
    );
    assert!((deg(aims.unit(UNIT).unwrap().body) - 90.).abs() < 1e-6);
    assert!(aims.unit(UNIT).unwrap().turned());
}

#[test]
fn a_driving_body_lines_up_with_its_route() {
    let rules = body_rules(30.);
    let mut aims = Aims::default();
    let east = [10_000., 2_000., 0.];
    let tracks = [track(UNIT, 0)];
    let still = |_: UnitId| {
        Some(Stance {
            origin: [0., 0., 0.],
            heading: 0.,
        })
    };
    run(&mut aims, &rules, &[], &|_, _| None, &still, east, (0, 0));
    run(
        &mut aims,
        &rules,
        &tracks,
        &|_, _| None,
        &still,
        east,
        (1, 240),
    );
    assert!((deg(aims.unit(UNIT).unwrap().body) - 60.).abs() < 1e-6);
    // It drives north at 50 ft/s, still holding the jet: back to the route.
    for tick in 241..=480u64 {
        let z = (tick - 240) as f64 * 50. / 120.;
        let driving = move |_: UnitId| {
            Some(Stance {
                origin: [0., 0., z],
                heading: 0.,
            })
        };
        aims.step(tick, &rules, &tracks, &|_, _| None, &driving, &|_| {
            Some(east)
        });
    }
    assert!(aims.unit(UNIT).unwrap().body.abs() < 1e-9);
}

/// A ship's two gun mounts: fore (station 0, hardpoint 0) and aft
/// (station 1, hardpoint 1), each 150 degrees either side of its rest.
fn ship_arc(unit: UnitId, hardpoint: usize) -> Option<(usize, MountArc)> {
    let rest = if hardpoint == 0 { 0. } else { 180. };
    (unit == SHIP && hardpoint < 2).then_some((
        hardpoint,
        MountArc {
            index: hardpoint,
            rest: [rest, 0.],
            limit: [150., 90.],
            offset: [0., 10., if hardpoint == 0 { 100. } else { -100. }],
        },
    ))
}

#[test]
fn turrets_traverse_in_their_arcs_and_return_to_rest() {
    let rules = BTreeMap::from([(SHIP, Rule::Turrets(vec![0, 1]))]);
    let stance = |_: UnitId| {
        Some(Stance {
            origin: [0., 0., 0.],
            heading: 0.,
        })
    };
    let lookup = ship_arc;
    // A jet far off the starboard bow: the fore turret turns right toward
    // it at 45 degrees a second; the aft one has nothing held.
    let jet = [100_000., 5_000., 100.];
    let mut aims = Aims::default();
    run(&mut aims, &rules, &[], &lookup, &stance, jet, (0, 0));
    run(
        &mut aims,
        &rules,
        &[track(SHIP, 0)],
        &lookup,
        &stance,
        jet,
        (1, 120),
    );
    let ship = aims.unit(SHIP).unwrap();
    assert!((deg(ship.turrets[&0]) - 45.).abs() < 1e-6);
    assert_eq!(ship.turrets[&1], 0.);
    // Both held, the aft one by its hardpoint (a burst): from astern to
    // the right is a quarter turn left.
    let both = [
        track(SHIP, 0),
        Track {
            unit: SHIP,
            weapon: Weapon::Mount(1),
            target: JET,
        },
    ];
    run(&mut aims, &rules, &both, &lookup, &stance, jet, (121, 600));
    let ship = aims.unit(SHIP).unwrap();
    assert!((deg(ship.turrets[&0]) - 90.).abs() < 0.2);
    assert!((deg(ship.turrets[&1]) + 90.).abs() < 0.2);
    // Dead ahead is outside the aft arc: it waits at the arc's edge.
    let ahead = [0., 5_000., 100_000.];
    run(
        &mut aims,
        &rules,
        &both,
        &lookup,
        &stance,
        ahead,
        (601, 1_200),
    );
    let ship = aims.unit(SHIP).unwrap();
    assert!(ship.turrets[&0].abs() < 1e-3);
    assert!((deg(ship.turrets[&1]).abs() - 150.).abs() < 0.2);
    // Released: back to rest at the same rate.
    run(
        &mut aims,
        &rules,
        &[],
        &lookup,
        &stance,
        ahead,
        (1_201, 1_440),
    );
    let ship = aims.unit(SHIP).unwrap();
    assert!((deg(ship.turrets[&1]).abs() - 60.).abs() < 0.2);
    run(
        &mut aims,
        &rules,
        &[],
        &lookup,
        &stance,
        ahead,
        (1_441, 1_800),
    );
    assert!(!aims.unit(SHIP).unwrap().turned());
}

#[test]
fn a_seek_sets_every_part_on_its_goal_and_a_rewind_starts_afresh() {
    let rules = body_rules(5.);
    let still = |_: UnitId| {
        Some(Stance {
            origin: [0., 0., 0.],
            heading: 0.,
        })
    };
    let west = [-10_000., 2_000., 0.];
    let tracks = [track(UNIT, 0)];
    let mut aims = Aims::default();
    run(
        &mut aims,
        &rules,
        &tracks,
        &|_, _| None,
        &still,
        west,
        (0, 0),
    );
    // Two seconds later in one step: more than a second, so it snaps.
    run(
        &mut aims,
        &rules,
        &tracks,
        &|_, _| None,
        &still,
        west,
        (240, 240),
    );
    assert!((deg(aims.unit(UNIT).unwrap().body) + 90.).abs() < 1e-9);
    // Back in time: everything as at the start, then the goal again.
    run(&mut aims, &rules, &[], &|_, _| None, &still, west, (10, 10));
    assert_eq!(aims.unit(UNIT).unwrap().body, 0.);
    // The same ticks give the same angles however they are stepped.
    let mut a = Aims::default();
    let mut b = Aims::default();
    run(&mut a, &rules, &[], &|_, _| None, &still, west, (0, 0));
    b.step(0, &rules, &[], &|_, _| None, &still, &|_| Some(west));
    run(
        &mut a,
        &rules,
        &tracks,
        &|_, _| None,
        &still,
        west,
        (1, 120),
    );
    for tick in (1..=120).step_by(7).chain([120]) {
        b.step(tick, &rules, &tracks, &|_, _| None, &still, &|_| Some(west));
    }
    assert!((a.unit(UNIT).unwrap().body - b.unit(UNIT).unwrap().body).abs() < 1e-12);
}

#[test]
fn a_burst_holds_its_target_for_five_seconds() {
    let bursts = [
        (100, UNIT.0, 2, JET),
        (200, UNIT.0, 2, 9),
        (50, SHIP.0, 0, JET),
    ];
    // The latest burst of each mount, until 600 ticks after it began.
    assert_eq!(
        burst_tracks(300, &bursts),
        vec![
            Track {
                unit: UNIT,
                weapon: Weapon::Mount(2),
                target: 9
            },
            Track {
                unit: SHIP,
                weapon: Weapon::Mount(0),
                target: JET
            },
        ]
    );
    assert_eq!(burst_tracks(700, &bursts).len(), 1);
    assert!(burst_tracks(801, &bursts).is_empty());
    // A burst not begun yet holds nothing.
    assert!(burst_tracks(40, &[(50, SHIP.0, 0, JET)]).is_empty());
}
