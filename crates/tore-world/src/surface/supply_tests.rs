//! Resupply (plan 9.1 SR1, 4.3 test 4's truck case): rail rearm times per
//! system with a truck in range and none without, a destroyed truck, one
//! truck for several units, the reserve refilled one magazine per swap
//! period, ships, a truck on a route, and the timers in a checkpoint. On
//! synthetic units with the retail records' numbers; no retail data.
use super::*;
use crate::surface::{SupplyTruck, supply::rearm_seconds};

const TRUCK_ID: u32 = 0x5800_0000;
const SECOND_TRUCK_ID: u32 = 0x5800_0001;
const OTHER_ID: u32 = 0x4000_0010;

/// A jet so far away that no unit ever sees it.
const DISTANT: [f64; 3] = [0., 15_000., 9_000_000.];

fn truck(id: u32, x: f64, z: f64) -> Placed {
    let mut p = placed(id, "MISTRK.NT", 0x0200, x, z, 1);
    p.unit.kind = UnitKind::Passive;
    p.unit.supply_truck = true;
    p
}

/// A fixture of `units` and `trucks`, the jet out of reach, one tick run so
/// every unit is armed.
fn site(units: Vec<Placed>, trucks: &[u32]) -> Fixture {
    let mut f = Fixture::new(units, Vec::new());
    f.surface.trucks = trucks
        .iter()
        .map(|id| SupplyTruck {
            id: UnitId(*id),
            serves: None,
            added: false,
        })
        .collect();
    // The digest covers the trucks.
    f.state = f.surface.fresh_state();
    f.jet.position = DISTANT;
    f.step();
    f
}

/// Three rails of one missile each.
fn launcher(id: u32, resource: &str, weapon: Weapon, x: f64) -> Placed {
    placed(id, resource, 0x1000, x, 0., 1).with_missile(
        weapon,
        &[
            (arc(0, [0., 0.], [0., 70.]), 1),
            (arc(1, [0., 0.], [0., 70.]), 1),
            (arc(2, [0., 0.], [0., 70.]), 1),
        ],
    )
}

fn zsu(id: u32, x: f64) -> Placed {
    placed(id, "ZSU23.NT", 0x0800, x, 0., 1)
        .with_gun(zsu23(), "ZSU23", arc(0, FREE, [0., 90.]), 0)
        .radar(false, None)
}

fn loaded(f: &Fixture, id: u32) -> Vec<u32> {
    f.unit_state(id).mounts.iter().map(|m| m.loaded).collect()
}

fn empty_rails(f: &mut Fixture, id: u32, keep: &[usize]) {
    let state = f.state.unit_mut(UnitId(id)).unwrap();
    for (index, mount) in state.mounts.iter_mut().enumerate() {
        if !keep.contains(&index) {
            mount.loaded = 0;
        }
    }
}

fn move_unit(f: &mut Fixture, id: u32, x: f64, z: f64) {
    let unit = f.surface.units.iter_mut().find(|u| u.id.0 == id).unwrap();
    unit.position = [x as i32, 0, z as i32];
}

fn rearms(f: &Fixture) -> Vec<u64> {
    f.traces
        .iter()
        .filter(|(_, t)| matches!(t, Trace::Rearm { .. }))
        .map(|(tick, _)| *tick)
        .collect()
}

#[test]
fn rearm_times_follow_the_system() {
    for (resource, seconds) in [
        ("SA2A.NT", 600),
        ("SA3.NT", 600),
        ("HAWK.NT", 600),
        ("SA6.NT", 420),
        ("ROLAND.NT", 420),
        ("SA15.NT", 420),
        ("SA9.NT", 420),
        ("SA13.NT", 420),
        ("ASA5.NT", 420),
        ("CHAP.NT", 420),
        ("2S6.NT", 420),
        ("SCUD.NT", 420),
        ("FIM92.NT", 300),
        ("MIS.NT", 300),
        ("SA7.NT", 300),
        ("SA14.NT", 300),
        ("SA16.NT", 300),
        ("sa7.nt", 300),
    ] {
        assert_eq!(rearm_seconds(resource), seconds, "{resource}");
    }
}

