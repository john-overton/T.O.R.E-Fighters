//! A mission recording's record of its world: the conversions between the
//! terrain and the header's `tore_replay::World`. They live here, in the app's
//! replay code, so the terrain itself knows nothing of recordings.
use crate::{
    AppResult,
    terrain::{CONDITION_NAMES, Recorded, Terrain},
};
use std::collections::BTreeMap;
use tore_formats::theater::CELL_FEET;
use tore_world::surface::{
    UnitKind,
    layout::Variation,
    resolve::{AreaDefense, GroundTarget},
};

/// The header's airfield scene for the retail airfields: what every
/// recording before the redraw holds (docs/REPLAYS.md, "The airfield scene").
pub const RETAIL_AIRFIELD_SCENE: u32 = 0;
/// The header's airfield scene for the redrawn airports of
/// docs/formats/redrawn-airports.md, the default since protocol 24
/// (2026-10-11). A later change to the plans that moves anything an aircraft
/// touches takes a new number, so an old recording is never replayed on
/// airfields it was not flown on.
pub const REDRAWN_AIRFIELD_SCENE: u32 = 1;
/// The redrawn airports with the runway piece a Quick Mission ground target
/// template brings redrawn too (B14, 2026-10-12): a template airfield
/// target's pavement, landable box and AI points moved, so it takes its own
/// number and a scene 1 recording keeps the template's retail strip.
pub const REDRAWN_TEMPLATES_AIRFIELD_SCENE: u32 = 2;
/// The header's city fill scene for a world without the city fill: what
/// every recording before the fill became the default holds (docs/REPLAYS.md,
/// "The city fill scene").
pub const NO_CITY_FILL_SCENE: u32 = 0;
/// The header's city fill scene for the city fill of `tore_world::city`, the
/// default since protocol 25 (2026-10-11). A later change to the fill that
/// moves anything an aircraft can hit takes a new number, so an old
/// recording is never replayed over buildings it was not flown over.
pub const CITY_FILL_SCENE: u32 = 1;

/// Reads a recorded identity back into launch settings, refusing values
/// a launch could not have produced.
pub fn recorded(identity: &tore_replay::World) -> AppResult<Recorded> {
    let code = identity.layout.trim_end_matches(".MM").to_owned();
    if code.is_empty() || tore_formats::theater::base_theater(&identity.layout).is_none() {
        return Err(format!(
            "the recording names an unknown map layout {:?}",
            identity.layout
        )
        .into());
    }
    let condition = identity
        .weather
        .map(|index| {
            usize::try_from(index)
                .ok()
                .filter(|index| *index < tore_sim::environment::CONDITIONS.len())
                .ok_or_else(|| {
                    format!("the recording names weather choice {index}, which does not exist")
                })
        })
        .transpose()?;
    let seconds = identity.time_of_day_s;
    if !(seconds.is_finite() && seconds.fract() == 0. && (0. ..86_400.).contains(&seconds)) {
        return Err(format!("the recording's start time {seconds} s is not a time of day").into());
    }
    let seconds = seconds as i32;
    if seconds % 60 != 0 {
        return Err("the recording's start time is not a whole minute".into());
    }
    let cloud_altitude = match identity.clouds.deck_ft {
        None => 0,
        Some(feet) if feet.fract() == 0. && (0. ..=400_000.).contains(&feet) => feet as i32,
        Some(feet) => {
            return Err(format!("the recording's cloud deck at {feet} ft is out of range").into());
        }
    };
    let weather_seed = match identity.weather_seed {
        None => 1,
        Some(seed) => i32::try_from(seed)
            .map_err(|_| format!("the recording's weather seed {seed} is out of range"))?,
    };
    Ok(Recorded {
        code,
        condition,
        layer: Some(identity.clouds.module.clone()).filter(|layer| !layer.is_empty()),
        time: [seconds / 3600, seconds / 60 % 60],
        wind: wind_setting(identity.wind_fps)?,
        cloud_altitude,
        weather_seed,
        redrawn_airports: [REDRAWN_AIRFIELD_SCENE, REDRAWN_TEMPLATES_AIRFIELD_SCENE]
            .contains(&identity.airfield_scene),
        redrawn_templates: identity.airfield_scene == REDRAWN_TEMPLATES_AIRFIELD_SCENE,
        city_fill: identity.city_fill_scene == CITY_FILL_SCENE,
        target: identity
            .ground_target
            .as_ref()
            .map(ground_target)
            .transpose()?,
        area: identity
            .area_defense
            .as_ref()
            .map(area_defense)
            .transpose()?,
    })
}

