//! The strike tactics (docs/spec/ai-strike.md): the pass choice table, the
//! seeded skill draw, the release gates, the heavy floor, the SEAD order,
//! the store choice, the pass, exit and re-attack flow, and the checkpoint.
//! Synthetic pictures and solutions only.
use super::*;
use crate::ai::ScalarSpeed;
use crate::ai::SpeedLimits;
use crate::ai::awareness::ground::{
    DefenseFacts, GroundPicture, GroundSubject, RecentWarnings, Subjects, ThreatLevel,
};
use crate::ai::engagement::{StrikeObjective, StrikeRole};
use crate::checkpoint::{Models, from_bytes, to_bytes};
use crate::sensors::GroundObject;

pub(super) const TARGET: u32 = 0x5000_0001;
pub(super) const OWN: u32 = 1;

pub(super) fn own(position: Vector, speed: f64) -> OwnState {
    OwnState {
        position,
        heading_deg: 0.,
        flight_path_pitch_deg: 0.,
        body_pitch_offset_deg: 2.,
        bank_deg: 0.,
        speed: ScalarSpeed(speed),
        limits: SpeedLimits {
            minimum: ScalarSpeed(220.),
            maximum: ScalarSpeed(1_000.),
            corner: ScalarSpeed(600.),
        },
        altitude_msl_ft: position[1],
        agl_ft: position[1],
        terrain_ahead_ft: 0.,
        terrain_climb_deg: -90.,
        minimum_altitude_ft: 300.,
        at_ceiling: false,
        on_ground: false,
        g_limit: 7.,
        roll_limit_deg_per_s: 180.,
        maximum_bank_deg: 80.,
        alive: true,
        fuel_endurance_s: 3_600.,
        time_home_s: Some(600.),
        internal_fuel_lbs: 8_000.,
        radar_emitting: true,
    }
}

pub(super) fn ground(
    id: u32,
    position: Vector,
    class: u16,
    defense: Option<DefenseFacts>,
) -> GroundSubject {
    GroundSubject {
        object: GroundObject {
            id,
            position,
            velocity: [0.; 3],
            class,
            listed: true,
            max_visible: 78,
            radar: 100.,
            infrared: 100.,
            emitting: defense.is_some_and(|d| d.radar),
            aircraft: false,
            airborne: false,
            airfield: false,
            destroyed: false,
        },
        side: Side(2),
        defense,
    }
}

pub(super) fn site(missile_ft: f64, gun_ft: f64, radar: bool) -> Option<DefenseFacts> {
    Some(DefenseFacts {
        missile_reach_ft: missile_ft,
        gun_reach_ft: gun_ft,
        radar,
        working: true,
        ship: false,
        heading_deg: 0.,
    })
}

/// A picture ordered onto `TARGET`, built from `subjects` (which hold it),
/// briefed on `defenses` too.
pub(super) fn picture(subjects: &[GroundSubject], defenses: Vec<u32>) -> GroundPicture {
    let mut picture = GroundPicture::new(StrikeRole::Strike);
    picture.set_ordered(Some(TARGET));
    picture.set_objective(StrikeObjective {
        targets: Vec::new(),
        defenses,
        point: None,
    });
    picture.rebuild(0, Vec::new(), &Subjects::new(subjects), Vec::new());
    picture
}

pub(super) fn bomb(rounds: u32) -> Store {
    Store {
        station: StationId(1),
        kind: StoreKind::Bomb,
        rounds,
        cadence_ticks: 30,
        fire_range_ft: 10_000.,
        minimum_range_ft: 0.,
        maximum_range_ft: 20_000.,
        round_speed_fps: None,
    }
}

pub(super) fn store(station: u8, kind: StoreKind) -> Store {
    Store {
        station: StationId(station),
        kind,
        ..bomb(4)
    }
}

pub(super) fn inputs(stores: Vec<Store>, solution: Option<Solution>) -> Inputs {
    Inputs {
        class: StrikeClass::Attack,
        stores,
        designator: false,
        mission_height_ft: None,
        solution,
    }
}

pub(super) fn solution(target: Vector, impact: Vector, due: bool) -> Solution {
    Solution {
        station: StationId(1),
        target: TARGET,
        target_position: target,
        target_velocity: [0.; 3],
        target_alive: true,
        impact: Some(impact),
        impact_seconds: Some(10.),
        due,
        warning: false,
        guided_in_flight: false,
    }
}

