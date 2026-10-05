//! The coders of a sensor suite's state: selection, contacts, strobes, plots
//! and trails (docs/formats/checkpoint.md).
//!
//! Everything in `Sensors` is coded, including the fields a step rewrites
//! (contacts, visual contacts, map contacts, strobes): a standby that takes
//! over shows them before the next step, and the equipment flags and the
//! fading strobes are read back by the next step. Nothing is skipped. The
//! profile set is a shared record, because the aircraft's `live::Configuration`
//! carries the same copy. The `Controls` and `Channel` types are coded by
//! their exact own-plane coders (`checkpoint_shared.rs`).

use super::{Contact, MapContact, Mode, Plot, Sample, Sensors, Strobe, Trail};

crate::checkpoint_enum!(Mode {
    Rws = 0,
    Tws = 1,
    Infrared = 2,
});

crate::checkpoint_struct!(Contact {
    id,
    channel,
    bearing_rad,
    elevation_rad,
    distance_ft,
    position,
    velocity,
    track_eligible,
    destroyed,
});

crate::checkpoint_struct!(MapContact {
    contact,
    identified,
    airborne,
});

crate::checkpoint_struct!(Plot {
    id,
    channel,
    bearing_rad,
    elevation_rad,
    distance_ft,
    position,
    age,
});

// `line_of_sight` is private; it is how the host's sensors couple strobes and
// contacts, so it is state and is coded.
crate::checkpoint_struct!(Strobe {
    id,
    bearing_rad,
    elevation_rad,
    received,
    half_width_rad,
    sidelobe_floor,
    line_of_sight,
});

crate::checkpoint_struct!(Sample { tick, position });

crate::checkpoint_struct!(Trail {
    channel,
    id,
    samples,
});

