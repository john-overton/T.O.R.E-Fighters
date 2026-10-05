//! A small PNG reader for the title bar pictures ([`super::header`]: the
//! lettering the game ships and the one a player can supply), so reading them
//! needs no dependency.
//!
//! It reads the first rows of a non-interlaced picture no wider than the
//! caller allows, at any bit depth and colour type PNG allows (palette and grey
//! levels included, a palette's `tRNS` alpha too), and stops inflating once it
//! has them. It checks the structure it relies on and bounds everything it
//! allocates, but not the chunk or zlib checksums (a damaged picture shows as
//! damaged pixels). *Agent decision*, 2026-10-05.
use std::cmp::min;

/// The eight bytes every PNG file starts with.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];

/// The first rows of a picture, straight RGBA.
pub struct Image {
    pub width: usize,
    pub rows: usize,
    pub rgba: Vec<u8>,
}

type Result<T> = std::result::Result<T, String>;

fn be32(bytes: &[u8]) -> usize {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize
}

/// The first `most` rows (or all of them, if fewer) of the PNG in `file`,
/// which may be at most `widest` pixels wide.
pub fn decode_rows(file: &[u8], widest: usize, most: usize) -> Result<Image> {
    if file.get(..8) != Some(&SIGNATURE) {
        return Err("it is not a PNG picture".into());
    }
    let (mut header, mut palette, mut alpha) = (None, Vec::new(), Vec::new());
    let mut compressed = Vec::new();
    let mut at = 8;
    while at + 8 <= file.len() {
        let (length, kind) = (be32(&file[at..]), &file[at + 4..at + 8]);
        let body = file
            .get(at + 8..at + 8 + length)
            .ok_or("the PNG ends inside a chunk")?;
        match kind {
            b"IHDR" => header = Some(Header::parse(body)?),
            b"PLTE" => palette = body.to_vec(),
            b"tRNS" => alpha = body.to_vec(),
            b"IDAT" => compressed.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        at += 12 + length;
    }
    let header = header.ok_or("the PNG has no header chunk")?;
    if header.width > widest {
        return Err(format!("it is {} pixels wide, over {widest}", header.width));
    }
    let (width, keep) = (header.width, header.height.min(most));
    if header.color == 3 && palette.is_empty() {
        return Err("the PNG has no palette".into());
    }
    let stride = header.stride();
    let raw = inflate_zlib(&compressed, keep * (stride + 1))?;
    if raw.len() < keep * (stride + 1) {
        return Err("the PNG's picture data ends early".into());
    }
    let lines = unfilter(&raw, keep, stride, header.pixel_bytes())?;
    let mut rgba = Vec::with_capacity(keep * width * 4);
    for line in lines.chunks_exact(stride) {
        header.rgba_row(line, &palette, &alpha, &mut rgba)?;
    }
    Ok(Image {
        width,
        rows: keep,
        rgba,
    })
}

struct Header {
    width: usize,
    height: usize,
    depth: usize,
    color: u8,
}

impl Header {
    fn parse(body: &[u8]) -> Result<Self> {
        if body.len() < 13 {
            return Err("the PNG's header chunk is short".into());
        }
        let (depth, color) = (usize::from(body[8]), body[9]);
        let allowed: &[usize] = match color {
            0 => &[1, 2, 4, 8, 16],
            3 => &[1, 2, 4, 8],
            2 | 4 | 6 => &[8, 16],
            _ => return Err(format!("the PNG has an unknown colour type {color}")),
        };
        if !allowed.contains(&depth) {
            return Err(format!("colour type {color} cannot have {depth} bits"));
        }
        if body[10] != 0 || body[11] != 0 {
            return Err("the PNG uses an unknown compression or filter method".into());
        }
        if body[12] != 0 {
            return Err("it is interlaced; save it without interlacing".into());
        }
        let (width, height) = (be32(body), be32(&body[4..]));
        if width == 0 || height == 0 || width > 16_384 || height > 16_384 {
            return Err(format!("a {width} by {height} PNG is out of range"));
        }
        Ok(Self {
            width,
            height,
            depth,
            color,
        })
    }

    fn channels(&self) -> usize {
        match self.color {
            0 | 3 => 1,
            4 => 2,
            2 => 3,
            _ => 4,
        }
    }

    /// Bytes in a row, without its filter byte.
    fn stride(&self) -> usize {
        (self.width * self.channels() * self.depth).div_ceil(8)
    }

    /// The filters' "previous pixel" distance in bytes.
    fn pixel_bytes(&self) -> usize {
        (self.channels() * self.depth).div_ceil(8)
    }

    /// One unfiltered row as RGBA pixels.
    fn rgba_row(&self, line: &[u8], palette: &[u8], alpha: &[u8], out: &mut Vec<u8>) -> Result<()> {
        // The sample at `index` across the row, as the file stores it.
        let sample = |index: usize| -> usize {
            match self.depth {
                16 => usize::from(line[index * 2]) << 8 | usize::from(line[index * 2 + 1]),
                8 => usize::from(line[index]),
                depth => {
                    let per_byte = 8 / depth;
                    let shift = 8 - depth * (index % per_byte + 1);
                    usize::from(line[index / per_byte] >> shift) & ((1 << depth) - 1)
                }
            }
        };
        // A sample scaled to 0..=255.
        let scaled = |value: usize| -> u8 {
            let top = (1usize << self.depth) - 1;
            (value * 255 / top) as u8
        };
        for x in 0..self.width {
            let pixel = match self.color {
                0 => {
                    let grey = scaled(sample(x));
                    [grey, grey, grey, 255]
                }
                2 => [
                    scaled(sample(x * 3)),
                    scaled(sample(x * 3 + 1)),
                    scaled(sample(x * 3 + 2)),
                    255,
                ],
                3 => {
                    let index = sample(x);
                    let entry = palette
                        .get(index * 3..index * 3 + 3)
                        .ok_or("the PNG uses a colour its palette lacks")?;
                    [
                        entry[0],
                        entry[1],
                        entry[2],
                        alpha.get(index).copied().unwrap_or(255),
                    ]
                }
                4 => {
                    let grey = scaled(sample(x * 2));
                    [grey, grey, grey, scaled(sample(x * 2 + 1))]
                }
                _ => [
                    scaled(sample(x * 4)),
                    scaled(sample(x * 4 + 1)),
                    scaled(sample(x * 4 + 2)),
                    scaled(sample(x * 4 + 3)),
                ],
            };
            out.extend_from_slice(&pixel);
        }
        Ok(())
    }
}

/// The rows of `raw` (each led by its filter byte) with the filters undone.
fn unfilter(raw: &[u8], rows: usize, stride: usize, bpp: usize) -> Result<Vec<u8>> {
    let mut out = vec![0u8; rows * stride];
    for row in 0..rows {
        let (before, now) = out.split_at_mut(row * stride);
        let up: &[u8] = if row == 0 {
            &[]
        } else {
            &before[(row - 1) * stride..]
        };
        let now = &mut now[..stride];
        let source = &raw[row * (stride + 1)..(row + 1) * (stride + 1)];
        now.copy_from_slice(&source[1..]);
        let above = |i: usize| up.get(i).copied().unwrap_or(0);
        for i in 0..stride {
            let left = if i >= bpp { now[i - bpp] } else { 0 };
            let corner = if i >= bpp { above(i - bpp) } else { 0 };
            let add = match source[0] {
                0 => 0,
                1 => left,
                2 => above(i),
                3 => ((u16::from(left) + u16::from(above(i))) / 2) as u8,
                4 => paeth(left, above(i), corner),
                other => return Err(format!("the PNG uses an unknown filter {other}")),
            };
            now[i] = now[i].wrapping_add(add);
        }
    }
    Ok(out)
}

fn paeth(a: u8, b: u8, c: u8) -> u8 {
    let (ia, ib, ic) = (i16::from(a), i16::from(b), i16::from(c));
    let p = ia + ib - ic;
    let (pa, pb, pc) = ((p - ia).abs(), (p - ib).abs(), (p - ic).abs());
    if pa <= pb && pa <= pc {
        a
    } else if pb <= pc {
        b
    } else {
        c
    }
}

// ---- zlib and deflate ----

/// The zlib stream in `data`, inflated until `limit` bytes are out (more may
/// come back: the block in progress finishes).
fn inflate_zlib(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    let (cmf, flg) = match data {
        [cmf, flg, ..] => (*cmf, *flg),
        _ => return Err("the PNG has no picture data".into()),
    };
    if cmf & 0x0f != 8 || (u16::from(cmf) << 8 | u16::from(flg)) % 31 != 0 || flg & 0x20 != 0 {
        return Err("the PNG's picture data is not plain zlib".into());
    }
    inflate(&data[2..], limit)
}

struct Bits<'a> {
    data: &'a [u8],
    at: usize,
    buffer: u32,
    count: u32,
}

