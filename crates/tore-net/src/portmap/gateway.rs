//! Where the router is, and what an outside address says about it.
//!
//! The gateway is read from the system without sending anything: on Linux
//! the default route in `/proc/net/route` (and `/proc/net/ipv6_route`), on
//! Windows `route print`, on macOS and the BSDs `route -n get default`, each
//! command bounded to a second. The parsers are public so their tests can
//! feed them every system's text on any system.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
#[cfg(not(any(target_os = "linux", target_os = "android")))]
use std::time::{Duration, Instant};

/// The IPv4 default gateway the system routes through, if it has one.
pub fn default_gateway() -> Option<Ipv4Addr> {
    system_gateway()
}

/// The IPv6 default router and the scope (interface index) to reach it
/// through, if the system has one. A link-local router needs its scope.
pub fn default_gateway_v6() -> Option<(Ipv6Addr, u32)> {
    system_gateway_v6()
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn system_gateway() -> Option<Ipv4Addr> {
    parse_proc_net_route(&std::fs::read_to_string("/proc/net/route").ok()?)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn system_gateway_v6() -> Option<(Ipv6Addr, u32)> {
    let (router, interface) =
        parse_proc_net_ipv6_route(&std::fs::read_to_string("/proc/net/ipv6_route").ok()?)?;
    let scope = if is_link_local_v6(router) {
        // The interface name comes from the kernel's own table; it never
        // holds a path separator.
        if interface.contains('/') || interface.contains("..") {
            return None;
        }
        std::fs::read_to_string(format!("/sys/class/net/{interface}/ifindex"))
            .ok()?
            .trim()
            .parse()
            .ok()?
    } else {
        0
    };
    Some((router, scope))
}

#[cfg(windows)]
fn system_gateway() -> Option<Ipv4Addr> {
    parse_windows_route_v4(&run(&windows_route(), &["print", "-4", "0.0.0.0"])?)
}

#[cfg(windows)]
fn system_gateway_v6() -> Option<(Ipv6Addr, u32)> {
    parse_windows_route_v6(&run(&windows_route(), &["print", "-6", "::/0"])?)
}

#[cfg(windows)]
fn windows_route() -> String {
    let root = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".to_owned());
    format!("{root}\\System32\\route.exe")
}

#[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
fn system_gateway() -> Option<Ipv4Addr> {
    parse_route_get(&run("/sbin/route", &["-n", "get", "default"])?)?
        .parse()
        .ok()
}

#[cfg(not(any(target_os = "linux", target_os = "android", windows)))]
fn system_gateway_v6() -> Option<(Ipv6Addr, u32)> {
    let text = parse_route_get(&run("/sbin/route", &["-n", "get", "-inet6", "default"])?)?;
    scoped_v6(&text)
}

/// Reads `fe80::1%en0` or `fe80::1%12` into the address and its scope. A
/// scope given by an interface's name is turned into its index by the
/// system's resolver, which reads a literal address without asking anyone.
#[cfg(any(not(any(target_os = "linux", target_os = "android", windows)), test))]
fn scoped_v6(text: &str) -> Option<(Ipv6Addr, u32)> {
    use std::net::ToSocketAddrs;
    let (address, scope) = match text.split_once('%') {
        Some((address, scope)) => (address, Some(scope)),
        None => (text, None),
    };
    let address: Ipv6Addr = address.parse().ok()?;
    let scope = match scope {
        None => 0,
        Some(scope) => match scope.parse::<u32>() {
            Ok(index) => index,
            Err(_) => match (text, 0).to_socket_addrs().ok()?.next()? {
                SocketAddr::V6(found) => found.scope_id(),
                SocketAddr::V4(_) => return None,
            },
        },
    };
    Some((address, scope))
}

/// Runs a system command for at most a second and returns what it printed.
#[cfg(not(any(target_os = "linux", target_os = "android")))]
fn run(program: &str, args: &[&str]) -> Option<String> {
    use std::io::Read;
    use std::process::{Command, Stdio};
    const TIMEOUT: Duration = Duration::from_secs(1);
    const MOST: u64 = 64 * 1024;
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: the game has no console to lend.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().ok()?;
    let stdout = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        let _ = stdout.take(MOST).read_to_end(&mut bytes);
        bytes
    });
    let until = Instant::now() + TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(10)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
    let bytes = reader.join().ok()?;
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

/// The default route's gateway from Linux's `/proc/net/route`: a route to
/// 0.0.0.0/0 that is up and goes through a gateway, the lowest metric first.
pub fn parse_proc_net_route(text: &str) -> Option<Ipv4Addr> {
    const UP_GATEWAY: u32 = 0x1 | 0x2;
    let mut best: Option<(u32, Ipv4Addr)> = None;
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 8 {
            continue;
        }
        let hex = |text: &str| u32::from_str_radix(text, 16).ok();
        let (Some(destination), Some(gateway), Some(flags), Some(mask)) = (
            hex(fields[1]),
            hex(fields[2]),
            hex(fields[3]),
            hex(fields[7]),
        ) else {
            continue;
        };
        let Ok(metric) = fields[6].parse::<u32>() else {
            continue;
        };
        if destination != 0 || mask != 0 || flags & UP_GATEWAY != UP_GATEWAY || gateway == 0 {
            continue;
        }
        // The kernel prints the address's network-order bytes as a number in
        // the machine's own order.
        let address = Ipv4Addr::from(gateway.to_ne_bytes());
        if best.is_none_or(|(lowest, _)| metric < lowest) {
            best = Some((metric, address));
        }
    }
    best.map(|(_, address)| address)
}

