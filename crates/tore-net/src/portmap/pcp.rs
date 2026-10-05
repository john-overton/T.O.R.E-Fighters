//! PCP (RFC 6887): a MAP request for a UDP port, asked of the gateway on
//! UDP 5351, with a random 96-bit nonce that every answer must repeat and
//! that renewing and removing the mapping reuse. This file also holds the
//! fake gateway the tests (and the game's tests) ask on loopback; it speaks
//! PCP and NAT-PMP, or NAT-PMP only, or nothing.

use super::{Protocol, Silence, exchange, natpmp};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The PCP version this side speaks.
pub(super) const VERSION: u8 = 2;
/// The MAP opcode.
const OP_MAP: u8 = 1;
/// An answer sets the top bit of the opcode byte.
const ANSWER: u8 = 0x80;
/// UDP's protocol number.
const UDP: u8 = 17;
/// A MAP request or answer without options.
const MAP_LEN: usize = 60;
/// The longest PCP message.
const MAX_LEN: usize = 1100;

/// Result codes this side names.
pub(super) const UNSUPP_VERSION: u8 = 1;
pub(super) const NOT_AUTHORIZED: u8 = 2;
pub(super) const MALFORMED_REQUEST: u8 = 3;
pub(super) const UNSUPP_OPCODE: u8 = 4;
pub(super) const ADDRESS_MISMATCH: u8 = 12;

/// A MAP request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MapRequest {
    /// Seconds; 0 removes the mapping.
    pub lifetime: u32,
    /// This side's address, which must be the one the request comes from.
    pub client: IpAddr,
    pub nonce: [u8; 12],
    pub internal_port: u16,
    pub external_port: u16,
    /// The outside address wished for; 0.0.0.0 or :: for no preference.
    pub external_ip: IpAddr,
}

/// A MAP answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct MapAnswer {
    pub result: u8,
    pub lifetime: u32,
    pub epoch: u32,
    pub nonce: [u8; 12],
    pub protocol: u8,
    pub internal_port: u16,
    pub external_port: u16,
    pub external_ip: IpAddr,
}

/// What a datagram from the gateway is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Parsed {
    /// NAT-PMP's answer to a request of another version: the gateway speaks
    /// NAT-PMP only.
    NatPmpOnly,
    /// A PCP error answer too short to carry the request's nonce.
    ShortError(u8),
    Map(MapAnswer),
}

/// An IPv4 address as PCP carries it: IPv4-mapped IPv6.
fn to_16(ip: IpAddr) -> [u8; 16] {
    match ip {
        IpAddr::V4(v4) => v4.to_ipv6_mapped().octets(),
        IpAddr::V6(v6) => v6.octets(),
    }
}

fn from_16(octets: [u8; 16]) -> IpAddr {
    let v6 = Ipv6Addr::from(octets);
    match v6.to_ipv4_mapped() {
        Some(v4) => IpAddr::V4(v4),
        None => IpAddr::V6(v6),
    }
}

fn octets_at<const N: usize>(bytes: &[u8], at: usize) -> [u8; N] {
    let mut out = [0u8; N];
    out.copy_from_slice(&bytes[at..at + N]);
    out
}

/// The request's bytes.
pub(super) fn encode(request: &MapRequest) -> [u8; MAP_LEN] {
    let mut bytes = [0u8; MAP_LEN];
    bytes[0] = VERSION;
    bytes[1] = OP_MAP;
    bytes[4..8].copy_from_slice(&request.lifetime.to_be_bytes());
    bytes[8..24].copy_from_slice(&to_16(request.client));
    bytes[24..36].copy_from_slice(&request.nonce);
    bytes[36] = UDP;
    bytes[40..42].copy_from_slice(&request.internal_port.to_be_bytes());
    bytes[42..44].copy_from_slice(&request.external_port.to_be_bytes());
    bytes[44..60].copy_from_slice(&to_16(request.external_ip));
    bytes
}

/// An answer's bytes (for the fake gateway).
pub(super) fn encode_answer(answer: &MapAnswer) -> [u8; MAP_LEN] {
    let mut bytes = [0u8; MAP_LEN];
    bytes[0] = VERSION;
    bytes[1] = ANSWER | OP_MAP;
    bytes[3] = answer.result;
    bytes[4..8].copy_from_slice(&answer.lifetime.to_be_bytes());
    bytes[8..12].copy_from_slice(&answer.epoch.to_be_bytes());
    bytes[24..36].copy_from_slice(&answer.nonce);
    bytes[36] = answer.protocol;
    bytes[40..42].copy_from_slice(&answer.internal_port.to_be_bytes());
    bytes[42..44].copy_from_slice(&answer.external_port.to_be_bytes());
    bytes[44..60].copy_from_slice(&to_16(answer.external_ip));
    bytes
}

