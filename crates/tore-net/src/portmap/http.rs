//! Just enough HTTP/1.1 for a router's UPnP device: a GET for its
//! description and a POST for each SOAP action, over `std::net::TcpStream`.
//!
//! Every request closes its connection, every read has a timeout and checks
//! the caller's stop flag at least every 50 ms, a head is at most 16 KB and a
//! body at most 64 KB, and the response reader takes `Content-Length`,
//! chunked bodies and bodies that end with the connection. Addresses are
//! literal: a router's answers name it by address, so no name is looked up.

use super::SLICE;
use std::io::{self, Read, Write};
use std::net::{IpAddr, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// The longest response head accepted.
pub(super) const MAX_HEAD: usize = 16 * 1024;
/// The longest response body accepted.
pub(super) const MAX_BODY: usize = 64 * 1024;
/// The most bytes read for one response (a chunked body's framing included).
const MAX_READ: usize = MAX_HEAD + 2 * MAX_BODY;
/// The most header lines in one head.
const MAX_HEADERS: usize = 100;

/// An `http://` address with a literal host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Url {
    pub host: IpAddr,
    pub port: u16,
    /// Starts with `/`; printable ASCII without spaces.
    pub path: String,
}

impl Url {
    /// Reads `http://HOST[:PORT][/PATH]`, where HOST is an IPv4 address or a
    /// bracketed IPv6 one.
    pub(super) fn parse(text: &str) -> Option<Url> {
        let text = text.trim();
        let scheme = text.get(..7)?;
        if !scheme.eq_ignore_ascii_case("http://") {
            return None;
        }
        let rest = &text[7..];
        let (authority, path) = match rest.find('/') {
            Some(at) => (&rest[..at], &rest[at..]),
            None => (rest, "/"),
        };
        let (host, port) = if let Some(inside) = authority.strip_prefix('[') {
            let (host, tail) = inside.split_once(']')?;
            let host: IpAddr = IpAddr::V6(host.parse().ok()?);
            match tail {
                "" => (host, 80),
                tail => (host, tail.strip_prefix(':')?.parse().ok()?),
            }
        } else {
            match authority.split_once(':') {
                Some((host, port)) => (IpAddr::V4(host.parse().ok()?), port.parse().ok()?),
                None => (IpAddr::V4(authority.parse().ok()?), 80),
            }
        };
        if port == 0 || !clean_path(path) {
            return None;
        }
        Some(Url {
            host,
            port,
            path: path.to_owned(),
        })
    }

    /// `HOST:PORT` for the Host header.
    pub(super) fn authority(&self) -> String {
        self.address().to_string()
    }

    /// Where to connect.
    pub(super) fn address(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }

    /// A reference from a description resolved against this address: a whole
    /// address on the same host, an absolute path, or a path relative to
    /// this one's folder. A whole address on another host is refused (agent
    /// decision: a device only ever controls itself).
    pub(super) fn join(&self, reference: &str) -> Option<Url> {
        let reference = reference.trim();
        if reference.len() > 512 {
            return None;
        }
        if reference
            .get(..7)
            .is_some_and(|s| s.eq_ignore_ascii_case("http://"))
        {
            return Url::parse(reference).filter(|url| url.host == self.host);
        }
        let path = if reference.starts_with('/') {
            reference.to_owned()
        } else {
            let folder = &self.path[..self.path.rfind('/').map_or(0, |at| at + 1)];
            let folder = if folder.is_empty() { "/" } else { folder };
            format!("{folder}{reference}")
        };
        clean_path(&path).then(|| Url {
            path,
            ..self.clone()
        })
    }
}

fn clean_path(path: &str) -> bool {
    path.starts_with('/') && path.len() <= 512 && path.bytes().all(|b| b.is_ascii_graphic())
}

/// A response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl Response {
    /// The first header called `name`, ignoring case.
    pub(super) fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }
}

/// A response and the address this side's connection came from.
#[derive(Clone, Debug)]
pub(super) struct Exchange {
    pub response: Response,
    pub local: SocketAddr,
}

/// What [`parse_response`] made of the bytes so far.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Parsed {
    Complete(Response),
    /// More bytes are needed.
    Partial,
}

