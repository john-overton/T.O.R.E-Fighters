//! Reviewed terrain arithmetic, not a world query or a live contact producer.
//! See docs/formats/native-land-contact.md. All positions are fixed8 feet.
use super::{div32, mul_div};
use crate::{Result, invalid};

/// FA 0x4d65c4 uses an imported 1024-dword seed table, then ONE integer Newton step.
/// Replacing it with a host square root changes normal rounding.
#[derive(Clone, Debug)]
pub struct SqrtTable([u32; 1024]);
impl SqrtTable {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 4096 {
            return Err(invalid("native square-root table requires 1024 dwords"));
        }
        let mut values = [0; 1024];
        for (v, b) in values.iter_mut().zip(bytes.chunks_exact(4)) {
            *v = u32::from_le_bytes(b.try_into().unwrap());
        }
        Ok(Self(values))
    }
    pub fn root(&self, n: u32) -> u32 {
        let (index_shift, seed_shift) = if n & 0xfc000000 != 0 {
            (22, 13)
        } else if n & 0x03f00000 != 0 {
            (16, 16)
        } else if n & 0x000fc000 != 0 {
            (10, 19)
        } else if n & 0x00003c00 != 0 {
            (4, 22)
        } else {
            (0, 24)
        };
        let seed = self.0[(n >> index_shift) as usize] >> seed_shift;
        if seed == 0 {
            0
        } else {
            (n / seed).wrapping_add(seed) >> 1
        }
    }
}

/// FA 0x4a8d30: (third-second) cross (first-second), arithmetic halving,
/// table root, then signed truncation to a 32767-scale word normal.
pub fn normal(
    table: &SqrtTable,
    first: [i16; 3],
    second: [i16; 3],
    third: [i16; 3],
) -> Result<[i16; 3]> {
    let a = std::array::from_fn::<_, 3, _>(|i| i32::from(third[i]) - i32::from(second[i]));
    let b = std::array::from_fn::<_, 3, _>(|i| i32::from(first[i]) - i32::from(second[i]));
    let mut n = [
        a[1].wrapping_mul(b[2])
            .wrapping_sub(a[2].wrapping_mul(b[1])),
        a[2].wrapping_mul(b[0])
            .wrapping_sub(a[0].wrapping_mul(b[2])),
        a[0].wrapping_mul(b[1])
            .wrapping_sub(a[1].wrapping_mul(b[0])),
    ];
    while n.iter().any(|v| v.wrapping_abs() > 20000) {
        n = n.map(|v| v >> 1);
    }
    let sum = n
        .iter()
        .fold(0i32, |sum, v| sum.wrapping_add(v.wrapping_mul(*v)));
    let divisor = (table.root(sum as u32) & 0xffff) as i32;
    let mut out = [0; 3];
    for (o, v) in out.iter_mut().zip(n) {
        *o = div32(v.wrapping_mul(32767), divisor)? as i16;
    }
    Ok(out)
}

/// FA 0x42dda0. Ending exactly on the plane is no hit unless starting below.
/// The horizontal interpolation uses a wrapping 32-bit product, unlike slopes.
pub fn horizontal_plane(
    start: [i32; 3],
    end: [i32; 3],
    height: i32,
    offset: i32,
) -> Result<Option<[i32; 3]>> {
    let plane = height.wrapping_sub(offset);
    let clamp_start = || [start[0], start[1].max(plane), start[2]];
    if plane > start[1] {
        return Ok(Some(clamp_start()));
    }
    if end[1] >= plane {
        return Ok(None);
    }
    let mut numerator = plane.wrapping_sub(start[1]);
    let mut denominator = end[1].wrapping_sub(start[1]);
    if numerator < 0 {
        numerator = numerator.wrapping_neg();
        denominator = denominator.wrapping_neg();
    }
    while numerator > 200 || denominator > 200 {
        numerator >>= 1;
        denominator >>= 1;
    }
    if denominator == 0 {
        return Ok(Some(clamp_start()));
    }
    let component = |i: usize| -> Result<i32> {
        Ok(start[i].wrapping_add(div32(
            end[i].wrapping_sub(start[i]).wrapping_mul(numerator),
            denominator,
        )?))
    };
    Ok(Some([component(0)?, plane, component(2)?]))
}

