//! Fighters Anthology media sources: installed folders and disc folders, detected by content.
//! Behaviour: docs/spec/first-run-import.md. Work package C owns this file.
//!
//! A source is always a folder. An installed folder holds `FA_1.LIB`,
//! `FA_2.LIB` and `FA.EXE` as loose files; a disc folder holds an Electronic
//! Arts installer container, normally `SETUP.ESA`, with the same files inside.
//! Detection never trusts a name: the installed folder is recognised by the
//! three files and the disc folder by the container signature. Reading a disc
//! never copies an archive into memory, the container serves stored archives in
//! place by offset.

use crate::{AppResult, preferences};
use std::{
    collections::HashSet,
    fmt, fs,
    io::{self, Read},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};
use tore_formats::{Archive, esa};

/// Every reader in the importer stays inside this bound, as the executable reader does.
const EXECUTABLE_LIMIT: usize = 16 * 1024 * 1024;
/// Files probed for the container signature in one folder when no `.esa` name matches.
const MAGIC_PROBES: usize = 32;
/// Name of the remembered-source file beside the imported pack.
const REMEMBERED: &str = "media-source.txt";

/// How a folder provides the five files the importer needs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// Loose `FA_1.LIB`, `FA_2.LIB`, `FA.EXE` and optional music archives.
    Installed,
    /// An Electronic Arts installer container holding the same files.
    Disc,
}
impl Kind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Kind::Installed => "Installed folder",
            Kind::Disc => "Disc or mounted image",
        }
    }
    /// The word written to `media-source.txt`.
    fn word(self) -> &'static str {
        match self {
            Kind::Installed => "installed",
            Kind::Disc => "disc",
        }
    }
    fn from_word(word: &str) -> Option<Self> {
        match word {
            "installed" => Some(Kind::Installed),
            "disc" => Some(Kind::Disc),
            _ => None,
        }
    }
}

/// A folder the importer can read, with the container path when it is a disc.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MediaSource {
    /// The folder the player chose, or the folder holding the container.
    pub path: PathBuf,
    pub kind: Kind,
    /// The installer container, for [`Kind::Disc`] only.
    pub container: Option<PathBuf>,
}

/// Why a path is not a source. Every message is plain words for the screen.
#[derive(Debug)]
pub(crate) enum DetectError {
    NotFound(PathBuf),
    NotASource(PathBuf),
    RawImage(PathBuf),
    Io(io::Error),
}
impl fmt::Display for DetectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DetectError::NotFound(path) => write!(f, "{} does not exist", path.display()),
            DetectError::NotASource(path) => write!(
                f,
                "{} is not a Fighters Anthology source. Choose an installed game folder, or the folder of a mounted disc 1.",
                path.display()
            ),
            DetectError::RawImage(path) => write!(
                f,
                "{} is a disc image. Mount the image and choose the mounted folder: double-click it on Windows and macOS, or use the file manager or udisksctl loop-setup on Linux.",
                path.display()
            ),
            DetectError::Io(error) => write!(f, "{error}"),
        }
    }
}
impl std::error::Error for DetectError {}
impl From<io::Error> for DetectError {
    fn from(error: io::Error) -> Self {
        DetectError::Io(error)
    }
}

/// Extensions of raw disc images, which are never opened.
const IMAGE_EXTENSIONS: [&str; 4] = ["iso", "img", "bin", "cue"];

/// The regular files of a folder, keyed by their real name.
fn files(directory: &Path) -> io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        // DirEntry::file_type does not follow symlinks, so a linked folder is skipped.
        if entry.file_type()?.is_file() {
            found.push(entry.path());
        }
    }
    found.sort();
    Ok(found)
}

fn named<'a>(files: &'a [PathBuf], name: &str) -> Option<&'a PathBuf> {
    files.iter().find(|path| {
        path.file_name()
            .is_some_and(|found| found.to_string_lossy().eq_ignore_ascii_case(name))
    })
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .is_some_and(|found| found.to_string_lossy().eq_ignore_ascii_case(extension))
}

