//! AC-130 muzzle flashes, the light they throw and the 105's blast smoke.
//! Presentation only: nothing here reaches the simulation, the wire or a
//! recording. Player-visible rules: docs/spec/ac130-linked-guns.md#muzzle-flash.
//!
//! A shot is a gunship round that is new in the picture: the live
//! simulation's own rounds in single player, the rounds a networked client
//! makes again from the host's gun bursts, and a replay's recorded rounds.
//! All three keep a round's number for its whole flight, so a number not seen
//! in the last picture is a round that has just left a barrel, whatever fire
//! rate the gun has. The flash is drawn at that barrel's tip as the barrel is
//! posed now, so it stays on the gun while the aircraft flies on.
use crate::countermeasure_renderer::FlareLight;
use crate::snapshot::{AircraftPose, GUN_AIM, RenderSnapshot};
use std::collections::{BTreeSet, VecDeque};
use std::f64::consts::{FRAC_PI_2, PI};
use tore_formats::aircraft::AircraftId;
use tore_sim::{
    attitude::{Basis, Vector, unit},
    combat::gunship,
};

/// How one calibre's firing looks (opinionated, agent, 2026-10-09).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Look {
    /// Ticks one shot's flash lasts.
    pub life: f64,
    /// Feet the flash reaches out along the barrel at full size.
    pub length: f64,
    /// Peak light, in the flare light's units (feet squared: the distance
    /// squared at which a facing surface is lit as brightly as full sun).
    pub light: f64,
    /// Blast smoke puffs left in the air, their life in ticks, their start
    /// and end radius in feet and their starting opacity.
    pub puffs: usize,
    pub puff_life: f64,
    pub puff_radius: [f64; 2],
    pub puff_opacity: f32,
}

/// The 25 mm, 40 mm and 105 mm, by gun slot.
pub const LOOKS: [Look; 3] = [
    Look {
        life: 5.,
        length: 5.,
        light: 60.,
        puffs: 0,
        puff_life: 0.,
        puff_radius: [0.; 2],
        puff_opacity: 0.,
    },
    Look {
        life: 9.,
        length: 9.,
        light: 130.,
        puffs: 1,
        puff_life: 180.,
        puff_radius: [2.5, 9.],
        puff_opacity: 0.35,
    },
    Look {
        life: 16.,
        length: 20.,
        light: 240.,
        puffs: 3,
        puff_life: 360.,
        puff_radius: [4., 20.],
        puff_opacity: 0.6,
    },
];

/// Two 25 mm shots this close together (ticks) are one unbroken, flickering
/// flash: the gatling reads as firing continuously at any rate it is given.
pub const BRIDGE_TICKS: f64 = 30.;
/// A picture this many ticks newer than the last one (a seek, a stall) is
/// taken as a fresh start: its rounds are not flashed.
const RESYNC_TICKS: u64 = 240;
/// Shots kept at once.
const MAX_SHOTS: usize = 256;
/// Flash instances drawn at once.
pub const MAX_FLASHES: usize = 64;
/// Muzzle position, intensity, barrel direction, length, seed and calibre.
pub const FLASH_BYTES: usize = 10 * 4;
/// The light sits this far out along the flash, as a fraction of its length.
const LIGHT_OUT: f64 = 0.3;

/// The gun slot a round of `weapon` (its record name) comes from.
pub fn slot(weapon: &str) -> Option<usize> {
    gunship::GUNS.iter().position(|name| *name == weapon)
}

