//! Joined native service diagnostic, with explicitly scripted producer cadence and queries.
use std::{env, fs::File, io::Read};
use tore_formats::{
    aircraft::Aircraft,
    flight_model::{
        clock_rng::NativeRng,
        departure::{DepartureMode, StallState},
        departure_stage::StageState,
        diagnostic::{Configuration, ContactQueries, GroundSample, Input, State},
        forces::DragDevices,
        ground::ContactSurface,
        integration::MovementAngles,
        loading::ControlCondition,
        rotation::{AtanTable, TrigTable, degrees_to_pa},
    },
};
fn read(path: &str, max: usize) -> std::io::Result<Vec<u8>> {
    let mut b = Vec::new();
    File::open(path)?.take(max as u64 + 1).read_to_end(&mut b)?;
    if b.len() > max {
        return Err(std::io::Error::other("diagnostic input too large"));
    }
    Ok(b)
}
struct Surface {
    ground: bool,
    water: bool,
    gear: bool,
}
impl ContactQueries for Surface {
    fn ground(&mut self, p: [i32; 3]) -> tore_formats::Result<GroundSample> {
        Ok(GroundSample {
            height_f8: 0,
            pitch_f8: 0,
            pitch_pa: 0,
            roll_pa: 0,
            on_ground: self.ground && p[1] <= 256,
            water: self.water,
            cp_0xe3_nonzero: true,
            surface: ContactSurface {
                difficulty_bypass: false,
                water: self.water,
                gear_down: self.gear,
                type_surface_bypass: false,
                surface_query: if self.ground { Some(0) } else { None },
            },
        })
    }
    fn touching_height(&mut self, _: [i32; 3]) -> tore_formats::Result<i32> {
        Ok(0)
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().skip(1).collect();
    if args.len() < 3 {
        return Err("usage: native_flight SINE ATAN PT [PT]".into());
    }
    let t = TrigTable::parse(&read(&args[0], 642)?)?;
    let a = AtanTable::parse(&read(&args[1], 1028)?)?;
    for path in &args[2..] {
        let aircraft = Aircraft::parse(&read(path, 1024 * 1024)?)?;
        let c = Configuration::from_aircraft(&aircraft)?;
        for scenario in [
            "level",
            "pull",
            "push",
            "roll-left",
            "roll-right",
            "rudder-left",
            "rudder-right",
            "stall",
            "spin-left",
            "spin-right",
            "ground",
            "water",
            "power",
            "control-disturbance",
            "flaps",
            "brake",
            "loaded",
            "damaged",
            "crosswind",
            "ground-steer",
            "gear-up",
        ] {
            let spin = scenario.starts_with("spin");
            let ground = matches!(scenario, "ground" | "water" | "ground-steer" | "gear-up");
            let sign = if scenario.ends_with("left") { -1 } else { 1 };
            let speed = if spin {
                180
            } else if ground {
                30
            } else if scenario == "stall" {
                100
            } else {
                500
            };
            let mut s = State {
                departure: StageState {
                    speed_f8: speed * 256,
                    movement: MovementAngles {
                        pitch: if scenario == "stall" { 75 * 256 } else { 0 },
                        roll: if spin { sign * 5 * 256 } else { 0 },
                        heading: 0,
                    },
                    departure: StallState {
                        mode: if spin || scenario == "stall" {
                            DepartureMode::Warning
                        } else {
                            DepartureMode::Normal
                        },
                        elapsed: if scenario == "stall" {
                            c.profile.departure.warning_delay - 2
                        } else {
                            0
                        },
                    },
                    ..Default::default()
                },
                position_f8: [0, if ground { 0 } else { 15000 * 256 }, 0],
                side_f8: 0,
                down_f8: 0,
                g_f8: 256,
                body_angles_pa: [
                    0,
                    0,
                    if spin {
                        degrees_to_pa(-sign * 5 * 256)?
                    } else {
                        0
                    },
                ],
                cached_speed_fps: speed,
                auxiliary_rates_f8: [0; 3],
                normalized_rudder_f8: 0,
                disturbance: Default::default(),
                on_ground: ground,
                ground_height_f8: 0,
                flags: 0,
                hold_ticks: 0,
                pitch_down_rate_f8: 0,
            };
            let mut rng = NativeRng::seeded(1)?;
            let (mut seen_spin, mut seen_recovery, mut seen_tumble) = (false, false, false);
            let mut callbacks = 0;
            let mut seen_disturbance = false;
            for tick in 0..900 {
                let command = if spin {
                    if tick < 300 {
                        [0, 256, sign * 256]
                    } else {
                        [0, -256, -sign * 256]
                    }
                } else if tick < 300 {
                    match scenario {
                        "pull" => [0, 256, 0],
                        "damaged" => [128, 128, 128],
                        "ground-steer" => [0, 0, 256],
                        "push" => [0, -256, 0],
                        "roll-left" | "roll-right" => [sign * 256, 0, 0],
                        "rudder-left" | "rudder-right" => [0, 0, sign * 256],
                        _ => [0; 3],
                    }
                } else {
                    [0; 3]
                };
                let i = Input {
                    now: 1000 + tick * 2,
                    ticks: 2,
                    commands: command,
                    global_flags: 0x0100_0000,
                    devices: DragDevices {
                        gear: ground && scenario != "gear-up",
                        flaps: scenario == "flaps",
                        brake: scenario == "brake",
                        ..Default::default()
                    },
                    throttle_f8: if matches!(scenario, "power" | "control-disturbance") {
                        100 * 256
                    } else {
                        0
                    },
                    vector_f8: 0,
                    fuel_f8: if matches!(scenario, "power" | "control-disturbance") {
                        1000 * 256
                    } else {
                        0
                    },
                    ordinary_stores: if scenario == "loaded" { 1000 } else { 0 },
                    flagged_stores: 0,
                    empty_weight_override: false,
                    player: true,
                    low_skill: false,
                    damage: ControlCondition::Damage {
                        pitch: if scenario == "damaged" { 25 } else { 0 },
                        roll: if scenario == "damaged" { 25 } else { 0 },
                        roll_locked: false,
                    },
                    rudder_damage: Some(if scenario == "damaged" { 25 } else { 0 }),
                    drag_damage: if scenario == "damaged" { 10 } else { 0 },
                    pull_drag_damage: if scenario == "damaged" { 10 } else { 0 },
                    afterburner: false,
                    halve_thrust: false,
                    thrust_scale_f8: 256,
                    lift_damage: if scenario == "damaged" { 10 } else { 0 },
                    disturbance_request: if scenario == "control-disturbance" && tick == 0 {
                        Some([4, 2])
                    } else if scenario == "control-disturbance" && tick == 300 {
                        Some([0, 0])
                    } else {
                        None
                    },
                    rate_shift: 0,
                    wind_fps: if scenario == "crosswind" { 20 } else { 0 },
                    wind_heading_pa: if scenario == "crosswind" {
                        degrees_to_pa(90 * 256)?
                    } else {
                        0
                    },
                };
                let mut replay = s.clone();
                let mut replay_rng = rng.clone();
                let mut q = Surface {
                    ground,
                    water: scenario == "water",
                    gear: ground && scenario != "gear-up",
                };
                let event = s.advance(&c, &t, &a, &mut rng, i, &mut q)?;
                let repeated = replay.advance(&c, &t, &a, &mut replay_rng, i, &mut q)?;
                assert_eq!(s, replay);
                assert_eq!(rng, replay_rng);
                assert_eq!(event, repeated);
                seen_disturbance |=
                    s.disturbance.pitch.offset_f8 != 0 || s.disturbance.heading.offset_f8 != 0;
                seen_spin |= s.departure.departure.mode == DepartureMode::Spinning;
                seen_recovery |= event.departure.recovered_spin;
                seen_tumble |= event.departure.tumble_applied;
                callbacks += usize::from(event.contact.contact_callback);
                if event.departure.recovered_spin {
                    assert!(!event.departure.run_normal_controls);
                }
                if scenario == "gear-up" {
                    assert_eq!(event.contact.code, 5);
                }
                if scenario == "water" {
                    assert_eq!(event.contact.code, 7);
                }
            }
            if scenario == "control-disturbance" {
                assert!(seen_disturbance, "control disturbance never became active");
            }
            if spin {
                assert!(seen_spin && seen_recovery, "spin entry/recovery");
            }
            if scenario == "stall" {
                assert!(seen_tumble, "tumble scheduling");
            }
            if ground {
                assert!(callbacks > 0);
            }
            println!(
                "PASS {} {scenario}: position={:?} speed={} G={} mode={:?} spin={seen_spin} recovered={seen_recovery} tumble={seen_tumble} contacts={callbacks}",
                aircraft.name,
                s.position_f8,
                s.departure.speed_f8,
                s.g_f8,
                s.departure.departure.mode
            );
        }
    }
    println!(
        "Joined diagnostic only: explicit setup/device/damage variants, 2-unit services, flat contact queries; no scheduler, damage events, terrain/carrier or retail trajectory claim."
    );
    Ok(())
}
