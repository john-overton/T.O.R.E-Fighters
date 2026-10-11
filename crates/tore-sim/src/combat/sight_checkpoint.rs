//! The coders of the targeting sight (docs/formats/checkpoint.md): its
//! sensor, mode, look, aim point, the seat's held controls, a pending key
//! and the last notice are stepped at the fixed tick, so a restore carries
//! them. The mode, the controls, the request and the notice share the
//! gunship's coders (`live_checkpoint.rs`).
use super::{DesignatorPod, DesignatorStore, Gimbal, Sight, SightSensor};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

impl Checkpoint for Gimbal {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match *self {
            Self::Hemisphere => s.writer().write_varint(0),
            Self::Cone { search, track } => {
                s.writer().write_varint(1);
                search.save(s, None)?;
                track.save(s, None)?;
            }
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Hemisphere,
            1 => Self::Cone {
                search: Checkpoint::load(l, None)?,
                track: Checkpoint::load(l, None)?,
            },
            other => return invalid(format!("a sight gimbal has no kind {other}")),
        })
    }
}

crate::checkpoint_struct!(SightSensor {
    laser,
    range_ft,
    gimbal,
    eye,
    default_look,
});

crate::checkpoint_struct!(Sight {
    sensor,
    mode,
    look,
    returning,
    aim,
    input,
    slew_held,
    request,
    notice,
});

crate::checkpoint_struct!(DesignatorStore {
    source,
    name,
    sensor,
    weight_lbs,
    flags,
});

crate::checkpoint_struct!(DesignatorPod { station, store });
