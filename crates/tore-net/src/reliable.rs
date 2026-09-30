//! Reliable ordered messages: the Messages section (kind 1).
//!
//! Each message has a 16-bit id counting up from 0 in each direction, a kind
//! byte and a body of up to 256 bytes. A larger message, up to 64 KB, is split
//! into 256-byte fragments that are messages of their own; the first carries
//! the kind and the total length, and in-order delivery puts them back
//! together. The sender keeps a window of 256 unacknowledged ids; a message is
//! acknowledged when a packet that carried it is. See "Reliable messages" in
//! [`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md).
//!
//! The section body is bit packed: a record count (8 bits, 1 to 255), then
//! each record:
//!
//! | Field | Bits | Present |
//! | --- | --- | --- |
//! | Id | 16 | always |
//! | Part | 2 | always: 0 whole, 1 first fragment, 2 later fragment |
//! | Kind | 8 | whole and first |
//! | Total length less one | 16 | first only; the total is 257 to 65,536 |
//! | Body length | 9 | always: whole 0 to 256, first exactly 256, later 1 to 256 |
//! | Body | 8 per byte | always |
//!
//! then zero bits to the byte boundary.

use std::collections::VecDeque;
use std::time::Duration;

use tore_codec::{BitReader, BitWriter};

use crate::SendError;

/// The largest message body, and the fragment size.
pub const MAX_MESSAGE_BODY: usize = 256;
/// The largest message, reassembled: 64 KB.
pub const MAX_MESSAGE_LEN: usize = 65_536;
/// Unacknowledged message ids the sender may have on the wire.
pub const MESSAGE_WINDOW: usize = 256;
/// Messages (fragments count one each) a sender queues, in the window and
/// waiting for it, before [`SendError::QueueFull`] (agent decision: 2 MB).
pub const MAX_QUEUED_MESSAGES: usize = 8_192;
/// The shortest resend interval.
pub const MIN_RESEND: Duration = Duration::from_millis(30);
const MAX_RECORDS: usize = 255;

/// Which part of a message a record is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    Whole,
    First,
    Later,
}

impl Part {
    fn code(self) -> u64 {
        match self {
            Self::Whole => 0,
            Self::First => 1,
            Self::Later => 2,
        }
    }

    fn from_code(code: u64) -> Option<Self> {
        match code {
            0 => Some(Self::Whole),
            1 => Some(Self::First),
            2 => Some(Self::Later),
            _ => None,
        }
    }
}

/// One record of a Messages section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Record {
    pub id: u16,
    pub part: Part,
    /// The message kind; 0 for a later fragment.
    pub kind: u8,
    /// The reassembled length, for a first fragment; 0 otherwise.
    pub total: u32,
    pub body: Vec<u8>,
}

/// A record's size in bits.
fn record_bits(part: Part, len: usize) -> usize {
    let kind = if part == Part::Later { 0 } else { 8 };
    let total = if part == Part::First { 16 } else { 0 };
    16 + 2 + kind + total + 9 + 8 * len
}

/// The section's size in bytes, header included, for `bits` of body.
fn section_bytes(bits: usize) -> usize {
    crate::packet::SECTION_HEADER_LEN + bits.div_ceil(8)
}

fn encode(records: &[&Outgoing]) -> Vec<u8> {
    let mut w = BitWriter::new();
    w.write_bits(records.len() as u64, 8).ok();
    for r in records {
        w.write_bits(u64::from(r.id), 16).ok();
        w.write_bits(r.part.code(), 2).ok();
        if r.part != Part::Later {
            w.write_bits(u64::from(r.kind), 8).ok();
        }
        if r.part == Part::First {
            w.write_bits(u64::from(r.total - 1), 16).ok();
        }
        w.write_bits(r.body.len() as u64, 9).ok();
        w.write_bytes(&r.body);
    }
    w.finish()
}

