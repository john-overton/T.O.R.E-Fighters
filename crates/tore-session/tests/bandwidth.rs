//! Bytes per snapshot on a 15 against 15 mission, against the plan's
//! bandwidth budget (docs/multiplayer-plan.md, "Bandwidth budget").
//!
//! Reads a real import through `TORE_DATA_DIR`, builds the mission with
//! `World::new`, flies it for three minutes with the player holding a gentle
//! turn, and encodes seat 0's snapshot packets as a host would: the entities
//! with their relevance, the seat's events, and the exact own state once a
//! second and whenever its ownship terms change. Three connections are coded
//! side by side: a clean link and one losing 5 percent of its packets, both
//! keeping 256 bytes of each packet for reliable messages as in flight, and a
//! clean one keeping none; each is acknowledged 100 ms later.
//!
//! The flight data link (slice G7) rides along: the readout carries the
//! seat's share, the seat's Link events go with its events, and the player
//! sorts its wing (Alt+A) every 5 seconds while it leads, so its flight holds
//! assignments.
//! The link's own parts and events are counted apart.
//!
//! ```sh
//! TORE_DATA_DIR=$PWD/.local/mpb-data-wire cargo test --release -p tore-session \
//!     --test bandwidth -- --ignored --nocapture
//! ```

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use tore_formats::aircraft::AircraftId;
use tore_net::SplitMix64;
use tore_session::wire::connection::HostConnection;
use tore_session::wire::entity::EntityKind;
use tore_session::wire::events::WireEvent;
use tore_session::wire::from_world;
use tore_session::wire::messages::Message;
use tore_session::wire::names::NameTable;
use tore_session::wire::snapshot::SnapshotHeader;
use tore_world::mission::{MissionSpec, Skill, Start};
use tore_world::readout::PlainBits;
use tore_world::seats::{PlaneId, SeatId, SeatInput};
use tore_world::world::plane::{ExactState, OwnPlane, OwnshipTerms};
use tore_world::world::{Seating, TickOutput, World};

const TICKS_PER_SNAPSHOT: u64 = 4;
const MINUTES: u64 = 3;
/// Snapshots before an acknowledgement comes back (100 ms).
const ACK_DELAY: u64 = 3;

#[derive(Default)]
struct Stat {
    values: Vec<usize>,
}

impl Stat {
    fn push(&mut self, value: usize) {
        self.values.push(value);
    }

    fn line(&self, name: &str) -> String {
        if self.values.is_empty() {
            return format!("{name:<28} none");
        }
        let min = self.values.iter().min().unwrap();
        let max = self.values.iter().max().unwrap();
        let mean = self.values.iter().sum::<usize>() as f64 / self.values.len() as f64;
        format!(
            "{name:<28} n {:>6}  min {min:>5}  mean {mean:>8.1}  max {max:>5}",
            self.values.len()
        )
    }
}

#[derive(Default)]
struct Link {
    host: Option<HostConnection>,
    loss: u64,
    /// Bytes kept for reliable messages in each snapshot packet.
    messages: usize,
    pending: VecDeque<(u64, u16, bool)>,
    sequence: u16,
    snapshot: Stat,
    events: Stat,
    packet: Stat,
    own_state: Stat,
    entities_sent: Stat,
    entities_full: Stat,
    waiting: Stat,
    names_bytes: usize,
    total_bytes: usize,
    late_waits: usize,
    readout: Stat,
    readout_waiting: Stat,
    part_bits: [usize; tore_session::wire::readout::PART_COUNT],
    /// The data link's four readout parts per snapshot, bytes.
    link_parts: Stat,
    /// The Link events' bytes per snapshot packet that carried any.
    link_events: Stat,
}

impl Link {
    fn new(loss: u64, messages: usize) -> Self {
        Self {
            host: Some(HostConnection::new(TICKS_PER_SNAPSHOT as u32)),
            loss,
            messages,
            ..Self::default()
        }
    }

