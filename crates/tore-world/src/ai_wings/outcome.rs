//! In-flight mission result cadence: the mission result and home checks that
//! the situation music (SUCC and HOME) and the two radio calls ("Mission
//! accomplished!" and "We're almost home!") share.
//!
//! The mission core runs one [`Tracker`] for each human-flown plane and sends
//! the calls to its seat, whether or not the host has an audio device
//! (docs/ARCHITECTURE.md, "Radio, orders and debrief for each seat"). The
//! music reads the tracker's [`Status`]. The result comes from [`succeeded`],
//! which is the one rule the debrief reads too ([`Standing`]), so the music,
//! the calls and the debrief agree. This file keeps the 4 second check,
//! the rule that a result already decided at flight start disables SUCC and
//! HOME, and the home check.
use crate::ai_wings::AiWings;
use crate::comms::journal::{Audience, Cause, Origin, Source};
use crate::comms::{Call, Kind, Phrase, Phrases};
use crate::terrain::Terrain;
use tore_sim::ai::launch::Side;
use tore_sim::combat::ledger::{Kill, Ledger};
use tore_sim::combat::live;
/// The mission result and home check run this often, game seconds.
pub const CHECK_S: f64 = 4.;
/// Home: this close to the home base...
pub const HOME_RANGE_FT: f64 = 42_240.;
/// ...and below this altitude (above sea level; retail datum unknown).
pub const HOME_CEILING_FT: f64 = 20_000.;
pub const MISSION_ACCOMPLISHED: &str = "^MISSACC";
pub const ALMOST_HOME: &str = "^ALMSTHM";

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
    succeeded: bool,
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
    /// What the music reads: the result and the home latch, with no calls.
    pub fn status(&self) -> Status {
        Status {
            succeeded: self.succeeded,
            home: self.home,
            radio: Vec::new(),
        }
    }
    /// The Quick Mission home base this tracker checks, if any.
    pub fn home_base(&self) -> Option<[f64; 3]> {
        self.home_base
    }
    /// `evaluate` is None when the flight has no mission.
    pub fn step(
        &mut self,
        now: f64,
        succeeded: Option<impl FnOnce() -> bool>,
        position: [f64; 3],
        airborne: bool,
    ) -> Status {
        let mut radio = Vec::new();
        if now >= self.next_check {
            self.next_check = now + CHECK_S;
            match (self.enabled, succeeded) {
                // Success already reached at flight start disables SUCC and
                // HOME. The debrief evaluator cannot report an in-flight
                // failure, so an already failed start is not detected.
                (None, Some(succeeded)) => self.enabled = Some(!succeeded()),
                (None, None) => self.enabled = Some(false),
                (Some(true), Some(succeeded)) => {
                    self.succeeded = succeeded();
                    if self.succeeded {
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
            succeeded: self.succeeded,
            home: self.home,
            radio,
        }
    }
}

/// `fitted`: the Quick Mission home base is the ground-start airport, placed
/// at the mean centre of its runways. An airborne start has no home base.
pub fn home_base(world: &Terrain, airport: Option<u32>) -> Option<[f64; 3]> {
    let runways: Vec<_> = world
        .airport_scene
        .runways
        .iter()
        .filter(|runway| Some(runway.airport) == airport)
        .collect();
    (!runways.is_empty()).then(|| {
        std::array::from_fn(|i| {
            runways.iter().map(|r| r.surface.center[i]).sum::<f64>() / runways.len() as f64
        })
    })
}

/// One aircraft as the mission result sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Aircraft {
    pub id: u32,
    /// On the same side as the plane whose result this is.
    pub friendly: bool,
    /// Still flying with its pilot aboard.
    pub alive: bool,
}

/// What the mission asks of one plane: the aircraft it must destroy and the
/// ones it must protect. The debrief's objectives and the result check read
/// the same lists (docs/spec/debrief.md).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Requirements {
    pub destroy: Vec<u32>,
    pub protect: Vec<u32>,
}

impl Requirements {
    /// The requirements of `plane`, which flies for `side`, from its own
    /// assignment. Retail Quick Missions make every aircraft of the other side
    /// a target when the plane has no assigned target group, and the aircraft
    /// the mission says must survive are protected.
    pub fn of(wings: &AiWings, plane: u32, side: Side) -> Self {
        let mission = wings.mission();
        let assignment = mission.human_assignment(plane);
        let mut destroy = assignment.destroy_ids.clone();
        if destroy.is_empty() {
            destroy = wings
                .slots()
                .iter()
                .filter(|slot| slot.side != side)
                .map(|slot| slot.id)
                .collect();
        }
        let mut protect = assignment.protected_ids.clone();
        // Only the plane's own side is its objective: an enemy group whose
        // survival its own side requires is not (docs/spec/debrief.md,
        // "Friendly objectives"; bug bash finding 2026-09-29).
        for id in mission.must_survive(plane) {
            let own_side = *id == plane
                || wings
                    .slots()
                    .iter()
                    .any(|slot| slot.id == *id && slot.side == side);
            if own_side && !protect.contains(id) {
                protect.push(*id);
            }
        }
        Self { destroy, protect }
    }
}

