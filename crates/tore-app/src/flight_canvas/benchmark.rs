//! Opt-in portable canvas cost probe. The stage copy is checked against begin;
//! it adds no clocks, flags or measurements to production frame composition.
use super::*;
use crate::{instruments::CombatReadout, scope};
use std::{hint::black_box, time::Instant};

#[derive(Default)]
struct Cost {
    // Clear, page raster, cache compare/update, scaling, composition.
    micros: [f64; 5],
    total_micros: f64,
    hits: usize,
    misses: usize,
}

fn staged(
    canvas: &mut FlightCanvas,
    size: [u32; 2],
    airframe: &Airframe,
    state: &State,
    panels: &Instruments,
) -> Cost {
    let total_started = Instant::now();
    let mut cost = Cost::default();
    let started = Instant::now();
    canvas.size = size;
    canvas
        .pixels
        .resize(size[0] as usize * size[1] as usize * 4, 0);
    canvas.pixels.fill(0);
    cost.micros[0] += started.elapsed().as_secs_f64() * 1e6;
    let (width, height) = (f64::from(size[0]), f64::from(size[1]));
    for (index, page) in panels.pages.iter().enumerate() {
        let started = Instant::now();
        let raster = panels.page(*page, airframe, state);
        cost.micros[1] += started.elapsed().as_secs_f64() * 1e6;

        let started = Instant::now();
        let rect = panels.screen_rect(index, [width, height]);
        let size = [rect.2.round() as u32, rect.3.round() as u32];
        let cached = canvas.panels.remove(page);
        let reuse = cached
            .as_ref()
            .is_some_and(|cached| cached.size == size && cached.source == raster.pixels);
        cost.micros[2] += started.elapsed().as_secs_f64() * 1e6;

        let cached = if reuse {
            cost.hits += 1;
            cached.unwrap()
        } else {
            cost.misses += 1;
            let started = Instant::now();
            let mut scaled = FlightCanvas {
                size,
                pixels: vec![0; size[0] as usize * size[1] as usize * 4],
                ..Default::default()
            };
            let source = Sprite {
                width: crate::instruments::WIDTH,
                height: crate::instruments::HEIGHT,
                rgba: raster.pixels,
                glyphs: vec![],
            };
            scaled.blit(&source, (0., 0., f64::from(size[0]), f64::from(size[1])));
            let result = PanelCache {
                source: source.rgba,
                size,
                image: Sprite {
                    width: size[0] as usize,
                    height: size[1] as usize,
                    rgba: scaled.pixels,
                    glyphs: vec![],
                },
            };
            cost.micros[3] += started.elapsed().as_secs_f64() * 1e6;
            Arc::new(result)
        };

        let started = Instant::now();
        canvas.blit(
            &cached.image,
            (
                rect.0.round(),
                rect.1.round(),
                f64::from(size[0]),
                f64::from(size[1]),
            ),
        );
        cost.micros[4] += started.elapsed().as_secs_f64() * 1e6;
        let started = Instant::now();
        canvas.panels.insert(*page, cached);
        cost.micros[2] += started.elapsed().as_secs_f64() * 1e6;
    }
    cost.total_micros = total_started.elapsed().as_secs_f64() * 1e6;
    cost
}

pub(super) fn fixture(masked_corners: bool) -> (Airframe, State, Instruments) {
    let mut airframe = crate::combat_view::render_hash_tests::hornet_airframe(false);
    airframe.font = tore_formats::font::Font {
        height: 7,
        glyphs: (0..256)
            .map(|character| tore_formats::font::Glyph {
                advance: 6,
                pixels: (0..7)
                    .flat_map(|y| (0..5).map(move |x| (x, y)))
                    .filter(|(x, y)| character != 32 && (character + x + y) % 3 != 0)
                    .collect(),
            })
            .collect(),
    };
    airframe.panel = tore_formats::Pic {
        width: 81,
        height: 80,
        pixels: (0..81 * 80).map(|i| (i % 251) as u8).collect(),
        mask: (0..81 * 80)
            .map(|index| {
                let (x, y) = (index % 81, index / 81);
                !masked_corners || x.min(80 - x) + y.min(79 - y) >= 6
            })
            .collect(),
        palette: Vec::new(),
        glyphs: Vec::new(),
    };
    let state = airframe.start(&tore_world::test_support::terrain());
    let mut panels = Instruments::new(crate::instruments::Layout::Large, None);
    panels.pages = vec![7, 5, 9, 4];
    panels.palette = airframe.palette;
    panels.camera_target = Some(7);
    panels.cameras = std::collections::BTreeMap::from([(
        4,
        (0..138 * 114)
            .flat_map(|index| [(index % 251) as u8, 100, 50, 255])
            .collect(),
    )]);
    panels.combat = Some(CombatReadout {
        target: Some(crate::target_window::Readout {
            id: 7,
            name: "Synthetic F/A-18D".into(),
            damage: 0.15,
            bearing: "12:00+".into(),
            metric: "10.0 NMI".into(),
            objective: None,
            activity: "ENGAGE".into(),
            goal: "A",
            player_goal: false,
            skill: Some(3),
        }),
        scope: scope::Scope {
            channel: "RADAR",
            mode: Some("RWS"),
            range_nmi: 40.,
            operating: true,
            history: true,
            contacts: (0..30)
                .map(|index| scope::Contact {
                    id: index + 1,
                    bearing_rad: -0.8 + f64::from(index) * 0.05,
                    distance_ft: 12_000. + f64::from(index) * 3000.,
                    heading_rad: Some(f64::from(index) * 0.2),
                    track_eligible: true,
                    destroyed: false,
                    selected: index == 6,
                    acquired: index == 6,
                    stale: false,
                    trail: (0..8)
                        .map(|back| {
                            (
                                -0.8 + f64::from(index) * 0.05 - f64::from(back) * 0.01,
                                12_000. + f64::from(index) * 3000. + f64::from(back) * 80.,
                            )
                        })
                        .collect(),
                })
                .collect(),
            strobes: vec![scope::Strobe {
                bearing_rad: 0.1,
                density: 0.2,
                half_width_rad: 0.1,
                sidelobe: 0.01,
            }],
            selected: Some(7),
            status: Some("LOCK"),
            ..Default::default()
        },
        rwr: scope::Rwr {
            operating: true,
            emitters: (0..30)
                .map(|index| scope::RwrEmitter {
                    id: index + 1,
                    bearing_rad: f64::from(index) * std::f64::consts::TAU / 30.,
                    distance_nmi: Some(2. + f64::from(index)),
                    kind: scope::EmitterKind::EnemyAircraft,
                    state: scope::EmitterState::Tracking,
                })
                .collect(),
            missiles: (0..4)
                .map(|index| scope::RwrMissile {
                    id: 100 + index,
                    bearing_rad: f64::from(index) - 1.,
                    distance_nmi: Some(5. + f64::from(index)),
                    known_targeting_receiver: true,
                    stale: false,
                })
                .collect(),
            radar_indicator: scope::Indicator::Incoming,
            ..Default::default()
        },
        ..Default::default()
    });
    (airframe, state, panels)
}

