//! Bounded nearest-detail static SH projection, not a complete native shape VM.
use crate::{Result, invalid, module, slice, u16_at, u32_at};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug)]
pub struct Face {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<u8>,
    pub uv: Vec<[f32; 2]>,
    pub texture: String,
    pub subtype: u8,
    pub normal: Option<[f32; 3]>,
    pub address: usize,
}
pub struct Shape {
    pub faces: Vec<Face>,
    pub state_words: BTreeSet<usize>,
}
fn word(c: &[u8], p: usize) -> Result<i32> {
    Ok(u16_at(c, p)? as u16 as i16 as i32)
}
fn target(p: usize, d: i32, c: &[u8]) -> Result<usize> {
    let t = p as i64 + d as i64;
    if t < 0 || t >= c.len() as i64 {
        return Err(invalid("shape branch out of range"));
    }
    Ok(t as usize)
}
impl Shape {
    pub fn parse(data: &[u8]) -> Result<Self> {
        Self::with_state(data, &BTreeMap::new())
    }
    pub fn with_state(data: &[u8], state: &BTreeMap<usize, i32>) -> Result<Self> {
        let (c, base) = module::code(data)?;
        let mut slots = BTreeMap::<usize, [f32; 3]>::new();
        let mut colors = BTreeMap::new();
        let mut faces = Vec::new();
        let mut seen = BTreeSet::new();
        let mut state_words = BTreeSet::new();
        let (mut p, mut end, mut texture, mut transform) = (0, None, String::new(), [0.; 3]);
        type Frame = (usize, Option<usize>, Option<([f32; 3], String)>);
        let mut stack: Vec<Frame> = Vec::new();
        let mut finished = false;
        for _ in 0..30000 {
            if stack.len() > 64 {
                return Err(invalid("shape call depth exceeded"));
            }
            let op = slice(c, p, 1)?[0];
            if op == 0 || (op == 0x1e && end.is_none_or(|e| p >= e)) {
                if let Some((ret, old_end, frame)) = stack.pop() {
                    p = ret;
                    end = old_end;
                    if let Some((t, s)) = frame {
                        transform = t;
                        texture = s;
                    }
                    continue;
                }
                finished = true;
                break;
            }
            match op {
                0x38 => {
                    let t = target(p + 3, word(c, p + 1)?, c)?;
                    if t <= p {
                        return Err(invalid("invalid shape scope"));
                    }
                    end = Some(end.unwrap_or(t).max(t));
                    p += 3;
                }
                0x12 => {
                    let t = target(p + 4, word(c, p + 2)?, c)?;
                    stack.push((p + 4, end, None));
                    end = None;
                    p = t;
                }
                0xc4 => {
                    let t = target(p + 16, word(c, p + 14)?, c)?;
                    stack.push((p + 16, end, Some((transform, texture.clone()))));
                    transform[0] += word(c, p + 2)? as f32;
                    transform[1] += word(c, p + 6)? as f32;
                    transform[2] += word(c, p + 4)? as f32;
                    end = None;
                    p = t;
                }
                0xf0 => {
                    let mut start = p + 2;
                    if slice(c, start, 3)? == [0x66, 0x83, 0x3d] {
                        let addr = u32_at(c, start + 3)?;
                        state_words.insert(addr);
                        let imm = slice(c, start + 7, 1)?[0] as i8 as i32;
                        let branch = slice(c, start + 8, 1)?[0];
                        let value = state.get(&addr).copied().unwrap_or(0);
                        let take = match branch {
                            0x74 => value == imm,
                            0x75 => value != imm,
                            _ => return Err(invalid("unsupported shape state guard")),
                        };
                        if take {
                            start = target(start + 10, slice(c, start + 9, 1)?[0] as i8 as i32, c)?;
                        }
                    }
                    let mut next = None;
                    for r in start..(start + 128).min(c.len().saturating_sub(10)) {
                        if c[r] == 0x68 && c[r + 5] == 0x68 && c[r + 10] == 0xc3 {
                            let t = u32_at(c, r + 1)?
                                .checked_sub(base)
                                .ok_or_else(|| invalid("shape reentry underflow"))?;
                            if t >= c.len() {
                                return Err(invalid("shape reentry outside CODE"));
                            }
                            next = Some(t);
                            break;
                        }
                    }
                    p = next.ok_or_else(|| invalid("unsupported shape native reentry"))?;
                }
                0x82 => {
                    let count = u16_at(c, p + 2)?;
                    let dest = u16_at(c, p + 4)?;
                    if dest % 8 != 0 || count > 8192 {
                        return Err(invalid("invalid vertex slots"));
                    }
                    for i in 0..count {
                        let at = p + 6 + i * 6;
                        slots.insert(
                            dest / 8 + i,
                            std::array::from_fn(|j| {
                                transform[j] + word(c, at + j * 2).unwrap_or(0) as f32
                            }),
                        );
                    }
                    slice(c, p, 6 + count * 6)?;
                    p += 6 + count * 6;
                }
                0xf6 => {
                    colors.insert(u16_at(c, p + 1)?, slice(c, p + 3, 1)?[0]);
                    p += 7;
                }
                0xe0 => {
                    texture = String::new();
                    p += 4;
                }
                0xe2 => {
                    texture = String::from_utf8_lossy(
                        slice(c, p + 2, 14)?.split(|b| *b == 0).next().unwrap(),
                    )
                    .to_ascii_uppercase();
                    p += 16;
                }
                0xfc => {
                    let addr = p;
                    let h = slice(c, p, 5)?;
                    let sub = h[1];
                    let flags = h[2];
                    let color = u16_at(c, p + 3)? as u8;
                    p += 5;
                    let normal = if sub & 0x40 != 0 {
                        let n = [
                            word(c, p)? as f32,
                            word(c, p + 2)? as f32,
                            word(c, p + 4)? as f32,
                        ];
                        p += 6 + if flags & 2 != 0 { 3 } else { 6 };
                        Some(n)
                    } else {
                        None
                    };
                    let count = slice(c, p, 1)?[0] as usize;
                    p += 1;
                    if !(3..=64).contains(&count) {
                        return Err(invalid("invalid polygon count"));
                    }
                    let mut positions = Vec::new();
                    let mut cs = Vec::new();
                    for _ in 0..count {
                        let i = if flags & 4 != 0 {
                            let i = u16_at(c, p)?;
                            p += 2;
                            i
                        } else {
                            let i = slice(c, p, 1)?[0] as usize;
                            p += 1;
                            i
                        };
                        positions.push(
                            *slots
                                .get(&i)
                                .ok_or_else(|| invalid("unresolved shape vertex"))?,
                        );
                        cs.push(if sub == 0xee {
                            *colors.get(&i).unwrap_or(&color)
                        } else {
                            color
                        });
                    }
                    let mut uv = Vec::new();
                    if sub & 4 != 0 {
                        for _ in 0..count {
                            if flags & 1 != 0 {
                                let b = slice(c, p, 2)?;
                                uv.push([b[0] as f32, b[1] as f32]);
                                p += 2;
                            } else {
                                uv.push([u16_at(c, p)? as f32, u16_at(c, p + 2)? as f32]);
                                p += 4;
                            }
                        }
                    }
                    if seen.insert((addr, transform.map(f32::to_bits)))
                        && !(sub & 4 != 0 && texture.is_empty())
                    {
                        faces.push(Face {
                            positions,
                            colors: cs,
                            uv,
                            texture: texture.clone(),
                            subtype: sub,
                            normal,
                            address: base + addr,
                        });
                    }
                }
                0x42 => {
                    let rest = slice(c, p + 2, c.len().saturating_sub(p + 2))?;
                    p += 3 + rest
                        .iter()
                        .position(|b| *b == 0)
                        .ok_or_else(|| invalid("unterminated source string"))?;
                }
                0x40 => p += 4 + 2 * u16_at(c, p + 2)?,
                0x44 => p += 8 + 2 * u16_at(c, p + 6)?,
                0xbc => {
                    p += match slice(c, p + 2, 1)?[0] {
                        0x72 | 0x08 => 6,
                        0x96 | 0x3a => 8,
                        0x68 => 10,
                        _ => return Err(invalid("unsupported shape primitive")),
                    }
                }
                0xff if slice(c, p, 2)? == [255, 255] => p += 14,
                _ => {
                    p += match op {
                        0x1e => 1,
                        0x46 | 0xb2 | 0x4e | 0xee => 2,
                        0xf2 | 0xb8 | 0x4d | 0xd0 | 0xca | 0xda | 0x05 | 0x14 | 0x18 | 0x4a
                        | 0x48 | 0xac => 4,
                        0xa6 => 6,
                        0x2e | 0x50 | 0x68 | 0xea | 0xc8 => 8,
                        0x7a | 0x0c | 0x0e | 0x10 | 0x66 | 0xe6 | 0x76 | 0x08 | 0x6c => 10,
                        0x78 => 12,
                        0x06 => 14,
                        0xe4 => 20,
                        0xce => 40,
                        _ => {
                            return Err(invalid(&format!(
                                "unsupported SH opcode {op:02x} at {:x}",
                                base + p
                            )));
                        }
                    }
                }
            }
        }
        if !finished || faces.is_empty() {
            return Err(invalid("shape instruction bound or no geometry"));
        }
        Ok(Self { faces, state_words })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn program() -> Vec<u8> {
        let mut c = vec![0x82, 0, 3, 0, 0, 0];
        for v in [0i16, 0, 0, 10, 0, 0, 0, 10, 0] {
            c.extend_from_slice(&v.to_le_bytes());
        }
        c.extend_from_slice(&[0xfc, 0, 0, 100, 0, 3, 0, 1, 2, 0]);
        c
    }
    #[test]
    fn resolve_shared_slots_and_reject_truncations() {
        let c = program();
        let s = Shape::parse(&module::fixture(&c)).unwrap();
        assert_eq!(s.faces.len(), 1);
        assert_eq!(s.faces[0].positions[2], [0., 10., 0.]);
        for n in 0..c.len() {
            assert!(Shape::parse(&module::fixture(&c[..n])).is_err());
        }
    }
    #[test]
    fn reject_unresolved_vertices_and_unknown_opcodes() {
        let mut c = program();
        let n = c.len();
        c[n - 2] = 7;
        assert!(Shape::parse(&module::fixture(&c)).is_err());
        assert!(Shape::parse(&module::fixture(&[0xab, 0])).is_err());
    }
}
