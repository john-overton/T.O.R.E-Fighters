//! Relay channels, keys, rates, idle, the monthly allowance and its file
//! ("Relay" in the master protocol, "The relay's monthly allowance" in the
//! operations guide; slice J3).
//!
//! A player whose race found no path sends a **Relay request** naming its
//! introduction, from the address the introduction saw. The master opens a
//! channel with a fresh number and key, sends the host a **Relay open**
//! (again every 250 ms until a **Relay open ack** carrying the listing's
//! token, three times at most), and then offers the player the channel
//! (**Relay offer**), or tells it why not. From then on a **Relay** frame
//! from either end, with the channel's key, is forwarded unchanged to the
//! other end, within the channel's rate each way.
//!
//! The rules, with the master's settings ([`RelaySettings`]):
//!
//! - **Refusals,** in this order: the relay switched off; the month's
//!   allowance at 95 percent or more (John, 2026-10-05); 64 channels open
//!   (`relay-channels`); 2 from the player's source (`relay-channels-per-
//!   source`, an IPv4 address or an IPv6 /64); 30 to one listing.
//! - **Rate:** `relay-rate` KB/s each way per channel (1 KB is 1,000 bytes),
//!   with bursts of twice that; frames over it are dropped, not queued. A
//!   channel that goes over its rate in every second for 30 seconds is
//!   closed (agent decision: a game never comes near it, so that is a flood).
//! - **Idle:** no frame either way for 30 seconds closes a channel.
//! - **Allowance:** the bytes the master sends in Relay frames this calendar
//!   month (UTC), each with its IP and UDP headers (28 bytes over IPv4, 48
//!   over IPv6), as the provider counts transfer (agent decision). At 95
//!   percent of `relay-month-gb` new channels are refused; at 100 percent
//!   the open ones are closed. The run loop writes the figure to
//!   `relay-YYYY-MM.txt` every minute and reads it back at start
//!   ([`crate::log::StateFiles`]).
//! - **Closing** sends Relay close, three copies, to both ends when the
//!   master closes a channel (idle, over its rate, the allowance, stopping),
//!   and to the other end when one end closes it.
//!
//! - **Moving** (stage K, slice K8): when a listing moves to a new address
//!   (its token arrives from there, at most once a minute), every channel of
//!   the listing takes the new address as its host end, keeping its number
//!   and key ([`Relays::move_host`]), so a migrated game's relayed players
//!   reach its new host on the channels they already have.
//!
//! A Relay offer goes to a player whose address its introduction's cookie
//! proved, and a Relay open to a host whose listing proves its address, so
//! neither is fitted to a request or charged to the answer rate (agent
//! decision, as the Meets); Relay requests are limited to 2 a minute per
//! source before anything else is looked at ([`crate::limits`]).

use std::collections::hash_map::RandomState;
use std::collections::{HashMap, VecDeque};
use std::hash::BuildHasher;
use std::net::SocketAddr;
use std::time::Duration;

use tore_net::master::candidate::canonical;
use tore_net::master::relay::refusal_text;
use tore_net::master::{
    CloseReason, GOODBYE_COPIES, MEET_RETRY, MEET_TRIES, MasterPacket, RELAY_IDLE, RelayClose,
    RelayFrame, RelayOffer, RelayOpen, RelayOpenAck, RelayRequest, RelayResult,
};
use tore_net::{Entropy, SplitMix64};

use crate::limits::SourceKey;
use crate::listings::Listings;

/// Channels to one listing at most.
pub const CHANNELS_PER_LISTING: usize = 30;
/// A channel over its rate in every second for this long is closed.
pub const OVER_RATE_CLOSE: Duration = Duration::from_secs(30);
/// New channels are refused from this share of the month's allowance, in
/// percent (John, 2026-10-05).
pub const REFUSE_PERCENT: u64 = 95;
/// Bytes in a kilobyte, for `relay-rate`.
const KB: u64 = 1_000;
/// Bytes in a gigabyte, for `relay-month-gb`.
pub const GB: u64 = 1_000_000_000;

