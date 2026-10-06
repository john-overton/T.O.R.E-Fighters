//! Content items (`content.rs`) over the synthetic import, and, ignored, over
//! real imports.
//!
//! The synthetic import holds one aircraft (the F/A-18D), one theater (UKR),
//! two weapons and no radio phrases, so the shared item here is the list of
//! the phrases a mission asks for, each missing. A second synthetic aircraft
//! is not possible (the aircraft reader accepts only the reviewed records), so
//! the tests that need two vary one item's files instead.
use crate::{
    content::{Content, Item, Kind, SHARED_KEY},
    mission::{Condition, LoadoutSpec, MissionSpec, Skill, Start},
    resources::{ResourceReads, ResourceSource},
    test_support::resources::{
        AIRPORT_RUNWAY, THEATER, airport_resources, loadable_missile, resources,
    },
    world::{Seating, World},
};
use std::collections::BTreeMap;
use tore_formats::aircraft::AircraftId;

type Map = BTreeMap<String, Vec<u8>>;

/// `name` of `map` with `from` replaced by `to` in its text.
fn edited(map: &mut Map, name: &str, from: &str, to: &str) {
    let text = String::from_utf8(map[name].clone()).expect("a text resource");
    assert!(text.contains(from), "{name} holds {from}");
    map.insert(name.to_owned(), text.replacen(from, to, 1).into_bytes());
}

fn digest_of(content: &Content, kind: Kind, key: &str) -> Option<u64> {
    content.get(kind, key).map(|item| item.digest)
}

/// The mission shapes the coverage rule is tested over: the player's flight
/// and an enemy flight in `condition`, airborne.
fn mission(condition: Condition, count: usize) -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.condition = condition;
    spec.wings[0].count = count;
    spec.wings[3].count = count;
    spec.wings[3].skill = Skill::Average;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// The coverage rule for one mission: every resource its build reads, with
/// its hash, lies in the shared item or an item the mission owns (its
/// aircraft, its theater, the weapons of its loadouts), and every resource of
/// the shared item is read.
fn coverage(map: &Map, content: &Content, spec: &MissionSpec) -> Result<(), String> {
    let reads = ResourceReads::new(map);
    World::new(spec, &reads, Seating::Open).map_err(|error| error.to_string())?;
    let manifest = reads.manifest();
    let mut owners: Vec<&Item> = Vec::new();
    let mut want = |kind: Kind, key: &str| match content.get(kind, key) {
        Some(item) => {
            owners.push(item);
            Ok(())
        }
        None => Err(format!("the content has no {} {key}", kind.name())),
    };
    want(Kind::Shared, SHARED_KEY)?;
    want(Kind::Theater, spec.theater.trim_end_matches(".MM"))?;
    for wing in spec.wings.iter().filter(|wing| wing.count > 0) {
        want(Kind::Aircraft, wing.aircraft.selection_key())?;
    }
    for load in spec.plane_loadouts.values() {
        for station in &load.stations {
            want(Kind::Weapon, &station.weapon)?;
        }
    }
    let owned: BTreeMap<&str, Option<u64>> = owners
        .iter()
        .flat_map(|item| item.manifest().entries.iter())
        .map(|entry| (entry.name.as_str(), entry.hash))
        .collect();
    let uncovered: Vec<&str> = manifest
        .entries
        .iter()
        .filter(|entry| owned.get(entry.name.as_str()) != Some(&entry.hash))
        .map(|entry| entry.name.as_str())
        .collect();
    if !uncovered.is_empty() {
        return Err(format!(
            "read by the build, in no item the mission owns: {uncovered:?}"
        ));
    }
    let shared = content.shared().expect("a shared item");
    let unread: Vec<&str> = shared
        .names()
        .filter(|name| !manifest.entries.iter().any(|entry| entry.name == *name))
        .collect();
    if !unread.is_empty() {
        return Err(format!(
            "in the shared item, not read by this mission: {unread:?}"
        ));
    }
    Ok(())
}

