//! Acknowledgements, round trip, loss, rates and arrival spread for one
//! connection. See "Acknowledgements and round trip" in
//! [`docs/formats/net-protocol.md`](../../../docs/formats/net-protocol.md).

use std::collections::VecDeque;
use std::time::Duration;

use crate::packet::{ACK_DELAY_MAX, ACK_DELAY_NONE, ACK_DELAY_UNIT_MICROS};
use crate::sequence_newer;

/// How many sequences behind the newest one the ack bits cover.
pub(crate) const ACK_BITS: u16 = 32;
/// Sent packets remembered for acknowledgement; older ones count as lost.
const MAX_SENT_RECORDS: usize = 4096;
/// The loss and bad-packet window.
pub(crate) const RECENT_WINDOW: Duration = Duration::from_secs(5);
/// The rate window.
const RATE_WINDOW: Duration = Duration::from_secs(1);

/// What became of an incoming sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Arrival {
    /// Not seen before and recent enough to acknowledge.
    New,
    /// Already received.
    Duplicate,
    /// More than 32 behind the newest: too old to acknowledge.
    TooOld,
}

/// The receiving side's record of which sequences arrived.
#[derive(Debug, Clone, Default)]
pub(crate) struct ReceiveWindow {
    newest: Option<u16>,
    bits: u32,
    newest_at: Duration,
}

impl ReceiveWindow {
    pub(crate) fn arrival(&self, sequence: u16) -> Arrival {
        let Some(newest) = self.newest else {
            return Arrival::New;
        };
        if sequence == newest {
            return Arrival::Duplicate;
        }
        if sequence_newer(sequence, newest) {
            return Arrival::New;
        }
        let back = newest.wrapping_sub(sequence);
        if back > ACK_BITS {
            return Arrival::TooOld;
        }
        if self.bits & (1 << (back - 1)) != 0 {
            Arrival::Duplicate
        } else {
            Arrival::New
        }
    }

    /// Records a sequence that [`Self::arrival`] called new.
    pub(crate) fn record(&mut self, sequence: u16, now: Duration) {
        let Some(newest) = self.newest else {
            self.newest = Some(sequence);
            self.bits = 0;
            self.newest_at = now;
            return;
        };
        if sequence_newer(sequence, newest) {
            let shift = u32::from(sequence.wrapping_sub(newest));
            self.bits = if shift > u32::from(ACK_BITS) {
                0
            } else {
                ((u64::from(self.bits) << shift) | (1u64 << (shift - 1))) as u32
            };
            self.newest = Some(sequence);
            self.newest_at = now;
        } else {
            let back = newest.wrapping_sub(sequence);
            if (1..=ACK_BITS).contains(&back) {
                self.bits |= 1 << (back - 1);
            }
        }
    }

    /// The ack, ack bits and ack delay to send at `now`.
    pub(crate) fn ack_fields(&self, now: Duration) -> (u16, u32, u16) {
        match self.newest {
            None => (0, 0, ACK_DELAY_NONE),
            Some(newest) => {
                let units = now.saturating_sub(self.newest_at).as_micros()
                    / u128::from(ACK_DELAY_UNIT_MICROS);
                let delay = units.min(u128::from(ACK_DELAY_MAX)) as u16;
                (newest, self.bits, delay)
            }
        }
    }
}

/// One sent Payload awaiting its fate.
#[derive(Debug, Clone)]
struct SentPacket {
    sequence: u16,
    at: Duration,
    messages: Vec<u16>,
    acked: bool,
}

/// What one acknowledgement decided.
#[derive(Debug, Default)]
pub(crate) struct AckOutcome {
    /// Newly delivered sequences with the message ids each carried.
    pub delivered: Vec<(u16, Vec<u16>)>,
    /// Sequences now judged lost.
    pub lost: Vec<u16>,
    /// A round-trip sample, when the ack named a new newest packet.
    pub sample: Option<Duration>,
}

/// The sending side's record of its packets.
#[derive(Debug, Clone, Default)]
pub(crate) struct SentLog {
    packets: VecDeque<SentPacket>,
    next: u16,
    any_sent: bool,
    highest_ack: Option<u16>,
}

