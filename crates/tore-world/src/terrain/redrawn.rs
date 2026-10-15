//! Redrawn airports (AP1, 2026-10-10; the default since 2026-10-11): they
//! replace the retail airfield shapes with airports drawn at real-world size
//! from small data files we own, textured with retail airport art. On unless
//! [`super::Overrides::redrawn_airports`] is off (single player's developer
//! escape hatch `TORE_REDRAWN_AIRPORTS=0`, or a replay of a recording from
//! before the redraw). See docs/formats/redrawn-airports.md.
//!
//! One plan per runway shape type (`crates/tore-world/airports/*.toml`)
//! gives its runways, taxiways, aprons, building lines and the AI's points
//! in feet in the **runway frame** of that type: `x` to the right of the
//! retail runway's centreline, `z` along it from its near threshold (STRIP
//! anchor 0x11). Every placement of the type shares that frame, so one plan
//! redraws standalone fields without moving their runway line. A paired
//! field now uses the single long-runway plan in `merge`; its former second
//! tile is an identity alias. Pair plans remain for legacy replay scenes.
//!
//! What a redrawn airport replaces, for its STRIP placements: the drawn mesh
//! ([`Built::patches`], drawn by the app), the landable and contact box
//! ([`Built::surface`], the whole redrawn field, grass included), the AI
//! anchors ([`Built::anchors`]) and the positions of the airport's
//! buildings, plus extra hangars and vehicles. Standalone fields retain
//! their source approach lines; merged pairs use their new plan's line.
pub mod audit;
mod buildings;
mod data;
pub mod geometry;
pub mod layout;
pub mod lights;
mod merge;
pub mod plan;

use super::{Placements, runway_length_ft};
use crate::WorldResult;
use geometry::Point;
pub use layout::{Cell, Grid, Patch};
pub use plan::{Along, Material, Paint, Plan};
use tore_formats::mission::Placement;
use tore_sim::{ai::airfield::AirfieldAnchors, airport::OrientedBox};

pub use buildings::ADDED_ORDINAL_BASE;

/// The shared default materials: the dark asphalt set, used wherever a
/// plan's own atlas lacks a piece.
const DEFAULTS: &str = include_str!("../../airports/defaults.toml");

/// The plans the build ships, one file per runway type and per pair.
const BUILTIN: [(&str, &str); 17] = [
    ("strip.toml", include_str!("../../airports/strip.toml")),
    ("strip1.toml", include_str!("../../airports/strip1.toml")),
    ("strip2.toml", include_str!("../../airports/strip2.toml")),
    ("strip3.toml", include_str!("../../airports/strip3.toml")),
    ("strip4.toml", include_str!("../../airports/strip4.toml")),
    ("strip5.toml", include_str!("../../airports/strip5.toml")),
    ("strip6.toml", include_str!("../../airports/strip6.toml")),
    ("strip7.toml", include_str!("../../airports/strip7.toml")),
    ("strip3a.toml", include_str!("../../airports/strip3a.toml")),
    ("strip5a.toml", include_str!("../../airports/strip5a.toml")),
    ("strip6a.toml", include_str!("../../airports/strip6a.toml")),
    ("strip7a.toml", include_str!("../../airports/strip7a.toml")),
    ("dtstrp.toml", include_str!("../../airports/dtstrp.toml")),
    ("pair3.toml", include_str!("../../airports/pair3.toml")),
    ("pair5.toml", include_str!("../../airports/pair5.toml")),
    ("pair6.toml", include_str!("../../airports/pair6.toml")),
    ("pair7.toml", include_str!("../../airports/pair7.toml")),
];

/// A base and its second tile are one airfield when the tile stands within
/// this distance of the base at the same heading (measured offsets 8,900 to
/// 9,700 ft in the 16 base layouts).
const PAIR_RANGE_FT: f64 = 14_000.;

/// The plans to apply.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plans {
    pub plans: Vec<Plan>,
    /// Keep the runway pieces a Quick Mission ground target template brings
    /// (`~QUSTRIP`, `~QFFACT`, ...) retail, as the redraw did before B14
    /// (2026-10-12): only a replay of a recording flown on airfield scene 1
    /// sets it.
    pub retail_templates: bool,
    /// Keep the pre-merge layout when rebuilding an old recording.
    pub legacy_pairs: bool,
}

