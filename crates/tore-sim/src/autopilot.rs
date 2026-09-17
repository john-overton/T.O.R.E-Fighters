//! Spec-derived mode behavior with fitted control gains. See docs/spec/autopilot.md.
use crate::flight::{PilotInput, State, Switch};
use crate::models::FlightModel;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    #[default]
    Off,
    Heading,
    Waypoint,
}

/// Selected mission waypoint. Coordinates use simulation world X/Z in feet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NavigationTarget {
    pub number: u32,
    pub position: [f64; 2],
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Autopilot {
    mode: Mode,
    heading: f64,
    altitude: f64,
    target: Option<NavigationTarget>,
}

fn angle(value: f64) -> f64 {
    (value + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU) - std::f64::consts::PI
}

impl Autopilot {
    pub fn mode(&self) -> Mode {
        self.mode
    }
    pub fn label(&self) -> String {
        match self.mode {
            Mode::Off => String::new(),
            Mode::Heading => "HDG ALT".into(),
            Mode::Waypoint => self
                .target
                .map_or_else(|| "WP --".into(), |target| format!("WP {}", target.number)),
        }
    }
    /// Future route selection supplies its waypoint number and world X/Z in feet, or None.
    /// This changes guidance immediately without recapturing the held altitude.
    pub fn set_navigation_target(&mut self, target: Option<NavigationTarget>) {
        self.target = target.filter(|p| p.position.iter().all(|v| v.is_finite()));
    }
    pub fn disengage(&mut self) {
        self.mode = Mode::Off;
    }
    pub(crate) fn select(
        &mut self,
        switch: Switch,
        setting: Option<bool>,
        heading: f64,
        altitude: f64,
    ) {
        let wanted = match switch {
            Switch::Autopilot => Mode::Heading,
            Switch::WaypointAutopilot => Mode::Waypoint,
            _ => return,
        };
        if !setting.unwrap_or(self.mode != wanted) {
            if self.mode == wanted {
                self.disengage();
            }
            return;
        }
        if self.mode == Mode::Off {
            self.heading = heading;
            self.altitude = altitude;
        }
        self.mode = wanted;
    }
    fn bearing(&self, position: [f64; 3]) -> f64 {
        if self.mode == Mode::Waypoint
            && let Some(NavigationTarget {
                position: [x, z], ..
            }) = self.target
        {
            let dx = x - position[0];
            let dz = z - position[2];
            if dx.hypot(dz) > 1. / 0.3048 {
                return dx.atan2(dz);
            }
        }
        self.heading
    }
    pub(crate) fn apply(&mut self, s: &State, ground: f64, input: &mut PilotInput) {
        let c = s.model().configuration();
        if s.crashed
            || s.position[1] <= ground + c.equipment.ground_clearance_ft
            || input.pitch.abs().max(input.roll.abs()).max(input.yaw.abs()) > 0.15
        {
            self.disengage();
        }
        if self.mode == Mode::Off {
            return;
        }
        let bank = (angle(self.bearing(s.position) - s.yaw) * 1.5)
            .clamp(-30_f64.to_radians(), 30_f64.to_radians());
        let roll_rate = angle(bank - s.bank);
        let stall = c
            .aerodynamics
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .and_then(|e| e.speeds(s.position[1]))
            .map_or(900., |p| p.0);
        let authority = (s.speed / stall.max(1.)).powi(2).clamp(0., 1.);
        let roll_limit = if let Some(p) = c.controls {
            let axis = if s.research.is_some() {
                p.hybrid_roll.unwrap_or(p.roll)
            } else {
                p.roll
            };
            let bound = if roll_rate < 0. {
                -f64::from(axis.minimum)
            } else {
                f64::from(axis.maximum)
            };
            bound.to_radians() * (s.speed / (2. * stall.max(1.))).clamp(0., 1.)
        } else if s.research.is_some() {
            c.aerodynamics.roll_limit_rad_per_second.clamp(0.1, 6.) * authority
        } else {
            c.tuning.legacy_roll_limit_rad_per_second * authority
        };
        input.roll = (roll_rate / roll_limit.max(0.01)).clamp(-1., 1.);
        let climb = ((self.altitude - s.position[1]) / 10.).clamp(-20. / 0.3048, 20. / 0.3048);
        let desired_g = (1. + (climb - s.vertical_speed) / (3. * 32.174)) / s.bank.cos().max(0.5);
        let (mut low, mut high) = (-1_f64, 1_f64);
        for e in &c.aerodynamics.envelopes {
            if let Some((min, max)) = e.speeds(s.position[1])
                && s.speed >= min
                && s.speed <= max
            {
                low = low.min(e.g as f64);
                high = high.max(e.g as f64);
            }
        }
        let loading = 1.
            + (s.fuel + s.payload_lbs) / c.mass.empty_lbs * c.aerodynamics.loaded_elevator_percent
                / 100.;
        low /= loading;
        high /= loading;
        let delta = desired_g / authority.max(0.01) - 1.;
        input.pitch = (delta
            / if delta > 0. {
                (high - 1.).max(0.01)
            } else {
                (1. - low).max(0.01)
            })
        .clamp(-1., 1.);
        input.yaw = 0.;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flight::{PilotCommand, integration_tests::profile};
    fn state(hybrid: bool) -> State {
        let mut s = State::new(&profile(), [0., 10000., 0.]).unwrap();
        if hybrid {
            s.enable_research(42).unwrap();
        }
        s
    }
    fn toggle(s: &mut State, switch: Switch) {
        s.step(
            &PilotInput {
                commands: vec![PilotCommand::Toggle(switch)],
                ..Default::default()
            },
            |_, _| 0.,
        );
    }
    #[test]
    fn mode_capture_fallback_override_and_contact() {
        let mut s = state(true);
        toggle(&mut s, Switch::Autopilot);
        let capture = (s.autopilot.heading, s.autopilot.altitude);
        s.yaw = 1.;
        s.position[1] += 100.;
        toggle(&mut s, Switch::WaypointAutopilot);
        assert_eq!((s.autopilot.heading, s.autopilot.altitude), capture);
        assert_eq!(s.autopilot.bearing(s.position), capture.0);
        assert_eq!(s.autopilot.label(), "WP --");
        s.autopilot.set_navigation_target(Some(NavigationTarget {
            number: 1,
            position: [s.position[0], s.position[2]],
        }));
        assert_eq!(s.autopilot.bearing(s.position), capture.0);
        s.autopilot.set_navigation_target(Some(NavigationTarget {
            number: 1,
            position: [f64::NAN, 1.],
        }));
        assert_eq!(s.autopilot.target, None);
        toggle(&mut s, Switch::WaypointAutopilot);
        assert_eq!(s.autopilot.mode(), Mode::Off);
        for axis in 0..3 {
            toggle(&mut s, Switch::Autopilot);
            let mut input = PilotInput::default();
            match axis {
                0 => input.pitch = 0.15,
                1 => input.roll = -0.15,
                _ => input.yaw = 0.15,
            }
            s.step(&input, |_, _| 0.);
            assert_eq!(s.autopilot.mode(), Mode::Heading);
            input.pitch *= 1.01;
            input.roll *= 1.01;
            input.yaw *= 1.01;
            s.step(&input, |_, _| 0.);
            assert_eq!(s.autopilot.mode(), Mode::Off);
        }
        toggle(&mut s, Switch::Autopilot);
        s.step(&PilotInput::default(), |_, _| 20000.);
        assert_eq!(s.autopilot.mode(), Mode::Off);
    }
    #[test]
    fn waypoint_updates_shortest_turn_and_manual_equipment() {
        let mut s = state(true);
        s.yaw = 359_f64.to_radians();
        s.command(PilotCommand::Toggle(Switch::WaypointAutopilot));
        let target = |degrees: f64| {
            let radians = degrees.to_radians();
            [radians.sin() * 100000., radians.cos() * 100000.]
        };
        s.autopilot.set_navigation_target(Some(NavigationTarget {
            number: 7,
            position: target(1.),
        }));
        assert_eq!(s.autopilot.label(), "WP 7");
        let mut ap = s.autopilot.clone();
        let mut input = PilotInput::default();
        ap.apply(&s, 0., &mut input);
        assert!(input.roll > 0. && input.roll < 0.1);
        ap.set_navigation_target(Some(NavigationTarget {
            number: 8,
            position: target(357.),
        }));
        ap.apply(&s, 0., &mut PilotInput::default());
        let mut opposite = PilotInput::default();
        ap.apply(&s, 0., &mut opposite);
        assert!(opposite.roll < 0. && opposite.roll > -0.1);
        s.step(
            &PilotInput {
                throttle: Some(0.5),
                commands: vec![PilotCommand::Toggle(Switch::Gear)],
                ..Default::default()
            },
            |_, _| 0.,
        );
        assert_eq!(s.throttle, 0.5);
        assert!(s.gear_down);
        assert_eq!(s.autopilot.mode(), Mode::Waypoint);
        s.crashed = true;
        s.step(&PilotInput::default(), |_, _| 0.);
        assert_eq!(s.autopilot.mode(), Mode::Off);
    }

    #[test]
    fn mode_tape_replays_identically() {
        use tore_input::recording;
        let frames: Vec<_> = (0..1200)
            .map(|tick| PilotInput {
                commands: match tick {
                    0 | 900 => vec![PilotCommand::Toggle(Switch::Autopilot)],
                    400 | 700 => vec![PilotCommand::Toggle(Switch::WaypointAutopilot)],
                    _ => vec![],
                },
                ..Default::default()
            })
            .collect();
        let mut tape = format!("{}\n", recording::HEADER).into_bytes();
        let mut live = state(true);
        for (i, frame) in frames.iter().enumerate() {
            recording::write_frame(&mut tape, i as u64 + 1, frame).unwrap();
            live.step(frame, |_, _| 0.);
        }
        let mut replay = state(true);
        for frame in recording::read(tape.as_slice()).unwrap() {
            replay.step(&frame, |_, _| 0.);
        }
        assert_eq!(live, replay);
        assert_eq!(state(true).autopilot.mode(), Mode::Off);
    }

    #[test]
    fn holds_and_turns_closed_loop_in_both_adapters() {
        for hybrid in [false, true] {
            for nav in [false, true] {
                let mut s = state(hybrid);
                s.command(PilotCommand::Toggle(if nav {
                    Switch::WaypointAutopilot
                } else {
                    Switch::Autopilot
                }));
                let heading = s.yaw;
                if nav {
                    s.autopilot.set_navigation_target(Some(NavigationTarget {
                        number: 1,
                        position: [100000., 100000.],
                    }));
                }
                s.bank = 20_f64.to_radians();
                s.position[1] -= 100.;
                for _ in 0..120 * 120 {
                    s.step(&PilotInput::default(), |_, _| 0.);
                }
                let error = angle(s.autopilot.bearing(s.position) - s.yaw).to_degrees();
                assert!(
                    error.abs() < 3.,
                    "hybrid={hybrid} nav={nav} heading error={error}"
                );
                assert!(
                    (s.position[1] - 10000.).abs() < 100.,
                    "hybrid={hybrid} nav={nav} altitude={}",
                    s.position[1]
                );
                assert!(s.bank.abs() < 5_f64.to_radians());
                assert!(!s.crashed);
                assert!(s.stall_alert(0.).is_none());
                assert_eq!(s.throttle, 0.7);
                if nav {
                    assert!(angle(s.yaw - heading).abs() > 0.2);
                }
            }
        }
    }
}