pub(super) struct World {
    pub(super) own: OwnState,
    pub(super) picture: GroundPicture,
    pub(super) inputs: Inputs,
}

impl World {
    pub(super) fn context(&self, tick: u64) -> Context<'_> {
        Context {
            tick,
            aircraft: OWN,
            side: Side(1),
            own: &self.own,
            velocity: [0., 0., self.own.speed.0],
            picture: &self.picture,
            inputs: &self.inputs,
            experience: Experience::Average,
            warnings: RecentWarnings::default(),
            defending: false,
            recovering: false,
            mates: &[],
            ceiling_ft: 50_000.,
        }
    }
}

pub(super) const TARGET_AT: Vector = [0., 0., 24_000.];

/// An A-4E-like striker at 5,000 ft, 4 nm south of a truck, nothing around.
pub(super) fn quiet_world() -> World {
    World {
        own: own([0., 5_000., 0.], 700.),
        picture: picture(&[ground(TARGET, TARGET_AT, 0x200, None)], Vec::new()),
        inputs: inputs(vec![bomb(4)], None),
    }
}

#[test]
fn the_pass_is_a_fixed_function_of_class_store_threat_and_speed() {
    use PassChoice::{Pass as P, Regain};
    let choose = |class, kind, threat, energy_good, dive_gate| {
        choose_pass(PassInputs {
            class,
            kind,
            threat,
            energy_good,
            dive_gate,
        })
    };
    let fighter = StrikeClass::Fighter;
    let bomb = StoreKind::Bomb;
    // The spec's table for bombs, first row that applies.
    assert_eq!(
        choose(fighter, bomb, ThreatLevel::Guns, false, true),
        Regain
    );
    assert_eq!(
        choose(fighter, bomb, ThreatLevel::Missiles, false, true),
        Regain
    );
    assert_eq!(
        choose(fighter, bomb, ThreatLevel::Missiles, true, true),
        P(Pass::PopUp)
    );
    assert_eq!(
        choose(fighter, bomb, ThreatLevel::Guns, true, true),
        P(Pass::Dive)
    );
    assert_eq!(
        choose(fighter, bomb, ThreatLevel::Clear, false, true),
        P(Pass::Dive)
    );
    assert_eq!(
        choose(fighter, bomb, ThreatLevel::Air, true, false),
        P(Pass::Straight)
    );
    assert_eq!(
        choose(StrikeClass::Attack, bomb, ThreatLevel::Clear, true, false),
        P(Pass::Straight)
    );
    // Other stores.
    for kind in [
        StoreKind::Seeker,
        StoreKind::RadarAgm,
        StoreKind::AntiRadiation,
    ] {
        assert_eq!(
            choose(fighter, kind, ThreatLevel::Missiles, true, false),
            P(Pass::FastHigh)
        );
        assert_eq!(
            choose(fighter, kind, ThreatLevel::Clear, false, false),
            P(Pass::Straight)
        );
    }
    for kind in [StoreKind::Laser, StoreKind::Coordinate] {
        assert_eq!(
            choose(fighter, kind, ThreatLevel::Missiles, false, true),
            P(Pass::Level)
        );
    }
    assert_eq!(
        choose(
            fighter,
            StoreKind::Rocket,
            ThreatLevel::Missiles,
            true,
            true
        ),
        P(Pass::Straight)
    );
    // A heavy flies level whatever it carries or faces.
    for kind in [bomb, StoreKind::Seeker, StoreKind::Coordinate] {
        assert_eq!(
            choose(StrikeClass::Heavy, kind, ThreatLevel::Missiles, false, true),
            P(Pass::Level)
        );
    }
    // The dive gate: max(10,000 ft, 2R) of height and of distance.
    assert!(dive_gate(10_000., 10_000., 1_000.));
    assert!(!dive_gate(9_999., 20_000., 1_000.));
    assert!(!dive_gate(12_000., 11_000., 6_000.));
    assert!((turn_radius_ft(600., 7.) - 1_179.6).abs() < 1.);
}

