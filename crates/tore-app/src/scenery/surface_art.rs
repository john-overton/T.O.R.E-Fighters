//! Surface units in the picture (docs/spec/surface-defenses.md, "Destroyed
//! looks and drawing"): what the static airport batch cannot hold by itself.
//!
//! - **Destroyed looks.** A unit whose wreck stays (a ship's `_A` hull, the
//!   `DEST.OT` wreck of a vehicle, SAM launcher or gun, a bunker's damaged
//!   `~` variant, a carrier's damaged island) has its wreck built beside its
//!   standing geometry, and the batch swaps one for the other when combat
//!   destroys it.
//! - **Launcher rails.** A launcher shape that draws one missile per round
//!   still loaded (CHAP, SA2, SA3, SCD) is redrawn from its hardpoints' loads
//!   whenever they change, through `shape::loaded_count_word`.
//! - **Carriers.** A carrier hull carries its island and deck parts from the
//!   FA.EXE carrier table (`tore_formats::carrier`), each standing on the
//!   flight deck by its ground offset, at the hull's placed scale.
//! - **Every frame** ([`SurfaceArt::frame`]): the units that follow a route
//!   where combat moved them, the men and deck crew as viewer-facing sprites,
//!   and the pieces a destroyed parked aircraft throws.
//!
//! Every texture these may draw is appended to the world pages when the
//! scenery is built, so the renderer uploads them once.
use crate::{AppResult, camera::Camera, static_art::Image};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{
    Pic,
    shape::{
        Billboard, CURRENT_TICKS_WORD, DAMAGED_WORD, LOADED_COUNT_BASE, SPRITE_FRAME_WORD, Shape,
        hardpoint_angle_word, loaded_count_word,
    },
    static_object::Definition,
};
use tore_sim::attitude::{Basis, Vector};
use tore_world::surface::{
    DestroyedLook, Surface, SurfaceState, UnitId,
    aim::{self, Aims, Rule, Track},
};

/// Floats in one vertex of the world batches.
const FLOATS: usize = 10;
/// The static scene's geometry budget, in floats (32 MiB).
pub(super) const BUDGET: usize = 32 * 1024 * 1024 / 4;
/// The deck crew sprite's frames (`shape::SPRITE_FRAME_WORD`, 0 to 10).
const CREW_FRAMES: i32 = 11;
/// Frames per second of the deck crew's signal cycle (fitted: the native
/// frame choice is not traced).
const CREW_FPS: f64 = 4.;

/// Textures already in the world pages, by upper-case name.
pub(super) type Layers = BTreeMap<String, Image>;

/// Where a shape stands: feet per unit, orientation and origin.
#[derive(Clone, Copy, Debug)]
pub(super) struct Stand {
    pub scale: f64,
    pub basis: Basis,
    pub origin: Vector,
}

/// Triangles and lines of the world batches, ten floats a vertex.
#[derive(Clone, Debug, Default)]
pub(super) struct Drawn {
    pub vertices: Vec<f32>,
    pub lines: Vec<f32>,
}

impl Drawn {
    fn extend(&mut self, other: &Drawn) {
        self.vertices.extend_from_slice(&other.vertices);
        self.lines.extend_from_slice(&other.lines);
    }
}

/// Appends every texture `shape` draws with to the world pages, once each.
/// `first_page` is the page count before the pages vector (the terrain's).
pub(super) fn preload(
    shape: &Shape,
    resources: &BTreeMap<String, Vec<u8>>,
    first_page: usize,
    pages: &mut Vec<u8>,
    layers: &mut Layers,
) -> AppResult<()> {
    let names = shape
        .faces
        .iter()
        .filter(|face| !face.texture.is_empty() && !face.uv.is_empty())
        .map(|face| &face.texture)
        .chain(shape.billboards.iter().map(|sprite| &sprite.texture));
    for name in names {
        let name = name.to_ascii_uppercase();
        if layers.contains_key(&name) {
            continue;
        }
        let pic = Pic::parse(
            resources
                .get(&name)
                .ok_or_else(|| format!("missing static texture {name}"))?,
        )?;
        let first = (first_page + pages.len()) / 65536;
        layers.insert(name, Image::append(&pic, pages, first)?);
    }
    Ok(())
}

/// One world vertex: position, texture coordinates, layer (-1 untextured),
/// colour slots and the palette index with its fog mode.
fn vertex(position: Vector, uv: [f32; 2], index: f32) -> [f32; FLOATS] {
    [
        position[0] as f32,
        position[1] as f32,
        position[2] as f32,
        uv[0],
        uv[1],
        -1.0,
        0.0,
        0.0,
        0.0,
        index,
    ]
}

/// Appends `shape`'s faces and lines standing at `stand` to `out`, as the
/// static scene draws a placement: shape axes right, forward, up; textures
/// from `layers` (a face whose texture is not there draws in its colour).
/// `origin` may be a world point (the static batch) or one relative to the
/// render origin (the moving batch). Fails past `limit` floats.
pub(super) fn emit(
    shape: &Shape,
    stand: &Stand,
    layers: &Layers,
    out: &mut Drawn,
    limit: usize,
) -> AppResult<()> {
    let Stand {
        scale,
        basis,
        origin,
    } = *stand;
    let place = |point: [f32; 3]| -> Vector {
        let right = f64::from(point[0]) * scale;
        let up = f64::from(point[2]) * scale;
        let forward = f64::from(point[1]) * scale;
        std::array::from_fn(|axis| {
            origin[axis]
                + basis.right[axis] * right
                + basis.up[axis] * up
                + basis.forward[axis] * forward
        })
    };
    for face in &shape.faces {
        if face.positions.len() < 3 {
            continue;
        }
        let image = if face.texture.is_empty() || face.uv.is_empty() {
            None
        } else {
            layers.get(&face.texture.to_ascii_uppercase())
        };
        for triangle in 1..face.positions.len() - 1 {
            if out.vertices.len() + 30 > limit {
                return Err("static scene exceeds 32 MiB geometry budget".into());
            }
            let points: Vec<[f32; FLOATS]> = [0, triangle, triangle + 1]
                .into_iter()
                .map(|at| {
                    let uv = face.uv.get(at).copied().unwrap_or([0.0; 2]);
                    let uv = image.map_or([0., 0.], |image| {
                        [uv[0] + 0.5, image.height as f32 - 0.5 - uv[1]]
                    });
                    vertex(
                        place(face.positions[at]),
                        uv,
                        f32::from(face.colors[at]) + f32::from(face.fog as u8) * 256.,
                    )
                })
                .collect();
            match image {
                Some(image) => image.triangle(
                    points.try_into().expect("three triangle corners"),
                    &mut out.vertices,
                    limit,
                )?,
                None => out.vertices.extend(points.into_iter().flatten()),
            }
        }
    }
    for line in &shape.lines {
        for point in line.positions {
            let mut v = vertex(
                place(point),
                [0., 0.],
                f32::from(line.color) + f32::from(line.fog as u8) * 256.,
            );
            v[5] = -1.;
            out.lines.extend(v);
        }
    }
    Ok(())
}

