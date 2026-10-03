use super::*;
use tore_workers::Executor;

fn executors() -> Vec<Executor> {
    let mut executors: Vec<_> = [0, 1, 2, 4, 8]
        .into_iter()
        .map(|count| Executor::parallel(count).unwrap())
        .collect();
    executors.extend([Executor::shuffled(0), Executor::shuffled(17)]);
    executors
}

fn launcher(index: u32) -> Launcher {
    Launcher {
        radar_power: true,
        position: [f64::from(index) * 650., 3000., 0.],
        basis: Basis::new(0., 0., 0.),
        speed_fps: 300.,
        velocity: [0., 0., 300.],
        bay_ready: true,
        radar: true,
        jammer: false,
        alive: true,
        body_present: true,
        controls: sensors::Controls::default(),
    }
}

fn fixture(guided: bool) -> State {
    let mut state = tests::fixture(guided);
    let config = state.own().configuration().clone();
    for id in 1..8 {
        state
            .add_ownship(Ownship::new(id, Side(1 + id % 2), config.clone(), true).unwrap())
            .unwrap();
    }
    state.weapon_rules = Rules::Spec;
    state.cheats.unlimited_ammo = true;
    for (index, own) in state.ownships.iter_mut().enumerate() {
        own.armed = true;
        let weapon = &mut own.config.stations[0].weapon;
        if guided {
            let (name, signature) = [
                ("AIM9M.JT", 2),
                ("AIM120.JT", 3),
                ("AIM7M.JT", 3),
                ("AGM88.JT", 4),
            ][index % 4];
            weapon.source = name.into();
            weapon.seeker.signature = signature;
        } else {
            weapon.source = "M61.JT".into();
        }
    }
    state.targets = (0..8)
        .map(|id| {
            let mut target = tests::target(
                100 + id,
                [f64::from(id) * 650., 3000., 4500.],
                100_000,
                0x80,
            );
            target.velocity = [13., 0., -80.];
            target.radar_emitting = true;
            target
        })
        .collect();
    state
}

fn ground(x: f64, z: f64) -> f64 {
    50. + (x * 0.001).sin() * (z * 0.001).cos() * 20.
}

fn inputs(tick: u32, held: bool) -> Vec<OwnshipInput> {
    (0..8)
        .rev()
        .map(|aircraft| {
            let mut launcher = launcher(aircraft);
            launcher.position[2] += f64::from(tick) * 2.5;
            launcher.radar = !(32..48).contains(&tick) || aircraft != 1;
            launcher.radar_power = launcher.radar;
            launcher.bay_ready = tick > 80 || aircraft != 0;
            OwnshipInput {
                aircraft,
                held,
                launcher,
            }
        })
        .collect()
}

/// Every changed floating-point observation, including values whose ordinary
/// PartialEq would equate positive and negative zero.
fn observation_bits(own: &Ownship) -> Vec<u64> {
    let mut out = vec![own.mounted.quality.to_bits()];
    for observation in [own.bore_observation, own.mounted.observation]
        .into_iter()
        .flatten()
    {
        out.extend(observation.position.map(f64::to_bits));
        out.extend(observation.velocity.map(f64::to_bits));
        out.extend([
            observation.quality.to_bits(),
            observation.off_axis.to_bits(),
            observation.range.to_bits(),
        ]);
    }
    if let Some(estimate) = own.range_estimate {
        out.push(estimate.maximum.to_bits());
        if let Some(band) = estimate.favorable {
            out.extend([band.minimum.to_bits(), band.maximum.to_bits()]);
        }
    }
    for contact in own.sensors.contacts().iter().chain(own.sensors.visual()) {
        out.extend(contact.position.map(f64::to_bits));
        out.extend(contact.velocity.map(f64::to_bits));
        out.extend([
            contact.bearing_rad.to_bits(),
            contact.elevation_rad.to_bits(),
            contact.distance_ft.to_bits(),
        ]);
    }
    for strobe in own.sensors.strobes() {
        out.extend([
            strobe.bearing_rad.to_bits(),
            strobe.elevation_rad.to_bits(),
            strobe.received.to_bits(),
            strobe.half_width_rad.to_bits(),
            strobe.sidelobe_floor.to_bits(),
        ]);
    }
    for plot in own.sensors.plots() {
        out.extend(plot.position.map(f64::to_bits));
        out.extend([
            plot.bearing_rad.to_bits(),
            plot.elevation_rad.to_bits(),
            plot.distance_ft.to_bits(),
        ]);
    }
    for emitter in &own.emitters {
        out.extend([emitter.bearing_rad.to_bits(), emitter.received.to_bits()]);
        out.extend(emitter.distance_nmi.map(f64::to_bits));
    }
    out
}

