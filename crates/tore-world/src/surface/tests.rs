//! Resolution, ids, sides and the digest against a synthetic import: every
//! surface type the equipment lists name, generated here, in the synthetic
//! theater. No retail data.
use super::{
    catalog::Catalog,
    resolve::{self, GroundTarget, Purpose, Stream},
    *,
};
use crate::{
    ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE},
    mission::{Condition, MissionSpec},
    terrain::{Overrides, Terrain},
    test_support::resources::{AIRPORT_AT, THEATER, resources, schema},
    world::{Seating, World},
};
use std::collections::{BTreeMap, BTreeSet};
use tore_formats::{
    aircraft::AircraftId,
    quick_template::{Placeholder, Template, tables},
    surface_unit::class,
};

pub(super) const HEADER: &str = "[brent's_relocatable_format]\n";

/// One line per field of `layout`; `value` overrides a field by name.
pub(super) fn fields(layout: &[(&str, &str)], value: &dyn Fn(&str) -> Option<String>) -> String {
    let mut text = String::new();
    for &(kind, name) in layout {
        let given = value(name);
        match (kind, given) {
            ("ptr", Some(block)) => text += &format!("ptr {block}\n"),
            ("ptr", None) => text += "dword 0\n",
            ("symbol", given) => text += &format!("symbol {}\n", given.unwrap_or_default()),
            (kind, given) => text += &format!("{kind} {}\n", given.unwrap_or_else(|| "0".into())),
        }
    }
    text
}

/// A synthetic NT: `class`, hit points, its proc and weapon mounts
/// (`store`, `maxItems`).
fn nt(stem: &str, class: u16, hp: i32, util: &str, mounts: &[(&str, i32)]) -> Vec<u8> {
    let size = 186 + 24 * mounts.len();
    let object = |name: &str| -> Option<String> {
        Some(match name {
            "structType" => "3".into(),
            "typeSize" => size.to_string(),
            "ot_names" => "ot_names".into(),
            "shape" => "shape".into(),
            "obj_class" => class.to_string(),
            "hitPoints" => hp.to_string(),
            "expType" => "21".into(),
            "craterSize" => "6".into(),
            "utilProc" => util.into(),
            _ => return None,
        })
    };
    let npc = |name: &str| -> Option<String> {
        Some(match name {
            "numHards" => mounts.len().to_string(),
            "hards" => "hards".into(),
            _ => return None,
        })
    };
    let mut text = String::from(HEADER);
    text += &fields(schema::OBJECT, &object);
    text += &fields(schema::NPC, &npc);
    text += ":hards\n";
    for (i, (_, items)) in mounts.iter().enumerate() {
        text += &format!(
            "word 8\nword 0\nword 30\nword 0\nword 0\nword 0\nword 0\nword 12740\nptr store{i}\nbyte 0\nword {items}\nbyte 0\n"
        );
    }
    text += &format!(
        ":ot_names\nstring \"{stem}\"\nstring \"Synthetic {stem}\"\nstring \"{stem}.NT\"\n"
    );
    text += &format!(":shape\nstring \"{stem}.SH\"\n");
    for (i, (store, _)) in mounts.iter().enumerate() {
        text += &format!(":store{i}\nstring \"{store}\"\n");
    }
    text += "end\n";
    text.into_bytes()
}

/// A synthetic static object (OT).
fn ot(stem: &str, class: u16, hp: i32) -> Vec<u8> {
    let object = |name: &str| -> Option<String> {
        Some(match name {
            "structType" => "1".into(),
            "ot_names" => "ot_names".into(),
            "shape" => "shape".into(),
            "obj_class" => class.to_string(),
            "hitPoints" => hp.to_string(),
            "utilProc" => "_OBJProc".into(),
            _ => return None,
        })
    };
    let mut text = String::from(HEADER);
    text += &fields(schema::OBJECT, &object);
    text += &format!(
        ":ot_names\nstring \"{stem}\"\nstring \"Synthetic {stem}\"\nstring \"{stem}.OT\"\n"
    );
    text += &format!(":shape\nstring \"{stem}.SH\"\nend\n");
    text.into_bytes()
}

/// The class a placeholder's picks carry in the fixture.
fn placeholder_class(placeholder: Placeholder) -> u16 {
    match placeholder {
        Placeholder::Sam => class::SAM,
        Placeholder::Aaa => class::AAA,
        Placeholder::Tank => class::TANK,
        Placeholder::Afv | Placeholder::Vehicle => class::VEHICLE,
        _ => class::SHIP,
    }
}

