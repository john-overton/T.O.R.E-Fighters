//! The debrief evaluator: reads a finished mission into the [`Report`] that
//! one seat's debrief shows. It lives in the mission core so a dedicated
//! server can build each seat's report; the app keeps the screen
//! (`crates/tore-app/src/debrief.rs`). Spec: docs/spec/debrief.md.
use crate::ai_wings::outcome::{self, Requirements, Standing};
use crate::seats::{Plane, PlaneId, Roster, SeatId, Slot};
use crate::world::{Cockpit, World};
use tore_formats::aircraft::AircraftId;
use tore_sim::ai::launch::Side;
use tore_sim::combat::ledger::{Kill, Ledger, ShotKind, Tally};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Outcome {
    Success,
    #[default]
    Failure,
}
impl Outcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Success => "SUCCESS",
            Self::Failure => "FAILURE",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    #[default]
    Alive,
    Ejected,
    Dead,
}

/// Kill table rows, in retail order.
pub const KILL_ROWS: [&str; 10] = [
    "Fighter",
    "Bomber",
    "Helicopter",
    "Ship",
    "SAM",
    "AAA",
    "Tank",
    "Vehicle",
    "Structure",
    "Other",
];

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pilot {
    pub status: Status,
    /// Airframe damage, 0 to 1; a dead or ejected pilot shows 100%.
    pub damage: f64,
    /// Average landing score as a percentage.
    pub landing_grade: Option<u32>,
    /// `overspeed` or `out of bounds` when the airframe was lost that way.
    pub cause: Option<&'static str>,
    pub kills: [u32; 10],
    pub friendly_fire: u32,
    pub air_to_air: Tally,
    pub air_to_ground: Tally,
    pub gun: Tally,
    pub bombs: Tally,
    /// Enemy fire aimed at this pilot. SAM and AAA sites do not exist yet.
    pub enemy_aam: Tally,
    pub enemy_sam: Tally,
    pub enemy_gun: Tally,
    pub enemy_aaa: Tally,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Objective {
    Destroy { destroyed: u32, total: u32 },
    Protect { protected: u32, total: u32 },
}
impl Objective {
    pub fn sentence(self) -> String {
        match self {
            Self::Destroy { destroyed, total } => match (destroyed, total) {
                (1, 1) => "Destroyed the target.".into(),
                (0, 1) => "Failed to destroy the target.".into(),
                (d, t) if d == t => format!("Destroyed the {t} targets."),
                (d, t) => format!("Destroyed {d} of {t} targets."),
            },
            Self::Protect { protected, total } => match (protected, total) {
                (1, 1) => "Protected the friendly objective.".into(),
                (0, 1) => "Failed to protect the friendly objective.".into(),
                (p, t) if p == t => format!("Protected the {t} friendly objectives."),
                (p, t) => format!("Protected {p} of {t} friendly objectives."),
            },
        }
    }
}

/// Everything the five pages show, captured when the mission ends, for one
/// seat.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub outcome: Outcome,
    pub objectives: Vec<Objective>,
    pub elapsed_seconds: u64,
    /// The pilot column: the plane the seat flies. The page calls it the
    /// player.
    pub player: Pilot,
    /// The one tracked wingman, the first other member of the seat's wing;
    /// `None` when the seat's plane flew alone.
    pub wingman: Option<Pilot>,
}

/// One aircraft as the mission ends. `id` is the plane's id, and `friendly`
/// means on the side of the plane whose debrief this is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Airframe {
    pub id: u32,
    pub friendly: bool,
    /// Still flying with its pilot aboard.
    pub alive: bool,
    /// The pilot escaped; a dead pilot is never ejected.
    pub ejected: bool,
    /// Airframe damage, 0 to 1.
    pub damage: f64,
    pub landing_grade: Option<u32>,
    /// Lost to overspeed or to leaving the map rather than to combat, a crash
    /// or the pilot (requested by John, 2026-09-29).
    pub cause: Option<tore_sim::aircraft_systems::LossCause>,
}

/// Host facts at mission end, gathered before the flight is torn down.
pub struct Ending<'a> {
    pub ledger: &'a Ledger,
    pub ticks: u64,
    /// The plane whose debrief this is: the pilot column.
    pub player: Airframe,
    /// Every other aircraft, both sides: the AI's and the other human-flown
    /// planes.
    pub aircraft: Vec<Airframe>,
    /// The aircraft shown in the WINGMAN column, if the plane had one.
    pub wingman: Option<u32>,
    /// What the mission asks of the plane.
    pub requirements: Requirements,
}

