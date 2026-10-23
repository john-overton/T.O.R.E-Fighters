//! From plans to paved patches: runway marking bands, taxiway legs, aprons
//! and corner fillets, cut so no two overlap, each with the texture grid its
//! material repeats on.
use super::geometry::{
    Point, Poly, add, band, bounds, ccw, contains, cross, direction, dot, length, minus_all,
    overlap_area, rect, right_of, scale, signed_area, sub, unit,
};
use super::plan::{Along, Material, Numbers, Paint, Plan, RunwaySpec};

/// Runway markings along the runway from each threshold, feet (`fitted`,
/// agent, from the usual real layout).
const THRESHOLD_FT: f64 = 150.;
const NUMBERS_FROM_FT: f64 = 190.;
const NUMBER_HEIGHT_FT: f64 = 60.;
const NUMBER_WIDTH_FT: f64 = 20.;
const NUMBER_GAP_FT: f64 = 10.;
const TOUCHDOWN_FROM_FT: f64 = 300.;
/// A curved corner piece's side, as a share of the taxiway width (`fitted`).
const FILLET_SHARE: f64 = 0.7;

/// How a material repeats over a patch: copies laid on a grid of `cell`
/// feet from `origin`, texture columns along `u` and rows along `v` (unit
/// axes of the runway frame, square to each other except in a fillet laid
/// into an angled corner).
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub origin: Point,
    pub u: Point,
    pub v: Point,
    pub cell: [f64; 2],
}

/// One convex paved piece.
#[derive(Clone, Debug, PartialEq)]
pub struct Patch {
    pub poly: Poly,
    pub material: usize,
    pub grid: Grid,
}

/// One grid cell of a patch: corners in the runway frame, the fraction of
/// the material's texels at each, and the cell's grid index.
pub type Cell = (Poly, Vec<Point>, [i64; 2]);

impl Patch {
    /// The patch cut at its grid lines.
    pub fn cells(&self) -> Vec<Cell> {
        let g = &self.grid;
        // The axes need not be square (a fillet in an angled corner): solve
        // p = origin + u * cell0 * i + v * cell1 * j.
        let (cu, cv) = (scale(g.u, g.cell[0]), scale(g.v, g.cell[1]));
        let det = cross(cu, cv);
        let to_grid = |p: Point| {
            let d = sub(p, g.origin);
            [cross(d, cv) / det, cross(cu, d) / det]
        };
        let from_grid = |q: Point| {
            add(
                g.origin,
                add(scale(g.u, q[0] * g.cell[0]), scale(g.v, q[1] * g.cell[1])),
            )
        };
        let local: Poly = self.poly.iter().map(|p| to_grid(*p)).collect();
        let Some((lo, hi)) = bounds(std::iter::once(&local)) else {
            return Vec::new();
        };
        let local = ccw(local);
        let mut out = Vec::new();
        for i in (lo[0] + 1e-9).floor() as i64..(hi[0] - 1e-9).ceil() as i64 {
            for j in (lo[1] + 1e-9).floor() as i64..(hi[1] - 1e-9).ceil() as i64 {
                let (a, b) = (i as f64, j as f64);
                let square = rect([a, b], [a + 1., b + 1.]);
                let piece = super::geometry::intersect(&local, &square);
                if piece.len() < 3 || signed_area(&piece) < 1e-9 {
                    continue;
                }
                let fractions = piece.iter().map(|q| [q[0] - a, q[1] - b]).collect();
                out.push((
                    piece.iter().map(|q| from_grid(*q)).collect(),
                    fractions,
                    [i, j],
                ));
            }
        }
        out
    }
}

/// A runway end's designator from the approach heading in degrees.
pub fn designator(heading_deg: f64) -> String {
    let n = (heading_deg.rem_euclid(360.) / 10.).round() as i64 % 36;
    if n == 0 { 36 } else { n }.to_string()
}

struct Element {
    poly: Poly,
    material: usize,
    grid: Grid,
    leg: Option<Leg>,
}

/// A taxiway leg: its ends, half width, plan, taxiway and place in it.
#[derive(Clone, Copy)]
struct Leg {
    a: Point,
    b: Point,
    half: f64,
    part: usize,
    taxiway: usize,
    index: usize,
}

