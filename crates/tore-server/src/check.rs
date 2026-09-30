//! `--check`: what a mission file will fly, printed without opening a port.
//! The plane numbers are the ones `open-planes` and the game's `--slot` use,
//! and the runway numbers are the ones `start ground` takes.

use crate::prepare::Prepared;
use tore_world::mission::{MissionSpec, RUNWAY_OBJECT_BASE, Start};

/// A one-line summary of the mission, for the start lines.
pub fn mission_summary(spec: &MissionSpec, aircraft: usize) -> String {
    let start = match spec.start {
        Start::Airborne { altitude_ft } => format!("airborne at {altitude_ft} ft"),
        Start::Ground { runway, .. } => format!(
            "ground start on runway {}",
            runway.saturating_sub(RUNWAY_OBJECT_BASE)
        ),
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
        let airport = scene
            .airports
            .iter()
            .find(|airport| airport.id == runway.airport)
            .map_or(runway.name.as_str(), |airport| airport.name.as_str());
        let short = if runway.short_strip() {
            ", a short strip: no ground start"
        } else {
            ""
        };
        lines.push(format!(
            "  {:>3}  {airport} ({:.0} ft{short})",
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
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        options::Options,
        prepare::{
            prepare,
            tests::{MISSION, data_folder},
        },
    };
    use std::fs;

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
        let _ = fs::remove_dir_all(dir);
    }
}
