//! City fill: real-size buildings over the light areas of a theater's city
//! texture tiles. On by default (John, 2026-10-11, protocol 25): a terrain
//! carries a fill when its [`crate::terrain::Overrides::city_fill`] (or a
//! recording's city fill scene) says so. The app's `TORE_CITY_FILL=0` turns
//! it off for single player and the offline tools, a developer escape hatch.
//!
//! Fill buildings are scenery. They are drawn, an aircraft that flies into
//! one below its roof crashes ([`CityFill::contact`]), and a ground burst can
//! knock them flat ([`CityFill::flatten`], the hook for the X1 splash). They
//! are not combat targets, not static objects, not on sensors and never
//! scored. The retail city clusters stay the mission objects; the fill keeps
//! clear of them.
//!
//! The fill is a function of the theater layout alone (its texture tiles, its
//! objects and the kit shapes), in integer feet and whole degrees, so every
//! client builds the same one. It is not on the wire: a mission text always
//! means the fill, so every machine of a networked game builds it.

pub mod fill;
pub mod kit;
pub mod mask;

use crate::{resources::ResourceSource, surface::layout::trig, terrain::Terrain};
use std::{
    collections::BTreeMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

static CAP: std::sync::atomic::AtomicUsize =
    std::sync::atomic::AtomicUsize::new(fill::MAX_INSTANCES);

/// Sets the per-theater building budget for fills built from now on (the
/// app's `TORE_CITY_FILL_CAP`, a measuring aid; default
/// [`fill::MAX_INSTANCES`]).
pub fn set_cap(cap: usize) {
    CAP.store(cap, Ordering::Relaxed);
}

/// The per-theater building budget.
pub fn cap() -> usize {
    CAP.load(Ordering::Relaxed)
}

/// A building falls where a ground burst still passes on at least this
/// percent of its record's damage (fitted): a Mk 84's 70 percent at the
/// burst reaches it out to 571 ft of its 1,000 ft radius; a missile's 17.5
/// percent knocks nothing down.
pub const FLATTEN_PERCENT: i32 = 30;

/// How far a ground burst with collateral `radius` (feet) and `percent` at
/// the burst knocks fill buildings flat: where the splash's straight-line
/// fall-off reaches [`FLATTEN_PERCENT`].
pub fn flatten_radius(radius: f64, percent: i32) -> f64 {
    if percent <= FLATTEN_PERCENT || radius <= 0. {
        return 0.;
    }
    radius * f64::from(percent - FLATTEN_PERCENT) / f64::from(percent)
}

/// One fill building.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Instance {
    /// Index into the kit.
    pub piece: u16,
    /// Base centre, whole feet.
    pub x: i32,
    pub z: i32,
    /// Compass heading of the piece's forward axis, whole degrees 0 to 359.
    pub heading: i16,
    /// Placed on a core lot.
    pub core: bool,
    /// Index into the layout's cities.
    pub city: u16,
}

/// A theater layout's fill: the same for every mission on that layout.
#[derive(Debug)]
pub struct Layout {
    pub layout: String,
    pub map: mask::LightMap,
    pub kit: kit::Kit,
    pub cities: Vec<fill::City>,
    pub instances: Vec<Instance>,
    /// Lots that passed before the budget.
    pub candidates: usize,
    /// Every resource name the build read, sorted. A terrain that takes
    /// the cached layout reads them again, so what a mission build reads
    /// (its content manifest) is the same whether or not this process built
    /// the fill before.
    pub inputs: Vec<String>,
}

/// A resource source that notes every name asked for.
struct Noting<'a> {
    inner: &'a dyn ResourceSource,
    names: std::cell::RefCell<std::collections::BTreeSet<String>>,
}

impl ResourceSource for Noting<'_> {
    fn get(&self, name: &str) -> Option<&Vec<u8>> {
        self.names.borrow_mut().insert(name.to_owned());
        self.inner.get(name)
    }
    fn theater_catalog(&self) -> tore_formats::Result<Vec<(String, String)>> {
        self.inner.theater_catalog()
    }
    fn theater_label(&self, code: &str) -> tore_formats::Result<Option<String>> {
        self.inner.theater_label(code)
    }
}

