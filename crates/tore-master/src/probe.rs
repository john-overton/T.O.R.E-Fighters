//! Probe answers on both ports, and what the master learns from them
//! ("Mapping test" in the master protocol).
//!
//! A game sends a Probe to each of the master's two ports from the same
//! socket; each port answers with the address it saw. The game reads its
//! router's mapping type from the two answers. The master needs the host's
//! type too, for a Page's "relay likely" flag and, in stage J, an
//! Introduction's hint, but Register does not carry it, so the master pairs
//! the two Probes of one test itself (agent decision): a game sends **the
//! same nonce** to both ports, and the master matches the Probes by that
//! nonce and the sender's IP address, then keeps the verdict under the
//! address its main port saw, where the host's Register then comes from.
//! A pair whose second half never comes is forgotten after 10 seconds, a
//! verdict after 15 minutes (the host repeats its test every 10).

use std::collections::{BTreeMap, HashMap};
use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use tore_net::master::{MappingType, ProbePort};

/// A half test waits this long for its other half.
pub const PAIR_WAIT: Duration = Duration::from_secs(10);
/// A verdict is kept this long.
pub const VERDICT_LIFETIME: Duration = Duration::from_secs(15 * 60);
/// The most half tests and verdicts kept, each (agent decision: bounded like
/// the sources table).
pub const MAX_KEPT: usize = 65_536;

#[derive(Debug, Clone, Copy)]
struct Half {
    main: Option<SocketAddr>,
    second: Option<SocketAddr>,
    /// Its key in `half_order`.
    order: (Duration, u64),
}

/// Mapping tests under way, and their verdicts.
#[derive(Debug, Clone, Default)]
pub struct MappingTests {
    halves: HashMap<(IpAddr, u64), Half>,
    half_order: BTreeMap<(Duration, u64), (IpAddr, u64)>,
    verdicts: HashMap<SocketAddr, (MappingType, (Duration, u64))>,
    verdict_order: BTreeMap<(Duration, u64), SocketAddr>,
    order: u64,
}

impl MappingTests {
    /// Notes a Probe with `nonce` that arrived at `port` from `seen`.
    pub fn probed(&mut self, now: Duration, port: ProbePort, seen: SocketAddr, nonce: u64) {
        self.expire(now);
        let key = (seen.ip(), nonce);
        let half = match self.halves.get_mut(&key) {
            Some(half) => half,
            None => {
                if self.halves.len() >= MAX_KEPT
                    && let Some((_, oldest)) = self.half_order.pop_first()
                {
                    self.halves.remove(&oldest);
                }
                self.order += 1;
                let order = (now, self.order);
                self.half_order.insert(order, key);
                self.halves.entry(key).or_insert(Half {
                    main: None,
                    second: None,
                    order,
                })
            }
        };
        match port {
            ProbePort::Main => half.main = Some(seen),
            ProbePort::Second => half.second = Some(seen),
        }
        if let (Some(main), Some(second)) = (half.main, half.second) {
            let order = half.order;
            self.halves.remove(&key);
            self.half_order.remove(&order);
            let verdict = MappingType::from_probes(None, main, Some(second));
            self.keep(now, main, verdict);
        }
    }

    fn keep(&mut self, now: Duration, main: SocketAddr, verdict: MappingType) {
        if let Some((_, order)) = self.verdicts.get(&main).copied() {
            self.verdict_order.remove(&order);
        } else if self.verdicts.len() >= MAX_KEPT
            && let Some((_, oldest)) = self.verdict_order.pop_first()
        {
            self.verdicts.remove(&oldest);
        }
        self.order += 1;
        let order = (now, self.order);
        self.verdicts.insert(main, (verdict, order));
        self.verdict_order.insert(order, main);
    }

    /// The mapping type of a socket the main port sees at `address`, as far
    /// as the master knows. `own` are the addresses the socket says it has
    /// (its Local and Global IPv6 candidates): seeing one of them means no
    /// translation.
    pub fn mapping_of(
        &mut self,
        now: Duration,
        address: SocketAddr,
        own: impl IntoIterator<Item = SocketAddr>,
    ) -> MappingType {
        self.expire(now);
        if own.into_iter().any(|a| a == address) {
            return MappingType::NoTranslation;
        }
        self.verdicts
            .get(&address)
            .map_or(MappingType::Unknown, |(verdict, _)| *verdict)
    }

    /// How many half tests and verdicts are kept.
    pub fn len(&self) -> (usize, usize) {
        (self.halves.len(), self.verdicts.len())
    }

    fn expire(&mut self, now: Duration) {
        while let Some((&(at, order), &key)) = self.half_order.first_key_value() {
            if now.saturating_sub(at) < PAIR_WAIT {
                break;
            }
            self.half_order.remove(&(at, order));
            self.halves.remove(&key);
        }
        while let Some((&(at, order), &address)) = self.verdict_order.first_key_value() {
            if now.saturating_sub(at) < VERDICT_LIFETIME {
                break;
            }
            self.verdict_order.remove(&(at, order));
            self.verdicts.remove(&address);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    #[test]
    fn two_probes_with_one_nonce_give_a_verdict() {
        let mut tests = MappingTests::default();
        let t = Duration::from_secs(1);
        tests.probed(t, ProbePort::Main, a("203.0.113.5:26900"), 7);
        tests.probed(t, ProbePort::Second, a("203.0.113.5:26900"), 7);
        assert_eq!(
            tests.mapping_of(t, a("203.0.113.5:26900"), []),
            MappingType::SamePort
        );
        tests.probed(t, ProbePort::Main, a("198.51.100.2:40000"), 8);
        tests.probed(t, ProbePort::Second, a("198.51.100.2:40001"), 8);
        assert_eq!(
            tests.mapping_of(t, a("198.51.100.2:40000"), []),
            MappingType::PortPerDestination
        );
        // Another nonce does not pair.
        tests.probed(t, ProbePort::Main, a("192.0.2.1:5"), 1);
        tests.probed(t, ProbePort::Second, a("192.0.2.1:6"), 2);
        assert_eq!(
            tests.mapping_of(t, a("192.0.2.1:5"), []),
            MappingType::Unknown
        );
        // A socket seen at its own address is not translated.
        assert_eq!(
            tests.mapping_of(t, a("192.0.2.1:5"), [a("192.0.2.1:5")]),
            MappingType::NoTranslation
        );
        assert_eq!(tests.len(), (2, 2));
        // Halves go after 10 s, verdicts after 15 minutes.
        let later = t + PAIR_WAIT;
        assert_eq!(
            tests.mapping_of(later, a("9.9.9.9:9"), []),
            MappingType::Unknown
        );
        assert_eq!(tests.len(), (0, 2));
        assert_eq!(
            tests.mapping_of(t + VERDICT_LIFETIME, a("203.0.113.5:26900"), []),
            MappingType::Unknown
        );
        assert_eq!(tests.len(), (0, 0));
    }
}
