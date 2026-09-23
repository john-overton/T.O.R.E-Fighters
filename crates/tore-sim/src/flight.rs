//! Deterministic 120 Hz free-flight adapter. PT facts are recovered; integration is authored.
use crate::attitude::{Basis, cross, dot, unit};
use crate::models::{Conditions, FlightModel};
use tore_formats::aircraft::Aircraft;
pub use tore_input::{PilotCommand, PilotInput, Switch};
pub const DT: f64 = 1.0 / 120.0;
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    model: crate::models::AircraftModel,
    pub research: Option<crate::research::Research>,
    pub native: Option<crate::native::Native>,
    pub position: [f64; 3],
    pub yaw: f64,
    pub pitch: f64,
    pub bank: f64,
    pub speed: f64,
    pub velocity: [f64; 3],
    pub roll_rate: f64,
    pub pitch_rate: f64,
    /// Additional body roll/pitch/yaw rates from low-speed powered control.
    pub auxiliary_rates: [f64; 3],
    pub vertical_speed: f64,
    /// Achieved aerodynamic normal load along aircraft up, excluding gravity/contact.
    pub g: f64,
    pub maneuver: crate::telemetry::Maneuver,
    lift_g: f64,
    pub throttle: f64,
    pub fuel: f64,
    pub systems: crate::aircraft_systems::Systems,
    pub payload_lbs: f64,
    pub engine: bool,
    pub burner: bool,
    pub exhaust: f64,
    pub rudder: f64,
    pub elevator: f64,
    pub aileron: f64,
    pub gear: f64,
    pub flaps: f64,
    pub brake: f64,
    pub hook: f64,
    /// Fitted presentation only, no flight-force or launch gate coupling.
    pub bay: f64,
    pub bay_open: bool,
    pub bay_auto_open: bool,
    pub gear_down: bool,
    pub flaps_down: bool,
    pub brake_out: bool,
    pub hook_down: bool,
    pub radar: bool,
    pub jammer: bool,
    /// Player sensor controls: scope channel, display range and history.
    pub sensors: crate::sensors::Controls,
    /// Combat presentation health, no additional flight-force coupling.
    pub damage_fraction: f64,
    /// Fitted localized damage body pair selected by combat impact location.
    pub damage_variant: Option<usize>,
    pub damage_regions: [f64; crate::combat::live::DAMAGE_SECTIONS],
    pub autopilot: crate::autopilot::Autopilot,
    pub crashed: bool,
    pub wreck: Option<crate::wreck::Wreck>,
    pub ticks: u64,
    /// Player-only session cheats. The native research path ignores them.
    pub cheats: crate::cheats::Cheats,
}
impl State {
    pub fn new(a: &Aircraft, position: [f64; 3]) -> tore_formats::Result<Self> {
        Ok(Self::from_model(
            crate::models::AircraftModel::for_aircraft(a)?,
            position,
        ))
    }
    /// Actual support state, including the legacy adapter's wheel/CG floor.
    pub fn supported_at(&self, height: f64) -> bool {
        if self.native.is_some() || self.crashed {
            return false;
        }
        self.research.as_ref().map_or_else(
            || {
                self.position[1]
                    <= height + self.model.configuration().equipment.ground_clearance_ft + 1e-6
            },
            |research| research.on_ground,
        )
    }
    /// Shared presentation signal. Legacy has a fitted speed warning, not a spin state.
    pub fn stall_alert(
        &self,
        ground_height: f64,
    ) -> Option<tore_formats::flight_model::departure::DepartureMode> {
        use tore_formats::flight_model::departure::DepartureMode;
        if self.crashed
            || self.position[1]
                <= ground_height + self.model.configuration().equipment.ground_clearance_ft
        {
            return None;
        }
        let mode = if let Some(n) = &self.native {
            n.state.as_ref().map(|s| s.departure.departure.mode)?
        } else if let Some(r) = &self.research {
            if r.on_ground {
                return None;
            }
            r.departure.mode
        } else {
            let stall = self
                .model
                .configuration()
                .aerodynamics
                .envelopes
                .iter()
                .find(|e| e.g == 1)?
                .speeds(self.position[1])?
                .0;
            if self.speed < stall {
                DepartureMode::Warning
            } else {
                DepartureMode::Normal
            }
        };
        (mode != DepartureMode::Normal).then_some(mode)
    }
    pub fn model(&self) -> &crate::models::AircraftModel {
        &self.model
    }
    /// Configure a model before constructing a flight; live fuel/payload remain state.
    pub fn from_model(model: crate::models::AircraftModel, position: [f64; 3]) -> Self {
        let fuel = model.configuration().mass.internal_fuel_lbs;
        Self {
            model,
            research: None,
            native: None,
            position,
            yaw: 0.3,
            pitch: 0.,
            bank: 0.,
            speed: 450. * 1.68781,
            velocity: Basis::new(0.3, 0., 0.).forward.map(|v| v * 450. * 1.68781),
            roll_rate: 0.,
            pitch_rate: 0.,
            auxiliary_rates: [0.; 3],
            vertical_speed: 0.,
            g: 1.,
            lift_g: 1.,
            maneuver: crate::telemetry::Maneuver::default(),
            throttle: 0.7,
            fuel,
            systems: Default::default(),
            payload_lbs: 0.,
            engine: true,
            burner: false,
            exhaust: 0.,
            rudder: 0.,
            elevator: 0.,
            aileron: 0.,
            gear: 0.,
            flaps: 0.,
            brake: 0.,
            hook: 0.,
            bay: 0.,
            bay_open: false,
            bay_auto_open: false,
            gear_down: false,
            flaps_down: false,
            brake_out: false,
            hook_down: false,
            radar: true,
            jammer: false,
            sensors: crate::sensors::Controls::default(),
            damage_fraction: 0.,
            damage_variant: None,
            damage_regions: [0.; crate::combat::live::DAMAGE_SECTIONS],
            autopilot: Default::default(),
            crashed: false,
            wreck: None,
            ticks: 0,
            cheats: Default::default(),
        }
    }
    /// Fitted creator ground start. The caller verifies the chosen runway surface.
    pub fn start_on_runway(
        &mut self,
        position: [f64; 3],
        heading: f64,
    ) -> tore_formats::Result<()> {
        if self.native.is_some() || self.research.is_none() {
            return Err(std::io::Error::other(
                "Ground start requires the researched flight model; choose Airborne for this adapter.",
            ));
        }
        if position.iter().any(|v| !v.is_finite()) || !heading.is_finite() {
            return Err(std::io::Error::other("Invalid runway start pose"));
        }
        self.position = position;
        self.position[1] += self.model.configuration().equipment.ground_clearance_ft;
        self.yaw = heading;
        self.pitch = 0.;
        self.bank = 0.;
        self.speed = 0.;
        self.velocity = [0.; 3];
        self.vertical_speed = 0.;
        self.roll_rate = 0.;
        self.pitch_rate = 0.;
        self.auxiliary_rates = [0.; 3];
        self.throttle = 0.;
        self.engine = true;
        self.burner = false;
        self.exhaust = 0.;
        self.gear_down = true;
        self.gear = 1.;
        self.flaps_down = true;
        self.flaps = 1.;
        self.brake_out = true;
        self.brake = 1.;
        self.hook_down = false;
        self.hook = 0.;
        self.rudder = 0.;
        self.elevator = 0.;
        self.aileron = 0.;
        self.autopilot = Default::default();
        self.crashed = false;
        self.research.as_mut().unwrap().on_ground = true;
        Ok(())
    }
    /// Interpolate presentation only, leaving fixed-tick state and discrete controls untouched.
    pub fn presented(&self, previous: &Self, alpha: f64) -> Self {
        if self.crashed
            && self
                .wreck
                .as_ref()
                .is_none_or(|w| w.phase != crate::wreck::Phase::Falling)
        {
            return self.clone();
        }
        let alpha = alpha.clamp(0., 1.);
        let lerp = |a: f64, b: f64| a + (b - a) * alpha;
        let mut result = self.clone();
        for i in 0..3 {
            result.position[i] = lerp(previous.position[i], self.position[i]);
        }
        [result.yaw, result.pitch, result.bank] =
            Basis::new(previous.yaw, previous.pitch, previous.bank)
                .blended(Basis::new(self.yaw, self.pitch, self.bank), alpha)
                .angles();
        result.velocity = std::array::from_fn(|i| lerp(previous.velocity[i], self.velocity[i]));
        result.speed = lerp(previous.speed, self.speed);
        result.vertical_speed = lerp(previous.vertical_speed, self.vertical_speed);
        result.g = lerp(previous.g, self.g);
        result.gear = lerp(previous.gear, self.gear);
        result.flaps = lerp(previous.flaps, self.flaps);
        result.brake = lerp(previous.brake, self.brake);
        result.hook = lerp(previous.hook, self.hook);
        result.bay = lerp(previous.bay, self.bay);
        result.exhaust = lerp(previous.exhaust, self.exhaust);
        result.rudder = lerp(previous.rudder, self.rudder);
        result.elevator = lerp(previous.elevator, self.elevator);
        result.aileron = lerp(previous.aileron, self.aileron);
        result
    }
    pub fn wreck_power(&self, engine_count: u8) -> crate::wreck::Power {
        if !self.engine || self.fuel + self.systems.external_lbs() <= 0. {
            return crate::wreck::Power {
                engine_count,
                ..Default::default()
            };
        }
        let c = self.model.configuration();
        let ab = self.burner
            && !self.systems.has(8)
            && self.throttle > c.equipment.afterburner_throttle
            && c.propulsion.afterburner_thrust_lbf > 0.;
        let thrust = if ab {
            c.propulsion.afterburner_thrust_lbf
        } else {
            c.propulsion.military_thrust_lbf * self.throttle
        };
        let flow = if ab {
            c.propulsion.afterburner_fuel_lbs_per_second
        } else {
            c.propulsion.military_fuel_lbs_per_second * self.throttle
        } * self.systems.power_available();
        let mass = c.mass.empty_lbs + self.fuel + self.carried_lbs();
        let lapse = self
            .model
            .response(Conditions {
                altitude_msl_ft: self.position[1],
                tas_fps: self.speed,
                load_factor: self.g,
            })
            .thrust_lapse;
        crate::wreck::Power {
            acceleration: self
                .systems
                .engine
                .thrust_shares(engine_count)
                .map(|share| share * thrust * lapse / mass * 32.174),
            engine_count,
            fuel_seconds: if flow > 0. {
                (self.fuel + self.systems.external_lbs()) / flow
            } else {
                0.
            },
        }
    }
    pub fn airburst(&self) -> bool {
        self.wreck
            .as_ref()
            .is_some_and(|w| w.phase == crate::wreck::Phase::Exploded)
    }
    pub fn ground_impact(&self) -> bool {
        self.wreck
            .as_ref()
            .is_some_and(|w| w.phase == crate::wreck::Phase::Grounded)
    }
    pub fn wreck_gone(&self) -> bool {
        self.airburst() || self.ground_impact()
    }
    fn finish_ground_crash(&mut self, height: f64) {
        if self.crashed
            && self.wreck.is_none()
            && self.position[1] <= height + self.model.configuration().equipment.ground_clearance_ft
        {
            let mut wreck = crate::wreck::Wreck::new(0, self.ticks, [0.; 3]);
            wreck.phase = crate::wreck::Phase::Grounded;
            self.wreck = Some(wreck);
            self.engine = false;
            self.burner = false;
            self.exhaust = 0.;
            self.velocity = [0.; 3];
            self.speed = 0.;
            self.vertical_speed = 0.;
            self.damage_fraction = 1.;
            self.systems.kill_pilot("Pilot killed in ground impact");
        }
    }
    pub fn afterburner_active(&self) -> bool {
        self.engine
            && !self.systems.has(8)
            && self.systems.power_available() > 0.
            && self.model.configuration().propulsion.afterburner_thrust_lbf > 0.
            && self.fuel + self.systems.external_lbs() > 0.
            && self.burner
            && self.throttle > self.model.configuration().equipment.afterburner_throttle
            && !self.crashed
    }
    /// Only F-22 has a reviewed main-bay presentation.
    pub fn bay_available(&self) -> bool {
        matches!(self.model, crate::models::AircraftModel::F22(_))
    }
    /// Hook capability includes explicitly authored concept equipment.
    pub fn hook_available(&self) -> bool {
        self.model.configuration().hook_available
    }
    pub fn command(&mut self, command: PilotCommand) {
        if self.crashed {
            return;
        }
        let (switch, setting) = match command {
            PilotCommand::Throttle(value) => {
                if value.is_finite() && self.systems.controls.throttle_lock.is_none() {
                    self.throttle = value.clamp(0., 1.);
                }
                return;
            }
            PilotCommand::AdjustThrottle(value) => {
                if value.is_finite() && self.systems.controls.throttle_lock.is_none() {
                    self.throttle = (self.throttle + value).clamp(0., 1.);
                }
                return;
            }
            PilotCommand::Toggle(switch) => (switch, None),
            PilotCommand::Set(switch, value) => (switch, Some(value)),
        };
        if switch == Switch::Engine
            && setting.unwrap_or(!self.engine)
            && self.systems.power_available() <= 0.
        {
            self.systems
                .notify("Engine restart unavailable due to damage");
            return;
        }
        if (switch == Switch::Hook && !self.hook_available())
            || (switch == Switch::Bay && !self.bay_available())
            || (switch == Switch::Burner
                && self.model.configuration().propulsion.afterburner_thrust_lbf == 0.)
        {
            return;
        }
        if matches!(switch, Switch::Autopilot | Switch::WaypointAutopilot) {
            if !self.systems.autopilot_available()
                || self.damage_regions[3..].iter().any(|v| *v > 0.)
                || (switch == Switch::WaypointAutopilot && self.systems.has(33))
            {
                self.systems.notify("Autopilot unavailable due to damage");
                self.autopilot.disengage();
                return;
            }
            self.autopilot
                .select(switch, setting, self.yaw, self.position[1]);
            return;
        }
        let target = match switch {
            Switch::Gear => &mut self.gear_down,
            Switch::Flaps => &mut self.flaps_down,
            Switch::Airbrake => &mut self.brake_out,
            Switch::Hook => &mut self.hook_down,
            Switch::Bay => &mut self.bay_open,
            Switch::Engine => &mut self.engine,
            Switch::Burner => &mut self.burner,
            Switch::Radar => &mut self.radar,
            Switch::Jammer => &mut self.jammer,
            Switch::Autopilot | Switch::WaypointAutopilot => unreachable!(),
        };
        *target = setting.unwrap_or(!*target);
    }
    /// Store mass the flight model carries. Ignore weapon weights leaves only
    /// the fuel still in external tanks.
    pub fn carried_lbs(&self) -> f64 {
        if self.cheats.ignore_weapon_weights {
            self.systems.external_lbs()
        } else {
            self.payload_lbs
        }
    }
    /// Runtime fuel debit, with external fuel mass removed from payload as consumed.
    pub(crate) fn consume_fuel(&mut self, pounds: f64) {
        if self.cheats.unlimited_fuel {
            return;
        }
        let before = self.systems.external_lbs();
        self.systems.consume(&mut self.fuel, pounds);
        self.payload_lbs = (self.payload_lbs - before + self.systems.external_lbs()).max(0.);
    }
    fn advance_systems(&mut self, ground: f64) {
        if self.crashed {
            return;
        }
        let restarting = self.systems.engine.flameout > 0.;
        let landed =
            self.research.as_ref().is_some_and(|r| r.on_ground) && self.supported_at(ground);
        // Unlimited fuel also covers damage leaks.
        let mut fuel = self.fuel;
        self.systems.advance(
            self.engine,
            self.throttle,
            self.g,
            self.damage_fraction,
            landed,
            &mut fuel,
        );
        if !self.cheats.unlimited_fuel {
            self.fuel = fuel;
        }
        if restarting && self.systems.engine.flameout == 0. && self.systems.power_available() > 0. {
            self.engine = true;
        }
        if self.systems.power_available() <= 0. {
            self.engine = false;
            self.burner = false;
        }
        if self.systems.fatal() {
            self.crashed = true;
        }
    }
    pub fn step(&mut self, input: &PilotInput, ground: impl Fn(f64, f64) -> f64) {
        self.step_surface(input, |x, z| {
            crate::research::Surface::terrain(ground(x, z))
        });
    }
    /// Mass-only payload API until loadout/weapon release is integrated.
    pub fn set_payload(&mut self, pounds: f64) -> tore_formats::Result<()> {
        if !pounds.is_finite()
            || pounds < 0.
            || pounds + self.fuel + self.model.configuration().mass.empty_lbs
                > self.model.configuration().mass.max_takeoff_lbs
        {
            return Err(std::io::Error::other(
                "payload outside aircraft mass limits",
            ));
        }
        self.payload_lbs = pounds;
        Ok(())
    }
    pub fn enable_native(
        &mut self,
        tables: std::sync::Arc<crate::native::Tables>,
        seed: i32,
    ) -> tore_formats::Result<()> {
        self.model.configuration().joined_native()?;
        if self.research.is_some() {
            return Err(std::io::Error::other(
                "native and hybrid modes are mutually exclusive",
            ));
        }
        self.native = Some(crate::native::Native::new(tables, seed)?);
        Ok(())
    }
    pub fn native_fault(&self) -> Option<&str> {
        self.native.as_ref().and_then(|n| n.fault.as_deref())
    }
    pub fn enable_research(&mut self, seed: i32) -> tore_formats::Result<()> {
        if self.native.is_some() {
            return Err(std::io::Error::other(
                "native and hybrid modes are mutually exclusive",
            ));
        }
        self.research = Some(crate::research::Research::new(seed)?);
        Ok(())
    }
    /// Authored coupling of recovered disturbance rates. Rotate the body basis
    /// without Euler singularities; velocity remains independent. Native movement
    /// and display-angle coupling/rounding are still a separate acceptance gate.
    pub fn apply_turbulence(&mut self, d: crate::turbulence::Disturbance) {
        if self.crashed || self.native.is_some() {
            return;
        }
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let rotation = std::array::from_fn(|i| {
            (basis.up[i] * d.yaw - basis.right[i] * d.pitch - basis.forward[i] * d.roll) * DT
        });
        [self.yaw, self.pitch, self.bank] = basis.rotated(rotation).angles();
        self.position[1] += d.vertical_fps * DT;
    }

