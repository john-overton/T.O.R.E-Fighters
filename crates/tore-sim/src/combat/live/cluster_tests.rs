//! Cluster bombs in the live adapter (A2G plan decision 13, slice X1): the
//! retail wide splash and no crater. The bomblet flashes are presentation and
//! are tested with the renderer (`effect_renderer/cluster.rs`).
use super::tests::fixture;
use super::*;
use crate::airport::OrientedBox;

const RED: Side = Side(2);

/// A cluster bomb as CBU-87 has it (retail data): bomb flag with the 0x800000
/// cluster flag, type 18 explosions for object, land and water, no crater,
/// 200 damage to every class, a 100 ft fuze, 2,200 ft of splash at 100 percent.
fn cbu87() -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = "CBU87.JT".into();
    w.flags = 0x82a052;
    w.movement.minimum_speed = 0;
    w.movement.maximum_speed = 2933;
    w.movement.initial_speed = 500;
    w.movement.final_speed = 278;
    w.movement.deceleration = 14;
    w.movement.launch_retard = 90;
    w.movement.remove_t = 240;
    w.damage.by_class = [200; 5];
    w.damage.fuze_radius = 100;
    w.damage.fuze_arm_t = 4;
    w.damage.collateral_radius = 2200;
    w.damage.collateral_percent = 100;
    w.effects.object_explosion = 18;
    w.effects.land_explosion = 18;
    w.effects.water_explosion = 18;
    w.effects.crater_size = 0;
    w.seeker.signature = 0;
    w
}

fn round(id: u32, w: &Weapon, from: Vector) -> Projectile {
    Projectile {
        id,
        owner: 9,
        weapon: Some(w.clone()),
        guidance: None,
        motion: None,
        guidance_ticks: None,
        age: 0,
        incoming: None,
        station: 0,
        position: from,
        previous: from,
        direction: [0., -1., 0.],
        speed_f8: i32::from(w.movement.initial_speed) * 256,
        launched_t: 0,
        target: None,
        fall: FallState::default(),
        gun_round: None,
        tracer: false,
    }
}

fn far() -> Launcher {
    Launcher {
        position: [90_000., 30_000., -90_000.],
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

#[test]
fn a_cluster_burst_splashes_the_records_radius_in_a_line_and_leaves_no_crater() {
    let mut s = fixture(false);
    s.targets.clear();
    // Vehicles 100 ft to 2,500 ft east of where the bomb lands (15 ft half
    // length, so each surface is 15 ft nearer than its centre).
    let east = [100., 500., 1_000., 1_500., 2_000., 2_300., 2_500.];
    for (n, e) in east.iter().enumerate() {
        s.add_ground_target(
            100 + n as u32,
            OrientedBox {
                center: [*e, 8., 0.],
                half: [15., 8., 15.],
                heading: 0.,
                pitch: 0.,
                bank: 0.,
            },
            1_000,
            0x800,
            RED,
        )
        .unwrap();
    }
    s.projectiles.push(round(70, &cbu87(), [0., 1_500., 0.]));
    // Until the bomb bursts (it falls past its 1 s arming time first, or it is
    // a dud): its flash lasts only a second.
    for _ in 0..600 {
        if s.projectiles.is_empty() {
            break;
        }
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: far(),
            }],
            |_, _| 0.,
        );
    }
    assert!(s.projectiles.is_empty(), "the bomb burst");
    // A ground impact with the type 18 family's explosion, and no crater.
    let burst = s
        .effects
        .iter()
        .find(|e| e.kind == EffectKind::Ground)
        .expect("a ground burst");
    assert!(
        matches!(burst.blast, Some(18..=20 | 24..=26 | 28 | 29)),
        "{:?}",
        burst.blast
    );
    assert!(burst.position[0].abs() < 1., "{:?}", burst.position);
    assert!(
        s.marks
            .iter()
            .all(|m| !matches!(m.kind, crate::combat::blast::MarkKind::Crater(_))),
        "no crater"
    );
    // The record's damage, 70 percent of it for a ground burst, falling in a
    // straight line to nothing at 2,200 ft from the vehicle's surface.
    let lost: Vec<i32> = (0..east.len() as u32)
        .map(|n| 1_000 - s.targets.iter().find(|t| t.id == 100 + n).unwrap().hp)
        .collect();
    for (e, lost) in east.iter().zip(&lost) {
        let distance = (e - 15.).max(0.) as i64;
        let percent = if distance >= 2_200 {
            0
        } else {
            (100 - 100 * distance / 2_200) * 70 / 100
        };
        assert_eq!(i64::from(*lost), 200 * percent / 100, "{e} ft east");
    }
    assert!(lost[0] > lost[3] && lost[3] > lost[4] && lost[4] > lost[5]);
    assert_eq!(lost[6], 0, "beyond the splash");
}
