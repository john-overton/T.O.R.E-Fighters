//! Complete configuration consumed by the current adapter, resolved once from PT.
//! Native fields retain native units; fitted coefficients are explicitly separate.
use super::Tuning;
use tore_formats::{
    Result,
    aircraft::{Aircraft, Envelope},
    flight_model::profile::FlightProfile,
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mass {
    pub empty_lbs: f64,
    pub internal_fuel_lbs: f64,
    pub max_takeoff_lbs: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Propulsion {
    pub military_thrust_lbf: f64,
    pub afterburner_thrust_lbf: f64,
    pub military_fuel_lbs_per_second: f64,
    pub afterburner_fuel_lbs_per_second: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Aerodynamics {
    /// Original speed (ft/s), altitude (ft MSL) polygons indexed by G.
    pub envelopes: Vec<Envelope>,
    pub loaded_drag_percent: f64,
    pub loaded_elevator_percent: f64,
    pub g_pull_drag_f8: f64,
    pub roll_limit_rad_per_second: f64,
    /// Imported fixed8 flap-lift coefficient.
    pub flaps_lift_f8: f64,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Equipment {
    /// Fitted actuator/visual response, not decoded native timing.
    pub deployment_seconds: f64,
    pub exhaust_seconds: f64,
    pub control_seconds: f64,
    pub throttle_rate_per_second: f64,
    pub afterburner_throttle: f64,
    pub ground_clearance_ft: f64,
}
#[derive(Clone, Debug, PartialEq)]
pub struct Configuration {
    pub(crate) controls: Option<super::handling::Profile>,
    // Resolve once even when the legacy/hybrid subset is the only available data.
    // Failure remains explicit and prevents native activation; no zero defaults.
    joined_native: std::result::Result<
        std::sync::Arc<tore_formats::flight_model::diagnostic::Configuration>,
        String,
    >,
    pub mass: Mass,
    pub propulsion: Propulsion,
    pub aerodynamics: Aerodynamics,
    /// Recovered departure, landing, device drag, velocity bounds and flags.
    /// Velocity bounds and rudder/bay/wheel drag remain diagnostic in this adapter.
    pub native: FlightProfile,
    pub equipment: Equipment,
    /// Reviewed carrier hook or explicitly authored concept equipment.
    pub hook_available: bool,
    pub tuning: Tuning,
    /// Required PT turbulence coefficient; mutable event state lives outside configuration.
    pub turbulence_percent: i16,
}
impl Configuration {
    pub(super) fn from_aircraft(
        a: &Aircraft,
        tuning: Tuning,
        equipment: Equipment,
    ) -> Result<Self> {
        let number = |key: &str| -> Result<f64> {
            let token = a
                .fields
                .get(key)
                .or_else(|| a.object.get(key))
                .ok_or_else(|| std::io::Error::other(format!("missing flight field {key}")))?;
            Ok(token.number()? as f64)
        };
        let result = Self {
            hook_available: matches!(
                a.id,
                tore_formats::aircraft::AircraftId::F18
                    | tore_formats::aircraft::AircraftId::F14
                    | tore_formats::aircraft::AircraftId::A4E
                    | tore_formats::aircraft::AircraftId::Faxx
            ),
            controls: if matches!(
                a.id,
                tore_formats::aircraft::AircraftId::F14
                    | tore_formats::aircraft::AircraftId::A4E
                    | tore_formats::aircraft::AircraftId::X31
                    | tore_formats::aircraft::AircraftId::Mig29
                    | tore_formats::aircraft::AircraftId::Su27
                    | tore_formats::aircraft::AircraftId::Mig21
                    | tore_formats::aircraft::AircraftId::Su25
                    | tore_formats::aircraft::AircraftId::Mig23
                    | tore_formats::aircraft::AircraftId::Su35
                    | tore_formats::aircraft::AircraftId::F22
                    | tore_formats::aircraft::AircraftId::Faxx
            ) {
                Some(super::handling::Profile::from_aircraft(a)?)
            } else {
                None
            },
            joined_native: tore_formats::flight_model::diagnostic::Configuration::from_aircraft(a)
                .map(std::sync::Arc::new)
                .map_err(|e| e.to_string()),
            mass: Mass {
                empty_lbs: number("weight")?,
                internal_fuel_lbs: number("internalFuel")?,
                max_takeoff_lbs: number("maxTakeoffWeight")?,
            },
            propulsion: Propulsion {
                military_thrust_lbf: number("thrust")?,
                afterburner_thrust_lbf: number("aftThrust")?,
                military_fuel_lbs_per_second: number("fuelConsumption")?,
                afterburner_fuel_lbs_per_second: number("aftFuelConsumption")?,
            },
            aerodynamics: Aerodynamics {
                envelopes: a.envelopes.clone(),
                loaded_drag_percent: number("loadedDrag")?,
                loaded_elevator_percent: number("loadedElevator")?,
                g_pull_drag_f8: number("_gpullDrag")?,
                roll_limit_rad_per_second: number("_brv.x.max")?.to_radians(),
                flaps_lift_f8: number("flapsLift")?,
            },
            turbulence_percent: i16::try_from(
                a.fields
                    .get("turbulencePercent")
                    .ok_or_else(|| std::io::Error::other("missing turbulencePercent"))?
                    .number()?,
            )
            .map_err(|_| std::io::Error::other("turbulencePercent outside signed word"))?,
            native: FlightProfile::from_fields(&a.fields)?,
            equipment,
            tuning,
        };
        result.validate()?;
        Ok(result)
    }
    pub fn joined_native(&self) -> Result<&tore_formats::flight_model::diagnostic::Configuration> {
        let n = self.joined_native.as_deref().map_err(|e| {
            std::io::Error::other(format!("native flight configuration unavailable: {e}"))
        })?;
        if n.profile != self.native
            || n.envelopes != self.aerodynamics.envelopes
            || n.empty_weight as f64 != self.mass.empty_lbs
            || n.max_weight as f64 != self.mass.max_takeoff_lbs
            || n.thrust as f64 != self.propulsion.military_thrust_lbf
            || n.ab_thrust as f64 != self.propulsion.afterburner_thrust_lbf
            || n.drag_loading as f64 != self.aerodynamics.loaded_drag_percent
            || n.elevator_loading as f64 != self.aerodynamics.loaded_elevator_percent
            || n.pull_drag as f64 != self.aerodynamics.g_pull_drag_f8
            || n.flaps_lift as f64 != self.aerodynamics.flaps_lift_f8
            || (n.axes[0][1] as f64).to_radians() != self.aerodynamics.roll_limit_rad_per_second
        {
            return Err(std::io::Error::other(
                "edited host configuration differs from resolved native configuration; native activation is unavailable",
            ));
        }
        Ok(n)
    }
    pub fn validate(&self) -> Result<()> {
        let m = self.mass;
        let p = self.propulsion;
        let a = &self.aerodynamics;
        let e = self.equipment;
        let positive = [
            m.empty_lbs,
            m.max_takeoff_lbs,
            p.military_thrust_lbf,
            a.roll_limit_rad_per_second,
        ];
        let nonnegative = [
            m.internal_fuel_lbs,
            p.afterburner_thrust_lbf,
            p.military_fuel_lbs_per_second,
            p.afterburner_fuel_lbs_per_second,
            a.loaded_drag_percent,
            a.loaded_elevator_percent,
            a.g_pull_drag_f8,
            a.flaps_lift_f8,
            e.throttle_rate_per_second,
            e.ground_clearance_ft,
        ];
        if self.turbulence_percent < 0 {
            return Err(std::io::Error::other("negative turbulencePercent"));
        }
        if positive.iter().any(|v| !v.is_finite() || *v <= 0.)
            || nonnegative.iter().any(|v| !v.is_finite() || *v < 0.)
            || m.empty_lbs + m.internal_fuel_lbs > m.max_takeoff_lbs
            || [e.deployment_seconds, e.exhaust_seconds, e.control_seconds]
                .iter()
                .any(|v| !v.is_finite() || *v < 1. / 120.)
            || !e.afterburner_throttle.is_finite()
            || !(0. ..=1.).contains(&e.afterburner_throttle)
        {
            return Err(std::io::Error::other(
                "invalid mass, propulsion, aerodynamic or equipment configuration",
            ));
        }
        if !a.envelopes.iter().any(|e| e.g == 1)
            || a.envelopes.len() > 64
            || a.envelopes.iter().any(|e| {
                !(-20..=30).contains(&e.g)
                    || !(3..=20).contains(&e.points.len())
                    || e.points
                        .iter()
                        .any(|p| p.iter().any(|v| !v.is_finite() || *v < 0.) || p[1] > 200000.)
            })
        {
            return Err(std::io::Error::other(
                "invalid flight envelopes (1G required)",
            ));
        }
        let n = self.native;
        if n.velocity
            .iter()
            .any(|v| v.minimum > v.maximum || v.acceleration < 0 || v.deceleration < 0)
            || [
                n.landing.forward_fps,
                n.landing.side_fps,
                n.landing.descent_fps,
                n.landing.pitch_degrees,
                n.landing.roll_degrees,
                n.departure.warning_delay,
                n.departure.stall_delay,
                n.departure.severity,
            ]
            .iter()
            .any(|v| *v < 0)
        {
            return Err(std::io::Error::other("invalid native flight limits"));
        }
        self.tuning.validate()?;
        Ok(())
    }
}
