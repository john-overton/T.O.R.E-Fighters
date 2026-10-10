//! The layout rules on a synthetic grid with synthetic units: jitter,
//! fallbacks, anchoring, relocation, starts, batteries, trucks and the
//! digest (plan 9.1, L1). No retail data. World-level checks (the player's
//! start, the automatic runway) are in `surface::tests`.
use super::{sites::Rule, *};
use crate::ai_wings::{ENEMY_SIDE, FRIENDLY_SIDE};
use crate::surface::{
    BATTERY_RADAR_BASE, Battery, BatterySystem, DestroyedLook, IdRange, LeftOut, SUPPLY_TRUCK_BASE,
    SupplyTruck, TemplateSite, UnitKind,
    catalog::{Entry, Family},
    resolve::GroundTarget,
};
use std::collections::BTreeMap;
use tore_formats::{quick_template::Placeholder, surface_unit::class, theater::TerrainCell};
use tore_sim::combat::live::Side;

/// A square grid, `size` cells a side.
struct Grid {
    size: usize,
    cells: Vec<TerrainCell>,
}

impl Grid {
    fn uniform(size: usize, class: u8, elevation: u8) -> Self {
        Self {
            size,
            cells: vec![
                TerrainCell {
                    color: 0,
                    class,
                    elevation,
                };
                size * size
            ],
        }
    }
    fn land(size: usize) -> Self {
        Self::uniform(size, 2, 1)
    }
    fn set(&mut self, col: usize, row: usize, class: u8) {
        self.cells[row * self.size + col].class = class;
    }
}

impl Cells for Grid {
    fn cols(&self) -> usize {
        self.size
    }
    fn rows(&self) -> usize {
        self.size
    }
    fn cell(&self, col: usize, row: usize) -> TerrainCell {
        self.cells[row * self.size + col]
    }
}

/// The middle of a 128-cell grid, feet.
const MID: i32 = 64 * 8192;

/// A unit of `resource` at `x`, `z` feet, with the kind and slot its class
/// implies.
fn unit(id: UnitId, resource: &str, class: u16, at: [i32; 2], side: Side) -> Unit {
    let kind = if resource.ends_with(".OT") {
        UnitKind::Structure
    } else {
        UnitKind::Active
    };
    let placeholder = match class {
        class::SAM => Some(Placeholder::Sam),
        class::AAA => Some(Placeholder::Aaa),
        _ => None,
    };
    let origin = match id.range() {
        IdRange::Template => Origin::Template {
            ordinal: id.0 - crate::surface::SURFACE_UNIT_BASE,
            placeholder,
        },
        _ => Origin::Layout {
            ordinal: id.0 - crate::surface::LAYOUT_OBJECT_BASE,
        },
    };
    Unit {
        id,
        origin,
        resource: resource.into(),
        kind,
        class,
        name: resource.into(),
        nationality: Some(if side == ENEMY_SIDE { 0x8a } else { 0 }),
        side,
        position: [at[0], 0, at[1]],
        angles: [90, 0, 0],
        flags: 0x13,
        skill: 1,
        react: None,
        search_dist: None,
        start_time: None,
        route: None,
        hit_points: 100,
        look: DestroyedLook::Removed,
        explosion: None,
        crater: None,
        supply_truck: false,
        in_scene: true,
    }
}

fn t(ordinal: u32) -> UnitId {
    UnitId::template(ordinal).unwrap()
}

fn l(ordinal: u32) -> UnitId {
    UnitId::layout(ordinal).unwrap()
}

fn target(seed: u32, variation: Variation) -> GroundTarget {
    GroundTarget {
        stem: "QTEST".into(),
        aaa: 3,
        sam: 3,
        seed,
        enemy_nationality: 10,
        night_stealth: false,
        variation,
        separation_nm: 20,
    }
}

const JITTER_ONLY: Variation = Variation {
    jitter: true,
    relocate: false,
};

