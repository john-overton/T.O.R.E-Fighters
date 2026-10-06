//! The sort: who takes which bandit when a flight lead orders its wingmen to
//! sort. Slice G3c of the flight data link; the guide is `docs/DATALINK.md`
//! ("Giving assignments") and the design is `docs/ARCHITECTURE.md`, "Flight
//! data link".
//!
//! Pure geometry over plain rows, so the human lead's order (Alt+A) and an AI
//! lead's sort (slice G4) run the same code. Nothing here reads a world, rolls
//! a random number or keeps state.
//!
//! The rule (agent decisions unless noted):
//!
//! - The lead keeps its own target. Every other bandit within
//!   [`SORT_REACH_FT`] of the lead, in a straight line, is in the pool.
//! - Wingmen that are known to be Winchester, at bingo fuel or worse, or
//!   heavily damaged are skipped and get nothing.
//! - The others take bandits in member order, each the one nearest to itself
//!   that nobody has yet (a tie goes to the lower aircraft id).
//! - When the pool runs out first, the wingmen still without one take, in
//!   member order again, the nearest bandit that fewer than [`MAX_ON_BANDIT`]
//!   wingmen have. A wingman left without one gets no assignment.
//! - The numbers 40 nm and two are John's design (2026-09-28); the order of
//!   the picks is the agent's.

use crate::sensors::FEET_PER_NAUTICAL_MILE;

/// How far from the lead a bandit can be and still be sorted: 40 nautical
/// miles.
pub const SORT_REACH_FT: f64 = 40. * FEET_PER_NAUTICAL_MILE;

/// The most wingmen a sort puts on one bandit: retail's "two" attacker
/// allowance.
pub const MAX_ON_BANDIT: usize = 2;

/// A hostile aircraft the lead knows of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bandit {
    /// The aircraft (its combat target id).
    pub id: u32,
    /// World position in feet, as the picture last had it.
    pub position: [f64; 3],
}

/// A wingman the sort may give a bandit to.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Wingman {
    /// The plane.
    pub id: u32,
    /// Its place in the flight from zero; picks go in this order.
    pub member: u8,
    pub position: [f64; 3],
    /// Known to have no missile and no gun rounds left.
    pub winchester: bool,
    /// Known to be at bingo fuel or worse.
    pub bingo: bool,
    /// Known to be heavily damaged.
    pub heavy_damage: bool,
}

impl Wingman {
    /// Whether the sort leaves this wingman out.
    pub fn skipped(&self) -> bool {
        self.winchester || self.bingo || self.heavy_damage
    }
}

/// What the sort decided.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Sorted {
    /// `(wingman, bandit)` for each wingman given one, in member order.
    pub given: Vec<(u32, u32)>,
    /// Wingmen left out for their state, in member order.
    pub skipped: Vec<u32>,
    /// Fit wingmen that got nothing because every bandit already has
    /// [`MAX_ON_BANDIT`] on it, or none was in reach.
    pub left: Vec<u32>,
}

fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// The bandit among `pool` nearest to `from` that has fewer than `limit`
/// wingmen on it: nearest first, the lower id on a tie. The count of each is
/// beside it.
fn nearest(pool: &[(Bandit, usize)], from: [f64; 3], limit: usize) -> Option<usize> {
    pool.iter()
        .enumerate()
        .filter(|(_, (_, on))| *on < limit)
        .min_by(|(_, (a, _)), (_, (b, _))| {
            distance(a.position, from)
                .total_cmp(&distance(b.position, from))
                .then(a.id.cmp(&b.id))
        })
        .map(|(index, _)| index)
}

