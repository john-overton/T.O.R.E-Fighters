//! `--city-fill-dump`: development aid for the CF0 city fill prototype
//! (`tore_world::city`). No window opens and nothing is simulated.
//!
//! `--city-fill-dump OUT [THEATER] [--render NAME@X,Z,HEIGHT,PITCH,YAW ...]
//! [--bench X,Z,HEIGHT,HEADING,FEET_PER_FRAME,FRAMES] [--size W,H]`
//!
//! - Prints the kit, every city's lots and buildings, the theater totals,
//!   the build time, the fill digest, a memory estimate and the cost of the
//!   collision query.
//! - Writes per city `mask-CITY.png` (one pixel per texel, north up: cream
//!   roof sites, white solid roofs, red streets, green parks, blue water,
//!   the texture dimmed elsewhere) and `fill-CITY.png` (64 ft per pixel,
//!   every building's footprint coloured by class over the dimmed texture:
//!   yellow houses, orange low-rise, red apartments, magenta towers, violet
//!   industrial).
//! - `--render` draws each spot offscreen at 1920 by 1080 with the fill off
//!   and on: `NAME-off.png` and `NAME-on.png`. The camera looks at ground
//!   point X, Z from HEIGHT ft above it, PITCH degrees down, facing YAW.
//! - `--target NAME@X,Z,HEIGHT,PITCH,YAW,ZOOM` renders the spot as the
//!   target display draws it (camera slot 4, its zoom), off and on.
//! - `--target-bench X,Z,HEIGHT,PITCH,YAW,ZOOM,FRAMES` times that view held
//!   still at `--size`, off then on.
//! - `--sequence NAME@X,Z,HEIGHT,HEADING,STEP,FRAMES[,PITCH[,ZOOM]]` flies
//!   the offscreen camera as `--bench` does (PITCH degrees down, 6.9 by
//!   default) with the fill on and writes every frame at 1920 by 1080 as
//!   `NAME-0000.png` on: the transitions frame by frame. With ZOOM the
//!   frames are the target display's (camera slot 4).
//! - `--sweep NAME@X,Z,HEIGHT,PITCH,YAW,KIND,FROM,TO,STEPS` holds the
//!   camera still (as `--render`) and moves one tier over STEPS frames:
//!   KIND 0 the building detail distance (nautical miles, the blend band a
//!   quarter beyond it, drawn to 30 nm), 1 the building distance (detail
//!   5 nm), 2 the smallest drawn size (pixels). Nothing moves but the tier,
//!   so the change between frames is the transition alone: it prints, per
//!   step, how many pixels changed by more than 40 of 255 (a pop) and the
//!   mean change, and writes `NAME-0000.png` on.
//! - `--bench` flies the offscreen camera FRAMES frames along HEADING from
//!   X, Z at HEIGHT ft above the ground, FEET_PER_FRAME apart, at `--size`
//!   (3840 by 2160 by default), fill off then on, and prints each pass's
//!   frame times (CPU encode plus GPU completion, per frame).
use crate::{AppResult, camera::Camera, reel::Gpu, scenery::Scenery, terrain::Terrain};
use std::{
    collections::BTreeMap,
    path::Path,
    time::{Duration, Instant},
};
use tore_world::{
    city::{self, kit::Class, mask},
    surface::layout::trig,
};

/// Noon, no wind, no cloud; the redrawn airports unless
/// `TORE_REDRAWN_AIRPORTS=0`; the city fill `on` or off.
fn overrides(on: bool) -> AppResult<crate::terrain::Overrides> {
    Ok(crate::terrain::Overrides {
        time: Some([12, 0]),
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports: crate::scenery::redrawn_airports()?,
        city_fill: on,
    })
}

fn rss_kib() -> u64 {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines()
                .find(|l| l.starts_with("VmRSS:"))
                .and_then(|l| l.split_whitespace().nth(1)?.parse().ok())
        })
        .unwrap_or(0)
}

fn world(
    resources: &BTreeMap<String, Vec<u8>>,
    theater: &str,
    on: bool,
) -> AppResult<(Terrain, Scenery)> {
    let world = Terrain::for_mission(resources, theater, Some(0), &overrides(on)?)?;
    let scenery = Scenery::build(resources, &world)?;
    Ok((world, scenery))
}

