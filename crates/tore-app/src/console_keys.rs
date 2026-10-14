//! The AC-130 sensor console's keys (slice nr-cona; John, 2026-10-14: "in
//! console it needs slew controls ... zoom controls ... maybe have keys to
//! tell the aircraft heading, altitude, and control throttle"). While the
//! console is open the stick does not fly the aircraft, so the flight
//! controls take console meanings (agent decisions, 2026-10-14):
//!
//! - **Slew:** the arrow keys (with the gunsight's fine-then-full ramp), the
//!   stick and the mouse with the right button held (as mouse look) slew the
//!   sight; Alt+arrows and the sight axes still do.
//! - **Heading and altitude:** Shift+Left and Shift+Right (the look keys)
//!   step the search heading, Shift+Up and Shift+Down the altitude (or the
//!   orbit's height); the look axes (a gamepad's right stick) do the same.
//!   A press steps once; held, it repeats after [`REPEAT_DELAY_TICKS`] every
//!   [`REPEAT_TICKS`].
//! - **Zoom and speed** are the app's ([`crate::App`]): the view zoom keys
//!   and the mouse wheel zoom the sight; the throttle keys set the speed.
//!
//! The steps leave as [`ConsoleCommand`]s on the seat's commands, so the
//! host applies them and a replay or a client sees what the host flies.
use tore_sim::autopilot::orbit::ConsoleCommand;

/// A held heading or altitude key repeats after this many ticks (0.3 s)...
pub const REPEAT_DELAY_TICKS: u32 = 36;
/// ...then every this many (0.15 s).
pub const REPEAT_TICKS: u32 = 18;
/// A look axis past this deflection counts as a held key.
pub const AXIS_HOLD: f64 = 0.5;
/// The mouse slews one field of view per this many pixels at sensitivity 1.
pub const MOUSE_PIXELS_PER_FIELD: f64 = 1_000.;
/// A full deflection slews this share of the field of view each tick.
const FIELD_PER_TICK: f64 = tore_sim::combat::sight::SLEW_FIELDS_PER_SECOND / 120.;

#[derive(Clone, Debug, Default)]
pub struct ConsoleKeys {
    /// The console is open on the presented flight (set before each tick).
    pub open: bool,
    /// Ticks each of heading left, heading right, altitude up and altitude
    /// down has been held.
    held: [u32; 4],
    /// Mouse slew still to make, fields of view, right and up positive.
    mouse: [f64; 2],
    pending: Vec<ConsoleCommand>,
}

impl ConsoleKeys {
    /// One tick of the heading and altitude keys from the look axes (right
    /// and up positive).
    pub fn step_look(&mut self, look: [f64; 2]) {
        let holds = [
            look[0] < -AXIS_HOLD,
            look[0] > AXIS_HOLD,
            look[1] > AXIS_HOLD,
            look[1] < -AXIS_HOLD,
        ];
        for (i, on) in holds.into_iter().enumerate() {
            if !on {
                self.held[i] = 0;
                continue;
            }
            let ticks = self.held[i];
            self.held[i] = ticks.saturating_add(1);
            let fires = ticks == 0
                || (ticks >= REPEAT_DELAY_TICKS
                    && (ticks - REPEAT_DELAY_TICKS).is_multiple_of(REPEAT_TICKS));
            if fires && self.pending.len() < 64 {
                self.pending.push(match i {
                    0 => ConsoleCommand::Heading(-1),
                    1 => ConsoleCommand::Heading(1),
                    2 => ConsoleCommand::Altitude(1),
                    _ => ConsoleCommand::Altitude(-1),
                });
            }
        }
    }
    /// The mouse moved `pixels` (right and up positive) with mouse look held.
    pub fn mouse_moved(&mut self, pixels: [f64; 2], sensitivity: f64) {
        for (slew, p) in self.mouse.iter_mut().zip(pixels) {
            if p.is_finite() {
                // At most a few fields queued: a flick does not run on.
                *slew = (*slew + p * sensitivity / MOUSE_PIXELS_PER_FIELD).clamp(-2., 2.);
            }
        }
    }
    /// This tick's share of the mouse slew as a deflection, -1 to 1.
    pub fn mouse_deflection(&mut self) -> [f64; 2] {
        let mut out = [0.; 2];
        for (o, slew) in out.iter_mut().zip(self.mouse.iter_mut()) {
            *o = (*slew / FIELD_PER_TICK).clamp(-1., 1.);
            *slew -= *o * FIELD_PER_TICK;
            if slew.abs() < 1e-9 {
                *slew = 0.;
            }
        }
        out
    }
    /// The console closed, a pause or a lost window: forget holds and slew.
    pub fn release(&mut self) {
        self.held = [0; 4];
        self.mouse = [0.; 2];
    }
    /// The steps made since the last call, in order.
    pub fn take(&mut self) -> Vec<ConsoleCommand> {
        std::mem::take(&mut self.pending)
    }
}

/// The sight zoom steps of a bound view zoom action (the mouse wheel's
/// `zoom-in` and `zoom-out`) in the console.
pub fn zoom_action(name: &str) -> Option<i8> {
    match name {
        "zoom-in" => Some(1),
        "zoom-out" => Some(-1),
        _ => None,
    }
}

