//! The listed games: by id, by token and by address, with their summaries,
//! sources and expiry ("Listing a game" in the master protocol).
//!
//! A listing is made by a Register whose cookie is good, kept alive by
//! Heartbeats and Keeps that carry its token, and removed by an Unregister,
//! by a new Register from its address with another nonce, or when nothing has
//! been heard from it for the expiry (90 seconds by default). Listings live in
//! memory only: a master that restarts forgets them, and every host lists
//! itself again within one heartbeat.

use std::collections::{BTreeSet, HashMap};
use std::net::SocketAddr;
use std::time::Duration;

use tore_net::master::{Build, Candidate, ListingSummary, MappingType};

use crate::limits::{Bucket, Rate, SourceKey};

/// Heartbeats and Keeps together, per listing: one every 2 seconds, with
/// bursts of 2 so a Heartbeat and a Keep that fall due together both count
/// (agent decision).
pub const BEAT_RATE: Rate = Rate {
    count: 1,
    per: Duration::from_secs(2),
    burst: 2,
};

/// A listing moves to a new address (its router gave the game port another
/// outside address) at most this often.
pub const MOVE_INTERVAL: Duration = Duration::from_secs(60);

/// One listed game.
#[derive(Debug, Clone)]
pub struct Listing {
    /// Public: what browsers name it by.
    pub id: u64,
    /// Secret: what the host proves itself with.
    pub token: u64,
    /// The Register's nonce.
    pub nonce: u64,
    /// The address the master hears it from: the host's game port as its
    /// router maps it.
    pub address: SocketAddr,
    /// The source it counts against.
    pub source: SourceKey,
    /// The host's build.
    pub build: Build,
    /// A dedicated server.
    pub dedicated: bool,
    /// The host's platform code, raw.
    pub platform: u8,
    /// Local, Mapped and Global IPv6, as the host knows them.
    pub candidates: Vec<Candidate>,
    /// The lobby's summary.
    pub summary: ListingSummary,
    /// The Heartbeat's change counter.
    pub change: u16,
    /// How the host's router maps the game port, from its mapping test.
    pub mapping: MappingType,
    /// When anything was last heard from it.
    pub heard: Duration,
    /// When it last moved to a new address.
    pub moved: Option<Duration>,
    /// Heartbeats and Keeps.
    pub beats: Bucket,
}

/// Why a listing went.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gone {
    /// Its host sent Unregister.
    Unregistered,
    /// Nothing heard for the expiry.
    Expired,
    /// A new Register from its address with another nonce (the game restarted
    /// on the same port).
    Replaced,
    /// Another listing moved to its address.
    Displaced,
}

impl Gone {
    /// The reason as the log says it.
    pub fn text(self) -> &'static str {
        match self {
            Self::Unregistered => "unregistered",
            Self::Expired => "expired",
            Self::Replaced => "replaced by a new registration",
            Self::Displaced => "another listing moved to its address",
        }
    }
}

/// Every listing, and the indexes to reach them.
#[derive(Debug, Clone, Default)]
pub struct Listings {
    by_id: HashMap<u64, Listing>,
    by_token: HashMap<u64, u64>,
    by_address: HashMap<SocketAddr, u64>,
    per_source: HashMap<SourceKey, u32>,
    /// (when it expires, id).
    deadlines: BTreeSet<(Duration, u64)>,
}

impl Listings {
    /// How many there are.
    pub fn len(&self) -> usize {
        self.by_id.len()
    }

    /// True when there is none.
    pub fn is_empty(&self) -> bool {
        self.by_id.is_empty()
    }

    /// How many listings a source has.
    pub fn of_source(&self, source: SourceKey) -> u32 {
        self.per_source.get(&source).copied().unwrap_or(0)
    }

    /// The listing with this id.
    pub fn get(&self, id: u64) -> Option<&Listing> {
        self.by_id.get(&id)
    }

    /// The id of the listing with this token.
    pub fn id_of_token(&self, token: u64) -> Option<u64> {
        self.by_token.get(&token).copied()
    }

    /// The listing heard from this address.
    pub fn at_address(&self, address: SocketAddr) -> Option<&Listing> {
        self.by_address
            .get(&address)
            .and_then(|id| self.by_id.get(id))
    }

    /// True when an id or a token is taken (the two never share a value).
    pub fn number_taken(&self, n: u64) -> bool {
        n == 0 || self.by_id.contains_key(&n) || self.by_token.contains_key(&n)
    }

    /// Every listing, in no order.
    pub fn iter(&self) -> impl Iterator<Item = &Listing> {
        self.by_id.values()
    }