fn class_color(class: Class) -> [u8; 3] {
    match class {
        Class::House => [255, 220, 60],
        Class::Industrial => [170, 110, 255],
        Class::LowRise => [255, 130, 30],
        Class::Apartment => [255, 40, 40],
        Class::Tower => [255, 60, 255],
    }
}

fn camera_at(world: &Terrain, v: &[f64]) -> Camera {
    let pitch = v[3].to_radians();
    let yaw = v[4].to_radians();
    let ground = f64::from(world.height(v[0] as f32, v[1] as f32));
    let back = v[2] / pitch.tan().abs().max(1e-3);
    let mut camera = Camera::new();
    camera.yaw = yaw as f32;
    camera.pitch = -pitch as f32;
    camera.position = [
        v[0] - back * yaw.sin(),
        ground + v[2],
        v[1] - back * yaw.cos(),
    ];
    camera
}

fn numbers(text: &str) -> AppResult<Vec<f64>> {
    Ok(text
        .split(',')
        .map(str::parse::<f64>)
        .collect::<Result<Vec<_>, _>>()?)
}

fn use_gpu(scenery: &Scenery, gpu: &mut Option<Gpu>) -> AppResult<()> {
    match gpu {
        Some(gpu) => {
            gpu.sim = crate::sim_renderer::SimRenderer::new(
                &gpu.device,
                &gpu.queue,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                scenery,
                crate::graphics::Options::default(),
                4,
            );
        }
        None => *gpu = Some(pollster::block_on(Gpu::new(scenery))?),
    }
    Ok(())
}

/// `--city-fill-survey [THEATER ...]`: per theater, the reference palette's
/// city-relevant colours and, per ground picture, the shares the classifier
/// could use, and whether a retail city cluster stands on it.
pub fn survey() -> AppResult<()> {
    let wanted: Vec<String> = std::env::args().skip(2).collect();
    let assets = crate::reel::load_assets()?;
    let resources = assets.theater_resources;
    for theater in tore_formats::quick_template::tables::THEATERS {
        if !wanted.is_empty() && !wanted.iter().any(|w| w.eq_ignore_ascii_case(theater)) {
            continue;
        }
        let layout = format!("{theater}.MM");
        let Some(bytes) = resources.get(&layout) else {
            println!("{theater}: no layout");
            continue;
        };
        let environment = tore_formats::theater::Environment::parse(bytes)?;
        let map = environment.map.clone();
        let Some(palette) = mask::reference_palette(&resources, &map) else {
            println!("{theater}: no reference palette ({map})");
            continue;
        };
        let luma = |c: [u8; 3]| {
            (u32::from(c[0]) * 299 + u32::from(c[1]) * 587 + u32::from(c[2]) * 114) / 1000
        };
        println!("== {theater} map {map}");
        let line: Vec<String> = (192..256)
            .map(|i| {
                let c = palette[i];
                format!("{i}:{},{},{}/{}", c[0], c[1], c[2], luma(c))
            })
            .collect();
        println!("palette {}", line.join(" "));
        // Cluster placements per texture.
        let placements = tore_formats::mission::Layout::parse(&layout, bytes)?;
        let mut clusters = BTreeMap::<String, usize>::new();
        let base = tore_formats::theater::base_theater(&layout).unwrap_or("");
        for p in &placements.placements {
            if !city::kit::CLUSTERS.contains(&p.object_type.as_str()) {
                continue;
            }
            let key = mask::LightMap::key(i64::from(p.position[0]), i64::from(p.position[2]));
            if let Some(t) = environment.textures.get(&key) {
                *clusters.entry(t.resource_name(base)).or_default() += 1;
            }
        }
        let mut uses = BTreeMap::<String, usize>::new();
        for t in environment.textures.values() {
            *uses.entry(t.resource_name(base)).or_default() += 1;
        }
        for (name, count) in &uses {
            let Some(pic) = resources
                .get(name)
                .and_then(|b| tore_formats::Pic::parse(b).ok())
                .filter(|p| p.width == 256 && p.height == 256)
            else {
                continue;
            };
            let px: Vec<u8> = pic
                .pixels
                .iter()
                .zip(&pic.mask)
                .map(|(p, m)| if *m { *p } else { 255 })
                .collect();
            let land = px.iter().filter(|&&p| p != 255).count().max(1);
            let (mut green, mut neutral, mut roof, mut dark, mut edges) = (0, 0, 0, 0, 0);
            for (i, &p) in px.iter().enumerate() {
                if p == 255 {
                    continue;
                }
                let c = palette[usize::from(p)].map(i32::from);
                let l = luma(palette[usize::from(p)]) as i32;
                let is_green = c[1] >= c[0] + 12 && c[1] >= c[2];
                let is_blue = c[2] >= c[0] + 20 && c[2] > c[1];
                if is_green {
                    green += 1;
                } else if !is_blue {
                    neutral += 1;
                    if l >= 125 {
                        roof += 1;
                    }
                    if l < 90 {
                        dark += 1;
                    }
                }
                if i % 256 < 255 && px[i + 1] != 255 {
                    let r = luma(palette[usize::from(px[i + 1])]) as i32;
                    if (l - r).abs() > 40 {
                        edges += 1;
                    }
                }
            }
            let pct = |n: usize| n * 100 / land;
            println!(
                "  {name} used {count} clusters {} land {}% green {}% neutral {}% roof {}% dark {}% edges {}%",
                clusters.get(name).copied().unwrap_or(0),
                land * 100 / 65536,
                pct(green),
                pct(neutral),
                pct(roof),
                pct(dark),
                pct(edges)
            );
        }
    }
    Ok(())
}

