//! File framing. Layout, all integers little endian:
//!
//! ```text
//! prelude   "TOREREPL", format version (u16), reserved (u16)
//! chunk*    32-byte chunk header, then the body
//! trailer   offset of the index chunk (u64), "TORE-IDX"   (finished files only)
//! ```
//!
//! Chunk header: marker "TORC", kind (u8), flags (u8, zero), reserved (u16,
//! zero), body length (u32), frame count (u32), first tick (u64), and an
//! FNV-1a 64 checksum over the first 24 header bytes followed by the body.
//! The first chunk is the text header; data chunks follow; a finished file
//! ends with a footer chunk, an index chunk and the trailer.

use crate::FORMAT_VERSION;
use crate::codec::{FNV_OFFSET, In, fnv1a, put_text, put_u16, put_u32, put_u64, put_uv};
use crate::error::{Error, Result, corrupt, invalid};
use crate::limits::{MAX_HEADER_BYTES, MAX_KEY_VALUES, MAX_STRING_BYTES};
use crate::model::{Clouds, Footer, Header, MissionKind, World};

pub(crate) const MAGIC: &[u8; 8] = b"TOREREPL";
pub(crate) const PRELUDE_BYTES: usize = 12;
pub(crate) const CHUNK_MARKER: &[u8; 4] = b"TORC";
pub(crate) const CHUNK_HEADER_BYTES: usize = 32;
pub(crate) const TRAILER_MAGIC: &[u8; 8] = b"TORE-IDX";
pub(crate) const TRAILER_BYTES: usize = 16;

pub(crate) const KIND_HEADER: u8 = 1;
pub(crate) const KIND_DATA: u8 = 2;
pub(crate) const KIND_FOOTER: u8 = 3;
pub(crate) const KIND_INDEX: u8 = 4;

pub(crate) const SECTION_STRINGS: u64 = 1;
pub(crate) const SECTION_ENTITIES: u64 = 2;
pub(crate) const SECTION_FRAMES: u64 = 3;
pub(crate) const SECTION_SPAWNS: u64 = 4;
pub(crate) const SECTION_EVENTS: u64 = 5;
pub(crate) const SECTION_TREES: u64 = 6;
pub(crate) const SECTION_CHECKSUMS: u64 = 7;

pub(crate) fn prelude() -> Vec<u8> {
    let mut buf = MAGIC.to_vec();
    put_u16(&mut buf, FORMAT_VERSION);
    put_u16(&mut buf, 0);
    buf
}

/// Checks the prelude and returns the format version.
pub(crate) fn read_prelude(bytes: &[u8]) -> Result<u16> {
    if bytes.len() < PRELUDE_BYTES || &bytes[..8] != MAGIC {
        return Err(corrupt("this is not a T.O.R.E recording"));
    }
    let version = u16::from_le_bytes([bytes[8], bytes[9]]);
    if version == 0 {
        return Err(corrupt("format version 0 does not exist"));
    }
    if version > FORMAT_VERSION {
        return Err(Error::Unsupported(format!(
            "format version {version} is newer than this build reads ({FORMAT_VERSION})"
        )));
    }
    Ok(version)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ChunkHeader {
    pub kind: u8,
    pub length: u32,
    pub frames: u32,
    pub first_tick: u64,
    pub checksum: u64,
}

/// A whole chunk: header and body, with its checksum.
pub(crate) fn chunk(kind: u8, frames: u32, first_tick: u64, body: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(CHUNK_HEADER_BYTES + body.len());
    buf.extend_from_slice(CHUNK_MARKER);
    buf.push(kind);
    buf.push(0);
    put_u16(&mut buf, 0);
    put_u32(&mut buf, body.len() as u32);
    put_u32(&mut buf, frames);
    put_u64(&mut buf, first_tick);
    let checksum = fnv1a(fnv1a(FNV_OFFSET, &buf), body);
    put_u64(&mut buf, checksum);
    buf.extend_from_slice(body);
    buf
}

pub(crate) fn read_chunk_header(bytes: &[u8]) -> Result<ChunkHeader> {
    if bytes.len() < CHUNK_HEADER_BYTES || &bytes[..4] != CHUNK_MARKER {
        return Err(corrupt("a chunk marker is missing"));
    }
    if bytes[5] != 0 || bytes[6] != 0 || bytes[7] != 0 {
        return Err(corrupt("a chunk header has unknown flags"));
    }
    let u32_at =
        |i: usize| u32::from_le_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]);
    let u64_at = |i: usize| {
        let mut a = [0; 8];
        a.copy_from_slice(&bytes[i..i + 8]);
        u64::from_le_bytes(a)
    };
    Ok(ChunkHeader {
        kind: bytes[4],
        length: u32_at(8),
        frames: u32_at(12),
        first_tick: u64_at(16),
        checksum: u64_at(24),
    })
}

