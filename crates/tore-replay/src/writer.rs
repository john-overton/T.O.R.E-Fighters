//! The synchronous writer. The app runs it on its own thread.
//!
//! It writes to `final_path` plus `.partial`, flushes each finished chunk
//! (one second of frames by default) so a crash keeps everything up to the
//! last second, syncs to disk every 30 seconds of ticks, and on `finish`
//! writes the footer, the seek index and the trailer, syncs, and renames the
//! file to its final name.

use crate::chunk::ChunkEncoder;
use crate::error::{Error, Result, invalid};
use crate::format::{
    CHUNK_HEADER_BYTES, IndexEntry, KIND_FOOTER, KIND_HEADER, KIND_INDEX, chunk, encode_footer,
    encode_header, encode_index, prelude, trailer,
};
use crate::limits::{
    MAX_AIRCRAFT, MAX_CHUNK_BYTES, MAX_CHUNK_FRAMES, MAX_CHUNKS, MAX_DEBRIS, MAX_EFFECTS_PER_TICK,
    MAX_ESCAPEES, MAX_EVENTS_PER_TICK, MAX_FIELDS_PER_EVENT, MAX_FILE_BYTES, MAX_IDS_PER_VALUE,
    MAX_PROJECTILES, MAX_PUFFS_PER_TICK, MAX_REGISTERED, MAX_STRING_BYTES,
    MAX_SURFACE_CHANGES_PER_TICK, MAX_TREE_DEPTH, MAX_TREE_NODES, MAX_TREES_PER_TICK,
};
use crate::model::{
    AircraftInfo, EffectKind, Event, Footer, Frame, Header, PuffKind, TreeSample, Value, WeaponInfo,
};
use crate::strings::Interner;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Room kept below the file limit for the footer and seek index.
const FINISH_RESERVE: u64 = 20 << 20;

/// How the writer cuts and syncs chunks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WriterOptions {
    /// Frames per chunk, 1 to 240. Default 120: one second, so a crash loses
    /// at most the last second.
    pub chunk_ticks: u32,
    /// Ticks between syncs to disk. Default 3,600: 30 seconds.
    pub sync_ticks: u64,
}

impl Default for WriterOptions {
    fn default() -> Self {
        Self {
            chunk_ticks: 120,
            sync_ticks: 3_600,
        }
    }
}

/// Why the writer stopped accepting frames.
enum Stop {
    /// The file reached its size limit. `finish` still works.
    Limit(String),
    /// Writing failed. The file keeps what was written before.
    Failed(String),
}

impl Stop {
    fn reason(&self) -> String {
        match self {
            Self::Limit(reason) | Self::Failed(reason) => reason.clone(),
        }
    }
}

/// The path a recording has while it is being written.
pub fn partial_path(final_path: &Path) -> PathBuf {
    let mut name = OsString::from(final_path.as_os_str());
    name.push(".partial");
    PathBuf::from(name)
}

/// Writes one recording. See the module documentation.
pub struct Writer {
    file: Option<File>,
    partial: PathBuf,
    final_path: PathBuf,
    options: WriterOptions,
    offset: u64,
    strings: Interner,
    aircraft: HashMap<u32, AircraftInfo>,
    weapons: HashMap<u32, WeaponInfo>,
    chunk: ChunkEncoder,
    last_tick: Option<u64>,
    frames: u64,
    index: Vec<IndexEntry>,
    unsynced_ticks: u64,
    stopped: Option<Stop>,
}

impl Writer {
    /// Starts a recording at `final_path` + `.partial` with default options.
    /// Fails if either file already exists.
    pub fn create(final_path: impl AsRef<Path>, header: &Header) -> Result<Self> {
        Self::create_with(final_path, header, WriterOptions::default())
    }

