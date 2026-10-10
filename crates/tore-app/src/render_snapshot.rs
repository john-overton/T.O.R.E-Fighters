//! Draws one tick's picture: the vertices for the other aircraft, debris,
//! weapons, tracers, ejected pilots and afterburner lights of a
//! [`RenderSnapshot`].
//!
//! The snapshot itself, and the blend of two of them at a frame's tick
//! fraction, are plain data in `snapshot.rs`. Live flight draws the blend of
//! its last two snapshots with [`aircraft_batches`] and [`combat_geometry`]; a
//! mission replay draws decoded snapshots through the same helpers, so both
//! show the same picture.
use crate::{
    AppResult,
    aircraft::Airframe,
    camera::Camera,
    flight,
    scenery::Scenery,
    sim_renderer::{CombatGeometry, Contact},
    snapshot::{AircraftPose, DebrisPose, Draw, RenderSnapshot, set_devices, wreck_in},
    terrain::Terrain,
};
use std::collections::BTreeMap;
use tore_formats::{Pic, aircraft::AircraftId, shape::Shape};
use tore_sim::{
    attitude::{Basis, Vector, cross, unit},
    combat::live,
};

/// One aircraft's lit afterburner as lights: one in each engine's flame,
/// behind its outlet in `offsets` (right, up and forward from the aircraft's
/// reference point, in feet), sharing the aircraft's strength
/// (docs/spec/engine-material.md#afterburner-glow).
pub fn afterburner_glow(
    position: Vector,
    [yaw, pitch, bank]: [f64; 3],
    offsets: &[Vector],
) -> Vec<crate::countermeasure_renderer::Afterburner> {
    use crate::countermeasure_renderer::{AFTERBURNER_BEHIND_FEET, AFTERBURNER_SHARE, Afterburner};
    let basis = Basis::new(yaw, pitch, bank);
    let share = AFTERBURNER_SHARE / offsets.len().max(1) as f64;
    offsets
        .iter()
        .map(|o| Afterburner {
            position: std::array::from_fn(|i| {
                position[i]
                    + basis.right[i] * o[0]
                    + basis.up[i] * o[1]
                    + basis.forward[i] * (o[2] - AFTERBURNER_BEHIND_FEET)
            }),
            share,
        })
        .collect()
}

/// Where an aircraft's engine outlets sit, for its flame lights: its own
/// model's, from `model_outlets` (parallel to `models`), when a model is
/// loaded for its type and that is not the player's type; otherwise the
/// player's airframe's.
pub fn engine_outlets<'a>(
    aircraft: Option<AircraftId>,
    player: AircraftId,
    models: &[Airframe],
    model_outlets: &'a [Vec<Vector>],
    player_outlets: &'a [Vector],
) -> &'a [Vector] {
    aircraft
        .filter(|id| *id != player)
        .and_then(|id| models.iter().position(|model| model.profile.id == id))
        .and_then(|index| model_outlets.get(index))
        .map_or(player_outlets, Vec::as_slice)
}

/// The lit afterburners of the aircraft in `snapshot` other than the player,
/// in target order, at their poses. `offsets` gives an aircraft's engine
/// outlets.
pub fn target_glows<'a>(
    snapshot: &RenderSnapshot,
    offsets: impl Fn(&AircraftPose) -> &'a [Vector],
) -> Vec<crate::countermeasure_renderer::Afterburner> {
    snapshot
        .targets
        .iter()
        .filter(|pose| pose.engine.flame)
        .flat_map(|pose| afterburner_glow(pose.position, pose.attitude, offsets(pose)))
        .collect()
}

/// A target drawn with its own model, over the model's start state.
fn model_pose(mut s: flight::State, pose: &AircraftPose, tick: u64) -> flight::State {
    s.ticks = tick;
    s.engine = pose.engine.lit;
    s.burner = pose.engine.afterburner;
    s.position = pose.position;
    s.damage_fraction = pose.damage.fraction();
    s.damage_variant = pose.damage.variant();
    s.damage_regions = pose.damage.regions();
    [s.yaw, s.pitch, s.bank] = pose.attitude;
    s.gear = 0.;
    s.flaps = 0.;
    s.exhaust = 0.;
    s.bay = 0.;
    if let Some(devices) = pose.devices {
        set_devices(&mut s, devices);
    }
    // A rotorcraft's blades turn at its rotor speed and its disks tilt as it
    // flies them (slice P7b).
    if pose.engine.rotor > 0. {
        crate::snapshot::set_rotor(&mut s, &pose.engine);
    }
    s
}