/// The relay's settings ("The configuration file" in the operations guide).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RelaySettings {
    /// Whether to relay at all.
    pub on: bool,
    /// Relayed pairs at once.
    pub channels: u32,
    /// Relayed pairs one player's address may have.
    pub channels_per_source: u32,
    /// Each channel's limit, each way, in KB/s.
    pub rate_kb: u32,
    /// Relayed gigabytes sent out each calendar month (UTC); new channels
    /// are refused at 95 percent of it and open ones closed at 100 (John,
    /// 2026-10-05).
    pub month_gb: u32,
}

impl Default for RelaySettings {
    fn default() -> Self {
        Self {
            on: true,
            channels: 64,
            channels_per_source: 2,
            rate_kb: 64,
            month_gb: 800,
        }
    }
}

impl RelaySettings {
    /// The month's allowance in bytes.
    pub fn allowance(&self) -> u64 {
        u64::from(self.month_gb) * GB
    }
}

/// The two ends of an introduction under way, which a Relay request names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ends {
    /// The listing the player was introduced to.
    pub listing_id: u64,
    /// The host's listed address.
    pub host: SocketAddr,
    /// The player's address as the master saw it.
    pub player: SocketAddr,
    /// The player's Introduce nonce.
    pub nonce: u64,
}

/// A calendar month, UTC: the year and the month (1 to 12).
pub type Month = (i64, u32);

/// A byte rate bucket: a generic cell rate algorithm over bytes, in integer
/// time, like [`crate::limits::Bucket`] over requests.
#[derive(Debug, Clone, Copy, Default)]
struct ByteBucket {
    /// When the bucket is empty again.
    full_until: Duration,
}

impl ByteBucket {
    /// Takes `bytes` at `now` at `rate` bytes a second with bursts of
    /// `burst` bytes: false when they are over it.
    fn take(&mut self, now: Duration, bytes: usize, rate: u64, burst: u64) -> bool {
        let nanos = |n: u64| Duration::from_nanos(n.saturating_mul(1_000_000_000) / rate.max(1));
        let cost = nanos(bytes as u64);
        let start = self.full_until.max(now);
        if start + cost - now > nanos(burst) {
            return false;
        }
        self.full_until = start + cost;
        true
    }
}

#[derive(Debug, Clone)]
struct Channel {
    key: u32,
    introduction_id: u64,
    nonce: u64,
    listing_id: u64,
    host: SocketAddr,
    player: SocketAddr,
    source: SourceKey,
    /// The host acknowledged it and the player has its offer.
    open: bool,
    opens_sent: u32,
    next_open: Duration,
    last_frame: Duration,
    to_host: ByteBucket,
    to_player: ByteBucket,
    /// Since when every second has had a frame over the rate.
    over_since: Option<Duration>,
    last_over: Duration,
    bytes_to_host: u64,
    bytes_to_player: u64,
}

/// Unpredictable numbers on a real network, repeatable for a seed in tests.
#[derive(Debug, Clone)]
enum Draw {
    System(u64),
    Seeded(SplitMix64),
}

impl Draw {
    fn new(entropy: Entropy) -> Self {
        match entropy {
            Entropy::System => Self::System(0),
            Entropy::Seeded(seed) => Self::Seeded(SplitMix64::new(seed ^ 0x7265_6c61_7973)),
        }
    }

    fn next(&mut self) -> u64 {
        match self {
            Self::System(counter) => {
                *counter = counter.wrapping_add(1);
                RandomState::new().hash_one(*counter)
            }
            Self::Seeded(rng) => rng.next_u64(),
        }
    }
}

/// What the relay counted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RelayCounters {
    /// Channels opened: the host acknowledged and the player was offered it.
    pub opened: u64,
    /// Relay requests refused with a Relay offer that says why.
    pub refused: u64,
    /// Channels closed, for any reason.
    pub closed: u64,
    /// Frames forwarded.
    pub frames: u64,
    /// Bytes forwarded (the frames' own bytes).
    pub bytes: u64,
    /// Frames dropped over a channel's rate.
    pub over_rate: u64,
    /// Channels that followed their listing to a new host address (stage K,
    /// slice K8).
    pub moved: u64,
}

