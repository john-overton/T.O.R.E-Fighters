//! Parked aircraft (plan section 9.1, slice PA1): a surface target for
//! weapons, an aircraft for damage.
use super::tests::{fixture, target};
use super::*;
use crate::combat::blast::{self, MarkKind};
use crate::combat::missiles::Profile;

/// A template parked aircraft's id (plan 2.5).
const PARKED: u32 = 0x5000_0008;
const REDFOR: Side = Side(2);

fn parked(position: Vector, aircraft: Option<AircraftId>) -> ParkedAircraft {
    ParkedAircraft {
        id: PARKED,
        position,
        basis: Basis::new(0., 0., 0.),
        ground: [position[0], 0., position[2]],
        water: false,
        hit_points: 100,
        category: 0x8000,
        side: REDFOR,
        radar_signature: 60.,
        infrared_signature: 80.,
        fragment_offsets: [[0., 0., 20.], [-12., 0., -4.]],
        aircraft,
    }
}
/// A scene with one parked aircraft 3,000 ft ahead of the origin.
fn scene(water: bool) -> State {
    let mut s = fixture(true);
    s.targets.clear();
    let mut p = parked([0., 8., 3000.], None);
    p.water = water;
    s.add_parked_aircraft(&p).unwrap();
    s
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
fn run(s: &mut State, ticks: usize, water: bool) -> Vec<Event> {
    (0..ticks)
        .flat_map(|_| {
            s.step_surface(
                &[OwnshipInput {
                    aircraft: 0,
                    held: false,
                    launcher: far(),
                }],
                |_, _| 0.,
                |_, _| water,
            )
        })
        .collect()
}
fn record(source: &str, damage: i16) -> Weapon {
    let mut w = fixture(false).own().configuration().stations[0]
        .weapon
        .clone();
    w.source = source.into();
    w.seeker.signature = 0;
    w.damage.by_class = [damage; 5];
    w
}
/// An unguided round flying along +z from `from`, owned by the ownship.
fn round(id: u32, w: Weapon, from: Vector) -> Projectile {
    toward(id, w, from, [0., 0., 1.])
}
fn toward(id: u32, w: Weapon, from: Vector, direction: Vector) -> Projectile {
    Projectile {
        id,
        owner: 0,
        weapon: Some(w),
        guidance: None,
        motion: None,
        guidance_ticks: None,
        age: 0,
        incoming: None,
        station: 0,
        position: from,
        previous: from,
        direction,
        speed_f8: 1500 << 8,
        launched_t: 0,
        target: None,
        fall: FallState::default(),
        gun_round: None,
        tracer: false,
    }
}
fn row(s: &State) -> &Target {
    s.targets.iter().find(|t| t.id == PARKED).unwrap()
}

#[test]
fn a_parked_aircraft_is_a_surface_target_on_the_ground_with_its_engines_off() {
    let s = scene(false);
    let t = row(&s);
    assert!(s.is_parked(PARKED));
    assert_eq!(t.role, TargetRole::Surface);
    assert!(t.on_ground && !t.airborne);
    assert_eq!(
        t.heat,
        Heat::Engine {
            on: false,
            throttle: 0.,
            afterburner: false
        }
    );
    assert_eq!(t.radius, AIRCRAFT_RADIUS_FT);
    assert_eq!((t.signature.radar, t.signature.infrared), (60., 80.));
    assert_eq!((t.category, t.side, t.hp), (0x8000, REDFOR, 100));
    // A Maverick locks it by its surface contrast; a heat-seeking
    // air-to-air missile never takes it.
    let view = seeker::View {
        position: [0., 3000., 0.],
        basis: Basis::new(0., -0.6, 0.),
        cap: None,
        obscured: &|_, _| false,
    };
    let mut maverick = record("AGM65G.JT", 100);
    maverick.seeker.signature = 2;
    let profile = Profile::for_weapon(&maverick).unwrap();
    assert!(profile.accepts(t));
    assert!(seeker::observe(&maverick, profile, &view, t).is_some());
    let mut sidewinder = record("AIM9M.JT", 100);
    sidewinder.seeker.signature = 2;
    assert!(!Profile::for_weapon(&sidewinder).unwrap().accepts(t));
    // A duplicate or dead registration is refused.
    let mut again = scene(false);
    assert!(again.add_parked_aircraft(&parked([0.; 3], None)).is_err());
    let mut dead = parked([0.; 3], None);
    dead.id = PARKED + 1;
    dead.hit_points = 0;
    assert!(again.add_parked_aircraft(&dead).is_err());
}

#[test]
fn radar_does_not_see_a_parked_aircraft() {
    let mut s = scene(false);
    let mut l = far();
    l.position = [0., 3000., 0.];
    for _ in 0..240 {
        s.step(
            &[OwnshipInput {
                aircraft: 0,
                held: false,
                launcher: l,
            }],
            |_, _| 0.,
        );
    }
    assert!(s.own().sensors.contact(PARKED).is_none());
}

#[test]
fn gun_rounds_meet_the_aircraft_volume_and_damage_its_sections() {
    // Through the left wing (right -0.7 radius): an aircraft section, not a
    // ground box.
    let mut s = scene(false);
    let left = -0.7 * AIRCRAFT_RADIUS_FT;
    s.projectiles
        .push(round(9, record("M61.JT", 10), [left, 8., 2900.]));
    let events = run(&mut s, 30, false);
    assert!(events.contains(&Event::Hit(PARKED)));
    let t = row(&s);
    assert!(t.hp < 100 && t.hp > 0);
    let amounts = t.localized_damage.amounts;
    assert!(amounts[DamageSection::LeftWing as usize] > 0, "{amounts:?}");
    assert_eq!(amounts.iter().filter(|a| **a > 0).count(), 1);
    // No crater or fire while it stands.
    assert!(s.marks.is_empty());
    // A gun round that passes beside the wing tip misses: the volume is the
    // aircraft's boxes, not a sphere.
    let mut s = scene(false);
    s.projectiles.push(round(
        9,
        record("M61.JT", 10),
        [
            -0.9 * AIRCRAFT_RADIUS_FT,
            8. + 0.3 * AIRCRAFT_RADIUS_FT,
            2900.,
        ],
    ));
    run(&mut s, 30, false);
    assert_eq!(row(&s).hp, 100);
}

#[test]
fn destroyed_it_explodes_as_an_aircraft_burns_and_leaves_a_crater_on_land() {
    for water in [false, true] {
        let mut s = scene(water);
        s.projectiles
            .push(round(9, record("MK82.JT", 500), [0., 8., 2900.]));
        let events = run(&mut s, 30, water);
        assert!(events.contains(&Event::Destroyed(PARKED)));
        let blast = s
            .effects
            .iter()
            .find(|e| e.kind == EffectKind::Destroyed)
            .and_then(|e| e.blast)
            .unwrap();
        // Type 30 with its usual variety (24 to 33), never the ground
        // object's 35.
        assert!((24..=33).contains(&blast), "{blast}");
        let craters: Vec<_> = s
            .marks
            .iter()
            .filter_map(|m| match m.kind {
                MarkKind::Crater(size) => Some((size, m.position)),
                _ => None,
            })
            .collect();
        let fires: Vec<_> = s
            .marks
            .iter()
            .filter(|m| m.kind == MarkKind::Fire)
            .collect();
        assert_eq!(fires.len(), 1);
        assert_eq!(fires[0].position, [0., 0., 3000.]);
        assert!(fires[0].ticks > blast::CRASH_TICKS - 60, "15 minutes");
        if water {
            assert!(craters.is_empty());
        } else {
            assert_eq!(craters, [(blast::CRASH_CRATER, [0., 0., 3000.])]);
        }
        // The kill goes to the shooter under the PT's class word, not as an
        // aircraft of the roster.
        let kills = s.ledger.kills();
        assert_eq!(kills.len(), 1);
        assert_eq!((kills[0].category, kills[0].aircraft), (0x8000, false));
        // Once only, and the wreck is no longer a body to hit.
        s.projectiles
            .push(round(10, record("MK82.JT", 500), [0., 8., 2900.]));
        run(&mut s, 30, water);
        assert_eq!(
            s.marks.iter().filter(|m| m.kind == MarkKind::Fire).count(),
            1
        );
    }
}

#[test]
fn a_broken_section_throws_the_types_fragment_from_its_attachment_point() {
    // A type the game does not fly: a break at the nose throws piece 0, at
    // the left wing piece 1, from the given offsets.
    // Gun rounds from ahead meet the nose or the left wing first.
    let ahead = |right: f64| toward(9, record("M61.JT", 3000), [right, 8., 3100.], [0., 0., -1.]);
    for (right, variant) in [(0., 0usize), (-20., 1)] {
        let mut s = scene(false);
        s.projectiles.push(ahead(right));
        run(&mut s, 10, false);
        assert_eq!(row(&s).hp, 0);
        assert_eq!(s.debris.len(), 1, "{right}");
        let piece = &s.debris[0];
        assert_eq!((piece.owner, piece.variant), (PARKED, variant));
        assert!(piece.velocity[1] > 0., "tossed up");
    }
    // The Rafale flies: its reviewed pair says a left-wing break is piece 0.
    let mut s = fixture(true);
    s.targets.clear();
    s.add_parked_aircraft(&parked([0., 8., 3000.], Some(AircraftId::Rafale)))
        .unwrap();
    s.projectiles.push(ahead(-20.));
    run(&mut s, 10, false);
    let piece = &s.debris[0];
    assert_eq!(piece.variant, 0);
    // Released at the attachment point: offset 0 from the origin.
    let start: Vector = [0., 8. + 0., 3000. + 20.];
    for axis in 0..3 {
        assert!((piece.position[axis] - start[axis]).abs() < 6., "{piece:?}");
    }
}

#[test]
fn surface_rounds_pass_a_parked_aircraft_by() {
    let mut s = scene(false);
    let mut w = record("ZSU23.JT", 10);
    w.flags = 0x140c0;
    w.movement.remove_t = 20;
    s.fire_surface(SurfaceShot {
        owner: 0x5000_0001,
        weapon: w,
        mount: 0,
        position: [0., 8., 2900.],
        direction: [0., 0., 1.],
        velocity: [0., 0., 1500.],
        target: None,
        observation: None,
        ordinal: 0,
        end_tick: None,
    })
    .unwrap();
    run(&mut s, 30, false);
    assert_eq!(row(&s).hp, 100);
}

#[test]
fn scene_replacement_removes_parked_aircraft_and_a_checkpoint_keeps_them() {
    use crate::checkpoint::{Models, from_bytes, to_bytes};
    let mut s = scene(false);
    s.targets.push(target(3, [0., 5000., 0.], 50, 0x8000));
    s.projectiles
        .push(round(9, record("MK82.JT", 500), [0., 8., 2900.]));
    run(&mut s, 2, false);
    let models = Models::default();
    let copy: State = from_bytes(&to_bytes(&s, &models).unwrap(), &models).unwrap();
    assert_eq!(copy.parked, s.parked);
    assert_eq!(copy.marks, s.marks);
    assert_eq!(copy.debris, s.debris);
    s.remove_ground_targets();
    assert!(!s.is_parked(PARKED));
    assert_eq!(s.targets.iter().map(|t| t.id).collect::<Vec<_>>(), [3]);
    // Registered again, it stands whole.
    s.add_parked_aircraft(&parked([0., 8., 3000.], None))
        .unwrap();
    assert_eq!(row(&s).hp, 100);
}

#[test]
fn a_fuze_radius_does_not_reach_a_parked_aircraft() {
    let mut s = scene(false);
    let mut bomb = record("MK82.JT", 500);
    bomb.damage.fuze_radius = 100;
    // Dropped 120 ft above it: inside fuze radius plus sphere, but it falls
    // to the aircraft before it bursts.
    s.projectiles
        .push(toward(9, bomb, [0., 128., 3000.], [0., -1., 0.]));
    run(&mut s, 1, false);
    assert_eq!(row(&s).hp, 100, "no burst at the release point");
    let events = run(&mut s, 20, false);
    assert!(events.contains(&Event::Destroyed(PARKED)));
    let blast = s
        .effects
        .iter()
        .find(|e| e.kind == EffectKind::Destroyed)
        .unwrap();
    assert!(blast.position[1] < 8. + AIRCRAFT_RADIUS_FT + 13.);
}
