//! A bot that hosts, and a bot that stands by (stage K, slice K9;
//! docs/ARCHITECTURE.md, "Host migration and rejoin"): what the game's hosting
//! thread and its standby do (`tore-app`, slice K7a), run headless in
//! `tore-bot` so the migration scenarios of the battery's net lane need no
//! window.
//!
//! One bot is one game. It owns a [`Mig`], which holds
//!
//! - the game's **peers router** ([`Peers`]) in front of its socket, so a
//!   joined bot answers Reach for its session and does the host selection's
//!   reach tests;
//! - the game's **standby** ([`StandbyThread`]) when the bot stands by
//!   (`--standby on`): the host's stream goes to it, its status goes back,
//!   and when [`Client::takeover_due`] says so the bot takes the game over;
//! - the game's **host** ([`Hosted`]) when it hosts: the original (`--host`)
//!   or the one it took over. Its own client joins it over an in-process
//!   link, here the routing of [`LINK_ADDRESS`] datagrams between the
//!   client and the host, as the simulator tests of slice K4 do.
//!
//! *Agent decisions (K9):* the bot runs on one thread (the host steps at 120
//! Hz inside the bot's loop, which wakes at least every 2 ms), where the game
//! gives the host a thread of its own: a bot's loop has no window to stall.
//! The standby is on its own thread, as in the game. A took-over host leaves
//! only after its guests have gone (at most [`GUESTS_WAIT`] past the bot's
//! `--seconds`), so a scenario's bots end in order. A hosting bot that ends
//! hands the game over when a standby is ready, and says the host left when
//! none is.

use std::collections::BTreeMap;
use std::net::{SocketAddr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tore_net::master::rendezvous::ListingPart;
use tore_net::master::{Build, HostListing, HostRendezvous};
use tore_net::peers::{Peers, Route};
use tore_net::{Datagrams, Entropy, LINK_ADDRESS, MAX_DATAGRAM, Platform, ServerSocket};
use tore_session::bot::Bot;
use tore_session::client::candidate::CandidateSettings;
use tore_session::host::content::GameContent;
use tore_session::host::{Present, ResumeNote, Resumption, hosting_answer, reach_packet};
use tore_session::standby::takeover::take_over_thread;
use tore_session::standby::{Builder, MissionKey, Note, StandbyThread};
use tore_session::wire::messages::Message;
use tore_session::{
    AfterEnd, BuildId, CrownRule, Host, HostConfig, HostLog, LeaveReason, OpenPlanes, Phase,
    StartMode,
};
use tore_world::mission::MissionSpec;
use tore_world::resources::ResourceReads;
use tore_world::world::{Seating, World};

/// A standby's status goes to the host this often (at most twice a second).
const STATUS_EVERY: Duration = Duration::from_millis(500);
/// An old host's game asks standby 1 whether it hosts now this often.
const ASK_EVERY: Duration = Duration::from_secs(1);
/// The host's figures and its world's go to the log this often.
const WORLD_EVERY: Duration = Duration::from_secs(1);
/// The standbys' figures go to the log this often.
const STANDBY_EVERY: Duration = Duration::from_secs(5);
/// How long a host that is stopping tells its players before it stops.
const STOP_GRACE: Duration = Duration::from_millis(1_500);
/// How long a handover's records may take to go through before the bot
/// leaves anyway.
const HANDOVER_WAIT: Duration = Duration::from_secs(3);
/// How long a host that took the game over waits, past its end, for its
/// guests to leave first.
pub const GUESTS_WAIT: Duration = Duration::from_secs(25);

/// The bot's socket: a joined game's, or a hosting game's.
pub enum Sock {
    Udp(UdpSocket),
    Server(ServerSocket),
}

impl Datagrams for Sock {
    fn send_datagram(&mut self, to: SocketAddr, datagram: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Udp(socket) => socket.send_datagram(to, datagram),
            Self::Server(socket) => socket.send_datagram(to, datagram),
        }
    }

    fn recv_datagram(&mut self, buf: &mut [u8]) -> std::io::Result<Option<(usize, SocketAddr)>> {
        match self {
            Self::Udp(socket) => socket.recv_datagram(buf),
            Self::Server(socket) => socket.recv_datagram(buf),
        }
    }
}

