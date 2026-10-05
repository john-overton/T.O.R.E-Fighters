//! Sharp text for the multiplayer screens.
//!
//! The menu is a 640 by 480 canvas the renderer stretches to the window, so
//! text drawn into it with the retail bitmap fonts (8 to 12 pixels tall) turns
//! to uneven blocks at full screen. While a screen is being drawn for the
//! window ([`begin`]), its text calls ([`text`]) are not drawn into the canvas
//! but recorded; the renderer then draws the glyphs at the window's own
//! resolution on top of the stretched canvas ([`Layer::quads`], the GPU side is
//! `ui_text_renderer.rs`). Headless snapshots and tests do not record, so they
//! draw the retail text into the canvas as before.
//!
//! A run keeps the retail font's size and place: it starts where the retail
//! text would, ends where it would (the retail advances sum to the same width,
//! so every layout, centring and cut-off the screens do from the retail widths
//! holds), and its capitals are the retail capitals' height on the retail
//! baseline. Between the ends the letters keep Noto's own proportions, squeezed
//! to the narrow retail letters, with the difference from the retail width
//! spread evenly: the retail letters sit in near fixed width cells that look
//! gappy with another face. Typed text ([`text_cells`]) is the exception: each
//! glyph sits in its retail cell, so the caret, which the field places from
//! the retail widths, stays between the letters. The shapes are Noto Sans
//! Medium (open licensed, not retail) from `assets/ui-text.bin`, made by
//! `tools/build_ui_text_atlas.py`.
//!
//! A pop-up panel drawn over earlier text hides it in the canvas, but recorded
//! text would stay on top, so [`occlude`] notes the panel's rectangle and the
//! layer cuts earlier glyphs to what shows around it.
//!
//! Requested by John on 2026-10-05 (*opinionated*): text easier to read, the
//! same size, not fuzzy at full screen.
use crate::menu::{Canvas, Sprite, text_width};
use crate::widgets::{Kit, Rect};
use std::cell::RefCell;
use std::sync::OnceLock;
use tore_formats::text::GlyphCodes;

const DATA: &[u8] = include_bytes!("../assets/ui-text.bin");

/// One glyph of the atlas, in atlas pixels.
#[derive(Clone, Copy, Debug)]
struct Glyph {
    slot: usize,
    ink_x: f32,
    ink_y: f32,
    ink_w: f32,
    ink_h: f32,
}

/// The glyph atlas: cells of `cell` pixels in `columns` columns, and the
/// glyphs in them.
pub struct Atlas {
    pub cell: (usize, usize),
    pub columns: usize,
    em: f32,
    cap: f32,
    origin_x: f32,
    baseline: f32,
    glyphs: [Option<Glyph>; 256],
    advances: [f32; 256],
    pub width: usize,
    pub height: usize,
    pub plane: &'static [u8],
}

fn le16(data: &[u8], at: usize) -> usize {
    usize::from(u16::from_le_bytes([data[at], data[at + 1]]))
}

fn le16s(data: &[u8], at: usize) -> f32 {
    f32::from(i16::from_le_bytes([data[at], data[at + 1]]))
}

/// The bundled atlas, parsed once. It is our own file, so a bad one is a
/// build error and panics.
pub fn atlas() -> &'static Atlas {
    static ATLAS: OnceLock<Atlas> = OnceLock::new();
    ATLAS.get_or_init(|| {
        assert_eq!(&DATA[..4], b"TUA1", "ui-text.bin is not a text atlas");
        let (cell_w, cell_h, columns) = (le16(DATA, 4), le16(DATA, 6), le16(DATA, 8));
        let (em, cap, origin_x, baseline, count) = (
            le16(DATA, 10),
            le16(DATA, 12),
            le16(DATA, 14),
            le16(DATA, 16),
            le16(DATA, 18),
        );
        let mut glyphs = [None; 256];
        let mut advances = [0.0; 256];
        for slot in 0..count {
            let at = 20 + slot * 12;
            let code = usize::from(DATA[at]);
            advances[code] = le16(DATA, at + 2) as f32 / 16.0;
            glyphs[code] = Some(Glyph {
                slot,
                ink_x: le16s(DATA, at + 4),
                ink_y: le16s(DATA, at + 6),
                ink_w: le16(DATA, at + 8) as f32,
                ink_h: le16(DATA, at + 10) as f32,
            });
        }
        let plane = &DATA[20 + count * 12..];
        let (width, height) = (columns * cell_w, count.div_ceil(columns) * cell_h);
        assert_eq!(plane.len(), width * height, "ui-text.bin is cut short");
        Atlas {
            cell: (cell_w, cell_h),
            columns,
            em: em as f32,
            cap: cap as f32,
            origin_x: origin_x as f32,
            baseline: baseline as f32,
            glyphs,
            advances,
            width,
            height,
            plane,
        }
    })
}