#[test]
fn the_synthetic_import_has_its_aircraft_theater_weapons_and_shared_item() {
    let content = Content::of(&resources());
    let found: Vec<(Kind, &str)> = content
        .items()
        .iter()
        .map(|item| (item.kind, item.key.as_str()))
        .collect();
    assert_eq!(
        found,
        [
            (Kind::Aircraft, "F18.PT"),
            (Kind::Theater, "UKR"),
            (Kind::Weapon, "AIM9M.JT"),
            (Kind::Weapon, "M61.JT"),
            (Kind::Shared, SHARED_KEY),
        ]
    );
    // The other thirteen aircraft and fifteen theaters are not in the import.
    assert!(content.get(Kind::Aircraft, "SU27.PT").is_none());
    assert!(content.get(Kind::Aircraft, "faxx").is_none());
    assert!(content.get(Kind::Theater, "BAL").is_none());
    assert_eq!(content.of_kind(Kind::Weapon).count(), 2);
    assert_eq!(
        content.shared().map(|item| item.key.as_str()),
        Some("shared")
    );
}

#[test]
fn an_items_names_are_what_its_loader_reads_and_its_digest_covers_their_bytes() {
    let map = resources();
    let content = Content::of(&map);
    let aircraft = content.get(Kind::Aircraft, "F18.PT").unwrap();
    // The profile, its sensors, its jammer and its default stores' weapons.
    for name in [
        "F18.PT", "F18R.SEE", "F18V.SEE", "F18.ECM", "M61.JT", "AIM9M.JT",
    ] {
        assert!(aircraft.reads(name), "{name} is read by the aircraft");
    }
    assert!(!aircraft.reads("UKR.T2"));
    let theater = content.get(Kind::Theater, "UKR").unwrap();
    // The layout, its own grid and the layers of the six conditions (the
    // synthetic import shares three of the six layers).
    for name in ["UKR.MM", "UKR.T2", "CLOUD1.LAY", "DAY2.LAY", "FOG1.LAY"] {
        assert!(theater.reads(name), "{name} is read by the theater");
    }
    assert!(!theater.reads("F18.PT"));
    for item in content.items() {
        let names: Vec<&str> = item.names().collect();
        assert!(names.windows(2).all(|pair| pair[0] < pair[1]), "sorted");
        assert_eq!(item.digest, item.manifest().digest());
        for entry in &item.manifest().entries {
            assert_eq!(
                entry.hash,
                map.get(&entry.name)
                    .map(|bytes| tore_codec::hash::fnv1a64(bytes)),
                "{} in {}",
                entry.name,
                item.key
            );
        }
    }
    // A weapon is its record alone.
    let weapon = content.get(Kind::Weapon, "AIM9M.JT").unwrap();
    assert_eq!(weapon.names().collect::<Vec<_>>(), ["AIM9M.JT"]);
    // The same import gives the same content, item for item.
    assert_eq!(content, Content::of(&map));
}

#[test]
fn items_are_found_by_a_resource_they_read() {
    let content = Content::of(&resources());
    let keys = |name: &str| -> Vec<(Kind, String)> {
        content
            .holding(name)
            .iter()
            .map(|item| (item.kind, item.key.clone()))
            .collect()
    };
    assert_eq!(keys("F18.PT"), [(Kind::Aircraft, "F18.PT".to_owned())]);
    // A weapon is read by its own item and by the aircraft that carries it.
    assert_eq!(
        keys("AIM9M.JT"),
        [
            (Kind::Aircraft, "F18.PT".to_owned()),
            (Kind::Weapon, "AIM9M.JT".to_owned())
        ]
    );
    assert_eq!(keys("UKR.T2"), [(Kind::Theater, "UKR".to_owned())]);
    assert_eq!(
        keys("TORE_RADIO_^YOUR"),
        [(Kind::Shared, SHARED_KEY.to_owned())]
    );
    assert!(keys("NOTHING.PT").is_empty());
}

#[test]
fn the_shared_item_is_what_a_mission_reads_beyond_its_aircraft_and_theater() {
    let content = Content::of(&resources());
    let shared = content.shared().unwrap();
    assert!(shared.names().count() > 100, "the radio phrases");
    assert!(shared.names().all(|name| name.starts_with("TORE_RADIO_")));
    // Nothing an aircraft or a theater item reads is also shared.
    for item in content.items().iter().filter(|i| i.kind != Kind::Shared) {
        for name in item.names() {
            assert!(!shared.reads(name), "{name} is in {} and shared", item.key);
        }
    }
    // The import has none of the phrases, and each is an entry without a hash,
    // so an import that has one differs.
    assert!(shared.manifest().entries.iter().all(|e| e.hash.is_none()));
    let mut with_phrase = resources();
    with_phrase.insert("TORE_RADIO_^YOUR".into(), vec![1, 2, 3]);
    let other = Content::of(&with_phrase);
    assert_ne!(
        digest_of(&content, Kind::Shared, SHARED_KEY),
        digest_of(&other, Kind::Shared, SHARED_KEY)
    );
    for (kind, key) in [
        (Kind::Aircraft, "F18.PT"),
        (Kind::Theater, "UKR"),
        (Kind::Weapon, "M61.JT"),
    ] {
        assert_eq!(digest_of(&content, kind, key), digest_of(&other, kind, key));
    }
}

