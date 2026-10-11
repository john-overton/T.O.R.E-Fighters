//! The lights of a redrawn airport: runway edge, threshold and end lights,
//! an approach light system on the ILS runway's marked ends, PAPI beside the
//! touchdown zone and blue taxiway edge lights. Positions only; the app draws
//! them (`airfield_lights.rs`). All `fitted`, agent choices of 2026-10-10 from
//! the usual real installations (John asked for edge lights, approach light
//! arrows and the four slope lights the same day).
use super::geometry::{Point, add, contains, direction, length, right_of, scale, sub, unit};
use super::layout::Patch;
use super::plan::Plan;
use super::{Frame, Material};

/// Runway edge lights, feet apart and out from the pavement edge.
pub const EDGE_SPACING_FT: f64 = 200.;
const EDGE_OUT_FT: f64 = 3.;
/// Threshold and end bars: lights across the runway, feet apart.
const BAR_SPACING_FT: f64 = 10.;
/// Taxiway edge lights, feet apart and out from the pavement edge.
pub const TAXI_SPACING_FT: f64 = 200.;
const TAXI_OUT_FT: f64 = 3.;
/// PAPI: switching angles from the outermost light inward, degrees, around
/// the 3 degree glide path (John, 2026-10-10); the first light this far from
/// the runway edge and the rest this far apart, abeam the ILS aim point.
pub const PAPI_ANGLES_DEG: [f64; 4] = [2.5, 2.83, 3.17, 3.5];
const PAPI_EDGE_FT: f64 = 50.;
const PAPI_SPACING_FT: f64 = 30.;

/// One row of the approach light system: its distance before the threshold
/// and the lateral offsets (feet, right of the centreline) of its lights.
pub struct ApproachRow {
    pub before_ft: f64,
    pub offsets: &'static [f64],
    pub kind: LightKind,
}

const BARRETTE: &[f64] = &[-7., -3.5, 0., 3.5, 7.];
const CROSSBAR: &[f64] = &[
    -50., -45., -40., -35., -30., -25., -20., -15., 15., 20., 25., 30., 35., 40., 45., 50.,
];
const SIDE_ROWS: &[f64] = &[-37.5, -34., -30.5, 30.5, 34., 37.5];
const CENTRE: &[f64] = &[0.];

/// The approach light system, ALSF-II in outline: a white barrette every
/// 100 ft out to 2,400 ft, a crossbar at 1,000 ft (the arrow's bar), red side
/// barrettes over the inner 1,000 ft, and a flasher on every station from
/// 1,000 ft out, sequenced toward the threshold. One table.
pub fn approach_rows() -> Vec<ApproachRow> {
    let mut rows = Vec::new();
    for station in 1..=24 {
        let before_ft = f64::from(station) * 100.;
        rows.push(ApproachRow {
            before_ft,
            offsets: BARRETTE,
            kind: LightKind::Approach,
        });
        if station <= 10 {
            rows.push(ApproachRow {
                before_ft,
                offsets: SIDE_ROWS,
                kind: LightKind::ApproachSide,
            });
        }
        if station >= 10 {
            rows.push(ApproachRow {
                before_ft,
                offsets: CENTRE,
                kind: LightKind::Flasher,
            });
        }
    }
    rows.push(ApproachRow {
        before_ft: 1_000.,
        offsets: CROSSBAR,
        kind: LightKind::Approach,
    });
    rows
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightKind {
    /// White runway edge light, seen from every side.
    Edge,
    /// Green threshold light, toward the approach.
    Threshold,
    /// Red end light, toward the runway.
    End,
    /// White approach light, toward the approach.
    Approach,
    /// Red approach side-row light, toward the approach.
    ApproachSide,
    /// Sequenced white flasher, toward the approach.
    Flasher,
    /// PAPI light: white above its angle, red below, toward the approach.
    Papi,
    /// Blue taxiway edge light, seen from every side.
    Taxiway,
}

/// One light in world feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    pub position: [f64; 3],
    pub kind: LightKind,
    /// The horizontal world direction `[x, z]` it shines toward; `None` for
    /// lights seen all round.
    pub facing: Option<[f64; 2]>,
    /// A PAPI light's switching angle in degrees, or a flasher's place in
    /// its sequence (0 first, toward 1 last).
    pub param: f64,
}