/// A sprite turned to face the viewer whose right and up are the world
/// vectors `right` and `up`, as a face in `stand`'s shape axes.
fn facing(
    sprite: &Billboard,
    stand: &Stand,
    right: Vector,
    up: Vector,
) -> tore_formats::shape::Face {
    let local = |v: Vector| -> [f32; 3] {
        let along = |axis: Vector| (0..3).map(|i| v[i] * axis[i]).sum::<f64>() as f32;
        [
            along(stand.basis.right),
            along(stand.basis.forward),
            along(stand.basis.up),
        ]
    };
    sprite.face(local(right), local(up))
}

/// A shape with the scale it is drawn at.
pub(super) struct Model {
    pub shape: Shape,
    pub scale: f64,
}

/// A unit that follows a route: drawn where combat moved it.
struct Mover {
    intact: Model,
    wreck: Option<Model>,
    /// Feet its shape stands above its pose (a land unit on its lowest
    /// point, as its standing placement and hit box).
    lift: f64,
    /// The same for its wreck, on the wreck's own lowest point.
    wreck_lift: f64,
}

/// Feet a land unit's shape stands above its placement point so its lowest
/// vertex (track or wheel bottom) meets the ground: the shape's lowest point
/// at `scale`, never negative.
pub(super) fn ground_lift(shape: &Shape, scale: f64) -> f64 {
    let low = shape
        .faces
        .iter()
        .flat_map(|face| face.positions.iter().map(|p| f64::from(p[2])))
        .fold(f64::INFINITY, f64::min);
    if low.is_finite() {
        (-low * scale).max(0.)
    } else {
        0.
    }
}

/// A unit whose picture aims (docs/spec/surface-defenses.md, "Aiming"):
/// its body turns, or its turrets, and its antennas spin. Drawn every frame
/// while anything is turned or spins, in the static batch otherwise.
struct Aimer {
    /// Where it stands when it does not follow a route.
    stand: Stand,
    /// Its main shape's bytes, projected again for turrets, antennas and
    /// rails.
    bytes: Vec<u8>,
    /// The scenery look, for a body that turns with nothing else changed.
    intact: Shape,
    /// Turrets: hardpoint and rest heading (binary angle units).
    turrets: Vec<(u8, i16)>,
    /// Its rotating antennas spin with the mission clock.
    antennas: bool,
    /// The hardpoints whose loads the shape draws and their full loads.
    rails: Vec<u8>,
    full: Vec<u32>,
    /// It follows a route (drawn at its pose).
    routed: bool,
}

/// An aimer's projected shape: the cached one for its rails, or one made
/// for this frame's turrets and antennas.
enum Projected<'a> {
    Cached(&'a Shape),
    Fresh(Shape),
}

impl Projected<'_> {
    fn shape(&self) -> &Shape {
        match self {
            Projected::Cached(shape) => shape,
            Projected::Fresh(shape) => shape,
        }
    }
}

impl Aimer {
    /// Nothing but its body turns: the shape depends only on the rails.
    fn body_only(&self) -> bool {
        self.turrets.is_empty() && !self.antennas
    }
    /// The rails' loaded-count words for `loads` (none: all full).
    fn rail_words(&self, loads: Option<&Vec<u32>>, state: &mut BTreeMap<usize, i32>) {
        for (at, hardpoint) in self.rails.iter().enumerate() {
            let loaded = loads.map_or(self.full[at], |l| l[at]);
            if loaded > 0 {
                state.insert(loaded_count_word(*hardpoint), loaded as i32);
            }
        }
    }
}

/// `basis` turned `angle` radians about its own up axis, positive toward
/// the right.
fn yawed(basis: Basis, angle: f64) -> Basis {
    if angle == 0. {
        return basis;
    }
    let (sin, cos) = angle.sin_cos();
    Basis {
        right: std::array::from_fn(|i| basis.right[i] * cos - basis.forward[i] * sin),
        up: basis.up,
        forward: std::array::from_fn(|i| basis.forward[i] * cos + basis.right[i] * sin),
    }
}

/// A traverse in radians as the binary angle units `@HardpointAngle@4`
/// returns (65,536 a turn).
fn binary_angle(radians: f64) -> i32 {
    (radians * 65536. / std::f64::consts::TAU).round() as i32
}

/// `_currentTicks` (256 a second) at mission time `seconds`.
fn current_ticks(seconds: f64) -> i32 {
    (seconds.max(0.) * 256.) as i64 as i32
}

/// A launcher whose rails show its load.
struct Launcher {
    bytes: Vec<u8>,
    stand: Stand,
    /// The hardpoints whose loads the shape draws.
    hardpoints: Vec<u8>,
    /// Their full loads: the scenery look.
    full: Vec<u32>,
}

/// A viewer-facing sprite standing in the scene: a man, or a carrier's
/// deck crew (one sprite per signal frame, front and back sheets).
struct Sprite {
    /// The object whose loss removes it: the man, or the crew's carrier.
    owner: u32,
    stand: Stand,
    sprites: Vec<Billboard>,
    crew: Option<[Vec<Billboard>; 2]>,
}

/// What a static object looks like now, when not standing intact.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Look {
    /// Not drawn: destroyed with nothing left, or never a combat target.
    Hidden,
    /// Its wreck.
    Wreck,
    /// A launcher with these loads on its drawn hardpoints.
    Rails(Vec<u8>),
    /// Aiming: drawn every frame ([`SurfaceArt::frame`]), not in the batch.
    Aimed,
}

/// The surface units' art beside the static batch. See the module comment.
#[derive(Default)]
pub struct SurfaceArt {
    pub(super) layers: Layers,
    wrecks: BTreeMap<u32, Drawn>,
    launchers: BTreeMap<u32, Launcher>,
    rails: BTreeMap<(u32, Vec<u8>), Drawn>,
    movers: BTreeMap<u32, Mover>,
    sprites: Vec<Sprite>,
    /// A parked aircraft's pieces 0 and 1 (`{stem}_B`, `{stem}_D`).
    debris: BTreeMap<u32, [Option<Model>; 2]>,
    /// Units whose pictures aim, how each turns, and their aim.
    aimers: BTreeMap<u32, Aimer>,
    rules: BTreeMap<UnitId, Rule>,
    aims: Aims,
    /// The loads an aimer's rails show now, when not full.
    aimer_loads: BTreeMap<u32, Vec<u32>>,
    /// The projected shape of a body-only aimer (no turret, no antenna) for
    /// its loads in `aimer_loads`, made when the loads change rather than
    /// every frame. `None`: the projection failed, so the intact look draws.
    rail_shapes: BTreeMap<u32, Option<Shape>>,
}

/// What [`SurfaceArt::add`] needs from the scenery build.
pub(super) struct Build<'a> {
    pub resources: &'a BTreeMap<String, Vec<u8>>,
    pub terrain: &'a tore_world::terrain::Terrain,
    pub definitions: &'a BTreeMap<String, Definition>,
    pub first_page: usize,
    pub pages: &'a mut Vec<u8>,
}

