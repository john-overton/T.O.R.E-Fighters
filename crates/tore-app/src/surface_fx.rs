//! What the surface defenses look like in the air and on the ground: the
//! muzzle flash and firing light of AAA and ship guns, the flash and smoke of
//! a SAM launch, the light and dark puff of a flak burst, and the smoke
//! column over a destroyed unit. Presentation only: nothing here reaches the
//! simulation, the wire or a recording, and everything is rebuilt from the
//! picture (projectiles, effects, targets and marks), so a single-player
//! game, a networked client and a replay show the same.
//!
//! The firing lights and flashes ride the AC-130's point-light path
//! ([`crate::gun_flash`]); player-visible rules are in
//! docs/spec/surface-defenses.md#flak-bursts-gunfire-launches-and-light.
use crate::countermeasure_renderer::{FLAK_LIGHT_TICKS, FlareLight, flak_light};
use crate::gun_flash::{self, Drawn, Flash, Look, Puff, Shot};
use crate::snapshot::{AircraftPose, RenderSnapshot};
use std::collections::{BTreeMap, BTreeSet};
use tore_sim::{
    attitude::Vector,
    combat::{
        blast::{self, MarkKind},
        live::EffectKind,
    },
};

/// Surface unit ids: the theater layout's at `0x4000_0000`, the template's
/// at `0x5000_0000`, added trucks and radars above them. Rounds a surface
/// unit fires are owned by it.
pub fn is_unit(id: u32) -> bool {
    (0x4000_0000..0x6000_0000).contains(&id)
}

/// How a gun record's muzzle looks: an index into [`LOOKS`]. 0 is the
/// light cannon (the Shilka, the Vulcan, the 30 mm and 25 mm), 1 the 37 mm
/// and 57 mm, 2 the 85 mm and 100 mm flak and the tank guns. Small arms and
/// the invisible barrage zone have no flash.
pub fn muzzle_class(record: &str) -> Option<usize> {
    match record.to_ascii_uppercase().trim_end_matches(".JT") {
        "ZSU23" | "2S6" | "PHALANX" | "AAA30" | "AAA30BAD" | "BMP2" | "BTR80" | "M113" | "M2" => {
            Some(0)
        }
        "ZSU57" | "M1939" => Some(1),
        "KS12" | "KS19" | "M1" | "T72" => Some(2),
        _ => None,
    }
}

/// How one gun class fires (opinionated, agent, 2026-10-10). The shape and
/// the envelope are the AC-130's ([`gun_flash::LOOKS`]); a surface gun is a
/// bigger flame seen from farther off, so its flash is longer and its light
/// stronger than the same calibre on the gunship. Light is in the flare
/// light's units, feet squared.
pub const LOOKS: [Look; 3] = [
    Look {
        life: 5.,
        length: 8.,
        light: 160.,
        puffs: 0,
        puff_life: 0.,
        puff_radius: [0.; 2],
        puff_opacity: 0.,
    },
    Look {
        life: 9.,
        length: 14.,
        light: 340.,
        puffs: 1,
        puff_life: 240.,
        puff_radius: [3., 12.],
        puff_opacity: 0.35,
    },
    Look {
        life: 16.,
        length: 28.,
        light: 700.,
        puffs: 3,
        puff_life: 420.,
        puff_radius: [5., 26.],
        puff_opacity: 0.5,
    },
];

/// A SAM leaving its rail (opinionated, agent, 2026-10-10): the motor's
/// flash for `FLASH_TICKS` along the missile's heading, a point light that
/// fades over `LIGHT_TICKS` from `LIGHT` (one flare's strength is 4,000), and
/// a cloud of white smoke on the pad that grows for `CLOUD_TICKS`. The
/// missile's own trail is the simulation's.
pub mod launch {
    /// A missile whose speed has reached this has its motor lit.
    pub const MOTOR_FPS: f64 = 30.;
    pub const FLASH_TICKS: f64 = 18.;
    pub const FLASH_LENGTH: f64 = 40.;
    pub const LIGHT: f64 = 2_800.;
    pub const LIGHT_TICKS: f64 = 40.;
    pub const CLOUD_TICKS: f64 = 960.;
    pub const CLOUD_PUFFS: usize = 6;
    pub const CLOUD_RADIUS: [f64; 2] = [10., 60.];
    pub const CLOUD_OPACITY: f32 = 0.6;
}

/// The dark puff a flak burst leaves (opinionated, agent, 2026-10-10): three
/// puffs, from `START` of the explosion's drawn width at the burst to `END`
/// of it, fading in over `FADE_IN_TICKS` (the fireball hides them first) and
/// out by `LIFE_TICKS` (about four seconds), rising a little as they hang.
pub mod puff {
    pub const COUNT: usize = 3;
    pub const START: f64 = 0.3;
    pub const END: f64 = 0.8;
    pub const FADE_IN_TICKS: f64 = 24.;
    pub const LIFE_TICKS: f64 = 480.;
    pub const OPACITY: f32 = 0.55;
    pub const RISE_FPS: f64 = 3.;
}

