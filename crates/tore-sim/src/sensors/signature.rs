//! One observer-relative aircraft signature function, shared by radar
//! detection, the jammer burn-through comparison and the RCS exposure contour.
//! The 1/2/4 aspect weights, configuration multipliers and reference contour
//! radar are agent tuning from docs/radar.md, not recovered RCS measurements.
use super::invalid;
use crate::attitude::{Basis, Vector, dot, unit};
use tore_formats::{Result, aircraft::Aircraft};

/// Actual deployed fractions, never command switches. A component an aircraft
/// does not have contributes zero deployment.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Configuration {
    pub gear: f64,
    pub flaps: f64,
    pub bay: f64,
}
impl Configuration {
    pub const CLEAN: Self = Self {
        gear: 0.,
        flaps: 0.,
        bay: 0.,
    };
    fn sanitized(self) -> Self {
        let clamp = |v: f64| if v.is_finite() { v.clamp(0., 1.) } else { 0. };
        Self {
            gear: clamp(self.gear),
            flaps: clamp(self.flaps),
            bay: clamp(self.bay),
        }
    }
}

/// Body-axis exposure weights: nose/tail, side and top/bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aspect {
    pub forward: f64,
    pub side: f64,
    pub vertical: f64,
}
impl Default for Aspect {
    fn default() -> Self {
        Self {
            forward: 1.,
            side: 2.,
            vertical: 4.,
        }
    }
}

/// Per-component configuration weights, kept as profile fields so an aircraft
/// never needs its own hardcoded rule.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Deployment {
    pub gear: f64,
    pub flaps: f64,
    pub bay: f64,
}
impl Default for Deployment {
    fn default() -> Self {
        Self {
            gear: 0.25,
            flaps: 0.15,
            bay: 0.50,
        }
    }
}

