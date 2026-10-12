//! The audit of a redrawn airport for the AI (slice CL-AP, 2026-10-11, for
//! John's "ensure AI actually taxi on taxiways instead of grass"): every
//! ground route the AI's airfield points give it, walked against the drawn
//! pavement; every runway end reachable over pavement from parking; and the
//! runway object's line (the ILS localizer and glide path) on a drawn
//! runway. `--airfield-audit` runs it over every theater; the plan tests run
//! it over every plan. An overhead SVG shows a field with its routes.
//! See docs/formats/redrawn-airports.md, "Audit".
use super::geometry::{self, Point};
use super::{Built, DrawnRunway};
use tore_sim::ai::airfield::{AirfieldAnchors, PARKING_SLOTS};
use tore_sim::airport::{ApproachEnd, Runway};

/// Route samples are this far apart along a leg, feet.
pub const STEP_FT: f64 = 5.;
/// Half the main-wheel track the audit keeps on pavement either side of a
/// route's centre line, feet (`fitted`, agent: a fighter's main gear is
/// about 10 to 12 ft across; a heavy's more, and it rides wider pavement).
pub const HALF_TRACK_FT: f64 = 6.;
/// A runway object's line counts as on a drawn runway within this many feet
/// of its centre line and this many degrees of its heading.
pub const LINE_OFFSET_FT: f64 = 2.;
pub const LINE_HEADING_DEG: f64 = 0.05;
/// Two runways a pair's plan says continue each other must lie on one line
/// within this lateral offset (feet) and heading difference (degrees), and
/// their paved ends must not leave a gap wider than [`JOIN_GAP_FT`]
/// (`fitted`, agent decision 2026-10-12, after John's Spassk Dalniy report
/// of a visible sideways step halfway down a paired runway). The layouts
/// place a tile 150 to 560 ft into its base's pavement, the 220 ft overrun
/// pads included, which is one continuous surface, so an overlap is
/// reported but not a problem.
pub const ALIGN_OFFSET_FT: f64 = 10.;
pub const ALIGN_HEADING_DEG: f64 = 0.5;
pub const JOIN_GAP_FT: f64 = 10.;

/// Where the AI stops on a route (the first taxi-out point, every taxi-in
/// point) it runs on this far before its turn to the next leg is done: about
/// 12 ft of stopping past the point and a turning radius of about 30 ft at
/// creeping speed, measured in the CL-AP probes (`fitted`, agent). The
/// audit keeps this run-out along the arriving leg on pavement.
pub const RUN_OUT_FT: f64 = 45.;

/// One AI ground route in world `[x, z]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Route {
    pub name: String,
    pub points: Vec<[f64; 2]>,
    /// Indices of the points the AI stops at before turning.
    pub stops: Vec<usize>,
}

fn xz(p: [f64; 3]) -> [f64; 2] {
    [p[0], p[2]]
}

/// The ground routes the AI's points give a runway object: from every
/// parking slot out along the taxi-out legs to the takeoff spot; the
/// takeoff roll to the far threshold; the landing from the aim point along
/// the runway to the rollout end and in along the taxi-in legs to every
/// parking slot.
pub fn routes(anchors: &AirfieldAnchors, runway: &Runway) -> Vec<Route> {
    let mut out = Vec::new();
    let taxi_out: Vec<_> = anchors.taxi_out.iter().map(|p| xz(*p)).collect();
    let taxi_in: Vec<_> = anchors.taxi_in.iter().map(|p| xz(*p)).collect();
    for slot in 0..PARKING_SLOTS as usize {
        let mut points = vec![xz(anchors.parking[slot])];
        points.extend(&taxi_out);
        points.push(xz(anchors.takeoff_spot));
        out.push(Route {
            name: format!("out from slot {}", slot + 1),
            points,
            stops: vec![1],
        });
    }
    let far = runway.threshold(ApproachEnd::Far);
    out.push(Route {
        name: "takeoff roll".into(),
        points: vec![xz(anchors.takeoff_spot), xz(far)],
        stops: Vec::new(),
    });
    let landing_end = landing_end(anchors, runway);
    let aim = runway.aim_point(landing_end);
    for slot in 0..PARKING_SLOTS as usize {
        let mut points = vec![xz(aim)];
        points.extend(&taxi_in);
        points.push(xz(anchors.parking[slot]));
        out.push(Route {
            name: format!("in to slot {}", slot + 1),
            points,
            stops: vec![1, 2, 3, 4],
        });
    }
    out
}

