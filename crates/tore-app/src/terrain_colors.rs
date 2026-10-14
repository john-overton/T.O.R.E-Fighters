//! Headless ground-colour regression against each retail MM's explicit palette.
use crate::{
    AppResult,
    camera::Camera,
    reel::{Gpu, HEIGHT, WIDTH},
    scenery::Scenery,
    terrain::{Overrides, Terrain},
};
use tore_formats::{
    theater::{Environment as MapEnvironment, THEATERS},
    weather::Module,
};
use tore_sim::environment::{CONDITIONS, Configuration, Environment};

fn mean(cells: &[tore_formats::theater::TerrainCell], palette: &[[u8; 3]; 256]) -> [f64; 3] {
    let mut sum = [0.; 3];
    let mut count = 0_f64;
    for cell in cells.iter().filter(|c| c.color != 255) {
        for (channel, value) in sum.iter_mut().zip(palette[usize::from(cell.color)]) {
            *channel += f64::from(value);
        }
        count += 1.;
    }
    sum.map(|v| v / count.max(1.))
}

pub fn run() -> AppResult<()> {
    let mut args: Vec<_> = std::env::args().skip(2).collect();
    let cpu = args.last().is_some_and(|s| s == "--cpu");
    if cpu {
        args.pop();
    }
    let render = match args.as_slice() {
        [] => None,
        [out, theater] => Some((std::path::Path::new(out), theater.as_str())),
        _ => return Err("--terrain-colors [OUT_DIR THEATER [--cpu]]".into()),
    };
    if render.is_some_and(|(_, selected)| !THEATERS.iter().any(|(code, _)| *code == selected)) {
        return Err("unknown base theater".into());
    }
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    let mut failures = Vec::new();
    for &(code, _) in THEATERS {
        if render.is_some_and(|(_, selected)| selected != code) {
            continue;
        }
        let mm = MapEnvironment::parse(resources.get(&format!("{code}.MM")).ok_or("missing MM")?)?;
        let suffix = mm
            .layer
            .strip_prefix("DAY2")
            .and_then(|s| s.strip_suffix(".LAY"))
            .ok_or("base MM needs DAY2 layer")?;
        for (condition, choice) in CONDITIONS.iter().enumerate() {
            if render.is_some() && !matches!(condition, 0 | 5) {
                continue;
            }
            let overrides = Overrides {
                city_fill: false,
                cloud_altitude: Some(0),
                ..Overrides::default()
            };
            let world = Terrain::for_mission(resources, code, Some(condition), &overrides)?;
            let expected_name = format!("{}{suffix}.LAY", choice.layer);
            let module = Module::parse(
                resources
                    .get(&expected_name)
                    .ok_or("missing reference LAY")?,
            )?;
            let reference = Environment::new(Configuration::new(
                module,
                choice.seconds_of_day / 3600,
                choice.seconds_of_day / 60 % 60,
                condition as i32,
                mm.wind,
            )?);
            for altitude in [0., 6000., 30000.] {
                let actual = world
                    .weather
                    .palette(altitude)
                    .ok_or("missing actual palette")?;
                let expected = reference
                    .palette(altitude)
                    .ok_or("missing reference palette")?;
                let rgb = mean(&world.theater.cells, &actual);
                let expected_rgb = mean(&world.theater.cells, &expected);
                let ok = actual[192..] == expected[192..];
                println!(
                    "Ground colours: {code} condition={condition} altitude={altitude} layer={} mean={rgb:.2?} expected={expected_rgb:.2?} {}",
                    world.environment.layer,
                    if ok { "PASS" } else { "FAIL" }
                );
                if !ok {
                    failures.push(format!("{code}/{condition}/{altitude}"));
                }
            }
            if let Some((out, _)) = render {
                std::fs::create_dir_all(out)?;
                let mut scene = Scenery::build(resources, &world)?;
                let mut gpu = if cpu {
                    None
                } else {
                    Some(pollster::block_on(Gpu::new(&scene))?)
                };
                // Fixed central land cell, shared by before/after and both times.
                let cell = world
                    .theater
                    .cells
                    .iter()
                    .enumerate()
                    .filter(|(_, c)| c.color != 255)
                    .min_by_key(|(i, _)| {
                        let x = i % world.theater.cols;
                        let z = i / world.theater.cols;
                        x.abs_diff(world.theater.cols / 2) + z.abs_diff(world.theater.rows / 2)
                    })
                    .ok_or("no land")?
                    .0;
                let x = (cell % world.theater.cols) as f64 * 8192.;
                let z = (cell / world.theater.cols) as f64 * 8192.;
                let y = f64::from(world.height(x as f32, z as f32));
                for (view, height) in [("near", 1800.), ("far", 18000.)] {
                    let mut camera = Camera::new();
                    camera.position = [x, y + height, z];
                    camera.yaw = 0.;
                    camera.pitch = -0.65;
                    scene.resolve_palette(&world, camera.position[1]);
                    scene.set_origin(camera.position);
                    let pixels = if let Some(gpu) = &mut gpu {
                        gpu.pixels(&camera, &world, &scene)?
                    } else {
                        cpu_ground(
                            &scene,
                            [x, z],
                            if view == "near" { 32768. } else { 262144. },
                        )
                    };
                    let name = format!(
                        "{code}-{}-{view}.png",
                        if condition == 0 { "day" } else { "night" }
                    );
                    std::fs::write(
                        out.join(name),
                        crate::replay::png::encode_rgba(WIDTH, HEIGHT, &pixels)?,
                    )?;
                }
            }
        }
    }
    if !failures.is_empty() {
        return Err(format!("Ground colour mismatches: {}", failures.join(", ")).into());
    }
    println!("Ground colours validated against retail map palettes");
    Ok(())
}