/// A straight-flight fixture drawn with the player's airframe over the
/// player's presented state, which supplies bay, speed and throttle. The
/// pose's resolved engine decides the nozzle heat, so a replay's rebuilt
/// player state draws fixtures exactly as the live one does.
fn ownship_pose(template: &flight::State, pose: &AircraftPose) -> flight::State {
    let mut s = template.clone();
    s.wreck = pose.wreck.map(wreck_in);
    s.crashed = pose.damage.hp <= 0;
    s.engine = pose.engine.lit;
    s.burner = pose.engine.afterburner;
    s.position = pose.position;
    s.damage_fraction = pose.damage.fraction();
    s.damage_variant = pose.damage.variant();
    s.damage_regions = pose.damage.regions();
    [s.yaw, s.pitch, s.bank] = pose.attitude;
    s.exhaust = 0.;
    s.gear = 0.;
    s.flaps = 0.;
    s.elevator = 0.;
    s.aileron = 0.;
    s.rudder = 0.;
    s.brake = 0.;
    s.hook = 0.;
    if let Some(devices) = pose.devices {
        set_devices(&mut s, devices);
    }
    s
}

/// Per-model vertices for the aircraft drawn with their own models: one batch
/// per `snapshot.models` entry found in `models`, with each airborne
/// aircraft's vertex range for the spotting aid, then that model's debris.
pub fn aircraft_batches<'a>(
    snapshot: &RenderSnapshot,
    models: &'a [Airframe],
    camera: &Camera,
    world: &Terrain,
    scenery: &Scenery,
) -> Vec<(&'a Airframe, Vec<f32>, Vec<Contact>)> {
    aircraft_batches_with(
        tore_workers::shared(),
        snapshot,
        models,
        camera,
        world,
        scenery,
    )
}

// Fitted dispatch threshold: preserve the inline path for small formations.
// The synthetic wall-time probe and integrated frame runs measure its cost.
const MIN_AIRCRAFT_JOBS: usize = 4;

