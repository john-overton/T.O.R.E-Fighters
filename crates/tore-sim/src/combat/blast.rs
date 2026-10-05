//! Original explosions, craters and burning crash sites. Numbers and
//! provenance: docs/spec/explosions.md. Presentation only: nothing here
//! changes damage, sensors, flight or AI, and the variety rolls use their
//! own stream so combat randomness is untouched.
use crate::attitude::Vector;

/// One original explosion type: its size before the size roll, how long its
/// animation plays, whether it sits on the surface, the recordings it picks
/// from and the distances its sound is full within and silent beyond.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Explosion {
    pub size: u16,
    pub seconds: u8,
    pub surface: bool,
    pub sounds: &'static [&'static str],
    pub full_ft: u16,
    pub silent_ft: u16,
}

/// The first and last explosion types the original defines.
pub const FIRST: u8 = 15;
pub const LAST: u8 = 38;

const BULLET_LAND: &[&str] = &["&BULLTS1.5K", "&BULLTS4.5K"];
const BULLET_AIR: &[&str] = &["&BULLTS2.8K", "&BULLTS3.5K"];
const MEDIUM: &[&str] = &[
    "&MEDEXP1.5K",
    "&MEDEXP2.5K",
    "&EXPL3.5K",
    "&EXPL7.5K",
    "&EXPL9.5K",
    "&EXPL10.5K",
];
const AIR: &[&str] = &["&AIREXP1.11K", "&AIREXP2.11K"];
const AIR_FLAK: &[&str] = &["&AIREXP1.11K", "&AIREXP2.11K", "&EXPL3.5K", "&EXPL9.5K"];
const AIR_FLAK_HEAVY: &[&str] = &[
    "&AIREXP1.11K",
    "&AIREXP2.11K",
    "&AIREXP4.11K",
    "&AIREXP5.11K",
];
const AIR_LARGE: &[&str] = &["&AIREXP1.11K", "&AIREXP2.11K", "&AIREXP3.11K"];
const WATER: &[&str] = &["&WTREXP1.5K", "&WTREXP2.5K"];
const BIG: &[&str] = &["&BIGEXP1.5K", "&BIGEXP2.5K"];

const fn row(
    size: u16,
    seconds: u8,
    surface: bool,
    sounds: &'static [&'static str],
    full_ft: u16,
    silent_ft: u16,
) -> Explosion {
    Explosion {
        size,
        seconds,
        surface,
        sounds,
        full_ft,
        silent_ft,
    }
}

/// FA explosion table 0x4f46c8, types 15 to 38.
const TABLE: [Explosion; 24] = [
    row(75, 1, true, BULLET_LAND, 100, 10000),
    row(45, 1, true, BULLET_LAND, 100, 10000),
    row(50, 1, true, &["&SPLASH3.11K"], 100, 10000),
    row(50, 1, false, BULLET_AIR, 100, 10000),
    row(60, 1, false, BULLET_AIR, 100, 10000),
    row(40, 1, false, BULLET_AIR, 100, 10000),
    row(400, 2, true, MEDIUM, 1000, 20000),
    row(400, 2, true, MEDIUM, 1000, 20000),
    row(400, 2, true, MEDIUM, 1000, 20000),
    row(200, 1, false, AIR, 1000, 25000),
    row(200, 1, false, AIR, 1000, 25000),
    row(200, 1, false, AIR, 1000, 25000),
    row(130, 2, false, AIR_FLAK, 1000, 25000),
    row(170, 1, false, AIR_FLAK_HEAVY, 1000, 25000),
    row(180, 1, false, AIR_FLAK_HEAVY, 1000, 25000),
    row(300, 1, false, AIR_LARGE, 2000, 25000),
    row(300, 1, false, AIR_LARGE, 2000, 25000),
    row(300, 1, false, AIR_LARGE, 2000, 25000),
    row(300, 1, false, AIR_LARGE, 2000, 25000),
    row(500, 2, true, WATER, 1000, 25000),
    row(
        400,
        2,
        true,
        &["&BIGEXP1.5K", "&BIGEXP2.5K", "&EXPL12.5K"],
        3000,
        25000,
    ),
    row(400, 2, true, BIG, 3000, 25000),
    row(380, 2, true, BIG, 3000, 25000),
    row(250, 1, false, &["&EMPEXP.11K", "&EMPEXP.11K"], 1000, 25000),
];

pub fn explosion(kind: u8) -> Option<&'static Explosion> {
    TABLE.get(usize::from(kind.checked_sub(FIRST)?))
}

/// Every recording any explosion type can play.
pub fn recordings() -> impl Iterator<Item = &'static str> {
    TABLE.iter().flat_map(|e| e.sounds.iter().copied())
}

