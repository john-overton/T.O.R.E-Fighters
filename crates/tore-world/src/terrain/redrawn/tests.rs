use super::geometry::{contains, dot, overlap_area, signed_area};
use super::layout::{compose, designator};
use super::*;
use crate::terrain::{Overrides, Terrain};
use crate::test_support::resources::{AIRPORT_RUNWAY, airport_resources};

fn builtin() -> Vec<Plan> {
    Plans::builtin().unwrap()
}

fn single(plans: &[Plan], object_type: &str) -> Plan {
    plans
        .iter()
        .find(|p| p.pair.is_none() && p.applies_to == object_type)
        .unwrap()
        .clone()
}

fn on_pavement(patches: &[Patch], p: Point) -> bool {
    patches.iter().any(|patch| contains(&patch.poly, p))
}

fn assert_no_overlaps(name: &str, patches: &[Patch]) {
    for (i, a) in patches.iter().enumerate() {
        for b in &patches[i + 1..] {
            let overlap = overlap_area(&a.poly, &b.poly);
            assert!(overlap < 0.5, "{name}: {a:?} overlaps {b:?} by {overlap}");
        }
    }
}

#[test]
fn the_switch_off_applies_no_plan_and_the_default_is_on() {
    assert!(Plans::new(false).unwrap().is_empty());
    assert!(!Plans::new(true).unwrap().is_empty());
    assert!(Overrides::default().redrawn_airports);
}

#[test]
fn every_runway_type_and_pair_has_a_plan() {
    let plans = builtin();
    for object_type in [
        "STRIP.OT",
        "STRIP1.OT",
        "STRIP2.OT",
        "STRIP3.OT",
        "STRIP4.OT",
        "STRIP5.OT",
        "STRIP6.OT",
        "STRIP7.OT",
        "STRIP3A.OT",
        "STRIP5A.OT",
        "STRIP6A.OT",
        "STRIP7A.OT",
        "DTSTRP.OT",
    ] {
        single(&plans, object_type);
    }
    let pairs: Vec<_> = plans
        .iter()
        .filter_map(|p| p.pair.as_ref().map(|pair| pair.tile.clone()))
        .collect();
    assert_eq!(
        pairs,
        ["STRIP3A.OT", "STRIP5A.OT", "STRIP6A.OT", "STRIP7A.OT"]
    );
}

#[test]
fn every_plan_keeps_the_ai_on_pavement_and_never_overlaps() {
    let plans = builtin();
    let mut fillets = 0;
    for plan in plans.iter().filter(|p| p.pair.is_none()) {
        let name = &plan.applies_to;
        let (materials, patches) = compose(&[(plan, [0., 0.])], None, 0.);
        assert_no_overlaps(name, &patches);
        let fillet = materials.iter().position(|m| m.name == "fillet");
        fillets += patches
            .iter()
            .filter(|p| Some(p.material) == fillet)
            .count();
        let Some(a) = &plan.anchors else {
            continue;
        };
        for p in a
            .taxi_out
            .iter()
            .chain(&a.taxi_in)
            .chain(&a.parking)
            .chain([&a.takeoff, &a.landing])
        {
            assert!(on_pavement(&patches, *p), "{name}: {p:?} off the pavement");
        }
        // Every taxi leg runs on pavement, sampled every 10 ft.
        let legs = a
            .taxi_out
            .windows(2)
            .chain(a.taxi_in.windows(2))
            .map(|w| (w[0], w[1]))
            .chain([(a.taxi_out[3], a.takeoff)]);
        for (from, to) in legs {
            let steps = ((to[0] - from[0]).hypot(to[1] - from[1]) / 10.).ceil() as usize;
            for step in 0..=steps {
                let t = step as f64 / steps as f64;
                let p = [
                    from[0] + (to[0] - from[0]) * t,
                    from[1] + (to[1] - from[1]) * t,
                ];
                assert!(
                    on_pavement(&patches, p),
                    "{name}: {p:?} on a taxi leg is off the pavement"
                );
            }
        }
        // The first runway is the retail one at a real width.
        if plan.ils_runway {
            let main = &plan.runways[0];
            assert!(main.width <= 200., "{name}");
            assert!(on_pavement(&patches, [0., main.length - 1.]), "{name}");
        }
    }
    assert!(fillets > 20, "only {fillets} curved corners");
}

