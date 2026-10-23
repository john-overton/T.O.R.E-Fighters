//! A UPnP Internet Gateway Device (IGD versions 1 and 2): its description's
//! WAN connection services, and the SOAP actions `GetExternalIPAddress`,
//! `AddPortMapping` and `DeletePortMapping` on one of them. This file also
//! holds the fake device the tests (and the game's tests) map against on
//! loopback: an SSDP responder and an HTTP device.

use super::http::{self, HttpError, Url};
use super::{ssdp, xml};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

const WAN_IP: &str = "urn:schemas-upnp-org:service:WANIPConnection:";
const WAN_PPP: &str = "urn:schemas-upnp-org:service:WANPPPConnection:";

/// UPnP error codes this side acts on.
#[cfg(test)]
pub(super) const NOT_AUTHORIZED: u16 = 606;
pub(super) const CONFLICT: u16 = 718;
pub(super) const SAME_PORTS_REQUIRED: u16 = 724;
pub(super) const ONLY_PERMANENT_LEASES: u16 = 725;

/// A WAN connection service and where to send its actions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Service {
    /// Its `serviceType`, for example `urn:schemas-upnp-org:service:WANIPConnection:2`.
    pub kind: String,
    pub control: Url,
}

/// The WAN connection services a device description offers, best first:
/// `WANIPConnection` of the highest version, then `WANPPPConnection`.
/// Control addresses resolve against `URLBase` when there is one, else the
/// description's own address, and must stay on the device's host.
pub(super) fn services(description: &str, location: &Url) -> Vec<Service> {
    let Ok(events) = xml::parse(description) else {
        return Vec::new();
    };
    let base = xml::first_text(&events, "URLBase")
        .filter(|text| !text.is_empty())
        .and_then(|text| location.join(text))
        .unwrap_or_else(|| location.clone());
    let mut found: Vec<(u8, Service)> = Vec::new();
    for inside in xml::each(&events, "service") {
        let (Some(kind), Some(control)) = (
            xml::first_text(inside, "serviceType"),
            xml::first_text(inside, "controlURL"),
        ) else {
            continue;
        };
        let rank = if let Some(version) = kind.strip_prefix(WAN_IP) {
            version.parse::<u8>().map_or(0, |v| v.min(9)) + 10
        } else if kind.starts_with(WAN_PPP) {
            1
        } else {
            continue;
        };
        let Some(control) = base.join(control).filter(|_| !control.is_empty()) else {
            continue;
        };
        found.push((
            rank,
            Service {
                kind: kind.to_owned(),
                control,
            },
        ));
    }
    // A stable sort keeps the document's order among equals.
    found.sort_by(|a, b| b.0.cmp(&a.0));
    found
        .into_iter()
        .map(|(_, service)| service)
        .take(8)
        .collect()
}

/// A SOAP request's body.
pub(super) fn envelope(kind: &str, action: &str, args: &[(&str, String)]) -> String {
    let mut body = format!(
        "<?xml version=\"1.0\"?>\r\n<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action} xmlns:u=\"{}\">",
        xml::escape(kind)
    );
    for (name, value) in args {
        body.push_str(&format!("<{name}>{}</{name}>", xml::escape(value)));
    }
    body.push_str(&format!("</u:{action}></s:Body></s:Envelope>\r\n"));
    body
}

/// Why an action failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum SoapError {
    /// The device's UPnP error code and description.
    Upnp {
        code: u16,
        description: String,
    },
    Http(HttpError),
    /// An HTTP status other than 200 with no UPnP error in it.
    Status(u16),
    /// An answer that is not the XML it should be.
    Malformed,
}

impl std::fmt::Display for SoapError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Upnp { code, description } if description.is_empty() => write!(f, "error {code}"),
            Self::Upnp { code, description } => write!(f, "error {code}, {description}"),
            Self::Http(error) => error.fmt(f),
            Self::Status(status) => write!(f, "HTTP status {status}"),
            Self::Malformed => f.write_str("an answer that could not be read"),
        }
    }
}