/// The relay's channels and the month's figure.
#[derive(Debug, Clone)]
pub struct Relays {
    settings: RelaySettings,
    draw: Draw,
    channels: HashMap<u32, Channel>,
    /// The channel of each (player address, introduction).
    by_player: HashMap<(SocketAddr, u64), u32>,
    month: Option<Month>,
    month_bytes: u64,
    /// The 95 and 100 percent lines were logged this month.
    said_refusing: bool,
    said_spent: bool,
    out: VecDeque<(SocketAddr, Vec<u8>)>,
    log: VecDeque<String>,
    /// What it counted.
    pub counters: RelayCounters,
    /// Relay packets dropped: requests for no introduction under way, or
    /// from another address or with another nonce; acks and closes that name
    /// no channel or come from elsewhere; frames of no open channel, with
    /// another key, or from neither end.
    pub dropped: u64,
}

impl Relays {
    /// No channels, their numbers and keys drawn from `entropy`.
    pub fn new(settings: RelaySettings, entropy: Entropy) -> Self {
        Self {
            settings,
            draw: Draw::new(entropy),
            channels: HashMap::new(),
            by_player: HashMap::new(),
            month: None,
            month_bytes: 0,
            said_refusing: false,
            said_spent: false,
            out: VecDeque::new(),
            log: VecDeque::new(),
            counters: RelayCounters::default(),
            dropped: 0,
        }
    }

    /// The settings in force.
    pub fn settings(&self) -> &RelaySettings {
        &self.settings
    }

    /// Channels open or opening.
    pub fn channels(&self) -> usize {
        self.channels.len()
    }

    /// Bytes relayed out this calendar month, headers included.
    pub fn month_bytes(&self) -> u64 {
        self.month_bytes
    }

    /// The month the figure is for, once the run loop has said.
    pub fn month(&self) -> Option<Month> {
        self.month
    }

    /// Carries on with `bytes` already relayed in `month` (read back from
    /// its file at start).
    pub fn resume_month(&mut self, month: Month, bytes: u64) {
        self.month = Some(month);
        self.month_bytes = bytes;
        self.said_refusing = self.refusing();
        self.said_spent = self.spent();
    }

    /// The calendar month is `month` now. When it changed, the figure starts
    /// again at zero and the finished month's figure is returned, to write.
    pub fn roll_month(&mut self, month: Month) -> Option<(Month, u64)> {
        match self.month {
            Some(current) if current == month => None,
            Some(current) => {
                let finished = (current, self.month_bytes);
                self.resume_month(month, 0);
                Some(finished)
            }
            None => {
                self.month = Some(month);
                None
            }
        }
    }

    /// The share of the allowance that refuses new channels is reached.
    fn refusing(&self) -> bool {
        let allowance = self.settings.allowance();
        allowance == 0 || self.month_bytes.saturating_mul(100) >= allowance * REFUSE_PERCENT
    }

    /// The whole allowance is spent.
    fn spent(&self) -> bool {
        self.month_bytes >= self.settings.allowance()
    }

    /// The next datagram to send from the main port, and where to.
    pub fn poll_send(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        self.out.pop_front()
    }

    /// The next line for the log: channels opened and closed, the
    /// allowance's lines.
    pub fn poll_log(&mut self) -> Option<String> {
        self.log.pop_front()
    }

    fn send(&mut self, to: SocketAddr, packet: &MasterPacket) {
        if let Ok(datagram) = packet.encode() {
            self.out.push_back((to, datagram));
        }
    }

    fn offer(&mut self, to: SocketAddr, offer: RelayOffer) {
        self.send(to, &MasterPacket::RelayOffer(offer));
    }

    fn refuse(&mut self, to: SocketAddr, request: &RelayRequest, result: RelayResult) {
        self.counters.refused += 1;
        self.log.push_back(format!(
            "relay refused player={to} result={}",
            result_name(result)
        ));
        self.offer(
            to,
            RelayOffer {
                nonce: request.nonce,
                introduction_id: request.introduction_id,
                result,
                channel: 0,
                key: 0,
                text: refusal_text(result).to_owned(),
            },
        );
    }

