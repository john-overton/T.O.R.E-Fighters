//! Mission results by shooter: every round, missile and bomb from launch to
//! its outcome, plus credited kills. The debrief reads these totals; nothing
//! in flight depends on them. See docs/spec/debrief.md.
use std::collections::{BTreeMap, VecDeque};
use tore_formats::weapons::Weapon;

/// Keeps the ledger bounded if a host records aims it never launches.
const MAX_PENDING_AIMS: usize = 4096;
/// Shot outcomes kept between drains; the oldest are dropped first, so a
/// host that never drains them still uses bounded memory.
pub const MAX_OUTCOMES: usize = 1024;
/// Decoyed missiles still flying that the ledger remembers, so a late strike
/// can still be counted. The oldest id is forgotten first.
const MAX_DECOYED: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ShotKind {
    AirToAir,
    AirToGround,
    Gun,
    Bomb,
    /// Unguided rockets and anything else the debrief does not list.
    Other,
}
impl ShotKind {
    /// The retail debrief's store-flag rule: guided (0x1) air (0x10000) or
    /// surface (0x20000) missiles, then bombs (0x10), then guns (0x80).
    pub fn of(weapon: &Weapon) -> Self {
        let flags = weapon.flags;
        if flags & 1 != 0 {
            if flags & 0x10000 != 0 {
                Self::AirToAir
            } else if flags & 0x20000 != 0 {
                Self::AirToGround
            } else {
                Self::Other
            }
        } else if flags & 0x10 != 0 {
            Self::Bomb
        } else if flags & 0x80 != 0 {
            Self::Gun
        } else {
            Self::Other
        }
    }
}

