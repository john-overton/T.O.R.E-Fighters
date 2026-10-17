//! `--vegetation-preview OUT [JOB ...]`: offscreen pictures of the
//! exploratory vegetation. No window. 2560 by 1440 (or
//! `TORE_VEG_PREVIEW_SIZE=WxH`), four samples, High density (or
//! `TORE_VEG_PREVIEW_DENSITY=low|medium|high|ultra`).
//!
//! Jobs: `ukr` (chase view, woodland, field, hillside, river, meadow,
//! close-ups),
//! `ab` (the chase view with and without an afterburner's light, and the
//! ground detail's energy in each), `egy`, `pgu`, `ira` (desert views at
//! 100, 300, 500, 1,000 and 5,000 ft, a dune field, rocky ground, an oasis
//! when the tile art has one, a wadi and a river), `cost` (frame cost at 1440p and 4K, Medium, High and Ultra) and
//! `overlay` (top-down classification pictures, Ukraine and Egypt). No job
//! named runs `ukr`, `ab`, `egy` and `pgu`.
//!
//! Every shot's camera is written to `cameras.txt`; `TORE_VEG_PREVIEW_CAMERAS`
//! renders such a file's cameras instead, so another build can take the
//! same pictures.

use crate::{AppResult, camera::Camera, scenery::Scenery, terrain::Terrain};
use std::{io::Write, path::Path, time::Instant};

#[derive(Clone, Copy)]
struct Shot {
    name: &'static str,
    spot: &'static str,
    /// Camera height above the ground, feet.
    height: f64,
    /// Look-down angle, degrees.
    pitch: f64,
    /// How far ahead of the camera the spot is, feet (0: from the pitch).
    ahead: f64,
}

const fn shot(name: &'static str, spot: &'static str, height: f64, pitch: f64, ahead: f64) -> Shot {
    Shot {
        name,
        spot,
        height,
        pitch,
        ahead,
    }
}

/// A camera, as written to and read from `cameras.txt`.
struct Cam {
    name: String,
    code: String,
    condition: usize,
    time: [i32; 2],
    x: f64,
    y: f64,
    z: f64,
    yaw: f64,
    pitch: f64,
}

fn size() -> AppResult<[u32; 2]> {
    match std::env::var("TORE_VEG_PREVIEW_SIZE") {
        Ok(v) => {
            let (w, h) = v.split_once('x').ok_or("TORE_VEG_PREVIEW_SIZE is WxH")?;
            Ok([w.parse()?, h.parse()?])
        }
        Err(_) => Ok([2560, 1440]),
    }
}

fn density() -> AppResult<crate::graphics::VegDensity> {
    match std::env::var("TORE_VEG_PREVIEW_DENSITY") {
        Ok(v) => crate::graphics::VegDensity::parse(&v)
            .ok_or_else(|| "bad TORE_VEG_PREVIEW_DENSITY".into()),
        Err(_) => Ok(crate::graphics::VegDensity::High),
    }
}

fn options(density: crate::graphics::VegDensity) -> crate::graphics::Options {
    let mut options = crate::graphics::Options {
        vegetation_density: density,
        ..Default::default()
    };
    if density == crate::graphics::VegDensity::Ultra {
        options.vegetation_distance_nm = 6;
        options.vegetation_detail_nm = 3;
    }
    options
}

