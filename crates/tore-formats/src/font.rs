//! Decode the restricted bitmap-writing grammar of retail FNT glyph routines.
use crate::{Result, invalid, module, slice, u32_at};
pub struct Glyph {
    pub advance: usize,
    pub pixels: Vec<(usize, usize)>,
}
pub struct Font {
    pub height: usize,
    pub glyphs: Vec<Glyph>,
}
impl Font {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let (c, base) = module::code(data)?;
        slice(c, 0, 0x804)?;
        let height = u32_at(c, 0)?;
        if !(1..=64).contains(&height) {
            return Err(invalid("font height outside bound"));
        }
        let mut glyphs = Vec::new();
        for i in 0..256 {
            let mut p = u32_at(c, 4 + i * 4)?
                .checked_sub(base)
                .ok_or_else(|| invalid("font pointer underflow"))?;
            if p < 0x804 {
                return Err(invalid("font pointer into header"));
            }
            let advance = u32_at(c, 0x404 + i * 4)?;
            if advance > 64 {
                return Err(invalid("font advance outside bound"));
            }
            let (mut x, mut y) = (0i32, 0usize);
            let mut pixels = Vec::new();
            let mut returned = false;
            for _ in 0..4096 {
                let mut op = slice(c, p, 1)?[0];
                let mut width = 1;
                if op == 0x66 {
                    p += 1;
                    op = slice(c, p, 1)?[0];
                    width = 2;
                }
                match op {
                    0xc3 => {
                        returned = true;
                        break;
                    }
                    0x03 if slice(c, p, 2)?[1] == 0xf9 => {
                        y += 1;
                        x = 0;
                        p += 2;
                    }
                    0x88 | 0x89 => {
                        if op == 0x89 && width == 1 {
                            width = 4;
                        }
                        let m = slice(c, p + 1, 1)?[0];
                        if m & 0x3f != 7 {
                            return Err(invalid("unsupported glyph store"));
                        }
                        let d = match m >> 6 {
                            0 => {
                                p += 2;
                                0
                            }
                            1 => {
                                let d = slice(c, p + 2, 1)?[0] as i8 as i32;
                                p += 3;
                                d
                            }
                            2 => {
                                let d = u32_at(c, p + 2)? as u32 as i32;
                                p += 6;
                                d
                            }
                            _ => return Err(invalid("register glyph store")),
                        };
                        for j in 0..width {
                            let xx = x
                                .checked_add(d)
                                .and_then(|v| v.checked_add(j))
                                .ok_or_else(|| invalid("glyph coordinate overflow"))?;
                            if !(0..64).contains(&xx) || y >= height {
                                return Err(invalid("glyph outside cell"));
                            }
                            pixels.push((xx as usize, y));
                        }
                    }
                    0x83 | 0x81 if slice(c, p + 1, 1)?[0] == 0xc7 => {
                        let d = if op == 0x83 {
                            let d = slice(c, p + 2, 1)?[0] as i8 as i32;
                            p += 3;
                            d
                        } else {
                            let d = u32_at(c, p + 2)? as u32 as i32;
                            p += 6;
                            d
                        };
                        x = x
                            .checked_add(d)
                            .ok_or_else(|| invalid("glyph coordinate overflow"))?;
                    }
                    _ => return Err(invalid("unsupported FNT glyph instruction")),
                }
            }
            if !returned {
                return Err(invalid("glyph instruction bound exceeded"));
            }
            glyphs.push(Glyph { advance, pixels });
        }
        Ok(Self { height, glyphs })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decode_bitmap_grammar_and_reject_native_instructions() {
        let mut c = vec![0; 0x804];
        c[..4].copy_from_slice(&2u32.to_le_bytes());
        for i in 0..256 {
            c[4 + i * 4..8 + i * 4].copy_from_slice(&0x1804u32.to_le_bytes());
            c[0x404 + i * 4..0x408 + i * 4].copy_from_slice(&2u32.to_le_bytes());
        }
        c.extend_from_slice(&[0x66, 0x89, 0x07, 0x03, 0xf9, 0x88, 0x47, 1, 0xc3]);
        let f = Font::parse(&module::fixture(&c)).unwrap();
        assert_eq!(f.glyphs[65].pixels, [(0, 0), (1, 0), (1, 1)]);
        c[0x804] = 0xe8;
        assert!(Font::parse(&module::fixture(&c)).is_err());
    }
}
