//! When a standby's game takes the game over, and how (slice K4;
//! docs/ARCHITECTURE.md, "Losing the host"): the timing rule ([`due`]) and
//! the step from a standby's [`Takeover`] to the new host
//! ([`crate::Host::resume`]).
//!
//! - **Standby 1** takes over after [`DETECT_SILENCE`] without a packet from
//!   the host, or at once when the host hands over to it. While the host's
//!   keepalives say it is building a mission (protocol 28, slice NET2) the
//!   silence is [`LOADING_SILENCE`] instead.
//! - **Standby 2** waits [`SECOND_STANDBY_WAIT`] more, racing standby 1 as a
//!   client meanwhile, and takes over only if standby 1 has not answered, so
//!   a second host never starts beside a live first.
//! - A standby that is not ready never takes over; it joins whoever does.
//!
//! The game's client says how long the host has been silent and whether a
//! race has found a new host ([`crate::Client::takeover_due`]); the game
//! (slice K7a) or `tore-bot` (slice K9) then takes its standby's world and
//! parts and builds the host with [`take_over`] or [`take_over_thread`].

use super::{Standby, StandbyThread, Takeover};
use crate::host::{Host, HostConfig, Resumption};
use crate::wire::messages::StandbyMark;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

/// Silence from the host after which a client races the succession and
/// standby 1 takes over (John, 2026-10-05).
pub const DETECT_SILENCE: Duration = Duration::from_millis(1_500);
/// Silence from a host whose last word was that it is building a mission
/// (its loading keepalive, protocol 28) after which a client counts it lost:
/// 4 seconds (agent decision, NET2). The host's loading thread speaks every
/// half second, so this is eight missed keepalives: a host that died in its
/// build, or one whose build ran past the loading bound (3 minutes), is
/// still replaced, before the transport's 5 seconds end the connection.
pub const LOADING_SILENCE: Duration = Duration::from_secs(4);

/// The silence after which a client counts its host lost: [`LOADING_SILENCE`]
/// while the host says it is loading, else [`DETECT_SILENCE`].
pub fn detect_silence(loading: bool) -> Duration {
    if loading {
        LOADING_SILENCE
    } else {
        DETECT_SILENCE
    }
}

/// How much longer standby 2 waits for standby 1 to answer before it takes
/// over itself.
pub const SECOND_STANDBY_WAIT: Duration = Duration::from_secs(3);
/// A client that has found no new host this long after it lost the old one
/// gives the session up: "No other game could take over."
pub const GIVE_UP: Duration = Duration::from_secs(15);

/// Whether a standby in `role` takes the game over now: `silence` since the
/// host's last packet, `loading` while the host's keepalives say it is
/// building a mission ([`detect_silence`]), `handover` when the host handed over to it,
/// `answered` when its client's race has found a new host already, `ready`
/// when it holds a world the journal continues from.
pub fn due(
    role: StandbyMark,
    silence: Duration,
    loading: bool,
    handover: bool,
    answered: bool,
    ready: bool,
) -> bool {
    if !ready || answered {
        return false;
    }
    let detect = detect_silence(loading);
    match role {
        StandbyMark::First => handover || silence >= detect,
        StandbyMark::Second => handover || silence >= detect + SECOND_STANDBY_WAIT,
        StandbyMark::None => false,
    }
}

/// The new host from a standby's takeover, with the hosting game's own
/// `config` (its house the in-process link) and `resumption`.
pub fn host_from(
    takeover: Takeover,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    config: HostConfig,
    resumption: Resumption,
) -> Result<Host, String> {
    Host::resume(takeover, resources, config, resumption)
}

/// Takes over with a standby run in process: it replays what it holds to
/// its end, and the new host is built from its world and parts.
pub fn take_over(
    standby: Standby,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    config: HostConfig,
    resumption: Resumption,
) -> Result<Host, String> {
    host_from(standby.take_over()?, resources, config, resumption)
}

/// Takes over with a standby on its worker thread, as the game and
/// `tore-bot` run it.
pub fn take_over_thread(
    thread: StandbyThread,
    resources: Arc<BTreeMap<String, Vec<u8>>>,
    config: HostConfig,
    resumption: Resumption,
) -> Result<Host, String> {
    host_from(thread.take_over()?, resources, config, resumption)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MS: Duration = Duration::from_millis(1);

    #[test]
    fn standby_one_takes_over_after_the_silence_and_standby_two_three_seconds_later() {
        let first = StandbyMark::First;
        let second = StandbyMark::Second;
        assert!(!due(first, 1_499 * MS, false, false, false, true));
        assert!(due(first, 1_500 * MS, false, false, false, true));
        assert!(!due(second, 1_500 * MS, false, false, false, true));
        assert!(!due(second, 4_499 * MS, false, false, false, true));
        assert!(due(second, 4_500 * MS, false, false, false, true));
        // Standby 1 answered the race: standby 2 joins it.
        assert!(!due(second, 9_000 * MS, false, false, true, true));
        // A handover is at once; a standby not ready never takes over.
        assert!(due(first, Duration::ZERO, false, true, false, true));
        assert!(!due(first, 9_000 * MS, false, true, false, false));
        assert!(!due(
            StandbyMark::None,
            9_000 * MS,
            false,
            true,
            false,
            true
        ));
    }

    /// While the host says it is loading, standby 1 waits 4 seconds of
    /// silence and standby 2 three more (protocol 28, slice NET2).
    #[test]
    fn a_loading_host_is_given_four_seconds_of_silence() {
        let first = StandbyMark::First;
        let second = StandbyMark::Second;
        assert!(!due(first, 3_999 * MS, true, false, false, true));
        assert!(due(first, 4_000 * MS, true, false, false, true));
        assert!(!due(second, 6_999 * MS, true, false, false, true));
        assert!(due(second, 7_000 * MS, true, false, false, true));
        // A handover is at once, loading or not.
        assert!(due(first, Duration::ZERO, true, true, false, true));
    }
}