/// What a hosting bot is built from (`--host`).
pub struct HostSetup {
    pub spec: MissionSpec,
    pub port: u16,
    /// Appoint standbys (`--standby on`, the default for a host).
    pub standbys: bool,
    /// List the game on this master (`--master`, a loopback master in the
    /// battery), as a hosting game does.
    pub master: Option<String>,
}

/// What a listing of the game on a master says about its host.
fn rendezvous(build: &BuildId) -> HostRendezvous {
    HostRendezvous {
        build: Build {
            protocol_version: tore_session::wire::PROTOCOL_VERSION,
            game_version: build.version.clone(),
            game_commit: build.commit.clone(),
            release: build.release,
        },
        dedicated: false,
        install_id: None,
        platform: Platform::current().code(),
        entropy: Entropy::System,
    }
}

/// The settings of a game a bot hosts, as the game's hosting thread has them
/// (`tore-app`, `net/hosting.rs`): the bot's own connection is the house,
/// which wears the crown and starts each mission, a mission's end returns
/// everyone to the lobby at once, and a mission nobody flies any more ends.
pub fn host_config(
    name: &str,
    password: &str,
    build: BuildId,
    content: Arc<GameContent>,
) -> HostConfig {
    let mut config = HostConfig::new(build);
    config.name = name.to_owned();
    config.password = (!password.is_empty()).then(|| password.to_owned());
    config.open_planes = OpenPlanes::All;
    config.house = Some(LINK_ADDRESS);
    config.crown = CrownRule::FirstPlayer;
    config.start = StartMode::King;
    config.after_end = AfterEnd::Restart;
    config.restart_delay = Duration::ZERO;
    // Not zero as the game's: a host that took the game over has its players
    // absent for a moment, and with no timeout it would end the mission as
    // "everyone left" (reported to the lead, K9).
    config.empty_timeout = Duration::from_secs(20);
    config.entropy = Entropy::System;
    config.content = Some(content);
    config
}

/// An old host's game asking standby 1 whether it hosts now.
struct Asking {
    session: u64,
    nonce: u64,
    /// Standby 1's addresses, remembered when the host was lost.
    to: Vec<SocketAddr>,
    last: Duration,
}

/// How a hosting bot is ending.
enum Ending {
    /// The host said it left; it stops when `by` comes.
    Stopping { by: Duration },
    /// The host handed over; its records must go through by `by`. `leaving`:
    /// the bot is leaving the game (its time is up); else the lobby moved to
    /// a better host (slice KP) and the bot goes on as a player.
    Handing { by: Duration, leaving: bool },
}

/// The game a bot hosts.
pub struct Hosted {
    pub host: Host,
    /// When it took over and T, when it is not the original host.
    pub took: Option<(Duration, u64)>,
    ending: Option<Ending>,
    world_at: Duration,
    standby_at: Duration,
    /// The game's listing on the master, when it is listed; its part goes to
    /// the host whenever it changes (stage K, slice K8), so a standby holds
    /// it.
    listing: Option<HostListing>,
    part_version: u64,
}

/// A bot's migration state.
pub struct Mig {
    pub peers: Peers,
    pub standby: Option<StandbyThread>,
    /// The flight the client holds, which the standby builds from.
    flight: Arc<Mutex<Option<String>>>,
    flight_at: Duration,
    status_at: Option<Duration>,
    pub hosted: Option<Hosted>,
    asking: Option<Asking>,
    /// The old host stepped down: another game hosts.
    pub stepped_down: bool,
    /// The game was handed over and the bot is done.
    pub handed_over: bool,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    config: HostConfig,
    /// A standby that takes over hosts with standbys of its own.
    standbys: bool,
}

impl Mig {
    fn new(resources: &Arc<BTreeMap<String, Vec<u8>>>, config: HostConfig, standbys: bool) -> Self {
        Self {
            peers: Peers::new(tore_session::wire::PROTOCOL_VERSION, Entropy::System),
            standby: None,
            flight: Arc::new(Mutex::new(None)),
            flight_at: Duration::ZERO,
            status_at: None,
            hosted: None,
            asking: None,
            stepped_down: false,
            handed_over: false,
            resources: Arc::clone(resources),
            config,
            standbys,
        }
    }

