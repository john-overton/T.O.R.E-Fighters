//! Heavy flak fights as a team (docs/spec/surface-defenses.md, "Flak teams";
//! John, 2026-10-14: "let's give flak a 'group logic' kind of how supply
//! trucks work, but give them double the detection range, and have the flak
//! attach and detect and work as a team within 1 mile of each other, and have
//! it attack the entire detected group once 25% of the detected group is in
//! range of the entire group", amended the same day: "or heading is within a
//! 90 [degree] cone of the known blob").
//!
//! - **Team.** The KS-12 and KS-19 guns of a side (land guns whose shells
//!   burst in the air, [`super::Kind::Gun`] with `flak`) form teams when the
//!   arsenal is read: a gun joins every team that has a gun within
//!   [`FLAK_TEAM_RADIUS_FT`] of it, so a chain of guns each a mile from the
//!   next is one team (agent decision). A gun with no other within a mile is a
//!   team of one and follows the same rules. Teams are fixed for the mission;
//!   a destroyed gun drops out and the rest stay a team.
//! - **Shared detection.** The team sees every hostile aircraft any of its
//!   live guns sees within twice the gun's detection range (`zone0`'s
//!   range, and `searchDist` where the unit has one; the altitude band is
//!   kept), with terrain line of sight from that gun. Every gun of the team
//!   knows all of them.
//! - **Groups.** The detected aircraft are split into groups: aircraft within
//!   [`FLAK_GROUP_RADIUS_FT`] (2 nm) of another of the group, transitively
//!   (agent decision). A flight in formation is one group; two flights far
//!   apart are two.
//! - **Holding and opening fire.** The team holds its fire at a group until
//!   either at least [`FLAK_GROUP_IN_RANGE_PERCENT`] percent of the group's
//!   aircraft are in range of the entire team (inside every live gun's fire
//!   zone, range and altitude band), or the group heads at the team: the
//!   mean ground track of its members lies within [`FLAK_HEADING_CONE_DEG`]
//!   degrees of the bearing from the group's middle to the team's middle,
//!   with at least one member inside some gun's fire zone. Once a gun of the
//!   team has begun firing at a member of a group (a burst begun, the pause
//!   after it or a magazine swap), the team keeps engaging that group until
//!   no gun is firing at any of it. While it holds, the guns still choose
//!   targets, prepare and lock on ([`tore_sim::ai::surface::Inputs::hold`]),
//!   so they fire the moment the team engages, but none fires before.
//! - **Spreading fire.** The team gives each gun a member of the groups it
//!   engages (while it holds, of every group it sees): the guns that reach
//!   farthest first (they open fire first), each the next member by id in
//!   turn, round again when every member has one ([`assign`]); a team with
//!   fewer guns than members moves the turn on one member every 10 s
//!   ([`FLAK_ROTATION_TICKS`]), so a lone gun walks over a flight. A gun then
//!   chooses ([`TeamPlan::order`]) its own member when it is within its
//!   reach or [`FLAK_REACH_MARGIN_FT`] past it, else any member it can
//!   reach, else its own, else any nearly in reach; the nearest, then the
//!   lowest id, first among equals. It picks again
//!   when its target dies or leaves and at flak's retarget period (10 s), so
//!   the fire stays spread over the group rather than on the lead.
//!
//! Everything is a function of this tick's positions and the controllers'
//! checkpointed state, read in ascending unit id, so the player, the AI, a
//! networked host and a replay all see the same teams and the same fire.
//! Nothing is added to the checkpoint or the wire.
use super::{
    Arms, Band, Contact, Kind, Place, Scene, SurfaceState, Trace, UnitId, WeaponArms, length,
    line_of_sight, sub, wanted,
};
use crate::surface::fire::Arsenal;
use std::collections::{BTreeMap, BTreeSet};
use tore_sim::{ai::surface::Phase, attitude::Vector, combat::live::Side};