    /// Adds a listing; it expires `expiry` after `listing.heard`.
    pub fn insert(&mut self, listing: Listing, expiry: Duration) {
        self.by_token.insert(listing.token, listing.id);
        self.by_address.insert(listing.address, listing.id);
        *self.per_source.entry(listing.source).or_default() += 1;
        self.deadlines.insert((listing.heard + expiry, listing.id));
        self.by_id.insert(listing.id, listing);
    }

    /// Removes a listing, giving it back.
    pub fn remove(&mut self, id: u64, expiry: Duration) -> Option<Listing> {
        let listing = self.by_id.remove(&id)?;
        self.by_token.remove(&listing.token);
        if self.by_address.get(&listing.address) == Some(&id) {
            self.by_address.remove(&listing.address);
        }
        if let Some(count) = self.per_source.get_mut(&listing.source) {
            *count -= 1;
            if *count == 0 {
                self.per_source.remove(&listing.source);
            }
        }
        self.deadlines.remove(&(listing.heard + expiry, id));
        Some(listing)
    }

    /// Changes a listing; `heard` and `address` changes keep the indexes
    /// right. Returns what `change` returned, or `None` for no such listing.
    pub fn update<T>(
        &mut self,
        id: u64,
        expiry: Duration,
        change: impl FnOnce(&mut Listing) -> T,
    ) -> Option<T> {
        let listing = self.by_id.get_mut(&id)?;
        let (heard, address, source) = (listing.heard, listing.address, listing.source);
        let result = change(listing);
        if listing.heard != heard {
            self.deadlines.remove(&(heard + expiry, id));
            self.deadlines.insert((listing.heard + expiry, id));
        }
        if listing.address != address {
            if self.by_address.get(&address) == Some(&id) {
                self.by_address.remove(&address);
            }
            self.by_address.insert(listing.address, id);
        }
        if listing.source != source {
            if let Some(count) = self.per_source.get_mut(&source) {
                *count -= 1;
                if *count == 0 {
                    self.per_source.remove(&source);
                }
            }
            *self.per_source.entry(listing.source).or_default() += 1;
        }
        Some(result)
    }

    /// Removes and gives back every listing whose expiry has come by `now`.
    pub fn expire(&mut self, now: Duration, expiry: Duration) -> Vec<Listing> {
        let mut gone = Vec::new();
        while let Some(&(deadline, id)) = self.deadlines.first() {
            if deadline > now {
                break;
            }
            self.deadlines.pop_first();
            // The deadline is already gone from the set; `remove` tolerates it.
            if let Some(listing) = self.remove(id, expiry) {
                gone.push(listing);
            }
        }
        gone
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(id: u64, address: &str, heard: u64) -> Listing {
        let address: SocketAddr = address.parse().unwrap();
        Listing {
            id,
            token: id + 1_000,
            nonce: 1,
            address,
            source: SourceKey::of(address),
            build: Build::default(),
            dedicated: false,
            platform: 3,
            candidates: Vec::new(),
            summary: ListingSummary::default(),
            change: 0,
            mapping: MappingType::Unknown,
            heard: Duration::from_secs(heard),
            moved: None,
            beats: Bucket::default(),
        }
    }

    #[test]
    fn indexes_follow_inserts_moves_and_removals() {
        let expiry = Duration::from_secs(90);
        let mut listings = Listings::default();
        listings.insert(listing(1, "203.0.113.5:26900", 0), expiry);
        listings.insert(listing(2, "203.0.113.5:26901", 0), expiry);
        let source = SourceKey::of("203.0.113.5:1".parse().unwrap());
        assert_eq!(listings.of_source(source), 2);
        assert_eq!(listings.id_of_token(1_001), Some(1));
        assert!(listings.number_taken(1_002) && listings.number_taken(0));
        let moved: SocketAddr = "198.51.100.9:40000".parse().unwrap();
        listings.update(1, expiry, |l| {
            l.address = moved;
            l.source = SourceKey::of(moved);
            l.heard = Duration::from_secs(50);
        });
        assert_eq!(listings.at_address(moved).map(|l| l.id), Some(1));
        assert!(
            listings
                .at_address("203.0.113.5:26900".parse().unwrap())
                .is_none()
        );
        assert_eq!(listings.of_source(source), 1);
        // Listing 2 expires at 90 s, listing 1 (heard at 50) at 140 s.
        let gone = listings.expire(Duration::from_secs(90), expiry);
        assert_eq!(gone.iter().map(|l| l.id).collect::<Vec<_>>(), [2]);
        assert_eq!(listings.of_source(source), 0);
        assert!(listings.expire(Duration::from_secs(139), expiry).is_empty());
        assert_eq!(listings.expire(Duration::from_secs(140), expiry).len(), 1);
        assert!(listings.is_empty());
    }
}