    /// A hosting bot's: the host built from `setup`.
    pub fn hosting(
        setup: HostSetup,
        resources: &Arc<BTreeMap<String, Vec<u8>>>,
        config: HostConfig,
    ) -> Result<(Self, Sock), String> {
        let socket = ServerSocket::bind(tore_net::Listen::Any, setup.port).map_err(|error| {
            format!(
                "Cannot host on UDP port {}: {error}. Is another game or server using it?",
                setup.port
            )
        })?;
        let mut host = Host::new(setup.spec, Arc::clone(resources), config.clone())
            .map_err(|error| format!("The game could not be hosted: {error}"))?;
        host.set_standbys_enabled(setup.standbys);
        let listing = match &setup.master {
            Some(master) => {
                let mut listing = HostListing::new(
                    master,
                    rendezvous(&config.build),
                    setup.port,
                    Duration::ZERO,
                )?;
                listing.set_listed(true, Duration::ZERO);
                Some(listing)
            }
            None => None,
        };
        let mut mig = Self::new(resources, config, setup.standbys);
        mig.hosted = Some(Hosted::new(host, None, listing));
        Ok((mig, Sock::Server(socket)))
    }

    /// A joined bot's: a standby when `standby` is on, with the candidates
    /// of its socket.
    pub fn joined(
        resources: &Arc<BTreeMap<String, Vec<u8>>>,
        config: HostConfig,
        bot: &mut Bot,
        local: Option<SocketAddr>,
        standby: bool,
    ) -> Result<Self, String> {
        let mut mig = Self::new(resources, config, true);
        if standby {
            let builder = mig.builder();
            mig.standby = Some(StandbyThread::spawn(builder).map_err(|e| e.to_string())?);
            let mut candidates = Vec::new();
            if let Some(address) = local {
                candidates.push(tore_net::master::candidate::Candidate::new(
                    tore_net::master::candidate::CandidateKind::Local,
                    address,
                ));
            }
            bot.client.set_candidate(CandidateSettings {
                candidates,
                ..CandidateSettings::default()
            });
        } else {
            bot.client.set_candidate(CandidateSettings {
                may_host: false,
                ..CandidateSettings::default()
            });
        }
        Ok(mig)
    }

    /// The standby's builder: the flight the client holds.
    fn builder(&self) -> Builder {
        let flight = Arc::clone(&self.flight);
        let resources = Arc::clone(&self.resources);
        Box::new(move |key: &MissionKey| {
            let text = flight
                .lock()
                .map_err(|_| "the flight is lost".to_owned())?
                .clone()
                .ok_or_else(|| "no flight held".to_owned())?;
            if let Some(hash) = key.spec_hash
                && hash != tore_codec::fnv1a64(text.as_bytes())
            {
                return Err("another flight".into());
            }
            let spec = MissionSpec::from_text(&text).map_err(|e| e.to_string())?;
            World::new(&spec, &ResourceReads::new(&resources), Seating::Open)
                .map_err(|e| e.to_string())
        })
    }

    /// Whether this bot hosts the game it started: no takeover yet.
    fn original(&self) -> bool {
        self.hosted.as_ref().is_some_and(|h| h.took.is_none())
    }

    /// Whether this bot hosts now.
    pub fn hosts(&self) -> bool {
        self.hosted.is_some()
    }

    /// The remote players of the host, 0 when the bot does not host.
    pub fn guests(&self) -> usize {
        self.hosted.as_ref().map_or(0, |h| {
            h.host
                .players()
                .iter()
                .filter(|p| p.address != LINK_ADDRESS)
                .count()
        })
    }

    /// What the socket holds, through the listing when the bot's host has
    /// one (the master's datagrams never reach the host).
    fn drain(&mut self, now: Duration, sock: &mut Sock) -> Vec<(SocketAddr, Vec<u8>)> {
        let mut buf = [0u8; MAX_DATAGRAM + 1];
        let mut out = Vec::new();
        let mut take = |net: &mut dyn Datagrams| {
            while let Ok(Some((len, from))) = net.recv_datagram(&mut buf) {
                out.push((from, buf[..len].to_vec()));
            }
        };
        match self.hosted.as_mut().and_then(|h| h.listing.as_mut()) {
            Some(listing) => take(&mut listing.over(sock, now)),
            None => take(sock),
        }
        out
    }

