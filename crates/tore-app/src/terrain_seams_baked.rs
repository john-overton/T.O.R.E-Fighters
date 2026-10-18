//! Cached seam material response. The shader performs one mask lookup and
//! one hardware-filtered tint lookup, without sampling neighbouring art.
use super::{BLEND, HALF_WIDTH, Map, SEGMENTS, TILE, linear};

pub const WIDTH: usize = 1024;
pub const DEPTH: usize = 9;
pub const HEIGHT: usize = DEPTH * 2;
pub const ROWS: usize = 1024 / HEIGHT;
pub const COLUMNS: usize = WIDTH * ROWS;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Baked {
    pub size: [u32; 3],
    pub records: Vec<u8>,
    pub pixels: Vec<u8>,
}

// Finite binary16 material corrections, including negative values and small
// subnormals. Identity is an exact zero correction, not a quantized tint.
fn half(v: f32) -> u16 {
    let bits = v.to_bits();
    let sign = ((bits >> 16) & 0x8000) as u16;
    let e = ((bits >> 23) & 255) as i32 - 112;
    let m = bits & 0x7fffff;
    if e <= 0 {
        if e < -10 {
            return sign;
        }
        let shift = (14 - e) as u32;
        return sign | ((((m | 0x800000) + (1 << (shift - 1))) >> shift) as u16);
    }
    sign | ((((e as u32) << 10) + ((m + 0x1000) >> 13)).min(0x7bff) as u16)
}
fn float(v: u16) -> f32 {
    let e = (v >> 10) & 31;
    let m = v & 1023;
    if e == 0 {
        return f32::from(m) / 16777216. * if v & 0x8000 != 0 { -1. } else { 1. };
    }
    f32::from_bits(
        (u32::from(v & 0x8000) << 16) | ((u32::from(e) + 112) << 23) | (u32::from(m) << 13),
    )
}
fn luminance(v: [f32; 3]) -> f32 {
    v[0] * 0.2126 + v[1] * 0.7152 + v[2] * 0.0722
}

impl Baked {
    pub fn build(map: &Map, pixels: &[u8], palette: &[[u8; 3]; 256]) -> Self {
        let reference = map.pack();
        let colors = palette.map(|rgb| rgb.map(linear));
        let mut records = reference.bytes.clone();
        let mut strips = Vec::new();
        let mut column = 0;
        for tile in 0..map.cols * map.rows {
            for axis in 0..2 {
                let mut start = 0;
                while start < SEGMENTS {
                    if map.record(tile, axis, start)[0] & BLEND == 0 {
                        start += 1;
                        continue;
                    }
                    let mut end = start + 1;
                    while end < SEGMENTS && map.record(tile, axis, end)[0] & BLEND != 0 {
                        end += 1;
                    }
                    let width = end - start + 2;
                    if column % WIDTH + width > WIDTH {
                        column = column.div_ceil(WIDTH) * WIDTH;
                    }
                    strips.push((column, tile, axis, start, end));
                    for s in start..end {
                        let offset = ((tile * 2 + axis) * SEGMENTS + s) * 8 + 4;
                        records[offset..offset + 4]
                            .copy_from_slice(&((column + 1 + s - start + 1) as u32).to_le_bytes());
                    }
                    column += width;
                    start = end;
                }
            }
        }
        if column == 0 {
            return Self {
                size: [1, 1, 1],
                records,
                pixels: [0_u16, 0, 0, 0x3c00]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            };
        }
        let layers = column.div_ceil(COLUMNS);
        let height = if layers > 1 {
            ROWS * HEIGHT
        } else {
            column.div_ceil(WIDTH) * HEIGHT
        };
        let size = [WIDTH as u32, height as u32, layers as u32];
        let mut out = Self {
            size,
            records,
            pixels: vec![0; WIDTH * height * layers * 8],
        };
        for (first, tile, axis, start, end) in strips {
            for col in 0..end - start + 2 {
                let segment = (start as isize + col as isize - 1).clamp(0, 255) as usize;
                let c = first + col;
                for side in 0..2 {
                    for depth in 0..DEPTH {
                        let distance = depth as f32 * HALF_WIDTH / (DEPTH - 1) as f32;
                        let mut p = [
                            (tile % map.cols) as f32 * TILE,
                            (tile / map.cols) as f32 * TILE,
                        ];
                        let boundary = p[axis] + TILE;
                        p[axis] = if depth == 0 && side == 0 {
                            f32::from_bits(boundary.to_bits() - 1)
                        } else {
                            boundary + distance * if side == 0 { -1. } else { 1. }
                        };
                        p[1 - axis] += segment as f32 * 128. + 64.;
                        let own = reference.sample(map, p, pixels, &colors);
                        let corrected = reference.surface(map, p, pixels, &colors);
                        // Apply the run fade once, in the draw shader. Baking
                        // it into the neighbouring column would attenuate a
                        // fully accepted segment beside a short run endpoint.
                        let fade =
                            map.edge_strength(tile, axis, segment as f32 * 128. + 64., BLEND);
                        let rgba = own.zip(corrected).map_or([0., 0., 0., 1.], |(a, b)| {
                            if fade == 0. {
                                [0., 0., 0., luminance(a)]
                            } else {
                                [
                                    (b[0] - a[0]) / fade,
                                    (b[1] - a[1]) / fade,
                                    (b[2] - a[2]) / fade,
                                    luminance(a),
                                ]
                            }
                        });
                        let at = out.address(c, side * DEPTH + depth);
                        for (k, v) in rgba.into_iter().enumerate() {
                            out.pixels[at + k * 2..at + k * 2 + 2]
                                .copy_from_slice(&half(v).to_le_bytes());
                        }
                    }
                }
            }
        }
        out
    }