/// A surface holding `units`, with a template site when `settings` is
/// given, as resolution would leave it.
fn surface(units: Vec<Unit>, settings: Option<GroundTarget>) -> Surface {
    let mut units = units;
    units.sort_by_key(|u| u.id);
    let placement = |u: &Unit| tore_formats::mission::Placement {
        key: tore_formats::mission::SourceKey {
            layout: "~QTEST.M".into(),
            ordinal: u.id.0,
        },
        section: None,
        object_type: u.resource.clone(),
        position: u.position,
        angles: u.angles,
        source_nationality: u.nationality,
        nationality2: false,
        nationality3: true,
        nationality: u.nationality,
        flags: Some(u.flags),
        speed: Some(0),
        name: None,
        alias: None,
        unknown: Vec::new(),
    };
    Surface {
        object_sides: units.iter().map(|u| (u.id.0, u.side)).collect(),
        placements: units
            .iter()
            .filter(|u| u.id.range() == IdRange::Template)
            .map(|u| (u.id, placement(u)))
            .collect(),
        template: settings.map(|settings| TemplateSite {
            stem: settings.stem.clone(),
            settings,
            group: 2,
            quickpos: None,
            objects: 0,
            removed: Vec::new(),
            left_out: Vec::<LeftOut>::new(),
            anchor: None,
        }),
        units,
        ..Surface::default()
    }
}

/// Every type is a 40 by 60 ft box, except strips (400 by 6,000 ft along
/// their heading) and the SA-2 site (800 ft square).
fn types(name: &str) -> TypeInfo {
    let square = |half: i64| Footprint {
        min: [-half, -half],
        max: [half, half],
    };
    TypeInfo {
        footprint: if name.starts_with("STRIP") {
            Footprint {
                min: [-200, 0],
                max: [200, 6_000],
            }
        } else if name == "SA2A.NT" {
            square(400)
        } else {
            Footprint {
                min: [-20, -30],
                max: [20, 30],
            }
        },
        strip: name.starts_with("STRIP"),
        bridge_or_road: name.starts_with("BRD"),
    }
}

fn entry(resource: &str) -> Arc<Entry> {
    let (class, truck) = match resource {
        "GCI.NT" => (class::STRUCTURE, false),
        "MISTRK.NT" | "TRUCK.NT" => (class::VEHICLE, true),
        _ => (class::VEHICLE, false),
    };
    Arc::new(Entry {
        resource: resource.into(),
        family: Family::Unit(UnitKind::Passive),
        class,
        name: resource.into(),
        hit_points: 50,
        look: DestroyedLook::Removed,
        explosion: Some(21),
        crater: Some(6),
        supply_truck: truck,
        carrier: false,
        unit: None,
        aircraft: None,
    })
}

/// Lays `surface` out on `ground`; every added type is available but
/// `missing`.
fn lay(surface: &mut Surface, ground: &Ground<'_>, missing: &[&str]) {
    let mut types = types;
    let mut added = |name: &str| (!missing.contains(&name)).then(|| entry(name));
    place(
        surface,
        &mut Inputs {
            ground,
            types: &mut types,
            added: &mut added,
            layout: "TEST.MM",
        },
    );
}

fn at(unit: &Unit) -> [i64; 2] {
    xz(unit.position)
}

fn in_band(d2: i64, [lo, hi]: [i64; 2]) -> bool {
    d2 >= (lo - 1).pow(2) && d2 <= (hi + 1).pow(2)
}

/// A defended site at the middle of the map: four SAM slots (the first a
/// target), two AAA slots, a bunker target, a tank.
fn site_units() -> Vec<Unit> {
    let mut units = vec![
        unit(t(0), "SA9.NT", class::SAM, [MID, MID], ENEMY_SIDE),
        unit(t(1), "SA9.NT", class::SAM, [MID + 3_000, MID], ENEMY_SIDE),
        unit(t(2), "SA9.NT", class::SAM, [MID, MID + 3_000], ENEMY_SIDE),
        unit(t(3), "SA9.NT", class::SAM, [MID - 3_000, MID], ENEMY_SIDE),
        unit(t(4), "ZSU23.NT", class::AAA, [MID, MID - 3_000], ENEMY_SIDE),
        unit(
            t(5),
            "ZSU23.NT",
            class::AAA,
            [MID + 2_000, MID + 2_000],
            ENEMY_SIDE,
        ),
        unit(
            t(6),
            "BNK5.OT",
            class::STRUCTURE,
            [MID + 500, MID + 500],
            ENEMY_SIDE,
        ),
        unit(
            t(7),
            "T72.NT",
            class::TANK,
            [MID - 1_000, MID - 1_000],
            ENEMY_SIDE,
        ),
    ];
    units[0].flags |= 0x80;
    units[6].flags |= 0x80;
    units
}

fn positions(surface: &Surface) -> BTreeMap<UnitId, ([i32; 3], i32)> {
    surface
        .units
        .iter()
        .map(|u| (u.id, (u.position, u.angles[0])))
        .collect()
}

