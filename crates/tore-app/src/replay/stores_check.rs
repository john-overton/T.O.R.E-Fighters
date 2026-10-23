//! Headless replay stores regression and optional offscreen comparison pictures.
use crate::{AppResult, aircraft::Airframe, pylons, render_snapshot::CombatArt};
use std::{collections::BTreeMap, path::Path};
use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_replay::{TimedEvent, stores::StoreTrack};
use tore_session::client::convert::stores::{StoresWatch, store_pose};
use tore_sim::combat::live;
use tore_world::snapshot::{StoreLook, StorePose, store_poses};

pub(crate) fn run() -> AppResult<()> {
    let args: Vec<_> = std::env::args().skip(2).collect();
    let (recording, renders) = match args.as_slice() {
        [] => (None, None),
        [file] => (Some(file), None),
        [file, flag, out] if flag == "--render" => (Some(file), Some(Path::new(out))),
        _ => return Err("--replay-stores-check [RECORDING [--render DIRECTORY]]".into()),
    };
    let assets = crate::reel::load_assets()?;
    let resources = &assets.theater_resources;
    for id in [
        AircraftId::Su27,
        AircraftId::F18,
        AircraftId::F14,
        AircraftId::F15,
        AircraftId::F16C,
        AircraftId::F22,
        AircraftId::F22n,
        AircraftId::Mig29,
        AircraftId::Rafale,
        AircraftId::Mi24,
        AircraftId::A4E,
        AircraftId::Su25,
        AircraftId::F4B,
    ] {
        check_load(id, resources, None)?;
    }
    check_load(AircraftId::F15, resources, Some((6, "MK82.JT")))?;
    check_load(AircraftId::F15, resources, Some((7, "AIM120.JT")))?;
    check_load(AircraftId::F22, resources, Some((8, "AIM120.JT")))?;
    check_load(AircraftId::F22n, resources, Some((8, "AIM120.JT")))?;
    if let Some(file) = recording {
        check_recording(Path::new(file), resources, renders)?;
    }
    println!("Replay stores: PASS");
    Ok(())
}

fn check_shapes(stores: &[StorePose], art: &CombatArt) -> AppResult<()> {
    for store in stores.iter().filter(|s| s.look == StoreLook::Store) {
        if let Some(name) = &store.shape
            && art.shape(name).is_none()
        {
            return Err(format!("replay store shape not loaded: {name}").into());
        }
    }
    Ok(())
}

