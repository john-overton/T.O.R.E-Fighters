//! The build's version string. Release builds are stamped from the git tag
//! through `TORE_BUILD_VERSION` at compile time; anything else reports the
//! crate version from `Cargo.toml`.

/// The version without a leading `v`, for example `0.1.0` or `0.1.0-3-gabc1234`.
pub fn version() -> &'static str {
    option_env!("TORE_BUILD_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"))
}

/// Source revision stamped by the build script.
pub fn commit() -> &'static str {
    env!("TORE_BUILD_COMMIT")
}

/// Rust target triple stamped by the build script.
pub fn target() -> &'static str {
    env!("TORE_BUILD_TARGET")
}

/// The application name and version printed at startup.
pub fn label() -> String {
    format!("T.O.R.E - v{}", version())
}

/// Profile-aware version output; no installed content is stated explicitly.
pub fn compatibility_label() -> String {
    let code = tore_import::data_directory()
        .map_err(|e| e.to_string())
        .and_then(|dir| tore_session::compatibility::installed(&dir));
    match code {
        Ok(code) => format!("{} (code {code:08X}, commit {})", label(), commit()),
        Err(_) => format!(
            "{} (code {:08X}, no imported content, commit {})",
            label(),
            tore_session::compatibility::current(&Default::default()),
            commit()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_version_is_never_empty_and_the_label_carries_it() {
        assert!(!version().is_empty());
        assert!(!version().starts_with('v'));
        assert_eq!(label(), format!("T.O.R.E - v{}", version()));
    }
}