/// One plane of a terrain cell, FA 0x42c1a0. The half-open cell and diagonal
/// predicates apply AFTER intersection, including the below-plane start case.
#[derive(Clone, Copy, Debug)]
pub struct CellPlane {
    pub origin: [i32; 3],
    pub normal: [i16; 3],
    pub upper_diagonal: bool,
    pub lower_diagonal: bool,
}
impl CellPlane {
    pub fn intersect(
        self,
        start: [i32; 3],
        end: [i32; 3],
        offset: i32,
    ) -> Result<Option<[i32; 3]>> {
        let n = self.normal.map(i32::from);
        let point = if n[0] == 0 && n[2] == 0 {
            horizontal_plane(start, end, self.origin[1], offset)?
        } else {
            let distance = |p: [i32; 3]| -> Result<i32> {
                let dot = (p[0].wrapping_sub(self.origin[0]) >> 8)
                    .wrapping_mul(n[0])
                    .wrapping_add((p[2].wrapping_sub(self.origin[2]) >> 8).wrapping_mul(n[2]))
                    .wrapping_sub((self.origin[1] >> 8).wrapping_mul(n[1]));
                Ok(div32(dot, n[1])?
                    .wrapping_add(p[1] >> 8)
                    .wrapping_add(offset >> 8))
            };
            let a = distance(start)?;
            let b = distance(end)?;
            if a >= 0 && b >= 0 {
                return Ok(None);
            }
            if a < 0 && b < 0 {
                Some(start)
            } else {
                let mut numerator = a;
                let mut denominator = a.wrapping_sub(b);
                if numerator < 0 {
                    numerator = numerator.wrapping_neg();
                    denominator = denominator.wrapping_neg();
                }
                while numerator >= 10000 || denominator >= 10000 {
                    numerator >>= 1;
                    denominator >>= 1;
                }
                if denominator == 0 {
                    Some(start)
                } else {
                    let mut p = [0; 3];
                    for i in 0..3 {
                        p[i] = start[i].wrapping_add(mul_div(
                            end[i].wrapping_sub(start[i]),
                            numerator,
                            denominator,
                        )?);
                    }
                    p[1] = p[1].max(0);
                    Some(p)
                }
            }
        };
        Ok(point.filter(|p| {
            p[0] >= self.origin[0]
                && p[0] < self.origin[0].wrapping_add(1 << 21)
                && p[2] >= self.origin[2]
                && p[2] < self.origin[2].wrapping_add(1 << 21)
                && if p[2].wrapping_sub(self.origin[2]) >= p[0].wrapping_sub(self.origin[0]) {
                    self.upper_diagonal
                } else {
                    self.lower_diagonal
                }
        }))
    }
}

/// A cell-local result. Candidate angles, dispatcher channels and cache ownership
/// remain separate; this is not a `ContactQueries` implementation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CellHit {
    pub position: [i32; 3],
    pub normal: [i16; 3],
}

