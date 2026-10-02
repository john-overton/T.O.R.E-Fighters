//! Repeatable no-AI imported-store range probes. Outputs measured outcomes, not
//! a guaranteed effective range. No retail resources are compiled into the app.
use crate::AppResult;
use tore_sim::{
    attitude::Basis,
    combat::{
        live::{self, Command, Configuration, Launcher, OwnshipInput},
        missiles::{self, LaunchMode},
    },
    sensors,
};

pub fn run(config: Configuration) -> AppResult<()> {
    println!(
        "weapon,mode,launcher_fps,launch_motion,target_motion,range_fraction,range_ft,outcome,time_s,travel_ft"
    );
    let mut seen = std::collections::BTreeSet::new();
    for (station, store) in config.stations.iter().enumerate() {
        let Some(profile) = missiles::Profile::for_weapon(&store.weapon) else {
            continue;
        };
        // This probe supplies aircraft only, not a surface-designation channel.
        if profile.role == missiles::TargetRole::Surface {
            continue;
        }
        if !seen.insert(store.weapon.source.clone()) {
            continue;
        }
        for mode in [LaunchMode::Cued, LaunchMode::Boresight] {
            if mode == LaunchMode::Boresight && !profile.supports_boresight() {
                continue;
            }
            for speed in [300., 600., 900.] {
                for (launch_motion, side, climb) in
                    [("level", 0., 0.), ("climb", 0., 100.), ("slip", 100., 0.)]
                {
                    if launch_motion != "level" && speed != 600. {
                        continue;
                    }
                    for (target_motion, velocity) in [
                        ("stationary", [0f64; 3]),
                        ("approaching", [0., 0., -600.]),
                        ("receding", [0., 0., 600.]),
                        ("crossing", [600., 0., 0.]),
                    ] {
                        for fraction in [0.25, 0.5, 0.75, 1.] {
                            let distance = (f64::from(store.weapon.seeker.zones[1].maximum_range)
                                * fraction)
                                .max(f64::from(store.weapon.seeker.zones[1].minimum_range));
                            let mut state = live::State::new(config.clone(), true)?;
                            state.own_mut().selected = station;
                            let l = Launcher {
                                position: [0., 10000., 0.],
                                basis: Basis::new(0., 0., 0.),
                                speed_fps: speed,
                                velocity: [
                                    side,
                                    climb,
                                    (speed * speed - side * side - climb * climb).sqrt(),
                                ],
                                bay_ready: true,
                                radar_power: true,
                                radar: true,
                                jammer: false,
                                alive: true,
                                body_present: true,
                                controls: sensors::Controls::default(),
                            };
                            state.range_target(0, l);
                            state.targets[0].position = [0., 10000., distance];
                            state.targets[0].velocity = [0.; 3];
                            state.targets[0].basis =
                                Basis::new(velocity[0].atan2(velocity[2]), 0., 0.);
                            state.step(
                                &[OwnshipInput {
                                    aircraft: 0,
                                    held: false,
                                    launcher: l,
                                }],
                                |_, _| 0.,
                            );
                            state.command(0, Command::Designate, l);
                            // Establish the cued track before applying the scripted velocity.
                            for _ in 0..90 {
                                state.step(
                                    &[OwnshipInput {
                                        aircraft: 0,
                                        held: false,
                                        launcher: l,
                                    }],
                                    |_, _| 0.,
                                );
                            }
                            state.own_mut().launch_mode = mode;
                            if mode == LaunchMode::Boresight {
                                state.command(0, Command::ClearDesignation, l);
                            }
                            state.targets[0].velocity = velocity;
                            state.targets[0].basis =
                                Basis::new(velocity[0].atan2(velocity[2]), 0., 0.);
                            let events = state.step(
                                &[OwnshipInput {
                                    aircraft: 0,
                                    held: true,
                                    launcher: l,
                                }],
                                |_, _| 0.,
                            );
                            state.release(0);
                            let mut outcome = if events
                                .iter()
                                .any(|e| matches!(e, live::Event::Fired { .. }))
                            {
                                "expiry".to_owned()
                            } else {
                                format!("inhibit:{}", state.own().release_readiness.label())
                            };
                            let mut travel = 0.;
                            let mut seconds = 0.;
                            let mut previous = l.position;
                            if !state.projectiles.is_empty() {
                                for tick in 0..u64::from(store.weapon.movement.remove_t) * 30 + 1 {
                                    let events = state.step(
                                        &[OwnshipInput {
                                            aircraft: 0,
                                            held: false,
                                            launcher: l,
                                        }],
                                        |_, _| 0.,
                                    );
                                    if let Some(p) = state.projectiles.first() {
                                        travel +=
                                            missiles::length(missiles::sub(p.position, previous));
                                        previous = p.position;
                                    }
                                    seconds = (tick + 2) as f64 / 120.;
                                    if events.iter().any(|e| matches!(e, live::Event::Hit(_))) {
                                        outcome = "hit".into();
                                        break;
                                    }
                                    if events.contains(&live::Event::Ground) {
                                        outcome = "ground".into();
                                        break;
                                    }
                                    if state.projectiles.is_empty() {
                                        break;
                                    }
                                }
                            }
                            println!(
                                "{},{},{speed},{launch_motion},{target_motion},{fraction},{distance},{outcome},{seconds:.3},{travel:.1}",
                                store.weapon.source,
                                mode.label()
                            );
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