/// Decodes a Messages section body; `None` if it breaks any rule.
pub(crate) fn decode(body: &[u8]) -> Option<Vec<Record>> {
    let mut r = BitReader::new(body);
    let count = r.read_bits(8).ok()? as usize;
    if count == 0 {
        return None;
    }
    let mut records = Vec::with_capacity(count);
    for _ in 0..count {
        let id = r.read_bits(16).ok()? as u16;
        let part = Part::from_code(r.read_bits(2).ok()?)?;
        let kind = if part == Part::Later {
            0
        } else {
            r.read_bits(8).ok()? as u8
        };
        let total = if part == Part::First {
            r.read_bits(16).ok()? as u32 + 1
        } else {
            0
        };
        let len = r.read_bits(9).ok()? as usize;
        let fits = match part {
            Part::Whole => len <= MAX_MESSAGE_BODY,
            Part::First => len == MAX_MESSAGE_BODY && total as usize > MAX_MESSAGE_BODY,
            Part::Later => (1..=MAX_MESSAGE_BODY).contains(&len),
        };
        if !fits {
            return None;
        }
        let body = r.read_bytes(len).ok()?;
        records.push(Record {
            id,
            part,
            kind,
            total,
            body,
        });
    }
    (r.bits_remaining() < 8 && r.only_zero_padding_left()).then_some(records)
}

/// A queued message record on the sending side.
#[derive(Debug, Clone)]
struct Outgoing {
    id: u16,
    part: Part,
    kind: u8,
    total: u32,
    body: Vec<u8>,
    last_sent: Option<Duration>,
    acked: bool,
}

/// The sending side.
#[derive(Debug, Clone, Default)]
pub(crate) struct Sender {
    queue: VecDeque<Outgoing>,
    next_id: u16,
}

/// How long the sender waits before sending a message again.
pub(crate) fn resend_interval(round_trip: Duration) -> Duration {
    round_trip.mul_f64(1.25).max(MIN_RESEND)
}

impl Sender {
    /// Messages queued, fragments counted one each.
    pub(crate) fn queued(&self) -> usize {
        self.queue.len()
    }

    /// Queues a message, split into fragments when over 256 bytes.
    pub(crate) fn push(&mut self, kind: u8, body: &[u8]) -> Result<(), SendError> {
        if body.len() > MAX_MESSAGE_LEN {
            return Err(SendError::MessageTooLarge);
        }
        let parts = if body.len() <= MAX_MESSAGE_BODY {
            1
        } else {
            body.len().div_ceil(MAX_MESSAGE_BODY)
        };
        if self.queue.len() + parts > MAX_QUEUED_MESSAGES {
            return Err(SendError::QueueFull);
        }
        if parts == 1 {
            self.enqueue(Part::Whole, kind, 0, body.to_vec());
        } else {
            for (i, chunk) in body.chunks(MAX_MESSAGE_BODY).enumerate() {
                if i == 0 {
                    self.enqueue(Part::First, kind, body.len() as u32, chunk.to_vec());
                } else {
                    self.enqueue(Part::Later, 0, 0, chunk.to_vec());
                }
            }
        }
        Ok(())
    }

    fn enqueue(&mut self, part: Part, kind: u8, total: u32, body: Vec<u8>) {
        self.queue.push_back(Outgoing {
            id: self.next_id,
            part,
            kind,
            total,
            body,
            last_sent: None,
            acked: false,
        });
        self.next_id = self.next_id.wrapping_add(1);
    }

    fn window(&self) -> impl Iterator<Item = &Outgoing> {
        self.queue.iter().take(MESSAGE_WINDOW)
    }

    /// True when a message in the window has never been sent or is due again.
    pub(crate) fn has_due(&self, now: Duration, interval: Duration) -> bool {
        self.window().any(|m| due(m, now, interval))
    }