#[test]
fn a_truck_in_range_refills_every_empty_rail_together_after_the_systems_time() {
    for (resource, weapon, seconds) in [
        ("SA2A.NT", sa2(), 600u64),
        ("SA6.NT", sa6(), 420),
        ("SA7.NT", sa7(), 300),
    ] {
        let mut f = site(
            vec![
                launcher(SA6_ID, resource, weapon, 0.),
                truck(TRUCK_ID, 300., 0.),
            ],
            &[TRUCK_ID],
        );
        // One rail still loaded; the other two empty.
        empty_rails(&mut f, SA6_ID, &[1]);
        let start = f.tick;
        f.run(seconds * TPS);
        assert_eq!(loaded(&f, SA6_ID), [0, 1, 0], "{resource}: not yet");
        assert!(f.unit_state(SA6_ID).supply);
        f.step();
        assert_eq!(loaded(&f, SA6_ID), [1, 1, 1], "{resource}: all together");
        assert_eq!(rearms(&f), vec![start + seconds * TPS], "{resource}");
        // Full: the timer is idle and nothing more happens.
        f.run(2 * seconds * TPS);
        assert_eq!(rearms(&f).len(), 1);
        assert!(!f.unit_state(SA6_ID).resupply.running());
    }
}

#[test]
fn without_a_truck_the_rails_stay_empty() {
    let mut f = site(vec![launcher(SA6_ID, "SA7.NT", sa7(), 0.)], &[]);
    empty_rails(&mut f, SA6_ID, &[]);
    f.run(2 * 300 * TPS);
    assert_eq!(loaded(&f, SA6_ID), [0, 0, 0]);
    assert!(!f.unit_state(SA6_ID).supply);
    // A truck of the other side, or one 529 ft away, does not count either.
    let mut enemy = truck(TRUCK_ID, 100., 0.);
    enemy.unit.side = BLUE;
    let mut f = site(
        vec![
            launcher(SA6_ID, "SA7.NT", sa7(), 0.),
            enemy,
            truck(SECOND_TRUCK_ID, 529., 0.),
        ],
        &[TRUCK_ID, SECOND_TRUCK_ID],
    );
    empty_rails(&mut f, SA6_ID, &[]);
    f.run(2 * 300 * TPS);
    assert_eq!(loaded(&f, SA6_ID), [0, 0, 0]);
    assert!(!f.unit_state(SA6_ID).supply);
}

#[test]
fn the_reach_is_528_ft_horizontal() {
    // Height does not count: a truck 500 ft below or above is as near as its
    // map position.
    let mut f = site(
        vec![
            launcher(SA6_ID, "SA7.NT", sa7(), 0.),
            truck(TRUCK_ID, 528., 0.),
        ],
        &[TRUCK_ID],
    );
    assert!(
        f.unit_state(SA6_ID).supply || {
            f.step();
            f.unit_state(SA6_ID).supply
        }
    );
    move_unit(&mut f, TRUCK_ID, 400., 350.);
    f.step();
    assert!(!f.unit_state(SA6_ID).supply, "{} ft", 400f64.hypot(350.));
    move_unit(&mut f, TRUCK_ID, 300., 400.);
    f.step();
    assert!(f.unit_state(SA6_ID).supply, "500 ft");
}

