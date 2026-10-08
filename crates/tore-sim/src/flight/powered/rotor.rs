//! One lifting rotor (VTOL overhaul design 4.4): momentum theory for the
//! inflow, a linear blade-element thrust law, and a first-order disk tilt
//! with blowback. The rotor speed, the engines and the airframe that carries
//! the rotor belong to the force law that owns it: the single-rotor
//! helicopters ([`super::helicopter`]) now, the CH-47's tandem pair (P3) and
//! the V-22's nacelle rotors (P5) later, through the same [`RotorModel`].
//!
//! Each tick the loads come from the start-of-tick state, and the rotor's two
//! lagged states (its induced velocity and its disk tilt, [`Rotor`]) move
//! toward their targets:
//!
//! - **Thrust**, along the disk normal:
//!   `T = rho A (Nr OmegaR)² (sigma a / 2) [theta0 (1/3 + mu²/2) - lambda / 2]`
//!   with `lambda = (Vc + vi) / (Nr OmegaR)`. `Vc` is the air velocity along
//!   the disk normal (positive when the rotor moves into its thrust, as in a
//!   climb), `mu` the in-plane speed over the tip speed.
//! - **Induced velocity** `vi` follows its target with a 0.1 s lag (dynamic
//!   inflow). The target is the momentum theory root of
//!   `vi sqrt(Vx² + (Vc + vi)²) = T / (2 rho A)`, found from the previous
//!   `vi` by a fixed number of safeguarded Newton steps (no tolerance test),
//!   so it keeps to the branch it is on (working state or windmill). In the
//!   vortex ring region (descending through the disk at up to twice the hover
//!   induced velocity, with little in-plane flow) the target rises toward an
//!   empirical curve; out of ground effect it is multiplied by
//!   `1 - (R / (k z))²` at hub height `z`.
//! - **Power**, `P = T (Vc + kappa vi) + P0 (1 + 4.65 mu²)` with
//!   `P0 = rho A (Nr OmegaR)³ sigma Cd0 / 8`. The `T Vc` term carries climb
//!   and parasite power and drives autorotation when it goes negative.
//! - **Disk tilt** follows the cyclic command, the blowback away from the
//!   in-plane airflow, and the retreating blade stall's pitch-up and roll,
//!   with a 0.08 s flapping lag. The rotor's rate damping (the disk lagging
//!   the body by `16 / (gamma Omega)` times its rate) is left to the owner as
//!   an implicit damping moment, [`RotorOutput::tilt_stiffness`] times
//!   [`RotorOutput::rate_lag_seconds`], so it stays stable at 120 Hz.
//!
//! The hazards a player can switch off with the Easy flight physics cheat
//! (design 4.12, slice P8) are flags in [`Hazards`]: vortex ring state,
//! retreating blade stall and rotor stall here; torque and dynamic rollover
//! in the owners.
//!
//! Every constant marked fitted is an agent decision of 2026-10-08, tuned to
//! the design's section 10 acceptance numbers.

use super::state::Rotor;
use crate::{
    attitude::{Vector, dot},
    models::variety::RotorParameters,
};

