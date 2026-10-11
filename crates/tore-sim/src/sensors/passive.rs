//! Passive emitter reception for the RCS instrument. It reports what reaches
//! the aircraft, not what any emitter knows. Nothing here asserts a lock, a
//! detection or a friend/foe classification, and no autonomous behaviour is
//! introduced.
use super::detection::Sighting;
use super::track::{Contact, Environment, Observable, Observer};

/// Maximum passive reception distance, matching the instrument's own largest
/// scale. It is a fitted gameplay receiver, not measured sensitivity.
pub const RECEIVER_LIMIT_NMI: f64 = 50.;
/// Scales offered by the instrument, reusing the existing warning-receiver set.
pub const SCALE_LADDER_NMI: [f64; 5] = [5., 10., 20., 30., 50.];
pub const DEFAULT_SCALE_INDEX: usize = 4;
/// The least height above the terrain a ground emitter is received from,
/// feet: its antenna (fitted, docs/spec/surface-defenses.md).
pub const GROUND_ANTENNA_FT: f64 = 10.;

/// What kind of symbol the instrument may draw. Anything not established by
/// available data stays an unknown emitter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Symbol {
    Unknown,
    Aircraft,
    Ground,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Emitter {
    pub id: u32,
    pub bearing_rad: f64,
    /// Present only when range is independently available from our own
    /// sensors. Passive noise alone never supplies range.
    pub distance_nmi: Option<f64>,
    pub symbol: Symbol,
    /// Normalized received strength, for symbol emphasis only.
    pub received: f64,
}

