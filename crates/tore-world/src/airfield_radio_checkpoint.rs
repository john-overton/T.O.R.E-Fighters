//! The coders of the tower conversation of each cockpit and of the AI
//! wingmen's airfield reports (the wing status section;
//! docs/formats/checkpoint.md).
//!
//! Stage H slice H7. [`AirfieldRadio`](super::AirfieldRadio) sits in a
//! cockpit, so the cockpits section (slice H2) calls its coder. Skipped:
//!
//! - `AirfieldRadio::notes`: why-record (journal entries waiting to be handed
//!   to the channel, which only journals them; a restored radio hands over
//!   none).
//!
//! `Notice::queued` is read only for the journal's age and the call's
//! `since`, but it is coded: it costs 64 bits and a restored notice then
//! compares equal. The runway views a conversation holds are copied records,
//! coded as shared records.

use super::{AirfieldRadio, Approach, Departure, Notice, WingStatus};
use tore_sim::ai::airfield::RunwayView;
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver};

/// An optional runway view, the view itself a shared record.
fn save_runway(s: &mut Saver, runway: &Option<RunwayView>) -> Result<(), CheckpointError> {
    s.writer().write_bool(runway.is_some());
    match runway {
        Some(view) => s.shared(view),
        None => Ok(()),
    }
}

fn load_runway(l: &mut Loader<'_>) -> Result<Option<RunwayView>, CheckpointError> {
    if l.reader().read_bool()? {
        Ok(Some(l.shared()?))
    } else {
        Ok(None)
    }
}

impl Checkpoint for Departure {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Departure {
            runway,
            cleared,
            airborne,
            farewell,
            hold,
        } = self;
        save_runway(s, runway)?;
        cleared.save(s, None)?;
        airborne.save(s, None)?;
        farewell.save(s, None)?;
        hold.save(s, None)
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(Departure {
            runway: load_runway(l)?,
            cleared: Checkpoint::load(l, None)?,
            airborne: Checkpoint::load(l, None)?,
            farewell: Checkpoint::load(l, None)?,
            hold: Checkpoint::load(l, None)?,
        })
    }
}

impl Checkpoint for Approach {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Approach {
            runway,
            wind,
            landed,
            welcomed,
        } = self;
        save_runway(s, runway)?;
        wind.save(s, None)?;
        landed.save(s, None)?;
        welcomed.save(s, None)
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(Approach {
            runway: load_runway(l)?,
            wind: Checkpoint::load(l, None)?,
            landed: Checkpoint::load(l, None)?,
            welcomed: Checkpoint::load(l, None)?,
        })
    }
}

tore_sim::checkpoint_struct!(Notice {
    actor,
    key,
    until,
    queued,
    call,
});

tore_sim::checkpoint_struct!(AirfieldRadio {
    seat,
    plane,
    departure,
    approach,
    clearance_announced,
    landing_count,
    landing_score,
    pending,
    next,
    clock,
} skip {
    // Why-record: journal entries waiting for the next delivery.
    notes = Vec::new(),
});

