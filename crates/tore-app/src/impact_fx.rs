//! The flash of light and the sparks of a weapon's impact (bug B11;
//! docs/spec/explosions.md, "Impact light and sparks"). Opinionated, agent
//! design within John's request of 2026-10-12 that every impact light the
//! night and throw sparks. Presentation only and derived from the picture's
//! effects alone (where, which explosion, how big and how far into its
//! life), so a single-player flight, a networked client and a replay show
//! the same, and nothing reaches the simulation, the wire or a recording.
//!
//! The light rides the flare light path (`countermeasure_renderer`), as a
//! flak burst's does; the sparks are short glowing streaks drawn with the
//! gun tracers' ribbon.
use crate::countermeasure_renderer::FLARE_STRENGTH;
use crate::snapshot::EffectPose;
use tore_sim::{
    attitude::Vector,
    combat::{blast, blast_scale, live::EffectKind},
};

/// Light at the flash's peak per foot of the explosion's drawn width, in the
/// flare light's units: a 150 ft fireball lights as two and a half flares,
/// as an 85 mm flak burst does.
pub const LIGHT_PER_FT: f64 = 2.5 * FLARE_STRENGTH / 150.;
/// The weakest and strongest impact light: a fifth of a flare (a rifle
/// round's dust) to twelve flares (the heaviest bombs).
pub const LIGHT_MIN: f64 = 0.2 * FLARE_STRENGTH;
pub const LIGHT_MAX: f64 = 12. * FLARE_STRENGTH;
/// The flash holds full for two ticks, then fades over `LIGHT_TICKS` plus
/// `LIGHT_TICKS_PER_SCALE` times its weapon's size factor: about 17 ticks
/// for a cannon shell, 24 for a 500 lb bomb or the 105 mm, 32 for 2,000 lb.
pub const LIGHT_HOLD: f64 = 2.;
pub const LIGHT_TICKS: f64 = 10.;
pub const LIGHT_TICKS_PER_SCALE: f64 = 14.;
/// The flash stands this share of its width above where it went off, so
/// the ground around it is lit from above.
pub const LIGHT_RISE: f64 = 0.15;

/// Sparks thrown by one impact: `SPARKS` plus `SPARKS_PER_SCALE` times its
/// size factor, at most `SPARKS_MAX` (11 for a cannon shell, 16 for a 500 lb
/// bomb or the 105 mm, 22 for a 2,000 lb).
pub const SPARKS: f64 = 6.;
pub const SPARKS_PER_SCALE: f64 = 10.;
pub const SPARKS_MAX: usize = 30;
/// Sparks drawn in one picture at most, the newest impacts first.
pub const MAX_SPARKS: usize = 1_500;
/// A spark leaves at 60 to 160 ft/s times the square root of the size
/// factor, falls under gravity and lives 24 to 48 ticks plus 12 per unit of
/// the factor, fading as it cools.
pub const SPARK_SPEED: [f64; 2] = [60., 160.];
pub const SPARK_TICKS: [u16; 2] = [24, 48];
pub const SPARK_TICKS_PER_SCALE: f64 = 12.;
pub const GRAVITY_FPS2: f64 = 32.2;
/// The glowing streak: the last `STREAK_SECONDS` of its path, at
/// `SPARK_BRIGHTNESS` times a tracer's radiance at first and `SPARK_WIDTH`
/// times its width.
pub const STREAK_SECONDS: f64 = 0.05;
pub const SPARK_BRIGHTNESS: f64 = 2.2;
pub const SPARK_WIDTH: f64 = 0.5;

/// The smoke a ground burst leaves (opinionated, John, 2026-10-12: the
/// smoke grows and lingers with the weapon's size; the numbers are agent
/// choices): `SMOKE_PUFFS` plus `SMOKE_PUFFS_PER_SCALE` times the size
/// factor dark dust puffs (3 for a cannon shell, 5 for a 500 lb bomb, 7 for
/// a 2,000 lb; the 105 mm as a 500 lb bomb), each from `SMOKE_START` to `SMOKE_END` of
/// the explosion's drawn width across, rising `SMOKE_RISE_FPS` times the
/// square root of the factor, fading in over `SMOKE_FADE_IN_TICKS` (the
/// fireball hides it first) and out by `SMOKE_TICKS` times the factor (6 s
/// for a 500 lb bomb, 9.5 s for a 2,000 lb, 3 s for a cannon shell).
pub const SMOKE_PUFFS: f64 = 1.;
pub const SMOKE_PUFFS_PER_SCALE: f64 = 4.;
pub const SMOKE_PUFFS_MAX: usize = 10;
pub const SMOKE_START: f64 = 0.15;
pub const SMOKE_END: f64 = 0.45;
pub const SMOKE_RISE_FPS: f64 = 6.;
pub const SMOKE_FADE_IN_TICKS: f64 = 30.;
pub const SMOKE_TICKS: f64 = 720.;
pub const SMOKE_OPACITY: f32 = 0.55;

