//! The coders of the AI's radio events and of what the chatter watch
//! remembers (docs/formats/checkpoint.md), part of the AI wings section.
//!
//! Stage H slice H6. The events waiting for `radio_calls` to drain, and the
//! watch's per-aircraft memory (who was alive, what each aimed at, when each
//! may report a contact again, the last fuel level said), are mutable state.
//!
//! The watch is restored in place, since it also holds setup. Not coded:
//!
//! - `Watch::names` and `Watch::seats`: setup. `AiWings::build_for` learns
//!   them from the types the mission loads, and nothing adds to them later
//!   (`insert_actor` does not call `learn`), so the fresh world has them.
//! - `Watch::engage_until`: why-record ("Journal only": it only tells the
//!   journal an accepted attack order's block from the contact cooldown).
//! - `Watch::journal`: why-record, drained into the radio channel's journal
//!   every step.
//!
//! `Release` and `Elevation` belong to this crate's radio, but only the AI's
//! events carry them, so their coders are here.

use super::{Chatter, Contact, ContactView, FuelLevel, Watch};
use crate::comms::Elevation;
use crate::radio_calls::Release;
use tore_sim::checkpoint::{Checkpoint, CheckpointError, InPlace, Loader, Saver, invalid};

tore_sim::checkpoint_struct!(Release {
    flags,
    seeker,
    phoenix,
    target,
});

tore_sim::checkpoint_enum!(Elevation {
    High = 0,
    Level = 1,
    Low = 2,
});

tore_sim::checkpoint_enum!(FuelLevel {
    Joker = 1,
    Bingo = 2,
    Fumes = 3,
    Out = 4,
});

tore_sim::checkpoint_struct!(ContactView {
    plane,
    named,
    hour,
    elevation,
    miles,
});

tore_sim::checkpoint_struct!(Contact {
    views,
    count,
    named,
    hour,
    elevation,
    miles,
    advise,
});

impl Checkpoint for Chatter {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            Chatter::Release { speaker, release } => {
                s.writer().write_varint(0);
                speaker.save(s, None)?;
                release.save(s, None)?;
            }
            Chatter::LaunchWarning {
                speaker,
                by_aircraft,
            } => {
                s.writer().write_varint(1);
                speaker.save(s, None)?;
                by_aircraft.save(s, None)?;
            }
            Chatter::Death {
                speaker,
                ejection_seat,
            } => {
                s.writer().write_varint(2);
                speaker.save(s, None)?;
                ejection_seat.save(s, None)?;
            }
            Chatter::Engage { speaker, aircraft } => {
                s.writer().write_varint(3);
                speaker.save(s, None)?;
                aircraft.save(s, None)?;
            }
            Chatter::Showtime { speaker } => {
                s.writer().write_varint(4);
                speaker.save(s, None)?;
            }
            Chatter::Contact {
                speaker,
                target,
                contact,
            } => {
                s.writer().write_varint(5);
                speaker.save(s, None)?;
                target.save(s, None)?;
                contact.save(s, None)?;
            }
            Chatter::Fuel { speaker, level } => {
                s.writer().write_varint(6);
                speaker.save(s, None)?;
                level.save(s, None)?;
            }
            Chatter::Leadership {
                speaker,
                side,
                wing_number,
                leader,
                previous_pilot_alive,
            } => {
                s.writer().write_varint(7);
                speaker.save(s, None)?;
                super::super::checkpoint::save_side(s, *side);
                wing_number.save(s, None)?;
                leader.save(s, None)?;
                previous_pilot_alive.save(s, None)?;
            }
        }
        Ok(())
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        Ok(match l.reader().read_varint()? {
            0 => Chatter::Release {
                speaker: Checkpoint::load(l, None)?,
                release: Checkpoint::load(l, None)?,
            },
            1 => Chatter::LaunchWarning {
                speaker: Checkpoint::load(l, None)?,
                by_aircraft: Checkpoint::load(l, None)?,
            },
            2 => Chatter::Death {
                speaker: Checkpoint::load(l, None)?,
                ejection_seat: Checkpoint::load(l, None)?,
            },
            3 => Chatter::Engage {
                speaker: Checkpoint::load(l, None)?,
                aircraft: Checkpoint::load(l, None)?,
            },
            4 => Chatter::Showtime {
                speaker: Checkpoint::load(l, None)?,
            },
            5 => Chatter::Contact {
                speaker: Checkpoint::load(l, None)?,
                target: Checkpoint::load(l, None)?,
                contact: Checkpoint::load(l, None)?,
            },
            6 => Chatter::Fuel {
                speaker: Checkpoint::load(l, None)?,
                level: Checkpoint::load(l, None)?,
            },
            7 => Chatter::Leadership {
                speaker: Checkpoint::load(l, None)?,
                side: super::super::checkpoint::load_side(l)?,
                wing_number: Checkpoint::load(l, None)?,
                leader: Checkpoint::load(l, None)?,
                previous_pilot_alive: Checkpoint::load(l, None)?,
            },
            other => return invalid(format!("Chatter has no variant {other}")),
        })
    }
}

