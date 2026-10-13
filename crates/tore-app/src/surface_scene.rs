//! `--surface-scene OUT_DIR THEATER STEM [--surface-seed N] [--seconds S]
//! [--surface-only] [--kill all|LIST] [--burn S] [--rails N] [--focus LIST]
//! [--views LIST] [--look YAW,PITCH] [--tag NAME] [--distance FT]
//! [--defenses AAA SAM] [--intruder EAST,UP,NORTH] [--blue LIST] [--no-aim]
//! [--time HH:MM]`:
//! renders a
//! Quick Mission ground target as the game draws it, offscreen at 1080p, for
//! review (docs/spec/surface-defenses.md, "Destroyed looks and drawing").
//!
//! It builds the mission, steps the whole world `S` seconds (the live game's
//! tick, no window, no audio; `--surface-only` steps just the surface
//! movement, for long marches), then optionally destroys units (`--kill`:
//! `all` the template's units and parked aircraft, or a comma list of
//! template ordinals, unit types such as `SA3.NT`, `within:FEET` of the
//! focus and `runways`, every runway object, the layout's too, within the
//! framed distance) and lets them burn
//! for `--burn` seconds of whole-world ticks, and sets every launcher's
//! rails to at most `--rails` rounds. Each view (`oblique`, `close`, `low`,
//! `top`, `deck`) frames the focus (`--focus`: `routed`, `parked`, `all`, or
//! ordinals and unit types; default every template unit; `--distance` sets
//! the radius held in view, `--look` a view of its own) and is written as
//! `TAG-VIEW.png` with the static scene, the moving surface units, the men
//! and deck crew, explosions, fires and smoke.
//!
//! The units aim as the game draws them (docs/spec/surface-defenses.md,
//! "Aiming"), stepped every tick; `--no-aim` draws them unturned, as before
//! aiming was drawn. `--intruder` holds the player's aircraft (invulnerable)
//! circling 3,000 ft round the point that many feet east, up and north of
//! the focus at 300 knots, so the defenses lock on and fire at it;
//! `--blue` hands template objects (ordinals) to Blue, as `--surface-battle`
//! does, so units fight each other.
//!
//! The scene's other static objects (buildings, bridges, runways) are named
//! `object:TYPE` (`object:HANGR.OT`) in `--focus` and `--kill`, and
//! `within:FEET` takes them too, so a destroyed airport's hangars, tower and
//! fuel tanks can be framed burning (B16). `--bomb-runway N` drops N Mk 82s
//! along the runway nearest the focus, then steps `--burn` seconds (at
//! least 12), to show the craters a bombed runway keeps. `--focus near:ITEM` frames only
//! the one ITEM nearest the template (`near:object:BR1MID.OT`). `--time 22:00` starts the mission
//! at night. `--flatten FEET` knocks the city fill's buildings within FEET of
//! the focus flat so they burn (nr-fx2), and `--frames N` times N more frames
//! of each view.
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
    /// Feet east, up and north of the focus the player's aircraft circles.
    intruder: Option<[f64; 3]>,
    blue: Vec<u32>,
    no_aim: bool,
    /// Mission start time, hours and minutes (`--time 22:00` for night).
    time: Option<[i32; 2]>,
    /// Mk 82s dropped along the runway nearest the focus (B16 craters).
    bomb_runway: usize,
    /// Knock every city fill building within this many feet of the focus
    /// flat, so it burns (nr-fx2).
    flatten: Option<f64>,
    /// Extra frames drawn per view to time them (nr-fx2).
    frames: usize,
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
        intruder: None,
        blue: Vec::new(),
        no_aim: false,
        time: None,
        bomb_runway: 0,
        flatten: None,
        frames: 0,
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
            "--flatten" => o.flatten = Some(next()?.parse()?),
            "--frames" => o.frames = next()?.parse()?,
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
            "--intruder" => {
                let text = next()?;
                let parts: Vec<f64> = text.split(',').map(str::parse).collect::<Result<_, _>>()?;
                let [east, up, north] = parts.as_slice() else {
                    return Err("--intruder EAST,UP,NORTH".into());
                };
                o.intruder = Some([*east, *up, *north]);
            }
            "--blue" => {
                for ordinal in list(next()?) {
                    o.blue.push(ordinal.parse()?);
                }
            }
            "--no-aim" => o.no_aim = true,
            "--bomb-runway" => o.bomb_runway = next()?.parse()?,
            "--time" => {
                let text = next()?;
                let (h, m) = text.split_once(':').ok_or("--time HH:MM")?;
                o.time = Some([h.parse()?, m.parse()?]);
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
        return Err("--surface-scene OUT_DIR THEATER STEM [--surface-seed N] [--seconds S] [--surface-only] [--kill all|LIST] [--burn S] [--rails N] [--focus LIST] [--views LIST] [--look YAW,PITCH] [--tag NAME] [--distance FT] [--defenses AAA SAM] [--intruder EAST,UP,NORTH] [--blue LIST] [--no-aim] [--time HH:MM] [--bomb-runway N]".into());
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
        (item == "all" && !resource.starts_with("object:"))
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
    // The scene's other static objects (buildings, bridges, runways), as
    // `object:TYPE` (B16): named only on purpose, never by default.
    let known: std::collections::BTreeSet<u32> = out.iter().map(|(id, ..)| *id).collect();
    out.extend(
        world
            .terrain
            .airport_scene
            .objects
            .iter()
            .filter(|object| {
                !known.contains(&object.id) && surface.unit(UnitId(object.id)).is_none()
            })
            .map(|object| {
                (
                    object.id,
                    format!("object:{}", object.object_type),
                    object.bounds.center,
                    false,
                )
            }),
    );
    out
}

