//! The client's capture: everything the client session was given, so it can
//! be run again offline into the same frames (docs/formats/net-protocol.md,
//! "Captures"). It records the join (the build, the callsign and the seed of
//! its randomness, never the password), every datagram received with its
//! arrival time, every update with the pilot's controls as the client rounded
//! them, every frame asked for, the leave and the disconnect, and every
//! Inputs section sent.
//!
//! [`replay`] runs a client from a capture with no network and calls back
//! with each frame. The replayed client writes its own capture, which equals
//! the original byte for byte when it behaved the same.

use super::{Client, ClientConfig, ClientFrame, Race, Sampled};
use crate::host::BuildId;
use crate::wire::entity::{EntityKey, EntityKind};
use crate::wire::inputs::{InputFrame, read_command, write_command};
use std::collections::BTreeMap;
use std::io::Write;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tore_codec::{BitReader, BitWriter};
use tore_net::master::Path;
use tore_net::{Entropy, Target};
use tore_sim::sensors::{Channel, Controls as Scope};

/// The file's first bytes.
pub const MAGIC: &[u8; 8] = b"TORE-CAP";
/// The capture format's version.
pub const FORMAT_VERSION: u16 = 2;

/// Record kinds.
pub mod kind {
    pub const START: u8 = 1;
    pub const RECEIVE: u8 = 2;
    pub const UPDATE: u8 = 3;
    pub const FRAME: u8 = 4;
    pub const LEAVE: u8 = 5;
    pub const DISCONNECT: u8 = 6;
    pub const SENT: u8 = 7;
    /// A lobby request the player made (format 2, slice EF4).
    pub const REQUEST: u8 = 8;
    /// The player left the game: Leave, then quit once the debrief is in.
    pub const LEAVE_GAME: u8 = 9;
    /// A join through the master (protocol 9, slice J2): right after the
    /// start, the host's addresses raced and the introduction.
    pub const RACE: u8 = 10;
}

/// A record body's largest size: a datagram and its header with room.
const MAX_RECORD: usize = 64 * 1024;

/// Why a capture could not be replayed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CaptureError {
    /// Not a capture, or one of another format or protocol version.
    NotACapture,
    /// A record that does not read.
    Damaged(&'static str),
    /// The client would not start from the capture's join.
    Start(String),
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotACapture => f.write_str("not a capture of this protocol version"),
            Self::Damaged(what) => write!(f, "damaged capture: {what}"),
            Self::Start(text) => write!(f, "the capture's join does not start: {text}"),
        }
    }
}

impl std::error::Error for CaptureError {}

fn put_str(out: &mut Vec<u8>, text: &str) {
    let bytes = text.as_bytes();
    let len = bytes.len().min(usize::from(u16::MAX));
    out.extend_from_slice(&(len as u16).to_le_bytes());
    out.extend_from_slice(&bytes[..len]);
}

fn put_time(out: &mut Vec<u8>, time: Duration) {
    out.extend_from_slice(&(time.as_nanos().min(u128::from(u64::MAX)) as u64).to_le_bytes());
}

/// The capture writer the client keeps.
pub struct CaptureWriter {
    out: Box<dyn Write>,
    failed: bool,
}

impl CaptureWriter {
    /// A capture on `out`; its magic and versions go first.
    pub fn new(out: Box<dyn Write>) -> Self {
        let mut writer = Self { out, failed: false };
        let mut head = MAGIC.to_vec();
        head.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
        head.extend_from_slice(&crate::wire::PROTOCOL_VERSION.to_le_bytes());
        writer.bytes(&head);
        writer
    }

    fn bytes(&mut self, bytes: &[u8]) {
        if !self.failed && self.out.write_all(bytes).is_err() {
            // A capture that cannot be written stops; the session goes on.
            self.failed = true;
        }
    }

    fn record(&mut self, kind: u8, body: &[u8]) {
        let mut out = Vec::with_capacity(body.len() + 5);
        out.push(kind);
        out.extend_from_slice(&(body.len() as u32).to_le_bytes());
        out.extend_from_slice(body);
        self.bytes(&out);
    }