/// Calls `action` with `args` and returns the answer's events and the
/// address this side's connection came from.
fn call(
    service: &Service,
    action: &str,
    args: &[(&str, String)],
    until: Instant,
    stop: &AtomicBool,
) -> Result<(Vec<xml::Event>, SocketAddr), SoapError> {
    let body = envelope(&service.kind, action, args);
    let soap_action = format!("\"{}#{action}\"", service.kind);
    let exchange = http::request(
        &service.control,
        "POST",
        &[
            ("Content-Type", "text/xml; charset=\"utf-8\""),
            ("SOAPAction", &soap_action),
        ],
        body.as_bytes(),
        until,
        stop,
    )
    .map_err(SoapError::Http)?;
    let status = exchange.response.status;
    let events = xml::parse(&String::from_utf8_lossy(&exchange.response.body));
    if status == 200 {
        return events
            .map(|events| (events, exchange.local))
            .map_err(|_| SoapError::Malformed);
    }
    let code = events
        .as_ref()
        .ok()
        .and_then(|events| Some((xml::first_text(events, "errorCode")?.parse().ok()?, events)));
    match code {
        Some((code, events)) => Err(SoapError::Upnp {
            code,
            description: xml::first_text(events, "errorDescription")
                .unwrap_or("")
                .chars()
                .take(80)
                .collect(),
        }),
        None => Err(SoapError::Status(status)),
    }
}

/// The device's outside address and the address this side reached it from.
/// An empty answer (a device not yet connected) reads as 0.0.0.0.
pub(super) fn external_address(
    service: &Service,
    until: Instant,
    stop: &AtomicBool,
) -> Result<(Ipv4Addr, SocketAddr), SoapError> {
    let (events, local) = call(service, "GetExternalIPAddress", &[], until, stop)?;
    let text = xml::first_text(&events, "NewExternalIPAddress").ok_or(SoapError::Malformed)?;
    if text.is_empty() {
        return Ok((Ipv4Addr::UNSPECIFIED, local));
    }
    let address = text.parse().map_err(|_| SoapError::Malformed)?;
    Ok((address, local))
}

/// One `AddPortMapping`.
#[derive(Clone, Debug)]
pub(super) struct AddMapping<'a> {
    pub external_port: u16,
    pub internal_port: u16,
    pub client: Ipv4Addr,
    /// Seconds; 0 asks for a mapping with no end.
    pub lease: u32,
    pub description: &'a str,
}

/// Maps UDP `external_port` on the device to `client`'s `internal_port`.
pub(super) fn add_port_mapping(
    service: &Service,
    mapping: &AddMapping<'_>,
    until: Instant,
    stop: &AtomicBool,
) -> Result<(), SoapError> {
    let args = [
        ("NewRemoteHost", String::new()),
        ("NewExternalPort", mapping.external_port.to_string()),
        ("NewProtocol", "UDP".to_owned()),
        ("NewInternalPort", mapping.internal_port.to_string()),
        ("NewInternalClient", mapping.client.to_string()),
        ("NewEnabled", "1".to_owned()),
        ("NewPortMappingDescription", mapping.description.to_owned()),
        ("NewLeaseDuration", mapping.lease.to_string()),
    ];
    call(service, "AddPortMapping", &args, until, stop).map(|_| ())
}

/// Removes the UDP mapping on `external_port`.
pub(super) fn delete_port_mapping(
    service: &Service,
    external_port: u16,
    until: Instant,
    stop: &AtomicBool,
) -> Result<(), SoapError> {
    let args = [
        ("NewRemoteHost", String::new()),
        ("NewExternalPort", external_port.to_string()),
        ("NewProtocol", "UDP".to_owned()),
    ];
    call(service, "DeletePortMapping", &args, until, stop).map(|_| ())
}

// ---------------------------------------------------------------------------
// The fake device
// ---------------------------------------------------------------------------

