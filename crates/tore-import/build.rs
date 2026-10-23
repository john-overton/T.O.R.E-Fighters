//! Stamps the commit this import code was built from, the same way
//! `crates/tore-app/build.rs` does (agent decision: a copy of its
//! `stamp_build_identity`, since the import writes the importer's identity into
//! the pack and this crate has no build script of its own to share it with).

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=TORE_BUILD_COMMIT");
    for path in [
        "../../.git/HEAD",
        "../../.git/refs",
        "../../.git/packed-refs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let commit = env::var("TORE_BUILD_COMMIT")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            std::process::Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .and_then(|output| String::from_utf8(output.stdout).ok())
        })
        .unwrap_or_else(|| "unknown".into());
    // Cargo directive values must stay on one line.
    let commit: String = commit
        .trim()
        .chars()
        .filter(|c| !c.is_control())
        .take(128)
        .collect();
    println!("cargo:rustc-env=TORE_BUILD_COMMIT={commit}");
}