fn assert_own_equal(actual: &Ownship, expected: &Ownship) {
    assert_eq!(observation_bits(actual), observation_bits(expected));
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
}

fn assert_state_equal(actual: &State, expected: &State) {
    // Include opaque histories, RNGs, counters, ammo, damage, trigger cadence,
    // all ownships and targets, effects, sounds, strikes and ordered ledger.
    assert_eq!(format!("{actual:?}"), format!("{expected:?}"));
    for (actual, expected) in actual.ownships.iter().zip(&expected.ownships) {
        assert_own_equal(actual, expected);
    }
    for (actual, expected) in actual.projectiles.iter().zip(&expected.projectiles) {
        assert_eq!(
            actual.position.map(f64::to_bits),
            expected.position.map(f64::to_bits)
        );
        assert_eq!(
            actual.previous.map(f64::to_bits),
            expected.previous.map(f64::to_bits)
        );
        assert_eq!(
            actual.direction.map(f64::to_bits),
            expected.direction.map(f64::to_bits)
        );
    }
}

#[test]
fn ownship_observation_extraction_matches_pre_extraction_reference() {
    let mut state = fixture(true);
    for tick in 0..100 {
        let inputs = inputs(tick, tick >= 80);
        let rows: Vec<_> = state
            .ownships
            .iter()
            .enumerate()
            .map(|(index, own)| {
                let launcher = inputs
                    .iter()
                    .find(|input| input.aircraft == own.aircraft)
                    .unwrap()
                    .launcher;
                OwnRow {
                    index,
                    launcher,
                    previous: launcher.position,
                    target: ownship_target(own, launcher),
                }
            })
            .collect();
        state.ownship_rows = rows.iter().map(|row| row.target.clone()).collect();
        for row in &rows {
            let mut actual = state.ownships[row.index].clone();
            let mut expected = actual.clone();
            observation_reference::observe(
                &state,
                &mut expected,
                row.index,
                row.launcher,
                &rows,
                &ground,
            );
            observation::observe_ownship(
                &mut actual,
                row.index,
                row.launcher,
                &observation::Context {
                    targets: &state.targets,
                    rows: &rows,
                    weapon_rules: state.weapon_rules,
                    easy_targeting: state.cheats.easy_targeting,
                    tick: state.tick,
                    ground: &ground,
                },
            );
            assert_own_equal(&actual, &expected);
            assert_eq!(
                view(&state, &actual).readiness(row.launcher),
                view(&state, &expected).readiness(row.launcher)
            );
        }
        state.step_rewound_with_executor(&inputs, &[], ground, |_, _| false, &Executor::serial());
        if tick == 20 {
            for id in 0..8 {
                state.command(id, Command::DesignateTarget(100 + id), launcher(id));
            }
        }
        state.cheats.easy_targeting = tick < 50;
    }
}

