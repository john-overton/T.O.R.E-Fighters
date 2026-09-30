//! The server's UDP sockets. `address any` listens on every IPv4 and IPv6
//! address: one IPv6 socket that takes IPv4 as well where the system allows it
//! (Linux and macOS), and an IPv4 socket beside it where it does not (Windows,
//! and systems with no IPv6). The standard library cannot set the IPv6-only
//! option, so the program tries the IPv6 socket first and treats "address in
//! use" on the IPv4 one as the IPv6 socket already covering it.

use crate::config::Listen;
use std::{
    io,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket},
};
use tore_net::{Datagrams, bind_udp};

/// One or two bound, non-blocking sockets behind the transport's datagram trait.
pub struct ServerSocket {
    v6: Option<UdpSocket>,
    v4: Option<UdpSocket>,
}

impl ServerSocket {
    /// Binds the sockets for `listen` on `port`.
    pub fn bind(listen: Listen, port: u16) -> io::Result<Self> {
        match listen {
            Listen::Address(IpAddr::V4(ip)) => Ok(Self {
                v6: None,
                v4: Some(bind_udp(SocketAddr::new(IpAddr::V4(ip), port))?),
            }),
            Listen::Address(IpAddr::V6(ip)) => Ok(Self {
                v6: Some(bind_udp(SocketAddr::new(IpAddr::V6(ip), port))?),
                v4: None,
            }),
            Listen::Any => {
                let v6 = bind_udp(SocketAddr::new(Ipv6Addr::UNSPECIFIED.into(), port));
                let v4 = bind_udp(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), port));
                match (v6, v4) {
                    (Ok(v6), Ok(v4)) => Ok(Self {
                        v6: Some(v6),
                        v4: Some(v4),
                    }),
                    // The IPv6 socket takes IPv4 traffic too.
                    (Ok(v6), Err(error)) if error.kind() == io::ErrorKind::AddrInUse => Ok(Self {
                        v6: Some(v6),
                        v4: None,
                    }),
                    // No IPv6 on this machine.
                    (Err(_), Ok(v4)) => Ok(Self {
                        v6: None,
                        v4: Some(v4),
                    }),
                    (Err(error), Err(_)) | (Ok(_), Err(error)) => Err(error),
                }
            }
        }
    }

    /// The addresses the sockets are bound to, for the start lines.
    pub fn local_addresses(&self) -> Vec<SocketAddr> {
        [&self.v6, &self.v4]
            .into_iter()
            .flatten()
            .filter_map(|socket| socket.local_addr().ok())
            .collect()
    }
}

impl Datagrams for ServerSocket {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> io::Result<()> {
        // An IPv4 peer goes out of the IPv4 socket when there is one; else the
        // IPv6 socket sends to its IPv4-mapped address.
        match (&mut self.v4, &mut self.v6, to) {
            (Some(v4), _, SocketAddr::V4(_)) => v4.send_datagram(to, datagram),
            (_, Some(v6), SocketAddr::V6(_)) => v6.send_datagram(to, datagram),
            (None, Some(v6), SocketAddr::V4(v4_to)) => {
                let mapped = SocketAddr::new(v4_to.ip().to_ipv6_mapped().into(), v4_to.port());
                v6.send_datagram(mapped, datagram)
            }
            _ => Ok(()),
        }
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> io::Result<Option<(usize, SocketAddr)>> {
        for socket in [&mut self.v6, &mut self.v4].into_iter().flatten() {
            if let Some(received) = socket.recv_datagram(buf)? {
                return Ok(Some(received));
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn receive(socket: &mut ServerSocket) -> Option<(Vec<u8>, SocketAddr)> {
        let mut buf = [0u8; 64];
        let start = Instant::now();
        while start.elapsed() < Duration::from_secs(2) {
            if let Some((length, from)) = socket.recv_datagram(&mut buf).unwrap() {
                return Some((buf[..length].to_vec(), from));
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        None
    }

    #[test]
    fn a_loopback_socket_receives_and_answers() {
        let mut server =
            ServerSocket::bind(Listen::Address(Ipv4Addr::LOCALHOST.into()), 0).unwrap();
        let address = server.local_addresses()[0];
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.send_to(b"hello", address).unwrap();
        let (bytes, from) = receive(&mut server).expect("a datagram arrives");
        assert_eq!(bytes, b"hello");
        server.send_datagram(from, b"hi").unwrap();
        let mut buf = [0u8; 16];
        client
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let (length, _) = client.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..length], b"hi");
        assert!(server.recv_datagram(&mut buf).unwrap().is_none());
    }

    #[test]
    fn a_busy_port_is_an_error() {
        let first = ServerSocket::bind(Listen::Address(Ipv4Addr::LOCALHOST.into()), 0).unwrap();
        let port = first.local_addresses()[0].port();
        assert!(ServerSocket::bind(Listen::Address(Ipv4Addr::LOCALHOST.into()), port).is_err());
    }

    #[test]
    fn any_listens_for_ipv4_clients() {
        // Find a free port first; the two sockets of `any` share it.
        let port = {
            let probe = UdpSocket::bind("0.0.0.0:0").unwrap();
            probe.local_addr().unwrap().port()
        };
        let Ok(mut server) = ServerSocket::bind(Listen::Any, port) else {
            return; // the port was taken in between; nothing to learn
        };
        let client = UdpSocket::bind("127.0.0.1:0").unwrap();
        client.send_to(b"x", ("127.0.0.1", port)).unwrap();
        let (bytes, _) = receive(&mut server).expect("an IPv4 datagram reaches `any`");
        assert_eq!(bytes, b"x");
    }
}
