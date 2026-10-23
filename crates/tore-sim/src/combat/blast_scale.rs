//! How big a weapon's impact explosion is drawn against its explosion type's
//! own size (bugs B8 to B10; docs/spec/explosions.md, "Size by weapon").
//! Presentation only: the scale rides on the effect so every machine and
//! every replay draws the same, and nothing in combat reads it.
//!
//! The rule is opinionated (John, 2026-10-12):
//!
//! - guns up to 40 mm, aircraft and surface alike (the AC-130's 25 mm and
//!   40 mm included), draw their hits at half size, on the ground and in the
//!   air;
//! - the AC-130's 105 mm draws like a 500 lb bomb, its type's own size
//!   (John retuned it from twice its size the same day);
//! - the 57 mm, the flak guns and the tank guns are unchanged;
//! - a bomb scales by the cube root of its weight against a 500 lb bomb
//!   (2,000 lb about 1.6 times, 250 lb about 0.8 times);
//! - a missile or rocket scales the same way by its warhead's weight.
//!
//! The retail records carry no warhead weight, so [`warhead_lb`] holds the
//! real weapons' published warhead weights, rounded (fitted). A missile or
//! rocket draws at least [`POWERED_FLOOR`] of its size, so no warhead reads
//! smaller than a cannon shell (agent decision, 2026-10-12). A record the
//! table does not name keeps its type's size.
use super::{ballistics, live};
use tore_formats::weapons::Weapon;

/// Drawn at its type's own size: every effect that is not a weapon's impact
/// (a kill, a crash, a launch) and every one from an older recording.
pub const FULL: u8 = 100;
/// An inert puff, a dud bomb's dirt: drawn at full size but no detonation,
/// so no flash of light and no sparks.
pub const INERT: u8 = 0;
/// Guns up to 40 mm: half size.
pub const SMALL_GUN: u8 = 50;
/// The AC-130's 105 mm: like a 500 lb bomb, kinetic and explosive (John,
/// 2026-10-12, down from twice the size).
pub const HOWITZER: u8 = 100;
/// The bomb (or warhead) weight that draws at its type's own size, pounds.
pub const REFERENCE_LB: f64 = 500.;
/// The least a missile or rocket's impact is drawn at.
pub const POWERED_FLOOR: f64 = 0.5;

/// The gun records drawn at their own size: the 57 mm, the 85 mm and 100 mm
/// flak and the 120 mm and 125 mm tank guns.
const FULL_SIZE_GUNS: [&str; 5] = ["ZSU57.JT", "KS12.JT", "KS19.JT", "M1.JT", "T72.JT"];

/// The size factor a scale stands for: 1 for [`INERT`] and [`FULL`].
pub fn factor(scale: u8) -> f64 {
    if scale == INERT {
        1.
    } else {
        f64::from(scale) / 100.
    }
}

/// Whether an effect of this scale is a detonation, which flashes and
/// throws sparks.
pub fn detonates(scale: u8) -> bool {
    scale != INERT
}

/// A record that leaves with its own speed and no motor: a gun round.
fn is_gun_round(w: &Weapon) -> bool {
    live::is_gun(w)
        || (w.movement.fuel_t == 0 && w.movement.initial_speed > 0 && !ballistics::is_bomb(w))
}

/// The scale a factor rounds to, within what a byte holds (and never the
/// inert marker).
fn percent(factor: f64) -> u8 {
    (factor * 100.).round().clamp(1., 255.) as u8
}

/// The scale `w`'s impacts are drawn at, in percent of the explosion type's
/// size.
pub fn of(w: &Weapon) -> u8 {
    if is_gun_round(w) {
        let source = w.source.to_ascii_uppercase();
        return if source == super::gunship::GUNS[2] {
            HOWITZER
        } else if FULL_SIZE_GUNS.contains(&source.as_str()) {
            FULL
        } else {
            SMALL_GUN
        };
    }
    if w.movement.fuel_t == 0 {
        // A bomb: a pack's weight is its bombs' together.
        let each = f64::from(w.weight.max(1)) / ballistics::release_count(w).max(1) as f64;
        return percent((each / REFERENCE_LB).cbrt());
    }
    match warhead_lb(&w.source) {
        Some(lb) => percent((lb / REFERENCE_LB).cbrt().max(POWERED_FLOOR)),
        None => FULL,
    }
}

/// The crater a 500 lb bomb leaves, in the original's crater units (the
/// Mk 82's, 288 ft across): every bomb's crater is this at its scale.
pub const BOMB_CRATER: u8 = 9;

