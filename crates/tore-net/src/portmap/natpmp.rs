//! NAT-PMP (RFC 6886): the gateway's outside address and a UDP mapping,
//! asked of the gateway on UDP 5351. Used when the gateway answers a PCP
//! request with NAT-PMP's "unsupported version".

use super::{Silence, exchange};
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

/// Ask for the gateway's outside address.
const OP_ADDRESS: u8 = 0;
/// Ask for a UDP mapping.
const OP_MAP_UDP: u8 = 1;
/// An answer's opcode is the request's plus 128.
const ANSWER: u8 = 128;

/// The request for the outside address.
pub(super) fn address_request() -> [u8; 2] {
    [0, OP_ADDRESS]
}

/// A request to map UDP `internal` for `lifetime` seconds, preferably on
/// outside port `suggested`. A lifetime of 0 with a suggested port of 0
/// removes the mapping.
pub(super) fn map_request(internal: u16, suggested: u16, lifetime: u32) -> [u8; 12] {
    let mut bytes = [0u8; 12];
    bytes[1] = OP_MAP_UDP;
    bytes[4..6].copy_from_slice(&internal.to_be_bytes());
    bytes[6..8].copy_from_slice(&suggested.to_be_bytes());
    bytes[8..12].copy_from_slice(&lifetime.to_be_bytes());
    bytes
}

/// A gateway's answer to an address request (for the fake gateway).
pub(super) fn address_answer(result: u16, epoch: u32, address: Ipv4Addr) -> [u8; 12] {
    let mut bytes = [0u8; 12];
    bytes[1] = ANSWER | OP_ADDRESS;
    bytes[2..4].copy_from_slice(&result.to_be_bytes());
    bytes[4..8].copy_from_slice(&epoch.to_be_bytes());
    bytes[8..12].copy_from_slice(&address.octets());
    bytes
}

/// A gateway's answer to a mapping request (for the fake gateway).
pub(super) fn map_answer(
    result: u16,
    epoch: u32,
    internal: u16,
    external: u16,
    lifetime: u32,
) -> [u8; 16] {
    let mut bytes = [0u8; 16];
    bytes[1] = ANSWER | OP_MAP_UDP;
    bytes[2..4].copy_from_slice(&result.to_be_bytes());
    bytes[4..8].copy_from_slice(&epoch.to_be_bytes());
    bytes[8..10].copy_from_slice(&internal.to_be_bytes());
    bytes[10..12].copy_from_slice(&external.to_be_bytes());
    bytes[12..16].copy_from_slice(&lifetime.to_be_bytes());
    bytes
}

/// What a NAT-PMP gateway answers a request of another version (RFC 6886,
/// section 3.5): version 0, the request's opcode plus 128, result 1.
pub(super) fn unsupported_version(opcode: u8, epoch: u32) -> [u8; 8] {
    let mut bytes = [0u8; 8];
    bytes[1] = ANSWER | (opcode & 0x7F);
    bytes[2..4].copy_from_slice(&1u16.to_be_bytes());
    bytes[4..8].copy_from_slice(&epoch.to_be_bytes());
    bytes
}

/// Reads a request (for the fake gateway): the opcode and, for a mapping,
/// the internal port, suggested port and lifetime.
pub(super) fn parse_request(bytes: &[u8]) -> Option<(u8, u16, u16, u32)> {
    match bytes {
        [0, OP_ADDRESS, ..] => Some((OP_ADDRESS, 0, 0, 0)),
        [0, OP_MAP_UDP, _, _, i0, i1, s0, s1, l0, l1, l2, l3, ..] => Some((
            OP_MAP_UDP,
            u16::from_be_bytes([*i0, *i1]),
            u16::from_be_bytes([*s0, *s1]),
            u32::from_be_bytes([*l0, *l1, *l2, *l3]),
        )),
        _ => None,
    }
}

/// A gateway's answer. An error answer may stop after its epoch; its other
/// fields are then 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Response {
    Address {
        result: u16,
        epoch: u32,
        address: Ipv4Addr,
    },
    Map {
        result: u16,
        epoch: u32,
        internal: u16,
        external: u16,
        lifetime: u32,
    },
}

/// Reads an answer; `None` for anything else.
pub(super) fn parse(bytes: &[u8]) -> Option<Response> {
    if bytes.len() < 8 || bytes.len() > 1100 || bytes[0] != 0 {
        return None;
    }
    let u16_at = |at: usize| {
        bytes
            .get(at..at + 2)
            .map_or(0, |b| u16::from_be_bytes([b[0], b[1]]))
    };
    let u32_at = |at: usize| {
        bytes
            .get(at..at + 4)
            .map_or(0, |b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
    };
    let result = u16_at(2);
    let epoch = u32_at(4);
    match bytes[1] {
        op if op == ANSWER | OP_ADDRESS => {
            if result == 0 && bytes.len() < 12 {
                return None;
            }
            Some(Response::Address {
                result,
                epoch,
                address: Ipv4Addr::from(u32_at(8)),
            })
        }
        op if op == ANSWER | OP_MAP_UDP => {
            if result == 0 && bytes.len() < 16 {
                return None;
            }
            Some(Response::Map {
                result,
                epoch,
                internal: u16_at(8),
                external: u16_at(10),
                lifetime: u32_at(12),
            })
        }
        _ => None,
    }
}

/// Why NAT-PMP gave nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Silence(Silence),
    /// The gateway's result code.
    Refused(u16),
}

