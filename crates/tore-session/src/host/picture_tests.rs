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