    fn host(&mut self) -> &mut HostConnection {
        self.host.as_mut().unwrap()
    }

    /// Sends the staged packet of `bytes` at snapshot `k`; a lost one is
    /// reported lost 33 packets on, a delivered one after the round trip.
    fn send(&mut self, k: u64, bytes: usize, rng: &mut SplitMix64) {
        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        self.host().sent(sequence);
        self.total_bytes += bytes;
        let lost = rng.below(100) < self.loss;
        let at = if lost { k + 11 } else { k + ACK_DELAY };
        self.pending.push_back((at, sequence, !lost));
    }

    fn hear(&mut self, k: u64) {
        let (due, later): (Vec<_>, Vec<_>) = self.pending.drain(..).partition(|(at, ..)| *at <= k);
        self.pending = later.into();
        for (_, sequence, delivered) in due {
            if delivered {
                self.host().delivered(sequence);
            } else {
                self.host().lost(sequence);
            }
        }
    }
}

fn spec() -> MissionSpec {
    let mut spec = MissionSpec::new("UKR", AircraftId::F18);
    for (index, wing) in spec.wings.iter_mut().enumerate() {
        wing.count = 5;
        wing.skill = Skill::Average;
        if index >= 3 {
            wing.aircraft = AircraftId::Mig29;
        }
    }
    spec.separation_nm = 10;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

#[test]
#[ignore = "reads a real import through TORE_DATA_DIR; run by hand"]
fn bytes_per_snapshot_on_a_15_against_15_mission() {
    let directory = tore_import::data_directory().expect("TORE_DATA_DIR");
    let resources = tore_import::load(&directory).expect("an imported pack");
    let mut world =
        World::new(&spec(), &resources, Seating::SinglePlayer).expect("the mission builds");
    let seat = SeatId(0);
    let player = PlaneId(0);
    let own_wing = world.roster.plane(player).unwrap().slot.wing;
    let flight: BTreeSet<u32> = world
        .roster
        .planes()
        .iter()
        .filter(|p| p.slot.wing == own_wing)
        .map(|p| p.id.0)
        .collect();
    println!(
        "planes {}, AI {}, theater UKR, 10 nm apart, {MINUTES} minutes",
        world.roster.planes().len(),
        world.ai_wings.as_ref().map_or(0, |ai| ai.len())
    );

    let mut rng = SplitMix64::new(30);
    let mut links = [Link::new(0, 256), Link::new(5, 256), Link::new(0, 0)];
    let mut out = TickOutput::default();
    let mut known_projectiles: BTreeSet<u32> = BTreeSet::new();
    let mut gun_last: HashMap<u32, u64> = HashMap::new();
    let mut known_effects: Vec<(u8, [i64; 3])> = Vec::new();
    let mut known_marks: usize = 0;
    let mut last_terms: Option<OwnshipTerms> = None;
    let mut last_own_state_tick = 0u64;
    let mut peak_entities = BTreeMap::new();
    let ticks = MINUTES * 60 * 120;
    let mut plain = Stat::default();
    let mut busiest = 0;
    let mut previous_picture = from_world::seat_picture(&world, player).unwrap();
    let mut link_kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut sorts: BTreeMap<bool, usize> = BTreeMap::new();
    for _ in 0..ticks {
        let tick = world.tick();
        // The lead sorts its wing every 5 seconds.
        let commands = if tick >= 600 && tick.is_multiple_of(600) {
            vec![tore_world::seats::SeatCommand::WingOrder(
                tore_sim::ai::wing::PlayerOrder::Sort,
            )]
        } else {
            Vec::new()
        };
        let input = SeatInput {
            seat,
            tick,
            commands,
            pilot: tore_sim::flight::PilotInput {
                roll: if (tick / 1_200).is_multiple_of(2) {
                    0.1
                } else {
                    -0.1
                },
                pitch: 0.05,
                ..Default::default()
            },
            ..SeatInput::default()
        };
        world.step(&[input], &mut out).expect("the tick steps");
        let tick = world.tick() - 1;
        let tick32 = tick as u32;
        let picture = from_world::seat_picture(&world, player).unwrap();

        // The tick's events for seat 0, as a host sorts them.
        let mut events: Vec<(WireEvent, bool)> = Vec::new();
        let mut names = NameTable::new();
        for cue in &out.cues {
            if let Some(event) = from_world::cue_event(cue, seat, &mut names).unwrap() {
                events.push((event, true));
            }
        }
        for release in &out.releases {
            if release.seat == seat {
                events.push((
                    from_world::release_event(release, &mut names).unwrap(),
                    true,
                ));
            }
        }
        for reply in &out.orders {
            if reply.seat == seat && reply.order == tore_sim::ai::wing::PlayerOrder::Sort {
                let given = matches!(reply.outcome, tore_world::world::OrderOutcome::Given { .. });
                *sorts.entry(given).or_insert(0usize) += 1;
            }
            if reply.seat == seat {
                events.push((from_world::order_event(reply), false));
            }
        }
        for emission in &out.emissions {
            events.push((from_world::sound_event(emission, None), false));
        }
        // The data link's changes about the player's flight, as the host
        // drains them.
        let journal = world.datalink.take_journal();
        for event in from_world::link_events(&world.datalink, player.0, &journal) {
            if let WireEvent::Link(change) = &event {
                let name = format!("{change:?}");
                let name = name.split([' ', '{']).next().unwrap_or_default().to_owned();
                *link_kinds.entry(name).or_insert(0usize) += 1;
            }
            events.push((event, true));
        }
        for projectile in &picture.projectiles {
            if projectile.gun {
                if gun_last.insert(projectile.owner, tick).is_none() {
                    events.push((
                        WireEvent::GunBurst {
                            shooter: projectile.owner,
                            station: 0,
                            length: None,
                        },
                        false,
                    ));
                }
            } else if known_projectiles.insert(projectile.id) {
                events.push((
                    WireEvent::Launch {
                        shooter: projectile.owner,
                        projectile: projectile.id,
                        weapon: names.intern(&projectile.weapon).unwrap(),
                    },
                    false,
                ));
            }
        }
        let ended: Vec<u32> = gun_last
            .iter()
            .filter(|(_, last)| tick - **last > 12)
            .map(|(owner, _)| *owner)
            .collect();
        for owner in ended {
            let _ = gun_last.remove(&owner);
            events.push((
                WireEvent::GunBurst {
                    shooter: owner,
                    station: 0,
                    length: Some(12),
                },
                false,
            ));
        }
        let effects: Vec<(u8, [i64; 3])> = picture
            .effects
            .iter()
            .map(|e| (e.kind as u8, e.position.map(|v| (v * 32.) as i64)))
            .collect();
        for (effect, key) in picture.effects.iter().zip(&effects) {
            if !known_effects.contains(key) {
                events.push((from_world::effect_event(effect), false));
            }
        }
        known_effects = effects;
        for mark in picture.marks.iter().skip(known_marks) {
            events.push((from_world::mark_event(mark), false));
        }
        known_marks = picture.marks.len();
        for note in world.combat.state.take_device_notes() {
            if let tore_sim::combat::live::DeviceNote::Released(release) = note {
                events.push((from_world::countermeasure_event(&release), false));
            }
        }

        let terms = out
            .terms
            .iter()
            .find(|(p, _)| *p == player)
            .map(|(_, t)| *t);
        let terms_changed = terms != last_terms;
        last_terms = terms;

        for link in &mut links {
            // Names: the per-tick table above only shaped the events; each
            // link interns into its own.
            let remap = |event: &WireEvent, host: &mut HostConnection| -> WireEvent {
                let mut event = event.clone();
                let intern = |index: &mut tore_session::wire::names::NameIndex,
                              host: &mut HostConnection| {
                    let name = names.name(*index).unwrap().to_owned();
                    *index = host.names.intern(&name).unwrap();
                };
                match &mut event {
                    WireEvent::Radio { stems, .. } | WireEvent::OrderVoice { stems } => {
                        for stem in stems {
                            intern(stem, host);
                        }
                    }
                    WireEvent::Tower { stem: Some(stem) } => intern(stem, host),
                    WireEvent::Release { sound, .. } => intern(sound, host),
                    WireEvent::Launch { weapon, .. } => intern(weapon, host),
                    _ => {}
                }
                event
            };
            let mut link_event_bits = 0;
            for (event, _) in &events {
                let host = link.host.as_mut().unwrap();
                let event = remap(event, host);
                if matches!(event, WireEvent::Link(_)) {
                    let mut w = tore_codec::BitWriter::new();
                    event.write(&mut w).unwrap();
                    link_event_bits += w.bit_len();
                }
                host.event(tick32, &event).unwrap();
            }
            if link_event_bits > 0 {
                link.link_events.push(link_event_bits.div_ceil(8));
            }
        }

        if !tick.is_multiple_of(TICKS_PER_SNAPSHOT) {
            previous_picture = picture;
            continue;
        }
        let k = tick / TICKS_PER_SNAPSHOT;
        let own = world.cockpits[0].flight.position;
        let exact = ExactState::of(&OwnPlane::of(&world.cockpits[0]), terms.as_ref());
        let readout = world
            .cockpit_readout(
                seat,
                tore_world::combat::launcher(&world.cockpits[0].flight),
            )
            .expect("seat 0 flies");
        plain.push(readout.plain_bits().div_ceil(8));
        busiest =
            busiest.max(readout.sensors.contacts.len() + readout.visual.len() + readout.map.len());
        let header = SnapshotHeader {
            flight: 1,
            tick: tick32,
            input_received: tick32,
            input_margin: 3,
            inputs_repeated: 0,
            commands_applied: 0,
            own_hash: Some(exact.hash().unwrap()),
        };
        let own_state_due = terms_changed || tick - last_own_state_tick >= 120;
        if own_state_due {
            last_own_state_tick = tick;
        }
        for link in &mut links {
            link.hear(k);
            let host = link.host.as_mut().unwrap();
            let mut entities =
                from_world::entities(&picture, Some(&previous_picture), player.0, &mut host.names)
                    .unwrap();
            entities.sort_by_key(|e| e.key());
            let with_relevance: Vec<_> = entities
                .iter()
                .map(|e| {
                    let mut relevance = from_world::distance_relevance(e, own, player.0);
                    relevance.own_flight =
                        e.state.kind() == EntityKind::Aircraft && flight.contains(&e.id);
                    (*e, relevance)
                })
                .collect();
            for kind in EntityKind::ALL {
                let count = entities.iter().filter(|e| e.state.kind() == kind).count();
                let peak = peak_entities.entry(format!("{kind:?}")).or_insert(0usize);
                *peak = (*peak).max(count);
            }
            if let Some(names) = host.names.take_new() {
                link.names_bytes += Message::Names(names).encode().unwrap().len();
            }
            let packet = host
                .snapshot_with_readout(&header, &with_relevance, Some(&readout), link.messages)
                .unwrap();
            if let Some(report) = packet.readout {
                link.readout.push(report.bits.div_ceil(8));
                link.readout_waiting.push(report.waiting);
                let mut link_bits = 0;
                for (index, bits) in report.part_bits.iter().enumerate() {
                    link.part_bits[index] += bits;
                    if tore_session::wire::readout::part_name(
                        tore_session::wire::readout::PARTS[index],
                    )
                    .starts_with("link")
                    {
                        link_bits += bits;
                    }
                }
                link.link_parts.push(link_bits.div_ceil(8));
            }
            let bytes = packet.bytes();
            assert!(
                bytes <= tore_net::MAX_DATAGRAM,
                "snapshot {k}: a packet of {bytes} bytes"
            );
            link.snapshot.push(packet.snapshot.len());
            if let Some(events) = &packet.events {
                link.events.push(events.len());
            }
            link.entities_sent.push(packet.entities.sent);
            link.entities_full.push(packet.entities.full);
            link.waiting.push(packet.entities.waiting);
            if packet.entities.waiting > 0 && k > 30 {
                link.late_waits += 1;
                if link.late_waits <= 2 {
                    println!(
                        "snapshot {k}: {} waiting, section {} of a {}-byte share, events {:?}",
                        packet.entities.waiting,
                        packet.snapshot.len(),
                        packet.shares.entities,
                        packet.events.as_ref().map(Vec::len)
                    );
                }
            }
            link.packet.push(bytes);
            link.send(k, bytes, &mut rng);
            if own_state_due {
                let section = link.host().own_state(tick32, &exact).unwrap();
                link.own_state.push(section.len());
                let bytes = tore_net::packet::PAYLOAD_HEADER_LEN
                    + tore_net::packet::SECTION_HEADER_LEN
                    + section.len();
                link.send(k, bytes, &mut rng);
            }
        }
        previous_picture = picture;
    }

    println!("peak entities per kind: {peak_entities:?}");
    println!(
        "sorts given {}, refused {} (the player's plane is lost about 20 s in)",
        sorts.get(&true).unwrap_or(&0),
        sorts.get(&false).unwrap_or(&0)
    );
    println!("data link events about the player's flight: {link_kinds:?}");
    println!("{}", plain.line("readout plain bytes"));
    println!("most scope, visual and map contacts at once: {busiest}");
    let seconds = (ticks as f64) / 120.;
    for link in &links {
        println!(
            "--- link with {} percent loss, {} bytes kept for messages ---",
            link.loss, link.messages
        );
        for (name, stat) in [
            ("snapshot section bytes", &link.snapshot),
            ("events section bytes", &link.events),
            ("own state section bytes", &link.own_state),
            ("snapshot packet bytes", &link.packet),
            ("entity records sent", &link.entities_sent),
            ("of them in full", &link.entities_full),
            ("due entities waiting", &link.waiting),
            ("readout record bytes", &link.readout),
            ("readout changes waiting", &link.readout_waiting),
        ] {
            println!("{}", stat.line(name));
        }
        println!(
            "snapshots after the first second with an entity waiting: {}",
            link.late_waits
        );
        let snapshots = link.readout.values.len().max(1) as f64;
        let mut parts: Vec<String> = Vec::new();
        for (index, part) in tore_session::wire::readout::PARTS.iter().enumerate() {
            let mean = link.part_bits[index] as f64 / snapshots / 8.;
            if mean >= 0.5 {
                parts.push(format!(
                    "{} {mean:.1}",
                    tore_session::wire::readout::part_name(*part)
                ));
            }
        }
        println!("readout mean bytes by part: {}", parts.join(", "));
        println!("{}", link.link_parts.line("data link parts bytes"));
        println!("{}", link.link_events.line("data link event bytes"));
        let link_bytes = link.link_parts.values.iter().sum::<usize>()
            + link.link_events.values.iter().sum::<usize>();
        // After the first second, which brings the whole share across.
        let settled = &link.link_parts.values[30.min(link.link_parts.values.len())..];
        let busiest_second = settled
            .windows(30)
            .map(|w| w.iter().sum::<usize>())
            .max()
            .unwrap_or(0);
        println!(
            "data link: {:.0} bytes/s on average; after the first second the most in one snapshot {} \
             bytes (of the readout's {}) and in one second {busiest_second} bytes",
            link_bytes as f64 / seconds,
            settled.iter().max().copied().unwrap_or(0),
            tore_session::wire::space::READOUT_SHARE
        );
        println!(
            "names messages {} bytes; download {:.1} KB/s ({:.0} kbit/s) without the transport's messages",
            link.names_bytes,
            link.total_bytes as f64 / seconds / 1_000.,
            link.total_bytes as f64 * 8. / seconds / 1_000.
        );
    }
}
