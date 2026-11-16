//! B52-5 clearances for the B-52H's bay and underwing pylons
//! (`TORE_STORES_PREVIEW_B52CLEAR=1`): how close each drawn store comes to the
//! parts of the airframe and to its neighbours, in feet, positive clear and
//! zero touching or crossing (the screening gap of [`pod_clearance`], sampled a
//! quarter foot apart, both ways). B1B-5: the same report for the B-1B's three
//! centred bays (each store measured against its own bay, the door leaves of
//! all three, no rails or posts, and no pylons).
//!
//! - **Bay**, doors fully open: for each bay store, the room to the two door
//!   leaves (which hang on the recess walls, 3 ft out), the end walls, the
//!   ceiling, the belly plane under the lowest tier, the rails and posts, and the
//!   nearest other store.
//! - **Pylons**: the whole station as drawn (pylons and stores) against the
//!   inboard engine pod, the flap strip with the flaps up and fully down
//!   (the B52-3 rig, 15 degrees), the fuselage, the gear down, and the rest
//!   of the airframe, and how high each store's lowest point is above the
//!   gear contact line.
//!
//! A screening aid, not a certification (docs/spec/stores-on-pylons.md, "B-52
//! bay and pylons").
use super::pod_clearance::{Tri, drawn, gap};
use super::{AircraftId, CombatArt, StorePose};
use crate::{aircraft::Airframe, flight::State};
use tore_sim::attitude::Vector;
use tore_sim::combat::bays::openings;

/// One store's triangles at `centre` (feet right, up, forward): the weapon
/// shape's faces as `pylons::mesh` draws them.
fn body(store: &StorePose, art: &CombatArt, centre: Vector) -> Vec<Tri> {
    use crate::snapshot::StoreLook;
    let Some(shape) = store
        .shape
        .as_deref()
        .and_then(|name| art.shape(name))
        .filter(|_| store.look == StoreLook::Store)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for face in shape
        .faces
        .iter()
        .filter(|f| !crate::pylons::texture_only(f))
    {
        let p: Vec<Vector> = face
            .positions
            .iter()
            .map(|q| {
                let q = [q[0], q[2], q[1]].map(|v| f64::from(v) / 3.);
                std::array::from_fn(|i| centre[i] + q[i])
            })
            .collect();
        for k in 1..p.len().saturating_sub(1) {
            out.push([p[0], p[k], p[k + 1]]);
        }
    }
    out
}

fn extremes(tris: &[Tri]) -> (Vector, Vector) {
    let (mut low, mut high) = ([f64::MAX; 3], [f64::MIN; 3]);
    for p in tris.iter().flatten() {
        for i in 0..3 {
            low[i] = low[i].min(p[i]);
            high[i] = high[i].max(p[i]);
        }
    }
    (low, high)
}

/// The airframe's triangles (feet right, up, forward) of the faces `keep`
/// accepts.
fn faces(
    airframe: &Airframe,
    state: &State,
    keep: impl Fn(&tore_formats::shape::Face, &[Vector]) -> bool,
) -> Vec<Tri> {
    let scale = f64::from(airframe.animation_scale());
    let mut out = Vec::new();
    for face in airframe.animation_faces(state) {
        let p: Vec<Vector> = face
            .positions
            .iter()
            .map(|q| [q[0], q[2], q[1]].map(|v| f64::from(v) * scale))
            .collect();
        if !keep(&face, &p) {
            continue;
        }
        for k in 1..p.len().saturating_sub(1) {
            out.push([p[0], p[k], p[k + 1]]);
        }
    }
    out
}

fn centroid(p: &[Vector]) -> Vector {
    let n = p.len().max(1) as f64;
    std::array::from_fn(|i| p.iter().map(|v| v[i]).sum::<f64>() / n)
}

pub(super) fn report(
    short: &str,
    id: AircraftId,
    airframe: &Airframe,
    art: &CombatArt,
    stores: &[StorePose],
    start: &State,
) {
    bay(short, id, airframe, art, stores, start);
    // The B-1B has no external station (B1B-5): the pylon report is the B-52's.
    if id.source() == AircraftId::B52 {
        pylons(short, airframe, art, stores, start);
    }
}

