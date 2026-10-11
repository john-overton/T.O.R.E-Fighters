//! `--airfield-sheets OUTPUT_DIRECTORY [--all] [THEATER ...]`: offscreen
//! renders of the redrawn airports (experiment AP1,
//! docs/formats/redrawn-airports.md), one airport of each plan (`--all`:
//! every one), overhead, oblique, from short final, along the parking
//! row, down the runway to its far end and over the first taxiway
//! junction, at noon with no clouds. The airports
//! and cameras are chosen from the redrawn scene; the scene drawn follows
//! `TORE_REDRAWN_AIRPORTS`, so a run with it unset renders the same views of
//! the retail airfields for a before and after pair.
use crate::{
    AppResult,
    camera::Camera,
    reel::{Gpu, HEIGHT, WIDTH},
    scenery::Scenery,
    terrain::{Overrides, Terrain},
};
use std::path::Path;

pub fn run() -> AppResult<()> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let Some((out, wanted)) = args.split_first() else {
        return Err("--airfield-sheets OUTPUT_DIRECTORY [--all] [THEATER ...]".into());
    };
    let all = wanted.iter().any(|w| w == "--all");
    let mut theaters: Vec<String> = wanted
        .iter()
        .filter(|w| *w != "--all")
        .map(|w| w.to_ascii_uppercase())
        .collect();
    if theaters.is_empty() {
        theaters = tore_formats::quick_template::tables::THEATERS
            .iter()
            .map(|t| (*t).to_owned())
            .collect();
    }
    let out = Path::new(out);
    std::fs::create_dir_all(out)?;
    let resources = crate::reel::load_assets()?.theater_resources;
    let overrides = |redrawn_airports: bool| Overrides {
        time: Some([12, 0]),
        wind: None,
        cloud_altitude: Some(0),
        redrawn_airports,
    };
    let drawn = crate::scenery::redrawn_airports()?;
    let mut seen = std::collections::BTreeSet::new();
    let mut gpu: Option<Gpu> = None;
    for theater in theaters {
        let plan_world = Terrain::for_mission(&resources, &theater, Some(0), &overrides(true))?;
        let chosen: Vec<_> = plan_world
            .redrawn
            .iter()
            .filter(|b| !b.patches.is_empty())
            .filter(|b| all || seen.insert(b.plan.clone()))
            .cloned()
            .collect();
        if chosen.is_empty() {
            continue;
        }
        let world = Terrain::for_mission(&resources, &theater, Some(0), &overrides(drawn))?;
        let mut scenery = Scenery::build(&resources, &world)?;
        let gpu = match &mut gpu {
            Some(gpu) => {
                gpu.sim = crate::sim_renderer::SimRenderer::new(
                    &gpu.device,
                    &gpu.queue,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    &scenery,
                    crate::graphics::Options::default(),
                    4,
                );
                gpu
            }
            None => gpu.insert(pollster::block_on(Gpu::new(&scenery))?),
        };
        for built in chosen {
            let name = plan_world
                .airport_scene
                .runway(built.strip_id)
                .map_or_else(String::new, |r| r.name.clone());
            let box_ = built.surface;
            let reach = box_.half[0].max(box_.half[2]);
            let heading = box_.heading;
            // Overhead and oblique views of the whole field, then the near
            // threshold from short final and the parking row from the
            // taxiway side.
            let mut views = vec![
                ("top", box_.center, heading, 84.0f64, 1.9 * reach),
                ("oblique", box_.center, heading, 24.0, 2.3 * reach),
            ];
            // The far end of the ILS runway (on a pair, where it runs on
            // into the second tile).
            if let Some(runway) = plan_world.airport_scene.runway(built.strip_id) {
                let far = built.frame.world([0., runway.length_ft]);
                views.push(("far-end", far, heading, 10.0, 1800.));
            }
            if let Some(anchors) = &built.anchors {
                // The taxi-out corner, a junction with its curved corners.
                views.push((
                    "junction",
                    anchors.taxi_out[1],
                    heading + std::f64::consts::FRAC_PI_4,
                    32.0,
                    700.,
                ));
                views.push(("threshold", built.frame.origin, heading, 9.0, 900.));
                let nose = anchors.parking_heading;
                let slot = anchors.parking[4];
                views.push(("apron", slot, nose + std::f64::consts::PI, 14.0, 520.));
            }
            for (view, target, yaw, pitch_deg, distance) in views {
                let pitch = pitch_deg.to_radians();
                let (s, c) = yaw.sin_cos();
                let mut camera = Camera::new();
                camera.yaw = yaw as f32;
                camera.pitch = -pitch as f32;
                let (back, up) = (distance * pitch.cos(), distance * pitch.sin());
                camera.position = [
                    target[0] - back * s,
                    box_.center[1] + up,
                    target[2] - back * c,
                ];
                scenery.resolve_palette(&world, camera.position[1]);
                scenery.set_origin(camera.position);
                let hidden = std::collections::BTreeSet::new();
                let geometry = std::sync::Arc::clone(scenery.static_geometry_where(&hidden));
                gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
                let pixels = gpu.pixels(&camera, &world, &scenery)?;
                let plan = built.plan.replace(".OT", "").replace('+', "-");
                let path = out.join(format!(
                    "{theater}-{plan}-{:x}-{view}-{}.png",
                    built.strip_id,
                    if drawn { "after" } else { "before" }
                ));
                std::fs::write(
                    &path,
                    crate::replay::png::encode_rgba(WIDTH, HEIGHT, &pixels)?,
                )?;
                let (moved, added) = built.building_counts();
                println!(
                    "airfield sheet: {} plan={} airport={name:?} buildings moved={moved} added={added} lights={}",
                    path.display(),
                    built.plan,
                    built.lights.len()
                );
            }
        }
    }
    Ok(())
}