/// Why a request failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum HttpError {
    /// The connection could not be made.
    Connect(io::ErrorKind),
    /// Reading or writing failed.
    Io(io::ErrorKind),
    /// The time ran out.
    TimedOut,
    /// The caller's stop flag was set.
    Stopped,
    /// A head, body or chunk over the limits.
    TooLarge,
    /// Not an HTTP response.
    Malformed,
    /// The connection closed before the response was whole.
    Closed,
}

impl std::fmt::Display for HttpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Connect(kind) => write!(f, "could not connect ({kind})"),
            Self::Io(kind) => write!(f, "the connection failed ({kind})"),
            Self::TimedOut => f.write_str("no answer in time"),
            Self::Stopped => f.write_str("stopped"),
            Self::TooLarge => f.write_str("the answer was too long"),
            Self::Malformed => f.write_str("the answer was not HTTP"),
            Self::Closed => f.write_str("the connection closed early"),
        }
    }
}

/// Reads a response from the bytes received so far. `eof` says the
/// connection has closed, which ends a body that has no length. Informational
/// (1xx) heads are skipped.
pub(super) fn parse_response(buf: &[u8], eof: bool) -> Result<Parsed, HttpError> {
    let mut start = 0;
    loop {
        let bytes = &buf[start..];
        let Some(head_len) = head_length(bytes) else {
            return if bytes.len() > MAX_HEAD {
                Err(HttpError::TooLarge)
            } else if eof {
                Err(HttpError::Closed)
            } else {
                Ok(Parsed::Partial)
            };
        };
        if head_len > MAX_HEAD {
            return Err(HttpError::TooLarge);
        }
        let head = std::str::from_utf8(&bytes[..head_len]).map_err(|_| HttpError::Malformed)?;
        let mut lines = head.split('\n').map(|line| line.trim_end_matches('\r'));
        let status_line = lines.next().ok_or(HttpError::Malformed)?;
        let mut parts = status_line.splitn(3, ' ');
        let version = parts.next().unwrap_or("");
        if !version.starts_with("HTTP/1.") {
            return Err(HttpError::Malformed);
        }
        let status: u16 = parts
            .next()
            .filter(|code| code.len() == 3)
            .and_then(|code| code.parse().ok())
            .filter(|code| (100..600).contains(code))
            .ok_or(HttpError::Malformed)?;
        let mut headers = Vec::new();
        for line in lines.filter(|line| !line.is_empty()) {
            let (name, value) = line.split_once(':').ok_or(HttpError::Malformed)?;
            if headers.len() >= MAX_HEADERS {
                return Err(HttpError::TooLarge);
            }
            headers.push((name.trim().to_owned(), value.trim().to_owned()));
        }
        if status < 200 {
            start += head_len;
            continue;
        }
        let response = Response {
            status,
            headers,
            body: Vec::new(),
        };
        let body = &bytes[head_len..];
        let chunked = response
            .header("Transfer-Encoding")
            .is_some_and(|value| value.to_ascii_lowercase().contains("chunked"));
        let whole = if chunked {
            dechunk(body)?
        } else if let Some(length) = response.header("Content-Length") {
            let length: usize = length.parse().map_err(|_| HttpError::Malformed)?;
            if length > MAX_BODY {
                return Err(HttpError::TooLarge);
            }
            (body.len() >= length).then(|| body[..length].to_vec())
        } else if body.len() > MAX_BODY {
            return Err(HttpError::TooLarge);
        } else {
            eof.then(|| body.to_vec())
        };
        return match whole {
            Some(body) => Ok(Parsed::Complete(Response { body, ..response })),
            None if eof => Err(HttpError::Closed),
            None => Ok(Parsed::Partial),
        };
    }
}

/// The length of the head, its blank line included, once it has arrived.
/// Bare line feeds are accepted as well as CR LF.
fn head_length(bytes: &[u8]) -> Option<usize> {
    let scan = &bytes[..bytes.len().min(MAX_HEAD + 4)];
    for (i, byte) in scan.iter().enumerate() {
        if *byte != b'\n' {
            continue;
        }
        if scan.get(i + 1) == Some(&b'\n') {
            return Some(i + 2);
        }
        if scan.get(i + 1) == Some(&b'\r') && scan.get(i + 2) == Some(&b'\n') {
            return Some(i + 3);
        }
    }
    None
}

