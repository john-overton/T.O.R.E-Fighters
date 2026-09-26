//! The bounded reader.
//!
//! `open` scans the file once: it checks every chunk's checksum, builds the
//! tick index, and collects the strings, entities, events, checksums and the
//! display tree directory. A truncated or `.partial` file opens with
//! `complete() == false` and keeps every whole chunk. A damaged chunk is
//! skipped and reported in `problems()`. Frames are decoded on demand, a
//! chunk at a time.

use crate::chunk::{decode_frames, get_entities};
use crate::error::{Error, Result, corrupt};
use crate::events::{get_checksums, get_events};
use crate::format::{
    CHUNK_HEADER_BYTES, ChunkHeader, IndexEntry, KIND_DATA, KIND_FOOTER, KIND_HEADER, KIND_INDEX,
    PRELUDE_BYTES, SECTION_CHECKSUMS, SECTION_ENTITIES, SECTION_EVENTS, SECTION_SPAWNS,
    SECTION_STRINGS, SECTION_TREES, TRAILER_BYTES, chunk_checksum, decode_footer, decode_header,
    decode_index, read_chunk_header, read_prelude, read_sections, read_trailer, section,
};
use crate::limits::{
    MAX_CHUNK_BYTES, MAX_CHUNK_FRAMES, MAX_CHUNKS, MAX_EVENTS_TOTAL, MAX_FILE_BYTES,
    MAX_HEADER_BYTES,
};
use crate::model::{
    AircraftInfo, EffectKind, EffectSpawn, Event, Footer, Frame, Header, LAYER_CONTRAILS, PuffKind,
    PuffSpawn, TreeSample, WeaponInfo,
};
use crate::spawns::{FrameSpawns, get_spawns};
use crate::strings::StringTable;
use crate::trees::{get_tree_directory, get_trees};
use std::collections::{BTreeMap, HashMap};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};

/// Smoke puffs the simulation keeps alive at once, matching its caps.
pub const SMOKE_PUFF_CAP: usize = 8_192;
/// Contrail puffs the simulation keeps alive at once.
pub const CONTRAIL_PUFF_CAP: usize = 72_000;
/// The longest a puff lives: a contrail's two minutes.
pub const LONGEST_PUFF_TICKS: u64 = 14_400;
/// Chunks of spawns the reader keeps decoded for smoke rebuilding.
const SPAWN_CACHE_CHUNKS: usize = 256;

/// Where one data chunk sits and what it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ChunkInfo {
    /// Byte offset of the chunk header.
    pub offset: u64,
    /// Body length in bytes.
    pub bytes: u32,
    pub first_tick: u64,
    /// Frames, one per consecutive tick.
    pub frames: u32,
}

impl ChunkInfo {
    pub fn last_tick(&self) -> u64 {
        self.first_tick
            .saturating_add(u64::from(self.frames))
            .saturating_sub(1)
    }

    pub fn contains(&self, tick: u64) -> bool {
        tick >= self.first_tick && tick <= self.last_tick()
    }
}

/// An event and the tick it happened.
#[derive(Clone, Debug, PartialEq)]
pub struct TimedEvent {
    pub tick: u64,
    pub event: Event,
}

/// Effects and puffs released in a tick range, with their ticks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Spawns {
    pub effects: Vec<(u64, EffectSpawn)>,
    pub puffs: Vec<(u64, PuffSpawn)>,
}

/// A puff as it looks at a given tick.
#[derive(Clone, Debug, PartialEq)]
pub struct LivePuff {
    pub spawn_tick: u64,
    pub age_ticks: u64,
    pub layer: u8,
    pub kind: PuffKind,
    /// Where it is now: smoke has risen 2 feet per second since release.
    pub position: [f64; 3],
}

/// An effect still playing at a given tick.
#[derive(Clone, Debug, PartialEq)]
pub struct LiveEffect {
    pub spawn_tick: u64,
    pub age_ticks: u64,
    pub kind: EffectKind,
    pub position: [f64; 3],
    pub duration_ticks: u32,
}

