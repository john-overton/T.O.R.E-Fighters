//! Joined diagnostic service. Explicit setup, clock, damage and contact producers;
//! not a retail scheduler, terrain implementation or live flight adapter.
//! Requires the native environmental-turbulence disable flag; its producer is external.
use super::{
    clock_rng::NativeRng,
    control_tail,
    departure_stage::{self, EnvelopeInputs},
    force_stage,
    forces::DragDevices,
    ground,
    integration::Velocity,
    loading, movement_stage, normal_control,
    profile::FlightProfile,
    rotation::{self, AtanTable, TrigTable},
};
use crate::{
    Result,
    aircraft::{Aircraft, Envelope},
    invalid,
};

#[derive(Clone, Debug)]
pub struct Configuration {
    pub profile: FlightProfile,
    pub envelopes: Vec<Envelope>,
    pub structure: [i16; 2],
    pub g_range: [i16; 2],
    pub axes: [[i16; 4]; 3],
    pub tail: control_tail::Profile,
    pub empty_weight: i32,
    pub max_weight: i32,
    pub max_altitude_f8: i32,
    pub no_lift: bool,
    pub minimum_speed: i16,
    pub drag: i16,
    pub pull_drag: i16,
    pub drag_loading: i16,
    pub pull_loading: i16,
    pub elevator_loading: i16,
    pub aileron_loading: i16,
    pub thrust: i32,
    pub ab_thrust: i32,
    pub flaps_lift: i16,
    pub pull_aoa: i16,
    pub low_speed_span: i16,
    pub low_speed_pitch: i16,
}
impl Configuration {
    pub fn from_aircraft(a: &Aircraft) -> Result<Self> {
        let n = |key: &str| {
            a.fields
                .get(key)
                .or_else(|| a.object.get(key))
                .ok_or_else(|| invalid(&format!("missing diagnostic field {key}")))?
                .number()
        };
        let w = |key: &str| i16::try_from(n(key)?).map_err(|_| invalid("diagnostic word overflow"));
        let axis = |prefix: &str| -> Result<[i16; 4]> {
            Ok([
                w(&format!("{prefix}.min"))?,
                w(&format!("{prefix}.max"))?,
                w(&format!("{prefix}.acc"))?,
                w(&format!("{prefix}.dacc"))?,
            ])
        };
        if n("vtLimitDown")? != 0 {
            return Err(invalid("diagnostic requires reviewed non-VTOL aircraft"));
        }
        Ok(Self {
            profile: FlightProfile::from_fields(&a.fields)?,
            envelopes: a.envelopes.clone(),
            structure: [w("structure[0]")?, w("structure[1]")?],
            g_range: [w("envMin")?, w("envMax")?],
            axes: [axis("_brv.x")?, axis("_brv.y")?, axis("_brv.z")?],
            tail: control_tail::Profile {
                rudder: loaded_axis(axis("rudderYaw")?),
                slip: w("rudderSlip")?,
                bank: w("rudderBank")?,
                nominal_max_g: w("envMax")?,
                puff: [
                    loaded_axis(axis("puffRot.x")?),
                    loaded_axis(axis("puffRot.y")?),
                    loaded_axis(axis("puffRot.z")?),
                ],
            },
            empty_weight: n("weight")?,
            max_weight: n("maxTakeoffWeight")?,
            max_altitude_f8: n("maxAlt")?
                .checked_mul(if a.object["maxAlt"].scaled { 256 } else { 1 })
                .ok_or_else(|| invalid("altitude overflow"))?,
            no_lift: n("flags")? & 8 != 0,
            minimum_speed: w("_minSpeed")?,
            drag: w("coefDrag")?,
            pull_drag: w("_gpullDrag")?,
            drag_loading: w("loadedDrag")?,
            pull_loading: w("loadedGpullDrag")?,
            elevator_loading: w("loadedElevator")?,
            aileron_loading: w("loadedAileron")?,
            thrust: n("thrust")?,
            ab_thrust: n("aftThrust")?,
            flaps_lift: w("flapsLift")?,
            pull_aoa: w("gpullAOA")?,
            low_speed_span: w("lowAOASpeed")?,
            low_speed_pitch: w("lowAOAPitch")?,
        })
    }
}
fn loaded_axis(a: [i16; 4]) -> normal_control::LoadedAxis {
    normal_control::LoadedAxis {
        minimum: a[0],
        maximum: a[1],
        acceleration: a[2],
        deceleration: a[3],
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub departure: departure_stage::StageState,
    pub position_f8: [i32; 3],
    pub side_f8: i32,
    pub down_f8: i32,
    pub g_f8: i32,
    pub body_angles_pa: [i16; 3],
    pub cached_speed_fps: i32,
    pub auxiliary_rates_f8: [i32; 3],
    pub normalized_rudder_f8: i32,
    pub disturbance: super::control_disturbance::State,
    pub on_ground: bool,
    pub ground_height_f8: i32,
    pub flags: u32,
    pub hold_ticks: i16,
    pub pitch_down_rate_f8: i32,
}
#[derive(Clone, Copy, Debug)]
pub struct Input {
    pub now: i32,
    pub ticks: i16,
    pub commands: [i32; 3],
    pub global_flags: u32,
    pub devices: DragDevices,
    pub throttle_f8: i32,
    pub vector_f8: i32,
    pub fuel_f8: i32,
    pub ordinary_stores: i32,
    pub flagged_stores: i32,
    pub empty_weight_override: bool,
    pub player: bool,
    pub low_skill: bool,
    pub damage: loading::ControlCondition,
    pub rudder_damage: Option<u8>,
    pub drag_damage: i16,
    pub pull_drag_damage: i16,
    pub afterburner: bool,
    pub halve_thrust: bool,
    pub thrust_scale_f8: i32,
    pub lift_damage: u8,
    pub disturbance_request: Option<[i16; 2]>,
    pub rate_shift: u8,
    pub wind_fps: i32,
    pub wind_heading_pa: i16,
}
#[derive(Clone, Copy, Debug)]
pub struct GroundSample {
    pub height_f8: i32,
    pub pitch_f8: i32,
    pub pitch_pa: i16,
    pub roll_pa: i16,
    /// GetGround's cached result, independent of the later touching query.
    pub on_ground: bool,
    pub water: bool,
    pub cp_0xe3_nonzero: bool,
    pub surface: ground::ContactSurface,
}
/// Read-only environment contract. Calls happen at newly integrated/retained positions.
/// No default terrain, runway or carrier answer is fabricated by the diagnostic.
pub trait ContactQueries {
    fn ground(&mut self, position_f8: [i32; 3]) -> Result<GroundSample>;
    fn touching_height(&mut self, position_f8: [i32; 3]) -> Result<i32>;
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Events {
    pub departure: departure_stage::StageOutput,
    pub contact: ground::ContactEvents,
    /// Source 0x412a60 event parameter at >=6,7,8 G; dispatch remains external.
    pub high_g: Option<u8>,
    pub vertical_speed_fps: i16,
    pub heading_chart_toggle: bool,
}
impl State {
    /// State and RNG are committed only after the complete diagnostic service succeeds.
    /// Query implementations must be read-only; external callback side effects cannot roll back.
    pub fn advance(
        &mut self,
        c: &Configuration,
        t: &TrigTable,
        a: &AtanTable,
        rng: &mut NativeRng,
        i: Input,
        q: &mut impl ContactQueries,
    ) -> Result<Events> {
        let mut next = self.clone();
        let mut random = rng.clone();
        let events = next.advance_inner(c, t, a, &mut random, i, q)?;
        *self = next;
        *rng = random;
        Ok(events)
    }
    fn advance_inner(
        &mut self,
        c: &Configuration,
        t: &TrigTable,
        a: &AtanTable,
        rng: &mut NativeRng,
        i: Input,
        q: &mut impl ContactQueries,
    ) -> Result<Events> {
        if i.ticks < 0 || i.commands.iter().any(|v| !(-256..=256).contains(v)) {
            return Err(invalid("invalid diagnostic time/control input"));
        }
        // 0x47a74b calls GetGround during setup, before departure dispatch.
        let initial_ground = q.ground(self.position_f8)?;
        if !initial_ground.on_ground && i.global_flags & 0x0100_0000 == 0 {
            return Err(invalid(
                "diagnostic requires native environmental turbulence disabled",
            ));
        }
        self.on_ground = initial_ground.on_ground;
        self.ground_height_f8 = initial_ground.height_f8;
        if let Some([pitch, heading]) = i.disturbance_request {
            let touching = if pitch > 0 {
                ground::touching_ground(self.position_f8[1], q.touching_height(self.position_f8)?)
            } else {
                false
            };
            self.disturbance.select(pitch, heading, touching);
        }
        let weight = loading::loaded_weight(
            c.empty_weight,
            c.max_weight,
            i.fuel_f8,
            i.ordinary_stores,
            i.flagged_stores,
            i.empty_weight_override,
        )?;
        let load = weight.ordinary_percent as i32 + weight.flagged_percent as i32;
        let mut devices = i.devices;
        devices.on_ground = self.on_ground;
        let envelopes = EnvelopeInputs::resolve(
            &c.envelopes,
            c.structure,
            self.g_f8,
            self.position_f8[1],
            self.departure.speed_f8,
            devices.flaps,
            i.global_flags,
        )?;
        let g_limits = loading::loaded_g_limits(
            &c.envelopes,
            loading::GLoadInput {
                range: c.g_range,
                altitude_f8: self.position_f8[1],
                speed_f8: self.departure.speed_f8,
                flaps: devices.flaps,
                structure: c.structure,
                load_percent: load,
                elevator_coefficient: c.elevator_loading,
                player: i.player,
                low_skill: i.low_skill,
                extra_g_flag: i.global_flags & 0x20 != 0,
            },
        )?;
        let axes = loading::loaded_controls(c.axes, g_limits, i.damage, load, c.aileron_loading)?;
        self.body_angles_pa[1] = rotation::degrees_to_pa(self.departure.movement.pitch)?;
        let pitch_cos = t.sin_cos(self.body_angles_pa[1]).cos.max(0);
        let departure = self.departure.advance(
            &c.profile.departure,
            t,
            a,
            rng,
            departure_stage::StageInput {
                now: i.now,
                ticks: i.ticks,
                on_ground: self.on_ground,
                body_bank_pa: self.body_angles_pa[2],
                global_flags: i.global_flags,
                extended_warning: c.profile.extended_warning,
                vertical_thrust_support: false,
                thrust_vector_f8: i.vector_f8,
                throttle_f8: i.throttle_f8,
                controls: i.commands,
                recovery_locked: self.departure.spin.recovery_locked,
                envelopes,
                lift_scale_f8: if c.no_lift || self.position_f8[1] > c.max_altitude_f8 {
                    0
                } else if i.player {
                    ((100 - i.lift_damage as i32) << 8) / 100
                } else {
                    256
                },
            },
        )?;
        if let Some(body) = departure.tumble_body_pa {
            self.body_angles_pa = body;
        }
        let mut high_g = None;
        if departure.run_normal_controls {
            self.departure.movement = normal_control::passive_fall(
                t,
                self.departure.movement,
                self.body_angles_pa[1],
                self.body_angles_pa[2],
                pitch_cos,
                self.departure.speed_f8,
                envelopes.clean_stall_fps,
                self.on_ground,
                self.departure.departure.mode,
                i.ticks,
            )?;
            let touching = if (self.departure.speed_f8 >> 8) >= c.minimum_speed as i32 {
                ground::touching_ground(self.position_f8[1], q.touching_height(self.position_f8)?)
            } else {
                false
            };
            self.disturbance.advance(
                i.now,
                i.ticks,
                i.throttle_f8,
                self.departure.speed_f8,
                c.minimum_speed,
                touching,
                i.rate_shift,
            )?;
            let normal = normal_control::advance(
                normal_control::State {
                    g_f8: self.g_f8,
                    pitch_rate_f8: self.departure.body_rates_f8[1],
                    roll_rate_f8: self.departure.body_rates_f8[0],
                    aoa_f8: self.departure.offsets_f8[1],
                    bank_offset_f8: self.departure.offsets_f8[0],
                },
                normal_control::Input {
                    g: loaded_axis(axes[1]),
                    roll: loaded_axis(axes[0]),
                    pull_aoa_coefficient: c.pull_aoa,
                    pitch_command: departure.controls[1],
                    roll_command: departure.controls[0],
                    speed_f8: self.departure.speed_f8,
                    stall_fps: envelopes.clean_stall_fps,
                    on_ground: self.on_ground,
                    ticks: i.ticks,
                },
            )?;
            self.g_f8 = normal.g_f8;
            self.cached_speed_fps = self.departure.speed_f8 >> 8;
            high_g = if self.g_f8 >= 8 * 256 {
                Some(2)
            } else if self.g_f8 >= 7 * 256 {
                Some(4)
            } else if self.g_f8 >= 6 * 256 {
                Some(8)
            } else {
                None
            };
            let tail = control_tail::advance(
                control_tail::State {
                    yaw_rate_f8: self.departure.body_rates_f8[2],
                    slip_f8: self.departure.offsets_f8[2],
                    normalized_rudder_f8: self.normalized_rudder_f8,
                    auxiliary_rates_f8: self.auxiliary_rates_f8,
                    movement_roll_f8: self.departure.movement.roll,
                },
                c.tail,
                control_tail::Input {
                    speed_f8: self.departure.speed_f8,
                    stall_fps: envelopes.clean_stall_fps,
                    loaded_max_g_f8: axes[1][1],
                    ground_yaw: loaded_axis(axes[2]),
                    on_ground: self.on_ground,
                    rudder_damage: i.rudder_damage,
                    roll_command: departure.controls[0],
                    rudder_command: departure.controls[2],
                    original_commands: i.commands,
                    throttle_f8: i.throttle_f8,
                    vector_f8: i.vector_f8,
                    ticks: i.ticks,
                },
            )?;
            self.departure.body_rates_f8 =
                [normal.roll_rate_f8, normal.pitch_rate_f8, tail.yaw_rate_f8];
            self.departure.offsets_f8 = [normal.bank_offset_f8, normal.aoa_f8, tail.slip_f8];
            self.departure.movement.roll = tail.movement_roll_f8;
            self.auxiliary_rates_f8 = tail.auxiliary_rates_f8;
            self.normalized_rudder_f8 = tail.normalized_rudder_f8;
        }
        let turbulence_f8 = [
            self.disturbance.heading.offset_f8,
            self.disturbance.pitch.offset_f8,
        ];
        let clean = c
            .envelopes
            .iter()
            .find(|e| e.g == 1)
            .ok_or_else(|| invalid("missing clean envelope"))?;
        let limits = super::envelope_limits(clean, self.position_f8[1], false, c.structure)?;
        let upper = i16::try_from(limits.maximum).map_err(|_| invalid("loaded speed overflow"))?;
        let force = force_stage::advance(
            force_stage::Setup {
                drag: c.profile.drag,
                loaded_drag: loading::loaded_drag(
                    c.drag,
                    c.drag_loading,
                    weight.ordinary_percent,
                    weight.flagged_percent,
                    i.drag_damage,
                ),
                loaded_pull_drag: loading::loaded_drag(
                    c.pull_drag,
                    c.pull_loading,
                    weight.ordinary_percent,
                    weight.flagged_percent,
                    i.pull_drag_damage,
                ),
                loaded_afterburner_thrust: loading::selected_thrust(
                    c.thrust,
                    c.ab_thrust,
                    true,
                    i.halve_thrust,
                ),
                selected_thrust: loading::selected_thrust(
                    c.thrust,
                    c.ab_thrust,
                    i.afterburner,
                    i.halve_thrust,
                ),
                flaps_lift: c.flaps_lift,
                upper_fps: upper,
                limits: c.profile.loaded_velocity(upper)?,
            },
            t,
            force_stage::Input {
                velocity: Velocity {
                    forward: self.departure.speed_f8,
                    side: self.side_f8,
                    down: self.down_f8,
                },
                weight: weight.weight,
                fuel: i.fuel_f8,
                altitude_f8: self.position_f8[1],
                g_f8: self.g_f8,
                departure: self.departure.departure.mode,
                lift_scale_f8: departure.lift_scale_f8,
                envelopes,
                devices,
                throttle_f8: i.throttle_f8,
                thrust_scale_f8: if self.position_f8[1] > c.max_altitude_f8 {
                    0
                } else {
                    i.thrust_scale_f8
                },
                thrust_vector_pa: rotation::degrees_to_pa(i.vector_f8)?,
                body_angles_pa: [self.body_angles_pa[1], self.body_angles_pa[2]],
                rudder_slip_f8: self.departure.offsets_f8[2],
                turbulence_pitch_f8: turbulence_f8[1],
                turbulence_yaw_f8: turbulence_f8[0],
                idle_floor: super::forces::idle_drag_floor(
                    i.throttle_f8,
                    self.on_ground,
                    self.body_angles_pa[1],
                ),
                ticks: i.ticks,
            },
        )?;
        let rates = std::array::from_fn(|j| {
            self.departure.body_rates_f8[j].wrapping_add(self.auxiliary_rates_f8[j])
        });
        let mut movement = movement_stage::advance(
            t,
            a,
            movement_stage::Input {
                movement: self.departure.movement,
                position_f8: self.position_f8,
                velocity: force.velocity,
                body_rates_f8: rates,
                cached_speed_fps: self.cached_speed_fps,
                departure: self.departure.departure.mode,
                on_ground: self.on_ground,
                offsets_f8: self.departure.offsets_f8,
                turbulence_f8,
                previous_heading_pa: self.body_angles_pa[0],
                clean_stall_fps: limits.minimum.max(1),
                low_speed_span: c.low_speed_span,
                low_speed_pitch: c.low_speed_pitch,
                wind_fps: i.wind_fps,
                wind_heading_pa: i.wind_heading_pa,
                ticks: i.ticks,
            },
        )?;
        let mut ground = q.ground(movement.position_f8)?;
        ground.surface.gear_down = devices.gear;
        ground.surface.water = ground.water;
        let (retained, hold) = ground::retain_height(
            ground::ContactRetention {
                previous_ground: self.on_ground,
                previous_height: self.ground_height_f8,
                height: ground.height_f8,
                ground_pitch_pa: ground.pitch_pa,
                minimum_lift_fps: envelopes.minimum_lift_fps,
                vertical_support: false,
            },
            movement.position_f8[1],
            self.hold_ticks,
            movement.velocity.forward,
            movement
                .movement
                .pitch
                .wrapping_add(movement.low_speed_pitch_f8),
            i.ticks,
        );
        movement.position_f8[1] = retained;
        let touching = ground::touching_ground(retained, q.touching_height(movement.position_f8)?);
        let severity = ground::landing_severity(
            c.profile.landing,
            movement.movement.roll,
            movement.movement.pitch,
            movement.velocity.forward,
            movement.velocity.side,
            movement.vertical_speed_fps,
        );
        let mut contact = ground::ContactState {
            y_f8: retained,
            pitch_f8: movement.movement.pitch,
            roll_f8: movement.movement.roll,
            roll_rate_f8: rates[0],
            yaw_rate_f8: rates[2],
            pitch_down_rate_f8: self.pitch_down_rate_f8,
            hold_ticks: hold,
            side_f8: movement.velocity.side,
            down_f8: movement.velocity.down,
        };
        let contact_events = movement.settle(
            &mut contact,
            ground::ContactInput {
                touching,
                previous_ground: self.on_ground,
                ground: ground.on_ground,
                water: ground.water,
                cp_0xe3_nonzero: ground.cp_0xe3_nonzero,
                classified_code: ground::contact_code(ground.surface, severity),
                ground_height_f8: ground.height_f8,
                ground_pitch_f8: ground.pitch_f8,
                ground_roll_pa: ground.roll_pa,
                low_speed_pitch_f8: movement.low_speed_pitch_f8,
                forward_fps: movement.velocity.forward >> 8,
                stall_fps: envelopes.clean_stall_fps,
                ticks: i.ticks,
            },
        )?;
        if contact_events.contact_callback {
            self.flags = ground::contact_flags(self.flags, self.on_ground, ground.on_ground);
        }
        self.departure.body_rates_f8 = [
            contact
                .roll_rate_f8
                .wrapping_sub(self.auxiliary_rates_f8[0]),
            rates[1].wrapping_sub(self.auxiliary_rates_f8[1]),
            contact.yaw_rate_f8.wrapping_sub(self.auxiliary_rates_f8[2]),
        ];
        self.departure.movement = movement.movement;
        self.departure.speed_f8 = movement.velocity.forward;
        self.position_f8 = movement.position_f8;
        self.side_f8 = movement.velocity.side;
        self.down_f8 = movement.velocity.down;
        self.body_angles_pa = movement.body_angles_pa;
        self.hold_ticks = contact.hold_ticks;
        self.pitch_down_rate_f8 = contact.pitch_down_rate_f8;
        self.on_ground = ground.on_ground;
        self.ground_height_f8 = ground.height_f8;
        Ok(Events {
            departure,
            contact: contact_events,
            high_g,
            vertical_speed_fps: movement.vertical_speed_fps,
            heading_chart_toggle: movement.heading_chart_toggle,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        departure::{DepartureMode, DepartureProfile, SpinState, StallState},
        integration::AxisLimits,
    };
    use super::*;
    fn fixture() -> (Configuration, State, Input, TrigTable, AtanTable) {
        let axis = normal_control::LoadedAxis {
            minimum: -2,
            maximum: 2,
            acceleration: 100,
            deceleration: 100,
        };
        let p = FlightProfile {
            departure: DepartureProfile {
                warning_delay: 512,
                stall_delay: 512,
                severity: 256,
                pitch_down: 30,
                spin_entry: 0,
                spin_exit: -2,
                spin_yaw: [120, 180],
                spin_aoa: [30, 70],
                spin_bank: [15, 5],
            },
            landing: ground::LandingLimits {
                forward_fps: 2000,
                side_fps: 200,
                descent_fps: 200,
                pitch_degrees: 90,
                roll_degrees: 180,
            },
            drag: super::super::forces::DragProfile {
                rudder: 1,
                flaps: 1,
                gear: 1,
                airbrake: 1,
                bay: 1,
                wheel: 1,
            },
            velocity: [AxisLimits {
                minimum: -1000,
                maximum: 2000,
                acceleration: 1000,
                deceleration: 1000,
            }; 3],
            extended_warning: false,
        };
        let c = Configuration {
            profile: p,
            envelopes: (-4..=9)
                .map(|g| Envelope {
                    g,
                    points: vec![[100., 0.], [200., 10000.], [1500., 10000.], [2000., 0.]],
                })
                .collect(),
            structure: [2500, 2500],
            g_range: [-4, 9],
            axes: [[-90, 90, 100, 100]; 3],
            tail: control_tail::Profile {
                rudder: axis,
                slip: 20,
                bank: 5,
                nominal_max_g: 9,
                puff: [axis; 3],
            },
            empty_weight: 10000,
            max_weight: 20000,
            max_altitude_f8: 60000 * 256,
            no_lift: false,
            minimum_speed: 1000,
            drag: 10,
            pull_drag: 10,
            drag_loading: 10,
            pull_loading: 10,
            elevator_loading: 10,
            aileron_loading: 10,
            thrust: 1000,
            ab_thrust: 2000,
            flaps_lift: 20,
            pull_aoa: 9,
            low_speed_span: 100,
            low_speed_pitch: 20,
        };
        let s = State {
            departure: departure_stage::StageState {
                speed_f8: 500 * 256,
                ..Default::default()
            },
            position_f8: [0, 5000 * 256, 0],
            side_f8: 0,
            down_f8: 0,
            g_f8: 256,
            body_angles_pa: [0; 3],
            cached_speed_fps: 500,
            auxiliary_rates_f8: [0; 3],
            normalized_rudder_f8: 0,
            disturbance: Default::default(),
            on_ground: false,
            ground_height_f8: 0,
            flags: 0,
            hold_ticks: 0,
            pitch_down_rate_f8: 0,
        };
        let i = Input {
            now: 1000,
            ticks: 2,
            commands: [0; 3],
            global_flags: 0x0100_0000,
            devices: DragDevices::default(),
            throttle_f8: 0,
            vector_f8: 0,
            fuel_f8: 0,
            ordinary_stores: 0,
            flagged_stores: 0,
            empty_weight_override: false,
            player: true,
            low_skill: false,
            damage: loading::ControlCondition::Damage {
                pitch: 0,
                roll: 0,
                roll_locked: false,
            },
            rudder_damage: Some(0),
            drag_damage: 0,
            pull_drag_damage: 0,
            afterburner: false,
            halve_thrust: false,
            thrust_scale_f8: 256,
            lift_damage: 0,
            disturbance_request: None,
            rate_shift: 0,
            wind_fps: 0,
            wind_heading_pa: 0,
        };
        let bytes: Vec<_> = (0..321)
            .flat_map(|n| {
                ((n as f64 * std::f64::consts::TAU / 256.)
                    .sin()
                    .mul_add(32767., 0.)
                    .round() as i16)
                    .to_le_bytes()
            })
            .collect();
        (
            c,
            s,
            i,
            TrigTable::parse(&bytes).unwrap(),
            AtanTable::parse(&[0; 1028]).unwrap(),
        )
    }
    #[derive(Default)]
    struct Queries {
        positions: Vec<[i32; 3]>,
        fail: bool,
        land: bool,
    }
    impl ContactQueries for Queries {
        fn ground(&mut self, p: [i32; 3]) -> Result<GroundSample> {
            self.positions.push(p);
            Ok(GroundSample {
                height_f8: 0,
                pitch_f8: 0,
                pitch_pa: 0,
                roll_pa: 0,
                on_ground: self.land && self.positions.len() > 1,
                water: false,
                cp_0xe3_nonzero: true,
                surface: ground::ContactSurface {
                    difficulty_bypass: false,
                    water: false,
                    gear_down: true,
                    type_surface_bypass: false,
                    surface_query: Some(0),
                },
            })
        }
        fn touching_height(&mut self, p: [i32; 3]) -> Result<i32> {
            self.positions.push(p);
            if self.fail {
                Err(invalid("synthetic query failure"))
            } else {
                Ok(0)
            }
        }
    }
    #[test]
    fn query_order_and_failed_late_query_preserve_state_and_rng() {
        let (c, mut s, i, t, a) = fixture();
        let initial = s.clone();
        let mut rng = NativeRng::seeded(1).unwrap();
        let saved = rng.clone();
        let mut q = Queries {
            fail: true,
            ..Default::default()
        };
        assert!(s.advance(&c, &t, &a, &mut rng, i, &mut q).is_err());
        assert_eq!(s, initial);
        assert_eq!(rng, saved);
        assert_eq!(q.positions.len(), 3);
        assert_eq!(q.positions[0], initial.position_f8);
        assert_ne!(q.positions[1][2], initial.position_f8[2]);
        assert_eq!(q.positions[1], q.positions[2]);
        let event = s
            .advance(&c, &t, &a, &mut rng, i, &mut Queries::default())
            .unwrap();
        assert!(!event.contact.contact_callback);
        assert!(s.position_f8[2] > 0);
    }
    #[test]
    fn unsupported_environment_branch_is_rejected_without_state_or_rng_changes() {
        let (c, mut s, mut i, t, a) = fixture();
        i.global_flags = 0;
        let initial = s.clone();
        let mut rng = NativeRng::seeded(1).unwrap();
        let saved = rng.clone();
        assert!(
            s.advance(&c, &t, &a, &mut rng, i, &mut Queries::default())
                .is_err()
        );
        assert_eq!(s, initial);
        assert_eq!(rng, saved);
    }
    #[test]
    fn recovery_tick_skips_controls_but_runs_forces_movement_and_contact() {
        let (c, mut s, mut i, t, a) = fixture();
        s.departure.departure = StallState {
            mode: DepartureMode::Spinning,
            elapsed: 0,
        };
        s.departure.spin = SpinState::entered(1, false).unwrap();
        s.departure.spin.recovery_elapsed = 254;
        s.g_f8 = 3 * 256;
        s.cached_speed_fps = 333;
        s.position_f8[1] = 1;
        s.auxiliary_rates_f8 = [256, 0, -256];
        i.commands = [0, -256, -256];
        i.devices.gear = true;
        let mut rng = NativeRng::seeded(1).unwrap();
        let e = s
            .advance(
                &c,
                &t,
                &a,
                &mut rng,
                i,
                &mut Queries {
                    land: true,
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(e.departure.recovered_spin);
        assert!(!e.departure.run_normal_controls);
        assert_eq!(s.g_f8, 3 * 256);
        assert_eq!(s.cached_speed_fps, 333);
        assert!(e.contact.touchdown);
        assert_eq!(s.departure.body_rates_f8[0], -256);
        assert_eq!(s.departure.body_rates_f8[2], 256);
        assert_eq!(s.hold_ticks, 128);
        assert!(s.flags & 0x04000000 != 0);
    }
}
