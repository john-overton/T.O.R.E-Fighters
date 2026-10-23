//! CPU-only mask audit and reproducible cameras for the lead's GPU review.
use super::{BLEND, HEDGE, Map, TILE};
#[path = "terrain_seams_flat_audit.rs"]
mod flat_audit;
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
    let mut mask_projection = std::fs::File::create(out.join("mask-projection.tsv"))?;
    writeln!(mask_projection, "camera\tx\tfirst_y\tlast_y\twidth_pixels")?;
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
            || scene.texture_indices != cached.texture_indices
        {
            return Err("seam/card/canopy cache differs".into());
        }
        crate::vegetation_limits::validate(&scene, &wgpu::Limits::downlevel_defaults())?;
        interiors_probe(resources, out, code, &world, &scene)?;
        flat_audit::run(out, code, &world, &scene)?;
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
            for height in [500_f64, 2000., 5000., 10000.] {
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
        texture_probe(out, code, &scene, &spot)?;
        for (side, label) in [(-1., "south"), (1., "north")] {
            let mut p = [spot.x, spot.z];
            p[spot.axis] += 8192. * side;
            let mut cam = crate::camera::Camera::new();
            let target = [
                p[0],
                f64::from(world.height(p[0] as f32, p[1] as f32)),
                p[1],
            ];
            cam.position = [p[0], target[1] + 5000., p[1]];
            cam.pitch = -std::f32::consts::FRAC_PI_2;
            cam.yaw = 0.;
            write_camera(
                &mut cameras,
                &mut checks,
                &format!("{code}-interior-{label}"),
                code,
                &cam,
                target,
                &world,
            )?;
        }
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
        if code == "UKR" {
            for height in [5000., 10000.] {
                overview.position[1] = target[1] + height;
                write_camera(
                    &mut cameras,
                    &mut checks,
                    &format!("UKR-LAND-edge-overhead-{height:.0}ft"),
                    code,
                    &overview,
                    target,
                    &world,
                )?;
                let bounds: Vec<_> = [-250., 250.]
                    .into_iter()
                    .map(|d| {
                        let mut p = [spot.x, spot.z];
                        p[spot.axis] += d;
                        overview
                            .project(
                                [3840, 2160],
                                [
                                    p[0],
                                    f64::from(world.height(p[0] as f32, p[1] as f32)),
                                    p[1],
                                ],
                            )
                            .unwrap()
                    })
                    .collect();
                let lo = bounds[0][1].min(bounds[1][1]);
                let hi = bounds[0][1].max(bounds[1][1]);
                writeln!(
                    mask_projection,
                    "UKR-LAND-edge-overhead-{height:.0}ft\t1920\t{lo:.6}\t{hi:.6}\t{:.6}",
                    hi - lo
                )?;
            }
        }
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

/// Unlit reference strips: upper half baked source, lower half texture blend.
/// Both halves use the same bilinear source filtering.
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
            let mut color = packed
                .sample(map, p.map(|v| v as f32), &scene.texture_indices, &palette)
                .unwrap_or(palette[usize::from(index)]);
            if y >= H / 2 {
                let point = p.map(|v| v as f32);
                color = map
                    .baked
                    .surface(map, point, map.baked.flat_surface(map, point, color));
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
    // Synthesis can remove the colour difference exactly at the camera's
    // centre. Keep the same edge, but choose the nearest nonzero ramp probe.
    let mut segments: Vec<usize> = (3..253).collect();
    segments.sort_by_key(|s| s.abs_diff(128));
    let segment = segments
        .into_iter()
        .find(|&s| {
            let x = s as f32 * 128. + 64.;
            map.record(0, 1, s)[0] & super::FALLBACK != 0
                && map.edge_strength(0, 1, x, BLEND) >= 0.999
                && packed
                    .correction(map, [x, 32767.], &palette)
                    .iter()
                    .any(|v| v.abs() > 0.001)
                && packed
                    .sample(map, [x, 32767.], &scene.texture_indices, &palette)
                    .zip(packed.sample(map, [x, 32769.], &scene.texture_indices, &palette))
                    .is_some_and(|(a, b)| (0..3).any(|k| (a[k] - b[k]).abs() > 0.002))
        })
        .ok_or("no nonzero UKR colour-ramp probe")?;
    let x = segment as f32 * 128. + 64.;
    let noise = f32::from(packed.record(0, 1, segment)[7]) / 127.5 - 1.;
    let point = |d: f32| [x, 32768. + d];
    let surface = |d: f32| {
        packed
            .sample(map, point(d), &scene.texture_indices, &palette)
            .map(|own| {
                map.baked
                    .surface(map, point(d), map.baked.flat_surface(map, point(d), own))
            })
            .ok_or("no material")
    };
    if map.record(0, 1, 128)[0] & (BLEND | super::FALLBACK) != (BLEND | super::FALLBACK) {
        return Err("known LAND boundary is not enabled".into());
    }
    let mut csv = std::fs::File::create(out.join("UKR-LAND-line.csv"))?;
    writeln!(
        csv,
        "offset_ft,source_r,source_g,source_b,after_r,after_g,after_b,mean_weight,texture_weight"
    )?;
    // Test the colour component independently of the now-varying texture.
    let delta = packed
        .correction(map, point(-1.), &palette)
        .map(|v| v / super::weight(1.));
    let mut max_error = 0_f32;
    for d in (-300_i32..=300).step_by(5) {
        let before = map.baked.flat_surface(
            map,
            point(d as f32),
            ground_color(world, scene, point(d as f32).map(f64::from))?,
        );
        let after = surface(d as f32)?;
        let correction = packed.correction(map, point(d as f32), &palette);
        for k in 0..3 {
            let expected = delta[k] * super::weight(d as f32) * if d < 0 { 1. } else { -1. };
            max_error = max_error.max((correction[k] - expected).abs());
        }
        if d.abs() >= 250 && before.iter().zip(after).any(|(a, b)| (a - b).abs() > 1e-6) {
            return Err("texture correction escapes the 500 ft band".into());
        }
        let a = srgb(before);
        let b = srgb(after);
        let t = ((d as f32 + 250.) / 500.).clamp(0., 1.);
        writeln!(
            csv,
            "{d},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6},{:.6}",
            a[0],
            a[1],
            a[2],
            b[0],
            b[1],
            b[2],
            t * t * (3. - 2. * t),
            super::texture::blend_weight(d as f32, noise)
        )?;
    }
    if max_error > 1e-6 {
        return Err("colour ramp differs from 500 ft analytic smoothstep".into());
    }
    let step = |a: [f32; 3], b: [f32; 3]| (0..3).map(|k| (a[k] - b[k]).abs()).fold(0., f32::max);
    let before = step(
        srgb(ground_color(world, scene, point(-1.).map(f64::from))?),
        srgb(ground_color(world, scene, point(1.).map(f64::from))?),
    );
    let after = step(srgb(surface(-1.)?), srgb(surface(1.)?));
    let text = format!(
        "UKR LAND/PIC boundary x={x} z=32768, nearest nonzero colour ramp to camera x=16384. Actual packed pages, rotations and palette.\nBaked source step -1/+1 ft: {before:.6} RGB levels. Texture blended: {after:.6}.\n500 ft colour component analytic error: {max_error:.9}; texture endpoints unchanged at +/-250 ft.\nPASS\n"
    );
    std::fs::write(out.join("line-check.txt"), &text)?;
    print!("{text}");
    Ok(())
}

fn interiors_probe(
    resources: &BTreeMap<String, Vec<u8>>,
    out: &Path,
    code: &str,
    world: &Terrain,
    scene: &Scenery,
) -> AppResult<()> {
    let baseline = Scenery::build_for(resources, world, false)?;
    let ground_bytes = baseline
        .land_texture
        .map_or(baseline.texture_indices.len(), |p| (p + 1) * 65536);
    if scene.texture_indices[..ground_bytes] != baseline.texture_indices[..ground_bytes]
        || scene.ground_layers != baseline.ground_layers
        || scene.vertices != baseline.vertices
    {
        return Err(format!("{code}: original ground pages, UVs or mesh changed").into());
    }
    let map = &scene.vegetation.as_ref().ok_or("no atlas")?.seams;
    let mut probes = 0;
    for tile in 0..map.cols * map.rows {
        for v in 0..16 {
            for u in 0..16 {
                let p = [
                    (tile % map.cols) as f32 * TILE + 512. + u as f32 * 2048.,
                    (tile / map.cols) as f32 * TILE + 512. + v as f32 * 2048.,
                ];
                if map.baked.response(map, p)[..3] != [0.; 3]
                    || (map.flat.cells[tile] == 0
                        && map.baked.flat_response(map, p)[..3] != [0.; 3])
                {
                    return Err(format!("{code}: changed tile interior at {p:?}").into());
                }
                probes += 1;
            }
        }
    }
    let mut stats = std::fs::File::create(out.join(format!("{code}-source-statistics.csv")))?;
    writeln!(
        stats,
        "page,kind,mean_r,mean_g,mean_b,luminance_sd,rms_256ft,rms_512ft,rms_1024ft,rms_2048ft,rms_4096ft,rms_8192ft"
    )?;
    for (page, pixels) in scene.texture_indices[..ground_bytes]
        .chunks_exact(65536)
        .enumerate()
    {
        let values = super::texture::statistics(&[pixels], &scene.palette);
        write!(
            stats,
            "{page},{}",
            if Some(page) == scene.land_texture {
                "LAND"
            } else {
                "placed"
            }
        )?;
        for v in &values[..10] {
            write!(stats, ",{v:.9}")?;
        }
        writeln!(stats)?;
    }
    let text = format!(
        "{code}: {ground_bytes} original page bytes and every terrain vertex identical; {probes} seam-interior probes zero; unclassified placements have zero modulation. PASS\n"
    );
    std::fs::write(
        out.join(format!("{code}-bake-size.txt")),
        format!(
            "Tint atlas {:?}, {} bytes; record bytes {}. One mask load plus one filtered tint sample within accepted bands.\n",
            map.baked.size,
            map.baked.pixels.len(),
            map.baked.records.len()
        ),
    )?;
    std::fs::write(out.join(format!("{code}-interior-check.txt")), &text)?;
    print!("{text}");
    Ok(())
}

fn texture_probe(out: &Path, code: &str, scene: &Scenery, spot: &Spot) -> AppResult<()> {
    let map = &scene.vegetation.as_ref().ok_or("no atlas")?.seams;
    let packed = map.pack();
    let palette = scene.palette.map(|p| p.map(super::linear));
    let mut csv = std::fs::File::create(out.join(format!("{code}-texture-band.csv")))?;
    writeln!(
        csv,
        "offset_ft,source_luminance_mean,source_luminance_sd,after_luminance_mean,after_luminance_sd,colour_only_mean,colour_only_sd,samples"
    )?;
    let lum = |p: [f32; 3]| p[0] * 0.2126 + p[1] * 0.7152 + p[2] * 0.0722;
    let mut centre = [Vec::new(), Vec::new()];
    for d in [-250., -125., -32., -1., 1., 32., 125., 250.] {
        let mut sums = [0_f64; 6];
        let mut count = 0;
        for along in (-4096..4096).step_by(16) {
            let mut p = [spot.x as f32, spot.z as f32];
            p[spot.axis] += d;
            p[1 - spot.axis] += along as f32;
            let cell = p.map(|v| (v / TILE).floor() as usize);
            let mut owner = cell;
            if d >= 0. {
                owner[spot.axis] -= 1;
            }
            let along = (p[1 - spot.axis] % TILE) / 128.;
            if map.edge_strength(
                owner[1] * map.cols + owner[0],
                spot.axis,
                along * 128.,
                BLEND,
            ) < 0.999
            {
                continue;
            }
            let Some(a) = packed.sample(map, p, &scene.texture_indices, &palette) else {
                continue;
            };
            let a = map.baked.flat_surface(map, p, a);
            let b = map.baked.surface(map, p, a);
            let correction = packed.correction(map, p, &palette);
            let c = f64::from(lum(std::array::from_fn(|k| (a[k] + correction[k]).max(0.))));
            let a = f64::from(lum(a));
            let b = f64::from(lum(b));
            sums[0] += a;
            sums[1] += a * a;
            sums[2] += b;
            sums[3] += b * b;
            sums[4] += c;
            sums[5] += c * c;
            count += 1;
            if d == -1. || d == 1. {
                centre[usize::from(d > 0.)].push([a, b, c, f64::from(p[1 - spot.axis])]);
            }
            if d.abs() == 250. && (a - b).abs() > 1e-6 {
                return Err("texture blend escapes band".into());
            }
        }
        if count == 0 {
            return Err(format!("{code}: no accepted texture strip samples").into());
        }
        let n = f64::from(count);
        let mean = [sums[0] / n, sums[2] / n, sums[4] / n];
        let sd = [
            (sums[1] / n - mean[0] * mean[0]).max(0.).sqrt(),
            (sums[3] / n - mean[1] * mean[1]).max(0.).sqrt(),
            (sums[5] / n - mean[2] * mean[2]).max(0.).sqrt(),
        ];
        writeln!(
            csv,
            "{d},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{count}",
            mean[0], sd[0], mean[1], sd[1], mean[2], sd[2]
        )?;
    }
    if centre[0].len() != centre[1].len()
        || centre[0].iter().zip(&centre[1]).any(|(a, b)| a[3] != b[3])
    {
        return Err("texture-strip sample positions differ across seam".into());
    }
    let n = centre[0].len();
    let rms = |k: usize| {
        (centre[0]
            .iter()
            .zip(&centre[1])
            .map(|(a, b)| (a[k] - b[k]).powi(2))
            .sum::<f64>()
            / n.max(1) as f64)
            .sqrt()
    };
    let before = rms(0);
    let after = rms(1);
    let colour_only = rms(2);
    if after > before * 0.05 + 0.0001 {
        return Err(format!("{code}: texture discontinuity {before} -> {after}").into());
    }
    let text = format!(
        "{code}: accepted seam strips {n} pairs; luminance RMS jump across -1/+1 ft: source {before:.9}, colour-only {colour_only:.9}, texture blend {after:.9}. PASS\n"
    );
    std::fs::write(out.join(format!("{code}-texture-check.txt")), &text)?;
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