pub(crate) fn chunk_checksum(header: &[u8], body: &[u8]) -> u64 {
    fnv1a(fnv1a(FNV_OFFSET, &header[..24]), body)
}

/// Appends a section: id, length, payload.
pub(crate) fn put_section(body: &mut Vec<u8>, id: u64, payload: &[u8]) {
    put_uv(body, id);
    put_uv(body, payload.len() as u64);
    body.extend_from_slice(payload);
}

/// A data chunk's sections. Unknown ids are returned too and callers skip
/// them; a known id twice is damage.
pub(crate) fn read_sections(body: &[u8]) -> Result<Vec<(u64, &[u8])>> {
    let mut input = In::new(body);
    let mut out: Vec<(u64, &[u8])> = Vec::new();
    while !input.done() {
        let id = input.uv()?;
        let n = input.count(body.len(), "section bytes")?;
        let payload = input.bytes(n)?;
        if (SECTION_STRINGS..=SECTION_CHECKSUMS).contains(&id) && out.iter().any(|(i, _)| *i == id)
        {
            return Err(corrupt(format!("section {id} appears twice in one chunk")));
        }
        out.push((id, payload));
    }
    Ok(out)
}

pub(crate) fn section<'a>(sections: &[(u64, &'a [u8])], id: u64) -> Option<&'a [u8]> {
    sections.iter().find(|(i, _)| *i == id).map(|(_, p)| *p)
}

fn escape(text: &str, key: bool) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '=' if key => out.push_str("\\q"),
            c => out.push(c),
        }
    }
    out
}

fn unescape(text: &str) -> Result<String> {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        out.push(match chars.next() {
            Some('\\') => '\\',
            Some('n') => '\n',
            Some('r') => '\r',
            Some('q') => '=',
            _ => return Err(corrupt("the header has a bad escape")),
        });
    }
    Ok(out)
}

fn number(v: f64) -> String {
    let text = format!("{v}");
    if text.len() > 24 {
        format!("{v:e}")
    } else {
        text
    }
}

fn numbers(values: &[f64]) -> String {
    values
        .iter()
        .map(|v| number(*v))
        .collect::<Vec<_>>()
        .join(",")
}

fn check_text(what: &str, text: &str) -> Result<()> {
    if text.len() > MAX_STRING_BYTES {
        return Err(invalid(format!(
            "{what} is {} bytes; the limit is {MAX_STRING_BYTES}",
            text.len()
        )));
    }
    Ok(())
}

/// The header as `key=value` lines.
pub(crate) fn encode_header(header: &Header) -> Result<Vec<u8>> {
    if header.extra.len() > MAX_KEY_VALUES {
        return Err(invalid(format!(
            "the header has {} extra entries; the limit is {MAX_KEY_VALUES}",
            header.extra.len()
        )));
    }
    let w = &header.world;
    let mut lines: Vec<(String, String)> = vec![
        ("game.version".into(), header.game_version.clone()),
        ("game.commit".into(), header.game_commit.clone()),
        ("recorded_at".into(), header.recorded_at.clone()),
        ("mission".into(), header.mission.as_str().to_owned()),
        ("world.theater".into(), w.theater.clone()),
        ("world.theater_name".into(), w.theater_name.clone()),
        ("world.layout".into(), w.layout.clone()),
        ("world.weather_name".into(), w.weather_name.clone()),
        ("world.time_of_day_s".into(), number(w.time_of_day_s)),
        ("world.wind_fps".into(), numbers(&w.wind_fps)),
        ("world.clouds.module".into(), w.clouds.module.clone()),
    ];
    if let Some(weather) = w.weather {
        lines.push(("world.weather".into(), weather.to_string()));
    }
    if let Some(seed) = w.weather_seed {
        lines.push(("world.weather_seed".into(), seed.to_string()));
    }
    if let Some(deck) = w.clouds.deck_ft {
        lines.push(("world.clouds.deck_ft".into(), number(deck)));
    }
    if let Some(extent) = w.extent_ft {
        lines.push(("world.extent_ft".into(), numbers(&extent)));
    }
    for (key, value) in &header.extra {
        if key.is_empty() {
            return Err(invalid("an extra header entry has an empty key"));
        }
        lines.push((format!("x.{key}"), value.clone()));
    }
    let mut text = String::new();
    for (key, value) in &lines {
        check_text(&format!("header entry {key}"), key)?;
        check_text(&format!("header entry {key}"), value)?;
        text.push_str(&escape(key, true));
        text.push('=');
        text.push_str(&escape(value, false));
        text.push('\n');
    }
    if text.len() > MAX_HEADER_BYTES {
        return Err(invalid(format!(
            "the header is {} bytes; the limit is {MAX_HEADER_BYTES}",
            text.len()
        )));
    }
    Ok(text.into_bytes())
}

