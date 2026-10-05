//! Introductions, Meets and their retries, hints ("Introductions" in the
//! master protocol; slice J2).
//!
//! A player who chose a listed game sends Introduce from the socket it will
//! join with. Once its address is proven (the cookie, checked by the
//! [`crate::master::Master`] before it gets here), the master tells the host
//! where the player is (a **Meet**, from the main port) and the player where
//! the host is (the **Introduction**), at the same moment, so both start
//! sending to each other: the player's Connect requests and the host's
//! punches open each router for the other.
//!
//! - The Introduction lists the host's addresses seen first (the address the
//!   master hears the listing from), then Mapped, Global IPv6 and Local; the
//!   Meet lists the player's seen first, then Global IPv6 and Local.
//! - The hint says to ask for the relay at once when both ends' routers map
//!   a port per destination and the host has no Mapped or Global IPv6
//!   address ([`relay_likely`]).
//! - A Meet is sent again every 250 ms until the host acknowledges it with
//!   its listing's token, three times at most. An introduction is forgotten
//!   30 seconds after it was made.
//! - The same Introduce again (the same address and nonce: the Introduction
//!   was lost) gets the same Introduction, and no second Meet.
//!
//! Limits ("Limits" in the master protocol): 4 a second per source before
//! the cookie is even checked (the master's own [`crate::limits`]); then 30
//! a minute per source, over which the player is told it has too many
//! introductions under way (result 4); and 10 a second per listing, over
//! which the Introduce is dropped. At most [`MAX_INTRODUCTIONS`] are under
//! way at once (agent decision: a few thousand, as the design says), the
//! rest dropped.

use std::collections::{HashMap, VecDeque};
use std::net::SocketAddr;
use std::time::Duration;

use tore_net::master::candidate::canonical;
use tore_net::master::join::refusal_text;
use tore_net::master::{
    Candidate, CandidateKind, Hint, INTRODUCTION_LIFETIME, Introduce, Introduction,
    IntroductionResult, MAX_CANDIDATES, MEET_RETRY, MEET_TRIES, MappingType, MasterPacket, Meet,
    MeetAck, relay_likely,
};
use tore_net::{Entropy, SplitMix64};

use crate::browse::same_build;
use crate::limits::{Bucket, Rate, SourceKey};
use crate::listings::Listings;

/// Introduce per source, after the cookie: 30 a minute.
pub const PER_SOURCE: Rate = Rate::per_minute(30, 30);
/// Introductions per listing: 10 a second.
pub const PER_LISTING: Rate = Rate::per_second(10, 10);
/// Introductions under way at most.
pub const MAX_INTRODUCTIONS: usize = 4_096;
/// A bucket unused this long is forgotten (it is empty again by then).
const BUCKET_IDLE: Duration = Duration::from_secs(60);

/// One introduction under way.
#[derive(Debug, Clone)]
struct Intro {
    listing_id: u64,
    host: SocketAddr,
    player: SocketAddr,
    nonce: u64,
    made: Duration,
    /// The Introduction the player got, for an Introduce repeated.
    answer: Introduction,
    /// The Meet, encoded once.
    meet: Vec<u8>,
    meets_sent: u32,
    next_meet: Duration,
    acked: bool,
}

/// Introductions under way, and what they counted.
#[derive(Debug, Clone)]
pub struct Introductions {
    ids: SplitMix64,
    system: bool,
    by_id: HashMap<u64, Intro>,
    by_player: HashMap<(SocketAddr, u64), u64>,
    /// Ids in the order made: an introduction's life is fixed, so the oldest
    /// is forgotten first.
    made: VecDeque<u64>,
    per_source: HashMap<SourceKey, (Bucket, Duration)>,
    per_listing: HashMap<u64, (Bucket, Duration)>,
    out: VecDeque<(SocketAddr, Vec<u8>)>,
    /// Introductions made (the status line's `introductions/min`).
    pub introduced: u64,
    /// Introductions refused: no such listing, another build, full, too
    /// many.
    pub refused: u64,
    /// Introduce repeated for an introduction made: answered the same.
    pub repeated: u64,
    /// Introduce dropped: over a listing's rate, or the table full.
    pub dropped: u64,
    /// Meets sent, retries included.
    pub meets: u64,
    /// Meet acks taken.
    pub meet_acks: u64,
    /// Meet acks for no introduction under way, or with a token not the
    /// listing's.
    pub bad_meet_acks: u64,
}