/// Every FA aircraft explodes as type 30 when destroyed.
pub const AIRCRAFT: u8 = 30;
/// A destroyed ground object or ship: its own type is not read yet, so a
/// large ground blast stands in (fitted).
pub const GROUND_OBJECT: u8 = 35;
/// An aircraft hitting land or water (opinionated, John 2026-09-28).
pub const CRASH_LAND: u8 = 35;
pub const CRASH_WATER: u8 = 34;
/// The crater an aircraft crash leaves on land (opinionated).
pub const CRASH_CRATER: u8 = 6;
/// A crash site's crater, fire and smoke last 15 minutes (John, 2026-09-28).
pub const CRASH_TICKS: u32 = 15 * 60 * 120;
/// The fire fades out over its final minute (opinionated).
pub const FIRE_FADE_TICKS: u32 = 60 * 120;
/// The original's fire size and the distance its `&FIRE.5K` loop reaches.
pub const FIRE_SIZE: u8 = 100;
pub const FIRE_SOUND: &str = "&FIRE.5K";
pub const FIRE_SOUND_FT: f64 = 2000.;
/// A weapon crater stays for the whole mission, as in the original; the
/// oldest is dropped past this many.
pub const MAX_CRATERS: usize = 256;
pub const MAX_FIRES: usize = 64;
/// Marker for a mark that never runs out.
pub const FOREVER: u32 = u32::MAX;

/// A weapon or crash crater's half width in feet: 16 feet per unit of the
/// original crater size, at most 333.
pub fn crater_half_width(size: u8) -> f64 {
    f64::from((u16::from(size) * 16).min(333))
}

/// The original's variety: some explosion types are sometimes drawn and
/// heard as a related type. `percent(n)` is an n in 100 chance and `roll(n)`
/// a draw from 0 to n - 1, both from the presentation stream.
pub fn vary(kind: u8, rolls: &mut Rolls) -> u8 {
    match kind {
        18 if rolls.percent(66) => match rolls.roll(100) {
            0..20 => 19,
            20..40 => 20,
            40..60 => 29,
            60..80 => 28,
            _ => 18,
        },
        18 if rolls.percent(10) => small_air(rolls),
        30 if rolls.percent(25) => small_air(rolls),
        30 if rolls.percent(75) => match rolls.roll(100) {
            0..25 => 31,
            25..50 => 32,
            50..75 => 33,
            _ => 30,
        },
        30 if rolls.percent(10) => {
            if rolls.percent(50) {
                28
            } else {
                29
            }
        }
        15 if rolls.percent(50) => 16,
        21 if rolls.percent(35) => 23,
        35 => match rolls.roll(100) {
            0..=33 => 37,
            34..=66 => 36,
            _ => 35,
        },
        _ => kind,
    }
}
fn small_air(rolls: &mut Rolls) -> u8 {
    match rolls.roll(100) {
        0..50 => 24,
        50..75 => 25,
        _ => 26,
    }
}

/// The presentation stream for [`vary`]: never the combat random stream.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Rolls(u32);
impl Default for Rolls {
    fn default() -> Self {
        Self(0x2545_f491)
    }
}
impl Rolls {
    pub fn roll(&mut self, bound: u16) -> u16 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 % u32::from(bound.max(1))) as u16
    }
    pub fn percent(&mut self, chance: u16) -> bool {
        self.roll(100) < chance
    }
}

/// A repeatable draw from 0 to `bound - 1` for one effect, from its exact
/// position. A replay recovers the same size, recording and crater style
/// from the position it stores, without recording the choice.
pub fn pick(position: Vector, salt: u64, bound: u16) -> u16 {
    let mut value = salt.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    for axis in position {
        value = (value ^ axis.to_bits()).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value ^= value >> 29;
    }
    (value % u64::from(bound.max(1))) as u16
}

/// The rolled size of one explosion: the table size times 66 to 131
/// percent, at most 255. Drawn as the explosion's width in feet (fitted).
pub fn rolled_size(kind: u8, position: Vector) -> u8 {
    explosion(kind).map_or(0, |e| {
        (u32::from(e.size) * (66 + u32::from(pick(position, 1, 66))) / 100).min(255) as u8
    })
}

/// The recording one explosion plays.
pub fn recording(kind: u8, position: Vector) -> Option<&'static str> {
    let e = explosion(kind)?;
    Some(e.sounds[usize::from(pick(position, 2, e.sounds.len() as u16))])
}

/// Which of the three `CRATERS.PIC` styles a crater shows.
pub fn crater_style(position: Vector) -> u8 {
    pick(position, 3, 3) as u8
}

