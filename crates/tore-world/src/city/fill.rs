//! Where the fill puts its buildings (CF0 prototype).
//!
//! Each city tile is cut into 2,048 ft blocks (16 by 16 texels) and each
//! block carries two lot grids. On the 10 a side grid (204.8 ft), a core lot
//! stands wholly inside a big light roof (all nine of a 3 by 3 sample grid on
//! solid roof texels) and a roof lot on a narrow roof (eight of the nine on
//! light roof texels). On the 16 a side grid (128 ft, one lot per texel), a
//! house lot stands on a painted house: a light texel with at least two more
//! among its eight neighbours, and no core or roof lot under it. A lot is
//! built when it keeps clear of the layout's objects and runways and the
//! chosen building's corners stay off streets, parks and water.
//!
//! When a theater has more lots than the budget, each city gets a share by
//! its number of tiles (a city with fewer lots than its share keeps them all
//! and the rest is shared again). Within a city the core and roof lots
//! nearest the centre go first, then house lots in a fixed shuffled order, so
//! a thinned city keeps houses over all its painted area.
//!
//! Integer feet, whole degrees and the integer sine table throughout, seeded
//! from the layout name alone: every client and every mission on a theater
//! gets the same fill.

use super::{
    Instance,
    kit::{Class, Kit},
    mask::{self, LightMap},
};
use crate::surface::{
    layout::trig,
    resolve::{Purpose, Stream},
};
use std::collections::BTreeMap;

/// Feet per block side: 16 texels.
pub const BLOCK_FT: i64 = 2_048;
/// Lots a side in a core block: 204.8 ft lots (John's 200 ft; fitted to the
/// texel grid).
pub const CORE_LOTS: i64 = 10;
/// Lots a side for houses: 128 ft lots, one per texel (John, 2026-10-10:
/// "house lots at about 100 to 150 ft").
pub const HOUSE_LOTS: i64 = 16;
/// Light texels of the nine around a house lot's centre it needs (fitted).
pub const HOUSE_HITS: u32 = 3;
/// Light roof samples of nine a roof lot needs: a narrow roof that builds
/// low-rise on 204.8 ft lots (fitted).
pub const ROOF_HITS: u32 = 8;
/// The share of a city's radius, in thousandths, inside which core lots
/// favour towers (fitted).
pub const DOWNTOWN: i64 = 350;
/// The smallest footprint side a core lot builds, feet (fitted): big roofs
/// get big buildings.
pub const CORE_MIN_SIDE_FT: i64 = 50;
/// The most buildings a theater gets by default (John, 2026-10-10, raised
/// from 50,000 to 200,000, then 500,000). `crate::city::set_cap` changes it
/// for measurements.
pub const MAX_INSTANCES: usize = 500_000;
/// Clearance kept from a runway's airfield box, past the lot's half width.
pub const RUNWAY_CLEARANCE_FT: i64 = 300;
/// Clearance kept from any other layout object's box.
pub const OBJECT_CLEARANCE_FT: i64 = 40;
/// A building keeps this far inside its lot (fitted).
pub const LOT_MARGIN_FT: i64 = 12;
/// Largest jitter inside the lot, each axis (fitted).
pub const MAX_JITTER_FT: i64 = 40;

/// A horizontal box the fill keeps out of: an object's footprint, whole feet
/// and whole degrees.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Keepout {
    pub center: [i64; 2],
    pub half: [i64; 2],
    pub heading: i32,
    pub clearance: i64,
}

impl Keepout {
    /// The box of a scene object; runways keep the wider clearance.
    pub fn of(object: &tore_sim::airport::StaticObject) -> Self {
        Self::of_box(
            &object.bounds,
            if object.runway {
                RUNWAY_CLEARANCE_FT
            } else {
                OBJECT_CLEARANCE_FT
            },
        )
    }
    /// A box with a clearance: whole feet, whole degrees.
    pub fn of_box(b: &tore_sim::airport::OrientedBox, clearance: i64) -> Self {
        Self {
            center: [b.center[0].round() as i64, b.center[2].round() as i64],
            half: [b.half[0].ceil() as i64, b.half[2].ceil() as i64],
            heading: b.heading.to_degrees().round() as i32,
            clearance,
        }
    }
    /// Whether a circle of `radius` at `point` reaches the box plus its
    /// clearance (a square test in the box's frame; conservative).
    pub fn reaches(&self, point: [i64; 2], radius: i64) -> bool {
        let d = [point[0] - self.center[0], point[1] - self.center[1]];
        let local = trig::to_local_scaled(d, self.heading);
        (0..2).all(|axis| {
            local[axis].abs() <= (self.half[axis] + self.clearance + radius) * trig::ONE
        })
    }
}