    pub fn step_surface(
        &mut self,
        input: &PilotInput,
        ground: impl Fn(f64, f64) -> crate::research::Surface,
    ) {
        if self.crashed {
            self.autopilot.disengage();
            self.finish_ground_crash(ground(self.position[0], self.position[2]).height);
            if self.wreck_gone() {
                return;
            }
            if self.wreck.is_none() {
                let mut wreck = crate::wreck::Wreck::new(
                    0,
                    self.ticks,
                    [-self.pitch_rate, 0., -self.roll_rate],
                );
                wreck.power = self.wreck_power(self.systems.engine.count());
                self.wreck = Some(wreck);
            }
            self.ticks += 1;
            let mut basis = Basis::new(self.yaw, self.pitch, self.bank);
            let wreck = self.wreck.as_mut().unwrap();
            let before = wreck.power.fuel_seconds;
            wreck.step(
                &mut self.position,
                &mut self.velocity,
                &mut basis,
                |x, z| ground(x, z).height,
            );
            let remaining = wreck.power.fuel_seconds;
            let falling = wreck.phase == crate::wreck::Phase::Falling;
            let phase = wreck.phase;
            if phase == crate::wreck::Phase::Grounded {
                self.systems.kill_pilot("Pilot killed in ground impact");
            } else if phase == crate::wreck::Phase::Exploded {
                self.systems
                    .kill_pilot("Pilot killed in aircraft explosion");
            }
            if before > 0. && remaining < before {
                self.consume_fuel(
                    (self.fuel + self.systems.external_lbs()) * (before - remaining) / before,
                );
            }
            if !falling || remaining <= 0. {
                self.engine = false;
                self.burner = false;
                self.exhaust = 0.;
            }
            [self.yaw, self.pitch, self.bank] = basis.angles();
            self.speed = dot(self.velocity, self.velocity).sqrt();
            self.vertical_speed = self.velocity[1];
            return;
        }
        let mut input = input.bounded();
        input.commands.retain(|command| {
            if matches!(
                command,
                PilotCommand::Toggle(Switch::Autopilot | Switch::WaypointAutopilot)
                    | PilotCommand::Set(Switch::Autopilot | Switch::WaypointAutopilot, _)
            ) {
                self.command(*command);
                false
            } else {
                true
            }
        });
        if !self.systems.autopilot_available()
            || self.damage_regions[3..].iter().any(|v| *v > 0.)
            || (self.systems.has(33) && self.autopilot.mode() == crate::autopilot::Mode::Waypoint)
        {
            self.autopilot.disengage();
        }
        let mut autopilot = std::mem::take(&mut self.autopilot);
        autopilot.apply(
            self,
            ground(self.position[0], self.position[2]).height,
            &mut input,
        );
        self.autopilot = autopilot;
        self.step_controlled(&input, &ground);
        self.finish_ground_crash(ground(self.position[0], self.position[2]).height);
        if self.crashed
            || self.position[1]
                <= ground(self.position[0], self.position[2]).height
                    + self.model.configuration().equipment.ground_clearance_ft
        {
            self.autopilot.disengage();
        }
    }

