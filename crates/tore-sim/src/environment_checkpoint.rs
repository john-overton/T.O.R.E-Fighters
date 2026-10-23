//! The coders of the weather clock: its ticks, remainder, fog selection and random stream. The configuration is mission setup.
//!
//! Stage H slice H2 (world shell). The weather section restores `Environment`
//! in place over a fresh one built from the same mission, so no constructor is
//! involved: the fresh one drew its first fog numbers when it was built, and
//! restoring overwrites the stream, the schedule and every tint those draws
//! changed (docs/formats/checkpoint.md, "Restoring").

use crate::checkpoint::{Checkpoint, CheckpointError, InPlace, Loader, Saver, invalid};
use crate::environment::Environment;
use tore_formats::weather::{Callback, Deck, Layer};

crate::checkpoint_enum!(Callback {
    None = 0,
    HorizonNoop = 1,
    Fog = 2,
});

crate::checkpoint_struct!(Deck {
    name,
    altitude_feet,
    tile_exponent,
});

// The active list holds blended copies of the records, so every field is
// coded: the blend moves most of them off the records' own values.
crate::checkpoint_struct!(Layer {
    flags,
    start_seconds,
    end_seconds,
    low_feet,
    high_feet,
    sky,
    fog_near,
    fog_near_density,
    fog_far,
    fog_far_density,
    see_distance,
    haze_low,
    haze_low_blend,
    haze_high,
    haze_high_blend,
    shade,
    terrain,
    tint,
    tint_scalar,
    callback,
    tint_reduction_max,
    tint_reduction_speed,
    decks,
    moon_azimuth,
    moon_elevation,
    sunrise_seconds,
    sunset_seconds,
    sun_azimuth_morning,
    sun_azimuth_evening,
    effects,
    shape,
});

impl InPlace for Environment {
    fn save_in_place(&self, s: &mut Saver) -> Result<(), CheckpointError> {
        let Environment {
            // Mission setup: the fresh environment was built from the same
            // configuration, and nothing a tick does changes it.
            configuration: _,
            clock,
            ticks,
            active,
            active_seconds,
            records,
            rng,
            next_selection,
        } = self;
        clock.save(s, None)?;
        ticks.save(s, None)?;
        active.save(s, None)?;
        active_seconds.save(s, None)?;
        // The records start as the configuration's layers and the fog callback
        // rewrites one field of them, the tint scalar, at each selection
        // (`select`, the only writer). So only that field is coded, one per
        // record, and a restore keeps the fresh environment's other fields.
        s.count(records.len());
        for record in records {
            record.tint_scalar.save(s, None)?;
        }
        rng.save(s, None)?;
        next_selection.save(s, None)?;
        Ok(())
    }

    fn restore_in_place(&mut self, l: &mut Loader<'_>) -> Result<(), CheckpointError> {
        let clock = Checkpoint::load(l, None)?;
        let ticks = Checkpoint::load(l, None)?;
        let active = Checkpoint::load(l, None)?;
        let active_seconds = Checkpoint::load(l, None)?;
        let count = l.count()?;
        if count != self.records.len() {
            return invalid(format!(
                "the weather has {count} records, this mission's has {}",
                self.records.len()
            ));
        }
        let mut scalars = Vec::with_capacity(count);
        for _ in 0..count {
            scalars.push(i32::load(l, None)?);
        }
        let rng = Checkpoint::load(l, None)?;
        let next_selection = Checkpoint::load(l, None)?;
        // Everything decoded: now change the environment.
        self.clock = clock;
        self.ticks = ticks;
        self.active = active;
        self.active_seconds = active_seconds;
        for (record, scalar) in self.records.iter_mut().zip(scalars) {
            record.tint_scalar = scalar;
        }
        self.rng = rng;
        self.next_selection = next_selection;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Coded, Models, round_trip_in_place};
    use crate::environment::Configuration;
    use tore_formats::weather::Module;

    /// Two fog layers on separate floors, both always in range, so every
    /// selection draws from the fog stream and moves both tints.
    fn fog() -> Configuration {
        let module = Module::parse(&tore_formats::weather::synthetic_module(2)).unwrap();
        let mut config = Configuration::new(module, 0, 0, 0, None)
            .unwrap()
            .with_weather_seed(7)
            .unwrap();
        for layer in &mut config.module.layers {
            layer.start_seconds = 0;
            layer.end_seconds = 86_000;
            layer.callback = Callback::Fog;
            layer.tint_scalar = 225;
        }
        config.module.layers[1].low_feet = 500;
        config
    }

    fn saved(environment: &Environment, models: &Models) -> Coded {
        let mut s = Saver::with_models(models.clone());
        environment.save_in_place(&mut s).unwrap();
        let body = s.finish_section();
        Coded {
            body,
            records: s.into_records(),
        }
    }

    /// A weather clock part way through a run of fog selections restores into
    /// a fresh environment, which differs from it before and equals it after,
    /// and both then draw the same fog for 40 seconds.
    #[test]
    fn a_stepped_environment_restores_into_a_fresh_one_and_steps_on_identically() {
        let models = Models::default();
        let mut original = Environment::new(fog());
        // A whole number of seconds plus a part: the clock's remainder and
        // the selection schedule are both mid-way.
        for _ in 0..(120 * 35 + 77) {
            original.step();
        }
        let mut restored = Environment::new(fog());
        assert_ne!(original, restored);
        round_trip_in_place(&original, &mut restored, &models).unwrap();
        assert_eq!(original, restored);
        assert_ne!(
            original.active()[0].tint_scalar,
            fog().layers()[0].tint_scalar,
            "the fog moved a tint, so the test is of state that matters"
        );
        for tick in 0..120 * 40 {
            original.step();
            restored.step();
            assert_eq!(original, restored, "tick {tick}");
        }
        assert!(original.ticks() > 0);
    }

    #[test]
    fn an_environment_of_another_mission_is_refused() {
        let models = Models::default();
        let mut original = Environment::new(fog());
        original.step();
        let coded = saved(&original, &models);
        let mut other = Environment::new(
            Configuration::new(
                Module::parse(&tore_formats::weather::synthetic_module(3)).unwrap(),
                0,
                0,
                0,
                None,
            )
            .unwrap(),
        );
        let before = other.clone();
        let mut l = Loader::new(&coded.body, &coded.records, &models);
        assert!(other.restore_in_place(&mut l).is_err());
        assert_eq!(other, before, "a refused restore changes nothing");
    }

    #[test]
    fn damaged_environment_bytes_are_refused_without_a_panic() {
        let models = Models::default();
        let mut original = Environment::new(fog());
        for _ in 0..500 {
            original.step();
        }
        let coded = saved(&original, &models);
        let mut target = Environment::new(fog());
        for cut in 0..coded.body.len() {
            let mut shorter = coded.clone();
            shorter.body.truncate(cut);
            let mut l = Loader::new(&shorter.body, &shorter.records, &models);
            assert!(
                target.restore_in_place(&mut l).is_err() || l.finish().is_err(),
                "cut at {cut}"
            );
        }
        for bit in 0..coded.body.len() * 8 {
            let mut flipped = coded.clone();
            flipped.body[bit / 8] ^= 1 << (bit % 8);
            let mut l = Loader::new(&flipped.body, &flipped.records, &models);
            let _ = target.restore_in_place(&mut l);
        }
    }
}