    /// A player's Relay request from `from`, past its source's limit;
    /// `ends` is the introduction it names, if one is under way.
    pub fn request(
        &mut self,
        now: Duration,
        from: SocketAddr,
        request: &RelayRequest,
        ends: Option<Ends>,
    ) {
        let from = canonical(from);
        let Some(ends) = ends.filter(|e| e.player == from && e.nonce == request.nonce) else {
            self.dropped += 1;
            return;
        };
        // The same request again: the offer again once the channel is open;
        // while it opens, the offer is still to come.
        if let Some(&number) = self.by_player.get(&(from, request.introduction_id)) {
            if let Some(channel) = self.channels.get(&number).filter(|c| c.open) {
                let offer = RelayOffer {
                    nonce: channel.nonce,
                    introduction_id: channel.introduction_id,
                    result: RelayResult::Open,
                    channel: number,
                    key: channel.key,
                    text: String::new(),
                };
                self.offer(from, offer);
            }
            return;
        }
        let source = SourceKey::of(from);
        let refusal = if !self.settings.on {
            Some(RelayResult::Off)
        } else if self.refusing() {
            Some(RelayResult::AllowanceSpent)
        } else if self.channels.len() >= self.settings.channels as usize {
            Some(RelayResult::Full)
        } else if self
            .channels
            .values()
            .filter(|c| c.source == source)
            .count()
            >= self.settings.channels_per_source as usize
        {
            Some(RelayResult::TooMany)
        } else if self
            .channels
            .values()
            .filter(|c| c.listing_id == ends.listing_id)
            .count()
            >= CHANNELS_PER_LISTING
        {
            Some(RelayResult::Full)
        } else {
            None
        };
        if let Some(result) = refusal {
            self.refuse(from, request, result);
            return;
        }
        let number = self.fresh_channel();
        let key = self.draw.next() as u32;
        self.channels.insert(
            number,
            Channel {
                key,
                introduction_id: request.introduction_id,
                nonce: request.nonce,
                listing_id: ends.listing_id,
                host: canonical(ends.host),
                player: from,
                source,
                open: false,
                opens_sent: 0,
                next_open: now,
                last_frame: now,
                to_host: ByteBucket::default(),
                to_player: ByteBucket::default(),
                over_since: None,
                last_over: now,
                bytes_to_host: 0,
                bytes_to_player: 0,
            },
        );
        self.by_player
            .insert((from, request.introduction_id), number);
        self.send_opens(now);
    }

    /// A channel number no channel uses.
    fn fresh_channel(&mut self) -> u32 {
        loop {
            let n = self.draw.next() as u32;
            if !self.channels.contains_key(&n) {
                return n;
            }
        }
    }

    /// Relay opens due: the first at once, then every 250 ms until acked,
    /// three in all; a channel the host never acknowledged is refused to the
    /// player 250 ms after the last.
    fn send_opens(&mut self, now: Duration) {
        let mut due = Vec::new();
        let mut silent = Vec::new();
        for (&number, channel) in &mut self.channels {
            if channel.open || now < channel.next_open {
                continue;
            }
            if channel.opens_sent >= MEET_TRIES {
                silent.push(number);
                continue;
            }
            channel.opens_sent += 1;
            channel.next_open = now + MEET_RETRY;
            due.push((
                channel.host,
                RelayOpen {
                    introduction_id: channel.introduction_id,
                    channel: number,
                    key: channel.key,
                    player: channel.player,
                },
            ));
        }
        due.sort_unstable_by_key(|(_, open)| open.channel);
        for (host, open) in due {
            self.send(host, &MasterPacket::RelayOpen(open));
        }
        silent.sort_unstable();
        for number in silent {
            if let Some(channel) = self.forget(number) {
                let request = RelayRequest {
                    nonce: channel.nonce,
                    introduction_id: channel.introduction_id,
                };
                self.refuse(channel.player, &request, RelayResult::HostSilent);
            }
        }
    }

    fn forget(&mut self, number: u32) -> Option<Channel> {
        let channel = self.channels.remove(&number)?;
        self.by_player
            .remove(&(channel.player, channel.introduction_id));
        Some(channel)
    }

    /// A host's Relay open ack from `from`: the channel opens when the token
    /// is its listing's and the ack comes from the host's address, and the
    /// player is offered it.
    pub fn open_ack(
        &mut self,
        now: Duration,
        from: SocketAddr,
        ack: &RelayOpenAck,
        listings: &Listings,
    ) {
        let from = canonical(from);
        let Some(channel) = self.channels.get_mut(&ack.channel) else {
            self.dropped += 1;
            return;
        };
        let token_ok = listings
            .get(channel.listing_id)
            .is_some_and(|l| l.token == ack.token);
        if !token_ok || from != channel.host {
            self.dropped += 1;
            return;
        }
        if channel.open {
            return;
        }
        channel.open = true;
        channel.last_frame = now;
        let (player, offer) = (
            channel.player,
            RelayOffer {
                nonce: channel.nonce,
                introduction_id: channel.introduction_id,
                result: RelayResult::Open,
                channel: ack.channel,
                key: channel.key,
                text: String::new(),
            },
        );
        let host = channel.host;
        self.counters.opened += 1;
        self.offer(player, offer);
        self.log.push_back(format!(
            "relay opened channel={:08x} host={host} player={player} channels={}",
            ack.channel,
            self.channels.len()
        ));
    }

