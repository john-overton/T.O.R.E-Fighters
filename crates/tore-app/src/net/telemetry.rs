//! Anonymous statistics for the Internet Lobby (slice I4): the install id,
//! the one-time notice, and the Report a joining player sends when a session
//! it found through the Internet Lobby ends.
//!
//! John's decision (2026-10-05): telemetry is on by default, with a switch
//! ("Send anonymous statistics" in the Internet Lobby's Options) and a
//! one-time notice. What is sent, and to whom, is the master's Report
//! (`docs/formats/master-protocol.md`, "Reports") and the guide's "Replay and
//! telemetry" section (`docs/MULTIPLAYER.md`); the README says it in plain
//! words. Only a game that uses the master sends anything: Direct
//! Connection and single player send nothing.
//!
//! - **The install id** is 64 random bits in `install-id` in the data folder,
//!   drawn the first time it is wanted. Turning statistics off deletes the
//!   file, so turning them on again draws a new id that cannot be linked to
//!   the old (as the dedicated server does with `server-install-id`).
//! - **The notice** is one line in the Internet Lobby's Messages the first
//!   time the screen opens. A game started from the command line with
//!   `--host FILE --list` prints it once on the log and the console the first
//!   time it sends (*agent decision*: a command line has no screen).
//! - **A player's Report** is sent by [`send`] on a thread of its own (the
//!   master's name is looked up there, so nothing waits), once, when the
//!   session ends. It is never answered and a lost one is lost. A hosting
//!   game's Report is the hosting thread's (`HostTally`).
//!
//! *Agent decision:* in stage I a joining player's path is told from the
//! host's address ([`tore_net::master::rendezvous::path_of`]); stage J's
//! join reports the path it really took.

use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::path::Path as FilePath;
use std::time::{Duration, Instant};
use tore_net::master::{
    MappingType, MasterPacket, Path, PortMapping, Report, Role, rendezvous::path_of,
};
use tore_net::{Platform, reach};

/// The file in the data folder that keeps the install id.
pub const INSTALL_ID_FILE: &str = "install-id";

/// The one line of the notice, as Messages shows it.
pub const NOTICE: &str =
    "This game sends anonymous statistics to the Internet Lobby. Turn them off in Options.";