#[test]
fn pairs_join_without_overlaps() {
    let plans = builtin();
    // Measured offsets of each tile's frame in its base's (16 base layouts).
    for (base, tile, offset) in [
        ("STRIP3.OT", "STRIP3A.OT", [-896., 9670.]),
        ("STRIP5.OT", "STRIP5A.OT", [-8896., -628.]),
        ("STRIP6.OT", "STRIP6A.OT", [-8907., 693.]),
        ("STRIP7.OT", "STRIP7A.OT", [3452., 8899.]),
    ] {
        let links = plans
            .iter()
            .find(|p| p.pair.as_ref().is_some_and(|pair| pair.base == base))
            .unwrap();
        let (base, tile) = (single(&plans, base), single(&plans, tile));
        let (_, patches) = compose(&[(&base, [0., 0.]), (&tile, offset)], Some(links), 0.);
        assert_no_overlaps(&base.applies_to, &patches);
    }
}

#[test]
fn patch_cells_tile_each_patch_exactly() {
    let plans = builtin();
    let plan = single(&plans, "STRIP1.OT");
    let (materials, patches) = compose(&[(&plan, [0., 0.])], None, 0.);
    for patch in &patches {
        let cells = patch.cells();
        let area: f64 = cells.iter().map(|(p, ..)| signed_area(p).abs()).sum();
        let whole = signed_area(&patch.poly);
        assert!(
            (area - whole).abs() < 1e-3 * whole.max(1.),
            "{patch:?}: cells {area}"
        );
        for (_, fractions, _) in cells {
            for f in fractions {
                assert!((-1e-9..=1. + 1e-9).contains(&f[0]) && (-1e-9..=1. + 1e-9).contains(&f[1]));
            }
        }
    }
    // A designation digit is exactly one copy of its texels: "36" and "18".
    for digit in ["digit_3", "digit_6", "digit_1", "digit_8"] {
        let m = materials.iter().position(|m| m.name == digit).unwrap();
        let patch = patches.iter().find(|p| p.material == m).unwrap();
        assert_eq!(patch.cells().len(), 1, "{digit}");
    }
}

#[test]
fn designators_follow_the_world_heading() {
    assert_eq!(designator(0.), "36");
    assert_eq!(designator(180.), "18");
    assert_eq!(designator(-90.), "27");
    assert_eq!(designator(90.), "9");
    assert_eq!(designator(-30.), "33");
}

/// A plan for the synthetic airport (STRIP.OT, an 8,000 ft runway): one
/// taxiway, an apron and a building line the hut moves to.
const SYNTHETIC: &str = r#"
applies_to = "STRIP.OT"
runway_length_ft = 8000
grass_margin_ft = 100
[[material]]
name = "runway_plain"
pic = "R.PIC"
rect = [0, 0, 8, 8]
tile_ft = [0, 100]
[[runway]]
name = "R0"
from = [0, 0]
length = 8000
width = 150
pad = 200
[[taxiway]]
name = "T"
width = 75
points = [[600, -170], [600, 7750]]
[[taxiway]]
name = "S"
width = 75
points = [[0, -170], [600, -170]]
[[taxiway]]
name = "N"
width = 75
points = [[0, 7750], [600, 7750]]
[[apron]]
name = "ramp"
min = [637.5, 1000]
max = [1400, 3500]
[[line]]
from = [1400, 1000]
to = [1400, 3500]
out = [1, 0]
[anchors]
taxi_out = [[600, 1000], [600, -170], [300, -170], [0, -170]]
takeoff = [0, 0]
landing = [0, 1000]
taxi_in = [[0, 7750], [300, 7750], [600, 7750], [600, 3500]]
parking = [[1000, 1200], [1000, 1450], [1000, 1700], [1000, 1950], [1000, 2200],
           [1000, 2450], [1000, 2700], [1000, 2950], [1000, 3200]]
parking_heading = -90
"#;

fn synthetic_plan(text: &str) -> Plans {
    let defaults = plan::default_materials(DEFAULTS).unwrap();
    Plans {
        plans: vec![Plan::parse(text, &defaults).unwrap()],
    }
}

