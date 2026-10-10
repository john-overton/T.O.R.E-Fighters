//! Resolution: which surface units a mission has, with their fixed ids,
//! owners and sides (docs/spec/surface-defenses.md, "Templates and defended
//! slots", "Ownership and sides", "Identifiers").
//!
//! - The theater layout's NT placements become units with their layout ids
//!   and their own owners (John, 2026-10-10: base-layout air defenses go
//!   active, on both sides).
//! - A ground target's template is resolved object by object in ordinal
//!   order: the owner rewrite, the `<sam>` and `<aaa>` defense rolls (0, 25,
//!   60 or 100 percent), the uniform pick from the enemy nationality's
//!   equipment group, the night and stealth rule. Retail data throughout.
//!
//! Every random choice comes from its own [`Stream`], keyed by the mission's
//! seed, the template, the object and the purpose, so one object's results
//! never depend on how many others survive or the order they are handled.
//! No placement geometry here: jitter, relocation, batteries and added trucks
//! are the layout slice's.
use super::{
    LeftOut, Origin, SupplyTruck, Surface, TemplateSite, Unit, UnitId, UnitKind,
    catalog::{Catalog, Family},
    side_of_owner,
    units::ParkedAircraft,
};
use crate::mission::{Condition, MissionSpec};
use std::collections::BTreeMap;
use tore_formats::{
    mission::{Layout, NationalityField, Placement, SourceKey},
    quick_template::{
        ObjectKind, Placeholder, Template,
        tables::{
            NIGHT_AAA, NIGHT_AAA_SKILL, NIGHT_STEALTH_AIRCRAFT, defense_percent, equipment,
            group_of,
        },
    },
};

/// Skill of a unit whose record names none: average, as the retail generator
/// writes `themGroundSkill 1` (fitted).
pub const DEFAULT_SKILL: i32 = 1;

/// The Quick Mission ground target: the creator's fields 30 to 32, the enemy
/// nationality (field 20), the mission's surface seed and the night rule's
/// condition.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GroundTarget {
    /// Template stem, `QUCOL` for `~QUCOL.M`.
    pub stem: String,
    /// Defense levels 0 (none) to 3 (heavy), field 31 and field 32.
    pub aaa: usize,
    pub sam: usize,
    pub seed: u32,
    /// Creator nationality index of the enemy, field 20.
    pub enemy_nationality: usize,
    /// Night, and a friendly wing flies an F-117 or a B-2.
    pub night_stealth: bool,
}

impl GroundTarget {
    /// The ground target `spec` carries, `None` without one: its template
    /// stem, the creator's defense levels, the surface seed (0 when none was
    /// drawn), the enemy nationality and the night rule.
    pub fn from_spec(spec: &MissionSpec) -> Option<Self> {
        let stem = spec.ground_target.as_deref()?;
        Some(Self {
            stem: stem.trim_start_matches('~').to_ascii_uppercase(),
            aaa: spec.aaa.level(),
            sam: spec.sam.level(),
            seed: spec.surface_seed,
            enemy_nationality: usize::from(spec.enemy_nationality),
            night_stealth: night_stealth(spec),
        })
    }
}

/// The night and stealth rule's condition: night, with an F-117 or a B-2 in
/// any friendly wing (retail data; dormant while neither is imported).
pub fn night_stealth(spec: &MissionSpec) -> bool {
    spec.condition == Condition::Night
        && spec.wings[..3]
            .iter()
            .any(|wing| wing.count > 0 && NIGHT_STEALTH_AIRCRAFT.contains(&wing.aircraft.pt()))
}

/// What a random draw decides. Each purpose has its own stream per object.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    /// Whether a defense slot is manned.
    Roll = 1,
    /// Which type fills a placeholder.
    Pick = 2,
    /// Jitter offset (layout slice).
    Offset = 3,
    /// Jitter heading (layout slice).
    Heading = 4,
    /// The template's relocation (layout slice).
    Group = 5,
    /// Added supply trucks (layout slice).
    Supply = 6,
    /// Added battery radars (layout slice).
    Radar = 7,
}