pub fn run() -> AppResult<()> {
    let out = std::env::args()
        .nth(2)
        .ok_or("--vegetation-preview needs an output directory")?;
    let out = Path::new(&out);
    std::fs::create_dir_all(out)?;
    let mut jobs: Vec<String> = std::env::args().skip(3).collect();
    if jobs.is_empty() {
        jobs = ["ukr", "ab", "egy", "pgu"].map(String::from).to_vec();
    }
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    if jobs.iter().any(|job| job == "cameras") {
        // CPU only: freeze additional camera coordinates before either build
        // renders. These are terrain-class candidates, not surveyed locations.
        let mut file = std::fs::File::create(out.join("extra-cameras.txt"))?;
        for (code, name, kind, height, pitch) in [
            ("UKR", "UKR-field-edge-pass4", "field", 220., 10.),
            ("UKR", "UKR-mountain-slope-pass4", "hill", 700., 10.),
            ("EGY", "EGY-delta-candidate-pass4", "river", 900., 22.),
            ("TVIET", "TVIET-jungle-pass4", "wood", 450., 12.),
        ] {
            let world = Terrain::for_mission(resources, code, Some(0), &overrides([12, 0])?)?;
            let atlas =
                crate::vegetation::Atlas::build(resources, &world)?.ok_or("vegetation disabled")?;
            let spots = crate::vegetation::spots(&atlas, &world);
            let spot = spots
                .iter()
                .find(|s| s.label == kind)
                .ok_or_else(|| format!("no {code} {kind} camera candidate"))?;
            let cam = look_at(&world, spot, &shot(name, kind, height, pitch, 0.));
            writeln!(
                file,
                "{name} {code} 0 12 0 {} {} {} {} {}",
                cam.position[0], cam.position[1], cam.position[2], cam.yaw, cam.pitch
            )?;
        }
        return Ok(());
    }
    if let Ok(file) = std::env::var("TORE_VEG_PREVIEW_CAMERAS") {
        return replay_cameras(resources, out, &std::fs::read_to_string(file)?);
    }
    let mut cameras = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("cameras.txt"))?;
    let size = size()?;
    let density = density()?;
    for job in &jobs {
        match job.as_str() {
            "ukr" => scenes(
                resources,
                out,
                &mut cameras,
                size,
                density,
                "UKR",
                &[
                    shot("chase", "open", 300., 6., 0.),
                    shot("woodland", "wood", 450., 9., 0.),
                    shot("field", "field", 450., 9., 0.),
                    shot("hill", "hill", 700., 3., 7_000.),
                    shot("river", "river", 350., 7., 0.),
                    shot("meadow", "meadow", 400., 8., 0.),
                    shot("trees-close", "wood", 45., 2., 400.),
                    shot("grass-close", "field", 10., 4., 120.),
                    shot("chase", "open", 1_000., 10., 0.),
                ],
            )?,
            "egy" | "pgu" | "ira" | "spa" => {
                let code = job.to_ascii_uppercase();
                scenes(
                    resources,
                    out,
                    &mut cameras,
                    size,
                    density,
                    &code,
                    &[
                        shot("open", "open", 100., 4., 0.),
                        shot("open", "open", 300., 6., 0.),
                        shot("open", "open", 500., 8., 0.),
                        shot("open", "open", 1_000., 10., 0.),
                        shot("open", "open", 5_000., 22., 0.),
                        shot("dunes", "dunes", 300., 7., 0.),
                        shot("rocky", "rocky", 250., 5., 2_500.),
                        shot("oasis", "oasis", 300., 7., 0.),
                        shot("wadi", "wadi", 300., 7., 0.),
                        shot("river", "river", 350., 7., 0.),
                        shot("ground-close", "open", 12., 6., 100.),
                    ],
                )?
            }
            "ab" => afterburner(resources, out, size)?,
            "cost" => cost(resources, out)?,
            "overlay" => overlay(resources, out)?,
            other => println!("vegetation preview: unknown job {other}"),
        }
    }
    Ok(())
}

