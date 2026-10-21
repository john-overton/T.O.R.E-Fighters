//! Row bookkeeping for the CPU-drawn canvases that are uploaded to the GPU
//! every frame. A row set says which rows may have changed; comparing those
//! rows with a copy of what the GPU already holds says which rows really
//! did, so only those are sent.
use std::ops::Range;

/// A set of canvas rows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowSet {
    rows: Vec<bool>,
}

impl RowSet {
    /// The number of rows the set covers.
    pub fn len(&self) -> usize {
        self.rows.len()
    }
    /// An empty set covering `height` rows.
    pub fn reset(&mut self, height: usize) {
        self.rows.clear();
        self.rows.resize(height, false);
    }
    /// A full set covering `height` rows.
    pub fn reset_all(&mut self, height: usize) {
        self.rows.clear();
        self.rows.resize(height, true);
    }
    /// Adds rows `from..to`, clamped to the rows covered.
    pub fn mark(&mut self, from: usize, to: usize) {
        let to = to.min(self.rows.len());
        if from < to {
            self.rows[from..to].fill(true);
        }
    }
    pub fn mark_all(&mut self) {
        self.rows.fill(true);
    }
    pub fn clear(&mut self) {
        self.rows.fill(false);
    }
    #[cfg(test)]
    pub fn contains(&self, row: usize) -> bool {
        self.rows.get(row).copied().unwrap_or(false)
    }
    /// Adds every row of `other` (which covers the same rows).
    pub fn merge(&mut self, other: &RowSet) {
        if self.rows.len() != other.rows.len() {
            self.reset_all(other.rows.len());
            return;
        }
        for (row, &marked) in self.rows.iter_mut().zip(&other.rows) {
            *row |= marked;
        }
    }
    /// The runs of consecutive rows in the set, top to bottom.
    pub fn runs(&self) -> Vec<Range<usize>> {
        let mut runs = Vec::new();
        let mut start = None;
        for (row, &marked) in self.rows.iter().enumerate() {
            match (marked, start) {
                (true, None) => start = Some(row),
                (false, Some(first)) => {
                    runs.push(first..row);
                    start = None;
                }
                _ => {}
            }
        }
        if let Some(first) = start {
            runs.push(first..self.rows.len());
        }
        runs
    }
}

/// How many unchanged rows may sit between two changed bands before they
/// become two uploads: a few extra rows cost less than another call.
pub const GAP: usize = 8;

/// The bands of rows among `candidates` where `current` differs from
/// `held` (the same canvas as the GPU last got it), with bands closer than
/// `gap` rows joined. `held` is brought up to date for every returned band,
/// so after uploading exactly those bands it matches what the GPU holds.
/// Both buffers hold rows of `stride` bytes.
pub fn changed_bands(
    current: &[u8],
    held: &mut [u8],
    stride: usize,
    candidates: &RowSet,
    gap: usize,
) -> Vec<Range<usize>> {
    debug_assert_eq!(current.len(), held.len());
    let height = if stride == 0 {
        0
    } else {
        current.len() / stride
    };
    let mut bands: Vec<Range<usize>> = Vec::new();
    for run in candidates.runs() {
        for row in run.start..run.end.min(height) {
            let at = row * stride..(row + 1) * stride;
            if current[at.clone()] == held[at] {
                continue;
            }
            match bands.last_mut() {
                Some(band) if row <= band.end + gap => band.end = row + 1,
                _ => bands.push(row..row + 1),
            }
        }
    }
    for band in &bands {
        let at = band.start * stride..band.end * stride;
        held[at.clone()].copy_from_slice(&current[at]);
    }
    bands
}

/// The rows of a row-major `width` by `height` RGBA buffer that hold any
/// pixel that is not all zero.
#[cfg(test)]
pub fn lit_rows(pixels: &[u8], width: usize) -> RowSet {
    let mut rows = RowSet::default();
    rows.reset(pixels.len() / (width * 4));
    for (row, line) in pixels.chunks_exact(width * 4).enumerate() {
        if line.iter().any(|&byte| byte != 0) {
            rows.mark(row, row + 1);
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(height: usize, rows: &[usize]) -> RowSet {
        let mut set = RowSet::default();
        set.reset(height);
        for &row in rows {
            set.mark(row, row + 1);
        }
        set
    }

    #[test]
    fn runs_join_consecutive_rows() {
        assert_eq!(set(10, &[]).runs(), Vec::<Range<usize>>::new());
        assert_eq!(set(10, &[0, 1, 2, 5, 9]).runs(), vec![0..3, 5..6, 9..10]);
        let mut all = RowSet::default();
        all.reset_all(4);
        assert_eq!(all.runs(), vec![0..4]);
        // Marks past the end are ignored.
        all.reset(3);
        all.mark(2, 99);
        assert_eq!(all.runs(), vec![2..3]);
    }

    #[test]
    fn merging_sets_of_another_height_marks_everything() {
        let mut a = set(4, &[1]);
        a.merge(&set(6, &[]));
        assert_eq!(a.runs(), vec![0..6]);
        let mut b = set(4, &[1]);
        b.merge(&set(4, &[3]));
        assert_eq!(b.runs(), vec![1..2, 3..4]);
    }

    /// Whatever the pattern of changes, uploading the returned bands into a
    /// copy of the old picture gives the new picture, provided every changed
    /// row is among the candidates.
    #[test]
    fn uploading_the_changed_bands_reproduces_the_picture() {
        let (width, height) = (5usize, 40usize);
        let stride = width * 4;
        let mut seed = 12345u32;
        let mut next = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 16) as usize
        };
        for gap in [0, 1, 3, GAP, 100] {
            let mut gpu = vec![0u8; stride * height];
            let mut held = gpu.clone();
            for _frame in 0..60 {
                let mut current = gpu.clone();
                let mut candidates = RowSet::default();
                candidates.reset(height);
                for _ in 0..next() % 6 {
                    let row = next() % height;
                    let len = next() % 4;
                    candidates.mark(row, row + len);
                    // Some candidates change, some are rewritten the same.
                    for r in row..(row + len).min(height) {
                        if next() % 2 == 0 {
                            current[r * stride + next() % stride] = (next() % 256) as u8;
                        }
                    }
                }
                let bands = changed_bands(&current, &mut held, stride, &candidates, gap);
                for band in &bands {
                    gpu[band.start * stride..band.end * stride]
                        .copy_from_slice(&current[band.start * stride..band.end * stride]);
                }
                assert_eq!(gpu, current, "gap {gap}");
                assert_eq!(held, current, "gap {gap}");
                // Bands never overlap and stay in order.
                for pair in bands.windows(2) {
                    assert!(pair[0].end <= pair[1].start);
                }
            }
        }
    }

    #[test]
    fn an_unchanged_canvas_sends_nothing() {
        let current = vec![7u8; 12 * 4 * 3];
        let mut held = current.clone();
        let mut all = RowSet::default();
        all.reset_all(12);
        assert!(changed_bands(&current, &mut held, 12, &all, GAP).is_empty());
    }

    #[test]
    fn nearby_changes_share_one_band_and_far_ones_do_not() {
        let stride = 4;
        let mut current = vec![0u8; stride * 100];
        let mut held = current.clone();
        for row in [10, 14, 60] {
            current[row * stride] = 1;
        }
        let mut all = RowSet::default();
        all.reset_all(100);
        assert_eq!(
            changed_bands(&current, &mut held, stride, &all, 8),
            vec![10..15, 60..61]
        );
    }
}
