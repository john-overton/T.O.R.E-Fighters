//! A gun row that points away from the nose (B52-6, 2026-11-15, fitted).
//!
//! An aircraft record's gun row carries `slewH`, `slewP`, `slewLimitH` and
//! `slewLimitP`: where the mount points at rest and the half-arc either side
//! of it, 182 units a degree. The B-52's M61 reads heading 32,760 (180
//! degrees) and 30 degree arcs, the Tu-26's and the Tu-95's `T23_1.JT`
//! turrets the same heading. Nothing read these before: every aircraft gun
//! fired along the nose, the B-52's tail gun included.
//!
//! The rule, in one place so every reader agrees:
//!
//! - A mount whose heading is zero is a **forward gun** and stays exactly as
//!   it was: the nose line, whatever its pitch word says (the helicopters'
//!   chain guns read a 20 degree depression that has never been applied, and
//!   is not applied now).
//! - A mount with a heading is **slewed**. Its fire direction is the
//!   mount's rest direction in the aircraft's frame: heading clockwise from
//!   the nose seen from above, pitch up, rotated with the aircraft.
//! - The arcs are what an AI gunner may swing the barrel through
//!   ([`Slew::contains`]); a human fires along the rest direction.
//!
//! Provenance: heading, pitch and arcs spec-derived (retail records); the
//! forward/slewed split, the centre-line fire for a human and the AI's swing
//! inside the arcs are fitted (agent decisions, 2026-11-15).
use crate::attitude::{Basis, Vector, dot, unit};
use std::f64::consts::PI;

/// Angle units a degree in the record words (the surface units' scale).
const UNITS_PER_DEGREE: f64 = 182.;

/// Fitted (BMB-AI, 2026-11-16, agent decision): how fast the AI gunner can
/// swing a slewed barrel, degrees a second. A target whose lead point sweeps
/// the mount faster than this cannot be followed: the gun holds fire (a
/// fighter crossing close astern at a few hundred feet).
pub const BARREL_SLEW_DEG_PER_S: f64 = 30.;
/// Fitted (BMB-AI): the barrel trails the lead point by this many seconds of
/// the lead's angular rate (the gunner's tracking lag).
pub const BARREL_LAG_S: f64 = 0.2;
/// Fitted (BMB-AI): one standard deviation of the gunner's aim error, degrees
/// on each of the two axes, redrawn about twice a second (a burst's laying),
/// and the extra scatter of each round. Together with the lag they make an
/// AI tail gunner dangerous and not a laser: a fighter in the rear arc takes
/// hits on roughly 10 to 25 percent of the rounds at 2,000 to 3,000 ft (the
/// M61's range is 7,500 ft), most of them at under 1,000 ft, and a few at
/// 5,000. The exact lead and instant swing hit 89 of 128 rounds
/// (docs/spec/ai-gun-employment.md, BMB-AI).
pub const AIM_ERROR_DEG: f64 = 1.0;
pub const ROUND_SCATTER_DEG: f64 = 0.35;

/// Where a gun mount points at rest and how far it may swing.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Slew {
    /// `[heading, pitch]` at rest, radians: heading clockwise from the nose
    /// (positive right), pitch up.
    pub rest: [f64; 2],
    /// The half-arc either side of `rest`, `[heading, pitch]` radians.
    pub limit: [f64; 2],
}

impl Slew {
    /// The slew of a record row's words (`slewH`, `slewP`, `slewLimitH`,
    /// `slewLimitP`). A row with no heading is a forward mount: all zero, so
    /// a forward gun carries exactly what it carried before.
    pub fn from_record(words: [i32; 4]) -> Self {
        if words[0] == 0 {
            return Self::default();
        }
        let angle = |w: i32| f64::from(w) / UNITS_PER_DEGREE * PI / 180.;
        Self {
            rest: [angle(words[0]), angle(words[1])],
            limit: [angle(words[2]).abs(), angle(words[3]).abs()],
        }
    }

    /// Whether the mount points away from the nose.
    pub fn is_slewed(&self) -> bool {
        self.rest[0] != 0.
    }