/// Collect received emitters. Reception needs a powered radio-frequency
/// emission, terrain visibility and the receiver limit above.
pub fn emitters(
    observer: &Observer,
    targets: &[Observable],
    contacts: &[Contact],
    environment: &Environment<'_>,
) -> Vec<Emitter> {
    let mut sorted: Vec<&Observable> = targets.iter().collect();
    sorted.sort_by_key(|t| t.id);
    let mut result = Vec::new();
    for target in sorted {
        // Reception needs a powered radio-frequency emission, so a device
        // without the radar-deception mode is not an emitter here either.
        let jamming = target.jammer_active
            && target
                .jammer
                .as_ref()
                .is_some_and(|j| j.radio_frequency && j.strength > 0.);
        let emitting = jamming || target.radar_emitting;
        // A surface emitter: the radar of a ground unit, a battery or a ship,
        // which is never airborne (docs/spec/surface-defenses.md, "RWR
        // emitters and radar state"). Anything else not airborne, a wreck on
        // the ground, is silent.
        let ground = !target.airborne;
        if !emitting || (ground && (target.destroyed || !target.radar_emitting)) {
            continue;
        }
        let sighting = Sighting::new(observer.position, &observer.basis, target.position);
        let distance = sighting.distance_nmi();
        if !distance.is_finite() || distance > RECEIVER_LIMIT_NMI {
            continue;
        }
        // A ground radar's antenna stands above the ground: the sight line
        // ends at least [`GROUND_ANTENNA_FT`] above the terrain under it, so a
        // unit whose contact volume sits low on a slope is not hidden by the
        // ground it stands on.
        let end = if ground {
            let [x, y, z] = target.position;
            [x, y.max((environment.ground)(x, z) + GROUND_ANTENNA_FT), z]
        } else {
            target.position
        };
        if (environment.obscured)(observer.position, end) {
            continue;
        }
        // Range is plotted only for an emitter our own sensors also observe.
        // The interference field's true position is never converted into one.
        let ranged = contacts
            .iter()
            .find(|c| c.id == target.id)
            .map(|c| c.distance_ft / super::profile::FEET_PER_NAUTICAL_MILE);
        result.push(Emitter {
            id: target.id,
            bearing_rad: sighting.heading_relative_bearing(observer.basis.angles()[0]),
            distance_nmi: ranged,
            symbol: if ground {
                Symbol::Ground
            } else if ranged.is_some() {
                Symbol::Aircraft
            } else {
                Symbol::Unknown
            },
            received: (1. / (1. + distance)).clamp(0., 1.),
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attitude::Basis;
    use crate::sensors::{
        profile::{Generation, JammerProfile},
        signature::{Configuration, SignatureProfile},
        track::Channel,
    };
    fn observer() -> Observer {
        Observer {
            position: [0., 10_000., 0.],
            basis: Basis::new(0., 0., 0.),
            radar_powered: true,
            radar_failed: false,
            infrared_failed: false,
            visual_failed: false,
        }
    }
    fn target(id: u32, position: [f64; 3], jamming: bool) -> Observable {
        Observable {
            id,
            position,
            velocity: [0.; 3],
            basis: Basis::new(0., 0., 0.),
            configuration: Configuration::CLEAN,
            signature: SignatureProfile::default(),
            jammer: Some(JammerProfile {
                record: "TEST.ECM".into(),
                generation: Generation::Early,
                strength: 0.3,
                band: 0,
                radio_frequency: true,
            }),
            jammer_active: jamming,
            radar_emitting: false,
            airborne: true,
            destroyed: false,
        }
    }
    fn environment<'a>(
        ground: &'a dyn Fn(f64, f64) -> f64,
        obscured: &'a dyn Fn([f64; 3], [f64; 3]) -> bool,
    ) -> Environment<'a> {
        Environment { ground, obscured }
    }
    #[test]
    fn a_jamming_emitter_without_a_sensor_contact_has_no_range() {
        let clear = |_: [f64; 3], _: [f64; 3]| false;
        let ground = |_: f64, _: f64| 0.;
        let e = environment(&ground, &clear);
        let targets = [target(3, [0., 10_000., 60_760.], true)];
        let found = emitters(&observer(), &targets, &[], &e);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].distance_nmi, None);
        assert_eq!(found[0].symbol, Symbol::Unknown);
        assert!(found[0].bearing_rad.abs() < 1e-9);
    }
    #[test]
    fn a_known_contact_supplies_range_and_an_aircraft_symbol() {
        let clear = |_: [f64; 3], _: [f64; 3]| false;
        let ground = |_: f64, _: f64| 0.;
        let e = environment(&ground, &clear);
        let targets = [target(3, [0., 10_000., 60_760.], true)];
        let contacts = [Contact {
            id: 3,
            channel: Channel::Radar,
            bearing_rad: 0.,
            elevation_rad: 0.,
            distance_ft: 60_760.,
            position: targets[0].position,
            velocity: [0.; 3],
            track_eligible: true,
            destroyed: false,
        }];
        let found = emitters(&observer(), &targets, &contacts, &e);
        assert_eq!(found[0].symbol, Symbol::Aircraft);
        assert!((found[0].distance_nmi.expect("ranged") - 10.).abs() < 1e-9);
    }
    #[test]
    fn silent_masked_and_distant_emitters_are_not_received() {
        let ground = |_: f64, _: f64| 0.;
        let clear = |_: [f64; 3], _: [f64; 3]| false;
        let blocked = |_: [f64; 3], _: [f64; 3]| true;
        let silent = [target(1, [0., 10_000., 60_760.], false)];
        assert!(emitters(&observer(), &silent, &[], &environment(&ground, &clear)).is_empty());
        let jamming = [target(1, [0., 10_000., 60_760.], true)];
        assert!(emitters(&observer(), &jamming, &[], &environment(&ground, &blocked)).is_empty());
        let distant = [target(1, [0., 10_000., 60_760. * 51.], true)];
        assert!(emitters(&observer(), &distant, &[], &environment(&ground, &clear)).is_empty());
        let mut grounded = jamming.clone();
        grounded[0].airborne = false;
        assert!(emitters(&observer(), &grounded, &[], &environment(&ground, &clear)).is_empty());
    }
    #[test]
    fn a_surface_radar_is_a_ground_emitter_until_it_is_destroyed() {
        let ground = |_: f64, _: f64| 0.;
        let clear = |_: [f64; 3], _: [f64; 3]| false;
        let e = environment(&ground, &clear);
        let mut radar = target(4, [0., 30., 60_760.], false);
        radar.airborne = false;
        radar.radar_emitting = true;
        let found = emitters(&observer(), std::slice::from_ref(&radar), &[], &e);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].symbol, Symbol::Ground);
        assert_eq!(found[0].distance_nmi, None);
        // The ground square stays a square when our sensors range it too.
        let contacts = [Contact {
            id: 4,
            channel: Channel::Radar,
            bearing_rad: 0.,
            elevation_rad: 0.,
            distance_ft: 60_760.,
            position: radar.position,
            velocity: [0.; 3],
            track_eligible: false,
            destroyed: false,
        }];
        let ranged = emitters(&observer(), std::slice::from_ref(&radar), &contacts, &e);
        assert_eq!(ranged[0].symbol, Symbol::Ground);
        assert!(ranged[0].distance_nmi.is_some());
        // Off, destroyed, masked or only jamming: not received.
        for change in [
            |t: &mut Observable| t.radar_emitting = false,
            |t: &mut Observable| t.destroyed = true,
            |t: &mut Observable| {
                t.radar_emitting = false;
                t.jammer_active = true;
            },
        ] {
            let mut silent = radar.clone();
            change(&mut silent);
            assert!(emitters(&observer(), &[silent], &[], &e).is_empty());
        }
        let blocked = |_: [f64; 3], _: [f64; 3]| true;
        assert!(emitters(&observer(), &[radar], &[], &environment(&ground, &blocked)).is_empty());
    }
    #[test]
    fn bearings_are_heading_relative_rather_than_body_relative() {
        let ground = |_: f64, _: f64| 0.;
        let clear = |_: [f64; 3], _: [f64; 3]| false;
        let mut o = observer();
        o.basis = Basis::new(0., 0., std::f64::consts::FRAC_PI_2);
        let right = [60_760., 10_000., 0.];
        let found = emitters(
            &o,
            &[target(1, right, true)],
            &[],
            &environment(&ground, &clear),
        );
        assert!((found[0].bearing_rad - std::f64::consts::FRAC_PI_2).abs() < 1e-9);
    }
    #[test]
    fn a_ground_emitter_sunk_into_a_slope_is_received_from_its_antenna() {
        // The terrain rises to 100 ft under the radar, whose aim point sits
        // 2 ft below it; a sight line to the aim point grazes the hill.
        let ground = |_: f64, z: f64| if z > 60_000. { 100. } else { 0. };
        let terrain = |from: [f64; 3], to: [f64; 3]| {
            (1..64).any(|i| {
                let t = f64::from(i) / 64.;
                let p: [f64; 3] = std::array::from_fn(|k| from[k] + (to[k] - from[k]) * t);
                p[1] <= ground(p[0], p[2])
            }) || to[1] <= ground(to[0], to[2])
        };
        let e = environment(&ground, &terrain);
        let mut radar = target(4, [0., 98., 60_760.], false);
        radar.airborne = false;
        radar.radar_emitting = true;
        let found = emitters(&observer(), std::slice::from_ref(&radar), &[], &e);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].symbol, Symbol::Ground);
        // An aircraft is still tested at its own position.
        let mut low = target(5, [0., 98., 60_760.], true);
        low.radar_emitting = true;
        assert!(emitters(&observer(), std::slice::from_ref(&low), &[], &e).is_empty());
    }
}