    /// A Relay frame from `from`, read in place; `bytes` is the whole frame,
    /// forwarded unchanged to the channel's other end.
    pub fn frame(&mut self, now: Duration, from: SocketAddr, frame: &RelayFrame<'_>, bytes: &[u8]) {
        let from = canonical(from);
        let rate = u64::from(self.settings.rate_kb) * KB;
        let spent = self.spent();
        let Some(channel) = self
            .channels
            .get_mut(&frame.channel)
            .filter(|c| c.open && c.key == frame.key)
        else {
            self.dropped += 1;
            return;
        };
        let (to, bucket) = if from == channel.host {
            (channel.player, &mut channel.to_player)
        } else if from == channel.player {
            (channel.host, &mut channel.to_host)
        } else {
            self.dropped += 1;
            return;
        };
        if spent {
            self.dropped += 1;
            return;
        }
        if !bucket.take(now, bytes.len(), rate, rate * 2) {
            self.counters.over_rate += 1;
            let carried_on = channel.over_since.is_some()
                && now.saturating_sub(channel.last_over) <= Duration::from_secs(1);
            if !carried_on {
                channel.over_since = Some(now);
            }
            channel.last_over = now;
            return;
        }
        channel.last_frame = now;
        if to == channel.host {
            channel.bytes_to_host += bytes.len() as u64;
        } else {
            channel.bytes_to_player += bytes.len() as u64;
        }
        let header = if to.is_ipv4() { 28 } else { 48 };
        self.month_bytes += (bytes.len() + header) as u64;
        self.counters.frames += 1;
        self.counters.bytes += bytes.len() as u64;
        self.out.push_back((to, bytes.to_vec()));
    }

    /// The listing `listing_id` moved to `to` (stage K, slice K8: a game that
    /// took a migrated mission over heartbeats with the listing's token from
    /// its own game port, or the host's router gave the port a new outside
    /// address). Every channel of the listing, open or still opening, takes
    /// `to` as its host end and keeps its number and key, so its relayed
    /// player's frames reach the new host and nothing more goes to the old
    /// address; a Relay open still due goes to `to`. Returns how many moved,
    /// and logs a line when any did.
    pub fn move_host(&mut self, listing_id: u64, to: SocketAddr) -> usize {
        let to = canonical(to);
        let mut moved: Vec<(u32, SocketAddr)> = Vec::new();
        for (&number, channel) in &mut self.channels {
            if channel.listing_id == listing_id && channel.host != to {
                moved.push((number, channel.host));
                channel.host = to;
            }
        }
        if let Some(&(_, from)) = moved.first() {
            self.counters.moved += moved.len() as u64;
            self.log.push_back(format!(
                "relay moved listing={listing_id:016x} from={from} to={to} channels={}",
                moved.len()
            ));
        }
        moved.len()
    }

    /// A Relay close from an end: the channel goes, and the other end is
    /// told.
    pub fn close(&mut self, now: Duration, from: SocketAddr, close: &RelayClose) {
        let _ = now;
        let from = canonical(from);
        let ok = self
            .channels
            .get(&close.channel)
            .is_some_and(|c| c.key == close.key && (from == c.host || from == c.player));
        if !ok {
            self.dropped += 1;
            return;
        }
        let Some(channel) = self.forget(close.channel) else {
            return;
        };
        let other = if from == channel.host {
            channel.player
        } else {
            channel.host
        };
        if channel.open {
            self.say_goodbye(other, close.channel, channel.key, CloseReason::Closed);
        }
        self.closed(close.channel, &channel, "closed by an end");
    }