/// Where an AC-130's guns are drawn: the aircraft's pose and its barrels'
/// train, in the snapshot's device units (heading over pi, elevation over a
/// right angle).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mount {
    pub position: Vector,
    pub basis: Basis,
    pub gun_aim: [[f64; 2]; 3],
}
impl Mount {
    /// A drawn aircraft or ground object, when it is an AC-130.
    pub fn of_pose(pose: &AircraftPose) -> Option<Self> {
        if pose.aircraft != Some(AircraftId::Ac130) {
            return None;
        }
        let devices = pose.devices.unwrap_or([0.; crate::snapshot::DEVICES]);
        let [yaw, pitch, bank] = pose.attitude;
        Some(Self {
            position: pose.position,
            basis: Basis::new(yaw, pitch, bank),
            gun_aim: std::array::from_fn(|slot| {
                [devices[GUN_AIM + slot * 2], devices[GUN_AIM + slot * 2 + 1]]
            }),
        })
    }
    /// A presented flight state (the player's own aircraft).
    pub fn of_state(s: &crate::flight::State) -> Self {
        Self {
            position: s.position,
            basis: Basis::new(s.yaw, s.pitch, s.bank),
            gun_aim: s.gun_aim,
        }
    }
    /// The barrel tip of gun `slot` in the world and the barrel's direction,
    /// as the drawn barrel points. A barrel with no train recorded (zero) is
    /// drawn as its mesh lies, so the flash follows the mesh too.
    pub fn muzzle(&self, slot: usize) -> (Vector, Vector) {
        let [heading, elevation] = self.gun_aim[slot];
        let (tip, direction) = if heading == 0. && elevation == 0. {
            let source = |p: Vector| {
                [
                    p[0] * gunship::SOURCE_SCALE,
                    p[2] * gunship::SOURCE_SCALE,
                    p[1] * gunship::SOURCE_SCALE,
                ]
            };
            let tip = source(gunship::TIPS_SOURCE[slot]);
            let pivot = source(gunship::PIVOTS_SOURCE[slot]);
            (tip, unit(std::array::from_fn(|i| tip[i] - pivot[i])))
        } else {
            let (h, e) = (heading * PI, elevation * FRAC_PI_2);
            (
                gunship::local_muzzle(slot, h, e),
                gunship::local_direction(h, e),
            )
        };
        let b = self.basis;
        let world = |v: Vector| -> Vector {
            std::array::from_fn(|i| b.right[i] * v[0] + b.up[i] * v[1] + b.forward[i] * v[2])
        };
        let offset = world(tip);
        (
            std::array::from_fn(|i| self.position[i] + offset[i]),
            unit(world(direction)),
        )
    }
}

/// The AC-130s in `picture` by id, the player first when it flies one:
/// `player` is the player's plane and its presented flight.
pub fn mounts<'a>(
    picture: &'a RenderSnapshot,
    player: Option<(u32, &'a crate::flight::State)>,
) -> impl Fn(u32) -> Option<Mount> + 'a {
    move |id| {
        if let Some((plane, state)) = player
            && plane == id
        {
            return Some(Mount::of_state(state));
        }
        if picture.player.id == id
            && let Some(mount) = Mount::of_pose(&picture.player)
        {
            return Some(mount);
        }
        picture.target(id).and_then(Mount::of_pose)
    }
}

/// One round let go, as a flash sees it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shot {
    pub aircraft: u32,
    pub slot: usize,
    /// The presentation tick it was seen at.
    pub at: f64,
    /// Ticks since the same gun's previous shot, when there was one.
    pub gap: Option<f64>,
    /// The muzzle and barrel direction when it fired, which the blast smoke
    /// keeps while the aircraft flies on.
    pub muzzle: Vector,
    pub direction: Vector,
    pub serial: u32,
}

/// One flash to draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flash {
    pub position: Vector,
    pub direction: Vector,
    /// Feet along the barrel.
    pub length: f64,
    /// 0 to 1.
    pub intensity: f64,
    pub seed: u32,
    /// The gun slot: 0 is the 25 mm, 1 the 40 mm, 2 the 105 mm.
    pub kind: u32,
}

/// One blast smoke puff, drawn with the original white smoke puff.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Puff {
    pub position: Vector,
    pub radius: f64,
    pub opacity: f32,
}

/// Everything a frame draws for the guns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Drawn {
    pub flashes: Vec<Flash>,
    pub lights: Vec<FlareLight>,
    pub puffs: Vec<Puff>,
}

/// The shots seen so far and the rounds the last picture held.
#[derive(Clone, Debug, Default)]
pub struct Tracker {
    seen: BTreeSet<u32>,
    tick: Option<u64>,
    shots: VecDeque<Shot>,
    serial: u32,
}

