//! Supply trucks (docs/spec/surface-defenses.md, "Resupply"): each manned
//! `<sam>` slot gets a MISTRK and each manned `<aaa>` slot a TRUCK, and each
//! template battery one more MISTRK, placed beside the unit it serves. Base
//! layouts get none: their units are resupplied by the trucks already
//! standing in them (lead ruling, 2026-10-10).
use super::{
    Added, Inputs, Obstacles, Pose, added_unit, distance2, ordinal_of, ring, stands, stream_for,
    xz, *,
};
use crate::surface::{Battery, IdRange, Origin, SupplyTruck, Surface, UnitId};
use std::collections::BTreeSet;
use tore_formats::quick_template::Placeholder;

/// Adds the trucks (`added`) and returns them, ascending id; fills each
/// template battery's `truck`. A truck type the import cannot place is left
/// out, and `notes` says so.
pub(super) fn add(
    surface: &Surface,
    inputs: &mut Inputs<'_, '_>,
    batteries: &mut [Battery],
    obstacles: &Obstacles<'_>,
    added: &mut Vec<Added>,
    notes: &mut Vec<String>,
) -> Vec<SupplyTruck> {
    let Some(site) = &surface.template else {
        return Vec::new();
    };
    let source = format!("~{}.M", site.stem);
    // (served unit, 0 for its slot or 1 for its battery, truck type,
    // battery index).
    let mut wanted: Vec<(UnitId, u8, &'static str, Option<usize>)> = Vec::new();
    let mut slot_served = BTreeSet::new();
    for unit in surface.template_units() {
        let truck = match unit.origin {
            Origin::Template {
                placeholder: Some(Placeholder::Sam),
                ..
            } => SAM_TRUCK,
            Origin::Template {
                placeholder: Some(Placeholder::Aaa),
                ..
            } => AAA_TRUCK,
            _ => continue,
        };
        wanted.push((unit.id, 0, truck, None));
        slot_served.insert(unit.id);
    }
    for (index, battery) in batteries.iter().enumerate() {
        if battery.launchers[0].range() != IdRange::Template {
            continue;
        }
        // The battery's truck serves its first launcher without a slot
        // truck of its own, else its first launcher.
        let served = battery
            .launchers
            .iter()
            .copied()
            .find(|id| !slot_served.contains(id))
            .unwrap_or(battery.launchers[0]);
        wanted.push((served, 1, SAM_TRUCK, Some(index)));
    }
    wanted.sort_by_key(|(served, order, _, _)| (*served, *order));
    let mut trucks = Vec::new();
    let mut n = 0u32;
    let mut stream_of = std::collections::BTreeMap::new();
    for (served, _, kind, battery) in wanted {
        let Some(entry) = (inputs.added)(kind) else {
            notes.push(format!("no {kind} to add beside {:#010x}", served.0));
            continue;
        };
        let Some(id) = UnitId::supply_truck(n) else {
            continue;
        };
        n += 1;
        let owner = surface.unit(served).expect("a served unit");
        // One stream per served unit: its slot truck draws first, then its
        // battery's.
        let draws = stream_of.entry(served).or_insert_with(|| {
            stream_for(
                surface,
                inputs.layout,
                true,
                ordinal_of(owner),
                Purpose::Supply,
            )
        });
        let size = (inputs.types)(kind).footprint.radius();
        let owner_size = (inputs.types)(&owner.resource).footprint.radius();
        let centre = xz(owner.position);
        let mut first = None;
        let mut chosen = None;
        for _ in 0..JITTER_CANDIDATES {
            let (offset, bearing) = ring(draws, TRUCK_FT);
            let pose = Pose {
                at: [centre[0] + offset[0], centre[1] + offset[1]],
                // Parked across the line to the unit it serves.
                heading: bearing + 90,
            };
            first.get_or_insert(pose);
            if distance2(pose.at, centre) >= (size + owner_size).pow(2)
                && stands(inputs.ground, obstacles, pose.at, size, false)
            {
                chosen = Some(pose);
                break;
            }
        }
        let pose = chosen.or(first).expect("at least one candidate");
        added.push(Added {
            unit: added_unit(id, &entry, pose, owner),
            source: source.clone(),
        });
        if let Some(index) = battery {
            batteries[index].truck = Some(id);
        }
        trucks.push(SupplyTruck {
            id,
            serves: Some(served),
            added: true,
        });
    }
    trucks
}