#[test]
fn jitter_is_seeded_bounded_and_skips_structures() {
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let build = |seed| {
        let mut s = surface(site_units(), Some(target(seed, JITTER_ONLY)));
        lay(&mut s, &ground, &[]);
        s
    };
    let (a, b, c) = (build(7), build(7), build(8));
    assert_eq!(positions(&a), positions(&b));
    assert_eq!(a.digest(), b.digest());
    assert_ne!(positions(&a), positions(&c));
    assert_ne!(a.digest(), c.digest());
    let retail: BTreeMap<UnitId, Unit> = site_units().into_iter().map(|u| (u.id, u)).collect();
    let mut moved = 0;
    for unit in a.template_units() {
        let before = &retail[&unit.id];
        let d = distance2(at(unit), at(before));
        let turn = trig::signed(unit.angles[0] - before.angles[0]).abs();
        match Rule::of(unit) {
            Rule::Defense => {
                assert!(d <= DEFENSE_JITTER_FT.pow(2), "{:?}", unit.id);
                assert!(turn <= DEFENSE_JITTER_DEG);
            }
            Rule::Vehicle => {
                assert!(d <= VEHICLE_JITTER_FT.pow(2));
                assert!(turn <= VEHICLE_JITTER_DEG);
            }
            Rule::Fixed => assert_eq!((d, turn), (0, 0)),
            other => panic!("unexpected {other:?}"),
        }
        moved += usize::from(d > 0);
    }
    assert_eq!(moved, 7, "every unit but the bunker moves");
    // The scene placements follow their units.
    for (id, placement) in &a.placements {
        let unit = a.unit(*id).unwrap();
        assert_eq!(
            (placement.position, placement.angles),
            (unit.position, unit.angles)
        );
    }
    // With jitter off nothing moves.
    let mut off = surface(site_units(), Some(target(7, Variation::OFF)));
    lay(&mut off, &ground, &[]);
    for unit in off.template_units() {
        assert_eq!(unit.position, retail[&unit.id].position);
    }
}

#[test]
fn a_units_jitter_does_not_depend_on_the_others() {
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let mut all = surface(site_units(), Some(target(3, JITTER_ONLY)));
    lay(&mut all, &ground, &[]);
    // The same site with slot 2 lost to its roll, given in reverse order.
    let mut fewer: Vec<Unit> = site_units().into_iter().filter(|u| u.id != t(2)).collect();
    fewer.reverse();
    let mut fewer = surface(fewer, Some(target(3, JITTER_ONLY)));
    lay(&mut fewer, &ground, &[]);
    for unit in fewer.template_units() {
        let same = all.unit(unit.id).unwrap();
        assert_eq!((unit.position, unit.angles), (same.position, same.angles));
    }
}

#[test]
fn a_unit_with_nowhere_to_stand_keeps_its_retail_spot() {
    // A theater object covering everything around the site: no candidate
    // stands, so every unit stays put.
    let grid = Grid::land(128);
    let cover = Placed {
        id: l(0).0,
        at: [i64::from(MID), i64::from(MID)],
        heading: 0,
        footprint: Footprint {
            min: [-6_000, -6_000],
            max: [6_000, 6_000],
        },
    };
    let ground = Ground::new(&grid, vec![cover], Vec::new(), None);
    let mut s = surface(site_units(), Some(target(5, JITTER_ONLY)));
    lay(&mut s, &ground, &[]);
    let retail: BTreeMap<UnitId, Unit> = site_units().into_iter().map(|u| (u.id, u)).collect();
    for unit in s.template_units() {
        assert_eq!(unit.position, retail[&unit.id].position);
    }
    // A ship on land has nowhere to go either; at sea it moves.
    let ship = |grid: &Grid| {
        let ground = Ground::new(grid, Vec::new(), Vec::new(), None);
        let mut s = surface(
            vec![unit(t(0), "KRIVAK.NT", class::SHIP, [MID, MID], ENEMY_SIDE)],
            Some(target(5, Variation::ON)),
        );
        lay(&mut s, &ground, &[]);
        s.units[0].position
    };
    assert_eq!(ship(&Grid::land(128)), [MID, 0, MID]);
    assert_ne!(
        ship(&Grid::uniform(128, ground::WATER_CLASS, 0)),
        [MID, 0, MID]
    );
}

fn object(ordinal: u32, at: [i64; 2], target: bool) -> sites::Object {
    sites::Object {
        ordinal,
        item: sites::Item::Unit(ordinal as usize),
        retail: at,
        heading: 0,
        rule: Rule::Defense,
        info: TypeInfo::default(),
        target,
        routed: false,
    }
}

