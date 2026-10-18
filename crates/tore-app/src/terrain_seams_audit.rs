//! CPU-only mask audit and reproducible cameras for the lead's GPU review.
use super::{BLEND, HEDGE, Map};
use crate::{
    AppResult,
    scenery::Scenery,
    terrain::{Overrides, Terrain},
};
use std::{collections::BTreeMap, io::Write, path::Path, time::Instant};

#[derive(Clone, Copy)]
pub struct Spot {
    pub x: f64,
    pub z: f64,
    pub axis: usize,
    pub flag: u8,
    pub length: usize,
}

pub fn camera(world: &Terrain, spot: &Spot, height: f64) -> crate::camera::Camera {
    let distance = height / 30_f64.to_radians().tan();
    let mut eye = [spot.x, spot.z];
    eye[spot.axis] -= distance;
    let y = f64::from(world.height(eye[0] as f32, eye[1] as f32)) + height;
    let target = f64::from(world.height(spot.x as f32, spot.z as f32));
    let mut camera = crate::camera::Camera::new();
    camera.position = [eye[0], y, eye[1]];
    camera.yaw = if spot.axis == 0 {
        std::f32::consts::FRAC_PI_2
    } else {
        0.
    };
    camera.pitch = (target - y).atan2(distance) as f32;
    camera
}

pub fn spots(map: &Map, flag: u8) -> Vec<Spot> {
    let mut all = Vec::new();
    for tile in 0..map.cols * map.rows {
        for axis in 0..2 {
            let mask: Vec<_> = (0..256).map(|s| map.record(tile, axis, s)[0]).collect();
            let mut start = 0;
            while start < 256 {
                if mask[start] & flag == 0 {
                    start += 1;
                    continue;
                }
                let end = (start + 1..256)
                    .find(|&s| mask[s] & flag == 0)
                    .unwrap_or(256);
                let mut p = [
                    (tile % map.cols) as f64 * 32768.,
                    (tile / map.cols) as f64 * 32768.,
                ];
                p[axis] += 32768.;
                p[1 - axis] += (start + end) as f64 * 64.;
                all.push(Spot {
                    x: p[0],
                    z: p[1],
                    axis,
                    flag,
                    length: end - start,
                });
                start = end;
            }
        }
    }
    all.sort_by_key(|s| std::cmp::Reverse(s.length));
    let mut chosen: Vec<Spot> = Vec::new();
    for s in all {
        if chosen.iter().all(|p| (p.x - s.x).hypot(p.z - s.z) > 65536.) {
            chosen.push(s);
        }
        if chosen.len() == 3 {
            break;
        }
    }
    chosen
}

pub fn run(resources: &BTreeMap<String, Vec<u8>>, out: &Path) -> AppResult<()> {
    let mut cameras = std::fs::File::create(out.join("seam-cameras.txt"))?;
    let mut report = std::fs::File::create(out.join("seam-metrics.tsv"))?;
    writeln!(
        report,
        "theater\tcols\trows\tbytes\tblend_segments\thedge_segments\tcold_scene_ms\tcached_scene_ms\tmean_fit_max_linear\tlocal_mean_fit_max_linear"
    )?;
    for code in ["UKR", "FRA", "BAL"] {
        let world = Terrain::for_mission(resources, code, Some(0), &Overrides::default())?;
        let timer = Instant::now();
        let scene = Scenery::build(resources, &world)?;
        let cold = timer.elapsed().as_secs_f64() * 1000.;
        let atlas = scene.vegetation.as_ref().ok_or("vegetation disabled")?;
        let map = &atlas.seams;
        let timer = Instant::now();
        let cached = Scenery::build(resources, &world)?;
        let warm = timer.elapsed().as_secs_f64() * 1000.;
        if !cached.vegetation_cache_hit
            || scene.seams != cached.seams
            || map.as_ref()
                != cached
                    .vegetation
                    .as_ref()
                    .ok_or("cached atlas missing")?
                    .seams
                    .as_ref()
            || scene.sky_indices != cached.sky_indices
        {
            return Err("seam/card/canopy cache differs".into());
        }
        crate::vegetation_limits::validate(&scene, &wgpu::Limits::downlevel_defaults())?;
        let mut counts = [0; 2];
        let mut edges = std::fs::File::create(out.join(format!("{code}-edges.tsv")))?;
        writeln!(edges, "tile_col\ttile_row\taxis\tsegment\tflags")?;
        for tile in 0..map.cols * map.rows {
            for axis in 0..2 {
                for segment in 0..256 {
                    let flag = map.record(tile, axis, segment)[0];
                    counts[0] += usize::from(flag & BLEND != 0);
                    counts[1] += usize::from(flag & HEDGE != 0);
                    if flag != 0 {
                        writeln!(
                            edges,
                            "{}\t{}\t{axis}\t{segment}\t{flag}",
                            tile % map.cols * 4,
                            tile / map.cols * 4
                        )?;
                    }
                }
            }
        }
        let fit = scene
            .texture_indices
            .chunks_exact(65536)
            .map(|p| super::mean(p, &scene.palette).2)
            .fold(0., f32::max);
        writeln!(
            report,
            "{code}\t{}\t{}\t{}\t{}\t{}\t{cold:.3}\t{warm:.3}\t{fit:.7}\t{:.7}",
            map.cols,
            map.rows,
            map.bytes.len(),
            counts[0],
            counts[1],
            map.mean_error
        )?;
        let found = spots(map, BLEND);
        if code != "BAL" && found.len() < 3 {
            return Err(format!("{code}: fewer than three separated colour seam cameras").into());
        }
        let mut selected = found.clone();
        selected.extend(spots(map, HEDGE).into_iter().take(1));
        for (i, spot) in selected.iter().enumerate() {
            let label = if spot.flag == HEDGE { "hedge" } else { "blend" };
            for height in [500_f64, 2000., 10000.] {
                let cam = camera(&world, spot, height);
                let [x, y, z] = cam.position;
                writeln!(
                    cameras,
                    "{code}-seam-{label}-{i}-{height:.0}ft {code} 0 12 0 {x:.3} {y:.3} {z:.3} {:.6} {:.6}",
                    cam.yaw.to_degrees(),
                    cam.pitch.to_degrees()
                )?;
            }
            strip(out, code, i, spot, &world, &scene, map)?;
        }
        let spot = *selected.first().ok_or("no seam or hedge control camera")?;
        let y = f64::from(world.height(spot.x as f32, spot.z as f32)) + 20000.;
        writeln!(
            cameras,
            "{code}-seams-overview-20000ft {code} 0 12 0 {:.3} {y:.3} {:.3} 0 -89.9",
            spot.x, spot.z
        )?;
        println!(
            "terrain seams: PASS {code} blend={} hedge={} bytes={} cold_ms={cold:.3} cached_ms={warm:.3} exact cache and portable limits",
            counts[0],
            counts[1],
            map.bytes.len()
        );
    }
    Ok(())
}