/// Blade section lift slope, per radian.
pub const LIFT_SLOPE: f64 = 5.7;
/// Induced power factor (kappa) over ideal momentum theory.
pub const INDUCED_POWER_FACTOR: f64 = 1.15;
/// Growth of profile power with advance ratio.
const ADVANCE_PROFILE_FACTOR: f64 = 4.65;
/// Dynamic inflow lag, seconds.
pub const INFLOW_SECONDS: f64 = 0.1;
/// Flapping lag of the disk tilt, seconds.
pub const FLAPPING_SECONDS: f64 = 0.08;
/// Safeguarded Newton steps for the momentum inflow, always all of them.
const INFLOW_STEPS: usize = 24;
/// Disk tilt authority lost deep in the vortex ring state (design 4.4).
const VORTEX_RING_TILT_LOSS: f64 = 0.3;
/// Thrust buffet amplitude deep in the vortex ring state, a share of the
/// thrust, and its frequency (fitted; a fixed sinusoid of the tick).
const VORTEX_RING_BUFFET: f64 = 0.04;
const VORTEX_RING_BUFFET_HZ: f64 = 3.;
/// Retreating blade stall: thrust lost at full stall, the advance ratio
/// past onset where it is full, and the disk tilt aft and toward the
/// retreating side it adds (fitted).
const BLADE_STALL_THRUST_LOSS: f64 = 0.1;
const BLADE_STALL_WIDTH: f64 = 0.06;
const BLADE_STALL_TILT_DEGREES: f64 = 4.;
/// Rotor stall: below 72 percent rotor speed the blades start to stall and
/// by 68 percent they are stalled: thrust collapses and profile power grows
/// so much that even autorotation cannot bring the rotor back (fitted).
const ROTOR_STALL_SPEED: [f64; 2] = [0.72, 0.68];
const ROTOR_STALL_THRUST_LOSS: f64 = 0.6;
const ROTOR_STALL_PROFILE_GROWTH: f64 = 8.;
/// Lateral flapping toward the advancing side per unit advance ratio, as a
/// share of the blowback (fitted).
const LATERAL_FLAPPING_SHARE: f64 = 0.3;
/// Disk tilt limit from the shaft, rad.
const TILT_LIMIT: f64 = 0.45;
/// Ground effect never takes more than half the induced velocity.
const GROUND_EFFECT_FLOOR: f64 = 0.5;
/// Ground effect fades out by this many hover induced velocities of
/// in-plane speed.
const GROUND_EFFECT_FADE: f64 = 1.5;
/// slug/ft³ per kg/m³.
const SLUG_FT3_PER_KG_M3: f64 = 0.001_940_320;

/// Air density, slug/ft³, in the standard atmosphere at `altitude_ft`
/// (clamped to the atmosphere model's range).
pub fn air_density(altitude_ft: f64) -> f64 {
    crate::telemetry::Atmosphere::standard(altitude_ft.clamp(-2_000., 100_000.))
        .map(|a| a.density_kg_m3() * SLUG_FT3_PER_KG_M3)
        .unwrap_or(0.002_377)
}

/// Sea-level standard density, slug/ft³.
pub fn sea_level_density() -> f64 {
    air_density(0.)
}

/// The hazards the Easy flight physics cheat removes (design 4.12). All on
/// by default; slice P8 builds them from the cheat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hazards {
    /// Main-rotor torque reaction and tail rotor imbalance.
    pub torque: bool,
    /// The vortex ring state's extra inflow, buffet and tilt loss.
    pub vortex_ring: bool,
    /// Retreating blade stall's pitch-up, roll and thrust loss.
    pub blade_stall: bool,
    /// Rotor stall at low rotor speed. Off, the rotor speed cannot fall
    /// below 85 percent in flight.
    pub rotor_stall: bool,
    /// Dynamic rollover on the ground.
    pub dynamic_rollover: bool,
}

impl Hazards {
    /// Every hazard on: the normal physics.
    pub const ALL: Self = Self {
        torque: true,
        vortex_ring: true,
        blade_stall: true,
        rotor_stall: true,
        dynamic_rollover: true,
    };
    /// Every hazard off: the Easy flight physics cheat.
    pub const NONE: Self = Self {
        torque: false,
        vortex_ring: false,
        blade_stall: false,
        rotor_stall: false,
        dynamic_rollover: false,
    };
}

impl Default for Hazards {
    fn default() -> Self {
        Self::ALL
    }
}

/// `x²(3 - 2x)` on `x` clamped to 0..1.
pub(super) fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0., 1.);
    x * x * (3. - 2. * x)
}

/// One rotor's fixed numbers, derived from its parameters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotorModel {
    /// Radius, ft, and disk area, ft².
    pub radius_ft: f64,
    pub area_ft2: f64,
    /// Tip speed at 100 percent rotor speed, ft/s, and the rotor's angular
    /// speed there, rad/s.
    pub tip_speed_fps: f64,
    pub omega: f64,
    pub solidity: f64,
    /// +1 for a rotor turning counter-clockwise seen from above (torque
    /// yaws the airframe nose right; the retreating blade is on the left),
    /// -1 clockwise.
    pub turn: f64,
    /// Hub height above the centre of gravity, ft.
    pub hub_height_ft: f64,
    /// Hub moment per radian of disk tilt at 100 percent rotor speed,
    /// ft·lbf.
    pub hub_stiffness: f64,
    collective_rad: [f64; 2],
    cyclic_rad: [f64; 2],
    blowback: f64,
    lock_number: f64,
    profile_drag: f64,
    ground_effect_constant: f64,
    vortex_ring_rise: f64,
    /// Advance ratio at the never-exceed speed.
    never_exceed_mu: f64,
    /// Blade loading (thrust coefficient over solidity) at the reference
    /// weight in a sea-level hover.
    reference_loading: f64,
}