#[test]
fn templates_anchor_by_rule() {
    let grid = Grid::land(128);
    let mid = i64::from(MID);
    let runway = Placed {
        id: l(9).0,
        at: [mid + 5_000, mid],
        heading: 0,
        footprint: types("STRIP.OT").footprint,
    };
    let house = Placed {
        id: l(8).0,
        at: [mid - 20_000, mid],
        heading: 0,
        footprint: types("HOUSE.OT").footprint,
    };
    let open = Ground::new(&grid, vec![house], Vec::new(), None);
    let airfield = Ground::new(&grid, vec![house], vec![runway], None);
    let site = vec![
        object(0, [mid, mid], true),
        object(1, [mid + 900, mid], false),
    ];
    assert_eq!(sites::anchor(&site, &open), None);
    // Its centroid within 1 nm of a runway (the runway's box is 4,350 ft
    // away), or a strip piece of its own.
    assert_eq!(sites::anchor(&site, &airfield), Some(Anchor::Runway));
    let mut strip = site.clone();
    strip[1].info.strip = true;
    assert_eq!(sites::anchor(&strip, &open), Some(Anchor::Strip));
    let mut bridge = site.clone();
    bridge[1].info.bridge_or_road = true;
    assert_eq!(sites::anchor(&bridge, &open), Some(Anchor::BridgeOrRoad));
    // A target within 2,000 ft of a layout object; a defense there does
    // not count.
    let mut town = site.clone();
    town[0].retail = [mid - 18_500, mid];
    assert_eq!(sites::anchor(&town, &open), Some(Anchor::Town));
    let mut not_town = site.clone();
    not_town[1].retail = [mid - 18_500, mid];
    assert_eq!(sites::anchor(&not_town, &open), None);
    let mut routed = site;
    routed[1].routed = true;
    assert_eq!(sites::anchor(&routed, &airfield), Some(Anchor::Route));
}

#[test]
fn an_anchored_template_never_moves() {
    let grid = Grid::land(128);
    let mid = i64::from(MID);
    let runway = Placed {
        id: l(9).0,
        at: [mid + 5_000, mid],
        heading: 0,
        footprint: types("STRIP.OT").footprint,
    };
    let ground = Ground::new(&grid, Vec::new(), vec![runway], None);
    for seed in 0..10 {
        let mut s = surface(site_units(), Some(target(seed, Variation::ON)));
        lay(&mut s, &ground, &[]);
        assert_eq!(s.template.as_ref().unwrap().anchor, Some(Anchor::Runway));
        assert!(s.transform.is_identity());
        assert_eq!(at(s.unit(t(6)).unwrap()), [mid + 500, mid + 500]);
    }
}

#[test]
fn a_relocated_site_obeys_the_site_rules() {
    let grid = Grid::land(128);
    let mid = i64::from(MID);
    // Blue to the south, Red to the north; scattered theater objects.
    let front = Front {
        blue: [mid, mid - 200_000],
        red: [mid, mid + 200_000],
    };
    let objects: Vec<Placed> = (0..40)
        .map(|i| Placed {
            id: l(i).0,
            at: [
                mid - 300_000 + i64::from(i % 8) * 80_000,
                mid - 200_000 + i64::from(i / 8) * 90_000,
            ],
            heading: 0,
            footprint: types("HOUSE.OT").footprint,
        })
        .collect();
    let ground = Ground::new(&grid, objects, Vec::new(), Some(front));
    let mut relocated = 0;
    for seed in 0..20 {
        let mut s = surface(site_units(), Some(target(seed, Variation::ON)));
        lay(&mut s, &ground, &[]);
        let mut types = types;
        assert_eq!(
            audit(&s, &ground, &mut types),
            Vec::<String>::new(),
            "seed {seed}"
        );
        if s.transform.is_identity() {
            continue;
        }
        relocated += 1;
        let shift = s.transform.translation.map(i64::from);
        let d2 = shift[0].pow(2) + shift[1].pow(2);
        assert!(in_band(
            d2,
            [RELOCATE_MIN_NM * NM_FT, RELOCATE_MAX_NM * NM_FT]
        ));
        // The bunker moved exactly with the group and turned with it.
        let bunker = s.unit(t(6)).unwrap();
        assert_eq!(
            at(bunker),
            sites::apply(&s.transform, [mid + 500, mid + 500])
        );
        assert_eq!(
            bunker.angles[0],
            trig::signed(90 + s.transform.rotation_deg)
        );
        // Still on the retail spot's side of the front, within 15 nm of its
        // depth.
        let pivot = s.transform.pivot.map(i64::from);
        assert!(front.same_band(
            [pivot[0] + shift[0], pivot[1] + shift[1]],
            pivot,
            RELOCATE_DEPTH_NM * NM_FT
        ));
    }
    assert!(relocated >= 15, "{relocated} of 20 relocated");
}

