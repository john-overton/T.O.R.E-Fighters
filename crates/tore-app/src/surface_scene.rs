//! `--surface-scene OUT_DIR THEATER STEM [--surface-seed N] [--seconds S]
//! [--surface-only] [--kill all|LIST] [--burn S] [--rails N] [--focus LIST]
//! [--views LIST] [--look YAW,PITCH] [--tag NAME] [--distance FT]
//! [--defenses AAA SAM]`: renders a
//! Quick Mission ground target as the game draws it, offscreen at 1080p, for
//! review (docs/spec/surface-defenses.md, "Destroyed looks and drawing").
//!
//! It builds the mission, steps the whole world `S` seconds (the live game's
//! tick, no window, no audio; `--surface-only` steps just the surface
//! movement, for long marches), then optionally destroys units (`--kill`:
//! `all` the template's units and parked aircraft, or a comma list of
//! template ordinals, unit types such as `SA3.NT` and `within:FEET` of the
//! focus) and lets them burn
//! for `--burn` seconds of whole-world ticks, and sets every launcher's
//! rails to at most `--rails` rounds. Each view (`oblique`, `close`, `low`,
//! `top`, `deck`) frames the focus (`--focus`: `routed`, `parked`, `all`, or
//! ordinals and unit types; default every template unit; `--distance` sets
//! the radius held in view, `--look` a view of its own) and is written as
//! `TAG-VIEW.png` with the static scene, the moving surface units, the men
//! and deck crew, explosions, fires and smoke.
use crate::{AppResult, camera::Camera, reel::Gpu, scenery::Scenery, surface_dump};
use std::path::Path;
use tore_formats::aircraft::AircraftId;
use tore_world::{
    mission::{Defense, MissionSpec},
    seats::SeatInput,
    surface::{SURFACE_UNIT_BASE, UnitId},
    world::{Seating, TickOutput, World},
};

struct Options {
    out: String,
    theater: String,
    stem: String,
    seed: u32,
    seconds: f64,
    surface_only: bool,
    kill: Vec<String>,
    burn: f64,
    rails: Option<u32>,
    focus: Vec<String>,
    views: Vec<String>,
    tag: Option<String>,
    distance: Option<f64>,
    defenses: (Defense, Defense),
    /// A view of its own: yaw and pitch in degrees.
    look: Option<(f64, f64)>,
}

fn options() -> AppResult<Options> {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let mut positional = Vec::new();
    let mut o = Options {
        out: String::new(),
        theater: String::new(),
        stem: String::new(),
        seed: 1,
        seconds: 1.,
        surface_only: false,
        kill: Vec::new(),
        burn: 20.,
        rails: None,
        focus: Vec::new(),
        views: vec!["oblique".into(), "close".into()],
        tag: None,
        distance: None,
        defenses: (Defense::Heavy, Defense::Heavy),
        look: None,
    };
    let list = |text: String| -> Vec<String> {
        text.split(',')
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()
    };
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        let mut next = || it.next().ok_or_else(|| format!("{arg} needs a value"));
        match arg.as_str() {
            "--surface-seed" => o.seed = next()?.parse()?,
            "--seconds" => o.seconds = next()?.parse()?,
            "--surface-only" => o.surface_only = true,
            "--kill" => o.kill = list(next()?),
            "--burn" => o.burn = next()?.parse()?,
            "--rails" => o.rails = Some(next()?.parse()?),
            "--focus" => o.focus = list(next()?),
            "--views" => o.views = list(next()?),
            "--tag" => o.tag = Some(next()?),
            "--distance" => o.distance = Some(next()?.parse()?),
            "--look" => {
                let text = next()?;
                let (yaw, pitch) = text.split_once(',').ok_or("--look YAW,PITCH")?;
                o.look = Some((yaw.parse()?, pitch.parse()?));
                o.views = vec!["look".into()];
            }
            "--defenses" => {
                let level = |text: String| {
                    Defense::parse(&text).ok_or_else(|| format!("unknown defense level {text}"))
                };
                o.defenses = (level(next()?)?, level(next()?)?);
            }
            other if other.starts_with("--") => return Err(format!("unknown {other}").into()),
            _ => positional.push(arg),
        }
    }
    let [out, theater, stem] = positional.as_slice() else {
        return Err("--surface-scene OUT_DIR THEATER STEM [--surface-seed N] [--seconds S] [--surface-only] [--kill all|LIST] [--burn S] [--rails N] [--focus LIST] [--views LIST] [--look YAW,PITCH] [--tag NAME] [--distance FT] [--defenses AAA SAM]".into());
    };
    o.out = out.clone();
    o.theater = theater.to_ascii_uppercase();
    o.stem = stem.trim_start_matches('~').to_ascii_uppercase();
    Ok(o)
}

