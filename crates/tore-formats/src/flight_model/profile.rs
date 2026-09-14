//! Import-time mapping only. Kernels consume Copy profiles, never strings/maps.
use super::{
    departure::DepartureProfile, forces::DragProfile, ground::LandingLimits,
    integration::AxisLimits,
};
use crate::{Result, aircraft::Token, invalid};
use std::collections::BTreeMap;
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlightProfile {
    pub departure: DepartureProfile,
    pub landing: LandingLimits,
    pub drag: DragProfile,
    /// PT _bv x/y/z map to forward/side/down; loaded forward maximum overrides x.max.
    pub velocity: [AxisLimits; 3],
    pub extended_warning: bool,
}
impl FlightProfile {
    /// FA COBv 0x477ea0 copies PT limits then replaces forward max with cp+0x245.
    /// FA 0x452482..0x4524d6 sets it from the current-altitude 1G envelope maximum.
    /// Using the PT placeholder (F18: zero) stops flight.
    pub fn loaded_velocity(&self, forward_maximum: i16) -> Result<[AxisLimits; 3]> {
        if forward_maximum <= 0 || forward_maximum < self.velocity[0].minimum {
            return Err(invalid("invalid loaded forward speed limit"));
        }
        let mut limits = self.velocity;
        limits[0].maximum = forward_maximum;
        Ok(limits)
    }
    /// Accepts named fields from a reviewed PT reader. Missing fields are errors.
    /// Does not authorize new PT layouts beyond Aircraft::parse's reviewed identities.
    pub fn from_fields(fields: &BTreeMap<String, Token>) -> Result<Self> {
        let number = |key: &str| -> Result<i32> {
            fields
                .get(key)
                .ok_or_else(|| invalid(&format!("missing flight field {key}")))?
                .number()
        };
        let word = |key: &str| -> Result<i16> {
            let t = fields
                .get(key)
                .ok_or_else(|| invalid(&format!("missing flight field {key}")))?;
            if t.kind != "word" {
                return Err(invalid("flight profile expected word"));
            }
            i16::try_from(t.number()?).map_err(|_| invalid("flight field outside word"))
        };
        let pair = |prefix: &str| -> Result<[i16; 2]> {
            Ok([
                word(&format!("{prefix}Low"))?,
                word(&format!("{prefix}High"))?,
            ])
        };
        let axis = |name: &str| -> Result<AxisLimits> {
            let l = AxisLimits {
                minimum: word(&format!("_bv.{name}.min"))?,
                maximum: word(&format!("_bv.{name}.max"))?,
                acceleration: word(&format!("_bv.{name}.acc"))?,
                deceleration: word(&format!("_bv.{name}.dacc"))?,
            };
            if l.minimum > l.maximum || l.acceleration < 0 || l.deceleration < 0 {
                return Err(invalid("invalid PT velocity limits"));
            }
            Ok(l)
        };
        Ok(Self {
            departure: DepartureProfile {
                warning_delay: word("stallWarningDelay")?,
                stall_delay: word("stallDelay")?,
                severity: word("stallSeverity")?,
                pitch_down: word("stallPitchDown")?,
                spin_entry: word("spinEntry")?,
                spin_exit: word("spinExit")?,
                spin_yaw: pair("spinYaw")?,
                spin_aoa: pair("spinAOA")?,
                spin_bank: pair("spinBank")?,
            },
            landing: LandingLimits {
                forward_fps: word("crashSpeedForward")?,
                side_fps: word("crashSpeedSide")?,
                descent_fps: word("crashSpeedVertical")?,
                pitch_degrees: word("crashPitch")?,
                roll_degrees: word("crashRoll")?,
            },
            drag: DragProfile {
                rudder: word("rudderDrag")?,
                flaps: word("flapsDrag")?,
                gear: word("gearDrag")?,
                airbrake: word("airBrakesDrag")?,
                bay: word("bayDrag")?,
                wheel: word("wheelBrakesDrag")?,
            },
            velocity: [axis("x")?, axis("y")?, axis("z")?],
            extended_warning: number("flags")? & 0x400 != 0,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn import_mapping_rejects_missing_and_wrong_width() {
        let mut fields = BTreeMap::new();
        assert!(FlightProfile::from_fields(&fields).is_err());
        for name in [
            "rudderDrag",
            "flapsDrag",
            "gearDrag",
            "airBrakesDrag",
            "bayDrag",
            "wheelBrakesDrag",
            "stallWarningDelay",
            "stallDelay",
            "stallSeverity",
            "stallPitchDown",
            "spinEntry",
            "spinExit",
            "spinYawLow",
            "spinYawHigh",
            "spinAOALow",
            "spinAOAHigh",
            "spinBankLow",
            "spinBankHigh",
            "crashSpeedForward",
            "crashSpeedSide",
            "crashSpeedVertical",
            "crashPitch",
            "crashRoll",
            "flags",
        ] {
            fields.insert(
                name.into(),
                Token {
                    kind: "word".into(),
                    value: "0".into(),
                    scaled: false,
                },
            );
        }
        for axis in ["x", "y", "z"] {
            for suffix in ["min", "max", "acc", "dacc"] {
                fields.insert(
                    format!("_bv.{axis}.{suffix}"),
                    Token {
                        kind: "word".into(),
                        value: "0".into(),
                        scaled: false,
                    },
                );
            }
        }
        fields.get_mut("spinExit").unwrap().value = "$fffe".into();
        let p = FlightProfile::from_fields(&fields).unwrap();
        assert_eq!(p.departure.spin_exit, -2);
        assert!(p.loaded_velocity(0).is_err());
        assert_eq!(p.loaded_velocity(1000).unwrap()[0].maximum, 1000);
        fields.get_mut("spinExit").unwrap().kind = "dword".into();
        assert!(FlightProfile::from_fields(&fields).is_err());
    }
}