impl Introductions {
    /// No introductions, their ids drawn from `entropy`.
    pub fn new(entropy: Entropy) -> Self {
        let (seed, system) = match entropy {
            Entropy::Seeded(seed) => (seed ^ 0x696e_7472_6f64_7563, false),
            Entropy::System => {
                use std::hash::BuildHasher;
                let seed = std::collections::hash_map::RandomState::new().hash_one(0x696e_u32);
                (seed, true)
            }
        };
        Self {
            ids: SplitMix64::new(seed),
            system,
            by_id: HashMap::new(),
            by_player: HashMap::new(),
            made: VecDeque::new(),
            per_source: HashMap::new(),
            per_listing: HashMap::new(),
            out: VecDeque::new(),
            introduced: 0,
            refused: 0,
            repeated: 0,
            dropped: 0,
            meets: 0,
            meet_acks: 0,
            bad_meet_acks: 0,
        }
    }

    /// An Introduce from `from`, whose address the cookie has proven. The
    /// Introduction for the player, or `None` when the request is dropped.
    /// A Meet for the host is queued ([`Introductions::poll_send`]).
    pub fn introduce(
        &mut self,
        now: Duration,
        from: SocketAddr,
        request: &Introduce,
        listings: &Listings,
    ) -> Option<Introduction> {
        let from = canonical(from);
        if let Some(intro) = self
            .by_player
            .get(&(from, request.nonce))
            .and_then(|id| self.by_id.get(id))
        {
            self.repeated += 1;
            return Some(intro.answer.clone());
        }
        let source = SourceKey::of(from);
        if !take(&mut self.per_source, source, now, PER_SOURCE) {
            return Some(self.refusal(from, request, IntroductionResult::TooMany));
        }
        let Some(listing) = listings.get(request.listing_id) else {
            return Some(self.refusal(from, request, IntroductionResult::NoListing));
        };
        if !same_build(&listing.build, &request.build) {
            return Some(self.refusal(from, request, IntroductionResult::OtherBuild));
        }
        if listing.summary.full {
            return Some(self.refusal(from, request, IntroductionResult::Full));
        }
        if !take(&mut self.per_listing, listing.id, now, PER_LISTING)
            || self.by_id.len() >= MAX_INTRODUCTIONS
        {
            self.dropped += 1;
            return None;
        }
        let id = self.fresh_id();
        let hint = if request.mapping == MappingType::PortPerDestination
            && relay_likely(listing.mapping, &listing.candidates)
        {
            Hint::RelayNow
        } else {
            Hint::Race
        };
        let answer = Introduction {
            nonce: request.nonce,
            result: IntroductionResult::Introduced,
            introduction_id: id,
            hint,
            seen: from,
            host_mapping: listing.mapping,
            host_candidates: ordered(
                listing.address,
                &listing.candidates,
                &[
                    CandidateKind::Mapped,
                    CandidateKind::GlobalIpv6,
                    CandidateKind::Local,
                ],
            ),
            text: String::new(),
        };
        let meet = MasterPacket::Meet(Meet {
            introduction_id: id,
            mapping: request.mapping,
            candidates: ordered(
                from,
                &request.candidates,
                &[CandidateKind::GlobalIpv6, CandidateKind::Local],
            ),
        })
        .encode()
        .ok()?;
        self.out.push_back((listing.address, meet.clone()));
        self.meets += 1;
        self.introduced += 1;
        self.by_player.insert((from, request.nonce), id);
        self.made.push_back(id);
        self.by_id.insert(
            id,
            Intro {
                listing_id: listing.id,
                host: listing.address,
                player: from,
                nonce: request.nonce,
                made: now,
                answer: answer.clone(),
                meet,
                meets_sent: 1,
                next_meet: now + MEET_RETRY,
                acked: false,
            },
        );
        Some(answer)
    }