/// The patches of one or more plans laid out together in the first plan's
/// frame: each part is a plan and where its own frame's origin lies in that
/// frame. `links` adds taxiways (a pair plan's) drawn with the first plan's
/// taxiway material. `world_heading` is the first frame's heading in
/// degrees, for the designators.
pub fn compose(
    parts: &[(&Plan, Point)],
    links: Option<&Plan>,
    world_heading: f64,
) -> (Vec<Material>, Vec<Patch>) {
    let mut materials = Vec::new();
    let mut bases = Vec::new();
    for (plan, _) in parts {
        bases.push(materials.len());
        materials.extend(plan.materials.iter().cloned());
    }
    let mat = |part: usize, name: &str| parts[part].0.material(name).map(|m| bases[part] + m);
    let mut runways = Vec::new();
    let mut taxiways = Vec::new();
    let mut aprons = Vec::new();
    let mut taxiway_count = 0;
    for (k, (plan, offset)) in parts.iter().enumerate() {
        for runway in &plan.runways {
            // A pair's base runway that runs on into the tile loses that
            // end's markings (only the base's: a tile marks its own).
            let mut runway = runway.clone();
            if k == 0
                && let Some(pair) = links.and_then(|l| l.pair.as_ref())
            {
                for (name, far) in &pair.unmark {
                    if *name == runway.name {
                        runway.marked[usize::from(*far)] = false;
                    }
                }
            }
            runway_elements(
                &runway,
                *offset,
                world_heading,
                &|name| mat(k, name),
                &materials,
                &mut runways,
            );
        }
        let taxiway = mat(k, "taxiway").expect("validated");
        for spec in &plan.taxiways {
            let points: Vec<Point> = spec.points.iter().map(|p| add(*p, *offset)).collect();
            let id = taxiway_count;
            taxiway_count += 1;
            taxiway_elements(
                &points,
                spec.width,
                taxiway,
                &materials[taxiway],
                (k, id),
                &mut taxiways,
            );
        }
        let apron = mat(k, "apron").expect("validated");
        for spec in &plan.aprons {
            let (min, max) = (add(spec.min, *offset), add(spec.max, *offset));
            aprons.push(Element {
                poly: rect(min, max),
                material: apron,
                grid: Grid {
                    origin: [min[0], max[1]],
                    u: [1., 0.],
                    v: [0., -1.],
                    cell: materials[apron].tile_ft,
                },
                leg: None,
            });
        }
    }
    if let Some(links) = links {
        let taxiway = mat(0, "taxiway").expect("validated");
        for spec in &links.taxiways {
            let id = taxiway_count;
            taxiway_count += 1;
            taxiway_elements(
                &spec.points,
                spec.width,
                taxiway,
                &materials[taxiway],
                (0, id),
                &mut taxiways,
            );
        }
    }
    // A junction's mouth: where a later taxiway joins, the earlier one is
    // drawn without its edge lines (its centreline stays).
    let mut mouth_of = std::collections::BTreeMap::new();
    for element in &taxiways {
        if let std::collections::btree_map::Entry::Vacant(slot) = mouth_of.entry(element.material) {
            let mut plain = materials[element.material].clone();
            plain.name = format!("{}_mouth", plain.name);
            for paint in &mut plain.paint {
                if *paint == Paint::Taxiway {
                    *paint = Paint::TaxiwayCentre;
                }
            }
            slot.insert(materials.len());
            materials.push(plain);
        }
    }
    let raw: Vec<Poly> = runways
        .iter()
        .chain(&taxiways)
        .chain(&aprons)
        .map(|e| e.poly.clone())
        .collect();
    let mut covered: Vec<Poly> = Vec::new();
    let mut patches = Vec::new();
    for (i, element) in runways.iter().chain(&taxiways).chain(&aprons).enumerate() {
        let near: Vec<Poly> = covered
            .iter()
            .filter(|c| overlap_area(&element.poly, c) > super::geometry::SLIVER_AREA)
            .cloned()
            .collect();
        let pieces = minus_all(&element.poly, &near);
        let mouths = match element.leg {
            Some(leg) => mouths(&leg, &taxiways[i - runways.len() + 1..]),
            None => Vec::new(),
        };
        for piece in pieces {
            let mut rest = vec![piece];
            for mouth in &mouths {
                let mut next = Vec::new();
                for part in rest {
                    let inside = super::geometry::intersect(&part, mouth);
                    if inside.len() >= 3
                        && signed_area(&inside).abs() > super::geometry::SLIVER_AREA
                    {
                        patches.push(Patch {
                            poly: ccw(inside),
                            material: mouth_of[&element.material],
                            grid: element.grid.clone(),
                        });
                    }
                    next.extend(super::geometry::minus(&part, mouth));
                }
                rest = next;
            }
            for part in rest {
                patches.push(Patch {
                    poly: part,
                    material: element.material,
                    grid: element.grid.clone(),
                });
            }
        }
        covered.push(element.poly.clone());
    }
    // Curved corners wherever a taxiway leg meets or crosses another
    // element's edge, at any angle, and on the inside of a taxiway's bends.
    let mut fillets: Vec<Poly> = Vec::new();
    for (l, element) in taxiways.iter().enumerate() {
        let Some(leg) = element.leg else {
            continue;
        };
        let Some(fillet) = mat(leg.part, "fillet") else {
            continue;
        };
        let d = unit(sub(leg.b, leg.a));
        let n = right_of(d);
        let side = (FILLET_SHARE * 2. * leg.half).max(20.);
        let span = sub(leg.b, leg.a);
        for (t, target) in raw.iter().enumerate() {
            let other = t
                .checked_sub(runways.len())
                .and_then(|k| taxiways.get(k))
                .and_then(|e| e.leg);
            if t == runways.len() + l {
                continue;
            }
            // A bend: only the next leg of the same taxiway, on the inside.
            let bend = other.filter(|o| o.taxiway == leg.taxiway);
            if let Some(o) = bend
                && o.index != leg.index + 1
            {
                continue;
            }
            for (at, p, q) in crossings(target, leg.a, leg.b) {
                let probe = add(leg.a, scale(span, (at + 0.5 / length(span)).min(1.)));
                let entering = contains(target, probe);
                let inward = if entering { d } else { scale(d, -1.) };
                let e = unit(sub(q, p));
                if dot(e, d).abs() > 0.97 {
                    continue;
                }
                let centre = add(leg.a, scale(span, at));
                for s in [1., -1.] {
                    let out = scale(n, s);
                    if let Some(o) = bend
                        && (!entering || dot(out, unit(sub(o.b, o.a))) <= 0.)
                    {
                        continue;
                    }
                    let Some(c) = line_hit(add(centre, scale(out, leg.half)), d, p, e) else {
                        continue;
                    };
                    let along = if dot(e, out) < 0. { scale(e, -1.) } else { e };
                    // The pavement met must run on along the corner's whole
                    // side: not so at the outside of an L where two
                    // taxiways' ends overlap.
                    let beyond = add(add(c, scale(along, side * 0.9)), scale(inward, 2.));
                    let backed = raw
                        .iter()
                        .enumerate()
                        .any(|(r, poly)| r != runways.len() + l && contains(poly, beyond));
                    if !backed {
                        continue;
                    }
                    let quad = ccw(vec![
                        c,
                        add(c, scale(along, side)),
                        sub(add(c, scale(along, side)), scale(inward, side)),
                        sub(c, scale(inward, side)),
                    ]);
                    let clear = raw
                        .iter()
                        .chain(&fillets)
                        .all(|poly| overlap_area(&quad, poly) < 1.);
                    if !clear {
                        continue;
                    }
                    patches.push(Patch {
                        poly: quad.clone(),
                        material: fillet,
                        grid: Grid {
                            origin: sub(add(c, scale(along, side)), scale(inward, side)),
                            u: inward,
                            v: scale(along, -1.),
                            cell: [side, side],
                        },
                    });
                    fillets.push(quad);
                }
            }
        }
    }
    (materials, patches)
}

