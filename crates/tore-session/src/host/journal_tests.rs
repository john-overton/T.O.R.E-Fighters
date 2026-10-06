//! Slice K1's tests (docs/ARCHITECTURE.md, "How stage K lands"): the
//! journal's driver makes the changes before a step only at the step; on
//! the network simulator a host and three bots fly a crowd fight with an
//! `ai-slot` revival, scoring on and a late joiner, and the journal, coded
//! through the standby stream and replayed over the Flight record's world,
//! codes to the host's checkpoint every 30 ticks, as does a checkpoint at a
//! tick with the journal from there; and every state part restores into a
//! fresh host's structures and codes to the same bytes. Synthetic resources.

use super::super::lobby::state::PlayerState;
use super::super::*;
use super::{Driver, JournalRecord, MAX_QUEUED};
use crate::client::{Client, ClientConfig, ClientPhase, Controls};
use crate::journal::{
    Change, Part, Record, StreamReader, StreamWriter, Ticks, apply_tick, check_hash, drain,
};
use crate::settings::{Respawn, number};
use tore_formats::aircraft::AircraftId;
use tore_net::Entropy;
use tore_net::sim::{LinkConfig, SimNetwork, SimSocket};
use tore_sim::flight::PilotCommand;
use tore_world::mission::{Skill, Start};
use tore_world::snapshot::RenderSnapshot;
use tore_world::test_support::resources::{THEATER, resources};
use tore_world::world::revive::RevivalWeapons;

const MS: Duration = Duration::from_millis(1);

fn host_address() -> SocketAddr {
    "10.0.0.1:26900".parse().unwrap()
}

fn build() -> BuildId {
    BuildId {
        version: "0.1.3-1-gtest".into(),
        commit: "test-commit".into(),
        release: false,
    }
}

/// The crowd fight's shape: five of ours (three bots, the AI plane an
/// `ai-slot` revival takes and a late joiner's) against four bandits 2 nm
/// ahead at 10,000 feet.
fn crowd_spec() -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 5;
    spec.wings[3].count = 4;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 2;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

fn config() -> HostConfig {
    HostConfig {
        entropy: Entropy::Seeded(11),
        open_planes: OpenPlanes::All,
        // The first ready player starts the mission, so the journal records
        // its Flight.
        start: StartMode::FirstPlayer,
        ..HostConfig::new(build())
    }
}

/// One bot's game: the client and the bot's pilot, chasing and firing at
/// the other side; one ejects at `eject_at` and flies again by the revival
/// rules as soon as it may.
struct Bot {
    socket: SimSocket,
    client: Client,
    pilot: crate::bot::ScriptedPilot,
    picture: Option<RenderSnapshot>,
    last_frame: Option<Duration>,
    eject_at: Option<Duration>,
    ejected: bool,
    revived: bool,
    /// Its game has gone silent: nothing is stepped or sent (a drop).
    silent: bool,
}

impl Bot {
    fn controls(&mut self, now: Duration) -> Controls {
        let Some(prediction) = self.client.prediction() else {
            return Controls::default();
        };
        let enemies = crate::bot::enemies(&self.client);
        let mut controls = self.pilot.controls(
            now,
            &prediction.plane().flight,
            self.picture.as_ref(),
            &|id| enemies.contains(&id),
        );
        if !self.ejected && self.eject_at.is_some_and(|at| now >= at) {
            controls.pilot.commands.push(PilotCommand::Eject);
            self.ejected = true;
        }
        controls
    }
}

/// A host and its bots on one simulated network, the journal recorded and
/// drained as the standby stream would.
struct Rig {
    net: SimNetwork,
    host: Host,
    socket: SimSocket,
    bots: Vec<Bot>,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    next_port: u16,
    /// Every record drained so far.
    records: Vec<JournalRecord>,
    /// The host's checkpoint hash between ticks, by the next tick, every 30
    /// ticks; whole checkpoints at the ticks asked for.
    hashes: BTreeMap<u64, u64>,
    keep_at: Vec<u64>,
    kept: BTreeMap<u64, Vec<u8>>,
    /// The flight's spec text, once it flies.
    spec_text: Option<String>,
}

