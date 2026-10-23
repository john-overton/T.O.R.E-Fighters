//! A small frames-per-second readout in the window's top right.
//!
//! Opinionated, requested by John on 2026-10-17. The half-second average, the
//! slowest-frame figure, the default of off, and where the line sits are
//! agent decisions the same day. It is drawn only in the window present
//! pass, with the bundled Noto atlas (`ui_text`), so offscreen renders,
//! previews, recordings and screenshots never include it.
use crate::renderer::Viewport;
use crate::ui_text::{self, Quad, srgb_to_linear};
use std::collections::VecDeque;

/// Capitals of the readout, in window pixels. One line this tall fits in the
/// margin above the large instrument window from 1080p up.
pub const LINE_PX: f32 = 13.0;
/// The average uses about this many seconds of recent frames.
const AVERAGE: f64 = 0.5;
/// The slowest frame is taken from this many seconds.
const WORST: f64 = 1.0;

/// Recent frame durations, newest last, covering at most [`WORST`] seconds.
#[derive(Clone, Debug, Default)]
pub struct Meter {
    last: Option<std::time::Instant>,
    samples: VecDeque<f64>,
    cpu: VecDeque<f64>,
    gpu: VecDeque<f64>,
}

impl Meter {
    /// Main-thread wall time for render preparation and encoding, excluding
    /// swapchain waits (not CPU seconds summed over workers). The GPU figure
    /// is the frame's whole GPU work, panels and mirrors included; samples
    /// arrive asynchronously and never block presentation.
    pub fn work(&mut self, cpu_ms: f64, gpu_ms: Option<f64>) {
        fn add(samples: &mut VecDeque<f64>, value: f64) {
            if value.is_finite() && value >= 0.0 {
                samples.push_back(value);
                if samples.len() > 30 {
                    samples.pop_front();
                }
            }
        }
        add(&mut self.cpu, cpu_ms);
        if let Some(ms) = gpu_ms {
            add(&mut self.gpu, ms);
        }
    }
    /// One continuously presented frame. The first call starts the clock.
    /// Slow frames count too. Disabling the counter resets its history.
    ///
    /// The counter measures continuous drawing. A gap longer than [`WORST`]
    /// means frames were not being drawn (the window was hidden, or the
    /// counter was off), so the clock restarts rather than counting the gap
    /// as one very slow frame.
    pub fn tick(&mut self) {
        self.tick_at(std::time::Instant::now());
    }

    /// [`Meter::tick`] at `now`.
    pub fn tick_at(&mut self, now: std::time::Instant) {
        if let Some(last) = self.last.replace(now) {
            let dt = now.saturating_duration_since(last).as_secs_f64();
            if dt > WORST {
                self.push(0.0);
            } else {
                self.push(dt);
            }
        }
    }

    /// Record one frame of `dt` seconds. Used by [`tick`] and the tests.
    pub fn push(&mut self, dt: f64) {
        if !(dt.is_finite() && dt > 0.0) {
            self.samples.clear();
            self.cpu.clear();
            self.gpu.clear();
            return;
        }
        self.samples.push_back(dt);
        let mut total: f64 = self.samples.iter().sum();
        while total > WORST && self.samples.len() > 1 {
            total -= self.samples.pop_front().unwrap_or(0.0);
        }
    }

    /// `62 fps  16.1 ms  cpu 4.2  gpu 11.8  low 48`, or nothing until a frame has been counted.
    /// The rate and the milliseconds are the mean frame over about the last
    /// half second. `low` is the slowest frame in the last second, as a rate.
    pub fn line(&self) -> Option<String> {
        if self.samples.is_empty() {
            return None;
        }
        let mut sum = 0.0;
        let mut count = 0usize;
        for dt in self.samples.iter().rev() {
            if count > 0 && sum >= AVERAGE {
                break;
            }
            sum += dt;
            count += 1;
        }
        let mean = sum / count as f64;
        let worst = self.samples.iter().copied().fold(0.0, f64::max);
        let time = |v: &VecDeque<f64>| {
            if v.is_empty() {
                "--".to_owned()
            } else {
                format!("{:.1}", v.iter().sum::<f64>() / v.len() as f64)
            }
        };
        Some(format!(
            "{} fps  {:.1} ms  cpu {}  gpu {}  low {}",
            rate(mean),
            mean * 1000.0,
            time(&self.cpu),
            time(&self.gpu),
            rate(worst)
        ))
    }
}

fn rate(seconds: f64) -> u32 {
    if seconds <= 0.0 {
        return 1;
    }
    (1.0 / seconds).round().clamp(1.0, 9999.0) as u32
}