#[test]
fn a_destroyed_truck_stops_resupply_at_once_and_the_timer_restarts() {
    let mut f = site(
        vec![
            launcher(SA6_ID, "SA6.NT", sa6(), 0.),
            truck(TRUCK_ID, 200., 0.),
            truck(SECOND_TRUCK_ID, 5_000., 0.),
        ],
        &[TRUCK_ID, SECOND_TRUCK_ID],
    );
    empty_rails(&mut f, SA6_ID, &[]);
    f.run(400 * TPS);
    assert!(f.unit_state(SA6_ID).resupply.rearm > 0);
    // 20 s short of the 420 s: the truck is destroyed.
    f.kill(TRUCK_ID);
    f.step();
    assert!(!f.unit_state(SA6_ID).supply, "the flag drops the same tick");
    assert_eq!(f.unit_state(SA6_ID).resupply.rearm, 0, "cancelled");
    f.run(1_000 * TPS);
    assert_eq!(loaded(&f, SA6_ID), [0, 0, 0]);
    assert!(rearms(&f).is_empty());
    // Another truck arrives: a whole new 420 s, not the 20 s left.
    move_unit(&mut f, SECOND_TRUCK_ID, 100., 0.);
    f.step();
    let arrival = f.tick - 1;
    f.run(419 * TPS);
    assert_eq!(loaded(&f, SA6_ID), [0, 0, 0]);
    f.run(TPS + 1);
    assert_eq!(loaded(&f, SA6_ID), [1, 1, 1]);
    assert_eq!(rearms(&f), vec![arrival + 420 * TPS]);
}

#[test]
fn a_truck_that_leaves_range_restarts_the_rearm() {
    let mut f = site(
        vec![
            launcher(SA6_ID, "SA6.NT", sa6(), 0.),
            truck(TRUCK_ID, 200., 0.),
        ],
        &[TRUCK_ID],
    );
    empty_rails(&mut f, SA6_ID, &[]);
    f.run(300 * TPS);
    move_unit(&mut f, TRUCK_ID, 2_000., 0.);
    f.run(10 * TPS);
    move_unit(&mut f, TRUCK_ID, 200., 0.);
    f.step();
    let back = f.tick - 1;
    f.run(419 * TPS);
    assert_eq!(
        loaded(&f, SA6_ID),
        [0, 0, 0],
        "300 s earlier does not count"
    );
    f.run(TPS + 1);
    assert_eq!(loaded(&f, SA6_ID), [1, 1, 1]);
    assert_eq!(rearms(&f), vec![back + 420 * TPS]);
}

#[test]
fn one_truck_serves_every_unit_in_range_at_once() {
    let mut f = site(
        vec![
            launcher(SA6_ID, "SA6.NT", sa6(), -400.),
            launcher(OTHER_ID, "SA6.NT", sa6(), 100.),
            zsu(ZSU_ID, 300.),
            // Out of reach of the truck at the origin.
            launcher(0x4000_0020, "SA6.NT", sa6(), 2_000.),
            truck(TRUCK_ID, 0., 0.),
        ],
        &[TRUCK_ID],
    );
    for id in [SA6_ID, OTHER_ID, 0x4000_0020] {
        empty_rails(&mut f, id, &[]);
    }
    {
        let gun = f.state.unit_mut(UnitId(ZSU_ID)).unwrap();
        gun.mounts[0].reserve = Some(0);
    }
    f.run(420 * TPS + 1);
    assert_eq!(loaded(&f, SA6_ID), [1, 1, 1]);
    assert_eq!(loaded(&f, OTHER_ID), [1, 1, 1]);
    assert_eq!(loaded(&f, 0x4000_0020), [0, 0, 0], "528 ft is the limit");
    // The same truck's gun refill runs beside the rails (120 s periods).
    let gun = f.unit_state(ZSU_ID);
    assert_eq!(gun.mounts[0].reserve, Some(2), "refilled twice by 420 s");
    assert_eq!(
        rearms(&f).len(),
        2,
        "both launchers rearmed on the same tick"
    );
    assert_eq!(rearms(&f)[0], rearms(&f)[1]);
}

