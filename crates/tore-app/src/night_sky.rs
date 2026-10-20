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

/// How visible the stars are from the sun alone. In the evening: none while
/// it is above the horizon, all by five degrees below it. In the morning
/// (after midnight, before noon) the sun lets them stay until it is two
/// degrees up and takes them by ten, so at dawn it is the weather's star flag
/// that fades them, as the high cloud deck fades in, just as it brings them
/// at dusk as the deck fades out (John, 2026-10-20). The star flag, the
/// sky's own brightness and each star's brightness order then decide which
/// stars show (`starfield` in terrain.wgsl). Both rules give 1 at midnight
/// and 0 at noon, so the switch between them is seamless.
pub fn twilight(sun_sine: f32, morning: bool) -> f32 {
    let elevation = sun_sine.clamp(-1., 1.).asin().to_degrees();
    let t = if morning {
        ((10. - elevation) / 8.).clamp(0., 1.)
    } else {
        (-elevation / 5.).clamp(0., 1.)
    };
    t * t * (3. - 2. * t)
}

/// Whether `elapsed` mission seconds fall between midnight and noon.
pub fn morning(elapsed: f64) -> bool {
    elapsed.rem_euclid(86_400.) < 43_200.
}

// The starfield's pixel art as the sky shader draws it (`starfield` in
// terrain.wgsl), for the tests; a test checks the constants match.
#[cfg(test)]
mod star_art {
    /// A star square's side is the image height over this, and at least a pixel.
    pub const STAR_ROWS: f32 = 1080.;
    /// Every star's brightness scale (John, 2026-10-20: 5 percent down).
    pub const STAR_INTENSITY: f32 = 0.95;
    /// A cross arm's brightness against its star's.
    pub const STAR_ARM: f32 = 0.35;
    /// The sky darkness at which the faintest star starts to show; brighter
    /// stars start proportionally earlier, the brightest at once.
    pub const STAR_FIRST: f32 = 0.55;
    /// The darkness over which each star fades in.
    pub const STAR_RAMP: f32 = 0.35;

    /// A star square's side in pixels for an image `height` pixels tall.
    pub fn star_block(height: f32) -> f32 {
        (height / STAR_ROWS).max(1.)
    }

    /// The share of the pixel centred on `pixel` that a star square of side
    /// `block` centred on `center` covers (pixel units).
    pub fn star_cover(center: [f32; 2], pixel: [f32; 2], block: f32) -> f32 {
        let overlap = |i: usize| {
            let low = (center[i] - 0.5 * block).max(pixel[i] - 0.5);
            let high = (center[i] + 0.5 * block).min(pixel[i] + 0.5);
            (high - low).max(0.)
        };
        overlap(0) * overlap(1)
    }

    /// The light one pixel of `sky` shows with a star of `level` (0..1)
    /// whose square sits at `center`, with the four arms of a cross when
    /// `cross`: a whole pixel adds the level (up to white), a partly covered
    /// one its share. The sky shader works in linear light, so the star's
    /// light and centre stay the same wherever it sits.
    pub fn star_pixel(
        center: [f32; 2],
        pixel: [f32; 2],
        block: f32,
        (level, cross): (f32, bool),
        sky: f32,
    ) -> f32 {
        let level = level * STAR_INTENSITY;
        let added = |level: f32| (sky + level).min(1.) - sky;
        let mut light = added(level) * star_cover(center, pixel, block);
        if cross {
            let arms: f32 = [[block, 0.], [-block, 0.], [0., block], [0., -block]]
                .iter()
                .map(|d| star_cover([center[0] + d[0], center[1] + d[1]], pixel, block))
                .sum();
            light += added(level * STAR_ARM) * arms;
        }
        sky + light
    }

