use super::*;
use crate::terrain::{Overrides, Terrain};
use crate::test_support::resources::{AIRPORT_RUNWAY, airport_resources};

fn kiev() -> Plan {
    plans(true)
        .unwrap()
        .into_iter()
        .find(|p| p.strip == "Kiev")
        .unwrap()
}

fn on_pavement(patches: &[Patch], p: [f64; 2]) -> bool {
    patches.iter().any(|patch| patch.rect.contains(p))
}

#[test]
fn the_switch_off_applies_no_plan() {
    assert!(plans(false).unwrap().is_empty());
    assert!(!Overrides::default().redrawn_airports);
}

#[test]
fn the_kiev_plan_keeps_the_ai_on_pavement_and_never_overlaps() {
    let plan = kiev();
    let patches = plan.patches();
    let a = &plan.anchors;
    for p in a
        .taxi_out
        .iter()
        .chain(&a.taxi_in)
        .chain(&a.parking)
        .chain([&a.takeoff, &a.landing])
    {
        assert!(on_pavement(&patches, *p), "{p:?} off the pavement");
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
                "{p:?} on a taxi leg is off the pavement"
            );
        }
    }
    for (i, a) in patches.iter().enumerate() {
        for b in &patches[i + 1..] {
            let overlap: f64 = (0..2)
                .map(|axis| {
                    (a.rect.max[axis].min(b.rect.max[axis])
                        - a.rect.min[axis].max(b.rect.min[axis]))
                    .max(0.)
                })
                .product();
            assert!(overlap < 1e-6, "{a:?} overlaps {b:?}");
        }
    }
    // The runway is the real width the plan asks for, not the retail 368 ft.
    let main = &plan.runways[0];
    assert_eq!(main.width, 200.);
    let landable = plan.landable(&patches);
    assert!(landable.contains([main.x, main.threshold + main.length]));
}

#[test]
fn patch_cells_tile_each_patch_exactly() {
    let plan = kiev();
    for patch in plan.patches() {
        let cells = patch.cells();
        let area: f64 = cells
            .iter()
            .map(|(p, _)| ((p[2][0] - p[0][0]) * (p[2][1] - p[0][1])).abs())
            .sum();
        assert!(
            (area - patch.rect.area()).abs() < 1e-3,
            "{patch:?}: cells {area}"
        );
        for (_, fractions) in cells {
            for f in fractions {
                assert!((-1e-9..=1. + 1e-9).contains(&f[0]) && (-1e-9..=1. + 1e-9).contains(&f[1]));
            }
        }
    }
    // A designation digit is exactly one copy of its texels.
    let digit = plan.material("digit_3").unwrap();
    let patch = plan
        .patches()
        .into_iter()
        .find(|p| p.material == digit)
        .unwrap();
    let cells = patch.cells();
    assert_eq!(cells.len(), 1);
    assert_eq!(cells[0].1, [[0., 0.], [1., 0.], [1., 1.], [0., 1.]]);
}

/// A plan for the synthetic airport: an 8,000 ft runway, one taxiway, an
/// apron, a moved hut and an added one.
const SYNTHETIC: &str = r#"
layout = "UKR"
strip = "Synthetic Field"
runway_length_ft = 8000
grass_margin_ft = 100
[[material]]
name = "runway_plain"
pic = "R.PIC"
rect = [0, 0, 8, 8]
tile_ft = [0, 100]
[[material]]
name = "runway_threshold"
pic = "R.PIC"
rect = [0, 0, 8, 8]
tile_ft = [50, 150]
[[material]]
name = "runway_touchdown"
pic = "R.PIC"
rect = [0, 0, 8, 8]
tile_ft = [0, 100]
[[material]]
name = "runway_centreline"
pic = "R.PIC"
rect = [0, 0, 8, 8]
tile_ft = [0, 100]
[[material]]
name = "taxiway"
pic = "R.PIC"
rect = [0, 0, 8, 8]
tile_ft = [100, 75]
[[material]]
name = "apron"
pic = "R.PIC"
rect = [0, 0, 8, 8]
tile_ft = [50, 50]
[[runway]]
name = "A"
x = 0
threshold = 0
length = 8000
width = 150
pad = 200
[[taxiway]]
name = "T"
width = 75
from = [600, -150]
to = [600, 8150]
[[taxiway]]
name = "S"
width = 75
from = [0, -150]
to = [600, -150]
[[taxiway]]
name = "N"
width = 75
from = [0, 8150]
to = [600, 8150]
[[apron]]
name = "ramp"
min = [637.5, 1000]
max = [1400, 3500]
[anchors]
taxi_out = [[600, 1000], [600, -150], [300, -150], [0, -150]]
takeoff = [0, 0]
landing = [0, 1000]
taxi_in = [[0, 7800], [0, 8150], [600, 8150], [600, 3500]]
parking = [[1000, 1200], [1000, 1450], [1000, 1700], [1000, 1950], [1000, 2200],
           [1000, 2450], [1000, 2700], [1000, 2950], [1000, 3200]]