#[test]
fn a_file_missing_from_the_import_leaves_out_the_items_that_need_it() {
    let whole = Content::of(&resources());
    // A sensor the aircraft reads: the aircraft is not loadable. With no
    // aircraft there is no reference mission, so no shared item.
    let mut map = resources();
    map.remove("F18R.SEE");
    let content = Content::of(&map);
    assert!(content.get(Kind::Aircraft, "F18.PT").is_none());
    assert!(content.shared().is_none());
    assert_eq!(
        digest_of(&content, Kind::Theater, "UKR"),
        digest_of(&whole, Kind::Theater, "UKR")
    );
    assert_eq!(content.of_kind(Kind::Weapon).count(), 2);

    // A weapon the aircraft carries by default: the weapon and the aircraft
    // go, the other weapon and the theater stay.
    let mut map = resources();
    map.remove("AIM9M.JT");
    let content = Content::of(&map);
    assert!(content.get(Kind::Weapon, "AIM9M.JT").is_none());
    assert!(content.get(Kind::Aircraft, "F18.PT").is_none());
    assert!(content.get(Kind::Weapon, "M61.JT").is_some());
    assert!(content.get(Kind::Theater, "UKR").is_some());

    // The theater's grid, and one of the six condition layers: the theater
    // goes, and the aircraft stays.
    for name in ["UKR.T2", "FOG1.LAY"] {
        let mut map = resources();
        map.remove(name);
        let content = Content::of(&map);
        assert!(content.get(Kind::Theater, "UKR").is_none(), "{name}");
        assert!(content.get(Kind::Aircraft, "F18.PT").is_some(), "{name}");
        assert!(content.shared().is_none(), "{name}: no theater to fly in");
    }

    // A file nothing reads changes nothing.
    let mut map = resources();
    map.insert("MENU.PIC".into(), vec![9; 16]);
    assert_eq!(Content::of(&map), whole);
}

#[test]
fn a_different_file_changes_the_digest_of_every_item_that_reads_it_and_no_other() {
    let whole = Content::of(&resources());
    let digests = |content: &Content| -> Vec<Option<u64>> {
        [
            (Kind::Aircraft, "F18.PT"),
            (Kind::Theater, "UKR"),
            (Kind::Weapon, "AIM9M.JT"),
            (Kind::Weapon, "M61.JT"),
            (Kind::Shared, SHARED_KEY),
        ]
        .iter()
        .map(|(kind, key)| digest_of(content, *kind, key))
        .collect()
    };
    let before = digests(&whole);
    let changed = |after: &[Option<u64>]| -> Vec<usize> {
        (0..before.len())
            .filter(|i| before[*i] != after[*i])
            .collect()
    };

    // The positions are [aircraft, theater, AIM9M, M61, shared].
    let mut map = resources();
    edited(&mut map, "AIM9M.JT", "Synthetic weapon", "Synthetic weapoN");
    // The weapon, and the aircraft that carries it.
    assert_eq!(changed(&digests(&Content::of(&map))), [0, 2]);

    let mut map = resources();
    edited(&mut map, "F18R.SEE", "Synthetic sensor", "Synthetic sensoR");
    assert_eq!(changed(&digests(&Content::of(&map))), [0]);

    let mut map = resources();
    map.get_mut("UKR.T2").unwrap()[4] = b'X';
    assert_eq!(changed(&digests(&Content::of(&map))), [1]);

    let mut map = resources();
    map.insert("TORE_RADIO_^BINGO".into(), vec![7]);
    assert_eq!(changed(&digests(&Content::of(&map))), [4]);

    let mut map = resources();
    map.get_mut("CLOUD1.LAY").unwrap().push(0);
    assert_eq!(changed(&digests(&Content::of(&map))), [1]);
}