/// The synthetic import with a hut (alias 77) beside its airport.
fn synthetic_resources() -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut map = airport_resources();
    let strip = String::from_utf8(map["STRIP.OT"].clone()).unwrap();
    let hut = strip
        .replace("_STRIPProc", "_OBJProc")
        .replace("AIRPORT.SH", "F18.SH")
        .replace("STRIP.OT", "HUT.OT");
    map.insert("HUT.OT".into(), hut.into_bytes());
    let layout = String::from_utf8(map["UKR.MM"].clone()).unwrap();
    let middle = crate::test_support::resources::AIRPORT_AT as i64;
    let layout = format!(
        "{layout}obj\n\ttype HUT.OT\n\tpos {} 0 {middle}\n\tangle 0 0 0\n\talias 77\n\t.\n",
        middle + 3000
    );
    map.insert("UKR.MM".into(), layout.into_bytes());
    map
}

fn redrawn_terrain(
    map: &std::collections::BTreeMap<String, Vec<u8>>,
    plans: &Plans,
) -> WorldResult<Terrain> {
    let mut terrain = Terrain::for_mission(map, "UKR", Some(0), &Overrides::default())?;
    terrain.static_manifest.clear();
    terrain.build_airport_scene(map, "UKR", None, plans)?;
    Ok(terrain)
}

#[test]
fn a_redrawn_airport_keeps_the_retail_runway_and_replaces_its_field() {
    let map = synthetic_resources();
    let plans = synthetic_plan(SYNTHETIC);
    let retail = Terrain::for_mission(&map, "UKR", Some(0), &Overrides::default()).unwrap();
    let redrawn = redrawn_terrain(&map, &plans).unwrap();
    let before = retail.airport_scene.runway(AIRPORT_RUNWAY).unwrap();
    let after = redrawn.airport_scene.runway(AIRPORT_RUNWAY).unwrap();
    // The runway, its length and its approach line (the ILS) do not move.
    assert_eq!(after.approach_center, before.approach_center);
    assert_eq!(after.length_ft, before.length_ft);
    assert_eq!(after.heading, before.heading);
    assert_eq!(after.elevation_ft, before.elevation_ft);
    // The landable box is the redrawn field's, grass margin included.
    let built = &redrawn.redrawn[0];
    assert_eq!(after.surface, built.surface);
    assert!((built.surface.half[0] - ((1400. + 75.) * 0.5 + 100.)).abs() < 1e-6);
    let threshold = before.threshold(tore_sim::airport::ApproachEnd::Near);
    assert!((built.frame.origin[0] - threshold[0]).abs() < 1e-9);
    assert!((built.frame.origin[2] - threshold[2]).abs() < 1e-9);
    // The AI's points are the plan's, and all of them survive the
    // landable-surface check.
    let anchors = redrawn.airfield_anchors[&AIRPORT_RUNWAY];
    assert_eq!(Some(anchors), built.anchors);
    assert_eq!(anchors.takeoff_spot, built.frame.world([0., 0.]));
    assert_eq!(anchors.parking[0], built.frame.world([1000., 1200.]));
    // The hut moved onto the building line, east of it; the synthetic
    // import has no hangars or trucks to add.
    let hut = crate::surface::LAYOUT_OBJECT_BASE + 1;
    let centre = |terrain: &Terrain| {
        terrain
            .airport_scene
            .objects
            .iter()
            .find(|o| o.id == hut)
            .map(|o| o.bounds.center)
            .unwrap()
    };
    let moved = built.frame.local(centre(&redrawn));
    assert!(moved[0] > 1400. && moved[0] < 1400. + 200., "{moved:?}");
    assert!(moved[1] > 1000. && moved[1] < 1500., "{moved:?}");
    assert_ne!(centre(&retail), centre(&redrawn));
    assert_eq!(built.building_counts(), (1, 0));
    // The scenery's placements match the scene's.
    let sources = Placements::for_terrain(&map, &redrawn, "UKR").unwrap();
    let placement = sources
        .layout
        .placements
        .iter()
        .find(|p| p.alias == Some(77))
        .unwrap();
    let scene = redrawn
        .airport_scene
        .objects
        .iter()
        .find(|o| o.id == hut)
        .unwrap();
    let rebuilt = sources.stance(placement, 0.).unwrap();
    assert!((rebuilt.center[0] - scene.bounds.center[0]).abs() < 1e-6);
}