/// Who fired, at whom, with what. `aim` is the intended target when known;
/// the player's id is 0.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Key {
    pub owner: u32,
    pub aim: Option<u32>,
    pub kind: ShotKind,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    pub launched: u32,
    pub hit: u32,
    /// Damage points the hits did.
    pub damage: u32,
    pub missed: u32,
    pub spoofed: u32,
    pub jammed: u32,
}
impl Tally {
    /// Every launch that neither hit nor was decoyed or jammed, including
    /// rounds still in flight when the mission ends.
    pub fn failed(&self) -> u32 {
        self.launched
            .saturating_sub(self.hit)
            .saturating_sub(self.spoofed)
            .saturating_sub(self.jammed)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resolution {
    /// Carries the damage points applied.
    Hit(u32),
    Missed,
    Spoofed,
    Jammed,
}

/// How one shot ended, kept for mission recordings. Write-only: nothing in
/// flight reads it; the host drains the list each tick with
/// [`Ledger::take_outcomes`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub projectile: u32,
    pub key: Key,
    pub resolution: Resolution,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kill {
    pub owner: u32,
    pub victim: u32,
    /// Victim object class word, from its PT or static object.
    pub category: u16,
    pub aircraft: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ledger {
    /// Projectiles in flight, keyed by id.
    open: BTreeMap<u32, Key>,
    /// Missiles resolved as spoofed that may still be flying: a decoyed
    /// missile coasts on and can strike an aircraft after all, which then
    /// counts as its hit (see [`Ledger::resolve`]).
    decoyed: BTreeMap<u32, Key>,
    aims: BTreeMap<u32, u32>,
    tallies: BTreeMap<Key, Tally>,
    kills: Vec<Kill>,
    /// The last shooter to damage each target, credited if it is later lost
    /// another way, such as its pilot ejecting or the wreck crashing.
    last_hit: BTreeMap<u32, Kill>,
    /// Aircraft lost with no shooter to blame (out of bounds): nobody is
    /// credited with them, whatever hit them earlier.
    uncredited: std::collections::BTreeSet<u32>,
    /// Outcomes since the host last drained them.
    outcomes: VecDeque<Outcome>,
}

impl Ledger {
    /// A host that knows an unguided round's intended target records it
    /// before the round's first step.
    pub fn aim(&mut self, projectile: u32, target: u32) {
        if self.aims.len() < MAX_PENDING_AIMS {
            self.aims.insert(projectile, target);
        }
    }
    pub fn launch(&mut self, projectile: u32, owner: u32, aim: Option<u32>, kind: ShotKind) {
        let aim = self.aims.remove(&projectile).or(aim);
        let key = Key { owner, aim, kind };
        if self.open.insert(projectile, key).is_none() {
            self.tallies.entry(key).or_default().launched += 1;
        }
    }
    /// Closes an open projectile. A missile resolves once, by what finally
    /// happened to it, with one exception: a missile resolved as spoofed
    /// keeps flying, and if it then damages an aircraft the spoof is
    /// withdrawn and the strike is its hit, with its damage. Without that the
    /// aircraft would lose hit points and the shooter be credited with the
    /// kill (see [`Ledger::damaged`]) that no hit in the tally accounts for.
    /// Its later ground impact, expiry, jamming or contact with a wreck leave
    /// the spoof as it was and do not count twice.
    pub fn resolve(&mut self, projectile: u32, resolution: Resolution) {
        let Some(key) = self.open.remove(&projectile) else {
            self.resolve_decoyed(projectile, resolution);
            return;
        };
        if resolution == Resolution::Spoofed {
            if self.decoyed.len() == MAX_DECOYED {
                self.decoyed.pop_first();
            }
            self.decoyed.insert(projectile, key);
        }
        let tally = self.tallies.entry(key).or_default();
        match resolution {
            Resolution::Hit(damage) => {
                tally.hit += 1;
                tally.damage = tally.damage.saturating_add(damage);
            }
            Resolution::Missed => tally.missed += 1,
            Resolution::Spoofed => tally.spoofed += 1,
            Resolution::Jammed => tally.jammed += 1,
        }
        self.report(projectile, key, resolution);
    }
    /// The end of a missile that was resolved as spoofed: only damage turns
    /// it into a hit.
    fn resolve_decoyed(&mut self, projectile: u32, resolution: Resolution) {
        let Some(key) = self.decoyed.remove(&projectile) else {
            return;
        };
        if let Resolution::Hit(damage) = resolution
            && damage > 0
        {
            let tally = self.tallies.entry(key).or_default();
            tally.spoofed = tally.spoofed.saturating_sub(1);
            tally.hit += 1;
            tally.damage = tally.damage.saturating_add(damage);
            self.report(projectile, key, resolution);
        }
    }
    fn report(&mut self, projectile: u32, key: Key, resolution: Resolution) {
        if self.outcomes.len() == MAX_OUTCOMES {
            self.outcomes.pop_front();
        }
        self.outcomes.push_back(Outcome {
            projectile,
            key,
            resolution,
        });
    }
    /// Shot outcomes since the last call, oldest first.
    pub fn take_outcomes(&mut self) -> Vec<Outcome> {
        self.outcomes.drain(..).collect()
    }
    /// Remembers who last damaged `victim`. Combat calls it only for a strike
    /// that did damage, right after [`Ledger::resolve`] tallied that same hit,
    /// so the last attacker always has a recorded hit behind it.
    pub fn damaged(&mut self, hit: Kill) {
        if !self.uncredited.contains(&hit.victim) {
            self.last_hit.insert(hit.victim, hit);
        }
    }
    /// `victim` was lost for a reason no shooter caused (it flew out of
    /// bounds): drop any credit already given or pending for it.
    pub fn lose_without_credit(&mut self, victim: u32) {
        self.uncredited.insert(victim);
        self.last_hit.remove(&victim);
        self.kills.retain(|k| k.victim != victim);
    }
    /// The kill a lost aircraft is credited with: its recorded kill, else the
    /// last shooter to damage it.
    pub fn credit(&self, victim: u32) -> Option<Kill> {
        if self.uncredited.contains(&victim) {
            return None;
        }
        self.kills
            .iter()
            .find(|k| k.victim == victim)
            .or_else(|| self.last_hit.get(&victim))
            .copied()
    }
    pub fn kill(&mut self, kill: Kill) {
        if !self.uncredited.contains(&kill.victim)
            && !self.kills.iter().any(|k| k.victim == kill.victim)
        {
            self.kills.push(kill);
        }
    }
    pub fn tallies(&self) -> impl Iterator<Item = (&Key, &Tally)> {
        self.tallies.iter()
    }
    pub fn kills(&self) -> &[Kill] {
        &self.kills
    }
    /// Sum of every tally matching `filter`.
    pub fn total(&self, filter: impl Fn(&Key) -> bool) -> Tally {
        self.tallies
            .iter()
            .filter(|(key, _)| filter(key))
            .fold(Tally::default(), |sum, (_, t)| Tally {
                launched: sum.launched + t.launched,
                hit: sum.hit + t.hit,
                damage: sum.damage.saturating_add(t.damage),
                missed: sum.missed + t.missed,
                spoofed: sum.spoofed + t.spoofed,
                jammed: sum.jammed + t.jammed,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn launches_resolve_once_and_spoofed_missiles_are_not_also_failed() {
        let mut ledger = Ledger::default();
        ledger.launch(1, 0, Some(7), ShotKind::AirToAir);
        ledger.launch(1, 0, Some(7), ShotKind::AirToAir);
        ledger.launch(2, 0, Some(7), ShotKind::AirToAir);
        ledger.launch(4, 0, Some(7), ShotKind::AirToAir);
        ledger.launch(3, 0, None, ShotKind::Gun);
        ledger.resolve(1, Resolution::Spoofed);
        ledger.resolve(1, Resolution::Missed);
        ledger.resolve(2, Resolution::Hit(120));
        ledger.resolve(3, Resolution::Missed);
        let missiles = ledger.total(|k| k.owner == 0 && k.kind == ShotKind::AirToAir);
        assert_eq!(
            missiles,
            Tally {
                launched: 3,
                hit: 1,
                damage: 120,
                spoofed: 1,
                ..Tally::default()
            }
        );
        // Missile 4 is still flying: it counts as failed.
        assert_eq!(missiles.failed(), 1);
        assert_eq!(ledger.total(|k| k.kind == ShotKind::Gun).failed(), 1);
    }
    #[test]
    fn each_resolved_shot_is_reported_once_and_the_report_stays_bounded() {
        let mut ledger = Ledger::default();
        ledger.launch(1, 3, Some(0), ShotKind::AirToAir);
        ledger.launch(2, 0, Some(3), ShotKind::Gun);
        ledger.resolve(1, Resolution::Spoofed);
        ledger.resolve(1, Resolution::Missed);
        ledger.resolve(9, Resolution::Missed);
        ledger.resolve(2, Resolution::Hit(40));
        let key = |owner, aim, kind| Key { owner, aim, kind };
        assert_eq!(
            ledger.take_outcomes(),
            [
                Outcome {
                    projectile: 1,
                    key: key(3, Some(0), ShotKind::AirToAir),
                    resolution: Resolution::Spoofed,
                },
                Outcome {
                    projectile: 2,
                    key: key(0, Some(3), ShotKind::Gun),
                    resolution: Resolution::Hit(40),
                },
            ]
        );
        assert!(ledger.take_outcomes().is_empty());
        // A host that never drains keeps only the newest.
        for id in 0..(MAX_OUTCOMES as u32 + 10) {
            ledger.launch(100 + id, 0, None, ShotKind::Gun);
            ledger.resolve(100 + id, Resolution::Missed);
        }
        let outcomes = ledger.take_outcomes();
        assert_eq!(outcomes.len(), MAX_OUTCOMES);
        assert_eq!(outcomes[0].projectile, 110);
        // The debrief totals are untouched by the drain.
        assert_eq!(
            ledger.total(|k| k.kind == ShotKind::Gun).missed,
            MAX_OUTCOMES as u32 + 10
        );
    }
    #[test]
    fn host_aims_override_unguided_targets_and_stay_bounded() {
        let mut ledger = Ledger::default();
        ledger.aim(9, 42);
        ledger.launch(9, 5, None, ShotKind::Gun);
        assert_eq!(ledger.total(|k| k.aim == Some(42)).launched, 1);
        for id in 0..(MAX_PENDING_AIMS as u32 + 10) {
            ledger.aim(100 + id, 1);
        }
        assert_eq!(ledger.aims.len(), MAX_PENDING_AIMS);
    }
    #[test]
    fn a_spoofed_missile_that_strikes_after_all_is_counted_as_a_hit_once() {
        let mut ledger = Ledger::default();
        for id in [1, 2, 3, 4] {
            ledger.launch(id, 0, Some(7), ShotKind::AirToAir);
        }
        ledger.resolve(1, Resolution::Spoofed);
        ledger.resolve(2, Resolution::Spoofed);
        ledger.resolve(3, Resolution::Spoofed);
        ledger.resolve(4, Resolution::Spoofed);
        ledger.take_outcomes();
        // It coasts on and damages the aircraft: the spoof is withdrawn.
        ledger.resolve(1, Resolution::Hit(140));
        ledger.resolve(1, Resolution::Hit(140));
        // Nothing else that happens to a spoofed missile changes the tally:
        // a miss, a jam, or touching a wreck (a hit that did no damage).
        ledger.resolve(2, Resolution::Missed);
        ledger.resolve(2, Resolution::Hit(50));
        ledger.resolve(3, Resolution::Hit(0));
        ledger.resolve(3, Resolution::Hit(50));
        ledger.resolve(4, Resolution::Jammed);
        let missiles = ledger.total(|k| k.owner == 0);
        assert_eq!(
            missiles,
            Tally {
                launched: 4,
                hit: 1,
                damage: 140,
                spoofed: 3,
                ..Tally::default()
            }
        );
        // Resolved once each: launches never fall short of their outcomes.
        assert_eq!(missiles.failed(), 0);
        // The recording hears of the late strike, as a hit on the same key.
        let outcomes = ledger.take_outcomes();
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].projectile, 1);
        assert_eq!(outcomes[0].resolution, Resolution::Hit(140));
        assert_eq!(outcomes[0].key.aim, Some(7));
    }

    #[test]
    fn the_spoofed_missiles_remembered_are_bounded() {
        let mut ledger = Ledger::default();
        for id in 0..(MAX_DECOYED as u32 + 10) {
            ledger.launch(id, 0, None, ShotKind::AirToAir);
            ledger.resolve(id, Resolution::Spoofed);
        }
        assert_eq!(ledger.decoyed.len(), MAX_DECOYED);
        // The oldest was forgotten, the newest is kept.
        ledger.resolve(0, Resolution::Hit(10));
        ledger.resolve(MAX_DECOYED as u32 + 9, Resolution::Hit(10));
        let total = ledger.total(|_| true);
        assert_eq!((total.hit, total.damage), (1, 10));
    }

    #[test]
    fn a_victim_is_credited_once() {
        let mut ledger = Ledger::default();
        let kill = Kill {
            owner: 0,
            victim: 3,
            category: 0x8000,
            aircraft: true,
        };
        ledger.kill(kill);
        ledger.kill(Kill { owner: 4, ..kill });
        assert_eq!(ledger.kills(), [kill]);
        // A damaged aircraft lost later goes to its last attacker.
        ledger.damaged(Kill {
            owner: 6,
            victim: 8,
            ..kill
        });
        ledger.damaged(Kill {
            owner: 7,
            victim: 8,
            ..kill
        });
        assert_eq!(ledger.credit(8).map(|k| k.owner), Some(7));
        assert_eq!(ledger.credit(3), Some(kill));
        assert_eq!(ledger.credit(9), None);
    }

    #[test]
    fn an_aircraft_lost_out_of_bounds_credits_nobody() {
        let mut ledger = Ledger::default();
        let hit = Kill {
            owner: 7,
            victim: 3,
            category: 0x8000,
            aircraft: true,
        };
        ledger.damaged(hit);
        assert_eq!(ledger.credit(3), Some(hit));
        ledger.kill(hit);
        ledger.lose_without_credit(3);
        assert_eq!(ledger.credit(3), None);
        assert!(ledger.kills().is_empty());
        // Later hits or kills on the wreck do not bring the credit back.
        ledger.damaged(hit);
        ledger.kill(hit);
        assert_eq!(ledger.credit(3), None);
        assert!(ledger.kills().is_empty());
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "ledger_checkpoint.rs"]
mod checkpoint;
