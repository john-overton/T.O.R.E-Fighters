//! The bolted panel and the composed background.
use super::{Kit, Rect, draw::blit_part};
use crate::menu::{Canvas, HEIGHT, WIDTH};

/// The generic retail panel at any rectangle (EF0, `docs/formats/menu.md`
/// "The generic panel"): the grey `PANEL` fill, `EDGETB` and `EDGELR` tiled
/// along the frame's two pixel edges, then the four corners, of which
/// `EDGETL` and `EDGEBR` carry the bolt heads. Sized panels in the dialogs
/// are 295 to 620 wide, always inside `PANEL`'s 640 by 480; a bigger one tiles
/// the fill (*agent decision*: retail's behaviour is unknown, U).
pub fn draw_panel(canvas: &mut Canvas, kit: &Kit, (x, y, w, h): Rect) {
    // Text recorded to be drawn sharp stays on top of the canvas, so it must
    // be told what this panel covers.
    crate::ui_text::occlude((x, y, w, h));
    let clip = (x, y, w, h);
    let fill = kit.sprite("PANEL");
    let mut ty = 0;
    while ty < h {
        let mut tx = 0;
        while tx < w {
            blit_part(
                canvas,
                fill,
                (x + tx, y + ty),
                (0, 0, fill.width, fill.height),
                clip,
                1.0,
            );
            tx += fill.width as i32;
        }
        ty += fill.height as i32;
    }
    let (tl, tr, bl, br) = (
        kit.sprite("EDGETL"),
        kit.sprite("EDGETR"),
        kit.sprite("EDGEBL"),
        kit.sprite("EDGEBR"),
    );
    let (tb, lr) = (kit.sprite("EDGETB"), kit.sprite("EDGELR"));
    // Edges first: the top two rows of `EDGETB` along the top, its bottom two
    // along the bottom; the left two columns of `EDGELR` down the left and
    // its right two down the right, in 25 pixel steps.
    let mut ex = x + 31;
    while ex < x + w - 31 {
        let cut = (tb.width as i32).min(x + w - 31 - ex) as usize;
        blit_part(canvas, tb, (ex, y), (0, 0, cut, 2), clip, 1.0);
        blit_part(canvas, tb, (ex, y + h - 2), (0, 2, cut, 2), clip, 1.0);
        ex += tb.width as i32;
    }
    let mut ey = y + 35;
    while ey < y + h - 34 {
        let cut = (lr.height as i32).min(y + h - 34 - ey) as usize;
        blit_part(canvas, lr, (x, ey), (0, 0, 2, cut), clip, 1.0);
        blit_part(canvas, lr, (x + w - 2, ey), (2, 0, 2, cut), clip, 1.0);
        ey += 25;
    }
    for (corner, at) in [
        (tl, (x, y)),
        (tr, (x + w - 30, y)),
        (bl, (x, y + h - 35)),
        (br, (x + w - 30, y + h - 35)),
    ] {
        blit_part(
            canvas,
            corner,
            at,
            (0, 0, corner.width, corner.height),
            clip,
            1.0,
        );
    }
}

/// The rows of `NETIPX3` that are its title bar: the two "3" backgrounds are
/// the same through row 76 apart from the title's text and the palettes; the
/// photograph under the bar starts at row 77 (measured on the imported
/// pictures, EF8).
pub const TITLE_BAR_ROWS: i32 = 77;

/// A background of one retail picture, with the player's own title bar over
/// its top rows when the kit holds one ([`super::header`]).
///
/// John's look for the connection screens, from 2026-10-05, is `NETIPX3`
/// alone: its grey photograph under its own title bar, which a player can
/// reword. (On 2026-10-01 he approved `MODEM3`'s red photograph under
/// `NETIPX3`'s bar; the red is no longer drawn.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Background {
    base: &'static str,
    header: bool,
}