fn bay(
    short: &str,
    id: AircraftId,
    airframe: &Airframe,
    art: &CombatArt,
    stores: &[StorePose],
    start: &State,
) {
    use crate::{b1b_animation, b52_animation};
    let bays: Vec<_> = openings(id).collect();
    if bays.is_empty() {
        println!("Bay clearance: {short} has no centred bay");
        return;
    }
    // The belly skins the doors are cut from and where the rails and posts
    // begin in the lining (the B-52's lining is a ceiling and four walls, then
    // furniture; the B-1B's has none).
    let (belly, furniture_from): (&[usize], usize) = match id.source() {
        AircraftId::B1b => (&b1b_animation::BELLY, usize::MAX),
        _ => (&[b52_animation::BELLY], b52_animation::LINING_BASE + 5),
    };
    let mut state = start.clone();
    state.bay = 1.;
    for (k, o) in bays.iter().enumerate() {
        let name = if bays.len() == 1 {
            String::new()
        } else {
            format!("bay {k} ")
        };
        println!(
            "Bay clearance: {short} {name}opening x +/-{:.2} ft, {:.2} to {:.2} ft forward, belly {:.3} ft, roof {:.3} ft (depth {:.2}), doors hang to {:.2} ft",
            o.outer,
            o.aft,
            o.fore,
            o.belly,
            o.belly + o.depth,
            o.depth,
            o.belly - o.outer
        );
    }
    // The leaves, and the rails and posts.
    let leaves = faces(airframe, &state, |f, p| {
        belly.contains(&f.address) && p.iter().any(|v| v[1] < bays[0].belly - 0.05)
    });
    let furniture = faces(airframe, &state, |f, _| f.address >= furniture_from);
    // Each placed store with the bay it hangs in.
    let mut placed: Vec<(String, usize, Vec<Tri>)> = Vec::new();
    for store in stores.iter().filter(|s| s.rack.location().bay()) {
        for place in crate::pylons::hung(store).iter().filter(|p| p.held) {
            let tris = body(store, art, place.centre);
            let (low, high) = extremes(&tris);
            let middle = (low[2] + high[2]) / 2.;
            let k = bays
                .iter()
                .enumerate()
                .min_by(|a, b| {
                    let off = |o: &tore_sim::combat::bays::Opening| {
                        (middle - (o.aft + o.fore) / 2.).abs()
                    };
                    off(a.1).total_cmp(&off(b.1))
                })
                .map_or(0, |(k, _)| k);
            let o = &bays[k];
            let roof = o.belly + o.depth;
            let name = format!(
                "row{} {} slot{}",
                store.hardpoint,
                store.shape.as_deref().unwrap_or("body"),
                place.slot
            );
            let side = o.outer - low[0].abs().max(high[0].abs());
            let (leaf, _) = gap(&tris, &leaves);
            let (rail, _) = gap(&tris, &furniture);
            println!(
                "Bay clearance: {name}: side wall {side:.2}, door leaf {leaf:.2}, aft end {:.2}, fore end {:.2}, roof {:.2}, above the belly plane {:.2}, rails and posts {:.2}",
                low[2] - o.aft,
                o.fore - high[2],
                roof - high[1],
                low[1] - o.belly,
                rail.min(99.)
            );
            placed.push((name, k, tris));
        }
    }
    let mut nearest = f64::MAX;
    let mut worst = String::new();
    for (a, one) in placed.iter().enumerate() {
        let mut best = (f64::MAX, "");
        // Stores of other bays stand behind a wall and a bulkhead: only a
        // bay's own stores can touch.
        for (b, two) in placed.iter().enumerate() {
            if a == b || one.1 != two.1 {
                continue;
            }
            let (d, _) = gap(&one.2, &two.2);
            if d < best.0 {
                best = (d, two.0.as_str());
            }
        }
        println!(
            "Bay clearance: {} nearest store {:.2} ({})",
            one.0, best.0, best.1
        );
        if best.0 < nearest {
            nearest = best.0;
            worst = format!("{} / {}", one.0, best.1);
        }
    }
    println!(
        "Bay clearance: {short} {} stores, nearest pair {nearest:.2} ft ({worst})",
        placed.len()
    );
}