/// The smoke over a destroyed surface unit (opinionated, agent,
/// 2026-10-10): one dark puff every `STEP_TICKS`, each rising `RISE_FPS` and
/// carried a fixed breeze, growing from `START_FT` by `GROWTH_FPS`, living
/// `PUFF_TICKS`, for `LIFE_TICKS` (15 minutes) with the last `FADE_TICKS`
/// fading the column out. A bigger unit (by its hit points) smokes bigger.
pub mod wreck {
    pub const STEP_TICKS: u64 = 48;
    pub const PUFF_TICKS: u64 = 3_600;
    pub const RISE_FPS: f64 = 16.;
    pub const BREEZE_FPS: f64 = 7.;
    pub const START_FT: f64 = 6.;
    pub const GROWTH_FPS: f64 = 3.;
    pub const OPACITY: f32 = 0.6;
    pub const LIFE_TICKS: u64 = 15 * 60 * 120;
    pub const FADE_TICKS: u64 = 60 * 120;
    /// A column already burning when first seen (a restart in progress, a
    /// seek) starts this old, so it stands at once.
    pub const WARM_TICKS: u64 = 1_200;
    /// A column's smoke starts this far above the unit's point.
    pub const SOURCE_FT: f64 = 6.;
    /// Wrecks drawn at once, the newest first.
    pub const MAX_COLUMNS: usize = 24;
    /// A wreck with a crash-site fire this close already has the
    /// simulation's own column.
    pub const FIRE_NEAR_FT: f64 = 60.;
    pub fn scale(hit_points: i32) -> f64 {
        (f64::from(hit_points.max(1)) / 100.)
            .powf(0.25)
            .clamp(0.6, 2.4)
    }
}

/// One gun's newest shot, by shooter and muzzle.
type GunKey = (u32, [i32; 3]);

#[derive(Clone, Copy, Debug, PartialEq)]
struct Launch {
    position: Vector,
    direction: Vector,
    at: f64,
    serial: u32,
}

/// One flak burst: where it went off, as which explosion and on which tick.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Burst {
    position: Vector,
    blast: u8,
    born: u64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Wreck {
    position: Vector,
    born: f64,
    scale: f64,
}

/// Rounds, bursts and wrecks seen so far, and what they still show.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    seen: BTreeSet<u32>,
    /// Surface missiles whose motor has lit.
    launched: BTreeSet<u32>,
    shots: BTreeMap<GunKey, Shot>,
    launches: Vec<Launch>,
    bursts: BTreeMap<(i64, i64, i64, u64), Burst>,
    alive: BTreeSet<u32>,
    wrecks: BTreeMap<u32, Wreck>,
    /// The fires of destroyed units and how wide each is drawn.
    fires: Vec<(Vector, f64)>,
    /// The tick each destroyed unit died on, when the picture's source knows
    /// it (a replay does): a wreck first seen already dead starts its smoke
    /// then, not as if it had just died ten seconds ago.
    deaths: BTreeMap<u32, u64>,
    serial: u32,
}

/// An effect that is a flak burst: the simulation's own flak effect, or an
/// air burst of the heavy-flak type 27, which is what a replay of an older
/// recording stores for one.
fn flak(effect: &crate::snapshot::EffectPose) -> Option<u8> {
    match (effect.kind, effect.blast) {
        (EffectKind::Flak, Some(kind)) => Some(kind),
        (EffectKind::Flak, None) => Some(27),
        (_, Some(27)) => Some(27),
        _ => None,
    }
}

fn cell(position: Vector) -> (i64, i64, i64) {
    (
        position[0].round() as i64,
        position[1].round() as i64,
        position[2].round() as i64,
    )
}

fn unit_noise(seed: u64) -> f64 {
    let mut v = seed.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    v ^= v >> 31;
    v = v.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    v ^= v >> 29;
    (v >> 11) as f64 / (1_u64 << 53) as f64
}

impl Tracker {
    /// Notes this picture's new rounds, bursts and wrecks at presentation
    /// tick `now`. `primed` is false for the first picture and after a seek
    /// or a stall: what it holds was already going on, so no gun flashes and
    /// no launch shows, and the wrecks start over.
    pub fn observe(&mut self, picture: &RenderSnapshot, now: f64, primed: bool) {
        if !primed {
            self.shots.clear();
            self.launches.clear();
            self.bursts.clear();
            self.wrecks.clear();
            self.alive.clear();
        }
        let mut seen = BTreeSet::new();
        let mut launched = BTreeSet::new();
        for p in picture.projectiles.iter().filter(|p| is_unit(p.owner)) {
            seen.insert(p.id);
            if !p.gun {
                // A missile launches when its motor lights, which can be
                // seconds after it first sits on the rail.
                let lit = f64::from(p.speed_f8) / 256. >= launch::MOTOR_FPS;
                if lit {
                    launched.insert(p.id);
                }
                if primed && lit && !self.launched.contains(&p.id) {
                    self.serial = self.serial.wrapping_add(1);
                    self.launches.push(Launch {
                        position: p.position,
                        direction: p.direction,
                        at: now,
                        serial: self.serial,
                    });
                }
                continue;
            }
            if !primed || self.seen.contains(&p.id) {
                continue;
            }
            self.serial = self.serial.wrapping_add(1);
            let Some(slot) = muzzle_class(&p.weapon) else {
                continue;
            };
            let muzzle = p.previous;
            let key = (p.owner, muzzle.map(|v| (v / 8.).round() as i32));
            let gap = self.shots.get(&key).map(|before| now - before.at);
            self.shots.insert(
                key,
                Shot {
                    aircraft: p.owner,
                    slot,
                    at: now,
                    gap,
                    muzzle,
                    direction: p.direction,
                    serial: self.serial,
                },
            );
        }
        self.launched = launched;
        self.seen = seen;
        self.expire(now);
        for effect in &picture.effects {
            let Some(kind) = flak(effect) else { continue };
            let Some(row) = blast::explosion(kind) else {
                continue;
            };
            let length = u64::from(row.seconds) * 120;
            let born = picture
                .tick
                .saturating_sub(length.saturating_sub(u64::from(effect.ticks)));
            let (x, y, z) = cell(effect.position);
            self.bursts.insert(
                (x, y, z, born),
                Burst {
                    position: effect.position,
                    blast: kind,
                    born,
                },
            );
        }
        self.bursts
            .retain(|_, b| (picture.tick.saturating_sub(b.born) as f64) <= puff::LIFE_TICKS + 1.);
        self.wrecks_of(picture);
    }