/// How a [`FakeUpnp`] behaves.
#[derive(Clone, Debug)]
pub struct FakeUpnpConfig {
    /// The IGD version, 1 or 2. Version 1 gives a `URLBase` and relative
    /// control addresses; version 2 absolute paths and a `WANPPPConnection`
    /// listed before its `WANIPConnection:2`.
    pub version: u8,
    /// Offer only `WANPPPConnection:1`.
    pub ppp_only: bool,
    /// Send every answer chunked.
    pub chunked: bool,
    /// The outside address it reports.
    pub outside: Ipv4Addr,
    /// Outside ports already taken by another machine (error 718).
    pub taken: Vec<u16>,
    /// Takes only a lease of 0 (error 725 otherwise).
    pub permanent_only: bool,
    /// After a conflict, refuses an outside port other than the inside one
    /// (error 724).
    pub same_ports_only: bool,
    /// Refuses every `AddPortMapping` with this code.
    pub refuse: Option<u16>,
    /// Never answers the search.
    pub silent_ssdp: bool,
    /// Reads every HTTP request and never answers it.
    pub silent_http: bool,
}

impl Default for FakeUpnpConfig {
    fn default() -> Self {
        Self {
            version: 2,
            ppp_only: false,
            chunked: false,
            outside: Ipv4Addr::new(203, 0, 113, 5),
            taken: Vec::new(),
            permanent_only: false,
            same_ports_only: false,
            refuse: None,
            silent_ssdp: false,
            silent_http: false,
        }
    }
}

/// A mapping a [`FakeUpnp`] holds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FakeUpnpMapping {
    pub external_port: u16,
    pub internal_port: u16,
    pub client: String,
    /// Seconds; 0 for no end.
    pub lease: u32,
    pub description: String,
}

#[derive(Debug)]
struct UpnpState {
    config: FakeUpnpConfig,
    mappings: Vec<FakeUpnpMapping>,
    searches: usize,
    actions: Vec<String>,
}

/// A UPnP gateway on loopback for tests: an SSDP responder (searched by
/// unicast, at [`FakeUpnp::ssdp_address`]) and the HTTP device it points to.
/// It never touches a real router.
pub struct FakeUpnp {
    ssdp: SocketAddr,
    location: String,
    state: Arc<Mutex<UpnpState>>,
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl FakeUpnp {
    /// Starts the responder and the device on 127.0.0.1.
    pub fn start(config: FakeUpnpConfig) -> std::io::Result<Self> {
        let ssdp_socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))?;
        ssdp_socket.set_read_timeout(Some(Duration::from_millis(10)))?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let http = listener.local_addr()?;
        let location = format!("http://{http}/rootDesc.xml");
        let state = Arc::new(Mutex::new(UpnpState {
            config,
            mappings: Vec::new(),
            searches: 0,
            actions: Vec::new(),
        }));
        let stop = Arc::new(AtomicBool::new(false));
        let ssdp = ssdp_socket.local_addr()?;
        let ssdp_thread = {
            let (state, stop, location) = (state.clone(), stop.clone(), location.clone());
            std::thread::spawn(move || serve_ssdp(&ssdp_socket, &location, &state, &stop))
        };
        let http_thread = {
            let (state, stop) = (state.clone(), stop.clone());
            std::thread::spawn(move || serve_http(&listener, http, &state, &stop))
        };
        Ok(Self {
            ssdp,
            location,
            state,
            stop,
            threads: vec![ssdp_thread, http_thread],
        })
    }

    /// Where to send the search.
    pub fn ssdp_address(&self) -> SocketAddr {
        self.ssdp
    }

    /// The description's address, as the search's answer gives it.
    pub fn location(&self) -> &str {
        &self.location
    }

    /// The mappings it holds.
    pub fn mappings(&self) -> Vec<FakeUpnpMapping> {
        self.state.lock().expect("fake device").mappings.clone()
    }

    /// Searches answered or ignored.
    pub fn searches(&self) -> usize {
        self.state.lock().expect("fake device").searches
    }

    /// Every SOAP action received, as `path action`, in order.
    pub fn actions(&self) -> Vec<String> {
        self.state.lock().expect("fake device").actions.clone()
    }

    /// Changes how it behaves from the next request on.
    pub fn configure(&self, change: impl FnOnce(&mut FakeUpnpConfig)) {
        change(&mut self.state.lock().expect("fake device").config);
    }
}