    /// Takes in what the socket holds: the host's, the router's and the
    /// client's.
    pub fn receive(&mut self, now: Duration, name: &str, bot: &mut Bot, sock: &mut Sock) {
        for (from, datagram) in self.drain(now, sock) {
            let datagram = datagram.as_slice();
            if let Some(asking) = &self.asking
                && hosting_answer(datagram, asking.session, asking.nonce)
            {
                // Another game hosts now: this host steps down without a
                // word, and the bot's own client resumes with the new host
                // from the socket the host had.
                if let Some(mut hosted) = self.hosted.take() {
                    hosted.host.step_down();
                    hosted.print_notes(name);
                    // The listing goes on with the new host: no word to the
                    // master.
                    if let Some(listing) = hosted.listing.as_mut() {
                        listing.release();
                    }
                }
                self.asking = None;
                self.stepped_down = true;
                println!("{name}: migrate: {from} hosts the game now; this host stepped down");
                bot.client.move_to(now, &[from]);
                continue;
            }
            if self.original() {
                if let Some(hosted) = &mut self.hosted {
                    hosted.host.receive(now, from, datagram);
                }
                continue;
            }
            match self.peers.route(now, from, datagram) {
                Route::Host => {
                    if let Some(hosted) = &mut self.hosted {
                        hosted.host.receive(now, from, datagram);
                    }
                }
                Route::Client => bot.client.receive(now, from, datagram),
                Route::Taken => {}
            }
        }
    }

    /// The host's update, and what it sent.
    pub fn update_host(&mut self, now: Duration, name: &str, bot: &mut Bot, sock: &mut Sock) {
        let Some(hosted) = &mut self.hosted else {
            return;
        };
        hosted.host.update(now);
        hosted.update_listing(now, name);
        hosted.flush(now, bot, sock);
        hosted.print(now, name);
        // An old host that lost every player, or heard it was taken over,
        // asks standby 1 whether it hosts now, every second, at the
        // addresses it knew when it first asked.
        if hosted.took.is_none()
            && self.asking.is_none()
            && hosted.ending.is_none()
            && (hosted.host.lost_everyone() || hosted.host.moved_to().is_some())
        {
            println!(
                "{name}: migrate: the host lost its players; asking standby 1 whether it hosts now"
            );
            self.asking = Some(Asking {
                session: hosted.host.session(),
                nonce: 0x5eed_0077,
                to: hosted.host.standby_addresses(),
                last: now.saturating_sub(ASK_EVERY),
            });
        }
        if let Some(asking) = &mut self.asking
            && now.saturating_sub(asking.last) >= ASK_EVERY
        {
            asking.last = now;
            let reach = reach_packet(asking.session, asking.nonce, 0);
            for to in &asking.to {
                let _ = sock.send_datagram(*to, &reach);
            }
        }
    }

    /// The standby: the records the client kept go to its thread, its status
    /// goes to the host, its notes to the log; and the takeover when due.
    pub fn update_standby(
        &mut self,
        now: Duration,
        name: &str,
        bot: &mut Bot,
        sock: &Sock,
        resources: &Arc<BTreeMap<String, Vec<u8>>>,
    ) {
        if now.saturating_sub(self.flight_at) >= Duration::from_millis(250) {
            self.flight_at = now;
            if let Some(spec) = bot.client.spec()
                && let Ok(mut flight) = self.flight.lock()
            {
                let text = spec.to_text();
                if flight.as_deref() != Some(text.as_str()) {
                    *flight = Some(text);
                }
            }
        }
        let Some(standby) = &self.standby else {
            return;
        };
        standby.records(bot.client.take_standby_records());
        for note in standby.take_notes() {
            println!("{name}: standby: {}", note_text(&note));
        }
        if standby.appointed()
            && self
                .status_at
                .is_none_or(|at| now.saturating_sub(at) >= STATUS_EVERY)
        {
            self.status_at = Some(now);
            bot.client
                .request(now, Message::StandbyStatus(standby.status()));
        }
        if self.hosted.is_some()
            || !bot
                .client
                .takeover_due(standby.ready(), standby.handover().is_some())
        {
            return;
        }
        let Some(standby) = self.standby.take() else {
            return;
        };
        let Some(house) = bot.client.lobby().map(|l| l.you) else {
            return;
        };
        let present: Option<Present> = bot.client.present(now);
        println!("{name}: migrate: taking the game over");
        match take_over_thread(
            standby,
            Arc::clone(resources),
            self.config.clone(),
            Resumption {
                house,
                now,
                present,
            },
        ) {
            Ok(mut host) => {
                let tick = host.world().tick();
                host.set_standbys_enabled(self.standbys);
                // The old host's listing goes on from this socket (K8).
                let port = match sock {
                    Sock::Udp(socket) => socket.local_addr().map_or(0, |a| a.port()),
                    Sock::Server(socket) => {
                        socket.local_addresses().first().map_or(0, SocketAddr::port)
                    }
                };
                let listing = host
                    .listing_part()
                    .and_then(|bytes| ListingPart::decode(bytes).ok())
                    .and_then(|part| {
                        HostListing::resume(&part, rendezvous(&self.config.build), port, now)
                            .map_err(|error| println!("{name}: listing: cannot resume: {error}"))
                            .ok()
                    });
                if listing.is_some() {
                    println!("{name}: listing: resumed from the old host's part");
                }
                self.hosted = Some(Hosted::new(host, Some((now, tick)), listing));
                bot.client.host_here(now, LINK_ADDRESS, tick);
                self.peers.set_hosting(true, bot.client.old_host());
            }
            Err(error) => println!("{name}: migrate: cannot take the game over: {error}"),
        }
    }

