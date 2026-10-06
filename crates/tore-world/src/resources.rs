//! What the mission core reads the imported resources through, and the record
//! of what a build read.
//!
//! The import is a map from a resource's name to its bytes. Everything the
//! simulation builds itself from (terrain, aircraft, weapons, sensors, the AI
//! wings, the radio phrases) asks for resources by name through
//! [`ResourceSource`], so a caller can hand over the plain map or a
//! [`ResourceReads`], which notes every name asked for. The sorted names with
//! a hash of each resource's bytes are the mission's [`Manifest`]: two players
//! build the same mission from the same spec, compare manifests and know they
//! simulate the same data, whatever else differs in their imports (see
//! docs/ARCHITECTURE.md, "A mission with no window").
use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
};
use tore_codec::hash::fnv1a64;

/// A read-only name to bytes lookup over the imported resources.
pub trait ResourceSource {
    /// The bytes of one resource, if the import has it.
    fn get(&self, name: &str) -> Option<&Vec<u8>>;

    /// The theater catalog: each base theater's name and every `~` layout
    /// variant, as `tore_formats::theater::map_catalog` lists them. It parses
    /// every theater grid; a build that needs one theater's label asks
    /// [`ResourceSource::theater_label`] instead.
    fn theater_catalog(&self) -> tore_formats::Result<Vec<(String, String)>>;

    /// The label of one theater layout (`UKR`, or a `~` variant such as
    /// `~UKR1`), the same one [`ResourceSource::theater_catalog`] gives it,
    /// reading only the grid of that layout's base theater. `None` when the
    /// import has no such theater.
    fn theater_label(&self, code: &str) -> tore_formats::Result<Option<String>>;
}

/// The label of `code` in `map`, as `tore_formats::theater::map_catalog`
/// words it: a base theater's label is its grid's name, a `~` variant's is
/// its base's name and the variant. Only the base theater's grid is read.
fn label_in(map: &BTreeMap<String, Vec<u8>>, code: &str) -> tore_formats::Result<Option<String>> {
    let base_label = |base: &str| -> tore_formats::Result<Option<String>> {
        map.get(&format!("{base}.T2"))
            .map(|grid| tore_formats::theater::Theater::parse(grid).map(|theater| theater.name))
            .transpose()
    };
    if tore_formats::theater::THEATERS
        .iter()
        .any(|(base, _)| *base == code)
    {
        return base_label(code);
    }
    let layout = format!("{code}.MM");
    if code.starts_with('~')
        && map.contains_key(&layout)
        && let Some(base) = tore_formats::theater::base_theater(&layout)
    {
        return Ok(
            base_label(base)?.map(|label| format!("{label} ({})", code.trim_start_matches('~')))
        );
    }
    Ok(None)
}

impl ResourceSource for BTreeMap<String, Vec<u8>> {
    fn get(&self, name: &str) -> Option<&Vec<u8>> {
        BTreeMap::get(self, name)
    }

    fn theater_catalog(&self) -> tore_formats::Result<Vec<(String, String)>> {
        tore_formats::theater::map_catalog(self)
    }

    fn theater_label(&self, code: &str) -> tore_formats::Result<Option<String>> {
        label_in(self, code)
    }
}

/// A view over the imported resources that notes every name a build asks
/// for, whether or not the import has it.
///
/// Reads are noted through a [`RefCell`], so a view belongs to one thread.
pub struct ResourceReads<'a> {
    map: &'a BTreeMap<String, Vec<u8>>,
    read: RefCell<BTreeSet<String>>,
}

impl<'a> ResourceReads<'a> {
    /// A view with nothing read yet.
    pub fn new(map: &'a BTreeMap<String, Vec<u8>>) -> Self {
        Self {
            map,
            read: RefCell::new(BTreeSet::new()),
        }
    }

    fn note(&self, name: &str) {
        let mut read = self.read.borrow_mut();
        if !read.contains(name) {
            read.insert(name.to_owned());
        }
    }

    /// The names read so far, sorted.
    pub fn names(&self) -> Vec<String> {
        self.read.borrow().iter().cloned().collect()
    }

    /// The content manifest of what has been read so far.
    pub fn manifest(&self) -> Manifest {
        Manifest {
            entries: self
                .read
                .borrow()
                .iter()
                .map(|name| ManifestEntry {
                    name: name.clone(),
                    hash: self.map.get(name).map(|bytes| fnv1a64(bytes)),
                })
                .collect(),
        }
    }
}