/// A chunked body, once its last chunk and trailers have arrived.
fn dechunk(bytes: &[u8]) -> Result<Option<Vec<u8>>, HttpError> {
    let mut body = Vec::new();
    let mut at = 0;
    loop {
        let Some(line) = line_at(bytes, at)? else {
            return Ok(None);
        };
        at = line.1;
        let size_text = line.0.split(';').next().unwrap_or("").trim();
        if size_text.is_empty() || size_text.len() > 8 {
            return Err(HttpError::Malformed);
        }
        let size = usize::from_str_radix(size_text, 16).map_err(|_| HttpError::Malformed)?;
        if size == 0 {
            for _ in 0..MAX_HEADERS {
                let Some((trailer, next)) = line_at(bytes, at)? else {
                    return Ok(None);
                };
                at = next;
                if trailer.trim().is_empty() {
                    return Ok(Some(body));
                }
            }
            return Err(HttpError::TooLarge);
        }
        if body.len() + size > MAX_BODY {
            return Err(HttpError::TooLarge);
        }
        let Some(data) = bytes.get(at..at + size) else {
            return Ok(None);
        };
        body.extend_from_slice(data);
        at += size;
        match bytes.get(at..) {
            Some([b'\r', b'\n', ..]) => at += 2,
            Some([b'\n', ..]) => at += 1,
            Some([] | [b'\r']) | None => return Ok(None),
            Some(_) => return Err(HttpError::Malformed),
        }
    }
}

/// The line starting at `at` (without its end) and where the next begins.
fn line_at(bytes: &[u8], at: usize) -> Result<Option<(&str, usize)>, HttpError> {
    let rest = bytes.get(at..).unwrap_or(&[]);
    let Some(end) = rest.iter().take(1024).position(|b| *b == b'\n') else {
        return if rest.len() >= 1024 {
            Err(HttpError::Malformed)
        } else {
            Ok(None)
        };
    };
    let line = std::str::from_utf8(&rest[..end]).map_err(|_| HttpError::Malformed)?;
    Ok(Some((line.trim_end_matches('\r'), at + end + 1)))
}