impl Rig {
    fn new(keep_at: Vec<u64>) -> Self {
        let net = SimNetwork::new(23);
        net.set_default_link(LinkConfig::for_round_trip(40 * MS, 0., 0., 0.));
        let socket = net.bind(host_address()).unwrap();
        let resources = Arc::new(resources());
        let mut host = Host::new(crowd_spec(), Arc::clone(&resources), config()).unwrap();
        // The `ai-slot` rule with half the gun's rounds, so its store cut
        // changes the world, and no wait.
        host.settings
            .apply(&[
                (number::RESPAWN, Respawn::AiSlot.value()),
                (number::REVIVE_WEAPONS, RevivalWeapons::HalfGuns.value()),
                (number::REVIVE_DELAY, 0),
            ])
            .unwrap();
        host.record_journal(true);
        Self {
            net,
            host,
            socket,
            bots: Vec::new(),
            resources,
            next_port: 41_000,
            records: Vec::new(),
            hashes: BTreeMap::new(),
            keep_at,
            kept: BTreeMap::new(),
            spec_text: None,
        }
    }

    fn join(&mut self, callsign: &str, plane: u32, eject_at: Option<Duration>) {
        self.join_with(callsign, plane, eject_at, None);
    }

    /// A join that may send the rejoin `token` (stage K, slice K5).
    fn join_with(
        &mut self,
        callsign: &str,
        plane: u32,
        eject_at: Option<Duration>,
        token: Option<tore_net::Token>,
    ) {
        let address: SocketAddr = format!("10.0.0.2:{}", self.next_port).parse().unwrap();
        let socket = self.net.bind(address).unwrap();
        let config = ClientConfig {
            entropy: Entropy::Seeded(u64::from(self.next_port)),
            plane: Some(plane),
            token,
            ..ClientConfig::new(host_address(), callsign, build())
        };
        self.next_port += 1;
        let client = Client::connect(config, Arc::clone(&self.resources), self.net.now()).unwrap();
        self.bots.push(Bot {
            socket,
            client,
            pilot: crate::bot::ScriptedPilot::new(),
            picture: None,
            last_frame: None,
            eject_at,
            ejected: false,
            revived: false,
            silent: false,
        });
    }

    /// One millisecond for everyone.
    fn step(&mut self) {
        self.net.advance(MS);
        let now = self.net.now();
        let before = self.host.world.tick();
        self.host.receive_from(now, &mut self.socket).unwrap();
        self.host.update(now);
        self.host.transmit(&mut self.socket).unwrap();
        while self.host.poll_log().is_some() {}
        let (records, lost) = self.host.drain_journal();
        assert!(!lost, "the journal dropped records");
        self.records.extend(records);
        let tick = self.host.world.tick();
        if tick != before && matches!(self.host.life, Life::Flying) {
            if self.spec_text.is_none() {
                self.spec_text = Some(self.host.spec_text.clone());
            }
            if tick.is_multiple_of(30) {
                self.hashes
                    .insert(tick, check_hash(&self.host.world).unwrap());
            }
            if self.keep_at.contains(&tick) {
                self.kept
                    .insert(tick, self.host.world.checkpoint().unwrap());
            }
        }
        for bot in &mut self.bots {
            if bot.silent {
                continue;
            }
            bot.client.receive_from(now, &mut bot.socket).unwrap();
            let controls = bot.controls(now);
            bot.client.update(now, &controls);
            if bot
                .last_frame
                .is_none_or(|last| now - last >= Duration::from_millis(16))
            {
                bot.last_frame = Some(now);
                if let Some(frame) = bot.client.frame(now) {
                    bot.picture = Some(frame.picture);
                }
            }
            if bot.ejected && !bot.revived && bot.client.may_fly_again() {
                bot.client.revive();
                bot.revived = true;
            }
            bot.client.transmit(&mut bot.socket).unwrap();
            while let Some(event) = bot.client.poll_event() {
                if std::env::var_os("K1_DEBUG").is_some() {
                    eprintln!("{:?} {event:?}", now);
                }
            }
        }
    }

    fn run_until(&mut self, limit: Duration, mut done: impl FnMut(&Rig) -> bool) -> bool {
        let end = self.net.now() + limit;
        while self.net.now() < end {
            self.step();
            if done(self) {
                return true;
            }
        }
        false
    }

    fn run(&mut self, time: Duration) {
        let end = self.net.now() + time;
        while self.net.now() < end {
            self.step();
        }
    }

    fn flying(&self, bot: usize) -> bool {
        self.bots[bot].client.phase() == ClientPhase::Flying
    }
}

