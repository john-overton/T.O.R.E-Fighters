//! Death and revival in the player's game (stage F phase 2, slice F2-V;
//! docs/ARCHITECTURE.md, "Death, revival and lives"): the newest Revival the
//! host sent since the player's plane was lost, counted down from its
//! arrival, the words a flight shows for it, and each revival's new plane
//! added to the client's own copy of the mission from the Spawned message
//! ([`tore_world::world::World::add_plane`]), so a Seated message for it
//! finds its aircraft and its stores.

use super::{Client, ClientEvent};
use crate::settings::Respawn;
use crate::wire::messages::{Revival, Spawned};
use std::time::Duration;
use tore_world::seats::{PlaneId, Slot};
use tore_world::world::revive::NewPlane;

/// The host's words for a game with no revival, no lives left, and no room
/// yet (`host::revive`'s).
const NO_REVIVAL: &str = "No revival in this game.";
const NO_LIVES: &str = "No lives left.";
const NO_ROOM: &str = "Waiting for room for another aircraft.";

/// The newest Revival, and when it arrived.
#[derive(Clone, Debug, PartialEq)]
pub struct Kept {
    pub revival: Revival,
    pub received: Duration,
}

impl Kept {
    /// Whole seconds until the player may fly again at `now`, rounded up.
    pub fn seconds_left(&self, now: Duration) -> u32 {
        let waited = now.saturating_sub(self.received);
        let left = Duration::from_secs(u64::from(self.revival.wait_seconds)).saturating_sub(waited);
        u32::try_from(left.as_secs() + u64::from(left.subsec_nanos() > 0)).unwrap_or(u32::MAX)
    }
}

/// What the flight's HUD says while the plane is lost, `seconds_left`
/// before it may fly again: "Press Enter to fly again (2 lives left)", "You
/// can fly again in 0:45", "No lives left. Esc, then Watch, shows the
/// battle." and the like.
pub fn prompt(revival: &Revival, seconds_left: u32) -> String {
    const WATCH: &str = "Esc, then Watch, shows the battle.";
    if revival.rule == Respawn::None {
        return format!("{NO_REVIVAL} {WATCH}");
    }
    match revival.why.as_deref() {
        Some(NO_LIVES) => return format!("{NO_LIVES} {WATCH}"),
        Some(NO_ROOM) => return NO_ROOM.to_owned(),
        _ => {}
    }
    if seconds_left > 0 {
        return format!(
            "You can fly again in {}:{:02}",
            seconds_left / 60,
            seconds_left % 60
        );
    }
    match revival.lives {
        None => "Press Enter to fly again".to_owned(),
        Some(1) => "Press Enter to fly again (1 life left)".to_owned(),
        Some(lives) => format!("Press Enter to fly again ({lives} lives left)"),
    }
}

/// Whether the player may ask to fly again now.
pub fn may_fly_again(revival: &Revival, seconds_left: u32) -> bool {
    revival.rule != Respawn::None
        && revival.lives != Some(0)
        && revival.why.as_deref() != Some(NO_ROOM)
        && seconds_left == 0
}

impl Client {
    /// The newest Revival since the player's plane was lost: `None` while
    /// it flies a plane that is not lost, in the lobby with none lost, and
    /// once it is seated again.
    pub fn revival(&self) -> Option<&Kept> {
        self.revival.as_ref()
    }

    /// What the flight says while the plane is lost, now: [`prompt`].
    pub fn revival_prompt(&self) -> Option<String> {
        let kept = self.revival.as_ref()?;
        Some(prompt(&kept.revival, kept.seconds_left(self.now)))
    }

    /// Whether Revive ([`Client::revive`]) may fly the player again now, as
    /// far as the newest Revival says.
    pub fn may_fly_again(&self) -> bool {
        self.revival
            .as_ref()
            .is_some_and(|kept| may_fly_again(&kept.revival, kept.seconds_left(self.now)))
    }

