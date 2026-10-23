//! `--render-scene DIR --scene-at S[,S...]` for the player's attack kinds
//! (slice BF): frames of the world as the game draws it, offscreen at
//! 1080p, S seconds after the first rocket, missile or bomb leaves the
//! aircraft, from just behind and beside the newest of them, chasing it.
//! For reviewing motor smoke, the motor flame and impacts (`--condition
//! night` for the dark). Each frame is `DIR/scene-tS.png`.
use super::Scene;
use crate::{
    AppResult, aircraft::Airframe, camera::Camera, reel::Gpu, render_snapshot, scenery::Scenery,
};
use std::path::{Path, PathBuf};
use tore_sim::attitude::Vector;

/// The camera: this far to the right of the newest store, across the
/// flight path, above it and back along it, feet.
const BESIDE_FT: f64 = 140.;
const ABOVE_FT: f64 = 25.;
const BEHIND_FT: f64 = 160.;

/// The frames still to draw and where the first store left.
#[derive(Default)]
pub(crate) struct Plan {
    dir: Option<PathBuf>,
    pending: Vec<f64>,
    /// The tick, position and travel direction of the first launch.
    launch: Option<(u64, Vector, Vector)>,
    rig: crate::flight_views::Rig,
}

impl Plan {
    pub fn new(dir: Option<PathBuf>, at: &[f64]) -> Self {
        let mut pending = at.to_vec();
        pending.sort_by(f64::total_cmp);
        Self {
            dir,
            pending,
            launch: None,
            rig: crate::flight_views::Rig::default(),
        }
    }

    pub(super) fn active(&self) -> bool {
        self.dir.is_some() && !self.pending.is_empty()
    }
}

/// What the run's motors did (bugs B6 and B7): the most missile smoke
/// puffs in the air at once, the rounds whose motor burned, and the rounds
/// whose motor was seen to burn out while still flying.
#[derive(Default)]
pub(crate) struct Motors {
    max_puffs: usize,
    lit: std::collections::BTreeSet<u32>,
    burnt_out: std::collections::BTreeSet<u32>,
}

impl Motors {
    fn observe(&mut self, state: &tore_sim::combat::live::State) {
        let puffs = state
            .smoke
            .puffs
            .iter()
            .filter(|p| p.kind == tore_sim::combat::smoke::Kind::Missile)
            .count();
        self.max_puffs = self.max_puffs.max(puffs);
        for p in &state.projectiles {
            if state.motor_powered(p) {
                self.lit.insert(p.id);
            } else if self.lit.contains(&p.id) {
                self.burnt_out.insert(p.id);
            }
        }
    }

    /// The `a2g-run: motors` line.
    pub fn line(&self) -> String {
        format!(
            "a2g-run: motors lit {} burnt-out-in-flight {} missile-smoke max-puffs {}",
            self.lit.len(),
            self.burnt_out.len(),
            self.max_puffs
        )
    }
}

/// Draws the frames due after the tick just stepped.
pub(crate) fn after_step(scene: &mut Scene) -> AppResult<()> {
    scene.motors.observe(&scene.world.combat.state);
    if !scene.scenes.active() {
        return Ok(());
    }
    if std::env::var_os("TORE_SCENE_F12").is_some() {
        super::ai_render::f12_camera(&scene.world, &mut scene.scenes.rig);
    }
    let owner = scene.owner;
    let newest = scene
        .world
        .combat
        .state
        .projectiles
        .iter()
        .filter(|p| {
            p.owner == owner && !tore_sim::combat::live::is_gun(scene.world.combat.state.weapon(p))
        })
        .max_by_key(|p| p.id)
        .map(|p| (p.position, p.direction));
    let tick = scene.world.combat.state.tick();
    if scene.scenes.launch.is_none()
        && let Some((position, direction)) = newest
    {
        scene.scenes.launch = Some((tick, position, direction));
    }
    let Some((launched, origin, along)) = scene.scenes.launch else {
        return Ok(());
    };
    let seconds = (tick - launched) as f64 / 120.;
    while scene
        .scenes
        .pending
        .first()
        .is_some_and(|at| *at <= seconds)
    {
        let at = scene.scenes.pending.remove(0);
        // Chase the newest store: from behind and beside where it is now
        // (or where the first left, once none is flying).
        let (look, along) = newest.unwrap_or((origin, along));
        let dir = scene
            .scenes
            .dir
            .clone()
            .expect("an active plan has a folder");
        let path = dir.join(format!("scene-t{at:.2}s.png"));
        render(scene, look, along, look, &path)?;
    }
    Ok(())
}

