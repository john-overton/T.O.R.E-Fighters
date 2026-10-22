//! `--render-scene DIR` for the AI strike driver (slice ST-T2): one frame of
//! the world as the game draws it, offscreen at 1080p, from a chase camera
//! behind the striker looking down its pass at the target, a moment after
//! its first release. The way `--surface-scene` draws a ground target, with
//! the aircraft, rounds and tracers added.
use super::Scene;
use crate::{
    AppResult, aircraft::Airframe, camera::Camera, reel::Gpu, render_snapshot, scenery::Scenery,
};
use std::path::Path;
use tore_sim::attitude::Vector;
use tore_world::seats::PlaneId;

/// The chase camera: this far behind the striker along its flight path,
/// beside it to the right and above it, feet.
const BEHIND_FT: f64 = 220.;
const BESIDE_FT: f64 = 120.;
const ABOVE_FT: f64 = 30.;

pub(super) fn render(
    scene: &mut Scene,
    observer: PlaneId,
    plane: PlaneId,
    target: Vector,
    path: &Path,
) -> AppResult<()> {
    let flight = scene
        .world
        .ai_wings
        .as_ref()
        .and_then(|w| w.mission().actor(plane.0))
        .ok_or("the striker is not AI-flown")?
        .flight()
        .clone();
    // Off the striker's right wing and a little behind, looking past it at
    // the target, so the dive's angle and the target are both in frame.
    let p = flight.position;
    let v = flight.velocity;
    let speed = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt().max(1.);
    let forward = v.map(|c| c / speed);
    let horizontal = forward[0].hypot(forward[2]).max(1e-6);
    let right = [forward[2] / horizontal, 0., -forward[0] / horizontal];
    let eye: Vector = std::array::from_fn(|i| {
        p[i] - forward[i] * BEHIND_FT + right[i] * BESIDE_FT + if i == 1 { ABOVE_FT } else { 0. }
    });
    render_world(&mut scene.world, observer, eye, target, path)?;
    println!("ai-strike: RENDER path={}", path.display());
    Ok(())
}

/// One frame of `world` as the game draws it, offscreen at 1080p, from a
/// camera at `eye` looking at `target`, the picture presented from the seat
/// plane `observer`. The AI probe's `--probe-render` uses it too (slice H2).
pub(crate) fn render_world(
    world: &mut tore_world::world::World,
    observer: PlaneId,
    eye: Vector,
    target: Vector,
    path: &Path,
) -> AppResult<()> {
    let mut loaded = Loaded::new(world, observer)?;
    let look = [target[0] - eye[0], target[1] - eye[1], target[2] - eye[2]];
    let mut camera = Camera::new();
    camera.position = eye;
    camera.yaw = look[0].atan2(look[2]) as f32;
    camera.pitch = look[1].atan2(look[0].hypot(look[2])) as f32;
    camera.near_clip = 1.;
    if std::env::var_os("TORE_SCENE_F12").is_some() {
        let frame = world
            .flight_frame(super::SEAT, None, &loaded.snapshot, &[])
            .ok_or("the seat flies no plane")?;
        let subjects = crate::flight_views::Scene::new(
            &frame,
            &loaded.seen_from,
            &world.combat,
            world.ai_wings.as_ref(),
            None,
        );
        let mut rig = crate::flight_views::Rig::default();
        camera = rig
            .camera(
                crate::flight_views::MISSILE,
                &subjects,
                Camera::new(),
                [0.; 2],
                1.,
            )
            .unwrap_or_else(|_| {
                rig.select(crate::flight_views::Reference::Aircraft(observer.0));
                rig.camera(0, &subjects, Camera::new(), [0.; 2], 1.)
                    .expect("seat exists")
            });
        println!(
            "probe F12: tick {} eye {:?} hidden {:?} weapons {:?}",
            loaded.snapshot.tick,
            camera.position,
            camera.hidden_projectile,
            loaded
                .snapshot
                .projectiles
                .iter()
                .filter(|p| !p.gun)
                .map(|p| (p.id, p.owner, &p.shape))
                .collect::<Vec<_>>()
        );
    }
    loaded.upload(world, &camera)?;
    if std::env::var_os("TORE_SCENE_F12").is_some() {
        // Weapon geometry uses the aircraft pass and its palette, even when
        // the seat aircraft is outside this camera's picture.
        let gpu = &mut loaded.gpu;
        gpu.sim
            .aircraft(&gpu.device, &gpu.queue, &loaded.observer_airframe, &[]);
    }
    let pixels = loaded
        .gpu
        .pixels(&camera, &world.terrain, &loaded.scenery)?;
    write_png(path, &pixels)
}

