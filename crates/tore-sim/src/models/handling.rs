//! Continuous host response to FA's published control values. See additional-aircraft spec.
use tore_formats::flight_model::normal_control::LoadedAxis;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Profile {
    pub roll: LoadedAxis,
    pub auxiliary: [LoadedAxis; 3],
}
impl Profile {
    pub fn from_aircraft(a: &tore_formats::aircraft::Aircraft) -> tore_formats::Result<Self> {
        let axis = |prefix: &str| -> tore_formats::Result<LoadedAxis> {
            let field = |suffix: &str| -> tore_formats::Result<i16> {
                let key = format!("{prefix}.{suffix}");
                let value = a
                    .fields
                    .get(&key)
                    .ok_or_else(|| std::io::Error::other(format!("missing control field {key}")))?
                    .number()?;
                i16::try_from(value).map_err(|_| std::io::Error::other("control field overflow"))
            };
            let p = LoadedAxis {
                minimum: field("min")?,
                maximum: field("max")?,
                acceleration: field("acc")?,
                deceleration: field("dacc")?,
            };
            if p.minimum > 0 || p.maximum < 0 || p.acceleration <= 0 || p.deceleration <= 0 {
                return Err(std::io::Error::other("invalid aircraft control response"));
            }
            Ok(p)
        };
        Ok(Self {
            roll: axis("_brv.x")?,
            auxiliary: [axis("puffRot.x")?, axis("puffRot.y")?, axis("puffRot.z")?],
        })
    }
}

pub fn approach(current: f64, command: f64, axis: LoadedAxis, authority: f64, dt: f64) -> f64 {
    let command = command.clamp(-1., 1.);
    let bound = if command < 0. {
        -f64::from(axis.minimum)
    } else {
        f64::from(axis.maximum)
    };
    let target = command * bound.to_radians() * authority;
    let rate = if command == 0. {
        f64::from(axis.deceleration)
    } else {
        let reversal = if current * target < 0. {
            f64::from(axis.deceleration) / 2.
        } else {
            0.
        };
        (f64::from(axis.acceleration) + reversal) * command.abs().max(0.25)
    }
    .to_radians();
    current + (target - current).clamp(-rate * dt, rate * dt)
}

pub fn auxiliary_authority(speed: f64, throttle: f64, powered: bool, ground: bool) -> f64 {
    if !powered || ground {
        0.
    } else {
        (throttle * 2.).clamp(0., 1.) * (1. - speed / 220.).clamp(0., 1.)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_rates_and_release() {
        for (max, acceleration, deceleration) in [(225, 286, 571), (180, 214, 427), (345, 498, 996)]
        {
            let axis = LoadedAxis {
                minimum: -max,
                maximum: max,
                acceleration,
                deceleration,
            };
            let mut rate = 0.;
            for _ in 0..60 {
                rate = approach(rate, 1., axis, 1., 1. / 120.);
            }
            assert!((rate.to_degrees() - f64::from(acceleration) / 2.).abs() < 1e-9);
            for _ in 0..120 {
                rate = approach(rate, 1., axis, 1., 1. / 120.);
            }
            assert!((rate.to_degrees() - f64::from(max)).abs() < 1e-9);
            for _ in 0..120 {
                rate = approach(rate, 0., axis, 1., 1. / 120.);
            }
            assert_eq!(rate, 0.);
        }
    }
    #[test]
    fn low_speed_power_and_ground_gates() {
        assert_eq!(auxiliary_authority(110., 0.5, true, false), 0.5);
        assert_eq!(auxiliary_authority(110., 0.25, true, false), 0.25);
        assert_eq!(auxiliary_authority(220., 1., true, false), 0.);
        assert_eq!(auxiliary_authority(0., 1., false, false), 0.);
        assert_eq!(auxiliary_authority(0., 1., true, true), 0.);
        assert_eq!(auxiliary_authority(0., 1., true, false), 1.);
    }
}