/// Flak guns within this horizontal distance of each other fight as one
/// team: 1 nautical mile (opinionated, John, 2026-10-14: "within 1 mile of
/// each other"; the nautical mile is the lead's reading).
pub const FLAK_TEAM_RADIUS_FT: f64 = 6_076.;
/// A team detects at this multiple of its guns' own detection range
/// (opinionated, John, 2026-10-14: "double the detection range").
pub const FLAK_TEAM_DETECTION_FACTOR: f64 = 2.;
/// Detected aircraft within this distance of another of a group belong to it
/// (2 nm; agent decision: a flight in formation is well inside it, two
/// flights on different tracks are usually outside it).
pub const FLAK_GROUP_RADIUS_FT: f64 = 2. * 6_076.;
/// The share of a group that must be in range of the entire team before the
/// team opens fire on it, percent (opinionated, John, 2026-10-14).
pub const FLAK_GROUP_IN_RANGE_PERCENT: usize = 25;
/// A group heads at the team when its track is within this many degrees
/// either side of the bearing to the team: a 90 degree cone (opinionated,
/// John, 2026-10-14, "a 90% cone", read as degrees).
pub const FLAK_HEADING_CONE_DEG: f64 = 45.;
/// A group slower than this over the ground (30 kt) has no track to judge,
/// feet a second (agent decision: a hovering helicopter group).
pub const FLAK_HEADING_MIN_SPEED_FPS: f64 = 50.;

/// A gun prefers targets within its reach, or this much past it, feet: a
/// nautical mile, so the members of a formation reaching the gun count
/// alike and the fire spreads over them instead of all landing on the front
/// one, the lead (agent decision).
pub const FLAK_REACH_MARGIN_FT: f64 = 6_076.;

/// A team with fewer guns than a group has members moves each gun on to the
/// next member this often, ticks: 10 s, flak's retarget period (`retargetT`
/// 40), when its gun chooses again (agent decision). A lone gun so walks its
/// fire over a flight instead of staying on the lead.
pub const FLAK_ROTATION_TICKS: u64 = 10 * 120;

/// One flak team: guns of one side within a mile of each other, chained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlakTeam {
    pub side: Side,
    /// Ascending id.
    pub guns: Vec<UnitId>,
}

/// One detected group as the trace reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlakGroup {
    /// The group's aircraft, ascending id.
    pub members: Vec<u32>,
    /// How many are in range of every live gun of the team.
    pub in_range: u32,
    /// Its track points at the team.
    pub heading_in: bool,
    /// A gun of the team already fights one of it.
    pub firing: bool,
    /// The team engages it.
    pub engaged: bool,
}

/// The flak weapon of a unit, with its index: a land gun whose shells burst
/// in the air, fired by its own controller.
pub(super) fn flak_weapon(arms: &Arms) -> Option<(usize, &WeaponArms)> {
    if arms.ship || arms.battery.is_some() {
        return None;
    }
    arms.weapons
        .iter()
        .enumerate()
        .find(|(_, w)| matches!(w.kind, Kind::Gun { flak: true, .. }) && !w.battery)
}