/// How long an impact shows anything (its flash or its smoke), ticks.
pub fn lasts(kind: u8, scale: u8) -> f64 {
    let factor = blast_scale::factor(scale);
    let smoke = if smokes(kind) {
        SMOKE_TICKS * factor
    } else {
        0.
    };
    smoke.max(LIGHT_HOLD + LIGHT_TICKS + LIGHT_TICKS_PER_SCALE * factor)
}

/// A ground (not water) explosion type, which leaves smoke.
fn smokes(kind: u8) -> bool {
    kind != 17 && kind != 34 && blast::explosion(kind).is_some_and(|row| row.surface)
}

/// The dust and smoke an impact leaves `age` ticks after it went off.
pub fn smoke(
    position: Vector,
    kind: u8,
    scale: u8,
    age: f64,
    out: &mut Vec<crate::gun_flash::Puff>,
) {
    if !smokes(kind) {
        return;
    }
    let factor = blast_scale::factor(scale);
    let life = SMOKE_TICKS * factor;
    if !(0. ..life).contains(&age) {
        return;
    }
    let width = f64::from(blast::rolled_size(kind, position)) * factor;
    let t = age / life;
    let count =
        ((SMOKE_PUFFS + SMOKE_PUFFS_PER_SCALE * factor).round() as usize).min(SMOKE_PUFFS_MAX);
    let grow = SMOKE_START + (SMOKE_END - SMOKE_START) * (1. - (1. - t).powi(2));
    for k in 0..count as u64 {
        let salt = 900 + k * 3;
        let around = f64::from(blast::pick(position, salt, 3600)).to_radians() / 10.;
        let off = width * 0.3 * f64::from(blast::pick(position, salt + 1, 1000)) / 1000.;
        let lift = f64::from(blast::pick(position, salt + 2, 1000)) / 1000.;
        out.push(crate::gun_flash::Puff {
            position: [
                position[0] + around.cos() * off,
                position[1]
                    + width * (0.1 + 0.25 * lift)
                    + SMOKE_RISE_FPS * factor.sqrt() * age / 120.,
                position[2] + around.sin() * off,
            ],
            radius: width * grow * (0.8 + 0.4 * lift),
            opacity: SMOKE_OPACITY
                * (age / SMOKE_FADE_IN_TICKS).min(1.) as f32
                * (1. - t as f32).powf(1.5),
            dark: true,
        });
    }
}

/// An effect that is a detonation of a weapon's impact or a kill: a typed
/// hit, ground strike or kill that is not a dud's inert puff, a flak burst
/// (which lights itself, `surface_fx`) or a splash in the water by a small
/// round (type 17).
pub fn detonation(effect: &EffectPose) -> Option<u8> {
    let kind = effect.blast?;
    let impact = matches!(
        effect.kind,
        EffectKind::Hit | EffectKind::Ground | EffectKind::Destroyed
    );
    (impact
        && blast_scale::detonates(effect.scale)
        && kind != 17
        && blast::explosion(kind).is_some())
    .then_some(kind)
}

/// The ticks since the effect went off and its whole life.
fn age(effect: &EffectPose, kind: u8) -> Option<(f64, f64)> {
    let life = u16::from(blast::explosion(kind)?.seconds) * 120;
    Some((
        f64::from(life.saturating_sub(effect.ticks)),
        f64::from(life),
    ))
}

/// The peak light of an impact of `width` feet drawn at size `factor`, and
/// the ticks it fades over after its hold.
pub fn light_of(width: f64, factor: f64) -> (f64, f64) {
    (
        (LIGHT_PER_FT * width).clamp(LIGHT_MIN, LIGHT_MAX),
        LIGHT_TICKS + LIGHT_TICKS_PER_SCALE * factor,
    )
}

/// The light an impact gives `age` ticks after it went off: where and how
/// strong, none before or after its flash.
pub fn light(position: Vector, kind: u8, scale: u8, age: f64) -> Option<(Vector, f64)> {
    let factor = blast_scale::factor(scale);
    let width = f64::from(blast::rolled_size(kind, position)) * factor;
    let (peak, fade) = light_of(width, factor);
    if !(0. ..LIGHT_HOLD + fade).contains(&age) {
        return None;
    }
    let left = 1. - (age - LIGHT_HOLD).max(0.) / fade;
    let rise = if blast::explosion(kind).is_some_and(|row| row.surface) {
        width * LIGHT_RISE
    } else {
        0.
    };
    Some((
        [position[0], position[1] + rise, position[2]],
        peak * left * left,
    ))
}