#[test]
fn a_plan_for_another_runway_length_keeps_the_retail_airfield() {
    let map = synthetic_resources();
    let retail = Terrain::for_mission(&map, "UKR", Some(0), &Overrides::default()).unwrap();
    // The built-in STRIP plan is for the retail 5,532 ft runway; the
    // synthetic map's is 8,000 ft, so the default build keeps it retail.
    assert!(retail.redrawn_airports);
    assert!(retail.redrawn.is_empty());
    let plans = synthetic_plan(&SYNTHETIC.replace("8000", "7000"));
    let terrain = redrawn_terrain(&map, &plans).unwrap();
    assert!(terrain.redrawn.is_empty());
    assert_eq!(
        terrain
            .airport_scene
            .runway(AIRPORT_RUNWAY)
            .unwrap()
            .surface,
        retail.airport_scene.runway(AIRPORT_RUNWAY).unwrap().surface
    );
}

#[test]
fn the_ils_runway_gets_edge_threshold_papi_and_approach_lights() {
    use lights::LightKind as K;
    let plans = builtin();
    let plan = single(&plans, "STRIP.OT");
    let parts = [(&plan, [0., 0.])];
    let (materials, patches) = compose(&parts, None, 0.);
    let frame = Frame {
        origin: [0.; 3],
        right: [1., 0., 0.],
        forward: [0., 0., 1.],
    };
    let all = lights::lights(&parts, None, &patches, &materials, &frame, &|_, _| -50.);
    let count = |kind| all.iter().filter(|l| l.kind == kind).count();
    // Three runways, both ends marked: four PAPI lights at each end.
    assert_eq!(count(K::Papi), 3 * 2 * 4);
    // The approach lights on the ILS runway's two ends only: 24 barrettes
    // of five and a crossbar of 16, ten side rows of six, 15 flashers.
    assert_eq!(count(K::Approach), 2 * (24 * 5 + 16));
    assert_eq!(count(K::ApproachSide), 2 * 10 * 6);
    assert_eq!(count(K::Flasher), 2 * 15);
    // Every 200 ft along both edges of all three runways.
    assert_eq!(count(K::Edge), 2 * (29 + 25 + 21));
    assert_eq!(count(K::Threshold), count(K::End));
    assert!(count(K::Taxiway) > 100);
    // Near-end PAPI: on the approach's left, the light nearest the runway
    // switching highest.
    let mut papi: Vec<_> = all
        .iter()
        .filter(|l| {
            l.kind == K::Papi
                && l.position[2] < 2_000.
                && l.position[0] < 0.
                && l.position[0] > -300.
        })
        .collect();
    papi.sort_by(|a, b| a.position[0].total_cmp(&b.position[0]));
    let angles: Vec<f64> = papi.iter().map(|l| l.param).collect();
    assert_eq!(angles, lights::PAPI_ANGLES_DEG);
    assert!((papi[0].position[2] - tore_sim::airport::AIM_PAST_THRESHOLD_FT).abs() < 1e-9);
    // No taxiway light stands on pavement; approach lights stand on their
    // stanchions at least at the runway's height.
    for light in &all {
        if light.kind == K::Taxiway {
            assert!(!on_pavement(
                &patches,
                [light.position[0], light.position[2]]
            ));
        }
        if light.kind == K::Approach {
            assert!(light.position[1] >= 3.);
        }
    }
}

#[test]
fn every_junction_gets_curved_corners_and_open_mouths() {
    let plans = builtin();
    // STRIP1 has a taxiway meeting the angled strip and a dog-leg; STRIP3A a
    // rapid exit at 30 degrees.
    for object_type in ["STRIP1.OT", "STRIP3A.OT", "STRIP.OT"] {
        let plan = single(&plans, object_type);
        let (materials, patches) = compose(&[(&plan, [0., 0.])], None, 0.);
        let fillet = materials.iter().position(|m| m.name == "fillet").unwrap();
        let fillets: Vec<_> = patches.iter().filter(|p| p.material == fillet).collect();
        // Some corners are not square: their grid axes are not at 90 degrees.
        let angled = fillets
            .iter()
            .filter(|p| dot(p.grid.u, p.grid.v).abs() > 0.05)
            .count();
        if object_type != "STRIP.OT" {
            assert!(angled > 0, "{object_type}: no angled corner");
        }
        // Where a later taxiway joins, the earlier one runs on with its
        // centreline only.
        let mouth = materials
            .iter()
            .position(|m| m.name == "taxiway_mouth")
            .unwrap();
        assert!(materials[mouth].paint.contains(&Paint::TaxiwayCentre));
        assert!(!materials[mouth].paint.contains(&Paint::Taxiway));
        assert!(patches.iter().any(|p| p.material == mouth), "{object_type}");
        // A fillet's paved corner meets its pavement: each cell of it is one
        // copy, at fractions up to (1, 1).
        for patch in &fillets {
            let cells = patch.cells();
            assert_eq!(cells.len(), 1, "{object_type}");
            assert!(
                cells[0]
                    .1
                    .iter()
                    .any(|f| (f[0] - 1.).abs() < 1e-6 && (f[1] - 1.).abs() < 1e-6)
            );
        }
    }
}