/// The default route's next hop and interface name from Linux's
/// `/proc/net/ipv6_route`, the lowest metric first.
pub fn parse_proc_net_ipv6_route(text: &str) -> Option<(Ipv6Addr, String)> {
    const UP_GATEWAY: u32 = 0x1 | 0x2;
    let mut best: Option<(u32, Ipv6Addr, String)> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() < 10 {
            continue;
        }
        let (Some(destination), Some(next_hop)) = (hex_v6(fields[0]), hex_v6(fields[4])) else {
            continue;
        };
        let (Ok(prefix), Ok(metric), Ok(flags)) = (
            u8::from_str_radix(fields[1], 16),
            u32::from_str_radix(fields[5], 16),
            u32::from_str_radix(fields[8], 16),
        ) else {
            continue;
        };
        if !destination.is_unspecified()
            || prefix != 0
            || next_hop.is_unspecified()
            || flags & UP_GATEWAY != UP_GATEWAY
            || fields[9] == "lo"
        {
            continue;
        }
        if best.as_ref().is_none_or(|(lowest, _, _)| metric < *lowest) {
            best = Some((metric, next_hop, fields[9].to_owned()));
        }
    }
    best.map(|(_, router, interface)| (router, interface))
}

fn hex_v6(text: &str) -> Option<Ipv6Addr> {
    if text.len() != 32 || !text.is_ascii() {
        return None;
    }
    let mut octets = [0u8; 16];
    for (i, octet) in octets.iter_mut().enumerate() {
        *octet = u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).ok()?;
    }
    Some(Ipv6Addr::from(octets))
}

/// The `gateway:` line of `route -n get default` on macOS and the BSDs.
pub fn parse_route_get(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        (key.trim() == "gateway").then(|| value.trim().to_owned())
    })
}

/// The default route's gateway from Windows' `route print -4 0.0.0.0`: a row
/// of five columns whose destination and mask are both 0.0.0.0, the lowest
/// metric first. The headings are translated on other languages' Windows;
/// the rows are not.
pub fn parse_windows_route_v4(text: &str) -> Option<Ipv4Addr> {
    let mut best: Option<(u32, Ipv4Addr)> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 5 || fields[0] != "0.0.0.0" || fields[1] != "0.0.0.0" {
            continue;
        }
        let (Ok(gateway), Ok(metric)) = (fields[2].parse::<Ipv4Addr>(), fields[4].parse::<u32>())
        else {
            continue;
        };
        if gateway.is_unspecified() {
            continue;
        }
        if best.is_none_or(|(lowest, _)| metric < lowest) {
            best = Some((metric, gateway));
        }
    }
    best.map(|(_, gateway)| gateway)
}

