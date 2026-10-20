//! Working out again only the part of a scaled picture that a change in its
//! source can reach. `blit`'s filter gives each destination pixel from at
//! most a 2x2 block of source pixels (and the pixel beneath it), so a
//! destination pixel whose block is unchanged, over an unchanged pixel,
//! comes out the same. These helpers find the destination pixels that are
//! not certain to.
use std::ops::Range;

/// A destination row and a range of its columns.
pub(super) type Segment = (usize, Range<usize>);

/// For each destination pixel of `origin..origin + span` that lies on a
/// `limit`-pixel canvas: the pixel, the two source pixels the filter takes
/// and the weight of the second (`blit`'s own tap rule).
pub(super) fn filter_taps(
    origin: f64,
    span: f64,
    source: usize,
    limit: u32,
) -> Vec<(usize, usize, usize, f64)> {
    ((origin.floor() as i32).max(0)..((origin + span).ceil() as i32).min(limit as i32))
        .map(|d| {
            let v = ((d as f64 + 0.5 - origin) * source as f64 / span - 0.5)
                .clamp(0., (source - 1) as f64);
            let first = v.floor() as usize;
            (
                d as usize,
                first,
                (first + 1).min(source - 1),
                v - first as f64,
            )
        })
        .collect()
}

/// Per row of two same-sized RGBA pictures `width` pixels wide, the columns
/// from the first to the last pixel that differ, or `None` for an equal row.
pub(super) fn changed_spans(old: &[u8], new: &[u8], width: usize) -> Vec<Option<Range<usize>>> {
    let stride = width * 4;
    old.chunks_exact(stride)
        .zip(new.chunks_exact(stride))
        .map(|(old, new)| {
            if old == new {
                return None;
            }
            let differs = |(a, b): (&[u8], &[u8])| a != b;
            let pairs = || old.chunks_exact(4).zip(new.chunks_exact(4));
            let first = pairs().position(differs)?;
            let last = width - 1 - pairs().rev().position(differs)?;
            Some(first..last + 1)
        })
        .collect()
}

/// The destination pixels of a `blit` of a `source`-sized picture onto
/// `rect` of a `size` canvas that read a source pixel in `changed` (per
/// source row, as [`changed_spans`] gives). It covers every path `blit`
/// takes: the whole-pixel paths read exactly the first tap of this rule.
pub(super) fn reach(
    size: [u32; 2],
    source: [usize; 2],
    (x, y, w, h): (f64, f64, f64, f64),
    changed: &[Option<Range<usize>>],
) -> Vec<Segment> {
    let hull = |a: Option<&Range<usize>>, b: Option<&Range<usize>>| match (a, b) {
        (Some(a), Some(b)) => Some(a.start.min(b.start)..a.end.max(b.end)),
        (a, b) => a.or(b).cloned(),
    };
    let rows = filter_taps(y, h, source[1], size[1]);
    let columns = filter_taps(x, w, source[0], size[0]);
    let mut out = Vec::new();
    for (row, first, second, _) in rows {
        let Some(span) = hull(
            changed.get(first).and_then(Option::as_ref),
            changed.get(second).and_then(Option::as_ref),
        ) else {
            continue;
        };
        // Both taps only grow left to right, so the columns that read
        // `span` are one run.
        let start = columns.partition_point(|c| c.2 < span.start);
        let end = columns.partition_point(|c| c.1 < span.end);
        if start < end {
            out.push((row, columns[start].0..columns[end - 1].0 + 1));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_spans_cover_first_to_last_differing_pixel() {
        let old = vec![0u8; 5 * 3 * 4];
        let mut new = old.clone();
        new[(5 + 1) * 4 + 2] = 9;
        new[(5 + 3) * 4] = 1;
        new[(2 * 5 + 4) * 4 + 3] = 7;
        assert_eq!(
            changed_spans(&old, &new, 5),
            vec![None, Some(1..4), Some(4..5)]
        );
    }

    #[test]
    fn the_reach_holds_every_pixel_whose_taps_read_a_change() {
        for (size, rect, source) in [
            ([640, 480], (10., 20., 364.5, 360.), [162, 160]),
            ([640, 480], (0., 0., 162., 160.), [162, 160]),
            ([300, 200], (-30.5, -10.25, 330., 240.), [40, 30]),
            ([1920, 1080], (240., 0., 1440., 1080.), [640, 480]),
        ] {
            for seed in 0..20usize {
                let mut changed = vec![None; source[1]];
                for (n, row) in changed.iter_mut().enumerate() {
                    if (n * 7 + seed) % 5 == 0 {
                        let a = (n * 13 + seed * 3) % source[0];
                        *row = Some(a..(a + 1 + seed % 4).min(source[0]));
                    }
                }
                let segments = reach(size, source, rect, &changed);
                let rows = filter_taps(rect.1, rect.3, source[1], size[1]);
                let columns = filter_taps(rect.0, rect.2, source[0], size[0]);
                for &(yy, sy, sy1, _) in &rows {
                    for &(xx, sx, sx1, _) in &columns {
                        let reads = [sy, sy1].iter().any(|&r| {
                            changed[r]
                                .as_ref()
                                .is_some_and(|c| c.contains(&sx) || c.contains(&sx1))
                        });
                        let covered = segments
                            .iter()
                            .any(|(row, cols)| *row == yy && cols.contains(&xx));
                        assert!(!reads || covered, "{size:?} {rect:?} pixel {xx},{yy}");
                    }
                }
            }
        }
    }
}
