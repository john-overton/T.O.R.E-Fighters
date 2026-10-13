//! Stores on pylons in a recording (B3, docs/REPLAYS.md "Stores"): each
//! aircraft's drawn stations as `stores.layout` and `stores.station` events,
//! the first time the aircraft is recorded and whenever its layout changes,
//! and the whole stores still on each as a `stores.items` event whenever one
//! empties (a release, a jettison, a rearm). The viewer rebuilds an
//! aircraft's stores at any tick from these with [`StoreTrack`], so a seek
//! shows what forward play shows. They are ordinary events inside the
//! format: an older reader skips them, and a recording from before them has
//! none, so its aircraft carry no stores.

use crate::model::{Event, Value};
use crate::reader::TimedEvent;
use crate::vocab::kind;
use std::collections::BTreeMap;

/// How a store is drawn: with its own shape (a missile or a bomb, or a plain
/// body without one), a fitted rocket pod or a fitted tank.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StoreLook {
    #[default]
    Store,
    Pod,
    Tank,
}

impl StoreLook {
    fn name(self) -> &'static str {
        match self {
            Self::Store => "store",
            Self::Pod => "pod",
            Self::Tank => "tank",
        }
    }
    fn of(name: &str) -> Self {
        match name {
            "pod" => Self::Pod,
            "tank" => Self::Tank,
            _ => Self::Store,
        }
    }
}

/// One drawn station as a recording keeps it: everything the drawing needs,
/// so the viewer needs no loadout.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreRecord {
    /// The source hardpoint record (PT ordinal) it hangs from.
    pub hardpoint: u8,
    /// The record's position, feet right, up and forward of the aircraft's
    /// reference point (the right-hand pylon of a paired record).
    pub mount: [f64; 3],
    /// The source location byte.
    pub location: u8,
    /// Stores alternate between the position and its mirror.
    pub paired: bool,
    /// The store's own shape, when its record names one.
    pub shape: Option<String>,
    pub look: StoreLook,
    /// Whole stores still on the station.
    pub items: u16,
    /// Whole stores it was loaded with.
    pub loaded: u16,
    /// A fitted body's length and diameter, feet; zero for a shaped store.
    pub body: [f64; 2],
    /// The model's underside over its pylons (slice PY2): the heights, feet
    /// up, at the record, the inboard and outboard pylons and 7 ft ahead and
    /// behind; `None` where there is none or the recording predates them,
    /// so its stores hang at the record's height.
    pub over: [Option<f64>; 5],
    /// A wingtip station's tip edge, [feet right, feet up of its middle]
    /// (slice PY2); `None` elsewhere or in an older recording.
    pub tip: Option<[f64; 2]>,
}

/// The `stores.station` fields of [`StoreRecord::over`].
const OVER: [&str; 5] = [
    "over_ft",
    "over_inboard_ft",
    "over_outboard_ft",
    "over_front_ft",
    "over_rear_ft",
];

/// The events that record `aircraft`'s layout `stores`: a `stores.layout`
/// with the count, then one `stores.station` for each, in order.
pub fn layout_events(aircraft: u32, stores: &[StoreRecord]) -> Vec<Event> {
    let mut out = vec![
        Event::new(kind::STORES_LAYOUT)
            .with_subject(aircraft)
            .with("count", stores.len() as i64),
    ];
    for (slot, store) in stores.iter().enumerate() {
        let mut event = Event::new(kind::STORES_STATION)
            .with_subject(aircraft)
            .with("slot", slot as i64)
            .with("hardpoint", i64::from(store.hardpoint))
            .with("x_ft", store.mount[0])
            .with("y_ft", store.mount[1])
            .with("z_ft", store.mount[2])
            .with("location", i64::from(store.location))
            .with("paired", store.paired)
            .with("look", store.look.name())
            .with("items", i64::from(store.items))
            .with("loaded", i64::from(store.loaded));
        if let Some(shape) = &store.shape {
            event = event.with("shape", shape.as_str());
        }
        if store.body != [0.; 2] {
            event = event
                .with("length_ft", store.body[0])
                .with("diameter_ft", store.body[1]);
        }
        for (name, over) in OVER.iter().zip(store.over) {
            if let Some(height) = over {
                event = event.with(name, height);
            }
        }
        if let Some([right, up]) = store.tip {
            event = event.with("tip_right_ft", right).with("tip_up_ft", up);
        }
        out.push(event);
    }
    out
}

