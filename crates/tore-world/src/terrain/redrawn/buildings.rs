//! The building move rule and the extras rule of a redrawn airport.
//!
//! Every retail building that belongs to the airfield (inside its retail
//! shape's footprint grown by [`ZONE_FT`], and nearer to one of its strips
//! than to any other) moves to the plan's building line nearest to where
//! retail stood it, keeping its retail heading and its order along the line,
//! backed onto the line with [`GAP_OUT_FT`] to spare and [`GAP_ALONG_FT`]
//! between neighbours. What is left of each line takes extra hangars, and
//! fuel trucks stand by the parking row. All `fitted`, agent rules of
//! 2026-10-10 (John asked for real-size buildings set against the aprons,
//! and for extra hangars and vehicles to fill the gaps).
use super::super::{PlacedSize, Placements};
use super::Frame;
use super::geometry::{Point, add, dot, length, scale, sub, unit};
use super::plan::{Extras, LineSpec};
use crate::resources::ResourceSource;
use tore_formats::mission::{Placement, SourceKey};

pub const ZONE_FT: f64 = 1_500.;
pub const GAP_OUT_FT: f64 = 20.;
pub const GAP_ALONG_FT: f64 = 40.;
/// Ordinal of the first building a plan adds to its layout: its object id is
/// `0x4000_0000` plus this plus a running count, inside the layout id range
/// and far above any retail layout's object count.
pub const ADDED_ORDINAL_BASE: u32 = 0x00F0_0000;
const HANGAR: &str = "HANGR.OT";
const BIG_HANGAR: &str = "HANGRB.OT";
const FUEL_TRUCK: &str = "TANKER.NT";
const TRUCK: &str = "TRUCK.NT";
/// Clusters of city buildings are never airport buildings.
const CITY_PREFIXES: [&str; 3] = ["CITY", "CTYBK", "TWNBK"];

/// A strip of the layout: its index, origin and footprint box.
pub struct Strip {
    pub index: usize,
    pub origin: [f64; 3],
    pub center: [f64; 3],
    pub half: [f64; 3],
    pub right: [f64; 3],
    pub forward: [f64; 3],
}

impl Strip {
    fn zone_contains(&self, x: f64, z: f64) -> bool {
        let d = [x - self.center[0], z - self.center[2]];
        let a = d[0] * self.right[0] + d[1] * self.right[2];
        let b = d[0] * self.forward[0] + d[1] * self.forward[2];
        a.abs() <= self.half[0] + ZONE_FT && b.abs() <= self.half[2] + ZONE_FT
    }
}

/// Where a placed type's footprint lies about its origin when it faces
/// `heading` degrees: its extent along `dir` and `out` in the frame.
fn footprint(
    sources: &Placements,
    object_type: &str,
    heading: i32,
    frame: &Frame,
    dir: Point,
    out: Point,
) -> Option<[f64; 4]> {
    let probe = Placement {
        key: SourceKey {
            layout: String::new(),
            ordinal: 0,
        },
        section: None,
        object_type: object_type.to_owned(),
        position: [0, 0, 0],
        angles: [heading, 0, 0],
        source_nationality: None,
        nationality2: false,
        nationality3: false,
        nationality: None,
        flags: None,
        speed: None,
        name: None,
        alias: None,
        unknown: Vec::new(),
    };
    let stance = sources.stance(&probe, 0.)?;
    let mut range = [
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
    ];
    for x in [stance.min[0], stance.max[0]] {
        for z in [stance.min[2], stance.max[2]] {
            let w = [
                stance.basis.right[0] * x + stance.basis.forward[0] * z,
                stance.basis.right[2] * x + stance.basis.forward[2] * z,
            ];
            let local = frame.local_vector(w);
            let (a, o) = (dot(local, dir), dot(local, out));
            range = [
                range[0].min(a),
                range[1].max(a),
                range[2].min(o),
                range[3].max(o),
            ];
        }
    }
    Some(range)
}

struct Line {
    from: Point,
    dir: Point,
    out: Point,
    length: f64,
    cursor: f64,
}

