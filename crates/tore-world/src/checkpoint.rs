//! The checkpoint container: `World::checkpoint` and `World::restore`
//! (docs/formats/checkpoint.md, "Layout" and "Sections").
//!
//! A checkpoint is a header (magic, container version, process switches, the
//! tick, the mission identity), the shared records, one section per mutable
//! field of `World`, and a CRC-32. Each section's coder lives beside the state
//! it codes (`<module>_checkpoint.rs`); this module frames them, checks the
//! whole container before touching the world, and restores the sections over
//! a world built fresh from the same mission.

use crate::seats::{PlaneId, Roster, SeatId};
use crate::world::World;
use tore_codec::{BitReader, BitWriter, crc32};
use tore_sim::checkpoint::{Checkpoint, CheckpointError, InPlace, Loader, Models, Saver};

/// The first eight bytes of every checkpoint.
pub const MAGIC: [u8; 8] = *b"TORECKPT";

/// The container's version: it changes only when the header, the section
/// framing or the shared-record table changes. A section's coding changes
/// with the build, which the session's handshake already checks.
pub const VERSION: u16 = 1;

/// Switch bit: the process runs with the retail stall speeds.
const RETAIL_STALL_SPEEDS: u8 = 1;

/// The shared records' section id.
const RECORDS: u8 = 0;

/// The most shared records a checkpoint may hold.
const MAX_RECORDS: u64 = 1 << 20;

/// One section per mutable field of `World`, in id order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Section {
    Roster,
    Combat,
    AiWings,
    Cockpits,
    Weather,
    Comms,
    WingStatus,
    Radio,
    DataLink,
    Score,
}

impl Section {
    /// Every section, in id order.
    pub const ALL: [Section; 10] = [
        Section::Roster,
        Section::Combat,
        Section::AiWings,
        Section::Cockpits,
        Section::Weather,
        Section::Comms,
        Section::WingStatus,
        Section::Radio,
        Section::DataLink,
        Section::Score,
    ];

    pub fn id(self) -> u8 {
        match self {
            Section::Roster => 1,
            Section::Combat => 2,
            Section::AiWings => 3,
            Section::Cockpits => 4,
            Section::Weather => 5,
            Section::Comms => 6,
            Section::WingStatus => 7,
            Section::Radio => 8,
            Section::DataLink => 9,
            Section::Score => 10,
        }
    }

    pub fn from_id(id: u8) -> Option<Section> {
        Section::ALL.into_iter().find(|section| section.id() == id)
    }

    pub fn name(self) -> &'static str {
        match self {
            Section::Roster => "roster",
            Section::Combat => "combat",
            Section::AiWings => "AI wings",
            Section::Cockpits => "cockpits",
            Section::Weather => "weather",
            Section::Comms => "comms",
            Section::WingStatus => "wing status",
            Section::Radio => "radio",
            Section::DataLink => "data link",
            Section::Score => "score",
        }
    }
}

/// What a checkpoint's container says, read and checked without restoring
/// anything: for measurement and for the restore's own first pass.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Layout {
    pub version: u16,
    pub retail_stall_speeds: bool,
    /// The tick the next step runs.
    pub tick: u64,
    pub identity: u64,
    /// The shared records' coding, in index order.
    pub records: Vec<Vec<u8>>,
    /// The bytes of the shared-record section.
    pub records_bytes: usize,
    /// Each section present and its body's byte range in the checkpoint.
    pub sections: Vec<(Section, std::ops::Range<usize>)>,
}

impl Layout {
    /// The body of `section`, if the checkpoint holds it.
    pub fn body<'a>(&self, bytes: &'a [u8], section: Section) -> Option<&'a [u8]> {
        self.sections
            .iter()
            .find(|(s, _)| *s == section)
            .map(|(_, range)| &bytes[range.clone()])
    }
}

fn invalid<T>(why: impl Into<String>) -> Result<T, CheckpointError> {
    tore_sim::checkpoint::invalid(why)
}