/// The lights of airport parts laid out as `layout::compose` lays them.
#[allow(clippy::too_many_arguments)]
pub fn lights(
    parts: &[(&Plan, Point)],
    links: Option<&Plan>,
    patches: &[Patch],
    materials: &[Material],
    frame: &Frame,
    height: &dyn Fn(f64, f64) -> f64,
) -> Vec<Light> {
    let plane = frame.origin[1];
    let mut out = Vec::new();
    let world_dir = |d: Point| Some(frame.world_vector(d));
    let at = |p: Point, lift: f64| {
        let w = frame.world(p);
        [w[0], plane + lift, w[2]]
    };
    let unmarked = |k: usize, name: &str, far: bool| {
        k == 0
            && links
                .and_then(|l| l.pair.as_ref())
                .is_some_and(|pair| pair.unmark.iter().any(|(n, f)| n == name && *f == far))
    };
    for (k, (plan, offset)) in parts.iter().enumerate() {
        for (index, runway) in plan.runways.iter().enumerate() {
            let d = direction(runway.heading);
            let n = right_of(d);
            let from = add(runway.from, *offset);
            let half = runway.width * 0.5;
            let point = |t: f64, s: f64| add(from, add(scale(n, t), scale(d, s)));
            let marked = [
                runway.marked[0] && !unmarked(k, &runway.name, false),
                runway.marked[1] && !unmarked(k, &runway.name, true),
            ];
            // Edge lights from threshold to threshold, both sides.
            let count = (runway.length / EDGE_SPACING_FT).round().max(1.) as usize;
            for i in 0..=count {
                let s = runway.length * i as f64 / count as f64;
                for side in [-1., 1.] {
                    out.push(Light {
                        position: at(point(side * (half + EDGE_OUT_FT), s), 1.5),
                        kind: LightKind::Edge,
                        facing: None,
                        param: 0.,
                    });
                }
            }
            // Threshold and end bars at each marked end.
            let bars = (runway.width / BAR_SPACING_FT).floor() as usize;
            for (end, s, approach) in [(0, 0., scale(d, -1.)), (1, runway.length, d)] {
                if !marked[end] {
                    continue;
                }
                for i in 0..=bars {
                    let t = -half + runway.width * i as f64 / bars as f64;
                    let p = at(point(t, s), 1.);
                    out.push(Light {
                        position: p,
                        kind: LightKind::Threshold,
                        facing: world_dir(approach),
                        param: 0.,
                    });
                    out.push(Light {
                        position: p,
                        kind: LightKind::End,
                        facing: world_dir(scale(approach, -1.)),
                        param: 0.,
                    });
                }
                // PAPI on the approach's left, abeam the aim point.
                let along = if end == 0 {
                    tore_sim::airport::AIM_PAST_THRESHOLD_FT
                } else {
                    runway.length - tore_sim::airport::AIM_PAST_THRESHOLD_FT
                };
                let left = if end == 0 { -1. } else { 1. };
                for (i, angle) in PAPI_ANGLES_DEG.iter().enumerate() {
                    // The outermost light switches lowest.
                    let t = left
                        * (half
                            + PAPI_EDGE_FT
                            + PAPI_SPACING_FT * (PAPI_ANGLES_DEG.len() - 1 - i) as f64);
                    out.push(Light {
                        position: at(point(t, along), 2.),
                        kind: LightKind::Papi,
                        facing: world_dir(approach),
                        param: *angle,
                    });
                }
                // The approach lights, on the ILS runway only.
                if k == 0 && index == 0 && plan.ils_runway {
                    let rows = approach_rows();
                    let flashers = rows
                        .iter()
                        .filter(|r| r.kind == LightKind::Flasher)
                        .map(|r| r.before_ft)
                        .fold(0., f64::max);
                    for row in rows {
                        for offset in row.offsets {
                            let lateral = if end == 0 { *offset } else { -offset };
                            let s_row = if end == 0 {
                                -row.before_ft
                            } else {
                                runway.length + row.before_ft
                            };
                            let p = point(lateral, s_row);
                            let w = frame.world(p);
                            let ground = height(w[0], w[2]).max(plane);
                            out.push(Light {
                                position: [w[0], ground + 3., w[2]],
                                kind: row.kind,
                                facing: world_dir(approach),
                                // The sequence runs in from the farthest.
                                param: if row.kind == LightKind::Flasher {
                                    1. - row.before_ft / flashers
                                } else {
                                    0.
                                },
                            });
                        }
                    }
                }
            }
        }
    }
    // Blue taxiway edge lights where the edge borders grass.
    let taxiway_material = materials.iter().position(|m| m.name == "taxiway");
    let mut legs: Vec<(Point, Point, f64)> = Vec::new();
    for (plan, offset) in parts {
        for spec in &plan.taxiways {
            for w in spec.points.windows(2) {
                legs.push((add(w[0], *offset), add(w[1], *offset), spec.width));
            }
        }
    }
    if let Some(links) = links {
        for spec in &links.taxiways {
            for w in spec.points.windows(2) {
                legs.push((w[0], w[1], spec.width));
            }
        }
    }
    if taxiway_material.is_some() {
        for (a, b, width) in legs {
            let d = unit(sub(b, a));
            let n = right_of(d);
            let span = length(sub(b, a));
            let count = (span / TAXI_SPACING_FT).ceil().max(1.) as usize;
            for i in 0..=count {
                let s = span * i as f64 / count as f64;
                for side in [-1., 1.] {
                    let p = add(
                        a,
                        add(scale(d, s), scale(n, side * (width * 0.5 + TAXI_OUT_FT))),
                    );
                    if patches.iter().any(|patch| contains(&patch.poly, p)) {
                        continue;
                    }
                    out.push(Light {
                        position: at(p, 1.),
                        kind: LightKind::Taxiway,
                        facing: None,
                        param: 0.,
                    });
                }
            }
        }
    }
    out
}