#[test]
fn a_site_with_nowhere_to_go_stays_at_its_retail_spot() {
    // An island of three by three cells in an open sea: every candidate 3
    // to 30 nm away puts a target or the bunker in the water.
    let mut grid = Grid::uniform(128, ground::WATER_CLASS, 0);
    for c in 63..=65 {
        for r in 63..=65 {
            grid.set(c, r, 2);
        }
    }
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let mut s = surface(site_units(), Some(target(1, Variation::ON)));
    lay(&mut s, &ground, &[]);
    assert!(s.transform.is_identity());
    assert_eq!(s.template.as_ref().unwrap().anchor, None);
}

#[test]
fn relocation_never_changes_the_rolls_or_the_counts() {
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let ids = |variation| {
        let mut s = surface(site_units(), Some(target(11, variation)));
        lay(&mut s, &ground, &[]);
        let units: Vec<(UnitId, String)> =
            s.units.iter().map(|u| (u.id, u.resource.clone())).collect();
        (units, s.trucks.len(), s.batteries.len(), s.transform)
    };
    let (on, off) = (ids(Variation::ON), ids(Variation::OFF));
    assert!(!on.3.is_identity() && off.3.is_identity());
    assert_eq!((on.0, on.1, on.2), (off.0, off.1, off.2));
}

#[test]
fn red_defends_the_target_and_blue_starts_the_separation_away() {
    let grid = Grid::land(128);
    let mid = i64::from(MID);
    let front = Front {
        blue: [mid, mid - 200_000],
        red: [mid, mid + 200_000],
    };
    let strip = |ordinal: u32, dz: i64| Placed {
        id: l(ordinal).0,
        at: [mid, mid + dz],
        heading: 0,
        footprint: types("STRIP.OT").footprint,
    };
    let runways = vec![
        strip(1, -10 * NM_FT),
        strip(2, -20 * NM_FT),
        strip(3, -40 * NM_FT),
        strip(4, 25 * NM_FT),
    ];
    let ground = Ground::new(&grid, Vec::new(), runways, Some(front));
    let margin = START_MARGIN_CELLS * ground::CELL_FT;
    let gap = |a: [i32; 2], b: [i32; 2]| {
        let d = [i64::from(a[0] - b[0]), i64::from(a[1] - b[1])];
        d[0].pow(2) + d[1].pow(2)
    };
    let mut reds = std::collections::BTreeSet::new();
    for seed in 0..30 {
        let mut s = surface(site_units(), Some(target(seed, Variation::OFF)));
        s.object_sides.insert(l(2).0, FRIENDLY_SIDE);
        s.object_sides.insert(l(4).0, ENEMY_SIDE);
        lay(&mut s, &ground, &[]);
        let starts = s.starts.clone().expect("starts");
        // The targets' centroid: the SAM and the bunker.
        assert_eq!(starts.target, [MID + 250, MID + 250]);
        // Red within 5 nm of it, at a seeded spot.
        assert!(gap(starts.red, starts.target) <= (RED_START_NM * NM_FT).pow(2));
        reds.insert(starts.red);
        // Blue the separation (20 nm) from Red, toward Blue (south) within
        // the spread, heading at the target, on the map.
        assert!(in_band(
            gap(starts.blue, starts.red),
            [20 * NM_FT, 20 * NM_FT]
        ));
        let from_red = [
            i64::from(starts.blue[0] - starts.red[0]),
            i64::from(starts.blue[1] - starts.red[1]),
        ];
        let bearing = trig::bearing(from_red);
        assert!(
            trig::signed(bearing - 180).abs() <= BLUE_START_SPREAD_DEG,
            "{bearing}"
        );
        let to_target = [
            i64::from(starts.target[0] - starts.blue[0]),
            i64::from(starts.target[1] - starts.blue[1]),
        ];
        assert_eq!(starts.blue_heading_deg, trig::bearing(to_target));
        assert!(ground.inside(starts.blue.map(i64::from), margin));
        // Airfields 15 nm or more from the target, nearest first, by owner,
        // else by their side of the front.
        assert_eq!(starts.blue_airfields, [l(2).0, l(3).0]);
        assert_eq!(starts.red_airfields, [l(4).0]);
    }
    assert!(reds.len() > 20, "Red's spot varies with the seed");
    // A target near the south edge: the bearing turns until Blue fits.
    let near_edge = || {
        site_units()
            .into_iter()
            .map(|mut u| {
                u.position[2] -= MID - 40_000;
                u
            })
            .collect::<Vec<Unit>>()
    };
    let mut s = surface(near_edge(), Some(target(2, Variation::OFF)));
    lay(&mut s, &ground, &[]);
    let starts = s.starts.unwrap();
    assert!(ground.inside(starts.blue.map(i64::from), margin));
    assert!(ground.inside(starts.red.map(i64::from), margin));
    assert!(in_band(
        gap(starts.blue, starts.red),
        [20 * NM_FT, 20 * NM_FT]
    ));
    // A separation longer than the map: the farthest that fits.
    let mut far = target(2, Variation::OFF);
    far.separation_nm = 300;
    let mut s = surface(site_units(), Some(far));
    lay(&mut s, &ground, &[]);
    let starts = s.starts.unwrap();
    assert!(ground.inside(starts.blue.map(i64::from), margin));
    let d2 = gap(starts.blue, starts.red);
    assert!(
        d2 < (300 * NM_FT).pow(2) && d2 > (60 * NM_FT).pow(2),
        "{d2}"
    );
}