#[test]
fn a_pair_keeps_the_ils_runways_far_end_marked() {
    let plans = builtin();
    let links = plans
        .iter()
        .find(|p| p.pair.as_ref().is_some_and(|pair| pair.base == "STRIP7.OT"))
        .unwrap();
    let pair = links.pair.as_ref().unwrap();
    assert!(!pair.unmark.iter().any(|(name, _)| name == "R0"));
    let pair3 = plans
        .iter()
        .find(|p| p.pair.as_ref().is_some_and(|pair| pair.base == "STRIP3.OT"))
        .unwrap();
    assert_eq!(
        pair3.pair.as_ref().unwrap().unmark,
        vec![("R1".to_owned(), true)]
    );
    // The base's far-end threshold bars are drawn.
    let (base, tile) = (single(&plans, "STRIP7.OT"), single(&plans, "STRIP7A.OT"));
    let (materials, patches) = compose(
        &[(&base, [0., 0.]), (&tile, [3452., 8899.])],
        Some(links),
        0.,
    );
    let threshold = materials
        .iter()
        .position(|m| m.name == "runway_threshold")
        .unwrap();
    let length = base.runway_length_ft.unwrap();
    assert!(
        patches
            .iter()
            .any(|p| p.material == threshold && contains(&p.poly, [0., length - 75.]))
    );
}

#[test]
fn template_aircraft_on_grass_move_to_free_apron_spots() {
    use super::buildings::{Strip, apron_spots, snap_parked};
    let aprons = [([100., 0.], [500., 1000.])];
    let spots = apron_spots(&aprons, &[[300., 500.]]);
    // Two rows a quarter and three quarters deep, every 120 ft, none by the
    // AI slot.
    assert!(
        spots
            .iter()
            .all(|p| (p[0] - 200.).abs() < 1e-9 || (p[0] - 400.).abs() < 1e-9)
    );
    assert!(
        spots
            .iter()
            .all(|p| (p[0] - 300.).hypot(p[1] - 500.) >= 100.)
    );
    assert_eq!(spots.len(), 16);
    let frame = Frame {
        origin: [10_000., 0., 10_000.],
        right: [1., 0., 0.],
        forward: [0., 0., 1.],
    };
    let strip = Strip {
        index: 0,
        origin: [10_000., 0., 10_000.],
        center: [10_000., 0., 10_500.],
        half: [500., 1., 1_000.],
        right: [1., 0., 0.],
        forward: [0., 0., 1.],
    };
    let aircraft = |x: i32, z: i32, deck: bool| crate::surface::ParkedAircraft {
        id: crate::surface::UnitId(0x5000_0001),
        resource: "F18.PT".into(),
        position: [x, 0, z],
        angles: [45, 0, 0],
        nationality: None,
        side: tore_sim::combat::live::NO_SIDE,
        target: false,
        deck: deck.then_some(crate::surface::UnitId(0x5000_0000)),
    };
    let mut parked = vec![
        aircraft(9_000, 10_100, false),  // on grass west of the field
        aircraft(10_300, 10_600, false), // already on the apron
        aircraft(9_000, 10_200, true),   // on a carrier's deck
        aircraft(40_000, 40_000, false), // another field's
    ];
    let on_pavement = |p: Point| {
        aprons
            .iter()
            .any(|(min, max)| p[0] >= min[0] && p[0] <= max[0] && p[1] >= min[1] && p[1] <= max[1])
    };
    let moved = snap_parked(&mut parked, &frame, &[0], &[strip], &spots, &on_pavement);
    assert_eq!(moved, 1);
    assert_eq!(parked[0].position, [10_200, 0, 10_060]);
    assert_eq!(parked[0].angles, [45, 0, 0]);
    assert_eq!(parked[1].position, [10_300, 0, 10_600]);
    assert_eq!(parked[2].position, [9_000, 0, 10_200]);
    assert_eq!(parked[3].position, [40_000, 0, 40_000]);
}

