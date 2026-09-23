//! Bounded nearest-detail static SH projection, not a complete native shape VM.
use crate::{Result, invalid, module, slice, u16_at, u32_at};
use std::collections::{BTreeMap, BTreeSet};

/// Object-space scale selected by the reviewed SH CODE header exponent.
/// Static scene geometry uses this directly; aircraft add their separate fitted rig scale.
pub fn object_scale(data: &[u8]) -> Result<f64> {
    let (code, _) = module::code(data)?;
    let exponent = i32::from(u16_at(code, 6)? as i16);
    if !(0..=16).contains(&exponent) {
        return Err(invalid("shape object scale exponent outside bound"));
    }
    Ok(2f64.powi(exponent - 8))
}
/// SH opcode 0xca (FA 0x4d4288). Conditional fog is disabled when the
/// sampled weather layer carries flag 0x40; all other nonzero words enable it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum FogMode {
    #[default]
    Enabled = 0,
    Disabled = 1,
    Conditional = 2,
}
impl FogMode {
    fn from_word(value: usize) -> Self {
        match value {
            0 => Self::Disabled,
            2 => Self::Conditional,
            _ => Self::Enabled,
        }
    }
    pub fn enabled(self, weather_flags: u16) -> bool {
        self == Self::Enabled || (self == Self::Conditional && weather_flags & 0x40 == 0)
    }
}
#[derive(Clone, Debug)]
pub struct Face {
    pub positions: Vec<[f32; 3]>,
    pub colors: Vec<u8>,
    pub fog: FogMode,
    pub uv: Vec<[f32; 2]>,
    pub texture: String,
    pub subtype: u8,
    pub normal: Option<[f32; 3]>,
    pub address: usize,
}
#[derive(Clone, Debug)]
pub struct Line {
    pub positions: [[f32; 3]; 2],
    pub color: u8,
    pub fog: FogMode,
}
pub struct Shape {
    pub lines: Vec<Line>,
    pub faces: Vec<Face>,
    pub state_words: BTreeSet<usize>,
}
/// FA 0x42e0c0 resolves the type's shape at +0x0f, then the F2 link at
/// CODE+0x0e. The contact consumer reads signed word +8 of that record.
/// `None` means no F2 record (native zero-record fallback), not malformed data.
/// This reads only the offset field; it does not validate full collision geometry.
pub fn contact_offset(data: &[u8]) -> Result<Option<i16>> {
    let (code, _) = module::code(data)?;
    contact_offset_code(code)
}

fn contact_offset_code(code: &[u8]) -> Result<Option<i16>> {
    if u16_at(code, 0x0e)? != 0xf2 {
        return Ok(None);
    }
    let record = 0x12 + u16_at(code, 0x10)?;
    // Bound every byte through the consumed word, without inventing meanings
    // for the other fields or accepting truncated offsets as default zero.
    slice(code, record, 10)?;
    Ok(Some(u16_at(code, record + 8)? as i16))
}

/// Inert F2 subrecord consumed by FA COLGetBox (0x42e100).
/// Coordinate pairs retain source order; flags other than bit 7 are uninterpreted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContactBox {
    pub flags: u8,
    pub id: u8,
    pub pairs: [[i16; 2]; 3],
}

impl ContactBox {
    /// STRIPAddProc averages signed endpoints using SAR, including negative odds.
    pub fn midpoint(self) -> [i16; 3] {
        self.pairs
            .map(|[a, b]| ((i32::from(a) + i32::from(b)) >> 1) as i16)
    }
}

/// Complete bounded subrecord list, not collision geometry acceptance.
/// Native lookup returns the first matching ID. Preserve duplicates and order.
/// Unlike that early-return lookup, parsing validates the entire list terminator.
pub fn contact_boxes(data: &[u8]) -> Result<Option<Vec<ContactBox>>> {
    let (code, _) = module::code(data)?;
    contact_boxes_code(code)
}