/// One city: a group of touching city tiles.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct City {
    /// The nearest named layout placement within 25 nm (an airfield), or
    /// "Town N".
    pub name: String,
    pub tiles: Vec<(i32, i32)>,
    /// The middle of its tiles, feet.
    pub center: [i64; 2],
    /// Half the diagonal of its tiles' bounding box, feet.
    pub radius: i64,
    /// Lots that passed, before the budget.
    pub candidates: usize,
    /// Buildings placed.
    pub placed: usize,
    /// ... of which on core lots.
    pub core: usize,
}

#[derive(Clone, Copy, Debug)]
struct Lot {
    x: i64,
    z: i64,
    pitch: i64,
    core: bool,
    house: bool,
    city: usize,
    ordinal: u32,
    long_x: bool,
    diagonal: Option<i32>,
}

fn cities(
    map: &LightMap,
    names: &[([i64; 2], String)],
) -> (Vec<City>, BTreeMap<(i32, i32), usize>) {
    let mut of = BTreeMap::new();
    let mut out = Vec::new();
    for &start in map.tiles.keys() {
        if of.contains_key(&start) {
            continue;
        }
        let index = out.len();
        let mut tiles = Vec::new();
        let mut stack = vec![start];
        of.insert(start, index);
        while let Some(t) = stack.pop() {
            tiles.push(t);
            for dc in [-4, 0, 4] {
                for dr in [-4, 0, 4] {
                    let n = (t.0 + dc, t.1 + dr);
                    if map.tiles.contains_key(&n) && !of.contains_key(&n) {
                        of.insert(n, index);
                        stack.push(n);
                    }
                }
            }
        }
        tiles.sort_unstable();
        let cell = mask::TILE_FT / 4;
        let lo = [
            tiles.iter().map(|t| i64::from(t.0)).min().unwrap_or(0) * cell,
            tiles.iter().map(|t| i64::from(t.1)).min().unwrap_or(0) * cell,
        ];
        let hi = [
            tiles.iter().map(|t| i64::from(t.0)).max().unwrap_or(0) * cell + mask::TILE_FT,
            tiles.iter().map(|t| i64::from(t.1)).max().unwrap_or(0) * cell + mask::TILE_FT,
        ];
        let center = [(lo[0] + hi[0]) / 2, (lo[1] + hi[1]) / 2];
        let radius =
            trig::isqrt(((hi[0] - lo[0]).pow(2) + (hi[1] - lo[1]).pow(2)) as u128) as i64 / 2;
        let limit = 25 * 6_076;
        let name = names
            .iter()
            .map(|(at, name)| {
                let d = (at[0] - center[0]).pow(2) + (at[1] - center[1]).pow(2);
                (d, name)
            })
            .filter(|(d, _)| *d <= limit * limit)
            .min()
            .map_or_else(|| format!("Town {}", index + 1), |(_, n)| n.clone());
        out.push(City {
            name,
            tiles,
            center,
            radius: radius.max(1),
            candidates: 0,
            placed: 0,
            core: 0,
        });
    }
    // Two cities named after one airfield are told apart by number.
    let mut counts = BTreeMap::<String, usize>::new();
    for city in &out {
        *counts.entry(city.name.clone()).or_default() += 1;
    }
    let mut seen = BTreeMap::<String, usize>::new();
    for city in &mut out {
        if counts[&city.name] > 1 {
            let n = seen.entry(city.name.clone()).or_default();
            *n += 1;
            city.name = format!("{} {}", city.name, n);
        }
    }
    (out, of)
}

/// Texels past a tile's edge a lot's tests read: the street window's seven
/// and the roof run's eight.
const APRON: i64 = 8;
/// Texels a side of a [`Patch`].
const SIDE: usize = mask::TEXELS + 2 * APRON as usize;
/// Half the street window, texels.
const STREET_REACH: i64 = 7;

/// One city tile and an [`APRON`] of its neighbours, world oriented: every
/// texel's flags as [`LightMap::flags_at`] reads them, and running sums of
/// the three street flags. A lot's samples are then plain reads, and its 15
/// by 15 street window four reads per flag instead of 225 map lookups. A
/// sample anywhere inside a texel reads that texel's flags (city tiles start
/// on whole texels), so this gives exactly what the map lookups gave.
struct Patch {
    /// The world texel (feet over [`mask::TEXEL_FT`]) of the first entry.
    origin: [i64; 2],
    flags: Vec<u8>,
    /// Summed-area tables of the axis, D1 and D2 street flags, `SIDE + 1`
    /// a row, with a zero first row and column.
    sums: [Vec<u32>; 3],
}

