//! Development watch for the headless flight probe (`--headless-flight`).
//!
//! It records the extremes of a run and prints one `extremes:` line so a
//! script can look for impossible states (a non-finite value, fuel that rises,
//! energy gained with the engine dead) without a per-tick log. With
//! `--flight-trace TICKS` it also prints a `trace:` line every that many ticks.
//! This is a harness, not game behaviour.
use crate::flight;

const FEET_PER_KNOT: f64 = 1.68781;
const GRAVITY_FPS2: f64 = 32.174;

#[derive(Debug)]
pub struct FlightWatch {
    /// Ticks between `trace:` lines, 0 for none.
    pub trace_ticks: u64,
    samples: u64,
    non_finite: u64,
    max_speed_kt: f64,
    max_g: f64,
    min_g: f64,
    min_altitude_ft: f64,
    max_altitude_ft: f64,
    max_pitch_rate_dps: f64,
    max_roll_rate_dps: f64,
    first_fuel_lb: f64,
    fuel_rise_lb: f64,
    last_fuel_lb: f64,
    last_energy_ft: f64,
    /// Largest energy height gained in one tick while the engine was off.
    max_dead_stick_gain_ft: f64,
    /// Total energy height gained over the whole run while the engine was off.
    dead_stick_gain_ft: f64,
    /// Full thrust over empty weight: energy height can rise no faster than
    /// this times the speed.
    thrust_to_weight: f64,
    /// Energy height at the last one-second mark, and the largest one-second gain.
    energy_mark_ft: f64,
    max_energy_rate_fps: f64,
    /// The largest ratio of a one-second energy gain to what thrust could give.
    max_energy_ratio: f64,
    /// The largest ratio of airspeed to the envelope's top speed at that altitude.
    max_over_top_speed: f64,
    /// Lowest height above the terrain and ticks spent well below it, when the
    /// run has terrain.
    min_agl_ft: Option<f64>,
    under_ground_ticks: u64,
    /// The pilot's G effects, stepped as the flight does, for the veil's peaks.
    veil: tore_sim::g_effects::GEffects,
    max_blackout: f64,
    max_redout: f64,
}

impl FlightWatch {
    pub fn new(trace_ticks: u64, state: &flight::State) -> Self {
        Self {
            trace_ticks,
            samples: 0,
            non_finite: 0,
            max_speed_kt: 0.,
            max_g: f64::MIN,
            min_g: f64::MAX,
            min_altitude_ft: f64::MAX,
            max_altitude_ft: f64::MIN,
            max_pitch_rate_dps: 0.,
            max_roll_rate_dps: 0.,
            first_fuel_lb: total_fuel(state),
            fuel_rise_lb: 0.,
            last_fuel_lb: total_fuel(state),
            last_energy_ft: energy_ft(state),
            max_dead_stick_gain_ft: 0.,
            dead_stick_gain_ft: 0.,
            thrust_to_weight: {
                use tore_sim::models::FlightModel;
                let c = state.model().configuration();
                c.propulsion
                    .afterburner_thrust_lbf
                    .max(c.propulsion.military_thrust_lbf)
                    / c.mass.empty_lbs.max(1.)
            },
            energy_mark_ft: energy_ft(state),
            max_energy_rate_fps: 0.,
            max_energy_ratio: 0.,
            max_over_top_speed: 0.,
            min_agl_ft: None,
            under_ground_ticks: 0,
            veil: Default::default(),
            max_blackout: 0.,
            max_redout: 0.,
        }
    }

