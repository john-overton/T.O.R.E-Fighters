//! Scripted pilots for the headless flight probe (`--headless-flight` with
//! `--maneuver spin-recover` or `--maneuver land`). Development harnesses, not
//! game behaviour: they exist so a script can fly the same manoeuvre in every
//! aircraft and check the outcome.
use crate::flight::{self, PilotCommand, PilotInput, Switch};
use tore_sim::models::FlightModel;

/// Height the spin recovery probe starts at, feet above the flat probe ground.
pub const SPIN_START_FT: f64 = 15_000.;
/// Ticks of pro-spin control allowed before the probe gives up on entry.
const SPIN_ENTRY_LIMIT: u64 = 120 * 30;
/// Ticks the spin is held once it has started, so recovery begins from a
/// developed rotation.
const SPIN_HOLD_TICKS: u64 = 120 * 4;
/// Ticks allowed for recovery once the manual procedure starts.
const SPIN_RECOVERY_LIMIT: u64 = 120 * 60;
/// Ticks the aircraft must stay out of the spin to count as recovered.
const SPIN_CALM_TICKS: u64 = 120 * 2;

/// The manual's spin recovery (Flight Manual, Spin Recovery): centre the
/// stick, full opposite rudder, stick forward slightly, full throttle, hold
/// until the aircraft stops rotating.
#[derive(Debug)]
pub struct SpinRecovery {
    phase: SpinPhase,
    phase_start: u64,
    entered_at: Option<u64>,
    recovery_started_at: Option<u64>,
    recovered_at: Option<u64>,
    calm_since: Option<u64>,
    start_altitude_ft: f64,
    altitude_at_recovery_start_ft: f64,
    min_altitude_ft: f64,
    last_yaw: f64,
    turned_rad: f64,
    turned_at_recovery_start_rad: f64,
    max_spin_rate_dps: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpinPhase {
    Enter,
    Hold,
    Recover,
    Done,
}

impl SpinRecovery {
    pub fn new(state: &flight::State) -> Self {
        Self {
            phase: SpinPhase::Enter,
            phase_start: 0,
            entered_at: None,
            recovery_started_at: None,
            recovered_at: None,
            calm_since: None,
            start_altitude_ft: state.position[1],
            altitude_at_recovery_start_ft: state.position[1],
            min_altitude_ft: state.position[1],
            last_yaw: state.yaw,
            turned_rad: 0.,
            turned_at_recovery_start_rad: 0.,
            max_spin_rate_dps: 0.,
        }
    }

    pub fn finished(&self) -> bool {
        self.phase == SpinPhase::Done
    }

    /// Controls for the next tick, from the state after the last one.
    pub fn keys(&mut self, state: &flight::State) -> PilotInput {
        let tick = state.ticks;
        let mut delta = state.yaw - self.last_yaw;
        while delta > std::f64::consts::PI {
            delta -= std::f64::consts::TAU;
        }
        while delta < -std::f64::consts::PI {
            delta += std::f64::consts::TAU;
        }
        self.turned_rad += delta;
        self.last_yaw = state.yaw;
        self.min_altitude_ft = self.min_altitude_ft.min(state.position[1]);
        let (spinning, rate) = state
            .research
            .as_ref()
            .map_or((false, 0.), |r| (r.spinning != 0, r.spin_rate));
        self.max_spin_rate_dps = self.max_spin_rate_dps.max(rate.to_degrees().abs());
        let mut keys = PilotInput::default();
        if state.crashed {
            self.phase = SpinPhase::Done;
            return keys;
        }
        match self.phase {
            SpinPhase::Enter => {
                keys.pitch = 1.;
                keys.yaw = 1.;
                if spinning {
                    self.entered_at = Some(tick);
                    self.phase = SpinPhase::Hold;
                    self.phase_start = tick;
                } else if tick >= SPIN_ENTRY_LIMIT {
                    self.phase = SpinPhase::Done;
                }
            }
            SpinPhase::Hold => {
                keys.pitch = 1.;
                keys.yaw = 1.;
                if tick >= self.phase_start + SPIN_HOLD_TICKS {
                    self.phase = SpinPhase::Recover;
                    self.phase_start = tick;
                    self.recovery_started_at = Some(tick);
                    self.altitude_at_recovery_start_ft = state.position[1];
                    self.turned_at_recovery_start_rad = self.turned_rad;
                }
            }
            SpinPhase::Recover => {
                // The HUD names the rudder to apply: against the rotation.
                let against = if rate > 0. {
                    -1.
                } else if rate < 0. {
                    1.
                } else {
                    0.
                };
                keys.yaw = against;
                keys.pitch = -0.3;
                keys.commands = vec![
                    PilotCommand::Throttle(1.),
                    PilotCommand::Set(Switch::Burner, true),
                ];
                let out = !spinning && state.stall_alert(f64::MIN).is_none();
                if out {
                    let since = *self.calm_since.get_or_insert(tick);
                    if tick >= since + SPIN_CALM_TICKS {
                        self.recovered_at = Some(since);
                        self.phase = SpinPhase::Done;
                    }
                } else {
                    self.calm_since = None;
                }
                if tick >= self.phase_start + SPIN_RECOVERY_LIMIT {
                    self.phase = SpinPhase::Done;
                }
            }
            SpinPhase::Done => {}
        }
        keys
    }