/// SplitMix64, seeded from (mission seed, template, object ordinal,
/// purpose). Integer arithmetic only, so every platform draws the same
/// numbers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stream(u64);

impl Stream {
    pub fn new(seed: u32, stem: &str, ordinal: u32, purpose: Purpose) -> Self {
        let stem = tore_codec::fnv1a64(stem.to_ascii_uppercase().as_bytes());
        let key = (u64::from(seed) << 32 | u64::from(ordinal))
            ^ stem.rotate_left(17)
            ^ (purpose as u64).wrapping_mul(0xD6E8_FEB8_6659_FD93);
        let mut stream = Self(key);
        // One step so nearby keys start far apart.
        stream.next_u64();
        stream
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// A uniform draw below `n` (multiply-high), 0 for `n` 0.
    pub fn below(&mut self, n: u64) -> u64 {
        ((u128::from(self.next_u64()) * u128::from(n)) >> 64) as u64
    }
}

/// The base layout's units and the side of every owned placement.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LayoutResolution {
    pub units: Vec<Unit>,
    pub object_sides: BTreeMap<u32, tore_sim::combat::live::Side>,
    pub unreadable: Vec<(String, String)>,
}

/// The layout's NT placements as units, keeping their layout ids, owners and
/// flags. A placement whose NT does not read stays plain scenery and is
/// reported. Every placement with an owner field records its side, buildings
/// included.
pub fn layout(layout: &Layout, catalog: &mut Catalog<'_>) -> LayoutResolution {
    let mut out = LayoutResolution::default();
    for placement in &layout.placements {
        let Some(id) = UnitId::layout(placement.key.ordinal) else {
            continue;
        };
        let side = side_of_owner(placement.redfor());
        if side != tore_sim::combat::live::NO_SIDE {
            out.object_sides.insert(id.0, side);
        }
        if !placement.object_type.ends_with(".NT") {
            continue;
        }
        let entry = match catalog.entry(&placement.object_type) {
            Ok(entry) => entry,
            Err(error) => {
                out.unreadable.push((placement.object_type.clone(), error));
                continue;
            }
        };
        let Family::Unit(kind) = entry.family else {
            continue;
        };
        out.units.push(Unit {
            id,
            origin: Origin::Layout {
                ordinal: placement.key.ordinal,
            },
            resource: entry.resource.clone(),
            kind,
            class: entry.class,
            name: entry.name.clone(),
            nationality: placement.nationality,
            side,
            position: placement.position,
            angles: placement.angles,
            flags: placement.flags.unwrap_or(0),
            skill: DEFAULT_SKILL,
            react: None,
            search_dist: None,
            start_time: None,
            route: None,
            hit_points: entry.hit_points,
            look: entry.look.clone(),
            explosion: entry.explosion,
            crater: entry.crater,
            supply_truck: entry.supply_truck,
            in_scene: false,
        });
    }
    out.units.sort_by_key(|unit| unit.id);
    out
}

/// A resolved template.
#[derive(Clone, Debug, PartialEq)]
pub struct TemplateResolution {
    pub site: TemplateSite,
    /// Surface units (NT and OT), ascending id.
    pub units: Vec<Unit>,
    /// Aircraft parked on the ground, ascending id.
    pub parked: Vec<ParkedAircraft>,
    /// The units as placements for the scene, ascending id.
    pub placements: Vec<(UnitId, Placement)>,
}

