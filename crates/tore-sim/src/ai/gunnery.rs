//! Authored AI gun employment, from docs/spec/ai-gun-employment.md.
//! Uses the player's trajectory solver and live collision volume. No hidden
//! target lookup, radar requirement, projectile homing or pose override.
use super::weapon_service::{Phase, StationId};
use crate::{
    attitude::{Basis, Vector, dot, unit},
    combat::{gunsight, live},
};
use tore_formats::weapons::Weapon;

pub const BURST_TICKS: u64 = 60;
pub const RECOVERY_TICKS: u64 = 60;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Target {
    pub id: u32,
    pub position: Vector,
    pub velocity: Vector,
    pub basis: Basis,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aim {
    pub direction: Vector,
    pub heading_rate: f64,
    pub pitch_rate: f64,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Solution {
    pub aim: Aim,
    pub aligned: bool,
    pub miss_ft: f64,
    pub seconds: f64,
    pub range_ft: f64,
}

pub fn solve(
    weapon: &Weapon,
    own: &live::Launcher,
    mount: Vector,
    target: Target,
) -> Option<Solution> {
    let pipper = gunsight::solve_observed(
        weapon,
        own,
        mount,
        Some(gunsight::TargetObservation {
            position: target.position,
            velocity: target.velocity,
        }),
    )
    .ok()??;
    let muzzle: Vector = std::array::from_fn(|i| {
        own.position[i]
            + own.basis.right[i] * mount[0]
            + own.basis.up[i] * mount[1]
            + own.basis.forward[i] * mount[2]
    });
    let delta: Vector = std::array::from_fn(|i| {
        target.position[i]
            + target.velocity[i] * pipper.seconds
            + if i == 1 { pipper.drop_ft } else { 0. }
            - muzzle[i]
    });
    let direction = unit(delta);
    if !direction.iter().all(|v| v.is_finite()) {
        return None;
    }
    let error: Vector = std::array::from_fn(|i| pipper.point[i] - target.position[i]);
    let relative: Vector = std::array::from_fn(|i| {
        own.basis.forward[i] * pipper.travel_ft / pipper.seconds.max(1e-6) - target.velocity[i]
    });
    let length = dot(relative, relative).sqrt();
    let along = relative.map(|v| v / length.max(1.));
    let half = live::AIRCRAFT_RADIUS_FT * 2.;
    let aligned = live::aircraft_contact(
        std::array::from_fn(|i| error[i] - along[i] * half),
        std::array::from_fn(|i| error[i] + along[i] * half),
        [0.; 3],
        target.basis,
        live::AIRCRAFT_RADIUS_FT,
    )
    .is_some();
    let relative_velocity: Vector = std::array::from_fn(|i| target.velocity[i] - own.velocity[i]);
    // Differentiate the same interception equation, including the change in
    // lead time and gravity compensation as range opens or closes.
    let arrival_velocity: Vector = std::array::from_fn(|i| {
        target.velocity[i] + if i == 1 { pipper.fall_speed_fps } else { 0. }
    });
    let denominator = pipper.travel_ft * pipper.flight_speed_fps - dot(delta, arrival_velocity);
    let time_rate = if denominator.abs() > 1e-6 {
        dot(delta, relative_velocity) / denominator
    } else {
        0.
    };
    let velocity: Vector =
        std::array::from_fn(|i| relative_velocity[i] + arrival_velocity[i] * time_rate);
    let horizontal = delta[0].hypot(delta[2]).max(1.);
    let distance_sq = dot(delta, delta).max(1.);
    Some(Solution {
        aim: Aim {
            direction,
            heading_rate: (delta[2] * velocity[0] - delta[0] * velocity[2])
                / (horizontal * horizontal),
            pitch_rate: (horizontal * velocity[1]
                - delta[1] * (delta[0] * velocity[0] + delta[2] * velocity[2]) / horizontal)
                / distance_sq,
        },
        aligned,
        miss_ft: dot(error, error).sqrt(),
        seconds: pipper.seconds,
        range_ft: pipper.range_ft,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct View {
    pub station: StationId,
    pub target: Option<u32>,
    pub solution: Option<Solution>,
    /// Physical rounds per source period and that period in 120 Hz ticks.
    pub rounds: u64,
    pub period_ticks: u64,
    pub target_speed: f64,
    pub ammunition: super::weapon_service::Rounds,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    pub view: View,
    pub phase: Phase,
    pub deadline: Option<u64>,
    pub requested_round: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cycle {
    burst: Option<(u32, u64)>,
    recovery_until: u64,
    next_scaled: u64,
}

impl Cycle {
    pub fn advance(
        &mut self,
        tick: u64,
        target: Option<u32>,
        ready: bool,
        rounds: u64,
        period: u64,
    ) -> bool {
        let ready = ready && target.is_some();
        if let Some((id, start)) = self.burst
            && (!ready || target != Some(id) || tick >= start + BURST_TICKS)
        {
            self.burst = None;
            self.recovery_until = tick + RECOVERY_TICKS;
        }
        if !ready || tick < self.recovery_until {
            return false;
        }
        let scale = rounds.max(1);
        let now = tick.saturating_mul(scale);
        if self.burst.is_none() {
            self.burst = target.map(|id| (id, tick));
            self.next_scaled = self.next_scaled.max(now);
        }
        if now < self.next_scaled {
            return false;
        }
        // Keep the fractional cadence through normal shots; never bank shots
        // during a pause/recovery and never bypass a future deadline.
        if self.next_scaled + period.max(1) <= now {
            self.next_scaled = now;
        }
        self.next_scaled = self.next_scaled.saturating_add(period.max(1));
        true
    }
    pub fn phase(&self, tick: u64) -> Phase {
        if self.burst.is_some() {
            Phase::Fire
        } else if tick < self.recovery_until {
            Phase::Reload
        } else {
            Phase::Tracking
        }
    }
    pub fn deadline(&self, tick: u64) -> Option<u64> {
        self.burst
            .map(|(_, start)| start + BURST_TICKS)
            .or((tick < self.recovery_until).then_some(self.recovery_until))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lead_rate_tracks_changing_intercept_time_and_gravity() {
        let weapon = gunsight::tests::weapon();
        let own = live::Launcher {
            position: [0., 1000., 0.],
            basis: Basis::new(0., 0., 0.),
            speed_fps: 300.,
            velocity: [0., 0., 300.],
            radar: false,
            radar_power: false,
            alive: true,
            body_present: true,
            bay_ready: true,
            jammer: false,
            controls: crate::sensors::Controls::default(),
        };
        for velocity in [[0., 0., -200.], [0., 0., 200.], [250., 0., 0.]] {
            let target = Target {
                id: 1,
                position: [0., 1000., 800.],
                velocity,
                basis: own.basis,
            };
            let initial = solve(&weapon, &own, [0.; 3], target).unwrap();
            let dt = 0.05;
            let later = solve(
                &weapon,
                &live::Launcher {
                    position: std::array::from_fn(|i| own.position[i] + own.velocity[i] * dt),
                    ..own
                },
                [0.; 3],
                Target {
                    position: std::array::from_fn(|i| target.position[i] + target.velocity[i] * dt),
                    ..target
                },
            )
            .unwrap();
            let a = initial.aim.direction;
            let b = later.aim.direction;
            let heading_rate = (b[0].atan2(b[2]) - a[0].atan2(a[2])) / dt;
            let pitch_rate = (b[1].asin() - a[1].asin()) / dt;
            assert!(
                (initial.aim.heading_rate - heading_rate).abs() < 0.025,
                "{velocity:?}"
            );
            assert!(
                (initial.aim.pitch_rate - pitch_rate).abs() < 0.025,
                "{velocity:?}"
            );
        }
    }
    #[test]
    fn complete_bursts_have_sixteen_spaced_rounds_at_any_start_phase() {
        for start in 0..30 {
            let mut cycle = Cycle::default();
            let shots: Vec<_> = (start..start + 240)
                .filter(|t| cycle.advance(*t, Some(3), true, 8, 30))
                .collect();
            assert_eq!(shots.len(), 32, "start {start}: {shots:?}");
            assert_eq!(shots.iter().filter(|t| **t < start + 60).count(), 16);
            assert_eq!(shots.iter().filter(|t| **t >= start + 120).count(), 16);
            assert!(shots.windows(2).all(|w| w[1] - w[0] >= 3));
        }
    }
    #[test]
    fn interruption_retarget_and_loss_cannot_bypass_recovery() {
        let mut cycle = Cycle::default();
        assert!(cycle.advance(0, Some(1), true, 8, 30));
        assert!(!cycle.advance(0, Some(1), true, 8, 30));
        assert!(!cycle.advance(1, Some(1), false, 8, 30));
        for tick in 2..61 {
            assert!(!cycle.advance(tick, Some(2), true, 8, 30));
        }
        assert!(cycle.advance(61, Some(2), true, 8, 30));
        assert!(!cycle.advance(62, None, false, 8, 30));
        assert!(!cycle.advance(63, Some(2), true, 8, 30));
    }
}
