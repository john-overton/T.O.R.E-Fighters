//! Building and decoding data chunks. A data chunk holds up to two seconds
//! of consecutive frames plus the strings and entities first used in them.
//! Its body is a list of sections: strings, entities, frames, spawns, events,
//! trees and checksums. Readers skip section ids they do not know.

use crate::codec::{In, put_uv};
use crate::error::{Result, corrupt};
use crate::events::{ChecksumCoder, EventCoder, get_checksums, get_events};
use crate::format::{
    KIND_DATA, SECTION_CHECKSUMS, SECTION_ENTITIES, SECTION_EVENTS, SECTION_FRAMES, SECTION_SPAWNS,
    SECTION_STRINGS, SECTION_TREES, chunk, put_section, section,
};
use crate::frames::FrameCoder;
use crate::limits::MAX_REGISTERED;
use crate::model::{AircraftInfo, Frame, Side, WeaponClass, WeaponInfo};
use crate::spawns::{SpawnCoder, get_spawns};
use crate::strings::{Interner, StringTable};
use crate::trees::{TreeCoder, get_trees};

/// Writer state for the chunk being filled.
#[derive(Default)]
pub(crate) struct ChunkEncoder {
    pub first_tick: Option<u64>,
    pub frames: u32,
    frame_coder: FrameCoder,
    frames_buf: Vec<u8>,
    spawns: SpawnCoder,
    events: EventCoder,
    trees: TreeCoder,
    checksums: ChecksumCoder,
    aircraft: Vec<u8>,
    aircraft_count: u64,
    weapons: Vec<u8>,
    weapon_count: u64,
}

impl ChunkEncoder {
    pub fn push(&mut self, frame: &Frame, strings: &mut Interner) {
        if self.first_tick.is_none() {
            self.first_tick = Some(frame.tick);
        }
        let index = self.frames;
        self.frame_coder.put(&mut self.frames_buf, frame);
        self.spawns.put(index, &frame.new_effects, &frame.new_puffs);
        self.events.put(index, &frame.events, strings);
        self.trees.put(index, &frame.trees, strings);
        self.checksums.put(index, frame.checksum);
        self.frames += 1;
    }

    pub fn add_aircraft(&mut self, info: &AircraftInfo, strings: &mut Interner) {
        let buf = &mut self.aircraft;
        put_uv(buf, u64::from(info.id));
        strings.put(buf, &info.pt);
        strings.put(buf, &info.name);
        strings.put(buf, &info.label);
        buf.push(info.side.code());
        put_uv(buf, u64::from(info.wing));
        put_uv(buf, u64::from(info.member));
        strings.put(buf, &info.skill);
        buf.push(u8::from(info.human));
        self.aircraft_count += 1;
    }

    pub fn add_weapon(&mut self, info: &WeaponInfo, strings: &mut Interner) {
        let buf = &mut self.weapons;
        put_uv(buf, u64::from(info.id));
        strings.put(buf, &info.source);
        match &info.shape {
            Some(shape) => {
                buf.push(1);
                strings.put(buf, shape);
            }
            None => buf.push(0),
        }
        strings.put(buf, &info.name);
        buf.push(info.class.code());
        self.weapon_count += 1;
    }

    /// True when there is nothing to write.
    pub fn is_empty(&self) -> bool {
        self.frames == 0 && self.aircraft_count == 0 && self.weapon_count == 0
    }

    /// Bytes the body holds so far, not counting pending strings.
    pub fn size(&self) -> usize {
        self.frames_buf.len()
            + self.spawns.len()
            + self.events.len()
            + self.trees.len()
            + self.aircraft.len()
            + self.weapons.len()
            + 16 * self.frames as usize
            + 64
    }

