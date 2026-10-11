//! `--airport-allegiance`: every base theater's airports by side, swept for
//! both sides (slice AL1, docs/spec/airports.md, "Allegiance").
//!
//! For Blue and for Redfor it lists the ground-start airports the creator
//! offers and its default, and checks them against the world the mission is
//! built in: each offered field is one the tower serves that side (its own
//! or a neutral one), never the other side's; the creator's owner agrees
//! with the imported allegiance; `start ground auto` picks no enemy field
//! and is refused when the side has none; and the AI's home runway, taken
//! from the middle and the four corners of the map, is always one the side
//! may use. A theater with no field for a side is recorded, and Ground is
//! locked to Airborne there. A developer probe without a window.

use super::*;
use crate::{ai_wings, mission_layout, terrain::Terrain};
use tore_sim::ai::launch::Side as LaunchSide;

/// The words a line uses for a side.
fn side_name(redfor: bool) -> &'static str {
    if redfor { "red" } else { "blue" }
}

/// The airport of a runway object in the world, if the object is a runway.
fn airport_of(world: &Terrain, object: u32) -> Option<&tore_sim::airport::Airport> {
    let scene = &world.airport_scene;
    let runway = scene.runway(object)?;
    scene.airports.iter().find(|a| a.id == runway.airport)
}

/// One theater for one side: the lines printed and the problems found.
fn sweep_side(
    quick: &mut QuickMission,
    data: &BTreeMap<String, Vec<u8>>,
    index: usize,
    code: &str,
    world: &Terrain,
    redfor: bool,
    problems: &mut Vec<String>,
) -> bool {
    let side = side_name(redfor);
    quick.set_player_redfor(redfor);
    quick.choose_theater_code(code, data);
    if quick.draft.values[13] != index {
        problems.push(format!("{code}: the creator did not select the theater"));
        return false;
    }
    let offered: Vec<u32> = quick.offered_airports(index).to_vec();
    let mut problem = |text: String| problems.push(format!("{code} {side}: {text}"));
    // Each offered field: the side's own or neutral, as the world has it.
    let mut names = Vec::new();
    for object in &offered {
        let Some(airport) = airport_of(world, *object) else {
            problem(format!("offered runway {object:#x} is not in the world"));
            continue;
        };
        if !airport.serves(redfor) {
            problem(format!(
                "offers {} ({:?} to this side)",
                airport.name,
                airport.allegiance_for(redfor)
            ));
        }
        names.push(airport.name.clone());
    }
    // The creator's owners agree with the world's allegiance.
    for field in quick.theater_fields(index) {
        if let Some(airport) = airport_of(world, field.id) {
            let expected = tore_sim::airport::Allegiance::of_owner(field.redfor);
            if airport.allegiance != expected {
                problem(format!(
                    "{} is {:?} in the world but {expected:?} in the creator",
                    field.name, airport.allegiance
                ));
            }
        }
    }
    // The creator's Ground: its default, or the lock to Airborne.
    quick.activate(33);
    let default = quick.ground_runway();
    if offered.is_empty() {
        if quick.ground_start() || quick.notice.as_deref() != Some(NO_FIELD_NOTICE) {
            problem("Ground is offered with no field of this side".into());
        }
    } else if default != offered.first().copied() {
        problem(format!(
            "the default runway {default:?} is not the first offered"
        ));
    }
    quick.apply(33, 0);
    // `start ground auto` never parks the wing at an enemy field.
    match mission_layout::auto_runway_for(world, 1, redfor) {
        Ok(object) => {
            if !airport_of(world, object).is_some_and(|a| a.serves(redfor)) {
                problem(format!("start ground auto picks runway {object:#x}"));
            }
        }
        Err(_) if offered.is_empty() => {}
        Err(error) => problem(format!("start ground auto refused: {error}")),
    }
    // The AI's home runway from the middle and the corners of the map.
    let fields = ai_wings::Airfields::from_world(world, None);
    let bounds = mission_layout::map_bounds(world);
    let launch = if redfor {
        LaunchSide::Enemy
    } else {
        LaunchSide::Friendly
    };
    let middle = [
        (bounds.min[0] + bounds.max[0]) / 2.,
        (bounds.min[1] + bounds.max[1]) / 2.,
    ];
    let mut homes = Vec::new();
    for [x, z] in [
        middle,
        bounds.min,
        bounds.max,
        [bounds.min[0], bounds.max[1]],
        [bounds.max[0], bounds.min[1]],
    ] {
        match fields.home([x, 10_000., z], launch) {
            Some(view) => match airport_of(world, view.object) {
                Some(airport) if airport.serves(redfor) => homes.push(airport.name.clone()),
                Some(airport) => problem(format!("an AI aircraft calls {} home", airport.name)),
                None => problem(format!("AI home runway {:#x} is unknown", view.object)),
            },
            None => homes.push("none".into()),
        }
    }
    if offered.is_empty() {
        println!("airport-allegiance: {code} {side} none: ground start locked to Airborne");
    } else {
        println!(
            "airport-allegiance: {code} {side} offered {} default {}: {}",
            offered.len(),
            names.first().map_or("?", String::as_str),
            names.join(", ")
        );
    }
    println!(
        "airport-allegiance: {code} {side} ai-homes {}",
        homes.join(", ")
    );
    !offered.is_empty()
}

/// Sweeps every base theater for both sides and prints the summary; an
/// error when anything was wrong.
pub fn run(
    data: &BTreeMap<String, Vec<u8>>,
    options: Options,
    aircraft: AircraftId,
) -> crate::AppResult<()> {
    let mut quick = QuickMission::new(aircraft, options, data);
    let mut problems = Vec::new();
    let mut none = [Vec::new(), Vec::new()];
    let codes = quick.theater_codes.clone();
    for (index, code) in codes.iter().enumerate() {
        let world = crate::scenery::launch_terrain(data, code, None)?;
        let fields = quick.theater_fields(index);
        let count = |redfor: Option<bool>| {
            fields
                .iter()
                .filter(|f| !f.short && f.redfor == redfor)
                .count()
        };
        println!(
            "airport-allegiance: {code} airports {} blue {} red {} neutral {} short {}",
            fields.len(),
            count(Some(false)),
            count(Some(true)),
            count(None),
            fields.iter().filter(|f| f.short).count()
        );
        for redfor in [false, true] {
            if !sweep_side(&mut quick, data, index, code, &world, redfor, &mut problems) {
                none[usize::from(redfor)].push(code.clone());
            }
        }
    }
    quick.set_player_redfor(false);
    for problem in &problems {
        println!("airport-allegiance: PROBLEM {problem}");
    }
    let list = |codes: &[String]| {
        if codes.is_empty() {
            "-".to_owned()
        } else {
            codes.join(" ")
        }
    };
    println!(
        "airport-allegiance: {} theaters, blue none: {}, red none: {}, {} problems",
        codes.len(),
        list(&none[0]),
        list(&none[1]),
        problems.len()
    );
    if problems.is_empty() {
        Ok(())
    } else {
        Err(format!("{} airport allegiance problems", problems.len()).into())
    }
}
