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
//! redraws them all, and the runway's position, heading, length and ILS do
//! not move. A pair plan joins a base type and its second tile (the A
//! types) placed beside it into one airfield.
//!
//! What a redrawn airport replaces, for its STRIP placements: the drawn mesh
//! ([`Built::patches`], drawn by the app), the landable and contact box
//! ([`Built::surface`], the whole redrawn field, grass included), the AI
//! anchors ([`Built::anchors`]) and the positions of the airport's
//! buildings, plus extra hangars and vehicles. The runway's approach line,
//! length and elevation stay the retail ones.
mod buildings;
mod data;
pub mod geometry;
pub mod layout;
pub mod lights;
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
}

impl Plans {
    /// The built-in plans when `enabled`, else none.
    pub fn new(enabled: bool) -> WorldResult<Self> {
        if !enabled {
            return Ok(Self::default());
        }
        Ok(Self {
            plans: Self::builtin()?,
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
    /// Layout placements moved: index in the layout and the new placement.
    moved: Vec<(usize, Placement)>,
    /// Placements added after the layout's own.
    added: Vec<Placement>,
    /// Parked template aircraft moved off the grass onto the aprons.
    pub snapped: usize,
    /// For a second tile (paired or alone): its runway object's approach
    /// line, moved onto the tile's main paved runway. A tile's STRIP anchors
    /// are copies of its base's, so its retail line lies beside its
    /// pavement; `None` keeps the retail line (every base and pad).
    pub runway_line: Option<RunwayLine>,
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
    /// The longest runway of a second tile's plan (the first of equals), in
    /// the tile's own frame, whose heading is `heading`.
    fn main(plan: &Plan, frame: &Frame, heading: f64) -> Option<Self> {
        let runway = plan
            .runways
            .iter()
            .reduce(|best, r| if r.length > best.length { r } else { best })?;
        let d = geometry::direction(runway.heading);
        let middle = [
            runway.from[0] + d[0] * runway.length * 0.5,
            runway.from[1] + d[1] * runway.length * 0.5,
        ];
        Some(Self {
            center: frame.world(middle),
            heading: (heading + runway.heading.to_radians()).rem_euclid(std::f64::consts::TAU),
            length: runway.length,
        })
    }
}

impl Built {
    /// The redrawn airport that replaces runway object `id`, if any.
    pub fn for_runway(built: &[Built], id: u32) -> Option<&Built> {
        built.iter().find(|b| b.strip_id == id)
    }
    /// How many retail buildings it moved and how many it added.
    pub fn building_counts(&self) -> (usize, usize) {
        (self.moved.len(), self.added.len())
    }
}

/// A STRIP placement of the layout, with what the plans need of it.
struct StripPlacement {
    index: usize,
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
) -> WorldResult<Vec<Built>> {
    if plans.is_empty() {
        return Ok(Vec::new());
    }
    let mut strips = Vec::new();
    for (index, placement) in sources.layout.placements.iter().enumerate() {
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
            index,
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
        let frame = base.frame;
        let mut parts = vec![(base_plan, [0., 0.])];
        let mut links = None;
        let mut name = base.object_type.clone();
        if let (Some(tile), Some(tile_plan)) = (tile, tile_plan) {
            parts.push((tile_plan, frame.local(tile.frame.origin)));
            links = plans.pair(&base.object_type);
            name = format!("{}+{}", base.object_type, tile.object_type);
        }
        let (materials, patches) = layout::compose(&parts, links, f64::from(base.angle));
        let airport_lights = lights::lights(&parts, links, &patches, &materials, &frame, &height);
        let a = base_plan.anchors.as_ref();
        let anchor_points: Vec<Point> = a
            .map(|a| {
                a.taxi_out
                    .iter()
                    .chain(&a.taxi_in)
                    .chain(&a.parking)
                    .chain([&a.takeoff, &a.landing])
                    .copied()
                    .collect()
            })
            .unwrap_or_default();
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
        // A tile alone (no base beside it) flies from its own main runway
        // and has no taxi points: the AI's runway fallback, as retail's.
        let runway_line = (!base_plan.ils_runway)
            .then(|| RunwayLine::main(base_plan, &frame, base.stance.heading))
            .flatten();
        let anchors = a.map(|a| AirfieldAnchors {
            taxi_out: a.taxi_out.map(|p| frame.world(p)),
            takeoff_spot: frame.world(a.takeoff),
            takeoff_heading: base.stance.heading,
            landing_point: frame.world(a.landing),
            landing_heading: base.stance.heading,
            taxi_in: a.taxi_in.map(|p| frame.world(p)),
            parking: a.parking.map(|p| frame.world(p)),
            parking_heading: (base.stance.heading + a.parking_heading.to_radians())
                .rem_euclid(std::f64::consts::TAU),
        });
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
        let slots: Vec<Point> = a.map(|a| a.parking.to_vec()).unwrap_or_default();
        let spots = buildings::apron_spots(&aprons, &slots);
        let on_pavement = |p: Point| {
            patches
                .iter()
                .any(|patch| geometry::contains(&patch.poly, p))
        };
        let snapped = buildings::snap_parked(parked, &frame, &own, &zones, &spots, &on_pavement);
        let id = |index: usize| -> WorldResult<u32> {
            Ok(
                crate::surface::UnitId::layout(sources.layout.placements[index].key.ordinal)
                    .ok_or("airport object ID overflow")?
                    .0,
            )
        };
        let base_id = id(base.index)?;
        let tile_id = tile.map(|t| id(t.index)).transpose()?;
        let built = Built {
            plan: name.clone(),
            strip_id: base_id,
            frame,
            materials,
            patches,
            surface,
            anchors,
            lights: airport_lights,
            moved: relaid.moved,
            added: relaid.added,
            snapped,
            runway_line,
        };
        apply_edits(std::slice::from_ref(&built), sources);
        if let (Some(tile_id), Some(tile), Some(tile_plan)) = (tile_id, tile, tile_plan) {
            // The tile's runway object lands and departs on the tile's own
            // main runway inside the pair's box. Its taxi points would be
            // the base's, 9,000 ft away, so it has none: the AI's runway
            // fallback, as at retail's tiles, whose points lay off their
            // pavement.
            out.push(Built {
                plan: name,
                strip_id: tile_id,
                frame,
                materials: Vec::new(),
                patches: Vec::new(),
                surface,
                anchors: None,
                lights: Vec::new(),
                moved: Vec::new(),
                added: Vec::new(),
                snapped: 0,
                runway_line: RunwayLine::main(tile_plan, &tile.frame, tile.stance.heading),
            });
        }
        out.push(built);
    }
    Ok(out)
}

fn apply_edits(built: &[Built], sources: &mut Placements) {
    for airport in built {
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
