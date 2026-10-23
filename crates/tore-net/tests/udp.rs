//! Two real UDP sockets on the loopback address: the handshake and messages
//! both ways, with real time. The only tests here that sleep.

use std::net::SocketAddr;
use std::time::Duration;

use tore_net::{
    AcceptInfo, Client, ClientConfig, ClientEvent, ConnectDetails, Decision, DisconnectReason,
    Event, RealClock, Server, ServerConfig, ServerEvent, bind_udp,
};

const VERSION: u16 = 1;

fn exchange(bind: &str) {
    let any: SocketAddr = bind.parse().unwrap();
    let mut host_socket = bind_udp(any).unwrap();
    let host_addr = host_socket.local_addr().unwrap();
    let mut client_socket = bind_udp(any).unwrap();
    let clock = RealClock::new();
    let mut server = Server::new(ServerConfig::new(VERSION));
    let mut gate = |details: &ConnectDetails| {
        assert_eq!(details.callsign, "Loopback");
        Decision::Accept(AcceptInfo {
            session_id: 1,
            ticks_per_second: 120,
            ticks_per_snapshot: 4,
            host_tick: 0,
        })
    };
    let mut client = Client::connect(
        ClientConfig::new(VERSION, "Loopback"),
        host_addr,
        clock.now(),
    )
    .unwrap();
    let big: Vec<u8> = (0..20_000).map(|i| (i % 253) as u8).collect();
    let mut connection = None;
    let mut client_got = Vec::new();
    let mut host_got = Vec::new();
    let mut host_closed = None;
    while clock.now() < Duration::from_secs(10) {
        let now = clock.now();
        server
            .receive_from(&mut host_socket, now, &mut gate)
            .unwrap();
        server.update(now);
        server.transmit(&mut host_socket).unwrap();
        client.receive_from(&mut client_socket, now).unwrap();
        client.update(now);
        client.transmit(&mut client_socket).unwrap();
        while let Some(event) = server.poll_event() {
            match event {
                ServerEvent::Connected { connection: id, .. } => {
                    connection = Some(id);
                    for i in 0..100u8 {
                        server.send_message(id, 1, &[i; 100]).unwrap();
                    }
                    server.send_message(id, 2, &big).unwrap();
                }
                ServerEvent::Connection {
                    event: Event::Message { body, .. },
                    ..
                } => host_got.push(body),
                ServerEvent::Closed { reason, .. } => host_closed = Some(reason),
                _ => {}
            }
        }
        while let Some(event) = client.poll_event() {
            match event {
                ClientEvent::Connected(_) => {
                    for i in 0..100u8 {
                        client.send_message(1, &[i; 10]).unwrap();
                    }
                }
                ClientEvent::Connection(Event::Message { body, .. }) => client_got.push(body),
                ClientEvent::Closed(reason) => panic!("closed: {reason:?}"),
                _ => {}
            }
        }
        if client_got.len() == 101 && host_got.len() == 100 {
            break;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(connection.is_some(), "no connection over {bind}");
    assert_eq!(host_got.len(), 100);
    assert_eq!(client_got.len(), 101);
    for i in 0..100u8 {
        assert_eq!(host_got[usize::from(i)], vec![i; 10]);
        assert_eq!(client_got[usize::from(i)], vec![i; 100]);
    }
    assert_eq!(client_got[100], big);
    assert!(client.stats().unwrap().round_trip < Duration::from_millis(100));

    // Leaving reaches the host.
    client.disconnect(DisconnectReason::Left);
    client.transmit(&mut client_socket).unwrap();
    let end = clock.now() + Duration::from_secs(2);
    while host_closed.is_none() && clock.now() < end {
        server
            .receive_from(&mut host_socket, clock.now(), &mut gate)
            .unwrap();
        while let Some(event) = server.poll_event() {
            if let ServerEvent::Closed { reason, .. } = event {
                host_closed = Some(reason);
            }
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(
        host_closed,
        Some(tore_net::CloseReason::Disconnected {
            reason: DisconnectReason::Left,
            by_peer: true
        })
    );
}

#[test]
fn two_udp_sockets_on_ipv4_loopback_connect_and_exchange_messages() {
    exchange("127.0.0.1:0");
}

#[test]
fn ipv6_loopback_works_where_the_system_has_it() {
    if bind_udp("[::1]:0".parse().unwrap()).is_err() {
        eprintln!("no IPv6 loopback here; skipped");
        return;
    }
    exchange("[::1]:0");
}
