//! Drawing the widgets share, over `menu::Canvas`: partial and clipped
//! blits, tiled bars and the focus mark. None of it changes what the menu's
//! own screens draw.
use super::Rect;
use crate::menu::{Canvas, HEIGHT, Sprite, WIDTH, text_width};
use tore_formats::text::GlyphCodes;

/// The keyboard focus mark's colour: the pale green of the focus outline the
/// existing menus draw.
pub const FOCUS_COLOUR: [u8; 4] = [204, 225, 205, 255];

/// The keyboard focus mark: a one pixel dotted rectangle, every other pixel
/// lit, the way the Windows the retail game ran on marked the focused control
/// (*agent decision*: the brief asks for a mark that fits the retail look;
/// dots read apart from the solid frames and bevels the pieces already have,
/// and on the panel's grey they stay visible without hiding a piece).
pub fn focus_mark(canvas: &mut Canvas, (x, y, w, h): Rect) {
    if w < 2 || h < 2 {
        return;
    }
    let mut dot = |px: i32, py: i32| {
        if (px + py) % 2 == 0 {
            canvas.rect((px, py, 1, 1), FOCUS_COLOUR);
        }
    };
    for px in x..x + w {
        dot(px, y);
        dot(px, y + h - 1);
    }
    for py in y + 1..y + h - 1 {
        dot(x, py);
        dot(x + w - 1, py);
    }
}

/// How much brighter ghosted text (an empty field's hint, a disabled label, a
/// dimmed row) is drawn than the dim levels the first widgets used (John,
/// 2026-10-05, *opinionated*, requested). He asked for 50 percent brighter,
/// saw it, and asked for it 15 percent darker: 1.5 * 0.85, about 28 percent
/// brighter than the first widgets drew it.
pub const GHOST_GAIN: f32 = 1.5 * 0.85;

/// A grey tint for ghosted text: `level` made [`GHOST_GAIN`] brighter.
pub fn ghost(level: u8) -> [u8; 3] {
    [(f32::from(level) * GHOST_GAIN).round().min(255.0) as u8; 3]
}

/// `sprite` with its colours `gain` times brighter, none past `ceiling`,
/// alpha untouched: the kit's dim button fonts are made [`GHOST_GAIN`]
/// brighter this way when it is built.
pub fn brighten(sprite: &mut Sprite, gain: f32, ceiling: u8) {
    for pixel in sprite.rgba.chunks_exact_mut(4) {
        for channel in &mut pixel[..3] {
            let lifted = (f32::from(*channel) * gain).round().min(f32::from(ceiling));
            // Never darken a pixel that is already past the ceiling.
            *channel = (lifted as u8).max(*channel);
        }
    }
}

/// Blends `rgba` (a source pixel with coverage in its alpha) over the canvas
/// pixel at `at`, exactly as `Canvas::blit` does, with the colour scaled by
/// `gain` and optionally multiplied by `tint`.
#[inline]
fn blend(pixels: &mut [u8], at: usize, source: &[u8], gain: f32, tint: Option<[u8; 3]>) {
    let alpha = source[3] as u32;
    if alpha == 0 {
        return;
    }
    if alpha == 255 && gain == 1.0 && tint.is_none() {
        pixels[at..at + 4].copy_from_slice(&source[..4]);
        return;
    }
    for c in 0..3 {
        let mut shade = source[c] as f32 * gain;
        if let Some(tint) = tint {
            shade = shade * tint[c] as f32 / 255.0;
        }
        let shade = shade.min(255.0) as u32;
        pixels[at + c] =
            ((shade * alpha + pixels[at + c] as u32 * (255 - alpha) + 127) / 255) as u8;
    }
    pixels[at + 3] = 255;
}

/// The part `(sx, sy, w, h)` of `sprite` at `(x, y)`, clipped to `clip` and
/// the canvas.
pub fn blit_part(
    canvas: &mut Canvas,
    sprite: &Sprite,
    (x, y): (i32, i32),
    (sx, sy, w, h): (usize, usize, usize, usize),
    clip: Rect,
    gain: f32,
) {
    let (cx, cy, cw, ch) = clip;
    for yy in 0..h.min(sprite.height.saturating_sub(sy)) {
        let dy = y + yy as i32;
        if dy < cy.max(0) || dy >= (cy + ch).min(HEIGHT as i32) {
            continue;
        }
        for xx in 0..w.min(sprite.width.saturating_sub(sx)) {
            let dx = x + xx as i32;
            if dx < cx.max(0) || dx >= (cx + cw).min(WIDTH as i32) {
                continue;
            }
            let source = ((sy + yy) * sprite.width + sx + xx) * 4;
            let dest = (dy as usize * WIDTH + dx as usize) * 4;
            blend(canvas.0, dest, &sprite.rgba[source..source + 4], gain, None);
        }
    }
}