fn root(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

/// Joins the items `0..n` that `near` pairs, transitively; returns each
/// item's set as the lowest item in it.
fn link(n: usize, near: impl Fn(usize, usize) -> bool) -> Vec<usize> {
    let mut parent: Vec<usize> = (0..n).collect();
    for i in 0..n {
        for j in i + 1..n {
            if near(i, j) {
                let (a, b) = (root(&mut parent, i), root(&mut parent, j));
                if a != b {
                    parent[a.max(b)] = a.min(b);
                }
            }
        }
    }
    (0..n).map(|i| root(&mut parent, i)).collect()
}

/// The teams of an arsenal's flak guns (`units` in ascending id): guns of
/// one side chained by [`FLAK_TEAM_RADIUS_FT`] on the ground plane, from
/// where the mission placed them. In order of their first gun.
pub fn flak_teams(units: &[Arms]) -> Vec<FlakTeam> {
    let guns: Vec<&Arms> = units
        .iter()
        .filter(|arms| flak_weapon(arms).is_some())
        .collect();
    let sets = link(guns.len(), |i, j| {
        let (a, b) = (guns[i], guns[j]);
        a.side == b.side
            && (a.position[0] - b.position[0]).hypot(a.position[2] - b.position[2])
                <= FLAK_TEAM_RADIUS_FT
    });
    let mut teams: BTreeMap<usize, FlakTeam> = BTreeMap::new();
    for (gun, set) in guns.iter().zip(sets) {
        teams
            .entry(set)
            .or_insert_with(|| FlakTeam {
                side: gun.side,
                guns: Vec::new(),
            })
            .guns
            .push(gun.unit);
    }
    teams.into_values().collect()
}

/// A live gun of a team as the plan keeps it: its flak controller, where it
/// looks from and where it can nearly reach.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct PlanGun {
    pub unit: UnitId,
    /// Its flak weapon's index in its arms (its controller).
    pub index: usize,
    pub eye: Vector,
    /// Its fire zone.
    pub reach: Band,
    /// Its fire zone with [`FLAK_REACH_MARGIN_FT`] added to the range.
    pub near: Band,
    /// The member of the group the team gives it ([`assign`]).
    pub assigned: Option<u32>,
}

impl PlanGun {
    fn nearly_reaches(&self, at: Vector) -> bool {
        let d = sub(at, self.eye);
        self.near.contains(length(d), d[1])
    }
    fn reaches(&self, at: Vector) -> bool {
        let d = sub(at, self.eye);
        self.reach.contains(length(d), d[1])
    }
    /// How much the gun wants a member at `at`, 0 most: its own member when
    /// it nearly reaches it, any member it reaches, its own member, any
    /// member it nearly reaches, the rest.
    fn tier(&self, id: u32, at: Vector) -> u8 {
        let own = self.assigned == Some(id);
        let near = self.nearly_reaches(at);
        match (own && near, self.reaches(at), own, near) {
            (true, ..) => 0,
            (_, true, ..) => 1,
            (_, _, true, _) => 2,
            (_, _, _, true) => 3,
            _ => 4,
        }
    }
}

/// One team's picture this tick.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct TeamPlan {
    /// Its live guns, ascending id.
    pub guns: Vec<PlanGun>,
    /// Every hostile aircraft the team sees, ascending id.
    pub detected: Vec<u32>,
    /// The aircraft of the groups it engages.
    pub engaged: BTreeSet<u32>,
}

impl TeamPlan {
    /// The targets gun `unit` of the team may choose, best first: the
    /// engaged groups' aircraft while there are any, else every detected one
    /// (so the guns choose and prepare while the team holds its fire): the
    /// member the team gives it ([`assign`]) when it can nearly reach it
    /// ([`FLAK_REACH_MARGIN_FT`]), then any it can reach, then its own, then
    /// any it can nearly reach, then the rest; the nearest, then the lowest
    /// id, first within each.
    pub fn order(&self, unit: UnitId, by_id: &BTreeMap<u32, Contact>) -> Vec<u32> {
        let Some(me) = self.guns.iter().find(|g| g.unit == unit) else {
            return Vec::new();
        };
        let pool: Vec<u32> = if self.engaged.is_empty() {
            self.detected.clone()
        } else {
            self.engaged.iter().copied().collect()
        };
        let mut keyed: Vec<(u8, f64, u32)> = pool
            .into_iter()
            .filter_map(|id| {
                let contact = by_id.get(&id)?;
                Some((
                    me.tier(id, contact.position),
                    length(sub(contact.position, me.eye)),
                    id,
                ))
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.cmp(&b.2)));
        keyed.into_iter().map(|(_, _, id)| id).collect()
    }
}