impl Line {
    fn new(spec: &LineSpec) -> Self {
        let d = sub(spec.to, spec.from);
        Self {
            from: spec.from,
            dir: unit(d),
            out: spec.out,
            length: length(d),
            cursor: 0.,
        }
    }
    fn distance(&self, p: Point) -> f64 {
        let t = dot(sub(p, self.from), self.dir).clamp(0., self.length);
        length(sub(p, add(self.from, scale(self.dir, t))))
    }
    /// Takes room for `print` and gives the origin that puts it there.
    fn take(&mut self, print: [f64; 4]) -> Option<Point> {
        let start = if self.cursor == 0. {
            GAP_ALONG_FT
        } else {
            self.cursor
        };
        let end = start + (print[1] - print[0]);
        if end > self.length {
            return None;
        }
        self.cursor = end + GAP_ALONG_FT;
        Some(add(
            self.from,
            add(
                scale(self.dir, start - print[0]),
                scale(self.out, GAP_OUT_FT - print[2]),
            ),
        ))
    }
}

fn whole_degrees(v: [f64; 2]) -> i32 {
    let h = v[0].atan2(v[1]).to_degrees().round() as i32;
    (h + 180).rem_euclid(360) - 180
}

/// What the rule did.
#[derive(Default)]
pub struct Relaid {
    pub moved: Vec<(usize, Placement)>,
    pub added: Vec<Placement>,
}