/// A second tile's runway object flies from the tile's own main runway: its
/// line's middle and both ends (10 ft in) lie on the tile's pavement, in the
/// tile's own frame, at the scene's heading convention.
#[test]
fn a_second_tiles_runway_line_lies_on_its_main_runway() {
    let plans = builtin();
    let frame = Frame {
        origin: [0.; 3],
        right: [1., 0., 0.],
        forward: [0., 0., 1.],
    };
    for tile in ["STRIP3A.OT", "STRIP5A.OT", "STRIP6A.OT", "STRIP7A.OT"] {
        let plan = single(&plans, tile);
        assert!(!plan.ils_runway, "{tile}");
        let (_, patches) = compose(&[(&plan, [0., 0.])], None, 0.);
        let line = RunwayLine::main(&plan, &frame, 0., [0., 0.]).unwrap();
        let longest = plan.runways.iter().map(|r| r.length).fold(0., f64::max);
        assert_eq!(line.length, longest, "{tile}");
        let d = [line.heading.sin(), line.heading.cos()];
        for along in [0., line.length * 0.5 - 10., 10. - line.length * 0.5] {
            let p = [line.center[0] + d[0] * along, line.center[2] + d[1] * along];
            assert!(
                on_pavement(&patches, p),
                "{tile}: {p:?} is off the pavement"
            );
        }
    }
    // A base keeps the retail line.
    assert!(single(&plans, "STRIP.OT").ils_runway);
}

/// The measured offsets of each tile's frame in its base's (16 base
/// layouts), as `pairs_join_without_overlaps` uses them.
const PAIR_OFFSETS: [(&str, &str, Point); 4] = [
    ("STRIP3.OT", "STRIP3A.OT", [-896., 9670.]),
    ("STRIP5.OT", "STRIP5A.OT", [-8896., -628.]),
    ("STRIP6.OT", "STRIP6A.OT", [-8907., 693.]),
    ("STRIP7.OT", "STRIP7A.OT", [3452., 8899.]),
];

/// A redrawn airport assembled from plans as `apply` assembles it, in an
/// identity frame at heading 0, with the runway objects it flies: the base
/// (or a lone tile) first, then a pair's tile.
fn assembled(
    base: &Plan,
    tile: Option<(&Plan, Point)>,
    links: Option<&Plan>,
) -> (
    Built,
    Vec<(tore_sim::airport::Runway, Option<AirfieldAnchors>)>,
) {
    let frame = Frame {
        origin: [0.; 3],
        right: [1., 0., 0.],
        forward: [0., 0., 1.],
    };
    let alone = (!base.ils_runway && tile.is_none()).then(|| marked_alone(base));
    let mut parts = vec![(alone.as_ref().unwrap_or(base), [0., 0.])];
    if let Some((tile, offset)) = tile {
        let names = links.and_then(|l| l.pair.as_ref()?.continues.as_ref());
        let offset = names.map_or(offset, |names| align(offset, base, tile, names));
        parts.push((tile, offset));
    }
    let (materials, patches) = compose(&parts, links, 0.);
    let runways: Vec<DrawnRunway> = parts
        .iter()
        .enumerate()
        .flat_map(|(k, (plan, offset))| {
            plan.runways.iter().map(move |r| DrawnRunway {
                name: if k == 0 {
                    r.name.clone()
                } else {
                    format!("{} {}", plan.applies_to, r.name)
                },
                from: super::geometry::add(r.from, *offset),
                heading: r.heading,
                length: r.length,
                width: r.width,
                pad: r.pad,
            })
        })
        .collect();
    let (min, max) = super::geometry::bounds(patches.iter().map(|p| &p.poly)).unwrap();
    let surface = OrientedBox {
        center: [(min[0] + max[0]) * 0.5, 0., (min[1] + max[1]) * 0.5],
        half: [
            (max[0] - min[0]) * 0.5 + 200.,
            1.,
            (max[1] - min[1]) * 0.5 + 200.,
        ],
        heading: 0.,
        pitch: 0.,
        bank: 0.,
    };
    let runway = |object: u32, line: RunwayLine| tore_sim::airport::Runway {
        object,
        airport: object,
        name: format!("Runway {object}"),
        surface,
        approach_center: line.center,
        elevation_ft: 0.,
        heading: line.heading,
        length_ft: line.length,
    };
    let base_line = if base.ils_runway {
        let length = base.runways[0].length;
        RunwayLine {
            center: [0., 0., length * 0.5],
            heading: 0.,
            length,
        }
    } else {
        RunwayLine::main(base, &frame, 0., [0., 0.]).unwrap()
    };
    let mut objects = vec![(
        runway(1, base_line),
        plan_anchors(base, [0., 0.], &frame, 0.),
    )];
    if let Some((tile, _)) = tile {
        let offset = parts[1].1;
        let pair = links.and_then(|l| l.pair.as_ref());
        let line = match pair.and_then(|p| p.joined.then_some(p.continues.as_ref()).flatten()) {
            Some(names) => RunwayLine::joined(base, tile, names, &frame, 0., offset),
            None => RunwayLine::main(tile, &frame, 0., offset),
        }
        .unwrap();
        objects.push((runway(2, line), plan_anchors(tile, offset, &frame, 0.)));
    }
    let continues = super::continued_runways(&runways, tile.map(|t| t.0), links);
    let built = Built {
        plan: base.applies_to.clone(),
        strip_id: 1,
        frame,
        materials,
        patches,
        surface,
        anchors: None,
        lights: Vec::new(),
        runways,
        moved: Vec::new(),
        added: Vec::new(),
        snapped: 0,
        runway_line: None,
        continues,
    };
    (built, objects)
}

