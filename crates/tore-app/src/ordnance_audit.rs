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
    loadout::{Loadout, NOVELTY_WEAPONS, SUPPORTED_WEAPONS},
    sight::{DESIGNATOR_PODS, DesignatorStore},
};
use tore_world::mission::{LoadoutSpec, MissionSpec};

/// What the audit must find: a short count of anything fails the run, so a
/// partial import cannot pass with fewer checks. Raise these on purpose, and
/// record why in `docs/baselines/ordnance-availability.md`, when the supported
/// weapon list, the tank records or the aircraft change.
const AIRCRAFT: usize = 36;
const RECOVERED_STATIONS: usize = 9;
const TANK_RECORDS: usize = 4;
const WEAPON_PLACEMENTS: usize = 3146;
const TANK_PLACEMENTS: usize = 146;
/// Station and novelty-record pairs the Cheat loading accepts and the normal
/// loading refuses (A2G decision 16).
const NOVELTY_PLACEMENTS: usize = 160;
/// Station and designator-pod pairs the normal rules accept (A2G decision 7).
const POD_PLACEMENTS: usize = 130;

pub fn validate(data: &BTreeMap<String, Vec<u8>>) -> AppResult<()> {
    if AircraftId::ALL.len() != AIRCRAFT {
        return Err(format!(
            "ordnance audit expects {AIRCRAFT} aircraft, the roster has {}",
            AircraftId::ALL.len()
        )
        .into());
    }
    if let Some(name) = SUPPORTED_WEAPONS
        .iter()
        .find(|name| !data.contains_key(**name))
    {
        return Err(format!("ordnance audit missing supported weapon record {name}").into());
    }
    let weapons = data
        .iter()
        .filter(|(name, _)| SUPPORTED_WEAPONS.contains(&name.as_str()))
        .map(|(name, bytes)| Weapon::parse(name, bytes))
        .collect::<std::io::Result<Vec<_>>>()?;
    let novelty = NOVELTY_WEAPONS
        .iter()
        .map(|name| {
            let bytes = data
                .get(*name)
                .ok_or_else(|| format!("ordnance audit missing novelty weapon record {name}"))?;
            Ok(Weapon::parse(name, bytes)?)
        })
        .collect::<AppResult<Vec<_>>>()?;
    let pods = DESIGNATOR_PODS
        .iter()
        .map(|name| {
            let bytes = data
                .get(*name)
                .ok_or_else(|| format!("ordnance audit missing designator pod record {name}"))?;
            Ok(DesignatorStore::parse(name, bytes)?)
        })
        .collect::<AppResult<Vec<_>>>()?;
    let tanks = data
        .iter()
        .filter(|(name, _)| name.ends_with(".GAS"))
        .map(|(name, bytes)| TankStore::parse(name, bytes))
        .collect::<std::io::Result<Vec<_>>>()?;
    if weapons.len() != SUPPORTED_WEAPONS.len() {
        return Err(format!(
            "ordnance audit read {} weapon records, expected {}",
            weapons.len(),
            SUPPORTED_WEAPONS.len()
        )
        .into());
    }
    if tanks.len() != TANK_RECORDS {
        return Err(format!(
            "ordnance audit read {} tank records, expected {TANK_RECORDS}",
            tanks.len()
        )
        .into());
    }
    let mut total_weapons = 0;
    let mut total_tanks = 0;
    let mut novelty_cases = 0;
    let mut pod_cases = 0;
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
        // What starts loaded: the source defaults, except where T.O.R.E's
        // default load differs (the A-4E's real three tanks, PY3).
        let records = tore_sim::combat::hardpoints::default_records(&aircraft);
        for ((source, record), station) in aircraft
            .hardpoints
            .iter()
            .zip(&records)
            .filter(|(h, _)| h.store.as_deref().is_some_and(|name| name.ends_with(".JT")))
            .zip(&standard.configuration.stations)
        {
            let count = if record.store == source.store {
                record.count
            } else {
                0
            };
            if source.store.as_deref() != Some(station.weapon.source.as_str())
                || count != i32::from(station.count)
            {
                return Err(format!("{} default weapon or quantity changed", id.pt()).into());
            }
        }
        for tank in &standard.configuration.tanks {
            let source = &records[tank.hardpoint];
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
        novelty_cases += check_novelty(&standard, &novelty, id.pt())?;
        pod_cases += check_pods(&standard, &pods, &aircraft, data)?;
        let mut weapon_cases = 0;
        let mut offered = std::collections::BTreeSet::new();
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
                offered.insert(weapon.source.clone());
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
        let (normal_catalog, cheat_catalog) = check_catalog(&standard, &offered, data, id.pt())?;
        total_weapons += weapon_cases;
        total_tanks += tank_cases;
        println!(
            "ordnance {}: {} source rows, {} optional weapon rows, {} compatible weapon placements, {} tank placements; defaults and restart passed; catalog {normal_catalog} normal, {cheat_catalog} Cheat",
            id.pt(),
            rows.len(),
            standard.configuration.stations.len() - default_count,
            weapon_cases,
            tank_cases
        );
    }
    if recovered != RECOVERED_STATIONS {
        return Err(format!(
            "expected {RECOVERED_STATIONS} reviewed missing weapon stations, found {recovered}"
        )
        .into());
    }
    if total_weapons != WEAPON_PLACEMENTS || total_tanks != TANK_PLACEMENTS {
        return Err(format!(
            "ordnance audit checked {total_weapons} weapon and {total_tanks} tank placements, expected {WEAPON_PLACEMENTS} and {TANK_PLACEMENTS}: a record or station is missing or new"
        )
        .into());
    }
    if novelty_cases != NOVELTY_PLACEMENTS {
        return Err(format!(
            "ordnance audit checked {novelty_cases} Cheat-only placements, expected {NOVELTY_PLACEMENTS}"
        )
        .into());
    }
    println!(
        "ordnance audit: {AIRCRAFT} aircraft, {recovered} recovered weapon stations, {total_weapons} weapon placements, {total_tanks} tank placements, {} weapon and {} tank records; passed",
        weapons.len(),
        tanks.len()
    );
    if pod_cases != POD_PLACEMENTS {
        return Err(format!(
            "ordnance audit checked {pod_cases} designator pod placements, expected {POD_PLACEMENTS}"
        )
        .into());
    }
    println!(
        "ordnance audit: {} designator pods, {pod_cases} pod placements, one to an aircraft, loaded and restored through the saved mission; passed",
        pods.len()
    );
    println!(
        "ordnance audit: {} novelty records, {novelty_cases} Cheat-only placements, refused without Cheat loading, listed only in the Cheat catalog; passed",
        novelty.len()
    );
    Ok(())
}