fn parse_number(key: &str, text: &str) -> Result<f64> {
    text.parse()
        .map_err(|_| corrupt(format!("header entry {key} is not a number")))
}

fn parse_numbers<const N: usize>(key: &str, text: &str) -> Result<[f64; N]> {
    let parts: Vec<&str> = text.split(',').collect();
    if parts.len() != N {
        return Err(corrupt(format!("header entry {key} needs {N} numbers")));
    }
    let mut out = [0.; N];
    for (v, part) in out.iter_mut().zip(parts) {
        *v = parse_number(key, part)?;
    }
    Ok(out)
}

pub(crate) fn decode_header(bytes: &[u8], version: u16) -> Result<Header> {
    if bytes.len() > MAX_HEADER_BYTES {
        return Err(corrupt("the header block is too large"));
    }
    let text = std::str::from_utf8(bytes).map_err(|_| corrupt("the header is not UTF-8"))?;
    let mut header = Header {
        format_version: version,
        mission: MissionKind::Other(String::new()),
        world: World {
            clouds: Clouds::default(),
            ..World::default()
        },
        ..Header::default()
    };
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or_else(|| corrupt("a header line has no '='"))?;
        let key = unescape(key)?;
        let value = unescape(value)?;
        let w = &mut header.world;
        match key.as_str() {
            "game.version" => header.game_version = value,
            "game.commit" => header.game_commit = value,
            "recorded_at" => header.recorded_at = value,
            "mission" => header.mission = MissionKind::parse(&value),
            "world.theater" => w.theater = value,
            "world.theater_name" => w.theater_name = value,
            "world.layout" => w.layout = value,
            "world.weather" => {
                w.weather = Some(
                    value
                        .parse()
                        .map_err(|_| corrupt("header entry world.weather is not a number"))?,
                )
            }
            "world.weather_name" => w.weather_name = value,
            "world.weather_seed" => {
                w.weather_seed = Some(
                    value
                        .parse()
                        .map_err(|_| corrupt("header entry world.weather_seed is not a number"))?,
                )
            }
            "world.time_of_day_s" => w.time_of_day_s = parse_number(&key, &value)?,
            "world.wind_fps" => w.wind_fps = parse_numbers(&key, &value)?,
            "world.clouds.module" => w.clouds.module = value,
            "world.clouds.deck_ft" => w.clouds.deck_ft = Some(parse_number(&key, &value)?),
            "world.extent_ft" => w.extent_ft = Some(parse_numbers(&key, &value)?),
            other => {
                if header.extra.len() >= MAX_KEY_VALUES {
                    return Err(corrupt("the header has too many entries"));
                }
                // Keys this build does not know are kept as extras, so a
                // newer file's additions are not lost.
                let key = other.strip_prefix("x.").unwrap_or(other).to_owned();
                header.extra.push((key, value));
            }
        }
    }
    Ok(header)
}

fn check_pairs(what: &str, pairs: &[(String, String)]) -> Result<()> {
    if pairs.len() > MAX_KEY_VALUES {
        return Err(invalid(format!(
            "{what} has {} entries; the limit is {MAX_KEY_VALUES}",
            pairs.len()
        )));
    }
    for (key, value) in pairs {
        check_text(what, key)?;
        check_text(what, value)?;
    }
    Ok(())
}

/// Footer body: end tick, the writer's totals, then the result entries.
pub(crate) fn encode_footer(footer: &Footer, frames: u64, chunks: u64) -> Result<Vec<u8>> {
    check_pairs("the footer result", &footer.result)?;
    let mut buf = Vec::new();
    put_uv(&mut buf, footer.end_tick);
    put_uv(&mut buf, frames);
    put_uv(&mut buf, chunks);
    put_uv(&mut buf, footer.result.len() as u64);
    for (key, value) in &footer.result {
        put_text(&mut buf, key);
        put_text(&mut buf, value);
    }
    Ok(buf)
}

