//! Electronic Arts installer container (`SETUP.ESA`) reader.
//! Layout and evidence: docs/formats/esa-installer.md. Work package A owns this file.
//! Nothing in the container is ever executed; entries are read as bounded data.
use crate::{Result, invalid, slice};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::Mutex,
};

/// The NUL-terminated ASCII signature at offset 0.
pub const MAGIC: &str = "ELECTRONIC_ARTS_ARCHIVE_FILE";
/// The supplied disc's directory is 1,183 bytes; these caps are deliberately generous.
const DIRECTORY_LIMIT: usize = 1024 * 1024;
const ENTRY_LIMIT: usize = 1024;
const NAME_LIMIT: usize = 255;

/// True when `head` starts with the container signature.
pub fn has_magic(head: &[u8]) -> bool {
    head.len() > MAGIC.len() && &head[..MAGIC.len()] == MAGIC.as_bytes() && head[MAGIC.len()] == 0
}

/// How an entry's bytes are stored: `NULL` is a byte slice, `PKWA` is PKWare DCL.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    Stored,
    Dcl,
}

#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub group: String,
    pub attributes: u32,
    pub decoded_size: u32,
    pub timestamp: u32,
    pub method: Method,
    pub packed_size: u32,
    pub offset: u64,
}
impl Entry {
    fn end(&self) -> u64 {
        self.offset + self.packed_size as u64
    }
}

struct Fields<'a> {
    data: &'a [u8],
    at: usize,
}
impl Fields<'_> {
    fn string(&mut self, what: &str) -> Result<String> {
        let rest = self
            .data
            .get(self.at..)
            .ok_or_else(|| invalid("container directory truncated"))?;
        let end = rest
            .iter()
            .position(|b| *b == 0)
            .ok_or_else(|| invalid("container directory truncated"))?;
        if end > NAME_LIMIT {
            return Err(invalid(&format!("container {what} is too long")));
        }
        let text = &rest[..end];
        if text.iter().any(|b| !(0x20..0x7f).contains(b)) {
            return Err(invalid(&format!("container {what} is not printable ASCII")));
        }
        self.at += end + 1;
        Ok(String::from_utf8(text.to_vec()).unwrap())
    }
    fn u32(&mut self) -> Result<u32> {
        let bytes =
            slice(self.data, self.at, 4).map_err(|_| invalid("container directory truncated"))?;
        self.at += 4;
        Ok(u32::from_le_bytes(bytes.try_into().unwrap()))
    }
}