#[test]
fn the_release_gate_is_a_tenth_of_the_circle_but_never_under_ten_feet() {
    let gate = |l| (GATE_SHARE * release_circle_ft(l)).max(GATE_FLOOR_FT);
    // Ace 33 ft: the floor. Experienced 115 ft: the floor's 11.5 ft is above it.
    assert_eq!(gate(Experience::Ace), GATE_FLOOR_FT);
    assert!((gate(Experience::Experienced) - 11.48).abs() < 0.01);
    assert!((gate(Experience::Novice) - 27.89).abs() < 0.01);
}

#[test]
fn the_release_circle_is_john_s_and_the_draw_is_deterministic() {
    let metres: Vec<f64> = Experience::ALL
        .iter()
        .map(|l| (release_circle_ft(*l) * 0.3048).round())
        .collect();
    assert_eq!(metres, vec![85., 60., 35., 10.]);
    let a = aim_draw(77, 3, 1, 1_000.);
    assert_eq!(
        a,
        aim_draw(77, 3, 1, 1_000.),
        "same seed, aircraft and pass"
    );
    assert_ne!(
        a.offset_ft,
        aim_draw(77, 3, 2, 1_000.).offset_ft,
        "the pass moves it"
    );
    assert_ne!(
        a.offset_ft,
        aim_draw(77, 4, 1, 1_000.).offset_ft,
        "the aircraft moves it"
    );
    assert_ne!(
        a.offset_ft,
        aim_draw(78, 3, 1, 1_000.).offset_ft,
        "the mission moves it"
    );
    // The radius scales the same draw.
    let half = aim_draw(77, 3, 1, 500.);
    assert!((half.offset_ft[0] * 2. - a.offset_ft[0]).abs() < 1e-9);
}

#[test]
fn ninety_percent_of_draws_are_inside_the_circle_uniform_by_area_and_the_rest_in_the_ring() {
    let r = 1_000.;
    let n = 20_000;
    let mut inside = 0;
    let mut inside_mean = 0.;
    let mut half = 0;
    let mut right = 0;
    for pass in 1..=n {
        let d = aim_draw(0x5eed, 7, pass, r);
        let dist = d.offset_ft[0].hypot(d.offset_ft[1]);
        if d.inside {
            inside += 1;
            assert!(dist <= r + 1e-9);
            inside_mean += dist;
            if dist <= r / 2f64.sqrt() {
                half += 1;
            }
        } else {
            assert!((r - 1e-9..=2. * r + 1e-9).contains(&dist), "{dist}");
        }
        if d.offset_ft[0] > 0. {
            right += 1;
        }
    }
    let share = f64::from(inside) / f64::from(n);
    assert!((share - 0.9).abs() < 0.01, "{share}");
    // Uniform by area: mean two thirds of R, half the draws inside R/√2.
    let mean = inside_mean / f64::from(inside);
    assert!((mean / r - 2. / 3.).abs() < 0.01, "{mean}");
    let half_share = f64::from(half) / f64::from(inside);
    assert!((half_share - 0.5).abs() < 0.015, "{half_share}");
    // A uniform bearing.
    assert!((f64::from(right) / f64::from(n) - 0.5).abs() < 0.015);
}

#[test]
fn the_release_floors_and_the_heavy_rule() {
    assert_eq!(release_floor_ft(ThreatLevel::Clear), 3_000.);
    assert_eq!(release_floor_ft(ThreatLevel::Air), 3_000.);
    assert_eq!(release_floor_ft(ThreatLevel::Guns), 5_000.);
    assert_eq!(release_floor_ft(ThreatLevel::Missiles), 5_000.);
    // max(5,000 ft, the mission's airborne height), +0/+5,000/+10,000 by
    // threat, at most 90% of the ceiling.
    assert_eq!(heavy_floor_ft(None, ThreatLevel::Clear, 50_000.), 5_000.);
    assert_eq!(
        heavy_floor_ft(Some(3_000.), ThreatLevel::Air, 50_000.),
        5_000.
    );
    assert_eq!(
        heavy_floor_ft(Some(12_000.), ThreatLevel::Clear, 50_000.),
        12_000.
    );
    assert_eq!(heavy_floor_ft(None, ThreatLevel::Guns, 50_000.), 10_000.);
    assert_eq!(
        heavy_floor_ft(Some(8_000.), ThreatLevel::Missiles, 50_000.),
        18_000.
    );
    assert_eq!(
        heavy_floor_ft(Some(30_000.), ThreatLevel::Missiles, 35_000.),
        31_500.
    );
}

