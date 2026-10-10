//! From plans to paved patches: runway marking bands, taxiway legs, aprons
//! and corner fillets, cut so no two overlap, each with the texture grid its
//! material repeats on.
use super::geometry::{
    Point, Poly, add, band, bounds, ccw, contains, cross, direction, dot, minus_all, overlap_area,
    rect, right_of, scale, signed_area, sub, unit,
};
use super::plan::{Along, Material, Numbers, Plan, RunwaySpec};

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
/// axes of the runway frame).
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
        let to_grid = |p: Point| {
            let d = sub(p, g.origin);
            [dot(d, g.u) / g.cell[0], dot(d, g.v) / g.cell[1]]
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
    /// A taxiway leg: start, end, half width, plan index, taxiway index.
    leg: Option<(Point, Point, f64, usize, usize)>,
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
    let raw: Vec<(Poly, Option<usize>)> = runways
        .iter()
        .chain(&taxiways)
        .chain(&aprons)
        .map(|e| (e.poly.clone(), e.leg.map(|l| l.4)))
        .collect();
    let mut covered: Vec<Poly> = Vec::new();
    let mut patches = Vec::new();
    for element in runways.iter().chain(&taxiways).chain(&aprons) {
        let near: Vec<Poly> = covered
            .iter()
            .filter(|c| overlap_area(&element.poly, c) > super::geometry::SLIVER_AREA)
            .cloned()
            .collect();
        for piece in minus_all(&element.poly, &near) {
            patches.push(Patch {
                poly: piece,
                material: element.material,
                grid: element.grid.clone(),
            });
        }
        covered.push(element.poly.clone());
    }
    // Curved corners where a taxiway meets another element square on.
    let mut fillets: Vec<Poly> = Vec::new();
    for element in &taxiways {
        let Some((a, b, half, part, id)) = element.leg else {
            continue;
        };
        let Some(fillet) = mat(part, "fillet") else {
            continue;
        };
        for (end, other) in [(b, a), (a, b)] {
            let d = unit(sub(end, other));
            for (target, owner) in &raw {
                if *owner == Some(id) || !contains(target, end) {
                    continue;
                }
                let Some((edge_a, edge_b)) = crossed_edge(target, other, end) else {
                    continue;
                };
                let e = unit(sub(edge_b, edge_a));
                if dot(e, d).abs() > 0.02 {
                    continue;
                }
                let n = right_of(d);
                let side = (FILLET_SHARE * 2. * half).max(20.);
                for s in [1., -1.] {
                    let start = add(other, scale(n, s * half));
                    let Some(c) = line_hit(start, d, edge_a, e) else {
                        continue;
                    };
                    let square = ccw(vec![
                        c,
                        add(c, scale(n, s * side)),
                        sub(add(c, scale(n, s * side)), scale(d, side)),
                        sub(c, scale(d, side)),
                    ]);
                    let clear = raw
                        .iter()
                        .map(|(p, _)| p)
                        .chain(&fillets)
                        .all(|p| overlap_area(&square, p) < 1.);
                    if !clear {
                        continue;
                    }
                    patches.push(Patch {
                        poly: square.clone(),
                        material: fillet,
                        grid: Grid {
                            origin: sub(add(c, scale(n, s * side)), scale(d, side)),
                            u: d,
                            v: scale(n, -s),
                            cell: [side, side],
                        },
                    });
                    fillets.push(square);
                }
                break;
            }
        }
    }
    (materials, patches)
}

/// The edge of convex `poly` that the segment from `outside` to `inside`
/// crosses, if any.
fn crossed_edge(poly: &[Point], outside: Point, inside: Point) -> Option<(Point, Point)> {
    let n = poly.len();
    let d = sub(inside, outside);
    let mut best: Option<(f64, Point, Point)> = None;
    for i in 0..n {
        let (p, q) = (poly[i], poly[(i + 1) % n]);
        let e = sub(q, p);
        let denom = cross(d, e);
        if denom.abs() < 1e-9 {
            continue;
        }
        let t = cross(sub(p, outside), e) / denom;
        let s = cross(sub(p, outside), d) / denom;
        if (0. ..=1.).contains(&t) && (-1e-6..=1. + 1e-6).contains(&s) {
            // The last crossing before `inside` is where the leg enters.
            if best.is_none_or(|(bt, ..)| t > bt) {
                best = Some((t, p, q));
            }
        }
    }
    best.map(|(_, p, q)| (p, q))
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
    for leg in points.windows(2) {
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
            leg: Some((a, b, half, part, id)),
        });
    }
}