impl Build<'_> {
    fn bytes(&self, name: &str) -> AppResult<&Vec<u8>> {
        self.resources
            .get(name)
            .ok_or_else(|| format!("missing {name}; re-import media").into())
    }
    /// A shape's scenery look, its textures preloaded.
    fn scenery(&mut self, layers: &mut Layers, name: &str) -> AppResult<Shape> {
        self.shape(layers, name, None)
    }
    fn shape(
        &mut self,
        layers: &mut Layers,
        name: &str,
        state: Option<&BTreeMap<usize, i32>>,
    ) -> AppResult<Shape> {
        let bytes = self.bytes(name)?;
        let shape = match state {
            Some(state) => Shape::with_export_state(bytes, state),
            None => Shape::scenery(bytes),
        }
        .map_err(|e| format!("{name}: {e}"))?;
        preload(&shape, self.resources, self.first_page, self.pages, layers)?;
        Ok(shape)
    }
    /// An object type's main shape and its placed scale.
    fn object(&mut self, layers: &mut Layers, resource: &str) -> AppResult<Model> {
        let definition = Definition::parse(self.bytes(resource)?)?;
        let name = definition
            .main_shape
            .clone()
            .ok_or_else(|| format!("{resource} names no shape"))?;
        let scale = tore_world::terrain::placed_shape_scale(&definition, self.bytes(&name)?)?;
        Ok(Model {
            shape: self.scenery(layers, &name)?,
            scale,
        })
    }
}

impl SurfaceArt {
    /// Prepares the extra looks of placement `id` (object type
    /// `object_type`, intact geometry `intact` already built at `stand`).
    /// Returns geometry to add to the placement's intact look (a carrier's
    /// parts) and whether the placement is drawn every frame instead of in
    /// the static batch (it follows a route).
    pub(super) fn add(
        &mut self,
        build: &mut Build,
        id: u32,
        object_type: &str,
        intact: &Shape,
        stand: &Stand,
        lift: f64,
    ) -> AppResult<(Drawn, bool)> {
        let surface = &build.terrain.surface;
        let unit = surface.unit(UnitId(id));
        let definition = build.definitions.get(object_type);
        let main_shape = definition.and_then(|d| d.main_shape.clone());
        // Its wreck: the unit's look, or for any other building its `~`
        // damaged variant when the import holds one (as the bunkers'), else
        // nothing (removed).
        let look = match unit {
            Some(unit) => unit.look.clone(),
            None => {
                let damaged = format!("~{object_type}");
                if !object_type.starts_with('~') && build.resources.contains_key(&damaged) {
                    DestroyedLook::DamagedObject(damaged)
                } else {
                    DestroyedLook::Removed
                }
            }
        };
        let wreck = (|| -> AppResult<Option<Model>> {
            Ok(match &look {
                DestroyedLook::DamagedShape(name) => match definition {
                    Some(definition) => {
                        let scale = tore_world::terrain::placed_shape_scale(
                            definition,
                            build.bytes(name)?,
                        )?;
                        Some(Model {
                            shape: build.scenery(&mut self.layers, name)?,
                            scale,
                        })
                    }
                    None => None,
                },
                DestroyedLook::Wreck(object) | DestroyedLook::DamagedObject(object) => {
                    Some(build.object(&mut self.layers, object)?)
                }
                DestroyedLook::Removed | DestroyedLook::Vanish => None,
            })
        })()
        .unwrap_or_else(|error| {
            log::warn!("Surface: {object_type} {id:#010x} has no destroyed look: {error}");
            None
        });
        // The men stand as sprites.
        if !intact.billboards.is_empty() {
            self.sprites.push(Sprite {
                owner: id,
                stand: *stand,
                sprites: intact.billboards.clone(),
                crew: None,
            });
        }
        // A land unit's wreck lies on its own lowest point too (the DEST
        // wreck reaches below its origin); a ship's `_A` hull keeps the
        // waterline and a building's variant its depth.
        let land = definition.is_some_and(tore_world::terrain::stands_on_wheels);
        let wreck_lift = match &wreck {
            Some(model) if land => ground_lift(&model.shape, model.scale),
            _ => 0.,
        };
        if let Some(name) = &main_shape
            && let Err(error) = self.add_aimer(build, id, object_type, name, intact, stand)
        {
            log::warn!("Surface: {object_type} {id:#010x} does not aim: {error}");
        }
        if surface.courses.contains_key(&UnitId(id))
            && let Some(name) = &main_shape
        {
            let shape = build.scenery(&mut self.layers, name)?;
            self.movers.insert(
                id,
                Mover {
                    intact: Model {
                        shape,
                        scale: stand.scale,
                    },
                    wreck,
                    lift,
                    wreck_lift,
                },
            );
            return Ok((Drawn::default(), true));
        }
        let mut wreck_drawn = wreck.map(|model| -> AppResult<Drawn> {
            let mut drawn = Drawn::default();
            // The wreck lies where the unit stood, on the ground.
            emit(
                &model.shape,
                &Stand {
                    scale: model.scale,
                    basis: stand.basis,
                    origin: std::array::from_fn(|i| {
                        stand.origin[i] + stand.basis.up[i] * (wreck_lift - lift)
                    }),
                },
                &self.layers,
                &mut drawn,
                BUDGET,
            )?;
            Ok(drawn)
        });
        let mut parts = Drawn::default();
        if let Some(carrier) = main_shape
            .as_deref()
            .and_then(tore_formats::carrier::for_hull)
        {
            match self.carrier(build, id, carrier, stand) {
                Ok((intact_parts, damaged_parts)) => {
                    parts = intact_parts;
                    if let Some(Ok(wreck)) = wreck_drawn.as_mut() {
                        wreck.extend(&damaged_parts);
                    }
                }
                Err(error) => log::warn!("Surface: {} parts not drawn: {error}", carrier.hull),
            }
        }
        if let Some(wreck) = wreck_drawn {
            self.wrecks.insert(id, wreck?);
        }
        // A launcher whose shape draws its load.
        if let Some(name) = &main_shape
            && let Some(arms) = surface.arsenal.arms(UnitId(id))
        {
            let bytes = build.bytes(name)?.clone();
            let hardpoints = Shape::with_export_state(&bytes, &BTreeMap::new())
                .map(|shape| rail_hardpoints(&shape))
                .unwrap_or_default();
            if !hardpoints.is_empty() {
                let full = hardpoints
                    .iter()
                    .map(|hp| arms.loads.get(usize::from(*hp)).map_or(0, |m| m.loaded))
                    .collect();
                self.launchers.insert(
                    id,
                    Launcher {
                        bytes,
                        stand: *stand,
                        hardpoints,
                        full,
                    },
                );
            }
        }
        Ok((parts, false))
    }

