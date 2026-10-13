//! What a large ground blast throws into the air, by the theater's climate
//! (slice B12, the shockwave overhaul; docs/spec/explosions.md, "Climate").
//! Opinionated, John, 2026-10-12: in a humid theater a bomb or a big warhead
//! throws a white condensation cloud around the blast; in an arid one it
//! kicks up wide tan dust that hangs for 30 to 60 seconds by the blast's
//! size; a missile gets a smaller version and a gun hit none; everything
//! scales with the weapon's yield (the effect's scale, slice BF). The shapes,
//! counts and timings are agent decisions tuned on the
//! `--impact-preview OUT_DIR shockwave` renders.
//!
//! Presentation only: every puff is derived from the effect (where, which
//! explosion, its scale, how long ago) and the mission's humidity
//! ([`tore_world::climate`]), with each puff's place and size drawn from the
//! blast's exact position, so a networked client and a replay show the same
//! and nothing reaches the simulation, the wire or a recording.
use crate::gun_flash::Puff;
use tore_sim::{
    attitude::Vector,
    combat::{blast, blast_scale, live::EffectKind},
};

/// The mission's climate as an explosion's look reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Climate {
    /// 0 (desert) to 1 (jungle), [`tore_world::climate`].
    pub humidity: f64,
}

impl Default for Climate {
    fn default() -> Self {
        Self {
            humidity: tore_world::climate::TEMPERATE,
        }
    }
}

/// Humidity below which a blast throws no vapor, and at which its vapor is
/// full.
pub const VAPOR_FROM: f64 = 0.45;
pub const VAPOR_FULL: f64 = 0.85;
/// The dust colour, multiplying the white puff: tan in the driest air,
/// grey-brown in the wettest (the ground is damp and darker).
pub const DUST_DRY: [f32; 3] = [0.88, 0.72, 0.50];
pub const DUST_WET: [f32; 3] = [0.56, 0.52, 0.46];
/// The shockwave ring's colour, multiplying its grey puff: tan when dry,
/// the puff's own pale grey when humid.
pub const RING_DRY: [f32; 3] = [1.0, 0.82, 0.58];

impl Climate {
    /// How dry, 0 to 1.
    pub fn dryness(self) -> f64 {
        1. - self.humidity.clamp(0., 1.)
    }
    /// How much of a full condensation cloud a blast throws, 0 to 1.
    pub fn vapor(self) -> f64 {
        let t = ((self.humidity - VAPOR_FROM) / (VAPOR_FULL - VAPOR_FROM)).clamp(0., 1.);
        t * t * (3. - 2. * t)
    }
    pub fn dust_tint(self) -> [f32; 3] {
        let h = self.humidity.clamp(0., 1.) as f32;
        std::array::from_fn(|i| DUST_DRY[i] + (DUST_WET[i] - DUST_DRY[i]) * h)
    }
    pub fn ring_tint(self) -> [f32; 3] {
        let h = self.humidity.clamp(0., 1.) as f32;
        std::array::from_fn(|i| RING_DRY[i] + (1. - RING_DRY[i]) * h)
    }
}

/// How much of the climate effect an impact throws.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    /// A bomb or a big warhead: the full cloud and dust.
    Full,
    /// A smaller missile or rocket: a smaller version.
    Small,
}

/// The least scale that throws the full effect: a 250 lb bomb's (79 %),
/// so every bomb and every warhead of about 250 lb or more (an AGM-65G, a
/// Harpoon) does; smaller warheads (Sidewinders, rockets, a Maverick A at
/// 63 %) throw the smaller version (agent decision). The effect carries no
/// weapon kind, so the class is read from its yield.
pub const FULL_FROM: u8 = 79;

/// The class of an effect, or none: only a weapon's detonation (a hit or a
/// ground strike, not a kill's own blast, a crash or a dud) as one of the
/// large land explosions (types 21 to 23, 35 to 37). Gun hits (15, 16, 18
/// to 20) and splashes (17) raise nothing; a blast in the water (34) keeps
/// its white spray (John, 2026-10-12).
pub fn class(kind: EffectKind, explosion: u8, scale: u8) -> Option<Class> {
    let weapon = matches!(kind, EffectKind::Hit | EffectKind::Ground);
    let land = matches!(explosion, 21..=23 | 35..=37);
    (weapon && land && blast_scale::detonates(scale)).then_some(if scale >= FULL_FROM {
        Class::Full
    } else {
        Class::Small
    })
}