impl Background {
    /// One whole picture, as retail's NETWORK CONNECTION is `NETIPX3`.
    #[cfg(test)]
    pub fn single(base: &'static str) -> Self {
        Self {
            base,
            header: false,
        }
    }
    /// `NETIPX3`, with the player's title bar over its bar when there is one.
    pub fn direct_connection() -> Self {
        Self {
            base: "NETIPX3",
            header: true,
        }
    }
    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        let base = kit.sprite(self.base);
        canvas.0[..WIDTH * HEIGHT * 4].copy_from_slice(&base.rgba[..WIDTH * HEIGHT * 4]);
        if let Some(header) = kit.header().filter(|_| self.header) {
            canvas.blit(header, (0, 0), 0, header.width, 1.0);
        }
    }
}

/// The part of a screen that never changes, drawn once and kept: its pixels,
/// and its text when that is recorded to be drawn sharp (`crate::ui_text`)
/// rather than baked into the pixels.
pub struct Backdrop {
    pixels: Vec<u8>,
    text: Option<crate::ui_text::Layer>,
    /// Whether the text was recorded: the cache is made again if that changes.
    sharp: bool,
}

impl Backdrop {
    /// Draws the screen's backdrop with `draw`.
    pub fn new(draw: impl FnOnce(&mut Canvas)) -> Self {
        let sharp = crate::ui_text::recording();
        let mut pixels = vec![0u8; crate::menu::WIDTH * crate::menu::HEIGHT * 4];
        let ((), text) = crate::ui_text::capture(|| draw(&mut Canvas(&mut pixels)));
        Self {
            pixels,
            text,
            sharp,
        }
    }

    /// Puts the backdrop on `canvas` and its text, if recorded, in the layer
    /// being recorded.
    pub fn put(&self, canvas: &mut Canvas) {
        canvas.0[..self.pixels.len()].copy_from_slice(&self.pixels);
        if let Some(text) = &self.text {
            crate::ui_text::replay(text);
        }
    }