fn pylons(short: &str, airframe: &Airframe, art: &CombatArt, stores: &[StorePose], start: &State) {
    use crate::b52_animation::{FLAP_L, FLAP_R, GEAR};
    // Where the gear's wheels touch, feet up (the shape's z -14 units).
    const CONTACT: f64 = -14. * 2. / 3.;
    for flaps in [0., 1.] {
        let mut state = start.clone();
        state.flaps = flaps;
        state.gear = 1.;
        state.bay = 0.;
        let pod = faces(airframe, &state, |f, p| {
            let c = centroid(p);
            !GEAR.contains(&f.address)
                && !FLAP_R.contains(&f.address)
                && !FLAP_L.contains(&f.address)
                && (28. ..=42.).contains(&c[0].abs())
                && c[1] < 3.
        });
        let flap = faces(airframe, &state, |f, _| {
            FLAP_R.contains(&f.address) || FLAP_L.contains(&f.address)
        });
        let gear = faces(airframe, &state, |f, _| GEAR.contains(&f.address));
        let fuselage = faces(airframe, &state, |f, p| {
            let c = centroid(p);
            !GEAR.contains(&f.address)
                && !FLAP_R.contains(&f.address)
                && !FLAP_L.contains(&f.address)
                && c[0].abs() < 5.5
        });
        let wing = faces(airframe, &state, |f, p| {
            let c = centroid(p);
            let other = !GEAR.contains(&f.address)
                && !FLAP_R.contains(&f.address)
                && !FLAP_L.contains(&f.address);
            let in_pod = (28. ..=42.).contains(&c[0].abs()) && c[1] < 3.;
            other && c[0].abs() >= 5.5 && !in_pod
        });
        for store in stores.iter().filter(|s| s.rack.location().external()) {
            let station = drawn(store, airframe, art);
            let mut line = format!(
                "Pylon clearance: {short} row {} flaps {} {} x{}:",
                store.hardpoint,
                if flaps == 0. { "up" } else { "down 15 deg" },
                store.shape.as_deref().unwrap_or("body"),
                store.items
            );
            // The wing meets the pylons by design: its gap is the stores' own
            // bodies', not the pylons'.
            let held: Vec<Tri> = crate::pylons::hung(store)
                .iter()
                .filter(|p| p.held)
                .flat_map(|p| body(store, art, p.centre))
                .collect();
            for (name, part) in [
                ("inboard pod", &pod),
                ("flap strip", &flap),
                ("fuselage", &fuselage),
                ("gear", &gear),
                ("wing (stores only)", &wing),
            ] {
                let mine = if name.starts_with("wing") {
                    &held
                } else {
                    &station
                };
                let (d, at) = gap(mine, part);
                line += &format!(
                    " {name} {d:.2} (at [{:.1} {:.1} {:.1}]),",
                    at[0], at[1], at[2]
                );
            }
            let low = extremes(&station).0[1];
            line += &format!(
                " lowest point {:.2} ft above the gear contact line",
                low - CONTACT
            );
            println!("{line}");
        }
    }
    // Store to store, pylon stores against the other pylon's stores and their
    // own neighbours.
    let mut bodies: Vec<(String, Vec<Tri>)> = Vec::new();
    for store in stores.iter().filter(|s| s.rack.location().external()) {
        for place in crate::pylons::hung(store).iter().filter(|p| p.held) {
            bodies.push((
                format!(
                    "row{} {} {}{}",
                    store.hardpoint,
                    store.shape.as_deref().unwrap_or("body"),
                    if place.side == 0 { "R" } else { "L" },
                    place.slot
                ),
                body(store, art, place.centre),
            ));
        }
    }
    let mut nearest = (f64::MAX, String::new());
    let mut touching = 0;
    for (a, one) in bodies.iter().enumerate() {
        for two in &bodies[a + 1..] {
            let (d, _) = gap(&one.1, &two.1);
            if d < 0.05 {
                touching += 1;
                println!(
                    "Pylon clearance: {short} {} / {} gap {d:.2} ft",
                    one.0, two.0
                );
            }
            if d < nearest.0 {
                nearest = (d, format!("{} / {}", one.0, two.0));
            }
        }
    }
    println!(
        "Pylon clearance: {short} {} stores, nearest pair {:.2} ft ({}), {touching} pairs under 0.05 ft",
        bodies.len(),
        nearest.0,
        nearest.1
    );
    // Each store against the airframe's pods, for the worst one.
    let mut state = start.clone();
    state.gear = 1.;
    let pods = faces(airframe, &state, |f, p| {
        let c = centroid(p);
        !GEAR.contains(&f.address) && (28. ..=42.).contains(&c[0].abs()) && c[1] < 3.
    });
    let mut closest = (f64::MAX, String::new());
    for (name, tris) in &bodies {
        let (d, _) = gap(tris, &pods);
        if d < closest.0 {
            closest = (d, name.clone());
        }
    }
    println!(
        "Pylon clearance: {short} nearest store to an inboard pod {:.2} ft ({})",
        closest.0, closest.1
    );
}