impl ResourceSource for ResourceReads<'_> {
    fn get(&self, name: &str) -> Option<&Vec<u8>> {
        self.note(name);
        self.map.get(name)
    }

    fn theater_catalog(&self) -> tore_formats::Result<Vec<(String, String)>> {
        // The catalog reads each base theater's grid and lists the names of
        // the layout variants; it reads no other bytes.
        for (code, _) in tore_formats::theater::THEATERS {
            let grid = format!("{code}.T2");
            if self.map.contains_key(&grid) {
                self.note(&grid);
            }
        }
        tore_formats::theater::map_catalog(self.map)
    }

    fn theater_label(&self, code: &str) -> tore_formats::Result<Option<String>> {
        // A label reads one grid: the base theater's, the only bytes in it.
        let base = tore_formats::theater::base_theater(&format!("{code}.MM")).unwrap_or(code);
        let grid = format!("{base}.T2");
        if self.map.contains_key(&grid) {
            self.note(&grid);
        }
        label_in(self.map, code)
    }
}

/// One resource a build read: its name and, when the import has it, the
/// FNV-1a 64 hash of its bytes. A name the build asked for and did not find
/// has no hash, so an import that lacks a file the host has is a difference
/// too.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManifestEntry {
    pub name: String,
    pub hash: Option<u64>,
}

/// The resources a mission's build read, sorted by name, each with its hash.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Manifest {
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    /// One number for the whole manifest, for a quick comparison. It covers
    /// every name and hash in order.
    pub fn digest(&self) -> u64 {
        let mut hash = tore_codec::hash::Fnv1a64::new();
        for entry in &self.entries {
            hash.update(entry.name.as_bytes());
            hash.update(&[0]);
            match entry.hash {
                Some(value) => {
                    hash.update(&[1]);
                    hash.update(&value.to_le_bytes());
                }
                None => hash.update(&[0]),
            }
        }
        hash.finish()
    }

    /// The names where this manifest and `other` disagree, sorted: present in
    /// one and not the other, or present in both with different bytes.
    pub fn differences(&self, other: &Self) -> Vec<String> {
        let ours: BTreeMap<_, _> = self.entries.iter().map(|e| (&e.name, e.hash)).collect();
        let theirs: BTreeMap<_, _> = other.entries.iter().map(|e| (&e.name, e.hash)).collect();
        let mut names: BTreeSet<&String> = ours.keys().copied().collect();
        names.extend(theirs.keys().copied());
        names
            .into_iter()
            .filter(|name| ours.get(name) != theirs.get(name))
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> BTreeMap<String, Vec<u8>> {
        BTreeMap::from([
            ("A.PT".to_owned(), b"alpha".to_vec()),
            ("B.PT".to_owned(), b"bravo".to_vec()),
            ("C.PT".to_owned(), b"charlie".to_vec()),
        ])
    }

    #[test]
    fn a_view_notes_each_name_asked_for_once_and_sorts_them() {
        let map = map();
        let reads = ResourceReads::new(&map);
        assert!(reads.names().is_empty());
        assert!(reads.get("C.PT").is_some());
        assert!(reads.get("A.PT").is_some());
        assert!(reads.get("C.PT").is_some());
        assert_eq!(reads.names(), ["A.PT", "C.PT"]);
    }

    #[test]
    fn a_name_the_import_lacks_is_noted_without_a_hash() {
        let map = map();
        let reads = ResourceReads::new(&map);
        assert!(reads.get("MISSING.PT").is_none());
        let manifest = reads.manifest();
        assert_eq!(
            manifest.entries,
            [ManifestEntry {
                name: "MISSING.PT".into(),
                hash: None
            }]
        );
    }

    #[test]
    fn the_manifest_hashes_the_bytes_it_read() {
        let map = map();
        let reads = ResourceReads::new(&map);
        reads.get("B.PT");
        let manifest = reads.manifest();
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].hash, Some(fnv1a64(b"bravo")));
    }

    #[test]
    fn manifests_name_the_resources_that_differ() {
        let host = map();
        let mut guest = map();
        guest.insert("B.PT".into(), b"changed".to_vec());
        guest.remove("C.PT");
        let (host_reads, guest_reads) = (ResourceReads::new(&host), ResourceReads::new(&guest));
        for name in ["A.PT", "B.PT", "C.PT"] {
            host_reads.get(name);
            guest_reads.get(name);
        }
        let (a, b) = (host_reads.manifest(), guest_reads.manifest());
        assert_eq!(a.differences(&b), ["B.PT", "C.PT"]);
        assert_ne!(a.digest(), b.digest());
        assert!(a.differences(&a).is_empty());
        assert_eq!(a.digest(), a.digest());
    }

    #[test]
    fn a_resource_the_other_side_never_read_is_a_difference_too() {
        let map = map();
        let (one, two) = (ResourceReads::new(&map), ResourceReads::new(&map));
        one.get("A.PT");
        two.get("A.PT");
        two.get("B.PT");
        assert_eq!(one.manifest().differences(&two.manifest()), ["B.PT"]);
    }
}