    fn say_goodbye(&mut self, to: SocketAddr, channel: u32, key: u32, reason: CloseReason) {
        for _ in 0..GOODBYE_COPIES {
            self.send(
                to,
                &MasterPacket::RelayClose(RelayClose {
                    channel,
                    key,
                    reason,
                }),
            );
        }
    }

    fn closed(&mut self, number: u32, channel: &Channel, why: &str) {
        self.counters.closed += 1;
        if !channel.open {
            return;
        }
        self.log.push_back(format!(
            "relay closed channel={number:08x} host={} player={} reason={why} to-host={} to-player={} channels={}",
            channel.host,
            channel.player,
            channel.bytes_to_host,
            channel.bytes_to_player,
            self.channels.len()
        ));
    }

    /// The master closes a channel itself: both ends are told.
    fn shut(&mut self, number: u32, reason: CloseReason, why: &str) {
        if let Some(channel) = self.forget(number) {
            if channel.open {
                self.say_goodbye(channel.host, number, channel.key, reason);
                self.say_goodbye(channel.player, number, channel.key, reason);
            }
            self.closed(number, &channel, why);
        }
    }

    /// Timers: Relay opens due again, channels idle or over their rate, and
    /// the allowance.
    pub fn update(&mut self, now: Duration) {
        self.send_opens(now);
        if self.settings.allowance() == 0 {
            // No allowance: every channel is refused, which the start lines
            // say; nothing to log.
            self.said_refusing = true;
            self.said_spent = true;
        }
        if !self.said_refusing && self.refusing() {
            self.said_refusing = true;
            self.log.push_back(format!(
                "relay allowance: {REFUSE_PERCENT} percent of {} GB relayed this month, so new channels are refused",
                self.settings.month_gb
            ));
        }
        let spent = self.spent();
        if !self.said_spent && spent {
            self.said_spent = true;
            self.log.push_back(format!(
                "relay allowance: all {} GB relayed this month, so the open channels are closed",
                self.settings.month_gb
            ));
        }
        let mut shut: Vec<(u32, CloseReason, &str)> = self
            .channels
            .iter()
            .filter(|(_, c)| c.open)
            .filter_map(|(&n, c)| {
                if spent {
                    Some((n, CloseReason::AllowanceSpent, "allowance spent"))
                } else if now.saturating_sub(c.last_frame) >= RELAY_IDLE {
                    Some((n, CloseReason::Idle, "idle"))
                } else if c.over_since.is_some_and(|since| {
                    c.last_over.saturating_sub(since) >= OVER_RATE_CLOSE
                        && now.saturating_sub(c.last_over) <= Duration::from_secs(1)
                }) {
                    Some((n, CloseReason::OverRate, "over its rate"))
                } else {
                    None
                }
            })
            .collect();
        shut.sort_unstable_by_key(|(n, _, _)| *n);
        for (n, reason, why) in shut {
            self.shut(n, reason, why);
        }
    }

    /// The master is stopping: every channel closes, both ends told.
    pub fn stop(&mut self) {
        let mut numbers: Vec<u32> = self.channels.keys().copied().collect();
        numbers.sort_unstable();
        for n in numbers {
            self.shut(n, CloseReason::Stopping, "the master is stopping");
        }
    }
}