/// The condensation cloud: `VAPOR_PUFFS` (the small version
/// `VAPOR_PUFFS_SMALL`) white puffs in two rings around the blast that swell
/// to `VAPOR_REACH` of the explosion's drawn width in `VAPOR_GROW_TICKS`
/// (times the square root of the size factor), each puff growing from
/// `VAPOR_PUFF` to `VAPOR_PUFF_END` of the width across, at
/// `VAPOR_OPACITY` times the climate's vapor, gone after `VAPOR_TICKS`
/// times the square root of the factor (2.5 s for a 500 lb bomb, 3.2 s for
/// a 2,000 lb).
pub const VAPOR_PUFFS: usize = 16;
pub const VAPOR_PUFFS_SMALL: usize = 8;
pub const VAPOR_REACH: f64 = 0.75;
/// The two rings' height round the blast, degrees above level from its
/// middle.
pub const VAPOR_LOW_DEG: f64 = 12.;
pub const VAPOR_HIGH_DEG: f64 = 40.;
pub const VAPOR_GROW_TICKS: f64 = 42.;
pub const VAPOR_PUFF: f64 = 0.2;
pub const VAPOR_PUFF_END: f64 = 0.38;
pub const VAPOR_OPACITY: f32 = 0.35;
pub const VAPOR_TICKS: f64 = 300.;

/// The dust: `DUST_PUFFS + DUST_PUFFS_PER_SCALE` times the size factor
/// puffs, at most `DUST_PUFFS_MAX` (18 for a 500 lb bomb, 23 for a
/// 2,000 lb); the small version `DUST_SMALL_PUFFS + DUST_SMALL_PER_SCALE`
/// times the factor, at most `DUST_SMALL_MAX`. They lie in a wide low ring
/// out to `DUST_REACH` of the drawn width, roll out a further
/// `DUST_ROLL` over `DUST_ROLL_TICKS`, rise `DUST_RISE_FPS` times the
/// square root of the factor, and grow as they thin; they hold their
/// opacity for `DUST_HOLD` of their life, then thin out.
pub const DUST_PUFFS: f64 = 10.;
pub const DUST_PUFFS_PER_SCALE: f64 = 8.;
pub const DUST_PUFFS_MAX: usize = 24;
pub const DUST_SMALL_PUFFS: f64 = 3.;
pub const DUST_SMALL_PER_SCALE: f64 = 4.;
pub const DUST_SMALL_MAX: usize = 8;
pub const DUST_REACH: f64 = 1.0;
pub const DUST_ROLL: f64 = 0.35;
pub const DUST_ROLL_TICKS: f64 = 720.;
pub const DUST_RISE_FPS: f64 = 1.2;
pub const DUST_OPACITY: f32 = 0.4;
/// The puffs' width across against the drawn width, before they grow.
pub const DUST_PUFF: f64 = 0.42;
/// The share of the dust's life it holds before it thins out.
pub const DUST_HOLD: f64 = 0.4;
pub const DUST_FADE_IN_TICKS: f64 = 48.;
/// How long dust hangs in the driest air (John, 2026-10-12: 30 to 60 s by
/// the blast's size): 30 s for a 250 lb bomb's scale, growing with the
/// scale to 60 s at a 2,000 lb bomb's and beyond; the small version 15 to
/// 30 s. Damp air settles it sooner: down to `DUST_WET_LIFE` of that in
/// the most humid theater (agent decision).
pub const DUST_SECONDS: [f64; 2] = [30., 60.];
pub const DUST_SMALL_SECONDS: [f64; 2] = [15., 30.];
pub const DUST_WET_LIFE: f64 = 0.5;
/// The scale at which the dust's life is longest: a 2,000 lb bomb's.
pub const DUST_LONGEST_AT: f64 = 1.59;

/// Raised impacts remembered at most, the newest (the puff budget, agent
/// decision): one more retires the oldest, whose dust fades out over
/// `RETIRE_TICKS` instead of vanishing; at most `MAX_RETIRING` fade at once.
pub const MAX_RAISED: usize = 32;
pub const MAX_RETIRING: usize = 8;
pub const RETIRE_TICKS: f64 = 360.;
/// Dust and vapor puffs drawn in one picture at most, the newest impacts
/// first: enough for every remembered impact at full count.
pub const MAX_PUFFS: usize = (MAX_RAISED + MAX_RETIRING) * DUST_PUFFS_MAX;

