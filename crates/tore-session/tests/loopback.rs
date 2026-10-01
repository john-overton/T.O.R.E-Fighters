//! A host and two bots over real UDP on the loopback interface: the network
//! job of CI (slice D10), on Linux, Windows and macOS.
//!
//! The host is `tore_session::Host` in this process, on a real socket with the
//! real clock and system entropy. The two bots are the real `tore-bot` program,
//! two processes, each loading a synthetic import from a data folder this test
//! writes (the fixtures of `tore-world`'s test support, with the import's
//! markers), joining, taking a plane, flying the scripted fight and leaving
//! with their debriefs. No retail data is used.
//!
//! The test fails on any refusal, any fault, any player that does not leave by
//! its own Leave (a drop), and any bot that does not exit cleanly. It flies
//! `TORE_LOOPBACK_SECONDS` seconds of real time (default 6).
//!
//! ```sh
//! cargo test --locked -p tore-session --test loopback -- --nocapture
//! ```

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tore_formats::aircraft::AircraftId;
use tore_net::{RealClock, bind_udp};
use tore_session::host::LeaveReason;
use tore_session::{BuildId, Host, HostConfig, HostLog};
use tore_world::mission::{MissionSpec, Skill, Start};
use tore_world::test_support::resources::{THEATER, resources};

/// The same build identity the `tore-bot` program computes.
fn build() -> BuildId {
    BuildId {
        version: option_env!("TORE_BUILD_VERSION")
            .unwrap_or(env!("CARGO_PKG_VERSION"))
            .to_owned(),
        commit: env!("TORE_BUILD_COMMIT").to_owned(),
        release: option_env!("TORE_BUILD_VERSION").is_some(),
    }
}

/// Two friendly flights' worth: the bots and two AI wingmen against two
/// enemy AI, 5 nautical miles apart.
fn spec() -> MissionSpec {
    let mut spec = MissionSpec::new(THEATER, AircraftId::F18);
    spec.wings[0].count = 4;
    spec.wings[3].count = 2;
    spec.wings[3].skill = Skill::Average;
    spec.separation_nm = 5;
    spec.start = Start::Airborne {
        altitude_ft: 10_000,
    };
    spec
}

/// A data folder in the system's temporary folder, removed at the end.
struct Folder(PathBuf);

impl Folder {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("tore-loopback-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("a temporary folder");
        Self(path)
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// A bot process, killed if the test ends without it having finished.
struct Bot {
    name: &'static str,
    child: Child,
    output: PathBuf,
    status: Option<ExitStatus>,
}

impl Bot {
    fn start(name: &'static str, folder: &Path, server: &str, seconds: u64) -> Self {
        let output = folder.join(format!("{name}.txt"));
        let file = File::create(&output).expect("an output file");
        let child = Command::new(env!("CARGO_BIN_EXE_tore-bot"))
            .args(["--connect", server, "--data-dir"])
            .arg(folder)
            .args(["--callsign", name, "--seconds", &seconds.to_string()])
            .stdout(Stdio::from(file.try_clone().expect("a handle")))
            .stderr(Stdio::from(file))
            .spawn()
            .expect("the tore-bot program starts");
        Self {
            name,
            child,
            output,
            status: None,
        }
    }

    fn poll(&mut self) -> bool {
        if self.status.is_none() {
            self.status = self.child.try_wait().expect("the child can be polled");
        }
        self.status.is_some()
    }

    fn said(&self) -> String {
        fs::read_to_string(&self.output).unwrap_or_default()
    }
}

impl Drop for Bot {
    fn drop(&mut self) {
        if self.status.is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

#[test]
fn a_host_and_two_bots_fly_over_loopback_udp() {
    let seconds: u64 = std::env::var("TORE_LOOPBACK_SECONDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(6);
    // The synthetic import, as the importer leaves one.
    let mut import = resources();
    for (marker, value) in [
        ("TORE_MUSIC_V1", "PCM1"),
        ("TORE_COMBAT_V1", "RAW1"),
        ("TORE_AIRPORTS_V1", "SCENE1"),
        ("TORE_SPEECH_V1", "ALL1"),
    ] {
        import.insert(marker.into(), value.as_bytes().to_vec());
    }
    let folder = Folder::new();
    tore_import::pack::write_pack(&folder.0.join("menu-loopback.pack"), &import)
        .expect("the synthetic import is written");

    let mut host = Host::new(spec(), Arc::new(import), HostConfig::new(build()))
        .expect("the host builds the mission");
    let mut socket = bind_udp("127.0.0.1:0".parse().unwrap()).expect("a loopback socket");
    let server = socket.local_addr().expect("an address").to_string();
    let clock = RealClock::new();

    let mut bots = [
        Bot::start("Alpha", &folder.0, &server, seconds),
        Bot::start("Bravo", &folder.0, &server, seconds),
    ];
    let mut log: Vec<HostLog> = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(seconds + 40);
    let mut all_done_at: Option<Instant> = None;
    loop {
        let now = clock.now();
        host.receive_from(now, &mut socket)
            .expect("the host reads its socket");
        host.update(now);
        host.transmit(&mut socket)
            .expect("the host writes its socket");
        while let Some(entry) = host.poll_log() {
            log.push(entry);
        }
        let done = bots.iter_mut().fold(true, |all, bot| bot.poll() && all);
        if done {
            // A moment more, for the host to hear the last Leave.
            let since = *all_done_at.get_or_insert_with(Instant::now);
            if since.elapsed() > Duration::from_millis(500) {
                break;
            }
        }
        assert!(
            Instant::now() < deadline,
            "the bots did not finish in time.\n{}\n{}\nhost log: {log:#?}",
            bots[0].said(),
            bots[1].said()
        );
        std::thread::sleep(
            host.next_wake(now)
                .clamp(Duration::from_micros(200), Duration::from_millis(2)),
        );
    }
    while let Some(entry) = host.poll_log() {
        log.push(entry);
    }

    for bot in &bots {
        eprintln!("--- {} ---\n{}", bot.name, bot.said());
    }
    for bot in &bots {
        assert!(
            bot.status.is_some_and(|s| s.success()),
            "{} exited with {:?}\n{}",
            bot.name,
            bot.status,
            bot.said()
        );
    }
    let mut overloads = 0;
    let mut seated = 0;
    let mut left = 0;
    for entry in &log {
        match entry {
            HostLog::Refused { .. }
            | HostLog::ContentRefused { .. }
            | HostLog::SeatRefused { .. }
            | HostLog::Fault { .. } => panic!("an error or refusal: {entry:?}\n{log:#?}"),
            HostLog::Seated { .. } => seated += 1,
            HostLog::Left { reason, .. } => {
                // Anything but the player's own Leave is a drop.
                assert_eq!(*reason, LeaveReason::Left, "a drop: {entry:?}\n{log:#?}");
                left += 1;
            }
            HostLog::Overloaded { .. } => overloads += 1,
            _ => {}
        }
    }
    assert_eq!((seated, left), (2, 2), "{log:#?}");
    assert!(host.players().is_empty(), "{:?}", host.players());
    eprintln!("host overloads: {overloads}");
}