parking_heading = 90
[[building]]
type = "HUT.OT"
at = [1500, 2000]
heading = -90
replaces = 77
[[building]]
type = "HUT.OT"
at = [1500, 2500]
heading = 90
"#;

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
    plan: &Plan,
) -> WorldResult<Terrain> {
    let mut terrain = Terrain::for_mission(map, "UKR", Some(0), &Overrides::default())?;
    terrain.static_manifest.clear();
    terrain.build_airport_scene(map, "UKR", None, std::slice::from_ref(plan))?;
    Ok(terrain)
}

#[test]
fn a_redrawn_airport_keeps_the_retail_runway_and_replaces_its_field() {
    let map = synthetic_resources();
    let plan = Plan::parse(SYNTHETIC).unwrap();
    let retail = Terrain::for_mission(&map, "UKR", Some(0), &Overrides::default()).unwrap();
    let redrawn = redrawn_terrain(&map, &plan).unwrap();
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
    assert_eq!(built.surface.half[0], (1400. + 100. - (-75. - 100.)) * 0.5);
    let threshold = before.threshold(tore_sim::airport::ApproachEnd::Near);
    assert!((built.frame.origin[0] - threshold[0]).abs() < 1e-9);
    assert!((built.frame.origin[2] - threshold[2]).abs() < 1e-9);
    // The AI's points are the plan's, and all of them survive the
    // landable-surface check.
    let anchors = redrawn.airfield_anchors[&AIRPORT_RUNWAY];
    assert_eq!(anchors, built.anchors);
    assert_eq!(anchors.takeoff_spot, built.frame.world([0., 0.]));
    assert_eq!(anchors.parking[0], built.frame.world([1000., 1200.]));
    // The hut moved; a second one was added after the layout's objects.
    let hut = |terrain: &Terrain, id: u32| {
        terrain
            .airport_scene
            .objects
            .iter()
            .find(|o| o.id == id)
            .map(|o| o.bounds.center)
    };
    let moved = hut(&redrawn, crate::surface::LAYOUT_OBJECT_BASE + 1).unwrap();
    let at = built.frame.world([1500., 2000.]);
    assert!((moved[0] - at[0].round()).abs() < 200. && (moved[2] - at[2].round()).abs() < 200.);
    assert_ne!(
        hut(&retail, crate::surface::LAYOUT_OBJECT_BASE + 1),
        Some(moved)
    );
    let added = crate::surface::LAYOUT_OBJECT_BASE + ADDED_ORDINAL_BASE;
    assert!(hut(&retail, added).is_none());
    assert!(hut(&redrawn, added).is_some());
    assert_eq!(
        redrawn.airport_scene.objects.len(),
        retail.airport_scene.objects.len() + 1
    );
    // The scenery's placements match the scene's.
    let sources = Placements::for_terrain(&map, &redrawn, "UKR").unwrap();
    assert_eq!(sources.layout.placements.len(), 3);
    let moved_placement = sources
        .layout
        .placements
        .iter()
        .find(|p| p.alias == Some(77))
        .unwrap();
    assert_eq!(
        moved_placement.position,
        [at[0].round() as i32, 0, at[2].round() as i32]
    );
    assert_eq!(moved_placement.angles, [-90, 0, 0]);
    assert_eq!(sources.layout.placements[2].angles, [90, 0, 0]);
}

#[test]
fn a_plan_for_another_runway_length_is_refused() {
    let map = synthetic_resources();
    let plan = Plan::parse(&SYNTHETIC.replace("8000", "7000")).unwrap();
    let error = redrawn_terrain(&map, &plan).err().unwrap().to_string();
    assert!(error.contains("plan expects 7000"), "{error}");
}

#[test]
fn a_plan_whose_ai_points_leave_its_field_is_refused() {
    let map = synthetic_resources();
    let plan = Plan::parse(&SYNTHETIC.replace("takeoff = [0, 0]", "takeoff = [5000, 0]")).unwrap();
    let error = redrawn_terrain(&map, &plan).err().unwrap().to_string();
    assert!(error.contains("anchor off its landable box"), "{error}");
}
