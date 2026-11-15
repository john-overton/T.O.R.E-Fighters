//! Surface air-defence radar detection: the aircraft radar model seen from a
//! ground antenna (docs/radar.md, "Surface radars").
//!
//! A SAM battery's radar, a self-contained radar missile unit and a
//! radar-directed gun detect an aircraft with the rule the aircraft radars
//! use ([`super::track::radar_ranges_ft`]): the effective signature from the
//! antenna's own direction (aspect, gear, flaps, bay and external stores:
//! [`super::SignatureProfile::effective_radar`]), the square-root range law
//! on the record's range band (the band is the range against a signature-100
//! target), the look-down clutter and notch terms, and the received jammer
//! interference with its burn-through. Nothing here copies those rules.
//!
//! What the host adds is geometry it already owned: the record's range and
//! altitude band, the mount arcs and the terrain line of sight stay with the
//! surface controllers. There is no radar horizon term: the aircraft radars
//! have none either (the terrain is the only mask), so a flat-earth line of
//! sight is the same model.
//!
//! Deterministic by construction: pure functions of this tick's positions and
//! the aircraft rows, no state kept.
use super::{
    detection::Sighting,
    profile::RadarProfile,
    track::{Environment, Observable, Observer, Strobe, radar_ranges_ft, strobes},
};
use crate::attitude::{Basis, Vector};

/// One surface radar's receiver at `antenna`: its profile and the jammers it
/// hears this tick.
pub struct Receiver<'a> {
    profile: &'a RadarProfile,
    antenna: Vector,
    strobes: Vec<Strobe>,
}

impl<'a> Receiver<'a> {
    /// A receiver at `antenna`. `aircraft` is every aircraft that may carry
    /// a jammer (only those whose jammer is on are heard, as an aircraft radar
    /// hears them: terrain-masked and received-power gated by
    /// [`super::track::strobes`]).
    pub fn new(
        profile: &'a RadarProfile,
        antenna: Vector,
        aircraft: &[Observable],
        environment: &Environment<'_>,
    ) -> Self {
        let strobes = if aircraft
            .iter()
            .any(|a| a.jammer_active && a.jammer.is_some())
        {
            let observer = Observer {
                position: antenna,
                basis: level(),
                radar_powered: true,
                radar_failed: false,
                infrared_failed: false,
                visual_failed: false,
            };
            strobes(&observer, aircraft, profile, environment)
        } else {
            Vec::new()
        };
        Self {
            profile,
            antenna,
            strobes,
        }
    }

    /// The distance out to which this radar detects `target` right now, feet:
    /// the record's search range (zone 0) scaled by the target's signature,
    /// clutter and jamming. Zero when the target returns no echo or is beyond
    /// the nominal range.
    pub fn range_ft(&self, target: &Observable, ground: &dyn Fn(f64, f64) -> f64) -> f64 {
        self.range_within(target, 1., ground)
    }

    /// [`Self::range_ft`] for a target no farther than `reach` times the
    /// nominal range (the cheap rejection before the geometry).
    fn range_within(
        &self,
        target: &Observable,
        reach: f64,
        ground: &dyn Fn(f64, f64) -> f64,
    ) -> f64 {
        let distance = super::detection::sight_distance_ft(self.antenna, target.position);
        if !distance.is_finite() || distance > self.profile.search.maximum_ft.max(0.) * reach {
            return 0.;
        }
        let sighting = Sighting::new(self.antenna, &level(), target.position);
        radar_ranges_ft(
            self.profile,
            self.antenna,
            target,
            &sighting,
            ground,
            &self.strobes,
        )
        .map_or(0., |(search, _)| search)
    }

    /// Whether the radar returns an echo from `target` at its distance now,
    /// the nominal range multiplied by `reach` (1 for a radar; a flak team's
    /// shared picture reads at twice its guns', docs/spec/surface-defenses.md,
    /// "Flak teams"). Terrain line of sight is the caller's.
    pub fn detects(
        &self,
        target: &Observable,
        reach: f64,
        ground: &dyn Fn(f64, f64) -> f64,
    ) -> bool {
        let distance = super::detection::sight_distance_ft(self.antenna, target.position);
        let range = self.range_within(target, reach, ground) * reach;
        range > 0. && distance <= range
    }
}