/// The Load Ordnance catalog in both modes against the station rules: the
/// normal catalog offers exactly the supported weapons some station accepts
/// (`offered`) and never a novelty bomb; Cheat loading offers every supported
/// weapon and both novelty bombs, and the novelty bombs only there.
fn check_catalog(
    standard: &Loadout,
    offered: &std::collections::BTreeSet<String>,
    data: &BTreeMap<String, Vec<u8>>,
    pt: &str,
) -> AppResult<(usize, usize)> {
    let normal = crate::ordnance::catalog_sources(standard, data, false)?;
    let cheat = crate::ordnance::catalog_sources(standard, data, true)?;
    let normal_set: std::collections::BTreeSet<_> = normal.iter().cloned().collect();
    if &normal_set != offered || normal.len() != normal_set.len() {
        return Err(format!(
            "{pt} normal catalog lists {normal:?}, the station rules accept {offered:?}"
        )
        .into());
    }
    for name in NOVELTY_WEAPONS {
        if normal.iter().any(|n| n == name) {
            return Err(format!("{pt} normal catalog lists {name}").into());
        }
        if !cheat.iter().any(|n| n == name) {
            return Err(format!("{pt} Cheat catalog lacks {name}").into());
        }
    }
    // Cheat loading lists every supported weapon, other aircraft's too; the
    // gun pod only where the aircraft has one.
    for name in SUPPORTED_WEAPONS {
        let listed = *name != "SUU16.JT" || !standard.configuration.gun_pods.is_empty();
        if cheat.iter().any(|n| n == name) != listed {
            return Err(format!("{pt} Cheat catalog and {name} disagree").into());
        }
    }
    Ok((normal.len(), cheat.len()))
}

