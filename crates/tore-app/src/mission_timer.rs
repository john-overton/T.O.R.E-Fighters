//! Mission countdown presentation, specified in docs/spec/hud-layout.md.
use crate::flight_canvas::{FlightCanvas, HUD_SCALE};
use crate::{hud::Paint, menu::Canvas};
use tore_formats::font::Font;
use tore_session::Client;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Timer {
    pub remaining: u32,
    pub hours: bool,
}

impl Timer {
    pub fn from_client(client: &Client) -> Option<Self> {
        Some(Self {
            remaining: client.seconds_left()?,
            hours: client.time_limit_seconds()? >= 3600,
        })
    }

    pub fn text(self) -> String {
        let s = self.remaining;
        let clock = if self.hours {
            format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
        } else {
            format!("{:02}:{:02}", s / 60, s % 60)
        };
        format!("Mission Timer: {clock}")
    }

    pub fn corner(self) -> bool {
        self.remaining <= 300
    }

    /// The status strip above the Escape menu buttons, clear of the help text.
    pub fn draw_menu(self, pixels: &mut [u8], font: &Font) {
        Canvas(pixels).rect((138, 432, 364, 14), [160, 172, 186, 255]);
        Paint {
            pixels,
            clip: (138, 432, 364, 14),
            color: [15, 30, 50, 255],
        }
        .text(font, &self.text(), 146, 434);
    }

    /// Obstacles are screen rectangles (left, top, width, height).
    pub fn draw_corner(self, canvas: &mut FlightCanvas, font: &Font, obstacles: &[[f64; 4]]) {
        if !self.corner() {
            return;
        }
        let [w, h] = canvas.size.map(f64::from);
        let layer = (w / 640.).min(h / 480.);
        let scale = layer * HUD_SCALE;
        let text = self.text();
        let width = FlightCanvas::text_width(font, &text, scale);
        let height = (font.height + 1) as f64 * scale;
        let [x, y] = position([w, h], [width, height], obstacles);
        let pad = 2. * layer;
        canvas.blend_rect(
            (
                (x - pad).floor() as i32,
                (y - pad).floor() as i32,
                (x + width + pad).ceil() as i32,
                (y + height + pad).ceil() as i32,
            ),
            [0, 0, 0],
            0.65,
        );
        // FlightCanvas::text accumulates glyph coverage on a clear layer.
        // Composite that layer over the backing so antialiased edges survive.
        let mut label = FlightCanvas::default();
        label.blank([
            (width + scale).ceil() as u32,
            (height + scale).ceil() as u32,
        ]);
        label.text(
            font,
            &text,
            [0., 0.],
            scale,
            if self.remaining <= 60 {
                [255, 200, 80]
            } else {
                [240, 245, 250]
            },
        );
        for (i, pixel) in label.pixels.chunks_exact(4).enumerate() {
            if pixel[3] != 0 {
                canvas.blend(
                    x.round() as i32 + (i % label.size[0] as usize) as i32,
                    y.round() as i32 + (i / label.size[0] as usize) as i32,
                    [pixel[0], pixel[1], pixel[2]],
                    f64::from(pixel[3]) / 255.,
                );
            }
        }
    }
}

fn position([w, h]: [f64; 2], [width, height]: [f64; 2], obstacles: &[[f64; 4]]) -> [f64; 2] {
    let layer = (w / 640.).min(h / 480.);
    let gap = 4. * layer;
    let rightmost = (w - 8. * layer - width).max(gap);
    let mut columns = vec![rightmost];
    columns.extend(
        obstacles
            .iter()
            .map(|r| r[0] - width - gap)
            .filter(|x| *x >= gap && *x < rightmost),
    );
    columns.sort_by(|a, b| b.total_cmp(a));
    for x in columns {
        let mut y = 8. * layer;
        // Each pass clears at least one rectangle, including unsorted blockers.
        for _ in 0..=obstacles.len() {
            let next = obstacles
                .iter()
                .filter(|&&[left, top, bw, bh]| {
                    x < left + bw + gap
                        && x + width + gap > left
                        && y < top + bh + gap
                        && y + height + gap > top
                })
                .map(|r| r[1] + r[3] + gap)
                .fold(y, f64::max);
            if next == y {
                break;
            }
            y = next;
        }
        if y + height + gap <= h {
            return [x, y];
        }
    }
    // A full-screen panel can leave no free area. Keep the deadline readable.
    [rightmost, 8. * layer]
}

