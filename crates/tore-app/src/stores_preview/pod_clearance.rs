//! TOR-13 whole-surface gate for the Tornado IDS (Combat)'s outer wing pods
//! (`TORE_STORES_PREVIEW_PODCLEAR=1`, with `TORE_STORES_PREVIEW_SWEEP`):
//! how close each authored pod body comes to the airframe as drawn at that
//! sweep (wing panel, flap strips, tailerons, fin, glove plates, fuselage),
//! to every other store and its pylon (the row 7 store and the AIM-9 rail),
//! to the gear when it is down, and how high its lowest point is above the
//! gear's contact height. Gaps are the smallest distance between sampled
//! surface points (a quarter foot apart) and the other surface's triangles,
//! both ways; zero means the surfaces touch or cross. A screening aid, not a
//! certification (docs/spec/stores-on-pylons.md, TOR-13). Update (TOR-12,
//! 2026-11-12): the glove plates are gone; the fuselage part now includes the
//! glove box's closing walls.
use super::{AircraftId, CombatArt, StorePose};
use crate::{aircraft::Airframe, flight, snapshot::StoreLook};
use tore_sim::attitude::{Vector, dot};
use tore_sim::combat::swing;

type Tri = [Vector; 3];

fn sub(a: Vector, b: Vector) -> Vector {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

/// The point of triangle `t` nearest `p` (Ericson, Real-Time Collision
/// Detection, 5.1.5).
fn closest(p: Vector, t: &Tri) -> Vector {
    let (a, b, c) = (t[0], t[1], t[2]);
    let (ab, ac, ap) = (sub(b, a), sub(c, a), sub(p, a));
    let (d1, d2) = (dot(ab, ap), dot(ac, ap));
    if d1 <= 0. && d2 <= 0. {
        return a;
    }
    let bp = sub(p, b);
    let (d3, d4) = (dot(ab, bp), dot(ac, bp));
    if d3 >= 0. && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0. && d1 >= 0. && d3 <= 0. {
        let v = d1 / (d1 - d3);
        return std::array::from_fn(|i| a[i] + v * ab[i]);
    }
    let cp = sub(p, c);
    let (d5, d6) = (dot(ab, cp), dot(ac, cp));
    if d6 >= 0. && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0. && d2 >= 0. && d6 <= 0. {
        let w = d2 / (d2 - d6);
        return std::array::from_fn(|i| a[i] + w * ac[i]);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0. && (d4 - d3) >= 0. && (d5 - d6) >= 0. {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return std::array::from_fn(|i| b[i] + w * sub(c, b)[i]);
    }
    let denominator = 1. / (va + vb + vc);
    let (v, w) = (vb * denominator, vc * denominator);
    std::array::from_fn(|i| a[i] + ab[i] * v + ac[i] * w)
}

fn distance(p: Vector, t: &Tri) -> f64 {
    let q = closest(p, t);
    dot(sub(p, q), sub(p, q)).sqrt()
}

/// Points on `t`, never more than `step` feet apart.
fn samples(t: &Tri, step: f64) -> Vec<Vector> {
    let longest = (0..3)
        .map(|i| {
            let e = sub(t[(i + 1) % 3], t[i]);
            dot(e, e).sqrt()
        })
        .fold(0., f64::max);
    let n = (longest / step).ceil().max(1.) as usize;
    let mut out = Vec::new();
    for i in 0..=n {
        for j in 0..=n - i {
            let (u, v) = (i as f64 / n as f64, j as f64 / n as f64);
            out.push(std::array::from_fn(|k| {
                t[0][k] * (1. - u - v) + t[1][k] * u + t[2][k] * v
            }));
        }
    }
    out
}

/// The smallest distance between two triangle sets, sampled both ways, with
/// where on the first it is (zero when they touch or cross).
fn gap(one: &[Tri], two: &[Tri]) -> (f64, Vector) {
    let (mut best, mut at) = (f64::MAX, [0.; 3]);
    for (first, second) in [(one, two), (two, one)] {
        let (low, high) = bounds(second);
        for tri in first {
            for p in samples(tri, 0.25) {
                // Only triangles near enough to beat the best so far.
                if (0..3).any(|i| p[i] < low[i] - best || p[i] > high[i] + best) {
                    continue;
                }
                for other in second {
                    let d = distance(p, other);
                    if d < best {
                        (best, at) = (d, p);
                    }
                }
            }
        }
    }
    (best, at)
}

fn bounds(tris: &[Tri]) -> (Vector, Vector) {
    let (mut low, mut high) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in tris.iter().flatten() {
        for i in 0..3 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    }
    (low, high)
}

/// Triangles of the airframe's faces (feet right, up, forward), by part.
fn airframe_parts(airframe: &Airframe, state: &flight::State) -> Vec<(&'static str, Vec<Tri>)> {
    use crate::tornado_animation as t;
    let scale = f64::from(airframe.animation_scale());
    let mut parts: Vec<(&'static str, Vec<Tri>)> = [
        "wing panel",
        "flap strip",
        "taileron",
        "fin",
        "gear",
        "fuselage",
    ]
    .map(|name| (name, Vec::new()))
    .into();
    for face in airframe.animation_faces(state) {
        let a = face.address;
        let part = if t::STRIP_R.contains(&a) || t::STRIP_L.contains(&a) {
            1
        } else if t::wing_right(a) || t::wing_left(a) {
            0
        } else if t::tail_right(a) || t::tail_left(a) {
            2
        } else if t::rudder(a) {
            3
        } else if t::GEAR.contains(&a) {
            4
        } else {
            5
        };
        let p: Vec<Vector> = face
            .positions
            .iter()
            .map(|q| [q[0], q[2], q[1]].map(|v| f64::from(v) * scale))
            .collect();
        for k in 1..p.len().saturating_sub(1) {
            parts[part].1.push([p[0], p[k], p[k + 1]]);
        }
    }
    parts
}

/// A drawn store's triangles (its pylon and body), feet right, up, forward.
fn drawn(store: &StorePose, airframe: &Airframe, art: &CombatArt) -> Vec<Tri> {
    let mut buffer = Vec::new();
    crate::pylons::vertices(
        &mut buffer,
        [store],
        [0.; 3],
        [0.; 3],
        Some(airframe),
        art,
        &[[128; 3]; 256],
    );
    buffer
        .chunks_exact(30)
        .map(|t| std::array::from_fn(|i| [t[i * 10], t[i * 10 + 1], t[i * 10 + 2]].map(f64::from)))
        .collect()
}

/// How far into a thin flat part `plate` (a taileron: one plane, a polygon)
/// the surface `one` reaches, and where (TOR-14, 2026-11-13): the deepest a
/// point of `one`'s crossing with the plate's plane lies inside the plate's
/// outline, as its distance from the nearest outline edge, feet. Zero when
/// the surface does not cross the plate or crosses only its edge. The gap
/// ([`gap`]) says only that two surfaces touch; this says how deep a crossing
/// goes.
fn entry(one: &[Tri], plate: &[Tri]) -> (f64, Vector) {
    let Some(first) = plate.first() else {
        return (0., [0.; 3]);
    };
    let cross = |a: Vector, b: Vector| {
        [
            a[1] * b[2] - a[2] * b[1],
            a[2] * b[0] - a[0] * b[2],
            a[0] * b[1] - a[1] * b[0],
        ]
    };
    let mut n = cross(sub(first[1], first[0]), sub(first[2], first[0]));
    let length = dot(n, n).sqrt();
    if length < 1e-9 {
        return (0., [0.; 3]);
    }
    n = n.map(|v| v / length);
    let d = dot(n, first[0]);
    // The outline: edges that belong to the fewest triangles.
    let key = |p: Vector| p.map(|v| (v * 1e4).round() as i64);
    type Edge = (usize, [Vector; 2]);
    let mut edges: std::collections::HashMap<([i64; 3], [i64; 3]), Edge> = Default::default();
    for t in plate {
        for i in 0..3 {
            let (a, b) = (t[i], t[(i + 1) % 3]);
            let id = if key(a) < key(b) {
                (key(a), key(b))
            } else {
                (key(b), key(a))
            };
            edges.entry(id).or_insert((0, [a, b])).0 += 1;
        }
    }
    let fewest = edges.values().map(|e| e.0).min().unwrap_or(0);
    let outline: Vec<[Vector; 2]> = edges
        .values()
        .filter(|e| e.0 == fewest)
        .map(|e| e.1)
        .collect();
    let inside = |q: Vector| {
        plate.iter().any(|t| {
            let (v0, v1, v2) = (sub(t[1], t[0]), sub(t[2], t[0]), sub(q, t[0]));
            let (d00, d01, d11) = (dot(v0, v0), dot(v0, v1), dot(v1, v1));
            let (d20, d21) = (dot(v2, v0), dot(v2, v1));
            let det = d00 * d11 - d01 * d01;
            if det.abs() < 1e-12 {
                return false;
            }
            let (v, w) = ((d11 * d20 - d01 * d21) / det, (d00 * d21 - d01 * d20) / det);
            v >= -1e-9 && w >= -1e-9 && v + w <= 1. + 1e-9
        })
    };
    let to_edge = |q: Vector| {
        outline
            .iter()
            .map(|[a, b]| {
                let ab = sub(*b, *a);
                let t = (dot(sub(q, *a), ab) / dot(ab, ab).max(1e-12)).clamp(0., 1.);
                let c: Vector = std::array::from_fn(|i| a[i] + t * ab[i]);
                dot(sub(q, c), sub(q, c)).sqrt()
            })
            .fold(f64::MAX, f64::min)
    };
    let (mut deepest, mut at) = (0., [0.; 3]);
    for tri in one {
        let s = tri.map(|v| dot(n, v) - d);
        if s.iter().all(|v| *v >= 0.) || s.iter().all(|v| *v <= 0.) {
            continue;
        }
        let mut ends = Vec::new();
        for i in 0..3 {
            let (a, b) = (i, (i + 1) % 3);
            if (s[a] > 0.) != (s[b] > 0.) {
                let t = s[a] / (s[a] - s[b]);
                ends.push(std::array::from_fn::<f64, 3, _>(|k| {
                    tri[a][k] + t * (tri[b][k] - tri[a][k])
                }));
            }
        }
        if let [p, q] = ends[..] {
            let span = sub(q, p);
            let steps = (dot(span, span).sqrt() / 0.05).ceil().max(1.) as usize;
            for k in 0..=steps {
                let t = k as f64 / steps as f64;
                let point: Vector = std::array::from_fn(|i| p[i] + t * span[i]);
                if inside(point) {
                    let depth = to_edge(point);
                    if depth > deepest {
                        (deepest, at) = (depth, point);
                    }
                }
            }
        }
    }
    (deepest, at)
}

/// Where each swivelling pylon's centre stands on the drawn wing (TOR-14,
/// 2026-11-13; `Pylon centre:` lines): carried back to the spread pose, its
/// span and forward place, the drawn panel's leading and trailing edge there
/// (read off the airframe's surface, a hundredth of a foot apart, flaps up)
/// and how far the centre is from the chord's middle. Zero at every sweep
/// means the pylon is locked to the wing's mid-chord.
pub(super) fn pylon_centres(short: &str, airframe: &Airframe, stores: &[StorePose], sweep: f64) {
    for store in stores.iter().filter(|s| matches!(s.hardpoint, 7..=10)) {
        for place in crate::pylons::hung(store) {
            // The rail's pylon stands its offset beside the rail's store.
            let out = store.rack.location().rail_out();
            let side = if place.pylon.point[0] < 0. { -1. } else { 1. };
            let centre = [
                place.pylon.point[0] - side * out,
                place.pylon.point[1],
                place.pylon.point[2],
            ];
            let spread = swing::unswept(side > 0., centre, f64::from(store.rack.turn));
            let span = spread[0].abs();
            let chord: Vec<f64> = (-800..=800)
                .map(|k| f64::from(k) * 0.01)
                .filter(|f| {
                    airframe
                        .underside(span, *f, -0.3, 0)
                        .is_some_and(|h| h < 1.5)
                })
                .collect();
            let (Some(back), Some(front)) = (chord.first(), chord.last()) else {
                continue;
            };
            let middle = (back + front) / 2.;
            println!(
                "Pylon centre: {short} sweep {sweep} row {} {} side {}: aircraft [{:.2} {:.2} {:.2}], wing frame {span:.2} ft out, {:.3} forward; drawn panel there {back:.3} to {front:.3}, middle {middle:.3}, off the middle {:.3} ft, chord {:.2} ft",
                store.hardpoint,
                store.shape.as_deref().unwrap_or("body"),
                place.side,
                centre[0],
                centre[1],
                centre[2],
                spread[2],
                spread[2] - middle,
                front - back
            );
        }
    }
}

/// The taileron entry of each wing store and pod (rows 7 to 10, with their
/// pylons) at this sweep with the tailerons neutral and at full pitch and
/// roll deflection (`Taileron entry:` lines).
fn taileron_entry(
    short: &str,
    sweep: f64,
    airframe: &Airframe,
    art: &CombatArt,
    stores: &[StorePose],
    state: &mut flight::State,
) {
    let subjects: Vec<(&StorePose, Vec<Tri>)> = stores
        .iter()
        .filter(|s| matches!(s.hardpoint, 7..=10))
        .map(|s| (s, drawn(s, airframe, art)))
        .filter(|(_, tris)| !tris.is_empty())
        .collect();
    state.speed = swing::speed_for_sweep(sweep);
    state.flaps = 0.;
    state.gear = 0.;
    for (name, elevator, aileron) in [
        ("neutral", 0., 0.),
        ("pitch up", 1., 0.),
        ("pitch down", -1., 0.),
        ("roll right", 0., 1.),
        ("roll left", 0., -1.),
    ] {
        state.elevator = elevator;
        state.aileron = aileron;
        let plates: Vec<(&str, Vec<Tri>)> = airframe_parts(airframe, state)
            .into_iter()
            .filter(|(part, _)| *part == "taileron")
            .collect();
        for (part, tris) in &plates {
            // One plate at a time (the right and the left taileron).
            let right: Vec<Tri> = tris.iter().filter(|t| t[0][0] >= 0.).copied().collect();
            let left: Vec<Tri> = tris.iter().filter(|t| t[0][0] < 0.).copied().collect();
            for (side, plate) in [("right", &right), ("left", &left)] {
                for (store, body) in &subjects {
                    // The store's triangles on this side of the aircraft.
                    let mine: Vec<Tri> = body
                        .iter()
                        .filter(|t| (t[0][0] + t[1][0] + t[2][0] >= 0.) == (side == "right"))
                        .copied()
                        .collect();
                    let (depth, at) = entry(&mine, plate);
                    println!(
                        "Taileron entry: {short} sweep {sweep} {name} {side} {part} vs row {} {}: {depth:.2} ft at [{:.2} {:.2} {:.2}]",
                        store.hardpoint,
                        store.shape.as_deref().unwrap_or("body"),
                        at[0],
                        at[1],
                        at[2]
                    );
                }
            }
        }
    }
    state.elevator = 0.;
    state.aileron = 0.;
}

pub(super) fn report(
    short: &str,
    airframe: &Airframe,
    art: &CombatArt,
    stores: &[StorePose],
    sweep: f64,
    mut state: flight::State,
) {
    let pods: Vec<&StorePose> = stores
        .iter()
        .filter(|s| {
            s.look == StoreLook::Pod
                && s.equipment_pod().is_some_and(|record| {
                    tore_formats::aircraft::stations::authored_pod(record).is_some()
                })
        })
        .collect();
    if pods.is_empty() || airframe.profile.id.source() != AircraftId::Tornado {
        println!("Pod clearance: {short} carries no authored pod");
        return;
    }
    // The pose: sweep from the speed, flaps and gear as asked.
    let mut poses = vec![(0., 0.)];
    if sweep < 26. {
        poses.extend([(1., 0.), (0., 1.)]);
    }
    // The gear's contact height, read with the gear down at any sweep (the
    // legs stand the same).
    let mut ground = f64::MAX;
    state.speed = swing::speed_for_sweep(sweep);
    state.flaps = 0.;
    state.gear = 1.;
    for (name, tris) in airframe_parts(airframe, &state) {
        if name == "gear" {
            ground = tris.iter().flatten().map(|p| p[1]).fold(ground, f64::min);
        }
    }
    for pod in &pods {
        let record = pod.equipment_pod().unwrap_or("?");
        for (color, entry, rgb) in crate::pylons::authored_palette_entries(pod, &airframe.palette) {
            println!(
                "Pod palette: {record} model colour {color:.2?} is palette entry {entry}, rgb {rgb:?}"
            );
        }
        for place in crate::pylons::hung(pod).iter().filter(|p| p.held) {
            let Some(body) = crate::pylons::authored_triangles(pod, place.centre) else {
                continue;
            };
            let (low, high) = bounds(&body);
            println!(
                "Pod clearance: {short} sweep {sweep} {record}: centre [{:.2} {:.2} {:.2}] x {:.2} to {:.2} y {:.2} to {:.2} z {:.2} to {:.2}, lowest point {:.2} ft above the gear contact (ground {ground:.2})",
                place.centre[0],
                place.centre[1],
                place.centre[2],
                low[0],
                high[0],
                low[1],
                high[1],
                low[2],
                high[2],
                low[1] - ground
            );
            // The pylon that carries it, for what it touches besides the wing.
            for (flaps, gear) in &poses {
                state.flaps = *flaps;
                state.gear = *gear;
                let label = format!("flaps {flaps} gear {gear}");
                for (name, tris) in airframe_parts(airframe, &state) {
                    if tris.is_empty() {
                        continue;
                    }
                    let (d, at) = gap(&body, &tris);
                    println!(
                        "Pod clearance: {short} sweep {sweep} {label} {record} vs {name}: gap {d:.2} ft at [{:.2} {:.2} {:.2}]",
                        at[0], at[1], at[2]
                    );
                }
            }
            // The other stores and their pylons.
            for other in stores
                .iter()
                .filter(|s| !std::ptr::eq(*s, *pod) && s.hardpoint != pod.hardpoint)
            {
                let tris = drawn(other, airframe, art);
                if tris.is_empty() {
                    continue;
                }
                let (d, at) = gap(&body, &tris);
                println!(
                    "Pod clearance: {short} sweep {sweep} {record} vs row {} {}: gap {d:.2} ft at [{:.2} {:.2} {:.2}]",
                    other.hardpoint,
                    other.shape.as_deref().unwrap_or("body"),
                    at[0],
                    at[1],
                    at[2]
                );
            }
        }
    }
    // TOR-12 (2026-11-12): the wing stores (rows 7 and 8: the Mk 82 pack and
    // the AIM-9 rail, with their pylons, both wings; TOR-14: and the pods'
    // pylons, rows 9 and 10) against the airframe
    // parts as drawn at this sweep. The pylon's own contact with the wing
    // panel reads zero by design.
    for other in stores.iter().filter(|s| matches!(s.hardpoint, 7..=10)) {
        let tris = drawn(other, airframe, art);
        if tris.is_empty() {
            continue;
        }
        for (flaps, gear) in &poses {
            state.flaps = *flaps;
            state.gear = *gear;
            for (name, parts) in airframe_parts(airframe, &state) {
                if parts.is_empty() {
                    continue;
                }
                let (d, at) = gap(&tris, &parts);
                println!(
                    "Wing store clearance: {short} sweep {sweep} flaps {flaps} gear {gear} row {} {} vs {name}: gap {d:.2} ft at [{:.2} {:.2} {:.2}]",
                    other.hardpoint,
                    other.shape.as_deref().unwrap_or("body"),
                    at[0],
                    at[1],
                    at[2]
                );
            }
        }
    }
    taileron_entry(short, sweep, airframe, art, stores, &mut state);
}
