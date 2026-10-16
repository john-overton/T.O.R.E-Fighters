//! Inert store separation, opinionated request by John, 2026-10-15.
use super::*;
use crate::combat::{
    ballistics::{BombMotion, ReleaseState},
    hardpoints::Rack,
    store_sizes::{self, StoreSize},
};

/// Fitted 2.5-foot-wide pock mark, one quarter of the existing ten-foot mark.
pub(super) const CRATER_SCALE: u8 = 25;

impl State {
    /// AI inventory bridge. Uses the player separation path, preserving the
    /// cockpit selection. Empty stations and already removed tanks are no-ops.
    pub fn jettison_ai(&mut self, own: &mut Ownship, launcher: Launcher, stations: &[usize]) {
        let selected = own.selected;
        self.jettison(own, launcher, true);
        for &station in stations {
            own.selected = station;
            self.jettison(own, launcher, false);
        }
        own.selected = selected;
    }

    pub(super) fn jettison(&mut self, own: &mut Ownship, launcher: Launcher, tanks: bool) {
        if !launcher.alive || own.hp <= 0 || !own.external {
            return;
        }
        let mut stores = Vec::new();
        if tanks {
            for tank in &mut own.config.tanks {
                let Some(store) = &tank.store else {
                    continue;
                };
                let body = store_sizes::tank_body(f64::from(store.tank.fuel_weight));
                let mut weapon = inert_record();
                weapon.source = format!("JETTISON-{}", store.source);
                weapon.name.clone_from(&store.name);
                weapon.hud_name.clone_from(&store.name);
                weapon.shape = Some(body_shape(body));
                let rack = Rack {
                    location: if tank.rack.location().external()
                        && !(tank.rack.location == 0 && tank.mount[0].abs() > 1e-9)
                    {
                        tank.rack.location
                    } else if tank.mount[0].abs() > 1e-9 {
                        4
                    } else {
                        0
                    },
                    paired: tank.quantity > 1,
                    ..tank.rack
                };
                for (side, count) in rack
                    .sides(tank.mount, tank.quantity)
                    .into_iter()
                    .enumerate()
                {
                    for slot in 0..count {
                        let point = rack.store_point(
                            tank.mount,
                            tank.quantity,
                            side,
                            slot,
                            StoreSize::body(body),
                        );
                        stores.push((weapon.clone(), 0, point));
                    }
                }
                tank.quantity = 0;
            }
            own.config
                .refresh_tanks()
                .expect("removing validated tanks");
        } else if let Some(index) = own
            .config
            .designator_pods
            .iter()
            .position(|p| p.station == own.selected)
        {
            let pod = own.config.designator_pods.remove(index);
            let mut weapon = inert_record();
            weapon.source = format!("JETTISON-{}", pod.store.source);
            weapon.name = pod.store.name;
            weapon.shape = Some(body_shape(store_sizes::PLAIN));
            stores.push((
                weapon,
                own.selected,
                own.config.stations[own.selected].mount,
            ));
            own.config
                .refresh_tanks()
                .expect("removing validated designator");
            own.cockpit.sight = own
                .config
                .sight_sensor()
                .map(super::super::sight::Sight::new);
            own.cockpit.laser = Default::default();
        } else if let Some(station) = own
            .config
            .stations
            .get(own.selected)
            .filter(|s| !s.internal)
        {
            let mut weapon = station.weapon.clone();
            let pod = own.config.gun_pod(own.selected);
            let count = pod.map_or_else(
                || {
                    own.rounds(own.selected)
                        .div_ceil(pod_rounds(&weapon).max(1) as u16)
                },
                |pod| {
                    if own.was_loaded(own.selected) {
                        pod.quantity
                    } else {
                        0
                    }
                },
            );
            if super::super::ballistics::is_pod(&weapon) || super::is_gun_pod(&weapon) {
                weapon.shape = Some(body_shape(store_sizes::pod_body(&weapon.source)));
            } else if weapon.shape.is_none() {
                weapon.shape = Some(body_shape(store_sizes::PLAIN));
            }
            weapon.source = format!("JETTISON-{}", weapon.source);
            // No launch audio: one authoritative release emission covers the group.
            weapon.fire_sound = None;
            weapon.flags = super::super::ballistics::BOMB_FLAG;
            weapon.effects.smoke = [0; 5];
            weapon.damage.by_class = [0; 5];
            weapon.damage.collateral_radius = 0;
            weapon.damage.collateral_percent = 0;
            weapon.damage.fuze_arm_t = u16::MAX;
            for (side, items) in station
                .rack
                .sides(station.mount, count)
                .into_iter()
                .enumerate()
            {
                for slot in 0..items {
                    let mount = station.rack.store_point(
                        station.mount,
                        station.count,
                        side,
                        slot,
                        station.size(),
                    );
                    stores.push((weapon.clone(), own.selected, mount));
                }
            }
            unload(&mut own.ammo[own.cockpit.selected], 0);
            if own.config.gun_pod(own.selected).is_some() {
                own.ever_loaded[own.cockpit.selected] = false;
            }
            own.release();
        }
        if stores.is_empty() {
            return;
        }
        self.emit_sound(
            launcher.position,
            crate::acoustics::Kind::Blast(super::super::blast::JETTISON),
        );
        for (weapon, station, mount) in stores {
            if self.projectiles.len() >= MAX_PROJECTILES {
                break;
            }
            let position = std::array::from_fn(|i| {
                launcher.position[i]
                    + launcher.basis.right[i] * mount[0]
                    + launcher.basis.up[i] * mount[1]
                    + launcher.basis.forward[i] * mount[2]
            });
            // Agent-fitted separation push: 6 ft/s away from the belly.
            let velocity =
                std::array::from_fn(|i| launcher.velocity[i] - launcher.basis.up[i] * 6.);
            // An inert body falls on the shared bomb model: its own mass and
            // size, no retarder and no guidance.
            let body = crate::combat::bomb_flight::Body {
                retarder: 0.,
                steering: 0,
                guidance_seconds: 0.,
                ..crate::combat::bomb_flight::Body::of(&weapon)
            };
            let motion = BombMotion::new(ReleaseState {
                body,
                position,
                velocity,
            });
            let id = self.next_shot;
            self.next_shot += 1;
            self.bombs.insert(id, motion);
            self.jettisoned.insert(id, true);
            self.projectiles.push(Projectile {
                id,
                owner: own.aircraft,
                weapon: Some(weapon),
                station,
                guidance: None,
                motion: None,
                guidance_ticks: None,
                age: 0,
                incoming: None,
                position,
                previous: position,
                direction: unit(velocity),
                speed_f8: (missiles::length(velocity) * 256.) as i32,
                launched_t: (self.tick / 30) as u16,
                target: None,
                fall: FallState::default(),
                gun_round: None,
                tracer: false,
            });
        }
    }
}

