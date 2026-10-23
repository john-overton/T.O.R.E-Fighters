//! Small bounded text files: the player's settings and the remembered media
//! source. A read is capped at 256 KiB and a write goes through a temporary
//! file and a rename, so a crash never leaves half a file.

use std::{
    fs,
    io::{self, Read, Write},
    path::Path,
};

/// Reads a settings file of at most 256 KiB.
pub fn read(path: &Path) -> io::Result<String> {
    let mut text = String::new();
    fs::File::open(path)?
        .take(256 * 1024 + 1)
        .read_to_string(&mut text)?;
    if text.len() > 256 * 1024 {
        return Err(io::Error::other("settings file exceeds 256 KiB"));
    }
    Ok(text)
}
/// Writes a settings file atomically, creating its folder if needed.
pub fn write(path: &Path, text: &str) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".tore-settings-{}-{}.tmp",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));
    // Cleanup is allowed only after successfully creating our own temporary file.
    let mut f = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let result = (|| {
        f.write_all(text.as_bytes())?;
        f.sync_all()?;
        drop(f);
        fs::rename(&temporary, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