impl Layout {
    /// FNV-1a over every instance, for determinism checks.
    pub fn digest(&self) -> u64 {
        let mut bytes = Vec::with_capacity(self.instances.len() * 12);
        for i in &self.instances {
            bytes.extend_from_slice(&i.piece.to_le_bytes());
            bytes.extend_from_slice(&i.x.to_le_bytes());
            bytes.extend_from_slice(&i.z.to_le_bytes());
            bytes.extend_from_slice(&i.heading.to_le_bytes());
        }
        tore_codec::fnv1a64(&bytes)
    }
}

/// Grid cell for the collision and flatten queries, feet.
pub const GRID_FT: i64 = 1_024;

/// A terrain's fill: its layout, each building's ground height, the
/// buildings a mission's own objects stand on (hidden), and which are
/// flattened.
pub struct CityFill {
    pub layout: Arc<Layout>,
    /// Ground height under each building's base centre, feet.
    pub base: Vec<f32>,
    /// Hidden under this mission's ground target objects.
    pub hidden: Vec<bool>,
    flattened: Vec<AtomicU64>,
    version: AtomicU64,
    grid: BTreeMap<(i64, i64), Vec<u32>>,
}

impl std::fmt::Debug for CityFill {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CityFill")
            .field("layout", &self.layout.layout)
            .field("instances", &self.layout.instances.len())
            .finish()
    }
}

static LAYOUTS: Mutex<BTreeMap<String, Arc<Layout>>> = Mutex::new(BTreeMap::new());

fn cell(v: i64) -> i64 {
    v.div_euclid(GRID_FT)
}

impl CityFill {
    /// The fill for a finished terrain: the layout's fill (built once per
    /// process and layout), minus the buildings this mission's ground target
    /// objects stand on. `None` when the theater has no city tiles.
    pub fn for_terrain(resources: &dyn ResourceSource, terrain: &Terrain) -> Option<Self> {
        let layout = {
            let mut cache = LAYOUTS.lock().unwrap_or_else(|e| e.into_inner());
            // The fill depends on the layout, the budget and what it keeps
            // out of (the redrawn airports move buildings and pavement).
            let keepouts = layout_keepouts(terrain);
            let mut bytes = Vec::with_capacity(keepouts.len() * 48);
            for k in &keepouts {
                for v in [k.center[0], k.center[1], k.half[0], k.half[1]] {
                    bytes.extend_from_slice(&v.to_le_bytes());
                }
                bytes.extend_from_slice(&i64::from(k.heading).to_le_bytes());
                bytes.extend_from_slice(&k.clearance.to_le_bytes());
            }
            let key = format!(
                "{}#{}#{:016x}",
                terrain.layout,
                cap(),
                tore_codec::fnv1a64(&bytes)
            );
            match cache.get(&key) {
                Some(layout) => {
                    for name in &layout.inputs {
                        resources.get(name);
                    }
                    Arc::clone(layout)
                }
                None => {
                    let layout = Arc::new(build_layout(resources, terrain));
                    cache.insert(key, Arc::clone(&layout));
                    layout
                }
            }
        };
        if layout.instances.is_empty() {
            return None;
        }
        let mission: Vec<fill::Keepout> = terrain
            .airport_scene
            .objects
            .iter()
            .filter(|o| crate::surface::UnitId(o.id).range() != crate::surface::IdRange::Layout)
            .map(|o| fill::Keepout {
                clearance: 10,
                ..fill::Keepout::of(o)
            })
            .collect();
        let mut base = Vec::with_capacity(layout.instances.len());
        let mut hidden = Vec::with_capacity(layout.instances.len());
        for i in &layout.instances {
            let piece = &layout.kit.pieces[usize::from(i.piece)];
            let r = piece.half[0].max(piece.half[1]) * 3 / 2;
            base.push(terrain.height(i.x as f32, i.z as f32));
            hidden.push(
                mission
                    .iter()
                    .any(|k| k.reaches([i64::from(i.x), i64::from(i.z)], r)),
            );
        }
        Some(Self::assemble(layout, base, hidden))
    }