    pub fn create_with(
        final_path: impl AsRef<Path>,
        header: &Header,
        options: WriterOptions,
    ) -> Result<Self> {
        if !(1..=MAX_CHUNK_FRAMES).contains(&options.chunk_ticks) {
            return Err(invalid(format!(
                "chunks hold 1 to {MAX_CHUNK_FRAMES} frames, not {}",
                options.chunk_ticks
            )));
        }
        let final_path = final_path.as_ref().to_path_buf();
        if final_path.exists() {
            return Err(invalid(format!("{} already exists", final_path.display())));
        }
        let header = encode_header(header)?;
        let partial = partial_path(&final_path);
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&partial)?;
        let mut start = prelude();
        start.extend_from_slice(&chunk(KIND_HEADER, 0, 0, &header));
        file.write_all(&start)?;
        file.flush()?;
        Ok(Self {
            file: Some(file),
            partial,
            final_path,
            options,
            offset: start.len() as u64,
            strings: Interner::default(),
            aircraft: HashMap::new(),
            weapons: HashMap::new(),
            chunk: ChunkEncoder::default(),
            last_tick: None,
            frames: 0,
            index: Vec::new(),
            unsynced_ticks: 0,
            stopped: None,
        })
    }

    pub fn partial_path(&self) -> &Path {
        &self.partial
    }

    pub fn final_path(&self) -> &Path {
        &self.final_path
    }

    /// Frames accepted so far.
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// Bytes written to disk so far (the chunk being filled is not included).
    pub fn bytes_written(&self) -> u64 {
        self.offset
    }

    pub fn last_tick(&self) -> Option<u64> {
        self.last_tick
    }

    fn check_running(&self) -> Result<()> {
        match &self.stopped {
            Some(stop) => Err(Error::Stopped(stop.reason())),
            None => Ok(()),
        }
    }

    /// Registers an aircraft's identity. Registering the same identity again
    /// does nothing; a different identity for a registered id is refused.
    pub fn register_aircraft(&mut self, info: &AircraftInfo) -> Result<()> {
        self.check_running()?;
        if let Some(old) = self.aircraft.get(&info.id) {
            return if old == info {
                Ok(())
            } else {
                Err(invalid(format!(
                    "aircraft {} is already registered as {} ({})",
                    info.id, old.label, old.name
                )))
            };
        }
        if self.aircraft.len() >= MAX_REGISTERED {
            return Err(invalid(format!(
                "more than {MAX_REGISTERED} registered aircraft"
            )));
        }
        for text in [&info.pt, &info.name, &info.label, &info.skill] {
            check_string("an aircraft identity", text)?;
        }
        self.make_room(512)?;
        self.chunk.add_aircraft(info, &mut self.strings);
        self.aircraft.insert(info.id, info.clone());
        Ok(())
    }

    /// Registers a weapon's identity, with the same rules as aircraft.
    pub fn register_weapon(&mut self, info: &WeaponInfo) -> Result<()> {
        self.check_running()?;
        if let Some(old) = self.weapons.get(&info.id) {
            return if old == info {
                Ok(())
            } else {
                Err(invalid(format!(
                    "weapon {} is already registered as {}",
                    info.id, old.name
                )))
            };
        }
        if self.weapons.len() >= MAX_REGISTERED {
            return Err(invalid(format!(
                "more than {MAX_REGISTERED} registered weapons"
            )));
        }
        for text in [&info.source, &info.name]
            .into_iter()
            .chain(info.shape.as_ref())
        {
            check_string("a weapon identity", text)?;
        }
        self.make_room(512)?;
        self.chunk.add_weapon(info, &mut self.strings);
        self.weapons.insert(info.id, info.clone());
        Ok(())
    }

    /// Adds one frame. A frame that breaks a rule is refused with a clear
    /// message and nothing is written for it. Ticks must increase; a jump
    /// starts a new chunk, and the missing ticks read back as a gap.
    pub fn push(&mut self, frame: &Frame) -> Result<()> {
        self.check_running()?;
        if let Some(last) = self.last_tick
            && frame.tick <= last
        {
            return Err(invalid(format!(
                "tick {} does not come after tick {last}",
                frame.tick
            )));
        }
        let bound = validate(frame)?;
        if bound + CHUNK_HEADER_BYTES + 1024 > MAX_CHUNK_BYTES {
            return Err(invalid(format!(
                "the frame at tick {} needs up to {bound} bytes, more than one chunk holds",
                frame.tick
            )));
        }
        let consecutive = self.last_tick.is_none_or(|last| frame.tick == last + 1);
        if self.chunk.frames > 0 && (!consecutive || self.chunk.frames >= self.options.chunk_ticks)
        {
            self.flush_chunk()?;
        }
        self.make_room(bound)?;
        self.chunk.push(frame, &mut self.strings);
        self.last_tick = Some(frame.tick);
        self.frames += 1;
        if self.chunk.frames >= self.options.chunk_ticks {
            self.flush_chunk()?;
        }
        Ok(())
    }

    /// Writes the current chunk first if `bytes` more would not fit in it.
    fn make_room(&mut self, bytes: usize) -> Result<()> {
        if !self.chunk.is_empty()
            && self.chunk.size() + self.strings.pending_bytes() + bytes + 1024 > MAX_CHUNK_BYTES
        {
            self.flush_chunk()?;
        }
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) -> Result<()> {
        let Some(file) = self.file.as_mut() else {
            return Err(Error::Stopped("the file is closed".into()));
        };
        if let Err(error) = file.write_all(bytes).and_then(|()| file.flush()) {
            self.stopped = Some(Stop::Failed(format!("writing failed: {error}")));
            return Err(error.into());
        }
        self.offset += bytes.len() as u64;
        Ok(())
    }

    fn flush_chunk(&mut self) -> Result<()> {
        if self.chunk.is_empty() {
            return Ok(());
        }
        let encoder = std::mem::take(&mut self.chunk);
        let frames = encoder.frames;
        let next_tick = self.last_tick.map_or(0, |t| t + 1);
        let first_tick = encoder.first_tick.unwrap_or(next_tick);
        let bytes = encoder.finish(&mut self.strings, next_tick);
        if self.offset + bytes.len() as u64 + FINISH_RESERVE > MAX_FILE_BYTES
            || self.index.len() >= MAX_CHUNKS
        {
            // These frames are dropped; the footer counts only what was kept.
            self.frames -= u64::from(frames);
            let reason = format!(
                "the recording reached its size limit of {} MiB",
                MAX_FILE_BYTES >> 20
            );
            self.stopped = Some(Stop::Limit(reason.clone()));
            return Err(Error::Stopped(reason));
        }
        let offset = self.offset;
        self.write(&bytes)?;
        if frames > 0 {
            self.index.push(IndexEntry {
                offset,
                first_tick,
                frames,
            });
        }
        self.unsynced_ticks += u64::from(frames);
        if self.unsynced_ticks >= self.options.sync_ticks {
            self.unsynced_ticks = 0;
            if let Some(file) = &self.file
                && let Err(error) = file.sync_all()
            {
                self.stopped = Some(Stop::Failed(format!("syncing failed: {error}")));
                return Err(error.into());
            }
        }
        Ok(())
    }

    /// Writes the footer and seek index, syncs, renames the file to its
    /// final name and returns that path. Works after the size limit stopped
    /// the recording, but not after a failed write.
    pub fn finish(mut self, footer: &Footer) -> Result<PathBuf> {
        if let Some(Stop::Failed(reason)) = &self.stopped {
            return Err(Error::Stopped(reason.clone()));
        }
        // Check the footer before writing anything, so a bad one changes nothing.
        encode_footer(footer, 0, 0)?;
        // Reaching the size limit here still leaves room for the footer.
        match self.flush_chunk() {
            Ok(()) | Err(Error::Stopped(_)) => {}
            Err(error) => return Err(error),
        }
        let footer_body = encode_footer(footer, self.frames, self.index.len() as u64)?;
        let footer_offset = self.offset;
        let footer_chunk = chunk(KIND_FOOTER, 0, 0, &footer_body);
        let index_offset = footer_offset + footer_chunk.len() as u64;
        let mut tail = footer_chunk;
        tail.extend_from_slice(&chunk(
            KIND_INDEX,
            0,
            0,
            &encode_index(footer_offset, &self.index),
        ));
        tail.extend_from_slice(&trailer(index_offset));
        self.write(&tail)?;
        let file = self
            .file
            .take()
            .ok_or_else(|| Error::Stopped("the file is closed".into()))?;
        file.sync_all()?;
        drop(file);
        std::fs::rename(&self.partial, &self.final_path)?;
        #[cfg(unix)]
        if let Some(parent) = self.final_path.parent()
            && let Ok(dir) = File::open(if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            })
        {
            // Make the rename itself durable; failure here loses nothing.
            let _ = dir.sync_all();
        }
        Ok(self.final_path.clone())
    }
}

