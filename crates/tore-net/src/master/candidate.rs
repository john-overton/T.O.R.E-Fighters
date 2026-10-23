//! The master protocol's common fields: addresses, candidates and mapping
//! types ("Common fields" in the master protocol).
//!
//! An **address** is a family bit (0 IPv4, 1 IPv6), the address's octets in
//! their usual order, 8 bits each, then the port (16 bits). An IPv4-mapped
//! IPv6 address is sent as IPv4, and one sent as IPv6 is malformed, so every
//! address has one encoding. An IPv6 address's flow label and scope are not
//! sent. A **candidate** is an address one end might be reached at, with its
//! kind (3 bits); a **candidate list** is a count (4 bits, at most 8) and the
//! candidates.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV6};

use tore_codec::{BitReader, BitWriter};

use super::packet::{MasterDecodeError, MasterEncodeError};

/// The most candidates a list carries.
pub const MAX_CANDIDATES: usize = 8;

/// An address's width on the wire, in bits: the family bit, the address and
/// the port.
pub fn address_bits(address: SocketAddr) -> usize {
    match canonical(address) {
        SocketAddr::V4(_) => 1 + 32 + 16,
        SocketAddr::V6(_) => 1 + 128 + 16,
    }
}

/// The address as the wire carries it: an IPv4-mapped IPv6 address becomes
/// IPv4, and an IPv6 address loses its flow label and scope.
pub fn canonical(address: SocketAddr) -> SocketAddr {
    match address {
        SocketAddr::V4(_) => address,
        SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
            Some(v4) => SocketAddr::new(IpAddr::V4(v4), v6.port()),
            None => SocketAddr::V6(SocketAddrV6::new(*v6.ip(), v6.port(), 0, 0)),
        },
    }
}

/// Writes an address.
pub fn write_address(w: &mut BitWriter, address: SocketAddr) {
    match canonical(address) {
        SocketAddr::V4(v4) => {
            w.write_bool(false);
            for octet in v4.ip().octets() {
                w.write_bits(u64::from(octet), 8).ok();
            }
        }
        SocketAddr::V6(v6) => {
            w.write_bool(true);
            for octet in v6.ip().octets() {
                w.write_bits(u64::from(octet), 8).ok();
            }
        }
    }
    w.write_bits(u64::from(address.port()), 16).ok();
}

/// Reads an address. An IPv4-mapped address sent as IPv6 is malformed.
pub fn read_address(r: &mut BitReader<'_>) -> Result<SocketAddr, MasterDecodeError> {
    let ip = if r.read_bool()? {
        let mut octets = [0u8; 16];
        for octet in &mut octets {
            *octet = r.read_bits(8)? as u8;
        }
        let ip = Ipv6Addr::from(octets);
        if ip.to_ipv4_mapped().is_some() {
            return Err(MasterDecodeError::Malformed);
        }
        IpAddr::V6(ip)
    } else {
        let mut octets = [0u8; 4];
        for octet in &mut octets {
            *octet = r.read_bits(8)? as u8;
        }
        IpAddr::V4(Ipv4Addr::from(octets))
    };
    let port = r.read_bits(16)? as u16;
    Ok(SocketAddr::new(ip, port))
}

codes! {
    /// What a candidate is.
    CandidateKind: 3 bits {
        /// An address on the sender's own network: its LAN IPv4 address, or
        /// an IPv6 address that is not global, with the game port.
        Local = 0,
        /// Where the master saw the sender's packet come from. Only the
        /// master fills these in; a sender's own list never has one.
        Seen = 1,
        /// An outside address a router's port mapping gave (UPnP, NAT-PMP or
        /// PCP).
        Mapped = 2,
        /// The sender's own global IPv6 address (2000::/3) with the game
        /// port: no translation, though a firewall may stand in front.
        GlobalIpv6 = 3,
    }
}

codes! {
    /// How a router treats the game port: the result of the mapping test.
    MappingType: 2 bits {
        /// Not tested, or the test could not tell.
        Unknown = 0,
        /// No translation: the master saw the sender's own address.
        NoTranslation = 1,
        /// The same outside port to both master ports: hole punching can
        /// work.
        SamePort = 2,
        /// A different outside port to each master port ("symmetric"):
        /// punching between two such routers fails.
        PortPerDestination = 3,
    }
}