/// A rotor's shaft axes in world coordinates: the shaft (thrust with no
/// tilt) and the directions a forward and a right disk tilt lean it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DiskFrame {
    pub shaft: Vector,
    pub forward: Vector,
    pub right: Vector,
}

/// What a rotor sees and is asked to do this tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RotorInput {
    pub frame: DiskFrame,
    /// The hub's velocity through the air, world axes, ft/s.
    pub air_velocity: Vector,
    /// Air density, slug/ft³.
    pub density: f64,
    /// Rotor speed as a share of 100 percent.
    pub rotor_speed: f64,
    /// Collective lever, 0..1.
    pub collective: f64,
    /// Cyclic, [forward, right] stick travel, -1..1 each.
    pub cyclic: [f64; 2],
    /// Hub height above the surface below it, ft; none out of ground effect.
    pub hub_height_agl_ft: Option<f64>,
    /// Simulation seconds, for the vortex ring buffet.
    pub seconds: f64,
    pub hazards: Hazards,
}

/// A rotor's loads and its condition this tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RotorOutput {
    /// Thrust along [`Self::normal`], lbf.
    pub thrust_lbf: f64,
    /// Disk normal, world axes.
    pub normal: Vector,
    /// The hub moment of the disk tilt, world axes, ft·lbf (the thrust's
    /// own lever arm about the centre of gravity is the owner's).
    pub hub_moment: Vector,
    /// Aerodynamic power the rotor absorbs, ft·lbf/s.
    pub power: f64,
    /// Air velocity along the disk normal and in the disk plane, ft/s.
    pub axial_fps: f64,
    pub in_plane_fps: f64,
    pub advance_ratio: f64,
    /// Hover induced velocity at this thrust, ft/s.
    pub hover_induced_fps: f64,
    /// Where the induced velocity and the disk tilt are heading.
    pub induced_target_fps: f64,
    pub tilt_target: [f64; 2],
    /// How deep in the vortex ring state, retreating blade stall and rotor
    /// stall the rotor is, 0..1 each.
    pub vortex_ring: f64,
    pub blade_stall: f64,
    pub rotor_stall: f64,
    /// Hub moment per radian of tilt now, ft·lbf, and the disk's lag behind
    /// the body's rate, seconds: their product (plus the thrust times the
    /// hub's lever arm, times the lag) is the rotor's rate damping.
    pub tilt_stiffness: f64,
    pub rate_lag_seconds: f64,
}

impl RotorModel {
    /// The model of one rotor of `p`, turning `turn` (+1 counter-clockwise
    /// from above), on an aircraft of `reference_weight_lbs` per rotor.
    pub fn new(p: &RotorParameters, turn: f64, reference_weight_lbs: f64) -> Self {
        let tip_speed_fps = p.tip_speed_fps();
        let area_ft2 = p.disk_area_ft2();
        let density = sea_level_density();
        Self {
            radius_ft: p.radius_ft,
            area_ft2,
            tip_speed_fps,
            omega: tip_speed_fps / p.radius_ft,
            solidity: p.solidity,
            turn,
            hub_height_ft: p.hub_height_ft,
            hub_stiffness: p.hub_stiffness * reference_weight_lbs * p.hub_height_ft.abs().max(1.),
            collective_rad: p.collective_degrees.map(f64::to_radians),
            cyclic_rad: p.cyclic_degrees.map(f64::to_radians),
            blowback: p.blowback,
            lock_number: p.lock_number,
            profile_drag: p.profile_drag,
            ground_effect_constant: p.ground_effect_constant,
            vortex_ring_rise: p.vortex_ring_rise,
            never_exceed_mu: p.never_exceed_kt * 1.687_81 / tip_speed_fps,
            reference_loading: reference_weight_lbs
                / (density * area_ft2 * p.solidity * tip_speed_fps * tip_speed_fps),
        }
    }

