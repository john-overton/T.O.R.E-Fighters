//! The coders of released flares and chaff (docs/formats/checkpoint.md, stage
//! H slice H3b).
//!
//! Presentation, but the next tick reads it back: a flare's flight, its
//! smoke trail and the release count that numbers the next device's look all
//! carry on. The one stored `f32` is a flare puff's opacity, coded by its
//! bits. A flare is coded against the flare before it (a pair is thrown to
//! each side from one outlet) and a flare's puffs each against the puff
//! before, since neighbours share most high bits.

use super::{Chaff, Devices, Flare, FlarePuff, MAX_CHAFF, MAX_FLARES, MAX_PUFFS_PER_FLARE};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};
use std::collections::VecDeque;

crate::checkpoint_struct!(FlarePuff {
    position,
    velocity,
    age,
    path,
    opacity,
});

crate::checkpoint_struct!(Chaff {
    position,
    velocity,
    age,
    seed,
});

impl Checkpoint for Flare {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        let Flare {
            position,
            velocity,
            side,
            motion,
            age,
            seed,
            path,
            last_puff,
            resting,
            puffs,
        } = self;
        position.save(s, base.map(|b| &b.position))?;
        velocity.save(s, base.map(|b| &b.velocity))?;
        side.save(s, base.map(|b| &b.side))?;
        motion.save(s, base.map(|b| &b.motion))?;
        age.save(s, base.map(|b| &b.age))?;
        seed.save(s, base.map(|b| &b.seed))?;
        path.save(s, base.map(|b| &b.path))?;
        last_puff.save(s, base.map(|b| &b.last_puff))?;
        resting.save(s, base.map(|b| &b.resting))?;
        s.count(puffs.len());
        let mut previous: Option<&FlarePuff> = None;
        for puff in puffs {
            puff.save(s, previous)?;
            previous = Some(puff);
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let position = Checkpoint::load(l, base.map(|b| &b.position))?;
        let velocity = Checkpoint::load(l, base.map(|b| &b.velocity))?;
        let side = Checkpoint::load(l, base.map(|b| &b.side))?;
        let motion = Checkpoint::load(l, base.map(|b| &b.motion))?;
        let age = Checkpoint::load(l, base.map(|b| &b.age))?;
        let seed = Checkpoint::load(l, base.map(|b| &b.seed))?;
        let path = Checkpoint::load(l, base.map(|b| &b.path))?;
        let last_puff = Checkpoint::load(l, base.map(|b| &b.last_puff))?;
        let resting = Checkpoint::load(l, base.map(|b| &b.resting))?;
        let count = l.count()?;
        if count > MAX_PUFFS_PER_FLARE {
            return invalid(format!(
                "{count} puffs behind one flare, more than the {MAX_PUFFS_PER_FLARE} it keeps"
            ));
        }
        let mut puffs = VecDeque::with_capacity(count);
        for _ in 0..count {
            let puff = FlarePuff::load(l, puffs.back())?;
            puffs.push_back(puff);
        }
        Ok(Flare {
            position,
            velocity,
            side,
            motion,
            age,
            seed,
            path,
            last_puff,
            resting,
            puffs,
        })
    }
}

