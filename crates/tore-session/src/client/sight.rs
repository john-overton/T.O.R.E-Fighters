//! Predicting the AC-130 gunsight's turn on the client (gunsight plan 2.13,
//! slice S4; docs/formats/net-protocol.md, "The gunsight").
//!
//! The host owns the sight: it integrates the look angles from each tick's
//! slew and zoom step and sends them back in the readout's Gunsight group, a
//! round trip late. A camera that waited for them would lag the pilot's hand
//! by that round trip, so the client turns its own copy with the same law
//! ([`gunship::slewed`], [`gunship::homeward`]) from the same quantized
//! frames the predictor steps, and corrects to the host's look:
//!
//! - The look never leaves the camera's gimbal, the hemisphere below the
//!   aircraft ([`gunship::GIMBAL_TOP`]): every step, correction and drawn
//!   offset is held inside it, as the host holds its own. The GIMBAL LIMIT
//!   notice is the host's (it rides in the readout).
//! - Every predicted tick steps the look. A free sight turns with the slew,
//!   or travels home after an L. A pinned sight looks at the host's pin from
//!   the predicted plane, turned by the slews the client made since the
//!   tick the host's pin shows. A tracked sight holds the host's look (the
//!   target camera frames a track by itself). An L (`ClearDesignation`) is
//!   predicted too: it drops a pin or a track to free slew, or starts the
//!   travel home.
//! - When a newer readout arrives, a free sight's look (for the tick before
//!   the readout's own) is compared with what the client predicted for that
//!   tick. When they differ, the client starts again from the host's look
//!   and steps the frames it sent since; a pinned sight takes the host's
//!   pin. A change of mode the client did not predict (Backslash, a pin, a
//!   kill) starts again from the host's sight too. A difference over a tenth
//!   of the field of view snaps; a smaller one is drawn away over about
//!   150 ms, like the plane's own corrections.
//!
//! *Agent decisions:* the pin and the track are never predicted (they need
//! the host's terrain and objects); a difference under ten microradians is
//! not a correction (the wire rounds the look to six);
//! and the drawn offset decays by tick, not by time, so a capture replays it
//! the same.

use super::prediction::Record;
use crate::wire::inputs::{Command, InputFrame};
use std::collections::VecDeque;
use tore_sim::attitude::Vector;
use tore_sim::combat::gunship::{self, DEFAULT_LOOK, Sight};
use tore_sim::combat::live;
use tore_sim::flight;
use tore_world::readout::GunsightReadout;
use tore_world::seats::SeatCommand;

/// Ticks of predicted looks kept, as the predictor keeps its inputs.
pub const HISTORY_TICKS: usize = super::prediction::HISTORY_TICKS;
/// A correction over this share of the field of view snaps.
pub const SNAP_FIELD: f64 = 0.1;
/// Smaller differences than this (radians) are not corrections: the wire
/// sends the look in steps of 2^-20 of a turn (6 microradians).
pub const SAME_RADIANS: f64 = 1e-5;
/// The drawn correction's time constant in ticks (0.05 s, the plane's).
pub const BLEND_TICKS: f64 = 6.;

/// What a newer readout did to the prediction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Corrected {
    /// Nothing new, or the prediction agreed.
    Same,
    /// The prediction started again from the host's look and the drawn look
    /// slides there.
    Blended,
    /// The difference was too big to slide: the drawn look jumped.
    Snapped,
}

/// The client's copy of its gunsight's line of sight. See the module notes.
#[derive(Clone, Debug, PartialEq)]
pub struct SightPrediction {
    /// The predicted look and travel-home flag after each tick, oldest first.
    after: VecDeque<(u64, [f64; 2], bool)>,
    look: [f64; 2],
    returning: bool,
    /// The newest host gunsight taken, and its readout tick.
    host: Option<(u64, GunsightReadout)>,
    /// The mode the client predicts with: the host's, or free after a
    /// predicted L.
    mode: Sight,
    /// A pinned sight's slews the host's pin does not hold yet: the tick
    /// and the turn in heading and elevation.
    pending: VecDeque<(u64, [f64; 2])>,
    /// The drawn look's offset from the prediction, decaying.
    offset: [f64; 2],
    /// Corrections made: blended and snapped.
    pub blended: u64,
    pub snapped: u64,
}

