//! Slice K0's journal tests (docs/ARCHITECTURE.md, "How stage K lands"):
//! a world stepped by the host's path and a twin replaying the coded Ticks
//! records code to the same checkpoint bytes every 30 ticks for 1,200 ticks
//! of a crowd fight; the standby stream's records round trip in order and
//! are refused, never panicking, when damaged. Synthetic resources.

use super::journal::*;
use crate::wire::messages::{Message, StandbyMark, kind};
use crate::wire::samples;
use std::collections::BTreeMap;
use tore_formats::aircraft::AircraftId;
use tore_sim::combat::live;
use tore_sim::flight::PilotInput;
use tore_world::mission::{MissionSpec, Skill, Start};
use tore_world::resources::ResourceReads;
use tore_world::seats::{PlaneId, SeatCommand, SeatId, SeatInput, SeatView};
use tore_world::test_support::resources::{THEATER, resources};
use tore_world::world::{MissionCommand, Seating, TickOutput, World};

/// Four of ours, two of them human, against four bandits 2 nm ahead at
/// 10,000 feet: the crowd fight's shape on the session's synthetic import.
fn crowd_spec() -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 4;
    spec.wings[3].count = 4;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 2;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

fn fresh(spec: &MissionSpec, import: &BTreeMap<String, Vec<u8>>) -> World {
    World::new(spec, &ResourceReads::new(import), Seating::Open).unwrap()
}

/// What a human does at `tick`: a weave, the trigger in bursts, the radar's
/// designation, chaff and flares, and the odd wing order.
fn pilot(seat: u8, tick: u64) -> SeatInput {
    let t = tick as f64 / 120. + f64::from(seat);
    let commands = match tick % 240 {
        5 => vec![SeatCommand::Combat(live::Command::Designate)],
        60 => vec![SeatCommand::ReleaseChaff, SeatCommand::ReleaseFlare],
        90 => vec![SeatCommand::CycleWeapon { forward: true }],
        150 => vec![SeatCommand::WingOrder(
            tore_sim::ai::wing::PlayerOrder::EngageMyTarget,
        )],
        _ => Vec::new(),
    };
    SeatInput {
        seat: SeatId(seat),
        tick,
        pilot: PilotInput {
            pitch: (t * 0.7).sin() * 0.25,
            roll: (t * 0.45).cos() * 0.3,
            throttle: Some(0.9),
            ..PilotInput::default()
        },
        trigger: tick % 240 < 40,
        commands,
        view: Some(SeatView {
            tick: tick.saturating_sub(10),
            interpolation_delay: 8,
        }),
        ..SeatInput::default()
    }
}

/// The host's side of the fight: each tick's journal, built as the host
/// builds it, and the world stepped through [`apply_tick`] and drained.
/// Seat 0 and 1 take planes 0 and 1 at tick 0, seat 2 takes plane 2 at
/// tick 300, seat 1 gives its plane back at tick 900.
fn fly(world: &mut World, ticks: u64) -> (Vec<Tick>, BTreeMap<u64, Vec<u8>>) {
    let mut out = TickOutput::default();
    let mut journal = Vec::new();
    let mut checkpoints = BTreeMap::new();
    let mut flying: Vec<u8> = Vec::new();
    for _ in 0..ticks {
        let number = world.tick();
        let mut tick = Tick::new(number);
        if number == 0 {
            tick.changes.push(Change::Scoring(true));
        }
        let take = |seat: u8, plane: u32| MissionCommand::Take {
            seat: SeatId(seat),
            plane: PlaneId(plane),
        };
        match number {
            0 => {
                tick.mission = vec![take(0, 0), take(1, 1)];
                flying = vec![0, 1];
            }
            300 => {
                tick.mission = vec![take(2, 2)];
                flying.push(2);
            }
            900 => {
                tick.mission = vec![MissionCommand::GiveBack { seat: SeatId(1) }];
                flying.retain(|&s| s != 1);
            }
            _ => {}
        }
        for &seat in &flying {
            tick.push_input(pilot(seat, number), (number / 240) as u16);
        }
        apply_tick(world, &tick, &mut out).unwrap();
        let _ = drain(world);
        journal.push(tick);
        if world.tick().is_multiple_of(30) {
            checkpoints.insert(world.tick(), world.checkpoint().unwrap());
        }
    }
    (journal, checkpoints)
}