impl Checkpoint for Devices {
    fn save(&self, s: &mut Saver, base: Option<&Self>) -> Result<(), CheckpointError> {
        let Devices {
            flares,
            chaff,
            wind,
            releases,
        } = self;
        s.count(flares.len());
        let mut previous: Option<&Flare> = None;
        for flare in flares {
            flare.save(s, previous)?;
            previous = Some(flare);
        }
        s.count(chaff.len());
        let mut previous: Option<&Chaff> = None;
        for cloud in chaff {
            cloud.save(s, previous)?;
            previous = Some(cloud);
        }
        wind.save(s, base.map(|b| &b.wind))?;
        releases.save(s, base.map(|b| &b.releases))
    }
    fn load(l: &mut Loader<'_>, base: Option<&Self>) -> Result<Self, CheckpointError> {
        let count = l.count()?;
        if count > MAX_FLARES {
            return invalid(format!(
                "{count} flares, more than the {MAX_FLARES} a mission keeps"
            ));
        }
        let mut flares = VecDeque::with_capacity(count);
        for _ in 0..count {
            let flare = Flare::load(l, flares.back())?;
            flares.push_back(flare);
        }
        let count = l.count()?;
        if count > MAX_CHAFF {
            return invalid(format!(
                "{count} chaff clouds, more than the {MAX_CHAFF} a mission keeps"
            ));
        }
        let mut chaff = VecDeque::with_capacity(count);
        for _ in 0..count {
            let cloud = Chaff::load(l, chaff.back())?;
            chaff.push_back(cloud);
        }
        Ok(Devices {
            flares,
            chaff,
            wind: Checkpoint::load(l, base.map(|b| &b.wind))?,
            releases: Checkpoint::load(l, base.map(|b| &b.releases))?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::Release;
    use super::*;
    use crate::attitude::Basis;
    use crate::checkpoint::{Coded, Models, from_bytes, round_trip, to_bytes};

    /// A hill, so some devices land while others are still falling.
    fn ground(x: f64, z: f64) -> f64 {
        (x * 0.02 + z * 0.01).max(0.)
    }

    fn release(n: u32) -> Release {
        let t = f64::from(n);
        Release {
            position: [t * 40., 12_000. - t * 3., 900. + t * 25.],
            velocity: [10. + t, -4., 480. - t],
            basis: Basis::new(0.1 * t, 0.02 * t, 0.3 - 0.01 * t),
        }
    }

    /// One tick of the scripted releases: flares at 100, 400 and 410, chaff
    /// at 150, 160 and 700, over twenty seconds of mission time.
    fn tick(devices: &mut Devices, n: u32) {
        match n {
            100 | 400 | 410 => devices.release_flare(release(n % 97)),
            150 | 160 | 700 => devices.release_chaff(release(n % 89)),
            _ => {}
        }
        devices.step(&ground);
    }

    fn scripted(wind: [f64; 3]) -> Devices {
        Devices {
            wind,
            ..Devices::default()
        }
    }

    #[test]
    fn a_restored_copy_evolves_identically_for_600_ticks() {
        let models = Models::default();
        let mut original = scripted([14., 0., -6.]);
        // With flares burning, puffs behind them and chaff drifting down.
        for n in 0..500 {
            tick(&mut original, n);
        }
        assert!(original.flares.len() >= 4, "{}", original.flares.len());
        assert!(original.puffs().count() > 20);
        assert!(original.chaff.len() >= 2);
        assert!(original.flares.iter().any(|f| f.puffs.len() > 1));
        let mut copy = round_trip(&original, &models).unwrap();
        assert_eq!(copy, original);
        assert_eq!(copy.released(), original.released());
        for n in 500..1100 {
            tick(&mut original, n);
            tick(&mut copy, n);
            assert_eq!(copy, original, "tick {n}");
            if n % 30 == 0 {
                assert_eq!(
                    to_bytes(&copy, &models).unwrap(),
                    to_bytes(&original, &models).unwrap(),
                    "coding at tick {n}"
                );
            }
        }
        // The release after the restore numbers its device alike: the same
        // look, down to the seed.
        assert_eq!(original.released(), 6);
    }

    #[test]
    fn a_flare_that_has_landed_and_one_that_burned_out_round_trip() {
        let models = Models::default();
        let mut devices = scripted([0.; 3]);
        devices.release_flare(Release {
            position: [0., 600., 0.],
            velocity: [0., 0., 200.],
            basis: Basis::new(0., 0., 0.),
        });
        let mut resting_seen = false;
        // 40 seconds: down to the ground and past the 30 second burn.
        for n in 0..4800 {
            devices.step(&ground);
            if n == 1500 {
                resting_seen = devices.flares.iter().any(|f| f.position[1] < 200.);
            }
            if n % 400 == 0 {
                let copy = round_trip(&devices, &models).unwrap();
                assert_eq!(copy, devices, "tick {n}");
            }
        }
        assert!(resting_seen, "a flare reached the ground while it burned");
    }

    #[test]
    fn the_opacity_survives_to_the_bit() {
        let models = Models::default();
        let mut devices = scripted([5., 0., 0.]);
        devices.release_flare(release(3));
        for _ in 0..90 {
            devices.step(&ground);
        }
        let copy = round_trip(&devices, &models).unwrap();
        let bits = |d: &Devices| -> Vec<u32> { d.puffs().map(|p| p.opacity().to_bits()).collect() };
        assert!(!bits(&devices).is_empty());
        assert!(bits(&devices).iter().any(|&b| b != 0));
        assert_eq!(bits(&copy), bits(&devices));
    }

    #[test]
    fn more_devices_than_a_mission_keeps_are_refused() {
        let models = Models::default();
        for (flares, chaff) in [(MAX_FLARES + 1, 0), (0, MAX_CHAFF + 1)] {
            let mut s = Saver::with_models(models.clone());
            s.count(flares);
            if chaff > 0 {
                s.count(chaff);
            }
            for _ in 0..4096 {
                s.writer().write_bool(false);
            }
            let coded = Coded {
                body: s.finish_section(),
                records: Vec::new(),
            };
            assert!(
                from_bytes::<Devices>(&coded, &models).is_err(),
                "{flares} flares, {chaff} chaff"
            );
        }
    }

    #[test]
    fn damaged_bytes_never_panic() {
        let models = Models::default();
        let mut devices = scripted([3., 0., 1.]);
        for n in 0..450 {
            tick(&mut devices, n);
        }
        let coded = to_bytes(&devices, &models).unwrap();
        for cut in 0..coded.body.len() {
            let _ = from_bytes::<Devices>(
                &Coded {
                    body: coded.body[..cut].to_vec(),
                    records: Vec::new(),
                },
                &models,
            );
        }
        let mut seed = 0x9e37_79b9_7f4a_7c15_u64;
        for _ in 0..2000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let mut body = coded.body.clone();
            let at = (seed as usize >> 8) % body.len();
            body[at] ^= 1 << (seed & 7);
            let _ = from_bytes::<Devices>(
                &Coded {
                    body,
                    records: Vec::new(),
                },
                &models,
            );
        }
    }
}