pub struct Container {
    path: PathBuf,
    file: Mutex<File>,
    entries: Vec<Entry>,
}
impl Container {
    /// Read just the directory. Entry payloads are read on demand by offset.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let mut file = File::open(&path)?;
        let file_len = file.metadata()?.len();
        let cap = usize::try_from(file_len).unwrap_or(DIRECTORY_LIMIT);
        let mut head = vec![0; cap.min(DIRECTORY_LIMIT)];
        let mut read = 0;
        while read < head.len() {
            match file.read(&mut head[read..])? {
                0 => break,
                count => read += count,
            }
        }
        head.truncate(read);
        let entries = Self::parse_directory(&head, file_len)?;
        Ok(Self {
            path,
            file: Mutex::new(file),
            entries,
        })
    }
    /// Parse a directory already in memory. `file_len` bounds every entry's payload.
    pub fn parse_directory(data: &[u8], file_len: u64) -> Result<Vec<Entry>> {
        if !has_magic(data) {
            return Err(invalid("not an Electronic Arts installer container"));
        }
        let mut fields = Fields {
            data: &data[..data.len().min(DIRECTORY_LIMIT)],
            at: MAGIC.len() + 1,
        };
        let mut entries: Vec<Entry> = Vec::new();
        loop {
            // The directory ends with an entry whose name is empty.
            let name = fields.string("entry name")?;
            if name.is_empty() {
                break;
            }
            if entries.len() == ENTRY_LIMIT {
                return Err(invalid("container directory lists too many entries"));
            }
            let group = fields.string("group name")?;
            let attributes = fields.u32()?;
            let decoded_size = fields.u32()?;
            let timestamp = fields.u32()?;
            let method = match fields.string("method name")?.as_str() {
                "NULL" => Method::Stored,
                "PKWA" => Method::Dcl,
                other => {
                    return Err(invalid(&format!(
                        "unsupported container compression method {other}"
                    )));
                }
            };
            let packed_size = fields.u32()?;
            let offset = u64::from(fields.u32()?);
            let entry = Entry {
                name,
                group,
                attributes,
                decoded_size,
                timestamp,
                method,
                packed_size,
                offset,
            };
            if entry.end() > file_len {
                return Err(invalid(&format!(
                    "container entry {} lies outside the file",
                    entry.name
                )));
            }
            entries.push(entry);
        }
        // The supplied disc stores entries back to back, but trust the directory, not
        // that layout: any overlap means the directory cannot be read safely.
        let mut ranges: Vec<(u64, u64, &str)> = entries
            .iter()
            .filter(|entry| entry.packed_size > 0)
            .map(|entry| (entry.offset, entry.end(), entry.name.as_str()))
            .collect();
        ranges.sort_by_key(|range| range.0);
        for pair in ranges.windows(2) {
            if pair[1].0 < pair[0].1 {
                return Err(invalid(&format!(
                    "container entries {} and {} overlap",
                    pair[0].2, pair[1].2
                )));
            }
        }
        Ok(entries)
    }
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }
    /// Entry names are matched case-insensitively, as elsewhere in the readers.
    pub fn entry(&self, name: &str) -> Option<&Entry> {
        self.entries
            .iter()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
    }
    fn find(&self, name: &str) -> Result<&Entry> {
        self.entry(name).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("missing container entry {name}"),
            )
        })
    }
    /// Decode one entry. `limit` caps the decoded bytes, as in `Archive::read_with_limit`.
    pub fn read(&self, name: &str, limit: usize) -> Result<Vec<u8>> {
        let entry = self.find(name)?;
        let packed = usize::try_from(entry.packed_size)
            .map_err(|_| invalid("container entry too large for this host"))?;
        let decoded = usize::try_from(entry.decoded_size)
            .map_err(|_| invalid("container entry too large for this host"))?;
        let within = match entry.method {
            Method::Stored => packed <= limit,
            Method::Dcl => decoded <= limit && packed <= limit.saturating_mul(2).saturating_add(16),
        };
        if !within {
            return Err(invalid("container entry exceeds configured byte limit"));
        }
        let data = self.payload(entry.offset, packed)?;
        match entry.method {
            Method::Stored => {
                if packed != decoded {
                    return Err(invalid(&format!(
                        "stored container entry {} declares a different decoded size",
                        entry.name
                    )));
                }
                Ok(data)
            }
            Method::Dcl => {
                if data.len() < 2 {
                    return Err(invalid("truncated container DCL stream"));
                }
                if data[0] != 0 || !(4..=6).contains(&data[1]) {
                    return Err(invalid(&format!(
                        "unsupported DCL header {:02x} {:02x} in container entry {}",
                        data[0], data[1], entry.name
                    )));
                }
                // `explode` requires the output to be exactly `decoded` bytes.
                crate::dcl::explode(&data, decoded, limit)
            }
        }
    }
    /// Serve a stored EALIB entry in place, without copying it out of the container.
    pub fn archive(&self, name: &str) -> Result<crate::Archive> {
        let entry = self.find(name)?;
        if entry.method != Method::Stored {
            return Err(invalid(&format!(
                "container entry {} is compressed and cannot be read in place",
                entry.name
            )));
        }
        crate::Archive::open_at(&self.path, entry.offset, u64::from(entry.packed_size))
    }
    fn payload(&self, offset: u64, size: usize) -> Result<Vec<u8>> {
        let mut file = self
            .file
            .lock()
            .map_err(|_| io::Error::other("container file lock poisoned"))?;
        file.seek(SeekFrom::Start(offset))?;
        let mut data = vec![0; size];
        file.read_exact(&mut data)?;
        Ok(data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// Public blast format example, synthetic text, no game bytes: "AIAIAIAIAIAIA".
    const DCL_STREAM: [u8; 8] = [0, 4, 0x82, 0x24, 0x25, 0x8f, 0x80, 0x7f];
    const DCL_TEXT: &[u8] = b"AIAIAIAIAIAIA";

    struct Build {
        directory: Vec<u8>,
        payload: Vec<u8>,
    }
    impl Build {
        fn new() -> Self {
            let mut directory = MAGIC.as_bytes().to_vec();
            directory.push(0);
            Self {
                directory,
                payload: Vec::new(),
            }
        }
        /// Append an entry whose data is placed after the directory, back to back.
        fn entry(mut self, name: &str, method: &str, bytes: &[u8], decoded: u32) -> Self {
            let offset = self.payload.len();
            self.payload.extend_from_slice(bytes);
            self.directory.extend_from_slice(name.as_bytes());
            self.directory.push(0);
            self.directory.extend_from_slice(b"FA_LIBS\0");
            self.directory.extend_from_slice(&0x211_u32.to_le_bytes());
            self.directory.extend_from_slice(&decoded.to_le_bytes());
            self.directory.extend_from_slice(&0_u32.to_le_bytes());
            self.directory.extend_from_slice(method.as_bytes());
            self.directory.push(0);
            self.directory
                .extend_from_slice(&(bytes.len() as u32).to_le_bytes());
            self.directory
                .extend_from_slice(&(offset as u32).to_le_bytes());
            self
        }
        /// Close the directory with the empty-name terminator and fix up the offsets.
        fn finish(self) -> Vec<u8> {
            let Self {
                mut directory,
                payload,
            } = self;
            directory.push(0);
            let base = directory.len();
            // Rewrite each entry's offset now that the directory length is known.
            let mut data = directory;
            let mut at = MAGIC.len() + 1;
            loop {
                let end = data[at..].iter().position(|b| *b == 0).unwrap();
                if end == 0 {
                    break;
                }
                at += end + 1;
                at += data[at..].iter().position(|b| *b == 0).unwrap() + 1;
                at += 12;
                at += data[at..].iter().position(|b| *b == 0).unwrap() + 1;
                at += 4;
                let offset = u32::from_le_bytes(data[at..at + 4].try_into().unwrap());
                data[at..at + 4].copy_from_slice(&(offset + base as u32).to_le_bytes());
                at += 4;
            }
            data.extend_from_slice(&payload);
            data
        }
    }

    fn ealib(name: &str, bytes: &[u8]) -> Vec<u8> {
        let mut data = b"EALIB\x01\x00".to_vec();
        let mut raw = [0; 13];
        raw[..name.len()].copy_from_slice(name.as_bytes());
        data.extend_from_slice(&raw);
        data.push(0);
        data.extend_from_slice(&43_u32.to_le_bytes());
        data.extend_from_slice(&[0; 14]);
        data.extend_from_slice(&(43 + bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(bytes);
        data
    }

    fn temp_file(data: &[u8]) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "tore-esa-{}-{}.bin",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, data).unwrap();
        path
    }

    fn sample() -> Vec<u8> {
        Build::new()
            .entry("HELLO.TXT", "NULL", b"hello world", 11)
            .entry("STORY.TXT", "PKWA", &DCL_STREAM, DCL_TEXT.len() as u32)
            .entry("INNER.LIB", "NULL", &ealib("TEST.TXT", b"abc"), 46)
            .finish()
    }

    #[test]
    fn magic_is_nul_terminated_at_offset_zero() {
        assert!(has_magic(&sample()));
        assert!(!has_magic(MAGIC.as_bytes()));
        assert!(!has_magic(b"EALIB\x01\x00"));
        assert!(!has_magic(&[]));
    }

    #[test]
    fn directory_lists_every_entry_and_stops_at_the_terminator() {
        let data = sample();
        let entries = Container::parse_directory(&data, data.len() as u64).unwrap();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].name, "HELLO.TXT");
        assert_eq!(entries[0].group, "FA_LIBS");
        assert_eq!(entries[0].method, Method::Stored);
        assert_eq!(entries[0].decoded_size, 11);
        assert_eq!(entries[1].method, Method::Dcl);
        assert_eq!(entries[1].packed_size, DCL_STREAM.len() as u32);
    }

    #[test]
    fn malformed_directories_fail_without_panicking() {
        let data = sample();
        for length in 0..data.len().min(200) {
            let _ = Container::parse_directory(&data[..length], length as u64);
        }
        assert!(Container::parse_directory(&data, 0).is_err());
    }

    #[test]
    fn entry_beyond_the_file_is_rejected() {
        let data = sample();
        let short = data.len() as u64 - 1;
        let error = Container::parse_directory(&data, short).unwrap_err();
        assert!(error.to_string().contains("outside the file"), "{error}");
    }

    #[test]
    fn overlapping_entries_are_rejected() {
        let mut data = sample();
        let entries = Container::parse_directory(&data, data.len() as u64).unwrap();
        // Point the second entry one byte inside the first one's payload.
        let at = data
            .windows(4)
            .position(|w| w == (entries[1].offset as u32).to_le_bytes())
            .unwrap();
        data[at..at + 4].copy_from_slice(&(entries[1].offset as u32 - 1).to_le_bytes());
        let error = Container::parse_directory(&data, data.len() as u64).unwrap_err();
        assert!(error.to_string().contains("overlap"), "{error}");
    }

    #[test]
    fn unknown_method_is_named() {
        let data = Build::new().entry("A.TXT", "LZSS", b"xy", 2).finish();
        let error = Container::parse_directory(&data, data.len() as u64).unwrap_err();
        assert!(error.to_string().contains("LZSS"), "{error}");
    }

    #[test]
    fn stored_and_compressed_entries_decode() {
        let path = temp_file(&sample());
        let container = Container::open(&path).unwrap();
        assert_eq!(container.entries().len(), 3);
        assert_eq!(container.read("HELLO.TXT", 4096).unwrap(), b"hello world");
        assert_eq!(container.read("STORY.TXT", 4096).unwrap(), DCL_TEXT);
        // Lookup ignores case, and an absent name is a NotFound error.
        assert!(container.entry("hello.txt").is_some());
        assert_eq!(container.read("hello.TxT", 4096).unwrap(), b"hello world");
        assert_eq!(
            container.read("NOPE.TXT", 4096).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        assert!(container.read("HELLO.TXT", 4).is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unsupported_dcl_header_names_its_two_bytes() {
        let mut stream = DCL_STREAM;
        stream[1] = 7;
        let data = Build::new().entry("A.TXT", "PKWA", &stream, 13).finish();
        let path = temp_file(&data);
        let error = Container::open(&path)
            .unwrap()
            .read("A.TXT", 4096)
            .unwrap_err();
        assert!(error.to_string().contains("00 07"), "{error}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn decoded_size_mismatch_is_rejected() {
        let data = Build::new()
            .entry("A.TXT", "PKWA", &DCL_STREAM, 12)
            .entry("B.TXT", "NULL", b"abc", 4)
            .finish();
        let path = temp_file(&data);
        let container = Container::open(&path).unwrap();
        assert!(container.read("A.TXT", 4096).is_err());
        let error = container.read("B.TXT", 4096).unwrap_err();
        assert!(error.to_string().contains("decoded size"), "{error}");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn stored_archive_is_served_in_place() {
        let path = temp_file(&sample());
        let container = Container::open(&path).unwrap();
        let archive = container.archive("inner.lib").unwrap();
        assert_eq!(archive.entries.len(), 1);
        assert_eq!(archive.read("TEST.TXT").unwrap(), b"abc");
        let error = match container.archive("STORY.TXT") {
            Err(error) => error,
            Ok(_) => panic!("a compressed entry must not be served as an archive"),
        };
        assert!(error.to_string().contains("compressed"), "{error}");
        let _ = std::fs::remove_file(&path);
    }
}
