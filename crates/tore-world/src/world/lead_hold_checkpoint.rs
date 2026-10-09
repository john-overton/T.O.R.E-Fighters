//! The coders of the lead hold (slice R2), written in the revival section
//! after the book's lists (docs/formats/checkpoint.md): whether the hold is
//! on, then each owned wing with its owner, the owner's newest plane and
//! whether it has led. The wing's side is coded field by field, as a roster
//! slot's is, since the type belongs to `tore-sim`.

use super::{LeadHold, LeadOwner, Owned};
use tore_sim::ai::launch::{Side, WingId};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, Loader, Saver, invalid};

impl Checkpoint for LeadOwner {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        match self {
            LeadOwner::Seat(seat) => {
                s.writer().write_varint(0);
                seat.save(s, None)
            }
            LeadOwner::Away(plane) => {
                s.writer().write_varint(1);
                plane.save(s, None)
            }
        }
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        match l.reader().read_varint()? {
            0 => Ok(LeadOwner::Seat(Checkpoint::load(l, None)?)),
            1 => Ok(LeadOwner::Away(Checkpoint::load(l, None)?)),
            other => invalid(format!("a lead owner has no variant {other}")),
        }
    }
}

impl Checkpoint for Owned {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let Owned {
            wing,
            owner,
            plane,
            led,
        } = self;
        let WingId { side, index } = wing;
        // Every variant, no catch-all, so a new side fails to compile here.
        s.writer().write_bool(match side {
            Side::Friendly => false,
            Side::Enemy => true,
        });
        index.save(s, None)?;
        owner.save(s, None)?;
        plane.save(s, None)?;
        led.save(s, None)
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let side = if l.reader().read_bool()? {
            Side::Enemy
        } else {
            Side::Friendly
        };
        Ok(Owned {
            wing: WingId {
                side,
                index: Checkpoint::load(l, None)?,
            },
            owner: Checkpoint::load(l, None)?,
            plane: Checkpoint::load(l, None)?,
            led: Checkpoint::load(l, None)?,
        })
    }
}

impl Checkpoint for LeadHold {
    fn save(&self, s: &mut Saver, _: Option<&Self>) -> Result<(), CheckpointError> {
        let LeadHold { on, owners } = self;
        on.save(s, None)?;
        owners.save(s, None)
    }

    fn load(l: &mut Loader<'_>, _: Option<&Self>) -> Result<Self, CheckpointError> {
        let hold = LeadHold {
            on: Checkpoint::load(l, None)?,
            owners: <Vec<Owned>>::load(l, None)?,
        };
        // A wing has one owner, and only a hold that is on has any: damaged
        // bytes that break this are refused.
        if !hold.on && !hold.owners.is_empty() {
            return invalid("the lead hold is off but names owners");
        }
        for (index, owned) in hold.owners.iter().enumerate() {
            if hold.owners[..index].iter().any(|o| o.wing == owned.wing) {
                return invalid("the lead hold names one wing's owner twice");
            }
        }
        Ok(hold)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seats::{PlaneId, SeatId};
    use tore_sim::checkpoint::{Models, from_bytes, round_trip, to_bytes};

    fn lived_in() -> LeadHold {
        LeadHold {
            on: true,
            owners: vec![
                Owned {
                    wing: WingId {
                        side: Side::Friendly,
                        index: 0,
                    },
                    owner: LeadOwner::Seat(SeatId(3)),
                    plane: PlaneId(14),
                    led: true,
                },
                Owned {
                    wing: WingId {
                        side: Side::Enemy,
                        index: 2,
                    },
                    owner: LeadOwner::Away(PlaneId(9)),
                    plane: PlaneId(9),
                    led: false,
                },
            ],
        }
    }

    #[test]
    fn a_lead_hold_with_owners_round_trips() {
        let models = Models::default();
        assert_eq!(round_trip(&lived_in(), &models).unwrap(), lived_in());
        let off = LeadHold::default();
        assert_eq!(round_trip(&off, &models).unwrap(), off);
    }

    #[test]
    fn damaged_lead_hold_bytes_are_refused_without_a_panic() {
        let models = Models::default();
        let coded = to_bytes(&lived_in(), &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut shorter = coded.clone();
            shorter.body.truncate(cut);
            assert!(from_bytes::<LeadHold>(&shorter, &models).is_err());
        }
        for bit in 0..coded.body.len() * 8 {
            let mut flipped = coded.clone();
            flipped.body[bit / 8] ^= 1 << (bit % 8);
            let _ = from_bytes::<LeadHold>(&flipped, &models);
        }
        // Off with owners, and one wing owned twice, are refused.
        let mut off = lived_in();
        off.on = false;
        assert!(from_bytes::<LeadHold>(&to_bytes(&off, &models).unwrap(), &models).is_err());
        let mut twice = lived_in();
        twice.owners[1].wing = twice.owners[0].wing;
        assert!(from_bytes::<LeadHold>(&to_bytes(&twice, &models).unwrap(), &models).is_err());
    }
}