    /// How much of a star of brightness `rank` (0 faintest, 1 brightest, uniform
    /// over the stars) shows where the sky's darkness is `dark` (0..1).
    pub fn star_seen(rank: f32, dark: f32) -> f32 {
        let first = STAR_FIRST * (1. - rank);
        let t = ((dark - first) / STAR_RAMP).clamp(0., 1.);
        t * t * (3. - 2. * t)
    }
}
#[cfg(test)]
use star_art::*;

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
                night * twilight(sun_sine, morning(elapsed)),
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
        let sine = |degrees: f32| degrees.to_radians().sin();
        // Evening: none above the horizon, all by five degrees below.
        assert_eq!(twilight(0.01, false), 0.);
        assert_eq!(twilight(0., false), 0.);
        assert!((twilight(sine(-5.), false) - 1.).abs() < 1e-6);
        assert_eq!(twilight(-1., false), 1.);
        // Morning: all until two degrees up, none by ten.
        assert_eq!(twilight(sine(2.), true), 1.);
        assert!(twilight(sine(10.), true) < 1e-6);
        assert_eq!(twilight(1., true), 0.);
        assert_eq!(twilight(-1., true), 1.);
        assert!(morning(3. * 3600.) && morning(86_400. + 7. * 3600.));
        assert!(!morning(19. * 3600.) && !morning(12. * 3600.));
        for is_morning in [false, true] {
            let mut previous = twilight(sine(12.), is_morning);
            for i in 0..=20_000 {
                // 0.25 degrees a minute: one 1/60 s frame is 0.00007 degrees.
                let degrees = 12. - f64::from(i) * 0.001;
                let now = twilight(degrees.to_radians().sin() as f32, is_morning);
                assert!((now - previous).abs() < 0.001);
                previous = now;
            }
        }
    }

    /// DAY2's sun: up at 07:00, down at 19:00, 0.25 degrees a minute.
    fn day2_sun(seconds: f64) -> f32 {
        let day = seconds.rem_euclid(86_400.);
        let phase = if (7. * 3600.0..19. * 3600.).contains(&day) {
            std::f64::consts::PI * (day - 7. * 3600.) / (12. * 3600.)
        } else {
            std::f64::consts::PI * (1. + (day - 19. * 3600.).rem_euclid(86_400.) / (12. * 3600.))
        };
        phase.sin() as f32
    }

    /// DAY2's star-flag share: the night records fade out over 07:00 to
    /// 07:05 (as the dawn record's high cloud fades in) and in over 19:05 to
    /// 19:08 (as dusk's fades out).
    fn day2_night(seconds: f64) -> f32 {
        let day = seconds.rem_euclid(86_400.);
        let dawn = 1. - ((day - 7. * 3600.) / 300.).clamp(0., 1.);
        let dusk = ((day - 19. * 3600. - 300.) / 180.).clamp(0., 1.);
        (if day < 12. * 3600. { dawn } else { dusk }) as f32
    }

    fn day2_dark(seconds: f64) -> f32 {
        day2_night(seconds) * twilight(day2_sun(seconds), morning(seconds))
    }

    #[test]
    fn stars_stay_until_the_high_cloud_fades_in_at_dawn() {
        // Step a whole DAY2 day at 1/60 s: no star's strength ever jumps,
        // midnight and noon included.
        let ranks = [0., 0.5, 0.9, 0.99, 1.];
        let mut previous = ranks.map(|r| star_seen(r, day2_dark(0.)));
        assert_eq!(previous, [1.; 5]);
        let mut gone = [f64::NAN; 5];
        for frame in 1..=(24 * 3600 * 60) {
            let seconds = f64::from(frame) / 60.;
            let now = ranks.map(|r| star_seen(r, day2_dark(seconds)));
            for (i, (a, b)) in now.iter().zip(&previous).enumerate() {
                assert!((a - b).abs() < 0.002, "{seconds} {i}");
                if *a == 0. && gone[i].is_nan() {
                    gone[i] = seconds;
                }
            }
            previous = now;
        }
        // Every star is still full at 07:00 when the high cloud starts to
        // fill in; the faintest go first, the brightest last, by 07:05.
        assert!(
            ranks
                .iter()
                .all(|r| star_seen(*r, day2_dark(7. * 3600.)) == 1.)
        );
        assert!(gone.windows(2).all(|w| w[1] > w[0]), "{gone:?}");
        assert!(gone[0] > 7. * 3600. && gone[4] <= 7. * 3600. + 300.);
    }

    /// The night sky's grey behind the stars in the shader's linear light
    /// (sRGB 72 on screen).
    const SKY: f32 = 0.065;

    /// The light a star at `center` adds to each pixel around it, over `sky`.
    fn spread(center: [f32; 2], block: f32, star: (f32, bool), sky: f32) -> Vec<f32> {
        let mut pixels = Vec::new();
        for y in -8..=8 {
            for x in -8..=8 {
                let pixel = [x as f32 + 0.5, y as f32 + 0.5];
                let shown = star_pixel(center, pixel, block, star, sky);
                pixels.push(shown - sky);
            }
        }
        pixels
    }

    #[test]
    fn a_star_footprint_keeps_its_light_wherever_it_sits() {
        for sky in [0., SKY, 0.6] {
            for block in [1., 4. / 3., 2.] {
                for cross in [false, true] {
                    let added = |level: f32| (sky + level).min(1.) - sky;
                    let whole = |level: f32| {
                        let l = level * STAR_INTENSITY;
                        let arms = if cross { 4. * added(l * STAR_ARM) } else { 0. };
                        block * block * (added(l) + arms)
                    };
                    for i in 0..200 {
                        let center = [0.5 + i as f32 * 0.0137, 0.31 + i as f32 * 0.0071];
                        for level in [1., 0.4] {
                            let sum: f32 = spread(center, block, (level, cross), sky).iter().sum();
                            assert!(
                                (sum - whole(level)).abs() < 2e-4,
                                "{sky} {block} {cross} {center:?} {sum}"
                            );
                        }
                    }
                }
            }
        }
        // Aligned to the pixels, a star adds its level to whole pixels.
        let shown = star_pixel([0.5, 0.5], [0.5, 0.5], 1., (0.6, false), SKY);
        assert!((shown - SKY - 0.6 * STAR_INTENSITY).abs() < 1e-5);
        // and stops at white, as the display would clip it.
        let shown = star_pixel([0.5, 0.5], [0.5, 0.5], 1., (1., false), SKY);
        assert!((shown - 1.).abs() < 1e-6);
        let star =
            |pixel: [f32; 2], cross: bool| star_pixel([0.5, 0.5], pixel, 1., (1., cross), SKY);
        assert!((star([1.5, 0.5], false) - SKY).abs() < 1e-6);
        assert!((star([1.5, 0.5], true) - SKY - STAR_INTENSITY * STAR_ARM).abs() < 1e-5);
        // Five percent down from before (a whole block added the full level).
        assert!((STAR_INTENSITY - 0.95).abs() < 1e-6);
    }

    #[test]
    fn a_star_moves_continuously_across_pixels() {
        // A slow pan: 1/1000 of a pixel a frame never jumps any pixel's light
        // by more than the distance allows (no snapping to a grid).
        for block in [1., 2.] {
            let mut previous = spread([0.2, 0.7], block, (1., true), SKY);
            for i in 1..=3000 {
                let t = i as f32 * 0.001;
                let now = spread([0.2 + t, 0.7 + 0.5 * t], block, (1., true), SKY);
                let step = now
                    .iter()
                    .zip(&previous)
                    .map(|(a, b)| (a - b).abs())
                    .fold(0., f32::max);
                assert!(step < 0.005, "{block} {t} {step}");
                previous = now;
            }
        }
        // The light's centre follows the star exactly.
        for block in [1., 2.] {
            for i in 0..50 {
                let center = [0.5 + i as f32 * 0.043, 0.5 - i as f32 * 0.029];
                let pixels = spread(center, block, (0.6, true), SKY);
                let (mut sum, mut x, mut y) = (0., 0., 0.);
                for (n, light) in pixels.iter().enumerate() {
                    sum += light;
                    x += light * ((n % 17) as f32 - 8. + 0.5);
                    y += light * ((n / 17) as f32 - 8. + 0.5);
                }
                assert!(
                    (x / sum - center[0]).abs() < 1e-3 && (y / sum - center[1]).abs() < 1e-3,
                    "{block} {center:?}"
                );
            }
        }
    }

    #[test]
    fn star_squares_are_a_quarter_of_the_old_blocks() {
        // Formerly 1/540 of the height: 2 pixels at 1080p, 4 at 2160p.
        assert_eq!(star_block(1080.), 1.);
        assert_eq!(star_block(2160.), 2.);
        assert_eq!(star_block(720.), 1.);
        assert!((star_block(1440.) - 4. / 3.).abs() < 1e-6);
    }

    #[test]
    fn the_brightest_stars_show_first_and_go_last() {
        // Darkening sky: each star appears at its own darkness, brightest first.
        let appears = |rank: f32| {
            (0..=1000)
                .map(|i| i as f32 / 1000.)
                .find(|dark| star_seen(rank, *dark) > 0.)
                .unwrap()
        };
        let ranks: Vec<f32> = (0..=20).map(|i| i as f32 / 20.).collect();
        for pair in ranks.windows(2) {
            assert!(appears(pair[1]) < appears(pair[0]));
        }
        assert!(appears(1.) < 0.01);
        for i in 0..=100 {
            let dark = i as f32 / 100.;
            for pair in ranks.windows(2) {
                assert!(star_seen(pair[1], dark) >= star_seen(pair[0], dark));
            }
        }
        // A full night shows every star at full strength.
        for rank in ranks {
            assert_eq!(star_seen(rank, 0.9), 1.);
            assert_eq!(star_seen(rank, 0.), 0.);
        }
    }

    #[test]
    fn stars_come_in_with_the_dusk_without_a_step() {
        // DAY2's dusk: sunset 19:00 then the sun sinks 0.25 degrees a minute;
        // the star-flag share (the night record, as the high cloud deck fades
        // out) rises over its 19:05 to 19:08 overlap. Step every 1/60 s.
        let dark_at = |seconds: f64| {
            let since = seconds - 19. * 3600.;
            let sun = (-since / 240.).to_radians().sin() as f32;
            let night = ((since - 300.) / 180.).clamp(0., 1.) as f32;
            night * twilight(sun, false)
        };
        let ranks = [0., 0.5, 0.9, 0.99, 1.];
        let mut previous = ranks.map(|r| star_seen(r, dark_at(18. * 3600.)));
        assert_eq!(previous, [0.; 5]);
        let mut first_seen = [f64::NAN; 5];
        for frame in 0..(90 * 60 * 60) {
            let seconds = 18.5 * 3600. + f64::from(frame) / 60.;
            let now = ranks.map(|r| star_seen(r, dark_at(seconds)));
            for (i, (a, b)) in now.iter().zip(&previous).enumerate() {
                assert!((a - b).abs() < 0.002, "{seconds} {i}");
                if *a > 0. && first_seen[i].is_nan() {
                    first_seen[i] = seconds;
                }
            }
            previous = now;
        }
        // The brightest show while the high cloud is still fading (by 19:08),
        // the faintest follow, and all are in by full night.
        assert!(first_seen[4] < 19. * 3600. + 8. * 60.);
        assert!(first_seen.windows(2).all(|w| w[1] < w[0]));
        assert_eq!(previous, [1.; 5]);
    }

    #[test]
    fn the_shader_draws_the_same_stars() {
        let shader = include_str!("terrain.wgsl");
        for line in [
            format!("const STAR_ROWS:f32={STAR_ROWS:.1};"),
            format!("const STAR_INTENSITY:f32={STAR_INTENSITY:.2};"),
            format!("const STAR_ARM:f32={STAR_ARM:.2};"),
            format!("const STAR_FIRST:f32={STAR_FIRST:.2};"),
            format!("const STAR_RAMP:f32={STAR_RAMP:.2};"),
        ] {
            assert!(shader.contains(&line), "{line}");
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