impl Bits<'_> {
    /// The next `need` (at most 16) bits, first bit lowest.
    fn take(&mut self, need: u32) -> Result<usize> {
        while self.count < need {
            let byte = *self
                .data
                .get(self.at)
                .ok_or("the PNG's picture data ends early")?;
            self.at += 1;
            self.buffer |= u32::from(byte) << self.count;
            self.count += 8;
        }
        let value = self.buffer & ((1 << need) - 1);
        self.buffer >>= need;
        self.count -= need;
        Ok(value as usize)
    }
}

/// A canonical Huffman code: how many codes of each length, and the symbols
/// in code order.
struct Code {
    count: [u16; 16],
    symbol: Vec<u16>,
}

impl Code {
    fn new(lengths: &[u8]) -> Result<Self> {
        let mut count = [0u16; 16];
        for length in lengths {
            count[usize::from(*length)] += 1;
        }
        let mut left = 1i32;
        for of_length in &count[1..] {
            left = (left << 1) - i32::from(*of_length);
            if left < 0 {
                return Err("the PNG has an over-subscribed code".into());
            }
        }
        let mut offset = [0u16; 16];
        for length in 1..15 {
            offset[length + 1] = offset[length] + count[length];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (value, length) in lengths.iter().enumerate() {
            if *length != 0 {
                let slot = &mut offset[usize::from(*length)];
                symbol[usize::from(*slot)] = value as u16;
                *slot += 1;
            }
        }
        Ok(Self { count, symbol })
    }

    fn decode(&self, bits: &mut Bits) -> Result<usize> {
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for length in 1..16 {
            code |= bits.take(1)? as i32;
            let count = i32::from(self.count[length]);
            if code - count < first {
                return self
                    .symbol
                    .get((index + code - first) as usize)
                    .map(|symbol| usize::from(*symbol))
                    .ok_or_else(|| "the PNG has a bad code".into());
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("the PNG has a bad code".into())
    }
}

const LENGTH_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LENGTH_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
const DISTANCE_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DISTANCE_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
/// The order the code length code's own lengths come in.
const CODE_LENGTH_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn inflate(data: &[u8], limit: usize) -> Result<Vec<u8>> {
    let mut bits = Bits {
        data,
        at: 0,
        buffer: 0,
        count: 0,
    };
    let mut out = Vec::with_capacity(min(limit, 1 << 22));
    loop {
        let last = bits.take(1)? == 1;
        match bits.take(2)? {
            0 => stored(&mut bits, &mut out)?,
            1 => {
                let mut lengths = [8u8; 288];
                lengths[144..256].fill(9);
                lengths[256..280].fill(7);
                let literals = Code::new(&lengths)?;
                let distances = Code::new(&[5u8; 30])?;
                codes(&mut bits, &mut out, &literals, &distances, limit)?;
            }
            2 => {
                let (literals, distances) = dynamic(&mut bits)?;
                codes(&mut bits, &mut out, &literals, &distances, limit)?;
            }
            _ => return Err("the PNG's picture data is damaged".into()),
        }
        if last || out.len() >= limit {
            return Ok(out);
        }
    }
}

fn stored(bits: &mut Bits, out: &mut Vec<u8>) -> Result<()> {
    // The bits left of the current byte are dropped; no more than 7 are held.
    bits.buffer = 0;
    bits.count = 0;
    let head = bits
        .data
        .get(bits.at..bits.at + 4)
        .ok_or("the PNG's picture data ends early")?;
    let length = usize::from(head[0]) | usize::from(head[1]) << 8;
    let check = usize::from(head[2]) | usize::from(head[3]) << 8;
    if length != !check & 0xffff {
        return Err("the PNG's picture data is damaged".into());
    }
    bits.at += 4;
    out.extend_from_slice(
        bits.data
            .get(bits.at..bits.at + length)
            .ok_or("the PNG's picture data ends early")?,
    );
    bits.at += length;
    Ok(())
}

fn dynamic(bits: &mut Bits) -> Result<(Code, Code)> {
    let literal_count = bits.take(5)? + 257;
    let distance_count = bits.take(5)? + 1;
    let code_length_count = bits.take(4)? + 4;
    if literal_count > 286 || distance_count > 30 {
        return Err("the PNG's picture data is damaged".into());
    }
    let mut lengths = [0u8; 19];
    for index in CODE_LENGTH_ORDER.iter().take(code_length_count) {
        lengths[*index] = bits.take(3)? as u8;
    }
    let code_lengths = Code::new(&lengths)?;
    let total = literal_count + distance_count;
    let mut all = Vec::with_capacity(total);
    while all.len() < total {
        let symbol = code_lengths.decode(bits)?;
        let (value, repeat) = match symbol {
            0..=15 => (symbol as u8, 1),
            16 => {
                let previous = *all.last().ok_or("the PNG's picture data is damaged")?;
                (previous, 3 + bits.take(2)?)
            }
            17 => (0, 3 + bits.take(3)?),
            _ => (0, 11 + bits.take(7)?),
        };
        if all.len() + repeat > total {
            return Err("the PNG's picture data is damaged".into());
        }
        all.resize(all.len() + repeat, value);
    }
    if all[256] == 0 {
        return Err("the PNG's picture data is damaged".into());
    }
    Ok((
        Code::new(&all[..literal_count])?,
        Code::new(&all[literal_count..])?,
    ))
}

/// One block's symbols, until its end code or `limit` bytes are out.
fn codes(
    bits: &mut Bits,
    out: &mut Vec<u8>,
    literals: &Code,
    distances: &Code,
    limit: usize,
) -> Result<()> {
    loop {
        let symbol = literals.decode(bits)?;
        match symbol {
            0..=255 => out.push(symbol as u8),
            256 => return Ok(()),
            _ => {
                let slot = symbol - 257;
                let base = *LENGTH_BASE
                    .get(slot)
                    .ok_or("the PNG's picture data is damaged")?;
                let length = usize::from(base) + bits.take(u32::from(LENGTH_EXTRA[slot]))?;
                let slot = distances.decode(bits)?;
                let base = *DISTANCE_BASE
                    .get(slot)
                    .ok_or("the PNG's picture data is damaged")?;
                let distance = usize::from(base) + bits.take(u32::from(DISTANCE_EXTRA[slot]))?;
                if distance > out.len() {
                    return Err("the PNG's picture data is damaged".into());
                }
                for _ in 0..length {
                    out.push(out[out.len() - distance]);
                }
            }
        }
        if out.len() >= limit {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::png::{crc32, zlib_stored};

    /// Raw deflate streams (no zlib wrapper) made by Python's zlib, in the
    /// three block kinds: of `PAYLOAD` one stored and one with the fixed
    /// code, of `TEXT` one with a dynamic code.
    const PAYLOAD: &[u8] = b"abcabcabcabc-the quick brown fox-abcabcabcabc";
    const TEXT: &[u8] = b"It was the best of times, it was the worst of times, it was the age of wisdom, it was the age of foolishness, it was the epoch of belief, it was the epoch of incredulity, it was the season of Light, it was the season of Darkness, it was the spring of hope, it was the winter of despair.";
    const STORED: &str = "012d00d2ff6162636162636162636162632d74686520717569636b2062726f776e20666f782d616263616263616263616263";
    const FIXED: &str = "4b4c4a4e8421dd928c5485c2d2cce46c85a4a2fcf23c85b4fc0add44240500";
    const DYNAMIC: &str = "75cfc109c3300c85e1553440e914bd14ba84133fdba289652415d3ed8b4fc524397fbf78e8e9d4839117d002739244ce3bec46fc972e7a45216340678bb29f4912d9d84a85cd8768b296112cd818e9dcb8ae8af8d9d8bf5360082675142fcec52fec11f47dd8b5a65cf3e0220df39b5c1d3a28c25a60bdff00";

    fn hex(text: &str) -> Vec<u8> {
        (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
            .collect()
    }

    #[test]
    fn stored_fixed_and_dynamic_blocks_inflate() {
        assert_eq!(inflate(&hex(STORED), usize::MAX).unwrap(), PAYLOAD);
        assert_eq!(inflate(&hex(FIXED), usize::MAX).unwrap(), PAYLOAD);
        assert_eq!(inflate(&hex(DYNAMIC), usize::MAX).unwrap(), TEXT);
    }

    #[test]
    fn inflating_stops_once_enough_is_out() {
        let few = inflate(&hex(DYNAMIC), 40).unwrap();
        assert!(few.len() >= 40 && few.len() < TEXT.len());
        assert_eq!(few, TEXT[..few.len()]);
    }

    #[test]
    fn damaged_streams_are_refused() {
        assert!(inflate(&[0x07], 10).is_err(), "block type 3");
        assert!(inflate(&hex(&STORED[..20]), 100).is_err(), "cut short");
        // A stored block whose length and its complement disagree.
        assert!(inflate(&[0x01, 0x03, 0x00, 0x00, 0x00, 1, 2, 3], 10).is_err());
        // A copy from before the start of the output.
        assert!(inflate(&[0x4b, 0x04, 0x01], 10).is_err());
        assert!(
            inflate_zlib(&[0x78, 0x00, 0x01], 10).is_err(),
            "bad check bits"
        );
    }

    fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], body: &[u8]) {
        out.extend_from_slice(&(body.len() as u32).to_be_bytes());
        let start = out.len();
        out.extend_from_slice(kind);
        out.extend_from_slice(body);
        let crc = crc32(&out[start..]);
        out.extend_from_slice(&crc.to_be_bytes());
    }

    /// A PNG of `width` by `height` from `lines` (each led by its filter
    /// byte), in stored zlib blocks.
    fn png(
        (width, height): (u32, u32),
        (depth, color): (u8, u8),
        extra: &[(&[u8; 4], &[u8])],
        lines: &[u8],
    ) -> Vec<u8> {
        let mut out = SIGNATURE.to_vec();
        let mut head = Vec::new();
        head.extend_from_slice(&width.to_be_bytes());
        head.extend_from_slice(&height.to_be_bytes());
        head.extend_from_slice(&[depth, color, 0, 0, 0]);
        chunk(&mut out, b"IHDR", &head);
        for (kind, body) in extra {
            chunk(&mut out, kind, body);
        }
        chunk(&mut out, b"IDAT", &zlib_stored(lines));
        chunk(&mut out, b"IEND", &[]);
        out
    }

    #[test]
    fn rgba_rows_come_back_with_their_alpha() {
        let file = png(
            (2, 2),
            (8, 6),
            &[],
            &[
                0, 1, 2, 3, 255, 4, 5, 6, 128, 0, 7, 8, 9, 0, 10, 11, 12, 255,
            ],
        );
        let image = decode_rows(&file, 2, 2).unwrap();
        assert_eq!((image.width, image.rows), (2, 2));
        assert_eq!(
            image.rgba,
            [1, 2, 3, 255, 4, 5, 6, 128, 7, 8, 9, 0, 10, 11, 12, 255]
        );
        // Only the rows asked for are kept, and a narrower picture is fine.
        assert_eq!(decode_rows(&file, 2, 1).unwrap().rgba.len(), 8);
        let narrow = decode_rows(&file, 640, 77).unwrap();
        assert_eq!(
            (narrow.width, narrow.rows),
            (2, 2),
            "its own size, under the limits"
        );
    }

    #[test]
    fn every_filter_is_undone() {
        // Four RGB rows of 2 pixels, one filter type each, all from the same
        // target picture.
        let target: [[u8; 6]; 4] = [
            [10, 20, 30, 40, 50, 60],
            [15, 25, 35, 45, 55, 66],
            [12, 22, 32, 52, 62, 72],
            [200, 100, 50, 190, 90, 40],
        ];
        let mut lines = Vec::new();
        // None.
        lines.push(0);
        lines.extend_from_slice(&target[0]);
        // Sub: each byte less the one a pixel (3 bytes) before.
        lines.push(1);
        for i in 0..6 {
            lines.push(target[1][i].wrapping_sub(if i >= 3 { target[1][i - 3] } else { 0 }));
        }
        // Up.
        lines.push(2);
        for (now, above) in target[2].iter().zip(&target[1]) {
            lines.push(now.wrapping_sub(*above));
        }
        // Paeth.
        lines.push(4);
        for i in 0..6 {
            let a = if i >= 3 { target[3][i - 3] } else { 0 };
            let b = target[2][i];
            let c = if i >= 3 { target[2][i - 3] } else { 0 };
            lines.push(target[3][i].wrapping_sub(paeth(a, b, c)));
        }
        let image = decode_rows(&png((2, 4), (8, 2), &[], &lines), 2, 4).unwrap();
        for (row, line) in target.iter().enumerate() {
            let got: Vec<u8> = image.rgba[row * 8..(row + 1) * 8]
                .chunks_exact(4)
                .flat_map(|p| p[..3].to_vec())
                .collect();
            assert_eq!(&got, line, "row {row}");
        }
        // Average, with a left and an up neighbour, on its own.
        let lines = [0, 10, 20, 30, 40, 50, 60, 3, 5, 5, 5, 5, 5, 5];
        let image = decode_rows(&png((2, 2), (8, 2), &[], &lines), 2, 2).unwrap();
        // First byte: 5 + (0 + 10) / 2 = 10, then 5 + (0 + 20) / 2 = 15 and
        // 5 + (0 + 30) / 2 = 20; the second pixel's first byte is
        // 5 + (10 + 40) / 2 = 30, then 5 + (15 + 50) / 2 = 37 and
        // 5 + (20 + 60) / 2 = 45.
        assert_eq!(image.rgba[8..12], [10, 15, 20, 255]);
        assert_eq!(image.rgba[12..16], [30, 37, 45, 255]);
    }

    #[test]
    fn palettes_grey_levels_and_sixteen_bits() {
        // A 4 bit palette: indices 0, 1, 2 packed two to a byte, with the
        // second entry half transparent.
        let palette = [255, 0, 0, 0, 255, 0, 0, 0, 255];
        let file = png(
            (3, 1),
            (4, 3),
            &[(b"PLTE", &palette), (b"tRNS", &[255, 128])],
            &[0, 0x01, 0x20],
        );
        let image = decode_rows(&file, 3, 1).unwrap();
        assert_eq!(image.rgba, [255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 255]);
        // 2 bit grey: 0, 1, 2, 3 are 0, 85, 170, 255.
        let image = decode_rows(&png((4, 1), (2, 0), &[], &[0, 0b00_01_10_11]), 4, 1).unwrap();
        let grey: Vec<u8> = image.rgba.chunks_exact(4).map(|p| p[0]).collect();
        assert_eq!(grey, [0, 85, 170, 255]);
        // 16 bit grey with alpha keeps the high byte of each sample.
        let image = decode_rows(
            &png((1, 1), (16, 4), &[], &[0, 0x12, 0x34, 0xab, 0xcd]),
            1,
            1,
        )
        .unwrap();
        assert_eq!(image.rgba, [0x12, 0x12, 0x12, 0xab]);
    }

    #[test]
    fn a_picture_that_does_not_fit_says_why() {
        let rgba = png((2, 2), (8, 6), &[], &[0; 18]);
        let say = |file: &[u8], width, keep| match decode_rows(file, width, keep) {
            Ok(_) => panic!("it should be refused"),
            Err(why) => why,
        };
        assert!(say(&rgba, 1, 2).contains("2 pixels wide, over 1"));
        assert!(say(b"GIF89a....", 2, 2).contains("not a PNG"));
        let mut interlaced = rgba.clone();
        interlaced[28] = 1;
        assert!(say(&interlaced, 2, 2).contains("interlaced"));
        assert!(say(&rgba[..45], 2, 2).contains("ends inside a chunk"));
        assert!(say(&rgba[..40], 2, 2).contains("no picture data"));
        // A palette picture with no palette, and one that points past it.
        assert!(say(&png((1, 1), (8, 3), &[], &[0, 0]), 1, 1).contains("no palette"));
        let plte = [(b"PLTE", &[1u8, 2, 3][..])];
        assert!(say(&png((1, 1), (8, 3), &plte, &[0, 5]), 1, 1).contains("palette lacks"));
    }
}