/// The default router and its interface index from Windows' `route print -6
/// ::/0`: rows of interface, metric, `::/0` and the gateway.
pub fn parse_windows_route_v6(text: &str) -> Option<(Ipv6Addr, u32)> {
    let mut best: Option<(u32, Ipv6Addr, u32)> = None;
    for line in text.lines() {
        let fields: Vec<&str> = line.split_whitespace().collect();
        if fields.len() != 4 || fields[2] != "::/0" {
            continue;
        }
        let (Ok(interface), Ok(metric), Ok(gateway)) = (
            fields[0].parse::<u32>(),
            fields[1].parse::<u32>(),
            fields[3].parse::<Ipv6Addr>(),
        ) else {
            continue;
        };
        if gateway.is_unspecified() {
            continue;
        }
        if best.is_none_or(|(lowest, _, _)| metric < lowest) {
            best = Some((metric, gateway, interface));
        }
    }
    best.map(|(_, gateway, interface)| (gateway, interface))
}

/// The address this machine sends from toward `to`: a UDP socket pointed at
/// it takes the address the system would use (no packet is sent).
pub fn local_address_toward(to: SocketAddr) -> Option<IpAddr> {
    let any: SocketAddr = match to {
        SocketAddr::V4(_) => (Ipv4Addr::UNSPECIFIED, 0).into(),
        SocketAddr::V6(_) => (Ipv6Addr::UNSPECIFIED, 0).into(),
    };
    let socket = UdpSocket::bind(any).ok()?;
    socket.connect(to).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    (!ip.is_unspecified()).then_some(ip)
}

/// The last guess for a gateway: the local network's address ending in 1.
pub fn guess_gateway(local: Ipv4Addr) -> Ipv4Addr {
    let [a, b, c, _] = local.octets();
    Ipv4Addr::new(a, b, c, 1)
}

/// True for an outside address that is itself inside another network: a
/// private address (10/8, 172.16/12, 192.168/16) or a carrier's shared
/// address (100.64/10). A router that reports one is behind another router.
pub fn is_inner_address(ip: Ipv4Addr) -> bool {
    let [a, b, _, _] = ip.octets();
    ip.is_private() || (a == 100 && b & 0xC0 == 64)
}

/// True for an outside address no one on the internet could send to: none
/// at all (a router not yet connected says 0.0.0.0), loopback, link-local,
/// broadcast or multicast.
pub fn is_no_address(ip: Ipv4Addr) -> bool {
    ip.is_unspecified()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_broadcast()
        || ip.is_multicast()
}