fn check_load(
    id: AircraftId,
    resources: &BTreeMap<String, Vec<u8>>,
    weapon: Option<(usize, &str)>,
) -> AppResult<()> {
    let profile = Aircraft::parse(resources.get(id.pt()).ok_or("missing aircraft")?)?;
    let mut load = tore_sim::combat::loadout::Loadout::new(&profile, |name| {
        resources
            .get(name)
            .cloned()
            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
    })?;
    let added: &[usize] = match id {
        AircraftId::F15 => &[6, 7],
        AircraftId::F16C => &[9],
        AircraftId::F22 | AircraftId::F22n | AircraftId::Mig29 => &[8],
        AircraftId::Rafale => &[9],
        _ => &[],
    };
    for &row in added {
        let name = match id {
            AircraftId::F15 => "F610.GAS",
            AircraftId::F22 | AircraftId::F22n => "F600.GAS",
            AircraftId::Mig29 => "M396.GAS",
            AircraftId::Rafale => "R528.GAS",
            _ => "F370.GAS",
        };
        let slot = load
            .configuration
            .tanks
            .iter()
            .position(|t| t.hardpoint == row)
            .ok_or("missing added tank row")?;
        load.select_tank(slot, live::TankStore::parse(name, &resources[name])?)?;
    }
    if let Some((row, name)) = weapon {
        let slot = load.configuration.hardpoint_slots[row].ok_or("missing added weapon row")?;
        load.select(
            slot,
            tore_formats::weapons::Weapon::parse(name, &resources[name])?,
        )?;
    }
    let config = load.configuration;
    let loaded = |i: usize| config.stations[i].count > 0;
    let mut rounds: Vec<_> = config
        .stations
        .iter()
        .map(|s| u32::from(s.count) * live::pod_rounds(&s.weapon))
        .collect();
    let full = store_poses(&config, |i| rounds[i], loaded);
    // No projectile catalog at all: every store is unfired at tick zero.
    let mut watch = StoresWatch::default();
    let mut events: Vec<TimedEvent> = watch
        .events(7, &full)
        .into_iter()
        .map(|event| TimedEvent { tick: 0, event })
        .collect();
    let mut expected = vec![full.clone()];
    for (i, station) in config.stations.iter().enumerate() {
        while rounds[i] > 0 && station.rack.location().drawn() && !live::is_gun(&station.weapon) {
            rounds[i] = rounds[i].saturating_sub(live::pod_rounds(&station.weapon));
            let now = store_poses(&config, |j| rounds[j], loaded);
            let tick = expected.len() as u64;
            events.extend(
                watch
                    .events(7, &now)
                    .into_iter()
                    .map(|event| TimedEvent { tick, event }),
            );
            expected.push(now);
        }
    }
    if !added.is_empty() {
        let mut dropped = config.clone();
        for tank in &mut dropped.tanks {
            tank.quantity = 0;
        }
        dropped.refresh_tanks()?;
        let now = store_poses(&dropped, |i| rounds[i], loaded);
        let tick = expected.len() as u64;
        events.extend(
            watch
                .events(7, &now)
                .into_iter()
                .map(|event| TimedEvent { tick, event }),
        );
        expected.push(now);
    }
    let mut art = CombatArt::load(resources)?;
    art.add_shapes(super::store_art::station_shapes(&events), resources);
    check_shapes(&full, &art)?;
    let mut live_art = CombatArt::load(resources)?;
    live_art.add_weapon_shapes(&config, resources);
    let airframe = Airframe::load(resources, id)?;
    let world = crate::terrain::Terrain::for_mission(
        resources,
        "UKR",
        None,
        &crate::terrain::Overrides::default(),
    )?;
    let scenery = crate::scenery::Scenery::build(resources, &world)?;
    let mut state = airframe.start(&world);
    if state.bay_available() {
        state.bay = 1.;
    }
    let track = StoreTrack::from_events(&events);
    // Reverse and forward seeks, including counts, fins, bars and pylons.
    for tick in (0..expected.len()).rev().chain(0..expected.len()) {
        let replayed: Vec<_> = track
            .at(7, tick as u64)
            .ok_or("missing replay stores")?
            .iter()
            .map(store_pose)
            .collect();
        if replayed != expected[tick]
            || pylons::for_flown(&replayed, &state, &airframe, &art, &scenery)
                != pylons::for_flown(&expected[tick], &state, &airframe, &live_art, &scenery)
        {
            return Err(format!("{} replay stores differ at {tick}", id.pt()).into());
        }
    }
    for (index, station) in config
        .stations
        .iter()
        .enumerate()
        .filter(|(_, s)| s.rack.location().drawn() && !live::is_gun(&s.weapon) && s.count > 0)
    {
        let pose = full
            .iter()
            .find(|s| config.hardpoint_slots.get(usize::from(s.hardpoint)) == Some(&Some(index)))
            .ok_or("missing station pose")?;
        let hung = pylons::hung(pose);
        for remaining in 0..station.count {
            let point = station.release_point(u32::from(remaining));
            if !hung.iter().any(|h| {
                h.held
                    && h.centre
                        .iter()
                        .zip(point)
                        .all(|(a, b)| (a - b).abs() < 1e-6)
            }) {
                return Err(format!("{} release not at drawn store: {point:?}", id.pt()).into());
            }
        }
        println!(
            "Replay stores: {} hardpoint={} shape={:?} location={} points={:?}",
            id.pt(),
            pose.hardpoint,
            pose.shape,
            pose.rack.location,
            hung.iter().map(|h| h.centre).collect::<Vec<_>>()
        );
    }
    println!(
        "Replay stores: {} unfired shapes, live vertices, release origins and {} count states agree",
        id.pt(),
        expected.len()
    );
    Ok(())
}

