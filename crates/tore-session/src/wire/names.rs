//! The connection's name table: weapon records, shapes, sounds and recording
//! stems that snapshots and events name by index.
//!
//! The host adds a name the first time a record or an event for that
//! connection uses it, and sends the new entries in a Names message before the
//! packet that uses them. The message is reliable and the packet is not, so a
//! client can see an index before its name; it holds such events until the
//! name arrives (see [`super::events::EventReceiver`]).

use super::{WireError, WireResult, limits, messages::Names};
use std::collections::HashMap;
use tore_codec::{BitReader, BitWriter};

/// An index into a connection's name table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NameIndex(pub u16);

/// Bits of a name index on the wire (4,096 entries).
pub const NAME_BITS: u32 = 12;

impl NameIndex {
    pub(crate) fn write(self, w: &mut BitWriter) {
        let _ = w.write_bits(u64::from(self.0), NAME_BITS);
    }

    pub(crate) fn read(r: &mut BitReader<'_>) -> WireResult<Self> {
        Ok(Self(r.read_bits(NAME_BITS)? as u16))
    }
}

/// The host's table for one connection: names in the order they were added.
#[derive(Clone, Debug, Default)]
pub struct NameTable {
    /// The connection's flight the table serves; its Names messages carry
    /// it (protocol 3).
    pub flight: u8,
    names: Vec<String>,
    index: HashMap<String, NameIndex>,
    /// Names from here on have not been put in a Names message yet.
    unsent: usize,
}

impl NameTable {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// The index of `name`, adding it when it is new. Refuses a name past the
    /// table's 4,096 entries or longer than a wire string.
    pub fn intern(&mut self, name: &str) -> WireResult<NameIndex> {
        if let Some(&index) = self.index.get(name) {
            return Ok(index);
        }
        if self.names.len() >= limits::NAMES {
            return Err(WireError::TooMany {
                what: "names",
                limit: limits::NAMES,
            });
        }
        if name.len() > tore_codec::text::MAX_STRING_BYTES {
            return Err(WireError::Invalid("name longer than 255 bytes"));
        }
        let index = NameIndex(self.names.len() as u16);
        self.names.push(name.to_owned());
        self.index.insert(name.to_owned(), index);
        Ok(index)
    }

    /// The name at `index`.
    pub fn name(&self, index: NameIndex) -> Option<&str> {
        self.names.get(usize::from(index.0)).map(String::as_str)
    }

    /// Every name, in index order.
    pub fn names(&self) -> &[String] {
        &self.names
    }

    /// The Names message with the entries added since the last call, if any.
    /// The host queues it before sending the packet that uses them.
    pub fn take_new(&mut self) -> Option<Names> {
        if self.unsent == self.names.len() {
            return None;
        }
        let first = self.unsent as u16;
        let names = self.names[self.unsent..].to_vec();
        self.unsent = self.names.len();
        Some(Names {
            flight: self.flight,
            first,
            names,
        })
    }
}

/// The client's copy of a connection's table.
#[derive(Clone, Debug, Default)]
pub struct ReceivedNames {
    names: Vec<String>,
}

impl ReceivedNames {
    /// An empty table.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds the entries of a Names message, which reliable delivery hands
    /// over in order: its first index must be the table's length.
    pub fn apply(&mut self, message: &Names) -> WireResult<()> {
        if usize::from(message.first) != self.names.len() {
            return Err(WireError::Invalid("names out of order"));
        }
        if self.names.len() + message.names.len() > limits::NAMES {
            return Err(WireError::TooMany {
                what: "names",
                limit: limits::NAMES,
            });
        }
        self.names.extend(message.names.iter().cloned());
        Ok(())
    }

    /// The name at `index`, once its Names message has arrived.
    pub fn name(&self, index: NameIndex) -> Option<&str> {
        self.names.get(usize::from(index.0)).map(String::as_str)
    }

    /// How many names have arrived: every index below it is known.
    pub fn len(&self) -> usize {
        self.names.len()
    }

    /// True before any name has arrived.
    pub fn is_empty(&self) -> bool {
        self.names.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_added_once_and_sent_once() {
        let mut table = NameTable::new();
        assert_eq!(table.intern("AIM9M.JT").unwrap(), NameIndex(0));
        assert_eq!(table.intern("AIM120.JT").unwrap(), NameIndex(1));
        assert_eq!(table.intern("AIM9M.JT").unwrap(), NameIndex(0));
        let first = table.take_new().unwrap();
        assert_eq!(first.first, 0);
        assert_eq!(first.names, ["AIM9M.JT", "AIM120.JT"]);
        assert!(table.take_new().is_none());
        table.intern("FOX2").unwrap();
        let second = table.take_new().unwrap();
        assert_eq!((second.first, second.names.len()), (2, 1));

        let mut client = ReceivedNames::new();
        assert!(client.apply(&second).is_err());
        client.apply(&first).unwrap();
        client.apply(&second).unwrap();
        assert_eq!(client.name(NameIndex(2)), Some("FOX2"));
        assert_eq!(client.len(), 3);
    }

    #[test]
    fn the_table_is_bounded() {
        let mut table = NameTable::new();
        for index in 0..limits::NAMES {
            table.intern(&format!("N{index}")).unwrap();
        }
        assert!(table.intern("one more").is_err());
        assert!(NameTable::new().intern(&"x".repeat(256)).is_err());
    }
}
