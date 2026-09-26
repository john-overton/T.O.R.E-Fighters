//! The replay viewer's interface, drawn in the 640x480 overlay layer with
//! the Controls screen's colours, font and button style: the transport bar
//! with its timeline along the bottom, the mission timer and the viewer's
//! own notices along the top, and subtitles above the cockpit messages that
//! the viewer prints over the bar as flight prints them. The debug panels
//! and the right-click menu draw in a layer of their own (`panels.rs`,
//! `context_menu.rs`). The
//! layer's top half is pinned to the top of the view and its bottom half to
//! the bottom, so the bar sits on the bottom edge of any window shape.
//! Layout and wording are agent design (2026-09-26).
use crate::controls_editor::{
    Editor, FOCUS, INK, PALE, Rect, TITLE, WHITE, fit, inside, text_width,
};
use crate::menu::Canvas;
use crate::replay::clock::Clock;
use tore_formats::font::Font;

pub const WIDTH: usize = 640;
pub const HEIGHT: usize = 480;

/// Everything on the bar a click can reach.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Control {
    Start,
    StepBack,
    Reverse,
    Pause,
    Play,
    FastForward,
    StepForward,
    End,
    Timeline,
    Camera,
    Aircraft,
    HideUi,
}

/// The transport buttons, left to right.
pub const TRANSPORT: [Control; 8] = [
    Control::Start,
    Control::StepBack,
    Control::Reverse,
    Control::Pause,
    Control::Play,
    Control::FastForward,
    Control::StepForward,
    Control::End,
];

/// The top of the transport bar, which runs to the layer's bottom.
pub const BAR_TOP: i32 = 430;
/// The bar's background.
const BAR: Rect = (0, BAR_TOP, 640, HEIGHT as i32 - BAR_TOP);
const BAR_FILL: [u8; 4] = [16, 24, 34, 215];
/// Where a click or drag scrubs.
const TIMELINE: Rect = (8, 432, 624, 20);
/// The drawn track inside it.
const TRACK: Rect = (8, 440, 624, 4);
const ROW: i32 = 455;
const BUTTON_WIDTH: i32 = 22;
const CAMERA: Rect = (388, ROW, 92, 20);
const AIRCRAFT: Rect = (484, ROW, 104, 20);
const HIDE: Rect = (592, ROW, 40, 20);
/// The speed readout and the time, between the buttons and the camera.
const SPEED_X: i32 = 204;
const TIME_X: i32 = 286;

/// Where `control` sits on the bar.
pub fn rect(control: Control) -> Rect {
    match control {
        Control::Timeline => TIMELINE,
        Control::Camera => CAMERA,
        Control::Aircraft => AIRCRAFT,
        Control::HideUi => HIDE,
        _ => {
            let index = TRANSPORT.iter().position(|c| *c == control).unwrap_or(0) as i32;
            (8 + index * (BUTTON_WIDTH + 2), ROW, BUTTON_WIDTH, 20)
        }
    }
}

/// The control under a point of the layer.
pub fn hit(point: (f64, f64)) -> Option<Control> {
    TRANSPORT
        .into_iter()
        .chain([
            Control::Timeline,
            Control::Camera,
            Control::Aircraft,
            Control::HideUi,
        ])
        .find(|control| inside(point, rect(*control)))
}

/// The tick a timeline position points at, clamped to the recording.
pub fn timeline_tick(x: f64, first: u64, last: u64) -> f64 {
    let (left, width) = (f64::from(TRACK.0), f64::from(TRACK.2));
    let fraction = ((x - left) / width).clamp(0., 1.);
    first as f64 + fraction * (last - first) as f64
}

/// Where a tick sits along the timeline.
pub fn timeline_x(tick: f64, first: u64, last: u64) -> f64 {
    let span = (last - first).max(1) as f64;
    f64::from(TRACK.0) + ((tick - first as f64) / span).clamp(0., 1.) * f64::from(TRACK.2)
}

/// The pointer on the bar: what it is over, what the left button pressed,
/// and whether it is dragging the playhead along the timeline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Pointer {
    pub hover: Option<Control>,
    pub pressed: Option<Control>,
    pub scrubbing: bool,
}

