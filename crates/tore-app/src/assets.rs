use crate::{
    AppResult,
    media_source::{self, MediaSource},
};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};
use tore_formats::{Button, Pic};

const ART: &[&str] = &[
    "QUIKMIS3.PIC",
    "ORD_AIR3.PIC",
    "ROCKER00.PIC",
    "DIAL00.PIC",
    "DIAL04.PIC",
    "DIAL11.PIC",
    "DIAL13.PIC",
    "LIGHTON.PIC",
    "LIGHTOFF.PIC",
    "PANELFNT.PIC",
    "CHOOSEV.PIC",
    "CHOOSEAC.PIC",
    "CHOOSE3.PIC",
    "CHOOSEU.PIC",
    "CHOOSEM.PIC",
    "ACTDFLT.PIC",
    "ACTDFT0L.PIC",
    "ACTDFT0M.PIC",
    "ACTDFT0R.PIC",
    "ACTION0L.PIC",
    "ACTION0M.PIC",
    "ACTION0R.PIC",
    "ACTIOD0L.PIC",
    "ACTIOD0M.PIC",
    "ACTIOD0R.PIC",
    "FONTACT.PIC",
    "FONTACD.PIC",
    "MENUFONT.PIC",
    "BODYFONT.PIC",
    "ARMFONT.PIC",
    "SMLFONT.PIC",
];
const DATA: &[&str] = &[
    "CHOOSEAC.DLG",
    "MAINMENU.MNU",
    "FMENUD.MNU",
    "&CLICK.11K",
    "&BUTTON.11K",
    "&TOGGLE1.5K",
];
const MAX_PACK_BYTES: u64 = 256 * 1024 * 1024;
pub struct Assets {
    pub creator_options: tore_formats::ui::creator::Options,
    pub theater_resources: BTreeMap<String, Vec<u8>>,
    pub pics: BTreeMap<String, Pic>,
    pub buttons: Vec<Button>,
    pub sounds: BTreeMap<String, Vec<u8>>,
    pub music_scores: BTreeMap<String, Vec<u8>>,
    pub palette: [[u8; 3]; 256],
}
pub fn data_directory() -> AppResult<PathBuf> {
    if let Some(path) = std::env::var_os("TORE_DATA_DIR") {
        return Ok(PathBuf::from(path));
    }
    #[cfg(target_os = "macos")]
    let root = PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?)
        .join("Library/Application Support");
    #[cfg(target_os = "windows")]
    let root = PathBuf::from(std::env::var_os("APPDATA").ok_or("APPDATA is unset")?);
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let root = match std::env::var_os("XDG_DATA_HOME") {
        Some(path) => PathBuf::from(path),
        None => {
            PathBuf::from(std::env::var_os("HOME").ok_or("HOME is unset")?).join(".local/share")
        }
    };
    Ok(root.join("T.O.R.E-Fighters"))
}
/// How far an import has got, for the first-run screen. `total` is the number
/// of resources selected from the archive being read, when it is known.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Progress {
    pub archive: String,
    pub done: usize,
    pub total: Option<usize>,
}

/// A finished import: the decoded assets and the plain-words summary the
/// locate screen shows. The same facts are in `import-report.txt` in full.
pub(crate) struct ImportOutcome {
    pub assets: Assets,
    /// Shown by the pre-game shell in package F.
    #[allow(dead_code)]
    pub summary: Vec<String>,
}

fn pack_generation(path: &Path) -> Option<u128> {
    let name = path.file_name()?.to_str()?;
    let generation = name.strip_prefix("menu-")?.strip_suffix(".pack")?;
    if generation.is_empty() || !generation.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    generation.parse().ok()
}

/// Only called after the retained pack has been decoded successfully.
fn remove_older_packs(directory: &Path, retained: &Path) -> std::io::Result<usize> {
    let Some(generation) = pack_generation(retained) else {
        return Ok(0);
    };
    let mut removed = 0;
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && pack_generation(&entry.path()).is_some_and(|old| old < generation)
        {
            fs::remove_file(entry.path())?;
            removed += 1;
        }
    }
    Ok(removed)
}