/// The runway end the AI lands on: the one whose approach heading is
/// nearest the anchors' landing heading (as the AI's sequence chooses it).
pub fn landing_end(anchors: &AirfieldAnchors, runway: &Runway) -> ApproachEnd {
    let off = |end| {
        let d = (runway.approach_heading(end) - anchors.landing_heading).to_degrees();
        ((d + 180.).rem_euclid(360.) - 180.).abs()
    };
    if off(ApproachEnd::Near) <= off(ApproachEnd::Far) {
        ApproachEnd::Near
    } else {
        ApproachEnd::Far
    }
}

/// Every sample of `route` that leaves the drawn pavement: its centre or a
/// point [`HALF_TRACK_FT`] either side, every [`STEP_FT`] along each leg and
/// along the [`RUN_OUT_FT`] past each stop.
pub fn walk(built: &[Built], route: &Route) -> Vec<[f64; 2]> {
    let mut off = Vec::new();
    let mut legs: Vec<([f64; 2], [f64; 2])> =
        route.points.windows(2).map(|w| (w[0], w[1])).collect();
    for &stop in &route.stops {
        if stop == 0 || stop >= route.points.len() {
            continue;
        }
        let (a, b) = (route.points[stop - 1], route.points[stop]);
        let d = geometry::sub(b, a);
        let length = geometry::length(d);
        if length > 1e-6 {
            legs.push((b, geometry::add(b, geometry::scale(d, RUN_OUT_FT / length))));
        }
    }
    for (a, b) in legs {
        let d = geometry::sub(b, a);
        let length = geometry::length(d);
        if length < 1e-6 {
            continue;
        }
        let along = geometry::scale(d, 1. / length);
        let side = geometry::right_of(along);
        let steps = (length / STEP_FT).ceil() as usize;
        for step in 0..=steps {
            let p = geometry::add(a, geometry::scale(d, step as f64 / steps as f64));
            let on = [-HALF_TRACK_FT, 0., HALF_TRACK_FT].iter().all(|o| {
                let q = geometry::add(p, geometry::scale(side, *o));
                super::on_pavement(built, q[0], q[1])
            });
            if !on {
                off.push(p);
            }
        }
    }
    off
}

/// Pavement pieces that touch (closer than half a foot), grouped: the
/// component of each patch of `built`.
pub fn components(built: &Built) -> Vec<usize> {
    let n = built.patches.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn root(parent: &mut [usize], mut i: usize) -> usize {
        while parent[i] != i {
            parent[i] = parent[parent[i]];
            i = parent[i];
        }
        i
    }
    let boxes: Vec<_> = built
        .patches
        .iter()
        .map(|p| geometry::bounds(std::iter::once(&p.poly)).unwrap_or(([0.; 2], [0.; 2])))
        .collect();
    for i in 0..n {
        for j in i + 1..n {
            let (a, b) = (boxes[i], boxes[j]);
            if a.0[0] > b.1[0] + 1.
                || b.0[0] > a.1[0] + 1.
                || a.0[1] > b.1[1] + 1.
                || b.0[1] > a.1[1] + 1.
            {
                continue;
            }
            if touching(&built.patches[i].poly, &built.patches[j].poly) {
                let (ri, rj) = (root(&mut parent, i), root(&mut parent, j));
                parent[ri] = rj;
            }
        }
    }
    (0..n).map(|i| root(&mut parent, i)).collect()
}

fn touching(a: &[Point], b: &[Point]) -> bool {
    const GAP: f64 = 0.5;
    let near = |p: Point, poly: &[Point]| {
        geometry::contains(poly, p)
            || (0..poly.len())
                .any(|k| segment_distance(p, poly[k], poly[(k + 1) % poly.len()]) <= GAP)
    };
    a.iter().any(|p| near(*p, b)) || b.iter().any(|p| near(*p, a))
}

fn segment_distance(p: Point, a: Point, b: Point) -> f64 {
    let ab = geometry::sub(b, a);
    let l = geometry::dot(ab, ab);
    let t = if l > 0. {
        (geometry::dot(geometry::sub(p, a), ab) / l).clamp(0., 1.)
    } else {
        0.
    };
    geometry::length(geometry::sub(p, geometry::add(a, geometry::scale(ab, t))))
}

