//! A small PNG writer for replay screenshots, so saving a picture needs no
//! dependency. The image data is stored uncompressed: a zlib stream of
//! stored deflate blocks, which every PNG reader accepts. A full-HD frame is
//! about 6 MB this way. Agent choice (2026-09-26) for the replay viewer.

/// The eight bytes every PNG file starts with.
pub const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n'];
/// Largest payload of one stored deflate block.
const STORED_BLOCK: usize = 65_535;

/// CRC-32 as PNG chunks use it (polynomial 0xEDB88320, reflected).
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

/// Adler-32, the zlib stream checksum.
pub fn adler32(bytes: &[u8]) -> u32 {
    const MOD: u32 = 65_521;
    let (mut a, mut b) = (1u32, 0u32);
    // 5,552 bytes is the longest run that cannot overflow before reducing.
    for block in bytes.chunks(5_552) {
        for byte in block {
            a += u32::from(*byte);
            b += a;
        }
        a %= MOD;
        b %= MOD;
    }
    (b << 16) | a
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    let length = u32::try_from(data.len()).expect("PNG chunk under 2 GiB");
    out.extend_from_slice(&length.to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// A zlib stream holding `data` in stored (uncompressed) deflate blocks.
pub fn zlib_stored(data: &[u8]) -> Vec<u8> {
    let blocks = data.len().div_ceil(STORED_BLOCK).max(1);
    let mut out = Vec::with_capacity(data.len() + blocks * 5 + 6);
    // CMF: deflate with a 32 KiB window; FLG: no dictionary, fastest level,
    // and the check bits that make the pair a multiple of 31.
    out.extend_from_slice(&[0x78, 0x01]);
    let mut pieces = data.chunks(STORED_BLOCK).peekable();
    if pieces.peek().is_none() {
        out.extend_from_slice(&[1, 0, 0, 0xff, 0xff]);
    }
    while let Some(piece) = pieces.next() {
        out.push(u8::from(pieces.peek().is_none()));
        let length = piece.len() as u16;
        out.extend_from_slice(&length.to_le_bytes());
        out.extend_from_slice(&(!length).to_le_bytes());
        out.extend_from_slice(piece);
    }
    out.extend_from_slice(&adler32(data).to_be_bytes());
    out
}

/// A PNG file for an 8-bit RGBA picture, `width` by `height`, rows top
/// first. The alpha channel is dropped: screenshots are opaque.
pub fn encode_rgba(width: u32, height: u32, rgba: &[u8]) -> Result<Vec<u8>, String> {
    let (w, h) = (width as usize, height as usize);
    if width == 0 || height == 0 || rgba.len() != w * h * 4 {
        return Err(format!(
            "a {width}x{height} picture needs {} bytes, not {}",
            w * h * 4,
            rgba.len()
        ));
    }
    // Every row starts with filter type 0: the bytes as they are.
    let mut raw = Vec::with_capacity(h * (w * 3 + 1));
    for row in rgba.chunks_exact(w * 4) {
        raw.push(0);
        for pixel in row.chunks_exact(4) {
            raw.extend_from_slice(&pixel[..3]);
        }
    }
    let mut header = Vec::with_capacity(13);
    header.extend_from_slice(&width.to_be_bytes());
    header.extend_from_slice(&height.to_be_bytes());
    // Bit depth 8, colour type 2 (RGB), deflate, adaptive filtering, no
    // interlace.
    header.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut out = Vec::with_capacity(raw.len() + raw.len() / STORED_BLOCK * 5 + 64);
    out.extend_from_slice(&SIGNATURE);
    chunk(&mut out, b"IHDR", &header);
    chunk(&mut out, b"IDAT", &zlib_stored(&raw));
    chunk(&mut out, b"IEND", &[]);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksums_match_their_published_check_values() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"IEND"), 0xAE42_6082);
        assert_eq!(adler32(b"Wikipedia"), 0x11E6_0398);
        assert_eq!(adler32(b""), 1);
        // Long input exercises the deferred reduction.
        let long = vec![0xffu8; 100_000];
        let (mut a, mut b) = (1u64, 0u64);
        for byte in &long {
            a = (a + u64::from(*byte)) % 65_521;
            b = (b + a) % 65_521;
        }
        assert_eq!(u64::from(adler32(&long)), (b << 16) | a);
    }

    /// A reader for exactly what [`encode_rgba`] writes: verifies every
    /// chunk's CRC, the zlib header and Adler-32, and undoes stored blocks
    /// and filter 0. Returns width, height and RGB bytes.
    fn decode(file: &[u8]) -> (u32, u32, Vec<u8>) {
        assert_eq!(file[..8], SIGNATURE);
        let mut at = 8;
        let (mut width, mut height) = (0, 0);
        let mut idat = Vec::new();
        let mut kinds = Vec::new();
        while at < file.len() {
            let length = u32::from_be_bytes(file[at..at + 4].try_into().unwrap()) as usize;
            let kind = &file[at + 4..at + 8];
            let data = &file[at + 8..at + 8 + length];
            let crc =
                u32::from_be_bytes(file[at + 8 + length..at + 12 + length].try_into().unwrap());
            assert_eq!(crc32(&file[at + 4..at + 8 + length]), crc, "chunk CRC");
            kinds.push(String::from_utf8(kind.to_vec()).unwrap());
            match kind {
                b"IHDR" => {
                    width = u32::from_be_bytes(data[0..4].try_into().unwrap());
                    height = u32::from_be_bytes(data[4..8].try_into().unwrap());
                    assert_eq!(&data[8..], &[8, 2, 0, 0, 0]);
                }
                b"IDAT" => idat.extend_from_slice(data),
                b"IEND" => assert!(data.is_empty()),
                _ => panic!("unexpected chunk"),
            }
            at += 12 + length;
        }
        assert_eq!(kinds, ["IHDR", "IDAT", "IEND"]);
        assert_eq!((u16::from(idat[0]) << 8 | u16::from(idat[1])) % 31, 0);
        assert_eq!(idat[0] & 0x0f, 8, "deflate");
        let mut raw = Vec::new();
        let mut at = 2;
        loop {
            let last = idat[at] & 1 == 1;
            assert_eq!(idat[at] >> 1 & 3, 0, "stored block");
            let length = u16::from_le_bytes([idat[at + 1], idat[at + 2]]);
            let check = u16::from_le_bytes([idat[at + 3], idat[at + 4]]);
            assert_eq!(length, !check);
            raw.extend_from_slice(&idat[at + 5..at + 5 + usize::from(length)]);
            at += 5 + usize::from(length);
            if last {
                break;
            }
        }
        let adler = u32::from_be_bytes(idat[at..at + 4].try_into().unwrap());
        assert_eq!(adler32(&raw), adler, "zlib Adler-32");
        assert_eq!(at + 4, idat.len());
        let stride = width as usize * 3 + 1;
        assert_eq!(raw.len(), stride * height as usize);
        let mut rgb = Vec::new();
        for row in raw.chunks_exact(stride) {
            assert_eq!(row[0], 0, "filter 0");
            rgb.extend_from_slice(&row[1..]);
        }
        (width, height, rgb)
    }

    #[test]
    fn pictures_round_trip_through_a_stored_block_reader() {
        // 200 x 120 spans several stored blocks; 1 x 1 fits in one.
        for (width, height) in [(1u32, 1u32), (3, 2), (200, 120)] {
            let rgba: Vec<u8> = (0..width * height * 4)
                .map(|i| (i * 7 % 256) as u8)
                .collect();
            let file = encode_rgba(width, height, &rgba).unwrap();
            let (w, h, rgb) = decode(&file);
            assert_eq!((w, h), (width, height));
            let expected: Vec<u8> = rgba
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            assert_eq!(rgb, expected);
        }
        assert!(encode_rgba(2, 2, &[0; 15]).is_err());
        assert!(encode_rgba(0, 2, &[]).is_err());
    }

    #[test]
    fn stored_blocks_split_at_their_limit_and_mark_the_last() {
        let data = vec![7u8; STORED_BLOCK + 10];
        let stream = zlib_stored(&data);
        assert_eq!(stream[2], 0, "first block is not final");
        assert_eq!(stream[3..5], (STORED_BLOCK as u16).to_le_bytes());
        let second = 2 + 5 + STORED_BLOCK;
        assert_eq!(stream[second], 1, "second block is final");
        assert_eq!(stream[second + 1..second + 3], 10u16.to_le_bytes());
        assert_eq!(stream.len(), 2 + 5 * 2 + data.len() + 4);
        // An empty stream is one empty final block.
        assert_eq!(
            zlib_stored(&[]),
            [0x78, 0x01, 1, 0, 0, 0xff, 0xff, 0, 0, 0, 1]
        );
    }
}
