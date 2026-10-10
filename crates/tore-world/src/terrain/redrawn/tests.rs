use super::geometry::{contains, overlap_area, signed_area};
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
fn the_switch_off_applies_no_plan() {
    assert!(Plans::new(false).unwrap().is_empty());
    assert!(!Overrides::default().redrawn_airports);
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
fn a_plan_for_another_runway_length_is_refused() {
    let map = synthetic_resources();
    let plans = synthetic_plan(&SYNTHETIC.replace("8000", "7000"));
    let error = redrawn_terrain(&map, &plans).err().unwrap().to_string();
    assert!(error.contains("plan expects 7000"), "{error}");
}