/// Reads and checks a checkpoint's container: the CRC, the magic, the
/// version, the framing of every section and the shared records. Nothing in
/// it is decoded beyond that.
pub fn layout(bytes: &[u8]) -> Result<Layout, CheckpointError> {
    if bytes.len() < MAGIC.len() + 4 {
        return invalid("too short to be a checkpoint");
    }
    let (body, crc) = bytes.split_at(bytes.len() - 4);
    if crc32(body).to_le_bytes() != crc {
        return invalid("damaged: the CRC-32 does not match");
    }
    let mut r = BitReader::new(body);
    if r.read_bytes(MAGIC.len())? != MAGIC {
        return invalid("not a checkpoint");
    }
    let version = r.read_bits(16)? as u16;
    if version != VERSION {
        return invalid(format!(
            "container version {version}, this build reads {VERSION}"
        ));
    }
    let switches = r.read_bits(8)? as u8;
    if switches & !RETAIL_STALL_SPEEDS != 0 {
        return invalid(format!("unknown process switches {switches:#04x}"));
    }
    let tick = r.read_bits(64)?;
    let identity = r.read_bits(64)?;
    let count = r.read_varint()?;
    if count > Section::ALL.len() as u64 + 1 {
        return invalid(format!("{count} sections"));
    }
    let mut records = None;
    let mut records_bytes = 0;
    let mut sections = Vec::new();
    let mut last: Option<u8> = None;
    for _ in 0..count {
        let id = r.read_bits(8)? as u8;
        if last.is_some_and(|last| id <= last) {
            return invalid(format!("section {id} out of order"));
        }
        last = Some(id);
        let len = usize::try_from(r.read_varint()?)
            .map_err(|_| CheckpointError::Invalid("section too long".into()))?;
        r.align_strict()?;
        let start = r.bit_position() / 8;
        if len > r.bits_remaining() / 8 {
            return invalid(format!("section {id} runs past the end"));
        }
        r.read_bytes(len)?;
        let range = start..start + len;
        if id == RECORDS {
            records = Some(read_records(&body[range.clone()])?);
            records_bytes = len;
        } else if let Some(section) = Section::from_id(id) {
            sections.push((section, range));
        } else {
            return invalid(format!("unknown section {id}"));
        }
    }
    r.align_strict()?;
    if r.bits_remaining() != 0 {
        return invalid("bytes after the last section");
    }
    let Some(records) = records else {
        return invalid("no shared-record section");
    };
    Ok(Layout {
        version,
        retail_stall_speeds: switches & RETAIL_STALL_SPEEDS != 0,
        tick,
        identity,
        records,
        records_bytes,
        sections,
    })
}

fn read_records(bytes: &[u8]) -> Result<Vec<Vec<u8>>, CheckpointError> {
    let mut r = BitReader::new(bytes);
    let count = r.read_varint()?;
    if count > MAX_RECORDS || count > r.bits_remaining() as u64 {
        return invalid(format!("{count} shared records"));
    }
    let mut records = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let len = usize::try_from(r.read_varint()?)
            .map_err(|_| CheckpointError::Invalid("shared record too long".into()))?;
        if len > r.bits_remaining() / 8 {
            return invalid("a shared record runs past its section");
        }
        records.push(r.read_bytes(len)?);
    }
    if r.bits_remaining() != 0 {
        return invalid("bytes after the last shared record");
    }
    Ok(records)
}

impl World {
    /// Every piece of mutable mission state, between two ticks, as bytes
    /// (docs/formats/checkpoint.md). [`World::restore`] loads it into a world
    /// built fresh from the same mission.
    pub fn checkpoint(&self) -> Result<Vec<u8>, CheckpointError> {
        self.write_checkpoint(&Section::ALL)
    }

    /// A checkpoint of only `sections` (and the shared records they use):
    /// for the twin restore of a slice's sections and for measuring them.
    #[cfg(any(test, feature = "test-support"))]
    pub fn checkpoint_sections(&self, sections: &[Section]) -> Result<Vec<u8>, CheckpointError> {
        self.write_checkpoint(sections)
    }

    /// Loads a checkpoint over this world, which must be built fresh from
    /// the same mission and import. The mission setup stays this world's;
    /// everything else, including which planes humans fly, comes from the
    /// bytes. The whole container is checked before anything changes; if a
    /// section then fails, the world is half restored and must be discarded.
    pub fn restore(&mut self, bytes: &[u8]) -> Result<(), CheckpointError> {
        let layout = self.checked_layout(bytes)?;
        if layout.sections.len() != Section::ALL.len() {
            return invalid("the checkpoint does not hold every section");
        }
        self.restore_layout(bytes, &layout)
    }