/// Gives each gun of `guns` a member of `pool` (aircraft ids, ascending):
/// the guns that reach farthest first (ties by id), each the next member in
/// turn, starting again from the first when every member has a gun. So a
/// four-gun team over a flight of two puts two guns on each, and the guns
/// that open fire first (the longest reach) are on different members. With
/// fewer guns than members the turn starts `rotation` members on, so the
/// guns move over the whole group as it rises ([`FLAK_ROTATION_TICKS`]).
fn assign(guns: &mut [PlanGun], pool: &[u32], rotation: u64) {
    let shift = if guns.len() < pool.len() {
        usize::try_from(rotation % pool.len() as u64).unwrap_or(0)
    } else {
        0
    };
    let mut by_reach: Vec<usize> = (0..guns.len()).collect();
    by_reach.sort_by(|&a, &b| {
        guns[b]
            .near
            .max_range
            .total_cmp(&guns[a].near.max_range)
            .then(guns[a].unit.cmp(&guns[b].unit))
    });
    for (k, gun) in by_reach.into_iter().enumerate() {
        guns[gun].assigned = (!pool.is_empty()).then(|| pool[(k + shift) % pool.len()]);
    }
}

/// A live gun of a team, where it stands this tick.
struct TeamGun<'a> {
    arms: &'a Arms,
    index: usize,
    weapon: &'a WeaponArms,
    place: Place,
}

impl TeamGun<'_> {
    /// The gun sees `contact` with the team's doubled detection.
    fn detects(&self, contact: &Contact, scene: &Scene<'_>) -> bool {
        if contact.surface || !wanted(self.arms.side, self.arms.react, self.weapon, contact) {
            return false;
        }
        let from = self.place.eye;
        let d = sub(contact.position, from);
        let distance = length(d);
        let band = self.weapon.detection;
        distance >= band.min_range
            && distance <= band.max_range * FLAK_TEAM_DETECTION_FACTOR
            && d[1] >= band.min_altitude
            && d[1] <= band.max_altitude
            && self
                .arms
                .search_limit
                .is_none_or(|limit| distance <= limit * FLAK_TEAM_DETECTION_FACTOR)
            && line_of_sight(from, contact.position, scene.ground)
    }
    /// `contact` is inside the gun's fire zone (range and altitude band).
    fn reaches(&self, contact: &Contact) -> bool {
        let d = sub(contact.position, self.place.eye);
        self.weapon.launch.contains(length(d), d[1])
    }
}

/// Whether a group moving at `track` (feet a second, its members' mean) from
/// `middle` heads at `team` (both feet).
fn heads_at(track: Vector, middle: Vector, team: Vector) -> bool {
    let speed = track[0].hypot(track[2]);
    if speed < FLAK_HEADING_MIN_SPEED_FPS {
        return false;
    }
    let to = [team[0] - middle[0], team[2] - middle[2]];
    let reach = to[0].hypot(to[1]);
    if reach < 1. {
        // Over the team already.
        return true;
    }
    let cos = (track[0] * to[0] + track[2] * to[1]) / (speed * reach);
    cos >= FLAK_HEADING_CONE_DEG.to_radians().cos()
}

