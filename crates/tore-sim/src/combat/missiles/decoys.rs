//! Physical countermeasure targets, independent of presentation pool limits.
//! SI units. One cartridge is one logical target and one episode contribution.
use super::energy::{DT, G, Observation};
use crate::attitude::{Vector, dot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Flare,
    Chaff,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Device {
    pub id: u64,
    pub owner: u32,
    pub kind: Kind,
    pub episode: u64,
    pub position: Vector,
    pub previous: Vector,
    pub velocity: Vector,
    pub age: u32,
    pub alive: bool,
    pub effectiveness: u8,
}
impl Device {
    pub fn release(
        id: u64,
        owner: u32,
        kind: Kind,
        episode: u64,
        position: Vector,
        velocity: Vector,
    ) -> Self {
        Self {
            id,
            owner,
            kind,
            episode,
            position,
            previous: position,
            velocity,
            age: 0,
            alive: true,
            effectiveness: 35,
        }
    }
    pub fn observation(self) -> Observation {
        Observation {
            position: self.position,
            velocity: self.velocity,
        }
    }
    pub fn lifetime(self) -> u32 {
        match self.kind {
            Kind::Flare => 3600,
            Kind::Chaff => 2400,
        }
    }
    pub fn strength(self) -> f64 {
        if !self.alive {
            return 0.;
        }
        let remaining = self.lifetime().saturating_sub(self.age);
        match self.kind {
            Kind::Flare => (f64::from(remaining) / 360.).min(1.),
            Kind::Chaff => (f64::from(remaining) / 600.).min(1.),
        }
    }
    pub fn step(&mut self, wind: Vector, ground: &impl Fn(f64, f64) -> f64) {
        if !self.alive {
            return;
        }
        self.previous = self.position;
        let air: Vector = std::array::from_fn(|i| self.velocity[i] - wind[i]);
        let damping = match self.kind {
            Kind::Flare => 1. / (1. + G / (30.48 * 30.48) * dot(air, air).sqrt() * DT),
            Kind::Chaff => (-DT / 0.12).exp(),
        };
        self.velocity = std::array::from_fn(|i| wind[i] + air[i] * damping);
        match self.kind {
            Kind::Flare => self.velocity[1] -= G * DT,
            Kind::Chaff => self.velocity[1] -= 1.2192 * (1. - damping),
        }
        for i in 0..3 {
            self.position[i] += self.velocity[i] * DT;
        }
        self.age += 1;
        // Physical targets retire on ground contact. Presentation may leave
        // a burning flare resting at its existing grounded visual position.
        self.alive = self.age < self.lifetime()
            && super::super::live::terrain_hit(self.previous, self.position, ground).is_none();
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pursuit {
    pub device: Device,
    pub passed: bool,
}
impl Pursuit {
    /// Relative swept closest approach detects passage even when a fast
    /// missile crosses the entire device position between fixed ticks.
    pub fn after_motion(&mut self, previous: Vector, position: Vector) {
        if self.passed {
            return;
        }
        let before: Vector = std::array::from_fn(|i| self.device.previous[i] - previous[i]);
        let after: Vector = std::array::from_fn(|i| self.device.position[i] - position[i]);
        let delta: Vector = std::array::from_fn(|i| after[i] - before[i]);
        // The relative range has passed its minimum during this step.
        self.passed = !self.device.alive || (dot(before, delta) < 0. && dot(after, delta) >= 0.);
    }
    pub fn target(self) -> Option<Observation> {
        (!self.passed && self.device.alive).then(|| self.device.observation())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Episode {
    pub owner: u32,
    pub kind: Kind,
    pub start_tick: u64,
    pub id: u64,
}
impl Episode {
    pub fn includes(self, owner: u32, kind: Kind, tick: u64) -> bool {
        self.owner == owner
            && self.kind == kind
            && tick >= self.start_tick
            && tick - self.start_tick < 240
    }
}

/// First-second seduction targets; captured devices live in missile state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scene {
    pub devices: Vec<Device>,
    pub episodes: Vec<Episode>,
}
impl Scene {
    pub fn release(&mut self, mut device: Device, tick: u64) {
        if let Some(episode) = self
            .episodes
            .iter()
            .find(|e| e.includes(device.owner, device.kind, tick))
        {
            device.episode = episode.id;
        } else {
            self.episodes
                .retain(|e| tick.saturating_sub(e.start_tick) < 240);
            self.episodes.push(Episode {
                owner: device.owner,
                kind: device.kind,
                start_tick: tick,
                id: device.id,
            });
            device.episode = device.id;
        }
        if self.devices.len() == 1024 {
            self.devices.remove(0);
        }
        self.devices.push(device);
    }
    pub fn effectiveness(&mut self, owner: u32, kind: Kind, value: u8) {
        if let Some(device) = self
            .devices
            .iter_mut()
            .rev()
            .find(|d| d.owner == owner && d.kind == kind)
        {
            device.effectiveness = value;
        }
    }
    pub fn step(&mut self, wind: Vector, ground: &impl Fn(f64, f64) -> f64) {
        for device in &mut self.devices {
            device.step(wind, ground);
        }
        self.devices
            .retain(|device| device.alive && device.age < 120);
    }
}

/// A missile remembers one episode for its owned target and seeker class.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Memory {
    pub last_episode: Option<u64>,
    pub pursuit: Option<Pursuit>,
}
impl Memory {
    pub fn eligible(&self, episode: u64) -> bool {
        self.pursuit.is_none() && self.last_episode != Some(episode)
    }
    pub fn evaluate(
        &mut self,
        device: Device,
        probability: f64,
        seed: u64,
        missile: u32,
    ) -> Option<bool> {
        if !self.eligible(device.episode) {
            return None;
        }
        self.last_episode = Some(device.episode);
        let draw = mix(seed ^ mix(u64::from(missile)) ^ mix(device.episode)) >> 11;
        let decoyed = (draw as f64) / ((1u64 << 53) as f64) < probability.clamp(0., 1.);
        if decoyed {
            self.pursuit = Some(Pursuit {
                device,
                passed: false,
            });
        }
        Some(decoyed)
    }
}

pub fn mix(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d049bb133111eb);
    value ^ (value >> 31)
}

/// Ratios are received signal ratios, after range, visibility and seeker FOV.
pub fn infrared_probability(
    susceptibility: u8,
    effectiveness: u8,
    ratio: f64,
    generation: f64,
    imaging: bool,
) -> f64 {
    let p = f64::from(susceptibility) * f64::from(effectiveness) / 10000.
        * generation
        * ratio.clamp(0., 3.);
    p.clamp(0., if imaging { 0.10 } else { 0.95 })
}
pub fn radar_probability(
    susceptibility: u8,
    effectiveness: u8,
    notch: f64,
    generation: f64,
) -> f64 {
    (f64::from(susceptibility) * f64::from(effectiveness) / 10000.
        * generation
        * (0.25 + 2.75 * notch.clamp(0., 1.)))
    .clamp(0., 0.95)
}

/// Terrain Doppler separation uses target ground velocity, not closing speed.
/// `look_down` is the sensor's independently established clutter strength.
pub fn notch(target_velocity: Vector, line: Vector, look_down: f64) -> f64 {
    let radial = dot(target_velocity, line).abs();
    (1. - radial / (60. * 0.3048)).clamp(0., 1.) * look_down.clamp(0., 1.)
}

crate::checkpoint_enum!(Kind {Flare=0,Chaff=1});
crate::checkpoint_struct!(Device {
    id,
    owner,
    kind,
    episode,
    position,
    previous,
    velocity,
    age,
    alive,
    effectiveness
});
crate::checkpoint_struct!(Scene { devices, episodes });
crate::checkpoint_struct!(Pursuit { device, passed });
crate::checkpoint_struct!(Episode {
    owner,
    kind,
    start_tick,
    id
});
crate::checkpoint_struct!(Memory {
    last_episode,
    pursuit
});

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn episode_does_not_compound_and_preserves_large_susceptibilities() {
        let device = Device::release(1, 2, Kind::Flare, 10, [0., 1000., 0.], [0., 0., 200.]);
        let mut memory = Memory::default();
        assert_eq!(memory.evaluate(device, 0., 123, 7), Some(false));
        assert_eq!(
            memory.evaluate(Device { id: 2, ..device }, 1., 123, 7),
            None
        );
        assert_eq!(
            memory.evaluate(
                Device {
                    id: 3,
                    episode: 11,
                    ..device
                },
                1.,
                123,
                7
            ),
            Some(true)
        );
        assert_eq!(
            memory.evaluate(
                Device {
                    id: 4,
                    episode: 12,
                    ..device
                },
                1.,
                123,
                7
            ),
            None
        );
        assert_eq!(infrared_probability(200, 35, 1., 1., false), 0.7);
        assert!((infrared_probability(150, 35, 1., 1., false) - 0.525).abs() < 1e-12);
        let e = Episode {
            owner: 2,
            kind: Kind::Flare,
            start_tick: 10,
            id: 1,
        };
        assert!(e.includes(2, Kind::Flare, 249));
        assert!(!e.includes(2, Kind::Flare, 250));
    }
    #[test]
    fn weaker_target_signal_increases_flare_odds_and_imaging_resists() {
        let tail = infrared_probability(50, 35, 1., 0.6, false);
        let beam = infrared_probability(50, 35, 1. / 0.7, 0.6, false);
        let cold = infrared_probability(50, 35, 1. / 0.07, 0.6, false);
        assert!(tail < beam && beam < cold);
        assert!(infrared_probability(50, 35, 1. / 0.07, 0.08, true) < tail);
    }
    #[test]
    fn radar_notch_is_relative_to_stationary_clutter() {
        assert_eq!(notch([250., 0., 0.], [0., 0., 1.], 1.), 1.);
        assert_eq!(notch([250., 0., 0.], [0., 0., 1.], 0.), 0.);
        assert_eq!(notch([0., 0., 250.], [0., 0., 1.], 1.), 0.);
        assert!(radar_probability(100, 35, 1., 0.35) > radar_probability(100, 35, 0., 0.35));
    }
    #[test]
    fn devices_move_physically_and_passing_ends_pursuit_without_a_device_fuze() {
        let mut d = Device::release(1, 2, Kind::Chaff, 1, [0., 1000., 0.], [0., 0., 250.]);
        for _ in 0..120 {
            d.step([10., 0., 0.], &|_, _| 0.);
        }
        assert!(d.velocity[2] < 0.1 && d.velocity[0] > 9.9 && d.velocity[1] < 0.);
        let mut p = Pursuit {
            device: d,
            passed: false,
        };
        let before = [d.position[0], d.position[1], d.position[2] - 5.];
        let after = [d.position[0], d.position[1], d.position[2] + 5.];
        p.after_motion(before, after);
        assert!(p.passed && p.target().is_none());
        // Fuzes are deliberately not evaluated here. Live aircraft contact
        // remains authoritative even while this physical decoy is pursued.
        let copy =
            crate::checkpoint::round_trip(&p, &crate::checkpoint::Models::default()).unwrap();
        assert_eq!(copy, p);
    }
}