impl Ending<'_> {
    fn airframe(&self, id: u32) -> Option<&Airframe> {
        if id == self.player.id {
            Some(&self.player)
        } else {
            self.aircraft.iter().find(|a| a.id == id)
        }
    }
    fn hostile(&self, id: u32) -> bool {
        self.airframe(id).is_some_and(|a| !a.friendly)
    }
    /// The mission result rule, shared with the in-flight result check.
    fn fates(&self) -> Vec<outcome::Aircraft> {
        std::iter::once(&self.player)
            .chain(&self.aircraft)
            .map(|a| outcome::Aircraft {
                id: a.id,
                friendly: a.friendly,
                alive: a.alive,
            })
            .collect()
    }
    fn pilot(&self, airframe: &Airframe, standing: &Standing, kills: &[Kill]) -> Pilot {
        let id = airframe.id;
        let own = |kind: ShotKind| self.ledger.total(|k| k.owner == id && k.kind == kind);
        // Enemy aircraft fire splits only into guns and everything else.
        let at = |gun: bool| {
            self.ledger.total(|k| {
                k.aim == Some(id) && (k.kind == ShotKind::Gun) == gun && self.hostile(k.owner)
            })
        };
        let status = if airframe.ejected {
            Status::Ejected
        } else if airframe.alive {
            Status::Alive
        } else {
            Status::Dead
        };
        let mut pilot = Pilot {
            status,
            damage: if status == Status::Alive {
                airframe.damage
            } else {
                1.
            },
            landing_grade: airframe.landing_grade,
            cause: airframe.cause.map(|c| c.label()),
            air_to_air: own(ShotKind::AirToAir),
            air_to_ground: own(ShotKind::AirToGround),
            gun: own(ShotKind::Gun),
            bombs: own(ShotKind::Bomb),
            enemy_aam: at(false),
            enemy_gun: at(true),
            ..Pilot::default()
        };
        for kill in kills.iter().filter(|k| k.owner == id) {
            if standing.friendly(kill.victim) {
                pilot.friendly_fire += 1;
            } else if let Some(row) = kill_row(kill.category) {
                pilot.kills[row] += 1;
            }
        }
        pilot
    }
}

/// The kill row for a victim's object class word: the first matching bit of
/// 0x8000 fighter, 0x4000 bomber, 0x2000 ship, 0x1000 SAM, 0x800 AAA,
/// 0x400 tank, 0x200 vehicle, 0x100 structure and 0x40 other. Retail puts
/// helicopters first by a PT flag; no supported aircraft is a helicopter yet.
pub fn kill_row(category: u16) -> Option<usize> {
    const BITS: [(u16, usize); 9] = [
        (0x8000, 0),
        (0x4000, 1),
        (0x2000, 3),
        (0x1000, 4),
        (0x800, 5),
        (0x400, 6),
        (0x200, 7),
        (0x100, 8),
        (0x40, 9),
    ];
    BITS.iter()
        .find(|(bit, _)| category & bit != 0)
        .map(|(_, row)| *row)
}

/// The aircraft in the wingman column for the plane `plane`: the first other
/// member of its wing, human-flown or not.
pub fn wingman_of(roster: &Roster, plane: PlaneId) -> Option<u32> {
    let wing = roster.plane(plane)?.slot.wing;
    roster
        .planes()
        .iter()
        .filter(|p| p.id != plane && p.slot.wing == wing)
        .min_by_key(|p| p.slot.member)
        .map(|p| p.id.0)
}

/// A human-flown plane as the mission ends, from its cockpit's flight and its
/// ownship's hit points.
pub fn cockpit_airframe(
    id: u32,
    friendly: bool,
    flight: &tore_sim::flight::State,
    hp: i32,
) -> Airframe {
    let pilot = &flight.systems.pilot;
    Airframe {
        id,
        friendly,
        alive: !flight.crashed && hp > 0 && !pilot.dead && !pilot.ejected,
        ejected: pilot.ejected && !pilot.dead,
        damage: flight.damage_fraction,
        landing_grade: flight.research.as_ref().and_then(|r| r.landings.grade()),
        cause: flight.systems.structure.cause,
    }
}

/// An AI aircraft as the mission ends, from its actor.
fn actor_airframe(mission: &tore_sim::ai::mission::AiMission, id: u32, friendly: bool) -> Airframe {
    let actor = mission.actor(id);
    Airframe {
        id,
        friendly,
        alive: actor.is_some_and(|a| a.alive() && a.flight().escape.is_none()),
        ejected: actor.is_some_and(|a| a.flight().escape.is_some()),
        damage: actor.map_or(1., |a| a.flight().damage_fraction),
        // AI aircraft do not land yet.
        landing_grade: None,
        cause: actor.and_then(|a| a.flight().systems.structure.cause),
    }
}