#[test]
fn a_heavy_flies_its_level_run_at_its_floor() {
    let mut world = quiet_world();
    world.inputs.class = StrikeClass::Heavy;
    world.inputs.mission_height_ft = Some(9_000.);
    let mut strike = Strike::new(3);
    let order = strike.step(&world.context(0));
    assert_eq!(strike.pass(), Some(Pass::Level));
    // Climbing from 5,000 ft toward 9,000 ft over the target.
    assert!(order.steer.unwrap().pitch_deg > 0.);
}

#[test]
fn stores_are_chosen_by_class_target_and_threat() {
    let none = TargetFacts::default();
    let radar = TargetFacts {
        radar: true,
        emitting: false,
    };
    let all = vec![
        store(0, StoreKind::Rocket),
        store(1, StoreKind::Bomb),
        store(2, StoreKind::Seeker),
        store(3, StoreKind::Laser),
        store(4, StoreKind::AntiRadiation),
        store(5, StoreKind::Command),
    ];
    let kind = |stores: &[Store], class, threat, target, designator| {
        choose_store(stores, class, threat, target, designator).map(|s| s.kind)
    };
    let fighter = StrikeClass::Fighter;
    let clear = ThreatLevel::Clear;
    // A HARM only at a radar, first.
    assert_eq!(
        kind(&all, fighter, clear, radar, false),
        Some(StoreKind::AntiRadiation)
    );
    assert_eq!(
        kind(&all, fighter, clear, none, false),
        Some(StoreKind::Bomb)
    );
    // Stand-off first under threat Missiles.
    assert_eq!(
        kind(&all, fighter, ThreatLevel::Missiles, none, false),
        Some(StoreKind::Seeker)
    );
    // A laser store only with its own designator.
    let laser_or_rocket = vec![store(0, StoreKind::Rocket), store(3, StoreKind::Laser)];
    assert_eq!(
        kind(&laser_or_rocket, fighter, clear, none, false),
        Some(StoreKind::Rocket)
    );
    assert_eq!(
        kind(&laser_or_rocket, fighter, clear, none, true),
        Some(StoreKind::Laser)
    );
    // A heavy fires no rockets; the AT-2 is never chosen here; empty stores are not.
    let rockets = [store(0, StoreKind::Rocket)];
    assert_eq!(kind(&rockets, StrikeClass::Heavy, clear, none, false), None);
    assert_eq!(
        kind(&[store(5, StoreKind::Command)], fighter, clear, none, false),
        None
    );
    assert_eq!(kind(&[bomb(0)], fighter, clear, none, false), None);
}

#[test]
fn an_anti_radiation_carrier_takes_sam_radars_then_aaa_radars_then_other_radars() {
    let subjects = [
        ground(TARGET, [0., 0., 30_000.], 0x200, None),
        ground(0x10, [0., 0., 10_000.], 0x800, site(0., 7_500., true)),
        ground(0x11, [0., 0., 20_000.], 0x1000, site(40_000., 0., true)),
        ground(0x12, [0., 0., 25_000.], 0x100, site(0., 0., true)),
        ground(0x13, [0., 0., 5_000.], 0x1000, site(40_000., 0., false)),
        ground(0x14, [0., 0., 28_000.], 0x1000, site(40_000., 0., true)),
    ];
    let pic = picture(&subjects, vec![0x10, 0x11, 0x12, 0x13, 0x14]);
    let none = |_: u32| 0;
    assert_eq!(
        sead_order(&pic, Side(1), [0.; 3], &none),
        vec![0x11, 0x14, 0x10, 0x12],
        "SAM radars nearest first, then AAA radar, then any radar; no radar, no place"
    );
    let full = |id: u32| if id == 0x11 { 2 } else { 0 };
    assert_eq!(
        sead_order(&pic, Side(1), [0.; 3], &full)[0],
        0x14,
        "two attackers fill it"
    );
    // A striker with a HARM and no order takes the SAM radar first.
    let mut briefed = GroundPicture::new(StrikeRole::Strike);
    briefed.set_objective(StrikeObjective {
        targets: vec![TARGET],
        defenses: vec![0x10, 0x11],
        point: Some([0., 0., 30_000.]),
    });
    briefed.rebuild(0, Vec::new(), &Subjects::new(&subjects), Vec::new());
    let world = World {
        own: own([0., 15_000., 0.], 700.),
        picture: briefed,
        inputs: inputs(vec![store(2, StoreKind::AntiRadiation), bomb(4)], None),
    };
    let mut strike = Strike::new(1);
    strike.step(&world.context(0));
    assert_eq!(strike.target(), Some(0x11));
    assert_eq!(
        strike.selection().map(|s| s.kind),
        Some(StoreKind::AntiRadiation)
    );
}

