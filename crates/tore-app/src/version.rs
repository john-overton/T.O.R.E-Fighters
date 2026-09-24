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