/// True when the file starts with the installer container signature.
fn is_container(path: &Path) -> bool {
    let mut head = [0; 64];
    let Ok(mut file) = fs::File::open(path) else {
        return false;
    };
    let mut read = 0;
    while read < head.len() {
        match file.read(&mut head[read..]) {
            Ok(0) => break,
            Ok(count) => read += count,
            Err(_) => return false,
        }
    }
    esa::has_magic(&head[..read])
}

/// The installer container in a folder, if one is there. `SETUP.ESA` is tried
/// first by name but still confirmed by signature, then any `.esa` file, then a
/// bounded number of other files, because a copied disc may rename the container.
fn container_in(files: &[PathBuf]) -> Option<PathBuf> {
    if let Some(path) = named(files, "SETUP.ESA")
        && is_container(path)
    {
        return Some(path.clone());
    }
    if let Some(path) = files
        .iter()
        .find(|path| has_extension(path, "esa") && is_container(path))
    {
        return Some(path.clone());
    }
    files
        .iter()
        .filter(|path| !has_extension(path, "esa"))
        .take(MAGIC_PROBES)
        .find(|path| is_container(path))
        .cloned()
}

/// Decide a folder's kind by its contents alone.
fn detect_directory(directory: &Path) -> Result<MediaSource, DetectError> {
    let files = files(directory)?;
    if ["FA_1.LIB", "FA_2.LIB", "FA.EXE"]
        .iter()
        .all(|name| named(&files, name).is_some())
    {
        return Ok(MediaSource {
            path: directory.to_path_buf(),
            kind: Kind::Installed,
            container: None,
        });
    }
    match container_in(&files) {
        Some(container) => Ok(MediaSource {
            path: directory.to_path_buf(),
            kind: Kind::Disc,
            container: Some(container),
        }),
        None => Err(DetectError::NotASource(directory.to_path_buf())),
    }
}

impl MediaSource {
    /// Decide what a chosen path is, by content. A folder is examined directly;
    /// a container file names its own folder; a raw disc image is refused with
    /// the hint to mount it; any other file falls back to its folder.
    pub(crate) fn detect(path: &Path) -> Result<MediaSource, DetectError> {
        let metadata = match fs::metadata(path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Err(DetectError::NotFound(path.to_path_buf()));
            }
            Err(error) => return Err(DetectError::Io(error)),
        };
        if metadata.is_dir() {
            return detect_directory(path);
        }
        if !metadata.is_file() {
            return Err(DetectError::NotASource(path.to_path_buf()));
        }
        if is_container(path) {
            let folder = path
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty());
            return Ok(MediaSource {
                path: folder.unwrap_or(Path::new(".")).to_path_buf(),
                kind: Kind::Disc,
                container: Some(path.to_path_buf()),
            });
        }
        if IMAGE_EXTENSIONS
            .iter()
            .any(|extension| has_extension(path, extension))
        {
            return Err(DetectError::RawImage(path.to_path_buf()));
        }
        match path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            Some(parent) => detect_directory(parent),
            None => Err(DetectError::NotASource(path.to_path_buf())),
        }
    }

    fn container(&self) -> AppResult<esa::Container> {
        let path = self
            .container
            .as_ref()
            .ok_or("this source has no installer container")?;
        Ok(esa::Container::open(path)?)
    }

    /// A required archive, `FA_1.LIB` or `FA_2.LIB`. A disc archive is served in
    /// place from inside the container; nothing is copied.
    pub(crate) fn archive(&self, name: &str) -> AppResult<Archive> {
        match self.kind {
            Kind::Installed => {
                let path = named(&files(&self.path)?, name)
                    .cloned()
                    .ok_or_else(|| format!("{}: missing {name}", self.path.display()))?;
                Ok(Archive::open(path)?)
            }
            Kind::Disc => Ok(self.container()?.archive(name)?),
        }
    }

    /// An archive the importer can do without, `FA_4B.LIB` or `FA_4D.LIB`.
    /// `Ok(None)` means the source does not carry it; an error means it is there
    /// but could not be read.
    pub(crate) fn optional_archive(&self, name: &str) -> AppResult<Option<Archive>> {
        match self.kind {
            Kind::Installed => match named(&files(&self.path)?, name).cloned() {
                Some(path) => Ok(Some(Archive::open(path)?)),
                None => Ok(None),
            },
            Kind::Disc => {
                let container = self.container()?;
                if container.entry(name).is_none() {
                    return Ok(None);
                }
                Ok(Some(container.archive(name)?))
            }
        }
    }

    /// The reviewed executable's bytes. Never executed, only read as data.
    pub(crate) fn executable(&self) -> AppResult<Vec<u8>> {
        match self.kind {
            Kind::Installed => {
                let path = named(&files(&self.path)?, "FA.EXE")
                    .cloned()
                    .ok_or_else(|| {
                        format!(
                            "{}: missing FA.EXE, which the importer reads for inert tables",
                            self.path.display()
                        )
                    })?;
                if fs::metadata(&path)?.len() > EXECUTABLE_LIMIT as u64 {
                    return Err("FA.EXE exceeds input bound".into());
                }
                let mut bytes = Vec::new();
                fs::File::open(path)?
                    .take(EXECUTABLE_LIMIT as u64 + 1)
                    .read_to_end(&mut bytes)?;
                Ok(bytes)
            }
            Kind::Disc => Ok(self.container()?.read("FA.EXE", EXECUTABLE_LIMIT)?),
        }
    }
}