/// Reads a request (for the fake gateway).
pub(super) fn parse_request(bytes: &[u8]) -> Option<MapRequest> {
    if bytes.len() < MAP_LEN || bytes[0] != VERSION || bytes[1] != OP_MAP {
        return None;
    }
    Some(MapRequest {
        lifetime: u32::from_be_bytes(octets_at(bytes, 4)),
        client: from_16(octets_at(bytes, 8)),
        nonce: octets_at(bytes, 24),
        internal_port: u16::from_be_bytes(octets_at(bytes, 40)),
        external_port: u16::from_be_bytes(octets_at(bytes, 42)),
        external_ip: from_16(octets_at(bytes, 44)),
    })
}

/// Reads a datagram from the gateway; `None` for anything that is not an
/// answer to a MAP request.
pub(super) fn parse(bytes: &[u8]) -> Option<Parsed> {
    if bytes.len() < 4 || bytes.len() > MAX_LEN {
        return None;
    }
    match bytes[0] {
        0 => (bytes[1] == ANSWER | OP_MAP && u16::from_be_bytes([bytes[2], bytes[3]]) == 1)
            .then_some(Parsed::NatPmpOnly),
        1 | VERSION => {
            if bytes.len() < 24 || !bytes.len().is_multiple_of(4) || bytes[1] != ANSWER | OP_MAP {
                return None;
            }
            let result = bytes[3];
            if bytes.len() < MAP_LEN || bytes[0] != VERSION {
                return (result != 0).then_some(Parsed::ShortError(result));
            }
            Some(Parsed::Map(MapAnswer {
                result,
                lifetime: u32::from_be_bytes(octets_at(bytes, 4)),
                epoch: u32::from_be_bytes(octets_at(bytes, 8)),
                nonce: octets_at(bytes, 24),
                protocol: bytes[36],
                internal_port: u16::from_be_bytes(octets_at(bytes, 40)),
                external_port: u16::from_be_bytes(octets_at(bytes, 42)),
                external_ip: from_16(octets_at(bytes, 44)),
            }))
        }
        _ => None,
    }
}

/// What a result code means, for the player's message.
pub(super) fn result_text(code: u8) -> &'static str {
    match code {
        UNSUPP_VERSION => "unsupported version",
        NOT_AUTHORIZED => "not authorized",
        MALFORMED_REQUEST => "malformed request",
        UNSUPP_OPCODE => "unsupported request",
        7 => "network failure",
        8 => "no resources",
        9 => "unsupported protocol",
        10 => "over the user's quota",
        11 => "cannot provide that outside address",
        ADDRESS_MISMATCH => "address mismatch",
        _ => "an unknown refusal",
    }
}

/// What a MAP request came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Reply {
    Mapped(MapAnswer),
    /// The gateway speaks NAT-PMP only.
    NatPmpOnly,
}

/// Why PCP gave nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Error {
    Silence(Silence),
    /// The gateway's result code.
    Refused(u8),
}

/// Sends a MAP request to `server` from the request's client address and
/// waits for its answer: one from `server` that repeats the nonce, the
/// protocol and the internal port. Answers that do not are ignored, as are
/// short error answers other than a version, a malformed request or an
/// unsupported opcode, which cannot carry the nonce.
pub(super) fn request(
    server: SocketAddr,
    request: &MapRequest,
    until: Instant,
    stop: &AtomicBool,
) -> Result<Reply, Error> {
    let socket = UdpSocket::bind(SocketAddr::new(request.client, 0))
        .map_err(|error| Error::Silence(Silence::Io(error.kind())))?;
    let bytes = encode(request);
    exchange(
        &socket,
        server,
        &bytes,
        until,
        stop,
        |datagram| match parse(datagram)? {
            Parsed::NatPmpOnly => Some(Ok(Reply::NatPmpOnly)),
            Parsed::ShortError(code @ (UNSUPP_VERSION | MALFORMED_REQUEST | UNSUPP_OPCODE)) => {
                Some(Err(Error::Refused(code)))
            }
            Parsed::ShortError(_) => None,
            Parsed::Map(answer) => {
                let ours = answer.nonce == request.nonce
                    && answer.protocol == UDP
                    && answer.internal_port == request.internal_port;
                match (ours, answer.result) {
                    (false, _) => None,
                    (true, 0) => Some(Ok(Reply::Mapped(answer))),
                    (true, code) => Some(Err(Error::Refused(code))),
                }
            }
        },
    )
    .map_err(Error::Silence)?
}