/// The laser designator pods (decision 7) on every station the normal rules
/// give them: each loads in place of the station's weapon, the aircraft's one
/// pod moves when a second is loaded, the pod survives the saved mission and
/// goes when a weapon takes the station back, and the normal catalog lists
/// exactly the pods some station takes. Returns the placements.
fn check_pods(
    standard: &Loadout,
    pods: &[DesignatorStore],
    aircraft: &Aircraft,
    data: &BTreeMap<String, Vec<u8>>,
) -> AppResult<usize> {
    let pt = aircraft.id.pt();
    let mut placements = 0;
    let mut offered = std::collections::BTreeSet::new();
    for pod in pods {
        let slots: Vec<usize> = (0..standard.configuration.stations.len())
            .filter(|slot| standard.designator_capacity(*slot, pod) > 0)
            .collect();
        for &slot in &slots {
            let mut draft = standard.clone();
            draft.load_designator(slot, pod.clone())?;
            if draft.designator(slot) != Some(pod) || draft.quantities[slot] != 0 {
                return Err(format!("{pt} station {slot} did not take {}", pod.source).into());
            }
            assert_roundtrip(&draft, standard, aircraft, data)?;
            // A weapon taking the station back removes the pod.
            let weapon = draft.configuration.stations[slot].weapon.clone();
            let mut back = draft.clone();
            if back.capacity(slot, &weapon) > 0 {
                back.change(slot, 1);
                if back.designator(slot).is_some() {
                    return Err(
                        format!("{pt} station {slot} kept {} under a weapon", pod.source).into(),
                    );
                }
            }
            // One pod to an aircraft: the next one moves it.
            for other in pods.iter().filter(|other| other.source != pod.source) {
                let mut second = draft.clone();
                if second.load_designator(slot, other.clone()).is_ok()
                    && (second.configuration.designator_pods.len() != 1
                        || second.designator(slot) != Some(other))
                {
                    return Err(format!("{pt} carries more than one designator pod").into());
                }
            }
            placements += 1;
        }
        if !slots.is_empty() {
            offered.insert(pod.source.clone());
        }
    }
    let listed = crate::ordnance::designator_sources(standard, data, false)?;
    if listed
        .iter()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>()
        != offered
    {
        return Err(format!(
            "{pt} normal catalog lists pods {listed:?}, the rules accept {offered:?}"
        )
        .into());
    }
    let cheat = crate::ordnance::designator_sources(standard, data, true)?;
    if cheat.len() != pods.len() {
        return Err(format!("{pt} Cheat catalog lists pods {cheat:?}").into());
    }
    Ok(placements)
}

/// Boulder and Moose (decision 16): the normal Load Ordnance rules give them
/// no capacity on any station and refuse a selection; Cheat loading gives
/// them capacity on the stations it allows, and a loadout carrying one passes
/// the loadout's own checks only with Cheat on. Returns the Cheat placements.
fn check_novelty(standard: &Loadout, novelty: &[Weapon], pt: &str) -> AppResult<usize> {
    let mut placements = 0;
    let mut cheat = standard.clone();
    cheat.cheat = true;
    for weapon in novelty {
        for slot in 0..standard.configuration.stations.len() {
            if standard.capacity(slot, weapon) != 0 {
                return Err(format!(
                    "{pt} station {slot} gives {} capacity without Cheat loading",
                    weapon.source
                )
                .into());
            }
            let mut refused = standard.clone();
            if refused.select(slot, weapon.clone()).is_ok() {
                return Err(format!(
                    "{pt} station {slot} accepted {} without Cheat loading",
                    weapon.source
                )
                .into());
            }
            let capacity = cheat.capacity(slot, weapon);
            if !(1..32767).contains(&capacity) {
                continue;
            }
            let mut draft = cheat.clone();
            draft.select(slot, weapon.clone())?;
            draft.fuel_lbs = 0.;
            for count in &mut draft.quantities {
                *count = (*count).min(1);
            }
            draft.refresh_equipment()?;
            draft.validate()?;
            draft.cheat = false;
            if !draft
                .validate()
                .is_err_and(|e| e.to_string().contains("Cheat loading only"))
            {
                return Err(format!(
                    "{pt} station {slot} kept {} when Cheat loading went off",
                    weapon.source
                )
                .into());
            }
            placements += 1;
        }
    }
    Ok(placements)
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
    // The profile's saved loadout (Quick Mission, item 19) keeps the same load.
    if super::quick_mission::loadout_memory::through_the_file(&accepted)? != accepted {
        return Err("the saved loadout file changed an accepted load".into());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_import_missing_a_supported_weapon_record_fails_the_audit() {
        let error = validate(&BTreeMap::new()).unwrap_err().to_string();
        assert!(error.contains("missing supported weapon record"), "{error}");
        assert!(error.contains(SUPPORTED_WEAPONS[0]), "{error}");
    }

    #[test]
    fn the_pinned_counts_match_the_reviewed_roster() {
        assert_eq!(AircraftId::ALL.len(), AIRCRAFT);
        assert_eq!(SUPPORTED_WEAPONS.len(), 72);
    }
}
