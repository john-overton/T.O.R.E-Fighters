//! The import pack: one file holding every imported resource, written in
//! generations so the previous import stays usable until the new one is
//! verified. Behaviour: `docs/spec/import-cache.md`.
//!
//! Format: the 12-byte magic `TOREMENU\x01\0\0\0`, a `u32` resource count, then
//! per resource a `u16` name length, the name (1 to 32 bytes), a `u32` length
//! and the bytes. Files are `menu-<nanoseconds>.pack`; the highest number is the
//! newest.

use crate::{ImportResult, Level, Resources};
use std::{
    fs,
    io::{Read, Write},
    path::Path,
};

/// Sanity bounds on the import pack, far above a full Fighters Anthology
/// import (about 4,100 resources and 180 MB in 2026-09), so a corrupt file is
/// refused without capping what an import may hold. The pack is read as a
/// stream, so memory follows the resources kept, not the bound.
const MAX_PACK_BYTES: u64 = 1024 * 1024 * 1024;
const MAX_PACK_RESOURCES: usize = 32_768;
const MAX_RESOURCE_BYTES: usize = 2 * 1024 * 1024;
pub(crate) fn pack_generation(path: &Path) -> Option<u128> {
    let name = path.file_name()?.to_str()?;
    let generation = name.strip_prefix("menu-")?.strip_suffix(".pack")?;
    if generation.is_empty() || !generation.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    generation.parse().ok()
}

/// Only called after the retained pack has been decoded successfully.
pub(crate) fn remove_older_packs(directory: &Path, retained: &Path) -> std::io::Result<usize> {
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

pub(crate) fn cleanup_previous_imports(directory: &Path, retained: &Path) {
    match remove_older_packs(directory, retained) {
        Ok(0) => {}
        Ok(count) => crate::note(
            Level::Info,
            &format!("Removed previous import packs: {count}"),
        ),
        Err(error) => crate::note(
            Level::Warn,
            &format!(
                "Could not finish cleaning older import packs in {}: {error}",
                directory.display()
            ),
        ),
    }
}

/// What loading a pack produced: the resources as stored, and whatever the
/// caller's decoder made of them.
pub struct Loaded<T> {
    pub resources: Resources,
    pub value: T,
}

/// The markers the import writes last, each naming a revision of one kind of
/// derived data. A pack that lacks one predates it and must be re-imported.
pub fn check_markers(resources: &Resources) -> ImportResult<()> {
    if resources.get("TORE_MUSIC_V1").map(Vec::as_slice) != Some(b"PCM1") {
        return Err("cache predates recorded music profile; re-import media".into());
    }
    if resources.get("TORE_COMBAT_V1").map(Vec::as_slice) != Some(b"RAW1") {
        return Err("cache predates combat dependencies; re-import media".into());
    }
    if resources.get("TORE_AIRPORTS_V1").map(Vec::as_slice) != Some(b"SCENE1") {
        return Err("cache predates airport scene dependencies; re-import media".into());
    }
    if resources.get("TORE_SPEECH_V1").map(Vec::as_slice) != Some(b"ALL1") {
        return Err("cache predates the full radio speech set; re-import media".into());
    }
    Ok(())
}

/// The marker of the multiplayer screens' art (slice EF1): pictures, dialogs,
/// menus and the quick-message file listed in [`crate::selection`]. The import
/// writes it with the others, but [`check_markers`] does not ask for it: the
/// dedicated server needs none of that art and must keep running on a pack an
/// older import made. The game asks with [`check_multiplayer_marker`].
pub const MULTIPLAYER_MARKER: &str = "TORE_MULTIPLAYER_V1";
/// What the marker holds.
pub const MULTIPLAYER_MARKER_VALUE: &[u8] = b"ART1";

/// The game's extra check: a pack from before the multiplayer art was kept
/// must be re-imported once. Call it after [`check_markers`] in the game's
/// decoder. The dedicated server and the bot do not call it.
pub fn check_multiplayer_marker(resources: &Resources) -> ImportResult<()> {
    if resources.get(MULTIPLAYER_MARKER).map(Vec::as_slice) != Some(MULTIPLAYER_MARKER_VALUE) {
        return Err("cache predates the multiplayer screens' art; re-import media".into());
    }
    Ok(())
}

/// Loads the newest pack in `directory` that `decode` accepts, then deletes the
/// older generations. A pack `decode` refuses is skipped and kept, and the next
/// older one is tried; if none is good the error names the last one tried.
///
/// `decode` is the caller's check of what it needs from the resources and
/// should start with [`check_markers`]. The game decodes its menu art there;
/// the server checks only the simulation data.
pub fn load_with<T>(
    directory: &Path,
    decode: &dyn Fn(&Resources) -> ImportResult<T>,
) -> ImportResult<Loaded<T>> {
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
        match load_pack(&path, decode) {
            Ok(loaded) => {
                cleanup_previous_imports(directory, &path);
                return Ok(loaded);
            }
            Err(error) => {
                last_error = format!("{}: {error}", path.display());
                crate::note(
                    Level::Warn,
                    &format!("Ignoring invalid menu cache: {last_error}"),
                );
            }
        }
    }
    Err(last_error.into())
}

