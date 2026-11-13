//! TOR-13 retail guard (John, 2026-11-12): the Tornado IDS comes in two
//! versions on the one `TOR.PT` record. The Combat version keeps the retail
//! `MIG31.ECM` (a jammer, 60 chaff, 60 flares) and carries the Sky Shadow ECM
//! pod on its left outer wing pylon and the BOZ-107 chaff and flare pod on its
//! right, fixed (they are no hardpoint row, so no load can change them); the
//! Clean version has neither pod, no jammer and no chaff or flares. The check
//! reads the user's own install and skips quietly without it; it reads
//! records only and no retail bytes are committed.
use std::path::PathBuf;
use tore_formats::aircraft::{Aircraft, AircraftId, stations};
use tore_import::MediaSource;
use tore_sim::combat::{hardpoints::Location, live::Configuration, loadout::Loadout};

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn the_combat_tornado_carries_its_two_pods_and_the_clean_one_none() {
    let root = install();
    let Ok(media) = MediaSource::detect(&root) else {
        eprintln!("skipped: no retail install at {}", root.display());
        return;
    };
    let archives: Vec<_> = ["FA_1.LIB", "FA_2.LIB", "FA_4B.LIB", "FA_4D.LIB"]
        .iter()
        .filter_map(|name| media.archive(name).ok())
        .collect();
    let read = |name: &str| -> std::io::Result<Vec<u8>> {
        archives
            .iter()
            .find_map(|library| library.read(name).ok())
            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
    };
    let bytes = read(AircraftId::Tornado.pt()).expect("TOR.PT");
    let load = |id: AircraftId| {
        let aircraft = Aircraft::parse_as(&bytes, id).unwrap();
        let config = Configuration::from_source(&aircraft, &read).unwrap();
        let loadout = Loadout::new(&aircraft, &read).unwrap();
        (aircraft, config, loadout)
    };
    let (combat, combat_config, combat_load) = load(AircraftId::Tornado);
    let (clean, clean_config, clean_load) = load(AircraftId::TornadoClean);
    assert_eq!(combat.id, AircraftId::Tornado);
    assert_eq!(clean.id, AircraftId::TornadoClean);

    // The Combat pair: BOZ-107 right, Sky Shadow left, on the wing, beyond
    // the nine source rows, swivelling with it.
    let pods = &combat_config.fixed_pods;
    let records: Vec<_> = pods.iter().map(|pod| pod.record.as_str()).collect();
    assert_eq!(records, ["BOZ107", "SKYSHADOW"]);
    assert!(pods[0].mount[0] > 10. && pods[1].mount[0] < -10.);
    assert_eq!(pods[0].mount[0], -pods[1].mount[0]);
    for pod in pods {
        assert!(pod.hardpoint >= combat.hardpoints.len(), "{pod:?}");
        assert_eq!(pod.rack.location(), Location::Wing);
        assert!(tore_sim::combat::swing::swivels(
            AircraftId::Tornado,
            pod.hardpoint
        ));
        // Hung from the wing's own underside, not at the record's height.
        assert!(pod.rack.over.iter().any(Option::is_some), "{pod:?}");
    }
    // Drawn as pods with their own authored models and real sizes.
    let loads = crate::snapshot::station_stores(&combat_config, |_| 0, |_| false);
    let poses = crate::snapshot::store_poses_of(&combat_config, &loads);
    let drawn: Vec<_> = poses
        .iter()
        .filter(|pose| pose.look == crate::snapshot::StoreLook::Pod)
        .map(|pose| (pose.equipment_pod(), pose.body, pose.size().lug_aft))
        .collect();
    assert_eq!(drawn.len(), 2);
    for (record, body, lug_aft) in drawn {
        let pod = stations::authored_pod(record.unwrap()).unwrap();
        assert_eq!(body, [pod.length, pod.diameter]);
        // Hung from lugs a third of its length aft of its middle (TOR-14b).
        assert_eq!(lug_aft, pod.lug_aft);
        assert!((lug_aft - pod.length / 3.).abs() < 1e-9);
    }
    // Carried mass: exactly the two pods' 441 and 717 lb more than the Clean
    // version, in the fixed external equipment (and so the flight's mass).
    assert_eq!(
        combat_config.external_equipment_lbs - clean_config.external_equipment_lbs,
        441 + 717
    );
    assert_eq!(
        combat_config.fixed_external_equipment_lbs - clean_config.fixed_external_equipment_lbs,
        441 + 717
    );
    // The Combat version keeps the retail jammer bundle and its dispenser
    // outlet is the BOZ-107's aft face; chaff and flares come from there.
    assert_eq!(
        (combat_config.ecm.chaff[0], combat_config.ecm.flare[0]),
        (60, 60)
    );
    assert!(combat_config.sensors.jammer.is_some());
    let exit = combat_config.dispenser_exit().expect("the BOZ-107's exit");
    // The pod hangs from lugs toward its rear (TOR-14b): its aft face is a
    // sixth of its length aft of the pylon.
    assert!(exit[0] > 10. && (exit[2] - (pods[0].mount[2] - 13.12 / 6.)).abs() < 0.1);
    // The Clean version: no pods, no jammer, no chaff, no flares, no outlet.
    assert!(clean_config.fixed_pods.is_empty());
    assert_eq!(
        clean_config.ecm,
        tore_formats::weapons::Countermeasures::NONE
    );
    assert!(clean_config.ecm_hardpoint.is_none());
    assert!(clean_config.sensors.jammer.is_none());
    assert_eq!(clean_config.dispenser_exit(), None);
    // Everything else is the same record: the same stations, the same loads,
    // the same radar and sights.
    assert_eq!(combat.hardpoints.len(), clean.hardpoints.len());
    assert_eq!(combat_config.stations.len(), clean_config.stations.len());
    for (one, other) in combat_config.stations.iter().zip(&clean_config.stations) {
        assert_eq!(
            (&one.weapon.source, one.count, one.mount),
            (&other.weapon.source, other.count, other.mount)
        );
    }
    assert_eq!(
        combat_config.sensors.radar.is_some(),
        clean_config.sensors.radar.is_some()
    );
    let rows = |load: &Loadout| -> Vec<usize> {
        load.editable_stations()
            .iter()
            .map(|r| r.hardpoint)
            .collect()
    };
    assert_eq!(rows(&combat_load), rows(&clean_load));
    assert!(
        rows(&combat_load)
            .iter()
            .all(|row| *row < combat.hardpoints.len()),
        "Load Ordnance offers no pod station"
    );
}
