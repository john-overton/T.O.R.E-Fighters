//! A carried weapon needs its mesh before its first launch. The projectile
//! catalog alone omits weapons never fired during a recording.
use tore_replay::{Recording, TimedEvent, vocab::kind};

pub(crate) fn shapes(recording: &Recording) -> impl Iterator<Item = &str> {
    recording
        .weapons()
        .filter_map(|w| w.shape.as_deref())
        .chain(station_shapes(recording.events()))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
}

pub(crate) fn station_shapes(events: &[TimedEvent]) -> impl Iterator<Item = &str> {
    events.iter().filter_map(|timed| {
        (timed.event.kind == kind::STORES_STATION)
            .then(|| timed.event.string("shape"))
            .flatten()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use tore_replay::{
        Event,
        stores::{StoreRecord, layout_events},
    };

    #[test]
    fn unfired_and_late_loadouts_supply_shapes_without_projectiles() {
        let layout = |id, tick, shape: &str| {
            layout_events(
                id,
                &[StoreRecord {
                    shape: Some(shape.into()),
                    items: 2,
                    loaded: 2,
                    ..StoreRecord::default()
                }],
            )
            .into_iter()
            .map(move |event| TimedEvent { tick, event })
        };
        let mut events: Vec<_> = layout(0, 0, "AIM9.3")
            .chain(layout(7, 0, "AA10.3"))
            .collect();
        let mut seen = BTreeSet::new();
        assert_eq!(
            station_shapes(&events)
                .filter(|s| seen.insert((*s).to_owned()))
                .count(),
            2
        );
        events.extend(layout(9, 600, "MK82.3"));
        events.push(TimedEvent {
            tick: 600,
            event: Event::new("unrelated").with("shape", "IGNORE.3"),
        });
        assert_eq!(
            station_shapes(&events)
                .filter(|s| seen.insert((*s).to_owned()))
                .collect::<Vec<_>>(),
            ["MK82.3"]
        );
        assert_eq!(station_shapes(&[]).count(), 0);
    }
}
