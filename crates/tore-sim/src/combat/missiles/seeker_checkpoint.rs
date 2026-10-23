//! The coders of a missile seeker's state: what it has measured, which target
//! it holds and how its lock stands (docs/formats/checkpoint.md). Every field
//! is coded; the seeker is stepped each tick from its own earlier values, so
//! nothing in it is scratch.

use super::{Heat, Observation, Seeker, Status};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

impl Checkpoint for Heat {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Unknown => s.writer().write_varint(0),
            Self::Engine {
                on,
                throttle,
                afterburner,
            } => {
                s.writer().write_varint(1);
                on.save(s, None)?;
                throttle.save(s, None)?;
                afterburner.save(s, None)?;
            }
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Unknown,
            1 => Self::Engine {
                on: Checkpoint::load(l, None)?,
                throttle: Checkpoint::load(l, None)?,
                afterburner: Checkpoint::load(l, None)?,
            },
            other => return invalid(format!("Heat has no variant {other}")),
        })
    }
}

crate::checkpoint_struct!(Observation {
    id,
    position,
    velocity,
    quality,
    off_axis,
    range,
});

crate::checkpoint_enum!(Status {
    Unguided = 0,
    Midcourse = 1,
    Search = 2,
    Acquiring = 3,
    Locked = 4,
    Pitbull = 5,
    Memory = 6,
    Lost = 7,
    Expired = 8,
});

crate::checkpoint_struct!(Seeker {
    target,
    candidate,
    dwell,
    missing,
    acquired,
    status,
    quality,
    observation,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    fn observation(id: u32) -> Observation {
        Observation {
            id,
            position: [1000.5, -20., 33_000.25],
            velocity: [-1.5, 0., 900.],
            quality: 0.731,
            off_axis: 0.0123,
            range: 5_432.1,
        }
    }

    #[test]
    fn every_heat_and_status_round_trips() {
        let models = Models::default();
        for heat in [
            Heat::Unknown,
            Heat::Engine {
                on: false,
                throttle: 0.,
                afterburner: false,
            },
            Heat::Engine {
                on: true,
                throttle: 0.83,
                afterburner: true,
            },
        ] {
            assert_eq!(round_trip(&heat, &models).unwrap(), heat);
        }
        for status in [
            Status::Unguided,
            Status::Midcourse,
            Status::Search,
            Status::Acquiring,
            Status::Locked,
            Status::Pitbull,
            Status::Memory,
            Status::Lost,
            Status::Expired,
        ] {
            assert_eq!(round_trip(&status, &models).unwrap(), status);
        }
    }

    #[test]
    fn a_seeker_keeps_its_lock_and_its_exact_floats() {
        let models = Models::default();
        let seekers = [
            Seeker::default(),
            Seeker::new(Some(7)),
            Seeker {
                target: Some(9),
                candidate: Some(11),
                dwell: 29,
                missing: 240,
                acquired: true,
                status: Status::Memory,
                quality: f64::from_bits(0x3fe5_5555_5555_5556),
                observation: Some(observation(9)),
            },
            Seeker {
                // A negative zero and a NaN payload survive, as every float
                // in a checkpoint does.
                quality: -0.,
                observation: Some(Observation {
                    range: f64::from_bits(0x7ff8_0000_0000_1234),
                    ..observation(1)
                }),
                ..Seeker::default()
            },
        ];
        for seeker in &seekers {
            let copy = round_trip(seeker, &models).unwrap();
            assert_eq!(
                to_bytes(&copy, &models).unwrap(),
                to_bytes(seeker, &models).unwrap()
            );
            assert_eq!(copy.quality.to_bits(), seeker.quality.to_bits());
        }
        assert_eq!(round_trip(&seekers[2], &models).unwrap(), seekers[2]);
    }

    #[test]
    fn a_damaged_seeker_is_refused() {
        let models = Models::default();
        let mut coded = to_bytes(
            &Seeker {
                observation: Some(observation(3)),
                ..Seeker::new(Some(3))
            },
            &models,
        )
        .unwrap();
        for cut in 0..coded.body.len() {
            let mut short = coded.clone();
            short.body.truncate(cut);
            assert!(from_bytes::<Seeker>(&short, &models).is_err(), "cut {cut}");
        }
        // A status number that names no status.
        coded.body = vec![0xff; coded.body.len()];
        let _ = from_bytes::<Seeker>(&coded, &models);
    }
}
