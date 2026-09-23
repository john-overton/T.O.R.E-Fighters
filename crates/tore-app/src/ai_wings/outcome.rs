//! `fitted`: in-flight mission result for situation music (SUCC and HOME).
//!
//! A stand-in. The unmerged debrief branch evaluates the mission at its end;
//! when it merges, its evaluator replaces [`AiWings::verdict`] and this file's
//! rules, keeping only the 4 second cadence and the home check. The rules are
//! the debrief's retail rules applied during flight: a friendly aircraft shot
//! down by the player fails; a friendly objective lost fails; every target
//! destroyed (shot down, crashed or its pilot ejected) succeeds; otherwise the
//! result is still open. Targets are the enemy group the player's flight is
//! assigned to destroy; with no such group every enemy aircraft is a target
//! (agent decision, 2026-09-23). Friendly objectives are the aircraft the
//! player's flight protects plus members of friendly must-survive groups.
use super::*;

/// The mission result and home check run this often, game seconds.
pub const CHECK_S: f64 = 4.;
/// Home: this close to the home base...
pub const HOME_RANGE_FT: f64 = 42_240.;
/// ...and below this altitude (above sea level; retail datum unknown).
pub const HOME_CEILING_FT: f64 = 20_000.;
pub const MISSION_ACCOMPLISHED: &str = "^MISSACC";
pub const ALMOST_HOME: &str = "^ALMSTHM";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Pending,
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Objectives {
    pub targets: Vec<u32>,
    pub protect: Vec<u32>,
}

pub fn verdict(
    objectives: &Objectives,
    lost: impl Fn(u32) -> bool,
    friendly_fire: bool,
) -> Verdict {
    if friendly_fire || objectives.protect.iter().any(|id| lost(*id)) {
        Verdict::Failed
    } else if objectives.targets.iter().all(|id| lost(*id)) {
        Verdict::Succeeded
    } else {
        Verdict::Pending
    }
}

impl AiWings {
    pub fn objectives(&self) -> Objectives {
        let side = |id: u32| {
            if id == PLAYER_ID {
                Some(launch::Side::Friendly)
            } else {
                self.slot(id).map(|slot| slot.side)
            }
        };
        let assignment = self.mission.player_assignment();
        let mut targets: Vec<u32> = assignment
            .destroy_ids
            .iter()
            .copied()
            .filter(|id| side(*id) == Some(launch::Side::Enemy))
            .collect();
        if targets.is_empty() {
            targets = self
                .slots
                .iter()
                .filter(|slot| slot.side == launch::Side::Enemy)
                .map(|slot| slot.id)
                .collect();
        }
        let mut protect: Vec<u32> = assignment
            .protected_ids
            .iter()
            .chain(self.mission.must_survive())
            .copied()
            .filter(|id| side(*id) == Some(launch::Side::Friendly))
            .collect();
        protect.sort_unstable();
        protect.dedup();
        Objectives { targets, protect }
    }
    /// Shot down, crashed or ejected. The player's own state comes from the host.
    pub fn aircraft_lost(&self, id: u32) -> bool {
        self.mission
            .actor(id)
            .is_none_or(|actor| !actor.alive() || actor.flight().escape.is_some())
    }
    pub fn is_friendly(&self, id: u32) -> bool {
        self.slot(id)
            .is_some_and(|slot| slot.side == launch::Side::Friendly)
    }
    pub fn verdict(&self, friendly_fire: bool, player_lost: bool) -> Verdict {
        verdict(
            &self.objectives(),
            |id| {
                if id == PLAYER_ID {
                    player_lost
                } else {
                    self.aircraft_lost(id)
                }
            },
            friendly_fire,
        )
    }
}

/// What the music needs from the mission this step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub succeeded: bool,
    /// Latched once reached; HOME itself still plays once.
    pub home: bool,
    /// Radio calls to queue now.
    pub radio: Vec<&'static str>,
}

