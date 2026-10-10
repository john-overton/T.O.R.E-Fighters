//! The surface unit types a mission places, read once each from the imported
//! records: NT surface units through `tore_formats::surface_unit`, static
//! objects (OT) and aircraft (PT) through their OBJECT block. It answers what
//! resolution needs per type: its kind, class, name, hit points, destroyed
//! look, explosion and whether it is a supply truck or a carrier. The weapon
//! tuning joins it in the weapons slices.
use super::{DestroyedLook, UnitKind};
use crate::resources::ResourceSource;
use std::collections::BTreeMap;
use std::sync::Arc;
use tore_formats::{
    static_object::Definition,
    surface_unit::{DESTROYED_VEHICLE_OBJECT, SurfaceUnit, class},
};

/// What resolution needs to know about one placed type.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// Archive resource: `SA6.NT`.
    pub resource: String,
    pub family: Family,
    pub class: u16,
    /// Short display name.
    pub name: String,
    pub hit_points: i32,
    pub look: DestroyedLook,
    pub explosion: Option<u8>,
    pub crater: Option<u8>,
    pub supply_truck: bool,
    /// A carrier (`_CARRIERProc`): its template's aircraft are scheduled deck
    /// launches, not parked aircraft.
    pub carrier: bool,
    /// The NT record, for surface units.
    pub unit: Option<Arc<SurfaceUnit>>,
}

/// The three record families a template or layout places.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Family {
    /// An NT surface unit; armed or not.
    Unit(UnitKind),
    /// An OT static object.
    Object,
    /// A PT aircraft.
    Aircraft,
}

/// The default hit points of a static object whose record names none, as
/// the airport scene gives it.
const DEFAULT_HIT_POINTS: i32 = 100;

/// Types read so far, by resource. A record that does not read is cached as
/// its error, so it is reported once.
pub struct Catalog<'a> {
    resources: &'a dyn ResourceSource,
    entries: BTreeMap<String, Result<Arc<Entry>, String>>,
}

impl<'a> Catalog<'a> {
    pub fn new(resources: &'a dyn ResourceSource) -> Self {
        Self {
            resources,
            entries: BTreeMap::new(),
        }
    }

    /// Whether the import holds `name`.
    pub fn has(&self, name: &str) -> bool {
        self.resources.get(name).is_some()
    }

    /// The archive name of a template's `type`: as written when the import
    /// has it, else with the first of `.NT`, `.OT` or `.PT` it has.
    pub fn resource_name(&self, written: &str) -> Option<String> {
        let upper = written.to_ascii_uppercase();
        if self.has(&upper) {
            return Some(upper);
        }
        if upper.contains('.') {
            return None;
        }
        ["NT", "OT", "PT"]
            .into_iter()
            .map(|ext| format!("{upper}.{ext}"))
            .find(|name| self.has(name))
    }

    /// The entry of `resource`, read on first use.
    pub fn entry(&mut self, resource: &str) -> Result<Arc<Entry>, String> {
        let key = resource.to_ascii_uppercase();
        if let Some(found) = self.entries.get(&key) {
            return found.clone();
        }
        let read = self.read(&key).map(Arc::new);
        self.entries.insert(key, read.clone());
        read
    }

    fn read(&self, resource: &str) -> Result<Entry, String> {
        let bytes = self
            .resources
            .get(resource)
            .ok_or_else(|| format!("missing {resource}; re-import media"))?;
        let extension = resource.rsplit_once('.').map_or("", |(_, ext)| ext);
        match extension {
            "NT" => {
                let unit = SurfaceUnit::parse(bytes).map_err(|e| format!("{resource}: {e}"))?;
                let kind = if unit.armed() {
                    UnitKind::Active
                } else {
                    UnitKind::Passive
                };
                Ok(Entry {
                    resource: resource.to_owned(),
                    family: Family::Unit(kind),
                    class: unit.class,
                    name: unit.short_name.clone(),
                    hit_points: unit.hit_points,
                    look: self.unit_look(&unit),
                    explosion: Some(unit.explosion),
                    crater: Some(unit.crater),
                    supply_truck: unit.is_supply_truck(),
                    carrier: unit.callback == "_CARRIERProc",
                    unit: Some(Arc::new(unit)),
                })
            }
            "OT" | "PT" => {
                let definition =
                    Definition::parse(bytes).map_err(|e| format!("{resource}: {e}"))?;
                let aircraft = extension == "PT";
                Ok(Entry {
                    resource: resource.to_owned(),
                    family: if aircraft {
                        Family::Aircraft
                    } else {
                        Family::Object
                    },
                    class: definition.category,
                    name: definition.display_name.clone(),
                    hit_points: definition.hit_points.unwrap_or(DEFAULT_HIT_POINTS),
                    look: if aircraft {
                        // The aircraft ground-crash look, drawn by the
                        // parked-aircraft path.
                        DestroyedLook::Removed
                    } else {
                        self.object_look(resource)
                    },
                    explosion: None,
                    crater: None,
                    supply_truck: false,
                    carrier: false,
                    unit: None,
                })
            }
            _ => Err(format!("{resource}: not a surface object type")),
        }
    }

    /// Ships swap to their `_A` shape; men and invisible units vanish; the
    /// GCI radar (a structure) is removed like a building; every other
    /// vehicle, SAM launcher and gun leaves the `DEST.OT` wreck.
    fn unit_look(&self, unit: &SurfaceUnit) -> DestroyedLook {
        if unit.shape.is_none() || unit.class & class::OTHER != 0 {
            return DestroyedLook::Vanish;
        }
        if unit.class & class::SHIP != 0 {
            return unit
                .damaged_shape
                .clone()
                .filter(|shape| self.has(shape))
                .map_or(DestroyedLook::Removed, DestroyedLook::DamagedShape);
        }
        if unit.class & class::STRUCTURE != 0 {
            return DestroyedLook::Removed;
        }
        if self.has(DESTROYED_VEHICLE_OBJECT) {
            DestroyedLook::Wreck(DESTROYED_VEHICLE_OBJECT.to_owned())
        } else {
            DestroyedLook::Removed
        }
    }

    /// A building whose damaged variant `~NAME.OT` the import holds swaps to
    /// it (`BNK5.OT` to `~BNK5.OT`); any other is removed.
    fn object_look(&self, resource: &str) -> DestroyedLook {
        let damaged = format!("~{resource}");
        if !resource.starts_with('~') && self.has(&damaged) {
            DestroyedLook::DamagedObject(damaged)
        } else {
            DestroyedLook::Removed
        }
    }
}