/// The records a standby would get for `journal`: a Flight, then a snapshot
/// interval of ticks at a time (4), each through its message's bytes.
fn stream(spec: &MissionSpec, world: &World, journal: &[Tick]) -> Vec<Vec<u8>> {
    let mut writer = StreamWriter::new();
    let mut records = vec![Record::Flight(FlightRecord {
        mission: 1,
        spec_hash: tore_codec::fnv1a64(spec.to_text().as_bytes()),
        identity: world.mission_identity(),
    })];
    for chunk in journal.chunks(4) {
        records.push(Record::Ticks(Ticks {
            first: chunk[0].tick as u32,
            ticks: chunk.to_vec(),
        }));
    }
    records
        .iter()
        .map(|record| {
            let message = Message::StandbyRecord(writer.encode(record).unwrap());
            let body = message.encode().unwrap();
            let Message::StandbyRecord(bytes) =
                Message::decode(kind::STANDBY_RECORD, &body).unwrap()
            else {
                panic!("not a standby record")
            };
            bytes
        })
        .collect()
}

#[test]
fn a_twin_replaying_the_coded_ticks_codes_to_the_hosts_checkpoint_every_30_ticks() {
    let import = resources();
    let spec = crowd_spec();
    let mut host = fresh(&spec, &import);
    let (journal, checkpoints) = fly(&mut host, 1_200);
    assert_eq!(checkpoints.len(), 40);
    let bytes = stream(&spec, &host, &journal);
    let mut twin = fresh(&spec, &import);
    let mut reader = StreamReader::new();
    let mut out = TickOutput::default();
    let mut compared = 0;
    for record in &bytes {
        match reader.decode(record).unwrap() {
            Record::Flight(flight) => assert_eq!(flight.identity, twin.mission_identity()),
            Record::Ticks(ticks) => {
                for tick in &ticks.ticks {
                    apply_tick(&mut twin, tick, &mut out).unwrap();
                    let _ = drain(&mut twin);
                    if let Some(expected) = checkpoints.get(&twin.tick()) {
                        assert!(
                            twin.checkpoint().unwrap() == *expected,
                            "the twin differs from the host at tick {}",
                            twin.tick()
                        );
                        compared += 1;
                    }
                }
            }
            other => panic!("unexpected record {other:?}"),
        }
    }
    assert_eq!(compared, 40);
    assert_eq!(check_hash(&twin).unwrap(), check_hash(&host).unwrap());
    // What the stream costs here (agent measure, K0): a stick moving every
    // tick costs about 24 bytes a seat a tick, its floats coded by the
    // checkpoint trait against the last; slice K3 measures the real stream.
    let total: usize = bytes.iter().map(Vec::len).sum();
    let seat_ticks: usize = journal.iter().map(|t| t.inputs.len()).sum();
    assert!(
        total < seat_ticks * 32,
        "{total} bytes for {seat_ticks} seat ticks"
    );
}

#[test]
fn a_tick_for_another_tick_or_with_unmatched_commands_is_refused_before_anything_changes() {
    let import = resources();
    let spec = crowd_spec();
    let mut world = fresh(&spec, &import);
    let before = world.checkpoint().unwrap();
    let mut out = TickOutput::default();
    assert!(apply_tick(&mut world, &Tick::new(1), &mut out).is_err());
    let mut unmatched = Tick::new(0);
    unmatched.inputs.push(pilot(0, 0));
    assert!(apply_tick(&mut world, &unmatched, &mut out).is_err());
    assert_eq!(world.checkpoint().unwrap(), before);
    assert_eq!(world.tick(), 0);
}

#[test]
fn the_scoring_change_turns_scoring_on_as_the_host_does() {
    let import = resources();
    let mut world = fresh(&crowd_spec(), &import);
    assert!(!world.scoring());
    let mut tick = Tick::new(0);
    tick.changes.push(Change::Scoring(true));
    apply_tick(&mut world, &tick, &mut TickOutput::default()).unwrap();
    assert!(world.scoring());
    let mut tick = Tick::new(1);
    tick.changes.push(Change::Scoring(false));
    apply_tick(&mut world, &tick, &mut TickOutput::default()).unwrap();
    assert!(!world.scoring());
}