impl Plans {
    /// The built-in plans when `enabled`, else none.
    pub fn new(enabled: bool) -> WorldResult<Self> {
        if !enabled {
            return Ok(Self::default());
        }
        Ok(Self {
            plans: Self::builtin()?,
            retail_templates: false,
            legacy_pairs: false,
        })
    }

    pub fn builtin() -> Result<Vec<Plan>, String> {
        let defaults =
            plan::default_materials(DEFAULTS).map_err(|e| format!("defaults.toml: {e}"))?;
        BUILTIN
            .iter()
            .map(|(name, text)| Plan::parse(text, &defaults).map_err(|e| format!("{name}: {e}")))
            .collect()
    }

    fn single(&self, object_type: &str) -> Option<&Plan> {
        self.plans
            .iter()
            .find(|p| p.pair.is_none() && p.applies_to.eq_ignore_ascii_case(object_type))
    }

    fn pair(&self, base: &str) -> Option<&Plan> {
        self.plans.iter().find(|p| {
            p.pair
                .as_ref()
                .is_some_and(|pair| pair.base.eq_ignore_ascii_case(base))
        })
    }

    pub fn is_empty(&self) -> bool {
        self.plans.is_empty()
    }
}

/// The world placement of a runway frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub origin: [f64; 3],
    pub right: [f64; 3],
    pub forward: [f64; 3],
}

impl Frame {
    pub fn world(&self, p: Point) -> [f64; 3] {
        std::array::from_fn(|axis| {
            self.origin[axis] + self.right[axis] * p[0] + self.forward[axis] * p[1]
        })
    }
    /// The horizontal world direction `[x, z]` of a frame direction.
    pub fn world_vector(&self, v: Point) -> [f64; 2] {
        [
            self.right[0] * v[0] + self.forward[0] * v[1],
            self.right[2] * v[0] + self.forward[2] * v[1],
        ]
    }
    /// The frame direction of a horizontal world direction `[x, z]`.
    pub fn local_vector(&self, w: [f64; 2]) -> Point {
        [
            w[0] * self.right[0] + w[1] * self.right[2],
            w[0] * self.forward[0] + w[1] * self.forward[2],
        ]
    }
    pub fn local(&self, p: [f64; 3]) -> Point {
        self.local_vector([p[0] - self.origin[0], p[2] - self.origin[2]])
    }
}

/// A plan applied to one STRIP placement: what the scene, the AI and the
/// scenery read in place of the retail airfield.
#[derive(Clone, Debug, PartialEq)]
pub struct Built {
    /// The plan (or `BASE+TILE` for a pair) that drew it.
    pub plan: String,
    /// The STRIP placement's object id: the runway it redraws.
    pub strip_id: u32,
    /// Former second runway identity, redirected to this field.
    pub merged_from: Option<u32>,
    /// Original tile centres, retained for terrain-art replacement.
    pub replaced_centres: Vec<[f64; 3]>,
    pub frame: Frame,
    pub materials: Vec<Material>,
    /// The paved pieces; empty for a pair's second tile, whose pavement its
    /// base draws.
    pub patches: Vec<Patch>,
    /// The landable and contact box: the whole redrawn airfield.
    pub surface: OrientedBox,
    /// The AI's points; `None` leaves the runway without anchors.
    pub anchors: Option<AirfieldAnchors>,
    /// The airport's lights, drawn by the app; empty for a pair's tile.
    pub lights: Vec<lights::Light>,
    /// The runways it draws, in its frame; empty for a pair's tile.
    pub runways: Vec<DrawnRunway>,
    /// Layout placements moved: index in the layout and the new placement.
    moved: Vec<(usize, Placement)>,
    /// Template structures moved with a merged field, keyed by stable unit ID.
    moved_surface: Vec<(u32, Placement)>,
    /// Placements added after the layout's own.
    added: Vec<Placement>,
    /// Parked template aircraft moved off the grass onto the aprons.
    pub snapped: usize,
    /// For a second tile (paired or alone): its runway object's approach
    /// line, moved onto the tile's main paved runway. A tile's STRIP anchors
    /// are copies of its base's, so its retail line lies beside its
    /// pavement; `None` keeps the retail line (every base and pad).
    pub runway_line: Option<RunwayLine>,
    /// For a pair's pavement owner: the indices in `runways` of the base
    /// runway and the tile runway that continue each other (`[pair]
    /// continues`), which the audit checks run as one straight runway.
    pub continues: Option<[usize; 2]>,
}