    pub fn report(&self, state: &flight::State) -> String {
        let show = |t: Option<u64>| t.map_or("never".to_owned(), |t| t.to_string());
        format!(
            "spin_recovery: entered_tick={} recovery_started_tick={} recovered_tick={} revolutions_during_recovery={:.2} revolutions_total={:.2} altitude_lost_ft={:.0} start_altitude_ft={:.0} min_altitude_ft={:.0} max_spin_rate_dps={:.1} crashed={}",
            show(self.entered_at),
            show(self.recovery_started_at),
            show(self.recovered_at),
            (self.turned_rad - self.turned_at_recovery_start_rad).abs() / std::f64::consts::TAU,
            self.turned_rad.abs() / std::f64::consts::TAU,
            self.start_altitude_ft - self.min_altitude_ft,
            self.start_altitude_ft,
            self.min_altitude_ft,
            self.max_spin_rate_dps,
            state.crashed,
        )
    }
}

/// Glide slope the landing probe flies, degrees.
const GLIDE_SLOPE_DEG: f64 = 3.;
/// Distance from the aim point where the landing probe starts, feet.
pub const LANDING_START_FT: f64 = 4. * 6076.;
/// The aim point sits this far past the threshold, feet.
pub const AIM_PAST_THRESHOLD_FT: f64 = 1_000.;
/// Height above the wheels' support plane where the probe starts the flare.
const FLARE_HEIGHT_FT: f64 = 30.;
/// Ticks after touchdown the rollout may take before the probe gives up.
const ROLLOUT_LIMIT: u64 = 120 * 240;

/// A scripted approach and landing on the ground-start runway, flown with the
/// player's own controls: gear, flaps and hook down, a three degree glide slope
/// at an approach speed from the aircraft's own limits, a flare, idle, brakes
/// on. `fitted` test harness (agent decision, 2026-09-28), not game behaviour;
/// it exists to check touchdown grading, rollout and crash handling.
#[derive(Debug)]
pub struct Landing {
    /// Aim point on the runway, feet (x, height of the surface, z).
    aim: [f64; 3],
    /// Unit vector along the landing direction, (x, z).
    dir: [f64; 2],
    length_ft: f64,
    threshold_along: f64,
    approach_fps: f64,
    limits_forward_fps: f64,
    clearance_ft: f64,
    phase: LandingPhase,
    touchdown: Option<TouchdownRecord>,
    touchdown_tick: u64,
    stopped_since: Option<u64>,
    stop_tick: Option<u64>,
    start_tick: u64,
    max_cross_ft: f64,
    max_sink_fps: f64,
    unsafe_reason: Option<String>,
    bounces: u32,
    last_on_ground: bool,
    touchdown_position: Option<[f64; 3]>,
    stop_position: Option<[f64; 3]>,
    rollout_start_speed_fps: f64,
    min_agl_ft: f64,
    variant: LandingVariant,
    /// First tick the aircraft rolled onto ground that is not a runway, after
    /// touching down, and its ground speed then.
    left_runway_tick: Option<u64>,
    left_runway_kt: f64,
}

/// What the scripted landing does wrong on purpose, to check the game's
/// crash handling. `Normal` is a good landing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LandingVariant {
    Normal,
    /// Gear left up.
    GearUp,
    /// No flare: touch down at the glide slope's sink rate.
    Hard,
    /// Line up 1,500 feet to the side of the runway.
    OffRunway,
}

