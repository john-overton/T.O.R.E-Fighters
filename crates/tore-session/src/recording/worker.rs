//! Bounded, ordered disk work for the authoritative recorder. The client uses
//! the same sync-channel pattern, but its drop-and-gap policy is different.
use super::{Options, bounds, retain};
use std::{
    fs::{self, File},
    io::{BufWriter, Write},
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
        mpsc::{self, Receiver, SyncSender, TrySendError},
    },
    thread::JoinHandle,
};
use tore_replay::{self as replay, Frame, Writer};

// The lead requested 60 seconds of headroom on 2026-10-23 so multi-second
// disk stalls do not end recording. This bounds the queue AND current chunk;
// pending frames are volatile. The writer's 3,600-frame sync cadence is unchanged.
const PENDING_FRAMES: u64 = 60 * replay::model::TICKS_PER_SECOND;
const BACKLOG: &str = "replay writer queue full: disk fell behind; recording stopped, incomplete .partial file retained";

#[derive(Default)]
pub(super) struct Tick {
    pub aircraft: Vec<replay::AircraftInfo>,
    pub weapons: Vec<replay::WeaponInfo>,
    pub frame: Frame,
}

enum Message {
    Tick(Box<Tick>),
    Finish(u64),
}

type Finished = Result<PathBuf, String>;

pub(super) struct Worker {
    sender: Option<SyncSender<Message>>,
    thread: Option<JoinHandle<Finished>>,
    flushed: Arc<AtomicU64>,
    submitted: u64,
    failure: Option<String>,
}

impl Worker {
    pub fn start(
        options: Options,
        path: PathBuf,
        header: replay::Header,
        surface: Vec<replay::SurfaceInfo>,
    ) -> Result<Self, String> {
        Self::spawn(move |receiver, flushed| {
            fs::create_dir_all(&options.folder).map_err(|e| e.to_string())?;
            let writer = Writer::create(&path, &header).map_err(|e| e.to_string())?;
            let log = BufWriter::new(
                File::create(path.with_extension("log")).map_err(|e| e.to_string())?,
            );
            write(receiver, flushed, writer, log, surface, options)
        })
    }

    fn spawn(
        run: impl FnOnce(Receiver<Message>, Arc<AtomicU64>) -> Finished + Send + 'static,
    ) -> Result<Self, String> {
        let (sender, receiver) = mpsc::sync_channel(PENDING_FRAMES as usize);
        let flushed = Arc::new(AtomicU64::new(0));
        let progress = flushed.clone();
        let thread = std::thread::Builder::new()
            .name("tore-server-recorder".into())
            .spawn(move || run(receiver, progress))
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sender: Some(sender),
            thread: Some(thread),
            flushed,
            submitted: 0,
            failure: None,
        })
    }

    /// Never waits for the disk or joins. A failed worker stays owned until
    /// the lifecycle boundary, so even a blocked fsync cannot stall a tick.
    pub fn push(&mut self, tick: Tick) -> Result<(), String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        if self.submitted - self.flushed.load(Ordering::Acquire) >= PENDING_FRAMES {
            return self.fail(BACKLOG.into());
        }
        match self
            .sender
            .as_ref()
            .expect("running worker")
            .try_send(Message::Tick(Box::new(tick)))
        {
            Ok(()) => {
                self.submitted += 1;
                Ok(())
            }
            Err(TrySendError::Full(_)) => self.fail(BACKLOG.into()),
            Err(TrySendError::Disconnected(_)) => {
                // The receiver can disappear before the thread finishes its
                // error cleanup. Defer that join too.
                self.fail(
                    "replay writer stopped; recording incomplete (details at mission end)".into(),
                )
            }
        }
    }

    fn fail(&mut self, error: String) -> Result<(), String> {
        self.failure = Some(error.clone());
        self.sender.take(); // Drain accepted ticks, then sync the partial file.
        Err(error)
    }

    /// At a mission boundary it is safe to wait for the last frame and footer.
    pub fn push_final(&mut self, tick: Tick) -> Result<(), String> {
        if let Some(error) = &self.failure {
            return Err(error.clone());
        }
        self.sender
            .as_ref()
            .expect("running worker")
            .send(Message::Tick(Box::new(tick)))
            .map_err(|_| "replay writer stopped".to_owned())
    }

    pub fn finish(mut self, feedback: u64) -> Finished {
        if let Some(sender) = self.sender.take() {
            let _ = sender.send(Message::Finish(feedback));
        }
        let result = self.join();
        match self.failure.take() {
            Some(error) => Err(format!("{error}; {}", result.err().unwrap_or_default())),
            None => result,
        }
    }

    fn join(&mut self) -> Finished {
        self.thread
            .take()
            .expect("writer joined once")
            .join()
            .unwrap_or_else(|_| {
                Err("replay writer panicked; incomplete .partial file retained".into())
            })
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.sender.take();
        if self.thread.is_some() {
            let _ = self.join();
        }
    }
}

