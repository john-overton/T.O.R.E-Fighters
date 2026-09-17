//! The authored detection model: geometry, look-down clutter, Doppler notching
//! and received RF interference. Every constant here is agent tuning recorded
//! in docs/radar.md. No physics fidelity claim is attached to it, and the
//! recovered equipment data stays in the profiles it reads.
use super::profile::{FEET_PER_NAUTICAL_MILE, JammerProfile, Notch, RadarProfile, Resistance};
use crate::attitude::{Basis, Vector, dot, unit};

/// Terrain-relative scale of the recovered look-down height term.
pub const CLUTTER_HEIGHT_FT: f64 = 5000.;
/// Downward sight angle at which the recovered angle term saturates.
pub const CLUTTER_ANGLE_DEG: f64 = 45.;
/// Jammer reference distance in the normalized interference term.
pub const JAMMER_REFERENCE_NMI: f64 = 20.;
/// Numerical floor for a jammer sharing the receiver's position.
pub const JAMMER_MINIMUM_NMI: f64 = 0.1;
/// Source deception chance treated as the reference jammer strength.
pub const JAMMER_REFERENCE_STRENGTH: f64 = 0.30;
/// Largest range penalty the interference term can apply.
pub const JAMMER_MAXIMUM_LOSS: f64 = 0.60;

/// One observer-to-target relation in shared simulation units.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sighting {
    /// Body-relative, right positive.
    pub azimuth_rad: f64,
    /// Body-relative, up positive.
    pub elevation_rad: f64,
    pub distance_ft: f64,
    /// Target altitude minus observer altitude.
    pub relative_altitude_ft: f64,
    /// World downward sight angle in degrees, zero when level or looking up.
    pub depression_deg: f64,
    /// World unit vector from the observer to the target.
    pub line_of_sight: Vector,
}
impl Sighting {
    pub fn new(observer: Vector, basis: &Basis, target: Vector) -> Self {
        let delta: Vector = std::array::from_fn(|i| target[i] - observer[i]);
        let distance_ft = dot(delta, delta).sqrt();
        let right = dot(delta, basis.right);
        let up = dot(delta, basis.up);
        let forward = dot(delta, basis.forward);
        let horizontal = delta[0].hypot(delta[2]);
        Self {
            azimuth_rad: right.atan2(forward),
            elevation_rad: up.atan2(right.hypot(forward)),
            distance_ft,
            relative_altitude_ft: delta[1],
            depression_deg: if delta[1] < 0. {
                (-delta[1]).atan2(horizontal).to_degrees()
            } else {
                0.
            },
            line_of_sight: unit(delta),
        }
    }
    pub fn distance_nmi(&self) -> f64 {
        self.distance_ft / FEET_PER_NAUTICAL_MILE
    }
    /// Bearing relative to the observer's heading: 0 ahead, 90 degrees right.
    /// It ignores bank, so a passive instrument stays readable in a turn.
    pub fn heading_relative_bearing(&self, heading_rad: f64) -> f64 {
        (self.line_of_sight[0].atan2(self.line_of_sight[2]) - heading_rad)
            .rem_euclid(std::f64::consts::TAU)
    }
}

/// Clutter exposure C. Zero for a target at or above the observer, otherwise
/// the larger of the downward-angle and terrain-proximity terms.
pub fn clutter_exposure(sighting: &Sighting, target_height_agl_ft: f64) -> f64 {
    if sighting.relative_altitude_ft >= 0. {
        return 0.;
    }
    let angle = sighting.depression_deg / CLUTTER_ANGLE_DEG;
    let height = if target_height_agl_ft.is_finite() {
        1. - target_height_agl_ft.max(0.) / CLUTTER_HEIGHT_FT
    } else {
        0.
    };
    angle.max(height).clamp(0., 1.)
}

/// Look-down range factor L. The coefficient is the installed source value.
pub fn look_down_factor(coefficient: f64, clutter: f64) -> f64 {
    1. - (coefficient.clamp(0., 100.) / 100.) * clutter.clamp(0., 1.)
}

/// Notch speed: the absolute projection of the target's ground-relative
/// velocity onto the radar line of sight. Ownship-relative closing speed would
/// wrongly notch a matching-speed tail chase.
pub fn notch_speed_fps(sighting: &Sighting, target_velocity: Vector) -> f64 {
    dot(target_velocity, sighting.line_of_sight).abs()
}

/// Notch range factor N. The penalty needs clutter exposure and blends out at
/// the preset's half width.
pub fn notch_factor(notch: &Notch, clutter: f64, notch_speed_fps: f64) -> f64 {
    if !notch.enabled || notch.half_width_fps <= 0. {
        return 1.;
    }
    let depth = (1. - notch_speed_fps.max(0.) / notch.half_width_fps).clamp(0., 1.);
    1. - clutter.clamp(0., 1.) * (1. - notch.centre_factor) * depth
}

