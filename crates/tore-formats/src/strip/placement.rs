//! Isolated, deliberately restricted mission object records; not a mission loader.
use crate::{Result, invalid};

/// Parsed inputs to the reviewed STRIP creation path, not an initialized object.
/// Native conversion contracts: docs/formats/native-strip.md.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placement {
    /// Whole-foot source coordinates; Y=0 still requires the initial ground query.
    pub position: [i32; 3],
    pub angles: [i32; 3],
    /// Low source byte, BEFORE the map-dependent native nationality conversion.
    pub nationality: u8,
    /// Source bits, BEFORE T_AddObj's flag mask.
    pub flags: u32,
    pub speed: i32,
    /// Post-creation alias word; never an allocated object ID.
    pub alias: u16,
    /// Entire delimited name, preserving non-UTF-8 bytes and truncated suffixes.
    pub name: Vec<u8>,
    /// Exact isolated input for provenance, including original numeric spelling.
    pub source: Vec<u8>,
}

impl Placement {
    /// Host subset: one `obj` / `.` block, all eight reviewed fields exactly once.
    /// At most 4096 bytes, LF/CRLF lines, no comments or unknown optional fields.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() > 4096 || bytes.contains(&0) {
            return Err(invalid("STRIP placement exceeds bounds or contains NUL"));
        }
        let mut lines = bytes
            .split(|b| *b == b'\n')
            .map(|line| trim(line.strip_suffix(b"\r").unwrap_or(line)))
            .filter(|line| !line.is_empty());
        if lines.next() != Some(b"obj".as_slice()) {
            return Err(invalid("expected isolated STRIP obj record"));
        }
        let mut fields: [Option<&[u8]>; 8] = [None; 8];
        let keys: [&[u8]; 8] = [
            b"type",
            b"pos",
            b"angle",
            b"nationality",
            b"flags",
            b"speed",
            b"name",
            b"alias",
        ];
        let mut ended = false;
        for line in lines.by_ref() {
            if line == b"." {
                ended = true;
                break;
            }
            let split = line
                .iter()
                .position(|b| matches!(b, b' ' | b'\t'))
                .ok_or_else(|| invalid("missing STRIP field value"))?;
            let key = &line[..split];
            let index = keys
                .iter()
                .position(|candidate| *candidate == key)
                .ok_or_else(|| invalid("unsupported STRIP placement field"))?;
            if fields[index].replace(trim(&line[split..])).is_some() {
                return Err(invalid("duplicate STRIP placement field"));
            }
        }
        if !ended || lines.next().is_some() || fields.iter().any(Option::is_none) {
            return Err(invalid("incomplete or trailing STRIP placement record"));
        }
        let fields = fields.map(Option::unwrap);
        if !fields[0].eq_ignore_ascii_case(b"STRIP.OT") {
            return Err(invalid("unsupported STRIP placement type"));
        }
        let name = fields[6]
            .strip_prefix(&[1])
            .and_then(|v| v.strip_suffix(&[1]))
            .ok_or_else(|| invalid("STRIP name requires byte-1 delimiters"))?;
        if name.iter().any(|b| *b < 32) {
            return Err(invalid("unsupported control byte in STRIP name"));
        }
        Ok(Self {
            position: vector(fields[1])?,
            angles: vector(fields[2])?,
            nationality: integer(fields[3])? as u8,
            flags: integer(fields[4])? as u32,
            speed: integer(fields[5])?,
            alias: integer(fields[7])? as u16,
            name: name.to_vec(),
            source: bytes.to_vec(),
        })
    }

    pub fn position_fixed8(&self) -> [i32; 3] {
        self.position.map(|value| value.wrapping_shl(8))
    }

    pub fn angles_pa(&self) -> [u16; 3] {
        self.angles.map(|value| (value as u16).wrapping_mul(182))
    }

    pub fn speed_fixed8(&self) -> i32 {
        self.speed.wrapping_shl(8)
    }

    /// Payload before the native trailing NUL; original bytes remain in `name`.
    pub fn native_name(&self) -> &[u8] {
        &self.name[..self.name.len().min(40)]
    }
}

fn trim(bytes: &[u8]) -> &[u8] {
    let start = bytes
        .iter()
        .position(|b| !matches!(b, b' ' | b'\t'))
        .unwrap_or(bytes.len());
    let end = bytes
        .iter()
        .rposition(|b| !matches!(b, b' ' | b'\t'))
        .map_or(start, |i| i + 1);
    &bytes[start..end]
}