/// Where the fixture's objects stand: the middle of the synthetic theater.
pub(super) const MIDDLE: i32 = AIRPORT_AT as i32;

/// The synthetic import with every equipment-list type, a few named types,
/// a base layout with surface units on both sides and two templates.
pub(super) fn surface_resources() -> BTreeMap<String, Vec<u8>> {
    let mut r = resources();
    let shape = r["F18.SH"].clone();
    let add_nt = |r: &mut BTreeMap<String, Vec<u8>>, stem: &str, bytes: Vec<u8>| {
        r.insert(format!("{stem}.NT"), bytes);
        r.insert(format!("{stem}.SH"), shape.clone());
    };
    for lists in tables::LISTS {
        for group in lists.groups {
            for name in group {
                if r.contains_key(*name) {
                    continue;
                }
                let stem = name.trim_end_matches(".NT");
                let class = placeholder_class(lists.placeholder);
                let armed = matches!(class, class::SAM | class::AAA | class::TANK);
                let mounts: &[(&str, i32)] = if armed { &[("GUN.JT", 32767)] } else { &[] };
                let util = if matches!(stem, "KIEV" | "NIMZ" | "CLEM") {
                    "_CARRIERProc"
                } else {
                    "_GVProc"
                };
                add_nt(&mut r, stem, nt(stem, class, 100, util, mounts));
            }
        }
    }
    add_nt(
        &mut r,
        "GCI",
        nt("GCI", class::STRUCTURE, 100, "_OBJProc", &[("GCIR.SEE", 1)]),
    );
    add_nt(
        &mut r,
        "MISTRK",
        nt("MISTRK", class::VEHICLE, 50, "_GVProc", &[]),
    );
    add_nt(
        &mut r,
        "TROOPS",
        nt(
            "TROOPS",
            class::OTHER,
            5,
            "_GVProc",
            &[("SMLARMS.JT", 32767)],
        ),
    );
    // Ships have a damaged `_A` shape.
    for stem in ["KIEV", "KRIVAK", "CARGO"] {
        r.insert(format!("{stem}_A.SH"), shape.clone());
    }
    for stem in ["BNK5", "~BNK5", "STORE", "DEST"] {
        r.insert(format!("{stem}.OT"), ot(stem, class::STRUCTURE, 250));
        r.insert(format!("{stem}.SH"), shape.clone());
    }
    // The base layout: an enemy SA-6 (`nationality3`), a friendly ZSU-23
    // (`nationality3`), an enemy store (`nationality2`), an enemy supply
    // truck and a store with no owner.
    let place = |ty: &str, dx: i32, owner: &str| {
        format!(
            "obj\n\ttype {ty}\n\tpos {} 0 {}\n\tangle 0 0 0\n{owner}\tflags $13\n\t.\n",
            MIDDLE + dx,
            MIDDLE
        )
    };
    let layout = String::from("textFormat\nmap UKR.T2\nlayer CLEAR.LAY 0\ntime 12 0\n")
        + &place("SA6.NT", 0, "\tnationality3 152\n")
        + &place("ZSU23.NT", 500, "\tnationality3 39\n")
        + &place("STORE.OT", 1000, "\tnationality2 137\n")
        + &place("MISTRK.NT", 1500, "\tnationality3 152\n")
        + &place("STORE.OT", 2000, "");
    r.insert("UKR.MM".into(), layout.into_bytes());
    r.insert("~QUCITY.M".into(), test_template().into_bytes());
    r.insert("~QUSFLT.M".into(), fleet_template().into_bytes());
    r
}

/// One template object, at `dx` feet east of the middle.
fn object(ty: &str, dx: i32, owner: &str, flags: &str, extra: &str) -> String {
    format!(
        "obj\n\ttype {ty}\n\tpos {} 0 {}\n\tangle 90 0 0\n\t{owner}\n\tflags {flags}\n\tspeed 0\n\talias {}\n{extra}\t.\n",
        MIDDLE + dx,
        MIDDLE + 5000,
        dx / 100 + 1
    )
}