fn cleanup_previous_imports(directory: &Path, retained: &Path) {
    match remove_older_packs(directory, retained) {
        Ok(0) => {}
        Ok(count) => println!("Removed previous import packs: {count}"),
        Err(error) => eprintln!(
            "Could not finish cleaning older import packs in {}: {error}",
            directory.display()
        ),
    }
}

impl Assets {
    fn decode(resources: &BTreeMap<String, Vec<u8>>) -> AppResult<Self> {
        if resources.get("TORE_MUSIC_V1").map(Vec::as_slice) != Some(b"PCM1") {
            return Err("cache predates recorded music profile; re-import media".into());
        }
        if resources.get("TORE_COMBAT_V1").map(Vec::as_slice) != Some(b"RAW1") {
            return Err("cache predates combat dependencies; re-import media".into());
        }
        if resources.get("TORE_AIRPORTS_V1").map(Vec::as_slice) != Some(b"SCENE1") {
            return Err("cache predates airport scene dependencies; re-import media".into());
        }
        for &name in tore_formats::aircraft::COMBAT_RESOURCES {
            if !resources.contains_key(name) {
                return Err(
                    format!("cache missing combat resource {name}; re-import media").into(),
                );
            }
        }
        let mut music_scores = BTreeMap::new();
        for name in tore_formats::music::SCORES {
            if let Some(bytes) = resources.get(*name) {
                tore_formats::music::Score::parse(bytes)?;
                music_scores.insert(name.to_string(), bytes.clone());
            }
        }
        for name in [
            "&GEARUP.5K",
            "&STALLWR.5K",
            "&STALL.5K",
            "WIN11.FNT",
            "HUDSYM11.FNT",
            "HUD11.FNT",
            "RAFALE.PT",
            "RAFALE.HUD",
            "~RAFH.PIC",
            "F18.PT",
            "F18.HUD",
            "~F18H.PIC",
            "WIN01.FNT",
            "UKR.T2",
            "UKR.MM",
            "SUN.SH",
            "MOON.SH",
            "STARS.SH",
            "OCEAN0.PIC",
            "_MOON.PIC",
            "_CLOUD1.PIC",
        ] {
            if !resources.contains_key(name) {
                return Err(format!("cache missing {name}; re-import media").into());
            }
        }
        for (code, _) in tore_formats::theater::THEATERS {
            if !resources.contains_key(&format!("{code}.MM")) {
                return Err(format!("cache missing {code}.MM; re-import all theaters").into());
            }
        }
        if !resources.contains_key("TVI0.PIC") {
            return Err("cache missing Vietnam textures; re-import media".into());
        }
        for id in tore_formats::aircraft::AircraftId::ALL {
            for name in [
                id.hud().to_string(),
                id.cockpit().to_string(),
                format!("{}.SH", id.stem()),
                format!("_{}.PIC", id.stem()),
            ] {
                if !resources.contains_key(&name) {
                    return Err(format!("cache missing {name}; re-import media").into());
                }
            }
            tore_formats::aircraft::Aircraft::parse(
                resources
                    .get(id.pt())
                    .ok_or_else(|| format!("cache missing {}; re-import media", id.pt()))?,
            )?;
        }
        tore_formats::font::Font::parse(&resources["WIN11.FNT"])?;
        let mut pics = BTreeMap::new();
        for name in ART {
            let bytes = resources
                .get(*name)
                .ok_or_else(|| format!("menu cache missing {name}; re-import media"))?;
            pics.insert(name.to_string(), Pic::parse(bytes)?);
        }
        for (name, bytes) in resources.iter().filter(|(n, _)| {
            n.starts_with('$') && n.ends_with(".PIC")
                || ["MCICONS.PIC", "FNTWPNB.PIC", "FNTWPNY.PIC"].contains(&n.as_str())
        }) {
            pics.insert(name.clone(), Pic::parse(bytes)?);
        }
        for name in [
            "CHOOSEV.PIC",
            "CHOOSEAC.PIC",
            "CHOOSE3.PIC",
            "CHOOSEU.PIC",
            "CHOOSEM.PIC",
        ] {
            let background = &pics[name];
            if background.width != 640
                || background.height != 480
                || background.palette.len() != 256
            {
                return Err("expected 640x480 menu backgrounds with full palettes".into());
            }
        }
        let background = &pics["CHOOSEV.PIC"];
        let palette = background
            .palette
            .clone()
            .try_into()
            .map_err(|_| "invalid background palette")?;
        for name in [
            "FONTACT.PIC",
            "FONTACD.PIC",
            "MENUFONT.PIC",
            "BODYFONT.PIC",
            "ARMFONT.PIC",
            "SMLFONT.PIC",
        ] {
            if pics[name].glyphs.len() != 256 {
                return Err(format!("{name}: missing glyph table").into());
            }
        }
        for prefix in ["ACTION0", "ACTIOD0"] {
            for part in ["L", "M", "R"] {
                let pic = &pics[&format!("{prefix}{part}.PIC")];
                if pic.width == 0 || pic.width > 32 || pic.height != 30 {
                    return Err("unsupported action sprite dimensions".into());
                }
            }
        }
        let buttons = tore_formats::activity_buttons(
            resources
                .get("CHOOSEAC.DLG")
                .ok_or("missing CHOOSEAC.DLG")?,
        )?;
        let sounds = resources
            .iter()
            .filter(|(name, _)| name.ends_with(".11K") || name.ends_with(".5K"))
            .map(|(name, bytes)| (name.clone(), bytes.clone()))
            .collect::<BTreeMap<_, _>>();
        if sounds.values().any(|s| s.is_empty() || s.len() > 1_000_000) {
            return Err("invalid menu PCM size".into());
        }
        tore_formats::weather::clouds::Layout::decode(
            resources
                .get("TORE_CLOUDS_V1")
                .ok_or("cache predates cloud layout; re-import media")?,
        )?;
        tore_formats::weather::flare::Layout::decode(
            resources
                .get("TORE_FLARE_V1")
                .ok_or("cache predates lens flare; re-import media")?,
        )?;
        let creator_options = tore_formats::ui::creator::Options::decode(
            resources
                .get("TORE_CREATOR_V1")
                .ok_or("cache predates creator options; re-import media")?,
        )?;
        Ok(Self {
            creator_options,
            theater_resources: resources
                .iter()
                .filter(|(name, _)| !tore_formats::music::resource(name))
                .map(|(n, b)| (n.clone(), b.clone()))
                .collect(),
            pics,
            buttons,
            sounds,
            music_scores,
            palette,
        })
    }
    /// Import from a path the caller has not classified yet, for the CLI.
    pub(crate) fn import_path(source: &Path, destination: &Path) -> AppResult<Self> {
        // The plain-words reason is what a terminal user needs, not the variant.
        let source = MediaSource::detect(source).map_err(|error| error.to_string())?;
        Self::import(&source, destination)
    }
    pub(crate) fn import(source: &MediaSource, destination: &Path) -> AppResult<Self> {
        Ok(Self::import_with_progress(source, destination, &mut |_| {})?.assets)
    }
    /// Import with progress reports, at least once per archive and every 64
    /// resources. The callback runs on the importing thread.
    pub(crate) fn import_with_progress(
        source: &MediaSource,
        destination: &Path,
        progress: &mut dyn FnMut(Progress),
    ) -> AppResult<ImportOutcome> {
        let mut resources = BTreeMap::new();
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
        let aircraft_libs = [source.archive("FA_1.LIB")?, source.archive("FA_2.LIB")?];
        let aircraft_names = tore_formats::aircraft::dependencies(
            &aircraft_libs.iter().collect::<Vec<_>>(),
            &tore_formats::aircraft::AircraftId::ALL,
            true,
        )?;
        let scene_layouts: Vec<String> = tore_formats::theater::THEATERS
            .iter()
            .map(|(code, _)| format!("{code}.MM"))
            .collect();
        let scene_names = tore_formats::mission::scene_dependencies(
            &aircraft_libs.iter().collect::<Vec<_>>(),
            &scene_layouts,
        )?;
        for (filename, names) in [("FA_1.LIB", ART), ("FA_2.LIB", DATA)] {
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
                        || n.as_str() == "MCICONS.PIC"
                        || aircraft_names.contains(*n)
                        || scene_names.contains(*n)
                        || tore_formats::ui::creator::resource(n)
                        || tore_formats::music::resource(n)
                        || tore_formats::radio::resource(n)
                        || tore_formats::theater::theater_resource(n, "ALL")
                })
                .cloned()
                .collect();
            progress(Progress {
                archive: filename.to_string(),
                done: 0,
                total: Some(selected.len()),
            });
            for (index, name) in selected.iter().enumerate() {
                if index > 0 && index.is_multiple_of(64) {
                    progress(Progress {
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
            progress(Progress {
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
                    eprintln!("Optional recorded music unavailable: {error}");
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
            progress(Progress {
                archive: filename.to_string(),
                done: 0,
                total: Some(scores.len()),
            });
            summary.push(format!("{filename}: {} music resources read", scores.len()));
            for (index, name) in scores.iter().enumerate() {
                if index > 0 && index.is_multiple_of(64) {
                    progress(Progress {
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
            progress(Progress {
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
        let assets = Self::decode(&resources)?;
        fs::create_dir_all(destination)?;
        // Generation files keep the previous import usable until the new pack is complete.
        let generation = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let encoded_size = 16u64
            + resources
                .iter()
                .map(|(name, bytes)| 6 + name.len() as u64 + bytes.len() as u64)
                .sum::<u64>();
        if encoded_size > MAX_PACK_BYTES || resources.len() > 4096 {
            return Err(format!(
                "import exceeds cache bounds: {} resources, {} bytes",
                resources.len(),
                encoded_size
            )
            .into());
        }
        let path = destination.join(format!("menu-{generation}.pack"));
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        file.write_all(b"TOREMENU\x01\0\0\0")?;
        file.write_all(&(resources.len() as u32).to_le_bytes())?;
        for (name, bytes) in resources {
            file.write_all(&(name.len() as u16).to_le_bytes())?;
            file.write_all(name.as_bytes())?;
            file.write_all(&(bytes.len() as u32).to_le_bytes())?;
            file.write_all(&bytes)?;
        }
        file.sync_all()?;
        drop(file);
        fs::write(destination.join("import-report.txt"), report)?;
        // Verify the on-disk pack before removing any previously usable import.
        drop(assets);
        let assets = Self::load_pack(&path)?;
        cleanup_previous_imports(destination, &path);
        // The remembered source only saves the player a second choice; failing to
        // write it does not spoil a finished import.
        if let Err(error) = media_source::remember(destination, source) {
            eprintln!("Could not remember the media source: {error}");
            summary.push(format!("Media source not remembered: {error}"));
        }
        println!(
            "Imported menu and all theater resources to {}",
            path.display()
        );
        Ok(ImportOutcome { assets, summary })
    }
    pub fn load(directory: &Path) -> AppResult<Self> {
        let mut paths = fs::read_dir(directory)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("menu-"))
                    && p.extension().is_some_and(|e| e == "pack")
            })
            .collect::<Vec<_>>();
        paths.sort();
        let mut last_error =
            "No imported menu. Run with --import gameassets/fighters-anthology".to_string();
        for path in paths.into_iter().rev() {
            match Self::load_pack(&path) {
                Ok(assets) => {
                    cleanup_previous_imports(directory, &path);
                    return Ok(assets);
                }
                Err(error) => {
                    last_error = format!("{}: {error}", path.display());
                    eprintln!("Ignoring invalid menu cache: {last_error}");
                }
            }
        }
        Err(last_error.into())
    }
    fn load_pack(path: &Path) -> AppResult<Self> {
        let file = fs::File::open(path)?;
        if file.metadata()?.len() > MAX_PACK_BYTES {
            return Err("asset pack exceeds 256 MiB".into());
        }
        let mut data = Vec::new();
        file.take(MAX_PACK_BYTES + 1).read_to_end(&mut data)?;
        let mut cursor = std::io::Cursor::new(data);
        let mut header = [0; 12];
        cursor.read_exact(&mut header)?;
        if &header != b"TOREMENU\x01\0\0\0" {
            return Err("unsupported menu pack".into());
        }
        fn word(c: &mut impl Read) -> AppResult<usize> {
            let mut b = [0; 4];
            c.read_exact(&mut b)?;
            Ok(u32::from_le_bytes(b) as usize)
        }
        let count = word(&mut cursor)?;
        if count > 4096 {
            return Err("too many menu resources".into());
        }
        let mut resources = BTreeMap::new();
        for _ in 0..count {
            let mut len = [0; 2];
            cursor.read_exact(&mut len)?;
            let len = u16::from_le_bytes(len) as usize;
            if len == 0 || len > 32 {
                return Err("invalid resource name length".into());
            }
            let mut name = vec![0; len];
            cursor.read_exact(&mut name)?;
            let name = String::from_utf8(name)?;
            let length = word(&mut cursor)?;
            if length > 2 * 1024 * 1024 {
                return Err("menu resource exceeds limit".into());
            }
            let mut bytes = vec![0; length];
            cursor.read_exact(&mut bytes)?;
            if resources.insert(name, bytes).is_some() {
                return Err("duplicate menu resource".into());
            }
        }
        if cursor.position() != cursor.get_ref().len() as u64 {
            return Err("trailing menu pack bytes".into());
        }
        Self::decode(&resources)
    }
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct CacheDirectory(PathBuf);
    impl CacheDirectory {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "tore-cache-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for CacheDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    #[test]
    fn cleanup_removes_only_older_numbered_regular_packs() {
        let directory = CacheDirectory::new();
        for name in [
            "menu-9.pack",
            "menu-10.pack",
            "menu-20.pack",
            "menu-30.pack",
            "menu-backup.pack",
            "menu-+1.pack",
            "preferences.conf",
            "other.pack",
        ] {
            fs::write(directory.0.join(name), b"synthetic").unwrap();
        }
        fs::create_dir(directory.0.join("menu-1.pack")).unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("preferences.conf", directory.0.join("menu-2.pack")).unwrap();
        assert_eq!(
            remove_older_packs(&directory.0, &directory.0.join("menu-20.pack")).unwrap(),
            2
        );
        assert!(!directory.0.join("menu-9.pack").exists());
        assert!(!directory.0.join("menu-10.pack").exists());
        for name in [
            "menu-20.pack",
            "menu-30.pack",
            "menu-backup.pack",
            "menu-+1.pack",
            "preferences.conf",
            "other.pack",
            "menu-1.pack",
        ] {
            assert!(directory.0.join(name).exists(), "removed {name}");
        }
        #[cfg(unix)]
        assert!(directory.0.join("menu-2.pack").is_symlink());
    }

    #[test]
    fn failed_load_preserves_all_existing_packs() {
        let directory = CacheDirectory::new();
        for name in ["menu-10.pack", "menu-20.pack"] {
            fs::write(directory.0.join(name), b"invalid synthetic pack").unwrap();
        }
        assert!(Assets::load(&directory.0).is_err());
        assert!(directory.0.join("menu-10.pack").exists());
        assert!(directory.0.join("menu-20.pack").exists());
    }

    #[test]
    fn non_generation_selection_does_not_authorize_cleanup() {
        let directory = CacheDirectory::new();
        fs::write(directory.0.join("menu-10.pack"), b"synthetic").unwrap();
        assert_eq!(
            remove_older_packs(&directory.0, &directory.0.join("menu-custom.pack")).unwrap(),
            0
        );
        assert!(directory.0.join("menu-10.pack").exists());
    }
}
