//! Which resources the surface round needs from a Fighters Anthology library,
//! worked out from the data itself (slice IM1).
//!
//! A Quick Mission ground target is a template (`~Q*.M`) whose objects name
//! surface units (NT), static objects (OT) and aircraft (PT). The executable's
//! equipment lists name the units a `<sam>`, `<aaa>` or ship placeholder can turn
//! into, and the base theater layouts name the units they place. This module
//! starts from those four sources, reads each record with the readers in this
//! crate and follows what the record names: shapes, damaged shapes, textures,
//! weapon and sensor records, sounds and scripts. It reads bounded data only; no
//! callback, symbol or mission statement is executed.
//!
//! The same code serves the importer (which keeps what [`select`] returns) and
//! the completeness check on an existing pack (which asks [`select`] over the
//! pack and compares). Nothing here holds a name that the data does not give,
//! except four naming rules, each seen in the retail library and listed on
//! [`select`]. Behaviour: `docs/formats/surface-units.md` and
//! `docs/spec/import-cache.md`.
use crate::{
    Archive, Result,
    aircraft::references,
    invalid,
    mission::Layout,
    quick_template::{ObjectKind, Template, tables},
    static_object::Definition,
    surface_unit::{DESTROYED_VEHICLE_OBJECT, SurfaceUnit},
    theater,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// The callback of the carriers whose towers and far shapes the library keeps
/// beside the hull (NIMZ, KITT, CLEM, WASP).
const CARRIER_CALLBACK: &str = "_CARRIERProc";
/// Suffixes of the aircraft look variants: the shadow and the four damage
/// states, as `SU27_S.SH` and `SU27_A.SH` to `SU27_D.SH`.
const AIRCRAFT_LOOKS: [char; 5] = ['S', 'A', 'B', 'C', 'D'];
/// A bound on the names one selection may hold (a full library has about
/// 5,400 resources in the second archive).
const MAX_RESOURCES: usize = 16_384;

/// Somewhere to look names up: the archives of a source, or the resources of a
/// pack.
pub trait Library {
    /// Every name the library holds.
    fn names(&self) -> Vec<String>;
    fn has(&self, name: &str) -> bool;
    fn read(&self, name: &str) -> Result<Vec<u8>>;
}

/// The archives of a source, searched in order.
pub struct Archives<'a>(pub &'a [&'a Archive]);

impl Library for Archives<'_> {
    fn names(&self) -> Vec<String> {
        let all: BTreeSet<&String> = self.0.iter().flat_map(|a| a.entries.keys()).collect();
        all.into_iter().cloned().collect()
    }
    fn has(&self, name: &str) -> bool {
        self.0.iter().any(|a| a.entries.contains_key(name))
    }
    fn read(&self, name: &str) -> Result<Vec<u8>> {
        for archive in self.0 {
            if archive.entries.contains_key(name) {
                return archive.read(name);
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("missing {name}"),
        ))
    }
}

/// The resources of a pack (or any name to bytes map).
impl Library for BTreeMap<String, Vec<u8>> {
    fn names(&self) -> Vec<String> {
        self.keys().cloned().collect()
    }
    fn has(&self, name: &str) -> bool {
        self.contains_key(name)
    }
    fn read(&self, name: &str) -> Result<Vec<u8>> {
        self.get(name).cloned().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, format!("missing {name}"))
        })
    }
}

/// A name the data requires and the library lacks.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Missing {
    pub name: String,
    /// What names it: a template, a record or a table.
    pub needed_by: String,
}

/// The families of record a selection holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Family {
    /// NT surface units.
    Unit,
    /// OT static objects.
    Object,
    /// PT aircraft, kept as parked targets: their shapes and OBJECT record.
    Aircraft,
}

/// What [`select`] decided.
#[derive(Debug, Default)]
pub struct Selection {
    /// Every resource to keep, by library name.
    pub resources: BTreeSet<String>,
    /// The template resources (`~QUCOL.M`).
    pub templates: BTreeSet<String>,
    /// The record types the templates, equipment lists and layouts can name,
    /// and the NT records the library holds, by family.
    pub types: BTreeMap<Family, BTreeSet<String>>,
    /// Required names the library lacks. An empty list is a complete set.
    pub missing: BTreeSet<Missing>,
    /// Records that did not read; their raw references were still followed.
    pub unread: BTreeMap<String, String>,
}

