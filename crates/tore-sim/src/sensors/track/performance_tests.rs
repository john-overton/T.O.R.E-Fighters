//! Independent pre-optimization observation and bounded visibility work.
use super::*;

impl Sensors {
    fn reference_observe(
        &self,
        observer: &Observer,
        target: &Observable,
        channel: Channel,
        environment: &Environment<'_>,
    ) -> Option<Contact> {
        let sighting = Sighting::new(observer.position, &observer.basis, target.position);
        if !sighting.distance_ft.is_finite()
            || (environment.obscured)(observer.position, target.position)
        {
            return None;
        }
        let (search, track) = match channel {
            Channel::Radar => {
                let radar = self.profiles.radar.as_ref()?;
                let to_observer: Vector =
                    std::array::from_fn(|i| observer.position[i] - target.position[i]);
                let effective = target.signature.effective_radar(
                    &target.basis,
                    to_observer,
                    target.configuration,
                );
                if effective <= 0. {
                    return None;
                }
                let relative = effective / 100.;
                let height = target.position[1]
                    - (environment.ground)(target.position[0], target.position[2]);
                let clutter = clutter_exposure(&sighting, height);
                let look_down = look_down_factor(radar.look_down, clutter);
                let notch = notch_factor(
                    &radar.notch,
                    clutter,
                    notch_speed_fps(&sighting, target.velocity),
                );
                let coupled: f64 = self
                    .strobes
                    .iter()
                    .map(|s| {
                        s.received
                            * angular_coupling(
                                &radar.resistance,
                                dot(s.line_of_sight, sighting.line_of_sight)
                                    .clamp(-1., 1.)
                                    .acos(),
                            )
                    })
                    .sum();
                let jammer = jammer_factor(interference_quotient(
                    &radar.resistance,
                    coupled,
                    sighting.distance_nmi(),
                    relative,
                ));
                (
                    (
                        &radar.search,
                        effective_range_ft(
                            radar.search.maximum_ft,
                            relative,
                            look_down,
                            notch,
                            jammer,
                        ),
                    ),
                    (
                        &radar.track,
                        effective_range_ft(
                            radar.track.maximum_ft,
                            relative,
                            look_down,
                            notch,
                            jammer,
                        ),
                    ),
                )
            }
            Channel::Infrared => {
                let infrared = self.profiles.infrared.as_ref()?;
                let signature = target.signature.infrared;
                (
                    (
                        &infrared.search,
                        infrared_range_ft(infrared.search.maximum_ft, signature),
                    ),
                    (
                        &infrared.track,
                        infrared_range_ft(infrared.track.maximum_ft, signature),
                    ),
                )
            }
            // The visual channel keeps the existing geometric contract: no
            // signature scaling, no interference and no weapon support.
            Channel::Visual => {
                let visual = self.profiles.visual.as_ref()?;
                (
                    (&visual.search, visual.search.maximum_ft),
                    (&visual.track, visual.track.maximum_ft),
                )
            }
        };
        let admits = |(volume, effective): (&crate::sensors::profile::Volume, f64)| {
            volume.admits_geometry(
                sighting.azimuth_rad,
                sighting.elevation_rad,
                sighting.distance_ft,
                sighting.relative_altitude_ft,
            ) && sighting.distance_ft <= effective
        };
        let in_track = admits(track);
        // A retained fire-control track may stay current inside its own
        // tracking envelope, which can reach past the search envelope. It can
        // never acquire an unseen target there.
        let retained = self.selected == Some(target.id) && self.acquired == Some(target.id);
        if !(admits(search) || (retained && in_track)) {
            return None;
        }
        Some(Contact {
            id: target.id,
            channel,
            bearing_rad: sighting.azimuth_rad,
            elevation_rad: sighting.elevation_rad,
            distance_ft: sighting.distance_ft,
            position: target.position,
            velocity: target.velocity,
            track_eligible: in_track,
            destroyed: target.destroyed,
        })
    }
}

fn fixture() -> (Sensors, Observer, Observable) {
    use crate::sensors::profile::{InfraredProfile, Preset, Volume};
    let volume = |range: f64| Volume {
        azimuth_rad: 60_f64.to_radians(),
        elevation_rad: 60_f64.to_radians(),
        minimum_ft: 0.,
        maximum_ft: range,
        minimum_relative_ft: -50_000.,
        maximum_relative_ft: 50_000.,
    };
    let passive = || InfraredProfile {
        record: "SYNTHETIC.SEE".into(),
        search: volume(50_000.),
        track: volume(80_000.),
    };
    let radar = RadarProfile {
        record: "SYNTHETIC.SEE".into(),
        search: volume(50_000.),
        track: volume(80_000.),
        look_down: 50.,
        preset: Preset::Advanced,
        notch: Preset::Advanced.notch(),
        resistance: Preset::Advanced.resistance(),
        band: 0,
        source_flags: [0; 2],
        source_doppler: [0; 3],
    };
    let sensors = Sensors::new(SensorProfiles {
        aircraft: tore_formats::aircraft::AircraftId::F18,
        radar: Some(radar),
        infrared: Some(passive()),
        visual: Some(passive()),
        jammer: None,
        signature: SignatureProfile::default(),
    });
    let observer = Observer {
        position: [0., 20_000., 0.],
        basis: Basis::new(0., 0., 0.),
        radar_powered: true,
        radar_failed: false,
        infrared_failed: false,
        visual_failed: false,
    };
    let target = Observable {
        id: 2,
        position: [0., 20_000., 20_000.],
        velocity: [0., 0., -500.],
        basis: Basis::new(180., 0., 0.),
        configuration: Configuration::CLEAN,
        signature: SignatureProfile::default(),
        jammer: None,
        jammer_active: false,
        radar_emitting: true,
        airborne: true,
        destroyed: false,
    };
    (sensors, observer, target)
}