/// Writes a 1080p RGBA frame as a PNG, making its folder.
pub(crate) fn write_png(path: &Path, pixels: &[u8]) -> AppResult<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        path,
        crate::replay::png::encode_rgba(crate::reel::WIDTH, crate::reel::HEIGHT, pixels)?,
    )?;
    Ok(())
}

/// The world loaded for offscreen frames of one moment: the GPU, the
/// scenery, the art and the picture as the seat plane `observer` presents
/// it. [`Loaded::upload`] then places it for a camera.
pub(crate) struct Loaded {
    pub gpu: Gpu,
    pub scenery: Scenery,
    pub art: render_snapshot::CombatArt,
    pub models: Vec<Airframe>,
    pub observer_airframe: Airframe,
    pub seen_from: tore_sim::flight::State,
    pub snapshot: tore_world::snapshot::RenderSnapshot,
}

impl Loaded {
    pub(crate) fn new(world: &mut tore_world::world::World, observer: PlaneId) -> AppResult<Self> {
        // The headless run keeps no picture: take one now, from the seat's
        // plane (the picture needs a seat's plane to present).
        let seen_from = world
            .cockpits
            .iter()
            .find(|c| c.plane == observer)
            .ok_or("no seat plane to present the picture from")?
            .flight
            .clone();
        world
            .combat
            .refresh_render(observer.0, &seen_from, world.ai_wings.as_ref());
        let world = &*world;
        // Retail art and the weather module, as the reel and the surface scene load them.
        let mut resources = crate::reel::load_assets()?.theater_resources;
        crate::surface_dump::with_retail_surface(&mut resources)?;
        let snapshot = world.combat.render_snapshot().clone();
        let models = snapshot
            .models
            .iter()
            .map(|id| Airframe::load(&resources, *id))
            .collect::<Result<Vec<_>, _>>()?;
        let observer_airframe = Airframe::load(
            &resources,
            world
                .combat
                .state
                .ownship(observer.0)
                .map(|o| o.configuration().aircraft)
                .ok_or("the seat plane has no ownship")?,
        )?;
        let scenery = Scenery::build(&resources, &world.terrain)?;
        let mut art = render_snapshot::CombatArt::load(&resources)?;
        if std::env::var_os("TORE_SCENE_F12").is_some() {
            art.add_shapes(
                snapshot
                    .projectiles
                    .iter()
                    .filter_map(|p| p.shape.as_deref()),
                &resources,
            );
        }
        let gpu = pollster::block_on(Gpu::new(&scenery))?;
        Ok(Self {
            gpu,
            scenery,
            art,
            models,
            observer_airframe,
            seen_from,
            snapshot,
        })
    }

    /// Places the picture for `camera`.
    pub(crate) fn upload(
        &mut self,
        world: &tore_world::world::World,
        camera: &Camera,
    ) -> AppResult<()> {
        let Self {
            gpu,
            scenery,
            art,
            models,
            observer_airframe,
            seen_from,
            snapshot,
        } = self;
        scenery.resolve_palette(&world.terrain, camera.position[1]);
        scenery.set_origin(camera.position);
        let geometry = std::sync::Arc::clone(
            scenery.static_geometry(&world.combat.state.targets, &world.combat.surface),
        );
        gpu.sim.airports(&gpu.device, &gpu.queue, &geometry);
        let standing = render_snapshot::standing(&world.combat.state.targets);
        gpu.sim.surface_units(
            &gpu.device,
            &gpu.queue,
            &scenery.surface_vertices(snapshot, &|id| standing.contains(&id), camera),
        );
        gpu.sim.dummies(
            &gpu.device,
            &gpu.queue,
            render_snapshot::aircraft_batches(
                snapshot,
                models,
                art,
                camera,
                &world.terrain,
                scenery,
            ),
        );
        gpu.sim.combat(
            &gpu.device,
            &gpu.queue,
            &render_snapshot::combat_geometry(
                snapshot,
                art,
                observer_airframe,
                seen_from,
                camera,
                &world.terrain,
                scenery,
            ),
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
        let now = snapshot.tick as f64;
        let mut tracker = crate::gun_flash::Tracker::default();
        tracker.observe(snapshot, now, |_| None);
        let guns = tracker.draw(now, |_| None);
        gpu.sim
            .emitters(&gpu.queue, &world.combat.state.devices, &[], &guns);
        Ok(())
    }
}