// ---------------------------------------------------------------------------
// The fake gateway
// ---------------------------------------------------------------------------

/// How a [`FakeGateway`] behaves.
#[derive(Clone, Debug)]
pub struct FakeGatewayConfig {
    /// Answers PCP; when false, answers PCP with NAT-PMP's "unsupported
    /// version" as a NAT-PMP-only gateway does.
    pub pcp: bool,
    /// Answers nothing at all.
    pub silent: bool,
    /// The outside address it reports for IPv4 clients.
    pub outside: Ipv4Addr,
    /// Gives this outside port instead of the one asked for.
    pub assign_port: Option<u16>,
    /// Answers every mapping request with this result code.
    pub refuse: Option<u8>,
    /// Grants at most this many seconds.
    pub max_lifetime: Option<u32>,
    /// Before each PCP answer, sends one with another nonce and outside port
    /// 1111, which the client must ignore.
    pub wrong_nonce_first: bool,
    /// Before each answer, sends one from another port with outside port
    /// 2222, which the client must ignore.
    pub decoy_first: bool,
}

impl Default for FakeGatewayConfig {
    fn default() -> Self {
        Self {
            pcp: true,
            silent: false,
            outside: Ipv4Addr::new(203, 0, 113, 5),
            assign_port: None,
            refuse: None,
            max_lifetime: None,
            wrong_nonce_first: false,
            decoy_first: false,
        }
    }
}

/// A mapping a [`FakeGateway`] holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeMapping {
    /// Which protocol made it.
    pub protocol: Protocol,
    pub client: IpAddr,
    pub internal_port: u16,
    pub external_port: u16,
    /// Seconds granted.
    pub lifetime: u32,
    /// PCP's nonce.
    pub nonce: Option<[u8; 12]>,
}

#[derive(Debug, Default)]
struct GatewayState {
    mappings: Vec<FakeMapping>,
    pcp_requests: usize,
    natpmp_requests: usize,
    silent: bool,
}