    /// The mount's own frame on an aircraft whose frame is `body`: the body
    /// turned by the heading about its up axis, then by the pitch about the
    /// turned right axis. A forward mount is `body` itself, bit for bit.
    pub fn basis(&self, body: Basis) -> Basis {
        if !self.is_slewed() {
            return body;
        }
        let (sh, ch) = self.rest[0].sin_cos();
        let forward: Vector = std::array::from_fn(|i| body.forward[i] * ch + body.right[i] * sh);
        let right: Vector = std::array::from_fn(|i| body.right[i] * ch - body.forward[i] * sh);
        let (sp, cp) = self.rest[1].sin_cos();
        Basis {
            right,
            up: std::array::from_fn(|i| body.up[i] * cp - forward[i] * sp),
            forward: std::array::from_fn(|i| forward[i] * cp + body.up[i] * sp),
        }
    }

    /// Whether `delta`, a vector from the muzzle in the aircraft's frame
    /// `body`, lies inside the arcs: its heading and pitch from the rest
    /// direction each within the half-arc.
    pub fn contains(&self, body: Basis, delta: Vector) -> bool {
        let mount = self.basis(body);
        let forward = dot(delta, mount.forward);
        let right = dot(delta, mount.right);
        let up = dot(delta, mount.up);
        if forward <= 0. {
            return false;
        }
        right.atan2(forward).abs() <= self.limit[0]
            && up.atan2(forward.hypot(right)).abs() <= self.limit[1]
    }

    /// Whether the barrel can follow a lead point sweeping the mount at
    /// `heading_rate` and `pitch_rate` radians a second at `pitch` radians of
    /// elevation (the lead direction's own rates, [`Aim`]-style):
    /// [`BARREL_SLEW_DEG_PER_S`] (fitted, BMB-AI).
    ///
    /// [`Aim`]: crate::ai::gunnery::Aim
    pub fn can_follow(heading_rate: f64, pitch_rate: f64, pitch: f64) -> bool {
        let sweep = (heading_rate * pitch.cos()).hypot(pitch_rate);
        sweep.is_finite() && sweep <= BARREL_SLEW_DEG_PER_S.to_radians()
    }

    /// The direction a slewed barrel actually fires along when the exact lead
    /// is `lead` and that lead is moving at `rates` (`[heading, pitch]`, radians
    /// a second): the lead, set back by the gunner's lag
    /// ([`BARREL_LAG_S`]) and moved by a deterministic aim error
    /// ([`AIM_ERROR_DEG`] a burst, [`ROUND_SCATTER_DEG`] a round) drawn from
    /// `burst` and `round` alone (fitted, BMB-AI). A forward gun never calls
    /// this: it is laid by the aircraft and fires as before.
    pub fn laid_direction(lead: Vector, rates: [f64; 2], burst: u64, round: u64) -> Vector {
        let lead = unit(lead);
        if !lead.iter().all(|v| v.is_finite()) {
            return lead;
        }
        // The two axes across the lead: the way world heading grows (+x for a
        // lead along +z) and the way elevation grows, both unit and square to it.
        let across = unit([lead[2], 0., -lead[0]]);
        let (across, up) = if across.iter().all(|v| v.is_finite()) && across != [0.; 3] {
            let up = [
                lead[1] * across[2] - lead[2] * across[1],
                lead[2] * across[0] - lead[0] * across[2],
                lead[0] * across[1] - lead[1] * across[0],
            ];
            (across, up)
        } else {
            ([1., 0., 0.], [0., 0., 1.])
        };
        let flat = lead[0].hypot(lead[2]);
        let mut seed = burst.wrapping_mul(0x9e37_79b9_7f4a_7c15) ^ 0x2545_f491_4f6c_dd1d;
        let bias = [normal(&mut seed), normal(&mut seed)];
        let mut seed = seed ^ round.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        let scatter = [normal(&mut seed), normal(&mut seed)];
        let sigma = |degrees: f64| degrees.to_radians();
        let across_error = -rates[0] * flat * BARREL_LAG_S
            + bias[0] * sigma(AIM_ERROR_DEG)
            + scatter[0] * sigma(ROUND_SCATTER_DEG);
        let up_error = -rates[1] * BARREL_LAG_S
            + bias[1] * sigma(AIM_ERROR_DEG)
            + scatter[1] * sigma(ROUND_SCATTER_DEG);
        unit(std::array::from_fn(|i| {
            lead[i] + across[i] * across_error + up[i] * up_error
        }))
    }

    /// The mount's rest direction in the world for an aircraft in `body`.
    pub fn direction(&self, body: Basis) -> Vector {
        self.basis(body).forward
    }