/// Loads the newest pack that has every marker, for callers that only need the
/// resources.
pub fn load(directory: &Path) -> ImportResult<Resources> {
    Ok(load_with(directory, &check_markers)?.resources)
}

pub(crate) fn load_pack<T>(
    path: &Path,
    decode: &dyn Fn(&Resources) -> ImportResult<T>,
) -> ImportResult<Loaded<T>> {
    let resources = read_pack(path)?;
    let value = decode(&resources)?;
    Ok(Loaded { resources, value })
}

/// Writes a new pack. `path` must not exist yet; the file is synced before
/// returning.
pub fn write_pack(path: &Path, resources: &Resources) -> ImportResult<()> {
    let encoded_size = 16u64
        + resources
            .iter()
            .map(|(name, bytes)| 6 + name.len() as u64 + bytes.len() as u64)
            .sum::<u64>();
    if encoded_size > MAX_PACK_BYTES
        || resources.len() > MAX_PACK_RESOURCES
        || resources.iter().any(|(name, bytes)| {
            !(1..=32).contains(&name.len()) || bytes.len() > MAX_RESOURCE_BYTES
        })
    {
        return Err(format!(
            "import exceeds cache bounds: {} resources, {} bytes",
            resources.len(),
            encoded_size
        )
        .into());
    }
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    let mut out = std::io::BufWriter::new(file);
    out.write_all(b"TOREMENU\x01\0\0\0")?;
    out.write_all(&(resources.len() as u32).to_le_bytes())?;
    for (name, bytes) in resources {
        out.write_all(&(name.len() as u16).to_le_bytes())?;
        out.write_all(name.as_bytes())?;
        out.write_all(&(bytes.len() as u32).to_le_bytes())?;
        out.write_all(bytes)?;
    }
    out.into_inner()
        .map_err(|error| error.into_error())?
        .sync_all()?;
    Ok(())
}

