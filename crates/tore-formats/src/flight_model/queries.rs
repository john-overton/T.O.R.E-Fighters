//! Static FA contact query contracts; collision geometry remains caller-owned.
use crate::{Result, invalid};

/// FA 0x4747c0. These are instance fields, not the OT structure discriminator.
/// The caller separately resolves the type's landing-eligibility flag 0x8000.
pub fn landing_object_preferred(
    instance_flags: u32,
    word_0x0e: i16,
    instance_kind: u8,
    byte_0xe3: u8,
) -> bool {
    instance_flags & 1 != 0
        && word_0x0e != 0
        && instance_flags & 0x2000 == 0
        && (!matches!(instance_kind, 2 | 4) || byte_0xe3 != 0)
}

/// Inputs to the cache expiry writes at FA 0x42bc5c..0x42bd24.
/// The dispatcher invokes this only after a qualifying expired-cache refresh.
#[derive(Clone, Copy, Debug)]
pub struct CacheLifetime {
    pub now: i32,
    pub byte_0x10: u8,
    pub instance_flags: u32,
    pub speed_f8: i32,
    pub instance_kind: u8,
    pub altitude_f8: i32,
}
impl CacheLifetime {
    /// Whether the source consumes one bound-4 draw. Even a zero result consumes
    /// RNG; callers must stage the draw and cache write in their transaction.
    pub fn needs_random(self) -> bool {
        self.byte_0x10 & 0x80 == 0
            && self.instance_flags & 4 != 0
            && self.speed_f8 <= 0
            && self.instance_kind != 4
    }

    /// Native signed comparisons and wrapping dword deadline arithmetic.
    /// An optional draw is required exactly on the reviewed random branch.
    /// This pure helper does not refresh a cache or choose a global RNG stream.
    pub fn deadline(self, draw_below_four: Option<u8>) -> Result<i32> {
        if self.needs_random() != draw_below_four.is_some()
            || draw_below_four.is_some_and(|v| v >= 4)
        {
            return Err(invalid("ground-cache RNG sample does not match branch"));
        }
        if self.byte_0x10 & 0x80 != 0 {
            return Ok(self.now.wrapping_add(1));
        }
        if self.instance_flags & 4 == 0 {
            return Ok(0x7fff00);
        }
        if let Some(draw) = draw_below_four {
            return Ok(self.now.wrapping_add((8 + i32::from(draw)) << 8));
        }
        let interval =
            256 + if self.altitude_f8 >= 10_000 * 256 {
                256
            } else {
                0
            } + if self.altitude_f8 >= 20_000 * 256 {
                512
            } else {
                0
            };
        Ok(self.now.wrapping_add(interval))
    }
}

