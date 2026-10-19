//! Slice S11: the seats' entities from the tick's shared picture against a
//! whole picture per seat and the walk of the last picture, as the host made
//! them before, compared packet byte for packet byte with 0, 15 and 30
//! humans in a fight with guns, missiles and the AI.
use super::*;
use crate::wire::inputs::Command;
use tore_workers::Executor;
use tore_world::seats::SeatCommand;

/// Thirty aircraft in six wings of five, two nm apart; every plane open.
fn thirty() -> MissionSpec {
    let mut spec = spec(5, 5, 2);
    for wing in spec.wings.iter_mut() {
        wing.count = 5;
        wing.skill = Skill::Average;
    }
    spec
}

fn rig(humans: usize, legacy: bool) -> Rig {
    let mut rig = Rig::new(
        thirty(),
        HostConfig {
            open_planes: OpenPlanes::All,
            start: StartMode::Now,
            ..config()
        },
        LinkConfig::for_round_trip(Duration::from_millis(30), 0.05, 0.01, 0.01),
    );
    rig.host.snapshot_executor = Some(Arc::new(Executor::serial()));
    rig.host.legacy_pictures = legacy;
    for id in 0..humans {
        rig.join(|client| client.callsign = format!("Seat{id}"));
    }
    rig
}

fn step(rig: &mut Rig) -> Vec<Transmit> {
    rig.net.advance(MS);
    let now = rig.net.now();
    rig.host.receive_from(now, &mut rig.socket).unwrap();
    rig.host.update(now);
    let mut packets = Vec::new();
    while let Some(packet) = rig.host.poll_transmit() {
        rig.socket
            .send_datagram(packet.to, &packet.datagram)
            .unwrap();
        packets.push(packet);
    }
    while let Some(log) = rig.host.poll_log() {
        rig.logs.push(log);
    }
    let tick = rig.host.world().tick();
    // Every other seat picks its missiles once seated, so the gun script's
    // trigger fires them.
    for (index, client) in rig.clients.iter_mut().enumerate() {
        if index % 2 == 1
            && client.seated.is_some()
            && client.commands.is_empty()
            && tick % 600 == 300
        {
            client.command(
                tick as u32 + client.lead,
                Command::Seat(SeatCommand::CycleWeapon { forward: true }),
            );
        }
        client.pump(now, tick);
    }
    packets
}

fn compare(humans: usize, millis: u64) {
    let mut legacy = rig(humans, true);
    let mut shared = rig(humans, false);
    let mut packets = 0;
    for _ in 0..millis {
        let before = step(&mut legacy);
        let after = step(&mut shared);
        assert_eq!(after, before, "packets at {:?}", shared.net.now());
        packets += after.len();
        assert_eq!(shared.host.world.tick(), legacy.host.world.tick());
        for (id, peer) in &shared.host.peers {
            let old = &legacy.host.peers[id];
            assert_eq!(
                peer.picture.as_ref().map(|p| p.keys().to_vec()),
                old.picture.as_ref().map(|p| p.keys().to_vec())
            );
            assert_eq!(peer.wire.names.names(), old.wire.names.names());
        }
    }
    let seated = shared
        .clients
        .iter()
        .filter(|client| client.seated.is_some())
        .count();
    assert_eq!(seated, humans, "every human seated");
    assert!(shared.clients.iter().all(|client| client.errors.is_empty()));
    let world = &shared.host.world;
    eprintln!(
        "{humans} humans: {packets} packets, tick {}, {} rounds, {} debris",
        world.tick(),
        world.combat.state.projectiles.len(),
        world.combat.state.debris.len()
    );
}

#[test]
fn the_shared_picture_sends_the_same_packets_with_no_human() {
    compare(0, 2_000);
}

#[test]
fn the_shared_picture_sends_the_same_packets_to_15_humans() {
    compare(15, 8_000);
}

#[test]
fn the_shared_picture_sends_the_same_packets_to_30_humans() {
    compare(30, 6_000);
}

/// A host and bots on the network simulator, for the real import.
struct Bots {
    net: SimNetwork,
    host: Host,
    socket: SimSocket,
    bots: Vec<(SimSocket, crate::bot::Bot)>,
}

