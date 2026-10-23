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
pub(super) fn half(v: f32) -> u16 {
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
pub(super) fn float(v: u16) -> f32 {
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
        Self::build_layers(map, pixels, palette, &[])
    }
    pub fn build_layers(map: &Map, pixels: &[u8], palette: &[[u8; 3]; 256], fields: &[u8]) -> Self {
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
            let mut out = Self {
                size: [1, 1, 1],
                records,
                pixels: [0_u16, 0, 0, 0x3c00]
                    .into_iter()
                    .flat_map(u16::to_le_bytes)
                    .collect(),
            };
            out.attach_flat(map, fields);
            return out;
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
                        let cell = p.map(|v| (v / TILE).floor() as usize);
                        let own = own.map(|c| {
                            map.flat.apply_raw(
                                fields,
                                cell[1] * map.cols + cell[0],
                                p.map(|v| (v / TILE).fract()),
                                c,
                            )
                        });
                        let corrected = reference.surface_layers(map, p, pixels, &colors, fields);
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
        out.attach_flat(map, fields);
        out
    }

    fn attach_flat(&mut self, map: &Map, fields: &[u8]) {
        use super::flat::{PATCH_BYTES, SIDE};
        if fields.is_empty() {
            return;
        }
        let old_size = self.size;
        let first_layer = old_size[2] as usize;
        let layers = first_layer + (fields.len() / PATCH_BYTES).div_ceil(100);
        let mut data = vec![0; 1024 * 1024 * layers * 8];
        for layer in 0..first_layer {
            for y in 0..old_size[1] as usize {
                let a = (layer * old_size[1] as usize + y) * old_size[0] as usize * 8;
                let b = (layer * 1024 + y) * 1024 * 8;
                data[b..b + old_size[0] as usize * 8]
                    .copy_from_slice(&self.pixels[a..a + old_size[0] as usize * 8]);
            }
        }
        for (slot, patch) in fields.chunks_exact(PATCH_BYTES).enumerate() {
            let layer = first_layer + slot / 100;
            let x = slot % 10 * SIDE;
            let y = slot % 100 / 10 * SIDE;
            for row in 0..SIDE {
                let at = ((layer * 1024 + y + row) * 1024 + x) * 8;
                data[at..at + SIDE * 8]
                    .copy_from_slice(&patch[row * SIDE * 8..(row + 1) * SIDE * 8]);
            }
        }
        self.size = [1024, 1024, layers as u32];
        self.pixels = data;
        let base = map.cols * map.rows * 512 * 8;
        let mut active_pages = vec![false; map.flat.targets.len()];
        for (tile, &slot) in map.flat.cells.iter().enumerate() {
            if slot != 0 {
                active_pages[map.material(tile)[0] as usize - 1] = true;
            }
            let value = if slot == 0 {
                0
            } else {
                first_layer as u32 * 100 + slot
            };
            self.records[base + tile * 8..base + tile * 8 + 4]
                .copy_from_slice(&value.to_le_bytes());
            self.records[base + tile * 8 + 4..base + tile * 8 + 8]
                .copy_from_slice(&u32::from(map.flat.boundaries[tile]).to_le_bytes());
        }
        let base = base + map.cols * map.rows * 8;
        for (page, &active) in active_pages.iter().enumerate() {
            self.records[base + page * 8..base + page * 8 + 4]
                .copy_from_slice(&u32::from(active).to_le_bytes());
        }
    }

    pub fn flat_response(&self, map: &Map, world: [f32; 2]) -> [f32; 4] {
        if map.cols == 0 || world.iter().any(|v| *v < 0.) {
            return [0., 0., 0., 1.];
        }
        let cell = world.map(|v| (v / TILE).floor() as usize);
        if cell[0] >= map.cols || cell[1] >= map.rows {
            return [0., 0., 0., 1.];
        }
        let tile = cell[1] * map.cols + cell[0];
        if map.flat.cells.get(tile).copied().unwrap_or(0) == 0 {
            return [0., 0., 0., 1.];
        }
        let at = (map.cols * map.rows * 512 + tile) * 8;
        let slot = u32::from_le_bytes(self.records[at..at + 4].try_into().unwrap()) as usize - 1;
        let flags = self.records[at + 4];
        let uv = world.map(|v| (v / TILE).fract());
        let p = uv.map(|v| v * (super::flat::SIDE - 1) as f32);
        let b = p.map(|v| v.floor() as usize);
        let f = p.map(|v| v - v.floor());
        let layer = slot / 100;
        let x = slot % 10 * super::flat::SIDE;
        let y = slot % 100 / 10 * super::flat::SIDE;
        let mut result = [0.; 4];
        for j in 0..2 {
            for i in 0..2 {
                let at = ((layer * 1024 + y + (b[1] + j).min(super::flat::SIDE - 1)) * 1024
                    + x
                    + (b[0] + i).min(super::flat::SIDE - 1))
                    * 8;
                let w =
                    if i == 0 { 1. - f[0] } else { f[0] } * if j == 0 { 1. - f[1] } else { f[1] };
                for (k, value) in result.iter_mut().enumerate() {
                    *value += float(u16::from_le_bytes(
                        self.pixels[at + k * 2..at + k * 2 + 2].try_into().unwrap(),
                    )) * w;
                }
            }
        }
        let fade = super::flat::fade(uv, flags);
        for v in &mut result[..3] {
            *v *= fade;
        }
        result
    }
    pub fn flat_surface(&self, map: &Map, world: [f32; 2], source: [f32; 3]) -> [f32; 3] {
        super::flat::apply(source, self.flat_response(map, world), 1.)
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
