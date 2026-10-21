//! The on-disk city layout cache (optimization S12): the placement a theater
//! layout's fill computed ([`super::fill::place`]), kept under the profile's
//! `city-cache/` so the next load, or the next dedicated server start, reads
//! it instead of placing hundreds of thousands of buildings again.
//!
//! Disposable local data, never imported game state and never on the wire.
//! An entry is keyed on every input of the placement, so a hit is the
//! placement the code would compute:
//!
//! - the light map (every city tile's grid key, quarter turns, picture name
//!   and classified flags, and the districts), the kit (every piece), the
//!   layout name, the keep-outs (the layout's objects and the redrawn
//!   airports, so the airports setting is in it), the named placements and
//!   the building budget;
//! - [`VERSION`], the generator version, bumped by hand for a deliberate
//!   change, and a hash of the source of this crate and `tore-codec` taken by
//!   the build script, so any code change starts over.
//!
//! The light map and the kit are built from the imported resources on every
//! load (tens of milliseconds) and hashed by content, so a re-import with
//! other pictures or shapes misses on its own. Each file holds a magic, the
//! key and an FNV-1a checksum of its payload; a missing, foreign, truncated,
//! corrupt or inconsistent file is ignored and the placement is computed and
//! written again. Writes go to a temporary file renamed into place, so a
//! reader never sees part of one (the file is not synced: one a crash tears
//! fails its checksum); a failed write leaves the load working.
//! At most [`KEEP_PER_LAYOUT`] entries are kept per theater layout, newest
//! first. `TORE_CITY_CACHE=0` bypasses it.

use super::{
    Instance,
    fill::{City, Keepout},
    kit::Kit,
    mask::LightMap,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::RwLock,
};
use tore_codec::{Fnv1a64, fnv1a64};

static DIRECTORY: RwLock<Option<PathBuf>> = RwLock::new(None);

/// The file's first eight bytes.
pub const MAGIC: &[u8; 8] = b"TORECTY1";
/// The generator version: bump it with any deliberate change to what the
/// placement computes (the source hash catches the rest).
pub const VERSION: u32 = 1;
/// The largest entry read or written: 500,000 buildings take 7.5 MB.
pub const MAX_BYTES: usize = 32 * 1024 * 1024;
/// Entries kept per theater layout (budgets, the airports setting).
pub const KEEP_PER_LAYOUT: usize = 4;
/// The source hash of this crate and `tore-codec` (build script).
pub const SOURCE: &str = env!("TORE_CITY_SOURCE_VERSION");
const HEADER: usize = 24;

/// Keeps the cache under `data_dir`'s `city-cache/` from now on: the app
/// and the dedicated server call it once they know the profile.
pub fn configure(data_dir: &Path) {
    *DIRECTORY.write().unwrap_or_else(|e| e.into_inner()) = Some(data_dir.join("city-cache"));
}

/// The cache directory, `None` before [`configure`] (tests, tools without a
/// profile) or with `TORE_CITY_CACHE=0`.
pub fn directory() -> Option<PathBuf> {
    if std::env::var("TORE_CITY_CACHE").as_deref() == Ok("0") {
        return None;
    }
    DIRECTORY.read().ok()?.clone()
}

/// Everything [`super::fill::place`] reads.
pub struct Inputs<'a> {
    pub map: &'a LightMap,
    pub kit: &'a Kit,
    pub layout: &'a str,
    pub keepouts: &'a [Keepout],
    pub names: &'a [([i64; 2], String)],
    pub cap: usize,
}

/// What [`super::fill::place`] returns.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    pub cities: Vec<City>,
    pub instances: Vec<Instance>,
    pub candidates: usize,
}

fn text(h: &mut Fnv1a64, s: &str) {
    h.update(&(s.len() as u64).to_le_bytes());
    h.update(s.as_bytes());
}

fn ints(h: &mut Fnv1a64, values: &[i64]) {
    for v in values {
        h.update(&v.to_le_bytes());
    }
}

