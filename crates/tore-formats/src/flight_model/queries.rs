//! Static FA contact query contracts; collision geometry remains caller-owned.
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
