//! Fitted point-mass bomb flight, shared by simulation and prediction.
//! Constants and limitations are specified in docs/spec/air-to-ground.md.
use crate::attitude::{Vector, dot, unit};
use tore_formats::weapons::Weapon;

pub const DT: f64 = 1. / 120.;
pub const MAX_SECONDS: f64 = 300.;

/// Aerodynamic parameters in feet, seconds and slugs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Body {
    pub mass: f64,
    pub area: f64,
    pub retarder: f64,
    /// 0 unguided, 1 bang-bang, 2 proportional.
    pub steering: u8,
    pub guidance_seconds: f64,
}
impl Default for Body {
    fn default() -> Self {
        Self {
            mass: 500. / 32.,
            area: 0.1323,
            retarder: 0.,
            steering: 0,
            guidance_seconds: 0.,
        }
    }
}
impl Body {
    pub fn of(w: &Weapon) -> Self {
        let weight = (f64::from(w.weight) / super::ballistics::release_count(w) as f64).max(50.);
        let guided = super::guided::is_glide_bomb(w);
        let retarded = super::ballistics::is_bomb(w)
            && w.flags & 0x800000 == 0
            && w.movement.launch_retard < 100;
        Self {
            mass: weight / 32.,
            area: 0.1323 * (weight / 500.).powf(2. / 3.) * if guided { 1.3 } else { 1. },
            retarder: if w.source == "MK82AIR.JT" {
                5.5
            } else if retarded {
                2.4
            } else {
                0.
            },
            steering: if !guided {
                0
            } else if matches!(w.source.as_str(), "GBU10.JT" | "GBU10A.JT" | "PAVEWAY.JT") {
                1
            } else {
                2
            },
            guidance_seconds: f64::from(w.movement.remove_t) / 4.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Flight {
    pub position: Vector,
    pub velocity: Vector,
}
impl Flight {
    fn acceleration(self, body: Body, seconds: f64, target: Option<Vector>) -> Vector {
        let speed = dot(self.velocity, self.velocity).sqrt();
        let forward = unit(self.velocity);
        let density = 0.002377 * (-self.position[1].max(0.) / 30_000.).exp();
        let q = 0.5 * density * speed * speed;
        let open = ((seconds - 0.25) / 0.5).clamp(0., 1.);
        let mut drag = q * (body.area + body.retarder * open) / body.mass;
        let mut lift = [0.; 3];
        if body.steering != 0
            && seconds < body.guidance_seconds
            && let Some(target) = target
        {
            let gap: Vector = std::array::from_fn(|i| target[i] - self.position[i]);
            let distance = dot(gap, gap).sqrt();
            // Velocity pursuit with gravity compensation. Acceleration is
            // normal to velocity, bounded by dynamic pressure and four g.
            let desired = unit(gap);
            let along = dot(desired, forward);
            let time = (distance / speed.max(1.)).max(0.25);
            let command: Vector = std::array::from_fn(|i| {
                4. * speed / time * (desired[i] - along * forward[i])
                    + if i == 1 { 32. } else { 0. }
            });
            let parallel = dot(command, forward);
            let normal: Vector = std::array::from_fn(|i| command[i] - parallel * forward[i]);
            let demand = dot(normal, normal).sqrt();
            let limit = (q * body.area * 15. / body.mass).min(128.);
            let amount = if body.steering == 1 && demand > 2. {
                limit
            } else {
                demand.min(limit)
            };
            if demand > 1e-9 {
                lift = normal.map(|a| a * amount / demand);
                drag += amount * amount / (q / body.mass * 12.).max(1.)
                    * if body.steering == 1 { 0.15 } else { 0.08 };
            }
        }
        std::array::from_fn(|i| lift[i] - forward[i] * drag - if i == 1 { 32. } else { 0. })
    }

    /// Explicit midpoint integration on the common 120 Hz grid.
    pub fn step(&mut self, body: Body, tick: u64, target: Option<Vector>) {
        let t = tick as f64 * DT;
        let a = self.acceleration(body, t, target);
        let mid = Self {
            position: std::array::from_fn(|i| self.position[i] + self.velocity[i] * DT * 0.5),
            velocity: std::array::from_fn(|i| self.velocity[i] + a[i] * DT * 0.5),
        };
        let a = mid.acceleration(body, t + DT * 0.5, target);
        for (i, acceleration) in a.into_iter().enumerate() {
            self.position[i] += mid.velocity[i] * DT;
            self.velocity[i] += acceleration * DT;
        }
    }
}