/// Resolves `template` for `target`. `map` is the theater layout's map name,
/// which only the legacy `nationality` field's remap reads.
pub fn template(
    template: &Template,
    target: &GroundTarget,
    catalog: &mut Catalog<'_>,
    map: Option<&str>,
) -> Result<TemplateResolution, String> {
    let stem = template.stem.as_str();
    let group = group_of(target.enemy_nationality).ok_or_else(|| {
        format!(
            "enemy nationality {} is outside the creator list",
            target.enemy_nationality
        )
    })?;
    let percent = |level: usize, what: &str| {
        defense_percent(level).ok_or_else(|| format!("{what} defense level {level} is past heavy"))
    };
    let (aaa_percent, sam_percent) = (percent(target.aaa, "AAA")?, percent(target.sam, "SAM")?);
    let enemy = i32::try_from(target.enemy_nationality).map_err(|e| e.to_string())? | 0x80;
    let resource = template.resource();
    // Aircraft in a template that holds a carrier stand on its deck when
    // their spot lies on it, and stay out otherwise (lead ruling after S2).
    let mut carrier = false;
    for object in &template.objects {
        carrier |= match &object.kind {
            ObjectKind::Placeholder(p) => *p == Placeholder::Carrier,
            ObjectKind::Named(name) => catalog
                .resource_name(name)
                .and_then(|name| catalog.entry(&name).ok())
                .is_some_and(|entry| entry.carrier),
        };
    }
    let mut site = TemplateSite {
        stem: stem.to_owned(),
        settings: target.clone(),
        group,
        quickpos: template.quickpos,
        objects: template.objects.len(),
        removed: Vec::new(),
        left_out: Vec::new(),
    };
    let mut units = Vec::new();
    let mut parked = Vec::new();
    let mut deck_candidates = Vec::new();
    let mut placements = Vec::new();
    for object in &template.objects {
        let ordinal = object.ordinal;
        let id = UnitId::template(ordinal)
            .ok_or_else(|| format!("{stem}: object {ordinal} outside the template id range"))?;
        // The owner: `nationality` and `nationality2` become the enemy with
        // the Redfor bit; `nationality3` passes through as written.
        let nationality = match object.owner.field {
            NationalityField::Legacy | NationalityField::Two => enemy,
            NationalityField::Three => object.owner.index(map),
        };
        let side = side_of_owner(Some(nationality & 0x80 != 0));
        let mut skill = object.skill.unwrap_or(DEFAULT_SKILL);
        let placeholder = object.placeholder();
        let chosen = match &object.kind {
            ObjectKind::Named(name) => catalog.resource_name(name).ok_or_else(|| {
                format!("{stem}: object {ordinal} names {name}, which the import lacks")
            })?,
            ObjectKind::Placeholder(Placeholder::Nothing) => {
                site.left_out.push(LeftOut {
                    ordinal,
                    resource: "<nothing>".into(),
                    why: "an empty slot",
                });
                continue;
            }
            ObjectKind::Placeholder(p) => {
                if p.is_defense() {
                    let chance = if *p == Placeholder::Sam {
                        sam_percent
                    } else {
                        aaa_percent
                    };
                    let manned = Stream::new(target.seed, stem, ordinal, Purpose::Roll).below(100)
                        < u64::from(chance);
                    if !manned {
                        site.removed.push(ordinal);
                        continue;
                    }
                }
                let night = *p == Placeholder::Aaa && target.night_stealth;
                let list = if night {
                    skill = NIGHT_AAA_SKILL;
                    NIGHT_AAA
                } else {
                    equipment(*p, group)
                        .ok_or_else(|| format!("{stem}: no list for <{}>", p.name()))?
                };
                let pick = Stream::new(target.seed, stem, ordinal, Purpose::Pick)
                    .below(list.len() as u64) as usize;
                list[pick].to_owned()
            }
        };
        let entry = catalog
            .entry(&chosen)
            .map_err(|error| format!("{stem}: object {ordinal}: {error}"))?;
        match entry.family {
            Family::Aircraft => {
                let aircraft = ParkedAircraft {
                    id,
                    resource: entry.resource.clone(),
                    position: object.position,
                    angles: object.angles,
                    nationality: Some(nationality),
                    side,
                    target: object.is_target(),
                    deck: None,
                };
                if carrier {
                    deck_candidates.push(aircraft);
                } else {
                    parked.push(aircraft);
                }
            }
            Family::Unit(_) | Family::Object => {
                let kind = match entry.family {
                    Family::Unit(kind) => kind,
                    _ => UnitKind::Structure,
                };
                units.push(Unit {
                    id,
                    origin: Origin::Template {
                        ordinal,
                        placeholder,
                    },
                    resource: entry.resource.clone(),
                    kind,
                    class: entry.class,
                    name: entry.name.clone(),
                    nationality: Some(nationality),
                    side,
                    position: object.position,
                    angles: object.angles,
                    flags: object.flags,
                    skill,
                    react: object.react,
                    search_dist: object.search_dist,
                    start_time: object.start_time,
                    route: object.route.clone(),
                    hit_points: entry.hit_points,
                    look: entry.look.clone(),
                    explosion: entry.explosion,
                    crater: entry.crater,
                    supply_truck: entry.supply_truck,
                    in_scene: false,
                });
                placements.push((
                    id,
                    Placement {
                        key: SourceKey {
                            layout: resource.clone(),
                            ordinal,
                        },
                        section: None,
                        object_type: entry.resource.clone(),
                        position: object.position,
                        angles: object.angles,
                        source_nationality: Some(nationality),
                        nationality2: false,
                        // The retail writer emits every owner as
                        // `nationality3` once rewritten.
                        nationality3: true,
                        nationality: Some(nationality),
                        flags: Some(object.flags),
                        speed: Some(object.speed),
                        name: None,
                        alias: Some(object.alias),
                        unknown: Vec::new(),
                    },
                ));
            }
        }
    }
    // Fleet aircraft: on the first carrier whose deck holds their spot.
    if !deck_candidates.is_empty() {
        let mut hulls = Vec::new();
        for unit in &units {
            let is_carrier = catalog
                .entry(&unit.resource)
                .is_ok_and(|entry| entry.carrier);
            if is_carrier
                && let Some(shape) = super::catalog::unit_shape(catalog.resources(), &unit.resource)
                && let Some(hull) = super::parked::Hull::of(catalog.resources(), unit, &shape)
            {
                hulls.push((unit, hull));
            }
        }
        for mut aircraft in deck_candidates {
            let on = hulls.iter().find(|(unit, hull)| {
                super::parked::deck_spot(
                    &hull.deck,
                    hull.authored_scale,
                    hull.placed_scale,
                    unit.position,
                    unit.angles[0],
                    aircraft.position,
                )
                .is_some()
            });
            match on {
                Some((unit, _)) => {
                    aircraft.deck = Some(unit.id);
                    parked.push(aircraft);
                }
                None => site.left_out.push(LeftOut {
                    ordinal: aircraft.id.0 - super::SURFACE_UNIT_BASE,
                    resource: aircraft.resource,
                    why: "on no carrier's deck",
                }),
            }
        }
        parked.sort_by_key(|aircraft| aircraft.id);
        site.left_out.sort_by_key(|left| left.ordinal);
    }
    Ok(TemplateResolution {
        site,
        units,
        parked,
        placements,
    })
}

