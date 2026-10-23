//! The external stores an aircraft carries right now, as the radar signature
//! reads them (docs/radar.md, "External stores"). One count for a human seat,
//! an AI aircraft, another player's aircraft and the RCS page, so every
//! radar, jammer comparison, missile seeker and the exposure contour sees the
//! same loadout. Agent decision, 2026-10-16 (John asked that ordnance raise
//! detectability).
use super::*;
use crate::sensors::Stores;

impl Configuration {
    /// The stores hanging outside the aircraft when station `s` holds
    /// `rounds(s)` (the live count, rockets for a rocket pod) and
    /// `pod_fitted(s)` says whether its gun pod is still on.
    ///
    /// Counted: selected tanks (their full weight, fuel capacity included),
    /// each external store a station still holds (a rocket pod while it has a
    /// rocket left, as jettison counts it), gun pods and designator pods.
    /// Not counted: bay stations (the open-bay term covers an open bay),
    /// internal guns and fixed-slot equipment, which the PT value already
    /// includes. A station in an unreviewed location counts as internal.
    pub fn external_stores(
        &self,
        rounds: impl Fn(usize) -> u32,
        pod_fitted: impl Fn(usize) -> bool,
    ) -> Stores {
        let mut count = 0u64;
        let mut lbs = 0f64;
        for tank in &self.tanks {
            if let Some(store) = &tank.store {
                count += u64::from(tank.quantity);
                lbs += f64::from(tank.quantity) * store.full_mass_lbs();
            }
        }
        for pod in &self.designator_pods {
            count += 1;
            lbs += f64::from(pod.store.weight_lbs.max(0));
        }
        for (index, station) in self.stations.iter().enumerate() {
            if station.internal || !station.rack.location().external() {
                continue;
            }
            if self.designator_pods.iter().any(|p| p.station == index) {
                continue;
            }
            if let Some(pod) = self.gun_pod(index) {
                if pod_fitted(index) {
                    count += u64::from(pod.quantity);
                    lbs += f64::from(pod.quantity) * f64::from(pod.weight_lbs.max(0));
                }
                continue;
            }
            if is_gun(&station.weapon) {
                continue;
            }
            let units = rounds(index).div_ceil(pod_rounds(&station.weapon).max(1));
            count += u64::from(units);
            lbs += f64::from(units) * f64::from(station.weapon.weight.max(0));
        }
        Stores {
            count: count as f64,
            lbs,
        }
    }
}

