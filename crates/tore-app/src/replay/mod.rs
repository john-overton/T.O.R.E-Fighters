//! Mission recordings in the app: converting live state into the
//! `tore_replay` model and back, recording every flight, the Replays screen
//! that lists them, and the viewer that plays a recording from any
//! viewpoint. Opinionated addition requested
//! by John on 2026-09-26; see docs/REPLAYS.md. The format, reader and exports
//! live in the dependency-free `tore-replay` crate; this module is the only
//! place the app's own types meet it.
pub mod cli;
pub mod clock;
pub mod convert;
#[cfg(test)]
mod demo;
pub mod drone;
#[cfg(test)]
pub(crate) mod fixture;
pub mod host;
pub mod library;
pub mod overlay;
pub mod playback;
pub mod png;
pub mod recorder;
pub mod screen;
pub mod sound;
pub mod tracks;
pub mod trails;
pub mod viewer;
pub mod weather;

#[cfg(test)]
pub(crate) mod tests {
    use std::path::{Path, PathBuf};

    /// A folder under the system temporary directory, removed on drop.
    pub(crate) struct TempDir(PathBuf);
    impl TempDir {
        pub(crate) fn new(name: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("tore-{name}-{}-{nanos}", std::process::id()));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        pub(crate) fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}