    /// Blade collective pitch, rad, for a lever position 0..1.
    pub fn blade_pitch(&self, lever: f64) -> f64 {
        let [low, high] = self.collective_rad;
        low + (high - low) * lever
    }

    /// The lever position, unclamped, for a blade pitch.
    pub fn lever_for_pitch(&self, pitch: f64) -> f64 {
        let [low, high] = self.collective_rad;
        (pitch - low) / (high - low)
    }

    /// Blade profile drag coefficient.
    pub fn profile_drag(&self) -> f64 {
        self.profile_drag
    }

    /// Disk tilt at full stick, [longitudinal, lateral], rad.
    pub fn cyclic_range(&self) -> [f64; 2] {
        self.cyclic_rad
    }

    /// Profile power at a rotor speed and advance ratio, ft·lbf/s.
    pub fn profile_power(&self, density: f64, rotor_speed: f64, advance_ratio: f64) -> f64 {
        let tip = rotor_speed * self.tip_speed_fps;
        density * self.area_ft2 * tip * tip * tip * self.solidity * self.profile_drag / 8.
            * (1. + ADVANCE_PROFILE_FACTOR * advance_ratio * advance_ratio)
    }

    /// Power to hover out of ground effect at `thrust_lbf` and 100 percent
    /// rotor speed, ft·lbf/s: the design's rule for rated power (4.4, 8.1).
    pub fn hover_power(&self, thrust_lbf: f64, density: f64) -> f64 {
        let induced = (thrust_lbf.max(0.) / (2. * density * self.area_ft2)).sqrt();
        thrust_lbf * INDUCED_POWER_FACTOR * induced + self.profile_power(density, 1., 0.)
    }