/// Text in `font` at `(x, y)` (the top of the font cell), clipped to `clip`,
/// optionally tinted. Returns the pen's end x.
pub fn text_clipped(
    canvas: &mut Canvas,
    font: &Sprite,
    text: &str,
    (mut x, y): (i32, i32),
    clip: Rect,
    tint: Option<[u8; 3]>,
) -> i32 {
    let (cx, cy, cw, ch) = clip;
    for code in text.glyph_codes() {
        let [sx, w, h] = font.glyphs[code as usize];
        // Skip glyphs that are wholly outside, the common case in a field
        // that has scrolled.
        if x + w as i32 > cx.max(0) && x < (cx + cw).min(WIDTH as i32) {
            for yy in 0..h {
                let dy = y + yy as i32;
                if dy < cy.max(0) || dy >= (cy + ch).min(HEIGHT as i32) {
                    continue;
                }
                for xx in 0..w {
                    let dx = x + xx as i32;
                    if dx < cx.max(0) || dx >= (cx + cw).min(WIDTH as i32) {
                        continue;
                    }
                    let source = (yy * font.width + sx + xx) * 4;
                    let dest = (dy as usize * WIDTH + dx as usize) * 4;
                    blend(canvas.0, dest, &font.rgba[source..source + 4], 1.0, tint);
                }
            }
        }
        x += w as i32;
    }
    x
}

/// A bar made of a left cap, a repeating middle and a right cap, as the list
/// rows, the text fields and the buttons are, from `x` over `width` pixels.
pub fn bar(
    canvas: &mut Canvas,
    (left, middle, right): (&Sprite, &Sprite, &Sprite),
    (x, y): (i32, i32),
    width: i32,
) {
    let end = width - right.width as i32;
    canvas.blit(left, (x, y), 0, left.width, 1.0);
    let mut at = left.width as i32;
    while at < end {
        canvas.blit(
            middle,
            (x + at, y),
            0,
            middle.width.min((end - at) as usize),
            1.0,
        );
        at += middle.width as i32;
    }
    canvas.blit(right, (x + end, y), 0, right.width, 1.0);
}

/// `text` cut to fit `width` pixels in `font`, with a trailing `~` when cut
/// (the font has no ellipsis).
pub fn fit(font: &Sprite, text: &str, width: i32) -> String {
    if text_width(font, text) <= width {
        return text.to_owned();
    }
    let mark = text_width(font, "~");
    let mut out = String::new();
    let mut used = 0;
    for c in text.chars() {
        let w = text_width(font, c.encode_utf8(&mut [0; 4]));
        if used + w + mark > width {
            break;
        }
        out.push(c);
        used += w;
    }
    out.push('~');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ghost_greys_are_brighter_by_the_gain_up_to_white() {
        assert_eq!(ghost(150), [191; 3]);
        assert_eq!(ghost(120), [153; 3]);
        assert_eq!(ghost(118), [150; 3]);
        assert_eq!(ghost(220), [255; 3]);
    }

    #[test]
    fn brightening_a_font_scales_its_colours_and_keeps_its_alpha() {
        let mut sprite = Sprite {
            width: 3,
            height: 1,
            rgba: vec![100, 60, 20, 255, 200, 200, 200, 255, 90, 90, 90, 0],
            glyphs: Vec::new(),
        };
        brighten(&mut sprite, GHOST_GAIN, 220);
        assert_eq!(
            sprite.rgba,
            [128, 77, 26, 255, 220, 220, 220, 255, 115, 115, 115, 0]
        );
        // A pixel already past the ceiling stays as it was.
        brighten(&mut sprite, GHOST_GAIN, 100);
        assert_eq!(sprite.rgba[..4], [128, 98, 33, 255]);
    }
}
