//! Browse pages and details ("Browsing" in the master protocol).
//!
//! A Browse asks for a page of the listings that match the asker's build and
//! filters, in the master's order: games in the lobby first, then flying,
//! then closed, then by players (most first), then by name, then by listing
//! id so the order is total. The master counts at most 200 matches. The
//! cursor is how many matching entries earlier pages gave (agent decision):
//! 0 for the first page, then the Page's next cursor, 0 again on the last
//! page. A list that changes between pages can repeat or skip a game; the
//! Internet Lobby asks again every 15 seconds.

use tore_net::master::{Browse, Build, Page, PageEntry, candidate::relay_likely};
use tore_net::packet::DiscoverPhase;

use crate::listings::{Listing, Listings};
use tore_net::master::MAX_BROWSE_MATCHES;

/// True when an asker of build `asker` can join a host of build `host`: the
/// same protocol version, and the game's own build rule (the same commit, or
/// a tagged release host of the same plain version; the rule in
/// `tore_session`'s `BuildId::matches`).
pub fn same_build(host: &Build, asker: &Build) -> bool {
    if host.protocol_version != asker.protocol_version {
        return false;
    }
    if host.game_commit == asker.game_commit && host.game_commit != "unknown" {
        return true;
    }
    host.release && asker.game_version == host.game_version && !asker.game_version.contains('-')
}

fn phase_rank(phase: DiscoverPhase) -> u8 {
    match phase {
        DiscoverPhase::Lobby => 0,
        DiscoverPhase::Flying => 1,
        DiscoverPhase::Closed => 2,
    }
}

/// The page a Browse gets, fitted to `max_len` bytes (the Browse's length).
pub fn page(listings: &Listings, browse: &Browse, max_len: usize) -> Page {
    let mut matches: Vec<(&Listing, bool)> = listings
        .iter()
        .filter_map(|listing| {
            let other = !same_build(&listing.build, &browse.build);
            let wanted =
                (!other || browse.other_builds) && (!listing.summary.full || browse.full_games);
            wanted.then_some((listing, other))
        })
        .collect();
    matches.sort_by(|(a, _), (b, _)| {
        phase_rank(a.summary.phase)
            .cmp(&phase_rank(b.summary.phase))
            .then(b.summary.players.cmp(&a.summary.players))
            .then(a.summary.name.cmp(&b.summary.name))
            .then(a.id.cmp(&b.id))
    });
    matches.truncate(usize::from(MAX_BROWSE_MATCHES));
    let matching = matches.len();
    let start = (browse.cursor as usize).min(matching);
    let entries = matches[start..]
        .iter()
        .map(|(listing, other)| entry(listing, *other))
        .collect();
    let mut page = Page {
        nonce: browse.nonce,
        matching: matching as u16,
        next_cursor: 0,
        entries,
    }
    .fit(max_len);
    let next = start + page.entries.len();
    if next < matching {
        page.next_cursor = next as u32;
    }
    page
}

/// One listing as a Page shows it.
fn entry(listing: &Listing, other_build: bool) -> PageEntry {
    let summary = &listing.summary;
    PageEntry {
        listing_id: listing.id,
        password: summary.password,
        full: summary.full,
        dedicated: listing.dedicated,
        other_build: other_build.then(|| summary.game_version.clone()),
        relay_likely: relay_likely(listing.mapping, &listing.candidates),
        phase: summary.phase,
        players: summary.players,
        capacity: summary.capacity,
        // A code a Page cannot carry (a later build's system) shows as unknown.
        platform: if listing.platform <= tore_net::master::packet::MAX_PAGE_PLATFORM {
            listing.platform
        } else {
            0
        },
        name: summary.name.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build(version: &str, commit: &str, release: bool) -> Build {
        Build {
            protocol_version: 7,
            game_version: version.into(),
            game_commit: commit.into(),
            release,
        }
    }

    #[test]
    fn the_build_rule_is_the_games() {
        let release = build("0.1.3", "abc", true);
        assert!(same_build(&release, &build("0.1.3", "abc", true)));
        assert!(same_build(&release, &build("0.1.3", "def", true)));
        assert!(!same_build(&release, &build("0.1.2", "def", true)));
        assert!(!same_build(&release, &build("0.1.3-4-gdef", "def", false)));
        let dev = build("0.1.3-4-gabc", "abc", false);
        assert!(same_build(&dev, &build("0.1.3-4-gabc", "abc", false)));
        assert!(!same_build(&dev, &build("0.1.3", "def", true)));
        let unknown = build("0.1.3", "unknown", false);
        assert!(!same_build(&unknown, &build("0.1.3", "unknown", false)));
        let mut other_protocol = build("0.1.3", "abc", true);
        other_protocol.protocol_version = 8;
        assert!(!same_build(&other_protocol, &release));
    }
}