impl Pointer {
    /// The pointer moved to layer point `at` (`None` when the layer is not
    /// under it); dragging the timeline moves the playhead with it.
    pub fn moved(&mut self, at: Option<(f64, f64)>, clock: &mut Clock) {
        self.hover = at.and_then(hit);
        if self.scrubbing
            && let Some((x, _)) = at
        {
            clock.seek(timeline_tick(x, clock.first(), clock.last()));
        }
    }

    /// The left button went down at `at`: a press on a control, and on the
    /// timeline the playhead jumps there and follows the pointer.
    pub fn down(&mut self, at: Option<(f64, f64)>, clock: &mut Clock) {
        self.pressed = at.and_then(hit);
        if self.pressed == Some(Control::Timeline)
            && let Some((x, _)) = at
        {
            self.scrubbing = true;
            clock.seek(timeline_tick(x, clock.first(), clock.last()));
        }
    }

    /// The left button came up at `at`: the control clicked, when the press
    /// began and ended on it. A timeline drag clicks nothing.
    pub fn up(&mut self, at: Option<(f64, f64)>) -> Option<Control> {
        let pressed = self.pressed.take();
        if std::mem::take(&mut self.scrubbing) {
            return None;
        }
        pressed.filter(|control| at.and_then(hit) == Some(*control))
    }
}

/// What a transport button does to the playback clock. False for the
/// controls that are not transport buttons.
pub fn transport(control: Control, clock: &mut Clock) -> bool {
    match control {
        Control::Start => clock.start(),
        Control::StepBack => clock.step(-1),
        Control::Reverse => clock.reverse(),
        Control::Pause => clock.pause(),
        Control::Play => clock.play(),
        Control::FastForward => clock.fast_forward(),
        Control::StepForward => clock.step(1),
        Control::End => clock.end(),
        Control::Timeline | Control::Camera | Control::Aircraft | Control::HideUi => return false,
    }
    true
}

/// How the 640x480 layer sits on a view of `size` pixels.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement {
    scale: f64,
    left: f64,
    /// Where the layer's bottom half starts, in view pixels.
    bottom: f64,
}

impl Placement {
    pub fn new(size: [u32; 2]) -> Self {
        let [w, h] = size.map(f64::from);
        let scale = (w / WIDTH as f64).min(h / HEIGHT as f64).max(1e-6);
        Self {
            scale,
            left: (w - WIDTH as f64 * scale) / 2.,
            bottom: h - HEIGHT as f64 / 2. * scale,
        }
    }

    /// One layer pixel in view pixels.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// The point of the debug panels' layer under a view point. That layer
    /// is centred on the view whole (`FlightCanvas::centered_rects`), so on
    /// a view taller than 4:3 it differs from [`Placement::layer`]. Points
    /// outside the layer come back outside 640x480.
    pub fn centered(&self, [x, y]: [f64; 2]) -> (f64, f64) {
        let top = (self.bottom - HEIGHT as f64 / 2. * self.scale) / 2.;
        ((x - self.left) / self.scale, (y - top) / self.scale)
    }

    /// The layer point under a view point, if the layer covers it.
    pub fn layer(&self, [x, y]: [f64; 2]) -> Option<(f64, f64)> {
        let lx = (x - self.left) / self.scale;
        let half = HEIGHT as f64 / 2.;
        let ly = if y >= self.bottom {
            half + (y - self.bottom) / self.scale
        } else if y < half * self.scale {
            y / self.scale
        } else {
            return None;
        };
        ((0. ..WIDTH as f64).contains(&lx) && (0. ..HEIGHT as f64).contains(&ly))
            .then_some((lx, ly))
    }
}

/// A timeline marker's kind, which sets its colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkerKind {
    Launch,
    Kill,
    Order,
    Bookmark,
}