/// Slice CL-AP (John, 2026-10-11: "ensure AI actually taxi on taxiways
/// instead of grass"): at every plan, alone and in its pair, every runway
/// object has AI points, every AI route from every parking slot out to the
/// takeoff spot and in from the landing aim point stays on the pavement
/// (with the wheel track and the run-out past each stop), every drawn
/// runway end is reachable over pavement from parking, and the runway
/// object's line lies on a drawn runway. `--airfield-audit` runs the same
/// audit over every layout of an import.
#[test]
fn every_plan_and_pair_passes_the_ai_route_audit() {
    let plans = builtin();
    let mut cases: Vec<(String, Built, Vec<_>)> = Vec::new();
    for plan in plans
        .iter()
        .filter(|p| p.pair.is_none() && p.applies_to != "DTSTRP.OT")
    {
        let (built, objects) = assembled(plan, None, None);
        cases.push((plan.applies_to.clone(), built, objects));
    }
    for (base, tile, offset) in PAIR_OFFSETS {
        let links = plans
            .iter()
            .find(|p| p.pair.as_ref().is_some_and(|pair| pair.base == base))
            .unwrap();
        let (base_plan, tile_plan) = (single(&plans, base), single(&plans, tile));
        // The tile at its measured offset and 60 ft either side of it.
        for shift in [-60., 0., 60.] {
            let at = [offset[0] + shift, offset[1] - shift];
            let (built, objects) = assembled(&base_plan, Some((&tile_plan, at)), Some(links));
            cases.push((format!("{base}+{tile} {shift}"), built, objects));
        }
    }
    for (name, built, objects) in &cases {
        let all = std::slice::from_ref(built);
        let components = audit::components(built);
        // A pair's continuing runways are one straight runway, at the
        // measured offset and 60 ft either side (John's Spassk Dalniy step).
        if let Some(alignment) = audit::alignment(built) {
            assert_eq!(alignment.problem(), None, "{name}: {alignment:?}");
        } else {
            assert!(!name.contains('+'), "{name}: no continuing runways");
        }
        for (runway, anchors) in objects {
            let anchors =
                anchors.unwrap_or_else(|| panic!("{name}: {} has no AI points", runway.name));
            for route in audit::routes(&anchors, runway) {
                let off = audit::walk(all, &route);
                assert!(
                    off.is_empty(),
                    "{name} {}: {} leaves the pavement at {:?}",
                    runway.name,
                    route.name,
                    off.first()
                );
            }
            let parking = [anchors.parking[0][0], anchors.parking[0][2]];
            let from = audit::component_at(built, &components, parking);
            assert!(from.is_some(), "{name}: parking off the pavement");
            for drawn in &built.runways {
                for end in audit::runway_ends(drawn, 5.) {
                    assert_eq!(
                        audit::component_at(built, &components, end),
                        from,
                        "{name} {}: {} end {end:?} not reachable from parking",
                        runway.name,
                        drawn.name
                    );
                }
            }
            audit::line_runway(built, runway)
                .unwrap_or_else(|why| panic!("{name} {}: {why}", runway.name));
        }
    }
    assert_eq!(cases.len(), 12 + 4 * 3);
}