/// Hands the bandits `known` to `wingmen`. `lead` is the lead's position and
/// `lead_target` the aircraft it keeps (its designation, or its AI target),
/// which is left out of the pool. The input may list a bandit twice; the
/// first row wins.
pub fn sort(
    lead: [f64; 3],
    lead_target: Option<u32>,
    known: &[Bandit],
    wingmen: &[Wingman],
) -> Sorted {
    // Each bandit in reach, with the number of wingmen on it so far.
    let mut pool: Vec<(Bandit, usize)> = Vec::new();
    for bandit in known {
        if Some(bandit.id) != lead_target
            && distance(bandit.position, lead) <= SORT_REACH_FT
            && !pool.iter().any(|(held, _)| held.id == bandit.id)
        {
            pool.push((*bandit, 0));
        }
    }
    let mut order: Vec<&Wingman> = wingmen.iter().collect();
    order.sort_by_key(|wingman| (wingman.member, wingman.id));
    let mut sorted = Sorted::default();
    let mut waiting: Vec<&Wingman> = Vec::new();
    // First pass: each takes the nearest bandit nobody has yet.
    for wingman in order {
        if wingman.skipped() {
            sorted.skipped.push(wingman.id);
        } else if let Some(index) = nearest(&pool, wingman.position, 1) {
            pool[index].1 += 1;
            sorted.given.push((wingman.id, pool[index].0.id));
        } else {
            waiting.push(wingman);
        }
    }
    // Second pass: more wingmen than bandits, so the spare ones double up, at
    // most MAX_ON_BANDIT to one bandit.
    for wingman in waiting {
        if let Some(index) = nearest(&pool, wingman.position, MAX_ON_BANDIT) {
            pool[index].1 += 1;
            sorted.given.push((wingman.id, pool[index].0.id));
        } else {
            sorted.left.push(wingman.id);
        }
    }
    // The second pass appended after the first: put the picks back in member
    // order.
    let place = |id: u32| {
        wingmen
            .iter()
            .find(|wingman| wingman.id == id)
            .map_or((u8::MAX, id), |wingman| (wingman.member, wingman.id))
    };
    sorted.given.sort_by_key(|(wingman, _)| place(*wingman));
    sorted.left.sort_by_key(|wingman| place(*wingman));
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    const NM: f64 = FEET_PER_NAUTICAL_MILE;

    fn bandit(id: u32, east_nm: f64, north_nm: f64) -> Bandit {
        Bandit {
            id,
            position: [east_nm * NM, 20_000., north_nm * NM],
        }
    }

    fn wingman(id: u32, member: u8, east_nm: f64) -> Wingman {
        Wingman {
            id,
            member,
            position: [east_nm * NM, 20_000., 0.],
            winchester: false,
            bingo: false,
            heavy_damage: false,
        }
    }

    const LEAD: [f64; 3] = [0., 20_000., 0.];

    #[test]
    fn each_wingman_gets_a_different_bandit_while_they_last() {
        let known = [
            bandit(10, 10., 20.),
            bandit(11, -10., 20.),
            bandit(12, 0., 25.),
        ];
        let wingmen = [wingman(1, 1, -2.), wingman(2, 2, 2.), wingman(3, 3, 0.)];
        let sorted = sort(LEAD, None, &known, &wingmen);
        let mut targets: Vec<u32> = sorted.given.iter().map(|(_, t)| *t).collect();
        targets.sort_unstable();
        assert_eq!(targets, [10, 11, 12]);
        assert!(sorted.skipped.is_empty() && sorted.left.is_empty());
    }

    #[test]
    fn a_wingman_takes_the_bandit_nearest_to_itself_in_member_order() {
        // Bandit 10 is nearest the lead's right wingman, 11 the left one's.
        let known = [bandit(10, 12., 10.), bandit(11, -12., 10.)];
        // Member 1 is on the right, so it picks first and takes 10; member 2
        // on the left takes the other.
        let wingmen = [wingman(7, 2, -3.), wingman(6, 1, 3.)];
        let sorted = sort(LEAD, None, &known, &wingmen);
        assert_eq!(sorted.given, [(6, 10), (7, 11)]);
        // An earlier member takes its nearest even when a later one is nearer
        // to it.
        let wingmen = [wingman(6, 1, -3.), wingman(7, 2, -10.)];
        let known = [bandit(10, -12., 10.), bandit(11, 12., 10.)];
        let sorted = sort(LEAD, None, &known, &wingmen);
        assert_eq!(sorted.given, [(6, 10), (7, 11)]);
    }

    #[test]
    fn the_lead_keeps_its_own_target() {
        let known = [bandit(10, 5., 10.), bandit(11, -5., 10.)];
        let wingmen = [wingman(1, 1, 0.), wingman(2, 2, 0.)];
        let sorted = sort(LEAD, Some(10), &known, &wingmen);
        // One bandit is left and two wingmen may share it.
        assert_eq!(sorted.given, [(1, 11), (2, 11)]);
    }

    #[test]
    fn the_spare_wingmen_double_up_but_never_three_to_one() {
        let known = [bandit(10, 0., 10.), bandit(11, 0., 12.)];
        let wingmen = [
            wingman(1, 1, 0.),
            wingman(2, 2, 0.),
            wingman(3, 3, 0.),
            wingman(4, 4, 0.),
            wingman(5, 5, 0.),
        ];
        let sorted = sort(LEAD, None, &known, &wingmen);
        assert_eq!(sorted.given.len(), 4);
        for bandit in [10, 11] {
            let on = sorted.given.iter().filter(|(_, t)| *t == bandit).count();
            assert_eq!(on, MAX_ON_BANDIT, "bandit {bandit}");
        }
        // The first pass gave the first two wingmen one each, the next two
        // doubled up in member order, and the fifth has nowhere to go.
        assert_eq!(sorted.given[0], (1, 10));
        assert_eq!(sorted.given[1], (2, 11));
        assert_eq!(sorted.left, [5]);
    }

    #[test]
    fn wingmen_that_are_out_of_missiles_low_on_fuel_or_badly_hurt_are_skipped() {
        let known = [
            bandit(10, 0., 10.),
            bandit(11, 3., 10.),
            bandit(12, 6., 10.),
        ];
        let mut winchester = wingman(1, 1, 0.);
        winchester.winchester = true;
        let mut bingo = wingman(2, 2, 0.);
        bingo.bingo = true;
        let mut hurt = wingman(3, 3, 0.);
        hurt.heavy_damage = true;
        let fit = wingman(4, 4, 0.);
        let sorted = sort(LEAD, None, &known, &[winchester, bingo, hurt, fit]);
        assert_eq!(sorted.skipped, [1, 2, 3]);
        assert_eq!(sorted.given, [(4, 10)]);
    }

    #[test]
    fn bandits_beyond_forty_miles_are_left_alone_and_a_bandit_listed_twice_counts_once() {
        let known = [
            bandit(10, 0., 39.9),
            bandit(11, 0., 40.1),
            bandit(10, 0., 5.),
        ];
        let wingmen = [wingman(1, 1, 0.), wingman(2, 2, 0.)];
        let sorted = sort(LEAD, None, &known, &wingmen);
        assert_eq!(sorted.given, [(1, 10), (2, 10)]);
        assert!(
            sort(LEAD, None, &[bandit(11, 0., 40.1)], &wingmen)
                .given
                .is_empty()
        );
    }

    #[test]
    fn equal_distances_go_to_the_lower_aircraft_id_and_the_sort_is_repeatable() {
        let known = [bandit(12, 5., 10.), bandit(11, -5., 10.)];
        let wingmen = [wingman(1, 1, 0.)];
        let first = sort(LEAD, None, &known, &wingmen);
        assert_eq!(first.given, [(1, 11)]);
        assert_eq!(first, sort(LEAD, None, &known, &wingmen));
    }

    #[test]
    fn nothing_to_sort_gives_nothing() {
        let wingmen = [wingman(1, 1, 0.)];
        let sorted = sort(LEAD, None, &[], &wingmen);
        assert!(sorted.given.is_empty());
        assert_eq!(sorted.left, [1]);
        assert_eq!(
            sort(LEAD, None, &[bandit(10, 0., 5.)], &[]),
            Sorted::default()
        );
    }
}