/// The surface of a mission: the layout's units, and the template's when it
/// has a ground target. The supply trucks already standing in either are
/// listed; added trucks, batteries and placement are the layout slice's.
pub fn surface(
    base: LayoutResolution,
    target: Option<TemplateResolution>,
) -> Result<Surface, String> {
    let LayoutResolution {
        mut units,
        mut object_sides,
        unreadable,
    } = base;
    let mut surface = Surface {
        unreadable,
        ..Surface::default()
    };
    if let Some(resolved) = target {
        for unit in &resolved.units {
            if unit.side != tore_sim::combat::live::NO_SIDE {
                object_sides.insert(unit.id.0, unit.side);
            }
        }
        for parked in &resolved.parked {
            object_sides.insert(parked.id.0, parked.side);
        }
        units.extend(resolved.units);
        surface.parked = resolved.parked;
        surface.placements = resolved.placements;
        surface.template = Some(resolved.site);
    }
    units.sort_by_key(|unit| unit.id);
    if units.windows(2).any(|pair| pair[0].id == pair[1].id) {
        return Err("two surface units share an id".into());
    }
    surface.trucks = units
        .iter()
        .filter(|unit| unit.supply_truck)
        .map(|unit| SupplyTruck {
            id: unit.id,
            serves: None,
            added: false,
        })
        .collect();
    surface.units = units;
    surface.object_sides = object_sides;
    Ok(surface)
}