/// The event that records `aircraft`'s whole stores on each station of its
/// layout, in order.
pub fn items_event(aircraft: u32, items: &[u16]) -> Event {
    Event::new(kind::STORES_ITEMS).with_subject(aircraft).with(
        "items",
        Value::Ids(items.iter().map(|i| u32::from(*i)).collect()),
    )
}

/// What a recorder says about an aircraft's stores this tick, after `told`
/// (its last recorded stores, `None` before any): the layout when it is new
/// or changed, the items when only they changed, nothing when nothing did.
pub fn change_events(
    aircraft: u32,
    told: Option<&[StoreRecord]>,
    now: &[StoreRecord],
) -> Vec<Event> {
    let same_layout = told.is_some_and(|told| {
        told.len() == now.len()
            && told.iter().zip(now).all(|(a, b)| {
                StoreRecord {
                    items: 0,
                    ..a.clone()
                } == StoreRecord {
                    items: 0,
                    ..b.clone()
                }
            })
    });
    if !same_layout {
        if told.is_none() && now.is_empty() {
            return Vec::new();
        }
        return layout_events(aircraft, now);
    }
    if told.is_some_and(|told| told.iter().zip(now).all(|(a, b)| a.items == b.items)) {
        return Vec::new();
    }
    vec![items_event(
        aircraft,
        &now.iter().map(|s| s.items).collect::<Vec<_>>(),
    )]
}

/// Every aircraft's recorded stores over time, rebuilt from a recording's
/// events.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StoreTrack {
    /// By aircraft, each state from its tick, in tick order.
    states: BTreeMap<u32, Vec<(u64, Vec<StoreRecord>)>>,
}

impl StoreTrack {
    /// The track of `events` (a recording's, in tick order).
    pub fn from_events(events: &[TimedEvent]) -> Self {
        let mut track = Self::default();
        // A layout being read: its tick, its count and the stations so far.
        let mut open: BTreeMap<u32, (u64, usize, Vec<StoreRecord>)> = BTreeMap::new();
        for timed in events {
            let event = &timed.event;
            let Some(aircraft) = event.subject else {
                continue;
            };
            match event.kind.as_str() {
                kind::STORES_LAYOUT => {
                    let count = event.num("count").unwrap_or(0.).max(0.) as usize;
                    if count == 0 {
                        track.push(aircraft, timed.tick, Vec::new());
                        open.remove(&aircraft);
                    } else {
                        open.insert(aircraft, (timed.tick, count.min(64), Vec::new()));
                    }
                }
                kind::STORES_STATION => {
                    let Some((tick, count, stations)) = open.get_mut(&aircraft) else {
                        continue;
                    };
                    stations.push(record(event));
                    if stations.len() >= *count {
                        let (tick, stations) = (*tick, std::mem::take(stations));
                        open.remove(&aircraft);
                        track.push(aircraft, tick, stations);
                    }
                }
                kind::STORES_ITEMS => {
                    let Some(Value::Ids(items)) = event.get("items") else {
                        continue;
                    };
                    let Some(mut stations) = track.latest(aircraft).map(<[_]>::to_vec) else {
                        continue;
                    };
                    for (store, items) in stations.iter_mut().zip(items) {
                        store.items = u16::try_from(*items).unwrap_or(u16::MAX);
                    }
                    track.push(aircraft, timed.tick, stations);
                }
                _ => {}
            }
        }
        track
    }

    fn push(&mut self, aircraft: u32, tick: u64, stations: Vec<StoreRecord>) {
        let states = self.states.entry(aircraft).or_default();
        if states.last().is_some_and(|(t, _)| *t == tick) {
            states.pop();
        }
        states.push((tick, stations));
    }

    fn latest(&self, aircraft: u32) -> Option<&[StoreRecord]> {
        self.states
            .get(&aircraft)
            .and_then(|states| states.last())
            .map(|(_, stations)| stations.as_slice())
    }

