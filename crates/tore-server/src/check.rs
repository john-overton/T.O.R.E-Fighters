//! `--check`: what a mission file will fly, printed without opening a port.
//! The plane numbers are the ones `open-planes` and the game's `--slot` use,
//! and the runway numbers are the ones `start ground` takes.

use crate::prepare::Prepared;
use tore_session::host::content::{GameContent, report_lines};
use tore_world::{
    mission::{MissionSpec, RUNWAY_OBJECT_BASE, Start},
    terrain::Terrain,
};

/// Why a mission's explicit ground start cannot be flown: `start ground N`
/// parks the friendly wing (Blue) on runway N, and an airport takes its
/// runway's layout side (slice AL1), so a Redfor field is refused. A neutral
/// field serves both sides. `None` for any other start, or a runway this
/// import does not have (the build reports that).
pub fn ground_start_problem(spec: &MissionSpec, terrain: &Terrain) -> Option<String> {
    let scene = &terrain.airport_scene;
    let runway = spec.ground_runway()?;
    let id = scene.runway(runway)?.airport;
    let airport = scene.airports.iter().find(|a| a.id == id)?;
    (!airport.serves(false)).then(|| {
        format!(
            "starts the friendly wing on runway {} at {}, an enemy airfield: a ground start needs a friendly or neutral runway (--check lists the runways and their sides)",
            runway.saturating_sub(RUNWAY_OBJECT_BASE),
            airport.name
        )
    })
}

/// How the runway list names an airport's side, from the friendly wing's
/// point of view, and whether a ground start may use it: a field that serves
/// both sides is neutral, one that serves only Redfor the enemy's.
fn side_words(blue: bool, red: bool) -> &'static str {
    match (blue, red) {
        (true, true) => "neutral",
        (true, false) => "friendly",
        (false, true) => "enemy: no ground start",
        (false, false) => "no ground start",
    }
}

/// A one-line summary of the mission, for the start lines.
pub fn mission_summary(spec: &MissionSpec, aircraft: usize) -> String {
    let start = match spec.start {
        Start::Airborne { altitude_ft } => format!("airborne at {altitude_ft} ft"),
        Start::Ground { runway, .. } => format!(
            "ground start on runway {}",
            runway.saturating_sub(RUNWAY_OBJECT_BASE)
        ),
        Start::GroundAuto { .. } => "ground start on a runway the world picks".to_owned(),
    };
    let side = |range: std::ops::Range<usize>| -> String {
        let counts: Vec<String> = spec.wings[range]
            .iter()
            .filter(|wing| wing.count > 0)
            .map(|wing| format!("{} {}", wing.count, wing.aircraft.selection_key()))
            .collect();
        if counts.is_empty() {
            "none".into()
        } else {
            counts.join(", ")
        }
    };
    format!(
        "Mission: {} ({}), {start}, enemy {} nm away; friendly {}; enemy {}; {aircraft} aircraft",
        spec.theater,
        spec.condition.name(),
        spec.separation_nm,
        side(0..3),
        side(3..6),
    )
}