/// The journal's records through the standby stream's coders, a snapshot
/// interval of ticks (four) at a time, as a standby reads them.
fn through_the_stream(records: &[JournalRecord]) -> Vec<Record> {
    let mut writer = StreamWriter::new();
    let mut reader = StreamReader::new();
    let mut out = Vec::new();
    let mut ticks: Vec<Tick> = Vec::new();
    let mut send = |record: Record, out: &mut Vec<Record>| {
        let bytes = writer.encode(&record).unwrap();
        out.push(reader.decode(&bytes).unwrap());
    };
    for record in records {
        let record = match record {
            JournalRecord::Tick(tick) => {
                ticks.push(tick.clone());
                if ticks.len() < 4 {
                    continue;
                }
                None
            }
            JournalRecord::Flight(flight) => Some(Record::Flight(*flight)),
            JournalRecord::State(part) => Some(Record::State(part.clone())),
            JournalRecord::Ended(reason) => Some(Record::Ended(*reason)),
        };
        if let Some(first) = ticks.first() {
            let first = first.tick as u32;
            let ticks = std::mem::take(&mut ticks);
            send(Record::Ticks(Ticks { first, ticks }), &mut out);
        }
        if let Some(record) = record {
            send(record, &mut out);
        }
    }
    if let Some(first) = ticks.first() {
        let first = first.tick as u32;
        send(Record::Ticks(Ticks { first, ticks }), &mut out);
    }
    out
}

/// Replays the stream's ticks over `world` from its tick on, comparing
/// its checkpoint with the host's every 30 ticks; the ticks compared.
fn replay(world: &mut World, stream: &[Record], hashes: &BTreeMap<u64, u64>) -> usize {
    let mut out = TickOutput::default();
    let mut compared = 0;
    for record in stream {
        let Record::Ticks(ticks) = record else {
            continue;
        };
        for tick in &ticks.ticks {
            if tick.tick < world.tick() {
                continue;
            }
            apply_tick(world, tick, &mut out).unwrap();
            let _ = drain(world);
            if let Some(expected) = hashes.get(&world.tick()) {
                assert_eq!(
                    check_hash(world).unwrap(),
                    *expected,
                    "the replay differs from the host after tick {}",
                    tick.tick
                );
                compared += 1;
            }
        }
    }
    compared
}

/// Every part the host codes now restores into a fresh host's structures,
/// which code to the same bytes; the players restore as the host's.
fn parts_restore(host: &Host) {
    let mut fresh = Host::new(crowd_spec(), Arc::clone(&host.resources), config()).unwrap();
    fresh.now = host.now;
    let mut ids = |order: u64| ConnectionId(1_000 + order as u32);
    // The players, in join order, each under the connection given for it.
    let bytes = host.encode_part(Part::Players).unwrap();
    let restored = fresh.restore_players(&bytes, &mut ids).unwrap();
    let mut expected: Vec<PlayerState> = host.peers.values().map(PlayerState::of).collect();
    expected.sort_by_key(|player| player.lobby.order);
    assert_eq!(
        restored.iter().map(|(_, p)| p.clone()).collect::<Vec<_>>(),
        expected
    );
    for (id, player) in restored {
        assert_eq!(id, ids(player.lobby.order));
        let clock = super::super::state::Clock {
            old: host.now,
            new: fresh.now,
        };
        let peer = player.into_peer(clock, fresh.config.ticks_per_snapshot());
        fresh.peers.insert(id, peer);
    }
    assert_eq!(fresh.encode_part(Part::Players).unwrap(), bytes);
    let session = host.encode_part(Part::Session).unwrap();
    let old_clock = fresh.session_clock(&session).unwrap();
    assert_eq!(old_clock, host.now);
    for part in [
        Part::Session,
        Part::Court,
        Part::Scores,
        Part::Revivals,
        Part::Rejoin,
    ] {
        let bytes = host.encode_part(part).unwrap();
        fresh
            .restore_part(part, &bytes, &mut ids, old_clock)
            .unwrap();
        assert_eq!(
            fresh.encode_part(part).unwrap(),
            bytes,
            "the {part:?} part restores differently"
        );
    }
    assert_eq!(fresh.settings, host.settings);
    assert_eq!(fresh.session_id, host.session_id);
    assert_eq!(fresh.spec, host.spec);
    assert_eq!(fresh.spec_text, host.spec_text);
    for order in 1..=4 {
        assert_eq!(fresh.score.player(order), host.score.player(order));
    }
}