/// Reserve the optional frame-rate readout even when switched off, so the
/// mission countdown does not jump when that preference changes.
pub fn counter_rect(size: [u32; 2], surface: crate::frame_rate::Surface) -> [f64; 4] {
    let a = crate::frame_rate::anchor(size, surface);
    [f64::from(a.right) - 420., f64::from(a.top), 420., 20.]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn font(height: usize) -> Font {
        Font {
            height,
            glyphs: (0..256)
                .map(|_| tore_formats::font::Glyph {
                    advance: 1,
                    pixels: vec![(0, 0)],
                })
                .collect(),
        }
    }

    #[test]
    fn small_glyphs_stay_visible_over_the_backing_and_menu_buttons_stay_clear() {
        let mut canvas = FlightCanvas::default();
        canvas.blank([640, 480]);
        Timer {
            remaining: 301,
            hours: false,
        }
        .draw_corner(&mut canvas, &font(1), &[]);
        assert!(canvas.pixels.iter().all(|&v| v == 0));
        Timer {
            remaining: 300,
            hours: false,
        }
        .draw_corner(&mut canvas, &font(1), &[]);
        assert!(
            canvas.pixels.chunks_exact(4).any(|p| p[0] > 30 && p[3] > 0),
            "subpixel glyphs must not disappear into the backing's alpha"
        );
        let mut menu = vec![99; 640 * 480 * 4];
        Timer {
            remaining: 350,
            hours: false,
        }
        .draw_menu(&mut menu, &font(10));
        assert!(menu[448 * 640 * 4..].iter().all(|&v| v == 99));
    }

    #[test]
    fn a_tall_watch_panel_moves_the_timer_left_inside_the_screen() {
        let blocks = [[380., 20., 254., 460.], [220., 2., 420., 20.]];
        let [x, y] = position([640., 480.], [140., 9.], &blocks);
        assert!(x + 140. <= 380. && y + 9. <= 480.);
    }

    #[test]
    fn formatting_and_five_minute_boundary() {
        for (remaining, hours, text, visible) in [
            (301, false, "05:01", false),
            (300, false, "05:00", true),
            (0, false, "00:00", true),
            (3600, true, "1:00:00", false),
            (3599, true, "0:59:59", false),
            (0, true, "0:00:00", true),
            (604800, true, "168:00:00", false),
        ] {
            let timer = Timer { remaining, hours };
            assert_eq!(timer.text(), format!("Mission Timer: {text}"));
            assert_eq!(timer.corner(), visible);
        }
    }

    #[test]
    fn corner_clears_instruments_and_counter_at_every_size() {
        for size in [
            [320, 240],
            [640, 480],
            [1280, 720],
            [1920, 1080],
            [3840, 2160],
        ] {
            let [w, h] = size.map(f64::from);
            let layer = (w / 640.).min(h / 480.);
            let blocks = [
                [w - 170. * layer, 8. * layer, 162. * layer, 160. * layer],
                counter_rect(
                    size,
                    crate::frame_rate::Surface::Flight { right_panel: false },
                ),
            ];
            let [tw, th] = [140. * layer, 9. * layer];
            let [x, y] = position([w, h], [tw, th], &blocks);
            assert!(x >= 0. && y >= 0. && x + tw <= w && y + th <= h);
            for [bx, by, bw, bh] in blocks {
                assert!(x + tw <= bx || x >= bx + bw || y + th <= by || y >= by + bh);
            }
        }
    }
}
