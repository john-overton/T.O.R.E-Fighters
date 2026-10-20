//! Presentation only: what the sky draws through dawn and dusk, as a
//! continuous mixture of the weather records that overlap in time.
//!
//! The weather clock crosses dawn and dusk by overlapping records. Their
//! colours already blend continuously in smooth weather
//! (`tore_sim::environment::visual`), but the native blend copies a record's
//! sky and ocean decks whole the moment it starts to count, switches the
//! horizon gradients with them, and resolves the haze through the nearest of a
//! few fixed shade tables. Each of those steps showed as a pop: the high cloud
//! deck appearing at once about five minutes into a dawn mission, its orange
//! horizon gradient vanishing with it. Here every record that counts at an
//! instant gets a weight, and the sky fades each deck, gradient, haze colour
//! and star flag by those weights instead. The simulation's weather, sensors
//! and visibility keep reading the native records; nothing here reaches them.
//! See docs/spec/sun-glow.md, "Continuous dawn and dusk".
use std::collections::BTreeMap;
use tore_formats::weather::{Deck, Layer};

/// The weather record flag that draws the stars and the moon.
pub const STARS_FLAG: u16 = 0x10;

/// Each record's share of the picture at `seconds` (fractional time of day)
/// and `altitude` feet, in record order; the shares sum to one, or the list is
/// empty when no record covers the instant. Mirrors the colour blend of
/// `Environment::visual_sample`: records sharing a floor cross over their
/// time overlap, then bands cross over their altitude overlap.
pub fn shares(records: &[Layer], seconds: f64, altitude: f64) -> Vec<(usize, f64)> {
    struct Band {
        low: i32,
        high: i32,
        end: i32,
        shares: Vec<(usize, f64)>,
    }
    let whole = seconds.floor() as i32;
    let mut bands: Vec<Band> = Vec::new();
    for (index, record) in records.iter().enumerate() {
        if !record.covers_time(whole) {
            continue;
        }
        if let Some(previous) = bands.last_mut()
            && previous.low == record.low_feet
        {
            let span = f64::from(previous.end - record.start_seconds);
            let t = if span > 0. {
                ((seconds - f64::from(record.start_seconds)) / span).clamp(0., 1.)
            } else {
                1.
            };
            for share in &mut previous.shares {
                share.1 *= 1. - t;
            }
            previous.shares.push((index, t));
            previous.high = previous.high.max(record.high_feet);
            previous.end = previous.end.max(record.end_seconds);
        } else {
            bands.push(Band {
                low: record.low_feet,
                high: record.high_feet,
                end: record.end_seconds,
                shares: vec![(index, 1.)],
            });
        }
    }
    let altitude = if altitude.is_finite() {
        altitude.max(0.)
    } else {
        0.
    };
    let feet = altitude.floor().clamp(0., f64::from(i32::MAX)) as i32;
    let mut result: Option<Band> = None;
    for band in bands {
        if !(band.low <= feet && feet <= band.high) {
            continue;
        }
        if let Some(previous) = &mut result {
            let span = f64::from(previous.high - band.low);
            let t = if span > 0. {
                ((altitude - f64::from(band.low)) / span).clamp(0., 1.)
            } else {
                1.
            };
            for share in &mut previous.shares {
                share.1 *= 1. - t;
            }
            previous
                .shares
                .extend(band.shares.into_iter().map(|(i, w)| (i, w * t)));
            previous.high = previous.high.max(band.high);
        } else {
            result = Some(band);
        }
    }
    let mut shares = result.map_or_else(Vec::new, |band| band.shares);
    let total: f64 = shares.iter().map(|s| s.1).sum();
    if total > 0. {
        for share in &mut shares {
            share.1 /= total;
        }
    }
    shares
}

/// One deck slot as drawn: the texture with the most weight, a second
/// texture it crosses into, and how much of the slot is drawn at all.
#[derive(Clone, Debug, PartialEq)]
pub struct DeckMix {
    /// The heaviest named deck, or `None` when no record names one.
    pub deck: Option<Deck>,
    /// Texture layers for `deck` and the second deck it crosses into (-1
    /// when there is none), and the second's share of the two.
    pub texture: f32,
    pub second: f32,
    pub mix: f32,
    /// The named decks' total weight: 0 where no record draws this slot.
    pub opacity: f32,
}

