//! Private, unicast rejoin reservation checks, protocol 71. No connection is made.
use super::{PacketError, Token, end, u8_of, u32_of};
use tore_codec::{BitReader, BitWriter};

/// Header plus nonce, session and token. Replies must fit inside this size.
pub const RESERVATION_QUERY_LEN: usize = 37;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservationQuery {
    pub nonce: u64,
    pub session_id: u64,
    pub token: Token,
}

/// A currently reclaimable aircraft. The remaining time limits the token,
/// not the aircraft's survival or the mission's duration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservedSlot {
    pub plane: u32,
    pub enemy: bool,
    /// Zero-based wing and member within that side.
    pub wing: u8,
    pub member: u8,
    pub token_seconds: u32,
    /// Upper bound from token expiry and any currently active mission deadlines.
    pub return_seconds: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReservationAnswer {
    pub nonce: u64,
    pub session_id: u64,
    /// None is a confirmed absence of a reclaimable reservation.
    pub held: Option<ReservedSlot>,
}

impl ReservationQuery {
    pub(super) fn write(&self, w: &mut BitWriter) {
        w.write_bits(self.nonce, 64).ok();
        w.write_bits(self.session_id, 64).ok();
        self.token.write(w);
    }
}

impl ReservationAnswer {
    pub(super) fn write(&self, w: &mut BitWriter) -> Result<(), super::EncodeError> {
        if self.held.is_some_and(|slot| {
            slot.wing >= 3
                || slot.token_seconds > 86_400
                || slot.return_seconds > slot.token_seconds
        }) {
            return Err(super::EncodeError::BadReservation);
        }
        w.write_bits(self.nonce, 64).ok();
        w.write_bits(self.session_id, 64).ok();
        w.write_bits(u64::from(self.held.is_some()), 8).ok();
        if let Some(slot) = self.held {
            w.write_bits(u64::from(slot.plane), 32).ok();
            w.write_bits(u64::from(slot.enemy), 8).ok();
            w.write_bits(u64::from(slot.wing), 8).ok();
            w.write_bits(u64::from(slot.member), 8).ok();
            w.write_bits(u64::from(slot.token_seconds), 32).ok();
            w.write_bits(u64::from(slot.return_seconds), 32).ok();
        }
        Ok(())
    }
}

pub fn decode_reservation_query(body: &[u8]) -> Result<ReservationQuery, PacketError> {
    let mut r = BitReader::new(body);
    let query = ReservationQuery {
        nonce: r.read_bits(64)?,
        session_id: r.read_bits(64)?,
        token: Token::read(&mut r)?,
    };
    end(&r)?;
    Ok(query)
}

pub fn decode_reservation_answer(body: &[u8]) -> Result<ReservationAnswer, PacketError> {
    let mut r = BitReader::new(body);
    let nonce = r.read_bits(64)?;
    let session_id = r.read_bits(64)?;
    let held = match u8_of(&mut r)? {
        0 => None,
        1 => {
            let plane = u32_of(&mut r)?;
            let enemy = match u8_of(&mut r)? {
                0 => false,
                1 => true,
                _ => return Err(PacketError::Malformed),
            };
            let wing = u8_of(&mut r)?;
            let member = u8_of(&mut r)?;
            let token_seconds = u32_of(&mut r)?;
            let return_seconds = u32_of(&mut r)?;
            if wing >= 3 || token_seconds > 86_400 || return_seconds > token_seconds {
                return Err(PacketError::Malformed);
            }
            Some(ReservedSlot {
                plane,
                enemy,
                wing,
                member,
                token_seconds,
                return_seconds,
            })
        }
        _ => return Err(PacketError::Malformed),
    };
    end(&r)?;
    Ok(ReservationAnswer {
        nonce,
        session_id,
        held,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::packet::Packet;

    #[test]
    fn reservation_packets_are_bounded_versioned_and_round_trip() {
        let query = ReservationQuery {
            nonce: 23,
            session_id: 42,
            token: Token(7),
        };
        let packets = [
            Packet::ReservationQuery(query),
            Packet::ReservationAnswer(ReservationAnswer {
                nonce: 23,
                session_id: 42,
                held: None,
            }),
            Packet::ReservationAnswer(ReservationAnswer {
                nonce: 23,
                session_id: 42,
                held: Some(ReservedSlot {
                    plane: 7,
                    enemy: true,
                    wing: 1,
                    member: 2,
                    token_seconds: 192,
                    return_seconds: 192,
                }),
            }),
        ];
        for packet in packets {
            let bytes = packet.encode(71).unwrap();
            assert!(bytes.len() <= RESERVATION_QUERY_LEN);
            assert_eq!(Packet::decode(&bytes, 71).unwrap(), packet);
            assert!(Packet::decode(&bytes, 69).is_err());
            for len in 0..bytes.len() {
                assert!(Packet::decode(&bytes[..len], 71).is_err());
            }
        }
    }
}
