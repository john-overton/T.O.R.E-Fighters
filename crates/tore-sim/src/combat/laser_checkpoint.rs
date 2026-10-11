//! The coders of the laser (docs/formats/checkpoint.md): its state, the
//! hand override, the automatic rule's last answer, the point and the limits
//! are stepped at the fixed tick, so a restore carries them.
use super::{Laser, LaserSpot, LaserState, Mask};

crate::checkpoint_enum!(LaserState {
    Off = 0,
    Ready = 1,
    Lasing = 2,
    Masked = 3,
});

crate::checkpoint_enum!(Mask {
    Range = 0,
    Gimbal = 1,
    Terrain = 2,
});

crate::checkpoint_struct!(LaserSpot {
    owner,
    side,
    position,
    target,
});

crate::checkpoint_struct!(Laser {
    state,
    manual,
    automatic,
    point,
    target,
    range_ft,
    limit_ft,
    mask,
});