/// Where segment `a`..`b` crosses the edges of convex `poly`: the share of
/// the way along it and the edge crossed, in order.
fn crossings(poly: &[Point], a: Point, b: Point) -> Vec<(f64, Point, Point)> {
    let n = poly.len();
    let d = sub(b, a);
    let mut out = Vec::new();
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let e = sub(q, p);
        let denom = cross(d, e);
        if denom.abs() < 1e-9 {
            continue;
        }
        let t = cross(sub(p, a), e) / denom;
        let s = cross(sub(p, a), d) / denom;
        if (0. ..=1.).contains(&t) && (-1e-6..=1. + 1e-6).contains(&s) {
            out.push((t, p, q));
        }
    }
    out.sort_by(|x, y| x.0.total_cmp(&y.0));
    out
}

/// The mouths later taxiway legs open in `leg`: their overlap with it,
/// kept to the side they come from when they end inside it, so the far
/// edge line runs on.
fn mouths(leg: &Leg, later: &[Element]) -> Vec<Poly> {
    let d = unit(sub(leg.b, leg.a));
    let n = right_of(d);
    let own = band(leg.a, leg.b, leg.half, leg.half);
    let mut out: Vec<Poly> = Vec::new();
    for element in later {
        let Some(other) = element.leg else {
            continue;
        };
        let mut mouth = super::geometry::intersect(&own, &element.poly);
        if mouth.len() < 3 || signed_area(&mouth).abs() <= super::geometry::SLIVER_AREA {
            continue;
        }
        let offset = |p: Point| dot(sub(p, leg.a), n);
        let inside = |p: Point| offset(p).abs() <= leg.half + 1.;
        let ends = [other.a, other.b];
        if let Some(outside) = match (inside(ends[0]), inside(ends[1])) {
            (true, false) => Some(ends[1]),
            (false, true) => Some(ends[0]),
            _ => None,
        } {
            // Keep the half of the leg facing the joining taxiway.
            let s = offset(outside).signum();
            mouth = super::geometry::clip(&mouth, scale(n, -s), -dot(leg.a, scale(n, s)));
            if mouth.len() < 3 {
                continue;
            }
        }
        for piece in super::geometry::minus_all(&ccw(mouth), &out) {
            out.push(piece);
        }
    }
    out
}

