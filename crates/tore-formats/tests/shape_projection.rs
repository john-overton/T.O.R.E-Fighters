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
    // Sprites joined the projection after the manifest was first recorded;
    // hashing them only when present keeps those digests valid while still
    // catching a recorded shape that gains one.
    if !shape.billboards.is_empty() {
        h.u64(shape.billboards.len() as u64);
        for b in &shape.billboards {
            h.u64(b.address as u64);
            h.f32s(&b.center);
            h.f32s(&b.size);
            h.f32s(b.uv.as_ref().map_or(&[][..], |uv| uv.as_flattened()));
            h.bytes(b.texture.as_bytes());
            h.bytes(&[b.fog as u8]);
        }
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

/// The carriers draw their full models on every path, the islands their
/// damage branch, and the deck crew its sprite frame (slice S2). Counts are
/// from the user's own FA_2.LIB; skips without it.
#[test]
fn carriers_islands_and_deck_crew_draw_their_full_models() {
    use tore_formats::shape::{DAMAGED_WORD, SPRITE_FRAME_WORD};
    let Ok(lib) = Archive::open(game_dir().join("FA_2.LIB")) else {
        eprintln!("skipped: no retail FA_2.LIB");
        return;
    };
    let read = |name: &str| lib.read(name).unwrap();
    let damaged = BTreeMap::from([(DAMAGED_WORD, 1)]);
    for (name, faces, damaged_faces) in [
        ("NIMZ.SH", 98, None),
        ("NIMZ_A.SH", 98, None),
        ("KITT.SH", 232, None),
        ("KITT_A.SH", 233, None),
        ("CLEM.SH", 96, None),
        ("CLEM_A.SH", 98, None),
        ("WASP.SH", 139, None),
        ("WASP_A.SH", 140, None),
        ("NIMZT.SH", 48, Some(48)),
        ("KITTT.SH", 77, Some(64)),
        ("CLEMT.SH", 66, Some(66)),
        ("WASPT.SH", 85, Some(83)),
    ] {
        let data = read(name);
        let state = Shape::with_state(&data, &BTreeMap::new()).unwrap();
        assert_eq!(state.faces.len(), faces, "{name} state path");
        assert_eq!(Shape::scenery(&data).unwrap().faces.len(), faces, "{name}");
        let broken = Shape::with_state(&data, &damaged).unwrap();
        assert_eq!(broken.faces.len(), damaged_faces.unwrap_or(faces), "{name}");
        if damaged_faces.is_some() {
            assert!(
                broken
                    .faces
                    .iter()
                    .all(|f| f.texture.contains("_A") || f.texture.ends_with("D.PIC"))
            );
        }
    }
    let crew = read("CATGUY.SH");
    let scenery = Shape::scenery(&crew).unwrap();
    let [sprite] = scenery.billboards.as_slice() else {
        panic!("one deck crew sprite");
    };
    assert_eq!(sprite.size, [8., 12.]);
    assert_eq!(sprite.texture, "CATF.PIC");
    assert_eq!(
        sprite.uv,
        Some([[1., 411.], [1., 469.], [52., 469.], [52., 411.]])
    );
    let wide = BTreeMap::from([(SPRITE_FRAME_WORD, 9 << 16 | 2)]);
    let posed = Shape::with_state(&crew, &wide).unwrap();
    assert!(posed.state_words.contains(&SPRITE_FRAME_WORD));
    assert_eq!(posed.billboards[0].size, [12., 12.]);
    assert_eq!(
        posed.billboards[0].uv,
        Some([[478., 253.], [478., 311.], [559., 311.], [559., 253.]])
    );
    let outside = BTreeMap::from([(SPRITE_FRAME_WORD, 11 << 16)]);
    assert!(Shape::with_state(&crew, &outside).is_err());
}