    /// A replay's record of when each destroyed unit died.
    pub fn set_deaths(&mut self, deaths: BTreeMap<u32, u64>) {
        self.deaths = deaths;
    }

    fn expire(&mut self, now: f64) {
        self.shots.retain(|_, s| {
            let look = LOOKS[s.slot];
            now - s.at <= look.life.max(look.puff_life) + 1.
        });
        self.launches
            .retain(|l| now - l.at <= launch::CLOUD_TICKS + 1.);
    }

    /// Destroyed surface units with no crash fire of their own start a
    /// column the tick they are first seen dead after being seen alive.
    fn wrecks_of(&mut self, picture: &RenderSnapshot) {
        let mut alive = BTreeSet::new();
        // A wreck that has since lit its own fire has the simulation's column.
        self.wrecks
            .retain(|_, w| fire_at(picture, w.position).is_none());
        self.fires.clear();
        for pose in picture.targets.iter().filter(|p| is_surface_row(p)) {
            if pose.crashed {
                if let Some(fire) = fire_at(picture, pose.position) {
                    // Its fire is drawn as big as the unit.
                    self.fires.push((fire, fire_width(pose.damage.initial_hp)));
                    continue;
                }
                if self.wrecks.contains_key(&pose.id) {
                    continue;
                }
                let born = match self.deaths.get(&pose.id) {
                    Some(died) if *died <= picture.tick => *died,
                    _ if self.alive.contains(&pose.id) => picture.tick,
                    _ => picture.tick.saturating_sub(wreck::WARM_TICKS),
                };
                self.wrecks.insert(
                    pose.id,
                    Wreck {
                        position: pose.position,
                        born: born as f64,
                        scale: wreck::scale(pose.damage.initial_hp),
                    },
                );
            } else {
                alive.insert(pose.id);
                // A restart brings a unit back: its column ends.
                self.wrecks.remove(&pose.id);
            }
        }
        self.alive = alive;
    }

    /// The shots, lights, puffs and wreck smoke showing at tick `now`.
    pub fn draw(&self, now: f64, out: &mut Drawn) {
        for shot in self.shots.values() {
            let look = LOOKS[shot.slot];
            let age = now - shot.at;
            if age < 0. {
                continue;
            }
            for k in 0..look.puffs {
                if let Some(puff) = gun_flash::puff(shot, &look, age, k) {
                    out.puffs.push(puff);
                }
            }
            let Some((intensity, scale, seed)) = gun_flash::envelope(shot, &look, now) else {
                continue;
            };
            let length = look.length * scale;
            if out.flashes.len() < gun_flash::MAX_FLASHES {
                out.flashes.push(Flash {
                    position: shot.muzzle,
                    direction: shot.direction,
                    length,
                    intensity,
                    seed,
                    kind: shot.slot as u32,
                });
            }
            out.lights.push(FlareLight {
                position: std::array::from_fn(|i| {
                    shot.muzzle[i] + shot.direction[i] * length * 0.3
                }),
                strength: look.light * intensity,
            });
        }
        for launch in &self.launches {
            self.draw_launch(launch, now, out);
        }
        for burst in self.bursts.values() {
            draw_burst(burst, now, out);
        }
        self.draw_wrecks(now, out);
        out.fires.extend(self.fires.iter().copied());
    }