/// How one retail font is drawn sharp: made from the font's own picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    /// Canvas pixels to an atlas pixel, so a capital matches the retail one.
    scale: f32,
    /// Canvas pixels from the top of a retail cell to the baseline.
    baseline: f32,
    /// How much narrower than its natural shape every glyph is drawn, so the
    /// letters fit the retail font's narrow cells.
    squeeze: f32,
    /// The text's colour before a tint: the font's brightest.
    color: [u8; 3],
}

/// The share of the font's brightest colour a glyph's body is drawn in: the
/// retail glyphs mix that colour with darker antialiasing, the sharp ones are
/// all body.
const BODY: f32 = 0.94;

impl Style {
    /// The style of `font`, measured on its capital H and its letter widths;
    /// none for a font with no H.
    pub fn of(font: &Sprite) -> Option<Self> {
        let atlas = atlas();
        let [sx, w, h] = *font.glyphs.get(usize::from(b'H'))?;
        let lit = |x: usize, y: usize| font.rgba[(y * font.width + sx + x) * 4 + 3] > 100;
        let rows: Vec<usize> = (0..h).filter(|y| (0..w).any(|x| lit(x, *y))).collect();
        let (top, bottom) = (*rows.first()?, *rows.last()?);
        let scale = (bottom - top + 1) as f32 / atlas.cap;
        let letters = (b'a'..=b'z').chain(b'A'..=b'Z');
        let (retail, natural) = letters.fold((0.0, 0.0), |(r, n), c| {
            (
                r + font.glyphs[usize::from(c)][1] as f32,
                n + atlas.advances[usize::from(c)] * scale,
            )
        });
        let mut best = ([0u8; 3], 0u32);
        for px in font.rgba.chunks_exact(4).filter(|px| px[3] == 255) {
            let sum = u32::from(px[0]) + u32::from(px[1]) + u32::from(px[2]);
            if sum > best.1 {
                best = ([px[0], px[1], px[2]], sum);
            }
        }
        Some(Self {
            scale,
            baseline: (bottom + 1) as f32,
            squeeze: (retail / natural).clamp(0.6, 1.0),
            color: best.0.map(|c| (f32::from(c) * BODY).round() as u8),
        })
    }
}

/// One glyph of a recorded run: the retail pen position and cell width.
#[derive(Clone, Copy, Debug)]
struct Placed {
    code: u8,
    x: i32,
    advance: u8,
}

/// One recorded text call.
#[derive(Clone, Debug)]
struct Run {
    glyphs: Vec<Placed>,
    y: i32,
    style: Style,
    tint: Option<[u8; 3]>,
    clip: Rect,
    /// Each glyph in its retail cell, not in the face's own spacing.
    cells: bool,
}

/// The width of Noto Sans' space, in ems.
const SPACE: f32 = 0.26;

/// What a screen's drawing recorded: its text runs in drawing order and the
/// rectangles later drawn over them.
#[derive(Clone, Debug, Default)]
pub struct Layer {
    runs: Vec<Run>,
    /// A rectangle and the number of runs recorded before it was drawn.
    occluders: Vec<([f32; 4], usize)>,
}

thread_local! {
    static RECORDER: RefCell<Option<Layer>> = const { RefCell::new(None) };
}

/// Starts recording: from now on [`text`] records instead of drawing, until
/// [`finish`].
pub fn begin() {
    RECORDER.with(|r| *r.borrow_mut() = Some(Layer::default()));
}

/// Stops recording and gives what was recorded.
pub fn finish() -> Option<Layer> {
    RECORDER.with(|r| r.borrow_mut().take())
}

/// Whether text is being recorded.
pub fn recording() -> bool {
    RECORDER.with(|r| r.borrow().is_some())
}

