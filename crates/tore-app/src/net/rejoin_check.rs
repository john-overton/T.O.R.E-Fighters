//! A selected game's private reservation preflight. Saved identity alone
//! never promises an aircraft. This never joins or occupies a slot.
use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;
use tore_net::packet::{Packet, ReservationQuery, ReservedSlot};
use tore_net::{Datagrams, Entropy, Token, TokenSource, bind_udp};
use tore_session::wire::PROTOCOL_VERSION;

pub type Target = (SocketAddr, u64, Token);
const POLL: Duration = Duration::from_secs(2);
const FRESH: Duration = Duration::from_secs(3);

#[derive(Default)]
pub struct Check {
    target: Option<Target>,
    socket: Option<UdpSocket>,
    sent: Option<Duration>,
    nonce: u64,
    answer: Option<(Duration, Option<ReservedSlot>)>,
    now: Duration,
}

impl Check {
    pub fn update(&mut self, now: Duration, target: Option<Target>) {
        if target != self.target {
            *self = Self {
                target,
                ..Self::default()
            };
            if let Some((address, _, _)) = target {
                let local = if address.is_ipv4() {
                    "0.0.0.0:0"
                } else {
                    "[::]:0"
                };
                self.socket = bind_udp(local.parse().expect("literal address")).ok();
            }
        }
        self.now = now;
        let Some((address, session_id, token)) = target else {
            return;
        };
        let Some(socket) = self.socket.as_mut() else {
            return;
        };
        let mut buffer = [0; tore_net::MAX_DATAGRAM];
        for _ in 0..16 {
            let Ok(Some((len, from))) = socket.recv_datagram(&mut buffer) else {
                break;
            };
            if from != address
                || self
                    .sent
                    .is_none_or(|sent| now.saturating_sub(sent) > FRESH)
            {
                continue;
            }
            if let Ok(Packet::ReservationAnswer(answer)) =
                Packet::decode(&buffer[..len], PROTOCOL_VERSION)
                && answer.nonce == self.nonce
                && answer.session_id == session_id
            {
                // Timestamp the query, not the receipt: a delayed answer does
                // not extend either freshness or the token's remaining life.
                self.answer = Some((self.sent.unwrap_or(now), answer.held));
            }
        }
        if self
            .sent
            .is_none_or(|sent| now.saturating_sub(sent) >= POLL)
        {
            self.nonce = TokenSource::new(Entropy::System).next_token().0 as u64;
            self.sent = Some(now);
            if let Ok(bytes) = Packet::ReservationQuery(ReservationQuery {
                nonce: self.nonce,
                session_id,
                token,
            })
            .encode(PROTOCOL_VERSION)
            {
                let _ = socket.send_datagram(address, &bytes);
            }
        }
    }

    pub fn line(&self, target: Target) -> Option<String> {
        if self.target != Some(target) {
            return None;
        }
        let (confirmed, held) = self.answer?;
        let age = self.now.saturating_sub(confirmed);
        if age > FRESH {
            return None;
        }
        let Some(slot) = held else {
            return Some("No reserved slot is available for your return.".into());
        };
        let seconds = u64::from(slot.return_seconds).saturating_sub(age.as_secs());
        if seconds == 0 {
            return None;
        }
        let side = if slot.enemy { "Red" } else { "Blue" };
        Some(format!(
            "You will be added back to {side} Wing {} #{} (up to {}:{:02}:{:02} more, unless lost, released or the mission ends).",
            u16::from(slot.wing) + 1,
            u16::from(slot.member) + 1,
            seconds / 3600,
            (seconds / 60) % 60,
            seconds % 60,
        ))
    }

    #[cfg(test)]
    pub fn confirmed_for_test(target: Target, held: Option<ReservedSlot>) -> Self {
        Self {
            target: Some(target),
            answer: Some((Duration::ZERO, held)),
            ..Self::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tore_net::packet::ReservationAnswer;

    #[test]
    fn only_a_fresh_matching_server_reply_promises_the_slot() {
        let mut server = bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
        let target = (server.local_addr().unwrap(), 42, Token(99));
        let mut check = Check::default();
        check.update(Duration::ZERO, Some(target));
        assert!(
            check.line(target).is_none(),
            "saved identity is not confirmation"
        );
        let mut buf = [0; tore_net::MAX_DATAGRAM];
        // Loopback delivery is not instant everywhere (macOS CI): wait for
        // each datagram rather than reading once.
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let (n, client) = loop {
            if let Some(got) = server.recv_datagram(&mut buf).unwrap() {
                break got;
            }
            assert!(std::time::Instant::now() < deadline, "no reservation query");
            std::thread::sleep(Duration::from_millis(1));
        };
        let settle = || std::thread::sleep(Duration::from_millis(50));
        let Packet::ReservationQuery(query) = Packet::decode(&buf[..n], PROTOCOL_VERSION).unwrap()
        else {
            panic!("reservation query");
        };
        assert_eq!(query.token, target.2);
        let held = Some(ReservedSlot {
            plane: 1,
            enemy: false,
            wing: 0,
            member: 1,
            token_seconds: 86_400,
            return_seconds: 192,
        });
        let reply = |nonce| {
            Packet::ReservationAnswer(ReservationAnswer {
                nonce,
                session_id: 42,
                held,
            })
            .encode(PROTOCOL_VERSION)
            .unwrap()
        };
        server
            .send_datagram(client, &reply(query.nonce ^ 1))
            .unwrap();
        settle();
        check.update(Duration::from_millis(1), Some(target));
        assert!(check.line(target).is_none(), "wrong nonce ignored");
        server.send_datagram(client, &reply(query.nonce)).unwrap();
        settle();
        check.update(Duration::from_secs(1), Some(target));
        let line = check.line(target).unwrap();
        assert!(line.contains("Blue Wing 1 #2"));
        assert!(line.contains("0:03:11 more"));
        let gone = Packet::ReservationAnswer(ReservationAnswer {
            nonce: query.nonce,
            session_id: 42,
            held: None,
        })
        .encode(PROTOCOL_VERSION)
        .unwrap();
        server.send_datagram(client, &gone).unwrap();
        settle();
        check.update(Duration::from_secs(2), Some(target));
        assert_eq!(
            check.line(target).as_deref(),
            Some("No reserved slot is available for your return.")
        );
        check.update(Duration::from_secs(4), Some(target));
        assert!(
            check.line(target).is_none(),
            "offline or timeout hides the old promise"
        );
        check.update(Duration::from_secs(4), None);
        assert!(check.line(target).is_none(), "clean leave clears the check");
    }

    #[test]
    fn not_held_and_selection_changes_cannot_claim_a_slot() {
        let target = ("127.0.0.1:31000".parse().unwrap(), 42, Token(99));
        let check = Check::confirmed_for_test(target, None);
        assert_eq!(
            check.line(target).as_deref(),
            Some("No reserved slot is available for your return.")
        );
        assert!(check.line((target.0, 43, target.2)).is_none());
        assert!(check.line((target.0, target.1, Token(100))).is_none());
    }
}