    /// The join: the time it started, the seed and the settings but the
    /// password.
    pub fn header(&mut self, config: &ClientConfig, seed: u64, started: Duration) {
        let mut body = Vec::new();
        put_time(&mut body, started);
        body.extend_from_slice(&seed.to_le_bytes());
        put_str(&mut body, &config.server.to_string());
        put_str(&mut body, &config.callsign);
        put_str(&mut body, &config.build.version);
        put_str(&mut body, &config.build.commit);
        body.push(u8::from(config.build.release));
        match config.plane {
            Some(plane) => {
                body.push(1);
                body.extend_from_slice(&plane.to_le_bytes());
            }
            None => body.push(0),
        }
        body.push(u8::from(config.auto_ready));
        self.record(kind::START, &body);
        if let Some(race) = &config.race {
            let mut body = Vec::new();
            body.extend_from_slice(&race.introduction.to_le_bytes());
            body.push(race.targets.len().min(usize::from(u8::MAX)) as u8);
            for target in race.targets.iter().take(usize::from(u8::MAX)) {
                put_str(&mut body, &target.address.to_string());
                body.push(target.path.code());
            }
            self.record(kind::RACE, &body);
        }
    }

    /// A lobby request the player made at `now`: the message's kind and
    /// body.
    pub fn request(&mut self, now: Duration, kind: u8, message: &[u8]) {
        let mut body = Vec::with_capacity(message.len() + 9);
        put_time(&mut body, now);
        body.push(kind);
        body.extend_from_slice(message);
        self.record(kind::REQUEST, &body);
    }

    /// The player left the game at `now`.
    pub fn leave_game(&mut self, now: Duration) {
        let mut body = Vec::with_capacity(8);
        put_time(&mut body, now);
        self.record(kind::LEAVE_GAME, &body);
    }

    /// A datagram that arrived at `now`.
    pub fn receive(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) {
        let mut body = Vec::with_capacity(datagram.len() + 32);
        put_time(&mut body, now);
        put_str(&mut body, &from.to_string());
        body.extend_from_slice(datagram);
        self.record(kind::RECEIVE, &body);
    }

    /// An update at `now` with these controls.
    pub fn update(&mut self, now: Duration, sampled: &Sampled) {
        let mut body = Vec::with_capacity(32);
        put_time(&mut body, now);
        body.extend_from_slice(&encode_sampled(sampled));
        self.record(kind::UPDATE, &body);
    }

    /// A frame asked for at `now`.
    pub fn frame(&mut self, now: Duration) {
        let mut body = Vec::with_capacity(8);
        put_time(&mut body, now);
        self.record(kind::FRAME, &body);
        let _ = self.out.flush();
    }

    /// The player left at `now`.
    pub fn leave(&mut self, now: Duration) {
        let mut body = Vec::with_capacity(8);
        put_time(&mut body, now);
        self.record(kind::LEAVE, &body);
    }

    /// The player quit at `now`.
    pub fn disconnect(&mut self, now: Duration) {
        let mut body = Vec::with_capacity(8);
        put_time(&mut body, now);
        self.record(kind::DISCONNECT, &body);
        let _ = self.out.flush();
    }

    /// An Inputs section sent at `now`.
    pub fn sent(&mut self, now: Duration, section: &[u8]) {
        let mut body = Vec::with_capacity(section.len() + 8);
        put_time(&mut body, now);
        body.extend_from_slice(section);
        self.record(kind::SENT, &body);
    }
}

fn channel_code(channel: Channel) -> u64 {
    match channel {
        Channel::Radar => 0,
        Channel::Infrared => 1,
        Channel::Visual => 2,
    }
}

/// The controls of an update, bit packed: the frame's fields at their
/// widths, the view subject, then the commands as the Inputs section codes
/// them.
pub fn encode_sampled(sampled: &Sampled) -> Vec<u8> {
    let f = &sampled.frame;
    let mut w = BitWriter::with_capacity(16);
    let _ = w.write_bits(u64::from(f.pitch as u16), 16);
    let _ = w.write_bits(u64::from(f.roll as u16), 16);
    let _ = w.write_bits(u64::from(f.yaw as u16), 16);
    let _ = w.write_bits(u64::from(f.throttle_rate as u8), 8);
    w.write_bool(f.throttle.is_some());
    if let Some(throttle) = f.throttle {
        let _ = w.write_bits(u64::from(throttle), 16);
    }
    w.write_bool(f.trigger);
    let _ = w.write_bits(channel_code(f.sensors.channel), 2);
    let _ = w.write_bits(f.sensors.range_index.min(15) as u64, 4);
    w.write_bool(f.sensors.history);
    w.write_bool(sampled.view_subject.is_some());
    if let Some(key) = sampled.view_subject {
        let _ = w.write_bits(u64::from(key.kind.code()), 2);
        w.write_varint(u64::from(key.id));
    }
    w.write_varint(sampled.commands.len() as u64);
    for command in &sampled.commands {
        write_command(&mut w, command);
    }
    w.finish()
}