/// Where the line is drawn.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Surface {
    /// A 640 by 480 menu, lobby or similar screen, letterboxed in the window.
    Menu,
    /// The loading photograph. Its words are centred.
    Loading,
    /// Live flight. The large instrument layout reserves the top-right window.
    Flight { right_panel: bool },
    /// The replay viewer. The mission timer is on the left. The HUD camera's
    /// tape counter is in the top right of the picture.
    Replay { right_panel: bool, hud_camera: bool },
}

/// The right edge and the top of the line, in window pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Anchor {
    pub right: f32,
    pub top: f32,
}

/// Where the readout sits so it misses the title strip, the large instrument
/// window, the HUD camera counter and an open right-hand debug panel.
pub fn anchor(size: [u32; 2], surface: Surface) -> Anchor {
    let (w, h) = (size[0].max(1) as f32, size[1].max(1) as f32);
    let anchor = match surface {
        Surface::Loading => Anchor {
            right: w - 10.0,
            top: 10.0,
        },
        Surface::Menu => menu_anchor(size[0].max(1), size[1].max(1)),
        Surface::Flight { right_panel } => view_anchor(w, h, true, right_panel, false),
        Surface::Replay {
            right_panel,
            hud_camera,
        } => view_anchor(w, h, false, right_panel, hud_camera),
    };
    Anchor {
        right: anchor.right.clamp(48.0, (w - 4.0).max(48.0)),
        top: anchor.top.max(2.0),
    }
}

fn menu_anchor(width: u32, height: u32) -> Anchor {
    let view = Viewport::new(width, height);
    let side = width as f32 - (view.x + view.width);
    if side >= 12.0 {
        // The black bar beside the canvas.
        return Anchor {
            right: width as f32 - 10.0,
            top: 10.0,
        };
    }
    // A 4:3 window: under the title strip, clear of the screen-context words.
    let scale = view.width / 640.0;
    Anchor {
        right: view.x + view.width - 8.0,
        top: view.y + 22.0 * scale,
    }
}

/// A rectangle the line must miss: left, top, right, bottom.
fn hits(top: f32, right: f32, block: [f32; 4]) -> bool {
    let left = right - 420.0;
    let bottom = top + LINE_PX + 2.0;
    left < block[2] && right > block[0] && top < block[3] && bottom > block[1]
}

fn view_anchor(w: f32, h: f32, instruments: bool, right_panel: bool, hud_camera: bool) -> Anchor {
    let scale = (w / 640.0).min(h / 480.0);
    let margin = 10.0;
    let mut top = margin;
    let mut right = w - margin;
    // Tight corner, used when it clears the large instrument's top margin
    // (8 reference pixels: 18 window pixels at 1080p, which holds this line).
    let tucked = 2.0;
    let instrument = [
        w - 170.0 * scale,
        8.0 * scale,
        w - 8.0 * scale,
        (8.0 + 160.0) * scale,
    ];
    let tape = [w * 0.70, h * 0.07, w * 0.96, h * 0.07 + LINE_PX + 6.0];
    let panel = [w - (6.0 + 254.0) * scale, 28.0 * scale, w - 6.0 * scale, h];
    let blocked = |top: f32, right: f32| {
        (instruments && hits(top, right, instrument))
            || (hud_camera && hits(top, right, tape))
            || (right_panel && hits(top, right, panel))
    };
    if instruments && blocked(top, right) && !blocked(tucked, right) {
        top = tucked;
    }
    if !blocked(top, right) {
        return Anchor { right, top };
    }
    if instruments {
        top = top.max(instrument[3] + 4.0);
    }
    if hud_camera {
        top = top.max(tape[3] + 4.0);
    }
    if right_panel && blocked(top, right) {
        let above = tucked;
        if panel[1] >= above + LINE_PX + 4.0 && !blocked(above, right) {
            top = above;
        } else {
            right = panel[0] - 4.0;
            top = margin;
            if blocked(top, right) {
                top = if instruments {
                    instrument[3] + 4.0
                } else {
                    tape[3] + 4.0
                };
            }
        }
    }
    Anchor { right, top }
}

/// Whether the readout is drawn. `TORE_SHOW_FPS=1` forces it on and `0`
/// forces it off, for a test run. Anything else leaves the saved setting.
/// The variable is not written into `graphics-v1.conf`.
pub fn shown(saved: bool) -> bool {
    let variable = std::env::var("TORE_SHOW_FPS").ok();
    shown_with(saved, variable.as_deref())
}

/// The same rule as [`shown`], with the variable's text passed in.
pub fn shown_with(saved: bool, variable: Option<&str>) -> bool {
    match variable {
        Some("1") => true,
        Some("0") => false,
        _ => saved,
    }
}