    /// Registers unit `id` (object type `object_type`, main shape `name`)
    /// as an aimer when its picture can turn: a shape with turrets (its
    /// mounts traverse) or rotating antennas, or an armed land unit whose
    /// type turns (its whole body turns; docs/spec/surface-defenses.md,
    /// "Aiming").
    fn add_aimer(
        &mut self,
        build: &mut Build,
        id: u32,
        object_type: &str,
        name: &str,
        intact: &Shape,
        stand: &Stand,
    ) -> AppResult<()> {
        let surface = &build.terrain.surface;
        let Some(unit) = surface.unit(UnitId(id)) else {
            return Ok(());
        };
        let bytes = build.bytes(name)?.clone();
        let parts = tore_formats::shape::articulation(&bytes)?;
        let arms = surface.arsenal.arms(UnitId(id));
        use tore_formats::surface_unit::class;
        let land = unit.class & (class::SHIP | class::STRUCTURE) == 0;
        let rule = if !parts.turrets.is_empty() {
            arms.map(|_| {
                Rule::Turrets(
                    parts
                        .turrets
                        .iter()
                        .map(|(hardpoint, _)| usize::from(*hardpoint))
                        .collect(),
                )
            })
        } else if land && arms.is_some() && !intact.faces.is_empty() {
            // The type's turn rate (`_turnRate`, 182 a degree, a second): a
            // fixed site (the SA-2's six rails) reads 0 and does not turn.
            let record = tore_formats::surface_unit::SurfaceUnit::parse(build.bytes(object_type)?)?;
            let rate = f64::from(record.movement.turn_rate)
                / f64::from(tore_formats::surface_unit::ANGLE_UNITS_PER_DEGREE);
            (rate > 0.).then_some(Rule::Body { rate_deg_s: rate })
        } else {
            None
        };
        if rule.is_none() && parts.antennas.is_empty() {
            return Ok(());
        }
        let rails = Shape::with_export_state(&bytes, &BTreeMap::new())
            .map(|shape| rail_hardpoints(&shape))
            .unwrap_or_default();
        let full = rails
            .iter()
            .map(|hp| {
                arms.and_then(|a| a.loads.get(usize::from(*hp)))
                    .map_or(0, |m| m.loaded)
            })
            .collect();
        if let Some(rule) = rule {
            self.rules.insert(UnitId(id), rule);
        }
        self.aimers.insert(
            id,
            Aimer {
                stand: *stand,
                bytes,
                intact: build.scenery(&mut self.layers, name)?,
                turrets: parts.turrets,
                antennas: !parts.antennas.is_empty(),
                rails,
                full,
                routed: surface.courses.contains_key(&UnitId(id)),
            },
        );
        Ok(())
    }

    /// Steps every aimer to `picture`'s tick: `tracks` are what the units'
    /// weapons hold, `surface` the mission's units and arms, `stock` their
    /// launchers' loads when the viewer knows them.
    pub fn step_aim(
        &mut self,
        picture: &crate::snapshot::RenderSnapshot,
        surface: &Surface,
        tracks: &[Track],
        stock: Option<&SurfaceState>,
    ) {
        let stance = |id: UnitId| -> Option<aim::Stance> {
            if let Some(pose) = picture.surface.iter().find(|pose| pose.id == id) {
                return Some(aim::Stance {
                    origin: pose.position,
                    heading: pose.attitude[0],
                });
            }
            surface.arsenal.arms(id).map(|arms| aim::Stance {
                origin: arms.position,
                heading: arms.heading,
            })
        };
        let arc = |id: UnitId, hardpoint: usize| {
            surface
                .arsenal
                .arms(id)
                .and_then(|arms| aim::mount_arc(arms, hardpoint))
        };
        // Nothing aims at a crashed aircraft or a wreck: a unit keeps the
        // aim it had (a body) or turns back to rest (a turret).
        let target = |id: u32| -> Option<Vector> {
            if picture.player.id == id {
                return (!picture.player.crashed).then_some(picture.player.position);
            }
            if let Some(pose) = picture.targets.iter().find(|pose| pose.id == id) {
                return (!pose.crashed).then_some(pose.position);
            }
            if let Some(pose) = picture.surface.iter().find(|pose| pose.id.0 == id) {
                return (!pose.wrecked).then_some(pose.position);
            }
            surface
                .arsenal
                .arms(UnitId(id))
                .map(|arms| arms.position)
                .or_else(|| {
                    surface
                        .unit(UnitId(id))
                        .map(|unit| unit.position.map(f64::from))
                })
        };
        // A destroyed unit no longer aims.
        let alive: Vec<Track> = tracks
            .iter()
            .filter(|track| {
                !picture
                    .targets
                    .iter()
                    .any(|pose| pose.id == track.unit.0 && pose.crashed)
            })
            .copied()
            .collect();
        self.aims
            .step(picture.tick, &self.rules, &alive, &arc, &stance, &target);
        if let Some(stock) = stock {
            for (id, aimer) in &self.aimers {
                if aimer.rails.is_empty() {
                    continue;
                }
                // A unit the stock does not list is fully loaded (a replay
                // lists only the units whose rails changed).
                let Some(state) = stock.unit(UnitId(*id)).filter(|unit| unit.armed) else {
                    self.aimer_loads.remove(id);
                    self.rail_shapes.remove(id);
                    continue;
                };
                let loads: Vec<u32> = aimer
                    .rails
                    .iter()
                    .map(|hp| state.mounts.get(usize::from(*hp)).map_or(0, |m| m.loaded))
                    .collect();
                if loads == aimer.full {
                    self.aimer_loads.remove(id);
                    self.rail_shapes.remove(id);
                } else {
                    if aimer.body_only() && self.aimer_loads.get(id) != Some(&loads) {
                        let mut words = BTreeMap::new();
                        aimer.rail_words(Some(&loads), &mut words);
                        self.rail_shapes
                            .insert(*id, Shape::with_export_state(&aimer.bytes, &words).ok());
                    }
                    self.aimer_loads.insert(*id, loads);
                }
            }
        }
    }

    /// Unit `id`'s aim now, when its picture aims.
    pub fn aim_of(&self, id: u32) -> Option<&aim::UnitAim> {
        self.aims.unit(UnitId(id))
    }

    /// Whether aimer `id` is drawn every frame now: aiming has begun and its
    /// antennas spin or a part stands turned.
    fn aiming(&self, id: u32) -> bool {
        let Some(aimer) = self.aimers.get(&id) else {
            return false;
        };
        self.aims.tick.is_some()
            && (aimer.antennas
                || !aimer.turrets.is_empty()
                || self.aims.unit(UnitId(id)).is_some_and(aim::UnitAim::turned))
    }

    /// Aimer `id`'s shape and the turn of its body now, at mission time
    /// `seconds`: projected again when a turret, antenna or rail changes the
    /// scenery look.
    fn aimed_shape(&self, id: u32, aimer: &Aimer, seconds: f64) -> (Option<Projected<'_>>, f64) {
        let aim = self.aims.unit(UnitId(id));
        let body = aim.map_or(0., |a| a.body);
        let loads = self.aimer_loads.get(&id);
        // A launcher whose body turned and whose rails emptied: its shape
        // changes only with the loads, so it was projected when they did.
        if aimer.body_only()
            && loads.is_some()
            && let Some(cached) = self.rail_shapes.get(&id)
        {
            return (cached.as_ref().map(Projected::Cached), body);
        }
        let mut state = BTreeMap::new();
        for (hardpoint, _) in &aimer.turrets {
            let angle = aim
                .and_then(|a| a.turrets.get(&usize::from(*hardpoint)))
                .copied()
                .unwrap_or(0.);
            state.insert(hardpoint_angle_word(*hardpoint), binary_angle(angle));
        }
        if aimer.antennas {
            state.insert(CURRENT_TICKS_WORD, current_ticks(seconds));
        }
        if state.is_empty() && loads.is_none() {
            return (None, body);
        }
        aimer.rail_words(loads, &mut state);
        (
            Shape::with_export_state(&aimer.bytes, &state)
                .ok()
                .map(Projected::Fresh),
            body,
        )
    }