/// A defended site: 6 `<sam>` and 6 `<aaa>` slots (the first `<sam>` a
/// target), a bunker target (ordinal 12), a tank, a supply truck, a friendly
/// `nationality3` ZSU-23 (15) and a parked aircraft target (16).
fn test_template() -> String {
    let mut text = String::new();
    let mut dx = 0;
    let mut next = || {
        dx += 300;
        dx
    };
    text += &object("<sam>", next(), "nationality2 137", "$93", "\tskill 2\n");
    for _ in 0..5 {
        text += &object("<sam>", next(), "nationality2 137", "$13", "");
    }
    for _ in 0..6 {
        text += &object("<aaa>", next(), "nationality2 137", "$13", "\tskill 3\n");
    }
    text += &object("BNK5.OT", next(), "nationality2 137", "$93", "");
    text += &object("<tank>", next(), "nationality 0", "$13", "");
    text += &object("TRUCK.NT", next(), "nationality2 137", "$13", "");
    text += &object("ZSU23.NT", next(), "nationality3 39", "$13", "");
    text += &object("F18.PT", next(), "nationality2 137", "$97", "");
    text
}

/// A fleet with a carrier target and an aircraft, which is a deck launch.
fn fleet_template() -> String {
    object("KIEV.NT", 0, "nationality2 137", "$93", "")
        + &object("<destroyer>", 2000, "nationality2 137", "$13", "")
        + &object(
            "F18.PT",
            400,
            "nationality2 137",
            "$13",
            "\tstartTime 3600\n",
        )
}

pub(super) fn target(stem: &str, aaa: usize, sam: usize, seed: u32) -> GroundTarget {
    GroundTarget {
        stem: stem.into(),
        aaa,
        sam,
        seed,
        // Russian: equipment group 2.
        enemy_nationality: 10,
        night_stealth: false,
    }
}

fn resolve_with(
    r: &BTreeMap<String, Vec<u8>>,
    target: &GroundTarget,
) -> resolve::TemplateResolution {
    let name = format!("~{}.M", target.stem);
    let template = Template::parse(&name, &r[&name]).unwrap();
    resolve::template(&template, target, &mut Catalog::new(r), Some("UKR.T2")).unwrap()
}

/// The units that filled a `which` placeholder, by ordinal and type.
fn slots(resolved: &resolve::TemplateResolution, which: Placeholder) -> Vec<(u32, String)> {
    resolved
        .units
        .iter()
        .filter_map(|unit| match &unit.origin {
            Origin::Template {
                ordinal,
                placeholder: Some(p),
            } if *p == which => Some((*ordinal, unit.resource.clone())),
            _ => None,
        })
        .collect()
}

fn defenses(resolved: &resolve::TemplateResolution) -> usize {
    slots(resolved, Placeholder::Sam).len() + slots(resolved, Placeholder::Aaa).len()
}

#[test]
fn defense_rolls_man_slots_at_the_retail_percentages() {
    let r = surface_resources();
    for (level, percent) in [(0, 0.), (1, 25.), (2, 60.), (3, 100.)] {
        let (mut manned, mut seen) = (0usize, 0usize);
        for seed in 0..400 {
            let resolved = resolve_with(&r, &target("QUCITY", level, level, seed));
            manned += defenses(&resolved);
            seen += 12;
            // Every slot is either placed or listed as removed.
            assert_eq!(defenses(&resolved) + resolved.site.removed.len(), 12);
        }
        let share = 100. * manned as f64 / seen as f64;
        if level == 0 || level == 3 {
            assert_eq!(share, percent, "level {level}");
        } else {
            // 4,800 slots: three standard deviations is under 2.2 points.
            assert!((share - percent).abs() < 2.5, "level {level}: {share:.1}%");
        }
    }
}

#[test]
fn sam_and_aaa_levels_roll_separately() {
    let r = surface_resources();
    let resolved = resolve_with(&r, &target("QUCITY", 3, 0, 7));
    assert_eq!(slots(&resolved, Placeholder::Aaa).len(), 6);
    assert!(slots(&resolved, Placeholder::Sam).is_empty());
    let resolved = resolve_with(&r, &target("QUCITY", 0, 3, 7));
    assert!(slots(&resolved, Placeholder::Aaa).is_empty());
    assert_eq!(slots(&resolved, Placeholder::Sam).len(), 6);
}

