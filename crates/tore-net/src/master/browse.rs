//! The browse client: what the Internet Lobby asks the master ("Browsing" in
//! the master protocol).
//!
//! [`Browser`] is a state machine like the transport's endpoints: it never
//! touches a socket or reads a clock. The caller looks the master up, gives
//! it the master's addresses, asks it to [`Browser::refresh`] the list or
//! [`Browser::ask_details`] of one game, feeds it the datagrams its socket
//! receives, calls [`Browser::update`] for its retries, sends what
//! [`Browser::poll_transmit`] gives and handles [`Browser::poll_event`]: the
//! games added, changed and dropped since the last list, a game's details,
//! a master that does not answer.
//!
//! A refresh asks for every page in turn (the Page's next cursor, at most
//! [`MAX_PAGES`]), then compares the whole list with the one before. Each
//! request is padded to its fixed length, so the master may answer it with
//! as many bytes; a request with no answer is sent again after
//! [`BROWSE_RETRY`], [`BROWSE_TRIES`] times in all, then the next of the
//! master's addresses is tried and [`BrowseEvent::Silent`] says so.

use std::collections::VecDeque;
use std::io;
use std::net::SocketAddr;
use std::time::Duration;

use super::candidate::canonical;
use super::packet::{
    Browse, Build, Details, ListingSummary, MAX_MASTER_DATAGRAM, MasterPacket, PageEntry,
};
use crate::datagram::{Datagrams, Transmit};
use crate::entropy::{Entropy, Rng};

/// A request with no answer is sent again after this long.
pub const BROWSE_RETRY: Duration = Duration::from_secs(1);
/// How many times a request is sent before the master counts as silent.
pub const BROWSE_TRIES: u32 = 3;
/// The most pages one refresh asks for (the master stops at 200 matches,
/// about 10 to 20 pages).
pub const MAX_PAGES: u32 = 32;
/// Datagrams one [`Browser::receive_from`] reads at most.
const MAX_READ: usize = 256;

/// What the browser asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserConfig {
    /// This game's build: the master lists games of the same build unless
    /// `other_builds` asks for the rest.
    pub build: Build,
    /// Include games of other builds (marked in each entry).
    pub other_builds: bool,
    /// Include full games.
    pub full_games: bool,
    /// Where the nonces come from: [`Entropy::System`] on a real network.
    pub entropy: Entropy,
}

/// What happened.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BrowseEvent {
    /// A game that was not in the list before.
    Added(PageEntry),
    /// A game in the list before whose entry changed.
    Changed(PageEntry),
    /// A game that is no longer listed (its id).
    Dropped(u64),
    /// A refresh finished: how many games matched (the master counts at most
    /// 200) and how many the list holds.
    Refreshed {
        /// Matching listings, as the master counted them.
        matching: u16,
        /// Games in the list.
        shown: usize,
    },
    /// The details of one game; `None` when the master no longer lists it.
    Details {
        /// The listing.
        listing_id: u64,
        /// Its summary.
        summary: Option<ListingSummary>,
    },
    /// The master did not answer a request [`BROWSE_TRIES`] times.
    Silent,
    /// The master does not speak this game's master protocol version; its
    /// text says what to do.
    Unsupported {
        /// The master's text for the player.
        text: String,
    },
}

#[derive(Debug, Clone)]
struct Asked {
    nonce: u64,
    sent: Duration,
    tries: u32,
}

#[derive(Debug, Clone)]
struct Refresh {
    asked: Asked,
    cursor: u32,
    pages: u32,
    collected: Vec<PageEntry>,
}

#[derive(Debug, Clone)]
struct DetailsAsked {
    asked: Asked,
    listing_id: u64,
}

/// The Internet Lobby's side of browsing.
#[derive(Debug, Clone)]
pub struct Browser {
    config: BrowserConfig,
    rng: Rng,
    masters: Vec<SocketAddr>,
    current: usize,
    games: Vec<PageEntry>,
    matching: u16,
    refresh: Option<Refresh>,
    details: Option<DetailsAsked>,
    out: VecDeque<Transmit>,
    events: VecDeque<BrowseEvent>,
}

impl Browser {
    /// A browser for the master at `masters` (its addresses, any family;
    /// empty until the name is looked up).
    pub fn new(config: BrowserConfig, masters: Vec<SocketAddr>) -> Self {
        Self {
            rng: Rng::new(config.entropy),
            config,
            masters: masters.into_iter().map(canonical).collect(),
            current: 0,
            games: Vec::new(),
            matching: 0,
            refresh: None,
            details: None,
            out: VecDeque::new(),
            events: VecDeque::new(),
        }
    }