pub fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let usage = "--city-fill-dump OUT [THEATER] [--render NAME@X,Z,H,PITCH,YAW ...] [--sequence NAME@X,Z,H,HEADING,STEP,FRAMES[,PITCH[,ZOOM]] ...] [--sweep NAME@X,Z,H,PITCH,YAW,KIND,FROM,TO,STEPS ...] [--bench X,Z,H,HEADING,STEP,FRAMES] [--size W,H]";
    let out = Path::new(args.first().ok_or(usage)?);
    let mut theater = "UKR".to_owned();
    let mut renders = Vec::new();
    let mut targets = Vec::new();
    let mut target_bench = None;
    let mut bench = None;
    let mut sequences = Vec::new();
    let mut sweeps = Vec::new();
    let mut size = [3840u32, 2160u32];
    let mut it = args.iter().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--render" => renders.push(it.next().ok_or(usage)?.clone()),
            "--target" => targets.push(it.next().ok_or(usage)?.clone()),
            "--target-bench" => target_bench = Some(numbers(it.next().ok_or(usage)?)?),
            "--bench" => bench = Some(numbers(it.next().ok_or(usage)?)?),
            "--sequence" => sequences.push(it.next().ok_or(usage)?.clone()),
            "--sweep" => sweeps.push(it.next().ok_or(usage)?.clone()),
            "--size" => {
                let v = numbers(it.next().ok_or(usage)?)?;
                size = [v[0] as u32, v[1] as u32];
            }
            other => other.clone_into(&mut theater),
        }
    }
    std::fs::create_dir_all(out)?;
    let assets = crate::reel::load_assets()?;
    let resources = assets.theater_resources;

    let rss_start = rss_kib();
    let (_, off_scenery) = world(&resources, &theater, false)?;
    let rss_off = rss_kib();
    let started = Instant::now();
    let on_world = Terrain::for_mission(&resources, &theater, Some(0), &overrides(true)?)?;
    let built = started.elapsed();
    let on_scenery = Scenery::build(&resources, &on_world)?;
    let rss_on = rss_kib();
    drop(off_scenery);
    let Some(fill) = on_world.city_fill.clone() else {
        println!("city fill: {theater} has no city tiles");
        return Ok(());
    };
    let layout = &fill.layout;
    println!("kit: {} pieces", layout.kit.pieces.len());
    for (n, p) in layout.kit.pieces.iter().enumerate() {
        println!(
            "  piece {n}: {} faces {} footprint {} x {} ft, {} ft tall, {:?}{}",
            p.source,
            p.faces.len(),
            2 * p.half[0],
            2 * p.half[1],
            p.height,
            p.class,
            if p.cut { " (cut)" } else { "" }
        );
    }
    let mut by_class = BTreeMap::<Class, usize>::new();
    for i in &layout.instances {
        *by_class
            .entry(layout.kit.pieces[usize::from(i.piece)].class)
            .or_default() += 1;
    }
    println!("cities: {}", layout.cities.len());
    for (n, city) in layout.cities.iter().enumerate() {
        // Where its buildings are: the mean of its core buildings, or of all.
        let mean = |core: bool| {
            let (mut sx, mut sz, mut k) = (0i64, 0i64, 0i64);
            for i in layout
                .instances
                .iter()
                .filter(|i| usize::from(i.city) == n && (!core || i.core))
            {
                sx += i64::from(i.x);
                sz += i64::from(i.z);
                k += 1;
            }
            (k > 0).then(|| [sx / k, sz / k])
        };
        let focus = mean(true).or_else(|| mean(false)).unwrap_or(city.center);
        println!(
            "  {}: {} tiles, centre {:?}, focus {:?}, {} lots passed, {} buildings ({} core)",
            city.name,
            city.tiles.len(),
            city.center,
            focus,
            city.candidates,
            city.placed,
            city.core
        );
    }
    let hidden = fill.hidden.iter().filter(|h| **h).count();
    println!(
        "theater {theater}: {} city tiles, {} lots passed, {} buildings, {} hidden by mission objects, by class {:?}",
        layout.map.tiles.len(),
        layout.candidates,
        layout.instances.len(),
        hidden,
        by_class
    );
    let mut pictures = BTreeMap::<&str, usize>::new();
    for tile in layout.map.tiles.values() {
        *pictures.entry(tile.texture.as_str()).or_default() += 1;
    }
    println!(
        "city pictures: {}; districts {}",
        pictures
            .iter()
            .map(|(name, n)| format!("{name} x{n}"))
            .collect::<Vec<_>>()
            .join(", "),
        layout.map.districts.len()
    );
    // Buildings inside a redrawn airfield's box (none expected).
    let inside = layout
        .instances
        .iter()
        .filter(|i| {
            on_world.redrawn.iter().any(|r| {
                r.surface
                    .contains_horizontal(f64::from(i.x), f64::from(i.z))
            })
        })
        .count();
    println!(
        "redrawn airfields: {}, fill buildings inside them: {inside}",
        on_world.redrawn.len()
    );
    println!("fill digest: {:016x}", layout.digest());
    println!(
        "build: terrain with fill {:.0} ms",
        built.as_secs_f64() * 1e3
    );
    let art = on_scenery.fill.as_ref().expect("fill art");
    println!(
        "memory: instances {} KiB, kit vertices {} KiB ({} vertices), process RSS start {} MiB, after fill-off scene {} MiB, after fill-on scene {} MiB",
        layout.instances.len() * std::mem::size_of::<city::Instance>() / 1024,
        art.vertex_bytes() / 1024,
        art.vertices.len() / 10,
        rss_start / 1024,
        rss_off / 1024,
        rss_on / 1024
    );
    // Collision cost: low passes along the rows of each of the first
    // cities' first buildings, 0.5 ft per tick (about 60 ft/s per tick at
    // 120 Hz would be 10 ft; short steps make more queries).
    for (n, city) in layout.cities.iter().enumerate().take(3) {
        let rows: Vec<i32> = layout
            .instances
            .iter()
            .filter(|i| usize::from(i.city) == n)
            .step_by(97)
            .take(10)
            .map(|i| i.z)
            .collect();
        let steps: u32 = 12_000;
        let started = Instant::now();
        let mut inside = 0;
        let mut queries = 0u32;
        for z in &rows {
            for s in 0..steps {
                let x = city.center[0] as f64 - 30_000. + f64::from(s) * 5.;
                let z = f64::from(*z);
                let y = f64::from(on_world.height(x as f32, z as f32)) + 40.;
                queries += 1;
                if on_world.fill_contact([x, y, z], [x + 5., y, z]) {
                    inside += 1;
                }
            }
        }
        println!(
            "contact: {} ns per tick query at 40 ft across {} ({inside} of {queries} ticks inside a building)",
            started.elapsed().as_nanos() / u128::from(queries.max(1)),
            city.name,
        );
    }
    write_images(out, &resources, &on_world, &on_scenery, &fill)?;

    let mut gpu: Option<Gpu> = None;
    let hidden_none = std::collections::BTreeSet::new();
    for item in &renders {
        let (name, spot) = item.split_once('@').ok_or(usage)?;
        let v = numbers(spot)?;
        for on in [false, true] {
            let (world, mut scenery) = world(&resources, &theater, on)?;
            use_gpu(&scenery, &mut gpu)?;
            let gpu = gpu.as_mut().expect("gpu");
            let camera = camera_at(&world, &v);
            scenery.resolve_palette(&world, camera.position[1]);
            scenery.set_origin(camera.position);
            let geometry = std::sync::Arc::clone(scenery.static_geometry_where(&hidden_none));
            gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
            let pixels = gpu.pixels(&camera, &world, &scenery)?;
            let path = out.join(format!("{name}-{}.png", if on { "on" } else { "off" }));
            std::fs::write(
                &path,
                crate::replay::png::encode_rgba(crate::reel::WIDTH, crate::reel::HEIGHT, &pixels)?,
            )?;
            let stats = match &gpu.sim.fill {
                Some(f) => f.stats(&gpu.device, &gpu.queue)?,
                None => Default::default(),
            };
            println!(
                "render: {} ({} buildings at full detail, {} as far boxes)",
                path.display(),
                stats.full,
                stats.boxes,
            );
        }
    }
    for item in &targets {
        let (name, spot) = item.split_once('@').ok_or(usage)?;
        let v = numbers(spot)?;
        for on in [false, true] {
            let (world, mut scenery) = world(&resources, &theater, on)?;
            use_gpu(&scenery, &mut gpu)?;
            let gpu = gpu.as_mut().expect("gpu");
            let mut camera = camera_at(&world, &v);
            camera.weather_slot = 4;
            camera.zoom = v[5] as f32;
            scenery.resolve_palette(&world, camera.position[1]);
            scenery.set_origin(camera.position);
            let geometry = std::sync::Arc::clone(scenery.static_geometry_where(&hidden_none));
            gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
            let pixels = gpu.pixels(&camera, &world, &scenery)?;
            let path = out.join(format!("{name}-{}.png", if on { "on" } else { "off" }));
            std::fs::write(
                &path,
                crate::replay::png::encode_rgba(crate::reel::WIDTH, crate::reel::HEIGHT, &pixels)?,
            )?;
            let stats = match &gpu.sim.fill {
                Some(f) => f.stats(&gpu.device, &gpu.queue)?,
                None => Default::default(),
            };
            println!(
                "target: {} ({} buildings at full detail, {} as far boxes)",
                path.display(),
                stats.full,
                stats.boxes
            );
        }
    }
    for item in &sequences {
        let (name, spot) = item.split_once('@').ok_or(usage)?;
        let v = numbers(spot)?;
        if v.len() < 6 {
            return Err(usage.into());
        }
        let (world, mut scenery) = world(&resources, &theater, true)?;
        use_gpu(&scenery, &mut gpu)?;
        let gpu = gpu.as_mut().expect("gpu");
        let geometry = std::sync::Arc::clone(scenery.static_geometry_where(&hidden_none));
        gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
        let heading = v[3].to_radians();
        let pitch = v
            .get(6)
            .copied()
            .unwrap_or(0.12f64.to_degrees())
            .to_radians();
        for frame in 0..v[5] as usize {
            let along = frame as f64 * v[4];
            let x = v[0] + along * heading.sin();
            let z = v[1] + along * heading.cos();
            let mut camera = Camera::new();
            camera.yaw = heading as f32;
            camera.pitch = -pitch as f32;
            camera.position = [x, f64::from(world.height(x as f32, z as f32)) + v[2], z];
            if let Some(&zoom) = v.get(7) {
                camera.weather_slot = 4;
                camera.zoom = zoom as f32;
            }
            scenery.resolve_palette(&world, camera.position[1]);
            scenery.set_origin(camera.position);
            let pixels = gpu.pixels(&camera, &world, &scenery)?;
            let path = out.join(format!("{name}-{frame:04}.png"));
            std::fs::write(
                &path,
                crate::replay::png::encode_rgba(crate::reel::WIDTH, crate::reel::HEIGHT, &pixels)?,
            )?;
        }
        println!("sequence: {name}, {} frames in {}", v[5], out.display());
    }
    for item in &sweeps {
        let (name, spot) = item.split_once('@').ok_or(usage)?;
        let v = numbers(spot)?;
        if v.len() < 9 {
            return Err(usage.into());
        }
        let (world, mut scenery) = world(&resources, &theater, true)?;
        use_gpu(&scenery, &mut gpu)?;
        let gpu = gpu.as_mut().expect("gpu");
        let geometry = std::sync::Arc::clone(scenery.static_geometry_where(&hidden_none));
        gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
        let camera = camera_at(&world, &v);
        scenery.resolve_palette(&world, camera.position[1]);
        scenery.set_origin(camera.position);
        let steps = (v[8] as usize).max(2);
        let mut previous: Option<Vec<u8>> = None;
        let (mut pops, mut worst, mut change) = (0usize, 0usize, 0f64);
        for step in 0..steps {
            let at = v[6] + (v[7] - v[6]) * step as f64 / (steps - 1) as f64;
            let mut tiers = crate::fill_renderer::Tiers {
                full_nm: 5.,
                blend_nm: 5. * crate::fill_renderer::BLEND_SHARE,
                draw_nm: 30.,
                min_px: 1.,
                detail_px: 0.,
            };
            match v[5] as u32 {
                0 => {
                    tiers.full_nm = at;
                    tiers.blend_nm = at * crate::fill_renderer::BLEND_SHARE;
                }
                1 => tiers.draw_nm = at,
                _ => tiers.min_px = at,
            }
            if let Some(fill) = gpu.sim.fill.as_mut() {
                fill.set_tiers(Some(tiers));
            }
            let pixels = gpu.pixels(&camera, &world, &scenery)?;
            if let Some(before) = &previous {
                let mut popped = 0usize;
                let mut sum = 0u64;
                for (a, b) in before.chunks_exact(4).zip(pixels.chunks_exact(4)) {
                    let d = (0..3).map(|k| a[k].abs_diff(b[k])).max().unwrap_or(0);
                    sum += u64::from(d);
                    if d > 40 {
                        popped += 1;
                    }
                }
                let mean = sum as f64 / (pixels.len() / 4) as f64;
                println!(
                    "sweep {name} step {step} at {at:.3}: {popped} pixels popped, mean change {mean:.4}"
                );
                pops += popped;
                worst = worst.max(popped);
                change += mean;
            }
            std::fs::write(
                out.join(format!("{name}-{step:04}.png")),
                crate::replay::png::encode_rgba(crate::reel::WIDTH, crate::reel::HEIGHT, &pixels)?,
            )?;
            previous = Some(pixels);
        }
        if let Some(fill) = gpu.sim.fill.as_mut() {
            fill.set_tiers(None);
        }
        println!(
            "sweep {name}: {steps} steps, {pops} pixels popped in all, at most {worst} in one step, mean change {change:.4} in all"
        );
    }
    if let Some(v) = target_bench {
        let frames = v[6] as usize;
        let mut means = Vec::new();
        for on in [false, true] {
            let (world, mut scenery) = world(&resources, &theater, on)?;
            use_gpu(&scenery, &mut gpu)?;
            let gpu = gpu.as_mut().expect("gpu");
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("City fill target bench target"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let geometry = std::sync::Arc::clone(scenery.static_geometry_where(&hidden_none));
            gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
            let mut camera = camera_at(&world, &v);
            camera.weather_slot = 4;
            camera.zoom = v[5] as f32;
            scenery.resolve_palette(&world, camera.position[1]);
            scenery.set_origin(camera.position);
            let mut times = Vec::with_capacity(frames);
            for frame in 0..frames + 30 {
                let started = Instant::now();
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                gpu.sim.draw(
                    &gpu.device,
                    &gpu.queue,
                    &mut encoder,
                    &view,
                    size,
                    &camera,
                    &world,
                    &scenery,
                );
                gpu.queue.submit([encoder.finish()]);
                gpu.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(60)),
                })?;
                if frame >= 30 {
                    times.push(started.elapsed().as_secs_f64() * 1e3);
                }
            }
            let stats = match &gpu.sim.fill {
                Some(f) => f.stats(&gpu.device, &gpu.queue)?,
                None => Default::default(),
            };
            let mean = times.iter().sum::<f64>() / times.len() as f64;
            println!(
                "target bench fill {}: {} frames at {}x{}, mean {:.3} ms, {} buildings at full detail, {} as far boxes",
                if on { "on" } else { "off" },
                times.len(),
                size[0],
                size[1],
                mean,
                stats.full,
                stats.boxes
            );
            means.push(mean);
        }
        println!("target bench added: {:.3} ms mean", means[1] - means[0]);
    }
    if let Some(v) = bench {
        let frames = v[5] as usize;
        let mut means = Vec::new();
        for on in [false, true] {
            let (world, mut scenery) = world(&resources, &theater, on)?;
            use_gpu(&scenery, &mut gpu)?;
            let gpu = gpu.as_mut().expect("gpu");
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("City fill bench target"),
                size: wgpu::Extent3d {
                    width: size[0],
                    height: size[1],
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
            let view = texture.create_view(&Default::default());
            let geometry = std::sync::Arc::clone(scenery.static_geometry_where(&hidden_none));
            gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
            let heading = v[3].to_radians();
            let mut times = Vec::with_capacity(frames);
            let (mut full, mut boxes, mut samples) = (0usize, 0usize, 0usize);
            for frame in 0..frames + 30 {
                let along = frame.saturating_sub(30) as f64 * v[4];
                let x = v[0] + along * heading.sin();
                let z = v[1] + along * heading.cos();
                let mut camera = Camera::new();
                camera.yaw = heading as f32;
                camera.pitch = -0.12;
                camera.position = [x, f64::from(world.height(x as f32, z as f32)) + v[2], z];
                scenery.resolve_palette(&world, camera.position[1]);
                scenery.set_origin(camera.position);
                let started = Instant::now();
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                gpu.sim.draw(
                    &gpu.device,
                    &gpu.queue,
                    &mut encoder,
                    &view,
                    size,
                    &camera,
                    &world,
                    &scenery,
                );
                gpu.queue.submit([encoder.finish()]);
                gpu.device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: Some(Duration::from_secs(60)),
                })?;
                if frame >= 30 {
                    times.push(started.elapsed().as_secs_f64() * 1e3);
                    // Every 20th frame, untimed: what it drew.
                    if frame % 20 == 0
                        && let Some(f) = &gpu.sim.fill
                    {
                        let stats = f.stats(&gpu.device, &gpu.queue)?;
                        full += stats.full;
                        boxes += stats.boxes;
                        samples += 1;
                    }
                }
            }
            let mut sorted = times.clone();
            sorted.sort_by(f64::total_cmp);
            let mean = times.iter().sum::<f64>() / times.len() as f64;
            let p = |q: f64| sorted[((sorted.len() - 1) as f64 * q).round() as usize];
            println!(
                "bench fill {}: {} frames at {}x{}, mean {:.3} ms, p50 {:.3}, p95 {:.3}, max {:.3}, per frame {:.0} buildings at full detail and {:.0} as far boxes, fill GPU memory {} KiB",
                if on { "on" } else { "off" },
                times.len(),
                size[0],
                size[1],
                mean,
                p(0.5),
                p(0.95),
                p(1.0),
                full as f64 / samples.max(1) as f64,
                boxes as f64 / samples.max(1) as f64,
                gpu.sim.fill.as_ref().map_or(0, |f| f.gpu_bytes()) / 1024
            );
            means.push(mean);
        }
        println!("bench added: {:.3} ms mean", means[1] - means[0]);
    }
    Ok(())
}