    /// A carrier's island and deck parts at `stand` (docs/spec/surface-
    /// defenses.md, "Carriers"): intact, and as they stand by the burning
    /// hull (the island's damage branch, the tractors as they are, the deck
    /// crew gone). Each part stands on the hull's flight deck by its own
    /// ground offset (lead ruling after S2), its table offset and the deck
    /// at the hull's placed scale; the deck crew is a sprite.
    fn carrier(
        &mut self,
        build: &mut Build,
        hull_id: u32,
        carrier: &tore_formats::carrier::Carrier,
        stand: &Stand,
    ) -> AppResult<(Drawn, Drawn)> {
        let hull_bytes = build.bytes(carrier.hull)?;
        let authored = tore_formats::shape::object_scale(hull_bytes)?;
        let deck = tore_formats::carrier::flight_deck(&Shape::scenery(hull_bytes)?)
            .ok_or_else(|| format!("{}: no flight deck", carrier.hull))?;
        // Table offsets are retail feet around the hull at its scenery
        // scale; the hull stands at `stand.scale`.
        let factor = stand.scale / authored;
        let deck_feet = f64::from(deck.height) * stand.scale;
        let mut intact = Drawn::default();
        let mut damaged = Drawn::default();
        for (index, part) in carrier.parts.iter().enumerate() {
            let island = index + 1 == carrier.parts.len();
            let bytes = build.bytes(part.shape)?;
            let scale = tore_formats::shape::object_scale(bytes)? * factor;
            // The F2 ground offset is in retail feet (the island's matches
            // its lowest point at the scenery scale), so it takes the hull's
            // factor, not the part's shape scale.
            let contact =
                f64::from(tore_formats::shape::contact_offset(bytes)?.unwrap_or(0)) * factor;
            let [right, up, forward] = part.offset.map(|v| f64::from(v) * factor);
            let lift = deck_feet + up - contact;
            let turn = f64::from(part.heading) * std::f64::consts::TAU / 65536.;
            let yaw = Basis::new(turn, 0., 0.);
            // The part's axes turned by its heading, then by the hull's.
            let rotate = |v: Vector| -> Vector {
                std::array::from_fn(|i| {
                    stand.basis.right[i] * v[0]
                        + stand.basis.up[i] * v[1]
                        + stand.basis.forward[i] * v[2]
                })
            };
            let part_stand = Stand {
                scale,
                basis: Basis {
                    right: rotate(yaw.right),
                    up: rotate(yaw.up),
                    forward: rotate(yaw.forward),
                },
                origin: std::array::from_fn(|i| {
                    stand.origin[i]
                        + stand.basis.right[i] * right
                        + stand.basis.up[i] * lift
                        + stand.basis.forward[i] * forward
                }),
            };
            let shape = build.scenery(&mut self.layers, part.shape)?;
            if !shape.billboards.is_empty() {
                // The deck crew: front and back sheets, one per frame.
                let mut sheets: [Vec<Billboard>; 2] = Default::default();
                for frame in 0..CREW_FRAMES {
                    let state = BTreeMap::from([(SPRITE_FRAME_WORD, frame << 16)]);
                    let front = build.shape(&mut self.layers, part.shape, Some(&state))?;
                    let back = Shape::with_state(build.bytes(part.shape)?, &state)
                        .map_err(|e| format!("{}: {e}", part.shape))?;
                    preload(
                        &back,
                        build.resources,
                        build.first_page,
                        build.pages,
                        &mut self.layers,
                    )?;
                    sheets[0].extend(front.billboards.first().cloned());
                    sheets[1].extend(back.billboards.first().cloned());
                }
                self.sprites.push(Sprite {
                    owner: hull_id,
                    stand: part_stand,
                    sprites: shape.billboards.clone(),
                    crew: Some(sheets),
                });
                continue;
            }
            emit(&shape, &part_stand, &self.layers, &mut intact, BUDGET)?;
            let after = if island {
                build.shape(
                    &mut self.layers,
                    part.shape,
                    Some(&BTreeMap::from([(DAMAGED_WORD, 1)])),
                )?
            } else {
                shape
            };
            emit(&after, &part_stand, &self.layers, &mut damaged, BUDGET)?;
        }
        Ok((intact, damaged))
    }

    /// Prepares parked aircraft `id`'s pieces, the `_B` and `_D` shapes of
    /// its type (`shape`, `MIG21F.SH`), drawn at `scale`. A piece the import
    /// lacks is not drawn.
    pub(super) fn add_parked(&mut self, build: &mut Build, id: u32, shape: &str, scale: f64) {
        let stem = shape.trim_end_matches(".SH");
        let pieces = ["B", "D"].map(|suffix| {
            let name = format!("{stem}_{suffix}.SH");
            build
                .resources
                .contains_key(&name)
                .then(|| build.scenery(&mut self.layers, &name).ok())
                .flatten()
                .map(|shape| Model { shape, scale })
        });
        self.debris.insert(id, pieces);
    }

    /// Placement `id`'s look given whether it stands (`alive`), whether
    /// combat holds it at all (`known`), and its launcher loads in `surface`
    /// (none: a replay, which records no loads yet). `None` is its intact
    /// look.
    pub(super) fn look(
        &self,
        id: u32,
        alive: bool,
        known: bool,
        surface: Option<&SurfaceState>,
    ) -> Option<Look> {
        if !alive {
            return Some(if known && self.wrecks.contains_key(&id) {
                Look::Wreck
            } else {
                Look::Hidden
            });
        }
        if self.aiming(id) {
            return Some(Look::Aimed);
        }
        let launcher = self.launchers.get(&id)?;
        let state = surface?.unit(UnitId(id)).filter(|unit| unit.armed)?;
        let loads: Vec<u32> = launcher
            .hardpoints
            .iter()
            .map(|hp| state.mounts.get(usize::from(*hp)).map_or(0, |m| m.loaded))
            .collect();
        (loads != launcher.full)
            .then(|| Look::Rails(loads.iter().map(|n| (*n).min(255) as u8).collect()))
    }

    /// The geometry of look `look` for `id`, building a rail state the
    /// first time it is asked for; `None` when it draws nothing or cannot be
    /// built.
    pub(super) fn drawn(&mut self, id: u32, look: &Look) -> Option<&Drawn> {
        match look {
            Look::Hidden | Look::Aimed => None,
            Look::Wreck => self.wrecks.get(&id),
            Look::Rails(loads) => {
                let key = (id, loads.clone());
                if !self.rails.contains_key(&key) {
                    let launcher = self.launchers.get(&id)?;
                    let state: BTreeMap<usize, i32> = launcher
                        .hardpoints
                        .iter()
                        .zip(loads)
                        .filter(|(_, n)| **n > 0)
                        .map(|(hp, n)| (loaded_count_word(*hp), i32::from(*n)))
                        .collect();
                    let shape = Shape::with_export_state(&launcher.bytes, &state).ok()?;
                    let mut drawn = Drawn::default();
                    emit(&shape, &launcher.stand, &self.layers, &mut drawn, BUDGET).ok()?;
                    self.rails.insert(key.clone(), drawn);
                }
                self.rails.get(&key)
            }
        }
    }