fn contact_boxes_code(code: &[u8]) -> Result<Option<Vec<ContactBox>>> {
    if u16_at(code, 0x0e)? != 0xf2 {
        return Ok(None);
    }
    let record = 0x12 + u16_at(code, 0x10)?;
    slice(code, record, 16)?;
    let mut at = record + 16;
    let mut boxes = Vec::new();
    loop {
        let flags = slice(code, at, 1)?[0];
        if flags & 0x80 == 0 {
            return Ok(Some(boxes));
        }
        // Host input bound, not a recovered native list capacity.
        if boxes.len() == 4096 {
            return Err(invalid("shape contact box count exceeds bound"));
        }
        let bytes = slice(code, at, 14)?;
        boxes.push(ContactBox {
            flags,
            id: bytes[1],
            pairs: std::array::from_fn(|axis| {
                std::array::from_fn(|side| {
                    let p = 2 + axis * 4 + side * 2;
                    i16::from_le_bytes([bytes[p], bytes[p + 1]])
                })
            }),
        });
        at += 14;
    }
}
/// Wing vapor attachment, from shape opcode 0xce. `?FindStreamerDef@@` at
/// 0x49fd70 looks at shape offset 0x0e, skips an optional 0xf2 collision record
/// and its four bytes, then requires the 0xce opcode word.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreamerDef {
    /// Hinge pivot in right/up/forward 24.8 source units. All-zero disables the hinge (0x4a0181).
    pub pivot: [i32; 3],
    /// Scale applied to the aircraft's swing-wing position before the hinge.
    pub hinge_scale: i16,
    /// Side 0 then side 1, in right/up/forward 24.8 source units. Side 0's X is mirrored.
    pub points: [[i32; 3]; 2],
}

impl StreamerDef {
    pub fn parse(data: &[u8]) -> Result<Option<Self>> {
        let (c, _) = module::code(data)?;
        let mut at = 0x0e;
        if u16_at(c, at)? == 0xf2 {
            at += 4;
        }
        if u16_at(c, at)? != 0xce {
            return Ok(None);
        }
        at += 2;
        let long = |i: usize| -> Result<i32> {
            Ok(i32::from_le_bytes(slice(c, at + i, 4)?.try_into().unwrap()))
        };
        Ok(Some(Self {
            pivot: [long(0)?, long(4)?, long(8)?],
            hinge_scale: word(c, at + 0x0c)? as i16,
            points: [
                [
                    long(0x0e)?
                        .checked_neg()
                        .ok_or_else(|| invalid("streamer coordinate cannot be mirrored"))?,
                    long(0x12)?,
                    long(0x16)?,
                ],
                [long(0x1a)?, long(0x1e)?, long(0x22)?],
            ],
        }))
    }

