//! Finding a UPnP Internet Gateway Device: an SSDP search (`M-SEARCH`) to
//! the multicast group and the answers' `LOCATION`, where the device's
//! description is.

use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4};

/// Where SSDP searches go: the multicast group 239.255.255.250, port 1900.
pub const SSDP_ADDRESS: SocketAddr =
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(239, 255, 255, 250), 1900));

/// The devices searched for. A device of version 2 answers a search for
/// version 1 too, but some answer only their own, so both are asked.
pub(super) const SEARCH_TARGETS: [&str; 2] = [
    "urn:schemas-upnp-org:device:InternetGatewayDevice:1",
    "urn:schemas-upnp-org:device:InternetGatewayDevice:2",
];

/// The longest answer read.
pub(super) const MAX_ANSWER: usize = 2048;

/// A search for `target` (`ST`), sent to `to`. Devices answer within a
/// second (`MX: 1`), well inside the two seconds the search waits (agent
/// decision).
pub(super) fn search_request(to: SocketAddr, target: &str) -> String {
    format!(
        "M-SEARCH * HTTP/1.1\r\nHOST: {to}\r\nMAN: \"ssdp:discover\"\r\nMX: 1\r\nST: {target}\r\n\r\n"
    )
}

/// What an answer says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Answer {
    /// Where the device's description is.
    pub location: String,
    /// What answered (`ST`).
    pub target: String,
}

/// Reads an answer to a search: `HTTP/1.1 200`, with a `LOCATION` and an
/// `ST` naming a gateway or one of its WAN connection services. Anything
/// else is `None`.
pub(super) fn parse_answer(bytes: &[u8]) -> Option<Answer> {
    if bytes.len() > MAX_ANSWER {
        return None;
    }
    let text = std::str::from_utf8(bytes).ok()?;
    let mut lines = text.split('\n').map(|line| line.trim_end_matches('\r'));
    let status = lines.next()?;
    let mut parts = status.split(' ');
    if !matches!(parts.next(), Some("HTTP/1.1" | "HTTP/1.0")) || parts.next() != Some("200") {
        return None;
    }
    let mut location = None;
    let mut target = None;
    for line in lines {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        let value = value.trim();
        if name.trim().eq_ignore_ascii_case("location") {
            location = Some(value);
        } else if name.trim().eq_ignore_ascii_case("st") {
            target = Some(value);
        }
    }
    let (location, target) = (location?, target?);
    let gateway = [
        "InternetGatewayDevice:",
        "WANIPConnection:",
        "WANPPPConnection:",
    ]
    .iter()
    .any(|kind| target.contains(kind));
    (gateway && !location.is_empty() && location.len() <= 512).then(|| Answer {
        location: location.to_owned(),
        target: target.to_owned(),
    })
}

/// A fake device's answer to a search, or `None` for anything that is not
/// one.
pub(super) fn fake_answer(request: &[u8], location: &str) -> Option<String> {
    let text = std::str::from_utf8(request).ok()?;
    if !text.starts_with("M-SEARCH * HTTP/1.1") {
        return None;
    }
    let target = text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("st")
            .then(|| value.trim().to_owned())
    })?;
    Some(format!(
        "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\nST: {target}\r\nUSN: uuid:fake-gateway::{target}\r\nEXT:\r\nSERVER: Fake/1.0 UPnP/1.1 FakeGateway/1.0\r\nLOCATION: {location}\r\n\r\n"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitMix64;

    #[test]
    fn a_search_and_its_answer() {
        let request = search_request(SSDP_ADDRESS, SEARCH_TARGETS[0]);
        assert!(request.starts_with("M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\n"));
        assert!(
            request.ends_with("ST: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\n\r\n")
        );
        let answer = fake_answer(request.as_bytes(), "http://10.0.0.1:5000/rootDesc.xml").unwrap();
        assert_eq!(
            parse_answer(answer.as_bytes()),
            Some(Answer {
                location: "http://10.0.0.1:5000/rootDesc.xml".into(),
                target: SEARCH_TARGETS[0].into()
            })
        );
    }

    #[test]
    fn other_answers_are_ignored() {
        let printer = "HTTP/1.1 200 OK\r\nST: urn:schemas-upnp-org:device:Printer:1\r\nLOCATION: http://10.0.0.9/\r\n\r\n";
        assert_eq!(parse_answer(printer.as_bytes()), None);
        let notify = "NOTIFY * HTTP/1.1\r\nNT: urn:schemas-upnp-org:device:InternetGatewayDevice:1\r\nLOCATION: http://10.0.0.1/\r\n\r\n";
        assert_eq!(parse_answer(notify.as_bytes()), None);
        let no_location =
            "HTTP/1.1 200 OK\r\nST: urn:schemas-upnp-org:service:WANIPConnection:1\r\n\r\n";
        assert_eq!(parse_answer(no_location.as_bytes()), None);
        let lower = "HTTP/1.1 200 OK\nst: urn:schemas-upnp-org:service:WANIPConnection:1\nlocation: http://10.0.0.1/x\n\n";
        assert_eq!(
            parse_answer(lower.as_bytes()).unwrap().location,
            "http://10.0.0.1/x"
        );
        assert_eq!(fake_answer(b"GET / HTTP/1.1\r\n\r\n", "x"), None);
    }

    #[test]
    fn fuzzed_answers_never_panic() {
        let request = search_request(SSDP_ADDRESS, SEARCH_TARGETS[1]);
        let answer = fake_answer(request.as_bytes(), "http://10.0.0.1:5000/rootDesc.xml").unwrap();
        let mut rng = SplitMix64::new(3);
        for _ in 0..100_000 {
            let mut bytes = answer.as_bytes().to_vec();
            for _ in 0..1 + rng.below(6) {
                if bytes.is_empty() {
                    break;
                }
                let at = rng.below(bytes.len() as u64) as usize;
                match rng.below(3) {
                    0 => bytes[at] = rng.next_u64() as u8,
                    1 => {
                        bytes.remove(at);
                    }
                    _ => bytes.truncate(at),
                }
            }
            let _ = parse_answer(&bytes);
            let _ = fake_answer(&bytes, "x");
        }
    }
}
