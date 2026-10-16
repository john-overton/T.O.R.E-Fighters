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
const RECOVERED_STATIONS: usize = 12;
const TANK_RECORDS: usize = 9;
const WEAPON_PLACEMENTS: usize = 3402;
const TANK_PLACEMENTS: usize = 175;
/// Station and novelty-record pairs the Cheat loading accepts and the normal
/// loading refuses (A2G decision 16).
const NOVELTY_PLACEMENTS: usize = 166;
/// Station and designator-pod pairs the normal rules accept (A2G decision 7).
const POD_PLACEMENTS: usize = 65;

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
    for (source, bytes) in data.iter().filter(|(source, _)| source.ends_with(".JT")) {
        let w = Weapon::parse(source, bytes)?;
        if w.flags & 0x10001 == 0x10001 && w.movement.fuel_t > 0 {
            println!(
                "missile catalog: {} HUD={} name={} seeker={:?} Fox={:?} aspect={} min={} max={} seeker_max={} mass={} burn={} removal={}",
                source,
                w.hud_name,
                w.name,
                w.seeker_kind(),
                w.seeker_kind().fox_stem(),
                w.infrared_all_aspect(),
                w.seeker.zones[1].minimum_range,
                w.seeker.zones[1].maximum_range,
                w.seeker.zones[0].maximum_range,
                w.weight,
                (f64::from(w.movement.fuel_t) - f64::from(w.movement.ignite_t)) / 4.,
                f64::from(w.movement.remove_t) / 4.
            );
        }
    }
    let weapons = data
        .iter()
        .filter(|(name, _)| SUPPORTED_WEAPONS.contains(&name.as_str()))
        .map(|(name, bytes)| Weapon::parse(name, bytes))
        .collect::<std::io::Result<Vec<_>>>()?;
    for (name, donor, _, _, _) in tore_formats::weapons::corrections::SMALL_BOMBS {
        let weapon = weapons
            .iter()
            .find(|w| w.source == name)
            .ok_or("missing small bomb")?;
        println!(
            "bomb variant: {name} donor={donor} weight={} damage={:?} collateral-radius={} collateral-percent={} shape={:?}",
            weapon.weight,
            weapon.damage.by_class,
            weapon.damage.collateral_radius,
            weapon.damage.collateral_percent,
            weapon.shape
        );
    }
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
        extra_stations(&standard, &aircraft, data)?;
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
        for (name, donor, weight, _, shape) in tore_formats::weapons::corrections::SMALL_BOMBS {
            let variant = weapons
                .iter()
                .find(|w| w.source == name)
                .ok_or("missing small bomb")?;
            let donor = weapons
                .iter()
                .find(|w| w.source == donor)
                .ok_or("missing donor bomb")?;
            if variant.weight != weight
                || variant.seeker_kind() != donor.seeker_kind()
                || tore_sim::combat::guided::kind(variant) != tore_sim::combat::guided::kind(donor)
                || tore_sim::combat::bomb_flight::Body::of(variant).steering
                    != tore_sim::combat::bomb_flight::Body::of(donor).steering
                || tore_sim::combat::release_sound(variant)
                    != tore_sim::combat::release_sound(donor)
                || variant.shape.as_deref() != Some(shape)
                || !data.contains_key(shape)
            {
                return Err(format!("invalid derived bomb {name}").into());
            }
            let counts: Vec<_> = (0..standard.configuration.stations.len())
                .map(|slot| standard.capacity(slot, variant))
                .collect();
            if counts.iter().any(|count| *count > 0) {
                println!("bomb carriage: {} {name} capacities={counts:?}", id.pt());
            }
            for slot in 0..standard.configuration.stations.len() {
                let capacity = standard.capacity(slot, donor);
                if capacity > 0 && standard.capacity(slot, variant) < capacity {
                    return Err(
                        format!("{} station {slot} loses {name} donor capacity", id.pt()).into(),
                    );
                }
            }
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
                // The reviewed gun-pod mounts are the source-default SUU16 (the
                // F-4J's) and the F-4B's centreline row.
                if weapon.source == "SUU16.JT"
                    && source.store.as_deref() != Some("SUU16.JT")
                    && tore_formats::aircraft::stations::gun_pod_row(id) != Some(hardpoint)
                {
                    continue;
                }
                let expected = rule.allowed_count(
                    loading::Store::weapon(weapon),
                    source.store.as_deref() == Some(weapon.source.as_str())
                        || tore_formats::weapons::corrections::donor(&weapon.source)
                            .filter(|_| weapon.source.starts_with("GBU"))
                            .is_some_and(|donor| source.store.as_deref() == Some(donor)),
                );
                // One pod a station, whatever the row's own capacity.
                let expected = if weapon.source == "SUU16.JT" {
                    expected.min(1)
                } else {
                    expected
                };
                let r27 = matches!(
                    weapon.source.as_str(),
                    "AA10.JT" | "R27ER.JT" | "R27T.JT" | "R27ET.JT"
                );
                let expected = if r27
                    || matches!((id, hardpoint), (AircraftId::F15, 6) | (AircraftId::F22, 8))
                {
                    rows.iter()
                        .find(|r| r.hardpoint == hardpoint)
                        .and_then(|r| r.weapon)
                        .map_or(0, |slot| standard.capacity(slot, weapon))
                } else {
                    expected
                };
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
        if matches!(
            id,
            AircraftId::Su27 | AircraftId::Su35 | AircraftId::Mig29 | AircraftId::Yak141
        ) {
            let expected: &[&str] =
                if matches!(id, AircraftId::Su27 | AircraftId::Su35 | AircraftId::Mig29) {
                    &["AA10.JT", "R27ER.JT", "R27T.JT", "R27ET.JT"]
                } else {
                    &["AA10.JT", "R27T.JT"]
                };
            for source in expected {
                if !offered.contains(*source) {
                    return Err(format!("{} lacks requested R-27 store {source}", id.pt()).into());
                }
            }
            println!("R-27 loadout {}: {}", id.pt(), expected.join(", "));
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

/// Full requested loads, through the same selector, save and live paths as flight.
fn extra_stations(
    standard: &Loadout,
    aircraft: &Aircraft,
    data: &BTreeMap<String, Vec<u8>>,
) -> AppResult<()> {
    use tore_sim::{attitude::Basis, combat::live, sensors};
    gun_pod_row(standard, aircraft, data)?;
    let (rows, name, count, fuel, shell): (&[usize], _, _, _, _) = match aircraft.id {
        AircraftId::F15 => (&[6, 7], "F610.GAS", 3, 12078., 1296.),
        AircraftId::F16C => (&[9], "F370.GAS", 2, 4884., 524.),
        AircraftId::F22 => (&[8], "F600.GAS", 2, 7920., 850.),
        AircraftId::Mig29 => (&[8], "M396.GAS", 1, 2614., 281.),
        AircraftId::Rafale => (&[9], "R528.GAS", 1, 3485., 374.),
        _ => return Ok(()),
    };
    let mut full = standard.clone();
    let before_ammo = full.ammunition()?;
    let before_fuel = full.external_fuel_lbs();
    let before_shell = full.tank_shell_lbs();
    for &row in rows {
        let slot = full
            .configuration
            .tanks
            .iter()
            .position(|s| s.hardpoint == row)
            .ok_or("derived tank row missing")?;
        if full.configuration.tanks[slot].quantity != 0 {
            return Err("derived row starts loaded".into());
        }
        let tank = TankStore::parse(name, data.get(name).ok_or("derived tank missing")?)?;
        let expected = if aircraft.id == AircraftId::F15 && row == 6
            || matches!(aircraft.id, AircraftId::Mig29 | AircraftId::Rafale)
        {
            1
        } else {
            2
        };
        if full.tank_capacity(slot, &tank) != expected {
            return Err("derived tank capacity".into());
        }
        full.select_tank(slot, tank)?;
    }
    if full.ammunition()? != before_ammo
        || full.external_fuel_lbs() != before_fuel + fuel
        || full.tank_shell_lbs() != before_shell + shell
    {
        return Err("extra tanks changed weapons or mass".into());
    }
    full.validate()?;
    assert_roundtrip(&full, standard, aircraft, data)?;
    if aircraft.id == AircraftId::F15 {
        let mut sparrows = full.clone();
        let slot = sparrows.configuration.hardpoint_slots[4].ok_or("missing fuselage row")?;
        sparrows.select(slot, Weapon::parse("AIM7.JT", &data["AIM7.JT"])?)?;
        if sparrows.quantities[slot] != 4 || sparrows.external_fuel_lbs() != 12078. {
            return Err("four Sparrows cannot coexist with three tanks".into());
        }
        assert_roundtrip(&sparrows, standard, aircraft, data)?;
    }
    // A pre-addition save ends before the appended optional weapon slot.
    let mut old = LoadoutSpec::of(standard);
    if matches!(
        aircraft.id,
        AircraftId::F15 | AircraftId::F16C | AircraftId::F22
    ) {
        old.stations.pop();
    }
    let restored = old.apply(standard.clone(), data, None)?;
    if restored.ammunition()? != before_ammo || restored.external_fuel_lbs() != before_fuel {
        return Err("old loadout did not preserve defaults with empty added rows".into());
    }
    let mut feeding = tore_sim::aircraft_systems::Fuel::new(full.configuration.external_fuel_lbs);
    let mut internal = full.fuel_lbs;
    feeding.consume(&mut internal, before_fuel + fuel);
    if internal != full.fuel_lbs || feeding.external_lbs() != 0. {
        return Err("extra tanks did not feed".into());
    }
    let launcher = live::Launcher {
        position: [0., 5000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 500.,
        velocity: [0., 0., 500.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    };
    let mut state = live::State::new(full.configuration.clone(), true)?;
    let mass = state.own().payload_lbs();
    let ammo = state.own().ammo.clone();
    state.command(0, live::Command::JettisonTanks, launcher);
    if state.own().external_fuel_lbs().iter().sum::<f64>() != 0.
        || state.own().ammo != ammo
        || (mass - state.own().payload_lbs() - fuel - shell - before_fuel - before_shell).abs()
            > 1e-6
    {
        return Err("extra tank jettison lost weapons or retained tank mass".into());
    }
    let weapons: &[(usize, &str, u16)] = match aircraft.id {
        AircraftId::F15 => &[(6, "MK82.JT", 3), (6, "GBU12.JT", 3), (6, "MK84.JT", 2)],
        AircraftId::F16C => &[(9, "MK82.JT", 2), (9, "AGM65G.JT", 2)],
        AircraftId::F22 => &[(8, "AIM120.JT", 4), (8, "AIM9M.JT", 4), (8, "AIM9X.JT", 4)],
        _ => &[],
    };
    for &(row, weapon, quantity) in weapons {
        let slot = standard.configuration.hardpoint_slots[row].ok_or("wet weapon slot missing")?;
        let mut armed = full.clone();
        armed
            .select(slot, Weapon::parse(weapon, &data[weapon])?)
            .map_err(|error| format!("{} row {row} {weapon}: {error}", aircraft.id.pt()))?;
        if armed.quantities[slot] != quantity
            || armed
                .configuration
                .tanks
                .iter()
                .any(|t| t.hardpoint == row && t.quantity > 0)
        {
            return Err(format!(
                "{} row {row} {weapon}: capacity or tank exclusion",
                aircraft.id.pt()
            )
            .into());
        }
        assert_roundtrip(&armed, standard, aircraft, data)?;
        let mut state = live::State::new(armed.configuration.clone(), true)?;
        state.own_mut().selected = slot;
        state.command(0, live::Command::Jettison, launcher);
        if state.own().rounds(slot) != 0 {
            return Err("wet selected store did not jettison".into());
        }
        let tank_slot = armed
            .configuration
            .tanks
            .iter()
            .position(|t| t.hardpoint == row)
            .unwrap();
        armed.select_tank(tank_slot, TankStore::parse(name, &data[name])?)?;
        if armed.quantities[slot] != 0 {
            return Err("tank selection retained weapons".into());
        }
    }
    if matches!(aircraft.id, AircraftId::F15 | AircraftId::F22) {
        let row = if aircraft.id == AircraftId::F15 { 6 } else { 8 };
        let slot = standard.configuration.hardpoint_slots[row].unwrap();
        let rejected: &[&str] = if aircraft.id == AircraftId::F15 {
            &["AIM120.JT", "AGM65G.JT", "LAU61.JT"]
        } else {
            &["MK82.JT", "AGM65G.JT", "AA11.JT"]
        };
        for name in rejected {
            if standard.capacity(slot, &Weapon::parse(name, &data[*name])?) != 0 {
                return Err(format!("unexpected derived weapon permission {name}").into());
            }
        }
        let accepted: Vec<_> = SUPPORTED_WEAPONS
            .iter()
            .filter_map(|name| {
                let w = Weapon::parse(name, &data[*name]).ok()?;
                let capacity = standard.capacity(slot, &w);
                (capacity > 0).then(|| format!("{name}:{capacity}"))
            })
            .collect();
        println!(
            "derived weapons {} row {row}: {}",
            aircraft.id.pt(),
            accepted.join(", ")
        );
    }
    println!(
        "derived stations {}: {count} extra tanks, {fuel} lb fuel, {shell} lb shells; feed, jettison, save and old load passed",
        aircraft.id.pt()
    );
    Ok(())
}

/// The F-4B's centreline row takes the SUU-16 gun pod in place of its stores
/// (2026-10-16): one pod, 1,702 lb counted in the weight whether it holds
/// rounds or not, the row's own stores back when the pod is taken off, the
/// saved load and the lobby rule agreeing, and the pod fired and jettisoned
/// through the live state. No other row and no other aircraft but the F-4J's
/// own fixed pod takes one.
fn gun_pod_row(
    standard: &Loadout,
    aircraft: &Aircraft,
    data: &BTreeMap<String, Vec<u8>>,
) -> AppResult<()> {
    use tore_sim::{attitude::Basis, combat::live, sensors};
    let pod = Weapon::parse("SUU16.JT", &data["SUU16.JT"])?;
    let Some(row) = tore_formats::aircraft::stations::gun_pod_row(aircraft.id) else {
        // Elsewhere the pod loads only on a source row that names it.
        for slot in 0..standard.configuration.stations.len() {
            let own = standard.configuration.stations[slot].weapon.source == "SUU16.JT";
            if (standard.capacity(slot, &pod) > 0) != own {
                return Err(
                    format!("{} station {slot} gun pod permission", aircraft.id.pt()).into(),
                );
            }
        }
        return Ok(());
    };
    let slot = standard.configuration.hardpoint_slots[row].ok_or("pod row has no weapon slot")?;
    let default = standard.configuration.stations[slot].weapon.source.clone();
    if standard.configuration.gun_pod(slot).is_some()
        || standard.configuration.gun_pods.len() != 1
        || standard.quantities[slot] == 0
        || default == "SUU16.JT"
        || standard.capacity(slot, &pod) != 1
    {
        return Err("the F-4B starts with the pod or cannot take one".into());
    }
    for other in (0..standard.configuration.stations.len()).filter(|s| *s != slot) {
        if standard.capacity(other, &pod) != 0 {
            return Err("the gun pod loads off its centreline row".into());
        }
    }
    let mut armed = standard.clone();
    armed.select(slot, pod.clone())?;
    let pod_lbs = f64::from(armed.configuration.gun_pods[0].weight_lbs);
    let swapped_out = f64::from(standard.configuration.stations[slot].weapon.weight)
        * f64::from(standard.quantities[slot]);
    if armed.quantities[slot] != 1
        || armed.configuration.gun_pod(slot).map(|p| p.quantity) != Some(1)
        || pod_lbs != 1702.
        || (armed.total_lbs() - standard.total_lbs() - (pod_lbs - swapped_out)).abs() > 1e-6
    {
        return Err("the pod's weight or count is wrong".into());
    }
    // 1,200 rounds are 600 logical units, the F-4J's interpretation.
    if armed.ammunition()?[slot] != 600 {
        return Err("the pod does not hold 600 logical units (1,200 rounds)".into());
    }
    armed.validate()?;
    assert_roundtrip(&armed, standard, aircraft, data)?;
    // Taking the pod off restores the row's stores and the pod's mass goes.
    let mut back = armed.clone();
    back.select(slot, standard.configuration.stations[slot].weapon.clone())?;
    if back.quantities[slot] != standard.quantities[slot]
        || back.configuration.gun_pod(slot).is_some()
        || back.configuration.gun_pods[0].quantity != 0
        || back.total_lbs() != standard.total_lbs()
    {
        return Err("taking the pod off left its mass or lost the stores".into());
    }
    let mut none = armed.clone();
    none.change(slot, -1);
    if none.configuration.gun_pods[0].quantity != 0 || none.total_lbs() >= armed.total_lbs() {
        return Err("removing the pod left its mass".into());
    }
    // Guns only keeps the pod, and a tank on the row refuses it.
    let mut guns = armed.clone();
    guns.restrict_to_guns();
    if guns.quantities[slot] != 1 || guns.quantities.iter().sum::<u16>() != 1 {
        return Err("Guns only does not keep exactly the gun pod".into());
    }
    let mut cheat = standard.clone();
    cheat.cheat = true;
    if cheat.capacity(slot, &pod) != 1 {
        return Err("Cheat loading gives more than one pod".into());
    }
    // In flight: Tab's gun is the pod, it fires from the pod's nose under the
    // aircraft and is jettisoned with its mass.
    let launcher = live::Launcher {
        position: [0., 5000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 500.,
        velocity: [0., 0., 500.],
        bay_ready: true,
        radar_power: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    };
    let mut state = live::State::new(armed.configuration.clone(), true)?;
    if live::key_gun_station(&armed.configuration, |i| state.own().rounds(i)) != Some(slot) {
        return Err("Tab does not pick the gun pod".into());
    }
    let station = &armed.configuration.stations[slot];
    let muzzle = station.muzzle();
    if !(muzzle[1] < -1. && muzzle[2] > station.mount[2] && muzzle[0].abs() < 1e-9) {
        return Err(format!("the gun pod's muzzle {muzzle:?} is not under the aircraft").into());
    }
    // Held trigger: the first round starts at the pod's nose, rounds keep
    // coming, and the pod and its mass stay when it has fired.
    let mut firing = state.clone();
    firing.own_mut().selected = slot;
    let expected = std::array::from_fn::<f64, 3, _>(|i| {
        launcher.position[i]
            + launcher.basis.right[i] * muzzle[0]
            + launcher.basis.up[i] * muzzle[1]
            + launcher.basis.forward[i] * muzzle[2]
    });
    let mut first = None;
    for _ in 0..60 {
        firing.step(
            &[live::OwnshipInput {
                aircraft: 0,
                held: true,
                launcher,
            }],
            |_, _| -10000.,
        );
        if first.is_none() {
            first = firing.projectiles.first().map(|p| p.previous);
        }
    }
    let Some(first) = first else {
        return Err("the gun pod fired nothing".into());
    };
    if first
        .iter()
        .zip(expected)
        .any(|(a, b)| (a - b).abs() > 1e-6)
        || firing.projectiles.len() < 5
        || firing.own().rounds(slot) >= 600
        || firing.own().payload_lbs() != state.own().payload_lbs()
    {
        return Err(format!(
            "the gun pod's rounds did not leave its nose: first {first:?}, wanted {expected:?}"
        )
        .into());
    }
    let payload = state.own().payload_lbs();
    state.own_mut().selected = slot;
    state.command(0, live::Command::Jettison, launcher);
    if state.own().rounds(slot) != 0 || payload - state.own().payload_lbs() < pod_lbs - 1e-6 {
        return Err("the gun pod did not jettison with its mass".into());
    }
    println!(
        "gun pod {}: row {row} takes one SUU-16 in place of {default} x{}; weight, 600 logical units (1,200 rounds), Tab, Guns only, save, lobby rule, firing from the pod's nose and jettison passed",
        aircraft.id.pt(),
        standard.quantities[slot]
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
        assert_eq!(SUPPORTED_WEAPONS.len(), 78);
    }
}