    /// `mount`, an offset in the aircraft's frame `body` given by its
    /// `[right, up, forward]` components, expressed in the mount's own
    /// frame, so a sight that places the muzzle with the mount's basis
    /// finds the same point.
    pub fn mount_in_frame(&self, body: Basis, mount: Vector) -> Vector {
        if !self.is_slewed() {
            return mount;
        }
        let frame = self.basis(body);
        let world: Vector = std::array::from_fn(|i| {
            body.right[i] * mount[0] + body.up[i] * mount[1] + body.forward[i] * mount[2]
        });
        [
            dot(world, unit(frame.right)),
            dot(world, unit(frame.up)),
            dot(world, unit(frame.forward)),
        ]
    }
}

/// A unit normal draw by summing four uniforms (no transcendental functions,
/// so every platform draws the same): the state advances by splitmix64.
fn normal(state: &mut u64) -> f64 {
    let mut sum = 0.;
    for _ in 0..4 {
        *state = state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = *state;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^= z >> 31;
        sum += (z >> 11) as f64 / (1u64 << 53) as f64;
    }
    // Four uniforms have mean 2 and variance 1/3.
    (sum - 2.) * 3f64.sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn aft() -> Slew {
        Slew::from_record([32_760, 0, 5_460, 5_460])
    }

    #[test]
    fn a_record_heading_of_32760_is_180_degrees_with_30_degree_arcs() {
        let slew = aft();
        assert!((slew.rest[0] - PI).abs() < 1e-12);
        assert!(slew.rest[1].abs() < 1e-12);
        assert!((slew.limit[0] - 30_f64.to_radians()).abs() < 1e-12);
        assert!(slew.is_slewed());
    }

    #[test]
    fn a_forward_mount_is_the_nose_whatever_its_pitch_word_says() {
        // The helicopters' chain guns read a 20 degree depression.
        let slew = Slew::from_record([0, -3_640, 8_190, 8_190]);
        assert_eq!(slew, Slew::default());
        let body = Basis::new(0.7, 0.2, 0.1);
        assert_eq!(slew.basis(body), body);
        assert_eq!(slew.direction(body), body.forward);
        assert_eq!(slew.mount_in_frame(body, [1., 2., 3.]), [1., 2., 3.]);
    }

    #[test]
    fn an_aft_mount_fires_back_along_the_tail_and_turns_with_the_aircraft() {
        for (yaw, pitch, bank) in [(0., 0., 0.), (1.1, 0.3, 0.4), (-2.5, -0.2, 1.2)] {
            let body = Basis::new(yaw, pitch, bank);
            let direction = aft().direction(body);
            for i in 0..3 {
                assert!(
                    (direction[i] + body.forward[i]).abs() < 1e-9,
                    "{direction:?}"
                );
            }
            let mount = aft().basis(body);
            assert!((dot(mount.right, mount.forward)).abs() < 1e-9);
            assert!((dot(mount.up, mount.forward)).abs() < 1e-9);
        }
    }

    #[test]
    fn a_turret_that_points_up_and_back_tilts_with_its_pitch_word() {
        // The Tu-95's dorsal turret: heading 180, pitch 20 up.
        let slew = Slew::from_record([32_760, 3_640, 8_190, 910]);
        let body = Basis::new(0., 0., 0.);
        let direction = slew.direction(body);
        assert!(direction[2] < -0.9 && direction[1] > 0.3, "{direction:?}");
    }

    #[test]
    fn the_arcs_bound_the_swing_either_side_of_the_rest_direction() {
        let slew = aft();
        let body = Basis::new(0., 0., 0.);
        // Dead astern, 20 degrees off to either side, 40 degrees off, ahead.
        assert!(slew.contains(body, [0., 0., -1.]));
        for degrees in [20_f64, -20.] {
            let a = degrees.to_radians();
            assert!(slew.contains(body, [a.sin(), 0., -a.cos()]));
        }
        let a = 40_f64.to_radians();
        assert!(!slew.contains(body, [a.sin(), 0., -a.cos()]));
        assert!(!slew.contains(body, [0., a.sin(), -a.cos()]));
        assert!(!slew.contains(body, [0., 0., 1.]));
    }

    /// The share of rounds that stay inside an aircraft's contact sphere
    /// ([`crate::combat::live::AIRCRAFT_RADIUS_FT`]) at `range_ft` when the barrel is
    /// laid on a still lead dead astern, over many bursts of four rounds.
    fn hit_share(range_ft: f64, rates: [f64; 2]) -> f64 {
        let lead = [0., 0., -1.];
        let (mut hits, mut rounds) = (0, 0);
        for burst in 0..4_000u64 {
            for round in 0..4u64 {
                let laid = Slew::laid_direction(lead, rates, burst, round);
                let cross = (laid[0] * laid[0] + laid[1] * laid[1]).sqrt() / -laid[2];
                hits += u32::from(cross * range_ft <= crate::combat::live::AIRCRAFT_RADIUS_FT);
                rounds += 1;
            }
        }
        f64::from(hits) / f64::from(rounds)
    }

    /// BMB-AI (2026-11-16, fitted): the AI tail gunner lays the barrel with
    /// an error of about 1 degree (redrawn twice a second), a lag behind a
    /// moving lead and a limit on how fast the barrel swings. A fighter in
    /// the rear arc takes hits on 10 to 25 percent of the rounds at 2,000 to
    /// 3,000 ft (the exact, instant swing hit 89 of 128 rounds), nearly all at
    /// 500 ft and few at 5,000 ft.
    #[test]
    fn a_tail_gunner_is_dangerous_up_close_and_not_a_laser() {
        for range in [2_000., 2_500., 3_000.] {
            let share = hit_share(range, [0.; 2]);
            assert!((0.10..=0.26).contains(&share), "{range} ft: {share}");
        }
        let (close, mid, far) = (
            hit_share(500., [0.; 2]),
            hit_share(1_000., [0.; 2]),
            hit_share(5_000., [0.; 2]),
        );
        assert!(
            close > 0.9 && mid > 0.5 && far < 0.06,
            "{close} {mid} {far}"
        );
        // The lag: a lead sweeping at 20 degrees a second is trailed by four
        // degrees, so almost nothing reaches a fighter 2,500 ft off.
        assert!(hit_share(2_500., [20_f64.to_radians(), 0.]) < 0.02);
    }

    #[test]
    fn the_laid_barrel_is_deterministic_and_the_same_for_the_same_burst_and_round() {
        let lead = unit([0.1, 0.05, -1.]);
        let a = Slew::laid_direction(lead, [0.01, -0.02], 17, 3);
        assert_eq!(a, Slew::laid_direction(lead, [0.01, -0.02], 17, 3));
        assert_ne!(a, Slew::laid_direction(lead, [0.01, -0.02], 18, 3));
        assert_ne!(a, Slew::laid_direction(lead, [0.01, -0.02], 17, 4));
        // A round of the same burst moves by the scatter only (a degree or two at most).
        let b = Slew::laid_direction(lead, [0.01, -0.02], 17, 4);
        assert!(dot(a, b) > 0.999, "{}", dot(a, b));
        assert!((dot(a, a) - 1.).abs() < 1e-12);
    }

    #[test]
    fn the_barrel_follows_a_lead_only_up_to_its_swing_rate() {
        let below = (BARREL_SLEW_DEG_PER_S - 1.).to_radians();
        let above = (BARREL_SLEW_DEG_PER_S + 1.).to_radians();
        assert!(Slew::can_follow(below, 0., 0.));
        assert!(Slew::can_follow(0., below, 0.));
        assert!(!Slew::can_follow(above, 0., 0.));
        assert!(!Slew::can_follow(0., above, 0.));
        // Heading rate counts for its cosine of elevation: the same rate is
        // easy to follow overhead.
        assert!(Slew::can_follow(above, 0., 1.4));
        assert!(!Slew::can_follow(f64::NAN, 0., 0.));
    }

    #[test]
    fn the_mount_offset_in_the_mounts_frame_places_the_muzzle_at_the_same_point() {
        let slew = aft();
        let body = Basis::new(0.8, 0.1, -0.3);
        let mount = [0.5, 2., -92.];
        let direct: Vector = std::array::from_fn(|i| {
            body.right[i] * mount[0] + body.up[i] * mount[1] + body.forward[i] * mount[2]
        });
        let frame = slew.basis(body);
        let local = slew.mount_in_frame(body, mount);
        let placed: Vector = std::array::from_fn(|i| {
            frame.right[i] * local[0] + frame.up[i] * local[1] + frame.forward[i] * local[2]
        });
        for i in 0..3 {
            assert!((direct[i] - placed[i]).abs() < 1e-9);
        }
    }
}
