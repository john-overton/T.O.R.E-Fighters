//! Page keys: an unchanged key means an unchanged raster, a change a page
//! draws changes its key, and a scripted run of frames through every page
//! composes the same canvas as drawing every page every frame.
use super::*;
use crate::{navigation::Destination, scope};
use std::collections::BTreeMap;

/// Every page id the instruments draw.
const PAGES: [u8; 10] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9];

fn fixture() -> (Airframe, State, Instruments) {
    let (airframe, state, mut panels) = benchmark::fixture(true);
    panels.navigation.waypoints = (0..5)
        .map(|n| Destination {
            id: n + 1,
            name: format!("Waypoint {n}"),
            position: [
                state.position[0] + 9000. * f64::from(n),
                0.,
                state.position[2] + 40_000.,
            ],
        })
        .collect();
    let scene = tore_sim::airport::Scene::default();
    let service = tore_sim::airport::Service::new(&scene).unwrap();
    panels
        .navigation
        .refresh(&scene, &service, state.position, false);
    assert!(panels.navigation.index().is_some());
    for id in [2, 3] {
        panels.cameras.insert(
            id,
            (0..138 * 114)
                .flat_map(|i| [(i % 199) as u8, id * 40, 9, 255])
                .collect(),
        );
    }
    let combat = panels.combat.as_mut().unwrap();
    combat.rcs = scope::Rcs {
        contour: (0..24)
            .map(|n| (f64::from(n) * 0.26, 4. + f64::from(n % 5)))
            .collect(),
        emitters: vec![
            scope::Emitter {
                bearing_rad: 0.5,
                distance_nmi: Some(3.),
                symbol: tore_sim::sensors::passive::Symbol::Ground,
            },
            scope::Emitter {
                bearing_rad: 2.5,
                distance_nmi: None,
                symbol: tore_sim::sensors::passive::Symbol::Aircraft,
            },
        ],
        scale_nmi: 10.,
        signature: 4.4,
    };
    combat.weapons = (0..9)
        .map(|n| (format!("Store {n}"), n * 2, n == 3))
        .collect();
    combat.gun_group = Some("M61 570".into());
    combat.scope.marks = vec![scope::LinkMarks {
        id: 7,
        locked_by: vec![2, 3, 4],
        assigned_to: vec![1],
        assigned: Some(false),
    }];
    (airframe, state, panels)
}

/// Frame `f` of a scripted flight: most inputs creep by amounts that often
/// do not move a pixel; blinks, presses, palette, pictures and lists change
/// now and then.
fn step(state: &mut State, panels: &mut Instruments, f: u64) {
    state.ticks = f * 7;
    state.throttle = 0.5 + f as f64 * 0.0013;
    state.fuel -= 0.37;
    state.speed += 0.21;
    state.g = 1. + (f as f64 * 0.05).sin() * 0.7;
    state.position[0] += 30.;
    state.position[1] += 0.1;
    state.velocity = [200., 0., 300. + f as f64 * 0.1];
    state.jammer = f % 90 > 60;
    state.engine = true;
    panels.hovered = (f % 40 > 30).then_some(7);
    panels.crosshair = (f % 50 > 25).then_some((30 + (f / 10) as i32, 40));
    panels.pressed = f
        .is_multiple_of(25)
        .then_some(((f / 25 % 4) as usize, (f % 3) as usize));
    if f.is_multiple_of(50) {
        panels.palette[(f / 50 % 256) as usize][1] ^= 0x10;
    }
    if f.is_multiple_of(6) {
        for pixel in panels
            .cameras
            .values_mut()
            .flat_map(|p| p.chunks_exact_mut(4))
        {
            pixel[0] = pixel[0].wrapping_add(1);
        }
    }
    if f.is_multiple_of(30) {
        panels.sight_bloom = (f % 90) as f64 / 90.;
    }
    if f.is_multiple_of(70) {
        panels.weapon_page += 1;
        panels.history = !panels.history;
    }
    let combat = panels.combat.as_mut().unwrap();
    combat.scope.tick = state.ticks;
    combat.rwr.tick = state.ticks;
    for contact in &mut combat.scope.contacts {
        contact.bearing_rad += 0.0003;
        contact.distance_ft -= 2.;
    }
    if f.is_multiple_of(15) {
        combat.scope.strobes[0].density = 0.1 + (f % 45) as f64 / 300.;
    }
    for emitter in &mut combat.rwr.emitters {
        emitter.bearing_rad += 0.0004;
    }
    for (n, contour) in combat.rcs.contour.iter_mut().enumerate() {
        contour.1 += if f.is_multiple_of(10) && n == (f / 10 % 24) as usize {
            0.3
        } else {
            0.001
        };
    }
    if f.is_multiple_of(40) {
        combat.weapons[(f / 40 % 9) as usize].1 += 1;
        combat.chaff = (f / 40) as u8;
    }
    if f.is_multiple_of(20)
        && let Some(target) = &mut combat.target
    {
        target.metric = format!("{:.1} NMI", 10. - f as f64 / 100.);
        target.damage += 0.01;
    }
}