impl Drop for Writer {
    /// A writer dropped without `finish` keeps its `.partial` file and
    /// flushes the frames it holds, so a crash-like exit loses nothing that
    /// was pushed.
    fn drop(&mut self) {
        if self.file.is_some() && self.stopped.is_none() {
            let _ = self.flush_chunk();
            if let Some(file) = &self.file {
                let _ = file.sync_all();
            }
        }
    }
}

fn check_string(what: &str, text: &str) -> Result<()> {
    if text.len() > MAX_STRING_BYTES {
        return Err(invalid(format!(
            "{what} holds a string of {} bytes; the limit is {MAX_STRING_BYTES}",
            text.len()
        )));
    }
    Ok(())
}

fn check_count(tick: u64, what: &str, n: usize, max: usize) -> Result<()> {
    if n > max {
        return Err(invalid(format!(
            "the frame at tick {tick} has {n} {what}; the limit is {max}"
        )));
    }
    Ok(())
}

fn string_bound(what: &str, text: &str) -> Result<usize> {
    check_string(what, text)?;
    Ok(text.len() + 6)
}

fn value_bound(what: &str, value: &Value) -> Result<usize> {
    Ok(match value {
        Value::Text(text) => 1 + string_bound(what, text)?,
        Value::Ids(ids) => {
            if ids.len() > MAX_IDS_PER_VALUE {
                return Err(invalid(format!(
                    "{what} holds {} ids; the limit is {MAX_IDS_PER_VALUE}",
                    ids.len()
                )));
            }
            12 + 10 * ids.len()
        }
        _ => 20,
    })
}