impl SentLog {
    pub(crate) fn next_sequence(&self) -> u16 {
        self.next
    }

    /// Records the packet with the next sequence. Returns a sequence that fell
    /// out of the record and so counts as lost, if any.
    pub(crate) fn record(&mut self, at: Duration, messages: Vec<u16>) -> Option<u16> {
        self.packets.push_back(SentPacket {
            sequence: self.next,
            at,
            messages,
            acked: false,
        });
        self.next = self.next.wrapping_add(1);
        self.any_sent = true;
        if self.packets.len() > MAX_SENT_RECORDS {
            let old = self.packets.pop_front()?;
            if !old.acked {
                return Some(old.sequence);
            }
        }
        None
    }

    /// True when `ack` names a packet this side has sent.
    pub(crate) fn ack_is_sane(&self, ack: u16) -> bool {
        let last = self.next.wrapping_sub(1);
        self.any_sent && !sequence_newer(ack, last)
    }

    fn index_of(&self, sequence: u16) -> Option<usize> {
        let front = self.packets.front()?.sequence;
        let index = usize::from(sequence.wrapping_sub(front));
        (index < self.packets.len()).then_some(index)
    }

    /// Applies an acknowledgement received at `now`.
    pub(crate) fn apply(
        &mut self,
        now: Duration,
        ack: u16,
        bits: u32,
        delay: Duration,
    ) -> AckOutcome {
        let mut outcome = AckOutcome::default();
        if self
            .highest_ack
            .is_none_or(|high| sequence_newer(ack, high))
        {
            self.highest_ack = Some(ack);
            if let Some(index) = self.index_of(ack) {
                let sent = self.packets[index].at;
                outcome.sample = Some(now.saturating_sub(sent).saturating_sub(delay));
            }
        }
        for back in 0..=ACK_BITS {
            let named = back == 0 || bits & (1 << (back - 1)) != 0;
            if !named {
                continue;
            }
            let sequence = ack.wrapping_sub(back);
            if let Some(index) = self.index_of(sequence) {
                let packet = &mut self.packets[index];
                if !packet.acked {
                    packet.acked = true;
                    outcome
                        .delivered
                        .push((sequence, std::mem::take(&mut packet.messages)));
                }
            }
        }
        // Retire from the front: delivered ones, and unacknowledged ones that
        // have left the acknowledgement window.
        while let Some(front) = self.packets.front() {
            if front.acked {
                self.packets.pop_front();
                continue;
            }
            let behind = ack.wrapping_sub(front.sequence);
            if behind > ACK_BITS && sequence_newer(ack, front.sequence) {
                outcome.lost.push(front.sequence);
                self.packets.pop_front();
                continue;
            }
            break;
        }
        outcome
    }
}

/// The smoothed round trip and its mean deviation (RFC 6298's gains).
#[derive(Debug, Clone, Copy)]
pub(crate) struct RoundTrip {
    mean: f64,
    deviation: f64,
    measured: bool,
}

/// The round trip assumed before the first sample (agent decision).
pub(crate) const INITIAL_ROUND_TRIP: Duration = Duration::from_millis(250);

impl RoundTrip {
    pub(crate) fn new(initial: Option<Duration>) -> Self {
        let mut rtt = Self {
            mean: INITIAL_ROUND_TRIP.as_secs_f64(),
            deviation: INITIAL_ROUND_TRIP.as_secs_f64() / 2.0,
            measured: false,
        };
        if let Some(sample) = initial {
            rtt.sample(sample);
        }
        rtt
    }

    pub(crate) fn sample(&mut self, sample: Duration) {
        let r = sample.as_secs_f64();
        if self.measured {
            self.deviation += ((self.mean - r).abs() - self.deviation) / 4.0;
            self.mean += (r - self.mean) / 8.0;
        } else {
            self.mean = r;
            self.deviation = r / 2.0;
            self.measured = true;
        }
    }

    pub(crate) fn mean(&self) -> Duration {
        Duration::from_secs_f64(self.mean.max(0.0))
    }

    pub(crate) fn deviation(&self) -> Duration {
        Duration::from_secs_f64(self.deviation.max(0.0))
    }

