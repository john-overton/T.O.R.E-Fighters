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
    let mut previous_picture = world.combat.render_snapshot().clone();
    for _ in 0..ticks {
        let tick = world.tick();
        let input = SeatInput {
            seat,
            tick,
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
        let picture = world.combat.render_snapshot().clone();

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
            if reply.seat == seat {
                events.push((from_world::order_event(reply), false));
            }
        }
        for emission in &out.emissions {
            events.push((from_world::sound_event(emission, None), false));
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
            for (event, _) in &events {
                let host = link.host.as_mut().unwrap();
                let event = remap(event, host);
                host.event(tick32, &event).unwrap();
            }
        }

        if !tick.is_multiple_of(TICKS_PER_SNAPSHOT) {
            previous_picture = picture;
            continue;
        }
        let k = tick / TICKS_PER_SNAPSHOT;
        let own = world.cockpits[0].flight.position;
        let exact = ExactState::of(&OwnPlane::of(&world.cockpits[0]), terms.as_ref());
        let header = SnapshotHeader {
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
                .snapshot(&header, &with_relevance, link.messages)
                .unwrap();
            let bytes = packet.bytes();
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
        ] {
            println!("{}", stat.line(name));
        }
        println!(
            "snapshots after the first second with an entity waiting: {}",
            link.late_waits
        );
        println!(
            "names messages {} bytes; download {:.1} KB/s ({:.0} kbit/s) without the transport's messages",
            link.names_bytes,
            link.total_bytes as f64 / seconds / 1_000.,
            link.total_bytes as f64 * 8. / seconds / 1_000.
        );
    }
}