    /// Record the state after a step.
    pub fn observe(&mut self, state: &flight::State) {
        self.samples += 1;
        let finite = state.position.iter().all(|v| v.is_finite())
            && state.velocity.iter().all(|v| v.is_finite())
            && [
                state.speed,
                state.g,
                state.yaw,
                state.pitch,
                state.bank,
                state.fuel,
                state.roll_rate,
                state.pitch_rate,
                state.throttle,
            ]
            .iter()
            .all(|v| v.is_finite());
        if !finite {
            self.non_finite += 1;
            return;
        }
        self.veil.step(state.g, !state.cheats.no_g_effects);
        self.max_blackout = self.max_blackout.max(self.veil.blackout);
        self.max_redout = self.max_redout.max(self.veil.redout);
        self.max_speed_kt = self.max_speed_kt.max(state.speed / FEET_PER_KNOT);
        self.max_g = self.max_g.max(state.g);
        self.min_g = self.min_g.min(state.g);
        self.min_altitude_ft = self.min_altitude_ft.min(state.position[1]);
        self.max_altitude_ft = self.max_altitude_ft.max(state.position[1]);
        self.max_pitch_rate_dps = self
            .max_pitch_rate_dps
            .max(state.pitch_rate.to_degrees().abs());
        self.max_roll_rate_dps = self
            .max_roll_rate_dps
            .max(state.roll_rate.to_degrees().abs());
        let fuel = total_fuel(state);
        if fuel > self.last_fuel_lb + 1e-6 {
            self.fuel_rise_lb += fuel - self.last_fuel_lb;
        }
        self.last_fuel_lb = fuel;
        let energy = energy_ft(state);
        // Only a free, airborne aircraft has an energy budget: a crash, a
        // wheel-braked roll or a ground-contact correction is not one.
        let free = !state.crashed && !state.engine && state.position[1] > 60.;
        if free && energy > self.last_energy_ft + 1e-9 {
            let gain = energy - self.last_energy_ft;
            self.max_dead_stick_gain_ft = self.max_dead_stick_gain_ft.max(gain);
            self.dead_stick_gain_ft += gain;
        }
        self.last_energy_ft = energy;
        let top = state
            .trace()
            .adapter
            .map_or(0., |adapter| adapter.envelope.top_speed_fps);
        if top > 0. && !state.crashed && state.position[1] > 60. {
            self.max_over_top_speed = self.max_over_top_speed.max(state.speed / top);
        }
        // One-second energy gain against the thrust power available at this speed,
        // only while flying free with the engine on.
        if state.ticks.is_multiple_of(120) {
            if state.engine && !state.crashed && state.position[1] > 60. {
                let rate = energy - self.energy_mark_ft;
                let bound = self.thrust_to_weight * state.speed.max(1.);
                self.max_energy_rate_fps = self.max_energy_rate_fps.max(rate);
                self.max_energy_ratio = self.max_energy_ratio.max(rate / bound);
            }
            self.energy_mark_ft = energy;
        }
        if self.trace_ticks > 0 && state.ticks.is_multiple_of(self.trace_ticks) {
            println!(
                "trace: tick={} alt_ft={:.1} speed_kt={:.1} vs_fps={:.1} g={:.2} aoa_deg={:.1} pitch_deg={:.1} bank_deg={:.1} yaw_deg={:.1} throttle={:.2} fuel_lb={:.0} gear={:.2} flaps={:.2} brake={:.2} hook={:.2} on_ground={}",
                state.ticks,
                state.position[1],
                state.speed / FEET_PER_KNOT,
                state.velocity[1],
                state.g,
                angle_of_attack_deg(state),
                state.pitch.to_degrees(),
                state.bank.to_degrees(),
                state.yaw.to_degrees(),
                state.throttle,
                fuel,
                state.gear,
                state.flaps,
                state.brake,
                state.hook,
                state.research.as_ref().is_some_and(|r| r.on_ground),
            );
        }
    }

    /// Record the height above the terrain under the aircraft after a step.
    pub fn observe_ground(&mut self, state: &flight::State, ground_height_ft: f64) {
        let agl = state.position[1] - ground_height_ft;
        self.min_agl_ft = Some(self.min_agl_ft.map_or(agl, |m| m.min(agl)));
        // Well below the surface: more than the wheels' clearance and a margin.
        if agl < -30. {
            self.under_ground_ticks += 1;
        }
    }

    /// The `extremes:` summary line.
    pub fn report(&self) -> String {
        if self.samples == 0 || self.max_g == f64::MIN {
            return format!(
                "extremes: samples={} non_finite={}",
                self.samples, self.non_finite
            );
        }
        format!(
            "extremes: samples={} non_finite={} max_speed_kt={:.1} max_g={:.2} min_g={:.2} min_altitude_ft={:.1} max_altitude_ft={:.1} max_pitch_rate_dps={:.1} max_roll_rate_dps={:.1} fuel_start_lb={:.1} fuel_end_lb={:.1} fuel_rise_lb={:.3} dead_stick_gain_ft={:.3} max_dead_stick_step_ft={:.4} max_energy_rate_fps={:.1} energy_rate_over_thrust={:.3} speed_over_envelope_top={:.3} max_blackout={:.3} max_redout={:.3}",
            self.samples,
            self.non_finite,
            self.max_speed_kt,
            self.max_g,
            self.min_g,
            self.min_altitude_ft,
            self.max_altitude_ft,
            self.max_pitch_rate_dps,
            self.max_roll_rate_dps,
            self.first_fuel_lb,
            self.last_fuel_lb,
            self.fuel_rise_lb,
            self.dead_stick_gain_ft,
            self.max_dead_stick_gain_ft,
            self.max_energy_rate_fps,
            self.max_energy_ratio,
            self.max_over_top_speed,
            self.max_blackout,
            self.max_redout,
        ) + &self.min_agl_ft.map_or(String::new(), |agl| {
            format!(
                " min_agl_ft={agl:.1} under_ground_ticks={}",
                self.under_ground_ticks
            )
        })
    }
}

/// Watches the four deployable devices (gear, flaps, airbrake, hook): positions
/// must stay within 0..1, never move against the command, and the time a full
/// travel takes is reported so it can be compared with the aircraft's own
/// deployment time.
#[derive(Debug, Default)]
pub struct DeviceWatch {
    previous: [f64; 4],
    commanded: [bool; 4],
    changed_at: [u64; 4],
    /// Seconds of the last full travel down (deploying) and up, per device.
    down_seconds: [Option<f64>; 4],
    up_seconds: [Option<f64>; 4],
    violations: u32,
    first: Option<String>,
}