/// A drawn runway in its airport's frame: the plan's, moved by the tile's
/// offset in a pair.
#[derive(Clone, Debug, PartialEq)]
pub struct DrawnRunway {
    /// The plan's runway name, with the tile type for a pair's tile
    /// (`STRIP5A.OT R0`).
    pub name: String,
    pub from: Point,
    /// Degrees against the frame's heading.
    pub heading: f64,
    pub length: f64,
    pub width: f64,
    pub pad: f64,
}

/// A runway object's approach line in the world: its centre on the runway
/// plane, its heading (radians, the scene's convention) and its length.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RunwayLine {
    pub center: [f64; 3],
    pub heading: f64,
    pub length: f64,
}

impl RunwayLine {
    /// A plan's main runway: the longest (the first of equals).
    fn main_runway(plan: &Plan) -> Option<&plan::RunwaySpec> {
        plan.runways
            .iter()
            .reduce(|best, r| if r.length > best.length { r } else { best })
    }

    /// The line from `near` to `far` (frame points) in `frame`, whose world
    /// heading is `heading`; `relative` is the line's heading against the
    /// frame, degrees.
    fn between(frame: &Frame, heading: f64, relative: f64, near: Point, far: Point) -> Self {
        Self {
            center: frame.world(geometry::scale(geometry::add(near, far), 0.5)),
            heading: (heading + relative.to_radians()).rem_euclid(std::f64::consts::TAU),
            length: geometry::length(geometry::sub(far, near)),
        }
    }

    /// The main runway of a second tile's plan drawn at `offset` in
    /// `frame`, whose heading is `heading`.
    fn main(plan: &Plan, frame: &Frame, heading: f64, offset: Point) -> Option<Self> {
        let runway = Self::main_runway(plan)?;
        let near = geometry::add(runway.from, offset);
        let d = geometry::direction(runway.heading);
        let far = geometry::add(near, geometry::scale(d, runway.length));
        Some(Self::between(frame, heading, runway.heading, near, far))
    }

    /// A pair whose tile's main runway continues a base runway as one
    /// (`joined`): from the tile runway's near threshold to the base
    /// runway's far threshold, along the base runway (the tile is aligned
    /// with it, [`align`]).
    fn joined(
        base: &Plan,
        tile: &Plan,
        names: &[String; 2],
        frame: &Frame,
        heading: f64,
        offset: Point,
    ) -> Option<Self> {
        let b = base.runways.iter().find(|r| r.name == names[0])?;
        let t = tile.runways.iter().find(|r| r.name == names[1])?;
        let d = geometry::direction(b.heading);
        let far = geometry::add(b.from, geometry::scale(d, b.length));
        let near = geometry::add(t.from, offset);
        // On the base runway's centre line, from the tile's threshold.
        let along = geometry::dot(geometry::sub(near, b.from), d);
        let near = geometry::add(b.from, geometry::scale(d, along));
        Some(Self::between(frame, heading, b.heading, near, far))
    }
}

/// The tile's offset in its base's frame, moved across the base runway the
/// tile's main runway continues so the two centre lines are one line
/// (`[pair] continues`). Layouts place the tile up to about 65 ft off that
/// line; the move is only across it, so the link taxiways still meet.
fn align(offset: Point, base: &Plan, tile: &Plan, names: &[String; 2]) -> Point {
    let (Some(b), Some(t)) = (
        base.runways.iter().find(|r| r.name == names[0]),
        tile.runways.iter().find(|r| r.name == names[1]),
    ) else {
        return offset;
    };
    let side = geometry::right_of(geometry::direction(b.heading));
    let off = geometry::dot(geometry::sub(geometry::add(t.from, offset), b.from), side);
    geometry::sub(offset, geometry::scale(side, off))
}

/// A plan's AI points drawn at `offset` in `frame` (world heading
/// `heading`): a base takes off and lands along the frame (the retail
/// runway), a second tile along its main runway.
fn plan_anchors(
    plan: &Plan,
    offset: Point,
    frame: &Frame,
    heading: f64,
) -> Option<AirfieldAnchors> {
    let a = plan.anchors.as_ref()?;
    let relative = if plan.ils_runway {
        0.
    } else {
        RunwayLine::main_runway(plan).map_or(0., |r| r.heading)
    };
    let runway = (heading + relative.to_radians()).rem_euclid(std::f64::consts::TAU);
    let at = |p: Point| frame.world(geometry::add(p, offset));
    Some(AirfieldAnchors {
        taxi_out: a.taxi_out.map(at),
        takeoff_spot: at(a.takeoff),
        takeoff_heading: runway,
        landing_point: at(a.landing),
        landing_heading: runway,
        taxi_in: a.taxi_in.map(at),
        parking: a.parking.map(at),
        parking_heading: (heading + a.parking_heading.to_radians())
            .rem_euclid(std::f64::consts::TAU),
    })
}