fn event_bound(event: &Event) -> Result<usize> {
    if event.kind.is_empty() {
        return Err(invalid("an event has no kind"));
    }
    if event.fields.len() > MAX_FIELDS_PER_EVENT {
        return Err(invalid(format!(
            "event {} has {} fields; the limit is {MAX_FIELDS_PER_EVENT}",
            event.kind,
            event.fields.len()
        )));
    }
    let mut bound = 24 + string_bound("an event kind", &event.kind)?;
    bound += string_bound("an event's text", &event.text)?;
    for (name, value) in &event.fields {
        if name.is_empty() {
            return Err(invalid(format!(
                "event {} has an unnamed field",
                event.kind
            )));
        }
        bound += string_bound("an event field name", name)?;
        bound += value_bound("an event field", value)?;
    }
    Ok(bound)
}

fn tree_bound(tree: &TreeSample) -> Result<usize> {
    if tree.channel.is_empty() {
        return Err(invalid("a display tree has no channel"));
    }
    if tree.nodes.len() > MAX_TREE_NODES {
        return Err(invalid(format!(
            "a {} tree has {} nodes; the limit is {MAX_TREE_NODES}",
            tree.channel,
            tree.nodes.len()
        )));
    }
    let mut bound = 24 + string_bound("a tree channel", &tree.channel)?;
    for node in &tree.nodes {
        if node.depth >= MAX_TREE_DEPTH {
            return Err(invalid(format!(
                "a {} tree node is {} levels deep; the limit is {}",
                tree.channel,
                node.depth,
                MAX_TREE_DEPTH - 1
            )));
        }
        bound += 8 + string_bound("a tree label", &node.label)?;
        bound += value_bound("a tree value", &node.value)?;
        bound += string_bound("a tree unit", &node.unit)?;
        bound += string_bound("a tree note", &node.note)?;
    }
    Ok(bound)
}

fn unique<K: Eq + std::hash::Hash>(
    tick: u64,
    what: &str,
    keys: impl Iterator<Item = K>,
) -> Result<()> {
    let mut seen = HashSet::new();
    for key in keys {
        if !seen.insert(key) {
            return Err(invalid(format!(
                "the frame at tick {tick} lists the same {what} twice"
            )));
        }
    }
    Ok(())
}

/// Checks every rule and returns an upper bound on the frame's encoded size.
fn validate(frame: &Frame) -> Result<usize> {
    let t = frame.tick;
    check_count(t, "aircraft", frame.aircraft.len(), MAX_AIRCRAFT)?;
    check_count(t, "projectiles", frame.projectiles.len(), MAX_PROJECTILES)?;
    check_count(t, "debris pieces", frame.debris.len(), MAX_DEBRIS)?;
    check_count(t, "ejected pilots", frame.escapees.len(), MAX_ESCAPEES)?;
    check_count(
        t,
        "new effects",
        frame.new_effects.len(),
        MAX_EFFECTS_PER_TICK,
    )?;
    check_count(t, "new puffs", frame.new_puffs.len(), MAX_PUFFS_PER_TICK)?;
    check_count(
        t,
        "surface changes",
        frame.surface_hp.len(),
        MAX_SURFACE_CHANGES_PER_TICK,
    )?;
    check_count(t, "events", frame.events.len(), MAX_EVENTS_PER_TICK)?;
    check_count(t, "display trees", frame.trees.len(), MAX_TREES_PER_TICK)?;
    unique(t, "aircraft", frame.aircraft.iter().map(|a| a.id))?;
    unique(t, "projectile", frame.projectiles.iter().map(|p| p.id))?;
    unique(
        t,
        "debris piece",
        frame.debris.iter().map(|d| (d.owner, d.index)),
    )?;
    for effect in &frame.new_effects {
        if let EffectKind::Other(code) = effect.kind
            && code < EffectKind::FIRST_OTHER
        {
            return Err(invalid(format!(
                "effect code {code} belongs to a named effect kind"
            )));
        }
    }
    for puff in &frame.new_puffs {
        if let PuffKind::Other(code) = puff.kind
            && code < PuffKind::FIRST_OTHER
        {
            return Err(invalid(format!(
                "puff code {code} belongs to a named puff kind"
            )));
        }
    }
    let mut bound = 64;
    bound += frame.aircraft.len() * 420;
    bound += frame.projectiles.len() * 220;
    bound += frame.debris.len() * 80;
    bound += frame.escapees.len() * 70;
    bound += frame.new_effects.len() * 48;
    bound += frame.new_puffs.len() * 40;
    bound += frame.surface_hp.len() * 16;
    for event in &frame.events {
        bound += event_bound(event)?;
    }
    for tree in &frame.trees {
        bound += tree_bound(tree)?;
    }
    Ok(bound)
}