#[test]
fn a_pass_releases_on_the_gate_exits_turns_back_and_attacks_again() {
    let mut world = quiet_world();
    let mut strike = Strike::new(9);
    let order = strike.step(&world.context(0));
    // Clear, 5,000 ft: no dive gate, so a straight run, already in its final.
    assert_eq!(strike.phase(), Phase::Attack);
    assert_eq!(strike.pass(), Some(Pass::Straight));
    assert_eq!(strike.pass_number(), 1);
    assert!(order.steer.is_some() && order.release.is_none());
    let selection = strike.selection().expect("a selection for the cockpit");
    assert_eq!(
        (selection.target, selection.station),
        (TARGET, StationId(1))
    );
    let (aim, inside) = strike.aim();
    let circle = release_circle_ft(Experience::Average);
    let draw = aim_draw(9, OWN, 1, circle);
    assert_eq!((aim, inside), (draw.offset_ft, draw.inside));
    assert_eq!(selection.aim_ft, aim, "a still target: no lead");
    assert!((selection.gate_ft - (GATE_SHARE * circle).max(GATE_FLOOR_FT)).abs() < 1e-9);

    // The cue far short: no release while the host says not due.
    world.own.position = [0., 3_000., 18_000.];
    world.inputs.solution = Some(solution(TARGET_AT, [0., 0., 21_000.], false));
    assert_eq!(strike.step(&world.context(30)).release, None);
    // Due, inside the zone: the bomb goes, and the run exits.
    let aim_point = [aim[0], 0., 24_000. + aim[1]];
    world.inputs.solution = Some(solution(TARGET_AT, aim_point, true));
    let order = strike.step(&world.context(60));
    assert_eq!(
        order.release,
        Some(Release {
            station: StationId(1),
            target: TARGET
        })
    );
    assert_eq!(strike.phase(), Phase::Exit);
    // The jink inside 10,000 ft: climbing at maximum speed.
    let exit = strike.step(&world.context(61)).steer.unwrap();
    assert_eq!(
        (exit.pitch_deg, exit.speed),
        (JINK_CLIMB_DEG, SpeedRequest::Maximum)
    );
    // Out past 30,000 ft it turns back...
    world.own.position = [0., 6_000., 24_000. + 31_000.];
    world.inputs.solution = Some(solution(TARGET_AT, aim_point, false));
    strike.step(&world.context(4_000));
    assert_eq!(strike.phase(), Phase::TurnBack);
    // ...and once pointing at the target attacks again, a new pass with a
    // new draw.
    world.own.heading_deg = 180.;
    strike.step(&world.context(4_100));
    assert_eq!(strike.phase(), Phase::Attack);
    assert_eq!(strike.pass_number(), 2);
    assert_eq!(strike.aim().0, aim_draw(9, OWN, 2, circle).offset_ft);
}

#[test]
fn a_dead_target_or_no_store_sends_it_home() {
    let mut world = quiet_world();
    let mut strike = Strike::new(9);
    strike.step(&world.context(0));
    assert_eq!(strike.phase(), Phase::Attack);
    // The target falls: the pass ends; at the turn back nothing is left.
    let mut dead = solution(TARGET_AT, [0., 0., 0.], false);
    dead.target_alive = false;
    world.inputs.solution = Some(dead);
    strike.step(&world.context(10));
    assert_eq!(strike.phase(), Phase::Exit);
    let mut fallen = ground(TARGET, TARGET_AT, 0x200, None);
    fallen.object.destroyed = true;
    world.picture = picture(&[fallen], Vec::new());
    world.own.position = [0., 6_000., 60_000.];
    strike.step(&world.context(20));
    world.own.heading_deg = 180.;
    strike.step(&world.context(30));
    assert_eq!(strike.phase(), Phase::Home);
    assert!(strike.home() && strike.selection().is_none());
    // No usable store: home at the first choice.
    let mut world = quiet_world();
    world.inputs.stores = vec![bomb(0)];
    let mut strike = Strike::new(9);
    strike.step(&world.context(0));
    assert!(strike.home());
}