    pub(crate) fn measured(&self) -> bool {
        self.measured
    }
}

/// Delivered and lost packets judged over the last 5 seconds.
#[derive(Debug, Clone, Default)]
pub(crate) struct LossWindow {
    outcomes: VecDeque<(Duration, bool)>,
}

impl LossWindow {
    pub(crate) fn push(&mut self, now: Duration, lost: bool) {
        self.outcomes.push_back((now, lost));
        while self
            .outcomes
            .front()
            .is_some_and(|(at, _)| now.saturating_sub(*at) > RECENT_WINDOW)
        {
            self.outcomes.pop_front();
        }
    }

    /// Lost over lost plus delivered, or `None` with nothing judged.
    pub(crate) fn ratio(&self, now: Duration) -> Option<f64> {
        let (mut lost, mut total) = (0u32, 0u32);
        for (at, was_lost) in &self.outcomes {
            if now.saturating_sub(*at) <= RECENT_WINDOW {
                total += 1;
                lost += u32::from(*was_lost);
            }
        }
        (total > 0).then(|| f64::from(lost) / f64::from(total))
    }
}

/// Bytes and packets over the last second.
#[derive(Debug, Clone, Default)]
pub(crate) struct RateWindow {
    entries: VecDeque<(Duration, usize)>,
}

impl RateWindow {
    pub(crate) fn push(&mut self, now: Duration, bytes: usize) {
        self.entries.push_back((now, bytes));
        while self
            .entries
            .front()
            .is_some_and(|(at, _)| now.saturating_sub(*at) >= RATE_WINDOW)
        {
            self.entries.pop_front();
        }
    }

    /// (bytes, packets) in the second before `now`.
    pub(crate) fn per_second(&self, now: Duration) -> (u64, u32) {
        let mut bytes = 0u64;
        let mut packets = 0u32;
        for (at, size) in &self.entries {
            if now.saturating_sub(*at) < RATE_WINDOW {
                bytes += *size as u64;
                packets += 1;
            }
        }
        (bytes, packets)
    }
}

/// Arrival spread in the manner of RFC 3550's interarrival jitter: the mean of
/// how much the gap between two arrivals differs from the gap between their
/// send times, with gain 1/16.
#[derive(Debug, Clone, Default)]
pub(crate) struct Spread {
    last: Option<(Duration, Duration)>,
    jitter: f64,
}

impl Spread {
    pub(crate) fn note(&mut self, sent: Duration, received: Duration) {
        if let Some((last_sent, last_received)) = self.last {
            let d = (received.as_secs_f64() - last_received.as_secs_f64())
                - (sent.as_secs_f64() - last_sent.as_secs_f64());
            self.jitter += (d.abs() - self.jitter) / 16.0;
        }
        self.last = Some((sent, received));
    }

    pub(crate) fn value(&self) -> Duration {
        Duration::from_secs_f64(self.jitter.max(0.0))
    }
}

/// Times of bad packets over the last 5 seconds.
#[derive(Debug, Clone, Default)]
pub(crate) struct BadWindow {
    times: VecDeque<Duration>,
}

impl BadWindow {
    /// Adds one; returns how many fall in the window.
    pub(crate) fn push(&mut self, now: Duration) -> usize {
        self.times.push_back(now);
        while self
            .times
            .front()
            .is_some_and(|at| now.saturating_sub(*at) >= RECENT_WINDOW)
        {
            self.times.pop_front();
        }
        self.times.len()
    }