/// Reads the mission's results for `seat` from the world: the pilot column is
/// the plane the seat flies, the wingman column the first other member of its
/// wing, the objectives are that plane's and friendly fire counts the kills it
/// made. `None` when the seat flies no plane. The caller reads it before the
/// world is dropped or restarted, because the AI wings go with the world.
pub fn capture(world: &World, seat: SeatId) -> Option<Report> {
    let plane = world.roster.seat(seat)?.plane?;
    let side = world.roster.plane(plane)?.slot.wing.side;
    let state = &world.combat.state;
    let fate = |cockpit: &Cockpit| {
        let id = cockpit.plane.0;
        let hp = state.ownship(id).map_or(0, |own| own.hp);
        let friendly = world
            .roster
            .plane(cockpit.plane)
            .is_some_and(|p| p.slot.wing.side == side);
        cockpit_airframe(id, friendly, &cockpit.flight, hp)
    };
    let player = fate(world.cockpits.iter().find(|c| c.plane == plane)?);
    // Every other human-flown plane, then the AI's aircraft.
    let mut aircraft: Vec<Airframe> = world
        .cockpits
        .iter()
        .filter(|c| c.plane != plane)
        .map(fate)
        .collect();
    let mut requirements = Requirements::default();
    if let Some(wings) = world.ai_wings.as_ref() {
        let mission = wings.mission();
        for slot in wings.slots() {
            if aircraft.iter().any(|a| a.id == slot.id) {
                continue;
            }
            aircraft.push(actor_airframe(mission, slot.id, slot.side == side));
        }
        requirements = Requirements::of(wings, plane.0, side);
    }
    Some(report(&Ending {
        ledger: &state.ledger,
        ticks: state.tick(),
        player,
        aircraft,
        wingman: wingman_of(&world.roster, plane),
        requirements,
    }))
}

/// How a plane ended the mission, in the multiplayer results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowStatus {
    Alive,
    /// The pilot escaped.
    Ejected,
    Dead,
    /// Taken out of the mission to make room for a revival
    /// ([`World::retire_plane`]); it was an abandoned wreck.
    Retired,
}

/// One plane's row of the multiplayer results: every plane the mission had,
/// human-flown or not, retired ones included. The host adds the pilots'
/// callsigns.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlaneResult {
    pub plane: PlaneId,
    /// Its side, wing and place in the wing.
    pub slot: Slot,
    pub aircraft: AircraftId,
    pub status: RowStatus,
    /// Airframe damage, 0 to 1; a plane that is not flying shows 1.
    pub damage: f64,
    /// Fighters, bombers and helicopters it shot down (the kill table's
    /// first three rows).
    pub aircraft_kills: u32,
    /// Everything else it destroyed: ships, SAM and AAA sites, vehicles,
    /// structures and the rest.
    pub other_kills: u32,
    /// Aircraft of its own side it shot down.
    pub friendly_fire: u32,
    pub air_to_air: Tally,
    pub gun: Tally,
    /// Air-to-ground missiles and bombs together.
    pub air_to_ground: Tally,
}

/// A plane id no plane has: the results are read from no plane's point of
/// view, so every lost aircraft credits its killer, a human's own included.
const NO_VIEWER: u32 = u32::MAX;