fn integer(bytes: &[u8]) -> Result<i32> {
    let error = || invalid("unsupported STRIP integer (signed decimal or $hex required)");
    let text = std::str::from_utf8(bytes).map_err(|_| error())?;
    if let Some(hex) = text.strip_prefix('$') {
        if hex.is_empty() || hex.len() > 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(error());
        }
        return u32::from_str_radix(hex, 16)
            .map(|value| value as i32)
            .map_err(|_| error());
    }
    let digits = text.strip_prefix('-').unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(error());
    }
    text.parse().map_err(|_| error())
}

fn vector(bytes: &[u8]) -> Result<[i32; 3]> {
    let mut words = bytes
        .split(|b| matches!(b, b' ' | b'\t'))
        .filter(|v| !v.is_empty());
    let mut result = [0; 3];
    for value in &mut result {
        *value = integer(words.next().ok_or_else(|| invalid("short STRIP vector"))?)?;
    }
    if words.next().is_some() {
        return Err(invalid("long STRIP vector"));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    const RECORD: &str = "obj\n type strip.ot\n pos -1 0 8388608\n angle -1 360 65537\n nationality 393\n flags $80004003\n speed -8388609\n name \x01Synthetic runway\x01\n alias -101\n .\n";

    #[test]
    fn conversions_preserve_native_widths_and_source() {
        let p = Placement::parse(RECORD.as_bytes()).unwrap();
        assert_eq!(p.position_fixed8(), [-256, 0, i32::MIN]);
        assert_eq!(p.angles_pa(), [65354, 65520, 182]);
        assert_eq!(p.nationality, 137); // Still requires the separate map conversion.
        assert_eq!(p.flags, 0x80004003);
        assert_eq!(p.speed_fixed8(), 2147483392);
        assert_eq!(p.alias, 65435);
        assert_eq!(p.native_name(), b"Synthetic runway");
        assert_eq!(p.source, RECORD.as_bytes());
        assert_eq!(
            Placement::parse(RECORD.replace('\n', "\r\n").as_bytes())
                .unwrap()
                .position,
            p.position
        );
    }

    #[test]
    fn names_preserve_bytes_and_native_truncation() {
        for length in [0, 39, 40, 41, 100] {
            let mut bytes = RECORD.as_bytes().to_vec();
            let start = bytes.iter().position(|b| *b == 1).unwrap() + 1;
            let end = start + b"Synthetic runway".len();
            bytes.splice(start..end, vec![0xff; length]);
            let p = Placement::parse(&bytes).unwrap();
            assert_eq!(p.name, vec![0xff; length]);
            assert_eq!(p.native_name(), vec![0xff; length.min(40)]);
        }
    }

    #[test]
    fn unsupported_or_ambiguous_records_fail() {
        for (from, to) in [
            ("obj", "OBJ"),
            ("strip.ot", "../STRIP.OT"),
            ("strip.ot", "$STRIP.OT"),
            ("strip.ot", "OTHER.OT"),
            ("speed -8388609", "controller 0"),
            ("speed -8388609", "speed 0\n speed 1"),
            ("speed -8388609\n", ""),
            ("pos -1 0 8388608", "pos 1 2"),
            ("angle -1 360 65537", "angle 1 2 3 4"),
            ("\x01Synthetic runway\x01", "Synthetic runway"),
            ("Synthetic runway", "a\x01b"),
            ("Synthetic runway", "a\0b"),
            (" .\n", ""),
            (" .\n", " .\nobj\n"),
            ("flags $80004003", "flags $100000000"),
            ("nationality 393", "nationality 2147483648"),
            ("nationality 393", "nationality -2147483649"),
            ("nationality 393", "nationality +1"),
            ("nationality 393", "nationality 1.5"),
        ] {
            assert!(
                Placement::parse(RECORD.replace(from, to).as_bytes()).is_err(),
                "{to:?}"
            );
        }
        assert!(Placement::parse(&vec![b' '; 4097]).is_err());
    }

    #[test]
    fn integer_boundaries_are_explicit() {
        for (text, expected) in [
            ("-2147483648", i32::MIN),
            ("2147483647", i32::MAX),
            ("$ffffffff", -1),
            ("$80000000", i32::MIN),
            ("$0", 0),
        ] {
            assert_eq!(integer(text.as_bytes()).unwrap(), expected);
        }
        for text in ["", "$", "$-1", "$+1", "0x10", "1 2", "--1"] {
            assert!(integer(text.as_bytes()).is_err());
        }
    }
}