enum AircraftJob<'a> {
    Target(&'a AircraftPose),
    Debris(&'a DebrisPose),
}

fn aircraft_batches_with<'a>(
    workers: &tore_workers::Executor,
    snapshot: &RenderSnapshot,
    models: &'a [Airframe],
    camera: &Camera,
    world: &Terrain,
    scenery: &Scenery,
) -> Vec<(&'a Airframe, Vec<f32>, Vec<Contact>)> {
    let selected = || {
        snapshot
            .models
            .iter()
            .filter_map(|id| models.iter().find(|model| model.profile.id == *id))
    };
    let visible = |target: &&AircraftPose, draw| {
        target.draw == draw && target.airborne && Some(target.id) != camera.hidden_target
    };
    let count = selected()
        .map(|model| {
            let draw = Draw::Model(model.profile.id);
            snapshot.targets.iter().filter(|t| visible(t, draw)).count()
                + snapshot.debris.iter().filter(|p| p.draw == draw).count()
        })
        .sum();
    if !workers.should_dispatch(count, MIN_AIRCRAFT_JOBS) {
        return aircraft_batches_serial(snapshot, models, camera, world, scenery);
    }

    let selected: Vec<_> = selected().collect();
    let mut jobs = Vec::with_capacity(count);
    for (batch, model) in selected.iter().enumerate() {
        let draw = Draw::Model(model.profile.id);
        jobs.extend(
            snapshot
                .targets
                .iter()
                .filter(|t| visible(t, draw))
                .map(|target| (batch, AircraftJob::Target(target))),
        );
        jobs.extend(
            snapshot
                .debris
                .iter()
                .filter(|piece| piece.draw == draw)
                .map(|piece| (batch, AircraftJob::Debris(piece))),
        );
    }
    // Each job is one aircraft or detached piece, including formations in
    // which every aircraft uses the same model. No GPU resource crosses here.
    let geometry = workers.ordered_map(&jobs, MIN_AIRCRAFT_JOBS, |_, (batch, job)| {
        let model = selected[*batch];
        match job {
            AircraftJob::Target(target) => {
                let pose = model_pose(model.start(world), target, snapshot.tick);
                model.vertices(&pose, camera, world, scenery)
            }
            AircraftJob::Debris(piece) => {
                let mut pose = model.start(world);
                pose.position = piece.position;
                pose.damage_variant = piece.variant;
                [pose.yaw, pose.pitch, pose.bank] = piece.attitude;
                model.fragment_vertices(&pose, camera, world, scenery)
            }
        }
    });
    let mut sizes = vec![0; selected.len()];
    for ((batch, _), vertices) in jobs.iter().zip(&geometry) {
        sizes[*batch] += vertices.len();
    }
    let extents: Vec<_> = selected.iter().map(|model| model.visual_extent()).collect();
    let mut batches: Vec<_> = selected
        .into_iter()
        .zip(sizes)
        .map(|(model, size)| (model, Vec::with_capacity(size), Vec::new()))
        .collect();
    // Completion order never decides draw order. Contacts use the offset in
    // the final model batch, and debris never gains a spotting-aid contact.
    for ((batch, job), vertices) in jobs.into_iter().zip(geometry) {
        let (_, merged, contacts) = &mut batches[batch];
        let first = merged.len() / 10;
        merged.extend(vertices);
        if let AircraftJob::Target(target) = job {
            contacts.extend(Contact::new(
                first,
                merged.len() / 10,
                scenery.relative(target.position),
                extents[batch],
            ));
        }
    }
    batches
}

/// Pre-threading ordered builder, retained for the inline path and as an independent
/// reference for worker output. It constructs no parallel staging buffers.
fn aircraft_batches_serial<'a>(
    snapshot: &RenderSnapshot,
    models: &'a [Airframe],
    camera: &Camera,
    world: &Terrain,
    scenery: &Scenery,
) -> Vec<(&'a Airframe, Vec<f32>, Vec<Contact>)> {
    snapshot
        .models
        .iter()
        .filter_map(|id| models.iter().find(|model| model.profile.id == *id))
        .map(|model| {
            let draw = Draw::Model(model.profile.id);
            let mut vertices = Vec::new();
            let mut contacts = Vec::new();
            let extent = model.visual_extent();
            for target in snapshot
                .targets
                .iter()
                .filter(|t| t.draw == draw && t.airborne && Some(t.id) != camera.hidden_target)
            {
                let pose = model_pose(model.start(world), target, snapshot.tick);
                let first = vertices.len() / 10;
                vertices.extend(model.vertices(&pose, camera, world, scenery));
                contacts.extend(Contact::new(
                    first,
                    vertices.len() / 10,
                    scenery.relative(pose.position),
                    extent,
                ));
            }
            for piece in snapshot.debris.iter().filter(|p| p.draw == draw) {
                let mut pose = model.start(world);
                pose.position = piece.position;
                pose.damage_variant = piece.variant;
                [pose.yaw, pose.pitch, pose.bank] = piece.attitude;
                vertices.extend(model.fragment_vertices(&pose, camera, world, scenery));
            }
            (model, vertices, contacts)
        })
        .collect()
}