/// The pavement component under world `x, z`, if any.
pub fn component_at(built: &Built, components: &[usize], at: [f64; 2]) -> Option<usize> {
    let p = built.frame.local([at[0], 0., at[1]]);
    built
        .patches
        .iter()
        .position(|patch| geometry::contains(&patch.poly, p))
        .map(|i| components[i])
}

/// A point `inset` feet in from each threshold of a drawn runway, in the
/// runway frame: near, then far.
pub fn runway_ends(runway: &DrawnRunway, inset: f64) -> [Point; 2] {
    let d = geometry::direction(runway.heading);
    [
        geometry::add(runway.from, geometry::scale(d, inset)),
        geometry::add(runway.from, geometry::scale(d, runway.length - inset)),
    ]
}

/// How far `p` lies off a drawn runway: across its centre line, past its
/// paved ends, and (scaled up) its heading against `heading`. Zero when
/// `p` is on its centre line inside its paved length at its heading.
fn runway_miss(drawn: &DrawnRunway, p: Point, heading: f64) -> f64 {
    let d = geometry::direction(drawn.heading);
    let side = geometry::right_of(d);
    let turn = ((heading - drawn.heading + 90.).rem_euclid(180.) - 90.).abs();
    let rel = geometry::sub(p, drawn.from);
    let along = geometry::dot(rel, d);
    let across = geometry::dot(rel, side).abs();
    let outside = (-drawn.pad - 1. - along)
        .max(along - drawn.length - drawn.pad - 1.)
        .max(0.);
    let turn_miss = if turn <= LINE_HEADING_DEG {
        0.
    } else {
        turn * 100.
    };
    across.max(outside).max(turn_miss)
}

/// The drawn runway a runway object's line lies on (its name), or why not:
/// both thresholds within [`LINE_OFFSET_FT`] of a drawn runway's centre
/// line, inside its paved length, at its heading within
/// [`LINE_HEADING_DEG`]. A line joining two runways on one centre line (a
/// pair's `joined` tile runway) names both, `A+B`. `pavement` is the
/// airport that draws it.
pub fn line_runway(pavement: &Built, runway: &Runway) -> Result<String, String> {
    let near = pavement.frame.local(runway.threshold(ApproachEnd::Near));
    let far = pavement.frame.local(runway.threshold(ApproachEnd::Far));
    let line = geometry::sub(far, near);
    let heading = line[0].atan2(line[1]).to_degrees();
    let mut best = None::<(f64, String)>;
    for drawn in &pavement.runways {
        let worst = runway_miss(drawn, near, heading).max(runway_miss(drawn, far, heading));
        if worst <= LINE_OFFSET_FT {
            return Ok(drawn.name.clone());
        }
        if best.as_ref().is_none_or(|(w, _)| worst < *w) {
            best = Some((
                worst,
                format!("nearest drawn runway {} misses by {worst:.1}", drawn.name),
            ));
        }
    }
    // Two runways on one centre line, the line's ends one on each, and the
    // pavement between them unbroken.
    for a in &pavement.runways {
        for b in &pavement.runways {
            let on_line = |r: &DrawnRunway, p: Point| runway_miss(r, p, heading) <= LINE_OFFSET_FT;
            if std::ptr::eq(a, b) || !on_line(a, near) || !on_line(b, far) {
                continue;
            }
            let d = geometry::direction(a.heading);
            let b_far = geometry::add(b.from, geometry::scale(d, b.length));
            let collinear = [b.from, b_far].iter().all(|p| {
                geometry::dot(geometry::sub(*p, a.from), geometry::right_of(d)).abs()
                    <= LINE_OFFSET_FT
            });
            let unbroken = (0..=100).all(|k| {
                let p = geometry::add(near, geometry::scale(line, f64::from(k) / 100.));
                let w = pavement.frame.world(p);
                pavement.on_pavement(w[0], w[2])
            });
            if collinear && unbroken {
                return Ok(format!("{}+{}", a.name, b.name));
            }
        }
    }
    Err(best.map_or_else(|| "no drawn runway".into(), |(_, why)| why))
}