fn check_recording(
    path: &Path,
    resources: &BTreeMap<String, Vec<u8>>,
    renders: Option<&Path>,
) -> AppResult<()> {
    let recording = tore_replay::Recording::open(path)?;
    let track = StoreTrack::from_events(recording.events());
    let mut art = CombatArt::load(resources)?;
    art.add_shapes(super::store_art::shapes(&recording), resources);
    // Exact old viewer loading path, retained only here for before pictures.
    let mut before = CombatArt::load(resources)?;
    before.add_shapes(
        recording.weapons().filter_map(|w| w.shape.as_deref()),
        resources,
    );
    let mut missing_before = std::collections::BTreeSet::new();
    let mut stations = 0;
    for event in recording
        .events()
        .iter()
        .filter(|e| e.event.kind == tore_replay::vocab::kind::STORES_LAYOUT)
    {
        let Some(id) = event.event.subject else {
            continue;
        };
        let Some(stores) = track.at(id, event.tick) else {
            continue;
        };
        let poses: Vec<_> = stores.iter().map(store_pose).collect();
        check_shapes(&poses, &art)?;
        stations += poses.len();
        for pose in &poses {
            println!(
                "Replay recorded stores: aircraft={id} tick={} hardpoint={} shape={:?} look={:?} location={} mount={:?}",
                event.tick, pose.hardpoint, pose.shape, pose.look, pose.rack.location, pose.mount
            );
        }
        for pose in &poses {
            if pose.look == StoreLook::Store
                && let Some(shape) = &pose.shape
                && before.shape(shape).is_none()
            {
                missing_before.insert(shape.clone());
            }
        }
    }
    println!(
        "Replay stores: {} stations={} missing-before={missing_before:?} missing-after=0",
        path.display(),
        stations
    );
    if stations == 0 {
        return Err("recording has no stores to check".into());
    }
    if let Some(out) = renders {
        std::fs::create_dir_all(out)?;
        let world = crate::terrain::Terrain::for_mission(
            resources,
            "UKR",
            None,
            &crate::terrain::Overrides {
                time: Some([13, 0]),
                cloud_altitude: Some(0),
                ..Default::default()
            },
        )?;
        let mut scenery = crate::scenery::Scenery::build(resources, &world)?;
        let mut gpu = pollster::block_on(crate::reel::Gpu::new(&scenery))?;
        let mut drawn = std::collections::BTreeSet::new();
        for info in recording.aircraft() {
            let Some(id) = super::convert::identity(&info.pt) else {
                continue;
            };
            if !drawn.insert(info.pt.clone()) {
                continue;
            }
            let Some(event) = recording.events().iter().find(|e| {
                e.event.subject == Some(info.id)
                    && e.event.kind == tore_replay::vocab::kind::STORES_LAYOUT
            }) else {
                continue;
            };
            let Some(stores) = track.at(info.id, event.tick) else {
                continue;
            };
            let poses: Vec<_> = stores.iter().map(store_pose).collect();
            let airframe = Airframe::load(resources, id)?;
            let mut state = airframe.start(&world);
            state.position[1] += 4000.;
            state.gear = 0.;
            state.flaps = 0.;
            state.yaw = 0.;
            state.pitch = 0.;
            state.bank = 0.;
            for (label, art) in [("before", &before), ("after", &art)] {
                crate::stores_preview::render(
                    &mut gpu,
                    &mut scenery,
                    &world,
                    art,
                    &airframe,
                    &state,
                    &poses,
                    &[],
                    ([-38., -16., 62.], [0., -3., 0.], 40.),
                    &out.join(format!("{}-{label}.png", info.pt)),
                )?;
            }
        }
    }
    Ok(())
}