/// `TORE_TRACE_PRESENT=1` logs every change of what the window presents (menu,
/// loading photograph, flight, replay) with the frames drawn of the one before,
/// to see a screen flash between two others. Development only.
pub fn trace_present(surface: Surface) {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    static LAST: std::sync::Mutex<Option<(&'static str, u32)>> = std::sync::Mutex::new(None);
    if !*ON.get_or_init(|| std::env::var("TORE_TRACE_PRESENT").is_ok_and(|v| v == "1")) {
        return;
    }
    let name = match surface {
        Surface::Menu => "menu",
        Surface::Loading => "loading",
        Surface::Flight { .. } => "flight",
        Surface::Replay { .. } => "replay",
    };
    let mut last = LAST.lock().unwrap_or_else(|e| e.into_inner());
    match &mut *last {
        Some((was, frames)) if *was == name => *frames += 1,
        Some((was, frames)) => {
            log::info!("present: {frames} {was} frames, now {name}");
            *last = Some((name, 1));
        }
        None => *last = Some((name, 1)),
    }
}

/// Glyphs for `text`, right-aligned on `anchor`, with a one-pixel shadow.
/// Empty when `text` is empty: the off state draws nothing.
pub fn quads(text: &str, anchor: Anchor, linear: bool) -> Vec<Quad> {
    if text.is_empty() {
        return Vec::new();
    }
    let tint = |rgb: [u8; 3], alpha: f32| {
        let color = if linear {
            rgb.map(srgb_to_linear)
        } else {
            rgb.map(|c| f32::from(c) / 255.0)
        };
        [color[0], color[1], color[2], alpha]
    };
    let (width, glyphs) =
        ui_text::atlas().line_quads(text, LINE_PX, [0.0, 0.0], tint([236, 242, 255], 1.0));
    let origin = [anchor.right - width, anchor.top];
    let mut out = Vec::with_capacity(glyphs.len() * 2);
    for quad in &glyphs {
        let mut shadow = *quad;
        shadow.color = tint([0, 0, 0], 0.85);
        shift(&mut shadow, origin[0] + 1.0, origin[1] + 1.0);
        out.push(shadow);
    }
    for mut quad in glyphs {
        shift(&mut quad, origin[0], origin[1]);
        out.push(quad);
    }
    out
}

fn shift(quad: &mut Quad, x: f32, y: f32) {
    quad.dst[0] += x;
    quad.dst[1] += y;
    quad.clip[0] += x;
    quad.clip[1] += y;
    quad.clip[2] += x;
    quad.clip[3] += y;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_times_are_measured_separately_and_unavailable_gpu_stays_unknown() {
        let mut m = Meter::default();
        m.push(0.0161);
        m.work(4.2, None);
        assert_eq!(
            m.line().unwrap(),
            "62 fps  16.1 ms  cpu 4.2  gpu --  low 62"
        );
        m.work(4.2, Some(11.8));
        assert_eq!(
            m.line().unwrap(),
            "62 fps  16.1 ms  cpu 4.2  gpu 11.8  low 62"
        );
        m = Meter::default();
        m.push(0.0161);
        assert!(m.line().unwrap().contains("cpu --  gpu --"));
        // Shown, the counter keeps every view drawing, paused flight and
        // idle menus included, through the frame pacing (`App::pace` and
        // the stopped flight's stillness), so the frame rate choice still
        // caps it.
        let main = include_str!("main.rs");
        for (function, needle) in [
            ("fn pace(&mut self", "counted("),
            ("fn flight_want(&mut self", "counter_shown()"),
        ] {
            let body = main.split(function).nth(1).unwrap();
            assert!(
                body.split("\n    }\n").next().unwrap().contains(needle),
                "{function}"
            );
        }
    }

    #[test]
    fn real_continuous_frames_read_as_the_rate_and_a_gap_restarts_the_clock() {
        use std::time::{Duration, Instant};
        for hz in [60u32, 100, 200] {
            let mut meter = Meter::default();
            let start = Instant::now();
            let period = Duration::from_secs_f64(1.0 / f64::from(hz));
            for n in 0..(hz * 2) {
                meter.tick_at(start + period * n);
            }
            let line = meter.line().unwrap();
            assert!(line.starts_with(&format!("{hz} fps")), "{hz}: {line}");
        }
        // A long idle gap (no frames drawn) is not one slow frame.
        let mut meter = Meter::default();
        let start = Instant::now();
        for n in 0..30 {
            meter.tick_at(start + Duration::from_millis(16) * n);
        }
        let resumed = start + Duration::from_secs(30);
        meter.tick_at(resumed);
        assert_eq!(meter.line(), None, "the history restarts after a gap");
        for n in 1..30 {
            meter.tick_at(resumed + Duration::from_millis(10) * n);
        }
        assert!(meter.line().unwrap().starts_with("100 fps"));
    }
    #[test]
    fn the_average_is_half_a_second_and_low_is_the_slowest_frame() {
        let mut meter = Meter::default();
        assert_eq!(meter.line(), None);
        // Thirty frames at 16.1 ms: 62 fps, and the same figure as the low.
        for _ in 0..30 {
            meter.push(0.0161);
        }
        assert_eq!(
            meter.line().as_deref(),
            Some("62 fps  16.1 ms  cpu --  gpu --  low 62")
        );
        // One slow frame in the last second. The half-second mean moves a
        // little; the low follows the slow frame.
        meter.push(1.0 / 48.0);
        let line = meter.line().unwrap();
        assert!(line.ends_with("low 48"), "{line}");
        assert!(line.contains("fps  "));
        // Slow renders count, even when longer than the averaging window.
        meter.push(2.0);
        assert!(meter.line().unwrap().contains("2000.0 ms"));
        meter = Meter::default();
        assert_eq!(meter.line(), None);
        meter.push(0.0161);
        assert_eq!(
            meter.line().as_deref(),
            Some("62 fps  16.1 ms  cpu --  gpu --  low 62")
        );
    }

    #[test]
    fn the_env_override_wins_and_is_only_zero_or_one() {
        assert!(!shown_with(false, None));
        assert!(shown_with(true, None));
        assert!(shown_with(false, Some("1")));
        assert!(!shown_with(true, Some("0")));
        assert!(!shown_with(false, Some("yes")));
        assert!(shown_with(true, Some("on")));
    }

    #[test]
    fn an_empty_line_draws_nothing() {
        let anchor = anchor([1920, 1080], Surface::Menu);
        assert!(quads("", anchor, false).is_empty());
    }

    #[test]
    fn the_line_is_right_aligned_in_the_bundled_face() {
        let anchor = Anchor {
            right: 400.0,
            top: 12.0,
        };
        let quads = quads("62 fps  16.1 ms   low 48", anchor, false);
        assert!(quads.len() > 4);
        let right = quads
            .iter()
            .map(|q| q.dst[0] + q.dst[2])
            .fold(0.0, f32::max);
        // The shadow sits one pixel past the glyphs.
        assert!((right - 401.0).abs() < 3.0, "{right}");
        let top = quads.iter().map(|q| q.dst[1]).fold(f32::MAX, f32::min);
        assert!((10.0..=14.0).contains(&top), "{top}");
    }

    #[test]
    fn each_screen_keeps_the_corner_clear_of_the_chrome() {
        // A wide menu: the black bar to the right of the canvas.
        let menu = anchor([1920, 1080], Surface::Menu);
        assert_eq!(
            menu,
            Anchor {
                right: 1910.0,
                top: 10.0
            }
        );
        // A 4:3 menu: under the title strip.
        let square = anchor([640, 480], Surface::Menu);
        assert_eq!(
            square,
            Anchor {
                right: 632.0,
                top: 22.0
            }
        );
        // Loading words are centred, so the corner is free.
        let loading = anchor([1920, 1080], Surface::Loading);
        assert_eq!(
            loading,
            Anchor {
                right: 1910.0,
                top: 10.0
            }
        );
        // 1080p flight: the line fits above the large instrument window.
        let flight = anchor([1920, 1080], Surface::Flight { right_panel: false });
        assert_eq!(
            flight,
            Anchor {
                right: 1910.0,
                top: 2.0
            }
        );
        // 720p flight: the margin is too thin, so the line sits under the window.
        let low = anchor([1280, 720], Surface::Flight { right_panel: false });
        assert_eq!(
            low,
            Anchor {
                right: 1270.0,
                top: 256.0
            }
        );
        // The same window with the right debug panel open: left of that panel.
        let panel = anchor([1280, 720], Surface::Flight { right_panel: true });
        assert_eq!(
            panel,
            Anchor {
                right: 886.0,
                top: 10.0
            }
        );
        // Replay: the timer is not in this corner, and the tape counter is lower.
        let replay = anchor(
            [1920, 1080],
            Surface::Replay {
                right_panel: false,
                hud_camera: true,
            },
        );
        assert_eq!(
            replay,
            Anchor {
                right: 1910.0,
                top: 10.0
            }
        );
        // A right-hand panel starts below the corner, so the corner stays.
        let replay_panel = anchor(
            [1920, 1080],
            Surface::Replay {
                right_panel: true,
                hud_camera: false,
            },
        );
        assert_eq!(
            replay_panel,
            Anchor {
                right: 1910.0,
                top: 10.0
            }
        );
    }
}
