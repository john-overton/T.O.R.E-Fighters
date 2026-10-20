//! A short row of whole numbers kept inline, for the entity groups and the
//! readout's list entries the host codes every snapshot for every seat
//! (optimization round 2): building, cloning and dropping one costs no
//! allocation while it holds at most [`INLINE`] numbers. A longer row moves
//! to the heap, so any length works. It compares, prints and indexes as the
//! slice it holds.

use std::fmt;
use std::ops::{Deref, DerefMut};

/// The most numbers a row keeps inline: the widest entity group (an
/// aircraft's devices, 22) and the widest readout entry fit.
pub(crate) const INLINE: usize = 24;

/// A row of whole numbers.
#[derive(Clone)]
pub(crate) enum Row {
    Inline { len: u8, values: [i64; INLINE] },
    Heap(Vec<i64>),
}

impl Row {
    /// An empty row.
    pub(crate) const fn new() -> Self {
        Self::Inline {
            len: 0,
            values: [0; INLINE],
        }
    }

    /// `len` zeros.
    pub(crate) fn zeros(len: usize) -> Self {
        if len <= INLINE {
            Self::Inline {
                len: len as u8,
                values: [0; INLINE],
            }
        } else {
            Self::Heap(vec![0; len])
        }
    }

    /// Adds `value` at the end.
    pub(crate) fn push(&mut self, value: i64) {
        match self {
            Self::Inline { len, values } if usize::from(*len) < INLINE => {
                values[usize::from(*len)] = value;
                *len += 1;
            }
            Self::Inline { len, values } => {
                let mut heap = values[..usize::from(*len)].to_vec();
                heap.push(value);
                *self = Self::Heap(heap);
            }
            Self::Heap(heap) => heap.push(value),
        }
    }
}

impl Default for Row {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for Row {
    type Target = [i64];

    fn deref(&self) -> &[i64] {
        match self {
            Self::Inline { len, values } => &values[..usize::from(*len)],
            Self::Heap(heap) => heap,
        }
    }
}

impl DerefMut for Row {
    fn deref_mut(&mut self) -> &mut [i64] {
        match self {
            Self::Inline { len, values } => &mut values[..usize::from(*len)],
            Self::Heap(heap) => heap,
        }
    }
}

impl PartialEq for Row {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for Row {}

impl PartialEq<Vec<i64>> for Row {
    fn eq(&self, other: &Vec<i64>) -> bool {
        **self == other[..]
    }
}

impl PartialEq<[i64]> for Row {
    fn eq(&self, other: &[i64]) -> bool {
        **self == *other
    }
}

impl fmt::Debug for Row {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl From<&[i64]> for Row {
    fn from(values: &[i64]) -> Self {
        if values.len() <= INLINE {
            let mut inline = [0; INLINE];
            inline[..values.len()].copy_from_slice(values);
            Self::Inline {
                len: values.len() as u8,
                values: inline,
            }
        } else {
            Self::Heap(values.to_vec())
        }
    }
}

impl From<Vec<i64>> for Row {
    fn from(values: Vec<i64>) -> Self {
        if values.len() <= INLINE {
            Self::from(&values[..])
        } else {
            Self::Heap(values)
        }
    }
}

impl<const N: usize> From<[i64; N]> for Row {
    fn from(values: [i64; N]) -> Self {
        Self::from(&values[..])
    }
}

impl Extend<i64> for Row {
    fn extend<I: IntoIterator<Item = i64>>(&mut self, iter: I) {
        for value in iter {
            self.push(value);
        }
    }
}

impl FromIterator<i64> for Row {
    fn from_iter<I: IntoIterator<Item = i64>>(iter: I) -> Self {
        let mut row = Self::new();
        row.extend(iter);
        row
    }
}

impl<'a> IntoIterator for &'a Row {
    type Item = &'a i64;
    type IntoIter = std::slice::Iter<'a, i64>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_row_holds_what_a_vector_holds_inline_or_not() {
        for len in [0, 1, 5, INLINE - 1, INLINE, INLINE + 1, 3 * INLINE] {
            let values: Vec<i64> = (0..len as i64).map(|v| v * 7 - 3).collect();
            let pushed: Row = values.iter().copied().collect();
            let from = Row::from(values.clone());
            assert_eq!(&*pushed, &values[..]);
            assert_eq!(pushed, from);
            assert_eq!(pushed, values);
            assert_eq!(format!("{pushed:?}"), format!("{values:?}"));
            assert_eq!(matches!(pushed, Row::Inline { .. }), len <= INLINE);
            let zeros = Row::zeros(len);
            assert!(zeros.iter().all(|v| *v == 0) && zeros.len() == len);
            let mut changed = from.clone();
            if len > 0 {
                changed[len - 1] += 1;
                assert_ne!(changed, from);
            }
        }
    }
}
