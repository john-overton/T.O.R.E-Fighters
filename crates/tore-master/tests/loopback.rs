//! The master on real sockets on 127.0.0.1: a host registers and a browser
//! finds it (slice I2's real-socket acceptance).

use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use tore_master::Config;
use tore_master::run::Running;
use tore_net::master::browse::{BrowseEvent, Browser, BrowserConfig};
use tore_net::master::{Build, ListingSummary, MasterPacket, Probe, ProbePort, Register};
use tore_net::{Datagrams, Entropy, Listen, bind_udp};

fn build() -> Build {
    Build {
        protocol_version: 7,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        release: true,
    }
}

/// Turns the master until `done` says so, or two seconds pass.
fn pump(master: &mut Running, mut done: impl FnMut() -> bool) -> bool {
    let mut out = Vec::new();
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(2) {
        master.turn(&mut out).unwrap();
        if done() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    false
}

fn receive(socket: &mut UdpSocket) -> Option<MasterPacket> {
    let mut buf = [0u8; 1_500];
    socket
        .recv_datagram(&mut buf)
        .unwrap()
        .map(|(len, _)| MasterPacket::decode(&buf[..len]).unwrap())
}

#[test]
fn a_host_registers_and_a_browser_finds_it_on_real_sockets() {
    let mut config = Config::defaults(Path::new("."));
    config.listen = Listen::Address(Ipv4Addr::LOCALHOST.into());
    config.port = 0;
    config.probe_port = 0;
    let (mut master, notes) = Running::bind(config, Entropy::System, false).unwrap();
    assert!(notes.is_empty());
    let main: SocketAddr = master.main_addresses()[0];
    assert!(master.probe_addresses().is_empty());

    // The host: Register, Challenge, Register with the cookie, Listed.
    let mut host = bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
    let summary = ListingSummary {
        protocol_version: 7,
        players: 1,
        capacity: 8,
        game_version: "0.1.3".into(),
        game_commit: "abc".into(),
        name: "Loopback game".into(),
        ..ListingSummary::default()
    };
    let register = |cookie| {
        MasterPacket::Register(Register {
            nonce: 5,
            cookie,
            build: build(),
            dedicated: true,
            telemetry: false,
            install_id: 0,
            platform: 3,
            candidates: Vec::new(),
            summary: summary.clone(),
        })
        .encode()
        .unwrap()
    };
    host.send_datagram(main, &register(0)).unwrap();
    let mut answer = None;
    assert!(pump(&mut master, || {
        answer = receive(&mut host);
        answer.is_some()
    }));
    let Some(MasterPacket::Challenge(challenge)) = answer.take() else {
        panic!("{answer:?}")
    };
    host.send_datagram(main, &register(challenge.cookie))
        .unwrap();
    assert!(pump(&mut master, || {
        answer = receive(&mut host);
        answer.is_some()
    }));
    let Some(MasterPacket::Listed(listed)) = answer.take() else {
        panic!("{answer:?}")
    };
    assert_eq!(listed.seen, host.local_addr().unwrap());

    // A probe on the main port says where it came from.
    let probe = MasterPacket::Probe(Probe { nonce: 9 }).encode().unwrap();
    host.send_datagram(main, &probe).unwrap();
    assert!(pump(&mut master, || {
        answer = receive(&mut host);
        answer.is_some()
    }));
    assert!(matches!(
        answer,
        Some(MasterPacket::ProbeAnswer(p)) if p.port == ProbePort::Main && p.seen == host.local_addr().unwrap()
    ));

    // The browser finds it.
    let mut socket = bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
    let mut browser = Browser::new(
        BrowserConfig {
            build: build(),
            other_builds: false,
            full_games: false,
            entropy: Entropy::System,
        },
        vec![main],
    );
    let clock = Instant::now();
    browser.refresh(clock.elapsed());
    browser.transmit(&mut socket).unwrap();
    assert!(pump(&mut master, || {
        browser.receive_from(&mut socket, clock.elapsed()).unwrap();
        !browser.refreshing()
    }));
    let events: Vec<_> = std::iter::from_fn(|| browser.poll_event()).collect();
    assert!(
        matches!(events.as_slice(), [BrowseEvent::Added(e), BrowseEvent::Refreshed { matching: 1, shown: 1 }]
            if e.listing_id == listed.listing_id && e.name == "Loopback game" && e.dedicated),
        "{events:?}"
    );

    // The console shows it, and `quit` stops the loop.
    let lines = master.command("listings");
    assert_eq!(lines.len(), 2);
    assert!(lines[0].contains("name=\"Loopback game\" players=1/8 phase=Lobby"));
    assert_eq!(lines[1], "listings=1");
    assert!(master.command("status")[0].starts_with("status listings=1 sources=1 "));
    let (send, console) = mpsc::channel();
    send.send("quit".to_string()).unwrap();
    let mut out = Vec::new();
    master
        .run(&AtomicBool::new(false), &console, &mut out)
        .unwrap();
    let out = String::from_utf8(out).unwrap();
    assert_eq!(out, "Stopping\n");
}