/// Combat geometry drawn with the player's airframe: fixture targets over the
/// player's presented state, debris, weapons, tracers and effects.
pub fn combat_geometry(
    snapshot: &RenderSnapshot,
    art: &CombatArt,
    ownship: &Airframe,
    ownship_state: &flight::State,
    camera: &Camera,
    world: &Terrain,
    scenery: &Scenery,
) -> CombatGeometry {
    let mut v = Vec::new();
    let mut contacts = Vec::new();
    let extent = ownship.visual_extent();
    // Everything here is built relative to the render origin, in f64 until
    // the vertex is written.
    let local = |p: Vector| scenery.relative(p);
    let mut eye = Camera::new();
    eye.position = local(camera.position);
    [eye.yaw, eye.pitch, eye.roll] = [camera.yaw, camera.pitch, camera.roll];
    for target in snapshot
        .targets
        .iter()
        .filter(|t| t.draw == Draw::Ownship && t.airborne && Some(t.id) != camera.hidden_target)
    {
        let pose = ownship_pose(ownship_state, target);
        let first = v.len() / 10;
        v.extend(ownship.vertices(&pose, camera, world, scenery));
        contacts.extend(Contact::new(
            first,
            v.len() / 10,
            local(pose.position),
            extent,
        ));
    }
    for piece in snapshot.debris.iter().filter(|p| p.draw == Draw::Ownship) {
        let mut pose = ownship_state.clone();
        // Detached pieces have their own lifecycle, not the observer's wreck state.
        pose.wreck = None;
        pose.position = piece.position;
        pose.damage_variant = piece.variant;
        [pose.yaw, pose.pitch, pose.bank] = piece.attitude;
        v.extend(ownship.fragment_vertices(&pose, camera, world, scenery));
    }
    // Attached external stores are hidden until the dedicated ordnance
    // rendering pass. Loadout/flight state and launched projectiles remain
    // independent of this presentation decision in every camera.
    for p in &snapshot.projectiles {
        if Some(p.id) == camera.hidden_projectile {
            continue;
        }
        if !p.gun
            && let Some(shape) = p.shape.as_ref().and_then(|name| art.shapes.get(name))
        {
            let right = unit([p.direction[2], 0., -p.direction[0]]);
            mesh(
                &mut v,
                shape,
                local(p.position),
                right,
                cross(p.direction, right),
                p.direction,
                &ownship.palette,
            );
        }
        if p.gun && p.tracer {
            tracer(
                &mut v,
                local(p.previous),
                local(p.position),
                &eye,
                tracer_brightness(&p.weapon),
            );
        } else if !p.gun {
            // A visible thin strip marks the actual swept projectile segment.
            let right = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.).right;
            let previous = local(p.previous);
            let a: Vector = std::array::from_fn(|i| previous[i] + right[i] * 0.4);
            let b: Vector = std::array::from_fn(|i| previous[i] - right[i] * 0.4);
            for pos in [a, b, local(p.position)] {
                vertex(&mut v, pos, [1., 0.8, 0.3]);
            }
        }
    }
    // Explosions, fires and craters are drawn by effect_renderer.
    CombatGeometry {
        vertices: v,
        contacts,
    }
}

/// Combat effect and weapon art: the explosion, fire and crater sheets, the
/// smoke sheet, weapon shapes by name and ejected-pilot poses.
pub struct CombatArt {
    pub smoke: Pic,
    shapes: BTreeMap<String, Shape>,
    pub effects: crate::effect_renderer::Art,
    pub escape: Option<crate::ejection_art::Art>,
}
impl CombatArt {
    /// Loads the sampled original effect art. `palette` supplies the colours
    /// the effect sheets do not carry themselves.
    pub fn load(data: &BTreeMap<String, Vec<u8>>) -> AppResult<Self> {
        let smoke = Pic::parse(data.get("SMOKE.PIC").ok_or("missing SMOKE.PIC")?)?;
        if smoke.width != 256 || smoke.height != 43 {
            return Err("unreviewed smoke sheet dimensions".into());
        }
        Ok(Self {
            smoke,
            shapes: BTreeMap::new(),
            effects: crate::effect_renderer::Art::load(data),
            escape: match crate::ejection_art::Art::load(data) {
                Ok(art) => Some(art),
                Err(error) => {
                    log::warn!("Optional ejection artwork unavailable: {error}");
                    None
                }
            },
        })
    }
    /// Loads every weapon shape a configuration's stations name.
    pub fn add_weapon_shapes(
        &mut self,
        config: &live::Configuration,
        data: &BTreeMap<String, Vec<u8>>,
    ) {
        self.add_shapes(
            config
                .stations
                .iter()
                .filter_map(|station| station.weapon.shape.as_deref()),
            data,
        );
    }
    /// Loads named weapon shapes; a missing or empty one draws as a strip.
    pub fn add_shapes<'a>(
        &mut self,
        names: impl IntoIterator<Item = &'a str>,
        data: &BTreeMap<String, Vec<u8>>,
    ) {
        for name in names {
            let shape = data
                .get(name)
                .and_then(|bytes| Shape::parse(bytes).ok())
                .filter(|shape| !shape.faces.is_empty());
            match shape {
                Some(shape) => {
                    self.shapes.insert(name.to_owned(), shape);
                }
                None => log::warn!(
                    "Combat: {name} uses a tracer marker; line/point drawing remains open"
                ),
            }
        }
    }
    /// Synthetic art for drawing tests without retail media.
    #[cfg(test)]
    pub(crate) fn synthetic(shapes: BTreeMap<String, Shape>) -> Self {
        Self {
            smoke: Pic {
                width: 1,
                height: 1,
                pixels: vec![0],
                mask: vec![true],
                palette: Vec::new(),
                glyphs: Vec::new(),
            },
            shapes,
            effects: crate::effect_renderer::Art::empty(),
            escape: None,
        }
    }
}

