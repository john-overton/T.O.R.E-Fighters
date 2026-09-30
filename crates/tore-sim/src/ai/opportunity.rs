//! The mission of opportunity of a wing whose human leader is lost.
//!
//! **Opinionated, requested by John on 2026-09-30:** when an AI aircraft takes
//! the lead of a wing from a lost human, the wing "continues on with the
//! mission of opportunity and visual based on last known enemy locations, if
//! they can't find anything then RTB's". Every number here is **fitted, an
//! agent decision**; the rule is in
//! [`docs/spec/ai.md`](../../../../docs/spec/ai.md), "Mission of opportunity
//! after a lost human leader".
//!
//! The wing engages any hostile aircraft it detects (the ordinary leader
//! release). With none in contact its leader searches by eye: it flies to the
//! nearest hostile position the wing knows (its members' sightings and sensor
//! tracks, kept in their awareness memory) and circles over it for
//! [`DWELL_TICKS`]; then the next. The search ends, and the wing returns to
//! base, when nothing is left to search or when [`SEARCH_LIMIT_TICKS`] pass
//! without a hostile in contact. This module holds the wing's state and the
//! decision; [`super::mission`] feeds it and flies the result.

use super::TICKS_PER_SECOND;
use super::awareness::FEET_PER_NAUTICAL_MILE;
use super::targeting::Side;

/// Fitted: the leader has reached a search point within 2 nm of it
/// (horizontal), about where the circle over it begins (the lost-contact
/// search circles 0.75 nm out once within 1 nm).
pub const ARRIVAL_FT: f64 = 2.0 * FEET_PER_NAUTICAL_MILE;
/// Fitted: time spent circling over one point before it counts as searched,
/// about one and a half turns of the search circle.
pub const DWELL_TICKS: u64 = 60 * TICKS_PER_SECOND;
/// Fitted: the search gives up and the wing returns to base after this long
/// with no hostile aircraft in contact, counted from the moment the AI took
/// the lead or from the wing's last contact.
pub const SEARCH_LIMIT_TICKS: u64 = 10 * 60 * TICKS_PER_SECOND;

/// A hostile aircraft's last known position, as the wing knows it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LastKnown {
    pub id: u32,
    pub position: [f64; 3],
    /// The simulation tick it was last seen there.
    pub observed_tick: u64,
    /// The leader has circled over it without finding anything. A newer
    /// sighting of the same aircraft makes it worth searching again.
    pub searched: bool,
}

/// One hostile aircraft seen by a member of the wing, from that member's
/// awareness memory.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sighting {
    pub id: u32,
    pub position: [f64; 3],
    pub observed_tick: u64,
    /// The member sees it now, rather than remembering it.
    pub current: bool,
}

/// Why the wing gave up its mission of opportunity and returned to base.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HomeReason {
    /// The wing flies under weapons hold or self-defense, so it has no
    /// mission of opportunity to fly.
    NotCleared,
    /// The wing knew of no hostile aircraft to look for.
    NothingKnown,
    /// Every known position was searched and nothing was found.
    AllSearched,
    /// [`SEARCH_LIMIT_TICKS`] passed without a hostile in contact.
    SearchTimeUp,
}

impl HomeReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotCleared => "not cleared to attack",
            Self::NothingKnown => "no enemy position known",
            Self::AllSearched => "every last known position searched",
            Self::SearchTimeUp => "search time up",
        }
    }
}

/// What the wing is doing on this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Plan {
    /// A hostile is in contact; the ordinary engagement rules fly the wing.
    Engage,
    /// Search this position.
    Search(LastKnown),
    /// Return to base. Stays so for the rest of the mission.
    Home(HomeReason),
}