    /// This frame's moving surface geometry, relative to the render origin
    /// `origin`: the routed units at their blended poses (their wreck once
    /// destroyed), the men and deck crew facing `camera` (gone with their
    /// owner), and parked aircraft pieces in flight. `standing` says whether
    /// an object stands; `seconds` is mission time, for the crew's signals.
    pub fn frame(
        &self,
        picture: &crate::snapshot::RenderSnapshot,
        standing: &dyn Fn(u32) -> bool,
        camera: &Camera,
        origin: Vector,
        seconds: f64,
    ) -> Vec<f32> {
        let mut out = Drawn::default();
        let local = |p: Vector| -> Vector { std::array::from_fn(|i| p[i] - origin[i]) };
        let eye = local(camera.position);
        for pose in &picture.surface {
            let Some(mover) = self.movers.get(&pose.id.0) else {
                continue;
            };
            let model = if pose.wrecked {
                match &mover.wreck {
                    Some(wreck) => wreck,
                    None => continue,
                }
            } else {
                &mover.intact
            };
            let [yaw, pitch, bank] = pose.attitude;
            let mut basis = Basis::new(yaw, pitch, bank);
            let at = local(pose.position);
            // A wreck lies on the ground; the unit stands on its lowest point.
            let lift = if pose.wrecked {
                mover.wreck_lift
            } else {
                mover.lift
            };
            // An aiming unit turns its body or turrets.
            let mut aimed = None;
            if !pose.wrecked
                && let Some(aimer) = self.aimers.get(&pose.id.0)
                && self.aims.tick.is_some()
            {
                let (shape, body) = self.aimed_shape(pose.id.0, aimer, seconds);
                basis = yawed(basis, body);
                aimed = shape;
            }
            let stand = Stand {
                scale: model.scale,
                basis,
                origin: std::array::from_fn(|i| at[i] + basis.up[i] * lift),
            };
            // Each frame's batch is bounded by the moving buffer, not the
            // static budget.
            let shape = aimed.as_ref().map_or(&model.shape, Projected::shape);
            let _ = emit(shape, &stand, &self.layers, &mut out, usize::MAX);
            if !pose.wrecked {
                self.sprites_of(pose.id.0, &stand, camera, eye, seconds, &mut out);
            }
        }
        // Standing units that aim, at their place with their aim.
        for (id, aimer) in &self.aimers {
            if aimer.routed || !standing(*id) || !self.aiming(*id) {
                continue;
            }
            let (shape, body) = self.aimed_shape(*id, aimer, seconds);
            let basis = yawed(aimer.stand.basis, body);
            let stand = Stand {
                scale: aimer.stand.scale,
                basis,
                origin: local(aimer.stand.origin),
            };
            let shape = shape.as_ref().map_or(&aimer.intact, Projected::shape);
            let _ = emit(shape, &stand, &self.layers, &mut out, usize::MAX);
        }
        for sprite in &self.sprites {
            if self.movers.contains_key(&sprite.owner) || !standing(sprite.owner) {
                continue;
            }
            let stand = Stand {
                origin: local(sprite.stand.origin),
                ..sprite.stand
            };
            self.draw_sprite(sprite, &stand, camera, eye, seconds, &mut out);
        }
        for piece in &picture.debris {
            let Some(pieces) = self.debris.get(&piece.owner) else {
                continue;
            };
            let Some(model) = piece
                .variant
                .and_then(|variant| pieces.get(variant))
                .and_then(Option::as_ref)
            else {
                continue;
            };
            let [yaw, pitch, bank] = piece.attitude;
            let stand = Stand {
                scale: model.scale,
                basis: Basis::new(yaw, pitch, bank),
                origin: local(piece.position),
            };
            let _ = emit(&model.shape, &stand, &self.layers, &mut out, usize::MAX);
        }
        out.vertices
    }

    /// The sprites a moving unit carries, at its pose. `eye` is the camera
    /// in the same frame as `stand`.
    fn sprites_of(
        &self,
        owner: u32,
        stand: &Stand,
        camera: &Camera,
        eye: Vector,
        seconds: f64,
        out: &mut Drawn,
    ) {
        for sprite in self.sprites.iter().filter(|s| s.owner == owner) {
            self.draw_sprite(sprite, stand, camera, eye, seconds, out);
        }
    }

    fn draw_sprite(
        &self,
        sprite: &Sprite,
        stand: &Stand,
        camera: &Camera,
        eye: Vector,
        seconds: f64,
        out: &mut Drawn,
    ) {
        let view = camera.uniform(1., [0.; 4], [0; 3]);
        let right: Vector = std::array::from_fn(|i| f64::from(view[4 + i]));
        let up: Vector = std::array::from_fn(|i| f64::from(view[8 + i]));
        let sprites: &[Billboard] = match &sprite.crew {
            Some(sheets) => {
                // The front sheet while the viewer is ahead of the crewman,
                // the back sheet behind; the signal frame cycles.
                let toward: Vector = std::array::from_fn(|i| eye[i] - stand.origin[i]);
                let ahead = (0..3)
                    .map(|i| toward[i] * stand.basis.forward[i])
                    .sum::<f64>()
                    >= 0.;
                let sheet = &sheets[usize::from(!ahead)];
                let frame = ((seconds.max(0.) * CREW_FPS) as usize) % sheet.len().max(1);
                match sheet.get(frame) {
                    Some(one) => std::slice::from_ref(one),
                    None => &sprite.sprites,
                }
            }
            None => &sprite.sprites,
        };
        let faces = sprites
            .iter()
            .map(|one| facing(one, stand, right, up))
            .collect();
        let shape = Shape {
            lines: Vec::new(),
            faces,
            billboards: Vec::new(),
            state_words: BTreeSet::new(),
        };
        let _ = emit(&shape, stand, &self.layers, out, usize::MAX);
    }
}

/// The hardpoints whose loaded count `shape` reads to draw its rails,
/// ascending; empty for a shape that draws no load.
pub(super) fn rail_hardpoints(shape: &Shape) -> Vec<u8> {
    shape
        .state_words
        .iter()
        .filter_map(|word| word.checked_sub(LOADED_COUNT_BASE))
        .filter_map(|hp| u8::try_from(hp).ok())
        .collect()
}