#[test]
fn picks_come_uniformly_from_the_enemy_groups_lists() {
    let r = surface_resources();
    // Russian, French, Islamic Egyptian, Turkish, American: groups 2, 1, 3,
    // 4 and 0.
    for (nationality, group) in [(10, 2), (3, 1), (14, 3), (41, 4), (0, 0)] {
        let mut seen: BTreeMap<Placeholder, BTreeMap<String, usize>> = BTreeMap::new();
        for seed in 0..300 {
            let mut t = target("QUCITY", 3, 3, seed);
            t.enemy_nationality = nationality;
            let resolved = resolve_with(&r, &t);
            assert_eq!(resolved.site.group, group);
            for which in [Placeholder::Sam, Placeholder::Aaa, Placeholder::Tank] {
                for (_, resource) in slots(&resolved, which) {
                    *seen.entry(which).or_default().entry(resource).or_default() += 1;
                }
            }
        }
        for (which, counts) in &seen {
            let list: BTreeSet<String> = tables::equipment(*which, group)
                .unwrap()
                .iter()
                .map(|s| s.to_string())
                .collect();
            // Every list entry turns up, nothing else does, and roughly as
            // often as the others.
            assert_eq!(
                counts.keys().cloned().collect::<BTreeSet<_>>(),
                list,
                "{which:?} for group {group}"
            );
            let total: usize = counts.values().sum();
            let even = total as f64 / list.len() as f64;
            for (name, n) in counts {
                assert!(
                    (*n as f64 - even).abs() < even * 0.3,
                    "{name}: {n} of {total}"
                );
            }
        }
    }
}

#[test]
fn the_night_rule_mans_every_aaa_slot_with_a_novice_zsu23() {
    let r = surface_resources();
    let mut t = target("QUCITY", 3, 3, 11);
    t.night_stealth = true;
    let resolved = resolve_with(&r, &t);
    let aaa = slots(&resolved, Placeholder::Aaa);
    assert_eq!(aaa.len(), 6);
    for (ordinal, resource) in aaa {
        assert_eq!(resource, "ZSU23.NT");
        let unit = resolved
            .units
            .iter()
            .find(|u| u.id == UnitId::template(ordinal).unwrap())
            .unwrap();
        // The template wrote skill 3; the rule writes novice.
        assert_eq!(unit.skill, 0);
    }
    // SAM slots keep their picks and skills.
    let sams = slots(&resolved, Placeholder::Sam);
    assert!(sams.iter().any(|(_, r)| r != "ZSU23.NT"));
    assert_eq!(resolved.units[0].skill, 2);
    // Dormant today: no F-117 or B-2 is imported, so no spec triggers it.
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.condition = Condition::Night;
    assert!(!resolve::night_stealth(&spec));
}

#[test]
fn a_target_placeholder_that_fails_its_roll_is_no_target() {
    let r = surface_resources();
    let targets = |resolved: &resolve::TemplateResolution| -> BTreeSet<u32> {
        resolved
            .units
            .iter()
            .filter(|u| u.is_target())
            .map(|u| u.id.0)
            .chain(resolved.parked.iter().filter(|p| p.target).map(|p| p.id.0))
            .collect()
    };
    let none = resolve_with(&r, &target("QUCITY", 0, 0, 3));
    let heavy = resolve_with(&r, &target("QUCITY", 3, 3, 3));
    // The SAM slot target (ordinal 0), the bunker (12) and the parked
    // aircraft (16) at heavy; without SAMs only the last two.
    assert_eq!(
        targets(&heavy),
        BTreeSet::from([
            SURFACE_UNIT_BASE,
            SURFACE_UNIT_BASE + 12,
            SURFACE_UNIT_BASE + 16
        ])
    );
    assert_eq!(
        targets(&none),
        BTreeSet::from([SURFACE_UNIT_BASE + 12, SURFACE_UNIT_BASE + 16])
    );
    assert!(none.site.removed.contains(&0));
}

#[test]
fn ids_follow_template_ordinals_whatever_the_rolls() {
    let r = surface_resources();
    let heavy = resolve_with(&r, &target("QUCITY", 3, 3, 21));
    let heavy_ids: BTreeSet<u32> = heavy.units.iter().map(|u| u.id.0).collect();
    for seed in 0..50 {
        for level in 0..4 {
            let resolved = resolve_with(&r, &target("QUCITY", level, level, seed));
            for unit in &resolved.units {
                let Origin::Template { ordinal, .. } = unit.origin else {
                    panic!("a template unit from elsewhere");
                };
                assert_eq!(unit.id, UnitId::template(ordinal).unwrap());
                assert_eq!(unit.id.range(), IdRange::Template);
                assert!(heavy_ids.contains(&unit.id.0));
            }
            // A removed slot leaves its id unused; the named objects keep
            // theirs at every level.
            for ordinal in [12u32, 13, 14, 15] {
                assert!(
                    resolved
                        .units
                        .iter()
                        .any(|u| u.id.0 == SURFACE_UNIT_BASE + ordinal)
                );
            }
        }
    }
    // The same inputs give the same resolution.
    assert_eq!(
        resolve_with(&r, &target("QUCITY", 2, 1, 5)),
        resolve_with(&r, &target("QUCITY", 2, 1, 5))
    );
}