/// A crater or fire left on the ground.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind {
    /// The original crater size.
    Crater(u8),
    Fire,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub position: Vector,
    pub kind: MarkKind,
    /// Ticks left, or [`FOREVER`].
    pub ticks: u32,
    /// Combat tick it started on.
    pub born: u64,
    /// Serial number, rising by one for each mark, so a recording can tell
    /// new marks from old.
    pub serial: u64,
}
impl Mark {
    /// A fire's strength: full, then fading to nothing over its last minute.
    pub fn strength(&self) -> f32 {
        if self.ticks == FOREVER {
            1.
        } else {
            (self.ticks as f32 / FIRE_FADE_TICKS as f32).min(1.)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_covers_types_15_to_38_with_the_original_sounds_and_distances() {
        assert!(explosion(14).is_none() && explosion(39).is_none());
        let air = explosion(30).unwrap();
        assert_eq!((air.size, air.seconds, air.surface), (300, 1, false));
        assert_eq!(air.sounds, AIR_LARGE);
        assert_eq!((air.full_ft, air.silent_ft), (2000, 25000));
        let bomb = explosion(35).unwrap();
        assert_eq!(bomb.sounds.len(), 3);
        assert_eq!((bomb.seconds, bomb.surface, bomb.full_ft), (2, true, 3000));
        assert_eq!(explosion(17).unwrap().sounds, ["&SPLASH3.11K"]);
        assert_eq!(explosion(34).unwrap().sounds, WATER);
        assert_eq!(recordings().count(), 70);
    }

    #[test]
    fn every_recording_is_an_imported_combat_resource() {
        for name in recordings().chain([FIRE_SOUND]) {
            assert!(
                tore_formats::aircraft::COMBAT_RESOURCES.contains(&name),
                "{name}"
            );
        }
    }

    #[test]
    fn variety_keeps_to_the_original_families() {
        let mut rolls = Rolls::default();
        let mut seen = std::collections::BTreeMap::<(u8, u8), u32>::new();
        for kind in [15, 17, 18, 21, 30, 34, 35] {
            for _ in 0..20000 {
                *seen.entry((kind, vary(kind, &mut rolls))).or_default() += 1;
            }
        }
        let share = |from, to| f64::from(seen.get(&(from, to)).copied().unwrap_or(0)) / 20000.;
        let family =
            |from: u8| -> Vec<u8> { seen.keys().filter(|k| k.0 == from).map(|k| k.1).collect() };
        assert_eq!(family(17), [17]);
        assert_eq!(family(34), [34]);
        assert_eq!(family(15), [15, 16]);
        assert_eq!(family(21), [21, 23]);
        assert_eq!(family(18), [18, 19, 20, 24, 25, 26, 28, 29]);
        assert_eq!(family(30), [24, 25, 26, 28, 29, 30, 31, 32, 33]);
        assert_eq!(family(35), [35, 36, 37]);
        assert!((share(15, 16) - 0.5).abs() < 0.02);
        assert!((share(21, 23) - 0.35).abs() < 0.02);
        // 30 stays 30 when the 25% and 75% draws both fail and the 10%
        // fails too, or when the 75% branch rolls 75 or more.
        let stay = 0.75 * (0.75 * 0.25 + 0.25 * 0.9);
        assert!((share(30, 30) - stay).abs() < 0.02);
    }

    #[test]
    fn sizes_sounds_and_crater_styles_repeat_for_the_same_position() {
        let at = [1234.5, 20.25, -987.0];
        assert_eq!(rolled_size(30, at), rolled_size(30, at));
        assert_eq!(recording(30, at), recording(30, at));
        assert_eq!(crater_style(at), crater_style(at));
        let mut sizes = std::collections::BTreeSet::new();
        let mut styles = std::collections::BTreeSet::new();
        for i in 0..2000 {
            let p = [f64::from(i) * 7.5, 0., f64::from(i) * -3.25];
            sizes.insert(rolled_size(24, p));
            styles.insert(crater_style(p));
            assert!(
                explosion(30)
                    .unwrap()
                    .sounds
                    .contains(&recording(30, p).unwrap())
            );
        }
        // 200 times 66% to 131%.
        assert_eq!(sizes.first(), Some(&132));
        assert_eq!(sizes.last(), Some(&255));
        assert_eq!(styles.len(), 3);
        assert_eq!(rolled_size(18, [0.; 3]).max(33), rolled_size(18, [0.; 3]));
    }

    #[test]
    fn craters_scale_16_feet_per_unit_up_to_333_and_fires_fade_in_their_last_minute() {
        assert_eq!(crater_half_width(3), 48.);
        assert_eq!(crater_half_width(18), 288.);
        assert_eq!(crater_half_width(30), 333.);
        let mut fire = Mark {
            position: [0.; 3],
            kind: MarkKind::Fire,
            ticks: CRASH_TICKS,
            born: 0,
            serial: 0,
        };
        assert_eq!(fire.strength(), 1.);
        fire.ticks = FIRE_FADE_TICKS / 2;
        assert_eq!(fire.strength(), 0.5);
        fire.ticks = FOREVER;
        assert_eq!(fire.strength(), 1.);
    }
}

// Exact checkpoints (docs/formats/checkpoint.md).
#[path = "blast_checkpoint.rs"]
mod checkpoint;
