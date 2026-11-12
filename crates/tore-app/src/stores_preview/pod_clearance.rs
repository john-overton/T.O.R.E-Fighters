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
    // the AIM-9 rail, with their pylons, both wings) against the airframe
    // parts as drawn at this sweep. The pylon's own contact with the wing
    // panel reads zero by design.
    for other in stores
        .iter()
        .filter(|s| matches!(s.hardpoint, 7 | 8) && s.look != StoreLook::Pod)
    {
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
}