/// Check one root and its immediate subdirectories, appending every hit.
/// Symlinked children are skipped and `seen` keeps one entry per real folder.
fn scan_root(
    root: &Path,
    deadline: Instant,
    seen: &mut HashSet<PathBuf>,
    found: &mut Vec<MediaSource>,
) {
    let mut consider = |directory: &Path, found: &mut Vec<MediaSource>| {
        let key = fs::canonicalize(directory).unwrap_or_else(|_| directory.to_path_buf());
        if !seen.insert(key) {
            return;
        }
        if let Ok(source) = detect_directory(directory) {
            found.push(source);
        }
    };
    if !root.is_dir() {
        return;
    }
    consider(root, found);
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    let mut children: Vec<PathBuf> = Vec::new();
    for entry in entries.flatten() {
        // is_dir on the entry's own type never follows a symlink.
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            children.push(entry.path());
        }
    }
    children.sort();
    for child in children {
        if Instant::now() >= deadline {
            return;
        }
        consider(&child, found);
    }
}

/// Every root the automatic scan looks at, in the spec's order.
fn roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut push = |path: PathBuf| {
        if !roots.contains(&path) {
            roots.push(path);
        }
    };
    if let Ok(executable) = std::env::current_exe()
        && let Some(beside) = executable.parent()
    {
        push(beside.to_path_buf());
        push(beside.join("gameassets/fighters-anthology"));
    }
    if let Ok(current) = std::env::current_dir() {
        push(current.join("gameassets/fighters-anthology"));
    }
    if cfg!(target_os = "linux") {
        for base in ["/run/media", "/media"] {
            push(PathBuf::from(base));
            if let Ok(entries) = fs::read_dir(base) {
                let mut users: Vec<PathBuf> = entries
                    .flatten()
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                    .map(|entry| entry.path())
                    .collect();
                users.sort();
                for user in users {
                    push(user);
                }
            }
        }
        push(PathBuf::from("/mnt"));
    }
    if cfg!(target_os = "macos") {
        push(PathBuf::from("/Volumes"));
    }
    if cfg!(target_os = "windows") {
        for letter in b'A'..=b'Z' {
            let drive = PathBuf::from(format!("{}:\\", letter as char));
            if drive.is_dir() {
                push(drive);
            }
        }
        for variable in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(program_files) = std::env::var_os(variable) {
                push(PathBuf::from(program_files).join("Jane's Combat Simulations"));
            }
        }
        push(PathBuf::from("C:\\JANES"));
    }
    roots
}

/// Automatically detected sources, in the spec's scan order, best effort. The
/// scan stops once `budget` is spent; anything slower is left to the path field.
pub(crate) fn candidates(budget: Duration) -> Vec<MediaSource> {
    let deadline = Instant::now() + budget;
    let mut seen = HashSet::new();
    let mut found = Vec::new();
    for root in roots() {
        if Instant::now() >= deadline {
            break;
        }
        scan_root(&root, deadline, &mut seen, &mut found);
    }
    found
}

