//! The resolved world identity shared by client and server recordings.
use tore_formats::theater::CELL_FEET;
use tore_world::surface::{
    UnitKind,
    resolve::{AreaDefense, GroundTarget},
};
use tore_world::terrain::{CONDITION_NAMES, Terrain};
pub const RETAIL_AIRFIELD_SCENE: u32 = 0;
pub const REDRAWN_AIRFIELD_SCENE: u32 = 1;
pub const REDRAWN_TEMPLATES_AIRFIELD_SCENE: u32 = 2;
pub const MERGED_AIRFIELD_SCENE: u32 = 3;
pub const NO_CITY_FILL_SCENE: u32 = 0;
pub const CITY_FILL_SCENE: u32 = 1;
pub fn recorded_area(area: &AreaDefense) -> tore_replay::AreaDefense {
    tore_replay::AreaDefense {
        aaa: u8::try_from(area.aaa).unwrap_or(u8::MAX),
        sam: u8::try_from(area.sam).unwrap_or(u8::MAX),
        seed: area.seed,
        enemy_nationality: u8::try_from(area.enemy_nationality).unwrap_or(u8::MAX),
        night_stealth: area.night_stealth,
        rolled: area.rolled,
    }
}
pub fn recorded_target(target: &GroundTarget) -> tore_replay::GroundTarget {
    tore_replay::GroundTarget {
        stem: target.stem.clone(),
        aaa: u8::try_from(target.aaa).unwrap_or(u8::MAX),
        sam: u8::try_from(target.sam).unwrap_or(u8::MAX),
        seed: target.seed,
        enemy_nationality: u8::try_from(target.enemy_nationality).unwrap_or(u8::MAX),
        night_stealth: target.night_stealth,
        jitter: target.variation.jitter,
        relocate: target.variation.relocate,
        front: target.variation.front,
        separation_nm: target.separation_nm,
    }
}
pub fn has_surface(terrain: &Terrain) -> bool {
    let surface = &terrain.surface;
    surface.template.is_some()
        || !surface.parked.is_empty()
        || surface
            .units
            .iter()
            .any(|unit| unit.kind == UnitKind::Active || unit.route.is_some())
}
pub fn of(terrain: &Terrain) -> tore_replay::World {
    let configuration = terrain.weather.configuration();
    let cell = f64::from(CELL_FEET);
    tore_replay::World {
        theater: tore_formats::theater::base_theater(&terrain.layout)
            .unwrap_or_default()
            .to_owned(),
        theater_name: terrain.theater.name.clone(),
        layout: terrain.layout.clone(),
        weather: terrain.condition.and_then(|c| u32::try_from(c).ok()),
        weather_name: terrain
            .condition
            .and_then(|c| CONDITION_NAMES.get(c))
            .map_or("map default", |name| name)
            .to_owned(),
        // Weather presentation is always seeded with 1 at launch.
        weather_seed: Some(1),
        time_of_day_s: f64::from(configuration.start_seconds()),
        wind_fps: configuration.wind_world_fps(),
        clouds: tore_replay::Clouds {
            module: terrain.environment.layer.clone(),
            deck_ft: terrain
                .environment
                .clouds
                .filter(|feet| *feet != 0)
                .map(f64::from),
        },
        extent_ft: Some([
            terrain.theater.cols.saturating_sub(1) as f64 * cell,
            terrain.theater.rows.saturating_sub(1) as f64 * cell,
        ]),
        ground_target: terrain
            .surface
            .template
            .as_ref()
            .map(|site| recorded_target(&site.settings)),
        area_defense: terrain.surface.area.as_ref().map(recorded_area),
        airfield_scene: if terrain.merged_airports {
            MERGED_AIRFIELD_SCENE
        } else if terrain.redrawn_templates {
            REDRAWN_TEMPLATES_AIRFIELD_SCENE
        } else if terrain.redrawn_airports {
            REDRAWN_AIRFIELD_SCENE
        } else {
            RETAIL_AIRFIELD_SCENE
        },
        city_fill_scene: if terrain.city_fill.is_some() {
            CITY_FILL_SCENE
        } else {
            NO_CITY_FILL_SCENE
        },
        surface: has_surface(terrain),
    }
}