    /// The master's addresses changed (looked up again).
    pub fn set_masters(&mut self, masters: Vec<SocketAddr>) {
        self.masters = masters.into_iter().map(canonical).collect();
        self.current = 0;
    }

    /// Changes the filters; the next refresh uses them.
    pub fn set_filters(&mut self, other_builds: bool, full_games: bool) {
        self.config.other_builds = other_builds;
        self.config.full_games = full_games;
    }

    /// The list from the last refresh, in the master's order.
    pub fn games(&self) -> &[PageEntry] {
        &self.games
    }

    /// How many listings matched at the last refresh.
    pub fn matching(&self) -> u16 {
        self.matching
    }

    /// True while a refresh is under way.
    pub fn refreshing(&self) -> bool {
        self.refresh.is_some()
    }

    /// Asks for the whole list again, from the first page. A refresh under
    /// way starts over.
    pub fn refresh(&mut self, now: Duration) {
        let nonce = self.rng.next_u64();
        self.refresh = Some(Refresh {
            asked: Asked {
                nonce,
                sent: now,
                tries: 0,
            },
            cursor: 0,
            pages: 0,
            collected: Vec::new(),
        });
        self.send_browse(now);
    }

    /// Asks for one game's details. One question at a time: a new one
    /// replaces the one under way.
    pub fn ask_details(&mut self, now: Duration, listing_id: u64) {
        let nonce = self.rng.next_u64();
        self.details = Some(DetailsAsked {
            asked: Asked {
                nonce,
                sent: now,
                tries: 0,
            },
            listing_id,
        });
        self.send_details(now);
    }

    fn master(&self) -> Option<SocketAddr> {
        self.masters.get(self.current).copied()
    }

    fn send_browse(&mut self, now: Duration) {
        let Some(to) = self.master() else { return };
        let Some(refresh) = &mut self.refresh else {
            return;
        };
        refresh.asked.sent = now;
        refresh.asked.tries += 1;
        let browse = MasterPacket::Browse(Browse {
            nonce: refresh.asked.nonce,
            build: self.config.build.clone(),
            other_builds: self.config.other_builds,
            full_games: self.config.full_games,
            cursor: refresh.cursor,
        });
        if let Ok(datagram) = browse.encode() {
            self.out.push_back(Transmit { to, datagram });
        }
    }

    fn send_details(&mut self, now: Duration) {
        let Some(to) = self.master() else { return };
        let Some(details) = &mut self.details else {
            return;
        };
        details.asked.sent = now;
        details.asked.tries += 1;
        let request = MasterPacket::Details(Details {
            nonce: details.asked.nonce,
            listing_id: details.listing_id,
        });
        if let Ok(datagram) = request.encode() {
            self.out.push_back(Transmit { to, datagram });
        }
    }

    /// Retries, and a master that does not answer.
    pub fn update(&mut self, now: Duration) {
        let mut silent = false;
        if let Some(refresh) = &self.refresh
            && now.saturating_sub(refresh.asked.sent) >= BROWSE_RETRY
        {
            if refresh.asked.tries >= BROWSE_TRIES || self.masters.is_empty() {
                self.refresh = None;
                silent = true;
            } else {
                self.send_browse(now);
            }
        }
        if let Some(details) = &self.details
            && now.saturating_sub(details.asked.sent) >= BROWSE_RETRY
        {
            if details.asked.tries >= BROWSE_TRIES || self.masters.is_empty() {
                self.details = None;
                silent = true;
            } else {
                self.send_details(now);
            }
        }
        if silent {
            if !self.masters.is_empty() {
                self.current = (self.current + 1) % self.masters.len();
            }
            self.events.push_back(BrowseEvent::Silent);
        }
    }