#[test]
fn every_synthetic_mission_is_covered_by_the_shared_item_and_its_own_items() {
    let map = resources();
    let content = Content::of(&map);
    // Every weather condition, a flight of one and a flight of five.
    for condition in Condition::ALL {
        for count in [1, 5] {
            coverage(&map, &content, &mission(condition, count))
                .unwrap_or_else(|error| panic!("{condition:?} x{count}: {error}"));
        }
    }
    // The creator's other settings that change what a build reads.
    let mut spec = mission(Condition::Night, 2);
    spec.guns_only = true;
    spec.separation_nm = 50;
    spec.start = Start::Airborne {
        altitude_ft: 40_000,
    };
    coverage(&map, &content, &spec).unwrap();
    // No enemy at all.
    let mut spec = mission(Condition::Clear, 1);
    spec.wings[3].count = 0;
    coverage(&map, &content, &spec).unwrap();
}

#[test]
fn a_ground_start_and_a_players_other_weapon_are_covered_too() {
    // A ground start on the airport of the synthetic theater.
    let map = airport_resources();
    let content = Content::of(&map);
    let mut spec = mission(Condition::Clear, 2);
    spec.start = Start::Ground {
        runway: AIRPORT_RUNWAY,
        altitude_ft: 10_000,
    };
    coverage(&map, &content, &spec).unwrap();

    // A loadout carrying a weapon the standard load never reads: the weapon
    // item covers it.
    let mut map = resources();
    map.insert("AIM9X.JT".into(), loadable_missile("AIM9X.JT"));
    let content = Content::of(&map);
    assert!(content.get(Kind::Weapon, "AIM9X.JT").is_some());
    let player = crate::aircraft_type::AircraftType::load(&map, AircraftId::F18).unwrap();
    let standard = tore_sim::combat::loadout::Loadout::new(&player.profile, |name| {
        map.get(name)
            .cloned()
            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
    })
    .unwrap();
    let missile = tore_formats::weapons::Weapon::parse("AIM9X.JT", &map["AIM9X.JT"]).unwrap();
    let capacity = standard.capacity(1, &missile) as u16;
    let mut load = LoadoutSpec::of(&standard);
    load.stations[1].weapon = "AIM9X.JT".into();
    load.stations[1].count = capacity;
    load.stations[1].quantity = capacity;
    let mut spec = mission(Condition::Clear, 2);
    spec.plane_loadouts.insert(1, load);
    coverage(&map, &content, &spec).unwrap();
}

#[test]
fn the_coverage_rule_fails_for_a_read_that_belongs_to_no_item() {
    // A content whose shared item lacks a resource the mission reads (a probe
    // that missed a read): the rule names it.
    let map = resources();
    let whole = Content::of(&map);
    let mut items = whole.items().to_vec();
    let shared = items.iter().position(|i| i.kind == Kind::Shared).unwrap();
    let mut manifest = items[shared].manifest().clone();
    let dropped = manifest.entries.remove(0).name;
    items[shared] = Item::new(Kind::Shared, SHARED_KEY, manifest);
    let stale = Content::from_items(items);
    let error = coverage(&map, &stale, &mission(Condition::Clear, 1)).unwrap_err();
    assert!(error.contains(&dropped), "{error}");

    // And a shared item that holds a resource this mission never reads.
    let mut items = whole.items().to_vec();
    let mut manifest = items[shared].manifest().clone();
    manifest.entries.push(crate::resources::ManifestEntry {
        name: "ZZ_ONLY_SOME_MISSIONS_READ_THIS".into(),
        hash: None,
    });
    items[shared] = Item::new(Kind::Shared, SHARED_KEY, manifest);
    let error = coverage(
        &map,
        &Content::from_items(items),
        &mission(Condition::Clear, 1),
    )
    .unwrap_err();
    assert!(error.contains("ZZ_ONLY_SOME_MISSIONS_READ_THIS"), "{error}");
}