    /// The loads from the start-of-tick `rotor` and where its states are
    /// heading. Changes nothing.
    pub fn evaluate(&self, rotor: &Rotor, i: &RotorInput) -> RotorOutput {
        let frame = i.frame;
        let rho = i.density;
        let nr = i.rotor_speed.max(0.);
        let tip = (nr * self.tip_speed_fps).max(1.);
        let [long, lat] = rotor.tilt;
        let normal = crate::attitude::unit(std::array::from_fn(|k| {
            frame.shaft[k] + long.tan() * frame.forward[k] + lat.tan() * frame.right[k]
        }));
        let v = i.air_velocity;
        let axial = dot(v, normal);
        let in_plane: Vector = std::array::from_fn(|k| v[k] - axial * normal[k]);
        let in_plane_fps = dot(in_plane, in_plane).sqrt();
        let mu = in_plane_fps / tip;
        let theta = self.blade_pitch(i.collective);
        let lambda = (axial + rotor.induced_fps) / tip;
        let scale = rho * self.area_ft2 * tip * tip;
        let mut thrust = scale * self.solidity * LIFT_SLOPE / 2.
            * (theta * (1. / 3. + mu * mu / 2.) - lambda / 2.);
        // Retreating blade stall: its onset advance ratio is the
        // never-exceed speed's at or below the reference blade loading and
        // falls as the blades are loaded harder (a pull, a heavy aircraft,
        // thin air).
        let loading = thrust / (scale * self.solidity);
        // Against the whole airspeed, so the onset is the never-exceed
        // speed whatever the disk's angle to the flight path.
        let airspeed_mu = dot(v, v).sqrt() / tip;
        let onset = self.never_exceed_mu
            * (1. - 0.5 * (loading / self.reference_loading - 1.)).clamp(0.5, 1.);
        let blade_stall = if i.hazards.blade_stall && thrust > 0. {
            smoothstep((airspeed_mu - onset) / BLADE_STALL_WIDTH)
        } else {
            0.
        };
        let rotor_stall = if i.hazards.rotor_stall {
            smoothstep((ROTOR_STALL_SPEED[0] - nr) / (ROTOR_STALL_SPEED[0] - ROTOR_STALL_SPEED[1]))
        } else {
            0.
        };
        thrust *= (1. - BLADE_STALL_THRUST_LOSS * blade_stall)
            * (1. - ROTOR_STALL_THRUST_LOSS * rotor_stall);
        let disk = 2. * rho * self.area_ft2;
        let hover_induced = (thrust.abs() / disk).sqrt();
        // The vortex ring state: descending through the disk at 0 to 2 hover
        // induced velocities with in-plane flow below one. Deepest at a
        // descent of one.
        let (bump, vortex_ring) = if i.hazards.vortex_ring && thrust > 0. && hover_induced > 1. {
            let u = (-axial / hover_induced / 2.).clamp(0., 1.);
            let bump = 4. * u * (1. - u);
            let flow = (1. - in_plane_fps / hover_induced).max(0.);
            (bump, bump * flow * flow)
        } else {
            (0., 0.)
        };
        thrust *= 1.
            + VORTEX_RING_BUFFET
                * vortex_ring
                * (std::f64::consts::TAU * VORTEX_RING_BUFFET_HZ * i.seconds).sin();
        let momentum = momentum_inflow(thrust / disk, axial, in_plane_fps, rotor.induced_fps);
        let ring = hover_induced * (1. + self.vortex_ring_rise * bump);
        let ground = match i.hub_height_agl_ft {
            Some(z) if hover_induced > 0. => {
                let close = if z > 0. {
                    let r = self.radius_ft / (self.ground_effect_constant * z);
                    (1. - r * r).clamp(GROUND_EFFECT_FLOOR, 1.)
                } else {
                    GROUND_EFFECT_FLOOR
                };
                let fade = (in_plane_fps / (GROUND_EFFECT_FADE * hover_induced)).clamp(0., 1.);
                close + (1. - close) * fade
            }
            _ => 1.,
        };
        let induced_target = (momentum + vortex_ring * (ring - momentum)) * ground;
        // Stalled blades drag; they no longer draw power from the air.
        let flow = thrust * (axial + INDUCED_POWER_FACTOR * rotor.induced_fps);
        let flow = if flow < 0. {
            flow * (1. - rotor_stall)
        } else {
            flow
        };
        let power = flow
            + self.profile_power(rho, nr, mu) * (1. + ROTOR_STALL_PROFILE_GROWTH * rotor_stall);
        // Disk tilt: cyclic (weaker in the vortex ring state), blowback away
        // from the in-plane flow, lateral flapping toward the advancing side,
        // and the retreating blade stall's pitch-up and roll.
        let authority = 1. - VORTEX_RING_TILT_LOSS * vortex_ring;
        let forward = dot(v, frame.forward) / tip;
        let right = dot(v, frame.right) / tip;
        let stall = BLADE_STALL_TILT_DEGREES.to_radians() * blade_stall;
        let tilt_target = [
            (i.cyclic[0].clamp(-1., 1.) * self.cyclic_rad[0] * authority
                - self.blowback * forward
                - stall)
                .clamp(-TILT_LIMIT, TILT_LIMIT),
            (i.cyclic[1].clamp(-1., 1.) * self.cyclic_rad[1] * authority - self.blowback * right
                + self.turn * LATERAL_FLAPPING_SHARE * self.blowback * forward
                - self.turn * stall)
                .clamp(-TILT_LIMIT, TILT_LIMIT),
        ];
        let tilt_stiffness = self.hub_stiffness * nr * nr;
        RotorOutput {
            thrust_lbf: thrust,
            normal,
            // A forward tilt pitches the nose down (a turn about +right), a
            // right tilt rolls right (a turn about -forward).
            hub_moment: std::array::from_fn(|k| {
                tilt_stiffness * (long * frame.right[k] - lat * frame.forward[k])
            }),
            power,
            axial_fps: axial,
            in_plane_fps,
            advance_ratio: mu,
            hover_induced_fps: hover_induced,
            induced_target_fps: induced_target,
            tilt_target,
            vortex_ring,
            blade_stall,
            rotor_stall,
            tilt_stiffness,
            rate_lag_seconds: 16. / (self.lock_number * self.omega * nr.max(0.05)),
        }
    }

    /// One tick: the loads from the start-of-tick state, then the inflow and
    /// the disk tilt move toward their targets.
    pub fn step(&self, rotor: &mut Rotor, input: &RotorInput, dt: f64) -> RotorOutput {
        let out = self.evaluate(rotor, input);
        relax(rotor, &out, dt);
        out
    }
}

