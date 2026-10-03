//! Snapshot workers compared through the complete seeded transport, including
//! ordering, acknowledgement history and flight handoffs.
use super::*;
use tore_workers::Executor;

fn rig(executor: Executor) -> Rig {
    let mut rig = Rig::new(
        spec(5, 0, 50),
        config(),
        LinkConfig::for_round_trip(Duration::from_millis(20), 0.1, 0.02, 0.01),
    );
    rig.host.snapshot_executor = Some(Arc::new(executor));
    for id in 0..5 {
        rig.join(|client| client.callsign = format!("Worker{id}"));
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
    for client in &mut rig.clients {
        client.pump(now, tick);
    }
    packets
}

fn bookkeeping(host: &Host) -> Vec<String> {
    host.peers
        .iter()
        .map(|(id, peer)| {
            // EntitySender uses hash tables internally. Compare its public
            // acknowledged states in entity order, and exercise all pending
            // history through exact outgoing packets and subsequent ACKs.
            let mut names = peer.wire.names.clone();
            let acknowledged: Vec<_> = peer
                .picture
                .as_ref()
                .map(|picture| {
                    from_world::entities(
                        picture,
                        None,
                        peer.plane.map_or(u32::MAX, |plane| plane.0),
                        &mut names,
                    )
                    .unwrap()
                    .into_iter()
                    .map(|entity| (entity.key(), peer.wire.entities.acknowledged(entity.key())))
                    .collect()
                })
                .unwrap_or_default();
            let unsent_names = peer.wire.names.clone().take_new();
            format!(
                "{id:?} {:?} {:?} {:?} {:?} {} {} {} {} {:?} {} {:?} {:?} {:?} {:?} {:?} {:?}",
                peer.stage,
                peer.seat,
                peer.plane,
                peer.holding_since,
                peer.last_own_state,
                peer.unforeseen,
                peer.mismatch_answered,
                peer.flight,
                acknowledged,
                peer.wire.entities.removals_pending(),
                peer.wire.events,
                peer.wire.own,
                peer.wire.readout,
                peer.wire.names.names(),
                unsent_names,
                host.server.stats(*id)
            )
        })
        .collect()
}

fn compare_step(actual: &mut Rig, expected: &mut Rig) {
    let before = step(expected);
    let after = step(actual);
    assert_eq!(after, before, "packets at {:?}", actual.net.now());
    assert_eq!(actual.host.world.tick(), expected.host.world.tick());
    if !after.is_empty() {
        assert_eq!(bookkeeping(&actual.host), bookkeeping(&expected.host));
    }
}

#[test]
fn snapshot_workers_keep_packet_bytes_order_and_bookkeeping_through_handoffs() {
    let mut executors: Vec<_> = [0, 1, 2, 4, 8]
        .into_iter()
        .map(|count| Executor::parallel(count).unwrap())
        .collect();
    executors.extend([Executor::shuffled(0), Executor::shuffled(17)]);
    for executor in executors {
        let mut expected = rig(Executor::serial());
        let mut actual = rig(executor);
        for millis in 0..3000 {
            for rig in [&mut actual, &mut expected] {
                match millis {
                    700 => {
                        assert!(rig.clients.iter().all(|client| client.seated.is_some()));
                        let first = rig.clients[0].plane().unwrap().0;
                        let second = rig.clients[1].plane().unwrap().0;
                        rig.clients[0].ready = Some(Some(second));
                        rig.clients[1].ready = Some(Some(first));
                        rig.clients[0].leave_flight();
                        rig.clients[1].leave_flight();
                    }
                    950 => {
                        for client in &mut rig.clients[..2] {
                            client.send(&Message::Slot(messages::Slot {
                                mission: client.number(),
                                request: SlotRequest::Leave,
                            }));
                        }
                    }
                    1200 => {
                        for client in &mut rig.clients[..2] {
                            client.flying = true;
                            client.take(client.ready.flatten());
                        }
                    }
                    1800 => rig.clients[1].leave(),
                    2300 => {
                        rig.join(|client| client.callsign = "Reconnected".into());
                    }
                    _ => {}
                }
            }
            compare_step(&mut actual, &mut expected);
        }
        let reconnected = actual.clients.last().unwrap();
        assert!(
            reconnected.seated.is_some(),
            "new client: closed={:?} seat_refused={:?} errors={:?}; previous closed={:?} peer stages={:?}",
            reconnected.closed,
            reconnected.seat_refused,
            reconnected.errors,
            actual.clients[1].closed,
            actual
                .host
                .peers
                .values()
                .map(|peer| (&peer.callsign, peer.stage, peer.plane))
                .collect::<Vec<_>>()
        );
        assert!(actual.clients.iter().all(|client| client.errors.is_empty()));
        assert!(actual.clients[0].seated.as_ref().unwrap().flight >= 2);
        assert_eq!(
            actual.clients[0].plane().map(|plane| plane.0),
            actual.clients[0].ready.flatten()
        );
        let tps = actual.host.config.ticks_per_snapshot();
        for client in &actual.clients {
            if let Some(seated) = &client.seated {
                for (header, _) in &client.snapshots {
                    // A re-seated client may have had another seat in an
                    // earlier flight. Its current flight retains its phase.
                    if header.flight == seated.flight {
                        assert_eq!(
                            u64::from(header.tick) % u64::from(tps),
                            crate::wire::snapshot_phase(seated.seat, tps)
                        );
                    }
                }
            }
        }
    }
}

fn ready_rig(executor: Executor) -> Rig {
    let mut rig = rig(executor);
    for _ in 0..700 {
        step(&mut rig);
    }
    assert!(rig.clients.iter().all(|client| client.seated.is_some()));
    rig
}

#[test]
fn snapshot_workers_preserve_failed_send_discard_and_recovery() {
    for executor in [Executor::parallel(4).unwrap(), Executor::shuffled(17)] {
        let mut expected = ready_rig(Executor::serial());
        let mut actual = ready_rig(executor);
        for rig in [&mut actual, &mut expected] {
            let connection = *rig.host.peers.keys().next().unwrap();
            let tick = rig.host.world.tick();
            let now = rig.net.now();
            let plane = rig.host.peers[&connection].plane.unwrap();
            let prepared = prepare_seat(&rig.host.world, plane).unwrap();
            let peer = &rig.host.peers[&connection];
            let previous = (peer.last_own_state, peer.unforeseen, peer.mismatch_answered);
            // The server has ended this connection but the session has not
            // consumed the close event yet, making send_payload fail.
            rig.host
                .server
                .disconnect(connection, DisconnectReason::ServerStopping);
            let out = std::mem::take(&mut rig.host.out);
            rig.host
                .snapshot(connection, tick, now, &out, &BTreeMap::new(), prepared)
                .unwrap();
            rig.host.out = out;
            let peer = &rig.host.peers[&connection];
            assert_eq!(
                (peer.last_own_state, peer.unforeseen, peer.mismatch_answered),
                previous
            );
            let mut discarded = peer.wire.clone();
            discarded.discard();
            assert_eq!(
                format!("{:?}", peer.wire),
                format!("{discarded:?}"),
                "a rejected send must already have discarded its staged packet"
            );
        }
        assert_eq!(bookkeeping(&actual.host), bookkeeping(&expected.host));
        for _ in 0..500 {
            compare_step(&mut actual, &mut expected);
        }
        for rig in [&mut actual, &mut expected] {
            rig.join(|client| client.callsign = "AfterFailure".into());
        }
        for _ in 0..700 {
            compare_step(&mut actual, &mut expected);
        }
        assert!(actual.clients.last().unwrap().seated.is_some());
    }
}

#[test]
fn snapshot_workers_keep_missing_and_duplicate_plane_skip_semantics() {
    let mut expected = ready_rig(Executor::serial());
    let mut actual = ready_rig(Executor::parallel(4).unwrap());
    for rig in [&mut actual, &mut expected] {
        let ids: Vec<_> = rig.host.peers.keys().copied().collect();
        let plane = rig.host.peers[&ids[0]].plane;
        // Co-locate phases to exercise all edge jobs in one ordered scope.
        for peer in rig.host.peers.values_mut() {
            peer.seat = Some(SeatId(0));
        }
        rig.host.peers.get_mut(&ids[1]).unwrap().plane = None;
        rig.host.peers.get_mut(&ids[2]).unwrap().plane = Some(PlaneId(u32::MAX));
        rig.host.peers.get_mut(&ids[3]).unwrap().plane = plane;
        let tps = rig.host.config.ticks_per_snapshot();
        let tick = rig.host.world.tick();
        let tick = tick + (u64::from(tps) - tick % u64::from(tps)) % u64::from(tps);
        let out = std::mem::take(&mut rig.host.out);
        rig.host.snapshots(tick, rig.net.now(), &out);
        rig.host.out = out;
    }
    let drain = |rig: &mut Rig| std::iter::from_fn(|| rig.host.poll_transmit()).collect::<Vec<_>>();
    assert_eq!(drain(&mut actual), drain(&mut expected));
    assert_eq!(bookkeeping(&actual.host), bookkeeping(&expected.host));
}