/// Renders `shots` in a theater at noon: the retail look (`-off`) and the
/// vegetation (`-after`), and prints the frame cost of each.
fn scenes(
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    out: &Path,
    cameras: &mut std::fs::File,
    size: [u32; 2],
    density: crate::graphics::VegDensity,
    code: &str,
    shots: &[Shot],
) -> AppResult<()> {
    println!("vegetation preview: building {code}");
    let world = Terrain::for_mission(resources, code, Some(0), &overrides([12, 0])?)?;
    let scenery = Scenery::build(resources, &world)?;
    let atlas = scenery
        .vegetation
        .as_ref()
        .ok_or("vegetation atlas missing (TORE_VEGETATION=0)")?;
    println!("  tone {:?}", atlas.tone);
    if let Some(lib) = &scenery.vegetation_meshes {
        for line in lib.report() {
            println!("  mesh {line}");
        }
    }
    let found = crate::vegetation::spots(atlas, &world);
    for spot in &found {
        println!(
            "  spot {} at {:.0}, {:.0} heading {:.0}",
            spot.label, spot.x, spot.z, spot.heading
        );
    }
    let mut gpu = pollster::block_on(View::new(&scenery, size, options(density)))?;
    for shot in shots {
        let Some(spot) = found.iter().find(|s| s.label == shot.spot) else {
            println!("  skip {} (no {} spot)", shot.name, shot.spot);
            continue;
        };
        let camera = look_at(&world, spot, shot);
        let stem = format!("{code}-{}-{}ft", shot.name, shot.height as u32);
        writeln!(
            cameras,
            "{stem} {code} 0 12 0 {} {} {} {} {}",
            camera.position[0], camera.position[1], camera.position[2], camera.yaw, camera.pitch
        )?;
        gpu.set_shown(false);
        let off_ms = gpu.time(&camera, &world, &scenery)?;
        let before = gpu.pixels(&camera, &world, &scenery)?;
        gpu.set_shown(true);
        let on_ms = gpu.time(&camera, &world, &scenery)?;
        let after = gpu.pixels(&camera, &world, &scenery)?;
        let (placed, tufts) = gpu
            .sim
            .vegetation
            .as_ref()
            .map(|v| (v.placed(), v.tufts_drawn()))
            .unwrap_or_default();
        write_png(out, &format!("{stem}-off.png"), size, &before)?;
        write_png(out, &format!("{stem}-after.png"), size, &after)?;
        println!(
            "  {stem}: off {off_ms:.2} ms, on {on_ms:.2} ms, objects {placed}, grass tufts {tufts}"
        );
    }
    Ok(())
}

/// The chase view at 300 and 40 ft with and without an afterburner's light just
/// ahead of the camera (where the jet is). The ground detail must be the
/// same: the light only adds light. Prints the detail energy (mean
/// absolute difference from a 3 by 3 blur, ground half of the picture) of
/// each and their ratio, and writes both pictures.
fn afterburner(
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    out: &Path,
    size: [u32; 2],
) -> AppResult<()> {
    println!("vegetation preview: afterburner check");
    let world = Terrain::for_mission(resources, "UKR", Some(0), &overrides([12, 0])?)?;
    let scenery = Scenery::build(resources, &world)?;
    let atlas = scenery
        .vegetation
        .as_ref()
        .ok_or("vegetation atlas missing")?;
    let found = crate::vegetation::spots(atlas, &world);
    let spot = found
        .iter()
        .find(|s| s.label == "open")
        .ok_or("no open spot")?;
    let mut gpu = pollster::block_on(View::new(&scenery, size, options(density()?)))?;
    gpu.set_shown(true);
    let devices = tore_sim::combat::countermeasures::Devices::default();
    let guns = crate::gun_flash::Drawn::default();
    for height in [300., 40.] {
        let camera = look_at(&world, spot, &shot("chase", "open", height, 6., 0.));
        gpu.sim.emitters(&gpu.queue, &devices, &[], &guns);
        gpu.time(&camera, &world, &scenery)?;
        let plain = gpu.pixels(&camera, &world, &scenery)?;
        let (sy, cy) = camera.yaw.sin_cos();
        let nozzle = [
            camera.position[0] + f64::from(sy) * 70.,
            camera.position[1] - 12.,
            camera.position[2] + f64::from(cy) * 70.,
        ];
        let burner = [crate::countermeasure_renderer::Afterburner {
            position: nozzle,
            share: 1.,
        }];
        gpu.sim.emitters(&gpu.queue, &devices, &burner, &guns);
        gpu.time(&camera, &world, &scenery)?;
        let lit = gpu.pixels(&camera, &world, &scenery)?;
        let stem = format!("UKR-chase-{}ft", height as u32);
        write_png(out, &format!("{stem}-no-afterburner.png"), size, &plain)?;
        write_png(out, &format!("{stem}-afterburner.png"), size, &lit)?;
        let a = detail_energy(&plain, size);
        let b = detail_energy(&lit, size);
        println!(
            "  {stem}: detail energy without {a:.3}, with afterburner {b:.3}, ratio {:.3}",
            b / a.max(1e-6)
        );
    }
    Ok(())
}

