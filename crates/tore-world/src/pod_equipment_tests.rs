//! EQ-2 retail guard (2026-11-06): the designator, FLIR and jammer pods the
//! import keeps as fixed external equipment name no shape, so each draws as a
//! fitted body. The check reads the user's own install (the `gameassets` link
//! or `TORE_GAME_DIR`) through the importer's media source and skips quietly
//! without it. It reads records only; no retail bytes are committed.
//!
//! Evidence: docs/formats/objects-and-shapes.md, "External equipment pods name
//! no shape", and docs/spec/stores-on-pylons.md.
use std::path::PathBuf;
use tore_formats::aircraft::{Aircraft, AircraftId};
use tore_import::MediaSource;
use tore_sim::combat::live::Configuration;

fn install() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology"),
        PathBuf::from,
    )
}

#[test]
fn a_fixed_pod_record_names_no_shape_and_draws_a_fitted_body() {
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

    // The pod carriers among the imported roster. Each draws one fitted body.
    let cases = [
        (AircraftId::A10, "PAVESPK.SEE"),
        (AircraftId::F16C, "PAVESPK.SEE"),
        (AircraftId::Rafale, "AAS38.SEE"),
        (AircraftId::Av8, "ALQ167.ECM"),
    ];
    let mut seen = Vec::new();
    for (id, record) in cases {
        let bytes = read(id.pt()).unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        let aircraft =
            Aircraft::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        let config = Configuration::from_source(&aircraft, &read)
            .unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        assert!(
            config.fixed_pods.iter().any(|pod| pod.record == record),
            "{} carries {record} as a fixed pod",
            id.pt()
        );
        // The retail pods carry a weight and the equipment flag bit 1; every
        // fixed pod the rule keeps must too, so a weight-zero sensor never
        // becomes a drawn body.
        for pod in &config.fixed_pods {
            let bytes = read(&pod.record).unwrap();
            let equipment = tore_formats::aircraft::Equipment::parse(&pod.record, &bytes)
                .unwrap_or_else(|error| panic!("{}: {error}", pod.record));
            let flag = equipment
                .fields
                .get("flags")
                .and_then(|f| f.number().ok())
                .unwrap_or(0);
            assert!(
                equipment.number("weight") > 0. && flag & 1 != 0,
                "{}: {} is a pod",
                id.pt(),
                pod.record
            );
            seen.push(pod.record.clone());
        }
        // Each fixed pod has no shape and draws as a pod body.
        let loads = crate::snapshot::station_stores(&config, |_| 0, |_| false);
        let poses = crate::snapshot::store_poses_of(&config, &loads);
        let pods: Vec<_> = poses
            .iter()
            .filter(|pose| pose.look == crate::snapshot::StoreLook::Pod)
            .collect();
        assert_eq!(
            pods.len(),
            config.fixed_pods.len(),
            "{} draws one pod body a fixed pod",
            id.pt()
        );
        assert!(pods.iter().all(|pose| pose.shape.is_none()));
    }
    for record in ["PAVESPK.SEE", "AAS38.SEE", "ALQ167.ECM"] {
        assert!(seen.iter().any(|name| name == record), "drew {record}");
    }
    println!("fixed pods drawn: {seen:?}");
}
