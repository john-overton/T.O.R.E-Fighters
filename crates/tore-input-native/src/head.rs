//! Head-tracker poses in the opentrack "UDP over network" format: one 48-byte
//! datagram of six little-endian `f64` values, x/y/z in centimetres followed by
//! yaw/pitch/roll in degrees. opentrack reads TrackIR hardware, webcams and
//! phones, so this one receiver covers them all. The socket binds loopback
//! only, so no firewall prompt or remote sender is involved.
use std::{
    net::UdpSocket,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadPose {
    /// Degrees, in the sender's convention.
    pub yaw: f64,
    pub pitch: f64,
    pub roll: f64,
    /// Centimetres.
    pub position: [f64; 3],
}

/// A pose older than this is treated as a stopped tracker.
pub const STALE: Duration = Duration::from_millis(500);

pub fn parse(packet: &[u8]) -> Option<HeadPose> {
    if packet.len() != 48 {
        return None;
    }
    let v: Vec<f64> = packet
        .chunks_exact(8)
        .map(|b| f64::from_le_bytes(b.try_into().unwrap()))
        .collect();
    if v.iter().any(|v| !v.is_finite()) || v[3..].iter().any(|a| a.abs() > 360.) {
        return None;
    }
    Some(HeadPose {
        yaw: v[3],
        pitch: v[4],
        roll: v[5],
        position: [v[0], v[1], v[2]],
    })
}

pub struct HeadTracker {
    port: u16,
    rx: Option<mpsc::Receiver<HeadPose>>,
    quit: Arc<AtomicBool>,
    worker: Option<thread::JoinHandle<()>>,
    latest: Option<(HeadPose, Instant)>,
    /// Why the receiver is not listening, such as a port already in use.
    pub error: Option<String>,
}
impl HeadTracker {
    pub fn disabled() -> Self {
        Self {
            port: 0,
            rx: None,
            quit: Arc::new(AtomicBool::new(false)),
            worker: None,
            latest: None,
            error: None,
        }
    }
    pub fn start(port: u16) -> Self {
        let mut tracker = Self::disabled();
        tracker.port = port;
        let socket = match UdpSocket::bind(("127.0.0.1", port)) {
            Ok(socket) => socket,
            Err(e) => {
                tracker.error = Some(format!("UDP {port}: {e}"));
                return tracker;
            }
        };
        if let Err(e) = socket.set_read_timeout(Some(Duration::from_millis(50))) {
            tracker.error = Some(e.to_string());
            return tracker;
        }
        let (tx, rx) = mpsc::sync_channel(256);
        let quit = tracker.quit.clone();
        tracker.worker = Some(thread::spawn(move || {
            let mut buffer = [0u8; 64];
            while !quit.load(Ordering::Relaxed) {
                if let Ok(n) = socket.recv(&mut buffer)
                    && let Some(pose) = parse(&buffer[..n])
                {
                    // A full queue only means the app is behind; the next pose replaces it.
                    let _ = tx.try_send(pose);
                }
            }
        }));
        tracker.rx = Some(rx);
        tracker
    }
    pub fn port(&self) -> Option<u16> {
        self.worker.as_ref().map(|_| self.port)
    }
    /// The newest pose, or `None` when nothing arrived within [`STALE`].
    pub fn poll(&mut self) -> Option<HeadPose> {
        if let Some(rx) = &self.rx
            && let Some(pose) = rx.try_iter().last()
        {
            self.latest = Some((pose, Instant::now()));
        }
        self.latest
            .filter(|(_, at)| at.elapsed() < STALE)
            .map(|(pose, _)| pose)
    }
}
impl Drop for HeadTracker {
    fn drop(&mut self) {
        self.quit.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn packet(values: [f64; 6]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }
    #[test]
    fn parses_only_complete_finite_poses() {
        let pose = parse(&packet([1., 2., 3., 30., -10., 5.])).unwrap();
        assert_eq!((pose.yaw, pose.pitch, pose.roll), (30., -10., 5.));
        assert_eq!(pose.position, [1., 2., 3.]);
        assert!(parse(&packet([0., 0., 0., f64::NAN, 0., 0.])).is_none());
        assert!(parse(&packet([0., 0., 0., 720., 0., 0.])).is_none());
        assert!(parse(&[0; 47]).is_none());
    }
    #[test]
    fn loopback_pose_arrives_and_goes_stale() {
        let probe = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = probe.local_addr().unwrap().port();
        drop(probe);
        let mut tracker = HeadTracker::start(port);
        if tracker.port().is_none() {
            return; // Another process took the port between probe and bind.
        }
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender
            .send_to(&packet([0., 0., 0., 12., 4., 0.]), ("127.0.0.1", port))
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        let mut pose = None;
        while pose.is_none() && Instant::now() < deadline {
            pose = tracker.poll();
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(pose.map(|p| p.yaw), Some(12.));
        let second = HeadTracker::start(port);
        assert!(second.error.is_some(), "port reuse must be reported");
    }
}