/// Whether template object `id` (unit type `resource`) is named by `list`:
/// `all`, its ordinal, or its type.
fn named(list: &[String], id: u32, resource: &str) -> bool {
    list.iter().any(|item| {
        item == "all"
            || item.parse::<u32>().ok() == Some(id.wrapping_sub(SURFACE_UNIT_BASE))
            || item.eq_ignore_ascii_case(resource)
    })
}

/// Where every unit of the template (with the trucks and radars the layout
/// added) and every parked aircraft stands now.
fn places(world: &World) -> Vec<(u32, String, [f64; 3], bool)> {
    let surface = &world.terrain.surface;
    let mut out: Vec<(u32, String, [f64; 3], bool)> = surface
        .units
        .iter()
        .filter(|unit| unit.in_scene && unit.id.range() != tore_world::surface::IdRange::Layout)
        .map(|unit| {
            let pose = tore_world::surface::movement::unit_pose(
                unit,
                world.combat.surface.unit(unit.id),
                &world.terrain,
            );
            (
                unit.id.0,
                unit.resource.clone(),
                pose.position,
                surface.courses.contains_key(&unit.id),
            )
        })
        .collect();
    out.extend(surface.parked_scene.iter().map(|pose| {
        (
            pose.id.0,
            format!("parked {}", pose.resource),
            pose.ground,
            false,
        )
    }));
    out
}

/// A camera looking along `yaw` (radians, 0 north) down `pitch` at
/// `center`, back far enough to hold `radius` feet around it.
fn framed(center: [f64; 3], radius: f64, yaw: f64, pitch: f64) -> Camera {
    let mut camera = Camera::new();
    camera.yaw = yaw as f32;
    camera.pitch = pitch as f32;
    // A 60 degree high view: tan 30 degrees holds the radius, with margin.
    let distance = (radius.max(30.) * 1.25 / 30f64.to_radians().tan()).max(60.);
    let (sy, cy) = yaw.sin_cos();
    let (sp, cp) = pitch.sin_cos();
    let forward = [sy * cp, sp, cy * cp];
    camera.position = std::array::from_fn(|i| center[i] - forward[i] * distance);
    camera.near_clip = 1.;
    camera
}

