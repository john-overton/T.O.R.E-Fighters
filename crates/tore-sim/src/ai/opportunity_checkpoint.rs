//! The coders of a wing's mission of opportunity after a lost human leader
//! (docs/formats/checkpoint.md, stage H slice H4). `Sighting` and `Plan` are
//! per-call values and are not state.

use super::{HomeReason, LastKnown, Opportunity, SearchEnd};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

crate::checkpoint_enum!(SearchEnd {
    NothingKnown = 0,
    AllSearched = 1,
    SearchTimeUp = 2,
});

impl Checkpoint for HomeReason {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::NotCleared => s.writer().write_varint(0),
            Self::SearchOver(end) => {
                s.writer().write_varint(1);
                end.save(s, None)?;
            }
            Self::RouteFlown => s.writer().write_varint(2),
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::NotCleared,
            1 => Self::SearchOver(Checkpoint::load(l, None)?),
            2 => Self::RouteFlown,
            other => return invalid(format!("HomeReason has no variant {other}")),
        })
    }
}

crate::checkpoint_struct!(LastKnown {
    id,
    position,
    observed_tick,
    searched,
});

crate::checkpoint_struct!(Opportunity {
    side,
    wing,
    started_tick,
    last_contact_tick,
    points,
    searching,
    arrived_tick,
    search_over,
    route,
    route_sector,
    route_started,
    home,
});