impl Drop for FakeUpnp {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn serve_ssdp(socket: &UdpSocket, location: &str, state: &Mutex<UpnpState>, stop: &AtomicBool) {
    let mut buf = [0u8; 2048];
    while !stop.load(Ordering::Acquire) {
        let Ok((len, from)) = socket.recv_from(&mut buf) else {
            continue;
        };
        let silent = {
            let mut state = state.lock().expect("fake device");
            state.searches += 1;
            state.config.silent_ssdp
        };
        if let Some(answer) = ssdp::fake_answer(&buf[..len], location).filter(|_| !silent) {
            let _ = socket.send_to(answer.as_bytes(), from);
        }
    }
}

fn serve_http(
    listener: &TcpListener,
    address: SocketAddr,
    state: &Mutex<UpnpState>,
    stop: &AtomicBool,
) {
    while !stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, _)) => serve_connection(stream, address, state, stop),
            Err(_) => std::thread::sleep(Duration::from_millis(2)),
        }
    }
}

/// Reads one request: the head's lines and the body.
fn read_request(stream: &mut TcpStream) -> Option<(Vec<String>, String)> {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        if let Some(end) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buf[..end]).into_owned();
            let lines: Vec<String> = head.split("\r\n").map(str::to_owned).collect();
            let length: usize = lines
                .iter()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse().ok())?
                })
                .unwrap_or(0);
            while buf.len() < end + 4 + length {
                let n = stream.read(&mut chunk).ok().filter(|n| *n > 0)?;
                buf.extend_from_slice(&chunk[..n]);
            }
            let body = String::from_utf8_lossy(&buf[end + 4..end + 4 + length]).into_owned();
            return Some((lines, body));
        }
        let n = stream.read(&mut chunk).ok().filter(|n| *n > 0)?;
        buf.extend_from_slice(&chunk[..n]);
        if buf.len() > 256 * 1024 {
            return None;
        }
    }
}

fn serve_connection(
    mut stream: TcpStream,
    address: SocketAddr,
    state: &Mutex<UpnpState>,
    stop: &AtomicBool,
) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
    let Some((lines, body)) = read_request(&mut stream) else {
        return;
    };
    let config = state.lock().expect("fake device").config.clone();
    if config.silent_http {
        while !stop.load(Ordering::Acquire) && state.lock().expect("fake device").config.silent_http
        {
            std::thread::sleep(Duration::from_millis(5));
        }
        return;
    }
    let request_line = lines.first().cloned().unwrap_or_default();
    let path = request_line.split(' ').nth(1).unwrap_or("").to_owned();
    let (status, text) = if request_line.starts_with("GET ") && path == "/rootDesc.xml" {
        (200, description(&config, address))
    } else if request_line.starts_with("POST ") {
        let action = lines
            .iter()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("soapaction")
                    .then(|| value.trim().trim_matches('"').to_owned())
            })
            .and_then(|value| value.split_once('#').map(|(_, action)| action.to_owned()))
            .unwrap_or_default();
        let mut state = state.lock().expect("fake device");
        state.actions.push(format!("{path} {action}"));
        soap(&mut state, &config, &path, &action, &body)
    } else {
        (404, String::new())
    };
    let reason = match status {
        200 => "OK",
        404 => "Not Found",
        _ => "Internal Server Error",
    };
    let mut answer = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: text/xml; charset=\"utf-8\"\r\nConnection: close\r\n"
    );
    if config.chunked {
        answer.push_str("Transfer-Encoding: chunked\r\n\r\n");
        for (i, piece) in text.as_bytes().chunks(97).enumerate() {
            let extension = if i == 1 { ";name=value" } else { "" };
            answer.push_str(&format!(
                "{:x}{extension}\r\n{}\r\n",
                piece.len(),
                String::from_utf8_lossy(piece)
            ));
        }
        answer.push_str("0\r\n\r\n");
    } else {
        answer.push_str(&format!("Content-Length: {}\r\n\r\n{text}", text.len()));
    }
    let _ = stream.write_all(answer.as_bytes());
}