/// One received emitter at the radar, before any per-contact coupling.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reception {
    pub bearing_rad: f64,
    pub elevation_rad: f64,
    pub line_of_sight: Vector,
    /// Normalized received interference I.
    pub received: f64,
}

/// Normalized received interference from one powered, compatible emitter.
/// This is noise arriving at the receiver, not a detected target, and it is
/// never clipped by the selected display range.
pub fn received(jammer: &JammerProfile, radar: &RadarProfile, distance_nmi: f64) -> f64 {
    let compatible = jammer.compatible(radar);
    if compatible <= 0. || !distance_nmi.is_finite() {
        return 0.;
    }
    let strength = jammer.strength / JAMMER_REFERENCE_STRENGTH;
    let distance = distance_nmi.max(JAMMER_MINIMUM_NMI);
    strength
        * jammer.generation.matchup(radar.preset)
        * compatible
        * (JAMMER_REFERENCE_NMI / distance).powi(2)
}

/// Angular coupling A between a candidate contact and an emitter. Nearby
/// angular contacts share the noisy sector; other sectors receive the weaker
/// sidelobe contribution.
pub fn angular_coupling(resistance: &Resistance, separation_rad: f64) -> f64 {
    let floor = resistance.sidelobe_floor.clamp(0., 1.);
    if resistance.coupling_rad <= 0. {
        return floor;
    }
    floor + (1. - floor) * (1. - separation_rad.abs() / resistance.coupling_rad).clamp(0., 1.)
}

/// Interference-to-return proxy Q for one candidate contact.
pub fn interference_quotient(
    resistance: &Resistance,
    coupled: f64,
    target_nmi: f64,
    relative_signature: f64,
) -> f64 {
    if coupled <= 0. || relative_signature <= 0. || !target_nmi.is_finite() {
        return 0.;
    }
    let reference = JAMMER_REFERENCE_NMI * resistance.burn_through_nmi;
    coupled * target_nmi.max(0.).powi(4) / (reference * reference * relative_signature)
}

/// Jammer range factor J. No penalty at or inside burn-through, reaching the
/// documented maximum loss at twice that distance.
pub fn jammer_factor(quotient: f64) -> f64 {
    if !quotient.is_finite() || quotient <= 0. {
        return 1.;
    }
    1. - JAMMER_MAXIMUM_LOSS * (quotient.sqrt() - 1.).clamp(0., 1.)
}

/// Effective burn-through distance for a single self-protection emitter at the
/// target's own position with full angular coupling.
pub fn burn_through_nmi(
    jammer: &JammerProfile,
    radar: &RadarProfile,
    relative_signature: f64,
) -> Option<f64> {
    let compatible = jammer.compatible(radar);
    let input = (jammer.strength / JAMMER_REFERENCE_STRENGTH)
        * jammer.generation.matchup(radar.preset)
        * compatible;
    (input > 0. && relative_signature > 0.)
        .then(|| radar.resistance.burn_through_nmi * (relative_signature / input).sqrt())
}

/// The shared detection rule. The nominal volume distance is a hard maximum;
/// a large signature improves interference margin instead of exceeding it.
pub fn effective_range_ft(
    nominal_ft: f64,
    relative_signature: f64,
    look_down: f64,
    notch: f64,
    jammer: f64,
) -> f64 {
    if !nominal_ft.is_finite()
        || nominal_ft <= 0.
        || !relative_signature.is_finite()
        || relative_signature <= 0.
    {
        return 0.;
    }
    let factors = look_down.clamp(0., 1.) * notch.clamp(0., 1.) * jammer.clamp(0., 1.);
    (nominal_ft * relative_signature.sqrt() * factors).min(nominal_ft)
}

/// Infrared effective range. The radar aspect, notch and RF jammer terms never
/// apply to this passive channel.
pub fn infrared_range_ft(nominal_ft: f64, signature: f64) -> f64 {
    if !nominal_ft.is_finite() || nominal_ft <= 0. || !signature.is_finite() || signature <= 0. {
        return 0.;
    }
    (nominal_ft * (signature / 100.).sqrt()).min(nominal_ft)
}

