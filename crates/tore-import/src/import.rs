//! The import: read a media source, select the resources the game and the
//! server need and write them as a pack with its report.
//! Behaviour: `docs/spec/first-run-import.md` and `docs/spec/import-cache.md`.

use crate::{
    ImportResult, Level, Resources,
    media_source::{self, MediaSource},
    note,
    pack::{cleanup_previous_imports, load_pack, write_pack},
    selection::{DEBRIEF_ART, DEBRIEF_DATA, MENU_ART, MENU_DATA},
};
use std::{fs, path::Path};

/// How far an import has got, for the first-run screen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Progress {
    /// Work before any archive is read, named in plain words for the player.
    Preparing(&'static str),
    /// Reading one archive. `total` is the number of resources selected from
    /// it, when it is known.
    Reading {
        archive: String,
        done: usize,
        total: Option<usize>,
    },
}

/// A finished import: the resources as read back from the written pack, what
/// the caller's decoder made of them, and the plain-words summary the locate
/// screen shows. The same facts are in `import-report.txt` in full.
pub struct Imported<T> {
    pub resources: Resources,
    pub value: T,
    /// Shown by the pre-game shell as the import summary.
    pub summary: Vec<String>,
}

/// Imports the media at `source` into `destination`: writes the pack and
/// `import-report.txt`, verifies the pack by loading it back, prunes older
/// generations and remembers the source. Progress is reported at least once per
/// archive and every 64 resources; the callback runs on the importing thread.
///
/// `decode` is the caller's check of what it needs from the resources (see
/// [`crate::pack::load_with`]). It runs on the collected resources before
/// anything is written, so a set the caller cannot use never reaches the
/// cache, and again on the pack read back from disk.
pub fn import_with_progress<T>(
    source: &MediaSource,
    destination: &Path,
    progress: &mut dyn FnMut(Progress),
    decode: &dyn Fn(&Resources) -> ImportResult<T>,
) -> ImportResult<Imported<T>> {
    let mut resources = Resources::new();
    let mut summary = Vec::new();
    let mut report = String::from(
        "T.O.R.E-Fighters menu import v1\nOnly selected resources decompressed. No executable resources executed.\n",
    );
    report.push_str(&format!(
        "Source: {} {}\n",
        source.kind.label(),
        source.path.display()
    ));
    // The build is identified before any archive is opened, so an unreviewed
    // executable never leaves a half-known data set in the cache.
    progress(Progress::Preparing("Identifying the game build"));
    let executable = source.executable()?;
    let layout = tore_formats::executable::identify(&executable)?;
    let tables = tore_formats::ui::creator::Options::parse(&executable)?;
    let clouds = tore_formats::weather::clouds::Layout::parse(&executable)?;
    resources.insert("TORE_CLOUDS_V1".into(), clouds.encode());
    resources.insert(
        "TORE_FLARE_V1".into(),
        tore_formats::weather::flare::Layout::parse(&executable)?.encode(),
    );
    resources.insert("TORE_CREATOR_V1".into(), tables.encode());
    report.push_str(&format!(
        "FA.EXE: {} SHA-256 {}; inert creator lists and cloud layout\n",
        layout.name,
        tore_formats::executable::sha256(&executable)
    ));
    summary.push(format!("Build read: FA.EXE {}", layout.name));
    progress(Progress::Preparing("Finding aircraft and theaters"));
    let aircraft_libs = [source.archive("FA_1.LIB")?, source.archive("FA_2.LIB")?];
    let aircraft_names = tore_formats::aircraft::dependencies(
        &aircraft_libs.iter().collect::<Vec<_>>(),
        &tore_formats::aircraft::AircraftId::ALL,
        true,
    )?;
    let scene_layouts: Vec<String> = aircraft_libs
        .iter()
        .flat_map(|archive| archive.entries.keys())
        .filter(|name| name.ends_with(".MM") && tore_formats::theater::base_theater(name).is_some())
        .cloned()
        .collect();
    resources.insert("TORE_TERRAIN_V2".into(), vec![2]);
    let scene_names = tore_formats::mission::scene_dependencies(
        &aircraft_libs.iter().collect::<Vec<_>>(),
        &scene_layouts,
    )?;
    for (filename, names, debrief) in [
        ("FA_1.LIB", MENU_ART, DEBRIEF_ART),
        ("FA_2.LIB", MENU_DATA, DEBRIEF_DATA),
    ] {
        let lib = source.archive(filename)?;
        report.push_str(&format!(
            "{filename}: {} unique entries\n",
            lib.entries.len()
        ));
        summary.push(format!("{filename}: {} entries read", lib.entries.len()));
        let selected: Vec<_> = lib
            .entries
            .keys()
            .filter(|n| {
                names.contains(&n.as_str())
                    || debrief.contains(&n.as_str())
                    || n.as_str() == "MCICONS.PIC"
                    || aircraft_names.contains(*n)
                    || scene_names.contains(*n)
                    || tore_formats::ui::creator::resource(n)
                    || tore_formats::music::resource(n)
                    || tore_formats::radio::resource(n)
                    || tore_formats::ejection::RESOURCES.contains(&n.as_str())
                    || tore_formats::theater::theater_resource(n, "ALL")
            })
            .cloned()
            .collect();
        progress(Progress::Reading {
            archive: filename.to_string(),
            done: 0,
            total: Some(selected.len()),
        });
        for (index, name) in selected.iter().enumerate() {
            if index > 0 && index.is_multiple_of(64) {
                progress(Progress::Reading {
                    archive: filename.to_string(),
                    done: index,
                    total: Some(selected.len()),
                });
            }
            let bytes = lib.read(name)?;
            let entry = &lib.entries[name];
            report.push_str(&format!(
                "{filename}/{name}: offset={}, stored={}, decoded={}\n",
                entry.offset,
                entry.size,
                bytes.len()
            ));
            if resources.get(name).is_some_and(|old| *old != bytes) {
                return Err(format!("conflicting resource {filename}/{name}").into());
            }
            resources.insert(name.to_string(), bytes);
        }
        progress(Progress::Reading {
            archive: filename.to_string(),
            done: selected.len(),
            total: Some(selected.len()),
        });
    }
    match tore_formats::radio::phrases(&executable) {
        Ok(phrases) => {
            report.push_str(&format!(
                "Radio: {} verified phrase mappings\n",
                phrases.len()
            ));
            summary.push(format!("Radio: {} phrase mappings read", phrases.len()));
            resources.extend(phrases);
        }
        Err(error) => {
            report.push_str(&format!("Optional radio metadata unavailable: {error}\n"));
            summary.push(format!("Radio phrases unavailable: {error}"));
        }
    }
    for filename in ["FA_4B.LIB", "FA_4D.LIB"] {
        let lib = match source.optional_archive(filename) {
            Ok(Some(lib)) => lib,
            Ok(None) => {
                report.push_str(&format!(
                    "Optional recorded music unavailable: {filename} is not in this source\n"
                ));
                summary.push(format!("Recorded music {filename} missing"));
                continue;
            }
            Err(error) => {
                note(
                    Level::Warn,
                    &format!("Optional recorded music unavailable: {error}"),
                );
                report.push_str(&format!("Optional recorded music unavailable: {error}\n"));
                summary.push(format!("Recorded music {filename} unreadable: {error}"));
                continue;
            }
        };
        let scores: Vec<String> = lib
            .entries
            .keys()
            .filter(|n| tore_formats::music::resource(n))
            .cloned()
            .collect();
        progress(Progress::Reading {
            archive: filename.to_string(),
            done: 0,
            total: Some(scores.len()),
        });
        summary.push(format!("{filename}: {} music resources read", scores.len()));
        for (index, name) in scores.iter().enumerate() {
            if index > 0 && index.is_multiple_of(64) {
                progress(Progress::Reading {
                    archive: filename.to_string(),
                    done: index,
                    total: Some(scores.len()),
                });
            }
            let bytes = lib.read(name)?;
            if resources.get(name).is_some_and(|old| *old != bytes) {
                return Err(format!("conflicting music resource {filename}/{name}").into());
            }
            let entry = &lib.entries[name];
            report.push_str(&format!(
                "{filename}/{name}: offset={}, stored={}, decoded={}\n",
                entry.offset,
                entry.size,
                bytes.len()
            ));
            resources.insert(name.clone(), bytes);
        }
        progress(Progress::Reading {
            archive: filename.to_string(),
            done: scores.len(),
            total: Some(scores.len()),
        });
    }
    let mut missing_scores = 0;
    for name in tore_formats::music::SCORES {
        if let Some(bytes) = resources.get(*name) {
            let score = tore_formats::music::Score::parse(bytes)?;
            for track in &score.tracks {
                let file = score.filename(*track);
                if !resources.contains_key(&file) {
                    report.push_str(&format!("{name}: unavailable {file}; no substitution\n"));
                }
            }
        } else {
            missing_scores += 1;
            report.push_str(&format!("Unavailable score {name}\n"));
        }
    }
    if missing_scores > 0 {
        summary.push(format!(
            "Recorded music: {missing_scores} of {} scores unavailable",
            tore_formats::music::SCORES.len()
        ));
    }
    resources.insert("TORE_MUSIC_V1".into(), b"PCM1".to_vec());
    resources.insert("TORE_COMBAT_V1".into(), b"RAW1".to_vec());
    resources.insert("TORE_AIRPORTS_V1".into(), b"SCENE1".to_vec());
    resources.insert("TORE_SPEECH_V1".into(), b"ALL1".to_vec());
    drop(decode(&resources)?);
    fs::create_dir_all(destination)?;
    // Generation files keep the previous import usable until the new pack is complete.
    let generation = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let path = destination.join(format!("menu-{generation}.pack"));
    write_pack(&path, &resources)?;
    fs::write(destination.join("import-report.txt"), report)?;
    // Verify the on-disk pack before removing any previously usable import.
    let loaded = load_pack(&path, decode)?;
    cleanup_previous_imports(destination, &path);
    // The remembered source only saves the player a second choice; failing to
    // write it does not spoil a finished import.
    if let Err(error) = media_source::remember(destination, source) {
        note(
            Level::Warn,
            &format!("Could not remember the media source: {error}"),
        );
        summary.push(format!("Media source not remembered: {error}"));
    }
    note(
        Level::Info,
        &format!(
            "Imported menu and all theater resources to {}",
            path.display()
        ),
    );
    Ok(Imported {
        resources: loaded.resources,
        value: loaded.value,
        summary,
    })
}