impl MappingType {
    /// The mapping test's verdict from what the master saw on its main port
    /// and, when the test's second half answered, on its second port.
    /// `own` is the socket's own address on the way to the master (its Local
    /// or Global IPv6 candidate), when the game knows it.
    pub fn from_probes(
        own: Option<SocketAddr>,
        main: SocketAddr,
        second: Option<SocketAddr>,
    ) -> Self {
        let main = canonical(main);
        if own.map(canonical) == Some(main) {
            return Self::NoTranslation;
        }
        match second.map(canonical) {
            Some(second) if second == main => Self::SamePort,
            Some(second) if second.ip() == main.ip() => Self::PortPerDestination,
            // Another outside address for each port: still per destination.
            Some(_) => Self::PortPerDestination,
            None => Self::Unknown,
        }
    }
}

/// An address one end might be reached at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Candidate {
    /// What the address is.
    pub kind: CandidateKind,
    /// The address and port; written in its [canonical] form.
    pub address: SocketAddr,
}

impl Candidate {
    /// A candidate of `kind` at `address`.
    pub fn new(kind: CandidateKind, address: SocketAddr) -> Self {
        Self { kind, address }
    }

    /// Its width on the wire, in bits.
    pub fn bits(&self) -> usize {
        CandidateKind::BITS as usize + address_bits(self.address)
    }
}

/// True when the relay is likely needed to reach a host: its router maps a
/// port per destination and it has neither a mapped port nor a global IPv6
/// address. The Page's "relay likely" flag, and with the player's own type
/// the Introduction's hint to ask for the relay at once.
pub fn relay_likely(mapping: MappingType, candidates: &[Candidate]) -> bool {
    mapping == MappingType::PortPerDestination
        && !candidates
            .iter()
            .any(|c| matches!(c.kind, CandidateKind::Mapped | CandidateKind::GlobalIpv6))
}

/// The width of a candidate list on the wire, in bits.
pub fn list_bits(candidates: &[Candidate]) -> usize {
    4 + candidates.iter().map(Candidate::bits).sum::<usize>()
}

/// Writes a candidate list. More than [`MAX_CANDIDATES`], or a Seen
/// candidate in a sender's own list (`from_sender`), is refused.
pub fn write_list(
    w: &mut BitWriter,
    candidates: &[Candidate],
    from_sender: bool,
) -> Result<(), MasterEncodeError> {
    if candidates.len() > MAX_CANDIDATES
        || (from_sender && candidates.iter().any(|c| c.kind == CandidateKind::Seen))
    {
        return Err(MasterEncodeError::BadField);
    }
    w.write_bits(candidates.len() as u64, 4).ok();
    for candidate in candidates {
        w.write_bits(u64::from(candidate.kind.code()), CandidateKind::BITS)
            .ok();
        write_address(w, candidate.address);
    }
    Ok(())
}

