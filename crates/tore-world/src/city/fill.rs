//! Where the fill puts its buildings (CF0 prototype).
//!
//! Each city tile is cut into 2,048 ft blocks (16 by 16 texels) and each
//! block carries two lot grids. Core lots, 10 a side (204.8 ft), stand wholly
//! inside a big light roof: all nine of a 3 by 3 sample grid on solid roof
//! texels. Outer lots, 6 a side (341.3 ft), stand on the smaller light areas
//! (the painted houses and narrow roofs): the centre and most of the samples
//! on roof texels, and no core lot under them, so the grids never overlap. A
//! lot is built when it keeps clear of the layout's objects and runways and
//! the chosen building's corners stay off streets, parks and water. When a
//! theater has more lots than the budget, the lots nearest each city's
//! centre win, core lots first by a margin.
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
/// Lots a side in an outer block: 341.3 ft lots (John's 350 ft).
pub const OUTER_LOTS: i64 = 6;
/// Light roof samples of nine an outer lot needs (fitted).
pub const OUTER_HITS: u32 = 5;
/// An outer lot with this many light samples stands on a narrow roof and
/// builds bigger than houses (fitted).
pub const ROOF_HITS: u32 = 8;
/// The share of a city's radius, in thousandths, inside which core lots
/// favour towers (fitted).
pub const DOWNTOWN: i64 = 350;
/// The smallest footprint side a core lot builds, feet (fitted): big roofs
/// get big buildings.
pub const CORE_MIN_SIDE_FT: i64 = 50;
/// The most buildings a theater gets (John, 2026-10-10).
pub const MAX_INSTANCES: usize = 50_000;
/// Over the budget, lots nearest their city's centre win; a core lot counts
/// as this many thousandths of the city's radius nearer (fitted).
pub const CORE_PRIORITY: i64 = 150;
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
        let b = &object.bounds;
        Self {
            center: [b.center[0].round() as i64, b.center[2].round() as i64],
            half: [b.half[0].ceil() as i64, b.half[2].ceil() as i64],
            heading: b.heading.to_degrees().round() as i32,
            clearance: if object.runway {
                RUNWAY_CLEARANCE_FT
            } else {
                OBJECT_CLEARANCE_FT
            },
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
    hits: u32,
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

fn lots(map: &LightMap, city_of: &BTreeMap<(i32, i32), usize>, keepouts: &[Keepout]) -> Vec<Lot> {
    let cell = mask::TILE_FT / 4;
    let t = mask::TEXEL_FT;
    let mut out = Vec::new();
    let mut ordinal = 0u32;
    let mut core_lots = std::collections::BTreeSet::<(i64, i64)>::new();
    for &key in map.tiles.keys() {
        let origin = [i64::from(key.0) * cell, i64::from(key.1) * cell];
        for bz in 0..mask::TILE_FT / BLOCK_FT {
            for bx in 0..mask::TILE_FT / BLOCK_FT {
                let o = [origin[0] + bx * BLOCK_FT, origin[1] + bz * BLOCK_FT];
                for core in [true, false] {
                    let k = if core { CORE_LOTS } else { OUTER_LOTS };
                    let pitch = BLOCK_FT / k;
                    let step = BLOCK_FT / (3 * k);
                    for j in 0..k {
                        for i in 0..k {
                            let x = o[0] + (2 * i + 1) * BLOCK_FT / (2 * k);
                            let z = o[1] + (2 * j + 1) * BLOCK_FT / (2 * k);
                            if map.flags_at(x, z) & mask::SITE == 0 {
                                continue;
                            }
                            let (mut hits, mut solid) = (0, 0);
                            for sz in -1..=1 {
                                for sx in -1..=1 {
                                    let f = map.flags_at(x + sx * step, z + sz * step);
                                    hits += u32::from(f & mask::SITE != 0);
                                    solid += u32::from(f & mask::SOLID != 0);
                                }
                            }
                            // A core lot's cell on the global core grid, and
                            // the core cells an outer lot covers.
                            let core_cell = |v: i64| (v * CORE_LOTS).div_euclid(BLOCK_FT);
                            let pass = if core {
                                solid == 9
                            } else {
                                let r = pitch / 2;
                                hits >= OUTER_HITS
                                    && (core_cell(z - r)..=core_cell(z + r)).all(|cz| {
                                        (core_cell(x - r)..=core_cell(x + r))
                                            .all(|cx| !core_lots.contains(&(cx, cz)))
                                    })
                            };
                            if !pass || keepouts.iter().any(|k| k.reaches([x, z], pitch / 2)) {
                                continue;
                            }
                            if core {
                                core_lots.insert((core_cell(x), core_cell(z)));
                            }
                            // The roof's longer extent through the lot centre.
                            let run = |dx: i64, dz: i64| {
                                let mut n = 0;
                                for s in [-1, 1] {
                                    for step in 1..=8 {
                                        let f = map
                                            .flags_at(x + s * step * dx * t, z + s * step * dz * t);
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
                            let (mut axis, mut d1, mut d2) = (0, 0, 0);
                            for wz in -7..=7 {
                                for wx in -7..=7 {
                                    let f = map.flags_at(x + wx * t, z + wz * t);
                                    axis += i32::from(f & mask::STREET_AXIS != 0);
                                    d1 += i32::from(f & mask::STREET_D1 != 0);
                                    d2 += i32::from(f & mask::STREET_D2 != 0);
                                }
                            }
                            // D1 runs east and south in the picture: a line
                            // at bearing 135; D2 at bearing 45.
                            let diagonal = (d1 + d2 > axis && d1.max(d2) >= 8)
                                .then_some(if d1 > d2 { 135 } else { 45 });
                            out.push(Lot {
                                x,
                                z,
                                pitch,
                                core,
                                hits,
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

/// What each kind of lot builds: (class, weight) (fitted; towers downtown,
/// houses on the painted house areas).
fn weights(lot: &Lot, downtown: bool) -> &'static [(Class, u64)] {
    match (lot.core, downtown, lot.hits >= ROOF_HITS) {
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
            (Class::House, 70),
            (Class::LowRise, 25),
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
) -> (Vec<City>, Vec<Instance>, usize) {
    let (mut cities, city_of) = cities(map, names);
    let mut lots = lots(map, &city_of, keepouts);
    let candidates = lots.len();
    for lot in &lots {
        cities[lot.city].candidates += 1;
    }
    if lots.len() > MAX_INSTANCES {
        let key = |lot: &Lot| {
            let city = &cities[lot.city];
            let d = trig::isqrt(
                ((lot.x - city.center[0]).pow(2) + (lot.z - city.center[1]).pow(2)) as u128,
            ) as i64;
            (
                d * 1_000 / city.radius - if lot.core { CORE_PRIORITY } else { 0 },
                lot.ordinal,
            )
        };
        lots.sort_by_key(key);
        lots.truncate(MAX_INSTANCES);
        lots.sort_by_key(|lot| lot.ordinal);
    }
    let stem = format!("CITYFILL:{}", layout.trim_end_matches(".MM"));
    let mut out = Vec::with_capacity(lots.len());
    for lot in &lots {
        let mut stream = Stream::new(0, &stem, lot.ordinal, Purpose::CityFill);
        let fit = lot.pitch / 2 - LOT_MARGIN_FT;
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
        let city = &cities[lot.city];
        let d = trig::isqrt(
            ((lot.x - city.center[0]).pow(2) + (lot.z - city.center[1]).pow(2)) as u128,
        ) as i64;
        let downtown = d * 1_000 / city.radius < DOWNTOWN;
        let choices: Vec<(Vec<usize>, u64)> = weights(lot, downtown)
            .iter()
            .map(|&(class, w)| (fits(class), w))
            .filter(|(pieces, _)| !pieces.is_empty())
            .collect();
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
                mask: Arc::new(mask::TileMask::classify(&pixels)),
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
        let (cities, a, lots) = place(&map, &kit, "TEST.MM", &[], &[]);
        let (_, b, _) = place(&map, &kit, "TEST.MM", &[], &[]);
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
        let (_, c, _) = place(&map, &kit, "OTHER.MM", &[], &[]);
        assert_ne!(a, c);
        // Nothing inside an object's box and its clearance.
        let keepout = Keepout {
            center: [i64::from(a[0].x), i64::from(a[0].z)],
            half: [400, 400],
            heading: 30,
            clearance: OBJECT_CLEARANCE_FT,
        };
        let (_, d, _) = place(&map, &kit, "TEST.MM", &[keepout], &[]);
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
}