    fn draw_launch(&self, launch: &Launch, now: f64, out: &mut Drawn) {
        let age = now - launch.at;
        if age < 0. {
            return;
        }
        if age < launch::FLASH_TICKS && out.flashes.len() < gun_flash::MAX_FLASHES {
            let t = age / launch::FLASH_TICKS;
            out.flashes.push(Flash {
                position: launch.position,
                direction: launch.direction,
                length: launch::FLASH_LENGTH * (0.8 + 0.4 * t),
                intensity: (1. - t).powi(2),
                seed: launch.serial.wrapping_mul(2_654_435_761),
                kind: 2,
            });
        }
        if age < launch::LIGHT_TICKS {
            let t = age / launch::LIGHT_TICKS;
            out.lights.push(FlareLight {
                position: launch.position,
                strength: launch::LIGHT * (1. - t).powi(2),
            });
        }
        if age < launch::CLOUD_TICKS {
            let t = age / launch::CLOUD_TICKS;
            for k in 0..launch::CLOUD_PUFFS {
                let angle = std::f64::consts::TAU
                    * (k as f64 + unit_noise(u64::from(launch.serial) ^ k as u64))
                    / launch::CLOUD_PUFFS as f64;
                let spread = launch::CLOUD_RADIUS[1] * (1. - (1. - t).powi(3)) * 0.8;
                out.puffs.push(Puff {
                    position: [
                        launch.position[0] + angle.cos() * spread,
                        launch.position[1] + 4. + 10. * t,
                        launch.position[2] + angle.sin() * spread,
                    ],
                    radius: launch::CLOUD_RADIUS[0]
                        + (launch::CLOUD_RADIUS[1] - launch::CLOUD_RADIUS[0])
                            * (1. - (1. - t).powi(2)),
                    opacity: launch::CLOUD_OPACITY * (1. - t as f32).powi(2),
                    dark: false,
                });
            }
        }
    }

    fn draw_wrecks(&self, now: f64, out: &mut Drawn) {
        let mut newest: Vec<(&u32, &Wreck)> = self.wrecks.iter().collect();
        newest.sort_by(|a, b| b.1.born.total_cmp(&a.1.born));
        for (id, w) in newest.into_iter().take(wreck::MAX_COLUMNS) {
            let t = now - w.born;
            if t < 0. || t >= wreck::LIFE_TICKS as f64 {
                continue;
            }
            let overall = ((wreck::LIFE_TICKS as f64 - t) / wreck::FADE_TICKS as f64).clamp(0., 1.);
            let angle = std::f64::consts::TAU * unit_noise(u64::from(*id));
            let breeze = wreck::BREEZE_FPS * (0.75 + 0.5 * unit_noise(u64::from(*id) ^ 0x55));
            let (dx, dz) = (angle.cos() * breeze, angle.sin() * breeze);
            let mut n = (t / wreck::STEP_TICKS as f64).floor() as u64;
            loop {
                let age = t - (n * wreck::STEP_TICKS) as f64;
                if age >= wreck::PUFF_TICKS as f64 {
                    break;
                }
                let s = age / 120.;
                let fade_in = (age / 360.).min(1.);
                let life = 1. - (age / wreck::PUFF_TICKS as f64).powf(1.5);
                // Each puff wanders a little more the higher it has risen.
                let jitter = |axis: u64| {
                    (unit_noise((u64::from(*id) << 20) ^ (n << 3) ^ axis) - 0.5)
                        * (3. + 0.9 * s)
                        * w.scale
                };
                out.puffs.push(Puff {
                    position: [
                        w.position[0] + dx * s + jitter(1),
                        w.position[1] + wreck::SOURCE_FT * w.scale + wreck::RISE_FPS * s,
                        w.position[2] + dz * s + jitter(2),
                    ],
                    radius: w.scale * (wreck::START_FT + wreck::GROWTH_FPS * s),
                    opacity: wreck::OPACITY * (fade_in * life * overall) as f32,
                    dark: true,
                });
                if n == 0 {
                    break;
                }
                n -= 1;
            }
        }
    }

    /// The simulation ticks a flak burst's light has been on at `now`, for
    /// tests.
    #[cfg(test)]
    fn bursts(&self) -> usize {
        self.bursts.len()
    }
    #[cfg(test)]
    fn wrecks(&self) -> usize {
        self.wrecks.len()
    }
    #[cfg(test)]
    fn launches(&self) -> usize {
        self.launches.len()
    }
    #[cfg(test)]
    fn shots(&self) -> impl Iterator<Item = &Shot> {
        self.shots.values()
    }
}

/// A flak burst's light and dark puff at `now`.
fn draw_burst(burst: &Burst, now: f64, out: &mut Drawn) {
    let age = now - burst.born as f64;
    if age < 0. {
        return;
    }
    let strength = flak_light(burst.blast, age);
    if strength > 0. && age < FLAK_LIGHT_TICKS {
        out.lights.push(FlareLight {
            position: burst.position,
            strength,
        });
    }
    if age >= puff::LIFE_TICKS {
        return;
    }
    let width = f64::from(blast::rolled_size(burst.blast, burst.position));
    let t = age / puff::LIFE_TICKS;
    let seed = burst.born ^ (burst.position[0].to_bits() >> 7);
    for k in 0..puff::COUNT {
        let around = std::f64::consts::TAU * unit_noise(seed ^ ((k as u64 + 1) * 0x1f));
        let off = width * 0.25 * unit_noise(seed ^ ((k as u64 + 9) * 0x3b));
        let grow = puff::START + (puff::END - puff::START) * (1. - (1. - t).powi(2));
        out.puffs.push(Puff {
            position: [
                burst.position[0] + around.cos() * off,
                burst.position[1] + puff::RISE_FPS * age / 120. + (k as f64 - 1.) * width * 0.15,
                burst.position[2] + around.sin() * off,
            ],
            radius: width * grow * (0.8 + 0.3 * k as f64 / puff::COUNT as f64),
            opacity: puff::OPACITY
                * (age / puff::FADE_IN_TICKS).min(1.) as f32
                * (1. - t as f32).powf(1.5),
            dark: true,
        });
    }
}