/// Launchers and radars for the battery tests: template SA-6s and SA-2s
/// about the middle, base-layout SA-3s with two GCIs to the west and a
/// friendly SA-6.
fn battery_units() -> Vec<Unit> {
    let sa6 = |o, dx, dz| unit(t(o), "SA6.NT", class::SAM, [MID + dx, MID + dz], ENEMY_SIDE);
    vec![
        // A chain of three, each within 1 nm of the next, then one far off.
        sa6(0, 0, 0),
        sa6(1, 5_000, 0),
        sa6(2, 10_000, 0),
        sa6(3, 60_000, 0),
        // Six in one cluster: two batteries of up to four.
        sa6(4, 0, 40_000),
        sa6(5, 1_000, 40_000),
        sa6(6, 2_000, 40_000),
        sa6(7, 0, 41_000),
        sa6(8, 1_000, 41_000),
        sa6(9, 2_000, 41_000),
        // An existing Straight Flush 1.5 nm from the chain's centroid.
        unit(
            t(10),
            "SFLUSH.NT",
            class::VEHICLE,
            [MID + 5_000, MID + 9_000],
            ENEMY_SIDE,
        ),
        // Two SA-2 sites near each other: one battery each.
        unit(
            t(11),
            "SA2A.NT",
            class::SAM,
            [MID, MID - 40_000],
            ENEMY_SIDE,
        ),
        unit(
            t(12),
            "SA2A.NT",
            class::SAM,
            [MID + 3_000, MID - 40_000],
            ENEMY_SIDE,
        ),
        // Base layout: SA-3s with a GCI 4 nm off (too far) and one 1 nm off;
        // a friendly SA-6 never joins an enemy battery.
        unit(l(0), "SA3.NT", class::SAM, [MID - 100_000, MID], ENEMY_SIDE),
        unit(l(1), "SA3.NT", class::SAM, [MID - 99_000, MID], ENEMY_SIDE),
        unit(
            l(2),
            "GCI.NT",
            class::STRUCTURE,
            [MID - 99_500, MID + 4 * 6_076],
            ENEMY_SIDE,
        ),
        unit(
            l(3),
            "GCI.NT",
            class::STRUCTURE,
            [MID - 99_500, MID + 6_076],
            ENEMY_SIDE,
        ),
        unit(
            l(4),
            "SA6.NT",
            class::SAM,
            [MID - 100_000, MID + 500],
            FRIENDLY_SIDE,
        ),
    ]
}