/// The install id: read from the data folder, or drawn and written the first
/// time, while statistics are `on`. With them off there is none and the file
/// is deleted, so turning them on again draws a new one. A file that cannot
/// be written leaves the id for this run only.
pub fn install_id(data: &FilePath, on: bool) -> Option<u64> {
    let path = data.join(INSTALL_ID_FILE);
    if !on {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    let kept = std::fs::read_to_string(&path)
        .ok()
        .and_then(|text| u64::from_str_radix(text.trim(), 16).ok())
        .filter(|id| *id != 0);
    if kept.is_some() {
        return kept;
    }
    let id = loop {
        let id = reach::random_nonce();
        if id != 0 {
            break id;
        }
    };
    if let Err(error) = std::fs::write(&path, format!("{id:016x}\n")) {
        log::warn!("The install id was not saved: {error}");
    }
    Some(id)
}

/// The install id for a game started from the command line with
/// `--host FILE --list`: statistics follow the remembered switch (on by
/// default). The first time they are on, the one-time notice is said on the
/// log and the console, since a command line has no screen for it, and the
/// id is only given from then on (*agent decision*).
pub fn for_command_line(data: &FilePath, listing: &mut Option<crate::net::hosting::Listing>) {
    let Some(listing) = listing else {
        return;
    };
    let mut settings = crate::net::settings::Remembered::load(data);
    if settings.telemetry && !settings.telemetry_notice {
        log::info!("Telemetry: {NOTICE}");
        println!("{NOTICE} (the Internet Lobby screen's Options turn them off)");
        settings.telemetry_notice = true;
        if let Err(error) = settings.save(data) {
            log::warn!("Network settings not saved: {error}");
        }
    }
    listing.install_id = install_id(data, settings.telemetry && settings.telemetry_notice);
}

/// What a joined session tells the Report: when it began, how many humans it
/// held at most, how the player got there and how long that took.
#[derive(Clone, Debug)]
pub struct PlayerTally {
    started: Instant,
    humans: usize,
    path: Path,
    connect_tenths: u8,
}

impl PlayerTally {
    /// A session joined to `host`, begun now, after `asked` since the player
    /// pressed Join.
    pub fn begin(host: SocketAddr, asked: Duration) -> Self {
        Self {
            started: Instant::now(),
            humans: 0,
            path: path_of(host),
            connect_tenths: u8::try_from(asked.as_millis() / 100).unwrap_or(u8::MAX),
        }
    }

    /// The humans in the session now (the lobby's player count): the most of
    /// them is kept.
    pub fn present(&mut self, humans: usize) {
        self.humans = self.humans.max(humans);
    }

    /// True once the session held anyone (it reached its lobby).
    pub fn joined(&self) -> bool {
        self.humans > 0
    }

    /// The Report for a session that ends after `lasted` (its install id
    /// filled in by the caller).
    pub fn report(&self, install_id: u64, version: &str) -> Report {
        self.report_after(self.started.elapsed(), install_id, version)
    }

    fn report_after(&self, lasted: Duration, install_id: u64, version: &str) -> Report {
        let mut game_version = version.to_owned();
        while game_version.len() > 64 {
            game_version.pop();
        }
        Report {
            install_id,
            role: Role::Player,
            game_version,
            platform: Platform::current().code(),
            minutes: u16::try_from(lasted.as_secs() / 60).unwrap_or(u16::MAX),
            humans: u8::try_from(self.humans).unwrap_or(u8::MAX),
            path: self.path,
            connect_tenths: self.connect_tenths,
            mapping: MappingType::Unknown,
            port_mapping: PortMapping::NotTried,
            relayed_kb: 0,
            players_by_path: [0; 6],
            migrations: 0,
            failed_migrations: 0,
        }
    }
}

/// Sends `report` to `master` (`HOST` or `HOST:PORT`) once, on a thread of its
/// own: the name is looked up there and the datagram goes from a socket of
/// its own. Nothing is answered. The thread is detached; a game that exits
/// first loses the report.
pub fn send(master: &str, report: Report) {
    let master = master.to_owned();
    let spawned = std::thread::Builder::new()
        .name("tore-report".into())
        .spawn(move || match send_now(&master, &report) {
            Ok(to) => log::info!("Telemetry: sent the session's report to {to}"),
            Err(error) => log::info!("Telemetry: the report was not sent: {error}"),
        });
    if let Err(error) = spawned {
        log::info!("Telemetry: the report was not sent: {error}");
    }
}

/// [`send`] on the calling thread; the address it went to.
pub fn send_now(master: &str, report: &Report) -> io::Result<SocketAddr> {
    let (host, port) = tore_net::master::local::parse_master(master)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let addresses = reach::resolve(&host, port)?;
    let to = *addresses
        .first()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no address"))?;
    let datagram = MasterPacket::Report(report.clone())
        .encode()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{error:?}")))?;
    let bind: SocketAddr = if to.is_ipv6() {
        "[::]:0".parse().expect("an address")
    } else {
        "0.0.0.0:0".parse().expect("an address")
    };
    let socket = UdpSocket::bind(bind)?;
    socket.send_to(&datagram, to)?;
    Ok(to)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir =
            std::env::temp_dir().join(format!("tore-telemetry-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_id_is_kept_while_statistics_are_on_and_deleted_when_off() {
        let dir = scratch("id");
        let first = install_id(&dir, true).expect("an id");
        assert_ne!(first, 0);
        assert_eq!(install_id(&dir, true), Some(first));
        assert!(dir.join(INSTALL_ID_FILE).exists());
        assert_eq!(install_id(&dir, false), None);
        assert!(!dir.join(INSTALL_ID_FILE).exists());
        // On again: a new id, not the old one (2^-64 of colliding).
        let second = install_id(&dir, true).expect("an id");
        assert_ne!(second, first);
        // A file with nonsense in it is replaced.
        std::fs::write(dir.join(INSTALL_ID_FILE), "nonsense").unwrap();
        assert!(install_id(&dir, true).is_some());
        std::fs::write(dir.join(INSTALL_ID_FILE), "0000000000000000").unwrap();
        assert_ne!(install_id(&dir, true), Some(0));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_players_report_carries_only_what_the_guide_lists() {
        let host: SocketAddr = "203.0.113.9:26900".parse().unwrap();
        let mut tally = PlayerTally::begin(host, Duration::from_millis(2_350));
        assert!(!tally.joined());
        tally.present(2);
        tally.present(5);
        tally.present(3);
        assert!(tally.joined());
        let report = tally.report_after(Duration::from_secs(61 * 60 + 5), 0xabc, "0.1.3");
        assert_eq!(report.install_id, 0xabc);
        assert_eq!(report.role, Role::Player);
        assert_eq!(report.game_version, "0.1.3");
        assert_eq!(report.platform, Platform::current().code());
        assert_eq!((report.minutes, report.humans), (61, 5));
        assert_eq!(report.path, Path::ByAddress);
        assert_eq!(report.connect_tenths, 23);
        assert_eq!(report.relayed_kb, 0);
        assert_eq!(report.players_by_path, [0; 6]);
        // A host on this machine or this network is the local network.
        let lan = PlayerTally::begin("192.168.1.20:26900".parse().unwrap(), Duration::ZERO);
        assert_eq!(lan.path, Path::LocalNetwork);
        // A connect time past 25.5 s saturates; a long session saturates.
        let slow = PlayerTally::begin(host, Duration::from_secs(90));
        assert_eq!(slow.connect_tenths, 255);
        // It encodes as the master reads it.
        let datagram = MasterPacket::Report(report.clone()).encode().unwrap();
        assert_eq!(
            MasterPacket::decode(&datagram).unwrap(),
            MasterPacket::Report(report)
        );
    }

    #[test]
    fn a_report_reaches_a_master_on_loopback_and_a_bad_address_is_refused() {
        let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let port = socket.local_addr().unwrap().port();
        let tally = PlayerTally::begin("127.0.0.1:26900".parse().unwrap(), Duration::ZERO);
        let report = tally.report(7, "0.1.3");
        let to = send_now(&format!("127.0.0.1:{port}"), &report).unwrap();
        assert_eq!(to.port(), port);
        let mut buf = [0u8; 2_000];
        let (len, _) = socket.recv_from(&mut buf).unwrap();
        match MasterPacket::decode(&buf[..len]).unwrap() {
            MasterPacket::Report(got) => assert_eq!(got, report),
            other => panic!("not a report: {other:?}"),
        }
        assert!(send_now("two words", &report).is_err());
    }
}