#[test]
fn the_reserve_refills_one_magazine_per_swap_period() {
    for (resource, source, record, unit) in [
        ("ZSU23.NT", "ZSU23.JT", zsu23(), "ZSU23"),
        ("M1939.NT", "M1939.JT", m1939(), "M1939"),
    ] {
        let row = surface_guns::tuning(unit, source).unwrap();
        let period = u64::from(row.magazine_reload_s) * TPS;
        let gun = placed(ZSU_ID, resource, 0x0800, 0., 0., 1)
            .with_gun(record, unit, arc(0, FREE, [0., 90.]), 0)
            .radar(false, None);
        let mut f = site(vec![gun, truck(TRUCK_ID, 250., 100.)], &[TRUCK_ID]);
        assert_eq!(f.unit_state(ZSU_ID).mounts[0].reserve, Some(2));
        f.state.unit_mut(UnitId(ZSU_ID)).unwrap().mounts[0].reserve = Some(0);
        let start = f.tick;
        f.run(period);
        assert_eq!(f.unit_state(ZSU_ID).mounts[0].reserve, Some(0), "{unit}");
        f.step();
        assert_eq!(f.unit_state(ZSU_ID).mounts[0].reserve, Some(1), "{unit}");
        f.run(period - 1);
        assert_eq!(f.unit_state(ZSU_ID).mounts[0].reserve, Some(1), "{unit}");
        f.step();
        assert_eq!(f.unit_state(ZSU_ID).mounts[0].reserve, Some(2), "{unit}");
        let refills: Vec<u64> = f
            .traces
            .iter()
            .filter(|(_, t)| matches!(t, Trace::Refill { .. }))
            .map(|(tick, _)| *tick)
            .collect();
        assert_eq!(refills, vec![start + period, start + 2 * period], "{unit}");
        // Full: no more, and the magazine itself is not topped up.
        f.run(3 * period);
        assert_eq!(f.unit_state(ZSU_ID).mounts[0].reserve, Some(2), "{unit}");
        assert_eq!(f.unit_state(ZSU_ID).mounts[0].loaded, row.magazine);
    }
    // Without a truck nothing refills.
    let gun = zsu(ZSU_ID, 0.);
    let mut f = site(vec![gun], &[]);
    f.state.unit_mut(UnitId(ZSU_ID)).unwrap().mounts[0].reserve = Some(0);
    f.run(400 * TPS);
    assert_eq!(f.unit_state(ZSU_ID).mounts[0].reserve, Some(0));
}

#[test]
fn an_empty_gun_swaps_from_the_truck_and_the_reserve_refills_beside_it() {
    // The ZSU-23 of 4.3 test 4: it empties its magazine and its two spares,
    // falls silent, and fires again once a truck is within 528 ft.
    let row = surface_guns::tuning("ZSU23", "ZSU23.JT").unwrap();
    let period = u64::from(row.magazine_reload_s) * TPS;
    let mut f = site(
        vec![zsu(ZSU_ID, 0.), truck(TRUCK_ID, 5_000., 0.)],
        &[TRUCK_ID],
    );
    f.jet.position = [0., 1_000., 3_000.];
    f.run(700 * TPS);
    assert_eq!(f.rounds(), 3 * row.magazine);
    assert_eq!(f.phase(ZSU_ID), Phase::Empty);
    let stock = f.unit_state(ZSU_ID).mounts[0];
    assert_eq!((stock.loaded, stock.reserve), (0, Some(0)));
    let silent = f.rounds();
    // The truck drives up: a 120 s swap from the truck (the reserve is not
    // debited), a new search and preparation, and fire.
    move_unit(&mut f, TRUCK_ID, 300., 0.);
    f.step();
    let arrival = f.tick - 1;
    assert_eq!(f.phase(ZSU_ID), Phase::Reload);
    f.run(period - 1);
    assert_eq!(f.rounds(), silent, "still swapping");
    f.step();
    let swap = f
        .traces
        .iter()
        .rev()
        .find(|(_, t)| matches!(t, Trace::Swap { .. }))
        .unwrap()
        .0;
    assert_eq!(swap, arrival + period);
    let stock = f.unit_state(ZSU_ID).mounts[0];
    assert_eq!(stock.loaded, row.magazine);
    assert_eq!(
        stock.reserve,
        Some(1),
        "the truck's delivery to the reserve lands beside the swap, not in it"
    );
    f.run(30 * TPS);
    assert!(f.rounds() > silent, "the gun fires again");
    // The truck is destroyed: the next time the gun runs dry with no reserve
    // it is silent again.
    f.kill(TRUCK_ID);
    f.run(2_000 * TPS);
    assert_eq!(f.phase(ZSU_ID), Phase::Empty);
    let after = f.rounds();
    f.run(300 * TPS);
    assert_eq!(f.rounds(), after);
}

