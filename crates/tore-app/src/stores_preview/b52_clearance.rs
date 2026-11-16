//! B52-5 clearances for the B-52H's bay and underwing pylons
//! (`TORE_STORES_PREVIEW_B52CLEAR=1`): how close each drawn store comes to the
//! parts of the airframe and to its neighbours, in feet, positive clear and
//! zero touching or crossing (the screening gap of [`pod_clearance`], sampled a
//! quarter foot apart, both ways).
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
//! The same variable reports the Tu-26 (T26-5, 2026-11-16): its one bay (the
//! same measures, the belly faces and the lining of `tu26_animation`) and its
//! three external rows against the fuselage, the glove, the swing panels at
//! spread and at full sweep, the flap strips and the gear
//! ([`tu26_pylons`]).
//!
//! A screening aid, not a certification (docs/spec/stores-on-pylons.md, "B-52
//! bay and pylons").
use super::pod_clearance::{Tri, drawn, gap};
use super::{AircraftId, CombatArt, StorePose};
use crate::{aircraft::Airframe, flight::State};
use tore_sim::attitude::Vector;
use tore_sim::combat::bays::opening;

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
    if id.source() == AircraftId::Tu26 {
        tu26_pylons(short, airframe, art, stores, start);
    } else {
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
    // The belly faces the doors are cut from and the first lining address of
    // the aircraft: the B-52's one skin, the Tu-26's 17 faces. The first five
    // lining faces are the ceiling and walls; any after them are rails and
    // posts.
    let (belly, lining_base): (&[usize], usize) = if id.source() == AircraftId::Tu26 {
        (
            &crate::tu26_animation::BELLY,
            crate::tu26_animation::LINING_BASE,
        )
    } else {
        (
            &[crate::b52_animation::BELLY],
            crate::b52_animation::LINING_BASE,
        )
    };
    let Some(o) = opening(id) else {
        println!("Bay clearance: {short} has no centred bay");
        return;
    };
    let mut state = start.clone();
    state.bay = 1.;
    let roof = o.belly + o.depth;
    println!(
        "Bay clearance: {short} opening x +/-{:.2} ft, {:.2} to {:.2} ft forward, belly {:.3} ft, roof {roof:.3} ft (depth {:.2}), doors hang to {:.2} ft",
        o.outer,
        o.aft,
        o.fore,
        o.belly,
        o.depth,
        o.belly - o.outer
    );
    // The leaves, and the rails and posts (the first five lining faces are the
    // ceiling and walls).
    let leaves = faces(airframe, &state, |f, p| {
        belly.contains(&f.address) && p.iter().any(|v| v[1] < o.belly - 0.05 && v[0].abs() > 2.)
    });
    let furniture = faces(airframe, &state, |f, _| f.address >= lining_base + 5);
    let mut placed: Vec<(String, Vec<Tri>)> = Vec::new();
    for store in stores.iter().filter(|s| s.rack.location().bay()) {
        for place in crate::pylons::hung(store).iter().filter(|p| p.held) {
            let tris = body(store, art, place.centre);
            let (low, high) = extremes(&tris);
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
                "Bay clearance: {name}: side wall {side:.2}, door leaf {leaf:.2}, aft end {:.2}, fore end {:.2}, roof {:.2}, above the belly plane {:.2}, rails and posts {rail:.2}",
                low[2] - o.aft,
                o.fore - high[2],
                roof - high[1],
                low[1] - o.belly
            );
            placed.push((name, tris));
        }
    }
    let mut nearest = f64::MAX;
    let mut worst = String::new();
    for (a, one) in placed.iter().enumerate() {
        let mut best = (f64::MAX, "");
        for (b, two) in placed.iter().enumerate() {
            if a == b {
                continue;
            }
            let (d, _) = gap(&one.1, &two.1);
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

/// The Tu-26's external rows (6 under the intake, 7 and 8 on the glove) as the
/// stores hang: each row's stores (their bodies, the pylon meets the skin it
/// hangs from by design and is left out) against the fuselage (faces within 5.5
/// units of the centre line), the glove (the fixed wing root beyond it, the
/// swing panels and the gear aside), the swing panel at spread, with the flaps
/// down (the rig's 0.40 rad droop) and swept to full sweep, and the whole
/// station (pylon and stores) against the gear down, then store against store
/// (T26-5). Depth into the airframe is the generic clip report's.
fn tu26_pylons(
    short: &str,
    airframe: &Airframe,
    art: &CombatArt,
    stores: &[StorePose],
    start: &State,
) {
    use crate::tu26_animation::{FLAME, GEAR, TAIL_L, TAIL_R, WING_L, WING_R};
    let panel = |a: usize| WING_R.contains(&a) || WING_L.contains(&a);
    let other = |a: usize| {
        GEAR.contains(&a) || FLAME.contains(&a) || TAIL_R.contains(&a) || TAIL_L.contains(&a)
    };
    // The gear's contact line, feet up: the plates' low point, z -11 units.
    const CONTACT: f64 = -11. * 4. / 3.;
    let mut state = start.clone();
    state.gear = 1.;
    state.bay = 0.;
    state.speed = 0.;
    let fuselage = faces(airframe, &state, |f, p| {
        !panel(f.address) && !other(f.address) && centroid(p)[0].abs() < 5.5 * 4. / 3.
    });
    let glove = faces(airframe, &state, |f, p| {
        !panel(f.address) && !other(f.address) && centroid(p)[0].abs() >= 5.5 * 4. / 3.
    });
    let gear = faces(airframe, &state, |f, _| GEAR.contains(&f.address));
    // The swing panels spread (flaps up and down) and fully swept (gear up, or
    // the rig holds them spread).
    let panels: Vec<(&str, Vec<Tri>)> = [
        ("spread", 0., 0.),
        ("flaps down", 1., 0.),
        ("swept", 0., 700.),
    ]
    .into_iter()
    .map(|(name, flaps, knots)| {
        let mut s = start.clone();
        s.flaps = flaps;
        s.gear = 0.;
        s.bay = 0.;
        s.speed = knots * 1.68781;
        (name, faces(airframe, &s, |f, _| panel(f.address)))
    })
    .collect();
    for store in stores.iter().filter(|s| s.rack.location().external()) {
        let station = drawn(store, airframe, art);
        let held: Vec<Tri> = crate::pylons::hung(store)
            .iter()
            .filter(|p| p.held)
            .flat_map(|p| body(store, art, p.centre))
            .collect();
        let mut line = format!(
            "Pylon clearance: {short} row {} {} x{}:",
            store.hardpoint,
            store.shape.as_deref().unwrap_or("body"),
            store.items
        );
        for (name, part) in [("fuselage", &fuselage), ("glove", &glove)] {
            let (d, at) = gap(&held, part);
            line += &format!(
                " {name} {d:.2} (at [{:.1} {:.1} {:.1}]),",
                at[0], at[1], at[2]
            );
        }
        for (name, part) in &panels {
            let (d, at) = gap(&held, part);
            line += &format!(
                " wing panel {name} {d:.2} (at [{:.1} {:.1} {:.1}]),",
                at[0], at[1], at[2]
            );
        }
        let (d, at) = gap(&station, &gear);
        line += &format!(
            " gear {d:.2} (at [{:.1} {:.1} {:.1}]),",
            at[0], at[1], at[2]
        );
        let low = extremes(&station).0[1];
        line += &format!(
            " lowest point {:.2} ft above the gear contact line",
            low - CONTACT
        );
        println!("{line}");
    }
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