/// How a pair's continuing runways lie against each other (see
/// [`alignment`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Alignment {
    /// The most either tile runway end lies off the base runway's centre
    /// line, feet.
    pub offset_ft: f64,
    /// The heading difference of the two runways, degrees (0 to 90, the
    /// runways being undirected).
    pub heading_deg: f64,
    /// The distance between the two runways' paved ends along the base
    /// runway: positive a gap, negative an overlap (the pavements share
    /// that length), feet.
    pub gap_ft: f64,
    /// The same between the runways' thresholds, overrun pads left out.
    pub threshold_gap_ft: f64,
}

impl Alignment {
    /// Why this is not one straight runway, if it is not.
    pub fn problem(&self) -> Option<String> {
        let mut why = Vec::new();
        if self.offset_ft > ALIGN_OFFSET_FT {
            why.push(format!("{:.1} ft off the line", self.offset_ft));
        }
        if self.heading_deg > ALIGN_HEADING_DEG {
            why.push(format!("{:.2} degrees off the heading", self.heading_deg));
        }
        if self.gap_ft > JOIN_GAP_FT {
            why.push(format!("a {:.0} ft gap between the ends", self.gap_ft));
        }
        (!why.is_empty()).then(|| why.join(", "))
    }
}

/// The alignment of the two runways a pair's plan says continue each other
/// (`[pair] continues`), measured on the drawn pavement: `None` for a field
/// with no such pair. Their paved extent counts each runway's overrun pad.
pub fn alignment(pavement: &Built) -> Option<Alignment> {
    let [i, j] = pavement.continues?;
    let (base, tile) = (pavement.runways.get(i)?, pavement.runways.get(j)?);
    let d = geometry::direction(base.heading);
    let side = geometry::right_of(d);
    let dt = geometry::direction(tile.heading);
    let ends = [
        geometry::add(tile.from, geometry::scale(dt, -tile.pad)),
        geometry::add(tile.from, geometry::scale(dt, tile.length + tile.pad)),
    ];
    let across = |p: Point| geometry::dot(geometry::sub(p, base.from), side).abs();
    let along = |p: Point| geometry::dot(geometry::sub(p, base.from), d);
    let (t0, t1) = (
        along(ends[0]).min(along(ends[1])),
        along(ends[0]).max(along(ends[1])),
    );
    let (b0, b1) = (-base.pad, base.length + base.pad);
    let (s0, s1) = (
        along(geometry::add(tile.from, geometry::scale(dt, 0.))),
        along(geometry::add(tile.from, geometry::scale(dt, tile.length))),
    );
    let (s0, s1) = (s0.min(s1), s0.max(s1));
    Some(Alignment {
        threshold_gap_ft: (s0 - base.length).max(0. - s1),
        offset_ft: across(ends[0]).max(across(ends[1])),
        heading_deg: ((tile.heading - base.heading + 90.).rem_euclid(180.) - 90.).abs(),
        gap_ft: (t0 - b1).max(b0 - t1),
    })
}

/// How much of an airfield's drawn pavement lies on water cells (see
/// [`water`]), sampled every [`WATER_STEP_FT`].
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Water {
    /// Samples on a drawn runway's pavement, and how many are on water.
    pub runway: (usize, usize),
    /// Samples on the other pavement (taxiways, aprons, links), and how many
    /// are on water.
    pub other: (usize, usize),
}

/// The spacing of the water samples, feet.
pub const WATER_STEP_FT: f64 = 50.;

/// Samples every drawn patch of `pavement` against `over_water` (world
/// `x, z`), split into runway pavement and the rest.
pub fn water(pavement: &Built, over_water: &dyn Fn(f64, f64) -> bool) -> Water {
    let mut out = Water::default();
    for patch in &pavement.patches {
        let Some((min, max)) = geometry::bounds(std::iter::once(&patch.poly)) else {
            continue;
        };
        let mut x = min[0];
        while x <= max[0] {
            let mut z = min[1];
            while z <= max[1] {
                let p = [x, z];
                if geometry::contains(&patch.poly, p) {
                    let on_runway = pavement.runways.iter().any(|r| {
                        let d = geometry::direction(r.heading);
                        let rel = geometry::sub(p, r.from);
                        let along = geometry::dot(rel, d);
                        let across = geometry::dot(rel, geometry::right_of(d)).abs();
                        across <= r.width * 0.5 && along >= -r.pad && along <= r.length + r.pad
                    });
                    let w = pavement.frame.world(p);
                    let wet = over_water(w[0], w[2]);
                    let bucket = if on_runway {
                        &mut out.runway
                    } else {
                        &mut out.other
                    };
                    bucket.0 += 1;
                    bucket.1 += usize::from(wet);
                }
                z += WATER_STEP_FT;
            }
            x += WATER_STEP_FT;
        }
    }
    out
}