/// Mean absolute difference between each pixel's luma and its 3 by 3 mean,
/// over the lower half of the picture (the ground in a chase view).
pub fn detail_energy(rgba: &[u8], size: [u32; 2]) -> f64 {
    let (w, h) = (size[0] as usize, size[1] as usize);
    let luma = |x: usize, y: usize| {
        let p = &rgba[(y * w + x) * 4..];
        0.2126 * f64::from(p[0]) + 0.7152 * f64::from(p[1]) + 0.0722 * f64::from(p[2])
    };
    let mut sum = 0.;
    let mut n = 0usize;
    for y in (h / 2).max(1)..h - 1 {
        for x in 1..w - 1 {
            let mut mean = 0.;
            for dy in 0..3 {
                for dx in 0..3 {
                    mean += luma(x + dx - 1, y + dy - 1);
                }
            }
            sum += (luma(x, y) - mean / 9.).abs();
            n += 1;
        }
    }
    sum / n.max(1) as f64
}

/// Frame cost of a low pass over Ukraine at 1440p and 4K, Medium, High and Ultra:
/// everything off, then on.
fn cost(resources: &std::collections::BTreeMap<String, Vec<u8>>, out: &Path) -> AppResult<()> {
    println!("vegetation preview: frame cost");
    let world = Terrain::for_mission(resources, "UKR", Some(0), &overrides([12, 0])?)?;
    let scenery = Scenery::build(resources, &world)?;
    let atlas = scenery
        .vegetation
        .as_ref()
        .ok_or("vegetation atlas missing")?;
    let found = crate::vegetation::spots(atlas, &world);
    let mut report = String::new();
    for spot_name in ["open", "wood"] {
        let spot = found
            .iter()
            .find(|s| s.label == spot_name)
            .ok_or("no spot")?;
        let camera = look_at(&world, spot, &shot("pass", spot_name, 300., 6., 0.));
        for size in [[2560u32, 1440u32], [3840, 2160]] {
            for density in [
                crate::graphics::VegDensity::Medium,
                crate::graphics::VegDensity::High,
                crate::graphics::VegDensity::Ultra,
            ] {
                let mut gpu = pollster::block_on(View::new(&scenery, size, options(density)))?;
                gpu.set_shown(false);
                let off = gpu.time(&camera, &world, &scenery)?;
                let gpu_off = gpu.gpu_time(&camera, &world, &scenery)?;
                gpu.set_shown(true);
                let on = gpu.time(&camera, &world, &scenery)?;
                let gpu_on = gpu.gpu_time(&camera, &world, &scenery)?;
                let eye = [camera.position[0], camera.position[2]];
                let (placed, tufts, reach, squeeze) = gpu
                    .sim
                    .vegetation
                    .as_ref()
                    .map(|v| (v.placed(), v.tufts_drawn(), v.kept_reach(eye), v.squeeze()))
                    .unwrap_or_default();
                let line = format!(
                    "{spot_name} {}x{} {}: off {off:.2} ms, on {on:.2} ms, cost {:.2} ms, objects {placed} out to {:.1} nm, full density x{squeeze:.2}, grass tufts {tufts}\n",
                    size[0],
                    size[1],
                    density.label(),
                    on - off,
                    reach / crate::vegetation::NM as f32
                );
                print!("  {line}");
                report += &line;
                if let (Some(off), Some(on)) = (gpu_off, gpu_on) {
                    let line = format!(
                        "GPU {spot_name} {}x{} {}: off {off:.3} ms, on {on:.3} ms, cost {:.3} ms\n",
                        size[0],
                        size[1],
                        density.label(),
                        on - off
                    );
                    print!("  {line}");
                    report += &line;
                }
            }
        }
    }
    std::fs::write(out.join("frame-cost.txt"), report)?;
    Ok(())
}