/// The entry key: every input, the generator version and `source`.
pub fn key(inputs: &Inputs<'_>, source: &str) -> u64 {
    let mut h = Fnv1a64::new();
    h.update(MAGIC);
    h.update(&VERSION.to_le_bytes());
    text(&mut h, source);
    text(&mut h, env!("CARGO_PKG_VERSION"));
    text(&mut h, inputs.layout);
    h.update(&(inputs.cap as u64).to_le_bytes());
    h.update(&(inputs.keepouts.len() as u64).to_le_bytes());
    for k in inputs.keepouts {
        ints(
            &mut h,
            &[
                k.center[0],
                k.center[1],
                k.half[0],
                k.half[1],
                i64::from(k.heading),
                k.clearance,
            ],
        );
    }
    h.update(&(inputs.names.len() as u64).to_le_bytes());
    for (at, name) in inputs.names {
        ints(&mut h, at);
        text(&mut h, name);
    }
    // The map: each distinct mask once, in first-use order.
    let mut masks = BTreeMap::<*const super::mask::TileMask, u64>::new();
    h.update(&(inputs.map.tiles.len() as u64).to_le_bytes());
    for (key, tile) in &inputs.map.tiles {
        h.update(&key.0.to_le_bytes());
        h.update(&key.1.to_le_bytes());
        h.update(&[tile.rotation]);
        text(&mut h, &tile.texture);
        let next = masks.len() as u64;
        match masks.entry(std::sync::Arc::as_ptr(&tile.mask)) {
            std::collections::btree_map::Entry::Occupied(seen) => {
                h.update(&[0]);
                h.update(&seen.get().to_le_bytes());
            }
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(next);
                h.update(&[1]);
                h.update(&(tile.mask.flags.len() as u64).to_le_bytes());
                h.update(&tile.mask.flags);
            }
        }
    }
    h.update(&(inputs.map.districts.len() as u64).to_le_bytes());
    for d in &inputs.map.districts {
        ints(
            &mut h,
            &[
                d.center[0],
                d.center[1],
                d.half[0],
                d.half[1],
                i64::from(d.heading),
            ],
        );
    }
    h.update(&(inputs.kit.pieces.len() as u64).to_le_bytes());
    for p in &inputs.kit.pieces {
        text(&mut h, &p.source);
        text(&mut h, &p.shape);
        h.update(&(p.faces.len() as u64).to_le_bytes());
        for f in &p.faces {
            h.update(&f.to_le_bytes());
        }
        for v in p.origin {
            h.update(&v.to_bits().to_le_bytes());
        }
        h.update(&p.scale.to_bits().to_le_bytes());
        ints(&mut h, &[p.half[0], p.half[1], p.height]);
        h.update(&[p.class as u8, u8::from(p.cut)]);
    }
    h.finish()
}

/// One cache file.
#[derive(Clone, Debug)]
pub struct Entry {
    pub path: PathBuf,
    pub key: u64,
}