/// Reads what [`encode_sampled`] wrote.
pub fn decode_sampled(bytes: &[u8]) -> Result<Sampled, CaptureError> {
    let bad = |_| CaptureError::Damaged("controls");
    let mut r = BitReader::new(bytes);
    let pitch = r.read_bits(16).map_err(bad)? as u16 as i16;
    let roll = r.read_bits(16).map_err(bad)? as u16 as i16;
    let yaw = r.read_bits(16).map_err(bad)? as u16 as i16;
    let throttle_rate = r.read_bits(8).map_err(bad)? as u8 as i8;
    let throttle = if r.read_bool().map_err(bad)? {
        Some(r.read_bits(16).map_err(bad)? as u16)
    } else {
        None
    };
    let trigger = r.read_bool().map_err(bad)?;
    let channel = match r.read_bits(2).map_err(bad)? {
        0 => Channel::Radar,
        1 => Channel::Infrared,
        2 => Channel::Visual,
        _ => return Err(CaptureError::Damaged("scope channel")),
    };
    let range_index = r.read_bits(4).map_err(bad)? as usize;
    let history = r.read_bool().map_err(bad)?;
    let view_subject = if r.read_bool().map_err(bad)? {
        let kind = EntityKind::from_code(r.read_bits(2).map_err(bad)? as u8);
        let id = u32::try_from(r.read_varint().map_err(bad)?)
            .map_err(|_| CaptureError::Damaged("view subject"))?;
        Some(EntityKey { kind, id })
    } else {
        None
    };
    let count = r.read_varint().map_err(bad)?;
    if count > 4096 {
        return Err(CaptureError::Damaged("command count"));
    }
    let mut commands = Vec::with_capacity(count as usize);
    for _ in 0..count {
        commands.push(read_command(&mut r).map_err(|_| CaptureError::Damaged("command"))?);
    }
    Ok(Sampled {
        frame: InputFrame {
            pitch,
            roll,
            yaw,
            throttle_rate,
            throttle,
            trigger,
            sensors: Scope {
                channel,
                range_index,
                history,
            },
        },
        commands,
        view_subject,
    })
}

/// One record of a capture.
#[derive(Clone, Debug, PartialEq)]
pub enum Record {
    Start {
        started: Duration,
        seed: u64,
        server: SocketAddr,
        callsign: String,
        build: BuildId,
        plane: Option<u32>,
        auto_ready: bool,
    },
    Receive {
        now: Duration,
        from: SocketAddr,
        datagram: Vec<u8>,
    },
    Update {
        now: Duration,
        sampled: Sampled,
    },
    Frame {
        now: Duration,
    },
    Leave {
        now: Duration,
    },
    Disconnect {
        now: Duration,
    },
    Sent {
        now: Duration,
        section: Vec<u8>,
    },
    Request {
        now: Duration,
        kind: u8,
        body: Vec<u8>,
    },
    LeaveGame {
        now: Duration,
    },
    Race(Race),
}

/// A capture's records, read one by one. A capture cut short (the game
/// stopped while writing) ends at its last whole record.
pub struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], CaptureError> {
        let end = self
            .at
            .checked_add(n)
            .filter(|end| *end <= self.bytes.len())
            .ok_or(CaptureError::Damaged("record too short"))?;
        let out = &self.bytes[self.at..end];
        self.at = end;
        Ok(out)
    }
    fn u8(&mut self) -> Result<u8, CaptureError> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, CaptureError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("4 bytes"),
        ))
    }
    fn u64(&mut self) -> Result<u64, CaptureError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("8 bytes"),
        ))
    }
    fn time(&mut self) -> Result<Duration, CaptureError> {
        Ok(Duration::from_nanos(self.u64()?))
    }
    fn str(&mut self) -> Result<String, CaptureError> {
        let len = u16::from_le_bytes(self.take(2)?.try_into().expect("2 bytes"));
        String::from_utf8(self.take(usize::from(len))?.to_vec())
            .map_err(|_| CaptureError::Damaged("text"))
    }
    fn address(&mut self) -> Result<SocketAddr, CaptureError> {
        self.str()?
            .parse()
            .map_err(|_| CaptureError::Damaged("address"))
    }
    fn rest(&mut self) -> &'a [u8] {
        let out = &self.bytes[self.at..];
        self.at = self.bytes.len();
        out
    }
}

