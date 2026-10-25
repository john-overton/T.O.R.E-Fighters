//! Resolves authored physical metadata and explicit fitted class defaults.
//! No weapon-name flight branches. Calibration is recorded in the feature baseline.
use super::energy::{Body, FT, Motor};
use tore_formats::weapons::{SeekerKind, Weapon};

pub fn body(w: &Weapon) -> Body {
    let reference = tore_formats::missile_data::reference(&w.source);
    let surface = super::surface_launched(&w.source);
    let infrared = w.seeker_kind() == SeekerKind::Infrared;
    let mass = reference.map_or_else(
        || {
            if w.weight > 1 {
                f64::from(w.weight) * 0.45359237
            } else if infrared {
                15.
            } else {
                250.
            }
        },
        |r| r.mass_kg,
    );
    let diameter = reference.map_or_else(
        || (0.127 * (mass / 86.).cbrt()).clamp(0.07, 0.6),
        |r| r.diameter_m,
    );
    let range = reference.map_or(f64::from(w.seeker.zones[1].maximum_range) * FT, |r| {
        r.range_km * 1000.
    });
    let strike = w.flags & 0x20000 != 0;
    let cruise = strike && w.flags & super::CRUISE_FLAG != 0 && w.movement.fuel_t > 200;
    let fraction = if cruise { 0.20 } else { 0.35 };
    let fuel = mass * fraction;
    let burn = if cruise {
        3.
    } else {
        (0.2 * (range / 1000.).sqrt()).clamp(2., 6.)
    };
    let sustain = if cruise {
        (1.3 * range / 250.).clamp(30., 1800.)
    } else {
        (0.15 * range / 1000.).clamp(0., 25.)
    };
    let boost_ticks = (burn * 120.).round() as u32;
    let sustain_ticks = (sustain * 120.).round() as u32;
    let boost_fuel = if sustain_ticks == 0 {
        fuel
    } else {
        fuel * 0.65
    };
    let sustain_fuel = fuel - boost_fuel;
    let radar = matches!(
        w.seeker_kind(),
        SeekerKind::ActiveRadar | SeekerKind::SemiActiveRadar
    );
    let mut body = Body {
        launch_elevation: if surface {
            12f64.to_radians()
        } else {
            -std::f64::consts::FRAC_PI_2
        },
        mass,
        diameter,
        boost: Motor {
            ticks: boost_ticks,
            propellant: boost_fuel,
            thrust: boost_fuel * 2200. / burn,
        },
        sustain: Motor {
            ticks: sustain_ticks,
            propellant: sustain_fuel,
            thrust: if sustain > 0. {
                sustain_fuel * if cruise { 10000. } else { 2200. } / sustain
            } else {
                0.
            },
        },
        ignition: if surface {
            0
        } else {
            u32::from(w.movement.ignite_t) * 30
        },
        life: (u32::from(w.movement.remove_t) * 30).max(if cruise {
            boost_ticks + sustain_ticks + 1200
        } else {
            0
        }),
        structural_g: if strike {
            12.
        } else if mass > 400. {
            20.
        } else {
            30.
        },
        lift_coefficient: 12.,
        induced_factor: 0.03,
        drag_scale: tore_formats::missile_data::fallback(&w.source).map_or(1., |r| r.drag_scale),
        cruise_drag: cruise,
        minimum_speed: if cruise
            || super::Guidance::air_to_ground(w) == Some(super::Guidance::Command)
        {
            70.
        } else {
            120.
        },
        navigation_gain: 3.5,
        loft_height: if !surface && !strike && radar && range >= 37040. {
            (range * 0.08).min(6000.)
        } else {
            0.
        },
        loft_range: range * 0.6,
        cruise_feet: strike.then(|| super::Cruise::for_weapon(w)).flatten(),
    };
    body.life = body.life.max((2. * range / 300. * 120.).ceil() as u32);
    if let Some(reference) = reference {
        body.drag_scale = reference.drag_scale;
        if let Some(g) = reference.structural_g {
            body.structural_g = g;
        }
        if reference.imaging && !strike {
            body.structural_g = 40.;
            body.navigation_gain = 4.;
        }
        if let Some([boost, burn, sustain, coast]) = reference.motor {
            body.boost = Motor {
                ticks: (burn * 120.).round() as u32,
                thrust: boost,
                propellant: boost * burn / reference.motor_exhaust_mps,
            };
            body.sustain = Motor {
                ticks: (coast * 120.).round() as u32,
                thrust: sustain,
                propellant: sustain * coast / reference.motor_exhaust_mps,
            };
        }
    }
    body
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub body: Body,
    pub flight: super::energy::Flight,
    pub target: Option<super::energy::Observation>,
    /// Current qualified observation, as distinct from a frozen memory waypoint.
    pub observed: bool,
    pub wind: crate::attitude::Vector,
    pub decoy: super::decoys::Memory,
    pub tracking: super::energy_seeker::Tracking,
}
crate::checkpoint_struct!(Motion {
    body,
    flight,
    target,
    observed,
    wind,
    decoy,
    tracking
});