#[cfg(test)]
mod tests {
    use super::super::profile::{Generation, Preset};
    use super::*;
    fn radar(preset: Preset) -> RadarProfile {
        RadarProfile {
            record: "TEST.SEE".into(),
            search: super::super::profile::Volume {
                azimuth_rad: 1.,
                elevation_rad: 1.,
                minimum_ft: 0.,
                maximum_ft: 90. * FEET_PER_NAUTICAL_MILE,
                minimum_relative_ft: f64::NEG_INFINITY,
                maximum_relative_ft: f64::INFINITY,
            },
            track: super::super::profile::Volume {
                azimuth_rad: 0.8,
                elevation_rad: 0.8,
                minimum_ft: 0.,
                maximum_ft: 50. * FEET_PER_NAUTICAL_MILE,
                minimum_relative_ft: f64::NEG_INFINITY,
                maximum_relative_ft: f64::INFINITY,
            },
            look_down: 50.,
            preset,
            notch: preset.notch(),
            resistance: preset.resistance(),
            band: 0,
            source_flags: [0; 2],
            source_doppler: [0; 3],
        }
    }
    fn jammer(generation: Generation, strength: f64) -> JammerProfile {
        JammerProfile {
            record: "TEST.ECM".into(),
            generation,
            strength,
            band: 0,
            radio_frequency: true,
        }
    }
    #[test]
    fn signature_law_matches_the_documented_ninety_mile_examples() {
        let nominal = 90. * FEET_PER_NAUTICAL_MILE;
        let at = |signature: f64| {
            effective_range_ft(nominal, signature / 100., 1., 1., 1.) / FEET_PER_NAUTICAL_MILE
        };
        assert!((at(100.) - 90.).abs() < 1e-9);
        assert!((at(50.) - 63.639_610_306_789_28).abs() < 1e-6);
        assert!((at(10.) - 28.460_498_941_515_41).abs() < 1e-6);
        assert!((at(80.) - 80.498_447_189_992_1).abs() < 1e-6);
        assert!((at(120.) - 90.).abs() < 1e-9);
        assert_eq!(at(0.), 0.);
        assert_eq!(effective_range_ft(nominal, -1., 1., 1., 1.), 0.);
        assert_eq!(effective_range_ft(nominal, f64::NAN, 1., 1., 1.), 0.);
    }
    #[test]
    fn look_down_uses_the_larger_penalty_and_ignores_level_or_higher_targets() {
        let level = Sighting::new([0.; 3], &Basis::new(0., 0., 0.), [0., 0., 10_000.]);
        assert_eq!(clutter_exposure(&level, 0.), 0.);
        let below = Sighting::new(
            [0., 20_000., 0.],
            &Basis::new(0., 0., 0.),
            [0., 10_000., 10_000.],
        );
        assert!((below.depression_deg - 45.).abs() < 1e-9);
        assert!((clutter_exposure(&below, 10_000.) - 1.).abs() < 1e-9);
        let shallow = Sighting::new(
            [0., 20_000., 0.],
            &Basis::new(0., 0., 0.),
            [0., 19_000., 10_000.],
        );
        let c = clutter_exposure(&shallow, 10_000.);
        assert!((c - 5.710_593_137_499_642 / 45.).abs() < 1e-9);
        assert!((look_down_factor(50., 0.5) - 0.75).abs() < 1e-9);
        assert!((look_down_factor(30., 0.5) - 0.85).abs() < 1e-9);
        assert!((look_down_factor(0., 0.5) - 1.).abs() < 1e-9);
        assert!((clutter_exposure(&shallow, 2_500.) - 0.5).abs() < 1e-9);
        assert!((clutter_exposure(&shallow, 0.) - 1.).abs() < 1e-9);
    }
    #[test]
    fn notch_needs_clutter_and_blends_out_at_the_preset_width() {
        let advanced = Preset::Advanced.notch();
        let transitional = Preset::Transitional.notch();
        assert!((notch_factor(&advanced, 1., 0.) - 0.45).abs() < 1e-9);
        assert!((notch_factor(&transitional, 1., 0.) - 0.20).abs() < 1e-9);
        assert!((notch_factor(&advanced, 1., 60.) - 1.).abs() < 1e-9);
        assert!((notch_factor(&transitional, 1., 100.) - 1.).abs() < 1e-9);
        assert!((notch_factor(&advanced, 0., 0.) - 1.).abs() < 1e-9);
        assert!((notch_factor(&Preset::Basic.notch(), 1., 0.) - 1.).abs() < 1e-9);
        assert!((notch_factor(&advanced, 0.5, 30.) - (1. - 0.5 * 0.55 * 0.5)).abs() < 1e-9);
    }
    #[test]
    fn a_matching_speed_tail_chase_does_not_notch() {
        let basis = Basis::new(0., 0., 0.);
        let sighting = Sighting::new([0., 10_000., 0.], &basis, [0., 10_000., 30_000.]);
        assert!((notch_speed_fps(&sighting, [0., 0., 800.]) - 800.).abs() < 1e-9);
        assert!((notch_factor(&Preset::Advanced.notch(), 1., 800.) - 1.).abs() < 1e-9);
        let crossing = notch_speed_fps(&sighting, [800., 0., 0.]);
        assert!(crossing.abs() < 1e-9);
        assert!((notch_factor(&Preset::Advanced.notch(), 1., crossing) - 0.45).abs() < 1e-9);
    }
    #[test]
    fn interference_falls_with_the_square_of_emitter_distance() {
        let r = radar(Preset::Advanced);
        let j = jammer(Generation::LateColdWar, 0.30);
        let near = received(&j, &r, 20.);
        let far = received(&j, &r, 40.);
        assert!((near - 1.).abs() < 1e-9);
        assert!((far - 0.25).abs() < 1e-9);
        assert!(
            (received(&j, &r, 0.) - (JAMMER_REFERENCE_NMI / JAMMER_MINIMUM_NMI).powi(2)).abs()
                < 1e-6
        );
        let incompatible = JammerProfile {
            radio_frequency: false,
            ..j.clone()
        };
        assert_eq!(received(&incompatible, &r, 20.), 0.);
    }
    #[test]
    fn equal_generation_burns_through_at_the_reference_distance() {
        for preset in [Preset::Basic, Preset::Transitional, Preset::Advanced] {
            let r = radar(preset);
            let generation = match preset {
                Preset::Basic => Generation::Early,
                Preset::Transitional => Generation::Transitional,
                Preset::Advanced => Generation::LateColdWar,
            };
            let j = jammer(generation, 0.30);
            let b = r.resistance.burn_through_nmi;
            assert!((burn_through_nmi(&j, &r, 1.).unwrap() - b).abs() < 1e-9);
            let at = |distance: f64| {
                let coupled = received(&j, &r, distance) * 1.;
                jammer_factor(interference_quotient(&r.resistance, coupled, distance, 1.))
            };
            assert!((at(b) - 1.).abs() < 1e-9);
            assert!((at(b * 2.) - 0.40).abs() < 1e-9);
            assert!((at(b * 0.5) - 1.).abs() < 1e-9);
            assert!((at(b * 4.) - 0.40).abs() < 1e-9);
        }
    }
    #[test]
    fn documented_cross_generation_burn_through_distances() {
        let advanced = radar(Preset::Advanced);
        let early = jammer(Generation::Early, 0.30);
        assert!(
            (burn_through_nmi(&early, &advanced, 1.).unwrap() - 18.257_418_583_505_54).abs() < 1e-6
        );
        let basic = radar(Preset::Basic);
        let late = jammer(Generation::LateColdWar, 0.30);
        assert!(
            (burn_through_nmi(&late, &basic, 1.).unwrap() - 3.952_847_075_210_474).abs() < 1e-6
        );
        // A lower signature brings burn-through closer for the same jammer.
        let low = burn_through_nmi(&early, &advanced, 0.10).unwrap();
        assert!(low < 18.257_418_583_505_54 && (low - 5.773_502_691_896_258).abs() < 1e-6);
    }
    #[test]
    fn coupling_falls_to_the_sidelobe_floor_outside_the_receiver_width() {
        let r = radar(Preset::Advanced);
        assert!((angular_coupling(&r.resistance, 0.) - 1.).abs() < 1e-9);
        assert!((angular_coupling(&r.resistance, 1f64.to_radians()) - 0.005).abs() < 1e-9);
        assert!((angular_coupling(&r.resistance, 1.) - 0.005).abs() < 1e-9);
        let half = angular_coupling(&r.resistance, 0.5f64.to_radians());
        assert!((half - (0.005 + 0.995 * 0.5)).abs() < 1e-9);
        assert_eq!(jammer_factor(0.), 1.);
        assert_eq!(jammer_factor(1.), 1.);
        assert!((jammer_factor(4.) - 0.40).abs() < 1e-9);
        assert!((jammer_factor(100.) - 0.40).abs() < 1e-9);
    }
    #[test]
    fn infrared_range_ignores_radar_terms() {
        let nominal = 15. * FEET_PER_NAUTICAL_MILE;
        assert!((infrared_range_ft(nominal, 100.) - nominal).abs() < 1e-9);
        assert!((infrared_range_ft(nominal, 60.) - nominal * 0.6f64.sqrt()).abs() < 1e-6);
        assert!((infrared_range_ft(nominal, 400.) - nominal).abs() < 1e-9);
        assert_eq!(infrared_range_ft(nominal, 0.), 0.);
    }
}