/// The fight: three bots from the start, the second ejecting at `eject`
/// and flying again in an AI aircraft, a fourth joining at `late`, for
/// `seconds` in all; then the checks.
fn crowd_fight(seconds: u64, eject: Duration, late: Duration, keep_at: Vec<u64>) {
    let mut rig = Rig::new(keep_at.clone());
    rig.join("Viper", 0, None);
    rig.join("Cobra", 1, Some(eject));
    rig.join("Hawk", 2, None);
    assert!(
        rig.run_until(Duration::from_secs(5), |r| (0..3).all(|b| r.flying(b))),
        "the bots fly"
    );
    // The lobby's parts, then the Flight before any tick.
    let flight = rig
        .records
        .iter()
        .position(|r| matches!(r, JournalRecord::Flight(_)))
        .expect("the Flight");
    assert!(
        rig.records[..flight]
            .iter()
            .all(|r| matches!(r, JournalRecord::State(_)))
    );
    // The revival: Cobra flies again in the first free AI aircraft of its
    // wing, its stores cut.
    let limit = (eject + Duration::from_secs(15)).saturating_sub(rig.net.now());
    let revived = rig.run_until(limit, |r| {
        r.bots[1]
            .client
            .seat()
            .is_some_and(|(_, plane)| plane == PlaneId(3))
    });
    assert!(revived, "Cobra flies again in plane 3");
    rig.run(late.saturating_sub(rig.net.now()));
    rig.join("Mako", 4, None);
    assert!(
        rig.run_until(Duration::from_secs(5), |r| r.flying(3)),
        "the late joiner flies"
    );
    // Mako's game goes silent: the host drops it and keeps its plane for it
    // (the rejoin part holds the reservation), and its game comes back with
    // its token and flies that plane again.
    let token = rig.bots[3].client.token().expect("Mako's token").token;
    let kept = rig.bots[3].client.seat().map(|(_, plane)| plane).unwrap();
    rig.bots[3].silent = true;
    assert!(
        rig.run_until(Duration::from_secs(8), |r| {
            r.host.rejoin.reserved.values().any(|r| r.plane == kept)
        }),
        "Mako's plane is kept for it"
    );
    rig.run(Duration::from_millis(300));
    parts_restore(&rig.host);
    rig.join_with("Mako", 4, None, Some(token));
    assert!(
        rig.run_until(Duration::from_secs(6), |r| r.flying(4)),
        "Mako rejoins"
    );
    assert_eq!(rig.bots[4].client.seat().map(|(_, p)| p), Some(kept));
    assert!(rig.host.rejoin.reserved.is_empty());
    parts_restore(&rig.host);
    let end = Duration::from_secs(seconds);
    rig.run(end.saturating_sub(rig.net.now()));
    assert!([0, 1, 2, 4].iter().all(|&b| rig.flying(b)));
    parts_restore(&rig.host);

    // What the journal recorded.
    let ticks: Vec<&Tick> = rig
        .records
        .iter()
        .filter_map(|r| match r {
            JournalRecord::Tick(tick) => Some(tick),
            _ => None,
        })
        .collect();
    assert_eq!(ticks[0].tick, 0);
    assert!(ticks.windows(2).all(|w| w[1].tick == w[0].tick + 1));
    assert_eq!(
        ticks[0].changes,
        [Change::Scoring(true)],
        "scoring at tick 0"
    );
    let cuts: Vec<&&Tick> = ticks
        .iter()
        .filter(|t| {
            t.changes
                .iter()
                .any(|c| matches!(c, Change::StoreCut { .. }))
        })
        .collect();
    assert_eq!(cuts.len(), 1, "the revival's store cut");
    assert_eq!(
        cuts[0].changes,
        [Change::StoreCut {
            plane: PlaneId(3),
            weapons: RevivalWeapons::HalfGuns
        }]
    );
    assert!(
        cuts[0]
            .mission
            .iter()
            .any(|c| matches!(c, MissionCommand::Take { plane, .. } if *plane == PlaneId(3))),
        "the cut and the take in one tick"
    );
    assert!(ticks.iter().any(|t| t.inputs.len() == 4), "the late joiner");
    let parts: BTreeSet<Part> = rig
        .records
        .iter()
        .filter_map(|r| match r {
            JournalRecord::State(part) => Some(part.part),
            _ => None,
        })
        .collect();
    assert_eq!(parts.len(), 6, "every part went out: {parts:?}");
    assert!(
        rig.host.revival.players.values().any(|p| p.used == 1),
        "the revival counted"
    );

    // The stream as a standby reads it, replayed over the Flight's world.
    let stream = through_the_stream(&rig.records);
    let Some(Record::Flight(flight)) = stream.iter().find(|r| matches!(r, Record::Flight(_)))
    else {
        panic!("the stream holds its Flight");
    };
    let spec_text = rig.spec_text.clone().unwrap();
    assert_eq!(flight.spec_hash, tore_codec::fnv1a64(spec_text.as_bytes()));
    let spec = MissionSpec::from_text(&spec_text).unwrap();
    let fresh = || World::new(&spec, &ResourceReads::new(&rig.resources), Seating::Open).unwrap();
    let mut twin = fresh();
    assert_eq!(flight.identity, twin.mission_identity());
    let compared = replay(&mut twin, &stream, &rig.hashes);
    assert_eq!(compared, rig.hashes.len(), "every 30th tick compared");
    assert!(compared >= seconds as usize * 4 - 8, "{compared} compared");
    assert_eq!(
        twin.checkpoint().unwrap(),
        rig.host.world.checkpoint().unwrap()
    );
    // A checkpoint at a tick, restored over a fresh world, with the
    // journal from that tick.
    for at in keep_at {
        let bytes = &rig.kept[&at];
        let mut world = fresh();
        world.restore(bytes).unwrap();
        assert_eq!(world.tick(), at);
        let compared = replay(&mut world, &stream, &rig.hashes);
        assert!(compared > 0);
        assert_eq!(
            world.checkpoint().unwrap(),
            rig.host.world.checkpoint().unwrap(),
            "from the checkpoint at {at}"
        );
    }
}