/// Each record's coding, debug-printed: records hold seat inputs, which
/// have no equality of their own.
fn shown(record: &Record) -> String {
    format!("{record:?}")
}

#[test]
fn every_record_round_trips_in_stream_order() {
    let records = samples::standby_records();
    assert_eq!(
        records.iter().map(Record::code).collect::<Vec<_>>(),
        [0, 2, 5, 3, 4, 6, 7, 9, 8, 1]
    );
    let mut writer = StreamWriter::new();
    let mut reader = StreamReader::new();
    for record in &records {
        let bytes = writer.encode(record).unwrap();
        assert_eq!(bytes[0] & 0x0F, record.code());
        assert_eq!(shown(&reader.decode(&bytes).unwrap()), shown(record));
    }
}

#[test]
fn a_seats_unchanged_input_costs_a_few_bytes_against_its_last() {
    let records = samples::standby_records();
    let Record::Ticks(ticks) = &records[2] else {
        panic!("the third sample is the Ticks")
    };
    // The same tick sent once alone and once after the tick before it.
    let mut alone = StreamWriter::new();
    let second = Ticks {
        first: 1_201,
        ticks: vec![ticks.ticks[1].clone()],
    };
    let cold = alone.encode(&Record::Ticks(second.clone())).unwrap();
    let mut warm = StreamWriter::new();
    let first = Ticks {
        first: 1_200,
        ticks: vec![ticks.ticks[0].clone()],
    };
    warm.encode(&Record::Ticks(first)).unwrap();
    let after = warm.encode(&Record::Ticks(second)).unwrap();
    assert!(
        after.len() * 2 < cold.len(),
        "{} against {}",
        after.len(),
        cold.len()
    );
    // A seat that changed nothing but the tick costs under 2 bytes, 1 of
    // them its seat (protocol 14, slice K3; under 9 in protocol 13).
    let steady = |seats: u8| {
        let mut writer = StreamWriter::new();
        let tick_of = |number: u64| {
            let mut tick = Tick::new(number);
            for seat in 0..seats {
                let mut input = quantized(pilot(seat, 10));
                input.tick = number;
                input.commands.clear();
                input.view = Some(SeatView {
                    tick: number - 10,
                    interpolation_delay: 8,
                });
                tick.push_input(input, 3);
            }
            Record::Ticks(Ticks {
                first: number as u32,
                ticks: vec![tick],
            })
        };
        writer.encode(&tick_of(10)).unwrap();
        writer.encode(&tick_of(11)).unwrap().len()
    };
    let per_seat = (steady(11) - steady(1)) as f64 / 10.;
    assert!(per_seat < 2., "{per_seat} bytes a seat");
}

/// `input` as the host steps it: its controls on the wire's grid.
fn quantized(mut input: SeatInput) -> SeatInput {
    let commands = std::mem::take(&mut input.pilot.commands);
    input.pilot =
        crate::wire::inputs::InputFrame::of(&input.pilot, input.trigger, input.sensors).pilot();
    input.pilot.commands = commands;
    input
}

/// Codes `ticks` four ticks a record (a snapshot interval) and reads them
/// back; the bytes in all.
fn through(ticks: &[Tick]) -> (Vec<Tick>, usize) {
    let mut writer = StreamWriter::new();
    let mut reader = StreamReader::new();
    let mut bytes = 0;
    let mut out = Vec::new();
    for chunk in ticks.chunks(4) {
        let record = Record::Ticks(Ticks {
            first: chunk[0].tick as u32,
            ticks: chunk.to_vec(),
        });
        let coded = writer.encode(&record).unwrap();
        bytes += coded.len();
        let Record::Ticks(read) = reader.decode(&coded).unwrap() else {
            panic!("a Ticks record")
        };
        out.extend(read.ticks);
    }
    (out, bytes)
}

