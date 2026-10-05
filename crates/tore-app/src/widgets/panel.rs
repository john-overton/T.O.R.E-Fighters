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

/// Where `NETIPX3`'s own lettering NETWORK CONNECTION and its shadow are: the
/// lettering spans x 80 to 350 and y 7 to 28, its shadow x 83 to 354 and y 9 to
/// 32 (measured on the imported picture), a few pixels more all round. The
/// help bar starts at row 36.
const OLD_TITLE: Rect = (76, 3, 282, 32);
/// Where the bar's clean texture to the right of that lettering starts. It is
/// even all along the bar (mean 79 to 82 in every row), so a copy of it hides
/// the lettering without a seam showing.
const CLEAN_FROM: i32 = 364;

/// Covers `NETIPX3`'s lettering with a copy of the bar's own texture from the
/// right of it, row for row. The pixels come from the player's own import and
/// stay in memory; nothing retail is shipped.
fn cover_old_title(canvas: &mut Canvas, picture: &crate::menu::Sprite) {
    let (x, y, w, h) = OLD_TITLE;
    let clean = WIDTH as i32 - CLEAN_FROM;
    for row in y..y + h {
        for column in x..x + w {
            let from = (row as usize * WIDTH + (CLEAN_FROM + (column - x) % clean) as usize) * 4;
            let to = (row as usize * WIDTH + column as usize) * 4;
            canvas.0[to..to + 4].copy_from_slice(&picture.rgba[from..from + 4]);
        }
    }
}

/// A background of one retail picture, with the connection screens' title
/// lettering over its title bar ([`super::header`]).
///
/// John's look for the connection screens, from 2026-10-05, is `NETIPX3`
/// alone: its grey photograph under its own title bar, the retail lettering
/// covered and DIRECT NETWORK CONNECTION drawn instead, or the player's own
/// lettering. (On 2026-10-01 he approved `MODEM3`'s red photograph under
/// `NETIPX3`'s bar; the red is no longer drawn.)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Background {
    base: &'static str,
    header: bool,
    title: super::header::Title,
}

impl Background {
    /// One whole picture, as retail's NETWORK CONNECTION is `NETIPX3`.
    #[cfg(test)]
    pub fn single(base: &'static str) -> Self {
        Self {
            base,
            header: false,
            title: super::header::Title::Direct,
        }
    }
    /// `NETIPX3` with its lettering covered and the title lettering, the
    /// player's or the game's, fixed to the bar's top right.
    pub fn direct_connection() -> Self {
        Self {
            base: "NETIPX3",
            header: true,
            title: super::header::Title::Direct,
        }
    }
    /// The same, lettered INTERNET LOBBY (or with the player's own
    /// `InternetLobby.png`): the Internet Lobby screen's.
    pub fn internet_lobby() -> Self {
        Self {
            base: "NETIPX3",
            header: true,
            title: super::header::Title::Internet,
        }
    }
    pub fn draw(&self, canvas: &mut Canvas, kit: &Kit) {
        let base = kit.sprite(self.base);
        canvas.0[..WIDTH * HEIGHT * 4].copy_from_slice(&base.rgba[..WIDTH * HEIGHT * 4]);
        if self.header {
            cover_old_title(canvas, base);
            let title = kit
                .title_picture(self.title)
                .unwrap_or_else(|| self.title.built_in());
            canvas.blit(
                title,
                (WIDTH as i32 - title.width as i32, 0),
                0,
                title.width,
                1.0,
            );
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

    #[test]
    fn the_internet_lobby_has_its_own_words_and_its_own_players_bar() {
        let kit = kit();
        let direct = {
            let mut pixels = blank();
            Background::direct_connection().draw(&mut Canvas(&mut pixels), &kit);
            pixels
        };
        let internet = {
            let mut pixels = blank();
            Background::internet_lobby().draw(&mut Canvas(&mut pixels), &kit);
            pixels
        };
        // Same photograph below the bar, other words in it.
        assert_eq!(direct[77 * 640 * 4..], internet[77 * 640 * 4..]);
        assert_ne!(direct[..77 * 640 * 4], internet[..77 * 640 * 4]);
        // A player's Internet Lobby picture is for that screen only, and the
        // Direct Connection's picture is for the other.
        let bar = Sprite {
            width: 640,
            height: 77,
            rgba: [200, 100, 50, 255].repeat(640 * 77),
            glyphs: Vec::new(),
        };
        let with_internet = kit.with_internet_header(bar);
        let mut pixels = blank();
        Background::internet_lobby().draw(&mut Canvas(&mut pixels), &with_internet);
        assert_eq!(at(&pixels, 50, 10), [200, 100, 50]);
        let mut pixels = blank();
        Background::direct_connection().draw(&mut Canvas(&mut pixels), &with_internet);
        assert_ne!(at(&pixels, 50, 10), [200, 100, 50]);
    }

    #[test]
    fn the_old_lettering_is_covered_with_the_bars_own_texture_and_the_words_go_top_right() {
        // A NETIPX3 whose red grows with x, so where a pixel was copied from
        // can be read off it.
        let mut rgba = Vec::new();
        for _y in 0..480 {
            for x in 0..640usize {
                rgba.extend_from_slice(&[(x % 251) as u8, 0, 0, 255]);
            }
        }
        let picture = Sprite {
            width: 640,
            height: 480,
            rgba,
            glyphs: Vec::new(),
        };
        let kit = with(kit(), "NETIPX3", picture);
        let mut pixels = blank();
        Background::direct_connection().draw(&mut Canvas(&mut pixels), &kit);
        let red = |x: i32, y: i32| at(&pixels, x, y)[0];
        // Under the old lettering (below the new words' ink at row 33) each
        // pixel is the bar's texture from the clean strip, 364 on.
        assert_eq!(
            red(100, 33),
            (388 % 251) as u8,
            "76 + 24 copies from 364 + 24"
        );
        assert_eq!(red(352, 33), (364 % 251) as u8, "the strip starts again");
        // Just outside the covered rectangle, nothing changed.
        for (x, y) in [(75, 33), (358, 33), (100, 2), (100, 35), (600, 15)] {
            assert_eq!(red(x, y), (x % 251) as u8, "{x}, {y}");
        }
        // The words are drawn in the top right: white ink from x 80 on.
        let white = |x: i32, y: i32| at(&pixels, x, y) == [255; 3];
        let inked = (81..522)
            .flat_map(|x| (7..31).map(move |y| (x, y)))
            .filter(|(x, y)| white(*x, *y))
            .count();
        assert!(inked > 400, "{inked} white pixels in the lettering's box");
        assert!(
            (0..80).all(|x| (0..36).all(|y| !white(x, y))),
            "nothing left of x 80"
        );
    }

    use crate::menu::Canvas;
}