impl<'a> Reader<'a> {
    /// The records of `bytes`, after checking its magic and versions.
    pub fn new(bytes: &'a [u8]) -> Result<Self, CaptureError> {
        if bytes.len() < 12 || &bytes[..8] != MAGIC {
            return Err(CaptureError::NotACapture);
        }
        let format = u16::from_le_bytes([bytes[8], bytes[9]]);
        let protocol = u16::from_le_bytes([bytes[10], bytes[11]]);
        if format != FORMAT_VERSION || protocol != crate::wire::PROTOCOL_VERSION {
            return Err(CaptureError::NotACapture);
        }
        Ok(Self { bytes, at: 12 })
    }

    /// Bytes read so far: the whole records.
    pub fn position(&self) -> usize {
        self.at
    }

    /// The next record, `None` at the end (or at a cut-off last record).
    pub fn next_record(&mut self) -> Result<Option<Record>, CaptureError> {
        let rest = &self.bytes[self.at..];
        if rest.len() < 5 {
            return Ok(None);
        }
        let kind = rest[0];
        let len = u32::from_le_bytes(rest[1..5].try_into().expect("4 bytes")) as usize;
        if len > MAX_RECORD {
            return Err(CaptureError::Damaged("record length"));
        }
        if rest.len() < 5 + len {
            return Ok(None);
        }
        let mut c = Cursor {
            bytes: &rest[5..5 + len],
            at: 0,
        };
        let record = match kind {
            kind::START => {
                let started = c.time()?;
                let seed = c.u64()?;
                let server = c.address()?;
                let callsign = c.str()?;
                let version = c.str()?;
                let commit = c.str()?;
                let release = c.u8()? != 0;
                let plane = match c.u8()? {
                    0 => None,
                    _ => Some(c.u32()?),
                };
                let auto_ready = c.u8()? != 0;
                Record::Start {
                    started,
                    seed,
                    server,
                    callsign,
                    build: BuildId {
                        version,
                        commit,
                        release,
                    },
                    plane,
                    auto_ready,
                }
            }
            kind::RECEIVE => Record::Receive {
                now: c.time()?,
                from: c.address()?,
                datagram: c.rest().to_vec(),
            },
            kind::UPDATE => Record::Update {
                now: c.time()?,
                sampled: decode_sampled(c.rest())?,
            },
            kind::FRAME => Record::Frame { now: c.time()? },
            kind::LEAVE => Record::Leave { now: c.time()? },
            kind::DISCONNECT => Record::Disconnect { now: c.time()? },
            kind::SENT => Record::Sent {
                now: c.time()?,
                section: c.rest().to_vec(),
            },
            kind::REQUEST => Record::Request {
                now: c.time()?,
                kind: c.u8()?,
                body: c.rest().to_vec(),
            },
            kind::LEAVE_GAME => Record::LeaveGame { now: c.time()? },
            kind::RACE => {
                let introduction = c.u64()?;
                let count = c.u8()?;
                let mut targets = Vec::with_capacity(usize::from(count));
                for _ in 0..count {
                    let address = c.address()?;
                    let path =
                        Path::from_code(u64::from(c.u8()?)).ok_or(CaptureError::Damaged("path"))?;
                    targets.push(Target::new(address, path));
                }
                Record::Race(Race {
                    targets,
                    introduction,
                })
            }
            _ => return Err(CaptureError::Damaged("record kind")),
        };
        self.at += 5 + len;
        Ok(Some(record))
    }
}

/// What a replay did.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Replayed {
    pub records: u64,
    pub frames: u64,
    /// The replayed client's own capture equals the original: it received,
    /// stepped and sent exactly the same.
    pub identical: bool,
}

#[derive(Clone, Default)]
struct Shared(Arc<Mutex<Vec<u8>>>);

impl Write for Shared {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("poisoned"))?
            .extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// A capture run again, with the client it ran kept.