impl Selection {
    /// The number of record types of a family.
    pub fn count(&self, family: Family) -> usize {
        self.types.get(&family).map_or(0, BTreeSet::len)
    }
}

/// The resources the surface round needs: the 129 Quick Mission templates, the
/// record types they, the executable's equipment lists and the base layouts
/// name, and everything those records name in turn.
///
/// Roots, all from data: every `~Q*.M` in the library and every template
/// [`tables::TEMPLATES`] lists; the named types of each template; every unit
/// of [`tables::LISTS`] and [`tables::NIGHT_AAA`]; the object type of every
/// placement in the base theater layouts; every `.NT` record in the library;
/// and [`DESTROYED_VEHICLE_OBJECT`].
///
/// Followed from a record: an NT's shape, shadow shape, damaged shape, weapon
/// and sensor records and script; an OT's main shape; the resources a record's
/// text names (`references`), kept when the library holds them as a shape,
/// picture, weapon or sensor record, script, sound or record; and a shape's
/// picture and sub-shape names. A parked aircraft (PT) keeps its record, main
/// shape and look variants only, never the flight, cockpit or sound records.
///
/// Four naming rules, each seen in the retail library and each applied only
/// when the library holds the name: a ship's or object's shape `S` has a damaged
/// look `S_A`; an object `X.OT` has a damaged variant `~X.OT`; a carrier (an NT
/// whose callback is `_CARRIERProc`) has a tower `<stem>T.SH` with a `~<stem>T.OT`
/// and a far shape `X<stem>.SH`; an aircraft shape `S` has looks `S_S` and `S_A`
/// to `S_D`.
pub fn select(library: &dyn Library) -> Result<Selection> {
    let mut walk = Walk::new(library);
    walk.roots()?;
    walk.run()?;
    Ok(walk.selection)
}

/// The base theater layouts: the names the importer keeps scenes for.
pub fn base_layouts(library: &dyn Library) -> Vec<String> {
    library
        .names()
        .into_iter()
        .filter(|name| name.ends_with(".MM") && theater::base_theater(name).is_some())
        .collect()
}

struct Walk<'a> {
    library: &'a dyn Library,
    selection: Selection,
    queue: VecDeque<String>,
    /// Names whose record has been visited.
    visited: BTreeSet<String>,
}

fn extension(name: &str) -> &str {
    name.rsplit_once('.').map_or("", |(_, ext)| ext)
}

/// The shape's name without `.SH`.
fn stem(shape: &str) -> &str {
    shape.strip_suffix(".SH").unwrap_or(shape)
}

/// Whether a name a record's text holds is something the surface round keeps.
fn followed(name: &str) -> bool {
    matches!(
        extension(name),
        "SH" | "PIC" | "JT" | "SEE" | "ECM" | "GAS" | "BI" | "NT" | "OT"
    ) || name.starts_with('&')
        || name.starts_with('^')
}

impl<'a> Walk<'a> {
    fn new(library: &'a dyn Library) -> Self {
        Self {
            library,
            selection: Selection::default(),
            queue: VecDeque::new(),
            visited: BTreeSet::new(),
        }
    }

    fn need(&mut self, name: &str, needed_by: &str) -> bool {
        if self.library.has(name) {
            self.add(name);
            true
        } else {
            self.selection.missing.insert(Missing {
                name: name.to_owned(),
                needed_by: needed_by.to_owned(),
            });
            false
        }
    }

    /// Keeps `name` and queues its record for a visit.
    fn add(&mut self, name: &str) {
        if self.selection.resources.insert(name.to_owned()) {
            self.queue.push_back(name.to_owned());
        }
    }

    /// Keeps `name` if the library holds it.
    fn maybe(&mut self, name: &str) -> bool {
        if self.library.has(name) {
            self.add(name);
            true
        } else {
            false
        }
    }