    /// Restores the sections a checkpoint holds and leaves the rest of this
    /// world alone: the twin restore of docs/formats/checkpoint.md, for a
    /// world stepped to the same tick the same way. Returns the sections
    /// restored.
    #[cfg(any(test, feature = "test-support"))]
    pub fn restore_sections(&mut self, bytes: &[u8]) -> Result<Vec<Section>, CheckpointError> {
        let layout = self.checked_layout(bytes)?;
        self.restore_layout(bytes, &layout)?;
        Ok(layout
            .sections
            .iter()
            .map(|(section, _)| *section)
            .collect())
    }

    /// FNV-1a 64 over what the fresh world fixes and a checkpoint relies on:
    /// the terrain's layout, the aircraft types loaded, every plane's slot,
    /// and the setup's start. It catches a restore into another mission.
    pub fn mission_identity(&self) -> u64 {
        let mut hash = tore_codec::Fnv1a64::new();
        hash.update(self.terrain.layout.as_bytes());
        hash.update(&[0xff]);
        for aircraft in self.combat.dummy_types() {
            hash.update(aircraft.profile.id.pt().as_bytes());
            hash.update(&[0xfe]);
        }
        for plane in self.roster.planes() {
            let PlaneId(id) = plane.id;
            hash.update(&id.to_le_bytes());
            hash.update(&[
                u8::from(plane.slot.wing.side == tore_sim::ai::launch::Side::Enemy),
                plane.slot.wing.index,
                plane.slot.member,
            ]);
        }
        if let Some((altitude, fuel)) = self.setup.mission {
            hash.update(&altitude.to_bits().to_le_bytes());
            hash.update(&fuel.to_bits().to_le_bytes());
        }
        if let Some(runway) = self.setup.ground_start {
            hash.update(&runway.to_le_bytes());
        }
        hash.finish()
    }

    /// Every distinct flight model this world holds, as the import built it
    /// (before weight scaling): what flight states are coded against. The
    /// set is the same for the whole mission, since handoffs move flights,
    /// never models, so a fresh world gives the same table.
    fn models(&self) -> Result<Models, CheckpointError> {
        let mut table = Models::default();
        for aircraft in self.combat.dummy_types() {
            table.insert(aircraft.profile.id, aircraft.model().clone())?;
        }
        if let Some(wings) = &self.ai_wings {
            for actor in wings.mission().actors() {
                table.insert(actor.identity().aircraft, actor.flight().import_model())?;
            }
        }
        for cockpit in &self.cockpits {
            if let Some(own) = self.combat.state.ownship(cockpit.plane.0) {
                table.insert(own.configuration().aircraft, cockpit.flight.import_model())?;
            }
        }
        Ok(table)
    }

    fn write_checkpoint(&self, sections: &[Section]) -> Result<Vec<u8>, CheckpointError> {
        let mut s = Saver::with_models(self.models()?);
        let mut bodies = Vec::new();
        for section in Section::ALL {
            if sections.contains(&section) {
                self.save_section(section, &mut s)?;
                bodies.push((section.id(), s.finish_section()));
            }
        }
        let mut records = BitWriter::new();
        let shared = s.into_records();
        records.write_varint(shared.len() as u64);
        for record in &shared {
            records.write_varint(record.len() as u64);
            records.write_bytes(record);
        }
        bodies.insert(0, (RECORDS, records.finish()));

        let mut w = BitWriter::new();
        w.write_bytes(&MAGIC);
        let switches = if tore_sim::flight::retail_stall_speeds() {
            RETAIL_STALL_SPEEDS
        } else {
            0
        };
        w.write_bits(u64::from(VERSION), 16)?;
        w.write_bits(u64::from(switches), 8)?;
        w.write_bits(self.tick(), 64)?;
        w.write_bits(self.mission_identity(), 64)?;
        w.write_varint(bodies.len() as u64);
        for (id, body) in &bodies {
            w.write_bits(u64::from(*id), 8)?;
            w.write_varint(body.len() as u64);
            w.align();
            w.write_bytes(body);
        }
        w.align();
        let mut bytes = w.finish();
        let crc = crc32(&bytes);
        bytes.extend_from_slice(&crc.to_le_bytes());
        Ok(bytes)
    }