    pub fn is_for_now(&self) -> bool {
        self.sharp == crate::ui_text::recording()
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_kit::{at, blank, font, kit, tone_of, with};
    use super::*;
    use crate::menu::Sprite;

    fn banded(width: usize, height: usize, bands: &[[u8; 3]], vertical: bool) -> Sprite {
        let mut rgba = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let band = if vertical {
                    y * bands.len() / height
                } else {
                    x * bands.len() / width
                };
                let c = bands[band];
                rgba.extend_from_slice(&[c[0], c[1], c[2], 255]);
            }
        }
        Sprite {
            width,
            height,
            rgba,
            glyphs: Vec::new(),
        }
    }

    #[test]
    fn the_panel_follows_the_recipe() {
        let _ = font;
        let top = [10, 0, 0];
        let bottom = [0, 10, 0];
        let left = [0, 0, 10];
        let right = [10, 10, 0];
        // EDGETB: top two rows one colour, bottom two another. EDGELR: left
        // two columns and right two columns.
        let kit = with(
            kit(),
            "EDGETB",
            banded(47, 4, &[top, top, bottom, bottom], true),
        );
        let kit = with(
            kit,
            "EDGELR",
            banded(4, 26, &[left, left, right, right], false),
        );
        let mut pixels = blank();
        let (x, y, w, h) = (100, 100, 200, 120);
        draw_panel(&mut Canvas(&mut pixels), &kit, (x, y, w, h));
        // The fill, inside.
        assert_eq!(at(&pixels, 150, 150), tone_of(&kit, "PANEL"));
        // The corners.
        assert_eq!(at(&pixels, x, y), tone_of(&kit, "EDGETL"));
        assert_eq!(at(&pixels, x + w - 1, y), tone_of(&kit, "EDGETR"));
        assert_eq!(at(&pixels, x, y + h - 1), tone_of(&kit, "EDGEBL"));
        assert_eq!(at(&pixels, x + w - 1, y + h - 1), tone_of(&kit, "EDGEBR"));
        // The edges, between the corners.
        assert_eq!(at(&pixels, x + 60, y), top);
        assert_eq!(at(&pixels, x + 60, y + 1), top);
        assert_eq!(
            at(&pixels, x + 60, y + 2),
            tone_of(&kit, "PANEL"),
            "two pixels thick"
        );
        assert_eq!(at(&pixels, x + 60, y + h - 1), bottom);
        assert_eq!(at(&pixels, x + 60, y + h - 2), bottom);
        assert_eq!(at(&pixels, x, y + 60), left);
        assert_eq!(at(&pixels, x + 1, y + 60), left);
        assert_eq!(at(&pixels, x + 2, y + 60), tone_of(&kit, "PANEL"));
        assert_eq!(at(&pixels, x + w - 1, y + 60), right);
        assert_eq!(at(&pixels, x + w - 2, y + 60), right);
        // Tiling: the last tile of the top edge is cut at the corner (x + w - 31).
        assert_eq!(at(&pixels, x + w - 32, y), top);
        // Nothing outside the rectangle.
        assert_eq!(at(&pixels, x - 1, y + 60), [0; 3]);
        assert_eq!(at(&pixels, x + w, y + 60), [0; 3]);
        assert_eq!(at(&pixels, x + 60, y + h), [0; 3]);
    }

    #[test]
    fn a_panel_bigger_than_the_fill_tiles_it() {
        let kit = kit();
        let mut pixels = blank();
        draw_panel(&mut Canvas(&mut pixels), &kit, (0, 0, 640, 480));
        // PANEL is 640 by 480 already; a smaller fill picture tiles.
        let small = with(kit, "PANEL", banded(100, 100, &[[1, 2, 3]], false));
        let mut pixels = blank();
        draw_panel(&mut Canvas(&mut pixels), &small, (0, 0, 400, 300));
        assert_eq!(at(&pixels, 250, 250), [1, 2, 3]);
        assert_eq!(at(&pixels, 399 - 40, 299 - 40), [1, 2, 3]);
    }

    #[test]
    fn the_background_is_netipx3_whole_with_the_players_bar_over_it() {
        let kit = kit();
        let mut pixels = blank();
        Background::direct_connection().draw(&mut Canvas(&mut pixels), &kit);
        // No red: every row is NETIPX3's, the bar and the photograph.
        for (x, y) in [(320, 40), (639, 76), (320, 77), (320, 300), (0, 479)] {
            assert_eq!(at(&pixels, x, y), tone_of(&kit, "NETIPX3"), "{x}, {y}");
        }
        // The player's bar replaces the bar's rows where it is opaque, shows
        // the retail bar through where it is clear and blends in between,
        // and stops at row 77.
        let (width, rows) = (640usize, 77usize);
        let mut rgba = Vec::new();
        for _y in 0..rows {
            for x in 0..width {
                let alpha = match x {
                    0..=99 => 255,
                    100..=199 => 0,
                    _ => 128,
                };
                rgba.extend_from_slice(&[200, 100, 50, alpha]);
            }
        }
        let bar = Sprite {
            width,
            height: rows,
            rgba,
            glyphs: Vec::new(),
        };
        let kit = kit.with_header(bar);
        let mut pixels = blank();
        Background::direct_connection().draw(&mut Canvas(&mut pixels), &kit);
        let retail = tone_of(&kit, "NETIPX3");
        assert_eq!(at(&pixels, 50, 10), [200, 100, 50]);
        assert_eq!(at(&pixels, 150, 10), retail);
        let blended = at(&pixels, 400, 10);
        assert!(blended[0] > retail[0].min(200) || blended[0] < retail[0].max(200));
        assert_ne!(blended, [200, 100, 50]);
        assert_ne!(blended, retail);
        assert_eq!(at(&pixels, 50, 77), retail, "the photograph is not covered");
        // A whole single picture takes no bar.
        let mut pixels = blank();
        Background::single("NETIPX3").draw(&mut Canvas(&mut pixels), &kit);
        assert_eq!(at(&pixels, 50, 10), retail);
    }

    use crate::menu::Canvas;
}
