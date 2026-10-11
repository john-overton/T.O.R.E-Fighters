//! Experiment AP1: the drawn pavement of a redrawn airport
//! (`terrain::redrawn`), tiled with retail runway art at real-world size.
//!
//! Each patch is cut at its texture grid and every cell maps to one copy of
//! its material's texel rectangle, so the art repeats instead of stretching.
//! Apron copies pick one of their material's rectangles and mirror it from a
//! fixed hash of where they lie, so the slabs do not repeat in step. Some
//! materials are drawn from a runtime copy of their retail texels with
//! markings painted in: white runway edge stripes, centreline dashes and
//! threshold bars, yellow taxiway centrelines and edges, and the runway
//! number boards with the board painted over in the runway's own asphalt.
//! The markings are this file's own art (`fitted`, agent, 2026-10-10; John
//! allowed simple non-retail lines in the retail palette): nothing retail is
//! stored, the copies are made from the player's import at load.
use crate::{
    AppResult,
    terrain::redrawn::{Along, Built, Material, Paint},
};
use std::collections::BTreeMap;
use tore_formats::Pic;

/// Retail palette entries nearest to the marking colours.
struct Inks {
    white: u8,
    yellow: u8,
    /// Perceived brightness of every palette entry, 0 to 255.
    brightness: [u8; 256],
}

impl Inks {
    fn from(resources: &BTreeMap<String, Vec<u8>>) -> AppResult<Self> {
        let raw = resources
            .get("PALETTE.PAL")
            .filter(|raw| raw.len() >= 768)
            .ok_or("redrawn airport: missing PALETTE.PAL")?;
        let colour = |i: usize| -> [i32; 3] {
            std::array::from_fn(|c| (i32::from(raw[i * 3 + c]) * 255 + 31) / 63)
        };
        // Entry 255 is the see-through index.
        let nearest = |target: [i32; 3]| {
            (0..255)
                .min_by_key(|i| {
                    let c = colour(*i);
                    (0..3).map(|k| (c[k] - target[k]).pow(2)).sum::<i32>()
                })
                .expect("palette") as u8
        };
        let brightness = std::array::from_fn(|i| {
            let c = colour(i);
            ((c[0] * 3 + c[1] * 6 + c[2]) / 10).clamp(0, 255) as u8
        });
        Ok(Self {
            white: nearest([235, 235, 230]),
            yellow: nearest([225, 185, 40]),
            brightness,
        })
    }
}

/// Where a material's texels are drawn from.
struct Source {
    image: crate::static_art::Image,
    rects: Vec<[f64; 4]>,
    /// Mirror the texels across u and v (a fillet turned to put its paved
    /// corner at the cell's far corner).
    flip: [bool; 2],
    vary: bool,
}

fn load<'p>(
    pics: &'p mut BTreeMap<String, Pic>,
    resources: &BTreeMap<String, Vec<u8>>,
    name: &str,
) -> AppResult<&'p Pic> {
    let key = name.to_ascii_uppercase();
    if !pics.contains_key(&key) {
        let parsed = Pic::parse(
            resources
                .get(&key)
                .ok_or_else(|| format!("redrawn airport: missing texture {key}"))?,
        )?;
        pics.insert(key.clone(), parsed);
    }
    Ok(&pics[&key])
}

fn crop(pic: &Pic, rect: [f64; 4]) -> Pic {
    let [x, y, w, h] = rect.map(|v| v as usize);
    let mut pixels = Vec::with_capacity(w * h);
    let mut mask = Vec::with_capacity(w * h);
    for row in y..y + h {
        for col in x..x + w {
            let at = row * pic.width + col;
            pixels.push(pic.pixels[at]);
            mask.push(pic.mask[at]);
        }
    }
    Pic {
        width: w,
        height: h,
        pixels,
        mask,
        palette: Vec::new(),
        glyphs: Vec::new(),
    }
}