#[test]
fn owners_are_rewritten_except_nationality3() {
    let r = surface_resources();
    let resolved = resolve_with(&r, &target("QUCITY", 3, 3, 1));
    for unit in &resolved.units {
        if unit.id.0 == SURFACE_UNIT_BASE + 15 {
            // `nationality3 39` passes through: friendly Taiwanese.
            assert_eq!(unit.nationality, Some(39));
            assert_eq!(unit.side, FRIENDLY_SIDE);
        } else {
            // `nationality` and `nationality2` become the enemy, Redfor.
            assert_eq!(unit.nationality, Some(10 | 0x80), "{}", unit.resource);
            assert_eq!(unit.side, ENEMY_SIDE);
        }
    }
    assert_eq!(resolved.parked.len(), 1);
    assert_eq!(resolved.parked[0].side, ENEMY_SIDE);
    assert_eq!(resolved.parked[0].resource, "F18.PT");
}

#[test]
fn fleet_aircraft_are_deck_launches_and_left_out() {
    let r = surface_resources();
    let resolved = resolve_with(&r, &target("QUSFLT", 3, 3, 1));
    assert!(resolved.parked.is_empty());
    assert_eq!(resolved.site.left_out.len(), 1);
    assert_eq!(resolved.site.left_out[0].resource, "F18.PT");
    let kiev = &resolved.units[0];
    assert!(kiev.is_target());
    assert_eq!(kiev.look, DestroyedLook::DamagedShape("KIEV_A.SH".into()));
}

#[test]
fn destroyed_looks_follow_the_unit_class() {
    let r = surface_resources();
    let mut catalog = Catalog::new(&r);
    let mut look = |name: &str| catalog.entry(name).unwrap().look.clone();
    assert_eq!(look("SA6.NT"), DestroyedLook::Wreck("DEST.OT".into()));
    assert_eq!(
        look("KRIVAK.NT"),
        DestroyedLook::DamagedShape("KRIVAK_A.SH".into())
    );
    assert_eq!(look("TROOPS.NT"), DestroyedLook::Vanish);
    assert_eq!(look("GCI.NT"), DestroyedLook::Removed);
    assert_eq!(
        look("BNK5.OT"),
        DestroyedLook::DamagedObject("~BNK5.OT".into())
    );
    assert_eq!(look("STORE.OT"), DestroyedLook::Removed);
    let sa6 = catalog.entry("SA6.NT").unwrap();
    assert_eq!((sa6.explosion, sa6.crater), (Some(21), Some(6)));
    assert!(catalog.entry("MISTRK.NT").unwrap().supply_truck);
    assert!(catalog.entry("TRUCK.NT").unwrap().supply_truck);
    assert!(!catalog.entry("TANKER.NT").unwrap().supply_truck);
}

#[test]
fn base_layout_units_keep_their_layout_ids_and_take_their_sides() {
    let r = surface_resources();
    let terrain = Terrain::for_mission(&r, THEATER, Some(0), &Overrides::default()).unwrap();
    let surface = &terrain.surface;
    let ids: Vec<u32> = surface.units.iter().map(|u| u.id.0).collect();
    // The SA-6, the ZSU-23 and the truck: layout ordinals 0, 1 and 3.
    assert_eq!(
        ids,
        [
            LAYOUT_OBJECT_BASE,
            LAYOUT_OBJECT_BASE + 1,
            LAYOUT_OBJECT_BASE + 3
        ]
    );
    assert_eq!(surface.units[0].side, ENEMY_SIDE);
    assert_eq!(surface.units[0].kind, UnitKind::Active);
    assert_eq!(surface.units[1].side, FRIENDLY_SIDE);
    assert_eq!(surface.units[2].kind, UnitKind::Passive);
    assert!(surface.units.iter().all(|u| u.in_scene && u.skill == 1));
    assert_eq!(surface.trucks.len(), 1);
    // Every placement keeps its scene id; the owned store is enemy, the
    // ownerless one neutral.
    let scene: Vec<u32> = terrain.airport_scene.objects.iter().map(|o| o.id).collect();
    assert_eq!(
        scene,
        (0..5).map(|n| LAYOUT_OBJECT_BASE + n).collect::<Vec<_>>()
    );
    assert_eq!(surface.side_of(LAYOUT_OBJECT_BASE + 2), ENEMY_SIDE);
    assert_eq!(
        surface.side_of(LAYOUT_OBJECT_BASE + 4),
        tore_sim::combat::live::NO_SIDE
    );
    assert!(surface.template.is_none());
}