#[test]
fn a_manifest_holds_only_its_own_theaters_grid() {
    let mut map = resources();
    // Other theaters' grids in the import.
    let grid = map["UKR.T2"].clone();
    for other in ["BAL", "EGY", "KURILE"] {
        map.insert(format!("{other}.T2"), grid.clone());
    }
    let reads = ResourceReads::new(&map);
    World::new(&mission(Condition::Clear, 1), &reads, Seating::Open).unwrap();
    let names = reads.names();
    assert!(names.contains(&"UKR.T2".to_owned()));
    for other in ["BAL", "EGY", "KURILE"] {
        assert!(
            !names.contains(&format!("{other}.T2")),
            "{other}.T2 was read: {names:?}"
        );
    }
    // And the item of the theater holds its own grid only.
    let content = Content::of(&map);
    let theater = content.get(Kind::Theater, "UKR").unwrap();
    assert!(theater.reads("UKR.T2"));
    assert!(!theater.reads("BAL.T2"));
}

#[test]
fn a_bad_grid_of_another_theater_fails_no_other_theater() {
    let whole = Content::of(&resources());
    let mut map = resources();
    map.insert("BAL.T2".into(), b"not a grid".to_vec());
    World::new(&mission(Condition::Clear, 1), &map, Seating::Open)
        .expect("a bad BAL grid does not fail a UKR mission");
    assert_eq!(Content::of(&map), whole, "the bad grid is in no UKR item");
    // The catalog of every grid still fails, as it always did; only the build
    // stopped asking for it.
    assert!(map.theater_catalog().is_err());
}

#[test]
fn a_theaters_label_is_the_catalogs_for_a_base_theater_and_a_variant() {
    let mut map = resources();
    let layout = map["UKR.MM"].clone();
    map.insert("~UKR1.MM".into(), layout);
    let catalog = map.theater_catalog().unwrap();
    assert_eq!(
        catalog,
        [
            ("UKR".to_owned(), "Synthetic".to_owned()),
            ("~UKR1".to_owned(), "Synthetic (UKR1)".to_owned())
        ]
    );
    for (code, label) in &catalog {
        assert_eq!(map.theater_label(code).unwrap().as_ref(), Some(label));
        let reads = ResourceReads::new(&map);
        assert_eq!(reads.theater_label(code).unwrap().as_ref(), Some(label));
        assert_eq!(reads.names(), ["UKR.T2"], "{code} reads one grid");
        // The terrain carries the label.
        let terrain =
            crate::terrain::Terrain::for_mission(&map, code, Some(0), &Default::default()).unwrap();
        assert_eq!(&terrain.theater.name, label);
        assert_eq!(terrain.catalog, [(code.clone(), label.clone())]);
    }
    // A theater the import has no grid for, a variant with no layout and a
    // code that is not a theater have no label.
    assert_eq!(map.theater_label("BAL").unwrap(), None);
    assert_eq!(map.theater_label("~UKR2").unwrap(), None);
    assert_eq!(map.theater_label("NOWHERE").unwrap(), None);
}

// ---- Real data (ignored: they need an import; run them in the full suite) ----

/// The newest `menu-*.pack` in `directory`, read without touching the folder
/// (the importer's own loader would also delete older packs). The format is
/// in `tore_import::pack`.
fn read_pack(directory: &std::path::Path) -> Map {
    use std::io::Read;
    let mut packs: Vec<_> = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("{}: {error}", directory.display()))
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension().is_some_and(|ext| ext == "pack")
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().starts_with("menu-"))
        })
        .collect();
    packs.sort();
    let path = packs.pop().expect("an import pack in the data folder");
    let mut bytes = Vec::new();
    std::fs::File::open(&path)
        .and_then(|mut file| file.read_to_end(&mut bytes))
        .expect("the pack reads");
    assert_eq!(&bytes[..12], b"TOREMENU\x01\0\0\0");
    let word = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let mut map = Map::new();
    let (count, mut at) = (word(12), 16);
    for _ in 0..count {
        let len = u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
        let name = String::from_utf8(bytes[at + 2..at + 2 + len].to_vec()).unwrap();
        let size = word(at + 2 + len);
        let start = at + 6 + len;
        map.insert(name, bytes[start..start + size].to_vec());
        at = start + size;
    }
    map
}

fn real(variable: &str) -> Map {
    let directory = std::env::var_os(variable)
        .unwrap_or_else(|| panic!("{variable} names no data folder with an import"));
    read_pack(std::path::Path::new(&directory))
}