/// Moves `rotor`'s inflow and disk tilt toward the targets of `out` for
/// `dt`, with their lags.
pub fn relax(rotor: &mut Rotor, out: &RotorOutput, dt: f64) {
    rotor.induced_fps +=
        (out.induced_target_fps - rotor.induced_fps) * (dt / INFLOW_SECONDS).min(1.);
    let flap = (dt / FLAPPING_SECONDS).min(1.);
    for (tilt, target) in rotor.tilt.iter_mut().zip(out.tilt_target) {
        *tilt += (target - *tilt) * flap;
    }
}

/// The momentum-theory induced velocity, ft/s, for a thrust of
/// `c = T / (2 rho A)` (ft²/s², signed), axial flow `axial` and in-plane
/// flow `in_plane`: the root of `v sqrt(in_plane² + (axial + v)²) = c`
/// reached from `previous`. A bracket that always holds a root, refined by
/// Newton steps where they stay inside it and by bisection where they do
/// not, for a fixed number of steps.
pub fn momentum_inflow(c: f64, axial: f64, in_plane: f64, previous: f64) -> f64 {
    if c == 0. || !c.is_finite() {
        return 0.;
    }
    // Negative thrust mirrors the problem: flip the flow and the result.
    let (sign, c, axial, previous) = if c < 0. {
        (-1., -c, -axial, -previous)
    } else {
        (1., c, axial, previous)
    };
    // f(0) = -c < 0. At `(-axial).max(0) + sqrt(c)` both factors are at
    // least sqrt(c), and at `c / in_plane` the root factor is at least
    // `in_plane`, so f >= 0 at the smaller of the two.
    let mut low = 0.;
    let mut high = (-axial).max(0.) + c.sqrt();
    if in_plane > 0. {
        high = f64::min(high, c / in_plane);
    }
    let mut v = previous.clamp(low, high);
    for _ in 0..INFLOW_STEPS {
        let root = (in_plane * in_plane + (axial + v) * (axial + v)).sqrt();
        let f = v * root - c;
        if f < 0. {
            low = v;
        } else {
            high = v;
        }
        let slope = if root > 0. {
            root + v * (axial + v) / root
        } else {
            0.
        };
        let next = if slope > 0. { v - f / slope } else { f64::NAN };
        v = if next >= low && next <= high {
            next
        } else {
            0.5 * (low + high)
        };
    }
    sign * v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hover_root(c: f64, axial: f64) -> f64 {
        -axial / 2. + (axial * axial / 4. + c).sqrt()
    }

    #[test]
    fn momentum_inflow_matches_the_closed_forms_and_glauert() {
        let c: f64 = 2_000.;
        let vh = c.sqrt();
        // Hover and climb: the working-state closed form.
        for axial in [0., 10., 40., 200.] {
            let v = momentum_inflow(c, axial, 0., vh);
            assert!((v - hover_root(c, axial)).abs() < 1e-9, "{axial} {v}");
        }
        // Fast descent: the windmill brake root from below, the working
        // state root from above; each start keeps its branch.
        let axial = -3. * vh;
        let windmill = -axial / 2. - (axial * axial / 4. - c).sqrt();
        assert!((momentum_inflow(c, axial, 0., 0.5 * vh) - windmill).abs() < 1e-9);
        assert!((momentum_inflow(c, axial, 0., 4. * vh) - hover_root(c, axial)).abs() < 1e-9);
        // Forward flight: Glauert's relation holds.
        for (axial, in_plane) in [(-20., 120.), (5., 250.), (-40., 30.)] {
            let v = momentum_inflow(c, axial, in_plane, vh);
            let residual = v * (in_plane * in_plane + (axial + v) * (axial + v)).sqrt() - c;
            assert!(residual.abs() < 1e-6, "{axial} {in_plane} {residual}");
        }
        // Negative thrust mirrors positive thrust.
        assert_eq!(
            momentum_inflow(-c, -10., 50., -vh),
            -momentum_inflow(c, 10., 50., vh)
        );
        assert_eq!(momentum_inflow(0., 10., 5., 3.), 0.);
    }
}