fn bits(contact: Option<Contact>) -> Option<(u32, Channel, [u64; 9], bool, bool)> {
    contact.map(|c| {
        (
            c.id,
            c.channel,
            [
                c.bearing_rad.to_bits(),
                c.elevation_rad.to_bits(),
                c.distance_ft.to_bits(),
                c.position[0].to_bits(),
                c.position[1].to_bits(),
                c.position[2].to_bits(),
                c.velocity[0].to_bits(),
                c.velocity[1].to_bits(),
                c.velocity[2].to_bits(),
            ],
            c.track_eligible,
            c.destroyed,
        )
    })
}

#[test]
fn deferred_visibility_matches_the_original_observation_bits() {
    let (mut sensors, mut observer, mut target) = fixture();
    let ground = |x: f64, z: f64| (x * 0.001).sin() * 200. + (z * 0.002).cos() * 100.;
    for retained in [false, true] {
        sensors.selected = retained.then_some(target.id);
        sensors.acquired = retained.then_some(target.id);
        for channel in [Channel::Radar, Channel::Infrared, Channel::Visual] {
            for distance in [
                0., 10., 20_000., 49_999., 50_000., 50_001., 79_999., 80_000., 80_001., 1e6,
            ] {
                for angle in [-100_f64, -60., -20., 0., 45., 60., 100.] {
                    for altitude in [0., 5_000., 20_000., 80_001.] {
                        for blocked in [false, true] {
                            observer.basis = Basis::new(angle * 0.1, 3., -12.);
                            let bearing = angle.to_radians();
                            target.position =
                                [distance * bearing.sin(), altitude, distance * bearing.cos()];
                            let obscured = |_: Vector, _: Vector| blocked;
                            let environment = Environment {
                                ground: &ground,
                                obscured: &obscured,
                            };
                            assert_eq!(
                                bits(sensors.observe(&observer, &target, channel, &environment)),
                                bits(sensors.reference_observe(
                                    &observer,
                                    &target,
                                    channel,
                                    &environment
                                )),
                                "{retained} {channel:?} {distance} {angle} {altitude} {blocked}"
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn visibility_cache_resamples_changed_and_reversed_segments() {
    use std::cell::Cell;
    let from = [0., 10., 0.];
    let to = [20., 20., 0.];
    let calls = Cell::new(0);
    let query = |from: Vector, to: Vector| {
        calls.set(calls.get() + 1);
        from[1] > to[1]
    };
    let obscured = cached_obscured(&query);
    assert!(!obscured(from, to));
    assert!(!obscured(from, to));
    assert_eq!(calls.get(), 1);
    for (other_from, other_to) in [([0., 30., 0.], to), (from, [20., 5., 0.]), (to, from)] {
        let before = calls.get();
        assert!(obscured(other_from, other_to));
        assert_eq!(
            calls.get(),
            before + 1,
            "a different segment needs a fresh answer"
        );
        assert!(!obscured(from, to));
        assert_eq!(calls.get(), before + 1, "the first segment remains cached");
        assert!(obscured(other_from, other_to));
        assert_eq!(
            calls.get(),
            before + 2,
            "mismatches do not replace the cached pair"
        );
    }
}

#[test]
fn visibility_cache_compares_every_endpoint_bit() {
    use std::cell::Cell;
    let from: Vector = [0., 10., 20.];
    let to: Vector = [30., 0., 50.];
    let bits = |from: Vector, to: Vector| {
        [from[0], from[1], from[2], to[0], to[1], to[2]].map(f64::to_bits)
    };
    let original = bits(from, to);
    let calls = Cell::new(0);
    let query = |from, to| {
        calls.set(calls.get() + 1);
        bits(from, to) != original
    };
    let obscured = cached_obscured(&query);
    assert!(!obscured(from, to));
    for coordinate in 0..6 {
        let mut other_from = from;
        let mut other_to = to;
        let value = if coordinate < 3 {
            &mut other_from[coordinate]
        } else {
            &mut other_to[coordinate - 3]
        };
        *value = f64::from_bits(value.to_bits() + 1);
        assert!(
            obscured(other_from, other_to),
            "coordinate {coordinate} was rounded or omitted"
        );
    }
    assert!(obscured([-0., 10., 20.], to));
    assert!(obscured(from, [30., -0., 50.]));
    assert_eq!(
        calls.get(),
        9,
        "signed zeros are distinct at either endpoint"
    );
    assert!(!obscured(from, to));
    assert_eq!(calls.get(), 9);
}

#[test]
fn out_of_volume_contacts_do_not_query_terrain_and_channels_share_one_query() {
    use std::cell::Cell;
    let (mut sensors, observer, target) = fixture();
    let calls = Cell::new(0);
    let obscured = |_: Vector, _: Vector| {
        calls.set(calls.get() + 1);
        false
    };
    let ground = |_: f64, _: f64| 0.;
    let environment = Environment {
        ground: &ground,
        obscured: &obscured,
    };
    sensors.step(&observer, std::slice::from_ref(&target), &environment);
    assert_eq!(
        calls.get(),
        1,
        "radar and visual must share their identical ray"
    );
    let distant = Observable {
        position: [0., 20_000., 1e6],
        ..target
    };
    calls.set(0);
    sensors.step(&observer, std::slice::from_ref(&distant), &environment);
    assert_eq!(
        calls.get(),
        0,
        "range rejection must precede terrain sampling"
    );
    assert!(crate::sensors::passive::emitters(&observer, &[distant], &[], &environment).is_empty());
    assert_eq!(
        calls.get(),
        0,
        "passive receiver range rejection must precede terrain sampling"
    );
}