tore_sim::checkpoint_struct!(WingStatus { memory });

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comms::{Call, Kind, Phrase};
    use crate::seats::SeatId;
    use tore_sim::ai::airfield::{AirfieldAnchors, Phase};
    use tore_sim::checkpoint::{Coded, Models, from_bytes, to_bytes};

    fn models() -> Models {
        Models::default()
    }

    fn coded<T: Checkpoint>(value: &T) -> Coded {
        to_bytes(value, &models()).expect("it codes")
    }

    fn restored<T: Checkpoint>(value: &T) -> T {
        from_bytes(&coded(value), &models()).expect("it decodes")
    }

    fn runway(object: u32) -> RunwayView {
        RunwayView {
            airport: 1,
            object,
            center: [1000. + f64::from(object), 0., -2000.],
            heading: 1.25,
            length_ft: 9000.,
            elevation_ft: 120.,
            anchors: Some(AirfieldAnchors {
                taxi_out: [[0.; 3]; 4],
                takeoff_spot: [1., 2., 3.],
                takeoff_heading: 0.5,
                landing_point: [4., 5., 6.],
                landing_heading: 0.75,
                taxi_in: [[7., 8., 9.]; 4],
                parking: [[10., 11., 12.]; 9],
                parking_heading: 1.5,
            }),
        }
    }

    fn notice(actor: u32, key: u8, at: f64) -> Notice {
        Notice {
            actor,
            key,
            until: at + 15.,
            queued: at,
            call: Call::new(
                "Tower to Red one",
                Phrase::default().raw("Cleared to land", Some("^CLRLAND")),
                if key.is_multiple_of(2) {
                    Kind::Important
                } else {
                    Kind::Chatter
                },
            )
            .airport(),
        }
    }

    /// A conversation part way through: runways held, flags set, notices
    /// waiting, the pacing clock running.
    fn mid_conversation() -> AirfieldRadio {
        let mut radio = AirfieldRadio::for_seat(SeatId(2), 7);
        radio.departure = Departure {
            runway: Some(runway(31)),
            cleared: true,
            airborne: false,
            farewell: true,
            hold: false,
        };
        radio.approach = Approach {
            runway: Some(runway(31)),
            wind: true,
            landed: false,
            welcomed: true,
        };
        radio.clearance_announced = Some(31);
        radio.landing_count = 3;
        radio.landing_score = 250;
        radio.pending.push_back(notice(u32::MAX, 1, 10.));
        radio.pending.push_back(notice(4, 2, 11.));
        radio.pending.push_back(notice(5, 3, 12.5));
        radio.next = 14.;
        radio.clock = 12.5;
        radio
    }

    /// The notices of `radio` as text: `Notice` has no `PartialEq`, and a
    /// call's origin is a why-record the coder leaves out.
    fn shape(radio: &AirfieldRadio) -> String {
        let AirfieldRadio {
            seat,
            plane,
            departure,
            approach,
            clearance_announced,
            landing_count,
            landing_score,
            pending,
            next,
            clock,
            notes: _,
        } = radio;
        let notices: Vec<_> = pending
            .iter()
            .map(|n| {
                (
                    n.actor,
                    n.key,
                    n.until,
                    n.queued,
                    n.call.line(),
                    n.call.kind,
                )
            })
            .collect();
        format!(
            "{seat:?} {plane} {:?} {} {} {} {} {} {:?} {:?} {clearance_announced:?} \
             {landing_count} {landing_score} {notices:?} {next:?} {clock:?}",
            departure.runway,
            departure.cleared,
            departure.airborne,
            departure.farewell,
            departure.hold,
            approach.wind,
            approach.runway,
            (approach.landed, approach.welcomed),
        )
    }

    #[test]
    fn a_tower_conversation_round_trips() {
        let radio = mid_conversation();
        let copy = restored(&radio);
        assert_eq!(shape(&copy), shape(&radio));
        assert_eq!(coded(&copy), coded(&radio));
        assert_eq!(copy.pending.len(), 3);
    }

    #[test]
    fn a_fresh_conversation_round_trips() {
        let radio = AirfieldRadio::for_seat(SeatId(0), 1);
        let copy = restored(&radio);
        assert_eq!(shape(&copy), shape(&radio));
        assert_eq!(coded(&copy), coded(&radio));
    }

    #[test]
    fn the_runway_views_of_a_conversation_are_shared_records() {
        // Departure and approach hold the same view: one record.
        let radio = mid_conversation();
        assert_eq!(coded(&radio).records.len(), 1);
    }

    #[test]
    fn the_waiting_notes_are_left_out_and_delivery_goes_on_identically() {
        use crate::comms::Comms;
        let mut original = mid_conversation();
        original.notes.push(crate::comms::journal::Entry::note(
            12.,
            "Tower",
            crate::comms::journal::Origin::default(),
            crate::comms::journal::Outcome::Cancelled(crate::comms::journal::Reason::TowerReply),
        ));
        let mut copy = restored(&original);
        assert!(copy.notes.is_empty(), "the notes are why-records");
        let (mut a, mut b) = (
            Comms::with_seats(3, [SeatId(2)]),
            Comms::with_seats(3, [SeatId(2)]),
        );
        let mut heard = 0;
        for tick in 0..2400u32 {
            let now = 12.5 + f64::from(tick) / 120.;
            original.deliver(now, &mut a);
            copy.deliver(now, &mut b);
            let (x, y) = (a.due(now), b.due(now));
            assert_eq!(x, y, "the deliveries agree at tick {tick}");
            heard += x.len();
        }
        assert!(heard >= 3, "the three notices were delivered, {heard}");
        assert_eq!(shape(&original), shape(&copy));
    }

    #[test]
    fn the_wing_status_memory_round_trips() {
        let mut status = WingStatus::default();
        assert_eq!(restored(&status).memory, status.memory);
        for (id, phase, turns) in [
            (3, Some(Phase::Taxi), 0),
            (4, Some(Phase::Final), 2),
            (9, None, 1),
            (12, Some(Phase::Parked), 0),
        ] {
            status.memory.insert(id, (phase, turns));
        }
        let copy = restored(&status);
        assert_eq!(copy.memory, status.memory);
        assert_eq!(coded(&copy), coded(&status));
    }

    #[test]
    fn damaged_conversations_are_refused_without_a_panic() {
        let good = coded(&mid_conversation());
        for cut in 0..good.body.len() {
            let damaged = Coded {
                body: good.body[..cut].to_vec(),
                records: good.records.clone(),
            };
            assert!(from_bytes::<AirfieldRadio>(&damaged, &models()).is_err());
        }
        for at in 0..good.body.len() {
            let mut body = good.body.clone();
            body[at] ^= 0xff;
            let _ = from_bytes::<AirfieldRadio>(
                &Coded {
                    body,
                    records: good.records.clone(),
                },
                &models(),
            );
        }
        // A record that is not a runway view.
        let mut bad = good.clone();
        bad.records[0].truncate(2);
        assert!(from_bytes::<AirfieldRadio>(&bad, &models()).is_err());
    }
}