impl Patch {
    fn new(map: &LightMap, key: (i32, i32), tile: &mask::PlacedTile) -> Self {
        let t = mask::TEXEL_FT;
        let first = |k: i32| i64::from(k) * (mask::TILE_FT / 4) / t - APRON;
        let origin = [first(key.0), first(key.1)];
        let outside = map.lookup();
        let mut flags = vec![0u8; SIDE * SIDE];
        let inner = APRON as usize..APRON as usize + mask::TEXELS;
        for r in 0..SIDE {
            let z = (origin[1] + r as i64) * t + t / 2;
            for c in 0..SIDE {
                let x = (origin[0] + c as i64) * t + t / 2;
                flags[r * SIDE + c] = if inner.contains(&r) && inner.contains(&c) {
                    LightMap::flags_in(tile, key, x, z)
                } else {
                    outside(x, z)
                };
            }
        }
        let w = SIDE + 1;
        let sums = [mask::STREET_AXIS, mask::STREET_D1, mask::STREET_D2].map(|bit| {
            let mut sum = vec![0u32; w * w];
            for r in 0..SIDE {
                let mut row = 0;
                for c in 0..SIDE {
                    row += u32::from(flags[r * SIDE + c] & bit != 0);
                    sum[(r + 1) * w + c + 1] = sum[r * w + c + 1] + row;
                }
            }
            sum
        });
        Self {
            origin,
            flags,
            sums,
        }
    }

    /// The patch column and row of world point `x`, `z`.
    fn cell(&self, x: i64, z: i64) -> (usize, usize) {
        let c = x.div_euclid(mask::TEXEL_FT) - self.origin[0];
        let r = z.div_euclid(mask::TEXEL_FT) - self.origin[1];
        debug_assert!((0..SIDE as i64).contains(&c) && (0..SIDE as i64).contains(&r));
        (c as usize, r as usize)
    }

    fn at(&self, x: i64, z: i64) -> u8 {
        let (c, r) = self.cell(x, z);
        self.flags[r * SIDE + c]
    }

    /// The axis, D1 and D2 street texels among the 15 by 15 round the texel
    /// under `x`, `z`.
    fn streets(&self, x: i64, z: i64) -> [i32; 3] {
        let (c, r) = self.cell(x, z);
        let w = SIDE + 1;
        let reach = STREET_REACH as usize;
        let (c0, c1, r0, r1) = (c - reach, c + reach + 1, r - reach, r + reach + 1);
        self.sums
            .each_ref()
            .map(|s| (s[r1 * w + c1] + s[r0 * w + c0] - s[r0 * w + c1] - s[r1 * w + c0]) as i32)
    }
}

/// The keep-outs near each 4,096 ft square, so a lot tests only those that
/// can reach it. A keep-out reaches a circle of radius `r` only where its
/// box, clearance and `r` reach in the box's frame; the integer turn scales
/// a distance by at least [`trig::ONE`] over the square root of two (checked
/// for every heading in the tests), so nothing past twice that reach in
/// either world axis can pass. `any` does not depend on order, so testing
/// the near ones gives what testing them all gave.
struct Keepouts<'a> {
    all: &'a [Keepout],
    near: BTreeMap<(i64, i64), Vec<u32>>,
}

impl<'a> Keepouts<'a> {
    const CELL: i64 = 4_096;

    /// `radius` is the largest the lots ask about.
    fn new(all: &'a [Keepout], radius: i64) -> Self {
        let mut near = BTreeMap::<(i64, i64), Vec<u32>>::new();
        for (n, k) in all.iter().enumerate() {
            let reach = 2 * (k.half[0].max(k.half[1]) + k.clearance + radius) + 2;
            let lo = [k.center[0] - reach, k.center[1] - reach];
            let hi = [k.center[0] + reach, k.center[1] + reach];
            for cz in lo[1].div_euclid(Self::CELL)..=hi[1].div_euclid(Self::CELL) {
                for cx in lo[0].div_euclid(Self::CELL)..=hi[0].div_euclid(Self::CELL) {
                    near.entry((cx, cz)).or_default().push(n as u32);
                }
            }
        }
        Self { all, near }
    }

    fn reach(&self, point: [i64; 2], radius: i64) -> bool {
        let cell = (
            point[0].div_euclid(Self::CELL),
            point[1].div_euclid(Self::CELL),
        );
        self.near.get(&cell).is_some_and(|list| {
            list.iter()
                .any(|&n| self.all[n as usize].reaches(point, radius))
        })
    }
}

