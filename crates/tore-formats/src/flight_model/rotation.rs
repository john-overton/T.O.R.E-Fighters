//! Static translations of FA angle/trigonometry and body-rate helpers.
//! The sine table is imported data, never generated or embedded retail bytes.
use super::{divide, mul_div};
use crate::{Result, invalid};
#[derive(Clone, Debug)]
pub struct TrigTable([i16; 321]);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SinCos {
    pub sin: i16,
    pub cos: i16,
}
impl TrigTable {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 642 {
            return Err(invalid("native sine table must be 321 little-endian words"));
        }
        let mut values = [0; 321];
        for (value, pair) in values.iter_mut().zip(bytes.chunks_exact(2)) {
            *value = i16::from_le_bytes([pair[0], pair[1]]);
        }
        Ok(Self(values))
    }
    /// FA 0x4cd588: high byte selects sample, low byte interpolates with SAR.
    pub fn sin_cos(&self, angle_pa: i16) -> SinCos {
        let angle = angle_pa as u16 as usize;
        let index = angle >> 8;
        let fraction = (angle & 255) as i32;
        let sample = |i: usize| {
            (self.0[i] as i32 + (((self.0[i + 1] as i32 - self.0[i] as i32) * fraction) >> 8))
                as i16
        };
        SinCos {
            sin: sample(index),
            cos: sample(index + 64),
        }
    }
}
/// FA 0x4c6620: the initial multiply WRAPS before sign extension and +703.
pub fn degrees_to_pa(degrees_f8: i32) -> Result<i16> {
    divide(degrees_f8.wrapping_mul(1000) as i64 + 703, 1406).map(|x| x as i16)
}
/// FA 0x4c6638: signed PA to degrees fixed8, with asymmetric +500 rounding.
pub fn pa_to_degrees(angle_pa: i16) -> Result<i32> {
    divide(angle_pa as i64 * 1406 + 500, 1000)
}
fn multiply_f16(a: i32, b: i32) -> i32 {
    ((a as i64 * b as i64) >> 16) as i32
}
fn divide_f16(a: i32, b: i32) -> Result<i32> {
    mul_div(a, 65536, b)
}
/// FA 0x477010. Pitch is clamped ONLY for the rate transform (about +/-80 degrees).
/// Movement integration itself still crosses vertical attitudes.
pub fn body_rates(
    table: &TrigTable,
    rates: [i32; 3],
    roll_pa: i16,
    pitch_pa: i16,
) -> Result<[i32; 3]> {
    let pitch = table.sin_cos(pitch_pa.clamp(-14560, 14560));
    let roll = table.sin_cos(roll_pa);
    let (sp, cp, sr, cr) = (
        pitch.sin as i32 * 2,
        pitch.cos as i32 * 2,
        roll.sin as i32 * 2,
        roll.cos as i32 * 2,
    );
    let yaw_part = divide_f16(multiply_f16(rates[2].wrapping_shl(8), cr), cp)?;
    let pitch_part = divide_f16(multiply_f16(rates[1].wrapping_shl(8), sr), cp)?;
    let heading = (yaw_part >> 8).wrapping_add(pitch_part >> 8);
    let bank = (multiply_f16(yaw_part, sp) >> 8)
        .wrapping_add(multiply_f16(pitch_part, sp) >> 8)
        .wrapping_add(rates[0]);
    let pitch = (multiply_f16(rates[1].wrapping_shl(8), cr) >> 8)
        .wrapping_sub(multiply_f16(rates[2].wrapping_shl(8), sr) >> 8);
    Ok([bank, pitch, heading])
}
/// FA 0x4c6654 (Rotate2): rotate X/Z with independent signed wide products >>15.
pub fn rotate_xz(v: [i32; 3], trig: SinCos) -> [i32; 3] {
    let product = |value: i32, scale: i16| ((value as i64 * scale as i64) >> 15) as i32;
    [
        product(v[2], trig.sin).wrapping_add(product(v[0], trig.cos)),
        v[1],
        product(v[2], trig.cos).wrapping_sub(product(v[0], trig.sin)),
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_table_interpolation_and_signed_angle_rounding() {
        assert!(TrigTable::parse(&[0; 641]).is_err());
        assert!(TrigTable::parse(&[0; 643]).is_err());
        let mut values = [0i16; 321];
        values[0] = 100;
        values[1] = 99;
        values[64] = 200;
        values[65] = 204;
        let bytes: Vec<_> = values.into_iter().flat_map(i16::to_le_bytes).collect();
        let t = TrigTable::parse(&bytes).unwrap();
        assert_eq!(t.sin_cos(128), SinCos { sin: 99, cos: 202 });
        assert_eq!(t.sin_cos(-1), SinCos { sin: 0, cos: 0 });
        assert_eq!(degrees_to_pa(90 * 256).unwrap(), 16387);
        assert_eq!(degrees_to_pa(-90 * 256).unwrap(), -16386);
        assert_eq!(pa_to_degrees(-1).unwrap(), 0);
    }
    #[test]
    fn transform_order_and_faults_without_retail_fixtures() {
        let mut values = [0i16; 321];
        values[64] = 32767;
        let t = TrigTable(values);
        assert_eq!(
            body_rates(&t, [256, 512, 768], 0, 0).unwrap(),
            [256, 511, 768]
        );
        assert!(body_rates(&TrigTable([0; 321]), [0; 3], 0, 0).is_err());
        assert_eq!(
            rotate_xz([100, 200, 300], SinCos { sin: 32767, cos: 0 }),
            [299, 200, -99]
        );
        assert_eq!(
            rotate_xz([-1, 0, 0], SinCos { sin: 0, cos: 32767 }),
            [-1, 0, 0]
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Matrix(pub [[i16; 3]; 3]);
impl Matrix {
    pub const IDENTITY: Self = Self([[32767, 0, 0], [0, 32767, 0], [0, 0, 32767]]);
    /// FA 0x4cf2d0/0x4cf410: row/column dots, signed32 wrap then Q15 saturation.
    pub fn compose(self, rhs: Self) -> Self {
        let mut out = [[0; 3]; 3];
        for (r, row) in out.iter_mut().enumerate() {
            for (c, value) in row.iter_mut().enumerate() {
                let sum = (0..3).fold(0i32, |sum, k| {
                    sum.wrapping_add(self.0[r][k] as i32 * rhs.0[k][c] as i32)
                });
                *value = (sum >> 15).clamp(-32767, 32767) as i16;
            }
        }
        Self(out)
    }
    /// FA 0x4d5e58: zero-angle branches skip multiplies, preserving their rounding.
    pub fn from_angles(t: &TrigTable, heading: i16, pitch: i16, roll: i16) -> Self {
        let mut m = Self::IDENTITY;
        if roll != 0 {
            let v = t.sin_cos(roll.wrapping_neg());
            m = Self([
                [v.cos, v.sin.wrapping_neg(), 0],
                [v.sin, v.cos, 0],
                [0, 0, 32767],
            ]);
        }
        if pitch != 0 {
            let v = t.sin_cos(pitch);
            m = m.compose(Self([
                [32767, 0, 0],
                [0, v.cos, v.sin.wrapping_neg()],
                [0, v.sin, v.cos],
            ]));
        }
        if heading != 0 {
            let v = t.sin_cos(heading.wrapping_neg());
            m = m.compose(Self([
                [v.cos, 0, v.sin],
                [0, 32767, 0],
                [v.sin.wrapping_neg(), 0, v.cos],
            ]));
        }
        m
    }
    /// FA 0x4d64d8 arithmetic on representable outputs. Overflow policy is an error:
    /// native multi-bit SHLD overflow-flag behavior is not portable/proven.
    pub fn transform(self, v: [i32; 3]) -> Result<[i32; 3]> {
        let mut out = [0; 3];
        for (c, value) in out.iter_mut().enumerate() {
            let sum = (0..3)
                .map(|r| v[r] as i64 * self.0[r][c] as i64)
                .sum::<i64>();
            *value =
                i32::try_from(sum >> 15).map_err(|_| invalid("native matrix output overflow"))?;
            if *value == i32::MIN {
                *value = i32::MIN + 1;
            }
        }
        Ok(out)
    }
    /// FA 0x4d631c: signed-word vector result saturates to +/-32767.
    pub fn transform_word(self, v: [i16; 3]) -> [i16; 3] {
        let mut out = [0; 3];
        for (c, value) in out.iter_mut().enumerate() {
            let sum = (0..3).fold(0i32, |s, r| {
                s.wrapping_add(v[r] as i32 * self.0[r][c] as i32)
            });
            *value = (sum >> 15).clamp(-32767, 32767) as i16;
        }
        out
    }
}
/// FA 0x476fb0: native scalar components are reordered side/-down/forward.
pub fn world_velocity(
    t: &TrigTable,
    velocity: super::integration::Velocity,
    angles: super::integration::MovementAngles,
    effective_pitch_f8: i32,
) -> Result<[i32; 3]> {
    Matrix::from_angles(
        t,
        degrees_to_pa(angles.heading)?,
        degrees_to_pa(effective_pitch_f8)?,
        degrees_to_pa(angles.roll.wrapping_neg())?,
    )
    .transform([
        velocity.side,
        velocity.down.wrapping_neg(),
        velocity.forward,
    ])
}
#[derive(Clone, Debug)]
pub struct AtanTable([u16; 514]);
impl AtanTable {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 1028 {
            return Err(invalid("native atan table must be 514 words"));
        }
        let mut t = [0; 514];
        for (v, b) in t.iter_mut().zip(bytes.chunks_exact(2)) {
            *v = u16::from_le_bytes([b[0], b[1]]);
        }
        Ok(Self(t))
    }
    /// FA 0x4ccb88. Restricted to the reviewed signed-word vector callers.
    pub fn angle(&self, x: i16, z: i16) -> i16 {
        let mut a = (x as i32).unsigned_abs();
        let mut b = (z as i32).unsigned_abs();
        let swapped = b < a;
        if swapped {
            std::mem::swap(&mut a, &mut b);
        }
        let mut value = if b == 0 {
            0
        } else {
            let ratio = a.wrapping_shl(17) / b;
            let i = (ratio >> 8) as usize;
            let lo = self.0[i] as u32;
            let hi = self.0[i + 1] as u32;
            lo.wrapping_add(hi.wrapping_sub(lo).wrapping_mul(ratio & 255) >> 8)
        };
        if swapped {
            value = 0x3ffcu32.wrapping_sub(value);
        }
        value = match (x < 0, z < 0) {
            (false, false) => value,
            (true, false) => 0xffefu32.wrapping_sub(value),
            (false, true) => 0x7ff8u32.wrapping_sub(value),
            (true, true) => value.wrapping_add(0x7ff8),
        };
        value as i16
    }
}
/// FA 0x417f00: compose heading/pitch offsets through two 20,000-unit basis vectors.
/// Angles are [heading,pitch,roll] in native PA, not degrees.
pub fn cockpit_offset(
    t: &TrigTable,
    atan: &AtanTable,
    body: [i16; 3],
    yaw: i16,
    pitch: i16,
) -> [i16; 3] {
    if yaw == 0 && pitch == 0 {
        return body;
    }
    let offset = Matrix::from_angles(t, yaw, pitch, 0);
    let base = Matrix::from_angles(t, body[0], body[1], body[2]);
    let mut forward = base.transform_word(offset.transform_word([0, 0, 20000]));
    let mut right = base.transform_word(offset.transform_word([20000, 0, 0]));
    let heading = atan.angle(forward[0], forward[2]);
    let unheading = Matrix::from_angles(t, heading.wrapping_neg(), 0, 0);
    forward = unheading.transform_word(forward);
    let pitch = atan.angle(forward[1], forward[2]);
    right = unheading.transform_word(right);
    right = Matrix::from_angles(t, 0, pitch.wrapping_neg(), 0).transform_word(right);
    [heading, pitch, atan.angle(right[1], right[0])]
}
#[cfg(test)]
mod matrix_tests {
    use super::*;
    #[test]
    fn matrix_rounding_order_word_saturation_and_identity_skip() {
        let t = TrigTable([0; 321]);
        assert_eq!(Matrix::from_angles(&t, 0, 0, 0), Matrix::IDENTITY);
        assert_eq!(
            Matrix::IDENTITY.transform([1000, -1000, 0]).unwrap(),
            [999, -1000, 0]
        );
        assert_eq!(Matrix::IDENTITY.compose(Matrix::IDENTITY).0[0][0], 32766);
        let m = Matrix([[32767; 3]; 3]);
        assert!(m.transform([i32::MAX; 3]).is_err());
        assert_eq!(m.transform_word([20000; 3]), [32767; 3]);
    }
    #[test]
    fn atan_bounds_quadrants_and_no_offset_fast_path() {
        assert!(AtanTable::parse(&[0; 1027]).is_err());
        let a = AtanTable([0; 514]);
        assert_eq!(a.angle(0, 1), 0);
        assert_eq!(a.angle(1, 0), 0x3ffc);
        assert_eq!(a.angle(0, -1), 0x7ff8);
        assert_eq!(
            cockpit_offset(&TrigTable([0; 321]), &a, [1, 2, 3], 0, 0),
            [1, 2, 3]
        );
    }
}

/// FA 0x476cba..0x476d67. Movement -> display, then slip/AoA, bank, turbulence.
/// Returns display PA words and the native heading-chart flag-toggle predicate.
pub fn cockpit_angles(
    t: &TrigTable,
    atan: &AtanTable,
    movement: super::integration::MovementAngles,
    offsets_f8: [i32; 3],
    turbulence_f8: [i32; 2],
    previous_heading: i16,
) -> Result<([i16; 3], bool)> {
    let mut body = [
        degrees_to_pa(movement.heading)?,
        degrees_to_pa(movement.pitch)?.clamp(-0x3ffc, 0x3ffc),
        degrees_to_pa(movement.roll.wrapping_neg())?,
    ];
    body = cockpit_offset(
        t,
        atan,
        body,
        degrees_to_pa(offsets_f8[0])?,
        degrees_to_pa(offsets_f8[1])?,
    );
    body[2] = body[2].wrapping_add(degrees_to_pa(offsets_f8[2])?);
    body = cockpit_offset(
        t,
        atan,
        body,
        degrees_to_pa(turbulence_f8[0])?,
        degrees_to_pa(turbulence_f8[1])?,
    );
    let toggle = (body[0].wrapping_sub(previous_heading) as i32).abs() > 0x78dc;
    Ok((body, toggle))
}