const DEVICE_NAMES: [&str; 4] = ["gear", "flaps", "brake", "hook"];

impl DeviceWatch {
    pub fn new(state: &flight::State) -> Self {
        Self {
            previous: Self::positions(state),
            commanded: Self::commands(state),
            ..Default::default()
        }
    }

    fn positions(state: &flight::State) -> [f64; 4] {
        [state.gear, state.flaps, state.brake, state.hook]
    }

    fn commands(state: &flight::State) -> [bool; 4] {
        [
            state.gear_down,
            state.flaps_down,
            state.brake_out,
            state.hook_down,
        ]
    }

    pub fn observe(&mut self, state: &flight::State) {
        let now = Self::positions(state);
        let commanded = Self::commands(state);
        for i in 0..4 {
            if commanded[i] != self.commanded[i] {
                self.commanded[i] = commanded[i];
                self.changed_at[i] = state.ticks;
            }
            let delta = now[i] - self.previous[i];
            let bad = if !(-1e-9..=1. + 1e-9).contains(&now[i]) {
                Some("left 0..1")
            } else if (commanded[i] && delta < -1e-9) || (!commanded[i] && delta > 1e-9) {
                Some("moved against its command")
            } else {
                None
            };
            if let Some(why) = bad {
                self.violations += 1;
                self.first.get_or_insert_with(|| {
                    format!(
                        "{}_{}_at_tick_{}",
                        DEVICE_NAMES[i],
                        why.replace(' ', "_"),
                        state.ticks
                    )
                });
            }
            let seconds = (state.ticks - self.changed_at[i]) as f64 / 120.;
            if commanded[i] && now[i] >= 1. && self.previous[i] < 1. {
                self.down_seconds[i] = Some(seconds);
            }
            if !commanded[i] && now[i] <= 0. && self.previous[i] > 0. {
                self.up_seconds[i] = Some(seconds);
            }
        }
        self.previous = now;
    }

    pub fn report(&self, state: &flight::State) -> String {
        use tore_sim::models::FlightModel;
        let show = |v: Option<f64>| v.map_or("none".to_owned(), |v| format!("{v:.2}"));
        let mut line = format!(
            "devices: violations={} first_violation={} deployment_seconds={:.2} hook_available={}",
            self.violations,
            self.first.as_deref().unwrap_or("none"),
            state.model().configuration().equipment.deployment_seconds,
            state.hook_available()
        );
        for i in 0..4 {
            line += &format!(
                " {0}_down_s={1} {0}_up_s={2}",
                DEVICE_NAMES[i],
                show(self.down_seconds[i]),
                show(self.up_seconds[i])
            );
        }
        line
    }
}

fn total_fuel(state: &flight::State) -> f64 {
    state.fuel + state.systems.external_lbs()
}

/// Specific energy as a height: altitude plus the height the speed is worth.
fn energy_ft(state: &flight::State) -> f64 {
    state.position[1] + state.speed * state.speed / (2. * GRAVITY_FPS2)
}

fn angle_of_attack_deg(state: &flight::State) -> f64 {
    let basis = crate::attitude::Basis::new(state.yaw, state.pitch, state.bank);
    let forward = crate::attitude::dot(state.velocity, basis.forward);
    let up = crate::attitude::dot(state.velocity, basis.up);
    (-up).atan2(forward).to_degrees()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> flight::State {
        flight::State::new(&crate::flight::animation_tests::profile(), [0., 5000., 0.]).unwrap()
    }

    #[test]
    fn a_calm_run_reports_no_problems() {
        let mut s = state();
        let mut watch = FlightWatch::new(0, &s);
        for _ in 0..240 {
            s.step(&flight::PilotInput::default(), |_, _| 0.);
            watch.observe(&s);
        }
        let report = watch.report();
        assert!(report.contains("samples=240 non_finite=0"), "{report}");
        assert!(report.contains("fuel_rise_lb=0.000"), "{report}");
    }

    #[test]
    fn rising_fuel_and_non_finite_values_are_reported() {
        let mut s = state();
        let mut watch = FlightWatch::new(0, &s);
        s.step(&flight::PilotInput::default(), |_, _| 0.);
        watch.observe(&s);
        s.fuel += 10.;
        watch.observe(&s);
        s.speed = f64::NAN;
        watch.observe(&s);
        let report = watch.report();
        assert!(report.contains("fuel_rise_lb=10.000"), "{report}");
        assert!(report.contains("non_finite=1"), "{report}");
    }

    #[test]
    fn energy_gained_with_the_engine_off_is_reported() {
        let mut s = state();
        s.engine = false;
        let mut watch = FlightWatch::new(0, &s);
        s.speed += 100.;
        watch.observe(&s);
        assert!(!watch.report().contains("dead_stick_gain_ft=0.000"));
    }
}