    /// A type a template, list or layout names: resolves a bare name to the
    /// first of `.NT`, `.OT` and `.PT` the library has.
    fn type_root(&mut self, written: &str, needed_by: &str) {
        let upper = written.to_ascii_uppercase();
        let resolved = if extension(&upper).is_empty() {
            ["NT", "OT", "PT"]
                .iter()
                .map(|ext| format!("{upper}.{ext}"))
                .find(|name| self.library.has(name))
        } else {
            None
        };
        let name = resolved.unwrap_or(upper);
        let family = match extension(&name) {
            "NT" => Family::Unit,
            "OT" => Family::Object,
            "PT" => Family::Aircraft,
            _ => {
                self.selection.missing.insert(Missing {
                    name,
                    needed_by: format!("{needed_by} (not a surface record type)"),
                });
                return;
            }
        };
        if self.need(&name, needed_by) {
            self.selection.types.entry(family).or_default().insert(name);
        }
    }

    fn roots(&mut self) -> Result<()> {
        let library = self.library;
        // Templates: the executable's table, and any other `~Q*.M` the library
        // holds.
        let mut templates: BTreeSet<String> = tables::TEMPLATES
            .iter()
            .flat_map(|list| list.iter())
            .chain(tables::UNREFERENCED.iter())
            .map(|stem| format!("~{stem}.M"))
            .collect();
        let tabled = templates.clone();
        templates.extend(
            library
                .names()
                .into_iter()
                .filter(|name| name.starts_with("~Q") && name.ends_with(".M")),
        );
        for name in &templates {
            if library.has(name) {
                self.selection.templates.insert(name.clone());
                self.add(name);
            } else if tabled.contains(name) {
                self.selection.missing.insert(Missing {
                    name: name.clone(),
                    needed_by: "the executable's template table".into(),
                });
            }
        }
        // The units, objects and aircraft the templates name.
        for name in self.selection.templates.clone() {
            let bytes = library.read(&name)?;
            match Template::parse(&name, &bytes) {
                Ok(template) => {
                    let mut named = BTreeSet::new();
                    for object in &template.objects {
                        if let ObjectKind::Named(type_name) = &object.kind {
                            named.insert(type_name.clone());
                        }
                    }
                    for type_name in named {
                        self.type_root(&type_name, &name);
                    }
                }
                Err(error) => {
                    self.selection
                        .unread
                        .insert(name.clone(), error.to_string());
                }
            }
        }
        // The equipment lists and the night rule's guns.
        let mut listed: BTreeSet<&str> = tables::NIGHT_AAA.iter().copied().collect();
        for lists in tables::LISTS.iter() {
            for group in lists.groups {
                listed.extend(group.iter().copied());
            }
        }
        for name in listed {
            self.type_root(name, "the executable's equipment lists");
        }
        // The base layouts' placements.
        for name in base_layouts(library) {
            let bytes = library.read(&name)?;
            match Layout::parse(&name, &bytes) {
                Ok(layout) => {
                    let types: BTreeSet<&String> = layout
                        .placements
                        .iter()
                        .map(|placement| &placement.object_type)
                        .collect();
                    for type_name in types {
                        self.type_root(type_name, &name);
                    }
                }
                Err(error) => {
                    self.selection
                        .unread
                        .insert(name.clone(), error.to_string());
                }
            }
        }
        // Every surface unit record, named by a template or not (the pilots and
        // deck crew, the carriers the lists reach only by placeholder).
        for name in library.names() {
            if name.ends_with(".NT") {
                self.type_root(&name, "the library's surface unit records");
            }
        }
        // The wreck every destroyed vehicle leaves.
        self.type_root(DESTROYED_VEHICLE_OBJECT, "the destroyed vehicle record");
        Ok(())
    }

    fn run(&mut self) -> Result<()> {
        while let Some(name) = self.queue.pop_front() {
            if self.selection.resources.len() > MAX_RESOURCES {
                return Err(invalid("surface selection exceeds its bound"));
            }
            if !self.visited.insert(name.clone()) {
                continue;
            }
            let bytes = self.library.read(&name)?;
            match extension(&name) {
                "NT" => self.unit(&name, &bytes),
                "OT" => self.object(&name, &bytes),
                "PT" => self.aircraft(&name, &bytes),
                "SH" => self.scan(&name, &bytes, |n| matches!(extension(n), "PIC" | "SH")),
                "JT" | "SEE" | "ECM" | "GAS" => self.scan(&name, &bytes, followed),
                _ => {}
            }
        }
        Ok(())
    }