    /// The peers router: hosting state, the reach work the client hands it,
    /// and what it sends.
    pub fn drive_peers(&mut self, now: Duration, bot: &mut Bot, sock: &mut Sock) {
        if self.original() {
            return;
        }
        if self.hosted.is_some() {
            self.peers.set_hosting(true, bot.client.old_host());
        }
        bot.client.drive_peers(now, &mut self.peers);
        self.peers.update(now);
        let _ = self.peers.transmit(sock);
    }

    /// What the client sent: to the host over the link, or to the network.
    pub fn send_client(&mut self, now: Duration, bot: &mut Bot, sock: &mut Sock) {
        while let Some(t) = bot.client.poll_transmit() {
            if t.to == LINK_ADDRESS {
                if let Some(hosted) = &mut self.hosted {
                    hosted.host.receive(now, LINK_ADDRESS, &t.datagram);
                }
            } else {
                let _ = sock.send_datagram(t.to, &t.datagram);
            }
        }
    }

    /// The bot's time is up and it hosts: the game is handed over to a ready
    /// standby, or the host leaves. A host that took the game over waits for
    /// its guests first. True once it has begun to end.
    pub fn end_hosting(
        &mut self,
        now: Duration,
        name: &str,
        end: Duration,
        hand_over: bool,
    ) -> bool {
        let guests = self.guests();
        let Some(hosted) = &mut self.hosted else {
            return false;
        };
        if hosted.ending.is_some() {
            return true;
        }
        if hosted.took.is_some() && guests > 0 && now < end + GUESTS_WAIT {
            return false;
        }
        if hand_over {
            match hosted.host.hand_over() {
                Ok(to) => {
                    println!("{name}: host: handing the game over to player {to}");
                    hosted.ending = Some(Ending::Handing {
                        by: now + HANDOVER_WAIT,
                        leaving: true,
                    });
                    return true;
                }
                Err(why) => println!("{name}: host: no handover: {why}"),
            }
        }
        println!("{name}: host: leaving; the host left the game");
        hosted.host.host_left();
        hosted.ending = Some(Ending::Stopping {
            by: now + STOP_GRACE,
        });
        true
    }

    /// Moves a hosting bot on through its ending: true once the host is done
    /// and gone.
    pub fn finish_hosting(
        &mut self,
        now: Duration,
        name: &str,
        bot: &mut Bot,
        sock: &mut Sock,
    ) -> bool {
        let Some(hosted) = &mut self.hosted else {
            return false;
        };
        match hosted.ending {
            Some(Ending::Handing { by, leaving }) => {
                if hosted.host.handed_over() || now >= by {
                    println!("{name}: host: the game was handed over");
                    // The listing goes on with the new host: no word to the
                    // master.
                    if let Some(listing) = hosted.listing.as_mut() {
                        listing.release();
                    }
                    self.hosted = None;
                    if leaving {
                        self.handed_over = true;
                        return true;
                    }
                    // A lobby that moved: the bot is a player of the new host
                    // now, which its client races as any player's does.
                    self.stepped_down = true;
                    return false;
                }
            }
            Some(Ending::Stopping { by }) => {
                if now >= by || hosted.host.phase() == Phase::Stopped {
                    hosted.host.stop();
                    hosted.host.update(now);
                    if let Some(listing) = hosted.listing.as_mut() {
                        listing.stop(now);
                    }
                    // The stop's disconnects go out before the host goes.
                    hosted.flush(now, bot, sock);
                    hosted.print(now, name);
                    self.hosted = None;
                    return true;
                }
            }
            None => {}
        }
        false
    }