/// The area defenses a header holds, as the surface layout takes them.
fn area_defense(recorded: &tore_replay::AreaDefense) -> AppResult<AreaDefense> {
    let level = |what: &str, value: u8| {
        (value <= 3)
            .then_some(usize::from(value))
            .ok_or_else(|| format!("the recording's {what} strength {value} does not exist"))
    };
    Ok(AreaDefense {
        aaa: level("AAA", recorded.aaa)?,
        sam: level("SAM", recorded.sam)?,
        seed: recorded.seed,
        enemy_nationality: usize::from(recorded.enemy_nationality),
        night_stealth: recorded.night_stealth,
    })
}

/// The area defenses as a header keeps them.
fn recorded_area(area: &AreaDefense) -> tore_replay::AreaDefense {
    tore_replay::AreaDefense {
        aaa: u8::try_from(area.aaa).unwrap_or(u8::MAX),
        sam: u8::try_from(area.sam).unwrap_or(u8::MAX),
        seed: area.seed,
        enemy_nationality: u8::try_from(area.enemy_nationality).unwrap_or(u8::MAX),
        night_stealth: area.night_stealth,
    }
}

/// The ground target a header holds, as the surface resolution takes it.
fn ground_target(recorded: &tore_replay::GroundTarget) -> AppResult<GroundTarget> {
    let level = |what: &str, value: u8| {
        (value <= 3)
            .then_some(usize::from(value))
            .ok_or_else(|| format!("the recording's {what} defense level {value} does not exist"))
    };
    Ok(GroundTarget {
        stem: recorded.stem.clone(),
        aaa: level("AAA", recorded.aaa)?,
        sam: level("SAM", recorded.sam)?,
        seed: recorded.seed,
        enemy_nationality: usize::from(recorded.enemy_nationality),
        night_stealth: recorded.night_stealth,
        variation: Variation {
            jitter: recorded.jitter,
            relocate: recorded.relocate,
            front: recorded.front,
        },
        separation_nm: recorded.separation_nm,
    })
}