#[cfg(any(target_os = "linux", target_os = "android", test))]
fn is_link_local_v6(ip: Ipv6Addr) -> bool {
    ip.segments()[0] & 0xFFC0 == 0xFE80
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SplitMix64;

    const PROC_ROUTE: &str = "\
Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT
wlan0\t0000A8C0\t00000000\t0001\t0\t0\t600\t00FFFFFF\t0\t0\t0
eth0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0
wlan0\t00000000\t0102A8C0\t0003\t0\t0\t600\t00000000\t0\t0\t0
";

    #[test]
    fn linux_default_route_lowest_metric() {
        if cfg!(target_endian = "little") {
            assert_eq!(
                parse_proc_net_route(PROC_ROUTE),
                Some(Ipv4Addr::new(192, 168, 1, 1))
            );
        }
        let only_link = PROC_ROUTE.lines().take(2).collect::<Vec<_>>().join("\n");
        assert_eq!(parse_proc_net_route(&only_link), None);
        assert_eq!(parse_proc_net_route(""), None);
    }

    #[test]
    fn linux_ipv6_default_router() {
        let text = "\
20010db8000000000000000000000000 40 00000000000000000000000000000000 00 00000000000000000000000000000000 00000100 00000001 00000000 00000001 eth0
00000000000000000000000000000000 00 00000000000000000000000000000000 00 fe800000000000000000000000000001 00000400 00000001 00000000 00000003 eth0
00000000000000000000000000000000 00 00000000000000000000000000000000 00 00000000000000000000000000000000 ffffffff 00000001 00000000 00200200 lo
";
        let (router, interface) = parse_proc_net_ipv6_route(text).unwrap();
        assert_eq!(router, "fe80::1".parse::<Ipv6Addr>().unwrap());
        assert_eq!(interface, "eth0");
        assert!(is_link_local_v6(router));
    }

    #[test]
    fn bsd_route_get() {
        let text = "   route to: default\ndestination: default\n       mask: default\n    gateway: 192.168.1.254\n  interface: en0\n";
        assert_eq!(parse_route_get(text).as_deref(), Some("192.168.1.254"));
        assert_eq!(
            parse_route_get("route: writing to routing socket: not in table"),
            None
        );
    }

    #[test]
    fn scoped_ipv6_routers() {
        let router: Ipv6Addr = "fe80::1".parse().unwrap();
        assert_eq!(scoped_v6("fe80::1%12"), Some((router, 12)));
        assert_eq!(
            scoped_v6("2001:db8::1"),
            Some(("2001:db8::1".parse().unwrap(), 0))
        );
        assert_eq!(scoped_v6("not an address"), None);
    }

    #[test]
    fn windows_route_print() {
        let v4 = "\
===========================================================================
IPv4 Route Table
===========================================================================
Active Routes:
Network Destination        Netmask          Gateway       Interface  Metric
          0.0.0.0          0.0.0.0      192.168.1.1     192.168.1.23     25
          0.0.0.0          0.0.0.0         10.0.0.1         10.0.0.9     50
===========================================================================
Persistent Routes:
  Network Address          Netmask  Gateway Address  Metric
          0.0.0.0          0.0.0.0         10.9.9.9  Default
";
        assert_eq!(
            parse_windows_route_v4(v4),
            Some(Ipv4Addr::new(192, 168, 1, 1))
        );
        let v6 = "\
IPv6 Route Table
===========================================================================
Active Routes:
 If Metric Network Destination      Gateway
 12    281 ::/0                     fe80::1
  7    300 ::/0                     On-link
===========================================================================
";
        assert_eq!(
            parse_windows_route_v6(v6),
            Some(("fe80::1".parse().unwrap(), 12))
        );
    }

    #[test]
    fn inner_and_missing_outside_addresses() {
        for inner in [
            "10.1.2.3",
            "172.16.0.1",
            "172.31.255.255",
            "192.168.0.2",
            "100.64.0.1",
            "100.127.255.254",
        ] {
            assert!(is_inner_address(inner.parse().unwrap()), "{inner}");
        }
        for outside in [
            "203.0.113.5",
            "100.63.255.255",
            "100.128.0.1",
            "172.32.0.1",
            "8.8.8.8",
        ] {
            assert!(!is_inner_address(outside.parse().unwrap()), "{outside}");
        }
        for none in [
            "0.0.0.0",
            "127.0.0.1",
            "169.254.1.1",
            "255.255.255.255",
            "224.0.0.1",
        ] {
            assert!(is_no_address(none.parse().unwrap()), "{none}");
        }
        assert!(!is_no_address(Ipv4Addr::new(203, 0, 113, 5)));
        assert_eq!(
            guess_gateway(Ipv4Addr::new(192, 168, 7, 23)),
            Ipv4Addr::new(192, 168, 7, 1)
        );
    }

    #[test]
    fn loopback_is_reached_from_loopback() {
        assert_eq!(
            local_address_toward("127.0.0.1:9".parse().unwrap()),
            Some(IpAddr::V4(Ipv4Addr::LOCALHOST))
        );
    }

    #[test]
    fn route_parsers_survive_noise() {
        let samples = [
            PROC_ROUTE,
            "gateway: 1.2.3.4",
            " 12    281 ::/0                     fe80::1",
        ];
        let mut rng = SplitMix64::new(4);
        for round in 0..20_000 {
            let mut bytes: Vec<u8> = samples[round % samples.len()].as_bytes().to_vec();
            for _ in 0..1 + rng.below(8) {
                if bytes.is_empty() {
                    break;
                }
                let at = rng.below(bytes.len() as u64) as usize;
                match rng.below(3) {
                    0 => bytes[at] = rng.next_u64() as u8,
                    1 => {
                        bytes.remove(at);
                    }
                    _ => bytes.insert(at, b" \t\n:%0f"[rng.below(7) as usize]),
                }
            }
            let text = String::from_utf8_lossy(&bytes);
            let _ = parse_proc_net_route(&text);
            let _ = parse_proc_net_ipv6_route(&text);
            let _ = parse_route_get(&text);
            let _ = parse_windows_route_v4(&text);
            let _ = parse_windows_route_v6(&text);
        }
    }
}