/// Runs `draw` and gives what it recorded apart from the layer being
/// recorded, for a part of a screen that is drawn once and kept (its pixels
/// are cached, so its text is replayed from the cache with [`replay`]). None
/// when nothing is being recorded.
pub fn capture<R>(draw: impl FnOnce() -> R) -> (R, Option<Layer>) {
    let outer = RECORDER.with(|r| r.borrow_mut().as_mut().map(std::mem::take));
    let result = draw();
    let inner = outer.and_then(|outer| {
        RECORDER.with(|r| {
            r.borrow_mut()
                .as_mut()
                .map(|now| std::mem::replace(now, outer))
        })
    });
    (result, inner)
}

/// Adds a captured layer to the one being recorded, as if its drawing had
/// just happened.
pub fn replay(layer: &Layer) {
    RECORDER.with(|r| {
        if let Some(now) = r.borrow_mut().as_mut() {
            let offset = now.runs.len();
            now.runs.extend(layer.runs.iter().cloned());
            now.occluders.extend(
                layer
                    .occluders
                    .iter()
                    .map(|(rect, after)| (*rect, after + offset)),
            );
        }
    });
}

/// Notes that `rect` has just been drawn over whatever text came before it.
pub fn occlude((x, y, w, h): Rect) {
    RECORDER.with(|r| {
        if let Some(now) = r.borrow_mut().as_mut() {
            let rect = [x as f32, y as f32, (x + w) as f32, (y + h) as f32];
            now.occluders.push((rect, now.runs.len()));
        }
    });
}

/// Draws `text` in `font` at `at` (the top left of the font cell), clipped to
/// `clip` when given, tinted when given: the one call the widgets draw text
/// with. Into the canvas as the retail font, or recorded to be drawn sharp
/// while recording and `kit` can style the font. Returns the pen's end x.
pub fn text(
    canvas: &mut Canvas,
    kit: &Kit,
    font: &Sprite,
    text: &str,
    at: (i32, i32),
    clip: Option<Rect>,
    tint: Option<[u8; 3]>,
) -> i32 {
    place(canvas, kit, font, text, at, clip, tint, false)
}

/// [`text`] with every glyph in its retail cell: for text the player edits,
/// whose caret is placed from the retail widths.
pub fn text_cells(
    canvas: &mut Canvas,
    kit: &Kit,
    font: &Sprite,
    text: &str,
    at: (i32, i32),
    clip: Option<Rect>,
    tint: Option<[u8; 3]>,
) -> i32 {
    place(canvas, kit, font, text, at, clip, tint, true)
}

#[allow(clippy::too_many_arguments)]
fn place(
    canvas: &mut Canvas,
    kit: &Kit,
    font: &Sprite,
    text: &str,
    at: (i32, i32),
    clip: Option<Rect>,
    tint: Option<[u8; 3]>,
    cells: bool,
) -> i32 {
    if recording()
        && let Some(style) = kit.text_style(font)
    {
        let mut x = at.0;
        let glyphs = text
            .glyph_codes()
            .map(|code| {
                let advance = font.glyphs[usize::from(code)][1];
                let placed = Placed {
                    code,
                    x,
                    advance: advance as u8,
                };
                x += advance as i32;
                placed
            })
            .collect();
        let clip = clip.unwrap_or((0, 0, 640, 480));
        RECORDER.with(|r| {
            if let Some(now) = r.borrow_mut().as_mut() {
                now.runs.push(Run {
                    glyphs,
                    y: at.1,
                    style,
                    tint,
                    clip,
                    cells,
                });
            }
        });
        return x;
    }
    match clip {
        Some(clip) => crate::widgets::text_clipped(canvas, font, text, at, clip, tint),
        None => {
            canvas.text(font, text, at.0, at.1, tint);
            at.0 + text_width(font, text)
        }
    }
}

/// One glyph rectangle for the GPU: where it goes on the canvas, its place in
/// the atlas (both as fractions for the atlas), the canvas rectangle it may
/// show in and its colour (linear when the target is sRGB).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quad {
    /// x, y, width, height in canvas pixels.
    pub dst: [f32; 4],
    /// Left, top, right, bottom in atlas pixels.
    pub uv: [f32; 4],
    /// Left, top, right, bottom in canvas pixels.
    pub clip: [f32; 4],
    pub color: [f32; 4],
}

/// The atlas pixels of margin kept round each glyph's ink, for the sampler's
/// blur at its edge.
const MARGIN: f32 = 1.5;

