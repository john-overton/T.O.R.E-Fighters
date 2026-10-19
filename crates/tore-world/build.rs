//! `TORE_CITY_SOURCE_VERSION`: a hash of this crate's and `tore-codec`'s
//! source, the code half of the city layout cache key
//! (`src/city/cache.rs`). Any edit there starts the cache over. No retail
//! file is read and nothing is embedded.

use std::{fs, path::Path};

fn visit(path: &Path, hash: &mut u64) {
    if path.is_dir() {
        let mut paths: Vec<_> = fs::read_dir(path)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        for p in paths {
            visit(&p, hash);
        }
    } else {
        println!("cargo:rerun-if-changed={}", path.display());
        // Path with forward slashes and text without carriage returns, so a
        // Windows checkout hashes as a Unix one does.
        let name = path.to_string_lossy().replace('\\', "/");
        let bytes: Vec<u8> = fs::read(path)
            .unwrap()
            .into_iter()
            .filter(|&b| b != b'\r')
            .collect();
        for byte in name.bytes().chain(bytes) {
            *hash = (*hash ^ u64::from(byte)).wrapping_mul(0x100000001b3);
        }
    }
}

fn main() {
    let mut hash = 0xcbf29ce484222325;
    for root in ["src", "../tore-codec/src"] {
        println!("cargo:rerun-if-changed={root}");
        visit(Path::new(root), &mut hash);
    }
    println!("cargo:rustc-env=TORE_CITY_SOURCE_VERSION={hash:016x}");
}
