//! Everything the server does before it opens a port: the configuration, the
//! import, the mission file and a trial build of the mission. A failure is a
//! plain message that says what to do, and nothing is started.

use crate::{
    config::{Config, OpenPlanes},
    options::Options,
};
use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};
use tore_import::Resources;
use tore_world::{
    mission::MissionSpec,
    resources::{Manifest, ResourceReads},
    world::{Seating, World},
};

/// What start-up loaded and checked.
pub struct Prepared {
    pub config: Config,
    pub data_dir: PathBuf,
    pub spec: MissionSpec,
    pub resources: Arc<Resources>,
    /// What the trial build of the mission read, with hashes.
    pub manifest: Manifest,
    /// The trial build, for `--check` and for counting the aircraft. The host
    /// builds its own.
    pub world: World,
}

/// The default configuration file in a data folder.
pub fn default_config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("server.conf")
}

/// Refuses the game's retail stall-speed switch, on the command line or in the
/// environment: every machine in a session must fly one configuration, and
/// the players' games do not have the switch on.
pub fn refuse_retail_stall_speeds(
    options: &Options,
    environment: Option<OsString>,
) -> Result<(), String> {
    if options.retail_stall_speeds || environment.is_some() {
        let how = if options.retail_stall_speeds {
            "--retail-stall-speeds"
        } else {
            "TORE_RETAIL_STALL_SPEEDS"
        };
        return Err(format!(
            "{how} is on. It turns the weight-scaled stall speed off, and every machine in a session must fly one configuration, so the server will not start with it. Remove it and start again."
        ));
    }
    Ok(())
}

/// Reads the configuration file the options name, applying `--port` and
/// `--mission`. The default file is `server.conf` in the data folder; without
/// one the defaults apply.
pub fn load_config(options: &Options, data_dir: &Path) -> Result<Config, String> {
    let mut config = match &options.config {
        Some(path) => read_config(path)?,
        None => {
            let path = default_config_path(data_dir);
            if path.exists() {
                read_config(&path)?
            } else {
                Config::defaults(data_dir)
            }
        }
    };
    if let Some(port) = options.port {
        config.port = port;
    }
    if let Some(mission) = &options.mission {
        config.mission = mission.clone();
    }
    Ok(config)
}

fn read_config(path: &Path) -> Result<Config, String> {
    let text = tore_import::files::read(path).map_err(|error| {
        format!(
            "Cannot read the configuration file {}: {error}",
            path.display()
        )
    })?;
    let base = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    Config::parse(&text, base).map_err(|error| format!("{}: {error}", path.display()))
}

/// Loads the newest import in the data folder, pruning older ones as the game
/// does. A server needs only the simulation's data, so the check is the
/// import's own markers.
pub fn load_resources(data_dir: &Path) -> Result<Resources, String> {
    let has_pack = fs::read_dir(data_dir).is_ok_and(|entries| {
        entries.filter_map(Result::ok).any(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with("menu-") && name.ends_with(".pack")
        })
    });
    if !has_pack {
        return Err(format!(
            "There is no imported game in {}. Import Fighters Anthology first: tore-server --import /path/to/FIGHTERS (an installed game folder or a disc folder), or point --data-dir at the folder the game imported into.",
            data_dir.display()
        ));
    }
    tore_import::load_with(data_dir, &|resources| {
        tore_import::check_markers(resources)
    })
    .map(|loaded| loaded.resources)
    .map_err(|error| {
        let text = error.to_string();
        if text.contains("re-import") {
            format!(
                "The import in {} is out of date ({text}). Import again: tore-server --import /path/to/FIGHTERS",
                data_dir.display()
            )
        } else {
            format!(
                "The import in {} cannot be used ({text}). Import again: tore-server --import /path/to/FIGHTERS",
                data_dir.display()
            )
        }
    })
}