/// What a strike of `w` on land leaves (John, 2026-10-12): a crater size and
/// the scale it is drawn at, or none. Every crater follows the explosion's
/// scale:
///
/// - a bomb that leaves a crater at all leaves a 500 lb bomb's
///   ([`BOMB_CRATER`]) at its own scale, so a 2,000 lb bomb's is about 1.6
///   times as wide and a 250 lb bomb's 0.8 (the retail sizes, 9 for a Mk 82
///   and an FAB-1000 alike, 18 for a Mk 84, do not follow the weight);
/// - a missile or rocket leaves its record's crater at its scale;
/// - the AC-130's 105 mm, which has no crater in its record, leaves a 500 lb
///   bomb's;
/// - a tank gun keeps its record's crater; every other gun round leaves a
///   pock mark ([`super::blast::POCK`]) at its scale.
///
/// A cluster bomb leaves none, as its record says.
pub fn crater_of(w: &Weapon) -> Option<(u8, u8)> {
    let scale = of(w);
    let retail = w.effects.crater_size;
    if is_gun_round(w) {
        let source = w.source.to_ascii_uppercase();
        return Some(if source == super::gunship::GUNS[2] {
            (BOMB_CRATER, scale)
        } else if retail > 0 {
            (retail, FULL)
        } else {
            (super::blast::POCK, scale)
        });
    }
    if retail == 0 {
        return None;
    }
    Some(if w.movement.fuel_t == 0 {
        (BOMB_CRATER, scale)
    } else {
        (retail, scale)
    })
}