/// One spark's glowing streak: where it was a moment ago, where it is, and
/// how bright and wide to draw it against a tracer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spark {
    pub from: Vector,
    pub to: Vector,
    pub brightness: f64,
    pub width: f64,
}

/// The sparks of every detonation in `effects` that are still flying,
/// newest impact first, at most [`MAX_SPARKS`]. `ground` says the terrain's
/// height, so a spark that lands goes out.
pub fn sparks(effects: &[EffectPose], ground: impl Fn(f64, f64) -> f64) -> Vec<Spark> {
    let mut out = Vec::new();
    for effect in effects.iter().rev() {
        let Some(kind) = detonation(effect) else {
            continue;
        };
        // No sparks off the sea.
        if kind == 34 {
            continue;
        }
        let Some((elapsed, _)) = age(effect, kind) else {
            continue;
        };
        let factor = blast_scale::factor(effect.scale);
        let surface = blast::explosion(kind).is_some_and(|row| row.surface);
        let count = ((SPARKS + SPARKS_PER_SCALE * factor).round() as usize).min(SPARKS_MAX);
        let at = effect.position;
        let floor = ground(at[0], at[2]);
        for k in 0..count as u64 {
            if out.len() >= MAX_SPARKS {
                return out;
            }
            let salt = 500 + k * 5;
            let roll = |n: u64, bound: u16| f64::from(blast::pick(at, salt + n, bound));
            let life = f64::from(SPARK_TICKS[0])
                + roll(0, SPARK_TICKS[1] - SPARK_TICKS[0] + 1)
                + SPARK_TICKS_PER_SCALE * factor;
            if elapsed >= life {
                continue;
            }
            let azimuth = roll(1, 3600).to_radians() / 10.;
            // Off the ground the sparks fly up and out; in the air, every way.
            let elevation = if surface {
                (15. + roll(2, 61)).to_radians()
            } else {
                (-60. + roll(2, 141)).to_radians()
            };
            let speed = (SPARK_SPEED[0] + roll(3, (SPARK_SPEED[1] - SPARK_SPEED[0]) as u16 + 1))
                * factor.sqrt();
            let velocity = [
                speed * elevation.cos() * azimuth.sin(),
                speed * elevation.sin(),
                speed * elevation.cos() * azimuth.cos(),
            ];
            let place = |seconds: f64| -> Vector {
                let t = seconds.max(0.);
                [
                    at[0] + velocity[0] * t,
                    at[1] + velocity[1] * t - 0.5 * GRAVITY_FPS2 * t * t,
                    at[2] + velocity[2] * t,
                ]
            };
            let seconds = elapsed / 120.;
            let to = place(seconds);
            if surface && to[1] < floor.min(at[1]) - 1. {
                continue;
            }
            let cooled = 1. - elapsed / life;
            out.push(Spark {
                from: place(seconds - STREAK_SECONDS),
                to,
                brightness: SPARK_BRIGHTNESS * cooled.powf(1.5),
                width: SPARK_WIDTH,
            });
        }
    }
    out
}

/// The detonations in `effects` and the picture tick each went off on,
/// for a tracker that lights them at its own presentation time.
pub fn detonations(effects: &[EffectPose], tick: u64) -> Vec<(Vector, u8, u8, f64)> {
    effects
        .iter()
        .filter_map(|effect| {
            let kind = detonation(effect)?;
            let (elapsed, _) = age(effect, kind)?;
            Some((effect.position, kind, effect.scale, tick as f64 - elapsed))
        })
        .collect()
}

