use crate::{Result, invalid, slice, u16_at, u32_at};

pub struct Pic {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<u8>,
    pub mask: Vec<bool>,
    pub palette: Vec<[u8; 3]>,
    pub glyphs: Vec<[usize; 3]>,
}
impl Pic {
    pub fn parse(data: &[u8]) -> Result<Self> {
        slice(data, 0, 64)?;
        let kind = u16_at(data, 0)?;
        let (width, height) = (u32_at(data, 2)?, u32_at(data, 6)?);
        if kind > 1
            || width == 0
            || height == 0
            || width > 8192
            || height > 8192
            || width * height > 4_194_304
        {
            return Err(invalid("unsupported menu PIC dimensions/kind"));
        }
        if u32_at(data, 10)? != 64 || data[50..64].iter().any(|b| *b != 0) {
            return Err(invalid("invalid PIC header"));
        }
        let source = slice(data, 64, u32_at(data, 14)?)?;
        let pal = slice(data, u32_at(data, 18)?, u32_at(data, 22)?)?;
        if pal.len() > 768 || pal.len() % 3 != 0 || pal.iter().any(|b| *b > 63) {
            return Err(invalid("invalid 6-bit PIC palette"));
        }
        let palette = pal
            .chunks_exact(3)
            .map(|rgb| std::array::from_fn(|i| ((rgb[i] as u16 * 255 + 31) / 63) as u8))
            .collect();
        let mut pixels = vec![0; width * height];
        let mut mask = vec![kind == 0; width * height];
        if kind == 0 {
            if source.len() != pixels.len() {
                return Err(invalid("PIC raster length mismatch"));
            }
            pixels.copy_from_slice(source);
            let row_offset = u32_at(data, 34)?;
            let row_size = u32_at(data, 38)?;
            if row_offset != 0 || row_size != 0 {
                if row_size != height * 4 {
                    return Err(invalid("invalid PIC row table"));
                }
                slice(data, row_offset, row_size)?;
                for row in 0..height {
                    if u32_at(data, row_offset + row * 4)? != 64 + row * width {
                        return Err(invalid("invalid PIC row offset"));
                    }
                }
            }
        } else {
            let spans = slice(data, u32_at(data, 26)?, u32_at(data, 30)?)?;
            let mut total = 0;
            let mut terminated = false;
            if spans.len() % 10 != 0 {
                return Err(invalid("invalid PIC span table size"));
            }
            for (index, span) in spans.chunks_exact(10).enumerate() {
                let (y, x, end) = (u16_at(span, 0)?, u16_at(span, 2)?, u16_at(span, 4)?);
                if y == 65535 {
                    if (index + 1) * 10 != spans.len() {
                        return Err(invalid("trailing PIC spans"));
                    }
                    terminated = true;
                    break;
                }
                if y >= height || end < x || end >= width {
                    return Err(invalid("PIC span outside image"));
                }
                let count = end - x + 1;
                let row = y * width + x;
                pixels[row..row + count].copy_from_slice(slice(source, u32_at(span, 6)?, count)?);
                mask[row..row + count].fill(true);
                total += count;
            }
            if !terminated || total != source.len() {
                return Err(invalid("incomplete PIC spans"));
            }
        }
        let mut glyphs = Vec::new();
        let glyph_offset = u32_at(data, 42)?;
        if glyph_offset != 0 {
            for glyph in slice(data, glyph_offset, 256 * 6)?.chunks_exact(6) {
                let values = [u16_at(glyph, 0)?, u16_at(glyph, 2)?, u16_at(glyph, 4)?];
                if values[0] + values[1] > width || values[2] > height {
                    return Err(invalid("glyph outside PIC strip"));
                }
                glyphs.push(values);
            }
        }
        Ok(Self {
            width,
            height,
            pixels,
            mask,
            palette,
            glyphs,
        })
    }
    pub fn rgba(&self, base: &[[u8; 3]; 256]) -> Vec<u8> {
        let mut palette = *base;
        palette[..self.palette.len()].copy_from_slice(&self.palette);
        self.pixels
            .iter()
            .enumerate()
            .flat_map(|(i, p)| {
                let rgb = palette[*p as usize];
                [
                    rgb[0],
                    rgb[1],
                    rgb[2],
                    if self.mask[i] && (self.glyphs.is_empty() || *p != 255) {
                        255
                    } else {
                        0
                    },
                ]
            })
            .collect()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn span_mask_preserves_opaque_zero_and_rejects_out_of_bounds() {
        let mut data = vec![0; 85];
        data[0] = 1;
        for (at, value) in [(2, 2u32), (6, 1), (10, 64), (14, 1), (26, 65), (30, 20)] {
            data[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        // One opaque index-zero pixel at x=1, followed by the span terminator.
        data[67] = 1;
        data[69] = 1;
        data[75..77].copy_from_slice(&65535u16.to_le_bytes());
        let p = Pic::parse(&data).unwrap();
        assert_eq!(p.mask, [false, true]);
        let rgba = p.rgba(&[[12, 34, 56]; 256]);
        assert_eq!(rgba[3], 0);
        assert_eq!(&rgba[4..], &[12, 34, 56, 255]);
        data[69] = 2;
        assert!(Pic::parse(&data).is_err());
    }
    #[test]
    fn bounds_and_palette_expansion() {
        let mut data = vec![0; 71];
        for (at, value) in [(2, 2u32), (6, 2), (10, 64), (14, 4), (18, 68), (22, 3)] {
            data[at..at + 4].copy_from_slice(&value.to_le_bytes());
        }
        data[68..].copy_from_slice(&[63, 0, 31]);
        let pic = Pic::parse(&data).unwrap();
        assert_eq!(&pic.rgba(&[[0; 3]; 256])[..4], &[255, 0, 125, 255]);
        for end in 0..data.len() {
            assert!(Pic::parse(&data[..end]).is_err());
        }
        data[68] = 64;
        assert!(Pic::parse(&data).is_err());
    }
}
