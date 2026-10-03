use super::*;
use crate::instruments::Layout;
use std::collections::BTreeMap;

/// The pre-threading begin body with its original owned cache. It deliberately
/// shares neither prepare_panel nor the new grouping/publication algorithm.
#[derive(Default)]
pub(super) struct ReferenceCanvas {
    canvas: FlightCanvas,
    panels: BTreeMap<u8, PanelCache>,
}

impl ReferenceCanvas {
    pub(super) fn begin(&mut self, size: [u32; 2], h: &Airframe, s: &State, panels: &Instruments) {
        self.canvas.size = size;
        self.canvas
            .pixels
            .resize(size[0] as usize * size[1] as usize * 4, 0);
        let (w, height) = (size[0] as f64, size[1] as f64);
        self.canvas.pixels.fill(0);
        for (i, page) in panels.pages.iter().enumerate() {
            let raster = panels.page(*page, h, s);
            let rect = panels.screen_rect(i, [w, height]);
            let size = [rect.2.round() as u32, rect.3.round() as u32];
            let cached = self.panels.remove(page);
            let cached = match cached {
                Some(cached) if cached.size == size && cached.source == raster.pixels => cached,
                _ => {
                    let mut canvas = FlightCanvas {
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
                    canvas.blit(&source, (0., 0., size[0] as f64, size[1] as f64));
                    PanelCache {
                        source: source.rgba,
                        size,
                        image: Sprite {
                            width: size[0] as usize,
                            height: size[1] as usize,
                            rgba: canvas.pixels,
                            glyphs: vec![],
                        },
                    }
                }
            };
            self.canvas.blit(
                &cached.image,
                (
                    rect.0.round(),
                    rect.1.round(),
                    size[0] as f64,
                    size[1] as f64,
                ),
            );
            self.panels.insert(*page, cached);
        }
    }

    pub(super) fn assert_same(&self, actual: &FlightCanvas) {
        assert_eq!(actual.size, self.canvas.size);
        assert_bytes_equal(&actual.pixels, &self.canvas.pixels);
        assert_eq!(
            actual.panels.keys().collect::<Vec<_>>(),
            self.panels.keys().collect::<Vec<_>>(),
        );
        for (&page, expected) in &self.panels {
            let cached = &actual.panels[&page];
            assert_eq!(cached.size, expected.size);
            assert_bytes_equal(&cached.source, &expected.source);
            assert_eq!(cached.image.width, expected.image.width);
            assert_eq!(cached.image.height, expected.image.height);
            assert_bytes_equal(&cached.image.rgba, &expected.image.rgba);
            assert!(cached.image.glyphs.is_empty() && expected.image.glyphs.is_empty());
        }
    }
}

fn assert_bytes_equal(actual: &[u8], expected: &[u8]) {
    assert_eq!(actual.len(), expected.len());
    assert!(
        actual == expected,
        "first byte difference {:?}",
        actual.iter().zip(expected).position(|(a, b)| a != b),
    );
}

#[test]
fn panel_workers_match_frozen_begin_across_inputs_and_cache_lifecycles() {
    let executors: Vec<_> = [0, 1, 2, 4, 8]
        .into_iter()
        .map(|count| tore_workers::Executor::parallel(count).unwrap())
        .chain([1, 73, 991].map(tore_workers::Executor::shuffled))
        .collect();
    let (airframe, mut state, mut panels) = benchmark::fixture(true);
    let mut reference = ReferenceCanvas::default();
    let mut canvases: Vec<_> = executors.iter().map(|_| FlightCanvas::default()).collect();
    for step in 0..12 {
        let mut size = [640, 480];
        match step {
            0 | 1 => {} // Cold, then every page reuses its cached image.
            2 => benchmark::change_inputs(&mut state, &mut panels, 6),
            3 => {
                for colour in &mut panels.palette {
                    colour[0] = colour[0].wrapping_add(17);
                }
                size = [1280, 720];
            }
            4 => {
                panels.hovered = Some(7);
                panels.crosshair = Some((40, 45));
                panels.pressed = Some((2, 1));
                panels.weapon_debug = true;
            }
            5 => {
                // Repeated pages must prepare/cache in their own slot order,
                // while previously shown but now absent pages remain cached.
                panels.pages = vec![9, 4, 9, 4];
            }
            6 => {
                panels.layout = Layout::Small;
                panels.pages = vec![9, 4, 5, 9, 7, 4];
                panels.cameras.remove(&4);
                panels.camera_target = None;
                panels.combat.as_mut().unwrap().target = None;
                size = [960, 720];
            }
            7 => panels.pages = vec![9, 9, 9], // One distinct job stays inline.
            8 => panels.pages.clear(),
            9 => {
                panels.layout = Layout::Large;
                panels.pages = vec![7, 5, 9, 4];
                size = [1920, 1080];
            }
            10 => {
                panels.pages = vec![4];
                size = [800, 1000];
            }
            11 => panels.pages = vec![7, 5, 9, 4],
            _ => unreachable!(),
        }
        reference.begin(size, &airframe, &state, &panels);
        for (executor, canvas) in executors.iter().zip(&mut canvases) {
            canvas.begin_with(executor, size, &airframe, &state, &panels);
            reference.assert_same(canvas);
        }
        if step == 0 || step == 5 {
            let old: Vec<_> = canvases
                .iter()
                .map(|canvas| canvas.panels.clone())
                .collect();
            for ((executor, canvas), previous) in executors.iter().zip(&mut canvases).zip(old) {
                canvas.begin_with(executor, size, &airframe, &state, &panels);
                reference.assert_same(canvas);
                for (page, cached) in &canvas.panels {
                    assert!(Arc::ptr_eq(cached, &previous[page]), "warm page {page}");
                }
            }
        }
    }
}