    /// Keeps every name `bytes` holds that the library has and `keep` accepts.
    fn scan(&mut self, owner: &str, bytes: &[u8], keep: impl Fn(&str) -> bool) {
        for token in references(bytes) {
            if token != owner && keep(&token) {
                self.maybe(&token);
            }
        }
    }

    fn unit(&mut self, name: &str, bytes: &[u8]) {
        self.scan(name, bytes, followed);
        let unit = match SurfaceUnit::parse(bytes) {
            Ok(unit) => unit,
            Err(error) => {
                self.selection
                    .unread
                    .insert(name.to_owned(), error.to_string());
                return;
            }
        };
        let needed_by = name.to_owned();
        for shape in unit.shape.iter().chain(unit.shadow_shape.iter()) {
            self.need(shape, &needed_by);
        }
        let stores: Vec<String> = unit
            .mounts
            .iter()
            .filter_map(|mount| mount.store.clone())
            .collect();
        for store in stores {
            self.need(&store, &needed_by);
        }
        if let Some(sensor) = &unit.sensor {
            self.need(sensor, &needed_by);
        }
        if let Some(script) = &unit.npc.script {
            self.maybe(&script.to_ascii_uppercase());
        }
        if let Some(shape) = &unit.shape {
            // A ship's damaged look is derived, so the record must have it.
            if unit.is_ship() {
                if let Some(damaged) = &unit.damaged_shape {
                    self.need(damaged, &needed_by);
                }
            } else {
                self.maybe(&format!("{}_A.SH", stem(shape)));
            }
            if unit.callback == CARRIER_CALLBACK {
                let base = stem(shape).to_owned();
                self.maybe(&format!("{base}T.SH"));
                self.maybe(&format!("~{base}T.OT"));
                self.maybe(&format!("X{base}.SH"));
            }
        }
    }

    fn object(&mut self, name: &str, bytes: &[u8]) {
        self.scan(name, bytes, followed);
        match Definition::parse(bytes) {
            Ok(definition) => {
                if let Some(shape) = &definition.main_shape {
                    self.need(shape, name);
                    self.maybe(&format!("{}_A.SH", stem(shape)));
                }
            }
            Err(error) => {
                self.selection
                    .unread
                    .insert(name.to_owned(), error.to_string());
            }
        }
        if !name.starts_with('~') {
            self.maybe(&format!("~{name}"));
        }
    }

    /// A parked aircraft: the OBJECT record (the PT file), the main shape and
    /// the look variants. Flight, cockpit, weapon and sound records stay out.
    fn aircraft(&mut self, name: &str, bytes: &[u8]) {
        self.scan(name, bytes, |n| extension(n) == "SH");
        match Definition::parse(bytes) {
            Ok(definition) => {
                if let Some(shape) = &definition.main_shape {
                    self.need(shape, name);
                    let base = stem(shape).to_owned();
                    for look in AIRCRAFT_LOOKS {
                        self.maybe(&format!("{base}_{look}.SH"));
                    }
                }
            }
            Err(error) => {
                self.selection
                    .unread
                    .insert(name.to_owned(), error.to_string());
            }
        }
    }
}

/// Textures the shape reader finds in a shape, for the completeness check:
/// the scenery pose and the plain pose.
pub fn shape_textures(bytes: &[u8]) -> BTreeSet<String> {
    let mut textures = BTreeSet::new();
    for shape in [
        crate::shape::Shape::scenery(bytes),
        crate::shape::Shape::parse(bytes),
    ]
    .into_iter()
    .flatten()
    {
        textures.extend(
            shape
                .faces
                .iter()
                .map(|face| face.texture.clone())
                .chain(shape.billboards.iter().map(|b| b.texture.clone()))
                .filter(|t| !t.is_empty() && !t.starts_with('@')),
        );
    }
    textures
}

#[cfg(test)]
mod tests;