/// The lines `--check` prints.
pub fn report(prepared: &Prepared) -> Vec<String> {
    let spec = &prepared.spec;
    let planes = prepared.world.roster.planes();
    let mut lines = vec![
        mission_summary(spec, planes.len()),
        format!("Mission file: {}", prepared.config.mission.display()),
        String::new(),
        "Planes (the numbers open-planes and --slot use):".into(),
    ];
    for plane in planes {
        let wing = &spec.wings[MissionSpec::wing_index(plane.slot.wing)];
        let side = if plane.slot.wing.side.is_enemy() {
            "enemy"
        } else {
            "friendly"
        };
        lines.push(format!(
            "  {:>2}  {:<9} {side} wing {}, member {}, {}",
            plane.id.0,
            wing.aircraft.selection_key(),
            plane.slot.wing.index + 1,
            plane.slot.member + 1,
            wing.skill.name()
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        "Runways of {} (the numbers start ground takes):",
        spec.theater
    ));
    let scene = &prepared.world.terrain.airport_scene;
    let mut runways: Vec<_> = scene.runways.iter().collect();
    runways.sort_by_key(|runway| runway.object);
    if runways.is_empty() {
        lines.push("  none".into());
    }
    for runway in runways {
        let found = scene
            .airports
            .iter()
            .find(|airport| airport.id == runway.airport);
        let airport = found.map_or(runway.name.as_str(), |airport| airport.name.as_str());
        let side = found.map_or("no ground start", |a| {
            side_words(a.serves(false), a.serves(true))
        });
        let short = if runway.short_strip() {
            ", a short strip: no ground start"
        } else {
            ""
        };
        lines.push(format!(
            "  {:>3}  {airport} ({:.0} ft, {side}{short})",
            runway.object.saturating_sub(RUNWAY_OBJECT_BASE),
            runway.length_ft
        ));
    }
    lines.push(String::new());
    lines.push(format!(
        "Content manifest: {} resources, digest {:016x}",
        prepared.manifest.entries.len(),
        prepared.manifest.digest()
    ));
    // Stage L: the import's source and content, one line per item, so an
    // operator can compare two imports (`tore-bot --content-report` prints
    // the same lines).
    lines.push(String::new());
    let content = GameContent::read(&prepared.data_dir, &prepared.resources);
    lines.extend(report_lines(&content));
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        options::Options,
        prepare::{
            prepare,
            tests::{MISSION, data_folder, data_folder_with},
        },
    };
    use std::fs;
    use tore_world::test_support::resources::owned_airport_resources;

    /// A ground start on the synthetic field, owned by `nationality2`.
    fn ground_start_at(name: &str, nationality2: Option<u8>) -> Result<Prepared, String> {
        let dir = data_folder_with(name, true, owned_airport_resources(nationality2));
        fs::write(
            dir.join("mission.txt"),
            MISSION.replace("start airborne 10000", "start ground 0 10000"),
        )
        .unwrap();
        let prepared = prepare(&Options::default(), &dir);
        let _ = fs::remove_dir_all(dir);
        prepared
    }

    #[test]
    fn an_explicit_ground_start_at_an_enemy_field_is_refused() {
        // Bit 0x80 is Redfor: the friendly wing may not park there.
        let error = ground_start_at("check-enemy-field", Some(137))
            .err()
            .unwrap();
        assert!(
            error.contains("runway 0 at Synthetic Field, an enemy airfield"),
            "{error}"
        );
        assert!(error.contains("friendly or neutral runway"), "{error}");
        // A Blue field and an unowned (neutral) one are flown.
        let blue = ground_start_at("check-blue-field", Some(12)).unwrap();
        let text = report(&blue).join("\n");
        assert!(
            text.contains("    0  Synthetic Field (8000 ft, friendly)"),
            "{text}"
        );
        let neutral = ground_start_at("check-neutral-field", None).unwrap();
        assert!(report(&neutral).join("\n").contains("ft, neutral)"));
    }

    #[test]
    fn the_report_lists_planes_runways_and_the_digest() {
        let dir = data_folder("check-report", true);
        fs::write(dir.join("mission.txt"), MISSION).unwrap();
        let prepared = prepare(&Options::default(), &dir).unwrap();
        let text = report(&prepared).join("\n");
        assert!(
            text.contains("Mission: UKR (clear), airborne at 10000 ft, enemy 2 nm away"),
            "{text}"
        );
        assert!(text.contains("4 aircraft"), "{text}");
        assert!(
            text.contains("   0  F18.PT    friendly wing 1, member 1, average"),
            "{text}"
        );
        assert!(
            text.contains("   3  F18.PT    enemy wing 1, member 2, average"),
            "{text}"
        );
        assert!(text.contains("Runways of UKR"), "{text}");
        assert!(
            text.contains(&format!("digest {:016x}", prepared.manifest.digest())),
            "{text}"
        );
        // Stage L: the source and one line per item.
        assert!(
            text.contains(
                "Content: an unknown Fighters Anthology build, imported by an unknown T.O.R.E"
            ),
            "{text}"
        );
        assert!(
            text.contains("Content items: 1 aircraft, 1 theater, 2 weapons, the shared data"),
            "{text}"
        );
        assert!(text.contains("\n  aircraft F18.PT "), "{text}");
        assert!(text.contains("\n  shared data "), "{text}");
        let _ = fs::remove_dir_all(dir);
    }
}