/// A layout name as a file name stem: its letters and digits.
fn stem(layout: &str) -> String {
    layout
        .trim_end_matches(".MM")
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

impl Entry {
    pub fn new(directory: &Path, layout: &str, key: u64) -> Self {
        Self {
            path: directory.join(format!("{}-{key:016x}.city", stem(layout))),
            key,
        }
    }

    /// The cached placement, `None` on any miss: no file, too big, another
    /// key, a bad checksum, or contents that do not fit `kit_len` pieces and
    /// the `cap`.
    pub fn read(&self, kit_len: usize, cap: usize) -> Option<Placed> {
        use std::io::Read;
        let file = std::fs::File::open(&self.path).ok()?;
        if file.metadata().ok()?.len() > MAX_BYTES as u64 {
            return None;
        }
        let mut bytes = Vec::new();
        file.take(MAX_BYTES as u64 + 1)
            .read_to_end(&mut bytes)
            .ok()?;
        if bytes.len() < HEADER || bytes.len() > MAX_BYTES || &bytes[..8] != MAGIC {
            return None;
        }
        if u64::from_le_bytes(bytes[8..16].try_into().ok()?) != self.key
            || u64::from_le_bytes(bytes[16..24].try_into().ok()?) != fnv1a64(&bytes[HEADER..])
        {
            return None;
        }
        decode(&bytes[HEADER..], kit_len, cap)
    }

    /// Writes `placed` atomically (a temporary file renamed into place) and
    /// keeps the newest [`KEEP_PER_LAYOUT`] entries of this layout. Errors
    /// are reported under `TORE_LOADING_PROFILE` and otherwise ignored.
    pub fn write(&self, placed: &Placed) {
        if let Err(error) = self.try_write(placed)
            && std::env::var_os("TORE_LOADING_PROFILE").is_some()
        {
            eprintln!("city layout cache write: {error}");
        }
    }

    fn try_write(&self, placed: &Placed) -> std::io::Result<()> {
        use std::io::Write;
        static SERIAL: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let payload = encode(placed);
        if payload.len() + HEADER > MAX_BYTES {
            return Ok(());
        }
        let directory = self.path.parent().unwrap_or(Path::new("."));
        std::fs::create_dir_all(directory)?;
        let serial = SERIAL.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temp = self
            .path
            .with_extension(format!("{}-{serial}.tmp", std::process::id()));
        let result = (|| {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)?;
            let mut header = Vec::with_capacity(HEADER);
            header.extend_from_slice(MAGIC);
            header.extend_from_slice(&self.key.to_le_bytes());
            header.extend_from_slice(&fnv1a64(&payload).to_le_bytes());
            file.write_all(&header)?;
            file.write_all(&payload)?;
            drop(file);
            // Rename never shows a reader a partly written entry.
            std::fs::rename(&temp, &self.path)
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }
        result?;
        self.prune(directory);
        Ok(())
    }

    /// Removes this layout's entries past the newest [`KEEP_PER_LAYOUT`]
    /// (this one always stays).
    fn prune(&self, directory: &Path) {
        let Some(name) = self.path.file_name().and_then(|n| n.to_str()) else {
            return;
        };
        let Some((prefix, _)) = name.rsplit_once('-') else {
            return;
        };
        let prefix = format!("{prefix}-");
        let mut others: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(directory)
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_string_lossy();
                n.starts_with(&prefix)
                    && n.ends_with(".city")
                    && n.len() == prefix.len() + 16 + 5
                    && *n != *name
            })
            .map(|e| {
                let modified = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                (modified, e.path())
            })
            .collect();
        others.sort();
        let excess = (others.len() + 1).saturating_sub(KEEP_PER_LAYOUT);
        for (_, path) in others.into_iter().take(excess) {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn put(out: &mut Vec<u8>, n: u64) {
    out.extend_from_slice(&n.to_le_bytes());
}

fn put_text(out: &mut Vec<u8>, s: &str) {
    put(out, s.len() as u64);
    out.extend_from_slice(s.as_bytes());
}

/// The payload: candidates, the cities, then 15 bytes a building.
pub fn encode(placed: &Placed) -> Vec<u8> {
    let mut out = Vec::with_capacity(32 + placed.instances.len() * 15);
    put(&mut out, placed.candidates as u64);
    put(&mut out, placed.cities.len() as u64);
    for c in &placed.cities {
        put_text(&mut out, &c.name);
        put(&mut out, c.tiles.len() as u64);
        for t in &c.tiles {
            out.extend_from_slice(&t.0.to_le_bytes());
            out.extend_from_slice(&t.1.to_le_bytes());
        }
        for v in [c.center[0], c.center[1], c.radius] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        for v in [c.candidates, c.placed, c.core] {
            put(&mut out, v as u64);
        }
    }
    put(&mut out, placed.instances.len() as u64);
    for i in &placed.instances {
        out.extend_from_slice(&i.piece.to_le_bytes());
        out.extend_from_slice(&i.x.to_le_bytes());
        out.extend_from_slice(&i.z.to_le_bytes());
        out.extend_from_slice(&i.heading.to_le_bytes());
        out.push(u8::from(i.core));
        out.extend_from_slice(&i.city.to_le_bytes());
    }
    out
}

struct Reader<'a>(&'a [u8]);

impl<'a> Reader<'a> {
    fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }
    fn count(&mut self, limit: u64) -> Option<usize> {
        let n = u64::from_le_bytes(self.take()?);
        (n <= limit).then_some(n as usize)
    }
    fn i64(&mut self) -> Option<i64> {
        Some(i64::from_le_bytes(self.take()?))
    }
    fn text(&mut self, limit: u64) -> Option<String> {
        let n = self.count(limit)?;
        let bytes = self.0.get(..n)?;
        self.0 = &self.0[n..];
        String::from_utf8(bytes.to_vec()).ok()
    }
}