/// Vertical subset of FA 0x42bfc0. Heights are A=(x,z), B=(x+1,z),
/// C=(x,z+1), D=(x+1,z+1) T2 bytes, each worth 256 feet.
/// The enclosing traversal/lookup must supply the correct cell and class.
pub fn vertical_cell(
    table: &SqrtTable,
    cell: [i16; 2],
    heights: [u8; 4],
    start: [i32; 3],
    end: [i32; 3],
    offset: i32,
) -> Result<Option<CellHit>> {
    if start[0] != end[0] || start[2] != end[2] {
        return Err(invalid(
            "terrain cell diagnostic requires a vertical segment",
        ));
    }
    let max_height = i32::from(*heights.iter().max().unwrap());
    if i32::from((start[1] >> 16) as i16) > max_height
        && i32::from((end[1] >> 16) as i16) > max_height
    {
        return Ok(None);
    }
    let [x, z] = cell.map(|v| v.wrapping_shl(5));
    let a = [x, i16::from(heights[0]), z];
    let b = [x.wrapping_add(32), i16::from(heights[1]), z];
    let c = [x, i16::from(heights[2]), z.wrapping_add(32)];
    let d = [
        x.wrapping_add(32),
        i16::from(heights[3]),
        z.wrapping_add(32),
    ];
    let normals = [normal(table, c, d, a)?, normal(table, a, d, b)?];
    let plane = CellPlane {
        origin: a.map(|v| i32::from(v) << 16),
        normal: normals[0],
        upper_diagonal: true,
        lower_diagonal: normals[0] == normals[1],
    };
    let mut hit = plane
        .intersect(start, end, offset)?
        .map(|position| CellHit {
            position,
            normal: normals[0],
        });
    if normals[0] != normals[1] {
        let lower = CellPlane {
            normal: normals[1],
            upper_diagonal: false,
            lower_diagonal: true,
            ..plane
        };
        if let Some(position) = lower.intersect(start, end, offset)?
            && hit.is_none_or(|h| {
                super::queries::approximate_distance(position, start)
                    < super::queries::approximate_distance(h.position, start)
            })
        {
            hit = Some(CellHit {
                position,
                normal: normals[1],
            });
        }
    }
    Ok(hit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_selection_and_single_refinement_are_not_exact_sqrt() {
        assert!(SqrtTable::parse(&[0; 4095]).is_err());
        assert!(SqrtTable::parse(&[0; 4097]).is_err());
        let mut t = SqrtTable([0; 1024]);
        assert_eq!(t.root(0), 0);
        // Deliberately synthetic seeds distinguish each shift and one iteration.
        for (n, index, shift) in [
            (100, 100, 24),
            (1024, 64, 22),
            (16384, 16, 19),
            (1048576, 16, 16),
            (67108864, 16, 13),
            (u32::MAX, 1023, 13),
        ] {
            t.0[index] = 7 << shift;
            assert_eq!(t.root(n), (n / 7 + 7) >> 1);
            t.0[index] = 0;
            assert_eq!(t.root(n), 0);
        }
        t.0[100] = 7 << 24;
        assert_eq!(t.root(100), 10);
        t.0[100] = 2 << 24;
        assert_eq!(t.root(100), 26); // not host sqrt(100)
    }

    #[test]
    fn normal_winding_scaling_and_degenerate_failure() {
        let mut t = SqrtTable([0; 1024]);
        t.0[16] = 1024 << 16;
        assert_eq!(
            normal(&t, [0, 0, 32], [32, 0, 32], [0, 0, 0]).unwrap(),
            [0, 32767, 0]
        );
        assert_eq!(
            normal(&t, [0, 0, 0], [32, 0, 32], [0, 0, 32]).unwrap(),
            [0, -32767, 0]
        );
        assert_eq!(
            normal(&t, [0, 2, 32], [32, 3, 32], [0, 0, 0]).unwrap(),
            [-1021, 32703, -2043]
        );
        // Cross product 1,000,000 is arithmetically halved six times to 15,625.
        t.0[58] = 15625 << 13;
        assert_eq!(
            normal(&t, [0, 0, 1000], [1000, 0, 1000], [0, 0, 0]).unwrap(),
            [0, 32767, 0]
        );
        assert!(normal(&t, [0; 3], [0; 3], [0; 3]).is_err());
    }

    #[test]
    fn horizontal_boundary_offset_and_native_ratio_loss() {
        assert_eq!(
            horizontal_plane([1, 256, 2], [3, 0, 4], 0, 0).unwrap(),
            None
        );
        assert_eq!(
            horizontal_plane([1, 0, 2], [3, -1, 4], 0, 0).unwrap(),
            Some([1, 0, 2])
        );
        assert_eq!(
            horizontal_plane([1, -1, 2], [3, 999, 4], 0, 0).unwrap(),
            Some([1, 0, 2])
        );
        assert_eq!(
            horizontal_plane([0, 1000, 0], [1000, -1000, 1000], 0, 256).unwrap(),
            Some([624, -256, 624])
        );
        // 301/603 halves to 150/301 then 75/150, changing the ratio.
        assert_eq!(
            horizontal_plane([0, 301, 0], [1000, -302, 0], 0, 0).unwrap(),
            Some([500, 0, 0])
        );
    }

    fn slope() -> CellPlane {
        CellPlane {
            origin: [0, 0, 0],
            normal: [-1024, 32767, 0],
            upper_diagonal: true,
            lower_diagonal: true,
        }
    }
    #[test]
    fn sloped_intersection_quantization_start_below_and_cell_edges() {
        let p = slope();
        // Horizontal plane equation is truncated to integer feet first.
        assert_eq!(
            p.intersect([8192, 25600, 0], [8192, -25600, 0], 0).unwrap(),
            Some([8192, 256, 0])
        );
        assert_eq!(
            p.intersect([8192, 0, 0], [8192, -256, 0], 0).unwrap(),
            Some([8192, 0, 0])
        );
        assert_eq!(p.intersect([0, 256, 0], [0, 0, 0], 0).unwrap(), None);
        assert_eq!(p.intersect([-1, 1000, 0], [-1, -1000, 0], 0).unwrap(), None);
        assert_eq!(
            p.intersect([1 << 21, 1000, 0], [1 << 21, -1000, 0], 0)
                .unwrap(),
            None
        );
        assert_eq!(
            p.intersect([0, 1000, 1 << 21], [0, -1000, 1 << 21], 0)
                .unwrap(),
            None
        );
        let broken = CellPlane {
            normal: [1, 0, 0],
            ..p
        };
        assert!(broken.intersect([0, 1, 0], [0, -1, 0], 0).is_err());
    }
    #[test]
    fn vertical_cell_constructs_both_triangles_and_rejects_other_segments() {
        let mut t = SqrtTable([0; 1024]);
        t.0[16] = 1024 << 16;
        let start = [1024, 1000000, 1024];
        let end = [1024, -25600, 1024];
        let flat = vertical_cell(&t, [0, 0], [2; 4], start, end, 0)
            .unwrap()
            .unwrap();
        assert_eq!(flat.position, [1024, 131072, 1024]);
        assert_eq!(flat.normal, [0, 32767, 0]);
        let upper = vertical_cell(&t, [0, 0], [0, 1, 0, 0], start, end, 0)
            .unwrap()
            .unwrap();
        assert_eq!(upper.position, [1024, 0, 1024]);
        assert_eq!(upper.normal, [0, 32767, 0]);
        let lower = vertical_cell(
            &t,
            [0, 0],
            [0, 1, 0, 0],
            [65536, 1000000, 0],
            [65536, -25600, 0],
            0,
        )
        .unwrap()
        .unwrap();
        assert!(lower.normal[0] < 0 && lower.normal[2] > 0);
        assert!(lower.position[1] > 0);
        assert!(vertical_cell(&t, [0, 0], [0; 4], start, [1025, 0, 1024], 0).is_err());
        assert_eq!(
            vertical_cell(&t, [0, 0], [0; 4], start, [1024, 999999, 1024], 0).unwrap(),
            None
        );
    }

    #[test]
    fn diagonal_equality_belongs_to_upper_triangle() {
        let upper = CellPlane {
            normal: [0, 32767, 0],
            lower_diagonal: false,
            ..slope()
        };
        let lower = CellPlane {
            upper_diagonal: false,
            lower_diagonal: true,
            ..upper
        };
        for (x, z, want_upper) in [
            (0, 0, true),
            (100, 100, true),
            (100, 101, true),
            (101, 100, false),
        ] {
            assert_eq!(
                upper
                    .intersect([x, 1000, z], [x, -1000, z], 0)
                    .unwrap()
                    .is_some(),
                want_upper
            );
            assert_eq!(
                lower
                    .intersect([x, 1000, z], [x, -1000, z], 0)
                    .unwrap()
                    .is_some(),
                !want_upper
            );
        }
    }
}