impl Tracker {
    /// Notes the gunship rounds new in `picture` as shots at presentation
    /// tick `now`. A picture older than the last one, or much newer, starts
    /// afresh without flashing what it holds.
    pub fn observe(
        &mut self,
        picture: &RenderSnapshot,
        now: f64,
        mounts: impl Fn(u32) -> Option<Mount>,
    ) {
        let primed = self
            .tick
            .is_some_and(|last| picture.tick >= last && picture.tick - last <= RESYNC_TICKS);
        if self.tick.is_some_and(|last| picture.tick < last) {
            self.shots.clear();
        }
        let mut seen = BTreeSet::new();
        for p in picture.projectiles.iter().filter(|p| p.gun) {
            let Some(slot) = slot(&p.weapon) else {
                continue;
            };
            seen.insert(p.id);
            if primed
                && !self.seen.contains(&p.id)
                && let Some(mount) = mounts(p.owner)
            {
                self.fire(p.owner, slot, now, &mount);
            }
        }
        self.seen = seen;
        self.tick = Some(picture.tick);
        self.expire(now);
    }

    /// Records one shot of gun `slot` of `aircraft` at `now`.
    pub fn fire(&mut self, aircraft: u32, slot: usize, now: f64, mount: &Mount) {
        let gap = self
            .shots
            .iter()
            .rev()
            .find(|s| s.aircraft == aircraft && s.slot == slot)
            .map(|s| now - s.at);
        let (muzzle, direction) = mount.muzzle(slot);
        self.serial = self.serial.wrapping_add(1);
        self.shots.push_back(Shot {
            aircraft,
            slot,
            at: now,
            gap,
            muzzle,
            direction,
            serial: self.serial,
        });
        while self.shots.len() > MAX_SHOTS {
            self.shots.pop_front();
        }
    }

    /// Forgets every shot and round, as a new flight or replay does.
    #[allow(dead_code)] // For a viewer that changes recordings.
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    fn expire(&mut self, now: f64) {
        self.shots
            .retain(|s| now - s.at <= LOOKS[s.slot].life.max(LOOKS[s.slot].puff_life) + 1.);
    }

    /// The shots still showing anything.
    #[cfg(test)]
    pub fn shots(&self) -> impl Iterator<Item = &Shot> {
        self.shots.iter()
    }

    /// The frame's flashes, lights and smoke at presentation tick `now`.
    pub fn draw(&self, now: f64, mounts: impl Fn(u32) -> Option<Mount>) -> Drawn {
        let mut drawn = Drawn::default();
        // Each gun shows only its newest flash; an older one still running
        // is the same continuous fire.
        let mut shown: BTreeSet<(u32, usize)> = BTreeSet::new();
        for shot in self.shots.iter().rev() {
            let look = LOOKS[shot.slot];
            let age = now - shot.at;
            if age < 0. {
                continue;
            }
            for k in 0..look.puffs {
                if let Some(puff) = puff(shot, &look, age, k) {
                    drawn.puffs.push(puff);
                }
            }
            if shown.contains(&(shot.aircraft, shot.slot)) {
                continue;
            }
            let Some((intensity, scale, seed)) = envelope(shot, &look, now) else {
                continue;
            };
            shown.insert((shot.aircraft, shot.slot));
            let Some(mount) = mounts(shot.aircraft) else {
                continue;
            };
            let (position, direction) = mount.muzzle(shot.slot);
            let length = look.length * scale;
            if drawn.flashes.len() < MAX_FLASHES {
                drawn.flashes.push(Flash {
                    position,
                    direction,
                    length,
                    intensity,
                    seed,
                    kind: shot.slot as u32,
                });
            }
            drawn.lights.push(FlareLight {
                position: std::array::from_fn(|i| position[i] + direction[i] * length * LIGHT_OUT),
                strength: look.light * intensity,
            });
        }
        drawn
    }
}