    /// `aircraft`'s stores at `tick`: its newest recorded state at or before
    /// it. `None` when the recording has none for it then.
    pub fn at(&self, aircraft: u32, tick: u64) -> Option<&[StoreRecord]> {
        let states = self.states.get(&aircraft)?;
        let after = states.partition_point(|(t, _)| *t <= tick);
        states
            .get(after.checked_sub(1)?)
            .map(|(_, stations)| stations.as_slice())
    }

    /// Whether the recording keeps any stores.
    pub fn is_empty(&self) -> bool {
        self.states.is_empty()
    }
}

/// A `stores.station` event's store.
fn record(event: &Event) -> StoreRecord {
    let int = |name: &str| event.num(name).unwrap_or(0.);
    let byte = |name: &str| int(name).clamp(0., 255.) as u8;
    let count = |name: &str| int(name).clamp(0., f64::from(u16::MAX)) as u16;
    StoreRecord {
        hardpoint: byte("hardpoint"),
        mount: [int("x_ft"), int("y_ft"), int("z_ft")],
        location: byte("location"),
        paired: event.flag("paired").unwrap_or(false),
        shape: event.string("shape").map(str::to_owned),
        look: StoreLook::of(event.string("look").unwrap_or("store")),
        items: count("items"),
        loaded: count("loaded"),
        body: [int("length_ft"), int("diameter_ft")],
        over: OVER.map(|name| event.num(name)),
        tip: event
            .num("tip_right_ft")
            .zip(event.num("tip_up_ft"))
            .map(|(right, up)| [right, up]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store(items: u16) -> StoreRecord {
        StoreRecord {
            hardpoint: 6,
            mount: [9.5, -1.25, 2.],
            location: 4,
            paired: true,
            shape: Some("MK82.3".into()),
            look: StoreLook::Store,
            items,
            loaded: 4,
            body: [0.; 2],
            over: [Some(0.25), None, Some(-0.5), None, None],
            tip: Some([19.5, 0.25]),
        }
    }

    fn tank() -> StoreRecord {
        StoreRecord {
            hardpoint: 2,
            mount: [0., -2., 1.],
            look: StoreLook::Tank,
            items: 1,
            loaded: 1,
            body: [13., 2.],
            ..StoreRecord::default()
        }
    }

    fn timed(tick: u64, events: Vec<Event>) -> Vec<TimedEvent> {
        events
            .into_iter()
            .map(|event| TimedEvent { tick, event })
            .collect()
    }

    /// A layout and its releases come back at every tick, whatever tick is
    /// asked first; before the first event the aircraft has none.
    #[test]
    fn the_track_gives_each_tick_its_stores() {
        let full = vec![store(4), tank()];
        let mut events = timed(10, change_events(7, None, &full));
        let mut three = full.clone();
        three[0].items = 3;
        events.extend(timed(50, change_events(7, Some(&full), &three)));
        assert_eq!(events.len(), 4, "a layout of two, then the items");
        let mut empty = three.clone();
        empty[0].items = 0;
        events.extend(timed(90, change_events(7, Some(&three), &empty)));
        assert!(change_events(7, Some(&empty), &empty).is_empty());
        let track = StoreTrack::from_events(&events);
        assert_eq!(track.at(7, 9), None);
        assert_eq!(track.at(7, 10), Some(full.as_slice()));
        assert_eq!(track.at(7, 89), Some(three.as_slice()));
        assert_eq!(track.at(7, 500), Some(empty.as_slice()));
        assert_eq!(track.at(8, 500), None);
    }

    /// A new layout (a respawn with another loadout) replaces the old one.
    #[test]
    fn a_changed_layout_is_recorded_whole() {
        let before = vec![store(2)];
        let after = vec![tank()];
        let events = change_events(3, Some(&before), &after);
        assert_eq!(events[0].kind, kind::STORES_LAYOUT);
        let mut all = timed(0, layout_events(3, &before));
        all.extend(timed(5, events));
        let track = StoreTrack::from_events(&all);
        assert_eq!(track.at(3, 4), Some(before.as_slice()));
        assert_eq!(track.at(3, 5), Some(after.as_slice()));
    }
}
