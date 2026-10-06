//! A game's content: the aircraft, theaters and weapons it can load, and the
//! shared data every mission reads, each with a hash of exactly the files that
//! item reads.
//!
//! Two games whose item digests are equal simulate that item from the same
//! bytes. The host compares players' content to find what a mission cannot use
//! (docs/ARCHITECTURE.md, "Compatibility"; stage L of the multiplayer plan).
//!
//! An item's names are what the mission core really reads, not a list kept by
//! hand: each probe runs the item's own loader through a
//! [`ResourceReads`](crate::resources::ResourceReads). The coverage rule keeps
//! the probes honest: for any mission, its manifest lies inside the shared
//! item and its own items (its aircraft, its theater, its loadouts' weapons),
//! and the shared item lies inside its manifest. A future read that belongs to
//! no item, or a shared read that only some missions make, fails the tests in
//! `content_tests.rs`, and its author moves it into the right probe.
//!
//! The digest of an item is [`Manifest::digest`] of its resources, so a name
//! the import lacks is a difference too.
use crate::{
    aircraft_type::AircraftType,
    mission::{self, Condition, MissionSpec, Skill, Start},
    resources::{Manifest, ManifestEntry, ResourceReads},
    terrain::{Overrides, Terrain},
    world::{Seating, World},
};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{aircraft::AircraftId, weapons::Weapon};
use tore_sim::combat::loadout::Loadout;

/// The key of the shared item.
pub const SHARED_KEY: &str = "shared";

/// What kind of thing an item is. The order is the order of
/// [`Content::items`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Kind {
    /// An aircraft the creator offers.
    Aircraft,
    /// One of the sixteen base theaters.
    Theater,
    /// A weapon record the Load Ordnance page offers.
    Weapon,
    /// What every mission reads beyond its own items.
    Shared,
}

impl Kind {
    /// The word for the kind in logs and `tore-server --check`.
    pub fn name(self) -> &'static str {
        match self {
            Self::Aircraft => "aircraft",
            Self::Theater => "theater",
            Self::Weapon => "weapon",
            Self::Shared => "shared",
        }
    }
}

/// One item of a game's content: a kind, a key, the resources it reads with a
/// hash of each, and one digest of them all.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Item {
    pub kind: Kind,
    /// The aircraft's selection key (`F18.PT`, `faxx`), the theater's code
    /// (`UKR`), the weapon's record (`AIM9X.JT`), or [`SHARED_KEY`].
    pub key: String,
    /// [`Manifest::digest`] of the resources the item reads. Only this
    /// travels between games.
    pub digest: u64,
    manifest: Manifest,
}

impl Item {
    /// An item whose digest is the digest of `manifest`.
    pub fn new(kind: Kind, key: impl Into<String>, manifest: Manifest) -> Self {
        Self {
            kind,
            key: key.into(),
            digest: manifest.digest(),
            manifest,
        }
    }

    /// The resources the item reads, sorted, with their hashes.
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The names of the resources the item reads, sorted.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.manifest.entries.iter().map(|e| e.name.as_str())
    }

    /// Whether the item reads the resource `name`.
    pub fn reads(&self, name: &str) -> bool {
        self.manifest
            .entries
            .binary_search_by(|entry| entry.name.as_str().cmp(name))
            .is_ok()
    }
}

/// Everything a game can load: its items, sorted by kind then key.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Content {
    items: Vec<Item>,
}

impl Content {
    /// The content of an import: one item for every selectable aircraft and
    /// every base theater whose loaders succeed on it, every weapon record the
    /// Load Ordnance page would offer, and the shared item. An item the import
    /// cannot load (a file missing or unreadable) is left out: the game does
    /// not have it. When no aircraft or theater loads there is no mission to
    /// probe, so there is no shared item either.
    pub fn of(resources: &BTreeMap<String, Vec<u8>>) -> Self {
        let mut items = Vec::new();
        for id in AircraftId::SELECTABLE {
            if let Some(item) = aircraft_item(resources, id) {
                items.push(item);
            }
        }
        for code in mission::THEATERS {
            if let Some(item) = theater_item(resources, code) {
                items.push(item);
            }
        }
        items.extend(weapon_items(resources));
        let reference = (
            items.iter().find(|item| item.kind == Kind::Aircraft),
            items.iter().find(|item| item.kind == Kind::Theater),
        );
        if let (Some(aircraft), Some(theater)) = reference
            && let Some(shared) = shared_item(resources, aircraft, theater)
        {
            items.push(shared);
        }
        items.sort_by(|a, b| (a.kind, &a.key).cmp(&(b.kind, &b.key)));
        Self { items }
    }

    /// Builds content from items, for tests and for a caller that has decoded
    /// the digests of another game (those items carry no names).
    pub fn from_items(mut items: Vec<Item>) -> Self {
        items.sort_by(|a, b| (a.kind, &a.key).cmp(&(b.kind, &b.key)));
        Self { items }
    }

    /// Every item, sorted by kind then key.
    pub fn items(&self) -> &[Item] {
        &self.items
    }

