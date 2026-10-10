//! Experiment AP1: the drawn pavement of a redrawn airport
//! (`terrain::redrawn`), tiled with retail runway art at real-world size.
//! Each patch is cut at its texture grid and every cell maps to one copy of
//! its material's texel rectangle, so the art repeats instead of stretching.
use crate::{AppResult, terrain::redrawn::Built};
use std::collections::BTreeMap;
use tore_formats::Pic;

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
    let mut out = Vec::new();
    for patch in &built.patches {
        let material = &built.materials[patch.material];
        let name = material.pic.to_ascii_uppercase();
        if !static_layers.contains_key(&name) {
            let pic = Pic::parse(
                resources
                    .get(&name)
                    .ok_or_else(|| format!("redrawn airport: missing texture {name}"))?,
            )?;
            let first = first_page(pages);
            let image = crate::static_art::Image::append(&pic, pages, first)?;
            static_layers.insert(name.clone(), image);
        }
        let image = static_layers[&name];
        let [x, y, w, h] = material.rect;
        if x + w > image.width as f64 || y + h > image.height as f64 {
            return Err(format!("redrawn airport: {} lies outside {name}", material.name).into());
        }
        for (positions, fractions) in patch.cells() {
            // Half a texel in from the rectangle's edge, so a copy never
            // samples its neighbour in the atlas.
            let texel = |f: [f64; 2]| {
                [
                    (x + 0.5 + f[0] * (w - 1.)) as f32,
                    (y + 0.5 + f[1] * (h - 1.)) as f32,
                ]
            };
            let corner = |i: usize| {
                let p = built.frame.world(positions[i]);
                let uv = texel(fractions[i]);
                [
                    p[0] as f32,
                    p[1] as f32,
                    p[2] as f32,
                    uv[0],
                    uv[1],
                    -1.,
                    0.,
                    0.,
                    0.,
                    color,
                ]
            };
            for [a, b, c] in [[0, 1, 2], [0, 2, 3]] {
                image.triangle([corner(a), corner(b), corner(c)], &mut out, limit)?;
            }
        }
    }
    Ok(out)
}