/// Procedural geometry identifier retained by snapshots and recordings.
fn body_shape(body: [f64; 2]) -> String {
    format!("TORE-BODY:{:.6}:{:.6}", body[0], body[1])
}

fn inert_record() -> Weapon {
    use tore_formats::weapons::*;
    let zone = Zone {
        heading: 0,
        pitch: 0,
        minimum_range: 0,
        maximum_range: 0,
        minimum_altitude: i32::MIN,
        maximum_altitude: i32::MAX,
    };
    let seeker = Seeker {
        flags: [0; 2],
        signature: 0,
        look_down: 0,
        doppler_above: 0,
        doppler_below: 0,
        doppler_minimum_range: 0,
        all_aspect: 0,
        zones: [zone; 2],
        chaff_flare_chance: 0,
        deception_chance: 0,
    };
    Weapon {
        source: "SYNTHETIC.JT".into(),
        name: "Inert store".into(),
        hud_name: "INERT".into(),
        shape: None,
        fire_sound: None,
        native_callback: "".into(),
        flags: 0x10,
        object_flags: 0,
        weight: 0,
        movement: Movement {
            minimum_speed: 0,
            corner_speed: 0,
            maximum_speed: 0,
            acceleration: 0,
            deceleration: 0,
            initial_speed: 0,
            final_speed: 0,
            launch_retard: 0,
            ignite_t: 0,
            fuel_t: 0,
            remove_t: u16::MAX,
            powered_turn_rate: 0,
            unpowered_turn_rate: 0,
            performance_at_0: 0,
            performance_at_20: 0,
            cruise: [0; 4],
            jink: [0; 3],
        },
        burst: Burst {
            projectiles_in_pod: 1,
            actual_rounds_per_game: 1,
            game_rounds_in_burst: 1,
            game_rounds_in_carpet_burst: 0,
            game_burst_t: 0,
            reload_t: 0,
            startup_shots: 0,
            random_fire_percent: 0,
            offset_fire_percent: 0,
            offset_fire_heading: 0,
            offset_fire_pitch: 0,
            sine_pattern: [0; 4],
        },
        seeker,
        guidance: Guidance {
            track_t: 0,
            track_max_g_raw: 0,
            target_sun_chance: 0,
            max_aon: 0,
            chances: [100; 4],
            hit_modifiers: [0; 9],
        },
        damage: Damage {
            by_class: [0; 5],
            fuze_arm_t: u16::MAX,
            fuze_radius: 0,
            side_hit_fuze_failure: 0,
            collateral_radius: 0,
            collateral_percent: 0,
        },
        effects: Effects {
            object_explosion: 0,
            land_explosion: 0,
            water_explosion: 0,
            crater_size: 0,
            smoke: [0; 5],
            max_sound_distance: 0,
            frequency_adjustment: 0,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn launcher() -> Launcher {
        Launcher {
            position: [0., 5000., 0.],
            basis: Basis::new(0., 0., 0.),
            speed_fps: 500.,
            velocity: [30., 0., 500.],
            bay_ready: true,
            radar_power: true,
            radar: true,
            jammer: false,
            alive: true,
            body_present: true,
            controls: sensors::Controls::default(),
        }
    }
    fn loaded() -> State {
        let mut config = super::super::tests::fixture(true)
            .own()
            .configuration()
            .clone();
        config.tanks = [([10., -2., 0.], 2), ([0., -3., 0.], 1)]
            .into_iter()
            .enumerate()
            .map(|(hardpoint, (mount, quantity))| TankStation {
                hardpoint,
                mount,
                quantity,
                rack: Rack::default(),
                store: Some(TankStore {
                    source: "SYN.GAS".into(),
                    name: "Tank".into(),
                    tank: tore_formats::weapons::Tank {
                        empty_weight: 100,
                        fuel_weight: 1000,
                        flags: 1,
                    },
                }),
            })
            .collect();
        config.refresh_tanks().unwrap();
        State::new(config, true).unwrap()
    }
    #[test]
    fn every_added_tank_row_drops_its_full_mass_without_weapons() {
        for (row, name, count) in [
            (6, "F610.GAS", 1),
            (8, "F600.GAS", 2),
            (8, "M396.GAS", 1),
            (9, "R528.GAS", 1),
        ] {
            let mut config = loaded().own().configuration().clone();
            config.tanks.truncate(1);
            let bytes = tore_formats::aircraft::stations::resource(name).unwrap();
            let tank = TankStore::parse(name, &bytes).unwrap();
            let mass = f64::from(count)
                * f64::from(tank.tank.fuel_weight + i32::from(tank.tank.empty_weight));
            config.tanks[0].store = Some(tank);
            config.tanks[0].hardpoint = row;
            config.tanks[0].quantity = count;
            config.refresh_tanks().unwrap();
            let mut state = State::new(config, true).unwrap();
            let ammo = state.own().ammo.clone();
            let before = state.own().payload_lbs();
            state.command(0, Command::JettisonTanks, launcher());
            assert_eq!(state.own().external_fuel_lbs(), [0.; 9]);
            assert_eq!(state.own().payload_lbs(), before - mass);
            assert_eq!(state.own().ammo, ammo);
            assert_eq!(state.projectiles.len(), usize::from(count));
        }
    }

    #[test]
    fn appended_wet_row_jettisons_and_duplicate_fuel_banks_are_rejected() {
        let mut config = loaded().own().configuration().clone();
        config.tanks[0].hardpoint = 6;
        config.tanks[1].hardpoint = 9;
        config.refresh_tanks().unwrap();
        assert_eq!(config.external_fuel_lbs[0], 1000.);
        let mut state = State::new(config.clone(), true).unwrap();
        state.command(0, Command::JettisonTanks, launcher());
        assert_eq!(state.own().external_fuel_lbs(), [0.; 9]);
        assert_eq!(state.projectiles.len(), 3);
        config.tanks[0].hardpoint = 0;
        assert!(config.refresh_tanks().is_err());
    }

    #[test]
    fn tanks_leave_weapons_alone_and_drop_all_shell_and_fuel_mass() {
        let mut s = loaded();
        let ammo = s.own().ammo.clone();
        let mass = s.own().payload_lbs();
        s.command(0, Command::JettisonTanks, launcher());
        assert_eq!(s.own().ammo, ammo);
        assert_eq!(s.own().payload_lbs(), mass - 3300.);
        assert_eq!(s.own().external_fuel_lbs(), [0.; 9]);
        assert!(
            s.own()
                .configuration()
                .tanks
                .iter()
                .all(|t| t.quantity == 0)
        );
        assert_eq!(s.projectiles.len(), 3);
        assert_eq!(s.bombs[&s.projectiles[0].id].velocity(0.), [30., -6., 500.]);
        assert_eq!(
            s.take_sound_events()[0].kind,
            crate::acoustics::Kind::Blast(crate::combat::blast::JETTISON)
        );
        s.command(0, Command::JettisonTanks, launcher());
        assert_eq!(s.projectiles.len(), 3);
        assert!(s.take_sound_events().is_empty());
    }
    #[test]
    fn selected_designator_falls_once_and_removes_its_mass_and_laser() {
        use crate::combat::sight::{DesignatorPod, DesignatorStore, Gimbal, SightSensor};
        let mut s = loaded();
        let own = s.own_mut();
        own.ammo[0] = 0;
        own.config.designator_pods.push(DesignatorPod {
            station: 0,
            store: DesignatorStore {
                source: "TEST.SEE".into(),
                name: "Designator".into(),
                weight_lbs: 400,
                flags: 2,
                sensor: SightSensor {
                    laser: true,
                    range_ft: 60_000.,
                    gimbal: Gimbal::Hemisphere,
                    eye: [0.; 3],
                    default_look: [0.; 2],
                },
            },
        });
        own.config.refresh_tanks().unwrap();
        let mass = own.payload_lbs();
        s.command(0, Command::Jettison, launcher());
        assert_eq!(s.own().payload_lbs(), mass - 400.);
        assert!(s.own().configuration().designator_pods.is_empty());
        assert_eq!(s.projectiles.len(), 1);
        assert_eq!(
            s.projectiles[0].weapon.as_ref().unwrap().source,
            "JETTISON-TEST.SEE"
        );
        s.take_sound_events();
        s.command(0, Command::Jettison, launcher());
        assert_eq!(s.projectiles.len(), 1);
        assert!(s.take_sound_events().is_empty());
    }

    #[test]
    fn selected_stores_fall_without_detonation_or_damage_and_leave_small_craters() {
        let mut s = loaded();
        let mass = s.own().payload_lbs();
        let rounds = s.own().rounds(0);
        s.command(0, Command::Jettison, launcher());
        assert_eq!(s.projectiles.len(), usize::from(rounds));
        assert_eq!(s.own().rounds(0), 0);
        assert!(s.own().payload_lbs() < mass);
        assert_eq!(s.own().external_fuel_lbs().iter().sum::<f64>(), 3000.);
        // The inert body lands when the shared bomb model says it does.
        let motion = s.bomb_motion(s.projectiles[0].id).unwrap();
        let impact = crate::combat::ballistics::predict_impact(
            crate::combat::ballistics::ReleaseState {
                body: motion.body,
                position: motion.position(0.),
                velocity: motion.velocity(0.),
            },
            |_, _| 0.,
        )
        .unwrap();
        let seconds = impact.seconds;
        let mut ticks = 0;
        while !s.projectiles.is_empty() && ticks < 3600 {
            let events = s.step(
                &[OwnshipInput {
                    aircraft: 0,
                    held: false,
                    launcher: launcher(),
                }],
                |_, _| 0.,
            );
            assert!(events.iter().all(|e| !matches!(
                e,
                Event::Hit(_) | Event::Destroyed(_) | Event::OwnshipDamaged { .. } | Event::Jolt(_)
            )));
            assert!(s.ground_bursts.is_empty());
            ticks += 1;
        }
        assert!(s.projectiles.is_empty());
        assert!((ticks as f64 / 120. - seconds).abs() < 0.1);
        assert_eq!(s.marks.len(), usize::from(rounds));
        assert!(s.marks.iter().all(|m| matches!(
            m.kind,
            crate::combat::blast::MarkKind::Crater(crate::combat::blast::POCK)
        )));
        assert!(
            s.effects
                .iter()
                .all(|e| e.scale == crate::combat::blast_scale::INERT)
        );
        assert!(
            s.marks
                .iter()
                .any(|m| (m.position[0] - impact.position[0]).abs() < 1.
                    && (m.position[2] - impact.position[2]).abs() < 1.)
        );
    }
    #[test]
    fn water_jettison_splashes_without_a_crater_or_damage() {
        let mut s = loaded();
        let mut at = launcher();
        at.position[1] = 20.;
        s.command(0, Command::JettisonTanks, at);
        for _ in 0..240 {
            s.step_surface(
                &[OwnshipInput {
                    aircraft: 0,
                    held: false,
                    launcher: at,
                }],
                |_, _| 0.,
                |_, _| true,
            );
            if s.projectiles.is_empty() {
                break;
            }
        }
        assert!(s.projectiles.is_empty());
        assert!(s.marks.is_empty());
        assert!(
            s.effects
                .iter()
                .all(|e| e.blast == Some(17) && e.scale == crate::combat::blast_scale::INERT)
        );
    }
    #[test]
    fn jettison_audio_uses_retail_release_and_small_ground_impact() {
        assert!(
            tore_formats::aircraft::COMBAT_RESOURCES
                .contains(&crate::combat::blast::JETTISON_SOUND)
        );
        assert!(crate::combat::blast::explosion(crate::combat::blast::JETTISON).is_none());
        assert_eq!(
            crate::acoustics::Kind::Blast(crate::combat::blast::JETTISON).parameters(),
            (100., 7000., crate::acoustics::FULL_LEVEL)
        );
        assert_eq!(
            crate::combat::blast::recording(crate::combat::blast::JETTISON, [0.; 3]),
            Some("&BOMB.11K")
        );
        assert!(matches!(
            crate::combat::blast::recording(15, [0.; 3]),
            Some("&BULLTS1.5K" | "&BULLTS4.5K")
        ));
        assert_eq!(
            crate::combat::blast::recording(17, [0.; 3]),
            Some("&SPLASH3.11K")
        );
    }
}