/// A mission whose spec names `target`, built as the creator's would be.
pub(super) fn world_with_target(r: &BTreeMap<String, Vec<u8>>, target: &GroundTarget) -> World {
    use crate::mission::Defense;
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.ground_target = Some(target.stem.clone());
    spec.aaa = Defense::from_level(target.aaa).unwrap();
    spec.sam = Defense::from_level(target.sam).unwrap();
    spec.surface_seed = target.seed;
    spec.enemy_nationality = target.enemy_nationality as u8;
    assert_eq!(GroundTarget::from_spec(&spec).as_ref(), Some(target));
    World::new(&spec, r, Seating::SinglePlayer).unwrap()
}

fn side_in_combat(world: &World, id: u32) -> Option<tore_sim::combat::live::Side> {
    world
        .combat
        .state
        .targets
        .iter()
        .find(|t| t.id == id)
        .map(|t| t.side)
}

#[test]
fn combat_registers_every_unit_with_its_side() {
    let r = surface_resources();
    // The build registers the base layout's units with their sides.
    let spec = MissionSpec::new(THEATER, AircraftId::F18);
    let world = World::new(&spec, &r, Seating::SinglePlayer).unwrap();
    assert_eq!(side_in_combat(&world, LAYOUT_OBJECT_BASE), Some(ENEMY_SIDE));
    assert_eq!(
        side_in_combat(&world, LAYOUT_OBJECT_BASE + 1),
        Some(FRIENDLY_SIDE)
    );
    assert_eq!(
        side_in_combat(&world, LAYOUT_OBJECT_BASE + 4),
        Some(tore_sim::combat::live::NO_SIDE)
    );
    // With a ground target, its units join with theirs, and the parked
    // aircraft stays out of the scene until its own slice.
    let mut world = world_with_target(&r, &target("QUCITY", 3, 3, 9));
    let surface = world.terrain.surface.clone();
    for unit in surface.template_units() {
        assert!(unit.in_scene);
        assert_eq!(
            side_in_combat(&world, unit.id.0),
            Some(unit.side),
            "{}",
            unit.resource
        );
    }
    assert_eq!(side_in_combat(&world, SURFACE_UNIT_BASE + 16), None);
    // Units explode with their own record's look; base-layout buildings keep
    // the fitted ground-object one.
    let look = |id| {
        world
            .combat
            .state
            .ground_look(id)
            .map(|l| (l.explosion, l.crater))
    };
    assert_eq!(look(SURFACE_UNIT_BASE), Some((21, 6)));
    assert_eq!(look(SURFACE_UNIT_BASE + 12), Some((0, 0)));
    assert_eq!(look(LAYOUT_OBJECT_BASE), Some((21, 6)));
    assert_eq!(look(LAYOUT_OBJECT_BASE + 2), None);
    assert_eq!(surface.targets().count(), 3);
    assert_eq!(world.combat.surface.units.len(), surface.units.len());
    // A restart keeps the sides.
    world.combat.reset(&mut world.cockpits[0].flight).unwrap();
    assert_eq!(side_in_combat(&world, SURFACE_UNIT_BASE), Some(ENEMY_SIDE));
}