    fn step_controlled(
        &mut self,
        input: &PilotInput,
        ground: impl Fn(f64, f64) -> crate::research::Surface,
    ) {
        if self.crashed || self.native_fault().is_some() {
            return;
        }
        let mut input = input.bounded();
        self.advance_systems(ground(self.position[0], self.position[2]).height);
        [input.pitch, input.roll, input.yaw] = self.systems.controls(
            [input.pitch, input.roll, input.yaw],
            [self.elevator, self.aileron, self.rudder],
            self.ticks,
        );
        let regional = crate::aircraft_systems::regional_effects(self.damage_regions);
        let aero = regional.commands([input.pitch, input.roll, input.yaw]);
        if self.systems.controls.throttle_lock.is_some() {
            input.throttle_rate = 0.;
        }
        let input = &input;
        if self.native.is_some() {
            if self.crashed || self.native_fault().is_some() {
                return;
            }
            let mut candidate = self.clone();
            match crate::native::step(&mut candidate, input, ground) {
                Ok(()) => *self = candidate,
                Err(e) => self.native.as_mut().unwrap().fault = Some(e.to_string()),
            }
            return;
        }
        let input = input.bounded();
        if let Some(value) = input.throttle {
            self.command(PilotCommand::Throttle(value));
        }
        for command in &input.commands {
            self.command(*command);
        }
        let model = self.model.clone();
        let c = model.configuration();
        if self.crashed {
            return;
        }
        // Fitted tire grip suppresses wind coupling at low ground speed. Once
        // airborne, wind remains pure advection and does not change airspeed.
        let initial_surface = ground(self.position[0], self.position[2]);
        let wheel_contact = self.research.as_ref().is_some_and(|r| r.on_ground);
        let horizontal_speed = self.velocity[0].hypot(self.velocity[2]);
        let static_attitude_hold = wheel_contact
            && horizontal_speed <= 1e-9
            && self.throttle <= 0.
            && input.pitch.abs() <= 1e-9
            && input.roll.abs() <= 1e-9
            && input.yaw.abs() <= 1e-9;
        let runway_wind_fraction = if wheel_contact {
            let assessment = crate::runway_wind::assessment(
                c.mass.max_takeoff_lbs,
                initial_surface.wind,
                self.yaw,
            )
            .expect("validated aircraft mass and surface wind");
            assessment
                .crosswind_fraction
                .max(assessment.tailwind_fraction)
                * crate::runway_wind::ground_motion_fraction(horizontal_speed)
        } else {
            0.
        };
        let air_wind = initial_surface.wind;
        for (v, w) in self.velocity.iter_mut().zip(air_wind) {
            *v -= w;
        }
        if self.research.is_some() {
            self.speed = dot(self.velocity, self.velocity).sqrt();
        }
        self.ticks += 1;
        self.throttle = (self.throttle
            + input.throttle_rate * DT * c.equipment.throttle_rate_per_second)
            .clamp(0., 1.);
        for (v, on, movable) in [
            (&mut self.gear, self.gear_down, self.systems.device_free(16)),
            (
                &mut self.flaps,
                self.flaps_down,
                self.systems.device_free(17),
            ),
            (
                &mut self.brake,
                self.brake_out,
                self.systems.device_free(18),
            ),
            (
                &mut self.hook,
                self.hook_down,
                self.systems.fluids.hydraulic > 0.,
            ),
        ] {
            if !movable {
                continue;
            }
            *v = (*v + (if on { 1. } else { -1. }) * DT / c.equipment.deployment_seconds)
                .clamp(0., 1.);
        }
        let bay_target = f64::from(self.bay_available() && (self.bay_open || self.bay_auto_open));
        self.bay += (bay_target - self.bay).clamp(-DT, DT);
        if self.fuel + self.systems.external_lbs() <= 0. {
            self.engine = false;
            self.burner = false;
        }
        let ab = self.afterburner_active() && c.propulsion.afterburner_thrust_lbf > 0.;
        let target = f64::from(ab);
        self.exhaust = (self.exhaust
            + (target - self.exhaust).clamp(
                -DT / c.equipment.exhaust_seconds,
                DT / c.equipment.exhaust_seconds,
            ))
        .clamp(0., 1.);
        if self.systems.fluids.hydraulic > 0. {
            self.rudder += (input.yaw - self.rudder) * (DT / c.equipment.control_seconds);
            self.elevator += (input.pitch - self.elevator) * (DT / c.equipment.control_seconds);
            self.aileron += (input.roll - self.aileron) * (DT / c.equipment.control_seconds);
        }
        let rate = if ab {
            c.propulsion.afterburner_fuel_lbs_per_second
        } else {
            c.propulsion.military_fuel_lbs_per_second * self.throttle
        };
        if self.engine {
            self.consume_fuel(rate * DT);
        }
        let env = c.aerodynamics.envelopes.iter().find(|e| e.g == 1).unwrap();
        let (clean_stall, vmax) = env.speeds(self.position[1]).unwrap_or((900., 1000.));
        let stall = if self.research.is_some() {
            clean_stall * (1. - 0.25 * self.flaps)
        } else {
            clean_stall
        };
        let authority = (self.speed / stall.max(1.)).powi(2).clamp(0., 1.);
        let (mut lo, mut hi) = (-1., 1.);
        for e in &c.aerodynamics.envelopes {
            if let Some((low, high)) = e.speeds(self.position[1])
                && self.speed >= low
                && self.speed <= high
            {
                lo = f64::min(lo, e.g as f64);
                hi = f64::max(hi, e.g as f64);
            }
        }
        let loading = (self.fuel + self.carried_lbs()) / c.mass.empty_lbs;
        let load_factor = 1. + loading * c.aerodynamics.loaded_elevator_percent / 100.;
        hi /= load_factor;
        lo /= load_factor;
        // Pull extra G: 9 G whatever the load. Near stall the low-speed ceiling
        // still ramps up to it.
        let extra_g = self.cheats.extra_g;
        if extra_g {
            hi = hi.max(crate::cheats::EXTRA_G);
        }
        if self.research.is_some()
            && let Some(continuous) =
                low_speed_positive_g_ceiling(c, self.position[1], self.speed, stall, extra_g)
        {
            hi = if extra_g {
                continuous
            } else {
                continuous / load_factor
            };
        }
        let mut command =
            (1. + aero[0] * if aero[0] > 0. { hi - 1. } else { 1. - lo }).clamp(lo, hi) * authority;
        let drag_percent = tore_formats::flight_model::drag_percent(
            (self.speed * 256.) as i32,
            (self.position[1] * 256.) as i32,
            vmax.round().clamp(1., f64::from(i16::MAX)) as i16,
        )
        .unwrap_or_else(|_| ((self.speed / vmax.max(1.)) * 100.).round() as i32)
        .clamp(0, 100) as f64;
        if self.research.is_some() {
            let scaled_flap_lift = drag_percent * c.aerodynamics.flaps_lift_f8 / 100.;
            let flap_lift_f8 = if wheel_contact {
                scaled_flap_lift * 0.5
            } else {
                c.aerodynamics.flaps_lift_f8 * (1. - self.gear) + scaled_flap_lift * self.gear
            };
            command *= 1. + self.flaps * flap_lift_f8 / 256.;
        }
        if self.systems.has(25) {
            command *= 0.5;
        }
        command *= regional.lift;
        let requested_g = command;
        if let Some(r) = &mut self.research {
            r.advance(
                c,
                self.speed,
                stall,
                aero[0],
                aero[2],
                self.throttle,
                self.bank,
                self.roll_rate,
                dot(
                    Basis::new(self.yaw, self.pitch, self.bank).forward,
                    unit(self.velocity),
                ),
                !self.cheats.no_spins,
            );
            command *= 1. - 0.85 * r.spin_blend(c);
        }
        let severity = self.research.as_ref().map_or(0, |r| r.severity_f8);
        let stalled = self.research.as_ref().is_some_and(|r| r.stall_active);
        let (mut control_scale, lift_scale) = if stalled {
            let (controls, lift) =
                tore_formats::flight_model::departure::stall_authority(severity, [1024; 3], 256);
            (controls.map(|v| v as f64 / 1024.), lift as f64 / 256.)
        } else {
            ([1.; 3], 1.)
        };
        let spin_fraction = self.research.as_ref().map_or(0., |r| r.spin_blend(c));
        let spin_controls = self.research.as_ref().map_or(1., |r| {
            r.surface_effectiveness(
                c,
                dot(
                    Basis::new(self.yaw, self.pitch, self.bank).forward,
                    unit(self.velocity),
                ),
            )
        });
        control_scale[0] *= spin_controls;
        control_scale[2] *= spin_controls;
        self.lift_g += (command - self.lift_g) * (DT * 4.).min(1.);
        let basis = Basis::new(self.yaw, self.pitch, self.bank);
        let roll_limit = if self.research.is_some() {
            c.aerodynamics.roll_limit_rad_per_second.clamp(0.1, 6.)
        } else {
            c.tuning.legacy_roll_limit_rad_per_second
        };
        let tuning = model.tuning();
        let roll_command = aero[1] * roll_limit * authority * control_scale[0];
        // Aircraft-owned source controls for the new ports. Existing adapters remain selected as before.
        if let Some(profile) = c.controls {
            use crate::models::handling::{approach, auxiliary_authority};
            self.roll_rate = approach(
                self.roll_rate,
                aero[1],
                if self.research.is_some() {
                    profile.hybrid_roll.unwrap_or(profile.roll)
                } else {
                    profile.roll
                },
                (self.speed / (2. * stall.max(1.))).clamp(0., 1.) * control_scale[0],
                DT,
            );
            let on_ground = self.position[1]
                <= ground(self.position[0], self.position[2]).height
                    + c.equipment.ground_clearance_ft;
            let powered = self.engine && self.fuel > 0.;
            let scale = auxiliary_authority(self.speed, self.throttle, powered, on_ground);
            for (i, command) in [aero[1], aero[0], aero[2]].into_iter().enumerate() {
                self.auxiliary_rates[i] = if !powered || on_ground {
                    0.
                } else {
                    approach(
                        self.auxiliary_rates[i],
                        command * scale,
                        profile.auxiliary[i],
                        1.,
                        DT,
                    )
                };
            }
        } else {
            self.roll_rate += (roll_command - self.roll_rate) * (DT / tuning.roll_response_seconds);
        }
        let normal_pitch =
            (requested_g - basis.up[1]) * control_scale[1] * 32.174 / self.speed.max(60.);
        let spin_pitch = 40_f64.to_radians() * aero[0] * spin_controls * authority;
        let pitch_command = normal_pitch * (1. - spin_fraction) + spin_pitch * spin_fraction;
        self.pitch_rate += (pitch_command - self.pitch_rate) * (DT / tuning.pitch_response_seconds);
        // Authored trim target, not decoded gpullAOA units. Preserve a positive
        // nose/flight-path separation under load rather than aligning to zero AoA.
        let direction = unit(self.velocity);
        let along = dot(basis.up, direction);
        let lift_axis = unit(std::array::from_fn(|i| basis.up[i] - along * direction[i]));
        let response = model.response(Conditions {
            altitude_msl_ft: self.position[1],
            tas_fps: self.speed,
            load_factor: self.lift_g,
        });
        let alpha = if self.research.is_some()
            && let Some(blend) = low_speed_alignment_fraction(self.speed, stall, clean_stall)
        {
            let low_speed = (tuning.trim_degrees + 8. * self.elevator)
                .clamp(0., 10.)
                .to_radians();
            low_speed + (response.trim_aoa_rad - low_speed) * blend
        } else {
            response.trim_aoa_rad
        };
        let desired_nose =
            std::array::from_fn(|i| direction[i] * alpha.cos() + lift_axis[i] * alpha.sin());
        let alignment = cross(basis.forward, desired_nose);
        // The gravity component across aircraft-right contributes to body yaw
        // as the flight path turns. Pitch alone misses this during a banked pull.
        let turn_yaw = -basis.right[1] * 32.174 / self.speed.max(60.);
        let mut rotation = std::array::from_fn(|i| {
            DT * (-basis.right[i] * (self.pitch_rate + self.auxiliary_rates[1])
                - basis.forward[i] * (self.roll_rate + self.auxiliary_rates[0])
                + basis.up[i]
                    * (turn_yaw
                        + (self.rudder * regional.authority[2] + regional.yaw_bias)
                            * control_scale[2]
                            * tuning.rudder_rate
                            * authority
                        + self.auxiliary_rates[2])
                + alignment[i] * tuning.alignment_rate * authority)
        });
        if let Some(r) = &self.research {
            if r.on_ground {
                for (i, v) in rotation.iter_mut().enumerate() {
                    *v += basis.up[i] * self.rudder * 0.3 * (self.speed / 40.).clamp(0., 1.) * DT;
                }
            }
            for (i, v) in rotation.iter_mut().enumerate() {
                *v += DT * basis.up[i] * r.spin_rate;
            }
        }
        if static_attitude_hold {
            rotation = [0.; 3];
        }
        self.maneuver = crate::telemetry::Maneuver {
            tick: self.ticks,
            commanded_g: requested_g,
            lift_g: self.lift_g * lift_scale,
            body_rates_rad_per_second: [
                -dot(rotation, basis.forward) / DT,
                -dot(rotation, basis.right) / DT,
                dot(rotation, basis.up) / DT,
            ],
            rudder_command: aero[2],
            rudder_deflection: self.rudder,
            effective_rudder: (self.rudder * regional.authority[2] + regional.yaw_bias)
                * control_scale[2],
            departure: self.research.as_ref().map(|r| r.departure.mode),
            stall_severity_f8: severity,
            ..Default::default()
        };
        let basis = basis.rotated(rotation);
        [self.yaw, self.pitch, self.bank] = basis.angles();
        let max_thrust = c
            .propulsion
            .afterburner_thrust_lbf
            .max(c.propulsion.military_thrust_lbf);
        let lapse = response.thrust_lapse;
        let thrust = if self.engine {
            if ab {
                max_thrust
            } else {
                c.propulsion.military_thrust_lbf * self.throttle
            }
        } else {
            0.
        } * lapse
            * self.systems.power_available();
        let weight = c.mass.empty_lbs + self.fuel + self.carried_lbs();
        // Drag normalized against the source 1G upper envelope. This is not the native force law.
        // Fitted symmetric slip loss, based on air-relative motion rather than
        // rudder command or the native display-slip offset. Aircraft-owned tuning.
        let slip_fraction = dot(unit(self.velocity), basis.right);
        let slip_drag = weight * tuning.sideslip_drag * slip_fraction.powi(2) * authority;
        let device_drag_fraction = if self.research.is_some() {
            drag_percent / 100.
        } else {
            1.
        };
        let drag = slip_drag
            + max_thrust
                * lapse
                * (self.speed / vmax.max(100.)).powi(2)
                * (1. + loading * c.aerodynamics.loaded_drag_percent / 100.)
            + weight
                * (c.aerodynamics.g_pull_drag_f8 * (self.lift_g.abs() - 1.).max(0.)
                    + c.native.drag.gear as f64
                        * self.gear
                        * f64::from(!(self.research.is_some() && wheel_contact))
                    + c.native.drag.flaps as f64 * self.flaps * device_drag_fraction
                    + c.native.drag.airbrake as f64 * self.brake * device_drag_fraction)
                / 256.;
        let drag = drag * (1. + regional.drag_percent / 100.);
        let drag = if self.research.is_some() {
            drag.min(weight * self.speed / 32.174 / DT)
        } else {
            drag
        };
        let direction = unit(self.velocity);
        let along = dot(basis.up, direction);
        let lift = unit(std::array::from_fn(|i| basis.up[i] - along * direction[i]));
        // Specific force along body-up. Thrust is body-forward; drag can have
        // a normal component when attitude differs from the air-relative path.
        self.g = dot(lift, basis.up) * self.lift_g * lift_scale
            - dot(direction, basis.up) * drag / weight;
        let support_g = basis.forward[1] * thrust / weight - direction[1] * drag / weight
            + lift[1] * self.lift_g * lift_scale;
        let wheel_load_fraction = (1. - support_g).clamp(0., 1.);
        let static_hold = wheel_contact
            && horizontal_speed <= 1e-9
            && self.throttle <= 0.
            && wheel_load_fraction > 0.02;
        self.maneuver.achieved_g = self.g;
        for i in 0..3 {
            self.velocity[i] += (basis.forward[i] * thrust / weight * 32.174
                - direction[i] * drag / weight * 32.174
                + lift[i] * self.lift_g * lift_scale * 32.174
                - if i == 1 { 32.174 } else { 0. })
                * DT;
        }
        self.speed = dot(self.velocity, self.velocity).sqrt();
        if self.speed > 6000. {
            self.velocity = self.velocity.map(|v| v * 6000. / self.speed);
            self.speed = 6000.;
        }
        for (v, w) in self.velocity.iter_mut().zip(air_wind) {
            *v += w;
        }
        if static_hold {
            self.velocity[0] = 0.;
            self.velocity[2] = 0.;
            self.speed = self.velocity[1].abs();
        }
        self.vertical_speed = self.velocity[1];
        let previous_position = self.position;
        for i in 0..3 {
            self.position[i] += self.velocity[i] * DT;
        }
        let surface = ground(self.position[0], self.position[2]);
        if let Some(mut r) = self.research.take() {
            r.contact(
                self,
                surface,
                c,
                wheel_load_fraction,
                runway_wind_fraction,
                previous_position,
            );
            if static_hold && r.on_ground {
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
        if self.position[1] <= floor {
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

fn low_speed_positive_g_ceiling(
    c: &crate::models::config::Configuration,
    altitude_ft: f64,
    speed_fps: f64,
    effective_stall_fps: f64,
    extra_g: bool,
) -> Option<f64> {
    let next = c
        .aerodynamics
        .envelopes
        .iter()
        .filter(|envelope| envelope.g > 1)
        .filter_map(|envelope| {
            envelope
                .speeds(altitude_ft)
                .map(|speeds| (speeds.0, envelope.g))
        })
        .filter(|(minimum, _)| *minimum > effective_stall_fps)
        .min_by(|a, b| a.0.total_cmp(&b.0))?;
    if speed_fps >= next.0 {
        return None;
    }
    let fraction =
        ((speed_fps - effective_stall_fps) / (next.0 - effective_stall_fps)).clamp(0., 1.);
    let top = if extra_g {
        f64::from(next.1).max(crate::cheats::EXTRA_G)
    } else {
        f64::from(next.1)
    };
    Some(1. + fraction * (top - 1.))
}

fn low_speed_alignment_fraction(
    speed_fps: f64,
    effective_stall_fps: f64,
    clean_stall_fps: f64,
) -> Option<f64> {
    let end = clean_stall_fps * 2.;
    if speed_fps >= end || end <= effective_stall_fps {
        None
    } else {
        Some(((speed_fps - effective_stall_fps) / (end - effective_stall_fps)).clamp(0., 1.))
    }
}

pub struct Clock {
    pub remainder: f64,
}
impl Clock {
    pub fn steps(&mut self, seconds: f64) -> usize {
        self.steps_scaled(seconds, 1.)
    }
    pub fn steps_scaled(&mut self, seconds: f64, scale: f64) -> usize {
        self.remainder += seconds.clamp(0., 0.25) * scale.clamp(0.5, 8.);
        let n = ((self.remainder + 1e-10) / DT).floor() as usize;
        self.remainder -= n as f64 * DT;
        n
    }
}
#[cfg(test)]
mod tests {
    use super::integration_tests::profile;
    use super::*;
    #[test]
    fn airborne_player_ground_impact_is_terminal_and_kills_pilot_without_a_poll() {
        let mut s = State::new(&profile(), [0., 8.1, 0.]).unwrap();
        s.crashed = true;
        s.velocity = [0., -100., 0.];
        for _ in 0..120 {
            s.step(&PilotInput::default(), |_, _| 0.);
            if s.ground_impact() {
                break;
            }
        }
        assert!(s.ground_impact() && s.wreck_gone());
        assert!(s.systems.pilot.dead);
        assert!(!s.engine && !s.burner);
        assert_eq!(s.wreck.as_ref().unwrap().polls, 0);
        let impact = s.clone();
        s.step(&PilotInput::default(), |_, _| 0.);
        assert_eq!(s, impact);
    }
    #[test]
    fn direct_ground_crash_finishes_in_the_same_tick() {
        let mut s = State::new(&profile(), [0., 1., 0.]).unwrap();
        s.velocity = [0., -100., 0.];
        s.step(&PilotInput::default(), |_, _| 0.);
        assert!(s.crashed && s.ground_impact() && s.systems.pilot.dead);
        assert_eq!(s.damage_fraction, 1.);
    }
    #[test]
    fn destroyed_ownship_tumbles_with_surviving_engine_thrust_and_ignores_controls() {
        let mut s = State::new(&profile(), [0., 10000., 0.]).unwrap();
        s.systems = crate::aircraft_systems::Systems::new(2, [0.; 9]);
        s.systems.hit(9, s.throttle);
        let before = s.clone();
        s.crashed = true;
        let mut replica = s.clone();
        s.command(PilotCommand::Throttle(0.));
        s.command(PilotCommand::Set(Switch::Engine, false));
        assert_eq!(s, replica, "direct UI commands cannot control a wreck");
        for _ in 0..119 {
            s.step(
                &PilotInput {
                    pitch: 1.,
                    roll: 1.,
                    throttle: Some(0.),
                    ..Default::default()
                },
                |_, _| 0.,
            );
            replica.step(&PilotInput::default(), |_, _| 0.);
        }
        assert_eq!(s, replica);
        assert_ne!(s.position, before.position);
        assert_ne!(
            [s.yaw, s.pitch, s.bank],
            [before.yaw, before.pitch, before.bank]
        );
        let wreck = s.wreck.as_ref().unwrap();
        assert_eq!(wreck.polls, 0);
        assert_eq!(wreck.power.acceleration[0], 0.);
        assert!(wreck.power.acceleration[1] > 0.);
        assert!(s.engine);
        assert!(s.fuel < before.fuel);
        let mut dead_engines = before;
        dead_engines.systems.hit(10, dead_engines.throttle);
        dead_engines.crashed = true;
        dead_engines.step(&PilotInput::default(), |_, _| 0.);
        assert_eq!(dead_engines.wreck.as_ref().unwrap().power.total(), 0.);
    }
    #[test]
    fn torn_wing_changes_motion_without_fabricating_aileron_movement() {
        let mut healthy = State::new(&profile(), [0., 15000., 0.]).unwrap();
        let mut torn = healthy.clone();
        torn.damage_regions[3] = 0.75;
        for _ in 0..120 {
            healthy.step(&PilotInput::default(), |_, _| 0.);
            torn.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!(torn.bank < healthy.bank - 0.01);
        assert!(torn.g < healthy.g);
        assert_eq!(torn.aileron, healthy.aileron);
        assert!(torn.speed < healthy.speed);
        assert!(!torn.crashed);
    }
    #[test]
    fn damage_controls_devices_thrust_and_autopilot_affect_flight() {
        let aircraft = profile();
        let mut healthy = State::new(&aircraft, [0., 15000., 0.]).unwrap();
        let mut damaged = healthy.clone();
        damaged.systems.hit(5, 0.7);
        damaged.systems.hit(19, 0.7);
        damaged.systems.hit(16, 0.7);
        damaged.command(PilotCommand::Toggle(Switch::Autopilot));
        assert_eq!(damaged.autopilot.mode(), crate::autopilot::Mode::Off);
        let input = PilotInput {
            pitch: 0.5,
            commands: vec![PilotCommand::Set(Switch::Gear, true)],
            ..Default::default()
        };
        for _ in 0..120 {
            healthy.step(&input, |_, _| 0.);
            damaged.step(&input, |_, _| 0.);
        }
        assert_eq!(damaged.gear, 0.);
        assert!(healthy.gear > 0.);
        assert!(damaged.elevator < healthy.elevator * 0.6);
        assert!(damaged.g < healthy.g);
        let mut engine_only = State::new(&aircraft, [0., 15000., 0.]).unwrap();
        let mut intact = engine_only.clone();
        engine_only.systems.hit(9, 0.7);
        for _ in 0..120 {
            engine_only.step(&PilotInput::default(), |_, _| 0.);
            intact.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!(engine_only.speed < intact.speed);
        assert!(!engine_only.engine);
        engine_only.command(PilotCommand::Set(Switch::Engine, true));
        assert!(!engine_only.engine);
    }
    #[test]
    fn fuel_cheat_stops_burn_and_weight_cheat_keeps_only_external_fuel() {
        let mut normal = State::new(&profile(), [0., 15000., 0.]).unwrap();
        normal.payload_lbs = 3000.;
        let mut cheat = normal.clone();
        cheat.cheats.unlimited_fuel = true;
        cheat.cheats.ignore_weapon_weights = true;
        let fuel = cheat.fuel;
        let input = PilotInput {
            throttle: Some(1.),
            ..Default::default()
        };
        for _ in 0..240 {
            normal.step(&input, |_, _| 0.);
            cheat.step(&input, |_, _| 0.);
        }
        assert_eq!(cheat.fuel, fuel);
        assert!(normal.fuel < fuel);
        assert_eq!(cheat.carried_lbs(), cheat.systems.external_lbs());
        assert_eq!(normal.carried_lbs(), normal.payload_lbs);
        assert!(cheat.speed > normal.speed, "lighter and cleaner");
    }
    #[test]
    fn extra_g_commands_nine_g_at_full_stick() {
        let mut normal = State::new(&profile(), [0., 15000., 0.]).unwrap();
        normal.speed = 600. * 1.68781;
        normal.velocity = Basis::new(normal.yaw, 0., 0.)
            .forward
            .map(|v| v * normal.speed);
        normal.payload_lbs = 3000.;
        let mut cheat = normal.clone();
        cheat.cheats.extra_g = true;
        let input = PilotInput {
            pitch: 1.,
            ..Default::default()
        };
        normal.step(&input, |_, _| 0.);
        cheat.step(&input, |_, _| 0.);
        assert!(normal.maneuver.commanded_g < 9.);
        assert!((cheat.maneuver.commanded_g - 9.).abs() < 1e-9);
    }
    #[test]
    fn stuck_throttle_frozen_hydraulics_and_external_fuel_cannot_be_bypassed() {
        let mut s = State::new(&profile(), [0., 15000., 0.]).unwrap();
        s.systems.hit(29, s.throttle);
        s.systems.fluids.hydraulic = 0.;
        s.elevator = 0.2;
        s.aileron = -0.1;
        s.rudder = 0.3;
        s.step(
            &PilotInput {
                throttle: Some(1.),
                throttle_rate: 1.,
                pitch: -1.,
                roll: 1.,
                yaw: -1.,
                ..Default::default()
            },
            |_, _| 0.,
        );
        assert_eq!(s.throttle, 0.7);
        assert_eq!([s.elevator, s.aileron, s.rudder], [0.2, -0.1, 0.3]);
        s.systems =
            crate::aircraft_systems::Systems::new(2, [100., 0., 0., 0., 0., 0., 0., 0., 0.]);
        s.payload_lbs = 120.;
        s.fuel = 0.;
        s.engine = true;
        s.step(&PilotInput::default(), |_, _| 0.);
        assert!(s.engine);
        assert!(s.systems.external_lbs() < 100.);
        assert!((s.payload_lbs - s.systems.external_lbs() - 20.).abs() < 1e-9);
    }
    #[test]
    fn faxx_and_f22n_hook_starts_stowed_deploys_and_reverses_without_enabling_f22() {
        use tore_formats::aircraft::AircraftId;
        let mut a = profile();
        a.id = AircraftId::F22;
        a.name = "F-22".into();
        a.shape = "F22.SH".into();
        let mut donor = State::new(&a, [0., 15000., 0.]).unwrap();
        donor.command(PilotCommand::Toggle(Switch::Hook));
        assert!(!donor.hook_available());
        assert!(!donor.hook_down);
        a.id = AircraftId::F22n;
        a.shape = "F22N.SH".into();
        let mut carrier = State::new(&a, [0., 15000., 0.]).unwrap();
        assert!(carrier.hook_available());
        assert_eq!(carrier.hook, 0.);
        carrier.command(PilotCommand::Toggle(Switch::Hook));
        assert!(carrier.hook_down);
        a.id = AircraftId::Faxx;
        let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
        assert!(s.hook_available());
        assert_eq!(s.hook, 0.);
        assert!(!s.hook_down);
        s.command(PilotCommand::Toggle(Switch::Hook));
        for _ in 0..180 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!((s.hook - 0.5).abs() < 1e-9);
        let previous = s.clone();
        s.command(PilotCommand::Toggle(Switch::Hook));
        for _ in 0..90 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!((s.hook - 0.25).abs() < 1e-9);
        assert!((s.presented(&previous, 0.5).hook - 0.375).abs() < 1e-9);
        s.command(PilotCommand::Set(Switch::Hook, true));
        for _ in 0..270 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!((s.hook - 1.).abs() < 1e-9);
        s.command(PilotCommand::Set(Switch::Hook, false));
        for _ in 0..361 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert_eq!(s.hook, 0.);
    }
    #[test]
    fn faxx_preserves_donor_response_and_bay_support() {
        use tore_formats::aircraft::AircraftId;
        let mut a = profile();
        a.id = AircraftId::F22n;
        a.name = "F-22".into();
        a.shape = "F22N.SH".into();
        let mut donor = State::new(&a, [0., 15000., 0.]).unwrap();
        a.id = AircraftId::Faxx;
        let mut concept = State::new(&a, [0., 15000., 0.]).unwrap();
        assert!(concept.bay_available());
        let input = PilotInput {
            yaw: 1.,
            ..Default::default()
        };
        for _ in 0..120 {
            donor.step(&input, |_, _| 0.);
            concept.step(&input, |_, _| 0.);
        }
        assert_eq!(concept.position, donor.position);
        assert_eq!(concept.yaw, donor.yaw);
        assert_eq!(concept.rudder, donor.rudder);
    }
    #[test]
    fn bay_travel_reverses_interpolates_and_ignores_unsupported_aircraft() {
        let mut a = profile();
        let mut unsupported = State::new(&a, [0., 15000., 0.]).unwrap();
        unsupported.command(PilotCommand::Toggle(Switch::Bay));
        assert!(!unsupported.bay_open);
        a.id = tore_formats::aircraft::AircraftId::F22;
        a.name = "F-22".into();
        a.shape = "F22.SH".into();
        let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
        s.command(PilotCommand::Toggle(Switch::Bay));
        let previous = s.clone();
        for _ in 0..60 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!((s.bay - 0.5).abs() < 1e-9);
        assert!((s.presented(&previous, 0.5).bay - 0.25).abs() < 1e-9);
        s.command(PilotCommand::Toggle(Switch::Bay));
        for _ in 0..30 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!((s.bay - 0.25).abs() < 1e-9);
        s.bay_auto_open = true;
        for _ in 0..90 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!((s.bay - 1.).abs() < 1e-9);
        s.bay_auto_open = false;
        for _ in 0..120 {
            s.step(&PilotInput::default(), |_, _| 0.);
        }
        assert!(s.bay < 1e-9);
    }
    fn response_models() -> [crate::models::AircraftModel; 2] {
        use crate::models::{AircraftModel, f18::F18FlightModel, rafale_c::RafaleCFlightModel};
        [
            AircraftModel::F18(F18FlightModel::from_aircraft(&profile()).unwrap()),
            AircraftModel::RafaleC(RafaleCFlightModel::from_aircraft(&profile()).unwrap()),
        ]
    }
    #[test]
    fn achieved_load_and_body_rates_match_applied_motion_in_both_models_and_adapters() {
        for model in response_models() {
            for hybrid in [false, true] {
                for pitch in [-1.57, 0., 1.57, 2.5] {
                    let mut s = State::from_model(model.clone(), [0., 15000., 0.]);
                    if hybrid {
                        s.enable_research(1).unwrap();
                    }
                    s.pitch = pitch;
                    let before = Basis::new(s.yaw, s.pitch, s.bank);
                    let velocity = s.velocity;
                    s.step(
                        &PilotInput {
                            pitch: 1.,
                            roll: 0.7,
                            yaw: -0.6,
                            ..Default::default()
                        },
                        |_, _| 0.,
                    );
                    let after = Basis::new(s.yaw, s.pitch, s.bank);
                    let specific_force = std::array::from_fn(|i| {
                        (s.velocity[i] - velocity[i]) / DT / 32.174 + if i == 1 { 1. } else { 0. }
                    });
                    assert!((dot(specific_force, after.up) - s.g).abs() < 1e-10);
                    assert_eq!(s.maneuver.achieved_g, s.g);
                    assert_eq!(s.maneuver.tick, s.ticks);
                    let [roll, pitch, yaw] = s.maneuver.body_rates_rad_per_second;
                    let reconstructed = before.rotated(std::array::from_fn(|i| {
                        DT * (-roll * before.forward[i] - pitch * before.right[i]
                            + yaw * before.up[i])
                    }));
                    assert!(dot(reconstructed.forward, after.forward) > 1. - 1e-12);
                    assert!(dot(reconstructed.up, after.up) > 1. - 1e-12);
                    assert!((s.maneuver.commanded_g - s.g).abs() > 0.1);
                }
            }
        }
    }
    #[test]
    fn rudder_is_symmetric_releases_and_slip_dissipates_energy() {
        for model in response_models() {
            for hybrid in [false, true] {
                let mut left = State::from_model(model.clone(), [0., 15000., 0.]);
                left.yaw = 0.;
                left.velocity = [0., 0., left.speed];
                if hybrid {
                    left.enable_research(1).unwrap();
                }
                let mut right = left.clone();
                for tick in 0..1200 {
                    let yaw = if tick < 600 { 1. } else { 0. };
                    left.step(
                        &PilotInput {
                            yaw: -yaw,
                            ..Default::default()
                        },
                        |_, _| 0.,
                    );
                    right.step(
                        &PilotInput {
                            yaw,
                            ..Default::default()
                        },
                        |_, _| 0.,
                    );
                    assert!((left.speed - right.speed).abs() < 1e-8);
                    assert!((left.position[0] + right.position[0]).abs() < 1e-8);
                    assert!(
                        (left.maneuver.body_rates_rad_per_second[2]
                            + right.maneuver.body_rates_rad_per_second[2])
                            .abs()
                            < 1e-8
                    );
                }
                assert!(right.rudder.abs() < 1e-12);
                let mut config = model.configuration().clone();
                config.tuning.sideslip_drag = 0.;
                let mut no_drag_model = model.clone();
                no_drag_model.set_configuration(config).unwrap();
                let mut drag = State::from_model(model.clone(), [0., 15000., 0.]);
                let mut no_drag = State::from_model(no_drag_model, drag.position);
                drag.velocity[0] += 100.;
                no_drag.velocity = drag.velocity;
                drag.step(&Default::default(), |_, _| 0.);
                no_drag.step(&Default::default(), |_, _| 0.);
                assert!(drag.speed < no_drag.speed);
            }
        }
    }

    #[test]
    fn disturbance_rotation_completes_loop_without_rotating_velocity() {
        let mut state = State::new(&profile(), [0., 5000., 0.]).unwrap();
        state.yaw = 0.;
        state.pitch = 0.;
        state.bank = 0.;
        let velocity = state.velocity;
        let d = crate::turbulence::Disturbance {
            pitch: std::f64::consts::TAU / (240. * DT),
            ..Default::default()
        };
        let mut inverted = false;
        for _ in 0..240 {
            state.apply_turbulence(d);
            inverted |= Basis::new(state.yaw, state.pitch, state.bank).up[1] < -0.99;
            assert_eq!(state.velocity, velocity);
        }
        assert!(inverted);
        assert!(Basis::new(state.yaw, state.pitch, state.bank).forward[2] > 0.999999);
    }

    #[test]
    fn both_adapters_advect_once_and_air_data_remains_air_relative() {
        for hybrid in [false, true] {
            let mut calm = State::new(&profile(), [0., 5000., 0.]).unwrap();
            if hybrid {
                calm.enable_research(1).unwrap();
            }
            let mut windy = calm.clone();
            windy.velocity[0] += 40.;
            let mut surface = crate::research::Surface::terrain(0.);
            surface.wind = [40., 0., 0.];
            for _ in 0..1200 {
                calm.step_surface(&Default::default(), |_, _| {
                    crate::research::Surface::terrain(0.)
                });
                windy.step_surface(&Default::default(), |_, _| surface);
            }
            assert!((windy.position[0] - calm.position[0] - 400.).abs() < 1e-7);
            let reading = crate::telemetry::AirData::sample(
                &windy,
                crate::telemetry::EnvironmentReading {
                    terrain_msl_ft: 0.,
                    wind_world_fps: surface.wind,
                    atmosphere: crate::telemetry::Atmosphere::standard(windy.position[1]).unwrap(),
                },
            )
            .unwrap();
            assert!((reading.true_airspeed_knots * 1.6878098571 - calm.speed).abs() < 1e-7);
            assert!(reading.indicated_airspeed_knots.is_none());
        }
    }

    #[test]
    fn presentation_wraps_angles_without_changing_simulation() {
        let a = super::integration_tests::profile();
        let mut previous = State::new(&a, [0.; 3]).unwrap();
        previous.yaw = 359f64.to_radians();
        previous.bank = 179f64.to_radians();
        let mut current = previous.clone();
        current.yaw = 1f64.to_radians();
        current.bank = -179f64.to_radians();
        current.position[0] = 10.;
        current.gear_down = true;
        let before = current.clone();
        let rendered = current.presented(&previous, 0.5);
        assert!((rendered.yaw.sin()).abs() < 1e-9);
        assert!((rendered.bank.abs() - std::f64::consts::PI).abs() < 1e-9);
        assert_eq!(rendered.position[0], 5.);
        assert!(rendered.gear_down);
        assert_eq!(current, before);
        current.crashed = true;
        assert_eq!(current.presented(&previous, 0.), current);
    }
    #[test]
    fn fixed_clock_independent_of_render_rate() {
        for hz in [24, 30, 60, 144] {
            let mut c = Clock { remainder: 0. };
            let n: usize = (0..hz * 10).map(|_| c.steps(1. / hz as f64)).sum();
            assert_eq!(n, 1200);
        }
    }
}
#[cfg(test)]
pub(crate) mod integration_tests {
    use super::*;
    use std::collections::BTreeMap;
    use tore_formats::aircraft::{Envelope, Token};
    fn base_profile() -> Aircraft {
        let fields = [
            ("weight", 10000),
            ("internalFuel", 1000),
            ("thrust", 8000),
            ("aftThrust", 12000),
            ("fuelConsumption", 2),
            ("aftFuelConsumption", 10),
            ("maxTakeoffWeight", 15000),
            ("gearDrag", 23),
            ("flapsDrag", 70),
            ("flapsLift", 51),
            ("airBrakesDrag", 256),
            ("loadedElevator", 40),
            ("loadedDrag", 0),
            ("_gpullDrag", 0),
        ]
        .into_iter()
        .map(|(k, v)| {
            (
                k.into(),
                Token {
                    kind: "dword".into(),
                    value: v.to_string(),
                    scaled: false,
                },
            )
        })
        .collect();
        Aircraft {
            id: tore_formats::aircraft::AircraftId::F18,
            name: "Synthetic".into(),
            shape: "TEST.SH".into(),
            fields,
            object: BTreeMap::new(),
            hardpoints: vec![],
            sounds: BTreeMap::new(),
            envelopes: (-2..=6)
                .map(|g| Envelope {
                    g,
                    points: vec![[200., 0.], [250., 50000.], [1300., 50000.], [1800., 0.]],
                })
                .collect(),
        }
    }
    pub(crate) fn profile() -> Aircraft {
        let mut a = base_profile();
        for prefix in ["_brv.x", "puffRot.x", "puffRot.y", "puffRot.z"] {
            for (suffix, value) in [("min", -90), ("max", 90), ("acc", 200), ("dacc", 400)] {
                a.fields.insert(
                    format!("{prefix}.{suffix}"),
                    Token {
                        kind: "word".into(),
                        value: value.to_string(),
                        scaled: false,
                    },
                );
            }
        }

        for key in [
            "turbulencePercent",
            "rudderDrag",
            "bayDrag",
            "wheelBrakesDrag",
            "stallWarningDelay",
            "stallDelay",
            "stallSeverity",
            "stallPitchDown",
            "spinEntry",
            "spinExit",
            "spinYawLow",
            "spinYawHigh",
            "spinAOALow",
            "spinAOAHigh",
            "spinBankLow",
            "spinBankHigh",
            "crashSpeedForward",
            "crashSpeedSide",
            "crashSpeedVertical",
            "crashPitch",
            "crashRoll",
            "flags",
        ] {
            a.fields.insert(
                key.into(),
                Token {
                    kind: "word".into(),
                    value: "0".into(),
                    scaled: false,
                },
            );
        }
        for key in ["gearDrag", "flapsDrag", "airBrakesDrag"] {
            a.fields.get_mut(key).unwrap().kind = "word".into();
        }
        for axis in ["x", "y", "z"] {
            for suffix in ["min", "max", "acc", "dacc"] {
                a.fields.insert(
                    format!("_bv.{axis}.{suffix}"),
                    Token {
                        kind: "word".into(),
                        value: if suffix == "min" { "-100" } else { "100" }.into(),
                        scaled: false,
                    },
                );
            }
        }
        for (key, value) in [
            ("_brv.x.max", 100),
            ("crashSpeedForward", 330),
            ("crashSpeedSide", 50),
            ("crashSpeedVertical", 30),
            ("crashPitch", 25),
            ("crashRoll", 10),
            ("spinExit", -2),
        ] {
            a.fields.insert(
                key.into(),
                Token {
                    kind: "word".into(),
                    value: value.to_string(),
                    scaled: false,
                },
            );
        }
        a
    }
    #[test]
    fn stall_alert_tracks_adapter_state_and_suppresses_ground_and_crash() {
        use tore_formats::flight_model::departure::DepartureMode::*;
        let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
        s.speed = 1.;
        assert_eq!(s.stall_alert(0.), Some(Warning));
        assert_eq!(s.stall_alert(5000.), None);
        s.enable_research(1).unwrap();
        assert_eq!(s.stall_alert(0.), None);
        for mode in [Warning, ExtendedWarning, Stalled, Spinning] {
            s.research.as_mut().unwrap().departure.mode = mode;
            assert_eq!(s.stall_alert(0.), Some(mode));
        }
        s.research.as_mut().unwrap().on_ground = true;
        assert_eq!(s.stall_alert(0.), None);
        s.research.as_mut().unwrap().on_ground = false;
        s.crashed = true;
        assert_eq!(s.stall_alert(0.), None);
    }
    #[test]
    fn a4_hybrid_roll_is_fast_proportional_and_preserves_legacy_cap() {
        use tore_formats::aircraft::{AircraftId, Token};
        let mut a = profile();
        a.id = AircraftId::A4E;
        a.name = "A-4E".into();
        a.shape = "A4.SH".into();
        for (suffix, value) in [("min", -180), ("max", 180), ("acc", 214), ("dacc", 427)] {
            a.fields.insert(
                format!("_brv.x.{suffix}"),
                Token {
                    kind: "word".into(),
                    value: value.to_string(),
                    scaled: false,
                },
            );
        }
        for hybrid in [false, true] {
            for command in [-1_f64, -0.25, 0.25, 1.] {
                let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
                if hybrid {
                    s.enable_research(1).unwrap();
                }
                s.speed = 800.;
                s.velocity = Basis::new(s.yaw, s.pitch, s.bank)
                    .forward
                    .map(|v| v * s.speed);
                let mut replay = s.clone();
                let input = PilotInput {
                    roll: command,
                    ..Default::default()
                };
                for _ in 0..120 {
                    s.step(&input, |_, _| 0.);
                    replay.step(&input, |_, _| 0.);
                }
                assert_eq!(s, replay);
                let peak = if hybrid { 648. } else { 180. };
                assert!((s.roll_rate.to_degrees() - peak * command).abs() < 1e-8);
                for _ in 0..60 {
                    s.step(&PilotInput::default(), |_, _| 0.);
                }
                assert_eq!(s.roll_rate, 0.);
            }
        }
    }

    #[test]
    fn forward_stick_moves_the_nose_immediately_and_proportionally_in_spin() {
        use tore_formats::{aircraft::Token, flight_model::departure::DepartureMode};
        let mut a = profile();
        a.fields.insert(
            "spinYawHigh".into(),
            Token {
                kind: "word".into(),
                value: "180".into(),
                scaled: false,
            },
        );
        for direction in [-1., 1.] {
            let mut nose_rates = Vec::new();
            for pitch in [0., -0.001, -0.1, -0.5, -1.] {
                let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
                s.enable_research(1).unwrap();
                s.pitch = 0.;
                s.yaw = 0.;
                s.bank = 0.;
                s.speed = 600.;
                s.velocity = Basis::new(0., 0., 0.).forward.map(|v| v * s.speed);
                let r = s.research.as_mut().unwrap();
                r.spinning = direction as i8;
                r.spin_rate = direction * std::f64::consts::PI;
                r.departure.mode = DepartureMode::Spinning;
                s.step(
                    &PilotInput {
                        pitch,
                        ..Default::default()
                    },
                    |_, _| 0.,
                );
                assert_eq!(
                    s.research.as_ref().unwrap().departure.mode,
                    DepartureMode::Spinning
                );
                nose_rates.push(s.maneuver.body_rates_rad_per_second[1]);
            }
            assert!(nose_rates.windows(2).all(|p| p[1] < p[0]));
            assert!((nose_rates[4] - nose_rates[0]).abs() > 0.5_f64.to_radians());
        }
    }

    #[test]
    fn added_aircraft_powered_controls_are_deterministic_and_separate_from_lift() {
        use tore_formats::aircraft::{AircraftId, Token};
        for (id, name, shape, max, acc, dec) in [
            (AircraftId::F14, "F-14", "F14.SH", 225, 286, 571),
            (AircraftId::A4E, "A-4E", "A4.SH", 180, 214, 427),
            (AircraftId::X31, "X-31", "F31.SH", 345, 498, 996),
        ] {
            let mut a = profile();
            a.id = id;
            a.name = name.into();
            a.shape = shape.into();
            for prefix in ["_brv.x", "puffRot.x", "puffRot.y", "puffRot.z"] {
                let bound = if prefix == "_brv.x" { max } else { 90 };
                for (suffix, value) in
                    [("min", -bound), ("max", bound), ("acc", acc), ("dacc", dec)]
                {
                    a.fields.insert(
                        format!("{prefix}.{suffix}"),
                        Token {
                            kind: "word".into(),
                            value: value.to_string(),
                            scaled: false,
                        },
                    );
                }
            }
            for researched in [false, true] {
                let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
                if researched {
                    s.enable_research(1).unwrap();
                }
                s.speed = 110.;
                s.velocity = Basis::new(s.yaw, s.pitch, s.bank)
                    .forward
                    .map(|v| v * s.speed);
                s.throttle = 0.5;
                let input = PilotInput {
                    roll: 1.,
                    pitch: 1.,
                    yaw: 1.,
                    ..Default::default()
                };
                let mut replay = s.clone();
                s.step(&input, |_, _| 0.);
                replay.step(&input, |_, _| 0.);
                assert_eq!(s, replay);
                for rate in s.auxiliary_rates {
                    assert!((rate.to_degrees() - f64::from(acc) * 0.5 * DT).abs() < 1e-9);
                }
                assert!(s.maneuver.body_rates_rad_per_second[0] > s.roll_rate);
                let rendered = s.presented(&replay, 0.5);
                assert_eq!(rendered.auxiliary_rates, s.auxiliary_rates);
                s.fuel = 0.;
                s.step(&input, |_, _| 0.);
                assert_eq!(s.auxiliary_rates, [0.; 3]);
            }
        }
    }
    #[test]
    fn telemetry_separates_air_ground_and_altitude_datums() {
        use crate::telemetry::{AirData, Atmosphere, EnvironmentReading};
        let mut s = State::new(&profile(), [0., 10000., 0.]).unwrap();
        s.yaw = 0.;
        s.pitch = 0.;
        s.bank = 0.;
        s.velocity = [0., 10., 600.];
        let mut e = EnvironmentReading {
            terrain_msl_ft: 2500.,
            wind_world_fps: [0., 0., 100.],
            atmosphere: Atmosphere::standard(10000.).unwrap(),
        };
        let d = AirData::sample(&s, e).unwrap();
        assert_eq!(d.altitude_msl_ft, 10000.);
        assert_eq!(d.altitude_asl_ft(), 10000.);
        assert_eq!(d.altitude_agl_ft, 7500.);
        assert_eq!(d.vertical_speed_fpm, 600.);
        assert!(d.ground_speed_knots > d.true_airspeed_knots);
        assert!(d.equivalent_airspeed_knots < d.true_airspeed_knots);
        assert!(d.mach > 0.4 && d.mach < 0.6);
        assert!(d.angle_of_attack_deg.unwrap() < 0.);
        assert_eq!(d.sideslip_deg, Some(0.));
        assert!(d.indicated_airspeed_knots.is_none() && d.indicated_altitude_ft.is_none());
        e.atmosphere = Atmosphere::standard(0.).unwrap();
        let sea = AirData::sample(&s, e).unwrap();
        assert!((sea.equivalent_airspeed_knots - sea.true_airspeed_knots).abs() < 1e-9);
        s.velocity = e.wind_world_fps;
        assert!(
            AirData::sample(&s, e)
                .unwrap()
                .angle_of_attack_deg
                .is_none()
        );
        e.atmosphere.temperature_k = 0.;
        assert!(AirData::sample(&s, e).is_err());
        assert!(Atmosphere::standard(f64::NAN).is_err());
    }
    #[test]
    fn runway_start_is_supported_stationary_and_can_accelerate() {
        let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.fuel = 500.;
        s.set_payload(1000.).unwrap();
        s.start_on_runway([100., 1024., 200.], std::f64::consts::FRAC_PI_2)
            .unwrap();
        let start = s.position;
        assert_eq!(s.velocity, [0.; 3]);
        assert_eq!(s.gear, 1.);
        assert!(s.gear_down && s.brake_out && s.flaps_down);
        assert!(s.engine && !s.burner && !s.crashed);
        assert_eq!(s.fuel, 500.);
        assert_eq!(s.payload_lbs, 1000.);
        assert!(s.supported_at(1024.));
        assert!(
            (s.position[1] - 1024. - s.model().configuration().equipment.ground_clearance_ft).abs()
                < 1e-9
        );
        for _ in 0..600 {
            s.step_surface(&Default::default(), |_, _| {
                crate::research::Surface::runway(1024.)
            });
        }
        assert!(!s.crashed && s.research.as_ref().unwrap().on_ground);
        assert!((s.position[0] - start[0]).abs() < 0.01 && (s.position[2] - start[2]).abs() < 0.01);
        s.brake_out = false;
        s.throttle = 1.;
        for _ in 0..1200 {
            s.step_surface(&Default::default(), |_, _| {
                crate::research::Surface::runway(1024.)
            });
        }
        assert!(!s.crashed);
        assert!(
            s.position[0] > start[0] + 100.,
            "takeoff roll must advance along runway"
        );
    }
    #[test]
    fn full_flaps_provide_low_speed_lift_and_continuous_rotation() {
        let run = |flaps: bool| {
            let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
            s.enable_research(1).unwrap();
            s.start_on_runway([0., 1024., 0.], 0.).unwrap();
            s.brake_out = false;
            s.brake = 0.;
            s.flaps_down = flaps;
            s.flaps = f64::from(flaps);
            s.throttle = 1.;
            s.burner = true;
            for tick in 0..3600 {
                s.step_surface(
                    &PilotInput {
                        pitch: 0.35,
                        ..Default::default()
                    },
                    |_, _| crate::research::Surface::runway(1024.),
                );
                if !s.research.as_ref().unwrap().on_ground {
                    return (tick + 1, s.speed, s.maneuver.commanded_g, s.pitch);
                }
            }
            panic!("aircraft did not unload its wheels");
        };
        let flapped = run(true);
        let clean = run(false);
        assert!(
            flapped.0 < clean.0,
            "flaps did not shorten takeoff: {flapped:?} vs {clean:?}"
        );
        assert!(flapped.1 < clean.1, "flaps did not lower release speed");
        assert!(flapped.2 > 1. && flapped.3 > 0. && flapped.3 < 6f64.to_radians());

        let state = State::new(&profile(), [0., 1024., 0.]).unwrap();
        let c = state.model().configuration();
        let clean_stall = c
            .aerodynamics
            .envelopes
            .iter()
            .find(|envelope| envelope.g == 1)
            .unwrap()
            .speeds(1024.)
            .unwrap()
            .0;
        let effective = clean_stall * 0.75;
        assert_eq!(
            low_speed_positive_g_ceiling(c, 1024., effective, effective, false),
            Some(1.)
        );
        let next = c
            .aerodynamics
            .envelopes
            .iter()
            .filter(|envelope| envelope.g > 1)
            .filter_map(|envelope| envelope.speeds(1024.).map(|speeds| speeds.0))
            .filter(|minimum| *minimum > effective)
            .min_by(f64::total_cmp)
            .unwrap();
        let below = low_speed_positive_g_ceiling(c, 1024., next - 1e-6, effective, false).unwrap();
        assert!(below > 1. && below < 2.);
        assert_eq!(
            low_speed_positive_g_ceiling(c, 1024., next, effective, false),
            None
        );
        assert_eq!(
            low_speed_alignment_fraction(effective, effective, clean_stall),
            Some(0.)
        );
        let alignment_end = clean_stall * 2.;
        assert!(
            (low_speed_alignment_fraction(
                (effective + alignment_end) * 0.5,
                effective,
                clean_stall,
            )
            .unwrap()
                - 0.5)
                .abs()
                < 1e-12
        );
        assert_eq!(
            low_speed_alignment_fraction(alignment_end, effective, clean_stall),
            None
        );
    }
    #[test]
    fn tire_grip_holds_stationary_aircraft_in_wind_with_or_without_brakes() {
        for brakes in [true, false] {
            let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
            s.enable_research(1).unwrap();
            s.start_on_runway([100., 1024., 200.], 0.).unwrap();
            s.brake_out = brakes;
            s.throttle = 0.;
            let start = s.position;
            let attitude = [s.yaw, s.pitch, s.bank];
            let mut surface = crate::research::Surface::runway(1024.);
            surface.wind = [40., 0., -25.];
            for _ in 0..1200 {
                s.step_surface(&Default::default(), |_, _| surface);
            }
            assert!(!s.crashed && s.research.as_ref().unwrap().on_ground);
            assert!(
                s.position[0] == start[0] && s.position[2] == start[2],
                "stationary aircraft drifted with brakes={brakes}: {:?}",
                s.position
            );
            assert_eq!([s.velocity[0], s.velocity[2]], [0., 0.]);
            assert_eq!([s.yaw, s.pitch, s.bank], attitude, "parked attitude moved");
        }

        let mut moving = State::new(&profile(), [0., 5000., 0.]).unwrap();
        moving.enable_research(1).unwrap();
        moving.start_on_runway([0., 1024., 0.], 0.).unwrap();
        moving.brake_out = false;
        moving.velocity = [0., 0., 40.];
        moving.speed = 40.;
        for _ in 0..120 {
            moving.step_surface(
                &PilotInput {
                    yaw: 1.,
                    ..Default::default()
                },
                |_, _| crate::research::Surface::runway(1024.),
            );
        }
        assert!(moving.research.as_ref().unwrap().on_ground);
        assert!(
            moving.yaw.abs() > 0.01,
            "moving rudder steering was suppressed"
        );
    }

    #[test]
    fn mtow_runway_wind_is_continuous_during_takeoff_roll() {
        let fps = |knots: f64| knots * crate::runway_wind::FEET_PER_SECOND_PER_KNOT;
        let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
        s.enable_research(1).unwrap();
        s.start_on_runway([0., 1024., 0.], 0.).unwrap();
        s.brake_out = false;
        s.throttle = 1.;
        let mut surface = crate::research::Surface::runway(1024.);
        surface.wind = [30., 0., 0.];
        let mut previous = s.position;
        let mut crossed_ten = false;
        for _ in 0..1800 {
            s.step_surface(&Default::default(), |_, _| surface);
            let displacement = (s.position[0] - previous[0]).hypot(s.position[2] - previous[2]);
            assert!(
                displacement < 5.,
                "crosswind rollout jumped {displacement} ft"
            );
            previous = s.position;
            crossed_ten |= s.velocity[0].hypot(s.velocity[2]) >= fps(10.);
        }
        assert!(crossed_ten, "takeoff roll never entered the wind ramp");
        assert!(s.position[2] > 100., "takeoff roll did not advance");
    }
    #[test]
    fn mtow_crosswind_causes_signed_rollout_drift_and_headwind_changes_airflow() {
        let fps = |knots: f64| knots * crate::runway_wind::FEET_PER_SECOND_PER_KNOT;
        let base = State::new(&profile(), [0., 5000., 0.]).unwrap();
        let maximum = base.model().configuration().mass.max_takeoff_lbs;
        let limits = crate::runway_wind::limits(maximum).unwrap();
        let run = |crosswind_knots: f64, headwind_knots: f64| {
            let mut s = base.clone();
            s.enable_research(1).unwrap();
            s.start_on_runway([0., 1024., 0.], 0.).unwrap();
            s.brake_out = false;
            s.throttle = 0.;
            s.velocity = [0., 0., fps(15.)];
            s.speed = fps(15.);
            let mut surface = crate::research::Surface::runway(1024.);
            surface.wind = [fps(crosswind_knots), 0., -fps(headwind_knots)];
            let mut maximum_lateral = 0_f64;
            for _ in 0..300 {
                s.step_surface(&Default::default(), |_, _| surface);
                maximum_lateral = maximum_lateral.max(s.position[0].abs());
            }
            assert!(!s.crashed && s.research.as_ref().unwrap().on_ground);
            (s, maximum_lateral)
        };
        let (calm, calm_drift) = run(0., 0.);
        let (_, low_drift) = run(limits.noticeable_knots - 1., 0.);
        let (_, rough_drift) = run(limits.rough_knots, 0.);
        let (limit, limit_drift) = run(limits.limit_knots, 0.);
        let (mirrored, mirrored_drift) = run(-limits.limit_knots, 0.);
        assert_eq!(calm_drift, 0.);
        assert!(low_drift > calm_drift);
        assert!(rough_drift > low_drift);
        assert!(limit_drift > rough_drift);
        assert_eq!(limit.position[0].signum(), -mirrored.position[0].signum());
        assert!((limit_drift - mirrored_drift).abs() < 1e-8);

        let (headwind, _) = run(0., limits.limit_knots * 2.);
        assert_ne!(headwind.position, calm.position);
        assert_ne!(headwind.speed, calm.speed);

        let assessment = crate::runway_wind::assessment(maximum, [fps(30.), 0., 0.], 0.).unwrap();
        let mut loaded = base.clone();
        loaded.fuel *= 0.25;
        loaded.set_payload(1000.).unwrap();
        let loaded_assessment = crate::runway_wind::assessment(
            loaded.model().configuration().mass.max_takeoff_lbs,
            [fps(30.), 0., 0.],
            0.,
        )
        .unwrap();
        assert_eq!(assessment.limits(), loaded_assessment.limits());
    }
    #[test]
    fn runway_start_does_not_switch_legacy_adapter() {
        let mut s = State::new(&profile(), [0., 5000., 0.]).unwrap();
        let before = s.clone();
        assert!(s.start_on_runway([0.; 3], 0.).is_err());
        assert_eq!(s, before);
    }
    #[test]
    fn hybrid_wind_is_advection_and_payload_is_bounded() {
        let a = profile();
        let mut calm = State::new(&a, [0., 5000., 0.]).unwrap();
        calm.enable_research(1).unwrap();
        let mut windy = calm.clone();
        windy.velocity[0] += 40.;
        let mut surface = crate::research::Surface::runway(0.);
        surface.wind = [40., 0., 0.];
        for _ in 0..1200 {
            calm.step_surface(&Default::default(), |_, _| {
                crate::research::Surface::runway(0.)
            });
            windy.step_surface(&Default::default(), |_, _| surface);
        }
        assert!((windy.position[0] - calm.position[0] - 400.).abs() < 1e-7);
        assert!((windy.speed - calm.speed).abs() < 1e-7);
        assert!(calm.set_payload(f64::NAN).is_err());
        assert!(calm.set_payload(-1.).is_err());
        assert!(calm.set_payload(100000.).is_err());
        calm.set_payload(1000.).unwrap();
        assert_eq!(calm.payload_lbs, 1000.);
        let mut light = State::new(&a, [0., 5000., 0.]).unwrap();
        light.enable_research(1).unwrap();
        light.throttle = 1.;
        light.burner = true;
        let mut heavy = light.clone();
        heavy.set_payload(3000.).unwrap();
        for _ in 0..600 {
            light.step(&Default::default(), |_, _| 0.);
            heavy.step(&Default::default(), |_, _| 0.);
        }
        assert!(
            heavy.speed < light.speed,
            "payload must reduce acceleration"
        );
    }
    #[test]
    fn hybrid_pause_render_rate_and_ground_replay() {
        let a = profile();
        let mut initial = State::new(&a, [0., 5000., 0.]).unwrap();
        initial.enable_research(1).unwrap();
        let mut states = Vec::new();
        for hz in [30, 60, 144] {
            let mut s = initial.clone();
            let mut c = Clock { remainder: 0. };
            for _ in 0..hz * 3 {
                for _ in 0..c.steps(1. / hz as f64) {
                    s.step(&Default::default(), |_, _| 0.);
                }
            }
            states.push(s);
        }
        assert_eq!(states[0], states[1]);
        assert_eq!(states[1], states[2]);
        let before = initial.clone();
        let _ = initial.presented(&before, 0.5);
        assert_eq!(initial, before);
        initial.position[1] = 8.;
        initial.velocity = [0., -1., 100.];
        initial.yaw = 0.;
        initial.gear = 1.;
        initial.gear_down = true;
        initial.throttle = 0.;
        for _ in 0..1200 {
            initial.step_surface(&Default::default(), |_, _| {
                crate::research::Surface::runway(0.)
            });
        }
        assert!(!initial.crashed && initial.research.as_ref().unwrap().on_ground);
        assert!(initial.velocity[2] >= 0. && initial.speed < 100.);
    }
    #[test]
    fn momentum_and_roll_response_survive_control_release() {
        let a = profile();
        let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
        let keys = PilotInput {
            pitch: 1.,
            roll: 1.,
            ..Default::default()
        };
        for _ in 0..60 {
            s.step(&keys, |_, _| 0.);
        }
        let nose = Basis::new(s.yaw, s.pitch, s.bank).forward;
        assert!(dot(nose, unit(s.velocity)) < 0.99999);
        let rate = s.roll_rate;
        s.step(&Default::default(), |_, _| 0.);
        assert!(s.roll_rate > 0. && s.roll_rate < rate);
        assert_eq!(s.vertical_speed, s.velocity[1]);
        assert!(s.velocity.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn sustained_pull_can_complete_a_loop() {
        let a = profile();
        let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
        s.throttle = 1.;
        s.burner = true;
        let start = Basis::new(s.yaw, s.pitch, s.bank).forward;
        let keys = PilotInput {
            pitch: 1.,
            ..Default::default()
        };
        let (mut vertical, mut inverted, mut completed) = (false, false, false);
        for _ in 0..120 * 90 {
            s.step(&keys, |_, _| 0.);
            let b = Basis::new(s.yaw, s.pitch, s.bank);
            vertical |= b.forward[1] > 0.999;
            inverted |= b.up[1] < -0.9;
            if inverted && b.up[1] > 0.9 && dot(b.forward, start) > 0.98 {
                completed = true;
                break;
            }
        }
        assert!(
            vertical && inverted && completed && !s.crashed,
            "vertical={vertical} inverted={inverted} completed={completed} state={s:?}"
        );
    }
    #[test]
    fn banked_lift_does_not_create_a_climb_without_a_pull() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]).unwrap();
        s.bank = std::f64::consts::FRAC_PI_2;
        s.step(&Default::default(), |_, _| 0.);
        assert!(s.velocity[1] < 0. && s.position[1] < 5000.);
    }
    #[test]
    fn recorded_fractional_controls_and_commands_replay_across_render_rates() {
        use tore_input::recording;
        let mut bytes = format!("{}\n", recording::HEADER).into_bytes();
        for tick in 1..=360 {
            let mut input = PilotInput {
                pitch: (tick as f64 * 0.01).sin() * 0.4,
                roll: if tick < 180 { 0.25 } else { -0.25 },
                yaw: 0.13,
                ..Default::default()
            };
            if tick == 30 {
                input.commands.push(PilotCommand::Throttle(0.9));
            }
            if tick == 45 {
                input.commands.push(PilotCommand::Set(Switch::Gear, true));
            }
            if tick == 80 {
                input.commands.push(PilotCommand::Toggle(Switch::Gear));
            }
            recording::write_frame(&mut bytes, tick, &input).unwrap();
        }
        let tape = recording::read(bytes.as_slice()).unwrap();
        let start = State::new(&profile(), [0., 5000., 0.]).unwrap();
        let mut results = Vec::new();
        for hz in [30, 60, 144] {
            let mut state = start.clone();
            let mut clock = Clock { remainder: 0. };
            let mut tick = 0;
            for _ in 0..hz * 3 {
                for _ in 0..clock.steps(1. / hz as f64) {
                    state.step(&tape[tick], |_, _| 0.);
                    tick += 1;
                }
            }
            assert_eq!(tick, 360);
            assert!(!state.gear_down);
            results.push(state);
        }
        assert_eq!(results[0], results[1]);
        assert_eq!(results[1], results[2]);
    }
    #[test]
    fn simulation_is_identical_at_different_render_rates() {
        let a = profile();
        let start = State::new(&a, [0., 5000., 0.]).unwrap();
        let mut result = Vec::new();
        for hz in [30, 60, 144] {
            let mut s = start.clone();
            let mut c = Clock { remainder: 0. };
            let keys = PilotInput {
                pitch: 1.,
                roll: 1.,
                ..Default::default()
            };
            for _ in 0..hz * 3 {
                for _ in 0..c.steps(1. / hz as f64) {
                    s.step(&keys, |_, _| 0.);
                }
            }
            result.push(s);
        }
        assert_eq!(result[0], result[1]);
        assert_eq!(result[0], result[2]);
        assert_ne!(result[0].position, start.position);
    }
    #[test]
    fn depleted_fuel_stops_power_and_ground_contact_stops_flight() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]).unwrap();
        s.fuel = 0.;
        s.burner = true;
        s.step(&Default::default(), |_, _| 0.);
        assert!(!s.engine && !s.burner);
        s.position[1] = 0.;
        s.step(&Default::default(), |_, _| 0.);
        assert!(s.crashed);
        let final_state = s.clone();
        s.step(&Default::default(), |_, _| 0.);
        assert_eq!(s, final_state);
    }
    #[test]
    fn burner_consumes_more_fuel_and_actuators_reach_endpoints() {
        let a = profile();
        let mut dry = State::new(&a, [0., 5000., 0.]).unwrap();
        dry.throttle = 1.;
        let mut wet = dry.clone();
        wet.burner = true;
        wet.gear_down = true;
        for _ in 0..400 {
            dry.step(&Default::default(), |_, _| 0.);
            wet.step(&Default::default(), |_, _| 0.);
        }
        assert!(wet.fuel < dry.fuel);
        assert_eq!(wet.gear, 1.);
        wet.gear_down = false;
        for _ in 0..400 {
            wet.step(&Default::default(), |_, _| 0.);
        }
        assert_eq!(wet.gear, 0.);
    }
    #[test]
    fn animation_travel_reverses_and_presentation_interpolates() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]).unwrap();
        s.gear_down = true;
        s.flaps_down = true;
        s.brake_out = true;
        s.hook_down = true;
        for _ in 0..180 {
            s.step(&Default::default(), |_, _| 0.);
        }
        for value in [s.gear, s.flaps, s.brake, s.hook] {
            assert!((value - 0.5).abs() < 1e-8);
        }
        let previous = s.clone();
        s.gear_down = false;
        s.flaps_down = false;
        s.brake_out = false;
        s.hook_down = false;
        s.step(&Default::default(), |_, _| 0.);
        let render = s.presented(&previous, 0.5);
        assert!(s.gear < render.gear && render.gear < previous.gear);
        for _ in 0..180 {
            s.step(&Default::default(), |_, _| 0.);
        }
        assert_eq!([s.gear, s.flaps, s.brake, s.hook], [0.; 4]);
    }
    #[test]
    fn exhaust_and_controls_respond_then_settle() {
        let a = profile();
        let mut s = State::new(&a, [0., 5000., 0.]).unwrap();
        s.throttle = 1.;
        s.burner = true;
        let keys = PilotInput {
            pitch: 1.,
            roll: 1.,
            yaw: 1.,
            ..Default::default()
        };
        for _ in 0..30 {
            s.step(&keys, |_, _| 0.);
        }
        assert_eq!(s.exhaust, 1.);
        assert!(s.afterburner_active());
        assert!(s.elevator > 0.9 && s.aileron > 0.9 && s.rudder > 0.9);
        s.throttle = 0.95;
        assert!(!s.afterburner_active());
        for _ in 0..120 {
            s.step(&Default::default(), |_, _| 0.);
        }
        assert_eq!(s.exhaust, 0.);
        assert!(s.elevator.abs() < 0.001 && s.rudder.abs() < 0.001);
        s.throttle = 1.;
        s.engine = false;
        assert!(!s.afterburner_active());
        s.engine = true;
        s.fuel = 0.;
        assert!(!s.afterburner_active());
    }
    #[test]
    fn banked_pulls_retain_aoa_and_mirror_left_right() {
        let a = profile();
        let mut outputs = Vec::new();
        for bank in [-45f64, 45.] {
            let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
            s.bank = bank.to_radians();
            let keys = PilotInput {
                pitch: 1.,
                ..Default::default()
            };
            for _ in 0..360 {
                s.step(&keys, |_, _| 0.);
            }
            let body = Basis::new(s.yaw, s.pitch, s.bank);
            let forward = dot(s.velocity, body.forward);
            let side = dot(s.velocity, body.right);
            let up = dot(s.velocity, body.up);
            let alpha = (-up).atan2(forward);
            let beta = side.atan2(forward.hypot(up));
            assert!(
                alpha > 2f64.to_radians() && alpha < 20f64.to_radians(),
                "alpha={alpha}"
            );
            assert!(beta.abs() < 1f64.to_radians(), "beta={beta}");
            outputs.push((alpha, beta, s.position[1], s.speed));
        }
        assert!((outputs[0].0 - outputs[1].0).abs() < 1e-9);
        assert!((outputs[0].1 + outputs[1].1).abs() < 1e-9);
        assert!((outputs[0].2 - outputs[1].2).abs() < 1e-9);
        assert!((outputs[0].3 - outputs[1].3).abs() < 1e-9);
    }
    #[test]
    fn roll_in_and_pull_preserve_lateral_flight_path_lag() {
        let a = profile();
        let mut sides = Vec::new();
        for roll in [-1., 1.] {
            let mut s = State::new(&a, [0., 15000., 0.]).unwrap();
            let keys = PilotInput {
                pitch: 1.,
                roll,
                ..Default::default()
            };
            for _ in 0..90 {
                s.step(&keys, |_, _| 0.);
            }
            let b = Basis::new(s.yaw, s.pitch, s.bank);
            sides.push(dot(s.velocity, b.right));
            assert!(dot(s.velocity, b.up) < -1.);
        }
        assert!(sides[0].abs() > 0.1, "lateral lag {sides:?}");
        assert!((sides[0] + sides[1]).abs() < 1e-9);
    }
}

