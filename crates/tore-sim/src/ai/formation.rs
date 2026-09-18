//! Opinionated departure/rejoin guidance. All outputs are requests to the
//! physical input controller. Constants and limitations live in docs/spec/ai.md.
use super::controller::{LeaderView, OwnState};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Phase {
    #[default]
    Close,
    Trail,
    Breakout,
    Intercept,
    Stabilize,
    Capture,
}

/// Same-tick traffic observation; supplied by the mission, never read from
/// another actor after that actor has advanced its physics.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Traffic {
    pub id: u32,
    pub position: [f64; 3],
    pub velocity: [f64; 3],
    pub phase: Option<Phase>,
}

/// Hidden inspection hook. Hosts may record this without any normal-flight UI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Trace {
    pub phase: Phase,
    pub phase_seconds: f64,
    pub slot_distance_ft: f64,
    pub closure_fps: f64,
    pub altitude_error_ft: f64,
    pub minimum_predicted_separation_ft: f64,
    pub yielding_to: Option<u32>,
    pub aim: [f64; 3],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Guidance {
    phase: Phase,
    phase_seconds: f64,
    side: f64,
    capture: f64,
    last_heading: Option<f64>,
    pub trace: Option<Trace>,
}

pub struct Request {
    pub aim: [f64; 3],
    pub speed: f64,
    pub close: bool,
    pub burner: bool,
}

pub fn velocity(heading: f64, pitch: f64, speed: f64) -> [f64; 3] {
    let (h, p) = (heading.to_radians(), pitch.to_radians());
    [
        h.sin() * p.cos() * speed,
        p.sin() * speed,
        h.cos() * p.cos() * speed,
    ]
}
fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] + b[i])
}
fn sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn scale(a: [f64; 3], s: f64) -> [f64; 3] {
    a.map(|x| x * s)
}
fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|i| a[i] * b[i]).sum()
}
fn length(a: [f64; 3]) -> f64 {
    dot(a, a).sqrt()
}
fn unit(a: [f64; 3]) -> [f64; 3] {
    scale(a, 1.0 / length(a).max(1.0))
}
fn angle(a: f64, b: f64) -> f64 {
    (a - b + 180.).rem_euclid(360.) - 180.
}

/// Constant-velocity closest approach over a bounded lookahead. This is a
/// prediction for deciding controls, not a promise of achieved separation.
fn separation(p: [f64; 3], v: [f64; 3], other: &Traffic, horizon: f64) -> f64 {
    let r = sub(other.position, p);
    let relative = sub(other.velocity, v);
    let t = (-dot(r, relative) / dot(relative, relative).max(1.)).clamp(0., horizon);
    length(add(r, scale(relative, t)))
}