pub(crate) struct Run {
    pub client: Client,
    copy: Shared,
    pub records: u64,
    pub frames: u64,
    /// Bytes of the capture that held whole records.
    pub consumed: usize,
    /// The kind of the last record read.
    pub last: Option<u8>,
}

/// The kind byte of a record, for [`Run::last`].
fn kind_of(record: &Record) -> u8 {
    match record {
        Record::Start { .. } => kind::START,
        Record::Receive { .. } => kind::RECEIVE,
        Record::Update { .. } => kind::UPDATE,
        Record::Frame { .. } => kind::FRAME,
        Record::Leave { .. } => kind::LEAVE,
        Record::Disconnect { .. } => kind::DISCONNECT,
        Record::Sent { .. } => kind::SENT,
        Record::Request { .. } => kind::REQUEST,
        Record::Race(_) => kind::RACE,
        Record::LeaveGame { .. } => kind::LEAVE_GAME,
    }
}

/// Runs the client session of `capture` again, offline, with the game data
/// `resources`. `setup` sees the client before the first record; `on_frame`
/// every frame the player's game drew; `after` the client and the record's
/// time after each record has been given to it.
pub(crate) fn run(
    capture: &[u8],
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    setup: &mut dyn FnMut(&mut Client),
    on_frame: &mut dyn FnMut(&ClientFrame),
    after: &mut dyn FnMut(&mut Client, Duration, u8),
) -> Result<Run, CaptureError> {
    let mut reader = Reader::new(capture)?;
    let Some(Record::Start {
        started,
        seed,
        server,
        callsign,
        build,
        plane,
        auto_ready,
    }) = reader.next_record()?
    else {
        return Err(CaptureError::Damaged("no start record"));
    };
    // A join through the master names its race right after the start.
    let mut pending = reader.next_record()?;
    let race = match pending.take() {
        Some(Record::Race(race)) => Some(race),
        other => {
            pending = other;
            None
        }
    };
    let config = ClientConfig {
        plane,
        auto_ready,
        entropy: Entropy::Seeded(seed),
        race,
        ..ClientConfig::new(server, &callsign, build)
    };
    let mut client = Client::start(config, resources, started, seed)
        .map_err(|error| CaptureError::Start(error.to_string()))?;
    let copy = Shared::default();
    client.set_capture(Box::new(copy.clone()));
    setup(&mut client);
    let records = 1 + u64::from(client.config.race.is_some());
    let mut out = Run {
        client,
        copy,
        records,
        frames: 0,
        consumed: 0,
        last: Some(kind::START),
    };
    while let Some(record) = match pending.take() {
        Some(record) => Some(record),
        None => reader.next_record()?,
    } {
        out.records += 1;
        let record_kind = kind_of(&record);
        out.last = Some(record_kind);
        let client = &mut out.client;
        let at = match record {
            Record::Start { .. } => return Err(CaptureError::Damaged("a second start")),
            Record::Race(_) => return Err(CaptureError::Damaged("a race after the start")),
            Record::Receive {
                now,
                from,
                datagram,
            } => {
                client.receive(now, from, &datagram);
                now
            }
            Record::Update { now, sampled } => {
                client.update_sampled(now, sampled);
                now
            }
            Record::Frame { now } => {
                if let Some(frame) = client.frame(now) {
                    out.frames += 1;
                    on_frame(&frame);
                }
                now
            }
            Record::Leave { now } => {
                client.leave(now);
                now
            }
            Record::Disconnect { now } => {
                client.disconnect(now);
                now
            }
            Record::Request { now, kind, body } => {
                let message = crate::wire::messages::Message::decode(kind, &body)
                    .map_err(|_| CaptureError::Damaged("lobby request"))?;
                client.request(now, message);
                now
            }
            Record::LeaveGame { now } => {
                client.leave_game(now);
                now
            }
            // The replayed client writes its own; the comparison checks them.
            Record::Sent { now, .. } => now,
        };
        after(&mut out.client, at, record_kind);
    }
    out.consumed = reader.position();
    Ok(out)
}