fn line_hit(p: Point, d: Point, q: Point, e: Point) -> Option<Point> {
    let denom = cross(d, e);
    if denom.abs() < 1e-9 {
        return None;
    }
    let t = cross(sub(q, p), e) / denom;
    Some(add(p, scale(d, t)))
}

fn runway_elements(
    runway: &RunwaySpec,
    offset: Point,
    world_heading: f64,
    mat: &dyn Fn(&str) -> Option<usize>,
    materials: &[Material],
    out: &mut Vec<Element>,
) {
    let plain = mat("runway_plain").expect("validated");
    let threshold = mat("runway_threshold").expect("validated");
    let touchdown = mat("runway_touchdown").expect("validated");
    let centreline = mat("runway_centreline").expect("validated");
    let d = direction(runway.heading);
    let n = right_of(d);
    let from = add(runway.from, offset);
    let half = runway.width * 0.5;
    let length = runway.length;
    let at = |t: f64, s: f64| add(from, add(scale(n, t), scale(d, s)));
    let cell = |m: usize| {
        let tile = materials[m].tile_ft;
        [if tile[0] > 0. { tile[0] } else { runway.width }, tile[1]]
    };
    let grid = |m: usize, far: bool| {
        if far {
            Grid {
                origin: at(half, length),
                u: scale(n, -1.),
                v: d,
                cell: cell(m),
            }
        } else {
            Grid {
                origin: at(-half, 0.),
                u: n,
                v: scale(d, -1.),
                cell: cell(m),
            }
        }
    };
    let band = |out: &mut Vec<Element>, m: usize, s0: f64, s1: f64, far: bool| {
        if s1 - s0 < 1e-6 {
            return;
        }
        out.push(Element {
            poly: ccw(vec![
                at(-half, s0),
                at(half, s0),
                at(half, s1),
                at(-half, s1),
            ]),
            material: m,
            grid: grid(m, far),
            leg: None,
        });
    };
    let heading = world_heading + runway.heading;
    let designators = match &runway.numbers {
        Numbers::Auto => Some([designator(heading), designator(heading + 180.)]),
        Numbers::None => None,
        Numbers::Fixed(pair) => Some(pair.clone()),
    };
    let td = TOUCHDOWN_FROM_FT + runway.touchdown;
    let [near_marked, far_marked] = runway.marked;
    let middle_from = if near_marked { td } else { -runway.pad };
    let middle_to = if far_marked {
        length - td
    } else {
        length + runway.pad
    };
    if near_marked {
        band(out, plain, -runway.pad, 0., false);
        band(out, threshold, 0., THRESHOLD_FT, false);
        band(out, plain, THRESHOLD_FT, NUMBERS_FROM_FT, false);
        numbers(
            out,
            runway,
            &at,
            n,
            d,
            designators.as_ref().map(|p| p[0].as_str()),
            NUMBERS_FROM_FT,
            false,
            mat,
            plain,
            cell(plain),
            grid(plain, false),
        );
        band(
            out,
            plain,
            NUMBERS_FROM_FT + NUMBER_HEIGHT_FT,
            TOUCHDOWN_FROM_FT,
            false,
        );
        band(out, touchdown, TOUCHDOWN_FROM_FT, td, false);
    }
    band(out, centreline, middle_from, middle_to, false);
    if far_marked {
        band(
            out,
            touchdown,
            length - td,
            length - TOUCHDOWN_FROM_FT,
            true,
        );
        band(
            out,
            plain,
            length - TOUCHDOWN_FROM_FT,
            length - NUMBERS_FROM_FT - NUMBER_HEIGHT_FT,
            true,
        );
        numbers(
            out,
            runway,
            &at,
            n,
            d,
            designators.as_ref().map(|p| p[1].as_str()),
            length - NUMBERS_FROM_FT - NUMBER_HEIGHT_FT,
            true,
            mat,
            plain,
            cell(plain),
            grid(plain, true),
        );
        band(
            out,
            plain,
            length - NUMBERS_FROM_FT,
            length - THRESHOLD_FT,
            true,
        );
        band(out, threshold, length - THRESHOLD_FT, length, true);
        band(out, plain, length, length + runway.pad, true);
    }
}