impl Ownship {
    /// [`Configuration::external_stores`] for this aircraft's live loadout.
    /// An aircraft flying without external stores fitted carries none.
    pub fn external_stores(&self) -> Stores {
        if !self.external {
            return Stores::NONE;
        }
        self.config.external_stores(
            |station| u32::from(self.rounds(station)),
            |station| self.was_loaded(station),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::bomb_tests::{lau61, mk82};
    use super::super::tests::fixture;
    use super::*;
    use crate::combat::hardpoints::{Location, Rack};
    use crate::sensors::detection::effective_range_ft;

    const NMI: f64 = crate::sensors::FEET_PER_NAUTICAL_MILE;

    fn rack(location: Location) -> Rack {
        Rack {
            location: location.byte(),
            ..Rack::default()
        }
    }

    fn station(weapon: Weapon, count: u16, location: Location) -> Station {
        Station {
            weapon,
            mount: [4., -3., 0.],
            count,
            internal: false,
            rack: rack(location),
        }
    }

    /// The authored 600 US gallon tank: 425 lb shell, 3,960 lb of fuel.
    fn f600() -> TankStore {
        TankStore {
            source: "F600.GAS".into(),
            name: "600 gal".into(),
            tank: tore_formats::weapons::Tank {
                empty_weight: 425,
                fuel_weight: 3960,
                flags: 0,
            },
        }
    }

    fn missile(weight: i32) -> Weapon {
        let mut w = fixture(true).own().configuration().stations[0]
            .weapon
            .clone();
        w.source = "AIM120.JT".into();
        w.weight = weight;
        w
    }

    /// A signature-10 aircraft (the F-22's PT value) with an internal gun,
    /// two bay missiles, an empty wing station and two empty tank pylons.
    fn raptor() -> Configuration {
        let mut config = fixture(false).own().configuration().clone();
        let mut gun = mk82();
        gun.source = "M61.JT".into();
        config.stations = vec![
            Station {
                internal: true,
                ..station(gun, 750, Location::InternalGun)
            },
            station(missile(345), 2, Location::InternalBay),
            station(missile(345), 0, Location::Wing),
        ];
        config.tanks = vec![TankStation {
            hardpoint: 8,
            mount: [12., -3., -6.],
            rack: rack(Location::Wing),
            store: Some(f600()),
            quantity: 0,
        }];
        config.refresh_tanks().unwrap();
        config.sensors.signature.radar = 10.;
        config
    }

    fn with_tanks(mut config: Configuration, quantity: u16) -> Configuration {
        config.tanks[0].quantity = quantity;
        config.refresh_tanks().unwrap();
        config
    }

    fn launcher() -> Launcher {
        let basis = crate::attitude::Basis::new(0., 0., 0.);
        Launcher {
            position: [0., 20_000., 0.],
            basis,
            speed_fps: 800.,
            velocity: basis.forward.map(|v| v * 800.),
            bay_ready: true,
            radar_power: true,
            radar: true,
            jammer: false,
            alive: true,
            body_present: true,
            controls: sensors::Controls::default(),
        }
    }

    /// Nose-on detection distance of `own` for a 90-mile search radar in
    /// clear air, through the row every other radar observes.
    fn nose_on_nmi(own: &Ownship) -> f64 {
        let row = ownship_target(own, launcher());
        let seen = observable_of(&row);
        let effective =
            seen.signature
                .effective_radar(&seen.basis, [0., 0., 1.], seen.configuration);
        effective_range_ft(90. * NMI, effective / 100., 1., 1., 1.) / NMI
    }

    #[test]
    fn a_clean_aircraft_and_its_bay_and_gun_carry_nothing_outside() {
        let own = Ownship::new(0, NO_SIDE, raptor(), true).unwrap();
        assert_eq!(own.rounds(1), 2, "the bay holds its missiles");
        assert_eq!(own.external_stores(), sensors::Stores::NONE);
        assert!((nose_on_nmi(&own) - 28.460_498_941_515_41).abs() < 1e-6);
    }

    #[test]
    fn external_tanks_and_rail_missiles_raise_the_signature_and_detection_range() {
        let tanks = Ownship::new(0, NO_SIDE, with_tanks(raptor(), 2), true).unwrap();
        let stores = tanks.external_stores();
        assert_eq!(stores.count, 2.);
        assert!((stores.lbs - 8770.).abs() < 1e-9);
        let profile = tanks.configuration().sensors.signature;
        // 2 x 3 per store + 8.77 x 2 per 1,000 lb.
        assert!((profile.store_signature(stores) - 23.54).abs() < 1e-9);
        // Clean 28.5 nmi; two 600 gallon tanks 52.1 nmi.
        assert!((nose_on_nmi(&tanks) - 52.122).abs() < 1e-3);

        let mut config = raptor();
        config.stations[2].count = 4;
        let rails = Ownship::new(0, NO_SIDE, config, true).unwrap();
        assert_eq!(rails.external_stores().count, 4.);
        // Four AIM-120s on the wing: 44.8 nmi.
        assert!((nose_on_nmi(&rails) - 44.783).abs() < 1e-3);
    }

    #[test]
    fn stores_stop_counting_once_fired_dropped_or_jettisoned() {
        let mut config = with_tanks(raptor(), 2);
        config.stations[2] = station(mk82(), 3, Location::Centerline);
        config.stations.push(station(lau61(), 2, Location::Wing));
        let state = State::new(config.clone(), true).unwrap();
        let mut own = Ownship::new(0, NO_SIDE, config, true).unwrap();
        // Two tanks, three bombs and two rocket pods of 19.
        assert_eq!(own.rounds(3), 38);
        assert_eq!(own.external_stores().count, 7.);
        // One bomb gone, and the first pod's rockets.
        own.ammo[2] = 2;
        own.ammo[3] = 19;
        assert_eq!(own.external_stores().count, 5.);
        // A pod with a rocket left still hangs.
        own.ammo[3] = 1;
        assert_eq!(own.external_stores().count, 5.);
        let mut state = state;
        state.jettison_ai(&mut own, launcher(), true, &[]);
        assert_eq!(own.external_stores().count, 3.);
        state.jettison_ai(&mut own, launcher(), true, &[2, 3]);
        assert_eq!(own.external_stores(), sensors::Stores::NONE);
        assert!((nose_on_nmi(&own) - 28.460_498_941_515_41).abs() < 1e-6);
        // An aircraft flying without external stores carries none.
        let bare = Ownship::new(0, NO_SIDE, with_tanks(raptor(), 2), false).unwrap();
        assert_eq!(bare.external_stores(), sensors::Stores::NONE);
    }

    #[test]
    fn a_pod_row_counts_its_missiles_until_the_gun_pod_takes_its_place() {
        use super::super::GunPod;
        // The F-4B's centreline: a row that can take the SUU-16 keeps an
        // empty pod entry whatever else it carries (2026-10-16).
        let mut config = raptor();
        config.stations[2].count = 4;
        config.gun_pods = vec![GunPod {
            station: 2,
            quantity: 0,
            rounds_per_pod: 600,
            weight_lbs: 1702,
        }];
        let missiles = Ownship::new(0, NO_SIDE, config.clone(), true).unwrap();
        assert_eq!(missiles.external_stores().count, 4.);
        assert!((missiles.external_stores().lbs - 4. * 345.).abs() < 1e-9);
        // The pod in their place: one store of 1,702 lb, pod hardware only.
        config.stations[2].weapon.source = "SUU16.JT".into();
        config.stations[2].weapon.weight = 0;
        config.stations[2].count = 1;
        config.gun_pods[0].quantity = 1;
        config.refresh_tanks().unwrap();
        let pod = Ownship::new(0, NO_SIDE, config, true).unwrap();
        assert_eq!(pod.external_stores().count, 1.);
        assert!((pod.external_stores().lbs - 1702.).abs() < 1e-9);
    }

    #[test]
    fn a_zero_signature_stays_zero_whatever_it_carries() {
        let mut config = with_tanks(raptor(), 2);
        config.sensors.signature.radar = 0.;
        let own = Ownship::new(0, NO_SIDE, config, true).unwrap();
        assert!(own.external_stores().count > 0.);
        assert_eq!(nose_on_nmi(&own), 0.);
    }

    #[test]
    fn every_consumer_reads_the_same_loaded_signature() {
        let own = Ownship::new(0, NO_SIDE, with_tanks(raptor(), 2), true).unwrap();
        let row = ownship_target(&own, launcher());
        let configuration = sensors::Configuration::carrying(own.external_stores());
        // The hit-test row, the observable radars and jammer comparisons
        // read, and the missile seekers' row all carry it.
        assert_eq!(row.configuration, configuration);
        assert_eq!(observable_of(&row).configuration, configuration);
        // The RCS page counts the same stores from the seat's readout.
        let readout = own
            .config
            .external_stores(|s| u32::from(own.rounds(s)), |s| own.was_loaded(s));
        assert_eq!(readout, own.external_stores());
        // Its contour samples the same function at the same configuration.
        let s = own.config.sensors.signature;
        let basis = crate::attitude::Basis::new(0., 0., 0.);
        let contour = s.exposure_contour(&basis, configuration, 5.);
        let nose = s.effective_radar(&basis, [0., 0., 1.], configuration);
        assert!((contour[0].1 - s.exposure_nmi(nose)).abs() < 1e-12);
        assert!(contour[0].1 > s.exposure_contour(&basis, sensors::Configuration::CLEAN, 5.)[0].1);
        // A radar missile seeker's range uses it too.
        let mut target = row.clone();
        target.position = [0., 20_000., 30_000.];
        let seen =
            target
                .signature
                .effective_radar(&target.basis, [0., 0., -1.], target.configuration);
        assert!((seen - nose).abs() < 1e-9);
    }
}