/// FA 0x4c66cc: unsigned magnitudes, largest + quarter of each other component.
/// This is an approximate distance, not Euclidean distance or squared distance.
pub fn approximate_distance(a: [i32; 3], b: [i32; 3]) -> i32 {
    let mut d = [0u32; 3];
    for j in 0..3 {
        d[j] = a[j].wrapping_sub(b[j]).wrapping_abs() as u32;
    }
    if d[1] < d[2] {
        d.swap(1, 2);
    }
    if d[0] < d[1] {
        d.swap(0, 1);
    }
    d[0].wrapping_add(d[1] >> 2).wrapping_add(d[2] >> 2) as i32
}
#[derive(Clone, Copy, Debug)]
pub struct SurfaceCandidate {
    pub id: u16,
    pub position_f8: [i32; 3],
    /// cpt flag 0x8000, with any caller/team filters already resolved.
    pub eligible: bool,
    /// Instance active flag plus 0x4747c0 predicate.
    pub preferred: bool,
}
/// FA 0x4ba8e0: reverse inventory order; strict distance improvement keeps first
/// tie. Only if the preferred pass has no candidate does the fallback pass run.
/// This models the landing caller's null-object/disabled optional filters.
pub fn landing_surface(position: [i32; 3], candidates: &[SurfaceCandidate]) -> Option<(u16, i32)> {
    for preferred_only in [true, false] {
        let mut best = None;
        for c in candidates
            .iter()
            .rev()
            .filter(|c| c.eligible && (!preferred_only || c.preferred))
        {
            let mut p = position;
            p[1] = c.position_f8[1];
            let distance = approximate_distance(p, c.position_f8);
            if best.is_none_or(|(_, d)| distance < d) {
                best = Some((c.id, distance));
            }
        }
        if best.is_some() {
            return best;
        }
    }
    None
}
/// FA 0x4abb36 query mask before collision dispatch (0x42b800).
pub fn ground_query_mask(request: u16, object_flags: Option<(u32, u32)>) -> u32 {
    let mut mask = if request & 2 != 0 { 0x203 } else { 3 };
    if request & 4 != 0
        || object_flags.is_some_and(|(instance, kind)| kind & 0x8000 != 0 || instance & 0x4000 == 0)
    {
        mask &= !2;
    }
    mask
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_landing_preference_field_gates() {
        assert!(landing_object_preferred(1, -1, 1, 0));
        assert!(!landing_object_preferred(0, 1, 1, 1));
        assert!(!landing_object_preferred(1, 0, 1, 1));
        assert!(!landing_object_preferred(0x2001, 1, 1, 1));
        for kind in [2, 4] {
            assert!(!landing_object_preferred(1, 1, kind, 0));
            assert!(landing_object_preferred(1, 1, kind, 1));
        }
        // OT type 5 must not be confused with instance kind 4.
        assert!(landing_object_preferred(1, 1, 5, 0));
    }

    #[test]
    fn native_cache_deadline_precedence_rng_and_altitude_edges() {
        let mut c = CacheLifetime {
            now: 100,
            byte_0x10: 0x80,
            instance_flags: 0,
            speed_f8: 0,
            instance_kind: 1,
            altitude_f8: 0,
        };
        assert_eq!(c.deadline(None).unwrap(), 101);
        assert!(c.deadline(Some(0)).is_err());
        c.byte_0x10 = 0;
        assert_eq!(c.deadline(None).unwrap(), 0x7fff00);
        c.instance_flags = 4;
        assert!(c.needs_random());
        assert!(c.deadline(None).is_err());
        for draw in 0..4 {
            assert_eq!(
                c.deadline(Some(draw)).unwrap(),
                100 + (8 + draw as i32) * 256
            );
        }
        assert!(c.deadline(Some(4)).is_err());
        c.instance_kind = 4;
        assert!(!c.needs_random());
        for (altitude, interval) in [
            (-1, 256),
            (10_000 * 256 - 1, 256),
            (10_000 * 256, 512),
            (20_000 * 256 - 1, 512),
            (20_000 * 256, 1024),
        ] {
            c.altitude_f8 = altitude;
            assert_eq!(c.deadline(None).unwrap(), 100 + interval);
        }
        c.instance_kind = 1;
        c.speed_f8 = 1;
        assert!(!c.needs_random());
        c.speed_f8 = -1;
        assert!(c.needs_random());
        c.now = i32::MAX;
        c.byte_0x10 = 0x80;
        assert_eq!(c.deadline(None).unwrap(), i32::MIN);
    }
    #[test]
    fn surface_preference_ties_and_horizontal_metric() {
        assert_eq!(approximate_distance([400, 80, 40], [0; 3]), 430);
        let mut c = [
            SurfaceCandidate {
                id: 1,
                position_f8: [100, 5000, 100],
                eligible: true,
                preferred: true,
            },
            SurfaceCandidate {
                id: 2,
                position_f8: [1, 0, 0],
                eligible: true,
                preferred: false,
            },
        ];
        assert_eq!(landing_surface([0; 3], &c), Some((1, 125)));
        c[0].preferred = false;
        assert_eq!(landing_surface([0; 3], &c), Some((2, 1)));
        c[0].position_f8 = [1, 100, 0];
        assert_eq!(landing_surface([0; 3], &c), Some((2, 1)));
        assert_eq!(landing_surface([0; 3], &[]), None);
    }
    #[test]
    fn request_and_object_flags() {
        assert_eq!(ground_query_mask(0, None), 3);
        assert_eq!(ground_query_mask(2, None), 0x203);
        assert_eq!(ground_query_mask(6, None), 0x201);
        assert_eq!(ground_query_mask(0, Some((0x4000, 0))), 3);
        assert_eq!(ground_query_mask(0, Some((0, 0))), 1);
        assert_eq!(ground_query_mask(0, Some((0x4000, 0x8000))), 1);
    }
}