    /// Picks due messages for one packet and writes their section body.
    /// `room` is the bytes left in the packet; `budget` caps the section's
    /// bytes, except that the first record is taken whenever it fits the room.
    /// Returns the body and the ids it carries, or `None` when nothing is due
    /// or fits.
    pub(crate) fn select(
        &mut self,
        now: Duration,
        interval: Duration,
        room: usize,
        budget: usize,
    ) -> Option<(Vec<u8>, Vec<u16>)> {
        let mut bits = 8;
        let mut chosen = Vec::new();
        for (index, m) in self.queue.iter().take(MESSAGE_WINDOW).enumerate() {
            if chosen.len() == MAX_RECORDS {
                break;
            }
            if !due(m, now, interval) {
                continue;
            }
            let after = section_bytes(bits + record_bits(m.part, m.body.len()));
            let limit = if chosen.is_empty() {
                room
            } else {
                room.min(budget)
            };
            if after <= limit {
                bits += record_bits(m.part, m.body.len());
                chosen.push(index);
            }
        }
        if chosen.is_empty() {
            return None;
        }
        for &index in &chosen {
            self.queue[index].last_sent = Some(now);
        }
        let records: Vec<&Outgoing> = chosen.iter().map(|&i| &self.queue[i]).collect();
        let ids = records.iter().map(|m| m.id).collect();
        Some((encode(&records), ids))
    }

    /// Marks a message delivered and retires delivered ones from the front.
    pub(crate) fn ack(&mut self, id: u16) {
        let Some(front) = self.queue.front() else {
            return;
        };
        let index = usize::from(id.wrapping_sub(front.id));
        if index < self.queue.len().min(MESSAGE_WINDOW) {
            self.queue[index].acked = true;
        }
        while self.queue.front().is_some_and(|m| m.acked) {
            self.queue.pop_front();
        }
    }
}

fn due(m: &Outgoing, now: Duration, interval: Duration) -> bool {
    !m.acked
        && m.last_sent
            .is_none_or(|sent| now.saturating_sub(sent) > interval)
}

/// A message id outside the receive window, or fragments that do not fit
/// together: the peer broke the protocol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ProtocolError;

/// A large message being put back together.
#[derive(Debug, Clone)]
struct Assembly {
    kind: u8,
    total: usize,
    bytes: Vec<u8>,
}

/// The receiving side.
#[derive(Debug, Clone)]
pub(crate) struct Receiver {
    expected: u16,
    early: Vec<Option<Record>>,
    assembly: Option<Assembly>,
}

impl Default for Receiver {
    fn default() -> Self {
        Self {
            expected: 0,
            early: vec![None; MESSAGE_WINDOW],
            assembly: None,
        }
    }
}

impl Receiver {
    /// Checks that every record's id is inside the window (up to 255 ahead
    /// of the next id to hand over) or behind it, a duplicate. Behind means
    /// up to 32,768 back: a packet that waited on the way can carry an id the
    /// sender's window left far behind, but the receiver drops packets over
    /// 32 sequences old, and 32 packets carry at most 8,160 new ids, so no
    /// real duplicate is ever that far back.
    pub(crate) fn check(&self, records: &[Record]) -> Result<(), ProtocolError> {
        for r in records {
            let ahead = r.id.wrapping_sub(self.expected);
            if usize::from(ahead) >= MESSAGE_WINDOW && ahead < 0x8000 {
                return Err(ProtocolError);
            }
        }
        Ok(())
    }

    /// Takes one record; appends whole messages now ready, in order, to
    /// `out`. Duplicates are dropped; early records are held.
    pub(crate) fn accept(
        &mut self,
        record: Record,
        out: &mut Vec<(u8, Vec<u8>)>,
    ) -> Result<(), ProtocolError> {
        let ahead = usize::from(record.id.wrapping_sub(self.expected));
        if ahead >= MESSAGE_WINDOW {
            return Ok(());
        }
        if ahead > 0 {
            let slot = &mut self.early[usize::from(record.id) % MESSAGE_WINDOW];
            if slot.is_none() {
                *slot = Some(record);
            }
            return Ok(());
        }
        self.deliver(record, out)?;
        loop {
            self.expected = self.expected.wrapping_add(1);
            let slot = usize::from(self.expected) % MESSAGE_WINDOW;
            match self.early[slot].take() {
                Some(next) if next.id == self.expected => self.deliver(next, out)?,
                Some(other) => {
                    self.early[slot] = Some(other);
                    return Ok(());
                }
                None => return Ok(()),
            }
        }
    }