pub(super) fn change_inputs(state: &mut State, panels: &mut Instruments, frame: u64) {
    state.ticks = frame * 2;
    state.throttle = 0.6 + (frame as f64 * 0.05).sin() * 0.2;
    let combat = panels.combat.as_mut().unwrap();
    combat.scope.tick = state.ticks;
    combat.rwr.tick = state.ticks;
    for contact in &mut combat.scope.contacts {
        contact.bearing_rad += 0.002;
    }
    // A 10 Hz completed preview on a nominal 60 Hz frame stream.
    if frame.is_multiple_of(6) {
        for pixel in panels.cameras.get_mut(&4).unwrap().chunks_exact_mut(4) {
            pixel[0] = pixel[0].wrapping_add(1);
        }
    }
}

#[test]
#[ignore = "opt-in canvas stage timing; run in release mode on a quiet machine"]
fn instrument_canvas_stage_wall_time() {
    let size = [1920, 1080];
    for (masked_corners, changing) in [(false, false), (false, true), (true, false), (true, true)] {
        let (airframe, mut state, mut panels) = fixture(masked_corners);
        let mut canvas = FlightCanvas::default();
        let mut reference = FlightCanvas::default();
        let mut total = Cost::default();
        let samples = 300;
        for frame in 0..samples + 20 {
            if changing {
                change_inputs(&mut state, &mut panels, frame);
            }
            let cost = staged(&mut canvas, size, &airframe, &state, &panels);
            if frame >= 20 {
                for (sum, part) in total.micros.iter_mut().zip(cost.micros) {
                    *sum += part;
                }
                total.hits += cost.hits;
                total.misses += cost.misses;
                total.total_micros += cost.total_micros;
            }
            if frame == 0 || frame == samples + 19 {
                reference.begin(size, &airframe, &state, &panels);
                assert_eq!(
                    canvas.pixels, reference.pixels,
                    "probe must match actual begin"
                );
            }
            black_box(&canvas.pixels);
        }
        println!(
            "instrument_canvas masked_corners={masked_corners} changing={changing} size=1920x1080 contacts=30 emitters=30 pages=7,5,9,4 samples={samples} cache_hits={} cache_misses={} total_mean_us={:.3}",
            total.hits,
            total.misses,
            total.total_micros / samples as f64,
        );
        for (name, micros) in ["clear", "raster", "cache", "scale", "compose"]
            .into_iter()
            .zip(total.micros)
        {
            println!(
                "instrument_canvas stage={name} mean_us={:.3}",
                micros / samples as f64
            );
        }
    }
}

#[test]
#[ignore = "opt-in complete canvas timing; run in release mode on a quiet machine"]
fn instrument_canvas_workers_wall_time() {
    for changing in [false, true] {
        for workers in [0, 1, 2, 4, 8] {
            let executor = tore_workers::Executor::parallel(workers).unwrap();
            let (airframe, mut state, mut panels) = fixture(true);
            let mut canvas = FlightCanvas::default();
            let mut reference = super::worker_tests::ReferenceCanvas::default();
            let samples = 120;
            let mut micros = Vec::with_capacity(samples);
            for frame in 0..samples + 20 {
                if changing {
                    change_inputs(&mut state, &mut panels, frame as u64);
                }
                let start = Instant::now();
                canvas.begin_with(&executor, [1920, 1080], &airframe, &state, &panels);
                let elapsed = start.elapsed().as_secs_f64() * 1e6;
                if frame >= 20 {
                    micros.push(elapsed);
                }
                if frame == 0 || frame == samples + 19 {
                    reference.begin([1920, 1080], &airframe, &state, &panels);
                    reference.assert_same(&canvas);
                }
                black_box(&canvas.pixels);
            }
            micros.sort_by(f64::total_cmp);
            println!(
                "instrument_canvas_workers changing={changing} workers={workers} samples={samples} mean_us={:.3} p50_us={:.3} p95_us={:.3} p99_us={:.3} max_us={:.3}",
                micros.iter().sum::<f64>() / samples as f64,
                micros[samples / 2],
                micros[(samples - 1) * 95 / 100],
                micros[(samples - 1) * 99 / 100],
                micros[samples - 1],
            );
        }
    }
}
