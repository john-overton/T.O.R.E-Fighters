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
    let target = [
        spot.x,
        f64::from(world.height(spot.x as f32, spot.z as f32)),
        spot.z,
    ];
    for angle in [45_f64, 60., 75.] {
        for sign in [1., -1.] {
            let distance = height / angle.to_radians().tan();
            let mut eye = [spot.x, spot.z];
            eye[spot.axis] -= sign * distance;
            let mut camera = crate::camera::Camera::new();
            camera.position = [
                eye[0],
                f64::from(world.height(eye[0] as f32, eye[1] as f32)) + height,
                eye[1],
            ];
            camera.yaw = if spot.axis == 0 {
                (sign * std::f64::consts::FRAC_PI_2) as f32
            } else if sign > 0. {
                0.
            } else {
                std::f32::consts::PI
            };
            camera.pitch = (target[1] - camera.position[1]).atan2(distance) as f32;
            if verify_camera(world, &camera, target).is_ok() {
                return camera;
            }
        }
    }
    let mut camera = crate::camera::Camera::new();
    camera.position = [target[0], target[1] + height, target[2]];
    camera.yaw = 0.;
    camera.pitch = -std::f32::consts::FRAC_PI_2;
    camera
}

fn verify_camera(
    world: &Terrain,
    camera: &crate::camera::Camera,
    target: [f64; 3],
) -> AppResult<[f64; 2]> {
    if camera.axes().2[1] > -35_f64.to_radians().sin() {
        return Err("seam camera must look down at least 35 degrees".into());
    }
    let pixel = camera
        .project([3840, 2160], target)
        .ok_or("seam is outside the camera frame")?;
    if (pixel[0] - 1920.).hypot(pixel[1] - 1080.) > 0.1 {
        return Err("seam is not centred".into());
    }
    for i in 1..128 {
        let t = f64::from(i) / 128.;
        let p: [f64; 3] =
            std::array::from_fn(|k| camera.position[k] + (target[k] - camera.position[k]) * t);
        if p[1] < f64::from(world.height(p[0] as f32, p[2] as f32)) + 0.25 {
            return Err("terrain hides the chosen seam".into());
        }
    }
    Ok(pixel)
}