/// Top-down classification pictures: Ukraine around its woodland and its
/// fields, Egypt around its dunes and its river.
fn overlay(resources: &std::collections::BTreeMap<String, Vec<u8>>, out: &Path) -> AppResult<()> {
    for (code, labels) in [
        ("UKR", ["wood", "field", "hill"]),
        ("EGY", ["dunes", "rocky", "oasis"]),
    ] {
        println!("vegetation preview: overlay {code}");
        let world = Terrain::for_mission(resources, code, Some(0), &overrides([12, 0])?)?;
        let scenery = Scenery::build(resources, &world)?;
        let atlas = scenery
            .vegetation
            .as_ref()
            .ok_or("vegetation atlas missing")?;
        let found = crate::vegetation::spots(atlas, &world);
        for label in labels {
            let Some(spot) = found.iter().find(|s| s.label == label) else {
                continue;
            };
            for (span, px) in [(40_000., 1024usize), (8_000., 1024)] {
                let rgb = atlas.overlay(&world, [spot.x, spot.z], span, px);
                let mut rgba = Vec::with_capacity(px * px * 4);
                for c in rgb.chunks_exact(3) {
                    rgba.extend([c[0], c[1], c[2], 255]);
                }
                let png = crate::replay::png::encode_rgba(px as u32, px as u32, &rgba)?;
                let name = format!("overlay-{code}-{label}-{}ft.png", span as u32);
                std::fs::write(out.join(&name), png)?;
                println!("  {name}");
            }
        }
    }
    Ok(())
}

/// Renders every camera of a `cameras.txt`, writing `NAME.png`.
fn replay_cameras(
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    out: &Path,
    text: &str,
) -> AppResult<()> {
    let mut list = Vec::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        if f.len() < 10 {
            return Err(format!("bad camera line: {line}").into());
        }
        list.push(Cam {
            name: f[0].into(),
            code: f[1].into(),
            condition: f[2].parse()?,
            time: [f[3].parse()?, f[4].parse()?],
            x: f[5].parse()?,
            y: f[6].parse()?,
            z: f[7].parse()?,
            yaw: f[8].parse()?,
            pitch: f[9].parse()?,
        });
    }
    let size = size()?;
    let mut current: Option<(String, Terrain, Scenery, View)> = None;
    for cam in list {
        if current.as_ref().is_none_or(|c| c.0 != cam.code) {
            println!("vegetation preview: building {}", cam.code);
            let world = Terrain::for_mission(
                resources,
                &cam.code,
                Some(cam.condition),
                &overrides(cam.time)?,
            )?;
            let scenery = Scenery::build(resources, &world)?;
            let mut view = pollster::block_on(View::new(&scenery, size, options(density()?)))?;
            view.set_shown(true);
            current = Some((cam.code.clone(), world, scenery, view));
        }
        let (_, world, scenery, view) = current.as_mut().expect("built above");
        let mut camera = Camera::new();
        camera.position = [cam.x, cam.y, cam.z];
        camera.yaw = cam.yaw as f32;
        camera.pitch = cam.pitch as f32;
        let ms = view.time(&camera, world, scenery)?;
        let pixels = view.pixels(&camera, world, scenery)?;
        write_png(out, &format!("{}.png", cam.name), size, &pixels)?;
        println!("  {}: {ms:.2} ms", cam.name);
    }
    Ok(())
}

