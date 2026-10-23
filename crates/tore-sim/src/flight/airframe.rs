//! Pieces of the hybrid adapter's step that more than one force law uses: the
//! speed envelope's limits, the airframe drag and the ground contact after a
//! move. The conventional g-command law in `State::step_controlled` calls
//! them, and the powered-lift laws ([`super::powered`]) reuse them for their
//! wings, drag and contact (VTOL overhaul design, section 4.2).
//!
//! Each helper is the conventional step's own arithmetic, moved here without
//! reordering a single operation, so every conventional aircraft flies
//! bit for bit as before (the golden fingerprints check it).

use super::{DT, State, fast_side_hold, low_speed_ceiling, trace};
use crate::models::config::Configuration;

/// What the speed envelope allows at the current speed, altitude, flaps and
/// loading: the stall and top speeds, the control authority and the G limits
/// (docs/FLIGHT-MODEL.md, "Envelope limits and loading").
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct EnvelopeLimits {
    /// Stall speed of the 1 G envelope at this altitude, weight-scaled, ft/s.
    pub clean_stall: f64,
    /// Top speed of the 1 G envelope at this altitude, ft/s.
    pub top_speed: f64,
    /// Stall speed with the flaps (hybrid: 1 - 0.25 x flaps), ft/s.
    pub stall: f64,
    /// (airspeed / stall speed) squared, at most 1.
    pub authority: f64,
    /// Fuel plus carried stores over empty weight.
    pub loading: f64,
    /// Final [negative, positive] G limits.
    pub limits: [f64; 2],
    stall_scale: f64,
    flaps: f64,
    no_1g_envelope: bool,
    rows: u32,
    envelope_g: [f64; 2],
    fast_hold: Option<trace::FastSideHold>,
    load_divisor: f64,
    loaded_positive_g: f64,
    extra_g: bool,
    low_speed_ceiling: Option<trace::LowSpeedCeiling>,
}

impl EnvelopeLimits {
    /// The telemetry record of these limits, with the stick and the G it
    /// commanded.
    pub fn trace(&self, stick: f64, stick_g: f64) -> trace::EnvelopeTrace {
        trace::EnvelopeTrace {
            clean_stall_fps: self.clean_stall,
            stall_scale: self.stall_scale,
            stall_fps: self.stall,
            flaps: self.flaps,
            top_speed_fps: self.top_speed,
            no_1g_envelope: self.no_1g_envelope,
            authority: self.authority,
            rows: self.rows,
            envelope_g: self.envelope_g,
            fast_hold: self.fast_hold,
            loading: self.loading,
            load_divisor: self.load_divisor,
            loaded_positive_g: self.loaded_positive_g,
            extra_g: self.extra_g,
            low_speed_ceiling: self.low_speed_ceiling,
            limits_g: self.limits,
            stick,
            stick_g,
        }
    }
}

/// The inputs of [`State::airframe_drag`] that are not the aircraft's own
/// device positions.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct DragInputs {
    /// Current weight, lb.
    pub weight: f64,
    /// The 1 G envelope's top speed, ft/s.
    pub top_speed: f64,
    /// The thrust the clean airframe drag reaches at the top speed, before
    /// lapse, lbf.
    pub reference_thrust: f64,
    /// Thrust lapse with altitude.
    pub lapse: f64,
    /// Fuel plus carried stores over empty weight.
    pub loading: f64,
    /// Load factor for the g-pull drag, G.
    pub load_factor: f64,
    /// Sideslip drag, lbf.
    pub slip_drag: f64,
    /// The envelope drag percent that scales flap and airbrake drag on the
    /// hybrid adapter ([`State::device_drag_percent`]).
    pub drag_percent: f64,
    /// The wheels are on the ground (no gear drag on the hybrid adapter).
    pub wheel_contact: bool,
    /// Regional damage drag increase, percent.
    pub damage_percent: f64,
}

/// What the ground contact after a move needs besides the surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ContactInputs {
    /// Share of the weight the wheels carry, 0..1.
    pub wheel_load: f64,
    /// Runway wind share the tires feel.
    pub runway_wind_fraction: f64,
    /// Where the aircraft was before [`State::advance_position`].
    pub previous_position: [f64; 3],
    /// The parked hold (stationary, throttle closed, wheels loaded) applies,
    /// with the surface wind: the aircraft stays put on its wheels.
    pub parked_in_wind: Option<[f64; 3]>,
}