fn write_camera(
    cameras: &mut std::fs::File,
    checks: &mut std::fs::File,
    name: &str,
    code: &str,
    camera: &crate::camera::Camera,
    target: [f64; 3],
    world: &Terrain,
) -> AppResult<()> {
    let [x, y, z] = camera.position;
    let line = format!(
        "{name} {code} 0 12 0 {x:.6} {y:.6} {z:.6} {:.9} {:.9}",
        camera.yaw, camera.pitch
    );
    let decoded = crate::vegetation_preview::preview_camera(&line)?;
    let pixel = verify_camera(world, &decoded, target)?;
    writeln!(cameras, "{line}")?;
    writeln!(
        checks,
        "{name}\t{:.6}\t{:.6}\t{:.6}\t{:.6}\tclear",
        decoded.pitch,
        decoded.position[1] - f64::from(world.height(x as f32, z as f32)),
        pixel[0],
        pixel[1]
    )?;
    Ok(())
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
    let mut cost_cameras = std::fs::File::create(out.join("seam-cost-cameras.txt"))?;
    let mut checks = std::fs::File::create(out.join("camera-check.tsv"))?;
    writeln!(
        checks,
        "camera\tpitch_radians\tagl_ft\ttarget_pixel_x\ttarget_pixel_y\tterrain_ray"
    )?;
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
        if code == "UKR" {
            line_probe(out, &world, &scene)?;
        }
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
        let mut found = spots(map, super::FALLBACK);
        if code == "UKR" {
            let primary = Spot {
                x: 16384.,
                z: 32768.,
                axis: 1,
                flag: BLEND,
                length: 1,
            };
            if map.record(0, 1, 128)[0] & BLEND == 0 {
                return Err("reviewed UKR LAND seam is still excluded".into());
            }
            found.retain(|s| (s.x - primary.x).hypot(s.z - primary.z) > 65536.);
            found.insert(0, primary);
        }
        if found.len() < 3 {
            found.extend(spots(map, BLEND));
        }
        found.truncate(3);
        if code != "BAL" && found.len() < 3 {
            return Err(format!("{code}: fewer than three seam cameras").into());
        }
        let mut selected = found.clone();
        selected.extend(spots(map, HEDGE).into_iter().take(1));
        for (i, spot) in selected.iter().enumerate() {
            let label = if spot.flag == HEDGE { "hedge" } else { "blend" };
            for height in [500_f64, 2000., 10000.] {
                let cam = camera(&world, spot, height);
                let target = [
                    spot.x,
                    f64::from(world.height(spot.x as f32, spot.z as f32)),
                    spot.z,
                ];
                write_camera(
                    &mut cameras,
                    &mut checks,
                    &format!("{code}-seam-{label}-{i}-{height:.0}ft"),
                    code,
                    &cam,
                    target,
                    &world,
                )?;
            }
            strip(out, code, i, spot, &world, &scene, map)?;
        }
        let spot = *selected.first().ok_or("no seam or hedge control camera")?;
        let target = [
            spot.x,
            f64::from(world.height(spot.x as f32, spot.z as f32)),
            spot.z,
        ];
        let mut overview = crate::camera::Camera::new();
        overview.position = [spot.x, target[1] + 20000., spot.z];
        overview.yaw = 0.;
        overview.pitch = -std::f32::consts::FRAC_PI_2;
        write_camera(
            &mut cameras,
            &mut checks,
            &format!("{code}-seams-overview-20000ft"),
            code,
            &overview,
            target,
            &world,
        )?;
        for height in [100., 500., 2000., 10000.] {
            let cam = camera(&world, &spot, height);
            write_camera(
                &mut cost_cameras,
                &mut checks,
                &format!("{code}-seam-cost-{height:.0}ft"),
                code,
                &cam,
                target,
                &world,
            )?;
        }
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
    let packed = map.pack();
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
                let delta = packed.correction(map, p.map(|v| v as f32), &palette);
                color = std::array::from_fn(|k| (color[k] + delta[k]).max(0.));
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

pub fn review(resources: &BTreeMap<String, Vec<u8>>, out: &Path) -> AppResult<()> {
    let world = Terrain::for_mission(resources, "UKR", Some(0), &Overrides::default())?;
    let scene = Scenery::build(resources, &world)?;
    let atlas = scene.vegetation.as_ref().ok_or("no atlas")?;
    let mut file = std::fs::File::create(out.join("review-trace.txt"))?;
    let classes = tore_world::city::mask::Classes::from_palette(&atlas.palette);
    for x in [8192., 16384., 24576., 27456., 30000.] {
        let segment = (x / 128.) as usize;
        writeln!(
            file,
            "x={x} z=32768 mask={} edge={:?}",
            atlas.seams.record(0, 1, segment)[0],
            &atlas.seams.record(0, 1, segment)[1..]
        )?;
        for distance in [-320., -192., -64., 64., 192., 320.] {
            let z = 32768. + distance;
            let key = (((x / 32768.) as i32) * 4, ((z / 32768.) as i32) * 4);
            let place = world.environment.textures.get(&key);
            let page = scene
                .ground_layers
                .get(&key)
                .copied()
                .or(scene.land_texture)
                .unwrap();
            let (u, v) = crate::vegetation::rotate(
                (x % 32768. / 128.) as usize,
                (z % 32768. / 128.) as usize,
                place.map_or(0, |p| p.rotation),
            );
            let pixels = &scene.texture_indices[page * 65536..(page + 1) * 65536];
            let index = pixels[v * 256 + u];
            let cell = world
                .theater
                .cell((x / 8192.) as usize, (z / 8192.) as usize);
            writeln!(
                file,
                "  d={distance} pic={} index={index} rgb={:?} kind={:?} city={} T2={:?}",
                place.map_or("LAND.PIC".into(), |p| p.resource_name("UKR")),
                atlas.palette[usize::from(index)],
                atlas.kind_index(index),
                tore_world::city::mask::is_city_tile(pixels, &classes),
                cell
            )?;
        }
    }
    line_probe(out, &world, &scene)?;
    Ok(())
}

fn srgb(color: [f32; 3]) -> [f32; 3] {
    color.map(|v| {
        if v <= 0.0031308 {
            v * 12.92 * 255.
        } else {
            (1.055 * v.powf(1. / 2.4) - 0.055) * 255.
        }
    })
}

/// Same clamped bilinear indexed material sampling as sample_tile in WGSL.
fn ground_color(world: &Terrain, scene: &Scenery, p: [f64; 2]) -> AppResult<[f32; 3]> {
    let c = (p[0] / 8192.) as usize;
    let r = (p[1] / 8192.) as usize;
    let key = ((c & !3) as i32, (r & !3) as i32);
    let place = world.environment.textures.get(&key);
    if place.is_none() && world.theater.cell(c, r).color == 255 {
        return Err("line crosses untextured water".into());
    }
    let layer = scene
        .ground_layers
        .get(&key)
        .copied()
        .or(scene.land_texture)
        .ok_or("line has no ground art")?;
    let (u, v) = ((p[0] / 32768.).fract(), (p[1] / 32768.).fract());
    let uv = match place.map_or(0, |p| p.rotation) {
        1 => [1. - v, 1. - u],
        2 => [1. - u, v],
        3 => [v, u],
        _ => [u, 1. - v],
    };
    let texel = uv.map(|v| v * 256. - 0.5);
    let base = texel.map(f64::floor);
    let f = [texel[0] - base[0], texel[1] - base[1]];
    let mut rgb = [0.; 3];
    let mut coverage = 0.;
    for y in 0..2 {
        for x in 0..2 {
            let u = (base[0] as i32 + x).clamp(0, 255) as usize;
            let v = (base[1] as i32 + y).clamp(0, 255) as usize;
            let index = scene.texture_indices[layer * 65536 + v * 256 + u];
            if index == 255 {
                continue;
            }
            let w =
                (if x == 0 { 1. - f[0] } else { f[0] }) * (if y == 0 { 1. - f[1] } else { f[1] });
            for (sum, value) in rgb
                .iter_mut()
                .zip(scene.palette[usize::from(index)].map(super::linear))
            {
                *sum += value * w as f32;
            }
            coverage += w;
        }
    }
    if coverage < 0.5 {
        return Err("line crosses water cutout".into());
    }
    Ok(rgb.map(|v| v / coverage as f32))
}

fn line_probe(out: &Path, world: &Terrain, scene: &Scenery) -> AppResult<()> {
    let map = &scene.vegetation.as_ref().ok_or("no seam atlas")?.seams;
    let packed = map.pack();
    let palette = scene.palette.map(|p| p.map(super::linear));
    let point = |d: f64| [16384., 32768. + d];
    let sample = |d: f64| -> AppResult<([f32; 3], [f32; 3])> {
        let before = ground_color(world, scene, point(d))?;
        let correction = packed.correction(map, point(d).map(|v| v as f32), &palette);
        Ok((
            before,
            std::array::from_fn(|k| (before[k] + correction[k]).max(0.)),
        ))
    };
    if map.record(0, 1, 128)[0] & (BLEND | super::FALLBACK) != (BLEND | super::FALLBACK) {
        return Err("known LAND boundary is not enabled for colour correction".into());
    }
    let a = sample(-250.)?.0;
    let b = sample(250.)?.0;
    let mut csv = std::fs::File::create(out.join("UKR-LAND-line.csv"))?;
    writeln!(
        csv,
        "offset_ft,before_r,before_g,before_b,after_r,after_g,after_b"
    )?;
    let mut max_error = 0_f32;
    for d in (-300_i32..=300).step_by(5) {
        let (before, after) = sample(f64::from(d))?;
        let t = ((d as f32 + 250.) / 500.).clamp(0., 1.);
        let t = t * t * (3. - 2. * t);
        for k in 0..3 {
            max_error = max_error.max((after[k] - (a[k] + (b[k] - a[k]) * t)).abs());
        }
        if d.abs() >= 250 && before != after {
            return Err("correction escapes the 500 ft band".into());
        }
        let a = srgb(before);
        let b = srgb(after);
        writeln!(
            csv,
            "{d},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            a[0], a[1], a[2], b[0], b[1], b[2]
        )?;
    }
    let left = sample(-1.)?;
    let right = sample(1.)?;
    let step = |a: [f32; 3], b: [f32; 3]| (0..3).map(|k| (a[k] - b[k]).abs()).fold(0., f32::max);
    let before = step(srgb(left.0), srgb(right.0));
    let after = step(srgb(left.1), srgb(right.1));
    if before < 5. || after > 0.15 || max_error > 0.0007 {
        return Err(format!(
            "LAND ramp fails: before={before} after={after} linear error={max_error}"
        )
        .into());
    }
    let text = format!(
        "UKR LAND/PIC boundary x=16384 z=32768. Unlit bilinear source material, actual RG32Uint upload data.\nSource step across -1/+1 ft: {before:.6} RGB levels. Corrected: {after:.6}.\n500 ft smoothstep, -250/+250 endpoints unchanged; maximum linear-RGB error against analytic ramp: {max_error:.9}.\nPASS\n"
    );
    std::fs::write(out.join("line-check.txt"), &text)?;
    print!("{text}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn serialized_seam_cameras_look_down_and_centre_the_ground() {
        let mut world = tore_world::test_support::terrain();
        world.theater.cols = 9;
        world.theater.rows = 9;
        world.theater.cells = vec![
            tore_formats::theater::TerrainCell {
                color: 12,
                class: 2,
                elevation: 3
            };
            81
        ];
        for axis in 0..2 {
            for height in [100., 500., 2000., 10000., 20000.] {
                let spot = Spot {
                    x: 32768.,
                    z: 32768.,
                    axis,
                    flag: BLEND,
                    length: 10,
                };
                let camera = camera(&world, &spot, height);
                let [x, y, z] = camera.position;
                let line = format!(
                    "view UKR 0 12 0 {x} {y} {z} {:.9} {:.9}",
                    camera.yaw, camera.pitch
                );
                let decoded = crate::vegetation_preview::preview_camera(&line).unwrap();
                verify_camera(&world, &decoded, [32768., 768., 32768.]).unwrap();
                assert!((decoded.position[1] - 768. - height).abs() < 0.01);
            }
        }
        assert!(
            crate::vegetation_preview::preview_camera("broken UKR 0 12 0 0 2000 0 0 -30").is_err()
        );
    }
}