/// Reads every plane's results from the world, in the order a results page
/// lists them: friendly side first, then each wing, then each member. The
/// rows come from the same [`Ending::pilot`] rule each seat's report uses
/// (status, damage, kills by the ten rows summed, friendly fire, shots and
/// hits), read for each plane from its own side. Kills follow the debrief's
/// credit: the ledger's kill, else the last aircraft to hit the lost one.
/// Friendly fire counts by side, as the debrief does (so two humans of one
/// side in a free-for-all count as friends here, which the scores do not).
/// Planes retired for room are rows of status [`RowStatus::Retired`]; a
/// plane whose aircraft the mission cannot name has no row.
pub fn results(world: &World) -> Vec<PlaneResult> {
    let state = &world.combat.state;
    let retired = world.revival.retired();
    let planes: Vec<(Plane, bool)> = world
        .roster
        .planes()
        .iter()
        .map(|p| (*p, false))
        .chain(retired.iter().map(|p| (*p, true)))
        .collect();
    let mission = world.ai_wings.as_ref().map(|wings| wings.mission());
    // Each plane's fate, with `friendly` filled in per viewpoint below.
    let fate = |plane: &Plane, gone: bool| -> Airframe {
        let id = plane.id.0;
        if gone {
            return Airframe {
                id,
                friendly: false,
                alive: false,
                ejected: false,
                damage: 1.,
                landing_grade: None,
                cause: None,
            };
        }
        if let Some(cockpit) = world.cockpits.iter().find(|c| c.plane == plane.id) {
            let hp = state.ownship(id).map_or(0, |own| own.hp);
            return cockpit_airframe(id, false, &cockpit.flight, hp);
        }
        match mission {
            Some(mission) => actor_airframe(mission, id, false),
            None => Airframe {
                id,
                friendly: false,
                alive: false,
                ejected: false,
                damage: 1.,
                landing_grade: None,
                cause: None,
            },
        }
    };
    let fates: Vec<(Airframe, bool)> = planes
        .iter()
        .map(|(p, gone)| (fate(p, *gone), *gone))
        .collect();
    // The mission as each side sees it: its own aircraft are the friendly
    // ones.
    let viewpoint = |viewer: Side| -> Vec<Airframe> {
        fates
            .iter()
            .zip(&planes)
            .map(|((airframe, _), (plane, _))| Airframe {
                friendly: plane.slot.wing.side == viewer,
                ..*airframe
            })
            .collect()
    };
    let from = |viewer: Side| {
        let aircraft = viewpoint(viewer);
        let ending = Ending {
            ledger: &state.ledger,
            ticks: state.tick(),
            player: Airframe {
                id: NO_VIEWER,
                friendly: true,
                alive: true,
                ejected: false,
                damage: 0.,
                landing_grade: None,
                cause: None,
            },
            aircraft,
            wingman: None,
            requirements: Requirements::default(),
        };
        let kills = {
            let fates = ending.fates();
            Standing {
                ledger: ending.ledger,
                plane: NO_VIEWER,
                aircraft: &fates,
                requirements: &ending.requirements,
            }
            .kills()
        };
        (ending, kills)
    };
    let views = [
        (Side::Friendly, from(Side::Friendly)),
        (Side::Enemy, from(Side::Enemy)),
    ];
    let mut rows = Vec::with_capacity(planes.len());
    for ((plane, gone), (airframe, _)) in planes.iter().zip(&fates) {
        let Some(aircraft) = aircraft_of(world, plane) else {
            continue;
        };
        let side = plane.slot.wing.side;
        let (ending, kills) = &views[usize::from(side.is_enemy())].1;
        let fates = ending.fates();
        let standing = Standing {
            ledger: ending.ledger,
            plane: NO_VIEWER,
            aircraft: &fates,
            requirements: &ending.requirements,
        };
        let airframe = Airframe {
            friendly: true,
            ..*airframe
        };
        let pilot = ending.pilot(&airframe, &standing, kills);
        let sum = |rows: &[usize]| rows.iter().map(|row| pilot.kills[*row]).sum::<u32>();
        let merged = |a: Tally, b: Tally| Tally {
            launched: a.launched + b.launched,
            hit: a.hit + b.hit,
            damage: a.damage + b.damage,
            ..Tally::default()
        };
        rows.push(PlaneResult {
            plane: plane.id,
            slot: plane.slot,
            aircraft,
            status: if *gone {
                RowStatus::Retired
            } else {
                match pilot.status {
                    Status::Alive => RowStatus::Alive,
                    Status::Ejected => RowStatus::Ejected,
                    Status::Dead => RowStatus::Dead,
                }
            },
            damage: pilot.damage,
            aircraft_kills: sum(&[0, 1, 2]),
            other_kills: sum(&[3, 4, 5, 6, 7, 8, 9]),
            friendly_fire: pilot.friendly_fire,
            air_to_air: pilot.air_to_air,
            gun: pilot.gun,
            air_to_ground: merged(pilot.air_to_ground, pilot.bombs),
        });
    }
    rows.sort_by_key(|row| {
        (
            row.slot.wing.side.is_enemy(),
            row.slot.wing.index,
            row.slot.member,
            row.plane,
        )
    });
    rows
}

