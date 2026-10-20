//! Presentation only: the turning night sky of smooth weather, a moon that
//! rises and sets and the starfield's place and visibility.
//!
//! The sky turns once a sidereal day about the celestial pole, which stands
//! at the theater's latitude above the north horizon. The moon keeps its
//! original weather-record direction at the mission's nearest midnight and
//! moves with the sky from there, lagging the stars by a lunar day, so it
//! rises in the east and sets in the west. Everything follows from the
//! mission clock and the theater, so a replay shows the same sky. The
//! starfield itself is drawn in the sky shader (`starfield` in terrain.wgsl).
//! See docs/spec/sun-glow.md, "Night sky".
use std::f64::consts::TAU;

/// Seconds in a sidereal day: the stars' turn.
const SIDEREAL_DAY: f64 = 86_164.09;
/// Seconds in a mean lunar day: the moon's turn, about 50 minutes longer.
const LUNAR_DAY: f64 = 89_428.33;
/// The sky's turn at midnight of the mission's first day (opinionated: puts
/// the starfield's brighter band high across a midnight sky).
const MIDNIGHT_TURN: f64 = 0.9;
/// Latitude for a map outside the theater table, degrees north.
const DEFAULT_LATITUDE: f64 = 40.;

/// The night sky at one instant.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NightSky {
    /// The shader's `stars` vector: sine and cosine of latitude, the sky's
    /// turn in radians (0..2 pi) and the starfield's visibility.
    pub stars: [f32; 4],
    /// The moon's unit direction (x east, y up, z north) and visibility.
    pub moon: [f32; 4],
    /// How the moon art is placed: the record's angles, then this turn about
    /// the pole ([`Self::place_moon`]).
    pub moon_angles: [i16; 2],
    pub moon_turn: f64,
}

/// The theater's latitude in degrees north, from the replay export's
/// theater anchors (the world itself is flat and carries none).
pub fn latitude(layout: &str) -> f64 {
    tore_replay::export::acmi::theater_anchor(layout).map_or(DEFAULT_LATITUDE, |a| a.latitude)
}

/// Mission clock seconds since midnight of the launch day, without the
/// daily wrap, so the sky keeps turning through midnight.
pub fn elapsed(weather: &tore_sim::environment::Environment) -> f64 {
    f64::from(weather.configuration().start_seconds()) + weather.ticks() as f64 / 256.
}

/// The sky's turn angle at `elapsed` seconds, in 0..2 pi.
pub fn turn(elapsed: f64) -> f64 {
    (MIDNIGHT_TURN + TAU * elapsed / SIDEREAL_DAY).rem_euclid(TAU)
}

/// Rotates `v` about the unit axis `pole` by `angle` (right-handed).
fn rotate(v: [f64; 3], pole: [f64; 3], angle: f64) -> [f64; 3] {
    let (s, c) = angle.sin_cos();
    let dot = v[0] * pole[0] + v[1] * pole[1] + v[2] * pole[2];
    let cross = [
        pole[1] * v[2] - pole[2] * v[1],
        pole[2] * v[0] - pole[0] * v[2],
        pole[0] * v[1] - pole[1] * v[0],
    ];
    std::array::from_fn(|i| v[i] * c + cross[i] * s + pole[i] * dot * (1. - c))
}

/// The moon's direction at `elapsed` seconds for a mission that started at
/// `start` seconds of its first day: the record's `anchor` direction at the
/// nearest midnight, carried round the pole at the lunar rate.
pub fn moon_direction(anchor: [f32; 3], latitude: f64, start: f64, elapsed: f64) -> [f32; 3] {
    about_pole(anchor, latitude, moon_turn(start, elapsed))
}

/// The moon's turn about the pole since the mission's nearest midnight.
pub fn moon_turn(start: f64, elapsed: f64) -> f64 {
    let midnight = (start / 86_400.).round() * 86_400.;
    TAU * (elapsed - midnight) / LUNAR_DAY
}

/// `v` turned `angle` about the celestial pole at `latitude`.
fn about_pole(v: [f32; 3], latitude: f64, angle: f64) -> [f32; 3] {
    let phi = latitude.to_radians();
    rotate(v.map(f64::from), [0., phi.sin(), phi.cos()], angle).map(|v| v as f32)
}

