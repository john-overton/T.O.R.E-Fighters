//! Pins the face lists of every retail SH shape that the bounded reader
//! already projects, so new opcodes and envelopes cannot change them.
//!
//! The manifest holds one line per shape: the paths that projected when it was
//! recorded and one FNV-1a digest over every face, line and state word those
//! paths produced. It holds digests only, never retail bytes. Shapes that no
//! path could project are absent, so teaching the reader a new shape never
//! fails this test. The test reads the user's own install (the `gameassets`
//! link or `TORE_GAME_DIR`) and skips quietly without it.
//!
//! Refresh, only when a change is meant to alter existing shapes:
//! `TORE_UPDATE_SHAPE_DIGESTS=1 cargo test -p tore-formats --test shape_projection`.
use std::{collections::BTreeMap, path::PathBuf};
use tore_formats::{Archive, shape::Shape};

const MANIFEST: &str = "tests/data/shape-projection-digests.txt";
/// Archives scanned, in a fixed order; disc archives hold no shapes.
const ARCHIVES: [&str; 5] = [
    "FA_1.LIB",
    "FA_2.LIB",
    "FA_4B.LIB",
    "FA_4D.LIB",
    "swpatch.lib",
];
const STATE: u8 = 1;
const SCENERY: u8 = 2;
const EXPORT: u8 = 4;
type Project = fn(&[u8], &BTreeMap<usize, i32>) -> std::io::Result<Shape>;

struct Fnv(u64);
impl Fnv {
    fn bytes(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.0 = (self.0 ^ u64::from(*b)).wrapping_mul(0x100_0000_01b3);
        }
    }
    fn u64(&mut self, v: u64) {
        self.bytes(&v.to_le_bytes());
    }
    fn f32s(&mut self, values: &[f32]) {
        self.u64(values.len() as u64);
        for v in values {
            self.bytes(&v.to_bits().to_le_bytes());
        }
    }
}

fn hash_shape(h: &mut Fnv, shape: &Shape) {
    h.u64(shape.faces.len() as u64);
    for f in &shape.faces {
        h.u64(f.address as u64);
        h.f32s(&f.positions.concat());
        h.bytes(&f.colors);
        h.bytes(&[f.fog as u8, f.subtype, 0xa5]);
        h.f32s(&f.uv.concat());
        h.bytes(f.texture.as_bytes());
        h.f32s(f.normal.as_ref().map_or(&[][..], |n| &n[..]));
    }
    h.u64(shape.lines.len() as u64);
    for l in &shape.lines {
        h.f32s(&l.positions.concat());
        h.bytes(&[l.color, l.fog as u8]);
    }
    h.u64(shape.state_words.len() as u64);
    for w in &shape.state_words {
        h.u64(*w as u64);
    }
}

/// Projects every path in `mask` (or every path that succeeds, when `mask` is
/// `None`) and returns the mask of projected paths with their digest.
fn digest(data: &[u8], mask: Option<u8>) -> Result<(u8, u64), String> {
    let mut h = Fnv(0xcbf2_9ce4_8422_2325);
    let mut done = 0;
    let wanted = |bit| mask.is_none_or(|m| m & bit != 0);
    let empty = BTreeMap::new();
    for (bit, project) in [
        (STATE, Shape::with_state as Project),
        (EXPORT, Shape::with_export_state),
    ] {
        if !wanted(bit) {
            continue;
        }
        match project(data, &empty) {
            Ok(base) => {
                done |= bit;
                h.bytes(&[bit]);
                hash_shape(&mut h, &base);
                // Animated aircraft read single state words at -1 and 1.
                for word in &base.state_words {
                    for value in [-1, 1] {
                        h.u64(*word as u64);
                        h.bytes(&[value as u8]);
                        match project(data, &BTreeMap::from([(*word, value)])) {
                            Ok(pose) => hash_shape(&mut h, &pose),
                            Err(e) => h.bytes(e.to_string().as_bytes()),
                        }
                    }
                }
            }
            Err(e) if mask.is_some() => return Err(format!("path {bit} no longer projects: {e}")),
            Err(_) => {}
        }
    }
    if wanted(SCENERY) {
        match Shape::scenery(data) {
            Ok(shape) => {
                done |= SCENERY;
                h.bytes(&[SCENERY]);
                hash_shape(&mut h, &shape);
            }
            Err(e) if mask.is_some() => {
                return Err(format!("scenery path no longer projects: {e}"));
            }
            Err(_) => {}
        }
    }
    Ok((done, h.0))
}

fn game_dir() -> PathBuf {
    std::env::var_os("TORE_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../gameassets/fighters-anthology")
        })
}

#[test]
fn every_shape_that_projected_keeps_its_face_lists() {
    let root = game_dir();
    let mut shapes = Vec::new();
    for name in ARCHIVES {
        let Ok(archive) = Archive::open(root.join(name)) else {
            eprintln!("skipped: no retail {name}");
            return;
        };
        for entry in archive.entries.keys().filter(|n| n.ends_with(".SH")) {
            shapes.push((format!("{name}/{entry}"), archive.read(entry).unwrap()));
        }
    }
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(MANIFEST);
    if std::env::var_os("TORE_UPDATE_SHAPE_DIGESTS").is_some() {
        let mut text = String::new();
        for (key, data) in &shapes {
            if let Ok((mask, d)) = digest(data, None)
                && mask != 0
            {
                text += &format!("{key} {mask} {d:016x}\n");
            }
        }
        std::fs::write(&manifest, text).unwrap();
        return;
    }
    let recorded: BTreeMap<_, _> = std::fs::read_to_string(&manifest)
        .unwrap()
        .lines()
        .map(|line| {
            let mut it = line.split(' ');
            let key = it.next().unwrap().to_owned();
            let mask: u8 = it.next().unwrap().parse().unwrap();
            let d = u64::from_str_radix(it.next().unwrap(), 16).unwrap();
            (key, (mask, d))
        })
        .collect();
    let mut failures = Vec::new();
    let mut checked = 0;
    for (key, data) in &shapes {
        let Some((mask, want)) = recorded.get(key) else {
            continue;
        };
        checked += 1;
        match digest(data, Some(*mask)) {
            Ok((_, got)) if got == *want => {}
            Ok((_, got)) => {
                failures.push(format!("{key}: digest {got:016x}, recorded {want:016x}"))
            }
            Err(e) => failures.push(format!("{key}: {e}")),
        }
    }
    assert_eq!(
        checked,
        recorded.len(),
        "manifest names shapes the install lacks"
    );
    assert!(
        failures.is_empty(),
        "{} shapes changed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