fn write(
    receiver: Receiver<Message>,
    flushed: Arc<AtomicU64>,
    mut writer: Writer,
    mut log: BufWriter<File>,
    surface: Vec<replay::SurfaceInfo>,
    options: Options,
) -> Finished {
    for info in surface {
        writer
            .register_surface_unit(&info)
            .map_err(|e| e.to_string())?;
    }
    for message in receiver {
        match message {
            Message::Tick(tick) => {
                write_tick(&mut writer, &mut log, *tick)?;
                // push only returns after any scheduled sync_all completes.
                flushed.store(writer.flushed_frames(), Ordering::Release);
            }
            Message::Finish(feedback) => {
                log.flush().map_err(|e| e.to_string())?;
                let footer = replay::Footer {
                    end_tick: writer.last_tick().unwrap_or(0),
                    result: vec![("feedback".into(), feedback.to_string())],
                };
                let path = writer.finish(&footer).map_err(|e| e.to_string())?;
                // Close the companion before retention, including on Windows.
                drop(log);
                retain(&options.folder, options.kept)?;
                return Ok(path);
            }
        }
    }
    log.flush().map_err(|e| e.to_string())?;
    // Writer::drop flushes its final chunk and syncs without a footer/rename.
    drop(writer);
    Err("recording incomplete; accepted frames flushed to .partial".into())
}