    /// The migration's counts and the old host's progress, once a second.
    pub fn counts_line(&self, bot: &Bot) -> Option<String> {
        let counts = bot.client.migration_counts();
        (counts.resumed + counts.failed + counts.corrected > 0).then(|| {
            format!(
                "migrations resumed {}, failed {}, corrected {}",
                counts.resumed, counts.failed, counts.corrected
            )
        })
    }
}

impl Hosted {
    fn new(host: Host, took: Option<(Duration, u64)>, listing: Option<HostListing>) -> Self {
        let mut hosted = Self {
            host,
            took,
            ending: None,
            world_at: Duration::ZERO,
            standby_at: Duration::ZERO,
            listing,
            part_version: u64::MAX,
        };
        hosted.sync_part();
        hosted
    }

    /// The listing's part goes to the host whenever it may have changed.
    fn sync_part(&mut self) {
        if let Some(listing) = &self.listing
            && listing.part_version() != self.part_version
        {
            self.part_version = listing.part_version();
            self.host
                .set_listing_part(listing.part().map(|part| part.encode()));
        }
    }

    /// The listing's timers, its part and its events.
    fn update_listing(&mut self, now: Duration, name: &str) {
        let Some(listing) = self.listing.as_mut() else {
            return;
        };
        let host = &self.host;
        listing.update(now, || host.discover_answer(0).into());
        while let Some(event) = listing.poll_event() {
            println!("{name}: listing: {event:?}");
        }
        self.sync_part();
    }

    /// Sends what the host queued: to the bot's own client over the link,
    /// the rest to the network.
    fn flush(&mut self, now: Duration, bot: &mut Bot, sock: &mut Sock) {
        while let Some(t) = self.host.poll_transmit() {
            if t.to == LINK_ADDRESS {
                bot.client.receive(now, LINK_ADDRESS, &t.datagram);
            } else {
                // Through the listing: a relayed player's datagrams go to
                // its channel.
                let _ = match self.listing.as_mut() {
                    Some(listing) => listing.over(sock, now).send_datagram(t.to, &t.datagram),
                    None => sock.send_datagram(t.to, &t.datagram),
                };
            }
        }
        if let Some(listing) = self.listing.as_mut() {
            let _ = listing.transmit(sock);
        }
    }

    /// The host's log entries, notes and figures as lines.
    fn print(&mut self, now: Duration, name: &str) {
        while let Some(entry) = self.host.poll_log() {
            if let Some(line) = log_line(&entry) {
                println!("{name}: host: {line}");
            }
        }
        for note in self.print_notes(name) {
            // The lobby moved to a better host (the host's own succession
            // update handed over): stop hosting once its records are
            // through, as for a host that leaves.
            if let ResumeNote::HandedOver { .. } = note
                && self.ending.is_none()
            {
                self.ending = Some(Ending::Handing {
                    by: now + HANDOVER_WAIT,
                    leaving: false,
                });
            }
        }
        if now.saturating_sub(self.world_at) >= WORLD_EVERY && self.host.phase() == Phase::Flying {
            self.world_at = now;
            let world = self.host.world();
            let missiles = world
                .combat
                .state
                .projectiles
                .iter()
                .filter(|p| p.guidance.is_some())
                .count();
            let kills: u32 = tore_world::debrief::results(world)
                .iter()
                .map(|r| r.aircraft_kills)
                .sum();
            println!(
                "{name}: host: world: tick {}, {} missiles in flight, {} aircraft kills, {} players",
                world.tick(),
                missiles,
                kills,
                self.host.players().len()
            );
        }
        if now.saturating_sub(self.standby_at) >= STANDBY_EVERY {
            self.standby_at = now;
            for s in self.host.standby_figures() {
                println!(
                    "{name}: host: standby {} {:?}, {}, {}, checks {} equal {} differ, \
                     stream {} B",
                    s.callsign,
                    s.role,
                    if s.warm { "warm" } else { "cold" },
                    s.status
                        .map_or("no status yet".to_owned(), |s| format!("{:?}", s.state)),
                    s.checks_equal,
                    s.mismatches,
                    s.bytes.ticks + s.bytes.states + s.bytes.checks + s.bytes.checkpoints,
                );
            }
        }
    }