/// The reference radar the passive exposure contour is drawn against. It is an
/// estimate of directional vulnerability, not an assertion about any emitter.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reference {
    pub nominal_nmi: f64,
    pub signature: f64,
}
impl Default for Reference {
    fn default() -> Self {
        Self {
            nominal_nmi: 25.,
            signature: 400.,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SignatureProfile {
    /// PT sigs[3], a relative game statistic and not square metres.
    pub radar: f64,
    /// PT sigs[2], a relative game statistic and not measured emission.
    pub infrared: f64,
    pub aspect: Aspect,
    pub deployment: Deployment,
    pub reference: Reference,
}
impl Default for SignatureProfile {
    fn default() -> Self {
        Self {
            radar: 100.,
            infrared: 100.,
            aspect: Aspect::default(),
            deployment: Deployment::default(),
            reference: Reference::default(),
        }
    }
}
impl SignatureProfile {
    pub fn from_source(a: &Aircraft) -> Result<Self> {
        let value = |key: &str| -> Result<f64> {
            let raw = a
                .object
                .get(key)
                .ok_or_else(|| invalid("aircraft is missing a signature field"))?
                .number()?;
            if raw < 0 {
                return Err(invalid("negative aircraft signature"));
            }
            Ok(f64::from(raw))
        };
        Ok(Self {
            radar: value("sigs[3]")?,
            infrared: value("sigs[2]")?,
            ..Self::default()
        })
    }
    pub fn configuration_factor(&self, configuration: Configuration) -> f64 {
        let c = configuration.sanitized();
        (1. + self.deployment.gear * c.gear)
            * (1. + self.deployment.flaps * c.flaps)
            * (1. + self.deployment.bay * c.bay)
    }
    /// Effective radar signature seen from `to_observer`, a direction from this
    /// aircraft toward the observing radar in world axes. Exposure follows what
    /// that observer can see, so banking abeam raises it while banking nose-on
    /// does not.
    pub fn effective_radar(
        &self,
        body: &Basis,
        to_observer: Vector,
        configuration: Configuration,
    ) -> f64 {
        if !self.radar.is_finite() || self.radar <= 0. {
            return 0.;
        }
        let d = unit(to_observer);
        let f = dot(d, body.forward);
        let r = dot(d, body.right);
        let u = dot(d, body.up);
        let shape =
            self.aspect.forward * f * f + self.aspect.side * r * r + self.aspect.vertical * u * u;
        self.radar * shape * self.configuration_factor(configuration)
    }
    /// Passive exposure distance for the documented reference radar, using the
    /// same square-root range law as active detection.
    pub fn exposure_nmi(&self, effective: f64) -> f64 {
        if !effective.is_finite() || effective <= 0. {
            return 0.;
        }
        (self.reference.nominal_nmi * (effective / self.reference.signature).sqrt())
            .min(self.reference.nominal_nmi)
    }
    /// Heading-relative exposure contour at `step_deg` bearing steps: 0 ahead,
    /// 90 right. The reference radar is level with ownship at every sample.
    pub fn exposure_contour(
        &self,
        body: &Basis,
        configuration: Configuration,
        step_deg: f64,
    ) -> Vec<(f64, f64)> {
        let step = if step_deg.is_finite() && step_deg > 0.1 {
            step_deg
        } else {
            5.
        };
        let heading = body.angles()[0];
        let mut contour = Vec::new();
        let mut bearing_deg: f64 = 0.;
        while bearing_deg < 360. {
            let bearing = bearing_deg.to_radians();
            let direction = [(heading + bearing).sin(), 0., (heading + bearing).cos()];
            let effective = self.effective_radar(body, direction, configuration);
            contour.push((bearing, self.exposure_nmi(effective)));
            bearing_deg += step;
        }
        contour
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn level() -> Basis {
        Basis::new(0., 0., 0.)
    }
    #[test]
    fn nose_side_and_top_aspects_give_the_documented_one_two_four_factors() {
        let s = SignatureProfile::default();
        let nose = s.effective_radar(&level(), [0., 0., 1.], Configuration::CLEAN);
        let tail = s.effective_radar(&level(), [0., 0., -1.], Configuration::CLEAN);
        let side = s.effective_radar(&level(), [1., 0., 0.], Configuration::CLEAN);
        let top = s.effective_radar(&level(), [0., 1., 0.], Configuration::CLEAN);
        assert!((nose - 100.).abs() < 1e-9 && (tail - 100.).abs() < 1e-9);
        assert!((side - 200.).abs() < 1e-9);
        assert!((top - 400.).abs() < 1e-9);
    }
    #[test]
    fn banking_changes_abeam_exposure_but_not_nose_on_exposure() {
        let s = SignatureProfile::default();
        let abeam = [1., 0., 0.];
        let nose = [0., 0., 1.];
        let knife = Basis::new(0., 0., std::f64::consts::FRAC_PI_2);
        assert!((s.effective_radar(&knife, abeam, Configuration::CLEAN) - 400.).abs() < 1e-6);
        assert!((s.effective_radar(&knife, nose, Configuration::CLEAN) - 100.).abs() < 1e-6);
        let inverted = Basis::new(0., 0., std::f64::consts::PI);
        assert!((s.effective_radar(&inverted, abeam, Configuration::CLEAN) - 200.).abs() < 1e-6);
        let banked = Basis::new(0., 0., std::f64::consts::FRAC_PI_4);
        let value = s.effective_radar(&banked, abeam, Configuration::CLEAN);
        assert!(value > 200. && value < 400.);
    }
    #[test]
    fn configuration_multiplies_smoothly_and_clears_back_to_baseline() {
        let s = SignatureProfile::default();
        let gear = Configuration {
            gear: 1.,
            ..Configuration::CLEAN
        };
        let nose = [0., 0., 1.];
        assert!((s.effective_radar(&level(), nose, gear) - 125.).abs() < 1e-9);
        let half = Configuration {
            gear: 0.5,
            ..Configuration::CLEAN
        };
        assert!((s.effective_radar(&level(), nose, half) - 112.5).abs() < 1e-9);
        let all = Configuration {
            gear: 1.,
            flaps: 1.,
            bay: 1.,
        };
        assert!((s.configuration_factor(all) - 1.25 * 1.15 * 1.5).abs() < 1e-9);
        assert!((s.effective_radar(&level(), nose, Configuration::CLEAN) - 100.).abs() < 1e-9);
        let invalid = Configuration {
            gear: f64::NAN,
            flaps: 5.,
            bay: -1.,
        };
        assert!((s.configuration_factor(invalid) - 1.15).abs() < 1e-9);
    }
    #[test]
    fn reference_contour_matches_the_documented_front_side_and_top_radii() {
        let s = SignatureProfile::default();
        assert!((s.exposure_nmi(100.) - 12.5).abs() < 1e-9);
        assert!((s.exposure_nmi(200.) - 17.677_669_529_663_69).abs() < 1e-9);
        assert!((s.exposure_nmi(400.) - 25.).abs() < 1e-9);
        assert!((s.exposure_nmi(10_000.) - 25.).abs() < 1e-9);
        assert_eq!(s.exposure_nmi(0.), 0.);
        let contour = s.exposure_contour(&level(), Configuration::CLEAN, 5.);
        assert_eq!(contour.len(), 72);
        assert!((contour[0].1 - 12.5).abs() < 1e-9);
        assert!((contour[18].1 - 17.677_669_529_663_69).abs() < 1e-9);
        assert!((contour[36].1 - 12.5).abs() < 1e-9);
    }
    #[test]
    fn contour_is_heading_relative_rather_than_world_referenced() {
        let s = SignatureProfile::default();
        let turned = Basis::new(1.2, 0., 0.);
        let contour = s.exposure_contour(&turned, Configuration::CLEAN, 5.);
        assert!((contour[0].1 - 12.5).abs() < 1e-9);
        assert!((contour[18].1 - 17.677_669_529_663_69).abs() < 1e-9);
    }
    #[test]
    fn zero_signature_has_no_ordinary_return() {
        let s = SignatureProfile {
            radar: 0.,
            ..SignatureProfile::default()
        };
        assert_eq!(
            s.effective_radar(&level(), [0., 0., 1.], Configuration::CLEAN),
            0.
        );
    }
}
