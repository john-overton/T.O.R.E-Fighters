//! The coders of the AI's motion requests and its command clock
//! (docs/formats/checkpoint.md). The controller's active maneuver and its
//! last intent batch hold these.

use super::{
    Bank, CommandClock, CompletionAxis, Deadline, Duration, MotionRequest, PitchRequest,
    SpeedRequest,
};
use crate::ai::ScalarSpeed;
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

impl Checkpoint for PitchRequest {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Explicit(degrees) => {
                s.writer().write_varint(0);
                degrees.save(s, None)?;
            }
            Self::Engagement => s.writer().write_varint(1),
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Explicit(Checkpoint::load(l, None)?),
            1 => Self::Engagement,
            other => return invalid(format!("PitchRequest has no variant {other}")),
        })
    }
}

impl Checkpoint for Bank {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Unconstrained => s.writer().write_varint(0),
            Self::Explicit(degrees) => {
                s.writer().write_varint(1);
                degrees.save(s, None)?;
            }
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Unconstrained,
            1 => Self::Explicit(Checkpoint::load(l, None)?),
            other => return invalid(format!("Bank has no variant {other}")),
        })
    }
}

impl Checkpoint for SpeedRequest {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Explicit(speed) => {
                s.writer().write_varint(0);
                speed.save(s, None)?;
            }
            Self::Corner => s.writer().write_varint(1),
            Self::Maximum => s.writer().write_varint(2),
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Explicit(ScalarSpeed::load(l, None)?),
            1 => Self::Corner,
            2 => Self::Maximum,
            other => return invalid(format!("SpeedRequest has no variant {other}")),
        })
    }
}

impl Checkpoint for Duration {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Timed(seconds) => {
                s.writer().write_varint(0);
                seconds.save(s, None)?;
            }
            Self::Geometric => s.writer().write_varint(1),
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Timed(Checkpoint::load(l, None)?),
            1 => Self::Geometric,
            other => return invalid(format!("Duration has no variant {other}")),
        })
    }
}

crate::checkpoint_struct!(MotionRequest {
    heading_deg,
    pitch,
    bank,
    speed,
    duration,
});

crate::checkpoint_struct!(CommandClock { tick });
crate::checkpoint_tuple!(Deadline(quarters));

crate::checkpoint_enum!(CompletionAxis {
    Heading = 0,
    Pitch = 1,
    Bank = 2,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checkpoint::{Models, round_trip};

    fn same<T: Checkpoint + PartialEq + std::fmt::Debug>(value: T) {
        let copy = round_trip(&value, &Models::default()).unwrap();
        assert_eq!(copy, value);
    }

    #[test]
    fn every_motion_variant_round_trips() {
        same(PitchRequest::Explicit(-45));
        same(PitchRequest::Engagement);
        same(Bank::Unconstrained);
        same(Bank::Explicit(-180));
        same(SpeedRequest::Explicit(ScalarSpeed(612.5)));
        same(SpeedRequest::Corner);
        same(SpeedRequest::Maximum);
        same(Duration::Timed(15));
        same(Duration::Geometric);
        same(CommandClock::at_tick(1_234_567));
        same(Deadline(u64::MAX));
        for axis in [
            CompletionAxis::Heading,
            CompletionAxis::Pitch,
            CompletionAxis::Bank,
        ] {
            same(axis);
        }
        same(MotionRequest::new(
            -10,
            PitchRequest::Explicit(30),
            Bank::Explicit(60),
            SpeedRequest::Explicit(ScalarSpeed(700.)),
            Duration::Timed(3),
        ));
    }

    #[test]
    fn an_unknown_variant_is_refused() {
        let mut s = Saver::new();
        s.writer().write_varint(9);
        let body = s.finish_section();
        let models = Models::default();
        let mut l = Loader::new(&body, &[], &models);
        assert!(PitchRequest::load(&mut l, None).is_err());
    }
}