    /// The finished chunk: header and body. New strings go in the same chunk.
    pub fn finish(self, strings: &mut Interner, next_tick: u64) -> Vec<u8> {
        let trees = self.trees.section(strings);
        let mut body = Vec::with_capacity(self.size() + strings.pending_bytes() + 64);
        if let Some(payload) = strings.take_section() {
            put_section(&mut body, SECTION_STRINGS, &payload);
        }
        if self.aircraft_count + self.weapon_count > 0 {
            let mut payload = Vec::with_capacity(self.aircraft.len() + self.weapons.len() + 8);
            put_uv(&mut payload, self.aircraft_count);
            payload.extend_from_slice(&self.aircraft);
            put_uv(&mut payload, self.weapon_count);
            payload.extend_from_slice(&self.weapons);
            put_section(&mut body, SECTION_ENTITIES, &payload);
        }
        if self.frames > 0 {
            put_section(&mut body, SECTION_FRAMES, &self.frames_buf);
        }
        if let Some(payload) = self.spawns.section() {
            put_section(&mut body, SECTION_SPAWNS, &payload);
        }
        if let Some(payload) = self.events.section() {
            put_section(&mut body, SECTION_EVENTS, &payload);
        }
        if let Some(payload) = trees {
            put_section(&mut body, SECTION_TREES, &payload);
        }
        if let Some(payload) = self.checksums.section() {
            put_section(&mut body, SECTION_CHECKSUMS, &payload);
        }
        chunk(
            KIND_DATA,
            self.frames,
            self.first_tick.unwrap_or(next_tick),
            &body,
        )
    }
}

pub(crate) fn get_entities(
    payload: &[u8],
    strings: &StringTable,
) -> Result<(Vec<AircraftInfo>, Vec<WeaponInfo>)> {
    let mut input = In::new(payload);
    let n = input.count(MAX_REGISTERED, "registered aircraft")?;
    let mut aircraft = Vec::with_capacity(n);
    for _ in 0..n {
        let id = input.u32v()?;
        let pt = strings.read(&mut input)?;
        let name = strings.read(&mut input)?;
        let label = strings.read(&mut input)?;
        let side = Side::from_code(input.u8()?);
        let wing = u16::try_from(input.uv()?).map_err(|_| corrupt("a wing number is too large"))?;
        let member =
            u16::try_from(input.uv()?).map_err(|_| corrupt("a wing member is too large"))?;
        let skill = strings.read(&mut input)?;
        let human = input.u8()? != 0;
        aircraft.push(AircraftInfo {
            id,
            pt,
            name,
            label,
            side,
            wing,
            member,
            skill,
            human,
        });
    }
    let n = input.count(MAX_REGISTERED, "registered weapons")?;
    let mut weapons = Vec::with_capacity(n);
    for _ in 0..n {
        let id = input.u32v()?;
        let source = strings.read(&mut input)?;
        let shape = match input.u8()? {
            0 => None,
            1 => Some(strings.read(&mut input)?),
            _ => return Err(corrupt("a weapon shape flag is invalid")),
        };
        let name = strings.read(&mut input)?;
        let class = WeaponClass::from_code(input.u8()?);
        weapons.push(WeaponInfo {
            id,
            source,
            shape,
            name,
            class,
        });
    }
    if !input.done() {
        return Err(corrupt("the entities section has trailing bytes"));
    }
    Ok((aircraft, weapons))
}

/// Decodes every frame of a data chunk from its sections.
pub(crate) fn decode_frames(
    sections: &[(u64, &[u8])],
    first_tick: u64,
    frames: u32,
    strings: &StringTable,
) -> Result<Vec<Frame>> {
    let mut out: Vec<Frame> = (0..frames)
        .map(|i| Frame {
            tick: first_tick + u64::from(i),
            ..Frame::default()
        })
        .collect();
    if frames == 0 {
        return Ok(out);
    }
    let payload = section(sections, SECTION_FRAMES)
        .ok_or_else(|| corrupt("a chunk with frames has no frames section"))?;
    let mut input = In::new(payload);
    let mut coder = FrameCoder::default();
    for frame in &mut out {
        coder.get(&mut input, frame)?;
    }
    if !input.done() {
        return Err(corrupt("the frames section has trailing bytes"));
    }
    if let Some(payload) = section(sections, SECTION_SPAWNS) {
        for (i, effects, puffs) in get_spawns(payload, frames)? {
            let frame = &mut out[i as usize];
            frame.new_effects = effects;
            frame.new_puffs = puffs;
        }
    }
    if let Some(payload) = section(sections, SECTION_EVENTS) {
        for (i, events) in get_events(payload, frames, strings)? {
            out[i as usize].events = events;
        }
    }
    if let Some(payload) = section(sections, SECTION_TREES) {
        for (i, tree) in get_trees(payload, frames, strings)? {
            out[i as usize].trees.push(tree);
        }
    }
    if let Some(payload) = section(sections, SECTION_CHECKSUMS) {
        for (i, checksum) in get_checksums(payload, frames)? {
            out[i as usize].checksum = Some(checksum);
        }
    }
    Ok(out)
}