impl State {
    /// The speed envelope's limits now (see [`EnvelopeLimits`]). Reads the
    /// state's airspeed, altitude, flaps and loading; changes nothing.
    pub(crate) fn envelope_limits(&self, c: &Configuration) -> EnvelopeLimits {
        let env = c.aerodynamics.envelopes.iter().find(|e| e.g == 1).unwrap();
        let stall_scale = self.envelope_scale;
        let env_speeds = env.speeds(self.position[1]);
        let (clean_stall, vmax) = env_speeds.unwrap_or((900., 1000.));
        let stall = if self.research.is_some() {
            clean_stall * (1. - 0.25 * self.flaps)
        } else {
            clean_stall
        };
        let authority = (self.speed / stall.max(1.)).powi(2).clamp(0., 1.);
        let (mut lo, mut hi) = (-1., 1.);
        let mut rows = 0;
        for e in &c.aerodynamics.envelopes {
            if let Some((low, high)) = e.speeds(self.position[1])
                && self.speed >= low
                && self.speed <= high
            {
                lo = f64::min(lo, e.g as f64);
                hi = f64::max(hi, e.g as f64);
                rows += 1;
            }
        }
        // Past the fast edge of every row above 1 G the aircraft keeps that
        // last row's G up to and beyond its top speed, instead of falling to
        // 1 G with no pull left (docs/FLIGHT-MODEL.md, "Envelope limits and
        // loading"). Hybrid only; the legacy compatibility model is unchanged.
        let fast_hold = if self.research.is_some() {
            fast_side_hold(&c.aerodynamics.envelopes, self.position[1], self.speed)
        } else {
            None
        };
        if let Some(hold) = fast_hold {
            hi = hi.max(hold.g);
        }
        // Above the aircraft's own 1 G ceiling the air is too thin to lift its
        // weight (manual p. 90): the available lift falls with the air density
        // above the ceiling, so an aircraft carried past it by a zoom climb
        // sinks back instead of flying on. Fitted rule (agent decision,
        // 2026-09-29), hybrid adapter only: the legacy compatibility model is
        // unchanged. The density ratio is a standard atmosphere estimate; the
        // lookup altitude is clamped to the atmosphere model's range so the
        // thinning holds, finite and small, above 100,000 ft.
        let hybrid = self.research.is_some();
        if hybrid && env_speeds.is_none() {
            let top = env.points.iter().map(|p| p[1]).fold(f64::MIN, f64::max);
            if self.position[1] > top {
                hi *= super::ceiling_lift_ratio(self.position[1], top);
            }
        }
        let envelope_g = [lo, hi];
        let loading = (self.fuel + self.carried_lbs()) / c.mass.empty_lbs;
        let load_factor = 1. + loading * c.aerodynamics.loaded_elevator_percent / 100.;
        hi /= load_factor;
        lo /= load_factor;
        // Loading takes away manoeuvring G, not the aircraft's ability to fly
        // at all: inside the 1 G envelope (manual p. 90, "absolute limits at
        // 1G") an aircraft always keeps 1 G, however full its tanks. Without
        // this the outermost band, where only the 1 G row holds, gave a loaded
        // aircraft less than 1 G and it sank at full power near its top speed
        // and its ceiling. Fitted rule (agent decision, 2026-09-29), hybrid
        // adapter only.
        if hybrid && (rows > 0 || fast_hold.is_some()) && envelope_g[1] >= 1. {
            hi = hi.max(1.);
        }
        let loaded_positive_g = hi;
        // Pull extra G: 9 G whatever the load. Near stall the low-speed ceiling
        // still ramps up to it.
        let extra_g = self.cheats.extra_g;
        if extra_g {
            hi = hi.max(crate::cheats::EXTRA_G);
        }
        let ceiling = if self.research.is_some() {
            low_speed_ceiling(c, self.position[1], self.speed, stall, extra_g)
        } else {
            None
        };
        if let Some(ceiling) = ceiling {
            hi = if extra_g {
                ceiling.limit_g
            } else {
                ceiling.limit_g / load_factor
            };
        }
        EnvelopeLimits {
            clean_stall,
            top_speed: vmax,
            stall,
            authority,
            loading,
            limits: [lo, hi],
            stall_scale,
            flaps: self.flaps,
            no_1g_envelope: env_speeds.is_none(),
            rows,
            envelope_g,
            fast_hold,
            load_divisor: load_factor,
            loaded_positive_g,
            extra_g,
            low_speed_ceiling: ceiling,
        }
    }

    /// The retail drag percent at the current airspeed and altitude against
    /// `top_speed`, 0 to 100: it scales flap lift and, on the hybrid adapter,
    /// flap and airbrake drag.
    pub(crate) fn device_drag_percent(&self, top_speed: f64) -> f64 {
        tore_formats::flight_model::drag_percent(
            (self.speed * 256.) as i32,
            (self.position[1] * 256.) as i32,
            top_speed.round().clamp(1., f64::from(i16::MAX)) as i16,
        )
        .unwrap_or_else(|_| ((self.speed / top_speed.max(1.)) * 100.).round() as i32)
        .clamp(0, 100) as f64
    }