#[test]
fn ships_are_never_resupplied() {
    let ship = placed(OTHER_ID, "NIMZ.NT", 0x2000, 0., 0., 1)
        .with_missile(
            sa6(),
            &[
                (arc(0, [0., 0.], [0., 70.]), 1),
                (arc(1, [0., 0.], [0., 70.]), 1),
            ],
        )
        .with_gun(phalanx(), "NIMZ", arc(2, [0., 0.], [120., 90.]), 0)
        .radar(false, None);
    assert!(ship.arms.ship);
    let mut f = site(vec![ship, truck(TRUCK_ID, 150., 0.)], &[TRUCK_ID]);
    empty_rails(&mut f, OTHER_ID, &[2]);
    f.run(2 * 420 * TPS);
    let state = f.unit_state(OTHER_ID);
    assert_eq!(loaded(&f, OTHER_ID)[..2], [0, 0]);
    assert!(!state.supply);
    assert_eq!(state.resupply, Default::default());
    assert!(rearms(&f).is_empty());
    // Its guns' reserve is unlimited and stays so.
    assert_eq!(state.mounts[2].reserve, None);
}

#[test]
fn a_truck_on_a_route_is_measured_where_it_is_now() {
    use crate::surface::movement::{Halt, Mover};
    let fine = 65_536.;
    // The truck is placed 3 miles off but has driven up beside the launcher.
    let mut f = site(
        vec![
            launcher(SA6_ID, "SA7.NT", sa7(), 0.),
            truck(TRUCK_ID, 15_000., 0.),
        ],
        &[TRUCK_ID],
    );
    empty_rails(&mut f, SA6_ID, &[]);
    f.step();
    assert!(!f.unit_state(SA6_ID).supply);
    f.state.unit_mut(UnitId(TRUCK_ID)).unwrap().mover = Some(Mover {
        x: (200. * fine) as i64,
        y: 0,
        z: (100. * fine) as i64,
        heading: 0,
        pitch: 0,
        bank: 0,
        speed: (20. * fine) as i32,
        leg: 0,
        halt: Halt::Moving,
    });
    f.step();
    assert!(f.unit_state(SA6_ID).supply, "its mover, not its placement");
}

#[test]
fn a_rearm_in_progress_survives_a_checkpoint() {
    let mut f = site(
        vec![
            launcher(SA6_ID, "SA6.NT", sa6(), 0.),
            zsu(ZSU_ID, 100.),
            truck(TRUCK_ID, 250., 0.),
        ],
        &[TRUCK_ID],
    );
    empty_rails(&mut f, SA6_ID, &[]);
    f.state.unit_mut(UnitId(ZSU_ID)).unwrap().mounts[0].reserve = Some(0);
    f.run(200 * TPS);
    assert!(f.unit_state(SA6_ID).resupply.rearm > 0);
    assert!(f.unit_state(ZSU_ID).resupply.refill.iter().any(|t| *t > 0));
    let copy = round_trip(&f.state, &Models::default()).expect("it round-trips");
    assert_eq!(copy, f.state);
    assert_eq!(
        copy.unit(UnitId(SA6_ID)).unwrap().resupply,
        f.unit_state(SA6_ID).resupply
    );
    // The restored state finishes the rearm on the same tick as the
    // original.
    let mut resumed = Fixture::new(
        vec![
            launcher(SA6_ID, "SA6.NT", sa6(), 0.),
            zsu(ZSU_ID, 100.),
            truck(TRUCK_ID, 250., 0.),
        ],
        Vec::new(),
    );
    resumed.surface.trucks = f.surface.trucks.clone();
    resumed.jet.position = DISTANT;
    resumed.state = copy;
    resumed.live = f.live.clone();
    resumed.tick = f.tick;
    let first_rearm = |f: &mut Fixture| {
        for _ in 0..(300 * TPS) {
            f.step();
            if let Some(tick) = rearms(f).first() {
                return *tick;
            }
        }
        panic!("no rearm");
    };
    assert_eq!(first_rearm(&mut f), first_rearm(&mut resumed));
    assert_eq!(resumed.state, f.state);
    assert_eq!(loaded(&resumed, SA6_ID), [1, 1, 1]);
}

