//! T160-5 clearances for the Tu-160's two weapon bays
//! (`TORE_STORES_PREVIEW_TU160CLEAR=1`): how close each drawn store comes to the
//! parts of the bay and to its neighbours, in feet, positive clear and zero
//! touching or crossing (the screening gap of [`pod_clearance`], sampled a
//! quarter foot apart, both ways). Each bay with its doors fully open: the room
//! to the two door leaves (which hang on the recess walls, 3.8 ft out), the
//! ends, the ceiling, the belly plane under the lowest tier, the rail and the
//! nearest other store (in the same bay and in the other one).
//!
//! A screening aid, not a certification (docs/spec/stores-on-pylons.md, "Tu-160
//! weapon bays").
use super::b52_clearance::{body, extremes, faces};
use super::pod_clearance::gap;
use super::{AircraftId, CombatArt, StorePose};
use crate::{
    aircraft::Airframe,
    flight::State,
    tu160_animation::{LINING_BASE, UNIT},
};
use tore_sim::combat::bays::openings;

pub(super) fn report(
    short: &str,
    id: AircraftId,
    airframe: &Airframe,
    art: &CombatArt,
    stores: &[StorePose],
    start: &State,
) {
    let bays = openings(id);
    if bays.is_empty() {
        println!("Bay clearance: {short} has no fitted bays");
        return;
    }
    let mut state = start.clone();
    state.bay = 1.;
    // Each bay's furniture: the lining faces after its ceiling and four walls,
    // the leaves that hang on its walls.
    let mut placed: Vec<(String, Vec<[[f64; 3]; 3]>)> = Vec::new();
    for (k, o) in bays.iter().enumerate() {
        let row = 4 + k;
        let roof = o.belly + o.depth;
        println!(
            "Bay clearance: {short} bay row {row} opening x +/-{:.2} ft, {:.2} to {:.2} ft forward, belly {:.3} ft, roof {roof:.3} ft (depth {:.2}), doors hang to {:.2} ft",
            o.outer,
            o.aft,
            o.fore,
            o.belly,
            o.depth,
            o.belly - o.outer
        );
        let base = LINING_BASE + k * 0x40;
        let leaves = faces(airframe, &state, |f, p| {
            !(base..base + 0x40).contains(&f.address)
                && p.iter().any(|v| v[1] < o.belly - 0.05)
                && p.iter().all(|v| {
                    (v[0].abs() - o.outer).abs() < 0.3 && v[2] > o.aft - 0.1 && v[2] < o.fore + 0.1
                })
        });
        let furniture = faces(airframe, &state, |f, _| {
            (base + 5..base + 0x40).contains(&f.address)
        });
        for store in stores
            .iter()
            .filter(|s| s.rack.location().bay() && usize::from(s.hardpoint) == row)
        {
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
                    "Bay clearance: {name}: side wall {side:.2}, door leaf {leaf:.2}, aft end {:.2}, fore end {:.2}, roof {:.2}, above the belly plane {:.2}, rail {rail:.2}",
                    low[2] - o.aft,
                    o.fore - high[2],
                    roof - high[1],
                    low[1] - o.belly
                );
                placed.push((name, tris));
            }
        }
    }
    println!(
        "Bay clearance: {short} drawn at {UNIT:.4} ft a unit, {} stores in {} bays",
        placed.len(),
        bays.len()
    );
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