/// `--vegetation-art OUT`: every retail vegetation and rock texture of the
/// import as a PNG through the Ukraine reference palette, the cutout (index
/// 255) in magenta. A look at what the import holds; nothing is committed.
pub fn art() -> AppResult<()> {
    let out = std::env::args()
        .nth(2)
        .ok_or("--vegetation-art needs an output directory")?;
    let out = Path::new(&out);
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let environment =
        tore_formats::theater::Environment::parse(resources.get("UKR.MM").ok_or("no UKR.MM")?)?;
    let palette = tore_world::city::mask::reference_palette(resources, &environment.map)
        .ok_or("no reference palette")?;
    let art = crate::vegetation::art();
    let mut names: Vec<(&String, &Vec<u8>)> = art.iter().collect();
    names.extend(
        resources
            .iter()
            .filter(|(n, _)| n.starts_with("_ROCK") && n.ends_with(".PIC")),
    );
    println!("vegetation art: {} resources in the import", art.len());
    for (name, bytes) in names {
        if !name.ends_with(".PIC") {
            continue;
        }
        let pic = tore_formats::Pic::parse(bytes)?;
        let mut rgba = Vec::with_capacity(pic.width * pic.height * 4);
        let mut cut = 0usize;
        for (p, m) in pic.pixels.iter().zip(&pic.mask) {
            if !*m || *p == 255 {
                cut += 1;
                rgba.extend([255, 0, 255, 255]);
            } else {
                let c = palette[usize::from(*p)];
                rgba.extend([c[0], c[1], c[2], 255]);
            }
        }
        println!("  {name}: {}x{}, cutout {cut}", pic.width, pic.height);
        let png = crate::replay::png::encode_rgba(pic.width as u32, pic.height as u32, &rgba)?;
        std::fs::write(
            out.join(format!("{}.png", name.trim_end_matches(".PIC"))),
            png,
        )?;
    }
    let mut shapes: Vec<(&String, &Vec<u8>)> = art.iter().collect();
    shapes.extend(
        resources
            .iter()
            .filter(|(n, _)| n.starts_with("ROCK") && n.ends_with(".SH")),
    );
    for (name, bytes) in shapes {
        if !name.ends_with(".SH") {
            continue;
        }
        let shape = tore_formats::shape::Shape::scenery(bytes)?;
        let scale = tore_formats::shape::object_scale(bytes).unwrap_or(1.);
        let mut lo = [f32::INFINITY; 3];
        let mut hi = [f32::NEG_INFINITY; 3];
        let mut textures = std::collections::BTreeSet::new();
        let mut textured = 0;
        for face in &shape.faces {
            for p in &face.positions {
                for k in 0..3 {
                    lo[k] = lo[k].min(p[k]);
                    hi[k] = hi[k].max(p[k]);
                }
            }
            if !face.texture.is_empty() {
                textured += 1;
                textures.insert(face.texture.to_ascii_uppercase());
            }
        }
        println!(
            "  {name}: {} faces ({textured} textured), scale {scale}, x {:.0}..{:.0} y {:.0}..{:.0} up {:.0}..{:.0}, {:?}",
            shape.faces.len(),
            lo[0],
            hi[0],
            lo[1],
            hi[1],
            lo[2],
            hi[2],
            textures
        );
        if std::env::var_os("TORE_VEGETATION_FACES").is_some() {
            for face in &shape.faces {
                let c = face
                    .positions
                    .iter()
                    .fold([0f32; 3], |a, p| [a[0] + p[0], a[1] + p[1], a[2] + p[2]]);
                let n = face.positions.len() as f32;
                println!(
                    "    face {} pts centre {:.0},{:.0},{:.0} tex {} colour {:?}",
                    face.positions.len(),
                    c[0] / n,
                    c[1] / n,
                    c[2] / n,
                    face.texture,
                    face.colors.first()
                );
            }
        }
    }
    let mut pages = Vec::new();
    let lib =
        crate::vegetation_shapes::Library::build(&art, resources, &palette, &mut pages, |p| {
            p.len() / 65536
        });
    for (i, mesh) in lib.meshes.iter().enumerate() {
        let page = &pages[mesh.sprite_layer * 65536..][..65536];
        let rgba: Vec<u8> = page
            .iter()
            .flat_map(|&index| {
                if index == 255 {
                    [0, 0, 0, 0]
                } else {
                    let c = palette[usize::from(index)];
                    [c[0], c[1], c[2], 255]
                }
            })
            .collect();
        write_png(
            out,
            &format!("baked-{i}-{}", mesh.source),
            [256, 256],
            &rgba,
        )?;
        println!(
            "baked {i} {} mean {:?} visible {} black {}",
            mesh.source,
            mesh.recolour,
            page.iter().filter(|&&p| p != 255).count(),
            page.iter()
                .filter(|&&p| p != 255 && palette[usize::from(p)].iter().all(|&c| c < 8))
                .count()
        );
    }
    Ok(())
}