/// The real weapon's warhead, pounds, by retail record (fitted, published
/// figures rounded; fictional records an agent's estimate). A rocket pod's
/// is one rocket's.
pub fn warhead_lb(record: &str) -> Option<f64> {
    let name = record.to_ascii_uppercase();
    let lb = match name.trim_end_matches(".JT") {
        // Air-to-air.
        "AA2" => 13.,
        "AA6" => 154.,
        "AA8" => 7.,
        "AA9" => 103.,
        "AA10" | "R27ER" | "R27T" | "R27ET" => 86.,
        "AA11" | "AA11B" => 16.,
        "AA12" => 49.,
        "AAML" => 150.,
        "AIM9B" => 10.,
        "AIM9M" | "AIM9X" => 21.,
        "AIM7" => 88.,
        "AIM7E" => 66.,
        "AIM120" => 50.,
        "AIM54C" => 135.,
        "MICA" => 26.,
        "PL7" => 26.,
        "PL10" => 88.,
        "R530" => 66.,
        "R550" => 28.,
        "AEMP1" => 0.,
        // Air-to-ground and anti-ship.
        "AGM45" => 145.,
        "AGM65A" => 125.,
        "AGM65G" => 300.,
        "AGM84A" | "AGM84E" => 488.,
        "AGM88" => 146.,
        "AM39" => 364.,
        "AS7" => 220.,
        "AS14" => 705.,
        "AS15" => 66.,
        "AS16" => 330.,
        "AS30" => 529.,
        "AT2" => 12.,
        "AT12" => 9.,
        "SSN9" => 1_100.,
        "ASROC" => 96.,
        // Rockets: one rocket of the pod.
        "LAU61" => 10.,
        "LAU10" => 45.,
        "B8" => 8.,
        "B13" => 72.,
        // Surface-to-air.
        "FIM92" | "MIS" => 7.,
        "HQ2J" => 420.,
        "HQ61" => 110.,
        "MIM23" => 119.,
        "R440" => 33.,
        "ROLAND" => 20.,
        "SA2A" => 430.,
        "SA3" => 132.,
        "SA6" => 125.,
        "SA7" | "SAN5" => 3.,
        "SA9" | "SA13" => 7.,
        "SA14" | "SA16" | "SAN8" => 4.,
        "SA15" | "SAN9" => 33.,
        "SA19" | "SAN11" => 20.,
        "SAN3" => 265.,
        "SAN4" => 42.,
        "SAN7" => 154.,
        "SEA_SPAR" => 86.,
        _ => return None,
    };
    Some(lb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(source: &str, weight: i32, fuel_t: u16, initial_speed: i16, flags: u32) -> Weapon {
        let mut w = crate::combat::gunsight::tests::weapon();
        w.source = source.into();
        w.weight = weight;
        w.movement.fuel_t = fuel_t;
        w.movement.initial_speed = initial_speed;
        w.flags = flags;
        w
    }
    fn bomb(source: &str, weight: i32) -> Weapon {
        record(source, weight, 0, 0, ballistics::BOMB_FLAG)
    }
    fn gun(source: &str) -> Weapon {
        record(source, 1, 0, 2_933, 0)
    }
    fn missile(source: &str) -> Weapon {
        record(source, 500, 40, 0, 0)
    }

    #[test]
    fn guns_up_to_40_mm_draw_half_size_and_the_105_as_a_500_lb_bomb() {
        for small in [
            "M61.JT",
            "GAU8.JT",
            "C_25.JT",
            "C_40.JT",
            "ZSU23.JT",
            "M1939.JT",
            "SMLARMS.JT",
        ] {
            assert_eq!(of(&gun(small)), SMALL_GUN, "{small}");
        }
        assert_eq!(of(&gun("C_105.JT")), HOWITZER);
        assert_eq!(of(&gun("c_105.jt")), HOWITZER);
        assert_eq!(HOWITZER, of(&bomb("MK82.JT", 500)));
        for full in FULL_SIZE_GUNS {
            assert_eq!(of(&gun(full)), FULL, "{full}");
        }
    }

    #[test]
    fn bombs_scale_by_the_cube_root_of_their_weight_against_500_lb() {
        assert_eq!(of(&bomb("MK82.JT", 500)), 100);
        assert_eq!(of(&bomb("MK84.JT", 2_000)), 159);
        assert_eq!(of(&bomb("SMALL.JT", 250)), 79);
        assert_eq!(of(&bomb("GBU28.JT", 4_500)), 208);
        // A pack's weight is its bombs' together: a 7-pack of Mk 82s is a
        // 500 lb bomb seven times.
        let mut pack = bomb("MK82AIR.JT", 3_500);
        pack.burst.game_rounds_in_burst = 7;
        assert_eq!(of(&pack), 100);
    }

    #[test]
    fn missiles_and_rockets_scale_by_their_warhead_with_a_floor() {
        assert_eq!(of(&missile("AGM84A.JT")), 99);
        assert_eq!(of(&missile("AGM65G.JT")), 84);
        assert_eq!(of(&missile("SSN9.JT")), 130);
        // A Sidewinder's 21 lb would draw at 35 percent: the floor holds it
        // at a cannon shell's half.
        assert_eq!(of(&missile("AIM9M.JT")), 50);
        assert_eq!(of(&missile("LAU61.JT")), 50);
        // A record the table does not name keeps its size.
        assert_eq!(of(&missile("UNKNOWN.JT")), FULL);
    }

    #[test]
    fn craters_follow_the_explosions_scale_and_guns_leave_pock_marks() {
        use super::super::blast::{POCK, mark_half_width};
        let with_crater = |mut w: Weapon, crater: u8| {
            w.effects.crater_size = crater;
            w
        };
        let mk82 = crater_of(&with_crater(bomb("MK82.JT", 500), 9)).unwrap();
        let mk84 = crater_of(&with_crater(bomb("MK84.JT", 2_000), 18)).unwrap();
        let small = crater_of(&with_crater(bomb("SMALL.JT", 250), 9)).unwrap();
        assert_eq!(mk82, (BOMB_CRATER, 100));
        assert_eq!(mk84, (BOMB_CRATER, 159));
        let width = |(size, scale): (u8, u8)| mark_half_width(size, scale);
        assert!(width(small) < width(mk82) && width(mk82) < width(mk84));
        assert!((width(mk84) / width(mk82) - 1.59).abs() < 1e-9);
        // The 105 mm leaves a 500 lb bomb's crater.
        assert_eq!(crater_of(&gun("C_105.JT")), Some(mk82));
        // Small guns leave pock marks; a tank gun keeps its record's crater.
        assert_eq!(crater_of(&gun("M61.JT")), Some((POCK, SMALL_GUN)));
        assert_eq!(width((POCK, SMALL_GUN)), 2.5);
        assert_eq!(crater_of(&with_crater(gun("T72.JT"), 6)), Some((6, FULL)));
        // A cluster bomb leaves none; a missile its record's at its scale.
        assert_eq!(crater_of(&bomb("CBU87.JT", 500)), None);
        assert_eq!(
            crater_of(&with_crater(missile("AGM65G.JT"), 9)),
            Some((9, 84))
        );
    }

    #[test]
    fn the_inert_marker_draws_full_size_without_detonating() {
        assert_eq!(factor(INERT), 1.);
        assert!(!detonates(INERT));
        assert_eq!(factor(FULL), 1.);
        assert_eq!(factor(HOWITZER), 1.);
        assert_eq!(factor(200), 2.);
        assert!(detonates(SMALL_GUN));
    }
}