/// Returns the footer and the writer's frame and data chunk totals.
pub(crate) fn decode_footer(body: &[u8]) -> Result<(Footer, u64, u64)> {
    let mut input = In::new(body);
    let end_tick = input.uv()?;
    let frames = input.uv()?;
    let chunks = input.uv()?;
    let n = input.count(MAX_KEY_VALUES, "footer entries")?;
    let mut result = Vec::with_capacity(n);
    for _ in 0..n {
        result.push((input.text(MAX_STRING_BYTES)?, input.text(MAX_STRING_BYTES)?));
    }
    if !input.done() {
        return Err(corrupt("the footer has trailing bytes"));
    }
    Ok((Footer { end_tick, result }, frames, chunks))
}

/// One seek index entry: where a data chunk starts and what it holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct IndexEntry {
    pub offset: u64,
    pub first_tick: u64,
    pub frames: u32,
}

pub(crate) fn encode_index(footer_offset: u64, entries: &[IndexEntry]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(entries.len() * 8 + 16);
    put_uv(&mut buf, footer_offset);
    put_uv(&mut buf, entries.len() as u64);
    let (mut offset, mut tick) = (0, 0);
    for e in entries {
        put_uv(&mut buf, e.offset - offset);
        put_uv(&mut buf, e.first_tick.wrapping_sub(tick));
        put_uv(&mut buf, u64::from(e.frames));
        offset = e.offset;
        tick = e.first_tick;
    }
    buf
}

pub(crate) fn decode_index(body: &[u8], max_entries: usize) -> Result<(u64, Vec<IndexEntry>)> {
    let mut input = In::new(body);
    let footer_offset = input.uv()?;
    let n = input.count(max_entries, "index entries")?;
    let mut entries = Vec::with_capacity(n);
    let (mut offset, mut tick) = (0u64, 0u64);
    for _ in 0..n {
        offset = offset
            .checked_add(input.uv()?)
            .ok_or_else(|| corrupt("an index offset overflows"))?;
        tick = tick.wrapping_add(input.uv()?);
        let frames = u32::try_from(input.uv()?)
            .map_err(|_| corrupt("an index frame count is out of range"))?;
        entries.push(IndexEntry {
            offset,
            first_tick: tick,
            frames,
        });
    }
    if !input.done() {
        return Err(corrupt("the index has trailing bytes"));
    }
    Ok((footer_offset, entries))
}

pub(crate) fn trailer(index_offset: u64) -> Vec<u8> {
    let mut buf = Vec::with_capacity(TRAILER_BYTES);
    put_u64(&mut buf, index_offset);
    buf.extend_from_slice(TRAILER_MAGIC);
    buf
}

/// The index chunk offset a trailer names, if these bytes are a trailer.
pub(crate) fn read_trailer(bytes: &[u8]) -> Option<u64> {
    if bytes.len() != TRAILER_BYTES || &bytes[8..] != TRAILER_MAGIC {
        return None;
    }
    let mut a = [0; 8];
    a.copy_from_slice(&bytes[..8]);
    Some(u64::from_le_bytes(a))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_text_round_trips_awkward_values() {
        let header = Header {
            game_version: "0.1.0".into(),
            game_commit: "066597b".into(),
            recorded_at: "2026-09-26T15:40:00Z".into(),
            mission: MissionKind::Other("campaign = soon\nmaybe".into()),
            world: World {
                theater: "UKR".into(),
                theater_name: "Ukraine".into(),
                layout: "~UKR3.MM".into(),
                weather: Some(2),
                weather_name: "cloudy".into(),
                weather_seed: Some(-12),
                time_of_day_s: 43_200.25,
                wind_fps: [3.5, 0., -1e300],
                clouds: Clouds {
                    module: "CLOUD1.LAY".into(),
                    deck_ft: Some(7000.),
                },
                extent_ft: Some([1_703_936., 1_630_208.]),
            },
            extra: vec![
                ("flight=model".into(), "researched\\hybrid".into()),
                ("probe".into(), "no weather stepping\r\n".into()),
                ("probe".into(), "second entry".into()),
            ],
            ..Header::default()
        };
        let bytes = encode_header(&header).unwrap();
        assert_eq!(decode_header(&bytes, FORMAT_VERSION).unwrap(), header);
    }
}