fn description(config: &FakeUpnpConfig, address: SocketAddr) -> String {
    let version = config.version;
    let (base, ip_control, ppp_control) = if version == 1 {
        (
            format!("<URLBase>http://{address}/</URLBase>"),
            "ctl/IPConn",
            "ctl/PPPConn",
        )
    } else {
        (String::new(), "/ctl/IPConn", "/ctl/PPPConn")
    };
    let service = |kind: &str, control: &str| {
        format!(
            "<service><serviceType>{kind}</serviceType><serviceId>urn:upnp-org:serviceId:Conn</serviceId><controlURL>{control}</controlURL><eventSubURL>/evt</eventSubURL><SCPDURL>/scpd.xml</SCPDURL></service>"
        )
    };
    let connection_services = if config.ppp_only {
        service(&format!("{WAN_PPP}1"), ppp_control)
    } else if version == 1 {
        service(&format!("{WAN_IP}1"), ip_control)
    } else {
        service(&format!("{WAN_PPP}1"), ppp_control)
            + &service(&format!("{WAN_IP}{version}"), ip_control)
    };
    format!(
        r#"<?xml version="1.0"?>
<root xmlns="urn:schemas-upnp-org:device-1-0">
<specVersion><major>1</major><minor>{minor}</minor></specVersion>
{base}
<device>
<deviceType>urn:schemas-upnp-org:device:InternetGatewayDevice:{version}</deviceType>
<friendlyName>Fake &amp; gateway</friendlyName>
<serviceList>{l3f}</serviceList>
<deviceList><device>
<deviceType>urn:schemas-upnp-org:device:WANDevice:{version}</deviceType>
<serviceList>{common}</serviceList>
<deviceList><device>
<deviceType>urn:schemas-upnp-org:device:WANConnectionDevice:{version}</deviceType>
<serviceList>{connection_services}</serviceList>
</device></deviceList>
</device></deviceList>
</device>
</root>
"#,
        minor = if version == 1 { 0 } else { 1 },
        l3f = service(
            "urn:schemas-upnp-org:service:Layer3Forwarding:1",
            "/ctl/L3F"
        ),
        common = service(
            "urn:schemas-upnp-org:service:WANCommonInterfaceConfig:1",
            "/ctl/CmnIfCfg"
        ),
    )
}

fn fault(code: u16, description: &str) -> (u16, String) {
    (
        500,
        format!(
            "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><s:Fault><faultcode>s:Client</faultcode><faultstring>UPnPError</faultstring><detail><UPnPError xmlns=\"urn:schemas-upnp-org:control-1-0\"><errorCode>{code}</errorCode><errorDescription>{description}</errorDescription></UPnPError></detail></s:Fault></s:Body></s:Envelope>"
        ),
    )
}