/// The places `o.focus` names (every template unit when it names none).
/// `near:ITEM` names only the one place ITEM names nearest the template's
/// units (a theater has many bridges of one type).
fn focused(
    o: &Options,
    all: Vec<(u32, String, [f64; 3], bool)>,
) -> Vec<(u32, String, [f64; 3], bool)> {
    let matches = |item: &str, id: u32, resource: &str, routed: bool| match item {
        "routed" => routed,
        "parked" => resource.starts_with("parked "),
        _ => named(&[item.to_owned()], id, resource),
    };
    let units: Vec<_> = all
        .iter()
        .filter(|(_, resource, ..)| !resource.starts_with("object:"))
        .cloned()
        .collect();
    let middle = centre(&units);
    let mut nearest = std::collections::BTreeSet::new();
    for item in &o.focus {
        if let Some(item) = item.strip_prefix("near:") {
            let best = all
                .iter()
                .filter(|(id, resource, _, routed)| matches(item, *id, resource, *routed))
                .min_by(|a, b| {
                    let d = |p: &[f64; 3]| (p[0] - middle[0]).hypot(p[2] - middle[2]);
                    d(&a.2).total_cmp(&d(&b.2))
                });
            nearest.extend(best.map(|(id, ..)| *id));
        }
    }
    all.into_iter()
        .filter(|(id, resource, _, routed)| {
            (o.focus.is_empty() && !resource.starts_with("object:"))
                || nearest.contains(id)
                || o.focus
                    .iter()
                    .filter(|item| !item.starts_with("near:"))
                    .any(|item| matches(item, *id, resource, *routed))
        })
        .collect()
}