/// Every selectable aircraft once, every theater once and every weather
/// condition once, each mission an open one: its player's wing and an enemy
/// wing of the next aircraft.
fn real_missions(content: &Content) -> Vec<MissionSpec> {
    let aircraft: Vec<AircraftId> = content
        .of_kind(Kind::Aircraft)
        .map(|item| AircraftId::parse(&item.key).unwrap())
        .collect();
    let theaters: Vec<&str> = content
        .of_kind(Kind::Theater)
        .map(|item| item.key.as_str())
        .collect();
    let count = aircraft.len().max(theaters.len());
    (0..count)
        .map(|i| {
            let mut spec =
                MissionSpec::new(theaters[i % theaters.len()], aircraft[i % aircraft.len()]);
            spec.wings[3].aircraft = aircraft[(i + 1) % aircraft.len()];
            spec.wings[3].count = 1;
            spec.wings[3].skill = Skill::Average;
            spec.condition = Condition::ALL[i % Condition::ALL.len()];
            spec.start = Start::Airborne {
                altitude_ft: 40_000,
            };
            spec
        })
        .collect()
}

/// Run with `TORE_DATA_DIR` naming a data folder with an import, in the full
/// suite.
#[test]
#[ignore = "needs real data (TORE_DATA_DIR); run in the full suite"]
fn real_data_every_mission_is_covered_by_the_shared_item_and_its_own_items() {
    let map = real("TORE_DATA_DIR");
    let content = Content::of(&map);
    assert_eq!(
        content.of_kind(Kind::Theater).count(),
        16,
        "every base theater"
    );
    assert!(content.of_kind(Kind::Aircraft).count() >= 14, "the roster");
    assert!(content.of_kind(Kind::Weapon).count() > 50, "the weapons");
    assert!(content.shared().is_some(), "the shared item");
    for item in content.items() {
        println!("{} {} {:016x}", item.kind.name(), item.key, item.digest);
    }
    let missions = real_missions(&content);
    assert!(missions.len() >= 16);
    for spec in &missions {
        coverage(&map, &content, spec).unwrap_or_else(|error| {
            panic!(
                "{} {:?} {}: {error}",
                spec.theater,
                spec.condition,
                spec.player().label()
            )
        });
    }
    // The theaters are independent: no theater item reads another's grid.
    for item in content.of_kind(Kind::Theater) {
        let grids: Vec<&str> = item.names().filter(|n| n.ends_with(".T2")).collect();
        assert_eq!(grids, [format!("{}.T2", item.key)], "{}", item.key);
    }
}

/// The cost of the content, which the design expects under one second in a
/// release build (`cargo test --release`).
#[test]
#[ignore = "needs real data (TORE_DATA_DIR); run in the full suite, in a release build"]
fn real_data_the_content_costs_under_a_second_in_a_release_build() {
    let map = real("TORE_DATA_DIR");
    let mut times = Vec::new();
    for _ in 0..3 {
        let started = std::time::Instant::now();
        let content = Content::of(&map);
        times.push(started.elapsed().as_secs_f64());
        assert!(!content.items().is_empty());
    }
    println!("Content::of: {times:.3?} s");
    if !cfg!(debug_assertions) {
        let best = times.iter().copied().fold(f64::INFINITY, f64::min);
        assert!(best < 1.0, "the content took {best:.3} s");
    }
}

/// A 1.0 import and a 1.02F import play together (slice D3a): every item has
/// the same digest in both. `TORE_DATA_DIR` holds one import and
/// `TORE_DATA_DIR_10` the other.
#[test]
#[ignore = "needs two real imports (TORE_DATA_DIR and TORE_DATA_DIR_10); run in the full suite"]
fn real_data_a_1_0_import_and_a_1_02f_import_have_the_same_content() {
    let (a, b) = (
        Content::of(&real("TORE_DATA_DIR")),
        Content::of(&real("TORE_DATA_DIR_10")),
    );
    let keys = |content: &Content| -> Vec<(Kind, String)> {
        content
            .items()
            .iter()
            .map(|item| (item.kind, item.key.clone()))
            .collect()
    };
    assert_eq!(keys(&a), keys(&b), "the same items");
    for (left, right) in a.items().iter().zip(b.items()) {
        assert_eq!(
            left.digest,
            right.digest,
            "{} {} differs: {:?}",
            left.kind.name(),
            left.key,
            left.manifest().differences(right.manifest())
        );
    }
}