/// Paints the material's markings into a copy of its texels.
fn paint(copy: &mut Pic, material: &Material, inks: &Inks, plain: Option<&Pic>) {
    let (w, h) = (copy.width, copy.height);
    let set = |copy: &mut Pic, col: usize, row: usize, ink: u8| {
        if col < w && row < h {
            copy.pixels[row * w + col] = ink;
            copy.mask[row * w + col] = true;
        }
    };
    for mark in &material.paint {
        match mark {
            Paint::Edges => {
                for row in 0..h {
                    for col in [1, 2, w - 3, w - 2] {
                        set(copy, col, row, inks.white);
                    }
                }
            }
            Paint::Centre => {
                for row in 0..h / 2 {
                    for col in [w / 2 - 1, w / 2] {
                        set(copy, col, row, inks.white);
                    }
                }
            }
            Paint::Threshold => {
                // Eight bars each side of the centreline: seventeen slots.
                let slots = 17;
                for col in 0..w {
                    let slot = col * slots / w;
                    if slot % 2 == 1 && slot != slots / 2 {
                        for row in 3..h.saturating_sub(3) {
                            set(copy, col, row, inks.white);
                        }
                    }
                }
            }
            Paint::Taxiway | Paint::TaxiwayCentre => {
                let across = if material.along == Along::U { h } else { w };
                let lines = if *mark == Paint::Taxiway {
                    vec![1, across / 2, across - 2]
                } else {
                    vec![across / 2]
                };
                for line in lines {
                    if material.along == Along::U {
                        for col in 0..w {
                            set(copy, col, line, inks.yellow);
                        }
                    } else {
                        for row in 0..h {
                            set(copy, line, row, inks.yellow);
                        }
                    }
                }
            }
            Paint::Digit => {
                // Everything but the white figure becomes plain asphalt.
                let Some(plain) = plain else { continue };
                for row in 0..h {
                    for col in 0..w {
                        let at = row * w + col;
                        if inks.brightness[usize::from(copy.pixels[at])] >= 170 {
                            copy.pixels[at] = inks.white;
                            continue;
                        }
                        let px = (col * plain.width / w).min(plain.width - 1);
                        let py = (row * plain.height / h).min(plain.height - 1);
                        copy.pixels[at] = plain.pixels[py * plain.width + px];
                        copy.mask[at] = true;
                    }
                }
            }
        }
    }
}

/// Which corner of a fillet's texels is paved: the mirroring that puts it
/// at the cell's far corner (fractions 1, 1).
fn fillet_flip(pic: &Pic, rect: [f64; 4]) -> [bool; 2] {
    let [x, y, w, h] = rect.map(|v| v as usize);
    let paved = |col: usize, row: usize| {
        let at = row * pic.width + col;
        pic.mask[at] && pic.pixels[at] != 255
    };
    [
        ([true, true], paved(x + 1, y + 1)),
        ([false, true], paved(x + w - 2, y + 1)),
        ([true, false], paved(x + 1, y + h - 2)),
        ([false, false], paved(x + w - 2, y + h - 2)),
    ]
    .iter()
    .find(|(_, solid)| *solid)
    .map_or([false, false], |(flip, _)| *flip)
}