/// Per city: the mask at one pixel per texel and the placed footprints at
/// 64 ft per pixel, both north up.
fn write_images(
    out: &Path,
    resources: &BTreeMap<String, Vec<u8>>,
    world: &Terrain,
    scenery: &Scenery,
    fill: &city::CityFill,
) -> AppResult<()> {
    let layout = &fill.layout;
    let cell = mask::TILE_FT / 4;
    let palette = scenery.palette;
    let base = tore_formats::theater::base_theater(&world.layout).unwrap_or("");
    let mut pictures = BTreeMap::<String, Vec<u8>>::new();
    for placement in world.environment.textures.values() {
        let name = placement.resource_name(base);
        if pictures.contains_key(&name) {
            continue;
        }
        if let Some(pic) = resources
            .get(&name)
            .and_then(|b| tore_formats::Pic::parse(b).ok())
            .filter(|p| p.width == 256 && p.height == 256)
        {
            let pixels = pic
                .pixels
                .iter()
                .zip(&pic.mask)
                .map(|(p, m)| if *m { *p } else { 255 })
                .collect();
            pictures.insert(name, pixels);
        }
    }
    // The texture index under a world point, as the mask reads it.
    let index_at = |x: i64, z: i64| -> u8 {
        let key = mask::LightMap::key(x, z);
        let Some(placement) = world.environment.textures.get(&key) else {
            return 255;
        };
        let Some(pic) = pictures.get(&placement.resource_name(base)) else {
            return 255;
        };
        let tu = ((x - i64::from(key.0) * cell) / mask::TEXEL_FT).clamp(0, 255) as usize;
        let tv = ((z - i64::from(key.1) * cell) / mask::TEXEL_FT).clamp(0, 255) as usize;
        let (col, row) = match placement.rotation % 4 {
            0 => (tu, 255 - tv),
            1 => (255 - tv, 255 - tu),
            2 => (255 - tu, tv),
            _ => (tv, tu),
        };
        pic[row * 256 + col]
    };
    for (n, city) in layout.cities.iter().enumerate() {
        let lo = [
            city.tiles.iter().map(|t| i64::from(t.0)).min().unwrap_or(0) * cell,
            city.tiles.iter().map(|t| i64::from(t.1)).min().unwrap_or(0) * cell,
        ];
        let hi = [
            city.tiles.iter().map(|t| i64::from(t.0)).max().unwrap_or(0) * cell + mask::TILE_FT,
            city.tiles.iter().map(|t| i64::from(t.1)).max().unwrap_or(0) * cell + mask::TILE_FT,
        ];
        let slug: String = city
            .name
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() {
                    c.to_ascii_lowercase()
                } else {
                    '-'
                }
            })
            .collect();
        for (kind, feet) in [("mask", mask::TEXEL_FT), ("fill", 64)] {
            let w = ((hi[0] - lo[0]) / feet) as usize;
            let h = ((hi[1] - lo[1]) / feet) as usize;
            let mut rgba = vec![255u8; w * h * 4];
            for py in 0..h {
                for px in 0..w {
                    let x = lo[0] + px as i64 * feet + feet / 2;
                    let z = hi[1] - py as i64 * feet - feet / 2;
                    let flags = layout.map.flags_at(x, z);
                    let c = palette[usize::from(index_at(x, z))];
                    let dim = [c[0] / 3, c[1] / 3, c[2] / 3];
                    let rgb = if kind == "fill" {
                        dim
                    } else if flags & mask::STREET != 0 {
                        [230, 40, 40]
                    } else if flags & mask::SOLID != 0 {
                        [255, 255, 255]
                    } else if flags & mask::SITE != 0 {
                        [225, 205, 150]
                    } else if flags & mask::GREEN != 0 {
                        [30, 110, 50]
                    } else if flags & mask::WATER != 0 {
                        [30, 50, 150]
                    } else {
                        dim
                    };
                    let at = (py * w + px) * 4;
                    rgba[at..at + 3].copy_from_slice(&rgb);
                }
            }
            if kind == "fill" {
                for (i, inst) in layout.instances.iter().enumerate() {
                    if usize::from(inst.city) != n {
                        continue;
                    }
                    let piece = &layout.kit.pieces[usize::from(inst.piece)];
                    let color = if fill.hidden[i] {
                        [90, 90, 90]
                    } else {
                        class_color(piece.class)
                    };
                    let heading = i32::from(inst.heading);
                    let r = piece.half[0].max(piece.half[1]) * 3 / 2;
                    let mut sx = -r;
                    while sx <= r {
                        let mut sz = -r;
                        while sz <= r {
                            let local = trig::to_local_scaled([sx, sz], heading);
                            if local[0].abs() <= piece.half[0] * trig::ONE
                                && local[1].abs() <= piece.half[1] * trig::ONE
                            {
                                let px = (i64::from(inst.x) + sx - lo[0]) / feet;
                                let py = (hi[1] - i64::from(inst.z) - sz) / feet;
                                if (0..w as i64).contains(&px) && (0..h as i64).contains(&py) {
                                    let at = (py as usize * w + px as usize) * 4;
                                    rgba[at..at + 3].copy_from_slice(&color);
                                }
                            }
                            sz += feet / 2;
                        }
                        sx += feet / 2;
                    }
                }
            }
            let path = out.join(format!("{kind}-{slug}.png"));
            std::fs::write(
                &path,
                crate::replay::png::encode_rgba(w as u32, h as u32, &rgba)?,
            )?;
            println!("image: {}", path.display());
        }
    }
    Ok(())
}
