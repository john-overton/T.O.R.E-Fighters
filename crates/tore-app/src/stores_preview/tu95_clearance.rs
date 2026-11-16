//! T95-5 clearances for the Tu-95's bay and wing pylons
//! (`TORE_STORES_PREVIEW_T95CLEAR=1`): how close each drawn store comes to the
//! parts of the airframe and to its neighbours, in feet, positive clear and
//! zero touching or crossing (the screening gap of [`pod_clearance`], sampled a
//! quarter foot apart, both ways). The same report as the B-52's
//! ([`super::b52_clearance`], whose helpers it shares), against the Tu-95's own
//! parts.
//!
//! - **Bay**, doors fully open: for each bay store, the room to the door
//!   leaves (which hang on the recess walls, 2.5 ft out), the end walls, the
//!   ceiling, the belly plane under the lowest tier, the lane rails, and the
//!   nearest other store.
//! - **Pylons**: the whole station as drawn (pylons and stores) against the
//!   inboard propeller disc, the inboard nacelle, the fuselage, the gear down,
//!   and the rest of the airframe, and how high each store's lowest point is
//!   above the gear contact line.
//!
//! A screening aid, not a certification (docs/spec/stores-on-pylons.md,
//! "Tu-95 bay and pylons").
use super::b52_clearance::{body, centroid, extremes, faces};
use super::pod_clearance::{Tri, drawn, gap};
use super::{AircraftId, CombatArt, StorePose};
use crate::{aircraft::Airframe, flight::State};
use tore_sim::combat::bays::opening;

/// The kept faces of the two inboard propeller discs (right, left): the blur
/// images the airframe draws (`variety_rotors`, T95-3).
const INBOARD_DISCS: [usize; 4] = [0x1aa2, 0x1b27, 0x243f, 0x24c4];

pub(super) fn report(
    short: &str,
    id: AircraftId,
    airframe: &Airframe,
    art: &CombatArt,
    stores: &[StorePose],
    start: &State,
) {
    bay(short, id, airframe, art, stores, start);
    pylons(short, airframe, art, stores, start);
}

fn bay(
    short: &str,
    id: AircraftId,
    airframe: &Airframe,
    art: &CombatArt,
    stores: &[StorePose],
    start: &State,
) {
    use crate::tu95_animation::{BELLY, LINING_BASE};
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
    // The leaves: the belly pieces that hang below the plane inside the
    // opening (the keel's skin ahead of and behind it hangs lower too).
    let leaves = faces(airframe, &state, |f, p| {
        let c = centroid(p);
        // Feet right, up, forward.
        BELLY.contains(&f.address)
            && c[1] < o.belly - 0.05
            && c[2] >= o.aft - 0.01
            && c[2] <= o.fore + 0.01
            && c[0].abs() <= o.outer + 1.0
    });
    let rails = faces(airframe, &state, |f, _| f.address >= LINING_BASE + 5);
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
            let (rail, _) = gap(&tris, &rails);
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

/// The airframe's parts the pylon stores are measured against, each from the
/// faces of one pose (gear down).
struct Parts {
    disc: Vec<Tri>,
    nacelle: Vec<Tri>,
    fuselage: Vec<Tri>,
    gear: Vec<Tri>,
    wing: Vec<Tri>,
}

fn parts(airframe: &Airframe, state: &State) -> Parts {
    use crate::tu95_animation::GEAR;
    // The inboard nacelle and its plane of faces: 20 to 25 ft out (shape x 31 to
    // 36 units) and under the wing (below 1.5 ft up).
    let nacelle = |c: [f64; 3]| (19. ..=25.).contains(&c[0].abs()) && c[1] < 1.5;
    let other = |f: &tore_formats::shape::Face| {
        !GEAR.contains(&f.address) && !INBOARD_DISCS.contains(&f.address)
    };
    Parts {
        disc: faces(airframe, state, |f, _| INBOARD_DISCS.contains(&f.address)),
        nacelle: faces(airframe, state, |f, p| other(f) && nacelle(centroid(p))),
        fuselage: faces(airframe, state, |f, p| {
            other(f) && centroid(p)[0].abs() < 3.8
        }),
        gear: faces(airframe, state, |f, _| GEAR.contains(&f.address)),
        wing: faces(airframe, state, |f, p| {
            let c = centroid(p);
            other(f) && c[0].abs() >= 3.8 && !nacelle(c)
        }),
    }
}

fn pylons(short: &str, airframe: &Airframe, art: &CombatArt, stores: &[StorePose], start: &State) {
    // Where the gear's wheels touch, feet up (the shape's z -14 units).
    const CONTACT: f64 = -14. * 2. / 3.;
    let mut state = start.clone();
    state.gear = 1.;
    state.bay = 0.;
    // Propeller discs are drawn while the engine turns; the report reads their
    // faces in the neutral pose, which keeps the first image of each.
    let p = parts(airframe, &state);
    for store in stores.iter().filter(|s| s.rack.location().external()) {
        let station = drawn(store, airframe, art);
        let mut line = format!(
            "Pylon clearance: {short} row {} {} x{}:",
            store.hardpoint,
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
            ("inboard disc", &p.disc),
            ("inboard nacelle", &p.nacelle),
            ("fuselage", &p.fuselage),
            ("gear", &p.gear),
            ("wing (stores only)", &p.wing),
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
    // Each store against the inboard discs and nacelles, for the worst one.
    let mut closest = (f64::MAX, String::new());
    for (name, tris) in &bodies {
        for part in [&p.disc, &p.nacelle] {
            let (d, _) = gap(tris, part);
            if d < closest.0 {
                closest = (d, name.clone());
            }
        }
    }
    println!(
        "Pylon clearance: {short} nearest store to an inboard disc or nacelle {:.2} ft ({})",
        closest.0, closest.1
    );
}
