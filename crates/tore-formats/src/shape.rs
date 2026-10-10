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
/// SH `ea` sprite: a textured rectangle that always faces the viewer, as the
/// men (SOLDIER.SH) are drawn. Its centre is an ordinary vertex slot.
#[derive(Clone, Debug, PartialEq)]
pub struct Billboard {
    /// Centre in source units, in face-position order (right, forward, up).
    pub center: [f32; 3],
    /// Full width and height in source units; the centre is the middle.
    pub size: [f32; 2],
    pub texture: String,
    /// Texture corners from the last `e4` record: bottom left, top left,
    /// top right, bottom right, in PIC pixels with rows counted up from the
    /// bottom row, as face texture coordinates are.
    pub uv: Option<[[f32; 2]; 4]>,
    pub fog: FogMode,
    pub address: usize,
}
impl Billboard {
    /// The sprite as a four-corner face spanning `right` and `up`, unit
    /// vectors in face-position axes chosen by the viewer, in the same
    /// corner order as `uv`.
    pub fn face(&self, right: [f32; 3], up: [f32; 3]) -> Face {
        let [w, h] = self.size.map(|v| v * 0.5);
        let corner = |x: f32, y: f32| -> [f32; 3] {
            std::array::from_fn(|i| self.center[i] + right[i] * x * w + up[i] * y * h)
        };
        Face {
            positions: vec![
                corner(-1., -1.),
                corner(-1., 1.),
                corner(1., 1.),
                corner(1., -1.),
            ],
            colors: vec![0; 4],
            fog: self.fog,
            uv: self.uv.map(Vec::from).unwrap_or_default(),
            texture: self.texture.clone(),
            subtype: 0,
            normal: None,
            address: self.address,
        }
    }
}
pub struct Shape {
    pub lines: Vec<Line>,
    pub faces: Vec<Face>,
    /// Viewer-facing sprites. Shapes made only of sprites have no faces.
    pub billboards: Vec<Billboard>,
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

/// First synthetic state key of the HARDNumLoaded envelope. Launcher shapes
/// (CHAP, SA2, SA3, SCD) ask the host how many rounds a hardpoint still
/// carries and draw one rail's missile per count they pass. The key lies far
/// above any SH module address, so it never meets a real state word.
pub const LOADED_COUNT_BASE: usize = 0xffff_0000;

/// State key for the loaded count of `hardpoint` (the native call's index).
/// Absent from the state map means zero rounds loaded; scenery always draws
/// the full load.
pub const fn loaded_count_word(hardpoint: u8) -> usize {
    LOADED_COUNT_BASE + hardpoint as usize
}

/// `68 X 68 Y c3`: the push pair an f0 record uses to hand control to a
/// main-program thunk `Y` that returns to `X`. Returns `X` as a CODE offset.
fn trampoline(c: &[u8], base: usize, at: usize) -> Result<usize> {
    if slice(c, at, 1)? != [0x68]
        || slice(c, at + 5, 1)? != [0x68]
        || slice(c, at + 10, 1)? != [0xc3]
    {
        return Err(invalid("invalid shape reentry trampoline"));
    }
    let to = u32_at(c, at + 1)?
        .checked_sub(base)
        .ok_or_else(|| invalid("shape reentry underflow"))?;
    if to >= c.len() {
        return Err(invalid("shape reentry outside CODE"));
    }
    Ok(to)
}

/// Reviewed HARDNumLoaded prelude (SA3, SCD; CHAP and SA2 share it), after
/// the f0 marker: `mov ecx,[objId]; mov edx,hardpoint; or ecx,ecx; jz +13`,
/// then a trampoline into the main program that returns straight into the
/// `eb 05 b8 1` envelope behind it. Returns the hardpoint index and the
/// envelope's CODE offset; anything else is not this prelude.
fn loaded_count_prelude(c: &[u8], base: usize, start: usize) -> Option<(u8, usize)> {
    let b = c.get(start..start + 26)?;
    let hardpoint = u32::from_le_bytes(b[7..11].try_into().unwrap());
    let returns_to = u32::from_le_bytes(b[16..20].try_into().unwrap()) as usize;
    let envelope = start + 26;
    (b[..2] == [0x8b, 0x0d]
        && b[6] == 0xba
        && b[11..15] == [0x0b, 0xc9, 0x74, 0x0d]
        && b[15] == 0x68
        && b[20] == 0x68
        && b[25] == 0xc3
        && returns_to == base + envelope
        && hardpoint < 256)
        .then_some((hardpoint as u8, envelope))
}

/// The `eb 05 b8 01 00 00 00` envelope HARDNumLoaded returns into, followed
/// by `cmp eax,N; jb +17` (draw while at least N rounds remain) or
/// `or eax,eax; jz +17` (draw while any remain). Both arms land on a
/// trampoline: the drawing arm resumes at an SH call to the missile's
/// geometry, the skipping arm resumes inside the next f0 record. Returns the
/// CODE offset the chosen arm resumes at.
fn loaded_count_branch(c: &[u8], base: usize, at: usize, count: i32) -> Result<usize> {
    if slice(c, at, 7)? != [0xeb, 5, 0xb8, 1, 0, 0, 0] {
        return Err(invalid("unreviewed loaded-count envelope"));
    }
    let (draw, loaded) = if slice(c, at + 7, 2)? == [0x83, 0xf8]
        && (1..=4).contains(&slice(c, at + 9, 1)?[0])
        && slice(c, at + 10, 2)? == [0x72, 0x11]
    {
        (at + 12, count >= i32::from(c[at + 9]))
    } else if slice(c, at + 7, 4)? == [0x0b, 0xc0, 0x74, 0x11] {
        (at + 11, count != 0)
    } else {
        return Err(invalid("unreviewed loaded-count comparison"));
    };
    if loaded {
        let to = trampoline(c, base, draw)?;
        if slice(c, to, 2)? != [0x12, 0] {
            return Err(invalid(
                "loaded-count arm does not reference drawing records",
            ));
        }
        Ok(to)
    } else {
        // The 17-byte jump passes the drawing trampoline, the SH call and the
        // next record's f0 marker, landing on that record's own trampoline.
        let skip = draw + 0x11;
        if slice(c, skip - 2, 2)? != [0xf0, 0] {
            return Err(invalid("loaded-count skip does not land in an f0 record"));
        }
        trampoline(c, base, skip)
    }
}

/// Reviewed hardpoint-angle envelope (KRIV, SOVR), after the f0 marker:
/// `call $+5; pop ebx; add ebx,N; mov ecx,hardpoint`, a trampoline into the
/// main program's HardpointAngle that returns straight back, an optional
/// `add ax,imm16`, then `mov [ebx+6],ax` and the trampoline that resumes the
/// SH program. The write targets the heading word of the c4 transform the
/// program resumes at, so the turret under it turns. The added constant is
/// the hardpoint's mount heading (32760 in KRIVAK.NT and SOVR.NT for the aft
/// mounts, absent for the forward one), and HardpointAngle is taken as zero,
/// the mount's rest. Returns the resume offset and that rest heading in
/// binary angle units, or `None` when the bytes are not this form.
fn hardpoint_angle_envelope(c: &[u8], base: usize, start: usize) -> Result<Option<(usize, i16)>> {
    let Some(b) = c.get(start..start + 17) else {
        return Ok(None);
    };
    if b[..6] != [0xe8, 0, 0, 0, 0, 0x5b] || b[6..8] != [0x81, 0xc3] || b[12] != 0xb9 {
        return Ok(None);
    }
    let call = start + 17;
    if trampoline(c, base, call).ok() != Some(call + 11) {
        return Ok(None);
    }
    let mut at = call + 11;
    let mut rest = 0;
    if slice(c, at, 2)? == [0x66, 0x05] {
        rest = word(c, at + 2)? as i16;
        at += 4;
    }
    if slice(c, at, 4)? != [0x66, 0x89, 0x43, 0x06] {
        return Err(invalid("unreviewed hardpoint-angle write"));
    }
    let resume = trampoline(c, base, at + 4)?;
    // ebx is the address after the call plus N; the write lands at ebx+6.
    let written = start + 5 + u32_at(c, start + 8)? + 6;
    if slice(c, resume, 1)? != [0xc4] || written != resume + 8 {
        return Err(invalid(
            "hardpoint angle does not write the resumed c4 rotation",
        ));
    }
    Ok(Some((resume, rest)))
}

/// A c4 heading word in binary angle units (65536 a turn) as radians.
fn heading_radians(units: i16) -> f32 {
    f32::from(units) * std::f32::consts::TAU / 65536.
}

/// `local` placed under a transform: the translation plus the local point
/// turned by `heading` about the up axis, forward toward right. An unturned
/// transform keeps the plain sum the reader has always used.
fn place(transform: [f32; 3], heading: f32, local: [f32; 3]) -> [f32; 3] {
    if heading == 0. {
        return std::array::from_fn(|j| transform[j] + local[j]);
    }
    let (sin, cos) = heading.sin_cos();
    [
        transform[0] + local[0] * cos + local[1] * sin,
        transform[1] + local[1] * cos - local[0] * sin,
        transform[2] + local[2],
    ]
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
    /// Static scenery pose with loaded launchers and turrets at their mount
    /// rest. Interprets bounded drawing records and the reviewed loaded-count
    /// (CHAP, SA2, SA3, SCD) and hardpoint-angle (KRIV, SOVR) envelopes only.
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
        let mut billboards = Vec::new();
        let mut sprite_uv = None;
        let mut fog = FogMode::Enabled;
        let mut seen = BTreeSet::new();
        let mut state_words = BTreeSet::new();
        let (mut p, mut end, mut texture, mut transform) = (0, None, String::new(), [0.; 3]);
        // Heading of the current transform, and one a reviewed envelope gave
        // the c4 record it resumes at.
        let mut heading = 0f32;
        let mut pending_heading: Option<(usize, f32)> = None;
        type Frame = (usize, Option<usize>, Option<([f32; 3], String, f32)>);
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
                    if let Some((t, s, h)) = frame {
                        transform = t;
                        texture = s;
                        heading = h;
                    }
                    continue;
                }
                finished = true;
                break;
            }
            match op {
                0xeb if scenery => {
                    // CHAP/SA2/SA3/SCD: HARDNumLoaded returns through this
                    // envelope. The static host chooses its loaded visual
                    // branch. This is a data grammar, not a general x86
                    // interpreter.
                    p = loaded_count_branch(c, base, p, i32::MAX)?;
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
                    stack.push((p + 16, end, Some((transform, texture.clone(), heading))));
                    let offset = [word(c, p + 2)?, word(c, p + 6)?, word(c, p + 4)?];
                    transform = place(transform, heading, offset.map(|v| v as f32));
                    if let Some((at, turn)) = pending_heading.take()
                        && at == p
                    {
                        heading += turn;
                    }
                    end = None;
                    p = t;
                }
                0xf0 => {
                    if !scenery
                        && let Some((hardpoint, envelope)) = loaded_count_prelude(c, base, p + 2)
                    {
                        // The dynamic path draws the launcher's loaded rails from
                        // the caller's count instead of the scenery's full load.
                        let key = loaded_count_word(hardpoint);
                        state_words.insert(key);
                        let count = state.get(&key).copied().unwrap_or(0);
                        p = loaded_count_branch(c, base, envelope, count)?;
                        continue;
                    }
                    if let Some((resume, rest)) = hardpoint_angle_envelope(c, base, p + 2)? {
                        pending_heading = Some((resume, heading_radians(rest)));
                        p = resume;
                        continue;
                    }
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
                            // Y141.SH tests the signed flap word against zero.
                            // This selects inert geometry, never native execution.
                            0x7d => value >= imm,
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
                        let local =
                            std::array::from_fn(|j| word(c, at + j * 2).unwrap_or(0) as f32);
                        slots.insert(dest / 8 + i, place(transform, heading, local));
                    }
                    slice(c, p, 6 + count * 6)?;
                    p += 6 + count * 6;
                }
                0x7a => {
                    // One vertex: three signed words and its byte slot, the
                    // same slots `82` fills (FA dispatch, see weather/shape.rs).
                    let slot = u16_at(c, p + 8)?;
                    if slot % 8 == 0 {
                        let local =
                            std::array::from_fn(|j| word(c, p + 2 + j * 2).unwrap_or(0) as f32);
                        slots.insert(slot / 8, place(transform, heading, local));
                    }
                    p += 10;
                }
                0xe4 => {
                    // Sprite texture corners for the next `ea`.
                    sprite_uv = (u16_at(c, p + 2)? == 4).then(|| {
                        std::array::from_fn(|i| {
                            [
                                u16_at(c, p + 4 + i * 4).unwrap_or(0) as f32,
                                u16_at(c, p + 6 + i * 4).unwrap_or(0) as f32,
                            ]
                        })
                    });
                    slice(c, p, 20)?;
                    p += 20;
                }
                0xea => {
                    let center = u16_at(c, p + 2)?;
                    let size = [word(c, p + 4)? as f32, word(c, p + 6)? as f32];
                    if center % 8 != 0 || size.iter().any(|v| *v <= 0.) {
                        return Err(invalid("invalid shape billboard"));
                    }
                    let center = *slots
                        .get(&(center / 8))
                        .ok_or_else(|| invalid("unresolved shape billboard centre"))?;
                    if seen.insert((p, center.map(f32::to_bits), fog as u8, 0)) {
                        billboards.push(Billboard {
                            center,
                            size,
                            texture: texture.clone(),
                            uv: sprite_uv,
                            fog,
                            address: base + p,
                        });
                    }
                    p += 8;
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
                        // Directions turn with their part but do not move.
                        Some(place([0.; 3], heading, n))
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
                    if seen.insert((
                        addr,
                        transform.map(f32::to_bits),
                        fog as u8,
                        heading.to_bits(),
                    )) && !(sub & 4 != 0 && texture.is_empty())
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
                        0x2e | 0x50 | 0x68 | 0xc8 => 8,
                        0x0c | 0x0e | 0x10 | 0x66 | 0xe6 | 0x76 | 0x08 | 0x6c => 10,
                        0x78 => 12,
                        0x06 => 14,
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
        if !finished || (faces.is_empty() && billboards.is_empty()) {
            return Err(invalid("shape instruction bound or no geometry"));
        }
        Ok(Self {
            faces,
            lines,
            billboards,
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
    fn signed_flap_guard_selects_negative_and_nonnegative_geometry() {
        let mut code = vec![0xf0, 0, 0x66, 0x83, 0x3d, 0, 0x71, 0, 0, 0, 0x7d, 11];
        let mut pointers = Vec::new();
        for _ in 0..2 {
            pointers.push(code.len() + 1);
            code.extend([0x68, 0, 0, 0, 0, 0x68, 0, 0, 0, 0, 0xc3]);
        }
        for (index, pointer) in pointers.iter().enumerate() {
            let dest = 0x1000 + code.len() as u32;
            code[*pointer..*pointer + 4].copy_from_slice(&dest.to_le_bytes());
            let mut geometry = program();
            geometry[27] = 100 + index as u8;
            code.extend(geometry);
        }
        let data = module::fixture(&code);
        for (value, color) in [(-32768, 100), (-1, 100), (0, 101), (1, 101), (32767, 101)] {
            let shape = Shape::with_state(&data, &[(0x7100, value)].into()).unwrap();
            assert_eq!(shape.faces[0].colors, vec![color; 3]);
        }
        code[11] = 127;
        assert!(Shape::with_state(&module::fixture(&code), &[(0x7100, 0)].into()).is_err());
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

    fn push_trampoline(code: &mut Vec<u8>, to: usize) {
        code.push(0x68);
        code.extend((0x1000 + to as u32).to_le_bytes());
        code.extend([0x68, 0, 0, 0, 0, 0xc3]);
    }

    /// A launcher: the HARDNumLoaded prelude for `hardpoint`, the envelope
    /// drawing one missile (colour 101) while at least two rounds remain,
    /// then the launcher body (colour 100).
    fn launcher(hardpoint: u8) -> Vec<u8> {
        let mut c = vec![0xf0, 0, 0x8b, 0x0d, 0, 0, 0, 0, 0xba, hardpoint, 0, 0, 0];
        c.extend([0x0b, 0xc9, 0x74, 0x0d]);
        push_trampoline(&mut c, 28);
        c.extend([0xeb, 5, 0xb8, 1, 0, 0, 0, 0x83, 0xf8, 2, 0x72, 0x11]);
        push_trampoline(&mut c, 51);
        c.extend([0x12, 0, 47, 0, 0xf0, 0]);
        push_trampoline(&mut c, 68);
        assert_eq!(c.len(), 68);
        c.extend(program());
        let mut missile = program();
        missile[27] = 101;
        c.extend(missile);
        c
    }

    #[test]
    fn loaded_count_draws_one_missile_per_round_in_the_state_path() {
        let data = module::fixture(&launcher(3));
        let key = loaded_count_word(3);
        let colors =
            |shape: Shape| -> Vec<u8> { shape.faces.iter().map(|f| f.colors[0]).collect() };
        for (count, expected) in [
            (0, vec![100]),
            (1, vec![100]),
            (2, vec![101, 100]),
            (9, vec![101, 100]),
        ] {
            let shape = Shape::with_state(&data, &[(key, count)].into()).unwrap();
            assert!(shape.state_words.contains(&key));
            assert_eq!(colors(shape), expected, "count {count}");
        }
        // An absent count is an empty launcher; the scenery pose is fully loaded.
        assert_eq!(colors(Shape::parse(&data).unwrap()), vec![100]);
        let scenery = Shape::scenery(&data).unwrap();
        assert!(scenery.state_words.is_empty());
        assert_eq!(colors(scenery), vec![101, 100]);
    }

    #[test]
    fn loaded_count_rejects_unreviewed_landings() {
        let mut c = launcher(0);
        c[55] = 0x1e; // the skip no longer lands inside an f0 record
        assert!(Shape::parse(&module::fixture(&c)).is_err());
        let mut c = launcher(0);
        c[37] = 9; // compares against more rails than any reviewed launcher
        assert!(Shape::parse(&module::fixture(&c)).is_err());
        let mut c = launcher(0);
        c[18] = 0x30; // the call no longer returns into the envelope
        assert!(
            Shape::with_state(&module::fixture(&c), &[(loaded_count_word(0), 2)].into()).is_err()
        );
    }

    /// A turret: the HardpointAngle envelope, optionally adding `rest`, a c4
    /// record 16 units right and the turret geometry under it. `adjust`
    /// moves the native write away from the c4 heading word.
    fn turret(rest: Option<i16>, adjust: u32) -> Vec<u8> {
        let mut c = vec![0xf0, 0, 0xe8, 0, 0, 0, 0, 0x5b, 0x81, 0xc3];
        let resume: usize = if rest.is_some() { 49 } else { 45 };
        c.extend((resume as u32 + 8 - 13 + adjust).to_le_bytes());
        c.extend([0xb9, 1, 0, 0, 0]);
        push_trampoline(&mut c, 30);
        if let Some(rest) = rest {
            c.extend([0x66, 0x05]);
            c.extend(rest.to_le_bytes());
        }
        c.extend([0x66, 0x89, 0x43, 0x06]);
        push_trampoline(&mut c, resume);
        assert_eq!(c.len(), resume);
        c.extend([0xc4, 0, 16, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]);
        c.extend(program());
        c
    }

    #[test]
    fn hardpoint_angle_turns_the_turret_to_its_mount_rest() {
        let forward = Shape::parse(&module::fixture(&turret(None, 0))).unwrap();
        assert_eq!(
            forward.faces[0].positions,
            vec![[16., 0., 0.], [26., 0., 0.], [16., 10., 0.]]
        );
        // KRIVAK.NT and SOVR.NT give their aft mounts heading 32760.
        for shape in [
            Shape::parse(&module::fixture(&turret(Some(0x7ff8), 0))).unwrap(),
            Shape::scenery(&module::fixture(&turret(Some(0x7ff8), 0))).unwrap(),
        ] {
            let p = &shape.faces[0].positions;
            assert!((p[0][0] - 16.).abs() < 1e-4 && p[0][1].abs() < 1e-4);
            assert!((p[1][0] - 6.).abs() < 1e-3 && p[1][1].abs() < 0.02);
            assert!((p[2][0] - 16.).abs() < 0.02 && (p[2][1] + 10.).abs() < 1e-3);
            assert_eq!(p[2][2], 0.);
        }
        // The native write must land on the resumed c4 heading word.
        assert!(Shape::parse(&module::fixture(&turret(Some(0x7ff8), 2))).is_err());
        let mut c = turret(None, 0);
        c[31] = 0x88; // an unreviewed write
        assert!(Shape::parse(&module::fixture(&c)).is_err());
    }

    fn sprite() -> Vec<u8> {
        let mut c = vec![0x7a, 0, 0, 0, 0, 0, 6, 0, 0, 0, 0xe2, 0];
        c.extend(b"soldier.PIC\0\0\0");
        c.extend([0xe4, 0, 4, 0]);
        for word in [0u16, 150, 0, 199, 29, 199, 29, 150] {
            c.extend(word.to_le_bytes());
        }
        c.extend([0xea, 0, 0, 0, 7, 0, 12, 0, 0]);
        c
    }

    #[test]
    fn billboards_project_from_a_vertex_and_sprite_corners() {
        let shape = Shape::scenery(&module::fixture(&sprite())).unwrap();
        assert!(shape.faces.is_empty());
        let [b] = shape.billboards.as_slice() else {
            panic!("one sprite expected");
        };
        assert_eq!(b.center, [0., 0., 6.]);
        assert_eq!(b.size, [7., 12.]);
        assert_eq!(b.texture, "SOLDIER.PIC");
        assert_eq!(
            b.uv,
            Some([[0., 150.], [0., 199.], [29., 199.], [29., 150.]])
        );
        let face = b.face([1., 0., 0.], [0., 0., 1.]);
        assert_eq!(
            face.positions,
            vec![
                [-3.5, 0., 0.],
                [-3.5, 0., 12.],
                [3.5, 0., 12.],
                [3.5, 0., 0.]
            ]
        );
        assert_eq!(face.uv.len(), 4);
        assert_eq!(
            Shape::parse(&module::fixture(&sprite()))
                .unwrap()
                .billboards
                .len(),
            1
        );
    }

    #[test]
    fn billboards_reject_unresolved_centres_and_empty_sizes() {
        let at = sprite().len() - 9;
        let mut c = sprite();
        c[at + 2] = 8;
        assert!(Shape::parse(&module::fixture(&c)).is_err());
        let mut c = sprite();
        c[at + 4] = 0;
        assert!(Shape::parse(&module::fixture(&c)).is_err());
        let c = sprite();
        for end in 0..c.len() {
            assert!(
                Shape::parse(&module::fixture(&c[..end])).is_err(),
                "end={end}"
            );
        }
    }
}