/// The flash of every detonation in `effects` still lighting, newest
/// first: where, how strong (in the flare light's units). Picture tick
/// based, so it needs nothing but the effects.
#[cfg(test)]
pub fn lights(effects: &[EffectPose]) -> Vec<(Vector, f64)> {
    effects
        .iter()
        .rev()
        .filter_map(|effect| {
            let kind = detonation(effect)?;
            let (elapsed, _) = age(effect, kind)?;
            light(effect.position, kind, effect.scale, elapsed)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn impact(kind: EffectKind, blast: u8, scale: u8, elapsed: u16) -> EffectPose {
        let life = u16::from(tore_sim::combat::blast::explosion(blast).unwrap().seconds) * 120;
        EffectPose {
            kind,
            position: [1_000., 100., 2_000.],
            ticks: life - elapsed,
            blast: Some(blast),
            scale,
        }
    }

    #[test]
    fn every_impact_flashes_and_a_bigger_one_flashes_brighter_and_longer() {
        let cannon = impact(EffectKind::Ground, 15, 50, 1);
        let bomb = impact(EffectKind::Ground, 35, 159, 1);
        let lit = |e: &EffectPose| lights(std::slice::from_ref(e));
        let (_, small) = lit(&cannon)[0];
        let (at, large) = lit(&bomb)[0];
        assert!(small >= LIGHT_MIN && small < large && large <= LIGHT_MAX);
        // The bomb's flash stands above the ground.
        assert!(at[1] > 100.);
        // The cannon's flash is out after its short fade; the bomb's is not.
        let late = |e: &EffectPose, ticks: u16| EffectPose {
            ticks: e.ticks - ticks,
            ..*e
        };
        assert!(lit(&late(&cannon, 20)).is_empty());
        assert!(!lit(&late(&bomb, 20)).is_empty());
        // In its hold the flash is at its peak; it fades after.
        let (_, held) = lit(&late(&bomb, 1))[0];
        let (_, fading) = lit(&late(&bomb, 10))[0];
        assert_eq!(held, large);
        assert!(fading < held);
    }

    #[test]
    fn duds_flak_launches_and_small_splashes_neither_flash_nor_spark() {
        let none = [
            impact(EffectKind::Ground, 15, blast_scale::INERT, 1),
            impact(EffectKind::Flak, 27, 100, 1),
            impact(EffectKind::Hit, 17, 50, 1),
            EffectPose {
                kind: EffectKind::Launch,
                position: [0.; 3],
                ticks: 40,
                blast: None,
                scale: 100,
            },
        ];
        assert!(lights(&none).is_empty());
        assert!(sparks(&none, |_, _| 0.).is_empty());
    }

    #[test]
    fn sparks_fly_out_fall_cool_and_go_out() {
        let ground = |_: f64, _: f64| 100.;
        let at = |elapsed| sparks(&[impact(EffectKind::Ground, 35, 100, elapsed)], ground);
        let early = at(3);
        assert_eq!(early.len(), 16);
        assert!(early.iter().all(|s| s.to[1] >= 100.));
        let later = at(20);
        // They spread out from the burst as they fly.
        let reach = |s: &[Spark]| {
            s.iter()
                .map(|s| (s.to[0] - 1_000.).hypot(s.to[2] - 2_000.))
                .fold(0., f64::max)
        };
        assert!(reach(&later) > reach(&early));
        // And cool.
        let brightest = |s: &[Spark]| s.iter().map(|s| s.brightness).fold(0., f64::max);
        assert!(brightest(&later) < brightest(&early));
        assert!(at(119).is_empty());
        // A cannon shell throws fewer than a 2,000 lb bomb.
        let shell = sparks(&[impact(EffectKind::Ground, 15, 50, 2)], ground);
        let howitzer = sparks(&[impact(EffectKind::Ground, 35, 159, 2)], ground);
        assert!(shell.len() < howitzer.len());
        // The same impact throws the same sparks on every machine.
        assert_eq!(at(10), at(10));
    }

    #[test]
    fn a_bigger_weapon_leaves_more_smoke_that_lingers_longer() {
        let at = [0., 100., 0.];
        let puffs = |scale: u8, age: f64| {
            let mut out = Vec::new();
            smoke(at, 35, scale, age, &mut out);
            out
        };
        // A 2,000 lb bomb's smoke outnumbers and outgrows a 500 lb bomb's,
        // and a 250 lb bomb's is smaller still.
        let [small, mid, big] = [79, 100, 159].map(|s| puffs(s, 120.));
        assert!(small.len() < mid.len() && mid.len() < big.len());
        let widest = |p: &[crate::gun_flash::Puff]| p.iter().map(|p| p.radius).fold(0., f64::max);
        assert!(widest(&small) < widest(&mid) && widest(&mid) < widest(&big));
        // Seven seconds on the 500 lb bomb's has gone, the 2,000 lb's lingers.
        assert!(puffs(100, 840.).is_empty());
        assert!(!puffs(159, 840.).is_empty());
        assert_eq!(lasts(35, 100), SMOKE_TICKS);
        // No smoke off the water or from an air burst.
        assert!({
            let mut out = Vec::new();
            smoke(at, 34, 159, 120., &mut out);
            smoke(at, 30, 100, 120., &mut out);
            out.is_empty()
        });
    }

    #[test]
    fn sparks_are_capped_newest_impacts_first() {
        let many: Vec<EffectPose> = (0..200)
            .map(|n| EffectPose {
                position: [f64::from(n) * 10., 100., 0.],
                ..impact(EffectKind::Ground, 22, 200, 2)
            })
            .collect();
        let drawn = sparks(&many, |_, _| 0.);
        assert_eq!(drawn.len(), MAX_SPARKS);
        // The last effect is the newest: its sparks come first.
        assert!((drawn[0].to[0] - 1_990.).abs() < 100.);
    }
}