/// Runs the client session of `capture` again, offline, with the game data
/// `resources` (the same import), calling `on_frame` with every frame the
/// player's game drew.
pub fn replay(
    capture: &[u8],
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    on_frame: &mut dyn FnMut(&ClientFrame),
) -> Result<Replayed, CaptureError> {
    let Run {
        client,
        copy,
        records,
        frames,
        consumed,
        ..
    } = run(capture, resources, &mut |_| {}, on_frame, &mut |_, _, _| {})?;
    // The replayed client's capture is complete once it is dropped.
    drop(client);
    let copy = copy.0.lock().map(|b| b.clone()).unwrap_or_default();
    Ok(Replayed {
        records,
        frames,
        identical: copy == capture[..consumed],
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wire::inputs::Command;
    use tore_sim::flight::{PilotCommand, Switch};
    use tore_world::seats::SeatCommand;

    #[test]
    fn controls_round_trip_through_the_capture() {
        let sampled = Sampled {
            frame: InputFrame {
                pitch: -32_767,
                roll: 12,
                yaw: 0,
                throttle_rate: -5,
                throttle: Some(65_535),
                trigger: true,
                sensors: Scope {
                    channel: Channel::Infrared,
                    range_index: 3,
                    history: true,
                },
            },
            commands: vec![
                Command::Pilot(PilotCommand::Toggle(Switch::Gear)),
                Command::Seat(SeatCommand::ReleaseFlare),
            ],
            view_subject: Some(EntityKey {
                kind: EntityKind::Projectile,
                id: 70_000,
            }),
        };
        assert_eq!(decode_sampled(&encode_sampled(&sampled)).unwrap(), sampled);
        let neutral = Sampled::default();
        assert_eq!(decode_sampled(&encode_sampled(&neutral)).unwrap(), neutral);
    }

    #[test]
    fn records_read_back_and_a_cut_capture_ends_at_its_last_whole_record() {
        let shared = Shared::default();
        let mut writer = CaptureWriter::new(Box::new(shared.clone()));
        let config = ClientConfig::new(
            "127.0.0.1:26900".parse().unwrap(),
            "Viper",
            BuildId {
                version: "0.1.3".into(),
                commit: "abc".into(),
                release: false,
            },
        );
        writer.header(&config, 42, Duration::from_millis(5));
        writer.receive(Duration::from_millis(6), config.server, &[1, 2, 3]);
        writer.frame(Duration::from_millis(7));
        let bytes = shared.0.lock().unwrap().clone();
        let mut reader = Reader::new(&bytes).unwrap();
        assert!(matches!(
            reader.next_record().unwrap(),
            Some(Record::Start {
                seed: 42,
                plane: None,
                ..
            })
        ));
        assert_eq!(
            reader.next_record().unwrap(),
            Some(Record::Receive {
                now: Duration::from_millis(6),
                from: config.server,
                datagram: vec![1, 2, 3]
            })
        );
        assert_eq!(
            reader.next_record().unwrap(),
            Some(Record::Frame {
                now: Duration::from_millis(7)
            })
        );
        assert_eq!(reader.next_record().unwrap(), None);
        let cut = &bytes[..bytes.len() - 3];
        let mut reader = Reader::new(cut).unwrap();
        let mut count = 0;
        while reader.next_record().unwrap().is_some() {
            count += 1;
        }
        assert_eq!(count, 2);
        assert_eq!(
            Reader::new(b"TORE-CAX\x01\0\x01\0").err(),
            Some(CaptureError::NotACapture)
        );
    }

    #[test]
    fn a_race_is_recorded_right_after_the_start() {
        let shared = Shared::default();
        let mut writer = CaptureWriter::new(Box::new(shared.clone()));
        let race = Race {
            targets: vec![
                Target::new("203.0.113.5:26900".parse().unwrap(), Path::Punched),
                Target::new("[2001:db8::5]:26900".parse().unwrap(), Path::Ipv6),
            ],
            introduction: 0xABCD,
        };
        let config = ClientConfig {
            race: Some(race.clone()),
            ..ClientConfig::new(
                "203.0.113.5:26900".parse().unwrap(),
                "Viper",
                BuildId {
                    version: "0.1.3".into(),
                    commit: "abc".into(),
                    release: false,
                },
            )
        };
        writer.header(&config, 42, Duration::ZERO);
        let bytes = shared.0.lock().unwrap().clone();
        let mut reader = Reader::new(&bytes).unwrap();
        assert!(matches!(
            reader.next_record().unwrap(),
            Some(Record::Start { .. })
        ));
        assert_eq!(reader.next_record().unwrap(), Some(Record::Race(race)));
        assert_eq!(reader.next_record().unwrap(), None);
    }
}