/// Sends one request and reads its response, ending by `until` or soon after
/// `stop` is set.
pub(super) fn request(
    url: &Url,
    method: &str,
    headers: &[(&str, &str)],
    body: &[u8],
    until: Instant,
    stop: &AtomicBool,
) -> Result<Exchange, HttpError> {
    let left = || {
        until
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
    };
    let mut stream = TcpStream::connect_timeout(&url.address(), left().ok_or(HttpError::TimedOut)?)
        .map_err(|error| match error.kind() {
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => HttpError::TimedOut,
            kind => HttpError::Connect(kind),
        })?;
    let local = stream
        .local_addr()
        .map_err(|error| HttpError::Io(error.kind()))?;
    let mut out = format!(
        "{method} {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nUser-Agent: {}\r\n",
        url.path,
        url.authority(),
        super::AGENT
    );
    for (name, value) in headers {
        out.push_str(&format!("{name}: {value}\r\n"));
    }
    if method != "GET" || !body.is_empty() {
        out.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    out.push_str("\r\n");
    let mut bytes = out.into_bytes();
    bytes.extend_from_slice(body);
    let io = |error: io::Error| match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => HttpError::TimedOut,
        kind => HttpError::Io(kind),
    };
    stream
        .set_write_timeout(Some(left().ok_or(HttpError::TimedOut)?))
        .map_err(io)?;
    stream.write_all(&bytes).map_err(io)?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if stop.load(Ordering::Acquire) {
            return Err(HttpError::Stopped);
        }
        let wait = left().ok_or(HttpError::TimedOut)?.min(SLICE);
        stream.set_read_timeout(Some(wait)).map_err(io)?;
        match stream.read(&mut chunk) {
            Ok(0) => {
                return match parse_response(&buf, true)? {
                    Parsed::Complete(response) => Ok(Exchange { response, local }),
                    Parsed::Partial => Err(HttpError::Closed),
                };
            }
            Ok(n) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.len() > MAX_READ {
                    return Err(HttpError::TooLarge);
                }
                if let Parsed::Complete(response) = parse_response(&buf, false)? {
                    return Ok(Exchange { response, local });
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(error) => return Err(HttpError::Io(error.kind())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitMix64;
    use std::net::{Ipv4Addr, TcpListener};
    use std::time::Duration;

    fn complete(bytes: &[u8]) -> Response {
        match parse_response(bytes, false) {
            Ok(Parsed::Complete(response)) => response,
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn urls() {
        let url = Url::parse("http://192.168.1.1:5000/rootDesc.xml").unwrap();
        assert_eq!(url.host, IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)));
        assert_eq!(url.port, 5000);
        assert_eq!(url.path, "/rootDesc.xml");
        assert_eq!(url.authority(), "192.168.1.1:5000");
        assert_eq!(Url::parse("HTTP://10.0.0.1").unwrap().path, "/");
        assert_eq!(Url::parse("http://10.0.0.1").unwrap().port, 80);
        let v6 = Url::parse("http://[fe80::1]:49000/igd.xml").unwrap();
        assert_eq!(v6.authority(), "[fe80::1]:49000");
        for bad in [
            "https://10.0.0.1/",
            "http://router.local/",
            "http://10.0.0.1:0/",
            "http://10.0.0.1:99999/",
            "http://10.0.0.1/a b",
            "http://10.0.0.1/a\r\nX: y",
            "http://[fe80::1/",
            "ftp://10.0.0.1/",
            "",
        ] {
            assert_eq!(Url::parse(bad), None, "{bad}");
        }
    }

    #[test]
    fn joins() {
        let base = Url::parse("http://10.0.0.1:5000/dev/desc.xml").unwrap();
        assert_eq!(base.join("/ctl/IPConn").unwrap().path, "/ctl/IPConn");
        assert_eq!(base.join("ctl/IPConn").unwrap().path, "/dev/ctl/IPConn");
        assert_eq!(base.join("http://10.0.0.1:6000/x").unwrap().port, 6000);
        assert_eq!(base.join("http://10.0.0.2:5000/x"), None);
        assert_eq!(base.join("bad path"), None);
        let root = Url::parse("http://10.0.0.1:5000").unwrap();
        assert_eq!(root.join("ctl").unwrap().path, "/ctl");
    }

    #[test]
    fn lengths_chunks_and_eof() {
        let response = complete(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhelloEXTRA");
        assert_eq!(response.status, 200);
        assert_eq!(response.body, b"hello");
        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhel", false),
            Ok(Parsed::Partial)
        );
        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhel", true),
            Err(HttpError::Closed)
        );

        let chunked = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4;ext=1\r\nWiki\r\n5\r\npedia\r\nE\r\n in\r\n\r\nchunks.\r\n0\r\nX-Trailer: y\r\n\r\n";
        assert_eq!(complete(chunked).body, b"Wikipedia in\r\n\r\nchunks.");
        for cut in 0..chunked.len() - 1 {
            assert_eq!(
                parse_response(&chunked[..cut], false),
                Ok(Parsed::Partial),
                "cut at {cut}"
            );
        }
        let bare = b"HTTP/1.0 200 OK\nTransfer-Encoding: chunked\n\n3\nabc\n0\n\n";
        assert_eq!(complete(bare).body, b"abc");

        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\n\r\nto the end", false),
            Ok(Parsed::Partial)
        );
        match parse_response(b"HTTP/1.1 200 OK\r\n\r\nto the end", true) {
            Ok(Parsed::Complete(response)) => assert_eq!(response.body, b"to the end"),
            other => panic!("{other:?}"),
        }
        let continued = complete(b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.1 500 Internal Server Error\r\nContent-Length: 2\r\n\r\nno");
        assert_eq!(continued.status, 500);
        assert_eq!(continued.body, b"no");
    }

    #[test]
    fn limits_and_garbage() {
        let big = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
            MAX_BODY + 1
        );
        assert_eq!(
            parse_response(big.as_bytes(), false),
            Err(HttpError::TooLarge)
        );
        let long_head = format!("HTTP/1.1 200 OK\r\nX: {}", "a".repeat(MAX_HEAD + 10));
        assert_eq!(
            parse_response(long_head.as_bytes(), false),
            Err(HttpError::TooLarge)
        );
        let huge_chunk = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nFFFFF\r\n";
        assert_eq!(parse_response(huge_chunk, false), Err(HttpError::TooLarge));
        let bad_chunk = b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\nzz\r\n";
        assert_eq!(parse_response(bad_chunk, false), Err(HttpError::Malformed));
        assert_eq!(
            parse_response(b"SSH-2.0-OpenSSH\r\n\r\n", false),
            Err(HttpError::Malformed)
        );
        assert_eq!(
            parse_response(b"HTTP/1.1 2000 OK\r\n\r\n", false),
            Err(HttpError::Malformed)
        );
        assert_eq!(
            parse_response(b"HTTP/1.1 200 OK\r\nno colon\r\n\r\n", false),
            Err(HttpError::Malformed)
        );
    }

    #[test]
    fn fuzzed_responses_never_panic() {
        let samples: [&[u8]; 3] = [
            b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello",
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4;x\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n",
            b"HTTP/1.1 100 Continue\r\n\r\nHTTP/1.0 500 Oops\nContent-Length: 3\n\nabc",
        ];
        let mut rng = SplitMix64::new(9);
        for round in 0..50_000 {
            let mut bytes = samples[round % samples.len()].to_vec();
            if round % 5 == 0 {
                bytes = (0..rng.below(120))
                    .map(|_| b"HTP/1. 0\r\n:;aF"[rng.below(14) as usize])
                    .collect();
            }
            for _ in 0..rng.below(6) {
                if bytes.is_empty() {
                    break;
                }
                let at = rng.below(bytes.len() as u64) as usize;
                match rng.below(4) {
                    0 => bytes[at] = rng.next_u64() as u8,
                    1 => {
                        bytes.remove(at);
                    }
                    2 => bytes.insert(at, b"\r\n0F;:"[rng.below(6) as usize]),
                    _ => bytes.truncate(at),
                }
            }
            let _ = parse_response(&bytes, round % 2 == 0);
            let _ = Url::parse(&String::from_utf8_lossy(&bytes));
        }
    }

    /// A server that takes one connection and answers it with `answer`, or
    /// never when it is `None`.
    fn one_shot(answer: Option<&'static [u8]>) -> (Url, std::thread::JoinHandle<Vec<u8>>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let url = Url::parse(&format!(
            "http://{}/desc.xml",
            listener.local_addr().unwrap()
        ))
        .unwrap();
        let thread = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(200)))
                .unwrap();
            let mut got = vec![0u8; 4096];
            let n = stream.read(&mut got).unwrap_or(0);
            got.truncate(n);
            match answer {
                Some(answer) => {
                    for piece in answer.chunks(7) {
                        stream.write_all(piece).unwrap();
                        std::thread::sleep(Duration::from_millis(1));
                    }
                }
                None => std::thread::sleep(Duration::from_millis(1500)),
            }
            got
        });
        (url, thread)
    }

    #[test]
    fn a_request_on_loopback() {
        let (url, server) = one_shot(Some(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nhi\r\n0\r\n\r\n",
        ));
        let stop = AtomicBool::new(false);
        let until = Instant::now() + Duration::from_secs(2);
        let exchange = request(
            &url,
            "POST",
            &[("SOAPAction", "\"x#y\"")],
            b"<a/>",
            until,
            &stop,
        )
        .unwrap();
        assert_eq!(exchange.response.body, b"hi");
        assert_eq!(exchange.local.ip(), IpAddr::V4(Ipv4Addr::LOCALHOST));
        let sent = String::from_utf8(server.join().unwrap()).unwrap();
        assert!(sent.starts_with("POST /desc.xml HTTP/1.1\r\n"), "{sent}");
        assert!(sent.contains(&format!("Host: {}\r\n", url.authority())));
        assert!(sent.contains("SOAPAction: \"x#y\"\r\nContent-Length: 4\r\n\r\n<a/>"));
    }

    #[test]
    fn a_silent_server_ends_on_time_and_on_stop() {
        let (url, server) = one_shot(None);
        let stop = AtomicBool::new(false);
        let started = Instant::now();
        let until = started + Duration::from_millis(400);
        assert_eq!(
            request(&url, "GET", &[], &[], until, &stop).unwrap_err(),
            HttpError::TimedOut
        );
        let took = started.elapsed();
        assert!(
            took >= Duration::from_millis(390) && took < Duration::from_millis(800),
            "{took:?}"
        );
        server.join().unwrap();

        let (url, server) = one_shot(None);
        let stop = AtomicBool::new(true);
        let started = Instant::now();
        let until = started + Duration::from_secs(5);
        assert_eq!(
            request(&url, "GET", &[], &[], until, &stop).unwrap_err(),
            HttpError::Stopped
        );
        assert!(started.elapsed() < Duration::from_millis(500));
        server.join().unwrap();
    }
}