    fn refusal(
        &mut self,
        from: SocketAddr,
        request: &Introduce,
        result: IntroductionResult,
    ) -> Introduction {
        self.refused += 1;
        Introduction {
            nonce: request.nonce,
            result,
            introduction_id: 0,
            hint: Hint::Race,
            seen: from,
            host_mapping: MappingType::Unknown,
            host_candidates: Vec::new(),
            text: refusal_text(result).to_owned(),
        }
    }

    /// A number no introduction under way uses, never 0. Unpredictable on a
    /// real network: a player's relay request names it.
    fn fresh_id(&mut self) -> u64 {
        loop {
            let mut id = self.ids.next_u64();
            if self.system {
                use std::hash::BuildHasher;
                // Each id hashed with a fresh random key, as listing ids are.
                id = std::collections::hash_map::RandomState::new().hash_one(id);
            }
            if id != 0 && !self.by_id.contains_key(&id) {
                return id;
            }
        }
    }

    /// A host's Meet ack from `from`: it ends the Meet's retries when its
    /// token is the listing's.
    pub fn meet_ack(
        &mut self,
        now: Duration,
        from: SocketAddr,
        ack: &MeetAck,
        listings: &Listings,
    ) {
        let _ = (now, from);
        let Some(intro) = self.by_id.get_mut(&ack.introduction_id) else {
            self.bad_meet_acks += 1;
            return;
        };
        if listings.id_of_token(ack.token) != Some(intro.listing_id) {
            self.bad_meet_acks += 1;
            return;
        }
        intro.acked = true;
        self.meet_acks += 1;
    }

    /// Meets due again, introductions to forget, idle buckets.
    pub fn update(&mut self, now: Duration) {
        while let Some(&id) = self.made.front() {
            match self.by_id.get(&id) {
                Some(intro) if now.saturating_sub(intro.made) < INTRODUCTION_LIFETIME => break,
                Some(intro) => {
                    self.by_player.remove(&(intro.player, intro.nonce));
                    self.by_id.remove(&id);
                    self.made.pop_front();
                }
                None => {
                    self.made.pop_front();
                }
            }
        }
        for intro in self.by_id.values_mut() {
            if !intro.acked && intro.meets_sent < MEET_TRIES && now >= intro.next_meet {
                intro.meets_sent += 1;
                intro.next_meet = now + MEET_RETRY;
                self.out.push_back((intro.host, intro.meet.clone()));
                self.meets += 1;
            }
        }
        self.per_source
            .retain(|_, (_, used)| now.saturating_sub(*used) < BUCKET_IDLE);
        self.per_listing
            .retain(|_, (_, used)| now.saturating_sub(*used) < BUCKET_IDLE);
    }

    /// The next Meet to send from the main port, and where to.
    pub fn poll_send(&mut self) -> Option<(SocketAddr, Vec<u8>)> {
        self.out.pop_front()
    }

    /// Introductions under way.
    pub fn under_way(&self) -> usize {
        self.by_id.len()
    }
}

/// Takes one from the bucket of `key`, made when first used.
fn take<K: std::hash::Hash + Eq>(
    buckets: &mut HashMap<K, (Bucket, Duration)>,
    key: K,
    now: Duration,
    rate: Rate,
) -> bool {
    let (bucket, used) = buckets.entry(key).or_default();
    *used = now;
    bucket.take(now, rate)
}

/// `seen` as a Seen candidate, then `candidates` of each of `kinds` in that
/// order, at most [`MAX_CANDIDATES`] in all.
fn ordered(seen: SocketAddr, candidates: &[Candidate], kinds: &[CandidateKind]) -> Vec<Candidate> {
    let mut list = vec![Candidate::new(CandidateKind::Seen, canonical(seen))];
    for kind in kinds {
        list.extend(candidates.iter().filter(|c| c.kind == *kind).copied());
    }
    list.truncate(MAX_CANDIDATES);
    list
}
