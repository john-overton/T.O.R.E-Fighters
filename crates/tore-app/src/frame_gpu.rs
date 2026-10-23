//! Nonblocking whole-frame GPU timestamps for the optional frame counter.
//!
//! One frame can submit several command buffers: a camera panel readback per
//! open panel, the mirror picture, then the window pass (scene, cockpit and
//! interface). Each submission is a *segment* with its own begin and end
//! timestamp, and the frame's GPU time is the sum of the segments, so GPU idle
//! time between submissions is not counted. Three readbacks avoid waiting on
//! a busy GPU. Unsupported adapters show `--`.
use std::sync::mpsc;

/// Most submissions timed in one frame: three camera panels, the mirrors and
/// the window pass, with room to spare. Later ones are simply not timed.
const MAX_SEGMENTS: usize = 8;
/// `resolve_query_set` writes at multiples of this many bytes.
const RESOLVE_ALIGN: u64 = wgpu::QUERY_RESOLVE_BUFFER_ALIGNMENT;
/// Two timestamps of eight bytes.
const PAIR_BYTES: u64 = 16;

pub fn features(available: wgpu::Features) -> wgpu::Features {
    let timing = wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_ENCODERS;
    if available.contains(timing) {
        timing
    } else {
        wgpu::Features::empty()
    }
}

struct Slot {
    query: wgpu::QuerySet,
    resolve: wgpu::Buffer,
    read: wgpu::Buffer,
    pending: Option<mpsc::Receiver<Result<(), wgpu::BufferAsyncError>>>,
    /// Segments the pending readback holds.
    segments: usize,
}