/// Mission result cadence, success-disabled rule and home check for one flight.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tracker {
    /// None before the first check; false when the result was already
    /// decided at flight start or there is no mission.
    enabled: Option<bool>,
    next_check: f64,
    friendly_fire: bool,
    verdict: Option<Verdict>,
    announced: bool,
    home: bool,
    home_base: Option<[f64; 3]>,
}
impl Tracker {
    pub fn new(home_base: Option<[f64; 3]>) -> Self {
        Self {
            home_base,
            ..Self::default()
        }
    }
    /// The player shot down a friendly aircraft; the mission fails.
    pub fn friendly_fire(&mut self) {
        self.friendly_fire = true;
    }
    /// `evaluate` is None when the flight has no mission.
    pub fn step(
        &mut self,
        now: f64,
        evaluate: Option<impl FnOnce(bool) -> Verdict>,
        position: [f64; 3],
        airborne: bool,
    ) -> Status {
        let mut radio = Vec::new();
        if now >= self.next_check {
            self.next_check = now + CHECK_S;
            match (self.enabled, evaluate) {
                (None, Some(evaluate)) => {
                    self.enabled = Some(evaluate(self.friendly_fire) == Verdict::Pending);
                }
                (None, None) => self.enabled = Some(false),
                (Some(true), Some(evaluate)) => {
                    let verdict = evaluate(self.friendly_fire);
                    self.verdict = Some(verdict);
                    if verdict == Verdict::Succeeded {
                        if !self.announced {
                            self.announced = true;
                            radio.push(MISSION_ACCOMPLISHED);
                        }
                        if !self.home
                            && self.home_base.is_some_and(|base| {
                                let range = (base[0] - position[0]).hypot(base[2] - position[2]);
                                range.hypot(base[1] - position[1]) < HOME_RANGE_FT
                                    && position[1] < HOME_CEILING_FT
                            })
                        {
                            self.home = true;
                            if airborne {
                                radio.push(ALMOST_HOME);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Status {
            succeeded: self.verdict == Some(Verdict::Succeeded),
            home: self.home,
            radio,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verdict_rules() {
        let o = Objectives {
            targets: vec![5, 6],
            protect: vec![0, 2],
        };
        assert_eq!(verdict(&o, |_| false, false), Verdict::Pending);
        assert_eq!(verdict(&o, |id| id == 5, false), Verdict::Pending);
        assert_eq!(verdict(&o, |id| id >= 5, false), Verdict::Succeeded);
        assert_eq!(verdict(&o, |id| id >= 5, true), Verdict::Failed);
        assert_eq!(verdict(&o, |id| id >= 5 || id == 2, false), Verdict::Failed);
        assert_eq!(
            verdict(&Objectives::default(), |_| false, false),
            Verdict::Succeeded,
            "nothing to destroy is decided at once"
        );
    }

    #[test]
    fn success_is_disabled_when_decided_at_flight_start() {
        let mut t = Tracker::new(Some([0.; 3]));
        let decided = t.step(0., Some(|_| Verdict::Succeeded), [0.; 3], true);
        assert_eq!(decided, Status::default());
        let later = t.step(8., Some(|_| Verdict::Succeeded), [0.; 3], true);
        assert!(!later.succeeded && !later.home && later.radio.is_empty());
        let mut none = Tracker::new(None);
        none.step(0., None::<fn(bool) -> Verdict>, [0.; 3], true);
        assert!(
            !none
                .step(4., Some(|_| Verdict::Succeeded), [0.; 3], true)
                .succeeded
        );
    }

    #[test]
    fn four_second_cadence_radio_and_home() {
        let mut t = Tracker::new(Some([0., 0., 0.]));
        let far = [50_000., 5_000., 0.];
        assert!(
            t.step(0., Some(|_| Verdict::Pending), far, true)
                .radio
                .is_empty()
        );
        // Targets die at 1 s; nothing is re-evaluated before 4 s.
        assert!(
            !t.step(3.99, Some(|_| Verdict::Succeeded), far, true)
                .succeeded
        );
        let s = t.step(4., Some(|_| Verdict::Succeeded), far, true);
        assert!(s.succeeded && !s.home);
        assert_eq!(s.radio, [MISSION_ACCOMPLISHED]);
        // Within 42,240 ft but too high, then low enough.
        let high = [30_000., 20_000., 0.];
        assert!(!t.step(8., Some(|_| Verdict::Succeeded), high, true).home);
        let near = [30_000., 19_999., 0.];
        let s = t.step(12., Some(|_| Verdict::Succeeded), near, true);
        assert!(s.home);
        assert_eq!(s.radio, [ALMOST_HOME]);
        let s = t.step(16., Some(|_| Verdict::Succeeded), far, true);
        assert!(
            s.home && s.radio.is_empty(),
            "home is latched and called once"
        );
        // A later failure clears success but not the home latch.
        assert!(!t.step(20., Some(|_| Verdict::Failed), far, true).succeeded);
        // No home base: HOME never.
        let mut t = Tracker::new(None);
        t.step(0., Some(|_| Verdict::Pending), [0.; 3], true);
        assert!(!t.step(4., Some(|_| Verdict::Succeeded), [0.; 3], true).home);
        // On the ground the home condition is reached without the radio call.
        let mut t = Tracker::new(Some([0.; 3]));
        t.step(0., Some(|_| Verdict::Pending), [0.; 3], false);
        let s = t.step(4., Some(|_| Verdict::Succeeded), [0.; 3], false);
        assert!(s.home);
        assert_eq!(s.radio, [MISSION_ACCOMPLISHED]);
    }

    #[test]
    fn objectives_follow_assignment_sides_and_survival() {
        use tore_sim::ai::engagement::GroupObjective;
        // Fixture: friendly actors 1 and 2 are friendly group 2, enemies 3
        // and 4 are enemy group 1; the player leads friendly group 1.
        let (mut wings, _) = super::super::tests::build(None);
        assert_eq!(wings.objectives().targets, [3, 4], "no group: every enemy");
        assert!(wings.objectives().protect.is_empty());
        let mut objectives = [GroupObjective::Inherit; 6];
        objectives[0] =
            GroupObjective::Intercept(launch::WingId::new(launch::Side::Enemy, 0).unwrap());
        wings.apply_group_objectives(&objectives, [0.; 3]);
        wings.apply_group_survival(&[true, true, false, true, false, false]);
        assert_eq!(
            wings.objectives(),
            Objectives {
                targets: vec![3, 4],
                protect: vec![PLAYER_ID, 1, 2],
            },
            "enemy must-survive groups are not the player's objectives"
        );
        assert_eq!(wings.verdict(false, false), Verdict::Pending);
        assert_eq!(wings.verdict(false, true), Verdict::Failed);
        assert_eq!(wings.verdict(true, false), Verdict::Failed);
        for id in [3, 4] {
            wings.mission.actor_mut(id).unwrap().set_alive(false);
        }
        assert_eq!(wings.verdict(false, false), Verdict::Succeeded);
        wings.mission.actor_mut(2).unwrap().set_alive(false);
        assert_eq!(wings.verdict(false, false), Verdict::Failed);
        assert!(wings.is_friendly(1) && !wings.is_friendly(3) && !wings.is_friendly(PLAYER_ID));
    }
}