impl DeckMix {
    /// `[altitude, tile size, texture, 0]`, the shader's deck vector.
    pub fn uniform(&self) -> [f32; 4] {
        match &self.deck {
            Some(deck) => [
                deck.altitude_feet as f32,
                2_f32.powi(deck.tile_exponent),
                self.texture,
                0.,
            ],
            None => [0., 1., -1., 0.],
        }
    }
    pub fn is_ocean(&self) -> bool {
        self.deck
            .as_ref()
            .is_some_and(|deck| deck.name.starts_with("OCEAN"))
    }
}

/// The sky's continuous mixture at one instant and altitude.
#[derive(Clone, Debug, PartialEq)]
pub struct SkyMix {
    pub decks: [DeckMix; 2],
    /// How much of the upper (sky) and lower (ground) horizon gradients are
    /// drawn: 1 where the native picture draws them, 0 where a deck hides them.
    pub upper_band: f32,
    pub lower_band: f32,
    /// The weight of records that draw the stars and moon (`STARS_FLAG`).
    pub night: f32,
    /// Each counting record's haze shade and weight, which pick and cross
    /// the weather art's shade remap rows.
    pub shades: Vec<([u8; 3], f32)>,
}

impl SkyMix {
    /// The stepped picture's own mixture: whole decks and switched bands.
    pub fn stepped(layer: &Layer, textures: &BTreeMap<String, usize>, altitude: f64) -> Self {
        let horizon = tore_sim::environment::horizon::Horizon::new(layer, altitude);
        Self {
            decks: std::array::from_fn(|slot| {
                let deck = &layer.decks[slot];
                let texture = textures.get(&deck.name).map_or(-1., |i| *i as f32);
                DeckMix {
                    deck: (!deck.name.is_empty()).then(|| deck.clone()),
                    texture,
                    second: -1.,
                    mix: 0.,
                    opacity: f32::from(texture >= 0.),
                }
            }),
            upper_band: f32::from(u8::from(horizon.upper_band)),
            lower_band: f32::from(u8::from(horizon.lower_band)),
            night: f32::from(u8::from(layer.flags & STARS_FLAG != 0)),
            shades: vec![(layer.shade, 1.)],
        }
    }

    /// The smooth picture: each deck, band and star flag weighted by the
    /// records' `shares`.
    pub fn new(
        records: &[Layer],
        shares: &[(usize, f64)],
        textures: &BTreeMap<String, usize>,
        altitude: f64,
    ) -> Self {
        let decks: [DeckMix; 2] = std::array::from_fn(|slot| {
            // Weight per named deck, in the order each first appears.
            let mut named: Vec<(&Deck, f64)> = Vec::new();
            for &(record, weight) in shares {
                let deck = &records[record].decks[slot];
                if weight <= 0. || deck.name.is_empty() || !textures.contains_key(&deck.name) {
                    continue;
                }
                match named.iter_mut().find(|(d, _)| d.name == deck.name) {
                    Some(entry) => entry.1 += weight,
                    None => named.push((deck, weight)),
                }
            }
            named.sort_by(|a, b| b.1.total_cmp(&a.1));
            let Some(&(first, w1)) = named.first() else {
                return DeckMix {
                    deck: None,
                    texture: -1.,
                    second: -1.,
                    mix: 0.,
                    opacity: 0.,
                };
            };
            // A second deck crosses over in place only on the same plane.
            let second = named.get(1).filter(|(d, _)| {
                d.altitude_feet == first.altitude_feet && d.tile_exponent == first.tile_exponent
            });
            let w2 = second.map_or(0., |s| s.1);
            DeckMix {
                deck: Some(first.clone()),
                texture: textures[&first.name] as f32,
                second: second.map_or(-1., |(d, _)| textures[&d.name] as f32),
                mix: (w2 / (w1 + w2)) as f32,
                opacity: ((w1 + w2).min(1.)) as f32,
            }
        });
        let feet = altitude.floor().max(0.);
        // Horizon::new: the upper band shows unless a sky deck is overhead;
        // the lower band unless the eye is above the sky deck or an ocean.
        let sky_overhead = decks[0].deck.as_ref().map_or(0., |deck| {
            decks[0].opacity * f32::from(u8::from(feet < f64::from(deck.altitude_feet)))
        });
        let above_sky = decks[0].deck.as_ref().map_or(0., |deck| {
            decks[0].opacity * f32::from(u8::from(feet >= f64::from(deck.altitude_feet)))
        });
        let ocean = decks[1].deck.as_ref().map_or(0., |deck| {
            decks[1].opacity * f32::from(u8::from(feet > f64::from(deck.altitude_feet)))
        });
        let night = shares
            .iter()
            .filter(|(record, _)| records[*record].flags & STARS_FLAG != 0)
            .map(|(_, weight)| *weight)
            .sum::<f64>() as f32;
        Self {
            decks,
            upper_band: 1. - sky_overhead,
            lower_band: (1. - above_sky) * (1. - ocean),
            night: night.clamp(0., 1.),
            shades: shares
                .iter()
                .map(|&(record, weight)| (records[record].shade, weight as f32))
                .collect(),
        }
    }

