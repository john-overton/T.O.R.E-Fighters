//! The coders of the AI's weapon service (B42, docs/formats/checkpoint.md): the
//! per-actor state machine that paces target preparation, lock checks and
//! firing, with its identities and timing profile. The store accounting types
//! (`StationId`, `Rounds`, `StoreState`, `StoreCapability`) are leaf types
//! coded in `checkpoint_shared.rs`. The other types in the module (inputs,
//! outcomes, envelopes, candidates, release requests) live for one call.

use super::{
    ActorId, FireRequest, Phase, RequestId, State, TargetId, TimingProfile, WeaponService,
};
use crate::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

crate::checkpoint_tuple!(ActorId(id));
crate::checkpoint_tuple!(TargetId(id));
crate::checkpoint_tuple!(RequestId(id));

crate::checkpoint_enum!(Phase {
    Search = 0,
    Prepare = 1,
    LockCheck = 2,
    Tracking = 3,
    Fire = 4,
    Reload = 5,
    WindowExpired = 6,
});

crate::checkpoint_struct!(FireRequest {
    actor,
    station,
    target,
    request_id,
});

crate::checkpoint_struct!(TimingProfile {
    search_delay_s,
    prepare_delay_s,
    unready_prepare_delay_s,
});

impl Checkpoint for State {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Self::Search { retry_at } => {
                s.writer().write_varint(0);
                retry_at.save(s, None)?;
            }
            Self::Prepare {
                ready_at,
                window_ends,
            } => {
                s.writer().write_varint(1);
                ready_at.save(s, None)?;
                window_ends.save(s, None)?;
            }
            Self::LockCheck {
                retry_at,
                window_ends,
            } => {
                s.writer().write_varint(2);
                retry_at.save(s, None)?;
                window_ends.save(s, None)?;
            }
            Self::Tracking { fire_at } => {
                s.writer().write_varint(3);
                fire_at.save(s, None)?;
            }
            Self::Fire => s.writer().write_varint(4),
            Self::Reload => s.writer().write_varint(5),
            Self::WindowExpired { retry_at } => {
                s.writer().write_varint(6);
                retry_at.save(s, None)?;
            }
        }
        Ok(())
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Self::Search {
                retry_at: Checkpoint::load(l, None)?,
            },
            1 => Self::Prepare {
                ready_at: Checkpoint::load(l, None)?,
                window_ends: Checkpoint::load(l, None)?,
            },
            2 => Self::LockCheck {
                retry_at: Checkpoint::load(l, None)?,
                window_ends: Checkpoint::load(l, None)?,
            },
            3 => Self::Tracking {
                fire_at: Checkpoint::load(l, None)?,
            },
            4 => Self::Fire,
            5 => Self::Reload,
            6 => Self::WindowExpired {
                retry_at: Checkpoint::load(l, None)?,
            },
            other => return invalid(format!("weapon service state has no variant {other}")),
        })
    }
}

// The timing profile is derived from the aircraft's identity when a service
// is built, but a restore builds none, so it is coded.
crate::checkpoint_struct!(WeaponService {
    actor,
    timing,
    state,
    last_tick,
    next_request,
});

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::weapon_service::StationId;
    use crate::checkpoint::{Models, round_trip};

    fn same<T: Checkpoint + PartialEq + std::fmt::Debug>(value: T) {
        let copy = round_trip(&value, &Models::default()).unwrap();
        assert_eq!(copy, value);
    }

    #[test]
    fn every_service_state_round_trips() {
        for state in [
            State::Search { retry_at: None },
            State::Search { retry_at: Some(9) },
            State::Prepare {
                ready_at: 10,
                window_ends: 70,
            },
            State::LockCheck {
                retry_at: None,
                window_ends: 70,
            },
            State::LockCheck {
                retry_at: Some(14),
                window_ends: 70,
            },
            State::Tracking { fire_at: 99 },
            State::Fire,
            State::Reload,
            State::WindowExpired { retry_at: u64::MAX },
        ] {
            same(state);
        }
        for phase in [
            Phase::Search,
            Phase::Prepare,
            Phase::LockCheck,
            Phase::Tracking,
            Phase::Fire,
            Phase::Reload,
            Phase::WindowExpired,
        ] {
            same(phase);
        }
        same(FireRequest {
            actor: ActorId(3),
            station: StationId(2),
            target: TargetId(11),
            request_id: RequestId(u64::MAX),
        });
    }

    #[test]
    fn a_service_round_trips_through_its_phases() {
        let mut service = WeaponService::new(
            ActorId(4),
            TimingProfile::for_aircraft(tore_formats::aircraft::AircraftId::Su27),
        );
        same(service.clone());
        let mut random = crate::ai::DecisionRandom::seeded(8);
        let inputs = super::super::ServiceInputs {
            target: Some(TargetId(11)),
            station: Some(StationId(0)),
            unready: false,
            lock: super::super::LockStatus::Locked {
                tracking_delay: super::super::Delay::seconds(1),
            },
            path_blocked: false,
            pacing: super::super::ProjectilePacing {
                burst_count: 1,
                burst_interval: super::super::Delay::seconds(0),
                reload: super::super::Delay::seconds(2),
                startup: super::super::Delay::seconds(0),
            },
        };
        for tick in (0..1_800).step_by(30) {
            let _ = service.advance(tick, &inputs, &mut random);
            same(service.clone());
        }
    }
}
