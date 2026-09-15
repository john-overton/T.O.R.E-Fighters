//! Reviewed straight-line weather SH grammar. No imported code is executed.
//! FA dispatch: 7a vertices, 2e/bc fills, 3a circles, 08 points, ea billboards.
use crate::{Result, invalid, module, slice, u16_at};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    Point {
        center: [f32; 3],
        fill: u16,
    },
    Circle {
        center: [f32; 3],
        diameter: i16,
        fill: u16,
    },
    Billboard {
        center: [f32; 3],
        size: [i16; 2],
        texture: String,
        uv: Option<[[u16; 2]; 4]>,
    },
}
#[derive(Clone, Debug, PartialEq)]
pub struct WeatherShape {
    pub primitives: Vec<Primitive>,
    /// Source bounding scale; celestial geometry uses ratios, not world distances.
    pub scale_exponent: u16,
    /// d2 publishes a projected point for a separate consumer (sun glare).
    pub publishes_point: bool,
}
impl WeatherShape {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let (c, _) = module::code(data)?;
        if u16_at(c, 0)? != 0xffff {
            return Err(invalid("weather shape needs header"));
        }
        slice(c, 0, 14)?;
        let mut out = Self {
            primitives: Vec::new(),
            scale_exponent: u16_at(c, 6)? as u16,
            publishes_point: false,
        };
        let mut slots = BTreeMap::new();
        let mut texture = String::new();
        let mut uv = None;
        let mut fill = 0;
        let mut p = 14;
        for _ in 0..8192 {
            let opcode = u16_at(c, p)?;
            let op = opcode & 255;
            let word = |offset| u16_at(c, p + offset);
            let center = |offset| -> Result<[f32; 3]> {
                slots
                    .get(&word(offset)?)
                    .copied()
                    .ok_or_else(|| invalid("unresolved weather vertex"))
            };
            let size = match op {
                0 if opcode == 0 => {
                    if out.primitives.is_empty() {
                        return Err(invalid("empty weather shape"));
                    }
                    return Ok(out);
                }
                0x7a => {
                    let slot = word(8)?;
                    if slot % 8 != 0 {
                        return Err(invalid("invalid weather vertex slot"));
                    }
                    slots.insert(
                        slot,
                        [
                            word(2)? as i16 as f32,
                            word(6)? as i16 as f32,
                            word(4)? as i16 as f32,
                        ],
                    );
                    10
                }
                0x82 => {
                    let count = word(2)?;
                    let dest = word(4)?;
                    if count > 1024 || dest % 8 != 0 || dest + count * 8 > 65536 {
                        return Err(invalid("weather vertex bounds"));
                    }
                    slice(c, p, 6 + count * 6)?;
                    for i in 0..count {
                        slots.insert(
                            dest + i * 8,
                            [
                                word(6 + i * 6)? as i16 as f32,
                                word(10 + i * 6)? as i16 as f32,
                                word(8 + i * 6)? as i16 as f32,
                            ],
                        );
                    }
                    6 + count * 6
                }
                0x2e => {
                    fill = word(2)? as u16;
                    4
                }
                0xbc => {
                    fill = (opcode >> 8) as u16;
                    2
                }
                0x3a => {
                    let diameter = word(4)? as i16;
                    if diameter <= 0 {
                        return Err(invalid("invalid weather circle"));
                    }
                    out.primitives.push(Primitive::Circle {
                        center: center(2)?,
                        diameter,
                        fill,
                    });
                    6
                }
                0x08 => {
                    out.primitives.push(Primitive::Point {
                        center: center(2)?,
                        fill,
                    });
                    4
                }
                0xe2 => {
                    let raw = slice(c, p + 2, 14)?;
                    let end = raw
                        .iter()
                        .position(|b| *b == 0)
                        .ok_or_else(|| invalid("weather texture name unterminated"))?;
                    if end == 0
                        || !raw[..end]
                            .iter()
                            .all(|b| b.is_ascii_alphanumeric() || b"_~.$".contains(b))
                    {
                        return Err(invalid("unsafe weather texture name"));
                    }
                    texture = std::str::from_utf8(&raw[..end])
                        .map_err(|_| invalid("weather texture name"))?
                        .to_ascii_uppercase();
                    16
                }
                0xe4 => {
                    if word(2)? != 4 {
                        return Err(invalid("weather UV count"));
                    }
                    let mut coords = [[0; 2]; 4];
                    for (i, coord) in coords.iter_mut().enumerate() {
                        *coord = [word(4 + i * 4)? as u16, word(6 + i * 4)? as u16];
                    }
                    uv = Some(coords);
                    20
                }
                0xe6 => {
                    uv = None;
                    2
                }
                0xea => {
                    let size = [word(4)? as i16, word(6)? as i16];
                    if texture.is_empty() || size.iter().any(|n| *n <= 0) {
                        return Err(invalid("invalid weather billboard"));
                    }
                    out.primitives.push(Primitive::Billboard {
                        center: center(2)?,
                        size,
                        texture: texture.clone(),
                        uv,
                    });
                    8
                }
                0xd2 => {
                    center(2)?;
                    out.publishes_point = true;
                    8
                }
                0xee => 2,
                0xca | 0xda => 4,
                0x78 => 12,
                _ => {
                    return Err(invalid(&format!(
                        "unsupported weather SH opcode {opcode:04x}"
                    )));
                }
            };
            slice(c, p, size)?;
            p += size;
        }
        Err(invalid("weather shape instruction bound"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn program() -> Vec<u8> {
        let mut c = vec![0; 14];
        c[..2].copy_from_slice(&0xffffu16.to_le_bytes());
        for w in [
            0x7au16,
            3,
            400,
            7,
            0,
            0x2e,
            267,
            0x3a,
            0,
            19,
            0xbc | (158 << 8),
            8,
            0,
            0,
        ] {
            c.extend(w.to_le_bytes());
        }
        c
    }
    #[test]
    fn source_axes_fills_and_truncation() {
        let c = program();
        let s = WeatherShape::parse(&module::fixture(&c)).unwrap();
        assert_eq!(
            s.primitives[0],
            Primitive::Circle {
                center: [3., 7., 400.],
                diameter: 19,
                fill: 267
            }
        );
        assert_eq!(
            s.primitives[1],
            Primitive::Point {
                center: [3., 7., 400.],
                fill: 158
            }
        );
        for end in 0..c.len() {
            assert!(WeatherShape::parse(&module::fixture(&c[..end])).is_err());
        }
        let mut bad = c;
        bad[14] = 0xf0;
        assert!(WeatherShape::parse(&module::fixture(&bad)).is_err());
    }
}