/// Reproducible probes of reviewed helper translations using the imported PT.
/// Deliberately separate from the authored 120 Hz integrator.
pub fn native_report(a: &Aircraft) -> tore_formats::Result<()> {
    use tore_formats::flight_model as n;
    let word = |key: &str| -> tore_formats::Result<i16> {
        let token = a
            .fields
            .get(key)
            .ok_or_else(|| std::io::Error::other(format!("missing PT {key}")))?;
        i16::try_from(token.number()?)
            .map_err(|_| std::io::Error::other(format!("invalid word {key}")))
    };
    let coefficient = word("gpullAOA")?;
    println!(
        "native_helpers_v2 aircraft={} method=static_translation complete_model=false",
        a.name
    );
    println!(
        "gpullAOA={coefficient} lowAOASpeed={} lowAOAPitch={}",
        word("lowAOASpeed")?,
        word("lowAOAPitch")?
    );
    for g in [-3, 0, 1, 3, 6, 9] {
        let mut aoa = 0;
        for _ in 0..128 {
            aoa = n::pull_aoa(aoa, g * 256, coefficient, 2);
        }
        println!(
            "g={g} pull_offset_after_1s_deg={:.6} turn_at_750fps_deg_s={:.6}",
            aoa as f64 / 256.,
            n::g_to_turn(g * 256, 750)? as f64 / 256.
        );
    }
    let envelope = a
        .envelopes
        .iter()
        .find(|e| e.g == 1)
        .ok_or_else(|| std::io::Error::other("missing 1G envelope"))?;
    for altitude in [0, 5000, 20000, 36000, 50000] {
        let limits = n::envelope_limits(
            envelope,
            altitude * 256,
            false,
            [word("structure[0]")?, word("structure[1]")?],
        )?;
        let upper = i16::try_from(limits.maximum)
            .map_err(|_| std::io::Error::other("native upper speed overflow"))?;
        println!(
            "altitude_ft={altitude} min_fps={} max_fps={} structure_fps={} drag_percent_at_750fps={}",
            limits.minimum,
            limits.maximum,
            limits.structural,
            n::drag_percent(750 * 256, altitude * 256, upper)?
        );
    }
    for throttle in [0, 50, 100, 101] {
        println!(
            "throttle={throttle} fuel_rate_fixed8={}",
            n::fuel_rate(
                word("fuelConsumption")?,
                word("aftFuelConsumption")?,
                throttle
            )
        );
    }
    let profile = n::profile::FlightProfile::from_fields(&a.fields)?;
    println!("native_profile={profile:?}");
    let mut stall = n::departure::StallState::default();
    // A supplied below-envelope condition, not a complete simulated trajectory.
    for sample in 0..5 {
        stall.advance(
            &profile.departure,
            true,
            true,
            profile.extended_warning,
            false,
            256,
        )?;
        println!(
            "departure_condition_sample={sample} mode={:?} elapsed={} severity={}",
            stall.mode,
            stall.elapsed,
            n::departure::stall_severity(&profile.departure, stall.elapsed, 150, 200)?
        );
    }
    for direction in [-1, 1] {
        let mut state = n::departure::SpinState::entered(direction, false)?;
        let mut motion = n::departure::SpinMotion {
            speed_f8: 350 * 256,
            ..Default::default()
        };
        let input = n::departure::SpinInput {
            pitch_stick: 256,
            rudder: direction as i32 * 256,
            throttle_f8: 100 * 256,
            speed_f8: 350 * 256,
            clean_stall_fps: 200,
            thrust_vector_f8: 0,
            inhibited: false,
        };
        for _ in 0..128 {
            state.advance(&mut motion, &profile.departure, input, 2)?;
        }
        println!("spin_component_direction={direction} state={state:?} motion={motion:?}");
    }
    // FA 0x452482..0x4524d6 writes the 1G envelope maximum into cp+0x245.
    let upper = n::envelope_limits(
        envelope,
        5000 * 256,
        false,
        [word("structure[0]")?, word("structure[1]")?],
    )?
    .maximum;
    let velocity_limits =
        profile.loaded_velocity(i16::try_from(upper).map_err(std::io::Error::other)?)?;
    println!("velocity_probe_forward_limit={upper} source=reviewed_1G_envelope_update_at_5000ft");
    let velocity = n::integration::Velocity {
        forward: 100 * 256,
        side: 5 * 256,
        down: 0,
    };
    let forces = n::integration::Forces {
        drag: 100_000,
        forward: 500_000,
        side: 0,
        down: 0,
    };
    for ground in [false, true] {
        println!(
            "velocity_component_ground={ground} result={:?}",
            n::integration::velocity_step(
                velocity,
                forces,
                30_000,
                velocity_limits,
                ground,
                ground,
                2
            )?
        );
    }
    Ok(())
}