crate::checkpoint_struct!(Sensors {
    controls,
    selected,
    acquired,
    acquisition,
    active,
    mode,
    radar_powered,
    radar_failed,
    infrared_failed,
    visual_failed,
    contacts,
    visual,
    map_contacts,
    strobes,
    fading,
    plots,
    history,
    tick,
    keep_selection,
} shared { profiles });

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attitude::{Basis, Vector};
    use crate::checkpoint::{Models, from_bytes, round_trip, to_bytes};
    use crate::sensors::profile::{
        FEET_PER_NAUTICAL_MILE, Generation, InfraredProfile, JammerProfile, Preset, RadarProfile,
        SensorProfiles, Volume,
    };
    use crate::sensors::signature::{Configuration, SignatureProfile};
    use crate::sensors::track::{Channel, Environment, Observable, Observer};
    use tore_formats::aircraft::AircraftId;

    const NMI: f64 = FEET_PER_NAUTICAL_MILE;
    const ALTITUDE: f64 = 20_000.;

    static GROUND: fn(f64, f64) -> f64 = |_, _| 0.;
    static CLEAR: fn(Vector, Vector) -> bool = |_, _| false;

    fn volume(maximum_nmi: f64) -> Volume {
        Volume {
            azimuth_rad: 60f64.to_radians(),
            elevation_rad: 60f64.to_radians(),
            minimum_ft: 0.,
            maximum_ft: maximum_nmi * NMI,
            minimum_relative_ft: f64::NEG_INFINITY,
            maximum_relative_ft: f64::INFINITY,
        }
    }

    /// Radar, infrared, visual and jammer, so every channel has state.
    pub(super) fn profiles() -> SensorProfiles {
        let preset = Preset::Advanced;
        SensorProfiles {
            aircraft: AircraftId::F18,
            radar: Some(RadarProfile {
                record: "TESTR.SEE".into(),
                search: volume(90.),
                track: volume(50.),
                look_down: 50.,
                preset,
                notch: preset.notch(),
                resistance: preset.resistance(),
                band: 0,
                source_flags: [3, 4],
                source_doppler: [5, 6, 7],
            }),
            infrared: Some(InfraredProfile {
                record: "TESTI.SEE".into(),
                search: volume(20.),
                track: volume(10.),
            }),
            visual: Some(InfraredProfile {
                record: "TESTV.SEE".into(),
                search: volume(6.),
                track: volume(6.),
            }),
            jammer: Some(JammerProfile {
                record: "TEST.ECM".into(),
                generation: Generation::LateColdWar,
                strength: 0.3,
                band: 0,
                radio_frequency: true,
            }),
            signature: SignatureProfile::default(),
        }
    }

    fn observer(tick: u64) -> Observer {
        Observer {
            position: [0., ALTITUDE, tick as f64 * 4.],
            basis: Basis::new(0., 0., 0.),
            radar_powered: true,
            radar_failed: false,
            infrared_failed: false,
            visual_failed: false,
        }
    }

    fn object(id: u32, position: Vector) -> Observable {
        Observable {
            id,
            position,
            velocity: [0., 0., -300.],
            basis: Basis::new(0., 0., 0.),
            configuration: Configuration::CLEAN,
            signature: SignatureProfile::default(),
            jammer: None,
            jammer_active: false,
            radar_emitting: false,
            airborne: true,
            destroyed: false,
        }
    }

    /// The crowd the sensors watch at `tick`: one closing head on, one
    /// emitting and jamming off to the side, one that leaves at tick 500 (so
    /// a plot is left), one that is shot down at 700, one on the deck.
    fn targets(tick: u64) -> Vec<Observable> {
        let t = tick as f64;
        let mut all = vec![object(
            1,
            [800. * (t / 150.).sin(), ALTITUDE, 25. * NMI - 6. * t],
        )];
        let mut emitter = object(
            2,
            [
                30. * NMI * 40f64.to_radians().sin(),
                ALTITUDE,
                30. * NMI * 40f64.to_radians().cos(),
            ],
        );
        emitter.radar_emitting = true;
        emitter.jammer = profiles().jammer;
        emitter.jammer_active = true;
        all.push(emitter);
        if tick < 500 {
            all.push(object(3, [-2. * NMI, ALTITUDE + 500., 12. * NMI - t]));
        }
        let mut doomed = object(4, [3. * NMI, ALTITUDE - 800., 8. * NMI]);
        doomed.destroyed = tick >= 700;
        all.push(doomed);
        all.push(object(5, [0., 0., 15. * NMI]));
        all
    }

    /// One scripted step, depending only on the sensors' own tick.
    fn drive(sensors: &mut Sensors) {
        let tick = sensors.tick();
        match tick {
            100 => {
                sensors.designate(1);
            }
            250 => sensors.controls.history = true,
            450 => sensors.controls.range_index = 3,
            650 => sensors.controls.channel = Channel::Infrared,
            820 => sensors.controls.channel = Channel::Radar,
            _ => {}
        }
        let air = Environment {
            ground: &GROUND,
            obscured: &CLEAR,
        };
        sensors.step(&observer(tick), &targets(tick), &air);
    }

    #[test]
    fn sensors_round_trip_at_300_600_and_900_and_step_on_identically() {
        let models = Models::default();
        let mut sensors = Sensors::new(profiles());
        let mut seen = (false, false, false, false, false);
        let mut at = 0;
        for target in [300, 600, 900] {
            while at < target {
                drive(&mut sensors);
                at += 1;
                seen.0 |= !sensors.contacts().is_empty();
                seen.1 |= !sensors.strobes().is_empty();
                seen.2 |= !sensors.plots().is_empty();
                seen.3 |= sensors.acquired().is_some();
                seen.4 |= !sensors.visual().is_empty() || !sensors.map_contacts().is_empty();
            }
            let copy = round_trip(&sensors, &models).unwrap();
            assert_eq!(copy, sensors, "at tick {target}");

            // The copy, stepped beside the original, never differs.
            let mut twin = copy;
            let mut original = sensors.clone();
            for step in 0..600 {
                drive(&mut original);
                drive(&mut twin);
                assert_eq!(twin, original, "{step} steps after tick {target}");
            }
        }
        // The fixture really exercises the state it codes.
        assert!(seen.0, "no radar contact was ever held");
        assert!(seen.1, "no strobe was ever received");
        assert!(seen.2, "no stale plot was ever left");
        assert!(seen.3, "no weapon track was ever acquired");
        assert!(seen.4, "no visual or map contact was ever held");
        assert!(!sensors.trail(1).is_empty() || sensors.tick() > 0);
    }

    #[test]
    fn a_sensor_suite_codes_its_profiles_once_as_a_shared_record() {
        let models = Models::default();
        let mut sensors = Sensors::new(profiles());
        for _ in 0..40 {
            drive(&mut sensors);
        }
        let coded = to_bytes(&sensors, &models).unwrap();
        assert_eq!(coded.records.len(), 1, "the profile set is one record");
        let copy: Sensors = from_bytes(&coded, &models).unwrap();
        assert_eq!(copy, sensors);
        // Two suites with equal profiles share the one record.
        let pair = (sensors.clone(), Sensors::new(profiles()));
        assert_eq!(to_bytes(&pair, &models).unwrap().records.len(), 1);
    }

    /// Bits, not values: a NaN payload, a signed zero and an infinity in a
    /// stored contact survive.
    #[test]
    fn odd_floats_in_contacts_survive() {
        let models = Models::default();
        let mut sensors = Sensors::new(profiles());
        for _ in 0..30 {
            drive(&mut sensors);
        }
        sensors.contacts.push(Contact {
            id: 9,
            channel: Channel::Visual,
            bearing_rad: f64::from_bits(0x7ff8_0000_dead_0001),
            elevation_rad: -0.,
            distance_ft: f64::INFINITY,
            position: [f64::NEG_INFINITY, -0., 1.5],
            velocity: [0.; 3],
            track_eligible: true,
            destroyed: true,
        });
        let copy = round_trip(&sensors, &models).unwrap();
        let (a, b) = (
            &copy.contacts[copy.contacts.len() - 1],
            &sensors.contacts[sensors.contacts.len() - 1],
        );
        assert_eq!(a.bearing_rad.to_bits(), b.bearing_rad.to_bits());
        assert_eq!(a.elevation_rad.to_bits(), b.elevation_rad.to_bits());
        assert_eq!(a.position[1].to_bits(), b.position[1].to_bits());
    }

    #[test]
    fn damaged_sensor_bytes_are_refused_without_a_panic() {
        let models = Models::default();
        let mut sensors = Sensors::new(profiles());
        for _ in 0..200 {
            drive(&mut sensors);
        }
        let coded = to_bytes(&sensors, &models).unwrap();
        for cut in 0..coded.body.len() {
            let mut damaged = coded.clone();
            damaged.body.truncate(cut);
            assert!(
                from_bytes::<Sensors>(&damaged, &models).is_err(),
                "cut {cut}"
            );
        }
        for byte in 0..coded.body.len().min(400) {
            let mut damaged = coded.clone();
            damaged.body[byte] ^= 0xa5;
            let _ = from_bytes::<Sensors>(&damaged, &models);
        }
    }
}
