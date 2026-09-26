//! The global string table. It only grows: each string is defined in the
//! chunk where it is first used and referenced by number afterwards. A
//! reference is a varint: 0 is the empty string, 1 is an inline string that
//! follows, and `n + 2` is table entry `n`. Once the table holds
//! [`MAX_STRINGS`] entries, new strings are written inline.

use crate::codec::{In, put_text, put_uv};
use crate::error::{Result, corrupt};
use crate::limits::{MAX_STRING_BYTES, MAX_STRINGS};
use std::collections::HashMap;

/// Writer side.
#[derive(Default)]
pub(crate) struct Interner {
    ids: HashMap<String, u32>,
    next: u32,
    /// Strings first used since the last chunk was written.
    pending: Vec<String>,
    pending_first: u32,
    pending_bytes: usize,
}

impl Interner {
    pub fn put(&mut self, buf: &mut Vec<u8>, text: &str) {
        if text.is_empty() {
            buf.push(0);
            return;
        }
        if let Some(&id) = self.ids.get(text) {
            put_uv(buf, u64::from(id) + 2);
            return;
        }
        if (self.next as usize) < MAX_STRINGS {
            let id = self.next;
            self.next += 1;
            self.ids.insert(text.to_owned(), id);
            if self.pending.is_empty() {
                self.pending_first = id;
            }
            self.pending_bytes += text.len() + 3;
            self.pending.push(text.to_owned());
            put_uv(buf, u64::from(id) + 2);
        } else {
            buf.push(1);
            put_text(buf, text);
        }
    }

    pub fn pending_bytes(&self) -> usize {
        self.pending_bytes
    }

    /// The strings section for the chunk being written, if any string is new.
    pub fn take_section(&mut self) -> Option<Vec<u8>> {
        if self.pending.is_empty() {
            return None;
        }
        let mut buf = Vec::with_capacity(self.pending_bytes + 8);
        put_uv(&mut buf, u64::from(self.pending_first));
        put_uv(&mut buf, self.pending.len() as u64);
        for text in self.pending.drain(..) {
            put_text(&mut buf, &text);
        }
        self.pending_bytes = 0;
        Some(buf)
    }
}

/// Reader side. Entries lost with a damaged chunk read as a placeholder.
#[derive(Default)]
pub(crate) struct StringTable {
    items: Vec<Option<String>>,
}

impl StringTable {
    /// Adds a chunk's strings section.
    pub fn define(&mut self, section: &[u8]) -> Result<()> {
        let mut input = In::new(section);
        let first = input.count(MAX_STRINGS, "as the first string number")?;
        let count = input.count(MAX_STRINGS - first, "new strings")?;
        let mut defined = Vec::with_capacity(count);
        for _ in 0..count {
            defined.push(input.text(MAX_STRING_BYTES)?);
        }
        if !input.done() {
            return Err(corrupt("the strings section has trailing bytes"));
        }
        if self.items.len() < first + count {
            self.items.resize(first + count, None);
        }
        for (i, text) in defined.into_iter().enumerate() {
            let slot = &mut self.items[first + i];
            if slot.as_ref().is_some_and(|old| *old != text) {
                return Err(corrupt(format!("string {} is defined twice", first + i)));
            }
            *slot = Some(text);
        }
        Ok(())
    }

    pub fn read(&self, input: &mut In) -> Result<String> {
        match input.uv()? {
            0 => Ok(String::new()),
            1 => input.text(MAX_STRING_BYTES),
            n => {
                let id = n - 2;
                Ok(usize::try_from(id)
                    .ok()
                    .and_then(|i| self.items.get(i))
                    .and_then(Option::as_ref)
                    .cloned()
                    .unwrap_or_else(|| format!("<missing string {id}>")))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strings_are_defined_once_and_referenced_after() {
        let mut interner = Interner::default();
        let mut refs = vec![];
        for text in ["alpha", "", "beta", "alpha"] {
            interner.put(&mut refs, text);
        }
        let section = interner.take_section().unwrap();
        assert!(interner.take_section().is_none());
        let mut table = StringTable::default();
        table.define(&section).unwrap();
        let mut input = In::new(&refs);
        let read: Vec<_> = (0..4).map(|_| table.read(&mut input).unwrap()).collect();
        assert_eq!(read, ["alpha", "", "beta", "alpha"]);
        // Redefining an entry with different text is damage.
        let mut again = vec![];
        put_uv(&mut again, 0);
        put_uv(&mut again, 1);
        put_text(&mut again, "gamma");
        assert!(table.define(&again).is_err());
        // A reference to an entry that was never seen reads as a placeholder.
        let mut refs = vec![];
        put_uv(&mut refs, 99);
        assert_eq!(
            table.read(&mut In::new(&refs)).unwrap(),
            "<missing string 97>"
        );
    }
}