#[test]
fn the_floor_the_zone_and_a_warning_hold_the_release_back() {
    let mut world = quiet_world();
    let mut strike = Strike::new(9);
    strike.force(Some(Pass::Level), None);
    strike.step(&world.context(0));
    assert_eq!(strike.pass(), Some(Pass::Level));
    let (aim, _) = strike.aim();
    let aim_point = [aim[0], 0., 24_000. + aim[1]];
    // Below the 3,000 ft floor a level release waits.
    world.own.position = [0., 2_500., 15_000.];
    world.own.agl_ft = 2_500.;
    world.inputs.solution = Some(solution(TARGET_AT, aim_point, true));
    assert_eq!(strike.step(&world.context(10)).release, None);
    // Above it, but the cue past the record's zone 1 maximum.
    world.own.position = [0., 4_000., 15_000.];
    world.own.agl_ft = 4_000.;
    world.inputs.stores[0].maximum_range_ft = 5_000.;
    assert_eq!(strike.step(&world.context(20)).release, None);
    // In the zone with the sight warning LOW or FRAG.
    world.inputs.stores[0].maximum_range_ft = 20_000.;
    let mut warned = solution(TARGET_AT, aim_point, true);
    warned.warning = true;
    world.inputs.solution = Some(warned);
    assert_eq!(strike.step(&world.context(30)).release, None);
    // Clear of all three it goes.
    world.inputs.solution = Some(solution(TARGET_AT, aim_point, true));
    assert!(strike.step(&world.context(40)).release.is_some());
}

#[test]
fn a_defence_hands_over_and_the_striker_resumes_only_when_safe() {
    let mut world = quiet_world();
    let mut strike = Strike::new(9);
    strike.step(&world.context(0));
    let mut cx = world.context(10);
    cx.defending = true;
    let order = strike.step(&cx);
    assert_eq!((strike.phase(), order.steer), (Phase::Defend, None));
    // A warning keeps it there for 10 s.
    let mut cx = world.context(20);
    cx.warnings.surface_to_air = true;
    strike.step(&cx);
    assert_eq!(
        strike.step(&world.context(20 + WARNING_TICKS - 1)).steer,
        None
    );
    // Quiet and safe: back to the attack.
    strike.step(&world.context(20 + WARNING_TICKS));
    assert_eq!(strike.phase(), Phase::Attack);
    // Quiet but a SAM covers the target: it egresses, and with the SAM
    // still there at the exit distance it goes home.
    world.picture = picture(
        &[
            ground(TARGET, TARGET_AT, 0x200, None),
            ground(0x20, [0., 0., 26_000.], 0x1000, site(30_000., 0., true)),
        ],
        vec![0x20],
    );
    let mut cx = world.context(5_000);
    cx.defending = true;
    strike.step(&cx);
    strike.step(&world.context(5_001));
    assert_eq!(strike.phase(), Phase::Exit);
    world.own.position = [0., 6_000., -10_000.];
    strike.step(&world.context(6_000));
    assert_eq!(strike.phase(), Phase::Home);
}

#[test]
fn a_strike_round_trips_through_a_checkpoint_and_steps_on_identically() {
    let mut world = quiet_world();
    let mut strike = Strike::new(42);
    strike.force(Some(Pass::Dive), Some(StationId(1)));
    for tick in 0..200 {
        world.own.position[2] += 5.;
        strike.step(&world.context(tick));
    }
    let models = Models::default();
    let coded = to_bytes(&strike, &models).unwrap();
    let mut twin: Strike = from_bytes(&coded, &models).unwrap();
    assert_eq!(twin, strike);
    for tick in 200..400 {
        world.own.position[2] += 5.;
        let a = strike.step(&world.context(tick));
        let b = twin.step(&world.context(tick));
        assert_eq!(a, b, "tick {tick}");
    }
    assert_eq!(twin, strike);
    // The host's inputs never outlive a step.
    strike.set_inputs(inputs(vec![bomb(1)], None));
    let coded = to_bytes(&strike, &models).unwrap();
    let restored: Strike = from_bytes(&coded, &models).unwrap();
    assert!(restored.inputs().is_none());
}