/// A shot's flash at `now`: its intensity, its size as a fraction of the
/// calibre's length and the seed that shapes it; none once it is over.
fn envelope(shot: &Shot, look: &Look, now: f64) -> Option<(f64, f64, u32)> {
    let age = now - shot.at;
    if shot.slot == 0 {
        // The gatling: a shot lasts until the next is due at the cadence it
        // is firing at, and flickers in size and brightness every tick.
        let life = shot
            .gap
            .filter(|gap| *gap <= BRIDGE_TICKS)
            .map_or(look.life, |gap| look.life.max(gap + 2.));
        if age >= life {
            return None;
        }
        let tick = now.floor() as u32;
        let seed = hash(shot.aircraft ^ hash(tick));
        let flicker = f64::from(hash(seed) >> 8) / f64::from(1_u32 << 24);
        let fade = ((life - age) / 2.).min(1.);
        return Some(((0.6 + 0.4 * flicker) * fade, 0.7 + 0.45 * flicker, seed));
    }
    if age >= look.life {
        return None;
    }
    let t = age / look.life;
    // A pop: full at once, then gone over the flash's life, growing a little
    // as it goes. The 105 holds its peak for two ticks.
    let hold = if shot.slot == 2 { 2. / look.life } else { 0. };
    let fall = ((t - hold).max(0.) / (1. - hold)).min(1.);
    let intensity = (1. - fall).powi(2);
    let scale = if shot.slot == 2 {
        0.75 + 0.55 * t
    } else {
        0.85 + 0.3 * t
    };
    Some((intensity, scale, hash(shot.serial.wrapping_mul(2654435761))))
}

/// The `k`th blast puff of a shot at `age` ticks: blown out along the barrel
/// and slowed by the air at once, it stays where the air took it while the
/// aircraft flies on, so it drifts aft of the guns.
fn puff(shot: &Shot, look: &Look, age: f64, k: usize) -> Option<Puff> {
    if age >= look.puff_life {
        return None;
    }
    let t = age / look.puff_life;
    let spread = (k + 1) as f64 / look.puffs as f64;
    let reach = look.length * (0.35 + 0.55 * spread) * (1. - (-age / 10.).exp());
    let wobble = f64::from(hash(shot.serial ^ hash(k as u32)) >> 8) / f64::from(1_u32 << 24);
    let radius = look.puff_radius[0]
        + (look.puff_radius[1] - look.puff_radius[0])
            * (1. - (1. - t).powi(3))
            * (0.8 + 0.4 * wobble);
    Some(Puff {
        position: std::array::from_fn(|i| {
            shot.muzzle[i] + shot.direction[i] * reach + if i == 1 { age / 120. * 2. } else { 0. }
        }),
        radius,
        opacity: look.puff_opacity * (1. - t as f32).powi(2),
    })
}

fn hash(value: u32) -> u32 {
    let mut v = value.wrapping_mul(747_796_405).wrapping_add(2_891_336_453);
    v = ((v >> ((v >> 28) + 4)) ^ v).wrapping_mul(277_803_737);
    (v >> 22) ^ v
}