/// The indices in `drawn` of the base runway and the tile runway a pair's
/// `continues` joins.
fn continued_runways(
    drawn: &[DrawnRunway],
    tile: Option<&Plan>,
    links: Option<&Plan>,
) -> Option<[usize; 2]> {
    let names = links?.pair.as_ref()?.continues.as_ref()?;
    let tile_name = format!("{} {}", tile?.applies_to, names[1]);
    let base_at = drawn.iter().position(|r| r.name == names[0])?;
    let tile_at = drawn.iter().position(|r| r.name == tile_name)?;
    Some([base_at, tile_at])
}

/// A copy of a lone second tile's plan with every runway end marked: no
/// base runs on from it, so its main runway gets thresholds at both ends.
fn marked_alone(plan: &Plan) -> Plan {
    let mut plan = plan.clone();
    for runway in &mut plan.runways {
        runway.marked = [true, true];
    }
    plan
}

impl Built {
    /// The redrawn airport that replaces runway object `id`, if any.
    pub fn for_runway(built: &[Built], id: u32) -> Option<&Built> {
        built.iter().find(|b| b.strip_id == id)
    }
    /// How many retail buildings it moved and how many it added.
    pub fn building_counts(&self) -> (usize, usize) {
        (
            self.moved.len() + self.moved_surface.len(),
            self.added.len(),
        )
    }
    /// Whether world `x, z` lies on this airport's drawn pavement. A pair's
    /// second tile has none of its own: its base draws it.
    pub fn on_pavement(&self, x: f64, z: f64) -> bool {
        if self.patches.is_empty() || !self.surface.contains_horizontal(x, z) {
            return false;
        }
        let p = self.frame.local([x, 0., z]);
        self.patches
            .iter()
            .any(|patch| geometry::contains(&patch.poly, p))
    }
}

/// Whether world `x, z` lies on the drawn pavement of any redrawn airport.
pub fn on_pavement(built: &[Built], x: f64, z: f64) -> bool {
    built.iter().any(|b| b.on_pavement(x, z))
}

/// A STRIP placement of the layout or of the ground target's template, with
/// what the plans need of it.
struct StripPlacement {
    /// Its index among the airfields' strips (the building rule's zones).
    index: usize,
    /// Its runway object id.
    id: u32,
    /// The placement, for the buildings the plan adds.
    placement: Placement,
    object_type: String,
    angle: i32,
    stance: super::Stance,
    frame: Frame,
    length: f64,
}