#[test]
fn a_crowd_fight_replays_from_its_journal_and_every_part_restores() {
    // 40 seconds: the eject at 6, the late joiner at 15; checkpoints before
    // the eject, around the revival and after the join.
    crowd_fight(
        40,
        Duration::from_secs(6),
        Duration::from_secs(15),
        vec![600, 1_200, 2_400],
    );
}

/// The acceptance at full length (5 minutes), for the full run: slow
/// (minutes in a debug build).
#[test]
#[ignore = "slow: the full suite runs it (slice K1's five-minute crowd fight)"]
fn a_five_minute_crowd_fight_replays_from_its_journal_and_every_part_restores() {
    crowd_fight(
        300,
        Duration::from_secs(30),
        Duration::from_secs(60),
        vec![1_200, 6_000, 18_000, 30_000],
    );
}

#[test]
fn the_driver_makes_a_change_only_at_the_step_and_records_it() {
    let resources = resources();
    let reads = ResourceReads::new(&resources);
    let world = World::new(&crowd_spec(), &reads, Seating::Open).unwrap();
    let mut driver = Driver::new(world);
    driver.set_scoring(true);
    assert!(!driver.scoring(), "not before the step");
    // A plane the AI does not fly is refused now, in the world's words, and
    // nothing is recorded.
    let refused = driver
        .cut_ai_stores(PlaneId(99), RevivalWeapons::Guns)
        .unwrap_err();
    assert_eq!(
        refused.to_string(),
        "the AI has no configuration for plane 99"
    );
    driver
        .cut_ai_stores(PlaneId(4), RevivalWeapons::Guns)
        .unwrap();
    assert_eq!(driver.pending().len(), 2);
    let checkpoint = driver.checkpoint().unwrap();
    let mut out = TickOutput::default();
    let (tick, notes) = driver.step(Tick::new(0), &mut out).unwrap();
    assert!(notes.is_empty());
    assert!(driver.scoring());
    assert_eq!(
        tick.changes,
        [
            Change::Scoring(true),
            Change::StoreCut {
                plane: PlaneId(4),
                weapons: RevivalWeapons::Guns
            }
        ]
    );
    assert!(driver.pending().is_empty());
    let _ = driver.take_facts();
    // The checkpoint before the step and the recorded tick give the same
    // world: the changes were not in the world before the step.
    let mut twin = World::new(&crowd_spec(), &reads, Seating::Open).unwrap();
    twin.restore(&checkpoint).unwrap();
    apply_tick(&mut twin, &tick, &mut out).unwrap();
    let _ = drain(&mut twin);
    assert_eq!(twin.checkpoint().unwrap(), driver.checkpoint().unwrap());
}

