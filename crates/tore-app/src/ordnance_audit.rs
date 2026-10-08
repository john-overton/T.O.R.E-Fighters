//! Source-backed coverage for editable source stations and accepted stores.
use crate::AppResult;
use std::collections::BTreeMap;
use tore_formats::{
    aircraft::{Aircraft, AircraftId},
    weapons::Weapon,
};
use tore_sim::combat::{
    live::TankStore,
    loading,
    loadout::{Loadout, supported},
};
use tore_world::mission::{LoadoutSpec, MissionSpec};

pub fn validate(data: &BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    let weapons = data
        .iter()
        .filter(|(name, _)| supported(name))
        .map(|(name, bytes)| Weapon::parse(name, bytes))
        .collect::<std::io::Result<Vec<_>>>()?;
    let tanks = data
        .iter()
        .filter(|(name, _)| name.ends_with(".GAS"))
        .map(|(name, bytes)| TankStore::parse(name, bytes))
        .collect::<std::io::Result<Vec<_>>>()?;
    let mut total_weapons = 0;
    let mut total_tanks = 0;
    let mut recovered = 0;
    for id in AircraftId::ALL {
        let aircraft = Aircraft::parse(
            data.get(id.pt())
                .ok_or_else(|| format!("ordnance audit missing {}", id.pt()))?,
        )?;
        let standard = Loadout::new(&aircraft, |name| {
            data.get(name)
                .cloned()
                .ok_or_else(|| std::io::Error::other(format!("ordnance audit missing {name}")))
        })?;
        let rows = standard.editable_stations();
        if !rows
            .windows(2)
            .all(|pair| pair[0].hardpoint < pair[1].hardpoint)
        {
            return Err(format!("{} has duplicate or unordered source rows", id.pt()).into());
        }
        let default_count = aircraft
            .hardpoints
            .iter()
            .filter(|h| h.store.as_deref().is_some_and(|name| name.ends_with(".JT")))
            .count();
        for (index, station) in standard.configuration.stations.iter().enumerate() {
            if index >= default_count && (station.count != 0 || standard.quantities[index] != 0) {
                return Err(format!("{} optional station starts loaded", id.pt()).into());
            }
        }
        for (source, station) in aircraft
            .hardpoints
            .iter()
            .filter(|h| h.store.as_deref().is_some_and(|name| name.ends_with(".JT")))
            .zip(&standard.configuration.stations)
        {
            if source.store.as_deref() != Some(station.weapon.source.as_str())
                || source.count != i32::from(station.count)
            {
                return Err(format!("{} default weapon or quantity changed", id.pt()).into());
            }
        }
        for tank in &standard.configuration.tanks {
            let source = &aircraft.hardpoints[tank.hardpoint];
            let default = source
                .store
                .as_deref()
                .filter(|name| name.ends_with(".GAS"));
            if tank.store.as_ref().map(|s| s.source.as_str()) != default
                || i32::from(tank.quantity) != if default.is_some() { source.count } else { 0 }
            {
                return Err(format!("{} default tank or quantity changed", id.pt()).into());
            }
        }
        for slot in default_count..standard.configuration.stations.len() {
            let mut draft = standard.clone();
            draft.change(slot, 1);
            if draft.quantities[slot] == 0 {
                return Err(format!(
                    "{} optional station cannot restore its retained selection",
                    id.pt()
                )
                .into());
            }
            assert_roundtrip(&draft, &standard, &aircraft, data)?;
        }
        recovered += standard.configuration.stations.len() - default_count;
        // The previous schema wrote only the default weapon prefix.
        let mut old = LoadoutSpec::of(&standard);
        old.stations.truncate(default_count);
        let restored = old.apply(standard.clone(), data, None)?;
        if LoadoutSpec::of(&restored) != LoadoutSpec::of(&standard) {
            return Err(format!("{} old load changed source defaults", id.pt()).into());
        }
        old.tanks = None;
        let restored = old.apply(standard.clone(), data, None)?;
        if LoadoutSpec::of(&restored) != LoadoutSpec::of(&standard) {
            return Err(format!(
                "{} legacy load without tank data changed source defaults",
                id.pt()
            )
            .into());
        }
        assert_roundtrip(&standard, &standard, &aircraft, data)?;
        let mut weapon_cases = 0;
        let mut tank_cases = 0;
        for (hardpoint, source) in aircraft.hardpoints.iter().enumerate() {
            let rule = loading::Station::from_source(source)?;
            for weapon in &weapons {
                // The only reviewed installed gun-pod mount is source-default SUU16.
                if weapon.source == "SUU16.JT" && source.store.as_deref() != Some("SUU16.JT") {
                    continue;
                }
                let expected = rule.allowed_count(
                    loading::Store::weapon(weapon),
                    source.store.as_deref() == Some(weapon.source.as_str()),
                );
                if !(1..32767).contains(&expected) {
                    continue;
                }
                let slot = rows
                    .iter()
                    .find(|r| r.hardpoint == hardpoint)
                    .and_then(|r| r.weapon)
                    .ok_or_else(|| {
                        format!(
                            "{} source station {} hides compatible {}",
                            id.pt(),
                            hardpoint + 1,
                            weapon.source
                        )
                    })?;
                if standard.capacity(slot, weapon) != expected {
                    return Err(format!(
                        "{} source station {} capacity differs for {}",
                        id.pt(),
                        hardpoint + 1,
                        weapon.source
                    )
                    .into());
                }
                let mut draft = standard.clone();
                draft.select(slot, weapon.clone())?;
                assert_roundtrip(&draft, &standard, &aircraft, data)?;
                weapon_cases += 1;
            }
            if let Some(slot) = rows
                .iter()
                .find(|r| r.hardpoint == hardpoint)
                .and_then(|r| r.tank)
            {
                for tank in &tanks {
                    if standard.tank_capacity(slot, tank) <= 0 {
                        continue;
                    }
                    let mut draft = standard.clone();
                    draft.select_tank(slot, tank.clone())?;
                    assert_roundtrip(&draft, &standard, &aircraft, data)?;
                    if let Some(weapon) = rows
                        .iter()
                        .find(|r| r.hardpoint == hardpoint)
                        .and_then(|r| r.weapon)
                    {
                        let mut overlap = draft.clone();
                        overlap.quantities[weapon] = 1;
                        overlap.refresh_equipment()?;
                        if !overlap
                            .validate()
                            .is_err_and(|error| error.to_string().contains("cannot share"))
                            || LoadoutSpec::of(&overlap)
                                .check_for_plane(&aircraft, data, false)
                                .is_ok()
                        {
                            return Err(format!(
                                "{} source station {} accepted overlapping tank and weapon",
                                id.pt(),
                                hardpoint + 1
                            )
                            .into());
                        }
                    }
                    draft.unload_tank(slot)?;
                    assert_roundtrip(&draft, &standard, &aircraft, data)?;
                    tank_cases += 1;
                }
            }
        }
        total_weapons += weapon_cases;
        total_tanks += tank_cases;
        println!(
            "ordnance {}: {} source rows, {} optional weapon rows, {} compatible weapon placements, {} tank placements; defaults and restart passed",
            id.pt(),
            rows.len(),
            standard.configuration.stations.len() - default_count,
            weapon_cases,
            tank_cases
        );
    }
    if recovered != 9 {
        return Err(
            format!("expected 9 reviewed missing weapon stations, found {recovered}").into(),
        );
    }
    println!(
        "ordnance audit: 36 aircraft, {recovered} recovered weapon stations, {total_weapons} weapon placements, {total_tanks} tank placements; passed"
    );
    Ok(())
}