/// The mission of opportunity of one wing.
#[derive(Clone, Debug, PartialEq)]
pub struct Opportunity {
    pub side: Side,
    pub wing: u8,
    /// The tick the AI took the lead.
    pub started_tick: u64,
    /// The last tick a hostile aircraft was in contact (the start before any).
    pub last_contact_tick: u64,
    /// Every hostile position the wing knows, one per aircraft, in id order.
    pub points: Vec<LastKnown>,
    /// The hostile whose position is being searched.
    pub searching: Option<u32>,
    /// When the leader reached the point being searched.
    pub arrived_tick: Option<u64>,
    /// Set once the wing returns to base: the tick and the reason.
    pub home: Option<(u64, HomeReason)>,
}

impl Opportunity {
    pub fn new(side: Side, wing: u8, tick: u64) -> Self {
        Self {
            side,
            wing,
            started_tick: tick,
            last_contact_tick: tick,
            points: Vec::new(),
            searching: None,
            arrived_tick: None,
            home: None,
        }
    }

    fn nearest_unsearched(&self, from: [f64; 3]) -> Option<u32> {
        let horizontal =
            |point: &LastKnown| (point.position[0] - from[0]).hypot(point.position[2] - from[2]);
        self.points
            .iter()
            .filter(|p| !p.searched)
            .min_by(|a, b| {
                horizontal(a)
                    .total_cmp(&horizontal(b))
                    .then(a.id.cmp(&b.id))
            })
            .map(|p| p.id)
    }

    /// Give up the search and return to base.
    pub fn go_home(&mut self, tick: u64, reason: HomeReason) -> Plan {
        self.home.get_or_insert((tick, reason));
        self.searching = None;
        self.arrived_tick = None;
        Plan::Home(self.home.expect("just set").1)
    }