fn write_tick(writer: &mut Writer, log: &mut impl Write, mut tick: Tick) -> Result<(), String> {
    for info in tick.aircraft {
        writer.register_aircraft(&info).map_err(|e| e.to_string())?;
    }
    for info in tick.weapons {
        writer.register_weapon(&info).map_err(|e| e.to_string())?;
    }
    bounds::fit(&mut tick.frame);
    for event in &tick.frame.events {
        writeln!(
            log,
            "tick {}: {} {}",
            tick.frame.tick,
            event.kind,
            replay::feedback::Feedback::from_event(event)
                .map_or_else(|| event.text.clone(), |f| f.text)
                .replace(['\n', '\r'], " ")
        )
        .map_err(|e| e.to_string())?;
    }
    writer.push(&tick.frame).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    fn folder(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "tore-recorder-worker-{name}-{}",
            std::process::id()
        ));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn tick(n: u64) -> Tick {
        Tick {
            aircraft: if n == 0 || n == 120 {
                vec![replay::AircraftInfo {
                    id: n as u32,
                    pt: "F18.PT".into(),
                    name: "Synthetic F/A-18D".into(),
                    ..Default::default()
                }]
            } else {
                vec![]
            },
            weapons: if n == 121 {
                vec![replay::WeaponInfo {
                    id: 0,
                    source: "synthetic".into(),
                    name: "Synthetic gun".into(),
                    ..Default::default()
                }]
            } else {
                vec![]
            },
            frame: Frame {
                tick: n,
                events: vec![
                    replay::Event::new("server.log").with_text(format!("frame {n}\nsecond line")),
                ],
                ..Default::default()
            },
        }
    }

    #[test]
    fn ordered_threaded_output_and_companion_are_byte_identical_to_synchronous_writer() {
        let folder = folder("identical");
        let options = Options {
            folder: folder.clone(),
            kept: 10,
        };
        let header = replay::Header::default();
        let path = folder.join("server-async.tore-replay");
        let mut worker = Worker::start(options, path.clone(), header.clone(), vec![]).unwrap();
        let reference = folder.join("reference.tore-replay");
        let mut writer = Writer::create(&reference, &header).unwrap();
        let mut log = Vec::new();
        // Cross two fsync boundaries and end with a partial chunk.
        for n in 0..7_205 {
            let t = tick(n);
            // Reference is the former synchronous sequence, independently
            // of write_tick, so changed registration/log ordering is caught.
            for info in &t.aircraft {
                writer.register_aircraft(info).unwrap();
            }
            for info in &t.weapons {
                writer.register_weapon(info).unwrap();
            }
            for event in &t.frame.events {
                writeln!(
                    log,
                    "tick {}: {} {}",
                    n,
                    event.kind,
                    event.text.replace(['\n', '\r'], " ")
                )
                .unwrap();
            }
            writer.push(&t.frame).unwrap();
            // Synthetic input runs faster than real time; this test checks
            // ordering/bytes, with the nonblocking policy tested separately.
            worker.push_final(tick(n)).unwrap();
        }
        writer
            .finish(&replay::Footer {
                end_tick: 7_204,
                result: vec![("feedback".into(), "0".into())],
            })
            .unwrap();
        assert_eq!(worker.finish(0).unwrap(), path);
        assert_eq!(fs::read(&path).unwrap(), fs::read(reference).unwrap());
        assert_eq!(fs::read(path.with_extension("log")).unwrap(), log);
        let r = replay::Recording::open(&path).unwrap();
        assert!(r.complete());
        assert!(r.problems().is_empty());
        assert_eq!(r.frame_count(), 7_205);
        assert_eq!(
            r.frames(0, 7_204)
                .map(|f| f.unwrap().tick)
                .collect::<Vec<_>>(),
            (0..7_205).collect::<Vec<_>>()
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn blocked_disk_exhausts_budget_without_waiting_then_drains_to_partial_and_joins() {
        assert_eq!(PENDING_FRAMES, 7_200);
        let folder = folder("blocked");
        let path = folder.join("server-blocked.tore-replay");
        let output = path.clone();
        let options = Options {
            folder: folder.clone(),
            kept: 0,
        };
        let (release, gate) = mpsc::channel();
        let joined = Arc::new(AtomicBool::new(false));
        let done = joined.clone();
        let mut worker = Worker::spawn(move |rx, progress| {
            // Nothing can reach the disk until the producer's nonblocking
            // failure has returned and this test releases the writer.
            gate.recv().unwrap();
            let writer = Writer::create(&output, &replay::Header::default()).unwrap();
            let log = BufWriter::new(File::create(output.with_extension("log")).unwrap());
            let result = write(rx, progress, writer, log, vec![], options);
            done.store(true, Ordering::Release);
            result
        })
        .unwrap();
        for n in 0..PENDING_FRAMES {
            worker.push(tick(n)).unwrap();
        }
        assert_eq!(worker.push(tick(PENDING_FRAMES)).unwrap_err(), BACKLOG);
        assert!(!joined.load(Ordering::Acquire));
        release.send(()).unwrap();
        assert!(worker.finish(0).unwrap_err().contains("incomplete"));
        assert!(joined.load(Ordering::Acquire));
        assert!(!path.exists());
        let partial = replay::partial_path(&path);
        let r = replay::Recording::open(&partial).unwrap();
        assert!(!r.complete());
        assert_eq!(r.frame_count(), PENDING_FRAMES);
        assert!(
            path.with_extension("log").exists(),
            "incomplete recordings bypass retention"
        );
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn five_seconds_without_writer_progress_keeps_every_frame_and_feedback() {
        let folder = folder("five-second-stall");
        let path = folder.join("server-stall.tore-replay");
        let output = path.clone();
        let options = Options {
            folder: folder.clone(),
            kept: 0,
        };
        let (release, gate) = mpsc::channel();
        let mut worker = Worker::spawn(move |rx, progress| {
            gate.recv().unwrap();
            let writer = Writer::create(&output, &replay::Header::default()).unwrap();
            let log = BufWriter::new(File::create(output.with_extension("log")).unwrap());
            write(rx, progress, writer, log, vec![], options)
        })
        .unwrap();
        // Simulate five seconds at 120 Hz with a completely stalled writer,
        // without a wall-clock sleep. Include the chunk whose fsync is pending.
        let frames = 120 + 5 * replay::model::TICKS_PER_SECOND;
        let queued = (0..frames).try_for_each(|n| {
            let mut t = tick(n);
            if n == 300 {
                t.frame.events.push(
                    replay::feedback::Feedback {
                        tick: n,
                        text: "feedback during disk stall".into(),
                        ..Default::default()
                    }
                    .event(),
                );
            }
            worker.push(t)
        });
        let flushed_while_stalled = worker.flushed.load(Ordering::Acquire);
        // Release before asserting so a regressed budget fails without
        // leaving Drop waiting for this test's own gate.
        release.send(()).unwrap();
        queued.unwrap();
        assert_eq!(flushed_while_stalled, 0);
        assert!(worker.failure.is_none());
        // A later frame and Finish follow every frame queued during the stall.
        worker.push(tick(frames)).unwrap();
        assert_eq!(worker.finish(1).unwrap(), path);
        let r = replay::Recording::open(&path).unwrap();
        assert!(r.complete());
        assert!(r.problems().is_empty());
        assert_eq!(r.frame_count(), frames + 1);
        assert_eq!(
            r.frames(0, frames)
                .map(|f| f.unwrap().tick)
                .collect::<Vec<_>>(),
            (0..=frames).collect::<Vec<_>>()
        );
        let feedback: Vec<_> = r.feedback().collect();
        assert_eq!(feedback.len(), 1);
        assert_eq!(feedback[0].text, "feedback during disk stall");
        assert!(
            fs::read_to_string(path.with_extension("log"))
                .unwrap()
                .contains("feedback during disk stall")
        );
        assert!(path.exists(), "flagged recording survives kept=0");
        fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn budget_includes_frames_already_received_but_not_flushed() {
        let (seen, received) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let mut worker = Worker::spawn(move |rx, _progress| {
            for _ in 0..PENDING_FRAMES {
                assert!(matches!(rx.recv().unwrap(), Message::Tick(_)));
            }
            seen.send(()).unwrap();
            gate.recv().unwrap();
            assert!(rx.recv().is_err());
            Err("incomplete".into())
        })
        .unwrap();
        for n in 0..PENDING_FRAMES {
            worker.push(tick(n)).unwrap();
        }
        received.recv().unwrap();
        // The channel is empty but the chunk is not on disk yet.
        assert_eq!(worker.push(tick(PENDING_FRAMES)).unwrap_err(), BACKLOG);
        release.send(()).unwrap();
        assert!(worker.finish(0).is_err());
    }

    #[test]
    fn acknowledged_chunks_replenish_budget_and_drop_joins() {
        let (seen, received) = mpsc::channel();
        let joined = Arc::new(AtomicBool::new(false));
        let done = joined.clone();
        let mut worker = Worker::spawn(move |rx, progress| {
            for _ in 0..PENDING_FRAMES {
                assert!(matches!(rx.recv().unwrap(), Message::Tick(_)));
            }
            progress.store(PENDING_FRAMES, Ordering::Release);
            seen.send(()).unwrap();
            assert!(matches!(rx.recv().unwrap(), Message::Tick(_)));
            assert!(rx.recv().is_err());
            done.store(true, Ordering::Release);
            Err("incomplete".into())
        })
        .unwrap();
        for n in 0..PENDING_FRAMES {
            worker.push(tick(n)).unwrap();
        }
        received.recv().unwrap();
        worker.push(tick(PENDING_FRAMES)).unwrap();
        drop(worker);
        assert!(joined.load(Ordering::Acquire));
    }

    #[test]
    fn io_failure_and_panic_are_reported_and_joined() {
        let folder = folder("failure");
        let bad = folder.join("not-a-directory");
        fs::write(&bad, b"file").unwrap();
        let worker = Worker::start(
            Options {
                folder: bad.clone(),
                kept: 10,
            },
            bad.join("server.tore-replay"),
            replay::Header::default(),
            vec![],
        )
        .unwrap();
        assert!(worker.finish(0).is_err());
        let worker = Worker::spawn(|_, _| panic!("synthetic writer panic")).unwrap();
        assert!(worker.finish(0).unwrap_err().contains("panicked"));
        fs::remove_dir_all(folder).unwrap();
    }
}
