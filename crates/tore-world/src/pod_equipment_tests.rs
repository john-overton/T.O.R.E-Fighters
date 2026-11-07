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

    // The pod carriers among the imported roster. John 2026-11-07: only a
    // pod on a real station draws. The A-10's and AV-8B's rows sit at the
    // reference point and the F-16C's at [0, 0, 50], inside the airframe:
    // placeholders, drawn as nothing. The Rafale's AAS-38 row is 20 units
    // under the belly, on the centreline station, and draws where it did.
    let cases = [
        (AircraftId::A10, "PAVESPK.SEE", None),
        (AircraftId::F16C, "PAVESPK.SEE", None),
        (AircraftId::Rafale, "AAS38.SEE", Some([0., -20. / 3., 0.])),
        (AircraftId::Av8, "ALQ167.ECM", None),
    ];
    let mut seen = Vec::new();
    for (id, record, drawn_at) in cases {
        let bytes = read(id.pt()).unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        let aircraft =
            Aircraft::parse(&bytes).unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        let config = Configuration::from_source(&aircraft, &read)
            .unwrap_or_else(|error| panic!("{}: {error}", id.pt()));
        // The record stays on its hardpoint, drawn or not.
        assert!(
            aircraft
                .hardpoints
                .iter()
                .any(|h| h.store.as_deref() == Some(record)),
            "{} carries {record}",
            id.pt()
        );
        let drawn: Vec<_> = config
            .fixed_pods
            .iter()
            .filter(|pod| pod.record == record)
            .collect();
        match drawn_at {
            Some(mount) => {
                assert_eq!(drawn.len(), 1, "{} draws {record}", id.pt());
                for (got, want) in drawn[0].mount.iter().zip(mount) {
                    assert!(
                        (got - want).abs() < 1e-9,
                        "{}: {:?}",
                        id.pt(),
                        drawn[0].mount
                    );
                }
                assert_eq!(
                    drawn[0].rack.location(),
                    tore_sim::combat::hardpoints::Location::Centerline
                );
            }
            None => assert!(drawn.is_empty(), "{}: {record} is a placeholder", id.pt()),
        }
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
        // No retail shape: the pose carries only the record's name, which
        // picks an authored model (John 2026-11-07).
        for (pose, pod) in pods.iter().zip(&config.fixed_pods) {
            assert_eq!(pose.equipment_pod(), Some(pod.record.as_str()));
        }
    }
    assert_eq!(seen, ["AAS38.SEE"], "only the Rafale's pod is on a station");
    println!("fixed pods drawn: {seen:?}");
}
