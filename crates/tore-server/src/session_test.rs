//! The acceptance run: the real server loop and the real host session on
//! 127.0.0.1, a scripted client (the transport's client and the wire's
//! messages, as in `tore-session`'s host tests) joins over a real UDP socket,
//! takes a plane, flies, leaves, and the console's `quit` ends the server.

use crate::{
    app::{commit, serve_with, version},
    clock::RealTimer,
    config::Listen,
    console::Command,
    log::Logger,
    options::Options,
    prepare::{
        self,
        tests::{MISSION, data_folder},
    },
    wiring,
};
use std::{
    fs,
    net::{SocketAddr, UdpSocket},
    path::Path,
    sync::mpsc::channel,
    time::{Duration, Instant},
};
use tore_net::{Client, ClientConfig, ClientEvent, Entropy, Event, bind_udp};
use tore_session::wire::{
    PROTOCOL_VERSION, SECTION_INPUTS,
    inputs::{InputFrame, InputsSection},
    messages::{Message, TakePlane},
};

fn free_port() -> u16 {
    UdpSocket::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct Player {
    socket: UdpSocket,
    client: Client,
    origin: Instant,
    mission: bool,
    seated: Option<u32>,
    flight: u8,
    payloads: u32,
    closed: bool,
    last_inputs: Option<Duration>,
}

impl Player {
    fn now(&self) -> Duration {
        self.origin.elapsed()
    }

    fn pump(&mut self) {
        let now = self.now();
        let _ = self.client.receive_from(&mut self.socket, now);
        self.client.update(now);
        while let Some(event) = self.client.poll_event() {
            match event {
                ClientEvent::Closed(_) => self.closed = true,
                ClientEvent::Connected(_) => {}
                ClientEvent::Connection(Event::Message { kind, body }) => {
                    match Message::decode(kind, &body) {
                        Ok(Message::Mission(mission)) => {
                            self.mission = true;
                            let take = Message::TakePlane(TakePlane {
                                mission: mission.number,
                                plane: None,
                            });
                            let body = take.encode().unwrap();
                            self.client.send_message(take.kind(), &body).unwrap();
                        }
                        Ok(Message::Seated(seated)) => {
                            self.seated = Some(seated.plane);
                            self.flight = seated.flight;
                        }
                        _ => {}
                    }
                }
                ClientEvent::Connection(Event::Payload { .. }) => self.payloads += 1,
                ClientEvent::Connection(_) => {}
            }
        }
        let sending = self.seated.is_some()
            && self
                .last_inputs
                .is_none_or(|last| now - last >= Duration::from_millis(16));
        if sending {
            // Neutral controls a little ahead of the host's clock.
            let newest = (self.now().as_secs_f64() * 120.0) as u32 + 120;
            let frame = InputFrame::of(&Default::default(), false, Default::default());
            let section = InputsSection {
                flight: self.flight,
                newest_tick: newest,
                frames: vec![frame; 24],
                view_offset: 18,
                interpolation_delay: 12,
                view_subject: None,
                mismatch: 0,
                commands: Vec::new(),
            };
            if self
                .client
                .send_payload(now, &[(SECTION_INPUTS, &section.encode().unwrap())])
                .is_ok()
            {
                self.last_inputs = Some(now);
            }
        }
        let _ = self.client.transmit(&mut self.socket);
    }

    fn until(&mut self, what: &str, mut done: impl FnMut(&Self) -> bool) {
        let start = Instant::now();
        while !done(self) {
            assert!(
                start.elapsed() < Duration::from_secs(20),
                "timed out waiting for {what}"
            );
            self.pump();
            std::thread::sleep(Duration::from_millis(2));
        }
    }
}

fn logged(dir: &Path) -> String {
    let mut text = String::new();
    for entry in fs::read_dir(dir.join("logs")).unwrap().flatten() {
        text += &fs::read_to_string(entry.path()).unwrap();
    }
    text
}

#[test]
fn a_scripted_client_joins_flies_and_leaves_and_quit_ends_the_server() {
    let dir = data_folder("session", true);
    fs::write(dir.join("mission.txt"), MISSION).unwrap();
    let port = free_port();
    let address: SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();
    let (console, commands) = channel();

    let server_dir = dir.clone();
    let server = std::thread::spawn(move || {
        let mut prepared = prepare::prepare(&Options::default(), &server_dir).unwrap();
        prepared.config.address = Listen::Address("127.0.0.1".parse().unwrap());
        prepared.config.port = port;
        prepared.config.status_interval_seconds = 0;
        let log = Logger::new(server_dir.join("logs"), Box::new(std::io::sink()));
        serve_with(
            prepared,
            wiring::start_host,
            commands,
            &mut RealTimer::new(),
            log,
        )
    });

    let socket = bind_udp("127.0.0.1:0".parse().unwrap()).unwrap();
    let config = ClientConfig {
        game_version: version().to_owned(),
        game_commit: commit().to_owned(),
        entropy: Entropy::System,
        ..ClientConfig::new(PROTOCOL_VERSION, "Viper")
    };
    let origin = Instant::now();
    // The server may still be starting; the transport repeats its handshake.
    let client = Client::connect(config, address, origin.elapsed()).unwrap();
    let mut player = Player {
        socket,
        client,
        origin,
        mission: false,
        seated: None,
        flight: 0,
        payloads: 0,
        closed: false,
        last_inputs: None,
    };
    player.until("the mission", |p| p.mission);
    player.until("a plane", |p| p.seated.is_some());
    // The mission was waiting for its first player; now it flies and the
    // player's game receives snapshots.
    player.until("snapshots", |p| p.payloads >= 20);
    assert_eq!(player.seated, Some(0), "the first free friendly plane");

    // Leave the flight: the message, then the player is back in the lobby,
    // still connected, until the server quits.
    let leave = Message::Leave;
    player
        .client
        .send_message(leave.kind(), &leave.encode().unwrap())
        .unwrap();
    // Keep the client running a moment so the message goes out and the host
    // answers before the server is told to quit.
    let waited = Instant::now();
    while waited.elapsed() < Duration::from_millis(600) {
        player.pump();
        std::thread::sleep(Duration::from_millis(2));
    }

    console.send(Command::Quit).unwrap();
    let ended = server.join().expect("the server thread did not panic");
    assert_eq!(ended, Ok(()));

    let log = logged(&dir);
    for needle in [
        "Waiting for players",
        &format!("Listening on UDP 127.0.0.1:{port}"),
        "joined as Viper",
        "Viper took the slot of plane 0",
        "Viper is ready",
        "Viper took plane 0",
        "mission started",
        "Viper is back in the lobby",
        "Viper left: the mission ended",
        "console: quit",
        "Stopped",
    ] {
        assert!(log.contains(needle), "{needle} in\n{log}");
    }
    let _ = fs::remove_dir_all(dir);
}