/// The AI's aircraft as they stand, each friendly when it is on `side`, for
/// [`Standing`]. The plane itself and any other human-flown plane are not in
/// the AI's rows: the caller adds them.
pub fn ai_aircraft(wings: &AiWings, side: Side) -> Vec<Aircraft> {
    let mission = wings.mission();
    wings
        .slots()
        .iter()
        .map(|slot| Aircraft {
            id: slot.id,
            friendly: slot.side == side,
            alive: mission
                .actor(slot.id)
                .is_some_and(|a| a.alive() && a.flight().escape.is_none()),
        })
        .collect()
}

/// How the mission stands for one plane, from the combat ledger, every
/// aircraft's fate and the plane's requirements. The one place the mission
/// result rule lives: the plane shot down no friendly aircraft, every
/// aircraft it must destroy is gone and every one it must protect still
/// flies.
pub struct Standing<'a> {
    pub ledger: &'a Ledger,
    /// The plane whose result this is; it must be in `aircraft`.
    pub plane: u32,
    /// Every aircraft the plane knows of, the plane included. An id that is
    /// not listed counts as gone.
    pub aircraft: &'a [Aircraft],
    pub requirements: &'a Requirements,
}

impl Standing<'_> {
    fn aircraft(&self, id: u32) -> Option<&Aircraft> {
        self.aircraft.iter().find(|a| a.id == id)
    }
    pub fn friendly(&self, id: u32) -> bool {
        self.aircraft(id).is_some_and(|a| a.friendly)
    }
    /// Recorded kills, plus lost aircraft credited to their last attacker. The
    /// plane's own loss credits nobody.
    pub fn kills(&self) -> Vec<Kill> {
        let mut kills = self.ledger.kills().to_vec();
        for lost in self
            .aircraft
            .iter()
            .filter(|a| a.id != self.plane && !a.alive)
        {
            if !kills.iter().any(|k| k.victim == lost.id)
                && let Some(kill) = self.ledger.credit(lost.id)
            {
                kills.push(kill);
            }
        }
        kills
    }
    /// The plane shot down an aircraft of its own side.
    pub fn friendly_fire(&self) -> bool {
        self.kills()
            .iter()
            .any(|k| k.owner == self.plane && self.friendly(k.victim))
    }
    /// How many of the aircraft to destroy are gone.
    pub fn destroyed(&self) -> u32 {
        self.requirements
            .destroy
            .iter()
            .filter(|id| self.aircraft(**id).is_none_or(|a| !a.alive))
            .count() as u32
    }
    /// How many of the aircraft to protect still fly.
    pub fn protected(&self) -> u32 {
        self.requirements
            .protect
            .iter()
            .filter(|id| self.aircraft(**id).is_some_and(|a| a.alive))
            .count() as u32
    }
    pub fn succeeded(&self) -> bool {
        !self.friendly_fire()
            && self.destroyed() as usize == self.requirements.destroy.len()
            && self.protected() as usize == self.requirements.protect.len()
    }
}

/// Whether the mission has succeeded for the plane `plane`, as the debrief
/// decides it ([`Standing`]). `plane_alive` is whether the plane and its pilot
/// are alive. Human-flown planes other than `plane` are not in the AI's rows,
/// so they count as neither friendly nor alive here until the AI reports them
/// (B3).
pub fn succeeded(
    state: &live::State,
    wings: Option<&AiWings>,
    plane: u32,
    plane_alive: bool,
) -> bool {
    let mut aircraft = vec![Aircraft {
        id: plane,
        friendly: true,
        alive: plane_alive,
    }];
    let mut requirements = Requirements::default();
    if let Some(wings) = wings {
        aircraft.extend(ai_aircraft(wings, Side::Friendly));
        requirements = Requirements::of(wings, plane, Side::Friendly);
    }
    Standing {
        ledger: &state.ledger,
        plane,
        aircraft: &aircraft,
        requirements: &requirements,
    }
    .succeeded()
}

/// One mission-result call: the stem, why it is sent and who says it.
pub struct ResultCall {
    stem: &'static str,
    cause: Cause,
}
impl ResultCall {
    /// The call for `plane`'s seat, the mission result two seconds after it is
    /// decided and "almost home" at once, both important and addressed to the
    /// flight. `label` is the crew label or `YOU`.
    pub fn call(&self, plane: u32, label: &str, phrases: &Phrases) -> Call {
        let delay = if self.stem == MISSION_ACCOMPLISHED {
            2.
        } else {
            0.
        };
        Call::new(label, Phrase::stem(phrases, self.stem), Kind::Important)
            .after(delay)
            .because(
                Origin::of(Source::Radio, self.cause.clone())
                    .by(plane)
                    .to(Audience::Flight),
            )
    }
}