impl InPlace for Watch {
    fn save_in_place(&self, s: &mut Saver) -> Result<(), CheckpointError> {
        let Watch {
            alive,
            targets,
            contacts,
            fuel,
            // Setup: learned from the mission's aircraft types at build.
            names: _,
            seats: _,
            // Why-record: journal only.
            engage_until: _,
            // Why-record: drained into the radio journal every step.
            journal: _,
        } = self;
        alive.save(s, None)?;
        targets.save(s, None)?;
        contacts.save(s, None)?;
        fuel.save(s, None)
    }

    fn restore_in_place(&mut self, l: &mut Loader<'_>) -> Result<(), CheckpointError> {
        let Watch {
            alive,
            targets,
            contacts,
            fuel,
            names: _,
            seats: _,
            engage_until,
            journal: _,
        } = self;
        *alive = Checkpoint::load(l, None)?;
        *targets = Checkpoint::load(l, None)?;
        *contacts = Checkpoint::load(l, None)?;
        *fuel = Checkpoint::load(l, None)?;
        // The explanation of a block that began before the checkpoint is gone.
        engage_until.clear();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_sim::ai::launch::Side;
    use tore_sim::checkpoint::{Models, from_bytes, round_trip, round_trip_in_place, to_bytes};

    fn models() -> Models {
        Models::default()
    }

    fn view(plane: u32, named: Option<&str>) -> ContactView {
        ContactView {
            plane,
            named: named.map(str::to_owned),
            hour: 11,
            elevation: Elevation::High,
            miles: 14,
        }
    }

    /// One of every kind of event, with its optional parts both ways.
    fn every_event() -> Vec<Chatter> {
        vec![
            Chatter::Release {
                speaker: 3,
                release: Release {
                    flags: 0x11,
                    seeker: 3,
                    phoenix: true,
                    target: Some(9),
                },
            },
            Chatter::Release {
                speaker: 4,
                release: Release {
                    flags: 0,
                    seeker: 2,
                    phoenix: false,
                    target: None,
                },
            },
            Chatter::LaunchWarning {
                speaker: 5,
                by_aircraft: true,
            },
            Chatter::LaunchWarning {
                speaker: 5,
                by_aircraft: false,
            },
            Chatter::Death {
                speaker: 6,
                ejection_seat: true,
            },
            Chatter::Engage {
                speaker: 7,
                aircraft: false,
            },
            Chatter::Showtime { speaker: 8 },
            Chatter::Contact {
                speaker: 2,
                target: 17,
                contact: Contact {
                    views: vec![view(0, Some("MiG-29")), view(1, None)],
                    count: 3,
                    named: Some("MiG-29".into()),
                    hour: 11,
                    elevation: Elevation::Low,
                    miles: 14,
                    advise: true,
                },
            },
            Chatter::Contact {
                speaker: 2,
                target: 18,
                contact: Contact {
                    views: Vec::new(),
                    count: 1,
                    named: None,
                    hour: 0,
                    elevation: Elevation::Level,
                    miles: 0,
                    advise: false,
                },
            },
            Chatter::Fuel {
                speaker: 1,
                level: FuelLevel::Bingo,
            },
            Chatter::Leadership {
                speaker: 2,
                side: Side::Enemy,
                wing_number: 3,
                leader: 4,
                previous_pilot_alive: true,
            },
            Chatter::Leadership {
                speaker: 2,
                side: Side::Friendly,
                wing_number: 1,
                leader: 6,
                previous_pilot_alive: false,
            },
        ]
    }

    #[test]
    fn every_event_round_trips() {
        let events = every_event();
        let copy: Vec<Chatter> = round_trip(&events, &models()).unwrap();
        assert_eq!(copy, events);
        for level in [
            FuelLevel::Joker,
            FuelLevel::Bingo,
            FuelLevel::Fumes,
            FuelLevel::Out,
        ] {
            assert_eq!(round_trip(&level, &models()).unwrap(), level);
        }
        // The numbers are the levels the radio announces, in order.
        assert_eq!(
            [
                FuelLevel::Joker,
                FuelLevel::Bingo,
                FuelLevel::Fumes,
                FuelLevel::Out
            ]
            .map(|l| l as u8),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn an_unknown_event_is_refused() {
        let mut coded = to_bytes(&every_event(), &models()).unwrap();
        // The first event's number is the 8 bits after the one-byte count;
        // 0x1f is no event.
        coded.body[1] = 0x1f;
        assert!(from_bytes::<Vec<Chatter>>(&coded, &models()).is_err());
        for cut in 0..coded.body.len() {
            let mut shorter = to_bytes(&every_event(), &models()).unwrap();
            shorter.body.truncate(cut);
            let _ = from_bytes::<Vec<Chatter>>(&shorter, &models());
        }
    }

    fn watched() -> Watch {
        let mut watch = Watch::default();
        watch.alive.extend([(1, true), (2, false), (3, true)]);
        watch.targets.extend([(1, Some(3)), (2, None)]);
        watch
            .contacts
            .extend([(1, (1800, Some(3))), (2, (0, None))]);
        watch
            .fuel
            .extend([(1, FuelLevel::Joker), (3, FuelLevel::Out)]);
        watch.engage_until.insert(1, 2400);
        watch
    }

    #[test]
    fn a_watch_restores_in_place_and_keeps_its_setup() {
        let original = watched();
        let mut setup = Watch::default();
        setup.names.insert("F18.PT", "F/A-18".into());
        setup.seats.insert("F18.PT", true);
        // The fresh watch has structure of its own the checkpoint replaces.
        setup.alive.insert(77, true);
        setup.engage_until.insert(5, 9);
        round_trip_in_place(&original, &mut setup, &models()).unwrap();
        assert_eq!(setup.alive, original.alive);
        assert_eq!(setup.targets, original.targets);
        assert_eq!(setup.contacts, original.contacts);
        assert_eq!(setup.fuel, original.fuel);
        assert_eq!(setup.names["F18.PT"], "F/A-18");
        assert!(setup.seats["F18.PT"]);
        // A why-record: the journal's block explanation is not carried over.
        assert!(setup.engage_until.is_empty());
    }

    #[test]
    fn the_watch_leaves_the_why_records_and_setup_out() {
        let quiet = Watch::default();
        let mut loud = Watch::default();
        loud.names.insert("F18.PT", "F/A-18".into());
        loud.seats.insert("F18.PT", false);
        loud.engage_until.insert(1, 5);
        let (mut a, mut b) = (Saver::new(), Saver::new());
        quiet.save_in_place(&mut a).unwrap();
        loud.save_in_place(&mut b).unwrap();
        assert_eq!(a.finish_section(), b.finish_section());
    }
}