enum Source {
    File(Mutex<File>),
    Memory(Vec<u8>),
}

impl Source {
    fn read_at(&self, offset: u64, len: usize) -> Result<Vec<u8>> {
        match self {
            Self::File(file) => {
                let mut file = file.lock().unwrap_or_else(|e| e.into_inner());
                file.seek(SeekFrom::Start(offset))?;
                let mut buf = vec![0; len];
                file.read_exact(&mut buf)?;
                Ok(buf)
            }
            Self::Memory(bytes) => usize::try_from(offset)
                .ok()
                .and_then(|start| bytes.get(start..start.checked_add(len)?))
                .map(<[u8]>::to_vec)
                .ok_or_else(|| {
                    Error::Io(std::io::Error::new(
                        std::io::ErrorKind::UnexpectedEof,
                        "read past the end of the recording",
                    ))
                }),
        }
    }
}

/// An open recording. Safe to share between threads.
pub struct Recording {
    source: Source,
    path: Option<PathBuf>,
    file_bytes: u64,
    header: Header,
    footer: Option<Footer>,
    complete: bool,
    problems: Vec<String>,
    chunks: Vec<ChunkInfo>,
    strings: StringTable,
    aircraft: BTreeMap<u32, AircraftInfo>,
    weapons: BTreeMap<u32, WeaponInfo>,
    events: Vec<TimedEvent>,
    checksums: Vec<(u64, u64)>,
    trees: HashMap<(u32, String), Vec<(usize, u64)>>,
    frame_cache: Mutex<Option<(usize, Arc<Vec<Frame>>)>>,
    spawn_cache: Mutex<HashMap<usize, Arc<Vec<FrameSpawns>>>>,
}