/// The designation band from `s0`: the digits centred, plain runway around
/// them, or all plain when a digit has no material.
#[allow(clippy::too_many_arguments)]
fn numbers(
    out: &mut Vec<Element>,
    runway: &RunwaySpec,
    at: &dyn Fn(f64, f64) -> Point,
    n: Point,
    d: Point,
    text: Option<&str>,
    s0: f64,
    far: bool,
    mat: &dyn Fn(&str) -> Option<usize>,
    plain: usize,
    plain_cell: [f64; 2],
    plain_grid: Grid,
) {
    let half = runway.width * 0.5;
    let s1 = s0 + NUMBER_HEIGHT_FT;
    let whole = ccw(vec![
        at(-half, s0),
        at(half, s0),
        at(half, s1),
        at(-half, s1),
    ]);
    let digits: Option<Vec<usize>> =
        text.and_then(|t| t.chars().map(|c| mat(&format!("digit_{c}"))).collect());
    let Some(digits) = digits.filter(|d| !d.is_empty()) else {
        out.push(Element {
            poly: whole,
            material: plain,
            grid: plain_grid,
            leg: None,
        });
        return;
    };
    let count = digits.len() as f64;
    let span = count * NUMBER_WIDTH_FT + (count - 1.) * NUMBER_GAP_FT;
    let mut covered = Vec::new();
    for (i, material) in digits.iter().enumerate() {
        // Reading order is the pilot's left to right: +n near, -n far.
        let offset = -span * 0.5 + i as f64 * (NUMBER_WIDTH_FT + NUMBER_GAP_FT);
        let (a, b) = if far {
            (-offset - NUMBER_WIDTH_FT, -offset)
        } else {
            (offset, offset + NUMBER_WIDTH_FT)
        };
        let poly = ccw(vec![at(a, s0), at(b, s0), at(b, s1), at(a, s1)]);
        let grid = if far {
            Grid {
                origin: at(b, s0),
                u: scale(n, -1.),
                v: d,
                cell: [NUMBER_WIDTH_FT, NUMBER_HEIGHT_FT],
            }
        } else {
            Grid {
                origin: at(a, s1),
                u: n,
                v: scale(d, -1.),
                cell: [NUMBER_WIDTH_FT, NUMBER_HEIGHT_FT],
            }
        };
        covered.push(poly.clone());
        out.push(Element {
            poly,
            material: *material,
            grid,
            leg: None,
        });
    }
    let _ = plain_cell;
    for piece in minus_all(&whole, &covered) {
        out.push(Element {
            poly: piece,
            material: plain,
            grid: plain_grid.clone(),
            leg: None,
        });
    }
}

fn taxiway_elements(
    points: &[Point],
    width: f64,
    material: usize,
    m: &Material,
    (part, id): (usize, usize),
    out: &mut Vec<Element>,
) {
    let half = width * 0.5;
    for (index, leg) in points.windows(2).enumerate() {
        let (a, b) = (leg[0], leg[1]);
        let d = unit(sub(b, a));
        let n = right_of(d);
        let origin = sub(a, scale(n, half));
        let grid = match m.along {
            Along::U => Grid {
                origin,
                u: d,
                v: n,
                cell: [
                    m.tile_ft[0].max(1.),
                    if m.tile_ft[1] > 0. {
                        m.tile_ft[1]
                    } else {
                        width
                    },
                ],
            },
            Along::V => Grid {
                origin,
                u: n,
                v: d,
                cell: [
                    if m.tile_ft[0] > 0. {
                        m.tile_ft[0]
                    } else {
                        width
                    },
                    m.tile_ft[1].max(1.),
                ],
            },
        };
        out.push(Element {
            poly: band(a, b, half, half),
            material,
            grid,
            leg: Some(Leg {
                a,
                b,
                half,
                part,
                taxiway: id,
                index,
            }),
        });
    }
}