#[test]
fn launchers_form_batteries_with_adopted_or_added_radars() {
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let mut s = surface(battery_units(), Some(target(4, Variation::OFF)));
    lay(&mut s, &ground, &[]);
    let by_lead: BTreeMap<UnitId, &Battery> =
        s.batteries.iter().map(|b| (b.launchers[0], b)).collect();
    // Battery order: ascending lowest launcher, the layout's first.
    let leads: Vec<UnitId> = s.batteries.iter().map(|b| b.launchers[0]).collect();
    let mut sorted = leads.clone();
    sorted.sort();
    assert_eq!(leads, sorted);
    // Base layout SA-3s: one battery, adopting the near GCI with its id.
    let sa3 = by_lead[&l(0)];
    assert_eq!(
        (sa3.system, sa3.radar, sa3.radar_added),
        (BatterySystem::Sa3, l(3), false)
    );
    assert_eq!(sa3.launchers, [l(0), l(1)]);
    assert_eq!(sa3.truck, None, "base layouts get no trucks");
    // The friendly SA-6 fights alone, with an added radar of its side.
    let friendly = by_lead[&l(4)];
    assert_eq!((friendly.side, friendly.radar_added), (FRIENDLY_SIDE, true));
    // The chain joins and adopts the Straight Flush; the far one is alone.
    let chain = by_lead[&t(0)];
    assert_eq!(chain.launchers, [t(0), t(1), t(2)]);
    assert_eq!((chain.radar, chain.radar_added), (t(10), false));
    let alone = by_lead[&t(3)];
    assert_eq!(alone.launchers, [t(3)]);
    assert!(alone.radar_added);
    // Six in one cluster: two batteries of at most four, every launcher in
    // exactly one.
    let split: Vec<&Battery> = s
        .batteries
        .iter()
        .filter(|b| {
            b.launchers
                .iter()
                .any(|id| (4..10).map(t).any(|x| x == *id))
        })
        .collect();
    assert_eq!(split.len(), 2);
    assert!(split.iter().all(|b| b.launchers.len() <= 4));
    assert_eq!(split.iter().map(|b| b.launchers.len()).sum::<usize>(), 6);
    // SA-2: one site per battery.
    assert_eq!(by_lead[&t(11)].launchers, [t(11)]);
    assert_eq!(by_lead[&t(12)].launchers, [t(12)]);
    // Added radars: numbered in battery order, beside their launchers, of
    // their system's element, never targets, in the scene.
    let added: Vec<&Battery> = s.batteries.iter().filter(|b| b.radar_added).collect();
    assert_eq!(added.len(), 6);
    for (n, battery) in added.iter().enumerate() {
        assert_eq!(battery.radar.0, BATTERY_RADAR_BASE + n as u32);
        let radar = s.unit(battery.radar).expect("the radar is a unit");
        assert_eq!(radar.origin, Origin::Added);
        assert_eq!(radar.flags & 0x80, 0);
        assert_eq!(
            (radar.side, s.side_of(radar.id.0)),
            (battery.side, battery.side)
        );
        let centre = centroid(battery.launchers.iter().map(|id| at(s.unit(*id).unwrap()))).unwrap();
        let band = if battery.system == BatterySystem::Sa2 {
            SA2_RADAR_FT
        } else {
            BATTERY_RADAR_FT
        };
        assert!(in_band(distance2(at(radar), centre), band));
        assert_eq!(radar.resource, batteries::element(battery.system));
        assert!(s.placements.iter().any(|(id, _)| *id == radar.id));
    }
    assert_eq!(batteries::element(BatterySystem::Sa2), "GCI.NT");
    assert_eq!(batteries::element(BatterySystem::Sa6), "SFLUSH.NT");
    assert_eq!(
        batteries::element(BatterySystem::Hawk),
        crate::surface::catalog::HAWK_RADAR
    );
}

#[test]
fn a_battery_whose_radar_cannot_be_added_is_not_formed() {
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let mut s = surface(battery_units(), Some(target(4, Variation::OFF)));
    lay(&mut s, &ground, &["SFLUSH.NT"]);
    // The SA-6 battery that adopts stays; those needing an added Straight
    // Flush are left out, and say so.
    assert!(s.batteries.iter().any(|b| b.launchers[0] == t(0)));
    assert!(!s.batteries.iter().any(|b| b.launchers[0] == t(3)));
    assert!(s.layout_notes.iter().any(|n| n.contains("SFLUSH.NT")));
}

#[test]
fn every_manned_slot_and_battery_gets_a_truck_beside_it() {
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let mut units = site_units();
    units.push(unit(
        t(8),
        "SA6.NT",
        class::SAM,
        [MID + 20_000, MID],
        ENEMY_SIDE,
    ));
    // A named launcher, not a slot: only the battery's truck serves it.
    let mut named = unit(t(9), "SA6.NT", class::SAM, [MID + 21_000, MID], ENEMY_SIDE);
    named.origin = Origin::Template {
        ordinal: 9,
        placeholder: None,
    };
    units.push(named);
    let mut s = surface(units, Some(target(9, Variation::ON)));
    lay(&mut s, &ground, &[]);
    // Four SAM, two AAA and the SA-6 slot; one battery.
    assert_eq!(s.batteries.len(), 1);
    let added: Vec<&SupplyTruck> = s.trucks.iter().filter(|t| t.added).collect();
    assert_eq!(added.len(), 7 + 1);
    for (n, truck) in added.iter().enumerate() {
        assert_eq!(truck.id.0, SUPPLY_TRUCK_BASE + n as u32);
        let unit = s.unit(truck.id).unwrap();
        let served = s.unit(truck.serves.unwrap()).unwrap();
        assert!(unit.supply_truck && unit.flags & 0x80 == 0);
        assert_eq!(unit.side, served.side);
        let want = if served.class == class::AAA {
            AAA_TRUCK
        } else {
            SAM_TRUCK
        };
        assert_eq!(unit.resource, want);
        assert!(in_band(distance2(at(unit), at(served)), TRUCK_FT));
        assert!(s.placements.iter().any(|(id, _)| *id == unit.id));
    }
    // Numbered in the order of the units they serve.
    let served: Vec<UnitId> = added.iter().map(|t| t.serves.unwrap()).collect();
    let mut sorted = served.clone();
    sorted.sort();
    assert_eq!(served, sorted);
    // The battery's truck serves its launcher without a slot truck.
    let battery_truck = s.batteries[0].truck.unwrap();
    assert_eq!(
        added.iter().find(|t| t.id == battery_truck).unwrap().serves,
        Some(t(9))
    );
}

