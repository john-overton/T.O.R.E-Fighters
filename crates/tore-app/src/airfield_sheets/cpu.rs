//! CPU diagnostic views of the actual terrain and static scenery buffers.
//! No GPU lighting, city-fill instances, weather or moving objects are drawn.
use crate::{
    AppResult,
    scenery::Scenery,
    terrain::{Overrides, Terrain},
};
use std::{collections::BTreeSet, path::Path};
const W: usize = 1600;
const H: usize = 1000;

pub(super) fn run(out: &Path) -> AppResult<()> {
    std::fs::create_dir_all(out)?;
    let resources = crate::reel::load_assets()?.theater_resources;
    for (code, airport) in [("FRA", 14), ("VLA", 6)] {
        let world = Terrain::for_mission(
            &resources,
            code,
            Some(0),
            &Overrides {
                time: Some([12, 0]),
                cloud_altitude: Some(0),
                city_fill: false,
                ..Default::default()
            },
        )?;
        let r = world
            .airport_scene
            .airport(airport)
            .and_then(|a| a.runway_objects.first())
            .and_then(|id| world.airport_scene.runway(*id))
            .ok_or("missing render field")?;
        let center = r.approach_center;
        let heading = r.heading;
        let mut identity = crate::replay::identity::of(&world);
        identity.airfield_scene = crate::replay::identity::REDRAWN_TEMPLATES_AIRFIELD_SCENE;
        let before = crate::replay::identity::terrain(&resources, &identity)?;
        for (label, terrain) in [("before", &before), ("after", &world)] {
            let mut scene = Scenery::build(&resources, terrain)?;
            let geometry = std::sync::Arc::clone(scene.static_geometry_where(&BTreeSet::new()));
            for (view, pitch, width) in [("top", 90f64, 48000f64), ("approach", 12., 30000.)] {
                let pixels = render(
                    &scene,
                    &geometry.vertices,
                    center,
                    heading,
                    pitch.to_radians(),
                    width,
                );
                let path = out.join(format!("{code}-{airport}-{view}-{label}-cpu.png"));
                std::fs::write(
                    &path,
                    crate::replay::png::encode_rgba(W as u32, H as u32, &pixels)?,
                )?;
                println!("airfield CPU sheet: {}", path.display());
            }
        }
    }
    Ok(())
}

fn render(
    scene: &Scenery,
    objects: &[f32],
    center: [f64; 3],
    heading: f64,
    pitch: f64,
    width: f64,
) -> Vec<u8> {
    let (s, c) = heading.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let right = [c, 0., -s];
    let up = [s * sp, cp, c * sp];
    let forward = [-s * cp, sp, -c * cp];
    let dot = |a: [f64; 3], b: [f64; 3]| a.into_iter().zip(b).map(|(x, y)| x * y).sum::<f64>();
    let scale = W as f64 / width;
    let mut pixels = vec![0u8; W * H * 4];
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[145, 173, 190, 255]);
    }
    let mut depth = vec![f64::NEG_INFINITY; W * H];
    for stream in [&scene.vertices[..], objects] {
        for tri in stream.chunks_exact(30) {
            let vertices: [&[f32]; 3] = std::array::from_fn(|i| &tri[i * 10..i * 10 + 10]);
            let points: [[f64; 3]; 3] = vertices.map(|v| {
                let d = std::array::from_fn(|i| f64::from(v[i]) - center[i]);
                [
                    W as f64 * 0.5 + dot(d, right) * scale,
                    H as f64 * 0.5 - dot(d, up) * scale,
                    dot(d, forward),
                ]
            });
            let lo: [usize; 2] = std::array::from_fn(|a| {
                points
                    .iter()
                    .map(|p| p[a])
                    .fold(f64::INFINITY, f64::min)
                    .floor()
                    .max(0.) as usize
            });
            let hi: [usize; 2] = std::array::from_fn(|a| {
                points
                    .iter()
                    .map(|p| p[a])
                    .fold(f64::NEG_INFINITY, f64::max)
                    .ceil()
                    .max(0.)
                    .min(if a == 0 { W - 1 } else { H - 1 } as f64) as usize
            });
            if lo[0] > hi[0] || lo[1] > hi[1] {
                continue;
            }
            let [a, b, c] = points;
            let denom = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
            if denom.abs() < 1e-8 {
                continue;
            }
            for y in lo[1]..=hi[1] {
                for x in lo[0]..=hi[0] {
                    let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
                    let wa = ((b[1] - c[1]) * (px - c[0]) + (c[0] - b[0]) * (py - c[1])) / denom;
                    let wb = ((c[1] - a[1]) * (px - c[0]) + (a[0] - c[0]) * (py - c[1])) / denom;
                    let wc = 1. - wa - wb;
                    if wa < 0. || wb < 0. || wc < 0. {
                        continue;
                    }
                    let z = wa * a[2] + wb * b[2] + wc * c[2];
                    let at = y * W + x;
                    if z < depth[at] {
                        continue;
                    }
                    let interpolate = |k| {
                        wa * f64::from(vertices[0][k])
                            + wb * f64::from(vertices[1][k])
                            + wc * f64::from(vertices[2][k])
                    };
                    let layer = vertices[0][5];
                    let index = if layer >= 0. {
                        let u = (interpolate(3) * 256.).floor().clamp(0., 255.) as usize;
                        let v = (interpolate(4) * 256.).floor().clamp(0., 255.) as usize;
                        let at = layer as usize * 65536 + v * 256 + u;
                        if at < scene.texture_indices.len() {
                            scene.texture_indices[at]
                        } else {
                            *scene
                                .sky_indices
                                .get(at - scene.texture_indices.len())
                                .unwrap_or(&255)
                        }
                    } else {
                        (vertices[0][9] as u32 & 255) as u8
                    };
                    if index == 255 {
                        continue;
                    }
                    pixels[at * 4..at * 4 + 3].copy_from_slice(&scene.palette[index as usize]);
                    depth[at] = z;
                }
            }
        }
    }
    pixels
}