/// The payload back, bounded by its own length, `kit_len` and `cap`, and
/// checked for consistency: every building's piece, city and heading in
/// range, and every city's placed and core counts its buildings'.
pub fn decode(bytes: &[u8], kit_len: usize, cap: usize) -> Option<Placed> {
    let mut r = Reader(bytes);
    let candidates = r.count(u64::from(u32::MAX))?;
    let city_count = r.count(u64::from(u16::MAX) + 1)?;
    let mut cities = Vec::with_capacity(city_count.min(bytes.len() / 64 + 1));
    for _ in 0..city_count {
        let name = r.text(4096)?;
        let tile_count = r.count(r.0.len() as u64 / 8)?;
        let mut tiles = Vec::with_capacity(tile_count);
        for _ in 0..tile_count {
            tiles.push((i32::from_le_bytes(r.take()?), i32::from_le_bytes(r.take()?)));
        }
        let center = [r.i64()?, r.i64()?];
        let radius = r.i64()?;
        let counts = [
            r.count(u64::from(u32::MAX))?,
            r.count(u64::from(u32::MAX))?,
            r.count(u64::from(u32::MAX))?,
        ];
        cities.push(City {
            name,
            tiles,
            center,
            radius,
            candidates: counts[0],
            placed: counts[1],
            core: counts[2],
        });
    }
    let n = r.count(cap.min(MAX_BYTES / 15) as u64)?;
    if r.0.len() != n * 15 {
        return None;
    }
    let mut placed = vec![[0usize; 2]; cities.len()];
    let mut instances = Vec::with_capacity(n);
    for _ in 0..n {
        let i = Instance {
            piece: u16::from_le_bytes(r.take()?),
            x: i32::from_le_bytes(r.take()?),
            z: i32::from_le_bytes(r.take()?),
            heading: i16::from_le_bytes(r.take()?),
            core: match r.take::<1>()?[0] {
                0 => false,
                1 => true,
                _ => return None,
            },
            city: u16::from_le_bytes(r.take()?),
        };
        if usize::from(i.piece) >= kit_len
            || !(0..360).contains(&i.heading)
            || usize::from(i.city) >= cities.len()
        {
            return None;
        }
        placed[usize::from(i.city)][0] += 1;
        placed[usize::from(i.city)][1] += usize::from(i.core);
        instances.push(i);
    }
    let consistent = cities
        .iter()
        .zip(&placed)
        .all(|(c, p)| c.placed == p[0] && c.core == p[1] && c.placed <= c.candidates)
        && cities.iter().map(|c| c.candidates).sum::<usize>() == candidates;
    consistent.then_some(Placed {
        cities,
        instances,
        candidates,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::city::{
        kit::{Class, Piece},
        mask::{District, PlacedTile, TileMask},
    };
    use std::sync::Arc;

    fn piece(half: [i64; 2], class: Class) -> Piece {
        Piece {
            source: "T.OT".into(),
            shape: "T.SH".into(),
            faces: vec![0, 1],
            origin: [0.5, 0., 0.],
            scale: 1.,
            half,
            height: 50,
            class,
            cut: false,
        }
    }

    fn map() -> LightMap {
        let mut map = LightMap::default();
        map.tiles.insert(
            (0, 0),
            PlacedTile {
                texture: "T.PIC".into(),
                rotation: 1,
                mask: Arc::new(TileMask {
                    flags: vec![1; 256 * 256],
                }),
            },
        );
        map
    }

    fn placed() -> Placed {
        Placed {
            cities: vec![City {
                name: "Kyiv".into(),
                tiles: vec![(0, 0), (4, 0)],
                center: [100, -200],
                radius: 9_000,
                candidates: 5,
                placed: 2,
                core: 1,
            }],
            instances: vec![
                Instance {
                    piece: 1,
                    x: 1_000,
                    z: -2_000,
                    heading: 270,
                    core: true,
                    city: 0,
                },
                Instance {
                    piece: 0,
                    x: -7,
                    z: 33,
                    heading: 0,
                    core: false,
                    city: 0,
                },
            ],
            candidates: 5,
        }
    }

    fn root(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tore-city-cache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn a_placement_round_trips_through_the_file() {
        let dir = root("round");
        let entry = Entry::new(&dir, "UKR.MM", 0x1234);
        assert!(entry.read(2, 10).is_none(), "nothing cached yet");
        entry.write(&placed());
        assert_eq!(entry.read(2, 10), Some(placed()));
        // Another key never reads this file's contents.
        let other = Entry {
            path: entry.path.clone(),
            key: 0x1235,
        };
        assert!(other.read(2, 10).is_none());
        // No temporary file is left behind.
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, ["UKR-0000000000001234.city"]);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn a_damaged_or_foreign_file_is_a_miss() {
        let dir = root("damage");
        let entry = Entry::new(&dir, "UKR.MM", 7);
        entry.write(&placed());
        let good = std::fs::read(&entry.path).unwrap();
        // Every truncation, and a flipped bit anywhere, is a miss.
        for len in 0..good.len() {
            std::fs::write(&entry.path, &good[..len]).unwrap();
            assert!(entry.read(2, 10).is_none(), "truncated to {len}");
        }
        for at in 0..good.len() {
            let mut bad = good.clone();
            bad[at] ^= 0x10;
            std::fs::write(&entry.path, &bad).unwrap();
            assert!(entry.read(2, 10).is_none(), "bit flipped at {at}");
        }
        // Trailing bytes too.
        let mut long = good.clone();
        long.push(0);
        std::fs::write(&entry.path, &long).unwrap();
        assert!(entry.read(2, 10).is_none());
        // A good file that does not fit this kit or budget.
        std::fs::write(&entry.path, &good).unwrap();
        assert!(entry.read(1, 10).is_none(), "piece 1 past a one-piece kit");
        assert!(
            entry.read(2, 1).is_none(),
            "two buildings past a budget of one"
        );
        assert_eq!(entry.read(2, 2), Some(placed()));
        // A directory where the file should be is a miss and a failed write.
        std::fs::remove_file(&entry.path).unwrap();
        std::fs::create_dir(&entry.path).unwrap();
        assert!(entry.read(2, 10).is_none());
        entry.write(&placed());
        assert!(entry.read(2, 10).is_none());
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn decoding_checks_counts_and_ranges() {
        let good = encode(&placed());
        assert_eq!(decode(&good, 2, 10), Some(placed()));
        let with = |f: &dyn Fn(&mut Placed)| {
            let mut p = placed();
            f(&mut p);
            decode(&encode(&p), 2, 10)
        };
        assert!(with(&|p| p.instances[0].heading = 360).is_none());
        assert!(with(&|p| p.instances[0].heading = -1).is_none());
        assert!(with(&|p| p.instances[0].city = 1).is_none());
        assert!(with(&|p| p.cities[0].placed = 3).is_none());
        assert!(with(&|p| p.cities[0].core = 0).is_none());
        assert!(with(&|p| p.candidates = 6).is_none());
        // A core flag other than 0 or 1.
        let mut bad = good.clone();
        let core_at = bad.len() - 15 + 12;
        bad[core_at] = 2;
        assert!(decode(&bad, 2, 10).is_none());
    }

    #[test]
    fn writing_is_atomic_and_keeps_the_newest_entries() {
        let dir = root("prune");
        // A stale temporary file from a crashed writer is not an entry.
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("UKR-0000000000000001.99-0.tmp"), b"partial").unwrap();
        let other = Entry::new(&dir, "EGY.MM", 1);
        other.write(&placed());
        for key in 1..=6u64 {
            Entry::new(&dir, "UKR.MM", key).write(&placed());
            // Distinct modification times for the pruning order.
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        let mut names: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| n.ends_with(".city"))
            .collect();
        names.sort();
        assert_eq!(
            names,
            [
                "EGY-0000000000000001.city",
                "UKR-0000000000000003.city",
                "UKR-0000000000000004.city",
                "UKR-0000000000000005.city",
                "UKR-0000000000000006.city",
            ]
        );
        assert!(dir.join("UKR-0000000000000001.99-0.tmp").exists());
        // Every kept entry reads whole.
        for key in 3..=6 {
            assert_eq!(Entry::new(&dir, "UKR.MM", key).read(2, 10), Some(placed()));
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn every_input_moves_the_key() {
        let (map, kit) = (
            map(),
            Kit {
                pieces: vec![piece([20, 30], Class::House), piece([40, 40], Class::Tower)],
            },
        );
        let keepouts = [Keepout {
            center: [10, 20],
            half: [30, 40],
            heading: 15,
            clearance: 40,
        }];
        let names = [([5, 6], "Kyiv".to_owned())];
        fn inputs<'a>(
            map: &'a LightMap,
            kit: &'a Kit,
            keepouts: &'a [Keepout],
            names: &'a [([i64; 2], String)],
        ) -> Inputs<'a> {
            Inputs {
                map,
                kit,
                layout: "UKR.MM",
                keepouts,
                names,
                cap: 500_000,
            }
        }
        let base = key(&inputs(&map, &kit, &keepouts, &names), "src-1");
        assert_eq!(
            base,
            key(&inputs(&map, &kit, &keepouts, &names), "src-1"),
            "stable"
        );
        let mut changed = Vec::new();
        changed.push(key(&inputs(&map, &kit, &keepouts, &names), "src-2"));
        changed.push(key(
            &Inputs {
                layout: "EGY.MM",
                ..inputs(&map, &kit, &keepouts, &names)
            },
            "src-1",
        ));
        changed.push(key(
            &Inputs {
                cap: 1_000,
                ..inputs(&map, &kit, &keepouts, &names)
            },
            "src-1",
        ));
        let moved = [Keepout {
            heading: 16,
            ..keepouts[0]
        }];
        changed.push(key(
            &Inputs {
                keepouts: &moved,
                ..inputs(&map, &kit, &keepouts, &names)
            },
            "src-1",
        ));
        changed.push(key(
            &Inputs {
                keepouts: &[],
                ..inputs(&map, &kit, &keepouts, &names)
            },
            "src-1",
        ));
        let renamed = [([5, 6], "Kiev".to_owned())];
        changed.push(key(
            &Inputs {
                names: &renamed,
                ..inputs(&map, &kit, &keepouts, &names)
            },
            "src-1",
        ));
        // One texel's flags, a tile's turn, its picture name, a district.
        let mut m = map.clone();
        let tile = m.tiles.get_mut(&(0, 0)).unwrap();
        let mut flags = tile.mask.flags.clone();
        flags[1234] = 3;
        tile.mask = Arc::new(TileMask { flags });
        changed.push(key(&inputs(&m, &kit, &keepouts, &names), "src-1"));
        let mut m = map.clone();
        m.tiles.get_mut(&(0, 0)).unwrap().rotation = 2;
        changed.push(key(&inputs(&m, &kit, &keepouts, &names), "src-1"));
        let mut m = map.clone();
        m.tiles.get_mut(&(0, 0)).unwrap().texture = "U.PIC".into();
        changed.push(key(&inputs(&m, &kit, &keepouts, &names), "src-1"));
        let mut m = map.clone();
        let tile = m.tiles[&(0, 0)].clone();
        m.tiles.insert((4, 0), tile);
        changed.push(key(&inputs(&m, &kit, &keepouts, &names), "src-1"));
        let mut m = map.clone();
        m.districts.push(District {
            center: [0, 0],
            half: [10, 10],
            heading: 0,
        });
        changed.push(key(&inputs(&m, &kit, &keepouts, &names), "src-1"));
        // Any piece field.
        for f in [
            &(|p: &mut Piece| p.half[0] += 1) as &dyn Fn(&mut Piece),
            &|p| p.height += 1,
            &|p| p.class = Class::LowRise,
            &|p| p.source = "U.OT".into(),
            &|p| p.faces.push(9),
            &|p| p.scale = 2.,
            &|p| p.origin[2] = 1.,
            &|p| p.cut = true,
        ] {
            let mut k = kit.clone();
            f(&mut k.pieces[0]);
            changed.push(key(&inputs(&map, &k, &keepouts, &names), "src-1"));
        }
        let mut k = kit.clone();
        k.pieces.swap(0, 1);
        changed.push(key(&inputs(&map, &k, &keepouts, &names), "src-1"));
        for (n, c) in changed.iter().enumerate() {
            assert_ne!(*c, base, "input {n} did not move the key");
        }
        let distinct: std::collections::BTreeSet<_> = changed.iter().collect();
        assert_eq!(distinct.len(), changed.len());
    }
}