pub fn run() -> AppResult<()> {
    let o = options()?;
    let out = Path::new(&o.out);
    std::fs::create_dir_all(out)?;
    let mut resources = crate::reel::load_assets()?.theater_resources;
    surface_dump::with_retail_surface(&mut resources)?;
    let mut spec = MissionSpec::new(&o.theater, AircraftId::F18);
    spec.ground_target = Some(o.stem.clone());
    spec.surface_seed = o.seed;
    (spec.aaa, spec.sam) = o.defenses;
    let mut world = World::new(&spec, &resources, Seating::SinglePlayer)?;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    let mut output = TickOutput::default();
    let mut step = |world: &mut World, ticks: u64, surface_only: bool| -> AppResult<()> {
        for _ in 0..ticks {
            if surface_only {
                world.combat.step_surface(&world.terrain);
            } else {
                let input = SeatInput {
                    tick: world.tick(),
                    ..SeatInput::default()
                };
                world.step(&[input], &mut output)?;
            }
        }
        Ok(())
    };
    step(
        &mut world,
        (o.seconds * 120.).round() as u64,
        o.surface_only,
    )?;
    // One whole-world tick takes the picture the views draw.
    step(&mut world, 1, false)?;
    let all = places(&world);
    let focus: Vec<(u32, String, [f64; 3], bool)> = all
        .into_iter()
        .filter(|(id, resource, _, routed)| {
            o.focus.is_empty()
                || o.focus.iter().any(|item| match item.as_str() {
                    "routed" => *routed,
                    "parked" => resource.starts_with("parked "),
                    _ => named(std::slice::from_ref(item), *id, resource),
                })
        })
        .collect();
    if focus.is_empty() {
        return Err("nothing to frame".into());
    }
    let center: [f64; 3] = std::array::from_fn(|i| {
        focus.iter().map(|(_, _, p, _)| p[i]).sum::<f64>() / focus.len() as f64
    });
    let radius = o.distance.unwrap_or_else(|| {
        focus
            .iter()
            .map(|(_, _, p, _)| (p[0] - center[0]).hypot(p[2] - center[2]))
            .fold(0., f64::max)
            + 40.
    });
    if !o.kill.is_empty() {
        let doomed: Vec<u32> = places(&world)
            .into_iter()
            .filter(|(id, resource, at, _)| {
                named(&o.kill, *id, resource.trim_start_matches("parked "))
                    || o.kill.iter().any(|item| {
                        item.strip_prefix("within:")
                            .and_then(|feet| feet.parse::<f64>().ok())
                            .is_some_and(|feet| {
                                (at[0] - center[0]).hypot(at[2] - center[2]) <= feet
                            })
                    })
            })
            .map(|(id, ..)| id)
            .collect();
        for target in world
            .combat
            .state
            .targets
            .iter_mut()
            .filter(|t| doomed.contains(&t.id))
        {
            target.hp = 0;
        }
        println!("surface-scene: destroyed {} objects", doomed.len());
        step(&mut world, (o.burn * 120.).round() as u64, false)?;
    }
    if let Some(rails) = o.rails {
        let launchers: Vec<UnitId> = world
            .terrain
            .surface
            .arsenal
            .units
            .iter()
            .map(|arms| arms.unit)
            .collect();
        for id in launchers {
            if let Some(unit) = world.combat.surface.unit_mut(id) {
                for mount in &mut unit.mounts {
                    mount.loaded = mount.loaded.min(rails);
                }
            }
        }
    }
    let mut scenery = Scenery::build(&resources, &world.terrain)?;
    let art = crate::render_snapshot::CombatArt::load(&resources)?;
    let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
    for (id, resource, at, routed) in &focus {
        println!(
            "surface-scene: {id:#010x} {resource} at {:.0} {:.0} {:.0}{}",
            at[0],
            at[1],
            at[2],
            if *routed { " routed" } else { "" }
        );
    }
    let snapshot = world.combat.render_snapshot().clone();
    let standing = crate::render_snapshot::standing(&world.combat.state.targets);
    let tag = o
        .tag
        .clone()
        .unwrap_or_else(|| format!("{}-{}", o.theater, o.stem));
    for view in &o.views {
        let (yaw, pitch, scale) = match view.as_str() {
            "oblique" => (45f64, -25f64, 1.),
            "close" => (45., -20., 0.45),
            "low" => (135., -8., 0.6),
            "top" => (0., -80., 1.),
            "deck" => (300., -12., 0.35),
            "look" => {
                let (yaw, pitch) = o.look.ok_or("the look view needs --look")?;
                (yaw, pitch, 1.)
            }
            other => return Err(format!("unknown view {other}").into()),
        };
        let camera = framed(center, radius * scale, yaw.to_radians(), pitch.to_radians());
        scenery.resolve_palette(&world.terrain, camera.position[1]);
        scenery.set_origin(camera.position);
        let geometry = std::sync::Arc::clone(
            scenery.static_geometry(&world.combat.state.targets, &world.combat.surface),
        );
        gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
        gpu.sim.surface_units(
            &gpu.device,
            &gpu.queue,
            &scenery.surface_vertices(&snapshot, &|id| standing.contains(&id), &camera),
        );
        let contrails = tore_sim::combat::smoke::Smoke::default();
        gpu.sim.smoke(
            &gpu.device,
            &gpu.queue,
            &art.smoke,
            [&world.combat.state.smoke, &contrails],
            &world.combat.state.devices,
        );
        gpu.sim.effects(
            &gpu.device,
            &gpu.queue,
            &art.effects,
            &snapshot.effects,
            &snapshot.marks,
        );
        let pixels = gpu.pixels(&camera, &world.terrain, &scenery)?;
        let path = out.join(format!("{tag}-{view}.png"));
        std::fs::write(
            &path,
            crate::replay::png::encode_rgba(crate::reel::WIDTH, crate::reel::HEIGHT, &pixels)?,
        )?;
        println!("surface-scene: {}", path.display());
    }
    Ok(())
}