fn overrides(time: [i32; 2]) -> AppResult<crate::terrain::Overrides> {
    Ok(crate::terrain::Overrides {
        time: Some(time),
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports: crate::scenery::redrawn_airports()?,
        city_fill: crate::scenery::city_fill()?,
        city_fill_cap: crate::scenery::city_fill_cap()?,
    })
}

/// A camera `shot.height` above the ground looking down `shot.pitch` at the
/// spot, along the spot's heading.
fn look_at(world: &Terrain, spot: &crate::vegetation::Spot, shot: &Shot) -> Camera {
    let pitch = shot.pitch.to_radians();
    let yaw = spot.heading.to_radians();
    let back = if shot.ahead > 0. {
        shot.ahead
    } else {
        (shot.height / pitch.tan().abs().max(1e-3)).min(12_000.)
    };
    let x = spot.x - back * yaw.sin();
    let z = spot.z - back * yaw.cos();
    let ground = f64::from(world.height(x as f32, z as f32));
    let mut camera = Camera::new();
    camera.yaw = yaw as f32;
    camera.pitch = -pitch as f32;
    camera.position = [x, ground + shot.height, z];
    camera
}

fn write_png(dir: &Path, name: &str, size: [u32; 2], rgba: &[u8]) -> AppResult<()> {
    let bytes = crate::replay::png::encode_rgba(size[0], size[1], rgba)?;
    std::fs::write(dir.join(name), bytes)?;
    Ok(())
}

struct View {
    device: wgpu::Device,
    queue: wgpu::Queue,
    sim: crate::sim_renderer::SimRenderer,
    texture: wgpu::Texture,
    buffer: wgpu::Buffer,
    size: [u32; 2],
    options: crate::graphics::Options,
}