#[test]
fn ownship_workers_match_complete_serial_ticks_with_reordered_inputs() {
    for executor in executors() {
        let mut expected = fixture(true);
        let mut actual = expected.clone();
        for tick in 0..100 {
            for state in [&mut actual, &mut expected] {
                if tick == 20 {
                    for id in 0..8 {
                        state.command(id, Command::DesignateTarget(100 + id), launcher(id));
                    }
                }
                if tick == 25 {
                    state.ownship_mut(3).unwrap().pending_damage = true;
                }
                state.ownship_mut(2).unwrap().radar_failed = (35..50).contains(&tick);
                state.cheats.easy_targeting = tick < 60;
            }
            let mut inputs = inputs(tick, tick >= 80);
            if (55..65).contains(&tick) {
                inputs.retain(|input| input.aircraft % 2 == 0);
            }
            let mut sorted = inputs.clone();
            sorted.reverse();
            let before = expected.step_rewound_with_executor(
                &sorted,
                &[(0, 7), (2, 12)],
                ground,
                |_, _| false,
                &Executor::serial(),
            );
            let after = actual.step_rewound_with_executor(
                &inputs,
                &[(0, 7), (2, 12)],
                ground,
                |_, _| false,
                &executor,
            );
            assert_eq!(after, before, "tick {tick}");
            assert_state_equal(&actual, &expected);
        }
    }
}

#[test]
fn inactive_ownships_and_empty_input_lists_do_not_gain_observation_steps() {
    for executor in executors() {
        for active in [0, 1] {
            let mut actual = fixture(false);
            let mut expected = actual.clone();
            let inputs: Vec<_> = inputs(0, false).into_iter().take(active).collect();
            let snapshots: Vec<_> = actual
                .ownships
                .iter()
                .map(|own| format!("{own:?}"))
                .collect();
            let before = expected.step_rewound_with_executor(
                &inputs,
                &[],
                ground,
                |_, _| false,
                &Executor::serial(),
            );
            let after =
                actual.step_rewound_with_executor(&inputs, &[], ground, |_, _| false, &executor);
            assert_eq!(after, before);
            assert_state_equal(&actual, &expected);
            for own in &actual.ownships {
                if !inputs.iter().any(|input| input.aircraft == own.aircraft) {
                    assert_eq!(format!("{own:?}"), snapshots[own.aircraft as usize]);
                }
            }
        }
    }
}

#[test]
fn simultaneous_releases_keep_capacity_ammo_and_shot_ids_in_aircraft_order() {
    for executor in executors() {
        let mut initial = fixture(false);
        initial.cheats.unlimited_ammo = false;
        initial.command(0, Command::Incoming, launcher(0));
        let mut projectile = initial.projectiles[0].clone();
        projectile.position = [1_000_000., 3000., 1_000_000.];
        projectile.previous = projectile.position;
        projectile.incoming = None;
        initial.projectiles = (0..MAX_PROJECTILES - 2)
            .map(|index| {
                let mut projectile = projectile.clone();
                projectile.id = 10_000 + index as u32;
                projectile
            })
            .collect();
        let next = initial.next_shot;
        let mut expected = initial.clone();
        let mut actual = initial;
        let inputs = inputs(100, true);
        let before = expected.step_rewound_with_executor(
            &inputs,
            &[(0, 7), (1, 9)],
            ground,
            |_, _| false,
            &Executor::serial(),
        );
        let after = actual.step_rewound_with_executor(
            &inputs,
            &[(0, 7), (1, 9)],
            ground,
            |_, _| false,
            &executor,
        );
        assert_eq!(after, before);
        assert_state_equal(&actual, &expected);
        let fired: Vec<_> = after
            .iter()
            .filter_map(|event| match event {
                Event::Fired { aircraft, .. } => Some(*aircraft),
                _ => None,
            })
            .collect();
        assert_eq!(fired, [0, 1]);
        assert_eq!(actual.next_shot, next + 2);
        assert_eq!(actual.rewinds.get(&next), Some(&7));
        assert_eq!(actual.rewinds.get(&(next + 1)), Some(&9));
        for own in &actual.ownships[2..] {
            assert_eq!(own.release_readiness, Readiness::Capacity);
            assert_eq!(own.shots, 0);
        }
    }
}
