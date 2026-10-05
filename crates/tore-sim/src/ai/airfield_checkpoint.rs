//! The coders of an AI aircraft's takeoff and landing sequence and its
//! ground start (docs/formats/checkpoint.md, stage H slice H4).
//!
//! `RunwayView`, `AirfieldAnchors`, `Phase`, `LandingReason` and
//! `LandingOrder` are leaf types coded in `checkpoint_shared.rs`. The runway
//! view a sequence or a ground start holds is a copied record, so it is
//! coded as a shared record: an aircraft's departure and every other
//! aircraft's sequence on the same runway cost one coding. `Situation`,
//! `Control`, `AirGuidance`, `Command` and `Step` are per-tick inputs and
//! outputs of `Sequence::step`, never held between ticks.

use super::{GroundStart, Kind, Sequence};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

crate::checkpoint_struct!(GroundStart { end, order } shared { runway });

impl Checkpoint for Kind {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Departure => s.writer().write_varint(0),
            Self::Landing(reason) => {
                s.writer().write_varint(1);
                reason.save(s, None)?;
            }
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Departure,
            1 => Self::Landing(Checkpoint::load(l, None)?),
            other => return invalid(format!("airfield Kind has no variant {other}")),
        })
    }
}

crate::checkpoint_struct!(Sequence {
    kind,
    phase,
    end,
    order,
    leg,
    queue_leg,
    leg_origin,
    stopping,
    spot,
    timer,
    next_check,
    liftoff_tick,
    slot,
    route,
    marshal_target,
    park,
    flaps_up,
    go_arounds,
    climb_away,
    abort,
} shared { runway });