impl View {
    async fn new(
        scenery: &Scenery,
        size: [u32; 2],
        options: crate::graphics::Options,
    ) -> AppResult<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;
        println!("vegetation preview adapter: {}", adapter.get_info().name);
        let timing = std::env::var("TORE_VEG_GPU_TIMING").as_deref() == Ok("1");
        let features = if timing {
            wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS
        } else {
            wgpu::Features::empty()
        };
        if !adapter.features().contains(features) {
            return Err("vegetation GPU timestamps unsupported by this adapter".into());
        }
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: features,
                ..Default::default()
            })
            .await?;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let mut sim =
            crate::sim_renderer::SimRenderer::new(&device, &queue, format, scenery, options, 4);
        if let Some(vegetation) = &mut sim.vegetation {
            vegetation.set_settle(true);
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Vegetation preview"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vegetation preview readback"),
            size: u64::from(size[0]) * u64::from(size[1]) * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Ok(Self {
            device,
            queue,
            sim,
            texture,
            buffer,
            size,
            options,
        })
    }

    fn set_shown(&mut self, on: bool) {
        let mut options = self.options;
        if !on {
            options.ground_texture = crate::graphics::GroundTexture::Off;
        }
        self.sim.set_graphics(&self.device, options, 4);
        if let Some(vegetation) = &mut self.sim.vegetation {
            vegetation.set_suppressed(!on);
            vegetation.set_settle(true);
        }
    }

    fn draw(&mut self, camera: &Camera, world: &Terrain, scenery: &Scenery) {
        let view = self.texture.create_view(&Default::default());
        let mut encoder = self.device.create_command_encoder(&Default::default());
        self.sim.draw(
            &self.device,
            &self.queue,
            &mut encoder,
            &view,
            self.size,
            camera,
            world,
            scenery,
        );
        self.queue.submit([encoder.finish()]);
    }

    fn wait(&self) -> AppResult<()> {
        self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        })?;
        Ok(())
    }

    /// Actual GPU command time, excluding CPU placement/submission and readback.
    /// Optional so the ordinary preview remains portable to adapters without queries.
    fn gpu_time(
        &mut self,
        camera: &Camera,
        world: &Terrain,
        scenery: &Scenery,
    ) -> AppResult<Option<f64>> {
        if !self
            .device
            .features()
            .contains(wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS)
        {
            return Ok(None);
        }
        const FRAMES: u32 = 8;
        self.draw(camera, world, scenery);
        self.draw(camera, world, scenery);
        self.wait()?;
        let queries = self.device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("Vegetation frame timestamps"),
            ty: wgpu::QueryType::Timestamp,
            count: FRAMES * 2,
        });
        let bytes = u64::from(FRAMES) * 16;
        let resolved = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vegetation timestamps resolved"),
            size: bytes,
            usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Vegetation timestamps readback"),
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        for frame in 0..FRAMES {
            let view = self.texture.create_view(&Default::default());
            let mut encoder = self.device.create_command_encoder(&Default::default());
            encoder.write_timestamp(&queries, frame * 2);
            self.sim.draw(
                &self.device,
                &self.queue,
                &mut encoder,
                &view,
                self.size,
                camera,
                world,
                scenery,
            );
            encoder.write_timestamp(&queries, frame * 2 + 1);
            if frame + 1 == FRAMES {
                encoder.resolve_query_set(&queries, 0..FRAMES * 2, &resolved, 0);
                encoder.copy_buffer_to_buffer(&resolved, 0, &readback, 0, bytes);
            }
            self.queue.submit([encoder.finish()]);
        }
        let (tx, rx) = std::sync::mpsc::channel();
        readback.slice(..).map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.wait()?;
        rx.recv()??;
        let mapped = readback.slice(..).get_mapped_range();
        let ticks: u64 = mapped
            .chunks_exact(16)
            .map(|pair| {
                let start = u64::from_le_bytes(pair[..8].try_into().expect("timestamp"));
                let end = u64::from_le_bytes(pair[8..].try_into().expect("timestamp"));
                end.saturating_sub(start)
            })
            .sum();
        let milliseconds = ticks as f64 * f64::from(self.queue.get_timestamp_period())
            / 1_000_000.
            / f64::from(FRAMES);
        drop(mapped);
        readback.unmap();
        Ok(Some(milliseconds))
    }

    /// Mean of eight frames after a warmup, from submit through the wait.
    /// The readback is not included.
    fn time(&mut self, camera: &Camera, world: &Terrain, scenery: &Scenery) -> AppResult<f64> {
        self.draw(camera, world, scenery);
        self.draw(camera, world, scenery);
        self.wait()?;
        let start = Instant::now();
        const FRAMES: u32 = 8;
        for _ in 0..FRAMES {
            self.draw(camera, world, scenery);
        }
        self.wait()?;
        Ok(start.elapsed().as_secs_f64() * 1_000. / f64::from(FRAMES))
    }

    fn pixels(
        &mut self,
        camera: &Camera,
        world: &Terrain,
        scenery: &Scenery,
    ) -> AppResult<Vec<u8>> {
        self.draw(camera, world, scenery);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(self.size[0] * 4),
                    rows_per_image: Some(self.size[1]),
                },
            },
            wgpu::Extent3d {
                width: self.size[0],
                height: self.size[1],
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = tx.send(result);
            });
        self.wait()?;
        rx.recv()??;
        let mapped = self.buffer.slice(..).get_mapped_range();
        let pixels = mapped.to_vec();
        drop(mapped);
        self.buffer.unmap();
        Ok(pixels)
    }
}