/// The audit's alignment verdict: a lateral step, a turn or a gap is named,
/// an overlap of the pavements (the pads) is not a problem.
#[test]
fn the_alignment_verdict_names_a_step_a_turn_and_a_gap() {
    let straight = audit::Alignment {
        offset_ft: 0.,
        heading_deg: 0.,
        gap_ft: -190.,
        threshold_gap_ft: 30.,
    };
    assert_eq!(straight.problem(), None);
    let stepped = audit::Alignment {
        offset_ft: 21.,
        heading_deg: 1.,
        gap_ft: 40.,
        ..straight
    };
    let why = stepped.problem().unwrap();
    assert!(why.contains("21.0 ft off the line"), "{why}");
    assert!(why.contains("1.00 degrees"), "{why}");
    assert!(why.contains("40 ft gap"), "{why}");
}

/// A pair's tile is moved across the base runway its main runway continues
/// so the two are one line; with `joined` (STRIP5A and STRIP6A, whose main
/// runway runs on into the base's crossing runway) the tile's runway object
/// flies the two as one, so both ILS thresholds stand at the real, marked
/// runway ends rather than one in the middle of the pavement.
#[test]
fn a_pair_tile_lines_up_with_the_runway_it_continues() {
    let plans = builtin();
    for (base, tile, offset) in PAIR_OFFSETS {
        let links = plans
            .iter()
            .find(|p| p.pair.as_ref().is_some_and(|pair| pair.base == base))
            .unwrap();
        let pair = links.pair.as_ref().unwrap();
        let names = pair
            .continues
            .as_ref()
            .expect("every pair continues a runway");
        let (base_plan, tile_plan) = (single(&plans, base), single(&plans, tile));
        let (built, objects) = assembled(&base_plan, Some((&tile_plan, offset)), Some(links));
        let b = built.runways.iter().find(|r| r.name == names[0]).unwrap();
        let t = built
            .runways
            .iter()
            .find(|r| r.name == format!("{tile} {}", names[1]))
            .unwrap();
        let side = super::geometry::right_of(super::geometry::direction(b.heading));
        let across = dot(super::geometry::sub(t.from, b.from), side);
        assert!(
            across.abs() < 1e-6,
            "{tile}: {across} ft off the base runway's line"
        );
        let line = &objects[1].0;
        let expected = if pair.joined {
            t.length + b.length
        } else {
            t.length
        };
        assert!(
            if pair.joined {
                line.length_ft > expected * 0.9
            } else {
                (line.length_ft - expected).abs() < 1e-6
            },
            "{tile}: line {} ft",
            line.length_ft
        );
        assert_eq!(
            pair.joined,
            ["STRIP5A.OT", "STRIP6A.OT"].contains(&tile),
            "{tile}"
        );
    }
}

/// A lone tile marks both ends of its runways (no base runs on from it);
/// a dirt strip has no lights (John, 2026-10-11: lone grass or dirt fields
/// are fine unlit).
#[test]
fn a_lone_tile_is_marked_at_both_ends_and_a_dirt_strip_is_unlit() {
    let plans = builtin();
    for tile in ["STRIP3A.OT", "STRIP5A.OT", "STRIP6A.OT", "STRIP7A.OT"] {
        let plan = single(&plans, tile);
        assert!(
            plan.runways.iter().any(|r| r.marked != [true, true]),
            "{tile}"
        );
        assert!(
            marked_alone(&plan)
                .runways
                .iter()
                .all(|r| r.marked == [true, true]),
            "{tile}"
        );
    }
    let dirt = single(&plans, "DTSTRP.OT");
    assert!(!dirt.lights);
    let frame = Frame {
        origin: [0.; 3],
        right: [1., 0., 0.],
        forward: [0., 0., 1.],
    };
    let parts = [(&dirt, [0., 0.])];
    let (materials, patches) = compose(&parts, None, 0.);
    assert!(lights::lights(&parts, None, &patches, &materials, &frame, &|_, _| 0.).is_empty());
    assert!(single(&plans, "STRIP7A.OT").lights);
}