impl LandingVariant {
    /// `land`, `land-gear-up`, `land-hard` or `land-off-runway`.
    pub fn from_maneuver(name: &str) -> Option<Self> {
        Some(match name {
            "land" => Self::Normal,
            "land-gear-up" => Self::GearUp,
            "land-hard" => Self::Hard,
            "land-off-runway" => Self::OffRunway,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LandingPhase {
    Approach,
    Rollout,
    Done,
}

#[derive(Clone, Copy, Debug)]
struct TouchdownRecord {
    forward_fps: f64,
    side_fps: f64,
    vertical_fps: f64,
    pitch_deg: f64,
    bank_deg: f64,
    score: Option<u32>,
}

impl Landing {
    /// `threshold` and `heading` describe the runway end the landing runs from,
    /// `ground` gives the surface under any point.
    pub fn new(
        state: &flight::State,
        threshold: [f64; 3],
        heading: f64,
        length_ft: f64,
        aim_height: impl Fn(f64, f64) -> f64,
        variant: LandingVariant,
    ) -> Self {
        let dir = [heading.sin(), heading.cos()];
        let aim_x = threshold[0] + dir[0] * AIM_PAST_THRESHOLD_FT;
        let aim_z = threshold[2] + dir[1] * AIM_PAST_THRESHOLD_FT;
        let c = state.model().configuration();
        let limits = c.native.landing;
        // Clean 1 g stall speed at sea level, and a margin. With flaps the
        // model lowers the stall speed a quarter, so this is conservative.
        let stall = c
            .aerodynamics
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .and_then(|e| e.speeds(0.))
            .map_or(200., |(low, _)| low);
        let approach = (stall * 1.3)
            .min(f64::from(limits.forward_fps) * 0.85)
            .max(stall * 1.1);
        Self {
            aim: [aim_x, aim_height(aim_x, aim_z), aim_z],
            dir,
            length_ft,
            threshold_along: -AIM_PAST_THRESHOLD_FT,
            approach_fps: approach,
            limits_forward_fps: f64::from(limits.forward_fps),
            clearance_ft: c.equipment.ground_clearance_ft,
            phase: LandingPhase::Approach,
            touchdown: None,
            touchdown_tick: 0,
            stopped_since: None,
            stop_tick: None,
            start_tick: state.ticks,
            max_cross_ft: 0.,
            max_sink_fps: 0.,
            unsafe_reason: None,
            bounces: 0,
            last_on_ground: false,
            touchdown_position: None,
            stop_position: None,
            rollout_start_speed_fps: 0.,
            min_agl_ft: f64::MAX,
            variant,
            left_runway_tick: None,
            left_runway_kt: 0.,
        }
    }

    /// Put the aircraft on the approach: gear, flaps and hook down, on the
    /// glide slope, on the runway centre line, at the approach speed.
    pub fn place(&self, state: &mut flight::State, heading: f64) {
        let along = -LANDING_START_FT;
        let height =
            self.aim[1] + (-along) * GLIDE_SLOPE_DEG.to_radians().tan() + self.clearance_ft;
        let offset = if self.variant == LandingVariant::OffRunway {
            1_500.
        } else {
            0.
        };
        state.position = [
            self.aim[0] + self.dir[0] * along + self.dir[1] * offset,
            height,
            self.aim[2] + self.dir[1] * along - self.dir[0] * offset,
        ];
        state.yaw = heading;
        state.pitch = 0.;
        state.bank = 0.;
        let gamma = -GLIDE_SLOPE_DEG.to_radians();
        state.speed = self.approach_fps;
        state.velocity = [
            self.dir[0] * self.approach_fps * gamma.cos(),
            self.approach_fps * gamma.sin(),
            self.dir[1] * self.approach_fps * gamma.cos(),
        ];
        state.roll_rate = 0.;
        state.pitch_rate = 0.;
        state.engine = true;
        state.burner = false;
        state.throttle = 0.5;
        let gear = self.variant != LandingVariant::GearUp;
        state.gear_down = gear;
        state.gear = f64::from(gear);
        state.flaps_down = true;
        state.flaps = 1.;
        state.hook_down = true;
        state.hook = 1.;
        state.brake_out = false;
        state.brake = 0.;
        if let Some(research) = state.research.as_mut() {
            research.on_ground = false;
        }
    }

    pub fn finished(&self) -> bool {
        self.phase == LandingPhase::Done
    }

    /// Distance past the aim point along the runway and to its right, feet.
    fn along_cross(&self, position: [f64; 3]) -> (f64, f64) {
        let dx = position[0] - self.aim[0];
        let dz = position[2] - self.aim[2];
        (
            dx * self.dir[0] + dz * self.dir[1],
            dx * self.dir[1] - dz * self.dir[0],
        )
    }

    /// Controls for the next tick, from the state after the last one.
    pub fn keys(
        &mut self,
        state: &flight::State,
        ground_height: f64,
        landable: bool,
    ) -> PilotInput {
        let mut keys = PilotInput::default();
        let agl = state.position[1] - ground_height - self.clearance_ft;
        self.min_agl_ft = self.min_agl_ft.min(agl);
        let on_ground = state.research.as_ref().is_some_and(|r| r.on_ground);
        if let Some(contact) = state.trace().contact {
            use crate::flight::trace::Contact;
            match contact {
                Contact::Rolling(rolling) => {
                    if let Some(landing) = rolling.touchdown {
                        let t = landing.touchdown;
                        if self.touchdown.is_none() {
                            self.touchdown_tick = state.ticks;
                            self.touchdown_position = Some(state.position);
                            self.rollout_start_speed_fps = rolling.ground_speed_fps;
                            self.touchdown = Some(TouchdownRecord {
                                forward_fps: t.forward_fps,
                                side_fps: t.side_fps,
                                vertical_fps: t.vertical_fps,
                                pitch_deg: t.pitch_deg,
                                bank_deg: t.bank_deg,
                                score: landing.score,
                            });
                        }
                    }
                }
                Contact::Unsafe(u) => {
                    if self.unsafe_reason.is_none() {
                        let t = u.touchdown;
                        self.unsafe_reason = Some(format!(
                            "unsafe touchdown severity={:?} water={} not_landable={} gear_up={} forward_fps={:.1} side_fps={:.1} vertical_fps={:.1} pitch_deg={:.1} bank_deg={:.1} limits(fwd={} side={} descent={} pitch={} roll={})",
                            u.severity,
                            u.water,
                            u.not_landable,
                            u.gear_up,
                            t.forward_fps,
                            t.side_fps,
                            t.vertical_fps,
                            t.pitch_deg,
                            t.bank_deg,
                            t.limits.forward_fps,
                            t.limits.side_fps,
                            t.limits.descent_fps,
                            t.limits.pitch_degrees,
                            t.limits.roll_degrees
                        ));
                        self.touchdown_tick = state.ticks;
                        self.touchdown_position = Some(state.position);
                    }
                }
                Contact::LiftOff { .. } => {
                    if self.touchdown.is_some() {
                        self.bounces += 1;
                    }
                }
                _ => {}
            }
        }
        if self.last_on_ground && !on_ground && self.touchdown.is_some() {
            // Airborne again after touching down.
        }
        self.last_on_ground = on_ground;
        if on_ground && !landable && self.left_runway_tick.is_none() && self.touchdown.is_some() {
            self.left_runway_tick = Some(state.ticks);
            self.left_runway_kt = state.velocity[0].hypot(state.velocity[2]) / 1.68781;
        }
        if state.crashed {
            if self.unsafe_reason.is_none() {
                self.unsafe_reason = Some("crashed".to_owned());
            }
            self.phase = LandingPhase::Done;
            return keys;
        }
        let (along, cross) = self.along_cross(state.position);
        let forward_speed = state.velocity[0].hypot(state.velocity[2]);
        match self.phase {
            LandingPhase::Approach => {
                if self.touchdown.is_some() && on_ground {
                    self.phase = LandingPhase::Rollout;
                    self.stopped_since = None;
                }
                self.max_cross_ft = self.max_cross_ft.max(cross.abs());
                // Glide path: height above the aim plane the slope wants here.
                let target_agl = ((-along).max(0.) * GLIDE_SLOPE_DEG.to_radians().tan()).max(0.);
                let path_err = target_agl - (state.position[1] - self.aim[1] - self.clearance_ft);
                let mut vs_cmd =
                    -forward_speed * GLIDE_SLOPE_DEG.to_radians().tan() + 0.08 * path_err;
                let mut speed_cmd = self.approach_fps;
                if agl < FLARE_HEIGHT_FT && self.variant == LandingVariant::Hard {
                    vs_cmd = -35.;
                } else if agl < FLARE_HEIGHT_FT {
                    // Flare: sink rate falls with height, to a firm but
                    // gentle touchdown.
                    // A float past the touchdown zone is cut short by a growing
                    // sink rate, so the approach always reaches the ground.
                    let float = (along - 500.).max(0.) * 0.004;
                    vs_cmd = -(1.5 + agl * 0.2 + float)
                        .min(forward_speed * GLIDE_SLOPE_DEG.to_radians().tan());
                    speed_cmd = self.approach_fps * 0.97;
                }
                self.max_sink_fps = self.max_sink_fps.max(-state.velocity[1]);
                let err = vs_cmd - state.velocity[1];
                keys.pitch =
                    (0.03 * err - 0.35 * state.pitch_rate.to_degrees() / 10.).clamp(-1., 1.);
                // Wings level over the centre line.
                let cross_gain = if self.variant == LandingVariant::OffRunway {
                    0.
                } else {
                    0.004
                };
                let want_bank =
                    (-cross * cross_gain - self.heading_error(state) * 3.).clamp(-0.25, 0.25);
                keys.roll = ((want_bank - state.bank) * 4.).clamp(-1., 1.) - state.roll_rate * 0.5;
                let throttle = if agl < 12. {
                    0.
                } else {
                    (0.5 + (speed_cmd - state.speed) * 0.02).clamp(0.05, 1.)
                };
                keys.commands = vec![PilotCommand::Throttle(throttle)];
            }
            LandingPhase::Rollout => {
                keys.commands = vec![
                    PilotCommand::Throttle(0.),
                    PilotCommand::Set(Switch::Airbrake, true),
                ];
                keys.yaw = (-cross * 0.002 - self.heading_error(state) * 2.).clamp(-1., 1.);
                // Stick forward while fast, so the wheels stay on the runway.
                keys.pitch = if state.speed > 60. * 1.68781 {
                    -0.3
                } else {
                    0.
                };
                let speed = state.velocity[0].hypot(state.velocity[2]);
                self.max_cross_ft = self.max_cross_ft.max(cross.abs());
                if speed < 1.0 {
                    let since = *self.stopped_since.get_or_insert(state.ticks);
                    if state.ticks >= since + 240 {
                        self.stop_tick = Some(since);
                        self.stop_position = Some(state.position);
                        self.phase = LandingPhase::Done;
                    }
                } else {
                    self.stopped_since = None;
                }
                if state.ticks > self.touchdown_tick + ROLLOUT_LIMIT {
                    self.phase = LandingPhase::Done;
                }
            }
            LandingPhase::Done => {}
        }
        // A missed approach still ends: the ground was never reached.
        if self.phase == LandingPhase::Approach
            && (along > self.length_ft + 3_000. || state.ticks > self.start_tick + 120 * 300)
        {
            self.phase = LandingPhase::Done;
        }
        keys
    }

    fn heading_error(&self, state: &flight::State) -> f64 {
        let want = self.dir[0].atan2(self.dir[1]);
        let mut d = state.velocity[0].atan2(state.velocity[2]) - want;
        while d > std::f64::consts::PI {
            d -= std::f64::consts::TAU;
        }
        while d < -std::f64::consts::PI {
            d += std::f64::consts::TAU;
        }
        d
    }

    pub fn report(&self, state: &flight::State) -> String {
        let touch = match &self.touchdown {
            Some(t) => {
                let (along, cross) =
                    self.along_cross(self.touchdown_position.unwrap_or(state.position));
                format!(
                    "touchdown=true touchdown_tick={} touchdown_forward_kt={:.1} touchdown_side_fps={:.1} touchdown_sink_fps={:.1} touchdown_pitch_deg={:.1} touchdown_bank_deg={:.1} landing_score={} touchdown_past_aim_ft={:.0} touchdown_cross_ft={:.0} touchdown_past_threshold_ft={:.0}",
                    self.touchdown_tick,
                    t.forward_fps / 1.68781,
                    t.side_fps,
                    -t.vertical_fps,
                    t.pitch_deg,
                    t.bank_deg,
                    t.score.map_or("none".to_owned(), |s| s.to_string()),
                    along,
                    cross,
                    along - self.threshold_along,
                )
            }
            None => "touchdown=false".to_owned(),
        };
        let stop = match (self.stop_tick, self.stop_position) {
            (Some(tick), Some(p)) => {
                let (along, cross) = self.along_cross(p);
                format!(
                    "stopped=true stop_tick={tick} stop_past_threshold_ft={:.0} stop_cross_ft={:.0} runway_left_ft={:.0} rollout_start_kt={:.1}",
                    along - self.threshold_along,
                    cross,
                    self.length_ft - (along - self.threshold_along),
                    self.rollout_start_speed_fps / 1.68781
                )
            }
            _ => "stopped=false".to_owned(),
        };
        format!(
            "landing: {touch} {stop} left_runway={} left_runway_kt={:.1} unsafe={} bounces={} crashed={} approach_kt={:.1} limit_forward_kt={:.1} max_cross_ft={:.0} max_sink_fps={:.1} min_agl_ft={:.1} gear={:.2} hook={:.2} brake={:.2} ticks={}",
            self.left_runway_tick.is_some(),
            self.left_runway_kt,
            self.unsafe_reason
                .as_deref()
                .map_or("none".to_owned(), |r| r.replace([' ', ','], "_")),
            self.bounces,
            state.crashed,
            self.approach_fps / 1.68781,
            self.limits_forward_fps / 1.68781,
            self.max_cross_ft,
            self.max_sink_fps,
            self.min_agl_ft,
            state.gear,
            state.hook,
            state.brake,
            state.ticks,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> flight::State {
        let mut s = flight::State::new(&crate::flight::animation_tests::profile(), [0., 5000., 0.])
            .unwrap();
        s.enable_research(1).unwrap();
        s
    }

    #[test]
    fn spin_recovery_gives_up_on_an_aircraft_that_will_not_spin() {
        let mut s = state();
        let mut probe = SpinRecovery::new(&s);
        for _ in 0..(SPIN_ENTRY_LIMIT + 10) {
            let keys = probe.keys(&s);
            assert_eq!((keys.pitch, keys.yaw), (1., 1.));
            s.step(&flight::PilotInput::default(), |_, _| 0.);
            if probe.finished() {
                break;
            }
        }
        assert!(probe.finished());
        assert!(probe.report(&s).contains("entered_tick=never"));
    }

    #[test]
    fn landing_starts_on_the_glide_slope_with_everything_down() {
        let mut s = state();
        let probe = Landing::new(
            &s,
            [0., 0., 0.],
            0.,
            8000.,
            |_, _| 0.,
            LandingVariant::Normal,
        );
        probe.place(&mut s, 0.);
        assert!(s.gear_down && s.flaps_down && s.hook_down);
        // Four nautical miles out on a three degree slope: about 1,270 feet.
        assert!((s.position[2] + LANDING_START_FT - AIM_PAST_THRESHOLD_FT).abs() < 1.);
        assert!(
            (1_250. ..1_310.).contains(&s.position[1]),
            "{}",
            s.position[1]
        );
        assert!(s.velocity[1] < 0.);
        let gear_up = Landing::new(
            &s,
            [0., 0., 0.],
            0.,
            8000.,
            |_, _| 0.,
            LandingVariant::GearUp,
        );
        gear_up.place(&mut s, 0.);
        assert!(!s.gear_down && s.gear == 0.);
        let off = Landing::new(
            &s,
            [0., 0., 0.],
            0.,
            8000.,
            |_, _| 0.,
            LandingVariant::OffRunway,
        );
        off.place(&mut s, 0.);
        assert!((s.position[0].abs() - 1_500.).abs() < 1.);
    }

    #[test]
    fn landing_variants_are_named_by_maneuver() {
        assert_eq!(
            LandingVariant::from_maneuver("land"),
            Some(LandingVariant::Normal)
        );
        assert_eq!(
            LandingVariant::from_maneuver("land-gear-up"),
            Some(LandingVariant::GearUp)
        );
        assert_eq!(LandingVariant::from_maneuver("takeoff"), None);
    }
}

/// Start height of the stall recovery probe, feet above the flat probe ground.
pub const STALL_START_FT: f64 = 15_000.;
/// Airspeed the stall recovery probe starts at, feet per second.
pub const STALL_START_FPS: f64 = 200. * 1.687_81;
/// Ticks of pull allowed before the probe gives up on stalling the aircraft.
const STALL_ENTRY_LIMIT: u64 = 120 * 60;
/// Ticks the pull is held after the first warning, so recovery starts deep in it.
const STALL_HOLD_TICKS: u64 = 120 * 3;
/// Ticks allowed for the recovery.
const STALL_RECOVERY_LIMIT: u64 = 120 * 90;

/// The manual's stall recovery (Flight Manual, Stall Recovery): a pull at low
/// speed until the departure alert sounds, held for three seconds, then the
/// afterburner if there is one, nose down and wings level until the alert
/// clears. `fitted` test harness (agent decision, 2026-09-28).
#[derive(Debug)]
pub struct StallRecovery {
    phase: SpinPhase,
    phase_start: u64,
    alerted_at: Option<u64>,
    recovery_started_at: Option<u64>,
    recovered_at: Option<u64>,
    calm_since: Option<u64>,
    start_altitude_ft: f64,
    min_altitude_ft: f64,
    min_speed_kt: f64,
    worst_alert: u8,
}

impl StallRecovery {
    pub fn new(state: &flight::State) -> Self {
        Self {
            phase: SpinPhase::Enter,
            phase_start: 0,
            alerted_at: None,
            recovery_started_at: None,
            recovered_at: None,
            calm_since: None,
            start_altitude_ft: state.position[1],
            min_altitude_ft: state.position[1],
            min_speed_kt: state.speed / 1.687_81,
            worst_alert: 0,
        }
    }

    pub fn finished(&self) -> bool {
        self.phase == SpinPhase::Done
    }

    pub fn keys(&mut self, state: &flight::State) -> PilotInput {
        use tore_formats::flight_model::departure::DepartureMode;
        let tick = state.ticks;
        self.min_altitude_ft = self.min_altitude_ft.min(state.position[1]);
        self.min_speed_kt = self.min_speed_kt.min(state.speed / 1.687_81);
        let alert = state.stall_alert(f64::MIN);
        let rank = match alert {
            None | Some(DepartureMode::Normal) => 0,
            Some(DepartureMode::Warning) => 1,
            Some(DepartureMode::ExtendedWarning) => 2,
            Some(DepartureMode::Stalled) => 3,
            Some(DepartureMode::Spinning) => 4,
        };
        self.worst_alert = self.worst_alert.max(rank);
        let mut keys = PilotInput::default();
        if state.crashed {
            self.phase = SpinPhase::Done;
            return keys;
        }
        match self.phase {
            SpinPhase::Enter => {
                keys.pitch = 1.;
                keys.commands = vec![PilotCommand::Throttle(0.3)];
                if rank > 0 {
                    self.alerted_at = Some(tick);
                    self.phase = SpinPhase::Hold;
                    self.phase_start = tick;
                } else if tick >= STALL_ENTRY_LIMIT {
                    self.phase = SpinPhase::Done;
                }
            }
            SpinPhase::Hold => {
                keys.pitch = 1.;
                keys.commands = vec![PilotCommand::Throttle(0.3)];
                if tick >= self.phase_start + STALL_HOLD_TICKS {
                    self.phase = SpinPhase::Recover;
                    self.phase_start = tick;
                    self.recovery_started_at = Some(tick);
                }
            }
            SpinPhase::Recover => {
                // Nose down until the nose is well below the horizon, throttle
                // and afterburner in, wings level.
                keys.pitch = if state.pitch.to_degrees() > -25. {
                    -0.6
                } else {
                    0.
                };
                keys.roll = (-state.bank * 2.).clamp(-1., 1.);
                keys.commands = vec![
                    PilotCommand::Throttle(1.),
                    PilotCommand::Set(Switch::Burner, true),
                ];
                if rank == 0 {
                    let since = *self.calm_since.get_or_insert(tick);
                    if tick >= since + 240 {
                        self.recovered_at = Some(since);
                        self.phase = SpinPhase::Done;
                    }
                } else {
                    self.calm_since = None;
                }
                if tick >= self.phase_start + STALL_RECOVERY_LIMIT {
                    self.phase = SpinPhase::Done;
                }
            }
            SpinPhase::Done => {}
        }
        keys
    }

    pub fn report(&self, state: &flight::State) -> String {
        let show = |t: Option<u64>| t.map_or("never".to_owned(), |t| t.to_string());
        format!(
            "stall_recovery: alert_tick={} recovery_started_tick={} recovered_tick={} worst_alert={} altitude_lost_ft={:.0} min_altitude_ft={:.0} min_speed_kt={:.1} crashed={}",
            show(self.alerted_at),
            show(self.recovery_started_at),
            show(self.recovered_at),
            self.worst_alert,
            self.start_altitude_ft - self.min_altitude_ft,
            self.min_altitude_ft,
            self.min_speed_kt,
            state.crashed,
        )
    }
}