    /// The attachment point in source units for one side, before the object's
    /// own rotation. `0x4a0110` mirrors side 0 back across X after the hinge.
    pub fn attachment(&self, side: usize, swing_wing: i16) -> Result<[f64; 3]> {
        let mut point = self
            .points
            .get(side)
            .map(|p| p.map(f64::from))
            .ok_or_else(|| invalid("streamer side outside definition"))?;
        if self.pivot != [0; 3] {
            // 0x4a0196: the hinge angle is 182 * sweep * scale / 32767 in binary
            // angle units, which is degrees scaled by 65536 over 360.
            let units = f64::from(i32::from(swing_wing) * i32::from(self.hinge_scale));
            let radians = 182. * units / 32767. * std::f64::consts::TAU / 65536.;
            let (s, c) = radians.sin_cos();
            point = [
                point[0] * c + point[2] * s,
                point[1],
                point[2] * c - point[0] * s,
            ];
            for (value, pivot) in point.iter_mut().zip(self.pivot) {
                *value += f64::from(pivot);
            }
        }
        if side == 0 {
            point[0] = -point[0];
        }
        // Source geometry is 24.8 fixed point.
        Ok(point.map(|v| v / 256.))
    }
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
        Self::project(data, state, false, false)
    }
    /// Export validation follows explicit SH jumps; the gameplay projection stays unchanged.
    /// This interprets only bounded data records and reviewed state guard patterns, never x86.
    pub fn with_export_state(data: &[u8], state: &BTreeMap<usize, i32>) -> Result<Self> {
        Self::project(data, state, true, false)
    }
    /// Static scenery pose with loaded launchers. Interprets bounded drawing
    /// records and the reviewed CHAP/SA2 visual-selection envelope only.
    /// No imported callback runs and no autonomous behavior is implied.
    pub fn scenery(data: &[u8]) -> Result<Self> {
        Self::project(data, &BTreeMap::new(), true, true)
    }
    fn project(
        data: &[u8],
        state: &BTreeMap<usize, i32>,
        export: bool,
        scenery: bool,
    ) -> Result<Self> {
        let (c, base) = module::code(data)?;
        let mut slots = BTreeMap::<usize, [f32; 3]>::new();
        let mut colors = BTreeMap::new();
        let mut faces = Vec::new();
        let mut lines = Vec::new();
        let mut fog = FogMode::Enabled;
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
                0xeb if scenery => {
                    // CHAP/SA2: HARDNumLoaded returns through this envelope.
                    // The static host chooses its loaded visual branch. This is
                    // a data grammar, not a general x86 interpreter.
                    if slice(c, p, 7)? != [0xeb, 5, 0xb8, 1, 0, 0, 0] {
                        return Err(invalid("unreviewed scenery selection envelope"));
                    }
                    let draw = if slice(c, p + 7, 2)? == [0x83, 0xf8]
                        && (1..=4).contains(&slice(c, p + 9, 1)?[0])
                        && slice(c, p + 10, 2)? == [0x72, 0x11]
                    {
                        p + 12
                    } else if slice(c, p + 7, 4)? == [0x0b, 0xc0, 0x74, 0x11] {
                        p + 11
                    } else {
                        return Err(invalid("unreviewed scenery load comparison"));
                    };
                    if slice(c, draw, 1)? != [0x68]
                        || slice(c, draw + 5, 1)? != [0x68]
                        || slice(c, draw + 10, 1)? != [0xc3]
                    {
                        return Err(invalid("invalid scenery drawing reference"));
                    }
                    p = u32_at(c, draw + 1)?
                        .checked_sub(base)
                        .ok_or_else(|| invalid("scenery drawing reference underflow"))?;
                    if slice(c, p, 2)? != [0x12, 0] {
                        return Err(invalid(
                            "scenery selection does not reference drawing records",
                        ));
                    }
                }
                0x48 if export => p = target(p + 4, word(c, p + 2)?, c)?,
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
                    // EJECT.SH: inert presentation flag followed by the usual state guard.
                    // Do not write the flag or execute any original instruction.
                    if slice(c, start, 2)? == [0x83, 0x0d]
                        && slice(c, start + 6, 1)? == [2]
                        && slice(c, start + 7, 3)? == [0x66, 0x83, 0x3d]
                    {
                        start += 7;
                    }
                    for _ in 0..16 {
                        if slice(c, start, 3)? != [0x66, 0x83, 0x3d] {
                            break;
                        }
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
                        } else {
                            start += 10;
                            break;
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
                0xca => {
                    fog = FogMode::from_word(u16_at(c, p + 2)?);
                    p += 4;
                }
                0xf6 => {
                    colors.insert(u16_at(c, p + 1)?, slice(c, p + 3, 1)?[0]);
                    p += 7;
                }
                0xe0 => {
                    // Export checks retain decals even though their runtime material is unknown.
                    texture = if export && !scenery {
                        format!("@indexed:{}", u16_at(c, p + 2)?)
                    } else {
                        String::new()
                    };
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
                    let normal = if sub & 0x60 != 0 {
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
                    if seen.insert((addr, transform.map(f32::to_bits), fog as u8))
                        && !(sub & 4 != 0 && texture.is_empty())
                    {
                        faces.push(Face {
                            positions,
                            colors: cs,
                            fog,
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
                    // Inferred bounded line grammar for the explicit branch projection.
                    // Preserve the established gameplay projection for other shapes.
                    if export && slice(c, p + 2, 2)? == [0x96, 0] {
                        let mut positions = [[0.; 3]; 2];
                        for (i, point) in positions.iter_mut().enumerate() {
                            let slot = u16_at(c, p + 4 + i * 2)?;
                            if slot % 8 != 0 {
                                return Err(invalid("unaligned shape line slot"));
                            }
                            *point = *slots
                                .get(&(slot / 8))
                                .ok_or_else(|| invalid("missing shape line slot"))?;
                        }
                        lines.push(Line {
                            positions,
                            color: slice(c, p + 1, 1)?[0],
                            fog,
                        });
                    }
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
                        0xf2 | 0xb8 | 0x4d | 0xd0 | 0xda | 0x05 | 0x14 | 0x18 | 0x4a | 0x48
                        | 0xac => 4,
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
        Ok(Self {
            faces,
            lines,
            state_words,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contact_offset_link_is_unsigned_relative_and_bounded() {
        assert!(contact_offset_code(&[0; 15]).is_err());
        assert_eq!(contact_offset_code(&[0; 16]).unwrap(), None);
        let mut code = vec![0; 40];
        code[14..16].copy_from_slice(&0xf2u16.to_le_bytes());
        code[16..18].copy_from_slice(&12u16.to_le_bytes());
        code[38..40].copy_from_slice(&(-7i16).to_le_bytes());
        assert_eq!(contact_offset_code(&code).unwrap(), Some(-7));
        assert_eq!(contact_offset(&module::fixture(&code)).unwrap(), Some(-7));
        assert!(contact_offset_code(&code[..39]).is_err());
        code[16..18].copy_from_slice(&0xffffu16.to_le_bytes());
        assert!(contact_offset_code(&code).is_err());
    }

    #[test]
    fn contact_box_list_preserves_order_flags_signed_pairs_and_terminator() {
        let mut code = vec![0; 34];
        code[14] = 0xf2;
        for (flags, id, pairs) in [
            (0xc0, 37, [-4_i16, 1, i16::MIN, i16::MAX, 9, 9]),
            (0x81, 37, [10, 12, 20, 22, 30, 32]),
        ] {
            code.extend([flags, id]);
            for word in pairs {
                code.extend(word.to_le_bytes());
            }
        }
        code.push(0x7f); // Any clear bit 7 terminates; other bits are not guessed.
        let boxes = contact_boxes(&module::fixture(&code)).unwrap().unwrap();
        assert_eq!(boxes.len(), 2);
        assert_eq!(boxes[0].flags, 0xc0);
        assert_eq!(boxes[1].id, 37);
        assert_eq!(boxes[0].midpoint(), [-2, -1, 9]);
        assert_eq!(boxes[1].midpoint(), [11, 21, 31]);
        for end in 16..code.len() {
            assert!(contact_boxes_code(&code[..end]).is_err(), "end={end}");
        }
        assert_eq!(contact_boxes_code(&[0; 16]).unwrap(), None);
        code.truncate(34);
        code.push(0);
        assert_eq!(contact_boxes_code(&code).unwrap(), Some(vec![]));
        code[16..18].copy_from_slice(&u16::MAX.to_le_bytes());
        assert!(contact_boxes_code(&code).is_err());
    }

    #[test]
    fn contact_box_host_count_limit_requires_terminator() {
        let mut code = vec![0; 34];
        code[14] = 0xf2;
        for _ in 0..4096 {
            code.push(0x80);
            code.extend([0; 13]);
        }
        code.push(0);
        assert_eq!(contact_boxes_code(&code).unwrap().unwrap().len(), 4096);
        *code.last_mut().unwrap() = 0x80;
        code.extend([0; 14]);
        assert!(contact_boxes_code(&code).is_err());
    }
    #[test]
    fn ce_heading_hinge_preserves_up_coordinate() {
        let d = StreamerDef {
            pivot: [0, 256, 0],
            hinge_scale: 90,
            points: [[2560, 512, 0]; 2],
        };
        let point = d.attachment(1, 32767).unwrap();
        assert!((point[0]).abs() < 0.02);
        assert_eq!(point[1], 3.);
        assert!((point[2] + 10.).abs() < 0.02);
    }

    #[test]
    fn fog_opcode_survives_static_shape_projection() {
        for (word, mode) in [
            (0, FogMode::Disabled),
            (1, FogMode::Enabled),
            (2, FogMode::Conditional),
            (65535, FogMode::Enabled),
        ] {
            let mut code = vec![0xca, 0];
            code.extend((word as u16).to_le_bytes());
            code.extend(program());
            let shape = Shape::parse(&module::fixture(&code)).unwrap();
            assert!(shape.faces.iter().all(|f| f.fog == mode));
        }
        assert!(FogMode::Conditional.enabled(0));
        assert!(!FogMode::Conditional.enabled(0x40));
        assert!(FogMode::Enabled.enabled(0x40));
        assert!(!FogMode::Disabled.enabled(0));
    }
    #[test]
    fn streamer_mirroring_rejects_unrepresentable_coordinates() {
        let mut code = vec![0; 0x0e + 40];
        code[0x0e..0x10].copy_from_slice(&0xceu16.to_le_bytes());
        code[0x1e..0x22].copy_from_slice(&i32::MIN.to_le_bytes());
        assert!(StreamerDef::parse(&module::fixture(&code)).is_err());
        code[0x1e..0x22].copy_from_slice(&256i32.to_le_bytes());
        let def = StreamerDef::parse(&module::fixture(&code))
            .unwrap()
            .unwrap();
        assert_eq!(def.attachment(0, 0).unwrap(), [1., 0., 0.]);
    }
    fn program() -> Vec<u8> {
        let mut c = vec![0x82, 0, 3, 0, 0, 0];
        for v in [0i16, 0, 0, 10, 0, 0, 0, 10, 0] {
            c.extend_from_slice(&v.to_le_bytes());
        }
        c.extend_from_slice(&[0xfc, 0, 0, 100, 0, 3, 0, 1, 2, 0]);
        c
    }
    #[test]
    fn scenery_loaded_pose_accepts_only_reviewed_drawing_envelopes() {
        let mut code = vec![0xeb, 5, 0xb8, 1, 0, 0, 0, 0x83, 0xf8, 2, 0x72, 0x11, 0x68];
        code.extend(0x1017u32.to_le_bytes());
        code.extend([0x68, 0, 0, 0, 0, 0xc3, 0x12, 0, 1, 0, 0]);
        code.extend(program());
        let bytes = module::fixture(&code);
        assert_eq!(Shape::scenery(&bytes).unwrap().faces.len(), 1);
        assert!(
            Shape::parse(&bytes).is_err(),
            "aircraft projection stays separate"
        );
        code[10] = 0x75;
        assert!(Shape::scenery(&module::fixture(&code)).is_err());
        code[10] = 0x72;
        code[13..17].copy_from_slice(&0xffffu32.to_le_bytes());
        assert!(Shape::scenery(&module::fixture(&code)).is_err());
    }
    #[test]
    fn ejection_lines_resolve_bounded_vertex_slots() {
        let mut code = program();
        code.pop();
        let at = code.len();
        code.extend([0xbc, 155, 0x96, 0, 0, 0, 8, 0, 0]);
        let shape = Shape::with_export_state(&module::fixture(&code), &BTreeMap::new()).unwrap();
        assert_eq!(shape.lines.len(), 1);
        assert_eq!(shape.lines[0].positions, [[0., 0., 0.], [10., 0., 0.]]);
        assert_eq!(shape.lines[0].color, 155);
        code[at + 6] = 7;
        assert!(Shape::with_export_state(&module::fixture(&code), &BTreeMap::new()).is_err());
        code[at + 6] = 248;
        assert!(Shape::with_export_state(&module::fixture(&code), &BTreeMap::new()).is_err());
    }
    #[test]
    fn ejection_guard_chain_selects_geometry_without_running_side_effects() {
        let mut code = vec![0xf0, 0, 0x83, 0x0d, 0, 0x70, 0, 0, 2];
        let mut guards = Vec::new();
        for state in [34u8, 35] {
            code.extend([0x66, 0x83, 0x3d, 0, 0x71, 0, 0, state, 0x75, 11]);
            guards.push(code.len() + 1);
            code.extend([0x68, 0, 0, 0, 0, 0x68, 0, 0, 0, 0, 0xc3]);
        }
        guards.push(code.len() + 1);
        code.extend([0x68, 0, 0, 0, 0, 0x68, 0, 0, 0, 0, 0xc3]);
        for (index, pointer) in guards.iter().enumerate() {
            let dest = 0x1000 + code.len() as u32;
            code[*pointer..*pointer + 4].copy_from_slice(&dest.to_le_bytes());
            let mut geometry = program();
            geometry[27] = 100 + index as u8;
            code.extend(geometry);
        }
        let data = module::fixture(&code);
        for (state, color) in [(34, 100), (35, 101), (38, 102)] {
            let shape =
                Shape::with_export_state(&data, &BTreeMap::from([(0x7100, state)])).unwrap();
            assert_eq!(shape.faces.len(), 1);
            assert_eq!(shape.faces[0].colors, vec![color; 3]);
        }
    }
    #[test]
    fn export_keeps_indexed_decals_that_gameplay_projection_omits() {
        let mut code = program();
        code.pop();
        code.extend([0xe0, 0, 1, 0]);
        code.extend([0xfc, 4, 1, 0, 0, 3, 0, 1, 2, 0, 0, 10, 0, 0, 10, 0]);
        let data = module::fixture(&code);
        assert_eq!(Shape::parse(&data).unwrap().faces.len(), 1);
        let exported = Shape::with_export_state(&data, &BTreeMap::new()).unwrap();
        assert_eq!(exported.faces.len(), 2);
        assert_eq!(exported.faces[1].texture, "@indexed:1");
        assert_eq!(exported.faces[1].uv, vec![[0., 0.], [10., 0.], [0., 10.]]);
    }

    #[test]
    fn export_jumps_skip_dead_bytes_and_bound_cycles() {
        let mut code = vec![0x48, 0, 2, 0, 0xfe, 0xfe];
        code.extend(program());
        let shape = Shape::with_export_state(&module::fixture(&code), &BTreeMap::new()).unwrap();
        assert_eq!(shape.faces.len(), 1);
        assert!(Shape::parse(&module::fixture(&code)).is_err());
        let loop_code = [0x48, 0, 0xfc, 0xff];
        assert!(Shape::with_export_state(&module::fixture(&loop_code), &BTreeMap::new()).is_err());
        let outside = [0x48, 0, 0xff, 0x7f];
        assert!(Shape::with_export_state(&module::fixture(&outside), &BTreeMap::new()).is_err());
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