impl Bots {
    fn new(
        spec: MissionSpec,
        resources: &Arc<BTreeMap<String, Vec<u8>>>,
        bots: u32,
        legacy: bool,
    ) -> Self {
        let net = SimNetwork::new(13);
        net.set_default_link(LinkConfig::for_round_trip(
            Duration::from_millis(40),
            0.,
            0.,
            0.,
        ));
        let socket = net.bind(host_address()).unwrap();
        let mut host = Host::new(
            spec,
            Arc::clone(resources),
            HostConfig {
                open_planes: OpenPlanes::All,
                start: StartMode::Now,
                ..config()
            },
        )
        .unwrap();
        host.snapshot_executor = Some(Arc::new(Executor::serial()));
        host.legacy_pictures = legacy;
        let bots = (0..bots)
            .map(|plane| {
                let port = 41_000 + plane as u16;
                let address: SocketAddr = format!("10.0.0.2:{port}").parse().unwrap();
                let socket = net.bind(address).unwrap();
                let config = crate::client::ClientConfig {
                    entropy: Entropy::Seeded(u64::from(port)),
                    plane: Some(plane),
                    ..crate::client::ClientConfig::new(host_address(), "Bot", build())
                };
                let client =
                    crate::client::Client::connect(config, Arc::clone(resources), net.now())
                        .unwrap();
                (socket, crate::bot::Bot::new(client))
            })
            .collect();
        Self {
            net,
            host,
            socket,
            bots,
        }
    }

    fn step(&mut self) -> Vec<Transmit> {
        self.net.advance(MS);
        let now = self.net.now();
        self.host.receive_from(now, &mut self.socket).unwrap();
        self.host.update(now);
        let mut packets = Vec::new();
        while let Some(packet) = self.host.poll_transmit() {
            self.socket
                .send_datagram(packet.to, &packet.datagram)
                .unwrap();
            packets.push(packet);
        }
        while self.host.poll_log().is_some() {}
        for (socket, bot) in &mut self.bots {
            bot.client.receive_from(now, socket).unwrap();
            bot.update(now);
            bot.client.transmit(socket).unwrap();
            while bot.client.poll_event().is_some() {}
        }
        packets
    }
}

/// The comparison on a real import: 15 against 15 F/A-18Ds and MiG-29s, a
/// bot in each of `TORE_S11_BOTS` planes (default 15), for
/// `TORE_S11_SECONDS` (default 60), every packet compared. Run it in release:
///
/// ```sh
/// TORE_DATA_DIR=... cargo test --release -p tore-session --lib \
///     real_data_pictures -- --ignored --nocapture
/// ```
#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; run by hand in release"]
fn real_data_pictures_send_the_same_packets() {
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let import = Arc::new(tore_import::load(&directory).expect("an imported pack"));
    let mut spec = MissionSpec::new("UKR", AircraftId::F18);
    for (index, wing) in spec.wings.iter_mut().enumerate() {
        wing.count = 5;
        wing.skill = Skill::Average;
        if index >= 3 {
            wing.aircraft = AircraftId::Mig29;
        }
    }
    spec.separation_nm = 5;
    spec.start = Start::Airborne {
        altitude_ft: 20_000,
    };
    let number = |name: &str, default: u64| {
        std::env::var(name)
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(default)
    };
    let bots = number("TORE_S11_BOTS", 15) as u32;
    let seconds = number("TORE_S11_SECONDS", 60);
    let mut legacy = Bots::new(spec.clone(), &import, bots, true);
    let mut shared = Bots::new(spec, &import, bots, false);
    let (mut packets, mut bytes) = (0usize, 0usize);
    let (mut rounds, mut debris, mut pilots) = (0usize, 0usize, 0usize);
    for ms in 0..seconds * 1_000 {
        let before = legacy.step();
        let after = shared.step();
        assert_eq!(after, before, "packets at {ms} ms");
        packets += after.len();
        bytes += after.iter().map(|p| p.datagram.len()).sum::<usize>();
        let state = &shared.host.world.combat.state;
        rounds = rounds.max(state.projectiles.len());
        debris = debris.max(state.debris.len());
        pilots = pilots.max(
            shared
                .host
                .world
                .ai_wings
                .iter()
                .map(|w| w.escapees().count())
                .sum(),
        );
    }
    eprintln!(
        "{bots} bots, {seconds} s: {packets} packets, {bytes} bytes the same; \
         at most {rounds} rounds, {debris} debris, {pilots} AI pilots out"
    );
    assert!(packets > 0);
}