/// Every team's picture this tick, by index in `arsenal.flak_teams`, and a
/// trace line for each team whose groups or decisions changed.
pub(super) fn plan_teams(
    arsenal: &Arsenal,
    state: &mut SurfaceState,
    alive: &dyn Fn(UnitId) -> bool,
    contacts: &[Contact],
    scene: &Scene<'_>,
) -> Vec<TeamPlan> {
    let mut plans = Vec::with_capacity(arsenal.flak_teams.len());
    for (team, roster) in arsenal.flak_teams.iter().enumerate() {
        let guns: Vec<TeamGun<'_>> = roster
            .guns
            .iter()
            .filter(|id| alive(**id))
            .filter_map(|id| arsenal.arms(*id))
            .filter(|arms| state.unit(arms.unit).is_some_and(|u| u.armed))
            .filter_map(|arms| {
                let (index, weapon) = flak_weapon(arms)?;
                Some(TeamGun {
                    arms,
                    index,
                    weapon,
                    place: state.place(arms),
                })
            })
            .collect();
        let mut plan = TeamPlan {
            guns: guns
                .iter()
                .map(|g| {
                    let mut near = g.weapon.launch;
                    near.max_range += FLAK_REACH_MARGIN_FT;
                    PlanGun {
                        unit: g.arms.unit,
                        index: g.index,
                        eye: g.place.eye,
                        reach: g.weapon.launch,
                        near,
                        assigned: None,
                    }
                })
                .collect(),
            ..TeamPlan::default()
        };
        let mut groups_seen = Vec::new();
        if !guns.is_empty() {
            let mut detected: Vec<&Contact> = contacts
                .iter()
                .filter(|c| guns.iter().any(|g| g.detects(c, scene)))
                .collect();
            detected.sort_by_key(|c| c.id);
            plan.detected = detected.iter().map(|c| c.id).collect();
            let sets = link(detected.len(), |i, j| {
                length(sub(detected[i].position, detected[j].position)) <= FLAK_GROUP_RADIUS_FT
            });
            let mut groups: BTreeMap<usize, Vec<&Contact>> = BTreeMap::new();
            for (contact, set) in detected.iter().zip(sets) {
                groups.entry(set).or_default().push(contact);
            }
            let count = guns.len() as f64;
            let middle_of = |points: &mut dyn Iterator<Item = Vector>, n: f64| -> Vector {
                let mut sum = [0.; 3];
                for p in points {
                    for i in 0..3 {
                        sum[i] += p[i];
                    }
                }
                sum.map(|v| v / n)
            };
            let team_middle = middle_of(&mut guns.iter().map(|g| g.place.origin), count);
            // The members a gun of the team has begun firing at.
            let firing: BTreeSet<u32> = guns
                .iter()
                .filter_map(|g| {
                    let controller = &state.unit(g.arms.unit)?.engagers.get(g.index)?.controller;
                    matches!(
                        controller.phase(),
                        Phase::Fire | Phase::Pause | Phase::Reload
                    )
                    .then(|| controller.target())
                    .flatten()
                })
                .collect();
            for members in groups.values() {
                let n = members.len();
                let in_range = members
                    .iter()
                    .filter(|c| guns.iter().all(|g| g.reaches(c)))
                    .count();
                let any = members.iter().any(|c| guns.iter().any(|g| g.reaches(c)));
                let middle = middle_of(&mut members.iter().map(|c| c.position), n as f64);
                let track = middle_of(&mut members.iter().map(|c| c.velocity), n as f64);
                let heading_in = heads_at(track, middle, team_middle);
                let fighting = members.iter().any(|c| firing.contains(&c.id));
                let quarter = in_range > 0 && in_range * 100 >= n * FLAK_GROUP_IN_RANGE_PERCENT;
                let engaged = fighting || (any && (quarter || heading_in));
                if engaged {
                    plan.engaged.extend(members.iter().map(|c| c.id));
                }
                groups_seen.push(FlakGroup {
                    members: members.iter().map(|c| c.id).collect(),
                    in_range: in_range as u32,
                    heading_in,
                    firing: fighting,
                    engaged,
                });
            }
        }
        let pool: Vec<u32> = if plan.engaged.is_empty() {
            plan.detected.clone()
        } else {
            plan.engaged.iter().copied().collect()
        };
        assign(&mut plan.guns, &pool, scene.tick / FLAK_ROTATION_TICKS);
        // The trace: a line when the team's groups or what it does about
        // them change (not when only the counts in range do).
        let summary: Vec<(Vec<u32>, bool)> = groups_seen
            .iter()
            .map(|g| (g.members.clone(), g.engaged))
            .collect();
        let before = state.flak_teams.get(&team);
        if before.map_or(!summary.is_empty(), |b| *b != summary) {
            state.trace.push(Trace::FlakTeam {
                team,
                guns: plan.guns.iter().map(|g| g.unit).collect(),
                groups: groups_seen,
            });
        }
        if summary.is_empty() {
            state.flak_teams.remove(&team);
        } else {
            state.flak_teams.insert(team, summary);
        }
        plans.push(plan);
    }
    plans
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gun(id: u32, reach: f64) -> PlanGun {
        let band = Band {
            min_range: 0.,
            max_range: reach,
            min_altitude: 4_000.,
            max_altitude: 25_000.,
        };
        PlanGun {
            unit: UnitId(id),
            index: 0,
            eye: [0.; 3],
            reach: band,
            near: Band {
                max_range: reach + FLAK_REACH_MARGIN_FT,
                ..band
            },
            assigned: None,
        }
    }

    #[test]
    fn the_longest_reaching_guns_take_different_members_first() {
        // Two KS-12s (26,200 ft) listed before two KS-19s (44,250 ft): the
        // KS-19s, which open fire first, take the two members; the KS-12s
        // the two again.
        let mut guns = [
            gun(1, 26_200.),
            gun(2, 26_200.),
            gun(3, 44_250.),
            gun(4, 44_250.),
        ];
        assign(&mut guns, &[0, 1], 7);
        let given: Vec<Option<u32>> = guns.iter().map(|g| g.assigned).collect();
        assert_eq!(given, vec![Some(0), Some(1), Some(0), Some(1)]);
        // Four members, three guns: one each, the last member waits.
        let mut guns = [gun(1, 44_250.), gun(2, 44_250.), gun(3, 44_250.)];
        assign(&mut guns, &[0, 1, 2, 3], 0);
        let given: Vec<Option<u32>> = guns.iter().map(|g| g.assigned).collect();
        assert_eq!(given, vec![Some(0), Some(1), Some(2)]);
        // Ten seconds on, every gun has moved to the next member.
        assign(&mut guns, &[0, 1, 2, 3], 1);
        let given: Vec<Option<u32>> = guns.iter().map(|g| g.assigned).collect();
        assert_eq!(given, vec![Some(1), Some(2), Some(3)]);
        // A lone gun walks over a pair; enough guns never rotate (above, 7).
        let mut lone = [gun(1, 44_250.)];
        let walk: Vec<Option<u32>> = (0..3)
            .map(|r| {
                assign(&mut lone, &[0, 1], r);
                lone[0].assigned
            })
            .collect();
        assert_eq!(walk, vec![Some(0), Some(1), Some(0)]);
    }

    #[test]
    fn a_gun_prefers_its_own_member_in_reach_then_any_in_reach() {
        let mut me = gun(1, 26_200.);
        me.assigned = Some(5);
        let at = |range: f64| [0., 8_000., range];
        // Its own member just past its reach beats another inside it.
        assert!(me.tier(5, at(28_000.)) < me.tier(6, at(20_000.)));
        // Far past the margin, a member it can reach comes first.
        assert!(me.tier(6, at(20_000.)) < me.tier(5, at(40_000.)));
        // The heading cone: at the team, across, too slow to judge.
        assert!(heads_at([0., 0., -760.], [0., 8_000., 50_000.], [0.; 3]));
        assert!(!heads_at([760., 0., 0.], [0., 8_000., 50_000.], [0.; 3]));
        assert!(
            !heads_at([0., 0., -20.], [0., 8_000., 50_000.], [0.; 3]),
            "too slow"
        );
        // 44 degrees off the bearing is in the cone, 46 is not.
        for (deg, inside) in [(44., true), (46., false)] {
            let a = f64::to_radians(deg);
            let track = [760. * a.sin(), 0., -760. * a.cos()];
            assert_eq!(
                heads_at(track, [0., 8_000., 50_000.], [0.; 3]),
                inside,
                "{deg}"
            );
        }
    }
}