/// Applies the plans to `sources`: moves and adds the buildings and returns
/// what the scene needs. `height` is the terrain's ground height at a world
/// `x, z`.
pub(super) fn apply(
    resources: &dyn crate::resources::ResourceSource,
    sources: &mut Placements,
    plans: &Plans,
    parked: &mut [crate::surface::ParkedAircraft],
    height: impl Fn(f64, f64) -> f64,
    terrain: Option<&super::Terrain>,
) -> WorldResult<Vec<Built>> {
    if plans.is_empty() {
        return Ok(Vec::new());
    }
    // The layout's strips, then the template's (B14: a Quick Mission
    // airfield target's own runway piece is redrawn like any other).
    let mut candidates = Vec::new();
    for placement in &sources.layout.placements {
        let id = crate::surface::UnitId::layout(placement.key.ordinal)
            .ok_or("airport object ID overflow")?
            .0;
        candidates.push((id, placement));
    }
    if !plans.retail_templates {
        candidates.extend(
            sources
                .surface
                .iter()
                .map(|(id, placement)| (*id, placement)),
        );
    }
    let mut strips = Vec::new();
    for (id, placement) in candidates {
        let is_strip = sources
            .definitions
            .get(&placement.object_type)
            .is_some_and(|d| d.callbacks.iter().any(|c| c == "_STRIPProc"));
        if !is_strip {
            continue;
        }
        let ground = height(
            f64::from(placement.position[0]),
            f64::from(placement.position[2]),
        );
        let (Some(stance), Some(anchor)) = (
            sources.stance(placement, ground),
            sources.runway_anchors.get(&placement.object_type).copied(),
        ) else {
            continue;
        };
        let basis = stance.basis;
        let frame = Frame {
            origin: std::array::from_fn(|axis| {
                stance.support_origin[axis]
                    + basis.right[axis] * anchor[0]
                    + basis.forward[axis] * anchor[2]
            }),
            right: basis.right,
            forward: basis.forward,
        };
        let length = runway_length_ft(stance.min[2], stance.max[2], Some(anchor[2]));
        strips.push(StripPlacement {
            index: strips.len(),
            id,
            placement: placement.clone(),
            object_type: placement.object_type.to_ascii_uppercase(),
            angle: placement.angles[0],
            stance,
            frame,
            length,
        });
    }
    let zones: Vec<buildings::Strip> = strips
        .iter()
        .map(|s| buildings::Strip {
            index: s.index,
            origin: s.stance.support_origin,
            center: s.stance.center,
            half: s.stance.half,
            right: s.stance.basis.right,
            forward: s.stance.basis.forward,
        })
        .collect();
    // Pairs: each base with a pair plan takes its nearest unpaired tile.
    let mut tile_of = std::collections::BTreeMap::new();
    let mut paired = std::collections::BTreeSet::new();
    for (b, base) in strips.iter().enumerate() {
        let Some(pair) = plans.pair(&base.object_type).and_then(|p| p.pair.as_ref()) else {
            continue;
        };
        let distance = |s: &StripPlacement| {
            (s.stance.support_origin[0] - base.stance.support_origin[0])
                .hypot(s.stance.support_origin[2] - base.stance.support_origin[2])
        };
        let tile = strips
            .iter()
            .enumerate()
            .filter(|(t, s)| {
                s.object_type.eq_ignore_ascii_case(&pair.tile)
                    && (s.angle - base.angle).rem_euclid(360) == 0
                    && !paired.contains(t)
                    && distance(s) <= PAIR_RANGE_FT
            })
            .min_by(|a, b| distance(a.1).total_cmp(&distance(b.1)).then(a.0.cmp(&b.0)));
        if let Some((t, _)) = tile {
            paired.insert(t);
            tile_of.insert(b, t);
        }
    }
    let mut out = Vec::new();
    let mut taken = std::collections::BTreeSet::new();
    let mut added_count = 0u32;
    for (b, base) in strips.iter().enumerate() {
        if paired.contains(&b) {
            continue;
        }
        // A plan applies only to the runway it was drawn for: a shape whose
        // runway length differs from the plan's by more than a foot (an
        // import that is not the one the plans were drawn from, or a
        // synthetic test map) keeps its retail airfield, and so does its
        // second tile. Being the default, the redraw never refuses a
        // mission; every game of a session has the same import (the content
        // check), so they all fall back alike.
        let fits = |plan: &Plan, strip: &StripPlacement| {
            plan.runway_length_ft
                .is_none_or(|expected| (strip.length - expected).abs() <= 1.)
        };
        let Some(base_plan) = plans.single(&base.object_type).filter(|p| fits(p, base)) else {
            continue;
        };
        let tile = tile_of.get(&b).map(|t| &strips[*t]);
        let tile_plan = match tile {
            Some(tile) => match plans.single(&tile.object_type).filter(|p| fits(p, tile)) {
                Some(plan) => Some(plan),
                None => continue,
            },
            None => None,
        };
        if tile.is_some() && !plans.legacy_pairs {
            let built = merge::build(
                resources,
                sources,
                base,
                tile.expect("pair"),
                &zones,
                parked,
                &mut taken,
                &mut added_count,
                &height,
                terrain,
            )?;
            apply_edits(std::slice::from_ref(&built), sources);
            out.push(built);
            continue;
        }
        let frame = base.frame;
        // A tile alone marks both ends of its runways (slice CL-AP).
        let alone = (!base_plan.ils_runway && tile.is_none()).then(|| marked_alone(base_plan));
        let mut parts = vec![(alone.as_ref().unwrap_or(base_plan), [0., 0.])];
        let mut links = None;
        let mut name = base.object_type.clone();
        if let (Some(tile), Some(tile_plan)) = (tile, tile_plan) {
            links = plans.pair(&base.object_type);
            let mut offset = frame.local(tile.frame.origin);
            if let Some(names) = links.and_then(|l| l.pair.as_ref()?.continues.as_ref()) {
                offset = align(offset, base_plan, tile_plan, names);
            }
            parts.push((tile_plan, offset));
            name = format!("{}+{}", base.object_type, tile.object_type);
        }
        let (materials, patches) = layout::compose(&parts, links, f64::from(base.angle));
        let drawn_runways: Vec<DrawnRunway> = parts
            .iter()
            .enumerate()
            .flat_map(|(k, (plan, offset))| {
                plan.runways.iter().map(move |r| DrawnRunway {
                    name: if k == 0 {
                        r.name.clone()
                    } else {
                        format!("{} {}", plan.applies_to, r.name)
                    },
                    from: geometry::add(r.from, *offset),
                    heading: r.heading,
                    length: r.length,
                    width: r.width,
                    pad: r.pad,
                })
            })
            .collect();
        let continued = continued_runways(&drawn_runways, tile_plan, links);
        let airport_lights = lights::lights(&parts, links, &patches, &materials, &frame, &height);
        let heading = base.stance.heading;
        let anchors = plan_anchors(base_plan, [0., 0.], &frame, heading);
        let tile_anchors =
            tile_plan.and_then(|plan| plan_anchors(plan, parts[1].1, &frame, heading));
        let anchor_points: Vec<Point> = anchors
            .iter()
            .chain(&tile_anchors)
            .flat_map(|a| {
                a.taxi_out
                    .iter()
                    .chain(&a.taxi_in)
                    .chain(&a.parking)
                    .chain([&a.takeoff_spot, &a.landing_point])
                    .map(|p| frame.local(*p))
                    .collect::<Vec<_>>()
            })
            .collect();
        let (mut min, mut max) = geometry::bounds(patches.iter().map(|p| &p.poly))
            .ok_or_else(|| format!("redrawn airport: {name} has no pavement"))?;
        for p in &anchor_points {
            for axis in 0..2 {
                min[axis] = min[axis].min(p[axis]);
                max[axis] = max[axis].max(p[axis]);
            }
        }
        let m = base_plan.grass_margin_ft;
        let centre = frame.world([(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5]);
        let surface = OrientedBox {
            center: [centre[0], base.stance.support_origin[1], centre[2]],
            half: [(max[0] - min[0]) * 0.5 + m, 1., (max[1] - min[1]) * 0.5 + m],
            heading: base.stance.heading,
            pitch: base.stance.pitch,
            bank: base.stance.bank,
        };
        // A tile alone flies from its own main runway.
        let runway_line = (!base_plan.ils_runway)
            .then(|| RunwayLine::main(base_plan, &frame, heading, [0., 0.]))
            .flatten();
        let mut own = vec![base.index];
        let mut lines = base_plan.lines.clone();
        let mut hangars = base_plan.extras.hangars;
        if let Some(tile) = tile {
            own.push(tile.index);
            let (tile_plan, offset) = parts[1];
            lines.extend(tile_plan.lines.iter().map(|l| plan::LineSpec {
                from: geometry::add(l.from, offset),
                to: geometry::add(l.to, offset),
                out: l.out,
            }));
            hangars += tile_plan.extras.hangars;
        }
        let relaid = buildings::relay(
            resources,
            sources,
            &frame,
            &own,
            &zones,
            &lines,
            &plan::Extras {
                hangars,
                vehicles: base_plan.extras.vehicles,
            },
            anchors.map(|a| (a.parking, a.parking_heading)),
            &base.placement,
            &mut taken,
            &mut added_count,
        );
        // Template aircraft the retail field stood on grass go to the aprons.
        let mut aprons = Vec::new();
        for (plan, offset) in &parts {
            for apron in &plan.aprons {
                aprons.push((
                    geometry::add(apron.min, *offset),
                    geometry::add(apron.max, *offset),
                ));
            }
        }
        let slots: Vec<Point> = anchor_points.clone();
        let spots = buildings::apron_spots(&aprons, &slots);
        let on_pavement = |p: Point| {
            patches
                .iter()
                .any(|patch| geometry::contains(&patch.poly, p))
        };
        let snapped = buildings::snap_parked(parked, &frame, &own, &zones, &spots, &on_pavement);
        let base_id = base.id;
        let tile_id = tile.map(|t| t.id);
        let built = Built {
            plan: name.clone(),
            strip_id: base_id,
            merged_from: None,
            replaced_centres: Vec::new(),
            frame,
            materials,
            patches,
            surface,
            anchors,
            lights: airport_lights,
            runways: drawn_runways,
            moved: relaid.moved,
            moved_surface: Vec::new(),
            added: relaid.added,
            snapped,
            runway_line,
            continues: continued,
        };
        apply_edits(std::slice::from_ref(&built), sources);
        if let (Some(tile_id), Some(tile_plan)) = (tile_id, tile_plan) {
            // The tile's runway object lands and departs on the tile's own
            // main runway inside the pair's box (with `joined`, the tile
            // runway and the base runway it continues as one), with the
            // tile plan's own taxi and parking points (slice CL-AP).
            let offset = parts[1].1;
            let pair = links.and_then(|l| l.pair.as_ref());
            let line = match pair.and_then(|p| p.joined.then_some(p.continues.as_ref()).flatten()) {
                Some(names) => {
                    RunwayLine::joined(base_plan, tile_plan, names, &frame, heading, offset)
                }
                None => RunwayLine::main(tile_plan, &frame, heading, offset),
            };
            out.push(Built {
                plan: name,
                strip_id: tile_id,
                merged_from: None,
                replaced_centres: Vec::new(),
                frame,
                materials: Vec::new(),
                patches: Vec::new(),
                surface,
                anchors: tile_anchors,
                lights: Vec::new(),
                runways: Vec::new(),
                moved: Vec::new(),
                moved_surface: Vec::new(),
                added: Vec::new(),
                snapped: 0,
                runway_line: line,
                continues: None,
            });
        }
        out.push(built);
    }
    Ok(out)
}

fn apply_edits(built: &[Built], sources: &mut Placements) {
    for airport in built {
        for (id, placement) in &airport.moved_surface {
            if let Some((_, old)) = sources.surface.iter_mut().find(|(old, _)| old == id) {
                *old = placement.clone();
            }
        }
        for (index, placement) in &airport.moved {
            sources.layout.placements[*index] = placement.clone();
        }
        sources
            .layout
            .placements
            .extend(airport.added.iter().cloned());
    }
}

/// Makes `sources` (a fresh load of the same layout) match the built
/// airports' building moves and additions, as the terrain's scene has them.
pub(super) fn reapply(
    resources: &dyn crate::resources::ResourceSource,
    built: &[Built],
    sources: &mut Placements,
) -> WorldResult<()> {
    let layout = sources.layout.resource.clone();
    for placement in built.iter().flat_map(|b| &b.added) {
        sources.add_type(resources, &layout, &placement.object_type)?;
    }
    apply_edits(built, sources);
    Ok(())
}

#[cfg(test)]
mod tests;

/// Stable source IDs of second tiles and their base, for saved menu selections.
/// Geometry uses the same type, heading and distance rule in `apply`.
pub fn pair_aliases(
    layout: &tore_formats::mission::Layout,
) -> std::collections::BTreeMap<u32, u32> {
    let mut aliases = std::collections::BTreeMap::new();
    for base in &layout.placements {
        let kind = base.object_type.to_ascii_uppercase();
        if !["STRIP3.OT", "STRIP5.OT", "STRIP6.OT", "STRIP7.OT"].contains(&kind.as_str()) {
            continue;
        }
        let tile_kind = kind.replace(".OT", "A.OT");
        let distance = |p: &Placement| {
            let x = i64::from(p.position[0]) - i64::from(base.position[0]);
            let z = i64::from(p.position[2]) - i64::from(base.position[2]);
            i128::from(x) * i128::from(x) + i128::from(z) * i128::from(z)
        };
        if let Some(tile) = layout
            .placements
            .iter()
            .filter(|p| {
                p.object_type.eq_ignore_ascii_case(&tile_kind)
                    && (p.angles[0] - base.angles[0]).rem_euclid(360) == 0
                    && distance(p) <= 14_000i128 * 14_000
                    && !aliases.contains_key(&(0x4000_0000 + p.key.ordinal))
            })
            .min_by_key(|p| (distance(p), p.key.ordinal))
        {
            aliases.insert(
                0x4000_0000 + tile.key.ordinal,
                0x4000_0000 + base.key.ordinal,
            );
        }
    }
    aliases
}