/// Reads a candidate list. A count over [`MAX_CANDIDATES`], an unnamed kind,
/// or a Seen candidate in a sender's own list (`from_sender`) is malformed.
pub fn read_list(
    r: &mut BitReader<'_>,
    from_sender: bool,
) -> Result<Vec<Candidate>, MasterDecodeError> {
    let count = r.read_bits(4)? as usize;
    if count > MAX_CANDIDATES {
        return Err(MasterDecodeError::Malformed);
    }
    let mut candidates = Vec::with_capacity(count);
    for _ in 0..count {
        let kind = CandidateKind::from_code(r.read_bits(CandidateKind::BITS)?)
            .ok_or(MasterDecodeError::Malformed)?;
        if from_sender && kind == CandidateKind::Seen {
            return Err(MasterDecodeError::Malformed);
        }
        candidates.push(Candidate::new(kind, read_address(r)?));
    }
    Ok(candidates)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(text: &str) -> SocketAddr {
        text.parse().unwrap()
    }

    #[test]
    fn addresses_round_trip_in_their_canonical_form() {
        for (given, sent, bits) in [
            ("203.0.113.5:26900", "203.0.113.5:26900", 49),
            ("[2001:db8::1]:26900", "[2001:db8::1]:26900", 145),
            ("[::ffff:203.0.113.5]:40000", "203.0.113.5:40000", 49),
            ("[fe80::1%7]:1", "[fe80::1]:1", 145),
        ] {
            let mut w = BitWriter::new();
            write_address(&mut w, addr(given));
            assert_eq!(w.bit_len(), bits);
            assert_eq!(address_bits(addr(given)), bits);
            let bytes = w.finish();
            let back = read_address(&mut BitReader::new(&bytes)).unwrap();
            assert_eq!(back, addr(sent));
            assert_eq!(canonical(addr(given)), addr(sent));
        }
        // The octets go in their usual order after the family bit.
        let mut w = BitWriter::new();
        write_address(&mut w, addr("1.2.3.4:5"));
        assert_eq!(
            w.finish(),
            vec![1 << 1, 2 << 1, 3 << 1, 4 << 1, 5 << 1, 0, 0]
        );
    }

    #[test]
    fn a_mapped_address_sent_as_ipv6_is_malformed() {
        let mut w = BitWriter::new();
        w.write_bool(true);
        for octet in "::ffff:1.2.3.4".parse::<Ipv6Addr>().unwrap().octets() {
            w.write_bits(u64::from(octet), 8).unwrap();
        }
        w.write_bits(80, 16).unwrap();
        let bytes = w.finish();
        assert_eq!(
            read_address(&mut BitReader::new(&bytes)),
            Err(MasterDecodeError::Malformed)
        );
    }

    #[test]
    fn lists_are_bounded_and_a_sender_never_lists_seen() {
        let local = Candidate::new(CandidateKind::Local, addr("192.168.1.20:26900"));
        let seen = Candidate::new(CandidateKind::Seen, addr("203.0.113.5:26900"));
        let v6 = Candidate::new(CandidateKind::GlobalIpv6, addr("[2001:db8::2]:26900"));
        let list = [seen, local, v6];
        let mut w = BitWriter::new();
        write_list(&mut w, &list, false).unwrap();
        assert_eq!(w.bit_len(), list_bits(&list));
        let bytes = w.finish();
        assert_eq!(read_list(&mut BitReader::new(&bytes), false).unwrap(), list);
        assert_eq!(
            read_list(&mut BitReader::new(&bytes), true),
            Err(MasterDecodeError::Malformed)
        );
        let mut w = BitWriter::new();
        assert_eq!(
            write_list(&mut w, &list, true),
            Err(MasterEncodeError::BadField)
        );
        assert_eq!(
            write_list(&mut w, &[local; MAX_CANDIDATES + 1], false),
            Err(MasterEncodeError::BadField)
        );
        // A count of 9 to 15 does not decode.
        let mut w = BitWriter::new();
        w.write_bits(9, 4).unwrap();
        let bytes = w.finish();
        assert_eq!(
            read_list(&mut BitReader::new(&bytes), false),
            Err(MasterDecodeError::Malformed)
        );
        // Kinds 4 to 7 are not named.
        let mut w = BitWriter::new();
        w.write_bits(1, 4).unwrap();
        w.write_bits(4, 3).unwrap();
        write_address(&mut w, addr("1.2.3.4:5"));
        let bytes = w.finish();
        assert_eq!(
            read_list(&mut BitReader::new(&bytes), false),
            Err(MasterDecodeError::Malformed)
        );
    }

    #[test]
    fn the_mapping_test_reads_the_two_answers() {
        let own = addr("192.168.1.20:26900");
        let out = addr("203.0.113.5:26900");
        let other_port = addr("203.0.113.5:31000");
        let other_ip = addr("203.0.113.6:26900");
        use MappingType::*;
        assert_eq!(
            MappingType::from_probes(Some(own), own, Some(own)),
            NoTranslation
        );
        assert_eq!(
            MappingType::from_probes(Some(own), out, Some(out)),
            SamePort
        );
        assert_eq!(MappingType::from_probes(None, out, Some(out)), SamePort);
        assert_eq!(
            MappingType::from_probes(Some(own), out, Some(other_port)),
            PortPerDestination
        );
        assert_eq!(
            MappingType::from_probes(Some(own), out, Some(other_ip)),
            PortPerDestination
        );
        assert_eq!(MappingType::from_probes(Some(own), out, None), Unknown);
        assert_eq!(
            MappingType::from_probes(Some(own), own, None),
            NoTranslation
        );
    }

    #[test]
    fn the_relay_is_likely_only_for_a_symmetric_router_with_no_other_way_in() {
        let local = Candidate::new(CandidateKind::Local, addr("192.168.1.20:26900"));
        let mapped = Candidate::new(CandidateKind::Mapped, addr("203.0.113.5:26900"));
        let v6 = Candidate::new(CandidateKind::GlobalIpv6, addr("[2001:db8::2]:26900"));
        assert!(relay_likely(MappingType::PortPerDestination, &[local]));
        assert!(!relay_likely(
            MappingType::PortPerDestination,
            &[local, mapped]
        ));
        assert!(!relay_likely(MappingType::PortPerDestination, &[v6]));
        assert!(!relay_likely(MappingType::SamePort, &[local]));
        assert!(!relay_likely(MappingType::Unknown, &[]));
    }

    #[test]
    fn codes_are_strict() {
        for kind in CandidateKind::ALL {
            assert_eq!(
                CandidateKind::from_code(u64::from(kind.code())),
                Some(*kind)
            );
        }
        assert_eq!(CandidateKind::from_code(4), None);
        assert_eq!(MappingType::ALL.len(), 4);
        assert_eq!(MappingType::from_code(4), None);
    }
}