impl Tracker {
    /// One tick of `plane`'s mission result. `evaluate` is `None` without a
    /// mission. Returns the calls to send now, each with its trigger.
    pub fn results(
        &mut self,
        now: f64,
        evaluate: Option<impl FnOnce() -> bool>,
        position: [f64; 3],
        airborne: bool,
    ) -> Vec<ResultCall> {
        let base = self.home_base;
        self.step(now, evaluate, position, airborne)
            .radio
            .into_iter()
            .map(|stem| ResultCall {
                stem,
                cause: match stem {
                    ALMOST_HOME => {
                        let (range_ft, altitude_ft) = base.map_or((0., 0.), |base| {
                            let across = (base[0] - position[0]).hypot(base[2] - position[2]);
                            (across.hypot(base[1] - position[1]), position[1])
                        });
                        Cause::AlmostHome {
                            range_ft,
                            altitude_ft,
                        }
                    }
                    _ => Cause::MissionAccomplished,
                },
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn success_is_disabled_when_decided_at_flight_start() {
        let mut t = Tracker::new(Some([0.; 3]));
        let decided = t.step(0., Some(|| true), [0.; 3], true);
        assert_eq!(decided, Status::default());
        let later = t.step(8., Some(|| true), [0.; 3], true);
        assert!(!later.succeeded && !later.home && later.radio.is_empty());
        let mut none = Tracker::new(None);
        none.step(0., None::<fn() -> bool>, [0.; 3], true);
        assert!(!none.step(4., Some(|| true), [0.; 3], true).succeeded);
    }

    #[test]
    fn four_second_cadence_radio_and_home() {
        let mut t = Tracker::new(Some([0., 0., 0.]));
        let far = [50_000., 5_000., 0.];
        assert!(t.step(0., Some(|| false), far, true).radio.is_empty());
        // Targets die at 1 s; nothing is re-evaluated before 4 s.
        assert!(!t.step(3.99, Some(|| true), far, true).succeeded);
        let s = t.step(4., Some(|| true), far, true);
        assert!(s.succeeded && !s.home);
        assert_eq!(s.radio, [MISSION_ACCOMPLISHED]);
        // Within 42,240 ft but too high, then low enough.
        let high = [30_000., 20_000., 0.];
        assert!(!t.step(8., Some(|| true), high, true).home);
        let near = [30_000., 19_999., 0.];
        let s = t.step(12., Some(|| true), near, true);
        assert!(s.home);
        assert_eq!(s.radio, [ALMOST_HOME]);
        let s = t.step(16., Some(|| true), far, true);
        assert!(
            s.home && s.radio.is_empty(),
            "home is latched and called once"
        );
        // A later failure clears success but not the home latch.
        assert!(!t.step(20., Some(|| false), far, true).succeeded);
        // No home base: HOME never.
        let mut t = Tracker::new(None);
        t.step(0., Some(|| false), [0.; 3], true);
        assert!(!t.step(4., Some(|| true), [0.; 3], true).home);
        // On the ground the home condition is reached without the radio call.
        let mut t = Tracker::new(Some([0.; 3]));
        t.step(0., Some(|| false), [0.; 3], false);
        let s = t.step(4., Some(|| true), [0.; 3], false);
        assert!(s.home);
        assert_eq!(s.radio, [MISSION_ACCOMPLISHED]);
    }

    #[test]
    fn result_calls_carry_their_delay_label_and_trigger() {
        let mut t = Tracker::new(Some([0.; 3]));
        let far = [50_000., 5_000., 0.];
        assert!(t.results(0., Some(|| false), far, true).is_empty());
        let near = [0., 5_000., 10_000.];
        let results = t.results(4., Some(|| true), near, true);
        assert_eq!(results.len(), 2);
        let phrases: Phrases = [("^MISSACC", "Mission accomplished")]
            .into_iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();
        let calls: Vec<Call> = results
            .iter()
            .map(|result| result.call(0, "RIO", &phrases))
            .collect();
        assert_eq!(calls[0].stems, [MISSION_ACCOMPLISHED]);
        assert_eq!(calls[0].text, "Mission accomplished");
        assert_eq!((calls[0].delay, calls[1].delay), (2., 0.));
        assert!(
            calls
                .iter()
                .all(|c| c.label == "RIO" && c.kind == Kind::Important)
        );
        assert_eq!(calls[0].origin.cause, Cause::MissionAccomplished);
        assert_eq!(
            calls[1].origin.cause,
            Cause::AlmostHome {
                range_ft: 10_000f64.hypot(5_000.),
                altitude_ft: 5_000.
            }
        );
        assert_eq!(t.status().radio, Vec::<&str>::new());
        assert!(t.status().succeeded && t.status().home);
    }

    #[test]
    fn a_mission_without_wings_succeeds_unless_the_plane_shot_a_friend() {
        let state = crate::test_support::combat_fixture(true);
        assert!(succeeded(&state, None, 0, true));
        assert!(
            succeeded(&state, None, 0, false),
            "the plane's own death is not a failure"
        );
    }

    fn aircraft(id: u32, friendly: bool, alive: bool) -> Aircraft {
        Aircraft {
            id,
            friendly,
            alive,
        }
    }

    #[test]
    fn the_standing_of_a_plane_follows_its_own_kills_and_requirements() {
        use tore_sim::combat::ledger::Kill;
        let mut ledger = Ledger::default();
        // Plane 2 shoots down a friendly, plane 0 an enemy; a lost enemy with
        // only a last hit is credited to the plane that hit it.
        ledger.kill(Kill {
            owner: 2,
            victim: 1,
            category: 0x8000,
            aircraft: true,
        });
        ledger.kill(Kill {
            owner: 0,
            victim: 10,
            category: 0x8000,
            aircraft: true,
        });
        ledger.damaged(Kill {
            owner: 2,
            victim: 11,
            category: 0x8000,
            aircraft: true,
        });
        let fates = [
            aircraft(0, true, true),
            aircraft(1, true, false),
            aircraft(2, true, true),
            aircraft(10, false, false),
            aircraft(11, false, false),
        ];
        let requirements = Requirements {
            destroy: vec![10, 11],
            protect: vec![0],
        };
        let standing = |plane| Standing {
            ledger: &ledger,
            plane,
            aircraft: &fates,
            requirements: &requirements,
        };
        assert_eq!(standing(0).kills().len(), 3);
        assert!(!standing(0).friendly_fire());
        assert!(standing(0).succeeded());
        assert_eq!((standing(0).destroyed(), standing(0).protected()), (2, 1));
        // The same ledger fails the plane that shot a friend down.
        assert!(standing(2).friendly_fire());
        assert!(!standing(2).succeeded());
        // An aircraft the standing does not list counts as gone: destroyed if
        // it is a target, lost if it must be protected.
        let unlisted = Requirements {
            destroy: vec![99],
            protect: vec![98],
        };
        let lists = Standing {
            requirements: &unlisted,
            ..standing(0)
        };
        assert_eq!((lists.destroyed(), lists.protected()), (1, 0));
    }

    /// The bug bash's objective-side rule (battery finding 2026-09-29): an
    /// enemy group whose own side must keep it alive is not the plane's
    /// objective, so shooting it down does not fail the mission.
    #[test]
    fn an_enemy_group_that_must_survive_is_not_the_planes_objective() {
        use tore_formats::aircraft::AircraftId;
        use tore_sim::ai::launch::{self, WingSelection, resolve_wings};
        let selections =
            [(launch::Side::Friendly, 0u8), (launch::Side::Enemy, 0)].map(|(side, index)| {
                WingSelection {
                    wing: launch::WingId::new(side, index).unwrap(),
                    aircraft: AircraftId::F18,
                    count: 2,
                    skill_level: 1,
                }
            });
        let launches = resolve_wings(&selections, None).unwrap();
        let targets = crate::test_support::spawned();
        let mut wings = AiWings::build_with(&launches, &targets, 0, |_| {
            Ok((crate::test_support::profile(), None))
        })
        .unwrap();
        let state = crate::test_support::combat_fixture(true);
        let enemies: Vec<u32> = wings
            .slots()
            .iter()
            .filter(|slot| slot.side != Side::Friendly)
            .map(|slot| slot.id)
            .collect();
        assert!(!enemies.is_empty());
        for id in &enemies {
            wings.mission.actor_mut(*id).unwrap().flight_mut().crashed = true;
        }
        // The enemy group (group 4) must survive for its own side.
        wings.apply_group_survival(&[false, false, false, true, false, false]);
        assert!(
            wings
                .mission()
                .must_survive(0)
                .iter()
                .any(|id| enemies.contains(id))
        );
        assert!(succeeded(&state, Some(&wings), 0, true));
        // A friendly group that must survive still is the plane's objective.
        wings.apply_group_survival(&[true, false, false, false, false, false]);
        let friends: Vec<u32> = wings
            .slots()
            .iter()
            .filter(|slot| slot.side == Side::Friendly)
            .map(|slot| slot.id)
            .collect();
        wings
            .mission
            .actor_mut(friends[0])
            .unwrap()
            .flight_mut()
            .crashed = true;
        assert!(!succeeded(&state, Some(&wings), 0, true));
    }
}