/// What a result code means, for the player's message.
pub(super) fn result_text(code: u16) -> &'static str {
    match code {
        1 => "unsupported version",
        2 => "not authorized",
        3 => "network failure",
        4 => "out of resources",
        5 => "unsupported request",
        _ => "an unknown refusal",
    }
}

fn socket_for(gateway: SocketAddr) -> Result<UdpSocket, Error> {
    let any: SocketAddr = match gateway {
        SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
        SocketAddr::V6(_) => (std::net::Ipv6Addr::UNSPECIFIED, 0).into(),
    };
    UdpSocket::bind(any).map_err(|error| Error::Silence(Silence::Io(error.kind())))
}

/// The gateway's outside address.
pub(super) fn public_address(
    gateway: SocketAddr,
    until: Instant,
    stop: &AtomicBool,
) -> Result<Ipv4Addr, Error> {
    let socket = socket_for(gateway)?;
    let answer = exchange(
        &socket,
        gateway,
        &address_request(),
        until,
        stop,
        |bytes| match parse(bytes) {
            Some(Response::Address {
                result, address, ..
            }) => Some((result, address)),
            _ => None,
        },
    )
    .map_err(Error::Silence)?;
    match answer {
        (0, address) => Ok(address),
        (code, _) => Err(Error::Refused(code)),
    }
}

/// Maps UDP `internal` (or removes its mapping, with a lifetime of 0): the
/// outside port and the lifetime the gateway granted.
pub(super) fn map_udp(
    gateway: SocketAddr,
    internal: u16,
    suggested: u16,
    lifetime: u32,
    until: Instant,
    stop: &AtomicBool,
) -> Result<(u16, u32), Error> {
    let socket = socket_for(gateway)?;
    let request = map_request(internal, suggested, lifetime);
    let answer = exchange(
        &socket,
        gateway,
        &request,
        until,
        stop,
        |bytes| match parse(bytes) {
            Some(Response::Map {
                result,
                internal: answered,
                external,
                lifetime,
                ..
            }) if answered == internal || result != 0 => Some((result, external, lifetime)),
            _ => None,
        },
    )
    .map_err(Error::Silence)?;
    match answer {
        (0, external, granted) => Ok((external, granted)),
        (code, _, _) => Err(Error::Refused(code)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitMix64;

    #[test]
    fn requests_and_answers() {
        assert_eq!(address_request(), [0, 0]);
        assert_eq!(
            map_request(26900, 26901, 3600),
            [0, 1, 0, 0, 0x69, 0x14, 0x69, 0x15, 0, 0, 0x0E, 0x10]
        );
        let address = [0, 128, 0, 0, 0, 0, 0, 9, 203, 0, 113, 5];
        assert_eq!(
            parse(&address),
            Some(Response::Address {
                result: 0,
                epoch: 9,
                address: Ipv4Addr::new(203, 0, 113, 5)
            })
        );
        let map = [
            0, 129, 0, 0, 0, 0, 0, 9, 0x69, 0x14, 0x69, 0x15, 0, 0, 0x0E, 0x10,
        ];
        assert_eq!(
            parse(&map),
            Some(Response::Map {
                result: 0,
                epoch: 9,
                internal: 26900,
                external: 26901,
                lifetime: 3600
            })
        );
        // An error answer may be cut after its epoch.
        assert_eq!(
            parse(&[0, 129, 0, 2, 0, 0, 0, 1]),
            Some(Response::Map {
                result: 2,
                epoch: 1,
                internal: 0,
                external: 0,
                lifetime: 0
            })
        );
        assert_eq!(map_answer(0, 9, 26900, 26901, 3600), map);
        assert_eq!(address_answer(0, 9, Ipv4Addr::new(203, 0, 113, 5)), address);
        assert_eq!(unsupported_version(1, 7), [0, 129, 0, 1, 0, 0, 0, 7]);
        assert_eq!(
            parse_request(&map_request(26900, 26901, 3600)),
            Some((1, 26900, 26901, 3600))
        );
        assert_eq!(parse_request(&address_request()), Some((0, 0, 0, 0)));
        assert_eq!(parse(&map[..12]), None);
        assert_eq!(parse(&[2, 129, 0, 0, 0, 0, 0, 0]), None);
        assert_eq!(parse(&[0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]), None);
    }

    #[test]
    fn fuzzed_answers_never_panic() {
        let mut rng = SplitMix64::new(5);
        for _ in 0..100_000 {
            let len = rng.below(24) as usize;
            let mut bytes: Vec<u8> = (0..len).map(|_| rng.next_u64() as u8).collect();
            if let Some(first) = bytes.first_mut() {
                *first = if rng.chance(0.8) { 0 } else { *first };
            }
            if bytes.len() > 1 && rng.chance(0.8) {
                bytes[1] = 128 + rng.below(3) as u8;
            }
            let _ = parse(&bytes);
        }
    }
}
