//! The surface unit types a mission places, read once each from the imported
//! records: NT surface units through `tore_formats::surface_unit`, static
//! objects (OT) through their OBJECT block and parked aircraft (PT) through
//! `tore_formats::parked_aircraft`. It answers what
//! resolution needs per type: its kind, class, name, hit points, destroyed
//! look, explosion and whether it is a supply truck or a carrier. The weapon
//! tuning joins it in the weapons slices.
use super::{DestroyedLook, UnitKind};
use crate::resources::ResourceSource;
use std::collections::BTreeMap;
use std::sync::Arc;
use tore_formats::{
    parked_aircraft::ParkedType,
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
    /// A carrier (`_CARRIERProc`): its template's aircraft stand on its deck
    /// when their spot lies on it ([`super::parked::deck_spot`]).
    pub carrier: bool,
    /// The NT record, for surface units.
    pub unit: Option<Arc<SurfaceUnit>>,
    /// The PT's OBJECT block, for parked aircraft.
    pub aircraft: Option<Arc<ParkedType>>,
}

/// The main shape `resource` (an NT) names, if it reads.
pub fn unit_shape(resources: &dyn ResourceSource, resource: &str) -> Option<String> {
    SurfaceUnit::parse(resources.get(resource)?).ok()?.shape
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

/// The HAWK battery's radar element (docs/spec/surface-defenses.md, "SAM
/// batteries"). The LIB has no HAWK radar, so this TORE-defined type takes
/// the Straight Flush record's numbers (hit points, vehicle class,
/// signatures, explosion) and draws a LIB radar shape: SRDR2, John's pick
/// from the radar shape sheet (2026-10-10, decision 12.2). No archive holds this name.
pub const HAWK_RADAR: &str = "HAWKRDR.NT";
/// The record the HAWK radar takes its numbers from.
pub const HAWK_RADAR_BASIS: &str = "SFLUSH.NT";
/// The shape the HAWK radar draws: John chose SRDR2 (2026-10-10).
pub const HAWK_RADAR_SHAPE: &str = "SRDR2.SH";
/// Its display name.
pub const HAWK_RADAR_NAME: &str = "HAWK Radar";

/// The HAWK radar's static definition, for the scene: the Straight Flush
/// record's with the HAWK radar's shape and name. `None` when the import
/// lacks either.
pub fn hawk_radar_definition(
    resources: &dyn ResourceSource,
) -> Option<tore_formats::static_object::Definition> {
    let mut definition = Definition::parse(resources.get(HAWK_RADAR_BASIS)?).ok()?;
    resources.get(HAWK_RADAR_SHAPE)?;
    definition.main_shape = Some(HAWK_RADAR_SHAPE.to_owned());
    definition.display_name = HAWK_RADAR_NAME.to_owned();
    definition.resource_name = HAWK_RADAR.to_owned();
    Some(definition)
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

    /// The import the catalog reads.
    pub fn resources(&self) -> &'a dyn ResourceSource {
        self.resources
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
        if resource == HAWK_RADAR {
            return self.hawk_radar();
        }
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
                    aircraft: None,
                })
            }
            // A parked aircraft: its OBJECT block only, whatever the type
            // (no whitelist, no aliasing).
            "PT" => {
                let record = ParkedType::parse(bytes).map_err(|e| format!("{resource}: {e}"))?;
                Ok(Entry {
                    resource: resource.to_owned(),
                    family: Family::Aircraft,
                    class: record.class,
                    name: record.short_name.clone(),
                    hit_points: record.hit_points,
                    // The aircraft ground-crash look, drawn by the
                    // parked-aircraft path.
                    look: DestroyedLook::Removed,
                    // Aircraft explode as aircraft (the parked-aircraft
                    // path), not with their record's look.
                    explosion: None,
                    crater: None,
                    supply_truck: false,
                    carrier: false,
                    unit: None,
                    aircraft: Some(Arc::new(record)),
                })
            }
            "OT" => {
                let definition =
                    Definition::parse(bytes).map_err(|e| format!("{resource}: {e}"))?;
                Ok(Entry {
                    resource: resource.to_owned(),
                    family: Family::Object,
                    class: definition.category,
                    name: definition.display_name.clone(),
                    hit_points: definition.hit_points.unwrap_or(DEFAULT_HIT_POINTS),
                    look: self.object_look(resource),
                    explosion: Some(definition.explosion),
                    crater: Some(definition.crater),
                    supply_truck: false,
                    carrier: false,
                    unit: None,
                    aircraft: None,
                })
            }
            _ => Err(format!("{resource}: not a surface object type")),
        }
    }

    /// [`HAWK_RADAR`]: the Straight Flush's entry under the HAWK radar's
    /// name and shape.
    fn hawk_radar(&self) -> Result<Entry, String> {
        if !self.has(HAWK_RADAR_SHAPE) {
            return Err(format!("missing {HAWK_RADAR_SHAPE}; re-import media"));
        }
        let mut entry = self.read(HAWK_RADAR_BASIS)?;
        entry.resource = HAWK_RADAR.to_owned();
        entry.name = HAWK_RADAR_NAME.to_owned();
        if let Some(unit) = &entry.unit {
            let mut unit = SurfaceUnit::clone(unit);
            unit.resource = HAWK_RADAR.to_owned();
            unit.short_name = HAWK_RADAR_NAME.to_owned();
            unit.name = HAWK_RADAR_NAME.to_owned();
            unit.shape = Some(HAWK_RADAR_SHAPE.to_owned());
            entry.unit = Some(Arc::new(unit));
        }
        Ok(entry)
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