#[test]
fn a_host_keeps_no_records_unless_recording_and_never_more_than_its_bound() {
    let mut host = Host::new(
        crowd_spec(),
        Arc::new(resources()),
        HostConfig {
            start: StartMode::Now,
            ..config()
        },
    )
    .unwrap();
    for n in 1..=10 {
        host.update(Duration::from_millis(10 * n));
    }
    assert!(host.world.tick() > 0);
    assert!(
        host.drain_journal().0.is_empty(),
        "nothing while not recording"
    );
    host.record_journal(true);
    host.update(Duration::from_millis(120));
    let (records, lost) = host.drain_journal();
    assert!(!lost);
    assert!(records.iter().any(|r| matches!(r, JournalRecord::Tick(_))));
    // Every part goes out once recording starts, after the tick.
    assert_eq!(
        records
            .iter()
            .filter(|r| matches!(r, JournalRecord::State(_)))
            .count(),
        6
    );
    // A stream that never drains: the queue is dropped at its bound.
    for _ in 0..=MAX_QUEUED {
        host.journal
            .push(JournalRecord::Ended(EndReason::EndedByServer));
    }
    let (records, lost) = host.drain_journal();
    assert!(lost);
    assert_eq!(records.len(), 1);
    // The mission's end is recorded.
    host.end();
    let (records, _) = host.drain_journal();
    assert!(
        records
            .iter()
            .any(|r| matches!(r, JournalRecord::Ended(EndReason::EndedByServer)))
    );
}

#[test]
fn the_session_part_restores_its_settings_and_timers_on_another_clock() {
    // A server's own limits past the King's lists, a name and a password.
    let config = HostConfig {
        start: StartMode::Now,
        name: "Furball".into(),
        password: Some("secret".into()),
        max_players: 30,
        time_limit: Some(Duration::from_secs(4 * 3_600)),
        restart_delay: Duration::from_secs(20),
        settings: vec![(number::MODE, crate::settings::Mode::Pvp.value())],
        ..config()
    };
    let mut host = Host::new(crowd_spec(), Arc::new(resources()), config.clone()).unwrap();
    for n in 1..=10 {
        host.update(Duration::from_millis(10 * n));
    }
    host.end();
    let Life::Ended {
        next_at: Some(next_at),
        stop_at,
    } = host.life
    else {
        panic!("ended, with a next mission");
    };
    let bytes = host.encode_part(Part::Session).unwrap();
    let mut fresh = Host::new(crowd_spec(), Arc::new(resources()), config).unwrap();
    fresh.now = Duration::from_secs(1_000);
    let old_clock = fresh.session_clock(&bytes).unwrap();
    assert_eq!(old_clock, host.now);
    fresh
        .restore_part(
            Part::Session,
            &bytes,
            &mut |o| ConnectionId(o as u32),
            old_clock,
        )
        .unwrap();
    assert_eq!(fresh.settings, host.settings);
    assert_eq!(fresh.settings.mode(), crate::settings::Mode::Pvp);
    assert_eq!(fresh.settings.max_players(), 30);
    assert_eq!(fresh.settings.time_limit_seconds(), Some(4 * 3_600));
    assert_eq!(fresh.settings.password(), Some("secret"));
    let moved = |at: Duration| fresh.now + (at - host.now);
    match fresh.life {
        Life::Ended {
            next_at: Some(next),
            stop_at: stop,
        } => {
            assert_eq!(next, moved(next_at));
            assert_eq!(stop, moved(stop_at));
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(fresh.number, host.number);
    assert_eq!(fresh.joins, host.joins);
    // Coded again on the new clock, it reads the same but for the reading.
    fresh.now = host.now;
    fresh.life = host.life;
    assert_eq!(fresh.encode_part(Part::Session).unwrap(), bytes);
    // Damaged bytes are refused, never a panic.
    for cut in 0..bytes.len() {
        let _ = fresh.restore_part(
            Part::Session,
            &bytes[..cut],
            &mut |o| ConnectionId(o as u32),
            old_clock,
        );
    }
}