#[allow(clippy::too_many_arguments)]
pub fn relay(
    resources: &dyn ResourceSource,
    sources: &mut Placements,
    frame: &Frame,
    own: &[usize],
    strips: &[Strip],
    lines: &[LineSpec],
    extras: &Extras,
    parking: Option<([[f64; 3]; 9], f64)>,
    taken: &mut std::collections::BTreeSet<usize>,
    added_count: &mut u32,
) -> Relaid {
    let mut out = Relaid::default();
    if lines.is_empty() {
        return out;
    }
    let mut lines: Vec<Line> = lines.iter().map(Line::new).collect();
    let own_strips: Vec<&Strip> = strips.iter().filter(|s| own.contains(&s.index)).collect();
    // The airfield's buildings, in layout order.
    let mut movable = Vec::new();
    for (index, placement) in sources.layout.placements.iter().enumerate() {
        if taken.contains(&index) {
            continue;
        }
        let Some(definition) = sources.definitions.get(&placement.object_type) else {
            continue;
        };
        let upper = placement.object_type.to_ascii_uppercase();
        if definition.callbacks.iter().any(|c| c == "_STRIPProc")
            || PlacedSize::of(definition) == PlacedSize::MapTied
            || !upper.ends_with(".OT")
            || CITY_PREFIXES.iter().any(|p| upper.starts_with(p))
            || !sources.shapes.contains_key(&placement.object_type)
        {
            continue;
        }
        let (x, z) = (
            f64::from(placement.position[0]),
            f64::from(placement.position[2]),
        );
        if !own_strips.iter().any(|s| s.zone_contains(x, z)) {
            continue;
        }
        let distance = |s: &Strip| (s.origin[0] - x).hypot(s.origin[2] - z);
        let nearest = strips
            .iter()
            .min_by(|a, b| distance(a).total_cmp(&distance(b)))
            .map(|s| s.index);
        if nearest.is_none_or(|n| !own.contains(&n)) {
            continue;
        }
        movable.push((index, frame.local([x, 0., z])));
    }
    // Each to its nearest line, in retail order along it.
    let mut queue: Vec<(usize, usize, f64)> = movable
        .iter()
        .map(|(index, p)| {
            let line = (0..lines.len())
                .min_by(|a, b| lines[*a].distance(*p).total_cmp(&lines[*b].distance(*p)))
                .expect("lines");
            (
                *index,
                line,
                dot(sub(*p, lines[line].from), lines[line].dir),
            )
        })
        .collect();
    queue.sort_by(|a, b| a.1.cmp(&b.1).then(a.2.total_cmp(&b.2)).then(a.0.cmp(&b.0)));
    for (index, first, _) in queue {
        let placement = sources.layout.placements[index].clone();
        let mut order: Vec<usize> = (0..lines.len()).collect();
        let at = movable
            .iter()
            .find(|m| m.0 == index)
            .map(|m| m.1)
            .expect("movable");
        order.sort_by(|a, b| {
            (*a != first)
                .cmp(&(*b != first))
                .then(lines[*a].distance(at).total_cmp(&lines[*b].distance(at)))
        });
        for line in order {
            let (dir, outward) = (lines[line].dir, lines[line].out);
            let Some(print) = footprint(
                sources,
                &placement.object_type,
                placement.angles[0],
                frame,
                dir,
                outward,
            ) else {
                break;
            };
            if let Some(origin) = lines[line].take(print) {
                let world = frame.world(origin);
                let mut moved = placement.clone();
                moved.position = [
                    world[0].round() as i32,
                    placement.position[1],
                    world[2].round() as i32,
                ];
                out.moved.push((index, moved));
                taken.insert(index);
                break;
            }
        }
    }
    let template = out.moved.first().map(|(_, p)| p.clone());
    let base = template
        .clone()
        .unwrap_or_else(|| sources.layout.placements[own[0]].clone());
    let mut place_new =
        |sources: &mut Placements, object_type: &str, position: [f64; 3], heading: i32| -> bool {
            if resources.get(object_type).is_none()
                || sources
                    .add_type(resources, "redrawn airport", object_type)
                    .is_err()
                || !sources.shapes.contains_key(object_type)
            {
                return false;
            }
            out.added.push(Placement {
                key: SourceKey {
                    layout: base.key.layout.clone(),
                    ordinal: ADDED_ORDINAL_BASE + *added_count,
                },
                section: base.section.clone(),
                object_type: object_type.to_owned(),
                position: [position[0].round() as i32, 0, position[2].round() as i32],
                angles: [heading, 0, 0],
                source_nationality: base.source_nationality,
                nationality2: base.nationality2,
                nationality3: base.nationality3,
                nationality: base.nationality,
                flags: template.as_ref().and_then(|t| t.flags),
                speed: Some(0),
                name: None,
                alias: None,
                unknown: Vec::new(),
            });
            *added_count += 1;
            true
        };
    // Extra hangars, every third a big one where it fits, round the lines.
    let mut left = extras.hangars;
    let mut full = vec![false; lines.len()];
    let mut made = 0u32;
    while left > 0 && full.iter().any(|f| !f) {
        for line in 0..lines.len() {
            if left == 0 || full[line] {
                continue;
            }
            let heading = whole_degrees(frame.world_vector(lines[line].out));
            let (dir, outward) = (lines[line].dir, lines[line].out);
            let mut placed = false;
            let kinds: &[&str] = if made % 3 == 2 {
                &[BIG_HANGAR, HANGAR]
            } else {
                &[HANGAR]
            };
            for kind in kinds {
                if resources.get(kind).is_none()
                    || sources
                        .add_type(resources, "redrawn airport", kind)
                        .is_err()
                {
                    continue;
                }
                let Some(print) = footprint(sources, kind, heading, frame, dir, outward) else {
                    continue;
                };
                if let Some(origin) = lines[line].take(print) {
                    placed = place_new(sources, kind, frame.world(origin), heading);
                    break;
                }
            }
            if placed {
                left -= 1;
                made += 1;
            } else {
                full[line] = true;
            }
        }
    }
    if extras.vehicles {
        if let Some((slots, parked)) = parking {
            let nose = [parked.sin(), parked.cos()];
            for k in [0, 3, 6] {
                let (a, b) = (slots[k], slots[k + 1]);
                let mid = [
                    (a[0] + b[0]) * 0.5 - nose[0] * 48.,
                    0.,
                    (a[2] + b[2]) * 0.5 - nose[1] * 48.,
                ];
                let heading = whole_degrees([b[0] - a[0], b[2] - a[2]]);
                place_new(sources, FUEL_TRUCK, mid, heading);
            }
        }
        for line in &lines {
            let p = add(line.from, add(scale(line.dir, 30.), scale(line.out, -70.)));
            place_new(
                sources,
                TRUCK,
                frame.world(p),
                whole_degrees(frame.world_vector(line.dir)),
            );
        }
    }
    out
}
