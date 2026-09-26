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
    aims: BTreeMap<u32, u32>,
    tallies: BTreeMap<Key, Tally>,
    kills: Vec<Kill>,
    /// The last shooter to damage each target, credited if it is later lost
    /// another way, such as its pilot ejecting or the wreck crashing.
    last_hit: BTreeMap<u32, Kill>,
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
    /// Closes an open projectile. A decoyed missile is resolved when it is
    /// spoofed, so its later ground impact or expiry does not count twice.
    pub fn resolve(&mut self, projectile: u32, resolution: Resolution) {
        let Some(key) = self.open.remove(&projectile) else {
            return;
        };
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
    /// Remembers who last damaged `victim`.
    pub fn damaged(&mut self, hit: Kill) {
        self.last_hit.insert(hit.victim, hit);
    }
    /// The kill a lost aircraft is credited with: its recorded kill, else the
    /// last shooter to damage it.
    pub fn credit(&self, victim: u32) -> Option<Kill> {
        self.kills
            .iter()
            .find(|k| k.victim == victim)
            .or_else(|| self.last_hit.get(&victim))
            .copied()
    }
    pub fn kill(&mut self, kill: Kill) {
        if !self.kills.iter().any(|k| k.victim == kill.victim) {
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
}