impl Default for SightPrediction {
    fn default() -> Self {
        Self {
            after: VecDeque::new(),
            look: DEFAULT_LOOK,
            returning: false,
            host: None,
            mode: Sight::Free,
            pending: VecDeque::new(),
            offset: [0.; 2],
            blended: 0,
            snapped: 0,
        }
    }
}

fn sub(a: Vector, b: Vector) -> Vector {
    std::array::from_fn(|i| a[i] - b[i])
}

/// An angle in (-pi, pi].
fn wrap(angle: f64) -> f64 {
    use std::f64::consts::PI;
    let wrapped = (angle + PI).rem_euclid(2. * PI) - PI;
    if wrapped == -PI { PI } else { wrapped }
}

/// How far apart two looks are, radians across the picture.
pub fn separation(a: [f64; 2], b: [f64; 2]) -> f64 {
    let heading = wrap(a[0] - b[0]) * a[1].cos().max(b[1].cos());
    heading.hypot(a[1] - b[1])
}

impl SightPrediction {
    /// The predicted look, as the sim would hold it now.
    pub fn look(&self) -> [f64; 2] {
        self.look
    }

    /// The look to draw: the prediction plus what is left of the last small
    /// correction.
    pub fn presented(&self) -> [f64; 2] {
        [
            wrap(self.look[0] + self.offset[0]),
            gunship::clamp_elevation(self.look[1] + self.offset[1]),
        ]
    }

    /// The predicted look after `tick`, while it is kept.
    pub fn after(&self, tick: u64) -> Option<[f64; 2]> {
        self.after
            .iter()
            .rev()
            .find(|(t, ..)| *t == tick)
            .map(|(_, look, _)| *look)
    }

    /// One predicted tick: `frame` and `commands` as the predictor stepped
    /// them, and the predicted plane after it.
    pub fn step(
        &mut self,
        tick: u64,
        frame: &InputFrame,
        commands: &[Command],
        plane: &flight::State,
    ) {
        self.advance(tick, frame, commands, Some(plane));
        self.offset = self.offset.map(|o| o * (-1. / BLEND_TICKS).exp());
        self.after.push_back((tick, self.look, self.returning));
        while self.after.len() > HISTORY_TICKS {
            self.after.pop_front();
        }
    }

    /// A pinned sight's bearing: from the camera's eye (sensor dome D on the
    /// predicted plane) at the host's pin, turned by the slews the pin does
    /// not hold yet. It can lie above the camera's gimbal.
    fn pinned_bearing(&self, pin: Vector, plane: &flight::State) -> [f64; 2] {
        let launcher = tore_world::combat::launcher(plane);
        let at = gunship::body_angles(launcher, sub(pin, gunship::eye_position(launcher)));
        let turn = self
            .pending
            .iter()
            .fold([0.; 2], |sum, (_, d)| [sum[0] + d[0], sum[1] + d[1]]);
        [wrap(at[0] + turn[0]), at[1] + turn[1]]
    }

    /// A pinned look: the bearing held inside the camera's gimbal, so a pin
    /// above the hemisphere leaves the camera at its edge, as the host's.
    fn pinned_look(&self, pin: Vector, plane: &flight::State) -> [f64; 2] {
        gunship::clamp_look(self.pinned_bearing(pin, plane))
    }

    fn advance(
        &mut self,
        tick: u64,
        frame: &InputFrame,
        commands: &[Command],
        plane: Option<&flight::State>,
    ) {
        let dropped = commands.iter().any(|c| {
            matches!(
                c,
                Command::Seat(SeatCommand::Combat(live::Command::ClearDesignation))
            )
        });
        if dropped {
            match self.mode {
                Sight::Free => self.returning = self.look != DEFAULT_LOOK,
                Sight::Pinned(_) | Sight::Tracked(_) => {
                    self.mode = Sight::Free;
                    self.returning = false;
                    self.pending.clear();
                }
            }
        }
        let deflection = frame.sight.map(|v| f64::from(v.max(-127)) / 127.);
        let deflected = deflection != [0., 0.];
        let zoom = gunship::zoom_step(frame.sight_zoom);
        match self.mode {
            Sight::Free => {
                if deflected {
                    self.returning = false;
                    self.look = gunship::slewed(self.look, deflection, zoom);
                } else if self.returning {
                    self.look = gunship::homeward(self.look);
                    self.returning = self.look != DEFAULT_LOOK;
                }
            }
            Sight::Pinned(pin) => {
                let mut bearing = self.look;
                if let Some(plane) = plane {
                    bearing = self.pinned_bearing(pin, plane);
                    self.look = gunship::clamp_look(bearing);
                }
                if deflected {
                    // The host's pin law: it neither rises above the horizon
                    // nor above where it already is.
                    let turned = gunship::slew_pin(bearing, deflection, zoom);
                    self.pending
                        .push_back((tick, [wrap(turned[0] - bearing[0]), turned[1] - bearing[1]]));
                    self.look = gunship::clamp_look(turned);
                }
            }
            Sight::Tracked(_) => {
                if let Some((_, host)) = &self.host {
                    self.look = host.look;
                }
            }
        }
    }

