//! The Fighters Anthology import, without the game: find the media, read it,
//! write the import pack and load it back.
//!
//! This crate depends on the standard library and `tore-formats` only, so the
//! dedicated server can import and load the game data without linking the
//! windowing, graphics or audio libraries of the game binary. It knows nothing
//! about what the menus draw; the game checks and decodes that itself from the
//! resources this crate loads (see [`pack::load_with`]).
//!
//! - [`data_directory`]: where the pack, the import report and the player's
//!   files live (`TORE_DATA_DIR` overrides it).
//! - [`media_source`]: what a chosen folder is (installed game or disc) and the
//!   remembered source.
//! - [`import`]: the import itself, with progress and its report file.
//! - [`pack`]: the pack format, loading the newest good pack and pruning older
//!   generations. Behaviour: `docs/spec/import-cache.md`.
//! - [`selection`]: the resource lists the import keeps for the menus, the
//!   debrief and the multiplayer screens.
//! - [`files`]: the two bounded file helpers the settings files also use.
//! - [`set_log`]: where this crate's notes go (it has no logging dependency).

use std::{collections::BTreeMap, path::PathBuf, sync::OnceLock};

pub mod files;
pub mod import;
pub mod media_source;
pub mod pack;
pub mod selection;

pub use import::{Imported, Progress, import_with_progress};
pub use media_source::{DetectError, Kind, MediaSource};
pub use pack::{Loaded, check_markers, check_multiplayer_marker, load, load_with};

/// The same boxed error as the game's `AppResult`, so the two mix freely.
pub type ImportResult<T> = Result<T, Box<dyn std::error::Error>>;

/// Every imported file by its retail name (`F18.PT`, `UKR.MM`) or its
/// synthetic `TORE_*` name, in name order.
pub type Resources = BTreeMap<String, Vec<u8>>;

/// How serious a note from this crate is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Info,
    Warn,
}

static LOG: OnceLock<fn(Level, &str)> = OnceLock::new();

/// Sets where this crate's notes go, once per process; a later call is
/// ignored. The game routes them to its log. Until a sink is set they are
/// dropped.
pub fn set_log(sink: fn(Level, &str)) {
    let _ = LOG.set(sink);
}

pub(crate) fn note(level: Level, text: &str) {
    if let Some(sink) = LOG.get() {
        sink(level, text);
    }
}

/// The folder holding the import pack, the import report, the remembered media
/// source and the player's files. `TORE_DATA_DIR` overrides the platform
/// default.
pub fn data_directory() -> ImportResult<PathBuf> {
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