/// The ground target as a header keeps it.
fn recorded_target(target: &GroundTarget) -> tore_replay::GroundTarget {
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

/// Whether the world holds surface units that act: a ground target, units
/// that fire or drive, or parked aircraft. Its recording carries the surface
/// tracks (format 3).
fn has_surface(terrain: &Terrain) -> bool {
    let surface = &terrain.surface;
    surface.template.is_some()
        || !surface.parked.is_empty()
        || surface
            .units
            .iter()
            .any(|unit| unit.kind == UnitKind::Active || unit.route.is_some())
}

/// The terrain's resolved identity, as a recording's header keeps it.
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
        airfield_scene: if terrain.redrawn_templates {
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

/// Rebuilds the terrain a recording was flown in from its header's identity,
/// reading none of the environment variables a launch honours, so a replay
/// looks the same whatever the viewer's settings are. The airfields are the
/// header's airfield scene: a recording from before the redraw (no scene)
/// rebuilds the retail airfields, a newer one the redrawn airports, whatever
/// `TORE_REDRAWN_AIRPORTS` says. A scene this build does not know (a newer
/// build's) falls back to the retail airfields, with a warning in the log.
/// The city fill likewise follows the header's city fill scene, whatever
/// `TORE_CITY_FILL` says: a recording from before the fill was the default
/// (no scene) replays without it, and so does a scene this build does not
/// know, with a warning.
pub fn terrain(
    resources: &BTreeMap<String, Vec<u8>>,
    identity: &tore_replay::World,
) -> AppResult<Terrain> {
    if ![
        RETAIL_AIRFIELD_SCENE,
        REDRAWN_AIRFIELD_SCENE,
        REDRAWN_TEMPLATES_AIRFIELD_SCENE,
    ]
    .contains(&identity.airfield_scene)
    {
        log::warn!(
            "Replay: the recording's airfield scene {} is not one this build draws; showing the retail airfields",
            identity.airfield_scene
        );
    }
    if ![NO_CITY_FILL_SCENE, CITY_FILL_SCENE].contains(&identity.city_fill_scene) {
        log::warn!(
            "Replay: the recording's city fill scene {} is not one this build draws; showing no city fill",
            identity.city_fill_scene
        );
    }
    Terrain::for_recorded(resources, &recorded(identity)?)
}

/// The wind setting that resolves to exactly `fps`, bit for bit: `None` when
/// it is the generated default, otherwise the explicit heading and speed.
/// Every heading that resolves to the same vector behaves the same, because
/// the simulation reads only the vector.
fn wind_setting(fps: [f64; 3]) -> AppResult<Option<[i32; 2]>> {
    use tore_formats::flight_model::clock_rng::NativeRng;
    use tore_sim::environment::wind::Wind;
    let same = |wind: Wind| wind.world_fps().map(f64::to_bits) == fps.map(f64::to_bits);
    if same(Wind::resolve(None, &mut NativeRng::seeded(1)?)?) {
        return Ok(None);
    }
    let speed = fps[0].hypot(fps[2]).round();
    if (0. ..=200.).contains(&speed) {
        for heading in -360..=360 {
            let setting = [heading, speed as i32];
            if same(Wind::resolve(Some(setting), &mut NativeRng::seeded(1)?)?) {
                return Ok(Some(setting));
            }
        }
    }
    Err(format!("the recorded wind {fps:?} matches no wind setting").into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::theater::CELL_FEET;
    #[test]
    fn recorded_identity_restores_the_launch_settings() {
        use tore_sim::environment::{Configuration, Environment as Weather};
        let module = || {
            tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(1))
                .unwrap()
        };
        let dir = crate::replay::tests::TempDir::new("identity");
        let winds = [
            None,
            Some([-155, 20]),
            Some([90, 0]),
            Some([-360, 7]),
            Some([360, 200]),
            Some([13, 13]),
        ];
        for (n, wind) in winds.into_iter().enumerate() {
            let configuration = Configuration::new(module(), 7, 21, 3, wind).unwrap();
            let mut w = tore_world::test_support::terrain();
            w.layout = "~UKR1.MM".into();
            w.theater.name = "Ukraine (UKR1)".into();
            w.condition = Some(3);
            w.environment.layer = "DAY2.LAY".into();
            w.environment.clouds = Some(12_345);
            w.weather = tore_sim::environment::Environment::new(configuration.clone());
            let identity = of(&w);
            assert_eq!(identity.theater, "UKR");
            assert_eq!(identity.weather_name, "dawn");
            assert_eq!(identity.extent_ft, Some([f64::from(CELL_FEET); 2]));
            // Through a recording's text header and back.
            let header = tore_replay::Header {
                world: identity,
                ..Default::default()
            };
            let path = dir.path().join(format!("{n}.tore-replay"));
            let writer = tore_replay::Writer::create(&path, &header).unwrap();
            let path = writer.finish(&tore_replay::Footer::default()).unwrap();
            let identity = tore_replay::Recording::open(path)
                .unwrap()
                .header()
                .world
                .clone();
            let recorded = recorded(&identity).unwrap();
            assert_eq!(
                recorded,
                Recorded {
                    code: "~UKR1".into(),
                    condition: Some(3),
                    layer: Some("DAY2.LAY".into()),
                    time: [7, 21],
                    wind: recorded.wind,
                    cloud_altitude: 12_345,
                    weather_seed: 1,
                    redrawn_airports: false,
                    redrawn_templates: false,
                    city_fill: false,
                    target: None,
                    area: None,
                }
            );
            let rebuilt = Configuration::new(module(), 7, 21, 3, recorded.wind).unwrap();
            assert_eq!(
                rebuilt.wind_world_fps().map(f64::to_bits),
                configuration.wind_world_fps().map(f64::to_bits),
                "wind {wind:?} rebuilt as {:?}",
                recorded.wind
            );
            assert_eq!(rebuilt.wind().origin, configuration.wind().origin);
            let _ = Weather::new(rebuilt);
        }
        // No deck and the map's own weather.
        let mut w = tore_world::test_support::terrain();
        w.environment.clouds = Some(0);
        let identity = of(&w);
        assert_eq!(identity.clouds.deck_ft, None);
        assert_eq!(identity.weather, None);
        assert_eq!(identity.weather_name, "map default");
    }

    #[test]
    fn impossible_identities_are_refused() {
        let good = || {
            let mut w = tore_world::test_support::terrain();
            w.layout = "UKR.MM".into();
            of(&w)
        };
        assert!(recorded(&good()).is_ok());
        let mut bad = good();
        bad.layout = "NOWHERE.MM".into();
        assert!(recorded(&bad).is_err());
        let mut bad = good();
        bad.weather = Some(6);
        assert!(recorded(&bad).is_err());
        for seconds in [-60., 86_400., 30.5, 90., f64::NAN] {
            let mut bad = good();
            bad.time_of_day_s = seconds;
            assert!(recorded(&bad).is_err(), "{seconds}");
        }
        let mut bad = good();
        bad.clouds.deck_ft = Some(400_001.);
        assert!(recorded(&bad).is_err());
        let mut bad = good();
        bad.wind_fps = [3.3, 0., 4.4];
        assert!(recorded(&bad).is_err());
    }

    /// The airfields travel in the header: a redrawn world records scene 2
    /// (a format 3 file) and replays redrawn with the template's strip; a
    /// scene 1 recording replays redrawn with the template's strip retail; a
    /// recording from before the redraw, with no scene, replays on the
    /// retail airfields whatever the viewer's switch says, and so does a
    /// scene this build does not know.
    #[test]
    fn the_airfield_scene_is_recorded_and_rebuilt() {
        let dir = crate::replay::tests::TempDir::new("airfield-scene");
        let through_a_file = |name: &str, world: tore_replay::World| {
            let header = tore_replay::Header {
                world,
                ..Default::default()
            };
            let path = dir.path().join(format!("{name}.tore-replay"));
            let writer = tore_replay::Writer::create(&path, &header).unwrap();
            let path = writer.finish(&tore_replay::Footer::default()).unwrap();
            let recording = tore_replay::Recording::open(path).unwrap();
            let header = recording.header();
            (header.format_version, header.world.clone())
        };
        let mut w = tore_world::test_support::terrain();
        w.layout = "UKR.MM".into();
        w.redrawn_airports = true;
        w.redrawn_templates = true;
        let (format, redrawn) = through_a_file("redrawn", of(&w));
        assert_eq!(redrawn.airfield_scene, REDRAWN_TEMPLATES_AIRFIELD_SCENE);
        assert_eq!(format, 3);
        let rebuilt = recorded(&redrawn).unwrap();
        assert!(rebuilt.redrawn_airports && rebuilt.redrawn_templates);

        w.redrawn_templates = false;
        let (_, scene_1) = through_a_file("scene-1", of(&w));
        assert_eq!(scene_1.airfield_scene, REDRAWN_AIRFIELD_SCENE);
        let rebuilt = recorded(&scene_1).unwrap();
        assert!(rebuilt.redrawn_airports && !rebuilt.redrawn_templates);

        w.redrawn_airports = false;
        let (format, retail) = through_a_file("retail", of(&w));
        assert_eq!(retail.airfield_scene, RETAIL_AIRFIELD_SCENE);
        assert_eq!(format, 2, "a retail world without surface stays format 2");
        assert!(!recorded(&retail).unwrap().redrawn_airports);
        assert!(!recorded(&retail).unwrap().redrawn_templates);

        let mut unknown = redrawn;
        unknown.airfield_scene = 7;
        assert!(!recorded(&unknown).unwrap().redrawn_airports);
    }

    /// The city fill travels in the header: a world with a fill records
    /// scene 1 (a format 3 file) and replays with the fill; a recording
    /// from before the fill was the default, with no scene, replays without
    /// it, and so does a scene this build does not know.
    #[test]
    fn the_city_fill_scene_is_recorded_and_rebuilt() {
        use tore_world::city;
        let dir = crate::replay::tests::TempDir::new("city-fill-scene");
        let through_a_file = |name: &str, world: tore_replay::World| {
            let header = tore_replay::Header {
                world,
                ..Default::default()
            };
            let path = dir.path().join(format!("{name}.tore-replay"));
            let writer = tore_replay::Writer::create(&path, &header).unwrap();
            let path = writer.finish(&tore_replay::Footer::default()).unwrap();
            let recording = tore_replay::Recording::open(path).unwrap();
            let header = recording.header();
            (header.format_version, header.world.clone())
        };
        let layout = city::Layout {
            layout: "UKR.MM".into(),
            map: city::mask::LightMap::default(),
            kit: city::kit::Kit::default(),
            cities: Vec::new(),
            instances: Vec::new(),
            candidates: 0,
            inputs: Vec::new(),
        };
        let mut w = tore_world::test_support::terrain();
        w.layout = "UKR.MM".into();
        w.city_fill = Some(std::sync::Arc::new(city::CityFill::assemble(
            std::sync::Arc::new(layout),
            Vec::new(),
            Vec::new(),
        )));
        let (format, filled) = through_a_file("filled", of(&w));
        assert_eq!(filled.city_fill_scene, CITY_FILL_SCENE);
        assert_eq!(format, 3);
        assert!(recorded(&filled).unwrap().city_fill);

        w.city_fill = None;
        let (format, old) = through_a_file("old", of(&w));
        assert_eq!(old.city_fill_scene, NO_CITY_FILL_SCENE);
        assert_eq!(format, 2, "a world without fill or surface stays format 2");
        assert!(!recorded(&old).unwrap().city_fill);

        let mut unknown = filled;
        unknown.city_fill_scene = 7;
        assert!(!recorded(&unknown).unwrap().city_fill);
    }

    #[test]
    fn the_area_defenses_are_recorded_and_rebuilt() {
        let area = AreaDefense {
            aaa: 2,
            sam: 3,
            seed: 4_000_000_001,
            enemy_nationality: 33,
            night_stealth: true,
        };
        let header = recorded_area(&area);
        assert_eq!(area_defense(&header).unwrap(), area);
        assert!(
            of(&tore_world::test_support::terrain())
                .area_defense
                .is_none()
        );
        // A strength past heavy is no launch's.
        assert!(area_defense(&tore_replay::AreaDefense { sam: 4, ..header }).is_err());
    }
}