    /// Takes the host's gunsight from a readout of `readout_tick` and
    /// corrects the prediction to it, stepping again the `history` of
    /// frames (the predictor's) after the tick it shows; `plane` is the
    /// predicted plane now.
    pub fn correct(
        &mut self,
        readout_tick: u64,
        host: &GunsightReadout,
        history: &VecDeque<Record>,
        plane: &flight::State,
    ) -> Corrected {
        if self
            .host
            .as_ref()
            .is_some_and(|(tick, _)| *tick >= readout_tick)
        {
            return Corrected::Same;
        }
        // The readout of tick T shows the sight after the input of T - 1.
        let shown = readout_tick.saturating_sub(1);
        let predicted = self
            .after
            .iter()
            .find(|(t, ..)| *t == shown)
            .map(|(_, look, returning)| (*look, *returning));
        let same_mode = match (self.mode, host.sight) {
            (Sight::Free, Sight::Free) | (Sight::Pinned(_), Sight::Pinned(_)) => true,
            (Sight::Tracked(a), Sight::Tracked(b)) => a == b,
            _ => false,
        };
        self.host = Some((readout_tick, host.clone()));
        let before = self.presented();
        self.mode = host.sight;
        match host.sight {
            Sight::Tracked(_) => {
                // A track's look is the host's: the target camera frames it.
                self.look = host.look;
                self.pending.clear();
                if same_mode {
                    return Corrected::Same;
                }
            }
            Sight::Pinned(pin) => {
                // The host's pin holds every slew up to the tick it shows;
                // the look follows it from the predicted plane.
                self.returning = false;
                self.pending.retain(|(t, _)| *t > shown);
                self.look = self.pinned_look(pin, plane);
                if same_mode {
                    // Drawn smoothly when small; it is the plane's and the
                    // pin's own correction, not the sight's.
                    let small = separation(before, self.look)
                        <= SNAP_FIELD * gunship::field_of_view(host.zoom);
                    self.offset = if small {
                        [wrap(before[0] - self.look[0]), before[1] - self.look[1]]
                    } else {
                        [0.; 2]
                    };
                    return Corrected::Same;
                }
            }
            Sight::Free => {
                self.pending.clear();
                if same_mode
                    && let Some((look, returning)) = predicted
                    && separation(look, host.look) <= SAME_RADIANS
                    && returning == host.returning
                {
                    return Corrected::Same;
                }
                // Start again from the host's look and step the frames since.
                self.look = host.look;
                self.returning = host.returning;
                self.after.retain(|(t, ..)| *t <= shown);
                if let Some(last) = self.after.back_mut() {
                    last.1 = self.look;
                    last.2 = self.returning;
                }
                for record in history.iter().filter(|r| r.tick > shown) {
                    self.advance(record.tick, &record.frame, &record.commands, Some(plane));
                    self.after
                        .push_back((record.tick, self.look, self.returning));
                }
            }
        }
        let field = gunship::field_of_view(host.zoom);
        let jump = separation(before, self.look);
        if jump <= SAME_RADIANS {
            // The host's look stepped on comes to where the client was
            // (its first readout after seating, say): nothing to draw.
            self.offset = [0.; 2];
            return Corrected::Same;
        }
        if jump <= SNAP_FIELD * field {
            self.offset = [wrap(before[0] - self.look[0]), before[1] - self.look[1]];
            self.blended += 1;
            Corrected::Blended
        } else {
            self.offset = [0.; 2];
            self.snapped += 1;
            Corrected::Snapped
        }
    }
}