impl Guidance {
    fn transition(&mut self, next: Phase) {
        if next != self.phase {
            self.phase = next;
            self.phase_seconds = 0.;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn step(
        &mut self,
        id: u32,
        own: &OwnState,
        leader: LeaderView,
        leader_velocity: [f64; 3],
        slot: [f64; 3],
        traffic: &[Traffic],
        dt: f64,
    ) -> Request {
        self.phase_seconds += dt;
        let forward = velocity(leader.heading_deg, 0., 1.);
        let right = [forward[2], 0., -forward[0]];
        let own_velocity = velocity(own.heading_deg, own.flight_path_pitch_deg, own.speed.0);
        let error = sub(slot, own.position);
        let slot_distance = length(error);
        let closure = dot(sub(own_velocity, leader_velocity), unit(error));
        let alignment = angle(own.heading_deg, leader.heading_deg).abs();
        let turn_rate = self
            .last_heading
            .map_or(0., |h| angle(leader.heading_deg, h).abs() / dt);
        self.last_heading = Some(leader.heading_deg);
        let leader_pitch = leader_velocity[1]
            .atan2(leader_velocity[0].hypot(leader_velocity[2]))
            .to_degrees();
        let maneuvering = turn_rate > 4. || leader_pitch.abs() > 20. || alignment > 40.;
        let clearance = traffic
            .iter()
            .filter(|t| t.id != id)
            .map(|t| separation(own.position, own_velocity, t, 8.))
            .fold(f64::INFINITY, f64::min);
        if self.phase == Phase::Close {
            self.side = dot(sub(slot, leader.position), right).signum();
            if self.side == 0. {
                self.side = 1.;
            }
        }
        if clearance < 220. {
            self.capture = 0.;
            self.transition(Phase::Breakout);
        } else if self.phase == Phase::Breakout {
            if clearance > 500. && self.phase_seconds > 2. {
                self.transition(Phase::Intercept);
            }
        } else if self.phase == Phase::Close && (maneuvering || slot_distance > 1800.) {
            self.transition(Phase::Trail);
            self.capture = 0.;
            // Select the occupied side, not a mandatory turn toward the assigned slot.
            let lateral = dot(sub(own.position, leader.position), right);
            self.side = if lateral.abs() > 100. {
                lateral.signum()
            } else {
                dot(sub(slot, leader.position), right).signum()
            };
        }
        if self.side == 0. {
            self.side = 1.;
        }
        let slot_offset = sub(slot, leader.position);
        let lateral = dot(slot_offset, right).abs().max(256.) + 256.;
        let aft = (-dot(slot_offset, forward) + 1200.).max(1800.);
        let gate = add(
            add(leader.position, scale(right, self.side * lateral)),
            scale(forward, -aft),
        );
        let gate_error = sub(gate, own.position);
        let gate_distance = length(gate_error);
        // Rank only arrivals sharing an approach corridor. Nearer aircraft go
        // first, with stable IDs breaking exact ties. Opposite sides may join together.
        let yielding_to = traffic
            .iter()
            .filter(|t| {
                self.phase != Phase::Capture
                    && t.id != id
                    && t.phase
                        .is_some_and(|p| p != Phase::Close && p != Phase::Trail)
            })
            .filter(|t| dot(sub(t.position, gate), right).abs() < 450.)
            .filter(|t| {
                let d = length(sub(t.position, gate));
                let priority = |phase| match phase {
                    Phase::Capture => 0,
                    Phase::Stabilize => 1,
                    _ => 2,
                };
                let theirs = priority(t.phase.unwrap_or(Phase::Close));
                let ours = priority(self.phase);
                d < 1800.
                    && (theirs < ours
                        || (theirs == ours
                            && (d + 100. < gate_distance
                                || ((d - gate_distance).abs() <= 100. && t.id < id))))
            })
            .map(|t| t.id)
            .min();
        if self.phase == Phase::Trail && !maneuvering {
            self.transition(Phase::Intercept);
        }
        if self.phase == Phase::Intercept
            && gate_distance < 450.
            && alignment < 15.
            && length(sub(own_velocity, leader_velocity)) < 70.
            && yielding_to.is_none()
        {
            self.transition(Phase::Stabilize);
        }
        if self.phase == Phase::Stabilize
            && gate_distance < 450.
            && alignment < 10.
            && length(sub(own_velocity, leader_velocity)) < 40.
            && yielding_to.is_none()
        {
            self.capture = 0.;
            self.transition(Phase::Capture);
        }
        if matches!(self.phase, Phase::Capture | Phase::Stabilize)
            && (maneuvering || (slot_distance < 1200. && closure > 120.) || yielding_to.is_some())
        {
            self.capture = 0.;
            self.transition(Phase::Intercept);
        }
        if self.phase == Phase::Capture {
            if alignment < 15. && length(sub(own_velocity, leader_velocity)) < 70. {
                self.capture = (self.capture + dt / 12.).min(1.);
            }
            if self.capture == 1. && slot_distance < 200. && closure.abs() < 40. {
                self.transition(Phase::Close);
            }
        }
        let target = match self.phase {
            Phase::Close => slot,
            Phase::Capture => add(gate, scale(sub(slot, gate), self.capture)),
            _ => {
                if yielding_to.is_some() {
                    add(gate, scale(forward, -1800.))
                } else {
                    gate
                }
            }
        };
        let target_error = sub(target, own.position);
        let distance = length(target_error);
        let mut aim = add(target, scale(leader_velocity, 3.));
        let close = self.phase == Phase::Close;
        let mut speed = if close {
            leader.speed.0 + (dot(error, forward) / 6.).clamp(-100., 100.)
        } else {
            // Follow a velocity vector toward a moving gate. Closure decreases
            // with available braking distance and with turn misalignment.
            let closing = (2. * 12. * distance).sqrt().min(distance / 6.).min(300.);
            let desired = add(leader_velocity, scale(unit(target_error), closing));
            aim = add(own.position, scale(desired, 3.));
            if alignment > 60. {
                own.limits.corner.0.min(leader.speed.0)
            } else {
                length(desired)
            }
        };
        if self.phase == Phase::Breakout {
            // Compare escape corridors against every observed aircraft. A small
            // side preference cannot outweigh clearance; never dive to escape.
            let mut best = f64::NEG_INFINITY;
            for offset in [0., -30., 30., -60., 60., -90., 90.] {
                for pitch in [own.flight_path_pitch_deg.max(0.), 15.] {
                    let candidate = velocity(own.heading_deg + offset, pitch, own.speed.0);
                    // Blend current velocity to account for roll/turn response.
                    let predicted = add(scale(own_velocity, 0.5), scale(candidate, 0.5));
                    let margin = traffic
                        .iter()
                        .filter(|t| t.id != id)
                        .map(|t| separation(own.position, predicted, t, 8.))
                        .fold(10000., f64::min);
                    let score = margin - offset.abs() * 0.2 + offset * self.side * 0.05;
                    if score > best {
                        best = score;
                        aim = add(own.position, scale(candidate, 5.));
                    }
                }
            }
            speed = own.speed.0.max(own.limits.corner.0);
        }
        speed = speed.clamp(own.limits.minimum.0, own.limits.maximum.0);
        // Screen the requested approach as well as the current trajectory.
        // This starts a shallow detour before physical convergence requires a
        // breakout, including around members that have already rejoined.
        if !matches!(self.phase, Phase::Close | Phase::Breakout) {
            let direction = sub(aim, own.position);
            let heading = direction[0].atan2(direction[2]).to_degrees();
            let pitch = direction[1]
                .atan2(direction[0].hypot(direction[2]))
                .to_degrees();
            let mut best = f64::NEG_INFINITY;
            for offset in [0., -15., 15., -30., 30., -45., 45.] {
                let candidate = velocity(heading + offset, pitch, speed);
                let predicted = add(scale(own_velocity, 0.5), scale(candidate, 0.5));
                let margin = traffic
                    .iter()
                    .filter(|t| t.id != id)
                    .map(|t| separation(own.position, predicted, t, 10.))
                    .fold(500., f64::min);
                let score = margin - offset.abs() * 2.;
                if score > best {
                    best = score;
                    aim = add(own.position, scale(candidate, 3.));
                }
            }
        }
        // Do not chase a close point behind the nose at high speed.
        let burner = self.phase == Phase::Intercept
            && alignment < 20.
            && distance > 6000.
            && speed > own.speed.0 + 100.
            && own.fuel_endurance_s > 180.;
        self.trace = Some(Trace {
            phase: self.phase,
            phase_seconds: self.phase_seconds,
            slot_distance_ft: slot_distance,
            closure_fps: closure,
            altitude_error_ft: error[1],
            minimum_predicted_separation_ft: clearance,
            yielding_to,
            aim,
        });
        Request {
            aim,
            speed: speed.clamp(own.limits.minimum.0, own.limits.maximum.0),
            close,
            burner,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::{ScalarSpeed, SpeedLimits};

    fn own(position: [f64; 3], speed: f64) -> OwnState {
        OwnState {
            position,
            heading_deg: 0.,
            flight_path_pitch_deg: 0.,
            body_pitch_offset_deg: 0.,
            bank_deg: 0.,
            speed: ScalarSpeed(speed),
            limits: SpeedLimits {
                minimum: ScalarSpeed(300.),
                corner: ScalarSpeed(700.),
                maximum: ScalarSpeed(1800.),
            },
            altitude_msl_ft: position[1],
            agl_ft: position[1],
            terrain_ahead_ft: 0.,
            minimum_altitude_ft: 300.,
            at_ceiling: false,
            on_ground: false,
            g_limit: 7.,
            roll_limit_deg_per_s: 90.,
            maximum_bank_deg: 85.,
            alive: true,
            fuel_endurance_s: 1000.,
            time_home_s: None,
            internal_fuel_lbs: 1000.,
            radar_emitting: false,
        }
    }
    fn leader() -> LeaderView {
        LeaderView {
            position: [0., 20000., 0.],
            velocity: [0., 0., 800.],
            heading_deg: 0.,
            speed: ScalarSpeed(800.),
            target: None,
            recovering: false,
        }
    }

    #[test]
    fn approach_reservations_exclude_finished_and_opposite_side_aircraft() {
        let own = own([768., 20000., -2800.], 800.);
        let mut peer = Traffic {
            id: 2,
            position: [768., 20000., -1800.],
            velocity: [0., 0., 800.],
            phase: Some(Phase::Capture),
        };
        let inspect = |phase, peer| {
            let mut g = Guidance {
                phase,
                side: 1.,
                ..Guidance::default()
            };
            g.step(
                1,
                &own,
                leader(),
                leader().velocity,
                [512., 20000., -512.],
                &[peer],
                1. / 120.,
            );
            g.trace.unwrap().yielding_to
        };
        assert_eq!(inspect(Phase::Intercept, peer), Some(2));
        assert_eq!(
            inspect(Phase::Capture, peer),
            None,
            "capture retains its reservation; collision prediction still applies"
        );
        peer.position[0] = -768.;
        assert_eq!(inspect(Phase::Intercept, peer), None);
        peer.position[0] = 768.;
        peer.phase = Some(Phase::Close);
        assert_eq!(inspect(Phase::Intercept, peer), None);
    }

    #[test]
    fn requested_intercept_detours_before_current_motion_becomes_unsafe() {
        let own = own([768., 20000., -6000.], 800.);
        let traffic = [Traffic {
            id: 2,
            position: [768., 20000., -5000.],
            velocity: [0., 0., 800.],
            phase: Some(Phase::Close),
        }];
        let mut g = Guidance {
            phase: Phase::Intercept,
            side: 1.,
            ..Guidance::default()
        };
        let request = g.step(
            1,
            &own,
            leader(),
            leader().velocity,
            [512., 20000., -512.],
            &traffic,
            1. / 120.,
        );
        assert_eq!(g.trace.unwrap().phase, Phase::Intercept);
        assert!((request.aim[0] - own.position[0]).abs() > 100.);
    }

    #[test]
    fn excessive_closure_abandons_capture() {
        let mut guidance = Guidance {
            phase: Phase::Capture,
            side: 1.,
            ..Guidance::default()
        };
        let own = own([512., 20000., -1100.], 1000.);
        let result = guidance.step(
            1,
            &own,
            leader(),
            leader().velocity,
            [512., 20000., -512.],
            &[],
            1. / 120.,
        );
        assert_eq!(guidance.trace.unwrap().phase, Phase::Intercept);
        assert!(!result.burner);
        assert!(result.speed < own.speed.0);
        assert!(
            result.aim[2] > own.position[2],
            "reduce speed without turning back into traffic"
        );
    }

    #[test]
    fn neighboring_traffic_overrides_formation_side_preference() {
        let own = own([0., 20000., 0.], 800.);
        let traffic = [
            Traffic {
                id: 2,
                position: [0., 20000., 2500.],
                velocity: [0., 0., 400.],
                phase: None,
            },
            Traffic {
                id: 3,
                position: [500., 20000., 0.],
                velocity: [0., 0., 800.],
                phase: None,
            },
        ];
        let mut guidance = Guidance::default();
        let result = guidance.step(
            1,
            &own,
            leader(),
            leader().velocity,
            [512., 20000., -512.],
            &traffic,
            1. / 120.,
        );
        assert_eq!(guidance.trace.unwrap().phase, Phase::Breakout);
        assert!(
            result.aim[0] < own.position[0] || result.aim[1] > own.position[1] + 500.,
            "occupied right escape must not win just because the slot is right"
        );
        assert!(result.aim[1] >= own.position[1]);
    }

    #[test]
    fn matched_speed_is_required_before_inward_capture() {
        let mut guidance = Guidance {
            phase: Phase::Intercept,
            side: 1.,
            ..Guidance::default()
        };
        let mut own = own([768., 20000., -1800.], 880.);
        let slot = [512., 20000., -512.];
        guidance.step(1, &own, leader(), leader().velocity, slot, &[], 1. / 120.);
        assert_eq!(guidance.trace.unwrap().phase, Phase::Intercept);
        own.speed.0 = 850.;
        guidance.step(1, &own, leader(), leader().velocity, slot, &[], 1. / 120.);
        assert_eq!(guidance.trace.unwrap().phase, Phase::Stabilize);
        own.speed.0 = 800.;
        guidance.step(1, &own, leader(), leader().velocity, slot, &[], 1. / 120.);
        assert_eq!(guidance.trace.unwrap().phase, Phase::Capture);
    }

    #[test]
    fn afterburner_requires_aligned_distant_intercept_and_fuel() {
        let mut guidance = Guidance {
            phase: Phase::Intercept,
            side: 1.,
            ..Guidance::default()
        };
        let mut own = own([768., 20000., -20000.], 800.);
        let slot = [512., 20000., -512.];
        assert!(
            guidance
                .step(1, &own, leader(), leader().velocity, slot, &[], 1. / 120.)
                .burner
        );
        own.fuel_endurance_s = 100.;
        assert!(
            !guidance
                .step(1, &own, leader(), leader().velocity, slot, &[], 1. / 120.)
                .burner
        );
        own.fuel_endurance_s = 1000.;
        own.heading_deg = 90.;
        assert!(
            !guidance
                .step(1, &own, leader(), leader().velocity, slot, &[], 1. / 120.)
                .burner
        );
    }
}
