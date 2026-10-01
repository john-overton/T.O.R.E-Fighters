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

/// A background of one retail picture with a rectangle of another laid over
/// it, each in its own palette: John's approved look for Direct Connection is
/// `MODEM3`'s red photograph with `NETIPX3`'s title bar over its top 80 rows,
/// so the screen reads NETWORK CONNECTION (2026-10-01).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Background {
    base: &'static str,
    over: Option<(&'static str, Rect)>,
}

impl Background {
    /// One whole picture, as NETWORK CONNECTION is retail's `NETIPX3`.
    pub fn single(base: &'static str) -> Self {
        Self { base, over: None }
    }
    /// `base` with the rectangle `rect` of `over` laid over the same place.
    pub fn composed(base: &'static str, over: &'static str, rect: Rect) -> Self {
        Self {
            base,
            over: Some((over, rect)),
        }
    }
    /// `MODEM3` under `NETIPX3`'s title bar (the top 80 rows, above the
    /// panel at y 80).
    pub fn direct_connection() -> Self {
        Self::composed("MODEM3", "NETIPX3", (0, 0, 640, 80))
    }
    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        let base = kit.sprite(self.base);
        canvas.0[..WIDTH * HEIGHT * 4].copy_from_slice(&base.rgba[..WIDTH * HEIGHT * 4]);
        if let Some((name, (x, y, w, h))) = self.over {
            let over = kit.sprite(name);
            let (x0, y0) = (x.clamp(0, WIDTH as i32), y.clamp(0, HEIGHT as i32));
            let (x1, y1) = (
                (x + w).clamp(0, WIDTH as i32),
                (y + h).clamp(0, HEIGHT as i32),
            );
            for row in y0..y1 {
                let a = (row as usize * WIDTH + x0 as usize) * 4;
                let b = (row as usize * WIDTH + x1 as usize) * 4;
                canvas.0[a..b].copy_from_slice(&over.rgba[a..b]);
            }
        }
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
    fn the_composed_background_lays_one_picture_over_another() {
        let kit = kit();
        let mut pixels = blank();
        let background = Background::direct_connection();
        background.draw(&mut Canvas(&mut pixels), &kit);
        // The title bar is NETIPX3's, the rest MODEM3's.
        assert_eq!(at(&pixels, 320, 40), tone_of(&kit, "NETIPX3"));
        assert_eq!(at(&pixels, 639, 79), tone_of(&kit, "NETIPX3"));
        assert_eq!(at(&pixels, 320, 80), tone_of(&kit, "MODEM3"));
        assert_eq!(at(&pixels, 0, 479), tone_of(&kit, "MODEM3"));
        // One whole picture.
        Background::single("NETIPX3").draw(&mut Canvas(&mut pixels), &kit);
        assert_eq!(at(&pixels, 320, 300), tone_of(&kit, "NETIPX3"));
        // A rectangle off the edge is clipped, not a panic.
        Background::composed("MODEM3", "NETIPX3", (600, 460, 100, 100))
            .draw(&mut Canvas(&mut pixels), &kit);
        assert_eq!(at(&pixels, 639, 479), tone_of(&kit, "NETIPX3"));
        assert_eq!(at(&pixels, 599, 479), tone_of(&kit, "MODEM3"));
    }

    use crate::menu::Canvas;
}