/// The aircraft `plane` is: the AI bridge's slot, else a human's ownship.
/// A retired wreck has neither left, so it is what its wing flies: the
/// mission's launch for that wing, else another plane of the wing (a wing
/// flies one aircraft, and a revival keeps the type).
fn aircraft_of(world: &World, plane: &Plane) -> Option<AircraftId> {
    let own = |id: u32| {
        world
            .ai_wings
            .as_ref()
            .and_then(|wings| wings.slot(id))
            .map(|slot| slot.aircraft)
            .or_else(|| {
                world
                    .combat
                    .state
                    .ownship(id)
                    .map(|own| own.configuration().aircraft)
            })
    };
    own(plane.id.0)
        .or_else(|| {
            world
                .setup
                .ai
                .as_ref()
                .and_then(|ai| ai.wings.iter().find(|w| w.wing == plane.slot.wing))
                .map(|wing| wing.aircraft)
        })
        .or_else(|| {
            world
                .roster
                .planes()
                .iter()
                .filter(|other| other.slot.wing == plane.slot.wing)
                .find_map(|other| own(other.id.0))
        })
}

pub fn report(end: &Ending) -> Report {
    let fates = end.fates();
    let standing = Standing {
        ledger: end.ledger,
        plane: end.player.id,
        aircraft: &fates,
        requirements: &end.requirements,
    };
    let kills = standing.kills();
    let mut objectives = Vec::new();
    if !end.requirements.destroy.is_empty() {
        objectives.push(Objective::Destroy {
            destroyed: standing.destroyed(),
            total: end.requirements.destroy.len() as u32,
        });
    }
    if !end.requirements.protect.is_empty() {
        objectives.push(Objective::Protect {
            protected: standing.protected(),
            total: end.requirements.protect.len() as u32,
        });
    }
    let player = end.pilot(&end.player, &standing, &kills);
    Report {
        outcome: if standing.succeeded() {
            Outcome::Success
        } else {
            Outcome::Failure
        },
        objectives,
        elapsed_seconds: end.ticks / 120,
        player,
        wingman: end
            .wingman
            .and_then(|id| end.airframe(id))
            .map(|a| end.pilot(a, &standing, &kills)),
    }
}

impl Report {
    /// One line for headless probes: outcome, objectives and both columns.
    pub fn summary(&self) -> String {
        let pilot = |p: &Pilot| {
            format!(
                "{:?} damage={:.0}% kills={:?} ff={} a2a={}/{} dmg={} gun={}/{} enemy_aam={}/{} enemy_gun={}/{}{}",
                p.status,
                p.damage * 100.,
                p.kills,
                p.friendly_fire,
                p.air_to_air.hit,
                p.air_to_air.launched,
                p.air_to_air.damage,
                p.gun.hit,
                p.gun.launched,
                p.enemy_aam.hit,
                p.enemy_aam.launched,
                p.enemy_gun.hit,
                p.enemy_gun.launched,
                p.cause.map_or_else(String::new, |c| format!(" cause={c}")),
            )
        };
        format!(
            "{} {:?} elapsed={}s player[{}] wingman[{}]",
            self.outcome.label(),
            self.objectives,
            self.elapsed_seconds,
            pilot(&self.player),
            self.wingman.as_ref().map_or_else(|| "-".into(), pilot),
        )
    }
    /// The retail reference case: a failed Quick Mission flown alone for one
    /// second with nothing fired. Used by `--snapshot-state debrief-N`.
    pub fn sample() -> Self {
        Self {
            outcome: Outcome::Failure,
            objectives: vec![Objective::Destroy {
                destroyed: 0,
                total: 3,
            }],
            elapsed_seconds: 1,
            player: Pilot::default(),
            wingman: None,
        }
    }
}

#[cfg(test)]
mod results_tests;