#[cfg(test)]
impl SurfaceArt {
    /// A wreck for `id`, as the build makes one.
    pub(super) fn with_wreck(mut self, id: u32, drawn: Drawn) -> Self {
        self.wrecks.insert(id, drawn);
        self
    }
    /// A unit with spinning antennas whose aim has been stepped once: it
    /// is drawn by the frame pass, never by the static batch.
    pub(super) fn with_spinning_antenna(mut self, id: u32) -> Self {
        self.aimers.insert(
            id,
            Aimer {
                stand: Stand {
                    scale: 1.,
                    basis: Basis::new(0., 0., 0.),
                    origin: [0.; 3],
                },
                bytes: Vec::new(),
                intact: Shape {
                    lines: Vec::new(),
                    faces: Vec::new(),
                    billboards: Vec::new(),
                    state_words: BTreeSet::new(),
                },
                turrets: Vec::new(),
                antennas: true,
                rails: Vec::new(),
                full: Vec::new(),
                routed: false,
            },
        );
        self.aims.tick = Some(0);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::shape::{Face, FogMode};
    use tore_world::surface::{MountStock, SurfacePose, SurfaceUnitState};

    fn triangle(at: f32) -> Shape {
        Shape {
            lines: Vec::new(),
            faces: vec![Face {
                fog: FogMode::Enabled,
                positions: vec![[at, 0., 0.], [at + 1., 0., 0.], [at, 1., 0.]],
                colors: vec![7; 3],
                uv: Vec::new(),
                texture: String::new(),
                subtype: 0,
                normal: None,
                address: 0,
            }],
            billboards: Vec::new(),
            state_words: BTreeSet::new(),
        }
    }

    fn level(origin: Vector) -> Stand {
        Stand {
            scale: 2.,
            basis: Basis::new(0., 0., 0.),
            origin,
        }
    }

    fn positions(vertices: &[f32]) -> Vec<[f32; 3]> {
        vertices
            .chunks_exact(FLOATS)
            .map(|v| [v[0], v[1], v[2]])
            .collect()
    }

    #[test]
    fn a_shape_is_placed_by_its_scale_basis_and_origin() {
        let mut out = Drawn::default();
        emit(
            &triangle(0.),
            &level([100., 10., -5.]),
            &Layers::new(),
            &mut out,
            BUDGET,
        )
        .unwrap();
        // Shape axes right, forward, up: the second corner is 2 ft east,
        // the third 2 ft north, at the palette colour without a texture.
        assert_eq!(
            positions(&out.vertices),
            [[100., 10., -5.], [102., 10., -5.], [100., 10., -3.]]
        );
        assert!(
            out.vertices
                .chunks_exact(FLOATS)
                .all(|v| v[5] == -1. && v[9] == 7.)
        );
        let mut small = Drawn::default();
        assert!(
            emit(
                &triangle(0.),
                &level([0.; 3]),
                &Layers::new(),
                &mut small,
                20
            )
            .is_err()
        );
    }

    #[test]
    fn a_placement_shows_its_wreck_its_rails_or_nothing() {
        let mut art = SurfaceArt::default().with_wreck(1, Drawn::default());
        art.launchers.insert(
            2,
            Launcher {
                bytes: Vec::new(),
                stand: level([0.; 3]),
                hardpoints: vec![0, 2],
                full: vec![2, 1],
            },
        );
        // Destroyed: its wreck if it keeps one, else hidden; a placement
        // combat never registered hides.
        assert_eq!(art.look(1, false, true, None), Some(Look::Wreck));
        assert_eq!(art.look(1, false, false, None), Some(Look::Hidden));
        assert_eq!(art.look(3, false, true, None), Some(Look::Hidden));
        assert_eq!(art.look(3, true, true, None), None);
        // A launcher at full load, or not armed yet, keeps its scenery look;
        // with rounds gone its rails show what is left.
        let mut unit = SurfaceUnitState::new(UnitId(2));
        let state = |unit: &SurfaceUnitState| SurfaceState::new(0, vec![unit.clone()]);
        assert_eq!(art.look(2, true, true, Some(&state(&unit))), None);
        unit.armed = true;
        unit.mounts = [2, 0, 1]
            .map(|loaded| MountStock {
                loaded,
                reserve: None,
                ordinal: 0,
            })
            .to_vec();
        assert_eq!(art.look(2, true, true, Some(&state(&unit))), None);
        unit.mounts[0].loaded = 1;
        assert_eq!(
            art.look(2, true, true, Some(&state(&unit))),
            Some(Look::Rails(vec![1, 1]))
        );
        unit.mounts[2].loaded = 0;
        assert_eq!(
            art.look(2, true, true, Some(&state(&unit))),
            Some(Look::Rails(vec![1, 0]))
        );
        // A replay records no loads: the scenery look.
        assert_eq!(art.look(2, true, true, None), None);
    }

    #[test]
    fn a_moving_unit_draws_where_it_is_on_its_lowest_point_and_as_a_wreck() {
        let mut art = SurfaceArt::default();
        art.movers.insert(
            5,
            Mover {
                intact: Model {
                    shape: triangle(0.),
                    scale: 1.,
                },
                wreck: Some(Model {
                    shape: triangle(10.),
                    scale: 1.,
                }),
                lift: 3.,
                wreck_lift: 0.,
            },
        );
        art.movers.insert(
            6,
            Mover {
                intact: Model {
                    shape: triangle(0.),
                    scale: 1.,
                },
                wreck: None,
                lift: 0.,
                wreck_lift: 0.,
            },
        );
        let pose = |id: u32, wrecked: bool| SurfacePose {
            id: UnitId(id),
            position: [1000., 50., 2000.],
            attitude: [0.; 3],
            shape: None,
            wrecked,
        };
        let camera = Camera::new();
        let draw = |poses: Vec<SurfacePose>| {
            let picture = crate::snapshot::RenderSnapshot {
                surface: poses,
                ..Default::default()
            };
            positions(&art.frame(&picture, &|_| true, &camera, [1000., 0., 2000.], 0.))
        };
        // Relative to the render origin, lifted onto its lowest point.
        assert_eq!(draw(vec![pose(5, false)])[0], [0., 53., 0.]);
        // Destroyed: its wreck, lying on the ground; with none, nothing.
        assert_eq!(draw(vec![pose(5, true)])[0], [10., 50., 0.]);
        assert!(draw(vec![pose(6, true)]).is_empty());
        assert_eq!(draw(vec![pose(6, false)]).len(), 3);
    }

    /// docs/spec/surface-defenses.md, "Aiming": a land unit with no turret
    /// turns as a whole toward what its weapon holds, leaving the static
    /// batch for the frame's, and keeps its aim once nothing is held.
    #[test]
    fn an_aiming_unit_turns_its_body_and_is_drawn_every_frame() {
        use tore_world::surface::aim::{Rule, Track, Weapon};
        let mut art = SurfaceArt::default();
        art.aimers.insert(
            7,
            Aimer {
                stand: level([1000., 50., 2000.]),
                bytes: Vec::new(),
                intact: triangle(0.),
                turrets: Vec::new(),
                antennas: false,
                rails: Vec::new(),
                full: Vec::new(),
                routed: false,
            },
        );
        art.rules.insert(UnitId(7), Rule::Body { rate_deg_s: 90. });
        let mut surface = Surface::default();
        surface.arsenal.units.push(tore_world::surface::fire::Arms {
            unit: UnitId(7),
            position: [1000., 50., 2000.],
            heading: 0.,
            skill: 1,
            side: tore_sim::combat::live::Side(2),
            react: 0,
            search_limit: None,
            ship: false,
            weapons: Vec::new(),
            radar: None,
            battery: None,
            loads: Vec::new(),
            npc: [0; 4],
        });
        let camera = Camera::new();
        // A jet due east of it.
        let picture = |tick: u64| crate::snapshot::RenderSnapshot {
            tick,
            targets: vec![crate::snapshot::AircraftPose {
                id: 3,
                position: [9000., 3000., 2000.],
                ..Default::default()
            }],
            ..Default::default()
        };
        let held = [Track {
            unit: UnitId(7),
            weapon: Weapon::Station(0),
            target: 3,
        }];
        let frame = |art: &SurfaceArt, standing: bool| {
            positions(&art.frame(&picture(0), &|_| standing, &camera, [1000., 0., 2000.], 0.))
        };
        // Before aiming begins, and at rest, it stays in the static batch.
        assert_eq!(art.look(7, true, true, None), None);
        art.step_aim(&picture(0), &surface, &[], None);
        assert_eq!(art.look(7, true, true, None), None);
        assert!(frame(&art, true).is_empty());
        // Held: a quarter turn right at 90 degrees a second takes a second.
        for tick in 1..=60 {
            art.step_aim(&picture(tick), &surface, &held, None);
        }
        assert!((art.aim_of(7).unwrap().body.to_degrees() - 45.).abs() < 1e-6);
        for tick in 61..=200 {
            art.step_aim(&picture(tick), &surface, &held, None);
        }
        assert_eq!(art.look(7, true, true, None), Some(Look::Aimed));
        assert_eq!(art.drawn(7, &Look::Aimed).map(|d| d.vertices.len()), None);
        // Its nose (shape forward) now points east, its right side south.
        let turned = frame(&art, true);
        let close = |a: [f32; 3], b: [f32; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-4);
        assert!(close(turned[0], [0., 50., 0.]), "{turned:?}");
        assert!(close(turned[1], [0., 50., -2.]), "{turned:?}");
        assert!(close(turned[2], [2., 50., 0.]), "{turned:?}");
        // Nothing held: it keeps that aim. Destroyed: it is not drawn.
        art.step_aim(&picture(201), &surface, &[], None);
        assert_eq!(art.look(7, true, true, None), Some(Look::Aimed));
        assert!(frame(&art, false).is_empty());
        assert_eq!(art.look(7, false, true, None), Some(Look::Hidden));
    }

    /// A launcher whose body turned and whose rails are not full is
    /// projected once when its loads change, not on every frame.
    #[test]
    fn a_turned_launchers_rails_are_projected_when_the_loads_change() {
        let mut art = SurfaceArt::default();
        art.aimers.insert(
            4,
            Aimer {
                stand: level([0.; 3]),
                bytes: Vec::new(),
                intact: triangle(0.),
                turrets: Vec::new(),
                antennas: false,
                rails: vec![0],
                full: vec![2],
                routed: false,
            },
        );
        let surface = Surface::default();
        let picture = crate::snapshot::RenderSnapshot::default();
        let mut unit = SurfaceUnitState::new(UnitId(4));
        unit.armed = true;
        let mut stock = |loaded: u32| {
            unit.mounts = vec![MountStock {
                loaded,
                reserve: None,
                ordinal: 0,
            }];
            SurfaceState::new(0, vec![unit.clone()])
        };
        // Full: nothing is projected or kept.
        art.step_aim(&picture, &surface, &[], Some(&stock(2)));
        assert!(art.rail_shapes.is_empty() && art.aimer_loads.is_empty());
        // One round gone: projected once and kept (here the empty test
        // bytes cannot project, which the entry records as the intact look).
        art.step_aim(&picture, &surface, &[], Some(&stock(1)));
        assert_eq!(art.aimer_loads.get(&4), Some(&vec![1]));
        assert!(art.rail_shapes.contains_key(&4));
        art.rail_shapes.insert(4, Some(triangle(5.)));
        // The same loads on the next step keep the entry as it was.
        art.step_aim(&picture, &surface, &[], Some(&stock(1)));
        assert!(art.rail_shapes[&4].is_some());
        let aimer = &art.aimers[&4];
        let (shape, _) = art.aimed_shape(4, aimer, 0.);
        assert!(matches!(shape, Some(Projected::Cached(_))));
        // New loads project again; a reload drops the entry.
        art.step_aim(&picture, &surface, &[], Some(&stock(0)));
        assert!(art.rail_shapes[&4].is_none());
        art.step_aim(&picture, &surface, &[], Some(&stock(2)));
        assert!(art.rail_shapes.is_empty());
    }

    #[test]
    fn a_man_faces_the_viewer_and_goes_with_his_owner() {
        let sprite = Billboard {
            center: [0., 0., 6.],
            size: [7., 12.],
            texture: String::new(),
            uv: None,
            fog: FogMode::Enabled,
            address: 0,
        };
        let mut art = SurfaceArt::default();
        art.sprites.push(Sprite {
            owner: 9,
            stand: Stand {
                scale: 1. / 3.,
                basis: Basis::new(1.2, 0., 0.),
                origin: [0.; 3],
            },
            sprites: vec![sprite],
            crew: None,
        });
        let mut camera = Camera::new();
        camera.position = [0., 2., -100.];
        camera.yaw = 0.;
        camera.pitch = 0.;
        let picture = crate::snapshot::RenderSnapshot::default();
        let quad = positions(&art.frame(&picture, &|_| true, &camera, [0.; 3], 0.));
        assert_eq!(quad.len(), 6);
        // Seen from the south, whatever its heading: the quad lies across
        // the view (constant z), 4 ft tall from its feet, 7/3 ft wide.
        assert!(quad.iter().all(|p| p[2].abs() < 1e-4), "{quad:?}");
        let ys: Vec<f32> = quad.iter().map(|p| p[1]).collect();
        let xs: Vec<f32> = quad.iter().map(|p| p[0]).collect();
        let span = |v: &[f32]| {
            v.iter().cloned().fold(f32::MIN, f32::max) - v.iter().cloned().fold(f32::MAX, f32::min)
        };
        assert!((span(&ys) - 4.).abs() < 1e-4 && (span(&xs) - 7. / 3.).abs() < 1e-4);
        assert!(
            art.frame(&picture, &|id| id != 9, &camera, [0.; 3], 0.)
                .is_empty()
        );
    }

    #[test]
    fn a_parked_aircraft_piece_draws_with_its_own_shape() {
        let mut art = SurfaceArt::default();
        let model = |at| Model {
            shape: triangle(at),
            scale: 1.,
        };
        art.debris
            .insert(0x5000_0008, [Some(model(0.)), Some(model(20.))]);
        let piece = |variant| crate::snapshot::DebrisPose {
            owner: 0x5000_0008,
            draw: crate::snapshot::Draw::Hidden,
            position: [10., 5., 10.],
            attitude: [0.; 3],
            variant,
        };
        let draw = |pieces: Vec<crate::snapshot::DebrisPose>| {
            let picture = crate::snapshot::RenderSnapshot {
                debris: pieces,
                ..Default::default()
            };
            positions(&art.frame(&picture, &|_| true, &Camera::new(), [0.; 3], 0.))
        };
        // Piece 0 is the `_B` shape, piece 1 the `_D`; no piece, nothing.
        assert_eq!(draw(vec![piece(Some(0))])[0], [10., 5., 10.]);
        assert_eq!(draw(vec![piece(Some(1))])[0], [30., 5., 10.]);
        assert!(draw(vec![piece(None)]).is_empty());
    }
}
