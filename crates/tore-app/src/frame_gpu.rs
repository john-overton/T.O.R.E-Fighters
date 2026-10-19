//! Nonblocking whole-window GPU timestamps for the optional frame counter.
//! Three readbacks avoid waiting on a busy GPU. Unsupported adapters show `--`.
use std::sync::mpsc;

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
}

pub struct Timer {
    slots: Vec<Slot>,
    active: Option<usize>,
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
                        count: 2,
                    }),
                    resolve: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Frame counter resolve"),
                        size: 16,
                        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
                        mapped_at_creation: false,
                    }),
                    read: device.create_buffer(&wgpu::BufferDescriptor {
                        label: Some("Frame counter readback"),
                        size: 16,
                        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                        mapped_at_creation: false,
                    }),
                    pending: None,
                })
                .collect()
        };
        Self {
            slots,
            active: None,
            period: f64::from(queue.get_timestamp_period()),
        }
    }
    pub fn poll(&mut self, device: &wgpu::Device) -> Option<f64> {
        let _ = device.poll(wgpu::PollType::Poll);
        let mut latest = None;
        for slot in &mut self.slots {
            let Some(receive) = &slot.pending else {
                continue;
            };
            match receive.try_recv() {
                Ok(Ok(())) => {
                    let data = slot.read.slice(..).get_mapped_range();
                    let start = u64::from_le_bytes(data[..8].try_into().expect("timestamp"));
                    let end = u64::from_le_bytes(data[8..16].try_into().expect("timestamp"));
                    latest = elapsed(start, end, self.period);
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
    pub fn begin(&mut self, encoder: &mut wgpu::CommandEncoder) {
        self.active = self.slots.iter().position(|s| s.pending.is_none());
        if let Some(i) = self.active {
            encoder.write_timestamp(&self.slots[i].query, 0);
        }
    }
    pub fn end(&self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(i) = self.active {
            let slot = &self.slots[i];
            encoder.write_timestamp(&slot.query, 1);
            encoder.resolve_query_set(&slot.query, 0..2, &slot.resolve, 0);
            encoder.copy_buffer_to_buffer(&slot.resolve, 0, &slot.read, 0, 16);
        }
    }
    pub fn submitted(&mut self) {
        if let Some(i) = self.active.take() {
            let (send, receive) = mpsc::channel();
            self.slots[i]
                .read
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    let _ = send.send(result);
                });
            self.slots[i].pending = Some(receive);
        }
    }
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
}