    /// Every revival's new plane of the mission flying, in order: what a
    /// game adds to another copy of the mission it builds.
    pub fn spawned(&self) -> &[Spawned] {
        &self.spawned
    }

    /// The host's Revival (message 29).
    pub(super) fn revival_message(&mut self, revival: Box<Revival>) {
        let words = prompt(&revival, revival.wait_seconds);
        self.log("revival", &[&format!("{:?}", revival.rule), &words]);
        self.revival = Some(Kept {
            revival: (*revival).clone(),
            received: self.now,
        });
        self.event(ClientEvent::Revival(revival));
    }

    /// A revival's new plane (message 30): added to the client's copy of
    /// the mission, and kept for the game's own copies.
    pub(super) fn spawned_message(&mut self, spawned: Box<Spawned>) {
        let new = NewPlane {
            plane: PlaneId(spawned.plane),
            slot: Slot {
                wing: spawned.wing,
                member: spawned.member,
            },
            aircraft: spawned.aircraft,
            spawn: spawned.spawn.clone(),
        };
        match self.mission.as_mut().map(|m| m.world.add_plane(&new)) {
            Some(Ok(())) => self.log("spawned", &[&spawned.plane.to_string()]),
            Some(Err(error)) => self.log(
                "spawn-failed",
                &[&spawned.plane.to_string(), &error.to_string()],
            ),
            None => self.log("spawn-failed", &[&spawned.plane.to_string(), "no mission"]),
        }
        self.spawned.push((*spawned).clone());
        self.event(ClientEvent::Spawned(spawned));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revival(rule: Respawn, lives: Option<u8>, wait_seconds: u32, why: Option<&str>) -> Revival {
        Revival {
            rule,
            lives,
            wait_seconds,
            why: why.map(str::to_owned),
        }
    }

    #[test]
    fn the_prompt_says_what_the_player_may_do() {
        let cases = [
            (
                revival(Respawn::Revive, Some(2), 0, None),
                0,
                "Press Enter to fly again (2 lives left)",
                true,
            ),
            (
                revival(Respawn::AiSlot, Some(1), 0, None),
                0,
                "Press Enter to fly again (1 life left)",
                true,
            ),
            (
                revival(Respawn::Revive, None, 0, None),
                0,
                "Press Enter to fly again",
                true,
            ),
            (
                revival(Respawn::Revive, None, 60, None),
                45,
                "You can fly again in 0:45",
                false,
            ),
            (
                revival(Respawn::Revive, None, 300, None),
                272,
                "You can fly again in 4:32",
                false,
            ),
            (
                revival(Respawn::Revive, Some(0), 0, Some(NO_LIVES)),
                0,
                "No lives left. Esc, then Watch, shows the battle.",
                false,
            ),
            (
                revival(Respawn::None, None, 0, Some(NO_REVIVAL)),
                0,
                "No revival in this game. Esc, then Watch, shows the battle.",
                false,
            ),
            (
                revival(Respawn::Revive, None, 0, Some(NO_ROOM)),
                0,
                "Waiting for room for another aircraft.",
                false,
            ),
        ];
        for (revival, left, words, may) in cases {
            assert_eq!(prompt(&revival, left), words);
            assert_eq!(may_fly_again(&revival, left), may, "{words}");
        }
    }

    #[test]
    fn the_wait_counts_down_from_the_arrival() {
        let kept = Kept {
            revival: revival(Respawn::Revive, None, 60, None),
            received: Duration::from_secs(10),
        };
        assert_eq!(kept.seconds_left(Duration::from_secs(10)), 60);
        assert_eq!(kept.seconds_left(Duration::from_millis(10_500)), 60);
        assert_eq!(kept.seconds_left(Duration::from_secs(11)), 59);
        assert_eq!(kept.seconds_left(Duration::from_secs(70)), 0);
        assert_eq!(kept.seconds_left(Duration::from_secs(700)), 0);
    }
}