pub fn read_pack(path: &Path) -> ImportResult<Resources> {
    let file = fs::File::open(path)?;
    let size = file.metadata()?.len();
    if size > MAX_PACK_BYTES {
        return Err("asset pack exceeds 1 GiB".into());
    }
    let mut cursor = std::io::BufReader::new(file.take(size));
    let mut header = [0; 12];
    cursor.read_exact(&mut header)?;
    if &header != b"TOREMENU\x01\0\0\0" {
        return Err("unsupported menu pack".into());
    }
    fn word(c: &mut impl Read) -> ImportResult<usize> {
        let mut b = [0; 4];
        c.read_exact(&mut b)?;
        Ok(u32::from_le_bytes(b) as usize)
    }
    let count = word(&mut cursor)?;
    if count > MAX_PACK_RESOURCES {
        return Err("too many menu resources".into());
    }
    let mut resources = Resources::new();
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
        if length > MAX_RESOURCE_BYTES {
            return Err("menu resource exceeds limit".into());
        }
        let mut bytes = vec![0; length];
        cursor.read_exact(&mut bytes)?;
        if resources.insert(name, bytes).is_some() {
            return Err("duplicate menu resource".into());
        }
    }
    if cursor.read(&mut [0])? != 0 {
        return Err("trailing menu pack bytes".into());
    }
    Ok(resources)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::{collections::BTreeMap, path::PathBuf};

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
    fn packs_hold_more_than_the_old_four_thousand_resources() {
        let directory = CacheDirectory::new();
        let path = directory.0.join("menu-1.pack");
        let resources: Resources = (0..9000u32)
            .map(|n| (format!("R{n:05}.RAW"), n.to_le_bytes().to_vec()))
            .collect();
        write_pack(&path, &resources).unwrap();
        assert_eq!(read_pack(&path).unwrap(), resources);
        // A pack is never overwritten in place.
        assert!(write_pack(&path, &resources).is_err());
        let mut bytes = fs::read(&path).unwrap();
        bytes.push(0);
        fs::write(&path, &bytes).unwrap();
        assert!(read_pack(&path).is_err());
        bytes.truncate(bytes.len() - 2);
        fs::write(&path, &bytes).unwrap();
        assert!(read_pack(&path).is_err());
    }

    #[test]
    fn packs_refuse_resources_the_reader_would_reject() {
        let directory = CacheDirectory::new();
        for (name, size) in [
            ("", 1),
            ("A_NAME_LONGER_THAN_THIRTY_TWO_BYTES", 1),
            ("BIG.RAW", MAX_RESOURCE_BYTES + 1),
        ] {
            let path = directory.0.join("menu-2.pack");
            let resources = BTreeMap::from([(name.to_string(), vec![0; size])]);
            assert!(write_pack(&path, &resources).is_err(), "{name}");
            assert!(!path.exists());
        }
    }

    #[test]
    fn failed_load_preserves_all_existing_packs() {
        let directory = CacheDirectory::new();
        for name in ["menu-10.pack", "menu-20.pack"] {
            fs::write(directory.0.join(name), b"invalid synthetic pack").unwrap();
        }
        assert!(load(&directory.0).is_err());
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

    fn marked() -> Resources {
        BTreeMap::from([
            ("TORE_MUSIC_V1".to_string(), b"PCM1".to_vec()),
            ("TORE_COMBAT_V1".to_string(), b"RAW1".to_vec()),
            ("TORE_AIRPORTS_V1".to_string(), b"SCENE1".to_vec()),
            ("TORE_SPEECH_V1".to_string(), b"ALL1".to_vec()),
            ("F18.PT".to_string(), vec![1, 2, 3]),
        ])
    }

    #[test]
    fn load_returns_the_newest_pack_with_every_marker_and_prunes_older_ones() {
        let directory = CacheDirectory::new();
        let newest = marked();
        let mut older = marked();
        older.insert("F18.PT".to_string(), vec![9]);
        write_pack(&directory.0.join("menu-10.pack"), &older).unwrap();
        write_pack(&directory.0.join("menu-20.pack"), &newest).unwrap();
        assert_eq!(load(&directory.0).unwrap(), newest);
        assert!(!directory.0.join("menu-10.pack").exists());
    }

    #[test]
    fn load_with_skips_a_pack_the_decoder_refuses_and_keeps_it() {
        let directory = CacheDirectory::new();
        let good = marked();
        let mut stale = marked();
        stale.remove("TORE_SPEECH_V1");
        write_pack(&directory.0.join("menu-10.pack"), &good).unwrap();
        write_pack(&directory.0.join("menu-20.pack"), &stale).unwrap();
        let loaded = load_with(&directory.0, &|r| {
            check_markers(r)?;
            Ok(r.len())
        })
        .unwrap();
        assert_eq!(loaded.resources, good);
        assert_eq!(loaded.value, 5);
        // The refused newer pack is neither loaded nor deleted.
        assert!(directory.0.join("menu-20.pack").exists());
        assert!(directory.0.join("menu-10.pack").exists());
    }

    #[test]
    fn a_missing_marker_names_what_to_do() {
        let mut resources = marked();
        check_markers(&resources).unwrap();
        resources.insert("TORE_COMBAT_V1".to_string(), b"RAW0".to_vec());
        let error = check_markers(&resources).unwrap_err().to_string();
        assert!(error.contains("re-import media"), "{error}");
    }

    #[test]
    fn a_pack_without_the_multiplayer_marker_is_refused_by_the_game_only() {
        let mut resources = marked();
        // The shared check, used by the dedicated server and the bot, accepts
        // a pack an older import wrote.
        check_markers(&resources).unwrap();
        let error = check_multiplayer_marker(&resources)
            .unwrap_err()
            .to_string();
        assert!(error.contains("re-import media"), "{error}");
        resources.insert(MULTIPLAYER_MARKER.to_string(), b"ART0".to_vec());
        assert!(check_multiplayer_marker(&resources).is_err());
        resources.insert(
            MULTIPLAYER_MARKER.to_string(),
            MULTIPLAYER_MARKER_VALUE.to_vec(),
        );
        check_multiplayer_marker(&resources).unwrap();
        check_markers(&resources).unwrap();
    }

    #[test]
    fn the_multiplayer_marker_name_fits_the_pack() {
        assert!((1..=32).contains(&MULTIPLAYER_MARKER.len()));
        assert!((1..=32).contains(&crate::selection::CHAT_RESOURCE.len()));
        let directory = CacheDirectory::new();
        let mut resources = marked();
        resources.insert(
            MULTIPLAYER_MARKER.to_string(),
            MULTIPLAYER_MARKER_VALUE.to_vec(),
        );
        let path = directory.0.join("menu-1.pack");
        write_pack(&path, &resources).unwrap();
        assert_eq!(read_pack(&path).unwrap(), resources);
    }
}