/// An impact that raised dust or vapor, as a picture draws it: where,
/// which explosion, its scale, ticks since it went off and how much of it
/// is left (1, or less while it retires).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Raised {
    pub position: Vector,
    pub kind: EffectKind,
    pub explosion: u8,
    pub scale: u8,
    pub age: f64,
    pub left: f64,
}

/// The longest any impact's dust or vapor can show, ticks: its dust in the
/// driest air.
pub fn lasts(class: Class, scale: u8) -> f64 {
    dust_ticks(class, scale, Climate { humidity: 0. }).max(VAPOR_TICKS * 2.)
}

/// How long the dust of an impact of `class` and `scale` hangs in `climate`.
pub fn dust_ticks(class: Class, scale: u8, climate: Climate) -> f64 {
    let factor = blast_scale::factor(scale);
    let ([short, long], from, to) = match class {
        Class::Full => (DUST_SECONDS, f64::from(FULL_FROM) / 100., DUST_LONGEST_AT),
        Class::Small => (
            DUST_SMALL_SECONDS,
            blast_scale::POWERED_FLOOR,
            f64::from(FULL_FROM) / 100.,
        ),
    };
    let t = ((factor - from) / (to - from)).clamp(0., 1.);
    let wet = DUST_WET_LIFE + (1. - DUST_WET_LIFE) * climate.dryness();
    (short + (long - short) * t) * wet * 120.
}

fn ease(t: f64) -> f64 {
    let t = t.clamp(0., 1.);
    1. - (1. - t).powi(3)
}

/// The dust and vapor puffs of `raised` in `climate`, the newest impact
/// first, at most [`MAX_PUFFS`].
pub fn puffs(raised: &[Raised], climate: Climate) -> Vec<Puff> {
    let mut newest: Vec<&Raised> = raised.iter().collect();
    newest.sort_by(|a, b| a.age.total_cmp(&b.age));
    let mut out = Vec::new();
    for r in newest {
        let Some(class) = class(r.kind, r.explosion, r.scale) else {
            continue;
        };
        let before = out.len();
        vapor(r, class, climate, &mut out);
        dust(r, class, climate, &mut out);
        if out.len() > MAX_PUFFS {
            out.truncate(before);
            break;
        }
    }
    out
}

fn width(r: &Raised) -> f64 {
    f64::from(blast::rolled_size(r.explosion, r.position)) * blast_scale::factor(r.scale)
}

/// The condensation cloud around the blast.
fn vapor(r: &Raised, class: Class, climate: Climate, out: &mut Vec<Puff>) {
    let strength = climate.vapor();
    let factor = blast_scale::factor(r.scale);
    let life = VAPOR_TICKS * factor.sqrt();
    if strength <= 0. || !(0. ..life).contains(&r.age) {
        return;
    }
    let (count, size) = match class {
        Class::Full => (VAPOR_PUFFS, 1.),
        Class::Small => (VAPOR_PUFFS_SMALL, 0.7),
    };
    let w = width(r) * size;
    let grow = ease(r.age / (VAPOR_GROW_TICKS * factor.sqrt()));
    let t = r.age / life;
    let opacity = VAPOR_OPACITY
        * strength as f32
        * r.left as f32
        * (r.age / 6.).min(1.) as f32
        * (1. - t as f32).powi(2);
    let p = r.position;
    let turn = f64::from(blast::pick(p, 1_200, 360)).to_radians();
    // A ring of vapor low round the blast, about two in three of the puffs,
    // and a thinner ring above it, staggered; none over the fireball, which
    // shows through the middle.
    let low = count * 2 / 3;
    for k in 0..count {
        let (around, up) = if k < low {
            (k as f64 / low as f64, VAPOR_LOW_DEG)
        } else {
            (
                (k - low) as f64 / (count - low) as f64 + 0.5 / low as f64,
                VAPOR_HIGH_DEG,
            )
        };
        let angle = turn + std::f64::consts::TAU * around;
        let jitter = 0.85 + 0.3 * f64::from(blast::pick(p, 1_201 + k as u64, 1000)) / 1000.;
        let reach = w * VAPOR_REACH * grow * jitter;
        let (flat, rise) = (up.to_radians().cos() * reach, up.to_radians().sin() * reach);
        out.push(Puff {
            position: [
                p[0] + angle.cos() * flat,
                p[1] + w * 0.12 + rise,
                p[2] + angle.sin() * flat,
            ],
            radius: w * (VAPOR_PUFF + (VAPOR_PUFF_END - VAPOR_PUFF) * grow) * jitter / 2.,
            opacity,
            dark: false,
            tint: crate::gun_flash::PLAIN,
        });
    }
}