    pub(crate) fn recent(&self, now: Duration) -> usize {
        self.times
            .iter()
            .filter(|at| now.saturating_sub(**at) < RECENT_WINDOW)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn receive_window_marks_bits_and_duplicates() {
        let mut w = ReceiveWindow::default();
        assert_eq!(w.ack_fields(MS).2, ACK_DELAY_NONE);
        for seq in [65_534u16, 0, 65_535, 3] {
            assert_eq!(w.arrival(seq), Arrival::New);
            w.record(seq, MS * u32::from(seq % 100));
        }
        let (ack, bits, _) = w.ack_fields(MS * 10);
        assert_eq!(ack, 3);
        // 0 is 3 back (bit 2), 65535 is 4 back (bit 3), 65534 is 5 back (bit 4).
        assert_eq!(bits, 0b1_1100);
        assert_eq!(w.arrival(0), Arrival::Duplicate);
        assert_eq!(w.arrival(3), Arrival::Duplicate);
        assert_eq!(w.arrival(1), Arrival::New);
        assert_eq!(w.arrival(3u16.wrapping_sub(33)), Arrival::TooOld);
        assert_eq!(w.arrival(3u16.wrapping_sub(32)), Arrival::New);
        // 35 is 32 ahead of 3: only 3 stays in the bits, as bit 31.
        w.record(35, MS);
        assert_eq!(w.ack_fields(MS).1, 1 << 31);
        w.record(80, MS);
        assert_eq!(w.ack_fields(MS).1, 0);
    }

    #[test]
    fn ack_delay_is_in_16_microsecond_units_and_capped() {
        let mut w = ReceiveWindow::default();
        w.record(1, Duration::from_micros(100));
        assert_eq!(w.ack_fields(Duration::from_micros(100 + 160)).2, 10);
        assert_eq!(w.ack_fields(Duration::from_secs(5)).2, ACK_DELAY_MAX);
    }

    #[test]
    fn sent_log_delivers_loses_and_samples() {
        let mut log = SentLog::default();
        assert!(!log.ack_is_sane(0));
        for i in 0..40u32 {
            log.record(MS * i, vec![i as u16]);
        }
        assert!(log.ack_is_sane(39));
        assert!(!log.ack_is_sane(40));
        // Ack 5 with 3 and 1 in the bits (2 back and 4 back).
        let out = log.apply(MS * 50, 5, 0b1010, MS * 2);
        assert_eq!(out.sample, Some(MS * 43));
        let seqs: Vec<u16> = out.delivered.iter().map(|d| d.0).collect();
        assert_eq!(seqs, vec![5, 3, 1]);
        assert!(out.lost.is_empty());
        // Ack 39 alone: 0, 2 and 4 to 6 are more than 32 behind and lost.
        let out = log.apply(MS * 60, 39, 0, Duration::ZERO);
        assert_eq!(out.delivered.len(), 1);
        assert_eq!(out.lost, vec![0, 2, 4, 6]);
        // An older ack gives no new sample.
        assert!(log.apply(MS * 61, 30, 0, Duration::ZERO).sample.is_none());
    }

    #[test]
    fn round_trip_smooths_like_tcp() {
        let mut rtt = RoundTrip::new(None);
        assert!(!rtt.measured());
        rtt.sample(MS * 100);
        assert_eq!(rtt.mean(), MS * 100);
        rtt.sample(MS * 200);
        assert!((rtt.mean().as_secs_f64() - 0.1125).abs() < 1e-9);
        assert!((rtt.deviation().as_secs_f64() - 0.0625).abs() < 1e-9);
    }

    #[test]
    fn windows_forget_old_entries() {
        let mut loss = LossWindow::default();
        loss.push(Duration::ZERO, true);
        loss.push(Duration::from_secs(1), false);
        assert_eq!(loss.ratio(Duration::from_secs(2)), Some(0.5));
        assert_eq!(loss.ratio(Duration::from_secs(6)), Some(0.0));
        let mut rate = RateWindow::default();
        rate.push(Duration::ZERO, 100);
        rate.push(MS * 500, 50);
        assert_eq!(rate.per_second(MS * 900), (150, 2));
        assert_eq!(rate.per_second(MS * 1200), (50, 1));
        let mut bad = BadWindow::default();
        for i in 0..10 {
            bad.push(Duration::from_secs(i));
        }
        assert_eq!(bad.recent(Duration::from_secs(9)), 5);
    }

    #[test]
    fn spread_tracks_arrival_variation() {
        let mut s = Spread::default();
        for i in 0..400u32 {
            let sent = MS * (10 * i);
            let wobble = if i % 2 == 0 { MS * 5 } else { Duration::ZERO };
            s.note(sent, sent + MS * 50 + wobble);
        }
        // Every gap differs by 5 ms, so the spread converges on 5 ms.
        assert!((s.value().as_secs_f64() - 0.005).abs() < 1e-4);
    }
}