    fn deliver(
        &mut self,
        record: Record,
        out: &mut Vec<(u8, Vec<u8>)>,
    ) -> Result<(), ProtocolError> {
        match record.part {
            Part::Whole => {
                if self.assembly.is_some() {
                    return Err(ProtocolError);
                }
                out.push((record.kind, record.body));
            }
            Part::First => {
                if self.assembly.is_some() {
                    return Err(ProtocolError);
                }
                let total = record.total as usize;
                let mut bytes = Vec::with_capacity(total);
                bytes.extend_from_slice(&record.body);
                self.assembly = Some(Assembly {
                    kind: record.kind,
                    total,
                    bytes,
                });
            }
            Part::Later => {
                let Some(assembly) = self.assembly.as_mut() else {
                    return Err(ProtocolError);
                };
                assembly.bytes.extend_from_slice(&record.body);
                let have = assembly.bytes.len();
                if have > assembly.total {
                    return Err(ProtocolError);
                }
                if have == assembly.total {
                    let done = self.assembly.take().ok_or(ProtocolError)?;
                    out.push((done.kind, done.bytes));
                } else if record.body.len() < MAX_MESSAGE_BODY {
                    return Err(ProtocolError);
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    fn pass(sender: &mut Sender, receiver: &mut Receiver, now: Duration) -> Vec<(u8, Vec<u8>)> {
        let mut out = Vec::new();
        while let Some((body, ids)) = sender.select(now, MIN_RESEND, 1181, 1181) {
            let records = decode(&body).expect("decodes");
            receiver.check(&records).expect("in window");
            for r in records {
                receiver.accept(r, &mut out).expect("fits");
            }
            for id in ids {
                sender.ack(id);
            }
        }
        out
    }

    #[test]
    fn whole_and_fragmented_messages_arrive_in_order() {
        let mut s = Sender::default();
        let mut r = Receiver::default();
        let big: Vec<u8> = (0..MAX_MESSAGE_LEN).map(|i| (i * 31) as u8).collect();
        s.push(1, b"hello").unwrap();
        s.push(2, &big).unwrap();
        s.push(3, &[]).unwrap();
        s.push(4, &big[..257]).unwrap();
        assert_eq!(s.queued(), 1 + 256 + 1 + 2);
        let mut got = Vec::new();
        for step in 0..10 {
            got.extend(pass(&mut s, &mut r, MS * step));
        }
        assert_eq!(
            got,
            vec![
                (1, b"hello".to_vec()),
                (2, big.clone()),
                (3, vec![]),
                (4, big[..257].to_vec())
            ]
        );
        assert_eq!(s.queued(), 0);
        assert_eq!(
            s.push(0, &vec![0; MAX_MESSAGE_LEN + 1]),
            Err(SendError::MessageTooLarge)
        );
    }

    #[test]
    fn early_records_wait_and_duplicates_drop() {
        let mut s = Sender::default();
        for i in 0..4u8 {
            s.push(i, &[i]).unwrap();
        }
        let (body, _) = s.select(Duration::ZERO, MIN_RESEND, 1181, 1181).unwrap();
        let records = decode(&body).unwrap();
        let mut r = Receiver::default();
        let mut out = Vec::new();
        for rec in records.iter().rev() {
            r.accept(rec.clone(), &mut out).unwrap();
        }
        assert_eq!(out.len(), 4);
        assert_eq!(
            out.iter().map(|m| m.0).collect::<Vec<_>>(),
            vec![0, 1, 2, 3]
        );
        for rec in &records {
            r.accept(rec.clone(), &mut out).unwrap();
        }
        assert_eq!(out.len(), 4);
        assert!(r.check(&records).is_ok());
    }

    #[test]
    fn ids_beyond_the_window_are_protocol_errors() {
        let r = Receiver::default();
        let rec = |id| Record {
            id,
            part: Part::Whole,
            kind: 0,
            total: 0,
            body: vec![],
        };
        assert!(r.check(&[rec(255)]).is_ok());
        assert!(r.check(&[rec(256)]).is_err());
        assert!(r.check(&[rec(32_767)]).is_err());
        // Behind the next id: duplicates, however far back.
        assert!(r.check(&[rec(32_768)]).is_ok());
        assert!(r.check(&[rec(65_535)]).is_ok());
        assert!(r.check(&[rec(65_000)]).is_ok());
    }

    #[test]
    fn fragments_that_do_not_fit_are_protocol_errors() {
        let later = |id, len| Record {
            id,
            part: Part::Later,
            kind: 0,
            total: 0,
            body: vec![0; len],
        };
        let mut out = Vec::new();
        // A later fragment with no first.
        assert!(Receiver::default().accept(later(0, 10), &mut out).is_err());
        // A short fragment before the end.
        let mut r = Receiver::default();
        let first = Record {
            id: 0,
            part: Part::First,
            kind: 9,
            total: 1000,
            body: vec![0; 256],
        };
        r.accept(first.clone(), &mut out).unwrap();
        assert!(r.accept(later(1, 10), &mut out).is_err());
        // Overrunning the total.
        let mut r = Receiver::default();
        let mut small = first.clone();
        small.total = 300;
        r.accept(small, &mut out).unwrap();
        assert!(r.accept(later(1, 100), &mut out).is_err());
        // A whole message in the middle of a large one.
        let mut r = Receiver::default();
        r.accept(first, &mut out).unwrap();
        let whole = Record {
            id: 1,
            part: Part::Whole,
            kind: 1,
            total: 0,
            body: vec![],
        };
        assert!(r.accept(whole, &mut out).is_err());
        assert!(out.is_empty());
    }

    #[test]
    fn decode_rejects_non_canonical_sections() {
        assert!(decode(&[0]).is_none());
        assert!(decode(&[]).is_none());
        let mut s = Sender::default();
        s.push(7, b"abc").unwrap();
        let (body, _) = s.select(Duration::ZERO, MIN_RESEND, 1181, 1181).unwrap();
        assert!(decode(&body).is_some());
        let mut extra = body.clone();
        extra.push(0);
        assert!(decode(&extra).is_none());
        let mut dirty = body.clone();
        *dirty.last_mut().unwrap() |= 0x80;
        // 67 bits of records: the last byte's top bit is padding.
        assert!(decode(&dirty).is_none());
    }

    #[test]
    fn budget_limits_all_but_the_first_record() {
        let mut s = Sender::default();
        for _ in 0..10 {
            s.push(1, &[0; 256]).unwrap();
        }
        let (body, ids) = s.select(Duration::ZERO, MIN_RESEND, 1181, 100).unwrap();
        assert_eq!(ids.len(), 1);
        assert!(body.len() + 3 > 100);
        let (_, ids) = s.select(Duration::ZERO, MIN_RESEND, 1181, 1181).unwrap();
        assert_eq!(ids.len(), 4);
        // Nothing else fits a 200-byte room.
        assert!(s.select(Duration::ZERO, MIN_RESEND, 200, 1181).is_none());
        // Sent ones are not due until the interval passes.
        let (_, ids) = s.select(MS * 31, MIN_RESEND, 1181, 1181).unwrap();
        assert_eq!(ids, vec![0, 1, 2, 3]);
    }

    #[test]
    fn window_holds_back_messages_beyond_256() {
        let mut s = Sender::default();
        for _ in 0..300 {
            s.push(1, &[]).unwrap();
        }
        let mut sent = 0;
        while let Some((_, ids)) = s.select(Duration::ZERO, MIN_RESEND, 1181, 1181) {
            sent += ids.len();
        }
        assert_eq!(sent, MESSAGE_WINDOW);
        s.ack(0);
        let (_, ids) = s.select(Duration::ZERO, MIN_RESEND, 1181, 1181).unwrap();
        assert_eq!(ids, vec![256]);
    }
}