/// A PCP and NAT-PMP gateway on loopback for tests: it answers on its own
/// port, keeps the mappings it grants, and never touches a real router.
pub struct FakeGateway {
    address: SocketAddr,
    state: Arc<Mutex<GatewayState>>,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl FakeGateway {
    /// Starts a gateway on `bind`, a loopback address with port 0 (for
    /// example `127.0.0.1:0` or `[::1]:0`).
    pub fn start(bind: SocketAddr, config: FakeGatewayConfig) -> std::io::Result<Self> {
        assert!(
            bind.ip().is_loopback(),
            "the fake gateway listens on loopback only"
        );
        let socket = UdpSocket::bind(bind)?;
        let decoy = UdpSocket::bind(SocketAddr::new(bind.ip(), 0))?;
        socket.set_read_timeout(Some(Duration::from_millis(10)))?;
        let address = socket.local_addr()?;
        let state = Arc::new(Mutex::new(GatewayState {
            silent: config.silent,
            ..GatewayState::default()
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let (state, stop) = (state.clone(), stop.clone());
            std::thread::spawn(move || serve_gateway(&socket, &decoy, &config, &state, &stop))
        };
        Ok(Self {
            address,
            state,
            stop,
            thread: Some(thread),
        })
    }

    /// Where it listens.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The mappings it holds.
    pub fn mappings(&self) -> Vec<FakeMapping> {
        self.state.lock().expect("fake gateway").mappings.clone()
    }

    /// PCP requests received.
    pub fn pcp_requests(&self) -> usize {
        self.state.lock().expect("fake gateway").pcp_requests
    }

    /// NAT-PMP requests received.
    pub fn natpmp_requests(&self) -> usize {
        self.state.lock().expect("fake gateway").natpmp_requests
    }

    /// Stops or starts answering.
    pub fn set_silent(&self, silent: bool) {
        self.state.lock().expect("fake gateway").silent = silent;
    }
}

impl Drop for FakeGateway {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

const EPOCH: u32 = 1000;

fn serve_gateway(
    socket: &UdpSocket,
    decoy: &UdpSocket,
    config: &FakeGatewayConfig,
    state: &Mutex<GatewayState>,
    stop: &AtomicBool,
) {
    let mut buf = [0u8; 1500];
    while !stop.load(Ordering::Acquire) {
        let Ok((len, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        let mut state = state.lock().expect("fake gateway");
        let bytes = &buf[..len];
        match bytes.first() {
            Some(&VERSION) => state.pcp_requests += 1,
            Some(0) => state.natpmp_requests += 1,
            _ => {}
        }
        if state.silent {
            continue;
        }
        let mut answers: Vec<(bool, Vec<u8>)> = Vec::new();
        match bytes.first() {
            Some(&VERSION) => {
                if !config.pcp {
                    let opcode = bytes.get(1).copied().unwrap_or(0);
                    answers.push((false, natpmp::unsupported_version(opcode, EPOCH).to_vec()));
                } else if let Some(request) = parse_request(bytes) {
                    let answer = pcp_answer(config, &mut state, &request, from.ip());
                    if config.decoy_first {
                        let decoy_answer = MapAnswer {
                            external_port: 2222,
                            ..answer
                        };
                        answers.push((true, encode_answer(&decoy_answer).to_vec()));
                    }
                    if config.wrong_nonce_first {
                        let mut nonce = answer.nonce;
                        nonce[0] ^= 0xFF;
                        let wrong = MapAnswer {
                            nonce,
                            external_port: 1111,
                            ..answer
                        };
                        answers.push((false, encode_answer(&wrong).to_vec()));
                    }
                    answers.push((false, encode_answer(&answer).to_vec()));
                }
            }
            Some(0) => {
                if let Some(answer) = natpmp_answer(config, &mut state, bytes, from.ip()) {
                    if config.decoy_first {
                        let mut decoy_answer = answer.clone();
                        if decoy_answer.len() >= 12 && decoy_answer[1] == 129 {
                            decoy_answer[10..12].copy_from_slice(&2222u16.to_be_bytes());
                        } else if decoy_answer.len() >= 12 {
                            decoy_answer[8..12].copy_from_slice(&[198, 51, 100, 2]);
                        }
                        answers.push((true, decoy_answer));
                    }
                    answers.push((false, answer));
                }
            }
            _ => {}
        }
        drop(state);
        for (from_decoy, answer) in answers {
            let _ = if from_decoy {
                decoy.send_to(&answer, from)
            } else {
                socket.send_to(&answer, from)
            };
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn pcp_answer(
    config: &FakeGatewayConfig,
    state: &mut GatewayState,
    request: &MapRequest,
    from: IpAddr,
) -> MapAnswer {
    let mut answer = MapAnswer {
        result: 0,
        lifetime: request.lifetime,
        epoch: EPOCH,
        nonce: request.nonce,
        protocol: UDP,
        internal_port: request.internal_port,
        external_port: request.external_port,
        external_ip: request.external_ip,
    };
    let refuse = |answer: MapAnswer, code: u8| MapAnswer {
        result: code,
        lifetime: 0,
        ..answer
    };
    if request.client != from {
        return refuse(answer, ADDRESS_MISMATCH);
    }
    if let Some(code) = config.refuse {
        return refuse(answer, code);
    }
    let existing = state
        .mappings
        .iter()
        .position(|m| m.client == request.client && m.internal_port == request.internal_port);
    if let Some(at) = existing
        && state.mappings[at]
            .nonce
            .is_some_and(|nonce| nonce != request.nonce)
    {
        return refuse(answer, NOT_AUTHORIZED);
    }
    let outside = match request.client {
        IpAddr::V4(_) => IpAddr::V4(config.outside),
        IpAddr::V6(v6) => IpAddr::V6(v6),
    };
    if request.lifetime == 0 {
        if let Some(at) = existing {
            let removed = state.mappings.remove(at);
            answer.external_port = removed.external_port;
        }
        answer.external_ip = outside;
        return answer;
    }
    let external_port = existing
        .map(|at| state.mappings[at].external_port)
        .or(config.assign_port)
        .unwrap_or(if request.external_port != 0 {
            request.external_port
        } else {
            request.internal_port
        });
    let lifetime = config
        .max_lifetime
        .map_or(request.lifetime, |most| request.lifetime.min(most));
    let mapping = FakeMapping {
        protocol: Protocol::Pcp,
        client: request.client,
        internal_port: request.internal_port,
        external_port,
        lifetime,
        nonce: Some(request.nonce),
    };
    match existing {
        Some(at) => state.mappings[at] = mapping,
        None => state.mappings.push(mapping),
    }
    MapAnswer {
        lifetime,
        external_port,
        external_ip: outside,
        ..answer
    }
}

fn natpmp_answer(
    config: &FakeGatewayConfig,
    state: &mut GatewayState,
    bytes: &[u8],
    from: IpAddr,
) -> Option<Vec<u8>> {
    let (opcode, internal, suggested, lifetime) = natpmp::parse_request(bytes)?;
    let refuse = config.refuse.map(u16::from);
    if opcode == 0 {
        return Some(natpmp::address_answer(refuse.unwrap_or(0), EPOCH, config.outside).to_vec());
    }
    if let Some(code) = refuse {
        return Some(natpmp::map_answer(code, EPOCH, internal, 0, 0).to_vec());
    }
    let existing = state
        .mappings
        .iter()
        .position(|m| m.client == from && m.internal_port == internal);
    if lifetime == 0 {
        if let Some(at) = existing {
            state.mappings.remove(at);
        }
        return Some(natpmp::map_answer(0, EPOCH, internal, 0, 0).to_vec());
    }
    let external = existing
        .map(|at| state.mappings[at].external_port)
        .or(config.assign_port)
        .unwrap_or(if suggested != 0 { suggested } else { internal });
    let lifetime = config
        .max_lifetime
        .map_or(lifetime, |most| lifetime.min(most));
    let mapping = FakeMapping {
        protocol: Protocol::NatPmp,
        client: from,
        internal_port: internal,
        external_port: external,
        lifetime,
        nonce: None,
    };
    match existing {
        Some(at) => state.mappings[at] = mapping,
        None => state.mappings.push(mapping),
    }
    Some(natpmp::map_answer(0, EPOCH, internal, external, lifetime).to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitMix64;

    fn sample() -> MapRequest {
        MapRequest {
            lifetime: 3600,
            client: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 23)),
            nonce: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12],
            internal_port: 26900,
            external_port: 26900,
            external_ip: IpAddr::V4(Ipv4Addr::UNSPECIFIED),
        }
    }

    #[test]
    fn a_map_request_is_laid_out_as_rfc_6887_says() {
        let bytes = encode(&sample());
        assert_eq!(&bytes[..4], &[2, 1, 0, 0]);
        assert_eq!(&bytes[4..8], &3600u32.to_be_bytes());
        assert_eq!(
            &bytes[8..24],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, 192, 168, 1, 23]
        );
        assert_eq!(&bytes[24..36], &sample().nonce);
        assert_eq!(&bytes[36..40], &[17, 0, 0, 0]);
        assert_eq!(&bytes[40..44], &[0x69, 0x14, 0x69, 0x14]);
        assert_eq!(
            &bytes[44..60],
            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0]
        );
        assert_eq!(parse_request(&bytes), Some(sample()));
    }

    #[test]
    fn answers_round_trip_and_refusals_are_read() {
        let answer = MapAnswer {
            result: 0,
            lifetime: 1800,
            epoch: 5,
            nonce: sample().nonce,
            protocol: UDP,
            internal_port: 26900,
            external_port: 40000,
            external_ip: IpAddr::V4(Ipv4Addr::new(203, 0, 113, 5)),
        };
        assert_eq!(parse(&encode_answer(&answer)), Some(Parsed::Map(answer)));
        assert_eq!(
            parse(&natpmp::unsupported_version(OP_MAP, 3)),
            Some(Parsed::NatPmpOnly)
        );
        let mut short = [0u8; 24];
        short[0] = VERSION;
        short[1] = ANSWER | OP_MAP;
        short[3] = UNSUPP_VERSION;
        assert_eq!(parse(&short), Some(Parsed::ShortError(UNSUPP_VERSION)));
        short[3] = 0;
        assert_eq!(parse(&short), None);
        assert_eq!(
            parse(&encode(&sample())),
            None,
            "a request is not an answer"
        );
        let v6 = IpAddr::V6("2001:db8::5".parse().unwrap());
        assert_eq!(from_16(to_16(v6)), v6);
    }

    #[test]
    fn fuzzed_answers_never_panic() {
        let answer = encode_answer(&MapAnswer {
            result: 0,
            lifetime: 1,
            epoch: 1,
            nonce: [7; 12],
            protocol: UDP,
            internal_port: 1,
            external_port: 2,
            external_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
        });
        let mut rng = SplitMix64::new(8);
        for round in 0..100_000 {
            let mut bytes = answer.to_vec();
            if round % 3 == 0 {
                bytes.truncate(rng.below(64) as usize);
            }
            for _ in 0..rng.below(8) {
                if bytes.is_empty() {
                    break;
                }
                let at = rng.below(bytes.len() as u64) as usize;
                bytes[at] = rng.next_u64() as u8;
            }
            let _ = parse(&bytes);
            let _ = parse_request(&bytes);
        }
    }
}