/// Record the source a successful import read, beside the pack.
pub(crate) fn remember(data_dir: &Path, source: &MediaSource) -> io::Result<()> {
    // A relative path only means something from the directory the app was
    // launched in; store an absolute one so a later launch elsewhere finds it.
    let path = if source.path.is_absolute() {
        source.path.clone()
    } else {
        std::env::current_dir()?.join(&source.path)
    };
    let text = format!("path={}\nkind={}\n", path.display(), source.kind.word());
    preferences::write(&data_dir.join(REMEMBERED), &text)
}

/// The remembered source, if one was written and still parses.
pub(crate) fn remembered(data_dir: &Path) -> Option<(PathBuf, Kind)> {
    let text = preferences::read(&data_dir.join(REMEMBERED)).ok()?;
    let mut path = None;
    let mut kind = None;
    for line in text.lines() {
        if let Some(value) = line.strip_prefix("path=") {
            path = Some(PathBuf::from(value));
        } else if let Some(value) = line.strip_prefix("kind=") {
            kind = Kind::from_word(value.trim());
        }
    }
    Some((path?, kind?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempDir(PathBuf);
    impl TempDir {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "tore-source-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn join(&self, name: &str) -> PathBuf {
            self.0.join(name)
        }
        fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
            let path = self.join(name);
            fs::write(&path, bytes).unwrap();
            path
        }
        fn dir(&self, name: &str) -> PathBuf {
            let path = self.join(name);
            fs::create_dir_all(&path).unwrap();
            path
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    /// A one-entry EALIB archive, synthetic bytes, no retail data.
    fn ealib(name: &str, bytes: &[u8]) -> Vec<u8> {
        let mut data = b"EALIB\x01\x00".to_vec();
        let mut raw = [0; 13];
        raw[..name.len()].copy_from_slice(name.as_bytes());
        data.extend_from_slice(&raw);
        data.push(0);
        data.extend_from_slice(&43_u32.to_le_bytes());
        data.extend_from_slice(&[0; 14]);
        data.extend_from_slice(&(43 + bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(bytes);
        data
    }

    /// Minimal stored-only installer container, in the layout of
    /// docs/formats/esa-installer.md. Written twice so the offsets can be
    /// resolved once the directory length is known.
    fn container(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let build = |base: usize| {
            let mut directory = esa::MAGIC.as_bytes().to_vec();
            directory.push(0);
            let mut at = base;
            for (name, bytes) in entries {
                directory.extend_from_slice(name.as_bytes());
                directory.push(0);
                directory.extend_from_slice(b"FA_LIBS\0");
                directory.extend_from_slice(&0x211_u32.to_le_bytes());
                directory.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                directory.extend_from_slice(&0_u32.to_le_bytes());
                directory.extend_from_slice(b"NULL\0");
                directory.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
                directory.extend_from_slice(&(at as u32).to_le_bytes());
                at += bytes.len();
            }
            directory.push(0);
            directory
        };
        let mut data = build(build(0).len());
        for (_, bytes) in entries {
            data.extend_from_slice(bytes);
        }
        data
    }

    fn installed(directory: &TempDir) {
        directory.file("FA_1.LIB", &ealib("ONE.PIC", b"one"));
        directory.file("FA_2.LIB", &ealib("TWO.DLG", b"two"));
        directory.file("FA.EXE", b"synthetic executable");
    }

    #[test]
    fn installed_folder_is_recognised_by_its_three_files() {
        let directory = TempDir::new();
        installed(&directory);
        let source = MediaSource::detect(&directory.0).unwrap();
        assert_eq!(source.kind, Kind::Installed);
        assert_eq!(source.path, directory.0);
        assert!(source.container.is_none());
        assert_eq!(source.archive("FA_1.LIB").unwrap().entries.len(), 1);
        assert_eq!(source.executable().unwrap(), b"synthetic executable");
        assert!(source.optional_archive("FA_4B.LIB").unwrap().is_none());
    }

    #[test]
    fn installed_folder_is_recognised_whatever_the_case() {
        let directory = TempDir::new();
        directory.file("fa_1.lib", &ealib("ONE.PIC", b"one"));
        directory.file("Fa_2.Lib", &ealib("TWO.DLG", b"two"));
        directory.file("fa.exe", b"synthetic executable");
        let source = MediaSource::detect(&directory.0).unwrap();
        assert_eq!(source.kind, Kind::Installed);
        assert_eq!(source.archive("FA_2.LIB").unwrap().entries.len(), 1);
    }

    #[test]
    fn disc_folder_serves_archives_and_the_executable_from_the_container() {
        let directory = TempDir::new();
        directory.file(
            "SETUP.ESA",
            &container(&[
                ("FA.EXE", b"synthetic executable"),
                ("FA_1.LIB", &ealib("ONE.PIC", b"one")),
                ("FA_2.LIB", &ealib("TWO.DLG", b"two")),
            ]),
        );
        let source = MediaSource::detect(&directory.0).unwrap();
        assert_eq!(source.kind, Kind::Disc);
        assert_eq!(source.container, Some(directory.join("SETUP.ESA")));
        let archive = source.archive("FA_1.LIB").unwrap();
        assert_eq!(archive.read("ONE.PIC").unwrap(), b"one");
        assert_eq!(source.executable().unwrap(), b"synthetic executable");
        assert!(source.optional_archive("FA_4D.LIB").unwrap().is_none());
    }

    #[test]
    fn a_container_under_another_name_is_found_by_its_signature() {
        let directory = TempDir::new();
        directory.file("readme.txt", b"not a container");
        let path = directory.file("install.dat", &container(&[("FA.EXE", b"exe")]));
        let source = MediaSource::detect(&directory.0).unwrap();
        assert_eq!(source.kind, Kind::Disc);
        assert_eq!(source.container, Some(path));
    }

    #[test]
    fn a_file_named_setup_esa_that_is_not_a_container_is_ignored() {
        let directory = TempDir::new();
        directory.file("SETUP.ESA", b"not really a container");
        let error = MediaSource::detect(&directory.0).unwrap_err();
        assert!(
            matches!(error, DetectError::NotASource(_)),
            "{error:?}: {error}"
        );
    }

    #[test]
    fn choosing_a_file_inside_a_source_resolves_to_its_folder() {
        let directory = TempDir::new();
        installed(&directory);
        let source = MediaSource::detect(&directory.join("FA_1.LIB")).unwrap();
        assert_eq!(source.kind, Kind::Installed);
        assert_eq!(source.path, directory.0);
        // The container itself names the folder it sits in.
        let disc = TempDir::new();
        let path = disc.file("SETUP.ESA", &container(&[("FA.EXE", b"exe")]));
        let source = MediaSource::detect(&path).unwrap();
        assert_eq!(source.kind, Kind::Disc);
        assert_eq!(source.path, disc.0);
    }

    #[test]
    fn a_raw_disc_image_asks_the_player_to_mount_it() {
        let directory = TempDir::new();
        for name in ["fa.iso", "fa.IMG", "fa.bin", "fa.cue"] {
            let path = directory.file(name, b"synthetic image");
            let error = MediaSource::detect(&path).unwrap_err();
            assert!(matches!(error, DetectError::RawImage(_)), "{name}: {error}");
            assert!(error.to_string().contains("Mount the image"), "{error}");
            fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn an_empty_folder_and_a_missing_path_are_named_plainly() {
        let directory = TempDir::new();
        let error = MediaSource::detect(&directory.0).unwrap_err();
        assert!(matches!(error, DetectError::NotASource(_)), "{error}");
        assert!(error.to_string().contains("not a Fighters Anthology"));
        let error = MediaSource::detect(&directory.join("absent")).unwrap_err();
        assert!(matches!(error, DetectError::NotFound(_)), "{error}");
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn an_incomplete_installed_folder_is_not_a_source() {
        let directory = TempDir::new();
        directory.file("FA_1.LIB", &ealib("ONE.PIC", b"one"));
        directory.file("FA.EXE", b"synthetic executable");
        assert!(matches!(
            MediaSource::detect(&directory.0).unwrap_err(),
            DetectError::NotASource(_)
        ));
    }

    #[test]
    fn scanning_a_root_finds_sources_one_level_down() {
        let root = TempDir::new();
        let disc = TempDir(root.dir("disc1"));
        disc.file("SETUP.ESA", &container(&[("FA.EXE", b"exe")]));
        let game = TempDir(root.dir("Janes"));
        installed(&game);
        root.dir("empty");
        let mut seen = HashSet::new();
        let mut found = Vec::new();
        scan_root(
            &root.0,
            Instant::now() + Duration::from_secs(30),
            &mut seen,
            &mut found,
        );
        let kinds: Vec<_> = found
            .iter()
            .map(|source| (&source.path, source.kind))
            .collect();
        assert_eq!(
            kinds,
            // Subdirectories are visited in sorted order, so "Janes" precedes "disc1".
            vec![(&game.0, Kind::Installed), (&disc.0, Kind::Disc)],
            "{found:?}"
        );
        // A second scan of the same root adds nothing, the folders are already seen.
        let mut again = Vec::new();
        scan_root(
            &root.0,
            Instant::now() + Duration::from_secs(30),
            &mut seen,
            &mut again,
        );
        assert!(again.is_empty(), "{again:?}");
    }

    #[test]
    fn a_source_root_is_reported_before_its_children() {
        let root = TempDir::new();
        installed(&root);
        let child = TempDir(root.dir("disc1"));
        child.file("SETUP.ESA", &container(&[("FA.EXE", b"exe")]));
        let mut found = Vec::new();
        scan_root(
            &root.0,
            Instant::now() + Duration::from_secs(30),
            &mut HashSet::new(),
            &mut found,
        );
        assert_eq!(found.len(), 2, "{found:?}");
        assert_eq!(found[0].kind, Kind::Installed);
        assert_eq!(found[1].kind, Kind::Disc);
    }

    #[test]
    fn an_exhausted_budget_stops_the_scan() {
        let root = TempDir::new();
        let child = TempDir(root.dir("disc1"));
        child.file("SETUP.ESA", &container(&[("FA.EXE", b"exe")]));
        let mut found = Vec::new();
        scan_root(&root.0, Instant::now(), &mut HashSet::new(), &mut found);
        assert!(found.is_empty(), "{found:?}");
        assert!(candidates(Duration::from_millis(0)).is_empty());
    }

    #[test]
    fn the_remembered_source_round_trips() {
        let data = TempDir::new();
        assert!(remembered(&data.0).is_none());
        // Absolute on every host: a Unix-style root is relative on Windows.
        let disc = data.join("FA_DISC1");
        let source = MediaSource {
            path: disc.clone(),
            kind: Kind::Disc,
            container: Some(disc.join("SETUP.ESA")),
        };
        remember(&data.0, &source).unwrap();
        assert_eq!(remembered(&data.0), Some((disc, Kind::Disc)));
        let folder = data.join("Fighters Anthology");
        let installed = MediaSource {
            path: folder.clone(),
            kind: Kind::Installed,
            container: None,
        };
        remember(&data.0, &installed).unwrap();
        assert_eq!(remembered(&data.0), Some((folder, Kind::Installed)));
        fs::write(data.join(REMEMBERED), "kind=disc\n").unwrap();
        assert!(remembered(&data.0).is_none());
        fs::write(data.join(REMEMBERED), "path=/a\nkind=floppy\n").unwrap();
        assert!(remembered(&data.0).is_none());
    }

    #[test]
    fn a_relative_source_is_remembered_absolutely() {
        let data = TempDir::new();
        let relative = MediaSource {
            path: PathBuf::from("gameassets/fighters-anthology"),
            kind: Kind::Installed,
            container: None,
        };
        remember(&data.0, &relative).unwrap();
        let (path, kind) = remembered(&data.0).unwrap();
        assert!(path.is_absolute(), "{}", path.display());
        assert!(path.ends_with("gameassets/fighters-anthology"));
        assert_eq!(kind, Kind::Installed);
    }

    #[test]
    fn kinds_carry_a_plain_label() {
        assert_eq!(Kind::Installed.label(), "Installed folder");
        assert_eq!(Kind::Disc.label(), "Disc or mounted image");
    }
}