/// Imported-table probes; no executable loader or emulation involved.
pub fn native_rotation_report(
    a: &Aircraft,
    table: &tore_formats::flight_model::rotation::TrigTable,
) -> tore_formats::Result<()> {
    use tore_formats::flight_model::{forces as f, rotation as r};
    println!(
        "native_rotation_v1 table=caller_supplied_data provenance=see_extractor_manifest complete_model=false"
    );
    for pitch in [0, 45, 80, 90, -90] {
        let angle = r::degrees_to_pa(pitch * 256)?;
        let trig = table.sin_cos(angle);
        let rates = r::body_rates(table, [0, 5 * 256, 0], r::degrees_to_pa(45 * 256)?, angle)?;
        println!("pitch_deg={pitch} pa={angle} trig={trig:?} rates_f8={rates:?}");
    }
    let word = |key: &str| -> tore_formats::Result<i16> {
        a.fields
            .get(key)
            .ok_or_else(|| std::io::Error::other(format!("missing {key}")))?
            .number()
            .and_then(|v| i16::try_from(v).map_err(std::io::Error::other))
    };
    for bank in [-45, 0, 45, 180] {
        let gravity = f::gravity_force(
            30000,
            table.sin_cos(0),
            table.sin_cos(r::degrees_to_pa(-bank * 256)?),
        )?;
        let lift = f::lift_force(
            f::LiftInput {
                speed_f8: 750 * 256,
                first_envelope_speed: 200,
                stall_fps: 213,
                lift_scale_f8: 256,
                flaps_lift: word("flapsLift")?,
                drag_percent: 62,
                weight: 30000,
            },
            f::DragDevices::default(),
        )?;
        let force = f::assemble(0, [0, 0], lift, gravity);
        println!(
            "force_probe_bank={bank} supplied_weight=30000 gravity={gravity:?} lift={lift} forward={} side={} down={}",
            force.forward, force.side, force.down
        );
    }
    Ok(())
}