fn assert_roundtrip(
    draft: &Loadout,
    standard: &Loadout,
    aircraft: &Aircraft,
    data: &BTreeMap<String, Vec<u8>>,
) -> AppResult<()> {
    let accepted = LoadoutSpec::of(draft);
    let mut mission = MissionSpec::new("UKR", aircraft.id);
    mission.loadout = Some(accepted.clone());
    let saved = MissionSpec::from_text(&mission.to_text())?;
    if saved != mission {
        return Err("ordnance mission text changed accepted load".into());
    }
    let restored = accepted.apply(standard.clone(), data, None)?;
    if LoadoutSpec::of(&restored) != accepted
        || restored.total_lbs() != draft.total_lbs()
        || restored.ammunition()? != draft.ammunition()?
    {
        return Err(
            "ordnance accepted-load restoration changed selection, mass or ammunition".into(),
        );
    }
    for tank in &restored.configuration.tanks {
        if tank.quantity > 0
            && let Some(Some(slot)) = restored.configuration.hardpoint_slots.get(tank.hardpoint)
            && restored.quantities[*slot] > 0
        {
            return Err("ordnance accepted load overlaps a weapon and tank".into());
        }
    }
    // Max-load catalog selections may exceed MTOW. Check placement and saved
    // state above, then validate a lightweight instance through the lobby rule.
    let mut light = restored;
    light.fuel_lbs = 0.;
    for count in &mut light.quantities {
        *count = (*count).min(1);
    }
    for tank in &mut light.configuration.tanks {
        tank.quantity = tank.quantity.min(1);
    }
    light.refresh_equipment()?;
    light.validate()?;
    tore_sim::combat::live::State::new(light.configuration.clone(), true)?;
    LoadoutSpec::of(&light).check_for_plane(aircraft, data, false)?;
    Ok(())
}