/// The sight zoom steps of a view zoom key (=, + and -, which the flight's
/// key handler zooms the view with) in the console.
pub fn zoom_key(key: &str) -> Option<i8> {
    match key {
        "=" | "+" => Some(1),
        "-" => Some(-1),
        _ => None,
    }
}

/// The console's speed command for a throttle key's flight command (the
/// presets 1 to 6 and the steps 7 and 8, from the keyboard or a bound
/// button), or None for any other command.
pub fn throttle(command: &crate::flight_ui::Command) -> Option<ConsoleCommand> {
    use crate::flight_ui::Command;
    match *command {
        Command::ThrottlePreset(value, burner) => Some(speed_preset(value, burner)),
        Command::ThrottleStep(delta) => speed_step(delta),
        _ => None,
    }
}

/// The seat's console command for a throttle preset (`value` 0 to 1, the
/// afterburner preset full): the speed at that share of the band.
pub fn speed_preset(value: f64, burner: bool) -> ConsoleCommand {
    let share = if burner { 1. } else { value.clamp(0., 1.) };
    ConsoleCommand::Speed((share * 100.).round() as u8)
}

/// The seat's console command for a throttle step (`delta`, signed): one
/// speed step its way.
pub fn speed_step(delta: f64) -> Option<ConsoleCommand> {
    (delta != 0. && delta.is_finite()).then(|| ConsoleCommand::SpeedStep(delta.signum() as i8))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_held_look_key_steps_once_then_repeats() {
        let mut keys = ConsoleKeys::default();
        let mut steps = 0;
        for _ in 0..REPEAT_DELAY_TICKS {
            keys.step_look([1., 0.]);
            steps += keys.take().len();
        }
        assert_eq!(steps, 1, "one step for the press");
        for _ in 0..REPEAT_TICKS * 2 {
            keys.step_look([1., 0.]);
            steps += keys.take().len();
        }
        assert_eq!(steps, 3, "then a step every repeat");
        keys.step_look([0., 0.]);
        keys.step_look([0., -1.]);
        assert_eq!(keys.take(), vec![ConsoleCommand::Altitude(-1)]);
        keys.step_look([-0.6, 0.6]);
        assert_eq!(
            keys.take(),
            vec![ConsoleCommand::Heading(-1), ConsoleCommand::Altitude(1)]
        );
        // A small stick deflection is not a key.
        keys.release();
        keys.step_look([0.3, -0.3]);
        assert!(keys.take().is_empty());
    }

    #[test]
    fn the_mouse_slews_its_distance_at_most_at_full_rate() {
        let mut keys = ConsoleKeys::default();
        keys.mouse_moved([MOUSE_PIXELS_PER_FIELD * FIELD_PER_TICK * 0.5, 0.], 1.);
        assert_eq!(keys.mouse_deflection(), [0.5, 0.]);
        assert_eq!(keys.mouse_deflection(), [0., 0.]);
        // A long drag runs out at full deflection.
        keys.mouse_moved([0., -MOUSE_PIXELS_PER_FIELD * FIELD_PER_TICK * 3.], 1.);
        let mut total = 0.;
        for _ in 0..5 {
            let [_, y] = keys.mouse_deflection();
            assert!(y >= -1.);
            total += y;
        }
        assert!((total + 3.).abs() < 1e-9, "{total}");
        keys.mouse_moved([1e6, f64::NAN], 1.);
        keys.release();
        assert_eq!(keys.mouse_deflection(), [0., 0.]);
    }

    #[test]
    fn the_console_takes_the_zoom_and_throttle_keys_only() {
        use crate::flight_ui::Command;
        assert_eq!(zoom_action("zoom-in"), Some(1));
        assert_eq!(zoom_action("zoom-out"), Some(-1));
        assert_eq!(zoom_action("sight-zoom-in"), None);
        assert_eq!(zoom_key("="), Some(1));
        assert_eq!(zoom_key("+"), Some(1));
        assert_eq!(zoom_key("-"), Some(-1));
        assert_eq!(zoom_key("0"), None);
        assert_eq!(
            throttle(&Command::ThrottlePreset(0.5, false)),
            Some(ConsoleCommand::Speed(50))
        );
        assert_eq!(
            throttle(&Command::ThrottleStep(-0.05)),
            Some(ConsoleCommand::SpeedStep(-1))
        );
        assert_eq!(throttle(&Command::SensorConsole), None);
    }

    #[test]
    fn the_throttle_keys_set_the_speed() {
        assert_eq!(speed_preset(0., false), ConsoleCommand::Speed(0));
        assert_eq!(speed_preset(0.25, false), ConsoleCommand::Speed(25));
        assert_eq!(speed_preset(0.5, true), ConsoleCommand::Speed(100));
        assert_eq!(speed_step(-0.05), Some(ConsoleCommand::SpeedStep(-1)));
        assert_eq!(speed_step(0.05), Some(ConsoleCommand::SpeedStep(1)));
        assert_eq!(speed_step(0.), None);
    }
}