pub struct Timer {
    slots: Vec<Slot>,
    /// The slot this frame's segments are written into.
    active: Option<usize>,
    /// Segments begun in this frame so far.
    segments: usize,
    /// The segment between `begin` and `end` right now.
    current: Option<usize>,
    period: f64,
}
impl Timer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let slots = if features(device.features()).is_empty() {
            Vec::new()
        } else {
            (0..3)
                .map(|_| Slot {
                    query: device.create_query_set(&wgpu::QuerySetDescriptor {
                        label: Some("Frame counter timestamps"),
                        ty: wgpu::QueryType::Timestamp,
                        count: 2 * MAX_SEGMENTS as u32,
                    }),
                    resolve: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Frame counter resolve"),
                        size: RESOLVE_ALIGN * MAX_SEGMENTS as u64,
                        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }),
                    read: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Frame counter readback"),
                        size: PAIR_BYTES * MAX_SEGMENTS as u64,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    pending: None,
                    segments: 0,
                })
                .collect()
        };
        Self {
            slots,
            active: None,
            segments: 0,
            current: None,
            period: f64::from(queue.get_timestamp_period()),
        }
    }
    /// The newest finished frame's GPU milliseconds, if one has arrived.
    pub fn poll(&mut self, device: &wgpu::Device) -> Option<f64> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut latest = None;
        for slot in &mut self.slots {
            let Some(receive) = &slot.pending else {
                continue;
            };
            match receive.try_recv() {
                Ok(Ok(())) => {
                    let data = slot
                        .read
                        .slice(..PAIR_BYTES * slot.segments as u64)
                        .get_mapped_range();
                    let ticks: Vec<u64> = data
                        .chunks_exact(8)
                        .map(|t| u64::from_le_bytes(t.try_into().expect("timestamp")))
                        .collect();
                    latest = total_ms(&ticks, self.period);
                    drop(data);
                    slot.read.unmap();
                    slot.pending = None;
                }
                Ok(Err(_)) | Err(mpsc::TryRecvError::Disconnected) => {
                    slot.pending = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        latest
    }
    /// Start timing the submission `encoder` will become. The first call
    /// after a frame was submitted opens the next frame. Call [`Timer::end`]
    /// on the same encoder before it is finished and submitted.
    pub fn begin(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if self.active.is_none() {
            self.active = self.slots.iter().position(|s| s.pending.is_none());
            self.segments = 0;
        }
        self.current = None;
        let Some(i) = self.active else {
            return;
        };
        if self.segments >= MAX_SEGMENTS {
            return;
        }
        encoder.write_timestamp(&self.slots[i].query, 2 * self.segments as u32);
        self.current = Some(self.segments);
        self.segments += 1;
    }
    pub fn end(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let (Some(i), Some(segment)) = (self.active, self.current.take()) {
            let slot = &self.slots[i];
            let first = 2 * segment as u32;
            encoder.write_timestamp(&slot.query, first + 1);
            let at = RESOLVE_ALIGN * segment as u64;
            encoder.resolve_query_set(&slot.query, first..first + 2, &slot.resolve, at);
            encoder.copy_buffer_to_buffer(
                &slot.resolve,
                at,
                &slot.read,
                PAIR_BYTES * segment as u64,
                PAIR_BYTES,
            );
        }
    }
    /// The frame's last submission went to the queue: read all its segments
    /// back without waiting.
    pub fn submitted(&mut self) {
        if let Some(i) = self.active.take() {
            let segments = std::mem::take(&mut self.segments);
            if segments == 0 {
                return;
            }
            let (send, receive) = mpsc::channel();
            self.slots[i]
                .read
                .slice(..PAIR_BYTES * segments as u64)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = send.send(result);
                });
            self.slots[i].pending = Some(receive);
            self.slots[i].segments = segments;
        }
    }
    /// Forget segments of a frame that will not be read back, such as when
    /// the counter is switched off between a panel and the window pass.
    pub fn cancel(&mut self) {
        self.active = None;
        self.segments = 0;
        self.current = None;
    }
}
/// The sum of each (begin, end) pair in `ticks`, in milliseconds. `None` when
/// there is no pair, or any pair ends before it begins.
fn total_ms(ticks: &[u64], period: f64) -> Option<f64> {
    let mut total = 0.;
    let mut pairs = 0;
    for pair in ticks.chunks_exact(2) {
        total += elapsed(pair[0], pair[1], period)?;
        pairs += 1;
    }
    (pairs > 0).then_some(total)
}
fn elapsed(start: u64, end: u64, period: f64) -> Option<f64> {
    (end >= start && period.is_finite() && period > 0.)
        .then_some(end.saturating_sub(start) as f64 * period / 1_000_000.)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn timestamps_are_optional_and_not_wall_clock_estimates() {
        assert!(features(wgpu::Features::TIMESTAMP_QUERY).is_empty());
        assert_eq!(elapsed(10, 4_200_010, 1.), Some(4.2));
        assert_eq!(elapsed(20, 10, 1.), None);
        assert_eq!(elapsed(0, 20, 0.), None);
    }
    #[test]
    fn a_frame_is_the_sum_of_its_submissions_and_not_the_gaps_between_them() {
        // A panel (1 ms), the mirrors (2 ms) and the window pass (4 ms), with
        // the GPU idle for 50 ms between them while the CPU prepared.
        let ticks = [
            1_000_000,
            2_000_000, // panel
            52_000_000,
            54_000_000, // mirrors
            104_000_000,
            108_000_000, // window
        ];
        let ms = total_ms(&ticks, 1.).unwrap();
        assert!((ms - 7.0).abs() < 1e-9, "{ms}");
        // The tick period scales every segment.
        let ms = total_ms(&ticks, 2.).unwrap();
        assert!((ms - 14.0).abs() < 1e-9, "{ms}");
        // One window pass alone is the old reading.
        assert_eq!(total_ms(&ticks[4..], 1.), Some(4.0));
    }
    #[test]
    fn a_broken_pair_or_no_pairs_is_unknown() {
        assert_eq!(total_ms(&[], 1.), None);
        assert_eq!(total_ms(&[5], 1.), None);
        assert_eq!(total_ms(&[1, 2, 9, 3], 1.), None);
        assert_eq!(total_ms(&[1, 2], f64::NAN), None);
    }
}