    /// The container checked against this process and this world.
    fn checked_layout(&self, bytes: &[u8]) -> Result<Layout, CheckpointError> {
        let layout = layout(bytes)?;
        if layout.retail_stall_speeds != tore_sim::flight::retail_stall_speeds() {
            return invalid(
                "the checkpoint's retail stall speed switch differs from this process's",
            );
        }
        if layout.identity != self.mission_identity() {
            return invalid("the checkpoint is of another mission");
        }
        Ok(layout)
    }

    fn restore_layout(&mut self, bytes: &[u8], layout: &Layout) -> Result<(), CheckpointError> {
        let models = self.models()?;
        for (section, range) in &layout.sections {
            let mut l = Loader::new(&bytes[range.clone()], &layout.records, &models);
            self.restore_section(*section, &mut l)?;
            l.finish()?;
        }
        if layout
            .sections
            .iter()
            .any(|(section, _)| *section == Section::Combat)
            && self.tick() != layout.tick
        {
            return invalid(format!(
                "restored to tick {} but the checkpoint is of tick {}",
                self.tick(),
                layout.tick
            ));
        }
        Ok(())
    }

    /// Writes one section. Destructuring `World` makes a new field fail to
    /// compile here until it has a section or is named as setup.
    fn save_section(&self, section: Section, s: &mut Saver) -> Result<(), CheckpointError> {
        let World {
            // Mission setup: the fresh world has it.
            setup: _,
            // Mission setup but for the weather clock, the weather section.
            terrain,
            roster,
            // The cockpits section reads the whole world for each plane's
            // aircraft identity.
            cockpits: _,
            combat,
            ai_wings,
            comms,
            wing_status,
            radio,
            // Imported phrase text: mission setup.
            phrases: _,
            // Stage G's data link (G0), the data link section (H10).
            datalink,
            // Stage F phase 2's score recorder (F2-S), the score section
            // (H10): present only while the host has scoring on.
            score,
        } = self;
        match section {
            Section::Roster => roster.save(s, None),
            Section::Combat => combat.save_in_place(s),
            Section::AiWings => {
                s.writer().write_bool(ai_wings.is_some());
                match ai_wings {
                    Some(wings) => wings.save_in_place(s),
                    None => Ok(()),
                }
            }
            Section::Cockpits => crate::world::checkpoint::save_cockpits(self, s),
            Section::Weather => terrain.weather.save_in_place(s),
            Section::Comms => comms.save(s, None),
            Section::WingStatus => wing_status.save(s, None),
            Section::Radio => radio.save(s, None),
            Section::DataLink => datalink.save(s, None),
            Section::Score => {
                s.writer().write_bool(score.is_some());
                match score {
                    Some(recorder) => recorder.save(s, None),
                    None => Ok(()),
                }
            }
        }
    }

    fn restore_section(
        &mut self,
        section: Section,
        l: &mut Loader<'_>,
    ) -> Result<(), CheckpointError> {
        match section {
            Section::Roster => self.roster = Roster::load(l, None)?,
            Section::Combat => self.combat.restore_in_place(l)?,
            Section::AiWings => {
                let present = l.reader().read_bool()?;
                match (present, self.ai_wings.as_mut()) {
                    (true, Some(wings)) => wings.restore_in_place(l)?,
                    (false, None) => {}
                    _ => return invalid("the AI wings exist in only one of the two worlds"),
                }
            }
            Section::Cockpits => crate::world::checkpoint::restore_cockpits(self, l)?,
            Section::Weather => self.terrain.weather.restore_in_place(l)?,
            Section::Comms => self.comms = Checkpoint::load(l, None)?,
            Section::WingStatus => self.wing_status = Checkpoint::load(l, None)?,
            Section::Radio => self.radio = Checkpoint::load(l, None)?,
            Section::DataLink => self.datalink = Checkpoint::load(l, None)?,
            Section::Score => {
                self.score = if l.reader().read_bool()? {
                    Some(Checkpoint::load(l, None)?)
                } else {
                    None
                }
            }
        }
        Ok(())
    }
}

// The small identity types every section's coder shares.
tore_sim::checkpoint_tuple!(PlaneId(id));
tore_sim::checkpoint_tuple!(SeatId(id));
tore_sim::checkpoint_enum!(crate::comms::Crew { Rio = 0, CoPilot = 1 });