impl MarkerKind {
    fn color(self) -> [u8; 4] {
        match self {
            Self::Launch => [255, 206, 84, 255],
            Self::Kill => [240, 82, 70, 255],
            Self::Order => [96, 176, 255, 255],
            Self::Bookmark => [120, 214, 140, 255],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Marker {
    pub tick: u64,
    pub kind: MarkerKind,
}

/// Everything the interface shows for one frame, as plain data.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    pub first: u64,
    pub last: u64,
    pub position: f64,
    pub markers: Vec<Marker>,
    /// The lit transport button.
    pub active: Option<Control>,
    pub pressed: Option<Control>,
    pub hover: Option<Control>,
    pub speed: String,
    pub time: String,
    pub camera: String,
    pub aircraft: String,
    pub timer: Option<String>,
    pub toast: Option<String>,
    pub subtitles: Vec<String>,
    /// How far above their usual place the subtitles sit, clear of the
    /// cockpit messages over the bar.
    pub subtitle_lift: i32,
}

fn text(pixels: &mut [u8], font: &Font, color: [u8; 4], text: &str, at: (i32, i32)) {
    Editor::text(
        pixels,
        font,
        (0, 0, WIDTH as i32, HEIGHT as i32),
        color,
        text,
        at,
    );
}

/// A rectangle holding text, sized to it.
fn caption(pixels: &mut [u8], font: &Font, line: &str, center: i32, y: i32, color: [u8; 4]) {
    let width = text_width(font, line);
    let x = center - width / 2;
    Canvas(pixels).rect(
        (x - 6, y - 3, width + 12, font.height as i32 + 6),
        [0, 0, 0, 160],
    );
    text(pixels, font, color, line, (x, y));
}

/// A filled triangle `w` wide and `h` tall pointing right or left.
fn triangle(pixels: &mut [u8], (x, y, w, h): Rect, right: bool, color: [u8; 4]) {
    for row in 0..h {
        let reach = 1. - ((2 * row + 1 - h) as f64 / h as f64).abs();
        let width = (f64::from(w) * reach).round() as i32;
        let left = if right { x } else { x + w - width };
        Canvas(pixels).rect((left, y + row, width, 1), color);
    }
}

/// The icon for a transport button, centred in `r`.
fn icon(pixels: &mut [u8], control: Control, (x, y, w, h): Rect, color: [u8; 4]) {
    let (cx, cy) = (x + w / 2, y + h / 2);
    let bar = |pixels: &mut [u8], at: i32| Canvas(pixels).rect((at, cy - 5, 2, 10), color);
    match control {
        Control::Start => {
            bar(pixels, cx - 6);
            triangle(pixels, (cx - 4, cy - 5, 5, 10), false, color);
            triangle(pixels, (cx + 1, cy - 5, 5, 10), false, color);
        }
        Control::StepBack => {
            triangle(pixels, (cx - 5, cy - 5, 6, 10), false, color);
            bar(pixels, cx + 3);
        }
        Control::Reverse => triangle(pixels, (cx - 4, cy - 5, 8, 10), false, color),
        Control::Pause => {
            bar(pixels, cx - 3);
            bar(pixels, cx + 1);
        }
        Control::Play => triangle(pixels, (cx - 3, cy - 5, 8, 10), true, color),
        Control::FastForward => {
            triangle(pixels, (cx - 6, cy - 5, 6, 10), true, color);
            triangle(pixels, (cx, cy - 5, 6, 10), true, color);
        }
        Control::StepForward => {
            bar(pixels, cx - 5);
            triangle(pixels, (cx - 1, cy - 5, 6, 10), true, color);
        }
        Control::End => {
            triangle(pixels, (cx - 6, cy - 5, 5, 10), true, color);
            triangle(pixels, (cx - 1, cy - 5, 5, 10), true, color);
            bar(pixels, cx + 4);
        }
        _ => {}
    }
}

/// A bar button in the Controls screen's footer style: pale, or the focus
/// colour when lit, pressed or under the pointer.
fn button(pixels: &mut [u8], r: Rect, lit: bool) -> [u8; 4] {
    Canvas(pixels).rect(r, if lit { FOCUS } else { PALE });
    if lit { WHITE } else { INK }
}

fn labelled(pixels: &mut [u8], font: &Font, r: Rect, label: &str, lit: bool) {
    let color = button(pixels, r, lit);
    let label = fit(font, label, r.2 - 6);
    let x = r.0 + (r.2 - text_width(font, &label)) / 2;
    Editor::text(pixels, font, r, color, &label, (x, r.1 + 4));
}

/// Draws the interface into a cleared 640x480 layer.
pub fn draw(pixels: &mut [u8], font: &Font, model: &Model) {
    let lit = |control| {
        model.active == Some(control)
            || model.pressed == Some(control)
            || model.hover == Some(control)
    };
    Canvas(pixels).rect(BAR, BAR_FILL);
    // Timeline: the played part, markers, then the playhead.
    Canvas(pixels).rect(TRACK, [70, 84, 104, 255]);
    let head = timeline_x(model.position, model.first, model.last).round() as i32;
    Canvas(pixels).rect((TRACK.0, TRACK.1, head - TRACK.0, TRACK.3), TITLE);
    for marker in &model.markers {
        let x = timeline_x(marker.tick as f64, model.first, model.last).round() as i32;
        Canvas(pixels).rect((x, TRACK.1 - 5, 2, TRACK.3 + 10), marker.kind.color());
    }
    Canvas(pixels).rect((head - 1, TRACK.1 - 7, 3, TRACK.3 + 14), WHITE);
    for control in TRANSPORT {
        let r = rect(control);
        let color = button(pixels, r, lit(control));
        icon(pixels, control, r, color);
    }
    text(
        pixels,
        font,
        TITLE,
        &fit(font, &model.speed, TIME_X - SPEED_X - 4),
        (SPEED_X, ROW + 4),
    );
    text(
        pixels,
        font,
        PALE,
        &fit(font, &model.time, CAMERA.0 - TIME_X - 4),
        (TIME_X, ROW + 4),
    );
    labelled(pixels, font, CAMERA, &model.camera, lit(Control::Camera));
    labelled(
        pixels,
        font,
        AIRCRAFT,
        &model.aircraft,
        lit(Control::Aircraft),
    );
    labelled(pixels, font, HIDE, "Hide", lit(Control::HideUi));
    // Subtitles stack upwards from just above the bar and the cockpit
    // messages over it, newest lowest.
    let step = font.height as i32 + 10;
    for (i, line) in model.subtitles.iter().rev().enumerate() {
        let line = fit(font, line, 600);
        let y = 408 - model.subtitle_lift - i as i32 * step;
        caption(pixels, font, &line, 320, y, WHITE);
    }
    if let Some(timer) = &model.timer {
        Canvas(pixels).rect(
            (6, 6, text_width(font, timer) + 12, font.height as i32 + 8),
            [0, 0, 0, 170],
        );
        text(pixels, font, TITLE, timer, (12, 10));
    }
    if let Some(toast) = &model.toast {
        caption(pixels, font, &fit(font, toast, 420), 320, 12, TITLE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font() -> Font {
        Font {
            height: 7,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 5,
                    pixels: vec![(0, 0), (1, 1), (2, 2), (3, 3)],
                })
                .collect(),
        }
    }

    #[test]
    fn every_control_is_hit_where_it_is_drawn_and_nowhere_else() {
        let controls = TRANSPORT.into_iter().chain([
            Control::Timeline,
            Control::Camera,
            Control::Aircraft,
            Control::HideUi,
        ]);
        for control in controls {
            let (x, y, w, h) = rect(control);
            let centre = (
                f64::from(x) + f64::from(w) / 2.,
                f64::from(y) + f64::from(h) / 2.,
            );
            assert_eq!(hit(centre), Some(control), "{control:?}");
            // Controls never overlap and stay on the bar.
            assert!(y >= BAR.1 && y + h <= 480 && x >= 0 && x + w <= 640);
        }
        for a in TRANSPORT {
            for b in [
                Control::Camera,
                Control::Aircraft,
                Control::HideUi,
                Control::Timeline,
            ] {
                let (ra, rb) = (rect(a), rect(b));
                let apart = ra.0 + ra.2 <= rb.0
                    || rb.0 + rb.2 <= ra.0
                    || ra.1 + ra.3 <= rb.1
                    || rb.1 + rb.3 <= ra.1;
                assert!(apart, "{a:?} overlaps {b:?}");
            }
        }
        assert_eq!(hit((320., 200.)), None);
        assert_eq!(hit((250., ROW as f64 + 10.)), None);
    }

    #[test]
    fn the_timeline_maps_both_ways_and_clamps() {
        let (first, last) = (100, 10_100);
        assert_eq!(timeline_tick(8., first, last), 100.);
        assert_eq!(timeline_tick(632., first, last), 10_100.);
        assert_eq!(timeline_tick(-50., first, last), 100.);
        assert_eq!(timeline_tick(900., first, last), 10_100.);
        for tick in [100., 2_500., 5_100., 10_100.] {
            let x = timeline_x(tick, first, last);
            assert!((timeline_tick(x, first, last) - tick).abs() < 1e-9);
        }
        assert_eq!(timeline_x(5_100., first, last), 320.);
        // A one-tick recording does not divide by zero.
        assert_eq!(timeline_x(5., 5, 5), 8.);
        assert_eq!(timeline_tick(300., 5, 5), 5.);
    }

    fn centre(control: Control) -> (f64, f64) {
        let (x, y, w, h) = rect(control);
        (
            f64::from(x) + f64::from(w) / 2.,
            f64::from(y) + f64::from(h) / 2.,
        )
    }

    #[test]
    fn dragging_along_the_timeline_scrubs_and_clicks_nothing() {
        let mut clock = Clock::new(0, 12_000);
        clock.pause();
        let mut pointer = Pointer::default();
        let y = centre(Control::Timeline).1;
        pointer.down(Some((320., y)), &mut clock);
        assert!(pointer.scrubbing);
        assert_eq!(clock.position(), 6_000.);
        // The drag keeps scrubbing even off the timeline's row.
        pointer.moved(Some((164., 200.)), &mut clock);
        assert_eq!(clock.position(), 3_000.);
        pointer.moved(Some((700., y)), &mut clock);
        assert_eq!(clock.position(), 12_000.);
        // Off the layer the playhead stays where it was.
        pointer.moved(None, &mut clock);
        assert_eq!(clock.position(), 12_000.);
        assert_eq!(pointer.up(Some((8., y))), None);
        assert!(!pointer.scrubbing);
        pointer.moved(Some((8., y)), &mut clock);
        assert_eq!(clock.position(), 12_000.);
        assert!(clock.paused());
    }

    #[test]
    fn a_click_needs_the_press_and_release_on_the_same_control() {
        let mut clock = Clock::new(0, 12_000);
        clock.pause();
        let mut pointer = Pointer::default();
        pointer.moved(Some(centre(Control::Play)), &mut clock);
        assert_eq!(pointer.hover, Some(Control::Play));
        pointer.down(Some(centre(Control::Play)), &mut clock);
        assert_eq!(pointer.up(Some(centre(Control::Pause))), None);
        pointer.down(Some(centre(Control::Play)), &mut clock);
        let clicked = pointer.up(Some(centre(Control::Play))).unwrap();
        assert!(transport(clicked, &mut clock));
        assert!(!clock.paused());
        // Every transport button reaches the clock.
        for (control, check) in [
            (Control::Pause, (0., Some(true))),
            (Control::End, (12_000., None)),
            (Control::StepBack, (11_999., Some(true))),
            (Control::Start, (0., None)),
            (Control::StepForward, (1., Some(true))),
        ] {
            pointer.down(Some(centre(control)), &mut clock);
            let clicked = pointer.up(Some(centre(control))).unwrap();
            assert!(transport(clicked, &mut clock));
            if control != Control::Pause {
                assert_eq!(clock.position(), check.0, "{control:?}");
            }
            if let Some(paused) = check.1 {
                assert_eq!(clock.paused(), paused, "{control:?}");
            }
        }
        for (control, speed) in [
            (Control::FastForward, 2.),
            (Control::FastForward, 4.),
            (Control::Reverse, 1.),
        ] {
            assert!(transport(control, &mut clock));
            assert_eq!(clock.speed(), speed);
        }
        assert!(!transport(Control::Camera, &mut clock));
        // A press that starts off the bar clicks nothing.
        pointer.down(Some((320., 100.)), &mut clock);
        assert_eq!(pointer.up(Some(centre(Control::Play))), None);
    }

    #[test]
    fn the_layer_pins_its_halves_to_the_top_and_bottom_edges() {
        // 4:3: the plain letterbox-free scale.
        let p = Placement::new([1280, 960]);
        assert_eq!(p.scale(), 2.);
        assert_eq!(p.layer([640., 960. - 1.]), Some((320., 479.5)));
        assert_eq!(p.layer([0., 0.]), Some((0., 0.)));
        // Wide: centred across with the full height.
        let p = Placement::new([1920, 1080]);
        assert_eq!(p.scale(), 2.25);
        assert_eq!(p.layer([240., 0.]), Some((0., 0.)));
        assert_eq!(p.layer([100., 500.]), None);
        let bar = p.layer([960., 1080. - 2.25 * 10.]).unwrap();
        assert!((bar.0 - 320.).abs() < 1e-9 && (bar.1 - 470.).abs() < 1e-9);
        // Tall: the bottom half hugs the bottom edge.
        let p = Placement::new([640, 1000]);
        assert_eq!(p.scale(), 1.);
        assert_eq!(p.layer([10., 990.]), Some((10., 470.)));
        assert_eq!(p.layer([10., 100.]), Some((10., 100.)));
        assert_eq!(p.layer([10., 500.]), None);
    }

    #[test]
    fn the_panel_layer_is_centred_whole() {
        // Wide and 4:3 views: the same point as the anchored layer.
        for size in [[1280, 960], [1920, 1080]] {
            let p = Placement::new(size);
            for view in [[300., 20.], [960., 700.], [1270., 950.]] {
                if let Some(layer) = p.layer(view) {
                    let centred = p.centered(view);
                    assert!(
                        (centred.0 - layer.0).abs() < 1e-9 && (centred.1 - layer.1).abs() < 1e-9
                    );
                }
            }
        }
        // Tall: centred up and down, and points in the bands come back
        // outside the layer.
        let p = Placement::new([640, 1000]);
        assert_eq!(p.centered([10., 260.]), (10., 0.));
        assert_eq!(p.centered([10., 500.]), (10., 240.));
        assert_eq!(p.centered([10., 100.]), (10., -160.));
        let wide = Placement::new([1920, 1080]);
        assert_eq!(wide.centered([0., 0.]), (-240. / 2.25, 0.));
    }

    #[test]
    fn the_interface_draws_its_parts() {
        let model = Model {
            first: 0,
            last: 12_000,
            position: 6_000.,
            markers: vec![
                Marker {
                    tick: 3_000,
                    kind: MarkerKind::Kill,
                },
                Marker {
                    tick: 9_000,
                    kind: MarkerKind::Bookmark,
                },
            ],
            active: Some(Control::Play),
            speed: "1x".into(),
            time: "00:50.0 / 01:40.0".into(),
            camera: "F10 External".into(),
            aircraft: "You F/A-18D".into(),
            timer: Some("00:50.0  tick 6,000".into()),
            toast: Some("Saved".into()),
            subtitles: vec!["Enemy 1-1: 'Fox two'".into()],
            ..Default::default()
        };
        let mut pixels = vec![0; WIDTH * HEIGHT * 4];
        draw(&mut pixels, &font(), &model);
        let at = |x: usize, y: usize| &pixels[(y * WIDTH + x) * 4..][..4];
        // The played half of the track, the unplayed half, the markers and
        // the lit play button.
        assert_eq!(at(100, 441), TITLE);
        assert_eq!(at(500, 441), [70, 84, 104, 255]);
        let kill = timeline_x(3_000., 0, 12_000) as usize;
        assert_eq!(at(kill, 437), MarkerKind::Kill.color());
        let play = rect(Control::Play);
        assert_eq!(at(play.0 as usize + 1, play.1 as usize + 1), FOCUS);
        let pause = rect(Control::Pause);
        assert_eq!(at(pause.0 as usize + 1, pause.1 as usize + 1), PALE);
        // Above the bar, only the timer, messages and subtitles cover the
        // view.
        assert_eq!(at(620, 300), [0; 4]);
        assert_ne!(at(8, 8), [0; 4]);
        assert_eq!(at(100, 40), [0; 4]);
        assert_eq!(at(100, 100), [0; 4]);
    }
}