    /// One datagram the browser's socket received. Returns false for one
    /// that is not an answer to it (another sender, an old nonce, another
    /// program's), which is dropped.
    pub fn receive(&mut self, now: Duration, from: SocketAddr, datagram: &[u8]) -> bool {
        if !self.masters.contains(&canonical(from)) {
            return false;
        }
        match MasterPacket::decode(datagram) {
            Ok(MasterPacket::Page(page)) => {
                let Some(refresh) = &mut self.refresh else {
                    return false;
                };
                if page.nonce != refresh.asked.nonce {
                    return false;
                }
                refresh.pages += 1;
                refresh.collected.extend(page.entries);
                let more = page.next_cursor > refresh.cursor && refresh.pages < MAX_PAGES;
                if more {
                    refresh.cursor = page.next_cursor;
                    refresh.asked.nonce = self.rng.next_u64();
                    refresh.asked.tries = 0;
                    self.send_browse(now);
                } else {
                    let refresh = self.refresh.take().expect("a refresh is under way");
                    self.finish(page.matching, refresh.collected);
                }
                true
            }
            Ok(MasterPacket::ListingDetails(answer)) => {
                let Some(details) = &self.details else {
                    return false;
                };
                if answer.nonce != details.asked.nonce || answer.listing_id != details.listing_id {
                    return false;
                }
                self.details = None;
                self.events.push_back(BrowseEvent::Details {
                    listing_id: answer.listing_id,
                    summary: answer.summary,
                });
                true
            }
            Ok(MasterPacket::Unsupported(unsupported)) => {
                if self.refresh.is_none() && self.details.is_none() {
                    return false;
                }
                self.refresh = None;
                self.details = None;
                self.events.push_back(BrowseEvent::Unsupported {
                    text: unsupported.text,
                });
                true
            }
            _ => false,
        }
    }

    /// A finished refresh: the new list, and what changed.
    fn finish(&mut self, matching: u16, collected: Vec<PageEntry>) {
        let mut list: Vec<PageEntry> = Vec::with_capacity(collected.len());
        for entry in collected {
            if !list.iter().any(|e| e.listing_id == entry.listing_id) {
                list.push(entry);
            }
        }
        for old in &self.games {
            if !list.iter().any(|e| e.listing_id == old.listing_id) {
                self.events.push_back(BrowseEvent::Dropped(old.listing_id));
            }
        }
        for entry in &list {
            match self.games.iter().find(|e| e.listing_id == entry.listing_id) {
                None => self.events.push_back(BrowseEvent::Added(entry.clone())),
                Some(old) if old != entry => {
                    self.events.push_back(BrowseEvent::Changed(entry.clone()))
                }
                Some(_) => {}
            }
        }
        self.matching = matching;
        self.games = list;
        self.events.push_back(BrowseEvent::Refreshed {
            matching,
            shown: self.games.len(),
        });
    }

    /// The next datagram to send.
    pub fn poll_transmit(&mut self) -> Option<Transmit> {
        self.out.pop_front()
    }

    /// The next event.
    pub fn poll_event(&mut self) -> Option<BrowseEvent> {
        self.events.pop_front()
    }

    /// Reads the waiting datagrams from `socket` (at most 256).
    pub fn receive_from<D: Datagrams + ?Sized>(
        &mut self,
        socket: &mut D,
        now: Duration,
    ) -> io::Result<usize> {
        let mut buf = [0u8; MAX_MASTER_DATAGRAM + 1];
        let mut read = 0;
        while read < MAX_READ {
            let Some((len, from)) = socket.recv_datagram(&mut buf)? else {
                break;
            };
            read += 1;
            self.receive(now, from, &buf[..len]);
        }
        Ok(read)
    }