pub(crate) fn srgb_to_linear(byte: u8) -> f32 {
    let c = f32::from(byte) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let encoded = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

/// `piece` less `hole` (left, top, right, bottom): up to four rectangles.
fn subtract(piece: [f32; 4], hole: [f32; 4], out: &mut Vec<[f32; 4]>) {
    let (x0, y0) = (piece[0].max(hole[0]), piece[1].max(hole[1]));
    let (x1, y1) = (piece[2].min(hole[2]), piece[3].min(hole[3]));
    if x0 >= x1 || y0 >= y1 {
        out.push(piece);
        return;
    }
    if piece[1] < y0 {
        out.push([piece[0], piece[1], piece[2], y0]);
    }
    if y1 < piece[3] {
        out.push([piece[0], y1, piece[2], piece[3]]);
    }
    if piece[0] < x0 {
        out.push([piece[0], y0, x0, y1]);
    }
    if x1 < piece[2] {
        out.push([x1, y0, piece[2], y1]);
    }
}

impl Run {
    /// The pen position of each glyph in canvas pixels: the retail cell for
    /// a cell run, otherwise the face's own spacing spread to end where the
    /// retail text would.
    fn lefts(&self, atlas: &Atlas) -> Vec<f32> {
        let first = self.glyphs.first().map_or(0.0, |g| g.x as f32);
        if self.cells {
            return self.glyphs.iter().map(|g| g.x as f32).collect();
        }
        let across = self.style.scale * self.style.squeeze;
        let own: Vec<f32> = self
            .glyphs
            .iter()
            .map(|g| match g.code {
                b' ' => SPACE * atlas.em * across,
                code if atlas.glyphs[usize::from(code)].is_some() => {
                    atlas.advances[usize::from(code)] * across
                }
                _ => f32::from(g.advance),
            })
            .collect();
        let retail: f32 = self.glyphs.iter().map(|g| f32::from(g.advance)).sum();
        let spread =
            ((retail - own.iter().sum::<f32>()) / own.len().max(1) as f32).clamp(-1.0, 1.5);
        let mut pen = first;
        own.iter()
            .map(|advance| {
                let at = pen;
                pen += advance + spread;
                at
            })
            .collect()
    }
}

impl Layer {
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.runs.is_empty()
    }

    /// The glyph rectangles to draw, in drawing order. `linear` is whether the
    /// target converts its output from linear to sRGB, so colours go in
    /// linear.
    pub fn quads(&self, linear: bool) -> Vec<Quad> {
        let atlas = atlas();
        let mut quads = Vec::new();
        let mut pieces = Vec::new();
        let mut next = Vec::new();
        for (index, run) in self.runs.iter().enumerate() {
            let tint = run.tint.unwrap_or([255; 3]);
            let color: [f32; 3] = std::array::from_fn(|c| {
                let byte = (u32::from(run.style.color[c]) * u32::from(tint[c]) + 127) / 255;
                if linear {
                    srgb_to_linear(byte as u8)
                } else {
                    byte as f32 / 255.0
                }
            });
            let clip = [
                run.clip.0 as f32,
                run.clip.1 as f32,
                (run.clip.0 + run.clip.2) as f32,
                (run.clip.1 + run.clip.3) as f32,
            ];
            let covers: Vec<[f32; 4]> = self
                .occluders
                .iter()
                .filter(|(_, after)| *after > index)
                .map(|(rect, _)| *rect)
                .collect();
            let scale = run.style.scale;
            let across = scale * run.style.squeeze;
            let lefts = run.lefts(atlas);
            for (placed, pen) in run.glyphs.iter().zip(lefts) {
                let Some(glyph) = atlas.glyphs[usize::from(placed.code)] else {
                    continue;
                };
                let advance = f32::from(placed.advance);
                let ink_w = if run.cells {
                    (glyph.ink_w * across).min(advance.max(1.0))
                } else {
                    glyph.ink_w * across
                };
                let ink_h = glyph.ink_h * scale;
                // The pad at each side is the margin, the same share of the
                // ink as in the atlas.
                let pad_x = MARGIN * ink_w / glyph.ink_w;
                let pad_y = MARGIN * scale;
                let left = if run.cells {
                    pen + (advance - ink_w) * 0.45
                } else {
                    pen + glyph.ink_x * across
                };
                let top = run.y as f32 + run.style.baseline + glyph.ink_y * scale;
                let dst = [
                    left - pad_x,
                    top - pad_y,
                    ink_w + 2.0 * pad_x,
                    ink_h + 2.0 * pad_y,
                ];
                let (cell_x, cell_y) = (
                    (glyph.slot % atlas.columns) as f32 * atlas.cell.0 as f32,
                    (glyph.slot / atlas.columns) as f32 * atlas.cell.1 as f32,
                );
                let uv = [
                    cell_x + atlas.origin_x + glyph.ink_x - MARGIN,
                    cell_y + atlas.baseline + glyph.ink_y - MARGIN,
                    cell_x + atlas.origin_x + glyph.ink_x + glyph.ink_w + MARGIN,
                    cell_y + atlas.baseline + glyph.ink_y + glyph.ink_h + MARGIN,
                ];
                let rect = [dst[0], dst[1], dst[0] + dst[2], dst[1] + dst[3]];
                pieces.clear();
                pieces.push([
                    rect[0].max(clip[0]),
                    rect[1].max(clip[1]),
                    rect[2].min(clip[2]),
                    rect[3].min(clip[3]),
                ]);
                if pieces[0][0] >= pieces[0][2] || pieces[0][1] >= pieces[0][3] {
                    continue;
                }
                for hole in &covers {
                    next.clear();
                    for piece in &pieces {
                        subtract(*piece, *hole, &mut next);
                    }
                    std::mem::swap(&mut pieces, &mut next);
                }
                for piece in &pieces {
                    quads.push(Quad {
                        dst,
                        uv,
                        clip: *piece,
                        color: [color[0], color[1], color[2], 1.0],
                    });
                }
            }
        }
        quads
    }
}