/// Unlit orthographic CPU diagnostic of the actual terrain mesh and uploaded
/// indexed textures. No GPU lighting, fog, scenery objects or distance filter.
/// Used when the execution host cannot expose a Vulkan adapter.
fn cpu_ground(scene: &Scenery, center: [f64; 2], span: f64) -> Vec<u8> {
    let mut pixels = vec![0; (WIDTH * HEIGHT * 4) as usize];
    for p in pixels.chunks_exact_mut(4) {
        p.copy_from_slice(&[40, 60, 80, 255]);
    }
    let scale = f64::from(HEIGHT) / span;
    for triangle in scene.vertices.chunks_exact(30) {
        let v: [&[f32]; 3] = [&triangle[..10], &triangle[10..20], &triangle[20..]];
        let screen = v.map(|p| {
            [
                (f64::from(p[0]) - center[0]) * scale + f64::from(WIDTH) / 2.,
                (center[1] - f64::from(p[2])) * scale + f64::from(HEIGHT) / 2.,
            ]
        });
        let lo: [i32; 2] = std::array::from_fn(|i| {
            screen
                .iter()
                .map(|p| p[i].floor() as i32)
                .min()
                .unwrap()
                .max(0)
        });
        let hi: [i32; 2] = std::array::from_fn(|i| {
            screen
                .iter()
                .map(|p| p[i].ceil() as i32)
                .max()
                .unwrap()
                .min(if i == 0 { WIDTH } else { HEIGHT } as i32 - 1)
        });
        let edge = |a: [f64; 2], b: [f64; 2], p: [f64; 2]| {
            (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
        };
        let area = edge(screen[0], screen[1], screen[2]);
        if area.abs() < 1e-8 {
            continue;
        }
        for y in lo[1]..=hi[1] {
            for x in lo[0]..=hi[0] {
                let p = [f64::from(x) + 0.5, f64::from(y) + 0.5];
                let w = [
                    edge(screen[1], screen[2], p) / area,
                    edge(screen[2], screen[0], p) / area,
                    edge(screen[0], screen[1], p) / area,
                ];
                if w.iter().any(|w| *w < -1e-8) {
                    continue;
                }
                let mut index = v[0][9] as usize;
                let layer = v[0][5] as i32;
                if layer >= 0 {
                    let uv: [usize; 2] = std::array::from_fn(|i| {
                        let t: f64 = (0..3).map(|j| w[j] * f64::from(v[j][3 + i])).sum();
                        (t.clamp(0., 1.) * 256.).floor().min(255.) as usize
                    });
                    index = usize::from(
                        scene.texture_indices[layer as usize * 65536 + uv[1] * 256 + uv[0]],
                    );
                }
                if index == 255 {
                    continue;
                }
                let at = (y as usize * WIDTH as usize + x as usize) * 4;
                pixels[at..at + 3].copy_from_slice(&scene.palette[index]);
            }
        }
    }
    pixels
}