/// Every field of every input, to the bit.
fn same(a: &[Tick], b: &[Tick]) {
    assert_eq!(a.len(), b.len());
    for (a, b) in a.iter().zip(b) {
        assert_eq!(a.applied, b.applied);
        assert_eq!(a.inputs.len(), b.inputs.len());
        for (x, y) in a.inputs.iter().zip(&b.inputs) {
            assert_eq!(format!("{x:?}"), format!("{y:?}"));
            for (p, q) in [
                (x.pilot.pitch, y.pilot.pitch),
                (x.pilot.roll, y.pilot.roll),
                (x.pilot.yaw, y.pilot.yaw),
                (x.pilot.throttle_rate, y.pilot.throttle_rate),
            ] {
                assert_eq!(p.to_bits(), q.to_bits());
            }
            assert_eq!(
                x.pilot.throttle.map(f64::to_bits),
                y.pilot.throttle.map(f64::to_bits)
            );
        }
    }
}

#[test]
fn a_moving_stick_on_the_wires_grid_costs_a_few_bytes_a_seat_a_tick() {
    // Protocol 14 (slice K3): the controls as the Inputs section codes
    // them. Two seats weave for ten seconds, commands and all.
    let fight = |seats: u8| -> Vec<Tick> {
        (0..1_200)
            .map(|n| {
                let mut tick = Tick::new(n);
                for seat in 0..seats {
                    tick.push_input(quantized(pilot(seat, n)), (n / 240) as u16);
                }
                tick
            })
            .collect()
    };
    let ticks = fight(2);
    let (read, bytes) = through(&ticks);
    same(&ticks, &read);
    // Over the records' own bytes, which nobody's inputs leave.
    let (_, empty) = through(&fight(0));
    let per_seat = (bytes - empty) as f64 / 2_400.;
    assert!(per_seat < 5., "{per_seat:.2} bytes a seat a tick");
}

#[test]
fn every_way_a_seat_input_is_coded_reads_back_to_the_bit() {
    let base = quantized(pilot(0, 100));
    let mut inputs = Vec::new();
    // Off the wire's grid: a stick of -0.0 and one between two steps, a
    // throttle that is not finite (none on the grid).
    let mut off = base.clone();
    off.pilot.pitch = -0.0;
    off.pilot.roll = 0.123_456_789;
    inputs.push(off);
    let mut off = base.clone();
    off.pilot.throttle = Some(0.5 + 1e-9);
    inputs.push(off);
    // Views: none, as Inputs codes it, a view ahead of the tick, an offset
    // past 255, a delay past 63, and the baseline's moved on.
    for view in [
        None,
        Some(SeatView {
            tick: 90,
            interpolation_delay: 63,
        }),
        Some(SeatView {
            tick: 105,
            interpolation_delay: 8,
        }),
        Some(SeatView {
            tick: 0,
            interpolation_delay: 8,
        }),
        Some(SeatView {
            tick: 99,
            interpolation_delay: 200,
        }),
    ] {
        let mut input = base.clone();
        input.view = view;
        inputs.push(input);
    }
    // Commands of both lists, and a pilot's alone.
    let mut input = quantized(pilot(0, 5));
    input.pilot.commands = vec![tore_sim::flight::PilotCommand::Eject];
    inputs.push(input);
    let mut input = base.clone();
    input.pilot.commands = vec![tore_sim::flight::PilotCommand::Eject];
    inputs.push(input);
    let ticks: Vec<Tick> = inputs
        .into_iter()
        .enumerate()
        .flat_map(|(n, input)| {
            // Each after its baseline, then again unchanged.
            let number = 300 + 2 * n as u64;
            [number, number + 1].map(|number| {
                let mut tick = Tick::new(number);
                let mut input = input.clone();
                if let Some(view) = &mut input.view {
                    view.tick = view.tick + number - 100;
                }
                input.tick = number;
                tick.push_input(input, n as u16 * 7);
                tick
            })
        })
        .collect();
    let (read, _) = through(&ticks);
    same(&ticks, &read);
}

#[test]
fn an_appoint_or_a_flight_starts_the_baselines_afresh() {
    let records = samples::standby_records();
    let mut fresh_writer = StreamWriter::new();
    let ticks_alone = fresh_writer.encode(&records[2]).unwrap();
    let mut writer = StreamWriter::new();
    writer.encode(&records[2]).unwrap();
    let again = writer.encode(&records[2]).unwrap();
    assert_ne!(again, ticks_alone, "the second codes against the first");
    for reset in [&records[0], &records[1]] {
        let mut writer = StreamWriter::new();
        writer.encode(&records[2]).unwrap();
        writer.encode(reset).unwrap();
        assert_eq!(writer.encode(&records[2]).unwrap(), ticks_alone);
    }
    // A reader that missed the first Ticks cannot read the second.
    let mut reader = StreamReader::new();
    assert!(
        reader.decode(&again).is_err()
            || shown(&reader.decode(&again).unwrap()) != shown(&records[2])
    );
}