#[test]
fn unit_hit_points_survive_a_checkpoint() {
    let r = surface_resources();
    let t = target("QUCITY", 3, 3, 4);
    let mut world = world_with_target(&r, &t);
    let damaged = [(SURFACE_UNIT_BASE + 12, 17), (LAYOUT_OBJECT_BASE, 0)];
    for (id, hp) in damaged {
        world
            .combat
            .state
            .targets
            .iter_mut()
            .find(|t| t.id == id)
            .unwrap()
            .hp = hp;
    }
    let bytes = world.checkpoint().unwrap();
    let mut fresh = world_with_target(&r, &t);
    fresh.restore(&bytes).unwrap();
    for (id, hp) in damaged {
        let row = fresh
            .combat
            .state
            .targets
            .iter()
            .find(|t| t.id == id)
            .unwrap();
        assert_eq!(row.hp, hp);
    }
    assert_eq!(fresh.combat.surface, world.combat.surface);
    assert_eq!(
        side_in_combat(&fresh, SURFACE_UNIT_BASE + 12),
        Some(ENEMY_SIDE)
    );
    // A world with another surface refuses it.
    let mut other = world_with_target(&r, &target("QUCITY", 1, 1, 4));
    assert!(other.restore(&bytes).is_err());
}

#[test]
fn the_digest_repeats_and_tells_surfaces_apart() {
    let r = surface_resources();
    let build = |t: &GroundTarget| {
        Terrain::for_mission_with(&r, THEATER, Some(0), &Overrides::default(), Some(t))
            .unwrap()
            .surface
    };
    let a = build(&target("QUCITY", 2, 2, 100));
    let b = build(&target("QUCITY", 2, 2, 100));
    assert_eq!(a.digest(), b.digest());
    assert_eq!(a, b);
    // Other rolls, another digest; so for a moved unit or a changed side.
    let other = (0..20)
        .map(|seed| build(&target("QUCITY", 2, 2, seed)))
        .find(|s| s.units != a.units)
        .unwrap();
    assert_ne!(other.digest(), a.digest());
    let mut moved = a.clone();
    moved.units.last_mut().unwrap().position[0] += 1;
    assert_ne!(moved.digest(), a.digest());
    let mut turned = a.clone();
    turned.units[0].side = FRIENDLY_SIDE;
    assert_ne!(turned.digest(), a.digest());
    // No target: the base layout alone.
    let base = Terrain::for_mission(&r, THEATER, Some(0), &Overrides::default())
        .unwrap()
        .surface;
    assert_ne!(base.digest(), a.digest());
}

#[test]
fn streams_are_independent_per_object_and_purpose() {
    let draw = |seed, stem, ordinal, purpose| Stream::new(seed, stem, ordinal, purpose).next_u64();
    let base = draw(1, "QUCOL", 3, Purpose::Roll);
    assert_eq!(base, draw(1, "QUCOL", 3, Purpose::Roll));
    assert_eq!(base, draw(1, "qucol", 3, Purpose::Roll));
    for other in [
        draw(2, "QUCOL", 3, Purpose::Roll),
        draw(1, "QUCIT", 3, Purpose::Roll),
        draw(1, "QUCOL", 4, Purpose::Roll),
        draw(1, "QUCOL", 3, Purpose::Pick),
    ] {
        assert_ne!(base, other);
    }
    let mut stream = Stream::new(9, "QX", 0, Purpose::Pick);
    assert!((0..1000).all(|_| stream.below(7) < 7));
    // Pinned: the draws are part of the network contract.
    assert_eq!(
        Stream::new(1234567, "QUCOL", 0, Purpose::Roll).next_u64(),
        PINNED_DRAW
    );
}

/// The first `Roll` draw for seed 1234567, `QUCOL`, object 0.
const PINNED_DRAW: u64 = 12_584_977_789_727_618_140;

#[test]
fn reserved_ranges_do_not_overlap() {
    assert_eq!(UnitId::layout(5).unwrap().range(), IdRange::Layout);
    assert_eq!(UnitId::template(5).unwrap().0, 0x5000_0005);
    assert_eq!(UnitId::supply_truck(0).unwrap().0, 0x5800_0000);
    assert_eq!(
        UnitId::supply_truck(0).unwrap().range(),
        IdRange::SupplyTruck
    );
    assert_eq!(UnitId::battery_radar(2).unwrap().0, 0x5C00_0002);
    assert_eq!(
        UnitId::battery_radar(2).unwrap().range(),
        IdRange::BatteryRadar
    );
    assert!(UnitId::template(0x0800_0000).is_none());
    assert!(UnitId::layout(0x1000_0000).is_none());
    assert_eq!(UnitId(SURFACE_PROJECTILE_ID_BASE).range(), IdRange::Other);
    // Client cosmetic rounds and AI shots stay outside every surface range.
    for id in [0xE000_0000, 0xF000_0000, 1 << 24] {
        assert_eq!(UnitId(id).range(), IdRange::Other);
    }
}