fn render(
    scene: &mut Scene,
    origin: Vector,
    along: Vector,
    look: Vector,
    path: &Path,
) -> AppResult<()> {
    let seen_from = scene.world.cockpits[0].flight.clone();
    let plane = scene.world.cockpits[0].plane;
    let world = &mut scene.world;
    world
        .combat
        .refresh_render(plane.0, &seen_from, world.ai_wings.as_ref());
    let world = &scene.world;
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
        world.combat.state.own().configuration().aircraft,
    )?;
    let mut scenery = Scenery::build(&resources, &world.terrain)?;
    let mut art = render_snapshot::CombatArt::load(&resources)?;
    art.add_shapes(
        snapshot
            .projectiles
            .iter()
            .filter_map(|p| p.shape.as_deref()),
        &resources,
    );
    let mut gpu = pollster::block_on(Gpu::new(&scenery))?;

    let horizontal = along[0].hypot(along[2]).max(1e-6);
    let forward = [along[0] / horizontal, 0., along[2] / horizontal];
    let right = [forward[2], 0., -forward[0]];
    let mut eye: Vector = std::array::from_fn(|i| {
        origin[i] + right[i] * BESIDE_FT - forward[i] * BEHIND_FT
            + if i == 1 { ABOVE_FT } else { 0. }
    });
    let floor = f64::from(world.terrain.height(eye[0] as f32, eye[2] as f32));
    eye[1] = eye[1].max(floor + 40.);
    let d = [look[0] - eye[0], look[1] - eye[1], look[2] - eye[2]];
    let mut camera = Camera::new();
    camera.position = eye;
    camera.yaw = d[0].atan2(d[2]) as f32;
    camera.pitch = d[1].atan2(d[0].hypot(d[2])) as f32;
    camera.near_clip = 1.;
    // An opt-in live F12 capture for comparison with the replay camera.
    if std::env::var_os("TORE_SCENE_F12").is_some() {
        camera = super::ai_render::f12_camera(world, &mut scene.scenes.rig);
        println!(
            "a2g-run: F12 tick {} eye {:?} hidden {:?} projectiles {:?}",
            snapshot.tick,
            camera.position,
            camera.hidden_projectile,
            snapshot
                .projectiles
                .iter()
                .filter(|p| !p.gun)
                .map(|p| (p.id, &p.shape, p.position))
                .collect::<Vec<_>>()
        );
    }

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
        &scenery.surface_vertices(&snapshot, &|id| standing.contains(&id), &camera),
    );
    gpu.sim.dummies(
        &gpu.device,
        &gpu.queue,
        render_snapshot::aircraft_batches(
            &snapshot,
            &models,
            &art,
            &camera,
            &world.terrain,
            &scenery,
        ),
    );
    gpu.sim.combat(
        &gpu.device,
        &gpu.queue,
        &render_snapshot::combat_geometry(
            &snapshot,
            &art,
            &observer_airframe,
            &seen_from,
            &camera,
            &world.terrain,
            &scenery,
        ),
    );
    // The combat geometry (stores, strips) draws with the aircraft pass.
    gpu.sim
        .aircraft(&gpu.device, &gpu.queue, &observer_airframe, &[]);
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
    tracker.observe(&snapshot, now, |_| None);
    let guns = tracker.draw(now, |_| None);
    gpu.sim
        .emitters(&gpu.queue, &world.combat.state.devices, &[], &guns);
    if std::env::var_os("TORE_SCENE_DEBUG").is_some() {
        let size = [crate::reel::WIDTH, crate::reel::HEIGHT];
        println!(
            "a2g-run: debug look-at on screen {:?}",
            camera.project(size, look)
        );
        for puff in world.combat.state.smoke.puffs.iter() {
            println!(
                "a2g-run: debug puff age {} at {:.0?} on screen {:?}",
                puff.age,
                puff.position,
                camera.project(size, puff.position)
            );
        }
    }
    let pixels = gpu.pixels(&camera, &world.terrain, &scenery)?;
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(
        path,
        crate::replay::png::encode_rgba(crate::reel::WIDTH, crate::reel::HEIGHT, &pixels)?,
    )?;
    println!(
        "a2g-run: SCENE path={} eye {:.0?} look-at {:.0?} look {:.0} ft projectiles {:?} puffs {}",
        path.display(),
        eye,
        look,
        (d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt(),
        snapshot
            .projectiles
            .iter()
            .map(|p| (
                p.weapon.clone(),
                p.shape.clone(),
                p.position.map(|v| v.round())
            ))
            .collect::<Vec<_>>(),
        world.combat.state.smoke.puffs.len()
    );
    Ok(())
}
