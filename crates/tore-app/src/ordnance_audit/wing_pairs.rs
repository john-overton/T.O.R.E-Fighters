//! Exhaustive imported-roster check of the physical wing-pair load rule.
use super::*;

pub(super) fn audit(
    standard: &Loadout,
    aircraft: &Aircraft,
    data: &BTreeMap<String, Vec<u8>>,
) -> AppResult<()> {
    let records = tore_sim::combat::hardpoints::default_records(aircraft);
    let mut cheat = standard.clone();
    cheat.cheat = true;
    for row in standard.editable_stations() {
        if let Some(slot) = row.weapon {
            let station = &standard.configuration.stations[slot];
            let policy = standard.wing_pairs(slot);
            if policy.step(1) != 2 {
                continue;
            }
            if !policy.valid(station.count) || !policy.valid(standard.quantities[slot]) {
                return Err("default wing load is not paired".into());
            }
            let source = &records[row.hardpoint];
            println!(
                "wing-pair source: {} row={} maxItems={} weight_limit_lb={} default={} count={}",
                aircraft.id.selection_key(),
                row.hardpoint,
                standard.hardpoints[slot].count,
                standard.hardpoints[slot].weight_class * 100,
                station.weapon.source,
                station.count
            );
            if source.store.as_deref() == Some(&station.weapon.source)
                && source.count != i32::from(station.count)
            {
                println!(
                    "wing-pair default: {} row={} {} {}->{} (human and AI)",
                    aircraft.id.selection_key(),
                    row.hardpoint,
                    station.weapon.source,
                    source.count,
                    station.count
                );
            }
            // Include imported setup-only definitions, not just live weapons.
            for (name, bytes) in data.iter().filter(|(name, _)| name.ends_with(".JT")) {
                let weapon = Weapon::parse(name, bytes)?;
                let cheat_old = cheat.capacity_before_wing_pairs(slot, &weapon);
                let cheat_new = cheat.capacity(slot, &weapon);
                if cheat_old != cheat_new {
                    println!(
                        "wing-pair cheat maximum: {} row={} {} {}->{}",
                        aircraft.id.selection_key(),
                        row.hardpoint,
                        name,
                        cheat_old,
                        cheat_new
                    );
                }
                let old = standard.capacity_before_wing_pairs(slot, &weapon);
                let new = standard.capacity(slot, &weapon);
                if new != old {
                    println!(
                        "wing-pair maximum: {} row={} {} {}->{} weight={} live={}",
                        aircraft.id.selection_key(),
                        row.hardpoint,
                        name,
                        old,
                        new,
                        weapon.weight,
                        SUPPORTED_WEAPONS.contains(&name.as_str())
                    );
                    if new != policy.maximum(old) || new % 2 != 0 {
                        return Err("wing maximum differs from pair policy".into());
                    }
                    let mut draft = standard.clone();
                    draft.select(slot, weapon)?;
                    draft.change(slot, -1);
                    if draft.quantities[slot] != new as u16 - 2 {
                        return Err("wing quantity does not step by a pair".into());
                    }
                    // Old saves shed the unmatched store. A lobby request
                    // with the old odd count must not bypass validation.
                    let mut saved = LoadoutSpec::of(&draft);
                    saved.stations[slot].count = old as u16;
                    saved.stations[slot].quantity = old as u16;
                    let migrated = saved.apply(standard.clone(), data, None)?;
                    if migrated.quantities[slot] != policy.saved_quantity(old as u16, new)
                        || migrated.configuration.stations[slot].count != new as u16
                    {
                        return Err("old odd wing load migration differs".into());
                    }
                    if saved.check_for_plane(aircraft, data, false).is_ok() {
                        return Err("lobby accepted an odd wing load".into());
                    }
                }
            }
        }
        if let Some(slot) = row.tank {
            let station = &standard.configuration.tanks[slot];
            let policy = loading::WingPairs::of(station.rack, station.mount);
            if !policy.valid(station.quantity) {
                return Err("default wing tanks are not paired".into());
            }
            for (name, bytes) in data.iter().filter(|(name, _)| name.ends_with(".GAS")) {
                let store = TankStore::parse(name, bytes)?;
                let cheat_old = cheat.tank_capacity_before_wing_pairs(slot, &store);
                let cheat_new = cheat.tank_capacity(slot, &store);
                if cheat_old != cheat_new {
                    println!(
                        "wing-pair cheat maximum: {} row={} {} {}->{}",
                        aircraft.id.selection_key(),
                        row.hardpoint,
                        name,
                        cheat_old,
                        cheat_new
                    );
                }
                let old = standard.tank_capacity_before_wing_pairs(slot, &store);
                let new = standard.tank_capacity(slot, &store);
                if old != new {
                    println!(
                        "wing-pair maximum: {} row={} {} {}->{} weight={} live=true",
                        aircraft.id.selection_key(),
                        row.hardpoint,
                        name,
                        old,
                        new,
                        i32::from(store.tank.empty_weight) + store.tank.fuel_weight
                    );
                }
            }
        }
    }
    println!(
        "wing-pair audit: {} defaults, maxima, steps, saved loads and lobby checked",
        aircraft.id.selection_key()
    );
    Ok(())
}