    /// One tick of the mission of opportunity.
    ///
    /// `sightings` are the hostile aircraft the wing's members see now or
    /// remember, and `alive` says whether one of them is still flying (a
    /// destroyed aircraft is dropped from the wing's memory, as from each
    /// member's). `leader_engaged` is true while the leader has a target.
    ///
    /// A hostile in sight, or a target, holds the search clock. The search
    /// goes to the nearest hostile a member sees now, otherwise it keeps to
    /// the point it chose until that point is searched, then takes the
    /// nearest point not yet searched. The leader flies the search only while
    /// it has no target of its own.
    pub fn step(
        &mut self,
        tick: u64,
        leader_position: [f64; 3],
        sightings: &[Sighting],
        leader_engaged: bool,
        alive: &dyn Fn(u32) -> bool,
    ) -> Plan {
        if let Some((_, reason)) = self.home {
            return Plan::Home(reason);
        }
        for seen in sightings {
            match self.points.iter_mut().find(|p| p.id == seen.id) {
                Some(point) if seen.observed_tick > point.observed_tick => {
                    point.position = seen.position;
                    point.observed_tick = seen.observed_tick;
                    point.searched = false;
                }
                Some(_) => {}
                None => self.points.push(LastKnown {
                    id: seen.id,
                    position: seen.position,
                    observed_tick: seen.observed_tick,
                    searched: false,
                }),
            }
        }
        self.points.retain(|p| alive(p.id));
        self.points.sort_by_key(|p| p.id);
        let horizontal = |point: &LastKnown| {
            (point.position[0] - leader_position[0]).hypot(point.position[2] - leader_position[2])
        };
        let in_sight = self
            .points
            .iter()
            .filter(|p| sightings.iter().any(|s| s.current && s.id == p.id))
            .min_by(|a, b| {
                horizontal(a)
                    .total_cmp(&horizontal(b))
                    .then(a.id.cmp(&b.id))
            })
            .map(|p| p.id);
        let in_contact = leader_engaged || in_sight.is_some();
        if in_contact {
            self.last_contact_tick = tick;
        } else if tick.saturating_sub(self.last_contact_tick) >= SEARCH_LIMIT_TICKS {
            return self.go_home(tick, HomeReason::SearchTimeUp);
        }
        if in_sight.is_some() && in_sight != self.searching {
            self.searching = in_sight;
            self.arrived_tick = None;
        }
        if self.searching.is_none() {
            self.searching = self.nearest_unsearched(leader_position);
        }
        // The point being searched: done after circling over it long enough.
        if let Some(id) = self.searching {
            match self.points.iter_mut().find(|p| p.id == id && !p.searched) {
                Some(point) => {
                    if horizontal(point) <= ARRIVAL_FT {
                        let arrived = *self.arrived_tick.get_or_insert(tick);
                        if tick.saturating_sub(arrived) >= DWELL_TICKS {
                            point.searched = true;
                            self.searching = None;
                            self.arrived_tick = None;
                        }
                    }
                }
                None => {
                    self.searching = None;
                    self.arrived_tick = None;
                }
            }
        }
        if self.searching.is_none() {
            self.searching = self.nearest_unsearched(leader_position);
        }
        match self
            .searching
            .and_then(|id| self.points.iter().find(|p| p.id == id))
        {
            Some(point) => Plan::Search(*point),
            None if in_contact => Plan::Engage,
            None if self.points.is_empty() => self.go_home(tick, HomeReason::NothingKnown),
            None => self.go_home(tick, HomeReason::AllSearched),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ENEMY: u32 = 50;

    fn seen(id: u32, x: f64, z: f64, tick: u64) -> Sighting {
        Sighting {
            id,
            position: [x, 10_000., z],
            observed_tick: tick,
            current: false,
        }
    }

    fn in_sight(id: u32, x: f64, z: f64, tick: u64) -> Sighting {
        Sighting {
            current: true,
            ..seen(id, x, z, tick)
        }
    }

    fn alive(_: u32) -> bool {
        true
    }

    #[test]
    fn a_wing_that_knows_no_enemy_returns_to_base_at_once() {
        let mut wing = Opportunity::new(Side(1), 0, 100);
        let plan = wing.step(100, [0.; 3], &[], false, &alive);
        assert_eq!(plan, Plan::Home(HomeReason::NothingKnown));
        assert_eq!(wing.home, Some((100, HomeReason::NothingKnown)));
        // It stays at home even if a sighting arrives later.
        let plan = wing.step(101, [0.; 3], &[seen(ENEMY, 0., 50_000., 101)], true, &alive);
        assert_eq!(plan, Plan::Home(HomeReason::NothingKnown));
    }

    #[test]
    fn the_nearest_last_known_position_is_searched_first_then_the_next() {
        let mut wing = Opportunity::new(Side(1), 0, 0);
        let far = seen(ENEMY, 0., 90_000., 0);
        let near = seen(ENEMY + 1, 30_000., 0., 0);
        let plan = wing.step(1, [0.; 3], &[far, near], false, &alive);
        let Plan::Search(point) = plan else {
            panic!("{plan:?}")
        };
        assert_eq!(point.id, ENEMY + 1);
        // Still on the way: the point stays chosen however long it takes.
        for tick in 2..600 {
            assert!(matches!(
                wing.step(tick, [15_000., 0., 0.], &[far, near], false, &alive),
                Plan::Search(p) if p.id == ENEMY + 1
            ));
        }
        assert_eq!(wing.arrived_tick, None);
        // Over it: circling for the dwell, then on to the other one.
        let over = [30_000. - ARRIVAL_FT + 1., 0., 0.];
        wing.step(600, over, &[far, near], false, &alive);
        assert_eq!(wing.arrived_tick, Some(600));
        let before = wing.step(600 + DWELL_TICKS - 1, over, &[far, near], false, &alive);
        assert!(matches!(before, Plan::Search(p) if p.id == ENEMY + 1));
        let after = wing.step(600 + DWELL_TICKS, over, &[far, near], false, &alive);
        assert!(
            matches!(after, Plan::Search(p) if p.id == ENEMY),
            "{after:?}"
        );
        assert!(wing.points.iter().any(|p| p.id == ENEMY + 1 && p.searched));
        assert_eq!(wing.arrived_tick, None);
    }

    #[test]
    fn nothing_found_at_every_point_sends_the_wing_home() {
        let mut wing = Opportunity::new(Side(1), 0, 0);
        let point = seen(ENEMY, 0., 20_000., 0);
        let over = [0., 0., 20_000.];
        wing.step(1, over, &[point], false, &alive);
        let plan = wing.step(1 + DWELL_TICKS, over, &[point], false, &alive);
        assert_eq!(plan, Plan::Home(HomeReason::AllSearched));
    }

    #[test]
    fn a_newer_sighting_moves_the_point_and_makes_it_worth_searching_again() {
        let mut wing = Opportunity::new(Side(1), 0, 0);
        let over = [0., 0., 20_000.];
        let old = seen(ENEMY, 0., 20_000., 0);
        wing.step(1, over, &[old], false, &alive);
        // The same memory seen again does not undo the search.
        wing.step(
            1 + DWELL_TICKS - 1,
            over,
            &[old, seen(ENEMY + 1, 0., 80_000., 0)],
            false,
            &alive,
        );
        wing.step(1 + DWELL_TICKS, over, &[old], false, &alive);
        assert!(wing.points.iter().any(|p| p.id == ENEMY && p.searched));
        let fresh = seen(ENEMY, 40_000., 20_000., 2 + DWELL_TICKS);
        wing.step(2 + DWELL_TICKS, over, &[fresh], false, &alive);
        let point = wing.points.iter().find(|p| p.id == ENEMY).unwrap();
        assert!(!point.searched);
        assert_eq!(point.position[0], 40_000.);
    }

    #[test]
    fn contact_holds_the_search_clock_and_its_absence_ends_the_search() {
        let mut wing = Opportunity::new(Side(1), 0, 0);
        let far = seen(ENEMY, 0., 5_000_000., 0);
        // The leader's own target counts as contact, even with nothing to search.
        assert_eq!(wing.step(10, [0.; 3], &[], true, &alive), Plan::Engage);
        assert!(matches!(
            wing.step(1000, [0.; 3], &[far], true, &alive),
            Plan::Search(_)
        ));
        assert_eq!(wing.last_contact_tick, 1000);
        let limit = 1000 + SEARCH_LIMIT_TICKS;
        assert!(matches!(
            wing.step(limit - 1, [0.; 3], &[far], false, &alive),
            Plan::Search(_)
        ));
        assert_eq!(
            wing.step(limit, [0.; 3], &[far], false, &alive),
            Plan::Home(HomeReason::SearchTimeUp)
        );
    }

    #[test]
    fn a_hostile_in_sight_now_is_searched_before_older_points() {
        let mut wing = Opportunity::new(Side(1), 0, 0);
        let near = seen(ENEMY, 0., 10_000., 0);
        assert!(matches!(
            wing.step(1, [0.; 3], &[near], false, &alive),
            Plan::Search(p) if p.id == ENEMY
        ));
        // A wingman sees another hostile farther off: the search goes there,
        // and the sighting holds the search clock.
        let spotted = in_sight(ENEMY + 1, 0., -25_000., 5_000);
        let plan = wing.step(5_000, [0.; 3], &[near, spotted], false, &alive);
        assert!(
            matches!(plan, Plan::Search(p) if p.id == ENEMY + 1),
            "{plan:?}"
        );
        assert_eq!(wing.last_contact_tick, 5_000);
    }

    #[test]
    fn a_destroyed_aircraft_is_not_searched_for() {
        let mut wing = Opportunity::new(Side(1), 0, 0);
        let gone = |id: u32| id != ENEMY;
        let plan = wing.step(1, [0.; 3], &[seen(ENEMY, 0., 20_000., 0)], false, &gone);
        assert_eq!(plan, Plan::Home(HomeReason::NothingKnown));
    }
}
