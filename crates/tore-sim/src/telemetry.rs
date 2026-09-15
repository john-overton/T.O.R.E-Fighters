//! Gauge-independent truth channels. Feet/knots for flight quantities, SI for air.
//! Instrument errors, lag and calibration belong to separate sensor/gauge models.
use crate::{
    attitude::{Basis, dot},
    flight::State,
};
use tore_formats::Result;
const FPS_PER_KNOT: f64 = 1.6878098571;
#[derive(Clone, Copy, Debug)]
pub struct Atmosphere {
    pub temperature_k: f64,
    pub static_pressure_pa: f64,
}
impl Atmosphere {
    /// NASA Glenn three-zone engineering atmosphere approximation, not retail weather
    /// or a precision ISA altimeter. Explicit range avoids extrapolating indefinitely.
    pub fn standard(altitude_msl_ft: f64) -> Result<Self> {
        if !altitude_msl_ft.is_finite() || !(-2000. ..=100000.).contains(&altitude_msl_ft) {
            return Err(std::io::Error::other(
                "atmosphere altitude outside model range",
            ));
        }
        let h = altitude_msl_ft * 0.3048;
        let (t, p) = if h < 11000. {
            let t = 15.04 - 0.00649 * h;
            (t, 101.29 * ((t + 273.1) / 288.08).powf(5.256))
        } else if h < 25000. {
            (-56.46, 22.65 * (1.73 - 0.000157 * h).exp())
        } else {
            let t = -131.21 + 0.00299 * h;
            (t, 2.488 * ((t + 273.1) / 216.6).powf(-11.388))
        };
        Ok(Self {
            temperature_k: t + 273.1,
            static_pressure_pa: p * 1000.,
        })
    }
    pub fn density_kg_m3(self) -> f64 {
        self.static_pressure_pa / (286.9 * self.temperature_k)
    }
}
#[derive(Clone, Copy, Debug)]
pub struct EnvironmentReading {
    pub terrain_msl_ft: f64,
    pub wind_world_fps: [f64; 3],
    pub atmosphere: Atmosphere,
}
#[derive(Clone, Copy, Debug)]
pub struct AirData {
    pub tick: u64,
    pub altitude_msl_ft: f64,
    pub altitude_agl_ft: f64,
    pub true_airspeed_knots: f64,
    pub equivalent_airspeed_knots: f64,
    pub ground_speed_knots: f64,
    pub mach: f64,
    pub vertical_speed_fpm: f64,
    pub angle_of_attack_deg: Option<f64>,
    pub sideslip_deg: Option<f64>,
    pub heading_true_deg: f64,
    pub pitch_deg: f64,
    pub bank_deg: f64,
    pub load_factor_g: f64,
    pub density_kg_m3: f64,
    pub dynamic_pressure_pa: f64,
    pub static_pressure_pa: f64,
    pub temperature_k: f64,
    /// Not aliases of TAS: pitot/static calibration and gauge errors are unmodeled.
    pub indicated_airspeed_knots: Option<f64>,
    pub calibrated_airspeed_knots: Option<f64>,
    /// Requires pressure reference/calibration, not geometric world height.
    pub indicated_altitude_ft: Option<f64>,
    pub pressure_altitude_ft: Option<f64>,
}
impl AirData {
    /// ASL here explicitly means above *mean* sea level, the same datum as MSL.
    pub fn altitude_asl_ft(self) -> f64 {
        self.altitude_msl_ft
    }
    pub fn sample(s: &State, e: EnvironmentReading) -> Result<Self> {
        let a = e.atmosphere;
        if s.position
            .iter()
            .chain(s.velocity.iter())
            .chain(e.wind_world_fps.iter())
            .any(|v| !v.is_finite())
            || [
                e.terrain_msl_ft,
                a.temperature_k,
                a.static_pressure_pa,
                s.yaw,
                s.pitch,
                s.bank,
                s.g,
            ]
            .iter()
            .any(|v| !v.is_finite())
            || a.temperature_k <= 0.
            || a.static_pressure_pa <= 0.
        {
            return Err(std::io::Error::other("invalid air-data inputs"));
        }
        let v = std::array::from_fn(|i| s.velocity[i] - e.wind_world_fps[i]);
        let speed = dot(v, v).sqrt();
        let tas = speed / FPS_PER_KNOT;
        let density = a.density_kg_m3();
        let reference = Atmosphere::standard(0.)?.density_kg_m3();
        let b = Basis::new(s.yaw, s.pitch, s.bank);
        let forward = dot(v, b.forward);
        let side = dot(v, b.right);
        let up = dot(v, b.up);
        Ok(Self {
            tick: s.ticks,
            altitude_msl_ft: s.position[1],
            altitude_agl_ft: s.position[1] - e.terrain_msl_ft,
            true_airspeed_knots: tas,
            equivalent_airspeed_knots: tas * (density / reference).sqrt(),
            ground_speed_knots: s.velocity[0].hypot(s.velocity[2]) / FPS_PER_KNOT,
            mach: speed * 0.3048 / (1.4 * 287.05 * a.temperature_k).sqrt(),
            vertical_speed_fpm: s.velocity[1] * 60.,
            angle_of_attack_deg: (speed > 1e-6).then(|| (-up).atan2(forward).to_degrees()),
            sideslip_deg: (speed > 1e-6).then(|| (side / speed).clamp(-1., 1.).asin().to_degrees()),
            heading_true_deg: s.yaw.to_degrees().rem_euclid(360.),
            pitch_deg: s.pitch.to_degrees(),
            bank_deg: s.bank.to_degrees(),
            load_factor_g: s.g,
            density_kg_m3: density,
            dynamic_pressure_pa: 0.5 * density * (speed * 0.3048).powi(2),
            static_pressure_pa: a.static_pressure_pa,
            temperature_k: a.temperature_k,
            indicated_airspeed_knots: None,
            calibrated_airspeed_knots: None,
            indicated_altitude_ft: None,
            pressure_altitude_ft: None,
        })
    }
}

/// Authoritative last fixed-step response. Rates are projections of the applied
/// rotation vector on the pre-step body axes, never wrapped Euler differences.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Maneuver {
    pub tick: u64,
    pub commanded_g: f64,
    pub lift_g: f64,
    pub achieved_g: f64,
    /// Positive right wing down / nose up / nose right, radians per second.
    pub body_rates_rad_per_second: [f64; 3],
    pub rudder_command: f64,
    pub rudder_deflection: f64,
    pub effective_rudder: f64,
    /// None: legacy adapter does not implement the native departure state machine.
    pub departure: Option<tore_formats::flight_model::departure::DepartureMode>,
    pub stall_severity_f8: i32,
}