/// The middle of `places`.
fn centre(places: &[(u32, String, [f64; 3], bool)]) -> [f64; 3] {
    std::array::from_fn(|i| {
        places.iter().map(|(_, _, p, _)| p[i]).sum::<f64>() / places.len().max(1) as f64
    })
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

/// Drops `count` Mk 82s on the runway nearest `center`, spaced along its
/// middle 70 percent, each from a 45 degree dive (B16: a bombed runway shows
/// its craters).
fn bomb_runway(
    world: &mut World,
    resources: &std::collections::BTreeMap<String, Vec<u8>>,
    center: [f64; 3],
    count: usize,
) -> AppResult<()> {
    use tore_sim::combat::{FallState, live::Projectile};
    let runway = world
        .terrain
        .airport_scene
        .runways
        .iter()
        .min_by(|a, b| {
            let d = |r: &tore_sim::airport::Runway| {
                (r.approach_center[0] - center[0]).hypot(r.approach_center[2] - center[2])
            };
            d(a).total_cmp(&d(b))
        })
        .ok_or("--bomb-runway: no runway")?
        .clone();
    let bomb = {
        let bytes = resources.get("MK82.JT").ok_or("missing MK82.JT")?;
        tore_formats::weapons::Weapon::parse("MK82.JT", bytes)?
    };
    let state = &mut world.combat.state;
    for n in 0..count {
        let along = (n as f64 + 0.5) / count as f64 * 0.7 - 0.35;
        let x = runway.approach_center[0] + runway.heading.sin() * runway.length_ft * along;
        let z = runway.approach_center[2] + runway.heading.cos() * runway.length_ft * along;
        let y = runway
            .support_height(x, z)
            .unwrap_or(runway.approach_center[1]);
        let release = crate::surface_objective::dive_release([x, y, z], [0.; 3], runway.heading);
        let from = release.position;
        let speed = release.speed();
        state.projectiles.push(Projectile {
            id: 0x00F8_0000 + n as u32,
            owner: 0,
            weapon: Some(bomb.clone()),
            guidance: None,
            motion: None,
            guidance_ticks: None,
            age: 0,
            incoming: None,
            station: 0,
            position: from,
            previous: from,
            direction: release.velocity.map(|v| v / speed),
            speed_f8: (speed * 256.) as i32,
            launched_t: (state.tick() / 30) as u16,
            target: None,
            fall: FallState::default(),
            gun_round: None,
            tracer: false,
        });
    }
    println!(
        "surface-scene: {count} bombs on runway {:#010x}",
        runway.object
    );
    Ok(())
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
    spec.weather.redrawn_airports = crate::scenery::redrawn_airports()?;
    if o.time.is_some() {
        spec.weather.time = o.time;
    }
    (spec.aaa, spec.sam) = o.defenses;
    if o.intruder.is_some() {
        spec.cheats.damage = tore_sim::cheats::Damage::Invulnerable;
    }
    let mut world =
        crate::surface_dump::world_without_front(&spec, &resources, Seating::SinglePlayer)?;
    if let Some(why) = &world.terrain.surface.unresolved {
        return Err(format!("the ground target stands nowhere: {why}").into());
    }
    for ordinal in &o.blue {
        let id = UnitId::template(*ordinal).ok_or("--blue: not a template ordinal")?;
        crate::surface_battle::hand_to_blue(&mut world, id)?;
    }
    let mut scenery = Scenery::build(&resources, &world.terrain)?;
    // The point the intruder circles, from where the focus stands at the
    // start.
    let anchor = o.intruder.map(|[east, up, north]| {
        let start = focused(&o, places(&world));
        let center = centre(&start);
        [center[0] + east, center[1] + up, center[2] + north]
    });
    let aim = !o.no_aim;
    let mut output = TickOutput::default();
    let mut step = |world: &mut World,
                    scenery: &mut Scenery,
                    ticks: u64,
                    surface_only: bool|
     -> AppResult<()> {
        for _ in 0..ticks {
            if let Some(anchor) = anchor {
                // 300 knots round a 3,000 ft circle, level.
                let (speed, radius) = (300. * 1.687_81, 3_000.);
                let turn = world.tick() as f64 / 120. * speed / radius;
                let flight = &mut world.cockpits[0].flight;
                flight.position = [
                    anchor[0] + radius * turn.cos(),
                    anchor[1],
                    anchor[2] + radius * turn.sin(),
                ];
                flight.velocity = [-speed * turn.sin(), 0., speed * turn.cos()];
                flight.speed = speed;
                flight.yaw = flight.velocity[0].atan2(flight.velocity[2]);
                flight.pitch = 0.;
                flight.bank = 0.;
                flight.vertical_speed = 0.;
            }
            if surface_only {
                world.combat.step_surface(&world.terrain);
            } else {
                let input = SeatInput {
                    tick: world.tick(),
                    ..SeatInput::default()
                };
                world.step(&[input], &mut output)?;
            }
            if aim {
                let surface = &world.terrain.surface;
                let tracks = tore_world::surface::aim::tracks(surface, &world.combat.surface);
                scenery.surface.step_aim(
                    world.combat.render_snapshot(),
                    surface,
                    &tracks,
                    Some(&world.combat.surface),
                );
            }
        }
        Ok(())
    };
    step(
        &mut world,
        &mut scenery,
        (o.seconds * 120.).round() as u64,
        o.surface_only,
    )?;
    // One whole-world tick takes the picture the views draw.
    step(&mut world, &mut scenery, 1, false)?;
    let focus = focused(&o, places(&world));
    if focus.is_empty() {
        let mut types = std::collections::BTreeMap::<String, usize>::new();
        for (_, resource, ..) in places(&world) {
            *types.entry(resource).or_default() += 1;
        }
        for (resource, count) in types {
            println!("surface-scene: place {resource} x{count}");
        }
        return Err("nothing to frame".into());
    }
    let center = centre(&focus);
    let radius = o.distance.unwrap_or_else(|| {
        focus
            .iter()
            .map(|(_, _, p, _)| (p[0] - center[0]).hypot(p[2] - center[2]))
            .fold(0., f64::max)
            + 40.
    });
    if !o.kill.is_empty() {
        let mut doomed: Vec<u32> = places(&world)
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
        if o.kill.iter().any(|item| item == "runways") {
            doomed.extend(
                world
                    .terrain
                    .airport_scene
                    .runways
                    .iter()
                    .filter(|runway| {
                        (runway.approach_center[0] - center[0])
                            .hypot(runway.approach_center[2] - center[2])
                            <= radius
                    })
                    .map(|runway| runway.object),
            );
        }
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
        step(
            &mut world,
            &mut scenery,
            (o.burn * 120.).round() as u64,
            false,
        )?;
    }
    if let Some(feet) = o.flatten {
        match &world.terrain.city_fill {
            Some(fill) => println!(
                "surface-scene: flattened {} of {} city fill buildings within {feet:.0} ft",
                fill.flatten(center, feet),
                fill.len()
            ),
            None => println!("surface-scene: this theater has no city fill"),
        }
    }
    if o.bomb_runway > 0 {
        bomb_runway(&mut world, &resources, center, o.bomb_runway)?;
        step(
            &mut world,
            &mut scenery,
            (o.burn.max(12.) * 120.).round() as u64,
            false,
        )?;
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
    let art = crate::render_snapshot::CombatArt::load(&resources)?;
    let mut gpu = pollster::block_on(Gpu::new(&scenery))?;
    let tracks = tore_world::surface::aim::tracks(&world.terrain.surface, &world.combat.surface);
    for (id, resource, at, routed) in &focus {
        println!(
            "surface-scene: {id:#010x} {resource} at {:.0} {:.0} {:.0}{}",
            at[0],
            at[1],
            at[2],
            if *routed { " routed" } else { "" }
        );
        if let Some(aim) = scenery.surface.aim_of(*id) {
            let held: Vec<u32> = tracks
                .iter()
                .filter(|t| t.unit.0 == *id)
                .map(|t| t.target)
                .collect();
            let turrets: Vec<String> = aim
                .turrets
                .iter()
                .map(|(hp, a)| format!("{hp}:{:.1}", a.to_degrees()))
                .collect();
            let hp = world
                .combat
                .state
                .targets
                .iter()
                .find(|t| t.id == *id)
                .map_or(0, |t| t.hp);
            println!(
                "surface-scene: aim {id:#010x} hp {hp} body {:.1} turrets [{}] holds {held:?}",
                aim.body.to_degrees(),
                turrets.join(" ")
            );
        }
    }
    let snapshot = world.combat.render_snapshot().clone();
    let standing = crate::render_snapshot::standing(&world.combat.state.targets);
    // The surface defenses' own presentation: wreck fires fitted to their
    // units and the smoke of wrecks without a fire (`surface_fx`).
    let now = snapshot.tick as f64;
    let mut tracker = crate::gun_flash::Tracker::default();
    tracker.set_fire_widths(std::sync::Arc::clone(&scenery.fire_widths));
    tracker.observe(&snapshot, now, |_| None);
    let guns = tracker.draw(now, |_| None);
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
        gpu.sim
            .emitters(&gpu.queue, &world.combat.state.devices, &[], &guns);
        let pixels = gpu.pixels(&camera, &world.terrain, &scenery)?;
        if o.frames > 0 {
            // Frame cost with the scene as drawn: the average of more frames
            // of the same view, readback included.
            let start = std::time::Instant::now();
            for _ in 0..o.frames {
                gpu.pixels(&camera, &world.terrain, &scenery)?;
            }
            println!(
                "surface-scene: {view} frame {:.2} ms over {} frames",
                start.elapsed().as_secs_f64() * 1e3 / o.frames as f64,
                o.frames
            );
        }
        let path = out.join(format!("{tag}-{view}.png"));
        std::fs::write(
            &path,
            crate::replay::png::encode_rgba(crate::reel::WIDTH, crate::reel::HEIGHT, &pixels)?,
        )?;
        println!("surface-scene: {}", path.display());
    }
    Ok(())
}