    /// Airframe drag at the current airspeed with the current gear, flaps
    /// and airbrake: the total applied, lbf, and its breakdown. The clean
    /// drag reaches `reference_thrust` at the top speed times the aircraft's
    /// level-speed fraction; loading, the g-pull, devices, sideslip and
    /// regional damage add to it, and the hybrid adapter caps it at the drag
    /// that would stop the aircraft in one step.
    pub(crate) fn airframe_drag(
        &self,
        c: &Configuration,
        d: DragInputs,
    ) -> (f64, trace::DragTrace) {
        let DragInputs {
            weight,
            top_speed: vmax,
            reference_thrust: max_thrust,
            lapse,
            loading,
            load_factor,
            slip_drag,
            drag_percent,
            wheel_contact,
            damage_percent,
        } = d;
        let device_drag_fraction = if self.research.is_some() {
            drag_percent / 100.
        } else {
            1.
        };
        // The drag reaches full thrust at the 1 G top speed times the
        // aircraft's level-speed fraction (1 except for fitted heavies).
        let drag_speed = (vmax * c.aerodynamics.level_speed_fraction).max(100.);
        let drag = slip_drag
            + max_thrust
                * lapse
                * (self.speed / drag_speed).powi(2)
                * (1. + loading * c.aerodynamics.loaded_drag_percent / 100.)
            + weight
                * (c.aerodynamics.g_pull_drag_f8 * (load_factor.abs() - 1.).max(0.)
                    + c.native.drag.gear as f64
                        * self.gear
                        * f64::from(!(self.research.is_some() && wheel_contact))
                    + c.native.drag.flaps as f64 * self.flaps * device_drag_fraction
                    + c.native.drag.airbrake as f64 * self.brake * device_drag_fraction)
                / 256.;
        let undamaged_drag = drag;
        let drag = drag * (1. + damage_percent / 100.);
        let uncapped_drag = drag;
        let drag_cap = if self.research.is_some() {
            Some(weight * self.speed / 32.174 / DT)
        } else {
            None
        };
        let drag = drag_cap.map_or(drag, |cap| drag.min(cap));
        let gear_on_wheels = self.research.is_some() && wheel_contact;
        // Display breakdown of the drag above, from the same inputs.
        let airframe_drag = max_thrust * lapse * (self.speed / drag_speed).powi(2);
        let breakdown = trace::DragTrace {
            total_lbf: drag,
            undamaged_lbf: undamaged_drag,
            uncapped_lbf: uncapped_drag,
            cap_lbf: drag_cap,
            airframe_lbf: airframe_drag,
            load_lbf: airframe_drag * loading * c.aerodynamics.loaded_drag_percent / 100.,
            pull_lbf: weight * c.aerodynamics.g_pull_drag_f8 * (load_factor.abs() - 1.).max(0.)
                / 256.,
            gear_lbf: weight * c.native.drag.gear as f64 * self.gear * f64::from(!gear_on_wheels)
                / 256.,
            flaps_lbf: weight * c.native.drag.flaps as f64 * self.flaps * device_drag_fraction
                / 256.,
            airbrake_lbf: weight
                * c.native.drag.airbrake as f64
                * self.brake
                * device_drag_fraction
                / 256.,
            slip_lbf: slip_drag,
            damage_percent,
            gear_on_wheels,
            device_fraction: device_drag_fraction,
        };
        (drag, breakdown)
    }

    /// Moves the aircraft by its velocity for one tick. Returns where it was.
    pub(crate) fn advance_position(&mut self) -> [f64; 3] {
        let previous_position = self.position;
        for i in 0..3 {
            self.position[i] += self.velocity[i] * DT;
        }
        previous_position
    }

    /// Ground contact after [`Self::advance_position`] over `surface`: the
    /// hybrid adapter's gear, landing and crash rules with `research` (taken
    /// out of the state by the caller, and put back here), or the legacy
    /// adapter's floor when there is none.
    pub(crate) fn finish_contact(
        &mut self,
        research: Option<crate::research::Research>,
        c: &Configuration,
        surface: crate::research::Surface,
        contact: ContactInputs,
    ) {
        let ContactInputs {
            wheel_load,
            runway_wind_fraction,
            previous_position,
            parked_in_wind,
        } = contact;
        if let Some(mut r) = research {
            r.contact(
                self,
                surface,
                c,
                wheel_load,
                runway_wind_fraction,
                previous_position,
            );
            if let Some(air_wind) = parked_in_wind
                && r.on_ground
            {
                self.position[0] = previous_position[0];
                self.position[2] = previous_position[2];
                self.velocity[0] = 0.;
                self.velocity[2] = 0.;
                self.speed = air_wind[0].hypot(air_wind[2]);
            }
            self.research = Some(r);
            return;
        }
        let floor = surface.height + c.equipment.ground_clearance_ft;
        if self.position[1] <= floor && self.cheats.no_crashes {
            self.trace.0.contact = Some(trace::Contact::LegacyFloor { bounced: true });
            self.ricochet(floor);
        } else if self.position[1] <= floor {
            self.trace.0.contact = Some(trace::Contact::LegacyFloor { bounced: false });
            self.position[1] = floor;
            self.crashed = true;
            self.speed = 0.;
            self.velocity = [0.; 3];
            self.vertical_speed = 0.;
            self.engine = false;
            self.burner = false;
        }
    }
}