/// Reads and parses a mission file; an error names the file and its line.
pub fn load_mission(path: &Path) -> Result<MissionSpec, String> {
    let text = tore_import::files::read(path)
        .map_err(|error| format!("Cannot read the mission file {}: {error}", path.display()))?;
    MissionSpec::from_text(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// Loads everything and builds the mission once to prove it can be built.
pub fn prepare(options: &Options, data_dir: &Path) -> Result<Prepared, String> {
    let config = load_config(options, data_dir)?;
    let resources = load_resources(data_dir)?;
    let spec = load_mission(&config.mission)?;
    build(config, data_dir, spec, resources)
}

/// The trial build and the checks that need the built mission.
pub fn build(
    config: Config,
    data_dir: &Path,
    spec: MissionSpec,
    resources: Resources,
) -> Result<Prepared, String> {
    let (world, manifest) = {
        let reads = ResourceReads::new(&resources);
        let world = World::new(&spec, &reads, Seating::Open).map_err(|error| {
            let hint = if spec.ground_runway().is_some() {
                " (`start ground` takes a runway number from --check)"
            } else {
                ""
            };
            format!(
                "The mission in {} cannot be built from this import: {error}{hint}",
                config.mission.display()
            )
        })?;
        (world, reads.manifest())
    };
    if let OpenPlanes::List(planes) = &config.open_planes {
        let count = world.roster.planes().len() as u32;
        if let Some(plane) = planes.iter().find(|plane| **plane >= count) {
            return Err(format!(
                "open-planes names plane {plane}, but the mission has planes 0 to {}. --check lists them.",
                count.saturating_sub(1)
            ));
        }
    }
    Ok(Prepared {
        config,
        data_dir: data_dir.to_owned(),
        spec,
        resources: Arc::new(resources),
        manifest,
        world,
    })
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::log::tests::scratch;
    use tore_import::pack::write_pack;
    use tore_world::test_support::resources::resources;

    /// A data folder holding a synthetic import with the markers.
    pub fn data_folder(name: &str, markers: bool) -> PathBuf {
        let dir = scratch(name);
        fs::create_dir_all(&dir).unwrap();
        let mut map = resources();
        if markers {
            map.insert("TORE_MUSIC_V1".into(), b"PCM1".to_vec());
            map.insert("TORE_COMBAT_V1".into(), b"RAW1".to_vec());
            map.insert("TORE_AIRPORTS_V1".into(), b"SCENE1".to_vec());
            map.insert("TORE_SPEECH_V1".into(), b"ALL1".to_vec());
        }
        write_pack(&dir.join("menu-1.pack"), &map).unwrap();
        dir
    }

    pub const MISSION: &str = "tore-mission 1\ntheater UKR\nstart airborne 10000\nseparation-nm 2\nwing friendly 1 F18.PT 2 average\nwing enemy 1 F18.PT 2 average\n";

    #[test]
    fn the_retail_stall_switch_is_refused_either_way() {
        let flag = Options {
            retail_stall_speeds: true,
            ..Default::default()
        };
        assert!(
            refuse_retail_stall_speeds(&flag, None)
                .unwrap_err()
                .contains("--retail-stall-speeds is on")
        );
        assert!(
            refuse_retail_stall_speeds(&Options::default(), Some("1".into()))
                .unwrap_err()
                .contains("TORE_RETAIL_STALL_SPEEDS is on")
        );
        assert!(refuse_retail_stall_speeds(&Options::default(), None).is_ok());
    }

    #[test]
    fn a_missing_import_says_how_to_make_one() {
        let dir = scratch("prep-none");
        fs::create_dir_all(&dir).unwrap();
        let error = load_resources(&dir).unwrap_err();
        assert!(error.contains("no imported game"), "{error}");
        assert!(error.contains("--import"), "{error}");
        let missing = load_resources(&dir.join("absent")).unwrap_err();
        assert!(missing.contains("no imported game"), "{missing}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_stale_import_says_to_import_again() {
        let dir = data_folder("prep-stale", false);
        let error = load_resources(&dir).unwrap_err();
        assert!(error.contains("out of date"), "{error}");
        assert!(error.contains("tore-server --import"), "{error}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_good_import_loads() {
        let dir = data_folder("prep-good", true);
        assert!(load_resources(&dir).unwrap().contains_key("UKR.T2"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_bad_mission_line_is_refused_with_its_number() {
        let dir = scratch("prep-mission");
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("mission.txt");
        fs::write(
            &path,
            "tore-mission 1\ntheater UKR\ncondition clear\ntheater MOON\n",
        )
        .unwrap();
        let error = load_mission(&path).unwrap_err();
        assert!(error.contains("mission.txt: line 4"), "{error}");
        fs::write(
            &path,
            "tore-mission 1\nwing friendly 1 F18.PT 2 average\ntheater MOON\n",
        )
        .unwrap();
        assert!(load_mission(&path).unwrap_err().contains("line 3"));
        assert!(
            load_mission(&dir.join("none.txt"))
                .unwrap_err()
                .contains("Cannot read the mission file")
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn the_options_override_the_configuration_and_the_default_file_is_optional() {
        let dir = scratch("prep-config");
        fs::create_dir_all(&dir).unwrap();
        // No server.conf: defaults, the mission beside it.
        let config = load_config(&Options::default(), &dir).unwrap();
        assert_eq!(config, Config::defaults(&dir));
        fs::write(dir.join("server.conf"), "port 27000\nmission duel.txt\n").unwrap();
        let config = load_config(&Options::default(), &dir).unwrap();
        assert_eq!(
            (config.port, config.mission.clone()),
            (27000, dir.join("duel.txt"))
        );
        let options = Options {
            port: Some(28000),
            mission: Some("other.txt".into()),
            ..Default::default()
        };
        let config = load_config(&options, &dir).unwrap();
        assert_eq!(
            (config.port, config.mission),
            (28000, PathBuf::from("other.txt"))
        );
        // A named file that is missing or wrong is an error naming it.
        let named = Options {
            config: Some(dir.join("nope.conf")),
            ..Default::default()
        };
        assert!(load_config(&named, &dir).unwrap_err().contains("nope.conf"));
        fs::write(dir.join("bad.conf"), "port 1\nprot 2\n").unwrap();
        let named = Options {
            config: Some(dir.join("bad.conf")),
            ..Default::default()
        };
        let error = load_config(&named, &dir).unwrap_err();
        assert!(
            error.contains("bad.conf: line 2: `prot` is not a setting"),
            "{error}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_good_mission_builds_and_gives_its_manifest() {
        let dir = data_folder("prep-build", true);
        fs::write(dir.join("mission.txt"), MISSION).unwrap();
        let prepared = prepare(&Options::default(), &dir).unwrap();
        assert_eq!(prepared.world.roster.planes().len(), 4);
        assert!(prepared.manifest.entries.iter().any(|e| e.name == "F18.PT"));
        assert_ne!(prepared.manifest.digest(), 0);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_mission_naming_what_the_import_lacks_is_refused() {
        let dir = data_folder("prep-lacks", true);
        // The synthetic import has the F/A-18D and no Su-27.
        fs::write(
            dir.join("mission.txt"),
            MISSION.replace("wing enemy 1 F18.PT", "wing enemy 1 SU27.PT"),
        )
        .unwrap();
        let error = prepare(&Options::default(), &dir).err().unwrap();
        assert!(
            error.contains("cannot be built from this import"),
            "{error}"
        );
        assert!(error.contains("SU27"), "{error}");
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn open_planes_must_name_planes_the_mission_has() {
        let dir = data_folder("prep-open", true);
        fs::write(dir.join("mission.txt"), MISSION).unwrap();
        fs::write(dir.join("server.conf"), "open-planes 0 4\n").unwrap();
        let error = prepare(&Options::default(), &dir).err().unwrap();
        assert!(
            error.contains("plane 4") && error.contains("0 to 3"),
            "{error}"
        );
        fs::write(dir.join("server.conf"), "open-planes 0 3\n").unwrap();
        assert!(prepare(&Options::default(), &dir).is_ok());
        let _ = fs::remove_dir_all(dir);
    }
}