#[test]
fn records_that_break_a_bound_are_refused_by_the_writer_and_change_nothing() {
    let records = samples::standby_records();
    let Record::Ticks(ticks) = &records[2] else {
        panic!("the third sample is the Ticks")
    };
    let mut writer = StreamWriter::new();
    let refused = |writer: &mut StreamWriter, record: Record| {
        assert!(writer.encode(&record).is_err(), "{record:?}");
    };
    refused(&mut writer, Record::Ticks(Ticks::default()));
    refused(
        &mut writer,
        Record::Ticks(Ticks {
            first: 0,
            ticks: (0..61).map(Tick::new).collect(),
        }),
    );
    // Not consecutive from `first`.
    refused(
        &mut writer,
        Record::Ticks(Ticks {
            first: 1_199,
            ticks: ticks.ticks.clone(),
        }),
    );
    // A seat's input for another tick, and inputs without their numbers.
    let mut wrong = ticks.ticks[0].clone();
    wrong.inputs[0].tick += 1;
    refused(
        &mut writer,
        Record::Ticks(Ticks {
            first: 1_200,
            ticks: vec![wrong],
        }),
    );
    let mut unmatched = ticks.ticks[0].clone();
    unmatched.applied.pop();
    refused(
        &mut writer,
        Record::Ticks(Ticks {
            first: 1_200,
            ticks: vec![unmatched],
        }),
    );
    refused(
        &mut writer,
        Record::CheckpointChunk(CheckpointChunk {
            index: 0,
            bytes: vec![0; 4_097],
        }),
    );
    refused(
        &mut writer,
        Record::Appoint(Appoint {
            role: StandbyMark::None,
            warm: false,
            check_every: 600,
            checkpoint_every: 1_200,
            mission: 1,
        }),
    );
    // Nothing refused moved a baseline: the stream reads on from the start.
    let bytes = writer.encode(&records[2]).unwrap();
    assert_eq!(
        shown(&StreamReader::new().decode(&bytes).unwrap()),
        shown(&records[2])
    );
}

#[test]
fn damaged_records_are_refused_without_a_panic_and_leave_the_reader_as_it_was() {
    let records = samples::standby_records();
    let mut writer = StreamWriter::new();
    let coded: Vec<Vec<u8>> = records.iter().map(|r| writer.encode(r).unwrap()).collect();
    for (index, bytes) in coded.iter().enumerate() {
        for cut in 0..bytes.len() {
            let mut reader = StreamReader::new();
            let _ = reader.decode(&bytes[..cut]);
        }
        for bit in 0..bytes.len().min(400) * 8 {
            let mut flipped = bytes.clone();
            flipped[bit / 8] ^= 1 << (bit % 8);
            let mut reader = StreamReader::new();
            let _ = reader.decode(&flipped);
        }
        // Types 10 to 15 name nothing; neither does a part 0 or 9.
        let mut unknown = bytes.clone();
        unknown[0] = (unknown[0] & 0xF0) | 0x0A;
        assert!(StreamReader::new().decode(&unknown).is_err(), "{index}");
    }
    let mut part = coded[5].clone();
    part[0] = (part[0] & 0x0F) | (9 << 4);
    assert!(StreamReader::new().decode(&part).is_err());
    // A refused record changes nothing: the next good one still reads.
    let mut reader = StreamReader::new();
    reader.decode(&coded[0]).unwrap();
    assert!(reader.decode(&coded[2][..coded[2].len() / 2]).is_err());
    assert_eq!(
        shown(&reader.decode(&coded[2]).unwrap()),
        shown(&records[2])
    );
}

#[test]
fn the_check_hash_is_the_checkpoints_fnv() {
    let import = resources();
    let world = fresh(&crowd_spec(), &import);
    assert_eq!(
        check_hash(&world).unwrap(),
        tore_codec::fnv1a64(&world.checkpoint().unwrap())
    );
}