    fn print_notes(&mut self, name: &str) -> Vec<ResumeNote> {
        let notes = self.host.take_resume_notes();
        for note in &notes {
            println!("{name}: host: {}", resume_text(note));
        }
        notes
    }
}

/// A host log entry as a line, in the dedicated server's words; `None` for
/// the entries a bot has no use for.
fn log_line(entry: &HostLog) -> Option<String> {
    let who = |address: &SocketAddr| {
        if *address == LINK_ADDRESS {
            "this game".to_owned()
        } else {
            address.to_string()
        }
    };
    Some(match entry {
        HostLog::Connected {
            address, callsign, ..
        } => format!("{} joined as {callsign}", who(address)),
        HostLog::Refused {
            address, reason, ..
        } => format!("{} refused: {reason}", who(address)),
        HostLog::Seated {
            seat,
            callsign,
            plane,
            ..
        } => format!("seat {seat} {callsign} took plane {plane}"),
        HostLog::Left {
            callsign,
            plane,
            reason,
            ..
        } => {
            let plane = plane.map_or_else(String::new, |plane| format!(" (plane {plane})"));
            let why = match reason {
                LeaveReason::Left => "left".to_owned(),
                LeaveReason::Silent => "no packet for 5 seconds".to_owned(),
                LeaveReason::Kicked => "kicked".to_owned(),
                LeaveReason::MissionEnded => "the mission ended".to_owned(),
                LeaveReason::HostLeft => "the host left the game".to_owned(),
                LeaveReason::Replaced => "replaced by a new connection".to_owned(),
                LeaveReason::Disconnected(why) => format!("disconnected ({why:?})"),
            };
            format!("{callsign}{plane} left: {why}")
        }
        HostLog::MissionStarted { tick } => format!("mission started at tick {tick}"),
        HostLog::MissionEnded { reason, .. } => format!("mission ended: {reason:?}"),
        HostLog::Overloaded { ticks_behind, .. } => {
            format!("overloaded: {ticks_behind} ticks behind real time")
        }
        HostLog::Fault { text, .. } => format!("fault: {text}"),
        HostLog::Stopped { .. } => "stopped".to_owned(),
        _ => return None,
    })
}

/// A resume note as a line.
fn resume_text(note: &ResumeNote) -> String {
    match note {
        ResumeNote::TookOver {
            tick,
            replayed,
            took,
            absent,
        } => format!(
            "took the game over at tick {tick}: replayed {replayed} ticks in {} ms, {absent} \
             players expected back",
            took.as_millis()
        ),
        ResumeNote::Resumed {
            callsign,
            flying,
            after,
        } => format!(
            "{callsign} resumed {} ms after the takeover{}",
            after.as_millis(),
            if *flying { ", flying" } else { "" }
        ),
        ResumeNote::Live {
            tick,
            after,
            fast_forward,
        } => format!(
            "live at tick {tick}, {} ms after the takeover, {fast_forward} ticks fast-forwarded",
            after.as_millis()
        ),
        ResumeNote::Dropped { callsign } => {
            format!("{callsign} never resumed: dropped, its plane kept for it")
        }
        ResumeNote::TakenOver { by, tick } => {
            format!("player {by} took the game over at tick {tick}")
        }
        ResumeNote::HandedOver { to, last_tick } => {
            format!("handed the game over to player {to} after tick {last_tick}")
        }
        ResumeNote::SteppedDown => "stepped down: another game hosts".to_owned(),
    }
}

/// A standby's note as a line.
fn note_text(note: &Note) -> String {
    match note {
        Note::Appointed { warm } => format!("appointed, {}", if *warm { "warm" } else { "cold" }),
        Note::Dismissed => "dismissed".to_owned(),
        other => format!("{other:?}").to_lowercase(),
    }
}