fn mesh(
    out: &mut Vec<f32>,
    shape: &Shape,
    position: Vector,
    right: Vector,
    up: Vector,
    forward: Vector,
    palette: &[[u8; 3]; 256],
) {
    for face in &shape.faces {
        // These are texture-only SH faces (including the missile exhaust
        // sheets). Palette index zero is not an opaque substitute for them.
        // Keep them omitted until the weapon texture/animation path is decoded.
        if matches!(face.subtype, 0x4c | 0x5c | 0x6c | 0x7c) {
            continue;
        }
        for i in 1..face.positions.len() - 1 {
            for j in [0, i, i + 1] {
                let q = face.positions[j];
                let pos = std::array::from_fn(|k| {
                    position[k]
                        + (right[k] * f64::from(q[0])
                            + up[k] * f64::from(q[2])
                            + forward[k] * f64::from(q[1]))
                            / 3.
                });
                vertex(
                    out,
                    pos,
                    palette[face.colors[j] as usize].map(|c| f32::from(c) / 255.),
                );
                let layer = out.len() - 5;
                out[layer] = -1.;
            }
        }
    }
}
/// How bright a gun's tracer is drawn against the ordinary one: the AC-130's
/// 105 mm round half as bright again (John, 2026-10-09), so it reads at
/// gunship ranges.
pub const HOWITZER_TRACER: f64 = 1.5;
fn tracer_brightness(weapon: &str) -> f64 {
    if weapon == tore_sim::combat::gunship::GUNS[2] {
        HOWITZER_TRACER
    } else {
        1.
    }
}
/// The vertex color that the shader's sRGB decode turns into `linear`, so a
/// tracer's brightness survives the decode the world vertices go through.
fn encoded(linear: f64) -> f32 {
    if linear <= 0.0031308 {
        (linear * 12.92) as f32
    } else {
        (1.055 * linear.powf(1. / 2.4) - 0.055) as f32
    }
}
/// Camera-facing luminous ribbon over the actual swept gun segment, at
/// `brightness` times the ordinary tracer's radiance.
fn tracer(
    out: &mut Vec<f32>,
    previous: Vector,
    position: Vector,
    camera: &Camera,
    brightness: f64,
) {
    let level = encoded(brightness);
    let segment: Vector = std::array::from_fn(|i| position[i] - previous[i]);
    if tore_sim::attitude::dot(segment, segment) < 1e-12 {
        return;
    }
    let view: Vector = std::array::from_fn(|i| camera.position[i] - position[i]);
    let cross = cross(segment, view);
    let (start, ribbon, side) = if tore_sim::attitude::dot(cross, cross)
        > 0.25 * tore_sim::attitude::dot(view, view).max(1e-12)
    {
        (previous, segment, unit(cross))
    } else {
        // Viewed along its path, retain a small glow instead of collapsing
        // the ribbon into a line with zero screen area.
        let basis = Basis::new(f64::from(camera.yaw), f64::from(camera.pitch), 0.);
        (
            std::array::from_fn(|i| position[i] - basis.up[i] * 0.25),
            basis.up.map(|v| v * 0.5),
            basis.right,
        )
    };
    for [along, across] in [
        [0., -1.],
        [1., -1.],
        [1., 1.],
        [0., -1.],
        [1., 1.],
        [0., 1.],
    ] {
        let pos: Vector =
            std::array::from_fn(|i| start[i] + ribbon[i] * along + side[i] * across * 1.2);
        out.extend([
            pos[0] as f32,
            pos[1] as f32,
            pos[2] as f32,
            along as f32,
            across as f32,
            -8.,
            level,
            level,
            level,
            -1.,
        ]);
    }
}