/// The dust rolled out and hanging over the blast.
fn dust(r: &Raised, class: Class, climate: Climate, out: &mut Vec<Puff>) {
    let factor = blast_scale::factor(r.scale);
    let life = dust_ticks(class, r.scale, climate);
    if !(0. ..life).contains(&r.age) {
        return;
    }
    let (count, size) = match class {
        Class::Full => (
            ((DUST_PUFFS + DUST_PUFFS_PER_SCALE * factor).round() as usize).min(DUST_PUFFS_MAX),
            1.,
        ),
        Class::Small => (
            ((DUST_SMALL_PUFFS + DUST_SMALL_PER_SCALE * factor).round() as usize)
                .min(DUST_SMALL_MAX),
            0.7,
        ),
    };
    let dry = climate.dryness();
    // Wide in dry air, narrower in damp.
    let w = width(r) * size * (0.75 + 0.25 * dry);
    let t = r.age / life;
    let roll = 1. + DUST_ROLL * ease(r.age / DUST_ROLL_TICKS);
    let opacity = DUST_OPACITY
        * (0.35 + 0.65 * dry) as f32
        * r.left as f32
        * (r.age / DUST_FADE_IN_TICKS).min(1.) as f32
        * ((1. - t) / (1. - DUST_HOLD)).min(1.).powf(1.2) as f32;
    let tint = climate.dust_tint();
    let p = r.position;
    for k in 0..count as u64 {
        let salt = 1_300 + k * 4;
        let roll_of = |n: u64| f64::from(blast::pick(p, salt + n, 1000)) / 1000.;
        let angle = std::f64::consts::TAU * (k as f64 + roll_of(0)) / count as f64;
        let out_by = w * DUST_REACH * (0.15 + 0.85 * roll_of(1).sqrt()) * roll;
        let size = 0.75 + 0.5 * roll_of(2);
        let radius = w * DUST_PUFF * size * (0.6 + 0.4 * ease(r.age / 960.)) * (1. + 0.5 * t);
        out.push(Puff {
            position: [
                p[0] + angle.cos() * out_by,
                // Standing on the ground rather than half sunk in it.
                p[1] + radius * (0.5 + 0.3 * roll_of(3))
                    + DUST_RISE_FPS * factor.sqrt() * r.age / 120.,
                p[2] + angle.sin() * out_by,
            ],
            radius,
            opacity,
            dark: false,
            tint,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HUMID: Climate = Climate { humidity: 0.9 };
    const ARID: Climate = Climate { humidity: 0.1 };

    fn raised(explosion: u8, scale: u8, age: f64) -> Raised {
        Raised {
            position: [1_000., 50., -2_000.],
            kind: EffectKind::Ground,
            explosion,
            scale,
            age,
            left: 1.,
        }
    }

    #[test]
    fn bombs_get_the_full_effect_missiles_a_smaller_one_and_guns_and_water_none() {
        // A 500 lb and a 2,000 lb bomb, an AGM-65G (84 %): full.
        assert_eq!(class(EffectKind::Ground, 35, 100), Some(Class::Full));
        assert_eq!(class(EffectKind::Ground, 36, 159), Some(Class::Full));
        assert_eq!(class(EffectKind::Hit, 22, 84), Some(Class::Full));
        // A Sidewinder or a rocket (the 50 % floor): small.
        assert_eq!(class(EffectKind::Ground, 21, 50), Some(Class::Small));
        // Gun hits on the ground and on aircraft, small splashes: none.
        for kind in [15, 16, 17, 18, 19, 20] {
            assert_eq!(class(EffectKind::Ground, kind, 50), None);
        }
        // A large blast in the water keeps its spray only.
        assert_eq!(class(EffectKind::Ground, 34, 159), None);
        // A kill's own blast, a dud and an air burst raise nothing.
        assert_eq!(class(EffectKind::Destroyed, 35, 100), None);
        assert_eq!(class(EffectKind::Ground, 35, blast_scale::INERT), None);
        assert_eq!(class(EffectKind::Hit, 30, 100), None);
    }

    #[test]
    fn humid_air_throws_vapor_and_dry_air_lingering_tan_dust() {
        let at = |climate, age| puffs(&[raised(35, 100, age)], climate);
        // Half a second in: the humid blast is wrapped in white vapor, the
        // arid one has none.
        let white = |p: &[Puff]| {
            p.iter()
                .filter(|p| p.tint == crate::gun_flash::PLAIN)
                .count()
        };
        assert_eq!(white(&at(HUMID, 60.)), VAPOR_PUFFS);
        assert_eq!(white(&at(ARID, 60.)), 0);
        // Ten seconds on the vapor is gone everywhere; dust remains, more
        // of it and tan in the dry air.
        let ten = (at(HUMID, 1_200.), at(ARID, 1_200.));
        assert_eq!(white(&ten.0), 0);
        let opacity = |p: &[Puff]| p.iter().map(|p| p.opacity).fold(0., f32::max);
        assert!(opacity(&ten.1) > opacity(&ten.0));
        let tint = ten.1[0].tint;
        assert!(tint[0] > tint[2] + 0.2, "tan");
        // The dust is wider in dry air.
        let reach = |p: &[Puff]| p.iter().map(|p| p.radius).fold(0., f64::max);
        assert!(reach(&ten.1) > reach(&ten.0));
    }

    #[test]
    fn dust_lingers_thirty_to_sixty_seconds_by_size_and_less_when_damp() {
        let s = |class, scale, climate| dust_ticks(class, scale, climate) / 120.;
        let driest = Climate { humidity: 0. };
        assert_eq!(s(Class::Full, 79, driest), 30.);
        assert_eq!(s(Class::Full, 159, driest), 60.);
        assert_eq!(s(Class::Full, 208, driest), 60.);
        let mid = s(Class::Full, 100, driest);
        assert!(mid > 30. && mid < 60.);
        assert_eq!(s(Class::Small, 50, driest), 15.);
        assert!(s(Class::Small, 78, driest) <= 30.);
        // Damp air settles it sooner, down to half.
        assert_eq!(s(Class::Full, 159, Climate { humidity: 1. }), 30.);
        // A 2,000 lb bomb's dust is still up at 50 s in the desert, a
        // 500 lb bomb's gone; nothing at all after a minute.
        let arid = Climate { humidity: 0. };
        assert!(!puffs(&[raised(35, 159, 50. * 120.)], arid).is_empty());
        assert!(puffs(&[raised(35, 100, 50. * 120.)], arid).is_empty());
        assert!(puffs(&[raised(35, 208, 60. * 120.)], arid).is_empty());
        // A missile's dust is less than a bomb's.
        let small = puffs(&[raised(21, 50, 600.)], arid);
        let full = puffs(&[raised(35, 100, 600.)], arid);
        assert!(!small.is_empty() && small.len() < full.len());
    }

    #[test]
    fn a_long_bombing_run_stays_within_the_puff_budget_newest_first() {
        // Sixty 2,000 lb bombs, a second apart, in a humid and an arid theater.
        let run: Vec<Raised> = (0..60)
            .map(|n| Raised {
                position: [f64::from(n) * 500., 0., 0.],
                ..raised(36, 208, f64::from(59 - n) * 120.)
            })
            .collect();
        for climate in [HUMID, ARID] {
            let drawn = puffs(&run, climate);
            assert!(drawn.len() <= MAX_PUFFS);
            // The newest bomb (the last) is drawn.
            assert!(drawn.iter().any(|p| (p.position[0] - 29_500.).abs() < 800.));
        }
        // The same impacts give the same puffs on every machine.
        assert_eq!(puffs(&run, ARID), puffs(&run, ARID));
        // A retiring impact's dust thins out.
        let full = puffs(&[raised(35, 100, 600.)], ARID);
        let fading = puffs(
            &[Raised {
                left: 0.25,
                ..raised(35, 100, 600.)
            }],
            ARID,
        );
        assert!(fading[0].opacity < full[0].opacity);
    }

    #[test]
    fn climate_colours_and_vapor_follow_humidity() {
        assert_eq!(ARID.vapor(), 0.);
        assert_eq!(HUMID.vapor(), 1.);
        let mid = Climate { humidity: 0.65 }.vapor();
        assert!(mid > 0. && mid < 1.);
        assert_eq!(Climate { humidity: 1. }.ring_tint(), [1.; 3]);
        let dry = Climate { humidity: 0. }.ring_tint();
        assert!(dry[0] > dry[2]);
    }
}