#[test]
fn an_unchanged_page_key_means_an_unchanged_raster_through_a_scripted_flight() {
    let (airframe, mut state, mut panels) = fixture();
    let mut last: BTreeMap<u8, (Option<PageKey>, Vec<u8>)> = BTreeMap::new();
    let mut kept = BTreeMap::<u8, usize>::new();
    let mut changed = BTreeMap::<u8, usize>::new();
    for f in 0..600 {
        step(&mut state, &mut panels, f);
        // Some frames with each page in a window, so presses reach it.
        panels.pages = vec![PAGES[(f % 10) as usize], 9, 5, 4];
        for id in PAGES {
            let key = panels.page_key(id, &state);
            assert!(key.is_some(), "page {id} has a key");
            let raster = panels.page(id, &airframe, &state).pixels;
            if let Some((previous, pixels)) = last.get(&id) {
                if *previous == key {
                    assert!(
                        *pixels == raster,
                        "page {id} frame {f}: same key, new pixels"
                    );
                    *kept.entry(id).or_default() += 1;
                } else {
                    *changed.entry(id).or_default() += 1;
                }
            }
            last.insert(id, (key, raster));
        }
    }
    // Every page both held its key across frames and changed it.
    for id in PAGES {
        assert!(
            kept.get(&id).copied().unwrap_or(0) > 20,
            "page {id} kept {kept:?}"
        );
        assert!(
            changed.get(&id).copied().unwrap_or(0) > 2,
            "page {id} changed {changed:?}"
        );
    }
}