fn vertex(out: &mut Vec<f32>, pos: Vector, color: [f32; 3]) {
    // Trailing -1 opts out of the weather palette: this color is already resolved.
    out.extend([
        pos[0] as f32,
        pos[1] as f32,
        pos[2] as f32,
        0.,
        0.,
        -6., // Emissive effect; mesh() opts solid weapon bodies into lighting.
        color[0],
        color[1],
        color[2],
        -1.,
    ]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn formation(count: usize, mixed: bool) -> RenderSnapshot {
        let mut snapshot = RenderSnapshot {
            models: if mixed {
                // Deliberately differ from asset order, retain an empty loaded
                // model and a missing model, and repeat one batch identity.
                vec![
                    AircraftId::Rafale,
                    AircraftId::F18,
                    AircraftId::F14,
                    AircraftId::Mig29,
                    AircraftId::F18,
                ]
            } else {
                vec![AircraftId::F18]
            },
            ..Default::default()
        };
        for i in 0..count {
            let id = if mixed && i % 2 == 0 {
                AircraftId::Rafale
            } else {
                AircraftId::F18
            };
            snapshot.targets.push(AircraftPose {
                id: i as u32 + 1,
                aircraft: Some(id),
                draw: Draw::Model(id),
                position: [50. * i as f64, 5000. + i as f64, 1000. + 31. * i as f64],
                attitude: [0.07 * i as f64, -0.01 * i as f64, 0.1 * i as f64],
                devices: (i % 3 != 0).then_some([
                    1., 0.6, 0.1, 0.4, 0.3, 0.6, 0.2, -0.3, 0.1, 670., 0.75, 0., 0., 0., 0., 0.,
                    0., 0., 0., 0., 0., 0.,
                ]),
                damage: crate::snapshot::Damage {
                    hp: if i % 7 == 0 { 0 } else { 85 },
                    initial_hp: 100,
                    sections: [0, 0, 0, 15, 0, 0],
                    structural: Some(live::DamageSection::LeftWing),
                },
                airborne: i % 11 != 0,
                ..Default::default()
            });
        }
        for (index, draw) in [Draw::Hidden, Draw::Ownship, Draw::Model(AircraftId::Mig29)]
            .into_iter()
            .enumerate()
        {
            snapshot.targets.push(AircraftPose {
                id: 1000 + index as u32,
                draw,
                airborne: true,
                ..Default::default()
            });
        }
        for (index, draw) in [
            Draw::Model(AircraftId::Rafale),
            Draw::Model(AircraftId::F18),
            Draw::Ownship,
            Draw::Model(AircraftId::F18),
        ]
        .into_iter()
        .enumerate()
        {
            snapshot.debris.push(DebrisPose {
                owner: index as u32 + 1,
                draw,
                position: [-30. * index as f64, 4900., 1000.],
                attitude: [0.25, 0.6, index as f64],
                // Exercise an empty debris job as well as drawn fragments.
                variant: (index != 3).then_some(live::DamageSection::LeftWing as usize),
            });
        }
        snapshot
    }

    fn assert_batches_identical(
        expected: &[(&Airframe, Vec<f32>, Vec<Contact>)],
        actual: &[(&Airframe, Vec<f32>, Vec<Contact>)],
    ) {
        assert_eq!(actual.len(), expected.len());
        for ((expected_model, expected_vertices, expected_contacts), (model, vertices, contacts)) in
            expected.iter().zip(actual)
        {
            assert_eq!(model.profile.id, expected_model.profile.id);
            assert_eq!(
                vertices.iter().map(|v| v.to_bits()).collect::<Vec<_>>(),
                expected_vertices
                    .iter()
                    .map(|v| v.to_bits())
                    .collect::<Vec<_>>(),
                "vertex bytes for {:?}",
                model.profile.id,
            );
            assert_eq!(contacts.len(), expected_contacts.len());
            for (actual, expected) in contacts.iter().zip(expected_contacts) {
                assert_eq!(
                    (actual.first, actual.count),
                    (expected.first, expected.count)
                );
                assert_eq!(
                    actual.center.map(f32::to_bits),
                    expected.center.map(f32::to_bits)
                );
                assert_eq!(actual.extent.to_bits(), expected.extent.to_bits());
            }
        }
    }

    #[test]
    fn aircraft_workers_preserve_original_vertices_contacts_and_order() {
        let models = crate::combat_view::render_hash_tests::models();
        let world = tore_world::test_support::terrain();
        let mut scenery = crate::scenery::tests::scenery();
        scenery.set_origin([20_000., 4500., -11_000.]);
        let mut executors = vec![tore_workers::Executor::serial()];
        executors
            .extend([1, 2, 4, 8].map(|workers| tore_workers::Executor::parallel(workers).unwrap()));
        executors.extend([1, 73, 991].map(tore_workers::Executor::shuffled));
        for snapshot in [
            formation(30, false),
            formation(30, true),
            RenderSnapshot::default(),
        ] {
            for smooth in [false, true] {
                scenery.smooth_weather = smooth;
                for camera in crate::combat_view::render_hash_tests::cameras() {
                    let expected =
                        aircraft_batches_serial(&snapshot, &models, &camera, &world, &scenery);
                    for executor in &executors {
                        let actual = aircraft_batches_with(
                            executor, &snapshot, &models, &camera, &world, &scenery,
                        );
                        assert_batches_identical(&expected, &actual);
                    }
                    if smooth && !snapshot.models.is_empty() {
                        let contacts: usize = expected.iter().map(|(_, _, c)| c.len()).sum();
                        assert!(contacts >= 20, "the formation must actually draw contacts");
                    }
                }
            }
        }
    }

    #[test]
    fn aircraft_workers_keep_the_small_inline_fallback() {
        let models = crate::combat_view::render_hash_tests::models();
        let world = tore_world::test_support::terrain();
        let scenery = crate::scenery::tests::scenery();
        let camera = Camera::new();
        let executor = tore_workers::Executor::parallel(2).unwrap();
        for count in 0..MIN_AIRCRAFT_JOBS {
            let mut snapshot = formation(count, false);
            snapshot.debris.clear();
            let expected = aircraft_batches_serial(&snapshot, &models, &camera, &world, &scenery);
            let actual =
                aircraft_batches_with(&executor, &snapshot, &models, &camera, &world, &scenery);
            assert_batches_identical(&expected, &actual);
        }
    }

    /// Portable CPU-only probe. Run separately on a quiet machine in release
    /// mode; actual frame gains still need the imported scene and graphics host.
    #[test]
    #[ignore = "opt-in elapsed geometry timing; run in release mode on a quiet machine"]
    fn aircraft_geometry_wall_time() {
        use std::{hint::black_box, time::Instant};
        let models = crate::combat_view::render_hash_tests::models();
        let world = tore_world::test_support::terrain();
        let scenery = crate::scenery::tests::scenery();
        let camera = crate::combat_view::render_hash_tests::cameras().remove(0);
        for count in [1, 4, 30] {
            let mut snapshot = formation(count, false);
            snapshot.debris.clear();
            snapshot
                .targets
                .retain(|target| target.draw == Draw::Model(AircraftId::F18));
            for target in &mut snapshot.targets {
                target.airborne = true;
            }
            let expected = aircraft_batches_serial(&snapshot, &models, &camera, &world, &scenery);
            for workers in [0, 1, 2, 4, 8] {
                let executor = if workers == 0 {
                    tore_workers::Executor::serial()
                } else {
                    tore_workers::Executor::parallel(workers).unwrap()
                };
                let build = || {
                    aircraft_batches_with(&executor, &snapshot, &models, &camera, &world, &scenery)
                };
                assert_batches_identical(&expected, &build());
                for _ in 0..20 {
                    black_box(build());
                }
                let start = Instant::now();
                let rounds = 500;
                for _ in 0..rounds {
                    black_box(build());
                }
                println!(
                    "aircraft_geometry aircraft={count} workers={workers} rounds={rounds} elapsed_us={:.3}",
                    start.elapsed().as_secs_f64() * 1e6 / f64::from(rounds),
                );
            }
        }
    }

    #[test]
    fn tracer_ribbon_is_finite_camera_facing_and_visible_end_on() {
        let mut camera = Camera::new();
        camera.position = [0., 0., -100.];
        camera.yaw = 0.;
        camera.pitch = 0.;
        for end in [[20., 0., 0.], [0., 0., 20.]] {
            let mut output = Vec::new();
            tracer(&mut output, [0.; 3], end, &camera, 1.);
            assert_eq!(output.len(), 60);
            assert!(output.iter().all(|v| v.is_finite()));
            let points: Vec<[f32; 2]> = output.chunks_exact(10).map(|v| [v[0], v[1]]).collect();
            let a = [points[1][0] - points[0][0], points[1][1] - points[0][1]];
            let b = [points[2][0] - points[0][0], points[2][1] - points[0][1]];
            assert!((a[0] * b[1] - a[1] * b[0]).abs() > 0.1);
            assert!(output.chunks_exact(10).all(|v| v[5] == -8.));
        }
        let mut output = Vec::new();
        tracer(&mut output, [0.; 3], [0.; 3], &camera, 1.);
        assert!(output.is_empty());
    }

    #[test]
    fn the_105_mm_tracer_is_half_as_bright_again_after_the_srgb_decode() {
        assert_eq!(tracer_brightness("C_105.JT"), 1.5);
        assert_eq!(tracer_brightness("C_25.JT"), 1.);
        assert_eq!(tracer_brightness("M61A1.GN"), 1.);
        // The shader decodes the vertex color from sRGB, as terrain.wgsl's
        // `linear` does; the ordinary tracer keeps its old white exactly.
        let decode = |c: f32| {
            let c = f64::from(c);
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        assert_eq!(encoded(1.), 1.);
        assert!((decode(encoded(1.5)) - 1.5).abs() < 1e-5);
        let camera = Camera::new();
        let mut output = Vec::new();
        tracer(&mut output, [0.; 3], [20., 0., 0.], &camera, 1.5);
        assert!(output.chunks_exact(10).all(|v| v[6] == encoded(1.5)));
    }

    #[test]
    fn palette_mesh_does_not_turn_texture_only_exhaust_into_solid_faces() {
        let face = tore_formats::shape::Face {
            fog: tore_formats::shape::FogMode::Enabled,
            positions: vec![[0., 0., 0.], [3., 0., 0.], [0., 3., 0.]],
            colors: vec![1; 3],
            uv: vec![],
            texture: "SYNTHETIC.PIC".into(),
            subtype: 0x61,
            normal: None,
            address: 0,
        };
        let mut exhaust = face.clone();
        exhaust.subtype = 0x4c;
        let shape = Shape {
            billboards: Vec::new(),
            lines: vec![],
            faces: vec![face, exhaust],
            state_words: Default::default(),
        };
        let mut out = vec![];
        mesh(
            &mut out,
            &shape,
            [0.; 3],
            [1., 0., 0.],
            [0., 1., 0.],
            [0., 0., 1.],
            &[[255; 3]; 256],
        );
        // One triangle of ten-float vertices; the exhaust face is omitted.
        assert_eq!(out.len(), 30);
    }
}