fn hash(parts: &[u64]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for part in parts {
        for byte in part.to_le_bytes() {
            h ^= u64::from(byte);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    }
    h
}

/// The triangles of `built`'s pavement, in world feet, with the vertex
/// layout of the static scene. `color` is the palette and fog word the
/// retail runway's own textured faces carry. Images are loaded into
/// `static_layers` and `pages` as the retail faces' are.
pub fn vertices(
    built: &Built,
    resources: &BTreeMap<String, Vec<u8>>,
    static_layers: &mut BTreeMap<String, crate::static_art::Image>,
    pages: &mut Vec<u8>,
    first_page: impl Fn(&Vec<u8>) -> usize,
    color: f32,
    limit: usize,
) -> AppResult<Vec<f32>> {
    if built.patches.is_empty() {
        return Ok(Vec::new());
    }
    let inks = Inks::from(resources)?;
    let mut pics = BTreeMap::new();
    let plain = match built.materials.iter().find(|m| m.name == "runway_plain") {
        Some(m) => Some(crop(load(&mut pics, resources, &m.pic)?, m.rect)),
        None => None,
    };
    let mut sources = Vec::new();
    for material in &built.materials {
        let whole = load(&mut pics, resources, &material.pic)?;
        for rect in std::iter::once(&material.rect).chain(&material.variants) {
            if rect[0] + rect[2] > whole.width as f64 || rect[1] + rect[3] > whole.height as f64 {
                return Err(format!(
                    "redrawn airport: {} lies outside {}",
                    material.name, material.pic
                )
                .into());
            }
        }
        let flip = if material.name == "fillet" {
            fillet_flip(whole, material.rect)
        } else {
            [false, false]
        };
        let source = if material.paint.is_empty() {
            let name = material.pic.to_ascii_uppercase();
            if !static_layers.contains_key(&name) {
                let first = first_page(pages);
                let image = crate::static_art::Image::append(whole, pages, first)?;
                static_layers.insert(name.clone(), image);
            }
            Source {
                image: static_layers[&name],
                rects: std::iter::once(material.rect)
                    .chain(material.variants.iter().copied())
                    .collect(),
                flip,
                vary: !material.variants.is_empty(),
            }
        } else {
            let key = format!(
                "{}:{:?}:{:?}:{:?}",
                material.pic, material.rect, material.paint, material.along
            );
            if !static_layers.contains_key(&key) {
                let mut copy = crop(whole, material.rect);
                paint(&mut copy, material, &inks, plain.as_ref());
                let first = first_page(pages);
                let image = crate::static_art::Image::append(&copy, pages, first)?;
                static_layers.insert(key.clone(), image);
            }
            let [_, _, w, h] = material.rect;
            Source {
                image: static_layers[&key],
                rects: vec![[0., 0., w, h]],
                flip,
                vary: false,
            }
        };
        sources.push(source);
    }
    let mut out = Vec::new();
    for (index, patch) in built.patches.iter().enumerate() {
        let source = &sources[patch.material];
        for (positions, fractions, [i, j]) in patch.cells() {
            let pick = if source.vary {
                hash(&[patch.material as u64, i as u64, j as u64, index as u64])
            } else {
                0
            };
            let [x, y, w, h] = source.rects[(pick % source.rects.len() as u64) as usize];
            let flip = [
                source.flip[0] ^ (source.vary && pick & 0x100 != 0),
                source.flip[1] ^ (source.vary && pick & 0x200 != 0),
            ];
            let corner = |k: usize| {
                let p = built.frame.world(positions[k]);
                let mut f = fractions[k];
                for axis in 0..2 {
                    if flip[axis] {
                        f[axis] = 1. - f[axis];
                    }
                }
                // Half a texel in from the rectangle's edge, so a copy never
                // samples its neighbour in the atlas.
                let u = x + 0.5 + f[0] * (w - 1.);
                let v = y + 0.5 + f[1] * (h - 1.);
                [
                    p[0] as f32,
                    p[1] as f32,
                    p[2] as f32,
                    u as f32,
                    v as f32,
                    -1.,
                    0.,
                    0.,
                    0.,
                    color,
                ]
            };
            for k in 1..positions.len() - 1 {
                source
                    .image
                    .triangle([corner(0), corner(k), corner(k + 1)], &mut out, limit)?;
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pic(width: usize, height: usize, fill: u8) -> Pic {
        Pic {
            width,
            height,
            pixels: vec![fill; width * height],
            mask: vec![true; width * height],
            palette: Vec::new(),
            glyphs: Vec::new(),
        }
    }

    fn material(paint: Vec<Paint>, along: Along) -> Material {
        Material {
            name: "m".into(),
            pic: "M.PIC".into(),
            rect: [0., 0., 10., 12.],
            tile_ft: [0., 100.],
            along,
            paint,
            variants: Vec::new(),
        }
    }

    fn inks() -> Inks {
        let mut brightness = [0; 256];
        brightness[7] = 250;
        Inks {
            white: 7,
            yellow: 9,
            brightness,
        }
    }

    #[test]
    fn markings_paint_lines_where_they_belong() {
        let mut copy = pic(10, 12, 1);
        paint(
            &mut copy,
            &material(vec![Paint::Edges, Paint::Centre], Along::V),
            &inks(),
            None,
        );
        assert_eq!(copy.pixels[1], 7, "edge stripe");
        assert_eq!(copy.pixels[8], 7, "far edge stripe");
        assert_eq!(copy.pixels[5], 7, "centreline dash");
        assert_eq!(copy.pixels[11 * 10 + 5], 1, "the dash covers half a copy");
        let mut taxiway = pic(10, 12, 1);
        paint(
            &mut taxiway,
            &material(vec![Paint::Taxiway], Along::U),
            &inks(),
            None,
        );
        for row in [1, 6, 10] {
            assert_eq!(taxiway.pixels[row * 10 + 3], 9, "row {row}");
        }
        assert_eq!(taxiway.pixels[3 * 10 + 3], 1);
    }

    #[test]
    fn a_number_board_becomes_asphalt_around_its_figure() {
        let mut digit = pic(10, 12, 2);
        digit.pixels[5 * 10 + 5] = 7;
        let plain = pic(4, 4, 3);
        paint(
            &mut digit,
            &material(vec![Paint::Digit], Along::V),
            &inks(),
            Some(&plain),
        );
        assert_eq!(digit.pixels[5 * 10 + 5], 7);
        assert!(
            digit
                .pixels
                .iter()
                .enumerate()
                .all(|(i, p)| i == 55 || *p == 3)
        );
    }

    #[test]
    fn a_fillet_turns_its_paved_corner_to_the_far_corner() {
        let mut fillet = pic(8, 8, 255);
        fillet.pixels[0] = 4;
        fillet.pixels[9] = 4;
        assert_eq!(fillet_flip(&fillet, [0., 0., 8., 8.]), [true, true]);
        let mut fillet = pic(8, 8, 255);
        fillet.pixels[6 * 8 + 6] = 4;
        assert_eq!(fillet_flip(&fillet, [0., 0., 8., 8.]), [false, false]);
    }
}