#[test]
fn base_layouts_get_no_trucks_and_lay_out_alike_in_every_mission() {
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let radar_of_friendly = |seed| {
        let mut s = surface(battery_units(), Some(target(seed, Variation::ON)));
        lay(&mut s, &ground, &[]);
        assert!(
            s.trucks
                .iter()
                .filter(|t| t.added)
                .all(|t| t.serves.unwrap().range() == IdRange::Template)
        );
        let battery = s.batteries.iter().find(|b| b.launchers[0] == l(4)).unwrap();
        s.unit(battery.radar).unwrap().position
    };
    assert_eq!(radar_of_friendly(1), radar_of_friendly(2));
}

#[test]
fn the_digest_repeats_and_follows_the_seed() {
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let build = |seed| {
        let mut s = surface(battery_units(), Some(target(seed, Variation::ON)));
        lay(&mut s, &ground, &[]);
        s.digest()
    };
    assert_eq!(build(21), build(21));
    assert_ne!(build(21), build(22));
}

#[test]
fn a_fleet_moves_as_one_in_a_template_that_stays() {
    let grid = Grid::uniform(128, ground::WATER_CLASS, 0);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let ships = || {
        vec![
            unit(t(0), "KIEV.NT", class::SHIP, [MID, MID], ENEMY_SIDE),
            unit(
                t(1),
                "KRIVAK.NT",
                class::SHIP,
                [MID + 3_000, MID],
                ENEMY_SIDE,
            ),
            unit(
                t(2),
                "KRIVAK.NT",
                class::SHIP,
                [MID - 3_000, MID],
                ENEMY_SIDE,
            ),
        ]
    };
    let mut s = surface(ships(), Some(target(6, JITTER_ONLY)));
    lay(&mut s, &ground, &[]);
    // Each ship within the fleet's shift and turn plus its own disc; the
    // escorts keep their 6,000 ft spacing within twice a ship's disc.
    let limit = FLEET_SHIFT_FT + 3_000 + SHIP_JITTER_FT;
    for (unit, before) in s.units.iter().zip(ships()) {
        assert!(distance2(at(unit), at(&before)) <= limit.pow(2));
    }
    let spacing = distance2(at(&s.units[1]), at(&s.units[2]));
    let slack = 2 * SHIP_JITTER_FT + 2;
    assert!(
        in_band(spacing, [6_000 - slack, 6_000 + slack]),
        "{spacing}"
    );
    // The flagship moved off its retail spot.
    assert_ne!(at(&s.units[0]), [i64::from(MID), i64::from(MID)]);
}

#[test]
fn a_routed_unit_neither_jitters_nor_relocates() {
    // Its legs are authored in the template's frame, so the unit and its
    // template stay exactly where retail put them (movement follows them).
    let grid = Grid::land(128);
    let ground = Ground::new(&grid, Vec::new(), Vec::new(), None);
    let mut units = site_units();
    units[7].route = Some(tore_formats::quick_template::Route {
        alias: 7,
        waypoints: Vec::new(),
    });
    for seed in 0..10 {
        let mut s = surface(units.clone(), Some(target(seed, Variation::ON)));
        lay(&mut s, &ground, &[]);
        assert_eq!(s.template.as_ref().unwrap().anchor, Some(Anchor::Route));
        assert!(s.transform.is_identity());
        let tank = s.unit(t(7)).unwrap();
        assert_eq!(
            (tank.position, tank.angles),
            (units[7].position, units[7].angles)
        );
    }
}