#[cfg(test)]
mod tests {
    use super::*;
    fn airframe(id: u32, friendly: bool, alive: bool) -> Airframe {
        Airframe {
            id,
            friendly,
            alive,
            ejected: false,
            damage: 0.25,
            landing_grade: None,
            cause: None,
        }
    }
    fn ending(ledger: &Ledger) -> Ending<'_> {
        Ending {
            ledger,
            ticks: 125 * 120 + 60,
            player: airframe(0, true, true),
            aircraft: vec![
                airframe(1, true, true),
                airframe(2, true, true),
                airframe(10, false, false),
                airframe(11, false, true),
            ],
            wingman: Some(1),
            requirements: Requirements {
                destroy: vec![10, 11],
                protect: vec![],
            },
        }
    }
    #[test]
    fn an_out_of_bounds_enemy_is_a_lost_aircraft_credited_to_nobody() {
        use tore_sim::aircraft_systems::LossCause;
        use tore_sim::combat::ledger::Kill;
        let mut ledger = Ledger::default();
        // The player had hit enemy 10 earlier, then it flew off the map.
        ledger.damaged(Kill {
            owner: 0,
            victim: 10,
            category: 0x8000,
            aircraft: true,
        });
        ledger.lose_without_credit(10);
        let mut end = ending(&ledger);
        end.aircraft[2].cause = Some(LossCause::OutOfBounds);
        let plain = report(&end);
        assert_eq!(plain.player.kills, [0; 10]);
        end.player.alive = false;
        end.player.cause = Some(LossCause::Overspeed);
        let lost = report(&end);
        assert_eq!(lost.player.status, Status::Dead);
        assert_eq!(lost.player.cause, Some("overspeed"));
        assert!(lost.summary().contains("cause=overspeed"));
    }
    #[test]
    fn overspeed_and_edge_losses_credit_no_kill_for_the_player_or_an_enemy() {
        use tore_sim::aircraft_systems::LossCause;
        use tore_sim::combat::ledger::Kill;
        let mut ledger = Ledger::default();
        // The player hit enemy 10 earlier, and enemy 10 had hit the player.
        for (owner, victim) in [(0, 10), (10, 0)] {
            ledger.damaged(Kill {
                owner,
                victim,
                category: 0x8000,
                aircraft: true,
            });
        }
        // Enemy 10 is then lost to overspeed or the map edge alike:
        // the ledger holds no credit for it.
        ledger.lose_without_credit(10);
        let mut end = ending(&ledger);
        end.aircraft[2].cause = Some(LossCause::Overspeed);
        // The player is lost to overspeed too; a lost player is never credited
        // to the last aircraft that hit it.
        end.player.alive = false;
        end.player.cause = Some(LossCause::Overspeed);
        let report = report(&end);
        assert_eq!(report.player.kills, [0; 10]);
        assert_eq!(report.player.status, Status::Dead);
        let fates = end.fates();
        let standing = Standing {
            ledger: end.ledger,
            plane: end.player.id,
            aircraft: &fates,
            requirements: &end.requirements,
        };
        assert!(
            standing
                .kills()
                .iter()
                .all(|k| k.victim != 10 && k.victim != 0)
        );
    }
    #[test]
    fn a_surviving_target_or_friendly_kill_fails_the_mission() {
        use tore_sim::combat::ledger::{Kill, Resolution};
        let mut ledger = Ledger::default();
        let first = report(&ending(&ledger));
        assert_eq!(first.outcome, Outcome::Failure);
        assert_eq!(
            first.objectives,
            [Objective::Destroy {
                destroyed: 1,
                total: 2
            }]
        );
        assert_eq!(first.elapsed_seconds, 125);
        let mut all_down = ending(&ledger);
        all_down.aircraft[3].alive = false;
        assert_eq!(report_outcome(&all_down), Outcome::Success);
        // Shooting down a friendly turns the same result into a failure.
        ledger.launch(5, 0, Some(2), ShotKind::AirToAir);
        ledger.resolve(5, Resolution::Hit(90));
        ledger.kill(Kill {
            owner: 0,
            victim: 2,
            category: 0x8000,
            aircraft: true,
        });
        let mut all_down = ending(&ledger);
        all_down.aircraft[3].alive = false;
        let result = report(&all_down);
        assert_eq!(result.outcome, Outcome::Failure);
        assert_eq!(result.player.friendly_fire, 1);
        assert_eq!(result.player.kills, [0; 10]);
    }
    fn report_outcome(end: &Ending) -> Outcome {
        report(end).outcome
    }
    #[test]
    fn wingman_column_and_enemy_fire_follow_owner_and_aim() {
        use tore_sim::combat::ledger::{Kill, Resolution};
        let mut ledger = Ledger::default();
        // The wingman downs a bomber; an enemy guns the wingman and fires a
        // missile at the player.
        ledger.kill(Kill {
            owner: 1,
            victim: 10,
            category: 0x4000,
            aircraft: true,
        });
        ledger.aim(20, 1);
        ledger.launch(20, 11, None, ShotKind::Gun);
        ledger.resolve(20, Resolution::Hit(15));
        ledger.launch(21, 11, Some(0), ShotKind::AirToAir);
        let mut end = ending(&ledger);
        end.player.alive = false;
        let result = report(&end);
        let wingman = result.wingman.as_ref().unwrap();
        assert_eq!(wingman.kills[1], 1);
        assert_eq!(wingman.enemy_gun.hit, 1);
        assert_eq!(wingman.enemy_gun.damage, 15);
        assert_eq!(result.player.enemy_aam.launched, 1);
        assert_eq!(result.player.enemy_gun.launched, 0);
        assert_eq!(result.player.status, Status::Dead);
        assert_eq!(result.player.damage, 1.);
        assert_eq!(wingman.damage, 0.25);
        // An enemy whose pilot ejects after the player's hit is the player's
        // kill; an ejected wingman shows as ejected, with full damage.
        let mut ledger = Ledger::default();
        ledger.damaged(Kill {
            owner: 0,
            victim: 11,
            category: 0x8000,
            aircraft: true,
        });
        let mut end = ending(&ledger);
        end.aircraft[3].alive = false;
        end.aircraft[3].ejected = true;
        end.aircraft[0].alive = false;
        end.aircraft[0].ejected = true;
        let result = report(&end);
        assert_eq!(result.player.kills[0], 1);
        assert_eq!(result.outcome, Outcome::Success);
        let wingman = result.wingman.unwrap();
        assert_eq!(wingman.status, Status::Ejected);
        assert_eq!(wingman.damage, 1.);
        let alone = report(&Ending {
            wingman: None,
            ..ending(&ledger)
        });
        assert!(alone.wingman.is_none());
    }
    /// Two humans in Friendly Wing 1 (plane 0, the lead, is seat 0; plane 2 is
    /// seat 1 and the wing's second member), an AI third member (plane 1) and
    /// an enemy pair (10, 11).
    fn two_seat_roster() -> Roster {
        use tore_sim::ai::launch::{Side, WingId};
        let slot = |side, member| Slot {
            wing: WingId { side, index: 0 },
            member,
        };
        Roster::with_humans(
            [
                (PlaneId(0), slot(Side::Friendly, 0), SeatId(0), None),
                (PlaneId(2), slot(Side::Friendly, 1), SeatId(1), None),
            ],
            [
                (PlaneId(1), slot(Side::Friendly, 2)),
                (PlaneId(10), slot(Side::Enemy, 0)),
                (PlaneId(11), slot(Side::Enemy, 1)),
            ],
        )
    }
    /// The debrief of the plane `plane` in the two seat mission: every other
    /// aircraft is a member of the ending, friendly when it flies the same
    /// side. Enemy 11 is the only aircraft still flying.
    fn seat_ending<'a>(ledger: &'a Ledger, roster: &Roster, plane: u32) -> Ending<'a> {
        let side = roster.plane(PlaneId(plane)).unwrap().slot.wing.side;
        let frame = |id: u32| {
            let p = roster.plane(PlaneId(id)).unwrap();
            airframe(id, p.slot.wing.side == side, id == 11)
        };
        let requirements = Requirements {
            destroy: [10, 11]
                .into_iter()
                .filter(|id| roster.plane(PlaneId(*id)).unwrap().slot.wing.side != side)
                .collect(),
            protect: vec![],
        };
        Ending {
            ledger,
            ticks: 600,
            player: airframe(plane, true, true),
            aircraft: [0, 1, 2, 10, 11]
                .into_iter()
                .filter(|id| *id != plane)
                .map(frame)
                .collect(),
            wingman: wingman_of(roster, PlaneId(plane)),
            requirements,
        }
    }
    #[test]
    fn the_wingman_is_the_first_other_member_of_the_planes_own_wing() {
        let roster = two_seat_roster();
        // Seat 0 keeps the lead's wingman, now the other human.
        assert_eq!(wingman_of(&roster, PlaneId(0)), Some(2));
        // Seat 1's is the lead, not the AI member and not an enemy.
        assert_eq!(wingman_of(&roster, PlaneId(2)), Some(0));
        assert_eq!(wingman_of(&roster, PlaneId(1)), Some(0));
        assert_eq!(wingman_of(&roster, PlaneId(10)), Some(11));
        // Alone in its wing: nobody.
        let alone = Roster::single_player(None, []);
        assert_eq!(wingman_of(&alone, PlaneId(0)), None);
        // Single player as it is today: the lowest AI member of the wing.
        use tore_sim::ai::launch::{Side, WingId};
        let wing = WingId {
            side: Side::Friendly,
            index: 0,
        };
        let solo = Roster::single_player(
            None,
            [
                (PlaneId(3), Slot { wing, member: 2 }),
                (PlaneId(1), Slot { wing, member: 1 }),
            ],
        );
        assert_eq!(wingman_of(&solo, PlaneId(0)), Some(1));
    }
    #[test]
    fn a_seats_debrief_puts_its_own_plane_in_the_pilot_column() {
        use tore_sim::combat::ledger::Kill;
        let roster = two_seat_roster();
        let mut ledger = Ledger::default();
        // Seat 1's plane shoots down enemy 10; the lead fires its gun at
        // enemy 11 and gets no kill.
        ledger.launch(1, 2, Some(10), ShotKind::AirToAir);
        ledger.launch(2, 0, Some(11), ShotKind::Gun);
        ledger.kill(Kill {
            owner: 2,
            victim: 10,
            category: 0x8000,
            aircraft: true,
        });
        let second = report(&seat_ending(&ledger, &roster, 2));
        let first = report(&seat_ending(&ledger, &roster, 0));
        // Seat 1: its own shot and kill in the pilot column, the lead's gun in
        // the wingman column.
        assert_eq!(second.player.kills[0], 1);
        assert_eq!(second.player.air_to_air.launched, 1);
        assert_eq!(second.player.gun.launched, 0);
        assert_eq!(second.wingman.as_ref().unwrap().gun.launched, 1);
        assert_eq!(second.wingman.as_ref().unwrap().kills, [0; 10]);
        // Seat 0 reads the same ledger from the other side.
        assert_eq!(first.player.kills, [0; 10]);
        assert_eq!(first.player.gun.launched, 1);
        assert_eq!(first.wingman.as_ref().unwrap().kills[0], 1);
        // Both must still destroy enemy 11.
        let open = [Objective::Destroy {
            destroyed: 1,
            total: 2,
        }];
        assert_eq!(second.objectives, open);
        assert_eq!(first.objectives, open);
    }
    #[test]
    fn friendly_fire_counts_only_the_kills_the_seats_own_plane_made() {
        use tore_sim::combat::ledger::Kill;
        let roster = two_seat_roster();
        let mut ledger = Ledger::default();
        // Seat 1's plane shoots down the AI member of its own wing.
        ledger.kill(Kill {
            owner: 2,
            victim: 1,
            category: 0x8000,
            aircraft: true,
        });
        let ending = |plane| {
            let mut end = seat_ending(&ledger, &roster, plane);
            // Every enemy is down, so only friendly fire can fail the mission.
            for a in end.aircraft.iter_mut().filter(|a| !a.friendly) {
                a.alive = false;
            }
            end
        };
        let (second, first) = (report(&ending(2)), report(&ending(0)));
        assert_eq!(second.player.friendly_fire, 1);
        assert_eq!(second.outcome, Outcome::Failure);
        assert_eq!(first.player.friendly_fire, 0);
        assert_eq!(first.outcome, Outcome::Success);
        assert_eq!(first.wingman.unwrap().friendly_fire, 1);
    }
    #[test]
    fn a_cockpits_airframe_reads_its_own_flight_and_ownship() {
        let mut flight =
            tore_sim::flight::State::new(&crate::test_support::profile(), [0., 5000., 0.]).unwrap();
        flight.damage_fraction = 0.4;
        let flying = cockpit_airframe(2, true, &flight, 500);
        assert_eq!((flying.id, flying.alive, flying.ejected), (2, true, false));
        assert_eq!(flying.damage, 0.4);
        // No hit points left: shot down, though the flight itself is intact.
        assert!(!cockpit_airframe(2, true, &flight, 0).alive);
        // The pilot escaped: ejected, not alive; a dead pilot is never ejected.
        flight.systems.pilot.ejected = true;
        let out = cockpit_airframe(2, true, &flight, 500);
        assert_eq!((out.alive, out.ejected), (false, true));
        flight.systems.pilot.dead = true;
        let dead = cockpit_airframe(2, true, &flight, 500);
        assert_eq!((dead.alive, dead.ejected), (false, false));
        flight.systems.pilot = Default::default();
        flight.crashed = true;
        assert!(!cockpit_airframe(2, true, &flight, 500).alive);
    }
    #[test]
    fn kill_rows_take_the_first_matching_class_bit() {
        assert_eq!(kill_row(0x8000), Some(0));
        assert_eq!(kill_row(0x4000), Some(1));
        assert_eq!(kill_row(0x2100), Some(3));
        assert_eq!(kill_row(0x100), Some(8));
        assert_eq!(kill_row(0x40), Some(9));
        assert_eq!(kill_row(0x1), None);
    }
    #[test]
    fn objective_sentences_follow_counts() {
        let destroy = |destroyed, total| Objective::Destroy { destroyed, total }.sentence();
        assert_eq!(destroy(0, 3), "Destroyed 0 of 3 targets.");
        assert_eq!(destroy(3, 3), "Destroyed the 3 targets.");
        assert_eq!(destroy(1, 1), "Destroyed the target.");
        assert_eq!(destroy(0, 1), "Failed to destroy the target.");
        let protect = |protected, total| Objective::Protect { protected, total }.sentence();
        assert_eq!(protect(1, 2), "Protected 1 of 2 friendly objectives.");
        assert_eq!(protect(0, 1), "Failed to protect the friendly objective.");
    }
}