/// The flashes as GPU instances, nearest camera first not needed: they add.
pub fn instances(flashes: &[Flash]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(flashes.len().min(MAX_FLASHES) * FLASH_BYTES);
    for flash in flashes.iter().take(MAX_FLASHES) {
        for value in flash
            .position
            .into_iter()
            .chain([flash.intensity])
            .chain(flash.direction)
            .chain([flash.length])
        {
            bytes.extend((value as f32).to_le_bytes());
        }
        bytes.extend(flash.seed.to_le_bytes());
        bytes.extend(flash.kind.to_le_bytes());
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::snapshot::ProjectilePose;

    fn ac130(id: u32) -> AircraftPose {
        let mut devices = [0.; crate::snapshot::DEVICES];
        // Every gun abeam left, 20 degrees down.
        for slot in 0..3 {
            devices[GUN_AIM + slot * 2] = -0.5;
            devices[GUN_AIM + slot * 2 + 1] = -20. / 90.;
        }
        AircraftPose {
            id,
            aircraft: Some(AircraftId::Ac130),
            position: [1000., 7000., -2000.],
            devices: Some(devices),
            ..AircraftPose::default()
        }
    }

    fn round(id: u32, owner: u32, weapon: &str) -> ProjectilePose {
        ProjectilePose {
            id,
            owner,
            weapon: weapon.into(),
            shape: None,
            gun: true,
            tracer: false,
            position: [0.; 3],
            previous: [0.; 3],
            direction: [0., 0., 1.],
            target: None,
            incoming: false,
            speed_f8: 0,
        }
    }

    fn picture(tick: u64, rounds: Vec<ProjectilePose>) -> RenderSnapshot {
        RenderSnapshot {
            tick,
            targets: vec![ac130(7)],
            projectiles: rounds,
            ..RenderSnapshot::default()
        }
    }

    #[test]
    fn a_round_new_in_the_picture_is_one_shot_of_its_gun() {
        let mut tracker = Tracker::default();
        let first = picture(100, vec![round(1, 7, "C_40.JT")]);
        // The first picture only primes: its rounds were in the air already.
        tracker.observe(&first, 100., mounts(&first, None));
        assert_eq!(tracker.shots().count(), 0);
        let next = picture(
            102,
            vec![
                round(1, 7, "C_40.JT"),
                round(2, 7, "C_105.JT"),
                round(3, 7, "AIM9M.JT"),
            ],
        );
        tracker.observe(&next, 102., mounts(&next, None));
        let shots: Vec<_> = tracker.shots().map(|s| (s.aircraft, s.slot)).collect();
        assert_eq!(shots, [(7, 2)]);
        // The same round seen again is not another shot.
        tracker.observe(&next, 103., mounts(&next, None));
        assert_eq!(tracker.shots().count(), 1);
        // A seek back clears the shots and primes again.
        let back = picture(50, vec![round(9, 7, "C_25.JT")]);
        tracker.observe(&back, 50., mounts(&back, None));
        assert_eq!(tracker.shots().count(), 0);
        // A round from an aircraft that is not drawn as an AC-130 flashes
        // nothing.
        let other = picture(51, vec![round(10, 8, "C_25.JT")]);
        tracker.observe(&other, 51., mounts(&other, None));
        assert_eq!(tracker.shots().count(), 0);
    }

    #[test]
    fn the_flash_sits_on_the_drawn_barrel_tip_along_the_barrel() {
        let pose = ac130(7);
        let mount = Mount::of_pose(&pose).unwrap();
        for slot in 0..3 {
            let (tip, direction) = mount.muzzle(slot);
            let h = -0.5 * PI;
            let e = -20_f64.to_radians();
            let launcher_tip = gunship::local_muzzle(slot, h, e);
            for i in 0..3 {
                assert!((tip[i] - pose.position[i] - launcher_tip[i]).abs() < 1e-9);
            }
            // Left of the aircraft and pointing down.
            assert!(direction[0] < -0.9 && direction[1] < -0.3);
        }
        // An untrained barrel flashes where its mesh points: out to the left.
        let mut still = pose.clone();
        still.devices = None;
        let (_, direction) = Mount::of_pose(&still).unwrap().muzzle(2);
        assert!(direction[0] < -0.8);
    }

    #[test]
    fn calibres_flash_and_light_by_their_own_rules() {
        let pose = ac130(7);
        let mount = Mount::of_pose(&pose).unwrap();
        let lookup = |id: u32| (id == 7).then_some(mount);
        // The 105: one strong pulse with smoke that outlives it.
        let mut tracker = Tracker::default();
        tracker.fire(7, 2, 0., &mount);
        let peak = tracker.draw(1., lookup);
        assert_eq!(peak.flashes.len(), 1);
        assert_eq!(peak.flashes[0].intensity, 1.);
        assert_eq!(peak.lights[0].strength, LOOKS[2].light);
        assert_eq!(peak.puffs.len(), 3);
        let late = tracker.draw(12., lookup);
        assert!(late.lights[0].strength < 0.2 * LOOKS[2].light);
        let after = tracker.draw(20., lookup);
        assert!(after.flashes.is_empty() && after.lights.is_empty());
        assert_eq!(after.puffs.len(), 3);
        assert!(tracker.draw(400., lookup).puffs.is_empty());
        // The puffs stay where the blast left them: aft of a moving aircraft.
        let mut moved = mount;
        moved.position[2] += 500.;
        let puff = tracker.draw(60., |_| Some(moved)).puffs[0];
        assert!((puff.position[2] - mount.muzzle(2).0[2]).abs() < 50.);

        // The 25 mm at any steady cadence never goes dark between rounds.
        for cadence in [2., 4., 9., 20.] {
            let mut tracker = Tracker::default();
            let mut dark = 0;
            let mut tick = 0.;
            for n in 0..20 {
                let at = f64::from(n) * cadence;
                tracker.fire(7, 0, at, &mount);
                while tick < at + cadence {
                    if n > 0 && tracker.draw(tick, lookup).flashes.is_empty() {
                        dark += 1;
                    }
                    tick += 0.5;
                }
            }
            assert_eq!(dark, 0, "cadence {cadence}");
            // One flash per gun however many shots overlap.
            assert_eq!(tracker.draw(tick - 1., lookup).flashes.len(), 1);
        }
        // A 40 mm round a second apart is a distinct pop each time.
        let mut tracker = Tracker::default();
        tracker.fire(7, 1, 0., &mount);
        tracker.fire(7, 1, 72., &mount);
        assert!(tracker.draw(40., lookup).flashes.is_empty());
        assert_eq!(tracker.draw(72., lookup).flashes.len(), 1);
    }

    #[test]
    fn every_round_the_simulation_fires_is_one_flash_of_its_gun() {
        use tore_sim::combat::live::{Command, Event};
        use tore_world::{
            mission::{MissionSpec, Start},
            seats::{SeatCommand, SeatInput},
            test_support::resources::{THEATER, gunship_resources},
            world::{Seating, TickOutput, World},
        };
        let mut spec = MissionSpec::new(THEATER, AircraftId::Ac130);
        spec.start = Start::Airborne { altitude_ft: 5_000 };
        let mut world = World::new(&spec, &gunship_resources(), Seating::SinglePlayer).unwrap();
        let mut out = TickOutput::default();
        let mut tracker = Tracker::default();
        let mut fired = [0_usize; 3];
        let mut flashed = [0_usize; 3];
        let mut flashes = 0;
        for tick in 0..900 {
            world
                .step(
                    &[SeatInput {
                        tick,
                        trigger: (120..720).contains(&tick),
                        // Link the 40 mm and the 105 mm to the 25 mm.
                        commands: match tick {
                            5 | 7 => vec![SeatCommand::Combat(Command::NextGunGroup)],
                            6 | 8 => vec![SeatCommand::Combat(Command::ToggleGunGroup)],
                            _ => Vec::new(),
                        },
                        ..SeatInput::default()
                    }],
                    &mut out,
                )
                .unwrap();
            let config = world.combat.state.own().configuration();
            for event in &out.events {
                if let Event::Fired { station, .. } = event
                    && let Some(slot) = slot(&config.stations[*station].weapon.source)
                {
                    fired[slot] += 1;
                }
            }
            let picture = world.combat.render_snapshot().clone();
            let state = world.cockpits[0].flight.clone();
            let before = tracker.shots().count();
            let now = tick as f64;
            tracker.observe(&picture, now, mounts(&picture, Some((0, &state))));
            for shot in tracker.shots().skip(before) {
                flashed[shot.slot] += 1;
            }
            flashes += tracker
                .draw(now, mounts(&picture, Some((0, &state))))
                .flashes
                .len();
        }
        assert!(
            fired.iter().all(|n| *n > 0),
            "the linked guns fired: {fired:?}"
        );
        assert_eq!(flashed, fired);
        assert!(flashes > 0);
    }

    #[test]
    fn instances_pack_ten_words_each() {
        let flash = Flash {
            position: [1., 2., 3.],
            direction: [0., 0., 1.],
            length: 20.,
            intensity: 0.5,
            seed: 9,
            kind: 2,
        };
        let bytes = instances(&[flash; 2]);
        assert_eq!(bytes.len(), 2 * FLASH_BYTES);
        assert_eq!(&bytes[36..40], &2_u32.to_le_bytes());
        assert_eq!(
            instances(&vec![flash; MAX_FLASHES + 5]).len(),
            MAX_FLASHES * FLASH_BYTES
        );
    }
}