/// Unlit reference strips: upper half source, lower half mean correction. Art
/// and its high-frequency residual stay at their own UVs on both sides.
fn strip(
    out: &Path,
    code: &str,
    i: usize,
    spot: &Spot,
    world: &Terrain,
    scene: &Scenery,
    map: &Map,
) -> AppResult<()> {
    const W: usize = 768;
    const H: usize = 256;
    let palette = scene.palette.map(|p| p.map(super::linear));
    let mut image = vec![255; W * H * 4];
    for y in 0..H {
        for x in 0..W {
            let mut p = [spot.x, spot.z];
            p[spot.axis] += (x as f64 + 0.5 - W as f64 / 2.) * 2.;
            p[1 - spot.axis] += ((y % (H / 2)) as f64 - H as f64 / 4.) * 8.;
            let (cx, cz) = ((p[0] / 8192.) as usize, (p[1] / 8192.) as usize);
            let key = ((cx & !3) as i32, (cz & !3) as i32);
            let layer = scene
                .ground_layers
                .get(&key)
                .copied()
                .or(scene.land_texture)
                .ok_or("no tile for strip")?;
            let rotation = world
                .environment
                .textures
                .get(&key)
                .map_or(0, |p| p.rotation);
            let (u, v) = crate::vegetation::rotate(
                (p[0] % 32768. / 128.) as usize,
                (p[1] % 32768. / 128.) as usize,
                rotation,
            );
            let index = scene.texture_indices[layer * 65536 + v * 256 + u];
            let mut color = palette[usize::from(index)];
            if y >= H / 2 {
                let cell = [cx / 4, cz / 4];
                let tile = cell[1] * map.cols + cell[0];
                let axis = spot.axis;
                let local = p[axis] as f32 % 32768.;
                let negative = local < 16384.;
                let step = if axis == 0 { 1 } else { map.cols };
                let other = if negative { tile - step } else { tile + step };
                let owner = if negative { other } else { tile };
                let strength = map.edge_strength(owner, axis, p[1 - axis] as f32 % 32768., BLEND);
                let weight = super::weight(local.min(32768. - local)) * strength;
                let delta = map.edge_delta(owner, axis, p[1 - axis] as f32 % 32768., &palette);
                let sign = if negative { -1. } else { 1. };
                color = std::array::from_fn(|k| (color[k] + sign * delta[k] * weight).max(0.));
            }
            let rgb = color.map(|v| {
                let v = if v <= 0.0031308 {
                    v * 12.92
                } else {
                    1.055 * v.powf(1. / 2.4) - 0.055
                };
                (v.clamp(0., 1.) * 255.).round() as u8
            });
            image[(y * W + x) * 4..(y * W + x) * 4 + 3].copy_from_slice(&rgb);
        }
    }
    std::fs::write(
        out.join(format!("{code}-seam-{i}-unlit.png")),
        crate::replay::png::encode_rgba(W as u32, H as u32, &image)?,
    )?;
    Ok(())
}
