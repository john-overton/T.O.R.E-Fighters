//! Stores on pylons between the picture and a recording (B3): the one
//! mapping the single-player recorder, the observer's live recording, the
//! capture conversion and the replay viewer share (docs/REPLAYS.md,
//! "Stores").

use std::collections::BTreeMap;
use tore_replay::stores::{self as recorded, StoreLook, StoreRecord};
use tore_world::snapshot::{self, AircraftPose, StorePose};

/// A drawn store as a recording keeps it.
pub fn store_record(pose: &StorePose) -> StoreRecord {
    StoreRecord {
        hardpoint: pose.hardpoint,
        mount: pose.mount,
        location: pose.rack.location,
        paired: pose.rack.paired,
        shape: pose.shape.clone(),
        look: match pose.look {
            snapshot::StoreLook::Store => StoreLook::Store,
            snapshot::StoreLook::Pod => StoreLook::Pod,
            snapshot::StoreLook::Tank => StoreLook::Tank,
        },
        items: pose.items,
        loaded: pose.loaded,
        body: pose.body,
    }
}

/// A recorded store as the picture draws it.
pub fn store_pose(record: &StoreRecord) -> StorePose {
    StorePose {
        hardpoint: record.hardpoint,
        mount: record.mount,
        rack: tore_sim::combat::hardpoints::Rack {
            location: record.location,
            paired: record.paired,
        },
        shape: record.shape.clone(),
        look: match record.look {
            StoreLook::Store => snapshot::StoreLook::Store,
            StoreLook::Pod => snapshot::StoreLook::Pod,
            StoreLook::Tank => snapshot::StoreLook::Tank,
        },
        items: record.items,
        loaded: record.loaded,
        body: record.body,
    }
}

/// What a recording learns of aircraft stores tick by tick: each
/// aircraft's stores as last recorded, so only changes become events.
#[derive(Clone, Debug, Default)]
pub struct StoresWatch {
    told: BTreeMap<u32, Vec<StoreRecord>>,
}

impl StoresWatch {
    /// The `stores.*` events for aircraft `id` drawn with `stores` this
    /// tick: its layout the first time it has any and whenever the layout
    /// changes, its items when only they changed.
    pub fn events(&mut self, id: u32, stores: &[StorePose]) -> Vec<tore_replay::Event> {
        let now: Vec<StoreRecord> = stores.iter().map(store_record).collect();
        let events = recorded::change_events(id, self.told.get(&id).map(Vec::as_slice), &now);
        if !events.is_empty() {
            self.told.insert(id, now);
        }
        events
    }

    /// The events for every aircraft of `poses` (ground objects, which carry
    /// no stores, are left out).
    pub fn poses<'a>(
        &mut self,
        poses: impl IntoIterator<Item = &'a AircraftPose>,
    ) -> Vec<tore_replay::Event> {
        poses
            .into_iter()
            .filter(|pose| pose.aircraft.is_some())
            .flat_map(|pose| self.events(pose.id, &pose.stores))
            .collect()
    }
}

/// Puts each recorded aircraft's stores at `tick` on the picture's poses.
pub fn put_recorded(
    track: &recorded::StoreTrack,
    tick: u64,
    poses: impl IntoIterator<Item = impl std::borrow::BorrowMut<AircraftPose>>,
) {
    for mut pose in poses {
        let pose = pose.borrow_mut();
        if let Some(stores) = track.at(pose.id, tick) {
            pose.stores = stores.iter().map(store_pose).collect();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A store goes into a recording and comes back as it was drawn.
    #[test]
    fn a_store_round_trips_through_a_recording() {
        let pose = StorePose {
            hardpoint: 4,
            mount: [8.25, -1.5, 3.125],
            rack: tore_sim::combat::hardpoints::Rack {
                location: 4,
                paired: true,
            },
            shape: Some("AGM65.3".into()),
            look: snapshot::StoreLook::Pod,
            items: 2,
            loaded: 3,
            body: [8.5, 1.4],
        };
        assert_eq!(store_pose(&store_record(&pose)), pose);
        let mut watch = StoresWatch::default();
        let first = watch.events(9, std::slice::from_ref(&pose));
        assert_eq!(first.len(), 2, "the layout and its one station");
        assert!(watch.events(9, std::slice::from_ref(&pose)).is_empty());
        let fired = StorePose {
            items: 1,
            ..pose.clone()
        };
        let then = watch.events(9, std::slice::from_ref(&fired));
        assert_eq!(then.len(), 1, "the items alone");
        let events: Vec<tore_replay::TimedEvent> = first
            .into_iter()
            .map(|event| tore_replay::TimedEvent { tick: 5, event })
            .chain(
                then.into_iter()
                    .map(|event| tore_replay::TimedEvent { tick: 9, event }),
            )
            .collect();
        let track = recorded::StoreTrack::from_events(&events);
        let mut drawn = [AircraftPose {
            id: 9,
            ..AircraftPose::default()
        }];
        put_recorded(&track, 8, drawn.iter_mut());
        assert_eq!(drawn[0].stores, [pose]);
        put_recorded(&track, 9, drawn.iter_mut());
        assert_eq!(drawn[0].stores, [fired]);
    }
}