    /// The item of this kind and key, if the game has it.
    pub fn get(&self, kind: Kind, key: &str) -> Option<&Item> {
        self.items
            .binary_search_by(|item| (item.kind, item.key.as_str()).cmp(&(kind, key)))
            .ok()
            .map(|index| &self.items[index])
    }

    /// The shared item, if the import could build the reference mission.
    pub fn shared(&self) -> Option<&Item> {
        self.get(Kind::Shared, SHARED_KEY)
    }

    /// The items of one kind, sorted by key.
    pub fn of_kind(&self, kind: Kind) -> impl Iterator<Item = &Item> {
        self.items.iter().filter(move |item| item.kind == kind)
    }

    /// The items that read the resource `name`, to attribute a differing file
    /// to them. The shared item is last; a name that is in no item belongs to
    /// nothing here.
    pub fn holding(&self, name: &str) -> Vec<&Item> {
        self.items.iter().filter(|item| item.reads(name)).collect()
    }
}

/// An aircraft item: what `AircraftType::load` and the standard loadout's
/// combat configuration read, the profile, its sensors and its default
/// stores' weapons and equipment. `None` when the import cannot load it.
fn aircraft_item(resources: &BTreeMap<String, Vec<u8>>, id: AircraftId) -> Option<Item> {
    let reads = ResourceReads::new(resources);
    probe_aircraft(&reads, id)?;
    Some(Item::new(
        Kind::Aircraft,
        id.selection_key(),
        reads.manifest(),
    ))
}

/// Loads the aircraft the way a mission build does, through `reads`.
fn probe_aircraft(reads: &ResourceReads<'_>, id: AircraftId) -> Option<()> {
    use crate::resources::ResourceSource;
    let kind = AircraftType::load(reads, id).ok()?;
    Loadout::new(&kind.profile, |name| {
        reads
            .get(name)
            .cloned()
            .ok_or_else(|| std::io::Error::other(format!("missing {name}")))
    })
    .ok()?;
    Some(())
}

/// A theater item: what `Terrain::for_mission` reads for the theater under
/// each of the six weather conditions, its layout, its own grid, the
/// condition layers, the placed objects and their shapes. `None` when the
/// import cannot build the theater under all six.
fn theater_item(resources: &BTreeMap<String, Vec<u8>>, code: &str) -> Option<Item> {
    let reads = ResourceReads::new(resources);
    for condition in Condition::ALL {
        Terrain::for_mission(&reads, code, Some(condition.index()), &Overrides::default()).ok()?;
    }
    Some(Item::new(Kind::Theater, code, reads.manifest()))
}

/// A weapon item for every record the Load Ordnance page would offer: a `.JT`
/// that parses, other than the `~` ones. The record is the only file a
/// loadout reads for it.
fn weapon_items(resources: &BTreeMap<String, Vec<u8>>) -> Vec<Item> {
    resources
        .iter()
        .filter(|(name, _)| name.ends_with(".JT") && !name.starts_with('~'))
        .filter(|(name, bytes)| Weapon::parse(name, bytes).is_ok())
        .map(|(name, bytes)| {
            let manifest = Manifest {
                entries: vec![ManifestEntry {
                    name: name.clone(),
                    hash: Some(tore_codec::hash::fnv1a64(bytes)),
                }],
            };
            Item::new(Kind::Weapon, name.clone(), manifest)
        })
        .collect()
}

/// The reference mission: the first aircraft and theater the import has, a
/// flight of one on each side at the creator's highest altitude, built the way
/// a host builds an open mission.
fn reference_spec(aircraft: AircraftId, theater: &str) -> MissionSpec {
    let mut spec = MissionSpec::new(theater, aircraft);
    spec.wings[3].count = 1;
    spec.wings[3].skill = Skill::Average;
    spec.start = Start::Airborne {
        altitude_ft: *mission::ALTITUDES_FT.last().expect("an altitude"),
    };
    spec
}

/// The aircraft the item stands for.
fn aircraft_of(item: &Item) -> Option<AircraftId> {
    AircraftId::parse(&item.key).ok()
}

/// The shared item: what the reference mission reads less the names of its
/// own aircraft and theater (its weapons are read by the aircraft). `None`
/// when the reference mission cannot be built.
fn shared_item(
    resources: &BTreeMap<String, Vec<u8>>,
    aircraft: &Item,
    theater: &Item,
) -> Option<Item> {
    let spec = reference_spec(aircraft_of(aircraft)?, &theater.key);
    let reads = ResourceReads::new(resources);
    World::new(&spec, &reads, Seating::Open).ok()?;
    let own: BTreeSet<&str> = aircraft.names().chain(theater.names()).collect();
    let manifest = Manifest {
        entries: reads
            .manifest()
            .entries
            .into_iter()
            .filter(|entry| !own.contains(entry.name.as_str()))
            .collect(),
    };
    Some(Item::new(Kind::Shared, SHARED_KEY, manifest))
}