fn lots(map: &LightMap, city_of: &BTreeMap<(i32, i32), usize>, keepouts: &[Keepout]) -> Vec<Lot> {
    let t = mask::TEXEL_FT;
    let keepouts = Keepouts::new(keepouts, BLOCK_FT / CORE_LOTS / 2);
    let mut out = Vec::new();
    let mut ordinal = 0u32;
    // The 204.8 ft grid's taken cells, which house lots keep off.
    let big_cell = |v: i64| (v * CORE_LOTS).div_euclid(BLOCK_FT);
    // Only asked whether it holds a cell, never walked, so its order
    // does not matter.
    let mut big_lots = std::collections::HashSet::<(i64, i64)>::new();
    for houses in [false, true] {
        let k = if houses { HOUSE_LOTS } else { CORE_LOTS };
        let pitch = BLOCK_FT / k;
        let step = if houses { t } else { BLOCK_FT / (3 * k) };
        for (&key, tile) in &map.tiles {
            let patch = Patch::new(map, key, tile);
            let cell = mask::TILE_FT / 4;
            let origin = [i64::from(key.0) * cell, i64::from(key.1) * cell];
            for bz in 0..mask::TILE_FT / BLOCK_FT {
                for bx in 0..mask::TILE_FT / BLOCK_FT {
                    let o = [origin[0] + bx * BLOCK_FT, origin[1] + bz * BLOCK_FT];
                    for j in 0..k {
                        for i in 0..k {
                            let x = o[0] + (2 * i + 1) * BLOCK_FT / (2 * k);
                            let z = o[1] + (2 * j + 1) * BLOCK_FT / (2 * k);
                            if patch.at(x, z) & mask::SITE == 0 {
                                continue;
                            }
                            let (mut hits, mut solid) = (0, 0);
                            for sz in -1..=1 {
                                for sx in -1..=1 {
                                    let f = patch.at(x + sx * step, z + sz * step);
                                    hits += u32::from(f & mask::SITE != 0);
                                    solid += u32::from(f & mask::SOLID != 0);
                                }
                            }
                            let core = !houses && solid == 9;
                            let pass = if houses {
                                let r = pitch / 2;
                                hits >= HOUSE_HITS
                                    && (big_cell(z - r)..=big_cell(z + r)).all(|cz| {
                                        (big_cell(x - r)..=big_cell(x + r))
                                            .all(|cx| !big_lots.contains(&(cx, cz)))
                                    })
                            } else {
                                core || hits >= ROOF_HITS
                            };
                            if !pass || keepouts.reach([x, z], pitch / 2) {
                                continue;
                            }
                            if !houses {
                                big_lots.insert((big_cell(x), big_cell(z)));
                            }
                            // The roof's longer extent through the lot centre.
                            let run = |dx: i64, dz: i64| {
                                let mut n = 0;
                                for s in [-1, 1] {
                                    for step in 1..=8 {
                                        let f =
                                            patch.at(x + s * step * dx * t, z + s * step * dz * t);
                                        if f & mask::SITE == 0 {
                                            break;
                                        }
                                        n += 1;
                                    }
                                }
                                n
                            };
                            // Streets around the lot: along the axes or a
                            // diagonal.
                            let [axis, d1, d2] = patch.streets(x, z);
                            // D1 runs east and south in the picture: a line
                            // at bearing 135; D2 at bearing 45.
                            let diagonal = (d1 + d2 > axis && d1.max(d2) >= 8)
                                .then_some(if d1 > d2 { 135 } else { 45 });
                            out.push(Lot {
                                x,
                                z,
                                pitch,
                                core,
                                house: houses,
                                city: city_of[&key],
                                ordinal,
                                long_x: run(1, 0) > run(0, 1),
                                diagonal,
                            });
                            ordinal += 1;
                        }
                    }
                }
            }
        }
    }
    out
}

/// Each city's share of `cap` by its number of tiles: a city with fewer lots
/// than its share keeps them all and the rest is shared again among the
/// others; leftovers go to the first cities in order.
fn shares(cities: &[City], cap: usize) -> Vec<usize> {
    let mut share = vec![0usize; cities.len()];
    let mut open: Vec<usize> = (0..cities.len()).collect();
    let mut left = cap;
    loop {
        let tiles: usize = open.iter().map(|&c| cities[c].tiles.len()).sum();
        if open.is_empty() || tiles == 0 {
            break;
        }
        let (full, rest): (Vec<usize>, Vec<usize>) = open
            .iter()
            .partition(|&&c| cities[c].candidates * tiles <= left * cities[c].tiles.len());
        if full.is_empty() {
            let mut given = 0;
            for &c in &open {
                share[c] = left * cities[c].tiles.len() / tiles;
                given += share[c];
            }
            for &c in open.iter().take(left - given) {
                share[c] += 1;
            }
            break;
        }
        for &c in &full {
            share[c] = cities[c].candidates;
            left -= share[c];
        }
        open = rest;
    }
    share
}