    /// The shader's `sky_mix` vectors: per deck slot its opacity, second
    /// texture and mix, then the upper and lower band weights.
    pub fn uniform(&self) -> [f32; 8] {
        let [a, b] = &self.decks;
        [
            a.opacity,
            a.second,
            a.mix,
            self.upper_band,
            b.opacity,
            b.second,
            b.mix,
            self.lower_band,
        ]
    }
}

/// A haze colour in six-bit components and its fog palette levels.
pub type Haze = ([f64; 3], Vec<[[u8; 3]; 256]>);

/// The haze colour (six-bit components) and fog palette levels as a
/// weighted mixture of each record's own shade table, so the haze does not
/// jump from one fixed table to the next part way through a crossing.
pub fn haze(
    records: &[Layer],
    shares: &[(usize, f64)],
    configuration: &tore_sim::environment::Configuration,
    palette: &[[u8; 3]; 256],
) -> Option<Haze> {
    // Every contributing table on the longest one's levels, so the level
    // count holds steady while the shares cross.
    let levels = shares
        .iter()
        .filter(|(_, weight)| *weight > 0.)
        .map(|(record, _)| {
            configuration
                .shade_remap(records[*record].shade)
                .levels
                .len()
        })
        .max()?;
    let mut colour = [0.; 3];
    let mut fog = vec![[[0_f64; 3]; 256]; levels];
    for &(record, weight) in shares {
        let shade = records[record].shade;
        for (out, c) in colour.iter_mut().zip(shade) {
            *out += f64::from(c) * weight;
        }
        let remap = configuration.shade_remap(shade);
        let count = remap.levels.len();
        if count == 0 || weight <= 0. {
            continue;
        }
        for (level, out) in fog.iter_mut().enumerate() {
            // The shader reads level `haze * levels`: the same haze reads
            // `haze * count` of a shorter table, between two of its levels.
            let at = (level as f64 * count as f64 / levels as f64).min((count - 1) as f64);
            let (low, t) = (at.floor() as usize, at.fract());
            let high = (low + 1).min(count - 1);
            for ((entry, a), b) in out
                .iter_mut()
                .zip(remap.levels[low])
                .zip(remap.levels[high])
            {
                let [a, b] = [a, b].map(|i| palette[usize::from(i)]);
                for k in 0..3 {
                    entry[k] += (f64::from(a[k]) * (1. - t) + f64::from(b[k]) * t) * weight;
                }
            }
        }
    }
    Some((
        colour,
        fog.into_iter()
            .map(|level| level.map(|c| c.map(|v| v.round().clamp(0., 255.) as u8)))
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_formats::weather::Module;

    /// DAY2's shape: night, dawn, day, dusk, night, with the day and dusk
    /// records naming different sky textures and dawn naming only an ocean.
    pub(crate) fn day2() -> (Vec<Layer>, BTreeMap<String, usize>) {
        let module = Module::parse(&tore_formats::weather::synthetic_module(5)).unwrap();
        let mut layers = module.layers;
        let spans = [
            (0, 25500, 0x72),
            (25200, 25800, 0x2e),
            (25560, 68640, 0x2e),
            (68400, 68880, 0x2e),
            (68700, i32::MAX, 0x72),
        ];
        let sky = ["", "", "SKY3.PIC", "SKY7.PIC", ""];
        let ocean = ["", "OCEAN2.PIC", "OCEAN2.PIC", "OCEAN5.PIC", ""];
        for (i, layer) in layers.iter_mut().enumerate() {
            (layer.start_seconds, layer.end_seconds, layer.flags) = spans[i];
            layer.low_feet = 0;
            layer.high_feet = i32::MAX;
            layer.decks[0] = Deck {
                name: sky[i].into(),
                altitude_feet: 75000,
                tile_exponent: 17,
            };
            layer.decks[1] = Deck {
                name: ocean[i].into(),
                altitude_feet: 0,
                tile_exponent: 15,
            };
        }
        let textures = ["SKY3.PIC", "SKY7.PIC", "OCEAN2.PIC", "OCEAN5.PIC"]
            .into_iter()
            .enumerate()
            .map(|(i, n)| (n.to_string(), i + 1))
            .collect();
        (layers, textures)
    }

    fn sample(layers: &[Layer], textures: &BTreeMap<String, usize>, seconds: f64) -> SkyMix {
        SkyMix::new(layers, &shares(layers, seconds, 10000.), textures, 10000.)
    }

    /// Every drawn quantity, with the second texture's weight in its own
    /// terms so a crossover is judged by what reaches the screen.
    fn drawn(mix: &SkyMix) -> Vec<f32> {
        let mut out = vec![mix.upper_band, mix.lower_band, mix.night];
        for deck in &mix.decks {
            out.push(deck.opacity * (1. - deck.mix));
            out.push(deck.opacity * deck.mix);
        }
        out
    }

    #[test]
    fn shares_sum_to_one_and_follow_the_overlap() {
        let (layers, _) = day2();
        for seconds in [100., 25300., 25600., 40000., 68500., 68800., 80000.] {
            let s = shares(&layers, seconds, 5000.);
            let total: f64 = s.iter().map(|s| s.1).sum();
            assert!((total - 1.).abs() < 1e-9, "{seconds}: {s:?}");
        }
        // Half way through the night-to-dawn overlap.
        let s = shares(&layers, 25350., 0.);
        assert_eq!(s.len(), 2);
        assert!((s[0].1 - 0.5).abs() < 1e-9 && (s[1].1 - 0.5).abs() < 1e-9);
    }

    #[test]
    fn a_full_day_has_no_step_between_frames() {
        // 1/60 s frames across every transition, plus the whole day at 1 s.
        let (layers, textures) = day2();
        let mut times: Vec<f64> = (0..86400).map(f64::from).collect();
        for start in [25100., 25500., 68300., 68600.] {
            times.extend((0..60 * 600).map(|i| start + f64::from(i) / 60.));
        }
        times.sort_by(f64::total_cmp);
        let mut previous: Option<(f64, Vec<f32>)> = None;
        for t in times {
            let now = drawn(&sample(&layers, &textures, t));
            if let Some((when, before)) = &previous {
                // A 240 s crossover moves about 0.0042 a second.
                let limit = (0.006 * (t - when)).max(1e-4) as f32;
                for (a, b) in before.iter().zip(&now) {
                    assert!((a - b).abs() <= limit, "step at {t}: {before:?} -> {now:?}");
                }
            }
            previous = Some((t, now));
        }
    }

    #[test]
    fn dawn_fades_the_sky_deck_in_and_dusk_crosses_textures() {
        let (layers, textures) = day2();
        let before = sample(&layers, &textures, 25559.);
        assert_eq!(before.decks[0].opacity, 0.);
        assert_eq!(before.upper_band, 1.);
        let middle = sample(&layers, &textures, 25680.);
        assert!((middle.decks[0].opacity - 0.5).abs() < 1e-3);
        assert!((middle.upper_band - 0.5).abs() < 1e-3);
        let day = sample(&layers, &textures, 30000.);
        assert_eq!(day.decks[0].opacity, 1.);
        assert_eq!(day.decks[0].texture, 1.);
        assert_eq!(day.upper_band, 0.);
        // Day into dusk: SKY3 crosses into SKY7 on the same plane.
        let dusk = sample(&layers, &textures, 68520.);
        assert_eq!(dusk.decks[0].opacity, 1.);
        assert!(dusk.decks[0].second >= 0. && dusk.decks[0].mix > 0.);
        // Stars arrive with the night record's weight, not at its midpoint.
        let night = sample(&layers, &textures, 68790.);
        assert!((night.night - 0.5).abs() < 1e-3);
    }

    #[test]
    fn stepped_mix_matches_the_native_switches() {
        let (layers, textures) = day2();
        let stepped = SkyMix::stepped(&layers[2], &textures, 10000.);
        assert_eq!(stepped.decks[0].opacity, 1.);
        assert_eq!(stepped.upper_band, 0.);
        assert_eq!(stepped.lower_band, 0.);
        let night = SkyMix::stepped(&layers[0], &textures, 10000.);
        assert_eq!(night.decks[0].opacity, 0.);
        assert_eq!(
            (night.upper_band, night.lower_band, night.night),
            (1., 1., 1.)
        );
    }
}