    fn address(&self, column: usize, row: usize) -> usize {
        let layer = column / COLUMNS;
        let y = column / WIDTH % ROWS * HEIGHT + row;
        ((layer * self.size[1] as usize + y) * WIDTH + column % WIDTH) * 8
    }

    /// Mirrors the draw shader, including its hard outside-band early return.
    /// The extra run-end fade prevents filtering leaking into exclusions.
    pub fn response(&self, map: &Map, world: [f32; 2]) -> [f32; 4] {
        if map.cols == 0 || world.iter().any(|v| *v < 0.) {
            return [0., 0., 0., 1.];
        }
        let cell = world.map(|v| (v / TILE).floor() as usize);
        if cell[0] >= map.cols || cell[1] >= map.rows {
            return [0., 0., 0., 1.];
        }
        let local = world.map(|v| v % TILE);
        let near = local.map(|v| v.min(TILE - v));
        let axis = usize::from(near[1] < near[0]);
        let distance = near[axis];
        if distance >= HALF_WIDTH {
            return [0., 0., 0., 1.];
        }
        let negative = local[axis] < TILE / 2.;
        if (negative && cell[axis] == 0)
            || (!negative && cell[axis] + 1 == [map.cols, map.rows][axis])
        {
            return [0., 0., 0., 1.];
        }
        let owner = cell[1] * map.cols + cell[0] - if negative { [1, map.cols][axis] } else { 0 };
        let along = local[1 - axis] / 128.;
        let segment = along.floor() as usize;
        let at = ((owner * 2 + axis) * SEGMENTS + segment) * 8;
        let flags = self.records[at + 3];
        if flags & BLEND == 0 {
            return [0., 0., 0., 1.];
        }
        let column =
            u32::from_le_bytes(self.records[at + 4..at + 8].try_into().unwrap()) as usize - 1;
        let x = column as f32 + along.fract() - 0.5;
        let y = distance / HALF_WIDTH * (DEPTH - 1) as f32;
        let sx = x.floor() as usize;
        let sy = y.floor() as usize;
        let mut result = [0.; 4];
        for j in 0..2 {
            for i in 0..2 {
                let p = self.address(
                    sx + i,
                    usize::from(negative) * DEPTH + (sy + j).min(DEPTH - 1),
                );
                let w = if i == 0 { 1. - x.fract() } else { x.fract() }
                    * if j == 0 { 1. - y.fract() } else { y.fract() };
                for (k, sum) in result.iter_mut().enumerate() {
                    *sum += float(u16::from_le_bytes(
                        self.pixels[p + k * 2..p + k * 2 + 2].try_into().unwrap(),
                    )) * w;
                }
            }
        }
        let strength = map.edge_strength(owner, axis, local[1 - axis], BLEND);
        [
            result[0] * strength,
            result[1] * strength,
            result[2] * strength,
            result[3],
        ]
    }
    pub fn surface(&self, map: &Map, world: [f32; 2], source: [f32; 3]) -> [f32; 3] {
        let r = self.response(map, world);
        let scale = luminance(source) / r[3].max(1e-6);
        std::array::from_fn(|k| (source[k] + r[k] * scale).max(0.))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn correction_half_conversion_keeps_zero_sign_and_precision() {
        assert_eq!(float(half(0.)), 0.);
        assert_eq!(float(half(1.)), 1.);
        for v in [
            -1., -0.1, -0.000001, 0.000001, 0.0625, 0.1, 0.4, 0.999, 1., 1.003, 2.5, 16.,
        ] {
            assert!((float(half(v)) - v).abs() < v.abs() * 0.001 + 3e-8);
        }
    }
}