#[test]
fn a_restart_stops_every_timer() {
    let mut f = site(
        vec![
            launcher(SA6_ID, "SA6.NT", sa6(), 0.),
            truck(TRUCK_ID, 250., 0.),
        ],
        &[TRUCK_ID],
    );
    empty_rails(&mut f, SA6_ID, &[]);
    f.run(100 * TPS);
    assert!(f.unit_state(SA6_ID).resupply.running());
    f.state.reset();
    assert_eq!(f.state, f.surface.fresh_state());
}

#[test]
fn a_battery_that_ran_dry_fires_again_once_its_truck_has_rearmed_a_launcher() {
    use crate::surface::{Battery, BatterySystem};
    const RADAR_ID: u32 = 0x4000_0010;
    const NEAR_ID: u32 = 0x4000_0011;
    const FAR_ID: u32 = 0x4000_0012;
    let radar = placed(RADAR_ID, "SFLUSH.NT", 0x0200, 0., 0., 1).radar(true, Some(true));
    let near = placed(NEAR_ID, "SA6.NT", 0x1000, 800., 0., 1)
        .with_missile(sa6(), &[(arc(0, FREE, [0., 70.]), 3)])
        .radar(false, Some(false));
    let far = placed(FAR_ID, "SA6.NT", 0x1000, 0., -4_000., 1)
        .with_missile(sa6(), &[(arc(0, FREE, [0., 70.]), 3)])
        .radar(false, Some(false));
    // The truck stands by the near launcher, far from the other.
    let mut f = Fixture::new(
        vec![radar, near, far, truck(TRUCK_ID, 1_000., 100.)],
        vec![Battery {
            system: BatterySystem::Sa6,
            side: RED,
            radar: UnitId(RADAR_ID),
            radar_added: false,
            launchers: vec![UnitId(NEAR_ID), UnitId(FAR_ID)],
            truck: Some(UnitId(TRUCK_ID)),
        }],
    );
    f.surface.trucks = vec![SupplyTruck {
        id: UnitId(TRUCK_ID),
        serves: Some(UnitId(NEAR_ID)),
        added: true,
    }];
    f.state = f.surface.fresh_state();
    f.jet.position = [0., 15_000., 40_000.];
    f.step();
    for id in [NEAR_ID, FAR_ID] {
        empty_rails(&mut f, id, &[]);
    }
    f.run(60 * TPS);
    assert_eq!(f.rounds(), 0, "no rails, no launches");
    assert_eq!(
        f.state.batteries[0].controller.phase(),
        Phase::Empty,
        "the battery waits for a rearm"
    );
    // The truck's launcher rearms at 420 s; the other, 4,000 ft away, never.
    while rearms(&f).is_empty() {
        assert!(f.tick < 430 * TPS, "no rearm");
        f.step();
    }
    assert_eq!(rearms(&f), vec![1 + 420 * TPS]);
    assert_eq!(f.rounds(), 0);
    assert_eq!(loaded(&f, NEAR_ID), [3], "the truck's launcher rearmed");
    assert_eq!(
        loaded(&f, FAR_ID),
        [0],
        "4,000 ft from the truck: not served"
    );
    // The battery wakes, prepares again and fires from the launcher that has
    // rails; the other has none.
    f.run(60 * TPS);
    assert_eq!(f.rounds(), 3, "its three rearmed missiles");
    assert!(f.shooters().iter().all(|id| *id == NEAR_ID));
    assert_eq!(loaded(&f, FAR_ID), [0]);
}
