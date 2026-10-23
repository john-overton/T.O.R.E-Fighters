//! Where an endpoint draws its secrets: nonces, connection ids and the cookie
//! key.
//!
//! A real endpoint uses [`Entropy::System`]: every value comes from the
//! standard library's randomly seeded hasher, as the protocol's "Connecting"
//! section asks, so no value predicts another. Tests and the network simulator
//! use [`Entropy::Seeded`], which makes a whole session repeat exactly for a
//! seed. A seeded endpoint's secrets are predictable by design; never use it on
//! a real network.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, DefaultHasher, Hash, Hasher};
use std::net::SocketAddr;

/// The source of an endpoint's secrets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Entropy {
    /// Unpredictable values from the standard library's randomly seeded
    /// hasher. The default, and the only choice for a real network.
    #[default]
    System,
    /// A repeatable stream for tests and the simulator.
    Seeded(u64),
}

/// SplitMix64: a small, fast, seeded generator. Used by the simulator, by
/// seeded endpoints and by tests. Not for secrets on a real network.
#[derive(Debug, Clone)]
pub struct SplitMix64(pub u64);

impl SplitMix64 {
    /// A generator starting from `seed`.
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// The next 64 random bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A number from 0 up to but not including 1, with 53 random bits.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// A number from 0 up to but not including `n`; 0 when `n` is 0.
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }

    /// True with probability `p` (0 never, 1 always).
    pub fn chance(&mut self, p: f64) -> bool {
        p > 0.0 && self.next_f64() < p
    }
}

/// An endpoint's generator for nonces and connection ids.
#[derive(Debug, Clone)]
pub(crate) enum Rng {
    /// Each value is a fresh `RandomState`'s hash of a counter: `RandomState`
    /// keys SipHash with random keys, so outputs reveal nothing of each other.
    System(u64),
    Seeded(SplitMix64),
}

impl Rng {
    pub(crate) fn new(entropy: Entropy) -> Self {
        match entropy {
            Entropy::System => Self::System(0),
            Entropy::Seeded(seed) => Self::Seeded(SplitMix64::new(seed)),
        }
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        match self {
            Self::System(counter) => {
                *counter = counter.wrapping_add(1);
                RandomState::new().hash_one(*counter)
            }
            Self::Seeded(rng) => rng.next_u64(),
        }
    }
}

/// Where a host draws its players' rejoin tokens (stage K): each token is two
/// draws of the endpoint's generator, so with [`Entropy::System`] it is 128
/// bits from the standard library's randomly keyed hasher, which the
/// operating system's random source keys: no new dependency (John,
/// 2026-10-05). A seeded source repeats for tests and the simulator.
#[derive(Debug, Clone)]
pub struct TokenSource(Rng);

impl TokenSource {
    /// A source of tokens.
    pub fn new(entropy: Entropy) -> Self {
        Self(Rng::new(entropy))
    }

    /// The next token: two draws, the first its low 64 bits.
    pub fn next_token(&mut self) -> crate::packet::Token {
        let low = self.0.next_u64();
        let high = self.0.next_u64();
        crate::packet::Token(u128::from(low) | u128::from(high) << 64)
    }
}

/// The host's key for the stateless connect cookie.
#[derive(Debug, Clone)]
pub(crate) enum CookieKey {
    /// SipHash with a random 128-bit key, drawn once at host start.
    System(RandomState),
    /// A repeatable key for seeded endpoints.
    Seeded(u64, u64),
}

impl CookieKey {
    pub(crate) fn new(entropy: Entropy, rng: &mut Rng) -> Self {
        match entropy {
            Entropy::System => Self::System(RandomState::new()),
            Entropy::Seeded(_) => Self::Seeded(rng.next_u64(), rng.next_u64()),
        }
    }

    /// The cookie for a client's address and port, its nonce and a 10-second
    /// time slot.
    pub(crate) fn cookie(&self, address: SocketAddr, nonce: u64, slot: u64) -> u64 {
        match self {
            Self::System(state) => state.hash_one((address, nonce, slot)),
            Self::Seeded(k0, k1) => {
                let mut hasher = DefaultHasher::new();
                (k0, k1, address, nonce, slot).hash(&mut hasher);
                hasher.finish()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeded_streams_repeat_and_system_ones_differ() {
        let mut a = Rng::new(Entropy::Seeded(7));
        let mut b = Rng::new(Entropy::Seeded(7));
        for _ in 0..10 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
        let mut s = Rng::new(Entropy::System);
        let first = s.next_u64();
        let second = s.next_u64();
        assert_ne!(first, second);
    }

    #[test]
    fn tokens_are_two_draws_and_repeat_only_when_seeded() {
        let mut a = TokenSource::new(Entropy::Seeded(9));
        let mut b = TokenSource::new(Entropy::Seeded(9));
        let mut draws = Rng::new(Entropy::Seeded(9));
        for _ in 0..4 {
            let token = a.next_token();
            assert_eq!(token, b.next_token());
            let low = draws.next_u64();
            let high = draws.next_u64();
            assert_eq!(token.0, u128::from(low) | u128::from(high) << 64);
        }
        let mut system = TokenSource::new(Entropy::System);
        let first = system.next_token();
        let second = system.next_token();
        assert_ne!(first, second);
        // Both halves are drawn: neither is left zero by the joining.
        assert_ne!(first.0 >> 64, 0);
        assert_ne!(first.0 as u64, 0);
    }

    #[test]
    fn cookies_depend_on_every_input() {
        let mut rng = Rng::new(Entropy::Seeded(1));
        let key = CookieKey::new(Entropy::Seeded(1), &mut rng);
        let addr: SocketAddr = "127.0.0.1:5000".parse().unwrap();
        let other: SocketAddr = "127.0.0.1:5001".parse().unwrap();
        let base = key.cookie(addr, 9, 100);
        assert_eq!(base, key.cookie(addr, 9, 100));
        assert_ne!(base, key.cookie(other, 9, 100));
        assert_ne!(base, key.cookie(addr, 10, 100));
        assert_ne!(base, key.cookie(addr, 9, 101));
        let system = CookieKey::new(Entropy::System, &mut rng);
        assert_eq!(system.cookie(addr, 9, 100), system.cookie(addr, 9, 100));
    }
}
