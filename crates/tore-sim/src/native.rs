//! Live research connection of the translated flight service.
//! Host input/device/fuel sampling and 120 Hz scheduling are adaptations.
//! Airborne only until native terrain/object/carrier query producers are verified.
use crate::models::FlightModel;
use crate::{
    attitude::{Basis, dot},
    flight::{DT, PilotCommand, PilotInput, State},
    research::Surface,
};
use std::sync::Arc;
use tore_formats::{
    Result,
    flight_model::{
        clock_rng::{FixedClock, NativeRng},
        departure_stage::StageState,
        diagnostic::{self, ContactQueries, GroundSample},
        forces::DragDevices,
        ground,
        integration::MovementAngles,
        loading::ControlCondition,
        rotation::{self, AtanTable, TrigTable},
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct Tables {
    pub sine: TrigTable,
    pub atan: AtanTable,
}
impl Tables {
    pub fn parse(sine: &[u8], atan: &[u8]) -> Result<Self> {
        Ok(Self {
            sine: TrigTable::parse(sine)?,
            atan: AtanTable::parse(atan)?,
        })
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Native {
    tables: Arc<Tables>,
    pub state: Option<diagnostic::State>,
    pub clock: FixedClock,
    pub elapsed: i32,
    pub rng: NativeRng,
    pub events: Option<diagnostic::Events>,
    /// Unsupported input/contact stops advancement; restart creates fresh state.
    pub fault: Option<String>,
}
impl Native {
    pub fn new(tables: Arc<Tables>, seed: i32) -> Result<Self> {
        Ok(Self {
            tables,
            state: None,
            clock: FixedClock::default(),
            elapsed: 0,
            rng: NativeRng::seeded(seed)?,
            events: None,
            fault: None,
        })
    }
}
fn f8(value: f64) -> Result<i32> {
    let scaled = value * 256.;
    if !scaled.is_finite() || scaled < i32::MIN as f64 || scaled > i32::MAX as f64 {
        return Err(std::io::Error::other("native fixed8 input outside range"));
    }
    Ok(scaled.round() as i32)
}
fn angle(value: f64) -> Result<i32> {
    f8(value.to_degrees())
}
fn radians(pa: i16) -> f64 {
    (pa as f64 * 360. / 65536.).to_radians()
}

struct Airborne<F>(F);
impl<F: Fn(f64, f64) -> Surface> Airborne<F> {
    fn sample(&self, p: [i32; 3]) -> Result<Surface> {
        let s = (self.0)(p[0] as f64 / 256., p[2] as f64 / 256.);
        if ground::touching_ground(p[1], f8(s.height)?) {
            return Err(std::io::Error::other(
                "native research flight reached terrain contact; native surface/carrier producers are not connected",
            ));
        }
        Ok(s)
    }
}
impl<F: Fn(f64, f64) -> Surface> ContactQueries for Airborne<F> {
    fn ground(&mut self, p: [i32; 3]) -> Result<GroundSample> {
        let s = self.sample(p)?;
        Ok(GroundSample {
            height_f8: f8(s.height)?,
            pitch_f8: 0,
            pitch_pa: 0,
            roll_pa: 0,
            on_ground: false,
            water: s.water,
            cp_0xe3_nonzero: false,
            surface: ground::ContactSurface {
                difficulty_bypass: false,
                water: s.water,
                gear_down: false,
                type_surface_bypass: false,
                surface_query: None,
            },
        })
    }
    fn touching_height(&mut self, p: [i32; 3]) -> Result<i32> {
        f8(self.sample(p)?.height)
    }
}

/// Called on a candidate host state; the caller commits it only on success.
pub(crate) fn step(
    s: &mut State,
    input: &PilotInput,
    surface: impl Fn(f64, f64) -> Surface,
) -> Result<()> {
    let input = input.bounded();
    if let Some(value) = input.throttle {
        s.command(PilotCommand::Throttle(value));
    }
    for command in &input.commands {
        s.command(*command);
    }
    let model = s.model().clone();
    let c = model.configuration();
    let configuration = c.joined_native()?;
    let wind = surface(s.position[0], s.position[2]).wind;
    if wind.iter().any(|v| !v.is_finite()) || wind[1] != 0. {
        return Err(std::io::Error::other(
            "native flight requires finite horizontal wind",
        ));
    }
    let speed = wind[0].hypot(wind[2]).round();
    if speed > 200. || !s.fuel.is_finite() || s.fuel < 0. {
        return Err(std::io::Error::other(
            "native research wind/fuel sample outside supported range",
        ));
    }
    s.set_payload(s.payload_lbs)?;
    let wind_heading = rotation::degrees_to_pa(angle(wind[0].atan2(wind[2]))?)?;
    let wind_trig = s.native.as_ref().unwrap().tables.sine.sin_cos(wind_heading);
    let actual_wind = [
        speed * wind_trig.sin as f64 / 32767.,
        0.,
        speed * wind_trig.cos as f64 / 32767.,
    ];
    let initial_velocity = s.native.as_ref().and_then(|n| n.events).map_or_else(
        || std::array::from_fn(|j| s.velocity[j] - wind[j]),
        |event| event.air_world_velocity_f8.map(|v| v as f64 / 256.),
    );
    let initial_basis = Basis::new(s.yaw, s.pitch, s.bank);
    // Existing host equipment/fuel laws, not recovered actuator/engine lifecycle.
    s.throttle = (s.throttle + input.throttle_rate * DT * c.equipment.throttle_rate_per_second)
        .clamp(0., 1.);
    for (v, on) in [
        (&mut s.gear, s.gear_down),
        (&mut s.flaps, s.flaps_down),
        (&mut s.brake, s.brake_out),
        (&mut s.hook, s.hook_down),
    ] {
        *v = (*v
            + if on {
                DT / c.equipment.deployment_seconds
            } else {
                -DT / c.equipment.deployment_seconds
            })
        .clamp(0., 1.);
    }
    if s.fuel <= 0. {
        s.engine = false;
        s.burner = false;
    }
    let ab = s.afterburner_active() && c.propulsion.afterburner_thrust_lbf > 0.;
    s.exhaust += (f64::from(ab) - s.exhaust).clamp(
        -DT / c.equipment.exhaust_seconds,
        DT / c.equipment.exhaust_seconds,
    );
    s.rudder += (input.yaw - s.rudder) * DT / c.equipment.control_seconds;
    s.elevator += (input.pitch - s.elevator) * DT / c.equipment.control_seconds;
    s.aileron += (input.roll - s.aileron) * DT / c.equipment.control_seconds;
    if s.engine {
        let rate = if ab {
            c.propulsion.afterburner_fuel_lbs_per_second
        } else {
            c.propulsion.military_fuel_lbs_per_second * s.throttle
        };
        s.fuel = (s.fuel - rate * DT).max(0.);
    }
    let n = s.native.as_mut().unwrap();
    if n.state.is_none() {
        n.state = Some(diagnostic::State {
            departure: StageState {
                movement: MovementAngles {
                    heading: angle(s.yaw)?,
                    pitch: angle(s.pitch)?,
                    roll: angle(s.bank)?,
                },
                speed_f8: f8(dot(initial_velocity, initial_basis.forward))?,
                ..Default::default()
            },
            position_f8: [f8(s.position[0])?, f8(s.position[1])?, f8(s.position[2])?],
            side_f8: f8(dot(initial_velocity, initial_basis.right))?,
            down_f8: f8(-dot(initial_velocity, initial_basis.up))?,
            g_f8: 256,
            body_angles_pa: [
                rotation::degrees_to_pa(angle(s.yaw)?)?,
                rotation::degrees_to_pa(angle(s.pitch)?)?,
                rotation::degrees_to_pa(angle(-s.bank)?)?,
            ],
            cached_speed_fps: dot(initial_velocity, initial_basis.forward) as i32,
            auxiliary_rates_f8: [0; 3],
            normalized_rudder_f8: 0,
            disturbance: Default::default(),
            on_ground: false,
            ground_height_f8: 0,
            flags: 0,
            hold_ticks: 0,
            pitch_down_rate_f8: 0,
        });
    }
    let ticks = n.clock.advance(false);
    n.elapsed = n.elapsed.wrapping_add(ticks as i32);
    let state = n.state.as_mut().unwrap();
    let event = state.advance(
        configuration,
        &n.tables.sine,
        &n.tables.atan,
        &mut n.rng,
        diagnostic::Input {
            now: n.elapsed,
            ticks,
            commands: [f8(input.roll)?, f8(input.pitch)?, f8(input.yaw)?],
            global_flags: 0x0100_0000,
            devices: DragDevices {
                gear: s.gear >= 0.5,
                flaps: s.flaps >= 0.5,
                brake: s.brake >= 0.5,
                ..Default::default()
            },
            throttle_f8: f8(s.throttle * 100.)?,
            vector_f8: 0,
            fuel_f8: f8(s.fuel)?,
            ordinary_stores: s.payload_lbs.round() as i32,
            flagged_stores: 0,
            empty_weight_override: false,
            player: true,
            low_skill: false,
            damage: ControlCondition::Damage {
                pitch: 0,
                roll: 0,
                roll_locked: false,
            },
            rudder_damage: Some(0),
            drag_damage: 0,
            pull_drag_damage: 0,
            afterburner: ab,
            halve_thrust: false,
            thrust_scale_f8: if s.engine { 256 } else { 0 },
            lift_damage: 0,
            disturbance_request: None,
            rate_shift: 0,
            wind_fps: speed as i32,
            wind_heading_pa: wind_heading,
        },
        &mut Airborne(surface),
    )?;
    s.position = state.position_f8.map(|v| v as f64 / 256.);
    s.yaw = radians(state.body_angles_pa[0]).rem_euclid(std::f64::consts::TAU);
    s.pitch = radians(state.body_angles_pa[1]);
    s.bank = -radians(state.body_angles_pa[2]);
    let air = event.air_world_velocity_f8.map(|v| v as f64 / 256.);
    s.velocity = std::array::from_fn(|j| air[j] + actual_wind[j]);
    s.speed = dot(air, air).sqrt();
    s.vertical_speed = air[1];
    s.auxiliary_rates = state
        .auxiliary_rates_f8
        .map(|v| (v as f64 / 256.).to_radians());
    s.roll_rate = (state.departure.body_rates_f8[0] as f64 / 256.).to_radians();
    s.pitch_rate = (state.departure.body_rates_f8[1] as f64 / 256.).to_radians();
    // Host acceleration measurement, including the quantized service interval.
    let acceleration = std::array::from_fn(|j| {
        (air[j] - initial_velocity[j]) * 256. / ticks as f64 + if j == 1 { 32.174 } else { 0. }
    });
    s.g = dot(acceleration, Basis::new(s.yaw, s.pitch, s.bank).up) / 32.174;
    s.ticks += 1;
    s.maneuver = crate::telemetry::Maneuver {
        tick: s.ticks,
        commanded_g: state.g_f8 as f64 / 256.,
        achieved_g: s.g,
        lift_g: event.lift_force as f64 / (event.weight as f64 * 256.),
        body_rates_rad_per_second: state
            .departure
            .body_rates_f8
            .map(|v| (v as f64 / 256.).to_radians()),
        rudder_command: input.yaw,
        rudder_deflection: s.rudder,
        effective_rudder: state.normalized_rudder_f8 as f64 / 256.,
        departure: Some(state.departure.departure.mode),
        stall_severity_f8: event.departure.severity_f8,
    };
    n.events = Some(event);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (State, Arc<Tables>) {
        let mut a = crate::flight::integration_tests::profile();
        let mut put = |key: &str, value: i32| {
            a.fields.insert(
                key.into(),
                tore_formats::aircraft::Token {
                    kind: "dword".into(),
                    value: value.to_string(),
                    scaled: false,
                },
            );
        };
        // Entirely synthetic source fields; no retail fixture bytes.
        for (k, v) in [
            ("vtLimitDown", 0),
            ("structure[0]", 100),
            ("structure[1]", 100),
            ("envMin", -2),
            ("envMax", 6),
            ("maxAlt", 60000 * 256),
            ("_minSpeed", 100),
            ("coefDrag", 256),
            ("loadedGpullDrag", 0),
            ("loadedAileron", 0),
            ("flapsLift", 0),
            ("gpullAOA", 0),
            ("lowAOASpeed", 100),
            ("lowAOAPitch", 5),
            ("rudderSlip", 4),
            ("rudderBank", 5),
        ] {
            put(k, v);
        }
        for axis in [
            "_brv.x",
            "_brv.y",
            "_brv.z",
            "rudderYaw",
            "puffRot.x",
            "puffRot.y",
            "puffRot.z",
        ] {
            for (suffix, v) in [("min", -100), ("max", 100), ("acc", 100), ("dacc", 100)] {
                put(&format!("{axis}.{suffix}"), v);
            }
        }
        let sine: Vec<u8> = (0..321)
            .flat_map(|i| {
                ((i as f64 * std::f64::consts::TAU / 256.)
                    .sin()
                    .mul_add(32767., 0.)
                    .round() as i16)
                    .to_le_bytes()
            })
            .collect();
        let atan: Vec<u8> = (0..514)
            .flat_map(|i| {
                (((i as f64 / 512.).atan() / std::f64::consts::TAU * 65536.).round() as u16)
                    .to_le_bytes()
            })
            .collect();
        (
            State::new(&a, [0., 15000., 0.]).unwrap(),
            Arc::new(Tables::parse(&sine, &atan).unwrap()),
        )
    }
    #[test]
    fn rejected_contact_rolls_back_host_equipment_clock_and_rng() {
        let (mut s, t) = fixture();
        s.enable_native(t, 1).unwrap();
        let initial = s.clone();
        let p = PilotInput {
            throttle: Some(1.),
            commands: vec![PilotCommand::Toggle(crate::flight::Switch::Gear)],
            ..Default::default()
        };
        s.step_surface(&p, |x, _| {
            Surface::terrain(if x > 0. { 20000. } else { 0. })
        });
        assert!(s.native_fault().unwrap().contains("contact"));
        s.native.as_mut().unwrap().fault = None;
        assert_eq!(s, initial);
    }
    #[test]
    fn runtime_replay_clock_and_presentation_are_independent() {
        let (mut s, t) = fixture();
        s.enable_native(t, 1).unwrap();
        let mut replay = s.clone();
        for _ in 0..120 {
            let old = s.clone();
            s.step_surface(&PilotInput::default(), |_, _| Surface::terrain(0.));
            replay.step_surface(&PilotInput::default(), |_, _| Surface::terrain(0.));
            let before = s.clone();
            for alpha in [0., 0.5, 1.] {
                let _ = s.presented(&old, alpha);
            }
            assert_eq!(s, before);
            assert_eq!(s, replay);
            assert!(s.native_fault().is_none(), "{:?}", s.native_fault());
        }
        assert_eq!(s.native.as_ref().unwrap().elapsed, 256);
        assert_eq!(s.ticks, 120);
        let saved = s.clone();
        s.apply_turbulence(crate::turbulence::Disturbance::default());
        assert_eq!(s, saved);
        assert!(s.enable_research(1).is_err());
    }
    #[test]
    fn host_basis_matches_native_movement_axes_through_inverted_attitudes() {
        let (_, t) = fixture();
        for heading in [0f64, 90., 225.] {
            for pitch in [-89f64, 0., 89.] {
                for bank in [-179f64, -90., 0., 90., 179.] {
                    let b = Basis::new(heading.to_radians(), pitch.to_radians(), bank.to_radians());
                    let v = rotation::world_velocity(
                        &t.sine,
                        tore_formats::flight_model::integration::Velocity {
                            forward: 500 * 256,
                            side: 20 * 256,
                            down: -10 * 256,
                        },
                        MovementAngles {
                            heading: (heading * 256.) as i32,
                            pitch: (pitch * 256.) as i32,
                            roll: (bank * 256.) as i32,
                        },
                        (pitch * 256.) as i32,
                    )
                    .unwrap();
                    for (j, actual) in v.into_iter().enumerate() {
                        let expected = 500. * b.forward[j] + 20. * b.right[j] + 10. * b.up[j];
                        assert!(
                            (actual as f64 / 256. - expected).abs() < 0.5,
                            "{heading}/{pitch}/{bank} axis {j}: {} vs {expected}",
                            actual as f64 / 256.
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn incomplete_profiles_cannot_silently_activate_native() {
        let a = crate::flight::integration_tests::profile();
        let mut s = State::new(&a, [0., 10000., 0.]).unwrap();
        let (_, t) = fixture();
        assert!(
            s.enable_native(t, 1)
                .unwrap_err()
                .to_string()
                .contains("configuration unavailable")
        );
        assert!(s.native.is_none());
        let (s, t) = fixture();
        let mut model = s.model().clone();
        let mut c = model.configuration().clone();
        c.propulsion.military_thrust_lbf += 1.;
        model.set_configuration(c).unwrap();
        let mut changed = State::from_model(model, [0., 10000., 0.]);
        assert!(
            changed
                .enable_native(t, 1)
                .unwrap_err()
                .to_string()
                .contains("differs")
        );
    }
}
