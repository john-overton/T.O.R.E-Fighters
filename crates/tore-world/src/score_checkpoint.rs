//! The coder of the score recorder (the score section;
//! docs/formats/checkpoint.md). Stage H slice H10.
//!
//! The recorder is `World::score`, present only while the host has scoring on,
//! so the section is a flag and then the [`Recorder`]: the set of targets
//! whose end is recorded (a restored recorder must not record a kill or a loss
//! twice) and the facts waiting for the host.
//!
//! The host drains the facts after every step, so a checkpoint taken between
//! ticks by that loop holds none. They are coded anyway, with the tick they
//! belong to (agent decision): a checkpoint taken at any other point of the
//! host's loop then restores the same facts, and a restored recorder drained
//! before its first step returns the same tick. Nothing is skipped.

use super::{Fact, Facts, Flown, Recorder, Victim};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

tore_sim::checkpoint_struct!(Flown { plane, pilot });

tore_sim::checkpoint_struct!(Victim {
    target,
    flown,
    aircraft,
});

tore_sim::checkpoint_struct!(Facts { tick, facts });

impl Checkpoint for Fact {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Fact::Kill {
                shooter,
                victim,
                pilot_aboard,
            } => {
                s.writer().write_varint(0);
                shooter.save(s, None)?;
                victim.save(s, None)?;
                pilot_aboard.save(s, None)
            }
            Fact::Damage {
                shooter,
                victim,
                fraction,
            } => {
                s.writer().write_varint(1);
                shooter.save(s, None)?;
                victim.save(s, None)?;
                fraction.save(s, None)
            }
            Fact::Loss { plane, seat } => {
                s.writer().write_varint(2);
                plane.save(s, None)?;
                seat.save(s, None)
            }
        }
    }
    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        match l.reader().read_varint()? {
            0 => Ok(Fact::Kill {
                shooter: Checkpoint::load(l, None)?,
                victim: Checkpoint::load(l, None)?,
                pilot_aboard: Checkpoint::load(l, None)?,
            }),
            1 => Ok(Fact::Damage {
                shooter: Checkpoint::load(l, None)?,
                victim: Checkpoint::load(l, None)?,
                fraction: Checkpoint::load(l, None)?,
            }),
            2 => Ok(Fact::Loss {
                plane: Checkpoint::load(l, None)?,
                seat: Checkpoint::load(l, None)?,
            }),
            other => invalid(format!("a score fact has no kind {other}")),
        }
    }
}

tore_sim::checkpoint_struct!(Recorder { recorded, pending });

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seats::{Pilot, PlaneId, SeatId};
    use tore_sim::checkpoint::{Models, round_trip};

    fn flown(plane: u32, pilot: Pilot) -> Flown {
        Flown {
            plane: PlaneId(plane),
            pilot,
        }
    }

    #[test]
    fn a_recorder_with_every_kind_of_fact_waiting_round_trips() {
        let victim = Victim {
            target: 4,
            flown: Some(flown(4, Pilot::Human(SeatId(2)))),
            aircraft: true,
        };
        let mut recorder = Recorder::default();
        recorder.recorded.extend([4, 9, 31]);
        recorder.pending = Facts {
            tick: 812,
            facts: vec![
                Fact::Damage {
                    shooter: Some(flown(0, Pilot::Ai)),
                    victim,
                    fraction: 0.375,
                },
                Fact::Kill {
                    shooter: None,
                    victim: Victim {
                        target: 31,
                        flown: None,
                        aircraft: false,
                    },
                    pilot_aboard: false,
                },
                Fact::Kill {
                    shooter: Some(flown(1, Pilot::Lost)),
                    victim,
                    pilot_aboard: true,
                },
                Fact::Loss {
                    plane: PlaneId(4),
                    seat: SeatId(2),
                },
            ],
        };
        let copy = round_trip(&recorder, &Models::default()).unwrap();
        assert_eq!(copy, recorder);
        assert_eq!(copy.recorded().collect::<Vec<_>>(), [4, 9, 31]);
    }

    #[test]
    fn an_empty_recorder_round_trips() {
        let recorder = Recorder::default();
        let copy = round_trip(&recorder, &Models::default()).unwrap();
        assert_eq!(copy, recorder);
    }
}