/// How visible the stars are from the sun alone: none until it is two
/// degrees below the horizon, all by six. The sky's own brightness and the
/// weather's star flag dim them further.
pub fn twilight(sun_sine: f32) -> f32 {
    let elevation = sun_sine.clamp(-1., 1.).asin().to_degrees();
    let t = ((-2. - elevation) / 4.).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

impl NightSky {
    /// `night` is the weather records' star-flag weight (`SkyMix::night`),
    /// `anchor` the moon's original record direction, `sun` the smooth sun's
    /// direction sine of elevation.
    /// `moon_angles` places the moon art's `center` direction at midnight.
    pub fn new(
        layout: &str,
        weather: &tore_sim::environment::Environment,
        (moon_angles, center): ([i16; 2], [f32; 3]),
        night: f32,
        sun_sine: f32,
    ) -> Self {
        let latitude = latitude(layout);
        let elapsed = elapsed(weather);
        let start = f64::from(weather.configuration().start_seconds());
        let phi = latitude.to_radians();
        let anchor = crate::celestial::rotate(center, moon_angles);
        let moon = moon_direction(anchor, latitude, start, elapsed);
        Self {
            stars: [
                phi.sin() as f32,
                phi.cos() as f32,
                turn(elapsed) as f32,
                night * twilight(sun_sine),
            ],
            moon: [moon[0], moon[1], moon[2], night],
            moon_angles,
            moon_turn: moon_turn(start, elapsed),
        }
    }

    /// A point of the moon art, as the sky shows it now.
    pub fn place_moon(&self, p: [f32; 3]) -> [f32; 3] {
        let latitude = f64::from(self.stars[0])
            .atan2(f64::from(self.stars[1]))
            .to_degrees();
        about_pole(
            crate::celestial::rotate(p, self.moon_angles),
            latitude,
            self.moon_turn,
        )
    }

    /// The night sky a camera at `altitude` sees, from the weather clock,
    /// the theater and the sky's star-flag weight.
    /// `center` is the moon art's direction from the eye (`Celestial::moon_center`).
    pub fn for_view(
        world: &crate::terrain::Terrain,
        night: f32,
        altitude: f64,
        center: [f32; 3],
    ) -> Option<Self> {
        let layer = world.weather.sample(altitude)?;
        let sun =
            crate::celestial::visual_sun_direction(&layer, &world.weather).map_or(-1., |d| d[1]);
        // The record that draws the moon fixes its midnight place.
        let anchor = world
            .weather
            .configuration()
            .layers()
            .iter()
            .find(|l| l.flags & crate::sky_mix::STARS_FLAG != 0)
            .unwrap_or(&layer);
        let angles = [anchor.moon_azimuth, anchor.moon_elevation];
        Some(Self::new(
            &world.layout,
            &world.weather,
            (angles, center),
            night,
            sun,
        ))
    }

    /// The moon's light direction for surfaces, faded out as it sets so the
    /// ground is never lit from below.
    pub fn moon_light(&self) -> [f32; 3] {
        let [x, y, z, _] = self.moon;
        let t = ((y + 0.05) / 0.15).clamp(0., 1.);
        let fade = t * t * (3. - 2. * t);
        [x * fade, y * fade, z * fade]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elevation(d: [f32; 3]) -> f32 {
        d[1].clamp(-1., 1.).asin().to_degrees()
    }

    #[test]
    fn the_moon_rises_in_the_east_and_sets_in_the_west() {
        // On the celestial equator, due south at midnight, at 48 degrees north.
        let lat = 48_f64;
        let anchor = [
            0.,
            (90. - lat).to_radians().sin() as f32,
            -(90. - lat).to_radians().cos() as f32,
        ];
        let at = |hours: f64| moon_direction(anchor, lat, 0., hours * 3600.);
        let midnight = at(0.);
        assert!((elevation(midnight) - 42.).abs() < 0.01);
        // Six lunar hours before and after, it sits on the horizon: east, then west.
        let quarter = LUNAR_DAY / 4. / 3600.;
        let rising = at(-quarter);
        let setting = at(quarter);
        assert!(elevation(rising).abs() < 0.01 && rising[0] > 0.99);
        assert!(elevation(setting).abs() < 0.01 && setting[0] < -0.99);
        // It climbs through the evening and falls after midnight.
        assert!(elevation(at(-3.)) < elevation(at(-1.)));
        assert!(elevation(at(1.)) > elevation(at(3.)));
    }

    #[test]
    fn the_moon_moves_smoothly_and_lags_the_stars() {
        let anchor = [0.3_f32, 0.5, -0.81];
        let mut previous = moon_direction(anchor, 30., 68_460., 68_460.);
        for frame in 1..=(120 * 600) {
            let t = 68_460. + f64::from(frame) / 120.;
            let now = moon_direction(anchor, 30., 68_460., t);
            let step: f32 = previous
                .iter()
                .zip(now)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f32>()
                .sqrt();
            // 360 degrees a lunar day is 0.0035 degrees a frame at most.
            assert!(step < 0.0035_f32.to_radians() * 1.01, "{step}");
            previous = now;
        }
    }

    #[test]
    fn the_sky_turns_continuously_through_midnight_and_is_repeatable() {
        let before = turn(86_400. - 1. / 120.);
        let after = turn(86_400.);
        let step = (after - before).rem_euclid(TAU);
        assert!(step > 0. && step < 1e-4);
        assert_eq!(turn(12_345.6), turn(12_345.6));
        assert!((turn(SIDEREAL_DAY) - turn(0.)).abs() < 1e-9);
    }

    #[test]
    fn stars_fade_through_twilight_without_a_step() {
        assert_eq!(twilight(0.), 0.);
        assert_eq!(twilight(-2_f32.to_radians().sin()), 0.);
        assert!((twilight(-6_f32.to_radians().sin()) - 1.).abs() < 1e-6);
        assert_eq!(twilight(-1.), 1.);
        let mut previous = twilight(0.1);
        for i in 0..=20_000 {
            // 0.25 degrees a minute: one 1/60 s frame is 0.00007 degrees.
            let degrees = 6. - f64::from(i) * 0.001;
            let now = twilight(degrees.to_radians().sin() as f32);
            assert!((now - previous).abs() < 0.001);
            previous = now;
        }
    }

    #[test]
    fn moonlight_fades_as_the_moon_sets() {
        let sky = |y: f32| NightSky {
            stars: [0.; 4],
            moon: [0., y, (1. - y * y).sqrt(), 1.],
            moon_angles: [0; 2],
            moon_turn: 0.,
        };
        assert_eq!(sky(-0.2).moon_light(), [0.; 3]);
        assert!(sky(0.3).moon_light()[1] > 0.29);
        assert!(sky(0.02).moon_light()[1] < 0.02);
    }

    #[test]
    fn the_moon_art_follows_its_direction() {
        let mut world = tore_world::test_support::terrain();
        let mut module =
            tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(1))
                .unwrap();
        let layer = &mut module.layers[0];
        (layer.start_seconds, layer.end_seconds) = (0, 86400);
        (layer.low_feet, layer.high_feet) = (0, i32::MAX);
        (layer.moon_azimuth, layer.moon_elevation) = (8192, 3640);
        world.weather = tore_sim::environment::Environment::new(
            tore_sim::environment::Configuration::new(module, 23, 0, 0, None).unwrap(),
        );
        let center = [0.1_f32, 0.05, 0.99];
        let length = center.iter().map(|v| v * v).sum::<f32>().sqrt();
        let sky = NightSky::for_view(&world, 1., 1000., center.map(|v| v / length)).unwrap();
        let placed = sky.place_moon(center).map(|v| v / length);
        for i in 0..3 {
            assert!(
                (placed[i] - sky.moon[i]).abs() < 1e-5,
                "{placed:?} {:?}",
                sky.moon
            );
        }
    }

    #[test]
    fn the_same_clock_gives_the_same_sky() {
        // The starfield is a pure function of this vector in the shader, so
        // equal clocks (a flight and its replay) draw the same stars.
        let module =
            tore_formats::weather::Module::parse(&tore_formats::weather::synthetic_module(1))
                .unwrap();
        let run = |queries: bool| {
            let mut weather = tore_sim::environment::Environment::new(
                tore_sim::environment::Configuration::new(module.clone(), 23, 50, 0, None).unwrap(),
            );
            let mut skies = Vec::new();
            for tick in 0..120 * 1200 {
                weather.step();
                if queries || tick % 600 == 0 {
                    let sky =
                        NightSky::new("UKR.MM", &weather, ([8192, 3640], [0., 0., 1.]), 1., -0.5);
                    if tick % 600 == 0 {
                        skies.push(sky);
                    }
                }
            }
            skies
        };
        assert_eq!(run(false), run(true));
    }

    #[test]
    fn theaters_have_their_own_latitude() {
        assert!((latitude("UKR.MM") - latitude("~UKR3")).abs() < 1e-9);
        assert!(latitude("EGY.MM") < latitude("BAL.MM"));
        assert_eq!(latitude("NOWHERE"), DEFAULT_LATITUDE);
    }
}