/// The antenna looks from a level frame: its angles never gate (the volume
/// is all-round), and only the world-frame clutter geometry is read.
fn level() -> Basis {
    Basis::new(0., 0., 0.)
}

#[cfg(test)]
mod tests {
    use super::super::{
        detection::burn_through_nmi,
        profile::{FEET_PER_NAUTICAL_MILE, Generation, JammerProfile, Preset, Volume},
        signature::{Configuration, SignatureProfile, Stores},
    };
    use super::*;

    /// A radar of the SA-6 class: 80,000 ft against a signature-100 target.
    const NOMINAL_FT: f64 = 80_000.;

    fn profile(look_down: f64) -> RadarProfile {
        let volume = Volume {
            azimuth_rad: std::f64::consts::PI,
            elevation_rad: std::f64::consts::PI,
            minimum_ft: 0.,
            maximum_ft: NOMINAL_FT,
            minimum_relative_ft: f64::NEG_INFINITY,
            maximum_relative_ft: f64::INFINITY,
        };
        RadarProfile {
            record: "TEST.JT".into(),
            search: volume,
            track: volume,
            look_down,
            preset: Preset::Basic,
            notch: Preset::Basic.notch(),
            resistance: Preset::Basic.resistance(),
            band: 0,
            source_flags: [0; 2],
            source_doppler: [0; 3],
        }
    }

    fn jammer(strength: f64) -> JammerProfile {
        JammerProfile {
            record: "TEST.ECM".into(),
            generation: Generation::Transitional,
            strength,
            band: 0,
            radio_frequency: true,
        }
    }

    /// An aircraft at `position`, level, heading +z, with PT radar signature
    /// `radar`.
    fn aircraft(radar: f64, position: Vector) -> Observable {
        Observable {
            id: 7,
            position,
            velocity: [0.; 3],
            basis: Basis::new(0., 0., 0.),
            configuration: Configuration::CLEAN,
            signature: SignatureProfile {
                radar,
                ..SignatureProfile::default()
            },
            jammer: None,
            jammer_active: false,
            radar_emitting: false,
            airborne: true,
            destroyed: false,
        }
    }

    fn flat(_: f64, _: f64) -> f64 {
        0.
    }

    fn range(target: &Observable) -> f64 {
        let p = profile(0.);
        let env = Environment {
            ground: &flat,
            obscured: &|_, _| false,
        };
        Receiver::new(&p, [0., 10., 0.], std::slice::from_ref(target), &env).range_ft(target, &flat)
    }

    /// Nose-on from a radar at the origin: the aircraft's nose points at it.
    const AHEAD: Vector = [0., 10., 40_000.];

    fn nose_on(radar: f64) -> Observable {
        let mut a = aircraft(radar, AHEAD);
        // Facing the radar: forward is -z.
        a.basis = Basis::new(std::f64::consts::PI, 0., 0.);
        a
    }

    #[test]
    fn a_signature_one_aircraft_is_seen_at_a_tenth_of_the_range() {
        // The record's band is the range against signature 100.
        let plain = range(&nose_on(100.));
        let stealth = range(&nose_on(1.));
        assert!((plain - NOMINAL_FT).abs() < 1e-6);
        assert!((stealth - NOMINAL_FT * 0.1).abs() < 1.0);
        // A signature-10 F-22 class aircraft: sqrt(0.1) of it.
        assert!((range(&nose_on(10.)) - NOMINAL_FT * 0.1_f64.sqrt()).abs() < 1.0);
        // Zero signature (an aircraft on the ground) returns nothing.
        assert_eq!(range(&nose_on(0.)), 0.);
    }

    #[test]
    fn aspect_raises_the_range_from_nose_to_side_to_top() {
        let nose = range(&nose_on(1.));
        let mut side = nose_on(1.);
        side.basis = Basis::new(std::f64::consts::FRAC_PI_2, 0., 0.);
        let mut top = nose_on(1.);
        // A radar nearly overhead sees the top.
        top.position = [0., 40_000., 100.];
        top.basis = Basis::new(0., 0., 0.);
        let side = range(&side);
        let top = range(&top);
        assert!(side > nose * 1.4 && (side - nose * 2f64.sqrt()).abs() < 1.0);
        assert!(top > side && (top - nose * 2.).abs() < 100.);
    }

