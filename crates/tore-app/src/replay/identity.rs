//! A mission recording's record of its world: the conversions between the
//! terrain and the header's `tore_replay::World`. They live here, in the app's
//! replay code, so the terrain itself knows nothing of recordings.
use crate::{
    AppResult,
    terrain::{CONDITION_NAMES, Recorded, Terrain},
};
use std::collections::BTreeMap;
use tore_formats::theater::CELL_FEET;

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
        redrawn_airports: false,
    })
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
    }
}

/// Rebuilds the terrain a recording was flown in from its header's identity,
/// reading none of the environment variables a launch honours, so a replay
/// looks the same whatever the viewer's settings are. The one exception is
/// the AP1 experiment's `TORE_REDRAWN_AIRPORTS`, which a recording does not
/// keep: the viewer's own setting applies.
pub fn terrain(
    resources: &BTreeMap<String, Vec<u8>>,
    identity: &tore_replay::World,
) -> AppResult<Terrain> {
    let mut recorded = recorded(identity)?;
    recorded.redrawn_airports = crate::scenery::redrawn_airports()?;
    Terrain::for_recorded(resources, &recorded)
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
}