/// A Relay offer's result in the log.
fn result_name(result: RelayResult) -> &'static str {
    match result {
        RelayResult::Open => "open",
        RelayResult::HostSilent => "host-silent",
        RelayResult::Full => "full",
        RelayResult::AllowanceSpent => "allowance-spent",
        RelayResult::Off => "off",
        RelayResult::TooMany => "too-many",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_byte_bucket_passes_its_rate_after_its_burst() {
        let mut bucket = ByteBucket::default();
        let rate = 64_000;
        let start = Duration::from_secs(10);
        // 200 KB/s offered in 1,000-byte frames, for 10 seconds.
        let mut passed = 0u64;
        let mut passed_after_3s = 0u64;
        for i in 0..2_000u64 {
            let now = start + Duration::from_micros(i * 5_000);
            if bucket.take(now, 1_000, rate, rate * 2) {
                passed += 1_000;
                if now >= start + Duration::from_secs(3) {
                    passed_after_3s += 1_000;
                }
            }
        }
        // The burst, then the rate.
        assert!(
            (128_000 + 640_000 - 1_000..=128_000 + 640_000 + 1_000).contains(&passed),
            "{passed}"
        );
        let steady = passed_after_3s as f64 / 7.0;
        assert!((63_000.0..=65_000.0).contains(&steady), "{steady}");
    }

    /// Slice K8: a channel still opening follows its listing too. Its next
    /// Relay open goes to the new address, an ack from the old one no longer
    /// counts, and one from the new one opens it.
    #[test]
    fn a_channel_still_opening_follows_its_listing_to_a_new_host() {
        use crate::limits::Bucket;
        use crate::listings::Listing;
        use tore_net::master::{Build, ListingSummary, MappingType};

        let a = |text: &str| text.parse::<SocketAddr>().unwrap();
        let (old, new, player) = (
            a("203.0.113.10:26900"),
            a("203.0.113.30:26900"),
            a("192.0.2.50:40000"),
        );
        let expiry = Duration::from_secs(90);
        let mut listings = Listings::default();
        listings.insert(
            Listing {
                id: 7,
                token: 77,
                nonce: 1,
                address: old,
                source: SourceKey::of(old),
                build: Build::default(),
                dedicated: false,
                platform: 3,
                candidates: Vec::new(),
                summary: ListingSummary::default(),
                change: 0,
                mapping: MappingType::Unknown,
                heard: Duration::ZERO,
                moved: None,
                beats: Bucket::default(),
            },
            expiry,
        );
        let mut relays = Relays::new(RelaySettings::default(), Entropy::Seeded(1));
        let request = RelayRequest {
            nonce: 5,
            introduction_id: 9,
        };
        let ends = Ends {
            listing_id: 7,
            host: old,
            player,
            nonce: 5,
        };
        relays.request(Duration::ZERO, player, &request, Some(ends));
        let open = relays.poll_send().expect("the first Relay open");
        assert_eq!(open.0, old);
        let MasterPacket::RelayOpen(RelayOpen { channel, .. }) =
            MasterPacket::decode(&open.1).unwrap()
        else {
            panic!("not a Relay open");
        };
        // Another listing's move leaves it; its own takes it along.
        assert_eq!(relays.move_host(8, new), 0);
        assert_eq!(relays.move_host(7, new), 1);
        assert_eq!(relays.move_host(7, new), 0, "already there");
        assert_eq!(relays.counters.moved, 1);
        assert!(
            relays
                .poll_log()
                .is_some_and(|l| l.starts_with("relay moved listing=0000000000000007")
                    && l.ends_with("from=203.0.113.10:26900 to=203.0.113.30:26900 channels=1"))
        );
        listings.update(7, expiry, |l| l.address = new);
        relays.update(MEET_RETRY);
        assert_eq!(relays.poll_send().map(|(to, _)| to), Some(new));
        let ack = RelayOpenAck { token: 77, channel };
        let dropped = relays.dropped;
        relays.open_ack(MEET_RETRY, old, &ack, &listings);
        assert_eq!(relays.dropped, dropped + 1, "the old address's ack");
        relays.open_ack(MEET_RETRY, new, &ack, &listings);
        assert_eq!(relays.counters.opened, 1);
        assert_eq!(relays.poll_send().map(|(to, _)| to), Some(player));
    }

    #[test]
    fn the_allowance_refuses_at_95_percent_and_is_spent_at_100() {
        let mut relays = Relays::new(
            RelaySettings {
                month_gb: 10,
                ..RelaySettings::default()
            },
            Entropy::Seeded(1),
        );
        relays.resume_month((2026, 10), 9 * GB);
        assert!(!relays.refusing());
        relays.resume_month((2026, 10), 9_500_000_000);
        assert!(relays.refusing() && !relays.spent());
        relays.resume_month((2026, 10), 10 * GB);
        assert!(relays.spent());
        // A new month starts at zero and hands back the old figure.
        assert_eq!(relays.roll_month((2026, 10)), None);
        assert_eq!(relays.roll_month((2026, 11)), Some(((2026, 10), 10 * GB)));
        assert_eq!(relays.month_bytes(), 0);
        assert!(!relays.refusing());
        // No allowance at all refuses everything.
        let none = Relays::new(
            RelaySettings {
                month_gb: 0,
                ..RelaySettings::default()
            },
            Entropy::Seeded(1),
        );
        assert!(none.refusing());
    }
}