/// What one overlay on [`svg`] draws: a polyline in world `[x, z]`.
#[derive(Clone, Debug, PartialEq)]
pub struct Overlay {
    pub points: Vec<[f64; 2]>,
    pub colour: &'static str,
    pub width: f64,
    pub dashed: bool,
}

/// An overhead picture of a redrawn airport in its runway frame (runway up),
/// pavement by kind, with `overlays` on top and `marks` (world `[x, z]`,
/// colour) as dots.
pub fn svg(
    built: &Built,
    title: &str,
    overlays: &[Overlay],
    marks: &[([f64; 2], &'static str)],
) -> String {
    let Some((min, max)) = geometry::bounds(built.patches.iter().map(|p| &p.poly)) else {
        return String::new();
    };
    let margin = 300.;
    let (x0, z0, x1, z1) = (
        min[0] - margin,
        min[1] - margin,
        max[0] + margin,
        max[1] + margin,
    );
    let scale = 1600. / (x1 - x0).max(z1 - z0);
    let px = |p: Point| [(p[0] - x0) * scale, (z1 - p[1]) * scale];
    let width = (x1 - x0) * scale;
    let height = (z1 - z0) * scale;
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width:.0}\" height=\"{:.0}\" viewBox=\"0 0 {width:.0} {:.0}\">\n<rect width=\"100%\" height=\"100%\" fill=\"#6f8f4e\"/>\n",
        height + 40.,
        height + 40.
    );
    for patch in &built.patches {
        let name = built
            .materials
            .get(patch.material)
            .map_or("", |m| m.name.as_str());
        let fill = if name.starts_with("runway") || name.starts_with("digit") {
            "#3a3a3a"
        } else if name == "apron" {
            "#8a8a84"
        } else {
            "#5c5c58"
        };
        let points: Vec<String> = patch
            .poly
            .iter()
            .map(|p| {
                let q = px(*p);
                format!("{:.1},{:.1}", q[0], q[1])
            })
            .collect();
        s += &format!(
            "<polygon points=\"{}\" fill=\"{fill}\" stroke=\"{fill}\" stroke-width=\"0.6\"/>\n",
            points.join(" ")
        );
    }
    let local = |w: [f64; 2]| px(built.frame.local([w[0], 0., w[1]]));
    for overlay in overlays {
        let points: Vec<String> = overlay
            .points
            .iter()
            .map(|w| {
                let q = local(*w);
                format!("{:.1},{:.1}", q[0], q[1])
            })
            .collect();
        s += &format!(
            "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\"{} stroke-linejoin=\"round\"/>\n",
            points.join(" "),
            overlay.colour,
            overlay.width,
            if overlay.dashed {
                " stroke-dasharray=\"8,6\""
            } else {
                ""
            }
        );
    }
    for (w, colour) in marks {
        let q = local(*w);
        s += &format!(
            "<circle cx=\"{:.1}\" cy=\"{:.1}\" r=\"3\" fill=\"{colour}\"/>\n",
            q[0], q[1]
        );
    }
    let bar = 1000. * scale;
    s += &format!(
        "<rect x=\"10\" y=\"{:.0}\" width=\"{bar:.1}\" height=\"6\" fill=\"white\"/><text x=\"{:.0}\" y=\"{:.0}\" font-family=\"sans-serif\" font-size=\"14\" fill=\"white\">1,000 ft</text>\n<text x=\"10\" y=\"{:.0}\" font-family=\"sans-serif\" font-size=\"16\" fill=\"white\">{}</text>\n</svg>\n",
        height + 8.,
        bar + 18.,
        height + 15.,
        height + 34.,
        title.replace('&', "and").replace('<', "")
    );
    s
}
