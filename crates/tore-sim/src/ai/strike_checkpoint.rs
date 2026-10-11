//! The coders of a striker's strike state (docs/formats/checkpoint.md).
//!
//! Every field the next step reads is coded: the phase and its start, the
//! pass, the target and store, the pass count that seeds the next aim, the
//! drawn aim, the final's line, the release count and the exit's heading.
//! `inputs` is per-step scratch: the host hands it over before every mission
//! step and the step takes it, so between steps it is always empty and a
//! restored strike starts without it. `Context`, `Order`, `Steer`,
//! `Selection`, `Inputs`, `Store`, `Solution`, `PassInputs`, `PassChoice`,
//! `TargetFacts` and `AimDraw` are per-call values, never held.

use super::{Pass, Phase, Segment, StoreKind, Strike};
use crate::ai::awareness::ground::ThreatLevel;

crate::checkpoint_enum!(Pass {
    Straight = 0,
    PopUp = 1,
    FastHigh = 2,
    Dive = 3,
    Level = 4,
    Orbit = 5,
    StandOff = 6,
});

crate::checkpoint_enum!(StoreKind {
    Bomb = 0,
    Rocket = 1,
    Laser = 2,
    Coordinate = 3,
    Command = 4,
    Seeker = 5,
    RadarAgm = 6,
    AntiRadiation = 7,
    Gun = 8,
});

crate::checkpoint_enum!(Phase {
    Ingress = 0,
    Regain = 1,
    Attack = 2,
    Exit = 3,
    TurnBack = 4,
    Wait = 5,
    Defend = 6,
    Home = 7,
});

crate::checkpoint_enum!(Segment {
    Approach = 0,
    Pull = 1,
    Final = 2,
    Climb = 3,
    Guide = 4,
});

crate::checkpoint_enum!(ThreatLevel {
    Clear = 0,
    Air = 1,
    Guns = 2,
    Missiles = 3,
});

crate::checkpoint_struct!(Strike {
    seed,
    phase,
    segment,
    since,
    pass,
    forced,
    forced_station,
    target,
    station,
    kind,
    pass_number,
    aim_ft,
    lead_ft,
    gate_ft,
    inside,
    holding,
    hold_ft,
    released,
    next_release,
    exit_heading_deg,
    jink_side,
    cruise_ft,
    warned,
    threat,
    withdrawn,
} skip {
    inputs = None,
});