impl std::fmt::Debug for Recording {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Recording")
            .field("path", &self.path)
            .field("complete", &self.complete)
            .field("chunks", &self.chunks.len())
            .field("problems", &self.problems)
            .finish()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// A chunk read from the source: header, body, and whether it was whole.
enum ChunkRead {
    Chunk(ChunkHeader, Vec<u8>),
    /// The file ends inside this chunk.
    Truncated,
    /// The bytes here are not a valid chunk.
    Damaged(String, Option<u64>),
}

impl Recording {
    /// Opens and indexes a recording file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Self::load(
            Source::File(Mutex::new(file)),
            Some(path.to_path_buf()),
            len,
        )
    }

    /// Indexes a recording held in memory.
    pub fn from_bytes(bytes: Vec<u8>) -> Result<Self> {
        let len = bytes.len() as u64;
        Self::load(Source::Memory(bytes), None, len)
    }

    fn load(source: Source, path: Option<PathBuf>, len: u64) -> Result<Self> {
        if len > MAX_FILE_BYTES {
            return Err(Error::Unsupported(format!(
                "the file is {} MiB; recordings are limited to {} MiB",
                len >> 20,
                MAX_FILE_BYTES >> 20
            )));
        }
        if len < PRELUDE_BYTES as u64 {
            return Err(corrupt("the file is too short to be a recording"));
        }
        let version = read_prelude(&source.read_at(0, PRELUDE_BYTES)?)?;
        let mut recording = Self {
            source,
            path,
            file_bytes: len,
            header: Header::default(),
            footer: None,
            complete: false,
            problems: Vec::new(),
            chunks: Vec::new(),
            strings: StringTable::default(),
            aircraft: BTreeMap::new(),
            weapons: BTreeMap::new(),
            events: Vec::new(),
            checksums: Vec::new(),
            trees: HashMap::new(),
            frame_cache: Mutex::new(None),
            spawn_cache: Mutex::new(HashMap::new()),
        };
        let mut pos = PRELUDE_BYTES as u64;
        match recording.read_chunk(pos, MAX_HEADER_BYTES)? {
            ChunkRead::Chunk(header, body) if header.kind == KIND_HEADER => {
                recording.header = decode_header(&body, version)?;
                pos += (CHUNK_HEADER_BYTES + body.len()) as u64;
            }
            ChunkRead::Chunk(..) => return Err(corrupt("the file does not start with a header")),
            ChunkRead::Truncated => return Err(corrupt("the file ends inside its header")),
            ChunkRead::Damaged(reason, _) => {
                return Err(corrupt(format!("the header is damaged: {reason}")));
            }
        }
        let index = recording.read_index();
        recording.scan(pos, index.as_ref())?;
        Ok(recording)
    }

    /// Reads the chunk at `pos`, checking its checksum.
    fn read_chunk(&self, pos: u64, max_body: usize) -> Result<ChunkRead> {
        if pos + CHUNK_HEADER_BYTES as u64 > self.file_bytes {
            return Ok(ChunkRead::Truncated);
        }
        let head = self.source.read_at(pos, CHUNK_HEADER_BYTES)?;
        let header = match read_chunk_header(&head) {
            Ok(header) => header,
            Err(error) => return Ok(ChunkRead::Damaged(error.to_string(), None)),
        };
        let end = pos + (CHUNK_HEADER_BYTES as u64) + u64::from(header.length);
        if header.length as usize > max_body {
            return Ok(ChunkRead::Damaged(
                format!("a chunk claims {} bytes", header.length),
                None,
            ));
        }
        if end > self.file_bytes {
            return Ok(ChunkRead::Truncated);
        }
        let body = self
            .source
            .read_at(pos + CHUNK_HEADER_BYTES as u64, header.length as usize)?;
        if chunk_checksum(&head, &body) != header.checksum {
            return Ok(ChunkRead::Damaged(
                format!("the chunk at byte {pos} fails its checksum"),
                Some(end),
            ));
        }
        Ok(ChunkRead::Chunk(header, body))
    }

    /// The seek index, if the file has an intact trailer and index.
    fn read_index(&self) -> Option<(u64, Vec<IndexEntry>)> {
        if self.file_bytes < (PRELUDE_BYTES + TRAILER_BYTES) as u64 {
            return None;
        }
        let tail = self
            .source
            .read_at(self.file_bytes - TRAILER_BYTES as u64, TRAILER_BYTES)
            .ok()?;
        let offset = read_trailer(&tail)?;
        match self.read_chunk(offset, MAX_CHUNK_BYTES).ok()? {
            ChunkRead::Chunk(header, body) if header.kind == KIND_INDEX => {
                decode_index(&body, MAX_CHUNKS).ok()
            }
            _ => None,
        }
    }

    fn scan(&mut self, mut pos: u64, index: Option<&(u64, Vec<IndexEntry>)>) -> Result<()> {
        let mut footer_totals = None;
        let mut index_seen = false;
        // After damage, continue at the next chunk the index knows about.
        let resync = |pos: u64| -> Option<u64> {
            let (footer, entries) = index?;
            entries
                .iter()
                .map(|e| e.offset)
                .chain([*footer])
                .filter(|o| *o > pos)
                .min()
        };
        while pos < self.file_bytes {
            match self.read_chunk(pos, MAX_CHUNK_BYTES)? {
                ChunkRead::Truncated => {
                    self.problems.push(format!(
                        "the file ends inside a chunk at byte {pos}; the last part of the flight is missing"
                    ));
                    break;
                }
                ChunkRead::Damaged(reason, next) => {
                    self.problems.push(reason);
                    match resync(pos).or(next) {
                        Some(next) if next > pos => pos = next,
                        _ => break,
                    }
                }
                ChunkRead::Chunk(header, body) => {
                    let end = pos + (CHUNK_HEADER_BYTES + body.len()) as u64;
                    match header.kind {
                        KIND_DATA => {
                            if let Err(error) = self.index_data_chunk(pos, &header, &body) {
                                self.problems.push(format!(
                                    "the chunk at byte {pos} is unreadable: {error}"
                                ));
                            }
                        }
                        KIND_FOOTER => match decode_footer(&body) {
                            Ok((footer, frames, chunks)) => {
                                self.footer = Some(footer);
                                footer_totals = Some((frames, chunks));
                            }
                            Err(error) => self
                                .problems
                                .push(format!("the footer is unreadable: {error}")),
                        },
                        KIND_INDEX => {
                            index_seen = true;
                            let rest = self.file_bytes - end;
                            let trailer_ok = rest == TRAILER_BYTES as u64
                                && self
                                    .source
                                    .read_at(end, TRAILER_BYTES)
                                    .ok()
                                    .and_then(|t| read_trailer(&t))
                                    == Some(pos);
                            if !trailer_ok {
                                self.problems
                                    .push("the file's closing trailer is missing".into());
                            }
                            self.complete = self.footer.is_some() && trailer_ok;
                            break;
                        }
                        KIND_HEADER => self
                            .problems
                            .push(format!("a second header at byte {pos} was ignored")),
                        // Chunk kinds from newer versions are skipped.
                        _ => {}
                    }
                    pos = end;
                }
            }
        }
        if !index_seen && self.footer.is_some() {
            self.problems
                .push("the file's seek index is missing".into());
        }
        if let Some((frames, chunks)) = footer_totals {
            let found: u64 = self.chunks.iter().map(|c| u64::from(c.frames)).sum();
            if frames != found {
                self.problems.push(format!(
                    "the footer lists {frames} frames but {found} could be read"
                ));
            }
            if chunks != self.chunks.len() as u64 {
                self.problems.push(format!(
                    "the footer lists {chunks} chunks of frames but {} could be read",
                    self.chunks.len()
                ));
            }
        }
        Ok(())
    }

    fn index_data_chunk(&mut self, offset: u64, header: &ChunkHeader, body: &[u8]) -> Result<()> {
        if header.frames > MAX_CHUNK_FRAMES {
            return Err(corrupt(format!("{} frames in one chunk", header.frames)));
        }
        let first = header.first_tick;
        if header.frames > 0 {
            if first.checked_add(u64::from(header.frames)).is_none() {
                return Err(corrupt("a chunk's ticks overflow"));
            }
            if let Some(last) = self.chunks.last()
                && first <= last.last_tick()
            {
                return Err(corrupt("the chunk's ticks overlap an earlier chunk"));
            }
            if self.chunks.len() >= MAX_CHUNKS {
                return Err(corrupt("the file has too many chunks"));
            }
        }
        let sections = read_sections(body)?;
        if let Some(payload) = section(&sections, SECTION_STRINGS) {
            self.strings.define(payload)?;
        }
        let entities = section(&sections, SECTION_ENTITIES)
            .map(|p| get_entities(p, &self.strings))
            .transpose()?;
        let events = section(&sections, SECTION_EVENTS)
            .map(|p| get_events(p, header.frames, &self.strings))
            .transpose()?
            .unwrap_or_default();
        let directory = section(&sections, SECTION_TREES)
            .map(|p| get_tree_directory(p, header.frames, &self.strings))
            .transpose()?
            .unwrap_or_default();
        let checksums = section(&sections, SECTION_CHECKSUMS)
            .map(|p| get_checksums(p, header.frames))
            .transpose()?
            .unwrap_or_default();
        let event_count: usize = events.iter().map(|(_, e)| e.len()).sum();
        if self.events.len() + event_count > MAX_EVENTS_TOTAL {
            return Err(corrupt(format!(
                "the file holds more than {MAX_EVENTS_TOTAL} events"
            )));
        }
        if let Some((aircraft, weapons)) = entities {
            for info in aircraft {
                self.aircraft.entry(info.id).or_insert(info);
            }
            for info in weapons {
                self.weapons.entry(info.id).or_insert(info);
            }
        }
        if header.frames == 0 {
            return Ok(());
        }
        let chunk = self.chunks.len();
        for (frame, list) in events {
            let tick = first + u64::from(frame);
            self.events
                .extend(list.into_iter().map(|event| TimedEvent { tick, event }));
        }
        for (subject, channel, last, _) in directory {
            self.trees
                .entry((subject, channel))
                .or_default()
                .push((chunk, first + u64::from(last)));
        }
        self.checksums.extend(
            checksums
                .into_iter()
                .map(|(frame, checksum)| (first + u64::from(frame), checksum)),
        );
        self.chunks.push(ChunkInfo {
            offset,
            bytes: header.length,
            first_tick: first,
            frames: header.frames,
        });
        Ok(())
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn footer(&self) -> Option<&Footer> {
        self.footer.as_ref()
    }

    /// True when the file was finished: it has its footer, seek index and
    /// trailer. A crash or a recording still in progress reads as false.
    pub fn complete(&self) -> bool {
        self.complete
    }

    /// Damage found while opening, in plain English. Empty for a healthy
    /// file, finished or not.
    pub fn problems(&self) -> &[String] {
        &self.problems
    }

    pub fn file_bytes(&self) -> u64 {
        self.file_bytes
    }

    pub fn chunks(&self) -> &[ChunkInfo] {
        &self.chunks
    }

    pub fn first_tick(&self) -> Option<u64> {
        self.chunks.first().map(|c| c.first_tick)
    }

    pub fn last_tick(&self) -> Option<u64> {
        self.chunks.last().map(ChunkInfo::last_tick)
    }

    /// Frames that can be read.
    pub fn frame_count(&self) -> u64 {
        self.chunks.iter().map(|c| u64::from(c.frames)).sum()
    }

    /// Tick ranges with no frames between the first and last tick, inclusive.
    pub fn gaps(&self) -> Vec<(u64, u64)> {
        self.chunks
            .windows(2)
            .filter(|w| w[1].first_tick > w[0].last_tick() + 1)
            .map(|w| (w[0].last_tick() + 1, w[1].first_tick - 1))
            .collect()
    }

    pub fn aircraft(&self) -> impl Iterator<Item = &AircraftInfo> {
        self.aircraft.values()
    }

    pub fn aircraft_info(&self, id: u32) -> Option<&AircraftInfo> {
        self.aircraft.get(&id)
    }

    pub fn weapons(&self) -> impl Iterator<Item = &WeaponInfo> {
        self.weapons.values()
    }

    pub fn weapon_info(&self, id: u32) -> Option<&WeaponInfo> {
        self.weapons.get(&id)
    }

    /// Every event in the recording, in tick order.
    pub fn events(&self) -> &[TimedEvent] {
        &self.events
    }

    /// Events from `from` to `to`, inclusive.
    pub fn events_between(&self, from: u64, to: u64) -> &[TimedEvent] {
        let start = self.events.partition_point(|e| e.tick < from);
        let end = self.events.partition_point(|e| e.tick <= to);
        &self.events[start..end.max(start)]
    }

    /// Every stored state checksum: `(tick, checksum)`.
    pub fn checksums(&self) -> &[(u64, u64)] {
        &self.checksums
    }

    /// The data chunk holding `tick`.
    pub fn chunk_for_tick(&self, tick: u64) -> Option<usize> {
        let i = self.chunks.partition_point(|c| c.first_tick <= tick);
        (i > 0 && self.chunks[i - 1].contains(tick)).then(|| i - 1)
    }

    fn chunk_body(&self, index: usize) -> Result<(ChunkInfo, Vec<u8>)> {
        let info = *self
            .chunks
            .get(index)
            .ok_or_else(|| Error::Invalid(format!("there is no chunk {index}")))?;
        match self.read_chunk(info.offset, MAX_CHUNK_BYTES)? {
            ChunkRead::Chunk(_, body) => Ok((info, body)),
            _ => Err(corrupt(format!(
                "the chunk at byte {} changed since the file was opened",
                info.offset
            ))),
        }
    }

    /// Every frame of one chunk, in tick order. The viewer keeps a few of
    /// these around the playhead to play forwards and backwards.
    pub fn decode_chunk(&self, index: usize) -> Result<Vec<Frame>> {
        let (info, body) = self.chunk_body(index)?;
        let sections = read_sections(&body)?;
        decode_frames(&sections, info.first_tick, info.frames, &self.strings)
    }

    /// The frame at `tick`, if one was recorded.
    pub fn frame(&self, tick: u64) -> Result<Option<Frame>> {
        let Some(index) = self.chunk_for_tick(tick) else {
            return Ok(None);
        };
        let cached = lock(&self.frame_cache)
            .as_ref()
            .filter(|(i, _)| *i == index)
            .map(|(_, frames)| Arc::clone(frames));
        let frames = match cached {
            Some(frames) => frames,
            None => {
                let frames = Arc::new(self.decode_chunk(index)?);
                *lock(&self.frame_cache) = Some((index, Arc::clone(&frames)));
                frames
            }
        };
        let first = self.chunks[index].first_tick;
        Ok(frames.get((tick - first) as usize).cloned())
    }

    /// Frames from `from` to `to`, inclusive, decoded a chunk at a time.
    pub fn frames(&self, from: u64, to: u64) -> FrameIter<'_> {
        let chunk = self.chunks.partition_point(|c| c.last_tick() < from);
        FrameIter {
            recording: self,
            chunk,
            pending: Vec::new().into_iter(),
            from,
            to,
            failed: false,
        }
    }

    /// Every display tree sample in one chunk: `(tick, sample)`.
    pub fn chunk_trees(&self, index: usize) -> Result<Vec<(u64, TreeSample)>> {
        let (info, body) = self.chunk_body(index)?;
        let sections = read_sections(&body)?;
        let Some(payload) = section(&sections, SECTION_TREES) else {
            return Ok(Vec::new());
        };
        Ok(get_trees(payload, info.frames, &self.strings)?
            .into_iter()
            .map(|(frame, tree)| (info.first_tick + u64::from(frame), tree))
            .collect())
    }

    /// The latest sample of `subject` on `channel` at or before `tick`, with
    /// the tick it was taken.
    pub fn tree(
        &self,
        subject: u32,
        channel: &str,
        tick: u64,
    ) -> Result<Option<(u64, TreeSample)>> {
        let Some(list) = self.trees.get(&(subject, channel.to_owned())) else {
            return Ok(None);
        };
        let candidates = list.partition_point(|(chunk, _)| self.chunks[*chunk].first_tick <= tick);
        for &(chunk, _) in list[..candidates].iter().rev() {
            let found = self
                .chunk_trees(chunk)?
                .into_iter()
                .filter(|(t, tree)| {
                    *t <= tick && tree.subject == subject && tree.channel == channel
                })
                .next_back();
            if found.is_some() {
                return Ok(found);
            }
            // A chunk wholly before `tick` always has a sample in range, so
            // only the chunk holding `tick` can come up empty.
        }
        Ok(None)
    }

    fn chunk_spawns(&self, index: usize) -> Result<Arc<Vec<FrameSpawns>>> {
        if let Some(found) = lock(&self.spawn_cache).get(&index) {
            return Ok(Arc::clone(found));
        }
        let (info, body) = self.chunk_body(index)?;
        let sections = read_sections(&body)?;
        let spawns = Arc::new(match section(&sections, SECTION_SPAWNS) {
            Some(payload) => get_spawns(payload, info.frames)?,
            None => Vec::new(),
        });
        let mut cache = lock(&self.spawn_cache);
        if cache.len() >= SPAWN_CACHE_CHUNKS {
            cache.clear();
        }
        cache.insert(index, Arc::clone(&spawns));
        Ok(spawns)
    }

    /// Effects and puffs released from `from` to `to`, inclusive, in order.
    pub fn spawns(&self, from: u64, to: u64) -> Result<Spawns> {
        let mut out = Spawns::default();
        let start = self.chunks.partition_point(|c| c.last_tick() < from);
        for index in start..self.chunks.len() {
            let first = self.chunks[index].first_tick;
            if first > to {
                break;
            }
            for (frame, effects, puffs) in self.chunk_spawns(index)?.iter() {
                let tick = first + u64::from(*frame);
                if tick < from || tick > to {
                    continue;
                }
                out.effects
                    .extend(effects.iter().map(|e| (tick, e.clone())));
                out.puffs.extend(puffs.iter().map(|p| (tick, p.clone())));
            }
        }
        Ok(out)
    }

    /// Every smoke and contrail puff alive at `tick`, rebuilt from release
    /// times the way the simulation ages them: smoke rises 2 feet per second,
    /// contrails stay put, missile smoke lasts 480 ticks, aircraft smoke 960
    /// and contrails 14,400, and each layer keeps only its newest puffs up to
    /// the simulation's caps. Kinds with no known lifetime are left out.
    pub fn live_puffs(&self, tick: u64) -> Result<Vec<LivePuff>> {
        let spawns = self.spawns(tick.saturating_sub(LONGEST_PUFF_TICKS - 1), tick)?;
        let mut smoke = Vec::new();
        let mut contrails = Vec::new();
        for (spawn_tick, puff) in spawns.puffs {
            let age = tick - spawn_tick;
            let Some(lifetime) = puff.kind.lifetime_ticks() else {
                continue;
            };
            if age >= lifetime {
                continue;
            }
            let mut position = puff.position;
            position[1] += puff.kind.rise_fps() * age as f64 / 120.;
            let live = LivePuff {
                spawn_tick,
                age_ticks: age,
                layer: puff.layer,
                kind: puff.kind,
                position,
            };
            if puff.layer == LAYER_CONTRAILS {
                contrails.push(live);
            } else {
                smoke.push(live);
            }
        }
        let keep_newest = |list: &mut Vec<LivePuff>, cap: usize| {
            if list.len() > cap {
                list.drain(..list.len() - cap);
            }
        };
        keep_newest(&mut smoke, SMOKE_PUFF_CAP);
        keep_newest(&mut contrails, CONTRAIL_PUFF_CAP);
        smoke.extend(contrails);
        Ok(smoke)
    }

    /// Every effect still playing at `tick`, looking back up to `lookback`
    /// ticks for its start.
    pub fn live_effects(&self, tick: u64, lookback: u64) -> Result<Vec<LiveEffect>> {
        let spawns = self.spawns(tick.saturating_sub(lookback), tick)?;
        Ok(spawns
            .effects
            .into_iter()
            .filter(|(spawn_tick, e)| tick - spawn_tick < u64::from(e.duration_ticks))
            .map(|(spawn_tick, e)| LiveEffect {
                spawn_tick,
                age_ticks: tick - spawn_tick,
                kind: e.kind,
                position: e.position,
                duration_ticks: e.duration_ticks,
            })
            .collect())
    }
}

/// Forward iteration over a tick range. Yields an error once and stops if a
/// chunk cannot be decoded.
pub struct FrameIter<'a> {
    recording: &'a Recording,
    chunk: usize,
    pending: std::vec::IntoIter<Frame>,
    from: u64,
    to: u64,
    failed: bool,
}

impl Iterator for FrameIter<'_> {
    type Item = Result<Frame>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(frame) = self.pending.next() {
                if frame.tick < self.from {
                    continue;
                }
                if frame.tick > self.to {
                    return None;
                }
                return Some(Ok(frame));
            }
            if self.failed
                || self.chunk >= self.recording.chunks.len()
                || self.recording.chunks[self.chunk].first_tick > self.to
            {
                return None;
            }
            match self.recording.decode_chunk(self.chunk) {
                Ok(frames) => self.pending = frames.into_iter(),
                Err(error) => {
                    self.failed = true;
                    return Some(Err(error));
                }
            }
            self.chunk += 1;
        }
    }
}