    #[test]
    fn open_bay_gear_flaps_and_stores_raise_it() {
        let clean = range(&nose_on(1.));
        let with = |c: Configuration| {
            let mut a = nose_on(1.);
            a.configuration = c;
            range(&a)
        };
        let bay = with(Configuration {
            bay: 1.,
            ..Configuration::CLEAN
        });
        let gear = with(Configuration {
            gear: 1.,
            ..Configuration::CLEAN
        });
        let flaps = with(Configuration {
            flaps: 1.,
            ..Configuration::CLEAN
        });
        // The configuration multiplies the signature: 1.5, 1.25 and 1.15.
        assert!((bay - clean * 1.5_f64.sqrt()).abs() < 1.0);
        assert!((gear - clean * 1.25_f64.sqrt()).abs() < 1.0);
        assert!((flaps - clean * 1.15_f64.sqrt()).abs() < 1.0);
        // Stores add to the PT value before the factors: 3 per store and 2
        // per thousand pounds, hugely more on a signature-1 airframe.
        let stores = with(Configuration::carrying(Stores {
            count: 2.,
            lbs: 2_000.,
        }));
        assert!((stores - NOMINAL_FT * 0.11_f64.sqrt()).abs() < 1.0);
        assert!(stores > clean * 3.);
    }

    #[test]
    fn a_jammer_shortens_detection_until_burn_through() {
        let mut target = nose_on(100.);
        target.jammer = Some(jammer(0.30));
        let p = profile(0.);
        let env = Environment {
            ground: &flat,
            obscured: &|_, _| false,
        };
        let at = |z: f64, on: bool| {
            let mut t = target.clone();
            t.position = [0., 10., z];
            t.jammer_active = on;
            let r = Receiver::new(&p, [0., 10., 0.], std::slice::from_ref(&t), &env);
            (r.range_ft(&t, &flat), t)
        };
        // Quiet: the full nominal range.
        assert!((at(40_000., false).0 - NOMINAL_FT).abs() < 1e-6);
        // Jammed, well outside burn-through: 40 percent of it.
        let (far, _) = at(70_000., true);
        assert!((far - NOMINAL_FT * 0.4).abs() < 1.0);
        // Inside burn-through the jammer has no effect.
        let burn = burn_through_nmi(&jammer(0.30), &p, 1.).unwrap() * FEET_PER_NAUTICAL_MILE;
        let (near, t) = at(burn * 0.9, true);
        assert!((near - NOMINAL_FT).abs() < 1e-6, "burn {burn} range {near}");
        let r = Receiver::new(&p, [0., 10., 0.], std::slice::from_ref(&t), &env);
        assert!(r.detects(&t, 1., &flat));
        // Past the jammed range the radar does not detect.
        let (_, t) = at(NOMINAL_FT * 0.75, true);
        let r = Receiver::new(&p, [0., 10., 0.], std::slice::from_ref(&t), &env);
        assert!(!r.detects(&t, 1., &flat));
    }

    #[test]
    fn a_radar_on_a_ridge_loses_a_low_flier_by_its_look_down_coefficient() {
        // Looking down from 5,000 ft at a target 100 ft over the ground, the
        // record's look-down 50 costs it the same range it costs a fighter.
        let ground = |_: f64, _: f64| 0.;
        let mut low = nose_on(100.);
        low.position = [0., 100., 40_000.];
        let p = profile(50.);
        let env = Environment {
            ground: &ground,
            obscured: &|_, _| false,
        };
        let hill = Receiver::new(&p, [0., 5_000., 0.], std::slice::from_ref(&low), &env);
        let flat_site = Receiver::new(&p, [0., 10., 0.], std::slice::from_ref(&low), &env);
        assert!(hill.range_ft(&low, &ground) < NOMINAL_FT * 0.6);
        // A radar below its target never meets the clutter term.
        assert!((flat_site.range_ft(&low, &ground) - NOMINAL_FT).abs() < 1e-6);
    }
}