/// What each kind of lot builds: (class, weight) (fitted; towers downtown,
/// houses on the painted house areas).
fn weights(lot: &Lot, downtown: bool) -> &'static [(Class, u64)] {
    match (lot.core, downtown, !lot.house) {
        (true, true, _) => &[
            (Class::Tower, 45),
            (Class::Apartment, 40),
            (Class::LowRise, 15),
        ],
        (true, false, _) => &[
            (Class::Tower, 15),
            (Class::Apartment, 45),
            (Class::LowRise, 35),
            (Class::Industrial, 5),
        ],
        (false, _, true) => &[
            (Class::LowRise, 50),
            (Class::Apartment, 30),
            (Class::Industrial, 20),
        ],
        (false, _, false) => &[
            (Class::House, 75),
            (Class::LowRise, 20),
            (Class::Industrial, 5),
        ],
    }
}

fn radius(half: [i64; 2]) -> i64 {
    trig::isqrt((half[0] * half[0] + half[1] * half[1]) as u128) as i64 + 1
}

/// The buildings for one theater layout: its cities and its instances, in
/// lot order.
pub fn place(
    map: &LightMap,
    kit: &Kit,
    layout: &str,
    keepouts: &[Keepout],
    names: &[([i64; 2], String)],
    cap: usize,
) -> (Vec<City>, Vec<Instance>, usize) {
    let (mut cities, city_of) = cities(map, names);
    let mut lots = lots(map, &city_of, keepouts);
    let candidates = lots.len();
    for lot in &lots {
        cities[lot.city].candidates += 1;
    }
    let stem = format!("CITYFILL:{}", layout.trim_end_matches(".MM"));
    let stem_hash = Stream::stem(&stem);
    if lots.len() > cap {
        let share = shares(&cities, cap);
        // Each lot's key once (a house's rank is a draw), not once per
        // comparison. The ordinal makes every key distinct, so any sort
        // orders them as the stable sort by key did.
        let mut keyed: Vec<((usize, bool, u64, u32), u32)> = lots
            .iter()
            .enumerate()
            .map(|(n, lot)| {
                let city = &cities[lot.city];
                let rank = if lot.house {
                    // A fixed shuffle: thinned houses stay spread out.
                    Stream::with_stem(0, stem_hash, lot.ordinal, Purpose::CityFill).next_u64() >> 1
                } else {
                    let d = trig::isqrt(
                        ((lot.x - city.center[0]).pow(2) + (lot.z - city.center[1]).pow(2)) as u128,
                    ) as u64;
                    d * 1_000 / city.radius as u64
                };
                ((lot.city, lot.house, rank, lot.ordinal), n as u32)
            })
            .collect();
        keyed.sort_unstable_by_key(|k| k.0);
        let mut taken = vec![0usize; cities.len()];
        keyed.retain(|&((city, ..), _)| {
            taken[city] += 1;
            taken[city] <= share[city]
        });
        // Back in lot order: the lots are in ordinal order already.
        let mut kept: Vec<u32> = keyed.into_iter().map(|(_, n)| n).collect();
        kept.sort_unstable();
        lots = kept.into_iter().map(|n| lots[n as usize]).collect();
    }
    let variety = Stream::stem(&format!("{stem}:VARIETY"));
    // What a lot may build depends only on its city, its kind, whether it
    // is downtown and its pitch (one per kind): each such list is made once.
    let mut menus = BTreeMap::<(usize, bool, bool, bool), Vec<(Vec<usize>, u64)>>::new();
    let mut out = Vec::with_capacity(lots.len());
    for lot in &lots {
        let mut stream = Stream::with_stem(0, stem_hash, lot.ordinal, Purpose::CityFill);
        let fit = lot.pitch / 2 - LOT_MARGIN_FT;
        let city = &cities[lot.city];
        let d = trig::isqrt(
            ((lot.x - city.center[0]).pow(2) + (lot.z - city.center[1]).pow(2)) as u128,
        ) as i64;
        let downtown = d * 1_000 / city.radius < DOWNTOWN;
        let choices = menus
            .entry((lot.city, lot.core, downtown, lot.house))
            .or_insert_with(|| {
                let min_side = if lot.core { CORE_MIN_SIDE_FT } else { 0 };
                let fits = |class: Class| -> Vec<usize> {
                    kit.pieces
                        .iter()
                        .enumerate()
                        .filter(|(_, p)| {
                            p.class == class
                                && radius(p.half) <= fit
                                && 2 * p.half[0].min(p.half[1]) >= min_side
                        })
                        .map(|(i, _)| i)
                        .collect()
                };
                // Per-city variety: each city builds its towers and
                // apartments from its own seeded half of the kit's, so
                // cities on the same art differ.
                let own = |class: Class, pieces: Vec<usize>| -> Vec<usize> {
                    if !matches!(class, Class::Tower | Class::Apartment) {
                        return pieces;
                    }
                    let kept: Vec<usize> = pieces
                        .iter()
                        .copied()
                        .filter(|&p| {
                            let key = (lot.city as u32).wrapping_mul(1_000_003) ^ p as u32;
                            Stream::with_stem(0, variety, key, Purpose::CityFill)
                                .next_u64()
                                .is_multiple_of(2)
                        })
                        .collect();
                    if kept.is_empty() { pieces } else { kept }
                };
                weights(lot, downtown)
                    .iter()
                    .map(|&(class, w)| (own(class, fits(class)), w))
                    .filter(|(pieces, _)| !pieces.is_empty())
                    .collect()
            });
        let total: u64 = choices.iter().map(|c| c.1).sum();
        if total == 0 {
            continue;
        }
        let mut roll = stream.below(total);
        let pieces = choices
            .iter()
            .find(|(_, w)| {
                let hit = roll < *w;
                if !hit {
                    roll -= *w;
                }
                hit
            })
            .map(|c| &c.0)
            .expect("roll below total weight");
        let index = pieces[stream.below(pieces.len() as u64) as usize];
        let piece = &kit.pieces[index];
        let elongated =
            (piece.half[0] - piece.half[1]).abs() * 10 > piece.half[0].max(piece.half[1]);
        let mut heading = match lot.diagonal {
            Some(bearing) => bearing,
            None if elongated && (piece.half[1] > piece.half[0]) == lot.long_x => 90,
            None => 0,
        };
        heading = trig::wrap(heading + 180 * stream.below(2) as i32);
        let slack = (fit - radius(piece.half)).min(MAX_JITTER_FT);
        let jitter = |stream: &mut Stream| {
            if slack > 0 {
                stream.below(2 * slack as u64 + 1) as i64 - slack
            } else {
                0
            }
        };
        let x = lot.x + jitter(&mut stream);
        let z = lot.z + jitter(&mut stream);
        // Corners and centre off streets, parks and water.
        let blocked = [[0, 0], [-1, -1], [-1, 1], [1, -1], [1, 1]]
            .iter()
            .any(|s| {
                let corner = trig::rotate([s[0] * piece.half[0], s[1] * piece.half[1]], heading);
                map.flags_at(x + corner[0], z + corner[1])
                    & (mask::STREET | mask::GREEN | mask::WATER)
                    != 0
            });
        if blocked {
            continue;
        }
        cities[lot.city].placed += 1;
        cities[lot.city].core += usize::from(lot.core);
        out.push(Instance {
            piece: index as u16,
            x: x as i32,
            z: z as i32,
            heading: heading as i16,
            core: lot.core,
            city: lot.city as u16,
        });
    }
    (cities, out, candidates)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::kit::{Kit, Piece};
    use std::sync::Arc;

    fn piece(half: [i64; 2], height: i64, class: Class) -> Piece {
        Piece {
            source: "T.OT".into(),
            shape: "T.SH".into(),
            faces: Vec::new(),
            origin: [0.; 3],
            scale: 1.,
            half,
            height,
            class,
            cut: false,
        }
    }

    /// One city tile at grid (0, 0): a 64 by 64 texel light roof in the
    /// north-west quarter, a scatter of painted houses in the south-east, a
    /// street along row 128 and dark ground elsewhere.
    fn map() -> LightMap {
        let n = mask::TEXELS;
        let mut pixels = vec![206u8; n * n];
        for r in 16..80 {
            for c in 16..80 {
                pixels[r * n + c] = 199;
            }
        }
        for r in (140..240).step_by(2) {
            for c in (140..240).step_by(2) {
                pixels[r * n + c] = 200;
                pixels[r * n + c + 1] = 199;
                pixels[(r + 1) * n + c] = 201;
            }
        }
        for c in 0..n {
            pixels[128 * n + c] = 201;
        }
        let mut map = LightMap::default();
        map.tiles.insert(
            (0, 0),
            mask::PlacedTile {
                texture: "T.PIC".into(),
                rotation: 1,
                mask: Arc::new(mask::TileMask::classify(&pixels, &mask::Classes::ukraine())),
            },
        );
        map
    }

    fn kit() -> Kit {
        Kit {
            pieces: vec![
                piece([21, 40], 44, Class::House),
                piece([30, 35], 70, Class::LowRise),
                piece([40, 40], 150, Class::Apartment),
                piece([50, 50], 240, Class::Tower),
                piece([60, 120], 40, Class::Industrial),
            ],
        }
    }

    #[test]
    fn the_fill_is_the_same_every_time_and_keeps_out_of_objects() {
        let (map, kit) = (map(), kit());
        let (cities, a, lots) = place(&map, &kit, "TEST.MM", &[], &[], MAX_INSTANCES);
        let (_, b, _) = place(&map, &kit, "TEST.MM", &[], &[], MAX_INSTANCES);
        assert_eq!(a, b);
        assert!(!a.is_empty() && a.len() <= lots && lots <= MAX_INSTANCES);
        assert_eq!(cities.len(), 1);
        assert_eq!(cities[0].placed, a.len());
        // Core lots stand on the big roof and build tall.
        assert!(
            a.iter()
                .any(|i| i.core && kit.pieces[usize::from(i.piece)].class == Class::Tower)
        );
        // Another layout name draws other buildings on the same lots.
        let (_, c, _) = place(&map, &kit, "OTHER.MM", &[], &[], MAX_INSTANCES);
        assert_ne!(a, c);
        // Nothing inside an object's box and its clearance.
        let keepout = Keepout {
            center: [i64::from(a[0].x), i64::from(a[0].z)],
            half: [400, 400],
            heading: 30,
            clearance: OBJECT_CLEARANCE_FT,
        };
        let (_, d, _) = place(&map, &kit, "TEST.MM", &[keepout], &[], MAX_INSTANCES);
        assert!(d.len() < a.len());
        assert!(
            d.iter()
                .all(|i| !keepout.reaches([i64::from(i.x), i64::from(i.z)], 0))
        );
        // Every building stands off the street row and inside the tile.
        for i in &a {
            let f = map.flags_at(i64::from(i.x), i64::from(i.z));
            assert_eq!(f & mask::STREET, 0);
            assert!((0..mask::TILE_FT).contains(&i64::from(i.x)));
        }
    }

    /// A small fixed generator for the exactness tests.
    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }
        fn below(&mut self, n: u64) -> u64 {
            self.next() % n
        }
    }

    /// A city picture of 8 by 8 texel patches: light roofs, dark ground,
    /// light and dark street lines (along the axes and the diagonals),
    /// painted house dots, parks and water.
    fn picture(rng: &mut Rng) -> Vec<u8> {
        let n = mask::TEXELS;
        let mut p = vec![206u8; n * n];
        for br in 0..n / 8 {
            for bc in 0..n / 8 {
                let kind = rng.below(9);
                for r in br * 8..br * 8 + 8 {
                    for c in bc * 8..bc * 8 + 8 {
                        let (lr, lc) = (r % 8, c % 8);
                        p[r * n + c] = match kind {
                            0 | 1 => 199,
                            2 if lr == 3 => 201,
                            3 if lc == 4 || lc == 5 => 205,
                            4 if lr == lc => 201,
                            5 if lr + lc == 7 => 206,
                            6 if (lr + lc) % 2 == 0 => 200,
                            7 => 194,
                            8 if lr < 4 => 255,
                            _ => 206,
                        };
                    }
                }
            }
        }
        // Long streets right across, so runs reach their minimum.
        for _ in 0..6 {
            let at = rng.below(n as u64) as usize;
            for c in 0..n {
                p[at * n + c] = 201;
            }
            let at = rng.below(n as u64) as usize;
            for r in 0..n {
                p[r * n + at] = 204;
            }
        }
        p
    }

    /// A theater of touching and separate city tiles in every quarter turn,
    /// classified pictures and raw random flags, some tiles sharing a mask.
    fn random_map(rng: &mut Rng) -> LightMap {
        let classes = mask::Classes::ukraine();
        let shared = Arc::new(mask::TileMask::classify(&picture(rng), &classes));
        let mut map = LightMap::default();
        for (n, key) in [(0, 0), (4, 0), (0, 4), (4, 4), (12, -4), (-8, 8), (16, 16)]
            .into_iter()
            .enumerate()
        {
            let mask = match n % 3 {
                0 => Arc::clone(&shared),
                1 => Arc::new(mask::TileMask::classify(&picture(rng), &classes)),
                _ => Arc::new(mask::TileMask {
                    flags: (0..mask::TEXELS * mask::TEXELS)
                        .map(|_| {
                            let f = rng.below(128) as u8;
                            // Mostly sites, so lots pass.
                            if rng.below(3) == 0 {
                                f
                            } else {
                                f | mask::SITE | mask::SOLID
                            }
                        })
                        .collect(),
                }),
            };
            map.tiles.insert(
                key,
                mask::PlacedTile {
                    texture: format!("T{n}.PIC"),
                    rotation: rng.below(4) as u8,
                    mask,
                },
            );
        }
        map
    }

    fn random_kit(rng: &mut Rng) -> Kit {
        let classes = [
            Class::House,
            Class::Industrial,
            Class::LowRise,
            Class::Apartment,
            Class::Tower,
        ];
        Kit {
            pieces: (0..24)
                .map(|n| {
                    piece(
                        [12 + rng.below(70) as i64, 12 + rng.below(70) as i64],
                        20 + rng.below(300) as i64,
                        classes[n % 5],
                    )
                })
                .collect(),
        }
    }

    /// The reworked placement gives exactly what the old one gave: every
    /// building, every city and the candidate count, with and without the
    /// budget thinning, keep-outs at any heading, and named placements.
    #[test]
    fn the_placement_is_the_one_it_was_before_s12() {
        for seed in 0..6 {
            let mut rng = Rng(seed);
            let (map, kit) = (random_map(&mut rng), random_kit(&mut rng));
            let keepouts: Vec<Keepout> = (0..40)
                .map(|_| Keepout {
                    center: [
                        rng.below(6 * mask::TILE_FT as u64) as i64 - 2 * mask::TILE_FT,
                        rng.below(6 * mask::TILE_FT as u64) as i64 - 2 * mask::TILE_FT,
                    ],
                    half: [rng.below(3_000) as i64, rng.below(600) as i64],
                    heading: rng.below(360) as i32,
                    clearance: [OBJECT_CLEARANCE_FT, RUNWAY_CLEARANCE_FT][rng.below(2) as usize],
                })
                .collect();
            let names = vec![
                ([10_000, 10_000], "Alpha".to_owned()),
                ([40_000, 40_000], "Alpha".to_owned()),
                ([500_000, 0], "Far".to_owned()),
            ];
            let full = place(&map, &kit, "TEST.MM", &keepouts, &names, MAX_INSTANCES);
            let lots = full.2;
            assert!(
                full.1.len() > 1_000,
                "seed {seed}: {} buildings",
                full.1.len()
            );
            for cap in [MAX_INSTANCES, lots, lots - 1, lots / 2, lots / 7, 100, 0] {
                let new = place(&map, &kit, "TEST.MM", &keepouts, &names, cap);
                let old = super::super::fill_reference::place(
                    &map, &kit, "TEST.MM", &keepouts, &names, cap,
                );
                assert_eq!(new, old, "seed {seed} cap {cap}");
            }
        }
    }

    /// The keep-out index finds every keep-out a lot reaches.
    #[test]
    fn the_keepout_index_misses_nothing() {
        // The integer turn never shrinks a distance below a half.
        for d in 0..360 {
            let (s, c) = (trig::sin(d), trig::cos(d));
            assert!(2 * (s * s + c * c) >= trig::ONE * trig::ONE, "{d}");
        }
        let mut rng = Rng(9);
        let keepouts: Vec<Keepout> = (0..200)
            .map(|_| Keepout {
                center: [
                    rng.below(40_000) as i64 - 20_000,
                    rng.below(40_000) as i64 - 20_000,
                ],
                half: [rng.below(2_000) as i64, rng.below(2_000) as i64],
                heading: rng.below(360) as i32,
                clearance: rng.below(400) as i64,
            })
            .collect();
        let index = Keepouts::new(&keepouts, 102);
        for _ in 0..20_000 {
            let point = [
                rng.below(50_000) as i64 - 25_000,
                rng.below(50_000) as i64 - 25_000,
            ];
            let r = [64, 102][rng.below(2) as usize];
            assert_eq!(
                index.reach(point, r),
                keepouts.iter().any(|k| k.reaches(point, r)),
                "{point:?} {r}"
            );
        }
    }

    #[test]
    fn the_budget_is_shared_by_city_size() {
        let city = |tiles: usize, candidates: usize| City {
            name: String::new(),
            tiles: vec![(0, 0); tiles],
            center: [0, 0],
            radius: 1,
            candidates,
            placed: 0,
            core: 0,
        };
        // A small city with few lots keeps them all; the rest is shared by
        // tiles.
        let cities = [city(9, 1_000), city(1, 10), city(3, 1_000)];
        assert_eq!(shares(&cities, 130), vec![90, 10, 30]);
        let shared = shares(&cities, 101);
        assert_eq!(shared.iter().sum::<usize>(), 101);
        // Enough for everyone: every lot.
        assert_eq!(shares(&cities, 5_000), vec![1_000, 10, 1_000]);
    }
}