#[test]
fn what_a_page_draws_is_in_its_key() {
    let (_, state, panels) = fixture();
    let key = |panels: &Instruments, state: &State, id: u8| panels.page_key(id, state);
    let differs = |id: u8, change: &dyn Fn(&mut State, &mut Instruments)| {
        let (mut s, mut p) = (state.clone(), fixture().2);
        change(&mut s, &mut p);
        key(&p, &s, id) != key(&panels, &state, id)
    };
    // Systems: the fuel as printed, not every fraction of a pound.
    let fuel = state.fuel.round();
    let with_fuel = |pounds: f64| {
        let mut s = state.clone();
        s.fuel = pounds;
        key(&panels, &s, 7)
    };
    assert_eq!(with_fuel(fuel + 0.1), with_fuel(fuel + 0.2));
    assert_ne!(with_fuel(fuel + 0.1), with_fuel(fuel + 1.));
    assert!(differs(7, &|s, _| s.throttle += 0.02));
    // RWR: its blink and its plotted symbols.
    assert!(differs(5, &|_, p| p.combat.as_mut().unwrap().rwr.tick += 60));
    assert!(!differs(5, &|_, p| p.combat.as_mut().unwrap().rwr.tick += 120));
    assert!(differs(5, &|_, p| {
        p.combat.as_mut().unwrap().rwr.emitters[3].bearing_rad += 0.2
    }));
    // Radar: the noise phase, a contact moved a pixel, hover, the crosshair.
    assert!(differs(9, &|_, p| p.combat.as_mut().unwrap().scope.tick += 12));
    assert!(differs(9, &|_, p| {
        p.combat.as_mut().unwrap().scope.contacts[2].bearing_rad += 0.05
    }));
    assert!(differs(9, &|_, p| p.hovered = Some(3)));
    assert!(differs(9, &|_, p| p.crosshair = Some((10, 10))));
    // Every page: the palette and a held button in its window.
    for id in PAGES {
        assert!(differs(id, &|_, p| p.palette[200][2] ^= 1), "page {id}");
        let slot = panels.pages.iter().position(|p| *p == id);
        if let Some(slot) = slot {
            assert!(
                differs(id, &|_, p| p.pressed = Some((slot, 1))),
                "page {id}"
            );
        }
    }
    // Pictures: a camera pixel and the target camera's bloom.
    assert!(differs(4, &|_, p| p.cameras.get_mut(&4).unwrap()[5] ^= 1));
    assert!(differs(4, &|_, p| p.sight_bloom = 0.5));
    assert!(differs(2, &|_, p| p.cameras.get_mut(&2).unwrap()[5] ^= 1));
    assert!(!differs(2, &|_, p| p.cameras.get_mut(&3).unwrap()[5] ^= 1));
    // Navigation: the distance as printed, the waypoint list.
    assert!(differs(6, &|s, _| s.position[2] += 2000.));
    assert!(differs(6, &|_, p| p.navigation.waypoints[0].name.push('X')));
    // Weapons: a count, the chaff.
    assert!(differs(8, &|_, p| p.combat.as_mut().unwrap().weapons[1].1 += 1));
    assert!(differs(8, &|_, p| p.combat.as_mut().unwrap().chaff += 1));
    // Envelope: the marker's flash and its position.
    assert!(differs(1, &|s, _| s.ticks += 20));
    assert!(differs(1, &|s, _| s.speed += 100.));
    // RCS: a contour point moved well.
    assert!(differs(0, &|_, p| p
        .combat
        .as_mut()
        .unwrap()
        .rcs
        .contour[4]
        .1 += 3.));
}

#[test]
fn a_scripted_flight_composes_what_drawing_every_page_every_frame_does() {
    let (airframe, mut state, mut panels) = fixture();
    let mut reference = worker_tests::ReferenceCanvas::default();
    let mut canvas = FlightCanvas::default();
    let [kept_before, ..] = page_key_stats();
    for f in 0..240 {
        step(&mut state, &mut panels, f);
        let size = match f / 60 {
            0 => [1920, 1080],
            1 => {
                panels.layout = crate::instruments::Layout::Small;
                panels.pages = vec![7, 5, 6, 9, 8, 4];
                [1280, 720]
            }
            2 => {
                panels.pages = vec![0, 1, 2, 3, 9, 4];
                [1280, 720]
            }
            _ => {
                panels.layout = crate::instruments::Layout::Large;
                panels.pages = vec![7, 5, 9, 4];
                [3840, 2160]
            }
        };
        reference.begin(size, &airframe, &state, &panels);
        canvas.begin(size, &airframe, &state, &panels);
        reference.assert_same(&canvas);
    }
    // Pages were kept by key (and, in this debug build, drawn again and
    // compared each time).
    assert!(page_key_stats()[0] > kept_before + 100);
}

#[test]
fn a_new_airframe_drops_the_kept_pages() {
    let (mut airframe, state, panels) = fixture();
    let mut canvas = FlightCanvas::default();
    canvas.begin([640, 480], &airframe, &state, &panels);
    let before = canvas.pixels.clone();
    for glyph in &mut airframe.font.glyphs {
        glyph.pixels.retain(|(x, _)| *x != 0);
    }
    canvas.begin([640, 480], &airframe, &state, &panels);
    let mut reference = worker_tests::ReferenceCanvas::default();
    reference.begin([640, 480], &airframe, &state, &panels);
    reference.assert_same(&canvas);
    assert!(canvas.pixels != before);
}