fn soap(
    state: &mut UpnpState,
    config: &FakeUpnpConfig,
    path: &str,
    action: &str,
    body: &str,
) -> (u16, String) {
    if path != "/ctl/IPConn" && path != "/ctl/PPPConn" {
        return (404, String::new());
    }
    // A version 2 device's PPP connection is not up.
    if path == "/ctl/PPPConn" && !config.ppp_only {
        return fault(401, "Invalid Action");
    }
    let kind = if path == "/ctl/PPPConn" {
        format!("{WAN_PPP}1")
    } else {
        format!("{WAN_IP}{}", config.version)
    };
    let events = xml::parse(body).unwrap_or_default();
    let arg = |name: &str| xml::first_text(&events, name).unwrap_or("").to_owned();
    let answer = |inside: String| {
        (
            200,
            format!(
                "<?xml version=\"1.0\"?><s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body><u:{action}Response xmlns:u=\"{kind}\">{inside}</u:{action}Response></s:Body></s:Envelope>"
            ),
        )
    };
    match action {
        "GetExternalIPAddress" => answer(format!(
            "<NewExternalIPAddress>{}</NewExternalIPAddress>",
            config.outside
        )),
        "AddPortMapping" => {
            let Ok(external_port) = arg("NewExternalPort").parse::<u16>() else {
                return fault(402, "Invalid Args");
            };
            let internal_port = arg("NewInternalPort").parse().unwrap_or(0);
            let lease = arg("NewLeaseDuration").parse().unwrap_or(0);
            let client = arg("NewInternalClient");
            if let Some(code) = config.refuse {
                return fault(code, "Refused");
            }
            if config.permanent_only && lease != 0 {
                return fault(ONLY_PERMANENT_LEASES, "OnlyPermanentLeasesSupported");
            }
            if config.same_ports_only && external_port != internal_port {
                return fault(SAME_PORTS_REQUIRED, "SamePortValuesRequired");
            }
            let other = state
                .mappings
                .iter()
                .any(|m| m.external_port == external_port && m.client != client);
            if config.taken.contains(&external_port) || other {
                return fault(CONFLICT, "ConflictInMappingEntry");
            }
            state.mappings.retain(|m| m.external_port != external_port);
            state.mappings.push(FakeUpnpMapping {
                external_port,
                internal_port,
                client,
                lease,
                description: arg("NewPortMappingDescription"),
            });
            answer(String::new())
        }
        "DeletePortMapping" => {
            let external_port = arg("NewExternalPort").parse::<u16>().unwrap_or(0);
            let before = state.mappings.len();
            state.mappings.retain(|m| m.external_port != external_port);
            if state.mappings.len() == before {
                return fault(714, "NoSuchEntryInArray");
            }
            answer(String::new())
        }
        _ => fault(401, "Invalid Action"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitMix64;

    fn location() -> Url {
        Url::parse("http://10.0.0.1:5000/rootDesc.xml").unwrap()
    }

    #[test]
    fn services_best_first_on_both_versions() {
        let address: SocketAddr = "10.0.0.1:5000".parse().unwrap();
        let v2 = services(
            &description(&FakeUpnpConfig::default(), address),
            &location(),
        );
        assert_eq!(v2.len(), 2);
        assert_eq!(v2[0].kind, "urn:schemas-upnp-org:service:WANIPConnection:2");
        assert_eq!(v2[0].control.path, "/ctl/IPConn");
        assert_eq!(
            v2[1].kind,
            "urn:schemas-upnp-org:service:WANPPPConnection:1"
        );
        let v1 = services(
            &description(
                &FakeUpnpConfig {
                    version: 1,
                    ..FakeUpnpConfig::default()
                },
                address,
            ),
            &location(),
        );
        assert_eq!(v1.len(), 1);
        assert_eq!(v1[0].kind, "urn:schemas-upnp-org:service:WANIPConnection:1");
        assert_eq!(
            v1[0].control,
            Url::parse("http://10.0.0.1:5000/ctl/IPConn").unwrap()
        );
    }

    #[test]
    fn control_addresses_stay_on_the_device() {
        let text = r#"<root><URLBase>http://10.9.9.9:80/</URLBase><device><serviceList>
            <service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>http://10.0.0.7/ctl</controlURL></service>
            <service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL>/ok</controlURL></service>
            <service><serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType><controlURL></controlURL></service>
        </serviceList></device></root>"#;
        let found = services(text, &location());
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(
            found[0].control,
            Url::parse("http://10.0.0.1:5000/ok").unwrap()
        );
    }

    #[test]
    fn an_envelope_and_a_fault() {
        let body = envelope(
            "urn:x:1",
            "AddPortMapping",
            &[("NewPortMappingDescription", "a<b".into())],
        );
        let events = xml::parse(&body).unwrap();
        assert_eq!(
            xml::first_text(&events, "NewPortMappingDescription"),
            Some("a<b")
        );
        assert!(body.contains("<u:AddPortMapping xmlns:u=\"urn:x:1\">"));
        let (status, text) = fault(718, "ConflictInMappingEntry");
        assert_eq!(status, 500);
        let events = xml::parse(&text).unwrap();
        assert_eq!(xml::first_text(&events, "errorCode"), Some("718"));
    }

    #[test]
    fn fuzzed_descriptions_never_panic() {
        let address: SocketAddr = "10.0.0.1:5000".parse().unwrap();
        let sample = description(
            &FakeUpnpConfig {
                version: 1,
                ..FakeUpnpConfig::default()
            },
            address,
        );
        let mut rng = SplitMix64::new(21);
        for _ in 0..10_000 {
            let mut bytes = sample.as_bytes().to_vec();
            for _ in 0..1 + rng.below(5) {
                let at = rng.below(bytes.len() as u64) as usize;
                match rng.below(3) {
                    0 => bytes[at] = rng.next_u64() as u8,
                    1 => bytes[at] = b"<>/:"[rng.below(4) as usize],
                    _ => {
                        bytes.remove(at);
                    }
                }
            }
            let _ = services(&String::from_utf8_lossy(&bytes), &location());
        }
    }
}