    /// Sends what is queued.
    pub fn transmit<D: Datagrams + ?Sized>(&mut self, socket: &mut D) -> io::Result<()> {
        crate::datagram::transmit_all(&mut self.out, socket)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::master::packet::{ListingDetails, Page, Unsupported};
    use crate::packet::DiscoverPhase;

    fn master() -> SocketAddr {
        "198.51.100.1:26901".parse().unwrap()
    }

    fn browser() -> Browser {
        Browser::new(
            BrowserConfig {
                build: Build::default(),
                other_builds: false,
                full_games: true,
                entropy: Entropy::Seeded(4),
            },
            vec![master()],
        )
    }

    fn entry(id: u64, players: u8) -> PageEntry {
        PageEntry {
            listing_id: id,
            password: false,
            full: false,
            dedicated: false,
            other_build: None,
            relay_likely: false,
            phase: DiscoverPhase::Lobby,
            players,
            capacity: 8,
            platform: 3,
            name: format!("game {id}"),
        }
    }

    /// The Browse the browser sent last.
    fn sent_browse(b: &mut Browser) -> Browse {
        let t = b.poll_transmit().expect("a request");
        assert_eq!(t.to, master());
        assert_eq!(t.datagram.len(), 1_200);
        match MasterPacket::decode(&t.datagram).unwrap() {
            MasterPacket::Browse(browse) => browse,
            other => panic!("{other:?}"),
        }
    }

    fn page(nonce: u64, next_cursor: u32, entries: Vec<PageEntry>) -> Vec<u8> {
        MasterPacket::Page(Page {
            nonce,
            matching: 3,
            next_cursor,
            entries,
        })
        .encode()
        .unwrap()
    }

    fn events(b: &mut Browser) -> Vec<BrowseEvent> {
        std::iter::from_fn(|| b.poll_event()).collect()
    }

    #[test]
    fn a_refresh_reads_every_page_and_reports_the_changes() {
        let mut b = browser();
        let t = Duration::from_secs(1);
        b.refresh(t);
        let first = sent_browse(&mut b);
        assert_eq!(first.cursor, 0);
        assert!(b.receive(
            t,
            master(),
            &page(first.nonce, 2, vec![entry(1, 3), entry(2, 1)])
        ));
        let second = sent_browse(&mut b);
        assert_eq!(second.cursor, 2);
        assert_ne!(second.nonce, first.nonce);
        // A late copy of the first page is no longer an answer.
        assert!(!b.receive(t, master(), &page(first.nonce, 2, vec![entry(9, 1)])));
        assert!(b.receive(t, master(), &page(second.nonce, 0, vec![entry(3, 0)])));
        assert_eq!(
            events(&mut b),
            [
                BrowseEvent::Added(entry(1, 3)),
                BrowseEvent::Added(entry(2, 1)),
                BrowseEvent::Added(entry(3, 0)),
                BrowseEvent::Refreshed {
                    matching: 3,
                    shown: 3
                },
            ]
        );
        // The next refresh: one changed, one gone.
        b.refresh(t);
        let again = sent_browse(&mut b);
        assert!(b.receive(
            t,
            master(),
            &page(again.nonce, 0, vec![entry(1, 4), entry(3, 0)])
        ));
        assert_eq!(
            events(&mut b),
            [
                BrowseEvent::Dropped(2),
                BrowseEvent::Changed(entry(1, 4)),
                BrowseEvent::Refreshed {
                    matching: 3,
                    shown: 2
                },
            ]
        );
        assert_eq!(b.games().len(), 2);
    }

    #[test]
    fn only_the_master_is_heard() {
        let mut b = browser();
        b.refresh(Duration::ZERO);
        let browse = sent_browse(&mut b);
        let stranger: SocketAddr = "203.0.113.9:26901".parse().unwrap();
        assert!(!b.receive(Duration::ZERO, stranger, &page(browse.nonce, 0, vec![])));
        // The master's IPv4 address as a dual-stack socket reports it.
        let mapped: SocketAddr = "[::ffff:198.51.100.1]:26901".parse().unwrap();
        assert!(b.receive(Duration::ZERO, mapped, &page(browse.nonce, 0, vec![])));
    }

    #[test]
    fn a_silent_master_is_asked_three_times_then_reported() {
        let mut b = browser();
        b.refresh(Duration::ZERO);
        let first = sent_browse(&mut b);
        b.update(Duration::from_millis(999));
        assert!(b.poll_transmit().is_none());
        b.update(Duration::from_secs(1));
        assert_eq!(sent_browse(&mut b).nonce, first.nonce);
        b.update(Duration::from_secs(2));
        sent_browse(&mut b);
        b.update(Duration::from_secs(3));
        assert!(b.poll_transmit().is_none());
        assert_eq!(events(&mut b), [BrowseEvent::Silent]);
        assert!(!b.refreshing());
    }

    #[test]
    fn details_and_unsupported() {
        let mut b = browser();
        b.ask_details(Duration::ZERO, 77);
        let t = b.poll_transmit().unwrap();
        assert_eq!(t.datagram.len(), 1_000);
        let MasterPacket::Details(details) = MasterPacket::decode(&t.datagram).unwrap() else {
            panic!()
        };
        let answer = MasterPacket::ListingDetails(ListingDetails {
            nonce: details.nonce,
            listing_id: 77,
            summary: None,
        })
        .encode()
        .unwrap();
        assert!(b.receive(Duration::ZERO, master(), &answer));
        assert_eq!(
            events(&mut b),
            [BrowseEvent::Details {
                listing_id: 77,
                summary: None
            }]
        );
        b.refresh(Duration::ZERO);
        let unsupported = MasterPacket::Unsupported(Unsupported {
            lowest: 2,
            highest: 2,
            text: "Update the game.".into(),
        })
        .encode_in(1)
        .unwrap();
        assert!(b.receive(Duration::ZERO, master(), &unsupported));
        assert_eq!(
            events(&mut b),
            [BrowseEvent::Unsupported {
                text: "Update the game.".into()
            }]
        );
    }
}