/// `canvas` (640 by 480 RGBA) stretched `scale` times by nearest pixels with
/// the layer's text drawn over it at that size, blended in sRGB: what the GPU
/// draws, for tests and for looking at the result without a window.
#[cfg(test)]
pub fn composite(canvas: &[u8], layer: &Layer, scale: usize) -> Vec<u8> {
    let (width, height) = (640 * scale, 480 * scale);
    let mut out = vec![255u8; width * height * 4];
    for y in 0..height {
        for x in 0..width {
            let from = ((y / scale) * 640 + x / scale) * 4;
            let to = (y * width + x) * 4;
            out[to..to + 4].copy_from_slice(&canvas[from..from + 4]);
        }
    }
    let atlas = atlas();
    let sample = |u: f32, v: f32| -> f32 {
        let (u, v) = (u - 0.5, v - 0.5);
        let (x0, y0) = (u.floor(), v.floor());
        let (fx, fy) = (u - x0, v - y0);
        let at = |x: f32, y: f32| -> f32 {
            let (x, y) = (
                (x.max(0.0) as usize).min(atlas.width - 1),
                (y.max(0.0) as usize).min(atlas.height - 1),
            );
            f32::from(atlas.plane[y * atlas.width + x]) / 255.0
        };
        let top = at(x0, y0) * (1.0 - fx) + at(x0 + 1.0, y0) * fx;
        let bottom = at(x0, y0 + 1.0) * (1.0 - fx) + at(x0 + 1.0, y0 + 1.0) * fx;
        top * (1.0 - fy) + bottom * fy
    };
    let s = scale as f32;
    for quad in layer.quads(false) {
        let [dx, dy, dw, dh] = quad.dst;
        let (x0, y0) = (
            (quad.clip[0] * s).floor() as usize,
            (quad.clip[1] * s).floor() as usize,
        );
        let (x1, y1) = (
            ((quad.clip[2] * s).ceil() as usize).min(width),
            ((quad.clip[3] * s).ceil() as usize).min(height),
        );
        for y in y0..y1 {
            for x in x0..x1 {
                let (cx, cy) = ((x as f32 + 0.5) / s, (y as f32 + 0.5) / s);
                if cx < quad.clip[0]
                    || cx >= quad.clip[2]
                    || cy < quad.clip[1]
                    || cy >= quad.clip[3]
                {
                    continue;
                }
                let (fx, fy) = ((cx - dx) / dw, (cy - dy) / dh);
                if !(0.0..1.0).contains(&fx) || !(0.0..1.0).contains(&fy) {
                    continue;
                }
                let alpha = sample(
                    quad.uv[0] + fx * (quad.uv[2] - quad.uv[0]),
                    quad.uv[1] + fy * (quad.uv[3] - quad.uv[1]),
                );
                // Blended in linear light, as the GPU does on an sRGB target.
                let at = (y * width + x) * 4;
                for c in 0..3 {
                    let ink = srgb_to_linear((quad.color[c] * 255.0).round() as u8);
                    let under = srgb_to_linear(out[at + c]);
                    out[at + c] = linear_to_srgb(ink * alpha + under * (1.0 - alpha));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::widgets::test_kit::{blank, kit};

    #[test]
    fn the_bundled_atlas_holds_the_glyphs_the_screens_need() {
        let atlas = atlas();
        assert_eq!(atlas.cell, (72, 76));
        assert_eq!(atlas.cap, 34.0, "a capital is 34 pixels at 48 to the em");
        for c in "Direct Network connection 0123456789 ~(){}[]/:.,'\"!?@#%&*+-=<>".chars() {
            if c != ' ' {
                assert!(atlas.glyphs[c as usize].is_some(), "no glyph for {c:?}");
            }
        }
        assert!(atlas.glyphs[0x82].is_some(), "CP437 e with an acute accent");
        assert_eq!(atlas.plane.len(), atlas.width * atlas.height);
    }

    #[test]
    fn a_style_matches_the_retail_capital_and_narrows_the_letters() {
        // The synthetic fonts light every pixel: a 12 row capital, 7 wide.
        let font = crate::widgets::test_kit::font(7, 12);
        let style = Style::of(&font).expect("a style");
        assert_eq!(style.baseline, 12.0);
        assert!((style.scale - 12.0 / 34.0).abs() < 1e-6);
        // Noto's letters are wider than 7 pixels at that size: squeezed.
        assert!(
            style.squeeze < 1.0 && style.squeeze >= 0.6,
            "{}",
            style.squeeze
        );
        assert_eq!(style.color, [240, 240, 240]);
        let none = Sprite {
            width: 4,
            height: 1,
            rgba: vec![0; 16],
            glyphs: Vec::new(),
        };
        assert!(Style::of(&none).is_none());
    }

    #[test]
    fn nothing_is_recorded_unless_asked_and_the_pixels_are_drawn() {
        let kit = kit();
        let font = kit.sprite("PANELFNT");
        let mut pixels = blank();
        assert!(!recording());
        let end = text(
            &mut Canvas(&mut pixels),
            &kit,
            font,
            "Hi",
            (10, 10),
            None,
            None,
        );
        assert_eq!(end, 10 + 2 * 5);
        assert!(
            pixels.iter().any(|b| *b != 0),
            "the retail text is in the canvas"
        );
        assert!(finish().is_none());
    }

    #[test]
    fn recording_keeps_text_out_of_the_canvas_and_returns_the_same_end() {
        let kit = kit();
        let font = kit.sprite("PANELFNT");
        let mut pixels = blank();
        begin();
        assert!(recording());
        let end = text(
            &mut Canvas(&mut pixels),
            &kit,
            font,
            "Hi",
            (10, 10),
            None,
            None,
        );
        let layer = finish().expect("a layer");
        assert_eq!(end, 20);
        assert!(
            pixels.iter().all(|b| *b == 0),
            "nothing was drawn into the canvas"
        );
        assert!(!layer.is_empty());
        let quads = layer.quads(false);
        assert_eq!(quads.len(), 2, "one rectangle for each letter");
        // The H starts in its cell at x 10 and sits on the baseline, 10 rows
        // down (the synthetic font's cell is 10 high).
        let h = quads[0];
        assert!(h.dst[0] > 8.0 && h.dst[0] < 12.0, "{:?}", h.dst);
        assert!(h.dst[1] + h.dst[3] > 10.0 && h.dst[1] < 20.0);
    }

    #[test]
    fn clips_cut_the_glyphs_and_a_later_panel_hides_earlier_text() {
        let kit = kit();
        let font = kit.sprite("SMLFONT");
        let mut pixels = blank();
        begin();
        // 20 letters, 6 wide each, from x 0 to 120, clipped at 60.
        let wide = "H".repeat(20);
        text_cells(
            &mut Canvas(&mut pixels),
            &kit,
            font,
            &wide,
            (0, 0),
            Some((0, 0, 60, 12)),
            None,
        );
        text_cells(
            &mut Canvas(&mut pixels),
            &kit,
            font,
            "HHHH",
            (200, 0),
            None,
            None,
        );
        // A panel over the second run's second and third letters.
        occlude((206, 0, 12, 12));
        // Text drawn after the panel is not hidden by it.
        text_cells(
            &mut Canvas(&mut pixels),
            &kit,
            font,
            "HH",
            (206, 0),
            None,
            None,
        );
        let layer = finish().unwrap();
        let quads = layer.quads(false);
        let first: Vec<_> = quads.iter().filter(|q| q.dst[0] < 150.0).collect();
        assert!(first.len() >= 10, "the ten letters inside the clip");
        assert!(
            first.iter().all(|q| q.clip[2] <= 60.0),
            "none shows past the clip"
        );
        // A glyph shows where its middle is inside its visible piece.
        let shown: Vec<f32> = quads
            .iter()
            .filter(|q| q.dst[0] >= 150.0)
            .map(|q| (q.dst[0] + q.dst[2] / 2.0, q.dst[1] + q.dst[3] / 2.0, q.clip))
            .filter(|(x, y, clip)| (clip[0]..clip[2]).contains(x) && (clip[1]..clip[3]).contains(y))
            .map(|(x, _, _)| x)
            .collect();
        let under = shown.iter().filter(|c| (206.0..218.0).contains(*c)).count();
        assert_eq!(
            under, 2,
            "only the two letters drawn after the panel: {shown:?}"
        );
        assert_eq!(
            shown.len(),
            4,
            "the first and last of the four stay: {shown:?}"
        );
    }

    #[test]
    fn a_runs_own_spacing_ends_where_the_retail_text_would() {
        // A font with retail-like cells: 5 wide, 3 for i and l, 2 for a space.
        let mut font = crate::widgets::test_kit::font(5, 10);
        for code in [b'i', b'l', b'I'] {
            font.glyphs[usize::from(code)][1] = 3;
        }
        font.glyphs[usize::from(b' ')][1] = 2;
        let words = "Hello little world";
        let retail: i32 = words
            .bytes()
            .map(|c| font.glyphs[usize::from(c)][1] as i32)
            .sum();
        let mut kit = kit();
        kit = crate::widgets::test_kit::with(kit, "PANELFNT", font);
        let font = kit.sprite("PANELFNT");
        let mut pixels = blank();
        begin();
        let end = text(
            &mut Canvas(&mut pixels),
            &kit,
            font,
            words,
            (100, 50),
            None,
            None,
        );
        let layer = finish().unwrap();
        assert_eq!(end, 100 + retail);
        let quads = layer.quads(false);
        let first = quads.first().unwrap().dst[0];
        let last = quads.last().unwrap();
        let right = last.dst[0] + last.dst[2];
        assert!((first - 100.0).abs() < 2.0, "starts at {first}");
        assert!(
            (right - (100 + retail) as f32).abs() < 3.0,
            "ends at {right}, retail text ends at {}",
            100 + retail
        );
        // Narrow letters take less room than wide ones: the two l's and i of
        // "little" are closer together than the letters of "world".
        let lefts: Vec<f32> = quads.iter().map(|q| q.dst[0]).collect();
        assert!(
            lefts.windows(2).all(|w| w[1] > w[0]),
            "left to right: {lefts:?}"
        );
    }

    #[test]
    fn subtracting_a_hole_leaves_the_rest() {
        let mut out = Vec::new();
        subtract([0.0, 0.0, 10.0, 10.0], [4.0, 4.0, 6.0, 6.0], &mut out);
        assert_eq!(out.len(), 4);
        let area: f32 = out.iter().map(|r| (r[2] - r[0]) * (r[3] - r[1])).sum();
        assert_eq!(area, 100.0 - 4.0);
        out.clear();
        subtract([0.0, 0.0, 10.0, 10.0], [20.0, 20.0, 30.0, 30.0], &mut out);
        assert_eq!(out, [[0.0, 0.0, 10.0, 10.0]]);
        out.clear();
        subtract([0.0, 0.0, 10.0, 10.0], [-5.0, -5.0, 15.0, 15.0], &mut out);
        assert!(out.is_empty());
    }

    #[test]
    fn a_captured_part_replays_into_the_layer_being_recorded() {
        let kit = kit();
        let font = kit.sprite("PANELFNT");
        let mut pixels = blank();
        begin();
        text(
            &mut Canvas(&mut pixels),
            &kit,
            font,
            "A",
            (0, 0),
            None,
            None,
        );
        let (_, part) = capture(|| {
            text(
                &mut Canvas(&mut pixels),
                &kit,
                font,
                "BC",
                (30, 0),
                None,
                None,
            );
            occlude((0, 0, 5, 5));
        });
        let part = part.expect("a captured layer");
        assert_eq!(part.runs.len(), 1);
        replay(&part);
        replay(&part);
        let layer = finish().unwrap();
        assert_eq!(layer.runs.len(), 3, "A, then BC twice");
        // The second copy's occluder comes after the runs before it.
        assert_eq!(layer.occluders[0].1, 2);
        assert_eq!(layer.occluders[1].1, 3);
        // Not recording: nothing is captured and the drawing still runs.
        let (answer, none) = capture(|| 7);
        assert_eq!((answer, none.is_none()), (7, true));
    }

    #[test]
    fn the_cpu_composite_draws_dark_ink_where_the_text_is() {
        let kit = kit();
        let font = kit.sprite("PANELFNT");
        let mut pixels = blank();
        for px in pixels.chunks_exact_mut(4) {
            px.copy_from_slice(&[0, 0, 0, 255]);
        }
        begin();
        text(
            &mut Canvas(&mut pixels),
            &kit,
            font,
            "HHHH",
            (20, 20),
            None,
            None,
        );
        let layer = finish().unwrap();
        let out = composite(&pixels, &layer, 2);
        assert_eq!(out.len(), 1280 * 960 * 4);
        let lit = out.chunks_exact(4).filter(|px| px[0] > 100).count();
        assert!(lit > 100, "{lit} lit pixels");
        // Far from the text the canvas shows as it was.
        assert_eq!(out[((900 * 1280) + 1200) * 4..][..3], [0, 0, 0]);
    }

    /// Writes 3 times size PPM renders of the multiplayer screens with their
    /// text drawn sharp (the CPU stand-in for what the GPU draws) from the
    /// imported pieces, beside the canvas as the game stretched it before:
    ///
    /// ```text
    /// TORE_DATA_DIR=... TORE_MOCK_OUT=out-dir cargo test -p tore-app --locked \
    ///     ui_text::tests::render_review_pictures -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "needs an imported data profile (TORE_DATA_DIR) and TORE_MOCK_OUT"]
    fn render_review_pictures() {
        use crate::direct_screen::preview as direct;
        use crate::lobby_screen::preview as lobby;
        use crate::widgets::KitSource;
        let dir = crate::assets::data_directory().expect("data directory");
        let assets = crate::assets::Assets::load(&dir).expect("an imported pack");
        let source = KitSource::of(&assets.pics, &assets.multiplayer_resources);
        let out =
            std::path::PathBuf::from(std::env::var_os("TORE_MOCK_OUT").expect("TORE_MOCK_OUT"));
        let write = |name: &str, rgba: &[u8], width: usize, height: usize| {
            let mut ppm = format!("P6\n{width} {height}\n255\n").into_bytes();
            for px in rgba.chunks_exact(4) {
                ppm.extend_from_slice(&px[..3]);
            }
            std::fs::write(out.join(name), ppm).unwrap();
        };
        let states: Vec<(&str, bool)> = direct::STATES
            .iter()
            .map(|s| (*s, true))
            .chain(lobby::STATES.iter().map(|s| (*s, false)))
            .collect();
        for (state, is_direct) in states {
            let mut pixels = vec![0u8; 640 * 480 * 4];
            begin();
            if is_direct {
                direct::render(&source, state, &mut pixels).expect("render");
            } else {
                lobby::render(&source, state, &mut pixels).expect("render");
            }
            let layer = finish().expect("a layer");
            write(
                &format!("{state}-sharp.ppm"),
                &composite(&pixels, &layer, 3),
                1920,
                1440,
            );
            let mut plain = vec![0u8; 640 * 480 * 4];
            if is_direct {
                direct::render(&source, state, &mut plain).expect("render");
            } else {
                lobby::render(&source, state, &mut plain).expect("render");
            }
            write(
                &format!("{state}-old.ppm"),
                &composite(&plain, &Layer::default(), 3),
                1920,
                1440,
            );
        }
    }
}