    /// A fill from its layout, each building's ground height and the hidden
    /// set; builds the query grid.
    pub fn assemble(layout: Arc<Layout>, base: Vec<f32>, hidden: Vec<bool>) -> Self {
        let mut grid = BTreeMap::<(i64, i64), Vec<u32>>::new();
        for (n, i) in layout.instances.iter().enumerate() {
            let piece = &layout.kit.pieces[usize::from(i.piece)];
            let r = piece.half[0].max(piece.half[1]) * 3 / 2;
            let (x, z) = (i64::from(i.x), i64::from(i.z));
            for cz in cell(z - r)..=cell(z + r) {
                for cx in cell(x - r)..=cell(x + r) {
                    grid.entry((cx, cz)).or_default().push(n as u32);
                }
            }
        }
        Self {
            flattened: (0..layout.instances.len().div_ceil(64))
                .map(|_| AtomicU64::new(0))
                .collect(),
            version: AtomicU64::new(0),
            layout,
            base,
            hidden,
            grid,
        }
    }

    /// Stands every knocked-down building up again: a restarted flight
    /// starts with the city whole.
    pub fn reset(&self) {
        let mut any = false;
        for word in &self.flattened {
            any |= word.swap(0, Ordering::Relaxed) != 0;
        }
        if any {
            self.version.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Whether building `n` is knocked flat.
    pub fn is_flattened(&self, n: usize) -> bool {
        self.flattened[n / 64].load(Ordering::Relaxed) & (1 << (n % 64)) != 0
    }

    /// Whether building `n` stands: neither hidden nor flattened.
    pub fn standing(&self, n: usize) -> bool {
        !self.hidden[n] && !self.is_flattened(n)
    }

    /// Bumped by every [`CityFill::flatten`] that knocks a building down, so
    /// a renderer knows when to look again.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }

    /// The X1 splash hook: knocks flat every standing building whose base
    /// centre lies within `radius_ft` (horizontal) of `center`, and returns
    /// how many fell. Presentation and collision only: nothing is scored.
    pub fn flatten(&self, center: [f64; 3], radius_ft: f64) -> usize {
        if radius_ft.is_nan() || radius_ft <= 0. {
            return 0;
        }
        let r = radius_ft.ceil() as i64;
        let (x, z) = (center[0].round() as i64, center[2].round() as i64);
        let mut fell = 0;
        let mut checked = std::collections::BTreeSet::new();
        for cz in cell(z - r)..=cell(z + r) {
            for cx in cell(x - r)..=cell(x + r) {
                for &n in self.grid.get(&(cx, cz)).into_iter().flatten() {
                    let n = n as usize;
                    if !checked.insert(n) || !self.standing(n) {
                        continue;
                    }
                    let i = &self.layout.instances[n];
                    let d = [f64::from(i.x) - center[0], f64::from(i.z) - center[2]];
                    if d[0] * d[0] + d[1] * d[1] <= radius_ft * radius_ft {
                        self.flattened[n / 64].fetch_or(1 << (n % 64), Ordering::Relaxed);
                        fell += 1;
                    }
                }
            }
        }
        if fell > 0 {
            self.version.fetch_add(1, Ordering::Relaxed);
        }
        fell
    }

    /// The earliest standing building the segment `from` to `to` enters
    /// below its roof: its index and the fraction along the segment. The box
    /// is the piece's footprint turned to the building's whole-degree
    /// heading with the integer sine table, from its base up to its roof.
    pub fn contact(&self, from: [f64; 3], to: [f64; 3]) -> Option<(usize, f64)> {
        let lo = [from[0].min(to[0]), from[2].min(to[2])];
        let hi = [from[0].max(to[0]), from[2].max(to[2])];
        let mut best: Option<(usize, f64)> = None;
        for cz in cell(lo[1].floor() as i64)..=cell(hi[1].ceil() as i64) {
            for cx in cell(lo[0].floor() as i64)..=cell(hi[0].ceil() as i64) {
                for &n in self.grid.get(&(cx, cz)).into_iter().flatten() {
                    let n = n as usize;
                    if !self.standing(n) {
                        continue;
                    }
                    if let Some(at) = self.segment(n, from, to)
                        && best.is_none_or(|(b, t)| at < t || (at == t && n < b))
                    {
                        best = Some((n, at));
                    }
                }
            }
        }
        best
    }

    fn segment(&self, n: usize, from: [f64; 3], to: [f64; 3]) -> Option<f64> {
        let i = &self.layout.instances[n];
        let piece = &self.layout.kit.pieces[usize::from(i.piece)];
        let one = trig::ONE as f64;
        let (s, c) = (
            trig::sin(i32::from(i.heading)) as f64 / one,
            trig::cos(i32::from(i.heading)) as f64 / one,
        );
        let base = f64::from(self.base[n]);
        let local = |p: [f64; 3]| {
            let (dx, dz) = (p[0] - f64::from(i.x), p[2] - f64::from(i.z));
            [dx * c - dz * s, p[1] - base, dx * s + dz * c]
        };
        let (a, b) = (local(from), local(to));
        let lo = [-(piece.half[0] as f64), 0., -(piece.half[1] as f64)];
        let hi = [
            piece.half[0] as f64,
            piece.height as f64,
            piece.half[1] as f64,
        ];
        let (mut enter, mut leave) = (0.0_f64, 1.0_f64);
        for axis in 0..3 {
            let d = b[axis] - a[axis];
            if d == 0. {
                if a[axis] < lo[axis] || a[axis] > hi[axis] {
                    return None;
                }
            } else {
                let (mut p, mut q) = ((lo[axis] - a[axis]) / d, (hi[axis] - a[axis]) / d);
                if p > q {
                    std::mem::swap(&mut p, &mut q);
                }
                enter = enter.max(p);
                leave = leave.min(q);
                if enter > leave {
                    return None;
                }
            }
        }
        Some(enter)
    }
}

/// Reads every resource the fill of `terrain` is built from, without
/// building it: the reference palette's weather layer, every ground
/// placement's picture, the kit's definitions and shapes, and the layout.
/// A game's content lists these for the theater whether or not this process
/// has built (and cached) that layout's fill already.
pub fn read_inputs(resources: &dyn ResourceSource, terrain: &Terrain) {
    let base = tore_formats::theater::base_theater(&terrain.layout).unwrap_or("");
    mask::read_inputs(resources, &terrain.environment, base);
    kit::Kit::load(resources);
    resources.get(&terrain.layout);
}

/// What the fill keeps out of: every layout object's box (runways with the
/// wider clearance) and, with the redrawn airports on, each redrawn
/// airfield's whole box with the runway clearance.
fn layout_keepouts(terrain: &Terrain) -> Vec<fill::Keepout> {
    let mut keepouts: Vec<fill::Keepout> = terrain
        .airport_scene
        .objects
        .iter()
        .filter(|o| crate::surface::UnitId(o.id).range() == crate::surface::IdRange::Layout)
        .map(fill::Keepout::of)
        .collect();
    keepouts.extend(
        terrain
            .redrawn
            .iter()
            .map(|r| fill::Keepout::of_box(&r.surface, fill::RUNWAY_CLEARANCE_FT)),
    );
    keepouts
}

pub(crate) fn build_layout(resources: &dyn ResourceSource, terrain: &Terrain) -> Layout {
    let noting = Noting {
        inner: resources,
        names: Default::default(),
    };
    let mut layout = build_layout_from(&noting, terrain);
    layout.inputs = noting.names.into_inner().into_iter().collect();
    layout
}

fn build_layout_from(resources: &dyn ResourceSource, terrain: &Terrain) -> Layout {
    let base = tore_formats::theater::base_theater(&terrain.layout).unwrap_or("");
    // A theater with no city pictures fills its retail clusters' footprints
    // at the retail shape scale, three times their drawn size.
    let districts: Vec<mask::District> = terrain
        .airport_scene
        .objects
        .iter()
        .filter(|o| {
            crate::surface::UnitId(o.id).range() == crate::surface::IdRange::Layout
                && kit::CLUSTERS.contains(&o.object_type.as_str())
        })
        .map(|o| {
            let k = fill::Keepout::of(o);
            mask::District {
                center: k.center,
                half: [k.half[0] * 3, k.half[1] * 3],
                heading: k.heading,
            }
        })
        .collect();
    let map = mask::LightMap::build(resources, &terrain.environment, base, &districts);
    let kit = kit::Kit::load(resources);
    let keepouts = layout_keepouts(terrain);
    // City names: the layout's named placements (its airfields).
    let names: Vec<([i64; 2], String)> = resources
        .get(&terrain.layout)
        .and_then(|bytes| tore_formats::mission::Layout::parse(&terrain.layout, bytes).ok())
        .map(|layout| {
            layout
                .placements
                .iter()
                .filter_map(|pl| {
                    let name = pl.name.as_ref()?.trim();
                    (!name.is_empty()).then(|| {
                        (
                            [i64::from(pl.position[0]), i64::from(pl.position[2])],
                            name.to_owned(),
                        )
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let (cities, instances, candidates) =
        fill::place(&map, &kit, &terrain.layout, &keepouts, &names, cap());
    Layout {
        layout: terrain.layout.clone(),
        map,
        kit,
        cities,
        instances,
        candidates,
        inputs: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tower() -> kit::Piece {
        kit::Piece {
            source: "T.OT".into(),
            shape: "T.SH".into(),
            faces: Vec::new(),
            origin: [0.; 3],
            scale: 1.,
            half: [20, 50],
            height: 200,
            class: kit::Class::Tower,
            cut: false,
        }
    }

    fn fill(heading: i16) -> CityFill {
        let layout = Layout {
            layout: "TEST.MM".into(),
            map: mask::LightMap::default(),
            kit: kit::Kit {
                pieces: vec![tower()],
            },
            cities: Vec::new(),
            instances: vec![Instance {
                piece: 0,
                x: 10_000,
                z: 20_000,
                heading,
                core: true,
                city: 0,
            }],
            candidates: 1,
            inputs: Vec::new(),
        };
        CityFill::assemble(Arc::new(layout), vec![100.], vec![false])
    }

    #[test]
    fn an_aircraft_below_the_roof_inside_the_footprint_crashes() {
        let f = fill(0);
        // Across the 40 ft wide side at 150 ft above the 100 ft ground.
        let hit = f.contact([9_900., 250., 20_000.], [10_100., 250., 20_000.]);
        assert_eq!(hit.map(|h| h.0), Some(0));
        assert!((hit.unwrap().1 - 0.4).abs() < 1e-9);
        // Over the roof, beside the footprint, or wholly inside one tick.
        assert!(
            f.contact([9_900., 310., 20_000.], [10_100., 310., 20_000.])
                .is_none()
        );
        assert!(
            f.contact([9_900., 250., 20_060.], [10_100., 250., 20_060.])
                .is_none()
        );
        assert!(
            f.contact([10_000., 250., 20_000.], [10_000.5, 250., 20_000.])
                .is_some()
        );
        // Turned a quarter, the long side lies east-west.
        let f = fill(90);
        assert!(
            f.contact([9_900., 250., 20_015.], [10_100., 250., 20_015.])
                .is_some()
        );
        assert!(
            f.contact([9_900., 250., 20_030.], [10_100., 250., 20_030.])
                .is_none()
        );
        assert!(
            f.contact([10_045., 250., 19_000.], [10_045., 250., 21_000.])
                .is_some()
        );
    }

    #[test]
    fn a_burst_flattens_buildings_and_they_stop_colliding() {
        let f = fill(0);
        assert_eq!(f.flatten([12_000., 0., 20_000.], 500.), 0);
        assert_eq!(f.version(), 0);
        assert_eq!(f.flatten([10_300., 0., 20_000.], 500.), 1);
        assert_eq!(f.version(), 1);
        assert!(f.is_flattened(0));
        assert!(
            f.contact([9_900., 250., 20_000.], [10_100., 250., 20_000.])
                .is_none()
        );
        // Already flat: nothing more falls.
        assert_eq!(f.flatten([10_300., 0., 20_000.], 500.), 0);
        // A restart stands it up again.
        f.reset();
        assert!(!f.is_flattened(0));
        assert_eq!(f.version(), 2);
        assert!(
            f.contact([9_900., 250., 20_000.], [10_100., 250., 20_000.])
                .is_some()
        );
    }

    #[test]
    fn only_a_strong_ground_burst_flattens() {
        assert_eq!(flatten_radius(1_000., 70), 1_000. * 40. / 70.);
        assert_eq!(flatten_radius(750., 17), 0.);
        assert_eq!(flatten_radius(0., 100), 0.);
    }
}