/// A ground row that is a surface unit: neither an aircraft nor in the air.
fn is_surface_row(pose: &AircraftPose) -> bool {
    pose.aircraft.is_none() && !pose.airborne && is_unit(pose.id)
}

/// The spot of the crash-site fire already burning by `position`, if any.
fn fire_at(picture: &RenderSnapshot, position: Vector) -> Option<Vector> {
    picture
        .marks
        .iter()
        .find(|m| {
            m.kind == MarkKind::Fire
                && (m.position[0] - position[0]).hypot(m.position[2] - position[2])
                    <= wreck::FIRE_NEAR_FT
        })
        .map(|m| m.position)
}

/// How wide a burning wreck's fire is drawn, in feet, by the unit's hit
/// points (opinionated, agent, 2026-10-10): 30 ft for a 100 point vehicle,
/// growing with the square root, from 24 ft to 140 ft, against the 100 ft of
/// a crash site.
pub fn fire_width(hit_points: i32) -> f64 {
    (30. * (f64::from(hit_points.max(1)) / 100.).sqrt()).clamp(24., 140.)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::{Damage, EffectPose, MarkPose, ProjectilePose};

    const UNIT: u32 = 0x5000_0007;

    fn round(id: u32, weapon: &str, gun: bool, at: Vector) -> ProjectilePose {
        ProjectilePose {
            id,
            owner: UNIT,
            weapon: weapon.into(),
            shape: None,
            gun,
            tracer: false,
            position: [at[0], at[1] + 20., at[2]],
            previous: at,
            direction: [0., 1., 0.],
            target: None,
            incoming: false,
            speed_f8: 0,
        }
    }
    fn picture(tick: u64) -> RenderSnapshot {
        RenderSnapshot {
            tick,
            ..RenderSnapshot::default()
        }
    }
    fn drawn(tracker: &Tracker, now: f64) -> Drawn {
        let mut out = Drawn::default();
        tracker.draw(now, &mut out);
        out
    }
    fn unit_pose(id: u32, hp: i32) -> AircraftPose {
        AircraftPose {
            id,
            position: [1000., 50., 2000.],
            crashed: hp <= 0,
            damage: Damage {
                hp,
                initial_hp: 100,
                ..Damage::default()
            },
            ..AircraftPose::default()
        }
    }

    #[test]
    fn a_new_round_of_a_surface_gun_is_one_flash_of_its_class() {
        let mut tracker = Tracker::default();
        let first = RenderSnapshot {
            projectiles: vec![round(1, "ZSU23.JT", true, [0., 10., 0.])],
            ..picture(100)
        };
        // The first picture only primes: its rounds were in the air already.
        tracker.observe(&first, 100., false);
        assert_eq!(tracker.shots().count(), 0);
        let next = RenderSnapshot {
            projectiles: vec![
                round(1, "ZSU23.JT", true, [0., 10., 0.]),
                round(2, "ZSU23.JT", true, [0., 10., 0.]),
                round(3, "ZSU57.JT", true, [300., 10., 0.]),
                round(4, "KS19.JT", true, [900., 10., 0.]),
                round(5, "A_M1939.JT", true, [0., 10., 600.]),
                round(6, "SMLARMS.JT", true, [0., 10., 900.]),
            ],
            ..picture(102)
        };
        tracker.observe(&next, 102., true);
        let slots: Vec<usize> = tracker.shots().map(|s| s.slot).collect();
        assert_eq!(
            slots,
            [0, 1, 2],
            "the barrage zone and small arms flash nothing"
        );
        // The same rounds seen again are not more shots.
        tracker.observe(&next, 103., true);
        assert_eq!(tracker.shots().count(), 3);
        // The shot sits on the muzzle, along the barrel, with its light.
        let out = drawn(&tracker, 102.);
        assert_eq!(out.flashes.len(), 3);
        assert_eq!(out.lights.len(), 3);
        assert_eq!(out.flashes[0].position, [0., 10., 0.]);
        assert_eq!(out.flashes[0].direction, [0., 1., 0.]);
        // Bigger guns throw more light, and the big ones leave smoke.
        assert!(out.lights[0].strength < out.lights[1].strength);
        assert!(out.lights[1].strength < out.lights[2].strength);
        assert_eq!(out.puffs.len(), LOOKS[1].puffs + LOOKS[2].puffs);
        // Gone after the flash's life; the smoke outlives it.
        let late = drawn(&tracker, 102. + LOOKS[2].life + 2.);
        assert!(late.flashes.is_empty() && late.lights.is_empty());
        assert!(!late.puffs.is_empty());
        // A seek starts over without flashing what the new picture holds.
        let back = RenderSnapshot {
            projectiles: vec![round(9, "ZSU23.JT", true, [0., 10., 0.])],
            ..picture(50)
        };
        tracker.observe(&back, 50., false);
        assert_eq!(tracker.shots().count(), 0);
    }

    #[test]
    fn a_firing_gun_shows_only_its_newest_flash_and_never_goes_dark() {
        let mut tracker = Tracker::default();
        tracker.observe(&picture(0), 0., false);
        for n in 1..=40_u32 {
            let tick = u64::from(n) * 3;
            let picture = RenderSnapshot {
                projectiles: vec![round(n, "ZSU23.JT", true, [0., 10., 0.])],
                ..picture(tick)
            };
            tracker.observe(&picture, tick as f64, true);
            let out = drawn(&tracker, tick as f64 + 1.);
            assert_eq!(out.flashes.len(), 1, "one flash per gun");
            assert!(
                out.flashes[0].intensity > 0.3,
                "a cannon at 1,200 a minute flickers"
            );
        }
        // Two guns of one unit (a ship's mounts) show a flash each.
        let both = RenderSnapshot {
            projectiles: vec![
                round(100, "AAA30.JT", true, [0., 10., 0.]),
                round(101, "AAA30.JT", true, [60., 10., 0.]),
            ],
            ..picture(200)
        };
        tracker.observe(&both, 200., true);
        assert_eq!(drawn(&tracker, 200.).flashes.len(), 2);
    }

    #[test]
    fn a_sam_leaving_its_rail_flashes_lights_and_clouds_the_pad() {
        let mut tracker = Tracker::default();
        tracker.observe(&picture(10), 10., false);
        // On the rail with its motor out: nothing yet.
        let mut missile = round(7, "SA6.JT", false, [100., 5., 100.]);
        let rail = RenderSnapshot {
            projectiles: vec![missile.clone()],
            ..picture(11)
        };
        tracker.observe(&rail, 11., true);
        assert_eq!(tracker.launches(), 0);
        // The motor lights: the launch.
        missile.speed_f8 = 100 * 256;
        let launch = RenderSnapshot {
            projectiles: vec![missile.clone()],
            ..picture(12)
        };
        tracker.observe(&launch, 12., true);
        assert_eq!(tracker.launches(), 1);
        let at = drawn(&tracker, 12.);
        assert_eq!(at.flashes.len(), 1);
        assert_eq!(at.flashes[0].kind, 2);
        assert_eq!(at.lights[0].strength, launch::LIGHT);
        assert_eq!(at.puffs.len(), launch::CLOUD_PUFFS);
        assert!(at.puffs.iter().all(|p| !p.dark));
        // The flash and light end well before the cloud does.
        let later = drawn(&tracker, 12. + launch::LIGHT_TICKS + 1.);
        assert!(later.flashes.is_empty() && later.lights.is_empty());
        assert!(
            later
                .puffs
                .iter()
                .all(|p| p.radius > launch::CLOUD_RADIUS[0])
        );
        let gone = drawn(&tracker, 12. + launch::CLOUD_TICKS + 5.);
        assert!(gone.puffs.is_empty());
        // The same missile in flight is not another launch; an aircraft's
        // missile (a small owner) is not a surface launch, nor is one that
        // was already flying when the picture started.
        tracker.observe(&launch, 13., true);
        assert_eq!(tracker.launches(), 1);
        let mut jet = round(8, "AIM9M.JT", false, [0.; 3]);
        jet.owner = 3;
        jet.speed_f8 = 900 * 256;
        let from_jet = RenderSnapshot {
            projectiles: vec![jet],
            ..picture(14)
        };
        tracker.observe(&from_jet, 14., true);
        assert_eq!(tracker.launches(), 1);
        let mut late = Tracker::default();
        late.observe(&launch, 12., false);
        assert_eq!(late.launches(), 0);
        late.observe(&launch, 13., true);
        assert_eq!(late.launches(), 0, "already flying at the first picture");
    }

    fn burst(kind: u8, ticks: u16, at: Vector) -> EffectPose {
        EffectPose {
            kind: EffectKind::Flak,
            position: at,
            ticks,
            blast: Some(kind),
        }
    }

    #[test]
    fn a_flak_burst_flashes_light_then_leaves_a_dark_puff_that_outlives_it() {
        let mut tracker = Tracker::default();
        tracker.observe(&picture(1000), 1000., false);
        // A 100 mm burst (type 28, one second) and an 85 mm one (type 27).
        let with = RenderSnapshot {
            effects: vec![
                burst(28, 119, [500., 4000., 0.]),
                burst(27, 239, [0., 4000., 500.]),
            ],
            ..picture(1001)
        };
        tracker.observe(&with, 1001., true);
        assert_eq!(tracker.bursts(), 2);
        let out = drawn(&tracker, 1001.);
        assert_eq!(out.lights.len(), 2);
        let strengths: Vec<f64> = out.lights.iter().map(|l| l.strength).collect();
        assert!(
            strengths.contains(&crate::countermeasure_renderer::FLAK_LIGHT_LARGE)
                && strengths.contains(&crate::countermeasure_renderer::FLAK_LIGHT_SMALL),
            "{strengths:?}"
        );
        assert!(out.puffs.iter().all(|p| p.dark));
        assert_eq!(out.puffs.len(), 2 * puff::COUNT);
        // The light is over in a few ticks; the puff hangs for about four
        // seconds, long after the effect itself is gone.
        let later = drawn(&tracker, 1001. + FLAK_LIGHT_TICKS + 1.);
        assert!(later.lights.is_empty());
        assert_eq!(later.puffs.len(), 2 * puff::COUNT);
        let gone = RenderSnapshot {
            effects: vec![],
            ..picture(1300)
        };
        tracker.observe(&gone, 1300., true);
        let hanging = drawn(&tracker, 1300.);
        assert_eq!(hanging.puffs.len(), 2 * puff::COUNT);
        assert!(
            hanging
                .puffs
                .iter()
                .all(|p| p.opacity > 0. && p.radius > 20.)
        );
        let end = RenderSnapshot {
            effects: vec![],
            ..picture(1001 + puff::LIFE_TICKS as u64 + 10)
        };
        tracker.observe(&end, end.tick as f64, true);
        assert!(drawn(&tracker, end.tick as f64).puffs.is_empty());
        assert_eq!(tracker.bursts(), 0);
        // The same burst seen on every picture is one burst.
        let mut again = Tracker::default();
        again.observe(&picture(1000), 1000., false);
        for tick in 1001..1010 {
            let p = RenderSnapshot {
                effects: vec![burst(27, 240 - (tick - 1000) as u16, [0., 4000., 500.])],
                ..picture(tick)
            };
            again.observe(&p, tick as f64, true);
        }
        assert_eq!(again.bursts(), 1);
        // Another explosion draws no flak light; a replay's flak (a hit of
        // type 27) does.
        let other = RenderSnapshot {
            effects: vec![
                EffectPose {
                    kind: EffectKind::Hit,
                    position: [1.; 3],
                    ticks: 40,
                    blast: Some(18),
                },
                EffectPose {
                    kind: EffectKind::Hit,
                    position: [9.; 3],
                    ticks: 239,
                    blast: Some(27),
                },
            ],
            ..picture(5000)
        };
        let mut t = Tracker::default();
        t.observe(&other, 5000., false);
        assert_eq!(t.bursts(), 1);
    }

    #[test]
    fn a_destroyed_unit_smokes_for_fifteen_minutes_and_a_bigger_one_smokes_bigger() {
        let mut tracker = Tracker::default();
        let alive = RenderSnapshot {
            targets: vec![unit_pose(UNIT, 100), unit_pose(UNIT + 1, 4000)],
            ..picture(100)
        };
        tracker.observe(&alive, 100., false);
        assert_eq!(tracker.wrecks(), 0);
        let mut small = unit_pose(UNIT, 0);
        small.damage.initial_hp = 100;
        let mut big = unit_pose(UNIT + 1, 0);
        big.damage.initial_hp = 4000;
        let dead = RenderSnapshot {
            targets: vec![small, big],
            ..picture(101)
        };
        tracker.observe(&dead, 101., true);
        assert_eq!(tracker.wrecks(), 2);
        // It starts with nothing and builds a column that rises.
        let start = drawn(&tracker, 101.);
        assert_eq!(start.puffs.len(), 2);
        let column = drawn(&tracker, 101. + 2400.);
        assert!(column.puffs.len() > 50);
        assert!(column.puffs.iter().all(|p| p.dark && p.opacity >= 0.));
        let top = column
            .puffs
            .iter()
            .map(|p| p.position[1])
            .fold(0., f64::max);
        assert!(top > 50. + 16. * 18., "{top}");
        let widest = |from: f64, to: f64| {
            column
                .puffs
                .iter()
                .filter(|p| (from..to).contains(&p.position[1]))
                .map(|p| p.radius)
                .fold(0., f64::max)
        };
        assert!(widest(0., 1e9) > 40.);
        // 15 minutes, the last minute fading, then nothing.
        let nearly = drawn(
            &tracker,
            101. + (wreck::LIFE_TICKS - wreck::FADE_TICKS / 2) as f64,
        );
        let full = drawn(&tracker, 101. + 5_000.);
        let mean = |d: &Drawn| {
            d.puffs.iter().map(|p| f64::from(p.opacity)).sum::<f64>() / d.puffs.len() as f64
        };
        assert!(mean(&nearly) < 0.6 * mean(&full));
        assert!(
            drawn(&tracker, 101. + wreck::LIFE_TICKS as f64 + 1.)
                .puffs
                .is_empty()
        );
        // The 4,000 point unit's puffs are bigger than the 100 point one's.
        assert!(wreck::scale(4000) > 2. * wreck::scale(100) * 0.9);
        // A restart brings it back and ends the column.
        let again = RenderSnapshot {
            targets: vec![unit_pose(UNIT, 100), unit_pose(UNIT + 1, 4000)],
            ..picture(5)
        };
        tracker.observe(&again, 5., false);
        assert_eq!(tracker.wrecks(), 0);
    }

    #[test]
    fn wrecks_that_have_a_crash_fire_aircraft_and_the_airborne_make_no_second_column() {
        let mut tracker = Tracker::default();
        let mut parked = unit_pose(UNIT + 2, 0);
        parked.position = [5000., 0., 5000.];
        let mut plane = unit_pose(UNIT + 3, 0);
        plane.aircraft = Some(tore_formats::aircraft::AircraftId::F18);
        let mut flying = unit_pose(UNIT + 4, 0);
        flying.airborne = true;
        let mut scenery = unit_pose(3, 0);
        scenery.position = [0.; 3];
        let picture = RenderSnapshot {
            targets: vec![parked, plane, flying, scenery, unit_pose(UNIT + 5, 0)],
            marks: vec![MarkPose {
                kind: MarkKind::Fire,
                position: [5010., 0., 5020.],
                age: 10,
                strength: 1.,
            }],
            ..picture(3000)
        };
        tracker.observe(&picture, 3000., false);
        // Only the plain destroyed unit; one first seen dead stands warm.
        assert_eq!(tracker.wrecks(), 1);
        // The parked one's fire is drawn at its unit's width, not a crash site's.
        assert_eq!(tracker.fires, vec![([5010., 0., 5020.], fire_width(100))]);
        // A wreck that lights its fire later gives up its own column.
        let mut late = Tracker::default();
        let mut dead = unit_pose(UNIT + 7, 0);
        dead.position = [9000., 0., 9000.];
        let bare = RenderSnapshot {
            targets: vec![dead.clone()],
            ..self::picture(10)
        };
        late.observe(&bare, 10., false);
        assert_eq!(late.wrecks(), 1);
        let lit = RenderSnapshot {
            marks: vec![MarkPose {
                kind: MarkKind::Fire,
                position: [9000., 0., 9000.],
                age: 1,
                strength: 1.,
            }],
            ..bare
        };
        late.observe(&lit, 11., true);
        assert_eq!(late.wrecks(), 0);
        assert_eq!(late.fires.len(), 1);
        let out = drawn(&tracker, 3000.);
        assert!(out.puffs.len() as u64 >= wreck::WARM_TICKS / wreck::STEP_TICKS);
        // Only the 24 newest columns draw.
        let many = RenderSnapshot {
            targets: (0..40).map(|n| unit_pose(UNIT + 10 + n, 0)).collect(),
            ..self::picture(400)
        };
        let mut crowded = Tracker::default();
        crowded.observe(&many, 400., false);
        assert_eq!(crowded.wrecks(), 40);
        let per_column = drawn(&crowded, 400.).puffs.len() / wreck::MAX_COLUMNS;
        assert_eq!(
            drawn(&crowded, 400.).puffs.len(),
            per_column * wreck::MAX_COLUMNS
        );
    }

    #[test]
    fn a_replay_that_knows_when_a_unit_died_starts_its_smoke_then() {
        let dead = RenderSnapshot {
            targets: vec![unit_pose(UNIT + 1, 0)],
            ..picture(9_000)
        };
        // Seeking straight to a wreck: it stands warm unless told better.
        let mut seeked = Tracker::default();
        seeked.observe(&dead, 9_000., false);
        assert_eq!(
            seeked.wrecks[&(UNIT + 1)].born,
            (9_000 - wreck::WARM_TICKS) as f64
        );
        let mut told = Tracker::default();
        told.set_deaths(BTreeMap::from([(UNIT + 1, 4_000)]));
        told.observe(&dead, 9_000., false);
        assert_eq!(told.wrecks[&(UNIT + 1)].born, 4_000.);
        // A death the playhead has not reached yet says nothing.
        let mut early = Tracker::default();
        early.set_deaths(BTreeMap::from([(UNIT + 1, 12_000)]));
        early.observe(&dead, 9_000., false);
        assert_eq!(
            early.wrecks[&(UNIT + 1)].born,
            (9_000 - wreck::WARM_TICKS) as f64
        );
    }

    #[test]
    fn a_fire_is_as_big_as_its_unit() {
        assert_eq!(fire_width(100), 30.);
        assert_eq!(fire_width(5), 24., "never smaller than a campfire");
        assert!(fire_width(650) > 2. * fire_width(100));
        assert_eq!(
            fire_width(4_000),
            140.,
            "a carrier's tops out above a crash site's 100 ft"
        );
        assert!(fire_width(500) < blast::FIRE_SIZE as f64);
    }

    #[test]
    fn muzzle_classes_follow_the_gun_records() {
        for light in [
            "ZSU23.JT",
            "2S6.JT",
            "PHALANX.JT",
            "AAA30.JT",
            "AAA30BAD.JT",
            "M2.JT",
        ] {
            assert_eq!(muzzle_class(light), Some(0), "{light}");
        }
        assert_eq!(muzzle_class("M1939.JT"), Some(1));
        assert_eq!(muzzle_class("ZSU57.JT"), Some(1));
        for heavy in ["KS12.JT", "KS19.JT", "M1.JT", "T72.JT"] {
            assert_eq!(muzzle_class(heavy), Some(2), "{heavy}");
        }
        assert_eq!(muzzle_class("A_M1939.JT"), None);
        assert_eq!(muzzle_class("AIM9M.JT"), None);
        // Every gun of the tuning table either flashes or is named here as
        // one that does not.
        for row in tore_sim::combat::surface_guns::TABLE {
            let silent = ["A_M1939.JT", "SMLARMS.JT"].contains(&row.record);
            assert_eq!(muzzle_class(row.record).is_none(), silent, "{}", row.record);
        }
    }
}
